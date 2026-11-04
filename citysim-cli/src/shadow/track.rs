use super::*;

// ---------------------------------------------------------------------------
// Diary state
// ---------------------------------------------------------------------------

pub(super) struct Entry {
    pub(super) tick: Tick,
    pub(super) seq: u32,
    pub(super) cat: &'static str,
    pub(super) text: String,
}

pub(super) struct OpenAction {
    pub(super) key: (Tick, u8),
    pub(super) action: ActionKind,
    pub(super) target: Option<EntityId>,
    pub(super) start: Tick,
    pub(super) start_place: String,
    /// The pressure pass F1: the body's tile at the step's start (a GoTo's
    /// line gives the Manhattan tiles it moved).
    pub(super) start_tile: Option<citysim::components::TilePos>,
    /// The perf pass phase 1 (OPEN-195): the step's plan goal (an idle Rest
    /// is one outside a Work plan).
    pub(super) goal: Option<GoalKind>,
}

pub(super) struct Track {
    pub(super) id: EntityId,
    pub(super) arch: String,
    pub(super) name: String,
    pub(super) header: String,
    pub(super) entries: Vec<Entry>,
    seq: u32,
    pub(super) died: Option<Tick>,
    // diff state
    pub(super) last_goal: Option<GoalKind>,
    pub(super) plan_started: Option<Tick>,
    pub(super) open: Option<OpenAction>,
    pub(super) last_place: String,
    pub(super) last_building: Option<EntityId>,
    /// The city milestone C7: the last climb logged (its key) and the deck
    /// level the body last stood on in the street.
    pub(super) last_climb: Option<u64>,
    pub(super) last_deck: u8,
    /// V7: the entries of `World::crossing_watch` already logged.
    pub(super) cross_seen: usize,
    /// The view milestone V9 (OPEN-255): the El ride seen last (its plan
    /// key, the leg, aboard) and its alight stop's tile.
    pub(super) el_seen: Option<((Tick, u8), u8, bool, citysim::components::TilePos)>,
    pub(super) cooldowns: BTreeMap<GoalKind, Tick>,
    pub(super) wallet: Option<i64>,
    pub(super) inv: Option<(u32, u32, u16, u16)>,
    /// Hunger as sickness: the day's meal tally last seen (a meal logged on a rise).
    pub(super) tally: Option<citysim::MealTally>,
    pub(super) mem_seen: BTreeSet<(Tick, MemoryKind, Option<EntityId>, bool)>,
    pub(super) mem_primed: bool,
    pub(super) edges_primed: bool,
    pub(super) edges: BTreeMap<EntityId, (f32, RelKind)>,
    pub(super) sentence: Option<Tick>,
    pub(super) gang_order: Option<String>,
    pub(super) following: Option<String>,
    pub(super) grudges: BTreeSet<(EntityId, Tick)>,
    pub(super) hunt: Option<EntityId>,
    pub(super) rep: [f32; 4],
    pub(super) class: String,
    // tallies
    pub(super) cur_day: u64,
    pub(super) day_use: BTreeMap<&'static str, u32>,
    pub(super) day_places: BTreeMap<String, u32>,
    pub(super) day_net: i64,
    pub(super) total_use: BTreeMap<&'static str, u32>,
    pub(super) use_by_day: BTreeMap<u64, BTreeMap<&'static str, u32>>,
    pub(super) total_places: BTreeMap<String, u32>,
    pub(super) total_net: i64,
    pub(super) flows: BTreeMap<String, (i64, u32)>,
    pub(super) contacts: BTreeMap<EntityId, (String, u32)>,
    pub(super) event_kinds: BTreeMap<String, u32>,
    pub(super) notable: Vec<(Tick, String)>,
    pub(super) pinned_lod_noted: bool,
    pub(super) last_action: Option<ActionKind>,
    // V2: the L2 life
    pub(super) pinned: bool,
    pub(super) last_lod: Option<Lod>,
    pub(super) last_pick: Option<String>,
    pub(super) holes_seen: BTreeSet<u64>,
    pub(super) hang_others: BTreeSet<EntityId>,
    pub(super) hangouts: u32,
    pub(super) hang_known: BTreeSet<EntityId>,
    /// Flow name -> (coins out, coins in, count), from the sim's flow notes.
    pub(super) flow_kinds: BTreeMap<String, (i64, i64, u32)>,
    pub(super) tribute_in: i64,
    pub(super) gossip_about: u32,
    pub(super) stat_notes: Vec<(Tick, String)>,
    pub(super) stat_minutes: u32,
    pub(super) held_settles: u32,
    pub(super) held_minutes: u64,
    // The behaviour tier (`--assert`, docs/TESTING.md): the diary's numbers as metrics.
    pub(super) alive_min: u32,
    pub(super) work_min: u32,
    pub(super) sleep_run: u32,
    /// `(end tick, minutes)` of every finished block of sleep-class minutes.
    pub(super) sleep_blocks: Vec<(Tick, u32)>,
    pub(super) energy0_min: u32,
    /// Hunger as sickness: free minutes starving (hunger < `[life]
    /// starving_hunger`), and meals by kind (synth includes a Mission's).
    pub(super) starve_min: u32,
    pub(super) meals: u32,
    pub(super) meals_synth: u32,
    pub(super) meals_dive: u32,
    /// The job's shift under way, and the shifts seen whole.
    pub(super) shift: Option<ShiftObs>,
    pub(super) shifts: Vec<ShiftObs>,
    /// Ticks of every wage paid to the agent (`Flow::Wage`, `Flow::ExecWage`).
    pub(super) wages: Vec<Tick>,
    /// Coins handed back to the agent by `ownership::refund` (a purchase undone).
    pub(super) refunds: u32,
    /// The Work goal under way: a work-class minute reached in it.
    pub(super) work_plan: Option<bool>,
    /// Work goals dropped before a single work-class minute (a commute turned back).
    pub(super) commute_aborts: u32,
    /// Bouts naming the agent, and those with the agent inside the pit.
    pub(super) bouts: u32,
    pub(super) bouts_present: u32,
    // Lives at work phase 0 (`--lives`; the module header names the signals).
    /// The spouse at the window's start (`World::spouse_of`); a marriage or a
    /// death inside the window does not move it.
    pub(super) spouse: Option<EntityId>,
    /// Lives at work phase 7 (§ 5.3): the Home's tier at the last minute
    /// alive (`None` homeless).
    pub(super) home_tier: Option<u8>,
    /// Hours (tick / 60) in which a social move or a HangOut named the spouse.
    pub(super) spouse_named_hours: BTreeSet<Tick>,
    /// Hourly samples with the spouse alive; of those co-located and both awake;
    /// of those with the spouse Statistical (its position a demotion snapshot).
    pub(super) spouse_samples: u32,
    pub(super) spouse_coloc: u32,
    pub(super) spouse_coloc_stat: u32,
    /// HangOuts closed with nobody else at the tile.
    pub(super) hang_empty: u32,
    /// The perf pass phase 1 (OPEN-195): idle Rests cut within `SHORT_REST_MIN`.
    pub(super) short_rests: u32,
    /// `SERVED_SOURCE_KINDS` events seen at the workplace during a shift, by kind.
    pub(super) source_kinds: BTreeMap<String, u32>,
    /// Shadow V6: the camera lens.
    pub(super) cam: Cam,
    /// Round 3 S7 (OPEN-225): the last sighting logged per subject (the
    /// place and the day); an hourly refresh of the same one is not news.
    pub(super) sighted: BTreeMap<EntityId, (Option<EntityId>, u64)>,
    /// Round 3 S7: each diary day's rest-day note (`rest_day_note`).
    pub(super) rest_days: BTreeMap<u64, &'static str>,
    /// Round 3 S7: the hour's quiet lines (`flush_hourly`), and the hour.
    pub(super) hourly: Vec<Quiet>,
    pub(super) hourly_hour: Tick,
    /// Round 4 T7 (OPEN-233): a standing fact (a daily 00:07 refusal, a
    /// quiet headcount) told on its first day and on the change, by class.
    pub(super) standing: BTreeMap<&'static str, DayFold>,
    /// Round 4 T7: the day's folds (a chase given up, the door's refusals,
    /// the word about me), each into the day's first line, by key.
    pub(super) day_folds: BTreeMap<String, DayFold>,
    /// Round 4 T7: a goal's back-off streak folded into the day's first line.
    pub(super) cool_folds: BTreeMap<GoalKind, DayFold>,
    /// Round 4 T7: the day's encounters told (the counterpart, the line's
    /// shape): the same pair in the same role again that day is not news.
    pub(super) served_today: BTreeSet<(EntityId, String)>,
}

/// Round 4 T7 (OPEN-233): a line other lines fold into, in place: the
/// diary entry and the notable list, the count and what was tallied.
pub(super) struct DayFold {
    /// The fact's key (a standing class compares it; a day fold is keyed by it).
    pub(super) key: String,
    pub(super) day: u64,
    pub(super) entry: usize,
    pub(super) notable: Option<usize>,
    /// The first line as it was logged.
    pub(super) base: String,
    /// The days it was told again (a standing fact), or the lines folded in.
    pub(super) days: Vec<u64>,
    pub(super) count: u32,
    pub(super) names: Vec<String>,
    pub(super) tally: Vec<(String, u32)>,
    pub(super) last: String,
}

impl DayFold {
    pub(super) fn new(key: String, day: u64, entry: usize, notable: Option<usize>, base: String) -> Self {
        DayFold {
            key,
            day,
            entry,
            notable,
            base,
            days: Vec::new(),
            count: 1,
            names: Vec::new(),
            tally: Vec::new(),
            last: String::new(),
        }
    }

    pub(super) fn note(&mut self, what: &str) {
        match self.tally.iter_mut().find(|(k, _)| k == what) {
            Some((_, n)) => *n += 1,
            None => self.tally.push((what.to_string(), 1)),
        }
    }

    pub(super) fn name(&mut self, who: &str) {
        if !self.names.iter().any(|n| n == who) {
            self.names.push(who.to_string());
        }
    }

    pub(super) fn tally_text(&self) -> String {
        let parts: Vec<String> =
            self.tally.iter().map(|(k, n)| if *n > 1 { format!("{k} x{n}") } else { k.clone() }).collect();
        parts.join(", ")
    }

    pub(super) fn names_text(&self) -> String {
        let shown: Vec<&str> = self.names.iter().take(FOLD_NAMES).map(String::as_str).collect();
        let more = self.names.len().saturating_sub(shown.len());
        let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
        format!("{}{tail}", shown.join(", "))
    }
}

/// Round 4 T7: a minute's new encounter (the name, a regular, off screen, the line).
pub(super) type Met = (String, bool, bool, String);

/// Round 4 T7: `d22-d24, d26` for a sorted list of days.
pub(super) fn days_text(days: &[u64]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < days.len() {
        let mut j = i;
        while j + 1 < days.len() && days[j + 1] == days[j] + 1 {
            j += 1;
        }
        parts.push(if j > i { format!("d{}-d{}", days[i], days[j]) } else { format!("d{}", days[i]) });
        i = j + 1;
    }
    parts.join(", ")
}

/// Round 3 S7 (OPEN-225): a line that carries little alone (a new
/// Acquaintance, an Acquaintance's affinity drifting, a retelling of my deed
/// its hearer knew). A 100k cell wrote ~1,200 a day, one or two a minute,
/// past any per-minute fold; an hour of them folds by `key`.
pub(super) struct Quiet {
    pub(super) tick: Tick,
    pub(super) cat: &'static str,
    pub(super) key: &'static str,
    pub(super) name: String,
    pub(super) line: String,
}

/// One shift of the agent's job, as observed.
#[derive(Clone, Copy)]
pub(super) struct ShiftObs {
    pub(super) start: Tick,
    pub(super) end: Tick,
    pub(super) len: u32,
    pub(super) work: u32,
    /// A working day of the job (`routine::workday_of`), not a rest day.
    pub(super) workday: bool,
    /// Opened at the shift's first minute (not mid-shift at the window's start).
    pub(super) whole: bool,
    /// Never jailed during the shift.
    pub(super) free: bool,
    // Lives at work phase 0: who was on the other side of the counter.
    /// The job's building at the shift's first minute.
    pub(super) employer: Option<EntityId>,
    /// The agent owns the employer building: an owner-operator's post, which pays no wage by design
    /// (an owner pays the staff, not themselves), so the wage metrics skip it.
    pub(super) own_post: bool,
    /// Hourly samples during the shift, and the non-staff bodies inside the
    /// workplace summed over them.
    pub(super) samples: u32,
    pub(super) nonstaff: u32,
    /// `SERVED_SOURCE_KINDS` events at the workplace during the shift.
    pub(super) source_events: u32,
    /// The workplace's `Venue.visits_today` rise over the shift's hours
    /// (leisure venues only; a Statistical visit shows here and nowhere else).
    pub(super) venue_visits: u32,
    pub(super) last_visits: Option<u16>,
    /// Lives at work phase 1: `Served` memories the agent gained during the
    /// shift (the encounter itself, visible to neither body nor event).
    pub(super) encounters: u32,
    /// Lives at work phase 5: an exec's office hours (`[life] exec_shift`
    /// at his corp's `life::hq_of`), read by `--lives` as a shift (an exec
    /// holds no Job); left out of the behaviour tier's shift metrics.
    pub(super) office: bool,
}

/// Lives at work phase 0: two agents outdoors this close (Manhattan tiles)
/// at an hourly sample are co-located, as two inside one building are.
pub(super) const COLOC_TILES: u32 = 2;

/// Shadow V6: the camera's places, in print order (`cam_place`).
pub(super) const CAM_PLACES: [&str; 8] = ["home", "work", "building", "street", "vehicle", "held", "stat", "coarse"];
pub(super) const CAM_HOME: usize = 0;
pub(super) const CAM_WORK: usize = 1;
pub(super) const CAM_BUILDING: usize = 2;
pub(super) const CAM_STREET: usize = 3;
pub(super) const CAM_VEHICLE: usize = 4;
pub(super) const CAM_HELD: usize = 5;
pub(super) const CAM_STAT: usize = 6;
pub(super) const CAM_COARSE: usize = 7;
/// Shadow V6: tiles a position may change by beyond the travel allowance
/// before it is a jump (stepping out of a door, entering a building rect).
pub(super) const CAM_JUMP_SLACK: f32 = 2.0;

/// Shadow V6: one day of the camera block.
#[derive(Default, Clone)]
pub(super) struct CamDay {
    pub(super) place_min: [u32; 8],
    pub(super) plans: u32,
    pub(super) aborts: BTreeMap<String, u32>,
    pub(super) jumps: u32,
    /// Jumps with the pick Full and free at both ends (a body teleporting).
    pub(super) jumps_body: u32,
    pub(super) max_jump: u32,
    pub(super) first_jump: Option<Tick>,
    /// The longest unchanged stretch begun this day: minutes, start, class, place.
    pub(super) still: Option<(u32, Tick, &'static str, String)>,
    /// The same, among stretches that are not sleep.
    pub(super) still_awake: Option<(u32, Tick, &'static str, String)>,
    /// Flow notes and `Served` memories naming a person.
    pub(super) settles: u32,
    /// Of those, with the counterpart a body elsewhere.
    pub(super) settles_apart: u32,
    /// With the counterpart Statistical (off screen).
    pub(super) settles_stat: u32,
    /// With the pick asleep.
    pub(super) settles_asleep: u32,
    /// Shadow V7 (tool): the longest run of work-class minutes on one plan
    /// step begun this day (the shift's longest single work action).
    pub(super) work_run_max: u32,
    /// The distinct tiles the pick stood on at work-class minutes this day.
    pub(super) work_tiles: BTreeSet<citysim::TilePos>,
    /// GoTo steps begun this day that ran over `CAM_LONG_GOTO_MIN` minutes.
    pub(super) goto_long: u32,
}

/// Shadow V7: a GoTo step longer than this (minutes) is a long leg (PREVIEW_PASS § 3 phase 5's row).
pub(super) const CAM_LONG_GOTO_MIN: u64 = 120;

/// The perf pass phase 1 (OPEN-195): an idle Rest this short or shorter
/// (minutes) is a cut one (`--lives`' `short_rest/d`; the loop cut at 1).
pub(super) const SHORT_REST_MIN: u64 = 5;

/// The key of an unchanged stretch: tile, building, step, exec state.
pub(super) type StillKey = (citysim::TilePos, Option<EntityId>, Option<ActionKind>, u8);

/// Shadow V6: the camera's running state.
#[derive(Default)]
pub(super) struct Cam {
    pub(super) days: BTreeMap<u64, CamDay>,
    /// The position at the last tile change, its tick, and its place text.
    pub(super) anchor: Option<(citysim::TilePos, Tick, String)>,
    /// Tiles of travel allowance accrued since the anchor.
    pub(super) allow: f32,
    /// The allowance a tick in the current exec state accrues.
    pub(super) speed: f32,
    pub(super) prev_lod: Option<Lod>,
    pub(super) prev_held: bool,
    /// The unchanged stretch under way: its key, start, class and place.
    pub(super) still_key: Option<StillKey>,
    pub(super) still_start: Tick,
    pub(super) still_act: &'static str,
    pub(super) still_place: String,
    /// A courier's yard's pending parcels at the last tick (V5 bug 20).
    pub(super) parcels: Vec<citysim::systems::streetwork::Parcel>,
    pub(super) parcels_handed: u32,
    /// Shadow V7: the work-class run under way: its plan step key and start.
    pub(super) work_run: Option<((Tick, u8), Tick)>,
    /// Shadow V8: the vehicle in motion the pick rode at the last minute
    /// (the island pass's I3, `motion::vehicle_of`) and its tile then.
    pub(super) veh: Option<(EntityId, citysim::TilePos)>,
}

/// Lives at work phase 0: the event kinds that are a service encounter's
/// source today (docs/LIVES_AT_WORK.md § 1.2's list, as far as the city
/// writes an event for it: a sale, an install, a treatment, a stabilise, a
/// cure, a licence, a table or a bout, a Mission's day, a gift). No event
/// exists for a visit's completion, a tip, a cab fare or a detox.
pub(super) const SERVED_SOURCE_KINDS: &[EventKind] = &[
    EventKind::AssetBought,
    EventKind::Installed,
    EventKind::Treated,
    EventKind::Rescued,
    EventKind::Cured,
    EventKind::LicenceSold,
    EventKind::Gambled,
    EventKind::Bout,
    EventKind::MissionServed,
    EventKind::Donated,
    EventKind::TraumaSave,
];

impl Track {
    pub(super) fn new(id: EntityId, arch: &str, name: String, start: Tick) -> Track {
        Track {
            id,
            arch: arch.into(),
            name,
            header: String::new(),
            entries: Vec::new(),
            seq: 0,
            died: None,
            last_goal: None,
            plan_started: None,
            open: None,
            last_place: String::new(),
            last_building: None,
            last_climb: None,
            last_deck: 0,
            cross_seen: 0,
            el_seen: None,
            cooldowns: BTreeMap::new(),
            wallet: None,
            tally: None,
            inv: None,
            mem_seen: BTreeSet::new(),
            mem_primed: false,
            edges_primed: false,
            edges: BTreeMap::new(),
            sentence: None,
            gang_order: None,
            following: None,
            grudges: BTreeSet::new(),
            hunt: None,
            rep: [0.0; 4],
            class: String::new(),
            cur_day: start / TICKS_PER_DAY,
            day_use: BTreeMap::new(),
            day_places: BTreeMap::new(),
            day_net: 0,
            total_use: BTreeMap::new(),
            use_by_day: BTreeMap::new(),
            total_places: BTreeMap::new(),
            total_net: 0,
            flows: BTreeMap::new(),
            contacts: BTreeMap::new(),
            event_kinds: BTreeMap::new(),
            notable: Vec::new(),
            pinned_lod_noted: false,
            last_action: None,
            pinned: true,
            last_lod: None,
            last_pick: None,
            holes_seen: BTreeSet::new(),
            hang_others: BTreeSet::new(),
            hangouts: 0,
            hang_known: BTreeSet::new(),
            flow_kinds: BTreeMap::new(),
            tribute_in: 0,
            gossip_about: 0,
            stat_notes: Vec::new(),
            stat_minutes: 0,
            held_settles: 0,
            held_minutes: 0,
            alive_min: 0,
            work_min: 0,
            sleep_run: 0,
            sleep_blocks: Vec::new(),
            energy0_min: 0,
            starve_min: 0,
            meals: 0,
            meals_synth: 0,
            meals_dive: 0,
            shift: None,
            shifts: Vec::new(),
            wages: Vec::new(),
            refunds: 0,
            work_plan: None,
            commute_aborts: 0,
            bouts: 0,
            bouts_present: 0,
            spouse: None,
            home_tier: None,
            spouse_named_hours: BTreeSet::new(),
            spouse_samples: 0,
            spouse_coloc: 0,
            spouse_coloc_stat: 0,
            hang_empty: 0,
            short_rests: 0,
            source_kinds: BTreeMap::new(),
            cam: Cam::default(),
            sighted: BTreeMap::new(),
            rest_days: BTreeMap::new(),
            hourly: Vec::new(),
            hourly_hour: start / TICKS_PER_HOUR,
            standing: BTreeMap::new(),
            day_folds: BTreeMap::new(),
            cool_folds: BTreeMap::new(),
            served_today: BTreeSet::new(),
        }
    }

    pub(super) fn log(&mut self, tick: Tick, cat: &'static str, text: String) {
        self.seq += 1;
        self.entries.push(Entry { tick, seq: self.seq, cat, text });
    }

    pub(super) fn contact(&mut self, world: &World, other: EntityId) {
        if other == self.id || other == EntityId::NONE || !world.has::<Identity>(other) {
            return;
        }
        let e = self.contacts.entry(other).or_insert_with(|| (nm(world, other), 0));
        e.1 += 1;
    }
}

impl Track {
    /// Round 4 T7: rewrite a fold's first line in place (the diary entry and the notable list).
    pub(super) fn refold_line(&mut self, entry: usize, notable: Option<usize>, text: &str) {
        if let Some(e) = self.entries.get_mut(entry) {
            e.text = text.to_string();
        }
        if let Some(n) = notable.and_then(|i| self.notable.get_mut(i)) {
            n.1 = text.to_string();
        }
    }
}
