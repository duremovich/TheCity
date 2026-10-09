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
    // The Real economy E27: the Statistical kitchen pass at 12:00.
    crate::systems::charity::hourly(world);
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
    // Real economy phase 2 (plan E17, E18): the wage rule, then hiring and
    // layoffs on the margin, before the band and `demography`'s hires.
    crate::systems::wages::daily(world);
    crate::systems::wages::staff(world);
    // Real economy E21 (phase 3a): with `no_net` the Treasury's tax band
    // replaces L2's budget band (no works, no upkeep cut).
    if crate::systems::econ::no_net(world) {
        crate::systems::treasury::daily(world);
    } else {
        crate::systems::budget::daily(world);
    }
    // L2 shadow fixes: no-shows, the sweepers' wage, the told-memory.
    crate::systems::fixes::daily(world);
    crate::systems::outside::export_daily(world);
    // Real economy (plan E6-E10): the World's book, appetite, refill and the
    // midnight Parts and Data sales (`export_daily` returns at once with it on).
    crate::systems::world_market::daily(world);
    // L2 phase 2: the spots, the wealth decile, the venues' hand-over.
    crate::systems::leisure::daily(world);
    // The Real economy E27, E34, E37, E39, E40, E42: the kitchens' day, the
    // camps' day (before `demography` feeds the Blocks and takes), the
    // corps' camp founding.
    crate::systems::charity::daily(world);
    crate::systems::camp::daily(world);
    crate::systems::camp::found_daily(world);
}
