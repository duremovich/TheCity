//! Execution layer: three executor states as in F.E.A.R. (Goto, Use, Wait),
//! pathfinding, flow fields and reservations.
//!
//! `run` advances every Full and Coarse agent's current plan step by one
//! tick. It never decides *what* to do: plans come from the routine (M1),
//! the GOAP planner (M3) or, later, a player input system. Effects apply at
//! completion, money at start.

#![deny(clippy::unwrap_used)]

pub mod actions;
pub mod flowfield;
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

pub use flowfield::FlowField;
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
        path: Vec<TilePos>,
        next_move_tick: Tick,
        /// When the agent first found the door or building full.
        blocked_since: Option<Tick>,
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
    for id in world.citizens() {
        let Some(brain) = world.comp::<Brain>(id) else { continue };
        if brain.lod == Lod::Statistical || world.has::<crate::components::Sentence>(id) || brain.cuffed_by.is_some() {
            continue;
        }
        if brain.plan.is_none() {
            continue; // the think system plans
        }
        step_agent(world, id);
        // An escorting guard drags the suspect along.
        if world.comp::<Brain>(id).is_some_and(|b| b.escorting.is_some()) {
            crate::systems::law::follow_guard(world, id);
        }
    }
}

/// Run the current step's state machine for one tick.
fn step_agent(world: &mut World, id: EntityId) {
    let Some(brain) = world.comp::<Brain>(id) else { return };
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
    let tick = world.tick;
    let started = brain.plan.as_ref().map_or(tick, |p| p.started_tick);

    // Replan trigger (d): a plan that has run too long.
    if tick.saturating_sub(started) > world.config.brain.plan_timeout_ticks {
        world.fail_plan(id);
        return;
    }

    let result = match state {
        ExecState::Idle => start_step(world, id, &step, lod),
        ExecState::Goto { target, path, next_move_tick, blocked_since } => {
            advance_goto(world, id, target, path, next_move_tick, blocked_since)
        }
        ExecState::GotoTimed { target, arrive_tick, blocked_since } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                arrive(world, id, target, blocked_since)
            }
        }
        ExecState::Use { kind, until, started } => {
            if tick >= until || actions::finishes_early(world, id, kind) {
                actions::on_complete(world, id, kind, step.target, started, tick)
            } else {
                StepResult::Running
            }
        }
        ExecState::Wait { until } => {
            if tick >= until || routine::must_leave_for_work(world, id) {
                StepResult::Done
            } else {
                StepResult::Running
            }
        }
    };

    match result {
        StepResult::Running => {}
        StepResult::Done => {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.plan_step = b.plan_step.saturating_add(1);
                b.exec = ExecState::Idle;
                b.action_until = tick;
            }
            actions::on_arrive(world, id, &step);
        }
        StepResult::Failed(reason) => {
            let goal = world.comp::<Brain>(id).and_then(|b| b.plan_goal());
            world.push_event(
                EventKind::PlanAborted,
                &[id],
                format!("{goal:?} failed at {:?}: {reason:?}", step.action),
            );
            world.fail_plan(id);
        }
    }
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
                _ => b.last_plan_failure = Some((goal, tick)),
            }
        }
    }

    /// Drop the current plan: abandon the running step (partial effects per
    /// `actions::on_abort`), release reservations, reset execution.
    pub fn abort_plan(&mut self, id: EntityId) {
        // A guard abandoning an escort lets the suspect go (to be re-arrested).
        if let Some(suspect) = self.comp::<Brain>(id).and_then(|b| b.escorting) {
            if let Some(b) = self.comp_mut::<Brain>(suspect) {
                b.cuffed_by = None;
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
        if let Some(b) = self.comp_mut::<Brain>(id) {
            b.clear_plan();
        }
        self.release_all(id);
    }
}

/// Begin a step: resolve a GoTo, or check preconditions and apply start
/// effects for an action.
fn start_step(world: &mut World, id: EntityId, step: &crate::components::ActionInstance, lod: Lod) -> StepResult {
    let tick = world.tick;
    // Escort and FleeToHome are walks with a completion effect.
    let walk_key = match step.action {
        ActionKind::GoTo(key) => Some(key),
        ActionKind::Escort => Some(LocationKey::Jail),
        ActionKind::FleeToHome => Some(LocationKey::Home),
        _ => None,
    };
    match walk_key {
        Some(key) => {
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
            let state = match lod {
                Lod::Coarse | Lod::Statistical => timed_goto(world, id, target),
                Lod::Full => match walking_goto(world, id, target) {
                    Some(s) => s,
                    None => return StepResult::Failed(FailReason::NoSuchPlace),
                },
            };
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.exec = state;
            }
            StepResult::Running
        }
        None => {
            let kind = step.action;
            // Replan trigger (b): re-observe; a false precondition fails the step.
            let ctx = crate::goap::PlanCtx::build_light(world, id, plan_target_of(world, id));
            let ws = crate::goap::WorldState::observe(world, id, ctx.target);
            if !kind.preconditions(&ws, &ctx) || !actions::can_start(world, id, kind, step.target) {
                return StepResult::Failed(FailReason::PreconditionLost);
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

fn plan_target_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).and_then(|p| p.target)
}

/// The tile a walk starts from: the street outside the current building, or
/// the agent's own tile.
fn walk_origin(world: &World, id: EntityId) -> Option<TilePos> {
    let pos = world.comp::<Position>(id)?;
    match pos.building.and_then(|b| world.comp::<Building>(b)) {
        Some(b) => Some(world.outside_door(b)),
        None => Some(pos.tile),
    }
}

/// A Coarse Goto: arrive after Manhattan distance × move ticks.
pub fn timed_goto(world: &World, id: EntityId, target: GotoTarget) -> ExecState {
    let from = walk_origin(world, id).unwrap_or(target.tile);
    let arrive_tick = world.tick + Tick::from(from.manhattan(target.tile)) * world.config.exec.move_ticks_full;
    ExecState::GotoTimed { target, arrive_tick, blocked_since: None }
}

/// A Full Goto: flow field for buildings, A* otherwise. `None` if unreachable.
pub fn walking_goto(world: &World, id: EntityId, target: GotoTarget) -> Option<ExecState> {
    let path = if target.building.is_some() {
        Vec::new()
    } else {
        let from = walk_origin(world, id)?;
        let mut p = pathfind::astar(&world.map, from, target.tile, world.config.exec.astar_max_expansions)?;
        p.reverse();
        p
    };
    Some(ExecState::Goto { target, path, next_move_tick: world.tick, blocked_since: None })
}

/// One tick of Full-LOD movement.
fn advance_goto(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    mut path: Vec<TilePos>,
    next_move_tick: Tick,
    blocked_since: Option<Tick>,
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
        return set_goto(world, id, target, path, tick + move_ticks, None);
    }

    match target.building {
        Some(b) => {
            let door = world.comp::<Building>(b).map(|bd| bd.door);
            if door == Some(pos.tile) {
                return try_enter(world, id, target, path, blocked_since);
            }
            match world.flow_step(b, pos.tile) {
                Some(n) if door == Some(n) => try_enter(world, id, target, path, blocked_since),
                Some(n) => {
                    move_to(world, id, n);
                    set_goto(world, id, target, path, tick + move_ticks, None)
                }
                // The field never reached this tile: there is no way there.
                None => StepResult::Failed(FailReason::NoSuchPlace),
            }
        }
        None => match path.pop() {
            Some(n) => {
                move_to(world, id, n);
                set_goto(world, id, target, path, tick + move_ticks, None)
            }
            None if pos.tile == target.tile => StepResult::Done,
            None => StepResult::Failed(FailReason::NoSuchPlace),
        },
    }
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
        return StepResult::Failed(if full { FailReason::BuildingFull } else { FailReason::Timeout });
    }
    set_goto(world, id, target, path, tick + 1, Some(since))
}

fn set_goto(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    path: Vec<TilePos>,
    next_move_tick: Tick,
    blocked_since: Option<Tick>,
) -> StepResult {
    if let Some(brain) = world.comp_mut::<Brain>(id) {
        brain.exec = ExecState::Goto { target, path, next_move_tick, blocked_since };
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
                return StepResult::Done;
            }
            if world.comp::<Building>(b).is_some_and(|bd| bd.is_full()) && !world.capacity_exempt(id, b) {
                let since = blocked_since.unwrap_or(tick);
                if tick - since >= world.config.exec.door_queue_max_ticks {
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
                .or_else(|| self.building_of_kind(K::Farm)),
            LocationKey::Market => target.or_else(|| self.building_of_kind(K::Market)),
            LocationKey::Bar => target.or_else(|| self.building_of_kind(K::Bar)),
            LocationKey::Jail => target.or_else(|| self.building_of_kind(K::Jail)),
            LocationKey::Cemetery => target.or_else(|| self.building_of_kind(K::Cemetery)),
            LocationKey::Hall => target.or_else(|| self.building_of_kind(K::Hall)),
            LocationKey::Hideout => target.or_else(|| self.building_of_kind(K::Hideout)),
            LocationKey::Warehouse => target.or_else(|| self.building_of_kind(K::Warehouse)),
            // A suspect inside a building is reached through its door.
            LocationKey::SuspectTile => target.and_then(|s| self.comp::<Position>(s)).and_then(|p| p.building),
            LocationKey::PatrolWaypoint => self
                .comp::<Brain>(agent)
                .and_then(|b| b.patrol_route.get(usize::from(b.patrol_legs) % b.patrol_route.len().max(1)).copied()),
            LocationKey::Anywhere | LocationKey::Street | LocationKey::CorpseTile => None,
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
            LocationKey::SuspectTile => target.and_then(|t| self.last_seen.get(&t)).map(|&(tile, _)| tile),
            LocationKey::CorpseTile => target.and_then(|t| self.comp::<Position>(t)).map(|p| p.tile),
            _ => None,
        }
    }

    /// Guards enter the Jail whatever its occupancy: the prisoner cap is the
    /// law system's rule (fines and early releases), not the door's.
    pub fn capacity_exempt(&self, agent: EntityId, b: EntityId) -> bool {
        self.comp::<Building>(b).is_some_and(|bd| bd.kind == crate::components::BuildingKind::Jail)
            && crate::systems::law::is_guard(self, agent)
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

    /// Lazily build and cache the flow field for a building's door.
    pub fn flow_field_for(&mut self, b: EntityId) -> Option<&FlowField> {
        if !self.flow_fields.contains_key(&b) {
            let door = self.comp::<Building>(b)?.door;
            let field = FlowField::build(&self.map, door);
            self.flow_fields.insert(b, field);
        }
        self.flow_fields.get(&b)
    }

    /// Next tile toward building `b` from `from`.
    pub fn flow_step(&mut self, b: EntityId, from: TilePos) -> Option<TilePos> {
        self.flow_field_for(b).and_then(|f| f.step(from))
    }

    /// Drop every cached field (after BuildHome / DemolishHome).
    pub fn invalidate_flow_fields(&mut self) {
        self.flow_fields.clear();
    }
}
