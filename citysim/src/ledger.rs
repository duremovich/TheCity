//! L2 (plan L24): the order-rates ledger's types. A cell counts, per
//! `(source, district, victim class)`, what the on-screen tier did and
//! suffered over the last `RATE_DAYS` days: victims and exposure (phase 4,
//! the victim side) and the members' acts and member-days (phase 3, the
//! actor side). The Statistical tier reads the cells as per-agent-day
//! rates. Everything here is counters between fictional agents of a
//! simulated city; the logic lives in `systems::fviolence`.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::components::{DistrictId, Order};
use crate::entity::EntityId;

/// Days a cell's windows hold (`[fviolence] rate_days` in phase 4).
pub const RATE_DAYS: usize = 14;

/// What drove an on-screen act of violence.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ViolenceSource {
    Order(Order),
    Vendetta,
    Riot,
    Episode,
}

/// The victim's class (`Watch`: public and private guards).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum VictimClass {
    Civilian,
    Member,
    Watch,
}

/// The actor side's acts (phase 3): an extortion, a claim blow, a deal shift.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ActKind {
    Extort = 0,
    Claim = 1,
    Deal = 2,
}

/// One cell: daily windows (oldest first, at most `RATE_DAYS`) and today's
/// running counts, pushed at midnight.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RateCell {
    /// Killed, Assaulted, Robbed, Abducted per day.
    pub victims: [VecDeque<u16>; 4],
    /// Body-hours of the class in the district under the source, per day.
    pub exposure: VecDeque<u32>,
    /// Extort, Claim, Deal per day (bodies only).
    pub acts: [VecDeque<u16>; 3],
    /// Members following the order who held a body that day (L23: per
    /// member-day, the spec's `member_hours`).
    pub member_days: VecDeque<u16>,
    pub victims_today: [u16; 4],
    pub exposure_today: u32,
    pub acts_today: [u16; 3],
    pub member_today: u16,
}

impl RateCell {
    /// Midnight: push today's counts into the windows, trim to `days`,
    /// reset today.
    pub fn roll(&mut self, days: usize) {
        fn push<T>(w: &mut VecDeque<T>, v: T, days: usize) {
            w.push_back(v);
            while w.len() > days {
                w.pop_front();
            }
        }
        for k in 0..4 {
            push(&mut self.victims[k], self.victims_today[k], days);
        }
        push(&mut self.exposure, self.exposure_today, days);
        for k in 0..3 {
            push(&mut self.acts[k], self.acts_today[k], days);
        }
        push(&mut self.member_days, self.member_today, days);
        self.victims_today = [0; 4];
        self.exposure_today = 0;
        self.acts_today = [0; 3];
        self.member_today = 0;
    }

    /// Every window and today's count is zero: the cell can go.
    pub fn is_empty(&self) -> bool {
        self.victims.iter().all(|w| w.iter().all(|&v| v == 0))
            && self.exposure.iter().all(|&v| v == 0)
            && self.acts.iter().all(|w| w.iter().all(|&v| v == 0))
            && self.member_days.iter().all(|&v| v == 0)
            && self.victims_today == [0; 4]
            && self.exposure_today == 0
            && self.acts_today == [0; 3]
            && self.member_today == 0
    }

    /// The window's acts of one kind.
    pub fn acts_sum(&self, kind: ActKind) -> u32 {
        self.acts[kind as usize].iter().map(|&v| u32::from(v)).sum()
    }

    /// The window's member-days.
    pub fn member_days_sum(&self) -> u32 {
        self.member_days.iter().map(|&v| u32::from(v)).sum()
    }
}

/// The cell key: source, district, victim class.
pub type CellKey = (ViolenceSource, DistrictId, VictimClass);

/// `World::order_rates`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OrderRates {
    pub cells: BTreeMap<CellKey, RateCell>,
}

impl OrderRates {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn cell_mut(&mut self, key: CellKey) -> &mut RateCell {
        self.cells.entry(key).or_default()
    }
}

/// A source touching a district today (phase 4 rebuilds the list at
/// midnight; never saved).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct ActiveSource {
    pub source: ViolenceSource,
    pub district: DistrictId,
    pub faction: Option<EntityId>,
    pub riot: Option<u32>,
    pub episode: Option<EntityId>,
}

/// L2 phase 4: the faction-violence tallies. `day_kills`: today's violent
/// deaths by the victim's tier at death (bodies, Statistical, bodies who
/// were civilians, Statistical civilians), read and reset by the CSV
/// snapshot. The rest accumulate over the run for
/// `test_faction_violence_parity`: civilian victims (Killed + Assaulted) of
/// on-screen faction violence and the civilian body-hours in touched
/// districts, against the daily pass's civilian Killed + Assaulted hits and
/// the Statistical civilian agent-days it rolled.
///
/// `day_sources`: the riots and episodes live at any hourly tally since the
/// last midnight (both end within hours, so the midnight pass would
/// otherwise never apply the rates their cells learn); read and cleared by
/// `fviolence::daily`. `riot_rosters`: each such riot's rioters and the
/// tick it was last live, so its holes bind among them after it ends;
/// pruned past `[bind] hole_ttl_days`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvTally {
    pub day_kills: [u32; 4],
    pub body_civ_victims: u64,
    pub body_civ_hours: u64,
    pub stat_civ_hits: u64,
    pub stat_civ_days: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub day_sources: Vec<ActiveSource>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub riot_rosters: BTreeMap<u32, (crate::time::Tick, Vec<EntityId>)>,
}

impl FvTally {
    pub fn is_zero(&self) -> bool {
        *self == FvTally::default()
    }
}
