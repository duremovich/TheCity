//! Execution layer: three executor states as in F.E.A.R. (Goto, Use, Wait),
//! pathfinding, flow fields and reservations.
//!
//! `run` advances every Full and Coarse agent's current plan step by one
//! tick. It never decides *what* to do: plans come from the routine (M1),
//! the GOAP planner (M3) or, later, a player input system. Effects apply at
//! completion, money at start.

#![deny(clippy::unwrap_used)]

pub mod actions;
pub mod bigpath;
pub mod flowfield;
pub mod leglog;
pub mod legs;
pub mod pathfind;
pub mod reservations;
pub mod routine;

use serde::{Deserialize, Serialize};

use crate::components::{Brain, Building, Household, Lod, Position, TilePos};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::{ActionKind, LocationKey};
use crate::time::Tick;
use crate::world::World;

pub use flowfield::{FlowCache, FlowField};
pub use reservations::{Reservation, ReservationKind};

/// Where a Goto is heading. Shared by the Full and Coarse variants so an LOD
/// change can convert one into the other without re-resolving.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GotoTarget {
    pub dest: LocationKey,
    /// The door of `building`, or the street tile for non-building destinations.
    pub tile: TilePos,
    pub building: Option<EntityId>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum ExecState {
    #[default]
    Idle,
    Goto {
        target: GotoTarget,
        /// Remaining A* path, reversed (next tile is `last()`); empty when following a flow field.
        /// Scale S3a: on a big map (`World::big_paths`) a building's walk carries its path too, the
        /// door its first element.
        path: Vec<TilePos>,
        next_move_tick: Tick,
        /// When the agent first found the door or building full.
        blocked_since: Option<Tick>,
        /// M13 D19: quarter ticks carried past `next_move_tick` (a driver's
        /// step can cost less than a tick).
        #[serde(default)]
        carry_q: u8,
    },
    /// Coarse LOD.
    GotoTimed {
        target: GotoTarget,
        arrive_tick: Tick,
        /// When the agent first found the building full on arrival.
        #[serde(default)]
        blocked_since: Option<Tick>,
    },
    Use {
        kind: ActionKind,
        until: Tick,
        started: Tick,
    },
    Wait {
        until: Tick,
    },
    /// M13 D22: a flyer's hop, at every tier: off the ground at `from` (the
    /// street outside the building it left), at the door at `arrive_tick`.
    /// The renderer lerps `from` to the door and draws it over walls.
    Fly {
        target: GotoTarget,
        from: TilePos,
        depart: Tick,
        arrive_tick: Tick,
    },
    /// Transit (docs/TRANSIT.md T12): on the El's platform until `depart`,
    /// then aboard until `arrive_tick` (the body stays on the board stop's
    /// tile, as a flyer stays at its `from`), then the walk on to `target`
    /// (`World::rides` holds the itinerary).
    Ride {
        target: GotoTarget,
        depart: Tick,
        arrive_tick: Tick,
    },
    /// M14 V12: seated at a chair while a run (an event chain of dice
    /// contests on the Virt plane) is in `World::runs`; `Done` once it is
    /// gone. Needs decay as Wait's; the body is no witness.
    JackedIn {
        run: crate::virt::RunId,
        since: Tick,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailReason {
    NoSuchPlace,
    PreconditionLost,
    StockGone,
    PartnerLeft,
    Timeout,
    BuildingFull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepResult {
    Running,
    Done,
    Failed(FailReason),
}

/// Advance every executing agent by one tick.
pub fn run(world: &mut World) {
    world.door_queue.clear();
    world.sweep_reservations();
    for id in world.bodies() {
        let Some(brain) = world.comp::<Brain>(id) else { continue };
        if brain.lod == Lod::Statistical || world.has::<crate::components::Sentence>(id) || brain.cuffed_by.is_some() {
            continue;
        }
        if brain.emigrating {
            emigrate_step(world, id);
            continue;
        }
        if brain.plan.is_none() {
            continue; // the think system plans
        }
        // The pre-view pass phase 5 (the `fares` shape): a cab's rider waits
        // at his door or rides along; the driver's step moves him.
        if !world.jobs2.passengers.is_empty() && world.jobs2.passengers.contains_key(&id) {
            continue;
        }
        step_agent(world, id);
        if !world.jobs2.passengers.is_empty() {
            crate::systems::streetwork::carry(world, id);
        }
        // An escorting guard drags the suspect along; a carried corpse follows.
        if world.comp::<Brain>(id).is_some_and(|b| b.escorting.is_some()) {
            crate::systems::law::follow_guard(world, id);
        }
        if let Some(c) = world.comp::<Brain>(id).and_then(|b| b.carrying_corpse) {
            follow_bearer(world, id, c);
        }
    }
}

/// The carried corpse takes its bearer's tile and building.
fn follow_bearer(world: &mut World, bearer: EntityId, corpse: EntityId) {
    let Some((tile, building)) = world.comp::<Position>(bearer).map(|p| (p.tile, p.building)) else { return };
    if !world.has::<crate::components::Corpse>(corpse) {
        if let Some(b) = world.comp_mut::<Brain>(bearer) {
            b.carrying_corpse = None;
        }
        return;
    }
    if let Some(p) = world.comp_mut::<Position>(corpse) {
        p.tile = tile;
        p.building = building;
    }
}

/// An emigrant walks to the nearest map-edge Road and is gone on arrival.
fn emigrate_step(world: &mut World, id: EntityId) {
    // M14 V12: a runner leaving the city drops out of its run first.
    if world.runner_of.contains_key(&id) {
        crate::systems::virt::dump(world, id, "left the city");
    }
    let tick = world.tick;
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let lod = brain.lod;
    let state = brain.exec.clone();
    let result = match state {
        ExecState::Goto { target, path, next_move_tick, blocked_since, carry_q } => {
            advance_goto(world, id, target, path, next_move_tick, blocked_since, carry_q)
        }
        ExecState::GotoTimed { target, arrive_tick, blocked_since } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                arrive(world, id, target, blocked_since)
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
fn step_agent(world: &mut World, id: EntityId) {
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
        | ExecState::Ride { arrive_tick, .. } => tick < *arrive_tick,
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
        // Plan finished: a success resets the consecutive-failure count.
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.clear_plan();
            b.last_plan_failure = None;
        }
        world.release_all(id);
        return;
    };
    let lod = brain.lod;
    let state = brain.exec.clone();
    let started = brain.plan.as_ref().map_or(tick, |p| p.started_tick);

    // M14 V12: a runner dazed by a lost contest sits it out in the chair.
    let dazed = brain.dazed_until.is_some_and(|t| t > tick);
    let daze_end = brain.dazed_until;
    // Replan trigger (d): a plan that has run too long.
    if tick.saturating_sub(started) > world.config.brain.plan_timeout_ticks && !dazed {
        world.fail_plan(id);
        return;
    }

    let result = match state {
        ExecState::Idle => start_step(world, id, &step, lod),
        ExecState::Goto { target, path, next_move_tick, blocked_since, carry_q } => {
            advance_goto(world, id, target, path, next_move_tick, blocked_since, carry_q)
        }
        ExecState::GotoTimed { target, arrive_tick, blocked_since } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                arrive(world, id, target, blocked_since)
            }
        }
        // M13 D22: the flyer lands at the door (the door queue and a full
        // building as a timed arrival).
        ExecState::Fly { target, arrive_tick, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
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
            // The pre-view pass phase 5: a segment's end re-starts the step.
            if matches!(step.action, ActionKind::GoTo(_)) && !world.segments.is_empty() && legs::segment_done(world, id)
            {
                return;
            }
            // Transit T12: a leg of a ride ended (the board stop, the alight
            // stop), not the GoTo.
            if matches!(step.action, ActionKind::GoTo(_))
                && !world.rides.is_empty()
                && crate::systems::transit::leg_done(world, id, lod)
            {
                return;
            }
            if world.leg_log.is_some() && matches!(step.action, ActionKind::GoTo(_) | ActionKind::Escort) {
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
            // L2 (L30, instrumentation): a bed step's abort names the bed
            // kind and the failing check.
            let detail = abort_detail(world, id, &step);
            world.push_event(
                EventKind::PlanAborted,
                &[id],
                format!("{goal:?} failed at {:?}: {reason:?}{detail}", step.action),
            );
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
fn step_ctx(world: &World, id: EntityId, kind: ActionKind) -> crate::goap::PlanCtx {
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
    if !matches!(step.action, ActionKind::Sleep | ActionKind::CheckIn) {
        return String::new();
    }
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let hideout = world.gang_of(id).and_then(|g| world.hideout_of(g));
    let squat = world.comp::<crate::components::Squatter>(id).map(|s| s.building);
    let bed = match here {
        None => "street".to_string(),
        Some(b) if Some(b) == home => "Home".to_string(),
        Some(b) if Some(b) == hideout => "Hideout".to_string(),
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

impl World {
    /// A step failed: the goal is replanned once; a second consecutive
    /// failure cools it.
    pub fn fail_plan(&mut self, id: EntityId) {
        if let Some(goal) = self.comp::<Brain>(id).and_then(|b| b.plan_goal()) {
            self.goal_failed(id, goal);
        }
        self.abort_plan(id);
    }

    /// A goal that cannot be planned at all: nothing will change in a tick, so
    /// it cools straight away (the replan-once rule is for steps that fail
    /// in execution, where the world may have moved).
    pub fn cool_goal(&mut self, id: EntityId, goal: crate::components::GoalKind) {
        let until = self.tick + self.config.brain.goal_cooldown_ticks;
        if let Some(b) = self.comp_mut::<Brain>(id) {
            b.cooldowns.insert(goal, until);
            b.last_plan_failure = None;
        }
        self.drop_cooled_visit(id, goal);
    }

    /// Shadow V5 bug 3: a Visit that cools (it could not plan, or its plan
    /// failed twice) is dropped for the day, not re-chosen when the cooldown
    /// lapses (each win had cut a shift). Nothing for any other goal.
    fn drop_cooled_visit(&mut self, id: EntityId, goal: crate::components::GoalKind) {
        if goal == crate::components::GoalKind::Visit {
            self.visits.remove(&id);
        }
    }

    /// A goal could not be planned or its plan failed: replanned once; a
    /// second consecutive failure of the same goal (no plan completed in
    /// between) cools it for `goal_cooldown_ticks`. The spec's "within 60
    /// ticks" window is shorter than one door wait plus think latency, so
    /// consecutive-ness is the usable reading.
    pub fn goal_failed(&mut self, id: EntityId, goal: crate::components::GoalKind) {
        let tick = self.tick;
        let cooldown = self.config.brain.goal_cooldown_ticks;
        if let Some(b) = self.comp_mut::<Brain>(id) {
            match b.last_plan_failure {
                Some((g, _)) if g == goal => {
                    b.cooldowns.insert(goal, tick + cooldown);
                    b.last_plan_failure = None;
                }
                _ => {
                    b.last_plan_failure = Some((goal, tick));
                    return;
                }
            }
        }
        self.drop_cooled_visit(id, goal);
    }

    /// Drop the current plan: abandon the running step (partial effects per
    /// `actions::on_abort`), release reservations, reset execution.
    pub fn abort_plan(&mut self, id: EntityId) {
        // A guard abandoning an escort lets the suspect go (to be re-arrested);
        // M13 D36: an abductor, its abductee.
        if let Some(suspect) = self.comp::<Brain>(id).and_then(|b| b.escorting) {
            if let Some(b) = self.comp_mut::<Brain>(suspect) {
                b.cuffed_by = None;
                b.abducted_by = None;
            }
            if let Some(b) = self.comp_mut::<Brain>(id) {
                b.escorting = None;
            }
        }
        let running = match self.comp::<Brain>(id).map(|b| &b.exec) {
            Some(ExecState::Use { kind, started, .. }) => Some((*kind, *started)),
            _ => None,
        };
        if let Some((kind, started)) = running {
            actions::on_abort(self, id, kind, started);
        }
        // M14 V12: a body pulled out of the chair dumps its run.
        if self.runner_of.contains_key(&id) {
            crate::systems::virt::dump(self, id, "pulled from the chair");
        }
        // M13 D20: a vehicle on the road parks where its driver stands.
        crate::systems::vehicles::end_trip(self, id, false);
        crate::systems::transit::end_ride(self, id);
        self.segments.remove(&id);
        if !self.jobs2.passengers.is_empty() {
            crate::systems::streetwork::passenger_off(self, id);
        }
        if self.leg_log.is_some() {
            leglog::end(self, id, "aborted");
        }
        self.commute_start.remove(&id);
        if let Some(b) = self.comp_mut::<Brain>(id) {
            b.clear_plan();
            b.shop_pick = None;
        }
        self.release_all(id);
    }
}

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
fn commute_arrived(world: &mut World, id: EntityId, driving: bool) {
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
fn start_step(world: &mut World, id: EntityId, step: &crate::components::ActionInstance, lod: Lod) -> StepResult {
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
                            br.exec = ExecState::GotoTimed { target, arrive_tick, blocked_since: None };
                        }
                        return StepResult::Running;
                    }
                }
            }
            // Escort's target is the suspect, not the destination.
            let walk_target = if matches!(step.action, ActionKind::GoTo(_)) { step.target } else { None };
            let building = world.resolve_building(id, key, walk_target);
            let Some(tile) = world.resolve_location(id, key, walk_target) else {
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
                    });
                if let Some(state) = street {
                    leglog::start(world, id, &target, mode);
                    commute_started(world, id, &target);
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
            // leg, the walk to the board stop, comes back).
            if driving.is_none() && matches!(step.action, ActionKind::GoTo(_)) {
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
            if driving.is_none() && matches!(step.action, ActionKind::GoTo(_)) && legs::on(world) {
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
                )
            });
            // The pre-view pass phase 4 (P8): so does a sweeper's round (its
            // close is inside the Recycler, which the planner reads as no Beat).
            if goal == Some(crate::components::GoalKind::Work)
                && (crate::systems::shift::scripted(world, id) || crate::systems::jobs::sweep_round_on(world, id))
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
                let corner = step.target.is_none() && matches!(kind, ActionKind::Deal | ActionKind::BuyStims);
                let symbols = corner || {
                    let ctx = step_ctx(world, id, kind);
                    let ws = crate::goap::WorldState::observe(world, id, ctx.target);
                    kind.preconditions(&ws, &ctx)
                };
                if !symbols || !actions::can_start(world, id, kind, step.target) {
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
            actions::on_start(world, id, kind, step.target);
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

fn plan_target_of(world: &World, id: EntityId) -> Option<EntityId> {
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

/// The pre-view pass phase 5 (P12): an escort to the cells walks, at
/// walking speed (a Full guard on foot with the suspect in tow, a Coarse
/// one timed at the walk), up to `[legs] escort_walk_tiles`; past it the
/// guard calls the van: `van_call_ticks` for it to come, then the drive
/// with both aboard at the Car's speed (`[vehicles] mult`). No escort
/// covers ground faster than its mode. `None` with `[legs]` off (the old
/// timed leg capped at `escort_van_ticks`). Statistical mirror: none (an
/// arrest is made by a body; a Statistical guard's arrests are the law's
/// off-screen roll, with no leg).
fn escort_leg(world: &mut World, id: EntityId, target: &GotoTarget, lod: Lod) -> Option<ExecState> {
    let cfg = world.config.legs.clone()?;
    let from = walk_origin(world, id)?;
    let tiles = from.manhattan(target.tile);
    let (state, ticks) = if tiles <= cfg.escort_walk_tiles {
        let state = match lod {
            Lod::Full => {
                walking_goto(world, id, target.clone()).unwrap_or_else(|| timed_goto(world, id, target.clone()))
            }
            _ => timed_goto(world, id, target.clone()),
        };
        crate::systems::ownership::jobs2_row(world).escort_walks += 1;
        (state, timed_walk(world, id, from, target.tile))
    } else {
        let m = crate::systems::vehicles::timed_mult_of(world, crate::components::AssetKind::Car);
        let drive = ((Tick::from(tiles) * world.config.exec.move_ticks_full) as f32 * m).round().max(1.0) as Tick;
        let ticks = Tick::from(cfg.van_call_ticks) + drive;
        crate::systems::ownership::jobs2_row(world).escort_vans += 1;
        (ExecState::GotoTimed { target: target.clone(), arrive_tick: world.tick + ticks, blocked_since: None }, ticks)
    };
    let tpm = u32::try_from(u64::from(tiles) * 100 / ticks.max(1)).unwrap_or(u32::MAX);
    let row = crate::systems::ownership::jobs2_row(world);
    row.escort_tpm_max = row.escort_tpm_max.max(tpm);
    Some(state)
}

/// The pre-view pass phase 5 (the `fares` shape): a cab driver's leg past
/// `[legs] leg_replan_tiles` drives in segments too, each timed at the
/// cab's speed (the car seen en route, the rider aboard with it).
fn fare_segment(world: &mut World, id: EntityId, target: &GotoTarget) -> Option<ExecState> {
    if !legs::on(world) || !crate::systems::streetwork::on_fare(world, id) {
        return None;
    }
    let from = walk_origin(world, id)?;
    let end = legs::segment_end(world, from, target.tile)?;
    let key = legs::plan_key(world, id)?;
    let arrive_tick = world.tick + crate::systems::streetwork::cab_ticks(world, from, end);
    world.segments.insert(id, key);
    Some(ExecState::GotoTimed {
        target: GotoTarget { dest: LocationKey::Street, tile: end, building: None },
        arrive_tick,
        blocked_since: None,
    })
}

/// The pre-view pass phase 5 (OPEN-139): a walk past `[legs]
/// leg_replan_tiles` toward `target` walks to the next segment's end first
/// (`legs::segment_end`; a Full body on foot, a Coarse one timed), filed in
/// `World::segments` against the plan step. `None`: one leg as before.
fn segment(world: &mut World, id: EntityId, target: &GotoTarget, lod: Lod) -> Option<ExecState> {
    let from = walk_origin(world, id)?;
    let key = legs::plan_key(world, id)?;
    let state = if lod == Lod::Full && !world.config.exec.straight_line_paths {
        full_segment(world, from, target)?
    } else {
        let end = legs::segment_end(world, from, target.tile)?;
        timed_goto(world, id, GotoTarget { dest: LocationKey::Street, tile: end, building: None })
    };
    world.segments.insert(id, key);
    Some(state)
}

/// A Full walk's segment follows the walk's own route: the tile `[legs]
/// leg_replan_tiles` steps along the path the one leg would take (the
/// door's flow field, the big map's A*, a street target's A*), so the body
/// walks the streets it walked before, re-checking its ride at each end.
/// The straight line's Road snap took the walker off its route: a guard's
/// patrol leg left the street its beat passed down, and the thief that
/// beat sighted was never seen again (`law::test_theft_witnessed…`).
/// `None`: the route is no longer than a segment, or there is none.
fn full_segment(world: &mut World, from: TilePos, target: &GotoTarget) -> Option<ExecState> {
    let len = world.config.legs.as_ref()?.leg_replan_tiles.max(1) as usize;
    let route: Vec<TilePos> = match target.building {
        Some(b) if !world.big_paths() => {
            let door = world.comp::<Building>(b)?.door;
            let mut route = Vec::with_capacity(len + 1);
            let mut here = from;
            while route.len() <= len && here != door {
                here = world.flow_step(b, here)?;
                route.push(here);
            }
            route
        }
        Some(b) => {
            let door = world.comp::<Building>(b)?.door;
            world.big_path(from, door)?
        }
        None if world.big_paths() => world.big_path(from, target.tile)?,
        None => pathfind::astar(&world.map, from, target.tile, world.config.exec.astar_max_expansions)?,
    };
    if route.len() <= len {
        return None;
    }
    let end = route[len - 1];
    let mut path = route[..len].to_vec();
    path.reverse();
    let leg = GotoTarget { dest: LocationKey::Street, tile: end, building: None };
    Some(ExecState::Goto { target: leg, path, next_move_tick: world.tick, blocked_since: None, carry_q: 0 })
}

/// A Coarse Goto: arrive after Manhattan distance × move ticks; M12 D18: ×
/// `1 + timed_mult × litter` of the target's district, rounded. M13 D21:
/// × the vehicle's `timed_mult` when driving (else `Kit.walk_mult`); at
/// exactly 1.0 the integer formula runs unchanged (a branch, not a multiply).
pub fn timed_goto(world: &World, id: EntityId, target: GotoTarget) -> ExecState {
    let from = walk_origin(world, id).unwrap_or(target.tile);
    let walk = timed_walk(world, id, from, target.tile);
    let arrive_tick = world.tick + walk;
    ExecState::GotoTimed { target, arrive_tick, blocked_since: None }
}

/// The ticks of a timed walk from `from` to `to` (`timed_goto`'s; the L2
/// shadow fixes' commute estimate, `fixes::walk_estimate`).
pub fn timed_walk(world: &World, id: EntityId, from: TilePos, to: TilePos) -> Tick {
    let base = Tick::from(from.manhattan(to)) * world.config.exec.move_ticks_full;
    let m = crate::systems::vehicles::timed_mult(world, id);
    let mut litter = 1.0;
    if crate::systems::litter::enabled(world) {
        let dirt = world.district(world.district_of(to)).litter;
        litter = 1.0 + world.config.litter.timed_mult * dirt;
    }
    if m == 1.0 {
        if litter > 1.0 {
            (base as f32 * litter).round() as Tick
        } else {
            base
        }
    } else {
        (base as f32 * m * litter.max(1.0)).round() as Tick
    }
}

/// M12 D18: the extra ticks before a Full mover's next step off `tile`,
/// by the tile's litter band (`[litter] step_ticks`). One byte read on a
/// step already taken; flow fields never change.
pub fn litter_delay(world: &World, tile: TilePos) -> Tick {
    if !crate::systems::litter::enabled(world) {
        return 0;
    }
    let band = crate::systems::litter::band(crate::systems::litter::at(world, tile));
    Tick::from(world.config.litter.step_ticks.get(band).copied().unwrap_or(0))
}

/// A Full Goto: flow field for buildings, A* otherwise. `None` if unreachable.
pub fn walking_goto(world: &World, id: EntityId, target: GotoTarget) -> Option<ExecState> {
    if world.config.exec.straight_line_paths {
        return Some(timed_goto(world, id, target));
    }
    // Scale S3a: on a big map a building's path is found on the first step
    // (`advance_goto`, from outside the building being left).
    let path = if target.building.is_some() {
        Vec::new()
    } else if world.big_paths() {
        let from = walk_origin(world, id)?;
        let mut p = world.big_path(from, target.tile)?;
        p.reverse();
        p
    } else {
        let from = walk_origin(world, id)?;
        let mut p = pathfind::astar(&world.map, from, target.tile, world.config.exec.astar_max_expansions)?;
        p.reverse();
        p
    };
    Some(ExecState::Goto { target, path, next_move_tick: world.tick, blocked_since: None, carry_q: 0 })
}

/// One tick of Full-LOD movement. M13 D19: steps are counted in quarter
/// ticks, `next_q = next_move_tick × 4 + carry_q` (never earlier than now);
/// the call keeps stepping while `next_q` is inside this tick, up to
/// `[vehicles] max_steps_per_tick` steps, stopping before the door (the
/// door is entered by `try_enter` on the next call, never in a burst) and
/// at the path's end. An unchromed walker's step is 8 quarters, so it takes
/// one step and moves on `tick + 2`: the M12 arithmetic exactly. Leaving a
/// building keeps `tick + move_ticks`.
fn advance_goto(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    mut path: Vec<TilePos>,
    next_move_tick: Tick,
    blocked_since: Option<Tick>,
    carry_q: u8,
) -> StepResult {
    let tick = world.tick;
    if tick < next_move_tick {
        return StepResult::Running;
    }
    let move_ticks = world.config.exec.move_ticks_full;
    let Some(pos) = world.comp::<Position>(id).cloned() else { return StepResult::Failed(FailReason::NoSuchPlace) };

    // Inside some other building: step out onto the street first.
    if let Some(here) = pos.building {
        if Some(here) == target.building {
            return StepResult::Done;
        }
        world.leave_building(id);
        return set_goto(world, id, target, path, tick + move_ticks, None, 0);
    }

    let driving = world.comp::<crate::components::Kit>(id).is_some_and(|k| k.driving.is_some());
    let max_steps = if driving { world.config.vehicles.max_steps_per_tick.max(1) } else { 1 };
    let big = target.building.is_some() && world.big_paths();
    let mut next_q = (next_move_tick * 4 + Tick::from(carry_q)).max(tick * 4);
    let end_q = (tick + 1) * 4;
    let mut here = pos.tile;
    let mut steps = 0u8;
    loop {
        let next = match target.building {
            Some(b) => {
                let door = world.comp::<Building>(b).map(|bd| bd.door);
                if door == Some(here) {
                    if steps == 0 {
                        return try_enter(world, id, target, path, blocked_since);
                    }
                    break;
                }
                let step = match (big, door) {
                    (false, _) => world.flow_step(b, here),
                    (true, None) => None,
                    (true, Some(d)) => big_step(world, &mut path, here, d),
                };
                match step {
                    Some(n) if door == Some(n) => {
                        if steps == 0 {
                            return try_enter(world, id, target, path, blocked_since);
                        }
                        break;
                    }
                    Some(n) => n,
                    // The field never reached this tile: there is no way there.
                    None if steps == 0 => return StepResult::Failed(FailReason::NoSuchPlace),
                    None => break,
                }
            }
            None => match path.pop() {
                Some(n) => n,
                None if steps == 0 && here == target.tile => return StepResult::Done,
                None if steps == 0 => return StepResult::Failed(FailReason::NoSuchPlace),
                None => break,
            },
        };
        move_to(world, id, next);
        here = next;
        steps += 1;
        if driving {
            crate::systems::vehicles::note_step(world, id, next);
        }
        next_q += u64::from(crate::systems::vehicles::step_q(world, id, next)) + 4 * litter_delay(world, next);
        if next_q >= end_q || steps >= max_steps || (target.building.is_none() && path.is_empty()) {
            break;
        }
    }
    let carry = (next_q % 4) as u8;
    set_goto(world, id, target, path, next_q / 4, None, carry)
}

/// Scale S3a: the next tile toward `door` on a big map, off the walk's own
/// path (reversed, the door its first element: the door stays on it until
/// `try_enter` lets the walker in). The path is found here when the walk
/// has none, or when its next tile is no longer a step away or no longer
/// walkable (a wall went up, rubble fell, the body was moved).
fn big_step(world: &World, path: &mut Vec<TilePos>, here: TilePos, door: TilePos) -> Option<TilePos> {
    let usable = path.last().is_some_and(|&n| n.manhattan(here) == 1 && world.map.walkable(n));
    if !usable {
        let mut p = world.big_path(here, door)?;
        p.reverse();
        *path = p;
    }
    let n = *path.last()?;
    if n != door {
        path.pop();
    }
    Some(n)
}

fn move_to(world: &mut World, id: EntityId, tile: TilePos) {
    if let Some(p) = world.comp_mut::<Position>(id) {
        p.tile = tile;
    }
}

/// Step onto the door and into the building, or queue outside it.
fn try_enter(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    path: Vec<TilePos>,
    blocked_since: Option<Tick>,
) -> StepResult {
    let tick = world.tick;
    let Some(b) = target.building else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else {
        return StepResult::Failed(FailReason::NoSuchPlace);
    };
    let cap = world.config.exec.door_capacity_per_tick;
    let slots = world.door_queue.get(&door).copied().unwrap_or(0);
    let full = world.comp::<Building>(b).is_some_and(|bd| bd.is_full()) && !world.capacity_exempt(id, b);
    if slots < cap && !full {
        *world.door_queue.entry(door).or_insert(0) += 1;
        world.enter_building(id, b);
        return StepResult::Done;
    }
    let since = blocked_since.unwrap_or(tick);
    if tick - since >= world.config.exec.door_queue_max_ticks {
        if full {
            note_refused(world, id, b);
            return StepResult::Failed(FailReason::BuildingFull);
        }
        return StepResult::Failed(FailReason::Timeout);
    }
    set_goto(world, id, target, path, tick + 1, Some(since), 0)
}

/// Workplace room: remember the full building that refused `id` (`[exec]
/// refused_ticks`; `World::local` passes it over until then).
fn note_refused(world: &mut World, id: EntityId, b: EntityId) {
    if world.config.exec.refused_ticks == 0 {
        return;
    }
    let tick = world.tick;
    if let Some(brain) = world.comp_mut::<Brain>(id) {
        brain.refused = Some((b, tick));
    }
}

fn set_goto(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    path: Vec<TilePos>,
    next_move_tick: Tick,
    blocked_since: Option<Tick>,
    carry_q: u8,
) -> StepResult {
    if let Some(brain) = world.comp_mut::<Brain>(id) {
        brain.exec = ExecState::Goto { target, path, next_move_tick, blocked_since, carry_q };
    }
    StepResult::Running
}

/// Coarse arrival: leave wherever we are and appear at the destination. A
/// full building is queued at for `door_queue_max_ticks`, as on foot.
fn arrive(world: &mut World, id: EntityId, target: GotoTarget, blocked_since: Option<Tick>) -> StepResult {
    let tick = world.tick;
    match target.building {
        Some(b) => {
            if world.comp::<Position>(id).is_some_and(|p| p.building == Some(b)) {
                // The pre-view pass phase 2: across the room to the spot.
                if target.dest == LocationKey::Station {
                    // The pre-view pass phase 3: a spot another occupant
                    // stands on (the night crew's one stock tile) gives the
                    // nearest free interior tile (one body a tile).
                    let mut tile = target.tile;
                    if crate::systems::shift::haul_on(world) {
                        if let Some(bd) = world.comp::<Building>(b) {
                            let taken: Vec<TilePos> = bd
                                .occupants
                                .iter()
                                .filter(|&&o| o != id)
                                .filter_map(|&o| world.comp::<Position>(o).map(|p| p.tile))
                                .collect();
                            if taken.contains(&tile) {
                                if let Some(t) = bd
                                    .interior()
                                    .filter(|t| !taken.contains(t) && world.map.walkable(*t))
                                    .min_by_key(|&t| (t.manhattan(target.tile), t))
                                {
                                    tile = t;
                                }
                            }
                        }
                    }
                    if let Some(p) = world.comp_mut::<Position>(id) {
                        p.tile = tile;
                    }
                }
                return StepResult::Done;
            }
            if world.comp::<Building>(b).is_some_and(|bd| bd.is_full()) && !world.capacity_exempt(id, b) {
                let since = blocked_since.unwrap_or(tick);
                if tick - since >= world.config.exec.door_queue_max_ticks {
                    note_refused(world, id, b);
                    return StepResult::Failed(FailReason::BuildingFull);
                }
                if let Some(brain) = world.comp_mut::<Brain>(id) {
                    brain.exec = ExecState::GotoTimed { target, arrive_tick: tick + 1, blocked_since: Some(since) };
                }
                return StepResult::Running;
            }
            world.leave_building(id);
            world.enter_building(id, b);
            StepResult::Done
        }
        None => {
            world.leave_building(id);
            move_to(world, id, target.tile);
            StepResult::Done
        }
    }
}

impl World {
    /// The building a symbolic location refers to for this agent, if any.
    pub fn resolve_building(&self, agent: EntityId, key: LocationKey, target: Option<EntityId>) -> Option<EntityId> {
        use crate::components::BuildingKind as K;
        match key {
            LocationKey::Home => self.comp::<Household>(agent).and_then(|h| h.home),
            LocationKey::TargetHome => target,
            LocationKey::Farm => target
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::Farm))
                .or_else(|| self.comp::<crate::components::Job>(agent).and_then(|j| j.employer))
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::Farm))
                .or_else(|| self.local(agent, K::Farm)),
            // M10: several of each; the agent's own (D21).
            LocationKey::Market => target.or_else(|| self.local(agent, K::Market)),
            LocationKey::Bar => target.or_else(|| self.local(agent, K::Bar)),
            // Lives at work phase 7 (§ 5.2): an escort's cells (the nearest
            // of the Precinct and the Stations with a free cell), a Station
            // guard's post; else the Precinct.
            LocationKey::Jail => target.or_else(|| crate::systems::law::jail_for(self, agent)),
            LocationKey::Cemetery => target.or_else(|| self.building_of_kind(K::Cemetery)),
            LocationKey::Hall => target.or_else(|| self.building_of_kind(K::Hall)),
            LocationKey::Hideout => target.or_else(|| crate::systems::gang::hideout_for(self, agent)),
            LocationKey::Warehouse => target.or_else(|| self.building_of_kind(K::Warehouse)),
            // A suspect or corpse inside a building is reached through its door.
            LocationKey::SuspectTile | LocationKey::CorpseTile => {
                target.and_then(|s| self.comp::<Position>(s)).and_then(|p| p.building)
            }
            LocationKey::PatrolWaypoint => self
                .comp::<Brain>(agent)
                .and_then(|b| b.patrol_route.get(usize::from(b.patrol_legs) % b.patrol_route.len().max(1)).copied()),
            // M11 D13: the wage desk (the Hall for a city job).
            LocationKey::Workplace => target
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::SecurityOffice))
                // L2: a venue's or Fab's staff work at their employer (a
                // city-owned one's wage desk is the Hall).
                .or_else(|| {
                    self.comp::<crate::components::Job>(agent)
                        .and_then(|j| j.employer)
                        // The Real economy E26: a Mission's Volunteers work
                        // at it (a city deed's wage desk would be the Hall).
                        .filter(|&e| {
                            crate::systems::jobs::is_l2_building(self, e)
                                || crate::systems::charity::is_mission(self, e)
                                // Jobs and room J13: a trade's staff work at
                                // their employer, whoever pays.
                                || crate::systems::trades::at_post(self, agent, e)
                        })
                })
                .or_else(|| self.wage_desk(agent)),
            // M12 D21: the booked Hotel, else the one the agent can reach and pay.
            // OPEN-140: a housed sleeper's Hotel is the planner's
            // (`life::away_hotel`: a Hotel nearer than Home), never the
            // homeless's nearest cot or a bed out of the planner's reach
            // ("Hotel" resolved to a Soup Kitchen, or to none: NoSuchPlace).
            LocationKey::Hotel
                if self.comp::<crate::components::Household>(agent).is_some_and(|h| h.home.is_some()) =>
            {
                target
                    .filter(|&t| {
                        crate::systems::street::is_hotel(self, t) && !crate::systems::charity::is_cot_house(self, t)
                    })
                    .or_else(|| crate::systems::life::away_hotel(self, agent))
            }
            // The island pass phase 2 (OPEN-171): with `[distance.free]` on, a
            // "Hotel" is a Hotel and a cot is `Cot`; a pick that turned from
            // one to the other since the plan is no such place (a replan).
            LocationKey::Hotel if crate::systems::distance::free(self).is_some() => target
                .filter(|&t| crate::systems::street::is_hotel(self, t) && !crate::systems::charity::is_mission(self, t))
                .or_else(|| {
                    crate::systems::street::hotel_for(self, agent)
                        .filter(|&h| !crate::systems::charity::is_mission(self, h))
                }),
            LocationKey::Hotel => target
                .filter(|&t| crate::systems::street::is_hotel(self, t))
                .or_else(|| crate::systems::street::hotel_for(self, agent)),
            LocationKey::Cot => target.filter(|&t| crate::systems::charity::is_cot_house(self, t)).or_else(|| {
                crate::systems::street::hotel_for(self, agent).filter(|&h| crate::systems::charity::is_mission(self, h))
            }),
            // M12 D27: the agent's squat, else the bound derelict.
            LocationKey::Squat => self
                .comp::<crate::components::Squatter>(agent)
                .map(|s| s.building)
                .or_else(|| target.filter(|&t| crate::systems::street::is_derelict(self, t))),
            // M12 D37: inside the Hideout; a door muster resolves to its tile.
            LocationKey::MusterPoint => match crate::systems::raid::muster_point(self, agent) {
                Some(crate::systems::raid::MusterAt::Inside(b)) => Some(b),
                _ => None,
            },
            // M13 D29: the bound seller; D47: a bound Garage, else the nearest.
            LocationKey::Seller => target.filter(|&t| self.has::<Building>(t)),
            LocationKey::Garage => {
                let garage = |t: &EntityId| self.comp::<Building>(*t).is_some_and(|b| b.kind == K::Garage);
                target
                    .filter(garage)
                    .or_else(|| self.comp::<crate::components::Job>(agent).and_then(|j| j.employer).filter(garage))
                    .or_else(|| self.local(agent, K::Garage))
            }
            // M13 D34: the bound Clinic, else a Ripperdoc's employer, else the nearest.
            LocationKey::Clinic => {
                let clinic = |t: &EntityId| self.comp::<Building>(*t).is_some_and(|b| b.kind == K::Clinic);
                target
                    .filter(clinic)
                    .or_else(|| self.comp::<crate::components::Job>(agent).and_then(|j| j.employer).filter(clinic))
                    .or_else(|| self.local(agent, K::Clinic))
            }
            // M13 D38/D40: the bound Stims source (a dealer's Bar, a legal Market).
            LocationKey::StimSource => target.filter(|&t| self.has::<Building>(t)),
            // M13 D36: a quarry inside a building is reached through its door.
            LocationKey::Victim => target.and_then(|s| self.comp::<Position>(s)).and_then(|p| p.building),
            // M14 V5: the chair of the agent's run order.
            LocationKey::Chair => self.run_orders.get(&agent).map(|o| o.chair),
            // M14 V29: the bound Lab of a Data buyer, else the nearest one.
            LocationKey::DataBuyer => target
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::Lab))
                .or_else(|| crate::systems::tech::data_buyer_lab(self, agent)),
            // M15 W19: the Hunt's venue, then its intel building.
            LocationKey::Intel => crate::systems::hunt::intel_building(self, agent),
            // M13 D26: a vehicle is reached on the street outside its door.
            LocationKey::Anywhere
            | LocationKey::Street
            | LocationKey::RaidTarget
            | LocationKey::Vehicle
            | LocationKey::Beat
            | LocationKey::Spot
            | LocationKey::Bin => None,
            // The pre-view pass phase 2: a stations spot's building.
            LocationKey::Station => target.filter(|&t| self.has::<Building>(t)),
        }
    }

    /// The tile a symbolic location resolves to: a building's door, or a
    /// street tile. `None` means the place does not exist for this agent.
    pub fn resolve_location(&self, agent: EntityId, key: LocationKey, target: Option<EntityId>) -> Option<TilePos> {
        if let Some(b) = self.resolve_building(agent, key, target) {
            return self.comp::<Building>(b).map(|bd| bd.door);
        }
        match key {
            LocationKey::Street | LocationKey::Anywhere => {
                let pos = self.comp::<Position>(agent)?;
                match pos.building.and_then(|b| self.comp::<Building>(b)) {
                    Some(b) => Some(self.outside_door(b)),
                    None => Some(pos.tile),
                }
            }
            LocationKey::SuspectTile => target.and_then(|t| self.last_seen().get(&t)).map(|&(tile, _)| tile),
            LocationKey::CorpseTile | LocationKey::Victim => {
                target.and_then(|t| self.comp::<Position>(t)).map(|p| p.tile)
            }
            LocationKey::RaidTarget => crate::systems::raid::target_tile(self, agent),
            LocationKey::MusterPoint => match crate::systems::raid::muster_point(self, agent) {
                Some(crate::systems::raid::MusterAt::Door(t)) => Some(t),
                _ => None,
            },
            LocationKey::Vehicle => target.and_then(|v| crate::systems::vehicles::vehicle_stand(self, v)),
            // M15 W19: a street intel (a database sighting's tile).
            LocationKey::Intel => crate::systems::hunt::intel_tile(self, agent),
            // L2 L8: the beat's dirtiest street tile.
            LocationKey::Beat => crate::systems::jobs::beat_tile(self, agent),
            // L2 L14: the leisure pick's street spot.
            LocationKey::Spot => crate::systems::leisure::spot_of(self, agent),
            // Hunger as sickness § 4: the bin outside the bound building's door.
            LocationKey::Bin => target.and_then(|b| crate::systems::bins::bin_tile(self, b)),
            _ => None,
        }
    }

    /// Stand outside a building's door (off its occupant list). Promotion,
    /// demotion and coming of age all place agents this way.
    pub fn stand_at_door(&mut self, agent: EntityId, b: EntityId) {
        let Some(door) = self.comp::<Building>(b).map(|bd| bd.door) else { return };
        self.remove_from_building(agent);
        let tick = self.tick;
        if let Some(p) = self.comp_mut::<Position>(agent) {
            p.tile = door;
            p.building = None;
            p.entered = tick;
        }
    }

    /// Guards enter the Jail whatever its occupancy: the prisoner cap is the
    /// law system's rule (fines and early releases), not the door's.
    pub fn capacity_exempt(&self, agent: EntityId, b: EntityId) -> bool {
        // Lives at work phase 7: and at a Station (its desk and its cells).
        let guard_at_jail = self.comp::<Building>(b).is_some_and(|bd| {
            matches!(bd.kind, crate::components::BuildingKind::Jail | crate::components::BuildingKind::Station)
        }) && crate::systems::law::is_guard(self, agent);
        // Residents always get into their own Home (births may exceed the cap).
        let own_home = self.comp::<Household>(agent).and_then(|h| h.home) == Some(b);
        // M12: a booked guest gets into their Hotel, a squatter into the
        // squat, a sweeper into the Recycler it reports to.
        let own_bed = crate::systems::street::booked_hotel(self, agent) == Some(b)
            || self.comp::<crate::components::Squatter>(agent).is_some_and(|s| s.building == b);
        let sweeper = self
            .comp::<crate::components::Job>(agent)
            .is_some_and(|j| j.role == crate::components::Role::Sanitation && j.employer == Some(b));
        // Workplace room: with `[exec] staff_door`, any staff into their
        // own workplace (the shift's room is the jobs round's to keep).
        let staff = self.config.exec.staff_door
            && self.comp::<crate::components::Job>(agent).is_some_and(|j| j.employer == Some(b));
        // Jobs and room J13: a trade's staff into their workplace, a Super
        // into the Blocks of his round.
        guard_at_jail
            || own_home
            || own_bed
            || sweeper
            || staff
            || crate::systems::trades::capacity_exempt(self, agent, b)
    }

    /// The street tile just outside a building's door (a Road if there is one).
    pub fn outside_door(&self, b: &Building) -> TilePos {
        let mut fallback = None;
        for n in self.map.neighbours4(b.door) {
            if b.rect.contains(n) || !self.map.walkable(n) {
                continue;
            }
            if self.map.tile_at(n) == crate::components::TileKind::Road {
                return n;
            }
            fallback.get_or_insert(n);
        }
        fallback.unwrap_or(b.door)
    }

    /// Put the agent inside `b` on the first free interior tile.
    pub fn enter_building(&mut self, agent: EntityId, b: EntityId) {
        let Some(bd) = self.comp::<Building>(b) else { return };
        let taken: Vec<TilePos> =
            bd.occupants.iter().filter_map(|&o| self.comp::<Position>(o).map(|p| p.tile)).collect();
        let slot = bd.interior().find(|t| !taken.contains(t)).unwrap_or(bd.door);
        if let Some(bd) = self.comp_mut::<Building>(b) {
            if let Err(i) = bd.occupants.binary_search(&agent) {
                bd.occupants.insert(i, agent);
            }
        }
        let tick = self.tick;
        if let Some(p) = self.comp_mut::<Position>(agent) {
            p.tile = slot;
            p.building = Some(b);
            p.entered = tick;
        }
        // OPEN-140: home at last.
        if self.moving_in.get(&agent) == Some(&b) {
            self.moving_in.remove(&agent);
        }
    }

    /// Step out onto the street outside the current building, if inside one.
    pub fn leave_building(&mut self, agent: EntityId) {
        let Some(here) = self.comp::<Position>(agent).and_then(|p| p.building) else { return };
        let outside = self.comp::<Building>(here).map(|bd| self.outside_door(bd));
        self.remove_from_building(agent);
        if let (Some(p), Some(out)) = (self.comp_mut::<Position>(agent), outside) {
            p.tile = out;
            p.building = None;
        }
    }

    /// Lazily build and cache the flow field for a building's door; past
    /// `[exec] flow_field_cache` fields (Scale S3a: or the
    /// `flow_field_budget_mb` bytes) the least recently used is evicted.
    pub fn flow_field_for(&mut self, b: EntityId) -> Option<&FlowField> {
        if !self.flow_fields.contains(b) {
            let door = self.comp::<Building>(b)?.door;
            let field = FlowField::build(&self.map, door);
            let e = &self.config.exec;
            let cap = FlowCache::cap_for(e.flow_field_cache, e.flow_field_budget_mb << 20, field.next.len());
            self.flow_fields.insert(b, field, cap);
        }
        self.flow_fields.get(b)
    }

    /// Scale S3a: does a Full walk on this map follow exact A* paths
    /// (`exec::bigpath`) rather than per-door flow fields? Past `[exec]
    /// flow_field_max_tiles` tiles.
    pub fn big_paths(&self) -> bool {
        self.map.w() * self.map.h() > self.config.exec.flow_field_max_tiles
    }

    /// Scale S3a: the shortest path from `from` (exclusive) to `to`
    /// (inclusive), forward, or `None` with no way there.
    pub fn big_path(&self, from: TilePos, to: TilePos) -> Option<Vec<TilePos>> {
        let labels = self.flow_fields.labels(&self.map);
        let marks = self.flow_fields.landmarks(&self.map);
        crate::prof::timed!("exec::big_path", bigpath::find(&self.map, &labels, &marks, from, to))
    }

    /// Next tile toward building `b` from `from`.
    pub fn flow_step(&mut self, b: EntityId, from: TilePos) -> Option<TilePos> {
        self.flow_field_for(b).and_then(|f| f.step(from))
    }

    /// Drop every cached field (after BuildHome / DemolishHome).
    pub fn invalidate_flow_fields(&mut self) {
        self.flow_fields.clear();
        self.flow_fields.keep_landmarks_if(&self.map);
        self.note_landmark_base();
    }

    /// Review finding 10: what a save carries of the landmarks (their tiles
    /// while a closure left them older than the map's).
    fn note_landmark_base(&mut self) {
        self.landmark_base = self.flow_fields.stale_base(&self.map).map(bigpath::BaseTiles);
    }

    /// A building went up on a Lot (`rect`, its door `door`): drop the
    /// cached fields whose paths from outside cross its new walls or its
    /// interior, and reseal the rest (OPEN-112: each kept field is then
    /// byte-identical to a rebuild).
    pub fn invalidate_flow_fields_for_lot(&mut self, rect: crate::components::Rect, door: TilePos) {
        let tiles: Vec<TilePos> = (rect.y..rect.y + rect.h)
            .flat_map(|y| (rect.x..rect.x + rect.w).map(move |x| TilePos { x, y }))
            .filter(|&t| self.map.tile_at(t) != crate::components::TileKind::Door)
            .collect();
        let sealed = |t: TilePos| rect.contains(t) && self.map.tile_at(t) != crate::components::TileKind::Door;
        self.flow_fields.invalidate_sealed(&self.map, rect, door, &tiles, &sealed);
        self.flow_fields.keep_landmarks_if(&self.map);
        self.note_landmark_base();
    }
}
