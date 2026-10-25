//! Scale S1: a flat bucket grid over the map's tiles, for "who is within r
//! tiles of here" without a scan of everyone. Items are the caller's indices
//! (`u32`) into its own list; a query returns a superset (every item in a
//! cell the square of radius `r` touches), which the caller filters by its
//! exact rule, so a grid query is the same answer as the scan it replaces.
//! No RNG, no hashing: the grid is rebuilt from its items when they move.

use crate::components::TilePos;

/// The perf pass phase 3 (the world build): in a unit test, the indexed
/// reads of the world build (relocation's Homes, the labour pool, the graded
/// pick, the free beds, the housing build's Lots and storeys) fall back to
/// the scans they replaced, so a test builds the city both ways and
/// compares (`life::tests::test_indexed_build_matches_the_scans`). Always
/// false outside the crate's unit tests.
#[cfg(test)]
pub fn scans() -> bool {
    SCANS.with(|c| c.get())
}

/// [`scans`] outside the unit tests: never.
#[cfg(not(test))]
pub const fn scans() -> bool {
    false
}

#[cfg(test)]
thread_local! {
    /// [`scans`]' switch, per test thread.
    pub static SCANS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Items bucketed by `cell`-sized squares of tiles (a counting sort: one
/// `Vec` of items in cell order and each cell's start).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileGrid {
    cell: u32,
    gw: usize,
    gh: usize,
    /// `starts[c]..starts[c + 1]` are cell `c`'s entries in `items`.
    starts: Vec<u32>,
    items: Vec<u32>,
}

impl TileGrid {
    /// A grid over a `w` x `h` map with `cell`-tile squares (at least 1),
    /// holding `points[i]` as item `i`.
    pub fn build(w: usize, h: usize, cell: u32, points: impl Iterator<Item = TilePos> + Clone) -> TileGrid {
        let cell = cell.max(1);
        let gw = w.div_ceil(cell as usize).max(1);
        let gh = h.div_ceil(cell as usize).max(1);
        let mut g = TileGrid { cell, gw, gh, starts: vec![0; gw * gh + 1], items: Vec::new() };
        let mut n = 0u32;
        for t in points.clone() {
            let c = g.cell_of(t);
            g.starts[c + 1] += 1;
            n += 1;
        }
        for c in 0..gw * gh {
            g.starts[c + 1] += g.starts[c];
        }
        let mut fill = g.starts.clone();
        g.items = vec![0; n as usize];
        for (i, t) in points.enumerate() {
            let c = g.cell_of(t);
            g.items[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        g
    }

    fn cxy(&self, t: TilePos) -> (usize, usize) {
        let cx = (usize::from(t.x) / self.cell as usize).min(self.gw - 1);
        let cy = (usize::from(t.y) / self.cell as usize).min(self.gh - 1);
        (cx, cy)
    }

    fn cell_of(&self, t: TilePos) -> usize {
        let (cx, cy) = self.cxy(t);
        cy * self.gw + cx
    }

    /// Is any item in a cell [`TileGrid::near`] would read?
    pub fn any_near(&self, t: TilePos, r: u32) -> bool {
        let c = self.cell as usize;
        let (x, y) = (usize::from(t.x), usize::from(t.y));
        let r = r as usize;
        let x0 = (x.saturating_sub(r) / c).min(self.gw - 1);
        let x1 = ((x + r) / c).min(self.gw - 1);
        let y0 = (y.saturating_sub(r) / c).min(self.gh - 1);
        let y1 = ((y + r) / c).min(self.gh - 1);
        (y0..=y1).any(|cy| self.starts[cy * self.gw + x0] != self.starts[cy * self.gw + x1 + 1])
    }

    /// Every item in a cell that the Chebyshev square of radius `r` around
    /// `t` touches (a superset of the items within `r`), appended to `out`
    /// in cell order (callers needing an order sort it).
    pub fn near(&self, t: TilePos, r: u32, out: &mut Vec<u32>) {
        let c = self.cell as usize;
        let (x, y) = (usize::from(t.x), usize::from(t.y));
        let r = r as usize;
        let x0 = (x.saturating_sub(r) / c).min(self.gw - 1);
        let x1 = ((x + r) / c).min(self.gw - 1);
        let y0 = (y.saturating_sub(r) / c).min(self.gh - 1);
        let y1 = ((y + r) / c).min(self.gh - 1);
        for cy in y0..=y1 {
            let row = cy * self.gw;
            let (a, b) = (self.starts[row + x0] as usize, self.starts[row + x1 + 1] as usize);
            out.extend_from_slice(&self.items[a..b]);
        }
    }

    /// The perf pass phase 3: every item in a cell that the square of any
    /// `(centre, radius)` disc touches (a superset of the items within any
    /// disc's Manhattan radius), ascending, each once.
    pub fn near_discs(&self, discs: &[(TilePos, u32)]) -> Vec<u32> {
        let mut out = Vec::new();
        for &(t, r) in discs {
            self.near(t, r, &mut out);
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The items in the cells at Chebyshev cell distance exactly `k` from
    /// `t`'s cell, appended to `out` in cell order. `false` once the ring
    /// lies wholly outside the grid (every later ring does too).
    pub fn ring(&self, t: TilePos, k: usize, out: &mut Vec<u32>) -> bool {
        let (cx, cy) = self.cxy(t);
        let (cx, cy) = (cx as isize, cy as isize);
        let (gw, gh) = (self.gw as isize, self.gh as isize);
        let k = k as isize;
        if cx - k < 0 && cy - k < 0 && cx + k >= gw && cy + k >= gh {
            return false;
        }
        let mut cell = |x: isize, y: isize| {
            if (0..gw).contains(&x) && (0..gh).contains(&y) {
                let c = (y * gw + x) as usize;
                let (a, b) = (self.starts[c] as usize, self.starts[c + 1] as usize);
                out.extend_from_slice(&self.items[a..b]);
            }
        };
        if k == 0 {
            cell(cx, cy);
            return true;
        }
        for x in cx - k..=cx + k {
            cell(x, cy - k);
            cell(x, cy + k);
        }
        for y in cy - k + 1..=cy + k - 1 {
            cell(cx - k, y);
            cell(cx + k, y);
        }
        true
    }

    /// Every non-empty cell with a floor under `Σ` the Manhattan distances
    /// from the points `ts` to any tile in it (the distances to the cell's
    /// square, summed).
    pub fn cell_floors(&self, ts: &[TilePos]) -> Vec<(u32, u32)> {
        let c = self.cell as i64;
        let axis = |v: i64, lo: i64| {
            if v < lo {
                lo - v
            } else if v >= lo + c {
                v - (lo + c - 1)
            } else {
                0
            }
        };
        let mut out = Vec::new();
        for cy in 0..self.gh {
            for cx in 0..self.gw {
                let i = cy * self.gw + cx;
                if self.starts[i] == self.starts[i + 1] {
                    continue;
                }
                let (x0, y0) = (cx as i64 * c, cy as i64 * c);
                let lb: i64 = ts.iter().map(|t| axis(i64::from(t.x), x0) + axis(i64::from(t.y), y0)).sum();
                out.push((lb as u32, i as u32));
            }
        }
        out
    }

    /// The items of cell `c`, appended to `out`.
    pub fn cell_items(&self, c: u32, out: &mut Vec<u32>) {
        let c = c as usize;
        out.extend_from_slice(&self.items[self.starts[c] as usize..self.starts[c + 1] as usize]);
    }

    /// The perf pass phase 2: [`TileGrid::ring`] around the box of cells
    /// that the tiles `lo..=hi` span (a point when `lo == hi`): the items in
    /// the cells at Chebyshev cell distance exactly `k` from that box (ring
    /// 0 is the box itself), appended to `out` in cell order. `false` once
    /// the ring lies wholly outside the grid (every later ring does too).
    /// [`TileGrid::ring_floor`] bounds the Manhattan distance from any tile
    /// of the box to any tile of ring `k`, as it does for a point.
    pub fn ring_box(&self, lo: TilePos, hi: TilePos, k: usize, out: &mut Vec<u32>) -> bool {
        let (ax, ay) = self.cxy(lo);
        let (bx, by) = self.cxy(hi);
        let (x0, x1) = (ax.min(bx) as isize, ax.max(bx) as isize);
        let (y0, y1) = (ay.min(by) as isize, ay.max(by) as isize);
        let (gw, gh) = (self.gw as isize, self.gh as isize);
        let k = k as isize;
        if x0 - k < 0 && y0 - k < 0 && x1 + k >= gw && y1 + k >= gh {
            return false;
        }
        let mut cell = |x: isize, y: isize| {
            if (0..gw).contains(&x) && (0..gh).contains(&y) {
                let c = (y * gw + x) as usize;
                let (a, b) = (self.starts[c] as usize, self.starts[c + 1] as usize);
                out.extend_from_slice(&self.items[a..b]);
            }
        };
        if k == 0 {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    cell(x, y);
                }
            }
            return true;
        }
        for x in x0 - k..=x1 + k {
            cell(x, y0 - k);
            cell(x, y1 + k);
        }
        for y in y0 - k + 1..=y1 + k - 1 {
            cell(x0 - k, y);
            cell(x1 + k, y);
        }
        true
    }

    /// A floor under the Manhattan distance from `t` to any tile of a cell
    /// in ring `k` of `t`'s cell: `(k − 1) × cell + 1` (0 for ring 0).
    pub fn ring_floor(&self, k: usize) -> u32 {
        if k == 0 {
            0
        } else {
            (k as u32 - 1) * self.cell + 1
        }
    }
}

/// Scale S4 perf: every building, bucketed by its door tile (doors never
/// move, and buildings are never despawned in play), for "the buildings
/// near here" and "the nearest building that passes a test" without a scan
/// of a kind's list (4,000 Blocks at 20,000 residents). Built by
/// `World::building_grid` and kept until a Building is inserted or removed.
/// A query is a superset that the caller filters by its exact rule, so the
/// answer is the scan's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildingGrid {
    /// Every building, ascending.
    pub ids: Vec<crate::entity::EntityId>,
    /// Each building's door, by `ids`' index.
    doors: Vec<TilePos>,
    grid: TileGrid,
}

/// The cell size of [`BuildingGrid`] in tiles.
pub const BUILDING_CELL: u32 = 16;

impl BuildingGrid {
    /// `ids` ascending, each with its door tile, on a `w` x `h` map.
    pub fn build(w: usize, h: usize, ids: Vec<crate::entity::EntityId>, doors: &[TilePos]) -> BuildingGrid {
        let grid = TileGrid::build(w, h, BUILDING_CELL, doors.iter().copied());
        BuildingGrid { ids, doors: doors.to_vec(), grid }
    }

    /// The perf pass phase 2 (the venue index): the buildings whose door
    /// lies within `max_d` of the box `lo..=hi` by `dist` (the caller's exact
    /// distance, at least the Manhattan distance from the box to the door),
    /// handed to `test` in ascending `(dist, id)` until one passes, which is
    /// returned. Rings of cells are read outward from the box; after each,
    /// the candidates nearer than any cell further out can hold are tested
    /// in order, so `test` sees exactly the sequence a sort of every
    /// building by `(dist, id)` would hand it, up to the first pass (the
    /// scan it replaces), cut at `max_d`.
    pub fn first_by_distance(
        &self,
        lo: TilePos,
        hi: TilePos,
        max_d: u32,
        dist: impl Fn(TilePos) -> u32,
        mut test: impl FnMut(u32, crate::entity::EntityId) -> bool,
    ) -> Option<(u32, crate::entity::EntityId)> {
        use std::cmp::Reverse;
        if self.ids.is_empty() {
            return None;
        }
        let mut pending: std::collections::BinaryHeap<Reverse<(u32, crate::entity::EntityId)>> =
            std::collections::BinaryHeap::new();
        let mut idx = Vec::new();
        let mut k = 0usize;
        let mut more = true;
        loop {
            if more && self.grid.ring_floor(k) <= max_d {
                idx.clear();
                more = self.grid.ring_box(lo, hi, k, &mut idx);
                for &i in &idx {
                    let d = dist(self.doors[i as usize]);
                    if d <= max_d {
                        pending.push(Reverse((d, self.ids[i as usize])));
                    }
                }
                k += 1;
            } else {
                more = false;
            }
            // Every door in ring `k` or further is at least this far.
            let floor = if more { self.grid.ring_floor(k) } else { u32::MAX };
            while let Some(&Reverse((d, b))) = pending.peek() {
                if more && d >= floor {
                    break;
                }
                pending.pop();
                if test(d, b) {
                    return Some((d, b));
                }
            }
            if !more {
                return None;
            }
        }
    }

    /// The perf pass phase 3: every building whose door lies in a cell the
    /// square of any `(centre, radius)` disc touches (a superset of the
    /// doors within any disc's Manhattan radius), ascending, each once.
    pub fn near_discs(&self, discs: &[(TilePos, u32)]) -> Vec<crate::entity::EntityId> {
        self.grid.near_discs(discs).into_iter().map(|i| self.ids[i as usize]).collect()
    }

    /// Every building whose door lies within the Chebyshev square of
    /// radius `r` around `t` (and maybe a few beyond it), ascending: a
    /// superset of the doors within `r` Manhattan tiles.
    pub fn near(&self, t: TilePos, r: u32) -> Vec<crate::entity::EntityId> {
        let mut idx = Vec::new();
        self.grid.near(t, r, &mut idx);
        idx.sort_unstable();
        idx.into_iter().map(|i| self.ids[i as usize]).collect()
    }

    /// The building minimising `(d, key, id)` over those `f` scores `Some((d,
    /// key))`, where `d` is a distance at least the door's Manhattan
    /// distance from `t` less `slack` (an outside-door distance: 1). Rings
    /// of cells are read outward from `t` and the search stops once no
    /// cell further out can hold a `d` as small as the best. The same
    /// answer as `f` over every building and the minimum.
    pub fn nearest<K: Ord + Copy>(
        &self,
        t: TilePos,
        slack: u32,
        mut f: impl FnMut(crate::entity::EntityId) -> Option<(u32, K)>,
    ) -> Option<(u32, K, crate::entity::EntityId)> {
        let mut best: Option<(u32, K, crate::entity::EntityId)> = None;
        let mut idx = Vec::new();
        for k in 0.. {
            if best.is_some_and(|(d, _, _)| self.grid.ring_floor(k) > d.saturating_add(slack)) {
                break;
            }
            idx.clear();
            if !self.grid.ring(t, k, &mut idx) {
                break;
            }
            for &i in &idx {
                let b = self.ids[i as usize];
                if let Some((d, key)) = f(b) {
                    if best.is_none_or(|(bd, bk, bb)| (d, key, b) < (bd, bk, bb)) {
                        best = Some((d, key, b));
                    }
                }
            }
        }
        best
    }
}

impl BuildingGrid {
    /// [`BuildingGrid::nearest`] for a distance at least the door's summed
    /// Manhattan distances from every point of `ts` (a walk via two places):
    /// the cells are read in the order of that sum's floor over them, and
    /// the search stops once no cell left can hold a `d` as small as the
    /// best. The same answer as `f` over every building and the minimum.
    pub fn nearest_sum<K: Ord + Copy>(
        &self,
        ts: &[TilePos],
        mut f: impl FnMut(crate::entity::EntityId) -> Option<(u32, K)>,
    ) -> Option<(u32, K, crate::entity::EntityId)> {
        use std::cmp::Reverse;
        let mut heap: std::collections::BinaryHeap<Reverse<(u32, u32)>> =
            self.grid.cell_floors(ts).into_iter().map(Reverse).collect();
        let mut best: Option<(u32, K, crate::entity::EntityId)> = None;
        let mut idx = Vec::new();
        while let Some(Reverse((lb, c))) = heap.pop() {
            if best.is_some_and(|(d, _, _)| lb > d) {
                break;
            }
            idx.clear();
            self.grid.cell_items(c, &mut idx);
            for &i in &idx {
                let b = self.ids[i as usize];
                if let Some((d, key)) = f(b) {
                    if best.is_none_or(|(bd, bk, bb)| (d, key, b) < (bd, bk, bb)) {
                        best = Some((d, key, b));
                    }
                }
            }
        }
        best
    }
}

/// Scale S1: every entity with a `Position`, bucketed by tile cell and by
/// the building it is inside, kept by marks: every write path of the
/// Position store (`comp_mut`, `insert`, `remove`, `despawn`) marks the
/// entity, and a query first re-files the marked ones from their current
/// Position. A query is a superset of "within `r` tiles or inside
/// `building`", which the caller filters by its exact rule. Not saved: a
/// loaded world (and a new one) files everyone on its first query.
#[derive(Clone, Debug, Default)]
pub struct PosIndex {
    built: bool,
    cell: u32,
    gw: usize,
    gh: usize,
    /// Entity indices per cell, unordered.
    cells: Vec<Vec<u32>>,
    /// Entity indices per building they are inside, unordered.
    inside: std::collections::BTreeMap<u32, Vec<u32>>,
    /// Per entity index: where it is filed, `(cell, building index)`.
    at: Vec<Option<(u32, Option<u32>)>>,
    dirty: Vec<u32>,
    flagged: Vec<bool>,
}

/// The cell size of [`PosIndex`] in tiles.
pub const POS_CELL: u32 = 8;

impl PosIndex {
    /// An entity's Position may have changed.
    pub fn mark(&mut self, index: u32) {
        if !self.built {
            return;
        }
        let i = index as usize;
        if self.flagged.len() <= i {
            self.flagged.resize(i + 1, false);
        }
        if !self.flagged[i] {
            self.flagged[i] = true;
            self.dirty.push(index);
        }
    }

    fn cell_of(&self, t: TilePos) -> u32 {
        let cx = (usize::from(t.x) / self.cell as usize).min(self.gw - 1);
        let cy = (usize::from(t.y) / self.cell as usize).min(self.gh - 1);
        (cy * self.gw + cx) as u32
    }

    fn file(&mut self, i: u32, now: Option<(TilePos, Option<u32>)>) {
        let iu = i as usize;
        if self.at.len() <= iu {
            self.at.resize(iu + 1, None);
        }
        let new = now.map(|(t, b)| (self.cell_of(t), b));
        let old = self.at[iu];
        if old == new {
            return;
        }
        if let Some((c, b)) = old {
            let v = &mut self.cells[c as usize];
            if let Some(k) = v.iter().position(|&x| x == i) {
                v.swap_remove(k);
            }
            if let Some(b) = b {
                if let Some(v) = self.inside.get_mut(&b) {
                    if let Some(k) = v.iter().position(|&x| x == i) {
                        v.swap_remove(k);
                    }
                    if v.is_empty() {
                        self.inside.remove(&b);
                    }
                }
            }
        }
        if let Some((c, b)) = new {
            self.cells[c as usize].push(i);
            if let Some(b) = b {
                self.inside.entry(b).or_default().push(i);
            }
        }
        self.at[iu] = new;
    }

    /// Bring the index up to date: everyone on the first call (`now(i)` is
    /// entity `i`'s `(tile, building index)` if alive with a Position, for
    /// `i < n`), else the marked.
    pub fn refresh(&mut self, w: usize, h: usize, n: usize, now: impl Fn(u32) -> Option<(TilePos, Option<u32>)>) {
        if !self.built {
            let cell = POS_CELL;
            let (gw, gh) = (w.div_ceil(cell as usize).max(1), h.div_ceil(cell as usize).max(1));
            *self = PosIndex { built: true, cell, gw, gh, cells: vec![Vec::new(); gw * gh], ..Default::default() };
            for i in 0..n as u32 {
                self.file(i, now(i));
            }
            return;
        }
        let dirty = std::mem::take(&mut self.dirty);
        for &i in &dirty {
            self.flagged[i as usize] = false;
            self.file(i, now(i));
        }
        self.dirty = dirty;
        self.dirty.clear();
    }

    /// Entity indices in a cell the Chebyshev square of radius `r` around
    /// `t` touches, and those inside `building` (index), appended to `out`
    /// unordered and possibly twice. Call `refresh` first.
    pub fn near(&self, t: TilePos, r: u32, building: Option<u32>, out: &mut Vec<u32>) {
        let c = self.cell as usize;
        let (x, y, r) = (usize::from(t.x), usize::from(t.y), r as usize);
        let x0 = (x.saturating_sub(r) / c).min(self.gw - 1);
        let x1 = ((x + r) / c).min(self.gw - 1);
        let y0 = (y.saturating_sub(r) / c).min(self.gh - 1);
        let y1 = ((y + r) / c).min(self.gh - 1);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                out.extend_from_slice(&self.cells[cy * self.gw + cx]);
            }
        }
        if let Some(v) = building.and_then(|b| self.inside.get(&b)) {
            out.extend_from_slice(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cheb(a: TilePos, b: TilePos) -> u32 {
        let dx = (i32::from(a.x) - i32::from(b.x)).unsigned_abs();
        let dy = (i32::from(a.y) - i32::from(b.y)).unsigned_abs();
        dx.max(dy)
    }

    /// The position index, kept by marks across days of moves, spawns,
    /// deaths and despawns, holds every entity a scan of the Position store
    /// finds within `r` tiles or inside the building (and only live ones),
    /// at agents' and buildings' doors all over the map.
    #[test]
    fn test_agents_near_holds_the_scan() {
        use crate::components::{Building, Position};
        use crate::entity::EntityId;
        let mut w = crate::world::World::new(42, crate::config::Config::load());
        let cheb = |a: TilePos, b: TilePos| {
            (i32::from(a.x) - i32::from(b.x)).unsigned_abs().max((i32::from(a.y) - i32::from(b.y)).unsigned_abs())
        };
        let mut checked = 0usize;
        for round in 0..8 {
            // Built on the first query, then kept by marks.
            w.run_ticks(if round == 0 { 1 } else { 9 * crate::time::TICKS_PER_HOUR + 13 });
            let buildings = w.with::<Building>();
            let mut probes: Vec<(TilePos, Option<EntityId>)> = Vec::new();
            for &b in buildings.iter().step_by(7) {
                if let Some(bd) = w.comp::<Building>(b) {
                    probes.push((bd.door, Some(b)));
                }
            }
            // scan-ok: a unit test.
            for a in w.citizens().into_iter().step_by(97) {
                if let Some(p) = w.comp::<Position>(a) {
                    probes.push((p.tile, p.building));
                }
            }
            for (k, &(tile, building)) in probes.iter().enumerate() {
                let r = [0u32, 1, 6, 9, 30][k % 5];
                let got = w.agents_near(tile, r, building);
                let mut sorted = got.clone();
                sorted.sort_unstable();
                sorted.dedup();
                assert_eq!(sorted, got, "ascending, each once");
                assert!(got.iter().all(|&e| w.is_alive(e)), "a dead entity returned");
                for e in w.entities() {
                    let Some(p) = w.comp::<Position>(e) else { continue };
                    if cheb(p.tile, tile) <= r || (building.is_some() && p.building == building) {
                        assert!(got.binary_search(&e).is_ok(), "{e} missed at round {round}");
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 1000, "the test checked {checked} entities");
    }

    /// The grid's query holds every item the scan finds, for any radius and
    /// cell size, at the map's edges too.
    #[test]
    fn test_near_is_a_superset_of_the_scan() {
        let (w, h) = (256usize, 192usize);
        let mut pts = Vec::new();
        let mut s: u32 = 12345;
        for _ in 0..500 {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let x = (s >> 8) as usize % w;
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let y = (s >> 8) as usize % h;
            pts.push(TilePos { x: x as u16, y: y as u16 });
        }
        pts.push(TilePos { x: 255, y: 191 });
        pts.push(TilePos { x: 0, y: 0 });
        for cell in [1u32, 3, 7, 16, 64, 300] {
            let g = TileGrid::build(w, h, cell, pts.iter().copied());
            for (qi, &q) in pts.iter().enumerate().step_by(7) {
                for r in [0u32, 1, 6, 9, 40, 400] {
                    let mut got = Vec::new();
                    g.near(q, r, &mut got);
                    assert_eq!(g.any_near(q, r), !got.is_empty());
                    let mut sorted = got.clone();
                    sorted.sort_unstable();
                    sorted.dedup();
                    assert_eq!(sorted.len(), got.len(), "an item twice (cell {cell}, r {r})");
                    for (i, &p) in pts.iter().enumerate() {
                        if cheb(p, q) <= r {
                            assert!(
                                sorted.binary_search(&(i as u32)).is_ok(),
                                "item {i} missed from {qi}, cell {cell} r {r}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Scale S4 perf: the building grid's `near`, `nearest` and
    /// `nearest_sum` are the scans they replace, on random doors with
    /// random filters, ties and points past the map's last full cell.
    #[test]
    fn test_building_grid_queries_match_the_scan() {
        use crate::entity::EntityId;
        let (w, h) = (203usize, 117usize);
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let n = 600;
        let ids: Vec<EntityId> = (0..n).map(|i| EntityId { index: 3 * i + 1, generation: 0 }).collect();
        let doors: Vec<TilePos> =
            (0..n).map(|_| TilePos { x: (next() % w as u64) as u16, y: (next() % h as u64) as u16 }).collect();
        let grid = BuildingGrid::build(w, h, ids.clone(), &doors);
        let door_of = |b: EntityId| doors[ids.iter().position(|&i| i == b).expect("an id")];
        for round in 0..400 {
            let t = TilePos { x: (next() % w as u64) as u16, y: (next() % h as u64) as u16 };
            let u = TilePos { x: (next() % w as u64) as u16, y: (next() % h as u64) as u16 };
            let keep = 1 + next() % 40;
            let pass = |b: EntityId| u64::from(b.index) % keep == 0;
            // near: every door within r (Chebyshev), ascending.
            let r = (next() % 40) as u32;
            let got = grid.near(t, r);
            assert!(got.windows(2).all(|p| p[0] < p[1]));
            for (&b, &d) in ids.iter().zip(&doors) {
                if cheb(d, t) <= r {
                    assert!(got.contains(&b), "round {round}: {b} at {d:?} within {r} of {t:?}");
                }
            }
            // nearest: (distance, key, id) over the passing, key a coarse
            // tie-maker so ties on (d, key) fall to the id.
            let slack = (round % 2) as u32;
            let dist = |b: EntityId| door_of(b).manhattan(t) + slack;
            let want = ids.iter().copied().filter(|&b| pass(b)).map(|b| (dist(b), b.index % 3, b)).min();
            let got = grid.nearest(t, slack, |b| pass(b).then(|| (dist(b), b.index % 3)));
            assert_eq!(got, want, "round {round}: nearest from {t:?}");
            let sum = |b: EntityId| door_of(b).manhattan(t) + door_of(b).manhattan(u);
            let want = ids.iter().copied().filter(|&b| pass(b)).map(|b| (sum(b), b.index, b)).min();
            let got = grid.nearest_sum(&[t, u], |b| pass(b).then(|| (sum(b), b.index)));
            assert_eq!(got, want, "round {round}: nearest_sum from {t:?} via {u:?}");
        }
    }

    /// The perf pass phase 2: `first_by_distance` hands `test` the same
    /// sequence a sort of every building by `(distance, id)` does, up to
    /// the first pass and cut at `max_d`, from points and boxes (a day's
    /// rectangle), on random doors with many ties, random filters and
    /// points past the map's last full cell.
    #[test]
    fn test_first_by_distance_is_the_sorted_scan() {
        use crate::entity::EntityId;
        let (w, h) = (203usize, 117usize);
        let mut x = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let n = 300;
        let ids: Vec<EntityId> = (0..n).map(|i| EntityId { index: 5 * i + 2, generation: 0 }).collect();
        // Doors on a coarse lattice, so equal distances are common.
        let doors: Vec<TilePos> = (0..n)
            .map(|_| TilePos { x: (next() % (w as u64 / 3)) as u16 * 3, y: (next() % (h as u64 / 3)) as u16 * 3 })
            .collect();
        let grid = BuildingGrid::build(w, h, ids.clone(), &doors);
        let mut ties = 0;
        for round in 0..600 {
            let a = TilePos { x: (next() % w as u64) as u16, y: (next() % h as u64) as u16 };
            let b = if round % 2 == 0 {
                a
            } else {
                TilePos { x: (next() % w as u64) as u16, y: (next() % h as u64) as u16 }
            };
            let (lo, hi) = (TilePos { x: a.x.min(b.x), y: a.y.min(b.y) }, TilePos { x: a.x.max(b.x), y: a.y.max(b.y) });
            let dist = |t: TilePos| {
                let n = TilePos { x: t.x.clamp(lo.x, hi.x), y: t.y.clamp(lo.y, hi.y) };
                n.manhattan(t)
            };
            let keep = 1 + next() % 30;
            let pass = |b: EntityId| u64::from(b.index) % keep == 0;
            let max_d = if round % 3 == 0 { u32::MAX } else { (next() % 120) as u32 };
            let mut all: Vec<(u32, EntityId)> = ids.iter().zip(&doors).map(|(&b, &t)| (dist(t), b)).collect();
            all.sort_unstable();
            ties += all.windows(2).filter(|p| p[0].0 == p[1].0).count().min(1);
            let mut want_seq = Vec::new();
            let mut want = None;
            for &(d, b) in all.iter().filter(|&&(d, _)| d <= max_d) {
                want_seq.push((d, b));
                if pass(b) {
                    want = Some((d, b));
                    break;
                }
            }
            let mut seq = Vec::new();
            let got = grid.first_by_distance(lo, hi, max_d, dist, |d, b| {
                seq.push((d, b));
                pass(b)
            });
            assert_eq!(got, want, "round {round}: {lo:?}..{hi:?} cut {max_d}");
            assert_eq!(seq, want_seq, "round {round}: the tested sequence");
        }
        assert!(ties > 100, "too few ties ({ties}) to exercise the tie-break");
        let empty = BuildingGrid::build(w, h, Vec::new(), &[]);
        assert_eq!(empty.first_by_distance(TilePos::default(), TilePos::default(), u32::MAX, |_| 0, |_, _| true), None);
    }
}
