//! Scale S1: a flat bucket grid over the map's tiles, for "who is within r
//! tiles of here" without a scan of everyone. Items are the caller's indices
//! (`u32`) into its own list; a query returns a superset (every item in a
//! cell the square of radius `r` touches), which the caller filters by its
//! exact rule, so a grid query is the same answer as the scan it replaces.
//! No RNG, no hashing: the grid is rebuilt from its items when they move.

use crate::components::TilePos;

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
}
