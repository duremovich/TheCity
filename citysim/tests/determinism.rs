//! Same seed, same bytes: the shipped city (`Config::load().dole_city()`, 2,000
//! residents) hashed at the first day boundary (the midnight passes have
//! run). The mid-day save that runs on identically is `save.rs`'s
//! `test_word_save_mid_day_bit_identical`. The perf pass phase 8: the same
//! at any `[perf] threads` (the think slice scored ahead on a pool).

use citysim::{save, Config, World, TICKS_PER_DAY};

fn hash_after(seed: u64, ticks: u64, threads: usize) -> blake3::Hash {
    let mut config = Config::load().dole_city();
    config.perf.threads = threads;
    let mut w = World::new(seed, config);
    // The perf pass phase 7b: through the last slot of a staggered midnight
    // (`ticks` ends one tick past a day boundary).
    let window = citysim::systems::midnight::window(&w);
    w.run_ticks(ticks + window - 1);
    blake3::hash(save::to_ron(&w).as_bytes())
}

#[test]
fn test_determinism_same_seed_same_hash() {
    let serial = hash_after(42, TICKS_PER_DAY + 1, 1);
    assert_eq!(serial, hash_after(42, TICKS_PER_DAY + 1, 1));
    // The pool's path runs here (a race or a panic in it shows); the proof
    // that it is the serial loop's city is the perf pass's thread-count
    // identity (2k 42-44 x 30, 20k x 30): an unguarded split (every score
    // taken at the top of the tick) moved the shipped city on day 1 but
    // passed here for five days.
    assert_eq!(serial, hash_after(42, TICKS_PER_DAY + 1, 4), "the think slice on 4 threads moved the city");
}

#[test]
fn test_different_seed_different_hash() {
    assert_ne!(hash_after(42, 600, 1), hash_after(43, 600, 1));
}
