//! Scale S3b (docs/SCALE_20K.md): a deterministic map generator for a city
//! of any size, the in-repo stand-in for the big island at 20k.
//!
//! The city is a grid of **boroughs**, each a copy of the shipped 2k map's
//! layout (`assets/gen_map.py`, 256 x 184 tiles of 7 x 7 cells between
//! roads): its own Vats strip of 12 Vat Farms, Sump, Mid and Spire, 3
//! Markets, 3 Bars, 2 Hideouts, 2 Security Offices and 160 Lots, and up to
//! 400 Blocks (5 residents each). One borough per 2,000 residents, so the
//! employers and the food scale with the population and stay near the homes
//! (a 20k city on one Vats strip would put Farms 600 tiles from the west
//! Sump). The one-per-city buildings (the Precinct, the Civic Hall, the
//! Reserve Depot, the Recycler) and the Civic zone sit in the central
//! borough only. At 2,000 residents and seed 0 the tiles, zones and
//! buildings are the shipped `assets/map.txt` exactly.
//!
//! The map carries its own districts (a grid: Sump, Mid, Spire and Vats per
//! borough, merged when more than `MAX_DISTRICTS` would be needed) and El
//! lines (two east-west lines per borough row, two north-south per borough
//! column). `seed` rotates which free cells take the Blocks and Lots.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{BuildingKind, Rect, Zone, MAX_DISTRICTS};
use crate::config::TransitLineCfg;
use crate::map::{MapDistrict, MapText};

/// Residents per borough (the shipped city's population).
pub const BOROUGH_RESIDENTS: u32 = 2_000;
/// Blocks (Homes) per 1,000 residents: 5 residents a Block.
pub const RESIDENTS_PER_HOME: u32 = 5;
const COLS: usize = 32;
const ROWS: usize = 23;
const BW: usize = 8 * COLS; // 256
const BH: usize = 8 * ROWS; // 184
/// Open Sump ground (5 rows) and a Water strip (3 rows) along the south edge.
const SOUTH: usize = 8;

/// A generated map: the text, its size and its building counts.
#[derive(Clone, Debug)]
pub struct GenMap {
    pub text: String,
    pub w: usize,
    pub h: usize,
    pub boroughs: usize,
    pub districts: usize,
    pub lines: usize,
    pub counts: BTreeMap<BuildingKind, usize>,
}

/// Template kinds (`Lot2` is a double-wide Lot).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Tpl {
    Home,
    Single(BuildingKind),
    Cemetery,
    Double(BuildingKind),
    Farm,
    Hideout,
    Jail,
}

/// splitmix64: the seed's rotation of a free-cell list (no `rand` stream).
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A cell or tile `(x, y)`; a rect `(x, y, w, h)`.
type Cell = (usize, usize);
type Cell4 = (usize, usize, usize, usize);

struct Borough {
    bc: usize,
    br: usize,
    central: bool,
}

struct Gen {
    brows: usize,
    central: (usize, usize),
    seed: u64,
    used: BTreeSet<(usize, usize)>,
    /// `(kind, rect, door, tier)`.
    buildings: Vec<(BuildingKind, Rect, (usize, usize), u8)>,
}

impl Gen {
    /// The zone of tile `(x, y)` (`gen_map.py`'s `zone_of`, per borough).
    fn zone(&self, x: usize, y: usize) -> Zone {
        let (bc, lx) = (x / BW, x % BW);
        let br = (y / BH).min(self.brows - 1);
        let ly = y - br * BH;
        if lx >= 208 {
            return Zone::Vats;
        }
        if ly >= 112 {
            return Zone::Sump;
        }
        if ly < 40 && lx >= 64 {
            return Zone::Spire;
        }
        if (bc, br) == self.central && (40..80).contains(&ly) && (96..176).contains(&lx) {
            return Zone::Civic;
        }
        Zone::Mid
    }

    fn cell_zone(&self, gc: usize, gr: usize) -> Zone {
        self.zone(8 * gc, 8 * gr)
    }

    fn take(&mut self, cells: &[(usize, usize)]) {
        for &c in cells {
            assert!(self.used.insert(c), "mapgen: cell {c:?} used twice");
        }
    }

    /// One building from its template at global cell `(gc, gr)` (the
    /// top-left cell of a multi-cell kind).
    fn place(&mut self, t: Tpl, gc: usize, gr: usize, tier: u8) {
        let (x, y) = (8 * gc, 8 * gr);
        let (rect, door, cells): (Cell4, Cell, Vec<Cell>) = match t {
            Tpl::Home => ((x + 1, y + 3, 5, 4), (x + 3, y + 6), vec![(gc, gr)]),
            Tpl::Single(_) => ((x, y + 1, 7, 6), (x + 3, y + 6), vec![(gc, gr)]),
            Tpl::Cemetery => ((x, y, 7, 7), (x + 3, y + 6), vec![(gc, gr)]),
            Tpl::Double(_) => ((x, y + 1, 15, 6), (x + 7, y + 6), vec![(gc, gr), (gc + 1, gr)]),
            Tpl::Farm => ((x, y, 15, 7), (x + 7, y + 6), vec![(gc, gr), (gc + 1, gr)]),
            Tpl::Hideout => ((x, y + 2, 15, 5), (x + 7, y + 6), vec![(gc, gr), (gc + 1, gr)]),
            Tpl::Jail => {
                ((x, y, 15, 15), (x + 7, y + 14), vec![(gc, gr), (gc + 1, gr), (gc, gr + 1), (gc + 1, gr + 1)])
            }
        };
        self.take(&cells);
        let kind = match t {
            Tpl::Home => BuildingKind::Home,
            Tpl::Single(k) | Tpl::Double(k) => k,
            Tpl::Cemetery => BuildingKind::Cemetery,
            Tpl::Farm => BuildingKind::Farm,
            Tpl::Hideout => BuildingKind::Hideout,
            Tpl::Jail => BuildingKind::Jail,
        };
        let r = Rect { x: rect.0 as u16, y: rect.1 as u16, w: rect.2 as u16, h: rect.3 as u16 };
        self.buildings.push((kind, r, door, tier));
    }

    /// Free cells of `zone` in borough `b`, row-major, inside the local
    /// `rows` x `cols` window.
    fn free_cells(
        &self,
        b: &Borough,
        zone: Zone,
        rows: std::ops::Range<usize>,
        cols: std::ops::Range<usize>,
    ) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for r in rows {
            for c in cols.clone() {
                let (gc, gr) = (b.bc * COLS + c, b.br * ROWS + r);
                if !self.used.contains(&(gc, gr)) && self.cell_zone(gc, gr) == zone {
                    out.push((gc, gr));
                }
            }
        }
        out
    }

    /// `gen_map.py`'s `stride`, after the seed's rotation of the list
    /// (none at seed 0).
    fn stride(&self, mut cells: Vec<(usize, usize)>, n: usize, salt: u64) -> Vec<(usize, usize)> {
        assert!(cells.len() >= n, "mapgen: {} free cells for {n}", cells.len());
        if self.seed != 0 && !cells.is_empty() {
            let k = (mix(self.seed ^ mix(salt)) % cells.len() as u64) as usize;
            cells.rotate_left(k);
        }
        (0..n).map(|i| cells[i * cells.len() / n]).collect()
    }

    fn borough(&mut self, b: &Borough, homes: [usize; 3]) {
        let (oc, or) = (b.bc * COLS, b.br * ROWS);
        let salt = |k: u64| (b.bc as u64) << 40 | (b.br as u64) << 32 | k;
        use BuildingKind as K;
        // --- Civic (the central borough) / its Mid twin elsewhere -------------
        if b.central {
            self.place(Tpl::Jail, oc + 16, or + 6, 1);
            self.place(Tpl::Single(K::Hall), oc + 14, or + 7, 1);
        }
        self.place(Tpl::Double(K::Market), oc + 12, or + 7, 1);
        self.place(Tpl::Double(K::Bar), oc + 18, or + 7, 1);
        for c in 12..22 {
            self.place(Tpl::Single(K::Lot), oc + c, or + 5, 1);
        }
        // --- Mid -------------------------------------------------------------
        self.place(Tpl::Double(K::Market), oc + 2, or + 7, 1);
        self.place(Tpl::Double(K::Bar), oc + 4, or + 7, 1);
        self.place(Tpl::Double(K::Market), oc + 16, or + 11, 1);
        self.place(Tpl::Double(K::Bar), oc + 18, or + 11, 1);
        for c in 0..8 {
            self.place(Tpl::Single(K::Lot), oc + c, or, 1);
        }
        for c in [8, 9, 22, 23, 24, 25] {
            self.place(Tpl::Single(K::Lot), oc + c, or + 5, 1);
        }
        // --- Spire -----------------------------------------------------------
        for c in 8..18 {
            self.place(Tpl::Single(K::Lot), oc + c, or, 1);
        }
        // --- Vats ------------------------------------------------------------
        for c in [26, 28, 30] {
            for r in 1..5 {
                self.place(Tpl::Farm, oc + c, or + r, 1);
            }
        }
        if b.central {
            self.place(Tpl::Single(K::Warehouse), oc + 26, or + 6, 1);
            self.place(Tpl::Cemetery, oc + 28, or + 6, 1);
        }
        self.place(Tpl::Single(K::SecurityOffice), oc + 30, or + 6, 1);
        self.place(Tpl::Single(K::SecurityOffice), oc + 26, or + 8, 1);
        for c in 27..32 {
            self.place(Tpl::Single(K::Lot), oc + c, or + 8, 1);
        }
        for c in 26..31 {
            self.place(Tpl::Single(K::Lot), oc + c, or + 10, 1);
        }
        // --- Sump ------------------------------------------------------------
        self.place(Tpl::Hideout, oc, or + 22, 1);
        self.place(Tpl::Hideout, oc + 24, or + 22, 1);
        for c in 0..16 {
            self.place(Tpl::Single(K::Lot), oc + c, or + 14, 1);
        }
        // --- Blocks by stride over each zone's free cells ----------------------
        let spire = self.free_cells(b, Zone::Spire, 0..ROWS, 0..COLS);
        let mid = self.free_cells(b, Zone::Mid, 0..ROWS, 0..26);
        let sump = self.free_cells(b, Zone::Sump, 15..ROWS, 0..26);
        for (c, r) in self.stride(spire, homes[0], salt(1)) {
            self.place(Tpl::Home, c, r, 2);
        }
        for (c, r) in self.stride(mid, homes[1], salt(2)) {
            self.place(Tpl::Home, c, r, 1);
        }
        for (c, r) in self.stride(sump, homes[2], salt(3)) {
            self.place(Tpl::Home, c, r, 0);
        }
        // --- Jobs and room J5: Lots on the free ground -------------------------
        // The Vats take 20 double-wide yards and 20 single Lots; the Civic
        // quota goes to the Mid outside the central borough.
        const VATS_DOUBLE: usize = 20;
        let vats_free = self.free_cells(b, Zone::Vats, 0..ROWS, 0..COLS);
        let free: BTreeSet<(usize, usize)> = vats_free.iter().copied().collect();
        let mut pairs = Vec::new();
        let mut paired = BTreeSet::new();
        for &(c, r) in &vats_free {
            if paired.contains(&(c, r)) || paired.contains(&(c + 1, r)) || !free.contains(&(c + 1, r)) {
                continue;
            }
            pairs.push((c, r));
            paired.insert((c, r));
            paired.insert((c + 1, r));
        }
        for (c, r) in self.stride(pairs, VATS_DOUBLE, salt(4)) {
            self.place(Tpl::Double(K::Lot), c, r, 1);
        }
        let civic_to_mid = if b.central { 0 } else { 12 };
        for (zone, n, k) in [
            (Zone::Vats, 40 - VATS_DOUBLE, 5),
            (Zone::Mid, 30 + civic_to_mid, 6),
            (Zone::Civic, 12 - civic_to_mid, 7),
            (Zone::Spire, 10, 8),
            (Zone::Sump, 8, 9),
        ] {
            if n == 0 {
                continue;
            }
            let cells = self.free_cells(b, zone, 0..ROWS, 0..COLS);
            for (c, r) in self.stride(cells, n, salt(k)) {
                self.place(Tpl::Single(K::Lot), c, r, 1);
            }
        }
    }
}

/// The district grouping: how many districts a borough splits into.
fn district_mode(others: usize, budget: usize) -> usize {
    (1..=4).rev().find(|&g| g * others <= budget).unwrap_or(1)
}

/// The shipped city's districts (`[districts]` in `assets/config.toml`):
/// name, zone, local x range. The central borough keeps them.
const SHIPPED: [(&str, Zone, u16, u16); 8] = [
    ("Spire", Zone::Spire, 0, 256),
    ("Civic", Zone::Civic, 0, 256),
    ("Vats", Zone::Vats, 0, 256),
    ("Mid West", Zone::Mid, 0, 64),
    ("Mid East", Zone::Mid, 64, 256),
    ("Sump West", Zone::Sump, 0, 72),
    ("Sump Central", Zone::Sump, 72, 136),
    ("Sump East", Zone::Sump, 136, 256),
];

fn zone_name(z: Zone) -> &'static str {
    match z {
        Zone::Sump => "Sump",
        Zone::Mid => "Mid",
        Zone::Spire => "Spire",
        Zone::Vats => "Vats",
        Zone::Civic => "Civic",
    }
}

/// Generate the map for `residents` (deterministic in `(residents, seed)`).
pub fn generate(residents: u32, seed: u64) -> GenMap {
    let residents = residents.max(1);
    let boroughs = residents.div_ceil(BOROUGH_RESIDENTS) as usize;
    // About 2.5 boroughs wide per row: 1 x 1, 4 x 1, 5 x 2 (10), 9 x 3 (25).
    let brows = ((boroughs as f64 / 2.5).sqrt().round() as usize).clamp(1, boroughs);
    let bcols = boroughs.div_ceil(brows);
    let (w, h) = (bcols * BW, brows * BH + SOUTH);
    assert!(w <= crate::map::MAX_SIDE && h <= crate::map::MAX_SIDE, "mapgen: {residents} residents need {w} x {h}");
    let central = (bcols / 2, brows / 2);
    let mut g = Gen { brows, central, seed, used: BTreeSet::new(), buildings: Vec::new() };

    // Blocks: ceil(residents / 5) over the boroughs, 15 % Spire, 35 % Mid, 50 % Sump.
    let homes_total = residents.div_ceil(RESIDENTS_PER_HOME) as usize;
    let mut list: Vec<Borough> = Vec::new();
    for br in 0..brows {
        for bc in 0..bcols {
            if list.len() < boroughs {
                list.push(Borough { bc, br, central: (bc, br) == central });
            }
        }
    }
    // The central borough must exist even when the last row is short.
    if !list.iter().any(|b| b.central) {
        list[0].central = true;
        g.central = (list[0].bc, list[0].br);
    }
    for (i, b) in list.iter().enumerate() {
        let hb = homes_total / boroughs + usize::from(i < homes_total % boroughs);
        let s = (hb as f64 * 0.15).round() as usize;
        let m = (hb as f64 * 0.35).round() as usize;
        g.borough(b, [s, m, hb - s - m]);
    }

    // --- tiles ----------------------------------------------------------------
    let mut tiles = vec!['.'; w * h];
    for k in 0..bcols * COLS {
        let x = 7 + 8 * k;
        for y in 0..h - 3 {
            tiles[y * w + x] = '=';
        }
    }
    for k in 0..brows * ROWS {
        let y = 7 + 8 * k;
        tiles[y * w..(y + 1) * w].fill('=');
    }
    tiles[(h - 3) * w..].fill('~');
    for &(kind, r, (dx, dy), _) in &g.buildings {
        let (x0, y0, rw, rh) = (usize::from(r.x), usize::from(r.y), usize::from(r.w), usize::from(r.h));
        for y in y0..y0 + rh {
            for x in x0..x0 + rw {
                let edge = y == y0 || y == y0 + rh - 1 || x == x0 || x == x0 + rw - 1;
                tiles[y * w + x] = match (kind, edge) {
                    (BuildingKind::Lot, _) => '.',
                    (_, true) => '#',
                    (BuildingKind::Farm, false) => 'f',
                    _ => '.',
                };
            }
        }
        tiles[dy * w + dx] = 'D';
    }
    let mut zones = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            zones.push(g.zone(x, y).letter());
        }
    }

    // --- districts --------------------------------------------------------------
    // The central borough keeps the shipped city's eight districts by name
    // and x range (seeding passes look districts up by name: the Clinics,
    // the Fixer, the Labs, the Feeds, the Unplugged); every other borough
    // splits by zone as far as `MAX_DISTRICTS` allows.
    let others = list.len() - 1;
    let budget = MAX_DISTRICTS - SHIPPED.len();
    let mode = district_mode(others, budget);
    let group = |z: Zone| match (mode, z) {
        (4, z) => z,
        (3, Zone::Spire) => Zone::Mid,
        (3, z) => z,
        (2, Zone::Sump) => Zone::Sump,
        _ => Zone::Mid,
    };
    let borough_no: BTreeMap<(usize, usize), usize> = list.iter().enumerate().map(|(i, b)| ((b.bc, b.br), i)).collect();
    let central_bi = borough_no.get(&g.central).copied().unwrap_or(0);
    // A non-central borough's group (past `budget` boroughs, neighbours share).
    let bgroup = |bi: usize| {
        let j = if bi > central_bi { bi - 1 } else { bi };
        if mode * others <= budget {
            j
        } else {
            j * budget / others.max(1)
        }
    };
    // Key: (0, shipped row) for the central borough, (1 + group, zone) else.
    let mut keys: Vec<(usize, usize)> = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let z = g.zone(x, y);
            let br = (y / BH).min(brows - 1);
            let bi = borough_no.get(&(x / BW, br)).copied().unwrap_or(central_bi);
            if bi == central_bi {
                let lx = (x % BW) as u16;
                let row = SHIPPED.iter().position(|&(_, sz, from, to)| sz == z && from <= lx && lx < to).unwrap_or(0);
                keys.push((0, row));
            } else {
                keys.push((1 + bgroup(bi), group(z).index()));
            }
        }
    }
    let set: BTreeSet<(usize, usize)> = keys.iter().copied().collect();
    let mut districts: Vec<MapDistrict> = Vec::new();
    let mut index: BTreeMap<(usize, usize), u8> = BTreeMap::new();
    for (i, &(gi, r)) in set.iter().enumerate() {
        index.insert((gi, r), i as u8);
        let (name, zone) = if gi == 0 {
            (SHIPPED[r].0.to_string(), SHIPPED[r].1)
        } else {
            let z = Zone::ALL[r];
            (format!("{} {}", zone_name(z), gi), z)
        };
        districts.push(MapDistrict { name, zone, x_from: 0, x_to: w as u16 });
    }
    let grid: Vec<u8> = keys.iter().map(|k| index[k]).collect();
    assert!(districts.len() <= MAX_DISTRICTS, "mapgen: {} districts", districts.len());

    // --- the El ---------------------------------------------------------------
    let (xe, yn) = (w - 25, 15u16);
    let mut lines = Vec::new();
    for br in 0..brows {
        let y0 = br * BH;
        for (name, ly) in [("Vat Line", 127), ("Low Line", 167)] {
            let label = if brows > 1 { format!("{name} {}", br + 1) } else { name.to_string() };
            lines.push(TransitLineCfg {
                name: label,
                waypoints: vec![[7, (y0 + ly) as u16], [xe as u16, (y0 + ly) as u16]],
                in_flight: false,
            });
        }
    }
    let ys = ((brows - 1) * BH + 167) as u16;
    for bc in 0..bcols {
        let x0 = bc * BW;
        for (name, lx) in [("Crosstown", 39), ("Vats Spur", 231)] {
            let label = if bcols > 1 { format!("{name} {}", bc + 1) } else { name.to_string() };
            lines.push(TransitLineCfg {
                name: label,
                waypoints: vec![[(x0 + lx) as u16, yn], [(x0 + lx) as u16, ys]],
                in_flight: false,
            });
        }
    }

    // --- output -----------------------------------------------------------------
    let order = |k: BuildingKind| BuildingKind::ALL.iter().position(|&x| x == k).unwrap_or(usize::MAX);
    let mut bl: Vec<(BuildingKind, Rect, u8, u8)> = g.buildings.iter().map(|&(k, r, _, t)| (k, r, t, 1)).collect();
    bl.sort_by_key(|&(k, r, ..)| (order(k), r.y, r.x));
    let mut counts = BTreeMap::new();
    for &(k, ..) in &bl {
        *counts.entry(k).or_insert(0) += 1;
    }
    let text = MapText {
        w,
        h,
        tiles,
        zones,
        buildings: bl,
        districts: districts.clone(),
        district_grid: grid,
        lines: lines.clone(),
        extra: Vec::new(),
        comments: vec![format!(
            "generated by citysim mapgen (Scale S3b): {residents} residents, seed {seed}, {} boroughs ({bcols} x {brows})",
            list.len()
        )],
    }
    .to_text(true);
    GenMap { text, w, h, boroughs: list.len(), districts: districts.len(), lines: lines.len(), counts }
}
