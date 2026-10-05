//! HUD and panels. The top bar is macroquad text; the inspector (right) and
//! the event log (bottom) and the city panel (left) are egui.

pub mod building;
pub mod city;
pub mod corp;
pub mod district;
pub mod inspector;
pub mod log;

use macroquad::prelude::*;

use citysim::{time, BuildingKind, World};

use crate::App;

pub const HUD_H: f32 = 28.0;

/// Gang colours by index in `World::gangs()`, as `0xRRGGBB`; the map and
/// the panels both read this table.
pub const GANG_COLOURS: [u32; 3] = [0x8e44ad, 0x1abc9c, 0xe67e22];

pub fn gang_hex(index: usize) -> u32 {
    GANG_COLOURS[index % GANG_COLOURS.len()]
}

/// A gang's panel colour.
pub fn gang_colour(index: usize) -> egui_macroquad::egui::Color32 {
    let c = gang_hex(index);
    egui_macroquad::egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// M11 D43: corp colours by index in `World::corps()` (ascending id).
pub const CORP_COLOURS: [u32; 12] = [
    0xe6194b, 0x3cb44b, 0xffe119, 0x4363d8, 0xf58231, 0x911eb4, 0x46f0f0, 0xf032e6, 0xbcf60c, 0xfabebe, 0x008080,
    0xe6beff,
];

pub fn corp_hex(index: usize) -> u32 {
    CORP_COLOURS[index % CORP_COLOURS.len()]
}

/// A corp's panel colour.
pub fn corp_colour(index: usize) -> egui_macroquad::egui::Color32 {
    let c = corp_hex(index);
    egui_macroquad::egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}
pub const INSPECTOR_W: f32 = 360.0;
pub const LOG_H: f32 = 220.0;
const C_PAUSED: Color = Color::new(1.0, 0.6, 0.1, 1.0);

pub fn draw(app: &mut App, world: &World) {
    draw_hud(app, world);

    egui_macroquad::ui(|ctx| {
        egui_macroquad::egui::SidePanel::left("city")
            .exact_width(city::CITY_W)
            .show(ctx, |ui| city::draw(ui, app, world));
        if let Some(sel) = app.selected {
            egui_macroquad::egui::SidePanel::right("inspector").exact_width(INSPECTOR_W).show(ctx, |ui| {
                if world.has::<citysim::Building>(sel) {
                    building::draw(ui, app, world, sel);
                } else if world.has::<citysim::Corp>(sel) {
                    corp::draw(ui, app, world, sel);
                } else {
                    inspector::draw(ui, app, world);
                }
            });
        } else if let Some(d) = app.selected_district.filter(|d| d.index() < world.districts.len()) {
            egui_macroquad::egui::SidePanel::right("inspector")
                .exact_width(INSPECTOR_W)
                .show(ctx, |ui| district::draw(ui, app, world, d));
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
    let price = world.mean_price();
    let treasury = world.treasury().map_or(0, |t| t.coins);
    let text = format!(
        "Day {} · {} · {} {} · {}x · Pop {} · Food {} · Price {}¢ · Treasury {}¢",
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
