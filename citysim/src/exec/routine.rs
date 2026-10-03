//! Goal → plan without a planner. Utility picks the goal (M2); this module
//! turns it into the fixed step list the M1 routine used. The GOAP planner
//! (M3) replaces `plan_for_goal` and nothing else changes.

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, GoalKind, Household, Inventory, Job, Needs, Position, Role, Wallet,
};
use crate::entity::EntityId;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::economy;
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

pub fn workplace_key(role: Role) -> LocationKey {
    match role {
        Role::Farmer => LocationKey::Farm,
        Role::Guard => LocationKey::Jail,
        Role::Clerk => LocationKey::Market,
        Role::Bartender => LocationKey::Bar,
        Role::Gravedigger => LocationKey::Cemetery,
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

/// The fixed plan for a goal, or `None` if it cannot be pursued right now.
pub fn plan_for_goal(world: &World, id: EntityId, goal: GoalKind) -> Option<Plan> {
    let tick = world.tick;
    let day = world.day();
    world.comp::<Needs>(id)?;
    let pos = world.comp::<Position>(id)?;
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let inv = world.comp::<Inventory>(id).cloned().unwrap_or_default();
    let at_home = home.is_some() && pos.building == home;
    let go_home = step(ActionKind::GoTo(LocationKey::Home), None);
    // Home if there is one, else where the agent stands.
    let at_bed = |action: ActionKind| -> Vec<ActionInstance> {
        if home.is_some() {
            vec![go_home.clone(), step(action, None)]
        } else {
            vec![step(action, None)]
        }
    };

    match goal {
        GoalKind::Eat => {
            if inv.food >= 1 {
                return Some(plan(goal, None, vec![step(ActionKind::EatFromInventory, None)], tick));
            }
            let pantry = home.and_then(|h| world.comp::<Building>(h)).map_or(0, |b| b.stock_food);
            if pantry > 0 {
                return Some(plan(goal, home, vec![go_home, step(ActionKind::EatAtHome, None)], tick));
            }
            if economy::buy_quantity(world, id) > 0 {
                let steps = vec![
                    step(ActionKind::GoTo(LocationKey::Market), None),
                    step(ActionKind::BuyFood, None),
                    step(ActionKind::EatFromInventory, None),
                ];
                return Some(plan(goal, None, steps, tick));
            }
            None
        }
        GoalKind::Sleep => Some(plan(goal, home, at_bed(ActionKind::Sleep), tick)),
        GoalKind::Work => {
            let job = world.comp::<Job>(id)?;
            let employer = job.employer?;
            let tod = world.tick_of_day();
            if shift_pending(world, id, job) {
                let go = step(ActionKind::GoTo(workplace_key(job.role)), Some(employer));
                if job.on_shift(tod) {
                    let mut steps = vec![go, step(ActionKind::work_for(job.role), Some(employer))];
                    // The wage trip, unless the Treasury already came up short today.
                    if job.last_wage_attempt_day != Some(day) {
                        steps.push(step(ActionKind::GoTo(LocationKey::Hall), None));
                        steps.push(step(ActionKind::CollectWage, None));
                    }
                    return Some(plan(goal, Some(employer), steps, tick));
                }
                // Not on shift yet: head over (or stay) and rest inside until it begins.
                return Some(plan(goal, Some(employer), vec![go, step(ActionKind::Rest, None)], tick));
            }
            if wage_pending(world, job) {
                let mut steps = Vec::new();
                // A farmer still at a stocked farm takes a load to the Market on the way.
                let farm_stock = world.comp::<Building>(employer).map_or(0, |b| b.stock_food);
                if job.role == Role::Farmer
                    && pos.building == Some(employer)
                    && farm_stock >= world.config.economy.haul_min_stock
                {
                    steps.push(step(ActionKind::HaulToMarket, Some(employer)));
                    steps.push(step(ActionKind::GoTo(LocationKey::Market), None));
                }
                steps.push(step(ActionKind::GoTo(LocationKey::Hall), None));
                steps.push(step(ActionKind::CollectWage, None));
                return Some(plan(goal, Some(employer), steps, tick));
            }
            None
        }
        GoalKind::Earn => {
            let hall = step(ActionKind::GoTo(LocationKey::Hall), None);
            if let Some(job) = world.comp::<Job>(id) {
                if job.wage_collectable(day) && !job.on_shift(world.tick_of_day()) {
                    return Some(plan(goal, None, vec![hall, step(ActionKind::CollectWage, None)], tick));
                }
                return None;
            }
            let dole_due = world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day != Some(day));
            let treasury_ok = world.treasury().is_some_and(|t| t.coins >= 0) && world.levers.dole_per_day > 0;
            // Collectable at any hour: visits then spread with wallet depletion
            // instead of bunching at dawn in front of a 20-capacity Hall.
            if dole_due && treasury_ok {
                return Some(plan(goal, None, vec![hall, step(ActionKind::CollectDole, None)], tick));
            }
            None
        }
        GoalKind::Socialise => {
            // M2: one drink at the Bar per day; Chat with a partner arrives with the social graph (M5).
            let drank_today = world.comp::<crate::components::Memory>(id).is_some_and(|m| {
                m.entries
                    .iter()
                    .any(|e| e.kind == crate::components::MemoryKind::Socialised && crate::time::day(e.tick) == day)
            });
            if coins >= 2 && !drank_today {
                let steps = vec![step(ActionKind::GoTo(LocationKey::Bar), None), step(ActionKind::Drink, None)];
                return Some(plan(goal, None, steps, tick));
            }
            None
        }
        GoalKind::Idle => {
            // Bypasses the planner: Rest at Home or the Bar; otherwise go home
            // (loitering inside the Hall or Market blocks everyone else), or
            // Wander outside if homeless. Spare food goes into the pantry first.
            if at_home && inv.food > inv.stolen_food && inv.food >= 2 {
                return Some(plan(goal, home, vec![step(ActionKind::StoreFood, None)], tick));
            }
            let here = pos.building.and_then(|b| world.comp::<Building>(b)).map(|b| b.kind);
            if at_home || here == Some(BuildingKind::Bar) {
                return Some(plan(goal, None, vec![step(ActionKind::Rest, None)], tick));
            }
            if home.is_some() {
                return Some(plan(goal, home, vec![go_home, step(ActionKind::Rest, None)], tick));
            }
            Some(plan(goal, None, vec![step(ActionKind::Wander, None)], tick))
        }
        // Planned by GOAP from M3 (Court, Flee, Fight, ReportCrime, Patrol,
        // Arrest, JoinGang, GangWork, Bury): unavailable until then.
        _ => None,
    }
}
