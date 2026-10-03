//! M1's hard-coded daily routine. Produces ordinary `Plan`s so the execution
//! layer cannot tell it from the GOAP planner that replaces it in M3.
//!
//! Night: go home and sleep. Work: go to the workplace. Evening: Market if
//! hungry and able to pay, else Bar if sociable; eat at home. Wages and the
//! dole are collected at the Hall.

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, GoalKind, Household, Inventory, Job, MemoryKind, Needs, Personality,
    Position, Role, Wallet,
};
use crate::entity::EntityId;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::economy;
use crate::time::{self, DayPhase};
use crate::world::World;

/// Extra ticks of slack when leaving for work.
const DEPARTURE_MARGIN: u64 = 60;
/// An agent already at the workplace waits for a shift this far off.
const WAIT_AT_WORK_MAX: u64 = 600;
/// Below this an agent is "hungry" (matches `hunger_satisfied` in WorldState).
const HUNGRY_BELOW: f32 = 0.6;
/// Below this an agent goes to bed whatever the hour.
const EXHAUSTED_BELOW: f32 = 0.3;
const RESTED_AT: f32 = 0.9;

fn step(action: ActionKind, target: Option<EntityId>) -> ActionInstance {
    ActionInstance { action, target, tile: None }
}

fn plan(goal: GoalKind, target: Option<EntityId>, steps: Vec<ActionInstance>, tick: u64) -> Plan {
    Plan { goal, target, steps, started_tick: tick }
}

fn workplace_key(role: Role) -> LocationKey {
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

/// The next plan for an idle agent, or `None` to stand still this tick.
pub fn plan_for(world: &World, id: EntityId) -> Option<Plan> {
    let tick = world.tick;
    let day = world.day();
    let phase = world.phase();
    let dark = world.is_dark();
    let needs = world.comp::<Needs>(id)?;
    let pos = world.comp::<Position>(id)?;
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let inv = world.comp::<Inventory>(id).cloned().unwrap_or_default();
    let sociability = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
    let job = world.comp::<Job>(id);
    let at_home = home.is_some() && pos.building == home;
    let go_home = step(ActionKind::GoTo(LocationKey::Home), None);

    // 1. Work: on shift, or close enough to the shift to head over (or, once
    //    there, to wait at the workplace rather than wander off home).
    if let Some(job) = job {
        if let Some(employer) = job.employer {
            let tod = world.tick_of_day();
            let on_shift = job.on_shift(tod);
            let key = job.next_shift_key(tick);
            if is_workday(key) && job.last_shift_day != Some(key) {
                let go = step(ActionKind::GoTo(workplace_key(job.role)), Some(employer));
                if on_shift {
                    let steps = vec![go, step(ActionKind::work_for(job.role), Some(employer))];
                    return Some(plan(GoalKind::Work, Some(employer), steps, tick));
                }
                let at_work = pos.building == Some(employer);
                let door = world.comp::<Building>(employer).map_or(pos.tile, |b| b.door);
                let until = u64::from(job.ticks_until_shift(tod));
                // Already there (arrived early): stay put rather than wander off and bounce.
                let wait_here = at_work && until <= WAIT_AT_WORK_MAX;
                if wait_here || until <= travel_estimate(world, pos.tile, door) + DEPARTURE_MARGIN {
                    // Arrive early and idle at the door until the shift begins.
                    let steps = vec![go, step(ActionKind::Wander, None)];
                    return Some(plan(GoalKind::Work, Some(employer), steps, tick));
                }
            }
        }
    }

    // 2. Money: wages owed, or the daily dole, when the Hall is open (not at night).
    if phase != DayPhase::Night && !dark {
        if let Some(job) = job {
            // One visit per day: a short Treasury must not be hammered every few ticks.
            if job.days_unpaid >= 1 && !job.on_shift(world.tick_of_day()) && job.last_wage_attempt_day != Some(day) {
                let steps = vec![step(ActionKind::GoTo(LocationKey::Hall), None), step(ActionKind::CollectWage, None)];
                return Some(plan(GoalKind::Earn, None, steps, tick));
            }
        } else {
            let dole_due = world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day != Some(day));
            let treasury_ok = world.treasury().is_some_and(|t| t.coins >= 0) && world.levers.dole_per_day > 0;
            if dole_due && treasury_ok {
                let steps = vec![step(ActionKind::GoTo(LocationKey::Hall), None), step(ActionKind::CollectDole, None)];
                return Some(plan(GoalKind::Earn, None, steps, tick));
            }
        }
    }

    // 3. Hunger: inventory first, then the pantry, then the Market.
    if needs.hunger < HUNGRY_BELOW {
        if inv.food >= 1 {
            return Some(plan(GoalKind::Eat, None, vec![step(ActionKind::EatFromInventory, None)], tick));
        }
        let pantry = home.and_then(|h| world.comp::<Building>(h)).map_or(0, |b| b.stock_food);
        if pantry > 0 {
            let steps = vec![go_home.clone(), step(ActionKind::EatAtHome, None)];
            return Some(plan(GoalKind::Eat, home, steps, tick));
        }
        let market_stock = world
            .building_of_kind(BuildingKind::Market)
            .and_then(|m| world.comp::<Building>(m))
            .map_or(0, |b| b.stock_food);
        if economy::buy_quantity(world, id) > 0 && market_stock > 0 {
            let steps = vec![step(ActionKind::GoTo(LocationKey::Market), None), step(ActionKind::BuyFood, None)];
            return Some(plan(GoalKind::Eat, None, steps, tick));
        }
    }

    // 3b. Carrying spare food at home: put it in the pantry.
    if at_home && inv.food > inv.stolen_food && inv.food >= 2 {
        return Some(plan(GoalKind::Idle, home, vec![step(ActionKind::StoreFood, None)], tick));
    }

    // 4. Evening: a drink for the sociable, once per evening.
    if phase == DayPhase::Evening && !dark && sociability >= 0.5 && coins >= 2 {
        let drank_today = world
            .comp::<crate::components::Memory>(id)
            .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Socialised && time::day(e.tick) == day));
        if !drank_today {
            let steps = vec![step(ActionKind::GoTo(LocationKey::Bar), None), step(ActionKind::Drink, None)];
            return Some(plan(GoalKind::Socialise, None, steps, tick));
        }
    }

    // 5. Night, or exhausted: home to bed (the homeless sleep where they stand).
    let bed = |action: ActionKind| -> Vec<ActionInstance> {
        if home.is_some() {
            vec![go_home.clone(), step(action, None)]
        } else {
            vec![step(action, None)]
        }
    };
    if dark || needs.energy < EXHAUSTED_BELOW {
        let steps = if needs.energy < RESTED_AT { bed(ActionKind::Sleep) } else { bed(ActionKind::Rest) };
        return Some(plan(GoalKind::Sleep, home, steps, tick));
    }

    // 6. Nothing to do: rest at home.
    Some(plan(GoalKind::Idle, home, bed(ActionKind::Rest), tick))
}
