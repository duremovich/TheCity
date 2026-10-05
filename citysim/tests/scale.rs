//! M10 phase 1: tier and role indices, and the spread Statistical tick.

use std::collections::BTreeMap;

use citysim::systems::{demography, lod};
use citysim::{
    save, Brain, BuildingKind, Config, DeathCause, EntityId, Job, Lod, Role, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
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
    let mut w = world(104);
    w.run_ticks(TICKS_PER_HOUR);
    // Tick 60 has been assigned at the start of its own tick; step into the window.
    let start = w.tick;
    assert_eq!(start % TICKS_PER_HOUR, 0);
    let always_stat: Vec<EntityId> = w.tier(Lod::Statistical).to_vec();
    let mut still: BTreeMap<EntityId, bool> = always_stat.iter().map(|&id| (id, true)).collect();
    let mut runs: BTreeMap<EntityId, u32> = BTreeMap::new();
    let mut max_per_tick = 0usize;
    let mut max_n = 0usize;
    while w.tick < start + TICKS_PER_DAY {
        // `World::tick` assigns, then runs the due agents: read what it will run.
        w.tick();
        // The tick counter has advanced; the run used `tick - 1`.
        let slot = (w.tick - 1) % TICKS_PER_HOUR;
        for (&id, ok) in still.iter_mut() {
            if w.comp::<Brain>(id).is_none_or(|b| b.lod != Lod::Statistical) {
                *ok = false;
            }
        }
        let n = w.tier(Lod::Statistical).len();
        max_n = max_n.max(n);
        let due = w.tier(Lod::Statistical).iter().filter(|id| u64::from(id.index) % TICKS_PER_HOUR == slot).count();
        max_per_tick = max_per_tick.max(due);
        for &id in &always_stat {
            if u64::from(id.index) % TICKS_PER_HOUR == slot && *still.get(&id).unwrap_or(&false) {
                *runs.entry(id).or_default() += 1;
            }
        }
    }
    assert!(max_per_tick <= 2 * max_n.div_ceil(TICKS_PER_HOUR as usize), "{max_per_tick} due in one tick of {max_n}");
    let steady: Vec<EntityId> = still.iter().filter(|(_, &ok)| ok).map(|(&id, _)| id).collect();
    assert!(steady.len() > 50, "too few steady statistical agents: {}", steady.len());
    for id in steady {
        assert_eq!(runs.get(&id).copied().unwrap_or(0), 24, "{id} ran a wrong number of times");
    }
}

#[test]
fn test_due_this_tick_lists_the_slot() {
    let mut w = world(105);
    w.run_ticks(TICKS_PER_HOUR + 7);
    let due = lod::due_this_tick(&w);
    let slot = w.tick % TICKS_PER_HOUR;
    assert!(due.iter().all(|id| u64::from(id.index) % TICKS_PER_HOUR == slot));
    assert!(due.iter().all(|id| w.comp::<Brain>(*id).is_some_and(|b| b.lod == Lod::Statistical)));
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
