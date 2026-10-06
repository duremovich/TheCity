//! M13 D28: the one tier contest (docs/M13_ASSETS.md § 2 theft, § 5 the
//! robot's sensor). A thief against a lock, a hacker against a firewall
//! (M14), an intruder against a robot: `p = clamp(0.5 + contest_step ×
//! (attacker − defender), 0.05, 0.95)` on the caller's stream.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// The attacker's chance against the defender at `step` per tier.
pub fn contest_p(attacker: u8, defender: u8, step: f32) -> f32 {
    (0.5 + step * (f32::from(attacker) - f32::from(defender))).clamp(0.05, 0.95)
}

/// A thief's tier from its stealth (`law::stealth`): `1 + round(2 ×
/// stealth)`, the attacker's side of a lock or a robot's sensor.
pub fn thief_tier(stealth: f32) -> u8 {
    (1.0 + (2.0 * stealth).round()).clamp(1.0, 255.0) as u8
}

/// Does the attacker win? One draw from `rng`.
pub fn contest(attacker: u8, defender: u8, step: f32, rng: &mut ChaCha8Rng) -> bool {
    let roll: f32 = rng.random();
    roll < contest_p(attacker, defender, step)
}
