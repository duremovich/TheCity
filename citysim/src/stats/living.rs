//! L2: the living city's group.

use super::{is_zero_i64, is_zero_u32, DISTRICT_SLOTS};
use serde::{Deserialize, Serialize};

/// L2 (plan L34): the six leisure kinds' CSV names (`BuildingKind::LEISURE` order).
pub const LEISURE_SLOTS: [&str; 6] = ["club", "arcade", "noodle_bar", "fight_pit", "den", "lounge"];

cols! {
    /// L2 (plan L34): the living city's CSV columns, in header order: spec
    /// §§ 1-3 and the plan's additions. Phase 1 fills the § 1 columns; the
    /// rest stay zero until their phase.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct LivingCols {
        pub flow_leisure: i64,
        /// Net to houses: losses minus wins.
        pub flow_gamble: i64,
        pub flow_gamble_win: i64,
        pub flow_tribute: i64,
        pub flow_export: i64,
        /// The day's wage bill of the public-works hires.
        pub flow_public_works: i64,
        /// Σ `flow_wages` ÷ max(Σ `flow_dole`, 1), the day's.
        pub wage_dole_ratio: f32 => fmt("{:.3}"),
        /// Employed adults ÷ adults.
        pub employed_share: f32 => fmt("{:.3}"),
        /// Standing venues per leisure kind (`LEISURE_SLOTS`).
        pub venues: [u32; 6] => with(
            |h| h.extend(LEISURE_SLOTS.iter().map(|k| format!("venues_{k}"))),
            |s, v, _| v.extend(s.venues.iter().map(u32::to_string))
        ),
        /// Visits today per leisure kind.
        pub visits: [u32; 6] => with(
            |h| h.extend(LEISURE_SLOTS.iter().map(|k| format!("visits_{k}"))),
            |s, v, _| v.extend(s.visits.iter().map(u32::to_string))
        ),
        pub fab_parts: u32,
        pub scrap_parts: u32,
        /// Σ the asset sellers' `Flow::Import` coins today.
        pub parts_imported: i64,
        pub works_jobs: u32,
        pub upkeep_mult: f32 => fmt("{:.2}"),
        pub outside_inbound: i64,
        pub outside_minted: i64,
        // § 2 (phase 2).
        pub fun_mean: f32 => fmt("{:.3}"),
        pub fun_satisfied_share: f32 => fmt("{:.3}"),
        pub hangouts: u32,
        pub hangout_contacts_mean: f32 => fmt("{:.3}"),
        pub fronts: u32,
        pub collected: u32,
        pub preached: u32,
        /// Per district slot (`DISTRICT_SLOTS`).
        pub street_density: Vec<f32> => with(
            |h| h.extend((1..=DISTRICT_SLOTS).map(|i| format!("d{i}_street_density"))),
            |s, v, _| {
                v.extend(
                    (0..s.street_density.len().max(DISTRICT_SLOTS))
                        .map(|i| format!("{:.3}", s.street_density.get(i).copied().unwrap_or(0.0))),
                )
            }
        ),
        // § 3 (phase 4).
        pub fv_killed: u32,
        pub fv_assaulted: u32,
        pub fv_robbed: u32,
        pub fv_abducted: u32,
        pub fv_bound: u32,
        pub fv_unknown: u32,
        pub fv_capped: u32,
        pub fv_bound_wrong: u32,
        pub kill_rate_body: f32 => fmt("{:.3}"),
        pub kill_rate_stat: f32 => fmt("{:.3}"),
        pub kill_rate_body_civ: f32 => fmt("{:.3}"),
        pub kill_rate_stat_civ: f32 => fmt("{:.3}"),
        // Phase 5 gate probes (not CSV columns; the L2 gate reads them).
        /// Coins the Statistical leisure pass (`leisure::stat_daily`) paid today.
        #[serde(skip_serializing_if = "is_zero_i64")]
        pub stat_spend: i64 => skip,
        /// The island pass phase 10 (not a CSV column): the midnight pass's
        /// hits not made because their source had no free hand or budget left
        /// (`fviolence::daily`; 0 with `[fviolence.hands]` absent).
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub fv_hands_capped: u32 => skip,
        /// Parts `assets::parts_market` moved today from a Fab, from the Recycler.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub parts_sold_fab: u32 => skip,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub parts_sold_recycler: u32 => skip,
        /// Review fix: street dice's peer coins (`Flow::StreetDice`), a CSV
        /// column after the budget columns (appended; not in `csv()`).
        #[serde(skip_serializing_if = "is_zero_i64")]
        pub flow_street_dice: i64 => skip,
        /// L2 shadow fixes (not CSV columns): shifts paid pro rata after a cut (item 2), no-show
        /// dismissals (item 11), commitments that held a plan against a new
        /// winner (items 1-4, 15), tier dwells that kept a body (item 7).
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub fix_pro_rata: u32 => skip,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub fix_noshow: u32 => skip,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub fix_held: u32 => skip,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub fix_dwell: u32 => skip,
    }
}
