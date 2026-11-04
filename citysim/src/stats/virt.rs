//! M14: the plane's group.

use super::CORP_SLOTS;
use serde::{Deserialize, Serialize};

cols! {
    /// M14 V43: the plane's CSV columns, in header order. `nodes`, `labs`,
    /// `decks`, `cameras`, `ice_mean_corp`, `data_held` and the per-corp tiers
    /// and Data are day-end snapshots; the rest are daily counters (zero until
    /// their phase).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct VirtCols {
        pub nodes: u32,
        pub labs: u32,
        pub decks: u32,
        pub runs: u32,
        pub runs_ok: u32,
        pub data_made: u32,
        pub data_stolen: u32,
        pub data_wiped: u32,
        pub data_sold: u32,
        pub ledger_hacks: u32,
        pub doors_hacked: u32,
        pub traced: u32,
        pub fried: u32,
        pub flatlined: u32,
        pub hack_arrests: u32,
        pub ice_mean_corp: f32 => fmt("{:.3}"),
        pub ice_spend: i64,
        pub runs_bounced: u32,
        pub runs_captured: u32,
        pub runs_dumped: u32,
        pub hack_arrests_chair: u32,
        pub robots_turned: u32,
        pub blinded: u32,
        pub cameras: u32,
        pub sightings: u32,
        pub ice_raised: u32,
        pub ice_lowered: u32,
        pub tech_gained: u32,
        pub tech_lost: u32,
        pub research_spent: u32,
        pub data_held: u32,
        /// Per corp slot (`CORP_SLOTS`): Chrome, Deck, Industry tiers and Data held.
        pub corps: Vec<[u32; 4]> => with(
            |h| h.extend((1..=CORP_SLOTS).flat_map(|i| ["tier_chrome", "tier_deck", "tier_industry", "data"].map(|c| format!("corp{i}_{c}")))),
            |s, v, _| {
                v.extend((0..s.corps.len().max(CORP_SLOTS)).map(|i| {
                    let [c, d, n, data] = s.corps.get(i).copied().unwrap_or_default();
                    format!("{c},{d},{n},{data}")
                }))
            }
        ),
        pub flow_data: i64,
        pub flow_hack: i64,
        pub flow_ice_upkeep: i64,
        pub flow_research: i64,
        pub flow_terminal: i64,
    }
}
