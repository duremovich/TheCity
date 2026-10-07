//! M15 (plan W47): the word's place in the tick, after `bind` (so a bound
//! hole is named before the pools decay) and before `law`, `gang` and
//! `corp_brain` (which from phase 2-3 read heat, fear and honour). Phase 1
//! runs only the midnight chain; phase 3 adds `hunt::tick` every tick.
//!
//! The daily chain (plan "Daily pass"): decay and leak the pools, the
//! Statistical hearing and post-back, the kin channel, the reputation
//! rebuild (axes, `known_by`, factions, regard, the kill-watch sample),
//! competence (phase 2: the Lab shifts' knowledge, the rust, every corp's
//! and the Law's competence, `TalentLost`), then expiry. With `[gossip]
//! enabled = false` nothing runs.

use crate::world::World;

pub fn run(world: &mut World) {
    if !world.config.gossip.enabled || world.tick_of_day() != 0 {
        return;
    }
    let n = world.districts.len();
    if world.rumours.len() != n {
        world.rumours.resize_with(n, Default::default);
    }
    crate::systems::gossip::decay_and_leak(world);
    crate::systems::gossip::hear(world);
    crate::systems::gossip::kin(world);
    crate::systems::reputation::rebuild(world);
    crate::systems::competence::daily(world);
    crate::systems::gossip::expire_sightings(world);
    crate::systems::gossip::prune_anon(world);
}
