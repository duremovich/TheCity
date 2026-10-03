//! Execution layer: Goto / Use / Wait, pathfinding, flow fields, reservations.
//! M0 ships the data types only; movement and actions arrive in M1.

#![deny(clippy::unwrap_used)]

pub mod flowfield;
pub mod reservations;

use serde::{Deserialize, Serialize};

use crate::components::TilePos;
use crate::goap::{ActionKind, LocationKey};
use crate::time::Tick;

pub use flowfield::FlowField;
pub use reservations::{Reservation, ReservationKind};

pub const MOVE_TICKS_FULL: Tick = 2;
pub const INTERRUPT_CHECK_TICKS: Tick = 30;
pub const DOOR_CAPACITY_PER_TICK: u8 = 2;
pub const DOOR_QUEUE_MAX_TICKS: Tick = 30;
pub const RESERVATION_TTL: Tick = 120;

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum ExecState {
    #[default]
    Idle,
    Goto {
        path: Vec<TilePos>,
        next_move_tick: Tick,
        dest: LocationKey,
    },
    /// Coarse LOD.
    GotoTimed {
        arrive_tick: Tick,
        dest: LocationKey,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepResult {
    Running,
    Done,
    Failed(FailReason),
}
