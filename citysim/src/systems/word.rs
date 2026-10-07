//! M15 (plan W47): the word's place in the tick, after `bind` (so a bound
//! hole is named before the pools decay) and before `law`, `gang` and
//! `corp_brain` (which from phase 2-3 read heat, fear and honour). Phase 1
//! runs only the midnight chain; phase 3 adds `hunt::tick` every tick and
//! grudges, vendettas and the Hunt's daily pass to the chain.
//!
//! The daily chain (plan "Daily pass"): decay and leak the pools, the
//! Statistical hearing and post-back, the kin channel, the reputation
//! rebuild (axes, `known_by`, factions, regard, the kill-watch sample),
//! competence (phase 2: the Lab shifts' knowledge, the rust, every corp's
//! and the Law's competence, `TalentLost`), then expiry; phase 4 runs the
//! Feeds (`news::daily`) after the pools decay. With `[gossip]
//! enabled = false` nothing runs.

use crate::world::World;

pub fn run(world: &mut World) {
    if !world.config.gossip.enabled {
        return;
    }
    // Phase 3 (W22): the Hunts' validity, every tick (≤ `max_hunts`).
    crate::systems::hunt::tick(world);
    if world.tick_of_day() != 0 {
        return;
    }
    let n = world.districts.len();
    if world.rumours.len() != n {
        world.rumours.resize_with(n, Default::default);
    }
    crate::systems::gossip::decay_and_leak(world);
    // Phase 4 (W37, W40): the Feeds' reach, Spin, stories and ads, before
    // the hearing so a story is read the same night.
    crate::systems::news::daily(world);
    crate::systems::gossip::hear(world);
    crate::systems::gossip::kin(world);
    crate::systems::reputation::rebuild(world);
    crate::systems::competence::daily(world);
    // Phase 3: grudge decay and settlement, kill_chain expiry, vendettas
    // (W16, W18); abandoned Hunts and the Statistical pass (W22, W23).
    crate::systems::grudges::daily(world);
    crate::systems::hunt::daily(world);
    crate::systems::gossip::expire_sightings(world);
    crate::systems::gossip::prune_anon(world);
}
