//! M15 § 10, the Word overlay (`J`; `K` is M13's hooked heat): each
//! district filled crimson by its talk (Σ pool reach against the loudest
//! district), an eye over every hunter on a stake-out, and a red line
//! between every pair of factions in an open vendetta. Reads state only.

use macroquad::prelude::*;

use citysim::{Building, Corp, EntityId, Gang, Position, TilePos, World};

use crate::App;

const CRIMSON: Color = Color::new(0xD0 as f32 / 255.0, 0x30 as f32 / 255.0, 0x3A as f32 / 255.0, 1.0);

/// Σ reach of each district's pool.
fn heat(world: &World) -> Vec<f32> {
    world.rumours.iter().map(|p| p.entries.iter().map(|e| e.reach).sum()).collect()
}

/// The fill, under the buildings and agents: alpha from `0.04` (the
/// quietest district) to `0.4` (the loudest).
pub fn fill(world: &World, app: &App, bounds: (u16, u16, u16, u16)) {
    if world.districts.is_empty() || world.rumours.is_empty() {
        return;
    }
    let cam = &app.camera;
    let ppt = cam.px_per_tile;
    let h = heat(world);
    // Relative to the quietest and loudest district, so a city where every
    // district talks still shows where it talks most.
    let loud = h.iter().copied().fold(0.0f32, f32::max);
    let quiet = h.iter().copied().fold(f32::MAX, f32::min).min(loud);
    let span = (loud - quiet).max(1e-3);
    let (vx0, vy0, x1, y1) = bounds;
    let (w, ht) = (world.map.w() as u16, world.map.h() as u16);
    for y in vy0..y1.min(ht) {
        for x in vx0..x1.min(w) {
            let d = world.district_of(TilePos { x: x as u8, y: y as u8 });
            let v = h.get(d.index()).copied().unwrap_or(0.0);
            if v <= 0.0 {
                continue;
            }
            let p = cam.tile_to_screen(vec2(f32::from(x), f32::from(y)));
            draw_rectangle(p.x, p.y, ppt + 0.5, ppt + 0.5, Color { a: 0.04 + 0.36 * (v - quiet) / span, ..CRIMSON });
        }
    }
}

/// Where a faction is drawn from: a gang's Hideout, a corp's buildings'
/// mean door, the Law's Precinct.
fn anchor(world: &World, f: EntityId) -> Option<Vec2> {
    let centre = |b: EntityId| {
        world.comp::<Building>(b).map(|bd| {
            vec2(f32::from(bd.rect.x) + f32::from(bd.rect.w) / 2.0, f32::from(bd.rect.y) + f32::from(bd.rect.h) / 2.0)
        })
    };
    if let Some(g) = world.comp::<Gang>(f) {
        return centre(g.hideout);
    }
    if let Some(c) = world.comp::<Corp>(f) {
        let pts: Vec<Vec2> = c.buildings.iter().filter_map(|&b| centre(b)).collect();
        if pts.is_empty() {
            return None;
        }
        return Some(pts.iter().copied().fold(Vec2::ZERO, |a, b| a + b) / pts.len() as f32);
    }
    centre(f)
}

/// Over everything: each district's talk as a number at its centroid, an
/// eye over each hunter on a stake-out (Full or Coarse: both have a
/// position), and a red line per open vendetta with its kills.
pub fn marks(world: &World, app: &App) {
    let cam = &app.camera;
    let ppt = cam.px_per_tile;
    let h = heat(world);
    for (i, d) in world.districts.iter().enumerate() {
        let v = h.get(i).copied().unwrap_or(0.0);
        let p = cam.tile_to_screen(vec2(f32::from(d.centroid.x), f32::from(d.centroid.y)));
        let text = format!("{} · talk {v:.1}", d.name);
        let dims = measure_text(&text, None, 16, 1.0);
        draw_rectangle(
            p.x - dims.width / 2.0 - 3.0,
            p.y - dims.offset_y - 2.0,
            dims.width + 6.0,
            dims.height + 4.0,
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        draw_text(&text, p.x - dims.width / 2.0, p.y, 16.0, WHITE);
    }
    for v in &world.vendettas {
        let (Some(a), Some(b)) = (anchor(world, v.a), anchor(world, v.b)) else { continue };
        let (pa, pb) = (cam.tile_to_screen(a), cam.tile_to_screen(b));
        // Heavier feuds draw bolder (the weights at the last midnight).
        let w = ((v.w[0] + v.w[1]) / 2.0).clamp(0.0, 1.0);
        draw_line(pa.x, pa.y, pb.x, pb.y, 1.0 + 2.0 * w, Color { a: 0.35 + 0.5 * w, ..CRIMSON });
        draw_circle(pa.x, pa.y, 4.0, CRIMSON);
        draw_circle(pb.x, pb.y, 4.0, CRIMSON);
        let mid = (pa + pb) / 2.0;
        let text = format!("{}:{}", v.kills[0], v.kills[1]);
        draw_text(&text, mid.x + 4.0, mid.y - 4.0, 16.0, CRIMSON);
    }
    for &hunter in world.hunts.keys() {
        let staking = world
            .comp::<citysim::Brain>(hunter)
            .and_then(|b| b.current_step())
            .is_some_and(|s| s.action == citysim::ActionKind::StakeOut);
        if !staking {
            continue;
        }
        let Some(pos) = world.comp::<Position>(hunter) else { continue };
        let p = cam.tile_to_screen(vec2(f32::from(pos.tile.x) + 0.5, f32::from(pos.tile.y) + 0.5));
        let r = (ppt * 0.9).max(6.0);
        // An eye: a ring, a pupil, a bar above.
        draw_circle_lines(p.x, p.y, r, 2.0, CRIMSON);
        draw_circle(p.x, p.y, r * 0.35, CRIMSON);
        draw_line(p.x - r, p.y - r * 1.3, p.x + r, p.y - r * 1.3, 2.0, CRIMSON);
    }
}
