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
    // Past the next van.
    w.run_ticks(TICKS_PER_DAY);
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
/// evicted; a bed freed lets him move up at the next pass.
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
    life::spire_homes(&mut w);
    assert!(tier_of(&w, e) >= tier, "a free Spire bed: he moves up");
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
        },
        |c| {
            c.strip_stations();
            c.life.exec_home_tier = 0;
            c.life.exec_home_spare = 0.0;
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
