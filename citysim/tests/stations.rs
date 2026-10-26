//! Lives at work phase 7 (docs/LIVES_AT_WORK.md § 5.2, § 5.3, § 9.1): the
//! district Stations and the execs' Spire homes. Unit tests on the 2k
//! seed-42 city with `station_min_tiles = 1` set in-test (the shipped 2k map
//! gets none: its farthest Lot is under the 240 tiles; the 20k island is the
//! device, and `core_20k` prints the bullets). Each failed on the base
//! `1810392` (no `BuildingKind::Station`).
mod common;

use citysim::components::{BuildingKind, Crime, Household, Job, Personality, Position, Role};
use citysim::systems::{law, life, stations};
use citysim::time::TICKS_PER_DAY;
use citysim::{Building, Config, EntityId, EventKind, Sentence, World};

/// The 2k city with a Station in every district (each at least a tile from
/// the Precinct and the others).
fn station_world() -> World {
    common::world_with(|c| c.law.station_min_tiles = 1)
}

fn door(w: &World, b: EntityId) -> citysim::components::TilePos {
    w.comp::<Building>(b).expect("a building").door
}

/// The Station farthest from the Precinct.
fn far_station(w: &World) -> EntityId {
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    let jd = door(w, jail);
    stations::stations(w).into_iter().max_by_key(|&s| (door(w, s).manhattan(jd), s)).expect("a Station")
}

/// A free, unjailed adult with no job, put on `tile` outdoors.
fn stand_at(w: &mut World, id: EntityId, tile: citysim::components::TilePos) {
    w.abort_plan(id);
    w.leave_building(id);
    if let Some(p) = w.comp_mut::<Position>(id) {
        p.tile = tile;
        p.building = None;
    }
}

/// A city guard and a suspect with an open Theft report beside each other
/// at `tile`; the suspect is broke and meek (the cuffs hold, no fine).
fn guard_and_thief(w: &mut World, tile: citysim::components::TilePos) -> (EntityId, EntityId) {
    let guard =
        citysim::systems::law_brain::guards(w).into_iter().find(|&g| !w.has::<Sentence>(g)).expect("a free city guard");
    let thief = common::free_adults(w).into_iter().find(|&a| !w.has::<Job>(a) && a != guard).expect("a free adult");
    stand_at(w, guard, tile);
    stand_at(w, thief, tile);
    if let Some(p) = w.comp_mut::<Personality>(thief) {
        p.courage = 0.0;
    }
    common::set_coins(w, thief, 0);
    law::file_report(w, Crime::Theft, thief, None);
    (guard, thief)
}

/// § 5.2: the 2k placement is printed; the shipped 2k map gets none, every
/// district one at `station_min_tiles = 1` where the spacing allows.
#[test]
fn test_stations_seeded_by_distance() {
    let w = common::world();
    assert!(stations::on(&w), "the shipped config has stations on");
    let n0 = stations::stations(&w).len();
    let w1 = station_world();
    let n1 = stations::stations(&w1).len();
    println!("FINDING stations at seed, 2k seed 42: {n0} at station_min_tiles 240, {n1} at 1 (8 districts)");
    assert_eq!(n0, 0, "the 2k map's farthest Lot is under 240 tiles");
    assert!(n1 >= 4, "a Station in most districts at min 1: {n1}");
    let opened = w1.events.iter().filter(|e| e.kind == EventKind::Founded && e.text.contains("Station")).count();
    assert_eq!(opened, n1, "a Founded event per Station");
    // The seeded city guards are posted: some at a Station, the rest at the Precinct.
    let posted = citysim::systems::law_brain::guards(&w1)
        .into_iter()
        .filter(|&g| stations::station_post(&w1, g).is_some())
        .count();
    assert!(posted > 0, "seeded guards posted to Stations");
}

/// § 9.1: the escort takes the nearest Station: a suspect cuffed at a
/// Station's door far from the Precinct is booked there.
#[test]
fn test_escort_takes_the_nearest_station() {
    let mut w = station_world();
    let s = far_station(&w);
    let tile = door(&w, s);
    let (guard, thief) = guard_and_thief(&mut w, tile);
    assert!(law::arrest(&mut w, guard, thief), "the cuffs hold");
    assert_eq!(law::jail_for(&w, guard), Some(s), "the escort's target is the nearest Station");
    // The escort arrives (the walk's end): inside the Station with the suspect.
    w.enter_building(guard, s);
    law::follow_guard(&mut w, guard);
    let before = w.stats.current.jobs2.as_ref().map_or(0, |j| j.station_bookings);
    law::jail_suspect(&mut w, guard, thief);
    assert!(w.has::<Sentence>(thief), "sentenced");
    assert_eq!(w.comp::<Position>(thief).and_then(|p| p.building), Some(s), "held in the Station's cells");
    let after = w.stats.current.jobs2.as_ref().map_or(0, |j| j.station_bookings);
    assert_eq!(after, before + 1, "a Station booking counted");
}

/// § 9.1: a full Station sends the escort on: with its cells full the
/// target is the next nearest post (another Station or the Precinct).
#[test]
fn test_full_station_sends_the_escort_on() {
    let mut w = station_world();
    let s = far_station(&w);
    let cells = w.config.law.station_cells as usize;
    let until = w.tick + 10 * TICKS_PER_DAY;
    let fill: Vec<EntityId> = common::free_adults(&w).into_iter().filter(|&a| !w.has::<Job>(a)).take(cells).collect();
    assert_eq!(fill.len(), cells);
    for &a in &fill {
        law::sentence(&mut w, a, Crime::Theft, until, s);
    }
    assert!(!stations::has_cell(&w, s), "the cells are full");
    let tile = door(&w, s);
    let (guard, thief) = guard_and_thief(&mut w, tile);
    let full_before = w.stats.current.jobs2.as_ref().map_or(0, |j| j.stations_full);
    assert!(law::arrest(&mut w, guard, thief));
    let target = law::jail_for(&w, guard).expect("a target");
    assert_ne!(target, s, "the full Station is passed over");
    let kind = w.comp::<Building>(target).map(|b| b.kind);
    assert!(matches!(kind, Some(BuildingKind::Station | BuildingKind::Jail)), "the next post: {kind:?}");
    let full_after = w.stats.current.jobs2.as_ref().map_or(0, |j| j.stations_full);
    assert_eq!(full_after, full_before + 1, "the escort sent on is counted");
}

/// § 9.1: a guard is hired to the Station nearest his Home (under its
/// share): `reconcile_guards` with one vacancy and one candidate living in a
/// Station's district.
#[test]
fn test_guard_hired_to_the_station_nearest_home() {
    let mut w = station_world();
    let s = far_station(&w);
    let d = w.district_of_building(s);
    // The Station's guards leave (its share is unmet).
    for g in citysim::systems::law_brain::guards(&w) {
        if stations::station_post(&w, g) == Some(s) {
            w.vacate_job(g);
        }
    }
    w.vacancies.clear();
    // One candidate: a jobless, lawful adult housed in the Station's district.
    let cand = common::jobless_housed(&w)
        .into_iter()
        .filter(|&a| citysim::systems::careers::age_ok(&w, a, Role::Guard))
        .find(|&a| w.comp::<Household>(a).and_then(|h| h.home).is_some_and(|h| w.district_of_building(h) == d))
        .expect("a jobless adult in the Station's district");
    for a in w.citizens() {
        if let Some(p) = w.comp_mut::<Personality>(a) {
            p.lawfulness = if a == cand { 1.0 } else { p.lawfulness.min(0.1) };
        }
    }
    let have = citysim::systems::law_brain::guards(&w).len() as u16;
    w.levers.guard_count = have + 1 - w.outside.state_guards.min(have);
    law::reconcile_guards(&mut w);
    let job = w.comp::<Job>(cand).expect("hired");
    assert_eq!(job.role, Role::Guard);
    assert_eq!(job.employer, Some(s), "posted at the Station of his district");
    assert_eq!(law::jail_for(&w, cand), Some(s), "his post is the Jail key's building");
}

/// § 5.2: a night's sentence is served at the Station; a longer one waits
/// there for the van and is at the Precinct after `transfer_hour`, its
/// `Sentence` unchanged.
#[test]
fn test_van_transfers_the_long_sentence() {
    let mut w = station_world();
    let s = far_station(&w);
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    // Start just after the day's van so both are booked before the next.
    let van = w.config.law.transfer_hour as u64;
    let day = w.tick - w.tick % TICKS_PER_DAY;
    let start = day + van + 1 + if w.tick > day + van { TICKS_PER_DAY } else { 0 };
    w.run_ticks(start - w.tick);
    let picks: Vec<EntityId> = common::free_adults(&w).into_iter().filter(|&a| !w.has::<Job>(a)).take(2).collect();
    let (night, long) = (picks[0], picks[1]);
    let night_until = w.tick + w.config.law.vagrancy_sentence_ticks;
    let long_until = w.tick + 4 * TICKS_PER_DAY;
    law::sentence(&mut w, night, Crime::Vagrancy, night_until, s);
    law::sentence(&mut w, long, Crime::Theft, long_until, s);
    // Past the next van and its drive (the island pass phase 7: the van
    // rides the road to the Precinct, ~2 h from the far Station here).
    w.run_ticks(TICKS_PER_DAY + 6 * 60);
    assert_eq!(w.comp::<Sentence>(long).map(|x| x.until_tick), Some(long_until), "the Sentence from booking stands");
    assert_eq!(w.comp::<Position>(long).and_then(|p| p.building), Some(jail), "the van took the long sentence");
    assert!(
        w.comp::<Position>(night).and_then(|p| p.building) != Some(jail),
        "the night was served at the Station, never transferred"
    );
    assert!(
        w.events.iter().any(|e| e.kind == EventKind::Transferred && e.actors.first() == Some(&long)),
        "a Transferred event"
    );
    assert!(!w.events.iter().any(|e| e.kind == EventKind::Transferred && e.actors.first() == Some(&night)));
    let released = w.events.iter().any(|e| e.kind == EventKind::Release && e.actors.first() == Some(&night));
    assert!(released, "the night's sentence ended");
}

/// § 5.3: a seeded exec lives at `exec_home_tier` or better when the Spire
/// has room; with none free, an exec housed low stays put and nobody is
/// evicted; a bed freed lets him move up at the next pass. The perf pass
/// phase 10 (ISLAND_PASS I9, `[life] exec_home_reach`): into the free Home
/// of the tier nearest his corp's Hq, not the emptiest.
#[test]
fn test_exec_lives_in_the_spire_without_evictions() {
    let mut w = common::world();
    let tier = life::exec_home_tier(&w);
    assert_eq!(tier, 2, "the shipped config's exec tier");
    let (execs, housed, beds, free, want) = life::spire_supply(&w);
    println!("FINDING spire homes at seed, 2k seed 42: {housed}/{execs} execs at tier >= {tier}; tier beds {beds}, free {free}, wanted {want} (execs x exec_home_spare)");
    let tier_of = |w: &World, e: EntityId| {
        w.comp::<Household>(e).and_then(|h| h.home).and_then(|h| w.comp::<Building>(h)).map_or(0, |b| b.tier)
    };
    let e = *citysim::systems::classes::exec_set(&w).iter().next().expect("an exec");
    if free > 0 {
        assert!(tier_of(&w, e) >= tier, "an exec at seed lives in the Spire when it has room");
    }
    // Move him to a tier-0 Home with room, then fill every tier-2 Home.
    let low = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .find(|&h| {
            w.comp::<Building>(h)
                .is_some_and(|b| b.tier == 0 && !b.derelict && w.residents_of(h).len() + 2 < usize::from(b.capacity))
        })
        .expect("a Sump Home with room");
    w.set_home(e, Some(low));
    let spire: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| w.comp::<Building>(h).is_some_and(|b| b.tier >= tier))
        .collect();
    let caps: Vec<(EntityId, u16)> = spire.iter().map(|&h| (h, w.comp::<Building>(h).unwrap().capacity)).collect();
    let residents: Vec<Vec<EntityId>> = spire.iter().map(|&h| w.residents_of(h).to_vec()).collect();
    for &h in &spire {
        let n = w.residents_of(h).len() as u16;
        if let Some(b) = w.comp_mut::<Building>(h) {
            b.capacity = n;
        }
    }
    life::spire_homes(&mut w);
    assert_eq!(tier_of(&w, e), 0, "no room: he stays low");
    let after: Vec<Vec<EntityId>> = spire.iter().map(|&h| w.residents_of(h).to_vec()).collect();
    assert_eq!(after, residents, "nobody evicted to make room");
    // A bed freed: he moves up.
    for (h, c) in caps {
        if let Some(b) = w.comp_mut::<Building>(h) {
            b.capacity = c;
        }
    }
    // I9: the free tier Home nearest his Hq, door to door (then the id).
    assert!(w.config.life.exec_home_reach > 0, "the shipped config's reach");
    let hq = life::exec_hq_door(&w, e).expect("his corp's Hq");
    let nearest = spire
        .iter()
        .copied()
        .filter(|&h| {
            w.comp::<Building>(h)
                .is_some_and(|b| !b.demolished && !b.derelict && w.residents_of(h).len() < usize::from(b.capacity))
                && citysim::systems::company::open_to_all(&w, h)
        })
        .min_by_key(|&h| (w.comp::<Building>(h).unwrap().door.manhattan(hq), h))
        .expect("a free Spire Home");
    life::spire_homes(&mut w);
    assert!(tier_of(&w, e) >= tier, "a free Spire bed: he moves up");
    let home = w.comp::<Household>(e).and_then(|h| h.home);
    assert_eq!(home, Some(nearest), "the free Spire Home nearest his Hq");
}

/// The off cities (stations off, Spire homes off, both): three days write
/// nothing of the phase, and the config holds none of its keys.
#[test]
fn test_off_cities_write_nothing() {
    let edits: [fn(&mut Config); 3] = [
        |c| c.strip_stations(),
        |c| {
            c.life.exec_home_tier = 0;
            c.life.exec_home_spare = 0.0;
            c.life.exec_home_reach = 0;
        },
        |c| {
            c.strip_stations();
            c.life.exec_home_tier = 0;
            c.life.exec_home_spare = 0.0;
            c.life.exec_home_reach = 0;
        },
    ];
    // The three cities side by side (the change tier's budget, 2026-10-19).
    let case = |k: usize, edit: fn(&mut Config)| {
        let mut w = common::world_with(|c| {
            c.law.station_min_tiles = 1;
            edit(c);
        });
        let (st, sp) = (k != 1, k != 0);
        w.run_ticks(3 * TICKS_PER_DAY);
        for r in &w.stats.history {
            let j = r.jobs2.as_ref().expect("jobs2 on");
            if st {
                assert_eq!((j.stations, j.station_bookings, j.station_fines, j.station_nights), (0, 0, 0, 0));
                assert_eq!((j.transfers, j.stations_full, j.precinct_full), (0, 0, 0));
            }
            if sp {
                assert_eq!((j.execs, j.execs_spire), (0, 0));
            }
        }
        if st {
            assert!(w.buildings_of_kind(BuildingKind::Station).is_empty(), "case {k}");
            assert!(!w.events.iter().any(|e| e.kind == EventKind::Transferred));
        }
        let text = ron::to_string(&w.config).expect("config");
        if st {
            assert!(!text.contains("station"), "case {k}");
        }
        if sp {
            assert!(!text.contains("exec_home"), "case {k}");
        }
    };
    std::thread::scope(|s| {
        let hs: Vec<_> = edits.into_iter().enumerate().map(|(k, edit)| s.spawn(move || case(k, edit))).collect();
        for h in hs {
            h.join().expect("an off city");
        }
    });
    let on = ron::to_string(&Config::load()).expect("config");
    assert!(on.contains("station_cells") && on.contains("exec_home_tier"), "the on config holds them");
}

/// § 5.3 at 20k (diag, printed): the execs housed at tier 2 or better on
/// days 0, 10, 20 and 30 of seed 42, both switches on against both off (the
/// off city is main's), and the Spire's supply at seed. `PROBE_DAYS` sets
/// the days (30).
#[test]
#[ignore = "tier: diag"]
fn diag_spire_homes_20k() {
    let days: u64 = std::env::var("PROBE_DAYS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    let cfg = |on: bool| {
        let mut c = Config::load().scaled_to(20_000);
        c.world.map = "gen:20000".into();
        c.ambient.flats_per_slot = 50.0;
        if !on {
            c.strip_stations();
            c.life.exec_home_tier = 0;
            c.life.exec_home_spare = 0.0;
        }
        c
    };
    let tier2 = |w: &World| {
        let execs = citysim::systems::classes::exec_set(w);
        let n = execs
            .iter()
            .filter(|&&e| {
                w.comp::<Household>(e)
                    .and_then(|h| h.home)
                    .and_then(|h| w.comp::<Building>(h))
                    .is_some_and(|b| b.tier >= 2)
            })
            .count();
        (n, execs.len())
    };
    type Rows = Vec<(bool, Vec<(u64, usize, usize)>, (usize, usize, usize, usize, usize))>;
    let rows: Rows = std::thread::scope(|s| {
        let hs: Vec<_> = [true, false]
            .into_iter()
            .map(|on| {
                s.spawn(move || {
                    let mut w = World::new(42, cfg(on));
                    let mut probe = w.config.clone();
                    probe.life.exec_home_tier = 2;
                    probe.life.exec_home_spare = 1.5;
                    probe.lives.enabled = true;
                    let saved = std::mem::replace(&mut w.config, probe);
                    let supply = life::spire_supply(&w);
                    w.config = saved;
                    let mut out = Vec::new();
                    for d in 0..=days {
                        if d % 10 == 0 || d == days {
                            let (n, e) = tier2(&w);
                            out.push((d, n, e));
                        }
                        if d < days {
                            w.run_ticks(TICKS_PER_DAY);
                        }
                    }
                    (on, out, supply)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a run")).collect()
    });
    for (on, out, (execs, housed, beds, free, want)) in rows {
        let tag = if on { "on" } else { "off (main)" };
        println!(
            "FINDING spire homes 20k seed 42 {tag}: at seed {housed}/{execs} execs at tier >= 2; tier-2 beds {beds}, free {free}, wanted {want} (execs x 1.5)"
        );
        for (d, n, e) in out {
            println!("FINDING spire homes 20k seed 42 {tag}: day {d}: {n}/{e} execs at tier >= 2");
        }
    }
}

/// Fill `s`'s cells with `n` free adults on a ten-day Theft (each bound for
/// the Precinct by the van).
fn fill_cells(w: &mut World, s: EntityId, n: usize) {
    let until = w.tick + 10 * TICKS_PER_DAY;
    let fill: Vec<EntityId> =
        common::free_adults(w).into_iter().filter(|&a| !w.has::<Job>(a) && !w.has::<Sentence>(a)).take(n).collect();
    assert_eq!(fill.len(), n);
    for a in fill {
        law::sentence(w, a, Crime::Theft, until, s);
    }
}

/// Escort a fresh thief to `s` (the guard inside with him) and book him.
fn book_at(w: &mut World, s: EntityId) -> EntityId {
    let tile = door(w, s);
    let (guard, thief) = guard_and_thief(w, tile);
    assert!(law::arrest(w, guard, thief), "the cuffs hold");
    w.enter_building(guard, s);
    law::follow_guard(w, guard);
    law::jail_suspect(w, guard, thief);
    thief
}

/// Review fix 5: the booking re-checks the Station's cells (they filled
/// while the escort walked): a full Station books to the Precinct, never
/// past `station_cells`. Failed on the base `f795548`.
#[test]
fn test_a_full_station_never_books_past_its_cells() {
    let mut w = station_world();
    let s = far_station(&w);
    let cells = w.config.law.station_cells as usize;
    fill_cells(&mut w, s, cells);
    let thief = book_at(&mut w, s);
    assert!(w.has::<Sentence>(thief), "sentenced");
    assert!(stations::held_at(&w, s) <= cells, "never past the cells: {}", stations::held_at(&w, s));
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    // The island pass phase 7: booked to the Precinct, he rides the van on
    // from the Station's door (no write of the body to the Precinct).
    let v = motion::vehicle_of(&w, thief).expect("on the van to the Precinct");
    assert_eq!(w.van_runs.get(&v).map(|r| r.to), Some(jail), "the van runs to the Precinct");
    assert!(at(&w, thief).manhattan(door(&w, s)) <= 2, "at the Station's door, not the Precinct's");
    for _ in 0..600 {
        law_tick(&mut w);
        if w.comp::<Position>(thief).and_then(|p| p.building) == Some(jail) {
            break;
        }
    }
    assert_eq!(w.comp::<Position>(thief).and_then(|p| p.building), Some(jail), "booked to the Precinct");
}

/// Review fix 5: the Station's prisoners bound for the Precinct (a term
/// past the next van) count against its room: with the Precinct's free
/// cells all spoken for by them, a broke thief booked at another Station
/// is released (`Precinct full, cannot pay`); the van never moves the
/// Precinct past its capacity. Failed on the base `f795548` (sentenced, and
/// the van overfilled the Precinct).
#[test]
fn test_station_prisoners_bound_for_the_precinct_count_and_the_van_respects_it() {
    let mut w = station_world();
    let mut st = stations::stations(&w);
    st.sort();
    let (s, s2) = (st[0], st[1]);
    let cells = w.config.law.station_cells as usize;
    fill_cells(&mut w, s, cells);
    let held = stations::precinct_held(&w);
    w.config.buildings.jail.capacity = u16::try_from(held + cells).expect("capacity");
    let thief = book_at(&mut w, s2);
    assert!(!w.has::<Sentence>(thief), "the Precinct is full: the broke thief is released");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Unpunished && e.actors.first() == Some(&thief)));
    // The van with the Precinct one short of the Station's prisoners.
    w.config.buildings.jail.capacity = u16::try_from(held + cells - 1).expect("capacity");
    let van = w.config.law.transfer_hour as u64;
    let day = w.tick - w.tick % TICKS_PER_DAY;
    w.tick = day + TICKS_PER_DAY + van;
    stations::van(&mut w);
    let cap = usize::from(w.config.buildings.jail.capacity);
    assert!(
        stations::precinct_held(&w) <= cap,
        "the van respects the Precinct: {} > {cap}",
        stations::precinct_held(&w)
    );
    assert_eq!(stations::held_at(&w, s), 1, "the one without a cell waits for the next van");
}

// ---------------------------------------------------------------------------
// The island pass phase 7 (docs/ISLAND_PASS.md I3, OPEN-166): the van. Each
// test failed on the base `0a693b2` in behaviour (the escort a statue then a
// landing in the cells; the 04:00 transfer a write of the body to the
// Precinct). They name the ride by its Debug label so they build on the base.
// ---------------------------------------------------------------------------

use citysim::components::{ActionInstance, Brain, Lod, TileKind, TilePos};
use citysim::goap::{ActionKind, Plan};
use citysim::systems::{lod, motion};

/// One executor tick (no other system runs).
fn exec_tick(w: &mut World) {
    citysim::exec::run(w);
    w.tick += 1;
}

/// One executor tick and the law's (the van's arrival), no tier pass.
fn law_tick(w: &mut World) {
    citysim::exec::run(w);
    law::run(w);
    w.tick += 1;
}

/// Where a body stands, a building read at its door.
fn at(w: &World, id: EntityId) -> TilePos {
    let p = w.comp::<Position>(id).expect("a position");
    p.building.map_or(p.tile, |b| door(w, b))
}

fn ride_of(w: &World, id: EntityId) -> Option<String> {
    motion::vehicle_of(w, id).and_then(|v| w.motions.get(&v)).map(|m| format!("{:?}", m.ride))
}

/// The lowest Road tile (row, then column) whose Manhattan distance to the
/// Precinct's door is within `lo..=hi`.
fn road_from_precinct(w: &World, lo: u32, hi: u32) -> TilePos {
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    let jd = door(w, jail);
    for y in 0..w.map.h() {
        for x in 0..w.map.w() {
            let t = TilePos::new(x as u16, y as u16);
            let d = t.manhattan(jd);
            if (lo..=hi).contains(&d) && w.map.tile_at(t) == TileKind::Road {
                return t;
            }
        }
    }
    panic!("no Road tile {lo}..={hi} from the Precinct");
}

/// A guard (at `lod`) and a meek, broke thief cuffed at `tile`, the guard's
/// plan the Escort.
fn cuffed_at(w: &mut World, tile: TilePos, lod_of_guard: Lod) -> (EntityId, EntityId) {
    let (guard, thief) = guard_and_thief(w, tile);
    lod::set_lod(w, guard, lod_of_guard);
    assert!(law::arrest(w, guard, thief), "the cuffs hold");
    let tick = w.tick;
    let b = w.comp_mut::<Brain>(guard).expect("brain");
    b.plan = Some(Plan {
        goal: citysim::GoalKind::Idle,
        target: Some(thief),
        steps: vec![ActionInstance { action: ActionKind::Escort, target: Some(thief), tile: None }],
        started_tick: tick,
    });
    b.plan_step = 0;
    b.exec = citysim::ExecState::Idle;
    (guard, thief)
}

/// What an escort did, tick by tick, to the booking.
struct Escorted {
    /// The most either body moved in one tick (a building read at its door).
    step_max: u32,
    /// The longest the guard stood on one tile.
    stand_max: u64,
    /// The suspect was inside the cells before the guard reached the door.
    early: bool,
    /// Ticks the suspect was aboard a van, and aboard at the guard's tile.
    van_ticks: u64,
    with_guard: bool,
}

fn escort(w: &mut World, guard: EntityId, thief: EntityId, max: u64) -> Escorted {
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    let jd = door(w, jail);
    let mut out = Escorted { step_max: 0, stand_max: 0, early: false, van_ticks: 0, with_guard: true };
    let (mut g0, mut t0) = (at(w, guard), at(w, thief));
    let mut since = w.tick;
    for _ in 0..max {
        exec_tick(w);
        w.check_indices().expect("the aboard links agree");
        let (g, t) = (at(w, guard), at(w, thief));
        out.step_max = out.step_max.max(g.manhattan(g0)).max(t.manhattan(t0));
        if g != g0 {
            since = w.tick;
        }
        out.stand_max = out.stand_max.max(w.tick - since);
        if ride_of(w, thief).as_deref() == Some("Van") {
            out.van_ticks += 1;
            out.with_guard &= w.comp::<Position>(thief).map(|p| p.tile) == w.comp::<Position>(guard).map(|p| p.tile);
        }
        let in_cells = w.comp::<Position>(thief).and_then(|p| p.building) == Some(jail);
        if in_cells && g0.manhattan(jd) > 4 {
            out.early = true;
        }
        (g0, t0) = (g, t);
        if w.has::<Sentence>(thief) {
            return out;
        }
    }
    panic!("the escort never booked him in {max} ticks");
}

/// I3, the arrest hand-off past `escort_walk_tiles`: the van pulls up, the
/// guard drives it with the cuffed suspect aboard (his tile the guard's),
/// and both reach the cells' door on the road: no tick moves either body
/// more than the van's pace, the guard never stands longer than the van's
/// call, and the suspect is never written into the cells before the guard
/// is at the door. A save mid-ride resumes byte for byte. Base: both stood
/// on the arrest tile for the whole leg, then landed in the Precinct.
#[test]
fn test_the_arrest_hand_off_rides_the_van_to_the_cells() {
    let mut w = common::world();
    let call = w.config.legs.as_ref().expect("[legs]").van_call_ticks;
    let tile = road_from_precinct(&w, 100, 140);
    let (guard, thief) = cuffed_at(&mut w, tile, Lod::Full);
    exec_tick(&mut w);
    assert_eq!(ride_of(&w, guard).as_deref(), Some("Van"), "the guard is at the van's wheel");
    assert_eq!(ride_of(&w, thief).as_deref(), Some("Van"), "the suspect is aboard in cuffs");
    let v = motion::vehicle_of(&w, guard).expect("the van");
    assert_eq!(w.motions.get(&v).and_then(|m| m.driver), Some(guard), "he drives");
    // Mid-ride: the save round-trips and the loaded world runs on the same.
    for _ in 0..u64::from(call) + 10 {
        exec_tick(&mut w);
    }
    assert_eq!(motion::vehicle_of(&w, thief), Some(v), "still aboard mid-ride");
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text, "the save round-trips mid-ride");
    let e = escort(&mut w, guard, thief, 2000);
    let _ = escort(&mut back, guard, thief, 2000);
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w), "identical at the booking");
    assert!(e.van_ticks > 20, "a real drive aboard ({} ticks)", e.van_ticks);
    assert!(e.with_guard, "aboard, the suspect is where the guard is");
    assert!(e.step_max <= 4, "no jump: the most a body moved in a tick was {}", e.step_max);
    assert!(!e.early, "the suspect was never written into the cells ahead of the guard");
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    assert_eq!(w.comp::<Position>(thief).and_then(|p| p.building), Some(jail), "booked at the Precinct");
    assert!(
        !w.motions.contains_key(&v) && motion::vehicle_of(&w, thief).is_none(),
        "the van's motion ended at the cells"
    );
    let _ = call;
}

/// I3, the escort is no statue: a Coarse guard within `escort_walk_tiles`
/// leads the cuffed suspect along the road at the walk's pace (no stand of
/// more than a few ticks, no landing). Base: a timed walk, both bodies on
/// the arrest tile for the whole leg, then in the cells.
#[test]
fn test_a_coarse_escort_walks_the_suspect_in() {
    let mut w = common::world();
    let walk = w.config.legs.as_ref().expect("[legs]").escort_walk_tiles;
    let tile = road_from_precinct(&w, walk / 2, walk - 2);
    let (guard, thief) = cuffed_at(&mut w, tile, Lod::Coarse);
    exec_tick(&mut w);
    assert_eq!(ride_of(&w, thief).as_deref(), Some("Foot"), "led on foot");
    let e = escort(&mut w, guard, thief, 600);
    assert!(e.stand_max <= 4, "the escort is no statue: the guard stood {} ticks", e.stand_max);
    assert!(e.step_max <= 2, "no jump: {}", e.step_max);
    assert!(!e.early, "never written into the cells ahead of the guard");
}

/// Put the clock at the next van (`transfer_hour`) and send it.
fn send_the_van(w: &mut World) {
    let van = w.config.law.transfer_hour as u64;
    let day = w.tick - w.tick % TICKS_PER_DAY;
    w.tick = day + TICKS_PER_DAY + van;
    stations::van(w);
}

/// Three long-sentenced prisoners in the far Station's cells: a body, a
/// body demoted mid-ride, and a Statistical one promoted mid-ride.
fn three_held(w: &mut World) -> (EntityId, [EntityId; 3]) {
    let s = far_station(w);
    let until = w.tick + 10 * TICKS_PER_DAY;
    let picks: Vec<EntityId> =
        common::free_adults(w).into_iter().filter(|&a| !w.has::<Job>(a) && !w.has::<Sentence>(a)).take(3).collect();
    for &a in &picks {
        law::sentence(w, a, Crime::Theft, until, s);
    }
    lod::set_lod(w, picks[0], Lod::Coarse);
    lod::set_lod(w, picks[1], Lod::Full);
    lod::set_lod(w, picks[2], Lod::Statistical);
    (s, [picks[0], picks[1], picks[2]])
}

/// I3, the 04:00 transfer is a van run: the Station's prisoners board its
/// van at the door and ride the road to the Precinct, the body's position
/// the van's every tick, no tick moving him more than the van's pace, and
/// in the Precinct only after the drive. LOD: a demotion mid-ride takes the
/// body off and the run carries him on bodiless; a Statistical prisoner
/// promoted mid-ride gets his body aboard; all three are booked in at the
/// van's arrival (the mirror's duration). Base: all three written to the
/// Precinct at 04:00.
#[test]
fn test_the_transfer_is_a_van_run_with_no_jump() {
    let mut w = station_world();
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    let (s, [body, demoted, promoted]) = three_held(&mut w);
    let start_tile = at(&w, body);
    send_the_van(&mut w);
    let start = w.tick;
    assert_eq!(ride_of(&w, body).as_deref(), Some("Van"), "aboard the van");
    assert_eq!(ride_of(&w, demoted).as_deref(), Some("Van"), "aboard the van");
    assert_eq!(ride_of(&w, promoted), None, "a Statistical prisoner has no body aboard");
    for p in [body, demoted, promoted] {
        let b = w.comp::<Position>(p).and_then(|x| x.building);
        assert!(b != Some(s) && b != Some(jail), "out of the Station, not yet at the Precinct: {b:?}");
    }
    let v = motion::vehicle_of(&w, body).expect("the van");
    let way_start = w.motions.get(&v).expect("in motion").pos;
    assert!(way_start.manhattan(start_tile) <= 2, "the van leaves from the Station's door");
    let jd = door(&w, jail);
    let mut prev = at(&w, body);
    let mut step_max = 0u32;
    let mut aboard_ticks = 0u64;
    let mut last_aboard = prev;
    let mut swapped = None;
    let mut arrived = None;
    for _ in 0..600 {
        law_tick(&mut w);
        w.check_indices().expect("the aboard links agree");
        let now = at(&w, body);
        step_max = step_max.max(now.manhattan(prev));
        prev = now;
        if let Some(vv) = motion::vehicle_of(&w, body) {
            assert_eq!(
                w.comp::<Position>(body).map(|p| p.tile),
                w.motions.get(&vv).map(|m| m.pos),
                "at the van's tile"
            );
            aboard_ticks += 1;
            last_aboard = now;
        }
        if swapped.is_some_and(|t| w.tick > t) {
            assert_eq!(ride_of(&w, demoted), None, "demoted: off the van");
        }
        if swapped.is_none() && aboard_ticks >= 5 {
            lod::set_lod(&mut w, demoted, Lod::Statistical);
            lod::set_lod(&mut w, promoted, Lod::Coarse);
            swapped = Some(w.tick);
            assert_eq!(ride_of(&w, promoted).as_deref(), Some("Van"), "promoted: his body is aboard");
        }
        let inside = |w: &World, p: EntityId| w.comp::<Position>(p).and_then(|x| x.building) == Some(jail);
        if inside(&w, body) {
            for p in [demoted, promoted] {
                assert!(inside(&w, p), "every prisoner on the run is booked in at the van's arrival");
            }
            arrived = Some(w.tick);
            break;
        }
        assert!(!inside(&w, demoted) && !inside(&w, promoted), "nobody on the run arrives ahead of the van");
    }
    let arrived = arrived.expect("the van reached the Precinct");
    assert!(swapped.is_some(), "the tier changes ran mid-ride");
    let tiles = start_tile.manhattan(jd);
    assert!(arrived - start >= u64::from(tiles) / 4, "the real drive: {} ticks for {tiles} tiles", arrived - start);
    assert!(step_max <= 4, "no jump: {step_max} tiles in a tick");
    assert!(last_aboard.manhattan(jd) <= 4, "set down at the Precinct's door ({last_aboard} vs {jd})");
    assert!(!w.motions.contains_key(&v) && w.aboard.is_empty(), "the van's run ended");
    for p in [body, demoted, promoted] {
        assert!(w.has::<Sentence>(p), "the Sentence stands");
    }
}
