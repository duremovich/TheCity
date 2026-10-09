//! M16a § 9 (plan C42), the contracts overlay (`C`; Shift+C opens the
//! Board panel): a glyph on each Fixer sized by its record book, and a
//! crosshair on each live mission's door (a thinner one where a solo
//! taker's chase has intel). A contract is a record with a price between
//! fictional agents; the overlay reads state only.

use macroquad::prelude::*;

use citysim::contract::Broker;
use citysim::{Building, BuildingKind, TilePos, World};

use crate::App;

const AMBER: Color = Color::new(0xE0 as f32 / 255.0, 0xA0 as f32 / 255.0, 0x30 as f32 / 255.0, 1.0);
const RED: Color = Color::new(0.86, 0.24, 0.24, 1.0);

fn label(p: Vec2, text: &str, colour: Color) {
    let dims = measure_text(text, None, 16, 1.0);
    draw_rectangle(
        p.x - 2.0,
        p.y - dims.offset_y - 2.0,
        dims.width + 4.0,
        dims.height + 4.0,
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    draw_text(text, p.x, p.y, 16.0, colour);
}

fn crosshair(p: Vec2, r: f32, thick: f32, colour: Color) {
    draw_circle_lines(p.x, p.y, r, thick, colour);
    draw_line(p.x - r * 1.5, p.y, p.x - r * 0.4, p.y, thick, colour);
    draw_line(p.x + r * 0.4, p.y, p.x + r * 1.5, p.y, thick, colour);
    draw_line(p.x, p.y - r * 1.5, p.x, p.y - r * 0.4, thick, colour);
    draw_line(p.x, p.y + r * 0.4, p.x, p.y + r * 1.5, thick, colour);
}

fn centre(tile: TilePos) -> Vec2 {
    vec2(f32::from(tile.x) + 0.5, f32::from(tile.y) + 0.5)
}

/// Over everything but the night tint.
pub fn draw(world: &World, app: &App) {
    let cam = &app.camera;
    let ppt = cam.px_per_tile;
    // Each Fixer: an amber ring sized by its book (open and taken records
    // it brokered), grey while the law has it closed, and the counts.
    for &f in world.buildings_of_kind(BuildingKind::Fixer) {
        let (Some(b), Some(k)) = (world.comp::<Building>(f), world.comp::<Broker>(f)) else { continue };
        if b.demolished {
            continue;
        }
        let live = k.book.iter().filter(|id| world.contracts.get(id).is_some_and(|c| c.is_live())).count();
        let open = citysim::systems::contracts::fixer_open(world, f);
        let colour = if open { AMBER } else { Color::new(0.6, 0.6, 0.6, 1.0) };
        let c = vec2(f32::from(b.rect.x) + f32::from(b.rect.w) / 2.0, f32::from(b.rect.y) + f32::from(b.rect.h) / 2.0);
        let p = cam.tile_to_screen(c);
        let r = (ppt * (1.0 + (live as f32).sqrt() * 0.6)).clamp(6.0, 60.0);
        draw_circle(p.x, p.y, r, Color { a: 0.25, ..colour });
        draw_circle_lines(p.x, p.y, r, 2.0, colour);
        let state = if open { String::new() } else { " closed".to_string() };
        let text = format!("X {live} · {} reg · heat {:.2}{state}", k.regulars.len(), k.heat);
        label(vec2(p.x + r + 3.0, p.y), &text, colour);
    }
    // Each live mission: a bold crosshair on its door, a line from the muster.
    for (id, m) in &world.missions {
        let door = cam.tile_to_screen(centre(m.door));
        let muster = cam.tile_to_screen(centre(m.muster));
        draw_line(muster.x, muster.y, door.x, door.y, 1.5, Color { a: 0.5, ..RED });
        crosshair(door, (ppt * 1.2).max(7.0), 2.5, RED);
        let kind = world.contracts.get(id).map_or("?", |c| c.kind.label());
        label(vec2(door.x + 10.0, door.y - 10.0), &format!("#{id} {kind} · crew {}", m.crew.len()), RED);
    }
    // Each solo run with intel: a thin crosshair where the taker will look.
    for r in world.contract_runs.values() {
        let Some(intel) = r.intel else { continue };
        let p = cam.tile_to_screen(centre(intel.tile));
        crosshair(p, (ppt * 0.8).max(5.0), 1.5, Color { a: 0.8, ..AMBER });
    }
}
