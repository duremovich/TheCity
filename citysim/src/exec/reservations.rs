//! Shared-resource reservations made at plan time (used from M3).

use serde::{Deserialize, Serialize};

use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationKind {
    FoodUnits {
        building: EntityId,
        units: u32,
    },
    Bed {
        home: EntityId,
    },
    Partner {
        other: EntityId,
    },
    Corpse {
        corpse: EntityId,
    },
    Suspect {
        suspect: EntityId,
    },
    /// L2 (L30): a place at a venue, held from plan to arrival.
    Seat {
        building: EntityId,
    },
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

    /// L2 (L30): live `Bed` and `Seat` reservations at `b` by holders other
    /// than `exclude` who are not inside it yet (an occupant already counts).
    pub fn reserved_at(&self, b: EntityId, exclude: EntityId) -> usize {
        let tick = self.tick;
        let inside = |h: EntityId| self.comp::<crate::components::Position>(h).is_some_and(|p| p.building == Some(b));
        self.reservations
            .iter()
            .filter(|&(&h, _)| h != exclude && !inside(h))
            .map(|(_, rs)| {
                rs.iter()
                    .filter(|r| {
                        r.expires > tick
                            && matches!(r.kind, ReservationKind::Bed { home } | ReservationKind::Seat { building: home } if home == b)
                    })
                    .count()
            })
            .sum()
    }

    /// L2 (L30): live `Bed` reservations at `b` by holders other than
    /// `exclude` who hold no Hotel booking there (a booking is a bed).
    pub fn bed_reservations_unbooked(&self, b: EntityId, exclude: EntityId) -> usize {
        let tick = self.tick;
        self.reservations
            .iter()
            .filter(|&(&h, _)| h != exclude && crate::systems::street::booked_hotel(self, h) != Some(b))
            .map(|(_, rs)| {
                rs.iter()
                    .filter(|r| r.expires > tick && matches!(r.kind, ReservationKind::Bed { home } if home == b))
                    .count()
            })
            .sum()
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
