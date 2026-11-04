//! The Hit market's group.

use serde::{Deserialize, Serialize};

cols! {
    /// The Hit market (docs/HIT_MARKET.md § 7): organic records (motive not
    /// `None`). Counts are the day's; `hm_price_sum` and `hm_deposit_sum` sum
    /// the day's organic postings (means = sum ÷ posted); `hm_wants` counts
    /// wants written for the walk to a Fixer (H9) and `hm_commissions` those
    /// posted there; `hm_unaffordable` counts tries short of the deposit;
    /// `hm_refused` relatives who would not chip in; `hm_pooled` the coins they
    /// did put in; `hm_recanted` beaten witnesses who took a report back (H15);
    /// `hm_cold` reports gone cold on a dead witness (H14).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct HitCols {
        pub hm_posted_revenge: u32,
        pub hm_posted_rival: u32,
        pub hm_posted_culprit: u32,
        pub hm_posted_silence: u32,
        pub hm_posted_debt: u32,
        pub hm_posted_protect: u32,
        pub hm_wants: u32,
        pub hm_unaffordable: u32,
        pub hm_refused: u32,
        pub hm_pooled: i64,
        pub hm_price_sum: i64,
        pub hm_deposit_sum: i64,
        pub hm_hits_done: u32,
        pub hm_beats_done: u32,
        pub hm_stiffed: u32,
        pub hm_recanted: u32,
        pub hm_cold: u32,
        pub hm_commissions: u32,
    }
}

impl HitCols {
    pub fn is_zero(&self) -> bool {
        *self == HitCols::default()
    }

    /// One organic posting by motive.
    pub fn note_posted(&mut self, m: crate::contract::Motive) {
        use crate::contract::Motive;
        match m {
            Motive::None => {}
            Motive::Revenge => self.hm_posted_revenge += 1,
            Motive::Rival => self.hm_posted_rival += 1,
            Motive::Culprit => self.hm_posted_culprit += 1,
            Motive::Silence => self.hm_posted_silence += 1,
            Motive::Debt => self.hm_posted_debt += 1,
            Motive::Protect => self.hm_posted_protect += 1,
        }
    }
}
