//! M13 phase 2: vehicles (docs/M13_ASSETS.md § 2, § 6, plan 2.9): step
//! durations, Coarse and flyer timing, haul trucks, crashes, the tier
//! contest, theft and the chop, the Shop, the Garage, and the phase 1
//! review's carry-overs (all-or-nothing payments, the death ordering, the
//! lender at death, the trip-then-rekit order, `seller_open`).

use rand::SeedableRng;

use citysim::components::ActionInstance;
use citysim::exec::{ExecState, GotoTarget};
use citysim::goap::{ActionKind, LocationKey, Plan};
use citysim::systems::{assets, demography, lod, ownership, plan, security, vehicles};
use citysim::{
    Asset, AssetKind, AssetLoc, Body, Brain, Building, BuildingKind, Config, Controller, Corp, DeathCause, EntityId,
    EventKind, Finance, GoalKind, Good, Job, Kit, Lod, Needs, Position, Role, ShopPick, Slot, TileKind, TilePos, Trip,
    Wallet, World,
};

fn city() -> World {
    World::new(42, Config::load().dole_city())
}

/// Living adults with a Brain and a Wallet, ascending.
fn adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && w.has::<Wallet>(a) && demography::is_adult(w, a))
        .collect()
}

fn set_coins(w: &mut World, a: EntityId, coins: i64) {
    w.comp_mut::<Wallet>(a).expect("wallet").coins = coins;
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |x| x.coins)
}

fn asset(w: &World, a: EntityId) -> &Asset {
    w.comp::<Asset>(a).expect("asset")
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

/// `len + 1` Road tiles in a straight horizontal run.
fn road_run(w: &World, len: usize) -> Vec<TilePos> {
    for y in 0..w.map.h().min(256) {
        let mut run: Vec<TilePos> = Vec::new();
        for x in 0..w.map.w().min(256) {
            let t = TilePos { x: x as u16, y: y as u16 };
            if w.map.tile_at(t) == TileKind::Road {
                run.push(t);
                if run.len() > len {
                    return run;
                }
            } else {
                run.clear();
            }
        }
    }
    panic!("no straight road of {len} tiles");
}

/// A Full agent standing on `tile`, out of any building.
fn on_street(w: &mut World, id: EntityId, tile: TilePos) {
    lod::set_lod(w, id, Lod::Full);
    w.leave_building(id);
    let p = w.comp_mut::<Position>(id).expect("pos");
    p.tile = tile;
    p.building = None;
}

/// A Full agent inside `b`.
fn inside(w: &mut World, id: EntityId, b: EntityId) {
    lod::set_lod(w, id, Lod::Full);
    w.leave_building(id);
    w.enter_building(id, b);
}

/// Put `id` behind the wheel of `v` as `begin_trip` does: the Trip first,
/// then `InUse` (the rekit reads it).
fn drive(w: &mut World, id: EntityId, v: EntityId, to: TilePos) {
    let from = w.comp::<Position>(id).expect("pos").tile;
    let half = (from.manhattan(to) / 2) as u16;
    let start = w.tick;
    w.trips.insert(id, Trip { vehicle: v, start, from, road_tiles: 0, steps: 0, half, mid: None, chase: false });
    assets::set_loc(w, v, AssetLoc::InUse(id));
}

/// A one-step plan walking a street path (`path[0]` is the first tile entered).
fn path_plan(w: &mut World, id: EntityId, path: &[TilePos]) {
    let dest = *path.last().expect("a path");
    let mut rev = path.to_vec();
    rev.reverse();
    let tick = w.tick;
    let b = w.comp_mut::<Brain>(id).expect("brain");
    b.plan = Some(Plan {
        goal: GoalKind::Idle,
        target: None,
        steps: vec![ActionInstance { action: ActionKind::GoTo(LocationKey::Street), target: None, tile: None }],
        started_tick: tick,
    });
    b.plan_step = 0;
    b.exec = ExecState::Goto {
        target: GotoTarget { dest: LocationKey::Street, tile: dest, building: None },
        path: rev,
        next_move_tick: tick,
        blocked_since: None,
        carry_q: 0,
    };
}

/// One executor tick (no other system runs: nobody else has a plan yet).
fn exec_tick(w: &mut World) {
    citysim::exec::run(w);
    w.tick += 1;
}

fn goto_state(w: &World, id: EntityId) -> Option<(u64, u8)> {
    match &w.comp::<Brain>(id)?.exec {
        ExecState::Goto { next_move_tick, carry_q, .. } => Some((*next_move_tick, *carry_q)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Steps (D19, D21, D22)
// ---------------------------------------------------------------------------

#[test]
fn test_unchromed_walker_steps_as_m12() {
    let mut w = city();
    let run = road_run(&w, 25);
    let who = adults(&w)[3];
    on_street(&mut w, who, run[0]);
    path_plan(&mut w, who, &run[1..]);
    let t0 = w.tick;
    let mut seen = Vec::new();
    for _ in 0..80 {
        exec_tick(&mut w);
        if let Some((next, carry)) = goto_state(&w, who) {
            if seen.last().is_none_or(|&(n, _)| n != next) {
                seen.push((next, carry));
            }
        }
        if seen.len() >= 20 {
            break;
        }
    }
    assert_eq!(seen.len(), 20);
    for (k, &(next, carry)) in seen.iter().enumerate() {
        assert_eq!(next, t0 + 2 * (k as u64 + 1), "step {k}");
        assert_eq!(carry, 0, "step {k}");
    }
    assert_eq!(w.comp::<Position>(who).expect("pos").tile, run[20]);
}

#[test]
fn test_car_cuts_road_trip_and_keeps_flow_fields() {
    let mut w = city();
    let run = road_run(&w, 40);
    let who = adults(&w)[5];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    on_street(&mut w, who, run[0]);
    drive(&mut w, who, car, run[40]);
    assert_eq!(w.comp::<Kit>(who).expect("kit").driving, Some(AssetKind::Car));
    path_plan(&mut w, who, &run[1..]);
    let fields = w.flow_fields.len();
    let t0 = w.tick;
    let mut arrived = None;
    for _ in 0..60 {
        exec_tick(&mut w);
        if w.comp::<Brain>(who).expect("brain").plan.as_ref().is_none_or(|p| p.steps.len() as u8 <= 1)
            && w.comp::<Brain>(who).expect("brain").plan_step >= 1
        {
            arrived = Some(w.tick - 1);
            break;
        }
    }
    // 40 road tiles at 2 quarters a step: 2 steps a tick, 20 ticks.
    assert_eq!(arrived, Some(t0 + 20), "arrival");
    assert_eq!(w.flow_fields.len(), fields, "a car never builds a field of its own");
    // The trip ended at the street: parked at the nearest door.
    assert!(w.trips.is_empty());
    assert!(matches!(asset(&w, car).loc, AssetLoc::Parked(_)));
    assert_eq!(w.comp::<Kit>(who).expect("kit").driving, None);
}

#[test]
fn test_max_steps_per_tick_bounds_burst() {
    let mut cfg = Config::load().dole_city();
    cfg.vehicles.mult.car = vec![0.01, 0.01, 0.01];
    cfg.vehicles.max_steps_per_tick = 3;
    let mut w = World::new(42, cfg);
    let run = road_run(&w, 30);
    let who = adults(&w)[5];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    on_street(&mut w, who, run[0]);
    drive(&mut w, who, car, run[30]);
    assert_eq!(vehicles::step_q(&w, who, run[1]), 1, "max(1, round(8 x 0.01))");
    path_plan(&mut w, who, &run[1..]);
    let mut at = 0usize;
    for _ in 0..8 {
        exec_tick(&mut w);
        let tile = w.comp::<Position>(who).expect("pos").tile;
        let now = run.iter().position(|&t| t == tile).expect("on the run");
        assert!(now - at <= 3, "{} steps in one call", now - at);
        at = now;
    }
    assert_eq!(at, 24, "three steps a tick, eight ticks");
}

#[test]
fn test_coarse_driver_timed_estimate() {
    let mut w = city();
    let who = adults(&w)[9];
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    lod::set_lod(&mut w, who, Lod::Coarse);
    w.leave_building(who);
    w.enter_building(who, home);
    let from = citysim::exec::walk_origin(&w, who).expect("origin");
    let far = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .max_by_key(|&m| w.comp::<Building>(m).expect("b").door.manhattan(from))
        .expect("a Market");
    let door = w.comp::<Building>(far).expect("b").door;
    let d = u64::from(from.manhattan(door));
    let target = GotoTarget { dest: LocationKey::Market, tile: door, building: Some(far) };
    let tick = w.tick;
    let walk = match citysim::exec::timed_goto(&w, who, target.clone()) {
        ExecState::GotoTimed { arrive_tick, .. } => arrive_tick - tick,
        s => panic!("{s:?}"),
    };
    assert_eq!(walk, 2 * d, "a walker: 2 ticks a tile");
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    assert_eq!(asset(&w, car).loc, AssetLoc::Parked(home));
    assert_eq!(vehicles::begin_trip(&mut w, who, &target), Some(AssetKind::Car));
    let drive = match citysim::exec::timed_goto(&w, who, target) {
        ExecState::GotoTimed { arrive_tick, .. } => arrive_tick - tick,
        s => panic!("{s:?}"),
    };
    assert_eq!(drive, (d as f32 * 2.0 * (0.7 * 0.2 + 0.3 * 1.0)).round() as u64, "round(d x 0.88)");
}

#[test]
fn test_flyer_crosses_wall_in_chebyshev_time() {
    let mut w = city();
    let who = adults(&w)[11];
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    let flyer = assets::grant(&mut w, who, AssetKind::Flyer, 1).expect("flyer");
    inside(&mut w, who, home);
    let from = citysim::exec::walk_origin(&w, who).expect("origin");
    let cheb = |t: TilePos| citysim::systems::law::chebyshev(from, t);
    let mut candidates: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .chain(w.buildings_of_kind(BuildingKind::Market))
        .copied()
        .collect();
    candidates.sort_by_key(|&b| (cheb(w.comp::<Building>(b).expect("b").door).abs_diff(30), b));
    let target = candidates[0];
    let door = w.comp::<Building>(target).expect("b").door;
    let expected = ((f64::from(cheb(door)) * 0.3) - 1e-4).ceil().max(1.0) as u64;
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
    let fields = w.flow_fields.len();
    exec_tick(&mut w);
    match w.comp::<Brain>(who).expect("brain").exec.clone() {
        ExecState::Fly { arrive_tick, depart, .. } => assert_eq!(arrive_tick - depart, expected),
        s => panic!("not flying: {s:?}"),
    }
    assert_eq!(w.comp::<Position>(who).expect("pos").building, None, "airborne");
    for _ in 0..expected + 2 {
        exec_tick(&mut w);
    }
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(target), "landed at the door");
    assert_eq!(asset(&w, flyer).loc, AssetLoc::Parked(target));
    assert_eq!(w.flow_fields.len(), fields, "no flow field built");
    assert_eq!(w.stats.current.crashes, 0, "a flyer never crashes");
}

// ---------------------------------------------------------------------------
// Hauls (D23)
// ---------------------------------------------------------------------------

#[test]
fn test_truck_hauls_six_batches_and_returns() {
    let mut w = city();
    let farmer = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .find(|&f| w.comp::<Job>(f).and_then(|j| j.employer).is_some_and(|e| w.owner_of(e).is_some()))
        .expect("a corp farmer");
    let farm = w.comp::<Job>(farmer).and_then(|j| j.employer).expect("farm");
    let owner = w.owner_of(farm);
    let truck = assets::spawn_asset(&mut w, AssetKind::Truck, 1, owner, AssetLoc::Parked(farm), 1500);
    assert_eq!(vehicles::fleet_vehicle_at(&w, farm), Some(truck));
    w.comp_mut::<Building>(farm).expect("farm").stock_food = 400;
    inside(&mut w, farmer, farm);
    let tick = w.tick;
    let b = w.comp_mut::<Brain>(farmer).expect("brain");
    b.plan = Some(Plan {
        goal: GoalKind::Work,
        target: None,
        steps: vec![
            ActionInstance { action: ActionKind::HaulToMarket, target: Some(farm), tile: None },
            ActionInstance { action: ActionKind::GoTo(LocationKey::Market), target: None, tile: None },
        ],
        started_tick: tick,
    });
    b.plan_step = 0;
    b.exec = ExecState::Idle;
    exec_tick(&mut w);
    assert_eq!(w.comp::<Building>(farm).expect("farm").stock_food, 100, "6 x 50 on the truck");
    assert_eq!(w.stats.current.truck_hauls, 1);
    let steps: Vec<ActionKind> =
        w.comp::<Brain>(farmer).expect("b").plan.as_ref().expect("plan").steps.iter().map(|s| s.action).collect();
    assert_eq!(
        steps,
        [ActionKind::HaulToMarket, ActionKind::GoTo(LocationKey::Market), ActionKind::GoTo(LocationKey::Farm)],
        "the truck comes home"
    );
    assert_eq!(asset(&w, truck).loc, AssetLoc::InUse(farmer));
    for _ in 0..3000 {
        exec_tick(&mut w);
        if w.comp::<Brain>(farmer).expect("b").plan.is_none() {
            break;
        }
    }
    assert!(w.comp::<Brain>(farmer).expect("b").plan.is_none(), "the haul finished");
    let x = asset(&w, truck);
    assert_eq!((x.loc, x.keeper), (AssetLoc::Parked(farm), None), "home, nobody's");
    w.check_indices().expect("indices in step");
}

// ---------------------------------------------------------------------------
// Crashes (D25)
// ---------------------------------------------------------------------------

#[test]
fn test_fleeing_driver_hits_body_in_radius() {
    let mut cfg = Config::load().dole_city();
    cfg.vehicles.crash_per_tile = 1.0;
    cfg.vehicles.p_crash_kill = 0.0;
    let mut w = World::new(42, cfg);
    let run = road_run(&w, 12);
    let people = adults(&w);
    let (driver, near, far) = (people[1], people[2], people[3]);
    let car = assets::grant(&mut w, driver, AssetKind::Car, 1).expect("car");
    let tile = run[5];
    on_street(&mut w, near, run[7]);
    on_street(&mut w, far, run[2]);
    on_street(&mut w, driver, run[0]);
    vehicles::crash(&mut w, driver, car, tile, false);
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::Crash).expect("a crash");
    assert_eq!(e.actors.get(1).copied(), Some(near), "the body at 2, not the one at 3: {}", e.text);
    // A god pin makes an arrival a chase: at crash_per_tile 1 it crashes.
    let home = w.comp::<citysim::Household>(driver).and_then(|h| h.home).expect("home");
    assert_eq!(asset(&w, car).loc, AssetLoc::Parked(home));
    inside(&mut w, driver, home);
    w.chase_pins.insert(driver);
    let door = w.comp::<Building>(home).expect("b").door;
    let target = GotoTarget { dest: LocationKey::Home, tile: door, building: Some(home) };
    assert_eq!(vehicles::begin_trip(&mut w, driver, &target), Some(AssetKind::Car));
    assert!(w.trips.get(&driver).expect("trip").chase, "pinned: a chase");
    let before = count(&w, EventKind::Crash);
    vehicles::end_trip(&mut w, driver, true);
    assert_eq!(count(&w, EventKind::Crash), before + 1);
    assert!(!w.chase_pins.contains(&driver), "the pin is consumed");
    // Reflex dodges: a quick victim is missed far more often.
    let mut dodged = [0usize; 2];
    for (i, reflex) in [0.9f32, 0.2].into_iter().enumerate() {
        let mut w = World::new(42, {
            let mut c = Config::load().dole_city();
            c.vehicles.p_crash_kill = 0.0;
            c
        });
        let people = adults(&w);
        let (driver, victim) = (people[1], people[2]);
        let car = assets::grant(&mut w, driver, AssetKind::Car, 1).expect("car");
        on_street(&mut w, victim, run[5]);
        w.comp_mut::<Body>(victim).expect("body").reflex = reflex;
        for _ in 0..200 {
            vehicles::crash(&mut w, driver, car, run[5], false);
            if let Some(m) = w.comp_mut::<Asset>(car) {
                m.condition = 100;
            }
            let e = w.events.iter().rev().find(|e| e.kind == EventKind::Crash).expect("a crash");
            if e.text.contains("missed") {
                dodged[i] += 1;
            }
        }
    }
    assert!(dodged[0] > dodged[1] + 40, "reflex 0.9 dodged {} vs 0.2 {}", dodged[0], dodged[1]);
}

#[test]
fn test_contest_half_at_equal_tiers() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
    let wins = (0..10_000).filter(|_| security::contest(2, 2, 0.25, &mut rng)).count();
    assert!((wins as f32 / 10_000.0 - 0.5).abs() < 0.02, "equal tiers: {wins}");
    assert_eq!(security::contest_p(3.0, 1.0, 0.25), 0.95, "clamped");
    assert_eq!(security::contest_p(1.0, 3.0, 0.25), 0.05, "clamped");
    let wins = (0..10_000).filter(|_| security::contest(3, 1, 0.25, &mut rng)).count();
    assert!((wins as f32 / 10_000.0 - 0.95).abs() < 0.01, "3 vs 1: {wins}");
}

// ---------------------------------------------------------------------------
// Theft and the chop (D26, D27)
// ---------------------------------------------------------------------------

#[test]
fn test_stolen_vehicle_chopped_into_parts() {
    let mut w = city();
    let gang = w.gangs()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let recruits: Vec<EntityId> =
        adults(&w).into_iter().filter(|&a| w.gang_of(a).is_none() && !w.has::<Job>(a)).take(6).collect();
    for r in recruits {
        citysim::systems::gang::enlist(&mut w, r, gang);
    }
    let members = w.comp::<citysim::Gang>(gang).expect("g").members.len();
    assert!(members >= 6);
    let quota = (w.config.vehicles.bikes_max_share * members as f32).floor() as usize;
    for _ in 0..quota {
        assets::spawn_asset(&mut w, AssetKind::Motorcycle, 1, Some(gang), AssetLoc::Parked(hideout), 300);
    }
    let car = assets::spawn_asset(&mut w, AssetKind::Car, 1, Some(gang), AssetLoc::Stock(hideout), 800);
    w.comp_mut::<Asset>(car).expect("car").stolen = true;
    let parts0 = w.stock(hideout, Good::Parts);
    vehicles::chop_daily(&mut w, gang);
    assert!(w.comp::<Asset>(car).is_none(), "chopped");
    assert_eq!(w.stock(hideout, Good::Parts) - parts0, 8, "parts_per.car");
    assert_eq!(count(&w, EventKind::Chopped), 1);
    assert_eq!(w.stats.current.chops, 1);
    // Under quota, a stolen bike is kept for a rider.
    let bike = assets::spawn_asset(&mut w, AssetKind::Motorcycle, 1, Some(gang), AssetLoc::Stock(hideout), 300);
    w.comp_mut::<Asset>(bike).expect("bike").stolen = true;
    let spare = w.comp::<citysim::Gang>(gang).expect("g").members.len();
    assert!(spare > 0);
    let parked: Vec<EntityId> = assets::assets_of(&w, Some(gang)).to_vec();
    for v in parked.into_iter().filter(|&v| v != bike) {
        assets::despawn(&mut w, v);
    }
    vehicles::chop_daily(&mut w, gang);
    let x = asset(&w, bike);
    assert_eq!(x.loc, AssetLoc::Parked(hideout), "kept");
    assert!(x.keeper.is_some(), "a member rides it");
    w.check_indices().expect("indices in step");
}

#[test]
fn test_street_parked_theft_roll_binds_to_controlling_gang() {
    let mut cfg = Config::load().dole_city();
    cfg.vehicles.vehicle_theft_base = 1.0;
    let mut w = World::new(42, cfg);
    let gang = w.gangs()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let who = adults(&w).into_iter().find(|&a| w.gang_of(a).is_none()).expect("a civilian");
    lod::set_lod(&mut w, who, Lod::Statistical);
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    let AssetLoc::Parked(home) = asset(&w, car).loc else { panic!("parked") };
    assert!(vehicles::street_parked(&w, car));
    let d = w.district_of_building(home);
    w.district_mut(d).control = Controller::Gang(gang);
    vehicles::theft_daily(&mut w);
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc, x.stolen), (Some(gang), AssetLoc::Stock(hideout), true));
    assert!(count(&w, EventKind::VehicleStolen) >= 1);
    assert!(w.stats.current.vehicle_thefts >= 1);
    // A thief on foot beats the lock (or botches it) by the tier contest.
    let mut w = city();
    let people = adults(&w);
    let (owner, thief) = (people[20], people[21]);
    let bike = assets::grant(&mut w, owner, AssetKind::Motorcycle, 1).expect("bike");
    let stand = vehicles::vehicle_stand(&w, bike).expect("stand");
    on_street(&mut w, thief, stand);
    assert!(vehicles::may_steal(&w, thief, bike));
    let mut won = false;
    for _ in 0..20 {
        if vehicles::steal(&mut w, thief, bike) {
            won = true;
            break;
        }
    }
    assert!(won, "tier 1 against tier 1 wins half the time");
    let x = asset(&w, bike);
    assert_eq!((x.loc, x.keeper, x.stolen), (AssetLoc::InUse(thief), Some(thief), true));
    assert_eq!(vehicles::stolen_held_by(&w, thief), Some(bike));
    assert_eq!(w.comp::<Kit>(thief).expect("kit").driving, Some(AssetKind::Motorcycle));
}

// ---------------------------------------------------------------------------
// The Shop, the Garage, sellers (D16, D18, D29, D43)
// ---------------------------------------------------------------------------

/// Hire `who` as a Mechanic at Garage `g`, on shift (9:00-18:00) at 10:00.
fn open_garage(w: &mut World, g: EntityId, who: EntityId) {
    demography::hire(w, who, g, Role::Mechanic);
    w.tick = 600;
}

#[test]
fn test_seeded_garages_and_seller_open() {
    let w = city();
    let garages = w.buildings_of_kind(BuildingKind::Garage).to_vec();
    assert_eq!(garages.len(), 2);
    let names: Vec<String> = garages.iter().map(|&g| w.district_name(w.district_of_building(g)).to_string()).collect();
    assert_eq!(names, ["Civic", "Sump Central"]);
    let zetatech = w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == "Zetatech"));
    assert_eq!(w.owner_of(garages[0]), zetatech);
    let agent = w.owner_of(garages[1]).expect("an agent owner");
    assert!(w.has::<Wallet>(agent) && !w.has::<Job>(agent));
    assert_eq!(coins(&w, agent), w.config.shop.seed_owner_coins);
    // D16: open while one of its staff is on shift.
    let mut w = w;
    assert!(!assets::seller_open(&w, garages[0]), "nobody hired yet");
    let who = adults(&w).into_iter().find(|&a| !w.has::<Job>(a)).expect("jobless");
    open_garage(&mut w, garages[0], who);
    assert!(assets::seller_open(&w, garages[0]));
    w.tick = 1200;
    assert!(!assets::seller_open(&w, garages[0]), "off shift at 20:00");
}

#[test]
fn test_shop_buys_car_when_commute_long_and_affordable() {
    let mut w = city();
    let g = w.buildings_of_kind(BuildingKind::Garage)[0];
    let people = adults(&w);
    let mechanic = people.iter().copied().find(|&a| !w.has::<Job>(a)).expect("jobless");
    open_garage(&mut w, g, mechanic);
    let who = people
        .iter()
        .copied()
        .find(|&a| a != mechanic && w.comp::<Job>(a).is_some() && w.gang_of(a).is_none())
        .expect("a worker");
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    let hd = w.comp::<Building>(home).expect("b").door;
    let work = w
        .buildings_of_kind(BuildingKind::Farm)
        .iter()
        .chain(w.buildings_of_kind(BuildingKind::Market))
        .copied()
        .find(|&b| w.comp::<Building>(b).expect("b").door.manhattan(hd) >= 120)
        .expect("a workplace 120 tiles away");
    {
        let j = w.comp_mut::<Job>(who).expect("job");
        j.employer = Some(work);
        j.wage_per_day = 8;
    }
    set_coins(&mut w, who, 900);
    w.comp_mut::<Needs>(who).expect("needs").wealth = 0.5;
    lod::set_lod(&mut w, who, Lod::Full);
    let o = assets::shop_choice(&w, who, true).expect("an offer");
    assert_eq!((o.pick.kind, o.pick.tier, o.financed, o.seller), (AssetKind::Car, 1, false, g));
    assert!(o.score > 0.05, "Shop {} beats Idle", o.score);
    let (goal, trace) = citysim::utility::think(&w, who).expect("think");
    let shop = trace.goals.iter().find(|s| s.goal == GoalKind::Shop).map(|s| s.score);
    let idle = 0.05;
    assert!(shop.is_some_and(|s| s > idle) || goal == GoalKind::Shop, "{trace:?}");
    w.comp_mut::<Brain>(who).expect("b").current_goal = Some(GoalKind::Shop);
    plan::plan_for(&mut w, who, GoalKind::Shop);
    let b = w.comp::<Brain>(who).expect("b");
    let p = b.plan.as_ref().expect("a plan");
    let steps: Vec<ActionKind> = p.steps.iter().map(|s| s.action).collect();
    assert_eq!(steps, [ActionKind::GoTo(LocationKey::Seller), ActionKind::BuyAsset]);
    assert_eq!(p.target, Some(g));
    assert_eq!(b.shop_pick, Some(ShopPick { kind: AssetKind::Car, tier: 1, used: None, upgrade: false }));
    // The purchase at the counter.
    inside(&mut w, who, g);
    let mut b = w.comp::<Brain>(who).expect("b").clone();
    b.plan_step = 1;
    b.exec = ExecState::Idle;
    w.insert(who, b);
    for _ in 0..25 {
        exec_tick(&mut w);
    }
    let car = w.comp::<Kit>(who).expect("kit").vehicle.expect("bought a car");
    assert_eq!((asset(&w, car).kind, asset(&w, car).owner), (AssetKind::Car, Some(who)));
    assert!(assets::shop_choice(&w, who, true).is_none(), "on cooldown, and has one");
}

#[test]
fn test_garage_rent_paid_to_garage_owner() {
    let mut w = city();
    let g = w.buildings_of_kind(BuildingKind::Garage)[0];
    let landlord = w.owner_of(g);
    let who = adults(&w)[13];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    assets::set_loc(&mut w, car, AssetLoc::Parked(g));
    set_coins(&mut w, who, 50);
    let (l0, city0, total) = (w.purse(landlord), w.purse(None), ownership::total_coins(&w));
    assets::run(&mut w);
    // Upkeep 2 to the city, rent 1 to the Garage's owner (taxed).
    assert_eq!(coins(&w, who), 50 - 2 - 1);
    let landlord_gain = w.purse(landlord) - l0;
    assert!((0..=1).contains(&landlord_gain), "{landlord_gain}");
    assert_eq!(landlord_gain + (w.purse(None) - city0), 1 + 2, "rent and upkeep, tax included");
    assert_eq!(ownership::total_coins(&w), total);
    // Short of the rent: nothing is taken (all-or-nothing).
    set_coins(&mut w, who, 2);
    assets::run(&mut w);
    assert_eq!(coins(&w, who), 0, "the upkeep (2) taken in full, the rent (1) not at all");
}

// ---------------------------------------------------------------------------
// Trips (D20) and the phase 1 review's carry-overs
// ---------------------------------------------------------------------------

/// The building whose door is nearest `t` (not a Lot, standing), ties lower id.
fn nearest_door(w: &World, t: TilePos) -> EntityId {
    w.with::<Building>()
        .into_iter()
        .filter_map(|b| {
            let bd = w.comp::<Building>(b)?;
            (!bd.demolished && bd.kind != BuildingKind::Lot).then_some((bd.door.manhattan(t), b))
        })
        .min()
        .map(|(_, b)| b)
        .expect("a building")
}

#[test]
fn test_trip_abort_parks_at_nearest_building() {
    let mut w = city();
    let run = road_run(&w, 20);
    let who = adults(&w)[17];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    on_street(&mut w, who, run[10]);
    drive(&mut w, who, car, run[20]);
    path_plan(&mut w, who, &run[11..]);
    w.abort_plan(who);
    assert!(w.trips.is_empty());
    assert_eq!(asset(&w, car).loc, AssetLoc::Parked(nearest_door(&w, run[10])));
    assert_eq!(w.comp::<Kit>(who).expect("kit").driving, None);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_begin_trip_files_trip_before_rekit() {
    let mut w = city();
    let who = adults(&w)[19];
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    inside(&mut w, who, home);
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let door = w.comp::<Building>(market).expect("b").door;
    let target = GotoTarget { dest: LocationKey::Market, tile: door, building: Some(market) };
    assert_eq!(vehicles::vehicle_for_trip(&w, who), Some(car));
    assert_eq!(vehicles::begin_trip(&mut w, who, &target), Some(AssetKind::Car));
    // Carry-over 7: the Kit already reads the Trip (no stale `driving`).
    let k = w.comp::<Kit>(who).expect("kit").clone();
    assert_eq!((k.driving, k.vehicle), (Some(AssetKind::Car), Some(car)));
    assert_eq!(asset(&w, car).loc, AssetLoc::InUse(who));
    w.check_indices().expect("the Kit equals a recompute");
}

#[test]
fn test_death_at_the_wheel_parks_for_the_heir() {
    let mut w = city();
    let (who, spouse) = adults(&w)
        .into_iter()
        .find_map(|a| w.spouse_of(a).filter(|&s| w.has::<Brain>(s)).map(|s| (a, s)))
        .expect("a couple");
    let run = road_run(&w, 10);
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    on_street(&mut w, who, run[4]);
    drive(&mut w, who, car, run[10]);
    w.kill(who, DeathCause::Violence);
    // Carry-over 5: the trip ends before the heirs are dealt.
    let x = asset(&w, car);
    assert_eq!(x.loc, AssetLoc::Parked(nearest_door(&w, run[4])), "parked, not lost with the corpse");
    assert_eq!(x.owner, Some(spouse), "the heir's");
    assert!(w.trips.is_empty());
    w.check_indices().expect("indices in step");
}

#[test]
fn test_financed_implant_in_arrears_goes_to_lender_at_death() {
    let mut w = city();
    let lender = ownership::spawn_corp(&mut w, "Doc".into(), Default::default(), 1000, None);
    let who = adults(&w)[23];
    let arm = assets::grant(&mut w, who, AssetKind::Implant(Slot::Arms), 1).expect("arm");
    w.comp_mut::<Asset>(arm).expect("arm").finance =
        Some(Finance { lender: Some(lender), remaining: 100, per_day: 3, arrears: 2 });
    w.kill(who, DeathCause::Violence);
    // Carry-over 6: the lender is tried before the body keeps its chrome.
    let x = asset(&w, arm);
    assert_eq!((x.owner, x.loc), (Some(lender), AssetLoc::Installed(who)));
    assert!(x.finance.is_none());
}

#[test]
fn test_short_payment_takes_nothing() {
    let mut w = city();
    let lender = ownership::spawn_corp(&mut w, "Lender".into(), Default::default(), 1000, None);
    let who = adults(&w)[29];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    w.comp_mut::<Asset>(car).expect("car").finance =
        Some(Finance { lender: Some(lender), remaining: 600, per_day: 12, arrears: 0 });
    // 13 coins: the upkeep (2) is paid, the finance (12) is not, and the
    // wallet keeps the 11 (carry-over 1: no partial sweep).
    set_coins(&mut w, who, 13);
    assets::run(&mut w);
    assert_eq!(coins(&w, who), 11);
    let f = asset(&w, car).finance.clone().expect("plan");
    assert_eq!((f.remaining, f.arrears), (600, 1));
    assert_eq!(asset(&w, car).upkeep_arrears, 0);
    // Next day the due is 24 (the catch-up), still short: nothing again.
    assets::run(&mut w);
    assert_eq!(coins(&w, who), 9);
    assert_eq!(asset(&w, car).finance.as_ref().expect("plan").arrears, 2);
    // Carry-over 2: the Shop's burden reads `per_day`, never the catch-up.
    set_coins(&mut w, who, 1000);
    assets::run(&mut w);
    let f = asset(&w, car).finance.clone().expect("plan");
    assert_eq!((f.remaining, f.arrears), (600 - 36, 0), "paid in full: three days' worth");
}

#[test]
fn test_abandoned_stolen_vehicle_is_recovered() {
    let mut w = city();
    let people = adults(&w);
    let (owner, thief, other) = (people[20], people[21], people[22]);
    let home = w.comp::<citysim::Household>(owner).and_then(|h| h.home).expect("home");
    let bike = assets::grant(&mut w, owner, AssetKind::Motorcycle, 1).expect("bike");
    let stand = vehicles::vehicle_stand(&w, bike).expect("stand");
    on_street(&mut w, thief, stand);
    while !vehicles::steal(&mut w, thief, bike) {}
    // The thief gives up before the fence: the bike is parked where it stands.
    w.abort_plan(thief);
    assert!(matches!(asset(&w, bike).loc, AssetLoc::Parked(_)));
    assert!(asset(&w, bike).stolen);
    let stand = vehicles::vehicle_stand(&w, bike).expect("stand");
    on_street(&mut w, other, stand);
    assert!(!vehicles::may_steal(&w, other, bike), "a stolen vehicle is not stolen again");
    vehicles::recover_abandoned(&mut w);
    let x = asset(&w, bike);
    assert_eq!((x.loc, x.stolen, x.keeper, x.owner), (AssetLoc::Parked(home), false, None, Some(owner)));
    w.check_indices().expect("indices in step");
}

// ---------------------------------------------------------------------------
// Phase 2 fix round
// ---------------------------------------------------------------------------

#[test]
fn test_jailed_driver_office_car_goes_home() {
    let mut w = city();
    let office = w.buildings_of_kind(BuildingKind::SecurityOffice)[0];
    let corp = w.owner_of(office).expect("a Security corp");
    let guard = adults(&w).into_iter().find(|&a| !w.has::<Job>(a) && w.gang_of(a).is_none()).expect("jobless");
    demography::hire(&mut w, guard, office, Role::Guard);
    let car = assets::spawn_asset(&mut w, AssetKind::Car, 1, Some(corp), AssetLoc::Parked(office), 800);
    let run = road_run(&w, 12);
    on_street(&mut w, guard, run[3]);
    assets::set_keeper(&mut w, car, Some(guard));
    drive(&mut w, guard, car, run[12]);
    path_plan(&mut w, guard, &run[4..]);
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let until = w.tick + 5 * citysim::TICKS_PER_DAY;
    citysim::systems::law::sentence(&mut w, guard, citysim::Crime::Assault, until, jail);
    let x = asset(&w, car);
    assert!(matches!(x.loc, AssetLoc::Parked(b) if b != office), "left where the trip ended");
    assert_eq!(x.keeper, Some(guard), "still kept by the jailed guard");
    // The next midnight's pass clears the stale keeper (jailed) and recalls it.
    w.tick = citysim::TICKS_PER_DAY;
    assets::run(&mut w);
    let x = asset(&w, car);
    assert_eq!((x.loc, x.keeper), (AssetLoc::Parked(office), None), "home within a day");
    w.check_indices().expect("indices in step");
}

#[test]
fn test_leaving_the_gang_hands_back_its_bike() {
    let mut w = city();
    let gang = w.gangs()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let member = adults(&w).into_iter().find(|&a| w.gang_of(a).is_none() && !w.has::<Job>(a)).expect("recruit");
    citysim::systems::gang::enlist(&mut w, member, gang);
    let bike = assets::spawn_asset(&mut w, AssetKind::Motorcycle, 1, Some(gang), AssetLoc::Parked(hideout), 300);
    assets::set_keeper(&mut w, bike, Some(member));
    let run = road_run(&w, 8);
    on_street(&mut w, member, run[2]);
    drive(&mut w, member, bike, run[8]);
    path_plan(&mut w, member, &run[3..]);
    assert_eq!(w.comp::<Kit>(member).expect("kit").vehicle, Some(bike));
    citysim::systems::gang::leave(&mut w, member, "test");
    let x = asset(&w, bike);
    assert_eq!((x.loc, x.keeper, x.owner), (AssetLoc::Parked(hideout), None, Some(gang)));
    assert!(w.trips.is_empty());
    assert_eq!(w.comp::<Kit>(member).expect("kit").vehicle, None, "no longer theirs to ride");
    w.check_indices().expect("indices in step");
}

#[test]
fn test_chase_pin_consumed_on_abort() {
    let mut w = city();
    let who = adults(&w)[31];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("car");
    let run = road_run(&w, 8);
    on_street(&mut w, who, run[1]);
    w.chase_pins.insert(who);
    drive(&mut w, who, car, run[8]);
    path_plan(&mut w, who, &run[2..]);
    w.abort_plan(who);
    assert!(!w.chase_pins.contains(&who), "a pin lasts one trip, arrived or not");
}

#[test]
fn test_thief_steals_drives_and_fences_end_to_end() {
    let mut w = city();
    let people = adults(&w);
    let victim = people[40];
    let car = assets::grant(&mut w, victim, AssetKind::Car, 1).expect("car");
    let stand = vehicles::vehicle_stand(&w, car).expect("stand");
    let thief = people
        .iter()
        .copied()
        .find(|&a| a != victim && w.gang_of(a).is_none() && !w.has::<Job>(a))
        .expect("a jobless non-member");
    // A Road tile a few steps from the car's stand.
    let start = (0..w.map.h().min(256))
        .flat_map(|y| (0..w.map.w().min(256)).map(move |x| TilePos { x: x as u16, y: y as u16 }))
        .filter(|&t| w.map.tile_at(t) == TileKind::Road && (4..=8).contains(&t.manhattan(stand)))
        .min_by_key(|&t| (t.manhattan(stand), t.y, t.x))
        .expect("a road near the car");
    on_street(&mut w, thief, start);
    w.comp_mut::<citysim::Personality>(thief).expect("p").lawfulness = 0.1;
    w.comp_mut::<citysim::Skills>(thief).expect("s").stealth = 1.0;
    let today = w.day();
    w.comp_mut::<Brain>(thief).expect("b").last_dole_day = Some(today);
    let (gang, hideout) = {
        let h = citysim::systems::gang::hideout_for(&w, thief).expect("a Hideout");
        (w.owner_of(h).expect("its gang"), h)
    };
    w.comp_mut::<citysim::Gang>(gang).expect("g").treasury = 5000;
    // The who: a guard and a lawful agent may not steal.
    let ctx = citysim::PlanCtx::build(&w, thief, Some(car));
    assert!(ctx.vehicle_target && ActionKind::StealVehicle.allowed(&ctx));
    let guard = w.guards()[0];
    assert!(!ActionKind::StealVehicle.allowed(&citysim::PlanCtx::build(&w, guard, Some(car))));
    let lawful = people.iter().copied().find(|&a| a != thief && a != victim).expect("someone");
    w.comp_mut::<citysim::Personality>(lawful).expect("p").lawfulness = 0.4;
    assert!(!ActionKind::StealVehicle.allowed(&citysim::PlanCtx::build(&w, lawful, Some(car))));
    // Plan the Earn goal: the theft chain.
    w.comp_mut::<Brain>(thief).expect("b").current_goal = Some(GoalKind::Earn);
    plan::plan_for(&mut w, thief, GoalKind::Earn);
    let steps: Vec<ActionKind> =
        w.comp::<Brain>(thief).expect("b").plan.as_ref().expect("a plan").steps.iter().map(|s| s.action).collect();
    assert_eq!(
        steps,
        [
            ActionKind::GoTo(LocationKey::Vehicle),
            ActionKind::StealVehicle,
            ActionKind::GoTo(LocationKey::Hideout),
            ActionKind::Fence
        ],
        "walk to the car, steal it, drive to the Hideout, fence"
    );
    let value = asset(&w, car).value;
    let price = (w.config.vehicles.fence_frac * value as f32).round() as i64;
    let (coins0, gang0) = (coins(&w, thief), w.purse(Some(gang)));
    for _ in 0..3000 {
        exec_tick(&mut w);
        if w.comp::<Brain>(thief).expect("b").plan.is_none() {
            break;
        }
    }
    assert!(count(&w, EventKind::VehicleStolen) >= 1);
    let robbed = w
        .comp::<citysim::Memory>(victim)
        .expect("memory")
        .entries
        .iter()
        .any(|e| e.kind == citysim::MemoryKind::WasRobbed && e.subject == Some(thief));
    assert!(robbed, "Grand Theft raised against the thief (the owner's WasRobbed)");
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc, x.stolen, x.keeper), (Some(gang), AssetLoc::Parked(hideout), true, None));
    assert_eq!(coins(&w, thief) - coins0, price, "fence_frac x value");
    assert_eq!(gang0 - w.purse(Some(gang)), price, "paid by the gang (Flow::Sale)");
    w.check_indices().expect("indices in step");
}

// ---------------------------------------------------------------------------
// M13 review fixes
// ---------------------------------------------------------------------------

/// A lawless jobless non-member who has just stolen `people[40]`'s car and
/// parked it at the nearest Market it walked into (an Eat plan cut in before
/// the fence), the dole taken; a rich gang at its Hideout.
/// Returns (thief, car, gang, hideout, market).
fn thief_holding_car(w: &mut World) -> (EntityId, EntityId, EntityId, EntityId, EntityId) {
    let people = adults(w);
    let victim = people[40];
    let car = assets::grant(w, victim, AssetKind::Car, 1).expect("car");
    let stand = vehicles::vehicle_stand(w, car).expect("stand");
    let thief = people
        .iter()
        .copied()
        .find(|&a| a != victim && w.gang_of(a).is_none() && !w.has::<Job>(a) && w.spouse_of(a) != Some(victim))
        .expect("a jobless non-member");
    on_street(w, thief, stand);
    w.comp_mut::<citysim::Personality>(thief).expect("p").lawfulness = 0.1;
    w.comp_mut::<citysim::Skills>(thief).expect("s").stealth = 1.0;
    let today = w.day();
    w.comp_mut::<Brain>(thief).expect("b").last_dole_day = Some(today);
    let hideout = citysim::systems::gang::hideout_for(w, thief).expect("a Hideout");
    let gang = w.owner_of(hideout).expect("its gang");
    w.comp_mut::<citysim::Gang>(gang).expect("g").treasury = 5000;
    while !vehicles::steal(w, thief, car) {}
    let market = w.local(thief, BuildingKind::Market).expect("a Market");
    inside(w, thief, market);
    vehicles::end_trip(w, thief, false);
    assert_eq!(asset(w, car).loc, AssetLoc::Parked(market));
    assert_eq!(vehicles::stolen_held_by(w, thief), Some(car), "still the thief's to fence");
    (thief, car, gang, hideout, market)
}

fn plan_steps(w: &World, id: EntityId) -> Vec<ActionKind> {
    w.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).map_or(Vec::new(), |p| p.steps.iter().map(|s| s.action).collect())
}

/// Review finding 2: a non-member interrupted between the theft and the
/// fence still plans the fence on its next Earn.
#[test]
fn test_interrupted_thief_plans_the_fence() {
    let mut w = city();
    let (thief, car, gang, hideout, _) = thief_holding_car(&mut w);
    assert!(
        !citysim::utility::goals::already_satisfied(&w, thief, GoalKind::Earn, false),
        "a held vehicle is something to Earn by"
    );
    w.comp_mut::<Brain>(thief).expect("b").current_goal = Some(GoalKind::Earn);
    plan::plan_for(&mut w, thief, GoalKind::Earn);
    assert_eq!(plan_steps(&w, thief), [ActionKind::GoTo(LocationKey::Hideout), ActionKind::Fence]);
    for _ in 0..3000 {
        exec_tick(&mut w);
        if w.comp::<Brain>(thief).expect("b").plan.is_none() {
            break;
        }
    }
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc, x.keeper), (Some(gang), AssetLoc::Parked(hideout), None), "fenced");
    w.check_indices().expect("indices in step");
}

/// Review finding 3: the midnight recovery leaves a stolen vehicle whose
/// thief is free and still means to fence it; one given up is recovered.
#[test]
fn test_recovery_spares_a_pending_fence() {
    let mut w = city();
    let (thief, car, _, _, market) = thief_holding_car(&mut w);
    w.comp_mut::<Brain>(thief).expect("b").current_goal = Some(GoalKind::Earn);
    plan::plan_for(&mut w, thief, GoalKind::Earn);
    assert!(plan_steps(&w, thief).contains(&ActionKind::Fence));
    vehicles::recover_abandoned(&mut w);
    let x = asset(&w, car);
    assert_eq!((x.stolen, x.keeper, x.loc), (true, Some(thief), AssetLoc::Parked(market)), "the fence is pending");
    // Given up: recovered.
    w.abort_plan(thief);
    vehicles::recover_abandoned(&mut w);
    let x = asset(&w, car);
    assert_eq!((x.stolen, x.keeper), (false, None), "abandoned, recovered");
    w.check_indices().expect("indices in step");
}

/// Review finding 4: a financed vehicle in arrears is the lender's: the
/// trip's end tows it, and the gang does not fence it.
#[test]
fn test_fence_loses_to_the_tow() {
    let mut w = city();
    let people = adults(&w);
    let victim = people[40];
    let g = w.buildings_of_kind(BuildingKind::Garage)[0];
    let lender = w.owner_of(g).expect("a Garage owner");
    let car = assets::grant(&mut w, victim, AssetKind::Car, 1).expect("car");
    let repo_days = w.config.assets.repo_days;
    w.comp_mut::<Asset>(car).expect("asset").finance =
        Some(Finance { lender: Some(lender), remaining: 400, per_day: 5, arrears: repo_days });
    let stand = vehicles::vehicle_stand(&w, car).expect("stand");
    let thief = people
        .iter()
        .copied()
        .find(|&a| a != victim && w.gang_of(a).is_none() && w.spouse_of(a) != Some(victim))
        .expect("a non-member");
    on_street(&mut w, thief, stand);
    w.comp_mut::<citysim::Skills>(thief).expect("s").stealth = 1.0;
    let hideout = citysim::systems::gang::hideout_for(&w, thief).expect("a Hideout");
    let gang = w.owner_of(hideout).expect("its gang");
    w.comp_mut::<citysim::Gang>(gang).expect("g").treasury = 5000;
    while !vehicles::steal(&mut w, thief, car) {}
    // Driven straight into the Hideout: the trip is still running.
    inside(&mut w, thief, hideout);
    assert!(w.trips.contains_key(&thief));
    let (coins0, gang0) = (coins(&w, thief), w.purse(Some(gang)));
    assert!(!vehicles::fence(&mut w, thief), "the lender wins");
    let x = asset(&w, car);
    assert_eq!((x.owner, x.keeper, x.stolen), (Some(lender), None, false), "towed to the lender");
    assert!(matches!(x.loc, AssetLoc::Stock(_)));
    assert_eq!((coins(&w, thief), w.purse(Some(gang))), (coins0, gang0), "nobody paid");
    w.check_indices().expect("indices in step");
}

/// Review finding 6: the soft recall's count is per keeper: two one-night
/// absences by different keepers do not recall the car.
#[test]
fn test_away_days_reset_with_the_keeper() {
    let mut w = city();
    let office = w.buildings_of_kind(BuildingKind::SecurityOffice)[0];
    let corp = w.owner_of(office).expect("a Security corp");
    let mut pool = adults(&w).into_iter().filter(|&a| !w.has::<Job>(a) && w.gang_of(a).is_none());
    let (a, b) = (pool.next().expect("a"), pool.next().expect("b"));
    demography::hire(&mut w, a, office, Role::Guard);
    demography::hire(&mut w, b, office, Role::Guard);
    let away = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .find(|&m| w.owner_of(m) != Some(corp))
        .expect("a building the corp does not own");
    let car = assets::spawn_asset(&mut w, AssetKind::Car, 1, Some(corp), AssetLoc::Parked(away), 800);
    assets::set_keeper(&mut w, car, Some(a));
    vehicles::fleet_recall(&mut w);
    assert_eq!(asset(&w, car).away_days, 1, "one night away under the first keeper");
    // The first keeper hands it on where it stands.
    assets::set_keeper(&mut w, car, None);
    assets::set_keeper(&mut w, car, Some(b));
    assert_eq!(asset(&w, car).away_days, 0, "a new keeper starts afresh");
    vehicles::fleet_recall(&mut w);
    let x = asset(&w, car);
    assert_eq!((x.loc, x.keeper, x.away_days), (AssetLoc::Parked(away), Some(b), 1), "not recalled");
}

// ---------------------------------------------------------------------------
// The island pass phase 5 (docs/ISLAND_PASS.md I3; OPEN-165): a vehicle in
// motion with its riders aboard. Each fails on the base (d5269fc): a timed
// leg kept the body inside the building it left until the arrival tick,
// then put it at the door, hundreds of tiles on.
// ---------------------------------------------------------------------------

use citysim::systems::motion::{self, RideKind, Route};

/// A Coarse adult in his Home with his own car, and the Market or Bar
/// farthest from it (a ride of 100+ ticks), the GoTo planned.
fn coarse_driver_set_out(w: &mut World, nth: usize) -> (EntityId, EntityId, EntityId, TilePos) {
    let who = adults(w)
        .into_iter()
        .filter(|&a| w.comp::<citysim::Household>(a).and_then(|h| h.home).is_some() && w.gang_of(a).is_none())
        .nth(nth)
        .expect("a housed adult");
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    let car = assets::grant(w, who, AssetKind::Car, 1).expect("car");
    lod::set_lod(w, who, Lod::Coarse);
    w.leave_building(who);
    w.enter_building(who, home);
    let hd = w.comp::<Building>(home).expect("home").door;
    let target = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .chain(w.buildings_of_kind(BuildingKind::Bar))
        .copied()
        .max_by_key(|&b| (w.comp::<Building>(b).map_or(0, |bd| bd.door.manhattan(hd)), b))
        .expect("a Market or Bar");
    let origin = citysim::exec::walk_origin(w, who).expect("origin");
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
    (who, car, target, origin)
}

fn arrive_tick(w: &World, id: EntityId) -> u64 {
    match w.comp::<Brain>(id).expect("brain").exec {
        ExecState::GotoTimed { arrive_tick, .. } => arrive_tick,
        ref s => panic!("not a timed leg: {s:?}"),
    }
}

/// I3: mid-ride the driver's position is his car's, the car has left the
/// boarding tile and is out on the road; on arrival there is no jump (the
/// last tile aboard is the door's), and no tick moves him farther than the
/// vehicle's pace. Base: the body stood inside its Home for the whole ride.
#[test]
fn test_a_coarse_car_carries_its_driver_along_the_road() {
    let mut w = city();
    assert!(motion::on(&w), "the shipped config has [motion]");
    let (who, car, target, origin) = coarse_driver_set_out(&mut w, 3);
    exec_tick(&mut w);
    let arrive = arrive_tick(&w, who);
    let ticks = arrive - (w.tick - 1);
    assert!(ticks >= 40, "a long ride ({ticks} ticks)");
    assert_eq!(motion::vehicle_of(&w, who), Some(car), "aboard his car");
    let m = w.motions.get(&car).expect("the car in motion");
    assert_eq!((m.ride, m.driver, m.asset), (RideKind::Car, Some(who), true));
    assert!(matches!(m.route, Route::Timed { .. }));
    let door = w.comp::<Building>(target).expect("target").door;
    let mut last_aboard = origin;
    let mut moved_max = 0u32;
    let mut mid_checked = false;
    while w.tick <= arrive + 2 {
        let before = w.comp::<Position>(who).expect("pos").clone();
        exec_tick(&mut w);
        let now = w.comp::<Position>(who).expect("pos").clone();
        if motion::vehicle_of(&w, who) == Some(car) {
            let m = w.motions.get(&car).expect("motion");
            assert_eq!(now.tile, m.pos, "the rider is where the car is");
            assert_eq!(now.building, None, "out on the road");
            moved_max = moved_max.max(before.tile.manhattan(now.tile));
            last_aboard = now.tile;
            if !mid_checked && w.tick > arrive - ticks / 2 {
                assert!(m.pos.manhattan(origin) > 10, "the car left the boarding tile: {} vs {origin}", m.pos);
                mid_checked = true;
            }
        }
        w.check_indices().expect("the aboard links agree");
    }
    assert!(mid_checked, "seen mid-ride");
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(target), "arrived");
    // No landing jump: the last tile aboard is one tick's pace from the
    // door (the car makes 1-2 tiles a tick), where the way ends.
    assert!(moved_max <= 3, "no tick moved him more than the car's pace ({moved_max} tiles)");
    assert!(
        citysim::systems::law::chebyshev(last_aboard, door) <= moved_max.max(1),
        "no landing jump: last aboard {last_aboard}, door {door}"
    );
    assert!(w.motions.is_empty() && w.aboard.is_empty(), "the motion ends with its rider");
    assert!(matches!(asset(&w, car).loc, AssetLoc::Parked(b) if b == target), "parked at the destination");
}

/// I3, an interrupt mid-ride (the plan dropped, as an urgent Eat does, or
/// replaced without an abort): he steps off where the car stands, never
/// back on the boarding tile; the car parks by him. Base: left in his Home.
#[test]
fn test_an_interrupt_mid_ride_leaves_no_parked_body() {
    for replace in [false, true] {
        let mut w = city();
        let (who, car, _, origin) = coarse_driver_set_out(&mut w, 5);
        exec_tick(&mut w);
        let arrive = arrive_tick(&w, who);
        let half = w.tick + (arrive - w.tick) / 2;
        while w.tick < half {
            exec_tick(&mut w);
        }
        let at = w.motions.get(&car).expect("in motion").pos;
        if replace {
            // A new plan installed over the leg (no abort): the next step
            // starts a walk from where he is.
            let tick = w.tick;
            let b = w.comp_mut::<Brain>(who).expect("brain");
            b.plan = Some(Plan {
                goal: GoalKind::Idle,
                target: None,
                steps: vec![ActionInstance { action: ActionKind::Rest, target: None, tile: None }],
                started_tick: tick,
            });
            b.plan_step = 0;
            b.exec = ExecState::Idle;
            exec_tick(&mut w);
        } else {
            w.abort_plan(who);
        }
        let p = w.comp::<Position>(who).expect("pos").clone();
        assert_eq!(motion::vehicle_of(&w, who), None, "off the car");
        assert!(w.motions.is_empty(), "the car's motion ended");
        assert!(p.tile.manhattan(at) <= 2, "off where the car stood ({} vs {at})", p.tile);
        assert!(p.tile.manhattan(origin) > 10, "not back on the boarding tile");
        assert_ne!(p.building, w.comp::<citysim::Household>(who).and_then(|h| h.home), "not in his Home");
        if !replace {
            assert!(matches!(asset(&w, car).loc, AssetLoc::Parked(_)), "the car parked by him");
        }
        w.check_indices().expect("the aboard links agree");
    }
}

/// I3: a save taken mid-ride resumes byte for byte (the vehicle, its way
/// and its rider), and the loaded world arrives as the live one does.
#[test]
fn test_a_save_mid_ride_resumes_identically() {
    let mut w = city();
    let (who, car, target, _) = coarse_driver_set_out(&mut w, 7);
    exec_tick(&mut w);
    let arrive = arrive_tick(&w, who);
    for _ in 0..(arrive - w.tick) / 3 {
        exec_tick(&mut w);
    }
    assert!(w.motions.contains_key(&car), "mid-ride");
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text, "the save round-trips");
    assert_eq!(back.motions, w.motions);
    assert_eq!(back.aboard, w.aboard);
    while w.tick <= arrive + 1 {
        exec_tick(&mut w);
        exec_tick(&mut back);
        assert_eq!(
            back.comp::<Position>(who).expect("pos").tile,
            w.comp::<Position>(who).expect("pos").tile,
            "tick {}",
            w.tick
        );
    }
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w), "identical at the arrival");
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(target));
}

/// I3, LOD: a Coarse rider promoted to Full stays aboard and rides on; a
/// demotion to Statistical mid-ride steps him off where the car is (then
/// the off-screen snap), the motion gone.
#[test]
fn test_a_tier_change_mid_ride_keeps_the_rider_with_the_car() {
    let mut w = city();
    let (who, car, target, _) = coarse_driver_set_out(&mut w, 9);
    exec_tick(&mut w);
    let arrive = arrive_tick(&w, who);
    for _ in 0..(arrive - w.tick) / 3 {
        exec_tick(&mut w);
    }
    lod::set_lod(&mut w, who, Lod::Full);
    assert_eq!(motion::vehicle_of(&w, who), Some(car), "still aboard at Full");
    assert_eq!(arrive_tick(&w, who), arrive, "the same leg runs on");
    while w.tick <= arrive + 1 {
        exec_tick(&mut w);
    }
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(target), "arrived at Full");
    // The demotion.
    let mut w = city();
    let (who, car, _, _) = coarse_driver_set_out(&mut w, 9);
    exec_tick(&mut w);
    for _ in 0..20 {
        exec_tick(&mut w);
    }
    assert!(w.motions.contains_key(&car));
    lod::set_lod(&mut w, who, Lod::Statistical);
    assert_eq!(motion::vehicle_of(&w, who), None);
    assert!(w.motions.is_empty());
    w.check_indices().expect("the aboard links agree");
}

/// I3, the flyer: airborne, the body is where the flyer is, on the straight
/// line between the take-off and the door (base: on the take-off tile the
/// whole flight).
#[test]
fn test_a_flyer_carries_its_pilot_across() {
    let mut w = city();
    let (who, _, target, origin) = coarse_driver_set_out(&mut w, 11);
    // Swap the car for a flyer: park the car elsewhere, grant a flyer.
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    for v in w.vehicles.clone() {
        if asset(&w, v).owner == Some(who) {
            assets::set_loc(&mut w, v, AssetLoc::Parked(target));
        }
    }
    let flyer = assets::grant(&mut w, who, AssetKind::Flyer, 1).expect("flyer");
    assert_eq!(asset(&w, flyer).loc, AssetLoc::Parked(home));
    exec_tick(&mut w);
    let ExecState::Fly { arrive_tick, .. } = w.comp::<Brain>(who).expect("brain").exec else { panic!("flying") };
    assert!(arrive_tick > w.tick + 4, "a flight of some ticks");
    assert_eq!(w.motions.get(&flyer).map(|m| m.ride), Some(RideKind::Flyer));
    let door = w.comp::<Building>(target).expect("t").door;
    let mut seen_between = false;
    while w.tick <= arrive_tick + 1 {
        exec_tick(&mut w);
        if let Some(m) = w.motions.get(&flyer) {
            let p = w.comp::<Position>(who).expect("pos").tile;
            assert_eq!(p, m.pos);
            seen_between |= p != origin && p != door;
        }
    }
    assert!(seen_between, "seen in the air between the ends");
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(target));
}

/// I3, the cab (the Garage's off-screen cab, `OPEN-149`): a commuter's GoTo
/// to work rides a cab vehicle spawned for the ride, driverless, him aboard;
/// it moves, and the vehicle is gone with the ride.
#[test]
fn test_an_off_screen_cab_carries_its_fare() {
    let mut cfg = Config::load();
    cfg.jobs2.p_cab = 1.0;
    cfg.jobs2.cab_min_tiles = 40;
    if let Some(l) = cfg.legs.as_mut() {
        l.fare_wait_ticks = 0;
    }
    let mut w = World::new(42, cfg);
    let rider = adults(&w)
        .into_iter()
        .find(|&a| {
            let Some(j) = w.comp::<Job>(a) else { return false };
            let (Some(e), Some(h)) = (j.employer, w.comp::<citysim::Household>(a).and_then(|h| h.home)) else {
                return false;
            };
            w.gang_of(a).is_none()
                && w.comp::<Building>(e).zip(w.comp::<Building>(h)).is_some_and(|(x, y)| x.door.manhattan(y.door) >= 60)
        })
        .expect("a far commuter");
    set_coins(&mut w, rider, 5_000);
    let home = w.comp::<citysim::Household>(rider).and_then(|h| h.home).expect("home");
    // A cab driver at the Garage nearest his Home, off screen (no body
    // drives: the Garage's off-screen cab takes the ride).
    let hd = w.comp::<Building>(home).expect("home").door;
    let garage = w
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .copied()
        .min_by_key(|&g| (w.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(hd)), g))
        .expect("a Garage");
    let driver = adults(&w).into_iter().find(|&a| a != rider && !w.has::<Job>(a)).expect("a jobless adult");
    let role = w.config.trade_role("cab_driver").expect("the cab_driver trade");
    demography::hire(&mut w, driver, garage, role);
    lod::set_lod(&mut w, driver, Lod::Statistical);
    let work = w.comp::<Job>(rider).and_then(|j| j.employer).expect("work");
    lod::set_lod(&mut w, rider, Lod::Coarse);
    w.leave_building(rider);
    w.enter_building(rider, home);
    for v in w.vehicles.clone() {
        if asset(&w, v).owner == Some(rider) {
            assets::set_loc(&mut w, v, AssetLoc::Parked(work));
        }
    }
    let tick = w.tick;
    let b = w.comp_mut::<Brain>(rider).expect("brain");
    b.plan = Some(Plan {
        goal: GoalKind::Work,
        target: Some(work),
        steps: vec![ActionInstance {
            action: ActionKind::GoTo(LocationKey::Workplace),
            target: Some(work),
            tile: None,
        }],
        started_tick: tick,
    });
    b.plan_step = 0;
    b.exec = ExecState::Idle;
    exec_tick(&mut w);
    let state = w.comp::<Brain>(rider).map(|b| b.exec.clone());
    let cab =
        motion::vehicle_of(&w, rider).unwrap_or_else(|| panic!("aboard a cab: {state:?} {:?}", w.stats.current.jobs2));
    let m = w.motions.get(&cab).expect("the cab in motion").clone();
    assert_eq!((m.ride, m.driver, m.asset), (RideKind::Cab, None, false), "the off-screen cab, no asset");
    let arrive = arrive_tick(&w, rider);
    let start = m.pos;
    let mut moved = false;
    while w.tick <= arrive + 1 {
        exec_tick(&mut w);
        if let Some(m) = w.motions.get(&cab) {
            assert_eq!(w.comp::<Position>(rider).expect("pos").tile, m.pos);
            moved |= m.pos.manhattan(start) > 10;
        }
    }
    assert!(moved, "the cab moved off the boarding tile");
    assert_eq!(w.comp::<Position>(rider).expect("pos").building, Some(work), "at work");
    assert!(!w.is_alive(cab), "the cab's entity went with the ride");
    assert!(w.motions.is_empty() && w.aboard.is_empty());
}

/// I3, a Full driver steers his car (`Route::Driven`): aboard from the
/// first step, the car where he is; demoted to Coarse mid-drive, the car
/// runs his timed leg on from where it stands, him aboard, to the door.
#[test]
fn test_a_full_driver_steers_and_a_demotion_runs_on_timed() {
    let mut w = city();
    let (who, car, target, _) = coarse_driver_set_out(&mut w, 13);
    lod::set_lod(&mut w, who, Lod::Full);
    for _ in 0..12 {
        exec_tick(&mut w);
        assert_eq!(motion::vehicle_of(&w, who), Some(car), "aboard while he drives");
    }
    let m = w.motions.get(&car).expect("in motion");
    assert!(matches!(m.route, Route::Driven));
    assert_eq!(m.pos, w.comp::<Position>(who).expect("pos").tile, "the car is where he is");
    lod::set_lod(&mut w, who, Lod::Coarse);
    assert_eq!(motion::vehicle_of(&w, who), Some(car), "still aboard at Coarse");
    assert!(matches!(w.motions.get(&car).expect("motion").route, Route::Timed { .. }));
    let arrive = arrive_tick(&w, who);
    let mut last = w.comp::<Position>(who).expect("pos").tile;
    let mut step = 0;
    while w.tick <= arrive + 1 {
        exec_tick(&mut w);
        let now = w.comp::<Position>(who).expect("pos").clone();
        if now.building.is_none() {
            step = step.max(last.manhattan(now.tile));
            last = now.tile;
        }
        w.check_indices().expect("the aboard links agree");
    }
    assert!(step <= 3, "no jump on the timed run ({step} tiles)");
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(target), "arrived");
    assert!(w.motions.is_empty());
}

// ---------------------------------------------------------------------------
// The island pass phase 6 (docs/ISLAND_PASS.md I3; OPEN-165): the El as
// trains running the line. Each fails on the base (b68297e): an El rider
// stood on the board stop's tile for the whole ride (no train, nothing
// aboard), then landed on the alight stop, tens of tiles on.
// ---------------------------------------------------------------------------

/// The shipped city with the El on (the 2k fixture).
fn el_city() -> World {
    let mut cfg = Config::load();
    cfg.transit.enabled = true;
    World::new(42, cfg)
}

/// An adult whose Home is in the Sump near the Vat Line, and a Farm in the
/// Vats: a trip the El wins (`transit.rs`'s `sump_farmer`).
fn el_farmer(w: &World) -> (EntityId, EntityId, EntityId) {
    let farm = *w.buildings_of_kind(BuildingKind::Farm).first().expect("a Farm");
    let who = adults(w)
        .into_iter()
        .find(|&a| {
            w.comp::<citysim::Household>(a)
                .and_then(|h| h.home)
                .and_then(|h| w.comp::<Building>(h))
                .is_some_and(|b| b.door.y >= 125 && b.door.y <= 135 && b.door.x < 120)
        })
        .expect("a Sump resident near the Vat Line");
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    (who, home, farm)
}

/// `who` inside `home` at 08:00 on day 1, solvent and law-abiding, with a
/// plan of one GoTo to `target`.
fn el_set_out(w: &mut World, who: EntityId, home: EntityId, target: EntityId, lod_: Lod) {
    w.tick = 1440 + 8 * 60;
    lod::set_lod(w, who, lod_);
    w.leave_building(who);
    w.enter_building(who, home);
    w.comp_mut::<Wallet>(who).expect("wallet").coins = 50;
    let p = w.comp_mut::<citysim::Personality>(who).expect("personality");
    p.lawfulness = 0.6;
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

/// Where a body stands: its building's door inside one, else its tile.
fn stands(w: &World, id: EntityId) -> TilePos {
    let p = w.comp::<Position>(id).expect("pos");
    match p.building.and_then(|b| w.comp::<Building>(b)) {
        Some(b) => b.door,
        None => p.tile,
    }
}

/// Run until `who` is aboard a vehicle between two stops (off every stop
/// tile): the vehicle.
fn until_mid_track(w: &mut World, who: EntityId) -> EntityId {
    for _ in 0..1_500 {
        exec_tick(w);
        let Some(v) = motion::vehicle_of(w, who) else { continue };
        let at = w.comp::<Position>(who).expect("pos").tile;
        if !w.transit.net.stops.contains(&at) {
            return v;
        }
    }
    panic!("never aboard a train between stops (exec {:?})", w.comp::<Brain>(who).map(|b| b.exec.clone()));
}

/// I3: a train is a persistent vehicle on the line: aboard it, the rider's
/// position is the train's, the train moves along the track between stops,
/// and it runs on after he gets off.
#[test]
fn test_a_train_carries_its_riders_between_stops() {
    let mut w = el_city();
    let (who, home, farm) = el_farmer(&w);
    el_set_out(&mut w, who, home, farm, Lod::Full);
    let v = until_mid_track(&mut w, who);
    assert_eq!(format!("{:?}", w.motions.get(&v).expect("in motion").ride), "Train");
    let start = w.motions[&v].pos;
    let mut far = 0;
    while motion::vehicle_of(&w, who) == Some(v) {
        let m = &w.motions[&v];
        assert_eq!(m.pos, w.comp::<Position>(who).expect("pos").tile, "aboard: the rider is where the train is");
        assert!(m.riders.contains(&who));
        far = far.max(m.pos.manhattan(start));
        w.check_indices().expect("the aboard links agree");
        exec_tick(&mut w);
    }
    assert!(far >= 10, "the train moved along the line ({far} tiles)");
    let at = w.motions.get(&v).expect("the train runs on after its rider is off").pos;
    for _ in 0..5 {
        exec_tick(&mut w);
    }
    assert_ne!(w.motions[&v].pos, at, "the empty train keeps running");
}

/// I3: the rider walks to the stop, waits on its platform, boards, rides
/// and alights at his stop, and walks on: no tick moves him farther than a
/// train's pace, none parks him an hour on one tile.
#[test]
fn test_an_el_rider_waits_boards_rides_and_alights_without_a_jump() {
    let mut w = el_city();
    let (who, home, farm) = el_farmer(&w);
    el_set_out(&mut w, who, home, farm, Lod::Full);
    let mut last = stands(&w, who);
    let (mut step, mut since, mut stand, mut waited, mut aboard) = (0u32, w.tick, 0u64, 0u32, 0u32);
    for _ in 0..2_000 {
        exec_tick(&mut w);
        let now = stands(&w, who);
        step = step.max(last.manhattan(now));
        if now != last {
            since = w.tick;
        }
        stand = stand.max(w.tick - since);
        last = now;
        if let Some(ExecState::Ride { depart, .. }) = w.comp::<Brain>(who).map(|b| b.exec.clone()) {
            if w.tick < depart && motion::vehicle_of(&w, who).is_none() {
                assert!(w.transit.net.stops.contains(&now), "waits on the platform");
                waited += 1;
            }
        }
        if motion::vehicle_of(&w, who).is_some() {
            aboard += 1;
        }
        if w.comp::<Position>(who).expect("pos").building == Some(farm) {
            break;
        }
    }
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(farm), "arrived");
    assert_eq!(w.stats.current.transit.rides_full, 1, "rode");
    assert!(aboard > 0, "aboard a train");
    assert!(step <= 12, "no jump ({step} tiles in a tick)");
    assert!(stand < 60, "no parked hour ({stand} ticks on one tile)");
    println!("waited {waited} ticks, aboard {aboard}, max step {step}, longest stand {stand}");
}

/// I3: a cut line stops its trains where they are; a rider aboard gets off
/// at the nearest stop (walking the track to it, no jump) and walks on.
#[test]
fn test_a_cut_line_stops_its_trains_and_sets_riders_down_at_a_stop() {
    let mut w = el_city();
    let (who, home, farm) = el_farmer(&w);
    el_set_out(&mut w, who, home, farm, Lod::Full);
    let v = until_mid_track(&mut w, who);
    let line = w.rides.get(&who).expect("a ride").line;
    citysim::systems::transit::cut_line(&mut w, line, 1).expect("cut");
    let held = w.motions[&v].pos;
    let mut last = stands(&w, who);
    let (mut step, mut at_stop) = (0u32, false);
    for _ in 0..2_000 {
        exec_tick(&mut w);
        assert_eq!(w.motions.get(&v).map(|m| m.pos), Some(held), "the cut line's train stands still");
        assert_ne!(motion::vehicle_of(&w, who), Some(v), "off the stopped train");
        let now = stands(&w, who);
        step = step.max(last.manhattan(now));
        last = now;
        at_stop |= w.transit.net.stops.contains(&now);
        if w.comp::<Position>(who).expect("pos").building == Some(farm) {
            break;
        }
    }
    assert!(at_stop, "set down at a stop");
    assert!(step <= 12, "no jump ({step} tiles in a tick)");
    assert_eq!(w.comp::<Position>(who).expect("pos").building, Some(farm), "walked on to the Farm");
}

/// I3: interrupted between stops, a rider stays aboard to the next stop and
/// gets off there (never mid-track).
#[test]
fn test_an_interrupt_aboard_a_train_gets_off_at_the_next_stop() {
    let mut w = el_city();
    let (who, home, farm) = el_farmer(&w);
    el_set_out(&mut w, who, home, farm, Lod::Full);
    let v = until_mid_track(&mut w, who);
    w.abort_plan(who);
    let mut ticks = 0;
    while motion::vehicle_of(&w, who) == Some(v) {
        assert_eq!(w.comp::<Position>(who).expect("pos").tile, w.motions[&v].pos, "carried on to the stop");
        exec_tick(&mut w);
        ticks += 1;
        assert!(ticks < 200, "never off");
    }
    let at = w.comp::<Position>(who).expect("pos").tile;
    assert!(w.transit.net.stops.contains(&at), "off at a stop, not mid-track ({at:?})");
    w.check_indices().expect("the aboard links agree");
}

/// I3: a save taken while a rider is aboard a train between stops resumes
/// byte for byte.
#[test]
fn test_a_save_mid_train_ride_resumes_identically() {
    let mut w = el_city();
    let (who, home, farm) = el_farmer(&w);
    el_set_out(&mut w, who, home, farm, Lod::Coarse);
    until_mid_track(&mut w, who);
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text, "the load is the save");
    for _ in 0..300 {
        exec_tick(&mut w);
        exec_tick(&mut back);
        assert_eq!(w.comp::<Position>(who).map(|p| p.tile), back.comp::<Position>(who).map(|p| p.tile));
    }
    assert_eq!(citysim::save::to_ron(&w), citysim::save::to_ron(&back));
}
