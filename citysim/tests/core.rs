//! The core tier (docs/TESTING.md): the shipped city run long enough to
//! catch *broken*: a panic, a coin made or lost, a collapse, a mechanism
//! that never fires. Numbers that only describe the city (how many murders,
//! how full the Jail is) are reports, not gates: `tools/analyze_run.py`
//! prints them from `citysim-cli run --report`.
//!
//! `core_sanity`: seeds 42-44 in parallel threads, 120 days, one shared
//! collector: the coin identity every day, the collapse bounds, and the
//! mechanism-existence bullets over the three seeds. `core_year`: seed 42,
//! 365 days, the same bounds over 30-day windows (`#[ignore]`; run alone).

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use citysim::systems::econ;
use citysim::{Brain, Config, Controller, Corp, CorpOrder, DayRow, EntityId, EventKind, Lod, Posture, Sentence, World};
use citysim::{TICKS_PER_DAY, TICKS_PER_HOUR};

/// The ticks/s floor (release only; 2026-10-06, Dylan): a catastrophic
/// regression (a sim half as fast), not a target. Asserted on seed 42 in
/// `core_sanity` (the only non-ignored test in this binary, and cargo runs
/// test binaries one at a time, so it runs alone but for its two sibling
/// seeds); printed everywhere else.
const TPS_FLOOR: f64 = 4000.0;

// The collapse bounds: the v1 sanity trio scaled to 2,000 residents (M10
// plan D38), unchanged since M8.
const POP_MIN: u32 = 1333;
const POP_MAX: u32 = 2667;
/// Starvation deaths per 120 days.
const STARVATION_MAX_120: f64 = 200.0;
/// Assault + Murder events per day (2x the M7 baseline, per capita).
const ASSAULTS_PER_DAY_MAX: f64 = 42.7;
/// Seeded corps still trading at the run's end (the year gate's number).
const CORPS_ALIVE_MIN: usize = 4;

/// One seed's run: the day rows, every event kind's count, the mechanism
/// marks that need more than a kind, and the sanity readings.
struct SeedRun {
    seed: u64,
    rows: Vec<DayRow>,
    kinds: BTreeMap<EventKind, u32>,
    marks: BTreeMap<&'static str, u32>,
    /// Assault + Murder events per day.
    assaults: Vec<u32>,
    /// Days whose closing coin identity differs from day 0's.
    identity_broken: Vec<u64>,
    /// Starvation deaths of prisoners held at the Statistical tier.
    held_starved: u32,
    corps_seeded: usize,
    corps_alive: usize,
    tps: f64,
}

impl SeedRun {
    fn sum(&self, from: u64, to: u64, f: impl Fn(&DayRow) -> u32) -> f64 {
        self.rows.iter().filter(|r| r.day >= from && r.day < to).map(|r| f64::from(f(r))).sum()
    }

    fn kind(&self, k: EventKind) -> u32 {
        self.kinds.get(&k).copied().unwrap_or(0)
    }

    fn mark(&self, m: &str) -> u32 {
        self.marks.get(m).copied().unwrap_or(0)
    }
}

fn run_seed(seed: u64, days: u64) -> SeedRun {
    let mut w = World::new(seed, Config::load());
    let base = econ::identity(&w);
    let seeded: Vec<EntityId> = w.corps();
    let mut r = SeedRun {
        seed,
        rows: Vec::new(),
        kinds: BTreeMap::new(),
        marks: BTreeMap::new(),
        assaults: Vec::new(),
        identity_broken: Vec::new(),
        held_starved: 0,
        corps_seeded: seeded.len(),
        corps_alive: 0,
        tps: 0.0,
    };
    let mut cursor = 0u64;
    // Prisoners at the last hour's end, by their tier then.
    let mut held: BTreeSet<EntityId> = BTreeSet::new();
    let started = Instant::now();
    for day in 0..days {
        let mut assaults = 0u32;
        for _ in 0..24 {
            w.run_ticks(TICKS_PER_HOUR);
            for e in w.events.iter().rev().take_while(|e| e.id >= cursor) {
                *r.kinds.entry(e.kind).or_insert(0) += 1;
                let t = e.text.as_str();
                let mark = match e.kind {
                    EventKind::Assault | EventKind::Murder => {
                        assaults += 1;
                        None
                    }
                    EventKind::Raid if t.contains(" raided ") => Some("raid resolved"),
                    EventKind::Raid if t.contains("stormed the Precinct") => Some("Precinct stormed"),
                    EventKind::OrderChanged if t.contains("-> BreakOut") => Some("BreakOut order"),
                    EventKind::Founded if t.contains(" registered ") => Some("NPC founding"),
                    EventKind::Founded if t.contains("(researching)") => Some("Lab under Research"),
                    EventKind::Acquired if t.contains("(hostile)") => Some("hostile takeover"),
                    EventKind::Death if t.contains("died of Starvation") => {
                        let a = e.actors.first().copied();
                        if a.is_some_and(|a| held.contains(&a)) {
                            r.held_starved += 1;
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(m) = mark {
                    *r.marks.entry(m).or_insert(0) += 1;
                }
            }
            cursor = w.next_event_id;
            held = w
                .with::<Sentence>()
                .into_iter()
                .filter(|&a| w.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical))
                .collect();
        }
        let row = w.stats.history.back().expect("a day row").clone();
        assert_eq!(row.day, day, "seed {seed}: the day row closes with the day");
        if econ::identity(&w) != base {
            r.identity_broken.push(day);
        }
        let mut day_mark = |m: &'static str, on: bool| {
            if on {
                *r.marks.entry(m).or_insert(0) += 1;
            }
        };
        day_mark(
            "Squeeze held",
            w.corps().into_iter().any(|c| w.comp::<Corp>(c).is_some_and(|k| k.order == CorpOrder::Squeeze)),
        );
        day_mark("Crackdown held", w.law().is_some_and(|l| l.posture == Posture::Crackdown));
        day_mark("gang controls a district", w.districts.iter().any(|d| matches!(d.control, Controller::Gang(_))));
        day_mark("gang front", row.living.fronts > 0);
        *r.marks.entry("Dealing report").or_insert(0) += row.dealing_reports;
        *r.marks.entry("crash death").or_insert(0) += row.crash_deaths;
        *r.marks.entry("Mission meal").or_insert(0) += row.econ.mission_meals;
        r.assaults.push(assaults);
        r.rows.push(row);
    }
    r.tps = (days * TICKS_PER_DAY) as f64 / started.elapsed().as_secs_f64();
    let alive: BTreeSet<EntityId> = w.corps().into_iter().collect();
    r.corps_alive = seeded.iter().filter(|c| alive.contains(c)).count();
    r
}

/// The collapse bounds over days `[from, to)` of one run, as failure lines.
/// The starvation bound scales with the window (200 per 120 days).
fn sanity(r: &SeedRun, from: u64, to: u64, assaults_max: f64, failures: &mut Vec<String>) {
    let s = r.seed;
    let n = (to - from) as f64;
    let rows: Vec<&DayRow> = r.rows.iter().filter(|x| x.day >= from && x.day < to).collect();
    let jail_cap = u32::from(Config::load().buildings.jail.capacity);
    let mut fail = |ok: bool, what: String| {
        if !ok {
            failures.push(format!("seed {s} days {from}-{to}: {what}"));
        }
    };
    let pop: Vec<u32> = rows.iter().map(|x| x.population).collect();
    let (lo, hi) = (pop.iter().copied().min().unwrap_or(0), pop.iter().copied().max().unwrap_or(0));
    fail((POP_MIN..=POP_MAX).contains(&lo) && hi <= POP_MAX, format!("population {lo}..{hi} in {POP_MIN}..={POP_MAX}"));
    let starv = r.sum(from, to, |x| x.deaths_starvation);
    let starv_max = STARVATION_MAX_120 * n / 120.0;
    fail(starv <= starv_max, format!("starvation {starv:.0} <= {starv_max:.0}"));
    let asl: f64 = r.assaults[from as usize..to as usize].iter().map(|&a| f64::from(a)).sum::<f64>() / n;
    fail(asl <= assaults_max, format!("assaults/day {asl:.2} <= {assaults_max:.2}"));
    // The Treasury from day 30 (the opening hoard is spent into corps' capital first).
    let tmin = rows.iter().filter(|x| x.day >= 29).map(|x| x.treasury).min().unwrap_or(0);
    fail(tmin >= 0, format!("Treasury >= 0 from day 30 (min {tmin})"));
    let jmax = rows.iter().map(|x| x.jailed).max().unwrap_or(0);
    fail(jmax <= jail_cap, format!("jailed {jmax} <= capacity {jail_cap}"));
    // The Markets all empty for three days running: the food chain broke.
    let mut run = 0;
    let mut worst = 0;
    for x in &rows {
        run = if x.food_market == 0 { run + 1 } else { 0 };
        worst = worst.max(run);
    }
    fail(worst <= 2, format!("every Market empty {worst} days running <= 2"));
}

/// A mechanism bullet: its name, and how to count it on one seed.
type Mechanism = (&'static str, Box<dyn Fn(&SeedRun) -> u32>);

/// The mechanisms every seed set must show: (name, how to count it on one
/// seed). One count >= 1 over the three seeds passes. A mechanism that fired
/// on 2 or fewer of 6 seeds historically is not here; it has a unit test in
/// a seeded world (the file named after it).
fn mechanisms() -> Vec<Mechanism> {
    let k = |kind: EventKind| -> Box<dyn Fn(&SeedRun) -> u32> { Box::new(move |r: &SeedRun| r.kind(kind)) };
    let m = |mark: &'static str| -> Box<dyn Fn(&SeedRun) -> u32> { Box::new(move |r: &SeedRun| r.mark(mark)) };
    // Chosen on main 38210ce, 120 days, seeds 42-47 (each fired on 3-6 of 6 seeds); the 42-44
    // counts in brackets are the city after the off switches retired and the stat table was
    // regenerated in the full city (2026-10-09).
    vec![
        // M5, M6: the social and the vital.
        ("a Marriage", k(EventKind::Marriage)), // [591, 605, 607]
        ("a GangJoin", k(EventKind::GangJoin)), // [475, 525, 557]
        ("a Birth", k(EventKind::Birth)),       // [75, 71, 78]
        ("a Burial", k(EventKind::Burial)),     // [123, 119, 122]
        // M8 factions.
        ("a raid resolved", m("raid resolved")),            // [12, 13, 15]
        ("a Home flipped", k(EventKind::TerritoryFlipped)), // [35, 33, 95]
        // M9 the law.
        ("a Jailbreak", k(EventKind::Jailbreak)),    // [5, 4, 2]
        ("a day in Crackdown", m("Crackdown held")), // [47, 31, 44]
        // M10 off-screen lives.
        ("a hole bound", k(EventKind::Attributed)), // [905, 1007, 993]
        // M11 ownership and corps.
        ("an eviction", k(EventKind::Evicted)),        // [173, 216, 244]
        ("an NPC founding", m("NPC founding")),        // [9, 12, 8]
        ("a hostile takeover", m("hostile takeover")), // [9, 10, 9]
        ("a Strike", k(EventKind::Strike)),            // [10, 9, 9]
        // M12 districts.
        ("a Riot", k(EventKind::Riot)),        // [2, 10, 2]
        ("a gang Split", k(EventKind::Split)), // [1, 1, 1]; 5 of 6
        ("a Squat", k(EventKind::Squatted)),   // [93, 82, 94]
        // M13 assets.
        ("a Crash", k(EventKind::Crash)),              // [4, 3, 3]
        ("a chrome Install", k(EventKind::Installed)), // [252, 176, 263]
        ("a Dealing report", m("Dealing report")),     // [197, 238, 222]
        // M14 the Virt plane.
        ("a run (JackedIn)", k(EventKind::JackedIn)), // [133, 70, 61]
        ("a Data sale", k(EventKind::DataSold)),      // [8, 1, 0]; 5 of 6
        ("a Flatline", k(EventKind::Flatlined)),      // [3, 2, 3]
        // M15 the word and the blood.
        ("a Feed Story", k(EventKind::Story)),          // [810, 790, 789]
        ("a Hunt", k(EventKind::HuntStarted)),          // [94, 63, 66]
        ("a Vendetta", k(EventKind::Vendetta)),         // [7, 7, 6]
        ("a Purist expulsion", k(EventKind::Expelled)), // [0, 4, 5]; 5 of 6
        // L2 the living city, the Real economy.
        ("a World export", k(EventKind::Exported)),      // [294, 328, 319]
        ("a Collect", k(EventKind::Collected)),          // [57, 57, 58]
        ("a Bout", k(EventKind::Bout)),                  // [150, 182, 223]
        ("a gang front", m("gang front")),               // [0, 0, 74] days; 3 of 6 (the rarest bullet)
        ("a Mission meal", k(EventKind::MissionServed)), // [166, 250, 146]
        // M16a phase 3 (seeds 42-47 at phase 3, 120 days: each on 6 of 6).
        ("a BountyPaid", k(EventKind::BountyPaid)), // [729, 744, 731]; 45-47 [642, 725, 654]
        ("a city guard on the take", k(EventKind::GuardTaken)), // [23, 8, 37]; 45-47 [22, 27, 26]
    ]
}

/// Every mechanism's count per seed, then the bullets: one line each.
fn judge_mechanisms(runs: &[SeedRun], failures: &mut Vec<String>) {
    for (name, f) in mechanisms() {
        let per: Vec<u32> = runs.iter().map(&f).collect();
        let ok = per.iter().any(|&c| c >= 1);
        eprintln!("{} mechanism {name}: per seed {per:?}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures.push(format!("mechanism never fired on any seed: {name} {per:?}"));
        }
    }
}

fn report(r: &SeedRun) {
    let kinds: Vec<String> = r.kinds.iter().map(|(k, n)| format!("{k:?} {n}")).collect();
    let marks: Vec<String> = r.marks.iter().map(|(k, n)| format!("{k} {n}")).collect();
    let last = r.rows.last().map_or(0, |x| x.population);
    eprintln!(
        "seed {}: {} days, population {last}, starvation {:.0}, assaults/day {:.2}, held prisoners starved {}, corps alive {}/{}, {:.0} ticks/s",
        r.seed,
        r.rows.len(),
        r.sum(0, r.rows.len() as u64, |x| x.deaths_starvation),
        r.assaults.iter().sum::<u32>() as f64 / r.rows.len().max(1) as f64,
        r.held_starved,
        r.corps_alive,
        r.corps_seeded,
        r.tps
    );
    eprintln!("  events: {}", kinds.join(", "));
    eprintln!("  marks: {}", marks.join(", "));
}

/// The core sanity run: seeds 42-44, 120 days, in parallel threads.
#[test]
fn core_sanity() {
    let runs: Vec<SeedRun> = std::thread::scope(|s| {
        let hs: Vec<_> = [42u64, 43, 44].into_iter().map(|seed| s.spawn(move || run_seed(seed, 120))).collect();
        hs.into_iter().map(|h| h.join().expect("a core seed panicked")).collect()
    });
    let mut failures: Vec<String> = Vec::new();
    for r in &runs {
        report(r);
        if !r.identity_broken.is_empty() {
            failures.push(format!("seed {}: the coin identity moved on days {:?}", r.seed, r.identity_broken));
        }
        sanity(r, 0, 120, ASSAULTS_PER_DAY_MAX, &mut failures);
        if r.held_starved > 0 {
            failures.push(format!("seed {}: {} held prisoners starved", r.seed, r.held_starved));
        }
        if r.corps_alive < CORPS_ALIVE_MIN {
            failures.push(format!("seed {}: seeded corps alive {}/{}", r.seed, r.corps_alive, r.corps_seeded));
        }
    }
    judge_mechanisms(&runs, &mut failures);
    let tps42 = runs[0].tps;
    eprintln!("ticks/s seed 42 {tps42:.0} (floor {TPS_FLOOR:.0}, release; three seeds in parallel)");
    if !cfg!(debug_assertions) && tps42 < TPS_FLOOR {
        failures.push(format!("ticks/s {tps42:.0} >= {TPS_FLOOR:.0} on seed 42"));
    }
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "core_sanity failures: {failures:?}");
}

/// The year: seed 42, 365 days, the collapse bounds over each 30-day
/// window (the 5-day tail judged as its own window). `#[ignore]`: ~90 s;
/// run alone: `cargo test --release -p citysim --test core -- --ignored core_year`.
#[test]
#[ignore]
fn core_year() {
    const DAYS: u64 = 365;
    let r = run_seed(42, DAYS);
    report(&r);
    let mut failures: Vec<String> = Vec::new();
    if !r.identity_broken.is_empty() {
        failures.push(format!("the coin identity moved on days {:?}", r.identity_broken));
    }
    let mut from = 0;
    while from < DAYS {
        let to = (from + 30).min(DAYS);
        let n = (to - from) as f64;
        eprintln!(
            "  days {:>3}-{:<3} population {:>5} starvation {:>3.0} assaults/day {:>6.2} Treasury {:>7}",
            from + 1,
            to,
            r.rows[(to - 1) as usize].population,
            r.sum(from, to, |x| x.deaths_starvation),
            r.assaults[from as usize..to as usize].iter().sum::<u32>() as f64 / n,
            r.rows[(to - 1) as usize].treasury
        );
        // A 30-day window is noisier than 120 days: the year gate's 1.5x (seed 42 peaks at 36.3).
        sanity(&r, from, to, 1.5 * ASSAULTS_PER_DAY_MAX, &mut failures);
        from = to;
    }
    if r.held_starved > 0 {
        failures.push(format!("{} held prisoners starved", r.held_starved));
    }
    if r.corps_alive < CORPS_ALIVE_MIN {
        failures.push(format!("seeded corps alive on day 365 {}/{}", r.corps_alive, r.corps_seeded));
    }
    eprintln!("ticks/s {:.0} (floor {TPS_FLOOR:.0}, printed)", r.tps);
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "core_year failures: {failures:?}");
}
