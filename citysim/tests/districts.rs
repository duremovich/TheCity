//! M12 phase 1: districts, the lookup, aggregates and control
//! (docs/M12_DISTRICTS.md § 1; plan phase 1 tests, D1-D8).

use citysim::config::{ControlWeights, DistrictsCfg};
use citysim::systems::{classes, districts, gang};
use citysim::{
    Brain, Building, BuildingKind, Claim, Config, Controller, DayTrace, DistrictId, EntityId, EventKind, Household,
    Mood, Position, TilePos, Trace, World, Zone,
};

fn v2_world() -> World {
    World::new(42, Config::load().scaled_to(400))
}

/// The v1 city cut into three Mid districts by x (thirds of the 96-wide map).
fn v1_three() -> World {
    let mut c = Config::load().v1_profile();
    c.districts = DistrictsCfg {
        names: vec!["West".into(), "Centre".into(), "East".into()],
        zones: vec!["M".into(), "M".into(), "M".into()],
        x_from: vec![0, 32, 64],
        x_to: vec![32, 64, 256],
        control_min_share: 0.4,
        control_weights: ControlWeights { held_home: 1.0, owned_home: 1.0, owned_other: 3.0, city_per_coverage: 1.0 },
    };
    World::new(7, c)
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

#[test]
fn test_district_of_matches_cuts_on_v2_map() {
    let w = v2_world();
    assert_eq!(w.districts.len(), 8);
    let spots = [
        ((63, 100), 3),
        ((64, 100), 4),
        ((71, 150), 5),
        ((72, 150), 6),
        ((136, 150), 7),
        ((100, 50), 1),
        ((100, 20), 0),
        ((220, 100), 2),
        ((255, 191), 2),
    ];
    for ((x, y), d) in spots {
        assert_eq!(w.district_of(TilePos { x, y }), DistrictId(d), "tile ({x}, {y})");
    }
    // Every tile is covered by exactly one row (first match is the only match),
    // and the grid agrees with the rows.
    let cfg = &w.config.districts;
    for y in 0..w.map.h() {
        for x in 0..w.map.w() {
            let p = TilePos { x: x as u8, y: y as u8 };
            let z = w.map.zone(p);
            let hits: Vec<usize> = (0..cfg.names.len())
                .filter(|&i| {
                    cfg.zone_of(i) == Some(z) && usize::from(cfg.x_from[i]) <= x && x < usize::from(cfg.x_to[i])
                })
                .collect();
            assert_eq!(hits.len(), 1, "tile {p:?} in zone {z} matches rows {hits:?}");
            assert_eq!(w.district_of(p), DistrictId(hits[0] as u8), "tile {p:?}");
            assert_eq!(w.district(w.district_of(p)).zone, z);
        }
    }
    // The Block column of the cuts table.
    let blocks: Vec<usize> = w.districts.iter().map(|d| d.homes.len()).collect();
    assert_eq!(blocks, vec![60, 0, 0, 71, 69, 70, 64, 66]);
    // Each Home sits in the district of its door, and the street bit is a walkable non-building tile.
    for d in &w.districts {
        for &h in &d.homes {
            assert_eq!(w.district_of_building(h), d.id);
        }
        assert!(d.walk_tiles > 0, "{} has street", d.name);
    }
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let door = w.comp::<Building>(jail).expect("b").door;
    assert!(!w.is_street(door), "a door is inside its building");
    assert_eq!(w.district_of_building(jail), DistrictId(1), "the Precinct is in the Civic");
    // Adjacency: Sump Central touches Sump West and Sump East, not the Spire.
    let adj = w.district_adjacent[6];
    assert!(adj & (1 << 5) != 0 && adj & (1 << 7) != 0 && adj & 1 == 0);
}

#[test]
fn test_v1_map_is_one_district() {
    let w = World::new(3, Config::load().v1_profile());
    assert_eq!(w.districts.len(), 1);
    assert_eq!(w.districts[0].name, "City");
    for y in 0..w.map.h() {
        for x in 0..w.map.w() {
            assert_eq!(w.district_of(TilePos { x: x as u8, y: y as u8 }), DistrictId(0));
        }
    }
    // The assets' eight rows on the zoneless map still give one district (D1).
    let mut c = Config::load().v1_profile();
    c.districts = Config::load().districts;
    let w = World::new(3, c);
    assert_eq!(w.districts.len(), 1);
}

/// A housed adult with a Brain whose Home door is in district `d` and who
/// matches `pred`.
fn pick(w: &World, d: u8, pred: impl Fn(&World, EntityId) -> bool) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(w, a)
                && w.comp::<Household>(a)
                    .and_then(|h| h.home)
                    .is_some_and(|h| w.district_of_building(h) == DistrictId(d))
                && pred(w, a)
        })
        .expect("an agent")
}

#[test]
fn test_aggregates_on_hand_built_world() {
    let mut w = v1_three();
    w.run_ticks(1);
    assert_eq!(w.districts.len(), 3);
    // A worker in the West at 0.6, a jobless resident of the Centre at 0.0,
    // and a homeless adult standing in the East at -0.6.
    let worker = pick(&w, 0, |w, a| w.has::<citysim::Job>(a));
    let idle = pick(&w, 1, |w, a| !w.has::<citysim::Job>(a));
    let drifter = pick(&w, 2, |_, _| true);
    w.set_home(drifter, None);
    let east_street = (0..w.map.h() as u8)
        .flat_map(|y| (64..96u8).map(move |x| TilePos { x, y }))
        .find(|&t| w.is_street(t))
        .expect("an East street tile");
    if let Some(p) = w.comp_mut::<Position>(drifter) {
        p.tile = east_street;
        p.building = None;
    }
    for (a, m) in [(worker, 0.6), (idle, 0.0), (drifter, -0.6)] {
        w.comp_mut::<Mood>(a).expect("mood").value = m;
    }
    for (i, d) in w.districts.iter_mut().enumerate() {
        d.crimes = (1..=7).map(|c| c * (i as u16 + 1)).collect();
    }
    districts::aggregates(&mut w);

    // Recompute D7 by hand.
    let execs = classes::exec_set(&w);
    let mut pop = [0u32; 3];
    let mut adults = [0u32; 3];
    let mut cls = [[0u32; 3]; 3];
    let mut happy = [0.0f32; 3];
    for a in w.citizens() {
        let at = match w.comp::<Household>(a).and_then(|h| h.home) {
            Some(h) => w.comp::<Building>(h).expect("home").door,
            None => w.comp::<Position>(a).expect("pos").tile,
        };
        let d = if at.x < 32 {
            0
        } else if at.x < 64 {
            1
        } else {
            2
        };
        pop[d] += 1;
        if w.has::<Brain>(a) && citysim::systems::demography::is_adult(&w, a) {
            adults[d] += 1;
            cls[d][classes::class_in(&w, a, &execs).index()] += 1;
            happy[d] += (w.comp::<Mood>(a).expect("mood").value + 1.0) / 2.0;
        }
    }
    for i in 0..3 {
        let d = &w.districts[i];
        assert_eq!(d.population, pop[i], "{} population", d.name);
        assert_eq!(d.adults, adults[i]);
        assert_eq!(d.classes, cls[i]);
        assert!((d.happiness - happy[i] / adults[i] as f32).abs() < 1e-5, "{} happiness", d.name);
        let crimes = 28.0 * (i as f32 + 1.0);
        assert!((d.crime_rate - crimes / 7.0 / pop[i] as f32 * 100.0).abs() < 1e-5, "{} crime rate", d.name);
        assert!(!d.trace.is_empty());
        assert_eq!(d.residents.len() as u32, adults[i]);
    }
    assert!(w.districts[0].residents.contains(&worker));
    assert!(w.districts[1].residents.contains(&idle));
    assert!(w.districts[2].residents.contains(&drifter), "the homeless bin by their tile");
    assert_eq!(classes::class_of(&w, drifter), citysim::Class::Dreg);
    // Control from the presence list (D8).
    districts::update_control(&mut w);
    for i in 0..3 {
        let pres = districts::presence(&w, DistrictId(i as u8));
        let (c, share) = districts::control_of(&pres, 0.4);
        assert_eq!(w.districts[i].control, c);
        assert!((w.districts[i].control_share - share).abs() < 1e-6);
    }
}

#[test]
fn test_gang_holding_most_homes_controls() {
    let mut w = v1_three();
    w.run_ticks(1);
    let g = w.gangs()[0];
    let d = DistrictId(1);
    let homes = w.district(d).homes.clone();
    assert!(homes.len() >= 4);
    for &h in &homes {
        w.comp_mut::<Building>(h).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD });
    }
    // An unfinished claim counts for nothing.
    w.comp_mut::<Building>(homes[0]).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD - 1 });
    w.district_mut(d).coverage = 0.5;
    let pres = districts::presence(&w, d);
    let gang_p = pres.iter().find(|(c, _)| *c == Controller::Gang(g)).map_or(0.0, |&(_, v)| v);
    let hideout_here = w.hideout_of(g).is_some_and(|h| w.district_of_building(h) == d);
    let want = (homes.len() - 1) as f32 + if hideout_here { 3.0 } else { 0.0 };
    assert!((gang_p - want).abs() < 1e-5, "gang presence {gang_p} vs {want}");
    // City: its standing buildings in d (Blocks 1, others 3, no Lots, no Hideouts) x coverage.
    let mut city = 0.0f32;
    for &b in &w.district(d).buildings {
        let bd = w.comp::<Building>(b).expect("b");
        if bd.owner.is_some() || bd.demolished || matches!(bd.kind, BuildingKind::Lot | BuildingKind::Hideout) {
            continue;
        }
        city += if bd.kind == BuildingKind::Home { 1.0 } else { 3.0 };
    }
    let city_p = pres.iter().find(|(c, _)| *c == Controller::City).map_or(0.0, |&(_, v)| v);
    assert!((city_p - city * 0.5).abs() < 1e-4, "city presence {city_p} vs {}", city * 0.5);
    assert!(gang_p > city_p, "the gang out-holds a thinly watched City");
    districts::update_control(&mut w);
    assert_eq!(w.district(d).control, Controller::Gang(g));
}

#[test]
fn test_city_under_high_coverage_beats_lone_corp() {
    let mut w = v1_three();
    w.run_ticks(1);
    let d = DistrictId(0);
    let corp = w.spawn();
    w.insert(corp, citysim::Corp::new("TestCo".into(), Default::default(), 1000, None));
    let homes = w.district(d).homes.clone();
    let half = homes.len() / 2;
    for &h in &homes[..half] {
        w.comp_mut::<Building>(h).expect("b").owner = Some(corp);
    }
    w.district_mut(d).coverage = 2.0;
    let pres = districts::presence(&w, d);
    assert_eq!(pres[0].0, Controller::City, "{pres:?}");
    let corp_p = pres.iter().find(|(c, _)| *c == Controller::Corp(corp)).map_or(0.0, |&(_, v)| v);
    assert!((corp_p - half as f32).abs() < 1e-5);
    let city_hi = pres[0].1;
    w.district_mut(d).coverage = 0.5;
    let city_lo = districts::presence(&w, d).iter().find(|(c, _)| *c == Controller::City).map_or(0.0, |&(_, v)| v);
    assert!((city_lo * 4.0 - city_hi).abs() < 1e-3, "city presence scales with coverage");
    w.district_mut(d).coverage = 2.0;
    districts::update_control(&mut w);
    assert_eq!(w.district(d).control, Controller::City);
}

#[test]
fn test_control_first_computation_is_silent() {
    let mut w = v1_three();
    w.run_ticks(1);
    assert_eq!(count(&w, EventKind::DistrictControl), 0, "day 0 sets control silently");
    assert!(w.districts.iter().all(|d| d.control_init));
    // Hand the Centre to a gang: one event, and a gang that loses it later is shocked.
    let g = w.gangs()[0];
    let d = DistrictId(1);
    for h in w.district(d).homes.clone() {
        w.comp_mut::<Building>(h).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD });
    }
    w.district_mut(d).coverage = 0.5;
    districts::update_control(&mut w);
    assert_eq!(w.district(d).control, Controller::Gang(g));
    assert_eq!(count(&w, EventKind::DistrictControl), 1);
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::DistrictControl).expect("event");
    assert!(e.text.starts_with("Centre: "), "{}", e.text);
    assert!(e.actors.contains(&g));
    // Unchanged: no event.
    districts::update_control(&mut w);
    assert_eq!(count(&w, EventKind::DistrictControl), 1);
    for h in w.district(d).homes.clone() {
        w.comp_mut::<Building>(h).expect("b").claim = None;
    }
    let before = w.comp::<citysim::Gang>(g).expect("g").shocks.len();
    districts::update_control(&mut w);
    assert_eq!(count(&w, EventKind::DistrictControl), 2);
    let shocks = &w.comp::<citysim::Gang>(g).expect("g").shocks;
    assert_eq!(shocks.len(), before + 1);
    assert_eq!(shocks.last(), Some(&citysim::Shock::LostDistrict));
}

#[test]
fn test_daytrace_legacy_decode() {
    // A pre-M12 value: zone Sump (4), flags ALIVE|ATE, hunger 2, mood 1; bit 19 clear.
    let legacy: u32 = 4 | (1 | 128) << 3 | 2 << 11 | 1 << 13;
    let t = DayTrace::from(legacy);
    assert_eq!(t.zone, Zone::Sump);
    assert!(t.district.is_unset());
    assert_eq!(t.flags, 1 | 128);
    assert_eq!((t.hunger, t.mood), (2, 1));
    assert_eq!(u32::from(t), legacy, "UNSET packs back to the legacy value");
    // A current value round-trips the district and the high flags.
    let now =
        DayTrace { district: DistrictId(7), flags: citysim::trace_flags::ALIVE | citysim::trace_flags::SQUAT, ..t };
    assert_eq!(DayTrace::from(u32::from(now)), now);
    // Load fixes UNSET to the zone's first district.
    let mut w = v2_world();
    let a = w.citizens().into_iter().find(|&a| w.has::<Brain>(a)).expect("agent");
    w.insert(a, Trace::default());
    w.comp_mut::<Trace>(a).expect("trace").push(0, t, 30);
    w.migrate_legacy();
    let fixed = w.comp::<Trace>(a).expect("trace").on_day(0).expect("day 0");
    assert_eq!(fixed.district, DistrictId(5), "Sump West is the Sump's first district");
    assert_eq!(districts::first_of_zone(&w, Zone::Mid), DistrictId(3));
    assert_eq!(districts::first_of_zone(&w, Zone::Vats), DistrictId(2));
}

#[test]
fn test_daily_trace_and_crime_counter() {
    let mut w = v2_world();
    w.run_ticks(citysim::TICKS_PER_DAY * 3 + 1);
    for d in &w.districts {
        assert!(!d.trace.is_empty(), "{} has a trace", d.name);
        assert_eq!(d.crimes.len(), 3, "{}: three finished days rolled", d.name);
    }
    // Civic and Vats have no Blocks; every Sump and Mid district is inhabited.
    assert_eq!(w.districts[1].homes.len() + w.districts[2].homes.len(), 0);
    for i in [0, 3, 4, 5, 6, 7] {
        assert!(w.districts[i].population > 0, "{} inhabited", w.districts[i].name);
    }
    let total: u32 = w.districts.iter().map(|d| d.population).sum();
    assert_eq!(total as usize, w.citizens().len());
    // Same CSV slots as the districts.
    let row = w.stats.history.back().expect("a day");
    assert_eq!(row.districts.len(), 8);
    assert_eq!(row.districts[3].1, w.districts[3].control.csv_code());
}
