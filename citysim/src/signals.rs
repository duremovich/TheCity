//! The view milestone V7 (docs/VIEW_MILESTONE.md § V7, OPEN-260): the
//! junctions' signals. A signal is a pure function of the time and its
//! junction's tile (no state, nothing saved): every junction runs one cycle
//! of `[crossings] cycle_s`, offset by a hash of its tile, the walk phase
//! first (`walk_share` of the cycle: the walkers cross the main road, its
//! traffic held, the side street's moving), then don't-walk (the main road
//! moving). The sim's Full walker waits at a crossing's kerb for the walk
//! phase ([`wait_ms`], at its sub-tick time); the path search and the timed
//! walks use the expected wait ([`expected_wait_s`]); the view draws the
//! heads from the same function at its real-time lerp. Ground vehicles stop
//! at red later (OPEN-275): [`vehicle_go`] is their hook.
//!
//! Time is milliseconds of game time from tick 0 (a tick a minute: `tick ×
//! 60_000` plus the sub-tick).

use crate::components::TilePos;
use crate::config::CrossingsCfg;

/// Milliseconds a tick (a game minute).
pub const MS_PER_TICK: u64 = 60_000;

/// The cycle in ms (at least 1 s).
pub fn cycle_ms(cfg: &CrossingsCfg) -> u64 {
    ((f64::from(cfg.cycle_s) * 1000.0).round() as u64).max(1000)
}

/// The walk phase in ms (within the cycle).
pub fn walk_ms(cfg: &CrossingsCfg) -> u64 {
    let c = cycle_ms(cfg);
    ((c as f64 * f64::from(cfg.walk_share.clamp(0.0, 1.0))).round() as u64).min(c)
}

/// The junction's offset into the cycle: a fixed hash of its tile.
pub fn offset_ms(junction: TilePos, cycle: u64) -> u64 {
    let mut z = (u64::from(junction.x) << 16 | u64::from(junction.y)).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) % cycle.max(1)
}

/// Where the junction's cycle stands at `t_ms`: ms into it.
pub fn phase_ms(cfg: &CrossingsCfg, junction: TilePos, t_ms: u64) -> u64 {
    let c = cycle_ms(cfg);
    (t_ms % c + offset_ms(junction, c)) % c
}

/// The walk phase at `t_ms`?
pub fn walk(cfg: &CrossingsCfg, junction: TilePos, t_ms: u64) -> bool {
    phase_ms(cfg, junction, t_ms) < walk_ms(cfg)
}

/// The ms a walker reaching the kerb at `t_ms` waits for the walk phase (0
/// in it).
pub fn wait_ms(cfg: &CrossingsCfg, junction: TilePos, t_ms: u64) -> u64 {
    let p = phase_ms(cfg, junction, t_ms);
    if p < walk_ms(cfg) {
        0
    } else {
        cycle_ms(cfg) - p
    }
}

/// The ms left in the junction's current phase at `t_ms` (the view's
/// countdown and flash).
pub fn left_ms(cfg: &CrossingsCfg, junction: TilePos, t_ms: u64) -> u64 {
    let p = phase_ms(cfg, junction, t_ms);
    let w = walk_ms(cfg);
    if p < w {
        w - p
    } else {
        cycle_ms(cfg) - p
    }
}

/// The expected wait of a walker reaching the kerb at a random time, s:
/// don't-walk's share of the cycle times half of it.
pub fn expected_wait_s(cfg: &CrossingsCfg) -> f64 {
    let c = cycle_ms(cfg) as f64 / 1000.0;
    let red = 1.0 - walk_ms(cfg) as f64 / cycle_ms(cfg) as f64;
    red * red * c / 2.0
}

/// The expected wait in tenths of a walker's step (a Road step is 7) at
/// `step_s` seconds a step: the path search's price of a crossing.
pub fn wait_tenths(cfg: &CrossingsCfg, step_s: f64) -> u32 {
    if step_s <= 0.0 {
        return 0;
    }
    (expected_wait_s(cfg) / step_s * 7.0).round() as u32
}

/// A timed walk's expected signal waits over `tiles` Manhattan tiles, in
/// ticks (fractional): the crossings it is likely to make
/// (`timed_crossings_per_tile`) at the expected wait each.
pub fn timed_wait_ticks(cfg: &CrossingsCfg, tiles: u64) -> f64 {
    tiles as f64 * f64::from(cfg.timed_crossings_per_tile.max(0.0)) * expected_wait_s(cfg) / 60.0
}

/// OPEN-275's hook: may a ground vehicle on the junction's main road
/// (`main`) or its side street enter at `t_ms`? The main road moves in
/// don't-walk, the side street with the walk phase (its walkers cross the
/// main road beside it).
pub fn vehicle_go(cfg: &CrossingsCfg, junction: TilePos, main: bool, t_ms: u64) -> bool {
    walk(cfg, junction, t_ms) != main
}
