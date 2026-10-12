//! Scale S2 (docs/SCALE_20K.md): the widened types. A map larger than 256
//! in both dimensions loads and runs a day; a save with coordinates past
//! 255 round-trips; the CSV grows a slot group per district or corp past the
//! 2k header's.

use citysim::{save, stats, Building, BuildingKind, Config, Position, TilePos, World, TICKS_PER_DAY};

const W: usize = 600;
const H: usize = 400;
/// The shipped 256 x 192 city sits at this offset, so every building of it
/// lies past column 255 and row 255.
const OX: usize = 300;
const OY: usize = 200;

/// The shipped city embedded at (`OX`, `OY`) in a 600 x 400 map of open
/// Sump ground. The city's west-edge and north-edge roads run on to the big
/// map's edges (its edge roads), and the west-edge rows' roads also run east
/// from the city to the east edge.
fn big_map_text(cfg: &Config) -> String {
    let text = std::fs::read_to_string(cfg.asset(&cfg.world.map)).expect("the shipped map");
    let lines: Vec<&str> = text.lines().collect();
    let (w0, h0): (usize, usize) = {
        let p: Vec<usize> = lines[0].split_whitespace().map(|s| s.parse().expect("header")).collect();
        (p[0], p[1])
    };
    let tiles: Vec<Vec<char>> = lines[1..=h0].iter().map(|l| l.chars().collect()).collect();
    let zones: Vec<Vec<char>> = lines[h0 + 2..h0 + 2 + h0].iter().map(|l| l.chars().collect()).collect();
    let mut grid = vec![vec!['.'; W]; H];
    let mut zone = vec![vec!['U'; W]; H];
    for y in 0..h0 {
        for x in 0..w0 {
            grid[OY + y][OX + x] = tiles[y][x];
            zone[OY + y][OX + x] = zones[y][x];
        }
    }
    for y in 0..h0 {
        if tiles[y][0] == '=' {
            for x in (0..OX).chain(OX + w0..W) {
                grid[OY + y][x] = '=';
            }
        }
    }
    for x in 0..w0 {
        if tiles[0][x] == '=' {
            for row in grid.iter_mut().take(OY) {
                row[OX + x] = '=';
            }
        }
    }
    let mut out = format!("{W} {H}\n");
    for row in &grid {
        out.extend(row.iter());
        out.push('\n');
    }
    out.push('\n');
    for row in &zone {
        out.extend(row.iter());
        out.push('\n');
    }
    out.push('\n');
    for line in &lines[h0 + 2 + h0..] {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.first() != Some(&"B") {
            continue;
        }
        let x: usize = p[2].parse().expect("x");
        let y: usize = p[3].parse().expect("y");
        let rest = p[4..].join(" ");
        out.push_str(&format!("B {} {} {} {rest}\n", p[1], x + OX, y + OY));
    }
    out
}

/// The shipped config on the big map: the El's waypoints moved with the
/// city; fourteen districts cut from the empty ground west of it ahead of
/// the city's eight (so the city's districts are ids 14-21, past a 4-bit
/// district and a 16-bit mask), the city's eight moved with it.
fn big_config(tag: &str) -> Config {
    let mut cfg = Config::load();
    let path = std::env::temp_dir().join(format!("citysim-big-map-{tag}-{}.txt", std::process::id()));
    std::fs::write(&path, big_map_text(&cfg)).expect("write the big map");
    cfg.world.map = path.to_string_lossy().into_owned();
    for line in &mut cfg.transit.lines {
        for p in &mut line.waypoints {
            p[0] += OX as u16;
            p[1] += OY as u16;
        }
    }
    let d = &mut cfg.districts;
    let (names, zones, from, to) = (d.names.clone(), d.zones.clone(), d.x_from.clone(), d.x_to.clone());
    d.names.clear();
    d.zones.clear();
    d.x_from.clear();
    d.x_to.clear();
    let strips = 14u16;
    let step = OX as u16 / strips;
    for i in 0..strips {
        d.names.push(format!("Waste {i}"));
        d.zones.push("U".into());
        d.x_from.push(i * step);
        d.x_to.push(if i + 1 == strips { OX as u16 } else { (i + 1) * step });
    }
    for i in 0..names.len() {
        d.names.push(names[i].clone());
        d.zones.push(zones[i].clone());
        d.x_from.push(if from[i] == 0 {
            if zones[i] == "U" {
                OX as u16
            } else {
                0
            }
        } else {
            from[i] + OX as u16
        });
        d.x_to.push(if to[i] == 256 { W as u16 } else { to[i] + OX as u16 });
    }
    cfg
}

#[test]
fn test_big_map_loads_and_runs_a_day() {
    let cfg = big_config("day");
    let mut w = World::new(42, cfg);
    assert_eq!((w.map.w(), w.map.h()), (W, H));
    assert_eq!(w.districts.len(), 22);
    let doors: Vec<TilePos> = w.map.buildings.iter().map(|b| b.door).collect();
    assert!(!doors.is_empty() && doors.iter().all(|d| usize::from(d.x) >= OX && usize::from(d.y) >= OY));
    let start: Vec<(citysim::EntityId, TilePos)> =
        w.citizens().into_iter().filter_map(|a| w.comp::<Position>(a).map(|p| (a, p.tile))).collect();
    assert!(start.len() > 1_000, "the city spawned: {}", start.len());
    w.run_ticks(TICKS_PER_DAY);
    let moved = start.iter().filter(|(a, t)| w.comp::<Position>(*a).is_some_and(|p| p.tile != *t)).count();
    assert!(moved > 100, "people walked: {moved}");
    // A district past 15 is a real district: its Homes are found by door.
    let sump = w.districts.iter().find(|d| d.name == "Sump East").expect("Sump East");
    assert!(sump.id.index() >= 16 && !sump.homes.is_empty(), "{} with {} homes", sump.id.index(), sump.homes.len());
    let home = sump.homes[0];
    let door = w.comp::<Building>(home).expect("home").door;
    assert_eq!(w.district_of(door), sump.id);
    // The CSV: one slot group per district.
    let header = citysim::systems::stats::csv_header(&w);
    let row = w.stats.history.back().expect("a day").csv_row();
    assert_eq!(row.split(',').count(), header.split(',').count());
    assert!(header.contains("d22_coverage") && header.contains("d22_street_density") && header.contains("d22_dregs"));
    assert!(!header.contains("d23_"));
    // A Jail can hold more than 255.
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    w.comp_mut::<Building>(jail).expect("jail").capacity = 400;
    w.run_ticks(60);
    assert_eq!(w.comp::<Building>(jail).expect("jail").capacity, 400);
}

#[test]
fn test_save_round_trips_large_coordinates() {
    let cfg = big_config("save");
    let mut w = World::new(43, cfg);
    // Past a midnight: the day traces hold districts past 15 (the packed
    // trace's fifth district bit).
    w.run_ticks(TICKS_PER_DAY + 60);
    let traced = |w: &World| -> Vec<(citysim::EntityId, usize)> {
        w.citizens()
            .into_iter()
            .filter_map(|a| w.comp::<citysim::Trace>(a).and_then(|t| t.on_day(0)).map(|t| (a, t.district.index())))
            .collect()
    };
    let before = traced(&w);
    assert!(before.iter().any(|&(_, d)| (16..22).contains(&d)), "a day ended in a district past 15");
    let a = w.citizens()[0];
    let far = TilePos::new(599, 399);
    assert_eq!(w.map.tile_at(far), citysim::TileKind::Ground);
    w.comp_mut::<Position>(a).expect("pos").tile = far;
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    w.comp_mut::<Building>(jail).expect("jail").capacity = 1_000;
    w.levers.guard_count = 300;
    let text = save::to_ron(&w);
    let mut back = save::from_ron(&text).expect("load");
    assert_eq!(back.comp::<Position>(a).expect("pos").tile, far);
    assert_eq!(back.comp::<Building>(jail).expect("jail").capacity, 1_000);
    assert_eq!(back.levers.guard_count, 300);
    assert_eq!(back.districts.len(), 22);
    assert_eq!(traced(&back), before);
    assert_eq!(save::to_ron(&back), text);
    w.run_ticks(120);
    back.run_ticks(120);
    assert_eq!(blake3::hash(save::to_ron(&w).as_bytes()), blake3::hash(save::to_ron(&back).as_bytes()));
}

/// The 2k header is the constant; more corps or districts grow each slot
/// group in place, after its last slot.
#[test]
fn test_csv_header_grows_its_slot_groups() {
    assert_eq!(stats::csv_header(9, 8), stats::CSV_HEADER);
    assert_eq!(stats::csv_header(3, 5), stats::CSV_HEADER);
    let h = stats::csv_header(11, 10);
    let extra = 2 * (2 + 4 + 3) + 2 * (6 + 1 + 3);
    assert_eq!(h.split(',').count(), stats::CSV_HEADER.split(',').count() + extra);
    for run in [
        "corp9_order,corp10_treasury,corp10_order,corp11_treasury,corp11_order,acquisitions",
        "corp9_data,corp10_tier_chrome,corp10_tier_deck,corp10_tier_industry,corp10_data,corp11_tier_chrome",
        "c9_competence,c10_honour,c10_standing,c10_competence,c11_honour,c11_standing,c11_competence,law_competence",
        "d8_guards,d9_coverage,d9_control,d9_litter,d9_unrest,d9_crime,d9_guards,d10_coverage",
        "d8_street_density,d9_street_density,d10_street_density,fv_killed",
        "d8_dregs,d9_meals,d9_starving,d9_dregs,d10_meals,d10_starving,d10_dregs,civic_payroll",
    ] {
        assert!(h.contains(run), "{run}");
    }
}
