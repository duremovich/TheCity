//! Seeded, serialisable randomness. The world stream drives spawning and
//! systems; each agent also owns a private ChaCha8 stream keyed by its index
//! so the Statistical tick can draw without disturbing the world stream.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::entity::EntityId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimRng {
    seed: u64,
    world: ChaCha8Rng,
    /// Indexed by `id.index` (a map lookup per draw was a measurable share of
    /// the Statistical tick); saved as the `index -> stream` map it was.
    #[serde(with = "agent_streams")]
    agents: Vec<Option<ChaCha8Rng>>,
}

/// The agent streams on disk: a map from index to stream, ascending, as the
/// `BTreeMap` they were kept in wrote it (saves stay byte-identical).
mod agent_streams {
    use rand_chacha::ChaCha8Rng;
    use serde::ser::SerializeMap;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(v: &[Option<ChaCha8Rng>], s: S) -> Result<S::Ok, S::Error> {
        let n = v.iter().filter(|r| r.is_some()).count();
        let mut m = s.serialize_map(Some(n))?;
        for (i, r) in v.iter().enumerate() {
            if let Some(r) = r {
                m.serialize_entry(&(i as u32), r)?;
            }
        }
        m.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Option<ChaCha8Rng>>, D::Error> {
        let map = BTreeMap::<u32, ChaCha8Rng>::deserialize(d)?;
        let mut v: Vec<Option<ChaCha8Rng>> = Vec::new();
        for (i, r) in map {
            let i = i as usize;
            if v.len() <= i {
                v.resize(i + 1, None);
            }
            v[i] = Some(r);
        }
        Ok(v)
    }
}

impl SimRng {
    pub fn new(seed: u64) -> Self {
        SimRng { seed, world: ChaCha8Rng::seed_from_u64(seed), agents: Vec::new() }
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
        let i = id.index as usize;
        if self.agents.len() <= i {
            self.agents.resize(i + 1, None);
        }
        self.agents[i].get_or_insert_with(|| {
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
        if let Some(r) = self.agents.get_mut(id.index as usize) {
            *r = None;
        }
    }
}
