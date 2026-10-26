//! Life pass L2 (plan L36): the living city's slot in the tick order,
//! `…, gang, corp_brain, living, demography, stats`. Every L2 daily pass
//! runs here (phase 1: deferred seeding, `jobs::daily`, `jobs::top_up`,
//! `budget::daily`, the World market); `demography` after it hires
//! today's vacancies.

use crate::world::World;

pub fn run(world: &mut World) {
    // L2 phase 2 (L17, L18, L20): the hourly leisure passes (the bout, the
    // 21:00 Statistical evening, the off-screen leaders, street density).
    crate::prof::timed!("living::leisure::hourly", crate::systems::leisure::hourly(world));
    // The pre-view pass phase 3 (OPEN-145): the counter's queued visits, each `counter_settle_window`.
    crate::prof::timed!("living::leisure::counter_due", crate::systems::leisure::counter_due(world));
    // The pre-view pass phase 4 (OPEN-119): the arms sales booked for a meeting.
    crate::prof::timed!("living::weapons::arms_meetings", crate::systems::weapons::arms_meetings(world));
    // The Real economy E27: the Statistical kitchen pass at 12:00.
    crate::prof::timed!("living::charity::hourly", crate::systems::charity::hourly(world));
    // Jobs & lives phase 1: the Yards' and plants' crews, the outages, the
    // sites that stand, the night's pilfer (nothing with `[jobs2]` off).
    crate::prof::timed!("living::industry::hourly", crate::systems::industry::hourly(world));
    // Jobs & lives phase 2: the vendors' stock, the day posts' end, the
    // dawn pick (nothing with `[jobs2] street` off).
    crate::prof::timed!("living::streetwork::hourly", crate::systems::streetwork::hourly(world));
    // Jobs & lives phase 3: the crates, the carry sightings (nothing with
    // `[weapons]` off).
    crate::prof::timed!("living::weapons::hourly", crate::systems::weapons::hourly(world));
    // Jobs & lives phase 4: the street's night (nothing with `[vice]` off).
    crate::prof::timed!("living::vice::hourly", crate::systems::vice::hourly(world));
    // The pressure pass phase 1 (OPEN-178): the miserable and drunk brawl
    // (nothing with `[misery]` absent).
    crate::prof::timed!("living::misery::hourly", crate::systems::misery::hourly(world));
    // Lives at work phase 1 (§ 1.2): the service encounter's hourly pass.
    crate::prof::timed!("service", crate::systems::service::hourly(world));
    // Lives at work phase 2 (§ 3.5): a Statistical visitor credited at its hour.
    crate::prof::timed!("living::household::hourly", crate::systems::household::hourly(world));
    // Jobs & lives phase 5: an episode under a chip ends the contract
    // (nothing with `[vice] chip` off).
    crate::prof::timed!("living::dollhouse::hourly", crate::systems::dollhouse::hourly(world));
    // The perf pass phase 7b: the day in two slots, split before the
    // faith's day (`systems::midnight`).
    use crate::systems::midnight::{due, Mid};
    if due(world, Mid::LivingEarly) {
        early(world);
    }
    if due(world, Mid::LivingLate) {
        late(world);
    }
}

fn early(world: &mut World) {
    // Plan L7: an older save's venues, at the first midnight with Lots.
    // Review fix: `venues_due` holds until a venue stands (no Lot at this
    // midnight: the next one retries).
    if world.venues_due && !crate::systems::jobs::seed_venues(world).is_empty() {
        world.venues_due = false;
        crate::prof::timed!("mid::virt::relink", crate::systems::virt::relink(world));
    }
    crate::prof::timed!("mid::jobs::daily", crate::systems::jobs::daily(world));
    crate::prof::timed!("mid::jobs::top_up", crate::systems::jobs::top_up(world));
    // Real economy phase 2 (plan E17, E18): the wage rule, then hiring and
    // layoffs on the margin, before the band and `demography`'s hires.
    crate::prof::timed!("mid::wages::daily", crate::systems::wages::daily(world));
    crate::prof::timed!("mid::wages::staff", crate::systems::wages::staff(world));
    // Class pay: the skilled promotions, before `demography`'s hires.
    crate::prof::timed!("mid::classpay::daily", crate::systems::classpay::daily(world));
    // Class pay P3: the rich spend on status (Spire nights, fashion, chrome).
    crate::prof::timed!("mid::classpay::status_daily", crate::systems::classpay::status_daily(world));
    // P10: the safety net lapses when the Treasury cannot pay it (before
    // the band reads the lever).
    crate::prof::timed!("mid::policy::daily", crate::systems::policy::daily(world));
    // Real economy E21 (phase 3a): under the market wage law the Treasury's
    // tax band replaces L2's budget band (no upkeep levy to cut); public
    // works, when enacted, open and close on the tax band's reading (P10).
    if crate::systems::econ::wages_on(world) {
        crate::prof::timed!("mid::treasury::daily", crate::systems::treasury::daily(world));
        crate::prof::timed!("mid::budget::market_works", crate::systems::budget::market_works(world));
    } else {
        crate::prof::timed!("mid::budget::daily", crate::systems::budget::daily(world));
    }
    // L2 shadow fixes: no-shows, the sweepers' wage, the told-memory.
    crate::prof::timed!("mid::fixes::daily", crate::systems::fixes::daily(world));
    // The pressure pass phase 1 (OPEN-178): the miserable skip, are fired,
    // quit; off screen they brawl and join (nothing with `[misery]` absent).
    crate::prof::timed!("mid::misery::daily", crate::systems::misery::daily(world));
    // Real economy (plan E6-E10): the World's book, appetite, refill and the
    // midnight Parts and Data sales.
    crate::prof::timed!("mid::world_market::daily", crate::systems::world_market::daily(world));
    // Jobs & lives phase 1: the Rations sale (after the book's refill), the
    // civic posts, the offices and an Hq built (nothing with `[jobs2]` off).
    crate::prof::timed!("mid::industry::daily", crate::systems::industry::daily(world));
    // Jobs & lives phase 2: the Pitches' day (the cut, sweeps, closings,
    // openings), the couriers' parcels (nothing with the street off).
    // Jobs & lives phase 4: the pimps named and the Brothels the law shuts,
    // before the street's day closes (nothing with `[vice]` off).
    crate::prof::timed!("mid::vice::daily", crate::systems::vice::daily(world));
    crate::prof::timed!("mid::streetwork::daily", crate::systems::streetwork::daily(world));
    // Lives at work phase 2 (§ 3.4, § 3.5): the newborns' keep and the visits due.
    crate::prof::timed!("household", crate::systems::household::daily(world));
    // Jobs & lives phase 3: the armouries, the fence, the carry roll, the
    // wants and licences, the guards' stock (nothing with `[weapons]` off).
    crate::prof::timed!("mid::weapons::daily", crate::systems::weapons::daily(world));
    // Jobs & lives phase 4: status's decay, the gangs' clothing (nothing with `[fashion]` off).
    crate::prof::timed!("mid::fashion::daily", crate::systems::fashion::daily(world));
    // Jobs & lives phase 5: the chip contracts and the braindance day
    // (nothing with `[vice] chip` / `bd` off).
    crate::prof::timed!("mid::dollhouse::daily", crate::systems::dollhouse::daily(world));
    crate::prof::timed!("mid::braindance::daily", crate::systems::braindance::daily(world));
    // Jobs & lives phase 6: the gray market's and the faith's day (nothing
    // with `[gray]` / `[faith]` off).
    crate::prof::timed!("mid::gray::daily", crate::systems::gray::daily(world));
    // Lives at work phase 5 (§ 2.4): the walk-ins' walks, the Fixer owners'
    // weekly collections (nothing with `[service] tower` off).
    crate::prof::timed!("mid::service::tower_daily", crate::systems::service::tower_daily(world));
    // The pressure pass phase 6: a business under water: the loans, the
    // stiffed workers, the shut businesses, the fronts, the owners' choices
    // and the hiring freezes (after the top-up and the visits due, before
    // `demography` hires; nothing with `[underwater]` absent).
    crate::prof::timed!("mid::underwater::daily", crate::systems::underwater::daily(world));
}

fn late(world: &mut World) {
    crate::prof::timed!("mid::faith::daily", crate::systems::faith::daily(world));
    // Jobs & lives phase 7: the children's routines, the kin, the
    // prosthetics (nothing with `[ages]` off).
    crate::prof::timed!("mid::ages::daily", crate::systems::ages::daily(world));
    // Lives at work phase 6 (§ 4): retirements, frailty exits, the foremen,
    // the gangs' lieutenants (nothing with `[careers]` / `[gangs]
    // lieutenants` off).
    crate::prof::timed!("mid::careers::daily", crate::systems::careers::daily(world));
    // L2 phase 2: the spots, the wealth decile, the venues' hand-over.
    crate::prof::timed!("mid::leisure::daily", crate::systems::leisure::daily(world));
    // The Real economy E27, E34, E37, E39, E40, E42: the kitchens' day, the
    // camps' day (before `demography` feeds the Blocks and takes), the
    // corps' camp founding.
    crate::prof::timed!("mid::charity::daily", crate::systems::charity::daily(world));
    crate::prof::timed!("mid::camp::daily", crate::systems::camp::daily(world));
    crate::prof::timed!("mid::camp::found_daily", crate::systems::camp::found_daily(world));
}
