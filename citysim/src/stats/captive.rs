//! M16b phase 2: captives' group.

use serde::{Deserialize, Serialize};

cols! {
    /// M16b phase 2: the captives' and the new kinds' CSV columns, after
    /// leverage's. `captives_held`, `prison_pop` (captives held at a Private
    /// Prison) and `prisons` are day-end snapshots; the rest daily counters.
    /// `hostages_freed` counts every release (`ransomed`: a demand met,
    /// `rescued_captives`: a Rescue's win, `escaped`: the hold gone, the rest
    /// at expiry); `flow_ransom` the coins paid for a hostage.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct CaptiveCols {
        pub captives_held: u32,
        pub prison_pop: u32,
        pub prisons: u32,
        pub hostages_taken: u32,
        pub hostages_freed: u32,
        pub ransomed: u32,
        pub rescued_captives: u32,
        pub escaped: u32,
        pub hostages_killed: u32,
        pub harvest_handovers: u32,
        pub captives_unfed: u32,
        pub lev_hostage_tries: u32,
        pub lev_hostage_ok: u32,
        /// Posted per M16b phase-2 kind (`ContractKind::PHASE2` order).
        pub k_posted: [u32; 6] => with(
            |h| h.extend(["k_abduct_posted", "k_abduct_done", "k_rescue_posted", "k_rescue_done", "k_escort_posted", "k_escort_done", "k_steal_posted", "k_steal_done", "k_deliver_posted", "k_deliver_done", "k_spin_posted", "k_spin_done"].map(String::from)),
            |s, v, _| v.extend((0..6).flat_map(|k| [s.k_posted[k].to_string(), s.k_done[k].to_string()]))
        ),
        /// Fulfilled per phase-2 kind.
        pub k_done: [u32; 6] => skip,
        pub fabricated: u32,
        pub exposed: u32,
        pub steal_runs: u32,
        pub flow_ransom: i64,
    }
}

impl CaptiveCols {
    pub fn is_zero(&self) -> bool {
        *self == CaptiveCols::default()
    }

    /// The slot of a phase-2 kind (`None` for the others).
    pub fn slot(k: crate::contract::ContractKind) -> Option<usize> {
        crate::contract::ContractKind::PHASE2.iter().position(|&x| x == k)
    }
}
