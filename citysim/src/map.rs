//! The tile map, parsed from `assets/map.txt` (v2, 256 x 192 with a zone
//! grid) or `assets/map_v1.txt` (96 x 64, no header, no zones).
//!
//! The loader panics on any dimension or legend violation: the map is a
//! committed asset, so a bad one is a build error, not a runtime condition.
//!
//! v2 format: a `"<w> <h>"` header; `h` tile rows of `w` chars; a blank line;
//! `h` zone rows of `w` chars from `S C V M U`; a blank line; then
//! `B <Kind> <x> <y> <w> <h> [<tier> [<floors>]]` lines. v1 has no header and no zone
//! grid: `w` is the first row's length, `h` the rows before the first blank.
//!
//! Scale S3b (docs/SCALE_20K.md, docs/IMPORT_GRAPHICS.md) adds, all optional
//! (an older map parses as it did):
//! - the header `"<w> <h> rle"`: every grid row (tiles, zones, districts) is
//!   run-length encoded, `[<count>]<char>` runs (`7.=` is `.......=`);
//! - in the building section, `# ...` comment lines;
//! - `D <zone> <x_from> <x_to> <name...>`: a district row, as a `[districts]`
//!   row of the config (the map's rows replace the config's, `Map::apply_to`);
//! - `districts` followed by `h` rows of district ids (`a`-`z` are 0-25,
//!   `A`-`F` 26-31): the district of every tile, in place of the rows' zone
//!   and x ranges (an import's districts are not x ranges);
//! - `L <x>,<y> <x>,<y> [...] <name...>`: an El line (the map's lines
//!   replace `[[transit.lines]]`); `L+` the same, a line in flight (the
//!   pre-view pass phase 5's, dropped with `[legs]` off).
//!
//! Scale S3c (the TheCityGraphics sim export), also optional:
//! - the tile `m`, Mountain: walkable at 1.5 a step, never built on;
//! - after a `B` line's floors field, `flats=<n>` and `storeys=<n>`: the
//!   source city's own flat count and storeys (an import's; the sim deals a
//!   Home's households by its floors field and only carries these).
//!
//! A save packs the grids as run-length strings (SAVE_VERSION 13); an older
//! save's one-enum-per-tile lists still load.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::components::{default_tier, is_one_u8, one_u8, BuildingKind, Rect, TileKind, TilePos, Zone, MAX_DISTRICTS};
use crate::config::{Config, TransitLineCfg};

/// One `B <Kind> <x> <y> <w> <h> [<tier> [<floors>]]` line, with its door derived from the grid.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapBuilding {
    pub kind: BuildingKind,
    pub rect: Rect,
    pub door: TilePos,
    /// 0 Sump, 1 Mid, 2 Spire; 1 when the line has no seventh field.
    #[serde(default = "default_tier")]
    pub tier: u8,
    /// Jobs and room J6: storeys (1 when the line has no eighth field).
    #[serde(default = "one_u8", skip_serializing_if = "is_one_u8")]
    pub floors: u8,
    /// Scale S3c: the source city's flats in this building (`flats=`), for
    /// the Ambient tier; `None` on a map without the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flats: Option<u32>,
    /// Scale S3c: the source city's storeys (`storeys=`); `None` without.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storeys: Option<u16>,
}

/// Scale S3c: a `B` line's optional `key=value` fields ([`MapText::extra`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildingExtra {
    pub flats: Option<u32>,
    pub storeys: Option<u16>,
}

/// Scale S3b: one `D <zone> <x_from> <x_to> <name...>` line, a `[districts]`
/// row the map carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapDistrict {
    pub name: String,
    pub zone: Zone,
    pub x_from: u16,
    pub x_to: u16,
}

/// The largest map side (Scale S2): `TilePos` and `Rect` are `u16`, and a
/// rect's exclusive edge (`x + w`) is computed in `u32`.
pub const MAX_SIDE: usize = u16::MAX as usize;

fn v1_w() -> u16 {
    96
}

fn v1_h() -> u16 {
    64
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "MapRepr", into = "MapRepr")]
pub struct Map {
    w: u16,
    h: u16,
    /// Row-major, index `y * w + x`.
    tiles: Vec<TileKind>,
    /// Row-major like `tiles`; empty for a v1 map (every tile reads `Mid`).
    zones: Vec<Zone>,
    /// In file order; `EntityId`s are assigned in this order.
    pub buildings: Vec<MapBuilding>,
    /// M12 D19 (the Damage hook, off in M12): rubble tiles that block
    /// movement while `[litter] rubble_blocks`; written only by
    /// `litter::set_rubble`.
    pub blocked: BTreeSet<TilePos>,
    /// Scale S3b: the map's district rows (`D` lines); empty: the config's.
    pub districts: Vec<MapDistrict>,
    /// Scale S3b: a district id per tile (the `districts` grid); empty: the
    /// rows' zone and x ranges decide.
    district_grid: Vec<u8>,
    /// Scale S3b: the map's El lines (`L` lines); empty: the config's.
    pub lines: Vec<TransitLineCfg>,
    /// The perf pass phase 7: bumped by every `set_tile` (the key of
    /// `districts::rebuild`'s grid memo; not saved: a loaded map counts
    /// from 0 and a loaded world's memo is empty).
    tile_edits: u64,
}

/// The saved shape of a [`Map`]. SAVE_VERSION 13 writes the grids as
/// run-length strings (`tiles_rle`, `zones_rle`, `district_rle`); a save
/// before it carries `tiles` / `zones` as one enum name per tile, read here.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct MapRepr {
    /// Pre-M10 saves have no dimensions: they were all 96 x 64.
    #[serde(default = "v1_w")]
    w: u16,
    #[serde(default = "v1_h")]
    h: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tiles: Vec<TileKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    zones: Vec<Zone>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    tiles_rle: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    zones_rle: String,
    buildings: Vec<MapBuilding>,
    #[serde(default)]
    blocked: BTreeSet<TilePos>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    districts: Vec<MapDistrict>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    district_rle: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lines: Vec<TransitLineCfg>,
}

impl From<Map> for MapRepr {
    fn from(m: Map) -> MapRepr {
        MapRepr {
            w: m.w,
            h: m.h,
            tiles: Vec::new(),
            zones: Vec::new(),
            tiles_rle: rle_encode(m.tiles.iter().map(|&t| tile_char(t))),
            zones_rle: rle_encode(m.zones.iter().map(|z| z.letter())),
            buildings: m.buildings,
            blocked: m.blocked,
            districts: m.districts,
            district_rle: rle_encode(m.district_grid.iter().map(|&d| district_char(d))),
            lines: m.lines,
        }
    }
}

impl TryFrom<MapRepr> for Map {
    type Error = String;

    fn try_from(r: MapRepr) -> Result<Map, String> {
        let n = usize::from(r.w) * usize::from(r.h);
        let tiles = if r.tiles_rle.is_empty() {
            r.tiles
        } else {
            rle_decode(&r.tiles_rle)?
                .into_iter()
                .map(|c| try_tile(c).ok_or_else(|| format!("map save: unknown tile {c:?}")))
                .collect::<Result<Vec<_>, _>>()?
        };
        if tiles.len() != n {
            return Err(format!("map save: {} tiles for {} x {}", tiles.len(), r.w, r.h));
        }
        let zones = if r.zones_rle.is_empty() {
            r.zones
        } else {
            rle_decode(&r.zones_rle)?
                .into_iter()
                .map(|c| Zone::parse(c).ok_or_else(|| format!("map save: unknown zone {c:?}")))
                .collect::<Result<Vec<_>, _>>()?
        };
        if !zones.is_empty() && zones.len() != n {
            return Err(format!("map save: {} zones for {} x {}", zones.len(), r.w, r.h));
        }
        let district_grid = rle_decode(&r.district_rle)?
            .into_iter()
            .map(|c| district_id(c).ok_or_else(|| format!("map save: unknown district {c:?}")))
            .collect::<Result<Vec<_>, _>>()?;
        if !district_grid.is_empty() && district_grid.len() != n {
            return Err(format!("map save: {} district ids for {} x {}", district_grid.len(), r.w, r.h));
        }
        Ok(Map {
            w: r.w,
            h: r.h,
            tiles,
            zones,
            buildings: r.buildings,
            blocked: r.blocked,
            districts: r.districts,
            district_grid,
            lines: r.lines,
            tile_edits: 0,
        })
    }
}

/// The tile kind of a map-file char (review finding 10: the saved landmark
/// tiles), `None` for no tile.
pub fn tile_of_char(ch: char) -> Option<TileKind> {
    try_tile(ch)
}

fn try_tile(ch: char) -> Option<TileKind> {
    Some(match ch {
        '.' => TileKind::Ground,
        '#' => TileKind::Wall,
        '=' => TileKind::Road,
        'D' => TileKind::Door,
        'f' => TileKind::Farmland,
        '~' => TileKind::Water,
        'm' => TileKind::Mountain,
        _ => return None,
    })
}

fn parse_tile(ch: char, x: usize, y: usize) -> TileKind {
    try_tile(ch).unwrap_or_else(|| panic!("row {y} col {x}: unknown tile {ch:?}"))
}

/// The map-file char of a tile kind.
pub fn tile_char(t: TileKind) -> char {
    match t {
        TileKind::Ground => '.',
        TileKind::Wall => '#',
        TileKind::Road => '=',
        TileKind::Door => 'D',
        TileKind::Farmland => 'f',
        TileKind::Water => '~',
        TileKind::Mountain => 'm',
    }
}

/// The map-file char of a district id: `a`-`z` are 0-25, `A`-`F` 26-31.
pub fn district_char(d: u8) -> char {
    match d {
        0..=25 => char::from(b'a' + d),
        _ => char::from(b'A' + (d - 26).min(5)),
    }
}

/// The district id of a `districts` grid char.
pub fn district_id(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a'),
        'A'..='F' => Some(c as u8 - b'A' + 26),
        _ => None,
    }
}

/// Run-length encode chars: a run of 2 or more is `<count><char>`, a single
/// char is itself. No map legend char is a digit.
pub fn rle_encode(chars: impl Iterator<Item = char>) -> String {
    let mut out = String::new();
    let mut run: Option<(char, usize)> = None;
    fn flush(out: &mut String, r: Option<(char, usize)>) {
        if let Some((c, n)) = r {
            if n > 1 {
                out.push_str(&n.to_string());
            }
            out.push(c);
        }
    }
    for c in chars {
        match &mut run {
            Some((rc, n)) if *rc == c => *n += 1,
            _ => {
                flush(&mut out, run);
                run = Some((c, 1));
            }
        }
    }
    flush(&mut out, run);
    out
}

/// Decode [`rle_encode`]'s runs.
pub fn rle_decode(s: &str) -> Result<Vec<char>, String> {
    let mut out = Vec::new();
    let mut count: Option<usize> = None;
    for c in s.chars() {
        if let Some(d) = c.to_digit(10) {
            let n = count.unwrap_or(0).checked_mul(10).and_then(|n| n.checked_add(d as usize));
            count = Some(n.ok_or("run-length count overflows")?);
            continue;
        }
        let n = count.take().unwrap_or(1);
        if out.len() + n > MAX_SIDE * MAX_SIDE {
            return Err("run-length grid larger than the largest map".into());
        }
        out.extend(std::iter::repeat_n(c, n));
    }
    if count.is_some() {
        return Err(format!("run-length string ends in a count: {:?}", &s[s.len().saturating_sub(12)..]));
    }
    Ok(out)
}

/// Scale S3b: where a world's map comes from. `[world] map` names a text map
/// under the assets directory (the default, `map.txt`), an absolute path, or
/// `gen:<residents>[:<seed>]`, the deterministic generator (`mapgen`).
#[derive(Clone, Debug, PartialEq)]
pub enum MapSource {
    File(PathBuf),
    Generated { residents: u32, seed: u64 },
}

impl MapSource {
    /// The source `[world] map` names.
    pub fn from_config(cfg: &Config) -> MapSource {
        let spec = cfg.world.map.as_str();
        match spec.strip_prefix("gen:") {
            Some(rest) => {
                let mut parts = rest.split(':');
                let residents = parts
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| panic!("[world] map {spec:?}: expected gen:<residents>[:<seed>]"));
                let seed = parts
                    .next()
                    .map_or(0, |s| s.parse().unwrap_or_else(|_| panic!("[world] map {spec:?}: bad seed {s:?}")));
                MapSource::Generated { residents, seed }
            }
            None => MapSource::File(cfg.asset(spec)),
        }
    }

    /// Load (or generate) and validate the map. Panics on a bad map.
    pub fn load(&self) -> Map {
        match self {
            MapSource::File(path) => Map::load(path),
            MapSource::Generated { residents, seed } => Map::parse(&crate::mapgen::generate(*residents, *seed).text),
        }
    }
}

/// Scale S3b: the parts of a map file, for a writer (the generator, the
/// TheCityGraphics adapter). `to_text` writes the text format.
#[derive(Clone, Debug, Default)]
pub struct MapText {
    pub w: usize,
    pub h: usize,
    /// Row-major tile chars (`.#=Dfm~`).
    pub tiles: Vec<char>,
    /// Row-major zone letters; empty for none.
    pub zones: Vec<char>,
    /// `(kind, rect, tier, floors)` in file order.
    pub buildings: Vec<(BuildingKind, Rect, u8, u8)>,
    pub districts: Vec<MapDistrict>,
    /// Row-major district ids; empty for none (the rows decide).
    pub district_grid: Vec<u8>,
    pub lines: Vec<TransitLineCfg>,
    /// `#` lines written before the buildings.
    pub comments: Vec<String>,
    /// Scale S3c: per building (aligned with `buildings`; empty for none),
    /// the optional `flats=` / `storeys=` fields.
    pub extra: Vec<BuildingExtra>,
}

impl MapText {
    /// The text format; `rle` encodes every grid row.
    pub fn to_text(&self, rle: bool) -> String {
        let mut out = String::new();
        out.push_str(&format!("{} {}{}\n", self.w, self.h, if rle { " rle" } else { "" }));
        let grid = |out: &mut String, cells: &mut dyn Iterator<Item = char>| {
            let cells: Vec<char> = cells.collect();
            for row in cells.chunks(self.w.max(1)) {
                if rle {
                    out.push_str(&rle_encode(row.iter().copied()));
                } else {
                    out.extend(row.iter());
                }
                out.push('\n');
            }
        };
        grid(&mut out, &mut self.tiles.iter().copied());
        out.push('\n');
        if !self.zones.is_empty() {
            grid(&mut out, &mut self.zones.iter().copied());
            out.push('\n');
        }
        for c in &self.comments {
            out.push_str(&format!("# {c}\n"));
        }
        for (i, (kind, r, tier, floors)) in self.buildings.iter().enumerate() {
            out.push_str(&format!("B {kind:?} {} {} {} {} {tier}", r.x, r.y, r.w, r.h));
            let extra = self.extra.get(i).copied().unwrap_or_default();
            if *floors > 1 || extra != BuildingExtra::default() {
                out.push_str(&format!(" {floors}"));
            }
            if let Some(n) = extra.flats {
                out.push_str(&format!(" flats={n}"));
            }
            if let Some(n) = extra.storeys {
                out.push_str(&format!(" storeys={n}"));
            }
            out.push('\n');
        }
        for d in &self.districts {
            out.push_str(&format!("D {} {} {} {}\n", d.zone.letter(), d.x_from, d.x_to, d.name));
        }
        if !self.district_grid.is_empty() {
            out.push_str("districts\n");
            grid(&mut out, &mut self.district_grid.iter().map(|&d| district_char(d)));
        }
        for l in &self.lines {
            out.push_str(if l.in_flight { "L+" } else { "L" });
            for [x, y] in &l.waypoints {
                out.push_str(&format!(" {x},{y}"));
            }
            out.push_str(&format!(" {}\n", l.name));
        }
        out
    }
}

impl Map {
    pub fn load(path: &Path) -> Map {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read map {}: {e}", path.display()));
        Map::parse(&text)
    }

    /// Parse and validate. Panics with a precise message on any violation.
    pub fn parse(text: &str) -> Map {
        let lines: Vec<&str> = text.lines().collect();
        assert!(!lines.is_empty(), "empty map");

        // v2 header: exactly two numbers (S3b: and an optional `rle`).
        let header: Option<(u16, u16, bool)> = {
            let parts: Vec<&str> = lines[0].split_whitespace().collect();
            match parts.as_slice() {
                [a, b] => a.parse().ok().zip(b.parse().ok()).map(|(a, b)| (a, b, false)),
                [a, b, "rle"] => a.parse().ok().zip(b.parse().ok()).map(|(a, b)| (a, b, true)),
                _ => None,
            }
        };
        let (w, h, rle, mut at) = match header {
            Some((w, h, rle)) => (usize::from(w), usize::from(h), rle, 1),
            None => {
                let w = lines[0].chars().count();
                let h = lines.iter().take_while(|l| !l.trim().is_empty()).count();
                (w, h, false, 0)
            }
        };
        assert!(
            w > 0 && h > 0 && w <= MAX_SIDE && h <= MAX_SIDE,
            "map is {w} x {h}; both must be 1..={MAX_SIDE} (TilePos is u16)"
        );
        assert!(lines.len() > at + h, "map must have {h} grid rows then a blank line");
        // One grid row's chars (decoded when the header says `rle`).
        let row_chars = |line: &str, what: &str, y: usize| -> Vec<char> {
            let chars = if rle {
                rle_decode(line.trim_end()).unwrap_or_else(|e| panic!("{what} row {y}: {e}"))
            } else {
                line.chars().collect()
            };
            assert_eq!(chars.len(), w, "{what} row {y} must be {w} chars");
            chars
        };

        let mut tiles = Vec::with_capacity(w * h);
        for (y, line) in lines[at..at + h].iter().enumerate() {
            for (x, ch) in row_chars(line, "row", y).into_iter().enumerate() {
                tiles.push(parse_tile(ch, x, y));
            }
        }
        at += h;
        assert!(lines[at].trim().is_empty(), "line {} must be blank", at + 1);
        at += 1;

        // Optional zone grid: present when the next non-blank line is not a
        // building-section line (`B`, and since S3b `D`, `L`, `#`, `districts`).
        let section_line = |l: &str| {
            let t = l.trim_start();
            t.starts_with("B ")
                || t.starts_with("D ")
                || t.starts_with("L ")
                || t.starts_with("L+ ")
                || t.starts_with('#')
                || t == "districts"
        };
        let mut zones = Vec::new();
        if lines.get(at).is_some_and(|l| !l.trim().is_empty() && !section_line(l)) {
            assert!(lines.len() >= at + h, "map must have {h} zone rows after the tile grid");
            zones.reserve(w * h);
            for (y, line) in lines[at..at + h].iter().enumerate() {
                for (x, ch) in row_chars(line, "zone", y).into_iter().enumerate() {
                    let z = Zone::parse(ch).unwrap_or_else(|| panic!("zone row {y} col {x}: unknown zone {ch:?}"));
                    zones.push(z);
                }
            }
            at += h;
            assert!(lines.get(at).is_none_or(|l| l.trim().is_empty()), "line {} must be blank", at + 1);
        }

        let mut map = Map {
            w: w as u16,
            h: h as u16,
            tiles,
            zones,
            buildings: Vec::new(),
            blocked: Default::default(),
            districts: Vec::new(),
            district_grid: Vec::new(),
            lines: Vec::new(),
            tile_edits: 0,
        };
        let mut n = at;
        while n < lines.len() {
            let line = lines[n].trim();
            if line.is_empty() || line.starts_with('#') {
                n += 1;
                continue;
            }
            if line == "districts" {
                assert!(map.district_grid.is_empty(), "line {}: a second districts grid", n + 1);
                assert!(lines.len() > n + h, "line {}: the districts grid needs {h} rows", n + 1);
                map.district_grid.reserve(w * h);
                for (y, row) in lines[n + 1..=n + h].iter().enumerate() {
                    for (x, ch) in row_chars(row, "districts", y).into_iter().enumerate() {
                        let d = district_id(ch)
                            .unwrap_or_else(|| panic!("districts row {y} col {x}: unknown district {ch:?}"));
                        map.district_grid.push(d);
                    }
                }
                n += h + 1;
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts[0] {
                "D" => map.districts.push(parse_district(&parts, n + 1)),
                "L" | "L+" => map.lines.push(parse_line(&parts, n + 1, w, h)),
                _ => {
                    // S3c: `key=value` fields follow the floors field.
                    let keyed = parts.iter().position(|p| p.contains('=')).unwrap_or(parts.len());
                    let (parts, keyed) = parts.split_at(keyed);
                    assert!(
                        (6..=8).contains(&parts.len()) && parts[0] == "B" && (keyed.is_empty() || parts.len() == 8),
                        "line {}: expected `B <Kind> <x> <y> <w> <h> [<tier> [<floors> [key=value...]]]`",
                        n + 1
                    );
                    let kind = BuildingKind::parse(parts[1])
                        .unwrap_or_else(|| panic!("line {}: unknown building kind {}", n + 1, parts[1]));
                    let num =
                        |s: &str| -> u8 { s.parse().unwrap_or_else(|_| panic!("line {}: bad number {s}", n + 1)) };
                    let coord =
                        |s: &str| -> u16 { s.parse().unwrap_or_else(|_| panic!("line {}: bad number {s}", n + 1)) };
                    let rect = Rect { x: coord(parts[2]), y: coord(parts[3]), w: coord(parts[4]), h: coord(parts[5]) };
                    let tier = parts.get(6).map_or(1, |s| num(s));
                    assert!(tier <= 2, "line {}: tier {tier} must be 0..=2", n + 1);
                    // Jobs and room J6: the optional eighth field.
                    let floors = parts.get(7).map_or(1, |s| num(s));
                    assert!(floors >= 1, "line {}: floors must be at least 1", n + 1);
                    let (mut flats, mut storeys) = (None, None);
                    for kv in keyed {
                        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                        let bad = || -> ! { panic!("line {}: bad field {kv}", n + 1) };
                        match k {
                            "flats" => flats = Some(v.parse().unwrap_or_else(|_| bad())),
                            "storeys" => storeys = Some(v.parse().unwrap_or_else(|_| bad())),
                            _ => panic!("line {}: unknown field {k:?} (flats, storeys)", n + 1),
                        }
                    }
                    let door = map.validate_building(kind, rect);
                    map.buildings.push(MapBuilding { kind, rect, door, tier, floors, flats, storeys });
                }
            }
            n += 1;
        }

        map.validate_orphans();
        map.validate_districts();
        map
    }

    /// S3b: the district rows and grid agree, and every tile has a district.
    fn validate_districts(&self) {
        let n = self.districts.len();
        assert!(n <= MAX_DISTRICTS, "the map has {n} districts; at most {MAX_DISTRICTS}");
        if !self.district_grid.is_empty() {
            assert!(n > 0, "a districts grid needs `D` lines");
            if let Some(&bad) = self.district_grid.iter().find(|&&d| usize::from(d) >= n) {
                panic!("districts grid: id {bad} ({}) but the map has {n} `D` lines", district_char(bad));
            }
            return;
        }
        if n == 0 {
            return;
        }
        for d in &self.districts {
            assert!(d.x_from < d.x_to, "district {}: x_from {} must be under x_to {}", d.name, d.x_from, d.x_to);
        }
        for y in 0..self.h() {
            for x in 0..self.w() {
                let z = self.zone(TilePos::new(x as u16, y as u16));
                let xx = x as u16;
                assert!(
                    self.districts.iter().any(|d| d.zone == z && d.x_from <= xx && xx < d.x_to),
                    "no `D` row covers tile ({x},{y}) (zone {z})"
                );
            }
        }
    }

    /// Check one building rect and return its door tile.
    fn validate_building(&self, kind: BuildingKind, rect: Rect) -> TilePos {
        assert!(rect.w >= 3 && rect.h >= 3, "{kind} at {},{}: rect too small", rect.x, rect.y);
        assert!(
            usize::from(rect.x) + usize::from(rect.w) <= self.w()
                && usize::from(rect.y) + usize::from(rect.h) <= self.h(),
            "{kind} at {},{}: rect out of bounds",
            rect.x,
            rect.y
        );
        let lot = kind == BuildingKind::Lot;
        let mut door = None;
        let mut farmland = 0;
        for y in usize::from(rect.y)..usize::from(rect.y) + usize::from(rect.h) {
            for x in usize::from(rect.x)..usize::from(rect.x) + usize::from(rect.w) {
                let p = TilePos { x: x as u16, y: y as u16 };
                let t = self.tile_at(p);
                if rect.on_perimeter(p) {
                    match (t, lot) {
                        (TileKind::Door, _) => {
                            assert!(door.is_none(), "{kind} at {},{}: more than one door", rect.x, rect.y);
                            door = Some(p);
                        }
                        (TileKind::Wall, false) | (TileKind::Ground, true) => {}
                        (other, false) => {
                            panic!("{kind} at {},{}: perimeter tile {p} is {other:?}, not Wall/Door", rect.x, rect.y)
                        }
                        (other, true) => {
                            panic!("Lot at {},{}: perimeter tile {p} is {other:?}, not Ground/Door", rect.x, rect.y)
                        }
                    }
                } else {
                    match (kind, t) {
                        (BuildingKind::Farm, TileKind::Farmland) => farmland += 1,
                        (BuildingKind::Farm, other) => {
                            panic!("Farm at {},{}: interior tile {p} is {other:?}, not Farmland", rect.x, rect.y)
                        }
                        (_, TileKind::Ground) => {}
                        (_, other) => {
                            panic!("{kind} at {},{}: interior tile {p} is {other:?}, not Ground", rect.x, rect.y)
                        }
                    }
                }
            }
        }
        if kind == BuildingKind::Farm {
            assert!(farmland >= 60, "Farm at {},{}: only {farmland} Farmland tiles (need 60)", rect.x, rect.y);
        }
        let door = door.unwrap_or_else(|| panic!("{kind} at {},{}: no door", rect.x, rect.y));
        assert!(
            self.door_faces_road(rect, door),
            "{kind} at {},{}: the tile outside door {door} is not Road",
            rect.x,
            rect.y
        );
        door
    }

    /// Every Door and Farmland tile must belong to some building.
    fn validate_orphans(&self) {
        // Scale S3b: one pass over the rects, not a building scan per tile.
        let mut covered = vec![false; self.w() * self.h()];
        for b in &self.buildings {
            let r = b.rect;
            for y in usize::from(r.y)..usize::from(r.y) + usize::from(r.h) {
                let row = y * self.w();
                covered[row + usize::from(r.x)..row + usize::from(r.x) + usize::from(r.w)].fill(true);
            }
        }
        for y in 0..self.h() {
            for x in 0..self.w() {
                let p = TilePos { x: x as u16, y: y as u16 };
                let t = self.tile_at(p);
                if matches!(t, TileKind::Door | TileKind::Farmland) {
                    assert!(covered[y * self.w() + x], "{t:?} tile at {p} is outside every building");
                }
            }
        }
    }

    /// True if a 4-neighbour of `door` outside `rect` is a Road tile.
    pub fn door_faces_road(&self, rect: Rect, door: TilePos) -> bool {
        self.neighbours4(door).any(|n| !rect.contains(n) && self.tile_at(n) == TileKind::Road)
    }

    /// Scale S3b: the map's districts and El lines replace the config's
    /// `[districts]` rows and `[[transit.lines]]` (each only when the map
    /// carries any). A districts grid's rows keep the whole width as their x
    /// range (the grid decides).
    pub fn apply_to(&self, cfg: &mut Config) {
        if !self.districts.is_empty() {
            let d = &mut cfg.districts;
            d.names = self.districts.iter().map(|r| r.name.clone()).collect();
            d.zones = self.districts.iter().map(|r| r.zone.letter().to_string()).collect();
            d.x_from = self.districts.iter().map(|r| r.x_from).collect();
            d.x_to = self.districts.iter().map(|r| r.x_to).collect();
        }
        if !self.lines.is_empty() {
            // The pre-view pass phase 5: an `L+` line is in flight, dropped
            // with `[legs]` absent.
            let keep = cfg.legs.is_some();
            cfg.transit.lines = self.lines.iter().filter(|l| keep || !l.in_flight).cloned().collect();
        }
    }

    /// Scale S3b: the district the map's grid gives a tile (`None` without a grid).
    pub fn district_at(&self, p: TilePos) -> Option<u8> {
        if self.district_grid.is_empty() {
            return None;
        }
        self.district_grid.get(self.index(p)).copied()
    }

    /// Width in tiles.
    pub fn w(&self) -> usize {
        usize::from(self.w)
    }

    /// Height in tiles.
    pub fn h(&self) -> usize {
        usize::from(self.h)
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.w() && (y as usize) < self.h()
    }

    /// Row-major index of a tile.
    pub fn index(&self, p: TilePos) -> usize {
        usize::from(p.y) * self.w() + usize::from(p.x)
    }

    pub fn tile(&self, x: usize, y: usize) -> TileKind {
        self.tiles[y * self.w() + x]
    }

    /// Every tile, row-major (`index`).
    pub fn tiles(&self) -> &[TileKind] {
        &self.tiles
    }

    pub fn tile_at(&self, p: TilePos) -> TileKind {
        self.tile(usize::from(p.x), usize::from(p.y))
    }

    pub fn set_tile(&mut self, p: TilePos, kind: TileKind) {
        let i = self.index(p);
        self.tiles[i] = kind;
        self.tile_edits += 1;
    }

    /// The perf pass phase 7: how many `set_tile`s this map has taken.
    pub fn tile_edits(&self) -> u64 {
        self.tile_edits
    }

    /// The zone a tile lies in; `Mid` everywhere on a map without a zone grid.
    pub fn zone(&self, p: TilePos) -> Zone {
        if self.zones.is_empty() {
            return Zone::Mid;
        }
        self.zones.get(self.index(p)).copied().unwrap_or_default()
    }

    /// True if the map file carried a zone grid.
    pub fn has_zones(&self) -> bool {
        !self.zones.is_empty()
    }

    pub fn walkable(&self, p: TilePos) -> bool {
        self.tile_at(p).walkable() && !self.is_blocked(p)
    }

    /// M12 D19: a rubble tile (empty in M12 unless the hook is switched on).
    pub fn is_blocked(&self, p: TilePos) -> bool {
        !self.blocked.is_empty() && self.blocked.contains(&p)
    }

    /// In-bounds 4-neighbours, in the fixed order W, E, N, S.
    pub fn neighbours4(&self, p: TilePos) -> impl Iterator<Item = TilePos> + '_ {
        let (x, y) = (i32::from(p.x), i32::from(p.y));
        [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
            .into_iter()
            .filter(|&(nx, ny)| self.in_bounds(nx, ny))
            .map(|(nx, ny)| TilePos { x: nx as u16, y: ny as u16 })
    }

    /// Road tiles on the map border, ascending by `(y, x)`.
    pub fn edge_roads(&self) -> Vec<TilePos> {
        let (w, h) = (self.w(), self.h());
        let mut out = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let on_edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                if on_edge && self.tile(x, y) == TileKind::Road {
                    out.push(TilePos { x: x as u16, y: y as u16 });
                }
            }
        }
        out
    }

    pub fn count_kind(&self, kind: BuildingKind) -> usize {
        self.buildings.iter().filter(|b| b.kind == kind).count()
    }
}

/// `D <zone> <x_from> <x_to> <name...>`.
fn parse_district(parts: &[&str], line: usize) -> MapDistrict {
    assert!(parts.len() >= 5, "line {line}: expected `D <zone> <x_from> <x_to> <name...>`");
    let mut zc = parts[1].chars();
    let zone = match (zc.next(), zc.next()) {
        (Some(c), None) => Zone::parse(c),
        _ => None,
    }
    .unwrap_or_else(|| panic!("line {line}: unknown zone {:?}", parts[1]));
    let coord = |s: &str| -> u16 { s.parse().unwrap_or_else(|_| panic!("line {line}: bad number {s}")) };
    MapDistrict { name: parts[4..].join(" "), zone, x_from: coord(parts[2]), x_to: coord(parts[3]) }
}

/// `L <x>,<y> <x>,<y> [...] <name...>`.
fn parse_line(parts: &[&str], line: usize, w: usize, h: usize) -> TransitLineCfg {
    let mut waypoints = Vec::new();
    let mut k = 1;
    while let Some((x, y)) = parts.get(k).and_then(|s| s.split_once(',')) {
        let num = |s: &str| -> u16 { s.parse().unwrap_or_else(|_| panic!("line {line}: bad waypoint {}", parts[k])) };
        let (x, y) = (num(x), num(y));
        assert!(usize::from(x) < w && usize::from(y) < h, "line {line}: waypoint {x},{y} is off the map");
        waypoints.push([x, y]);
        k += 1;
    }
    assert!(waypoints.len() >= 2, "line {line}: an `L` line needs at least two waypoints");
    assert!(k < parts.len(), "line {line}: an `L` line needs a name after its waypoints");
    TransitLineCfg { name: parts[k..].join(" "), waypoints, in_flight: parts[0] == "L+" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rle_round_trips() {
        let s: Vec<char> = "......=====#D#..fff~~~~~~~~~~~~a".chars().collect();
        let e = rle_encode(s.iter().copied());
        assert_eq!(e, "6.5=#D#2.3f12~a");
        assert_eq!(rle_decode(&e).expect("decode"), s);
        assert!(rle_decode("12").is_err());
        assert_eq!(rle_encode(std::iter::empty()), "");
    }

    #[test]
    fn test_district_chars() {
        for d in 0..MAX_DISTRICTS as u8 {
            assert_eq!(district_id(district_char(d)), Some(d));
        }
        assert_eq!(district_id('G'), None);
    }
}
