//! M16a: the contract records' group.

use serde::{Deserialize, Serialize};

cols! {
    /// M16a (plan C38): the contract records' CSV columns, in header order
    /// (counters of a game abstraction: records posted, taken, settled).
    /// `contracts_open`, `regulars`, `live_parties`, `live_queued`,
    /// `guards_on_take`, `escrow_held`, `escrow_leak`, `escrow_stuck` and the per-Fixer slots
    /// are day-end snapshots; the rest are daily counters (zero until their
    /// phase: `hits_squad`, `strikes_declined_pol`, `sold_out` phase 2;
    /// `contract_cleared`, `accessory*`, `interrogations*`, `guards_on_take`,
    /// `fixer_runs`, `f*_heat` phase 3; `live_queued` phase 4).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ContractCols {
        pub contracts_open: u32,
        pub contracts_posted: u32,
        pub contracts_fulfilled: u32,
        pub contracts_failed: u32,
        pub contracts_expired: u32,
        pub contracts_cancelled: u32,
        pub reneged: u32,
        /// Posted per kind (`ContractKind::ALL` order).
        pub k_posted: [u32; 4] => with(
            |h| h.extend(["hit", "beat", "guard", "locate"].iter().flat_map(|k| [format!("k_{k}_posted"), format!("k_{k}_done")])),
            |s, v, _| v.extend((0..4).flat_map(|k| [s.k_posted[k].to_string(), s.k_done[k].to_string()]))
        ),
        /// Fulfilled per kind.
        pub k_done: [u32; 4] => skip,
        pub hits_done: u32,
        pub hits_squad: u32,
        /// Fulfilled-or-taken solo Hits whose solo estimate was below `squad_below`.
        pub hits_solo_weak: u32,
        pub strikes_declined_pol: u32,
        pub sold_out: u32,
        /// Hits fulfilled (live and ledger).
        pub contract_murders: u32,
        pub contract_cleared: u32,
        pub contract_holes: u32,
        pub contract_hole_wrong: u32,
        pub accessory: u32,
        pub interrogations: u32,
        pub interrogations_won: u32,
        pub bounties_paid: u32,
        pub guards_on_take: u32,
        pub fixer_runs: u32,
        pub regulars: u32,
        pub live_parties: u32,
        pub live_queued: u32,
        pub escrow_held: i64,
        pub escrow_leak: i64,
        /// Review fix (probe): escrow held by closed records (must be 0).
        pub escrow_stuck: i64,
        /// Per Fixer slot (by building id).
        pub f_heat: [f32; 4] => with(
            |h| h.extend((0..4).map(|i| format!("f{i}_heat"))),
            |s, v, _| v.extend(s.f_heat.iter().map(|h| format!("{h:.3}")))
        ),
        pub f_income: [i64; 4] => with(
            |h| h.extend((0..4).map(|i| format!("f{i}_income"))),
            |s, v, _| v.extend(s.f_income.iter().map(i64::to_string))
        ),
        pub flow_escrow: i64,
        pub flow_payout: i64,
        pub flow_fixer_cut: i64,
        /// Review fix (phase 3): brain postings refused at `max_open` (a full
        /// board drops them silently otherwise).
        pub contracts_refused_full: u32,
    }
}

impl ContractCols {
    pub fn is_zero(&self) -> bool {
        *self == ContractCols::default()
    }
}
