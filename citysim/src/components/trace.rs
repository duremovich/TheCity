use super::*;

/// Bits of `DayTrace::flags`.
pub mod trace_flags {
    pub const ALIVE: u16 = 1;
    pub const JAILED: u16 = 2;
    pub const HOMELESS: u16 = 4;
    pub const EMPLOYED: u16 = 8;
    pub const GANG: u16 = 16;
    pub const STATISTICAL_ALL_DAY: u16 = 32;
    pub const SLEPT_AT_HOME: u16 = 64;
    pub const ATE: u16 = 128;
    /// M12 phase 3: slept in a booked Hotel bed.
    pub const HOTEL: u16 = 1 << 8;
    /// M12 phase 3: slept in a squat.
    pub const SQUAT: u16 = 1 << 9;
    /// Shadow V4 review 3: at Full at an hourly assignment that day (the
    /// binder never draws such an agent for the day's off-screen deeds).
    pub const FULL: u16 = 1 << 10;
}

/// One day of an adult's life, packed into a u32 for the save (M12 D4):
/// zone (3 bits) | flags 0-7 (8) << 3 | hunger band (2) << 11 | mood band (2) << 13
/// | district bits 0-3 (4) << 15 | has-district (1) << 19 | flags 8-9 (2) << 20 | district bit 4 (1) << 22 (Scale S2: a district under 16 packs as before) | flag 10 (1) << 23 (Shadow V4 review 3: FULL; an older save reads it clear).
/// A pre-M12 value has bit 19 clear and decodes `district = UNSET`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "u32", into = "u32")]
pub struct DayTrace {
    /// The zone the agent ended the day in.
    pub zone: Zone,
    /// The district the agent ended the day in.
    pub district: DistrictId,
    pub flags: u16,
    /// `0..=3` band at day end.
    pub hunger: u8,
    /// `0..=3` band at day end.
    pub mood: u8,
}

/// Bit 19 of a packed `DayTrace`: the district nibble is meaningful.
const TRACE_HAS_DISTRICT: u32 = 1 << 19;

impl DayTrace {
    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    /// Hunger `< 0.25 → 0, < 0.5 → 1, < 0.75 → 2, else 3`.
    pub fn hunger_band(h: f32) -> u8 {
        if h < 0.25 {
            0
        } else if h < 0.5 {
            1
        } else if h < 0.75 {
            2
        } else {
            3
        }
    }

    /// Mood (`-1..1`) `< -0.5 → 0, < 0 → 1, < 0.5 → 2, else 3`.
    pub fn mood_band(m: f32) -> u8 {
        if m < -0.5 {
            0
        } else if m < 0.0 {
            1
        } else if m < 0.5 {
            2
        } else {
            3
        }
    }
}

impl From<u32> for DayTrace {
    fn from(v: u32) -> DayTrace {
        let z = (v & 0b111) as usize;
        let district = if v & TRACE_HAS_DISTRICT != 0 {
            DistrictId((((v >> 15) & 0xf) | (((v >> 22) & 1) << 4)) as u8)
        } else {
            DistrictId::UNSET
        };
        DayTrace {
            zone: Zone::ALL[z.min(Zone::ALL.len() - 1)],
            district,
            flags: ((v >> 3) & 0xff) as u16 | (((v >> 20) & 0b11) as u16) << 8 | (((v >> 23) & 1) as u16) << 10,
            hunger: ((v >> 11) & 0b11) as u8,
            mood: ((v >> 13) & 0b11) as u8,
        }
    }
}

impl From<DayTrace> for u32 {
    fn from(t: DayTrace) -> u32 {
        let district = if t.district.is_unset() {
            0
        } else {
            u32::from(t.district.0 & 0xf) << 15 | u32::from((t.district.0 >> 4) & 1) << 22 | TRACE_HAS_DISTRICT
        };
        t.zone.index() as u32
            | u32::from(t.flags & 0xff) << 3
            | u32::from(t.hunger & 0b11) << 11
            | u32::from(t.mood & 0b11) << 13
            | district
            | u32::from((t.flags >> 8) & 0b11) << 20
            | u32::from((t.flags >> 10) & 1) << 23
    }
}

/// The last `[lod] trace_days` days of an adult, newest last. Survives death
/// (the binder and the biography read the past of the dead too).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    /// The day of the newest entry.
    pub last_day: u64,
    pub days: VecDeque<DayTrace>,
}

impl Trace {
    /// The entry for `day`, if recorded and not yet evicted.
    pub fn on_day(&self, day: u64) -> Option<DayTrace> {
        if self.days.is_empty() || day > self.last_day {
            return None;
        }
        let back = usize::try_from(self.last_day - day).ok()?;
        let n = self.days.len();
        if back >= n {
            return None;
        }
        self.days.get(n - 1 - back).copied()
    }

    /// Record `day`, one entry per day, the oldest evicted past `cap`.
    pub fn push(&mut self, day: u64, t: DayTrace, cap: usize) {
        if !self.days.is_empty() && day <= self.last_day {
            // Same day twice (a re-run of the daily pass): replace.
            if day == self.last_day {
                if let Some(last) = self.days.back_mut() {
                    *last = t;
                }
            }
            return;
        }
        // Keep indices contiguous: a skipped day repeats the previous entry.
        if let Some(&prev) = self.days.back() {
            let gap = (day - self.last_day - 1).min(cap as u64);
            for _ in 0..gap {
                self.days.push_back(prev);
            }
        }
        self.days.push_back(t);
        self.last_day = day;
        while self.days.len() > cap.max(1) {
            self.days.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// M11: classes (docs/M11_OWNERSHIP.md § 7)
// ---------------------------------------------------------------------------

/// Derived, never stored: Corp = employed by a corp-owned building or a
/// corp's exec; Dreg = no Home; Street = everyone else.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Class {
    Corp,
    Street,
    Dreg,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Corp, Class::Street, Class::Dreg];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Class::Corp => "Corp",
            Class::Street => "Street",
            Class::Dreg => "Dreg",
        }
    }
}

/// One class's daily aggregate (§ 7), recomputed at midnight after rent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClassAggregate {
    /// Adults with a Brain in the class.
    pub count: u32,
    /// Mean `(mood + 1) / 2`.
    pub happiness: f32,
    /// Employed adults / adults.
    pub employment: f32,
    /// Mean over members of yesterday's guard-hours near their Home ÷
    /// `fear_hours_full`, clamped to 1 (D34; a Dreg reads its zone's Homes).
    pub fear: f32,
    /// `happiness × min(employment + 0.5, 1)`.
    pub loyalty: f32,
    /// `0.3 + 0.7 × fear`.
    pub submission: f32,
    /// `(1 − loyalty) × (1 − submission) + 0.1 × evictions_7d ÷ count`.
    pub unrest: f32,
    pub evictions_7d: u32,
    /// The inputs, for the City panel; not saved.
    #[serde(skip)]
    pub trace: Vec<(&'static str, f32)>,
}
