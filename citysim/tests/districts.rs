//! M12 districts: phase 1 (the lookup, aggregates and control; § 1, D1-D8)
//! and phase 2 (the law in districts; § 2, D9-D15, D38).

use citysim::config::{ControlWeights, DistrictsCfg};
use citysim::save;
use citysim::systems::{bind, classes, corp_brain, districts, faction, gang, law, law_brain, lod, street};
use citysim::util::largest_remainder;
use citysim::{
    Brain, Building, BuildingKind, Claim, Config, Controller, Crime, DayTrace, DistrictId, EntityId, EventKind, Gang,
    Hole, HoleKind, Household, Lod, Mood, Order, Personality, Position, Posture, Sentence, Stance, TilePos, Trace,
    Wallet, World, Zone, TICKS_PER_DAY,
};

fn v2_world() -> World {
    World::new(42, Config::load().scaled_to(400))
}

/// The v1 city cut into three Mid districts by x (thirds of the 96-wide map).
fn v1_three() -> World {
    World::new(7, three_cfg())
}

fn three_cfg() -> Config {
    let mut c = Config::load().v1_profile();
    c.districts = DistrictsCfg {
        names: vec!["West".into(), "Centre".into(), "East".into()],
        zones: vec!["M".into(), "M".into(), "M".into()],
        x_from: vec![0, 32, 64],
        x_to: vec![32, 64, 256],
        control_min_share: 0.4,
        control_weights: ControlWeights { held_home: 1.0, owned_home: 1.0, owned_other: 3.0, city_per_coverage: 1.0 },
    };
    c
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
    // The Block column of the cuts table (M12 D25: a derelict Block is no Home).
    let blocks: Vec<usize> = w
        .districts
        .iter()
        .map(|d| {
            let derelict = d
                .buildings
                .iter()
                .filter(|&&b| w.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Home && bd.derelict))
                .count();
            d.homes.len() + derelict
        })
        .collect();
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
        // Exact on purpose (fix pass, phase 1 review): the hand count bins the
        // same world by the same rule and draws nothing, so any difference is
        // a binning bug, not noise.
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
    // Review fix: a share wobbling into Contested and back is not a loss.
    for h in w.district(d).homes.clone() {
        w.comp_mut::<Building>(h).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD });
    }
    districts::update_control(&mut w);
    assert_eq!(w.district(d).control, Controller::Gang(g));
    let before = w.comp::<citysim::Gang>(g).expect("g").shocks.len();
    for h in w.district(d).homes.clone() {
        w.comp_mut::<Building>(h).expect("b").claim = None;
    }
    w.district_mut(d).coverage = 0.0;
    districts::update_control(&mut w);
    assert_eq!(w.district(d).control, Controller::Contested, "nobody present");
    for h in w.district(d).homes.clone() {
        w.comp_mut::<Building>(h).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD });
    }
    districts::update_control(&mut w);
    assert_eq!(w.district(d).control, Controller::Gang(g));
    assert_eq!(w.comp::<citysim::Gang>(g).expect("g").shocks.len(), before, "no shock for a Contested spell");
    assert_eq!(count(&w, EventKind::DistrictControl), 5, "the flips are still logged");
}

#[test]
fn test_district_lookup_before_rebuild_and_short_crime_history() {
    // A hand-built world with no districts reads an empty default.
    let mut w = v1_three();
    w.districts.clear();
    assert_eq!(w.district(DistrictId(3)).population, 0);
    assert_eq!(w.district_name(DistrictId(0)), "?");
    // Two days of history: the rate divides by 2, not 7.
    let mut w = v1_three();
    w.run_ticks(1);
    for d in &mut w.districts {
        d.crimes = [7u16, 7].into_iter().collect();
    }
    districts::aggregates(&mut w);
    for d in &w.districts {
        let expect = 7.0 / d.population as f32 * 100.0;
        assert!((d.crime_rate - expect).abs() < 1e-4, "{}: {} vs {expect}", d.name, d.crime_rate);
    }
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
    // Binned at midnight, before demography's midnight births and arrivals.
    let total: u32 = w.districts.iter().map(|d| d.population).sum();
    let late = w.stats.history.back().map_or(0, |r| r.births + r.immigrants) + w.stats.current.births;
    let citizens = w.citizens().len() as u32;
    assert!(total <= citizens && citizens - total <= late, "{total} binned of {citizens} ({late} born or arrived)");
    // Same CSV slots as the districts.
    let row = w.stats.history.back().expect("a day");
    assert_eq!(row.districts.len(), 8);
    assert_eq!(row.districts[3].1, w.districts[3].control.csv_code());
}

// ---------------------------------------------------------------------------
// Phase 2: the law in districts (docs/M12_DISTRICTS.md § 2, plan D9-D15, D38)
// ---------------------------------------------------------------------------

/// The three-district v1 city with the district law on.
fn law_city() -> World {
    let mut c = three_cfg();
    c.law.district_beats = true;
    World::new(7, c)
}

/// Mark district `d` inhabited with `adults` residents at `crime` per 100.
fn inhabit(w: &mut World, d: usize, adults: u32, crime: f32) {
    let x = &mut w.districts[d];
    x.adults = adults;
    x.population = adults;
    x.crime_rate = crime;
    x.classes = [0, adults, 0];
}

/// The captain, with the given traits.
fn captain(w: &mut World, courage: f32, lawfulness: f32) -> EntityId {
    let c = law_brain::recompute_captain(w).expect("a captain");
    let p = w.comp_mut::<Personality>(c).expect("personality");
    p.courage = courage;
    p.lawfulness = lawfulness;
    c
}

/// Adults with a Brain who are not guards or gang members, ascending.
fn civilians(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && citysim::systems::demography::is_adult(w, a))
        .filter(|&a| !law::is_guard(w, a) && w.gang_of(a).is_none() && !w.has::<Sentence>(a))
        .collect()
}

/// Put an agent out on the street at `tile`, homeless.
fn sleep_rough(w: &mut World, a: EntityId, tile: TilePos) {
    w.leave_building(a);
    w.comp_mut::<Household>(a).expect("household").home = None;
    let p = w.comp_mut::<Position>(a).expect("position");
    p.tile = tile;
    p.building = None;
}

#[test]
fn test_largest_remainder_is_exact_and_stable() {
    assert_eq!(largest_remainder(10, &[1.0, 1.0, 1.0]), vec![4, 3, 3], "ties go to the lower index");
    assert_eq!(largest_remainder(7, &[1.0, 2.0, 4.0]), vec![1, 2, 4]);
    assert_eq!(largest_remainder(5, &[0.0, 0.0]), vec![0, 0], "all zero deals nothing");
    assert_eq!(largest_remainder(0, &[1.0, 3.0]), vec![0, 0]);
    assert_eq!(largest_remainder(3, &[f32::NAN, -1.0, 2.0]), vec![0, 0, 3], "bad weights count as zero");
    for n in 0..40u32 {
        let w = [0.3, 1.7, 0.0, 2.2, 0.9];
        let out = largest_remainder(n, &w);
        assert_eq!(out.iter().map(|&x| u32::from(x)).sum::<u32>(), n, "sums to n");
        assert_eq!(out[2], 0, "a zero weight gets nothing");
        assert_eq!(out, largest_remainder(n, &w), "stable");
    }
}

#[test]
fn test_allocation_follows_crime_and_lever() {
    let mut w = law_city();
    w.config.law.alloc_base = 0.0;
    inhabit(&mut w, 0, 50, 1.0);
    inhabit(&mut w, 1, 50, 4.0);
    // District 2 has Blocks but nobody binned there: not inhabited.
    let patrol = law_brain::patrol_guards(&w);
    assert!(patrol.len() >= 3, "patrol guards: {}", patrol.len());
    law_brain::allocate(&mut w);
    let g: Vec<u8> = w.districts.iter().map(|d| d.guards).collect();
    assert_eq!(g.iter().map(|&x| usize::from(x)).sum::<usize>(), patrol.len(), "allocation sums to the roster");
    assert!(g[1] >= 2 * g[0] && g[1] > 0, "crime 4 vs 1 draws at least 2:1, got {g:?}");
    assert_eq!(g[2], 0, "an uninhabited district gets nobody");
    let beats = w.law().expect("law").beats.clone();
    assert_eq!(beats.keys().copied().collect::<Vec<_>>(), patrol, "every patrol guard has a beat");
    for (i, &n) in g.iter().enumerate() {
        assert_eq!(beats.values().filter(|d| d.index() == i).count(), usize::from(n));
    }
    // With the base back on, the higher-crime district still gets more.
    w.config.law.alloc_base = 1.0;
    law_brain::allocate(&mut w);
    assert!(
        w.districts[1].guards > w.districts[0].guards,
        "{:?}",
        w.districts.iter().map(|d| d.guards).collect::<Vec<_>>()
    );
    // The lever at 0 abandons the district: no guards, stance Withdrawn.
    captain(&mut w, 0.5, 0.9);
    w.levers.guard_weight[1] = 0.0;
    law_brain::allocate(&mut w);
    law_brain::rescore_stances(&mut w, 0.1, "test");
    assert_eq!(w.districts[1].guards, 0);
    assert_eq!(w.districts[1].stance, Stance::Withdrawn);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Stance && e.text.starts_with("Centre: Patrol -> Withdrawn")));
    // The beat routes the guard inside its district.
    let (&g0, &d0) = w.law().expect("law").beats.iter().next().expect("a beat");
    let route = law::new_patrol_route(&mut w, g0);
    let homes: Vec<EntityId> = route
        .iter()
        .copied()
        .filter(|&b| w.comp::<Building>(b).is_some_and(|x| x.kind == BuildingKind::Home))
        .collect();
    assert!(
        !homes.is_empty() && homes.iter().all(|&h| w.district_of_building(h) == d0),
        "the beat's Homes are in its district"
    );
}

#[test]
fn test_gang_landlord_draws_crackdown() {
    let mut w = law_city();
    w.config.law.gang_landlord_homes = 5;
    let gang = w.gang_list()[0];
    let mut homes: Vec<EntityId> = w.districts[1].homes.iter().take(5).copied().collect();
    homes.sort();
    assert_eq!(homes.len(), 5);
    w.comp_mut::<Gang>(gang).expect("gang").territory = homes;
    inhabit(&mut w, 1, 50, 1.0);
    w.districts[1].control = Controller::Gang(gang);
    w.districts[1].guards = 3;
    assert!(w.report_places.is_empty(), "no reports at all");
    assert_eq!(law_brain::gang_landlord(&w, DistrictId(1)), Some(gang));
    captain(&mut w, 1.0, 1.0);
    law_brain::rescore_stances(&mut w, 0.1, "test");
    assert_eq!(w.districts[1].stance, Stance::Crackdown(gang), "trace {:?}", w.districts[1].stance_trace);
    assert!(law::cracking_down_on(&w, gang), "the city-wide question sees the district Crackdown");
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::Stance).expect("a Stance event");
    assert!(e.actors.contains(&gang), "the target gang is an actor");
    // A gang landlord also lifts the allocation weight.
    let (_, terms) = law_brain::alloc_weight(&w, DistrictId(1));
    assert!(terms.iter().any(|&(k, v)| k == "gang landlord" && v > 0.0));
    // A taken bribe takes the gang off the target list.
    let now = w.tick;
    w.comp_mut::<Gang>(gang).expect("gang").paid_until = Some(now + 1000);
    assert_eq!(law_brain::gang_landlord(&w, DistrictId(1)), None);
}

#[test]
fn test_sweep_wins_with_rough_sleepers_and_lawful_captain() {
    let mut w = law_city();
    w.config.law.vagrancy_base = 0.0;
    let at = w.districts[2].centroid;
    let rough: Vec<EntityId> = civilians(&w).into_iter().take(12).collect();
    for &a in &rough {
        sleep_rough(&mut w, a, at);
    }
    street::vagrancy(&mut w);
    assert!(w.districts[2].rough >= 12, "rough {}", w.districts[2].rough);
    inhabit(&mut w, 2, 50, 1.0);
    w.districts[2].rough = 12;
    captain(&mut w, 0.5, 0.9);
    law_brain::rescore_stances(&mut w, 0.1, "test");
    assert_eq!(w.districts[2].stance, Stance::Sweep, "trace {:?}", w.districts[2].stance_trace);
    // The stance triples the sweep.
    w.config.law.vagrancy_base = 0.04;
    w.districts[2].coverage = 1.0;
    let swept = street::vagrancy_p(&w, DistrictId(2));
    w.districts[2].stance = Stance::Patrol;
    let patrol = street::vagrancy_p(&w, DistrictId(2));
    assert!((swept - 3.0 * patrol).abs() < 1e-6 && (patrol - 0.04).abs() < 1e-6, "{swept} vs {patrol}");
    // A timid captain does not sweep.
    w.districts[2].stance = Stance::Patrol;
    captain(&mut w, 0.5, 0.3);
    law_brain::rescore_stances(&mut w, 0.1, "test");
    assert_eq!(w.districts[2].stance, Stance::Patrol);
}

#[test]
fn test_withdrawn_pin_empties_the_beat_on_the_same_deal() {
    // M12 review: the deal runs before the stances are rescored (Crackdown's
    // gate reads today's guards), so the allocation reads a stance pin as it
    // stands: a fresh Withdrawn pin gets no guards on the very deal that
    // applies it, not one deal later.
    let mut w = law_city();
    for d in 0..2 {
        inhabit(&mut w, d, 50, 1.0);
    }
    captain(&mut w, 0.5, 0.9);
    law_brain::redeal(&mut w, "test");
    assert!(w.districts[1].guards > 0, "district 1 starts with guards");
    assert_ne!(w.districts[1].stance, Stance::Withdrawn);
    w.levers.stance_pin[1] = Some(Stance::Withdrawn);
    law_brain::redeal(&mut w, "pinned");
    assert_eq!(w.districts[1].guards, 0, "no guards on the same deal");
    assert!(w.law().expect("law").beats.values().all(|d| d.index() != 1), "no beat in district 1");
    assert_eq!(w.districts[1].stance, Stance::Withdrawn);
    let (_, terms) = law_brain::alloc_weight(&w, DistrictId(1));
    assert!(terms.iter().any(|&(k, v)| k == "off" && v == 1.0), "{terms:?}");
    // Unpinned, the district draws guards again on the same deal, though its
    // stance still read Withdrawn when the deal ran.
    w.levers.stance_pin[1] = None;
    law_brain::redeal(&mut w, "unpinned");
    assert!(w.districts[1].guards > 0, "guards back once the pin is lifted");
    assert_ne!(w.districts[1].stance, Stance::Withdrawn);
}

#[test]
fn test_garrison_zeroes_allocations() {
    let mut w = law_city();
    for d in 0..3 {
        inhabit(&mut w, d, 50, 1.0 + d as f32);
    }
    law_brain::allocate(&mut w);
    assert!(w.districts.iter().any(|d| d.guards > 0));
    w.districts[0].stance = Stance::Sweep;
    captain(&mut w, 0.5, 0.9);
    w.law_mut().expect("law").pinned = Some(Posture::Garrison);
    law_brain::rescore(&mut w, 0.0, "pinned");
    assert_eq!(w.law().expect("law").posture, Posture::Garrison);
    law_brain::allocate(&mut w);
    assert!(w.districts.iter().all(|d| d.guards == 0));
    assert!(w.law().expect("law").beats.is_empty());
    law_brain::rescore_stances(&mut w, 0.1, "daily");
    assert!(w.districts.iter().all(|d| d.stance == Stance::Patrol));
}

#[test]
fn test_max_crackdowns_respected() {
    let mut w = law_city();
    let gang = w.gang_list()[0];
    for d in 0..3 {
        inhabit(&mut w, d, 50, 1.0);
        w.districts[d].guards = 3;
        for _ in 0..20 {
            w.report_places.push_back((0, gang, DistrictId(d as u8)));
        }
    }
    captain(&mut w, 1.0, 1.0);
    law_brain::rescore_stances(&mut w, 0.1, "test");
    let n = w.districts.iter().filter(|d| matches!(d.stance, Stance::Crackdown(_))).count();
    assert_eq!(n, 2, "max_crackdowns = 2");
    // The third has the gate shut, not merely a lower score.
    let shut = w.districts.iter().find(|d| d.stance == Stance::Patrol).expect("one left on Patrol");
    assert!(shut.stance_trace.iter().all(|s| !matches!(s.stance, Stance::Crackdown(_))));
    // The holders keep their slots on the next rescoring.
    let before: Vec<Stance> = w.districts.iter().map(|d| d.stance).collect();
    law_brain::rescore_stances(&mut w, 0.1, "test");
    assert_eq!(w.districts.iter().map(|d| d.stance).collect::<Vec<_>>(), before);
}

#[test]
fn test_vagrancy_fines_payer_and_jails_broke_vagrant() {
    let mut w = law_city();
    w.config.law.vagrancy_base = 1.0;
    let at = w.districts[0].centroid;
    let civ = civilians(&w);
    let (payer, broke, full) = (civ[0], civ[1], civ[2]);
    // The tier first: a change of tier may move an agent.
    lod::set_lod(&mut w, payer, Lod::Statistical);
    lod::set_lod(&mut w, broke, Lod::Statistical);
    lod::set_lod(&mut w, full, Lod::Full);
    for a in [payer, broke, full] {
        sleep_rough(&mut w, a, at);
    }
    w.comp_mut::<Wallet>(payer).expect("wallet").coins = 10;
    w.comp_mut::<Wallet>(broke).expect("wallet").coins = 0;
    w.comp_mut::<Wallet>(full).expect("wallet").coins = 0;
    w.districts[0].coverage = 1.0;
    let treasury = w.treasury().expect("treasury").coins;
    let now = w.tick;
    street::vagrancy(&mut w);
    assert_eq!(w.comp::<Wallet>(payer).expect("wallet").coins, 7, "the payer pays the fine");
    let fines: i64 = w
        .events
        .iter()
        .filter(|e| e.kind == EventKind::Vagrancy && e.text.contains("fined"))
        .map(|e| if e.actors.contains(&payer) { 3 } else { 0 })
        .sum();
    assert_eq!(fines, 3);
    assert!(w.treasury().expect("treasury").coins >= treasury + 3, "the fine goes to the Treasury, untaxed");
    let s = w.comp::<Sentence>(broke).expect("the broke Statistical vagrant is jailed");
    assert_eq!(s.crime, Crime::Vagrancy);
    assert_eq!(s.until_tick - now, 600, "one night");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Vagrancy && e.text.contains("jailed for the night")));
    assert!(
        w.crime_reports().iter().any(|r| r.suspect == full && r.crime == Crime::Vagrancy && !r.resolved),
        "a Full vagrant is reported for the arrest path"
    );
    assert!(!w.has::<Sentence>(full));
    // Escorted to the Precinct in another district, the sentence still counts where they slept.
    let logged = w.districts[0].vagrancy_log.len();
    let far = w.districts[2].centroid;
    w.comp_mut::<Position>(full).expect("position").tile = far;
    let guard = law_brain::guards(&w)[0];
    law::jail_suspect(&mut w, guard, full);
    assert_eq!(w.comp::<Sentence>(full).map(|s| s.crime), Some(Crime::Vagrancy));
    assert_eq!(w.districts[0].vagrancy_log.len(), logged + 1, "logged where swept");
    assert!(w.stats.current.vagrancy >= 2);
    assert!(w.districts[0].vagrancy_log.len() >= 3);
    // A night in the cells is the least crime: it never sets a sentence over a real one,
    // and it is no convict for a BreakOut.
    assert!(Crime::Vagrancy < Crime::Theft);
    // A full Precinct is never emptied for a vagrant: they are moved on.
    let other = civ[3];
    lod::set_lod(&mut w, other, Lod::Statistical);
    sleep_rough(&mut w, other, at);
    w.comp_mut::<Wallet>(other).expect("wallet").coins = 0;
    w.config.buildings.jail.capacity = w.sentenced().len() as u8;
    street::vagrancy(&mut w);
    assert!(!w.has::<Sentence>(other), "no cell is freed for a vagrant");
}

#[test]
fn test_raid_gated_by_crackdown_on_raider() {
    let cfg = Config::load().gangs;
    let base = faction::OrderInputs {
        frontier: 0,
        frontier_total: 10,
        rival_territory: 0,
        own: 10,
        rival: 5,
        heat: 0.0,
        prize: 400,
        grudge: true,
        greed: 0.5,
        courage: 0.8,
        pride: 0.8,
        raid_ready: true,
        rival_exists: true,
        sacked: false,
        jailed: 3,
        boss_jailed: true,
        breakout_ready: true,
        garrison: false,
        loyalty: 0.8,
        hoard: 0.0,
        hoard_corp: None,
        hoard_tilt: 0.0,
        target_cover: 0.3,
        jail_cover: 0.3,
        derelicts: 0,
        districts_held: 0,
        open_districts: 0,
        corp_prize: None,
        corp_cover: 0.0,
        corp_guards: 0,
        corp_raids: false,
        clinic_exists: false,
        harvest_target: None,
        harvest_cover: 0.0,
        lawfulness: 0.5,
        treasury_x: 0.0,
    };
    let has = |i: &faction::OrderInputs, o: Order| faction::score_orders(i, &cfg).iter().any(|s| s.order == o);
    for o in [Order::Raid, Order::Retaliate, Order::BreakOut] {
        assert!(has(&base, o), "{o:?} scores under partial cover");
    }
    let covered = faction::OrderInputs { target_cover: 1.0, jail_cover: 1.0, ..base.clone() };
    for o in [Order::Raid, Order::Retaliate, Order::BreakOut] {
        assert!(!has(&covered, o), "{o:?} is gated under full cover");
    }
    // The cover term lowers the score below full cover.
    let open = faction::OrderInputs { target_cover: 0.0, jail_cover: 0.0, ..base.clone() };
    let score = |i: &faction::OrderInputs| {
        faction::score_orders(i, &cfg).iter().find(|s| s.order == Order::Raid).map(|s| s.score).expect("Raid")
    };
    assert!(score(&open) > score(&base));
}

#[test]
fn test_breakout_not_gated_by_civic_coverage() {
    let mut w = v2_world();
    assert!(w.config.law.district_beats, "the assets turn the district law on");
    districts::daily(&mut w);
    let gang = w.gang_list()[0];
    let cover = faction::target_cover(&w, gang, Order::BreakOut);
    assert!(cover < 1.0 && (cover - 1.0 / 3.0).abs() < 1e-5, "the Civic reads coverage 1.0: {cover}");
    // Garrison shuts it.
    w.law_mut().expect("law").posture = Posture::Garrison;
    assert_eq!(faction::target_cover(&w, gang, Order::BreakOut), 1.0);
    w.law_mut().expect("law").posture = Posture::Patrol;
    // A Crackdown on the raider (or a Cordon) where the rival's Hideout stands shuts the raids.
    let rival = w.rival_of(gang).expect("a rival");
    let hq = w.hideout_of(rival).expect("a Hideout");
    let d = w.district_of_building(hq);
    assert!(faction::target_cover(&w, gang, Order::Raid) < 1.0);
    w.district_mut(d).stance = Stance::Crackdown(gang);
    assert_eq!(faction::target_cover(&w, gang, Order::Raid), 1.0);
    assert!(faction::target_cover(&w, rival, Order::Raid) < 1.0, "only against the raider");
    w.district_mut(d).stance = Stance::Cordon;
    assert_eq!(faction::target_cover(&w, gang, Order::Raid), 1.0);
    // Off with the district law: the M11 brain.
    w.config.law.district_beats = false;
    assert_eq!(faction::target_cover(&w, gang, Order::Raid), 0.0);
}

#[test]
fn test_report_carries_its_district() {
    let mut w = v2_world();
    let gang = w.gang_list()[0];
    let m = civilians(&w)[0];
    gang::enlist(&mut w, m, gang);
    assert_eq!(w.gang_of(m), Some(gang));
    let tile = w.comp::<Position>(m).expect("position").tile;
    let d = w.district_of(tile);
    let before = w.report_places.len();
    law::file_report(&mut w, Crime::Theft, m, None);
    assert_eq!(w.report_places.len(), before + 1);
    assert_eq!(*w.report_places.back().expect("a place"), (w.tick, gang, d));
    assert_eq!(law_brain::top_gang(&w, d).map(|(g, _)| g), Some(gang));
}

#[test]
fn test_binder_draws_from_district() {
    let mut w = v2_world();
    assert_eq!(w.config.bind.same_zone_weight, 0.5);
    let civ = civilians(&w);
    let victim = civ[0];
    let picks: Vec<EntityId> = civ[1..]
        .iter()
        .copied()
        .filter(|&a| Some(a) != w.spouse_of(victim) && !w.enemies.get(&victim).is_some_and(|s| s.contains(&a)))
        .take(3)
        .collect();
    let rows = [(DistrictId(5), Zone::Sump), (DistrictId(6), Zone::Sump), (DistrictId(3), Zone::Mid)];
    for (&a, &(district, zone)) in picks.iter().zip(&rows) {
        let t = DayTrace { zone, district, flags: citysim::trace_flags::ALIVE, hunger: 0, mood: 0 };
        w.insert(a, Trace::default());
        w.comp_mut::<Trace>(a).expect("trace").push(0, t, 7);
    }
    let hole = Hole {
        id: 99,
        kind: HoleKind::Robbed,
        victim,
        zone: Zone::Sump,
        district: DistrictId(5),
        tick: 10,
        event_id: 0,
        consequential: false,
        spouse: w.spouse_of(victim),
        loot: 0,
        home: None,
        gang: None,
    };
    let cands = bind::candidates(&w, &hole);
    let weight = |a: EntityId| cands.iter().find(|&&(c, _)| c == a).map(|&(_, x)| x).expect("a candidate");
    let (same, zone, other) = (weight(picks[0]), weight(picks[1]), weight(picks[2]));
    assert!((zone / same - 0.5).abs() < 1e-9, "same zone, other district: x0.5");
    assert!((other / same - 0.25).abs() < 1e-9, "another zone: x0.25");
}

#[test]
fn test_coverage_per_district() {
    let mut w = v2_world();
    // Guard-hours in proportion to Homes read 1.0 everywhere with Homes.
    let homes: Vec<u32> = w.districts.iter().map(|d| d.homes.len() as u32).collect();
    for (i, &h) in homes.iter().enumerate() {
        w.district_watch.yesterday[i] = 10 * h;
    }
    for (i, d) in w.districts.iter().enumerate() {
        let c = bind::district_coverage(&w, DistrictId(i as u8));
        if d.homes.is_empty() {
            assert_eq!(c, w.config.bind.coverage_max, "{} has no Homes", d.name);
        } else {
            assert!((c - 1.0).abs() < 1e-5, "{}: {c}", d.name);
        }
    }
    // Every hour in Sump West: it reads the max, the rest the floor.
    w.district_watch.yesterday = [0; 12];
    w.district_watch.yesterday[5] = 1000;
    assert_eq!(bind::district_coverage(&w, DistrictId(5)), w.config.bind.coverage_max);
    assert_eq!(bind::district_coverage(&w, DistrictId(6)), w.config.bind.coverage_min);
    // The M11 zone coverage is the district watch summed by zone: the Sump's
    // 1000 hours over its 200 Homes against 1000 over 400.
    let z = bind::zone_law_coverage(&w, Zone::Sump);
    let expect = (1000.0 / 200.0) / (1000.0 / 400.0);
    assert!((z - f32::min(expect, w.config.bind.coverage_max)).abs() < 1e-5, "{z}");
    // The binder's witness roll reads the cached District.coverage.
    districts::daily(&mut w);
    assert_eq!(w.districts[5].coverage, w.config.bind.coverage_max);
}

#[test]
fn test_private_guard_fills_thin_district() {
    let mut w = v2_world();
    districts::daily(&mut w);
    // A corp with an unsecured building in an inhabited district.
    let (corp, b) = w
        .corps()
        .into_iter()
        .find_map(|c| {
            let bs = w.comp::<citysim::Corp>(c)?.buildings.clone();
            bs.into_iter()
                .find(|&b| {
                    w.comp::<Building>(b).is_some_and(|x| x.secured_by.is_none())
                        && !w.district(w.district_of_building(b)).homes.is_empty()
                })
                .map(|b| (c, b))
        })
        .expect("a corp building in an inhabited district");
    for d in &mut w.districts {
        d.coverage = 2.0;
    }
    assert_eq!(corp_brain::at_risk_share(&w, corp), 0.0, "well covered: nothing at risk");
    let d = w.district_of_building(b);
    w.district_mut(d).coverage = 0.5;
    let share = corp_brain::at_risk_share(&w, corp);
    assert!(share > 0.0);
    let i = corp_brain::gather_inputs(&w, corp).expect("inputs");
    assert!(i.losses >= share * w.config.law.private_fill_weight - 1e-6, "losses {} share {share}", i.losses);
    // Secure contracts the thinly covered building even without a loss.
    w.comp_mut::<citysim::Corp>(corp).expect("corp").order = citysim::CorpOrder::Secure;
    corp_brain::act(&mut w, corp);
    let thin: Vec<EntityId> = w
        .comp::<citysim::Corp>(corp)
        .expect("corp")
        .buildings
        .iter()
        .copied()
        .filter(|&x| corp_brain::thinly_covered(&w, x))
        .collect();
    assert!(
        thin.iter().any(|&x| w.comp::<Building>(x).is_some_and(|bd| bd.secured_by.is_some())),
        "a private guard contract where the law is thin"
    );
}

#[test]
fn test_district_beats_run_daily_and_survive_a_save() {
    let mut w = World::new(19, Config::load().scaled_to(300));
    w.run_ticks(TICKS_PER_DAY + 10);
    assert!(!w.law().expect("law").beats.is_empty(), "the first midnight deals the beats");
    let alloc: u32 = w.districts.iter().map(|d| u32::from(d.guards)).sum();
    assert_eq!(alloc as usize, w.law().expect("law").beats.len());
    assert!(w.districts.iter().all(|d| !d.alloc_trace.is_empty()));
    // A pre-M12 save (no beats, no report places) loads and deals at the next midnight.
    let text = save::to_ron(&w);
    let text = text.replacen("beats:{", "beats_gone:{", 1);
    let stripped = {
        let start = text.find("beats_gone:{").expect("beats in the save");
        let mut depth = 0usize;
        let mut end = start;
        for (i, ch) in text[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = if text[end..].starts_with(',') { end + 1 } else { end };
        format!("{}{}", &text[..start], &text[end..])
    };
    let mut back = save::from_ron(&stripped).expect("a save without beats loads");
    assert!(back.law().expect("law").beats.is_empty());
    back.run_ticks(TICKS_PER_DAY);
    assert!(!back.law().expect("law").beats.is_empty());
    // A current save round-trips its beats.
    let text = save::to_ron(&w);
    assert_eq!(save::to_ron(&save::from_ron(&text).expect("loads")), text);
}

#[test]
fn test_full_precinct_frees_the_vagrant_first() {
    let mut w = law_city();
    let jail = w.building_of_kind(BuildingKind::Jail).expect("a Precinct");
    let civ = civilians(&w);
    let (vagrant, thief, assailant) = (civ[0], civ[1], civ[2]);
    let now = w.tick;
    law::sentence(&mut w, vagrant, Crime::Vagrancy, now + 600, jail);
    law::sentence(&mut w, thief, Crime::Theft, now + 3 * TICKS_PER_DAY, jail);
    w.config.buildings.jail.capacity = w.sentenced().len() as u8;
    law::file_report(&mut w, Crime::Assault, assailant, None);
    let guard = law_brain::guards(&w)[0];
    law::jail_suspect(&mut w, guard, assailant);
    assert!(w.has::<Sentence>(assailant), "the assailant is jailed");
    assert!(!w.has::<Sentence>(vagrant), "the vagrant makes room");
    assert!(w.has::<Sentence>(thief), "the thief stays");
    // A vagrant brought to a full Precinct is fined or let go, never jailed over a convict.
    let late = civ[3];
    law::file_report(&mut w, Crime::Vagrancy, late, None);
    w.comp_mut::<Wallet>(late).expect("wallet").coins = 0;
    w.config.buildings.jail.capacity = w.sentenced().len() as u8;
    law::jail_suspect(&mut w, guard, late);
    assert!(!w.has::<Sentence>(late));
    assert!(w.has::<Sentence>(thief) && w.has::<Sentence>(assailant));
}

// ---------------------------------------------------------------------------
// Phase 4: unrest, riots, crossfire, strikes (docs/M12_DISTRICTS.md § 6, plan D29-D35, D41)
// ---------------------------------------------------------------------------

/// A v2 city with today's aggregates, every corp at price level 1.0 (so a
/// riot's grievances are the ones a test sets), crossfire off, no deaths.
fn riot_city() -> World {
    let mut w = v2_world();
    w.config.riots.p_crossfire = 0.0;
    w.config.crime.fight_death_p = 0.0;
    for c in w.corps() {
        let cc = w.comp_mut::<citysim::Corp>(c).expect("corp");
        for v in cc.price_level.values_mut() {
            *v = 1.0;
        }
    }
    districts::daily(&mut w);
    w
}

/// Make `n` residents of `d` eligible rioters (lawless, miserable, not Corp
/// class, no gang, no guard); returns them ascending.
fn agitate(w: &mut World, d: usize, n: usize) -> Vec<EntityId> {
    let execs = classes::exec_set(w);
    let picks: Vec<EntityId> = w.districts[d]
        .residents
        .iter()
        .copied()
        .filter(|&a| !law::is_guard(w, a) && w.gang_of(a).is_none() && !w.has::<Sentence>(a))
        .filter(|&a| classes::class_in(w, a, &execs) != citysim::Class::Corp)
        .filter(|&a| w.comp::<Brain>(a).is_some_and(|b| !b.emigrating))
        .take(n)
        .collect();
    assert_eq!(picks.len(), n, "district {d} has {n} non-corp residents");
    for &a in &picks {
        w.comp_mut::<Personality>(a).expect("p").lawfulness = 0.1;
        w.comp_mut::<Mood>(a).expect("mood").value = -0.6;
    }
    picks
}

/// Every guard (city and private) far away, off any beat.
fn guards_away(w: &mut World) {
    let far = TilePos { x: 250, y: 2 };
    for g in w.guards().to_vec() {
        w.abort_plan(g);
        w.leave_building(g);
        let p = w.comp_mut::<Position>(g).expect("pos");
        p.tile = far;
        p.building = None;
    }
    if let Some(l) = w.law_mut() {
        l.beats.clear();
    }
}

/// A rioter standing at `tile` on the Raid goal, with a body.
fn stage_rioter(w: &mut World, a: EntityId, tile: TilePos) {
    lod::set_lod(w, a, Lod::Coarse);
    w.abort_plan(a);
    w.leave_building(a);
    let p = w.comp_mut::<Position>(a).expect("pos");
    p.tile = tile;
    p.building = None;
    w.comp_mut::<Brain>(a).expect("brain").current_goal = Some(citysim::GoalKind::Raid);
}

#[test]
fn test_riot_fires_at_streak_and_picks_target() {
    use citysim::systems::riot;
    let mut w = riot_city();
    // An inhabited district with a corp-owned Block.
    let (d, block) = (0..w.districts.len())
        .find_map(|i| w.districts[i].homes.iter().copied().find(|&h| w.corp_of_building(h).is_some()).map(|h| (i, h)))
        .expect("a corp Block");
    let rioters = agitate(&mut w, d, 8);
    // The evicting corp's Block (3 evictions in 14 days) over the Precinct (1 Vagrancy).
    let corp = w.corp_of_building(block);
    let now = w.tick;
    for _ in 0..3 {
        w.eviction_places.push_back((now, DistrictId(d as u8), corp));
    }
    w.districts[d].vagrancy_log.push_back(now);
    let (target, kind, grievance) = riot::score_targets(&w, DistrictId(d as u8)).expect("a target");
    assert_eq!(target, block, "the evicting landlord's Block");
    assert_eq!(kind, citysim::RiotTarget::Corp);
    assert!((grievance - 3.0).abs() < 1e-5, "{grievance}");
    // One midnight short of the streak: nothing.
    w.districts[d].unrest_streak = w.config.riots.riot_days - 1;
    riot::trigger(&mut w);
    assert!(w.riots.is_empty());
    w.districts[d].unrest_streak = w.config.riots.riot_days;
    riot::trigger(&mut w);
    assert_eq!(w.riots.len(), 1);
    let r = w.riots[0].clone();
    assert_eq!(r.district, DistrictId(d as u8));
    assert_eq!(r.target, block);
    assert!(r.rioters.len() >= w.config.riots.riot_min && r.rioters.len() <= w.config.riots.riot_max);
    for a in &rioters {
        assert!(r.rioters.contains(a), "every agitated resident marches");
        assert_eq!(w.rioter_of.get(a), Some(&r.id));
    }
    assert_eq!(r.muster_at % TICKS_PER_DAY, 20 * citysim::TICKS_PER_HOUR, "musters at 20:00");
    assert!(riot::riot_in(&w, r.district));
    // The rioters run the Raid goal's machinery.
    let a = rioters[0];
    assert_eq!(citysim::systems::raid::target_building(&w, a), Some(block));
    assert!(!citysim::systems::raid::raid_done(&w, a));
    assert!(matches!(citysim::systems::raid::muster_point(&w, a), Some(citysim::systems::raid::MusterAt::Door(_))));
    // No second riot in a district already rising, and the cooldown holds after it.
    riot::trigger(&mut w);
    assert_eq!(w.riots.len(), 1);
}

/// Build a riot by hand at `target` with `rioters` staged at its door.
fn hand_riot(w: &mut World, d: usize, target: EntityId, rioters: &[EntityId], response: citysim::RiotResponse) -> u32 {
    let now = w.tick;
    let id = w.next_riot_id;
    w.next_riot_id += 1;
    w.riots.push(citysim::Riot {
        id,
        district: DistrictId(d as u8),
        target,
        kind: citysim::RiotTarget::Corp,
        muster: target,
        muster_at: now,
        rioters: rioters.to_vec(),
        response,
        started: now,
        departed: Some(now),
    });
    for &a in rioters {
        w.rioter_of.insert(a, id);
    }
    let door = w.comp::<Building>(target).map(|b| w.outside_door(b)).expect("door");
    for &a in rioters {
        stage_rioter(w, a, door);
    }
    id
}

#[test]
fn test_riot_win_loots_market_and_closes_it_and_crush_raises_fear() {
    let mut w = riot_city();
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let d = w.district_of_building(market).index();
    let rioters = agitate(&mut w, 5, 6);
    guards_away(&mut w);
    for s in citysim::systems::ownership::staff_at(&w, market) {
        w.leave_building(s);
    }
    w.comp_mut::<Building>(market).expect("b").stock_food = 100;
    let food_before: u32 = rioters.iter().map(|&a| w.comp::<citysim::Inventory>(a).map_or(0, |i| i.food)).sum();
    let fear_before = districts::district_fear(&w)[d];
    hand_riot(&mut w, d, market, &rioters, citysim::RiotResponse::Crush);
    let out = citysim::systems::raid::resolve(&mut w, rioters[0]);
    assert_eq!(out, Some(citysim::systems::raid::Outcome::Won), "no defenders: the riot wins");
    assert_eq!(w.comp::<Building>(market).expect("b").stock_food, 70, "30 % looted");
    let food_after: u32 = rioters.iter().map(|&a| w.comp::<citysim::Inventory>(a).map_or(0, |i| i.food)).sum();
    assert_eq!(food_after, food_before + 30, "into the rioters' inventories");
    assert!(w.is_closed(market), "closed for riot_close_days");
    assert!(count(&w, EventKind::Looted) == 1);
    assert!(w
        .events
        .iter()
        .any(|e| e.kind == EventKind::Riot && e.text.contains("rioted at") && e.text.contains("6 rioters")));
    assert!(w.riots.is_empty() && w.rioter_of.is_empty(), "the riot is over");
    assert_eq!(w.stats.current.riots, 1);
    assert_eq!(w.districts[d].unrest_streak, 0);
    assert_eq!(w.districts[d].last_riot, Some(w.tick));
    // A closed Market sells nothing: nobody shops there.
    assert!(w.market_by_price(w.comp::<Building>(market).expect("b").door) != Some(market));
    // Crush: fear + 0.3 for 14 days.
    assert!(w.districts[d].crush_until.is_some_and(|t| t > w.tick));
    let fear_after = districts::district_fear(&w)[d];
    assert!((fear_after - (fear_before + 0.3).min(1.0)).abs() < 1e-5, "{fear_before} -> {fear_after}");
}

/// Fix pass (item 39): a won riot on the Precinct frees convicts but closes
/// nothing (only a Market, Bar or Hotel closes), and a closed Market takes
/// no restock.
#[test]
fn test_riot_closes_only_trade_and_a_closed_market_takes_no_delivery() {
    let mut w = riot_city();
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let d = w.district_of_building(jail).index();
    let rioters = agitate(&mut w, 5, 6);
    guards_away(&mut w);
    hand_riot(&mut w, d, jail, &rioters, citysim::RiotResponse::Contain);
    let out = citysim::systems::raid::resolve(&mut w, rioters[0]);
    assert_eq!(out, Some(citysim::systems::raid::Outcome::Won));
    assert!(!w.is_closed(jail), "the Precinct does not close");
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let until = w.tick + TICKS_PER_DAY;
    {
        let b = w.comp_mut::<Building>(market).expect("b");
        b.stock_food = 0;
        b.closed_until = Some(until);
    }
    while !w.tick.is_multiple_of(TICKS_PER_DAY) {
        w.tick += 1;
    }
    citysim::systems::economy::run(&mut w);
    assert_eq!(w.comp::<Building>(market).expect("b").stock_food, 0, "a closed Market is not restocked");
}

#[test]
fn test_riot_lost_jails_the_beaten_and_too_few_disperse() {
    let mut w = riot_city();
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let d = w.district_of_building(market).index();
    let rioters = agitate(&mut w, 5, 6);
    guards_away(&mut w);
    // Three hard guards at the door against six soft rioters.
    let door = w.comp::<Building>(market).map(|b| w.outside_door(b)).expect("door");
    let guards: Vec<EntityId> = law_brain::guards(&w).into_iter().take(3).collect();
    for &g in &guards {
        let p = w.comp_mut::<Position>(g).expect("pos");
        p.tile = door;
        w.comp_mut::<citysim::Skills>(g).expect("s").fighting = 1.0;
        w.comp_mut::<Personality>(g).expect("p").courage = 1.0;
    }
    for &a in &rioters {
        w.comp_mut::<citysim::Skills>(a).expect("s").fighting = 0.0;
        w.comp_mut::<Personality>(a).expect("p").courage = 0.0;
    }
    hand_riot(&mut w, d, market, &rioters, citysim::RiotResponse::Contain);
    let out = citysim::systems::raid::resolve(&mut w, rioters[0]);
    assert_eq!(out, Some(citysim::systems::raid::Outcome::Lost));
    let jailed = rioters.iter().filter(|&&a| w.comp::<Sentence>(a).is_some_and(|s| s.crime == Crime::Assault)).count();
    assert!(jailed >= 1, "rioters who lost a pairing are jailed");
    assert!(!w.is_closed(market));
    assert_eq!(count(&w, EventKind::Looted), 0);
    // Two at the door (fewer than riot_min / 2): dispersed without a fight.
    let mut w2 = riot_city();
    let few = agitate(&mut w2, 5, 6);
    guards_away(&mut w2);
    hand_riot(&mut w2, d, market, &few, citysim::RiotResponse::Contain);
    for &a in &few[2..] {
        w2.comp_mut::<Brain>(a).expect("b").current_goal = None;
    }
    let assaults = count(&w2, EventKind::Assault);
    let riots = w2.stats.current.riots;
    let unrest = w2.districts[d].unrest;
    let litter_before: u64 = w2.litter.iter().map(|&v| u64::from(v)).sum();
    citysim::systems::raid::resolve(&mut w2, few[0]);
    assert!(w2.events.iter().any(|e| e.kind == EventKind::Riot && e.text.contains("dispersed")));
    assert_eq!(count(&w2, EventKind::Assault), assaults, "no fight");
    // Fix pass (item 34): a dispersed crowd is a fizzle: no count, no vent, no mess.
    assert_eq!(w2.stats.current.riots, riots);
    assert_eq!(w2.districts[d].unrest, unrest);
    assert_eq!(w2.litter.iter().map(|&v| u64::from(v)).sum::<u64>(), litter_before);
    assert_eq!(w2.districts[d].last_riot, Some(w2.tick), "the cooldown still runs");
}

#[test]
fn test_riot_dissolves_after_march_window() {
    use citysim::systems::riot;
    let mut w = riot_city();
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let rioters = agitate(&mut w, 5, 6);
    hand_riot(&mut w, 5, market, &rioters, citysim::RiotResponse::Disperse);
    riot::run(&mut w);
    assert_eq!(w.riots.len(), 1, "inside the march window");
    w.tick += citysim::systems::faction::RAID_MARCH_TICKS;
    riot::run(&mut w);
    assert!(w.riots.is_empty() && w.rioter_of.is_empty());
    assert!(w.events.iter().any(|e| e.kind == EventKind::Riot && e.text.contains("fizzled")));
    assert_eq!(w.stats.current.riots, 0, "a fizzled riot is not counted");
    assert!(w.districts[5].last_riot.is_some(), "the cooldown starts");
}

#[test]
fn test_crossfire_hits_only_non_parties_within_radius() {
    let mut w = riot_city();
    w.config.riots.p_crossfire = 1.0;
    w.config.riots.p_crossfire_kill = 0.0;
    let door = TilePos { x: 39, y: 39 };
    let civ = civilians(&w);
    let (attacker, defender, near, far) = (civ[0], civ[1], civ[2], civ[3]);
    let spots = [(attacker, door), (defender, door), (near, TilePos { x: 42, y: 39 }), (far, TilePos { x: 43, y: 39 })];
    for (a, t) in spots {
        lod::set_lod(&mut w, a, Lod::Coarse);
        w.abort_plan(a);
        w.leave_building(a);
        let p = w.comp_mut::<Position>(a).expect("pos");
        p.tile = t;
        p.building = None;
    }
    // Nobody else stands within the radius.
    for b in w.bodies() {
        if ![attacker, defender, near, far].contains(&b) {
            if let Some(p) = w.comp_mut::<Position>(b) {
                if law::chebyshev(p.tile, door) <= 4 {
                    p.tile = TilePos { x: 250, y: 2 };
                    p.building = None;
                }
            }
        }
    }
    let caught = |w: &World, a: EntityId| {
        w.comp::<citysim::Memory>(a)
            .is_some_and(|m| m.entries.iter().any(|e| e.kind == citysim::MemoryKind::CaughtInCrossfire))
    };
    // Fix pass (item 37): the attacker's gang-mate at 2 tiles is on a side, not a bystander.
    let mate = civ[4];
    let g = w.gang_list()[0];
    citysim::systems::gang::enlist(&mut w, attacker, g);
    citysim::systems::gang::enlist(&mut w, mate, g);
    lod::set_lod(&mut w, mate, Lod::Coarse);
    w.abort_plan(mate);
    w.leave_building(mate);
    {
        let p = w.comp_mut::<Position>(mate).expect("pos");
        p.tile = TilePos { x: 41, y: 39 };
        p.building = None;
    }
    let crimes_before: u32 = w.districts.iter().map(|d| u32::from(d.crimes_today)).sum();
    let hits = citysim::systems::raid::crossfire(&mut w, door, &[attacker, defender], attacker, None, "a door");
    assert_eq!(hits, 1);
    assert!(!caught(&w, mate), "the attacker's own side is never a bystander");
    // Fix pass (item 36): the hit is a crime (counted, an Assault raised).
    let crimes_after: u32 = w.districts.iter().map(|d| u32::from(d.crimes_today)).sum();
    assert_eq!(crimes_after, crimes_before + 1, "the hit is raised as an Assault");
    assert!(caught(&w, near), "the bystander at 3 is hit");
    assert!(!caught(&w, far), "the one at 4 is not");
    assert!(!caught(&w, attacker) && !caught(&w, defender), "the parties never are");
    assert_eq!(count(&w, EventKind::Crossfire), 1);
    assert_eq!(w.stats.current.crossfire, 1);
    assert!(w.comp::<citysim::Needs>(near).is_some_and(|n| n.safety <= 0.4 + 1e-5));
    // Off: no roll at all.
    w.config.riots.p_crossfire = 0.0;
    assert_eq!(citysim::systems::raid::crossfire(&mut w, door, &[attacker], attacker, None, "a door"), 0);
}

#[test]
fn test_district_unrest_reads_rent_burden_and_counts_the_streak() {
    let mut w = riot_city();
    let d = 5usize;
    let before = w.districts[d].unrest;
    assert!(w.districts[d].trace.iter().any(|(k, _)| *k == "rent_burden"));
    w.config.classes.rent_burden_w = 0.0;
    districts::unrest(&mut w);
    let without = w.districts[d].unrest;
    assert!(before >= without, "rent burden adds unrest: {before} vs {without}");
    w.config.riots.riot_threshold = -1.0;
    let s = w.districts[d].unrest_streak;
    districts::unrest(&mut w);
    assert_eq!(w.districts[d].unrest_streak, s + 1);
    w.config.riots.riot_threshold = 10.0;
    districts::unrest(&mut w);
    assert_eq!(w.districts[d].unrest_streak, 0);
}

#[test]
fn test_district_strike_hits_highest_priced_employer_of_residents() {
    let mut w = riot_city();
    w.tick = TICKS_PER_DAY;
    // A district with residents working for two corps or more.
    let mut pick = None;
    for (i, d) in w.districts.iter().enumerate() {
        let mut corps: Vec<EntityId> = d
            .residents
            .iter()
            .filter_map(|&a| w.comp::<citysim::Job>(a).and_then(|j| j.employer).and_then(|e| w.corp_of_building(e)))
            .collect();
        corps.sort_unstable();
        corps.dedup();
        if corps.len() >= 2 {
            pick = Some((i, corps));
            break;
        }
    }
    let (d, corps) = pick.expect("a district employed by two corps");
    let (dear, cheap) = (corps[0], corps[1]);
    for (c, level) in [(dear, 1.5f32), (cheap, 1.2)] {
        let cc = w.comp_mut::<citysim::Corp>(c).expect("corp");
        let k = *cc.price_level.keys().next().expect("a niche");
        cc.price_level.insert(k, level);
    }
    for (i, x) in w.districts.iter_mut().enumerate() {
        x.street_unrest = if i == d { 0.9 } else { 0.0 };
        x.last_strike = None;
    }
    classes::strike(&mut w);
    let e = w.events.iter().find(|e| e.kind == EventKind::Strike).expect("a Strike");
    assert_eq!(e.actors[0], dear, "the highest-priced employer");
    assert!(e.text.contains(&w.districts[d].name), "{}", e.text);
    let struck: Vec<EntityId> = w.districts[d]
        .residents
        .iter()
        .copied()
        .filter(|&a| w.comp::<citysim::Job>(a).is_some_and(|j| j.struck_shift.is_some()))
        .collect();
    assert!(!struck.is_empty());
    for a in struck {
        let employer = w.comp::<citysim::Job>(a).and_then(|j| j.employer).and_then(|e| w.corp_of_building(e));
        assert_eq!(employer, Some(dear), "only the dear corp's workers walk out");
    }
    assert_eq!(w.districts[d].last_strike, Some(w.tick));
    // The district's cooldown holds the next midnight.
    let n = count(&w, EventKind::Strike);
    classes::strike(&mut w);
    assert_eq!(count(&w, EventKind::Strike), n);
}

// ---------------------------------------------------------------------------
// M12 fix pass (phase 1 and 2 reviews)
// ---------------------------------------------------------------------------

/// Item 6: a gang-owned Home weighs as an owned Home (1.0); held as well it
/// is 2x a held Home, not 4x.
#[test]
fn test_gang_owned_home_weighs_as_a_home() {
    let mut w = v1_three();
    w.run_ticks(1);
    let g = w.gangs()[0];
    let d = DistrictId(1);
    let h = w.district(d).homes[0];
    let base =
        |w: &World| districts::presence(w, d).iter().find(|(c, _)| *c == Controller::Gang(g)).map_or(0.0, |&(_, v)| v);
    let before = base(&w);
    w.comp_mut::<Building>(h).expect("b").owner = Some(g);
    assert!((base(&w) - before - 1.0).abs() < 1e-5, "owned: +1, not +3");
    w.comp_mut::<Building>(h).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD });
    assert!((base(&w) - before - 2.0).abs() < 1e-5, "owned and held: +2");
}

/// Item 22: a broke owner, corp or not, does not pay to clean its doors.
#[test]
fn test_broke_corp_skips_owner_cleaning() {
    let mut w = v1_three();
    w.config.litter = Config::load().litter;
    let corp = w.spawn();
    w.insert(corp, citysim::Corp::new("Broke".into(), Default::default(), 0, None));
    w.comp_mut::<citysim::Corp>(corp).expect("c").order = citysim::CorpOrder::Secure;
    let h = w.district(DistrictId(0)).homes[0];
    w.comp_mut::<Building>(h).expect("b").owner = Some(corp);
    let outside = {
        let b = w.comp::<Building>(h).expect("b");
        w.outside_door(b)
    };
    citysim::systems::litter::deposit(&mut w, outside, 100, 0);
    let dirt = citysim::systems::litter::at(&w, outside);
    assert!(dirt > 0);
    districts::owner_cleaning(&mut w);
    assert_eq!(citysim::systems::litter::at(&w, outside), dirt, "a broke Secure corp leaves its door dirty");
    assert_eq!(w.comp::<citysim::Corp>(corp).expect("c").treasury, 0, "and pays nothing");
    w.comp_mut::<citysim::Corp>(corp).expect("c").treasury = 100;
    districts::owner_cleaning(&mut w);
    assert!(citysim::systems::litter::at(&w, outside) < dirt, "with coins it cleans");
}

/// Item 3: a district with residents but no adults (here: nobody with a
/// Brain) reads crime rate 0, not crimes over its population.
#[test]
fn test_crime_rate_zero_without_adults() {
    let mut w = v1_three();
    w.run_ticks(1);
    for d in &mut w.districts {
        d.crimes = (0..7).map(|_| 5).collect();
    }
    let in_d2: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| {
            let at = match w.comp::<Household>(a).and_then(|h| h.home) {
                Some(h) => w.comp::<Building>(h).expect("home").door,
                None => w.comp::<Position>(a).expect("pos").tile,
            };
            w.district_of(at) == DistrictId(2)
        })
        .collect();
    assert!(!in_d2.is_empty());
    for a in in_d2 {
        w.remove::<Brain>(a);
    }
    districts::aggregates(&mut w);
    let d2 = w.district(DistrictId(2));
    assert!(d2.population > 0 && d2.adults == 0 && !d2.homes.is_empty());
    assert_eq!(d2.crime_rate, 0.0);
    assert!(w.district(DistrictId(0)).crime_rate > 0.0);
}
