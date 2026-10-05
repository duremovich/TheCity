//! Same seed, same bytes.

use citysim::{save, Config, World};

fn hash_after(seed: u64, ticks: u64) -> blake3::Hash {
    let mut w = World::new(seed, Config::load().v1_profile());
    w.run_ticks(ticks);
    blake3::hash(save::to_ron(&w).as_bytes())
}

#[test]
fn test_determinism_same_seed_same_hash() {
    assert_eq!(hash_after(42, 2000), hash_after(42, 2000));
}

#[test]
fn test_different_seed_different_hash() {
    assert_ne!(hash_after(42, 2000), hash_after(43, 2000));
}
