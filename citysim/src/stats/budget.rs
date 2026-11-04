//! L2 phase 3: the LOD budget's and the churn's group.

use serde::{Deserialize, Serialize};

cols! {
    /// L2 phase 3 (L34, spec § 4): the LOD budget's and the churn's columns.
    /// `tier_held` is every sentenced agent and `tier_held_body` those of them
    /// with a body; with `[lod] budget` on, `tier_full`, `tier_coarse` and
    /// `tier_stat` exclude the sentenced (off: as M15). `gang_bodies` and
    /// `gang_stat` are free members by tier at the day's end; `stat_*` the
    /// Statistical GangWork days that acted; `aborts` the plan failures
    /// (`step_agent`'s `Failed`, `plan_for`'s unplannable), by the failing
    /// step: Scavenge, Sleep, CheckIn, a full non-bed door (`aborts_seat`);
    /// `rough_sleeps` the failed bed steps turned into a street Sleep (L30);
    /// `scavenge_dry` the dry Scavenge hours made `Done` (L29).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct BudgetCols {
        pub tier_held: u32,
        pub tier_held_body: u32,
        pub held_fed: u32,
        pub gang_bodies: u32,
        pub gang_stat: u32,
        pub stat_extorts: u32,
        pub stat_claims: u32,
        pub stat_deals: u32,
        pub aborts: u32,
        pub aborts_scavenge: u32,
        pub aborts_sleep: u32,
        pub aborts_checkin: u32,
        pub aborts_seat: u32,
        pub rough_sleeps: u32,
        pub scavenge_dry: u32,
    }
}

impl BudgetCols {
    pub fn is_zero(&self) -> bool {
        *self == BudgetCols::default()
    }
}
