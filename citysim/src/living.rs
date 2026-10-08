//! Life pass L2 (docs/LIFE_L2.md, plan L4): the living city's types. Logic
//! lives in `systems::{living, jobs, budget, outside}`; every L2 path runs
//! behind its section's `on()` (plan L5), so with `[living] enabled =
//! false` none of these is ever written and the M15-closing city is
//! reproduced byte for byte.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::entity::EntityId;

/// Days of `Venue.visits` kept (newest last).
pub const VISIT_DAYS: usize = 7;
/// Days of a corp's import tally kept (`JobsBook.corp_imports`).
pub const IMPORT_DAYS: usize = 14;

/// `Building.venue` (plan L4): a leisure venue's day. Set by
/// `founding::convert` for the six leisure kinds; the Fab has none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Venue {
    /// Today's entry or meal price (`jobs::price_venues`, daily).
    pub price: i64,
    #[serde(default)]
    pub visits_today: u16,
    /// The last `VISIT_DAYS` days' visits, newest last.
    #[serde(default)]
    pub visits: VecDeque<u16>,
    /// The house's gambling margin today (Den, FightPit; phase 2).
    #[serde(default)]
    pub take_today: i64,
    /// A gang front: the gang whose dealers deal here and whose leader
    /// collects (phase 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_of: Option<EntityId>,
    /// Plan field: the house's take since the last `Collect` (phase 2).
    #[serde(default)]
    pub take_week: i64,
    /// Plan field: a house that could not pay a win shuts its table for
    /// the day (phase 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_shut_day: Option<u64>,
    /// Phase 2 (the phase 1 deviation's hand-over): the owner a seeded
    /// venue standing on the city's deed passes to once it earns (a front's
    /// gang, a Club's or the Lounge's megacorp).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heir: Option<EntityId>,
    /// Phase 2 (L18): tonight's bets on a FightPit's bout, `(bettor, stake,
    /// side)` (side `true` backs the fitter Fighter); settled at `bout_hour`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bets: Vec<(EntityId, i64, bool)>,
}

/// L2 § 2 (plan L14): the rung ladder, by coins after `hotel_reserve_meals`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rung {
    Free,
    Cheap,
    Mid,
    High,
}

/// L2 L14: what an `Unwind` (or a Socialise's HangOut, or a Preach) was
/// planned for: `World::unwind`, written when the plan is built and read
/// by the executor (`LocationKey::Spot`) and the HangOut registry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnwindPick {
    pub rung: Rung,
    /// The satisfier: `Enjoy`, `Gamble`, `EatOut`, `Drink`, `HangOut` or `Preach`.
    pub act: crate::goap::ActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot: Option<crate::components::TilePos>,
    /// The pick's score (the inspector shows it).
    #[serde(default)]
    pub score: f32,
}

/// L2 L15: where a street spot stands.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SpotKind {
    /// The street outside a Bar's, Market's, Club's or NoodleBar's door.
    Door(EntityId),
    /// A gang's Hideout door (its members only).
    Hideout(EntityId),
    /// A Sump fire barrel.
    Barrel,
}

/// L2 L15: a HangOut spot (`World::spots`, per district, rebuilt daily).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spot {
    pub tile: crate::components::TilePos,
    pub kind: SpotKind,
}

/// `World::budget` (spec § 1, plan L10): the Treasury band's state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CityBudget {
    /// Scales the `[corps] upkeep` corps pay the Treasury; 1.0 at seed.
    pub upkeep_mult: f32,
    /// Public-works Sanitation jobs posted and standing (vacancies + hires).
    pub works_posted: u16,
    /// Days in a row on `band_side`.
    pub band_days: u16,
    /// Plan field: -1 below `lo`, 0 inside, 1 above `hi`.
    #[serde(default)]
    pub band_side: i8,
}

impl Default for CityBudget {
    fn default() -> Self {
        CityBudget { upkeep_mult: 1.0, works_posted: 0, band_days: 0, band_side: 0 }
    }
}

impl CityBudget {
    /// `skip_serializing_if`: a world whose band never moved saves as before.
    pub fn is_default(&self) -> bool {
        *self == CityBudget::default()
    }
}

/// `World::jobs_book` (plan L4): the jobs economy's running tallies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JobsBook {
    /// Per corp, the coins its own sellers paid as `Flow::Import` per day,
    /// the last `IMPORT_DAYS` days, today last (the Fab trigger, L9).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub corp_imports: BTreeMap<EntityId, VecDeque<i64>>,
    /// Public-works hires (city Sanitation on top of the M12 headcount),
    /// oldest first (L10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub works: Vec<EntityId>,
    /// Plan field (L8): sweepers whose `Sweep` completed today; the D23
    /// midnight credit skips them (no double count), then clears it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swept: Vec<EntityId>,
}

impl JobsBook {
    pub fn is_empty(&self) -> bool {
        self.corp_imports.is_empty() && self.works.is_empty() && self.swept.is_empty()
    }

    /// A corp's imports over the window.
    pub fn imports_of(&self, corp: EntityId) -> i64 {
        self.corp_imports.get(&corp).map_or(0, |v| v.iter().sum())
    }
}
