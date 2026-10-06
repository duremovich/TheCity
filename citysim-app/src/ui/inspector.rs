//! Inspector panel (right, 360 px): everything about the selected agent,
//! in the spec's ten sections, all collapsible and open by default.

use egui_macroquad::egui::{self, Color32, ProgressBar, RichText, Ui};

use citysim::story::{self, Span};
use citysim::{
    time, Brain, Building, Corpse, EntityId, ExecState, Household, Identity, Inventory, Job, Memory, Mood, Needs,
    Personality, PlayerCommand, Position, Sentence, Wallet, World, TICKS_PER_DAY,
};

use crate::App;

const RED: Color32 = Color32::from_rgb(220, 60, 60);
const GREEN: Color32 = Color32::from_rgb(80, 170, 90);
const BLUE: Color32 = Color32::from_rgb(70, 120, 210);
const GOLD: Color32 = Color32::from_rgb(217, 162, 61);

/// Which half of the inspector is showing.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub enum InspectorTab {
    #[default]
    Now,
    Story,
}

fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title).default_open(true).show(ui, body);
}

fn bar(ui: &mut Ui, label: &str, value: f32, warn_below: Option<f32>) {
    let colour = match warn_below {
        Some(t) if value < t => RED,
        _ => BLUE,
    };
    ui.horizontal(|ui| {
        ui.label(format!("{label:<10}"));
        ui.add(ProgressBar::new(value.clamp(0.0, 1.0)).fill(colour).text(format!("{value:.2}")).desired_width(200.0));
    });
}

/// `-1..1` bar.
fn signed_bar(ui: &mut Ui, label: &str, value: f32) {
    let colour = if value < -0.3 {
        RED
    } else if value > 0.3 {
        GREEN
    } else {
        BLUE
    };
    ui.horizontal(|ui| {
        ui.label(format!("{label:<10}"));
        ui.add(ProgressBar::new((value + 1.0) / 2.0).fill(colour).text(format!("{value:+.2}")).desired_width(200.0));
    });
}

fn building_label(world: &World, b: EntityId) -> String {
    match world.comp::<Building>(b) {
        Some(bd) => format!("{}#{}", bd.kind.label(), b.index),
        None => format!("#{}", b.index),
    }
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World) {
    let Some(id) = app.selected else { return };
    if !world.is_alive(id) || !world.has::<Identity>(id) {
        ui.label("Selection is gone.");
        app.selected = None;
        return;
    }
    // Opening an agent binds their open victim holes, once per selection,
    // through the command so replays stay deterministic.
    if app.bound_for != Some(id) {
        app.bound_for = Some(id);
        if let Some(holes) = world.holes_by_agent.get(&id) {
            app.cmds.extend(holes.iter().map(|&h| PlayerCommand::Bind(h)));
        }
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        identity(ui, app, world, id);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.inspector_tab, InspectorTab::Now, "Now");
            ui.selectable_value(&mut app.inspector_tab, InspectorTab::Story, "Story");
        });
        if app.inspector_tab == InspectorTab::Story {
            story_tab(ui, app, world, id);
            return;
        }
        if let Some(n) = world.comp::<Needs>(id) {
            section(ui, "Needs", |ui| {
                bar(ui, "hunger", n.hunger, Some(0.25));
                bar(ui, "energy", n.energy, Some(0.25));
                bar(ui, "safety", n.safety, Some(0.25));
                bar(ui, "wealth", n.wealth, Some(0.25));
                bar(ui, "belonging", n.belonging, Some(0.25));
                bar(ui, "intimacy", n.intimacy, Some(0.25));
                if let Some(t) = n.starving_since {
                    ui.colored_label(
                        RED,
                        format!("starving for {:.1} days", (world.tick - t) as f32 / TICKS_PER_DAY as f32),
                    );
                }
            });
        }
        if let Some(p) = world.comp::<Personality>(id) {
            section(ui, "Personality", |ui| {
                bar(ui, "lawful", p.lawfulness, None);
                bar(ui, "greed", p.greed, None);
                bar(ui, "pride", p.pride, None);
                bar(ui, "sociable", p.sociability, None);
                bar(ui, "courage", p.courage, None);
                bar(ui, "loyalty", p.loyalty, None);
            });
        }
        if let (Some(m), Some(n), Some(mem)) =
            (world.comp::<Mood>(id), world.comp::<Needs>(id), world.comp::<Memory>(id))
        {
            section(ui, "Mood", |ui| {
                signed_bar(ui, "mood", m.value);
                ui.label(format!(
                    "need_term {:+.3}   memory_term {:+.3}",
                    citysim::mood::need_term(n),
                    citysim::mood::memory_term(mem, world.tick)
                ));
                if let Some(t) = m.low_since {
                    ui.colored_label(
                        RED,
                        format!("low for {:.1} days", (world.tick - t) as f32 / TICKS_PER_DAY as f32),
                    );
                }
            });
        }
        if let Some(b) = world.comp::<Brain>(id) {
            goal(ui, world, b);
            plan(ui, world, b);
        }
        if let Some(mem) = world.comp::<Memory>(id) {
            memories(ui, world, mem);
        }
        edges(ui, world, id);
        if let (Some(w), Some(inv)) = (world.comp::<Wallet>(id), world.comp::<Inventory>(id)) {
            section(ui, "Wallet and inventory", |ui| {
                let unpaid = world.comp::<Job>(id).map_or(0, |j| j.days_unpaid);
                ui.label(format!(
                    "{}¢ · {} food ({} stolen) · {unpaid} days unpaid",
                    w.coins, inv.food, inv.stolen_food
                ));
            });
        }
        buttons(ui, app, world, id);
    });
}

/// The biography: life events and folded trace runs, newest first. The same
/// for every tier; an unbound victim line offers "find out".
fn story_tab(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let lines = story::lines(world, id);
    if lines.is_empty() {
        ui.label("Nothing to tell yet.");
        return;
    }
    for line in lines {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(line.when()).weak().monospace());
            for span in &line.spans {
                match span {
                    Span::Text(t) if line.run => {
                        ui.label(RichText::new(t).italics().color(Color32::GRAY));
                    }
                    Span::Text(t) => {
                        ui.label(t);
                    }
                    Span::Agent(o, name) if world.is_alive(*o) && world.has::<Identity>(*o) => {
                        if ui.link(name).clicked() {
                            app.selected = Some(*o);
                        }
                    }
                    Span::Agent(_, name) => {
                        ui.label(name);
                    }
                }
            }
            if let Some(h) = line.find_out {
                if ui.small_button("find out").clicked() {
                    app.cmds.push(PlayerCommand::Bind(h));
                }
            }
        });
    }
}

fn identity(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let Some(ident) = world.comp::<Identity>(id) else { return };
    section(ui, "Identity", |ui| {
        ui.heading(&ident.name);
        let years = ident.age_years();
        let days = ident.age_days % citysim::time::DAYS_PER_YEAR as u32;
        ui.label(format!("{years} years {days} days · {:?} · id {id}", ident.sex));
        if let Some(b) = world.comp::<Brain>(id) {
            ui.horizontal(|ui| {
                ui.label(format!("LOD {:?}", b.lod));
                let mut pinned = b.pinned;
                if ui.checkbox(&mut pinned, "Pin").changed() {
                    app.cmds.push(PlayerCommand::Pin(id, pinned));
                }
            });
        }
        match world.comp::<Job>(id) {
            Some(j) => {
                let at = j.employer.map_or("nowhere".to_string(), |e| building_label(world, e));
                let captain = if world.law().is_some_and(|l| l.captain == Some(id)) { " · captain" } else { "" };
                ui.label(format!("{} @ {at} · wage {}¢/day{captain}", j.role.label(), j.wage_per_day));
            }
            None => {
                ui.label("No job");
            }
        }
        match world.comp::<Household>(id).and_then(|h| h.home) {
            Some(h) => ui.label(format!("Block {}", building_label(world, h))),
            // M12 phase 3: the street's rung, if any.
            None => match (
                world.comp::<citysim::Squatter>(id).map(|s| s.building),
                citysim::systems::street::booked_hotel(world, id),
            ) {
                (Some(b), _) => ui.colored_label(RED, format!("Homeless · squatting in {}", building_label(world, b))),
                (None, Some(h)) => ui.colored_label(RED, format!("Homeless · slept at {}", building_label(world, h))),
                (None, None) => ui.colored_label(RED, "Homeless"),
            },
        };
        // M12 § 8: the district they stand in (and live in, if elsewhere),
        // and their Vagrancy record from the report list.
        if let Some(p) = world.comp::<Position>(id) {
            let here = world.district_of(p.tile);
            let home = world.comp::<Household>(id).and_then(|h| h.home).map(|h| world.district_of_building(h));
            let text = match home {
                Some(h) if h != here => {
                    format!("In {} · lives in {}", world.district_name(here), world.district_name(h))
                }
                _ => format!("In {}", world.district_name(here)),
            };
            ui.label(text);
        }
        let vagrancy =
            world.crime_reports().iter().filter(|r| r.suspect == id && r.crime == citysim::Crime::Vagrancy).count();
        if vagrancy > 0 {
            ui.label(format!("Vagrancy reports: {vagrancy}"));
        }
        ownership(ui, app, world, id);
        ui.horizontal(|ui| {
            if let Some(s) = world.comp::<Sentence>(id) {
                let boss =
                    world.gang_of(id).and_then(|g| world.comp::<citysim::Gang>(g)).is_some_and(|g| g.boss == Some(id));
                let tag = if boss { " (the boss)" } else { "" };
                ui.colored_label(RED, format!("Jailed until day {}{tag}", time::day(s.until_tick)));
            }
            if let Some(gid) = world.gang_of(id) {
                let name = world.comp::<citysim::Gang>(gid).map_or("gang".to_string(), |g| g.name.clone());
                let following = world.comp::<Brain>(id).and_then(|b| b.following_order);
                let text = match following {
                    Some(o) => format!("{name} · following {o}"),
                    None => format!("{name} · freelancing"),
                };
                ui.colored_label(crate::ui::gang_colour(world.gang_index(gid)), text);
            }
            if world.has::<Corpse>(id) {
                ui.colored_label(Color32::GRAY, "Corpse");
            }
        });
    });
}

fn goal(ui: &mut Ui, world: &World, b: &Brain) {
    section(ui, "Goal", |ui| {
        let current = b.current_goal.map_or("none".to_string(), |g| format!("{g:?}"));
        let score = b
            .last_think
            .as_ref()
            .and_then(|t| t.goals.iter().find(|g| Some(g.goal) == b.current_goal))
            .map_or(String::new(), |g| format!(" ({:.3})", g.score));
        ui.label(RichText::new(format!("{current}{score}")).strong());
        let cooled: Vec<String> = b
            .cooldowns
            .iter()
            .filter(|(_, &until)| until > world.tick)
            .map(|(g, until)| format!("{g:?} {}t", until - world.tick))
            .collect();
        if !cooled.is_empty() {
            ui.label(format!("cooling: {}", cooled.join(", ")));
        }
        let Some(trace) = &b.last_think else {
            ui.label("no think yet");
            return;
        };
        ui.label(format!("last think {} ticks ago", world.tick.saturating_sub(trace.tick)));
        for g in &trace.goals {
            egui::CollapsingHeader::new(format!("{:?}  {:.3}  (raw {:.3})", g.goal, g.score, g.raw))
                .default_open(Some(g.goal) == b.current_goal)
                .show(ui, |ui| {
                    egui::Grid::new(format!("cons-{:?}", g.goal)).striped(true).show(ui, |ui| {
                        for c in &g.considerations {
                            ui.label(c.name.as_ref());
                            ui.label(format!("{:.3}", c.input));
                            ui.label("->");
                            let text = format!("{:.3}", c.output);
                            if c.output <= 0.0 {
                                ui.colored_label(RED, text);
                            } else {
                                ui.label(text);
                            }
                            ui.end_row();
                        }
                    });
                });
        }
    });
}

fn plan(ui: &mut Ui, world: &World, b: &Brain) {
    section(ui, "Plan", |ui| {
        let Some(p) = &b.plan else {
            ui.label("no plan");
            return;
        };
        ui.label(format!("{:?} · started {} ticks ago", p.goal, world.tick.saturating_sub(p.started_tick)));
        for (i, s) in p.steps.iter().enumerate() {
            let marker = if i == usize::from(b.plan_step) { ">" } else { " " };
            let target = s.target.map_or(String::new(), |t| format!("({})", world.name_of(t)));
            ui.monospace(format!("{marker} {:?}{target}", s.action));
        }
        let line = match &b.exec {
            ExecState::Idle => "Idle".to_string(),
            ExecState::Goto { target, path, .. } => {
                let left = if path.is_empty() { "flow field".to_string() } else { format!("{} tiles", path.len()) };
                format!("Goto {} {left}", target.tile)
            }
            ExecState::GotoTimed { target, arrive_tick, .. } => {
                format!("GotoTimed {} arrives in {}", target.tile, arrive_tick.saturating_sub(world.tick))
            }
            ExecState::Use { kind, until, .. } => format!("Use {kind:?} {} left", until.saturating_sub(world.tick)),
            ExecState::Wait { until } => format!("Wait {}", until.saturating_sub(world.tick)),
            ExecState::Fly { target, arrive_tick, .. } => {
                format!("Flying to {} lands in {}", target.tile, arrive_tick.saturating_sub(world.tick))
            }
        };
        ui.label(line);
    });
}

fn memories(ui: &mut Ui, world: &World, mem: &Memory) {
    section(ui, "Memories", |ui| {
        if mem.entries.is_empty() {
            ui.label("none");
            return;
        }
        let mut entries: Vec<_> = mem.entries.iter().collect();
        entries.sort_by(|a, b| b.salience.partial_cmp(&a.salience).unwrap_or(std::cmp::Ordering::Equal));
        egui::Grid::new("memories").striped(true).show(ui, |ui| {
            ui.strong("kind");
            ui.strong("subject");
            ui.strong("age");
            ui.strong("sal");
            ui.strong("val");
            ui.strong("2nd");
            ui.end_row();
            for e in entries {
                ui.label(format!("{:?}", e.kind));
                ui.label(e.subject.map_or("-".to_string(), |s| world.name_of(s)));
                ui.label(format!("{:.1}d", (world.tick.saturating_sub(e.tick)) as f32 / TICKS_PER_DAY as f32));
                ui.label(format!("{:.2}", e.salience));
                ui.label(format!("{:+.2}", e.valence));
                ui.label(if e.second_hand { "✓" } else { "" });
                ui.end_row();
            }
        });
    });
}

fn edges(ui: &mut Ui, world: &World, id: EntityId) {
    section(ui, "Edges", |ui| {
        // The agent's own edges, ascending by the other id (as the key-ordered
        // edge scan gave them, without walking every edge each frame).
        let mut rows: Vec<_> = world.neighbours(id).filter_map(|o| world.edge(id, o).map(|e| (o, e))).collect();
        if rows.is_empty() {
            ui.label("none");
            return;
        }
        rows.sort_by(|a, b| b.1.affinity.abs().partial_cmp(&a.1.affinity.abs()).unwrap_or(std::cmp::Ordering::Equal));
        egui::Grid::new("edges").striped(true).show(ui, |ui| {
            ui.strong("other");
            ui.strong("kind");
            ui.strong("aff");
            ui.strong("trust");
            ui.strong("debt");
            ui.end_row();
            for (other, e) in rows {
                ui.label(world.name_of(other));
                ui.label(format!("{:?}", e.kind));
                ui.label(format!("{:+.2}", e.affinity));
                ui.label(format!("{:.2}", e.trust));
                ui.label(format!("{}", e.debt));
                ui.end_row();
            }
        });
    });
}

fn buttons(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui.button("Grant 50").clicked() {
            app.cmds.push(PlayerCommand::GrantCoins { agent: id, amount: 50 });
        }
        if world.has::<Sentence>(id) {
            if ui.button("Release").clicked() {
                app.cmds.push(PlayerCommand::Release(id));
            }
        } else if ui.button("Arrest").clicked() {
            app.cmds.push(PlayerCommand::Arrest(id));
        }
        if ui.button("Centre camera").clicked() {
            if let Some(p) = world.comp::<Position>(id) {
                app.camera.centre_on(p.tile);
            }
        }
        let follow_label = if app.follow { "Following" } else { "Follow" };
        if ui.selectable_label(app.follow, follow_label).clicked() {
            app.follow = !app.follow;
        }
        if ui.button("Close").clicked() {
            app.selected = None;
            app.follow = false;
        }
    });
    ui.add_space(4.0);
    ui.small(RichText::new("Arrest and Release land with the law system (M4).").color(GOLD));
}

/// M11 § 9: class, the employer's owner, rent paid and arrears, what the
/// agent founded, and the corp they run.
fn ownership(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    if world.has::<citysim::Child>(id) {
        return;
    }
    let class = citysim::systems::classes::class_of(world, id);
    let colour = match class {
        citysim::Class::Corp => GOLD,
        citysim::Class::Street => BLUE,
        citysim::Class::Dreg => RED,
    };
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(colour, format!("{} class", class.label()));
        if let Some(e) = world.comp::<Job>(id).and_then(|j| j.employer) {
            let owner = world.owner_of(e);
            ui.label("· employer owned by");
            owner_link(ui, app, world, owner);
        }
    });
    if let Some(h) = world.comp::<Household>(id) {
        let paid = h.rent_paid_7d(world.day());
        if h.home.is_some() || paid > 0 || h.arrears > 0 {
            let rent = h.home.and_then(|b| world.comp::<Building>(b)).map_or(0, |b| b.rent_per_day);
            let line = format!("Rent {rent}¢/day · paid {paid}¢ in 7 days");
            if h.arrears > 0 {
                ui.colored_label(RED, format!("{line} · {} days in arrears", h.arrears));
            } else {
                ui.label(line);
            }
        }
        if let Some((by, t)) = h.evicted_by {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(RED, format!("evicted day {} by", time::day(t)));
                owner_link(ui, app, world, by);
            });
        }
    }
    // "founded X": the biography counts the foundings (a Life row names
    // agents only); the event ring still holding them names the buildings.
    let rows = world
        .life
        .get(id.index as usize)
        .and_then(|l| l.as_ref())
        .map_or(0, |l| l.events.iter().filter(|e| e.kind == citysim::LifeKind::Founded).count());
    if rows > 0 {
        let named: Vec<EntityId> = world
            .events
            .iter()
            .filter(|e| e.kind == citysim::EventKind::Founded && e.actors.first() == Some(&id))
            .filter_map(|e| e.actors.get(1).copied())
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label("founded");
            for &b in &named {
                if ui.link(building_label(world, b)).clicked() {
                    app.selected = Some(b);
                    app.follow = false;
                }
            }
            if rows > named.len() {
                ui.label(format!("{} more", rows - named.len()));
            }
        });
    }
    let owned: Vec<EntityId> = world
        .with::<Building>()
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(id) && !bd.demolished))
        .collect();
    if !owned.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label("owns");
            for b in owned {
                if ui.link(building_label(world, b)).clicked() {
                    app.selected = Some(b);
                    app.follow = false;
                }
            }
        });
    }
    for c in world.corps() {
        let Some(corp) = world.comp::<citysim::Corp>(c).filter(|cc| cc.exec == Some(id)) else { continue };
        ui.horizontal_wrapped(|ui| {
            ui.label("exec of");
            let name = RichText::new(&corp.name).color(crate::ui::corp_colour(world.corp_index(c)));
            if ui.link(name).clicked() {
                app.selected = Some(c);
                app.follow = false;
            }
        });
    }
}

/// A clickable owner: a corp opens its panel, a gang its Hideout, an agent
/// the inspector; the city is plain text.
fn owner_link(ui: &mut Ui, app: &mut App, world: &World, owner: Option<EntityId>) {
    let label = world.owner_label(owner);
    let Some(o) = owner else {
        ui.label(label);
        return;
    };
    let (text, target) = if world.has::<citysim::Corp>(o) {
        (RichText::new(label).color(crate::ui::corp_colour(world.corp_index(o))), Some(o))
    } else if world.has::<citysim::Gang>(o) {
        (RichText::new(label).color(crate::ui::gang_colour(world.gang_index(o))), world.hideout_of(o))
    } else if world.is_alive(o) {
        (RichText::new(label), Some(o))
    } else {
        (RichText::new(label), None)
    };
    match target {
        Some(t) => {
            if ui.link(text).clicked() {
                app.selected = Some(t);
                app.follow = false;
            }
        }
        None => {
            ui.label(text);
        }
    }
}
