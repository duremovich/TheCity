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
        if let ReservationKind::Partner { other } = kind {
            *self.res_partners.entry(other).or_default().entry(holder).or_default() += 1;
        }
    }

    pub fn release_all(&mut self, holder: EntityId) {
        if let Some(rs) = self.reservations.remove(&holder) {
            for b in rs.iter().filter_map(|r| r.kind.building()) {
                self.unfile_reservation(holder, b);
            }
            for r in &rs {
                if let ReservationKind::Partner { other } = r.kind {
                    self.unfile_partner(holder, other);
                }
            }
        }
    }

    /// The perf pass phase 4: one `Partner` reservation of `holder` on
    /// `other` gone from the partner index.
    fn unfile_partner(&mut self, holder: EntityId, other: EntityId) {
        if let Some(hs) = self.res_partners.get_mut(&other) {
            if let Some(n) = hs.get_mut(&holder) {
                *n -= 1;
                if *n == 0 {
                    hs.remove(&holder);
                }
            }
            if hs.is_empty() {
                self.res_partners.remove(&other);
            }
        }
    }

    /// The perf pass phase 4: is `o` held by a `Partner` reservation of
    /// anyone but `holder` (`social::reserved_partners(holder)` contains
    /// `o`), off the partner index.
    pub fn partner_reserved(&self, o: EntityId, holder: EntityId) -> bool {
        self.res_partners.get(&o).is_some_and(|hs| hs.keys().any(|&h| h != holder))
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
        let mut partners: std::collections::BTreeMap<EntityId, std::collections::BTreeMap<EntityId, u32>> =
            Default::default();
        for (&h, rs) in &self.reservations {
            for r in rs {
                if let ReservationKind::Partner { other } = r.kind {
                    *partners.entry(other).or_default().entry(h).or_default() += 1;
                }
            }
        }
        self.res_partners = partners;
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
        let mut partners: Vec<(EntityId, EntityId)> = Vec::new();
        self.reservations.retain(|&h, v| {
            v.retain(|r| {
                let live = r.expires > tick;
                if !live {
                    if let Some(b) = r.kind.building() {
                        dropped.push((h, b));
                    }
                    if let ReservationKind::Partner { other } = r.kind {
                        partners.push((h, other));
                    }
                }
                live
            });
            !v.is_empty()
        });
        for (h, b) in dropped {
            self.unfile_reservation(h, b);
        }
        for (h, other) in partners {
            self.unfile_partner(h, other);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{TICKS_PER_DAY, TICKS_PER_HOUR};

    /// The partner index is the one `rebuild_reservation_index` builds from
    /// the reservations, and `partner_reserved` is `reserved_partners`' set
    /// for every holder and every agent anyone holds. Returns how many
    /// Partner reservations the city holds.
    fn check(w: &World, label: &str) -> usize {
        let mut fresh = w.clone();
        fresh.rebuild_reservation_index();
        assert_eq!(w.res_partners, fresh.res_partners, "{label}: the partner index drifted");
        let held: Vec<EntityId> = w.res_partners.keys().copied().collect();
        let mut holders: Vec<EntityId> = w.reservations.keys().copied().collect();
        // scan-ok: a unit test.
        holders.extend(w.citizens().iter().step_by(50));
        for &h in &holders {
            let set = crate::systems::social::reserved_partners(w, h);
            for &o in held.iter().chain(&holders) {
                assert_eq!(w.partner_reserved(o, h), set.contains(&o), "{label}: {o} held, for {h}");
            }
        }
        w.reservations.values().flatten().filter(|r| matches!(r.kind, ReservationKind::Partner { .. })).count()
    }

    /// The perf pass phase 4: the partner index through the evening's
    /// Socialise and Court plans (reserved, released, expired), a death of
    /// an agent someone holds, and a holder's death, each read at once.
    /// Without one of its write paths the index drifts from the rebuild.
    #[test]
    fn test_partner_index_matches_the_reservations() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 17 * TICKS_PER_HOUR);
        let mut seen = 0;
        for _ in 0..60 {
            w.run_ticks(5);
            seen += check(&w, "the evening");
            if seen > 0 && !w.res_partners.is_empty() {
                break;
            }
        }
        // An agent someone holds dies, then a holder.
        let held = w.res_partners.keys().next().copied().expect("no Partner reservation all evening");
        w.remove_agent(held);
        check(&w, "the held one gone");
        if let Some(h) = w.res_partners.values().flat_map(|hs| hs.keys()).next().copied() {
            w.remove_agent(h);
            check(&w, "a holder gone");
        }
        w.run_ticks(3 * TICKS_PER_HOUR);
        check(&w, "three hours on");
    }
}
