//! Transit phase 1 (docs/TRANSIT.md, plan `~/.claude/plans/transit.md`):
//! the El's network and timetable, the walk-or-ride choice, Full and Coarse
//! rides, fares and fare dodging, closures, power, saves, and the
//! switched-off city.

use citysim::components::ActionInstance;
use citysim::exec::ExecState;
use citysim::goap::{ActionKind, LocationKey, Plan};
use citysim::systems::transit::{self, Refusal};
use citysim::systems::{demography, lod};
use citysim::{
    Brain, Building, BuildingKind, Config, EntityId, GoalKind, Household, Lod, Personality, PlayerCommand, Position,
    TileKind, Wallet, World, TICKS_PER_DAY,
};

/// The shipped city with the El switched on.
fn city_on() -> World {
    let mut cfg = Config::load();
    cfg.transit.enabled = true;
    World::new(42, cfg)
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |x| x.coins)
}

fn treasury(w: &World) -> i64 {
    w.treasury().map_or(0, |t| t.coins)
}

/// An adult whose Home is deep in the Sump (south, west of the Vats), and a
/// Farm in the Vats: a trip the El should win.
fn sump_farmer(w: &World) -> (EntityId, EntityId, EntityId) {
    let farm = *w.buildings_of_kind(BuildingKind::Farm).first().expect("a Farm");
    let who = w
        .citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && w.has::<Wallet>(a) && demography::is_adult(w, a))
        .find(|&a| {
            w.comp::<Household>(a)
                .and_then(|h| h.home)
                .and_then(|h| w.comp::<Building>(h))
                .is_some_and(|b| b.door.y >= 125 && b.door.y <= 135 && b.door.x < 120)
        })
        .expect("a Sump resident near the Vat Line");
    let home = w.comp::<Household>(who).and_then(|h| h.home).expect("home");
    (who, home, farm)
}

/// `who` inside `home` at 08:00 on day 1, decent and solvent, with a plan
/// of one GoTo to `target`.
fn set_out(w: &mut World, who: EntityId, home: EntityId, target: EntityId, lod_: Lod, wallet: i64, law: f32) {
    w.tick = TICKS_PER_DAY + 8 * 60;
    lod::set_lod(w, who, lod_);
    w.leave_building(who);
    w.enter_building(who, home);
    w.comp_mut::<Wallet>(who).expect("wallet").coins = wallet;
    let p = w.comp_mut::<Personality>(who).expect("personality");
    p.lawfulness = law;
    p.courage = 0.5;
    let tick = w.tick;
    let b = w.comp_mut::<Brain>(who).expect("brain");
    b.plan = Some(Plan {
        goal: GoalKind::Idle,
        target: Some(target),
        steps: vec![ActionInstance {
            action: ActionKind::GoTo(LocationKey::TargetHome),
            target: Some(target),
            tile: None,
        }],
        started_tick: tick,
    });
    b.plan_step = 0;
    b.exec = ExecState::Idle;
}

fn exec_tick(w: &mut World) {
    citysim::exec::run(w);
    w.tick += 1;
}

/// Run the executor until `who` is inside `target` (the GoTo done); the tick.
fn run_until_inside(w: &mut World, who: EntityId, target: EntityId, cap: u64) -> u64 {
    for _ in 0..cap {
        exec_tick(w);
        if w.comp::<Position>(who).is_some_and(|p| p.building == Some(target)) {
            return w.tick;
        }
    }
    panic!("never arrived (exec {:?})", w.comp::<Brain>(who).map(|b| b.exec.clone()));
}

// ---------------------------------------------------------------------------
// The network and the timetable
// ---------------------------------------------------------------------------

#[test]
fn test_network_from_the_shipped_lines() {
    let mut w = city_on();
    let text = transit::describe(&mut w);
    println!("{text}");
    let net = &w.transit.net;
    assert_eq!(net.lines.len(), 3);
    assert!(net.stops.len() >= 40, "{} stops", net.stops.len());
    for &s in &net.stops {
        assert_eq!(w.map.tile_at(s), TileKind::Road, "stop {s:?} on a road");
    }
    for line in &net.lines {
        assert!(line.stops.len() >= 8, "{} has {} stops", line.name, line.stops.len());
        assert!(line.offsets.windows(2).all(|p| p[1] > p[0]), "{}: offsets rise", line.name);
    }
    // Every stop reaches every other (the three lines share transfer stops).
    let n = net.stops.len() as u16;
    for s in 0..n {
        for t in 0..n {
            if s != t {
                assert!(net.hop(s, t).ticks < u32::MAX, "{s} -> {t} unreachable");
            }
        }
    }
}

#[test]
fn test_next_departure_follows_the_headway() {
    let w = city_on();
    let line = &w.transit.net.lines[0];
    let (stop, off) = (line.stops[3], u64::from(line.offsets[3]));
    let h = u64::from(w.config.transit.headway);
    let base = 10 * TICKS_PER_DAY;
    let d = transit::next_departure(&w, 0, true, stop, base).expect("calls there");
    assert_eq!(d % h, off % h, "forward departures at the offset");
    assert!(d >= base && d < base + h);
    assert_eq!(transit::next_departure(&w, 0, true, stop, d), Some(d), "a departure now is now");
    assert_eq!(transit::next_departure(&w, 0, true, stop, d + 1), Some(d + h));
    let back = transit::next_departure(&w, 0, false, stop, base).expect("calls there");
    assert_eq!(back % h, (u64::from(line.total) - off) % h, "backward from the far terminus");
}

// ---------------------------------------------------------------------------
// The choice (T8)
// ---------------------------------------------------------------------------

#[test]
fn test_choice_refusals() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    let from = citysim::exec::walk_origin(&w, who).expect("origin");
    let door = w.comp::<Building>(farm).expect("farm").door;
    let t = w.tick;
    let r = transit::choose(&w, who, from, door, t).expect("a ride");
    assert!(r.total < r.walk, "the ride beats the walk: {} vs {}", r.total, r.walk);
    assert!(!r.dodge);
    // A short trip walks.
    let near = citysim::TilePos::new(from.x.saturating_add(3), from.y);
    assert_eq!(transit::choose(&w, who, from, near, t), Err(Refusal::Short));
    // Broke and lawful: walks.
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 0;
    assert_eq!(transit::choose(&w, who, from, door, t), Err(Refusal::Fare));
    // Broke and lawless: jumps the gate.
    w.comp_mut::<Personality>(who).expect("p").lawfulness = 0.2;
    assert!(transit::choose(&w, who, from, door, t).expect("a ride").dodge);
    // No service 01:00-05:00.
    let night = 2 * TICKS_PER_DAY + 2 * 60;
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 50;
    w.comp_mut::<Personality>(who).expect("p").lawfulness = 0.6;
    assert_eq!(transit::choose(&w, who, from, door, night), Err(Refusal::NoRoute));
    // A dangerous night district turns a timid rider back.
    let late = 2 * TICKS_PER_DAY + 22 * 60;
    let r = transit::choose(&w, who, from, door, late).expect("a late ride");
    let d = w.district_of(w.transit.net.stops[usize::from(r.board)]);
    w.districts[d.index()].crime_rate = 100.0;
    w.comp_mut::<Personality>(who).expect("p").courage = 0.0;
    assert_eq!(transit::choose(&w, who, from, door, late), Err(Refusal::Danger));
    // By day the same stop is fine.
    assert!(transit::choose(&w, who, from, door, t).is_ok());
}

#[test]
fn test_closures_cut_lines_and_stops() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    let from = citysim::exec::walk_origin(&w, who).expect("origin");
    let door = w.comp::<Building>(farm).expect("farm").door;
    let t = w.tick;
    let first = transit::choose(&w, who, from, door, t).expect("a ride");
    // The board stop closed: another stop, or no ride, never that one.
    transit::close_stop(&mut w, first.board, 3).expect("a stop");
    if let Ok(r) = transit::choose(&w, who, from, door, t) {
        assert_ne!(r.board, first.board);
    }
    // Every line cut: no ride.
    for l in 0..3 {
        w.push_command(PlayerCommand::CutLine { line: l, days: 2 });
    }
    w.apply_commands();
    transit::ensure_net(&mut w);
    assert_eq!(transit::choose(&w, who, from, door, t), Err(Refusal::NoRoute));
    // Reopened: rides again.
    for l in 0..3 {
        transit::cut_line(&mut w, l, 0).expect("a line");
    }
    transit::close_stop(&mut w, first.board, 0).expect("a stop");
    transit::ensure_net(&mut w);
    assert_eq!(transit::choose(&w, who, from, door, t).map(|r| r.board), Ok(first.board));
    // A curfew closes the district's stops at night only.
    let d = w.district_of(w.transit.net.stops[usize::from(first.board)]);
    w.districts[d.index()].curfew = true;
    assert!(transit::stop_open(&w, first.board, t));
    assert!(!transit::stop_open(&w, first.board, 2 * TICKS_PER_DAY + 23 * 60));
}

// ---------------------------------------------------------------------------
// Rides (T12, T13)
// ---------------------------------------------------------------------------

#[test]
fn test_full_rider_pays_and_beats_the_walk() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    let from = citysim::exec::walk_origin(&w, who).expect("origin");
    let door = w.comp::<Building>(farm).expect("farm").door;
    let walk = citysim::exec::timed_walk(&w, who, from, door);
    let (t0, purse0) = (w.tick, treasury(&w));
    let at = run_until_inside(&mut w, who, farm, 2_000);
    let c = &w.stats.current.transit;
    assert_eq!(c.rides_full, 1, "one ride");
    assert_eq!(c.fares, 1);
    assert_eq!(coins(&w, who), 49, "the fare paid");
    assert_eq!(treasury(&w), purse0 + 1, "to the Treasury");
    assert!(w.rides.is_empty(), "the ride ended at the alight stop");
    assert!(at - t0 < walk, "rode in {} ticks against a {walk}-tick walk", at - t0);
    println!("rode in {} ticks against a {walk}-tick walk", at - t0);
}

#[test]
fn test_coarse_rider_arrives_on_the_route_estimate() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Coarse, 50, 0.6);
    let from = citysim::exec::walk_origin(&w, who).expect("origin");
    let door = w.comp::<Building>(farm).expect("farm").door;
    let t0 = w.tick;
    let r = transit::choose(&w, who, from, door, t0).expect("a ride");
    let at = run_until_inside(&mut w, who, farm, 2_000);
    assert_eq!(w.stats.current.transit.rides_coarse, 1);
    // One tick per leg of executor slack, at most.
    assert!((at - t0).abs_diff(r.total) <= 3, "Coarse {} vs the route's {}", at - t0, r.total);
}

#[test]
fn test_dodger_rides_free_and_commits_fare_evasion() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 0, 0.05);
    let crimes0: u32 = w.districts.iter().map(|d| u32::from(d.crimes_today)).sum();
    let purse0 = treasury(&w);
    run_until_inside(&mut w, who, farm, 2_000);
    let c = &w.stats.current.transit;
    assert_eq!(c.rides_full, 1);
    assert_eq!(c.fare_dodges, 1);
    assert_eq!(c.fares, 0);
    assert_eq!(treasury(&w), purse0, "no fare");
    let crimes: u32 = w.districts.iter().map(|d| u32::from(d.crimes_today)).sum();
    assert_eq!(crimes, crimes0 + 1, "a FareEvasion on the district's books");
}

#[test]
fn test_abort_and_death_end_the_ride() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    exec_tick(&mut w);
    assert!(w.rides.contains_key(&who), "the ride is filed at the GoTo's start");
    w.abort_plan(who);
    assert!(w.rides.is_empty());
    // A ride whose plan was replaced is dropped at the next leg, never resumed.
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    exec_tick(&mut w);
    assert!(w.rides.contains_key(&who));
    if let Some(p) = w.comp_mut::<Brain>(who).and_then(|b| b.plan.as_mut()) {
        p.started_tick += 1;
    }
    for _ in 0..600 {
        exec_tick(&mut w);
        if w.rides.is_empty() {
            break;
        }
    }
    assert!(w.rides.is_empty(), "the stale ride dropped");
    assert_eq!(w.stats.current.transit.rides_full, 0, "never boarded");
}

#[test]
fn test_save_mid_ride_runs_on_identically() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    // Onto the platform or aboard.
    for _ in 0..400 {
        exec_tick(&mut w);
        if matches!(w.comp::<Brain>(who).map(|b| &b.exec), Some(ExecState::Ride { .. })) {
            break;
        }
    }
    assert!(matches!(w.comp::<Brain>(who).map(|b| &b.exec), Some(ExecState::Ride { .. })), "riding");
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(back.transit.net.stops, w.transit.net.stops, "the network rebuilt on load");
    for _ in 0..300 {
        exec_tick(&mut w);
        exec_tick(&mut back);
    }
    assert_eq!(citysim::save::to_ron(&w), citysim::save::to_ron(&back));
}

#[test]
fn test_power_is_paid_at_midnight() {
    let mut w = city_on();
    w.tick = 3 * TICKS_PER_DAY;
    let purse0 = treasury(&w);
    let identity0 = citysim::systems::econ::identity(&w);
    transit::run(&mut w);
    let power = 3 * w.config.transit.power_per_line_day;
    assert_eq!(w.stats.current.transit.transit_power, power);
    assert_eq!(treasury(&w), purse0 - power, "the Treasury pays the power");
    assert_eq!(citysim::systems::econ::identity(&w), identity0, "to the World: no coin made or lost");
    // A cut line draws none.
    w.tick = 4 * TICKS_PER_DAY - 1;
    transit::cut_line(&mut w, 1, 5).expect("a line");
    w.tick = 4 * TICKS_PER_DAY;
    transit::run(&mut w);
    assert_eq!(w.stats.current.transit.transit_power, power + 2 * w.config.transit.power_per_line_day);
}

// ---------------------------------------------------------------------------
// The switched-off city
// ---------------------------------------------------------------------------

/// With `[transit] enabled = false` nothing of the El reaches the world: no
/// network, no ride, no column, and nothing in the save (so the save is
/// the one a world built before transit existed would write).
#[test]
fn test_switched_off_city_is_untouched() {
    let mut cfg = Config::load();
    cfg.transit.enabled = false;
    let mut w = World::new(42, cfg);
    assert!(w.transit.net.stops.is_empty());
    w.run_ticks(TICKS_PER_DAY + 1);
    assert!(w.rides.is_empty());
    assert!(w.transit.is_idle());
    assert!(w.stats.current.transit.is_zero());
    assert!(w.stats.history.iter().all(|r| r.transit.is_zero()));
    let ron = citysim::save::to_ron(&w);
    assert!(!ron.contains("transit:"), "no transit config or state in the save");
    assert!(!ron.contains("rides:"), "no rides in the save");
    assert!(!ron.contains("FareEvasion"), "no fare evasion anywhere");
}

/// A save hashed with `ticks_per_sec` (the runner's wall clock) zeroed.
fn off_city_hash(ron: &str) -> String {
    let mut out = String::with_capacity(ron.len());
    let mut rest = ron;
    while let Some(i) = rest.find("ticks_per_sec:") {
        out.push_str(&rest[..i]);
        out.push_str("ticks_per_sec:0.0");
        rest = &rest[i + "ticks_per_sec:".len()..];
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
        rest = &rest[end..];
    }
    out.push_str(rest);
    blake3::hash(out.as_bytes()).to_hex().to_string()
}

/// The off city is byte-identical to main before transit (`d771073`): the
/// save after one day, hashed with the wall clock masked. Main's hash was
/// pinned from `citysim-cli run --days 2 --seed 42 --save-at 1441` built at
/// `d771073`. Ignored: any behaviour change on main moves the pin. To check
/// against another main, set `CITYSIM_MAIN_SAVE` to that main's
/// `42-1441.ron` (same command) and run it: the file is hashed the same way.
#[test]
#[ignore]
fn test_switched_off_city_matches_main() {
    let mut cfg = Config::load();
    cfg.transit.enabled = false;
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 1);
    let hash = off_city_hash(&citysim::save::to_ron(&w));
    println!("off-city hash {hash}");
    let want = match std::env::var("CITYSIM_MAIN_SAVE") {
        Ok(path) => off_city_hash(&std::fs::read_to_string(path).expect("main's save")),
        Err(_) => MAIN_OFF_HASH.to_string(),
    };
    println!("main's hash   {want}");
    assert_eq!(hash, want);
}

/// See `test_switched_off_city_matches_main`.
const MAIN_OFF_HASH: &str = "3229f4eae7b9bba85b0a92e1e83cff02882bf2f7db25c982b4340ec1048073c1";
