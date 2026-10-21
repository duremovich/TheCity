//! The pre-view pass phase 5 (docs/PREVIEW_PASS.md § 0 P10-P12, § 2 "20k
//! legs"; OPEN-139): a long walk re-plans in segments, an escort walks or
//! takes the van, the map's in-flight lines go with the switch.

use citysim::components::ActionInstance;
use citysim::exec::ExecState;
use citysim::goap::{ActionKind, LocationKey, Plan};
use citysim::systems::lod;
use citysim::{
    Brain, Building, BuildingKind, Config, EntityId, GoalKind, Lod, Position, TilePos, World, TICKS_PER_DAY,
};

mod common;

/// The shipped 2k city with the El off (every long leg a walk) and the
/// street's cab roll off (no cab), `[legs]` as `edit` leaves it.
fn city(edit: impl FnOnce(&mut Config)) -> World {
    common::world_with(|cfg| {
        cfg.transit.enabled = false;
        cfg.jobs2.p_cab = 0.0;
        if let Some(l) = cfg.legs.as_mut() {
            l.fares_el_missed = false;
        }
        edit(cfg);
    })
}

fn exec_tick(w: &mut World) {
    citysim::exec::run(w);
    w.tick += 1;
}

/// An adult with a Brain and a Home, and the building farthest from his Home.
fn walker_and_far_target(w: &World) -> (EntityId, EntityId, EntityId) {
    let who = w
        .citizens()
        .into_iter()
        .find(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(w, a)
                && w.comp::<citysim::Household>(a).and_then(|h| h.home).is_some()
        })
        .expect("a housed adult");
    let home = w.comp::<citysim::Household>(who).and_then(|h| h.home).expect("home");
    let hd = w.comp::<Building>(home).expect("home").door;
    let far = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .chain(w.buildings_of_kind(BuildingKind::Bar).iter().copied())
        .max_by_key(|&b| (w.comp::<Building>(b).map_or(0, |bd| bd.door.manhattan(hd)), b))
        .expect("a Market or Bar");
    (who, home, far)
}

fn set_out(w: &mut World, who: EntityId, home: EntityId, target: EntityId, lod_: Lod) {
    w.tick = TICKS_PER_DAY + 8 * 60;
    lod::set_lod(w, who, lod_);
    w.leave_building(who);
    w.enter_building(who, home);
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
}

/// The legs a walk was cut into: each segment's (start tile, end tile), read
/// off the exec while the body walks to `target`, and whether it got there.
fn walk_segments(w: &mut World, who: EntityId, target: EntityId, cap: u64) -> (Vec<(TilePos, TilePos)>, bool, u32) {
    let mut legs: Vec<(TilePos, TilePos)> = Vec::new();
    let mut moves = 0u32;
    let mut last = w.comp::<Position>(who).map(|p| p.tile);
    for _ in 0..cap {
        exec_tick(w);
        let pos = w.comp::<Position>(who).cloned().expect("pos");
        if pos.building == Some(target) {
            return (legs, true, moves);
        }
        if Some(pos.tile) != last {
            moves += 1;
            last = Some(pos.tile);
        }
        let leg = match w.comp::<Brain>(who).map(|b| &b.exec) {
            Some(ExecState::Goto { target: t, .. } | ExecState::GotoTimed { target: t, .. })
                if t.dest == LocationKey::Street =>
            {
                Some(t.tile)
            }
            _ => None,
        };
        if let Some(end) = leg {
            if legs.last().is_none_or(|&(_, e)| e != end) {
                legs.push((citysim::exec::walk_origin(w, who).unwrap_or(pos.tile), end));
            }
        }
    }
    (legs, false, moves)
}

/// OPEN-139: a Full walk past `leg_replan_tiles` walks in segments, none
/// longer than a segment (plus the snap), each end re-planning the rest;
/// off (`[legs]` none) the same walk is one leg. Fails on the base (no
/// segments: one leg to the door).
#[test]
fn test_a_long_full_walk_replans_in_segments() {
    let mut w = city(|_| {});
    let cfg = w.config.legs.clone().expect("the shipped config has [legs]");
    let (who, home, far) = walker_and_far_target(&w);
    let d = w.comp::<Building>(home).expect("home").door.manhattan(w.comp::<Building>(far).expect("far").door);
    assert!(d > 3 * cfg.leg_replan_tiles, "a far target ({d} tiles)");
    set_out(&mut w, who, home, far, Lod::Full);
    let j0 = w.stats.current.jobs2.clone().unwrap_or_default().leg_segments;
    let (legs, arrived, _) = walk_segments(&mut w, who, far, 4 * TICKS_PER_DAY);
    assert!(arrived, "the walk arrives");
    let replans = w.stats.current.jobs2.clone().unwrap_or_default().leg_segments - j0;
    assert!(legs.len() >= 2, "segments: {legs:?}");
    assert_eq!(replans as usize, legs.len(), "every segment's end re-plans");
    let most = cfg.leg_replan_tiles + 2 * cfg.segment_snap;
    for &(a, b) in &legs {
        assert!(a.manhattan(b) <= most, "a segment of {} tiles (at most {most})", a.manhattan(b));
    }
    // Off: one leg, no segment.
    let mut off = city(|c| c.legs = None);
    set_out(&mut off, who, home, far, Lod::Full);
    let (legs, arrived, _) = walk_segments(&mut off, who, far, 4 * TICKS_PER_DAY);
    assert!(arrived && legs.is_empty(), "off, one leg to the door");
}

/// OPEN-139: a Coarse walk past a segment is seen en route: the body moves
/// at each segment's end, where it used to stand at its door for the whole
/// leg and then appear at the end.
#[test]
fn test_a_long_coarse_walk_moves_by_segments() {
    let mut w = city(|_| {});
    let (who, home, far) = walker_and_far_target(&w);
    set_out(&mut w, who, home, far, Lod::Coarse);
    let (legs, arrived, moves) = walk_segments(&mut w, who, far, 4 * TICKS_PER_DAY);
    assert!(arrived, "the walk arrives");
    assert!(legs.len() >= 2 && moves as usize >= legs.len(), "{moves} moves over {} segments", legs.len());
    let mut off = city(|c| c.legs = None);
    set_out(&mut off, who, home, far, Lod::Coarse);
    let (_, arrived, moves) = walk_segments(&mut off, who, far, 4 * TICKS_PER_DAY);
    assert!(arrived && moves == 0, "off, frozen at the door then at the end ({moves} moves)");
}

/// A guard escorting a cuffed suspect from `from` (a street tile).
fn escort_from(w: &mut World, from: TilePos) -> (EntityId, EntityId) {
    let guard = w
        .citizens()
        .into_iter()
        .find(|&a| w.comp::<citysim::components::Job>(a).is_some_and(|j| j.role == citysim::Role::Guard))
        .expect("a guard");
    let suspect = w
        .citizens()
        .into_iter()
        .find(|&a| a != guard && w.has::<Brain>(a) && citysim::systems::demography::is_adult(w, a))
        .expect("a suspect");
    for (a, l) in [(guard, Lod::Full), (suspect, Lod::Full)] {
        lod::set_lod(w, a, l);
        w.leave_building(a);
        w.comp_mut::<Position>(a).expect("pos").tile = from;
    }
    w.comp_mut::<Brain>(suspect).expect("brain").cuffed_by = Some(guard);
    let tick = w.tick;
    let b = w.comp_mut::<Brain>(guard).expect("brain");
    b.escorting = Some(suspect);
    b.plan = Some(Plan {
        goal: GoalKind::Arrest,
        target: Some(suspect),
        steps: vec![ActionInstance { action: ActionKind::Escort, target: Some(suspect), tile: None }],
        started_tick: tick,
    });
    b.plan_step = 0;
    b.exec = ExecState::Idle;
    (guard, suspect)
}

/// A street door `lo..=hi` tiles from `to`.
fn door_between(w: &World, to: TilePos, lo: u32, hi: u32) -> TilePos {
    w.buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter_map(|&h| w.comp::<Building>(h).map(|b| w.outside_door(b)))
        .find(|t| (lo..=hi).contains(&t.manhattan(to)))
        .expect("a door in range")
}

/// P12: an escort within `escort_walk_tiles` walks (a Full guard on foot, at
/// walking speed, the suspect at his side); past it the van comes and
/// drives both at the Car's speed. Neither covers ground faster than its
/// mode. Fails on the base (every escort a timed leg of at most 60 min:
/// 200 tiles in 72 min at 20k).
#[test]
fn test_a_long_escort_uses_the_van_and_a_short_one_walks() {
    let mut w = city(|_| {});
    w.tick = TICKS_PER_DAY + 8 * 60;
    let cfg = w.config.legs.clone().expect("[legs]");
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
    let jd = w.comp::<Building>(jail).expect("jail").door;
    // Short: a walk.
    let near = door_between(&w, jd, cfg.escort_walk_tiles / 2, cfg.escort_walk_tiles);
    let (g, s) = escort_from(&mut w, near);
    exec_tick(&mut w);
    let state = w.comp::<Brain>(g).expect("brain").exec.clone();
    let target = match &state {
        ExecState::Goto { target, .. } => target.clone(),
        other => panic!("a short escort walks, not {other:?}"),
    };
    let to = target.tile;
    let start = w.tick;
    let mut arrived = None;
    for _ in 0..(6 * 60) {
        exec_tick(&mut w);
        let gp = w.comp::<Position>(g).cloned().expect("pos");
        if gp.building.is_none() {
            assert_eq!(w.comp::<Position>(s).map(|p| p.tile), Some(gp.tile), "the suspect at his side");
        }
        if w.comp::<Brain>(g).is_some_and(|b| b.escorting.is_none()) {
            arrived = Some(w.tick - start);
            break;
        }
    }
    let ticks = arrived.expect("the escort ends at the cells");
    let walk = u64::from(near.manhattan(to)) * w.config.exec.move_ticks_full;
    assert!(ticks + 2 >= walk, "walking speed: {ticks} ticks for a {walk}-tick walk");
    // Long: the van.
    let mut w = city(|_| {});
    w.tick = TICKS_PER_DAY + 8 * 60;
    let far = door_between(&w, jd, 3 * cfg.escort_walk_tiles, u32::MAX);
    let (g, _) = escort_from(&mut w, far);
    exec_tick(&mut w);
    let ExecState::GotoTimed { arrive_tick, target, .. } = w.comp::<Brain>(g).expect("brain").exec.clone() else {
        panic!("a long escort is the van's drive");
    };
    let tiles = u64::from(far.manhattan(target.tile));
    let m = citysim::systems::vehicles::timed_mult_of(&w, citysim::components::AssetKind::Car);
    let drive = ((tiles * w.config.exec.move_ticks_full) as f32 * m).round() as u64;
    assert_eq!(arrive_tick - (w.tick - 1), u64::from(cfg.van_call_ticks) + drive, "the call, then the Car's speed");
    let j = w.stats.current.jobs2.clone().unwrap_or_default();
    assert_eq!(j.escort_vans, 1);
    // Off: the old leg capped at `escort_van_ticks`.
    let mut off = city(|c| c.legs = None);
    off.tick = TICKS_PER_DAY + 8 * 60;
    let (g, _) = escort_from(&mut off, far);
    exec_tick(&mut off);
    let ExecState::GotoTimed { arrive_tick, .. } = off.comp::<Brain>(g).expect("brain").exec.clone() else {
        panic!("off, the timed leg");
    };
    assert!(arrive_tick - (off.tick - 1) <= off.config.life.escort_van_ticks);
}

/// P11: a map's `L+` lines (in flight) are the El's with `[legs]` on and
/// dropped with it off; plain `L` lines stay either way.
#[test]
fn test_in_flight_lines_go_with_the_switch() {
    let map = citysim::map::Map::load(std::path::Path::new("../assets/maps/isle_m_k2.txt"));
    let flight = map.lines.iter().filter(|l| l.in_flight).count();
    let plain = map.lines.len() - flight;
    assert!(flight >= 1 && plain >= 2, "the island's export lines and the phase's");
    let mut on = Config::load();
    map.apply_to(&mut on);
    assert_eq!(on.transit.lines.len(), map.lines.len());
    let mut off = Config::load();
    off.legs = None;
    map.apply_to(&mut off);
    assert_eq!(off.transit.lines.len(), plain);
    assert!(off.transit.lines.iter().all(|l| !l.in_flight));
}
