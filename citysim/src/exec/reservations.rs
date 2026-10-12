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
    /// Review item 4: synth meals a buyer takes from the unit a Market has
    /// open (`diet::open_meals`), held beside its `FoodUnits` (the shelf
    /// units past them), so two buyers never both count the same open meals.
    OpenMeals {
        building: EntityId,
        meals: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    pub kind: ReservationKind,
    pub expires: Tick,
}

impl ReservationKind {
    /// The building a reservation is held at (`FoodUnits`, `Bed`, `Seat`,
    /// `OpenMeals`), for the per-building index.
    pub fn building(&self) -> Option<EntityId> {
        match *self {
            ReservationKind::FoodUnits { building, .. }
            | ReservationKind::Bed { home: building }
            | ReservationKind::Seat { building }
            | ReservationKind::OpenMeals { building, .. } => Some(building),
            _ => None,
        }
    }
}

impl World {
    pub fn reserve(&mut self, holder: EntityId, kind: ReservationKind, expires: Tick) {
        self.reservations.entry(holder).or_default().push(Reservation { kind, expires });
        if let Some(b) = kind.building() {
            self.res_by_building.entry(b).or_default().insert(holder);
        }
    }

    pub fn release_all(&mut self, holder: EntityId) {
        if let Some(rs) = self.reservations.remove(&holder) {
            for b in rs.iter().filter_map(|r| r.kind.building()) {
                self.unfile_reservation(holder, b);
            }
        }
    }

    /// Scale S4 perf: drop `holder` from `b`'s list unless it still holds a
    /// reservation there.
    fn unfile_reservation(&mut self, holder: EntityId, b: EntityId) {
        if self.reservations.get(&holder).is_some_and(|rs| rs.iter().any(|r| r.kind.building() == Some(b))) {
            return;
        }
        if let Some(set) = self.res_by_building.get_mut(&b) {
            set.remove(&holder);
            if set.is_empty() {
                self.res_by_building.remove(&b);
            }
        }
    }

    /// Scale S4 perf: the holders of a reservation at building `b`, from
    /// the per-building index (every holder with one there, ascending), and
    /// their reservations: what the per-building counts below read instead
    /// of every reservation in the city.
    fn reservations_at(&self, b: EntityId) -> impl Iterator<Item = (EntityId, &Vec<Reservation>)> + '_ {
        self.res_by_building
            .get(&b)
            .into_iter()
            .flatten()
            .filter_map(move |&h| self.reservations.get(&h).map(|rs| (h, rs)))
    }

    /// Scale S4 perf: the per-building index from `reservations` (a load).
    pub(crate) fn rebuild_reservation_index(&mut self) {
        let mut idx: std::collections::BTreeMap<EntityId, std::collections::BTreeSet<EntityId>> = Default::default();
        for (&h, rs) in &self.reservations {
            for b in rs.iter().filter_map(|r| r.kind.building()) {
                idx.entry(b).or_default().insert(h);
            }
        }
        self.res_by_building = idx;
    }

    /// L2 (L30): live `Bed` and `Seat` reservations at `b` by holders other
    /// than `exclude` who are not inside it yet (an occupant already counts).
    pub fn reserved_at(&self, b: EntityId, exclude: EntityId) -> usize {
        let tick = self.tick;
        let inside = |h: EntityId| self.comp::<crate::components::Position>(h).is_some_and(|p| p.building == Some(b));
        self.reservations_at(b)
            .filter(|&(h, _)| h != exclude && !inside(h))
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
        self.reservations_at(b)
            .filter(|&(h, _)| h != exclude && crate::systems::street::booked_hotel(self, h) != Some(b))
            .map(|(_, rs)| {
                rs.iter()
                    .filter(|r| r.expires > tick && matches!(r.kind, ReservationKind::Bed { home } if home == b))
                    .count()
            })
            .sum()
    }

    /// Review item 4: open-unit synth meals at `b` held by holders other
    /// than `exclude` (`OpenMeals`).
    pub fn open_meals_reserved(&self, b: EntityId, exclude: EntityId) -> u32 {
        self.reservations_at(b)
            .filter(|&(h, _)| h != exclude)
            .flat_map(|(_, rs)| rs.iter())
            .filter_map(|r| match r.kind {
                ReservationKind::OpenMeals { building, meals } if building == b => Some(meals),
                _ => None,
            })
            .sum()
    }

    /// Food units reserved on `b` by holders other than `except`
    /// (`FoodUnits`; `WorldState::reserved_by_others`).
    pub fn food_units_reserved(&self, b: EntityId, except: Option<EntityId>) -> u32 {
        self.reservations_at(b)
            .filter(|&(h, _)| Some(h) != except)
            .flat_map(|(_, rs)| rs.iter())
            .filter_map(|r| match r.kind {
                ReservationKind::FoodUnits { building, units } if building == b => Some(units),
                _ => None,
            })
            .sum()
    }

    /// Drop expired reservations. Called once per tick by the exec system.
    pub fn sweep_reservations(&mut self) {
        let tick = self.tick;
        let mut dropped: Vec<(EntityId, EntityId)> = Vec::new();
        self.reservations.retain(|&h, v| {
            v.retain(|r| {
                let live = r.expires > tick;
                if !live {
                    if let Some(b) = r.kind.building() {
                        dropped.push((h, b));
                    }
                }
                live
            });
            !v.is_empty()
        });
        for (h, b) in dropped {
            self.unfile_reservation(h, b);
        }
    }
}
