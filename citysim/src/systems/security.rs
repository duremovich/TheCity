//! M13 D28: the one tier contest (docs/M13_ASSETS.md § 2 theft, § 5 the
//! robot's sensor). A thief against a lock, a hacker against a firewall
//! (M14), an intruder against a robot: `p = clamp(0.5 + contest_step ×
//! (attacker − defender), 0.05, 0.95)` on the caller's stream.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// The attacker's chance against the defender at `step` per tier (M14 V8:
/// over fractional tiers, `att = deck_eff + hack_w x hacking`; for integer
/// tiers `0.5 + 0.25 x k` is exact in f32, so M13's odds are unchanged).
pub fn contest_p(att: f32, def: f32, step: f32) -> f32 {
    (0.5 + step * (att - def)).clamp(0.05, 0.95)
}

/// M14 V8: does the attacker win? One draw from `rng`, M13's draw.
pub fn contest_f(att: f32, def: f32, step: f32, rng: &mut ChaCha8Rng) -> bool {
    let roll: f32 = rng.random();
    roll < contest_p(att, def, step)
}

/// A thief's tier from its stealth (`law::stealth`): `1 + round(2 ×
/// stealth)`, the attacker's side of a lock or a robot's sensor.
pub fn thief_tier(stealth: f32) -> u8 {
    (1.0 + (2.0 * stealth).round()).clamp(1.0, 255.0) as u8
}

/// Does the attacker win? One draw from `rng` ([`contest_f`] over the tiers).
pub fn contest(attacker: u8, defender: u8, step: f32, rng: &mut ChaCha8Rng) -> bool {
    contest_f(f32::from(attacker), f32::from(defender), step, rng)
}
