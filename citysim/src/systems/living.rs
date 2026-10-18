//! Life pass L2 (plan L36): the living city's slot in the tick order,
//! `…, gang, corp_brain, living, demography, stats`. Every L2 daily pass
//! runs here (phase 1: deferred seeding, `jobs::daily`, `jobs::top_up`,
//! `budget::daily`, the World market); `demography` after it hires
//! today's vacancies.

use crate::world::World;

pub fn run(world: &mut World) {
    // L2 phase 2 (L17, L18, L20): the hourly leisure passes (the bout, the
    // 21:00 Statistical evening, the off-screen leaders, street density).
    crate::systems::leisure::hourly(world);
    // The Real economy E27: the Statistical kitchen pass at 12:00.
    crate::systems::charity::hourly(world);
    // Jobs & lives phase 1: the Yards' and plants' crews, the outages, the
    // sites that stand, the night's pilfer (nothing with `[jobs2]` off).
    crate::systems::industry::hourly(world);
    // Jobs & lives phase 2: the vendors' stock, the day posts' end, the
    // dawn pick (nothing with `[jobs2] street` off).
    crate::systems::streetwork::hourly(world);
    // Jobs & lives phase 3: the crates, the carry sightings (nothing with
    // `[weapons]` off).
    crate::systems::weapons::hourly(world);
    // Jobs & lives phase 4: the street's night (nothing with `[vice]` off).
    crate::systems::vice::hourly(world);
    // Lives at work phase 1 (§ 1.2): the service encounter's hourly pass.
    crate::prof::timed!("service", crate::systems::service::hourly(world));
    // Jobs & lives phase 5: an episode under a chip ends the contract
    // (nothing with `[vice] chip` off).
    crate::systems::dollhouse::hourly(world);
    if world.tick_of_day() != 0 {
        return;
    }
    // Plan L7: an older save's venues, at the first midnight with Lots.
    // Review fix: `venues_due` holds until a venue stands (no Lot at this
    // midnight: the next one retries).
    if world.venues_due && !crate::systems::jobs::seed_venues(world).is_empty() {
        world.venues_due = false;
        crate::systems::virt::relink(world);
    }
    crate::systems::jobs::daily(world);
    crate::systems::jobs::top_up(world);
    // Real economy phase 2 (plan E17, E18): the wage rule, then hiring and
    // layoffs on the margin, before the band and `demography`'s hires.
    crate::systems::wages::daily(world);
    crate::systems::wages::staff(world);
    // Class pay: the skilled promotions, before `demography`'s hires.
    crate::systems::classpay::daily(world);
    // Class pay P3: the rich spend on status (Spire nights, fashion, chrome).
    crate::systems::classpay::status_daily(world);
    // P10: the safety net lapses when the Treasury cannot pay it (before
    // the band reads the lever).
    crate::systems::policy::daily(world);
    // Real economy E21 (phase 3a): under the market wage law the Treasury's
    // tax band replaces L2's budget band (no upkeep levy to cut); public
    // works, when enacted, open and close on the tax band's reading (P10).
    if crate::systems::econ::wages_on(world) {
        crate::systems::treasury::daily(world);
        crate::systems::budget::market_works(world);
    } else {
        crate::systems::budget::daily(world);
    }
    // L2 shadow fixes: no-shows, the sweepers' wage, the told-memory.
    crate::systems::fixes::daily(world);
    // Real economy (plan E6-E10): the World's book, appetite, refill and the
    // midnight Parts and Data sales.
    crate::systems::world_market::daily(world);
    // Jobs & lives phase 1: the Rations sale (after the book's refill), the
    // civic posts, the offices and an Hq built (nothing with `[jobs2]` off).
    crate::systems::industry::daily(world);
    // Jobs & lives phase 2: the Pitches' day (the cut, sweeps, closings,
    // openings), the couriers' parcels (nothing with the street off).
    // Jobs & lives phase 4: the pimps named and the Brothels the law shuts,
    // before the street's day closes (nothing with `[vice]` off).
    crate::systems::vice::daily(world);
    crate::systems::streetwork::daily(world);
    // Jobs & lives phase 3: the armouries, the fence, the carry roll, the
    // wants and licences, the guards' stock (nothing with `[weapons]` off).
    crate::systems::weapons::daily(world);
    // Jobs & lives phase 4: status's decay, the gangs' clothing (nothing with `[fashion]` off).
    crate::systems::fashion::daily(world);
    // Jobs & lives phase 5: the chip contracts and the braindance day
    // (nothing with `[vice] chip` / `bd` off).
    crate::systems::dollhouse::daily(world);
    crate::systems::braindance::daily(world);
    // Jobs & lives phase 6: the gray market's and the faith's day (nothing
    // with `[gray]` / `[faith]` off).
    crate::systems::gray::daily(world);
    crate::systems::faith::daily(world);
    // Jobs & lives phase 7: the children's routines, the kin, the
    // prosthetics (nothing with `[ages]` off).
    crate::systems::ages::daily(world);
    // L2 phase 2: the spots, the wealth decile, the venues' hand-over.
    crate::systems::leisure::daily(world);
    // The Real economy E27, E34, E37, E39, E40, E42: the kitchens' day, the
    // camps' day (before `demography` feeds the Blocks and takes), the
    // corps' camp founding.
    crate::systems::charity::daily(world);
    crate::systems::camp::daily(world);
    crate::systems::camp::found_daily(world);
}
