//! The plan queue. Think pushes `(urgency, agent)`; this system pops at most
//! `plan_budget_per_tick` entries (and stops once the per-tick expansion
//! budget is spent), binds the goal's target, observes the world, runs the
//! planner, reserves shared resources and installs the plan. Entries older
//! than `PLAN_QUEUE_MAX_AGE` are dropped and the agent idles until its next
//! think.

use crate::components::{Brain, Building, BuildingKind, GoalKind, Household, Position};
use crate::entity::EntityId;
use crate::exec::{routine, ExecState, ReservationKind};
use crate::goap::{self, ActionKind, Limits, LocationKey, Plan, PlanCtx, StealSource, WorldState};
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
        // Socialise, Court, GangWork, Arrest, Bury, Fight bind with M4–M5 data.
        _ => None,
    }
}

/// Plan one agent's current goal. Returns the expansions used.
pub fn plan_for(world: &mut World, id: EntityId, goal: GoalKind) -> usize {
    let tick = world.tick;
    // Goals that bypass the planner.
    let bypass = match goal {
        GoalKind::Idle => routine::idle_plan(world, id),
        GoalKind::Work => routine::commute_plan(world, id),
        _ => None,
    };
    if let Some(plan) = bypass {
        install(world, id, plan);
        return 0;
    }
    let Some(goal_state) = goap::goal_state(goal) else {
        world.goal_failed(id, goal);
        return 0;
    };

    let target = bind_target(world, id, goal);
    let ctx = PlanCtx::build(world, id, target);
    let start = WorldState::observe(world, id, target);
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

fn install(world: &mut World, id: EntityId, plan: Plan) {
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.plan = Some(plan);
        b.plan_step = 0;
        b.exec = ExecState::Idle;
    }
}

/// Reserve the shared resources a plan consumes, for `reservation_ttl` ticks.
fn reserve_for(world: &mut World, id: EntityId, plan: &Plan) {
    let expires = world.tick + world.config.exec.reservation_ttl;
    let market = world.building_of_kind(BuildingKind::Market);
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    for step in &plan.steps {
        let kind = match step.action {
            ActionKind::BuyFood => market.map(|b| ReservationKind::FoodUnits { building: b, units: 1 }),
            ActionKind::StealFood(StealSource::Market) => {
                market.map(|b| ReservationKind::FoodUnits { building: b, units: 2 })
            }
            ActionKind::StealFood(StealSource::Home) => {
                plan.target.map(|b| ReservationKind::FoodUnits { building: b, units: 2 })
            }
            ActionKind::EatAtHome => home.map(|b| ReservationKind::FoodUnits { building: b, units: 1 }),
            ActionKind::GoTo(LocationKey::Home) => None,
            _ => None,
        };
        if let Some(kind) = kind {
            world.reserve(id, kind, expires);
        }
    }
}
