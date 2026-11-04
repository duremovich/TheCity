use super::*;

// ---------------------------------------------------------------------------
// The behaviour tier (`shadow --assert`; docs/TESTING.md)
// ---------------------------------------------------------------------------

/// The behaviour tier's windows: 7 days from day 19 (a Friday, so
/// `worker_friday` starts on its Friday) with every archetype, and from day
/// 90 (the gangs full) with the gang archetypes: the V2 shadow pass's
/// pinned windows (docs/history/SHADOW_V2.md), on each of `BEHAVIOUR_SEEDS`.
///
/// Each archetype is measured in its own clone of the window's world, with
/// up to `BEHAVIOUR_SAMPLE` of its free candidates pinned (every one in a
/// small pool), and a row pools the three seeds: it reads the archetype,
/// not five people on one city, and another archetype's pins never move
/// it. The first `BEHAVIOUR_COUNT` of each sample are the diaries (`--out`),
/// the human-readable judge.
///
/// The diary seed (`--out` writes its diaries under `d<day>`, the others under `s<seed>_d<day>`).
pub const BEHAVIOUR_SEED: u64 = 42;
/// The seeds pooled into every row: a gang's trajectory or the Law's posture on one seed is one
/// sample of the city, not the archetype.
pub(super) const BEHAVIOUR_SEEDS: [u64; 3] = [BEHAVIOUR_SEED, 43, 44];
pub(super) const BEHAVIOUR_DAYS: u64 = 7;
const BEHAVIOUR_COUNT: usize = 5;
/// Agents measured per archetype and window: under `[lod] max_full` (50), so
/// every pinned agent holds a Full slot.
pub(super) const BEHAVIOUR_SAMPLE: usize = 30;
/// The share of agents dropped at each end before a per-agent metric is
/// averaged (a trimmed mean): one agent's 6 h commute or week at energy 0
/// does not carry a row; a regression shared by most of them does.
const BEHAVIOUR_TRIM: f64 = 0.1;
/// A share (paid of worked, waged workdays, bouts attended) is read over at
/// least this many shifts or bouts, else it is a SKIP: a share of five shifts
/// is one person's week.
const BEHAVIOUR_MIN_SHARE: u32 = 20;
/// A row is read over at least this many agents (the seeds pooled), else it
/// is a SKIP: never a smaller sample than the 5-pick tier judged.
pub(super) const BEHAVIOUR_MIN_AGENTS: usize = 5;
pub(super) const BEHAVIOUR_WINDOWS: [(u64, &str); 2] = [
    (
        19,
        "gang_member,gang_leader,ripperdoc,homeless,ceo,guard,worker,runner,purist,dealer,reporter,cook,club_staff,fighter,fabber,sweeper,worker_friday,super,bouncer,dancer,night_shift,orderly,camp_warden",
    ),
    (90, "gang_member,gang_leader,dealer,runner,purist,homeless"),
];

/// One agent's numbers in one window, or (merged) an archetype's (sums, then rates).
#[derive(Clone, Default)]
pub(super) struct Agg {
    pub(super) agents: u32,
    alive_min: u64,
    pub(super) travel_min: u64,
    sleep_min: u64,
    work_min: u64,
    energy0_min: u64,
    starve_min: u64,
    meals: u32,
    meals_synth: u32,
    meals_dive: u32,
    /// The longest sleep block of each agent-night (noon to noon), minutes.
    nights: Vec<u32>,
    /// Whole workday shifts while free; of those worked (half the shift at work-class
    /// steps); paid (a wage by two hours after the end); worked and paid.
    shifts: u32,
    worked: u32,
    paid: u32,
    worked_paid: u32,
    pub(super) hangouts: u32,
    known_met: u32,
    refunds: u32,
    commute_aborts: u32,
    bouts: u32,
    bouts_present: u32,
    // Lives at work phase 0 (`--lives`): read by `lives_report` only.
    pub(super) idle_min: u64,
    /// Married picks (a spouse at the window's start), their alive minutes,
    /// and those that never met the spouse by each signal.
    pub(super) married: u32,
    pub(super) married_min: u64,
    pub(super) never_named: u32,
    pub(super) never_coloc: u32,
    pub(super) spouse_named_hours: u32,
    spouse_samples: u32,
    pub(super) spouse_coloc: u32,
    pub(super) spouse_coloc_stat: u32,
    pub(super) hang_empty: u32,
    /// The perf pass phase 1 (OPEN-195): idle Rests cut within `SHORT_REST_MIN`.
    pub(super) short_rests: u32,
    /// Whole free workday shifts at a building; Served-able by a body or an
    /// event; by any of the three signals; the hourly samples and the
    /// non-staff bodies summed; the venue tally; the source events by kind.
    pub(super) served_shifts: u32,
    pub(super) servedable: u32,
    pub(super) servedable_any: u32,
    pub(super) shift_samples: u64,
    pub(super) nonstaff: u64,
    pub(super) venue_visits: u64,
    /// Phase 1: shifts with an encounter, and the encounters.
    pub(super) shifts_met: u32,
    pub(super) encounters: u64,
    pub(super) source_kinds: BTreeMap<String, u32>,
    /// Phase 7 (§ 5.3): picks housed at the window's end, and of them in the
    /// Spire (tier 2).
    pub(super) housed: u32,
    pub(super) spire: u32,
}

impl Agg {
    pub(super) fn add(&mut self, t: &Track) {
        self.add_lives(t);
        self.agents += 1;
        self.alive_min += u64::from(t.alive_min);
        self.travel_min += u64::from(t.total_use.get("travel").copied().unwrap_or(0));
        self.sleep_min += u64::from(t.total_use.get("sleep").copied().unwrap_or(0));
        self.work_min += u64::from(t.work_min);
        self.energy0_min += u64::from(t.energy0_min);
        self.starve_min += u64::from(t.starve_min);
        self.meals += t.meals;
        self.meals_synth += t.meals_synth;
        self.meals_dive += t.meals_dive;
        let mut by_night: BTreeMap<u64, u32> = BTreeMap::new();
        for &(end, len) in &t.sleep_blocks {
            let night = end.saturating_sub(TICKS_PER_DAY / 2) / TICKS_PER_DAY;
            let e = by_night.entry(night).or_insert(0);
            *e = (*e).max(len);
        }
        self.nights.extend(by_night.values());
        for s in t.shifts.iter().filter(|s| s.whole && s.workday && s.free && !s.office && !s.own_post) {
            let worked = s.work * 2 >= s.len;
            let paid = t.wages.iter().any(|&w| w >= s.start && w <= s.end + 2 * TICKS_PER_HOUR);
            self.shifts += 1;
            self.worked += u32::from(worked);
            self.paid += u32::from(paid);
            self.worked_paid += u32::from(worked && paid);
        }
        self.hangouts += t.hangouts;
        self.known_met += t.hang_known.len() as u32;
        self.refunds += t.refunds;
        self.commute_aborts += t.commute_aborts;
        self.bouts += t.bouts;
        self.bouts_present += t.bouts_present;
    }

    /// Lives at work phase 0: the `--lives` tallies of one agent.
    fn add_lives(&mut self, t: &Track) {
        self.idle_min += u64::from(t.total_use.get("idle").copied().unwrap_or(0));
        if t.spouse.is_some() {
            self.married += 1;
            self.married_min += u64::from(t.alive_min);
            self.never_named += u32::from(t.spouse_named_hours.is_empty());
            self.never_coloc += u32::from(t.spouse_coloc == 0);
            self.spouse_named_hours += t.spouse_named_hours.len() as u32;
            self.spouse_samples += t.spouse_samples;
            self.spouse_coloc += t.spouse_coloc;
            self.spouse_coloc_stat += t.spouse_coloc_stat;
        }
        self.hang_empty += t.hang_empty;
        self.short_rests += t.short_rests;
        self.housed += u32::from(t.home_tier.is_some());
        self.spire += u32::from(t.home_tier.is_some_and(|x| x >= 2));
        for s in t.shifts.iter().filter(|s| s.whole && s.workday && s.free && s.employer.is_some()) {
            let served = s.nonstaff > 0 || s.source_events > 0;
            self.served_shifts += 1;
            self.servedable += u32::from(served);
            self.servedable_any += u32::from(served || s.venue_visits > 0);
            self.shift_samples += u64::from(s.samples);
            self.nonstaff += u64::from(s.nonstaff);
            self.venue_visits += u64::from(s.venue_visits);
            self.shifts_met += u32::from(s.encounters > 0);
            self.encounters += u64::from(s.encounters);
        }
        for (k, n) in &t.source_kinds {
            *self.source_kinds.entry(k.clone()).or_insert(0) += n;
        }
    }

    /// Pool another agent's sums into these.
    fn merge(&mut self, o: &Agg) {
        self.idle_min += o.idle_min;
        self.housed += o.housed;
        self.spire += o.spire;
        self.married += o.married;
        self.married_min += o.married_min;
        self.never_named += o.never_named;
        self.never_coloc += o.never_coloc;
        self.spouse_named_hours += o.spouse_named_hours;
        self.spouse_samples += o.spouse_samples;
        self.spouse_coloc += o.spouse_coloc;
        self.spouse_coloc_stat += o.spouse_coloc_stat;
        self.hang_empty += o.hang_empty;
        self.short_rests += o.short_rests;
        self.served_shifts += o.served_shifts;
        self.servedable += o.servedable;
        self.servedable_any += o.servedable_any;
        self.shift_samples += o.shift_samples;
        self.nonstaff += o.nonstaff;
        self.venue_visits += o.venue_visits;
        self.shifts_met += o.shifts_met;
        self.encounters += o.encounters;
        for (k, n) in &o.source_kinds {
            *self.source_kinds.entry(k.clone()).or_insert(0) += n;
        }
        self.agents += o.agents;
        self.alive_min += o.alive_min;
        self.travel_min += o.travel_min;
        self.sleep_min += o.sleep_min;
        self.work_min += o.work_min;
        self.energy0_min += o.energy0_min;
        self.starve_min += o.starve_min;
        self.meals += o.meals;
        self.meals_synth += o.meals_synth;
        self.meals_dive += o.meals_dive;
        self.nights.extend_from_slice(&o.nights);
        self.shifts += o.shifts;
        self.worked += o.worked;
        self.paid += o.paid;
        self.worked_paid += o.worked_paid;
        self.hangouts += o.hangouts;
        self.known_met += o.known_met;
        self.refunds += o.refunds;
        self.commute_aborts += o.commute_aborts;
        self.bouts += o.bouts;
        self.bouts_present += o.bouts_present;
    }

    pub(super) fn agent_days(&self) -> f64 {
        self.alive_min as f64 / TICKS_PER_DAY as f64
    }

    /// The metric's value; `None` when the window gave it nothing to read
    /// (no shifts, no nights, no bouts).
    pub(super) fn get(&self, m: Metric) -> Option<f64> {
        let days = self.agent_days();
        if days <= 0.0 {
            return None;
        }
        let ratio = |a: u32, b: u32| (b > 0).then(|| f64::from(a) / f64::from(b));
        match m {
            Metric::WalkHDay => Some(self.travel_min as f64 / 60.0 / days),
            Metric::SleepHDay => Some(self.sleep_min as f64 / 60.0 / days),
            Metric::LongestSleepH => (!self.nights.is_empty())
                .then(|| self.nights.iter().map(|&n| f64::from(n)).sum::<f64>() / 60.0 / self.nights.len() as f64),
            Metric::Energy0HWeek => Some(self.energy0_min as f64 / 60.0 / days * 7.0),
            Metric::WorkHDay => Some(self.work_min as f64 / 60.0 / days),
            Metric::ShiftsWorkedShare => ratio(self.worked, self.shifts),
            Metric::PaidOfWorked => ratio(self.worked_paid, self.worked),
            Metric::WagedWorkdays => ratio(self.paid, self.shifts),
            Metric::HangoutsWeek => Some(f64::from(self.hangouts) / days * 7.0),
            Metric::KnownMetWeek => Some(f64::from(self.known_met) / days * 7.0),
            Metric::RefundsWeek => Some(f64::from(self.refunds) / days * 7.0),
            Metric::CommuteAbortsDay => Some(f64::from(self.commute_aborts) / days),
            Metric::BoutsAttended => ratio(self.bouts_present, self.bouts),
            Metric::StarveHWeek => Some(self.starve_min as f64 / 60.0 / days * 7.0),
            Metric::SynthShare => ratio(self.meals_synth, self.meals),
            Metric::DivesWeek => Some(f64::from(self.meals_dive) / days * 7.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Metric {
    /// Travel-class hours a day.
    WalkHDay,
    /// Sleep-class hours a day.
    SleepHDay,
    /// The mean over agent-nights (noon to noon) of the night's longest sleep block, hours.
    LongestSleepH,
    /// Hours at energy 0 per agent-week (free minutes: a pinned prisoner's cell is the pin's artefact).
    Energy0HWeek,
    /// Work-class hours a day.
    WorkHDay,
    /// Whole workday shifts (free) with half the shift at work-class steps.
    ShiftsWorkedShare,
    /// Worked shifts paid a wage by two hours after the shift's end.
    PaidOfWorked,
    /// Whole workday shifts (free) with a wage.
    WagedWorkdays,
    /// HangOut steps per agent-week.
    HangoutsWeek,
    /// Distinct known contacts met at a HangOut per agent-week.
    KnownMetWeek,
    /// Purchases refunded (a started buy undone) per agent-week.
    RefundsWeek,
    /// Work goals dropped before any work-class minute (a commute turned back), per agent-day.
    CommuteAbortsDay,
    /// Bouts with the fighter inside the pit, of the bouts naming her.
    BoutsAttended,
    /// Hunger as sickness (printed; no bound yet): free hours starving
    /// (hunger < `starving_hunger`) per agent-week.
    StarveHWeek,
    /// Synth (and Mission) meals of all meals, pooled (zero with `[sickness]` off).
    SynthShare,
    /// Bin meals per agent-week (zero with `[sickness]` off).
    DivesWeek,
}

impl Metric {
    /// A share of shifts or bouts: pooled over the sample (one agent's two
    /// shifts are no rate). Every other metric is a per-agent rate,
    /// trimmed-mean averaged.
    fn pooled(self) -> bool {
        matches!(
            self,
            Metric::ShiftsWorkedShare
                | Metric::PaidOfWorked
                | Metric::WagedWorkdays
                | Metric::BoutsAttended
                | Metric::SynthShare
        )
    }
}

/// One archetype in one window: each sampled agent's numbers, and their pool.
#[derive(Default)]
pub(super) struct Row {
    pub(super) agents: Vec<Agg>,
    pub(super) pooled: Agg,
}

impl Row {
    pub(super) fn add(&mut self, t: &Track) {
        let mut a = Agg::default();
        a.add(t);
        self.pooled.merge(&a);
        self.agents.push(a);
    }

    pub(super) fn extend(&mut self, o: &Row) {
        self.pooled.merge(&o.pooled);
        self.agents.extend(o.agents.iter().cloned());
    }

    /// The row's value of `m`: the pooled share, or the trimmed mean of the
    /// per-agent rates over the agents alive a whole day of the window.
    /// `judged`: `None` under `BEHAVIOUR_MIN_AGENTS` agents or (a share)
    /// `BEHAVIOUR_MIN_SHARE` shifts or bouts; the printed tables read every row.
    pub(super) fn get(&self, m: Metric, judged: bool) -> Option<f64> {
        let floor = |n: usize, min: usize| !judged || n >= min;
        if m.pooled() {
            let base = match m {
                Metric::PaidOfWorked => self.pooled.worked,
                Metric::BoutsAttended => self.pooled.bouts,
                _ => self.pooled.shifts,
            };
            let enough =
                floor(self.agents.len(), BEHAVIOUR_MIN_AGENTS) && floor(base as usize, BEHAVIOUR_MIN_SHARE as usize);
            return if enough { self.pooled.get(m) } else { None };
        }
        let v: Vec<f64> =
            self.agents.iter().filter(|a| a.alive_min >= TICKS_PER_DAY).filter_map(|a| a.get(m)).collect();
        if !floor(v.len(), BEHAVIOUR_MIN_AGENTS) {
            return None;
        }
        trimmed_mean(v, BEHAVIOUR_TRIM)
    }
}

/// The mean of `v` without its `floor(n x trim)` lowest and highest values.
fn trimmed_mean(mut v: Vec<f64>, trim: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let k = (v.len() as f64 * trim).floor() as usize;
    let kept = &v[k..v.len() - k];
    Some(kept.iter().sum::<f64>() / kept.len() as f64)
}

pub(super) const METRICS: [Metric; 16] = [
    Metric::WalkHDay,
    Metric::SleepHDay,
    Metric::LongestSleepH,
    Metric::Energy0HWeek,
    Metric::WorkHDay,
    Metric::ShiftsWorkedShare,
    Metric::PaidOfWorked,
    Metric::WagedWorkdays,
    Metric::HangoutsWeek,
    Metric::KnownMetWeek,
    Metric::RefundsWeek,
    Metric::CommuteAbortsDay,
    Metric::BoutsAttended,
    Metric::StarveHWeek,
    Metric::SynthShare,
    Metric::DivesWeek,
];

/// A bound: the value must be at least (`Min`) or at most (`Max`) it.
#[derive(Clone, Copy)]
pub(super) enum Bound {
    Min(f64),
    Max(f64),
}

/// One behaviour check: (window start day, archetype, metric, bound, and
/// the lowest and highest value the row read on the calibration ensemble (a printed FINDING band, see `COLLAPSE`):
/// main and seven config butterflies, the noise a city change that is not a
/// behaviour change makes). Bounds sit a cushion outside that range: they
/// catch a regression in "acts like a person", they are not targets.
type Check = (u64, &'static str, Metric, Bound, f64, f64);

/// Doctrine 1 (Dylan, 2026-10-22): the tier asserts broken-system signals, it does not police calibration.
/// A `CHECKS` row's own bound and range are the calibration band: printed as a `FINDING`, never failing.
/// Only a metric listed here asserts, against the bound here (where only a broken mechanism lands), and
/// only when the row's pooled n reaches `BEHAVIOUR_ASSERT_MIN_N`. One line of rationale per kind.
const COLLAPSE: &[(Metric, Bound)] = &[
    // Wages: under half of worked shifts (or workdays) paid is a dead pay path, not one broke owner.
    (Metric::PaidOfWorked, Bound::Min(0.5)),
    (Metric::WagedWorkdays, Bound::Min(0.5)),
    // Social: known contacts met at a HangOut near 0 a week is a dead social mechanism.
    (Metric::KnownMetWeek, Bound::Min(0.05)),
    // Travel: 20 h or more a day walking is a stuck commute or a lost plan, not a long day.
    (Metric::WalkHDay, Bound::Max(20.0)),
    // Body: energy at 0 for most of a 168 h week (> 84 h) is a person who never recovers.
    (Metric::Energy0HWeek, Bound::Max(84.0)),
    // Commute: more than one Work goal dropped before any work per agent-day is a broken planner (the CEO's
    // office-hours goal flickers at 0.5-0.9 by design, so the bound sits above it).
    (Metric::CommuteAbortsDay, Bound::Max(1.0)),
    // Core action: the reporter's work hours under half an hour a day, the role's job not happening.
    (Metric::WorkHDay, Bound::Min(0.5)),
    // Absolute, as before: a purchase refund is a bug (<= 1 a week); the fighter is in the pit for 3 bouts in 4.
    (Metric::RefundsWeek, Bound::Max(1.0)),
    (Metric::BoutsAttended, Bound::Min(0.75)),
];

/// A row whose pooled n (agents over the seeds) is under this prints and never asserts: one person's week.
const BEHAVIOUR_ASSERT_MIN_N: usize = 10;

pub(super) const CHECKS: &[Check] = &[
    // Re-calibrated 2026-10-11 on a6887f1 (after the housing pre-roll near jobs, grocery runs, workplaces capped
    // by room with night crews, vagrancy at work and the Hit market) by the same rules: main and seven
    // butterflies (`stat_violence_mult` 0.98, 0.99, 1.01, 1.02, 1.03, `max_coarse` 149, 151), the shipped
    // values being 1.0 and 150. d19 gang_leader PaidOfWorked/WagedWorkdays (read on 2 and 3 of 8 runs) and
    // d90 gang_leader PaidOfWorked (6 of 8) kept their bound and range; a row read on 7 of 8 was re-derived.
    // Re-derived 2026-10-10 after the flip (wages, no net, sickness, shanties the shipped default) by
    // the same rules: the tier on the new default and the same seven butterflies; the >30 % moves
    // are listed in the flip commit for review. d90 gang_leader PaidOfWorked read on 6 of 8 runs
    // (under 20 shifts on two): its bound and range kept.
    // Calibrated 2026-10-09 on d11e397: the tier run on main and on seven config butterflies
    // (`[lod] stat_violence_mult` 0.98, 0.99, 1.01, 1.02, 1.03, `max_coarse` 149, 151: the same rules,
    // a different week for every pick), the range each row read beside its bound. Cushions outside
    // the range: walking + max(1 h, 15 %), the longest nightly block - 0.5 h, energy 0 + 2 h a week,
    // known contacts met x 0.6, commute aborts + 0.05 a day, paid of worked and waged workdays - 0.1,
    // the reporter's work hours x 0.5; refunds <= 1 a week and the fighter inside the pit for 3 bouts
    // in 4 as before. A Min is rounded down, a Max up (to 0.01 for aborts and shares, else 0.1).
    // Not judged: d19 reporter PaidOfWorked (under 20 worked shifts on every run) and d90 runner
    // (3-10 agents over the three seeds, an archetype defined by a run in the last 7 days: its rows
    // moved more under the butterflies than any injected regression moved a row).
    // Not judged since Lives at work phase 4a (2026-10-18): d19 ripperdoc PaidOfWorked and WagedWorkdays.
    // The 18-21 picks share three clinic owners; the unpaid shifts are all at the two small clinics whose
    // owners run out of coin (Ripperdoc#515, #546), so the pooled share reads one owner's month. Phase 4a
    // read 0.73 / 0.69, its street switch off 0.89 / 0.85, `cab_shifts` off 0.86 / 0.83, the worlds
    // diverging from tick 588; unpaid-wage quits by Ripperdocs and Orderlies over 26 days, seeds 42-50:
    // 64 on, 55 off, 59 cab_shifts off (42-44 on is high, 45-50 on is the lowest). Workers going unpaid
    // by a broke owner is the harsh city, not a broken system (doctrine 1).
    (19, "ceo", Metric::WalkHDay, Bound::Max(5.5), 3.83, 4.42),
    (19, "ceo", Metric::LongestSleepH, Bound::Min(4.3), 4.81, 5.16),
    (19, "ceo", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    // Jobs & lives phase 7b (2026-10-15): this row alone re-derived on the phase-7 city with the
    // regenerated stat table (the eight-run ensemble: main 2.57; stat_violence_mult 0.98 1.32, 0.99
    // 1.42, 1.01 2.21, 1.02 2.50, 1.03 1.83; max_coarse 149 1.59, 151 2.05): 1.32 x 0.6 = 0.79,
    // rounded down. It read 0.77-2.00 across the milestone's phases against the old 1.3 (2.29-4.71).
    // Shadow V4 (2026-10-17, bug 5's known reason): the CEO holds office at its Hq every workday
    // (the office was the nearest Vat Farm or none, and the exec's day leaked into the street):
    // 0.55 pooled (0.12 / 0.89 / 1.22 by seed), the range widened by it, 0.55 x 0.6 rounded down.
    // Shadow V4 review 7 (2026-10-17, its known reason): no office in a corp's Home or Lot (bug 5's
    // unranked fallback had seated housing corps' execs among their tenants): 0.22 pooled (0.00 /
    // 0.89 / 0.12 by seed) against 0.59 with the fallback on the same tree, 0.22 x 0.6 rounded down.
    (19, "ceo", Metric::KnownMetWeek, Bound::Min(0.13), 0.22, 2.57),
    (19, "ceo", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    // An exec never does a work-class action, so every run of the office-hours
    // Work goal counts here: the row reads goal flicker (2026-10-11: 0.63 after
    // the own-shift off hours, more Idle -> Work switches, the same day). Not
    // the office gate's estimate: with it read against the deadline (review
    // item 6) the row reads 0.58, and 0.57 with that change alone undone;
    // 0.52-0.63 over the round's tables, so the 0.62 bound would flip on noise.
    // Shadow V4 (2026-10-17, bug 5's known reason): an office day at the Hq every workday, so
    // more office runs: 0.84 pooled (0.73 / 0.86 / 0.76), the range widened by it, + 0.05.
    (19, "ceo", Metric::CommuteAbortsDay, Bound::Max(0.89), 0.52, 0.84),
    (19, "club_staff", Metric::WalkHDay, Bound::Max(5.6), 3.95, 4.53),
    (19, "club_staff", Metric::LongestSleepH, Bound::Min(4.0), 4.54, 4.76),
    (19, "club_staff", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "club_staff", Metric::KnownMetWeek, Bound::Min(3.2), 5.42, 7.03),
    (19, "club_staff", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "club_staff", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "club_staff", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "club_staff", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 1.00),
    (19, "cook", Metric::WalkHDay, Bound::Max(5.1), 3.71, 4.02),
    (19, "cook", Metric::LongestSleepH, Bound::Min(4.3), 4.81, 4.89),
    (19, "cook", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "cook", Metric::KnownMetWeek, Bound::Min(3.4), 5.83, 6.88),
    (19, "cook", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "cook", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "cook", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "cook", Metric::WagedWorkdays, Bound::Min(0.89), 0.99, 1.00),
    (19, "fabber", Metric::WalkHDay, Bound::Max(5.8), 4.45, 4.79),
    (19, "fabber", Metric::LongestSleepH, Bound::Min(4.2), 4.75, 4.88),
    (19, "fabber", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "fabber", Metric::KnownMetWeek, Bound::Min(0.9), 1.62, 2.88),
    (19, "fabber", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "fabber", Metric::CommuteAbortsDay, Bound::Max(0.07), 0.00, 0.02),
    (19, "fabber", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "fabber", Metric::WagedWorkdays, Bound::Min(0.83), 0.93, 0.95),
    (19, "fighter", Metric::WalkHDay, Bound::Max(6.9), 4.77, 5.84),
    (19, "fighter", Metric::LongestSleepH, Bound::Min(4.5), 5.02, 5.34),
    (19, "fighter", Metric::Energy0HWeek, Bound::Max(2.2), 0.01, 0.12),
    // Lives at work gate (2026-10-19): this row re-derived on the gate city (the eight-run ensemble:
    // shipped 2.55; stat_violence_mult 0.98 2.95, 0.99 2.60, 1.01 3.05, 1.02 3.05, 1.03 3.00; max_coarse
    // 149 3.00, 151 2.70): 2.55 x 0.6 = 1.53, rounded down. Cause: phase 2's own-Home HangOut moved free
    // time indoors with flatmates (5.55 -> 3.46 at phase 2) and the 8a regen echoes it (OPEN-103, closed
    // by the street-life feature row).
    (19, "fighter", Metric::KnownMetWeek, Bound::Min(1.5), 2.55, 3.05),
    (19, "fighter", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "fighter", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    (19, "fighter", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "fighter", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 0.99),
    (19, "fighter", Metric::BoutsAttended, Bound::Min(0.75), 1.00, 1.00),
    (19, "gang_leader", Metric::WalkHDay, Bound::Max(10.1), 4.43, 8.72),
    (19, "gang_leader", Metric::LongestSleepH, Bound::Min(4.0), 4.59, 5.61),
    (19, "gang_leader", Metric::Energy0HWeek, Bound::Max(2.8), 0.00, 0.77),
    (19, "gang_leader", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "gang_leader", Metric::CommuteAbortsDay, Bound::Max(0.26), 0.00, 0.21),
    (19, "gang_leader", Metric::PaidOfWorked, Bound::Min(0.85), 0.95, 1.00),
    (19, "gang_leader", Metric::WagedWorkdays, Bound::Min(0.64), 0.74, 1.00),
    (19, "gang_member", Metric::WalkHDay, Bound::Max(7.1), 5.55, 6.04),
    (19, "gang_member", Metric::LongestSleepH, Bound::Min(4.4), 4.90, 5.35),
    (19, "gang_member", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.03),
    // OPEN-111 (2026-10-19): this row re-derived on the fix tree (the eight-run ensemble: shipped 5.35, pick 58;
    // stat_violence_mult 0.98 4.23 (57), 0.99 4.23 (57), 1.01 7.07 (71), 1.02 7.07 (71), 1.03 6.64 (75);
    // max_coarse 149 7.34 (80), 151 6.45 (85)): 4.23 x 0.6 = 2.54, rounded down. Cause: the venues' evening staff
    // and the stable server move who is free on d19 (pick sizes 57-85, seed 42's 22 -> 9), a thin pick moved by
    // the milestone's design (OPEN-109; Dylan's 2026-10-19 rule: re-derive social-contact rows the milestone moved).
    (19, "gang_member", Metric::KnownMetWeek, Bound::Min(2.5), 4.23, 7.34),
    (19, "gang_member", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "gang_member", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "guard", Metric::WalkHDay, Bound::Max(11.4), 9.43, 9.89),
    (19, "guard", Metric::LongestSleepH, Bound::Min(4.6), 5.15, 5.25),
    (19, "guard", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "guard", Metric::KnownMetWeek, Bound::Min(2.8), 4.78, 6.94),
    (19, "guard", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "guard", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "guard", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "guard", Metric::WagedWorkdays, Bound::Min(0.86), 0.96, 0.98),
    (19, "homeless", Metric::WalkHDay, Bound::Max(6.6), 4.97, 5.53),
    (19, "homeless", Metric::LongestSleepH, Bound::Min(5.1), 5.60, 5.82),
    (19, "homeless", Metric::Energy0HWeek, Bound::Max(2.3), 0.03, 0.23),
    // Lives at work phase 5 (2026-10-18): this row alone re-derived on the phase-5 city (the eight-run
    // ensemble: shipped 5.16; stat_violence_mult 0.98 6.00, 0.99 6.00, 1.01 5.61, 1.02 6.44, 1.03 5.45;
    // max_coarse 149 7.47, 151 7.24): 5.16 x 0.6 = 3.10, rounded down. The row drifted across phases 1-5
    // from 10.8-12.5 to about 7 on main (7.00) and 5.2 here: drift across the milestone's
    // phases, not a single change (no other row moved in any of the eight runs bar max_coarse 149's three).
    (19, "homeless", Metric::KnownMetWeek, Bound::Min(3.0), 5.16, 7.47),
    (19, "homeless", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "homeless", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    // M17 phase 4b: restored to the pre-phase-3 bounds. Phase 3 loosened them to 0.54 / 0.51 for the
    // brothers' larger gangs (the shipped city read 0.64 / 0.61, 0.91 / 0.92 with the brothers off);
    // with the brothers a rare sack-gutting event the shipped city reads 0.84 / 0.80.
    (19, "homeless", Metric::PaidOfWorked, Bound::Min(0.7), 0.80, 0.94),
    // Shadow V4 (2026-10-17, the regenerated stat table): 0.59 pooled (0.44 / 0.75 / 0.62), the
    // off-screen run to day 19 housing more of the poor (11 homeless picks on seed 42, 16 before)
    // and one of the few left a new Bartender at a Bar that paid him nothing (city wages paid on
    // 42-47 x 120 rose 1-7 % over ce5afe6): the range widened by it, - 0.1.
    (19, "homeless", Metric::WagedWorkdays, Bound::Min(0.49), 0.59, 0.88),
    (19, "reporter", Metric::WalkHDay, Bound::Max(5.9), 4.03, 4.85),
    (19, "reporter", Metric::LongestSleepH, Bound::Min(4.4), 4.90, 5.27),
    (19, "reporter", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.01),
    (19, "reporter", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "reporter", Metric::CommuteAbortsDay, Bound::Max(0.19), 0.01, 0.14),
    (19, "reporter", Metric::WagedWorkdays, Bound::Min(0.64), 0.74, 0.83),
    (19, "reporter", Metric::WorkHDay, Bound::Min(2.8), 5.79, 6.17),
    (19, "ripperdoc", Metric::WalkHDay, Bound::Max(5.5), 3.72, 4.44),
    (19, "ripperdoc", Metric::LongestSleepH, Bound::Min(4.0), 4.52, 4.94),
    (19, "ripperdoc", Metric::Energy0HWeek, Bound::Max(2.2), 0.00, 0.11),
    // M17 phase 2 (one-row re-calibration, TESTING.md): the parents moved the city's trajectory and the
    // ten-agent ripperdoc row read 2.78 known met and 0.05 aborts on 42-44 (2.90 / 0.00 on c6d6ebe;
    // 5.23 / 0.03 and 4.30 / 0.01 under the 1.01 and 0.99 stat_violence_mult butterflies with the parents
    // on): the ranges widened by the new reading, the cushions as the rule.
    (19, "ripperdoc", Metric::KnownMetWeek, Bound::Min(1.6), 2.78, 9.47),
    (19, "ripperdoc", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "ripperdoc", Metric::CommuteAbortsDay, Bound::Max(0.10), 0.00, 0.05),
    // Not judged since the pre-view pass phase 5 (2026-10-21): d19 sweeper WalkHDay (a printed value, no bound).
    // `[legs] sweep_by_ride` (OPEN-156) reads the way back to the Recycler by the ride, so a shift begun far
    // from it is no longer the old stand on the beat (`sweep_stands`) but a commute to the Recycler (the round
    // must end there, P8): 7.60 h/day with it, 6.23 with it off, 6.06 with `[legs]` off. The extra
    // travel minutes are the in-shift ones with no round route (the commute to the post, ridden or walked:
    // 44k -> 93k of +54k over the tier's sweepers; the walk to the first stop +4.6k, the way back +6.7k, the stops +1.2k). Moved for a
    // known reason, the bullet demoted, the bound not widened (CLAUDE.md doctrine 2).
    (19, "sweeper", Metric::LongestSleepH, Bound::Min(3.5), 4.00, 4.08),
    (19, "sweeper", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "sweeper", Metric::KnownMetWeek, Bound::Min(2.2), 3.75, 4.67),
    (19, "sweeper", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "sweeper", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    (19, "sweeper", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "sweeper", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 1.00),
    (19, "worker", Metric::WalkHDay, Bound::Max(6.9), 5.37, 5.83),
    (19, "worker", Metric::LongestSleepH, Bound::Min(4.4), 4.97, 5.18),
    (19, "worker", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "worker", Metric::KnownMetWeek, Bound::Min(1.0), 1.74, 4.93),
    (19, "worker", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "worker", Metric::CommuteAbortsDay, Bound::Max(0.08), 0.00, 0.03),
    (19, "worker", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "worker", Metric::WagedWorkdays, Bound::Min(0.82), 0.92, 0.96),
    (19, "worker_friday", Metric::WalkHDay, Bound::Max(6.3), 4.42, 5.24),
    (19, "worker_friday", Metric::LongestSleepH, Bound::Min(3.5), 4.09, 4.39),
    (19, "worker_friday", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "worker_friday", Metric::KnownMetWeek, Bound::Min(2.6), 4.47, 5.55),
    (19, "worker_friday", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "worker_friday", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "worker_friday", Metric::PaidOfWorked, Bound::Min(0.86), 0.96, 0.99),
    (19, "worker_friday", Metric::WagedWorkdays, Bound::Min(0.85), 0.95, 0.97),
    (90, "gang_leader", Metric::WalkHDay, Bound::Max(9.9), 6.10, 8.53),
    (90, "gang_leader", Metric::LongestSleepH, Bound::Min(4.4), 4.99, 6.08),
    (90, "gang_leader", Metric::Energy0HWeek, Bound::Max(3.1), 0.12, 1.05),
    (90, "gang_leader", Metric::KnownMetWeek, Bound::Min(0.1), 0.25, 6.38),
    (90, "gang_leader", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "gang_leader", Metric::CommuteAbortsDay, Bound::Max(0.18), 0.00, 0.13),
    // Jobs & lives phase-8 review (2026-10-17): this row alone re-derived on the review's city with
    // the eight-run ensemble (main 0.77; stat_violence_mult 0.98 1.00, 0.99 1.00, 1.01 0.74, 1.02
    // 0.85, 1.03 SKIP (16 shifts); max_coarse 149 0.96, 151 0.97; read on 7 of 8): 0.74 - 0.1 =
    // 0.64. The misses were leaders on side jobs at an owner short of coins (unpaid, then a quit).
    (90, "gang_leader", Metric::PaidOfWorked, Bound::Min(0.64), 0.74, 1.00),
    (90, "gang_leader", Metric::WagedWorkdays, Bound::Min(0.59), 0.69, 0.97),
    (90, "gang_member", Metric::WalkHDay, Bound::Max(7.8), 5.35, 6.70),
    (90, "gang_member", Metric::LongestSleepH, Bound::Min(5.2), 5.78, 6.09),
    (90, "gang_member", Metric::Energy0HWeek, Bound::Max(2.7), 0.19, 0.70),
    (90, "gang_member", Metric::KnownMetWeek, Bound::Min(2.3), 3.94, 5.91),
    (90, "gang_member", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "gang_member", Metric::CommuteAbortsDay, Bound::Max(0.07), 0.00, 0.02),
    (90, "homeless", Metric::WalkHDay, Bound::Max(6.1), 4.37, 5.01),
    (90, "homeless", Metric::LongestSleepH, Bound::Min(5.6), 6.15, 6.52),
    (90, "homeless", Metric::Energy0HWeek, Bound::Max(3.8), 1.23, 1.79),
    // Lives at work gate (2026-10-19): this row re-derived on the gate city (the eight-run ensemble:
    // shipped 3.73; stat_violence_mult 0.98 3.74, 0.99 4.48, 1.01 4.84, 1.02 4.16, 1.03 2.91; max_coarse
    // 149 4.13, 151 5.06): 2.91 x 0.6 = 1.75, rounded down. Cause: phase 2's own-Home HangOut and the 8a
    // regen (OPEN-103, closed by the street-life feature row).
    (90, "homeless", Metric::KnownMetWeek, Bound::Min(1.7), 2.91, 5.06),
    (90, "homeless", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    // Lives at work gate 8d (2026-10-19): re-derived on the gate city (the eight-run ensemble on the 8d
    // fix round's stat table: shipped 0.07; stat_violence_mult 0.98 0.04, 0.99 0.04, 1.01 0.04, 1.02
    // 0.03, 1.03 0.04; max_coarse 149 0.08, 151 0.04): 0.08 + 0.05 = 0.13. The milestone moved it off the
    // old 0.00-0.02 before the regen (8c main 0.06, the 8d tree 0.04 on the old table); not the deposit
    // (`move_in_rent_days` 0 reads 0.09). The highs are one seed's commuters (seed 44: 0.24, 0.20 of
    // 16-19 picks; the seeds read 0.00-0.24 over the ensemble).
    (90, "homeless", Metric::CommuteAbortsDay, Bound::Max(0.13), 0.03, 0.08),
    (90, "homeless", Metric::PaidOfWorked, Bound::Min(0.68), 0.78, 0.92),
    (90, "homeless", Metric::WagedWorkdays, Bound::Min(0.43), 0.53, 0.69),
    (90, "purist", Metric::WalkHDay, Bound::Max(7.1), 4.39, 6.08),
    (90, "purist", Metric::LongestSleepH, Bound::Min(5.0), 5.51, 6.22),
    (90, "purist", Metric::Energy0HWeek, Bound::Max(2.6), 0.01, 0.56),
    (90, "purist", Metric::KnownMetWeek, Bound::Min(1.0), 1.73, 10.06),
    (90, "purist", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "purist", Metric::CommuteAbortsDay, Bound::Max(0.07), 0.00, 0.02),
];

/// One archetype's measured sample in one window of one seed (`None`: no free candidate).
type SampleRun = Result<(u64, u64, String, Option<Row>), String>;

/// What `behaviour` found: the printed table and the failed checks.
pub struct BehaviourReport {
    pub text: String,
    pub failures: Vec<String>,
}

/// The behaviour tier's sample of `arch`: its free candidates in the
/// shadow's pick order (shuffled by the seed; the CEOs by treasury), at most
/// `BEHAVIOUR_SAMPLE` of them.
pub(super) fn sample(world: &World, seed: u64, arch: &str) -> Result<Vec<EntityId>, String> {
    let c = candidates(world, arch)?;
    let ordered = if arch == "ceo" { c } else { shuffled(c, seed, arch) };
    Ok(ordered.into_iter().filter(|&id| is_free(world, id)).take(BEHAVIOUR_SAMPLE).collect())
}

/// One window of one seed: the world run to `day` once, then each
/// archetype's sample followed in its own clone of it (a thread each); the
/// first `BEHAVIOUR_COUNT` of a sample write their diaries under
/// `out/d<day>` (seed 42) or `out/s<seed>_d<day>`.
fn behaviour_window(seed: u64, day: u64, list: &'static str, out: Option<&std::path::Path>) -> Vec<SampleRun> {
    let archs: Vec<&str> = list.split(',').collect();
    window_runs(seed, day, &archs, out)
}

/// `behaviour_window`'s body over an archetype list (shared with `--lives`).
pub(super) fn window_runs(seed: u64, day: u64, archs: &[&str], out: Option<&std::path::Path>) -> Vec<SampleRun> {
    let world = start_world(seed, day, None, None).expect("the shipped map");
    let jobs: Vec<(&str, Result<Vec<EntityId>, String>)> =
        archs.iter().map(|&arch| (arch, sample(&world, seed, arch))).collect();
    std::thread::scope(|s| {
        let hs: Vec<_> = jobs
            .into_iter()
            .map(|(arch, ids)| {
                let mut w = world.clone();
                s.spawn(move || -> SampleRun {
                    let ids = ids?;
                    if ids.is_empty() {
                        return Ok((seed, day, arch.to_string(), None));
                    }
                    let picked: Vec<(String, EntityId)> = ids.iter().map(|&id| (arch.to_string(), id)).collect();
                    let mut tracks = follow(&mut w, &picked, BEHAVIOUR_DAYS, true, false);
                    if let Some(dir) = out {
                        let sub = if seed == BEHAVIOUR_SEED { format!("d{day}") } else { format!("s{seed}_d{day}") };
                        let dir = dir.join(sub);
                        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                        for t in tracks.iter_mut().take(BEHAVIOUR_COUNT) {
                            write_diary(&w, t, &dir, w.tick)?;
                        }
                    }
                    let mut row = Row::default();
                    for t in &tracks {
                        row.add(t);
                    }
                    Ok((seed, day, arch.to_string(), Some(row)))
                })
            })
            .collect();
        hs.into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err(format!("a seed {seed} d{day} behaviour sample panicked"))))
            .collect()
    })
}

/// Run the behaviour tier's windows (each archetype's sample in its own
/// thread), write the diaries under `out` if given, and judge `CHECKS`.
pub fn behaviour(out: Option<&std::path::Path>) -> Result<BehaviourReport, String> {
    let seeds = BEHAVIOUR_SEEDS;
    let runs: Vec<Vec<SampleRun>> = std::thread::scope(|s| {
        let hs: Vec<_> = seeds
            .iter()
            .flat_map(|&seed| BEHAVIOUR_WINDOWS.iter().map(move |&(day, list)| (seed, day, list)))
            .map(|(seed, day, list)| s.spawn(move || behaviour_window(seed, day, list, out)))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| vec![Err("a behaviour window panicked".into())])).collect()
    });
    let mut aggs: BTreeMap<(u64, String), Row> = BTreeMap::new();
    let mut per_seed: BTreeMap<(u64, String, u64), Row> = BTreeMap::new();
    for r in runs.into_iter().flatten() {
        if let (seed, day, arch, Some(row)) = r? {
            aggs.entry((day, arch.clone())).or_default().extend(&row);
            per_seed.insert((day, arch, seed), row);
        }
    }
    let mut text = String::new();
    let _ = writeln!(
        text,
        "behaviour tier: seeds {seeds:?} pooled, {BEHAVIOUR_DAYS} days, up to {BEHAVIOUR_SAMPLE} free agents per archetype \
         and seed (each archetype in its own clone, pinned at Full); shares pooled (>= {BEHAVIOUR_MIN_SHARE} shifts or \
         bouts), rates a {:.0} % trimmed mean",
        BEHAVIOUR_TRIM * 100.0
    );
    let _ = write!(text, "{:<4} {:<14} {:>2}", "day", "archetype", "n");
    for m in METRICS {
        let _ = write!(text, " {:>9}", short(m));
    }
    let _ = writeln!(text);
    for ((day, arch), a) in &aggs {
        row_line(&mut text, *day, arch, a);
    }
    // Which seed carried a row: read before the diaries of a failure.
    let _ = writeln!(text, "per seed (archetype@seed):");
    for ((day, arch, seed), a) in &per_seed {
        row_line(&mut text, *day, &format!("{arch}@{seed}"), a);
    }
    let (mut failures, mut asserted, mut findings, mut missed) = (Vec::new(), 0usize, 0usize, 0usize);
    for &(day, arch, m, bound, lo, hi) in CHECKS {
        let Some(a) = aggs.get(&(day, arch.to_string())) else {
            let _ = writeln!(text, "SKIP d{day} {arch} {m:?}: no candidate in the window");
            continue;
        };
        let Some(v) = a.get(m, true) else {
            let _ = writeln!(
                text,
                "SKIP d{day} {arch} {m:?}: too little to read ({} agents, {} shifts, {} bouts; the floors are \
                 {BEHAVIOUR_MIN_AGENTS} agents, {BEHAVIOUR_MIN_SHARE} shifts or bouts for a share)",
                a.agents.len(),
                a.pooled.shifts,
                a.pooled.bouts
            );
            continue;
        };
        let holds = |bd: Bound, v: f64| match bd {
            Bound::Min(b) => (v >= b, format!(">= {b}")),
            Bound::Max(b) => (v <= b, format!("<= {b}")),
        };
        let n = a.agents.len();
        let (cal_ok, cal_rule) = holds(bound, v);
        let cal = format!("calibration {cal_rule} {} (range {lo:.2}-{hi:.2})", if cal_ok { "ok" } else { "MISS" });
        let collapse = COLLAPSE.iter().find(|(cm, _)| *cm == m).map(|&(_, cb)| cb);
        match collapse {
            Some(cb) if n >= BEHAVIOUR_ASSERT_MIN_N => {
                let (ok, rule) = holds(cb, v);
                let line = format!("d{day} {arch} {m:?} {v:.2} n {n} collapse {rule}; {cal}");
                let _ = writeln!(text, "{} {line}", if ok { "PASS" } else { "FAIL" });
                asserted += 1;
                if !ok {
                    failures.push(line);
                }
                if !cal_ok {
                    let _ =
                        writeln!(text, "FINDING d{day} {arch} {m:?} {v:.2} n {n} (asserted row, collapse ok); {cal}");
                    missed += 1;
                }
            }
            _ => {
                let why = if collapse.is_some() {
                    format!("n under {BEHAVIOUR_ASSERT_MIN_N}, not asserted")
                } else {
                    "no collapse rule".to_string()
                };
                let _ = writeln!(text, "FINDING d{day} {arch} {m:?} {v:.2} n {n} ({why}); {cal}");
                findings += 1;
                if !cal_ok {
                    missed += 1;
                }
            }
        }
    }
    let _ = writeln!(
        text,
        "behaviour tier: {asserted} rows asserted (collapse bounds), {findings} rows printed only as FINDING, {missed} \
         rows outside their calibration range (FINDING lines), {} failed",
        failures.len()
    );
    Ok(BehaviourReport { text, failures })
}
