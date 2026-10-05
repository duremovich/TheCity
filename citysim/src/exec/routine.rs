//! Shift helpers shared by the Work goal gate and the planner, plus the two
//! plans that bypass GOAP: Idle, and the commute to a shift that has not
//! started yet (the planner cannot express "go early and wait").

use crate::components::{ActionInstance, Building, BuildingKind, GoalKind, Household, Inventory, Job, Position, Role};
use crate::entity::EntityId;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::world::World;

/// Extra ticks of slack when leaving for work.
const DEPARTURE_MARGIN: u64 = 60;
/// An agent already at the workplace waits for a shift this far off.
const WAIT_AT_WORK_MAX: u64 = 600;

fn step(action: ActionKind, target: Option<EntityId>) -> ActionInstance {
    ActionInstance { action, target, tile: None }
}

fn plan(goal: GoalKind, target: Option<EntityId>, steps: Vec<ActionInstance>, tick: u64) -> Plan {
    Plan { goal, target, steps, started_tick: tick }
}

/// The location key of a role's workplace (the v1 table).
pub fn workplace_key(role: Role) -> LocationKey {
    match role {
        Role::Farmer => LocationKey::Farm,
        Role::Guard => LocationKey::Jail,
        Role::Clerk => LocationKey::Market,
        Role::Bartender => LocationKey::Bar,
        Role::Gravedigger => LocationKey::Cemetery,
    }
}

/// M11 D13: the key of this agent's workplace: a guard employed at a
/// Security Office works at `Workplace`; every other role at its own kind.
pub fn workplace_key_for(world: &World, _agent: EntityId, job: &Job) -> LocationKey {
    if crate::systems::law::job_is_private_guard(world, job) {
        LocationKey::Workplace
    } else {
        workplace_key(job.role)
    }
}

/// Ticks to walk between two tiles: Manhattan distance at Full speed, padded
/// by half for the detours a road grid forces.
fn travel_estimate(world: &World, from: crate::components::TilePos, to: crate::components::TilePos) -> u64 {
    u64::from(from.manhattan(to)) * world.config.exec.move_ticks_full * 3 / 2
}

/// Is a shift key a working day (6 days in 7)?
pub fn is_workday(shift_key: i64) -> bool {
    shift_key.rem_euclid(7) != 6
}

/// A worked shift whose wages have not been collected today: the tail of the
/// Work plan (haul, Hall visit) is still pending.
pub fn wage_pending(world: &World, job: &Job) -> bool {
    job.last_shift_day.is_some() && job.wage_collectable(world.day())
}

/// A shift to attend to right now: on shift, waiting at the workplace for one
/// that starts soon, or time to set off.
pub fn shift_pending(world: &World, id: EntityId, job: &Job) -> bool {
    let key = job.next_shift_key(world.tick);
    if !is_workday(key) || job.last_shift_day == Some(key) {
        return false;
    }
    // A guard's Patrol day is the Patrol goal's business.
    if job.role == Role::Guard && !crate::systems::law::jail_duty(world, id, key) {
        return false;
    }
    let tod = world.tick_of_day();
    if job.on_shift(tod) {
        return true;
    }
    let at_work = job.employer.is_some() && world.comp::<Position>(id).is_some_and(|p| p.building == job.employer);
    let until = u64::from(job.ticks_until_shift(tod));
    (at_work && until <= WAIT_AT_WORK_MAX) || must_leave_for_work(world, id)
}

/// The Work goal's gate: a shift to work, or wages to collect for one.
pub fn work_pending(world: &World, id: EntityId, job: &Job) -> bool {
    shift_pending(world, id, job) || wage_pending(world, job)
}

/// Should this agent drop what it is doing and head to work now?
pub fn must_leave_for_work(world: &World, id: EntityId) -> bool {
    let Some(job) = world.comp::<Job>(id) else { return false };
    let key = job.next_shift_key(world.tick);
    if !is_workday(key) || job.last_shift_day == Some(key) {
        return false;
    }
    if job.role == Role::Guard && !crate::systems::law::jail_duty(world, id, key) {
        return false;
    }
    let Some(employer) = job.employer else { return false };
    let Some(pos) = world.comp::<Position>(id) else { return false };
    if pos.building == Some(employer) {
        return false;
    }
    let tod = world.tick_of_day();
    let until = u64::from(job.ticks_until_shift(tod));
    let door = world.comp::<Building>(employer).map_or(pos.tile, |b| b.door);
    until <= travel_estimate(world, pos.tile, door) + DEPARTURE_MARGIN
}

/// Work before the shift starts: head to the workplace and rest inside until
/// it begins. `None` once the shift is on (the planner takes over) or when
/// nothing is pending.
pub fn commute_plan(world: &World, id: EntityId) -> Option<Plan> {
    let job = world.comp::<Job>(id)?;
    let employer = job.employer?;
    if job.on_shift(world.tick_of_day()) || !shift_pending(world, id, job) {
        return None;
    }
    let key = workplace_key_for(world, id, job);
    let steps = vec![step(ActionKind::GoTo(key), Some(employer)), step(ActionKind::Rest, None)];
    Some(plan(GoalKind::Work, Some(employer), steps, world.tick))
}

/// Idle bypasses the planner: Rest at Home or the Bar; otherwise go home
/// (loitering inside the Hall or Market blocks everyone else), or Wander
/// outside if homeless. Spare food goes into the pantry first.
pub fn idle_plan(world: &World, id: EntityId) -> Option<Plan> {
    let tick = world.tick;
    let pos = world.comp::<Position>(id)?;
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let inv = world.comp::<Inventory>(id).cloned().unwrap_or_default();
    let at_home = home.is_some() && pos.building == home;
    let goal = GoalKind::Idle;
    if at_home && inv.food > inv.stolen_food && inv.food >= 2 {
        return Some(plan(goal, home, vec![step(ActionKind::StoreFood, None)], tick));
    }
    // A member lying low, or without a Home, idles at the Hideout.
    if let Some(h) = crate::systems::gang::holes_up_at(world, id) {
        if pos.building == Some(h) {
            return Some(plan(goal, None, vec![step(ActionKind::Rest, None)], tick));
        }
        let steps = vec![step(ActionKind::GoTo(LocationKey::Hideout), Some(h)), step(ActionKind::Rest, None)];
        return Some(plan(goal, Some(h), steps, tick));
    }
    let here = pos.building.and_then(|b| world.comp::<Building>(b)).map(|b| b.kind);
    if at_home || here == Some(BuildingKind::Bar) {
        return Some(plan(goal, None, vec![step(ActionKind::Rest, None)], tick));
    }
    if home.is_some() {
        let steps = vec![step(ActionKind::GoTo(LocationKey::Home), None), step(ActionKind::Rest, None)];
        return Some(plan(goal, home, steps, tick));
    }
    Some(plan(goal, None, vec![step(ActionKind::Wander, None)], tick))
}
