//! The pre-view pass phase 2 (docs/PREVIEW_PASS.md § 1, OPEN-137): the
//! shift's shapes (`systems/shift.rs`). Each test is a seeded world and one
//! mechanism: a `stations` shift moves between spots in blocks and is paid,
//! and its break is a meal when hungry, else outside the door.

use citysim::config::ShiftShape;
use citysim::systems::{law, shift};
use citysim::{
    ActionKind, BuildingKind, Config, EntityId, ExecState, Job, TilePos, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn world() -> World {
    World::new(42, Config::load())
}

/// A day worker on a `stations` shift on a workday whose workplace has four
/// interior tiles, housed (lowest id); with `near_food`, a Market or a
/// NoodleBar within the break's walk of the workplace's door.
fn stations_worker(w: &World, near_food: bool) -> (EntityId, EntityId) {
    let walk = w.config.shift.as_ref().map_or(0, |c| u64::from(c.break_walk_max));
    let reach = walk / w.config.exec.move_ticks_full.max(1) / 2;
    let fed_near = |door: TilePos| {
        [BuildingKind::Market, BuildingKind::NoodleBar].iter().any(|k| {
            w.buildings_by_kind.get(k).is_some_and(|v| {
                v.iter().any(|&m| {
                    w.comp::<citysim::Building>(m).is_some_and(|b| u64::from(b.door.manhattan(door)) <= reach)
                })
            })
        })
    };
    w.citizens()
        .into_iter()
        .filter(|&a| law::living(w, a) && !w.has::<citysim::Sentence>(a))
        .find_map(|a| {
            let j = w.comp::<Job>(a)?;
            let e = j.employer?;
            let key = j.next_shift_key(w.tick);
            let day = j.shifts.len() == 1 && j.shifts.iter().all(|&(s, e)| s >= 360 && e <= 1200);
            let tiles = w.comp::<citysim::Building>(e)?.interior().filter(|&t| w.map.walkable(t)).count();
            (day && tiles >= 4
                && shift::shape_of(w, j.role) == ShiftShape::Stations
                && citysim::exec::routine::workday_of(w, a, j, key)
                && w.comp::<citysim::Household>(a).is_some_and(|h| h.home.is_some())
                && (!near_food
                    || w.comp::<citysim::Building>(e)
                        .is_some_and(|b| b.kind != BuildingKind::Clinic && fed_near(b.door))))
            .then_some((a, e))
        })
        .expect("a stations day worker")
}

type Trace = Vec<(Option<EntityId>, TilePos, Option<ActionKind>)>;

/// Runs `a` (pinned) for 16 hours: the tiles worked inside `post`, the
/// longest work action, and every tick's (building, tile, running action).
fn shift_trace(w: &mut World, a: EntityId, post: EntityId) -> (Vec<TilePos>, u64, Trace) {
    let end = w.tick + 16 * TICKS_PER_HOUR;
    let (mut tiles, mut longest, mut trace) = (Vec::new(), 0u64, Vec::new());
    while w.tick < end {
        w.comp_mut::<citysim::Brain>(a).expect("brain").pinned = true;
        w.run_ticks(1);
        let p = w.comp::<citysim::Position>(a).expect("position").clone();
        let running = match w.comp::<citysim::Brain>(a).expect("brain").exec {
            ExecState::Use { kind, until, started } => {
                if kind.is_work() {
                    longest = longest.max(until - started);
                    if p.building == Some(post) && !tiles.contains(&p.tile) {
                        tiles.push(p.tile);
                    }
                }
                Some(kind)
            }
            _ => None,
        };
        trace.push((p.building, p.tile, running));
    }
    (tiles, longest, trace)
}

/// P2 (OPEN-137): a `stations` shift moves between at least two tiles of
/// the workplace in blocks no longer than `station_block_max`, and is still
/// worked to its end and closed (P5: the wage at the post). On the base the
/// shift is one ClerkWork of 530-540 minutes at one tile.
#[test]
fn test_stations_shift_moves_between_spots_in_blocks() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    let (a, post) = stations_worker(&w, false);
    let key = w.comp::<Job>(a).expect("job").next_shift_key(w.tick);
    let max = u64::from(w.config.shift.as_ref().expect("[shift]").station_block_max);
    let (tiles, longest, _) = shift_trace(&mut w, a, post);
    let j = w.comp::<Job>(a).expect("still employed");
    eprintln!("worker {}: tiles {tiles:?}, longest block {longest} min, last shift {:?}", a.index, j.last_shift_day);
    assert!(tiles.len() >= 2, "a stations shift worked {} tile(s)", tiles.len());
    assert!(longest <= max, "a work action of {longest} min (max {max})");
    assert_eq!(j.last_shift_day, Some(key), "the shift was worked to its end and closed");
}

/// P7: the mid-shift break: a hungry worker with coins and no packed meal
/// eats at a food venue (a Market's `BuyFood` or a NoodleBar's `EatOut`)
/// and comes back to the post; a fed one stands the break outside the
/// workplace's door.
#[test]
fn test_stations_break_meal_if_hungry_else_the_door() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    let (a, post) = stations_worker(&w, true);
    // To 90 minutes before mid-shift (before the break, staggered up to 60).
    let (s, e) = w.comp::<Job>(a).expect("job").shifts[0];
    let mid = w.tick - (w.tick % TICKS_PER_DAY) + u64::from(s + (e - s) / 2);
    while w.tick + 90 < mid {
        w.comp_mut::<citysim::Brain>(a).expect("brain").pinned = true;
        w.run_ticks(1);
    }
    let mut hungry = w.clone();
    hungry.comp_mut::<citysim::Needs>(a).expect("needs").hunger = 0.35;
    hungry.comp_mut::<citysim::Inventory>(a).expect("inventory").food = 0;
    hungry.comp_mut::<citysim::Wallet>(a).expect("wallet").coins = 500;
    let mut fed = w;
    fed.comp_mut::<citysim::Needs>(a).expect("needs").hunger = 1.0;
    let (_, _, ht) = shift_trace(&mut hungry, a, post);
    let food = |b: Option<EntityId>| {
        b.and_then(|b| hungry.comp::<citysim::Building>(b))
            .is_some_and(|bd| matches!(bd.kind, BuildingKind::Market | BuildingKind::NoodleBar))
    };
    let meal = ht.iter().position(|&(b, _, k)| food(b) && matches!(k, Some(ActionKind::BuyFood | ActionKind::EatOut)));
    let back = meal.is_some_and(|i| ht[i..].iter().any(|&(b, _, k)| b == Some(post) && k.is_some_and(|k| k.is_work())));
    assert!(
        meal.is_some() && back,
        "the hungry worker's break: a meal at a food venue {meal:?}, back at the post {back}"
    );
    let (_, _, ft) = shift_trace(&mut fed, a, post);
    let door = fed.comp::<citysim::Building>(post).map(|bd| fed.outside_door(bd)).expect("post");
    let outside = ft.iter().filter(|&&(b, t, k)| b.is_none() && t == door && k == Some(ActionKind::Break)).count();
    assert!(outside >= 20, "the fed worker stood {outside} min of the break outside the door");
}

/// Diagnosis (prints only): the stations shifts left unclosed at 05:00 next
/// on seed 42, days 1-4, and where their bodies were.
#[test]
#[ignore = "tier: diag"]
fn diag_unclosed_stations_shifts() {
    let mut w = world();
    for day in 1..5u64 {
        w.run_ticks((day + 1) * TICKS_PER_DAY + 5 * TICKS_PER_HOUR - w.tick);
        let (mut open, mut all) = (0, 0);
        for a in w.citizens() {
            let (Some(b), Some(j)) = (w.comp::<citysim::Brain>(a), w.comp::<Job>(a)) else { continue };
            let Some(s) = b.shift.as_deref() else { continue };
            if s.key != day as i64 || j.shifts.iter().any(|&(s0, _)| s0 == 0) {
                continue;
            }
            all += 1;
            if j.last_shift_day != Some(s.key) {
                open += 1;
                eprintln!(
                    "day {day} {}: lod {:?} stage {} goal {:?} plan {:?} at post {} sick {}",
                    a.index,
                    b.lod,
                    s.stage,
                    b.current_goal,
                    b.plan.as_ref().map(|p| p.goal),
                    w.comp::<citysim::Position>(a).and_then(|p| p.building) == j.employer,
                    citysim::systems::health::sick(&w, a)
                );
                for e in w.events.iter().filter(|e| e.actors.first() == Some(&a) && e.tick / TICKS_PER_DAY == day) {
                    eprintln!("    {} {:?} {}", e.tick % TICKS_PER_DAY, e.kind, e.text);
                }
            }
        }
        eprintln!("FINDING day {day}: {open} of {all} stations shifts unclosed");
    }
}
