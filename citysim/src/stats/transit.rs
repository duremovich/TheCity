//! Transit (docs/TRANSIT.md § 4): the El's group.

use serde::{Deserialize, Serialize};

cols! {
    /// Transit (docs/TRANSIT.md § 4): the El's day. `commute_ride_*` are the
    /// commutes that rode (ticks and Manhattan tiles door to door), read as
    /// `commute_tpt_ride` beside `commute_tpt_walk`/`_drive`.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TransitCols {
        pub rides_full: u32,
        pub rides_coarse: u32,
        /// Phase 2 (T13): Statistical commute rides (two a riding workday).
        pub rides_stat: u32,
        /// Coins of fare paid (`Flow::Fare`).
        pub fares: i64,
        pub fare_dodges: u32,
        pub refused_no_route: u32,
        pub refused_fare: u32,
        pub refused_slow: u32,
        pub refused_danger: u32,
        /// A ride given up at the platform (no service, or the fare spent).
        pub ride_abandoned: u32,
        /// The most boardings at one stop in one hour today.
        pub peak_stop_load: u32,
        /// Coins of power paid by the Treasury to the World.
        pub transit_power: i64,
        /// Phase 3: the El's maintenance crews funded from fares (`TransitState.staff`).
        pub el_staff: u32,
        pub commute_ride_ticks: u64 => skip,
        pub commute_ride_tiles: u64 => with(
            |h| h.push("commute_tpt_ride".to_string()),
            |s, v, _| {
                let tpt = if s.commute_ride_tiles > 0 {
                    s.commute_ride_ticks as f32 / s.commute_ride_tiles as f32
                } else {
                    0.0
                };
                v.push(format!("{tpt:.2}"))
            }
        ),
        /// Phase 2, the parity test's counters (not columns): the bodies'
        /// commutes to work and those that rode; the Statistical workdays whose
        /// commute was judged (`transit::stat_commute`) and those that rode.
        pub commutes: u32 => skip,
        pub commute_rides: u32 => skip,
        pub stat_commutes: u32 => skip,
        pub stat_commute_rides: u32 => skip,
        /// Parity (arrests): the Statistical non-commute rides
        /// (`transit::stat_errand`; not a column).
        pub stat_errand_rides: u32 => skip,
    }
}

impl TransitCols {
    pub fn is_zero(&self) -> bool {
        *self == TransitCols::default()
    }
}
