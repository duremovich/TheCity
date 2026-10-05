//! Daily statistics: counters that systems bump during the day and
//! snapshots taken at the end of it. `--report` prints one row per day.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::time::Season;

/// Days of history kept for the city panel's sparklines.
pub const STATS_HISTORY_CAP: usize = 120;

pub const CSV_HEADER: &str = "day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,holes_opened,holes_open,holes_bound,holes_unknown,deaths_violence_offscreen,tier_full,tier_coarse,tier_stat,ticks_per_sec";

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
            ticks_per_sec: 0.0,
        }
    }

    /// One CSV line matching [`CSV_HEADER`], without a trailing newline.
    pub fn csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{:.0}",
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
