//! Jobs and room P3 (docs/JOBS_V2.md § 1.3, plan J9-J15, P3 step 3): trades
//! as data. Each test is a seeded world and one mechanism: a kind's roles
//! and their places with floors, a trade's shift, the Super's round, a save
//! round trip with a trade, and an unknown trade key failing the load.

use citysim::systems::{demography, jobs, law, ownership};
use citysim::{BuildingKind, Config, EntityId, EventKind, Job, Role, World, TICKS_PER_DAY, TICKS_PER_HOUR};

fn world() -> World {
    World::new(42, Config::load())
}

fn trade(w: &World, key: &str) -> Role {
    w.config.trade_role(key).unwrap_or_else(|| panic!("trade {key} configured"))
}

/// A jobless free adult (lowest id).
fn jobless(w: &World) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            law::living(w, a)
                && demography::is_adult(w, a)
                && !w.has::<Job>(a)
                && !w.has::<citysim::Sentence>(a)
                && w.comp::<citysim::Brain>(a).is_some_and(|b| !b.emigrating)
        })
        .expect("a jobless adult")
}

/// J10: a Club employs its Hosts, then the Bouncers and Dancers, in row
/// order; a second floor doubles every per-floor role's places.
#[test]
fn test_roles_for_two_roles_and_floors() {
    let mut w = world();
    let (bouncer, dancer) = (trade(&w, "bouncer"), trade(&w, "dancer"));
    let roles = ownership::roles_for(&w, BuildingKind::Club);
    let host_full = citysim::systems::corp_brain::full_staff(&w, BuildingKind::Club);
    assert_eq!(roles, vec![(Role::Host, host_full), (bouncer, 2), (dancer, 3)]);
    // A Den has the Bouncer as a second role; a Farm no trade.
    assert_eq!(ownership::roles_for(&w, BuildingKind::Den).len(), 2);
    assert_eq!(ownership::roles_for(&w, BuildingKind::Farm).len(), 1);
    let club = w.buildings_of_kind(BuildingKind::Club)[0];
    assert_eq!(jobs::places_of(&w, club, bouncer), 2);
    assert_eq!(jobs::places_of(&w, club, dancer), 3);
    assert_eq!(jobs::places_of(&w, club, trade(&w, "night_stocker")), 0, "a Club has no stockers");
    w.comp_mut::<citysim::Building>(club).expect("club").floors = 2;
    assert_eq!(jobs::places_of(&w, club, Role::Host), 2 * host_full);
    assert_eq!(jobs::places_of(&w, club, bouncer), 4);
    assert_eq!(jobs::places_of(&w, club, dancer), 6);
    // J15: the seeded Club posted its trade places.
    let open = |r: Role| w.vacancies.get(&club).map_or(0, |v| v.iter().filter(|&&x| x == r).count());
    assert!(open(bouncer) + w.staff_of(club).len() >= 2, "the Bouncers' places were posted at seed");
}

/// J14: the Super's places: one per 15 of a corp's standing Blocks, at the
/// lowest id of each group; a city Block posts none.
#[test]
fn test_super_places_per_owned_group() {
    let w = world();
    let sup = trade(&w, "super");
    let corp = w
        .corps()
        .into_iter()
        .max_by_key(|&c| ownership::owned_of_kind(&w, Some(c), BuildingKind::Home).len())
        .expect("a landlord corp");
    let blocks: Vec<EntityId> = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Home)
        .into_iter()
        .filter(|&b| w.comp::<citysim::Building>(b).is_some_and(|bd| !bd.derelict && !bd.demolished))
        .collect();
    assert!(blocks.len() > 15, "the test needs a corp with more than 15 Blocks ({})", blocks.len());
    let places: usize = blocks.iter().map(|&b| jobs::places_of(&w, b, sup)).sum();
    assert_eq!(places, blocks.len().div_ceil(15));
    assert_eq!(jobs::places_of(&w, blocks[0], sup), 1);
    assert_eq!(jobs::places_of(&w, blocks[1], sup), 0);
    assert_eq!(jobs::places_of(&w, blocks[15], sup), 1);
    let city_block = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .find(|&b| w.owner_of(b).is_none())
        .expect("a city Block");
    assert_eq!(jobs::places_of(&w, city_block, sup), 0);
}

/// J12: a night trade works `[world] shift_night`, an evening trade the
/// `[leisure] evening_shift`, a split trade night for an even id and day
/// for an odd one; the wage is the row's.
#[test]
fn test_trade_shifts_and_wage() {
    let mut w = world();
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let stocker = trade(&w, "night_stocker");
    let a = jobless(&w);
    demography::hire(&mut w, a, market, stocker);
    let j = w.comp::<Job>(a).expect("hired");
    assert_eq!(j.role, stocker);
    assert_eq!(j.shifts, w.config.world.shift_night);
    assert_eq!(j.wage_per_day, w.config.trades[stocker.trade().expect("a trade").index()].wage);
    let club = w.buildings_of_kind(BuildingKind::Club)[0];
    let b = jobless(&w);
    let bouncer = trade(&w, "bouncer");
    demography::hire(&mut w, b, club, bouncer);
    assert_eq!(w.comp::<Job>(b).expect("hired").shifts, w.config.leisure.evening_shift);
    let clinic = w.buildings_of_kind(BuildingKind::Clinic)[0];
    let orderly = trade(&w, "orderly");
    let (mut even, mut odd) = (None, None);
    for _ in 0..2 {
        let c = jobless(&w);
        demography::hire(&mut w, c, clinic, orderly);
        let s = w.comp::<Job>(c).expect("hired").shifts.clone();
        if c.index.is_multiple_of(2) {
            even = Some(s);
        } else {
            odd = Some(s);
        }
    }
    if let Some(s) = even {
        assert_eq!(s, w.config.world.shift_night);
    }
    if let Some(s) = odd {
        assert_eq!(s, w.config.world.shift_day);
    }
    assert_eq!(w.workers(stocker), &[a]);
    w.check_indices().expect("indices");
}

/// J14: a Super over two workdays walks his round: he enters at least three of
/// his corp's Blocks in the district and the round is logged (`SuperRound`).
#[test]
fn test_super_walks_the_round() {
    let mut w = world();
    let sup = trade(&w, "super");
    w.run_ticks(TICKS_PER_DAY + 8 * TICKS_PER_HOUR);
    let s = w
        .workers(sup)
        .iter()
        .copied()
        .find(|&a| {
            law::living(&w, a)
                && !w.has::<citysim::Sentence>(a)
                && w.comp::<Job>(a).is_some_and(|j| {
                    citysim::exec::routine::workday_of(&w, a, j, j.shift_key_at(w.tick + 2 * TICKS_PER_HOUR))
                })
        })
        .expect("a Super on a workday");
    let post = w.comp::<Job>(s).and_then(|j| j.employer).expect("post");
    let corp = w.owner_of(post);
    let mut visited: Vec<EntityId> = Vec::new();
    // Two workdays: a person may run an errand into the morning and start
    // the round late.
    let end = w.tick + 2 * TICKS_PER_DAY;
    while w.tick < end {
        w.comp_mut::<citysim::Brain>(s).expect("brain").pinned = true;
        w.run_ticks(1);
        if let Some(b) = w.comp::<citysim::Position>(s).and_then(|p| p.building) {
            let blk = w.comp::<citysim::Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Home);
            if blk && b != post && w.owner_of(b) == corp && !visited.contains(&b) {
                visited.push(b);
            }
        }
    }
    let rounds = w.events.iter().filter(|e| e.kind == EventKind::SuperRound && e.actors.first() == Some(&s)).count();
    let br = w.comp::<citysim::Brain>(s).expect("brain");
    eprintln!(
        "Super {}: visited {} Blocks {:?}, rounds {rounds}; route {:?} legs {} key {:?}; lod {:?}",
        s.index,
        visited.len(),
        visited.iter().map(|b| b.index).collect::<Vec<_>>(),
        br.patrol_route.iter().map(|b| b.index).collect::<Vec<_>>(),
        br.patrol_legs,
        br.patrol_shift_key,
        br.lod
    );
    assert!(visited.len() >= 3, "the Super visited {} of his Blocks", visited.len());
    assert!(rounds >= 1, "no SuperRound logged");
    // Review fix: the round leaves the shift's end to the post, so both
    // shifts are worked and paid (the last one marked done).
    let j = w.comp::<Job>(s).expect("still a Super");
    let last = j.shift_key_at(w.tick.saturating_sub(TICKS_PER_DAY / 2));
    assert_eq!(j.last_shift_day, Some(last), "the last shift was worked to its end");
}

/// P3 review (stable posts): a Block that already employs a Super keeps its
/// post when a lower-id Block of the corp goes: demolishing the corp's
/// first Block (a post) moves no other Super's place and lays no Super off.
#[test]
fn test_super_posts_stable_when_a_block_goes() {
    let mut w = world();
    let sup = trade(&w, "super");
    let corp = w
        .corps()
        .into_iter()
        .max_by_key(|&c| ownership::owned_of_kind(&w, Some(c), BuildingKind::Home).len())
        .expect("a landlord corp");
    let blocks: Vec<EntityId> = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Home)
        .into_iter()
        .filter(|&b| w.comp::<citysim::Building>(b).is_some_and(|bd| !bd.derelict && !bd.demolished))
        .collect();
    assert!(blocks.len() > 15, "a corp with two groups ({})", blocks.len());
    // A Super at every post.
    let posts: Vec<EntityId> = blocks.iter().copied().filter(|&b| jobs::places_of(&w, b, sup) > 0).collect();
    w.vacancies.clear();
    let mut supers = Vec::new();
    for &b in &posts {
        let a = jobless(&w);
        demography::hire(&mut w, a, b, sup);
        supers.push((a, b));
    }
    // The first Block goes (its Super with it).
    let first = blocks[0];
    if let Some(&(a, _)) = supers.iter().find(|&&(_, b)| b == first) {
        citysim::systems::economy::dismiss(&mut w, a, Some(first), "the Block came down".to_string());
    }
    w.comp_mut::<citysim::Building>(first).expect("block").demolished = true;
    for &(_, b) in supers.iter().filter(|&&(_, b)| b != first) {
        assert_eq!(jobs::places_of(&w, b, sup), 1, "a served post keeps its place");
    }
    let laid =
        |w: &World| w.events.iter().filter(|e| e.kind == EventKind::LaidOff && e.text.contains("as Super")).count();
    let before = laid(&w);
    citysim::systems::wages::shed_extra(&mut w, corp, 0.0);
    assert_eq!(laid(&w), before, "no Super laid off");
    let total: usize = blocks.iter().map(|&b| jobs::places_of(&w, b, sup)).sum();
    assert_eq!(total, (blocks.len() - 1).div_ceil(15), "one post per started group of the standing Blocks");
}

/// P3 review: the last two hours of a Super's shift belong to his post (the
/// shift is worked and paid there): no round leg is planned then.
#[test]
fn test_super_round_yields_to_the_post_near_shift_end() {
    let mut w = world();
    let sup = trade(&w, "super");
    w.run_ticks(TICKS_PER_DAY);
    let s = w
        .workers(sup)
        .iter()
        .copied()
        .find(|&a| {
            law::living(&w, a)
                && w.comp::<Job>(a).is_some_and(|j| {
                    citysim::exec::routine::workday_of(&w, a, j, j.shift_key_at(w.tick + 10 * TICKS_PER_HOUR))
                })
        })
        .expect("a Super on a workday");
    w.run_ticks(10 * TICKS_PER_HOUR); // 10:00, on shift
    if let Some(j) = w.comp_mut::<Job>(s) {
        j.last_shift_day = None; // not yet worked today
    }
    assert!(citysim::systems::trades::round_plan(&mut w, s).is_some(), "a leg at 10:00");
    w.run_ticks(6 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2); // 16:30: 90 minutes left
    if let Some(b) = w.comp_mut::<citysim::Brain>(s) {
        b.patrol_legs = 0; // the round unfinished
    }
    if let Some(j) = w.comp_mut::<Job>(s) {
        j.last_shift_day = None;
    }
    assert!(citysim::systems::trades::round_plan(&mut w, s).is_none(), "at 16:30 the Super goes to his post");
}

/// J32: a save with a trade's Job round-trips (the key, not the index) and
/// the indices rebuild; an unknown key fails the load.
#[test]
fn test_save_round_trip_with_a_trade_and_unknown_key_fails() {
    let mut w = world();
    let hotel = w.buildings_of_kind(BuildingKind::Hotel).first().copied().expect("a Hotel");
    let porter = trade(&w, "night_porter");
    let a = jobless(&w);
    demography::hire(&mut w, a, hotel, porter);
    let text = citysim::save::to_ron(&w);
    assert!(text.contains("Trade(\"night_porter\")"), "the role is saved by key");
    let back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(back.comp::<Job>(a).map(|j| j.role), Some(porter));
    assert_eq!(back.workers(porter), w.workers(porter));
    back.check_indices().expect("indices");
    assert_eq!(citysim::save::to_ron(&back), text, "same bytes after the round trip");
    let bad = text.replace("Trade(\"night_porter\")", "Trade(\"lamplighter\")");
    let err = citysim::save::from_ron(&bad).expect_err("an unknown trade key fails the load");
    assert!(err.to_string().contains("lamplighter"), "{err}");
}

/// P6 x P3: a trade's open post is offerable to a migrant: the offer
/// migrant arrives holding the trade with its row's shift, the role's
/// skill floored at `offer_skill`, and the vacancy closes.
#[test]
fn test_offer_migrant_takes_a_trade_post() {
    let mut w = world();
    let hotel = w.buildings_of_kind(BuildingKind::Hotel).first().copied().expect("a Hotel");
    let porter = trade(&w, "night_porter");
    w.vacancies.insert(hotel, vec![porter]);
    let id = demography::spawn_immigrant_for(&mut w, hotel, porter).expect("an offer migrant");
    let j = w.comp::<Job>(id).expect("hired at spawn");
    assert_eq!((j.role, j.employer), (porter, Some(hotel)));
    assert_eq!(j.shifts, w.config.world.shift_night);
    let floor = w.config.demography.offer_skill;
    let s = w.comp::<citysim::Skills>(id).expect("skills").persuasion;
    assert!(s >= floor - 1e-6, "persuasion {s} floored at {floor}");
    assert!(!w.vacancies.get(&hotel).is_some_and(|v| v.contains(&porter)), "the post closed");
    w.check_indices().expect("indices");
}
