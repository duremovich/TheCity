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

    // By 23:30 they are home again, and a day of wages is owed.
    w.run_ticks(1410 - 800);
    let pos = w.comp::<Position>(farmer).expect("pos");
    assert_eq!(pos.building, Some(home), "farmer should be home at 23:30");
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
            assert!(!tiles.contains(&p.tile) || b.kind == BuildingKind::Home && b.occupants.len() > 6);
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
