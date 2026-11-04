//! M16b: leverage's group.

use serde::{Deserialize, Serialize};

cols! {
    /// M16b § 4 (phase 1): leverage's CSV columns, after the Hit market's and
    /// before transit's. `coercions_active`, `protection_active`,
    /// `blackmail_active` and `tribute_active` are day-end snapshots; the rest
    /// are daily counters. `lev_*_tries`/`_ok` count the moves by leverage
    /// kind (the gang shakedown, M15's `extort_tries`, apart); `silenced_threat`
    /// reports withdrawn under a threat; `k_persuade_*` the `Persuade` kind's
    /// records (M16a's `k_*` slots are `ContractCols`').
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct LeverageCols {
        pub coercions_active: u32,
        pub lev_threat_tries: u32,
        pub lev_threat_ok: u32,
        pub lev_secret_tries: u32,
        pub lev_secret_ok: u32,
        pub lev_debt_tries: u32,
        pub lev_debt_ok: u32,
        pub coercions_broken: u32,
        pub protection_made: u32,
        pub protection_paid: u32,
        pub protection_coins: i64,
        pub protection_active: u32,
        pub blackmail_made: u32,
        pub blackmail_active: u32,
        pub secrets_lapsed: u32,
        pub silenced_threat: u32,
        pub coerced_jobs: u32,
        pub coerced_done: u32,
        pub poach_threats: u32,
        pub parleys: u32,
        pub parley_fails: u32,
        pub tribute_active: u32,
        pub fake_jobs: u32,
        pub frauds_filed: u32,
        pub k_persuade_posted: u32,
        pub k_persuade_done: u32,
        pub flow_coerced: i64,
    }
}

impl LeverageCols {
    pub fn is_zero(&self) -> bool {
        *self == LeverageCols::default()
    }

    /// One move by leverage kind.
    pub fn note_try(&mut self, k: crate::contract::LeverageKind, ok: bool) {
        use crate::contract::LeverageKind;
        let (t, o) = match k {
            LeverageKind::Threat => (&mut self.lev_threat_tries, &mut self.lev_threat_ok),
            LeverageKind::Secret => (&mut self.lev_secret_tries, &mut self.lev_secret_ok),
            LeverageKind::Debt => (&mut self.lev_debt_tries, &mut self.lev_debt_ok),
            // M16b phase 2: counted in `CaptiveCols`.
            LeverageKind::Hostage => return,
        };
        *t += 1;
        if ok {
            *o += 1;
        }
    }
}
