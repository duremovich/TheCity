//! HUD and panels. The top bar is macroquad text; the inspector (right) and
//! the event log (bottom) are egui. The city panel arrives with M7.

pub mod inspector;
pub mod log;

use macroquad::prelude::*;

use citysim::{time, BuildingKind, World};

use crate::App;

pub const HUD_H: f32 = 28.0;
pub const INSPECTOR_W: f32 = 360.0;
pub const LOG_H: f32 = 220.0;
const C_PAUSED: Color = Color::new(1.0, 0.6, 0.1, 1.0);

pub fn draw(app: &mut App, world: &World) {
    draw_hud(app, world);

    egui_macroquad::ui(|ctx| {
        if app.selected.is_some() {
            egui_macroquad::egui::SidePanel::right("inspector")
                .exact_width(INSPECTOR_W)
                .show(ctx, |ui| inspector::draw(ui, app, world));
        }
        egui_macroquad::egui::TopBottomPanel::bottom("log")
            .resizable(true)
            .default_height(LOG_H)
            .show(ctx, |ui| log::draw(ui, app, world));
        app.ui_hover = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
    });
    egui_macroquad::draw();
}

fn draw_hud(app: &App, world: &World) {
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
}
