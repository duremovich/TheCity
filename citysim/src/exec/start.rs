use super::*;

/// M13 D49: a `GoTo` toward the agent's own workplace starts (Full and
/// Coarse): remember when and how far, door to door.
fn commute_started(world: &mut World, id: EntityId, target: &GotoTarget) {
    // Transit phase 3: the El's commutes are counted in an asset-free city
    // too (the calibration city the parity test replays).
    if !world.config.assets.enabled && !world.config.transit.enabled {
        return;
    }
    let employer = world.comp::<crate::components::Job>(id).and_then(|j| j.employer);
    if target.building.is_none() || target.building != employer {
        return;
    }
    // The pre-view pass phase 5: a segment's re-start keeps the commute's start.
    if legs::on(world) && world.commute_start.contains_key(&id) {
        return;
    }
    if crate::systems::shift::on(world) && crate::systems::shift::reentering_shift(world, id) {
        return;
    }
    let Some(from) = walk_origin(world, id) else { return };
    let tiles = from.manhattan(target.tile);
    if tiles > 0 {
        let tick = world.tick;
        world.commute_start.insert(id, (tick, tiles));
    }
}

/// M13 D49: an arrival at the workplace adds its ticks and tiles to the
/// day's walked or driven sums (`commute_tpt_*`).
pub(super) fn commute_arrived(world: &mut World, id: EntityId, driving: bool) {
    let Some((start, tiles)) = world.commute_start.remove(&id) else { return };
    let ticks = world.tick.saturating_sub(start);
    let rode = !world.transit.rode.is_empty() && world.transit.rode.remove(&id);
    if let Some(log) = world.commute_log.as_mut() {
        let mode = if rode { 2 } else { u8::from(driving) };
        log.push((ticks as u32, tiles, mode));
    }
    // Transit phase 2: the bodies' commutes and those that rode (the parity
    // test's ride share; nothing with transit off).
    if world.config.transit.enabled {
        let c = &mut world.stats.current.transit;
        c.commutes += 1;
        c.commute_rides += u32::from(rode);
    }
    // Transit: a commute that rode goes to its own sums (`commute_tpt_ride`).
    if rode {
        world.stats.current.transit.commute_ride_ticks += ticks;
        world.stats.current.transit.commute_ride_tiles += u64::from(tiles);
        return;
    }
    let i = if driving { 2 } else { 0 };
    world.commute_acc[i] += ticks;
    world.commute_acc[i + 1] += u64::from(tiles);
}

/// Begin a step: resolve a GoTo, or check preconditions and apply start
/// effects for an action.
pub(super) fn start_step(
    world: &mut World,
    id: EntityId,
    step: &crate::components::ActionInstance,
    lod: Lod,
) -> StepResult {
    let tick = world.tick;
    // Escort and FleeToHome are walks with a completion effect.
    let walk_key = match step.action {
        ActionKind::GoTo(key) => Some(key),
        ActionKind::Escort => Some(LocationKey::Jail),
        // L1: without a Home, the nearest refuge (`life::refuge`).
        ActionKind::FleeToHome => Some(crate::systems::life::refuge(world, id).map_or(LocationKey::Home, |(k, _)| k)),
        _ => None,
    };
    match walk_key {
        Some(key) => {
            // The pre-view pass phase 5: a walk (re)starting drops its old
            // segment (`segment` files the next one).
            if !world.segments.is_empty() {
                world.segments.remove(&id);
            }
            // The island pass phase 5 (I3): a walk starting aboard a vehicle
            // (a stale leg) steps off where it stands first.
            if !world.aboard.is_empty() {
                crate::systems::motion::alight(world, id);
            }
            // M15 W22: a hunter setting out to its intel promotes a
            // Statistical target.
            if key == LocationKey::Intel {
                crate::systems::hunt::on_goto_intel(world, id);
            }
            // The pre-view pass phase 2 (OPEN-137): a stations spot inside
            // the workplace is a walk across the room (timed, the tiles at
            // `move_ticks_full`, at most `[shift] station_move_max`), the
            // body kept inside.
            if key == LocationKey::Station {
                if let (Some(tile), Some(b)) = (step.tile, step.target) {
                    let inside = world.comp::<Position>(id).filter(|p| p.building == Some(b)).map(|p| p.tile);
                    if let Some(from) = inside {
                        if from == tile {
                            return StepResult::Done;
                        }
                        let arrive_tick = tick + crate::systems::shift::move_ticks(world, from, tile);
                        let target = GotoTarget { dest: key, tile, building: Some(b) };
                        if let Some(br) = world.comp_mut::<Brain>(id) {
                            br.exec = ExecState::GotoTimed {
                                target,
                                arrive_tick,
                                blocked_since: None,
                                walk: None,
                                climb: None,
                            };
                        }
                        return StepResult::Running;
                    }
                }
            }
            // Escort's target is the suspect, not the destination.
            let walk_target = if matches!(step.action, ActionKind::GoTo(_)) { step.target } else { None };
            let building = world.resolve_building(id, key, walk_target);
            // M18 P1: a picked street tile is the step's own.
            let picked = (key == LocationKey::Picked && building.is_none()).then_some(step.tile).flatten();
            let Some(tile) = picked.or_else(|| world.resolve_location(id, key, walk_target)) else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let Some(pos) = world.comp::<Position>(id).cloned() else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let already_there = match building {
                Some(b) => pos.building == Some(b),
                None => pos.tile == tile,
            };
            if already_there {
                return StepResult::Done;
            }
            let target = GotoTarget { dest: key, tile, building };
            // L2 shadow fixes item 6: the street fled from is avoided by the
            // spot pick for `avoid_spot_ticks`.
            if step.action == ActionKind::FleeToHome {
                let until = tick + world.config.life.avoid_spot_ticks;
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.avoid_spot = Some((pos.tile, until));
                }
            }
            // Lives at work phase 4 (§ 2.3): a cab driver's leg is a drive;
            // a commuter's GoTo to work may be a cab (the roll shared with the
            // Statistical tier), the leg timed at the cab's speed.
            if matches!(step.action, ActionKind::GoTo(_)) {
                let mut mode = "fare_leg";
                let street = fare_segment(world, id, &target)
                    .or_else(|| crate::systems::streetwork::fare_leg(world, id, &target))
                    .or_else(|| {
                        mode = "cab";
                        crate::systems::streetwork::full_cab(world, id, &target)
                    })
                    .or_else(|| {
                        mode = "exec_ride";
                        crate::systems::streetwork::exec_ride(world, id, &target)
                    })
                    .or_else(|| {
                        // The pressure pass phase 2: the rich or the hurried hail one.
                        mode = "cab";
                        crate::systems::streetwork::hail_cab(world, id, &target)
                    });
                if let Some(state) = street {
                    leglog::start(world, id, &target, mode);
                    commute_started(world, id, &target);
                    board_street(world, id, &target, &state, mode);
                    if let Some(b) = world.comp_mut::<Brain>(id) {
                        b.exec = state;
                    }
                    return StepResult::Running;
                }
            }
            // M13 D20: a GoTo rides when a vehicle is to hand (D22: a flyer hops).
            let driving = if matches!(step.action, ActionKind::GoTo(_)) {
                commute_started(world, id, &target);
                crate::systems::vehicles::begin_trip(world, id, &target)
            } else {
                None
            };
            // L1: an escort is a van ride of at most `escort_van_ticks` (the
            // suspect walked the guard's whole walk in cuffs, hours at a time).
            if step.action == ActionKind::Escort {
                let timed = match escort_leg(world, id, &target, lod) {
                    Some(s) => s,
                    None => {
                        let mut timed = timed_goto(world, id, target.clone());
                        if let ExecState::GotoTimed { arrive_tick, .. } = &mut timed {
                            *arrive_tick = (*arrive_tick).min(tick + world.config.life.escort_van_ticks);
                        }
                        timed
                    }
                };
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.exec = timed;
                }
                leglog::start(world, id, &target, "escort");
                return StepResult::Running;
            }
            // Transit T8, T12: on foot, a far GoTo may ride the El (the first
            // leg, the walk to the board stop, comes back). The pressure pass
            // F1: so may a FleeToHome (the night's walk home of the unsafe:
            // a Sleep's `[FleeToHome > Sleep]` walked 600-1,000 tiles, 10-20 h).
            if driving.is_none() && matches!(step.action, ActionKind::GoTo(_) | ActionKind::FleeToHome) {
                if let Some(state) = crate::systems::transit::try_ride(world, id, &target, lod) {
                    if let Some(b) = world.comp_mut::<Brain>(id) {
                        b.exec = state;
                    }
                    leglog::start(world, id, &target, "el");
                    return StepResult::Running;
                }
            }
            // The pre-view pass phase 5 (OPEN-139): a long walk walks in
            // segments; each end re-starts the step (`exec::legs`).
            if driving.is_none()
                && matches!(step.action, ActionKind::GoTo(_) | ActionKind::FleeToHome)
                && legs::on(world)
            {
                if let Some(state) = segment(world, id, &target, lod) {
                    leglog::start(world, id, &target, "walk");
                    if let Some(b) = world.comp_mut::<Brain>(id) {
                        b.exec = state;
                    }
                    return StepResult::Running;
                }
            }
            if world.leg_log.is_some() {
                let mode = match driving {
                    Some(crate::components::AssetKind::Flyer) => "fly",
                    Some(_) => "drive",
                    None => "walk",
                };
                leglog::start(world, id, &target, mode);
            }
            let state = match (driving, lod) {
                (Some(crate::components::AssetKind::Flyer), _) => crate::systems::vehicles::fly(world, id, target),
                (_, Lod::Coarse | Lod::Statistical) => timed_goto(world, id, target),
                (_, Lod::Full) => match walking_goto(world, id, target) {
                    Some(s) => s,
                    None => {
                        crate::systems::vehicles::end_trip(world, id, false);
                        return StepResult::Failed(FailReason::NoSuchPlace);
                    }
                },
            };
            if let Some(kind) = driving {
                board_trip(world, id, kind, &state);
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.exec = state;
            }
            StepResult::Running
        }
        None => {
            let kind = step.action;
            // L1: the partner a Chat or Flirt walked to may have left: talk to
            // whoever is here instead (the shadowed walked 2 h to a 1-minute
            // chat with nobody).
            let rebound;
            let step = if matches!(kind, ActionKind::Chat | ActionKind::Flirt | ActionKind::Propose) {
                rebound = rebind_partner(world, id, step);
                &rebound
            } else {
                step
            };
            // M14 V11: a `JackIn` begun before its order's `not_before` waits
            // in the chair (then the step is retried).
            if kind == ActionKind::JackIn {
                let due = world.run_orders.get(&id).map(|o| o.not_before).filter(|&t| t > tick);
                if let Some(until) = due {
                    if let Some(b) = world.comp_mut::<Brain>(id) {
                        b.exec = ExecState::Wait { until };
                    }
                    return StepResult::Running;
                }
            }
            // Round 5 U3 (OPEN-235): the soup line. Inside the kitchen with
            // the hour's bowls out, the body waits for the next hour; with
            // the pot dry, it is turned away.
            if kind == ActionKind::EatAlms {
                if let Some(until) = crate::systems::charity::queue_until(world, id, step.target) {
                    if let Some(b) = world.comp_mut::<Brain>(id) {
                        b.exec = ExecState::Wait { until };
                    }
                    return StepResult::Running;
                }
                if crate::systems::charity::turned_away(world, id, step.target) {
                    return StepResult::Failed(FailReason::StockGone);
                }
            }
            // M15 W19/W35: a scripted Hunt or GuardBody plan checks its
            // steps' own start conditions, not the planner's symbols.
            let goal = world.comp::<Brain>(id).and_then(|b| b.plan_goal());
            let scripted = goal.is_some_and(|g| {
                matches!(
                    g,
                    crate::components::GoalKind::Hunt
                        | crate::components::GoalKind::GuardBody
                        | crate::components::GoalKind::Contract
                        | crate::components::GoalKind::Commission
                        | crate::components::GoalKind::Rescue
                        | crate::components::GoalKind::Pitch
                        | crate::components::GoalKind::Visit
                        | crate::components::GoalKind::FindHome
                        | crate::components::GoalKind::Found
                )
            });
            // The pre-view pass phase 4 (P8): so does a sweeper's round (its
            // close is inside the Recycler, which the planner reads as no Beat).
            // Pressure pass round 2, R5 (OPEN-214): a scav's bin round
            // (`customers::scav_plan`) is GangWork's scripted Scavenge, not
            // the broke's Earn: the planner's `poor` symbol failed it
            // `PreconditionLost` on every scav with a meal's coins.
            // R5: every Scavenge checks its own start (the planner reads
            // someone else's Home as the street, so a broke extorter's
            // Earn Scavenge failed the same way inside the Block).
            let scav_round = kind == ActionKind::Scavenge;
            // R5: so is a try-out at a front (the planner's `at(Hideout)`
            // reads the building's kind).
            let tryout = goal == Some(crate::components::GoalKind::JoinGang) && crate::systems::crew::on(world);
            // Round 8 X1 (`OPEN-246`): a promise's ReportCrime at a Station or
            // the Precinct (the planner's `at(Hall)` reads the building's kind).
            let word = kind == ActionKind::ReportCrime
                && step.target.is_some_and(|s| crate::systems::law::owes_word_on(world, id, s));
            if scav_round
                || tryout
                || word
                || (goal == Some(crate::components::GoalKind::Work)
                    && (crate::systems::shift::scripted(world, id) || crate::systems::jobs::sweep_round_on(world, id)))
            {
                // The pre-view pass phase 2 (OPEN-137): a shape plan's steps
                // check their own start (the planner reads a worker inside
                // his own Market as at `Workplace`, so a break's `BuyFood`
                // there failed its symbols).
                if !actions::can_start(world, id, kind, step.target) {
                    return StepResult::Failed(FailReason::PreconditionLost);
                }
            } else if scripted {
                // M16a (plan C13): a Contract step's own start check (and
                // the Hit market's Commission, H9).
                let ok = if goal == Some(crate::components::GoalKind::Pitch) {
                    // Jobs & lives phase 2: the street's own steps.
                    crate::systems::streetwork::can_start(world, id, kind)
                } else if goal == Some(crate::components::GoalKind::Found) {
                    // The pressure pass F1: a Station's desk is no planner `Hall`.
                    actions::can_start(world, id, kind, step.target)
                } else if goal == Some(crate::components::GoalKind::FindHome) {
                    // M18 P5h: the walk as any walk; the Apply at the desk.
                    crate::systems::housing::can_start(world, id, kind, step.target)
                } else if goal == Some(crate::components::GoalKind::Visit) {
                    // Lives at work phase 2 (§ 3.5): the visit's own steps.
                    crate::systems::household::can_start(world, id, kind)
                } else if goal == Some(crate::components::GoalKind::Rescue) {
                    // M16b phase 3: the Rescue's own steps.
                    crate::systems::wounded::can_start(world, id, kind, step.target)
                } else if matches!(
                    goal,
                    Some(crate::components::GoalKind::Contract | crate::components::GoalKind::Commission)
                ) {
                    crate::systems::contracts::can_start(world, id, kind, step.target)
                } else {
                    crate::systems::hunt::can_start(world, id, kind, step.target)
                };
                if !ok {
                    return StepResult::Failed(FailReason::PreconditionLost);
                }
            } else {
                // Replan trigger (b): re-observe; a false precondition fails the step.
                // Lives at work phase 4b (§ 2.3): a corner's `Deal` or
                // `BuyStims` (no target: a tile, no planner location) checks
                // its own start condition only.
                let corner = step.target.is_none() && matches!(kind, ActionKind::Deal | ActionKind::BuyStims)
                    // Round 3 S5 (OPEN-217): the cab's wait at its rank (the street).
                    || (kind == ActionKind::Rest && crate::systems::streetwork::at_rank(world, id));
                let symbols = corner || {
                    let mut ctx = step_ctx(world, id, kind);
                    let mut ws = crate::goap::WorldState::observe(world, id, ctx.target);
                    // M18 P2: the human at the wheel supplies the will; the
                    // verb's willingness terms are waived (`player::waive`).
                    if crate::systems::player::drives(world, id) {
                        crate::systems::player::waive(world, id, kind, &mut ws, &mut ctx);
                    }
                    kind.preconditions(&ws, &ctx)
                };
                if !symbols || !actions::can_start(world, id, kind, step.target) {
                    // R5 (OPEN-214): a marcher at the door after the brawl
                    // (the raid done) reads its result, as `raid::resolve`'s.
                    if kind == ActionKind::Brawl {
                        if let Some(g) = world.gang_of(id) {
                            crate::systems::crew::late_to_raid(world, id, g);
                        }
                    }
                    // Lives at work phase 1 (§ 2.1): a venue step failing to
                    // start inside its venue against the door policy is a
                    // refusal at the door (the bouncer's encounter).
                    if matches!(kind, ActionKind::Enjoy | ActionKind::EatOut | ActionKind::Gamble) {
                        let here = world.comp::<Position>(id).and_then(|p| p.building);
                        if let Some(b) = step.target.filter(|&b| here == Some(b)) {
                            if !crate::systems::leisure::door_ok(world, id, b) {
                                crate::systems::service::refused(world, id, b);
                            }
                        }
                    }
                    return StepResult::Failed(FailReason::PreconditionLost);
                }
            }
            let dur = actions::duration(world, id, kind);
            // S2 (`OPEN-289`): a street stay on a crowded tile stands beside it (a HangOut once it is on the
            // spot's registry, which keeps the spot's tile).
            if kind != ActionKind::HangOut {
                crate::systems::fixes::spread_spot(world, id, kind);
            }
            actions::on_start(world, id, kind, step.target);
            if kind == ActionKind::HangOut {
                crate::systems::fixes::spread_spot(world, id, kind);
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.exec = ExecState::Use { kind, until: tick + dur, started: tick };
                b.action_until = tick + dur;
            }
            StepResult::Running
        }
    }
}

/// L1: a social step whose partner is not in the room is re-bound to the
/// best co-located partner, the plan's target with it; unchanged when the
/// partner is here or nobody fits. The floors are the planner's own
/// (`court_target`): a Chat to anyone not disliked, a Flirt at 0.3
/// unmarried, a Propose only to someone `propose_allowed` (review fix: at
/// -1.0 a Propose went to a stranger, who then "rejected" it for a week).
fn rebind_partner(
    world: &mut World,
    id: EntityId,
    step: &crate::components::ActionInstance,
) -> crate::components::ActionInstance {
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let present =
        step.target.is_some_and(|t| here.is_some() && world.comp::<Position>(t).and_then(|p| p.building) == here);
    if present || here.is_none() {
        return step.clone();
    }
    use crate::systems::social;
    let (min, unmarried) = match step.action {
        ActionKind::Chat => (0.0, false),
        ActionKind::Flirt => (0.3, true),
        _ => (world.config.social.propose_affinity, true),
    };
    let Some(new) = social::best_colocated_partner(world, id, min, unmarried)
        .filter(|&t| step.action != ActionKind::Propose || social::propose_allowed(world, id, t))
    else {
        return step.clone();
    };
    let old = step.target;
    if let Some(plan) = world.comp_mut::<Brain>(id).and_then(|b| b.plan.as_mut()) {
        if plan.target == old {
            plan.target = Some(new);
        }
        for s in plan.steps.iter_mut().filter(|s| s.target == old) {
            s.target = Some(new);
        }
    }
    crate::components::ActionInstance { target: Some(new), ..step.clone() }
}

pub(super) fn plan_target_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).and_then(|p| p.target)
}

/// The tile a walk starts from: the street outside the current building, or
/// the agent's own tile.
pub fn walk_origin(world: &World, id: EntityId) -> Option<TilePos> {
    let pos = world.comp::<Position>(id)?;
    match pos.building.and_then(|b| world.comp::<Building>(b)) {
        Some(b) => Some(world.outside_door(b)),
        None => Some(pos.tile),
    }
}
