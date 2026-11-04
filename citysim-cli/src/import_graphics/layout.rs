use super::*;

/// `sim/terrain_sim.json`: a class per their tile, north-up (0 sea, 1
/// urban, 2 mountain).
pub(super) struct Terrain {
    pub(super) tile_m: f64,
    pub(super) nx: usize,
    pub(super) ny: usize,
    cls: Vec<u8>,
}

pub(super) const SEA: u8 = 0;
pub(super) const URBAN: u8 = 1;
pub(super) const MOUNTAIN: u8 = 2;

impl Terrain {
    pub(super) fn read(dir: &Path, rep: &mut Report) -> Result<Terrain, String> {
        const F: &str = "sim/terrain_sim.json";
        let v = read_json(dir, "terrain_sim.json")?;
        check_schema(&v, F, rep)?;
        let tile_m = num(get(&v, "tile_m", F)?, &format!("{F} \"tile_m\""))?;
        if tile_m <= 0.0 {
            return Err(format!("{F}: \"tile_m\" {tile_m} is not positive"));
        }
        let nx = usize::try_from(uint(&v, "nx", F)?).unwrap_or(usize::MAX);
        let ny = usize::try_from(uint(&v, "ny", F)?).unwrap_or(usize::MAX);
        let rows = arr(&v, "rows", F)?;
        if rows.len() != ny {
            return Err(format!("{F}: {} rows but ny {ny}", rows.len()));
        }
        let mut cls = Vec::with_capacity(nx.saturating_mul(ny));
        for (r, row) in rows.iter().enumerate() {
            let ctx = format!("{F} rows[{r}]");
            let s = row.as_str().ok_or_else(|| format!("{ctx}: not a string"))?;
            let chars = citysim::map::rle_decode(s).map_err(|e| format!("{ctx}: {e}"))?;
            if chars.len() != nx {
                return Err(format!("{ctx}: {} tiles but nx {nx}", chars.len()));
            }
            for c in chars {
                cls.push(match c {
                    '~' => SEA,
                    'u' => URBAN,
                    'm' => MOUNTAIN,
                    _ => return Err(format!("{ctx}: unknown class {c:?} (the legend is ~ u m)")),
                });
            }
        }
        Ok(Terrain { tile_m, nx, ny, cls })
    }

    /// The class under a point in metres (x east, y north); sea off the grid.
    pub(super) fn at(&self, x: f64, y: f64) -> u8 {
        let c = (x / self.tile_m).floor();
        let r = ((self.ny as f64 * self.tile_m - y) / self.tile_m).floor();
        if c < 0.0 || r < 0.0 || c as usize >= self.nx || r as usize >= self.ny {
            return SEA;
        }
        self.cls[r as usize * self.nx + c as usize]
    }
}

// ---------------------------------------------------------------------------
// The grid frame: metres (y north) to tiles (y south).
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub(super) struct Frame {
    pub(super) x0: f64,
    pub(super) y1: f64,
    pub(super) s: f64,
    pub(super) w: usize,
    pub(super) h: usize,
}

impl Frame {
    /// A rect's tiles by rounded edges, half-open, clamped: `(x0, y0, x1, y1)`.
    pub(super) fn round(&self, r: [f64; 4]) -> (usize, usize, usize, usize) {
        let cx = |m: f64| (((m - self.x0) / self.s).round().max(0.0) as usize).min(self.w);
        let cy = |m: f64| (((self.y1 - m) / self.s).round().max(0.0) as usize).min(self.h);
        (cx(r[0]), cy(r[3]), cx(r[2]), cy(r[1]))
    }

    /// A road's tiles: rounded edges like [`Frame::round`], but at least one
    /// tile across (a 2 m alley keeps a tile at any downsample).
    pub(super) fn road(&self, r: [f64; 4]) -> (usize, usize, usize, usize) {
        let (mut x0, mut y0, mut x1, mut y1) = self.round(r);
        if x1 <= x0 {
            x0 = ((((r[0] + r[2]) / 2.0 - self.x0) / self.s).floor().max(0.0) as usize).min(self.w.saturating_sub(1));
            x1 = x0 + 1;
        }
        if y1 <= y0 {
            y0 = (((self.y1 - (r[1] + r[3]) / 2.0) / self.s).floor().max(0.0) as usize).min(self.h.saturating_sub(1));
            y1 = y0 + 1;
        }
        (x0, y0, x1, y1)
    }

    pub(super) fn tile(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let tx = ((x - self.x0) / self.s).floor();
        let ty = ((self.y1 - y) / self.s).floor();
        (tx >= 0.0 && ty >= 0.0 && (tx as usize) < self.w && (ty as usize) < self.h)
            .then_some((tx as usize, ty as usize))
    }
}

/// A rect's wall tiles a door can take (not the corners), each with the
/// tile outside it: `(door, outside)`; the S, N, E, W walls in turn.
pub(super) fn wall_doors(r: (usize, usize, usize, usize), w: usize, h: usize) -> Vec<((usize, usize), (usize, usize))> {
    let (x0, y0, x1, y1) = r;
    let mut out = Vec::new();
    for v in x0 + 1..x1.saturating_sub(1) {
        if y1 < h {
            out.push(((v, y1 - 1), (v, y1)));
        }
        if y0 > 0 {
            out.push(((v, y0), (v, y0 - 1)));
        }
    }
    for v in y0 + 1..y1.saturating_sub(1) {
        if x1 < w {
            out.push(((x1 - 1, v), (x1, v)));
        }
        if x0 > 0 {
            out.push(((x0, v), (x0 - 1, v)));
        }
    }
    out
}

pub(super) fn inside(r: [f64; 4], win: [f64; 4]) -> bool {
    r[0] >= win[0] && r[1] >= win[1] && r[2] <= win[2] && r[3] <= win[3]
}

pub(super) fn overlaps(r: [f64; 4], win: [f64; 4]) -> bool {
    r[0] < win[2] && win[0] < r[2] && r[1] < win[3] && win[1] < r[3]
}

// ---------------------------------------------------------------------------
// Lots and their layouts.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(super) struct Lot {
    /// The perf pass phase 10: the lot's `lots.json` id (a `B` line's `lot=`).
    pub(super) id: String,
    pub(super) rect: [f64; 4],
    pub(super) front: char,
    pub(super) floors: u32,
    pub(super) flats: usize,
    pub(super) shops: usize,
    pub(super) layout: bool,
    pub(super) residential: bool,
    pub(super) door_off: Option<usize>,
    /// S3c: the lot's `lots_sim.json` record (`None` without the export).
    pub(super) sim: Option<SimLot>,
}

/// Floors (non-roof), flats (units with a living room), shop units, the
/// building kind and the first street door's x offset of one `sim_layout.toml`.
pub(super) fn read_layout(path: &Path) -> Result<(u32, usize, usize, bool, Option<usize>), String> {
    let ctx = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| format!("{ctx}: {e}"))?;
    let v: toml::Value = toml::from_str(&text).map_err(|e| format!("{ctx}: not TOML: {e}"))?;
    let building = v.get("building").ok_or_else(|| format!("{ctx}: missing [building]"))?;
    let kind = building.get("kind").and_then(|k| k.as_str()).ok_or_else(|| format!("{ctx}: missing building.kind"))?;
    let door_off = building
        .get("door_tiles")
        .and_then(|d| d.as_array())
        .and_then(|d| d.first())
        .and_then(|t| t.as_array())
        .and_then(|t| t.first())
        .and_then(|x| x.as_integer())
        .and_then(|x| usize::try_from(x).ok());
    let floors = v.get("floors").and_then(|f| f.as_array()).ok_or_else(|| format!("{ctx}: missing [[floors]]"))?;
    let mut storeys = 0;
    for (i, f) in floors.iter().enumerate() {
        let tier = f.get("tier").and_then(|t| t.as_str()).ok_or_else(|| format!("{ctx}: floors[{i}] missing tier"))?;
        if tier != "roof" {
            storeys += 1;
        }
    }
    let rooms = v.get("rooms").and_then(|r| r.as_array()).ok_or_else(|| format!("{ctx}: missing [[rooms]]"))?;
    let (mut flats, mut shops) = (BTreeSet::new(), BTreeSet::new());
    for (i, r) in rooms.iter().enumerate() {
        let k = r.get("kind").and_then(|k| k.as_str()).ok_or_else(|| format!("{ctx}: rooms[{i}] missing kind"))?;
        let unit = r.get("unit").and_then(|u| u.as_str()).ok_or_else(|| format!("{ctx}: rooms[{i}] missing unit"))?;
        match k {
            "living" => {
                flats.insert(unit.to_string());
            }
            "shop" => {
                shops.insert(unit.to_string());
            }
            _ => {}
        }
    }
    Ok((storeys.max(1), flats.len(), shops.len(), kind == "residential", door_off))
}

/// A cut of a block part: one sim building before its kind is chosen.
#[derive(Clone, Debug)]
pub(super) struct Seg {
    pub(super) rect: Rect,
    pub(super) door: (usize, usize),
    pub(super) district: usize,
    pub(super) floors: u32,
    pub(super) shops: usize,
    pub(super) residential: bool,
    /// S3c, from the export (`None` / 0 without it): the members' use (a
    /// named civic member's, else the most common), whether most members'
    /// ground floor is a workshop, their flats and the named civic site.
    pub(super) use_: Option<Use>,
    pub(super) workshop: bool,
    pub(super) flats: u32,
    pub(super) civic: Option<String>,
    /// The perf pass phase 10 (OPEN-182): the cut's member lots, id and
    /// rect in metres, for the door's lot (`lot=`).
    pub(super) lots: Vec<(String, [f64; 4])>,
    /// The perf pass phase 10: the cut holds a lot centre (it has members);
    /// one that does not is a piece of a lot another cut holds.
    pub(super) own_lot: bool,
    /// The city milestone C6a: the members' work units (from 0.2's levels),
    /// ground first, and the storeys holding their flats.
    pub(super) units: Vec<Unit>,
    pub(super) flat_span: Option<(u8, u8)>,
    /// C7: the members' lift cores: their count and the highest storey one
    /// serves (`lift=` on the cut's host).
    pub(super) lift: Option<(u8, u8)>,
    /// C6a: each member lot's street doors' `out` tiles (sim tiles), by the
    /// member's index in `lots`.
    pub(super) lot_outs: Vec<Vec<(usize, usize)>>,
}

/// The city milestone C6a: one work unit of a cut, a use's place: its storey,
/// its kind (`UNIT_SHOP`, `UNIT_WORKSHOP`, `UNIT_OFFICE`, `UNIT_WORKS`) and
/// its member lot (an index into `Seg.lots`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct Unit {
    pub(super) level: u8,
    pub(super) kind: u8,
    pub(super) lot: usize,
}

/// A `B` line before it is written: kind, rect, tier, floors and the keyed fields.
pub(super) type OutB = (BuildingKind, Rect, u8, u8, BuildingExtra);

/// C9a: the zones whose homes a Farm takes past the cuts nobody lives in,
/// in order (the Spire's towers never: they are the execs' homes).
pub(super) const FARM_HOME_ZONES: [Zone; 4] = [Zone::Vats, Zone::Sump, Zone::Mid, Zone::Civic];

pub(super) const UNIT_SHOP: u8 = 0;
pub(super) const UNIT_WORKSHOP: u8 = 1;
pub(super) const UNIT_OFFICE: u8 = 2;
pub(super) const UNIT_WORKS: u8 = 3;

/// C6a: the unit kinds a use takes, in its order of preference: a venue a
/// shop front, a Hideout the alley's workshop, a Security Office or a Lot
/// an office stack first.
fn unit_order(kind: BuildingKind) -> [u8; 4] {
    match kind {
        BuildingKind::Market | BuildingKind::Bar => [UNIT_SHOP, UNIT_WORKSHOP, UNIT_OFFICE, UNIT_WORKS],
        BuildingKind::Hideout => [UNIT_WORKSHOP, UNIT_SHOP, UNIT_OFFICE, UNIT_WORKS],
        _ => [UNIT_OFFICE, UNIT_WORKS, UNIT_WORKSHOP, UNIT_SHOP],
    }
}

/// C6a: take the unit `kind` prefers from `free` (its kind order, then the
/// lowest storey, then file order).
pub(super) fn take_unit(free: &mut Vec<Unit>, kind: BuildingKind) -> Option<Unit> {
    let order = unit_order(kind);
    let rank = |u: &Unit| (order.iter().position(|&k| k == u.kind).unwrap_or(4), u.level);
    let j = (0..free.len()).min_by_key(|&j| (rank(&free[j]), j))?;
    Some(free.remove(j))
}

/// The perf pass phase 10 (OPEN-182, MAP_RECONCILE "TheCity" 2): the lot
/// the cut's door stands on, so the view can tell which of a multi-lot cut's
/// buildings is the venue: of the cut's members, the one whose rect is
/// nearest the door tile's centre (0 inside it), ties the lower id. `None`
/// for a cut holding no lot centre (a piece of a lot another cut names).
pub(super) fn door_lot(f: &Frame, sg: &Seg) -> Option<String> {
    if !sg.own_lot {
        return None;
    }
    let (mx, my) = (f.x0 + (sg.door.0 as f64 + 0.5) * f.s, f.y1 - (sg.door.1 as f64 + 0.5) * f.s);
    let gap = |r: &[f64; 4]| {
        let dx = (r[0] - mx).max(mx - r[2]).max(0.0);
        let dy = (r[1] - my).max(my - r[3]).max(0.0);
        dx + dy
    };
    sg.lots.iter().min_by(|a, b| gap(&a.1).total_cmp(&gap(&b.1)).then_with(|| a.0.cmp(&b.0))).map(|(id, _)| id.clone())
}

/// A named civic site's rank when two share a cut (the first is kept).
pub(super) fn civic_rank(name: &str) -> usize {
    match name {
        "city_hall" => 0,
        n if n.starts_with("precinct") => 1,
        "depot" => 2,
        "crematorium" => 3,
        _ => 4,
    }
}

pub(super) fn zone_letter(z: &str) -> Option<Zone> {
    Some(match z {
        "Sump" => Zone::Sump,
        "Civic" => Zone::Civic,
        "Mid" => Zone::Mid,
        "Spire" => Zone::Spire,
        "Vats" => Zone::Vats,
        _ => return None,
    })
}

pub(super) fn tier_of(z: Zone) -> u8 {
    match z {
        Zone::Sump | Zone::Vats => 0,
        Zone::Mid | Zone::Civic => 1,
        Zone::Spire => 2,
    }
}

/// Ramer-Douglas-Peucker over tile points, keeping the forced indices.
fn simplify(pts: &[(f64, f64)], tol: f64, keep: &BTreeSet<usize>) -> Vec<usize> {
    fn rdp(pts: &[(f64, f64)], a: usize, b: usize, tol: f64, out: &mut BTreeSet<usize>) {
        if b <= a + 1 {
            return;
        }
        let (ax, ay) = pts[a];
        let (bx, by) = pts[b];
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (mut best, mut at) = (0.0, a);
        for (i, &(px, py)) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = ((px - ax) * dy - (py - ay) * dx).abs() / len;
            if d > best {
                best = d;
                at = i;
            }
        }
        if best > tol {
            out.insert(at);
            rdp(pts, a, at, tol, out);
            rdp(pts, at, b, tol, out);
        }
    }
    let mut out: BTreeSet<usize> = keep.iter().copied().filter(|&i| i < pts.len()).collect();
    out.insert(0);
    out.insert(pts.len() - 1);
    let anchors: Vec<usize> = out.iter().copied().collect();
    for w in anchors.windows(2) {
        rdp(pts, w[0], w[1], tol, &mut out);
    }
    out.into_iter().collect()
}

/// A polyline in metres to tile waypoints inside the frame (the longest run
/// of in-frame points), keeping the forced indices; `None` under two points.
pub(super) fn line_waypoints(
    f: &Frame,
    pts: &[(f64, f64)],
    keep: &BTreeSet<usize>,
) -> Option<(Vec<[u16; 2]>, Vec<u16>)> {
    let mut best: (usize, usize) = (0, 0);
    let mut start = None;
    for i in 0..=pts.len() {
        let ok = i < pts.len() && f.tile(pts[i].0, pts[i].1).is_some();
        match (ok, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if i - s > best.1 - best.0 {
                    best = (s, i);
                }
                start = None;
            }
            _ => {}
        }
    }
    let run = &pts[best.0..best.1];
    if run.len() < 2 {
        return None;
    }
    let tiles: Vec<(f64, f64)> =
        run.iter().map(|&(x, y)| f.tile(x, y).map_or((0.0, 0.0), |(tx, ty)| (tx as f64, ty as f64))).collect();
    let keep: BTreeSet<usize> = keep.iter().filter(|&&i| i >= best.0 && i < best.1).map(|&i| i - best.0).collect();
    let mut out: Vec<[u16; 2]> = Vec::new();
    // The view milestone V9 (OPEN-255): the kept points are the stations
    // (`TransitLineCfg::stations`); the simplification's are bends.
    let mut stations: Vec<u16> = Vec::new();
    for i in simplify(&tiles, 1.5, &keep) {
        let p = [tiles[i].0 as u16, tiles[i].1 as u16];
        if out.last() != Some(&p) {
            out.push(p);
        }
        let at = u16::try_from(out.len() - 1).unwrap_or(u16::MAX);
        if keep.contains(&i) && stations.last() != Some(&at) {
            stations.push(at);
        }
    }
    (out.len() >= 2).then_some((out, stations))
}
