//! M12 District panel (right, read-only in phase 1): name and zone,
//! population and class mix, happiness, coverage, fear, crime rate, the
//! controller with its share and top three presences, and the daily trace.

use egui_macroquad::egui::{self, Color32, RichText, Ui};

use citysim::{Controller, DistrictId, World};

use crate::App;

/// A controller's name, coloured like its map outline where it has one.
pub fn controller_text(world: &World, c: Controller) -> RichText {
    let label = citysim::systems::districts::controller_label(world, c);
    match c {
        Controller::Gang(g) => RichText::new(label).color(crate::ui::gang_colour(world.gang_index(g))),
        Controller::Corp(k) => RichText::new(label).color(crate::ui::corp_colour(world.corp_index(k))),
        Controller::City => RichText::new(label).color(Color32::from_rgb(200, 192, 176)),
        Controller::Contested => RichText::new(label).italics(),
    }
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World, d: DistrictId) {
    let dist = world.district(d);
    ui.horizontal(|ui| {
        ui.heading(&dist.name);
        if ui.small_button("x").clicked() {
            app.selected_district = None;
        }
    });
    ui.label(format!(
        "{} zone · {} buildings · {} Blocks · {} street tiles",
        dist.zone,
        dist.buildings.len(),
        dist.homes.len(),
        dist.walk_tiles
    ));
    ui.separator();
    egui::Grid::new("district_numbers").striped(true).show(ui, |ui| {
        ui.label("Population");
        ui.label(format!("{} ({} adults)", dist.population, dist.adults));
        ui.end_row();
        ui.label("Classes");
        ui.label(format!("Corp {} · Street {} · Dreg {}", dist.classes[0], dist.classes[1], dist.classes[2]));
        ui.end_row();
        ui.label("Happiness");
        ui.label(format!("{:.2}", dist.happiness));
        ui.end_row();
        ui.label("Coverage");
        ui.label(format!("{:.2}", dist.coverage));
        ui.end_row();
        ui.label("Fear");
        ui.label(format!("{:.2}", dist.fear));
        ui.end_row();
        ui.label("Crime rate");
        let crimes: u32 = dist.crimes.iter().map(|&c| u32::from(c)).sum();
        ui.label(format!("{:.2} /100/day ({crimes} in 7 days, {} today)", dist.crime_rate, dist.crimes_today));
        ui.end_row();
        ui.label("Controller");
        ui.horizontal(|ui| {
            ui.label(controller_text(world, dist.control));
            ui.label(format!("{:.2} since day {}", dist.control_share, citysim::time::day(dist.control_since)));
        });
        ui.end_row();
    });
    ui.separator();
    ui.strong("Presence");
    let pres = citysim::systems::districts::presence(world, d);
    let total: f32 = pres.iter().map(|&(_, v)| v).sum();
    egui::Grid::new("district_presence").striped(true).show(ui, |ui| {
        for &(c, v) in pres.iter().take(3) {
            ui.label(controller_text(world, c));
            ui.label(format!("{v:.1}"));
            ui.label(format!("{:.2}", if total > 0.0 { v / total } else { 0.0 }));
            ui.end_row();
        }
    });
    ui.separator();
    ui.strong("Trace (last midnight)");
    egui::Grid::new("district_trace").striped(true).show(ui, |ui| {
        for &(name, v) in &dist.trace {
            ui.label(name);
            ui.label(format!("{v:.3}"));
            ui.end_row();
        }
    });
}
