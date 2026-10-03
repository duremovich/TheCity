//! Ticks, days, phases and seasons. One tick is one in-game minute.

use serde::{Deserialize, Serialize};
use std::fmt;

pub type Tick = u64;

pub const TICKS_PER_DAY: Tick = 1440;
pub const TICKS_PER_HOUR: Tick = 60;
pub const DAYS_PER_SEASON: u64 = 30;
pub const DAYS_PER_YEAR: u64 = 120;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum DayPhase {
    Night,
    Morning,
    Work,
    Evening,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn of_day(day: u64) -> Season {
        match (day % DAYS_PER_YEAR) / DAYS_PER_SEASON {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// Index into the per-season config arrays.
    pub fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for Season {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        })
    }
}

impl fmt::Display for DayPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DayPhase::Night => "Night",
            DayPhase::Morning => "Morning",
            DayPhase::Work => "Work",
            DayPhase::Evening => "Evening",
        })
    }
}

pub fn tick_of_day(tick: Tick) -> u16 {
    (tick % TICKS_PER_DAY) as u16
}

pub fn day(tick: Tick) -> u64 {
    tick / TICKS_PER_DAY
}

pub fn phase(tick: Tick) -> DayPhase {
    match tick_of_day(tick) {
        0..=359 => DayPhase::Night,
        360..=539 => DayPhase::Morning,
        540..=1079 => DayPhase::Work,
        _ => DayPhase::Evening,
    }
}

/// 21:00–06:00.
pub fn is_dark(tick: Tick) -> bool {
    let t = tick_of_day(tick);
    !(360..1260).contains(&t)
}

pub fn season(tick: Tick) -> Season {
    Season::of_day(day(tick))
}

/// `hh:mm` for the HUD and the event log.
pub fn clock(tick: Tick) -> String {
    let t = Tick::from(tick_of_day(tick));
    format!("{:02}:{:02}", t / TICKS_PER_HOUR, t % TICKS_PER_HOUR)
}
