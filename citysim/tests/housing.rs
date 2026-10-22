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
        let mut w = common::world();
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
