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
//!
//! Jobs v2 P5a (docs/JOBS_V2.md § 5; plan J16-J18): the civic budget. The
//! same midnight pass turns the Treasury's receipts into the city's police
//! and Sanitation headcounts: `civic_target` (a share of the trailing
//! receipts plus a payout of the hoard above `band[1]`, nothing at or under
//! `band[0]`), `civic_allocation` (police first up to `police_per_1000`, then
//! Sanitation up to `sanitation_max`, each at its wage), written to
//! `levers.{guard_count, sanitation_count}` at most `civic_pace` lower a day;
//! `law::reconcile_guards` and `districts::reconcile_sanitation` hire toward
//! them unchanged. A god's `SetGuardCount`/`SetSanitation` pins both
//! (`EconState.civic_pinned`) until `SetCivicAuto`. J18: the band reads the
//! Treasury after the day's civic wages (paid at shift end, before this
//! midnight pass).

use crate::components::Role;
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
    // J16: the civic budget (it moves no coins: the wages are paid at shift end).
    civic_daily(world);
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

// ---------------------------------------------------------------------------
// Jobs v2 P5a (plan J16, J17): the civic budget.
// ---------------------------------------------------------------------------

/// J16: yesterday's receipts into `EconState.receipts` (`stats::run` rolls
/// the row at 23:59; this pass runs at 00:00, so `history.back()` is
/// yesterday), then the headcounts unless a god pinned them. The first
/// midnight has no receipts yet and holds the configured headcounts.
pub fn civic_daily(world: &mut World) {
    if !on(world) {
        return;
    }
    if let Some(row) = world.stats.history.back() {
        let r = row.flow_tax + row.jobs.flow_fines + row.econ.flow_customs + row.econ.flow_property;
        let days = world.config.treasury.receipts_days.max(1) as usize;
        let ring = &mut world.econ.receipts;
        ring.push_back(r);
        while ring.len() > days {
            ring.pop_front();
        }
    }
    if world.econ.civic_pinned || world.econ.receipts.is_empty() {
        return;
    }
    let target = civic_target(world);
    let (guards, sweepers) = civic_allocation(world, target);
    let pace = world.config.treasury.civic_pace;
    world.levers.guard_count = paced(world.levers.guard_count, guards, pace);
    world.levers.sanitation_count = paced(world.levers.sanitation_count, sweepers, pace);
}

/// J16: the day's civic payroll target in coins: `civic_share` x the mean
/// receipts in the ring + `max(0, Treasury − band[1]) ÷ payout_days`; 0 at or
/// under `band[0]` (the budget is spent only above it).
/// Under `band[0]` the target is 0 but the headcounts fall only `civic_pace`
/// a day, so the posts still standing are paid from a low Treasury for a few
/// days and some go short (`collect_wage`'s `days_unpaid` path): expected,
/// not a leak.
pub fn civic_target(world: &World) -> i64 {
    let cfg = &world.config.treasury;
    let t = world.treasury().map_or(0, |x| x.coins);
    if t <= cfg.band[0] {
        return 0;
    }
    let ring = &world.econ.receipts;
    let mean = if ring.is_empty() { 0.0 } else { ring.iter().sum::<i64>() as f64 / ring.len() as f64 };
    let share = (f64::from(cfg.civic_share) * mean).round().max(0.0) as i64;
    let hoard = (t - cfg.band[1]).max(0) / i64::from(cfg.payout_days.max(1));
    share + hoard
}

/// J16: a Sanitation post's daily cost: `wage_sanitation`, raised to the
/// works wage the city pays every sweeper (`fixes::daily`, item 14).
pub fn sweeper_wage(world: &World) -> i64 {
    world.config.economy.wage(Role::Sanitation).max(world.config.budget.works_wage).max(1)
}

/// J16: `target` coins a day spent in priority order: police (`wage_guard`
/// each) up to `police_ceiling`, then Sanitation up to `sanitation_max`.
/// Headcounts, before the pace.
pub fn civic_allocation(world: &World, target: i64) -> (u8, u8) {
    let cfg = &world.config.treasury;
    let target = target.max(0);
    let police_cap = police_ceiling(world);
    let gw = world.config.economy.wage(Role::Guard).max(1);
    let guards = (target / gw).min(police_cap).clamp(0, i64::from(u8::MAX));
    let left = target - guards * gw;
    let sweepers = (left / sweeper_wage(world)).min(i64::from(cfg.sanitation_max)).clamp(0, i64::from(u8::MAX));
    (guards as u8, sweepers as u8)
}

/// J16: the police ceiling, `police_per_1000` x population ÷ 1,000, capped
/// by the Jail's room (plan risk 8, P5a's "Done when": police growth pinned
/// the Jail at capacity, and a full Jail fines the Vagrancy and Theft it
/// cannot hold, so fines paid for more police) at `police_per_jail_bed` x
/// the Jail's capacity. Static on purpose: a cap read from today's held
/// count swung the police, and the sweepers behind them, by dozens a week.
pub fn police_ceiling(world: &World) -> i64 {
    let cfg = &world.config.treasury;
    let by_pop = (f64::from(cfg.police_per_1000) * world.population() as f64 / 1000.0).round() as i64;
    let by_jail = (f64::from(cfg.police_per_jail_bed) * f64::from(world.config.buildings.jail.capacity)).round() as i64;
    by_pop.min(by_jail)
}

/// J16: a headcount moves up at once (the reconcilers hire five a day) and
/// down at most `pace` a day.
pub fn paced(current: u8, want: u8, pace: u8) -> u8 {
    want.max(current.saturating_sub(pace))
}
