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
//!
//! Scale S3c: their sim export (`<src>/sim/`, schema `city-sim/0.1`,
//! SIM_NOTES "The sim export") is preferred where present, file by file:
//! `lots_sim.json` (per-lot floors, use, flats, shop units, street doors,
//! the named civic sites), `terrain_sim.json` (sea, urban land and mountain:
//! the hills become walkable Mountain), `rail.json` (the Terrace Line and its
//! named stations). Without it the S3b inputs stand; the report says which
//! were read.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::Value;

use citysim::components::{BuildingKind, Rect, Zone};
use citysim::config::TransitLineCfg;
use citysim::map::{BuildingExtra, MapDistrict, MapText};

/// The sim export's schema this adapter reads (`<src>/sim/*.json`).
pub const SIM_SCHEMA: &str = "city-sim/0.1";

/// Residents a Home's floor slot (a household) holds: the 2k city's
/// `[world] residents_per_home`, the generator's convention.
const RESIDENTS_PER_SLOT: u32 = citysim::mapgen::RESIDENTS_PER_HOME;

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
    /// The pre-view pass phase 5 (P11): authored El lines in this map's tile
    /// frame, one a row (`x,y x,y [...] name`, `#` comments), written as
    /// `L+` lines (in flight) after the export's.
    #[arg(long)]
    pub lines: Option<PathBuf>,
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
    /// S3c: which inputs were read (the sim export or the S3b fallbacks).
    pub inputs: Vec<String>,
    /// S3c: the residents the employers and beds are sized for.
    pub residents: u32,
    /// S3c: doors placed on their `doors_m` `out` tile, on the nearest
    /// road-facing wall tile to it, or by the S3b side search.
    pub doors_exact: usize,
    pub doors_near: usize,
    pub doors_searched: usize,
    /// Doors moved off an outside tile a later cut built over.
    pub doors_moved: usize,
    /// S3c: flats in the Homes (their estimate) and Home floor slots capped at 255.
    pub home_flats: u64,
    pub slots_capped: usize,
    /// S3c: sim tiles read as Mountain and as sea from `terrain_sim.json`
    /// (before blocks and roads are laid over them).
    pub mountain_tiles: usize,
    pub sea_tiles: usize,
    /// S3c: buildings by kind and their cut's `use` (`-` without the export).
    pub kind_use: BTreeMap<(BuildingKind, String), usize>,
}

/// The generator's per-1,000 mix (docs/SCALE_20K.md § S3b), for the report.
const GEN_MIX: [(BuildingKind, f64); 7] = [
    (BuildingKind::Home, 200.0),
    (BuildingKind::Farm, 6.0),
    (BuildingKind::Market, 1.5),
    (BuildingKind::Bar, 1.5),
    (BuildingKind::Hideout, 1.0),
    (BuildingKind::SecurityOffice, 1.0),
    (BuildingKind::Lot, 80.0),
];

/// Track B (OPEN-136): the generator's 80 Lots per 1,000 by zone
/// (`mapgen`'s borough: the Spire's 10 + 10, the Mid's and the Civic row's 66,
/// the Sump's 16 + 8, the Vats' 50 per 2,000; `gen:20000` measures Spire
/// 200, Mid 638 + Civic 22, Sump 240, Vats 500), dealt zone by zone in this
/// order before the rest of the Lots go anywhere.
const GEN_LOTS_BY_ZONE: &[(&[Zone], f64)] =
    &[(&[Zone::Spire], 10.0), (&[Zone::Mid, Zone::Civic], 33.0), (&[Zone::Sump], 12.0), (&[Zone::Vats], 25.0)];

/// `n` dealt over the keys by their weights (largest remainder, ties the
/// lower key); an even deal when every weight is 0. Keys with no share are
/// left out.
fn largest_remainder(weights: &BTreeMap<usize, u64>, n: usize) -> Vec<(usize, usize)> {
    let total: u64 = weights.values().sum();
    let len = weights.len() as u64;
    if len == 0 {
        return Vec::new();
    }
    let n64 = n as u64;
    let mut out: Vec<(usize, usize, u64)> = weights
        .iter()
        .enumerate()
        .map(|(j, (&k, &w))| match ((n64 * w).checked_div(total), (n64 * w).checked_rem(total)) {
            (Some(q), Some(r)) => (k, q as usize, r),
            _ => (k, (n64 / len + u64::from((j as u64) < n64 % len)) as usize, 0),
        })
        .collect();
    let dealt: usize = out.iter().map(|o| o.1).sum();
    let mut order: Vec<usize> = (0..out.len()).collect();
    order.sort_by_key(|&j| (std::cmp::Reverse(out[j].2), out[j].0));
    for &j in order.iter().take(n.saturating_sub(dealt)) {
        out[j].1 += 1;
    }
    out.into_iter().filter(|o| o.1 > 0).map(|o| (o.0, o.1)).collect()
}

/// Make `n` of the free cuts passing `ok` a `want`, the fewest flats first
/// (ties by position); returns how many it made.
fn fewest_flats(
    kind: &mut [Option<BuildingKind>],
    segs: &[Seg],
    want: BuildingKind,
    n: usize,
    ok: &dyn Fn(usize) -> bool,
) -> usize {
    let mut c: Vec<usize> = (0..segs.len()).filter(|&i| kind[i].is_none() && ok(i)).collect();
    c.sort_by_key(|&i| (segs[i].flats, segs[i].rect.y, segs[i].rect.x));
    let m = n.min(c.len());
    for &i in &c[..m] {
        kind[i] = Some(want);
    }
    m
}

impl Report {
    /// Kind counts per 1,000 residents beside the generator's, one line each.
    pub fn mix_lines(&self) -> Vec<String> {
        let per_k = f64::from(self.residents.max(1)) / 1000.0;
        GEN_MIX
            .iter()
            .map(|&(k, gen)| {
                let n = self.kinds.get(&k).copied().unwrap_or(0);
                format!("{:<15} {n:>6}  {:>7.2} per 1,000 (generator {gen})", format!("{k:?}"), n as f64 / per_k)
            })
            .collect()
    }

    pub fn print(&self) {
        for i in &self.inputs {
            println!("input: {i}");
        }
        println!(
            "map {} x {} tiles ({} m a tile); {} lots read ({} with a sim_layout), {} block parts",
            self.w, self.h, self.tile_m, self.lots, self.lots_with_layout, self.parts
        );
        println!(
            "buildings: {} segments kept; dropped {} too small, {} with no road at any side, {} overlapping ({} lots in dropped parts or segments)",
            self.segments, self.dropped_small, self.dropped_no_road, self.dropped_overlap, self.lots_dropped
        );
        println!(
            "flats read {}, shop units {}; Homes hold {} flats; Home floor slots {} ({} residents at {RESIDENTS_PER_SLOT} a slot; {} capped at 255)",
            self.flats,
            self.shop_units,
            self.home_flats,
            self.home_floors,
            self.home_floors * RESIDENTS_PER_SLOT as usize,
            self.slots_capped
        );
        println!(
            "doors: {} on their out tile, {} on the nearest road-facing wall tile, {} by the side search; {} moved off a built-over alley",
            self.doors_exact, self.doors_near, self.doors_searched, self.doors_moved
        );
        println!(
            "terrain: {} Mountain tiles, {} sea tiles before the blocks and roads",
            self.mountain_tiles, self.sea_tiles
        );
        for (k, n) in &self.kinds {
            println!("  {:<15} {n:>6}", format!("{k:?}"));
        }
        if self.kind_use.keys().any(|(_, u)| u != "-") {
            println!("kinds by their cut's use:");
            for ((k, u), n) in &self.kind_use {
                println!("  {:<15} {u:<12} {n:>6}", format!("{k:?}"));
            }
        }
        println!("mix for {} residents:", self.residents);
        for l in self.mix_lines() {
            println!("  {l}");
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

/// A whole number field.
fn uint(v: &Value, key: &str, ctx: &str) -> Result<u64, String> {
    get(v, key, ctx)?.as_u64().ok_or_else(|| format!("{ctx}: \"{key}\" is not a whole number"))
}

/// A `[x, y, ...]` point field.
fn point(v: &Value, key: &str, ctx: &str) -> Result<(f64, f64), String> {
    let c = format!("{ctx} \"{key}\"");
    let a = get(v, key, ctx)?.as_array().filter(|a| a.len() >= 2).ok_or_else(|| format!("{c}: not a point"))?;
    Ok((num(&a[0], &c)?, num(&a[1], &c)?))
}

/// Warn when a sim export file's `schema` is not the one this adapter reads.
fn check_schema(v: &Value, file: &str, rep: &mut Report) -> Result<(), String> {
    let s = text(v, "schema", file)?;
    if s != SIM_SCHEMA {
        rep.warnings.push(format!("{file}: schema {s:?}, this adapter reads {SIM_SCHEMA:?}; reading it anyway"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The sim export (`<src>/sim/`, S3c).
// ---------------------------------------------------------------------------

/// A lot's `use` in `lots_sim.json`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Use {
    ShopHouse,
    Commercial,
    Residential,
    Factory,
    VatFarm,
    Godown,
    Civic,
    Vacant,
}

impl Use {
    fn parse(s: &str) -> Option<Use> {
        Some(match s {
            "shop_house" => Use::ShopHouse,
            "commercial" => Use::Commercial,
            "residential" => Use::Residential,
            "factory" => Use::Factory,
            "vat_farm" => Use::VatFarm,
            "godown" => Use::Godown,
            "civic" => Use::Civic,
            "vacant" => Use::Vacant,
            _ => return None,
        })
    }

    fn industrial(self) -> bool {
        matches!(self, Use::Factory | Use::VatFarm | Use::Godown)
    }
}

/// One lot's record in `sim/lots_sim.json`.
#[derive(Clone, Debug)]
struct SimLot {
    floors: u32,
    use_: Use,
    /// `ground == "workshop"` (an alley-only lot's ground floor).
    workshop: bool,
    flats: u32,
    shops: usize,
    /// Each door's `out` point: the centre of the road tile outside it.
    doors_out: Vec<(f64, f64)>,
    layout: bool,
    civic: Option<String>,
}

/// `sim/lots_sim.json`'s lots by id.
fn read_sim_lots(dir: &Path, rep: &mut Report) -> Result<BTreeMap<String, SimLot>, String> {
    const F: &str = "sim/lots_sim.json";
    let v = read_json(dir, "lots_sim.json")?;
    check_schema(&v, F, rep)?;
    let lots = get(&v, "lots", F)?.as_object().ok_or_else(|| format!("{F}: \"lots\" is not an object"))?;
    let mut out = BTreeMap::new();
    let mut odd: BTreeMap<String, usize> = BTreeMap::new();
    for (id, r) in lots {
        let ctx = format!("{F} lots.{id}");
        let u = text(r, "use", &ctx)?;
        let use_ = Use::parse(u).unwrap_or_else(|| {
            *odd.entry(format!("use {u:?} (read as vacant)")).or_default() += 1;
            Use::Vacant
        });
        let ground = text(r, "ground", &ctx)?;
        if !matches!(ground, "shop" | "workshop" | "lobby" | "works" | "civic" | "none") {
            *odd.entry(format!("ground {ground:?}")).or_default() += 1;
        }
        let source = text(r, "source", &ctx)?;
        if !matches!(source, "layout" | "estimate") {
            *odd.entry(format!("source {source:?}")).or_default() += 1;
        }
        let mut doors_out = Vec::new();
        for (j, d) in arr(r, "doors_m", &ctx)?.iter().enumerate() {
            let c = format!("{ctx} doors_m[{j}]");
            point(d, "at", &c)?;
            doors_out.push(point(d, "out", &c)?);
        }
        let civic = match r.get("civic") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => return Err(format!("{ctx}: \"civic\" is not a string")),
        };
        let rec = SimLot {
            floors: u32::try_from(uint(r, "floors", &ctx)?).unwrap_or(u32::MAX),
            use_,
            workshop: ground == "workshop",
            flats: u32::try_from(uint(r, "flats", &ctx)?).unwrap_or(u32::MAX),
            shops: usize::try_from(uint(r, "shop_units", &ctx)?).unwrap_or(usize::MAX),
            doors_out,
            layout: source == "layout",
            civic,
        };
        out.insert(id.clone(), rec);
    }
    for (what, n) in odd {
        rep.warnings.push(format!("{F}: {n} lot(s) with an unknown {what}"));
    }
    Ok(out)
}

/// `sim/terrain_sim.json`: a class per their tile, north-up (0 sea, 1
/// urban, 2 mountain).
struct Terrain {
    tile_m: f64,
    nx: usize,
    ny: usize,
    cls: Vec<u8>,
}

const SEA: u8 = 0;
const URBAN: u8 = 1;
const MOUNTAIN: u8 = 2;

impl Terrain {
    fn read(dir: &Path, rep: &mut Report) -> Result<Terrain, String> {
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
    fn at(&self, x: f64, y: f64) -> u8 {
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

/// A rect's wall tiles a door can take (not the corners), each with the
/// tile outside it: `(door, outside)`; the S, N, E, W walls in turn.
fn wall_doors(r: (usize, usize, usize, usize), w: usize, h: usize) -> Vec<((usize, usize), (usize, usize))> {
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
    /// S3c: the lot's `lots_sim.json` record (`None` without the export).
    sim: Option<SimLot>,
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
    /// S3c, from the export (`None` / 0 without it): the members' use (a
    /// named civic member's, else the most common), whether most members'
    /// ground floor is a workshop, their flats and the named civic site.
    use_: Option<Use>,
    workshop: bool,
    flats: u32,
    civic: Option<String>,
}

/// A named civic site's rank when two share a cut (the first is kept).
fn civic_rank(name: &str) -> usize {
    match name {
        "city_hall" => 0,
        n if n.starts_with("precinct") => 1,
        "depot" => 2,
        "crematorium" => 3,
        _ => 4,
    }
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
    let mut rep = Report { residents: args.residents, ..Report::default() };

    // S3c: the sim export, file by file; the S3b inputs where one is absent.
    let sim_dir = src.join("sim");
    let sim_lots = if sim_dir.join("lots_sim.json").is_file() {
        let l = read_sim_lots(&sim_dir, &mut rep)?;
        rep.inputs.push(format!(
            "lots: sim/lots_sim.json ({} records, {} from layouts): floors, use, flats, shop units, doors, civic sites",
            l.len(),
            l.values().filter(|r| r.layout).count()
        ));
        Some(l)
    } else {
        rep.inputs.push(
            "lots: lots.json floor ranges and bldg/<lot>/sim_layout.toml (no sim/lots_sim.json: the S3b fallback)"
                .into(),
        );
        None
    };
    let terrain = if sim_dir.join("terrain_sim.json").is_file() {
        let t = Terrain::read(&sim_dir, &mut rep)?;
        rep.inputs.push(format!(
            "terrain: sim/terrain_sim.json ({} x {} at {} m): sea Water, mountain Mountain",
            t.nx, t.ny, t.tile_m
        ));
        Some(t)
    } else {
        rep.inputs.push(
            "terrain: none (no sim/terrain_sim.json: every tile off the blocks and ways is Water, the S3b fallback)"
                .into(),
        );
        None
    };
    let rail_json = if sim_dir.join("rail.json").is_file() {
        let r = read_json(&sim_dir, "rail.json")?;
        check_schema(&r, "sim/rail.json", &mut rep)?;
        rep.inputs
            .push("rail: sim/rail.json (lines and named stations; the timetable stays the sim's [transit])".into());
        Some(r)
    } else {
        rep.inputs.push("rail: elevated.json rail elements (no sim/rail.json: the S3b fallback)".into());
        None
    };

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
    // S3c: the terrain first (each sim tile the most common class of its
    // samples, one per their tile; ties urban, then mountain), then the
    // blocks and ways over it.
    if let Some(t) = &terrain {
        let sub = ((s / t.tile_m).round() as usize).max(1);
        for ty in 0..h {
            for tx in 0..w {
                let mut n = [0usize; 3];
                for j in 0..sub {
                    for i in 0..sub {
                        let x = f.x0 + (tx as f64 + (i as f64 + 0.5) / sub as f64) * s;
                        let y = f.y1 - (ty as f64 + (j as f64 + 0.5) / sub as f64) * s;
                        n[usize::from(t.at(x, y))] += 1;
                    }
                }
                let c = [URBAN, MOUNTAIN, SEA].into_iter().fold(URBAN, |b, c| {
                    if n[usize::from(c)] > n[usize::from(b)] {
                        c
                    } else {
                        b
                    }
                });
                tiles[at(tx, ty)] = match c {
                    URBAN => '.',
                    MOUNTAIN => 'm',
                    _ => '~',
                };
            }
        }
        rep.mountain_tiles = tiles.iter().filter(|&&c| c == 'm').count();
        rep.sea_tiles = tiles.iter().filter(|&&c| c == '~').count();
    }
    // S3c: with the terrain, only the blocks are laid over its Mountain (a
    // superblock's or a green's rect is a bounding box, often up the hill).
    let land = |r: [f64; 4], tiles: &mut Vec<char>, over_hills: bool| {
        let (x0, y0, x1, y1) = f.round(r);
        for y in y0..y1 {
            for x in x0..x1 {
                if over_hills || tiles[at(x, y)] != 'm' {
                    tiles[at(x, y)] = '.';
                }
            }
        }
    };
    for (i, sb) in arr(&districts_json, "superblocks", "districts.json")?.iter().enumerate() {
        land(field_rect(sb, "rect_m", &format!("districts.json superblocks[{i}]"))?, &mut tiles, false);
    }
    for list in ["blocks", "open", "greens_m", "leftover_m", "voids_m"] {
        let Some(items) = blocks_json.get(list).and_then(|v| v.as_array()) else {
            rep.warnings.push(format!("blocks.json: no \"{list}\" list"));
            continue;
        };
        for (i, it) in items.iter().enumerate() {
            let ctx = format!("blocks.json {list}[{i}]");
            let r = if it.is_array() { rect_m(it, &ctx)? } else { field_rect(it, "rect_m", &ctx)? };
            land(r, &mut tiles, list == "blocks");
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
        // S3c: the export's record replaces the floor range and the layout read.
        if let Some(sim) = &sim_lots {
            let rec = sim
                .get(id)
                .ok_or_else(|| format!("sim/lots_sim.json lots: no record for lot {id} (lots.json lots[{i}])"))?;
            if rec.layout {
                rep.lots_with_layout += 1;
            }
            rep.flats += rec.flats as usize;
            rep.shop_units += rec.shops;
            let lot = Lot {
                rect,
                front,
                floors: rec.floors.max(1),
                flats: rec.flats as usize,
                shops: rec.shops,
                layout: rec.layout,
                residential: rec.use_ == Use::Residential,
                door_off: None,
                sim: Some(rec.clone()),
            };
            lots_by_block.entry(block).or_default().push(lot);
            continue;
        }
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
            sim: None,
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
    if let Some(sim) = &sim_lots {
        if sim.len() != lots_list.len() {
            rep.warnings.push(format!(
                "sim/lots_sim.json holds {} records, lots.json {} lots (records past lots.json are ignored)",
                sim.len(),
                lots_list.len()
            ));
        }
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
            // The Vats' godowns stay whole (a Vat Farm needs 60 interior
            // tiles). S3c: with the export, a Vats part is cut between two
            // lots each big enough for a Farm on its own (at 2 m tiles most
            // vat farms are; at 8 m none, and the parts stay whole).
            if vats_block {
                let depth = if along_x { py1 - py0 } else { px1 - px0 };
                let span = |l: &Lot| {
                    let (x0, y0, x1, y1) = f.round(l.rect);
                    if along_x {
                        (x0, x1)
                    } else {
                        (y0, y1)
                    }
                };
                let farm_sized = |l: &Lot| {
                    let (a, b) = span(l);
                    (b.saturating_sub(a)).saturating_sub(2) * depth.saturating_sub(2) >= 60
                };
                edges.retain(|&e| {
                    let before: Vec<&&Lot> = lots.iter().filter(|l| span(l).1 == e).collect();
                    let after: Vec<&&Lot> = lots.iter().filter(|l| span(l).0 == e).collect();
                    !before.is_empty()
                        && !after.is_empty()
                        && before.iter().chain(&after).all(|l| l.sim.is_some() && farm_sized(l))
                });
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
                // S3c: the members' (else the part's) lots, for the export's fields.
                let src_lots: Vec<&Lot> =
                    if members.is_empty() { lots.clone() } else { members.iter().map(|l| **l).collect() };
                // S3c: their street doors first: a wall tile (not a corner)
                // whose outside is the `out` tile, else the road-facing wall
                // tile nearest any `out` tile.
                let outs: Vec<(usize, usize)> = src_lots
                    .iter()
                    .filter_map(|l| l.sim.as_ref())
                    .flat_map(|r| r.doors_out.iter())
                    .filter_map(|&(x, y)| f.tile(x, y))
                    .collect();
                let mut door = None;
                if !outs.is_empty() {
                    let mut cands = wall_doors((x0, y0, x1, y1), w, h);
                    cands.retain(|&(_, (ox, oy))| road(&tiles, ox, oy));
                    let dist = |c: &((usize, usize), (usize, usize))| {
                        outs.iter().map(|o| c.1 .0.abs_diff(o.0) + c.1 .1.abs_diff(o.1)).min().unwrap_or(usize::MAX)
                    };
                    if let Some(c) = outs.iter().find_map(|o| cands.iter().find(|c| c.1 == *o)) {
                        door = Some(c.0);
                        rep.doors_exact += 1;
                    } else if let Some(c) = cands.iter().min_by_key(|c| dist(c)) {
                        door = Some(c.0);
                        rep.doors_near += 1;
                    }
                }
                let searched = door.is_none() && sim_lots.is_some();
                for side in std::iter::once(front).chain(['S', 'N', 'E', 'W'].into_iter().filter(|&c| c != front)) {
                    if door.is_some() {
                        break;
                    }
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
                if searched {
                    rep.doors_searched += 1;
                }
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
                // S3c: the export's use, ground, flats and civic name.
                let recs: Vec<&SimLot> = src_lots.iter().filter_map(|l| l.sim.as_ref()).collect();
                let civic = members
                    .iter()
                    .filter_map(|l| l.sim.as_ref().and_then(|r| r.civic.clone()))
                    .min_by_key(|c| (civic_rank(c), c.clone()));
                let use_ = if recs.is_empty() {
                    None
                } else if civic.is_some() {
                    Some(Use::Civic)
                } else {
                    let mut n: BTreeMap<Use, usize> = BTreeMap::new();
                    for r in recs.iter().filter(|r| r.civic.is_none()) {
                        *n.entry(r.use_).or_default() += 1;
                    }
                    n.iter()
                        .max_by_key(|(u, c)| (**c, std::cmp::Reverse(**u)))
                        .map_or(Some(Use::Civic), |(u, _)| Some(*u))
                };
                segs.push(Seg {
                    rect,
                    door,
                    district: usize::from(dgrid[at(cx, cy)]),
                    floors: floors.clamp(1, 255),
                    shops: members.iter().map(|l| l.shops).sum(),
                    residential: members.iter().all(|l| l.residential),
                    use_,
                    workshop: !recs.is_empty() && recs.iter().filter(|r| r.workshop).count() * 2 >= recs.len(),
                    flats: members.iter().map(|l| l.flats as u32).sum(),
                    civic,
                });
            }
        }
    }
    // A door's outside tile can be a rear alley a whole-block cut built
    // over later: such a door moves to the nearest wall tile still facing an
    // open road (else the cut is dropped).
    let mut keep_seg = vec![true; segs.len()];
    for (j, sg) in segs.iter_mut().enumerate() {
        let r = sg.rect;
        let (x0, y0) = (usize::from(r.x), usize::from(r.y));
        let cands: Vec<((usize, usize), (usize, usize))> =
            wall_doors((x0, y0, x0 + usize::from(r.w), y0 + usize::from(r.h)), w, h)
                .into_iter()
                .filter(|&(_, (ox, oy))| road(&tiles, ox, oy) && !occupied[at(ox, oy)])
                .collect();
        if cands.iter().any(|c| c.0 == sg.door) {
            continue;
        }
        let d = sg.door;
        match cands.iter().min_by_key(|c| c.0 .0.abs_diff(d.0) + c.0 .1.abs_diff(d.1)) {
            Some(c) => {
                sg.door = c.0;
                rep.doors_moved += 1;
            }
            None => {
                keep_seg[j] = false;
                rep.dropped_no_road += 1;
            }
        }
    }
    let mut j = 0;
    segs.retain(|_| {
        j += 1;
        keep_seg[j - 1]
    });
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
    // Take `n` of the free candidates, stage by stage (each a filter; S3c's
    // use-led picks), within a stage the preferred zones first, spread by
    // stride over the city (sorted by position) or the largest first.
    let pick = |kind: &mut Vec<Option<BuildingKind>>,
                rep: &mut Report,
                want: BuildingKind,
                n: usize,
                prefer: &[Zone],
                stages: &[&dyn Fn(usize) -> bool],
                largest: bool| {
        let mut got = 0;
        for ok in stages {
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
        }
        if got < n {
            rep.shortfall.push(format!("{want:?}: {got} of {n}"));
        }
    };
    use BuildingKind as K;
    use Zone as Z;
    let any = |_: usize| true;
    let farm_ok = |i: usize| interior(i) >= 60;
    let by_use = segs.iter().any(|sg| sg.use_.is_some());
    if !by_use {
        // S3b: no export, the generator's mix by district.
        pick(&mut kind, &mut rep, K::Jail, 1, &[Z::Civic], &[&any], true);
        pick(&mut kind, &mut rep, K::Hall, 1, &[Z::Civic], &[&any], true);
        pick(&mut kind, &mut rep, K::Warehouse, 1, &[Z::Vats], &[&any], true);
        pick(&mut kind, &mut rep, K::Cemetery, 1, &[Z::Vats], &[&any], false);
        pick(&mut kind, &mut rep, K::Farm, target(6.0), &[Z::Vats], &[&farm_ok], true);
        pick(&mut kind, &mut rep, K::SecurityOffice, target(1.0), &[Z::Vats], &[&any], false);
        pick(&mut kind, &mut rep, K::Hideout, target(1.0), &[Z::Sump], &[&any], false);
        // Shop cuts first; too few shop units: the rest on any block.
        let shop_ok = |i: usize| segs[i].shops > 0 || !segs[i].residential;
        for (kd, per) in [(K::Market, 1.5), (K::Bar, 1.5)] {
            pick(&mut kind, &mut rep, kd, target(per), &[Z::Mid, Z::Sump], &[&shop_ok, &any], false);
        }
        pick(&mut kind, &mut rep, K::Lot, target(80.0), &[], &[&any], false);
    } else {
        // S3c: the export's use leads; the counts stay the generator's mix.
        let segs = &segs;
        let is = |u: Use| move |i: usize| segs[i].use_ == Some(u);
        let (vat, factory, godown) = (is(Use::VatFarm), is(Use::Factory), is(Use::Godown));
        let (shop_house, commercial) = (is(Use::ShopHouse), is(Use::Commercial));
        let industrial = |i: usize| segs[i].use_.is_some_and(Use::industrial);
        // The named civic sites: city_hall the Hall, depot the Reserve Depot,
        // crematorium the Recycler; the precinct in a Civic district (else
        // the first) the Precinct (the Jail), every other precinct a
        // Security Office.
        let mut precincts: Vec<usize> = Vec::new();
        for (i, sg) in segs.iter().enumerate() {
            let Some(c) = sg.civic.as_deref() else { continue };
            let kd = match c {
                "city_hall" => K::Hall,
                "depot" => K::Warehouse,
                "crematorium" => K::Cemetery,
                p if p.starts_with("precinct") => {
                    precincts.push(i);
                    continue;
                }
                other => {
                    rep.warnings.push(format!("civic site {other:?}: no sim kind; read as a Lot"));
                    continue;
                }
            };
            if kind.contains(&Some(kd)) {
                rep.warnings.push(format!("civic site {c:?}: a second {kd:?}; read as a Lot"));
            } else {
                kind[i] = Some(kd);
            }
        }
        precincts.sort_by_key(|&i| (zone(i) != Z::Civic, segs[i].district, pos(i)));
        for (j, &i) in precincts.iter().enumerate() {
            kind[i] = Some(if j == 0 { K::Jail } else { K::SecurityOffice });
        }
        // A site the window or the cuts lost: the S3b rule, with a warning.
        for (kd, prefer, largest) in [
            (K::Jail, Z::Civic, true),
            (K::Hall, Z::Civic, true),
            (K::Warehouse, Z::Vats, true),
            (K::Cemetery, Z::Vats, false),
        ] {
            if !kind.contains(&Some(kd)) {
                rep.warnings.push(format!("no named civic site became the {kd:?}: placed by the S3b rule"));
                pick(&mut kind, &mut rep, kd, 1, &[prefer], &[&any], largest);
            }
        }
        let farm_vat = |i: usize| vat(i) && farm_ok(i);
        let farm_ind = |i: usize| industrial(i) && farm_ok(i);
        // Then a cut nobody lives in (commercial, civic), before any Block.
        let farm_open =
            |i: usize| farm_ok(i) && !segs[i].use_.is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse));
        pick(
            &mut kind,
            &mut rep,
            K::Farm,
            target(6.0),
            &[Z::Vats],
            &[&farm_vat, &farm_ind, &farm_open, &farm_ok],
            true,
        );
        let have_so = kind.iter().filter(|x| **x == Some(K::SecurityOffice)).count();
        let n_so = target(1.0).saturating_sub(have_so);
        if n_so > 0 {
            pick(
                &mut kind,
                &mut rep,
                K::SecurityOffice,
                n_so,
                &[Z::Vats],
                &[&factory, &godown, &industrial, &any],
                false,
            );
        }
        let alley = |i: usize| shop_house(i) && segs[i].workshop;
        let shopfront = |i: usize| shop_house(i) && !segs[i].workshop;
        pick(&mut kind, &mut rep, K::Hideout, target(1.0), &[Z::Sump], &[&alley, &shop_house, &any], false);
        // Track B (OPEN-136): the Markets go where the households live, as the
        // generator puts a borough's food beside its homes: one a district
        // first, the rest by the district's household slots (largest
        // remainder; a cut's slots are its flats' share of the households, at
        // least one, as dealt below); within a district shop fronts,
        // commercial cuts, shop-houses, then the cuts with the fewest flats.
        // What a district cannot place goes by the S3c rule.
        let n_markets = target(1.5);
        let lives = |i: usize| {
            kind[i].is_none()
                && segs[i].use_.is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse | Use::Commercial))
        };
        let all_flats: u64 = (0..segs.len()).filter(|&i| lives(i)).map(|i| u64::from(segs[i].flats)).sum();
        let want_hh = u64::from(args.residents.div_ceil(RESIDENTS_PER_SLOT));
        let mut slots_of_d: BTreeMap<usize, u64> = BTreeMap::new();
        for i in (0..segs.len()).filter(|&i| lives(i)) {
            let slots = (want_hh * u64::from(segs[i].flats)).checked_div(all_flats).unwrap_or(0).max(1);
            *slots_of_d.entry(segs[i].district).or_default() += slots;
        }
        // One a district first (when there are enough), the rest by share.
        let one_each = n_markets >= slots_of_d.len();
        let rest_n = if one_each { n_markets - slots_of_d.len() } else { n_markets };
        let mut deal: BTreeMap<usize, usize> = slots_of_d.keys().map(|&d| (d, usize::from(one_each))).collect();
        for (d, n) in largest_remainder(&slots_of_d, rest_n) {
            *deal.entry(d).or_default() += n;
        }
        let mark = rep.shortfall.len();
        for (d, n) in deal.into_iter().filter(|&(_, n)| n > 0) {
            let in_d = |i: usize| segs[i].district == d;
            let (sf, cm, sh) = (
                |i: usize| in_d(i) && shopfront(i),
                |i: usize| in_d(i) && commercial(i),
                |i: usize| in_d(i) && shop_house(i),
            );
            pick(&mut kind, &mut rep, K::Market, n, &[], &[&sf, &cm, &sh], false);
            let have = (0..segs.len()).filter(|&i| in_d(i) && kind[i] == Some(K::Market)).count();
            fewest_flats(&mut kind, segs, K::Market, n.saturating_sub(have), &|i| in_d(i) && segs[i].flats > 0);
        }
        rep.shortfall.truncate(mark);
        let have_markets = kind.iter().filter(|x| **x == Some(K::Market)).count();
        pick(
            &mut kind,
            &mut rep,
            K::Market,
            n_markets.saturating_sub(have_markets),
            &[Z::Sump, Z::Mid],
            &[&shopfront, &commercial, &any],
            false,
        );
        pick(&mut kind, &mut rep, K::Bar, target(1.5), &[Z::Mid, Z::Sump], &[&commercial, &shopfront, &any], false);
        // Lots: the leftover industrial, civic and vacant cuts, then (to the
        // mix's count) commercial cuts without flats, commercial, shop-houses.
        for (i, sg) in segs.iter().enumerate() {
            if kind[i].is_none() && sg.use_.is_some_and(|u| u.industrial() || matches!(u, Use::Civic | Use::Vacant)) {
                kind[i] = Some(K::Lot);
            }
        }
        // Never past the generator's Lots to Blocks (80 to 200): a coarse
        // import has too few cuts for both, and the Blocks come first.
        let have_lots = kind.iter().filter(|x| **x == Some(K::Lot)).count();
        let housing = (0..segs.len())
            .filter(|&i| {
                kind[i].is_none()
                    && segs[i].use_.is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse | Use::Commercial))
            })
            .count();
        let room = (housing as f64 * 0.4 - have_lots as f64) / 1.4;
        let want = target(80.0).saturating_sub(have_lots);
        let n_lots = want.min(room.max(0.0).floor() as usize);
        if n_lots < want {
            rep.shortfall.push(format!(
                "Lot: {} of {} (more would take Blocks past the generator's 80 Lots to 200 Blocks)",
                have_lots + n_lots,
                have_lots + want
            ));
        }
        let no_flats = |i: usize| commercial(i) && segs[i].flats == 0;
        // Track B (OPEN-136): first each zone's share of the generator's Lots
        // (`GEN_LOTS_BY_ZONE`, less the leftovers already there), so the
        // Spire has the Lots its Hqs and Lounges need (`founding::tier_ok`)
        // and the Mid its Gunsmiths and Schools: in the zone commercial cuts
        // without flats, commercial, shop-houses, then the cuts with the
        // fewest flats (the households follow the flats, so a small tower
        // gives up its slot, not its people). The rest as before, anywhere.
        let mut budget = n_lots;
        let mark = rep.shortfall.len();
        for &(zs, per) in GEN_LOTS_BY_ZONE {
            let in_z = |i: usize| zs.contains(&zone(i));
            let have = (0..segs.len()).filter(|&i| in_z(i) && kind[i] == Some(K::Lot)).count();
            // No per-zone 80-to-200 room: capped so, the k = 2 Spire held 88
            // Lots, the seeded industries spilled onto them from the short
            // Vats and `grow_hq` never fired (20 Hqs at d29 against the
            // generator city's 49; docs/IMPORT_GRAPHICS.md, Track B).
            let n = target(per).saturating_sub(have).min(budget);
            let (nf, cm, sh) = (
                |i: usize| in_z(i) && no_flats(i),
                |i: usize| in_z(i) && commercial(i),
                |i: usize| in_z(i) && shop_house(i),
            );
            let before = kind.iter().filter(|x| **x == Some(K::Lot)).count();
            pick(&mut kind, &mut rep, K::Lot, n, &[], &[&nf, &cm, &sh], false);
            let got = kind.iter().filter(|x| **x == Some(K::Lot)).count() - before;
            let homes = |i: usize| {
                in_z(i)
                    && segs[i].use_.is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse | Use::Commercial))
            };
            let more = fewest_flats(&mut kind, segs, K::Lot, n - got, &homes);
            budget -= got + more;
        }
        rep.shortfall.truncate(mark);
        if budget > 0 {
            pick(&mut kind, &mut rep, K::Lot, budget, &[], &[&no_flats, &commercial, &shop_house], false);
        }
    }
    // The rest: Blocks where people live, open Lots elsewhere (S3b: in the
    // Vats and Civic; S3c: commercial cuts without flats).
    let rest = |i: usize| match segs[i].use_ {
        None if matches!(zone(i), Z::Vats | Z::Civic) => K::Lot,
        None => K::Home,
        Some(Use::Residential | Use::ShopHouse) => K::Home,
        Some(Use::Commercial) if segs[i].flats > 0 => K::Home,
        Some(_) => K::Lot,
    };
    let kinds: Vec<BuildingKind> = (0..segs.len()).map(|i| kind[i].unwrap_or_else(|| rest(i))).collect();
    // Home floor slots (a household each, `RESIDENTS_PER_SLOT` residents).
    // S3b: a Home's floors. S3c: the `--residents` households (one a slot)
    // dealt over the Homes by their flats (largest remainder), at least one
    // slot a Home; World::new deals households by stride over the slots, so
    // a tower holds its flats' share and the beds come to at least N.
    let homes: Vec<usize> = (0..segs.len()).filter(|&i| kinds[i] == K::Home).collect();
    let mut slots: Vec<u32> = segs.iter().map(|sg| sg.floors).collect();
    if by_use && !homes.is_empty() {
        let want = u64::from(args.residents.div_ceil(RESIDENTS_PER_SLOT));
        let total: u64 = homes.iter().map(|&i| u64::from(segs[i].flats)).sum();
        rep.home_flats = total;
        let n = homes.len() as u64;
        let mut rems: Vec<(u64, usize)> = Vec::with_capacity(homes.len());
        let mut dealt = 0u64;
        for (j, &i) in homes.iter().enumerate() {
            let p = want * u64::from(segs[i].flats);
            // No flats anywhere: an even deal.
            let (q, r) = match (p.checked_div(total), p.checked_rem(total)) {
                (Some(q), Some(r)) => (q, r),
                _ => (want / n + u64::from((j as u64) < want % n), 0),
            };
            slots[i] = u32::try_from(q).unwrap_or(u32::MAX - 1);
            dealt += q;
            rems.push((r, i));
        }
        rems.sort_by_key(|&(r, i)| (std::cmp::Reverse(r), i));
        for &(_, i) in rems.iter().take(want.saturating_sub(dealt) as usize) {
            slots[i] += 1;
        }
        for &i in &homes {
            slots[i] = slots[i].max(1);
        }
        rep.slots_capped = homes.iter().filter(|&&i| slots[i] > 255).count();
    }

    let mut out_b: Vec<(BuildingKind, Rect, u8, u8, BuildingExtra)> = Vec::new();
    let mut doors: Vec<(usize, usize)> = Vec::new();
    for (i, sg) in segs.iter().enumerate() {
        let z = zone(i);
        let kd = kinds[i];
        let floors = if kd == K::Home { slots[i].clamp(1, 255) as u8 } else { 1 };
        if kd == K::Home {
            rep.home_floors += usize::from(floors);
        }
        let extra = if by_use {
            BuildingExtra {
                flats: (kd == K::Home).then_some(sg.flats),
                storeys: Some(u16::try_from(sg.floors).unwrap_or(u16::MAX)),
            }
        } else {
            BuildingExtra::default()
        };
        *rep.kinds.entry(kd).or_default() += 1;
        let u = sg.use_.map_or("-".to_string(), |u| format!("{u:?}"));
        *rep.kind_use.entry((kd, u)).or_default() += 1;
        out_b.push((kd, sg.rect, tier_of(z), floors, extra));
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
    let mut comments: Vec<String> = vec![
        format!(
            "imported from TheCityGraphics by citysim-cli import-graphics (Scale S3c): downsample {k}, {s} m tiles, crop {}, employers and beds for {} residents",
            args.crop.as_deref().unwrap_or("none"),
            args.residents
        ),
        format!("inputs: {}", rep.inputs.join("; ")),
        "docs/IMPORT_GRAPHICS.md: the mapping and what is not mapped".into(),
    ];
    let elements = arr(&elevated_json, "elements", "elevated.json")?;
    let mut unmapped_el: BTreeMap<String, usize> = BTreeMap::new();
    for (i, e) in elements.iter().enumerate() {
        let kd = text(e, "kind", &format!("elevated.json elements[{i}]"))?;
        if kd != "rail" && kd != "station" && kd != "funicular" {
            *unmapped_el.entry(kd.to_string()).or_default() += 1;
        }
    }
    if let Some(r) = &rail_json {
        // S3c: sim/rail.json, each line by its name with its stations kept
        // as waypoints (a waypoint is a stop); its service is not read (the
        // sim's `[transit]` headway and dwell run every line).
        for (i, l) in arr(r, "lines", "sim/rail.json")?.iter().enumerate() {
            let ctx = format!("sim/rail.json lines[{i}]");
            let name = text(l, "name", &ctx)?;
            let pts = points(l, "pts", &ctx)?;
            let u: Vec<f64> = arr(l, "u", &ctx)?.iter().map(|v| num(v, &ctx)).collect::<Result<_, _>>()?;
            if u.len() != pts.len() {
                return Err(format!("{ctx}: {} pts but {} u", pts.len(), u.len()));
            }
            let mut keep = BTreeSet::new();
            let mut names = Vec::new();
            for (j, st) in arr(l, "stops", &ctx)?.iter().enumerate() {
                let c = format!("{ctx} stops[{j}]");
                let su = num(get(st, "u", &c)?, &format!("{c} \"u\""))?;
                names.push(text(st, "name", &c)?.to_string());
                let at = (0..u.len()).min_by(|&a, &b| (u[a] - su).abs().total_cmp(&(u[b] - su).abs())).unwrap_or(0);
                keep.insert(at);
            }
            match line_waypoints(&f, &pts, &keep) {
                Some(wp) => {
                    comments.push(format!(
                        "El line {name}: stations {} (sim/rail.json; headway {} s, {} s at night there; the sim's [transit] timetable runs it)",
                        names.join(", "),
                        l.get("headway_s").and_then(Value::as_f64).unwrap_or(0.0),
                        l.get("night_headway_s").and_then(Value::as_f64).unwrap_or(0.0)
                    ));
                    lines.push(TransitLineCfg { name: name.to_string(), waypoints: wp, in_flight: false });
                }
                None => rep.warnings.push(format!("{ctx} ({name}): under two track points in the window")),
            }
        }
        rep.unmapped.push(
            "sim/rail.json service (headway_s, night_headway_s, service_hours, dwell_s, speed_mps, trains, cars): the sim's [transit] timetable runs every line".into(),
        );
    } else {
        let mut stations: Vec<(f64, f64)> = Vec::new();
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
            if text(e, "kind", &ctx)? != "rail" {
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
                Some(wp) => lines.push(TransitLineCfg { name: format!("El {id}"), waypoints: wp, in_flight: false }),
                None => rep.warnings.push(format!("{ctx} ({id}): under two track points in the window")),
            }
        }
    }
    // The pre-view pass phase 5 (P11): the lines in flight go after the
    // export's (the God's line rows keep their numbers).
    let mut flight: Vec<TransitLineCfg> = Vec::new();
    // The pre-view pass phase 5 (P11): the funicular (the Peak's tram up the
    // ridge) is a line between its two stair portals (its ends), in flight
    // (`L+`) until the phase lands. The escalator stays unmapped: a moving
    // walkway at a walk's pace on a ground the sim reads as flat.
    for (i, e) in elements.iter().enumerate() {
        let ctx = format!("elevated.json elements[{i}]");
        if text(e, "kind", &ctx)? != "funicular" {
            continue;
        }
        let pts = points(e, "pts", &ctx)?;
        if pts.len() < 2 {
            continue;
        }
        let keep: BTreeSet<usize> = [0, pts.len() - 1].into_iter().collect();
        match line_waypoints(&f, &pts, &keep) {
            Some(wp) => flight.push(TransitLineCfg { name: "Funicular".into(), waypoints: wp, in_flight: true }),
            None => rep.warnings.push(format!("{ctx} (funicular): under two track points in the window")),
        }
    }
    for (kd, n) in &unmapped_el {
        rep.unmapped.push(format!("elevated.json: {n} \"{kd}\" element(s) (no sim counterpart)"));
    }
    // The tram (street/tram.json): its loop out to the terminus is a line,
    // named after its street (street/names.json) where that file names it.
    let street_names = read_json(&src.join("street"), "names.json").ok();
    match read_json(&src.join("street"), "tram.json") {
        Err(e) => rep.warnings.push(format!("{e}; no tram line")),
        Ok(t) => {
            for (i, l) in arr(&t, "lines", "tram.json")?.iter().enumerate() {
                let ctx = format!("tram.json lines[{i}]");
                let way = l.get("way").and_then(|v| v.as_str()).unwrap_or("tram");
                let street = street_names.as_ref().and_then(|n| n.get(way)).and_then(Value::as_str).unwrap_or(way);
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
                    Some(wp) => {
                        lines.push(TransitLineCfg { name: format!("Tram {street}"), waypoints: wp, in_flight: false })
                    }
                    None => rep.warnings.push(format!("{ctx}: under two tram points in the window")),
                }
            }
        }
    }
    lines.append(&mut flight);
    // The pre-view pass phase 5 (P11): the authored lines (`--lines`), each
    // waypoint inside the map.
    if let Some(path) = &args.lines {
        let t = std::fs::read_to_string(path).map_err(|e| format!("--lines {}: {e}", path.display()))?;
        for (n, row) in t.lines().enumerate() {
            let row = row.trim();
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = row.split_whitespace().collect();
            let mut wp: Vec<[u16; 2]> = Vec::new();
            let mut k = 0;
            while let Some((x, y)) = parts.get(k).and_then(|p| p.split_once(',')) {
                let (x, y) = (x.parse::<u16>(), y.parse::<u16>());
                let (Ok(x), Ok(y)) = (x, y) else {
                    return Err(format!("--lines {} row {}: bad waypoint {}", path.display(), n + 1, parts[k]));
                };
                if usize::from(x) >= w || usize::from(y) >= h {
                    return Err(format!("--lines {} row {}: {x},{y} is off the {w} x {h} map", path.display(), n + 1));
                }
                wp.push([x, y]);
                k += 1;
            }
            if wp.len() < 2 || k >= parts.len() {
                return Err(format!("--lines {} row {}: two waypoints and a name", path.display(), n + 1));
            }
            let name = parts[k..].join(" ");
            lines.push(TransitLineCfg { name, waypoints: wp, in_flight: true });
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
    if sim_lots.is_some() {
        rep.unmapped.push(
            "lots_sim.json shop_units: Markets and Bars follow use and ground, not the unit count; workplaces keep one floor (their storeys are written as storeys=)".into(),
        );
    } else {
        rep.unmapped.push("lots.json floors: the district's range, not the lot's massing (a seeded pick in it)".into());
    }
    if terrain.is_none() {
        rep.unmapped
            .push("terrain: sea and mountain both read as Water (outside every block, way and superblock)".into());
    }
    rep.unmapped.push("roads.json nodes/edges, traffic.json lanes: vehicles are not simulated on roads".into());
    rep.unmapped.push(
        "sim_layout.toml rooms, portals, slots: a Home is 5 residents a floor slot; flats and shop units are counted, rooms are not placed"
            .into(),
    );

    let zones: Vec<char> =
        dgrid.iter().map(|&d| zone_of_d.get(usize::from(d)).copied().unwrap_or(Z::Mid).letter()).collect();
    let grid: Vec<u8> = dgrid.iter().map(|&d| d as u8).collect();
    let extra: Vec<BuildingExtra> = out_b.iter().map(|b| b.4).collect();
    let text_out = MapText {
        w,
        h,
        tiles,
        zones,
        buildings: out_b.into_iter().map(|(kd, r, t, fl, _)| (kd, r, t, fl)).collect(),
        districts,
        district_grid: grid,
        lines,
        comments,
        extra,
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
        ImportArgs {
            src: src.to_path_buf(),
            out: src.join("out.txt"),
            downsample: 1,
            residents: 1_000,
            crop: None,
            lines: None,
        }
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

    /// S3c: the tiny city with their sim export (`sim/`): the hills north of
    /// the city (outside every block and way) are Mountain, the named civic
    /// site is the Hall, the Homes carry their flats, the rail line its name.
    fn write_sim(d: &Path, schema: &str, drop_use_of: Option<&str>) {
        std::fs::write(d.join("terrain.json"), r#"{"summary":{"size_m":[80,70]}}"#).expect("write");
        let sim = d.join("sim");
        std::fs::create_dir_all(&sim).expect("mkdir");
        let mut lots = Vec::new();
        for i in 0..10 {
            let x = f64::from(i) * 8.0 + 4.0;
            let (use_s, ground, flats, civic) =
                if i == 0 { ("civic", "civic", 0, r#","civic":"city_hall""#) } else { ("shop_house", "shop", 3, "") };
            lots.push(format!(
                r#""m{i}":{{"floors":5,"use":"{use_s}","ground":"{ground}","flats":{flats},"shop_units":1,"source":"estimate","doors_m":[{{"at":[{x},48],"out":[{x},49],"side":"N"}}]{civic}}}"#
            ));
            let use_field =
                if drop_use_of == Some(&format!("l{i}")) { String::new() } else { r#""use":"residential","#.into() };
            lots.push(format!(
                r#""l{i}":{{"floors":6,{use_field}"ground":"lobby","flats":{},"shop_units":0,"source":"estimate","doors_m":[{{"at":[{x},12],"out":[{x},11],"side":"S"}}]}}"#,
                4 + i
            ));
        }
        let lots = format!(r#"{{"schema":"{schema}","lots":{{{}}}}}"#, lots.join(","));
        let rows: Vec<String> =
            (0..35).map(|r| if r < 5 { "\"40m\"".to_string() } else { "\"40u\"".to_string() }).collect();
        let terrain = format!(r#"{{"schema":"{schema}","tile_m":2,"nx":40,"ny":35,"rows":[{}]}}"#, rows.join(","));
        let rail = format!(
            r#"{{"schema":"{schema}","lines":[{{"name":"Test Line","pts":[[2,6,40],[40,6,40],[78,6,40]],"u":[0,38,76],
            "stops":[{{"name":"West","u":0}},{{"name":"East","u":76}}],"headway_s":240}}]}}"#
        );
        for (n, t) in [("lots_sim.json", lots), ("terrain_sim.json", terrain), ("rail.json", rail)] {
            std::fs::write(sim.join(n), t).expect("write");
        }
    }

    #[test]
    fn test_import_reads_the_sim_export() {
        let d = tmp("sim");
        write_city(&d, "rect_m");
        write_sim(&d, "city-sim/0.9", None);
        let (text, rep) = build(&args(&d)).expect("imports");
        let m = citysim::Map::parse(&text);
        assert_eq!((m.w(), m.h()), (40, 35));
        assert_eq!(m.tiles().iter().filter(|&&t| t == citysim::TileKind::Mountain).count(), 200, "{rep:?}");
        assert!(m.walkable(citysim::TilePos::new(10, 2)));
        assert_eq!(m.lines.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Test Line"]);
        assert_eq!(m.count_kind(BuildingKind::Hall), 1);
        assert!(m.buildings.iter().any(|b| b.kind == BuildingKind::Home && b.flats.is_some_and(|f| f > 0)));
        assert!(rep.doors_exact > 0, "{rep:?}");
        assert!(rep.inputs.iter().any(|i| i.starts_with("lots: sim/lots_sim.json")));
        assert!(rep.warnings.iter().any(|w| w.contains("schema \"city-sim/0.9\"")), "{:?}", rep.warnings);
        // A record missing a field fails naming the file, the record and the field.
        write_sim(&d, SIM_SCHEMA, Some("l3"));
        let err = build(&args(&d)).expect_err("a lot without use");
        assert!(err.contains("sim/lots_sim.json lots.l3: missing field \"use\""), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Track B (OPEN-136): the tiny city's south row (the `l` lots, all
    /// residential towers) is a Spire district. Before, a Lot only came from
    /// commercial cuts and shop-houses, so a Spire of towers had none and the
    /// megacorps' Hqs (`founding::tier_ok`: the Spire only) never stood; now
    /// the Spire takes its share of the generator's Lots from its smallest
    /// towers and keeps Homes. A Market stands (one a district when there
    /// are enough; at 200 residents the mix is one).
    #[test]
    fn test_import_deals_lots_and_markets_by_zone() {
        let d = tmp("zones");
        write_city(&d, "rect_m");
        write_sim(&d, SIM_SCHEMA, None);
        let districts = r#"{"rows":[{"id":"D1","name":"The Sump","zone":"Sump"},{"id":"D2","name":"The Spire","zone":"Spire"}],
            "superblocks":[{"district":"D1","rect_m":[0,30,80,70]},{"district":"D2","rect_m":[0,0,80,29]}]}"#;
        std::fs::write(d.join("districts.json"), districts).expect("write");
        let (text, rep) = build(&ImportArgs { residents: 200, ..args(&d) }).expect("imports");
        let m = citysim::Map::parse(&text);
        let spire = |b: &citysim::map::MapBuilding| m.zone(b.door) == Zone::Spire;
        let count =
            |k: BuildingKind, inside: bool| m.buildings.iter().filter(|b| b.kind == k && spire(b) == inside).count();
        assert!(count(BuildingKind::Lot, true) >= 1, "no Spire Lot: {rep:?}");
        assert!(count(BuildingKind::Home, true) >= 1, "the Spire lost every Home: {rep:?}");
        assert!(count(BuildingKind::Market, true) + count(BuildingKind::Market, false) >= 1, "{rep:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
