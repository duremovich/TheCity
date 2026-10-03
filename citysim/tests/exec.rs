//! M1: pathfinding, flow fields, movement and the daily routine.

use rand::{Rng, SeedableRng};

use citysim::exec::flowfield::FlowField;
use citysim::exec::pathfind;
use citysim::{
    Brain, Building, BuildingKind, Config, Job, Lod, Position, Role, TileKind, TilePos, World, MAP_H, MAP_W,
    TICKS_PER_DAY,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
}

fn random_walkable(w: &World, rng: &mut rand_chacha::ChaCha8Rng) -> TilePos {
    loop {
        let p = TilePos { x: rng.random_range(0..MAP_W as u8), y: rng.random_range(0..MAP_H as u8) };
        // stay on the street: building interiors are only reachable through their door
        if w.map.tile_at(p) == TileKind::Road || w.map.tile_at(p) == TileKind::Ground {
            let inside = w.map.buildings.iter().any(|b| b.rect.contains(p));
            if !inside {
                return p;
            }
        }
    }
}

#[test]
fn test_flowfield_matches_astar_length() {
    let w = world(1);
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(99);
    let doors: Vec<TilePos> = w.map.buildings.iter().map(|b| b.door).collect();
    for i in 0..20 {
        let start = random_walkable(&w, &mut rng);
        let door = doors[i % doors.len()];
        let field = FlowField::build(&w.map, door);
        let descent = field.descend(start);
        let astar = pathfind::astar(&w.map, start, door, 100_000).expect("reachable");
        assert_eq!(descent.last(), Some(&door));
        let dc = pathfind::path_cost(&w.map, &descent);
        let ac = pathfind::path_cost(&w.map, &astar);
        assert!((dc - ac).abs() < 1e-3, "start {start} door {door}: field {dc} vs A* {ac}");
        assert!((field.cost_from(start) - ac).abs() < 1e-3);
        assert_eq!(descent.len(), astar.len(), "start {start} door {door}");
    }
}

#[test]
fn test_pathfinder_never_crosses_wall_or_water() {
    let w = world(1);
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
    let mut found = 0;
    for _ in 0..200 {
        let a = random_walkable(&w, &mut rng);
        let b = random_walkable(&w, &mut rng);
        let Some(path) = pathfind::astar(&w.map, a, b, 100_000) else { continue };
        found += 1;
        let mut prev = a;
        for &p in &path {
            assert!(w.map.walkable(p), "{p} is {:?}", w.map.tile_at(p));
            assert_eq!(prev.manhattan(p), 1, "non-adjacent step {prev} -> {p}");
            prev = p;
        }
        assert_eq!(path.last().copied(), Some(b));
    }
    assert!(found >= 190, "only {found} of 200 random street pairs were connected");
}

#[test]
fn test_full_agent_walks_to_work_and_produces() {
    let mut w = world(5);
    w.config.lod.force = Some(Lod::Full);
    let farmer =
        w.citizens().into_iter().find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Farmer)).expect("farmer");
    let farm = w.comp::<Job>(farmer).expect("job").employer.expect("employer");
    let home = w.comp::<Position>(farmer).expect("pos").building.expect("at home");
    assert_ne!(home, farm);
    // broke and fed: Work beats Eat and Idle all day
    w.comp_mut::<citysim::Wallet>(farmer).expect("wallet").coins = 0;
    w.comp_mut::<citysim::Needs>(farmer).expect("needs").hunger = 1.0;

    // By mid-shift the farmer is inside the farm and working.
    w.run_ticks(800);
    let pos = w.comp::<Position>(farmer).expect("pos");
    assert_eq!(pos.building, Some(farm), "farmer should be at the farm at 13:20");
    assert!(w.comp::<Building>(farm).expect("farm").rect.contains(pos.tile));
    let brain = w.comp::<Brain>(farmer).expect("brain");
    assert!(
        matches!(brain.exec, citysim::ExecState::Use { kind: citysim::ActionKind::FarmWork, .. }),
        "{:?}",
        brain.exec
    );

    // By 23:30 they are home again (or on the last stretch of the walk there).
    w.run_ticks(1410 - 800);
    let pos = w.comp::<Position>(farmer).expect("pos");
    let heading_home = matches!(
        &w.comp::<Brain>(farmer).expect("brain").exec,
        citysim::ExecState::Goto { target, .. } if target.building == Some(home)
    );
    assert!(pos.building == Some(home) || heading_home, "farmer should be home at 23:30: {pos:?}");
    let job = w.comp::<Job>(farmer).expect("job");
    assert_eq!(job.last_shift_day, Some(0));
    assert_eq!(job.days_unpaid, 0, "wages were collected at the Hall in the evening");
    let paid =
        w.comp::<citysim::Memory>(farmer).expect("memory").entries.iter().any(|e| e.kind == citysim::MemoryKind::Paid);
    assert!(paid, "a Paid memory from CollectWage");
}

#[test]
fn test_building_capacity_is_respected() {
    let mut w = world(11);
    w.run_ticks(TICKS_PER_DAY * 2);
    for id in w.with::<Building>() {
        let b = w.comp::<Building>(id).expect("b");
        assert!(b.occupants.len() <= usize::from(b.capacity), "{} has {} > {}", b.kind, b.occupants.len(), b.capacity);
        // every occupant agrees it is inside, on a distinct interior tile
        let mut tiles = Vec::new();
        for &o in &b.occupants {
            let p = w.comp::<Position>(o).expect("pos");
            assert_eq!(p.building, Some(id));
            assert!(b.rect.contains(p.tile));
            assert!(
                !tiles.contains(&p.tile),
                "{} #{}: two occupants on {} (occupants {:?})",
                b.kind,
                id.index,
                p.tile,
                b.occupants
            );
            tiles.push(p.tile);
        }
    }
    // and nobody claims to be inside a building that does not list them
    for id in w.citizens() {
        let p = w.comp::<Position>(id).expect("pos");
        if let Some(b) = p.building {
            assert!(w.comp::<Building>(b).expect("b").occupants.contains(&id));
        }
    }
}

#[test]
fn test_lod_assigns_fifty_full() {
    let mut w = world(2);
    w.run_ticks(1);
    let full = w.citizens().into_iter().filter(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Full)).count();
    assert_eq!(full, w.config.lod.max_full);
    w.config.lod.force = Some(Lod::Coarse);
    w.run_ticks(60);
    let full = w.citizens().into_iter().filter(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Full)).count();
    assert_eq!(full, 0);
}

#[test]
fn test_night_guard_works_both_halves_of_the_shift() {
    let mut w = world(8);
    let guard = w
        .citizens()
        .into_iter()
        .find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard && j.shifts[0].0 == 1260))
        .expect("a night guard");
    let jail = w.comp::<Job>(guard).expect("job").employer.expect("jail");
    // broke, fed and content: Work outscores Eat and Socialise on both evenings
    w.comp_mut::<citysim::Wallet>(guard).expect("wallet").coins = 0;
    let n = w.comp_mut::<citysim::Needs>(guard).expect("needs");
    n.hunger = 1.0;
    n.belonging = 1.0;
    let working = |w: &World| {
        w.comp::<Position>(guard).expect("pos").building == Some(jail)
            && matches!(
                w.comp::<Brain>(guard).expect("brain").exec,
                citysim::ExecState::Use { kind: citysim::ActionKind::GuardJail, .. }
            )
    };
    // Day 0 22:00: the shift that starts at 21:00 is in progress.
    w.run_ticks(1320);
    assert!(working(&w), "22:00 day 0");
    // Day 1 03:00: still the same shift.
    w.run_ticks(TICKS_PER_DAY - 1320 + 180);
    assert!(working(&w), "03:00 day 1");
    // Day 1 22:00: the next shift, not "already worked today".
    w.run_ticks(1320 - 180);
    assert!(working(&w), "22:00 day 1");
    let job = w.comp::<Job>(guard).expect("job");
    // shifts are marked at their end: day 0's shift (ended 06:00 day 1) is the last complete one
    assert_eq!(job.last_shift_day, Some(0));
    assert!(job.days_unpaid <= 1, "one wage day per shift, not per segment: {}", job.days_unpaid);
}

#[test]
fn test_buy_quantity_capped_by_market_stock() {
    let mut w = world(9);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    w.comp_mut::<Building>(market).expect("market").stock_food = 2;
    let rich = w.citizens()[0];
    w.comp_mut::<citysim::Wallet>(rich).expect("wallet").coins = 100;
    w.comp_mut::<citysim::Inventory>(rich).expect("inv").food = 0;
    assert_eq!(citysim::systems::economy::buy_quantity(&w, rich), 2);
    w.comp_mut::<Building>(market).expect("market").stock_food = 0;
    assert_eq!(citysim::systems::economy::buy_quantity(&w, rich), 0);
}

#[test]
fn test_homeless_agent_sleeps_on_the_street() {
    let mut w = world(10);
    let id = w.citizens()[0];
    w.comp_mut::<citysim::Household>(id).expect("hh").home = None;
    w.leave_building(id);
    w.comp_mut::<citysim::Needs>(id).expect("needs").energy = 0.2;
    w.run_ticks(5);
    let brain = w.comp::<Brain>(id).expect("brain");
    assert!(matches!(brain.exec, citysim::ExecState::Use { kind: citysim::ActionKind::Sleep, .. }), "{:?}", brain.exec);
    assert!(w.comp::<Position>(id).expect("pos").building.is_none());
}

#[test]
fn test_short_treasury_does_not_make_workers_quit_in_a_day() {
    let mut w = world(12);
    w.treasury_mut().expect("treasury").coins = 0;
    w.levers.dole_per_day = 0;
    let workers = w.citizens().into_iter().filter(|&id| w.has::<Job>(id)).count();
    assert_eq!(workers, 40);
    w.run_ticks(TICKS_PER_DAY * 2);
    let still = w.citizens().into_iter().filter(|&id| w.has::<Job>(id)).count();
    assert_eq!(
        still, 40,
        "nobody quits after two unpaid days; the rule is seven days or three Unpaid memories in a week"
    );
    for id in w.citizens() {
        let Some(job) = w.comp::<Job>(id) else { continue };
        assert!(job.days_unpaid <= 2, "{}", job.days_unpaid);
    }
}

#[test]
fn test_idle_agent_leaves_the_hall() {
    use citysim::exec::routine;
    use citysim::{ActionKind, GoalKind, LocationKey};
    let mut w = world(13);
    let id = w.citizens().into_iter().find(|&id| !w.has::<Job>(id)).expect("unemployed");
    let hall = w.building_of_kind(BuildingKind::Hall).expect("hall");
    w.leave_building(id);
    w.enter_building(id, hall);
    let plan = routine::plan_for_goal(&w, id, GoalKind::Idle).expect("idle plan");
    assert_eq!(plan.steps[0].action, ActionKind::GoTo(LocationKey::Home), "idle inside the Hall goes home: {plan:?}");
    // and a homeless idler steps outside rather than squatting
    w.comp_mut::<citysim::Household>(id).expect("hh").home = None;
    let plan = routine::plan_for_goal(&w, id, GoalKind::Idle).expect("idle plan");
    assert_eq!(plan.steps[0].action, ActionKind::Wander);
}
