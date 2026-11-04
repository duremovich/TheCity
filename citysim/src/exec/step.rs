use super::*;

/// The carried corpse takes its bearer's tile and building.
pub(super) fn follow_bearer(world: &mut World, bearer: EntityId, corpse: EntityId) {
    let Some((tile, building, floor)) = world.comp::<Position>(bearer).map(|p| (p.tile, p.building, p.floor)) else {
        return;
    };
    if !world.has::<crate::components::Corpse>(corpse) {
        if let Some(b) = world.comp_mut::<Brain>(bearer) {
            b.carrying_corpse = None;
        }
        return;
    }
    if let Some(p) = world.comp_mut::<Position>(corpse) {
        p.tile = tile;
        p.building = building;
        p.floor = floor;
    }
}

/// An emigrant walks to the nearest map-edge Road and is gone on arrival.
pub(super) fn emigrate_step(world: &mut World, id: EntityId) {
    // M14 V12: a runner leaving the city drops out of its run first.
    if world.runner_of.contains_key(&id) {
        crate::systems::virt::dump(world, id, "left the city");
    }
    let tick = world.tick;
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let lod = brain.lod;
    let state = brain.exec.clone();
    let result = match state {
        ExecState::Goto { target, path, next_move_tick, blocked_since, carry_q, levels, hop } => {
            advance_goto(world, id, target, path, levels, next_move_tick, blocked_since, carry_q, hop)
        }
        ExecState::GotoTimed { target, arrive_tick, blocked_since, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                arrive(world, id, target, blocked_since)
            }
        }
        ExecState::Climb { target, arrive_tick, path, levels, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                if let Some(p) = world.comp_mut::<Position>(id) {
                    p.floor = 0;
                }
                let state = ExecState::Goto {
                    target,
                    path,
                    next_move_tick: tick,
                    blocked_since: None,
                    carry_q: 0,
                    levels,
                    hop: None,
                };
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.exec = state;
                }
                StepResult::Running
            }
        }
        // M13 D22: an emigrant in the air lands as a timed walk does.
        ExecState::Fly { target, arrive_tick, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                arrive(world, id, target, None)
            }
        }
        _ => {
            let from = world.comp::<Position>(id).map_or(TilePos::default(), |p| p.tile);
            let Some(edge) = crate::systems::demography::nearest_edge_road(world, from) else {
                crate::systems::demography::emigrate(world, id);
                return;
            };
            let target = GotoTarget { dest: LocationKey::Street, tile: edge, building: None };
            let exec = match lod {
                Lod::Full => walking_goto(world, id, target.clone()).unwrap_or_else(|| timed_goto(world, id, target)),
                _ => timed_goto(world, id, target),
            };
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.exec = exec;
            }
            StepResult::Running
        }
    };
    match result {
        StepResult::Running => {}
        StepResult::Done | StepResult::Failed(_) => crate::systems::demography::emigrate(world, id),
    }
}

/// Run the current step's state machine for one tick.
pub(crate) fn step_agent(world: &mut World, id: EntityId) {
    step_agent_at(world, id, false);
}

/// S1 (`OPEN-311`): a segment's end re-starts the step at once and walks on
/// in the same tick, its time left carried (`resume`, step units): once a
/// tick (`step_agent_at`'s `chained`), never a loop.
fn chain_segment(world: &mut World, id: EntityId, resume: Option<u64>) {
    step_agent_at(world, id, true);
    let upt = world.config.exec.units_per_tick().max(1);
    let tick = world.tick;
    let Some(q) = resume else { return };
    let Some(b) = world.comp_mut::<Brain>(id) else { return };
    // Only a walk the re-start has just begun (a ride, a cab, a failure:
    // as they are).
    let ExecState::Goto { next_move_tick, carry_q, .. } = &mut b.exec else { return };
    if *next_move_tick != tick || *carry_q != 0 {
        return;
    }
    let q = q.max(tick * upt);
    *next_move_tick = q / upt;
    *carry_q = (q % upt) as u32;
    step_agent_at(world, id, true);
}

fn step_agent_at(world: &mut World, id: EntityId, chained: bool) {
    let Some(brain) = world.comp::<Brain>(id) else { return };
    // M18 P4: an offer to a body the human drives that ran out says so.
    if brain.player.as_ref().is_some_and(|p| p.offers.iter().any(|o| o.until <= world.tick)) {
        crate::systems::player::lapse(world, id);
    }
    let Some(brain) = world.comp::<Brain>(id) else { return };
    // The city milestone C7: a timed leg's climbs are booked on its first
    // step (the lift's lobby, the day's counters).
    if matches!(&brain.exec, ExecState::GotoTimed { climb: Some(c), .. } if !c.booked) {
        book_timed(world, id);
    }
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let tick = world.tick;
    // A walk between moves (or a timed walk not yet arrived) is `Running` and
    // touches nothing: skip the step and path clones (perf). The plan
    // timeout still fires first, as below.
    let between_moves = brain.plan.as_ref().is_some_and(|p| {
        usize::from(brain.plan_step) < p.steps.len()
            && tick.saturating_sub(p.started_tick) <= world.config.brain.plan_timeout_ticks
    }) && match &brain.exec {
        ExecState::Goto { next_move_tick, .. } => tick < *next_move_tick,
        ExecState::GotoTimed { arrive_tick, .. }
        | ExecState::Fly { arrive_tick, .. }
        | ExecState::Ride { arrive_tick, .. }
        | ExecState::Climb { arrive_tick, .. } => tick < *arrive_tick,
        _ => false,
    };
    if between_moves {
        return;
    }
    // M14 V12: a seated runner waits on its run (no plan timeout: the run's
    // own steps bound it); the run's end moves the plan on.
    if let ExecState::JackedIn { run, .. } = brain.exec {
        if world.runs.contains_key(&run) {
            return;
        }
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.plan_step = b.plan_step.saturating_add(1);
            b.exec = ExecState::Idle;
            b.action_until = tick;
        }
        return;
    }
    let Some(step) = brain.current_step().cloned() else {
        // Plan finished: a success resets the consecutive-failure count
        // (and the goal's back-off, round 3 S2).
        if let Some(b) = world.comp_mut::<Brain>(id) {
            if let Some(g) = b.plan_goal() {
                b.cool_streak.remove(&g);
            }
            b.clear_plan();
            b.last_plan_failure = None;
        }
        world.release_all(id);
        // M18 W: a directed shift goes on to its next part.
        if world.comp::<Brain>(id).is_some_and(|b| b.player.as_ref().is_some_and(|p| p.shift.is_some())) {
            crate::systems::player::shift_next(world, id);
        }
        // M18 P3: a directed contract goes on to its next plan.
        if world.comp::<Brain>(id).is_some_and(|b| b.player.as_ref().is_some_and(|p| p.contract.is_some())) {
            crate::systems::player::job_next(world, id);
        }
        // M18 P4: nothing followed it: the HUD's "doing" goes.
        crate::systems::player::plan_ended(world, id);
        return;
    };
    let lod = brain.lod;
    let state = brain.exec.clone();
    let started = brain.plan.as_ref().map_or(tick, |p| p.started_tick);

    // M14 V12: a runner dazed by a lost contest sits it out in the chair.
    let dazed = brain.dazed_until.is_some_and(|t| t > tick);
    let daze_end = brain.dazed_until;
    // The island pass phase 4 (OPEN-164): a leg in motion is spared the
    // timeout (`[legs] timeout_spares_legs`): a timed walk, a flight or a
    // ride before its arrival, a walk not blocked, and a segment's end
    // re-starting its step (`legs::segment_done` keeps the step's entry).
    // The base cut an island CEO's 931-tick ride at 900 (and a segmented
    // walk at the segment end it had reached): the body stayed at the
    // board stop, in another district, and the next plan rode again.
    let moving = world.config.legs.as_ref().is_some_and(|l| l.timeout_spares_legs)
        && match &state {
            ExecState::GotoTimed { arrive_tick, .. }
            | ExecState::Fly { arrive_tick, .. }
            | ExecState::Ride { arrive_tick, .. }
            | ExecState::Climb { arrive_tick, .. } => tick <= *arrive_tick,
            ExecState::Goto { blocked_since, .. } => blocked_since.is_none(),
            ExecState::Idle => world.segments.get(&id).is_some_and(|&k| legs::plan_key(world, id) == Some(k)),
            _ => false,
        };
    // Replan trigger (d): a plan that has run too long.
    if tick.saturating_sub(started) > world.config.brain.plan_timeout_ticks && !dazed && !moving {
        world.fail_plan(id);
        return;
    }

    let result = match state {
        ExecState::Idle => start_step(world, id, &step, lod),
        ExecState::Goto { target, path, next_move_tick, blocked_since, carry_q, levels, hop } => {
            advance_goto(world, id, target, path, levels, next_move_tick, blocked_since, carry_q, hop)
        }
        // The city milestone C7: the climb at the door, then the GoTo done
        // (up to the storey) or the walk on (down to the door).
        ExecState::Climb { target, arrive_tick, to, out, path, levels, walk, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else if out {
                // S3: the climb ends at the core (`Building::core_tile`), and
                // the walk from it to the door is walked over its own ticks.
                let core = world
                    .comp::<Position>(id)
                    .and_then(|p| p.building)
                    .and_then(|b| world.comp::<Building>(b))
                    .and_then(|bd| bd.core_tile());
                if let Some(p) = world.comp_mut::<Position>(id) {
                    p.floor = 0;
                    if let Some(c) = core {
                        p.tile = c;
                    }
                }
                advance_goto(world, id, target, path, levels, tick, None, 0, None)
            } else {
                // S4: a climb in that began with the walk to the core ends there.
                let core = walk
                    .and_then(|_| world.comp::<Position>(id)?.building)
                    .and_then(|b| world.comp::<Building>(b))
                    .and_then(|bd| bd.core_tile());
                if let Some(p) = world.comp_mut::<Position>(id) {
                    p.floor = to;
                    if let Some(c) = core {
                        p.tile = c;
                    }
                }
                // V8 (`OPEN-264`): a raider's own climb to the target's floor
                // (`vertical::climb_column`) ends in the Brawl it was begun
                // from, on the floor: the step goes on, not past it.
                if step.action == ActionKind::Brawl {
                    if let Some(b) = world.comp_mut::<Brain>(id) {
                        b.exec = ExecState::Use { kind: ActionKind::Brawl, until: tick, started: tick };
                    }
                    StepResult::Running
                } else {
                    StepResult::Done
                }
            }
        }
        ExecState::GotoTimed { target, arrive_tick, blocked_since, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                // The island pass phase 5 (I3): off the vehicle at the
                // way's end (the door), then in.
                crate::systems::motion::alight(world, id);
                arrive(world, id, target, blocked_since)
            }
        }
        // M13 D22: the flyer lands at the door (the door queue and a full
        // building as a timed arrival).
        ExecState::Fly { target, arrive_tick, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                crate::systems::motion::alight(world, id);
                arrive(world, id, target, None)
            }
        }
        // Transit T12: on the platform, then aboard; off at `arrive_tick`
        // (`transit::leg_done` alights). A ride gone from `World::rides`
        // walks on to its target.
        ExecState::Ride { target, arrive_tick, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else if world.rides.contains_key(&id) {
                StepResult::Done
            } else {
                let state = timed_goto(world, id, target);
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.exec = state;
                }
                StepResult::Running
            }
        }
        ExecState::Use { kind, until, started } => {
            if tick >= until || actions::finishes_early(world, id, kind, started) {
                actions::on_complete(world, id, kind, step.target, started, tick)
            } else {
                StepResult::Running
            }
        }
        // M14 V11: a `JackIn` begun before its order's `not_before` waits
        // in place, then retries the step (not the next one).
        ExecState::Wait { until }
            if step.action == ActionKind::JackIn && !dazed && world.run_orders.contains_key(&id) =>
        {
            if tick >= until {
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.exec = ExecState::Idle;
                }
            }
            StepResult::Running
        }
        // M14 review: a `JackIn` waiting on an order that is gone (expired,
        // dropped with its raid) fails; it used to finish the wait as done.
        // The post-run daze in the chair is the wait that ends at
        // `dazed_until`, and completes the step as before.
        ExecState::Wait { until } if step.action == ActionKind::JackIn && !dazed && daze_end != Some(until) => {
            StepResult::Failed(FailReason::PreconditionLost)
        }
        // Round 5 U3 (OPEN-235): the soup line waits out the hour's bowls,
        // then retries the step (never done uneaten).
        // S3 (`OPEN-318`): a patrol's muster (`start::patrol_muster`) waits
        // inside, then retries the walk.
        ExecState::Wait { until } if step.action == ActionKind::GoTo(LocationKey::PatrolWaypoint) => {
            if tick >= until {
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.exec = ExecState::Idle;
                }
            }
            StepResult::Running
        }
        ExecState::Wait { until } if step.action == ActionKind::EatAlms => {
            if tick >= until {
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.exec = ExecState::Idle;
                }
            }
            StepResult::Running
        }
        ExecState::Wait { until } => {
            if tick >= until || (!dazed && routine::must_leave_for_work(world, id)) {
                StepResult::Done
            } else {
                StepResult::Running
            }
        }
        // Handled above (the run's end moves the plan on).
        ExecState::JackedIn { .. } => StepResult::Running,
    };

    match result {
        StepResult::Running => {}
        StepResult::Done => {
            // The pre-view pass phase 5: a segment's end re-starts the step
            // (S1, `OPEN-311`: now, walking on with the tick's time left).
            if matches!(step.action, ActionKind::GoTo(_) | ActionKind::FleeToHome) && !world.segments.is_empty() {
                let upt = world.config.exec.units_per_tick();
                let resume = world.comp::<Brain>(id).and_then(|b| match &b.exec {
                    ExecState::Goto { next_move_tick, carry_q, .. } => Some(next_move_tick * upt + u64::from(*carry_q)),
                    _ => None,
                });
                if legs::segment_done(world, id) {
                    if !chained {
                        chain_segment(world, id, resume);
                    }
                    return;
                }
            }
            // Transit T12: a leg of a ride ended (the board stop, the alight
            // stop), not the GoTo.
            if matches!(step.action, ActionKind::GoTo(_) | ActionKind::FleeToHome)
                && !world.rides.is_empty()
                && crate::systems::transit::leg_done(world, id, lod)
            {
                return;
            }
            if world.leg_log.is_some()
                && matches!(step.action, ActionKind::GoTo(_) | ActionKind::Escort | ActionKind::FleeToHome)
            {
                leglog::end(world, id, "done");
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.plan_step = b.plan_step.saturating_add(1);
                b.exec = ExecState::Idle;
                b.action_until = tick;
            }
            if matches!(step.action, ActionKind::GoTo(_)) {
                // M13 D49: a commute sample; D20: the vehicle parks.
                let driving = world.trips.contains_key(&id);
                commute_arrived(world, id, driving);
                if driving {
                    crate::systems::vehicles::end_trip(world, id, true);
                }
            }
            actions::on_arrive(world, id, &step);
        }
        StepResult::Failed(reason) => {
            // L2 (L30): a bed lost at the step sleeps rough where it stands.
            if rough_fallback(world, id, &step, reason) {
                return;
            }
            if world.leg_log.is_some() {
                leglog::end(world, id, "failed");
            }
            count_abort(world, &step, reason);
            let goal = world.comp::<Brain>(id).and_then(|b| b.plan_goal());
            if world.leg_log.is_some() {
                leglog::step_failed(world, id, goal, &format!("{:?}", step.action), &format!("{reason:?}"));
            }
            // L2 (L30, instrumentation): a bed step's abort names the bed
            // kind and the failing check.
            let detail = abort_detail(world, id, &step);
            let line = format!("{goal:?} failed at {:?}: {reason:?}{detail}", step.action);
            // M18 P1: the human reads why (the HUD).
            if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
                p.last_failure = Some((tick, line.clone()));
            }
            world.push_event(EventKind::PlanAborted, &[id], line);
            // Round 4 T3 (OPEN-222): an arrest chase that failed says so.
            if goal == Some(crate::components::GoalKind::Arrest) {
                crate::systems::law::chase_dropped(world, id, None);
            }
            // Round 9 Y1 (`OPEN-249`): a starving body's failed food source
            // is passed over and the next think tries the next one; Eat is
            // not cooled (V17 `jobless_6204 d21 10:05`: two emptied bins
            // cooled Eat two hours at hunger 0.04).
            if goal == Some(crate::components::GoalKind::Eat) {
                let src = step.target.or_else(|| world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()?.target));
                if crate::systems::fixes::food_failed(world, id, src) {
                    world.abort_plan(id);
                    return;
                }
            }
            // OPEN-280: a patrol stop that cannot be reached or entered (a
            // full Block) is passed over for the next one, not the whole
            // Patrol cooled two hours mid-shift (the loop runs to the end).
            if goal == Some(crate::components::GoalKind::Patrol)
                && step.action == crate::goap::ActionKind::GoTo(crate::goap::LocationKey::PatrolWaypoint)
            {
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.patrol_legs = b.patrol_legs.wrapping_add(1);
                }
                world.abort_plan(id);
                return;
            }
            world.fail_plan(id);
        }
    }
}

/// The context a step's re-check reads (`PlanCtx::build_light`). L2 (L30,
/// `[lod] budget`): the light context leaves out the second beds the plan
/// was made for (`life::hideout_bed`, `life::away_hotel`), so every Hideout
/// Sleep and every housed CheckIn failed its re-check (phase 3's
/// instrumentation: 2,489 of 2,533 bed aborts in 30 days on seed 42); a
/// `Sleep` or `CheckIn` step re-reads them.
pub(super) fn step_ctx(world: &World, id: EntityId, kind: ActionKind) -> crate::goap::PlanCtx {
    let mut ctx = crate::goap::PlanCtx::build_light(world, id, plan_target_of(world, id));
    if matches!(kind, ActionKind::Sleep | ActionKind::CheckIn) {
        ctx.hideout_bed = crate::systems::life::hideout_bed(world, id).is_some();
        if !ctx.homeless {
            let away = crate::systems::life::away_hotel(world, id).is_some();
            ctx.away_hotel = away;
            ctx.hotel_available |= away;
        }
    }
    ctx
}

/// A bed's location key (Home, Hideout, Hotel, Squat).
fn bed_key(key: LocationKey) -> bool {
    matches!(key, LocationKey::Home | LocationKey::Hideout | LocationKey::Hotel | LocationKey::Squat | LocationKey::Cot)
}

/// L2 (L34): the day's plan failures by the failing step.
fn count_abort(world: &mut World, step: &crate::components::ActionInstance, reason: FailReason) {
    let c = &mut world.stats.current.budget;
    c.aborts += 1;
    match step.action {
        ActionKind::Scavenge => c.aborts_scavenge += 1,
        ActionKind::Sleep => c.aborts_sleep += 1,
        ActionKind::CheckIn => c.aborts_checkin += 1,
        ActionKind::GoTo(k) if reason == FailReason::BuildingFull && !bed_key(k) => c.aborts_seat += 1,
        _ => {}
    }
}

/// L2 (L30): with `[lod] budget`, a Sleep plan whose `Sleep` step fails its
/// re-check, or whose walk to a bed finds it full, lies down where the
/// agent stands (`ExecState::Use { Sleep }` started now, the plan moved to
/// its `Sleep` step) when `life::rough_ok` or the agent is exhausted.
fn rough_fallback(
    world: &mut World,
    id: EntityId,
    step: &crate::components::ActionInstance,
    reason: FailReason,
) -> bool {
    let lost = match step.action {
        ActionKind::Sleep => reason == FailReason::PreconditionLost,
        ActionKind::GoTo(k) => reason == FailReason::BuildingFull && bed_key(k),
        _ => false,
    };
    if !lost {
        return false;
    }
    let Some(sleep_at) = world.comp::<Brain>(id).and_then(|b| {
        let plan = b.plan.as_ref().filter(|p| p.goal == crate::components::GoalKind::Sleep)?;
        plan.steps.iter().position(|s| s.action == ActionKind::Sleep)
    }) else {
        return false;
    };
    let exhausted =
        world.comp::<crate::components::Needs>(id).is_some_and(|n| n.energy < world.config.life.exhausted_energy);
    if !(exhausted || crate::systems::life::rough_ok(world, id)) {
        return false;
    }
    let tick = world.tick;
    let target = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).and_then(|p| p.steps[sleep_at].target);
    let dur = actions::duration(world, id, ActionKind::Sleep);
    actions::on_start(world, id, ActionKind::Sleep, target);
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.plan_step = u8::try_from(sleep_at).unwrap_or(u8::MAX);
        b.exec = ExecState::Use { kind: ActionKind::Sleep, until: tick + dur, started: tick };
        b.action_until = tick + dur;
    }
    world.stats.current.budget.rough_sleeps += 1;
    true
}

/// L2 (L30, instrumentation): the bed a failed `Sleep` or `CheckIn` step
/// stood at (`Home`, `Hideout`, `Hotel`, `Squat`, `street`, else the
/// building's kind) and the check that failed, read as `start_step` reads
/// it (`PlanCtx::build_light`); the planning-time value of a second bed
/// (`life::hideout_bed`, `life::away_hotel`) beside it. Empty for any other
/// step. Pure reads, no draw.
fn abort_detail(world: &World, id: EntityId, step: &crate::components::ActionInstance) -> String {
    use crate::systems::{life, street};
    // Round 3 S2 (OPEN-222): a leader's step at the door names its check.
    if matches!(step.action, ActionKind::Collect | ActionKind::Court) {
        return crate::systems::leaders::door_why(world, id, step.action, step.target)
            .map_or_else(String::new, |w| format!(" [{w}]"));
    }
    // Round 9 Y1 (`OPEN-249`): a dive's failure names the bin's state.
    if step.action == ActionKind::DumpsterDive {
        return step.target.map_or_else(String::new, |b| format!(" [{}]", crate::systems::bins::why_not(world, id, b)));
    }
    if !matches!(step.action, ActionKind::Sleep | ActionKind::CheckIn) {
        return String::new();
    }
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let gang = world.gang_of(id);
    let squat = world.comp::<crate::components::Squatter>(id).map(|s| s.building);
    let bed = match here {
        None => "street".to_string(),
        Some(b) if Some(b) == home => "Home".to_string(),
        Some(b) if gang.is_some_and(|g| crate::systems::vertical::is_hideout_of(world, g, b)) => "Hideout".to_string(),
        Some(b) if street::is_hotel(world, b) => "Hotel".to_string(),
        Some(b) if Some(b) == squat => "Squat".to_string(),
        Some(b) => world.comp::<Building>(b).map_or("gone".to_string(), |bd| format!("{:?}", bd.kind)),
    };
    let ctx = step_ctx(world, id, step.action);
    let ws = crate::goap::WorldState::observe(world, id, ctx.target);
    let mut why: Vec<&str> = Vec::new();
    let mut flag = |cond: bool, name: &'static str| {
        if cond {
            why.push(name);
        }
    };
    match step.action {
        ActionKind::Sleep => match bed.as_str() {
            "Home" => flag(ctx.holes_up, "holes_up"),
            "Hideout" => {
                flag(!ctx.holes_up, "!holes_up");
                flag(!ctx.hideout_bed, "!hideout_bed(step)");
                flag(life::hideout_bed(world, id).is_some(), "hideout_bed(plan)");
            }
            "Hotel" => {
                flag(!ws.checked_in, "!checked_in");
                flag(street::booked_hotel(world, id).is_some_and(|h| Some(h) != here), "booked_elsewhere");
            }
            "Squat" => flag(!ctx.squatter, "!squatter"),
            "street" => {
                flag(!ctx.homeless, "!homeless");
                flag(ctx.homeless && ctx.hotel_available, "hotel_available");
                flag(ctx.homeless && ctx.squatter, "squatter");
                flag(!ctx.rough_ok, "!rough_ok");
            }
            _ => flag(true, "not_a_bed"),
        },
        _ => {
            flag(here.is_none_or(|b| !street::is_hotel(world, b)), "!in_hotel");
            flag(!ctx.adult, "!adult");
            flag(!ctx.hotel_available, "!hotel_available(step)");
            flag(ws.checked_in, "checked_in");
            flag(!ctx.homeless && life::away_hotel(world, id).is_some(), "away_hotel(plan)");
            flag(here.is_some_and(|b| street::is_hotel(world, b) && street::free_beds(world, b) == 0), "no_free_bed");
        }
    }
    format!(" [bed {bed}: {}]", why.join(","))
}
