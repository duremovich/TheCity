//! M15 § 10: the word's panel sections. Every number here is read off
//! the world (reputation, regard, grudges, Hunts, stories); the panels
//! only send player commands.

use egui_macroquad::egui::{self, Color32, ProgressBar, RichText, Ui};

use citysim::systems::{grudges, news, reputation};
use citysim::word::{Grudges, HuntPhase};
use citysim::{time, Brain, Corp, EntityId, Gang, PlayerCommand, Skills, World, TICKS_PER_DAY};

use crate::App;

/// The word's colour (plan W43, `#D0303A`).
pub const CRIMSON: Color32 = Color32::from_rgb(0xD0, 0x30, 0x3A);
const GOLD: Color32 = Color32::from_rgb(217, 162, 61);

pub(super) fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title).default_open(true).show(ui, body);
}

fn bar(ui: &mut Ui, label: &str, value: f32) {
    ui.horizontal(|ui| {
        ui.label(format!("{label:<12}"));
        ui.add(ProgressBar::new(value.clamp(0.0, 1.0)).fill(CRIMSON).text(format!("{value:.2}")).desired_width(180.0));
    });
}

pub(super) fn link(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    if ui.link(grudges::label(world, id)).clicked() {
        app.selected = Some(id);
        app.follow = false;
    }
}

/// The four axes and `known_by` of an agent or a faction.
pub fn axes(ui: &mut Ui, world: &World, id: EntityId) {
    let r = reputation::rep(world, id);
    bar(ui, "dread", r.dread);
    bar(ui, "standing", r.standing);
    bar(ui, "honour", r.honour);
    bar(ui, "heat", r.heat);
    let press = if r.press != 0.0 { format!(" · press {:+.2}", r.press) } else { String::new() };
    ui.label(format!("known by {}{press}", r.known_by));
    if let Some((_, until)) = r.pinned.filter(|&(_, t)| t > world.tick) {
        ui.colored_label(GOLD, format!("pinned by god until day {}", until / TICKS_PER_DAY));
    }
}

/// A faction panel's Word section (spec § 10): the four axes, competence,
/// the Regard row against every other faction, open vendettas with their
/// kills, a corp's plants and buries, a gang's creed.
pub fn faction(ui: &mut Ui, app: &mut App, world: &World, f: EntityId) {
    if !world.config.gossip.enabled {
        return;
    }
    section(ui, "Word", |ui| {
        axes(ui, world, f);
        if let Some(c) = world.comp::<Corp>(f) {
            ui.label(format!("competence {:.3}", c.competence));
            // W40 (a side spend beside the corp-wide order).
            match c.spin_since {
                Some(t) => ui
                    .colored_label(CRIMSON, format!("spinning since day {} (score {:.2})", time::day(t), c.spin_score)),
                None => ui.label(format!("not spinning (score {:.2})", c.spin_score)),
            };
            if !c.spin_trace.is_empty() {
                egui::CollapsingHeader::new("Spin score").id_salt(format!("spin-{}", f.index)).show(ui, |ui| {
                    egui::Grid::new(format!("spin-cons-{}", f.index)).striped(true).show(ui, |ui| {
                        for k in &c.spin_trace {
                            ui.label(k.name.as_ref());
                            ui.label(format!("{:.3}", k.input));
                            ui.label(format!("{:.3}", k.output));
                            ui.end_row();
                        }
                    });
                });
            }
        }
        if let Some(l) = world.comp::<citysim::components::Law>(f) {
            ui.label(format!("competence {:.3}", l.competence));
        }
        if let Some(g) = world.comp::<Gang>(f) {
            match g.creed {
                Some(c) => ui.colored_label(CRIMSON, format!("creed: {}", c.label())),
                None => ui.label("no creed"),
            };
        }
        let others: Vec<EntityId> = reputation::factions(world).into_iter().filter(|&o| o != f).collect();
        if !others.is_empty() {
            ui.small("Regard (value · fear)");
            egui::Grid::new(format!("regard-{}", f.index)).striped(true).show(ui, |ui| {
                for o in others {
                    link(ui, app, world, o);
                    let r = reputation::regard(world, f, o);
                    let c = if r.value < -0.1 { CRIMSON } else { Color32::LIGHT_GRAY };
                    ui.colored_label(c, format!("{:+.2}", r.value));
                    ui.label(format!("{:.2}", r.fear));
                    ui.end_row();
                }
            });
        }
        let feuds: Vec<&citysim::word::Vendetta> = world.vendettas.iter().filter(|v| v.a == f || v.b == f).collect();
        if !feuds.is_empty() {
            ui.small("Vendettas (kills ours · theirs)");
            for v in feuds {
                let (other, ours, theirs) =
                    if v.a == f { (v.b, v.kills[0], v.kills[1]) } else { (v.a, v.kills[1], v.kills[0]) };
                ui.horizontal(|ui| {
                    ui.colored_label(CRIMSON, "blood with");
                    link(ui, app, world, other);
                    ui.label(format!("since day {} · {ours} · {theirs}", time::day(v.since)));
                });
            }
        }
        if world.has::<Corp>(f) {
            let spins: Vec<&citysim::Event> = world
                .events
                .iter()
                .rev()
                .filter(|e| {
                    matches!(e.kind, citysim::EventKind::Planted | citysim::EventKind::Buried)
                        && e.actors.first() == Some(&f)
                })
                .take(6)
                .collect();
            if !spins.is_empty() {
                ui.small("Spin (newest first)");
                for e in spins {
                    ui.label(RichText::new(format!("day {} · {}", time::day(e.tick), e.text)).color(CRIMSON).small());
                }
            }
        }
    });
}

/// The inspector's word sections (spec § 10): the active Hunt (and who
/// hunts this agent), the grudges, the social skills.
pub fn agent(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    if !world.config.gossip.enabled {
        return;
    }
    if let Some(h) = world.hunts.get(&id) {
        section(ui, "Hunt", |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(CRIMSON, "hunting");
                link(ui, app, world, h.target);
                ui.label(format!("since day {} · chain {} · {:?}", time::day(h.since), h.chain, h.why));
            });
            let phase = match h.phase {
                HuntPhase::Ask => "asking around",
                HuntPhase::Watch => "watching",
            };
            ui.label(format!("{phase} · grudge {:.2}", h.weight));
            if let Some(v) = h.venue {
                ui.horizontal(|ui| {
                    ui.label("venue");
                    link(ui, app, world, v);
                });
            }
            match h.intel {
                Some(i) => {
                    ui.horizontal(|ui| {
                        ui.label(format!("intel ({:?})", i.source));
                        match i.building {
                            Some(b) => link(ui, app, world, b),
                            None => {
                                ui.label(format!("tile {}", i.tile));
                            }
                        }
                    });
                }
                None => {
                    ui.label("no intel yet");
                }
            }
            let staking = world
                .comp::<Brain>(id)
                .and_then(|b| b.current_step())
                .is_some_and(|s| s.action == citysim::ActionKind::StakeOut);
            if let Some(t) = h.stakeout_until {
                let left = t.saturating_sub(world.tick) as f32 / 60.0;
                let what = if staking { "staking out" } else { "stake-out set" };
                ui.colored_label(CRIMSON, format!("{what}: {left:.1} h left ({})", time::clock(t)));
            }
            if h.deceived {
                ui.label("(was sent the wrong way)");
            }
        });
    }
    if let Some(&hunter) = world.hunted_by.get(&id) {
        ui.horizontal(|ui| {
            ui.colored_label(CRIMSON, "hunted by");
            link(ui, app, world, hunter);
        });
    }
    section(ui, "Grudges", |ui| {
        let list = world.comp::<Grudges>(id).map(|g| g.list.to_vec()).unwrap_or_default();
        if list.is_empty() {
            ui.label("none");
        } else {
            egui::Grid::new("grudges").striped(true).show(ui, |ui| {
                ui.strong("on");
                ui.strong("why");
                ui.strong("weight");
                ui.strong("chain");
                ui.end_row();
                for g in list {
                    link(ui, app, world, g.target);
                    ui.label(cause_text(world, g.cause));
                    let w = format!("{:.2}", g.weight);
                    match g.settled {
                        Some(t) => ui.label(format!("{w} settled d{}", time::day(t))),
                        None => ui.colored_label(CRIMSON, w),
                    };
                    ui.label(format!("{}", g.chain));
                    ui.end_row();
                }
            });
        }
    });
    if let Some(s) = world.comp::<Skills>(id) {
        section(ui, "Social skills", |ui| {
            bar(ui, "persuasion", s.persuasion);
            bar(ui, "intimidation", s.intimidation);
            bar(ui, "knowledge", s.knowledge);
            bar(ui, "deception", s.deception);
        });
    }
}

fn cause_text(world: &World, c: citysim::word::GrudgeCause) -> String {
    use citysim::word::GrudgeCause as G;
    match c {
        G::KilledKin(x) => format!("killed kin {}", world.name_of(x)),
        G::KilledFriend(x) => format!("killed friend {}", world.name_of(x)),
        G::Assaulted => "assault".to_string(),
        G::Robbed => "robbery".to_string(),
        G::Stripped(x) => format!("stripped {}", world.name_of(x)),
        G::Evicted => "eviction".to_string(),
        G::Betrayed => "betrayal".to_string(),
        G::Inherited(x) => format!("inherited from {}", world.name_of(x)),
        G::Hired => "a contract".to_string(),
        G::ChildTaken => "a child taken".to_string(),
    }
}

/// One story's line: `day · feed: actor deed object (slant, paid by)`.
fn story_line(world: &World, s: &citysim::word::Story) -> String {
    let obj = s.object.map_or_else(String::new, |o| format!(" {}", grudges::label(world, o)));
    let paid = s.paid_by.map_or_else(String::new, |p| format!(" · paid by {}", world.owner_label(Some(p))));
    format!(
        "{}: {} {}{obj} (slant {:+.1}{paid})",
        news::feed_name(world, s.feed),
        grudges::label(world, s.actor),
        s.deed.label(),
        s.slant
    )
}

/// The City panel's Word section (spec § 10): rumours heard today, the
/// second-hand share, the top five stories, open vendettas, Hunts under
/// way, the longest chain; and the three news levers.
pub fn city(ui: &mut Ui, app: &mut App, world: &World) {
    if !world.config.gossip.enabled {
        return;
    }
    ui.separator();
    // `--scroll-word` (screenshots): keep the section in view.
    if app.scroll_word {
        ui.scroll_to_cursor_animation(Some(egui::Align::TOP), egui::style::ScrollAnimation::none());
    }
    ui.horizontal(|ui| {
        ui.strong("Word");
        ui.small(if app.show_word { "overlay on (J)" } else { "J draws the talk" });
    });
    let today = &world.stats.current.word;
    let yday = world.stats.history.back().map(|r| r.word.clone()).unwrap_or_default();
    egui::Grid::new("city_word").striped(true).show(ui, |ui| {
        let rows: [(&str, String); 6] = [
            ("Rumours heard today", today.rumours_heard.to_string()),
            ("Second-hand share", format!("{:.0}%", 100.0 * yday.second_hand_share)),
            ("Stories yesterday", yday.stories.to_string()),
            ("Open vendettas", world.vendettas.len().to_string()),
            ("Hunts under way", world.hunts.len().to_string()),
            ("Longest chain", yday.chain_max.to_string()),
        ];
        for (k, v) in rows {
            ui.label(k);
            ui.label(v);
            ui.end_row();
        }
    });
    // The top five of the last day's stories, by the deed's newsworthiness now.
    let last = world.stories.back().map(|s| time::day(s.tick));
    let mut top: Vec<&citysim::word::Story> =
        world.stories.iter().filter(|s| Some(time::day(s.tick)) == last).collect();
    top.sort_by(|a, b| {
        let sal = |s: &citysim::word::Story| world.config.gossip.deed_sal.get(s.deed);
        sal(b).total_cmp(&sal(a)).then(a.id.cmp(&b.id))
    });
    if !top.is_empty() {
        ui.small("Top stories");
        for s in top.into_iter().take(5) {
            ui.label(RichText::new(story_line(world, s)).color(CRIMSON).small());
        }
    }
    // The six heaviest feuds (the weights at the last midnight).
    let mut feuds: Vec<&citysim::word::Vendetta> = world.vendettas.iter().collect();
    feuds.sort_by(|a, b| (b.w[0] + b.w[1]).total_cmp(&(a.w[0] + a.w[1])).then(a.since.cmp(&b.since)));
    for v in feuds.iter().take(6) {
        ui.label(
            RichText::new(format!(
                "blood: {} vs {} ({}:{} kills)",
                grudges::label(world, v.a),
                grudges::label(world, v.b),
                v.kills[0],
                v.kills[1]
            ))
            .small(),
        );
    }
    if feuds.len() > 6 {
        ui.small(format!("and {} more feuds", feuds.len() - 6));
    }
    if !world.config.news.enabled {
        return;
    }
    ui.small("Levers");
    let mut licence = world.levers.press_licence;
    if ui.checkbox(&mut licence, "Press licence (off: only the Civic Wire)").changed() {
        app.cmds.push(PlayerCommand::SetPressLicence(licence));
    }
    let r = ui.add(egui::Slider::new(&mut app.city.news_tax, 0.0..=1.0).text("News tax"));
    if r.drag_stopped() || (r.changed() && !r.dragged()) {
        app.cmds.push(PlayerCommand::SetNewsTax(app.city.news_tax));
    }
    ui.small("Censor on the Civic Wire");
    ui.horizontal_wrapped(|ui| {
        for f in reputation::factions(world) {
            let mut on = world.levers.censored.contains(&f);
            if ui.checkbox(&mut on, grudges::label(world, f)).changed() {
                app.cmds.push(PlayerCommand::CensorStories { faction: f, on });
            }
        }
    });
}
