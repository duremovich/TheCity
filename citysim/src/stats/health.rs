//! Hunger as sickness: the health group.

use serde::{Deserialize, Serialize};

cols! {
    /// Hunger as sickness (docs/HUNGER_SICKNESS.md § 9): the health group,
    /// after `JobsCols` and before `ticks_per_sec`.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct HealthCols {
        pub meals_real: u32,
        pub meals_synth: u32,
        pub meals_dive: u32,
        pub meals_alms: u32,
        /// Citizens at or over `mal_onset` / adults at or over `tox_onset` at midnight.
        pub malnourished: u32,
        pub toxic: u32,
        pub mean_malnutrition: f32 => fmt("{:.4}"),
        pub mean_toxicity: f32 => fmt("{:.4}"),
        /// Adults off sick today (rolled at midnight).
        pub sick_days: u32,
        pub cancer_new: u32,
        pub cancer_diagnosed: u32,
        pub cancer_cured: u32,
        pub deaths_disease: u32,
        /// Scrap meals into the bins today.
        pub bin_meals_in: u32,
        pub bin_contests: u32,
        pub bin_shakedowns: u32,
    }
}

impl HealthCols {
    pub fn is_zero(&self) -> bool {
        *self == HealthCols::default()
    }
}
