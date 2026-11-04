//! M16b phase 4: boards, tags and attention.

use serde::{Deserialize, Serialize};

cols! {
    /// M16b phase 4: boards, tags, the Patrol and attention, after the
    /// wounded's. `boards` the board corps, `campaign_moves` a challenger's
    /// tries on a swing member, `board_persuades` the campaign's `Persuade`
    /// records; `distracted` the factions below their pool at the snapshot,
    /// `alert_law` the law's alertness ÷ pool, `alert_min` the lowest.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct GovCols {
        pub votes: u32,
        pub votes_swung: u32,
        pub chairs_fallen: u32,
        pub boards: u32,
        pub campaign_moves: u32,
        pub board_persuades: u32,
        pub k_vote_posted: u32,
        pub k_vote_done: u32,
        pub k_tag_posted: u32,
        pub k_tag_done: u32,
        pub tags_planted: u32,
        pub tags_live: u32,
        pub tags_found: u32,
        pub scans: u32,
        pub patrols: u32,
        pub patrol_finds: u32,
        pub scanners_bought: u32,
        pub distracted: u32,
        pub alert_law: f32 => fmt("{:.3}"),
        pub alert_min: f32 => fmt("{:.3}"),
    }
}

impl GovCols {
    pub fn is_zero(&self) -> bool {
        *self == GovCols::default()
    }
}
