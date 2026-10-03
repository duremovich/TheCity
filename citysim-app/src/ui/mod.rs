//! HUD and panels. M0 draws the top bar with macroquad text; the egui
//! inspector, city panel and event log arrive with M2.

use macroquad::prelude::*;

use citysim::{time, BuildingKind, World};

use crate::App;

pub const HUD_H: f32 = 28.0;
const C_PAUSED: Color = Color::new(1.0, 0.6, 0.1, 1.0);

pub fn draw(app: &mut App, world: &World) {
    draw_rectangle(0.0, 0.0, screen_width(), HUD_H, Color::new(0.0, 0.0, 0.0, 0.7));

    let food: u32 = world
        .with::<citysim::Building>()
        .into_iter()
        .filter_map(|id| world.comp::<citysim::Building>(id))
        .filter(|b| matches!(b.kind, BuildingKind::Market | BuildingKind::Warehouse | BuildingKind::Home))
        .map(|b| b.stock_food)
        .sum();
    let price = world.market().map_or(0, |m| m.price_food);
    let treasury = world.treasury().map_or(0, |t| t.coins);
    let text = format!(
        "Day {} · {} · {} {} · {}x · Pop {} · Food {} · Price {} · Treasury {}",
        world.day(),
        world.season(),
        world.phase(),
        time::clock(world.tick),
        app.speed.multiplier(),
        world.population(),
        food,
        price,
        treasury,
    );
    draw_text(&text, 10.0, 19.0, 18.0, WHITE);

    let mut right = screen_width() - 10.0;
    if app.paused {
        let dims = measure_text("PAUSED", None, 18, 1.0);
        right -= dims.width;
        draw_text("PAUSED", right, 19.0, 18.0, C_PAUSED);
        right -= 16.0;
    }
    if app.show_fps {
        let fps = format!("{:.0} fps", app.fps_avg);
        let dims = measure_text(&fps, None, 18, 1.0);
        right -= dims.width;
        draw_text(&fps, right, 19.0, 18.0, WHITE);
    }

    if get_time() < app.status_until {
        draw_rectangle(0.0, HUD_H, screen_width(), 22.0, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_text(&app.status, 10.0, HUD_H + 16.0, 16.0, WHITE);
    }

    if let Some(sel) = app.selected {
        let line = format!("Selected {} ({sel})", world.name_of(sel));
        draw_text(&line, 10.0, screen_height() - 10.0, 16.0, WHITE);
    }
}
