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

/// Review item 7 (`/code-review high 7324e60..dfaca2e`): a loop line calls
/// at its terminus twice (first and last). A ride the table starts from the
/// last call departs at that call's offset, not the first's: the route's
/// departure and arrival are the timetable's, and a body boards the train
/// the route planned.
#[test]
fn test_loop_line_departs_from_the_call_it_boards() {
    let mut cfg = Config::load();
    cfg.transit.enabled = true;
    let mut lines = cfg.transit.lines.clone();
    lines.truncate(1);
    lines[0].name = "Loop".into();
    lines[0].waypoints = vec![[7, 167], [231, 167], [231, 127], [7, 127], [7, 167]];
    cfg.transit.lines = lines;
    // A headway that does not divide the loop's length, so the two calls at
    // the terminus depart at different minutes.
    let mut w = World::new(42, cfg);
    let total = w.transit.net.lines[0].total;
    let h = (7..30).find(|h| !total.is_multiple_of(*h)).expect("a headway");
    w.config.transit.headway = h;
    let line = w.transit.net.lines[0].clone();
    let n = line.stops.len();
    assert!(n >= 4, "{n} stops");
    assert_eq!(line.stops[0], line.stops[n - 1], "a loop: the terminus is called twice");
    let (s0, near_end) = (line.stops[0], line.stops[n - 2]);
    let hop = w.transit.net.hop(s0, near_end);
    assert!(!hop.forward, "the short way is back from the last call");
    assert_eq!(hop.ticks, line.offsets[n - 1] - line.offsets[n - 2], "one hop back");
    // The last call's backward departures leave at `total - offsets[n-1]` = 0.
    let t = 10 * TICKS_PER_DAY + 9 * 60 + 1;
    let from = w.transit.net.stops[usize::from(s0)];
    let to = w.transit.net.stops[usize::from(near_end)];
    let who = w.citizens().into_iter().find(|&a| demography::is_adult(&w, a)).expect("an adult");
    let r = transit::route(&w, who, from, to, t).expect("a route");
    assert_eq!((r.board, r.alight), (s0, near_end));
    let off = u64::from(line.total - line.offsets[n - 1]);
    assert_eq!(r.depart % u64::from(h), off % u64::from(h), "the last call's departure");
    let first_call = u64::from(line.total - line.offsets[0]);
    assert_ne!(first_call % u64::from(h), off % u64::from(h), "the calls differ");
}

/// Review item 8 (`/code-review high 7324e60..dfaca2e`): a cut line runs
/// out at its tick, any time of day, and the ride table is rebuilt that
/// tick, not at the next `try_ride` (`commute_ticks` reads `&World`). Every
/// agent is Statistical here, so no body's ride rebuilds it by chance.
#[test]
fn test_cut_line_expiry_rebuilds_the_table_at_once() {
    let mut cfg = Config::load();
    cfg.transit.enabled = true;
    cfg.lod.force = Some(Lod::Statistical);
    let mut w = World::new(42, cfg);
    let (a, b) = (citysim::components::TilePos::new(20, 128), citysim::components::TilePos::new(228, 30));
    let open_ticks = transit::commute_ticks(&w, a, b);
    w.tick = TICKS_PER_DAY + 8 * 60 + 13;
    for l in 0..3 {
        transit::cut_line(&mut w, l, 1).expect("a line");
    }
    let cut_ticks = transit::commute_ticks(&w, a, b);
    assert!(cut_ticks > open_ticks, "the cut takes the ride away ({open_ticks} -> {cut_ticks})");
    // The cut's last tick, then the tick it runs out (not an hour's start,
    // not midnight).
    w.tick = 2 * TICKS_PER_DAY + 8 * 60 + 13;
    for id in w.citizens() {
        lod::set_lod(&mut w, id, Lod::Statistical);
    }
    w.tick();
    assert_eq!(w.transit.net.open, vec![true; 3], "the table holds the reopened lines");
    assert_eq!(transit::commute_ticks(&w, a, b), open_ticks, "the ride is back");
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
// Phase 2: the shared distance, the departure gate, off-screen riders
// ---------------------------------------------------------------------------

fn door(w: &World, b: EntityId) -> citysim::TilePos {
    w.comp::<Building>(b).expect("a building").door
}

/// The structural commute rides when the ride wins and is the walk
/// otherwise: a Sump Home to a Vats Farm, a short hop, the El off, the
/// board stops closed.
#[test]
fn test_commute_ticks_rides_when_it_wins() {
    let mut w = city_on();
    let (_, home, farm) = sump_farmer(&w);
    let (hd, fd) = (door(&w, home), door(&w, farm));
    let walk = u64::from(hd.manhattan(fd)) * w.config.exec.move_ticks_full;
    let ride = transit::commute_ticks(&w, hd, fd);
    assert!(ride + 30 < walk, "the El wins the Sump to the Vats: {ride} vs a {walk}-tick walk");
    assert_eq!(citysim::systems::life::commute_cost(&w, hd, fd), ride, "commute_cost is the shared distance");
    let near = citysim::TilePos::new(hd.x.saturating_add(5), hd.y);
    assert_eq!(transit::commute_ticks(&w, hd, near), u64::from(hd.manhattan(near)) * w.config.exec.move_ticks_full);
    // God closes the Home's nearest stops: another stop or the walk, never faster.
    for &s in w.transit.net.near(hd).to_vec().iter().take(3) {
        transit::close_stop(&mut w, s, 2).expect("a stop");
    }
    assert!(transit::commute_ticks(&w, hd, fd) >= ride);
    w.config.transit.enabled = false;
    assert_eq!(transit::commute_ticks(&w, hd, fd), walk, "off: the walk exactly");
}

/// An employed adult with a Home, no vehicle and a home -> work trip the El
/// wins by `margin` ticks setting out at 06:00 on day 1: (who, home,
/// employer).
fn rider_worker(w: &World, margin: u64) -> (EntityId, EntityId, EntityId) {
    use citysim::{Job, Role};
    let t = TICKS_PER_DAY + 6 * 60;
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && demography::is_adult(w, a))
        .filter(|&a| citysim::systems::vehicles::vehicle_for_trip(w, a).is_none())
        .find_map(|a| {
            let job = w.comp::<Job>(a).filter(|j| j.role != Role::Guard && j.shifts.len() == 1)?;
            let employer = job.employer?;
            let home = w.comp::<Household>(a).and_then(|h| h.home)?;
            let (hd, wd) = (w.outside_door(w.comp::<Building>(home)?), door(w, employer));
            let walk = citysim::exec::timed_walk(w, a, hd, wd);
            let r = transit::choose(w, a, hd, wd, t).ok()?;
            (r.total + margin <= walk).then_some((a, home, employer))
        })
        .expect("a worker the El carries home to work")
}

/// T16: the departure gate reads the trip as it would be made, so a rider
/// at home leaves later than the walk would; with the El off the same body
/// at the same tick sets out.
#[test]
fn test_rider_leaves_later_for_work() {
    use citysim::Job;
    let mut w = city_on();
    let (who, home, employer) = rider_worker(&w, 40);
    let job = w.comp::<Job>(who).cloned().expect("a job");
    let (start, _) = job.shifts[0];
    // A workday whose shift is still pending.
    let day = (1..8u64).find(|&d| citysim::exec::routine::is_workday(d as i64)).expect("a workday");
    w.leave_building(who);
    w.enter_building(who, home);
    if let Some(b) = w.comp_mut::<Brain>(who) {
        b.clear_plan();
    }
    if let Some(j) = w.comp_mut::<Job>(who) {
        j.last_shift_day = None;
    }
    let (hd, wd) = (w.outside_door(w.comp::<Building>(home).expect("home")), door(&w, employer));
    let walk = citysim::exec::timed_walk(&w, who, hd, wd);
    // The walker's gate opened 10 ticks ago.
    let arrive_by = day * TICKS_PER_DAY + u64::from(start) - 60;
    w.tick = arrive_by - walk + 10;
    let est = transit::estimate_by(&w, who, hd, wd, arrive_by);
    assert!(est + 30 < walk, "the rider's estimate {est} vs the walk {walk}");
    assert!(!citysim::exec::routine::must_leave_for_work(&w, who), "a rider stays in bed");
    // The deadline's estimate holds still while the body waits (the clock's
    // own moved with the timetable and the 05:00 start of service).
    for t in [arrive_by - walk - 200, arrive_by - walk, arrive_by - est - 5] {
        w.tick = t;
        assert_eq!(transit::estimate_by(&w, who, hd, wd, arrive_by), est, "at {t}");
    }
    w.tick = arrive_by - walk + 10;
    w.config.transit.enabled = false;
    assert!(citysim::exec::routine::must_leave_for_work(&w, who), "a walker sets out");
    w.config.transit.enabled = true;
    // ... and the rider sets out once its own gate opens.
    w.tick = arrive_by - est;
    assert!(citysim::exec::routine::must_leave_for_work(&w, who), "the rider sets out at its time");
    w.tick -= 1;
    assert!(!citysim::exec::routine::must_leave_for_work(&w, who), "not a tick before");
}

/// T13: a Statistical worker the El carries pays two fares and counts two
/// rides at the shift's first hour, and nothing in the next hour.
#[test]
fn test_statistical_rider_pays_two_fares_at_the_shift_start() {
    use citysim::Job;
    let mut w = city_on();
    let (who, _, _) = rider_worker(&w, 40);
    lod::set_lod(&mut w, who, Lod::Statistical);
    // The model's own answer (the learned commute share pinned off).
    w.stat_table.as_mut().expect("a table").p_commute_ride = None;
    let job = w.comp::<Job>(who).cloned().expect("a job");
    let (start, _) = job.shifts[0];
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 50;
    w.comp_mut::<Personality>(who).expect("p").lawfulness = 0.6;
    w.tick = TICKS_PER_DAY + u64::from(start) + 7;
    let (purse0, id0) = (treasury(&w), citysim::systems::econ::identity(&w));
    let c0 = w.stats.current.transit.clone();
    transit::stat_commute(&mut w, who, &job, None);
    let c = &w.stats.current.transit;
    assert_eq!(c.rides_stat, c0.rides_stat + 2, "out and back");
    assert_eq!(c.fares, c0.fares + 2);
    assert_eq!(coins(&w, who), 48);
    assert_eq!(treasury(&w), purse0 + 2, "to the Treasury");
    assert_eq!(citysim::systems::econ::identity(&w), id0);
    assert_eq!(w.transit.load.values().map(|h| h.iter().map(|&n| u32::from(n)).sum::<u32>()).sum::<u32>(), 2);
    // The next hour of the same shift: nothing.
    w.tick += 60;
    transit::stat_commute(&mut w, who, &job, None);
    assert_eq!(w.stats.current.transit.rides_stat, c0.rides_stat + 2);
    // Broke and lawful off screen: the walk, no ride.
    w.tick = 2 * TICKS_PER_DAY + u64::from(start) + 7;
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 0;
    transit::stat_commute(&mut w, who, &job, None);
    assert_eq!(w.stats.current.transit.rides_stat, c0.rides_stat + 2);
}

/// Phase 3 (parity: Full's dodgers are arrested, 70 % of the all-Full
/// city's sentences): an off-screen dodger is reported at the table's
/// `p_dodge_caught` and comes on screen for the arrest; at 0, never.
#[test]
fn test_statistical_dodger_is_caught_at_the_learned_rate() {
    use citysim::Job;
    for (p, caught) in [(0.0f32, false), (1.0, true)] {
        let mut w = city_on();
        let (who, _, _) = rider_worker(&w, 40);
        lod::set_lod(&mut w, who, Lod::Statistical);
        w.stat_table.as_mut().expect("a table").p_dodge_caught = Some(p);
        w.stat_table.as_mut().expect("a table").p_commute_ride = None;
        let job = w.comp::<Job>(who).cloned().expect("a job");
        w.comp_mut::<Wallet>(who).expect("wallet").coins = 0;
        w.comp_mut::<Personality>(who).expect("p").lawfulness = 0.05;
        w.tick = TICKS_PER_DAY + u64::from(job.shifts[0].0) + 7;
        transit::stat_commute(&mut w, who, &job, None);
        assert_eq!(w.stats.current.transit.rides_stat, 2, "p {p}: rode through the gate");
        let lod_now = w.comp::<Brain>(who).expect("brain").lod;
        assert_eq!(lod_now != Lod::Statistical, caught, "p {p}: on screen for the arrest");
    }
}

/// Parity (commute rides): an off-screen commute rides on the Full city's
/// share for the Home-door model's answer (`p_commute_ride`): a rider the
/// model carries walks at a ride share of 0; a commute the model walks
/// (here: under `min_ride_tiles` from the Home door) rides the Home-to-work
/// route at a share of 1, paying its two fares, and not at 0.
#[test]
fn test_statistical_commute_rides_at_the_learned_share() {
    use citysim::Job;
    // (model rides, cells, rode)
    for (model_rides, cells, rode) in
        [(true, [0.0f32, 0.0], false), (true, [0.0, 1.0], true), (false, [1.0, 0.0], true), (false, [0.0, 1.0], false)]
    {
        let mut w = city_on();
        let (who, _, _) = rider_worker(&w, 40);
        lod::set_lod(&mut w, who, Lod::Statistical);
        if !model_rides {
            w.config.transit.min_ride_tiles = u32::MAX;
        }
        w.stat_table.as_mut().expect("a table").p_commute_ride = Some(cells);
        let job = w.comp::<Job>(who).cloned().expect("a job");
        w.comp_mut::<Wallet>(who).expect("wallet").coins = 50;
        w.comp_mut::<Personality>(who).expect("p").lawfulness = 0.6;
        w.tick = TICKS_PER_DAY + u64::from(job.shifts[0].0) + 7;
        let c0 = w.stats.current.transit.clone();
        transit::stat_commute(&mut w, who, &job, None);
        let c = &w.stats.current.transit;
        assert_eq!(c.stat_commutes, c0.stat_commutes + 1, "judged");
        let n = u32::from(rode);
        assert_eq!(c.stat_commute_rides, c0.stat_commute_rides + n, "model {model_rides}, {cells:?}");
        assert_eq!(c.rides_stat, c0.rides_stat + 2 * n);
        assert_eq!(coins(&w, who), 50 - 2 * i64::from(n), "two fares a riding workday");
    }
}

/// Parity (arrests): an off-screen adult rides the El off the commute on
/// its row's `p_errand_ride`: a payer pays the fare; a lawless broke rider
/// jumps the gate, is counted, and at `p_dodge_caught` 1 is reported and
/// comes on screen for the arrest. A table without the rate: no ride.
#[test]
fn test_statistical_errand_ride_pays_or_jumps_the_gate() {
    for (rated, payer) in [(true, true), (true, false), (false, false)] {
        let mut w = city_on();
        w.run_ticks(TICKS_PER_DAY + 10 * 60);
        let who = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| demography::is_adult(&w, a) && !w.has::<citysim::Sentence>(a))
            .expect("a Statistical adult");
        w.comp_mut::<Wallet>(who).expect("wallet").coins = if payer { 50 } else { 0 };
        w.comp_mut::<Personality>(who).expect("p").lawfulness = if payer { 0.6 } else { 0.05 };
        {
            let t = w.stat_table.as_mut().expect("a table");
            t.p_errand_ride = rated.then_some([1.0; citysim::STAT_ROWS]);
            t.p_dodge_caught = Some(1.0);
        }
        let c0 = w.stats.current.transit.clone();
        let before = coins(&w, who);
        for _ in 0..60 {
            lod::run_statistical(&mut w);
            w.tick += 1;
        }
        let c = &w.stats.current.transit;
        let mine = c.stat_errand_rides > c0.stat_errand_rides;
        assert_eq!(mine, rated, "rated {rated}: an errand ride");
        if !rated {
            continue;
        }
        let lod_now = w.comp::<Brain>(who).expect("brain").lod;
        if payer {
            assert!(c.fares > c0.fares, "the fare paid");
            assert!(coins(&w, who) < before, "from the rider's purse");
            assert_eq!(lod_now, Lod::Statistical);
        } else {
            assert!(c.fare_dodges > c0.fare_dodges, "the gate jumped");
            assert_ne!(lod_now, Lod::Statistical, "reported, on screen for the arrest");
        }
    }
}

/// The tick at or after `t` when `who`'s Statistical hour runs (its slot);
/// the world's clock set there, and every other agent of that slot given a
/// body, so `run_statistical` runs `who` alone.
fn alone_at_slot(w: &mut World, who: EntityId, t: u64) {
    w.tick = t + (u64::from(who.index) + 60 - t % 60) % 60;
    for id in w.stat_slots.at(w.tick).to_vec() {
        if id != who {
            lod::set_lod(w, id, Lod::Coarse);
        }
    }
}

/// Review item 1 (`/code-review high 7324e60..dfaca2e`): a dodger caught at
/// the commute's gate comes on screen for the arrest, and the rest of the
/// Statistical hour is not run on it (no meal from the hourly draw, no
/// grocery run, no rolls), as after the errand ride's catch.
#[test]
fn test_caught_commuter_leaves_the_statistical_hour() {
    use citysim::{Inventory, Job, Needs};
    let mut w = city_on();
    let (who, _, _) = rider_worker(&w, 40);
    lod::set_lod(&mut w, who, Lod::Statistical);
    {
        let t = w.stat_table.as_mut().expect("a table");
        t.p_dodge_caught = Some(1.0);
        t.p_commute_ride = None;
        t.p_attend = None;
        // Every hour with a meal at hand eats (the draw this hour would).
        t.p_meal = Some(vec![1.0; citysim::world::MEAL_CELLS_SPLIT]);
    }
    let job = w.comp::<Job>(who).cloned().expect("a job");
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 0;
    w.comp_mut::<Personality>(who).expect("p").lawfulness = 0.05;
    w.comp_mut::<Inventory>(who).expect("inv").food = 2;
    w.comp_mut::<Needs>(who).expect("needs").hunger = 0.3;
    alone_at_slot(&mut w, who, TICKS_PER_DAY + u64::from(job.shifts[0].0));
    lod::run_statistical(&mut w);
    assert_eq!(w.stats.current.transit.rides_stat, 2, "rode through the gate");
    assert_ne!(w.comp::<Brain>(who).expect("brain").lod, Lod::Statistical, "on screen for the arrest");
    assert_eq!(w.comp::<Inventory>(who).expect("inv").food, 2, "no off-screen meal after the catch");
}

/// Review item 2 (`/code-review high 7324e60..dfaca2e`): attendance is
/// rolled before the commute: a no-show (no hour of the shift worked) does
/// not ride or pay; at full attendance the same worker rides.
#[test]
fn test_statistical_no_show_does_not_ride() {
    use citysim::Job;
    for (attend, rides) in [(0.0f32, 0u32), (1.0, 2)] {
        let mut w = city_on();
        let (who, _, _) = rider_worker(&w, 40);
        lod::set_lod(&mut w, who, Lod::Statistical);
        let job = w.comp::<Job>(who).cloned().expect("a job");
        {
            let t = w.stat_table.as_mut().expect("a table");
            t.p_commute_ride = None;
            let cells: std::collections::BTreeMap<String, f32> = [(format!("{:?}", job.role), attend)].into();
            t.p_attend = Some(cells.into());
        }
        w.comp_mut::<Wallet>(who).expect("wallet").coins = 50;
        w.comp_mut::<Personality>(who).expect("p").lawfulness = 0.6;
        alone_at_slot(&mut w, who, TICKS_PER_DAY + u64::from(job.shifts[0].0));
        let fares = w.stats.current.transit.fares;
        lod::run_statistical(&mut w);
        assert_eq!(w.stats.current.transit.rides_stat, rides, "attendance {attend}");
        assert_eq!(w.stats.current.transit.fares - fares, i64::from(rides), "attendance {attend}: fares");
    }
}

/// Review item 6 (`/code-review high 7324e60..dfaca2e`): an exec's office
/// gate reads the trip to arrive by the office's deadline, not the trip
/// setting out now (the now-based estimate flips between the ride and the
/// walk as the next departure's wait shrinks, so the gate opened and shut).
/// Over the morning, every exec in every Home: once open, it stays open.
#[test]
fn test_exec_office_gate_holds_once_open() {
    use citysim::components::Corp;
    let mut w = city_on();
    let day = (1..8u64).find(|&d| citysim::exec::routine::is_workday(d as i64)).expect("a workday");
    let (s, _) = w.config.life.exec_shift;
    let execs: Vec<EntityId> = w
        .corps()
        .into_iter()
        .filter_map(|c| w.comp::<Corp>(c).and_then(|cc| cc.exec))
        .filter(|&e| !w.has::<citysim::Job>(e) && w.has::<Brain>(e))
        .collect();
    assert!(!execs.is_empty(), "an exec");
    let homes: Vec<EntityId> = w.buildings_of_kind(BuildingKind::Home).iter().copied().step_by(7).take(40).collect();
    let mut opened = 0;
    for &e in &execs {
        if let Some(b) = w.comp_mut::<Brain>(e) {
            b.clear_plan();
            b.office_day = None;
        }
        for &h in &homes {
            w.leave_building(e);
            w.enter_building(e, h);
            let mut was_open = false;
            for t in day * TICKS_PER_DAY + u64::from(s) - 240..day * TICKS_PER_DAY + u64::from(s) {
                w.tick = t;
                let open = citysim::systems::life::exec_day_pending(&w, e).is_some();
                assert!(open || !was_open, "exec {e:?} from Home {h:?}: the gate shut again at {t}");
                was_open |= open;
            }
            opened += usize::from(was_open);
        }
    }
    assert!(opened > 0, "the gate opened");
}

/// Review item 3 (`/code-review high 7324e60..dfaca2e`): an off-screen
/// errand ride is gated as a body's boarding: no ride out of service
/// (01:00-05:00), with every line cut, or with every stop closed; the
/// control rides.
#[test]
fn test_statistical_errand_ride_is_gated_like_a_boarding() {
    for case in ["control", "out of service", "lines cut", "stops closed"] {
        let mut w = city_on();
        let hour = if case == "out of service" { 2 } else { 10 };
        w.run_ticks(TICKS_PER_DAY + hour * 60);
        {
            let t = w.stat_table.as_mut().expect("a table");
            t.p_errand_ride = Some([1.0; citysim::STAT_ROWS]);
            t.p_dodge_caught = Some(0.0);
        }
        match case {
            "lines cut" => {
                for l in 0..3 {
                    transit::cut_line(&mut w, l, 2).expect("a line");
                }
            }
            "stops closed" => {
                for s in 0..w.transit.net.stops.len() as u16 {
                    transit::close_stop(&mut w, s, 2).expect("a stop");
                }
            }
            _ => {}
        }
        let c0 = w.stats.current.transit.stat_errand_rides;
        for _ in 0..60 {
            lod::run_statistical(&mut w);
            w.tick += 1;
        }
        let rides = w.stats.current.transit.stat_errand_rides - c0;
        assert_eq!(rides > 0, case == "control", "{case}: {rides} errand rides");
    }
}

/// The app's lerp: a rider aboard is on the track between its board and
/// alight stops, nowhere on the platform.
#[test]
fn test_aboard_rider_is_on_the_track() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    let mut seen = 0;
    for _ in 0..1_000 {
        exec_tick(&mut w);
        let Some(ride) = w.rides.get(&who).cloned() else { continue };
        let Some(ExecState::Ride { depart, arrive_tick, .. }) = w.comp::<Brain>(who).map(|b| b.exec.clone()) else {
            continue;
        };
        if w.tick >= arrive_tick {
            continue;
        }
        let at = transit::aboard_at(&w, who);
        if w.tick < depart {
            assert_eq!(at, None, "on the platform");
            continue;
        }
        let (x, y) = at.expect("aboard");
        let line = &w.transit.net.lines[usize::from(ride.line)];
        let (lo_x, hi_x, lo_y, hi_y) = line
            .stops
            .iter()
            .map(|&s| w.transit.net.stops[usize::from(s)])
            .fold((u16::MAX, 0u16, u16::MAX, 0u16), |(a, b, c, d), p| (a.min(p.x), b.max(p.x), c.min(p.y), d.max(p.y)));
        assert!(x >= f32::from(lo_x) && x <= f32::from(hi_x) && y >= f32::from(lo_y) && y <= f32::from(hi_y));
        seen += 1;
    }
    assert!(seen > 0, "rode");
}

/// Phase 3 (the behaviour tier's d90 gang leader, seed 42: jumped the gate
/// every morning, arrested by 11:00, ten hours in the cells, out at 22:00,
/// four days running, his purse full): a rider arrested in the last
/// `dodge_deterred_days` pays if he can; a broke one still jumps.
#[test]
fn test_an_arrest_deters_the_next_dodge() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.05);
    let from = citysim::exec::walk_origin(&w, who).expect("origin");
    let door = w.comp::<Building>(farm).expect("farm").door;
    let t = w.tick;
    assert!(transit::choose(&w, who, from, door, t).expect("a ride").dodge, "the lawless never pay");
    w.remember(who, citysim::MemoryKind::WasArrested, None, 0.7, -0.5, false);
    assert!(!transit::choose(&w, who, from, door, t).expect("a ride").dodge, "fresh from the cells: pays");
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 0;
    assert!(transit::choose(&w, who, from, door, t).expect("a ride").dodge, "broke: jumps anyway");
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 50;
    w.tick += u64::from(w.config.transit.dodge_deterred_days) * TICKS_PER_DAY + 1;
    assert!(transit::choose(&w, who, from, door, w.tick).expect("a ride").dodge, "the lesson fades");
}

/// Phase 3 (core_sanity's ride check, seed 43): a plan replaced without an
/// abort (a goal interrupt mid-walk) leaves no ride filed behind: the next
/// tick's sweep drops it.
#[test]
fn test_replaced_plan_leaves_no_stale_ride() {
    let mut w = city_on();
    let (who, home, farm) = sump_farmer(&w);
    set_out(&mut w, who, home, farm, Lod::Full, 50, 0.6);
    exec_tick(&mut w);
    assert!(w.rides.contains_key(&who), "the ride is filed");
    // A new plan, as `plan::run` writes one on an interrupt.
    let tick = w.tick;
    if let Some(b) = w.comp_mut::<Brain>(who) {
        b.plan = Some(Plan { goal: GoalKind::Idle, target: None, steps: Vec::new(), started_tick: tick });
        b.plan_step = 0;
        b.exec = ExecState::Idle;
    }
    transit::run(&mut w);
    assert!(!w.rides.contains_key(&who), "the stale ride was dropped");
}

/// Phase 3, the money circuit: the city operator spends its fares in the
/// city. The trailing fares less a day's power pay for maintenance crews
/// (Sanitation posts), the reconciler hires them on top of the civic
/// headcount, and a line with no fares keeps none.
#[test]
fn test_fares_pay_for_maintenance_crews() {
    let mut w = city_on();
    w.tick = 3 * TICKS_PER_DAY;
    assert_eq!(transit::staff_target(&w), 0, "no fares yet, no crews");
    let wage = citysim::systems::treasury::sweeper_wage(&w);
    let power = 3 * w.config.transit.power_per_line_day;
    for _ in 0..14 {
        let mut row = citysim::DayRow::new(0);
        row.transit.fares = power + 10 * wage;
        w.stats.history.push_back(row);
    }
    assert_eq!(transit::staff_target(&w), 10, "the fares past the power, over a sweeper's wage");
    let before = citysim::systems::districts::sweepers(&w).len();
    transit::run(&mut w);
    assert_eq!(w.transit.staff, 10);
    assert_eq!(w.stats.current.transit.el_staff, 10);
    citysim::systems::districts::reconcile_sanitation(&mut w);
    let after = citysim::systems::districts::sweepers(&w).len();
    assert!(after > before, "the crews are hired ({before} -> {after})");
    // Fares dry up: the crews go at the civic pace, not at once.
    for _ in 0..14 {
        w.stats.history.push_back(citysim::DayRow::new(0));
    }
    w.tick += TICKS_PER_DAY;
    transit::run(&mut w);
    assert_eq!(w.transit.staff, 10u16.saturating_sub(u16::from(w.config.treasury.civic_pace)));
}

/// The El crews' cap: however rich the fares, the crews stop at
/// `staff_per_line` a line open (a real metro staffs 0.2-0.8 % of its
/// population; uncapped, the fares paid ~250 crews on seed 42), and a cut
/// line stands its crews down with its power. The fares stay the upper bound.
#[test]
fn test_crews_capped_per_open_line() {
    let mut w = city_on();
    w.tick = 3 * TICKS_PER_DAY;
    let per = w.config.transit.staff_per_line;
    assert!(per > 0, "the shipped city caps the crews");
    let lines = w.config.transit.lines.len() as u16;
    let wage = citysim::systems::treasury::sweeper_wage(&w);
    for _ in 0..14 {
        let mut row = citysim::DayRow::new(0);
        row.transit.fares = i64::from(lines) * w.config.transit.power_per_line_day + 100 * wage;
        w.stats.history.push_back(row);
    }
    assert_eq!(transit::staff_target(&w), lines * per, "a hundred crews' fares hire the cap");
    transit::run(&mut w);
    assert_eq!(w.stats.current.transit.el_staff, u32::from(lines * per));
    transit::cut_line(&mut w, 0, 3).expect("a line");
    assert_eq!(transit::staff_target(&w), (lines - 1) * per, "a cut line's crews stand down");
    transit::cut_line(&mut w, 0, 0).expect("reopened");
    w.config.transit.staff_per_line = u16::MAX;
    assert_eq!(transit::staff_target(&w), 100, "uncapped: the fares past the power, over a sweeper's wage");
}

/// T14 (plan D9): a Statistical worker's attendance is its role's row in its
/// commute band when the table learned one, the role's row otherwise; a line
/// cut that moves the commute into a longer band moves the attendance with
/// no new rule.
#[test]
fn test_attendance_reads_the_commute_band() {
    use citysim::Job;
    let mut w = city_on();
    let (who, _, _) = rider_worker(&w, 40);
    let role = w.comp::<Job>(who).expect("a job").role;
    let band = transit::commute_band(&w, who).expect("housed and employed");
    let mut t = std::collections::BTreeMap::new();
    t.insert(format!("{role:?}"), 0.4);
    for b in 0..=transit::BANDS.len() {
        t.insert(format!("{role:?}/b{b}"), 0.9 - 0.1 * b as f32);
    }
    w.stat_table.as_mut().expect("a table").p_attend = Some(t.clone().into());
    let want = 0.9 - 0.1 * band as f32;
    assert_eq!(lod::stat_attend(&w, who, role), Some(want), "the band's row");
    // Every line cut: the commute is the walk, a longer band, a lower row.
    for l in 0..3 {
        transit::cut_line(&mut w, l, 5).expect("a line");
    }
    let walked = transit::commute_band(&w, who).expect("a band");
    assert!(walked > band, "the walk is a longer band ({band} -> {walked})");
    assert!(lod::stat_attend(&w, who, role) < Some(want));
    // A table without banded rows: the role's row (P5f's rule).
    t.retain(|k, _| !k.contains('/'));
    w.stat_table.as_mut().expect("a table").p_attend = Some(t.into());
    assert_eq!(lod::stat_attend(&w, who, role), Some(0.4));
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

// ---------------------------------------------------------------------------
// Measurement (printed findings, never asserted)
// ---------------------------------------------------------------------------

fn median(v: &mut [u32]) -> u32 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[v.len() / 2]
}

/// One run: rides, commute medians by mode, ticks/s.
fn measure(on: bool, full: bool, days: u64) -> String {
    let mut cfg = Config::load();
    cfg.transit.enabled = on;
    if full {
        cfg.lod.force = Some(Lod::Full);
    }
    let mut w = World::new(42, cfg);
    w.commute_log = Some(Vec::new());
    let mut rides = (0u32, 0u32, 0i64, 0u32, 0i64);
    let mut refused = [0u32; 4];
    let t = std::time::Instant::now();
    for _ in 0..days {
        w.run_ticks(TICKS_PER_DAY);
        let c = &w.stats.history.back().expect("a day").transit;
        rides.0 += c.rides_full;
        rides.1 += c.rides_coarse;
        rides.2 += c.fares;
        rides.3 += c.fare_dodges;
        rides.4 += c.transit_power;
        refused[0] += c.refused_no_route;
        refused[1] += c.refused_fare;
        refused[2] += c.refused_slow;
        refused[3] += c.refused_danger;
    }
    let secs = t.elapsed().as_secs_f64();
    let log = w.commute_log.take().unwrap_or_default();
    let mut all: Vec<u32> = log.iter().map(|e| e.0).collect();
    let mut tiles: Vec<u32> = log.iter().map(|e| e.1).collect();
    let mut walked: Vec<u32> = log.iter().filter(|e| e.2 == 0).map(|e| e.0).collect();
    let mut rode: Vec<u32> = log.iter().filter(|e| e.2 == 2).map(|e| e.0).collect();
    let mut rode_tiles: Vec<u32> = log.iter().filter(|e| e.2 == 2).map(|e| e.1).collect();
    let mut far: Vec<u32> = log.iter().filter(|e| e.1 >= 80).map(|e| e.0).collect();
    format!(
        "transit {} {} {days} d: tps {:.0}; rides full {} coarse {}, fares {}, dodges {}, power {}; refused no_route/fare/slow/danger {:?}\n  \
         commutes {} (median {} tiles): median ticks all {}, walked {} (n {}), rode {} (n {}, median {} tiles), 80+ tiles {} (n {})",
        if on { "ON " } else { "OFF" },
        if full { "all-Full" } else { "mixed LOD" },
        (days * TICKS_PER_DAY) as f64 / secs,
        rides.0,
        rides.1,
        rides.2,
        rides.3,
        rides.4,
        refused,
        log.len(),
        median(&mut tiles),
        median(&mut all),
        median(&mut walked),
        walked.len(),
        median(&mut rode),
        rode.len(),
        median(&mut rode_tiles),
        median(&mut far),
        far.len(),
    )
}

/// Seed 42, transit on vs off: rides, commute medians, ticks/s. Run with
/// `--ignored --nocapture`; `TRANSIT_DAYS` (default 10), `TRANSIT_FULL=1`
/// for the all-Full city.
#[test]
#[ignore]
fn measure_transit_on_vs_off() {
    let days = std::env::var("TRANSIT_DAYS").ok().and_then(|d| d.parse().ok()).unwrap_or(10);
    let full = std::env::var("TRANSIT_FULL").is_ok_and(|v| v == "1");
    for on in [false, true, false, true] {
        println!("{}", measure(on, full, days));
    }
}

/// The day's commute tail over the housed, free, non-sweeper workers
/// (`grow_housing`'s filter), each count past `commute_move_ticks`:
/// `[by the walk (Manhattan x move_ticks_full), by commute_cost (the
/// shared distance: the ride where it wins with the El on), by the agent's
/// own choice at 06:00 today (choose's route, else the walk), workers]`.
fn farmer_trips(w: &World) -> Vec<u32> {
    use citysim::systems::missions;
    use citysim::{Job, Role};
    let t = (w.tick / TICKS_PER_DAY) * TICKS_PER_DAY + 6 * 60;
    w.citizens()
        .into_iter()
        .filter_map(|a| {
            let j = w.comp::<Job>(a).filter(|j| j.role == Role::Farmer)?;
            let work = w.comp::<Building>(j.employer?)?.door;
            let hd = missions::home_of(w, a).and_then(|h| w.comp::<Building>(h)).map(|b| b.door)?;
            Some(transit::estimate_from(w, a, hd, work, t) as u32)
        })
        .collect()
}

fn far_workers(w: &World) -> [u32; 4] {
    use citysim::systems::{jobs, life, missions};
    use citysim::{Job, Sentence};
    let cap = w.config.life.commute_move_ticks;
    let t = (w.tick / TICKS_PER_DAY) * TICKS_PER_DAY + 6 * 60;
    let mut out = [0u32; 4];
    for a in w.citizens() {
        let Some(employer) = w.comp::<Job>(a).and_then(|j| j.employer) else { continue };
        if w.has::<Sentence>(a) || w.comp::<Brain>(a).is_none_or(|b| b.emigrating) || jobs::is_sweeper(w, a) {
            continue;
        }
        let Some(work) = w.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door) else { continue };
        let Some(hd) = missions::home_of(w, a).and_then(|h| w.comp::<Building>(h)).map(|b| b.door) else { continue };
        out[3] += 1;
        let walk = u64::from(hd.manhattan(work)) * w.config.exec.move_ticks_full;
        let cost = life::commute_cost(w, hd, work);
        let own = if transit::enabled(w) {
            transit::choose(w, a, hd, work, t).map_or(walk, |r| r.total.min(walk))
        } else {
            walk
        };
        for (i, v) in [walk, cost, own].into_iter().enumerate() {
            out[i] += u32::from(v > cap);
        }
        if std::env::var("TRANSIT_DIAG").is_ok() && cost <= cap && own > cap {
            let coins = w.comp::<Wallet>(a).map_or(0, |x| x.coins);
            let law = w.comp::<Personality>(a).map_or(0.0, |p| p.lawfulness);
            eprintln!(
                "DIAG walk {walk} cost {cost} own {own} {:?} coins {coins} law {law:.2} price {}",
                transit::choose(w, a, hd, work, t).err(),
                w.mean_price()
            );
        }
    }
    out
}

fn pct(v: &mut [u32], p: f32) -> u32 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[((v.len() - 1) as f32 * p).round() as usize]
}

/// One seed, transit on or off, `days` days: the day-`days` commute tail and
/// the city's vital signs (printed findings).
fn measure_tail(seed: u64, on: bool, days: u64) -> String {
    use citysim::EventKind;
    let mut cfg = Config::load();
    cfg.transit.enabled = on;
    // Experiments (printed findings): `TRANSIT_POOR_TPC` overrides `poor_ticks_per_coin`,
    // `TRANSIT_POWER` `power_per_line_day`.
    if let Some(v) = std::env::var("TRANSIT_POWER").ok().and_then(|v| v.parse().ok()) {
        cfg.transit.power_per_line_day = v;
    }
    if let Some(v) = std::env::var("TRANSIT_POOR_TPC").ok().and_then(|v| v.parse().ok()) {
        cfg.transit.poor_ticks_per_coin = v;
    }
    let mut w = World::new(seed, cfg);
    let seeded = w.corps();
    w.commute_log = Some(Vec::new());
    let mut cursor = 0u64;
    let mut murders = 0u32;
    let mut starved = 0u32;
    let (mut rf, mut rc, mut rs, mut fares, mut dodges, mut power) = (0u64, 0u64, 0u64, 0i64, 0u64, 0i64);
    let (mut sc, mut scr) = (0u64, 0u64);
    let mut refused = [0u64; 4];
    for _ in 0..days {
        w.run_ticks(TICKS_PER_DAY);
        murders +=
            w.events.iter().rev().take_while(|e| e.id >= cursor).filter(|e| e.kind == EventKind::Murder).count() as u32;
        cursor = w.next_event_id;
        starved += w.stats.history.back().map_or(0, |r| r.deaths_starvation);
        let c = &w.stats.history.back().expect("a day").transit;
        rf += u64::from(c.rides_full);
        rc += u64::from(c.rides_coarse);
        rs += u64::from(c.rides_stat);
        sc += u64::from(c.stat_commutes);
        scr += u64::from(c.stat_commute_rides);
        fares += c.fares;
        dodges += u64::from(c.fare_dodges);
        power += c.transit_power;
        refused[0] += u64::from(c.refused_no_route);
        refused[1] += u64::from(c.refused_fare);
        refused[2] += u64::from(c.refused_slow);
        refused[3] += u64::from(c.refused_danger);
    }
    let [far_walk, far_cost, far_own, workers] = far_workers(&w);
    let row = w.stats.history.back().expect("a day");
    let alive: std::collections::BTreeSet<EntityId> = w.corps().into_iter().collect();
    let corps = seeded.iter().filter(|c| alive.contains(c)).count();
    let log = w.commute_log.take().unwrap_or_default();
    let mut all: Vec<u32> = log.iter().map(|e| e.0).collect();
    let mut walked: Vec<u32> = log.iter().filter(|e| e.2 == 0).map(|e| e.0).collect();
    let mut rode: Vec<u32> = log.iter().filter(|e| e.2 == 2).map(|e| e.0).collect();
    let rode_n = rode.len();
    let share = if log.is_empty() { 0.0 } else { rode_n as f32 / log.len() as f32 };
    let stat_share = if sc == 0 { 0.0 } else { scr as f64 / sc as f64 };
    let purses: i64 = w.citizens().into_iter().filter_map(|a| w.comp::<Wallet>(a).map(|x| x.coins)).sum();
    // The money circuit: where the coins sit on the last day.
    let corps_sum: i64 = w.corps().iter().filter_map(|&c| w.comp::<citysim::Corp>(c)).map(|c| c.treasury).sum();
    let gangs_sum: i64 = w.gangs().iter().filter_map(|&g| w.comp::<citysim::Gang>(g)).map(|g| g.treasury).sum();
    let total = citysim::systems::ownership::total_coins(&w);
    let outside = w.outside.treasuries();
    let minted = w.outside.minted;
    let circuit =
        format!("coins in the city {total} (corps {corps_sum}, gangs {gangs_sum}), outside {outside}, minted {minted}");
    let mut farmers = farmer_trips(&w);
    let farmers_n = farmers.len();
    let (f50, f90) = (pct(&mut farmers, 0.5), pct(&mut farmers, 0.9));
    format!(
        "seed {seed} transit {}: d{days} far walk/cost/own {far_walk}/{far_cost}/{far_own} of {workers} housed \
         workers; farmers' own trip p50/p90 {f50}/{f90} (n {farmers_n}); employed {}, pop {}, treasury {}, purses \
         {purses}, {circuit}, starved {starved}, Murders {murders}, corps alive {corps}/{}; rides/day full {:.0} coarse {:.0} \
         stat {:.0}, fares {fares}, dodges {dodges}, power {power}, refused no_route/fare/slow/danger {refused:?}; \
         body commutes {} ride share {:.3}, stat ride share {stat_share:.3}; commute ticks all p50/p90 {}/{}, walk \
         {}/{}, ride {}/{}",
        if on { "ON " } else { "OFF" },
        row.employed,
        row.population,
        row.treasury,
        seeded.len(),
        rf as f64 / days as f64,
        rc as f64 / days as f64,
        rs as f64 / days as f64,
        log.len(),
        share,
        pct(&mut all, 0.5),
        pct(&mut all, 0.9),
        pct(&mut walked, 0.5),
        pct(&mut walked, 0.9),
        pct(&mut rode, 0.5),
        pct(&mut rode, 0.9),
    )
}

/// Seeds 42-44 (or `TRANSIT_SEEDS`, comma-separated) x `TRANSIT_DAYS`
/// (default 120), transit off and on, in threads: the commute tail
/// (`far_workers`), employed, Murders, corps alive, rides, the ride share
/// and commute ticks by mode. Printed findings; run alone with
/// `--ignored --nocapture`.
#[test]
#[ignore]
fn measure_transit_tail() {
    let days = std::env::var("TRANSIT_DAYS").ok().and_then(|d| d.parse().ok()).unwrap_or(120);
    let seeds: Vec<u64> = std::env::var("TRANSIT_SEEDS")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![42, 43, 44]);
    let mut handles = Vec::new();
    for &seed in &seeds {
        for on in [false, true] {
            handles.push(std::thread::spawn(move || measure_tail(seed, on, days)));
        }
    }
    for h in handles {
        println!("{}", h.join().expect("a seed run"));
    }
}

/// One run of `measure_el_crews`: the El crews capped at `cap` a line,
/// `seed` x `days`, the money circuit (printed findings).
fn measure_crews(name: &str, seed: u64, cap: u16, credit: f32, days: u64) -> String {
    use citysim::EventKind;
    let mut cfg = Config::load();
    cfg.transit.staff_per_line = cap;
    cfg.treasury.credit_max = credit;
    let mut w = World::new(seed, cfg);
    let mut cursor = 0u64;
    let mut murders = 0u32;
    let mut fares = 0i64;
    let mut payroll = 0i64;
    let mut credits = 0i64;
    let mut tre = Vec::new();
    for d in 1..=days {
        w.run_ticks(TICKS_PER_DAY);
        murders +=
            w.events.iter().rev().take_while(|e| e.id >= cursor).filter(|e| e.kind == EventKind::Murder).count() as u32;
        cursor = w.next_event_id;
        let row = w.stats.history.back().expect("a day");
        fares += row.transit.fares;
        payroll += row.jobs.civic_payroll;
        credits += row.jobs.tax_credit;
        if d % 30 == 0 || d == 15 {
            tre.push(format!("d{d} {}/{:.2}", row.treasury, row.econ.tax_rate));
        }
    }
    let row = w.stats.history.back().expect("a day");
    let cits = w.citizens();
    let purses: i64 = cits.iter().filter_map(|&a| w.comp::<Wallet>(a).map(|x| x.coins)).sum();
    let corps_sum: i64 = w.corps().iter().filter_map(|&c| w.comp::<citysim::Corp>(c)).map(|c| c.treasury).sum();
    let total = citysim::systems::ownership::total_coins(&w);
    format!(
        "{name} seed {seed}: el_staff {} employed {} homeless {} pop {} purses {purses} (mean {:.1}) Murders {murders} \
         fares {fares} civic_payroll {payroll} credits {credits} guards {} sanitation {} corps {corps_sum} till {} coins {total} \
         treasury/tax [{}]",
        row.transit.el_staff,
        row.employed,
        row.homeless,
        row.population,
        purses as f64 / cits.len().max(1) as f64,
        row.jobs.guard_count,
        row.jobs.sanitation_count,
        w.econ.recycler_till,
        tre.join(", "),
    )
}

/// The El crews' cap and the money circuit: `EL_RUNS` (comma-separated
/// `name=cap/credit_max`, default `main=65535/0,cap=4/0,capcredit=4/0.15`) x `EL_SEEDS` (default 42-47) x
/// `EL_DAYS` (default 120), in threads. Printed findings; run alone with
/// `--ignored --nocapture`.
#[test]
#[ignore]
fn measure_el_crews() {
    let days = std::env::var("EL_DAYS").ok().and_then(|d| d.parse().ok()).unwrap_or(120);
    let seeds: Vec<u64> = std::env::var("EL_SEEDS")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| (42..=47).collect());
    let runs: Vec<(String, u16, f32)> = std::env::var("EL_RUNS")
        .unwrap_or_else(|_| "main=65535/0,cap=4/0,capcredit=4/0.15".into())
        .split(',')
        .filter_map(|r| {
            let (n, rest) = r.split_once('=')?;
            let (c, k) = rest.split_once('/')?;
            Some((n.to_string(), c.parse().ok()?, k.parse().ok()?))
        })
        .collect();
    let mut handles = Vec::new();
    for (name, cap, credit) in runs {
        for &seed in &seeds {
            let name = name.clone();
            handles.push(std::thread::spawn(move || measure_crews(&name, seed, cap, credit, days)));
        }
    }
    for h in handles {
        println!("{}", h.join().expect("a seed run"));
    }
}
