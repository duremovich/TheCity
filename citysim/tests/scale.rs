//! M10 phase 1: tier and role indices, and the spread Statistical tick.
//! M10 phase 2: map v2, zones, nearest-of-kind, the flow-field LRU, 2,000 housed.

use std::collections::BTreeMap;

use citysim::systems::{demography, law, lod};
use citysim::{
    save, Brain, Building, BuildingKind, Config, Crime, DeathCause, EntityId, Household, Job, Lod, Map, Needs,
    Position, Role, TileKind, TilePos, World, Zone, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

#[test]
fn test_bodies_match_tiers_after_assignment() {
    let mut w = world(101);
    w.run_ticks(TICKS_PER_HOUR + 1);
    w.check_indices().expect("indices in step");
    let expected: Vec<EntityId> =
        w.citizens().into_iter().filter(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)).collect();
    assert_eq!(w.bodies(), expected);
    assert!(!w.tier(Lod::Statistical).is_empty(), "nobody statistical");
    let total = w.tier(Lod::Full).len() + w.tier(Lod::Coarse).len() + w.tier(Lod::Statistical).len();
    assert_eq!(total, w.citizens().into_iter().filter(|&id| w.has::<Brain>(id)).count());
}

#[test]
fn test_tiers_follow_spawn_and_death() {
    let mut w = world(102);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let imm = demography::spawn_immigrant(&mut w);
    assert!(w.tier(Lod::Coarse).contains(&imm), "an immigrant starts Coarse");
    w.check_indices().expect("after spawn");

    let victim = *w.tier(Lod::Statistical).first().expect("a statistical agent");
    w.kill(victim, DeathCause::OldAge);
    for lod in [Lod::Full, Lod::Coarse, Lod::Statistical] {
        assert!(!w.tier(lod).contains(&victim), "dead agent left in {lod:?}");
    }
    for role in Role::ALL {
        assert!(!w.workers(role).contains(&victim), "dead agent left in {role:?}");
    }
    w.check_indices().expect("after death");
}

#[test]
fn test_guard_index_tracks_hire_and_fire() {
    let mut w = world(103);
    w.run_ticks(5);
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let id = w
        .citizens()
        .into_iter()
        .find(|&id| w.has::<Brain>(id) && !w.has::<Job>(id) && demography::is_adult(&w, id))
        .expect("an unemployed adult");
    assert!(!w.guards().contains(&id));
    demography::hire(&mut w, id, jail, Role::Guard);
    assert!(w.guards().contains(&id));
    assert!(w.guards().windows(2).all(|p| p[0] < p[1]), "guards stay sorted");
    w.check_indices().expect("after hire");
    w.vacate_job(id);
    assert!(!w.guards().contains(&id));
    w.check_indices().expect("after fire");
}

#[test]
fn test_statistical_runs_once_per_hour_spread() {
    // Observe the sim, not the formula: a Statistical agent's hunger only
    // moves when its spread hourly run decays it (needs::run skips the tier),
    // so it must change exactly 24 times a day, each on its own slot.
    let mut w = world(104);
    w.run_ticks(TICKS_PER_HOUR);
    let start = w.tick;
    assert_eq!(start % TICKS_PER_HOUR, 0);
    let watched: Vec<EntityId> = w.tier(Lod::Statistical).to_vec();
    let hunger = |w: &World, id: EntityId| w.comp::<Needs>(id).map(|n| n.hunger);
    let mut last: BTreeMap<EntityId, Option<f32>> = watched.iter().map(|&id| (id, hunger(&w, id))).collect();
    let mut steady: BTreeMap<EntityId, bool> = watched.iter().map(|&id| (id, true)).collect();
    let mut changes: BTreeMap<EntityId, u32> = BTreeMap::new();
    let mut max_due = 0usize;
    let mut max_n = 0usize;
    while w.tick < start + TICKS_PER_DAY {
        let due = lod::due_this_tick(&w);
        max_due = max_due.max(due.len());
        max_n = max_n.max(w.tier(Lod::Statistical).len());
        let slot = w.tick % TICKS_PER_HOUR;
        w.tick();
        for &id in &watched {
            let now = hunger(&w, id);
            let stat = w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical);
            if !stat || now.is_none_or(|h| h <= 0.0) {
                steady.insert(id, false);
            }
            if now != last[&id] {
                *changes.entry(id).or_default() += 1;
                if u64::from(id.index) % TICKS_PER_HOUR != slot {
                    steady.insert(id, false); // moved off its slot: only legal if it left the tier
                }
            }
            last.insert(id, now);
        }
    }
    assert!(max_due <= 2 * max_n.div_ceil(TICKS_PER_HOUR as usize), "{max_due} due in one tick of {max_n}");
    let steady: Vec<EntityId> = steady.iter().filter(|(_, &ok)| ok).map(|(&id, _)| id).collect();
    assert!(steady.len() > 50, "too few steady statistical agents: {}", steady.len());
    for id in steady {
        assert_eq!(changes.get(&id).copied().unwrap_or(0), 24, "{id}: hunger changed a wrong number of times");
    }
}

#[test]
fn test_due_this_tick_lists_the_slot() {
    let mut w = world(105);
    w.run_ticks(TICKS_PER_HOUR + 7);
    let due = lod::due_this_tick(&w);
    let slot = w.tick % TICKS_PER_HOUR;
    assert!(!due.is_empty(), "nobody due in slot {slot}");
    assert!(due.iter().all(|id| u64::from(id.index) % TICKS_PER_HOUR == slot));
    assert!(due.iter().all(|id| w.comp::<Brain>(*id).is_some_and(|b| b.lod == Lod::Statistical)));
    // The 60 buckets partition the tier.
    let mut all: Vec<EntityId> = (0..TICKS_PER_HOUR).flat_map(|t| w.stat_slots.at(t).to_vec()).collect();
    all.sort();
    assert_eq!(all, w.tier(Lod::Statistical).to_vec());
}

#[test]
fn test_indices_survive_jail_remove_promote_demote() {
    let mut w = world(107);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let stat: Vec<EntityId> = w.tier(Lod::Statistical).to_vec();
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let jailed = stat[0];
    let until = w.tick + TICKS_PER_DAY;
    law::sentence(&mut w, jailed, Crime::Theft, until, jail);
    w.check_indices().expect("after jail");
    assert!(!w.tier(Lod::Statistical).contains(&jailed));
    let gone = *stat[1..].iter().find(|&&id| !w.has::<Job>(id)).expect("a jobless statistical agent");
    w.remove_agent(gone);
    w.check_indices().expect("after remove");
    let up = *stat.iter().rev().find(|&&id| id != jailed && id != gone).expect("another");
    lod::set_lod(&mut w, up, Lod::Coarse);
    w.check_indices().expect("after promote");
    let down = *w.tier(Lod::Full).first().expect("a full agent");
    lod::set_lod(&mut w, down, Lod::Statistical);
    w.check_indices().expect("after demote");
    assert!(w.stat_slots.at(u64::from(down.index)).contains(&down));
    assert!(!w.stat_slots.at(u64::from(up.index)).contains(&up));
}

#[test]
fn test_index_rebuilt_on_load() {
    let mut w = world(106);
    w.run_ticks(TICKS_PER_HOUR + 10);
    let loaded = save::from_ron(&save::to_ron(&w)).expect("round trip");
    loaded.check_indices().expect("indices rebuilt");
    assert_eq!(loaded.by_tier, w.by_tier);
    assert_eq!(loaded.by_role, w.by_role);
}

// ---------------------------------------------------------------------------
// M10 phase 2: map v2 and the 2,000-resident world
// ---------------------------------------------------------------------------

fn kind_count(m: &Map, kind: BuildingKind) -> usize {
    m.count_kind(kind)
}

#[test]
fn test_map_v2_loads_with_zones() {
    let cfg = Config::load();
    let m = Map::load(&cfg.asset("map.txt"));
    assert_eq!((m.w(), m.h()), (256, 192));
    assert!(m.has_zones());
    for (kind, n) in [
        (BuildingKind::Home, 400),
        (BuildingKind::Farm, 12),
        (BuildingKind::Market, 3),
        (BuildingKind::Bar, 3),
        (BuildingKind::Jail, 1),
        (BuildingKind::Hall, 1),
        (BuildingKind::Cemetery, 1),
        (BuildingKind::Warehouse, 1),
        (BuildingKind::SecurityOffice, 2),
        (BuildingKind::Hideout, 2),
        (BuildingKind::Lot, 60),
    ] {
        assert_eq!(kind_count(&m, kind), n, "{kind}");
    }
    let mut tiers = [0usize; 3];
    for b in m.buildings.iter().filter(|b| b.kind == BuildingKind::Home) {
        tiers[usize::from(b.tier)] += 1;
        let expected = match b.tier {
            2 => Zone::Spire,
            1 => Zone::Mid,
            _ => Zone::Sump,
        };
        assert_eq!(m.zone(b.door), expected, "Home at {} has tier {}", b.door, b.tier);
    }
    assert_eq!(tiers, [200, 140, 60], "Home tiers 0/1/2");
    for zone in Zone::ALL {
        let lots = m.buildings.iter().filter(|b| b.kind == BuildingKind::Lot && m.zone(b.door) == zone).count();
        assert!(lots >= 8, "{zone}: {lots} Lots");
    }
    // Lots have no walls: an open plot with one door.
    let lot = m.buildings.iter().find(|b| b.kind == BuildingKind::Lot).expect("a Lot");
    assert_eq!(m.tile_at(TilePos { x: lot.rect.x, y: lot.rect.y }), TileKind::Ground);
}

#[test]
fn test_map_v1_loads_without_zones() {
    let cfg = Config::load();
    let m = Map::load(&cfg.asset("map_v1.txt"));
    assert_eq!((m.w(), m.h()), (96, 64));
    assert!(!m.has_zones());
    for y in (0..64).step_by(7) {
        for x in (0..96).step_by(5) {
            assert_eq!(m.zone(TilePos { x, y }), Zone::Mid);
        }
    }
    assert!(m.buildings.iter().all(|b| b.tier == 1));
    assert_eq!(kind_count(&m, BuildingKind::Home), 60);
}

#[test]
fn test_nearest_market_per_agent() {
    let mut w = World::new(42, Config::load());
    let markets = w.buildings_of_kind(BuildingKind::Market).to_vec();
    assert_eq!(markets.len(), 3);
    // Two jobless adults (the employer rule does not apply), each put outside a different Market.
    let mut picked = w.citizens().into_iter().filter(|&id| !w.has::<Job>(id) && demography::is_adult(&w, id)).take(2);
    let (a, b) = (picked.next().expect("a"), picked.next().expect("b"));
    w.stand_at_door(a, markets[0]);
    w.stand_at_door(b, markets[2]);
    for (agent, expect) in [(a, markets[0]), (b, markets[2])] {
        let tile = w.comp::<Position>(agent).expect("pos").tile;
        let got = w.local(agent, BuildingKind::Market).expect("a market");
        assert_eq!(got, expect);
        let nearest =
            markets.iter().map(|&m| (w.comp::<Building>(m).expect("b").door.manhattan(tile), m)).min().map(|(_, m)| m);
        assert_eq!(Some(got), nearest, "local is the Manhattan-nearest");
    }
    assert_ne!(w.local(a, BuildingKind::Market), w.local(b, BuildingKind::Market));
    // A clerk uses their own Market wherever they stand.
    let clerk = w.workers(Role::Clerk)[0];
    let employer = w.comp::<Job>(clerk).and_then(|j| j.employer).expect("employer");
    let other = *markets.iter().find(|&&m| m != employer).expect("another market");
    w.stand_at_door(clerk, other);
    assert_eq!(w.local(clerk, BuildingKind::Market), Some(employer));
}

#[test]
fn test_flow_field_lazy_lru_and_wall_change() {
    let mut cfg = Config::load().v1_profile();
    cfg.exec.flow_field_cache = 2;
    let mut w = World::new(7, cfg);
    assert!(w.flow_fields.is_empty(), "fields are built lazily");
    let homes = w.buildings_of_kind(BuildingKind::Home).to_vec();
    let from = w.comp::<Building>(homes[10]).expect("b").door;
    let (b1, b2, b3) = (homes[0], homes[1], homes[2]);
    w.flow_step(b1, from);
    assert_eq!(w.flow_fields.len(), 1);
    assert!(w.flow_fields.contains(b1));
    w.flow_step(b2, from);
    w.flow_step(b1, from); // b1 is now the most recent
    assert_eq!(w.flow_fields.len(), 2);
    w.flow_step(b3, from);
    assert_eq!(w.flow_fields.len(), 2, "the cap holds");
    assert!(w.flow_fields.contains(b1) && w.flow_fields.contains(b3));
    assert!(!w.flow_fields.contains(b2), "the least recently used was evicted");
    w.invalidate_flow_fields();
    assert!(w.flow_fields.is_empty(), "a wall change clears the cache");
}

#[test]
fn test_two_thousand_spawn_housed() {
    let w = World::new(42, Config::load());
    assert_eq!(w.population(), 2000);
    let homeless =
        w.citizens().into_iter().filter(|&id| w.comp::<Household>(id).is_none_or(|h| h.home.is_none())).count();
    // M12 D26: the seeded derelicts' residents start homeless, nobody else.
    assert_eq!(homeless, 5 * w.config.street.seed_derelict_blocks);
    for (role, n) in
        [(Role::Farmer, 160), (Role::Guard, 36), (Role::Clerk, 12), (Role::Bartender, 8), (Role::Gravedigger, 4)]
    {
        assert_eq!(w.workers(role).len(), n, "{role}");
    }
    for &m in w.buildings_of_kind(BuildingKind::Market) {
        let clerks = w.workers(Role::Clerk).iter().filter(|&&c| w.comp::<Job>(c).and_then(|j| j.employer) == Some(m));
        assert_eq!(clerks.count(), 4, "clerks at Market#{}", m.index);
    }
}

/// M10 D39: the median single tick of the 2,000-resident city is under 250 µs.
/// Release only (a debug build is an order of magnitude slower); `#[ignore]`
/// because it times the machine. Run with `--release -- --ignored`.
#[test]
#[ignore]
fn test_tick_2000_median_under_250us() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(TICKS_PER_DAY);
    let mut times: Vec<u128> = (0..2000)
        .map(|_| {
            let start = std::time::Instant::now();
            w.tick();
            start.elapsed().as_nanos()
        })
        .collect();
    times.sort_unstable();
    let median_us = times[times.len() / 2] as f64 / 1000.0;
    let p99_us = times[times.len() * 99 / 100] as f64 / 1000.0;
    eprintln!("tick median {median_us:.1} us, p99 {p99_us:.1} us over 2000 ticks after day 1");
    if !cfg!(debug_assertions) {
        assert!(median_us <= 250.0, "median tick {median_us:.1} us > 250 us");
    }
}
