//! Shared builders for the integration tests (`mod common;` at the top of a
//! test file). One copy of what 36 files used to carry each (`.claude/rules/tests.md`).
//!
//! `world()` is the shipped seed-42 city (what the behaviour tests, probes and
//! god scenarios run); `dole_world()` is the pre-flip dole city most unit tests
//! pin so they keep testing the mechanism they were written for.
#![allow(dead_code)]

use citysim::systems::{demography, law};
use citysim::time::TICKS_PER_DAY;
use citysim::{Config, Corp, EntityId, Wallet, World};

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

/// Advance whole days.
pub fn run_days(w: &mut World, days: u64) {
    w.run_ticks(days * TICKS_PER_DAY);
}

/// Every living adult.
pub fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
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
