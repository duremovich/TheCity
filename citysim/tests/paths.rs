//! Scale S3a (docs/SCALE_20K.md): paths on big maps. Above `[exec]
//! flow_field_max_tiles` a Full walk follows an exact A* path
//! (`exec::bigpath`) instead of a per-door flow field; below it the flow
//! fields stay, inside a byte budget. The El's per-tile stop shortlist is a
//! per-cell candidate index, not a `w × h × 8` table.

use rand::{Rng, SeedableRng};

use citysim::exec::bigpath;
use citysim::exec::flowfield::{FlowCache, FlowField};
use citysim::map::Map;
use citysim::{Building, Config, EntityId, Position, TileKind, TilePos, World};

/// A random `w × h` map: mostly Road and Ground, some Mountain,
/// Farmland-free walls, water and rubble (Door and Farmland need a building around them).
fn random_map(rng: &mut rand_chacha::ChaCha8Rng, w: usize, h: usize) -> Map {
    random_maps(rng, w, h).0
}

/// A random map and a more open one: the same with a third of its walls
/// Ground and no rubble (a map the first only closed).
fn random_maps(rng: &mut rand_chacha::ChaCha8Rng, w: usize, h: usize) -> (Map, Map) {
    let mut text = format!("{w} {h}\n");
    let mut open = text.clone();
    for _ in 0..h {
        for _ in 0..w {
            let r: f32 = rng.random();
            let ch = match r {
                r if r < 0.40 => '=',
                r if r < 0.64 => '.',
                // Scale S3c: Mountain (15 tenths, the dearest step).
                r if r < 0.72 => 'm',
                r if r < 0.92 => '#',
                _ => '~',
            };
            text.push(ch);
            open.push(if ch == '#' && rng.random_range(0..3) == 0 { '.' } else { ch });
        }
        text.push('\n');
        open.push('\n');
    }
    text.push('\n');
    open.push('\n');
    let mut map = Map::parse(&text);
    for _ in 0..(w * h / 40) {
        let p = TilePos::new(rng.random_range(0..w as u16), rng.random_range(0..h as u16));
        map.blocked.insert(p);
    }
    (map, Map::parse(&open))
}

/// Plain Dijkstra from `start` over 4-neighbours, a tile's entry cost in
/// tenths (`bigpath::cost_tenths`): the reference.
fn dijkstra(map: &Map, start: TilePos) -> Vec<u64> {
    let (w, h) = (map.w(), map.h());
    let mut dist = vec![u64::MAX; w * h];
    let mut open = std::collections::BinaryHeap::new();
    dist[map.index(start)] = 0;
    open.push(std::cmp::Reverse((0u64, map.index(start))));
    while let Some(std::cmp::Reverse((d, i))) = open.pop() {
        if d > dist[i] {
            continue;
        }
        let p = TilePos::new((i % w) as u16, (i / w) as u16);
        for n in map.neighbours4(p) {
            if !map.walkable(n) {
                continue;
            }
            let nd = d + u64::from(bigpath::cost_tenths(map.tile_at(n)));
            let j = map.index(n);
            if nd < dist[j] {
                dist[j] = nd;
                open.push(std::cmp::Reverse((nd, j)));
            }
        }
    }
    dist
}

fn tenths(map: &Map, path: &[TilePos]) -> u64 {
    path.iter().map(|&t| u64::from(bigpath::cost_tenths(map.tile_at(t)))).sum()
}

fn walkable_tile(map: &Map, rng: &mut rand_chacha::ChaCha8Rng) -> TilePos {
    loop {
        let p = TilePos::new(rng.random_range(0..map.w() as u16), rng.random_range(0..map.h() as u16));
        if map.walkable(p) {
            return p;
        }
    }
}

/// Property: on random maps every big-map path is a legal walk (adjacent
/// steps on walkable tiles, ending at the goal) whose cost equals the plain
/// Dijkstra distance, and it is `None` exactly where Dijkstra never reaches:
/// with no landmarks, with landmarks of the map, and with landmarks of a
/// more open map (stale after walls went up: still exact).
#[test]
fn test_big_paths_equal_dijkstra_on_random_maps() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2026);
    let (mut found, mut none) = (0, 0);
    for m in 0..200 {
        let (w, h) = (rng.random_range(8..48), rng.random_range(8..40));
        let (map, open) = random_maps(&mut rng, w, h);
        let labels = bigpath::labels(&map);
        let marks = [bigpath::Landmarks::none(), bigpath::Landmarks::build(&map), bigpath::Landmarks::build(&open)];
        for q in 0..30 {
            let start = walkable_tile(&map, &mut rng);
            let goal = walkable_tile(&map, &mut rng);
            let truth = dijkstra(&map, start)[map.index(goal)];
            let got = bigpath::find(&map, &labels, &marks[q % 3], start, goal);
            match got {
                None => {
                    assert_eq!(truth, u64::MAX, "map {m}: {start} -> {goal} reachable at {truth}, no path");
                    none += 1;
                }
                Some(path) => {
                    assert_ne!(truth, u64::MAX, "map {m}: {start} -> {goal} has a path but Dijkstra never got there");
                    let mut prev = start;
                    for &p in &path {
                        assert!(map.walkable(p), "map {m}: {p} is {:?}", map.tile_at(p));
                        assert_eq!(prev.manhattan(p), 1, "map {m}: a jump {prev} -> {p}");
                        prev = p;
                    }
                    assert_eq!(path.last().copied().unwrap_or(start), goal);
                    assert_eq!(tenths(&map, &path), truth, "map {m}: {start} -> {goal}");
                    found += 1;
                }
            }
        }
    }
    assert!(found > 400 && none > 50, "both outcomes exercised: {found} paths, {none} unreachable");
}

/// The big-map path costs what the 2k city's flow-field descent costs (in
/// tenths, the move costs' exact unit), on the shipped map: where a walk
/// takes the A* path instead of the field it is never longer.
#[test]
fn test_big_paths_equal_flow_field_cost_on_the_shipped_map() {
    let w = World::new(42, Config::load());
    let labels = bigpath::labels(&w.map);
    let marks = bigpath::Landmarks::build(&w.map);
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
    let doors: Vec<TilePos> = w.map.buildings.iter().map(|b| b.door).collect();
    for i in 0..40 {
        let door = doors[(i * 37) % doors.len()];
        let field = FlowField::build(&w.map, door);
        let start = loop {
            let p = walkable_tile(&w.map, &mut rng);
            if !w.map.buildings.iter().any(|b| b.rect.contains(p)) {
                break p;
            }
        };
        let descent = field.descend(start);
        let path = bigpath::find(&w.map, &labels, &marks, start, door).expect("reachable");
        assert_eq!(tenths(&w.map, &path), tenths(&w.map, &descent), "{start} -> {door}");
    }
}

/// The flow cache holds at most `budget` bytes of fields, whatever the
/// configured count, and the 2k city's 512 fields of 49 KB fit the default
/// budget untouched.
#[test]
fn test_flow_cache_stays_within_its_byte_budget() {
    let tiles = 600 * 400;
    let budget = 8 * 1024 * 1024;
    let cap = FlowCache::cap_for(512, budget, tiles);
    assert_eq!(cap, budget / tiles);
    let mut cache = FlowCache::default();
    for i in 0..200u32 {
        let field = FlowField { next: vec![0; tiles], w: 600, door: Some(TilePos::new(0, 0)) };
        cache.insert(EntityId { index: i, generation: 0 }, field, cap);
        assert!(cache.bytes() <= budget, "after {i}: {} bytes", cache.bytes());
    }
    assert_eq!(cache.len(), cap);
    let cfg = Config::load();
    assert_eq!(FlowCache::cap_for(cfg.exec.flow_field_cache, cfg.exec.flow_field_budget_mb << 20, 256 * 192), 512);
}

/// The big-map pathing state (search scratch, labels, landmarks) is bounded
/// per tile: at the big island's 3,000 × 1,875 it stays under 200 MB, and
/// a world on it holds no flow field.
#[test]
fn test_big_path_memory_is_bounded_per_tile() {
    let per_tile = bigpath::BYTES_PER_TILE;
    assert!(per_tile * 3_000 * 1_875 < 200 << 20, "{per_tile} bytes a tile");
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(3);
    let map = random_map(&mut rng, 64, 48);
    let labels = bigpath::labels(&map);
    let marks = bigpath::Landmarks::build(&map);
    let (a, b) = (walkable_tile(&map, &mut rng), walkable_tile(&map, &mut rng));
    let _ = bigpath::find(&map, &labels, &marks, a, b);
    let held = bigpath::scratch_bytes() + labels.len() * 4 + marks.bytes();
    assert!(held <= per_tile * 64 * 48 + 64 * 1024, "{held} bytes");
}

/// Above the threshold a Full walk follows the A* path: forced on the 2k
/// map (threshold 0), a Full agent sent to a far building arrives, by a
/// path as cheap as the flow field's.
#[test]
fn test_full_walk_on_big_paths_arrives() {
    let mut cfg = Config::load();
    cfg.exec.flow_field_max_tiles = 0;
    cfg.lod.force = Some(citysim::Lod::Full);
    let mut w = World::new(42, cfg);
    assert!(w.big_paths());
    // Each walker seen on a path to a building, and whether it got inside.
    let mut walks: std::collections::BTreeMap<EntityId, (EntityId, bool)> = Default::default();
    for _ in 0..600 {
        w.run_ticks(1);
        for a in w.citizens() {
            let Some(b) = w.comp::<citysim::Brain>(a) else { continue };
            if let citysim::ExecState::Goto { target, path, .. } = &b.exec {
                if let (Some(dest), false) = (target.building, path.is_empty()) {
                    assert_eq!(path[0], w.comp::<Building>(dest).expect("dest").door, "the path ends at the door");
                    walks.entry(a).or_insert((dest, false));
                }
            }
            if let (Some((dest, inside)), Some(p)) = (walks.get_mut(&a), w.comp::<Position>(a)) {
                *inside |= p.building == Some(*dest);
            }
        }
    }
    assert!(w.flow_fields.is_empty(), "no flow field is built above the threshold");
    let arrived = walks.values().filter(|(_, inside)| *inside).count();
    eprintln!("{arrived} of {} walkers arrived", walks.len());
    assert!(walks.len() > 200 && arrived * 10 > walks.len() * 8, "{arrived} of {} walkers arrived", walks.len());
}

/// The landmarks are a pure function of the tiles they were measured on:
/// rubble (not a tile change) keeps them; a wall going up (a closure) keeps
/// them too (review finding 10: still admissible), the world noting their
/// tiles for a save (`landmark_base`); a wall coming down (an opening)
/// rebuilds them on the new tiles.
#[test]
fn test_landmarks_follow_the_tiles() {
    let mut cfg = Config::load();
    cfg.exec.flow_field_max_tiles = 0;
    let mut w = World::new(42, cfg);
    let a = w.flow_fields.landmarks(&w.map);
    assert!(a.same_tiles(&w.map) && a.points.len() == bigpath::LANDMARKS);
    let roads: Vec<TilePos> = (0..w.map.h() as u16)
        .flat_map(|y| (0..w.map.w() as u16).map(move |x| TilePos::new(x, y)))
        .filter(|&t| w.map.tile_at(t) == TileKind::Road)
        .take(2)
        .collect();
    w.map.blocked.insert(roads[0]);
    w.invalidate_flow_fields();
    assert!(std::sync::Arc::ptr_eq(&a, &w.flow_fields.landmarks(&w.map)), "rubble keeps them");
    assert!(w.landmark_base.is_none());
    w.map.set_tile(roads[1], TileKind::Wall);
    w.invalidate_flow_fields();
    let b = w.flow_fields.landmarks(&w.map);
    assert!(std::sync::Arc::ptr_eq(&a, &b) && !b.same_tiles(&w.map), "a wall going up keeps them");
    assert!(w.landmark_base.as_ref().is_some_and(|t| t.0 == *a.base_tiles()), "noted for a save");
    assert!(w.big_path(roads[0], roads[1]).is_none(), "rubble and wall ends are unreachable");
    // A wall coming down: rebuilt on the new tiles, nothing to save.
    let wall = (0..w.map.h() as u16)
        .flat_map(|y| (0..w.map.w() as u16).map(move |x| TilePos::new(x, y)))
        .find(|&t| t != roads[1] && w.map.tile_at(t) == TileKind::Wall)
        .expect("a wall");
    w.map.set_tile(wall, TileKind::Ground);
    w.invalidate_flow_fields();
    let c = w.flow_fields.landmarks(&w.map);
    assert!(!std::sync::Arc::ptr_eq(&a, &c) && c.same_tiles(&w.map), "a wall coming down rebuilds them");
    assert!(w.landmark_base.is_none());
}

/// Review finding 10: the landmarks kept across a closure (a building going
/// up on a Lot) are the ones a loaded world searches with, so a save after
/// the closure resumes byte for byte (the paths' tie-breaks follow the
/// heuristic). Big paths forced on the 2k map; Full walkers so the paths run.
#[test]
fn test_save_after_a_closure_resumes_byte_for_byte() {
    let mut cfg = Config::load();
    cfg.exec.flow_field_max_tiles = 0;
    cfg.lod.force = Some(citysim::Lod::Full);
    let mut w = World::new(42, cfg);
    w.run_ticks(120);
    let before = w.flow_fields.landmarks(&w.map);
    let lot = citysim::systems::founding::vacant_lots(&w).into_iter().next().expect("a vacant Lot");
    citysim::systems::founding::build_on_lot(&mut w, lot, citysim::BuildingKind::Bar, None).expect("built");
    assert!(std::sync::Arc::ptr_eq(&before, &w.flow_fields.landmarks(&w.map)), "the closure kept the landmarks");
    assert!(w.landmark_base.is_some(), "and the world notes their tiles");
    w.run_ticks(30);
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(citysim::save::to_ron(&back), text);
    w.run_ticks(240);
    back.run_ticks(240);
    assert!(citysim::save::to_ron(&back) == citysim::save::to_ron(&w), "the load walks on byte for byte");
    let (a, b) = (w.flow_fields.landmarks(&w.map), back.flow_fields.landmarks(&back.map));
    assert!(a.base_tiles() == b.base_tiles() && !a.same_tiles(&w.map), "the same stale landmarks");
}

/// The El's per-tile shortlist (`Net::near`) is the brute-force scan's: the
/// `NEAR` nearest stops by `(Manhattan, index)`, on every tile of the
/// shipped map, from a per-cell candidate index.
#[test]
fn test_near_index_matches_the_scan() {
    let w = World::new(42, Config::load());
    let net = &w.transit.net;
    assert!(!net.stops.is_empty());
    let k = citysim::systems::transit::NEAR.min(net.stops.len());
    for y in 0..w.map.h() as u16 {
        for x in 0..w.map.w() as u16 {
            let t = TilePos::new(x, y);
            let mut v: Vec<(u32, u16)> =
                net.stops.iter().enumerate().map(|(i, s)| (s.manhattan(t), i as u16)).collect();
            v.sort_unstable();
            let want: Vec<u16> = v[..k].iter().map(|&(_, i)| i).collect();
            assert_eq!(&*net.near(t), &want[..], "{t}");
        }
    }
    assert!(net.near(TilePos::new(w.map.w() as u16, 0)).is_empty(), "off the map");
    assert!(net.near_bytes() < 64 * 1024, "{} bytes", net.near_bytes());
}

/// A street grid: Road every 8th row and column, 7 × 7 walled blocks with a
/// yard between, a 6-row river across the middle with a bridge every 200
/// columns.
fn grid_map(w: usize, h: usize) -> Map {
    let mut text = format!("{w} {h}\n");
    let river = (h / 2 / 8) * 8 + 1..(h / 2 / 8) * 8 + 7;
    for y in 0..h {
        for x in 0..w {
            let ch = if river.contains(&y) {
                if x % 200 == 7 {
                    '='
                } else {
                    '~'
                }
            } else if x % 8 == 7 || y % 8 == 7 {
                '='
            } else if (x % 8 == 3 && y % 8 == 6) || ((1..6).contains(&(x % 8)) && (1..6).contains(&(y % 8))) {
                '.'
            } else {
                '#'
            };
            text.push(ch);
        }
        text.push('\n');
    }
    text.push('\n');
    Map::parse(&text)
}

fn road_tile(map: &Map, rng: &mut rand_chacha::ChaCha8Rng) -> TilePos {
    loop {
        let p = TilePos::new(rng.random_range(0..map.w() as u16), rng.random_range(0..map.h() as u16));
        if map.tile_at(p) == TileKind::Road {
            return p;
        }
    }
}

/// Measurement (S3a's threshold): a flow field's build against an A*
/// query, by map size, on the street grid. Prints; asserts nothing.
#[test]
#[ignore = "tier: diag"]
fn measure_flow_field_vs_big_path() {
    for (w, h) in [(256, 192), (500, 400), (700, 500), (1000, 1000), (1500, 1000), (3000, 1875)] {
        let map = grid_map(w, h);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(9);
        let builds = if w * h > 1_000_000 { 3 } else { 10 };
        let t = std::time::Instant::now();
        for _ in 0..builds {
            let door = road_tile(&map, &mut rng);
            std::hint::black_box(FlowField::build(&map, door));
        }
        let field_ms = t.elapsed().as_secs_f64() * 1e3 / builds as f64;
        let t = std::time::Instant::now();
        let labels = bigpath::labels(&map);
        let label_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let marks = bigpath::Landmarks::build(&map);
        let mark_ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!(
            "{w}x{h}: field build {field_ms:.1} ms ({} KB); labels {label_ms:.0} ms, landmarks {mark_ms:.0} ms",
            w * h / 1024
        );
        // Ends in the blocks' yards (Ground behind a gap: a door's tax).
        let yard = |rng: &mut rand_chacha::ChaCha8Rng| loop {
            let p = walkable_tile(&map, rng);
            if map.tile_at(p) == TileKind::Ground {
                break p;
            }
        };
        for (name, max) in [("local <=150", 150u32), ("any", u32::MAX)] {
            let mut pairs = Vec::new();
            while pairs.len() < 300 {
                let (a, b) = (yard(&mut rng), yard(&mut rng));
                if a.manhattan(b) <= max {
                    pairs.push((a, b));
                }
            }
            for (alt, m) in [("Manhattan", bigpath::Landmarks::none()), ("ALT", marks.clone())] {
                let mut us: Vec<f64> = Vec::new();
                let before = bigpath::expanded();
                for &(a, b) in &pairs {
                    let t = std::time::Instant::now();
                    std::hint::black_box(bigpath::find(&map, &labels, &m, a, b));
                    us.push(t.elapsed().as_secs_f64() * 1e6);
                }
                let per = (bigpath::expanded() - before) / pairs.len() as u64;
                us.sort_by(f64::total_cmp);
                eprintln!(
                    "  {name} {alt}: p50 {:.0} us p99 {:.0} us max {:.0} us, {per} expanded a query; scratch {} MB",
                    us[us.len() / 2],
                    us[us.len() * 99 / 100],
                    us[us.len() - 1],
                    bigpath::scratch_bytes() >> 20
                );
            }
        }
    }
}
