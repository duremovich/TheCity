//! The Real economy (docs/ECONOMY_V2.md, plan E1, E49): the switches, the
//! pure hashes every milestone decision draws on, and the conservation
//! identity. No path of the milestone runs without its `on()`.

use crate::config::Config;
use crate::rng::splitmix64;
use crate::world::World;

/// Plan E1: phase 1's switch: the World market (books, bids, asks, imports
/// to the World, migrants and the fence booked).
pub fn market_on_cfg(cfg: &Config) -> bool {
    cfg.living.enabled && cfg.economy2.enabled && cfg.economy2.market && cfg.world_market.enabled
}

pub fn market_on(world: &World) -> bool {
    market_on_cfg(&world.config)
}

/// Plan E1: phase 2's switch (wages from revenue). Nothing reads it in phase 1.
pub fn wages_on(world: &World) -> bool {
    world.config.living.enabled && world.config.economy2.enabled && world.config.economy2.wages
}

/// Plan E1: phase 3a's switch (no safety net). Nothing reads it in phase 1.
pub fn no_net(world: &World) -> bool {
    world.config.living.enabled && world.config.economy2.enabled && world.config.economy2.no_safety_net
}

/// Plan E49: a pure hash in `[0, 1)` of `(seed, purpose, a, b)` over
/// `splitmix64` (never a stream: no draw moves any other system).
pub fn hash_unit(seed: u64, purpose: u64, a: u64, b: u64) -> f32 {
    let x = mix(seed, purpose, a, b);
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// Plan E7: a bounded standard normal (variance 1, |z| ≤ 2√3) from four
/// hashed uniforms: `(Σ u − 2) × √3`.
pub fn hash_normal(seed: u64, purpose: u64, a: u64, b: u64) -> f32 {
    let x = mix(seed, purpose, a, b);
    let mut sum = 0.0f32;
    let mut y = x;
    for _ in 0..4 {
        y = splitmix64(y);
        sum += (y >> 40) as f32 / (1u64 << 24) as f32;
    }
    (sum - 2.0) * 3.0f32.sqrt()
}

fn mix(seed: u64, purpose: u64, a: u64, b: u64) -> u64 {
    let mut x = splitmix64(seed ^ 0xEC0_0000_0000_0000);
    x = splitmix64(x ^ purpose.rotate_left(32));
    x = splitmix64(x ^ a.rotate_left(17));
    splitmix64(x ^ b.rotate_left(41))
}

/// The conservation identity (plan "Money model"): `total_coins + Σ outside
/// treasuries − outside.minted`, constant to the coin with the market on.
pub fn identity(world: &World) -> i64 {
    crate::systems::ownership::total_coins(world) + world.outside.treasuries() - world.outside.minted
}

/// Plan E14 (phase 2, the last step of `World::new`): the opening hoard
/// becomes working capital: `[world] treasury_initial − [treasury]
/// treasury_initial` moves from the Treasury to the seeded corps pro rata
/// to their day-0 `corp_brain::wage_bill` (floor division, the remainder a
/// coin at a time to ascending ids), a plain purse move with a
/// `Flow::Subsidy` ledger line (capital: no corp reads it as trading);
/// `Corp.treasury_ref` rises by each share. `total_coins` is unchanged.
/// Returns the shares. Only with `wages_on`; never on a loaded save (E40).
pub fn seed_capital(world: &mut World) -> Vec<(crate::entity::EntityId, i64)> {
    if !wages_on(world) {
        return Vec::new();
    }
    let amount = world.config.world.treasury_initial - world.config.treasury.treasury_initial;
    if amount <= 0 {
        return Vec::new();
    }
    let bills: Vec<(crate::entity::EntityId, i64)> =
        world.corps().into_iter().map(|c| (c, crate::systems::corp_brain::wage_bill(world, c).max(0))).collect();
    let total: i64 = bills.iter().map(|&(_, b)| b).sum();
    if total <= 0 {
        return Vec::new();
    }
    let mut shares: Vec<(crate::entity::EntityId, i64)> =
        bills.iter().map(|&(c, b)| (c, ((i128::from(amount) * i128::from(b)) / i128::from(total)) as i64)).collect();
    let mut left = amount - shares.iter().map(|&(_, s)| s).sum::<i64>();
    let n = shares.len();
    let mut i = 0;
    while left > 0 && n > 0 {
        shares[i % n].1 += 1;
        left -= 1;
        i += 1;
    }
    for &(c, s) in &shares {
        if s <= 0 {
            continue;
        }
        world.purse_add(None, -s);
        world.purse_add(Some(c), s);
        if let Some(cc) = world.comp_mut::<crate::components::Corp>(c) {
            // A capital move, not a day of trading.
            cc.cashflow_today -= s;
            cc.treasury_ref += s;
            cc.closing += s;
        }
    }
    crate::systems::ownership::ledger_only(world, crate::systems::ownership::Flow::Subsidy, amount);
    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashes_are_pure_and_in_range() {
        for i in 0..2000u64 {
            let u = hash_unit(42, PURPOSE, i, 3);
            assert!((0.0..1.0).contains(&u));
            assert_eq!(u, hash_unit(42, PURPOSE, i, 3));
            let z = hash_normal(42, PURPOSE, i, 3);
            assert!(z.abs() <= 2.0 * 3.0f32.sqrt() + 1e-5);
        }
        assert_ne!(hash_unit(42, PURPOSE, 1, 0), hash_unit(43, PURPOSE, 1, 0));
        assert_ne!(hash_unit(42, PURPOSE, 1, 0), hash_unit(42, PURPOSE, 1, 1));
        // The normal's moments over a seed sweep.
        let n = 20_000u64;
        let (mut s, mut s2) = (0.0f64, 0.0f64);
        for i in 0..n {
            let z = f64::from(hash_normal(7, PURPOSE, i, 0));
            s += z;
            s2 += z * z;
        }
        let mean = s / n as f64;
        let var = s2 / n as f64 - mean * mean;
        assert!(mean.abs() < 0.03, "mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "variance {var}");
    }

    const PURPOSE: u64 = crate::econ::PURPOSE_APPETITE;
}
