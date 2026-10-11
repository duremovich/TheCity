//! P10 (docs/P10_POLICY.md): the city's policy, funded from the Treasury.
//! The policy is saved state on `World::levers` (the levy, the dole, public
//! works, school meals, the Reserve release), seeded from `[policy]` and
//! moved by levers. The dole and public works are paid in coins from the
//! Treasury; school meals and the release draw on the Reserve's stock, in
//! kind, and pay short when it is empty.
//!
//! A Treasury that cannot pay: a dole visit is refused while the Treasury is
//! below 0 (`economy::collect_dole`, the rule since M1), and the city's wages
//! pay short (`economy::collect_wage`). A day on which a dole was refused,
//! or that ends with the Treasury below 0, is a dry day; after
//! `[policy] lapse_days` of them in a row the dole and public works lapse
//! (`PolicyLapsed`): the council cannot fund them and they stay off until
//! enacted again.

use crate::events::EventKind;
use crate::world::World;

/// The coin-paid net is enacted (the dole or public works).
pub fn net_enacted(world: &World) -> bool {
    world.levers.dole_per_day > 0 || world.levers.public_works
}

/// The midnight pass (`living::run`, before the band reads the lever).
pub fn daily(world: &mut World) {
    let short = std::mem::take(&mut world.budget.short_today);
    let lapse = world.config.policy.lapse_days;
    if lapse == 0 || !net_enacted(world) {
        world.budget.dry_days = 0;
        return;
    }
    let t = world.treasury().map_or(0, |x| x.coins);
    if short || t < 0 {
        world.budget.dry_days = world.budget.dry_days.saturating_add(1);
    } else {
        world.budget.dry_days = 0;
    }
    if world.budget.dry_days < lapse {
        return;
    }
    let dole = world.levers.dole_per_day;
    world.levers.dole_per_day = 0;
    world.levers.public_works = false;
    world.budget.dry_days = 0;
    world.push_event(
        EventKind::PolicyLapsed,
        &[],
        format!("the city could not pay its safety net for {lapse} days (Treasury {t}): the dole ({dole} a day) and public works lapsed"),
    );
}
