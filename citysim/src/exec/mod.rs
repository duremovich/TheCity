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
use crate::goap::{ActionKind, LocationKey};
use crate::time::Tick;
use crate::world::World;

pub use flowfield::FlowField;
pub use reservations::{Reservation, ReservationKind};

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum ExecState {
    #[default]
    Idle,
    Goto {
        /// Remaining A* path, reversed (next tile is `last()`); empty = follow the flow field.
        path: Vec<TilePos>,
        next_move_tick: Tick,
        dest: LocationKey,
        /// Building being walked to, if any.
        building: Option<EntityId>,
        /// When the agent first found the door or building full.
        blocked_since: Option<Tick>,
    },
    /// Coarse LOD.
    GotoTimed {
        arrive_tick: Tick,
        dest: LocationKey,
        building: Option<EntityId>,
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
        if brain.lod == Lod::Statistical || world.has::<crate::components::Sentence>(id) {
            continue;
        }
        if brain.plan.is_none() {
            // M1: the hard-coded routine; from M2 the think/plan systems fill this.
            let Some(plan) = routine::plan_for(world, id) else { continue };
            let tick = world.tick;
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.current_goal = Some(plan.goal);
                b.goal_since = tick;
                b.plan = Some(plan);
                b.plan_step = 0;
                b.exec = ExecState::Idle;
            }
        }
        step_agent(world, id);
    }
}

/// Run the current step's state machine for one tick.
fn step_agent(world: &mut World, id: EntityId) {
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let Some(step) = brain.current_step().cloned() else {
        // Plan finished.
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.clear_plan();
        }
        world.release_all(id);
        return;
    };
    let lod = brain.lod;
    let state = brain.exec.clone();
    let tick = world.tick;

    let result = match state {
        ExecState::Idle => start_step(world, id, &step, lod),
        ExecState::Goto { path, next_move_tick, dest, building, blocked_since } => {
            advance_goto(world, id, path, next_move_tick, dest, building, blocked_since)
        }
        ExecState::GotoTimed { arrive_tick, building, .. } => {
            if tick < arrive_tick {
                StepResult::Running
            } else {
                arrive(world, id, building)
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
        }
        StepResult::Failed(_) => {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.clear_plan();
            }
            world.release_all(id);
        }
    }
}

/// Begin a step: resolve a GoTo, or check preconditions and apply start
/// effects for an action.
fn start_step(world: &mut World, id: EntityId, step: &crate::components::ActionInstance, lod: Lod) -> StepResult {
    let tick = world.tick;
    match step.action {
        ActionKind::GoTo(key) => {
            let building = world.resolve_building(id, key, step.target);
            let Some(dest_tile) = world.resolve_location(id, key, step.target) else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let Some(pos) = world.comp::<Position>(id) else { return StepResult::Failed(FailReason::NoSuchPlace) };
            let already_there = match building {
                Some(b) => pos.building == Some(b),
                None => pos.tile == dest_tile,
            };
            if already_there {
                return StepResult::Done;
            }
            let from = pos.tile;
            let state = match lod {
                Lod::Coarse | Lod::Statistical => ExecState::GotoTimed {
                    arrive_tick: tick + Tick::from(from.manhattan(dest_tile)) * world.config.exec.move_ticks_full,
                    dest: key,
                    building,
                },
                Lod::Full => {
                    let path = if building.is_some() {
                        Vec::new()
                    } else {
                        let Some(mut p) =
                            pathfind::astar(&world.map, from, dest_tile, world.config.exec.astar_max_expansions)
                        else {
                            return StepResult::Failed(FailReason::NoSuchPlace);
                        };
                        p.reverse();
                        p
                    };
                    ExecState::Goto { path, next_move_tick: tick, dest: key, building, blocked_since: None }
                }
            };
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.exec = state;
            }
            StepResult::Running
        }
        kind => {
            if !actions::can_start(world, id, kind, step.target) {
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

/// One tick of Full-LOD movement.
#[allow(clippy::too_many_arguments)]
fn advance_goto(
    world: &mut World,
    id: EntityId,
    mut path: Vec<TilePos>,
    next_move_tick: Tick,
    dest: LocationKey,
    building: Option<EntityId>,
    blocked_since: Option<Tick>,
) -> StepResult {
    let tick = world.tick;
    if tick < next_move_tick {
        return StepResult::Running;
    }
    let Some(pos) = world.comp::<Position>(id).cloned() else { return StepResult::Failed(FailReason::NoSuchPlace) };

    // Inside some other building: step out onto the street first.
    if let Some(here) = pos.building {
        if Some(here) == building {
            return StepResult::Done;
        }
        world.leave_building(id);
        return set_goto(world, id, path, tick + world.config.exec.move_ticks_full, dest, building, None);
    }

    // Next tile: flow field toward a building door, or the precomputed A* path.
    let next = match building {
        Some(b) => {
            let door = world.comp::<Building>(b).map(|bd| bd.door);
            if door == Some(pos.tile) {
                None
            } else {
                world.flow_step(b, pos.tile)
            }
        }
        None => path.pop(),
    };

    match (next, building) {
        (Some(n), Some(b)) if world.comp::<Building>(b).is_some_and(|bd| bd.door == n) => {
            try_enter(world, id, b, path, dest, blocked_since)
        }
        (None, Some(b)) => try_enter(world, id, b, path, dest, blocked_since),
        (Some(n), _) => {
            if let Some(p) = world.comp_mut::<Position>(id) {
                p.tile = n;
            }
            let move_ticks = world.config.exec.move_ticks_full;
            set_goto(world, id, path, tick + move_ticks, dest, building, None)
        }
        (None, None) => StepResult::Done,
    }
}

/// Step onto the door and into the building, or queue outside it.
fn try_enter(
    world: &mut World,
    id: EntityId,
    b: EntityId,
    path: Vec<TilePos>,
    dest: LocationKey,
    blocked_since: Option<Tick>,
) -> StepResult {
    let tick = world.tick;
    let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else {
        return StepResult::Failed(FailReason::NoSuchPlace);
    };
    let cap = world.config.exec.door_capacity_per_tick;
    let slots = world.door_queue.get(&door).copied().unwrap_or(0);
    let full = world.comp::<Building>(b).is_some_and(|bd| bd.is_full());
    if slots < cap && !full {
        *world.door_queue.entry(door).or_insert(0) += 1;
        world.enter_building(id, b);
        return StepResult::Done;
    }
    let since = blocked_since.unwrap_or(tick);
    if tick - since >= world.config.exec.door_queue_max_ticks {
        return StepResult::Failed(if full { FailReason::BuildingFull } else { FailReason::Timeout });
    }
    set_goto(world, id, path, tick + 1, dest, Some(b), Some(since))
}

fn set_goto(
    world: &mut World,
    id: EntityId,
    path: Vec<TilePos>,
    next_move_tick: Tick,
    dest: LocationKey,
    building: Option<EntityId>,
    blocked_since: Option<Tick>,
) -> StepResult {
    if let Some(brain) = world.comp_mut::<Brain>(id) {
        brain.exec = ExecState::Goto { path, next_move_tick, dest, building, blocked_since };
    }
    StepResult::Running
}

/// Coarse arrival: leave wherever we are and appear inside the destination.
fn arrive(world: &mut World, id: EntityId, building: Option<EntityId>) -> StepResult {
    match building {
        Some(b) => {
            if world.comp::<Position>(id).is_some_and(|p| p.building == Some(b)) {
                return StepResult::Done;
            }
            if world.comp::<Building>(b).is_some_and(|bd| bd.is_full()) {
                return StepResult::Failed(FailReason::BuildingFull);
            }
            world.leave_building(id);
            world.enter_building(id, b);
            StepResult::Done
        }
        None => StepResult::Done,
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
            LocationKey::Anywhere
            | LocationKey::Street
            | LocationKey::SuspectTile
            | LocationKey::CorpseTile
            | LocationKey::PatrolWaypoint => None,
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
            LocationKey::SuspectTile | LocationKey::CorpseTile => {
                target.and_then(|t| self.comp::<Position>(t)).map(|p| p.tile)
            }
            _ => None,
        }
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
        if let Some(p) = self.comp_mut::<Position>(agent) {
            p.tile = slot;
            p.building = Some(b);
        }
    }

    /// Step out onto the street outside the current building, if inside one.
    pub fn leave_building(&mut self, agent: EntityId) {
        let Some(here) = self.comp::<Position>(agent).and_then(|p| p.building) else { return };
        let outside = self.comp::<Building>(here).map(|bd| self.outside_door(bd));
        if let Some(bd) = self.comp_mut::<Building>(here) {
            if let Ok(i) = bd.occupants.binary_search(&agent) {
                bd.occupants.remove(i);
            }
        }
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

    /// Drop a cached field (after BuildHome / DemolishHome).
    pub fn invalidate_flow_fields(&mut self) {
        self.flow_fields.clear();
    }
}
