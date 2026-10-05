//! The social graph's edge store: a hash map keyed `(min, max)` for the
//! per-tick lookups (`World::edge`, `edge_entry`), with every iteration in
//! ascending key order, as the `BTreeMap` it replaces iterated, and saved
//! packed in that order (same world, same bytes; see `impl Serialize`).
//! Iterating sorts, so it is for daily passes, saves and tools, not per-tick
//! code.

use std::collections::hash_map::Entry;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::components::{Edge, RelKind};
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

/// The save form's tag; the payload follows it as standard padded base64.
const PACKED_TAG: &str = "e1:";

/// Saved as one string, `"e1:" + base64(payload)` (M12: the field-named
/// struct map was 21.1 of a 40.5 MB day-120 save). The payload is a varint
/// edge count, then per edge in ascending key order:
///
/// - `a.index` minus the previous edge's (wrapping, varint), `a.generation`;
/// - `b.index` minus the previous `b.index` when `a` repeats, else minus
///   `a.index` (wrapping, varint), `b.generation`;
/// - a flag byte: bits 0-2 `kind`, then present-bits for `trust` (when not
///   the bit-exact first trust), `debt` (non-zero), `last_birth_tick`,
///   `debt_since` and `affinity` (when its bits are not 0);
/// - `affinity` if present (f32 bits, LE), `last_interaction` (varint), then
///   the present optionals in that order (`trust` as f32 bits, `debt`
///   zigzagged).
///
/// Floats are stored as raw bits, so the round trip is bit-exact. A save in
/// the old form (a map of `(EntityId, EntityId) -> Edge` structs) still loads.
impl Serialize for EdgeMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let bytes = pack(self);
        let mut out = String::with_capacity(PACKED_TAG.len() + bytes.len().div_ceil(3) * 4);
        out.push_str(PACKED_TAG);
        base64_encode(&bytes, &mut out);
        s.serialize_str(&out)
    }
}

const F_TRUST: u8 = 1 << 3;
const F_DEBT: u8 = 1 << 4;
const F_BIRTH: u8 = 1 << 5;
const F_DEBT_SINCE: u8 = 1 << 6;
const F_AFFINITY: u8 = 1 << 7;

fn kind_code(k: RelKind) -> u8 {
    match k {
        RelKind::Acquaintance => 0,
        RelKind::Friend => 1,
        RelKind::Family => 2,
        RelKind::Spouse => 3,
        RelKind::Parent => 4,
        RelKind::Rival => 5,
        RelKind::Enemy => 6,
    }
}

fn kind_from(code: u8) -> Option<RelKind> {
    Some(match code {
        0 => RelKind::Acquaintance,
        1 => RelKind::Friend,
        2 => RelKind::Family,
        3 => RelKind::Spouse,
        4 => RelKind::Parent,
        5 => RelKind::Rival,
        6 => RelKind::Enemy,
        _ => return None,
    })
}

/// The trust a new edge starts with, as bits: the one value left out.
fn first_trust_bits() -> u32 {
    Edge::new(RelKind::Acquaintance, 0).trust.to_bits()
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn pack(edges: &EdgeMap) -> Vec<u8> {
    let mut out = Vec::with_capacity(edges.map.len() * 14 + 4);
    put_varint(&mut out, edges.map.len() as u64);
    let first_trust = first_trust_bits();
    let mut prev: Option<EdgeKey> = None;
    for (&(a, b), e) in edges.iter() {
        let prev_a = prev.map_or(0, |(pa, _)| pa.index);
        put_varint(&mut out, u64::from(a.index.wrapping_sub(prev_a)));
        put_varint(&mut out, u64::from(a.generation));
        let base = match prev {
            Some((pa, pb)) if pa == a => pb.index,
            _ => a.index,
        };
        put_varint(&mut out, u64::from(b.index.wrapping_sub(base)));
        put_varint(&mut out, u64::from(b.generation));
        prev = Some((a, b));

        let trust = e.trust.to_bits();
        let affinity = e.affinity.to_bits();
        let mut flags = kind_code(e.kind);
        if trust != first_trust {
            flags |= F_TRUST;
        }
        if e.debt != 0 {
            flags |= F_DEBT;
        }
        if e.last_birth_tick.is_some() {
            flags |= F_BIRTH;
        }
        if e.debt_since.is_some() {
            flags |= F_DEBT_SINCE;
        }
        if affinity != 0 {
            flags |= F_AFFINITY;
        }
        out.push(flags);
        if affinity != 0 {
            out.extend_from_slice(&affinity.to_le_bytes());
        }
        put_varint(&mut out, e.last_interaction);
        if trust != first_trust {
            out.extend_from_slice(&trust.to_le_bytes());
        }
        if e.debt != 0 {
            let zigzag = ((e.debt << 1) ^ (e.debt >> 31)) as u32;
            put_varint(&mut out, u64::from(zigzag));
        }
        if let Some(t) = e.last_birth_tick {
            put_varint(&mut out, t);
        }
        if let Some(t) = e.debt_since {
            put_varint(&mut out, t);
        }
    }
    out
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, &'static str> {
        let b = *self.bytes.get(self.pos).ok_or("packed edges: truncated")?;
        self.pos += 1;
        Ok(b)
    }

    fn varint(&mut self) -> Result<u64, &'static str> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("packed edges: varint too long")
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        u32::try_from(self.varint()?).map_err(|_| "packed edges: value out of u32 range")
    }

    fn f32(&mut self) -> Result<f32, &'static str> {
        let mut b = [0u8; 4];
        for x in &mut b {
            *x = self.byte()?;
        }
        Ok(f32::from_bits(u32::from_le_bytes(b)))
    }
}

fn unpack(bytes: &[u8]) -> Result<Map, &'static str> {
    let mut r = Reader { bytes, pos: 0 };
    let n = usize::try_from(r.varint()?).map_err(|_| "packed edges: count too large")?;
    let mut map = Map::default();
    // Each edge takes at least six bytes; a corrupt count reserves no more.
    map.reserve(n.min(bytes.len() / 6));
    let first_trust = f32::from_bits(first_trust_bits());
    let mut prev: Option<EdgeKey> = None;
    for _ in 0..n {
        let prev_a = prev.map_or(0, |(pa, _)| pa.index);
        let a_index = prev_a.wrapping_add(r.u32()?);
        let a = EntityId { index: a_index, generation: r.u32()? };
        let base = match prev {
            Some((pa, pb)) if pa == a => pb.index,
            _ => a.index,
        };
        let b_index = base.wrapping_add(r.u32()?);
        let b = EntityId { index: b_index, generation: r.u32()? };
        prev = Some((a, b));

        let flags = r.byte()?;
        let kind = kind_from(flags & 0b111).ok_or("packed edges: bad kind")?;
        let affinity = if flags & F_AFFINITY != 0 { r.f32()? } else { f32::from_bits(0) };
        let last_interaction = r.varint()?;
        let trust = if flags & F_TRUST != 0 { r.f32()? } else { first_trust };
        let debt = if flags & F_DEBT != 0 {
            let z = r.u32()?;
            ((z >> 1) as i32) ^ -((z & 1) as i32)
        } else {
            0
        };
        let last_birth_tick = if flags & F_BIRTH != 0 { Some(r.varint()?) } else { None };
        let debt_since = if flags & F_DEBT_SINCE != 0 { Some(r.varint()?) } else { None };
        let edge = Edge { affinity, trust, debt, kind, last_interaction, last_birth_tick, debt_since };
        if map.insert((a, b), edge).is_some() {
            return Err("packed edges: duplicate key");
        }
    }
    if r.pos != bytes.len() {
        return Err("packed edges: trailing bytes");
    }
    Ok(map)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8], out: &mut String) {
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        let sextet = |shift: u32| char::from(B64[((n >> shift) & 63) as usize]);
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
}

fn base64_value(c: u8) -> Result<u32, &'static str> {
    let v = match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return Err("packed edges: bad base64 character"),
    };
    Ok(u32::from(v))
}

fn base64_decode(text: &str) -> Result<Vec<u8>, &'static str> {
    let text = text.as_bytes();
    if !text.len().is_multiple_of(4) {
        return Err("packed edges: base64 length not a multiple of 4");
    }
    let (quad_slice, _) = text.as_chunks::<4>();
    let quads = quad_slice.len();
    let mut out = Vec::with_capacity(quads * 3);
    for (i, q) in quad_slice.iter().enumerate() {
        let pad = if i + 1 == quads { q.iter().rev().take_while(|&&c| c == b'=').count() } else { 0 };
        if pad > 2 {
            return Err("packed edges: bad base64 padding");
        }
        let mut n = 0u32;
        for &c in &q[..4 - pad] {
            n = (n << 6) | base64_value(c)?;
        }
        n <<= 6 * pad as u32;
        let three = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&three[..3 - pad]);
    }
    Ok(out)
}

/// Takes either form: the packed string, or the pre-M12 ordered map of
/// `(min, max) -> Edge` structs.
struct EdgeMapVisitor;

impl<'de> Visitor<'de> for EdgeMapVisitor {
    type Value = EdgeMap;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a packed edge string or a map of edge key to edge")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<EdgeMap, E> {
        let body = v.strip_prefix(PACKED_TAG).ok_or_else(|| E::custom("packed edges: unknown tag"))?;
        let bytes = base64_decode(body).map_err(E::custom)?;
        Ok(EdgeMap { map: unpack(&bytes).map_err(E::custom)? })
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<EdgeMap, A::Error> {
        let mut map = Map::default();
        while let Some((k, e)) = access.next_entry::<EdgeKey, Edge>()? {
            map.insert(k, e);
        }
        Ok(EdgeMap { map })
    }
}

impl<'de> Deserialize<'de> for EdgeMap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(EdgeMapVisitor)
    }
}
