//! Shared builders for the integration tests (`mod common;` at the top of a
//! test file). One copy of what 36 files used to carry each (`.claude/rules/tests.md`).
//!
//! `world()` is the shipped seed-42 city (what the behaviour tests, probes and
//! god scenarios run); `dole_world()` is the pre-flip dole city most unit tests
//! pin so they keep testing the mechanism they were written for.
#![allow(dead_code)]

use citysim::systems::{contracts, demography, law, life, missions};
use citysim::time::TICKS_PER_DAY;
use citysim::{Brain, Config, Corp, EntityId, Job, Personality, Position, Sentence, Skills, Wallet, World};

/// The shipped city, seed 42.
pub fn world() -> World {
    World::new(42, Config::load())
}

/// The shipped city on another seed.
pub fn world_seed(seed: u64) -> World {
    World::new(seed, Config::load())
}

/// The old default: the dole city (`Config::dole_city`), seed 42.
pub fn dole_world() -> World {
    World::new(42, Config::load().dole_city())
}

/// The pressure pass R2 (`[exec] move_ticks_full` 2 -> 1): a scene whose
/// pick or premise the halved walk moved (another body picked, a far
/// worker now near) is pinned to the v1 pace it was written for. Round 9
/// Y4: one tile every two ticks (`walk_tiles_per_tick` 1) at the M13
/// quarter-tick resolution (`step_res` 4).
#[allow(dead_code)]
pub fn v1_pace(cfg: &mut Config) {
    cfg.exec.move_ticks_full = 2;
    cfg.exec.walk_tiles_per_tick = 1;
    cfg.exec.step_res = 4;
}

/// Round 9 Y4 (`[exec] walk_tiles_per_tick` 1 -> 21): a scene whose pick
/// or premise the real walking pace moved is pinned to the R2 pace it was
/// written for (one tile a tick, the quarter-tick resolution).
#[allow(dead_code)]
pub fn r2_pace(cfg: &mut Config) {
    cfg.exec.move_ticks_full = 1;
    cfg.exec.walk_tiles_per_tick = 1;
    cfg.exec.step_res = 4;
}

/// Round 9 Y4: the scene was written on the 2k toy map (256 x 192 tiles, a
/// kilometre across) at R2's pace, where a walk was long enough for the El
/// to beat it, for a leg to reach the segment length, for a commute to be
/// far: at the real pace (21 tiles a minute) every leg there is minutes and
/// the premise is gone. The scene runs at R2's pace with the knobs Y4 moved
/// for it ([exec], [vehicles], [transit], the walk-written tile caps).
pub fn r2(mut cfg: Config) -> Config {
    r2_pace(&mut cfg);
    cfg.vehicles.max_steps_per_tick = 4;
    cfg.vehicles.flyer_ticks_per_tile = 0.3;
    cfg.transit.ride_ticks_per_tile = 0.25;
    cfg.transit.dwell_ticks = 1.0;
    cfg.transit.stop_spacing = 16;
    // The view milestone V9: the scenes' walk-or-ride was minute for minute
    // (at R2's pace a foot weight of 2 puts the El in every commute).
    cfg.transit.foot_weight = 1.0;
    cfg.gangs.raid_gather_hours = 8;
    cfg.life.muster_walk_cap_ticks = 480;
    cfg.life.rough_min_tiles = 45;
    cfg.life.bed_margin_tiles = 20;
    cfg.corps.shop_price_tiles = 12;
    cfg.jobs2.cab_min_tiles = 60;
    if let Some(c) = cfg.customers.as_mut() {
        c.hail_min_tiles = 50;
    }
    if let Some(h) = cfg.shift.as_mut().and_then(|s| s.haul.as_mut()) {
        h.haul_reach = 30;
    }
    if let Some(p) = cfg.presence.as_mut() {
        p.round_reach_tiles = 20;
    }
    if let Some(l) = cfg.legs.as_mut() {
        l.leg_replan_tiles = 40;
        l.escort_walk_tiles = 40;
    }
    cfg
}

/// [`world`] at R2's pace ([`r2`]).
pub fn world_r2() -> World {
    World::new(42, r2(Config::load()))
}

/// [`world_with`] at R2's pace ([`r2`]).
pub fn world_with_r2(edit: impl FnOnce(&mut Config)) -> World {
    let mut cfg = r2(Config::load());
    edit(&mut cfg);
    World::new(42, cfg)
}

/// The shipped config with one edit applied before the world is built.
pub fn world_with(edit: impl FnOnce(&mut Config)) -> World {
    let mut cfg = Config::load();
    edit(&mut cfg);
    World::new(42, cfg)
}

/// The shipped city run `ticks` on (the M17 review's city is
/// `world_at(4 * TICKS_PER_DAY + 30)`).
pub fn world_at(ticks: u64) -> World {
    let mut w = world();
    w.run_ticks(ticks);
    w
}

/// Advance whole days.
pub fn run_days(w: &mut World, days: u64) {
    w.run_ticks(days * TICKS_PER_DAY);
}

/// Every living adult.
pub fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
}

/// Free adults outside a gang (living, not sentenced), ascending.
pub fn free_adults(w: &World) -> Vec<EntityId> {
    adults(w).into_iter().filter(|&a| !w.has::<Sentence>(a) && w.gang_of(a).is_none()).collect()
}

/// Free civilian adults (no gang, no guard's job, not sentenced), ascending.
pub fn civilians(w: &World) -> Vec<EntityId> {
    adults(w).into_iter().filter(|&a| w.gang_of(a).is_none() && !law::is_guard(w, a) && !w.has::<Sentence>(a)).collect()
}

/// Jobless free adults with a home (no sentence, no exec's desk, not
/// emigrating), ascending.
pub fn jobless_housed(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            law::living(w, a)
                && demography::is_adult(w, a)
                && !w.has::<Job>(a)
                && !w.has::<Sentence>(a)
                && life::exec_corp(w, a).is_none()
                && missions::home_of(w, a).is_some()
                && w.comp::<Brain>(a).is_some_and(|b| !b.emigrating)
        })
        .collect()
}

/// Day workers (every shift from 09:00 on), free and housed, ascending.
pub fn day_workers(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            law::living(w, a)
                && !w.has::<Sentence>(a)
                && missions::home_of(w, a).is_some()
                && w.comp::<Job>(a).is_some_and(|j| j.employer.is_some() && j.shifts.iter().all(|&(s, _)| s >= 540))
        })
        .collect()
}

/// `n` free jobless adults outside any gang and not holding a body, with
/// `fighting` as their fighting skill and courage.
pub fn free_crew(w: &mut World, n: usize, fighting: f32) -> Vec<EntityId> {
    let c: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&id| {
            contracts::free_adult(w, id)
                && !w.has::<Job>(id)
                && !w.has::<citysim::GangMember>(id)
                && !contracts::holds_body(w, id)
        })
        .take(n)
        .collect();
    for &a in &c {
        w.comp_mut::<Skills>(a).expect("skills").melee = fighting;
        w.comp_mut::<Personality>(a).expect("p").courage = fighting;
    }
    c
}

/// `b` stands where `a` stands.
pub fn beside(w: &mut World, a: EntityId, b: EntityId) {
    let p = w.comp::<Position>(a).cloned().expect("pos");
    *w.comp_mut::<Position>(b).expect("pos") = p;
}

/// An agent's coins (0 without a wallet).
pub fn coins(w: &World, id: EntityId) -> i64 {
    w.comp::<Wallet>(id).map_or(0, |x| x.coins)
}

/// Set an agent's coins (the agent must have a wallet).
pub fn set_coins(w: &mut World, id: EntityId, coins: i64) {
    w.comp_mut::<Wallet>(id).expect("wallet").coins = coins;
}

/// A seeded corp by its config name.
pub fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps()
        .into_iter()
        .find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == name))
        .unwrap_or_else(|| panic!("no corp named {name}"))
}
