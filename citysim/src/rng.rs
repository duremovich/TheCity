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
        r.set_stream(stream_id_hole(id));
        r
    }

    /// M12 D17: a one-off stream for a keyed draw (where a Statistical
    /// crime's litter lands), never stored, so it touches neither the world
    /// stream nor any agent's. Bit 62 keeps it apart from holes (bit 63)
    /// and agents (`index + 1`).
    pub fn keyed(&self, key: u64) -> ChaCha8Rng {
        let mut r = ChaCha8Rng::seed_from_u64(self.seed);
        r.set_stream(stream_id_keyed(key));
        r
    }

    /// M14 V9: one contest of a run (a dice roll on the Virt plane) on its
    /// own fresh stream, keyed on the run id and the contest's ordinal,
    /// never stored: save/load, LOD and call order cannot move a run's dice,
    /// and the world stream is never touched. Stream layout: agents `index
    /// + 1 < 2^33`, holes bit 63, keyed draws bit 62 with keys below 2^60,
    /// runs bits 62 and 61 (plan deviation: bit 61 keeps them off every
    /// keyed key) with `run << 6 | contest` below (`RunId < 2^50`).
    pub fn run(&self, run: crate::virt::RunId, contest: u8) -> ChaCha8Rng {
        debug_assert!(run < (1 << 50), "a RunId fits 50 bits");
        let mut r = ChaCha8Rng::seed_from_u64(self.seed);
        r.set_stream(stream_id_run(run, contest));
        r
    }

    /// M15 W8: one roll of the word (an exchange, a day's hearing, the kin
    /// channel, a move, a story) on its own fresh stream, never stored, so
    /// the world and agent streams are never touched and call order,
    /// save/load and LOD cannot move it. Stream layout (see `run` and
    /// `keyed`): agents `index + 1 < 2^33`, holes bit 63, keyed draws bit 62
    /// with keys below 2^60, runs bits 62 and 61, **words bits 62 and 60**
    /// with bit 61 clear (plan deviation: the plan's `WORD_KEY` was bit 61,
    /// which M14's run streams took), the namespace in bits 52-55 and a
    /// 52-bit hash of `(a, b)` below.
    pub fn word(&self, ns: crate::word::WordNs, a: u64, b: u64) -> ChaCha8Rng {
        let mut r = ChaCha8Rng::seed_from_u64(self.seed);
        r.set_stream(stream_id_word(ns, a, b));
        r
    }

    /// M16a (plan C4): one roll of a contract record (renege, the ledger's
    /// due tick, outcome and taker death, the Fixer's talk) on its own
    /// fresh stream, never stored: the world and agent streams are never
    /// touched, and call order, save/load and LOD cannot move it. Stream
    /// layout (see `hole`, `keyed`, `run`, `word`): agents `index + 1 <
    /// 2^33`, holes bit 63 with ids below 2^44, keyed draws bit 62 (bit 63
    /// clear) with keys below 2^60, runs bits 62 and 61, words bits 62 and
    /// 60, **contracts bits 63 and 62** (`CONTRACT_KEY`; no other kind sets
    /// both) with the namespace in bits 56-59 and a 56-bit hash of `(a, b)`
    /// below.
    pub fn contract(&self, ns: crate::contract::ContractNs, a: u64, b: u64) -> ChaCha8Rng {
        let mut r = ChaCha8Rng::seed_from_u64(self.seed);
        r.set_stream(stream_id_contract(ns, a, b));
        r
    }

    /// Drop an agent's stream when the entity is despawned.
    pub fn forget_agent(&mut self, id: EntityId) {
        if let Some(r) = self.agents.get_mut(id.index as usize) {
            *r = None;
        }
    }
}

/// M15 W8: bit 60 marks a word stream (`SimRng::word`).
pub const WORD_KEY: u64 = 1 << 60;

/// M16a (plan C4): bits 63 and 62 mark a contract stream (`SimRng::contract`).
pub const CONTRACT_KEY: u64 = (1 << 63) | (1 << 62);

/// The stream id of `SimRng::hole(id)`.
pub(crate) fn stream_id_hole(id: u64) -> u64 {
    id | (1 << 63)
}

/// The stream id of `SimRng::keyed(key)`.
pub(crate) fn stream_id_keyed(key: u64) -> u64 {
    (key & !(1 << 63)) | (1 << 62)
}

/// The stream id of `SimRng::run(run, contest)`.
pub(crate) fn stream_id_run(run: u64, contest: u8) -> u64 {
    (1 << 62) | (1 << 61) | (run << 6) | u64::from(contest & 0x3F)
}

/// The stream id of `SimRng::word(ns, a, b)`.
pub(crate) fn stream_id_word(ns: crate::word::WordNs, a: u64, b: u64) -> u64 {
    WORD_KEY | (1 << 62) | ((ns as u64) << 52) | (splitmix64(a ^ b.rotate_left(29)) >> 12)
}

/// The stream id of `SimRng::contract(ns, a, b)`.
pub(crate) fn stream_id_contract(ns: crate::contract::ContractNs, a: u64, b: u64) -> u64 {
    CONTRACT_KEY | ((ns as u64) << 56) | (splitmix64(a ^ b.rotate_left(29)) >> 8)
}

/// SplitMix64's finaliser: a cheap, well-mixed 64-bit hash.
pub fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod contract_stream_tests {
    use super::*;
    use crate::contract::ContractNs;
    use crate::word::WordNs;

    /// M16a (plan 1.5): a contract stream id never equals a hole, keyed,
    /// run or word stream id, nor an agent's, over a spread of keys.
    #[test]
    fn test_contract_streams_never_meet_other_kinds() {
        let ns = [ContractNs::Renege, ContractNs::Due, ContractNs::Outcome, ContractNs::TakerDeath, ContractNs::Talk];
        let keys: Vec<u64> = (0..64u64)
            .map(splitmix64)
            .chain([0, 1, 2, u64::MAX, 1 << 44, (1 << 60) - 1, 0xB0D7 << 44, 0x4AC4 << 44])
            .collect();
        let mut others = std::collections::BTreeSet::new();
        for &k in &keys {
            // holes: ids below 2^44; keyed: keys below 2^60 (and the body/hack keys).
            others.insert(stream_id_hole(k & ((1 << 44) - 1)));
            others.insert(stream_id_keyed(k & ((1 << 60) - 1)));
            others.insert(stream_id_keyed((0xB0D7 << 44) ^ (k & 0xFFFF_FFFF)));
            others.insert(stream_id_keyed((0x4AC4 << 44) ^ (k & 0xFFFF_FFFF)));
            others.insert(stream_id_run(k & ((1 << 50) - 1), (k & 0x3F) as u8));
            for w in [WordNs::Exchange, WordNs::Move, WordNs::Hunt, WordNs::Story, WordNs::Held] {
                others.insert(stream_id_word(w, k, k.rotate_left(7)));
            }
            // agents: index + 1 < 2^33.
            others.insert((k & ((1 << 33) - 1)) + 1);
        }
        for &k in &keys {
            for &n in &ns {
                for b in [0u64, 1, 2, k] {
                    let id = stream_id_contract(n, k, b);
                    assert_eq!(id & CONTRACT_KEY, CONTRACT_KEY, "both key bits set");
                    assert!(!others.contains(&id), "contract stream {id:#x} meets another kind");
                }
            }
        }
        // No other kind sets both bits 63 and 62.
        assert!(others.iter().all(|&o| o & CONTRACT_KEY != CONTRACT_KEY));
    }
}
