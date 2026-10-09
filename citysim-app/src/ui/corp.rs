//! Corp panel (right, M11 § 9): a selected corp entity. Niches with their
//! price level, share and monopoly flag; governance and the outside parent;
//! the exec; treasury, a 14-day cashflow sparkline and the bankruptcy
//! countdown; the order, since when, and the top three of the order trace;
//! buildings, employees and contracts; Subsidise and BreakUp.

use egui_macroquad::egui::{self, Color32, RichText, Ui};

use citysim::{
    time, Building, BuildingKind, Corp, EntityId, Governance, Job, Niche, PlayerCommand, World, TICKS_PER_DAY,
};

use crate::App;

const RED: Color32 = Color32::from_rgb(220, 60, 60);
const GREEN: Color32 = Color32::from_rgb(80, 170, 90);
const GOLD: Color32 = Color32::from_rgb(217, 162, 61);

fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title).default_open(true).show(ui, body);
}

fn select(app: &mut App, id: EntityId) {
    app.selected = Some(id);
    app.follow = false;
}

/// "dictatorship" or "board of N" (D49).
pub fn governance_label(g: &Governance) -> String {
    match g {
        Governance::Dictator => "dictatorship".to_string(),
        Governance::Board { members } => format!("board of {}", members.len()),
    }
}

/// Days until bankruptcy for a corp in the red, `None` when solvent.
pub fn bankruptcy_countdown(world: &World, c: &Corp) -> Option<f32> {
    let since = c.negative_since?;
    let limit = world.config.corps.bankrupt_days as f32;
    Some((limit - world.tick.saturating_sub(since) as f32 / TICKS_PER_DAY as f32).max(0.0))
}

/// Agents employed at any of the corp's buildings.
fn employees(world: &World, c: &Corp) -> usize {
    world
        .with::<Job>()
        .into_iter()
        .filter(|&a| {
            world.comp::<Job>(a).and_then(|j| j.employer).is_some_and(|e| c.buildings.binary_search(&e).is_ok())
        })
        .count()
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let Some(c) = world.comp::<Corp>(id) else {
        ui.label("This corp is gone.");
        app.selected = None;
        return;
    };
    let colour = crate::ui::corp_colour(world.corp_index(id));
    let now = world.tick;
    egui::ScrollArea::vertical().show(ui, |ui| {
        section(ui, "Corp", |ui| {
            ui.heading(RichText::new(&c.name).color(colour));
            let niches: Vec<&str> = c.niches.iter().map(|n| n.label()).collect();
            let slot = c.slot.map_or("unslotted".to_string(), |s| format!("slot {}", s + 1));
            ui.label(format!("{} · {} · {slot}", niches.join(" + "), governance_label(&c.governance)));
            if c.parent.is_some() {
                ui.colored_label(GOLD, format!("branch of an outside parent ({}¢ abroad)", c.outside_treasury));
            }
            ui.horizontal(|ui| {
                ui.label("Exec");
                match c.exec {
                    Some(e) if world.is_alive(e) => {
                        if ui.link(world.name_of(e)).clicked() {
                            select(app, e);
                        }
                        if let Some(p) = world.comp::<citysim::Personality>(e) {
                            ui.small(format!("greed {:.2} · lawful {:.2}", p.greed, p.lawfulness));
                        }
                    }
                    _ => {
                        ui.colored_label(RED, "none (the brain reads 0.5)");
                    }
                }
            });
        });

        section(ui, "Money", |ui| {
            let tc = if c.treasury < 0 { RED } else { GREEN };
            ui.horizontal(|ui| {
                ui.label("Treasury");
                ui.colored_label(tc, format!("{}¢", c.treasury));
                ui.small(format!("(closed last midnight at {}¢)", c.closing));
            });
            let fortnight: i64 = c.cashflow.iter().sum();
            ui.label(format!("Cashflow {fortnight:+}¢ over {} days", c.cashflow.len()));
            cashflow_sparkline(ui, c);
            if let Some(left) = bankruptcy_countdown(world, c) {
                ui.colored_label(RED, format!("In the red: bankrupt in {left:.1} days"));
            }
            if let Some(t) = c.upkeep_grace_until.filter(|&t| t > now) {
                ui.label(format!("no upkeep until day {}", time::day(t)));
            }
            if let Some(t) = c.lobby_until.filter(|&t| t > now) {
                ui.label(format!("lobbying the Law until day {}", time::day(t)));
            }
        });

        section(ui, "Niches", |ui| {
            egui::Grid::new("corp_niches").striped(true).show(ui, |ui| {
                ui.strong("niche");
                ui.strong("price");
                ui.strong("share");
                ui.strong("");
                ui.end_row();
                for &n in &c.niches {
                    let shares = citysim::systems::corp_brain::shares(world, n);
                    let share = shares.get(&id).copied().unwrap_or(0.0);
                    ui.label(n.label());
                    ui.label(format!("×{:.2}", c.level(n)));
                    ui.label(format!("{:.0}%", share * 100.0));
                    if share >= citysim::systems::corps::MONOPOLY_SHARE {
                        ui.colored_label(RED, "MONOPOLY");
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        });

        section(ui, "Order", |ui| {
            let niche = c.order_niche.map_or(String::new(), |n| format!(" in {n}"));
            let held = now.saturating_sub(c.order_since) as f32 / TICKS_PER_DAY as f32;
            ui.colored_label(
                colour,
                format!("{}{niche} since day {} ({held:.1} d)", c.order, time::day(c.order_since)),
            );
            if let Some(t) = c.pinned_until.filter(|&t| t > now) {
                ui.colored_label(GOLD, format!("pinned by the player until day {}", time::day(t)));
            }
            if c.order_trace.is_empty() {
                ui.label("no rescoring yet");
            }
            for s in c.order_trace.iter().take(3) {
                let current = s.order == c.order && Some(s.niche) == c.order_niche;
                egui::CollapsingHeader::new(format!("{} in {}  {:.3}", s.order, s.niche, s.score))
                    .id_salt(format!("corp-trace-{}-{}", s.order, s.niche))
                    .default_open(current)
                    .show(ui, |ui| {
                        egui::Grid::new(format!("corp-cons-{}-{}", s.order, s.niche)).striped(true).show(ui, |ui| {
                            for k in &s.considerations {
                                ui.label(k.name.as_ref());
                                ui.label(format!("{:.3}", k.input));
                                ui.label("->");
                                ui.label(format!("{:.3}", k.output));
                                ui.end_row();
                            }
                        });
                    });
            }
        });

        tech_block(ui, app, world, id, c);
        // M15 § 10: the axes, competence, regard, vendettas, plants and buries.
        super::word::faction(ui, app, world, id);

        let mut by_kind: Vec<(BuildingKind, Vec<EntityId>)> = Vec::new();
        for &b in &c.buildings {
            let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { continue };
            match by_kind.iter_mut().find(|(k, _)| *k == kind) {
                Some((_, v)) => v.push(b),
                None => by_kind.push((kind, vec![b])),
            }
        }
        section(ui, &format!("Buildings ({}) · {} employees", c.buildings.len(), employees(world, c)), |ui| {
            for (kind, list) in &by_kind {
                egui::CollapsingHeader::new(format!("{} ({})", kind.label(), list.len()))
                    .id_salt(format!("corp-b-{kind:?}"))
                    .default_open(list.len() <= 8)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for &b in list {
                                if ui.link(format!("#{}", b.index)).clicked() {
                                    select(app, b);
                                }
                            }
                        });
                    });
            }
        });

        if c.niches.contains(&Niche::Security) || !c.contracts.is_empty() {
            section(ui, &format!("Contracts ({})", c.contracts.len()), |ui| {
                if c.contracts.is_empty() {
                    ui.label("no clients");
                }
                egui::Grid::new("corp_contracts").striped(true).show(ui, |ui| {
                    for &(client, until) in &c.contracts {
                        if ui.link(world.name_of(client)).clicked() {
                            select(app, client);
                        }
                        ui.label(world.owner_label(world.owner_of(client)));
                        ui.label(format!("until day {}", time::day(until)));
                        ui.end_row();
                    }
                });
            });
        }

        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::DragValue::new(&mut app.city.subsidise_amount).range(1..=50_000).suffix("¢"));
            if ui.button("Subsidise").clicked() {
                app.cmds.push(PlayerCommand::Subsidise { corp: id, amount: app.city.subsidise_amount });
            }
            let monopoly = c.niches.iter().any(|&n| citysim::systems::corps::is_monopoly(world, id, n));
            if ui.add_enabled(monopoly, egui::Button::new("Break up")).on_disabled_hover_text("no monopoly").clicked() {
                app.cmds.push(PlayerCommand::BreakUp(id));
            }
            if ui.button("Close").clicked() {
                app.selected = None;
            }
        });
    });
}

/// Net coins per day over the last 14 days, a zero line, red below.
fn cashflow_sparkline(ui: &mut Ui, c: &Corp) {
    let skip = c.cashflow.len().saturating_sub(14);
    let flows: Vec<f32> = c.cashflow.iter().skip(skip).map(|&v| v as f32).collect();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(320.0, 44.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_gray(30));
    if flows.is_empty() {
        return;
    }
    let max = flows.iter().fold(1.0f32, |m, v| m.max(v.abs()));
    let mid = rect.center().y;
    let half = rect.height() / 2.0 - 2.0;
    painter.line_segment(
        [egui::pos2(rect.left(), mid), egui::pos2(rect.right(), mid)],
        egui::Stroke::new(1.0_f32, Color32::from_gray(80)),
    );
    let bar_w = rect.width() / 14.0;
    for (i, &v) in flows.iter().enumerate() {
        let x = rect.left() + (i + 14 - flows.len()) as f32 * bar_w;
        let h = v / max * half;
        let (top, bottom) = if v >= 0.0 { (mid - h, mid) } else { (mid, mid - h) };
        let fill = if v >= 0.0 { GREEN } else { RED };
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x + 1.0, top), egui::pos2(x + bar_w - 1.0, bottom)),
            0.0,
            fill,
        );
    }
    ui.small(format!("14 days · max |{max:.0}|¢/day"));
}

/// M14 § 10: the Tech block: per track the tier, the progress bar toward the
/// next tier's `tier_cost`, the days of lapse and the Data held; the focus,
/// the Labs with their stores, and 30 days of ICE spend.
fn tech_block(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, c: &Corp) {
    use citysim::systems::virt;
    use citysim::virt::Track;
    if c.tech.is_unset() {
        return;
    }
    section(ui, "Tech", |ui| {
        let cost = &world.config.tech.tier_cost;
        egui::Grid::new("corp_tech").striped(true).show(ui, |ui| {
            ui.strong("track");
            ui.strong("tier");
            ui.strong("toward next");
            ui.strong("lapse");
            ui.strong("Data held");
            ui.end_row();
            for t in Track::ALL {
                let tier = c.tech.tier_of(t);
                let focus = if c.tech.focus == t { " (focus)" } else { "" };
                ui.label(format!("{}{focus}", t.label()));
                ui.label(format!("T{tier}"));
                match cost.get(usize::from(tier) + 1).copied().filter(|_| tier < 3) {
                    Some(need) if need > 0 => {
                        let have = c.tech.progress[t.index()];
                        ui.add(
                            egui::ProgressBar::new((have as f32 / need as f32).clamp(0.0, 1.0))
                                .text(format!("{have}/{need}"))
                                .desired_width(120.0),
                        );
                    }
                    _ => {
                        ui.label("max");
                    }
                }
                let lapse = c.tech.lapse[t.index()];
                if lapse > 0 {
                    ui.colored_label(RED, format!("{lapse} d"));
                } else {
                    ui.label("-");
                }
                ui.label(format!("{}", virt::holding(world, id, t)));
                ui.end_row();
            }
        });
        let labs: Vec<EntityId> = c
            .buildings
            .iter()
            .copied()
            .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lab && !bd.demolished))
            .collect();
        ui.label(format!("Labs ({})", labs.len()));
        for b in labs {
            let Some(bd) = world.comp::<Building>(b) else { continue };
            let held = virt::node_of_building(world, b).and_then(|n| world.virt.node(n)).map_or(0, |n| n.store.total());
            ui.horizontal(|ui| {
                if ui.link(format!("Lab#{}", b.index)).clicked() {
                    select(app, b);
                }
                ui.small(format!(
                    "focus {} · {held} Data",
                    bd.focus.map_or("none".to_string(), |t| t.label().to_string())
                ));
            });
        }
        let spend: i64 = c.ice_spend.iter().sum::<i64>() + c.ice_spend_today;
        ui.label(format!("ICE spend, last {} days: {spend}¢", c.ice_spend.len() + 1));
        ice_sparkline(ui, c);
    });
}

/// Coins spent on ICE per day over the window, bars from a zero line.
fn ice_sparkline(ui: &mut Ui, c: &Corp) {
    let days: Vec<f32> = c.ice_spend.iter().map(|&v| v as f32).collect();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(320.0, 30.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_gray(30));
    if days.is_empty() {
        return;
    }
    let max = days.iter().fold(1.0f32, |m, v| m.max(*v));
    let bar_w = rect.width() / days.len().max(30) as f32;
    for (i, &v) in days.iter().enumerate() {
        let x = rect.left() + i as f32 * bar_w;
        let h = v / max * (rect.height() - 2.0);
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(x + 0.5, rect.bottom() - h),
                egui::pos2(x + bar_w - 0.5, rect.bottom()),
            ),
            0.0,
            Color32::from_rgb(180, 140, 255),
        );
    }
    ui.small(format!("max {max:.0}¢/day"));
}
