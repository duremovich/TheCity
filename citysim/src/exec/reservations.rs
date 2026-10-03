//! Shared-resource reservations made at plan time.

use serde::{Deserialize, Serialize};

use crate::entity::EntityId;
use crate::time::Tick;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationKind {
    FoodUnits { building: EntityId, units: u32 },
    Bed { home: EntityId },
    Partner { other: EntityId },
    Corpse { corpse: EntityId },
    Suspect { suspect: EntityId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    pub kind: ReservationKind,
    pub expires: Tick,
}
