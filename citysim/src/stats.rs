//! Daily statistics: counters that systems bump during the day and
//! snapshots taken at the end of it. `--report` prints one row per day.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::components::CorpOrder;
use crate::time::Season;

/// Days of history kept for the city panel's sparklines.
pub const STATS_HISTORY_CAP: usize = 120;

pub const CSV_HEADER: &str = "day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,holes_opened,holes_open,holes_bound,holes_unknown,deaths_violence_offscreen,tier_full,tier_coarse,tier_stat,evictions,rent_paid,rent_short,housed,flow_food,flow_drink,flow_wages,flow_rent,flow_upkeep,flow_wholesale,flow_overflow,flow_restock,flow_contract,flow_tax,flow_dole,flow_other,wallets,wallet_gini,wallet_top10,corp1_treasury,corp1_order,corp2_treasury,corp2_order,corp3_treasury,corp3_order,corp4_treasury,corp4_order,corp5_treasury,corp5_order,corp6_treasury,corp6_order,corp7_treasury,corp7_order,corp8_treasury,corp8_order,corp9_treasury,corp9_order,acquisitions,bankruptcies,monopolies,foundings,incorporations,strikes,unrest_corp,unrest_street,unrest_dreg,class_corp,class_street,class_dreg,happiness_street,d1_coverage,d1_control,d1_litter,d1_unrest,d1_crime,d1_guards,d2_coverage,d2_control,d2_litter,d2_unrest,d2_crime,d2_guards,d3_coverage,d3_control,d3_litter,d3_unrest,d3_crime,d3_guards,d4_coverage,d4_control,d4_litter,d4_unrest,d4_crime,d4_guards,d5_coverage,d5_control,d5_litter,d5_unrest,d5_crime,d5_guards,d6_coverage,d6_control,d6_litter,d6_unrest,d6_crime,d6_guards,d7_coverage,d7_control,d7_litter,d7_unrest,d7_crime,d7_guards,d8_coverage,d8_control,d8_litter,d8_unrest,d8_crime,d8_guards,hotel_nights,squatters,derelicts,vagrancy,riots,crossfire,gangs,vehicles_moto,vehicles_car,vehicles_truck,vehicles_flyer,truck_hauls,walk_hauls,commute_tpt_walk,commute_tpt_drive,chrome_installs,chrome_agents,mean_sanity,episodes,hooked,stims_dealt,stims_legal,dealing_reports,repos,impounds,crashes,crash_deaths,vehicle_thefts,chops,abductions,stripped,robots,flow_asset,flow_asset_upkeep,flow_finance,flow_import,flow_stims,flow_parts,flow_treatment,overdoses,harvests,stripped_window,gang_income,gang_income_dealing,episodes_by_law,treatments,detoxes,\
nodes,labs,decks,runs,runs_ok,data_made,data_stolen,data_wiped,data_sold,ledger_hacks,doors_hacked,traced,fried,flatlined,hack_arrests,ice_mean_corp,ice_spend,\
runs_bounced,runs_captured,runs_dumped,hack_arrests_chair,robots_turned,blinded,cameras,sightings,ice_raised,ice_lowered,tech_gained,tech_lost,research_spent,data_held,\
corp1_tier_chrome,corp1_tier_deck,corp1_tier_industry,corp1_data,corp2_tier_chrome,corp2_tier_deck,corp2_tier_industry,corp2_data,corp3_tier_chrome,corp3_tier_deck,corp3_tier_industry,corp3_data,corp4_tier_chrome,corp4_tier_deck,corp4_tier_industry,corp4_data,corp5_tier_chrome,corp5_tier_deck,corp5_tier_industry,corp5_data,corp6_tier_chrome,corp6_tier_deck,corp6_tier_industry,corp6_data,corp7_tier_chrome,corp7_tier_deck,corp7_tier_industry,corp7_data,corp8_tier_chrome,corp8_tier_deck,corp8_tier_industry,corp8_data,corp9_tier_chrome,corp9_tier_deck,corp9_tier_industry,corp9_data,\
flow_data,flow_hack,flow_ice_upkeep,flow_research,flow_terminal,ticks_per_sec";

/// D38: corp CSV slots (seeding order). M13 D17: 9 (the Tech corp from phase 2).
pub const CORP_SLOTS: usize = 9;

/// M12 D45: district CSV slots (`[districts]` row order); a 9th-12th
/// district is not printed.
pub const DISTRICT_SLOTS: usize = 8;

/// M12 D45: one district's CSV numbers: coverage, control code, litter,
/// unrest, crime rate, guards.
pub type DistrictCols = (f32, u8, f32, f32, f32, u8);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub mean_hunger: f32,
    pub mean_mood: f32,
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
    pub goal_changes: u32,
    pub goal_changes_per_agent: f32,
    /// M10 counters: holes opened, bound to an actor, closed as Unknown, and
    /// Statistical agents killed off screen.
    #[serde(default)]
    pub holes_opened: u32,
    #[serde(default)]
    pub holes_bound: u32,
    #[serde(default)]
    pub holes_unknown: u32,
    #[serde(default)]
    pub deaths_violence_offscreen: u32,
    /// M10 snapshots: open holes and the tier sizes at day end.
    #[serde(default)]
    pub holes_open: u32,
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
    pub flow_contract: i64,
    #[serde(default)]
    pub flow_tax: i64,
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
    pub corps: Vec<Option<(i64, CorpOrder)>>,
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
    pub unrest_corp: f32,
    #[serde(default)]
    pub unrest_street: f32,
    #[serde(default)]
    pub unrest_dreg: f32,
    #[serde(default)]
    pub class_corp: u32,
    #[serde(default)]
    pub class_street: u32,
    #[serde(default)]
    pub class_dreg: u32,
    #[serde(default)]
    pub happiness_street: f32,
    /// M12 D45 snapshot: per district slot (`DISTRICT_SLOTS`).
    #[serde(default)]
    pub districts: Vec<DistrictCols>,
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
    pub raids_into_cover: u32,
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
    pub commute_tpt_walk: f32,
    #[serde(default)]
    pub commute_tpt_drive: f32,
    #[serde(default)]
    pub chrome_installs: u32,
    #[serde(default)]
    pub chrome_agents: u32,
    #[serde(default)]
    pub mean_sanity: f32,
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
    pub virt: VirtCols,
    /// Filled in by the runner (the library has no clock).
    pub ticks_per_sec: f32,
}

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
    pub ice_mean_corp: f32,
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
    pub corps: Vec<[u32; 4]>,
    pub flow_data: i64,
    pub flow_hack: i64,
    pub flow_ice_upkeep: i64,
    pub flow_research: i64,
    pub flow_terminal: i64,
}

impl VirtCols {
    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        let corps: Vec<String> = (0..CORP_SLOTS)
            .map(|i| {
                let [c, d, n, data] = self.corps.get(i).copied().unwrap_or_default();
                format!("{c},{d},{n},{data}")
            })
            .collect();
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.nodes,
            self.labs,
            self.decks,
            self.runs,
            self.runs_ok,
            self.data_made,
            self.data_stolen,
            self.data_wiped,
            self.data_sold,
            self.ledger_hacks,
            self.doors_hacked,
            self.traced,
            self.fried,
            self.flatlined,
            self.hack_arrests,
            self.ice_mean_corp,
            self.ice_spend,
            self.runs_bounced,
            self.runs_captured,
            self.runs_dumped,
            self.hack_arrests_chair,
            self.robots_turned,
            self.blinded,
            self.cameras,
            self.sightings,
            self.ice_raised,
            self.ice_lowered,
            self.tech_gained,
            self.tech_lost,
            self.research_spent,
            self.data_held,
            corps.join(","),
            self.flow_data,
            self.flow_hack,
            self.flow_ice_upkeep,
            self.flow_research,
            self.flow_terminal,
        )
    }
}

impl DayRow {
    pub fn new(day: u64) -> Self {
        DayRow {
            day,
            season: Season::of_day(day),
            population: 0,
            employed: 0,
            homeless: 0,
            jailed: 0,
            gang_members: 0,
            food_market: 0,
            food_warehouse: 0,
            food_pantry: 0,
            price: 0,
            treasury: 0,
            mean_hunger: 0.0,
            mean_mood: 0.0,
            thefts: 0,
            arrests: 0,
            deaths_starvation: 0,
            deaths_old_age: 0,
            deaths_violence: 0,
            births: 0,
            immigrants: 0,
            emigrants: 0,
            burials: 0,
            goal_changes: 0,
            goal_changes_per_agent: 0.0,
            holes_opened: 0,
            holes_bound: 0,
            holes_unknown: 0,
            deaths_violence_offscreen: 0,
            holes_open: 0,
            tier_full: 0,
            tier_coarse: 0,
            tier_stat: 0,
            evictions: 0,
            rent_paid: 0,
            rent_short: 0,
            housed: 0,
            flow_food: 0,
            flow_drink: 0,
            flow_wages: 0,
            flow_rent: 0,
            flow_upkeep: 0,
            flow_wholesale: 0,
            flow_overflow: 0,
            flow_restock: 0,
            flow_contract: 0,
            flow_tax: 0,
            flow_dole: 0,
            flow_other: 0,
            wallets: 0,
            wallet_gini: 0.0,
            wallet_top10: 0.0,
            corps: Vec::new(),
            acquisitions: 0,
            bankruptcies: 0,
            monopolies: 0,
            foundings: 0,
            incorporations: 0,
            strikes: 0,
            unrest_corp: 0.0,
            unrest_street: 0.0,
            unrest_dreg: 0.0,
            class_corp: 0,
            class_street: 0,
            class_dreg: 0,
            happiness_street: 0.0,
            districts: Vec::new(),
            hotel_nights: 0,
            squatters: 0,
            derelicts: 0,
            vagrancy: 0,
            riots: 0,
            crossfire: 0,
            gangs: 0,
            raids_into_cover: 0,
            vehicles_moto: 0,
            vehicles_car: 0,
            vehicles_truck: 0,
            vehicles_flyer: 0,
            truck_hauls: 0,
            walk_hauls: 0,
            commute_tpt_walk: 0.0,
            commute_tpt_drive: 0.0,
            chrome_installs: 0,
            chrome_agents: 0,
            mean_sanity: 0.0,
            episodes: 0,
            hooked: 0,
            stims_dealt: 0,
            stims_legal: 0,
            dealing_reports: 0,
            repos: 0,
            impounds: 0,
            crashes: 0,
            crash_deaths: 0,
            vehicle_thefts: 0,
            chops: 0,
            abductions: 0,
            stripped: 0,
            robots: 0,
            flow_asset: 0,
            flow_asset_upkeep: 0,
            flow_finance: 0,
            flow_import: 0,
            flow_stims: 0,
            flow_parts: 0,
            flow_treatment: 0,
            overdoses: 0,
            harvests: 0,
            stripped_window: 0,
            gang_income: 0,
            gang_income_dealing: 0,
            episodes_by_law: 0,
            treatments: 0,
            detoxes: 0,
            virt: VirtCols::default(),
            ticks_per_sec: 0.0,
        }
    }

    /// One CSV line matching [`CSV_HEADER`], without a trailing newline.
    pub fn csv_row(&self) -> String {
        let corps: Vec<String> = (0..CORP_SLOTS)
            .map(|i| match self.corps.get(i).copied().flatten() {
                Some((t, o)) => format!("{t},{o}"),
                None => "0,-".to_string(),
            })
            .collect();
        let districts: Vec<String> = (0..DISTRICT_SLOTS)
            .map(|i| {
                let (cov, ctrl, litter, unrest, crime, guards) = self.districts.get(i).copied().unwrap_or_default();
                format!("{cov:.3},{ctrl},{litter:.3},{unrest:.3},{crime:.3},{guards}")
            })
            .collect();
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.0}",
            self.day,
            self.season,
            self.population,
            self.employed,
            self.homeless,
            self.jailed,
            self.gang_members,
            self.food_market,
            self.food_warehouse,
            self.food_pantry,
            self.price,
            self.treasury,
            self.thefts,
            self.arrests,
            self.deaths_starvation,
            self.deaths_old_age,
            self.deaths_violence,
            self.births,
            self.immigrants,
            self.emigrants,
            self.burials,
            self.mean_hunger,
            self.mean_mood,
            self.goal_changes_per_agent,
            self.holes_opened,
            self.holes_open,
            self.holes_bound,
            self.holes_unknown,
            self.deaths_violence_offscreen,
            self.tier_full,
            self.tier_coarse,
            self.tier_stat,
            self.evictions,
            self.rent_paid,
            self.rent_short,
            self.housed,
            self.flow_food,
            self.flow_drink,
            self.flow_wages,
            self.flow_rent,
            self.flow_upkeep,
            self.flow_wholesale,
            self.flow_overflow,
            self.flow_restock,
            self.flow_contract,
            self.flow_tax,
            self.flow_dole,
            self.flow_other,
            self.wallets,
            self.wallet_gini,
            self.wallet_top10,
            corps.join(","),
            self.acquisitions,
            self.bankruptcies,
            self.monopolies,
            self.foundings,
            self.incorporations,
            self.strikes,
            self.unrest_corp,
            self.unrest_street,
            self.unrest_dreg,
            self.class_corp,
            self.class_street,
            self.class_dreg,
            self.happiness_street,
            districts.join(","),
            self.hotel_nights,
            self.squatters,
            self.derelicts,
            self.vagrancy,
            self.riots,
            self.crossfire,
            self.gangs,
            self.vehicles_moto,
            self.vehicles_car,
            self.vehicles_truck,
            self.vehicles_flyer,
            self.truck_hauls,
            self.walk_hauls,
            self.commute_tpt_walk,
            self.commute_tpt_drive,
            self.chrome_installs,
            self.chrome_agents,
            self.mean_sanity,
            self.episodes,
            self.hooked,
            self.stims_dealt,
            self.stims_legal,
            self.dealing_reports,
            self.repos,
            self.impounds,
            self.crashes,
            self.crash_deaths,
            self.vehicle_thefts,
            self.chops,
            self.abductions,
            self.stripped,
            self.robots,
            self.flow_asset,
            self.flow_asset_upkeep,
            self.flow_finance,
            self.flow_import,
            self.flow_stims,
            self.flow_parts,
            self.flow_treatment,
            self.overdoses,
            self.harvests,
            self.stripped_window,
            self.gang_income,
            self.gang_income_dealing,
            self.episodes_by_law,
            self.treatments,
            self.detoxes,
            self.virt.csv(),
            self.ticks_per_sec,
        )
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
