//! One direction field per building door, built by Dijkstra from the door
//! with the same entry costs A* uses, so descending the field reproduces an
//! optimal A* path cost in O(1) per step.
//!
//! M10: a field stores one direction byte per tile (49 KB at 256 x 192) and
//! no cost array after the build; fields are built lazily and kept in an LRU
//! ([`FlowCache`]). A field is a pure function of the map and its door, so
//! eviction never changes results.
//!
//! Scale S3a: the LRU is also capped by bytes (`[exec] flow_field_budget_mb`),
//! and maps past `[exec] flow_field_max_tiles` walk by exact A* paths
//! instead (`exec::bigpath`), whose labels and landmarks the cache holds.

use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::{Arc, OnceLock};

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

/// The cost of entering `t` as [`FlowField::build`] reads it (rubble is a
/// wall while it lies there).
fn enter_cost(map: &Map, t: TilePos) -> f32 {
    if map.blocked.contains(&t) {
        f32::INFINITY
    } else {
        map.tile_at(t).move_cost()
    }
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

    /// Build the field for `door`: Dijkstra in `(cost, tile)` order over cost
    /// buckets ([`CostQueue`]; the pop order, so every direction byte, is
    /// the binary heap's; see
    /// [`FlowField::build_binary_heap`] and `tests/exec.rs`).
    pub fn build(map: &Map, door: TilePos) -> FlowField {
        let (w, h) = (map.w(), map.h());
        let n = w * h;
        let mut f = FlowField { next: vec![0; n], w: w as u16, door: Some(door) };
        // Per tile, flat: the cost of entering it, infinite exactly where it
        // is not walkable (Wall, Water). The same Dijkstra as
        // `build_binary_heap`, with the per-neighbour tile matches and bounds
        // iterator hoisted out of the loop (perf).
        let mut enter: Vec<f32> = map.tiles().iter().map(|t| t.move_cost()).collect();
        // M12 D19: rubble is a wall while it lies there.
        for &p in &map.blocked {
            let i = usize::from(p.y) * w + usize::from(p.x);
            if let Some(e) = enter.get_mut(i) {
                *e = f32::INFINITY;
            }
        }
        let mut cost = vec![f32::INFINITY; n];
        let mut open = CostQueue::new();
        cost[f.idx(door)] = 0.0;
        open.push(key(0.0, door));
        // The byte a neighbour gets points back at `cur`: W of cur steps E, etc.
        let (to_e, to_w, to_s, to_n) = (
            dir_byte(TilePos { x: 0, y: 0 }, TilePos { x: 1, y: 0 }),
            dir_byte(TilePos { x: 1, y: 0 }, TilePos { x: 0, y: 0 }),
            dir_byte(TilePos { x: 0, y: 0 }, TilePos { x: 0, y: 1 }),
            dir_byte(TilePos { x: 0, y: 1 }, TilePos { x: 0, y: 0 }),
        );
        while let Some(k) = open.pop() {
            let (c, cur) = unkey(k);
            let (x, y) = (usize::from(cur.x), usize::from(cur.y));
            let ci = y * w + x;
            if c > cost[ci] {
                continue;
            }
            // Travelling from n to cur enters cur, so n's cost = cost(cur) + dist(cur).
            let nc = c + enter[ci];
            let mut relax = |i: usize, nx: usize, ny: usize, byte: u8| {
                if enter[i].is_finite() && nc < cost[i] {
                    cost[i] = nc;
                    f.next[i] = byte;
                    open.push(key(nc, TilePos { x: nx as u16, y: ny as u16 }));
                }
            };
            // W, E, N, S: the order of `Map::neighbours4`.
            if x > 0 {
                relax(ci - 1, x - 1, y, to_e);
            }
            if x + 1 < w {
                relax(ci + 1, x + 1, y, to_w);
            }
            if y > 0 {
                relax(ci - w, x, y - 1, to_s);
            }
            if y + 1 < h {
                relax(ci + w, x, y + 1, to_n);
            }
        }
        f
    }

    /// The M10 build (binary heap of `(cost, tile)`), kept as the reference
    /// the bucketed build is tested against.
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
        Some(TilePos { x: (i32::from(p.x) + dx) as u16, y: (i32::from(p.y) + dy) as u16 })
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
            let from = TilePos { x: x as u16, y: y as u16 };
            !sealed(from) && self.step(from) == Some(t)
        })
    }

    /// The cost [`FlowField::build`] gives `p` on `map`, or `None` where the
    /// field never reached it. The build sets a tile's cost to its
    /// successor's plus the successor's entry cost, so summing the entries
    /// back along the descent from the door repeats its float additions
    /// exactly. Valid for tiles whose descent `map` has not changed.
    fn build_cost(&self, map: &Map, p: TilePos) -> Option<f32> {
        if self.door == Some(p) {
            return Some(0.0);
        }
        let path = self.descend(p);
        if path.last() != self.door.as_ref() {
            return None;
        }
        // cost(t_k) = cost(t_k+1) + enter(t_k+1), from the door (cost 0) back to `p`.
        Some(path.iter().rev().fold(0.0f32, |c, &t| c + enter_cost(map, t)))
    }

    /// OPEN-112: a building went up on a Lot (`rect`, its door `door`; the
    /// map already walled) and this field was kept (no tile outside the
    /// rect steps into its walls or interior): rewrite the rect's bytes to
    /// what [`FlowField::build`] gives on the new map. The walls get none;
    /// the interior is reached only through `door`, so its bytes come from
    /// the same Dijkstra (the same `(cost, x, y)` pop order and strict
    /// relax) seeded at the door's cost, which the descent repeats exactly.
    pub fn reseal(&mut self, map: &Map, rect: crate::components::Rect, door: TilePos) {
        let inside = |t: TilePos| rect.contains(t) && t != door;
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                let t = TilePos { x, y };
                if inside(t) {
                    let i = self.idx(t);
                    self.next[i] = 0;
                }
            }
        }
        let Some(c_door) = self.build_cost(map, door) else { return };
        let mut cost: BTreeMap<TilePos, f32> = BTreeMap::new();
        let mut open = CostQueue::new();
        open.push(key(c_door, door));
        while let Some(k) = open.pop() {
            let (c, cur) = unkey(k);
            if cur != door && cost.get(&cur).is_some_and(|&x| c > x) {
                continue;
            }
            let nc = c + enter_cost(map, cur);
            for nb in map.neighbours4(cur) {
                if !inside(nb) || !enter_cost(map, nb).is_finite() {
                    continue;
                }
                if cost.get(&nb).is_none_or(|&x| nc < x) {
                    cost.insert(nb, nc);
                    let i = self.idx(nb);
                    self.next[i] = dir_byte(nb, cur);
                    open.push(key(nc, nb));
                }
            }
        }
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
    (u64::from(cost.to_bits()) << 32) | (u64::from(t.x) << 16) | u64::from(t.y)
}

fn unkey(k: u64) -> (f32, TilePos) {
    (f32::from_bits((k >> 32) as u32), TilePos { x: (k >> 16) as u16, y: k as u16 })
}

/// Dijkstra's open set as cost buckets: the tiles pushed at one cost are
/// sorted when that cost comes up and popped in `(cost, x, y)` key order,
/// exactly the order of a heap of the keys. Valid because every push lands at
/// a strictly higher cost than the one being popped (each step costs at least
/// 0.7), so a bucket is complete when it is reached. (M11's radix heap
/// re-bucketed on nearly every pop, as the tile bits differ within a cost.)
struct CostQueue {
    /// `cost bits -> tiles (x << 16 | y)` not yet reached.
    pending: BTreeMap<u32, Vec<u32>>,
    /// The bucket being popped, sorted, and the next position in it.
    cur: Vec<u32>,
    cur_cost: u64,
    pos: usize,
    spare: Vec<Vec<u32>>,
}

impl CostQueue {
    fn new() -> CostQueue {
        CostQueue { pending: BTreeMap::new(), cur: Vec::new(), cur_cost: 0, pos: 0, spare: Vec::new() }
    }

    fn push(&mut self, k: u64) {
        let cost = (k >> 32) as u32;
        debug_assert!(self.cur.is_empty() || u64::from(cost) > self.cur_cost, "costs must rise past the bucket");
        let spare = &mut self.spare;
        self.pending.entry(cost).or_insert_with(|| spare.pop().unwrap_or_default()).push(k as u32);
    }

    fn pop(&mut self) -> Option<u64> {
        if self.pos == self.cur.len() {
            let (cost, mut tiles) = self.pending.pop_first()?;
            tiles.sort_unstable();
            let mut done = std::mem::replace(&mut self.cur, tiles);
            done.clear();
            self.spare.push(done);
            self.cur_cost = u64::from(cost);
            self.pos = 0;
        }
        let t = self.cur[self.pos];
        self.pos += 1;
        Some((self.cur_cost << 32) | u64::from(t))
    }
}

/// Flow fields by building, with a use stamp for LRU eviction. Not saved.
/// Scale S3a: also the big-map paths' component labels
/// (`exec::bigpath::labels`, dropped with the fields on every wall or
/// rubble change) and landmarks (`exec::bigpath::Landmarks`, dropped only
/// when a tile change opened a way: `keep_landmarks_if`), each built on
/// first use.
#[derive(Clone, Debug, Default)]
pub struct FlowCache {
    fields: BTreeMap<EntityId, (FlowField, u64)>,
    clock: u64,
    labels: OnceLock<Arc<Vec<u32>>>,
    landmarks: OnceLock<Arc<crate::exec::bigpath::Landmarks>>,
    /// Review finding 10: the tiles the next landmarks are measured on
    /// when not the map's (a loaded world's `landmark_base`).
    base: Option<Arc<Vec<crate::components::TileKind>>>,
}

impl FlowCache {
    /// Scale S3a: the fields kept for a map of `tiles` tiles: the configured
    /// count (`[exec] flow_field_cache`), capped so the fields' bytes stay
    /// within `budget_bytes` (`[exec] flow_field_budget_mb`); at least one.
    pub fn cap_for(count: usize, budget_bytes: usize, tiles: usize) -> usize {
        count.min(budget_bytes / tiles.max(1)).max(1)
    }

    /// The fields' direction bytes held.
    pub fn bytes(&self) -> usize {
        self.fields.values().map(|(f, _)| f.next.len()).sum()
    }

    /// The big-map component labels for `map`, built on first use after a
    /// wall change.
    pub fn labels(&self, map: &Map) -> Arc<Vec<u32>> {
        self.labels.get_or_init(|| Arc::new(crate::exec::bigpath::labels(map))).clone()
    }

    /// The big-map landmarks, built on first use after a tile change
    /// opened a way: on `map`'s tiles, or on the loaded base's.
    pub fn landmarks(&self, map: &Map) -> Arc<crate::exec::bigpath::Landmarks> {
        self.landmarks
            .get_or_init(|| {
                Arc::new(crate::prof::timed!(
                    "exec::landmarks",
                    match &self.base {
                        Some(t) => crate::exec::bigpath::Landmarks::build_on(map, t.clone()),
                        None => crate::exec::bigpath::Landmarks::build(map),
                    }
                ))
            })
            .clone()
    }

    /// The tiles the landmarks were (or, loaded, will be) measured on.
    fn landmark_tiles(&self) -> Option<Arc<Vec<crate::components::TileKind>>> {
        self.landmarks.get().map(|l| l.base_tiles().clone()).or_else(|| self.base.clone())
    }

    /// After a wall change: keep the landmarks while the tiles since they
    /// were measured only closed ways (rubble is not a tile change; review
    /// finding 10: a closure keeps them admissible), drop them when one
    /// opened (rebuilt on the tiles of their next use). Either way they are
    /// a pure function of the tiles they were measured on.
    pub fn keep_landmarks_if(&mut self, map: &Map) {
        let Some(base) = self.landmark_tiles() else { return };
        if base.as_slice() != map.tiles() && !crate::exec::bigpath::only_closed(&base, map) {
            self.landmarks = OnceLock::new();
            self.base = None;
        }
    }

    /// Review finding 10: the landmarks' tiles while they differ from
    /// `map`'s (what a save carries: `World::landmark_base`).
    pub fn stale_base(&self, map: &Map) -> Option<Arc<Vec<crate::components::TileKind>>> {
        self.landmark_tiles().filter(|t| t.as_slice() != map.tiles())
    }

    /// Review finding 10 (a load): the next landmarks are measured on
    /// `base` (`None`: the map's tiles).
    pub fn set_landmark_base(&mut self, base: Option<Arc<Vec<crate::components::TileKind>>>) {
        self.landmarks = OnceLock::new();
        self.base = base;
    }

    /// The labels' and landmarks' bytes held (the bench's read).
    pub fn index_bytes(&self) -> usize {
        self.labels.get().map_or(0, |l| l.len() * 4) + self.landmarks.get().map_or(0, |l| l.bytes())
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// The cached fields by building (OPEN-112's test: each equals a rebuild).
    pub fn fields(&self) -> impl Iterator<Item = (EntityId, &FlowField)> + '_ {
        self.fields.iter().map(|(&b, (f, _))| (b, f))
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn contains(&self, b: EntityId) -> bool {
        self.fields.contains_key(&b)
    }

    pub fn clear(&mut self) {
        self.fields.clear();
        self.labels = OnceLock::new();
    }

    /// A building went up on a Lot (`rect`, its door `door`, the map already
    /// walled): `sealed` is its new walls plus its interior (reachable only
    /// through its door now). Drop only the fields in which a tile outside
    /// `sealed` steps into it. In every other field the sealed tiles were
    /// dead ends of the Dijkstra tree, so every tile outside them keeps its
    /// cost and its direction byte on the new map, and only the rect's own
    /// bytes are stale: [`FlowField::reseal`] rewrites them to a rebuild's,
    /// so a kept field is byte-identical to a rebuild and spares it.
    /// OPEN-112: before the reseal the rect kept the open Lot's bytes, and a
    /// body standing in it (a stored path walked through the new walls)
    /// stepped by them while a loaded world, building afresh, stepped by the
    /// rebuild's: a save did not resume byte for byte.
    pub fn invalidate_sealed(
        &mut self,
        map: &Map,
        rect: crate::components::Rect,
        door: TilePos,
        tiles: &[TilePos],
        sealed: &dyn Fn(TilePos) -> bool,
    ) {
        self.labels = OnceLock::new();
        self.fields.retain(|_, (f, _)| !tiles.iter().any(|&t| f.entered_from_outside(t, sealed)));
        for (f, _) in self.fields.values_mut() {
            f.reseal(map, rect, door);
        }
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
