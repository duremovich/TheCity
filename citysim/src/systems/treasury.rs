//! The Real economy phase 3a (docs/ECONOMY_V2.md §§ 2, 5; plan E21, E33):
//! the Treasury on taxes and the Recycler's till.
//!
//! With `econ::no_net` the Treasury has no dole to pay and no public works to
//! post: its inflows are the tax, customs, the property rate and fines; its
//! spending the city's own roles, the Jail's meal, the Reserve and the city
//! Camp's food. `daily` replaces L2's `budget::daily` and moves
//! `levers.tax_rate` a `tax_step` against `[treasury] band` with L2's
//! hysteresis (`[budget] band_hold_days`, the state in `CityBudget.{band_side,
//! band_days}`): above `hi` held, the rate falls; below `lo` held, it rises;
//! clamped `[tax_min, tax_max]`. A `SetTaxRate` pins it (`EconState.tax_pinned`)
//! until `SetTaxAuto`.
//!
//! The till (E33) is the Recycler's purse in all but name (the Recycler is
//! city-owned and has no `Corp`): its Parts sales (to the city's buyers in
//! `assets::parts_market`, to the World in `world_market::daily`) are moved
//! from the Treasury into `EconState.recycler_till`, and a scavenger's find is
//! paid from it alone (`exec::actions::scavenge`). Everything here is integer
//! purses and counters.

use crate::events::EventKind;
use crate::world::World;

/// E21: the band runs with `no_net` and `[treasury] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::econ::no_net(world)
}

/// E21: the midnight pass (in `living::run` in `budget::daily`'s slot).
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.treasury.clone();
    let t = world.treasury().map_or(0, |x| x.coins);
    let side: i8 = if t > cfg.band[1] {
        1
    } else if t < cfg.band[0] {
        -1
    } else {
        0
    };
    if side != 0 && side == world.budget.band_side {
        world.budget.band_days = world.budget.band_days.saturating_add(1);
    } else {
        world.budget.band_side = side;
        world.budget.band_days = u16::from(side != 0);
    }
    if world.econ.tax_pinned || side == 0 {
        return;
    }
    // L2's hysteresis on every step: a step fires on the hold's multiples.
    let hold = world.config.budget.band_hold_days.max(1);
    let held = world.budget.band_days >= hold && world.budget.band_days.is_multiple_of(hold);
    if !held {
        return;
    }
    let old = world.levers.tax_rate;
    let step = if side == 1 { -cfg.tax_step } else { cfg.tax_step };
    let new = ((old + step).clamp(cfg.tax_min, cfg.tax_max) * 1000.0).round() / 1000.0;
    if (new - old).abs() < 1e-6 {
        return;
    }
    world.levers.tax_rate = new;
    let why = if side == 1 { "above" } else { "below" };
    world.push_event(
        EventKind::TaxMoved,
        &[],
        format!(
            "the city moved the tax rate {old:.2} -> {new:.2} (Treasury {t}, {why} the band [{}, {}])",
            cfg.band[0], cfg.band[1]
        ),
    );
}

/// E33: the till holds the Recycler's sales (with `no_net`).
pub fn till_on(world: &World) -> bool {
    crate::systems::econ::no_net(world)
}

/// E33: coins the Recycler's Parts sale brought into the Treasury move into
/// the till (a purse move: `total_coins` counts both). With `no_net` only.
pub fn to_till(world: &mut World, coins: i64) {
    if coins <= 0 || !till_on(world) {
        return;
    }
    world.purse_add(None, -coins);
    world.econ.recycler_till += coins;
}
