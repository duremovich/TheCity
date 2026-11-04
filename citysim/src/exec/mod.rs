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

mod board;
mod failure;
mod goto;
mod places;
mod start;
mod step;
mod walk;

pub use board::*;
use goto::*;
pub use start::*;
pub(crate) use step::*;
pub use walk::*;

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
        /// M13 D19: step units (round 9 Y4: `[exec] units_per_tick` a tick,
        /// quarter ticks before it) carried past `next_move_tick` (a step
        /// can cost less than a tick).
        #[serde(default)]
        carry_q: u32,
        /// The city milestone C7: each `path` step's level (reversed alike;
        /// above 0 a deck node of the walkways); empty: every step on the
        /// ground.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        levels: Vec<u8>,
        /// S3 (`OPEN-319`): a walkway portal's hop under way (stairs, a lift,
        /// an escalator, a deck's run), begun at this tick: the body stands at
        /// the hop's near end until `next_move_tick` (+ `carry_q`), its time
        /// paid, then steps onto the far end (`path`'s last, `levels`' last).
        /// `None`: no hop running (a hop inside one tick takes no state).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hop: Option<Tick>,
    },
    /// Coarse LOD.
    GotoTimed {
        target: GotoTarget,
        arrive_tick: Tick,
        /// When the agent first found the building full on arrival.
        #[serde(default)]
        blocked_since: Option<Tick>,
        /// The island pass phase 9 (OPEN-181): a timed walk on foot's start
        /// (`timed_goto`): the body is on its way from `from` to the
        /// target at `depart..=arrive_tick` (`walk_place`). `None`: no walk
        /// (a ride, the door queue's retry, `[motion.walks]` off).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        walk: Option<Walk>,
        /// The city milestone C7: the climbs the timed leg holds (down out of
        /// the building left, then up to the target's storey), placed by
        /// `walk_place`; `None`: none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        climb: Option<TimedClimb>,
    },
    /// The city milestone C7: a Full body's climb inside a structure, at
    /// its door: up from the door to the storey of the building just
    /// entered (`out` false; the GoTo done at `arrive_tick`), or down from
    /// its storey to the door of the building being left (`out` true; then
    /// the walk on, `path` and `levels` the walk's). By the stairs, or by
    /// the lift (`lift`): the lobby (the lower end) until `board`, aboard
    /// until `arrive_tick` (`systems::levels`). The body stays in the
    /// building, on `from` until the climb ends.
    Climb {
        target: GotoTarget,
        /// The structure whose core is climbed (`levels::core_of`).
        core: EntityId,
        from: u8,
        to: u8,
        depart: Tick,
        board: Tick,
        arrive_tick: Tick,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        lift: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        out: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<TilePos>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        levels: Vec<u8>,
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

/// The island pass phase 9 (OPEN-181): where and when a Coarse timed walk
/// set out (`ExecState::GotoTimed::walk`). The way is a pure function of
/// `from` and the target (`motion::way`), so it is never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Walk {
    pub from: TilePos,
    pub depart: Tick,
}

/// The city milestone C7: a Coarse timed leg's climbs (`GotoTimed.climb`):
/// down out of the building left first (its core, the storey left, the
/// ticks, by the lift?), then the walk, then up to the target's storey (the
/// last `up` ticks before the arrival). `booked`: the lift calls and the
/// day's counters taken (`levels::book`, on the leg's first step).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedClimb {
    pub down: Option<ClimbLeg>,
    pub up: Option<ClimbLeg>,
    #[serde(default)]
    pub booked: bool,
    /// The leg's start.
    pub depart: Tick,
    /// A move inside one structure: no walk, the one climb (`up`) between
    /// the storeys.
    #[serde(default)]
    pub inside: bool,
}

/// One climb of a [`TimedClimb`]: the structure, the storey off the ground
/// (the one left going down, the one reached going up), the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClimbLeg {
    pub core: EntityId,
    pub building: EntityId,
    pub from: u8,
    pub to: u8,
    pub plan: crate::systems::levels::ClimbPlan,
}

impl TimedClimb {
    /// The ticks before the walk (the climb down).
    pub fn down_ticks(&self) -> Tick {
        self.down.map_or(0, |c| c.plan.ticks)
    }

    /// The ticks after the walk (the climb up).
    pub fn up_ticks(&self) -> Tick {
        self.up.map_or(0, |c| c.plan.ticks)
    }
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
        // C3a: a body in a live engagement acts there (`combat::step`).
        if !world.combat.engaged.is_empty() && world.combat.engaged.contains_key(&id) {
            continue;
        }
        // C5: the knocked out lie still.
        if crate::systems::combat::knocked_out(world, id) {
            continue;
        }
        // The island pass phase 6: a rider whose plan ended between two
        // stops rides on to the next one (`motion::alight`); he acts there.
        if !world.transit.held.is_empty() && world.transit.held.contains(&id) {
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
    // The island pass phase 6: the El's riders board their trains.
    crate::systems::transit::trains_tick(world);
    // The island pass phase 5 (I3): the vehicles in motion advance, their
    // riders with them.
    crate::systems::motion::advance(world);
}
