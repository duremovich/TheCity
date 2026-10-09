//! Mission panel (right, M14 § 7): a gang raid. With a streamer (a member
//! with a deck jacked in on `Overwatch`, `Gang.stream_by`) the panel is
//! tagged LIVE and follows the march and each pairing as it resolves; with
//! none it lists only the outcome events. The panel reads a `MissionView`
//! built from sim state each frame (`crate::mission`); it changes no roll.

use egui_macroquad::egui::{self, Color32, RichText, Ui};

use citysim::{time, Brain, EntityId, EventKind, Gang, World};

use crate::mission::{self, MissionView};
use crate::App;

const GREEN: Color32 = Color32::from_rgb(80, 170, 90);
const GOLD: Color32 = Color32::from_rgb(217, 162, 61);
const RED: Color32 = Color32::from_rgb(220, 60, 60);

fn go_agent(app: &mut App, who: EntityId) {
    app.selected_mission = None;
    app.selected_run = None;
    app.selected = Some(who);
    app.follow = false;
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World, gang: EntityId) {
    let Some(g) = world.comp::<Gang>(gang) else {
        ui.label("This gang is gone.");
        app.selected_mission = None;
        return;
    };
    let view = mission::build(world, gang);
    let colour = crate::ui::gang_colour(world.gang_index(gang));
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading(RichText::new(format!("{} raid", g.name)).color(colour));
            if view.is_some() {
                ui.colored_label(GREEN, RichText::new("LIVE").strong());
            }
        });
        let target = mission::target(world, gang).map_or("no target".to_string(), |b| world.name_of(b));
        ui.label(format!("{} · target {target}", g.order));
        match (g.raid_at, g.last_raid_tick) {
            (Some(t), _) if t > world.tick => ui.label(format!(
                "musters to leave in {} ticks (day {} {})",
                t - world.tick,
                time::day(t),
                time::clock(t)
            )),
            (Some(t), _) => ui.label(format!("left at day {} {}", time::day(t), time::clock(t))),
            (None, Some(t)) => ui.label(format!("last raid left at day {} {}", time::day(t), time::clock(t))),
            (None, None) => ui.label("no raid yet"),
        };

        ui.separator();
        match &view {
            Some(v) => live_block(ui, app, world, v),
            None => {
                ui.colored_label(GOLD, "No streamer: only the outcome events show.");
                if g.stream_by.is_some() {
                    ui.small("(a streamer is named but is not seated on an Overwatch run)");
                }
            }
        }

        ui.separator();
        ui.strong(if view.is_some() { "Each pairing as it resolves (LIVE) and the outcome" } else { "Outcome events" });
        let since = g.raid_at.or(g.last_raid_tick).unwrap_or(0).saturating_sub(120);
        let rows = world
            .events
            .iter()
            .rev()
            .take_while(|e| e.tick >= since)
            .filter(|e| {
                matches!(
                    e.kind,
                    EventKind::Raid
                        | EventKind::Assault
                        | EventKind::Murder
                        | EventKind::Sacked
                        | EventKind::DoorHacked
                        | EventKind::RobotTurned
                        | EventKind::Blinded
                        | EventKind::Death
                )
            })
            .filter(|e| e.actors.iter().any(|&a| a == gang || g.members.contains(&a)) || e.text.contains(&g.name))
            .take(40)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            ui.small("nothing yet");
        }
        for e in rows {
            let text = format!("{} {} | {:?} | {}", time::day(e.tick), time::clock(e.tick), e.kind, e.text);
            ui.label(RichText::new(text).monospace().small());
        }
        if ui.button("Close").clicked() {
            app.selected_mission = None;
        }
    });
}

/// The LIVE half: the streamer, the marchers with where they are, and the
/// pairings since the raid left.
fn live_block(ui: &mut Ui, app: &mut App, world: &World, v: &MissionView) {
    ui.horizontal(|ui| {
        ui.label("Streamed by");
        if ui.link(world.name_of(v.source)).clicked() {
            go_agent(app, v.source);
        }
    });
    ui.strong(format!("March ({})", v.subjects.len()));
    egui::Grid::new("mission_march").striped(true).show(ui, |ui| {
        for &m in &v.subjects {
            if ui.link(world.name_of(m)).clicked() {
                go_agent(app, m);
            }
            ui.label(mission::whereabouts(world, m));
            let step = world
                .comp::<Brain>(m)
                .and_then(|b| b.current_step().map(|s| format!("{:?}", s.action)))
                .unwrap_or_default();
            ui.small(step);
            ui.end_row();
        }
    });
    if v.subjects.is_empty() {
        ui.small("nobody is marching yet");
    }
    let dead = v.subjects.iter().filter(|&&m| !world.is_alive(m)).count();
    if dead > 0 {
        ui.colored_label(RED, format!("{dead} of the march are dead"));
    }
}

/// M16a § 9 (plan C42): a contract mission (a squad's or a gang's job,
/// marched by the raid machinery): the record, the crew and where each
/// stands, the door, the last strike decision's terms (`StrikeTerms`) and
/// the brawl's result lines. The M14 stream for a mission is deferred
/// (`Mission.stream_by` stays `None` in 16a): no streamed view.
pub fn draw_contract(ui: &mut Ui, app: &mut App, world: &World, id: citysim::contract::ContractId) {
    let Some(c) = world.contracts.get(&id) else {
        ui.label("This contract is gone.");
        app.selected_contract = None;
        return;
    };
    let m = world.missions.get(&id);
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading(RichText::new(format!("Contract #{id}: {}", c.kind.label())).color(crate::ui::board::AMBER));
            let tag = citysim::systems::contracts::render_tag(world, c);
            ui.colored_label(crate::ui::board::tag_colour(tag), RichText::new(tag).strong());
        });
        ui.label(crate::ui::board::line(world, c, None));
        ui.horizontal(|ui| {
            ui.label("Target");
            if ui.link(world.name_of(c.target.id())).clicked() {
                go_contract_agent(app, c.target.id());
            }
        });
        match m {
            Some(m) => {
                let when = if m.raid_at > world.tick {
                    format!(
                        "musters to leave in {} ticks (day {} {})",
                        m.raid_at - world.tick,
                        time::day(m.raid_at),
                        time::clock(m.raid_at)
                    )
                } else {
                    format!("left at day {} {}", time::day(m.raid_at), time::clock(m.raid_at))
                };
                ui.label(when);
                let door = m.door_building.map_or(format!("({}, {})", m.door.x, m.door.y), |b| world.name_of(b));
                ui.label(format!(
                    "door {door} · muster ({}, {}) · defenders expected {}",
                    m.muster.x, m.muster.y, m.defenders_hint
                ));
                ui.separator();
                ui.strong(format!("Crew ({})", m.crew.len()));
                egui::Grid::new("contract_crew").striped(true).show(ui, |ui| {
                    for &a in &m.crew {
                        if ui.link(world.name_of(a)).clicked() {
                            go_contract_agent(app, a);
                        }
                        ui.label(mission::whereabouts(world, a));
                        let step = world
                            .comp::<Brain>(a)
                            .and_then(|b| b.current_step().map(|s| format!("{:?}", s.action)))
                            .unwrap_or_default();
                        ui.small(step);
                        ui.end_row();
                    }
                });
            }
            None => {
                ui.colored_label(GOLD, "Not marching (a solo run, queued, or closed).");
            }
        }
        ui.separator();
        ui.strong("Strike decision");
        match c.strike {
            Some(s) => {
                let controller = match s.controller {
                    0 => "contested",
                    1 => "the city",
                    2 => "a gang",
                    3 => "a corp",
                    _ => "?",
                };
                let colour = match s.decision {
                    citysim::contract::Decision::Strike => GREEN,
                    citysim::contract::Decision::Hold => GOLD,
                    _ => RED,
                };
                ui.colored_label(colour, format!("{:?}", s.decision));
                ui.label(format!(
                    "p_win {:.2} · door held by {controller} · political cost {:.2} · EV {:.2} · holds {}",
                    s.p_win, s.political, s.ev, c.holds
                ));
            }
            None => {
                ui.small("no decision yet");
            }
        }
        ui.separator();
        ui.strong("Result lines");
        let crew: Vec<EntityId> = m.map(|m| m.crew.to_vec()).unwrap_or_default();
        let since = c.taken.unwrap_or(c.posted);
        let tag = format!("#{id}");
        let rows = world
            .events
            .iter()
            .rev()
            .take_while(|e| e.tick >= since)
            .filter(|e| {
                matches!(
                    e.kind,
                    EventKind::Assault
                        | EventKind::Murder
                        | EventKind::Death
                        | EventKind::Raid
                        | EventKind::StrikeDeclined
                        | EventKind::SoldOut
                        | EventKind::ContractTaken
                        | EventKind::ContractFulfilled
                        | EventKind::ContractFailed
                        | EventKind::ContractExpired
                )
            })
            .filter(|e| {
                e.actors.iter().any(|a| crew.contains(a) || *a == c.target.id() || Some(*a) == c.taker)
                    || e.text.contains(&tag)
            })
            .take(40)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            ui.small("nothing yet");
        }
        for e in rows {
            let text = format!("{} {} | {:?} | {}", time::day(e.tick), time::clock(e.tick), e.kind, e.text);
            ui.label(RichText::new(text).monospace().small());
        }
        let log: Vec<&(u64, citysim::contract::ContractId, String)> =
            world.contract_log.iter().filter(|(_, x, _)| *x == id).collect();
        if !log.is_empty() {
            ui.small("Record log");
            for (t, _, text) in log {
                ui.label(RichText::new(format!("{} {} | {text}", time::day(*t), time::clock(*t))).monospace().small());
            }
        }
        if ui.button("Close").clicked() {
            app.selected_contract = None;
        }
    });
}

fn go_contract_agent(app: &mut App, who: EntityId) {
    app.selected_contract = None;
    go_agent(app, who);
}
