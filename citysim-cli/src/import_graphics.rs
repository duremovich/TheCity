//! Scale S3b: the TheCityGraphics adapter (docs/IMPORT_GRAPHICS.md).
//!
//! `citysim-cli import-graphics --src <isle_m/out> --out <map> [--downsample k]
//! [--residents N] [--crop x0,y0,x1,y1]` reads the graphics pipeline's
//! generated city (read only: nothing is ever written under `--src`) and
//! writes the sim's own map text: roads from the street ways, buildings from
//! the blocks' platform parts cut at lot boundaries (floors and flats from the
//! per-lot `sim_layout.toml` where one exists), districts from the
//! superblocks, El lines from the elevated rail (and the tram, when its file
//! exists). Their format still evolves: a missing file or field is an error
//! naming the file, the record and the field, never a panic.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::Value;

use citysim::components::{BuildingKind, Rect, Zone};
use citysim::config::TransitLineCfg;
use citysim::map::{MapDistrict, MapText};

#[derive(clap::Args, Clone, Debug)]
pub struct ImportArgs {
    /// The pipeline's output directory (`assets/city/isle_m/out`).
    #[arg(long)]
    pub src: PathBuf,
    /// The map file to write.
    #[arg(long)]
    pub out: PathBuf,
    /// Sim tiles per graphics tile along each axis (1: their 2 m tiles).
    #[arg(long, default_value_t = 1)]
    pub downsample: u32,
    /// Residents the employers are sized for (the generator's mix per 1,000).
    #[arg(long, default_value_t = 20_000)]
    pub residents: u32,
    /// Import only this window, in metres of their frame (x east, y north):
    /// `x0,y0,x1,y1`.
    #[arg(long)]
    pub crop: Option<String>,
}

/// What the import read, wrote and could not map.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub w: usize,
    pub h: usize,
    pub tile_m: f64,
    pub lots: usize,
    pub lots_with_layout: usize,
    pub parts: usize,
    pub segments: usize,
    pub dropped_small: usize,
    pub dropped_no_road: usize,
    pub dropped_overlap: usize,
    pub lots_dropped: usize,
    pub flats: usize,
    pub shop_units: usize,
    pub kinds: BTreeMap<BuildingKind, usize>,
    pub home_floors: usize,
    pub shortfall: Vec<String>,
    pub districts: Vec<String>,
    pub lines: Vec<String>,
    pub piers: usize,
    pub unmapped: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn print(&self) {
        println!(
            "map {} x {} tiles ({} m a tile); {} lots read ({} with a sim_layout), {} block parts",
            self.w, self.h, self.tile_m, self.lots, self.lots_with_layout, self.parts
        );
        println!(
            "buildings: {} segments kept; dropped {} too small, {} with no road at any side, {} overlapping ({} lots in dropped parts or segments)",
            self.segments, self.dropped_small, self.dropped_no_road, self.dropped_overlap, self.lots_dropped
        );
        println!(
            "flats read {} (layouts only), shop units {}; Home floor slots {} ({} residents at 5 a floor)",
            self.flats,
            self.shop_units,
            self.home_floors,
            self.home_floors * 5
        );
        for (k, n) in &self.kinds {
            println!("  {:<15} {n:>6}", format!("{k:?}"));
        }
        for s in &self.shortfall {
            println!("  short: {s}");
        }
        println!("districts: {}", self.districts.join(", "));
        println!("El lines: {}", self.lines.join(", "));
        println!("edge piers carved: {}", self.piers);
        for u in &self.unmapped {
            println!("  unmapped: {u}");
        }
        for wn in &self.warnings {
            println!("  warning: {wn}");
        }
    }
}

// ---------------------------------------------------------------------------
// JSON access with messages that name the file, the record and the field.
// ---------------------------------------------------------------------------

fn read_json(dir: &Path, name: &str) -> Result<Value, String> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: not JSON: {e}", path.display()))
}

fn get<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("{ctx}: missing field \"{key}\""))
}

fn arr<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a Vec<Value>, String> {
    get(v, key, ctx)?.as_array().ok_or_else(|| format!("{ctx}: \"{key}\" is not a list"))
}

fn text<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a str, String> {
    get(v, key, ctx)?.as_str().ok_or_else(|| format!("{ctx}: \"{key}\" is not a string"))
}

fn num(v: &Value, ctx: &str) -> Result<f64, String> {
    v.as_f64().ok_or_else(|| format!("{ctx}: {v} is not a number"))
}

/// `[x0, y0, x1, y1]` metres.
fn rect_m(v: &Value, ctx: &str) -> Result<[f64; 4], String> {
    let a = v.as_array().filter(|a| a.len() == 4).ok_or_else(|| format!("{ctx}: not a [x0, y0, x1, y1] rect"))?;
    let r = [num(&a[0], ctx)?, num(&a[1], ctx)?, num(&a[2], ctx)?, num(&a[3], ctx)?];
    if !(r[0] <= r[2] && r[1] <= r[3]) {
        return Err(format!("{ctx}: rect {r:?} has x0 > x1 or y0 > y1"));
    }
    Ok(r)
}

fn field_rect(v: &Value, key: &str, ctx: &str) -> Result<[f64; 4], String> {
    rect_m(get(v, key, ctx)?, &format!("{ctx} \"{key}\""))
}

/// `[[x, y, ...], ...]` points.
fn points(v: &Value, key: &str, ctx: &str) -> Result<Vec<(f64, f64)>, String> {
    arr(v, key, ctx)?
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let c = format!("{ctx} \"{key}\"[{i}]");
            let a = p.as_array().filter(|a| a.len() >= 2).ok_or_else(|| format!("{c}: not a point"))?;
            Ok((num(&a[0], &c)?, num(&a[1], &c)?))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The grid frame: metres (y north) to tiles (y south).
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
struct Frame {
    x0: f64,
    y1: f64,
    s: f64,
    w: usize,
    h: usize,
}

impl Frame {
    /// A rect's tiles by rounded edges, half-open, clamped: `(x0, y0, x1, y1)`.
    fn round(&self, r: [f64; 4]) -> (usize, usize, usize, usize) {
        let cx = |m: f64| (((m - self.x0) / self.s).round().max(0.0) as usize).min(self.w);
        let cy = |m: f64| (((self.y1 - m) / self.s).round().max(0.0) as usize).min(self.h);
        (cx(r[0]), cy(r[3]), cx(r[2]), cy(r[1]))
    }

    /// A road's tiles: rounded edges like [`Frame::round`], but at least one
    /// tile across (a 2 m alley keeps a tile at any downsample).
    fn road(&self, r: [f64; 4]) -> (usize, usize, usize, usize) {
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

    fn tile(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let tx = ((x - self.x0) / self.s).floor();
        let ty = ((self.y1 - y) / self.s).floor();
        (tx >= 0.0 && ty >= 0.0 && (tx as usize) < self.w && (ty as usize) < self.h)
            .then_some((tx as usize, ty as usize))
    }
}

fn inside(r: [f64; 4], win: [f64; 4]) -> bool {
    r[0] >= win[0] && r[1] >= win[1] && r[2] <= win[2] && r[3] <= win[3]
}

fn overlaps(r: [f64; 4], win: [f64; 4]) -> bool {
    r[0] < win[2] && win[0] < r[2] && r[1] < win[3] && win[1] < r[3]
}

// ---------------------------------------------------------------------------
// Lots and their layouts.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Lot {
    rect: [f64; 4],
    front: char,
    floors: u32,
    flats: usize,
    shops: usize,
    layout: bool,
    residential: bool,
    door_off: Option<usize>,
}

/// Floors (non-roof), flats (units with a living room), shop units, the
/// building kind and the first street door's x offset of one `sim_layout.toml`.
fn read_layout(path: &Path) -> Result<(u32, usize, usize, bool, Option<usize>), String> {
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
struct Seg {
    rect: Rect,
    door: (usize, usize),
    district: usize,
    floors: u32,
    shops: usize,
    residential: bool,
}

fn zone_letter(z: &str) -> Option<Zone> {
    Some(match z {
        "Sump" => Zone::Sump,
        "Civic" => Zone::Civic,
        "Mid" => Zone::Mid,
        "Spire" => Zone::Spire,
        "Vats" => Zone::Vats,
        _ => return None,
    })
}

fn tier_of(z: Zone) -> u8 {
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
fn line_waypoints(f: &Frame, pts: &[(f64, f64)], keep: &BTreeSet<usize>) -> Option<Vec<[u16; 2]>> {
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
    for i in simplify(&tiles, 1.5, &keep) {
        let p = [tiles[i].0 as u16, tiles[i].1 as u16];
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    (out.len() >= 2).then_some(out)
}

/// The import itself: the map text and the report.
pub fn build(args: &ImportArgs) -> Result<(String, Report), String> {
    let src = &args.src;
    if !src.is_dir() {
        return Err(format!("--src {}: not a directory", src.display()));
    }
    let k = args.downsample.max(1);
    let mut rep = Report::default();

    let districts_json = read_json(src, "districts.json")?;
    let blocks_json = read_json(src, "blocks.json")?;
    let lots_json = read_json(src, "lots.json")?;
    let roads_json = read_json(src, "roads.json")?;
    let elevated_json = read_json(src, "elevated.json")?;
    let tile_src = lots_json.get("tile").and_then(|t| t.as_f64()).unwrap_or(2.0);
    let s = tile_src * f64::from(k);
    rep.tile_m = s;

    // The city's extent: terrain.json's size_m, else every block and way.
    let size = match read_json(src, "terrain.json") {
        Ok(t) => {
            let ctx = "terrain.json summary";
            let a = arr(get(&t, "summary", "terrain.json")?, "size_m", ctx)?;
            if a.len() != 2 {
                return Err(format!("{ctx}: size_m is not [w, h]"));
            }
            [0.0, 0.0, num(&a[0], ctx)?, num(&a[1], ctx)?]
        }
        Err(e) => {
            rep.warnings.push(format!("{e}; the extent is the blocks' and ways' bounding box"));
            let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for (file, v, list) in [("blocks.json", &blocks_json, "blocks"), ("roads.json", &roads_json, "ways")] {
                for (i, x) in arr(v, list, file)?.iter().enumerate() {
                    let r = field_rect(x, "rect_m", &format!("{file} {list}[{i}]"))?;
                    b = [b[0].min(r[0]), b[1].min(r[1]), b[2].max(r[2]), b[3].max(r[3])];
                }
            }
            b
        }
    };
    let win = match &args.crop {
        None => size,
        Some(c) => {
            let v: Vec<f64> = c
                .split(',')
                .map(|x| x.trim().parse::<f64>())
                .collect::<Result<_, _>>()
                .map_err(|e| format!("--crop {c}: {e}"))?;
            let r: [f64; 4] = v.try_into().map_err(|_| format!("--crop {c}: expected x0,y0,x1,y1"))?;
            if !(r[0] < r[2] && r[1] < r[3]) {
                return Err(format!("--crop {c}: empty window"));
            }
            r
        }
    };
    let (w, h) = (((win[2] - win[0]) / s).ceil() as usize, ((win[3] - win[1]) / s).ceil() as usize);
    if w < 8 || h < 8 || w > citysim::map::MAX_SIDE || h > citysim::map::MAX_SIDE {
        return Err(format!("the map would be {w} x {h} tiles; each side must be 8..={}", citysim::map::MAX_SIDE));
    }
    let f = Frame { x0: win[0], y1: win[3], s, w, h };
    rep.w = w;
    rep.h = h;
    let at = |x: usize, y: usize| y * w + x;

    // --- districts --------------------------------------------------------------
    let rows = arr(&districts_json, "rows", "districts.json")?;
    let mut drow: BTreeMap<String, (String, Zone)> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        let ctx = format!("districts.json rows[{i}]");
        let id = text(r, "id", &ctx)?.to_string();
        let name = text(r, "name", &ctx)?.to_string();
        let z = text(r, "zone", &ctx)?;
        let zone = zone_letter(z).unwrap_or_else(|| {
            rep.warnings.push(format!("{ctx}: zone {z:?} unknown to the sim; read as Mid"));
            Zone::Mid
        });
        drow.insert(id, (name, zone));
    }
    // Superblocks seed the district grid (multi-source BFS fills the rest).
    let mut dgrid: Vec<u16> = vec![u16::MAX; w * h];
    let mut queue = VecDeque::new();
    let mut used_ids: Vec<String> = Vec::new();
    for (i, sb) in arr(&districts_json, "superblocks", "districts.json")?.iter().enumerate() {
        let ctx = format!("districts.json superblocks[{i}]");
        let d = text(sb, "district", &ctx)?;
        if !drow.contains_key(d) {
            return Err(format!("{ctx}: district {d:?} has no row"));
        }
        let r = field_rect(sb, "rect_m", &ctx)?;
        if !overlaps(r, win) {
            continue;
        }
        let idx = match used_ids.iter().position(|x| x == d) {
            Some(p) => p,
            None => {
                used_ids.push(d.to_string());
                used_ids.len() - 1
            }
        } as u16;
        let (x0, y0, x1, y1) = f.round(r);
        for y in y0..y1 {
            for x in x0..x1 {
                if dgrid[at(x, y)] == u16::MAX {
                    dgrid[at(x, y)] = idx;
                    queue.push_back((x, y));
                }
            }
        }
    }
    if used_ids.is_empty() {
        return Err("districts.json: no superblock lies in the window".into());
    }
    // A district with no superblock of its own (D7 The Stack: its block says
    // D7, its superblock D1) takes its blocks' tiles.
    let superblock_ids = used_ids.clone();
    for (i, b) in arr(&blocks_json, "blocks", "blocks.json")?.iter().enumerate() {
        let ctx = format!("blocks.json blocks[{i}]");
        let Some(d) = b.get("district").and_then(|d| d.as_str()) else { continue };
        if superblock_ids.iter().any(|x| x == d) || !drow.contains_key(d) {
            continue;
        }
        let r = field_rect(b, "rect_m", &ctx)?;
        if !overlaps(r, win) {
            continue;
        }
        let idx = match used_ids.iter().position(|x| x == d) {
            Some(p) => p,
            None => {
                used_ids.push(d.to_string());
                used_ids.len() - 1
            }
        } as u16;
        let (x0, y0, x1, y1) = f.round(r);
        for y in y0..y1 {
            for x in x0..x1 {
                if dgrid[at(x, y)] != idx {
                    dgrid[at(x, y)] = idx;
                    queue.push_back((x, y));
                }
            }
        }
    }
    // Sort the districts by their row order (D1, D2, ...), then remap.
    let order: Vec<String> =
        rows.iter().filter_map(|r| r.get("id").and_then(|i| i.as_str()).map(String::from)).collect();
    let mut sorted = used_ids.clone();
    sorted.sort_by_key(|d| order.iter().position(|o| o == d).unwrap_or(usize::MAX));
    if sorted.len() > citysim::MAX_DISTRICTS {
        return Err(format!("{} districts; the sim holds {}", sorted.len(), citysim::MAX_DISTRICTS));
    }
    let remap: Vec<u16> = used_ids.iter().map(|d| sorted.iter().position(|x| x == d).unwrap_or(0) as u16).collect();
    for v in dgrid.iter_mut() {
        if *v != u16::MAX {
            *v = remap[usize::from(*v)];
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        let d = dgrid[at(x, y)];
        for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
            if nx < w && ny < h && dgrid[at(nx, ny)] == u16::MAX {
                dgrid[at(nx, ny)] = d;
                queue.push_back((nx, ny));
            }
        }
    }
    let districts: Vec<MapDistrict> = sorted
        .iter()
        .map(|d| {
            let (name, zone) = drow[d].clone();
            MapDistrict { name: format!("{d} {name}"), zone, x_from: 0, x_to: w as u16 }
        })
        .collect();
    for (d, (name, _)) in &drow {
        if !sorted.contains(d) {
            rep.unmapped.push(format!("district {d} {name}: no superblock in the window (not generated yet?)"));
        }
    }
    rep.districts = districts.iter().map(|d| format!("{} ({})", d.name, d.zone.letter())).collect();
    let zone_of_d: Vec<Zone> = districts.iter().map(|d| d.zone).collect();

    // --- tiles: water, land, roads ------------------------------------------------
    let mut tiles = vec!['~'; w * h];
    let land = |r: [f64; 4], tiles: &mut Vec<char>| {
        let (x0, y0, x1, y1) = f.round(r);
        for y in y0..y1 {
            for x in x0..x1 {
                tiles[at(x, y)] = '.';
            }
        }
    };
    for (i, sb) in arr(&districts_json, "superblocks", "districts.json")?.iter().enumerate() {
        land(field_rect(sb, "rect_m", &format!("districts.json superblocks[{i}]"))?, &mut tiles);
    }
    for list in ["blocks", "open", "greens_m", "leftover_m", "voids_m"] {
        let Some(items) = blocks_json.get(list).and_then(|v| v.as_array()) else {
            rep.warnings.push(format!("blocks.json: no \"{list}\" list"));
            continue;
        };
        for (i, it) in items.iter().enumerate() {
            let ctx = format!("blocks.json {list}[{i}]");
            let r = if it.is_array() { rect_m(it, &ctx)? } else { field_rect(it, "rect_m", &ctx)? };
            land(r, &mut tiles);
        }
    }
    let ways = arr(&roads_json, "ways", "roads.json")?;
    let mut by_cls: BTreeMap<String, usize> = BTreeMap::new();
    // A rear alley inside a block is built over when the block's parts are
    // too thin for the tile size (`whole` below).
    let (mut alley, mut street) = (vec![false; w * h], vec![false; w * h]);
    for (i, wy) in ways.iter().enumerate() {
        let ctx = format!("roads.json ways[{i}]");
        let r = field_rect(wy, "rect_m", &ctx)?;
        let cls = text(wy, "cls", &ctx)?;
        *by_cls.entry(cls.to_string()).or_default() += 1;
        let (x0, y0, x1, y1) = f.road(r);
        for y in y0..y1 {
            for x in x0..x1 {
                tiles[at(x, y)] = '=';
                if cls == "alley" {
                    alley[at(x, y)] = true;
                } else {
                    street[at(x, y)] = true;
                }
            }
        }
    }
    let road = |tiles: &Vec<char>, x: usize, y: usize| tiles[at(x, y)] == '=';
    // An island has no road on the map's edge: carve a pier from the nearest
    // road to each side, so migrants have a way in (`Map::edge_roads`).
    let has_edge = (0..w).any(|x| road(&tiles, x, 0) || road(&tiles, x, h - 1))
        || (0..h).any(|y| road(&tiles, 0, y) || road(&tiles, w - 1, y));
    if !has_edge {
        let roads: Vec<(usize, usize)> =
            (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| road(&tiles, x, y)).collect();
        if roads.is_empty() {
            return Err("no road in the window".into());
        }
        for side in ["west", "east", "north", "south"] {
            let dist = |p: &(usize, usize)| match side {
                "west" => p.0,
                "east" => w - 1 - p.0,
                "north" => p.1,
                _ => h - 1 - p.1,
            };
            let Some(&(x, y)) = roads.iter().min_by_key(|p| (dist(p), p.1, p.0)) else { break };
            let path: Vec<(usize, usize)> = match side {
                "west" => (0..x).map(|xx| (xx, y)).collect(),
                "east" => (x + 1..w).map(|xx| (xx, y)).collect(),
                "north" => (0..y).map(|yy| (x, yy)).collect(),
                _ => (y + 1..h).map(|yy| (x, yy)).collect(),
            };
            for (px, py) in path {
                tiles[at(px, py)] = '=';
            }
            rep.piers += 1;
        }
    }

    // --- lots and layouts ---------------------------------------------------------
    let lots_list = arr(&lots_json, "lots", "lots.json")?;
    let mut lots_by_block: BTreeMap<String, Vec<Lot>> = BTreeMap::new();
    for (i, l) in lots_list.iter().enumerate() {
        let ctx = format!("lots.json lots[{i}]");
        let id = text(l, "id", &ctx)?;
        let ctx = format!("{ctx} ({id})");
        let rect = field_rect(l, "rect_m", &ctx)?;
        if !inside(rect, win) {
            continue;
        }
        rep.lots += 1;
        let block = text(l, "block", &ctx)?.to_string();
        let front = text(l, "front", &ctx)?.chars().next().unwrap_or('S');
        let fl = arr(l, "floors", &ctx)?;
        if fl.len() != 2 {
            return Err(format!("{ctx}: \"floors\" is not [lo, hi]"));
        }
        let (lo, hi) = (num(&fl[0], &ctx)?.max(1.0) as u32, num(&fl[1], &ctx)?.max(1.0) as u32);
        let seed = get(l, "seed", &ctx)?.as_u64().ok_or_else(|| format!("{ctx}: \"seed\" is not an integer"))?;
        // Massing floors are not in lots.json (only the district's range):
        // a deterministic pick in the range from the lot's seed.
        let mut lot = Lot {
            rect,
            front,
            floors: lo + (seed % u64::from(hi.saturating_sub(lo) + 1)) as u32,
            flats: 0,
            shops: 0,
            layout: false,
            residential: true,
            door_off: None,
        };
        let layout = src.join("bldg").join(id).join("sim_layout.toml");
        if layout.is_file() {
            let (storeys, flats, shops, residential, door_off) = read_layout(&layout)?;
            lot.floors = storeys;
            lot.flats = flats;
            lot.shops = shops;
            lot.residential = residential;
            lot.door_off = door_off;
            lot.layout = true;
            rep.lots_with_layout += 1;
            rep.flats += flats;
            rep.shop_units += shops;
        }
        lots_by_block.entry(block).or_default().push(lot);
    }

    // --- buildings: block parts cut at lot boundaries ---------------------------
    let mut occupied = vec![false; w * h];
    let mut segs: Vec<Seg> = Vec::new();
    for (i, b) in arr(&blocks_json, "blocks", "blocks.json")?.iter().enumerate() {
        let ctx = format!("blocks.json blocks[{i}]");
        let id = text(b, "id", &ctx)?;
        let ctx = format!("{ctx} ({id})");
        let brect = field_rect(b, "rect_m", &ctx)?;
        if !inside(brect, win) {
            continue;
        }
        let parts: Vec<[f64; 4]> = match b.get("parts_m").and_then(|p| p.as_array()) {
            Some(p) if !p.is_empty() => p
                .iter()
                .enumerate()
                .map(|(j, r)| rect_m(r, &format!("{ctx} parts_m[{j}]")))
                .collect::<Result<_, _>>()?,
            _ => vec![brect],
        };
        // Parts thinner than 3 tiles (the Sump's 12-16 m lots past 4 m
        // tiles): the whole block is one part, over its rear alleys.
        let whole = parts.len() > 1
            && parts.iter().any(|p| {
                let (a, b, c, d) = f.round(*p);
                c.saturating_sub(a) < 3 || d.saturating_sub(b) < 3
            });
        let parts = if whole { vec![brect] } else { parts };
        let block_lots = lots_by_block.get(id).cloned().unwrap_or_default();
        let vats_block =
            b.get("district").and_then(|d| d.as_str()).and_then(|d| drow.get(d)).is_some_and(|(_, z)| *z == Zone::Vats);
        for part in parts {
            rep.parts += 1;
            let lots: Vec<&Lot> = block_lots
                .iter()
                .filter(|l| {
                    let (cx, cy) = ((l.rect[0] + l.rect[2]) / 2.0, (l.rect[1] + l.rect[3]) / 2.0);
                    cx >= part[0] && cx < part[2] && cy >= part[1] && cy < part[3]
                })
                .collect();
            if lots.is_empty() {
                continue;
            }
            let (px0, py0, px1, py1) = f.round(part);
            if px1.saturating_sub(px0) < 3 || py1.saturating_sub(py0) < 3 {
                rep.dropped_small += 1;
                rep.lots_dropped += lots.len();
                continue;
            }
            // The part's front: its lots' most common front.
            let mut fronts: BTreeMap<char, usize> = BTreeMap::new();
            for l in &lots {
                *fronts.entry(l.front).or_default() += 1;
            }
            let front = fronts.iter().max_by_key(|(c, n)| (**n, std::cmp::Reverse(**c))).map_or('S', |(c, _)| *c);
            let along_x = matches!(front, 'N' | 'S');
            // Cuts along the frontage at lot edges, every segment >= 3 tiles.
            let mut edges: Vec<usize> = lots
                .iter()
                .map(|l| {
                    let (_, y0, x1, _) = f.round(l.rect);
                    if along_x {
                        x1
                    } else {
                        y0
                    }
                })
                .collect();
            edges.sort_unstable();
            edges.dedup();
            let (a0, a1) = if along_x { (px0, px1) } else { (py0, py1) };
            let mut cuts = vec![a0];
            // The Vats' godowns stay whole (a Vat Farm needs 60 interior tiles).
            if vats_block {
                edges.clear();
            }
            for e in edges {
                let last = *cuts.last().unwrap_or(&a0);
                if e >= last + 3 && e + 3 <= a1 {
                    cuts.push(e);
                }
            }
            cuts.push(a1);
            for c in cuts.windows(2) {
                let (s0, s1) = (c[0], c[1]);
                let (mut x0, mut y0, mut x1, mut y1) = if along_x { (s0, py0, s1, py1) } else { (px0, s0, px1, s1) };
                // Shrink off road and taken tiles, edge by edge.
                let bad = |x: usize, y: usize| {
                    let i = at(x, y);
                    (tiles[i] == '=' && !(whole && alley[i] && !street[i])) || occupied[i]
                };
                // The edge with the largest share of bad tiles goes first (a
                // road along one side is one edge, not a column at a time).
                for _ in 0..8 {
                    if x1 <= x0 + 2 || y1 <= y0 + 2 {
                        break;
                    }
                    let share = |n: usize, len: usize| n as f64 / len as f64;
                    let (hw, hh) = (x1 - x0, y1 - y0);
                    let edges = [
                        share((y0..y1).filter(|&y| bad(x0, y)).count(), hh),
                        share((y0..y1).filter(|&y| bad(x1 - 1, y)).count(), hh),
                        share((x0..x1).filter(|&x| bad(x, y0)).count(), hw),
                        share((x0..x1).filter(|&x| bad(x, y1 - 1)).count(), hw),
                    ];
                    let (e, top) =
                        edges.iter().enumerate().fold((0, 0.0), |b, (i, &v)| if v > b.1 { (i, v) } else { b });
                    if top == 0.0 {
                        break;
                    }
                    match e {
                        0 => x0 += 1,
                        1 => x1 -= 1,
                        2 => y0 += 1,
                        _ => y1 -= 1,
                    }
                }
                let members: Vec<&&Lot> = lots
                    .iter()
                    .filter(|l| {
                        let (cx, cy) = ((l.rect[0] + l.rect[2]) / 2.0, (l.rect[1] + l.rect[3]) / 2.0);
                        let (tx, ty) = (((cx - f.x0) / s) as usize, ((f.y1 - cy) / s) as usize);
                        if along_x {
                            (s0..s1).contains(&tx)
                        } else {
                            (s0..s1).contains(&ty)
                        }
                    })
                    .collect();
                if x1 < x0 + 3 || y1 < y0 + 3 {
                    rep.dropped_small += 1;
                    rep.lots_dropped += members.len();
                    continue;
                }
                if (y0..y1).any(|y| (x0..x1).any(|x| bad(x, y))) {
                    rep.dropped_overlap += 1;
                    rep.lots_dropped += members.len();
                    continue;
                }
                let rect = Rect { x: x0 as u16, y: y0 as u16, w: (x1 - x0) as u16, h: (y1 - y0) as u16 };
                // The door: the front side first, from its middle out (or the
                // layout's street door on a one-lot cut), then the other sides.
                let hint = match (members.as_slice(), k) {
                    ([one], 1) if one.layout && front == 'S' => one.door_off.map(|o| x0 + o.min(x1 - x0 - 1)),
                    ([one], 1) if one.layout && front == 'N' => one.door_off.map(|o| x1 - 1 - o.min(x1 - x0 - 1)),
                    _ => None,
                };
                let side_tiles = |side: char| -> Vec<((usize, usize), (isize, isize))> {
                    let (n, mid) = match side {
                        'N' | 'S' => (x1 - x0, (x0 + x1) / 2),
                        _ => (y1 - y0, (y0 + y1) / 2),
                    };
                    let mut order: Vec<usize> = Vec::new();
                    if let Some(hx) = hint.filter(|_| side == front) {
                        order.push(hx);
                    }
                    for d in 0..n {
                        for v in [mid as isize - d as isize, mid as isize + d as isize] {
                            if v >= 0 {
                                order.push(v as usize);
                            }
                        }
                    }
                    order
                        .into_iter()
                        .filter_map(|v| match side {
                            'S' if v > x0 && v + 1 < x1 => Some(((v, y1 - 1), (0, 1))),
                            'N' if v > x0 && v + 1 < x1 => Some(((v, y0), (0, -1))),
                            'E' if v > y0 && v + 1 < y1 => Some(((x1 - 1, v), (1, 0))),
                            'W' if v > y0 && v + 1 < y1 => Some(((x0, v), (-1, 0))),
                            _ => None,
                        })
                        .collect()
                };
                let mut door = None;
                for side in std::iter::once(front).chain(['S', 'N', 'E', 'W'].into_iter().filter(|&c| c != front)) {
                    for ((dx, dy), (ox, oy)) in side_tiles(side) {
                        let (nx, ny) = (dx as isize + ox, dy as isize + oy);
                        if nx >= 0
                            && ny >= 0
                            && (nx as usize) < w
                            && (ny as usize) < h
                            && road(&tiles, nx as usize, ny as usize)
                        {
                            door = Some((dx, dy));
                            break;
                        }
                    }
                    if door.is_some() {
                        break;
                    }
                }
                let Some(door) = door else {
                    rep.dropped_no_road += 1;
                    rep.lots_dropped += members.len();
                    continue;
                };
                for y in y0..y1 {
                    for x in x0..x1 {
                        occupied[at(x, y)] = true;
                    }
                }
                let floors = if members.is_empty() {
                    lots.iter().map(|l| l.floors).max().unwrap_or(1)
                } else {
                    let sum: u32 = members.iter().map(|l| l.floors).sum();
                    (f64::from(sum) / members.len() as f64).round() as u32
                };
                let (cx, cy) =
                    (usize::from(rect.x) + usize::from(rect.w) / 2, usize::from(rect.y) + usize::from(rect.h) / 2);
                segs.push(Seg {
                    rect,
                    door,
                    district: usize::from(dgrid[at(cx, cy)]),
                    floors: floors.clamp(1, 255),
                    shops: members.iter().map(|l| l.shops).sum(),
                    residential: members.iter().all(|l| l.residential),
                });
            }
        }
    }
    rep.segments = segs.len();
    if segs.is_empty() {
        return Err("no lot became a building (the window holds no block with lots?)".into());
    }

    // --- kinds: the generator's mix per 1,000 residents --------------------------
    let per_k = f64::from(args.residents) / 1000.0;
    let target = |per: f64| ((per * per_k).round() as usize).max(1);
    let mut kind: Vec<Option<BuildingKind>> = vec![None; segs.len()];
    let zone = |i: usize| zone_of_d.get(segs[i].district).copied().unwrap_or(Zone::Mid);
    let area = |i: usize| usize::from(segs[i].rect.w) * usize::from(segs[i].rect.h);
    let interior = |i: usize| (usize::from(segs[i].rect.w) - 2) * (usize::from(segs[i].rect.h) - 2);
    let pos = |i: usize| (segs[i].rect.y, segs[i].rect.x);
    // Take `n` of the free candidates passing `ok`, preferred zones first,
    // spread by stride over the city (sorted by position).
    let take = |kind: &mut Vec<Option<BuildingKind>>,
                rep: &mut Report,
                want: BuildingKind,
                n: usize,
                prefer: &[Zone],
                ok: &dyn Fn(usize) -> bool,
                largest: bool| {
        let mut got = 0;
        let mut tiers: Vec<Vec<usize>> = prefer
            .iter()
            .map(|z| (0..segs.len()).filter(|&i| kind[i].is_none() && zone(i) == *z && ok(i)).collect())
            .collect();
        tiers.push((0..segs.len()).filter(|&i| kind[i].is_none() && !prefer.contains(&zone(i)) && ok(i)).collect());
        for mut c in tiers {
            if got >= n {
                break;
            }
            c.retain(|&i| kind[i].is_none());
            if largest {
                c.sort_by_key(|&i| (std::cmp::Reverse(area(i)), pos(i)));
            } else {
                c.sort_by_key(|&i| pos(i));
            }
            let m = (n - got).min(c.len());
            let picks: Vec<usize> =
                if largest { c[..m].to_vec() } else { (0..m).map(|j| c[j * c.len() / m]).collect() };
            for i in picks {
                kind[i] = Some(want);
                got += 1;
            }
        }
        if got < n {
            rep.shortfall.push(format!("{want:?}: {got} of {n}"));
        }
    };
    use BuildingKind as K;
    use Zone as Z;
    let any = |_: usize| true;
    take(&mut kind, &mut rep, K::Jail, 1, &[Z::Civic], &any, true);
    take(&mut kind, &mut rep, K::Hall, 1, &[Z::Civic], &any, true);
    take(&mut kind, &mut rep, K::Warehouse, 1, &[Z::Vats], &any, true);
    take(&mut kind, &mut rep, K::Cemetery, 1, &[Z::Vats], &any, false);
    let farm_ok = |i: usize| interior(i) >= 60;
    take(&mut kind, &mut rep, K::Farm, target(6.0), &[Z::Vats], &farm_ok, true);
    take(&mut kind, &mut rep, K::SecurityOffice, target(1.0), &[Z::Vats], &any, false);
    take(&mut kind, &mut rep, K::Hideout, target(1.0), &[Z::Sump], &any, false);
    let shop_ok = |i: usize| segs[i].shops > 0 || !segs[i].residential;
    for (kd, per) in [(K::Market, 1.5), (K::Bar, 1.5)] {
        let n = target(per);
        let before = rep.shortfall.len();
        take(&mut kind, &mut rep, kd, n, &[Z::Mid, Z::Sump], &shop_ok, false);
        if rep.shortfall.len() > before {
            // Too few shop units: the rest on any block.
            rep.shortfall.pop();
            let have = kind.iter().filter(|x| **x == Some(kd)).count();
            take(&mut kind, &mut rep, kd, n - have, &[Z::Mid, Z::Sump], &any, false);
        }
    }
    take(&mut kind, &mut rep, K::Lot, target(80.0), &[], &any, false);
    // The rest: Blocks in the living districts, open Lots in the Vats and Civic.
    let mut out_b: Vec<(BuildingKind, Rect, u8, u8)> = Vec::new();
    let mut doors: Vec<(usize, usize)> = Vec::new();
    for (i, sg) in segs.iter().enumerate() {
        let z = zone(i);
        let kd = kind[i].unwrap_or(if matches!(z, Z::Vats | Z::Civic) { K::Lot } else { K::Home });
        let floors = if kd == K::Home { sg.floors.min(255) as u8 } else { 1 };
        if kd == K::Home {
            rep.home_floors += usize::from(floors);
        }
        *rep.kinds.entry(kd).or_default() += 1;
        out_b.push((kd, sg.rect, tier_of(z), floors));
        doors.push(sg.door);
    }

    // --- stamp ------------------------------------------------------------------
    for ((kd, r, ..), &(dx, dy)) in out_b.iter().zip(&doors) {
        let (x0, y0, x1, y1) = (
            usize::from(r.x),
            usize::from(r.y),
            usize::from(r.x) + usize::from(r.w),
            usize::from(r.y) + usize::from(r.h),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                let edge = y == y0 || y + 1 == y1 || x == x0 || x + 1 == x1;
                tiles[at(x, y)] = match (kd, edge) {
                    (K::Lot, _) => '.',
                    (_, true) => '#',
                    (K::Farm, false) => 'f',
                    _ => '.',
                };
            }
        }
        tiles[at(dx, dy)] = 'D';
    }
    // File order: kind, then row, then column (the shipped map's order).
    let korder = |kd: BuildingKind| BuildingKind::ALL.iter().position(|&x| x == kd).unwrap_or(usize::MAX);
    out_b.sort_by_key(|&(kd, r, ..)| (korder(kd), r.y, r.x));

    // --- El lines ---------------------------------------------------------------
    let mut lines: Vec<TransitLineCfg> = Vec::new();
    let elements = arr(&elevated_json, "elements", "elevated.json")?;
    let mut stations: Vec<(f64, f64)> = Vec::new();
    let mut unmapped_el: BTreeMap<String, usize> = BTreeMap::new();
    for (i, e) in elements.iter().enumerate() {
        let ctx = format!("elevated.json elements[{i}]");
        if text(e, "kind", &ctx)? == "station" {
            let p = points(e, "pts", &ctx)?;
            if !p.is_empty() {
                let n = p.len() as f64;
                stations.push((p.iter().map(|q| q.0).sum::<f64>() / n, p.iter().map(|q| q.1).sum::<f64>() / n));
            }
        }
    }
    for (i, e) in elements.iter().enumerate() {
        let ctx = format!("elevated.json elements[{i}]");
        let kd = text(e, "kind", &ctx)?;
        if kd != "rail" {
            if kd != "station" {
                *unmapped_el.entry(kd.to_string()).or_default() += 1;
            }
            continue;
        }
        let id = text(e, "id", &ctx)?;
        let pts = points(e, "pts", &ctx)?;
        if pts.len() < 2 {
            continue;
        }
        // Each station's nearest track point is kept (a waypoint is a stop).
        let keep: BTreeSet<usize> = stations
            .iter()
            .map(|&(sx, sy)| {
                (0..pts.len())
                    .min_by(|&a, &b| {
                        let da = (pts[a].0 - sx).powi(2) + (pts[a].1 - sy).powi(2);
                        let db = (pts[b].0 - sx).powi(2) + (pts[b].1 - sy).powi(2);
                        da.total_cmp(&db)
                    })
                    .unwrap_or(0)
            })
            .collect();
        match line_waypoints(&f, &pts, &keep) {
            Some(wp) => lines.push(TransitLineCfg { name: format!("El {id}"), waypoints: wp }),
            None => rep.warnings.push(format!("{ctx} ({id}): under two track points in the window")),
        }
    }
    for (kd, n) in &unmapped_el {
        rep.unmapped.push(format!("elevated.json: {n} \"{kd}\" element(s) (no sim counterpart)"));
    }
    // The tram (street/tram.json): its loop out to the terminus is a line.
    match read_json(&src.join("street"), "tram.json") {
        Err(e) => rep.warnings.push(format!("{e}; no tram line")),
        Ok(t) => {
            for (i, l) in arr(&t, "lines", "tram.json")?.iter().enumerate() {
                let ctx = format!("tram.json lines[{i}]");
                let way = l.get("way").and_then(|v| v.as_str()).unwrap_or("tram");
                let pts = points(l, "pts", &ctx)?;
                let u: Vec<f64> = arr(l, "u", &ctx)?.iter().map(|v| num(v, &ctx)).collect::<Result<_, _>>()?;
                if u.len() != pts.len() {
                    return Err(format!("{ctx}: {} pts but {} u", pts.len(), u.len()));
                }
                let mut stops: Vec<(f64, String)> = Vec::new();
                for (j, st) in arr(l, "stops", &ctx)?.iter().enumerate() {
                    let a = st
                        .as_array()
                        .filter(|a| a.len() >= 3)
                        .ok_or_else(|| format!("{ctx} stops[{j}]: not [u, dwell, name]"))?;
                    stops.push((num(&a[0], &ctx)?, a[2].as_str().unwrap_or("").to_string()));
                }
                // The far terminus (the loop starts at one): out on one track, back on the other.
                let end_u = stops
                    .iter()
                    .find(|(su, n)| n == "terminus" && *su > 0.0)
                    .map_or(u.last().copied().unwrap_or(0.0), |s| s.0);
                let n = u.iter().position(|&x| x > end_u).unwrap_or(u.len());
                let half = &pts[..n.max(2).min(pts.len())];
                let keep: BTreeSet<usize> = stops
                    .iter()
                    .filter(|(su, _)| *su <= end_u)
                    .map(|(su, _)| u.iter().position(|&x| x >= *su).unwrap_or(0).min(half.len() - 1))
                    .collect();
                match line_waypoints(&f, half, &keep) {
                    Some(wp) => lines.push(TransitLineCfg { name: format!("Tram {way}"), waypoints: wp }),
                    None => rep.warnings.push(format!("{ctx}: under two tram points in the window")),
                }
            }
        }
    }
    rep.lines = lines.iter().map(|l| format!("{} ({} waypoints)", l.name, l.waypoints.len())).collect();

    // --- what the sim does not read ---------------------------------------------
    for (cls, n) in &by_cls {
        if !matches!(cls.as_str(), "street" | "arterial" | "lane" | "alley" | "promenade" | "tram") {
            rep.unmapped.push(format!("roads.json: {n} \"{cls}\" ways read as plain Road (no grade or stairs)"));
        }
    }
    rep.unmapped.push(
        "lots.json: platform z, under_deck, pylons, frontage classes, walkway_join (the sim's ground is flat)".into(),
    );
    rep.unmapped.push("lots.json floors: the district's range, not the lot's massing (a seeded pick in it)".into());
    rep.unmapped.push("terrain: sea and mountain both read as Water (outside every block, way and superblock)".into());
    rep.unmapped.push("roads.json nodes/edges, traffic.json lanes: vehicles are not simulated on roads".into());
    rep.unmapped.push(
        "sim_layout.toml rooms, portals, slots: a Home is 5 residents a floor; flats and shop units are counted, rooms are not placed"
            .into(),
    );

    let zones: Vec<char> =
        dgrid.iter().map(|&d| zone_of_d.get(usize::from(d)).copied().unwrap_or(Z::Mid).letter()).collect();
    let grid: Vec<u8> = dgrid.iter().map(|&d| d as u8).collect();
    let text_out = MapText {
        w,
        h,
        tiles,
        zones,
        buildings: out_b,
        districts,
        district_grid: grid,
        lines,
        comments: vec![
            format!(
                "imported from TheCityGraphics by citysim-cli import-graphics (Scale S3b): downsample {k}, {s} m tiles, crop {}, employers for {} residents",
                args.crop.as_deref().unwrap_or("none"),
                args.residents
            ),
            "docs/IMPORT_GRAPHICS.md: the mapping and what is not mapped".into(),
        ],
    }
    .to_text(true);
    // Self-check: the sim's own parser must accept it.
    let check = text_out.clone();
    let parsed = std::panic::catch_unwind(move || citysim::Map::parse(&check));
    if let Err(e) = parsed {
        let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
        return Err(format!("the imported map does not parse: {}", msg.unwrap_or_default()));
    }
    Ok((text_out, rep))
}

/// `import-graphics`: build, write, report.
pub fn import(args: ImportArgs) -> Result<(), String> {
    let (text, rep) = build(&args)?;
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&args.out, &text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    println!("{}: {} bytes", args.out.display(), text.len());
    rep.print();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("citysim-import-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("tmp");
        d
    }

    fn args(src: &Path) -> ImportArgs {
        ImportArgs { src: src.to_path_buf(), out: src.join("out.txt"), downsample: 1, residents: 1_000, crop: None }
    }

    /// A tiny city in their format: one Sump superblock, one block with two
    /// parts either side of an alley, a street on each long side.
    fn write_city(d: &Path, lots_field: &str) {
        let districts = r#"{"rows":[{"id":"D1","name":"The Sump","zone":"Sump"}],
            "superblocks":[{"district":"D1","rect_m":[0,0,80,60]}]}"#;
        let blocks = r#"{"blocks":[{"id":"b0","district":"D1","rect_m":[0,12,80,48],
            "parts_m":[[0,12,80,28],[0,30,80,48]]}],"open":[],"greens_m":[],"leftover_m":[],"voids_m":[]}"#;
        let mut lots = Vec::new();
        for i in 0..10 {
            let x = f64::from(i) * 8.0;
            lots.push(format!(
                r#"{{"id":"l{i}","block":"b0","front":"S","{lots_field}":[{x},12,{},28],"floors":[4,12],"seed":{i}}}"#,
                x + 8.0
            ));
            lots.push(format!(
                r#"{{"id":"m{i}","block":"b0","front":"N","rect_m":[{x},30,{},48],"floors":[4,12],"seed":{}}}"#,
                x + 8.0,
                i + 10
            ));
        }
        let lots = format!(r#"{{"tile":2.0,"lots":[{}]}}"#, lots.join(","));
        let roads = r#"{"ways":[{"cls":"street","rect_m":[0,0,80,12]},{"cls":"alley","rect_m":[0,28,80,30]},
            {"cls":"street","rect_m":[0,48,80,60]},{"cls":"street","rect_m":[0,0,4,60]}]}"#;
        let elevated = r#"{"elements":[{"id":"rail","kind":"rail","pts":[[2,6,40],[78,6,40]]},
            {"id":"escalator","kind":"escalator","pts":[[2,2,0],[3,3,0]]}]}"#;
        let terrain = r#"{"summary":{"size_m":[80,60]}}"#;
        for (n, t) in [
            ("districts.json", districts),
            ("blocks.json", blocks),
            ("lots.json", lots.as_str()),
            ("roads.json", roads),
            ("elevated.json", elevated),
            ("terrain.json", terrain),
        ] {
            std::fs::write(d.join(n), t).expect("write");
        }
    }

    #[test]
    fn test_import_tiny_city() {
        let d = tmp("tiny");
        write_city(&d, "rect_m");
        let (text, rep) = build(&args(&d)).expect("imports");
        let m = citysim::Map::parse(&text);
        assert_eq!((m.w(), m.h()), (40, 30));
        assert_eq!(m.districts.len(), 1);
        assert_eq!(m.lines.len(), 1);
        assert!(rep.segments >= 4, "{rep:?}");
        assert!(m.count_kind(BuildingKind::Home) + m.count_kind(BuildingKind::Lot) > 0);
        assert!(!m.edge_roads().is_empty());
        assert!(rep.unmapped.iter().any(|u| u.contains("escalator")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn test_import_missing_field_is_an_error_not_a_panic() {
        let d = tmp("missing");
        write_city(&d, "rect");
        let err = build(&args(&d)).expect_err("lots without rect_m");
        assert!(err.contains("lots.json lots[0] (l0): missing field \"rect_m\""), "{err}");
        std::fs::remove_file(d.join("roads.json")).expect("rm");
        let err = build(&args(&d)).expect_err("no roads.json");
        assert!(err.contains("roads.json"), "{err}");
        let err = build(&ImportArgs { src: d.join("nope"), ..args(&d) }).expect_err("no dir");
        assert!(err.contains("not a directory"), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
