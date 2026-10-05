//! M12 District panel (right, read-only): name and zone, population and
//! class mix, happiness, coverage, fear, crime rate, the controller with its
//! share and top three presences, the daily trace and (phase 2) the law:
//! guards allocated and the weight's terms, the stance and its trace, rough
//! sleepers and Vagrancy in 14 days.

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
    law(ui, world, d);
    ui.separator();
    street(ui, world, d);
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

/// M12 phase 3: litter, the sweepers, and the street's rungs here.
fn street(ui: &mut Ui, world: &World, d: DistrictId) {
    let dist = world.district(d);
    ui.strong("The street");
    let band = citysim::systems::litter::band((dist.litter * 255.0).round().clamp(0.0, 255.0) as u8);
    let derelicts: Vec<citysim::EntityId> = citysim::systems::street::derelicts(world)
        .into_iter()
        .filter(|&b| world.district_of_building(b) == d)
        .collect();
    let squatters: usize = derelicts.iter().map(|&b| world.squatters_of(b).len()).sum();
    let hotels = world
        .buildings_of_kind(citysim::BuildingKind::Hotel)
        .iter()
        .filter(|&&h| citysim::systems::street::is_hotel(world, h) && world.district_of_building(h) == d)
        .count();
    egui::Grid::new("district_street").striped(true).show(ui, |ui| {
        ui.label("Litter");
        ui.label(format!("{:.3} ({})", dist.litter, citysim::systems::litter::band_label(band)));
        ui.end_row();
        ui.label("Sweepers");
        ui.label(format!("{}", dist.sweepers));
        ui.end_row();
        ui.label("Rough sleepers");
        ui.label(format!("{} last night", dist.rough));
        ui.end_row();
        ui.label("Derelicts");
        ui.label(format!("{} ({squatters} squatters)", derelicts.len()));
        ui.end_row();
        ui.label("Hotels");
        ui.label(format!("{hotels}"));
        ui.end_row();
    });
}

/// M12 phase 2: the district's law.
fn law(ui: &mut Ui, world: &World, d: DistrictId) {
    let dist = world.district(d);
    ui.strong("The law");
    let now = world.tick;
    let fortnight = 14 * citysim::TICKS_PER_DAY;
    let vagrancy = dist.vagrancy_log.iter().filter(|&&t| now.saturating_sub(t) < fortnight).count();
    egui::Grid::new("district_law").striped(true).show(ui, |ui| {
        ui.label("Guards allocated");
        ui.label(format!("{}", dist.guards));
        ui.end_row();
        ui.label("Stance");
        ui.label(format!(
            "{} since day {}",
            citysim::systems::law_brain::stance_label(world, dist.stance),
            citysim::time::day(dist.stance_since)
        ));
        ui.end_row();
        ui.label("Rough sleepers");
        ui.label(format!("{} last night", dist.rough));
        ui.end_row();
        ui.label("Vagrancy (14 d)");
        ui.label(format!("{vagrancy} fined or jailed"));
        ui.end_row();
        if let Some((g, n)) = citysim::systems::law_brain::top_gang(world, d) {
            ui.label("Most reported");
            ui.label(format!(
                "{} ({n} reports)",
                world.comp::<citysim::Gang>(g).map_or_else(|| world.name_of(g), |x| x.name.clone())
            ));
            ui.end_row();
        }
    });
    egui::CollapsingHeader::new("Allocation weight").default_open(false).show(ui, |ui| {
        egui::Grid::new("district_alloc").striped(true).show(ui, |ui| {
            for &(name, v) in &dist.alloc_trace {
                ui.label(name);
                ui.label(format!("{v:.3}"));
                ui.end_row();
            }
        });
    });
    egui::CollapsingHeader::new("Stance trace").default_open(false).show(ui, |ui| {
        if dist.stance_trace.is_empty() {
            ui.label("not scored (uninhabited, pinned, Garrison or no captain)");
        }
        for (k, s) in dist.stance_trace.iter().take(3).enumerate() {
            let label = citysim::systems::law_brain::stance_label(world, s.stance);
            egui::CollapsingHeader::new(format!("{label}  {:.3}", s.score)).id_salt(("stance", d.0, k)).show(
                ui,
                |ui| {
                    egui::Grid::new(("stance_cs", d.0, k)).striped(true).show(ui, |ui| {
                        for c in &s.considerations {
                            ui.label(c.name.as_ref());
                            ui.label(format!("{:.3}", c.input));
                            ui.label("->");
                            ui.label(format!("{:.3}", c.output));
                            ui.end_row();
                        }
                    });
                },
            );
        }
    });
}
