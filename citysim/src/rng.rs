//! Seeded, serialisable randomness. The world stream drives spawning and
//! systems; each agent also owns a private ChaCha8 stream keyed by its index
//! so the Statistical tick can draw without disturbing the world stream.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::entity::EntityId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimRng {
    seed: u64,
    world: ChaCha8Rng,
    agents: BTreeMap<u32, ChaCha8Rng>,
}

impl SimRng {
    pub fn new(seed: u64) -> Self {
        SimRng { seed, world: ChaCha8Rng::seed_from_u64(seed), agents: BTreeMap::new() }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The shared world stream.
    pub fn world(&mut self) -> &mut ChaCha8Rng {
        &mut self.world
    }

    /// The agent's own stream, seeded from `(world_seed, id.index)` on first use.
    pub fn agent(&mut self, id: EntityId) -> &mut ChaCha8Rng {
        let seed = self.seed;
        self.agents.entry(id.index).or_insert_with(|| {
            let mut r = ChaCha8Rng::seed_from_u64(seed);
            r.set_stream(u64::from(id.index) + 1);
            r
        })
    }

    /// A hole's own stream (M10 D7): a fresh ChaCha8 from `(world_seed, hole
    /// id)`, never stored, so call order, save/load and repetition never
    /// change the draw. Agent streams use `index + 1 < 2^33`; this sets bit 63.
    pub fn hole(&self, id: crate::components::HoleId) -> ChaCha8Rng {
        let mut r = ChaCha8Rng::seed_from_u64(self.seed);
        r.set_stream(id | (1 << 63));
        r
    }

    /// Drop an agent's stream when the entity is despawned.
    pub fn forget_agent(&mut self, id: EntityId) {
        self.agents.remove(&id.index);
    }
}
