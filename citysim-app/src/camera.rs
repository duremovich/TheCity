//! Tile-space camera. The renderer does its own tile→screen transform so
//! the y axis is unambiguous: tiles grow downward like the map file.

use macroquad::prelude::*;

use citysim::{Rect as TileRect, TilePos};

pub const MIN_PX_PER_TILE: f32 = 8.0;
pub const MAX_PX_PER_TILE: f32 = 48.0;
pub const DEFAULT_PX_PER_TILE: f32 = 16.0;
pub const PAN_TILES_PER_SEC: f32 = 20.0;
pub const ZOOM_PER_NOTCH: f32 = 1.25;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Tile units.
    pub centre: Vec2,
    /// `8.0..=48.0`.
    pub px_per_tile: f32,
    /// Map size in tiles (from the loaded map).
    pub map_w: f32,
    pub map_h: f32,
}

impl Camera {
    pub fn new(map_w: usize, map_h: usize) -> Camera {
        let (map_w, map_h) = (map_w as f32, map_h as f32);
        Camera { centre: vec2(map_w / 2.0, map_h / 2.0), px_per_tile: DEFAULT_PX_PER_TILE, map_w, map_h }
    }

    /// Screen pixel of a tile-space point.
    pub fn tile_to_screen(&self, tile: Vec2) -> Vec2 {
        (tile - self.centre) * self.px_per_tile + vec2(screen_width(), screen_height()) * 0.5
    }

    /// Tile-space point under a screen pixel.
    pub fn screen_to_tile(&self, screen: Vec2) -> Vec2 {
        (screen - vec2(screen_width(), screen_height()) * 0.5) / self.px_per_tile + self.centre
    }

    /// The integer tile under a screen pixel, if on the map.
    pub fn tile_at(&self, screen: Vec2) -> Option<TilePos> {
        let t = self.screen_to_tile(screen);
        if t.x < 0.0 || t.y < 0.0 || t.x >= self.map_w || t.y >= self.map_h {
            return None;
        }
        Some(TilePos { x: t.x as u8, y: t.y as u8 })
    }

    /// Visible tiles, clamped to the map; `w` and `h` at most 255 (`u8`).
    /// Passed to `world.set_view` every frame.
    pub fn view_rect(&self) -> TileRect {
        let half = vec2(screen_width(), screen_height()) * 0.5 / self.px_per_tile;
        let x0 = (self.centre.x - half.x).floor().clamp(0.0, self.map_w - 1.0);
        let y0 = (self.centre.y - half.y).floor().clamp(0.0, self.map_h - 1.0);
        let x1 = (self.centre.x + half.x).ceil().clamp(1.0, self.map_w);
        let y1 = (self.centre.y + half.y).ceil().clamp(1.0, self.map_h);
        TileRect {
            x: x0 as u8,
            y: y0 as u8,
            w: (x1 - x0).clamp(1.0, 255.0) as u8,
            h: (y1 - y0).clamp(1.0, 255.0) as u8,
        }
    }

    pub fn pan(&mut self, delta_tiles: Vec2) {
        self.centre += delta_tiles;
        self.clamp_centre();
    }

    /// Zoom by `factor` keeping the tile under `screen` fixed.
    pub fn zoom_about(&mut self, screen: Vec2, factor: f32) {
        let before = self.screen_to_tile(screen);
        self.px_per_tile = (self.px_per_tile * factor).clamp(MIN_PX_PER_TILE, MAX_PX_PER_TILE);
        let after = self.screen_to_tile(screen);
        self.centre += before - after;
        self.clamp_centre();
    }

    pub fn centre_on(&mut self, tile: TilePos) {
        self.centre = vec2(f32::from(tile.x) + 0.5, f32::from(tile.y) + 0.5);
        self.clamp_centre();
    }

    fn clamp_centre(&mut self) {
        self.centre.x = self.centre.x.clamp(0.0, self.map_w);
        self.centre.y = self.centre.y.clamp(0.0, self.map_h);
    }
}
