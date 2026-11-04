use super::*;

impl World {
    /// One in-game minute, systems in the fixed order
    /// `commands, time, lod, needs, memory, mood, think, plan, exec, virt,
    /// ownership, assets, tech, classes, districts, economy, bind, word, law,
    /// social, gang, corp_brain, living, demography, stats` (L2 L36: `living`
    /// holds every L2 daily and hourly pass; M14 V39: `virt` relinks
    /// when dirty and pops run steps; `tech` is the plane's midnight pass;
    /// M15 W47: `word`'s midnight chain after the binder names its holes). The assets pass (M13 D9) runs at
    /// midnight right after ownership's, so upkeep and finance see the same
    /// purses the rent pass left. The district aggregates read the class pass's
    /// midnight state (M12 D6). The binder runs
    /// before the law so a cold-case report reaches the captain's daily
    /// rescoring (M10 D32); ownership's daily pass (rent, evictions,
    /// re-housing, upkeep) runs before the economy's price step (M11 D42).
    pub fn tick(&mut self) {
        self.apply_commands();
        // Review item 8: a cut line that ran out is back in the ride table
        // before anything reads it this tick.
        crate::prof::timed!("tick::transit::reopen", systems::transit::reopen(self));
        // time: the clock is `self.tick`; daily hooks live in the systems that need them.
        crate::prof::timed!("tick::lod", systems::lod::run(self));
        crate::prof::timed!("tick::needs", crate::needs::run(self));
        crate::prof::timed!("tick::memory", systems::memory::run(self));
        crate::prof::timed!("tick::mood", crate::mood::run(self));
        crate::prof::timed!("tick::think", systems::think::run(self));
        crate::prof::timed!("tick::plan", systems::plan::run(self));
        crate::prof::timed!("tick::exec", crate::exec::run(self));
        // C3a: the live engagements' beats (after exec: a fight begun by a
        // step this tick runs its first beats now).
        crate::prof::timed!("tick::engage", systems::combat::step(self));
        crate::prof::timed!("tick::transit", systems::transit::run(self));
        crate::prof::timed!("tick::virt", systems::virt::run(self));
        crate::prof::timed!("tick::ownership", systems::ownership::run(self));
        crate::prof::timed!("tick::assets", systems::assets::run(self));
        crate::prof::timed!("tick::tech", systems::tech::run(self));
        crate::prof::timed!("tick::classes", systems::classes::run(self));
        crate::prof::timed!("tick::districts", systems::districts::run(self));
        crate::prof::timed!("tick::economy", systems::economy::run(self));
        crate::prof::timed!("tick::bind", systems::bind::run(self));
        crate::prof::timed!("tick::word", systems::word::run(self));
        crate::prof::timed!("tick::law", systems::law::run(self));
        crate::prof::timed!("tick::social", systems::social::run(self));
        crate::prof::timed!("tick::gang", systems::gang::run(self));
        crate::prof::timed!("tick::corp_brain", systems::corp_brain::run(self));
        crate::prof::timed!("tick::living", systems::living::run(self));
        crate::prof::timed!("tick::outside", systems::outside::run(self));
        crate::prof::timed!("tick::contracts", systems::contracts::run(self));
        crate::prof::timed!("tick::leverage", systems::leverage::run(self));
        crate::prof::timed!("tick::captives", systems::captives::run(self));
        crate::prof::timed!("tick::wounded", systems::wounded::run(self));
        crate::prof::timed!("tick::combat", systems::combat::hourly(self));
        systems::fares::run(self);
        crate::prof::timed!("tick::governance", systems::governance::run(self));
        crate::prof::timed!("tick::tags", systems::tags::run(self));
        crate::prof::timed!("tick::attention", systems::attention::run(self));
        crate::prof::timed!("tick::demography", systems::demography::run(self));
        crate::prof::timed!("tick::stats", systems::stats::run(self));
        self.tick += 1;
    }

    // -----------------------------------------------------------------------
    // Shared mutations used by several systems
    // -----------------------------------------------------------------------
}

impl World {
    /// Rebuild a world from its seed and command log: the commands are queued
    /// at the ticks they were applied, and the world runs to `end_tick`.
    pub fn replay(seed: u64, config: Config, log: &[(Tick, PlayerCommand)], end_tick: Tick) -> World {
        let mut w = World::new(seed, config);
        let mut next = 0;
        while w.tick < end_tick {
            while next < log.len() && log[next].0 == w.tick {
                w.push_command(log[next].1.clone());
                next += 1;
            }
            w.tick();
        }
        w
    }

    /// Run `n` ticks.
    pub fn run_ticks(&mut self, n: u64) {
        for _ in 0..n {
            self.tick();
        }
    }
}
