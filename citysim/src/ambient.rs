//! Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier's state. The flats of
//! a map's Homes beyond the simulated residents' beds hold households that
//! are not simulated: a flat stores nothing (its household is a pure function
//! of the world seed, the building and the flat's index), a Home stores its
//! counts and a cursor, a district its daily aggregates. The logic is in
//! `systems::ambient`.

use serde::{Deserialize, Serialize};

use crate::entity::EntityId;

/// One map Home with Ambient flats. Flats are materialised in index order
/// from `cursor`, so the cursor is the whole set of touched flats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmbientHome {
    pub building: EntityId,
    /// Ambient flats (beyond the Home's household slots).
    pub flats: u32,
    /// Occupied flats not yet materialised.
    pub occupied: u32,
    /// Their adults.
    pub adults: u32,
    /// The next flat index to read (flats below it are materialised or vacant).
    pub cursor: u32,
}

/// One district's Ambient aggregates (`systems::ambient::daily`): estimates
/// read by the labour pool and offered to the density ledger; they move no
/// coin and no good.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AmbientDistrict {
    /// Occupied Ambient flats in standing Homes.
    pub households: u32,
    pub adults: u32,
    /// Adults x the district's simulated unemployment.
    pub labour: f32,
    /// Adults x the simulated city's meals per resident yesterday.
    pub food: f32,
    /// Households x the mean rent of the district's Homes.
    pub rent: f32,
    /// Adults x the district's simulated crimes per resident a day (7 days).
    pub crime: f32,
    /// Adults x the district's simulated street density per resident.
    pub street: f32,
}

/// The day's counters (into the CSV's Ambient group at the day's close).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AmbientDay {
    /// Adults materialised today (every path).
    pub materialised: u32,
    /// Labour-pool hires today.
    pub hires: u32,
    /// Households materialised as arrivals today (offers, the pull week, the
    /// dole city's weekly immigrants).
    pub arrivals: u32,
    /// Coins crossed in from the World with them today.
    pub coins_in: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AmbientState {
    /// Every map Home with Ambient flats, in building order.
    pub homes: Vec<AmbientHome>,
    /// Per district (`World::districts` order), yesterday's aggregates.
    pub districts: Vec<AmbientDistrict>,
    /// Donors by Home tier (Sump, Mid, Spire): yesterday's housed, free,
    /// simulated adults, ascending.
    pub donors: [Vec<EntityId>; 3],
    /// The living adults the tier materialised, ascending.
    pub materialised: Vec<EntityId>,
    /// Adults materialised since day 0.
    pub total: u64,
    pub today: AmbientDay,
}

impl AmbientState {
    /// No Ambient tier (the shipped map): nothing saved.
    pub fn is_empty(&self) -> bool {
        self.homes.is_empty() && self.total == 0
    }

    /// Occupied Ambient flats left.
    pub fn households(&self) -> u64 {
        self.homes.iter().map(|h| u64::from(h.occupied)).sum()
    }

    /// Their adults.
    pub fn adults(&self) -> u64 {
        self.homes.iter().map(|h| u64::from(h.adults)).sum()
    }

    /// Ambient flats, occupied or not, materialised or not.
    pub fn flats(&self) -> u64 {
        self.homes.iter().map(|h| u64::from(h.flats)).sum()
    }
}
