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
    World::new(42, Config::load())
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
            let t = TilePos { x: x as u8, y: y as u8 };
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
    let mut cfg = Config::load();
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
    let mut cfg = Config::load();
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
            let mut c = Config::load();
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
    assert_eq!(security::contest_p(3, 1, 0.25), 0.95, "clamped");
    assert_eq!(security::contest_p(1, 3, 0.25), 0.05, "clamped");
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
    let mut cfg = Config::load();
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
    assert_eq!(b.shop_pick, Some(ShopPick { kind: AssetKind::Car, tier: 1, used: None }));
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
