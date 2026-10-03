//! Shared-resource reservations made at plan time (used from M3).

use serde::{Deserialize, Serialize};

use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;

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

impl World {
    pub fn reserve(&mut self, holder: EntityId, kind: ReservationKind, expires: Tick) {
        self.reservations.entry(holder).or_default().push(Reservation { kind, expires });
    }

    pub fn release_all(&mut self, holder: EntityId) {
        self.reservations.remove(&holder);
    }

    /// Drop expired reservations. Called once per tick by the exec system.
    pub fn sweep_reservations(&mut self) {
        let tick = self.tick;
        self.reservations.retain(|_, v| {
            v.retain(|r| r.expires > tick);
            !v.is_empty()
        });
    }
}
