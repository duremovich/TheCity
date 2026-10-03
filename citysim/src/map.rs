//! The fixed 96×64 tile map, parsed from `assets/map.txt`.
//!
//! The loader panics on any dimension or legend violation: the map is a
//! committed asset, so a bad one is a build error, not a runtime condition.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::components::{BuildingKind, Rect, TileKind, TilePos};

pub const MAP_W: usize = 96;
pub const MAP_H: usize = 64;

/// One `B <Kind> <x> <y> <w> <h>` line, with its door derived from the grid.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapBuilding {
    pub kind: BuildingKind,
    pub rect: Rect,
    pub door: TilePos,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Map {
    /// Row-major, index `y * MAP_W + x`.
    tiles: Vec<TileKind>,
    /// In file order; `EntityId`s are assigned in this order.
    pub buildings: Vec<MapBuilding>,
}

impl Map {
    pub fn load(path: &Path) -> Map {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read map {}: {e}", path.display()));
        Map::parse(&text)
    }

    /// Parse and validate. Panics with a precise message on any violation.
    pub fn parse(text: &str) -> Map {
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > MAP_H, "map must have {MAP_H} grid rows then a blank line");

        let mut tiles = Vec::with_capacity(MAP_W * MAP_H);
        for (y, line) in lines.iter().take(MAP_H).enumerate() {
            assert_eq!(line.chars().count(), MAP_W, "row {y} must be {MAP_W} chars");
            for (x, ch) in line.chars().enumerate() {
                let kind = match ch {
                    '.' => TileKind::Ground,
                    '#' => TileKind::Wall,
                    '=' => TileKind::Road,
                    'D' => TileKind::Door,
                    'f' => TileKind::Farmland,
                    '~' => TileKind::Water,
                    other => panic!("row {y} col {x}: unknown tile {other:?}"),
                };
                tiles.push(kind);
            }
        }
        assert!(lines[MAP_H].trim().is_empty(), "line {} must be blank", MAP_H + 1);

        let mut map = Map { tiles, buildings: Vec::new() };
        for (n, line) in lines.iter().enumerate().skip(MAP_H + 1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert!(parts.len() == 6 && parts[0] == "B", "line {}: expected `B <Kind> <x> <y> <w> <h>`", n + 1);
            let kind = BuildingKind::parse(parts[1])
                .unwrap_or_else(|| panic!("line {}: unknown building kind {}", n + 1, parts[1]));
            let num = |s: &str| -> u8 { s.parse().unwrap_or_else(|_| panic!("line {}: bad number {s}", n + 1)) };
            let rect = Rect { x: num(parts[2]), y: num(parts[3]), w: num(parts[4]), h: num(parts[5]) };
            let door = map.validate_building(kind, rect);
            map.buildings.push(MapBuilding { kind, rect, door });
        }

        map.validate_orphans();
        map
    }

    /// Check one building rect and return its door tile.
    fn validate_building(&self, kind: BuildingKind, rect: Rect) -> TilePos {
        assert!(rect.w >= 3 && rect.h >= 3, "{kind} at {},{}: rect too small", rect.x, rect.y);
        assert!(
            usize::from(rect.x) + usize::from(rect.w) <= MAP_W && usize::from(rect.y) + usize::from(rect.h) <= MAP_H,
            "{kind} at {},{}: rect out of bounds",
            rect.x,
            rect.y
        );
        let mut door = None;
        let mut farmland = 0;
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                let p = TilePos { x, y };
                let t = self.tile_at(p);
                if rect.on_perimeter(p) {
                    match t {
                        TileKind::Wall => {}
                        TileKind::Door => {
                            assert!(door.is_none(), "{kind} at {},{}: more than one door", rect.x, rect.y);
                            door = Some(p);
                        }
                        other => {
                            panic!("{kind} at {},{}: perimeter tile {p} is {other:?}, not Wall/Door", rect.x, rect.y)
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
        for y in 0..MAP_H {
            for x in 0..MAP_W {
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

    pub fn in_bounds(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < MAP_W && (y as usize) < MAP_H
    }

    pub fn tile(&self, x: usize, y: usize) -> TileKind {
        self.tiles[y * MAP_W + x]
    }

    pub fn tile_at(&self, p: TilePos) -> TileKind {
        self.tile(usize::from(p.x), usize::from(p.y))
    }

    pub fn set_tile(&mut self, p: TilePos, kind: TileKind) {
        self.tiles[usize::from(p.y) * MAP_W + usize::from(p.x)] = kind;
    }

    pub fn walkable(&self, p: TilePos) -> bool {
        self.tile_at(p).walkable()
    }

    /// In-bounds 4-neighbours, in the fixed order W, E, N, S.
    pub fn neighbours4(&self, p: TilePos) -> impl Iterator<Item = TilePos> {
        let (x, y) = (i32::from(p.x), i32::from(p.y));
        [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
            .into_iter()
            .filter(|&(nx, ny)| Map::in_bounds(nx, ny))
            .map(|(nx, ny)| TilePos { x: nx as u8, y: ny as u8 })
    }

    /// Road tiles on the map border, ascending by `(y, x)`.
    pub fn edge_roads(&self) -> Vec<TilePos> {
        let mut out = Vec::new();
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let on_edge = x == 0 || y == 0 || x == MAP_W - 1 || y == MAP_H - 1;
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
