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
        adults: Vec::new(),
        dregs: Vec::new(),
        no_net: econ::wages_on(&w) && !citysim::systems::policy::net_enacted(&w),
        sickness: citysim::systems::health::on(&w),
        transit: w.config.transit.enabled,
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
        for k in ContractKind::ALL {
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
    let per_120 = if r.sickness { STARVATION_MAX_120_SICK } else { STARVATION_MAX_120 };
    let starv_max = per_120 * n / 120.0;
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
            e >= EMPLOYED_MIN_SHARE,
            format!("J24 employed {emp} of {free} free adults ({e:.3}) on day {d} >= {EMPLOYED_MIN_SHARE}"),
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
        if r.corps_alive < CORPS_ALIVE_MIN {
            failures.push(format!("seed {}: seeded corps alive {}/{}", r.seed, r.corps_alive, r.corps_seeded));
        }
        for m in ["stale ride", "ride past arrival"] {
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
    if r.corps_alive < CORPS_ALIVE_MIN {
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
