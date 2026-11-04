//! M16b phase 3: the wounded's group.

use serde::{Deserialize, Serialize};

cols! {
    /// M16b phase 3: the wounded's CSV columns, after the captives'.
    /// `down_now` and `trauma_subs` are day-end snapshots; the rest daily
    /// counters. `downed` counts wounds (a fight's death made a wound),
    /// `wounds_capped` the would-be wounds that stayed deaths at the quota,
    /// `finished_off` the downed killed where they lay, `rescued_*` the
    /// stabilisations by the carrier's tie, `stat_wounds` off-screen killings
    /// survived as assaults (the Statistical split).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct WoundedCols {
        pub downed: u32,
        pub wounds_capped: u32,
        pub bled_out: u32,
        pub finished_off: u32,
        pub stabilised: u32,
        pub recovered: u32,
        pub rescued_kin: u32,
        pub rescued_gang: u32,
        pub rescued_colleague: u32,
        pub rescued_guard: u32,
        pub trauma_calls: u32,
        pub trauma_saves: u32,
        pub down_now: u32,
        pub trauma_subs: u32,
        pub trauma_subs_new: u32,
        pub trauma_lapsed: u32,
        pub stat_wounds: u32,
        pub flow_trauma: i64,
    }
}

impl WoundedCols {
    pub fn is_zero(&self) -> bool {
        *self == WoundedCols::default()
    }
}
