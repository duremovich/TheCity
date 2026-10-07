//! Run panel (right, M14 § 10): one run on the Virt plane, a short chain of
//! seeded dice contests between a fictional runner (deck tier plus skill)
//! and the ICE of abstract nodes. The runner, deck, patron and purpose; the
//! route with each contest's odds and result; the payload and the outcome.
//! LIVE while the run is in `world.runs` (a run panel is always live: the
//! route and contests fill in as they resolve); a finished run is read from
//! the log of the last 64.

use egui_macroquad::egui::{self, Color32, RichText, Ui};

use citysim::systems::{security, virt};
use citysim::virt::{Purpose, Run, RunId, RunMode, RunOutcome, RunPhase, RunWhy, Track};
use citysim::{time, EntityId, World};

use crate::App;

const RED: Color32 = Color32::from_rgb(220, 60, 60);
const GREEN: Color32 = Color32::from_rgb(80, 170, 90);
const GOLD: Color32 = Color32::from_rgb(217, 162, 61);
pub const VIOLET: Color32 = Color32::from_rgb(180, 140, 255);

/// "a wipe", "Data", "a ledger"...
pub fn purpose_label(world: &World, p: Purpose) -> String {
    match p {
        Purpose::Data { wipe: true } => "wipe Data".to_string(),
        Purpose::Data { wipe: false } => "steal Data".to_string(),
        Purpose::Ledger => "drain a Ledger".to_string(),
        Purpose::Door => "open the doors".to_string(),
        Purpose::Robot(a) => format!("turn {}", world.name_of(a)),
        Purpose::Camera(a) => format!("blind {}", world.name_of(a)),
        Purpose::Overwatch(g) => format!("overwatch for {}", world.owner_label(Some(g))),
    }
}

pub fn outcome_label(o: RunOutcome) -> (&'static str, Color32) {
    match o {
        RunOutcome::Success => ("success", GREEN),
        RunOutcome::Bounced => ("bounced", GOLD),
        RunOutcome::Traced => ("traced", RED),
        RunOutcome::Fried => ("fried", RED),
        RunOutcome::Flatlined => ("flatlined", RED),
        RunOutcome::Captured => ("captured", RED),
        RunOutcome::Dumped => ("dumped", GOLD),
    }
}

fn why_label(w: RunWhy) -> &'static str {
    match w {
        RunWhy::Freelance => "freelance",
        RunWhy::GangOrder => "gang order",
        RunWhy::CorpOrder => "corp order",
        RunWhy::Prelude => "raid prelude",
        RunWhy::Overwatch => "stream",
        RunWhy::Stat => "off-screen pass",
        RunWhy::God => "god order",
    }
}

fn phase_label(p: RunPhase) -> &'static str {
    match p {
        RunPhase::Hop => "hopping",
        RunPhase::BreakIn => "breaking in",
        RunPhase::Act => "acting",
        RunPhase::Extract => "extracting",
        RunPhase::Out => "jacking out",
    }
}

/// A run by id: live, else the newest copy in the log.
pub fn find(world: &World, id: RunId) -> Option<(&Run, bool)> {
    match world.runs.get(&id) {
        Some(r) => Some((r, true)),
        None => world.run_log.iter().rev().find(|r| r.id == id).map(|r| (r, false)),
    }
}

/// The run to show for an agent: its live run, else its newest logged one.
pub fn run_of(world: &World, agent: EntityId) -> Option<RunId> {
    world
        .runner_of
        .get(&agent)
        .copied()
        .or_else(|| world.run_log.iter().rev().find(|r| r.runner == agent).map(|r| r.id))
}

fn go_agent(app: &mut App, who: EntityId) {
    app.selected_run = None;
    app.selected_mission = None;
    app.selected = Some(who);
    app.follow = false;
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World, id: RunId) {
    let Some((r, live)) = find(world, id) else {
        ui.label("This run has left the log.");
        app.selected_run = None;
        return;
    };
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Run");
            if live {
                ui.colored_label(GREEN, RichText::new("LIVE").strong());
            } else if let Some(o) = r.outcome {
                let (t, c) = outcome_label(o);
                ui.colored_label(c, RichText::new(t.to_uppercase()).strong());
            }
        });
        ui.horizontal(|ui| {
            ui.label("Runner");
            if world.is_alive(r.runner) {
                if ui.link(world.name_of(r.runner)).clicked() {
                    go_agent(app, r.runner);
                }
            } else {
                ui.label(format!("{} (dead)", world.name_of(r.runner)));
            }
        });
        let deck_tier = world.comp::<citysim::Asset>(r.deck).map(|a| a.tier);
        let eff = match deck_tier {
            Some(t) if t != r.deck_eff => format!("tier {t}, effective {} (maker's tech caps it)", r.deck_eff),
            _ => format!("tier {}", r.deck_eff),
        };
        ui.label(format!("Deck: {eff} · attacker {:.2}", r.att));
        ui.horizontal(|ui| {
            ui.label("Patron");
            super::inspector::owner_link(ui, app, world, r.patron);
            if r.patron.is_none() {
                ui.small("(freelance)");
            }
        });
        ui.label(format!(
            "Purpose: {} · {} · {}",
            purpose_label(world, r.purpose),
            if r.mode == RunMode::Loud { "loud" } else { "quiet" },
            why_label(r.why)
        ));
        ui.label(format!("Chair: {} · target {}", world.name_of(r.chair), virt::node_label(world, r.target)));

        ui.separator();
        ui.strong("Route");
        route_table(ui, world, r, live);

        ui.separator();
        let held: Vec<String> = Track::ALL
            .iter()
            .filter(|t| r.payload[t.index()] > 0)
            .map(|t| format!("{} {}", r.payload[t.index()], t.label()))
            .collect();
        ui.label(format!("Payload: {}", if held.is_empty() { "none".to_string() } else { held.join(", ") }));
        match (live, r.outcome) {
            (true, _) => {
                let due = r.next_at.saturating_sub(world.tick);
                ui.colored_label(GREEN, format!("{} · next step in {due} ticks", phase_label(r.phase)));
            }
            (false, Some(o)) => {
                let (t, c) = outcome_label(o);
                ui.colored_label(c, format!("Outcome: {t}"));
            }
            _ => {}
        }
        if let Some(src) = r.source {
            ui.small(format!("took its payload from {}", virt::node_label(world, src)));
        }
        if let Purpose::Overwatch(g) = r.purpose {
            if live && ui.button("Open the raid view").clicked() {
                app.selected_run = None;
                app.selected = None;
                app.selected_mission = Some(g);
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Open agent").clicked() {
                go_agent(app, r.runner);
            }
            if ui.button("Close").clicked() {
                app.selected_run = None;
            }
        });
    });
}

/// Each node of the route: the contest's odds (`contest_p` at the node's
/// ICE now) and its result from `Run.log`; an unreached node shows its odds
/// only; `>` marks where a live run is.
fn route_table(ui: &mut Ui, world: &World, r: &Run, live: bool) {
    let step = world.config.chrome.contest_step;
    egui::Grid::new(("run_route", r.id)).striped(true).show(ui, |ui| {
        ui.strong("");
        ui.strong("node");
        ui.strong("ICE");
        ui.strong("p");
        ui.strong("result");
        ui.end_row();
        // A node is entered once; the target is met again at the extract,
        // so a node has one row per logged contest (one open row if none).
        let mut rows: Vec<(usize, Option<(u64, bool)>)> = Vec::new();
        for (i, &n) in r.route.iter().enumerate() {
            let before = rows.len();
            rows.extend(r.log.iter().filter(|&&(_, ln, _)| ln == n).map(|&(t, _, w)| (i, Some((t, w)))));
            if rows.len() == before {
                rows.push((i, None));
            }
        }
        for (i, res) in rows {
            let n = r.route[i];
            let ice = virt::ice_eff(world, n);
            let def = virt::def(world, n);
            // The patron's own nodes are never contested (a freelancer meets every owner's).
            let met = virt::contested(world, n, r.patron);
            let here = live && usize::from(r.at) == i && r.phase == RunPhase::Hop;
            ui.label(if here { ">" } else { "" });
            ui.label(virt::node_label(world, n));
            ui.label(ice.to_string());
            if def == 0 || !met {
                ui.label("-");
            } else {
                ui.label(format!("{:.2}", security::contest_p(r.att, f32::from(def), step)));
            }
            match res {
                Some((t, true)) => ui.colored_label(GREEN, format!("won {}", time::clock(t))),
                Some((t, false)) => ui.colored_label(RED, format!("LOST {}", time::clock(t))),
                None if def == 0 => ui.label("open"),
                None if !met => ui.label("own"),
                None => ui.label(""),
            };
            ui.end_row();
        }
    });
}
