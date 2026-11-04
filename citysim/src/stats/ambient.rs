//! Scale S5: the Ambient tier's group.

use serde::{Deserialize, Serialize};

cols! {
    /// Scale S5: the Ambient tier's day (`systems::ambient::close_day`).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct AmbientCols {
        /// Ambient flats (occupied or not, materialised or not).
        pub flats: u64 => col("ambient_flats"),
        /// Occupied Ambient flats not yet materialised, and their adults.
        pub households: u64 => col("ambient_households"),
        pub adults: u64 => col("ambient_adults"),
        /// Adults materialised today (every path), the labour pool's hires and
        /// the households materialised as arrivals.
        pub materialised: u32 => col("ambient_materialised"),
        pub hires: u32 => col("ambient_hires"),
        pub arrivals: u32 => col("ambient_arrivals"),
        /// Coins crossed in from the World with them today.
        pub coins_in: i64 => col("ambient_coins_in"),
        /// The materialised adults alive at the day's close, and every adult
        /// materialised since day 0.
        pub living: u32 => col("ambient_living"),
        pub total: u64 => col("ambient_total"),
        /// The districts' labour pool and food demand estimates, summed.
        pub labour: f32 => col("ambient_labour", "{:.1}"),
        pub food: f32 => col("ambient_food", "{:.1}"),
        /// M17 phase 1: the Ambient book's day (`AmbientDay`): food units wanted
        /// and bought, venue visits wanted and made, the coins it paid for each
        /// (`Flow::AmbientSale`), and its appetite.
        pub food_wanted: u32 => col("ambient_food_wanted"),
        pub food_bought: u32 => col("ambient_food_bought"),
        pub visits_wanted: u32 => col("ambient_visits_wanted"),
        pub visits_made: u32 => col("ambient_visits_made"),
        pub paid_food: i64 => col("flow_ambient_food"),
        pub paid_visits: i64 => col("flow_ambient_visits"),
        pub appetite: f32 => col("ambient_appetite", "{:.3}"),
    }
}
