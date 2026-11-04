//! M18 P1 (the seam, docs/M18_PLAYER.md): a human drives one adult body.
//!
//! `PlayerCommand::Possess` puts `Brain.player` on the body; without
//! autopilot its own `think` and the plan queue skip it (`think::turn`,
//! `plan::run` / `plan::enqueue`), and the human's commands write its plan:
//! `Steps` (a directed plan of `ActionInstance`s, a click's walk), `Goal` (a
//! goal handed to `plan::plan_for`, the planner the NPCs use), `Stop`.
//! `SetAutopilot` hands the body back to its brain and takes it again.
//! Exec runs the plan like any other and re-checks each step at its start.
//!
//! No rule here is the player's own: a body that cannot choose
//! (`think::held`: off camera, sentenced, cuffed, leaving, down) cannot be
//! ordered either, and a step a goal change could not abort
//! (`think::uninterruptible`) cannot be stopped, but for the will terms the
//! human supplies (P2: a Sleep's first-hour lock, and each verb's
//! `Verb::waived`). Nobody under 18 can be possessed, and no step may target
//! a child.
//!
//! P2 (the verbs, below): menus built from precondition checks, the verbs
//! mapped to existing `ActionKind`s, the will-waivers per verb.
//!
//! P3 (the board and talk, below): the board is the records the body sees
//! (`contracts::visible`), taken through `contracts::player_take` (the
//! NPC's take, its gates of ability kept) and worked as the Contract goal's
//! own plans (`job_next`); talk is Chat, a courtship's Flirt and four moves
//! rolled through `moves::resolve` on a body beside you; the city's offers
//! (a cover shift, a gang's draw) wait on the board where an NPC is written.

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, CityOffer, GoalKind, Household, Identity, Inventory, Job,
    MemoryKind, OfferKind, PlayerControlled, Position, Squatter, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::{ActionKind, LocationKey, Plan, PlanCtx, StealSource, WorldState};
use crate::systems::{demography, plan, think};
use crate::world::World;
use serde::{Deserialize, Serialize};

mod menu;
mod nobody;
mod talk;
mod verbs;

pub use menu::*;
pub use nobody::*;
pub use talk::*;
pub use verbs::*;

/// Possess (`on`) an adult body, or hand it back to its brain for good.
pub fn possess(world: &mut World, id: EntityId, on: bool) {
    let name = world.name_of(id);
    if !on {
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.player = None;
        }
        world.push_event(EventKind::PlayerAction, &[id], format!("{name}: let go (the brain has the body)"));
        return;
    }
    let why = if world.comp::<Brain>(id).is_none() || !world.has::<Identity>(id) {
        Some("no living body")
    } else if !demography::is_adult(world, id) {
        Some("a child (under 18) is never possessed")
    } else {
        None
    };
    if let Some(why) = why {
        world.push_event(EventKind::PlayerActionFailed, &[id], format!("Possess {name}: {why}"));
        return;
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.player = Some(PlayerControlled::default());
    }
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: possessed (autopilot off)"));
}

/// Why the possessed body cannot take an order now (`None`: it can).
fn refusal(world: &World, id: EntityId) -> Option<String> {
    let Some(b) = world.comp::<Brain>(id) else { return Some("no living body".into()) };
    if b.player.is_none() {
        return Some("not possessed".into());
    }
    if let Some(why) = think::held(world, id) {
        return Some(why.into());
    }
    // M18 P2: a Sleep past its first hour is the NPC's anti-flap
    // (`think::uninterruptible`), a will term: the human may get up.
    let sleeping = matches!(b.exec, ExecState::Use { kind: ActionKind::Sleep, until, .. } if until > world.tick);
    if think::uninterruptible(b, world.tick) && !sleeping {
        let step = b.current_step().map_or("-".to_string(), |s| format!("{:?}", s.action));
        return Some(format!("busy: {step} cannot be cut short"));
    }
    None
}

/// The human's order refused: the line to the event ring and the HUD.
fn refuse(world: &mut World, id: EntityId, what: &str, why: &str) {
    let line = format!("{what} refused: {why}");
    let tick = world.tick;
    if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
        p.last_failure = Some((tick, line.clone()));
    }
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerActionFailed, &[id], format!("{name}: {line}"));
}

/// A direct order takes the wheel: autopilot off.
fn take_wheel(world: &mut World, id: EntityId) {
    if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
        p.autopilot = false;
        p.doing = None;
        p.shift = None;
        p.contract = None;
    }
}

/// The steps as one line for the log.
pub fn steps_line(steps: &[ActionInstance]) -> String {
    steps.iter().map(|s| format!("{:?}", s.action)).collect::<Vec<_>>().join(", ")
}

/// A directed plan: the body drops what it is doing and runs `steps`.
pub fn steps(world: &mut World, id: EntityId, steps: &[ActionInstance]) {
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, "Steps", &why);
    }
    if steps.is_empty() {
        return refuse(world, id, "Steps", "no steps");
    }
    let (w, h) = (world.map.w(), world.map.h());
    for s in steps {
        if s.tile.is_some_and(|t| usize::from(t.x) >= w || usize::from(t.y) >= h) {
            return refuse(world, id, "Steps", "a tile off the map");
        }
        if let Some(t) = s.target {
            if !world.is_alive(t) {
                return refuse(world, id, "Steps", "the target is gone");
            }
            // No child is ever a valid target (CLAUDE.md, 2026-10-27).
            if world.has::<Identity>(t) && !demography::is_adult(world, t) {
                return refuse(world, id, "Steps", "a child is never a target");
            }
        }
    }
    take_wheel(world, id);
    world.abort_plan(id);
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Idle);
        b.goal_since = tick;
    }
    let plan = Plan { goal: GoalKind::Idle, target: None, steps: steps.to_vec(), started_tick: tick };
    plan::install(world, id, plan);
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: directed [{}]", steps_line(steps)));
}

/// A goal: the planner the NPCs use makes the plan (`plan::plan_for`).
pub fn goal(world: &mut World, id: EntityId, goal: GoalKind) {
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, &format!("{goal:?}"), &why);
    }
    take_wheel(world, id);
    world.abort_plan(id);
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(goal);
        b.goal_since = tick;
    }
    plan::plan_for(world, id, goal);
    let planned = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).map(|p| steps_line(&p.steps));
    let name = world.name_of(id);
    match planned {
        Some(line) => {
            world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {goal:?} planned [{line}]"));
        }
        None => refuse(world, id, &format!("{goal:?}"), "no plan for it here and now"),
    }
}

/// M18 W (`exec`'s plan end, a body the human drives): a directed shift's
/// plan ended; the next part of the shift is planned (`plan::plan_for`, the
/// Work goal's bypass: the commute, the blocks, the break), until the shift
/// closes (`Job.last_shift_day`), ends or no plan is left for it.
pub fn shift_next(world: &mut World, id: EntityId) {
    let Some(b) = world.comp::<Brain>(id) else { return };
    if !b.possessed() || b.plan.is_some() {
        return;
    }
    let Some(key) = b.player.as_ref().and_then(|p| p.shift) else { return };
    let tick = world.tick;
    let going = world.comp::<Job>(id).is_some_and(|j| {
        j.next_shift_key(tick) == key && j.on_shift(world.tick_of_day()) && j.last_shift_day != Some(key)
    }) && think::held(world, id).is_none();
    let name = world.name_of(id);
    if going {
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.current_goal = Some(GoalKind::Work);
        }
        plan::plan_for(world, id, GoalKind::Work);
        if let Some(line) = world
            .comp::<Brain>(id)
            .and_then(|b| b.plan.as_ref())
            .filter(|p| p.goal == GoalKind::Work)
            .map(|p| steps_line(&p.steps))
        {
            world.push_event(EventKind::PlayerAction, &[id], format!("{name}: the shift goes on [{line}]"));
            return;
        }
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        if b.current_goal == Some(GoalKind::Work) {
            b.current_goal = None;
        }
        if let Some(p) = b.player.as_mut() {
            p.shift = None;
            p.doing = None;
        }
    }
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: the shift is over (standing)"));
}

/// Stop: the plan dropped, the body stands where it is.
pub fn stop(world: &mut World, id: EntityId) {
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, "Stop", &why);
    }
    take_wheel(world, id);
    world.abort_plan(id);
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = None;
    }
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: stopped"));
}

/// Autopilot on: the body's brain thinks and plans for it again (its next
/// think replaces a directed plan when another goal wins); off: the human
/// has the wheel (the brain's running plan goes on until it ends or a
/// command replaces it).
pub fn set_autopilot(world: &mut World, id: EntityId, on: bool) {
    let name = world.name_of(id);
    let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) else {
        world.push_event(EventKind::PlayerActionFailed, &[id], format!("Autopilot {name}: not possessed"));
        return;
    };
    p.autopilot = on;
    let text = if on { "autopilot on (the brain has the body)" } else { "autopilot off (the human has the body)" };
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {text}"));
}

/// The plan as the HUD reads it: the goal, each step (the current one
/// marked), and the step's exec state.
pub fn plan_line(world: &World, id: EntityId) -> String {
    let Some(b) = world.comp::<Brain>(id) else { return "-".into() };
    let Some(p) = b.plan.as_ref() else { return "no plan (standing)".into() };
    let steps: Vec<String> = p
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let a = match s.action {
                ActionKind::GoTo(k) => format!("GoTo {k:?}"),
                a => format!("{a:?}"),
            };
            if i == usize::from(b.plan_step) {
                format!("> {a}")
            } else {
                a
            }
        })
        .collect();
    format!("{:?}: {}", p.goal, steps.join(", "))
}

/// T2 (docs/TIME_MODEL.md § 3, auto-speed): the stretch the body is in, a
/// long thing it does not steer, as the HUD says it ("asleep", "at work",
/// "riding", "in a queue", "serving time", "indoors on autopilot"); `None`
/// on a walk or anything the human should watch. Read from what the sim
/// holds (the exec state, the sentence, the vehicle, the autopilot), no state
/// of its own. Whether someone is coming for the body is the caller's.
pub fn stretch(world: &World, id: EntityId) -> Option<&'static str> {
    if world.has::<crate::components::Sentence>(id) {
        return Some("serving time");
    }
    let b = world.comp::<Brain>(id)?;
    let pos = world.comp::<Position>(id)?;
    if world.aboard.contains_key(&id) {
        return Some("riding");
    }
    match &b.exec {
        ExecState::Ride { .. } | ExecState::Fly { .. } => Some("riding"),
        ExecState::Use { kind: ActionKind::Sleep, .. } => Some("asleep"),
        // the Sleep the human just picked, begun at the next tick
        ExecState::Idle if b.current_step().is_some_and(|s| s.action == ActionKind::Sleep) => Some("asleep"),
        ExecState::Use { kind, .. } if kind.is_work() && pos.building.is_some() => Some("at work"),
        ExecState::Use { kind: ActionKind::Apply | ActionKind::ServeTime, .. } => Some("in a queue"),
        ExecState::Goto { blocked_since: Some(_), path, .. } if path.is_empty() => Some("in a queue"),
        ExecState::Wait { .. } if pos.building.is_some() => Some("in a queue"),
        ExecState::Use { .. } | ExecState::JackedIn { .. }
            if pos.building.is_some() && b.player.as_ref().is_some_and(|p| p.autopilot) =>
        {
            Some("indoors on autopilot")
        }
        _ => None,
    }
}

/// T2 (docs/TIME_MODEL.md § 5): a walk the human just ordered begins now,
/// between two ticks: exec starts the body's GoTo step (its way planned, the
/// first steps taken at the next tick) instead of at the next tick's exec,
/// a tick behind the click. Only a GoTo from standing; anything else waits
/// for the tick as before.
pub fn start_now(world: &mut World, id: EntityId) {
    let Some(b) = world.comp::<Brain>(id) else { return };
    if b.player.is_none() || !matches!(b.exec, ExecState::Idle) {
        return;
    }
    if b.current_step().is_some_and(|s| matches!(s.action, ActionKind::GoTo(_))) {
        crate::exec::step_agent(world, id);
    }
}

/// T2 (docs/TIME_MODEL.md § 5, the body moves at the click): the street way
/// the body's current step walks, planned now, between two ticks, as exec
/// will plan it (`exec::walking_goto` to the step's place, a building's door
/// for a building), from the body's tile: the view walks it at once. `None`
/// off the street (indoors, aboard), with no GoTo under way, or no way.
pub fn way_now(world: &World, id: EntityId) -> Option<Vec<TilePos>> {
    let pos = world.comp::<Position>(id)?;
    if pos.building.is_some() || world.aboard.contains_key(&id) {
        return None;
    }
    let step = world.comp::<Brain>(id)?.current_step()?.clone();
    let ActionKind::GoTo(key) = step.action else { return None };
    let building = world.resolve_building(id, key, step.target);
    let picked = (key == LocationKey::Picked && building.is_none()).then_some(step.tile).flatten();
    let tile = picked.or_else(|| world.resolve_location(id, key, step.target))?;
    let target = crate::exec::GotoTarget { dest: key, tile, building: None };
    let ExecState::Goto { path, .. } = crate::exec::walking_goto(world, id, target)? else { return None };
    Some(std::iter::once(pos.tile).chain(path.into_iter().rev()).collect())
}

/// The standing building whose rect holds `tile` (the smallest, when a
/// shop-house's units overlap), for a click.
pub fn building_at(world: &World, tile: crate::components::TilePos) -> Option<EntityId> {
    world
        .buildings_by_kind()
        .values()
        .flatten()
        .filter_map(|&b| world.comp::<crate::components::Building>(b).map(|x| (b, x)))
        .filter(|(_, x)| !x.demolished && x.rect.contains(tile))
        .min_by_key(|(b, x)| (u32::from(x.rect.w) * u32::from(x.rect.h), b.index))
        .map(|(b, _)| b)
}

/// The standing building whose rect is nearest `tile` within `reach` tiles
/// (Chebyshev; 0 inside it), for a click on a roof or a wall: the view's lots
/// and the sim's rects disagree by a tile or two at their edges.
pub fn building_near(world: &World, tile: crate::components::TilePos, reach: u16) -> Option<EntityId> {
    let gap = |r: &crate::components::Rect| {
        let d = |p: u16, lo: u16, len: u16| {
            let hi = u32::from(lo) + u32::from(len.max(1)) - 1;
            let p = u32::from(p);
            if p < u32::from(lo) {
                u32::from(lo) - p
            } else {
                p.saturating_sub(hi)
            }
        };
        d(tile.x, r.x, r.w).max(d(tile.y, r.y, r.h))
    };
    world
        .buildings_by_kind()
        .values()
        .flatten()
        .filter_map(|&b| world.comp::<crate::components::Building>(b).map(|x| (b, x)))
        .filter(|(_, x)| !x.demolished)
        .map(|(b, x)| (gap(&x.rect), u32::from(x.rect.w) * u32::from(x.rect.h), b.index, b))
        .filter(|&(g, ..)| g <= u32::from(reach))
        .min()
        .map(|(.., b)| b)
}

/// A click on `tile`: the walk there, into the building that holds it (on a
/// roof or a wall, `on_building`: the nearest within `snap` tiles), else to
/// the street tile (`LocationKey::Picked`).
pub fn walk_to(world: &World, tile: crate::components::TilePos, on_building: bool, snap: u16) -> Vec<ActionInstance> {
    let target = if on_building { building_near(world, tile, snap) } else { building_at(world, tile) };
    vec![ActionInstance { action: ActionKind::GoTo(crate::goap::LocationKey::Picked), target, tile: Some(tile) }]
}
