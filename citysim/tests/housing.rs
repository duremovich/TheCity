//! The housing churn (the pre-view pass phase 1, docs/PREVIEW_PASS.md § 2,
//! OPEN-140; Shadow V6 § Before the camera row 5). Unit tests in the shipped
//! seed-42 city with the preconditions set by hand (each showed the bug on
//! the base `823890d` city; the switch retired at the island pass phase 0,
//! OPEN-174). The device rows are `housing_churn_rows` (print only).
mod common;

use citysim::components::{Brain, Building, BuildingKind, Household, Job, Position, Role, TilePos};
use citysim::systems::{ages, classes, demography, law, life};
use citysim::{Config, EntityId, EventKind, LocationKey, World, TICKS_PER_DAY, TICKS_PER_HOUR};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn base() -> World {
    static BASE: OnceLock<World> = OnceLock::new();
    BASE.get_or_init(|| {
        // The island pass phase 6: the scenes are pinned on the El's timed
        // ride (`[motion.trains]` off). With trains the 34-hour city
        // diverges and the table's transfer edge counts half a headway
        // more, so the hand-picked far and near Homes land on the other
        // side of the thresholds these scenes are built on.
        let mut cfg = Config::load();
        // The pressure pass S6 (OPEN-224): the seeded children off (they crowd the Homes this scene's room rule reads).
        cfg.ages.seed_child_slots = 0;
        if let Some(m) = cfg.motion.as_mut() {
            m.trains = None;
        }
        // The pressure pass R2: and the v1 pace (`common::v1_pace`; at 1 a
        // tile the farm worker's household and the bought Block move).
        common::v1_pace(&mut cfg);
        let mut w = World::new(42, cfg);
        w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
        w
    })
    .clone()
}

fn home_of(w: &World, a: EntityId) -> Option<EntityId> {
    w.comp::<Household>(a).and_then(|h| h.home)
}

fn door_of(w: &World, b: EntityId) -> TilePos {
    w.comp::<Building>(b).expect("a building").door
}

fn work_door(w: &World, a: EntityId) -> Option<TilePos> {
    w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.comp::<Building>(e)).map(|b| b.door)
}

/// The plain Home with room for `need` (not `a`'s) whose door is farthest
/// from `to` in commute ticks.
fn far_home(w: &World, a: EntityId, to: TilePos, need: usize) -> EntityId {
    w.buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| Some(h) != home_of(w, a))
        .filter(|&h| {
            w.comp::<Building>(h).is_some_and(|b| {
                !b.demolished && !b.derelict && b.tier <= 1 && w.residents_of(h).len() + need <= usize::from(b.capacity)
            })
        })
        .map(|h| (life::commute_cost(w, door_of(w, h), to), h))
        .max()
        .expect("a far Home with room")
        .1
}

fn housed_lines(w: &World, a: EntityId) -> Vec<String> {
    w.events
        .iter()
        .filter(|e| e.tick == w.tick && e.kind == EventKind::Housed && e.actors.first() == Some(&a))
        .map(|e| e.text.clone())
        .collect()
}

/// A plain worker: a clerk's Job, no spouse, no gang, not an exec.
fn plain_worker(w: &World) -> EntityId {
    let execs = classes::exec_set(w);
    common::adults(w)
        .into_iter()
        .filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.employer.is_some() && j.role == Role::Clerk))
        .find(|&a| {
            w.spouse_of(a).is_none()
                && w.gang_of(a).is_none()
                && !execs.contains(&a)
                && home_of(w, a).is_some()
                && w.has::<Brain>(a)
                && work_door(w, a).is_some()
        })
        .expect("a plain worker")
}

/// Bug 1 (corp bug 3: `ages.rs:478`, `life.rs:1359` against `life.rs:959`):
/// a body a midnight move already housed is left by the "nearer work"
/// relocation: one `Housed` line, the first move kept (the base moved him
/// again in the same minute).
#[test]
fn test_one_housed_writer_per_midnight() {
    let run = |mut w: World| {
        let a = plain_worker(&w);
        let work = work_door(&w, a).unwrap();
        let cap = w.config.life.commute_move_ticks;
        let h = far_home(&w, a, work, 1);
        assert!(life::commute_cost(&w, door_of(&w, h), work) > cap, "the far Home is past the move threshold");
        common::set_coins(&mut w, a, 10_000);
        ages::move_in(&mut w, a, h);
        life::relocate_up_to(&mut w, usize::MAX);
        (home_of(&w, a) == Some(h), housed_lines(&w, a))
    };
    let (kept, lines) = run(base());
    assert!(kept, "the first move is kept: {lines:?}");
    assert_eq!(lines.len(), 1, "one Housed line: {lines:?}");
}

/// Bug 2 (crime bug 3, street bug 4): "moved nearer work" is the employed
/// mover's line and lands nearer his work; the jobless spouse who comes
/// along "moved with" him (the base's spouse line read "nearer work").
#[test]
fn test_nearer_work_is_the_workers_line() {
    let run = |mut w: World| {
        let execs = classes::exec_set(&w);
        let (a, s) = common::adults(&w)
            .into_iter()
            .filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.employer.is_some()) && !execs.contains(&a))
            .filter(|&a| w.gang_of(a).is_none() && work_door(&w, a).is_some())
            .find_map(|a| {
                let s = w.spouse_of(a)?;
                (home_of(&w, s) == home_of(&w, a)
                    && !w.has::<Job>(s)
                    && !execs.contains(&s)
                    && w.has::<Brain>(s)
                    && law::living(&w, s))
                .then_some((a, s))
            })
            .expect("a worker with a jobless spouse at home");
        let work = work_door(&w, a).unwrap();
        let h = far_home(&w, a, work, 2);
        for m in [a, s] {
            w.set_home(m, Some(h));
        }
        common::set_coins(&mut w, a, 10_000);
        let before = life::commute_cost(&w, door_of(&w, h), work);
        life::relocate_up_to(&mut w, usize::MAX);
        let to = home_of(&w, a).expect("housed");
        let after = life::commute_cost(&w, door_of(&w, to), work);
        (before, after, to != h, housed_lines(&w, a), housed_lines(&w, s))
    };
    let (before, after, moved, la, ls) = run(base());
    assert!(moved, "the far worker moves: {la:?}");
    assert!(la.len() == 1 && la[0].contains("nearer work"), "the worker's line: {la:?}");
    assert!(ls.len() == 1 && ls[0].contains("moved with"), "the spouse's line: {ls:?}");
    assert!(after < before, "nearer his work: {before} -> {after} ticks");
}

/// Bug 4 (care bug 1): a body moved in walks to its new Home: an idle
/// adult told at 10:00 that his Home is 60-150 tiles off enters it the
/// same day (the base idled where he stood, `idle_near`: Wander, and slept
/// in whatever bed was nearer).
#[test]
fn test_moved_in_body_walks_home() {
    let run = |mut w: World| {
        let execs = classes::exec_set(&w);
        let a = common::adults(&w)
            .into_iter()
            .filter(|&a| !w.has::<Job>(a) && !execs.contains(&a) && w.gang_of(a).is_none())
            .filter(|&a| w.spouse_of(a).is_none() && home_of(&w, a).is_some() && w.has::<Brain>(a))
            .find(|&a| w.comp::<Position>(a).is_some_and(|p| p.building.is_some() && p.building == home_of(&w, a)))
            .expect("a jobless adult at home");
        let from = w.comp::<Position>(a).unwrap().tile;
        let h = w
            .buildings_of_kind(BuildingKind::Home)
            .iter()
            .copied()
            .filter(|&h| {
                w.comp::<Building>(h).is_some_and(|b| {
                    let t = b.door.manhattan(from);
                    (60..150).contains(&t) && !b.demolished && w.residents_of(h).len() < usize::from(b.capacity)
                })
            })
            .min()
            .expect("a Home 60-150 tiles off");
        // He stands outside his old door, awake, with the day ahead.
        w.remove_from_building(a);
        if let Some(n) = w.comp_mut::<citysim::components::Needs>(a) {
            n.energy = 1.0;
        }
        w.abort_plan(a);
        ages::move_in(&mut w, a, h);
        let end = w.tick + 14 * TICKS_PER_HOUR;
        let mut entered = false;
        while w.tick < end && !entered {
            w.comp_mut::<Brain>(a).expect("brain").pinned = true;
            w.run_ticks(1);
            entered = w.comp::<Position>(a).is_some_and(|p| p.building == Some(h));
        }
        entered
    };
    assert!(run(base()), "the body walks into its new Home the same day");
}

/// Bug 5 (gangs bug 5, street bug 5): a housed sleeper's "Hotel" is a
/// Hotel (`life::away_hotel`, the planner's), never a Mission's cot (the
/// homeless's nearest bed, by design E27; the base resolved a housed
/// body's "Hotel" at a cot house's door to the Soup Kitchen).
#[test]
fn test_housed_hotel_is_a_hotel() {
    let run = |mut w: World| {
        let m = *w.buildings_of_kind(BuildingKind::Mission).first().expect("a Mission");
        let price = w.config.charity.cot_price;
        if let Some(c) = w.comp_mut::<Building>(m).and_then(|b| b.charity.as_mut()) {
            c.purse = c.purse.max(price * 10);
        }
        assert!(citysim::systems::charity::is_cot_house(&w, m), "a cot house");
        let at = door_of(&w, m);
        let a = common::adults(&w)
            .into_iter()
            .filter(|&a| w.gang_of(a).is_none() && w.has::<Brain>(a))
            .find(|&a| home_of(&w, a).is_some_and(|h| door_of(&w, h).manhattan(at) > 100))
            .expect("a housed adult far from the cot house");
        w.remove_from_building(a);
        if let Some(p) = w.comp_mut::<Position>(a) {
            p.tile = at;
        }
        common::set_coins(&mut w, a, 500);
        let got = w.resolve_building(a, LocationKey::Hotel, None);
        let kind = got.and_then(|b| w.comp::<Building>(b)).map(|b| b.kind);
        (kind, got == life::away_hotel(&w, a))
    };
    let (kind, same) = run(base());
    assert!(kind.is_none_or(|k| k == BuildingKind::Hotel), "a Hotel or none: {kind:?}");
    assert!(same, "the planner's Hotel");
}

/// The island pass phase 1 (OPEN-163, ISLAND_PASS I1): a homeless worker
/// standing at a far Home's door, with two Homes of equal offer free (that
/// one and the one nearest his work). Base (and `[distance]` off): the
/// nearest door to where he stands. On: the one near work (the far one is
/// past `home_cap_ticks`). The far one alone still takes him (none is left).
#[test]
fn test_homeless_worker_moves_in_near_work() {
    let mut w = base();
    w.config.rent.rehouse_wait_days = 0;
    let a = plain_worker(&w);
    let work = work_door(&w, a).expect("a post");
    let plain: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| w.comp::<Building>(h).is_some_and(|b| !b.demolished && !b.derelict && b.tier <= 1))
        .collect();
    let by = |h: EntityId| (citysim::systems::distance::travel(&w, door_of(&w, h), work), h);
    let near = plain.iter().map(|&h| by(h)).min().expect("Homes").1;
    let far = plain.iter().map(|&h| by(h)).max().expect("Homes").1;
    let (t_near, t_far) = (by(near).0, by(far).0);
    assert!(t_far > t_near + 2, "{t_near} vs {t_far}");
    w.config.distance.as_mut().expect("[distance] on").home_cap_ticks = (t_near + t_far) / 2;
    w.set_home(a, None);
    if let Some(h) = w.comp_mut::<Household>(a) {
        h.homeless_since = None;
        h.evicted_by = None;
        h.arrears = 0;
    }
    w.remove_from_building(a);
    let stand = door_of(&w, far);
    if let Some(p) = w.comp_mut::<Position>(a) {
        p.tile = stand;
    }
    common::set_coins(&mut w, a, 10_000);
    // Every Home full at its own head count but the two (or the far one).
    let rooms = |w: &mut World, open: &[EntityId]| {
        for h in w.buildings_of_kind(BuildingKind::Home).to_vec() {
            let n = w.residents_of(h).len() as u16;
            w.comp_mut::<Building>(h).expect("b").capacity = n + if open.contains(&h) { 2 } else { 0 };
        }
    };
    let midnight = |mut x: World| {
        x.tick = (x.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
        citysim::systems::ownership::run(&mut x);
        home_of(&x, a)
    };
    // The other homeless cannot pay (the two beds are his to take).
    for b in common::adults(&w) {
        if b != a && home_of(&w, b).is_none() {
            common::set_coins(&mut w, b, 0);
        }
    }
    rooms(&mut w, &[near, far]);
    assert_eq!(midnight(w.clone()), Some(near), "on: the Home near his work");
    let mut off = w.clone();
    off.config.distance = None;
    assert_eq!(midnight(off), Some(far), "off (the base): the nearest door to where he stands");
    rooms(&mut w, &[far]);
    assert_eq!(midnight(w), Some(far), "the only Home, past the cap, still takes him");
}

/// The island pass phase 2 (OPEN-163, OPEN-171; ISLAND_PASS I1), in the
/// seed-42 city with a homeless adult at hand. The spot: of two corners of
/// equal offer the near one; a corner past `spot_cap_ticks` alone is no
/// spot on (the base walked to it: the nearest when none is within). The
/// bed out: with a Hotel at the door and a cot house farther by more than
/// `cot_bed_ticks`, "Hotel" is the Hotel (the base took the cot first and
/// "Hotel" resolved to the Mission); at the cot house's door the cot is
/// walked to as `Cot`, and "Hotel" never resolves to a Mission.
#[test]
fn test_free_time_spot_and_bed_out_by_distance() {
    use citysim::living::{Spot, SpotKind};
    use citysim::systems::{distance, leisure, lod, plan, street};
    let mut w = base();
    let f = w.config.distance.as_ref().and_then(|d| d.free.clone()).expect("[distance.free] on");
    let a = common::adults(&w)
        .into_iter()
        .find(|&a| w.gang_of(a).is_none() && w.has::<Brain>(a) && !law::is_guard(&w, a) && !w.has::<Job>(a))
        .expect("a jobless adult");
    let nbrs: Vec<EntityId> = w.neighbours(a).collect();
    for c in nbrs {
        w.edge_entry(a, c).affinity = 0.0;
    }
    w.set_home(a, None);
    w.remove_from_building(a);
    // The bed out: a Hotel with a door, a cot house in reach but far by travel.
    let reach = w.config.street.hotel_reach.max(w.config.life.hotel_reach_homeless);
    let hotels: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Hotel)
        .iter()
        .copied()
        .filter(|&h| street::is_hotel(&w, h) && street::free_beds(&w, h) > 0)
        .collect();
    let missions: Vec<EntityId> = w.buildings_of_kind(BuildingKind::Mission).to_vec();
    let out = |w: &World, b: EntityId| w.outside_door(w.comp::<Building>(b).expect("b"));
    let (hotel, cot) = hotels
        .iter()
        .flat_map(|&h| missions.iter().map(move |&m| (h, m)))
        .find(|&(h, m)| {
            let (o, d) = (out(&w, h), door_of(&w, m));
            o.manhattan(d) <= reach
                && distance::travel(&w, o, d) > distance::travel(&w, o, door_of(&w, h)) + f.cot_bed_ticks + 10
        })
        .expect("a Hotel and a Mission in reach, far by travel");
    let price = w.config.charity.cot_price;
    if let Some(c) = w.comp_mut::<Building>(cot).and_then(|b| b.charity.as_mut()) {
        c.purse = c.purse.max(price * 10);
    }
    assert!(citysim::systems::charity::is_cot_house(&w, cot), "a cot house");
    // Nothing else in reach takes him: the other cot houses shut by purse.
    for &m in &missions {
        if m != cot {
            if let Some(c) = w.comp_mut::<Building>(m).and_then(|b| b.charity.as_mut()) {
                c.purse = 0;
            }
        }
    }
    common::set_coins(&mut w, a, 500);
    let stand = |w: &mut World, t: TilePos| {
        if let Some(p) = w.comp_mut::<Position>(a) {
            p.tile = t;
            p.building = None;
        }
    };
    let at_hotel = out(&w, hotel);
    stand(&mut w, at_hotel);
    let mut off = w.clone();
    off.config.distance.as_mut().expect("[distance]").free = None;
    let kind = |w: &World, b: Option<EntityId>| b.and_then(|b| w.comp::<Building>(b)).map(|b| b.kind);
    assert_eq!(street::hotel_for(&w, a), Some(hotel), "on: the Hotel at the door over the far cot");
    assert_eq!(
        kind(&w, w.resolve_building(a, LocationKey::Hotel, None)),
        Some(BuildingKind::Hotel),
        "a Hotel is a Hotel"
    );
    assert_eq!(street::hotel_for(&off, a), Some(cot), "off (the base): any cot in reach first");
    assert_eq!(
        kind(&off, off.resolve_building(a, LocationKey::Hotel, None)),
        Some(BuildingKind::Mission),
        "off (the base): \"Hotel\" is the Mission (OPEN-171)"
    );
    // At the cot house's door: the cot, walked to as a cot.
    let at_cot = out(&w, cot);
    stand(&mut w, at_cot);
    assert_eq!(street::hotel_for(&w, a), Some(cot), "on: the cot at the door");
    assert_eq!(w.resolve_building(a, LocationKey::Cot, None), Some(cot));
    assert!(
        kind(&w, w.resolve_building(a, LocationKey::Hotel, None)) != Some(BuildingKind::Mission),
        "\"Hotel\" never resolves to a Mission"
    );
    let sleep_plan = |w: &mut World| -> Vec<citysim::ActionKind> {
        lod::set_lod(w, a, citysim::Lod::Full);
        w.comp_mut::<citysim::Needs>(a).expect("needs").energy = 0.3;
        w.comp_mut::<Brain>(a).expect("brain").current_goal = Some(citysim::GoalKind::Sleep);
        plan::plan_for(w, a, citysim::GoalKind::Sleep);
        w.comp::<Brain>(a).and_then(|b| b.plan.clone()).expect("a plan").steps.iter().map(|s| s.action).collect()
    };
    let mut off = w.clone();
    off.config.distance.as_mut().expect("[distance]").free = None;
    let steps = sleep_plan(&mut off);
    assert!(steps.contains(&citysim::ActionKind::GoTo(LocationKey::Hotel)), "off: the cot as \"Hotel\": {steps:?}");
    let steps = sleep_plan(&mut w);
    assert!(steps.contains(&citysim::ActionKind::GoTo(LocationKey::Cot)), "the cot's walk: {steps:?}");
    assert!(!steps.contains(&citysim::ActionKind::GoTo(LocationKey::Hotel)), "no \"Hotel\": {steps:?}");
    // The spot: two street corners of equal offer, one near, one past the cap.
    w.comp_mut::<Brain>(a).expect("brain").plan = None;
    let from = at_hotel;
    stand(&mut w, from);
    let streets: Vec<TilePos> = (0..w.map.h() as u16)
        .flat_map(|y| (0..w.map.w() as u16).map(move |x| TilePos { x, y }))
        .filter(|&t| w.is_street(t))
        .collect();
    let near = *streets.iter().find(|t| (3..=6).contains(&t.manhattan(from))).expect("a near corner");
    let cap_tiles = w.config.life.spot_walk_cap_tiles;
    let far = *streets
        .iter()
        .find(|&&t| t.manhattan(from) > cap_tiles && distance::travel(&w, from, t) > f.spot_cap_ticks)
        .expect("a far corner");
    let both = vec![Spot { tile: far, kind: SpotKind::Barrel }, Spot { tile: near, kind: SpotKind::Barrel }];
    w.spots = vec![both; w.districts.len()];
    assert_eq!(leisure::best_spot(&w, a, from).map(|s| s.0), Some(near), "the near corner");
    w.spots = vec![vec![Spot { tile: far, kind: SpotKind::Barrel }]; w.districts.len()];
    let mut off = w.clone();
    off.config.distance.as_mut().expect("[distance]").free = None;
    assert_eq!(leisure::best_spot(&w, a, from), None, "on: a corner past the cap alone is no spot");
    assert_eq!(leisure::best_spot(&off, a, from).map(|s| s.0), Some(far), "off (the base): walked to anyway");
}

/// The island pass phase 3a (ISLAND_PASS I6, OPEN-163): a company block
/// for the Farm `farm` in the seed-42 city, the plain Home with room
/// nearest its door, emptied of nobody (its room set to its head count +
/// `room`). Returns the block.
fn company_block(w: &mut World, farm: EntityId, room: u16) -> EntityId {
    let fd = door_of(w, farm);
    let block = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| w.comp::<Building>(h).is_some_and(|b| !b.demolished && !b.derelict))
        .map(|h| (citysim::systems::distance::travel(w, door_of(w, h), fd), h))
        .min()
        .expect("a Home")
        .1;
    let n = w.residents_of(block).len() as u16;
    w.comp_mut::<Building>(block).expect("b").capacity = n + room;
    w.company_homes.insert(block, farm);
    block
}

/// A Farm's worker with no spouse, housed, not an exec: `(worker, farm)`.
fn farm_worker(w: &World) -> (EntityId, EntityId) {
    let execs = classes::exec_set(w);
    common::adults(w)
        .into_iter()
        .filter_map(|a| {
            let e = w.comp::<Job>(a)?.employer?;
            (w.comp::<Building>(e)?.kind == BuildingKind::Farm).then_some((a, e))
        })
        .find(|&(a, _)| {
            w.spouse_of(a).is_none()
                && w.gang_of(a).is_none()
                && !execs.contains(&a)
                && home_of(w, a).is_some()
                && w.has::<Brain>(a)
        })
        .expect("a Farm worker")
}

fn midnight(w: &mut World) {
    w.tick = (w.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    citysim::systems::ownership::run(w);
}

/// The island pass phase 3a (I6): a Farm hand hired today with a Home past
/// `far_ticks` moves into the Farm's company block at midnight (the base
/// has no company bed: he stays far). One hired long ago does not (that
/// move is phase 4's), nor does a far worker of another workplace, and the
/// city's rehouse never puts a homeless outsider in a company bed.
#[test]
fn test_far_hire_takes_a_company_bed_near_the_farm() {
    let mut w = base();
    let (a, farm) = farm_worker(&w);
    let fd = door_of(&w, farm);
    let block = company_block(&mut w, farm, 2);
    let far = far_home(&w, a, fd, 1);
    w.set_home(a, Some(far));
    let cfg = w.config.company_housing.clone().expect("[company_housing] on");
    assert!(citysim::systems::distance::travel(&w, door_of(&w, far), fd) > cfg.far_ticks, "his Home is far");
    let now = w.tick;
    w.comp_mut::<Job>(a).expect("job").hired_tick = now;
    let mut on = w.clone();
    midnight(&mut on);
    // The island pass phase 4: the far hire moves on the commute pressure's
    // path (`hire_now`), later the same midnight.
    life::relocate(&mut on);
    assert_eq!(home_of(&on, a), Some(block), "on: the hire takes the company bed");
    assert!(
        on.events
            .iter()
            .any(|e| e.kind == EventKind::Housed && e.actors.first() == Some(&a) && e.text.contains("company housing")),
        "a Housed line naming the company housing"
    );
    let mut off = w.clone();
    off.config.company_housing = None;
    off.company_homes.clear();
    midnight(&mut off);
    assert_eq!(home_of(&off, a), Some(far), "off (the base): no company bed, he stays far");
    // Hired long ago: phase 4's move, not this one.
    let mut old = w.clone();
    old.comp_mut::<Job>(a).expect("job").hired_tick = 0;
    midnight(&mut old);
    assert_eq!(home_of(&old, a), Some(far), "a long-held post's far Home stays (phase 4)");
    // Reserved: a homeless outsider with coins is not rehoused into it.
    let mut res = w.clone();
    res.config.rent.rehouse_wait_days = 0;
    res.company_homes.insert(block, farm);
    let b = common::adults(&res)
        .into_iter()
        .find(|&b| {
            b != a
                && res.spouse_of(b).is_none()
                && !res.has::<Job>(b)
                && res.has::<Brain>(b)
                && home_of(&res, b).is_some()
        })
        .expect("an outsider");
    res.comp_mut::<Job>(a).expect("job").hired_tick = 0;
    res.set_home(b, None);
    if let Some(h) = res.comp_mut::<Household>(b) {
        h.homeless_since = None;
        h.evicted_by = None;
        h.arrears = 0;
    }
    common::set_coins(&mut res, b, 10_000);
    for h in res.buildings_of_kind(BuildingKind::Home).to_vec() {
        if h != block {
            let n = res.residents_of(h).len() as u16;
            res.comp_mut::<Building>(h).expect("b").capacity = n;
        }
    }
    midnight(&mut res);
    assert_ne!(home_of(&res, b), Some(block), "a company bed is the staff's only");
}

/// The island pass phase 3a (I6): a company bed's rent is `rent_mult x
/// [rent] base[tier]` a day per worker, accrued at midnight (his jobless
/// spouse owes nothing) and docked from the wage (`pay_rent_from_income`,
/// even with `[rent] pay_from_income` off) to the block's owner; no coin
/// made or lost.
#[test]
fn test_company_rent_is_docked_from_the_wage_and_conserved() {
    let mut w = base();
    let (a, farm) = farm_worker(&w);
    let block = company_block(&mut w, farm, 2);
    w.set_home(a, Some(block));
    let cfg = w.config.company_housing.clone().expect("[company_housing] on");
    let tier = usize::from(w.comp::<Building>(block).expect("b").tier.min(2));
    let rate = (cfg.rent_mult * w.config.rent.base[tier] as f32 * 10.0).round() as i64;
    assert_eq!(citysim::systems::company::rent_tenths(&w, block), rate);
    if let Some(h) = w.comp_mut::<Household>(a) {
        h.rent_due = 0.0;
        h.arrears = 0;
    }
    common::set_coins(&mut w, a, 500);
    midnight(&mut w);
    let due = w.comp::<Household>(a).expect("h").rent_due;
    assert!((due - rate as f32 / 10.0).abs() < 1e-3, "accrued {due}, the rate {rate} tenths");
    // A plain Block's per-adult split would have charged the other residents
    // (none of them on the owner's staff owes the company rent).
    let owner = w.owner_of(block);
    for &r in w.residents_of(block) {
        let corp_staff =
            w.comp::<Job>(r).and_then(|j| j.employer).is_some_and(|e| owner.is_some() && w.owner_of(e) == owner);
        if r != a && !corp_staff {
            assert_eq!(w.comp::<Household>(r).map(|h| h.rent_due), Some(0.0), "the household's others owe nothing");
        }
    }
    if let Some(h) = w.comp_mut::<Household>(a) {
        h.rent_due = 3.0;
    }
    w.config.rent.pay_from_income = false;
    let (before, purse, id) = (citysim::systems::econ::identity(&w), w.purse(owner), common::coins(&w, a));
    let city = w.purse(None);
    citysim::systems::ownership::pay_rent_from_income(&mut w, a);
    assert_eq!(common::coins(&w, a), id - 3, "three coins docked");
    // The pressure pass F2: the owner's share and the tax its accumulator
    // rolls over (a coin of the three when the carry passes one: the run
    // up to here picks a different block and owner as the city changes).
    assert_eq!(w.purse(owner) + w.purse(None), purse + city + 3, "paid to the block's owner (and the tax)");
    assert_eq!(citysim::systems::econ::identity(&w), before, "no coin made or lost");
    // Off (the base): a plain Block's rent waits for midnight with the
    // switch off.
    let mut off = w.clone();
    off.config.company_housing = None;
    off.company_homes.clear();
    if let Some(h) = off.comp_mut::<Household>(a) {
        h.rent_due = 3.0;
    }
    let c = common::coins(&off, a);
    citysim::systems::ownership::pay_rent_from_income(&mut off, a);
    assert_eq!(common::coins(&off, a), c, "off: nothing docked with pay_from_income off");
}

/// The island pass phase 3a (I6): a company resident who lost the job
/// keeps the bed `grace_days`, then is evicted (an `Evicted` line naming
/// the job), not before; one hired back at the workplace inside the grace
/// stays. Off (the base) no one is put out.
#[test]
fn test_company_eviction_after_the_grace_not_before() {
    let mut w = base();
    let (a, farm) = farm_worker(&w);
    let block = company_block(&mut w, farm, 2);
    w.set_home(a, Some(block));
    common::set_coins(&mut w, a, 10_000);
    let grace = w.config.company_housing.as_ref().expect("on").grace_days;
    let job = w.comp::<Job>(a).cloned().expect("job");
    citysim::systems::economy::dismiss(&mut w, a, Some(farm), "let go".into());
    assert!(!w.has::<Job>(a));
    let mut off = w.clone();
    off.config.company_housing = None;
    // The first midnight notes the job gone; `grace_days` more put him out.
    let mut kept = w.clone();
    for day in 0..grace {
        midnight(&mut w);
        assert_eq!(home_of(&w, a), Some(block), "still housed on grace day {day}");
        if day == 1 {
            kept = w.clone();
        }
    }
    midnight(&mut w);
    assert_eq!(home_of(&w, a), None, "evicted after the grace");
    assert!(
        w.events.iter().any(|e| e.kind == EventKind::Evicted
            && e.actors.first() == Some(&a)
            && e.text.contains("company housing")),
        "an Evicted line naming the company housing"
    );
    // Hired back inside the grace: he stays.
    citysim::systems::demography::hire(&mut kept, a, farm, job.role);
    for _ in 0..=grace {
        midnight(&mut kept);
    }
    assert_eq!(home_of(&kept, a), Some(block), "back on the staff, the bed is his");
    // Off: no eviction for the lost job.
    off.company_homes.clear();
    for _ in 0..=grace {
        midnight(&mut off);
    }
    assert_eq!(home_of(&off, a), Some(block), "off (the base): the Block keeps him");
}

/// The map's tiles, row-major (a building's footprint shows as walls).
fn tiles(w: &World) -> Vec<citysim::components::TileKind> {
    let mut v = Vec::with_capacity(w.map.w() * w.map.h());
    for y in 0..w.map.h() {
        for x in 0..w.map.w() {
            v.push(w.map.tile(x, y));
        }
    }
    v
}

/// The island pass phase 3b (I6, Dylan 2026-10-22): the seeded company
/// beds are floors inside the workplace: each company Home shares its
/// workplace's rect and door, `within` names it, its label is the kind's
/// room name, and no tile or Lot is added (no new footprint). The base (3a,
/// `[company_housing.inside]` off) puts its blocks on a Lot or a plot.
#[test]
fn test_seeded_company_beds_are_floors_inside_the_workplace() {
    let mut w = base();
    w.company_homes.clear();
    w.company_lapsed.clear();
    // Every housed post is far (0 ticks) and no plain bed is that near: each
    // staffed workplace of the kinds wants its staff's beds.
    w.config.company_housing.as_mut().expect("[company_housing] on").far_ticks = 0;
    let inside = w.config.company_housing.as_ref().and_then(|c| c.inside.clone()).expect("[company_housing.inside] on");
    let (lots, before) = (w.buildings_of_kind(BuildingKind::Lot).len(), tiles(&w));
    let mut on = w.clone();
    let (blocks, beds) = citysim::systems::company::seed(&mut on);
    assert!(blocks > 0 && beds > 0, "on: company floors seeded ({blocks}, {beds} beds)");
    for (&h, &wp) in &on.company_homes {
        let (hb, wb) = (on.comp::<Building>(h).expect("home"), on.comp::<Building>(wp).expect("workplace"));
        assert_eq!(hb.kind, BuildingKind::Home);
        assert_eq!(hb.within, Some(wp), "the floors name their workplace");
        assert_eq!((hb.rect, hb.door), (wb.rect, wb.door), "inside the workplace: its rect and door");
        assert_eq!(hb.label.as_deref(), Some(inside.room(wb.kind).0), "the kind's room name");
        assert_eq!(on.owner_of(h), on.owner_of(wp), "the workplace's owner holds them");
    }
    assert!(on.company_homes.keys().any(|&h| !on.residents_of(h).is_empty()), "the far staff moved in");
    assert_eq!(on.buildings_of_kind(BuildingKind::Lot).len(), lots, "no Lot built on or plot spawned");
    assert!(tiles(&on) == before, "no tile changed: no new footprint");
    // Off (the base, 3a): blocks of their own beside the workplace.
    let mut off = w.clone();
    off.config.company_housing.as_mut().expect("on").inside = None;
    let (ob, _) = citysim::systems::company::seed(&mut off);
    assert!(ob > 0, "off: 3a seeds blocks");
    assert!(
        off.company_homes.iter().all(|(&h, &wp)| {
            let hb = off.comp::<Building>(h).expect("home");
            hb.within.is_none() && hb.rect != off.comp::<Building>(wp).expect("w").rect
        }),
        "off: every block stands apart from its workplace"
    );
    assert!(tiles(&off) != before, "off: the blocks' walls went up");
}

/// Phase 3b's buyout setup: the seed-42 city with no company housing yet,
/// a Farm hand `a` (housed, no spouse) and his Farm (a corp's), the buyout
/// pass due tonight (`period_days` 1, `min_want` 1) and the corp funded.
/// Returns `(world, a, farm, corp, h)`, `h` the plain Block the pass will
/// buy: the nearest eligible one to the Farm's door, `a` moved into it.
fn buyout_setup() -> (World, EntityId, EntityId, EntityId, EntityId) {
    let mut w = base();
    w.company_homes.clear();
    w.company_lapsed.clear();
    let (a, farm) = farm_worker(&w);
    let corp = w.owner_of(farm).filter(|&o| w.has::<citysim::components::Corp>(o)).expect("a corp's Farm");
    {
        let ch = w.config.company_housing.as_mut().expect("[company_housing] on");
        ch.far_ticks = 0;
        let b = ch.buyout.as_mut().expect("[company_housing.buyout] on");
        b.period_days = 1;
        b.min_want = 1;
        b.build_fallback = false;
    }
    let buy = w.config.company_housing.as_ref().and_then(|c| c.buyout.clone()).expect("buyout");
    let fd = door_of(&w, farm);
    let h = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| {
            w.comp::<Building>(h).is_some_and(|b| {
                !b.demolished
                    && !b.derelict
                    && b.within.is_none()
                    && b.under_construction_until.is_none()
                    && b.tier <= buy.tier_max
            }) && !citysim::systems::street::is_shanty(&w, h)
        })
        .map(|h| (citysim::systems::distance::travel(&w, door_of(&w, h), fd), h))
        .min()
        .expect("a Block near the Farm")
        .1;
    w.config.company_housing.as_mut().and_then(|c| c.buyout.as_mut()).expect("buyout").cap_ticks =
        citysim::systems::distance::travel(&w, door_of(&w, h), fd);
    // A corp converts its own Block at no price; the tests read a sale.
    if w.owner_of(h) == Some(corp) {
        citysim::systems::ownership::transfer_building(&mut w, h, None);
    }
    let n = w.residents_of(h).len() as u16;
    w.comp_mut::<Building>(h).expect("b").capacity = w.comp::<Building>(h).expect("b").capacity.max(n + 1);
    w.set_home(a, Some(h));
    if let Some(c) = w.comp_mut::<citysim::components::Corp>(corp) {
        c.treasury += 1_000_000;
    }
    w.tick = (w.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    (w, a, farm, corp, h)
}

/// The island pass phase 3b (I6): a corp whose Farm staff commute far buys
/// the nearest plain Block in reach at `price_mult` x its value, paid to
/// its owner (`Flow::Sale`, an `Acquired` line), and converts it to company
/// housing for the Farm (bunks, the company rent); no coin made or lost.
/// Off (the base): no buyout.
#[test]
fn test_buyout_converts_a_nearby_block_and_conserves_coin() {
    let (w, _a, _farm, corp, h) = buyout_setup();
    let (seller, cap) = (w.owner_of(h), w.comp::<Building>(h).expect("b").capacity);
    let mut on = w.clone();
    let (before, purse, paid_to) = (citysim::systems::econ::identity(&on), on.purse(Some(corp)), on.purse(seller));
    citysim::systems::company::buyouts(&mut on);
    // The corp's most-wanting workplace buys (this Farm, or another of its
    // Farms with the same Block nearest).
    let wp = on.company_homes.get(&h).copied().expect("on: the Block is company housing");
    assert_eq!(on.owner_of(wp), Some(corp), "for one of the corp's workplaces");
    let mine = on.company_homes.values().filter(|&&x| on.owner_of(x) == Some(corp)).count();
    assert_eq!(mine, 1, "one buyout a corp a pass");
    assert_eq!(on.owner_of(h), Some(corp), "the corp holds it");
    let hb = on.comp::<Building>(h).expect("b");
    assert!(hb.capacity >= cap, "bunks: never fewer beds ({cap} -> {})", hb.capacity);
    assert_eq!(hb.rent_per_day, citysim::systems::company::rent_tenths(&on, h) / 10, "the company rent");
    assert_ne!(seller, Some(corp), "a sale");
    {
        let price = purse - on.purse(Some(corp));
        assert!(price > 0, "the corp paid");
        assert_eq!(on.purse(seller), paid_to + price, "paid to the owner");
        assert!(
            on.events
                .iter()
                .any(|e| e.kind == EventKind::Acquired && e.actors.contains(&h) && e.text.contains("company housing")),
            "an Acquired line naming the company housing"
        );
    }
    assert_eq!(citysim::systems::econ::identity(&on), before, "no coin made or lost");
    // Off (the base): no buyout.
    let mut off = w.clone();
    off.config.company_housing.as_mut().expect("on").buyout = None;
    citysim::systems::company::buyouts(&mut off);
    assert!(off.company_homes.is_empty(), "off: nothing bought");
    assert_eq!(off.owner_of(h), seller);
}

/// The island pass phase 3b (I6): a buyout's tenant not on the corp's
/// staff gets a `Notice` line, keeps the bed `grace_days` and is evicted
/// the midnight after (an `Evicted` line naming the buyout); the Farm hand
/// living there stays, on the company rent. Off (the base) both stay.
#[test]
fn test_buyout_notice_evicts_the_tenant_after_the_grace_staff_stays() {
    let (mut w, a, _farm, corp, h) = buyout_setup();
    let b = w
        .residents_of(h)
        .iter()
        .copied()
        .find(|&r| {
            r != a
                && w.has::<Brain>(r)
                && demography::is_adult(&w, r)
                && !demography::is_child_of(&w, r, a)
                && !demography::is_child_of(&w, a, r)
                && w.comp::<Job>(r).and_then(|j| j.employer).is_none_or(|e| w.owner_of(e) != Some(corp))
        })
        .expect("a tenant not on the corp's staff");
    common::set_coins(&mut w, b, 10_000);
    common::set_coins(&mut w, a, 10_000);
    let grace = w.config.company_housing.as_ref().expect("on").grace_days;
    let mut off = w.clone();
    citysim::systems::company::buyouts(&mut w);
    assert!(w.company_homes.get(&h).is_some_and(|&x| w.owner_of(x) == Some(corp)), "bought");
    // No second buyout over the grace.
    w.config.company_housing.as_mut().and_then(|c| c.buyout.as_mut()).expect("buyout").period_days = 100_000;
    let noticed =
        |w: &World, x: EntityId| w.events.iter().any(|e| e.kind == EventKind::Notice && e.actors.get(1) == Some(&x));
    assert!(noticed(&w, b), "the tenant is given notice");
    assert!(!noticed(&w, a), "the Farm hand is not");
    // Notice given tonight; out `grace_days` midnights on.
    for day in 1..grace {
        midnight(&mut w);
        assert_eq!(home_of(&w, b), Some(h), "the tenant keeps the bed on grace day {day}");
    }
    midnight(&mut w);
    assert_ne!(home_of(&w, b), Some(h), "evicted after the grace");
    assert!(
        w.events
            .iter()
            .any(|e| e.kind == EventKind::Evicted && e.actors.first() == Some(&b) && e.text.contains("bought out")),
        "an Evicted line naming the buyout"
    );
    assert_eq!(home_of(&w, a), Some(h), "the staff tenant stays");
    assert!(citysim::systems::company::payer(&w, a, h), "on the docked company rent");
    // Off (the base): no buyout, both stay.
    off.config.company_housing.as_mut().expect("on").buyout = None;
    for _ in 0..=grace {
        midnight(&mut off);
    }
    assert_eq!(home_of(&off, b), Some(h), "off: the tenant stays");
    assert!(off.company_homes.is_empty());
}

/// Phase 3c's setup: the seed-42 city with its company beds seeded as
/// floors inside the workplaces (`far_ticks` 0, as the 3b test). Returns
/// `(world, h, wp)`: the lowest-id floors with residents and their workplace.
fn floors_setup() -> (World, EntityId, EntityId) {
    let mut w = base();
    w.company_homes.clear();
    w.company_lapsed.clear();
    w.config.company_housing.as_mut().expect("[company_housing] on").far_ticks = 0;
    citysim::systems::company::seed(&mut w);
    let (h, wp) = w
        .company_homes
        .iter()
        .find(|&(&h, &wp)| !w.residents_of(h).is_empty() && w.comp::<Building>(h).is_some_and(|b| b.within == Some(wp)))
        .map(|(&h, &wp)| (h, wp))
        .expect("seeded floors with residents");
    (w, h, wp)
}

/// The island pass phase 3c (OPEN-177): a resident of a crowded company
/// floor (one storey, 30 to it) is set a lower mood target than the same
/// agent in an uncrowded plain Home of the same tier: crowding, the
/// floor's factor and the company bed. The base (`[mood.housing]` off)
/// sets both the same target.
#[test]
fn test_crowded_company_floor_lowers_the_mood_target() {
    let (mut w, h, _) = floors_setup();
    let tier = w.comp::<Building>(h).expect("floors").tier;
    // Crowd it: one storey, 30 residents (unmarried adults, lowest id first).
    w.comp_mut::<Building>(h).expect("floors").floors = 1;
    w.comp_mut::<Building>(h).expect("floors").capacity = 30;
    let movers: Vec<EntityId> = common::adults(&w)
        .into_iter()
        .filter(|&x| home_of(&w, x) != Some(h) && w.spouse_of(x).is_none())
        .take(30usize.saturating_sub(w.residents_of(h).len()))
        .collect();
    for x in movers {
        w.set_home(x, Some(h));
    }
    assert!(w.residents_of(h).len() >= 30, "the floor is crowded");
    let a = *w.residents_of(h).iter().find(|&&r| w.has::<Brain>(r)).expect("an adult on the floor");
    // The same agent in a plain Home of the tier with room for one more.
    let plain = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .find(|&p| {
            citysim::systems::company::open_to_all(&w, p)
                && w.comp::<Building>(p).is_some_and(|b| {
                    !b.demolished
                        && !b.derelict
                        && b.tier == tier
                        && b.within.is_none()
                        && (w.residents_of(p).len() as u16) < b.capacity
                })
        })
        .expect("a plain Home of the tier with room");
    let mut elsewhere = w.clone();
    elsewhere.set_home(a, Some(plain));
    let target = |w: &mut World| {
        w.tick = (w.tick / TICKS_PER_HOUR + 1) * TICKS_PER_HOUR;
        if let Some(m) = w.comp_mut::<citysim::components::Mood>(a) {
            m.value = 0.0;
        }
        citysim::mood::run(w);
        w.comp::<citysim::components::Mood>(a).expect("mood").value
    };
    let (mut off_floor, mut off_plain) = (w.clone(), elsewhere.clone());
    let (on_floor, on_plain) = (target(&mut w), target(&mut elsewhere));
    assert!(on_floor < on_plain, "on: the crowded company floor sets a lower target ({on_floor} vs {on_plain})");
    let cfg = w.config.mood.housing.clone().expect("[mood.housing] on");
    assert!(
        citysim::mood::housing_bias(&w, a) <= cfg.tier[usize::from(tier.min(2))] - cfg.crowd_w * 3.0 - cfg.company,
        "crowding and the company bed both count"
    );
    off_floor.config.mood.housing = None;
    off_plain.config.mood.housing = None;
    // The term itself is exactly 0 with the switch off; comparing the two
    // worlds' whole targets would also compare the rest of the mood inputs,
    // which drift with the Statistical table.
    let (f, p) = (citysim::mood::housing_bias(&off_floor, a), citysim::mood::housing_bias(&off_plain, a));
    assert!(f == 0.0 && p == 0.0, "off (the base): the Home sets no target ({f} vs {p})");
}

/// The island pass phase 3c (the 3b loose end): company floors inside a
/// workplace that is gone close with it at midnight: the residents are
/// evicted the normal way (an `Evicted` line each), the record is
/// demolished and dropped from the Home index, a `Derelict` line logs it.
/// The base left them standing as a plain Home in the dead workplace.
#[test]
fn test_company_floors_close_with_their_workplace() {
    let (mut w, h, wp) = floors_setup();
    let residents: Vec<EntityId> = w.residents_of(h).to_vec();
    w.comp_mut::<Building>(wp).expect("workplace").demolished = true;
    midnight(&mut w);
    let b = w.comp::<Building>(h).expect("floors");
    assert!(b.demolished && b.occupants.is_empty(), "the floors are demolished with the workplace");
    assert!(!w.buildings_of_kind(BuildingKind::Home).contains(&h), "dropped from the Home index");
    assert!(!w.company_homes.contains_key(&h), "no longer company housing");
    assert!(w.residents_of(h).is_empty(), "nobody lives there");
    for &r in &residents {
        assert_ne!(home_of(&w, r), Some(h), "every resident put out");
    }
    let adult = residents.iter().copied().find(|&r| w.has::<Brain>(r)).expect("an adult resident");
    assert!(
        w.events.iter().any(|e| e.kind == EventKind::Evicted
            && e.actors.first() == Some(&adult)
            && e.text.contains("company floors closed")),
        "an Evicted line for the resident"
    );
    assert!(
        w.events.iter().any(|e| e.kind == EventKind::Derelict && e.actors.first() == Some(&h)),
        "a Derelict line for the floors"
    );
}

/// The island pass phase 4 (I2): `n` midnights of the relocation pass
/// alone (the commute pressure's path); the night `done` first holds.
fn nights(w: &mut World, n: u32, done: impl Fn(&World) -> bool) -> Option<u32> {
    for night in 1..=n {
        w.tick = (w.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
        citysim::systems::commute::job_moves(w);
        life::relocate(w);
        if done(w) {
            return Some(night);
        }
    }
    None
}

/// The plain Home with room nearest `to` (not `a`'s), its room made
/// `need` beds: a free bed near work.
fn near_home(w: &mut World, a: EntityId, to: TilePos, need: usize) -> EntityId {
    let h = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| Some(h) != home_of(w, a) && !w.company_homes.contains_key(&h))
        .filter(|&h| w.comp::<Building>(h).is_some_and(|b| !b.demolished && !b.derelict && b.tier <= 1))
        .map(|h| (life::commute_cost(w, door_of(w, h), to), h))
        .min()
        .expect("a Home")
        .1;
    let n = w.residents_of(h).len() + need;
    w.comp_mut::<Building>(h).expect("b").capacity = n as u16;
    h
}

/// The island pass phase 4 (ISLAND_PASS I2, OPEN-163): a far commuter with
/// a free Home near work moves after `move_days` of pressure, not before
/// (the base relocation moved him the first midnight), and then rests on
/// the cooldown.
#[test]
fn test_far_commuter_moves_after_the_pressure_days() {
    let mut w = base();
    let cfg = w.config.commute_pressure.clone().expect("[commute_pressure] on");
    let a = plain_worker(&w);
    let work = work_door(&w, a).unwrap();
    let far = far_home(&w, a, work, 1);
    w.set_home(a, Some(far));
    assert!(life::commute_cost(&w, door_of(&w, far), work) > cfg.far_ticks, "the far Home is past far_ticks");
    let near = near_home(&mut w, a, work, 1);
    common::set_coins(&mut w, a, 10_000);
    let mut off = w.clone();
    off.config.commute_pressure = None;
    assert_eq!(nights(&mut off, 1, |x| home_of(x, a) != Some(far)), Some(1), "off (the base): the first midnight");
    let night = nights(&mut w, u32::from(cfg.move_days) + 1, |x| home_of(x, a) != Some(far));
    assert_eq!(night, Some(u32::from(cfg.move_days)), "on: after move_days of pressure");
    assert_eq!(home_of(&w, a), Some(near), "into the free Home near work");
    let p = w.commute_pressure.get(&a).copied().expect("the cooldown is kept");
    assert!(p.days == 0 && p.cool > w.day() as u32, "the cooldown runs: {p:?}");
}

/// The island pass phase 4 (I2): with no Home he can take near work, past
/// `job_days` the far commuter takes an open post of his role near his
/// Home (the normal quit and hire), not before; the base never does.
#[test]
fn test_far_commuter_with_no_home_takes_a_nearer_job() {
    let mut w = base();
    let cfg = w.config.commute_pressure.clone().expect("[commute_pressure] on");
    let a = plain_worker(&w);
    let old = w.comp::<Job>(a).and_then(|j| j.employer).unwrap();
    let work = work_door(&w, a).unwrap();
    let far = far_home(&w, a, work, 1);
    w.set_home(a, Some(far));
    // No bed anywhere, and no coin for a first rent or a trade.
    for h in w.buildings_of_kind(BuildingKind::Home).to_vec() {
        let n = w.residents_of(h).len().max(1) as u16;
        w.comp_mut::<Building>(h).expect("b").capacity = n;
    }
    common::set_coins(&mut w, a, 0);
    // A Clerk's post open at the workplace nearest his Home.
    let fd = door_of(&w, far);
    let to = w
        .with::<Building>()
        .into_iter()
        .filter(|&b| b != old)
        .filter(|&b| {
            w.comp::<Building>(b).is_some_and(|bd| {
                !bd.demolished && !bd.derelict && !matches!(bd.kind, BuildingKind::Home | BuildingKind::Lot)
            })
        })
        .map(|b| (citysim::systems::distance::travel(&w, fd, door_of(&w, b)), b))
        .min()
        .expect("a workplace")
        .1;
    w.vacancies.insert(to, vec![Role::Clerk]);
    let d = life::commute_cost(&w, fd, work);
    assert!(
        citysim::systems::distance::travel(&w, fd, door_of(&w, to)) + cfg.job_margin_ticks <= d,
        "the post is nearer"
    );
    let mut off = w.clone();
    off.config.commute_pressure = None;
    let span = u32::from(cfg.job_days + cfg.retry_days) + 1;
    let switched = |x: &World| x.comp::<Job>(a).and_then(|j| j.employer) == Some(to);
    assert_eq!(nights(&mut off, span, switched), None, "off (the base): no nearer job");
    let night = nights(&mut w, span, switched).expect("on: a nearer post");
    assert!(night >= u32::from(cfg.job_days), "not before job_days: night {night}");
    assert_eq!(home_of(&w, a), Some(far), "he stays housed where he was");
    assert!(
        w.events
            .iter()
            .any(|e| e.kind == EventKind::Quit && e.actors.first() == Some(&a) && e.text.contains("far commute")),
        "the normal quit, its reason named"
    );
    assert!(w.vacancies.get(&old).is_some_and(|v| v.contains(&Role::Clerk)), "the old post is open");
}

/// The island pass phase 4 (I2): a household moves together, after the
/// pressure days: the worker and his jobless spouse into one Home near his
/// work, the spouse's line "moved with" him.
#[test]
fn test_household_moves_together_on_the_pressure() {
    let mut w = base();
    let cfg = w.config.commute_pressure.clone().expect("[commute_pressure] on");
    let execs = classes::exec_set(&w);
    let (a, s) = common::adults(&w)
        .into_iter()
        .filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.employer.is_some()) && !execs.contains(&a))
        .filter(|&a| w.gang_of(a).is_none() && work_door(&w, a).is_some())
        .find_map(|a| {
            let s = w.spouse_of(a)?;
            (home_of(&w, s) == home_of(&w, a)
                && !w.has::<Job>(s)
                && !execs.contains(&s)
                && w.has::<Brain>(s)
                && law::living(&w, s))
            .then_some((a, s))
        })
        .expect("a worker with a jobless spouse at home");
    let work = work_door(&w, a).unwrap();
    let kids: Vec<EntityId> = demography::children_of_agent(&w, a)
        .into_iter()
        .filter(|&c| w.has::<citysim::components::Child>(c) && home_of(&w, c) == home_of(&w, a))
        .collect();
    let need = 2 + kids.len();
    let far = far_home(&w, a, work, need);
    for &m in [a, s].iter().chain(kids.iter()) {
        w.set_home(m, Some(far));
    }
    let near = near_home(&mut w, a, work, need);
    // No School rule in the way: the near Home keeps one if the far had it.
    w.config.ages.school_reach = u32::MAX / 4;
    common::set_coins(&mut w, a, 10_000);
    let night = nights(&mut w, u32::from(cfg.move_days) + 1, |x| home_of(x, a) != Some(far));
    assert_eq!(night, Some(u32::from(cfg.move_days)), "after move_days of pressure");
    assert_eq!(home_of(&w, a), Some(near), "the worker near work");
    assert_eq!(home_of(&w, s), Some(near), "the spouse with him");
    for c in kids {
        assert_eq!(home_of(&w, c), Some(near), "the children with them");
    }
    let lines = |m: EntityId| {
        w.events
            .iter()
            .filter(|e| e.kind == EventKind::Housed && e.actors.first() == Some(&m))
            .map(|e| e.text.clone())
            .collect::<Vec<_>>()
    };
    assert!(lines(s).last().is_some_and(|t| t.contains("moved with")), "the spouse's line: {:?}", lines(s));
}

/// The island fix round F2 (OPEN-189): a company bed is its own workplace's
/// staff's, within `[beds] company_post_ticks` of the post. A worker of
/// another of the corp's workplaces past the cap neither takes one (the
/// commute pressure's pick, `commute::company_for`) nor keeps one (the
/// grace, then the eviction); the base let any of the corp's posts take and
/// keep a bed at any distance (a D3 clinic's ripperdoc in a D6 Site
/// Barracks). The retiree's kin take-in goes to a relative's plain Home,
/// never a child's company bed (the base moved `worker_6720` into a D6
/// coffin rack).
#[test]
fn test_company_bed_is_its_own_staffs_within_the_cap() {
    let mut w = base();
    let (a, farm) = farm_worker(&w);
    let block = company_block(&mut w, farm, 4);
    let bd = door_of(&w, block);
    let travel = |w: &World, b: EntityId| citysim::systems::distance::travel(w, bd, door_of(w, b));
    // A worker of another workplace, no spouse, no gang; the block his
    // employer's owner's (the same corp), his post the farthest from it.
    let execs = classes::exec_set(&w);
    let (t, b, e) = common::adults(&w)
        .into_iter()
        .filter(|&b| b != a && w.spouse_of(b).is_none() && w.gang_of(b).is_none() && !execs.contains(&b))
        .filter(|&b| home_of(&w, b).is_some() && w.has::<Brain>(b))
        .filter_map(|b| {
            let e = w.comp::<Job>(b)?.employer?;
            (e != farm && w.owner_of(e).is_some()).then(|| (travel(&w, e), b, e))
        })
        .max()
        .expect("a far worker of another workplace");
    assert!(t > travel(&w, farm), "his post is farther from the block than the Farm");
    let corp = w.owner_of(e);
    citysim::systems::ownership::transfer_building(&mut w, block, corp);
    let cap = t - 1;
    w.config.beds.as_mut().expect("[beds] on").company_post_ticks = cap;
    assert!(travel(&w, farm) <= cap, "the Farm's own post is within the cap");
    let mut off = w.clone();
    off.config.beds = None;
    use citysim::systems::commute::company_for;
    assert!(company_for(&w, a, block), "on: the Farm's own staff takes it");
    assert!(!company_for(&w, b, block), "on: a far corp worker does not take it");
    assert!(company_for(&off, b, block), "off (the base): any of the corp's posts takes it");
    // Kept: the far corp worker is put out after the grace (the base keeps him).
    let grace = w.config.company_housing.as_ref().expect("on").grace_days;
    let keeps = |mut w: World| {
        w.set_home(b, Some(block));
        common::set_coins(&mut w, b, 10_000);
        for _ in 0..=grace {
            midnight(&mut w);
        }
        home_of(&w, b) == Some(block)
    };
    assert!(!keeps(w.clone()), "on: the far corp worker is evicted after the grace");
    assert!(keeps(off.clone()), "off (the base): the corp's bed keeps him at any distance");
    // The kin take-in: an elder whose grown child lives in the company bed.
    let pair = w.citizens().into_iter().find_map(|p| {
        let c = demography::children_of_agent(&w, p).into_iter().find(|&c| {
            demography::is_adult(&w, c)
                && law::living(&w, c)
                && home_of(&w, c).is_some()
                && home_of(&w, c) != home_of(&w, p)
        })?;
        Some((p, c))
    });
    let (p, c) = pair.expect("a parent with a grown child housed elsewhere");
    let kin = |mut w: World| {
        w.set_home(c, Some(block));
        let room = w.residents_of(block).len() as u16 + 2;
        w.comp_mut::<Building>(block).expect("b").capacity = room;
        ages::kin_home(&w, p)
    };
    assert_ne!(kin(w.clone()), Some(block), "on: the take-in never lands in a company bed");
    assert_eq!(kin(off), Some(block), "off (the base): the retiree moves into the child's company bed");
}

/// The island fix round F2 (OPEN-191, D2 = A): tonight's Hideout bed for a
/// housed member (the nearer bed, the watch, lying low) only within `[beds]
/// hideout_walk_ticks` of where the body stands and within
/// `hideout_work_ticks` of the post; past either, Home. A homeless member
/// takes it as before. The base sent a member lying low (or on watch) to
/// the Hideout whatever his work's distance.
#[test]
fn test_far_hideout_sleeps_at_home_homeless_takes_it() {
    let mut w = base();
    // A housed Farm hand with a post, enlisted in the first gang.
    let (m, _) = farm_worker(&w);
    let g0 = w.gangs()[0];
    citysim::systems::gang::enlist(&mut w, m, g0);
    let gang = w.gang_of(m).expect("gang");
    let h = w.hideout_of(gang).expect("a Hideout");
    let hd = door_of(&w, h);
    w.comp_mut::<citysim::components::Gang>(gang).expect("g").order = citysim::components::Order::LieLow;
    w.comp_mut::<Building>(h).expect("b").capacity += 8;
    // Five tiles from the Hideout's door, outside.
    let at = TilePos { x: hd.x.saturating_sub(5), y: hd.y };
    w.remove_from_building(m);
    if let Some(p) = w.comp_mut::<Position>(m) {
        p.tile = at;
        p.building = None;
    }
    let walk = citysim::systems::distance::travel(&w, at, hd);
    let to_work = citysim::systems::distance::travel(&w, hd, work_door(&w, m).expect("work"));
    assert!(walk > 0 && to_work > 0, "a walk to the Hideout and a commute from it");
    let with = |walk_cap: u64, work_cap: u64| {
        let mut w = w.clone();
        let c = w.config.beds.as_mut().expect("[beds] on");
        c.hideout_walk_ticks = walk_cap;
        c.hideout_work_ticks = work_cap;
        w
    };
    use citysim::systems::gang::holes_up_at;
    let mut off = w.clone();
    off.config.beds = None;
    assert_eq!(holes_up_at(&off, m), Some(h), "off (the base): lying low, the Hideout whatever the commute");
    let near = with(walk, to_work);
    assert_eq!(holes_up_at(&near, m), Some(h), "on: within both caps, the Hideout");
    let far_walk = with(walk - 1, to_work);
    assert_eq!(holes_up_at(&far_walk, m), None, "on: past the walk, Home");
    assert_eq!(life::hideout_bed(&far_walk, m), None, "on: past the walk, no Hideout bed");
    let far_work = with(walk, to_work - 1);
    assert_eq!(holes_up_at(&far_work, m), None, "on: past tomorrow's commute, Home");
    assert_eq!(life::hideout_bed(&far_work, m), None, "on: past tomorrow's commute, no Hideout bed");
    let mut homeless = with(0, 0);
    homeless.set_home(m, None);
    assert_eq!(holes_up_at(&homeless, m), Some(h), "on: a homeless member still takes the Hideout");
    assert_eq!(life::hideout_bed(&homeless, m), Some(h), "on: a homeless member's bed is the Hideout");
}

/// The island fix round F2 (OPEN-193): a Builder's far-site day begun away
/// from the Yard walks straight to the site (`[beds] site_from_home`); the
/// base walked to the Yard to load first (`builder_7236`: 473 min walking
/// for 311 min of work).
#[test]
fn test_builders_site_day_begins_at_the_site() {
    let mut w = base();
    let role = w.config.trade_role("builder").expect("the builder trade");
    let reach = w.config.shift.as_ref().and_then(|c| c.haul.as_ref()).map_or(0, |h| h.haul_reach);
    let (a, yard) = w
        .citizens()
        .into_iter()
        .filter(|&a| law::living(&w, a) && !w.has::<citysim::Sentence>(a) && home_of(&w, a).is_some())
        .find_map(|a| {
            let j = w.comp::<Job>(a)?;
            let e = j.employer?;
            let left = j.shift_end(w.tick)?.saturating_sub(w.tick);
            (j.role == role && j.shifts.len() == 1 && j.on_shift(w.tick_of_day()) && left >= 360).then_some((a, e))
        })
        .expect("a Builder on shift with six hours left");
    let yd = door_of(&w, yard);
    let site = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&b| w.comp::<Building>(b).is_some_and(|x| !x.demolished && !x.derelict))
        .map(|b| (citysim::systems::distance::travel(&w, yd, door_of(&w, b)), b))
        .filter(|&(t, b)| (30..=90).contains(&t) && door_of(&w, b).manhattan(yd) > reach)
        .min()
        .map(|(_, b)| b)
        .expect("a Block past haul_reach, under two hours from the Yard");
    let tick = w.tick;
    w.jobs2.sites.retain(|_, &mut (y, _)| y != yard);
    w.jobs2.sites.insert(site, (yard, tick));
    citysim::systems::lod::set_lod(&mut w, a, citysim::Lod::Full);
    w.comp_mut::<Job>(a).expect("job").last_shift_day = None;
    w.comp_mut::<Brain>(a).expect("brain").shift = None;
    // Out at his Home's door.
    let hd = door_of(&w, home_of(&w, a).expect("home"));
    w.remove_from_building(a);
    if let Some(p) = w.comp_mut::<Position>(a) {
        p.tile = hd;
        p.building = None;
    }
    let first = |mut w: World| {
        let p = citysim::systems::shift::plan(&mut w, a).expect("a haul plan");
        (p.steps[0].action, p.steps[0].target)
    };
    let mut off = w.clone();
    off.config.beds = None;
    let (act, to) = first(off);
    assert!(matches!(act, citysim::ActionKind::GoTo(_)) && to == Some(yard), "off (the base): to the Yard first");
    let (act, to) = first(w);
    assert_eq!((act, to), (citysim::ActionKind::GoTo(LocationKey::TargetHome), Some(site)), "on: straight to the site");
}

fn env<T: std::str::FromStr>(k: &str) -> Option<T> {
    std::env::var(k).ok().and_then(|v| v.parse().ok())
}

#[test]
#[ignore = "tier: diag"]
fn housing_churn_rows() {
    let seed: u64 = env("CITYSIM_HD_SEED").unwrap_or(42);
    let days: u64 = env("CITYSIM_HD_DAYS").unwrap_or(30);
    let mut c = Config::load();
    if let Some(pop) = env::<u32>("CITYSIM_HD_POP") {
        c = c.scaled_to(pop);
    }
    if let Ok(m) = std::env::var("CITYSIM_HD_MAP") {
        c.world.map = m;
    }
    if let Some(r) = env::<f32>("CITYSIM_HD_FLATS") {
        c.ambient.flats_per_slot = r;
    }
    let mut w = World::new(seed, c);
    let home = |w: &World, a: EntityId| w.comp::<Household>(a).and_then(|h| h.home);
    let door = |w: &World, b: EntityId| w.comp::<citysim::components::Building>(b).map(|b| b.door);
    let (mut double, mut nearer, mut nearer_jobless, mut nearer_not_nearer) = (0u32, 0u32, 0u32, 0u32);
    let (mut away_nights, mut nights, mut hotel_nosuch, mut hotel_mission) = (0u32, 0u32, 0u32, 0u32);
    let mut away_people: BTreeMap<EntityId, (u32, u32)> = BTreeMap::new();
    let mut not_nearer_cost = 0u32;
    let mut dsample: Vec<String> = Vec::new();
    let mut asample: Vec<String> = Vec::new();
    let mut nsample: Vec<String> = Vec::new();
    let mut housed_per: BTreeMap<EntityId, u32> = BTreeMap::new();
    let mut housed_text: BTreeMap<EntityId, Vec<String>> = BTreeMap::new();
    let mut away_where: BTreeMap<String, u32> = BTreeMap::new();
    let mut sample: Vec<String> = Vec::new();
    let mut last_event = w.events.back().map_or(0, |e| e.id);
    for _day in 0..days {
        // Before the midnight: each body's Home and work door.
        let execs = classes::exec_set(&w);
        let before: BTreeMap<EntityId, (Option<EntityId>, bool)> = w
            .citizens()
            .into_iter()
            .map(|a| {
                let employed = w.comp::<Job>(a).is_some_and(|j| j.employer.is_some()) || execs.contains(&a);
                (a, (home(&w, a), employed))
            })
            .collect();
        for t in 0..TICKS_PER_DAY {
            w.tick();
            for e in w.debug_events.drain(..) {
                if e.text.contains("GoTo(Hotel)") && e.text.contains("NoSuchPlace") {
                    hotel_nosuch += 1;
                }
            }
            if t == 0 {
                // The midnight's Housed lines.
                let mut per: BTreeMap<EntityId, u32> = BTreeMap::new();
                for e in w.events.iter().filter(|e| e.id > last_event && e.kind == EventKind::Housed) {
                    let a = e.actors[0];
                    *per.entry(a).or_default() += 1;
                    *housed_per.entry(a).or_default() += 1;
                    housed_text.entry(a).or_default().push(format!("d{} {}", w.day(), e.text));
                    if e.text.contains("nearer work") && !e.text.contains("traded") {
                        nearer += 1;
                        let (old, _) = before.get(&a).copied().unwrap_or((None, false));
                        // Employed at the relocation (after the midnight's hires).
                        let employed = w.comp::<Job>(a).is_some_and(|j| j.employer.is_some())
                            || classes::exec_set(&w).contains(&a);
                        if !employed {
                            nearer_jobless += 1;
                            if sample.len() < 6 {
                                sample.push(format!("jobless d{}: {}", w.day(), e.text));
                            }
                        }
                        let work = w.comp::<Job>(a).and_then(|j| j.employer).and_then(|b| door(&w, b)).or_else(|| {
                            w.corps()
                                .into_iter()
                                .find(|&c| w.comp::<citysim::Corp>(c).is_some_and(|x| x.exec == Some(a)))
                                .and_then(|c| citysim::systems::life::hq_of(&w, c, a))
                                .and_then(|b| door(&w, b))
                        });
                        let new = e.actors.get(1).and_then(|&b| door(&w, b));
                        if let (Some(wk), Some(o), Some(n)) = (work, old.and_then(|o| door(&w, o)), new) {
                            if n.manhattan(wk) >= o.manhattan(wk) {
                                nearer_not_nearer += 1;
                            }
                            if life::commute_cost(&w, n, wk) >= life::commute_cost(&w, o, wk) {
                                not_nearer_cost += 1;
                                if nsample.len() < 6 {
                                    let role = w.comp::<Job>(a).map(|j| format!("{:?}", j.role));
                                    nsample.push(format!("d{} {role:?}: {}", w.day(), e.text));
                                }
                            }
                        }
                    }
                }
                double += per.values().filter(|&&n| n >= 2).count() as u32;
                for (&a, _) in per.iter().filter(|&(_, &n)| n >= 2) {
                    if dsample.len() < 8 {
                        let lines: Vec<String> = w
                            .events
                            .iter()
                            .filter(|e| e.id > last_event && e.kind == EventKind::Housed && e.actors[0] == a)
                            .map(|e| e.text.clone())
                            .collect();
                        dsample.push(format!("d{} {lines:?}", w.day()));
                    }
                }
            }
            if t == 3 * 60 {
                let execs = classes::exec_set(&w);
                for a in w.citizens() {
                    let guard = w.comp::<Job>(a).is_some_and(|j| j.role == Role::Guard && j.employer.is_some());
                    if !(execs.contains(&a) || guard) || !law::living(&w, a) || !demography::is_adult(&w, a) {
                        continue;
                    }
                    let Some(h) = home(&w, a) else { continue };
                    // Asleep at 03:00 (a night guard on shift is not counted).
                    let at = w.comp::<Position>(a).and_then(|p| p.building);
                    let asleep = w
                        .comp::<citysim::Brain>(a)
                        .and_then(|b| b.current_step())
                        .is_some_and(|s| s.action == citysim::ActionKind::Sleep);
                    if !asleep || w.has::<citysim::Sentence>(a) {
                        continue;
                    }
                    nights += 1;
                    away_people.entry(a).or_default().1 += 1;
                    if at != Some(h) {
                        away_nights += 1;
                        away_people.entry(a).or_default().0 += 1;
                        let kind = at
                            .and_then(|b| w.comp::<citysim::components::Building>(b))
                            .map_or("street".to_string(), |b| format!("{:?}", b.kind));
                        let who = if execs.contains(&a) { "exec" } else { "guard" };
                        *away_where.entry(format!("{who}@{kind}")).or_default() += 1;
                        if asample.len() < 24 && (who == "exec" || asample.len() < 12) {
                            let me = w.comp::<Position>(a).map(|p| p.tile).unwrap_or_default();
                            let ht = door(&w, h).map_or(0, |d| d.manhattan(me));
                            let wt = w
                                .comp::<Job>(a)
                                .and_then(|j| j.employer)
                                .and_then(|b| door(&w, b))
                                .map_or(0, |d| d.manhattan(me));
                            let hw = w
                                .comp::<Job>(a)
                                .and_then(|j| j.employer)
                                .and_then(|b| door(&w, b))
                                .zip(door(&w, h))
                                .map_or(0, |(x, y)| x.manhattan(y));
                            let tier = w.comp::<citysim::components::Building>(h).map_or(0, |b| b.tier);
                            asample.push(format!(
                                "d{} {who} {} @{kind}: {ht} tiles from Home (tier {tier}), {wt} from work, home-work {hw}",
                                w.day(),
                                w.name_of(a)
                            ));
                        }
                        if at
                            .and_then(|b| w.comp::<citysim::components::Building>(b))
                            .is_some_and(|b| b.kind == BuildingKind::Mission)
                        {
                            hotel_mission += 1;
                        }
                    }
                }
            }
            last_event = w.events.back().map_or(last_event, |e| e.id);
        }
    }
    let homeless = w.citizens().into_iter().filter(|&a| demography::is_adult(&w, a) && home(&w, a).is_none()).count();
    let chronic = away_people.values().filter(|&&(n, t)| n > 0 && n * 2 >= t).count();
    let away_n = away_people.values().filter(|&&(n, _)| n > 0).count();
    println!(
        "HD seed {seed} days {days}: double_housed {double} nearer {nearer} nearer_jobless {nearer_jobless} \
         nearer_not_nearer {nearer_not_nearer} (by commute ticks {not_nearer_cost}) exec_guard_away {away_nights}/{nights} nights \
         (in a Mission {hotel_mission}; {away_n} of {} people, {chronic} away half their nights) hotel_nosuchplace {hotel_nosuch} homeless_end {homeless}",
        away_people.len()
    );
    println!("HD away by place: {away_where:?}");
    let churn = housed_per.values().filter(|&&n| n >= 3).count();
    println!("HD housed 3+ midnights of {days}: {churn} bodies (of {} housed at a midnight)", housed_per.len());
    let mut top: Vec<(u32, EntityId)> = housed_per.iter().map(|(&a, &n)| (n, a)).collect();
    top.sort_unstable_by(|x, y| y.cmp(x));
    for (_, a) in top.into_iter().take(5) {
        let role = w.comp::<Job>(a).map(|j| format!("{:?}", j.role));
        println!("HD churn {role:?} {:?}", housed_text.get(&a).map(|v| v.iter().take(8).collect::<Vec<_>>()));
    }
    for s in dsample {
        println!("HD double {s}");
    }
    for s in asample {
        println!("HD away {s}");
    }
    for s in nsample {
        println!("HD not-nearer {s}");
    }
    for s in sample {
        println!("HD   {s}");
    }
}

/// The island pass phase 3c's device rows (print only): a save's adults
/// (`CITYSIM_HM_SAVE`, a `run --save-at` file) by housing kind (company
/// floors in a workplace, a company block apart, a plain Home by tier, the
/// homeless): n, mean mood, the share below `low_mood` (`[demography]
/// emigrate_mood`) and the mean `mood::housing_bias`.
#[test]
#[ignore = "tier: diag"]
fn housing_mood_rows() {
    let Ok(path) = std::env::var("CITYSIM_HM_SAVE") else {
        println!("HM set CITYSIM_HM_SAVE to a save file");
        return;
    };
    let w = citysim::save::load_from_file(std::path::Path::new(&path)).expect("a save");
    let low = w.config.demography.emigrate_mood;
    let mut rows: BTreeMap<&str, (u32, f64, u32, f64)> = BTreeMap::new();
    for a in common::adults(&w) {
        let Some(m) = w.comp::<citysim::components::Mood>(a).map(|m| m.value) else { continue };
        let kind = match home_of(&w, a).and_then(|h| w.comp::<Building>(h).map(|b| (h, b))) {
            None => "homeless",
            Some((h, b)) if w.company_homes.contains_key(&h) && b.within.is_some() => "company floor",
            Some((h, _)) if w.company_homes.contains_key(&h) => "company block",
            Some((_, b)) => ["home Sump", "home Mid", "home Spire"][usize::from(b.tier.min(2))],
        };
        let r = rows.entry(kind).or_default();
        r.0 += 1;
        r.1 += f64::from(m);
        r.2 += u32::from(m < low);
        r.3 += f64::from(citysim::mood::housing_bias(&w, a));
    }
    println!("HM kind | n | mean mood | below low_mood {low} | mean housing_bias");
    for (k, (n, sum, below, bias)) in rows {
        let n1 = f64::from(n.max(1));
        println!("HM {k} | {n} | {:.3} | {:.3} | {:.3}", sum / n1, f64::from(below) / n1, bias / n1);
    }
}
