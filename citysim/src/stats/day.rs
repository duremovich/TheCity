//! The day's row: the city-wide columns, the slot groups and every domain group in CSV order.

use super::{
    AmbientCols, BudgetCols, CaptiveCols, ContractCols, EconCols, GovCols, HealthCols, HitCols, Jobs2Cols, JobsCols,
    LeverageCols, LivingCols, OutsideCols, TransitCols, VirtCols, WordCols, WoundedCols, CORP_SLOTS, DISTRICT_SLOTS,
    STATS_HISTORY_CAP,
};
use crate::components::CorpOrder;
use crate::components::MAX_DISTRICTS;
use crate::time::Season;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// M12 D45: one district's CSV numbers: coverage, control code, litter,
/// unrest, crime rate, guards.
pub type DistrictCols = (f32, u8, f32, f32, f32, u16);

cols! {
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct DayRow {
        pub day: u64,
        pub season: Season,
        // --- snapshots, taken at the end of the day ---
        pub population: u32,
        pub employed: u32,
        pub homeless: u32,
        pub jailed: u32,
        pub gang_members: u32,
        pub food_market: u32,
        pub food_warehouse: u32,
        pub food_pantry: u32,
        pub price: i64,
        pub treasury: i64,
        // --- counters, bumped by systems during the day ---
        pub thefts: u32,
        pub arrests: u32,
        pub deaths_starvation: u32,
        pub deaths_old_age: u32,
        pub deaths_violence: u32,
        pub births: u32,
        pub immigrants: u32,
        pub emigrants: u32,
        pub burials: u32,
        /// Goals displaced mid-plan by a different winner (flapping). Raw count;
        /// divided by population into `goal_changes_per_agent` at day end.
        pub goal_changes: u32 => skip,
        pub mean_hunger: f32 => fmt("{:.3}"),
        pub mean_mood: f32 => fmt("{:.3}"),
        pub goal_changes_per_agent: f32 => fmt("{:.3}"),
        /// M10 counters: holes opened, bound to an actor, closed as Unknown, and
        /// Statistical agents killed off screen.
        #[serde(default)]
        pub holes_opened: u32,
        /// M10 snapshots: open holes and the tier sizes at day end.
        #[serde(default)]
        pub holes_open: u32,
        #[serde(default)]
        pub holes_bound: u32,
        #[serde(default)]
        pub holes_unknown: u32,
        #[serde(default)]
        pub deaths_violence_offscreen: u32,
        #[serde(default)]
        pub tier_full: u32,
        #[serde(default)]
        pub tier_coarse: u32,
        #[serde(default)]
        pub tier_stat: u32,
        /// M11 counters: evicted adults, rent collected, adults short on a due
        /// rent, adults re-housed.
        #[serde(default)]
        pub evictions: u32,
        #[serde(default)]
        pub rent_paid: i64,
        #[serde(default)]
        pub rent_short: u32,
        #[serde(default)]
        pub housed: u32,
        /// M11 D3 ledger: gross coins moved per flow today (`ownership::pay` /
        /// `charge`). `flow_tax` is tax moved into the Treasury from other
        /// payers (owner revenue and non-city wages); a city wage's withheld tax
        /// never leaves the Treasury and is not counted. `flow_dole` is the dole
        /// paid; `flow_other` is sales, subsidies, bribes, jail food, SellFood.
        #[serde(default)]
        pub flow_food: i64,
        #[serde(default)]
        pub flow_drink: i64,
        #[serde(default)]
        pub flow_wages: i64,
        #[serde(default)]
        pub flow_rent: i64,
        #[serde(default)]
        pub flow_upkeep: i64,
        #[serde(default)]
        pub flow_wholesale: i64,
        /// Of `flow_wholesale`: the city buying a non-city Farm's overflow into
        /// the Reserve, and non-city Markets buying their restock from it (D5).
        #[serde(default)]
        pub flow_overflow: i64,
        #[serde(default)]
        pub flow_restock: i64,
        #[serde(default)]
        pub flow_contract: i64 => fmt("{:.3}"),
        #[serde(default)]
        pub flow_tax: i64 => fmt("{:.3}"),
        #[serde(default)]
        pub flow_dole: i64,
        #[serde(default)]
        pub flow_other: i64,
        /// M11 snapshots: coins in living adults' wallets, their Gini, and the
        /// richest tenth's share (VISION: an unequal city from day one).
        #[serde(default)]
        pub wallets: i64,
        #[serde(default)]
        pub wallet_gini: f32,
        #[serde(default)]
        pub wallet_top10: f32,
        /// M11 D38 snapshot: each seeded corp's treasury and order by slot
        /// (`None` once dissolved).
        #[serde(default)]
        pub corps: Vec<Option<(i64, CorpOrder)>> => with(
            |h| h.extend((1..=CORP_SLOTS).flat_map(|i| [format!("corp{i}_treasury"), format!("corp{i}_order")])),
            |s, v, _| {
                v.extend((0..s.corps.len().max(CORP_SLOTS)).map(|i| match s.corps.get(i).copied().flatten() {
                    Some((t, o)) => format!("{t},{o}"),
                    None => "0,-".to_string(),
                }))
            }
        ),
        /// M11 phase 3 counters: buildings that changed hands by sale (hostile
        /// acquisitions and bankruptcy sales), corps gone bankrupt; and a
        /// snapshot: corp-niche pairs at monopoly at day end.
        #[serde(default)]
        pub acquisitions: u32,
        #[serde(default)]
        pub bankruptcies: u32,
        #[serde(default)]
        pub monopolies: u32,
        /// M11 phase 4 counters: NPC foundings (`Register`; a corp's `Grow` build
        /// is not counted), incorporations, strikes.
        #[serde(default)]
        pub foundings: u32,
        #[serde(default)]
        pub incorporations: u32,
        #[serde(default)]
        pub strikes: u32,
        /// M11 phase 4 snapshots from `World::classes` (computed at midnight):
        /// unrest per class, class sizes, Street happiness.
        #[serde(default)]
        pub unrest_corp: f32 => fmt("{:.3}"),
        #[serde(default)]
        pub unrest_street: f32 => fmt("{:.3}"),
        #[serde(default)]
        pub unrest_dreg: f32 => fmt("{:.3}"),
        #[serde(default)]
        pub class_corp: u32,
        #[serde(default)]
        pub class_street: u32,
        #[serde(default)]
        pub class_dreg: u32,
        #[serde(default)]
        pub happiness_street: f32 => fmt("{:.3}"),
        /// M12 D45 snapshot: per district slot (`DISTRICT_SLOTS`).
        #[serde(default)]
        pub districts: Vec<DistrictCols> => with(
            |h| {
                for i in 1..=DISTRICT_SLOTS {
                    h.extend(["coverage", "control", "litter", "unrest", "crime", "guards"].map(|c| format!("d{i}_{c}")));
                }
            },
            |s, v, _| {
                v.extend((0..s.district_slots()).map(|i| {
                    let (cov, ctrl, litter, unrest, crime, guards) = s.districts.get(i).copied().unwrap_or_default();
                    format!("{cov:.3},{ctrl},{litter:.3},{unrest:.3},{crime:.3},{guards}")
                }))
            }
        ),
        /// M12 D45: squatters, derelict buildings and gangs are day-end
        /// snapshots; hotel nights, Vagrancy fines and sentences, riots and
        /// crossfire hits are daily counters (zero until their phase). The Dreg
        /// adults are `class_dreg` (M12 review: the `dregs` column repeated it).
        #[serde(default)]
        pub hotel_nights: u32,
        #[serde(default)]
        pub squatters: u32,
        #[serde(default)]
        pub derelicts: u32,
        #[serde(default)]
        pub vagrancy: u32,
        #[serde(default)]
        pub riots: u32,
        #[serde(default)]
        pub crossfire: u32,
        #[serde(default)]
        pub gangs: u32,
        /// M12 D38: raids and breakouts that reached a door under full cover
        /// (the stance turned mid-march); not a CSV column (the gate reads it).
        #[serde(default)]
        pub raids_into_cover: u32 => skip,
        /// M13 D49: vehicles by kind, chromed agents, the mean sanity of adults
        /// with a Body, hooked adults, the legal-stims lever and posted robots
        /// are day-end snapshots; the rest are daily counters (zero until their
        /// phase). `commute_tpt_*` are ticks per Manhattan tile of the day's
        /// arrivals at work, walked and driven.
        #[serde(default)]
        pub vehicles_moto: u32,
        #[serde(default)]
        pub vehicles_car: u32,
        #[serde(default)]
        pub vehicles_truck: u32,
        #[serde(default)]
        pub vehicles_flyer: u32,
        #[serde(default)]
        pub truck_hauls: u32,
        #[serde(default)]
        pub walk_hauls: u32,
        #[serde(default)]
        pub commute_tpt_walk: f32 => fmt("{:.3}"),
        #[serde(default)]
        pub commute_tpt_drive: f32 => fmt("{:.3}"),
        #[serde(default)]
        pub chrome_installs: u32,
        #[serde(default)]
        pub chrome_agents: u32,
        #[serde(default)]
        pub mean_sanity: f32 => fmt("{:.3}"),
        #[serde(default)]
        pub episodes: u32,
        #[serde(default)]
        pub hooked: u32,
        #[serde(default)]
        pub stims_dealt: u32,
        #[serde(default)]
        pub stims_legal: u32,
        #[serde(default)]
        pub dealing_reports: u32,
        #[serde(default)]
        pub repos: u32,
        #[serde(default)]
        pub impounds: u32,
        #[serde(default)]
        pub crashes: u32,
        #[serde(default)]
        pub crash_deaths: u32,
        #[serde(default)]
        pub vehicle_thefts: u32,
        #[serde(default)]
        pub chops: u32,
        #[serde(default)]
        pub abductions: u32,
        #[serde(default)]
        pub stripped: u32,
        #[serde(default)]
        pub robots: u32,
        #[serde(default)]
        pub flow_asset: i64,
        #[serde(default)]
        pub flow_asset_upkeep: i64,
        #[serde(default)]
        pub flow_finance: i64,
        #[serde(default)]
        pub flow_import: i64,
        #[serde(default)]
        pub flow_stims: i64,
        #[serde(default)]
        pub flow_parts: i64,
        #[serde(default)]
        pub flow_treatment: i64,
        #[serde(default)]
        pub overdoses: u32,
        #[serde(default)]
        pub harvests: u32,
        #[serde(default)]
        pub stripped_window: u32,
        #[serde(default)]
        pub gang_income: i64,
        #[serde(default)]
        pub gang_income_dealing: i64,
        #[serde(default)]
        pub episodes_by_law: u32,
        #[serde(default)]
        pub treatments: u32,
        #[serde(default)]
        pub detoxes: u32,
        /// M14 V43: the Virt columns (zero with the plane off).
        #[serde(default)]
        pub virt: VirtCols => group,
        /// M15 W43: the word's columns (zero with `[gossip]` off).
        #[serde(default)]
        pub word: WordCols => group,
        /// L2 (plan L34): the living city's columns (zero with `[living]` off,
        /// but for `parts_imported`).
        #[serde(default)]
        pub living: LivingCols => group,
        /// L2 phase 3 (L34): the LOD budget's and the churn's columns.
        #[serde(default, skip_serializing_if = "BudgetCols::is_zero")]
        pub budget: BudgetCols => with(
            |h| {
                BudgetCols::header_into(h);
                h.push("flow_street_dice".to_string())
            },
            |s, v, d| {
                s.budget.csv_into(v, d);
                v.push(s.living.flow_street_dice.to_string())
            }
        ),
        /// M16a (plan C38): the contract records' columns (zero with
        /// `[contracts]` off; created whole in phase 1, zeros until their phase).
        #[serde(default, skip_serializing_if = "ContractCols::is_zero")]
        pub contract: ContractCols => group,
        /// The Real economy (plan E45): one group after M16a's, created whole
        /// in phase 1 (zeros until their phase).
        #[serde(default, skip_serializing_if = "EconCols::is_zero")]
        pub econ: EconCols => group,
        /// Jobs v2 (plan J34): the jobs pass's group, after the Real economy's
        /// and before `ticks_per_sec`; later phases append to it.
        #[serde(default, skip_serializing_if = "JobsCols::is_zero")]
        pub jobs: JobsCols => group,
        /// Hunger as sickness (docs/HUNGER_SICKNESS.md § 9): after the jobs
        /// group, before `ticks_per_sec` (zeros with `[sickness]` off).
        #[serde(default, skip_serializing_if = "HealthCols::is_zero")]
        pub health: HealthCols => group,
        /// The Hit market (docs/HIT_MARKET.md § 7): after the jobs group,
        /// before `ticks_per_sec`.
        #[serde(default, skip_serializing_if = "HitCols::is_zero")]
        pub hit: HitCols => group,
        /// M16b § 4 (phase 1): leverage's group, after the Hit market's.
        #[serde(default, skip_serializing_if = "LeverageCols::is_zero")]
        pub leverage: LeverageCols => group,
        /// M16b phase 2: captives and the new kinds, after leverage's.
        #[serde(default, skip_serializing_if = "CaptiveCols::is_zero")]
        pub captive: CaptiveCols => group,
        /// M16b phase 3: the wounded, after the captives'.
        #[serde(default, skip_serializing_if = "WoundedCols::is_zero")]
        pub wounded: WoundedCols => group,
        /// M16b phase 4: boards, tags and attention, after the wounded's.
        #[serde(default, skip_serializing_if = "GovCols::is_zero")]
        pub gov: GovCols => group,
        /// Transit (docs/TRANSIT.md § 4): the El's group, after the hit group
        /// and before `ticks_per_sec` (zero, and not saved, with transit off).
        #[serde(default, skip_serializing_if = "TransitCols::is_zero")]
        pub transit: TransitCols => group,
        /// Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier's group, after the
        /// transit group and before `ticks_per_sec`, only on a world with an
        /// Ambient tier (`None` and no columns otherwise: the 2k CSV is unchanged;
        /// the header from `systems::stats::csv_header`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ambient: Option<AmbientCols> => opt,
        /// M17 phase 2: the parents' group, after the Ambient group and before
        /// `ticks_per_sec`, only with the parents on (`None` and no columns
        /// otherwise; the header from `systems::stats::csv_header`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub outside: Option<OutsideCols> => opt,
        /// Jobs & lives (docs/JOBS_AND_LIVES.md § 12): the milestone's group,
        /// after the parents' and before `ticks_per_sec`, only with `[jobs2]
        /// enabled` (`None` and no columns otherwise: the off CSV is unchanged;
        /// the header from `systems::stats::csv_header`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jobs2: Option<Jobs2Cols> => opt,
        /// Filled in by the runner (the library has no clock).
        pub ticks_per_sec: f32 => fmt("{:.0}"),
    }
}

impl DayRow {
    /// Scale S2: the row's district slots, [`DISTRICT_SLOTS`] or the run's
    /// districts when more (`districts` holds one entry per district).
    pub fn district_slots(&self) -> usize {
        self.districts.len().clamp(DISTRICT_SLOTS, MAX_DISTRICTS)
    }

    pub fn new(day: u64) -> Self {
        DayRow { day, season: Season::of_day(day), ..Default::default() }
    }

    /// One CSV line matching the run's header (`systems::stats::csv_header`), without a trailing newline.
    pub fn csv_row(&self) -> String {
        let mut v = Vec::new();
        self.csv_into(&mut v, self.district_slots());
        v.join(",")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DailyStats {
    /// The day in progress.
    pub current: DayRow,
    /// Finished days, oldest first, capped at [`STATS_HISTORY_CAP`].
    pub history: VecDeque<DayRow>,
}

impl DailyStats {
    pub fn new() -> Self {
        DailyStats { current: DayRow::new(0), history: VecDeque::new() }
    }

    /// Close the current day: move it into history and start the next.
    pub fn roll(&mut self, next_day: u64) -> &DayRow {
        let finished = std::mem::replace(&mut self.current, DayRow::new(next_day));
        if self.history.len() >= STATS_HISTORY_CAP {
            self.history.pop_front();
        }
        self.history.push_back(finished);
        self.history.back().expect("just pushed")
    }
}

impl Default for DailyStats {
    fn default() -> Self {
        DailyStats::new()
    }
}
