//! The tile map, parsed from `assets/map.txt` (v2, 256 x 192 with a zone
//! grid) or `assets/map_v1.txt` (96 x 64, no header, no zones).
//!
//! The loader panics on any dimension or legend violation: the map is a
//! committed asset, so a bad one is a build error, not a runtime condition.
//!
//! v2 format: a `"<w> <h>"` header; `h` tile rows of `w` chars; a blank line;
//! `h` zone rows of `w` chars from `S C V M U`; a blank line; then
//! `B <Kind> <x> <y> <w> <h> [<tier>]` lines. v1 has no header and no zone
//! grid: `w` is the first row's length, `h` the rows before the first blank.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::components::{default_tier, BuildingKind, Rect, TileKind, TilePos, Zone};

/// One `B <Kind> <x> <y> <w> <h> [<tier>]` line, with its door derived from the grid.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapBuilding {
    pub kind: BuildingKind,
    pub rect: Rect,
    pub door: TilePos,
    /// 0 Sump, 1 Mid, 2 Spire; 1 when the line has no seventh field.
    #[serde(default = "default_tier")]
    pub tier: u8,
}

fn v1_w() -> u16 {
    96
}

fn v1_h() -> u16 {
    64
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Map {
    /// Pre-M10 saves have no dimensions: they were all 96 x 64.
    #[serde(default = "v1_w")]
    w: u16,
    #[serde(default = "v1_h")]
    h: u16,
    /// Row-major, index `y * w + x`.
    tiles: Vec<TileKind>,
    /// Row-major like `tiles`; empty for a v1 map (every tile reads `Mid`).
    #[serde(default)]
    zones: Vec<Zone>,
    /// In file order; `EntityId`s are assigned in this order.
    pub buildings: Vec<MapBuilding>,
}

fn parse_tile(ch: char, x: usize, y: usize) -> TileKind {
    match ch {
        '.' => TileKind::Ground,
        '#' => TileKind::Wall,
        '=' => TileKind::Road,
        'D' => TileKind::Door,
        'f' => TileKind::Farmland,
        '~' => TileKind::Water,
        other => panic!("row {y} col {x}: unknown tile {other:?}"),
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

        // v2 header: exactly two numbers.
        let header: Option<(u16, u16)> = {
            let parts: Vec<&str> = lines[0].split_whitespace().collect();
            match parts.as_slice() {
                [a, b] => a.parse().ok().zip(b.parse().ok()),
                _ => None,
            }
        };
        let (w, h, mut at) = match header {
            Some((w, h)) => (usize::from(w), usize::from(h), 1),
            None => {
                let w = lines[0].chars().count();
                let h = lines.iter().take_while(|l| !l.trim().is_empty()).count();
                (w, h, 0)
            }
        };
        assert!(w > 0 && h > 0 && w <= 256 && h <= 256, "map is {w} x {h}; both must be 1..=256 (TilePos is u8)");
        assert!(lines.len() > at + h, "map must have {h} grid rows then a blank line");

        let mut tiles = Vec::with_capacity(w * h);
        for (y, line) in lines[at..at + h].iter().enumerate() {
            assert_eq!(line.chars().count(), w, "row {y} must be {w} chars");
            for (x, ch) in line.chars().enumerate() {
                tiles.push(parse_tile(ch, x, y));
            }
        }
        at += h;
        assert!(lines[at].trim().is_empty(), "line {} must be blank", at + 1);
        at += 1;

        // Optional zone grid: present when the next non-blank line is not a `B` line.
        let mut zones = Vec::new();
        if lines.get(at).is_some_and(|l| !l.trim().is_empty() && !l.trim_start().starts_with("B ")) {
            assert!(lines.len() >= at + h, "map must have {h} zone rows after the tile grid");
            zones.reserve(w * h);
            for (y, line) in lines[at..at + h].iter().enumerate() {
                assert_eq!(line.chars().count(), w, "zone row {y} must be {w} chars");
                for (x, ch) in line.chars().enumerate() {
                    let z = Zone::parse(ch).unwrap_or_else(|| panic!("zone row {y} col {x}: unknown zone {ch:?}"));
                    zones.push(z);
                }
            }
            at += h;
            assert!(lines.get(at).is_none_or(|l| l.trim().is_empty()), "line {} must be blank", at + 1);
        }

        let mut map = Map { w: w as u16, h: h as u16, tiles, zones, buildings: Vec::new() };
        for (n, line) in lines.iter().enumerate().skip(at) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert!(
                (parts.len() == 6 || parts.len() == 7) && parts[0] == "B",
                "line {}: expected `B <Kind> <x> <y> <w> <h> [<tier>]`",
                n + 1
            );
            let kind = BuildingKind::parse(parts[1])
                .unwrap_or_else(|| panic!("line {}: unknown building kind {}", n + 1, parts[1]));
            let num = |s: &str| -> u8 { s.parse().unwrap_or_else(|_| panic!("line {}: bad number {s}", n + 1)) };
            let rect = Rect { x: num(parts[2]), y: num(parts[3]), w: num(parts[4]), h: num(parts[5]) };
            let tier = parts.get(6).map_or(1, |s| num(s));
            assert!(tier <= 2, "line {}: tier {tier} must be 0..=2", n + 1);
            let door = map.validate_building(kind, rect);
            map.buildings.push(MapBuilding { kind, rect, door, tier });
        }

        map.validate_orphans();
        map
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
                let p = TilePos { x: x as u8, y: y as u8 };
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
        for y in 0..self.h() {
            for x in 0..self.w() {
                let p = TilePos { x: x as u8, y: y as u8 };
                let t = self.tile_at(p);
                if matches!(t, TileKind::Door | TileKind::Farmland) {
                    assert!(
                        self.buildings.iter().any(|b| b.rect.contains(p)),
                        "{t:?} tile at {p} is outside every building"
                    );
                }
            }
        }
    }

    /// True if a 4-neighbour of `door` outside `rect` is a Road tile.
    pub fn door_faces_road(&self, rect: Rect, door: TilePos) -> bool {
        self.neighbours4(door).any(|n| !rect.contains(n) && self.tile_at(n) == TileKind::Road)
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
        self.tile_at(p).walkable()
    }

    /// In-bounds 4-neighbours, in the fixed order W, E, N, S.
    pub fn neighbours4(&self, p: TilePos) -> impl Iterator<Item = TilePos> + '_ {
        let (x, y) = (i32::from(p.x), i32::from(p.y));
        [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
            .into_iter()
            .filter(|&(nx, ny)| self.in_bounds(nx, ny))
            .map(|(nx, ny)| TilePos { x: nx as u8, y: ny as u8 })
    }

    /// Road tiles on the map border, ascending by `(y, x)`.
    pub fn edge_roads(&self) -> Vec<TilePos> {
        let (w, h) = (self.w(), self.h());
        let mut out = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let on_edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                if on_edge && self.tile(x, y) == TileKind::Road {
                    out.push(TilePos { x: x as u8, y: y as u8 });
                }
            }
        }
        out
    }

    pub fn count_kind(&self, kind: BuildingKind) -> usize {
        self.buildings.iter().filter(|b| b.kind == kind).count()
    }
}
