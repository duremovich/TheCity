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
        w.comp_mut::<Skills>(a).expect("skills").fighting = fighting;
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
