//! Jobs & lives phase 8, the scavs (Dylan's answer 1: a gang type whose
//! business is killing people for their chrome and organs): a scav gang
//! ranks its prey by the value past the Law's cover (the street's poor, not
//! the guarded rich); an abductor dragging its take home holds its
//! GangWork against a meal; a victim in one abductor's hands is not taken
//! by a second. Each test is the shipped seed-42 city and one mechanism,
//! driven directly.

use citysim::components::{Controller, GoalKind, Position};
use citysim::systems::{chrome, gray};
use citysim::{Brain, Config, EntityId, Gang, Job, Kit, Needs, World, TICKS_PER_DAY};

fn city_with_gangs() -> World {
    // R3 (OPEN-216): `[care]` off: its covers and asks move the seed-42 city
    // and the covered/bare pair below stopped standing on the gang's ground
    // (a butterfly; the premise is the scav's ranking, not care).
    let mut cfg = Config::load();
    cfg.care = None;
    let mut w = World::new(42, cfg);
    for _ in 0..30 {
        if w.gangs().iter().any(|&g| w.comp::<Gang>(g).is_some_and(|x| x.members.len() >= 3)) {
            break;
        }
        w.run_ticks(TICKS_PER_DAY);
    }
    w
}

/// Free adults outside any gang (ascending).
fn outsiders(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(w, a)
                && w.gang_of(a).is_none()
                && !w.has::<Job>(a)
                && !w.has::<citysim::Sentence>(a)
        })
        .collect()
}

fn scav_row(w: &World) -> u8 {
    w.config.gray.gang_types.iter().position(|r| r.organ_targets).expect("a row with organ targets") as u8
}

/// A scav gang's prey: a chromed adult standing in a district at full
/// cover (0.95) loses to a poorer one in a district at none; an untyped
/// gang still takes the most valuable.
#[test]
fn test_scav_prey_ranked_past_cover() {
    let mut w = city_with_gangs();
    let g = w.gangs()[0];
    for d in w.districts.iter_mut() {
        d.control = Controller::Contested;
    }
    let pool = outsiders(&w);
    let (rich, poor) = (pool[0], pool[1]);
    // Two districts with Homes: one covered, one bare.
    let ds: Vec<usize> = (0..w.districts.len()).filter(|&i| !w.districts[i].homes.is_empty()).collect();
    let (covered, bare) = (ds[0], ds[1]);
    w.districts[covered].coverage = 3.0;
    w.districts[bare].coverage = 0.0;
    let tile_in = |w: &World, d: usize| {
        let h = w.districts[d].homes[0];
        w.comp::<citysim::Building>(h).map(|b| b.door).expect("door")
    };
    for (a, d) in [(rich, covered), (poor, bare)] {
        let t = tile_in(&w, d);
        w.remove_from_building(a);
        if let Some(p) = w.comp_mut::<Position>(a) {
            p.tile = t;
            p.building = None;
        }
    }
    let kit = |v: i64| Kit { chrome_value: v, visible: 3, ..Kit::default() };
    w.insert(rich, kit(5_000));
    w.insert(poor, kit(4_000));
    w.jobs2.gray.types.remove(&g);
    let untyped = chrome::harvest_target(&w, g).map(|(t, _)| t);
    assert_eq!(untyped, Some(rich), "an untyped gang takes the most valuable");
    let row = scav_row(&w);
    w.jobs2.gray.types.insert(g, row);
    assert!(gray::organ_targets(&w, g));
    let scav = chrome::harvest_target(&w, g).map(|(t, _)| t);
    assert_ne!(scav, Some(rich), "a scav passes the covered rich");
    let t = scav.expect("a scav target");
    let cover = |w: &World, a: EntityId| {
        let tile = w.comp::<Position>(a).expect("pos").tile;
        let d = &w.districts[w.district_of(tile).index()];
        if d.homes.is_empty() {
            1.0
        } else {
            ((d.coverage - 0.5) / 1.5).clamp(0.0, 0.95)
        }
    };
    assert!(cover(&w, t) < cover(&w, rich), "its prey stands under less cover");
}

/// An abductor dragging its take home holds GangWork against its other
/// goals (an emergency still wins: a starving one eats); the same member
/// with no one in tow turns to another goal.
#[test]
fn test_drag_holds_gangwork() {
    let mut w = city_with_gangs();
    let g = w.gangs().into_iter().find(|&g| w.comp::<Gang>(g).is_some_and(|x| x.members.len() >= 3)).expect("a gang");
    // Shadow V4: a member off the clock (on the regenerated table's city the
    // first member was a worker in shift, and Work outscored the drag). The
    // perf pass phase 10: and one the gang's order is not already working
    // (on the I9 city the first such member's order made GangWork win with
    // no one in tow).
    let members = w.comp::<Gang>(g).expect("gang").members.clone();
    let v = outsiders(&w)[0];
    // Rested (an exhausted member's Sleep is an emergency too).
    let rest = |w: &mut World, x: EntityId| {
        w.comp_mut::<Needs>(x).expect("needs").energy = 1.0;
        w.comp_mut::<Needs>(x).expect("needs").hunger = 0.4;
    };
    let mut m = members[0];
    let off_clock: Vec<EntityId> = members.iter().copied().filter(|&x| !w.has::<citysim::Job>(x)).collect();
    for x in off_clock {
        rest(&mut w, x);
        if citysim::utility::think(&w, x).map(|(goal, _)| goal) != Some(GoalKind::GangWork) {
            m = x;
            break;
        }
    }
    rest(&mut w, m);
    let free = citysim::utility::think(&w, m).map(|(goal, _)| goal);
    assert_ne!(free, Some(GoalKind::GangWork), "no one in tow: another goal wins ({free:?})");
    if let Some(b) = w.comp_mut::<Brain>(m) {
        b.escorting = Some(v);
    }
    if let Some(b) = w.comp_mut::<Brain>(v) {
        b.cuffed_by = Some(m);
        b.abducted_by = Some(m);
    }
    assert_eq!(chrome::dragging(&w, m), Some(v));
    let (goal, trace) = citysim::utility::think(&w, m).expect("a goal");
    assert_eq!(goal, GoalKind::GangWork, "the drag holds: {trace:?}");
    w.comp_mut::<Needs>(m).expect("needs").hunger = 0.05;
    // Round 3 S3: the moved city's pick was broke (Earn won: no meal it could
    // obtain); the emergency needs a meal in reach, so it holds a meal's coins.
    w.comp_mut::<citysim::components::Wallet>(m).expect("wallet").coins = 20;
    assert_eq!(citysim::utility::think(&w, m).map(|(g, _)| g), Some(GoalKind::Eat), "starving: the emergency wins");
}

/// A victim already in one abductor's hands is not taken by a second
/// standing beside it.
#[test]
fn test_abduct_refuses_a_victim_in_other_hands() {
    let mut w = city_with_gangs();
    let g = w.gangs().into_iter().find(|&g| w.comp::<Gang>(g).is_some_and(|x| x.members.len() >= 3)).expect("a gang");
    let members = w.comp::<Gang>(g).expect("gang").members.clone();
    let (first, second) = (members[0], members[1]);
    let v = outsiders(&w)[0];
    let t = w.comp::<Position>(v).expect("pos").tile;
    for a in [first, second] {
        w.remove_from_building(a);
        if let Some(p) = w.comp_mut::<Position>(a) {
            p.tile = t;
            p.building = None;
        }
    }
    w.remove_from_building(v);
    if let Some(b) = w.comp_mut::<Brain>(v) {
        b.cuffed_by = Some(first);
        b.abducted_by = Some(first);
    }
    assert!(!chrome::abduct_within(&mut w, second, v, 1), "in the first's hands");
    assert_eq!(w.comp::<Brain>(v).and_then(|b| b.abducted_by), Some(first));
}

/// Phase-8 review finding 9: the abduction-race guard holds with `[gray]`
/// off too (it is general abduction code; on d2eea01 it ran only under the
/// gray market and the second abductor took the victim).
#[test]
fn test_abduct_guard_with_gray_off() {
    let mut c = Config::load();
    c.gray.enabled = false;
    let mut w = World::new(42, c);
    for _ in 0..30 {
        if w.gangs().iter().any(|&g| w.comp::<Gang>(g).is_some_and(|x| x.members.len() >= 3)) {
            break;
        }
        w.run_ticks(TICKS_PER_DAY);
    }
    assert!(!gray::on(&w));
    let g = w.gangs().into_iter().find(|&g| w.comp::<Gang>(g).is_some_and(|x| x.members.len() >= 3)).expect("a gang");
    let members = w.comp::<Gang>(g).expect("gang").members.clone();
    let (first, second) = (members[0], members[1]);
    let v = outsiders(&w)[0];
    let t = w.comp::<Position>(v).expect("pos").tile;
    for a in [first, second] {
        w.remove_from_building(a);
        if let Some(p) = w.comp_mut::<Position>(a) {
            p.tile = t;
            p.building = None;
        }
    }
    w.remove_from_building(v);
    if let Some(b) = w.comp_mut::<Brain>(v) {
        b.cuffed_by = Some(first);
        b.abducted_by = Some(first);
    }
    assert!(citysim::systems::law::near(&w, second, v, 1), "within reach");
    // The second would win the fight: the guard, not the dice, refuses.
    if let Some(sk) = w.comp_mut::<citysim::components::Skills>(second) {
        sk.fighting = 1.0;
    }
    if let Some(sk) = w.comp_mut::<citysim::components::Skills>(v) {
        sk.fighting = 0.0;
    }
    for _ in 0..20 {
        assert!(!chrome::abduct_within(&mut w, second, v, 1), "in the first's hands");
        assert_eq!(w.comp::<Brain>(v).and_then(|b| b.abducted_by), Some(first));
        assert!(citysim::systems::law::living(&w, v), "untouched");
        w.tick += 1;
    }
}
