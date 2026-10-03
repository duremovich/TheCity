//! The symbolic snapshot the planner searches over.

use serde::{Deserialize, Serialize};

// Thresholds that define the symbolic keys (spec › WorldState comments).
/// `hunger_satisfied`: `needs.hunger >= 0.6`.
pub const HUNGER_SATISFIED: f32 = 0.6;
/// `energy_satisfied`: `needs.energy >= 0.6`.
pub const ENERGY_SATISFIED: f32 = 0.6;
/// `belonging_satisfied`: `needs.belonging >= 0.5`.
pub const BELONGING_SATISFIED: f32 = 0.5;
/// `is_safe`: `needs.safety >= 0.4`.
pub const SAFE: f32 = 0.4;
/// `has_savings`: `coins >= 7 × price_food`.
pub const SAVINGS_DAYS: i64 = 7;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum LocationKey {
    /// Only in goal states / preconditions, never observed.
    #[default]
    Anywhere,
    Home,
    Farm,
    Market,
    Bar,
    Jail,
    Cemetery,
    Hall,
    Hideout,
    Warehouse,
    /// On a non-building tile.
    Street,
    /// The Home bound to `Plan.target` (StealFood(Home), Extort).
    TargetHome,
    /// Last known tile of the `Plan.target` suspect.
    SuspectTile,
    /// Tile of the `Plan.target` corpse.
    CorpseTile,
    /// Next patrol leg.
    PatrolWaypoint,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct WorldState {
    pub at: LocationKey,
    pub hunger_satisfied: bool,
    pub energy_satisfied: bool,
    pub belonging_satisfied: bool,
    pub has_food: bool,
    pub has_coins: bool,
    pub has_savings: bool,
    /// 0: < price, 1: < 2*price, 2: more.
    pub coin_bucket: u8,
    /// `inventory.food` saturating at 3.
    pub food_count: u8,
    pub has_wage_due: bool,
    pub shift_done: bool,
    pub has_spouse: bool,
    pub has_partner_candidate: bool,
    pub is_safe: bool,
    pub threat_removed: bool,
    pub crime_reported: bool,
    pub suspect_jailed: bool,
    pub suspect_cuffed: bool,
    pub in_gang: bool,
    pub gang_task_done: bool,
    pub corpse_buried: bool,
    pub carrying_corpse: bool,
    pub carrying_stolen: bool,
    pub is_dark: bool,
    pub known_corpse: bool,
    pub known_suspect_location: bool,
    pub patrol_leg_done: bool,
    pub food_source_available: bool,
    pub forage_available: bool,
}
