//! The social graph's edge store: a hash map keyed `(min, max)` for the
//! per-tick lookups (`World::edge`, `edge_entry`), with every iteration in
//! ascending key order, as the `BTreeMap` it replaces iterated, and saved as
//! the same ordered map (saves are byte-identical). Iterating sorts, so it
//! is for daily passes, saves and tools, not per-tick code.

use std::collections::hash_map::Entry;
use std::collections::BTreeMap;
use std::hash::{BuildHasherDefault, Hasher};

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::components::Edge;
use crate::entity::EntityId;

pub type EdgeKey = (EntityId, EntityId);

/// A multiply-rotate hasher for the small integer keys (the default SipHash
/// costs more than the lookup it replaces). Never iterated in hash order.
#[derive(Default, Clone, Copy)]
pub struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.write_u64(u64::from(n));
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

/// The `disallowed_types` rule guards iteration order: every iteration here
/// is in key order (`iter`, `keys`, `values`, `iter_mut`, the save) except
/// `iter_mut_unordered`, whose one caller sorts what it collects. The hasher
/// is fixed, not seeded per process.
#[allow(clippy::disallowed_types)]
type Map = std::collections::HashMap<EdgeKey, Edge, BuildHasherDefault<KeyHasher>>;

#[derive(Clone, Debug, Default)]
pub struct EdgeMap {
    map: Map,
}

impl EdgeMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn get(&self, k: &EdgeKey) -> Option<&Edge> {
        self.map.get(k)
    }

    pub fn get_mut(&mut self, k: &EdgeKey) -> Option<&mut Edge> {
        self.map.get_mut(k)
    }

    pub fn contains_key(&self, k: &EdgeKey) -> bool {
        self.map.contains_key(k)
    }

    pub fn remove(&mut self, k: &EdgeKey) -> Option<Edge> {
        self.map.remove(k)
    }

    pub fn insert(&mut self, k: EdgeKey, e: Edge) -> Option<Edge> {
        self.map.insert(k, e)
    }

    pub fn entry(&mut self, k: EdgeKey) -> Entry<'_, EdgeKey, Edge> {
        self.map.entry(k)
    }

    /// Ascending by key.
    pub fn iter(&self) -> std::vec::IntoIter<(&EdgeKey, &Edge)> {
        let mut v: Vec<(&EdgeKey, &Edge)> = self.map.iter().collect();
        v.sort_unstable_by_key(|&(k, _)| *k);
        v.into_iter()
    }

    /// Ascending by key.
    pub fn iter_mut(&mut self) -> std::vec::IntoIter<(&EdgeKey, &mut Edge)> {
        let mut v: Vec<(&EdgeKey, &mut Edge)> = self.map.iter_mut().collect();
        v.sort_unstable_by_key(|(k, _)| **k);
        v.into_iter()
    }

    /// In no particular order: for a pass whose per-edge work is
    /// independent and whose collected keys are sorted afterwards.
    pub fn iter_mut_unordered(&mut self) -> impl Iterator<Item = (&EdgeKey, &mut Edge)> {
        self.map.iter_mut()
    }

    /// Ascending.
    pub fn keys(&self) -> std::vec::IntoIter<&EdgeKey> {
        let mut v: Vec<&EdgeKey> = self.map.keys().collect();
        v.sort_unstable();
        v.into_iter()
    }

    /// In ascending key order.
    pub fn values(&self) -> impl Iterator<Item = &Edge> {
        self.iter().map(|(_, e)| e)
    }
}

impl Serialize for EdgeMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.map.len()))?;
        for (k, e) in self.iter() {
            m.serialize_entry(k, e)?;
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for EdgeMap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let ordered = BTreeMap::<EdgeKey, Edge>::deserialize(d)?;
        Ok(EdgeMap { map: ordered.into_iter().collect() })
    }
}
