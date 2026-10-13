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

use citysim::contract::{Broker, ContractKind};
use citysim::exec::ExecState;
use citysim::systems::{contracts, demography, econ};
use citysim::{
    Brain, Building, BuildingKind, Config, Controller, Corp, CorpOrder, DayRow, EntityId, EventKind, Household, Job,
    Lod, Posture, Sentence, World,
};
use citysim::{TICKS_PER_DAY, TICKS_PER_HOUR};

/// The ticks/s floor (release only; 2026-10-06, Dylan): a catastrophic
/// regression (a sim half as fast), not a target. Asserted only in
/// `core_throughput` (`#[ignore]`, run alone on a quiet box); printed in
/// `core_sanity` and `core_year`. It used to be asserted in `core_sanity`,
/// which failed whenever parallel coders were compiling on the box
/// (2026-10-09: 3.3-3.6k readings with the sim unchanged).
const TPS_FLOOR: f64 = 4000.0;

// The collapse bounds: the v1 sanity trio scaled to 2,000 residents (M10
// plan D38), unchanged since M8.
const POP_MIN: u32 = 1333;
const POP_MAX: u32 = 2667;
/// Starvation deaths per 120 days.
const STARVATION_MAX_120: f64 = 200.0;
/// Hunger as sickness (Dylan, decision 5): with `[sickness]` on, starvation
/// over 20 per 120 days means synth, the bins and the Missions all failed:
/// a broken free-food floor, a broken city.
const STARVATION_MAX_120_SICK: f64 = 20.0;
/// Assault + Murder events per day (2x the M7 baseline, per capita).
const ASSAULTS_PER_DAY_MAX: f64 = 42.7;
/// Seeded corps still trading at the run's end (the year gate's number).
const CORPS_ALIVE_MIN: usize = 4;
// Jobs and room J24/J25 (overturnable rows; P9a's bounds, carried by the
// flip-readiness round): from day 30, a jobless majority or a city on the
// street is broken, not brutal. Asserted only on a no-net city (`[economy2]
// wages` and `no_safety_net` on): the dole city is jobless by design.
/// J24: employed free adults over free adults (adults not jailed), every day from day 30.
const EMPLOYED_MIN_SHARE: f64 = 0.5;
/// J25: adults sleeping on the street over adults, every day from day 30.
/// Dylan, 2026-10-10 (hunger as sickness P4b): a squat, a cot or a Hotel bed
/// is a rung, not the street; the Dregs (`Household.home == None`) are
/// printed with their split beside it.
const HOMELESS_MAX_SHARE: f64 = 0.25;

/// The fulfilled-contract marks, by `ContractKind::index`.
const DONE_MARKS: [&str; 4] = ["Hit fulfilled", "Beat fulfilled", "Guard fulfilled", "Locate fulfilled"];

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
    /// J24/J25 at each day's end: (adults, free adults, employed free adults, street sleepers).
    adults: Vec<(u32, u32, u32, u32)>,
    /// The Dregs' split at each day's end: (Dregs, street, squat, of them in a
    /// shanty, a Mission cot, a Hotel bed).
    dregs: Vec<(u32, u32, u32, u32, u32, u32)>,
    /// The run is a no-net city (J24/J25 asserted): the market wage law, no dole or public works (P10).
    no_net: bool,
    /// `[sickness]` on (the tighter starvation bound and its bullets).
    sickness: bool,
    /// Transit: the El on (its bullets and sanity are judged).
    transit: bool,
    /// Scale S4: residents over the 2,000 city's (`Config::per_capita`):
    /// the collapse bounds' counts scale by it (1 for the shipped city).
    per: f64,
    /// The Jail's capacity in this world (Scale S4: per capita).
    jail_cap: u32,
    /// The world's save hash at the run's end (determinism).
    end_hash: blake3::Hash,
    /// The employment floor `sanity` asserts from day 30 (J24's
    /// `EMPLOYED_MIN_SHARE`; `core_20k` asserts the collapse floor).
    employed_min: f64,
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
    run_city(seed, days, Config::load())
}

/// Scale S4: the corps alive the bounds ask for, per capita (4 of the
/// shipped 9 rows).
fn corps_alive_min(r: &SeedRun) -> usize {
    (CORPS_ALIVE_MIN as f64 * r.per).round() as usize
}

fn run_city(seed: u64, days: u64, cfg: Config) -> SeedRun {
    let mut w = World::new(seed, cfg);
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
        adults: Vec::new(),
        dregs: Vec::new(),
        no_net: econ::wages_on(&w) && !citysim::systems::policy::net_enacted(&w),
        sickness: citysim::systems::health::on(&w),
        transit: w.config.transit.enabled,
        per: w.config.per_capita().max(1.0),
        jail_cap: u32::from(w.config.buildings.jail.capacity),
        end_hash: blake3::hash(&[]),
        employed_min: EMPLOYED_MIN_SHARE,
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
                    EventKind::Founded
                        if t.contains(" registered ")
                            && e.actors.get(1).and_then(|&b| w.comp::<Building>(b)).map(|b| b.kind)
                                == Some(BuildingKind::Fixer) =>
                    {
                        *r.marks.entry("NPC-founded Fixer").or_insert(0) += 1;
                        Some("NPC founding")
                    }
                    EventKind::Founded if t.contains(" registered ") => Some("NPC founding"),
                    EventKind::Accessory if t.contains(" killing ") && !t.contains("(0 days after)") => {
                        Some("Hit later attributed")
                    }
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
            // Transit (docs/TRANSIT.md § 7): no body left aboard past its
            // arrival, and every filed ride belongs to a body on one of its
            // legs (walking, on the platform or aboard).
            if w.config.transit.enabled {
                for &a in w.rides.keys() {
                    let on_leg = w.comp::<Brain>(a).is_some_and(|b| {
                        matches!(b.exec, ExecState::Ride { .. } | ExecState::Goto { .. } | ExecState::GotoTimed { .. })
                    });
                    if !on_leg {
                        *r.marks.entry("stale ride").or_insert(0) += 1;
                    }
                }
                for a in w.citizens() {
                    if let Some(ExecState::Ride { arrive_tick, .. }) = w.comp::<Brain>(a).map(|b| &b.exec) {
                        if w.tick > arrive_tick + 1 {
                            *r.marks.entry("ride past arrival").or_insert(0) += 1;
                        }
                    }
                }
            }
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
        day_mark("malnourished agent", row.health.malnourished > 0);
        day_mark("a sick day", row.health.sick_days > 0);
        // M17 phase 2: a branch sent coins home at the day's midnight.
        day_mark("remittance", row.outside.as_ref().is_some_and(|o| o.remit > 0));
        *r.marks.entry("Dealing report").or_insert(0) += row.dealing_reports;
        // Transit: a ride (any tier) and a fare paid to the city.
        let t = &row.transit;
        *r.marks.entry("El ride").or_insert(0) += t.rides_full + t.rides_coarse + t.rides_stat;
        *r.marks.entry("El fare").or_insert(0) += u32::from(t.fares > 0);
        *r.marks.entry("crash death").or_insert(0) += row.crash_deaths;
        *r.marks.entry("Mission meal").or_insert(0) += row.econ.mission_meals;
        // Hunger as sickness: the day's synth and bin meals, a malnourished
        // day, the shakedowns (zeros with `[sickness]` off).
        *r.marks.entry("synth meal").or_insert(0) += row.health.meals_synth;
        *r.marks.entry("bin meal").or_insert(0) += row.health.meals_dive;
        *r.marks.entry("bin shakedown").or_insert(0) += row.health.bin_shakedowns;
        // M16a: the contract columns (printed; the bullets read the
        // Guard and Locate rows), and a Fixer in business (open, a record
        // brokered there in the last 7 days: the spec's definition).
        let c = &row.contract;
        for k in ContractKind::M16A {
            *r.marks.entry(DONE_MARKS[k.index()]).or_insert(0) += c.k_done[k.index()];
        }
        *r.marks.entry("squad Hit").or_insert(0) += c.hits_squad;
        // The Hit market: an organic Hit (a motive set) fulfilled or reneged.
        *r.marks.entry("organic Hit fulfilled").or_insert(0) += row.hit.hm_hits_done;
        *r.marks.entry("StrikeDeclined political").or_insert(0) += c.strikes_declined_pol;
        *r.marks.entry("Fixer run").or_insert(0) += c.fixer_runs;
        *r.marks.entry("escrow leak days").or_insert(0) += u32::from(c.escrow_leak != 0);
        *r.marks.entry("escrow stuck days").or_insert(0) += u32::from(c.escrow_stuck != 0);
        let week = w.tick.saturating_sub(7 * TICKS_PER_DAY);
        let in_business = contracts::open_fixers(&w)
            .into_iter()
            .any(|f| w.has::<Broker>(f) && w.contracts.values().any(|x| x.broker == Some(f) && x.posted >= week));
        *r.marks.entry("Fixer in business").or_insert(0) += u32::from(in_business);
        // M16b phase 1: leverage's day.
        let l = &row.leverage;
        *r.marks.entry("protection paid").or_insert(0) += l.protection_paid;
        *r.marks.entry("blackmail made").or_insert(0) += l.blackmail_made;
        *r.marks.entry("debt called").or_insert(0) += l.lev_debt_ok;
        *r.marks.entry("report withdrawn under a threat").or_insert(0) += l.silenced_threat;
        *r.marks.entry("coerced job").or_insert(0) += l.coerced_jobs;
        *r.marks.entry("fake job").or_insert(0) += l.fake_jobs;
        *r.marks.entry("Parley").or_insert(0) += l.parleys;
        *r.marks.entry("poach by threat").or_insert(0) += l.poach_threats;
        *r.marks.entry("Fraud filed").or_insert(0) += l.frauds_filed;
        *r.marks.entry("secret lapsed").or_insert(0) += l.secrets_lapsed;
        *r.marks.entry("Persuade posted").or_insert(0) += l.k_persuade_posted;
        *r.marks.entry("coerced job done").or_insert(0) += l.coerced_done;
        // M16b phase 2: captives and the new kinds.
        let k = &row.captive;
        *r.marks.entry("Abduct posted").or_insert(0) += k.k_posted[0];
        *r.marks.entry("Steal fulfilled").or_insert(0) += k.k_done[3];
        *r.marks.entry("heist run").or_insert(0) += k.steal_runs;
        *r.marks.entry("Spin posted").or_insert(0) += k.k_posted[5];
        *r.marks.entry("hostage taken").or_insert(0) += k.hostages_taken;
        *r.marks.entry("Rescue posted").or_insert(0) += k.k_posted[1];
        *r.marks.entry("Escort posted").or_insert(0) += k.k_posted[2];
        *r.marks.entry("Deliver fulfilled").or_insert(0) += k.k_done[4];
        *r.marks.entry("fabrication planted").or_insert(0) += k.fabricated;
        *r.marks.entry("fabrication exposed").or_insert(0) += k.exposed;
        // M16b phase 3: the wounded.
        let wd = &row.wounded;
        *r.marks.entry("downed").or_insert(0) += wd.downed;
        *r.marks.entry("bled out").or_insert(0) += wd.bled_out;
        *r.marks.entry("stabilised").or_insert(0) += wd.stabilised;
        *r.marks.entry("recovered").or_insert(0) += wd.recovered;
        *r.marks.entry("finished off").or_insert(0) += wd.finished_off;
        *r.marks.entry("Trauma subscription").or_insert(0) += wd.trauma_subs_new;
        *r.marks.entry("rescued by kin").or_insert(0) += wd.rescued_kin;
        *r.marks.entry("rescued by a gang-mate").or_insert(0) += wd.rescued_gang;
        *r.marks.entry("Trauma call").or_insert(0) += wd.trauma_calls;
        *r.marks.entry("TraumaSave").or_insert(0) += wd.trauma_saves;
        // M16b phase 4: boards, tags, the Patrol, attention.
        let gv = &row.gov;
        *r.marks.entry("board vote").or_insert(0) += gv.votes;
        *r.marks.entry("VoteSwung").or_insert(0) += gv.votes_swung;
        *r.marks.entry("campaign move").or_insert(0) += gv.campaign_moves;
        *r.marks.entry("tag planted").or_insert(0) += gv.tags_planted;
        *r.marks.entry("tag found").or_insert(0) += gv.tags_found;
        *r.marks.entry("scan").or_insert(0) += gv.scans;
        *r.marks.entry("Patrol scan").or_insert(0) += gv.patrols;
        *r.marks.entry("faction distracted").or_insert(0) += u32::from(gv.distracted > 0);
        // A broken-system signal: alertness ÷ pool under the floor (the
        // ratio is never under `alert_floor` while the pool is at most 1).
        let floor = w.config.attention.alert_floor;
        *r.marks.entry("alertness under the floor").or_insert(0) +=
            u32::from(w.config.attention.enabled && gv.alert_min < floor - 1e-4);
        r.assaults.push(assaults);
        r.rows.push(row);
        if r.no_net {
            r.adults.push(adult_counts(&w));
            r.dregs.push(dreg_split(&w));
        }
    }
    r.tps = (days * TICKS_PER_DAY) as f64 / started.elapsed().as_secs_f64();
    let alive: BTreeSet<EntityId> = w.corps().into_iter().collect();
    r.corps_alive = seeded.iter().filter(|c| alive.contains(c)).count();
    if r.per > 1.0 {
        r.end_hash = blake3::hash(citysim::save::to_ron(&w).as_bytes());
    }
    r
}

/// J24/J25's day-end counts: (adults, free adults, employed free adults,
/// street sleepers). An adult is a living citizen with a Brain of age; free
/// is not jailed; a street sleeper is a Dreg (a Household with no home)
/// with no Hotel or cot booked and no squat (Dylan, P4b).
fn adult_counts(w: &World) -> (u32, u32, u32, u32) {
    let (mut adults, mut free, mut employed, mut homeless) = (0u32, 0u32, 0u32, 0u32);
    for a in w.citizens() {
        if !w.has::<Brain>(a) || !demography::is_adult(w, a) {
            continue;
        }
        adults += 1;
        if matches!(w.comp::<Household>(a), Some(Household { home: None, .. }))
            && !w.has::<citysim::Squatter>(a)
            && citysim::systems::street::booked_hotel(w, a).is_none()
        {
            homeless += 1;
        }
        if w.has::<Sentence>(a) {
            continue;
        }
        free += 1;
        if w.has::<Job>(a) {
            employed += 1;
        }
    }
    (adults, free, employed, homeless)
}

/// The Dregs' split (printed beside J25): (Dregs, street, squat, in a
/// shanty, a Mission cot, a Hotel bed).
fn dreg_split(w: &World) -> (u32, u32, u32, u32, u32, u32) {
    let (mut dregs, mut street, mut squat, mut shanty, mut cot, mut hotel) = (0, 0, 0, 0, 0, 0);
    for a in w.citizens() {
        if !w.has::<Brain>(a) || !demography::is_adult(w, a) {
            continue;
        }
        if !matches!(w.comp::<Household>(a), Some(Household { home: None, .. })) {
            continue;
        }
        dregs += 1;
        if let Some(s) = w.comp::<citysim::Squatter>(a) {
            squat += 1;
            shanty += u32::from(citysim::systems::street::is_shanty(w, s.building));
        } else if let Some(h) = citysim::systems::street::booked_hotel(w, a) {
            if citysim::systems::charity::is_cot_house(w, h) {
                cot += 1;
            } else {
                hotel += 1;
            }
        } else {
            street += 1;
        }
    }
    (dregs, street, squat, shanty, cot, hotel)
}

/// The collapse bounds over days `[from, to)` of one run, as failure lines.
/// The starvation bound scales with the window (200 per 120 days).
fn sanity(r: &SeedRun, from: u64, to: u64, assaults_max: f64, failures: &mut Vec<String>) {
    let s = r.seed;
    let n = (to - from) as f64;
    let rows: Vec<&DayRow> = r.rows.iter().filter(|x| x.day >= from && x.day < to).collect();
    let jail_cap = r.jail_cap;
    let mut fail = |ok: bool, what: String| {
        if !ok {
            failures.push(format!("seed {s} days {from}-{to}: {what}"));
        }
    };
    // Scale S4: the counts per capita (`r.per` is 1 in the shipped city).
    let scaled = |v: u32| (f64::from(v) * r.per).round() as u32;
    let (pop_min, pop_max) = (scaled(POP_MIN), scaled(POP_MAX));
    let assaults_max = assaults_max * r.per;
    let pop: Vec<u32> = rows.iter().map(|x| x.population).collect();
    let (lo, hi) = (pop.iter().copied().min().unwrap_or(0), pop.iter().copied().max().unwrap_or(0));
    fail((pop_min..=pop_max).contains(&lo) && hi <= pop_max, format!("population {lo}..{hi} in {pop_min}..={pop_max}"));
    let starv = r.sum(from, to, |x| x.deaths_starvation);
    let per_120 = if r.sickness { STARVATION_MAX_120_SICK } else { STARVATION_MAX_120 };
    let starv_max = per_120 * n / 120.0 * r.per;
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
    // M16a (5.1 b): a contract closes only by paying or refunding its
    // escrow: Σ `Contract.escrow` equals `escrow_held` every day.
    let leaks: Vec<u64> = rows.iter().filter(|x| x.contract.escrow_leak != 0).map(|x| x.day).collect();
    fail(leaks.is_empty(), format!("escrow_leak == 0 every day (non-zero on days {leaks:?})"));
    // J24/J25 from day 30 (the day index 29, as the Treasury's), no-net only.
    let days: Vec<(u64, (u32, u32, u32, u32))> =
        (from.max(29)..to).filter_map(|d| r.adults.get(d as usize).map(|&c| (d, c))).collect();
    let share = |n: u32, of: u32| f64::from(n) / f64::from(of.max(1));
    if let Some(&(d, (_, free, emp, _))) =
        days.iter().min_by(|a, b| share(a.1 .2, a.1 .1).total_cmp(&share(b.1 .2, b.1 .1)))
    {
        let e = share(emp, free);
        fail(
            e >= r.employed_min,
            format!("J24 employed {emp} of {free} free adults ({e:.3}) on day {d} >= {}", r.employed_min),
        );
    }
    if let Some(&(d, (adults, _, _, hl))) =
        days.iter().max_by(|a, b| share(a.1 .3, a.1 .0).total_cmp(&share(b.1 .3, b.1 .0)))
    {
        let h = share(hl, adults);
        let over = days.iter().filter(|(_, c)| share(c.3, c.0) > HOMELESS_MAX_SHARE).count();
        // The Dregs beside it (printed, not asserted).
        if let Some(&(dregs, street, squat, shanty, cot, hotel)) = r.dregs.get(d as usize) {
            eprintln!(
                "seed {s} J25 peak day {d}: street {street} of {adults} adults ({h:.3}); Dregs {dregs} = street {street}, squat {squat} (shanty {shanty}), cot {cot}, Hotel {hotel}"
            );
        }
        fail(
            h <= HOMELESS_MAX_SHARE,
            format!(
                "J25 street sleepers {hl} of {adults} adults ({h:.3}) on day {d} <= {HOMELESS_MAX_SHARE} ({over} days over)"
            ),
        );
    }
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
        // "a day in Crackdown" is printed since the Hit market P2 (below).
        // M10 off-screen lives.
        ("a hole bound", k(EventKind::Attributed)), // [905, 1007, 993]
        // M11 ownership and corps.
        ("an eviction", k(EventKind::Evicted)),        // [173, 216, 244]
        ("an NPC founding", m("NPC founding")),        // [9, 12, 8]
        ("a hostile takeover", m("hostile takeover")), // [9, 10, 9]
        ("a Strike", k(EventKind::Strike)),            // [10, 9, 9]
        // M12 districts.
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
        ("a Mission meal", k(EventKind::MissionServed)), // [166, 250, 146]
        // M16a phase 3 (seeds 42-47 at phase 3, 120 days: each on 6 of 6).
        ("a BountyPaid", k(EventKind::BountyPaid)), // [729, 744, 731]; 45-47 [642, 725, 654]
        ("a city guard on the take", k(EventKind::GuardTaken)), // [23, 8, 37]; 45-47 [22, 27, 26]
        // M16a phase 5 (5.1 a; seeds 42-47 on ba516f2, 120 days). Fired on 6 of 6: a Guard and a
        // Locate fulfilled, a Fixer in business (days), a Reneged. On 0 of 6, so unit tests in
        // a seeded world instead (docs/TESTING.md): a Hit fulfilled (contracts.rs
        // test_fulfilled_pays_taker_and_fixer_at_cut), a Beat fulfilled (test_conservation_over_
        // every_phase_one_flow), a squad Hit (missions.rs test_squad_when_solo_estimate_below_
        // squad_below), a SoldOut and a StrikeDeclined on political cost (test_gang_sells_out_
        // non_member_refuses_member), an Accessory a day after the killing (contracts.rs
        // test_interrogation_names_buyer_then_conspiracy_filed), an NPC-founded Fixer
        // (test_npc_registers_a_fixer); a Fixer run to SellData on 2 of 6 ([1, 0, 4, 0, 0, 0]:
        // test_fixer_run_order_runs_to_selldata_with_cut).
        ("a Guard fulfilled", m("Guard fulfilled")), // [11, 4, 12]; 45-47 [16, 11, 8]
        ("a Locate fulfilled", m("Locate fulfilled")), // [167, 166, 155]; 45-47 [145, 137, 133]
        ("a Fixer in business", m("Fixer in business")), // [111, 107, 112] days; 45-47 [106, 111, 109]
        ("a Reneged", k(EventKind::Reneged)),
        // Transit (docs/TRANSIT.md § 7), with the El on: rides [..] and fare
        // days [..] on 42-44 (see `transit_mechanisms`).        // [22, 6, 12]; 45-47 [23, 6, 12]
        // "An organic Hit fulfilled" is printed (the Hit market phase 4, below).
        // M16b phase 1, leverage (seeds 42-47 x 120 days; each on 6 of 6). Fired on 2 or
        // fewer of 6, so printed below and unit-tested in `leverage.rs`: a blackmail lapsed
        // (test_blackmail_pays_then_lapses_when_public), a Fraud filed (test_renege_files_fraud_
        // by_a_lawful_taker, test_fake_job_fee_then_the_victim_learns), a coerced job done
        // (test_coerced_job_unpaid_and_complied), a Persuade posted (0 of 6: the Court suitor is
        // rare; test_court_persuade_posted_and_resolved); poaching by threat on 3-4 of 6
        // (test_poach_by_threat_only_for_dirty_exec).
        ("a protection paid", m("protection paid")), // [37, 6, 4]; 45-47 [10, 19, 20]
        ("a blackmail made", m("blackmail made")),   // [16, 24, 19]; 45-47 [15, 12, 23]
        ("a debt called in", m("debt called")),      // [39, 88, 53]; 45-47 [60, 56, 61]
        ("a report withdrawn under a threat", m("report withdrawn under a threat")), // [42, 33, 45]; 45-47 [36, 34, 47]
        ("a coerced job", m("coerced job")),         // [14, 7, 7]; 45-47 [5, 9, 3]
        ("a fake job", m("fake job")),               // [1, 4, 3]; 45-47 [1, 2, 10]
        ("a Parley", m("Parley")),                   // [1, 1, 2]; 45-47 [1, 1, 3] (3-6 of 6 over three runs)
        // M16b phase 2, captives and the new kinds (seeds 42-47 x 120 days; 5-6 of 6). Fired on 3
        // or fewer, so printed below and unit-tested in `captives.rs`: a hostage taken (and its
        // ransom, test_ransom_paid_frees_the_captive), a Rescue and an Escort posted (0 of 6:
        // test_refusal_posts_rescue_and_escort, test_rescue_win_frees_the_holders_captives,
        // test_escort_kept_is_fulfilled), a Deliver fulfilled (test_deliver_posted_for_a_corpse_and_
        // buried), a fabrication planted and exposed (test_fabricated_spin_and_exposure).
        ("an Abduct posted", m("Abduct posted")), // [8, 8, 5]; 45-47 [2, 3, 1]
        ("a Steal fulfilled", m("Steal fulfilled")), // [10, 8, 5]; 45-47 [21, 9, 28]
        ("a heist run", m("heist run")),          // [14, 10, 21]; 45-47 [15, 0, 23]
        ("a Spin posted", m("Spin posted")),      // [1, 6, 3]; 45-47 [6, 4, 4]
        // M16b phase 3, the wounded (seeds 42-47 x 120 days; each on 6 of 6). Fewer, so printed
        // below and unit-tested in `wounded.rs`: a rescue by a gang-mate (1 of 6;
        // test_abandon_gang_leaves_its_own carries the Loyal/Abandon rule), a Trauma call (2 of
        // 6) and a TraumaSave (0 of 6: test_trauma_call_and_save).
        ("a downed agent", m("downed")),          // [71, 65, 63]; 45-47 [73, 72, 44]
        ("a bleed-out", m("bled out")),           // [48, 48, 47]; 45-47 [44, 51, 30]
        ("a stabilisation", m("stabilised")),     // [3, 4, 5]; 45-47 [15, 8, 3]
        ("a rescue by kin", m("rescued by kin")), // [2, 1, 3]; 45-47 [10, 3, 3]
        ("a downed agent finished off", m("finished off")), // [9, 5, 3]; 45-47 [9, 5, 3]
        ("a Trauma subscription", m("Trauma subscription")), // [27, 15, 22]; 45-47 [34, 16, 19]
        // M16b phase 4 (seeds 42-47 x 120 days): boards, tags, attention.
        ("a board vote", m("board vote")),       // [4, 5, 4]; 45-47 [4, 4, 4]
        ("a campaign move", m("campaign move")), // [0, 3, 1]; 45-47 [4, 3, 0]
        ("a VoteSwung", m("VoteSwung")),         // [0, 1, 1]; 45-47 [4, 3, 0]
        ("a tag planted", m("tag planted")),     // [85, 109, 98]; 45-47 [61, 66, 84]
        ("a tag found", m("tag found")),         // [5, 8, 7]; 45-47 [7, 0, 6]
        ("a scan", m("scan")),                   // [200, 239, 182]; 45-47 [216, 191, 245]
        ("a day a faction is distracted", m("faction distracted")), // [116, 114, 105]; 45-47 [115, 117, 113]
        // M17 phase 2 (seeds 42-47 x 120 days): the parents.
        ("a day with a remittance home", m("remittance")), // [87, 87, 81]; 45-47 [85, 93, 87]
        ("a Subsidised", k(EventKind::Subsidised)),        // [30, 47, 32]; 45-47 [41, 96, 41]
    ]
}

/// Hunger as sickness: the bullets read with the diet on (always since
/// P10; `[sickness] diet` is the part key). Chosen on the flipped city,
/// 120 days, seeds 42-47 (each on 6 of 6; see the P5 commit). A regular's
/// bin contest, a diagnosis, a cure and a Disease death fired on 2 or fewer
/// of 6: unit tests in `sickness.rs` instead.
fn sickness_mechanisms() -> Vec<Mechanism> {
    let m = |mark: &'static str| -> Box<dyn Fn(&SeedRun) -> u32> { Box::new(move |r: &SeedRun| r.mark(mark)) };
    vec![
        ("a synth meal", m("synth meal")),
        ("a bin meal", m("bin meal")),
        ("a bin shakedown", m("bin shakedown")),
        ("a malnourished agent", m("malnourished agent")),
        ("a sick day", m("a sick day")),
    ]
}

/// Printed findings, not bullets: rare events (0-2 a seed) that flip on any
/// trajectory change (hunger as sickness P5i, 2026-10-10: a Riot [2, 2, 0]
/// and a gang front [0, 0, 32] days with item 11 off read [0, 0, 0] on
/// both under a one-step butterfly, `max_coarse` 151, and fired again
/// with P5i on under two others). Each is a unit-tested mechanism instead
/// (docs/TESTING.md): `districts.rs::test_riot_fires_at_streak_and_picks_target`
/// (and its streak-short negative), `leisure.rs::test_front_built_under_expand_in_held_district`
/// and `test_no_front_without_expand_or_treasury`. A Jailbreak likewise
/// (2026-10-10, the flipped city after the recalibration: [0, 0, 0] on
/// 42-44, [0, 1, 1] under a one-step butterfly): `factions.rs::
/// test_breach_with_no_guards_frees_the_boss_first_and_reopens_the_warrant`
/// (the break fires) with its negatives `test_breach_with_nobody_inside_fizzles`
/// and `test_breach_against_three_guards_fails_and_shocks`.
fn printed_mechanisms() -> Vec<Mechanism> {
    let k = |kind: EventKind| -> Box<dyn Fn(&SeedRun) -> u32> { Box::new(move |r: &SeedRun| r.kind(kind)) };
    let m = |mark: &'static str| -> Box<dyn Fn(&SeedRun) -> u32> { Box::new(move |r: &SeedRun| r.mark(mark)) };
    vec![
        ("a Riot", k(EventKind::Riot)),
        ("a gang front", m("gang front")),
        ("a Jailbreak", k(EventKind::Jailbreak)),
        // M9's day in Crackdown, demoted (the Hit market P2, 2026-10-11): on
        // the flipped city it is rare, days held over 42-47 x 120 days [4, 9,
        // 0, 1, 0, 0] with the market off (main) and [0, 0, 0, 4, 6, 0] with
        // P2 (cold cases and recants on); `law_faction.rs::
        // test_posture_patrol_at_rest_and_crackdown_under_pressure` carries it.
        ("a day in Crackdown", m("Crackdown held")),
        // The Hit market phase 4 (2026-10-11): an organic Hit (a motive set)
        // fulfilled or reneged (`hm_hits_done`), seeds 42-47 x 120 days: [1,
        // 1, 1, 0, 1, 0] on fe60d37 (4 of 6), [1, 1, 1, 0, 0, 0] with the
        // phase's D8 fix (a buyer does not hunt the head it paid for), 2 of 6
        // on the main before the market's last fix round: one a seed at most,
        // so it flips with any trajectory change. Unit-tested instead:
        // `hitmarket.rs` (`test_revenge_pooled_from_kin_half_down`,
        // `test_credit_buyer_pays_balance`, `test_stiffed_buyer_owes_and_is_hated`),
        // `person.rs::probe_widow_commissions_a_hit_at_the_fixer`, and the
        // god scenario `god_kill_ten_fathers_day_10`.
        ("an organic Hit fulfilled", m("organic Hit fulfilled")),
        // M16b phase 1 (seeds 42-47 x 120 days): rare, unit-tested in `leverage.rs`.
        ("a poach by threat", m("poach by threat")),   // [0, 3, 2, 0, 1, 1]
        ("a Fraud filed", m("Fraud filed")),           // [0, 2, 1, 0, 0, 0]
        ("a secret lapsed", m("secret lapsed")),       // [0, 1, 0, 0, 0, 0]
        ("a Persuade posted", m("Persuade posted")),   // [0, 0, 0, 0, 0, 0]
        ("a coerced job done", m("coerced job done")), // [2, 0, 0, 0, 0, 0]
        // M16b phase 2 (seeds 42-47 x 120 days): rare, unit-tested in `captives.rs`.
        ("a hostage taken", m("hostage taken")),             // [0, 1, 1, 1, 0, 0]
        ("a Rescue posted", m("Rescue posted")),             // [0, 0, 0, 0, 0, 0]
        ("an Escort posted", m("Escort posted")),            // [0, 0, 0, 0, 0, 0]
        ("a Deliver fulfilled", m("Deliver fulfilled")),     // [2, 0, 0, 0, 0, 1]
        ("a fabrication planted", m("fabrication planted")), // [0, 0, 0, 1, 0, 0]
        ("a fabrication exposed", m("fabrication exposed")), // [0, 0, 0, 0, 0, 0]
        // M16b phase 3 (seeds 42-47 x 120 days): rare, unit-tested in `wounded.rs`.
        ("a rescue by a gang-mate", m("rescued by a gang-mate")), // [0, 0, 0, 1, 0, 0]
        ("a Trauma call", m("Trauma call")),                      // [0, 0, 0, 1, 0, 1]
        ("a TraumaSave", m("TraumaSave")),                        // [0, 0, 0, 0, 0, 0]
        // M16b phase 4 (seeds 42-47 x 120 days): rare, unit-tested in `tags.rs`
        // and `governance.rs`.
        ("a Patrol scan", m("Patrol scan")), // [0, 0, 0, 0, 0, 0]
    ]
}

/// Every mechanism's count per seed, then the bullets: one line each.
/// Transit (docs/TRANSIT.md § 7): judged with the El on.
fn transit_mechanisms() -> Vec<Mechanism> {
    let m = |mark: &'static str| -> Box<dyn Fn(&SeedRun) -> u32> { Box::new(move |r: &SeedRun| r.mark(mark)) };
    vec![("an El ride", m("El ride")), ("a fare paid to the city", m("El fare"))]
}

fn judge_mechanisms(runs: &[SeedRun], failures: &mut Vec<String>) {
    for (name, f) in printed_mechanisms() {
        let per: Vec<u32> = runs.iter().map(&f).collect();
        eprintln!("FINDING mechanism {name} (printed, unit-tested): per seed {per:?}");
    }
    let mut all = mechanisms();
    if runs.first().is_some_and(|r| r.sickness) {
        all.extend(sickness_mechanisms());
    }
    if runs.first().is_some_and(|r| r.transit) {
        all.extend(transit_mechanisms());
    }
    for (name, f) in all {
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
        if r.corps_alive < corps_alive_min(r) {
            failures.push(format!("seed {}: seeded corps alive {}/{}", r.seed, r.corps_alive, r.corps_seeded));
        }
        for m in ["stale ride", "ride past arrival", "alertness under the floor"] {
            if r.mark(m) > 0 {
                failures.push(format!("seed {}: {} hours with a {m}", r.seed, r.mark(m)));
            }
        }
    }
    judge_mechanisms(&runs, &mut failures);
    let tps42 = runs[0].tps;
    eprintln!("ticks/s seed 42 {tps42:.0} (floor {TPS_FLOOR:.0} asserted in core_throughput; printed here)");
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
    if r.corps_alive < corps_alive_min(&r) {
        failures.push(format!("seeded corps alive on day 365 {}/{}", r.corps_alive, r.corps_seeded));
    }
    eprintln!("ticks/s {:.0} (floor {TPS_FLOOR:.0}, printed)", r.tps);
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "core_year failures: {failures:?}");
}

/// The throughput floor: seed 42 alone for 30 days, release only.
/// `#[ignore]`: run it alone on a quiet box (no other builds or sims):
/// `cargo test --release -p citysim --test core -- --ignored core_throughput`.
#[test]
#[ignore]
fn core_throughput() {
    let r = run_seed(42, 30);
    eprintln!("ticks/s seed 42, 30 days alone: {:.0} (floor {TPS_FLOOR:.0})", r.tps);
    if !cfg!(debug_assertions) {
        assert!(r.tps >= TPS_FLOOR, "ticks/s {:.0} under the {TPS_FLOOR:.0} floor", r.tps);
    }
}

/// Scale S4: the 20k profile's ticks/s floor, printed only (as
/// `core_year`'s): measured 155-173 ticks/s over 30 days (the two runs in
/// parallel, 2026-10-12), less ~25 %; S4 perf (the goal scoring's indices,
/// docs/SCALE_20K.md § S4 perf): 189, less ~25 %.
const TPS_FLOOR_20K: f64 = 140.0;

/// Scale S4 (coordinator ruling, 2026-10-12): `core_20k`'s employment
/// floor, a collapse floor and not a calibration band. J24's 0.5 is
/// printed beside it: the 20k city reads ~0.50 on day 30 (the 2,000 city
/// ~0.53), a calibration gap; S3b's collapsed city read 0.17.
const EMPLOYED_FLOOR_20K: f64 = 0.40;

/// Scale S4 (docs/SCALE_20K.md § S4): the generated 20,000-resident city
/// (`gen:20000`, `Config::scaled_to(20_000)`), seed 42 for 30 days, twice in
/// parallel threads: `core_sanity`'s collapse bounds per capita (the
/// population, starvation, assault and corps-alive counts scaled by
/// `Config::per_capita`; the Treasury, the Jail, the Markets, escrow and
/// J25 as they are; employment at the collapse floor
/// `EMPLOYED_FLOOR_20K`), the coin identity every day, and the two runs'
/// closing save hashes equal (determinism). The calibration numbers
/// (J24 against 0.5, employed, homeless, Murders per 1,000) are printed
/// `FINDING` lines, never asserted. `#[ignore]`; run alone:
/// `cargo test --release -p citysim --test core -- --ignored core_20k --nocapture`.
#[test]
#[ignore]
fn core_20k() {
    const DAYS: u64 = 30;
    let cfg = || {
        let mut c = Config::load().scaled_to(20_000);
        c.world.map = "gen:20000".into();
        // M17 phase 1: the 20k city sits on an island of Ambient flats (S5's
        // 50 a slot on gen20k), whose households buy at its businesses.
        c.ambient.flats_per_slot = 50.0;
        c
    };
    let mut runs: Vec<SeedRun> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..2).map(|_| s.spawn(move || run_city(42, DAYS, cfg()))).collect();
        hs.into_iter().map(|h| h.join().expect("a core_20k run panicked")).collect()
    });
    runs[0].employed_min = EMPLOYED_FLOOR_20K;
    let r = &runs[0];
    report(r);
    let mut failures: Vec<String> = Vec::new();
    if !r.identity_broken.is_empty() {
        failures.push(format!("the coin identity moved on days {:?}", r.identity_broken));
    }
    if runs[0].end_hash != runs[1].end_hash {
        failures.push("two runs of seed 42 ended in different worlds (nondeterminism)".into());
    }
    sanity(r, 0, DAYS, ASSAULTS_PER_DAY_MAX, &mut failures);
    if r.held_starved > 0 {
        failures.push(format!("{} held prisoners starved", r.held_starved));
    }
    if r.corps_alive < corps_alive_min(r) {
        failures.push(format!("seeded corps alive {}/{} < {}", r.corps_alive, r.corps_seeded, corps_alive_min(r)));
    }
    for m in ["stale ride", "ride past arrival", "alertness under the floor"] {
        if r.mark(m) > 0 {
            failures.push(format!("{} hours with a {m}", r.mark(m)));
        }
    }
    // M17 phase 1: the Ambient book buys on every day from day 2 (an
    // existence assert: the 2k city has no flats, so no core_sanity bullet).
    let dry: Vec<usize> = r
        .rows
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, x)| x.ambient.as_ref().is_none_or(|a| a.food_bought + a.visits_made == 0))
        .map(|(d, _)| d + 1)
        .collect();
    if !dry.is_empty() {
        failures.push(format!("no Ambient sale on days {dry:?}"));
    }
    for d in [1u64, 9, 19, DAYS - 1] {
        if let Some(a) = r.rows.get(d as usize).and_then(|x| x.ambient.as_ref()) {
            let wages = r.rows[d as usize].flow_wages.max(1);
            eprintln!(
                "FINDING day {:>2} Ambient: food {}/{} units, visits {}/{}, paid {} + {} ({:.1} % of wages), appetite {:.2}",
                d + 1,
                a.food_bought,
                a.food_wanted,
                a.visits_made,
                a.visits_wanted,
                a.paid_food,
                a.paid_visits,
                100.0 * (a.paid_food + a.paid_visits) as f64 / wages as f64,
                a.appetite
            );
        }
    }
    // Printed findings: the per-capita read (set it beside the 2,000 city's).
    for d in [0u64, 9, 19, DAYS - 1] {
        let (Some(x), Some(&(adults, free, emp, street))) = (r.rows.get(d as usize), r.adults.get(d as usize)) else {
            continue;
        };
        let k = 1000.0 / f64::from(x.population.max(1));
        eprintln!(
            "FINDING day {:>2}: population {}, employed {:.3} of free adults, street {:.3} of {adults} adults, homeless {:.1}/1k, jailed {}, Treasury {:.1}/resident",
            d + 1,
            x.population,
            f64::from(emp) / f64::from(free.max(1)),
            f64::from(street) / f64::from(adults.max(1)),
            f64::from(x.homeless) * k,
            x.jailed,
            x.treasury as f64 / f64::from(x.population.max(1)),
        );
    }
    if let Some(&(_, free, emp, _)) = r.adults.get((DAYS - 1) as usize) {
        let e = f64::from(emp) / f64::from(free.max(1));
        eprintln!(
            "FINDING J24 day {DAYS}: employed {e:.3} of free adults against the 2,000 city's {EMPLOYED_MIN_SHARE} ({}; asserted: the collapse floor {EMPLOYED_FLOOR_20K})",
            if e >= EMPLOYED_MIN_SHARE { "over" } else { "under" }
        );
    }
    let murders = r.kind(EventKind::Murder);
    let pop = r.rows.last().map_or(1, |x| x.population.max(1));
    eprintln!(
        "FINDING Murders {murders} ({:.2}/1k), starvation {:.0}, corps alive {}/{}",
        f64::from(murders) * 1000.0 / f64::from(pop),
        r.sum(0, DAYS, |x| x.deaths_starvation),
        r.corps_alive,
        r.corps_seeded,
    );
    eprintln!("ticks/s {:.0} (two runs in parallel; floor {TPS_FLOOR_20K:.0}, printed)", r.tps);
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "core_20k failures: {failures:?}");
}
