//! M15: the word's group.

use super::{CORP_SLOTS, GANG_SLOTS};
use serde::{Deserialize, Serialize};

cols! {
    /// M15 W43: the word's CSV columns, in header order: the spec's § 10 list,
    /// the plan's additions, per gang slot dread and heat, per corp slot
    /// honour, standing and competence, the Law's competence, the two ledger
    /// flows. Phase 1 fills `rumours_heard`, `second_hand_share`,
    /// `known_by_killers_median`, `distorted`, `contradicted`,
    /// `rumour_hops_max`, `pool_reach`, the gang axes and the corp honour and
    /// standing; the rest are zero until their phase.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct WordCols {
        pub rumours_heard: u32,
        pub second_hand_share: f32 => fmt("{:.3}"),
        pub known_by_killers_median: f32 => fmt("{:.1}"),
        pub distorted: u32,
        pub grudges: u32,
        pub grudges_inherited: u32,
        pub hunts: u32,
        pub hunts_active: u32,
        pub avenged: u32,
        pub revenge_kills: u32,
        pub chain_max: u32,
        pub vendettas_open: u32,
        pub stories: u32,
        pub planted: u32,
        pub buried: u32,
        pub poached: u32,
        pub talent_lost: u32,
        pub skill_rare_share: f32 => fmt("{:.3}"),
        pub extort_success: u32,
        pub rep_flips: u32,
        pub rumour_hops_max: u32,
        pub pool_reach: f32 => fmt("{:.3}"),
        pub contradicted: u32,
        pub silenced: u32,
        pub hunts_failed: u32,
        pub hunts_abandoned: u32,
        pub guard_body: u32,
        pub expelled: u32,
        pub contracts_lost_honour: u32,
        pub extort_tries: u32,
        /// Round 9 Z1: hunts that ended with a line saying how (avenged, lost,
        /// gave up, the man held or dead first, the hunter held).
        pub hunts_ended: u32,
        /// Z1: grudges handed on (a fixer's hire, kin or the gang, the law, a
        /// cellmate).
        pub hunts_handed: u32,
        /// Z1: grudges over a killing told the killer is held (they wait).
        pub hunts_waiting: u32,
        /// Z1: the hunted who got word of a hunt.
        pub hunted_heard: u32,
        /// Per gang slot (`GANG_SLOTS`): dread, heat.
        pub gangs: Vec<[f32; 2]> => with(
            |h| h.extend((1..=GANG_SLOTS).flat_map(|i| [format!("g{i}_dread"), format!("g{i}_heat")])),
            |s, v, _| {
                v.extend((0..GANG_SLOTS).map(|i| {
                    let [d, h] = s.gangs.get(i).copied().unwrap_or_default();
                    format!("{d:.3},{h:.3}")
                }))
            }
        ),
        /// Per corp slot (`CORP_SLOTS`): honour, standing, competence.
        pub corps: Vec<[f32; 3]> => with(
            |h| h.extend((1..=CORP_SLOTS).flat_map(|i| ["honour", "standing", "competence"].map(|c| format!("c{i}_{c}")))),
            |s, v, _| {
                v.extend((0..s.corps.len().max(CORP_SLOTS)).map(|i| {
                    let [h, st, c] = s.corps.get(i).copied().unwrap_or_default();
                    format!("{h:.3},{st:.3},{c:.3}")
                }))
            }
        ),
        pub law_competence: f32 => fmt("{:.3}"),
        pub flow_ads: i64,
        pub flow_plant: i64,
    }
}
