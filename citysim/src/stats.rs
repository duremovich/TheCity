//! Daily statistics: counters that systems bump during the day and
//! snapshots taken at the end of it. `--report` prints one row per day.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::components::CorpOrder;
use crate::time::Season;

/// Days of history kept for the city panel's sparklines.
pub const STATS_HISTORY_CAP: usize = 120;

pub const CSV_HEADER: &str = "day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,holes_opened,holes_open,holes_bound,holes_unknown,deaths_violence_offscreen,tier_full,tier_coarse,tier_stat,evictions,rent_paid,rent_short,housed,flow_food,flow_drink,flow_wages,flow_rent,flow_upkeep,flow_wholesale,flow_overflow,flow_restock,flow_contract,flow_tax,flow_dole,flow_other,wallets,wallet_gini,wallet_top10,corp1_treasury,corp1_order,corp2_treasury,corp2_order,corp3_treasury,corp3_order,corp4_treasury,corp4_order,corp5_treasury,corp5_order,corp6_treasury,corp6_order,corp7_treasury,corp7_order,corp8_treasury,corp8_order,ticks_per_sec";

/// D38: corp CSV slots (seeding order).
pub const CORP_SLOTS: usize = 8;

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
    /// Filled in by the runner (the library has no clock).
    pub ticks_per_sec: f32,
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
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{},{},{},{},{},{:.0}",
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
