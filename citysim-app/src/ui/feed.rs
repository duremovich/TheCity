//! M15 § 10: the Feed panel: reach, covered districts, the day's stories
//! with their slant and payer, the buried list. Reads state only.

use egui_macroquad::egui::{self, Color32, Ui};

use citysim::systems::{grudges, news};
use citysim::{time, EntityId, World};

use super::word::{link, section, CRIMSON};
use crate::App;

/// The Feed panel (spec § 10): reach, covered districts, today's stories
/// with slant and payer, the buried list.
pub fn draw(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let Some(st) = news::feed_state(world, id) else {
        section(ui, "Feed", |ui| {
            ui.label("not open yet (named at the next midnight)");
        });
        return;
    };
    section(ui, &st.name, |ui| {
        let licensed = news::feeds(world).contains(&id);
        ui.horizontal(|ui| {
            ui.label(format!("reach {:.2}", st.reach));
            if !licensed {
                ui.colored_label(CRIMSON, "unlicensed: silent, no ads");
            }
        });
        let covered: Vec<&str> = world
            .districts
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < 16 && st.covers & (1u16 << i) != 0)
            .map(|(_, d)| d.name.as_str())
            .collect();
        ui.label(format!(
            "covers {}",
            if covered.len() == world.districts.len() { "every district".to_string() } else { covered.join(", ") }
        ));
        ui.label(format!("{} stories today · {} buried", st.stories_today, st.buried.len()));
        for &(a, until) in &st.buried {
            ui.horizontal(|ui| {
                ui.small("buried:");
                link(ui, app, world, a);
                ui.small(format!("until day {}", time::day(until)));
            });
        }
        let last = world.stories.iter().rev().find(|s| s.feed == id).map(|s| time::day(s.tick));
        let mine: Vec<&citysim::word::Story> =
            world.stories.iter().filter(|s| s.feed == id && Some(time::day(s.tick)) == last).collect();
        if mine.is_empty() {
            ui.label("no stories run yet");
        } else {
            ui.small(format!("Stories of day {}", last.unwrap_or(0)));
            egui::Grid::new(format!("feed-stories-{}", id.index)).striped(true).show(ui, |ui| {
                ui.strong("who");
                ui.strong("deed");
                ui.strong("slant");
                ui.strong("paid by");
                ui.end_row();
                for s in mine {
                    link(ui, app, world, s.actor);
                    let obj = s.object.map_or_else(String::new, |o| format!(" {}", grudges::label(world, o)));
                    ui.label(format!("{}{obj}", s.deed.label()));
                    let c = if s.slant < 0.0 { CRIMSON } else { Color32::LIGHT_GRAY };
                    ui.colored_label(c, format!("{:+.1}", s.slant));
                    ui.label(s.paid_by.map_or_else(|| "-".to_string(), |p| world.owner_label(Some(p))));
                    ui.end_row();
                }
            });
        }
    });
}
