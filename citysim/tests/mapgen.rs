//! Scale S3b (docs/SCALE_20K.md): the map spec's extensions, the generator
//! and the committed maps (the generated 20k stand-in, the TheCityGraphics
//! import fixture).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use citysim::map::{MapSource, MapText};
use citysim::mapgen;
use citysim::{BuildingKind, Config, Map, TileKind, TilePos, World, TICKS_PER_DAY};

/// Same tiles, zones and buildings (kind, rect, door, tier, floors).
fn same_city(a: &Map, b: &Map) -> Result<(), String> {
    if (a.w(), a.h()) != (b.w(), b.h()) {
        return Err(format!("size {} x {} vs {} x {}", a.w(), a.h(), b.w(), b.h()));
    }
    if a.tiles() != b.tiles() {
        return Err("tiles differ".into());
    }
    for y in 0..a.h() {
        for x in 0..a.w() {
            let p = TilePos::new(x as u16, y as u16);
            if a.zone(p) != b.zone(p) {
                return Err(format!("zone at {p}"));
            }
        }
    }
    let key = |m: &Map| -> Vec<String> {
        m.buildings.iter().map(|b| format!("{:?} {:?} {} {} {}", b.kind, b.rect, b.door, b.tier, b.floors)).collect()
    };
    if key(a) != key(b) {
        return Err("buildings differ".into());
    }
    Ok(())
}

/// Every door reachable from the first over walkable tiles.
fn unreachable_doors(m: &Map) -> usize {
    let start = m.buildings[0].door;
    let mut seen = vec![false; m.w() * m.h()];
    seen[m.index(start)] = true;
    let mut q = VecDeque::from([start]);
    while let Some(p) = q.pop_front() {
        for n in m.neighbours4(p) {
            if m.walkable(n) && !seen[m.index(n)] {
                seen[m.index(n)] = true;
                q.push_back(n);
            }
        }
    }
    m.buildings.iter().filter(|b| !seen[m.index(b.door)]).count()
}

#[test]
fn test_generator_at_2000_is_the_shipped_map() {
    let cfg = Config::load();
    let shipped = Map::load(&cfg.asset("map.txt"));
    let gen = mapgen::generate(2_000, 0);
    let m = Map::parse(&gen.text);
    same_city(&m, &shipped).expect("the 2k generator reproduces assets/map.txt");
    assert_eq!(gen.boroughs, 1);
    // The shipped `[districts]` rows, as a grid: the same tile to district.
    let names: Vec<String> = m.districts.iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, cfg.districts.names);
    for y in 0..m.h() {
        for x in 0..m.w() {
            let p = TilePos::new(x as u16, y as u16);
            let d = usize::from(m.district_at(p).expect("grid"));
            let row = (0..cfg.districts.names.len())
                .find(|&i| {
                    cfg.districts.zone_of(i) == Some(m.zone(p))
                        && cfg.districts.x_from[i] <= p.x
                        && p.x < cfg.districts.x_to[i]
                })
                .expect("a row");
            assert_eq!(d, row, "tile {p}");
        }
    }
}

#[test]
fn test_generator_20k_mix_reach_and_determinism() {
    let gen = mapgen::generate(20_000, 0);
    let m = Map::parse(&gen.text);
    assert_eq!((gen.boroughs, m.w(), m.h()), (10, 1280, 376));
    let want: BTreeMap<BuildingKind, usize> = [
        (BuildingKind::Home, 4_000),
        (BuildingKind::Farm, 120),
        (BuildingKind::Market, 30),
        (BuildingKind::Bar, 30),
        (BuildingKind::Hideout, 20),
        (BuildingKind::SecurityOffice, 20),
        (BuildingKind::Lot, 1_600),
        (BuildingKind::Jail, 1),
        (BuildingKind::Hall, 1),
        (BuildingKind::Warehouse, 1),
        (BuildingKind::Cemetery, 1),
    ]
    .into_iter()
    .collect();
    assert_eq!(gen.counts, want);
    for (k, n) in &want {
        assert_eq!(m.count_kind(*k), *n, "{k:?}");
    }
    assert_eq!(unreachable_doors(&m), 0);
    assert!(m.districts.len() <= citysim::MAX_DISTRICTS && m.districts.len() == gen.districts);
    // Every district has tiles; every El waypoint is a Road tile.
    let mut seen = BTreeSet::new();
    for y in 0..m.h() {
        for x in 0..m.w() {
            seen.insert(m.district_at(TilePos::new(x as u16, y as u16)).expect("a grid"));
        }
    }
    assert_eq!(seen.len(), m.districts.len());
    assert_eq!(m.lines.len(), 14);
    for l in &m.lines {
        for &[x, y] in &l.waypoints {
            assert_eq!(m.tile_at(TilePos::new(x, y)), TileKind::Road, "{} waypoint {x},{y}", l.name);
        }
    }
    // Deterministic, and the seed moves the Blocks (not the mix).
    assert_eq!(mapgen::generate(20_000, 0).text, gen.text);
    let other = mapgen::generate(20_000, 7);
    assert_ne!(other.text, gen.text);
    assert_eq!(other.counts, gen.counts);
    assert_eq!(unreachable_doors(&Map::parse(&other.text)), 0);
}

/// The committed stand-in is the generator's output (regenerate with
/// `citysim-cli gen-map --residents 20000 --seed 0 --out assets/maps/gen20k.txt`).
#[test]
fn test_committed_gen20k_is_current() {
    let cfg = Config::load();
    let text = std::fs::read_to_string(cfg.asset("maps/gen20k.txt")).expect("assets/maps/gen20k.txt");
    assert_eq!(text, mapgen::generate(20_000, 0).text, "regenerate assets/maps/gen20k.txt");
}

#[test]
fn test_map_source_from_config() {
    let mut cfg = Config::load();
    assert_eq!(MapSource::from_config(&cfg), MapSource::File(cfg.asset("map.txt")));
    cfg.world.map = "gen:6000:3".into();
    assert_eq!(MapSource::from_config(&cfg), MapSource::Generated { residents: 6_000, seed: 3 });
    cfg.world.map = "gen:6000".into();
    assert_eq!(MapSource::from_config(&cfg), MapSource::Generated { residents: 6_000, seed: 0 });
}

/// A generated map's districts and El lines replace the config's; the city
/// is dealt into its Blocks and runs a day.
#[test]
fn test_world_on_a_generated_map() {
    let mut cfg = Config::load().scaled_to(4_000);
    cfg.world.map = "gen:4000".into();
    let mut w = World::new(42, cfg);
    assert_eq!((w.map.w(), w.map.h()), (512, 192));
    let names: Vec<&str> = w.districts.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names.len(), w.map.districts.len());
    // The central borough (the east one) keeps the shipped names.
    assert!(names.contains(&"Sump 1") && names.contains(&"Sump Central") && names.contains(&"Civic"), "{names:?}");
    assert_eq!(w.config.transit.lines, w.map.lines);
    assert_eq!(w.config.districts.names.len(), w.map.districts.len());
    let housed =
        w.citizens().into_iter().filter(|&a| w.comp::<citysim::Household>(a).and_then(|h| h.home).is_some()).count();
    assert!(housed >= 3_900, "housed {housed}");
    // A Home in the west borough is in one of its districts.
    let far = w.map.buildings.iter().find(|b| b.kind == BuildingKind::Home && b.door.x < 256).expect("a home").clone();
    let d = w.district_of(far.door);
    assert!(w.district_name(d).ends_with(" 1"), "{}", w.district_name(d));
    w.run_ticks(TICKS_PER_DAY);
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(citysim::save::to_ron(&back), text);
    assert_eq!(back.district_of(far.door), d);
}

/// A map from parts: the text format's S3b lines (rle grids, `D` rows, a
/// districts grid, `L` lines, comments) parse back to the same map.
#[test]
fn test_map_text_round_trips() {
    let (w, h) = (12usize, 9usize);
    let mut tiles = vec!['.'; w * h];
    for x in 0..w {
        tiles[4 * w + x] = '=';
    }
    // A Home at (1,0) 5 x 4, door bottom middle onto the road row.
    for y in 0..4 {
        for x in 1..6 {
            tiles[y * w + x] = if y == 0 || y == 3 || x == 1 || x == 5 { '#' } else { '.' };
        }
    }
    tiles[3 * w + 3] = 'D';
    // S3c: a Mountain tile.
    tiles[8 * w + 11] = 'm';
    let zones: Vec<char> = (0..w * h).map(|i| if i % w < 6 { 'U' } else { 'M' }).collect();
    let grid: Vec<u8> = (0..w * h).map(|i| u8::from(i / w >= 5)).collect();
    let parts = MapText {
        w,
        h,
        tiles: tiles.clone(),
        zones: zones.clone(),
        buildings: vec![(BuildingKind::Home, citysim::Rect { x: 1, y: 0, w: 5, h: 4 }, 0, 3)],
        districts: vec![
            citysim::map::MapDistrict { name: "North Side".into(), zone: citysim::Zone::Sump, x_from: 0, x_to: 12 },
            citysim::map::MapDistrict { name: "South".into(), zone: citysim::Zone::Mid, x_from: 0, x_to: 12 },
        ],
        district_grid: grid,
        lines: vec![citysim::config::TransitLineCfg { name: "Test Line".into(), waypoints: vec![[0, 4], [11, 4]] }],
        comments: vec!["a test".into()],
        extra: vec![citysim::map::BuildingExtra { flats: Some(12), storeys: Some(9) }],
    };
    for rle in [false, true] {
        let m = Map::parse(&parts.to_text(rle));
        assert_eq!(m.buildings.len(), 1);
        assert_eq!((m.buildings[0].floors, m.buildings[0].tier, m.buildings[0].door), (3, 0, TilePos::new(3, 3)));
        assert_eq!((m.buildings[0].flats, m.buildings[0].storeys), (Some(12), Some(9)));
        assert_eq!(m.tile(11, 8), TileKind::Mountain);
        assert!(m.walkable(TilePos::new(11, 8)));
        assert_eq!(m.districts, parts.districts);
        assert_eq!(m.lines, parts.lines);
        assert_eq!(m.district_at(TilePos::new(0, 0)), Some(0));
        assert_eq!(m.district_at(TilePos::new(0, 8)), Some(1));
        assert_eq!(m.zone(TilePos::new(8, 0)), citysim::Zone::Mid);
        let mut cfg = Config::load();
        m.apply_to(&mut cfg);
        assert_eq!(cfg.districts.names, vec!["North Side".to_string(), "South".to_string()]);
        assert_eq!(cfg.transit.lines, parts.lines);
    }
}

/// The shipped map has none of S3b's lines: the config's districts and El
/// lines stand.
#[test]
fn test_shipped_map_keeps_the_config() {
    let cfg = Config::load();
    let m = MapSource::from_config(&cfg).load();
    assert!(m.districts.is_empty() && m.lines.is_empty() && m.district_at(TilePos::new(0, 0)).is_none());
    let mut c2 = cfg.clone();
    m.apply_to(&mut c2);
    assert_eq!(c2.districts, cfg.districts);
    assert_eq!(c2.transit.lines, cfg.transit.lines);
}

/// A save written before SAVE_VERSION 13 holds one enum name per tile; it
/// still loads, and a new save packs the grids.
#[test]
fn test_old_map_save_shape_loads() {
    let old = "(w:4,h:2,tiles:[Ground,Road,Road,Water,Wall,Door,Farmland,Ground],zones:[Sump,Sump,Mid,Mid,Spire,Civic,Vats,Mid],buildings:[],blocked:[])";
    let m: Map = ron::from_str(old).expect("an old map save");
    assert_eq!(m.tile(1, 0), TileKind::Road);
    assert_eq!(m.tile(2, 1), TileKind::Farmland);
    assert_eq!(m.zone(TilePos::new(1, 1)), citysim::Zone::Civic);
    let new = ron::to_string(&m).expect("save");
    assert!(new.contains("tiles_rle:\".2=~#Df.\"") && !new.contains("Ground"), "{new}");
    let back: Map = ron::from_str(&new).expect("load");
    assert_eq!(back.tiles(), m.tiles());
    assert_eq!(back.zone(TilePos::new(2, 0)), citysim::Zone::Mid);
}

/// The committed TheCityGraphics import (`assets/maps/isle_m_k4.txt`: the
/// whole island at 8 m tiles, `citysim-cli import-graphics --downsample 4`;
/// docs/IMPORT_GRAPHICS.md). Tests never read the graphics repo: this
/// snapshot is the fixture.
#[test]
fn test_graphics_import_fixture_parses() {
    let cfg = Config::load();
    let m = Map::load(&cfg.asset("maps/isle_m_k4.txt"));
    assert_eq!((m.w(), m.h()), (750, 469));
    let names: Vec<&str> = m.districts.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "D1 The Sump",
            "D2 Civic Core",
            "D3 Neon Quarter",
            "D4 Mid-Levels",
            "D5 The Spire",
            "D6 The Vats",
            "D7 The Stack"
        ]
    );
    let lines: Vec<&str> = m.lines.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(lines, ["Terrace Line", "Tram Tai Ning Road"], "the El rail and the tram");
    // S3c: the sim export's hills are walkable Mountain, its flats carried.
    assert!(m.tiles().iter().filter(|&&t| t == TileKind::Mountain).count() > 100_000, "the hills");
    assert!(m.buildings.iter().filter(|b| b.kind == BuildingKind::Home).all(|b| b.flats.is_some()));
    assert!(!m.edge_roads().is_empty(), "the carved piers");
    for k in [BuildingKind::Jail, BuildingKind::Hall, BuildingKind::Warehouse, BuildingKind::Cemetery] {
        assert_eq!(m.count_kind(k), 1, "{k:?}");
    }
    assert!(m.count_kind(BuildingKind::Farm) > 0 && m.count_kind(BuildingKind::Market) > 0);
    assert!(m.count_kind(BuildingKind::Home) > 500);
    assert!(m.buildings.iter().any(|b| b.kind == BuildingKind::Home && b.floors > 1), "multi-storey Blocks");
    // Every tile has a district.
    for y in (0..m.h()).step_by(7) {
        for x in (0..m.w()).step_by(7) {
            assert!(m
                .district_at(TilePos::new(x as u16, y as u16))
                .is_some_and(|d| usize::from(d) < m.districts.len()));
        }
    }
    assert_eq!(unreachable_doors(&m), 0, "every door on one street network");
}
