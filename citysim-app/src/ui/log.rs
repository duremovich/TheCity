//! Event log panel (bottom): newest first, `day hh:mm | kind | text`, a kind
//! multi-select and an "Only selected" filter. Clicking a row selects the
//! first actor (an off-screen crime's unknown assailant, `EntityId::NONE`, is
//! skipped: the victim is selected) and centres the camera.

use std::collections::BTreeSet;

use egui_macroquad::egui::{self, Color32, Ui};

use citysim::{time, Brain, Building, EntityId, EventKind, Lod, Position, World};

use crate::App;

#[derive(Default)]
pub struct LogState {
    /// Kinds hidden by the filter; empty means all shown.
    pub hidden: BTreeSet<EventKind>,
    pub only_selected: bool,
    pub show_filter: bool,
    /// List open holes (crimes with no actor yet) instead of events.
    pub unattributed: bool,
}

/// L2 (plan L35): the living city's events.
pub const AMBER: Color32 = Color32::from_rgb(0xE0, 0xA0, 0x30);

fn kind_colour(kind: EventKind) -> Color32 {
    match kind {
        EventKind::Theft
        | EventKind::Extortion
        | EventKind::Assault
        | EventKind::Murder
        | EventKind::Robbed
        | EventKind::Assaulted => Color32::from_rgb(217, 47, 47),
        EventKind::Arrest | EventKind::Sentence | EventKind::Report | EventKind::Witness | EventKind::Attributed => {
            Color32::WHITE
        }
        EventKind::Death | EventKind::Starving | EventKind::Rotted => Color32::from_rgb(240, 140, 30),
        EventKind::Birth | EventKind::Marriage | EventKind::Proposal => Color32::from_rgb(80, 170, 90),
        EventKind::PlayerAction | EventKind::PlayerActionFailed => Color32::from_rgb(255, 215, 0),
        EventKind::Posture | EventKind::Bribe => Color32::from_rgb(120, 170, 220),
        // M11: rent trouble red, the rest of the corp world orange.
        EventKind::Evicted | EventKind::RentShort => Color32::from_rgb(217, 47, 47),
        EventKind::Housed
        | EventKind::Founded
        | EventKind::Incorporated
        | EventKind::Bankrupt
        | EventKind::Acquired
        | EventKind::BrokenUp
        | EventKind::CorpOrder
        | EventKind::Strike
        | EventKind::Contract => Color32::from_rgb(0xff, 0x9a, 0x3c),
        EventKind::OrderChanged
        | EventKind::TerritoryFlipped
        | EventKind::Raid
        | EventKind::Disobeyed
        | EventKind::Sacked
        | EventKind::Jailbreak
        | EventKind::GangJoin
        | EventKind::GangLeave
        | EventKind::Betrayal => Color32::from_rgb(142, 68, 173),
        // M12 D44: districts, the street and riots in amber.
        EventKind::DistrictControl
        | EventKind::Stance
        | EventKind::Sanitation
        | EventKind::Vagrancy
        | EventKind::Squatted
        | EventKind::SquatEvicted
        | EventKind::Derelict
        | EventKind::Riot
        | EventKind::Looted
        | EventKind::Crossfire
        | EventKind::Split => Color32::from_rgb(0xe0, 0xa0, 0x30),
        // M13 D47: assets in cyan.
        EventKind::AssetBought
        | EventKind::Repossessed
        | EventKind::Wrecked
        | EventKind::VehicleStolen
        | EventKind::Chopped
        | EventKind::Crash
        | EventKind::Installed
        | EventKind::Episode
        | EventKind::Treated
        | EventKind::Abducted
        | EventKind::Harvested
        | EventKind::Stripped
        | EventKind::Overdose => Color32::from_rgb(0x40, 0xc8, 0xe0),
        // M14 § 10: the Virt plane in violet.
        k if is_virt(k) => super::run::VIOLET,
        // M15 W43: the word in crimson.
        EventKind::Poached
        | EventKind::TalentLost
        | EventKind::Expelled
        | EventKind::Refused
        | EventKind::GrudgeFormed
        | EventKind::HuntStarted
        | EventKind::HuntAbandoned
        | EventKind::Avenged
        | EventKind::Vendetta
        | EventKind::VendettaEnded
        | EventKind::ContractLost
        | EventKind::Deceived
        | EventKind::Story
        | EventKind::Planted
        | EventKind::Buried => super::word::CRIMSON,
        // L2 (plan L35): every L2 kind in amber.
        EventKind::Refit
        | EventKind::Exported
        | EventKind::WorksPosted
        | EventKind::Gambled
        | EventKind::Bout
        | EventKind::Collected
        | EventKind::Preached
        | EventKind::Called => AMBER,
        _ => Color32::LIGHT_GRAY,
    }
}

/// M14's event kinds (the Virt plane): drawn violet.
fn is_virt(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::JackedIn
            | EventKind::DataStolen
            | EventKind::DataWiped
            | EventKind::DataSold
            | EventKind::LedgerHacked
            | EventKind::DoorHacked
            | EventKind::RobotTurned
            | EventKind::Blinded
            | EventKind::Traced
            | EventKind::Fried
            | EventKind::Flatlined
            | EventKind::Dumpshock
            | EventKind::IceRaised
            | EventKind::IceLowered
            | EventKind::TechGained
            | EventKind::TechLost
    )
}

/// The Virt events whose first actor is a runner: a click opens its Run panel.
fn opens_run(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::JackedIn
            | EventKind::DataStolen
            | EventKind::DataWiped
            | EventKind::LedgerHacked
            | EventKind::DoorHacked
            | EventKind::RobotTurned
            | EventKind::Blinded
            | EventKind::Traced
            | EventKind::Fried
            | EventKind::Flatlined
            | EventKind::Dumpshock
    )
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World) {
    ui.horizontal(|ui| {
        ui.strong("Events");
        ui.label(format!("{}", world.events.len()));
        ui.checkbox(&mut app.log.only_selected, "Only selected");
        ui.checkbox(&mut app.log.unattributed, format!("Unattributed ({})", world.holes.len()));
        if ui.selectable_label(app.log.show_filter, "Kinds…").clicked() {
            app.log.show_filter = !app.log.show_filter;
        }
        if !app.log.hidden.is_empty() && ui.button("Show all").clicked() {
            app.log.hidden.clear();
        }
    });
    if app.log.show_filter {
        ui.horizontal_wrapped(|ui| {
            for kind in EventKind::ALL {
                let shown = !app.log.hidden.contains(&kind);
                if ui.selectable_label(shown, format!("{kind:?}")).clicked() {
                    if shown {
                        app.log.hidden.insert(kind);
                    } else {
                        app.log.hidden.remove(&kind);
                    }
                }
            }
        });
    }
    ui.separator();

    let selected = app.selected;
    let only_selected = app.log.only_selected;
    let mut clicked = None;
    let mut clicked_run = None;
    if app.log.unattributed {
        // Open holes, newest first: click to bind the hole and select the victim.
        let mut bind = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for h in world.holes.values().rev().take(400) {
                let line = format!(
                    "{:>3} {} | {:<9} | {} | {}",
                    time::day(h.tick),
                    time::clock(h.tick),
                    format!("{:?}", h.kind),
                    world.name_of(h.victim),
                    world.district_name(h.district)
                );
                let text = egui::RichText::new(line).monospace().color(Color32::from_rgb(217, 47, 47));
                if ui.selectable_label(selected == Some(h.victim), text).clicked() {
                    bind = Some((h.id, h.victim));
                }
            }
        });
        if let Some((hole, victim)) = bind {
            app.cmds.push(citysim::PlayerCommand::Bind(hole));
            clicked = Some(victim);
        }
    } else {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let rows = world
                .events
                .iter()
                .rev()
                .filter(|e| !app.log.hidden.contains(&e.kind))
                .filter(|e| !only_selected || selected.is_some_and(|s| e.actors.contains(&s)))
                .take(400);
            for e in rows {
                let line = format!(
                    "{:>3} {} | {:<18} | {}",
                    time::day(e.tick),
                    time::clock(e.tick),
                    format!("{:?}", e.kind),
                    e.text
                );
                let highlight = selected.is_some_and(|s| e.actors.contains(&s));
                let text = egui::RichText::new(line).monospace().color(kind_colour(e.kind));
                if ui.selectable_label(highlight, text).clicked() {
                    clicked = e.actors.iter().copied().find(|&a| a != EntityId::NONE);
                    clicked_run = if opens_run(e.kind) {
                        e.actors.iter().find_map(|&a| super::run::run_of(world, a))
                    } else {
                        None
                    };
                }
            }
        });
    }

    if let Some(id) = clicked {
        app.selected = Some(id);
        app.selected_run = clicked_run;
        app.selected_node = None;
        app.selected_mission = None;
        app.follow = false;
        // Centre on the agent, or on its building if it is not drawn as a square.
        if let Some(p) = world.comp::<Position>(id) {
            let full = world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Full);
            let tile = match (full, p.building.and_then(|b| world.comp::<Building>(b))) {
                (false, Some(b)) => b.door,
                _ => p.tile,
            };
            app.camera.centre_on(tile);
        }
    }
}
