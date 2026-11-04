use super::*;

/// Rank every living adult and hand out the tiers with hysteresis. Jailed
/// and emigrating agents are Coarse without taking a slot (L2 `[lod]
/// budget`: a sentenced agent is held Statistical unless `prisoner_body`).
pub(super) fn assign(world: &mut World) {
    let view = world.view_rect;
    let centre = match view {
        Some(r) => TilePos { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        None => TilePos { x: (world.map.w() / 2) as u16, y: (world.map.h() / 2) as u16 },
    };
    let on_screen = |t: TilePos| -> bool {
        let Some(r) = view else { return false };
        let (x, y) = (i32::from(t.x), i32::from(t.y));
        x >= i32::from(r.x) - VIEW_MARGIN
            && x < i32::from(r.x) + i32::from(r.w) + VIEW_MARGIN
            && y >= i32::from(r.y) - VIEW_MARGIN
            && y < i32::from(r.y) + i32::from(r.h) + VIEW_MARGIN
    };

    let mut ranked: Vec<(i32, u32, u32, EntityId)> = Vec::new();
    let harvest = harvest_targets(world);
    let sets = Some(budget_sets(world));
    let held = held_on(world);
    // scan-ok: hourly: assign
    for id in world.citizens() {
        let (Some(pos), Some(brain)) = (world.comp::<Position>(id), world.comp::<Brain>(id)) else { continue };
        // The view milestone V5 (OPEN-149): the camera ring holds its bodies outside the budget.
        if world.ring.holds(id) {
            continue;
        }
        // M17 phase 5: away abroad, no body.
        if crate::systems::abroad::is_away(world, id) {
            set_lod(world, id, Lod::Statistical);
            continue;
        }
        if world.has::<Sentence>(id) || brain.emigrating {
            // L1: a pinned prisoner keeps its pin (the shadow went blind in the cells).
            let pinned = brain.pinned;
            let lod = if pinned {
                Lod::Full
            } else if held && world.has::<Sentence>(id) && !prisoner_body(world, id) {
                // L2 (L21): held in the cells, no slot.
                Lod::Statistical
            } else {
                Lod::Coarse
            };
            set_lod(world, id, lod);
            continue;
        }
        // A wanted agent counts as a story step: a reported thief must stay on
        // the map for the arrest path to reach them.
        let story = brain.current_step().is_some_and(|s| story_relevant(s.action))
            || crate::systems::law::wanted(world, id)
            || world.runner_of.contains_key(&id);
        let class = class_with(world, id, &harvest, sets.as_ref());
        // M11: a fresh lawless evictee (D36: so the spiral's first link,
        // JoinGang, can be planned) and an agent able to found (D25) get a
        // body; neither can happen in the hourly table.
        let m11 =
            crate::systems::classes::evicted_desperate(world, id) || crate::systems::founding::can_found(world, id);
        let priority = i32::from(on_screen(pos.tile)) * 4 + class + i32::from(story || m11);
        ranked.push((-priority, pos.tile.manhattan(centre), id.index, id));
    }
    ranked.sort_unstable();
    let ids: Vec<EntityId> = ranked.iter().map(|&(_, _, _, id)| id).collect();
    let prio: Vec<i32> = ranked.iter().map(|&(p, _, _, _)| -p).collect();
    let current: Vec<Lod> = ids.iter().map(|&id| world.comp::<Brain>(id).map_or(Lod::Coarse, |b| b.lod)).collect();

    let max_full = world.config.lod.max_full;
    let max_coarse = world.config.lod.max_coarse;
    let mut tier: Vec<Lod> = (0..ids.len())
        .map(|rank| {
            if rank < max_full {
                Lod::Full
            } else if rank < max_full + max_coarse {
                Lod::Coarse
            } else {
                Lod::Statistical
            }
        })
        .collect();
    // Hysteresis at each boundary: an incumbent ranked just past it keeps its
    // tier against a newcomer of equal priority (interchangeable agents do
    // not swap tiers over a tile of distance); a pinned or on-screen or
    // story newcomer always takes the slot. Newcomers at the Coarse line are
    // the Statistical: a Full agent being demoted keeps its plan as Coarse.
    for (upper, boundary) in [(Lod::Full, max_full), (Lod::Coarse, max_full + max_coarse)] {
        let lower = if upper == Lod::Full { Lod::Coarse } else { Lod::Statistical };
        let is_newcomer =
            |r: usize| if upper == Lod::Full { current[r] != Lod::Full } else { current[r] == Lod::Statistical };
        let band_end = (boundary + HYSTERESIS_BAND).min(ids.len());
        for rank in boundary..band_end {
            if current[rank] != upper || tier[rank] == upper {
                continue;
            }
            let newcomer = (0..boundary).rev().find(|&r| tier[r] == upper && is_newcomer(r) && prio[r] == prio[rank]);
            if let Some(r) = newcomer {
                tier[r] = lower;
                tier[rank] = upper;
            }
        }
    }
    // L2 shadow fixes item 7: a body within its tier dwell (or mid-purchase)
    // keeps a Coarse slot against an equal-priority newcomer, the lowest
    // ranked first (an unpinned stand-in flapped Coarse/Statistical every
    // ~3 h: each demotion aborted a paid Enjoy and snapped her home).
    let boundary = (max_full + max_coarse).min(ids.len());
    for rank in boundary..ids.len() {
        if tier[rank] != Lod::Statistical || current[rank] == Lod::Statistical {
            continue;
        }
        if !crate::systems::fixes::dwell_holds(world, ids[rank]) {
            continue;
        }
        let newcomer = (0..boundary)
            .rev()
            .find(|&r| tier[r] == Lod::Coarse && current[r] == Lod::Statistical && prio[r] == prio[rank]);
        if let Some(r) = newcomer {
            tier[r] = Lod::Statistical;
            tier[rank] = Lod::Coarse;
            world.stats.current.living.fix_dwell += 1;
        }
    }
    // ... and one demoted within its dwell is not promoted back over an
    // equal-priority incumbent (she was Statistical for an hour, then a
    // body again, a dozen times a week).
    for r in (0..boundary).rev() {
        if tier[r] != Lod::Coarse || current[r] != Lod::Statistical {
            continue;
        }
        if !crate::systems::fixes::dwell_recent(world, ids[r]) {
            continue;
        }
        let incumbent = (boundary..ids.len())
            .find(|&k| tier[k] == Lod::Statistical && current[k] != Lod::Statistical && prio[k] == prio[r]);
        if let Some(k) = incumbent {
            tier[k] = Lod::Coarse;
            tier[r] = Lod::Statistical;
            world.stats.current.living.fix_dwell += 1;
        }
    }
    for (i, &id) in ids.iter().enumerate() {
        set_lod(world, id, tier[i]);
    }
}

/// M13 D46: the agents some gang is harvesting now (its order is Harvest
/// and it has a cached target).
fn harvest_targets(world: &World) -> Vec<EntityId> {
    world
        .gangs()
        .into_iter()
        .filter_map(|g| world.comp::<crate::components::Gang>(g))
        .filter(|g| g.order == crate::components::Order::Harvest)
        .filter_map(|g| g.harvest_target)
        .collect()
}

/// An agent's rank class in the hourly LOD assignment: pinned 5, the
/// watch 3, gang members (and those ranked with them) 2, else 0.
pub fn rank_class(world: &World, id: EntityId) -> i32 {
    let sets = Some(budget_sets(world));
    class_with(world, id, &harvest_targets(world), sets.as_ref())
}

/// `sets`: L2 (L22) under `[lod] budget`, a gang member outside its gang's
/// quota and a private guard outside `private_quota` rank 0, and the
/// public watch ranks 3 only on duty (`watch_on_duty`).
fn class_with(world: &World, id: EntityId, harvest: &[EntityId], sets: Option<&BudgetSets>) -> i32 {
    let Some(brain) = world.comp::<Brain>(id) else { return 0 };
    // Gang members are never Statistical: the hourly table has no orders,
    // claims or raids, and two gangs fit inside the Coarse budget.
    // M12 D31 (plan risk 3): a rioter of a live riot ranks with them,
    // from `riot_promote_hours` before the muster.
    let member = world.has::<crate::components::GangMember>(id) && sets.is_none_or(|s| s.gang.contains(&id));
    let gang = member || crate::systems::riot::promoted(world, id);
    // M13 D46: a berserker, an abductee in tow and a gang's Harvest target
    // (while that gang's order is Harvest) too.
    let gang = gang
        || brain.abducted_by.is_some()
        // M16b phase 3: the downed keep a body (inside `wounded_quota`).
        || (!world.wounded.is_empty() && world.wounded.contains_key(&id))
        // Pressure pass R3: the people at hand of a body bleeding keep one.
        || (!world.wounded.is_empty() && crate::systems::care::bystander(world, id))
        || harvest.contains(&id)
        || (!world.episodes.is_empty() && world.episodes.contains(&id));
    // M10 D20: guards rank with gang members, or at 2,000 the gangs take
    // every Coarse slot and the whole watch is Statistical. Gravediggers
    // too: the hourly table cannot bury, and a Statistical digger never
    // even learns of a corpse (it keeps no SawCorpse memory).
    let guard = body_role(world, id);
    // M11: a private guard keeps a body but ranks with the gangs: at the
    // watch's rank the twelve took Full slots and walked their corp's
    // Blocks tile by tile (-800 ticks/s at 2,000).
    let private = guard && crate::systems::law::is_private_guard(world, id);
    let private_ranked = private && sets.is_none_or(|s| s.private.contains(&id));
    let watch =
        guard && !private && (sets.is_none() || !world.config.lod.watch_on_shift_only || watch_on_duty(world, id));
    // Pinned tops the ladder, then the watch (3), then gang members (2):
    // at the Coarse cap the farthest gang member falls first, not a guard.
    // M14 V13: a runner seated or holding a run order (above the watch).
    let runner =
        !world.run_orders.is_empty() && (world.runner_of.contains_key(&id) || world.run_orders.contains_key(&id));
    // M15 W22: a hunter and a hunted target rank with the runners.
    let runner =
        runner || (!world.hunts.is_empty() && (world.hunts.contains_key(&id) || world.hunted_by.contains_key(&id)));
    // M16a (plan C35, phase 4): the parties of a record worked live (its
    // taker, crew and target) inside `contract_quota`; without the hour's
    // sets, every running taker, chased target and mission crew.
    let contract = match sets {
        Some(s) => s.contract.contains(&id),
        None => {
            (!world.contract_runs.is_empty()
                && (world.contract_runs.contains_key(&id) || world.chased_by.contains_key(&id)))
                || (!world.missions.is_empty() && world.mission_of.contains_key(&id))
        }
    };
    let runner = runner || contract;
    // M16a (plan C35, phase 4): a Fixer's owner in the matching window,
    // inside `fixer_quota` (ranked with the gangs).
    let fixer = sets.is_some_and(|s| s.fixer.contains(&id));
    // The island pass phase 8 (OPEN-168): a Garage's quota of cab drivers
    // on shift keep a body, so a fare can be driven by one at 20k.
    let fixer = fixer || sets.is_some_and(|s| !s.cab.is_empty() && s.cab.contains(&id));
    if brain.pinned {
        5
    } else if runner {
        4
    } else if watch {
        3
    } else if gang || private_ranked || fixer {
        2
    } else {
        0
    }
}

/// Change an agent's LOD, converting its execution state per the transition
/// table: a walk becomes a timed arrival and back; Use and Wait keep their
/// remaining duration; a demotion to Statistical abandons the plan and snaps
/// the agent to the door its day phase implies; a promotion places it there.
pub fn set_lod(world: &mut World, id: EntityId, lod: Lod) {
    set_lod_at(world, id, lod, None);
}

/// `set_lod`, with a promotion out of Statistical placed inside `place` and a demotion to Statistical at its door
/// (the camera ring's: the building its schedule has it in now, `ring::schedule_door`) instead of its phase's door.
pub fn set_lod_at(world: &mut World, id: EntityId, lod: Lod, place: Option<EntityId>) {
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let from = brain.lod;
    if from == lod {
        return;
    }
    // M14 V13: a body jacked in is never Statistical; M15 W22: nor a
    // hunter or a hunted target.
    // M17 phase 5: a crew away abroad is Statistical whatever holds it.
    let away = crate::systems::abroad::is_away(world, id);
    if lod == Lod::Statistical
        && !away
        && (world.runner_of.contains_key(&id) || world.hunts.contains_key(&id) || world.hunted_by.contains_key(&id))
    {
        return;
    }
    // M16a (plan C17, C35 phase 4): nor a party of a record worked live
    // (the contract set: taker, crew, target, inside `contract_quota`).
    if lod == Lod::Statistical && !away && crate::systems::contracts::holds_body(world, id) {
        return;
    }
    // L2 shadow fixes item 7: the tier's start, for the dwell (after the
    // refusal above: a refused body has not changed tier).
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.lod_since = now;
    }
    let Some(brain) = world.comp::<Brain>(id) else { return };
    if lod == Lod::Statistical {
        if brain.plan.is_some() {
            let goal = brain.plan_goal();
            // The island pass phase 9: off screen the body is snapped below,
            // so a walk is not set down on its way first (no road search).
            if let Some(ExecState::GotoTimed { walk, .. }) = world.comp_mut::<Brain>(id).map(|b| &mut b.exec) {
                *walk = None;
            }
            world.abort_plan(id);
            world.push_event(EventKind::PlanAborted, &[id], format!("{goal:?} LodDemotion"));
        }
        // The island pass phase 6: off a train at once (no body to carry on
        // to the next stop off screen).
        if crate::systems::transit::trains_on(world) {
            crate::systems::motion::alight_now(world, id);
        }
        // M13 D46: no trip off screen; its own vehicles go home with it.
        crate::systems::vehicles::on_demoted(world, id);
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.lod = Lod::Statistical;
            b.exec = ExecState::Idle;
            b.current_goal = None;
            b.plan_queued = false;
        }
        world.retier(id);
        world.plan_queue.retain(|&(_, who), _| who != id);
        // L2 (L21): a held prisoner stays in the cells; its day is decayed
        // from here (`law::settle_held`).
        if held_on(world) && world.has::<Sentence>(id) {
            let now = world.tick;
            world.held_since.insert(id, now);
        } else {
            match place {
                Some(b) => world.stand_at_door(id, b),
                None => snap_to_phase_door(world, id, false),
            }
        }
        // M15 W2: off screen the heard store holds 3 rumours and no sighting.
        crate::systems::gossip::on_demoted(world, id);
        return;
    }
    if from == Lod::Statistical {
        // L2 (L21): the held hours since the last settlement, before the
        // body's per-tick decay takes over.
        if world.held_since.contains_key(&id) {
            crate::systems::law::settle_held(world, id);
            world.held_since.remove(&id);
        }
        let today = world.day();
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.lod = lod;
            b.exec = ExecState::Idle;
            b.current_goal = None;
            b.body_day = Some(today);
        }
        world.retier(id);
        // L2 (L21): a prisoner promoted out of the hold is still inside.
        if !(held_on(world) && world.has::<Sentence>(id)) {
            match place {
                Some(b) => {
                    world.stand_at_door(id, b);
                    world.enter_building(id, b);
                }
                None => snap_to_phase_door(world, id, true),
            }
        }
        // The island pass phase 7: a prisoner on a transfer run gets his
        // body aboard its van.
        crate::systems::stations::on_promoted(world, id);
        // A body's memories are never half-filled: its open victim holes bind
        // at the end of this tick's `lod::run`.
        if let Some(hs) = world.holes_by_agent.get(&id) {
            let hs = hs.clone();
            world.bind_queue.extend(hs);
        }
        // M16a (plan C17): a ledger record whose party became a body turns live.
        crate::systems::contracts::on_promoted(world, id);
        return;
    }
    // The island pass phase 5 (I3): a rider aboard a timed vehicle stays
    // aboard a promotion (his leg runs on at Full).
    let riding = crate::systems::motion::vehicle_of(world, id).is_some();
    // The island pass phase 9 (OPEN-181): a Coarse walker promoted mid-walk
    // is set down where his walk has him now; the Full walk goes on from
    // there.
    if lod == Lod::Full && !riding {
        exec::settle_walk(world, id);
    }
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let exec = brain.exec.clone();
    let new_exec = match (exec, lod) {
        (state @ ExecState::GotoTimed { .. }, Lod::Full) if riding => state,
        (ExecState::Goto { target, .. }, Lod::Coarse) => exec::timed_goto(world, id, target),
        (ExecState::GotoTimed { target, .. }, Lod::Full) => match exec::walking_goto(world, id, target) {
            Some(state) => state,
            // Unreachable on foot: let the step fail and the agent replan.
            None => ExecState::Idle,
        },
        (other, _) => other,
    };
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.lod = lod;
        b.exec = new_exec;
    }
    // A Full driver demoted to Coarse: the vehicle runs his timed leg.
    if riding && lod == Lod::Coarse {
        crate::systems::motion::to_timed(world, id);
    }
    // The island pass phase 7: a Full guard walking an escort demoted to
    // Coarse leads the suspect on the timed walk.
    if !riding && lod == Lod::Coarse {
        crate::exec::escort_to_timed(world, id);
    }
    world.retier(id);
}

/// The door an off-screen agent stands at for the current phase. Demotion:
/// Home at Night/Morning, workplace in the Work phase if employed, else
/// Home; Market if homeless. Promotion adds the Evening venue: the Bar for
/// the sociable, else the Market.
fn snap_to_phase_door(world: &mut World, id: EntityId, promotion: bool) {
    let phase = world.phase();
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let workplace = world.comp::<Job>(id).and_then(|j| j.employer);
    let sociable = world.comp::<Personality>(id).is_some_and(|p| p.sociability >= 0.5);
    // Flip readiness (wages on; a base-city bug the P10 retirement ungates):
    // a worker on its own shift stands at the workplace whatever the phase
    // (a Night Porter promoted at 00:00 stood at a Market 140 tiles off and
    // walked the shift away), and a homeless worker's night is spent near
    // the workplace, not at the cheapest Market across town.
    let wages = crate::systems::wages::on(world);
    let on_shift = wages
        && world.comp::<Job>(id).is_some_and(|j| {
            j.employer.is_some()
                && j.on_shift(world.tick_of_day())
                && crate::exec::routine::workday_of(world, id, j, j.shift_key_at(world.tick))
        });
    let rough_worker = if wages && home.is_none() { workplace } else { None };
    let building = if on_shift { workplace } else { None }
        .or(match phase {
            DayPhase::Night | DayPhase::Morning => home,
            DayPhase::Work => workplace.or(home),
            DayPhase::Evening if promotion => {
                if sociable {
                    world.local(id, BuildingKind::Bar)
                } else {
                    world.local(id, BuildingKind::Market)
                }
            }
            DayPhase::Evening => home,
        })
        .or(rough_worker)
        .or_else(|| world.local(id, BuildingKind::Market));
    if let Some(b) = building {
        world.stand_at_door(id, b);
    }
}
