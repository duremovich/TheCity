//! Life pass L2 (plan L36): the living city's slot in the tick order,
//! `…, gang, corp_brain, living, demography, stats`. Every L2 daily pass
//! runs here (phase 1: deferred seeding, `jobs::daily`, `jobs::top_up`,
//! `budget::daily`, `outside::export_daily`); `demography` after it hires
//! today's vacancies. With `[living] enabled = false` it returns at once.

use crate::world::World;

pub fn run(world: &mut World) {
    if !world.config.living.enabled {
        return;
    }
    // L2 phase 2 (L17, L18, L20): the hourly leisure passes (the bout, the
    // 21:00 Statistical evening, the off-screen leaders, street density).
    crate::systems::leisure::hourly(world);
    if world.tick_of_day() != 0 {
        return;
    }
    // Plan L7: an older save's venues, at the first midnight with Lots.
    // Review fix: `venues_due` holds until a venue stands (no Lot at this
    // midnight: the next one retries).
    if world.venues_due && crate::systems::jobs::on(world) && !crate::systems::jobs::seed_venues(world).is_empty() {
        world.venues_due = false;
        crate::systems::virt::relink(world);
    }
    crate::systems::jobs::daily(world);
    crate::systems::jobs::top_up(world);
    crate::systems::budget::daily(world);
    // L2 shadow fixes: no-shows, the sweepers' wage, the told-memory.
    crate::systems::fixes::daily(world);
    crate::systems::outside::export_daily(world);
    // Real economy (plan E6-E10): the World's book, appetite, refill and the
    // midnight Parts and Data sales (`export_daily` returns at once with it on).
    crate::systems::world_market::daily(world);
    // L2 phase 2: the spots, the wealth decile, the venues' hand-over.
    crate::systems::leisure::daily(world);
}
