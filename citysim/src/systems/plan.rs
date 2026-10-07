//! The plan queue. Think pushes `(urgency, agent)`; this system pops at most
//! `plan_budget_per_tick` entries (and stops once the per-tick expansion
//! budget is spent), binds the goal's target, observes the world, runs the
//! planner, reserves shared resources and installs the plan. Entries older
//! than `PLAN_QUEUE_MAX_AGE` are dropped and the agent idles until its next
//! think.

use crate::components::{Brain, Building, BuildingKind, GoalKind, Household, Position};
use crate::entity::EntityId;
use crate::exec::{routine, ExecState, ReservationKind};
use crate::goap::{self, ActionKind, Limits, Plan, PlanCtx, StealSource, WorldState};
use crate::time::Tick;
use crate::world::{Urgency, World};

/// Queue entries older than this are dropped.
pub const PLAN_QUEUE_MAX_AGE: Tick = 90;

pub fn run(world: &mut World) {
    let tick = world.tick;
    let max_plans = world.config.brain.plan_budget_per_tick;
    let max_expansions = world.config.brain.plan_expansion_budget_per_tick;
    let mut planned = 0usize;
    let mut expansions = 0usize;

    while planned < max_plans && expansions < max_expansions {
        let Some((&(urgency, id), &enqueued)) = world.plan_queue.iter().next() else { break };
        world.plan_queue.remove(&(urgency, id));
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.plan_queued = false;
        }
        if tick.saturating_sub(enqueued) > PLAN_QUEUE_MAX_AGE {
            continue; // stale: the agent idles until its next think
        }
        let Some(goal) = world.comp::<Brain>(id).filter(|b| b.plan.is_none()).and_then(|b| b.current_goal) else {
            continue;
        };
        planned += 1;
        expansions += plan_for(world, id, goal);
    }
}

/// Queue an agent for planning. No-op if already queued.
pub fn enqueue(world: &mut World, id: EntityId, urgency: f32) {
    let tick = world.tick;
    let Some(b) = world.comp_mut::<Brain>(id) else { return };
    if b.plan_queued {
        return;
    }
    b.plan_queued = true;
    world.plan_queue.insert((Urgency(ordered_float::OrderedFloat(urgency)), id), tick);
}

/// Bind `Plan.target` for a goal (spec › Target binding).
pub fn bind_target(world: &World, id: EntityId, goal: GoalKind) -> Option<EntityId> {
    match goal {
        // Eat binds the nearest other Home with pantry > 0 (for StealFood(Home)).
        GoalKind::Eat => {
            let home = world.comp::<Household>(id).and_then(|h| h.home);
            let tile = world.comp::<Position>(id)?.tile;
            world
                .buildings_by_kind
                .get(&BuildingKind::Home)?
                .iter()
                .copied()
                .filter(|&h| Some(h) != home)
                .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
                .filter(|(_, b)| b.stock_food > 0 && !b.demolished)
                .min_by_key(|(h, b)| (b.door.manhattan(tile), h.index))
                .map(|(h, _)| h)
        }
        // Arrest binds the located warrant suspect nearest the guard.
        GoalKind::Arrest => {
            let tile = world.comp::<Position>(id)?.tile;
            let located =
                crate::systems::law::located_suspects(world, Some(crate::systems::law::Pursuer { tile, guard: id }));
            let dist = |s: EntityId| world.last_seen.get(&s).map_or(u32::MAX, |&(t, _)| t.manhattan(tile));
            if world.config.gossip.enabled {
                // M15 W34: the hottest first, then the nearest.
                let heat = |s: EntityId| (crate::systems::reputation::rep(world, s).heat * 10.0).round() as i32;
                located.into_iter().min_by_key(|&s| (-heat(s), dist(s), s.index))
            } else {
                located.into_iter().min_by_key(|&s| (dist(s), s.index))
            }
        }
        // Socialise and Court bind the co-located agent with the highest affinity
        // who has no Partner reservation (Court: unmarried, known candidate first).
        GoalKind::Socialise => crate::systems::social::best_colocated_partner(world, id, -1.0, false),
        GoalKind::Court => crate::systems::social::court_target(world, id),
        // GangWork binds the Extort target per the Gang section (M12: under
        // the Squat order, the derelict to take).
        GoalKind::GangWork => crate::systems::gang::extort_target(world, id),
        // M12 D27: the derelict with a slot nearest the agent.
        GoalKind::Squat => crate::systems::street::squat_target(world, id),
        // M13 D29: the seller of the best offer (`plan_for` stores the pick).
        GoalKind::Shop => crate::systems::assets::shop_choice(world, id, true).map(|o| o.seller),
        // M13 D34: the nearest open Clinic.
        GoalKind::Treat => {
            let tile = world.comp::<Position>(id)?.tile;
            crate::systems::assets::nearest_seller(world, BuildingKind::Clinic, tile, true)
        }
        // M13 D35: the nearest body within reach.
        GoalKind::Loot => crate::systems::chrome::loot_target(world, id),
        // M13 D39: the nearest Stims source, unless a dose is in hand.
        GoalKind::GetHigh => {
            if world.comp::<crate::components::Inventory>(id).is_some_and(|i| i.stims > 0) {
                None
            } else {
                crate::systems::stims::stim_source(world, id)
            }
        }
        // M13 D33: an episode's quarry, the nearest living body in sight.
        GoalKind::Fight if crate::systems::chrome::in_episode(world, id) => {
            crate::systems::chrome::episode_target(world, id)
        }
        // M13 review: a thief already holding a stolen vehicle fences it
        // first and binds no second one (the Hideout is in reach through
        // `gang::hideout_for`; binding it as the target would make it a
        // `TargetHome` to steal food from).
        GoalKind::Earn if crate::systems::vehicles::stolen_held_by(world, id).is_some() => None,
        // M13 D26: the nearest street-parked vehicle a thief may take (and
        // a gang would pay for).
        GoalKind::Earn if crate::systems::vehicles::would_steal(world, id) => {
            crate::systems::vehicles::steal_target(world, id)
                .filter(|&v| crate::systems::vehicles::can_fence(world, id, Some(v)))
        }
        // Raid binds the expedition's building, the rival Hideout or the Jail
        // (the inspector shows it as the plan target).
        // M12 D31/D39: a riot's target, a corp building, the Jail or the rival Hideout.
        GoalKind::Raid => crate::systems::raid::target_building(world, id),
        // Fight binds the hostile within 4, or the revenge subject.
        GoalKind::Fight => {
            let tile = world.comp::<Position>(id)?.tile;
            let two_days = world.tick.saturating_sub(2 * crate::time::TICKS_PER_DAY);
            let mem = world.comp::<crate::components::Memory>(id);
            let fought = |o: EntityId| {
                mem.is_some_and(|m| {
                    m.entries.iter().any(|e| {
                        e.kind == crate::components::MemoryKind::Fought && e.subject == Some(o) && e.tick >= two_days
                    })
                })
            };
            let robbed_by = |o: EntityId| {
                mem.is_some_and(|m| {
                    m.entries.iter().any(|e| e.kind == crate::components::MemoryKind::WasRobbed && e.subject == Some(o))
                })
            };
            world
                .enemies_of(id)
                .filter(|&o| world.has::<Brain>(o) && crate::systems::law::near(world, id, o, 4) && !fought(o))
                .filter(|&o| crate::utility::goals::wronged_by(world, id, o))
                .min_by_key(|&o| {
                    (!robbed_by(o), world.comp::<Position>(o).map_or(u32::MAX, |p| p.tile.manhattan(tile)), o.index)
                })
        }
        // Bury binds the nearest unburied corpse the agent knows of (SawCorpse),
        // skipping corpses someone else has reserved.
        GoalKind::Bury => {
            let tile = world.comp::<Position>(id)?.tile;
            let reserved: Vec<EntityId> = world
                .reservations
                .iter()
                .filter(|(&h, _)| h != id)
                .flat_map(|(_, rs)| rs.iter())
                .filter_map(|r| match r.kind {
                    ReservationKind::Corpse { corpse } => Some(corpse),
                    _ => None,
                })
                .collect();
            let mem = world.comp::<crate::components::Memory>(id)?;
            mem.entries
                .iter()
                .filter(|e| e.kind == crate::components::MemoryKind::SawCorpse)
                .filter_map(|e| e.subject)
                .filter(|&c| world.comp::<crate::components::Corpse>(c).is_some_and(|k| !k.buried))
                .filter(|c| !reserved.contains(c))
                .min_by_key(|&c| (world.comp::<Position>(c).map_or(u32::MAX, |p| p.tile.manhattan(tile)), c.index))
        }
        _ => None,
    }
}

/// Plan one agent's current goal. Returns the expansions used.
pub fn plan_for(world: &mut World, id: EntityId, goal: GoalKind) -> usize {
    let tick = world.tick;
    // Goals that bypass the planner.
    let bypass = match goal {
        GoalKind::Idle => routine::idle_plan(world, id),
        // L1: an exec's office hours (no Job, so no commute plan).
        GoalKind::Work => routine::commute_plan(world, id).or_else(|| crate::systems::life::exec_plan(world, id)),
        // M12 phase 4: the expedition chain, built directly.
        GoalKind::Raid => routine::raid_plan(world, id),
        _ => None,
    };
    if let Some(plan) = bypass {
        install(world, id, plan);
        return 0;
    }
    // M15 W19/W35: the Hunt's and Guard the body's scripted plans.
    if matches!(goal, GoalKind::Hunt | GoalKind::GuardBody) {
        let plan = if goal == GoalKind::Hunt {
            crate::systems::hunt::plan(world, id)
        } else {
            crate::systems::grudges::guard_body_plan(world, id)
        };
        match plan {
            Some(plan) => {
                let n = plan.steps.len();
                install(world, id, plan);
                return n;
            }
            None => {
                world.cool_goal(id, goal);
                return 0;
            }
        }
    }
    // M14 V29: the Hack chain is built directly (`[GoTo(Chair)] -> JackIn`,
    // the freelance `RunOrder` written at bind; or `[GoTo(DataBuyer)] ->
    // SellData` for a deck still holding Data), never searched.
    if goal == GoalKind::Hack {
        match crate::systems::virt::hack_plan(world, id) {
            Some(plan) => {
                let n = plan.steps.len();
                install(world, id, plan);
                return n;
            }
            None => {
                world.cool_goal(id, goal);
                return 0;
            }
        }
    }
    let Some(goal_state) = goap::goal_state(goal) else {
        world.goal_failed(id, goal);
        return 0;
    };

    if goal == GoalKind::Patrol {
        // A fresh route and leg count per shift; a stale route from an earlier
        // day must not carry its legs over.
        let key = world.comp::<crate::components::Job>(id).map(|j| j.next_shift_key(world.tick));
        let needs_route =
            world.comp::<Brain>(id).is_some_and(|b| b.patrol_route.is_empty() || b.patrol_shift_key != key);
        if needs_route {
            let route = crate::systems::law::new_patrol_route(world, id);
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.patrol_route = route;
                b.patrol_legs = 0;
                b.patrol_shift_key = key;
            }
        }
    }
    // M13 D29: what to buy is fixed with the seller. Review fix: the offer
    // the think just scored is reused (`World::shop_offers`, this tick
    // only), not computed again for the target and again for the pick.
    let offer = if goal == GoalKind::Shop {
        let cached = world.shop_offers.remove(&id).filter(|(t, _)| *t == tick).map(|(_, o)| o);
        let offer = cached.unwrap_or_else(|| crate::systems::assets::shop_choice(world, id, true));
        if offer.is_none() {
            world.cool_goal(id, goal);
            return 0;
        }
        offer
    } else {
        None
    };
    let target = match &offer {
        Some(o) => Some(o.seller),
        None => bind_target(world, id, goal),
    };
    // M13 D33: a berserker with nobody in sight roams.
    if goal == GoalKind::Fight && target.is_none() && crate::systems::chrome::in_episode(world, id) {
        let step = crate::components::ActionInstance { action: ActionKind::Wander, target: None, tile: None };
        install(world, id, Plan { goal, target: None, steps: vec![step], started_tick: tick });
        return 0;
    }
    if goal == GoalKind::GangWork {
        crate::systems::gang::note_gang_work(world, id);
        // M13 D36: a Statistical Harvest target is promoted to Coarse at bind.
        if let Some(t) =
            target.filter(|&t| world.comp::<Brain>(t).is_some_and(|b| b.lod == crate::components::Lod::Statistical))
        {
            crate::systems::lod::set_lod(world, t, crate::components::Lod::Coarse);
        }
    }
    if let Some(o) = offer {
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.shop_pick = Some(o.pick);
        }
    }
    let ctx = PlanCtx::build_for(world, id, target, goal);
    let start = WorldState::observe(world, id, target);
    // M13 phase 5 (throughput): the dealer's chain and the claim on a bound
    // Home are built directly. A* needed ~420 expansions for `GoTo(Hideout)
    // -> PickUp -> GoTo(Seller) -> Deal` and up to ~200 for `GoTo(TargetHome)
    // -> Extort` (M13's UseStim, BuyStims and the like widen every node)
    // against the 200 cap, so those plans exhausted the cap and failed (69
    // a day by day 90, 8.6 ms of a 160 ms day), and a dealer sometimes
    // bought another dealer's doses to deal them. M13 review: gating each
    // M13 action on its goal (`PlanCtx::goal`) does not make them plannable:
    // the plateau is the cheap pre-M13 actions (a GoTo to every key, Wander,
    // Rest, Beg), so `GoTo(TargetHome) -> Extort` still takes ~255
    // expansions and the dealer's PickUp chain 267-485 (with BuyStims gone,
    // its old 140-312 shortcut through another dealer); without the chains
    // ~70 GangWork plans a day failed at the cap.
    if goal == GoalKind::GangWork {
        if let Some(kinds) = dealer_chain(&ctx, &start).or_else(|| claim_chain(&ctx, &start)) {
            let steps = kinds.iter().map(|k| k.instance(&ctx)).collect();
            let plan = Plan { goal, target, steps, started_tick: tick };
            reserve_for(world, id, &plan);
            install(world, id, plan);
            return kinds.len();
        }
    }
    // A shift is worked even when today's wage trip is blocked (short payment
    // already attempted): drop the HasWageDue key rather than skip the shift.
    // L1: wages are paid at the shift's end: the Work plan is the shift.
    let goal_state: crate::goap::GoalState = if goal == GoalKind::Work && (!ctx.wage_collectable || ctx.life) {
        goal_state.into_iter().filter(|&(k, _)| k != crate::goap::Key::HasWageDue).collect()
    } else if goal == GoalKind::Court && !start.has_partner_candidate {
        // Courtship in two visits: Flirt until affinity and trust clear the
        // proposal thresholds, Propose once they do. A single Flirt -> Propose
        // plan would fail at Propose's precondition re-check nearly every time.
        vec![(crate::goap::Key::HasPartnerCandidate, true)]
    } else {
        goal_state
    };
    // Fail fast: every unsatisfied goal key needs at least one feasible action
    // that produces it; otherwise A* would only exhaust its expansion cap.
    let hopeless = goal_state.iter().any(|&(key, value)| {
        start.get(key) != value
            && !crate::goap::actions::PLANNABLE.iter().any(|a| a.feasible(&ctx) && a.produces(key, value, &ctx))
    });
    if hopeless {
        world.push_event(
            crate::events::EventKind::PlanAborted,
            &[id],
            format!("{goal:?} unplannable (no feasible action)"),
        );
        world.cool_goal(id, goal);
        return 0;
    }
    let limits =
        Limits { max_expansions: world.config.brain.plan_max_expansions, max_len: world.config.brain.plan_max_len };
    match goap::planner::plan(&ctx, start, &goal_state, limits) {
        Ok(found) => {
            let steps = found.steps.iter().map(|k| k.instance(&ctx)).collect();
            let plan = Plan { goal, target, steps, started_tick: tick };
            reserve_for(world, id, &plan);
            install(world, id, plan);
            found.expansions
        }
        Err((err, used)) => {
            world.push_event(crate::events::EventKind::PlanAborted, &[id], format!("{goal:?} unplannable: {err:?}"));
            world.cool_goal(id, goal);
            used
        }
    }
}

/// A dealer's GangWork chain (D38): `[GoTo(Hideout)] -> PickUp` unless it
/// holds doses, then `[GoTo(Seller)] -> Deal`; `None` (the planner decides)
/// when the agent is no dealer, the task is done, or any step is infeasible.
fn dealer_chain(ctx: &PlanCtx, ws: &WorldState) -> Option<Vec<ActionKind>> {
    use crate::goap::LocationKey;
    if !ctx.dealer || ws.gang_task_done {
        return None;
    }
    let mut kinds = Vec::with_capacity(4);
    let mut at = ws.at;
    if !ws.has_stims {
        if ctx.hideout_stims < ctx.deal_batch {
            return None;
        }
        if at != LocationKey::Hideout {
            kinds.push(ActionKind::GoTo(LocationKey::Hideout));
            at = LocationKey::Hideout;
        }
        kinds.push(ActionKind::PickUp);
    }
    if at != LocationKey::Seller {
        kinds.push(ActionKind::GoTo(LocationKey::Seller));
    }
    kinds.push(ActionKind::Deal);
    kinds.iter().all(|k| k.feasible(ctx)).then_some(kinds)
}

/// GangWork on a bound Home (Expand, Contest: `Extort`; Squat: `Occupy`):
/// `[GoTo(TargetHome)] -> Extort | Occupy`, the first feasible; `None` when
/// done or neither is feasible.
fn claim_chain(ctx: &PlanCtx, ws: &WorldState) -> Option<Vec<ActionKind>> {
    use crate::goap::LocationKey;
    if ws.gang_task_done {
        return None;
    }
    [ActionKind::Extort, ActionKind::Occupy].into_iter().find_map(|act| {
        let mut kinds = Vec::with_capacity(2);
        if ws.at != LocationKey::TargetHome {
            kinds.push(ActionKind::GoTo(LocationKey::TargetHome));
        }
        kinds.push(act);
        (act.produces(crate::goap::Key::GangTaskDone, true, ctx) && kinds.iter().all(|k| k.feasible(ctx)))
            .then_some(kinds)
    })
}

fn install(world: &mut World, id: EntityId, plan: Plan) {
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.plan = Some(plan);
        b.plan_step = 0;
        b.exec = ExecState::Idle;
        b.chase_hops = 0;
    }
}

/// Reserve the shared resources a plan consumes, for `reservation_ttl` ticks.
fn reserve_for(world: &mut World, id: EntityId, plan: &Plan) {
    let expires = world.tick + world.config.exec.reservation_ttl;
    // The plan's Market: the one the agent is in, else the nearest (M10 D21).
    let market = world.local(id, BuildingKind::Market);
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    for step in &plan.steps {
        let kind = match step.action {
            ActionKind::BuyFood => {
                let units = crate::systems::economy::buy_quantity(world, id, market).max(1);
                market.map(|b| ReservationKind::FoodUnits { building: b, units })
            }
            ActionKind::StealFood(StealSource::Market) => {
                market.map(|b| ReservationKind::FoodUnits { building: b, units: 2 })
            }
            ActionKind::StealFood(StealSource::Home) => {
                plan.target.map(|b| ReservationKind::FoodUnits { building: b, units: 2 })
            }
            ActionKind::EatAtHome => home.map(|b| ReservationKind::FoodUnits { building: b, units: 1 }),
            ActionKind::Chat | ActionKind::Flirt | ActionKind::Propose => {
                plan.target.map(|other| ReservationKind::Partner { other })
            }
            ActionKind::CarryCorpse => plan.target.map(|corpse| ReservationKind::Corpse { corpse }),
            _ => None,
        };
        if let Some(kind) = kind {
            world.reserve(id, kind, expires);
        }
    }
}
