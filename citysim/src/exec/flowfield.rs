//! One direction field per building door, built by Dijkstra from the door
//! with the same entry costs A* uses, so descending the field reproduces an
//! optimal A* path cost in O(1) per step.
//!
//! M10: a field stores one direction byte per tile (49 KB at 256 x 192) and
//! no cost array after the build; fields are built lazily and kept in an LRU
//! ([`FlowCache`]). A field is a pure function of the map and its door, so
//! eviction never changes results.

use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use crate::components::TilePos;
use crate::entity::EntityId;
use crate::map::Map;

/// Direction bytes: 0 = none (unreachable, or the door itself); 1..=4 =
/// W, E, N, S, the order of `Map::neighbours4`.
const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

fn dir_byte(from: TilePos, to: TilePos) -> u8 {
    let d = (i32::from(to.x) - i32::from(from.x), i32::from(to.y) - i32::from(from.y));
    DIRS.iter().position(|&x| x == d).map_or(0, |i| i as u8 + 1)
}

/// Per tile: the direction toward the door. Index is `y * w + x`.
#[derive(Clone, Debug, Default)]
pub struct FlowField {
    pub next: Vec<u8>,
    /// Map width (row stride of `next`).
    pub w: u16,
    pub door: Option<TilePos>,
}

impl FlowField {
    fn idx(&self, p: TilePos) -> usize {
        usize::from(p.y) * usize::from(self.w) + usize::from(p.x)
    }

    /// Build the field for `door`: Dijkstra in `(cost, tile)` order with a
    /// monotone radix heap (M11: about twice the binary heap's speed; the
    /// pop order, so every direction byte, is the same; see
    /// [`FlowField::build_binary_heap`] and `tests/exec.rs`).
    pub fn build(map: &Map, door: TilePos) -> FlowField {
        let n = map.w() * map.h();
        let mut f = FlowField { next: vec![0; n], w: map.w() as u16, door: Some(door) };
        let mut cost = vec![f32::INFINITY; n];
        let mut open = RadixHeap::new();
        cost[f.idx(door)] = 0.0;
        open.push(key(0.0, door));
        while let Some(k) = open.pop() {
            let (c, cur) = unkey(k);
            if c > cost[f.idx(cur)] {
                continue;
            }
            // Travelling from n to cur enters cur, so n's cost = cost(cur) + dist(cur).
            let enter_cur = map.tile_at(cur).move_cost();
            for nb in map.neighbours4(cur) {
                if !map.walkable(nb) {
                    continue;
                }
                let nc = c + enter_cur;
                let i = f.idx(nb);
                if nc < cost[i] {
                    cost[i] = nc;
                    f.next[i] = dir_byte(nb, cur);
                    open.push(key(nc, nb));
                }
            }
        }
        f
    }

    /// The M10 build (binary heap of `(cost, tile)`), kept as the reference
    /// the radix-heap build is tested against.
    #[doc(hidden)]
    pub fn build_binary_heap(map: &Map, door: TilePos) -> FlowField {
        let n = map.w() * map.h();
        let mut f = FlowField { next: vec![0; n], w: map.w() as u16, door: Some(door) };
        let mut cost = vec![f32::INFINITY; n];
        let mut open = BinaryHeap::new();
        cost[f.idx(door)] = 0.0;
        open.push(Reverse((OrderedFloat(0.0f32), door)));
        while let Some(Reverse((OrderedFloat(c), cur))) = open.pop() {
            if c > cost[f.idx(cur)] {
                continue;
            }
            // Travelling from n to cur enters cur, so n's cost = cost(cur) + dist(cur).
            let enter_cur = map.tile_at(cur).move_cost();
            for nb in map.neighbours4(cur) {
                if !map.walkable(nb) {
                    continue;
                }
                let nc = c + enter_cur;
                let i = f.idx(nb);
                if nc < cost[i] {
                    cost[i] = nc;
                    f.next[i] = dir_byte(nb, cur);
                    open.push(Reverse((OrderedFloat(nc), nb)));
                }
            }
        }
        f
    }

    /// The next tile toward the door from `p`.
    pub fn step(&self, p: TilePos) -> Option<TilePos> {
        let b = *self.next.get(self.idx(p))?;
        if b == 0 {
            return None;
        }
        let (dx, dy) = DIRS[usize::from(b - 1)];
        Some(TilePos { x: (i32::from(p.x) + dx) as u8, y: (i32::from(p.y) + dy) as u8 })
    }

    /// Does a tile outside `sealed` step into `t`? (`t` is on the descent of
    /// some tile a mover can stand on.)
    pub fn entered_from_outside(&self, t: TilePos, sealed: &dyn Fn(TilePos) -> bool) -> bool {
        let w = i32::from(self.w);
        let h = (self.next.len() / usize::from(self.w.max(1))) as i32;
        DIRS.iter().any(|&(dx, dy)| {
            let (x, y) = (i32::from(t.x) - dx, i32::from(t.y) - dy);
            if x < 0 || y < 0 || x >= w || y >= h {
                return false;
            }
            let from = TilePos { x: x as u8, y: y as u8 };
            !sealed(from) && self.step(from) == Some(t)
        })
    }

    /// Full descent from `p` to the door, excluding `p`, including the door.
    pub fn descend(&self, p: TilePos) -> Vec<TilePos> {
        let mut out = Vec::new();
        let mut cur = p;
        while let Some(n) = self.step(cur) {
            out.push(n);
            cur = n;
            if out.len() > self.next.len() {
                break; // cannot happen on a well-formed field; guards against loops
            }
        }
        out
    }
}

/// `(cost, tile)` as one `u64` ordered like the tuple: a non-negative `f32`'s
/// bits order like its value, and `TilePos` orders by `(x, y)`.
fn key(cost: f32, t: TilePos) -> u64 {
    (u64::from(cost.to_bits()) << 16) | (u64::from(t.x) << 8) | u64::from(t.y)
}

fn unkey(k: u64) -> (f32, TilePos) {
    (f32::from_bits((k >> 16) as u32), TilePos { x: (k >> 8) as u8, y: k as u8 })
}

/// A monotone priority queue (radix heap): every pushed key is at least the
/// last popped one, as in Dijkstra with positive step costs. Pops in key
/// order; equal keys are identical entries, so their order is moot.
struct RadixHeap {
    last: u64,
    buckets: Vec<Vec<u64>>,
    len: usize,
}

impl RadixHeap {
    fn new() -> RadixHeap {
        RadixHeap { last: 0, buckets: vec![Vec::new(); 65], len: 0 }
    }

    fn bucket(&self, k: u64) -> usize {
        64 - (k ^ self.last).leading_zeros() as usize
    }

    fn push(&mut self, k: u64) {
        debug_assert!(k >= self.last, "radix heap keys must not decrease");
        let b = self.bucket(k);
        self.buckets[b].push(k);
        self.len += 1;
    }

    fn pop(&mut self) -> Option<u64> {
        if self.len == 0 {
            return None;
        }
        if self.buckets[0].is_empty() {
            let i = self.buckets.iter().position(|b| !b.is_empty())?;
            let moved = std::mem::take(&mut self.buckets[i]);
            self.last = moved.iter().copied().min()?;
            for k in &moved {
                let b = self.bucket(*k);
                self.buckets[b].push(*k);
            }
            // Hand the allocation back to the emptied bucket.
            let mut spare = moved;
            spare.clear();
            if self.buckets[i].is_empty() {
                self.buckets[i] = spare;
            }
        }
        self.len -= 1;
        self.buckets[0].pop()
    }
}

/// Flow fields by building, with a use stamp for LRU eviction. Not saved.
#[derive(Clone, Debug, Default)]
pub struct FlowCache {
    fields: BTreeMap<EntityId, (FlowField, u64)>,
    clock: u64,
}

impl FlowCache {
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn contains(&self, b: EntityId) -> bool {
        self.fields.contains_key(&b)
    }

    pub fn clear(&mut self) {
        self.fields.clear();
    }

    /// A building went up on a Lot: `sealed` is its new walls plus its
    /// interior (reachable only through its door now). Drop only the fields
    /// in which a tile outside `sealed` steps into it. In every other field
    /// the sealed tiles were dead ends of the Dijkstra tree for every tile a
    /// mover can stand on (movers leave a building through its door before
    /// stepping, and nobody stands on a wall), so those tiles' directions
    /// are byte-identical to a rebuild on the new map: keeping the field
    /// changes nothing and spares the rebuilds.
    pub fn invalidate_sealed(&mut self, tiles: &[TilePos], sealed: &dyn Fn(TilePos) -> bool) {
        self.fields.retain(|_, (f, _)| !tiles.iter().any(|&t| f.entered_from_outside(t, sealed)));
    }

    /// The cached field for `b`, stamped as just used.
    pub fn get(&mut self, b: EntityId) -> Option<&FlowField> {
        self.clock += 1;
        let clock = self.clock;
        self.fields.get_mut(&b).map(|(f, stamp)| {
            *stamp = clock;
            &*f
        })
    }

    /// Insert a field, evicting the least recently used past `cap` (min 1).
    pub fn insert(&mut self, b: EntityId, field: FlowField, cap: usize) {
        while self.fields.len() >= cap.max(1) && !self.fields.contains_key(&b) {
            let oldest = self.fields.iter().min_by_key(|(&id, (_, stamp))| (*stamp, id)).map(|(&id, _)| id);
            match oldest {
                Some(id) => {
                    self.fields.remove(&id);
                }
                None => break,
            }
        }
        self.clock += 1;
        self.fields.insert(b, (field, self.clock));
    }
}
