//! Node panel (right, M14 § 10): one node of the Virt plane, opened from the
//! `N` overlay. Owner, ICE and who made it, arrears, alarm, the store per
//! track, the last breaches, a hack effect in place, the links, and the runs
//! now aimed at it.

use egui_macroquad::egui::{self, Color32, RichText, Ui};

use citysim::systems::virt;
use citysim::virt::{HackEffect, LinkKind, NodeId, NodeKind, Track};
use citysim::{time, World, TICKS_PER_DAY};

use crate::App;

const RED: Color32 = Color32::from_rgb(220, 60, 60);
const GREEN: Color32 = Color32::from_rgb(80, 170, 90);
const GOLD: Color32 = Color32::from_rgb(217, 162, 61);

fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title).default_open(true).show(ui, body);
}

/// A section whose title carries a live count: a fixed id keeps its collapse state.
fn section_id(ui: &mut Ui, id: &str, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title).id_salt(id).default_open(true).show(ui, body);
}

fn ago(world: &World, t: u64) -> String {
    let d = world.tick.saturating_sub(t) as f32 / TICKS_PER_DAY as f32;
    format!("{d:.1} days ago")
}

fn hack_label(world: &World, e: HackEffect) -> String {
    match e {
        HackEffect::DoorOpen => "doors open".to_string(),
        HackEffect::RobotTurned(g) => format!("robot turned for {}", world.owner_label(Some(g))),
        HackEffect::Blind => "cameras blind".to_string(),
    }
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World, n: NodeId) {
    let Some(node) = world.virt.node(n) else {
        ui.label("This node is gone.");
        app.selected_node = None;
        return;
    };
    let now = world.tick;
    egui::ScrollArea::vertical().show(ui, |ui| {
        section(ui, "Node", |ui| {
            ui.heading(virt::node_label(world, n));
            let kind = match node.kind {
                NodeKind::Public(_) => "Public: the street net of a district",
                NodeKind::Building(_) => "a building's node",
                NodeKind::Ledger(_) => "a Ledger: a treasury",
            };
            ui.label(format!("#{} · {kind}{}", n.0, if node.alive { "" } else { " · DEAD (its building is gone)" }));
            ui.horizontal(|ui| {
                ui.label("Owner");
                super::inspector::owner_link(ui, app, world, node.owner);
            });
            if let NodeKind::Building(b) = node.kind {
                ui.horizontal(|ui| {
                    ui.label("Building");
                    super::asset::holder_link(ui, app, world, b);
                });
            }
        });

        if let Some(p) = virt::profile(world, n) {
            section(ui, "ICE", |ui| {
                let eff = virt::ice_eff(world, n);
                let colour = crate::overlay::C_ICE[usize::from(eff.min(3))];
                let c = Color32::from_rgb((colour >> 16) as u8, (colour >> 8) as u8, colour as u8);
                ui.horizontal(|ui| {
                    ui.label("effective");
                    ui.colored_label(c, RichText::new(format!("{eff}")).strong());
                    ui.label(format!("(installed {}, a runner meets {})", p.ice, virt::def(world, n)));
                });
                match p.ice_maker {
                    None => ui.label("maker: the city's own (uncapped)"),
                    Some(m) => ui.label(format!(
                        "maker: {} (its Deck tier caps it at {})",
                        world.owner_label(Some(m)),
                        virt::maker_tier(world, m, Track::Deck)
                    )),
                };
                if p.ice_arrears > 0 {
                    let off = world.config.ice.ice_off_days;
                    ui.colored_label(RED, format!("upkeep {} days behind (ICE off at {off})", p.ice_arrears));
                } else {
                    ui.label("upkeep paid");
                }
                ui.label(format!(
                    "value at risk {}¢ · its owner's target tier {}",
                    virt::value_at_risk(world, n),
                    virt::ice_target(world, n)
                ));
            });
        }

        section(ui, "Alarm and effects", |ui| {
            match node.alarm_until.filter(|&t| t > now) {
                Some(t) => ui.colored_label(RED, format!("alarm: +1 defence for {} more ticks", t - now)),
                None => ui.label("no alarm"),
            };
            match node.hacked.filter(|&(_, t)| t > now) {
                Some((e, t)) => ui.colored_label(
                    GOLD,
                    format!("hacked: {} until day {} {}", hack_label(world, e), time::day(t), time::clock(t)),
                ),
                None => ui.label("no hack in place"),
            };
        });

        section_id(ui, "node-store", &format!("Store ({} Data)", node.store.total()), |ui| {
            egui::Grid::new("node_store").striped(true).show(ui, |ui| {
                for t in Track::ALL {
                    ui.label(t.label());
                    ui.label(format!("{}", node.store.get(t)));
                    ui.end_row();
                }
            });
            ui.small(format!("cap {} per track", world.config.data.store_cap));
        });

        section_id(ui, "node-breaches", &format!("Breaches ({})", node.breaches.len()), |ui| {
            if node.breaches.is_empty() {
                ui.label("none recorded");
            }
            for &t in node.breaches.iter().rev() {
                ui.label(format!("day {} {} · {}", time::day(t), time::clock(t), ago(world, t)));
            }
        });

        let live: Vec<_> = world.runs.values().filter(|r| r.target == n || r.route.contains(&n)).collect();
        section_id(ui, "node-runs", &format!("Runs through here ({})", live.len()), |ui| {
            for r in live {
                ui.horizontal(|ui| {
                    ui.colored_label(GREEN, "LIVE");
                    if ui.link(world.name_of(r.runner)).clicked() {
                        app.selected_node = None;
                        app.selected_run = Some(r.id);
                        app.selected = Some(r.runner);
                    }
                    ui.small(super::run::purpose_label(world, r.purpose));
                });
            }
        });

        let links: Vec<_> = world.virt.adj.get(n.index()).map(|a| a.to_vec()).unwrap_or_default();
        section_id(ui, "node-links", &format!("Links ({})", links.len()), |ui| {
            for (m, li) in links {
                let Some(l) = world.virt.links.get(usize::from(li)) else { continue };
                let kind = match l.kind {
                    LinkKind::Street => "street",
                    LinkKind::Access => "access",
                    LinkKind::Trunk => "trunk",
                };
                ui.horizontal(|ui| {
                    if ui.link(virt::node_label(world, m)).clicked() {
                        app.selected_node = Some(m);
                    }
                    let fw = if l.firewall > 0 { format!(" · firewall {}", l.firewall) } else { String::new() };
                    ui.small(format!("{kind} tier {}{fw}", l.tier));
                });
            }
        });

        if ui.button("Close").clicked() {
            app.selected_node = None;
        }
    });
}
