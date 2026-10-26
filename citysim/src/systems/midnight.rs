//! The perf pass phase 7b (docs/PERF_PASS.md R8; OPEN-184): the midnight
//! stagger. The heavy daily passes run whole, in their old order, one slot
//! every `[perf] stagger_midnight` ticks over the night's first minutes,
//! not all on tick 0: a 100k midnight was 2.6 s of one tick (phase 7), its
//! largest pass 0.29 s.
//!
//! The schedule is a pure function of the tick of day: pass `p` runs at
//! tick of day `slot(p) * stagger_midnight`, so a save at any tick resumes
//! the same and nothing is saved for it. Slot 0 is not a `Mid`: the day's
//! rollover (`memory`'s decay, `ownership`, `assets`, `tech`, `classes`,
//! `districts`) and the price move (`economy`) keep `tick_of_day() == 0`,
//! as `lod`'s ledger roll and the 23:59 report row keep theirs; every daily
//! pass after them in the tick order is a `Mid` here. With
//! `stagger_midnight = 0` (absent, or `CITYSIM_STAGGER_OFF=1`) every slot is
//! tick 0: the base.
//!
//! The slots follow phase 7's 100k profile of days 27-30 (ms at midnight):
//! one heavy pass a slot, the light ones grouped with a neighbour. Hourly
//! passes stay on the hour: a `run` gated on the hour that held a midnight
//! piece (`outside`'s parents, `governance`'s boards, `attention`'s pools)
//! runs that piece at its slot instead.

use crate::world::World;

/// A staggered midnight pass (or a run of passes kept together), in the
/// old order of the tick (`World::tick`); `slot` is monotone in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mid {
    /// `bind::run` (`fviolence::daily`, the binds; 44 ms).
    Bind,
    /// `word::run` to the kin's hearing (news 35, hearing 38).
    WordEarly,
    /// `word::run` from the reputation rebuild (172) to the prune.
    WordLate,
    /// `law::run`'s day and `law_brain`'s daily rescore (205).
    Law,
    /// `social::daily` (150).
    Social,
    /// The gangs' day in `gang::run` (295, the largest).
    Gang,
    /// `corp_brain::run`'s day (206).
    Corp,
    /// `living::run`'s day to `service::tower_daily` (household 92,
    /// streetwork 42, wages 35; ~210).
    LivingEarly,
    /// `living::run`'s day from `faith::daily` (206) to the camps (~300).
    LivingLate,
    /// The tail, light (~150): `outside`'s parents and missions abroad,
    /// `contracts`' expiries (97), `leverage`, `captives`, `wounded`,
    /// `governance`'s boards, `attention`'s pools.
    Tail,
    /// `demography::run` to the far commuters' job moves (corpses 88).
    DemoEarly,
    /// `demography::run`'s job search to the Spire homes (job search 194).
    DemoJobs,
    /// `demography::run`'s relocation to the immigration (relocation 147).
    DemoHomes,
}

/// The last slot. `[perf] stagger_midnight` times it stays inside the
/// night's first hour (`Config::load` asserts it), so 01:00's hourly
/// passes (`fviolence::rebuild_active`, "after the midnight rescore") see
/// the whole midnight.
pub const LAST_SLOT: u16 = 12;

/// The slot of `p`: the tick of day it runs at is this times the spread.
pub const fn slot(p: Mid) -> u16 {
    match p {
        Mid::Bind | Mid::WordEarly => 1,
        Mid::WordLate => 2,
        Mid::Law => 3,
        Mid::Social => 4,
        Mid::Gang => 5,
        Mid::Corp => 6,
        Mid::LivingEarly => 7,
        Mid::LivingLate => 8,
        Mid::Tail => 9,
        Mid::DemoEarly => 10,
        Mid::DemoJobs => 11,
        Mid::DemoHomes => 12,
    }
}

/// The tick of day `p` runs at. Day 0's midnight is the seeded city's
/// first (the build's hires, posts and orders, read by every think from
/// tick 1): it stays whole on tick 0.
pub fn tick_of(world: &World, p: Mid) -> u16 {
    if world.day() == 0 {
        return 0;
    }
    slot(p) * world.config.perf.stagger_midnight
}

/// True on the tick `p` runs: its day's pass is due now.
pub fn due(world: &World, p: Mid) -> bool {
    world.tick_of_day() == tick_of(world, p)
}

/// Ticks from a midnight through its last slot: the ticks a caller (a test,
/// a tool) runs from tick 0 of a day after the first for the whole
/// midnight to have run (1 when nothing is staggered).
pub fn window(world: &World) -> crate::time::Tick {
    crate::time::Tick::from(LAST_SLOT * world.config.perf.stagger_midnight) + 1
}

/// Today's tick 0, the midnight a staggered pass belongs to. A daily pass
/// that read "the tick before now" as yesterday's last minute reads
/// `midnight(world) - 1` (the same tick on tick 0).
pub fn midnight(world: &World) -> crate::time::Tick {
    world.tick - crate::time::Tick::from(world.tick_of_day())
}

/// The tick a start or a deadline is stamped from: inside the staggered
/// window (tick 0 to the last slot) today's midnight, else now. A span of
/// whole days begun by a staggered pass then ends on the hour, and at the
/// midnight, it ended on before the stagger, whatever slot reads it (a
/// site begun at 00:07 for three days stood at 01:00 on the third day, an
/// hour after the job search that would have staffed it). Any tick on
/// tick 0, so the base is unchanged.
pub fn stamp(world: &World) -> crate::time::Tick {
    if world.day() > 0 && world.tick_of_day() <= LAST_SLOT * world.config.perf.stagger_midnight {
        midnight(world)
    } else {
        world.tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slots_keep_the_old_order() {
        let order = [
            Mid::Bind,
            Mid::WordEarly,
            Mid::WordLate,
            Mid::Law,
            Mid::Social,
            Mid::Gang,
            Mid::Corp,
            Mid::LivingEarly,
            Mid::LivingLate,
            Mid::Tail,
            Mid::DemoEarly,
            Mid::DemoJobs,
            Mid::DemoHomes,
        ];
        assert!(order.windows(2).all(|w| slot(w[0]) <= slot(w[1])));
        assert!(order.iter().all(|&p| (1..=LAST_SLOT).contains(&slot(p))));
    }
}
