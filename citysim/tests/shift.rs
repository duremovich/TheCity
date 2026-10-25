//! The pre-view pass phase 2 (docs/PREVIEW_PASS.md § 1, OPEN-137): the
//! shift's shapes (`systems/shift.rs`). Each test is a seeded world and one
//! mechanism: a `stations` shift moves between spots in blocks and is paid,
//! and its break is a meal when hungry, else outside the door. Phase 3
//! (OPEN-121, OPEN-141, OPEN-145): a `haul` shift carries loads to another
//! building; a courier hands a parcel over only to someone there to sign;
//! a day cook serves the day's customers; a bouncer turns a patron away at
//! the door.

use citysim::components::{ActionInstance, Brain, GoalKind, Lod, Position};
use citysim::config::ShiftShape;
use citysim::systems::{law, leisure, lod, service, shift, streetwork};
use citysim::{
    ActionKind, BuildingKind, Config, EntityId, ExecState, Job, Plan, TilePos, World, TICKS_PER_DAY, TICKS_PER_HOUR,
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
            w.buildings_by_kind().get(k).is_some_and(|v| {
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

// ---------------------------------------------------------------------------
// Phase 3: haul and counter
// ---------------------------------------------------------------------------

fn jobs2(w: &World) -> citysim::stats::Jobs2Cols {
    w.stats.current.jobs2.clone().unwrap_or_default()
}

/// A housed day worker on a `haul` shift on a workday (not a courier), with
/// `[shift.haul]` destinations from his post: (worker, post, destinations).
fn haul_worker(w: &World) -> (EntityId, EntityId, Vec<EntityId>) {
    w.citizens()
        .into_iter()
        .filter(|&a| law::living(w, a) && !w.has::<citysim::Sentence>(a))
        .find_map(|a| {
            let j = w.comp::<Job>(a)?;
            let e = j.employer?;
            let key = j.next_shift_key(w.tick);
            let day = j.shifts.len() == 1 && j.shifts.iter().all(|&(s, e)| s >= 360 && e <= 1200);
            if !(day
                && shift::shape_of(w, j.role) == ShiftShape::Haul
                && !streetwork::haul_courier(w, a)
                && citysim::exec::routine::workday_of(w, a, j, key)
                && w.comp::<citysim::Household>(a).is_some_and(|h| h.home.is_some()))
            {
                return None;
            }
            let dests = shift::haul_dests(w, a, e);
            (!dests.is_empty()).then_some((a, e, dests))
        })
        .expect("a haul day worker with a destination in reach")
}

/// Diagnosis (prints only): the haul trades on seed 42 day 1 and how many
/// of each have a destination in `[shift.haul] haul_reach`.
#[test]
#[ignore = "tier: diag"]
fn diag_haul_destinations() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    let mut by: std::collections::BTreeMap<String, (u32, u32, usize)> = std::collections::BTreeMap::new();
    for a in w.citizens() {
        let Some(j) = w.comp::<Job>(a) else { continue };
        let Some(e) = j.employer else { continue };
        if shift::shape_of(&w, j.role) != ShiftShape::Haul {
            continue;
        }
        let d = shift::haul_dests(&w, a, e);
        let r = by.entry(format!("{:?}", j.role)).or_default();
        r.0 += 1;
        r.1 += u32::from(!d.is_empty());
        r.2 += d.len();
    }
    for (k, (n, with, dests)) in by {
        eprintln!("FINDING {k}: {n} workers, {with} with a destination, {dests} destinations");
    }
}

/// Phase 3 (§ 1, `haul`): a haul shift loads at the post, carries the load
/// to another building and unloads it there, comes back, and is still
/// closed at the post (P5: the wage). The base works it as one block at one
/// tile (no `Unload` exists).
#[test]
fn test_haul_shift_moves_loads_between_two_buildings() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    let (a, post, dests) = haul_worker(&w);
    lod::set_lod(&mut w, a, Lod::Full);
    let key = w.comp::<Job>(a).expect("job").next_shift_key(w.tick);
    let j0 = jobs2(&w);
    let (_, _, trace) = shift_trace(&mut w, a, post);
    let unloads = trace
        .iter()
        .filter(|&&(b, _, k)| k == Some(ActionKind::Unload) && b.is_some_and(|b| dests.contains(&b)))
        .count();
    let loads = trace.iter().filter(|&&(b, _, k)| b == Some(post) && k.is_some_and(|k| k.is_work())).count();
    let j = jobs2(&w);
    eprintln!(
        "hauler {} at {}: {unloads} unload ticks at destinations, {loads} work ticks at the post, trips {}",
        a.index,
        post.index,
        j.haul_trips - j0.haul_trips
    );
    assert!(unloads > 0, "no load set down at another building");
    assert!(loads > 0, "no load made up at the post");
    assert_eq!(w.comp::<Job>(a).expect("job").last_shift_day, Some(key), "the haul shift closed at the post");
}

/// The Freight Yards' haul couriers on a workday, housed: (courier, yard).
fn couriers(w: &World) -> Vec<(EntityId, EntityId)> {
    w.citizens()
        .into_iter()
        .filter(|&a| law::living(w, a) && !w.has::<citysim::Sentence>(a))
        .filter_map(|a| {
            let j = w.comp::<Job>(a)?;
            let key = j.next_shift_key(w.tick);
            (streetwork::haul_courier(w, a)
                && citysim::exec::routine::workday_of(w, a, j, key)
                && w.comp::<citysim::Household>(a).is_some_and(|h| h.home.is_some()))
            .then_some((a, j.employer?))
        })
        .collect()
}

/// Two staffed buildings within `[jobs2] site_reach` of `yard` (not it),
/// nearest first, that fall to `c` (`courier_addresses`), each with its
/// first staff member.
fn two_addresses(w: &mut World, c: EntityId, yard: EntityId) -> Vec<(EntityId, EntityId)> {
    let ydoor = w.comp::<citysim::Building>(yard).expect("yard").door;
    let day = w.day();
    let mut near: Vec<(u32, EntityId)> = w
        .citizens()
        .into_iter()
        .filter_map(|s| w.comp::<Job>(s)?.employer)
        .filter(|&b| b != yard)
        .filter_map(|b| w.comp::<citysim::Building>(b).map(|x| (x.door.manhattan(ydoor), b)))
        .filter(|&(d, _)| d <= w.config.jobs2.site_reach)
        .collect();
    near.sort_unstable();
    near.dedup();
    let mut mine = Vec::new();
    for &(_, b) in &near {
        let to = w.staff_of(b)[0];
        w.jobs2.parcels.push(streetwork::Parcel { from: yard, to, at: b, yard, fee: 0, day });
        if streetwork::courier_addresses(w, c).contains(&b) {
            mine.push((b, to));
        }
        w.jobs2.parcels.pop();
        if mine.len() == 2 {
            break;
        }
    }
    mine
}

/// Phase 3 (OPEN-141, P3): a courier carries each parcel to its address and
/// hands it over only when someone is there to sign: a parcel to a present
/// addressee is handed to him (a `Parcel` encounter) with the courier at the
/// address; one to an address with nobody on shift stays (`parcels_missed`)
/// and goes back to the Yard at the shift's end. The base's Full courier
/// handed over none (the Statistical colleagues' first-hour round took the
/// Yard's parcels before his first leg arrived).
#[test]
fn test_courier_hands_a_parcel_to_a_present_addressee_and_not_to_an_absent_one() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    w.jobs2.parcels.clear();
    let day = w.day();
    let (c, yard, mine) = couriers(&w)
        .into_iter()
        .find_map(|(c, y)| {
            let mine = two_addresses(&mut w, c, y);
            (mine.len() == 2).then_some((c, y, mine))
        })
        .expect("a courier with two addresses in reach");
    lod::set_lod(&mut w, c, Lod::Full);
    let ((here, r), (away, _)) = (mine[0], mine[1]);
    // The present addressee: a body inside on an all-day shift. The absent
    // address: every staff member a body off shift all day.
    w.comp_mut::<Job>(r).expect("job").shifts = vec![(1, 1440)];
    lod::set_lod(&mut w, r, Lod::Full);
    for s in w.staff_of(away).to_vec() {
        w.comp_mut::<Job>(s).expect("job").shifts = vec![(0, 1)];
        lod::set_lod(&mut w, s, Lod::Full);
    }
    let to_away = w.staff_of(away)[0];
    for (b, to) in [(here, r), (away, to_away)] {
        w.jobs2.parcels.push(streetwork::Parcel { from: yard, to, at: b, yard, fee: 0, day });
    }
    let j0 = jobs2(&w);
    let end = w.tick + 16 * TICKS_PER_HOUR;
    let mut handed_inside = false;
    while w.tick < end {
        w.comp_mut::<Brain>(c).expect("brain").pinned = true;
        if w.comp::<Position>(r).and_then(|p| p.building) != Some(here) {
            w.enter_building(r, here);
        }
        let before = w.jobs2.parcels.iter().any(|p| p.at == here);
        w.run_ticks(1);
        let after = w.jobs2.parcels.iter().any(|p| p.at == here);
        if before && !after {
            handed_inside = w.comp::<Position>(c).and_then(|p| p.building) == Some(here);
        }
    }
    let j = jobs2(&w);
    eprintln!(
        "courier {}: delivered {}, missed {}, returned {}, trips {}",
        c.index,
        j.parcels_delivered - j0.parcels_delivered,
        j.parcels_missed - j0.parcels_missed,
        j.parcels_returned - j0.parcels_returned,
        j.haul_trips - j0.haul_trips
    );
    assert!(handed_inside, "the parcel was handed over with the courier at the address");
    assert!(service::regulars_of(&w, c).iter().any(|x| x.who == r), "the Parcel encounter names the addressee");
    assert!(j.parcels_missed > j0.parcels_missed, "nobody at the second address: the parcel stayed");
    assert!(!w.jobs2.parcels.iter().any(|p| p.at == away), "the unsigned parcel went back at the shift's end");
}

/// Phase 3 (OPEN-145): a NoodleBar's Cook on the day shift, a body at his
/// counter, serves the day's customers: the Statistical evening's visits
/// to his venue are drawn into his shift (`leisure::counter_due`) and
/// served then. The base served the off-screen customers at 21:00, so a
/// day cook served nobody.
#[test]
fn test_day_cook_serves_day_customers() {
    // The island pass phase 4: on the commute pressure's city seed 42's
    // first NoodleBar cook serves no off-screen customer on his day (who
    // lives near which bar moved by day 1); the premise reads the base
    // relocation's households.
    let mut w = World::new(42, {
        let mut c = Config::load();
        c.commute_pressure = None;
        c
    });
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    let day = w.config.world.shift_day.clone();
    let (k, nb) =
        w.buildings_of_kind(BuildingKind::NoodleBar)
            .iter()
            .find_map(|&b| {
                let k =
                    w.staff_of(b).iter().copied().find(|&s| {
                        w.comp::<Job>(s).is_some_and(|j| j.role == citysim::Role::Cook) && law::living(&w, s)
                    })?;
                Some((k, b))
            })
            .expect("a NoodleBar with a Cook");
    assert_eq!(shift::shape_of(&w, citysim::Role::Cook), ShiftShape::Counter);
    w.comp_mut::<Job>(k).expect("job").shifts = day.clone();
    lod::set_lod(&mut w, k, Lod::Full);
    // To the end of tomorrow's day shift.
    let end = w.tick - w.tick % TICKS_PER_DAY + TICKS_PER_DAY + u64::from(day[0].1);
    // The premise by hand: who lives near which bar is the relocation's
    // (it moves with every phase), so eight off-screen adults with a purse
    // have a visit queued to his bar across his shift, as the evening's
    // draw (`leisure::queue_counter`) would leave them.
    let window = u64::from(
        w.config.shift.as_ref().and_then(|c| c.counter.as_ref()).map_or(1, |c| c.counter_settle_window.max(1)),
    );
    let day0 = w.tick - w.tick % TICKS_PER_DAY + TICKS_PER_DAY;
    let (lo, hi) = (u64::from(day[0].0), u64::from(day[0].1));
    let guests: Vec<EntityId> = w
        .brained()
        .into_iter()
        .filter(|&x| w.comp::<Brain>(x).is_some_and(|b| b.lod == Lod::Statistical))
        .filter(|&x| {
            law::living(&w, x) && citysim::systems::demography::is_adult(&w, x) && leisure::spendable(&w, x) >= 30
        })
        .take(8)
        .collect();
    assert!(!guests.is_empty(), "off-screen adults with a purse");
    for (i, &x) in guests.iter().enumerate() {
        let at = day0 + lo + (hi - lo) * (i as u64 + 1) / (guests.len() as u64 + 1);
        w.counter_due.push(leisure::CounterVisit {
            at: at - at % window,
            who: x,
            venue: nb,
            act: ActionKind::EatOut,
            evening: None,
            day: w.day(),
            k: i as u32,
        });
    }
    // The cook's ledger: (customer, visits); a new visit on his shift by an
    // off-screen customer is one the base would have served at 21:00.
    let ledger = |w: &World| {
        service::regulars_of(w, k).iter().map(|x| (x.who, u32::from(x.visits))).collect::<Vec<(EntityId, u32)>>()
    };
    let (mut served_day, mut off_screen, mut last) = (0, 0, ledger(&w));
    while w.tick < end {
        w.comp_mut::<Brain>(k).expect("brain").pinned = true;
        w.run_ticks(1);
        let now = ledger(&w);
        let tod = (w.tick % TICKS_PER_DAY) as u16;
        if tod >= day[0].0 && tod <= day[0].1 {
            for &(who, n) in &now {
                let before = last.iter().find(|&&(x, _)| x == who).map_or(0, |&(_, m)| m);
                if n > before {
                    served_day += n - before;
                    off_screen += u32::from(w.comp::<Brain>(who).is_some_and(|b| b.lod == Lod::Statistical));
                }
            }
        }
        last = now;
    }
    eprintln!("cook {} at {}: {served_day} served on the day shift, {off_screen} off screen", k.index, nb.index);
    assert!(off_screen > 0, "the day cook served no off-screen customer on his shift");
}

/// Phase 3 (OPEN-121): the bouncer's door. A patron known to the law
/// (`heat` over `door_heat`) still sees the Club as open from the street
/// (`door_seen`: the door is the bouncer's call, made at the door), walks
/// in and is turned away there by a bouncer on shift at the door tile
/// (`door_refusals`), and passes the Club over after. The base read the
/// bouncer's verdict at the pick, so nobody was ever refused.
#[test]
fn test_bouncer_refuses_a_patron_at_the_door() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    let (bo, club) = w
        .buildings_of_kind(BuildingKind::Club)
        .iter()
        .find_map(|&b| {
            let s = w.staff_of(b).iter().copied().find(|&s| {
                w.comp::<Job>(s)
                    .is_some_and(|j| citysim::systems::trades::row_of(&w, j.role).is_some_and(|r| r.id == "bouncer"))
                    && law::living(&w, s)
            })?;
            Some((s, b))
        })
        .expect("a Club with a bouncer");
    w.comp_mut::<Job>(bo).expect("job").shifts = vec![(1, 1440)];
    lod::set_lod(&mut w, bo, Lod::Full);
    w.enter_building(bo, club);
    w.comp_mut::<citysim::Personality>(bo).expect("personality").courage = 1.0;
    let p = w
        .citizens()
        .into_iter()
        .find(|&a| {
            a != bo
                && law::living(&w, a)
                && !w.has::<citysim::Sentence>(a)
                && w.comp::<Job>(a).is_none_or(|j| j.employer != Some(club))
                && citysim::systems::demography::is_adult(&w, a)
        })
        .expect("a patron");
    lod::set_lod(&mut w, p, Lod::Full);
    let mut rep = citysim::systems::reputation::rep(&w, p);
    rep.heat = 0.9;
    w.reputation[p.index as usize] = Some(rep);
    w.comp_mut::<citysim::Wallet>(p).expect("wallet").coins = 500;
    assert!(!leisure::door_ok(&w, p, club), "the bouncer would turn him away");
    assert!(leisure::door_seen(&w, p, club), "from the street the Club is open to him");
    w.enter_building(p, club);
    let now = w.tick;
    {
        let b = w.comp_mut::<Brain>(p).expect("brain");
        b.plan = Some(Plan {
            goal: GoalKind::Unwind,
            target: Some(club),
            steps: vec![ActionInstance { action: ActionKind::Enjoy, target: Some(club), tile: None }],
            started_tick: now,
        });
        b.plan_step = 0;
        b.pinned = true;
    }
    let j0 = jobs2(&w);
    w.run_ticks(2);
    assert!(jobs2(&w).door_refusals > j0.door_refusals, "turned away at the door");
    assert!(!leisure::door_seen(&w, p, club), "the Club is passed over after the refusal");
}
