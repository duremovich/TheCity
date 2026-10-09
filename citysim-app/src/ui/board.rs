//! M16a § 9 (plan C42): the Board panel (Shift+C) and the contract rows the
//! Building, Inspector, Mission and City panels share. A contract is a
//! record in `World::contracts` (buyer, kind, target, price, deadline,
//! broker, taker, status), matched by a score and resolved by a seeded dice
//! roll between fictional agents; the panel reads the records and changes
//! nothing.

use egui_macroquad::egui::{self, Color32, RichText, Ui};

use citysim::contract::{Contract, ContractId, ContractStatus};
use citysim::systems::contracts;
use citysim::{time, EntityId, World, TICKS_PER_DAY};

use crate::App;

/// L2's amber (`ui/log.rs`): the contract events' colour.
pub const AMBER: Color32 = Color32::from_rgb(0xE0, 0xA0, 0x30);
const GREEN: Color32 = Color32::from_rgb(80, 170, 90);
const GREY: Color32 = Color32::from_rgb(150, 150, 150);

/// Whose board the panel shows: god's (every record) or the selected
/// agent's (`contracts::visible`: the M18 quest log).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum BoardView {
    #[default]
    God,
    Selected,
}

/// The buyer as `viewer` may name it: "anonymous" unless god (`None`) or
/// the viewer is in `known_by` (or is the buyer or the placing agent).
pub fn buyer_label(world: &World, c: &Contract, viewer: Option<EntityId>) -> String {
    let named = match viewer {
        None => true,
        Some(v) => c.known_by.contains(&v) || c.buyer == Some(v) || c.agent == Some(v),
    };
    if !named {
        return "anonymous".to_string();
    }
    match c.buyer {
        None => "the city".to_string(),
        b => world.owner_label(b),
    }
}

/// The render tag's colour.
pub fn tag_colour(tag: &str) -> Color32 {
    match tag {
        "LIVE" => GREEN,
        "QUEUED" => AMBER,
        _ => GREY,
    }
}

/// Days (one decimal) from now to `t`, signed.
fn days_to(world: &World, t: u64) -> String {
    let d = (t as f64 - world.tick as f64) / TICKS_PER_DAY as f64;
    format!("{d:+.1} d")
}

/// One record as a single line (the panels' shared row).
pub fn line(world: &World, c: &Contract, viewer: Option<EntityId>) -> String {
    let taker = match c.taker {
        Some(t) => {
            let crew = if c.crew.is_empty() { String::new() } else { format!(" +{}", c.crew.len()) };
            format!(" · taker {}{crew}", world.owner_label(Some(t)))
        }
        None => String::new(),
    };
    let broker = c.broker.map_or(" · direct".to_string(), |b| format!(" · via {}", world.name_of(b)));
    format!(
        "#{} {} on {} · {} · {}¢ · {}{broker}{taker} · deadline {}",
        c.id,
        c.kind.label(),
        world.name_of(c.target.id()),
        buyer_label(world, c, viewer),
        c.price,
        c.status.label(),
        days_to(world, c.deadline)
    )
}

/// A record's row: the render tag, then the line (a click on the line
/// opens its Mission panel when it marches as a mission).
pub fn row(ui: &mut Ui, app: &mut App, world: &World, c: &Contract, viewer: Option<EntityId>) {
    ui.horizontal_wrapped(|ui| {
        let tag = contracts::render_tag(world, c);
        ui.colored_label(tag_colour(tag), RichText::new(tag).monospace().small());
        if world.missions.contains_key(&c.id) {
            if ui.link(RichText::new(line(world, c, viewer)).small()).clicked() {
                open_mission(app, c.id);
            }
        } else {
            ui.label(RichText::new(line(world, c, viewer)).small());
        }
    });
}

/// Show a contract mission in the right panel.
pub fn open_mission(app: &mut App, id: ContractId) {
    app.selected_mission = None;
    app.selected_run = None;
    app.selected_contract = Some(id);
    app.follow = false;
}

/// The Board panel: a window over the map (Shift+C).
pub fn window(ctx: &egui::Context, app: &mut App, world: &World) {
    let mut open = app.show_board;
    egui::Window::new("Contract board")
        .open(&mut open)
        .default_pos(egui::pos2(320.0, 60.0))
        .default_size(egui::vec2(560.0, 420.0))
        .show(ctx, |ui| draw(ui, app, world));
    app.show_board = open && app.show_board;
}

fn draw(ui: &mut Ui, app: &mut App, world: &World) {
    let selected = app.selected.filter(|&s| world.has::<citysim::Identity>(s) && world.is_alive(s));
    ui.horizontal(|ui| {
        ui.label("View");
        ui.selectable_value(&mut app.board_view, BoardView::God, "god");
        let label = selected.map_or("selected agent (none)".to_string(), |s| world.name_of(s));
        ui.add_enabled_ui(selected.is_some(), |ui| {
            ui.selectable_value(&mut app.board_view, BoardView::Selected, label);
        });
    });
    let viewer = match app.board_view {
        BoardView::God => None,
        BoardView::Selected => selected,
    };
    if app.board_view == BoardView::Selected && viewer.is_none() {
        ui.small("select an agent to see the board as it sees it");
        return;
    }
    let ids: Vec<ContractId> = match viewer {
        None => world.contracts.values().filter(|c| c.is_live()).map(|c| c.id).collect(),
        Some(v) => {
            // `visible` covers the Open records it could take; its own
            // records (bought, taken, crewed) are on its board too.
            let mut ids = contracts::visible(world, v);
            for (&id, c) in &world.contracts {
                if c.is_live()
                    && (c.buyer == Some(v) || c.agent == Some(v) || c.taker == Some(v) || c.crew.contains(&v))
                {
                    ids.push(id);
                }
            }
            ids.sort_unstable();
            ids.dedup();
            ids
        }
    };
    let open = ids.iter().filter(|id| world.contracts.get(id).is_some_and(|c| c.is_open())).count();
    ui.label(format!("{} records: {open} open, {} taken", ids.len(), ids.len() - open));
    egui::ScrollArea::vertical().id_salt("board_rows").max_height(260.0).show(ui, |ui| {
        egui::Grid::new("board").striped(true).show(ui, |ui| {
            for h in ["", "#", "kind", "buyer", "target", "price", "broker", "deadline", "taker", "status"] {
                ui.strong(h);
            }
            ui.end_row();
            for id in &ids {
                let Some(c) = world.contracts.get(id) else { continue };
                let tag = contracts::render_tag(world, c);
                ui.colored_label(tag_colour(tag), RichText::new(tag).monospace().small());
                if world.missions.contains_key(id) {
                    if ui.link(format!("{id}")).clicked() {
                        open_mission(app, *id);
                    }
                } else {
                    ui.label(format!("{id}"));
                }
                ui.label(c.kind.label());
                ui.label(buyer_label(world, c, viewer));
                if ui.link(world.name_of(c.target.id())).clicked() {
                    app.selected = Some(c.target.id());
                    app.selected_contract = None;
                }
                ui.label(format!("{}¢", c.price));
                ui.label(c.broker.map_or("direct".to_string(), |b| world.name_of(b)));
                ui.label(days_to(world, c.deadline));
                match c.taker {
                    Some(t) if world.has::<citysim::Identity>(t) => {
                        if ui.link(world.name_of(t)).clicked() {
                            app.selected = Some(t);
                            app.selected_contract = None;
                        }
                    }
                    Some(t) => {
                        ui.label(world.owner_label(Some(t)));
                    }
                    None => {
                        ui.label("-");
                    }
                }
                let status = if c.status == ContractStatus::Taken && !c.crew.is_empty() {
                    format!("taken, crew {}", c.crew.len())
                } else {
                    c.status.label().to_string()
                };
                ui.label(status);
                ui.end_row();
            }
        });
    });
    ui.separator();
    ui.strong("History");
    let rows: Vec<&(u64, ContractId, String)> = world
        .contract_log
        .iter()
        .rev()
        .filter(|(_, id, _)| viewer.is_none() || ids.contains(id) || *id == 0)
        .take(40)
        .collect();
    egui::ScrollArea::vertical().id_salt("board_log").max_height(140.0).show(ui, |ui| {
        if rows.is_empty() {
            ui.small("nothing yet");
        }
        for (t, id, text) in rows {
            let head = format!("{} {} #{id}", time::day(*t), time::clock(*t));
            ui.label(RichText::new(format!("{head} | {text}")).monospace().small().color(AMBER));
        }
    });
}
