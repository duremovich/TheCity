//! Scale S3a (docs/SCALE_20K.md): exact paths on big maps.
//!
//! A flow field is one byte a tile and a full-map Dijkstra to build: 49 KB
//! and 0.3 ms on the 2k city's 256 × 192, 5.5 MB and ~80 ms on the big
//! island's 3,000 × 1,875 (the 512-field LRU would hold ~2.9 GB). Above
//! `[exec] flow_field_max_tiles` a Full walk instead asks [`find`] once for
//! its whole path and carries it in its `Goto` (saved with it, so a load
//! walks on byte for byte):
//!
//! - **A\*, exact.** Costs in tenths (`Road` 7, `Ground`/`Door` 10,
//!   `Farmland` 12, `Mountain` 15: the f32 move costs × 10, so sums are
//!   exact). The first pop of the goal is a shortest path, the cost a flow
//!   field's descent has. Ties pop the deeper node first (the larger `g`),
//!   then the lower tile index.
//! - **Landmarks (ALT).** 7 × Manhattan alone floods: every road tile in the
//!   box between the ends has `f` below the true cost as soon as the path
//!   pays for one Ground or Door tile (a cross-island walk expanded ~10⁶
//!   tiles, 130 ms). [`Landmarks`] holds, for 8 tiles near the corners and
//!   the edge midpoints, every tile's exact distance from it (u16 tenths),
//!   and the heuristic is the largest of 7 × Manhattan and the triangle
//!   bounds `d(L, goal) − d(L, n)` and `d(n, L) − d(goal, L)`: admissible
//!   and consistent. The landmarks are measured on the tiles without the
//!   rubble, so rubble (which only closes ways) never stales them. Review
//!   finding 10: a tile change that only closes ways (a Lot's walls: every
//!   tile's entry cost the same or higher) keeps them too (distances on the
//!   old tiles are lower bounds on the new: still admissible and
//!   consistent, ~1 s a rebuild spared on the 3,000 x 1,875 island); one
//!   that opens a way (a demolition) drops them, rebuilt on the tiles of
//!   their next use. They are a pure function of the tiles they were
//!   measured on (`base_tiles`), which a save carries while they differ
//!   from the map's (`World::landmark_base`), so a loaded world searches
//!   with the landmarks the saved one had and its paths tie-break alike.
//! - **Unreachable is O(1).** [`labels`] gives every walkable tile its
//!   4-connected component; a goal in another component is `None` without a
//!   search. Rebuilt on every wall or rubble change (`FlowCache::labels`).
//! - **Bounded memory.** One search scratch per thread, reused with a
//!   generation stamp (9 bytes a tile), the labels (4), the landmarks (16)
//!   and their tile snapshot (1): [`BYTES_PER_TILE`], ~160 MB at 3,000 ×
//!   1,875.
//!
//! The commute estimates (`transit::commute_ticks`, `exec::timed_walk`, the
//! hiring and relocation distances) stay Manhattan, as below the threshold.

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use crate::components::{TileKind, TilePos};
use crate::map::Map;

/// A tile's entry cost in tenths of a step; `u32::MAX` where nobody walks.
pub fn cost_tenths(kind: TileKind) -> u32 {
    match kind {
        TileKind::Road => 7,
        TileKind::Ground | TileKind::Door => 10,
        TileKind::Farmland => 12,
        TileKind::Mountain => 15,
        TileKind::Wall | TileKind::Water => u32::MAX,
    }
}

/// The cheapest entry cost: the Manhattan heuristic's step.
const MIN_STEP: u32 = 7;

/// Not walkable (a wall, water or rubble) in [`labels`].
pub const NO_LABEL: u32 = u32::MAX;

/// Landmarks kept ([`Landmarks`]).
pub const LANDMARKS: usize = 8;

/// No distance (another component, or past `u16`) in [`Landmarks`].
const FAR: u16 = u16::MAX;

/// Big-map pathing state, bytes per tile: the search scratch (stamp 4, `g`
/// 4, parent 1), the labels (4), the landmarks (2 × [`LANDMARKS`]) and
/// their tile snapshot (1), rounded up.
pub const BYTES_PER_TILE: usize = 32;

/// Each walkable tile's 4-connected component (row-major), [`NO_LABEL`]
/// elsewhere. Components are numbered in row-major order of their first
/// tile, so the labelling is a pure function of the map.
pub fn labels(map: &Map) -> Vec<u32> {
    let (w, h) = (map.w(), map.h());
    let mut walk: Vec<bool> = map.tiles().iter().map(|t| t.walkable()).collect();
    for &p in &map.blocked {
        if let Some(b) = walk.get_mut(usize::from(p.y) * w + usize::from(p.x)) {
            *b = false;
        }
    }
    let mut out = vec![NO_LABEL; w * h];
    let mut next = 0u32;
    let mut stack: Vec<usize> = Vec::new();
    for s in 0..w * h {
        if !walk[s] || out[s] != NO_LABEL {
            continue;
        }
        out[s] = next;
        stack.push(s);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if walk[j] && out[j] == NO_LABEL {
                    out[j] = next;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        next += 1;
    }
    out
}

/// The ALT landmarks of a map's tiles (rubble ignored): per tile, its
/// distance in tenths from each landmark, `LANDMARKS` u16 side by side.
#[derive(Clone, Debug, Default)]
pub struct Landmarks {
    /// The tiles the distances were measured on (`same_tiles`).
    tiles: Arc<Vec<TileKind>>,
    pub points: Vec<TilePos>,
    /// `dist[i * LANDMARKS + k]`: tile `i` from landmark `k`, [`FAR`] if
    /// unreached or too far for a u16.
    dist: Vec<u16>,
}

impl Landmarks {
    /// No landmarks: the search runs on 7 × Manhattan alone.
    pub fn none() -> Landmarks {
        Landmarks::default()
    }

    /// Measure the landmarks on `map`'s tiles: the walkable tiles nearest
    /// the four corners and the four edge midpoints (ties the lower index),
    /// each a Dijkstra over the whole map.
    pub fn build(map: &Map) -> Landmarks {
        Landmarks::build_on(map, Arc::new(map.tiles().to_vec()))
    }

    /// [`Landmarks::build`] on `tiles` (`map`'s size): a loaded world's
    /// landmarks measured before a closure (review finding 10).
    pub fn build_on(map: &Map, tiles: Arc<Vec<TileKind>>) -> Landmarks {
        let (w, h) = (map.w(), map.h());
        assert_eq!(tiles.len(), w * h, "landmark tiles of another map");
        let (mx, my) = ((w - 1) as i64, (h - 1) as i64);
        let aims = [(0, 0), (mx, 0), (0, my), (mx, my), (mx / 2, 0), (mx / 2, my), (0, my / 2), (mx, my / 2)];
        let mut points = Vec::with_capacity(LANDMARKS);
        for &(ax, ay) in aims.iter().take(LANDMARKS) {
            let best = (0..w * h)
                .filter(|&i| tiles[i].walkable())
                .min_by_key(|&i| (((i % w) as i64 - ax).abs() + ((i / w) as i64 - ay).abs(), i));
            if let Some(i) = best {
                points.push(TilePos::new((i % w) as u16, (i / w) as u16));
            }
        }
        let mut dist = vec![FAR; w * h * LANDMARKS];
        let mut d = Vec::new();
        for (k, &p) in points.iter().enumerate() {
            dial(&tiles, w, h, map.index(p), &mut d);
            for (i, &v) in d.iter().enumerate() {
                dist[i * LANDMARKS + k] = u16::try_from(v).unwrap_or(FAR);
            }
        }
        Landmarks { tiles, points, dist }
    }

    /// Were these landmarks measured on exactly `map`'s tiles?
    pub fn same_tiles(&self, map: &Map) -> bool {
        self.tiles.as_slice() == map.tiles()
    }

    /// The tiles the distances were measured on.
    pub fn base_tiles(&self) -> &Arc<Vec<TileKind>> {
        &self.tiles
    }

    pub fn bytes(&self) -> usize {
        self.dist.len() * 2 + self.tiles.len()
    }
}

/// Review finding 10: has `map` only closed ways since `base` (every tile's
/// entry cost the same or higher, a wall where there was none)? Then
/// landmarks measured on `base` stay admissible and consistent on `map`.
pub fn only_closed(base: &[TileKind], map: &Map) -> bool {
    base.len() == map.tiles().len() && base.iter().zip(map.tiles()).all(|(&a, &b)| cost_tenths(b) >= cost_tenths(a))
}

/// Review finding 10: the tiles a world's landmarks were measured on,
/// saved (as the map's run-length string) while they differ from the
/// map's (`World::landmark_base`).
#[derive(Clone, Debug, PartialEq)]
pub struct BaseTiles(pub Arc<Vec<TileKind>>);

impl serde::Serialize for BaseTiles {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&crate::map::rle_encode(self.0.iter().map(|&t| crate::map::tile_char(t))))
    }
}

impl<'de> serde::Deserialize<'de> for BaseTiles {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let chars = crate::map::rle_decode(&text).map_err(serde::de::Error::custom)?;
        let tiles = chars
            .into_iter()
            .map(|c| crate::map::tile_of_char(c).ok_or_else(|| serde::de::Error::custom(format!("bad tile {c:?}"))))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BaseTiles(Arc::new(tiles)))
    }
}

/// Dijkstra from `src` over `tiles` (rubble ignored) by entry cost, into
/// `d` (`u32::MAX` unreached): Dial's buckets, the costs being 7-15 (each
/// under `B`, so a push never lands in the bucket being popped).
fn dial(tiles: &[TileKind], w: usize, h: usize, src: usize, d: &mut Vec<u32>) {
    const B: usize = 16;
    d.clear();
    d.resize(w * h, u32::MAX);
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); B];
    d[src] = 0;
    buckets[0].push(src as u32);
    let (mut cur, mut pending) = (0u32, 1usize);
    while pending > 0 {
        let slot = cur as usize % B;
        while let Some(i) = buckets[slot].pop() {
            pending -= 1;
            let i = i as usize;
            if d[i] != cur {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let mut relax = |j: usize| {
                let c = cost_tenths(tiles[j]);
                if c != u32::MAX && cur + c < d[j] {
                    d[j] = cur + c;
                    buckets[(cur + c) as usize % B].push(j as u32);
                    pending += 1;
                }
            };
            if x > 0 {
                relax(i - 1);
            }
            if x + 1 < w {
                relax(i + 1);
            }
            if y > 0 {
                relax(i - w);
            }
            if y + 1 < h {
                relax(i + w);
            }
        }
        cur += 1;
    }
}

/// Per-thread search state, reused across queries: a tile's `g` and parent
/// are valid only where its stamp is the current query's.
#[derive(Default)]
struct Scratch {
    stamp: Vec<u32>,
    g: Vec<u32>,
    /// 1..=4: the step from the parent (W, E, N, S); 0 at the start.
    from: Vec<u8>,
    query: u32,
    /// `(Reverse(f), g, Reverse(index))`: the lowest `f`, then the deepest,
    /// then the lowest index.
    open: BinaryHeap<(Reverse<u32>, u32, Reverse<u32>)>,
}

impl Scratch {
    fn fit(&mut self, n: usize) {
        if self.stamp.len() != n {
            self.stamp = vec![0; n];
            self.g = vec![0; n];
            self.from = vec![0; n];
            self.query = 0;
        }
        self.query = self.query.wrapping_add(1);
        if self.query == 0 {
            self.stamp.iter_mut().for_each(|s| *s = 0);
            self.query = 1;
        }
        self.open.clear();
    }

    fn bytes(&self) -> usize {
        self.stamp.capacity() * 4
            + self.g.capacity() * 4
            + self.from.capacity()
            + self.open.capacity() * std::mem::size_of::<(Reverse<u32>, u32, Reverse<u32>)>()
    }
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
    static RECORD: RefCell<Option<Vec<(TilePos, TilePos)>>> = const { RefCell::new(None) };
    static EXPANDED: Cell<u64> = const { Cell::new(0) };
}

/// This thread's search scratch, in bytes (the memory test's and the bench's read).
pub fn scratch_bytes() -> usize {
    SCRATCH.with(|s| s.borrow().bytes())
}

/// Tiles expanded by [`find`] on this thread so far (the bench's read).
pub fn expanded() -> u64 {
    EXPANDED.with(Cell::get)
}

/// The bench's hook: record every [`find`] query on this thread from now
/// (`true`) or stop (`false`); [`take_recorded`] hands them over. A
/// measurement of the host only: nothing in the world reads it.
pub fn record(on: bool) {
    RECORD.with(|r| *r.borrow_mut() = on.then(Vec::new));
}

/// The queries recorded since [`record`]`(true)`, `(start, goal)`.
pub fn take_recorded() -> Vec<(TilePos, TilePos)> {
    RECORD.with(|r| r.borrow_mut().as_mut().map(std::mem::take).unwrap_or_default())
}

/// The shortest path from `start` (exclusive) to `goal` (inclusive) over
/// walkable, unblocked tiles, by entry cost; `None` when there is none.
/// `labels` is [`labels`] of this map; `marks` landmarks measured on these
/// tiles or on more open ones (any [`Landmarks`] of a map this one only
/// closed, or [`Landmarks::none`]). `start` itself need not be walkable (a
/// body on fresh rubble walks off it).
pub fn find(map: &Map, labels: &[u32], marks: &Landmarks, start: TilePos, goal: TilePos) -> Option<Vec<TilePos>> {
    RECORD.with(|r| {
        if let Some(v) = r.borrow_mut().as_mut() {
            v.push((start, goal));
        }
    });
    if start == goal {
        return Some(Vec::new());
    }
    let (w, h) = (map.w(), map.h());
    let gi = map.index(goal);
    let goal_label = *labels.get(gi)?;
    if goal_label == NO_LABEL || !map.walkable(goal) {
        return None;
    }
    let si = map.index(start);
    let connected = labels.get(si) == Some(&goal_label)
        || map.neighbours4(start).any(|n| labels.get(map.index(n)) == Some(&goal_label));
    if !connected {
        return None;
    }
    let tiles = map.tiles();
    let blocked = !map.blocked.is_empty();
    let enter = |i: usize| -> u32 {
        let c = cost_tenths(tiles[i]);
        if blocked && c != u32::MAX && map.is_blocked(TilePos::new((i % w) as u16, (i / w) as u16)) {
            u32::MAX
        } else {
            c
        }
    };
    let alt = marks.dist.len() == w * h * LANDMARKS;
    let goal_marks: [u16; LANDMARKS] =
        if alt { std::array::from_fn(|k| marks.dist[gi * LANDMARKS + k]) } else { [FAR; LANDMARKS] };
    // On the landmarks' tiles: d(n, L) = d(L, n) − enter(n) + enter(L) (the
    // same optimal path walked the other way pays for L, not for n), so
    // d(n, L) − d(goal, L) = d(L, n) + enter(goal) − d(L, goal) − enter(n).
    let goal_enter = if alt { cost_tenths(marks.tiles[gi]) } else { u32::MAX };
    let (gx, gy) = (i64::from(goal.x), i64::from(goal.y));
    let heur = |i: usize| -> u32 {
        let (x, y) = ((i % w) as i64, (i / w) as i64);
        let mut best = ((x - gx).unsigned_abs() + (y - gy).unsigned_abs()) as u32 * MIN_STEP;
        if alt {
            let here = &marks.dist[i * LANDMARKS..(i + 1) * LANDMARKS];
            let e = cost_tenths(marks.tiles[i]);
            for (&a, &b) in goal_marks.iter().zip(here) {
                if a == FAR || b == FAR {
                    continue;
                }
                let (a, b) = (u32::from(a), u32::from(b));
                // d(n, goal) >= d(L, goal) − d(L, n).
                best = best.max(a.saturating_sub(b));
                // d(n, goal) >= d(n, L) − d(goal, L).
                if e != u32::MAX && goal_enter != u32::MAX {
                    best = best.max((b + goal_enter).saturating_sub(a + e));
                }
            }
        }
        best
    };
    let mut expanded = 0u64;
    let path = SCRATCH.with(|cell| {
        let mut s = cell.borrow_mut();
        let s = &mut *s;
        s.fit(w * h);
        let q = s.query;
        s.stamp[si] = q;
        s.g[si] = 0;
        s.from[si] = 0;
        s.open.push((Reverse(heur(si)), 0, Reverse(si as u32)));
        while let Some((Reverse(_), g, Reverse(ci))) = s.open.pop() {
            let ci = ci as usize;
            if g > s.g[ci] {
                continue; // a stale entry
            }
            if ci == gi {
                return Some(walk_back(s, w, si, gi));
            }
            expanded += 1;
            let (x, y) = (ci % w, ci / w);
            // W, E, N, S: `Map::neighbours4`'s order; the byte is the step taken.
            let mut relax = |ni: usize, byte: u8| {
                let c = enter(ni);
                if c == u32::MAX {
                    return;
                }
                let ng = g + c;
                if s.stamp[ni] != q || ng < s.g[ni] {
                    s.stamp[ni] = q;
                    s.g[ni] = ng;
                    s.from[ni] = byte;
                    s.open.push((Reverse(ng + heur(ni)), ng, Reverse(ni as u32)));
                }
            };
            if x > 0 {
                relax(ci - 1, 1);
            }
            if x + 1 < w {
                relax(ci + 1, 2);
            }
            if y > 0 {
                relax(ci - w, 3);
            }
            if y + 1 < h {
                relax(ci + w, 4);
            }
        }
        None
    });
    EXPANDED.with(|e| e.set(e.get() + expanded));
    path
}

/// The path from the start (exclusive) to `gi` (inclusive), by the parent bytes.
fn walk_back(s: &Scratch, w: usize, si: usize, gi: usize) -> Vec<TilePos> {
    let mut out = Vec::new();
    let mut i = gi;
    while i != si {
        out.push(TilePos::new((i % w) as u16, (i / w) as u16));
        i = match s.from[i] {
            1 => i + 1,
            2 => i - 1,
            3 => i + w,
            4 => i - w,
            _ => break,
        };
    }
    out.reverse();
    out
}
