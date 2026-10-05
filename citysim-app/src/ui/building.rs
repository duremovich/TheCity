//! Building inspector (right panel): what a selected building is, who is
//! in it, and the per-kind state the world systems keep on it (pantry,
//! farm yield, market price, jail roster, gang treasury and territory).

use egui_macroquad::egui::{self, Color32, ProgressBar, RichText, Ui};

use citysim::{
    time, Brain, Building, BuildingKind, Child, Corpse, EntityId, Gang, GangMember, Household, Identity, Job, Lod,
    Market, PlayerCommand, Position, Role, Sentence, Wallet, World, TICKS_PER_DAY,
};

use crate::App;

const RED: Color32 = Color32::from_rgb(220, 60, 60);
const BLUE: Color32 = Color32::from_rgb(70, 120, 210);
const PURPLE: Color32 = Color32::from_rgb(142, 68, 173);
const GOLD: Color32 = Color32::from_rgb(217, 162, 61);

fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title).default_open(true).show(ui, body);
}

/// `value / cap` bar with the raw numbers as text.
fn stock_bar(ui: &mut Ui, label: &str, value: u32, cap: u32) {
    let frac = if cap == 0 { 0.0 } else { value as f32 / cap as f32 };
    let colour = if cap > 0 && frac < 0.1 { RED } else { BLUE };
    ui.horizontal(|ui| {
        ui.label(format!("{label:<10}"));
        ui.add(
            ProgressBar::new(frac.clamp(0.0, 1.0)).fill(colour).text(format!("{value} / {cap}")).desired_width(200.0),
        );
    });
}

/// A clickable agent name; clicking selects them in the inspector.
fn agent_link(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    if ui.link(world.name_of(id)).clicked() {
        app.selected = Some(id);
        app.follow = false;
    }
}

/// A clickable building label; clicking selects that building.
fn building_link(ui: &mut Ui, app: &mut App, world: &World, b: EntityId) {
    if ui.link(world.name_of(b)).clicked() {
        app.selected = Some(b);
        app.follow = false;
    }
}

fn lod_tag(world: &World, id: EntityId) -> &'static str {
    match world.comp::<Brain>(id).map(|b| b.lod) {
        Some(Lod::Full) => "F",
        Some(Lod::Coarse) => "C",
        Some(Lod::Statistical) => "S",
        None => "-",
    }
}

/// Agents whose `Job.employer` is this building, with their role.
fn workers(world: &World, b: EntityId) -> Vec<(EntityId, Role)> {
    let mut v: Vec<_> = world
        .with::<Job>()
        .into_iter()
        .filter_map(|id| world.comp::<Job>(id).filter(|j| j.employer == Some(b)).map(|j| (id, j.role)))
        .collect();
    v.sort();
    v
}

/// Adults and children whose `Household.home` is this building.
fn residents(world: &World, b: EntityId) -> (Vec<EntityId>, Vec<EntityId>) {
    let mut adults = Vec::new();
    let mut children = Vec::new();
    for id in world.with::<Household>() {
        if world.comp::<Household>(id).is_some_and(|h| h.home == Some(b)) && !world.has::<Corpse>(id) {
            if world.has::<Child>(id) {
                children.push(id);
            } else {
                adults.push(id);
            }
        }
    }
    (adults, children)
}

/// The gang whose territory holds this building, if any.
fn territory_of(world: &World, b: EntityId) -> Option<(EntityId, &Gang)> {
    world.gangs().into_iter().find_map(|g| {
        world.comp::<Gang>(g).filter(|gang| gang.territory.binary_search(&b).is_ok()).map(|gang| (g, gang))
    })
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let Some(b) = world.comp::<Building>(id) else {
        ui.label("Selection is gone.");
        app.selected = None;
        return;
    };
    egui::ScrollArea::vertical().show(ui, |ui| {
        header(ui, app, world, id, b);
        match b.kind {
            BuildingKind::Home => home(ui, app, world, id, b),
            BuildingKind::Farm => farm(ui, app, world, id, b),
            BuildingKind::Market => market(ui, app, world, id, b),
            BuildingKind::Bar => staff(ui, app, world, id, "Staff"),
            BuildingKind::Jail => jail(ui, app, world, id),
            BuildingKind::Cemetery => cemetery(ui, app, world, id),
            BuildingKind::Hall => hall(ui, app, world, id),
            BuildingKind::Hideout => hideout(ui, app, world, id, b),
            BuildingKind::Warehouse => warehouse(ui, world, b),
        }
        occupants(ui, app, world, b);
        buttons(ui, app, id, b);
    });
}

fn header(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    section(ui, "Building", |ui| {
        ui.heading(format!("{}#{}", b.kind, id.index));
        ui.label(format!(
            "{}×{} at ({}, {}) · door {} · capacity {}",
            b.rect.w, b.rect.h, b.rect.x, b.rect.y, b.door, b.capacity
        ));
        ui.horizontal(|ui| {
            match b.owner {
                Some(o) => {
                    ui.label("Owner");
                    agent_link(ui, app, world, o);
                }
                None => {
                    ui.label("City-owned");
                }
            }
            if b.demolished {
                ui.colored_label(RED, "Demolished");
            }
        });
        if let Some((gid, gang)) = territory_of(world, id) {
            ui.horizontal(|ui| {
                ui.colored_label(crate::ui::gang_colour(world.gang_index(gid)), format!("Territory of {}", gang.name));
                if let Some(h) = world.hideout_of(gid) {
                    building_link(ui, app, world, h);
                }
            });
        }
    });
}

fn home(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    let cap = world.config.buildings.for_kind(b.kind).stock_cap;
    let (adults, children) = residents(world, id);
    section(ui, "Pantry", |ui| {
        stock_bar(ui, "food", b.stock_food, cap);
        if b.child_food_debt > 0.0 {
            ui.colored_label(RED, format!("children owed {:.1} food", b.child_food_debt));
        }
    });
    section(ui, &format!("Residents ({} adults, {} children)", adults.len(), children.len()), |ui| {
        if adults.is_empty() && children.is_empty() {
            ui.label("empty");
        }
        egui::Grid::new("residents").striped(true).show(ui, |ui| {
            for &r in &adults {
                agent_link(ui, app, world, r);
                ui.label(lod_tag(world, r));
                let coins = world.comp::<Wallet>(r).map_or(0, |w| w.coins);
                ui.label(format!("{coins} coins"));
                let job = world.comp::<Job>(r).map_or("no job".to_string(), |j| j.role.to_string());
                ui.label(job);
                let flags =
                    [world.has::<GangMember>(r).then_some("gang"), world.has::<Sentence>(r).then_some("jailed")];
                ui.label(flags.into_iter().flatten().collect::<Vec<_>>().join(" "));
                ui.end_row();
            }
            for &c in &children {
                agent_link(ui, app, world, c);
                ui.label("child");
                let age = world.comp::<Identity>(c).map_or(0, |i| i.age_years());
                ui.label(format!("{age} y"));
                let hungry = world.comp::<Child>(c).map_or(0, |c| c.hunger_days);
                if hungry > 0 {
                    ui.colored_label(RED, format!("{hungry} hungry days"));
                } else {
                    ui.label("fed");
                }
                ui.label("");
                ui.end_row();
            }
        });
    });
    section(ui, "Gang pressure", |ui| {
        if territory_of(world, id).is_some() {
            ui.colored_label(PURPLE, "Pays 2 coins/day tribute");
        } else {
            match b.claim {
                Some(c) => {
                    let who = world.comp::<Gang>(c.gang).map_or("a gang", |g| g.name.as_str());
                    ui.label(format!("{who} has extorted it {} / 3 times", c.count));
                }
                None => {
                    ui.label("unclaimed");
                }
            }
        }
    });
}

fn farm(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    let cap = world.config.buildings.for_kind(b.kind).stock_cap;
    section(ui, "Production", |ui| {
        stock_bar(ui, "stock", b.stock_food, cap);
        ui.label(format!("accumulated {:.2} food toward the next unit", b.production_accum));
        let season = world.season();
        let mult = world.config.economy.season_mult[season.index()];
        ui.label(format!("{season} yield multiplier {mult:.2}"));
    });
    staff(ui, app, world, id, "Farmers");
}

fn market(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    let cap = world.config.buildings.for_kind(b.kind).stock_cap;
    section(ui, "Stock and price", |ui| {
        stock_bar(ui, "food", b.stock_food, cap);
        if let Some(m) = world.market() {
            ui.label(format!("price {} coins", m.price_food));
            price_history(ui, m);
        }
        let warehouse = world
            .building_of_kind(BuildingKind::Warehouse)
            .and_then(|w| world.comp::<Building>(w))
            .map_or(0, |w| w.stock_food);
        ui.label(format!("warehouse reserve {warehouse}"));
    });
    staff(ui, app, world, id, "Clerks");
}

/// Price over the last 120 days, drawn with the painter.
fn price_history(ui: &mut Ui, m: &Market) {
    let prices: Vec<f32> = m.price_history.iter().map(|&p| p as f32).collect();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(320.0, 48.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_gray(30));
    if prices.len() < 2 {
        return;
    }
    let max = prices.iter().cloned().fold(1.0f32, f32::max);
    let n = prices.len() as f32;
    let points: Vec<egui::Pos2> = prices
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let x = rect.left() + (i as f32 / (n - 1.0)) * rect.width();
            let y = rect.bottom() - (p / max) * (rect.height() - 4.0) - 2.0;
            egui::pos2(x, y)
        })
        .collect();
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.5_f32, GOLD)));
    ui.small(format!("{} days · max {max:.0}", prices.len()));
}

fn jail(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let capacity = world.config.buildings.jail.capacity;
    let mut inmates: Vec<(EntityId, &Sentence)> =
        world.with::<Sentence>().into_iter().filter_map(|a| world.comp::<Sentence>(a).map(|s| (a, s))).collect();
    inmates.sort_by_key(|(_, s)| s.until_tick);
    let open_warrants = world.crime_reports.iter().filter(|r| !r.resolved).count();
    section(ui, &format!("Inmates ({} / {capacity})", inmates.len()), |ui| {
        if inmates.len() >= usize::from(capacity) {
            ui.colored_label(RED, "Full: new convicts are fined or released");
        }
        ui.label(format!("{open_warrants} open warrants"));
        if inmates.is_empty() {
            ui.label("empty");
            return;
        }
        egui::Grid::new("inmates").striped(true).show(ui, |ui| {
            ui.strong("name");
            ui.strong("crime");
            ui.strong("release");
            ui.strong("");
            ui.end_row();
            for (a, s) in inmates {
                agent_link(ui, app, world, a);
                ui.label(format!("{:?}", s.crime));
                let left = s.until_tick.saturating_sub(world.tick) as f32 / TICKS_PER_DAY as f32;
                ui.label(format!("day {} ({left:.1}d)", time::day(s.until_tick)));
                if world.has::<GangMember>(a) {
                    ui.colored_label(PURPLE, "gang");
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
    });
    section(ui, "Guards", |ui| {
        let guards = workers(world, id);
        let on_duty =
            guards.iter().filter(|(g, _)| world.comp::<Position>(*g).is_some_and(|p| p.building == Some(id))).count();
        ui.label(format!("{} employed (lever {}) · {on_duty} inside now", guards.len(), world.levers.guard_count));
        ui.horizontal_wrapped(|ui| {
            for (g, _) in guards {
                agent_link(ui, app, world, g);
            }
        });
    });
}

fn cemetery(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let corpses = world.with::<Corpse>();
    let here = corpses.iter().filter(|c| world.comp::<Position>(**c).is_some_and(|p| p.building == Some(id))).count();
    let unburied = corpses.iter().filter(|c| world.comp::<Corpse>(**c).is_some_and(|c| !c.buried)).count();
    section(ui, "Graves", |ui| {
        ui.label(format!("{here} corpses here awaiting the grave · {unburied} unburied in the city"));
    });
    staff(ui, app, world, id, "Gravedigger");
}

fn hall(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    section(ui, "Treasury", |ui| {
        let coins = world.treasury().map_or(0, |t| t.coins);
        if coins < 0 {
            ui.colored_label(RED, format!("{coins} coins: wages unpaid"));
        } else {
            ui.label(format!("{coins} coins"));
        }
        let l = &world.levers;
        ui.label(format!(
            "tax {:.0}% · sentence ×{:.1} · {} guards · {} immigrants/week · dole {}/day",
            l.tax_rate * 100.0,
            l.sentence_mult,
            l.guard_count,
            l.immigration_per_week,
            l.dole_per_day
        ));
    });
    staff(ui, app, world, id, "Clerks");
}

fn hideout(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    let cap = world.config.buildings.for_kind(b.kind).stock_cap;
    let Some((gid, gang)) =
        world.gangs().into_iter().find_map(|g| world.comp::<Gang>(g).filter(|gg| gg.hideout == id).map(|gg| (g, gg)))
    else {
        section(ui, "Gang", |ui| {
            ui.label("no gang holds this Hideout");
        });
        return;
    };
    let colour = crate::ui::gang_colour(world.gang_index(gid));
    let now = world.tick;
    let days = |t: u64| t as f32 / TICKS_PER_DAY as f32;
    let rival = world.rival_of(gid);
    let rival_gang = rival.and_then(|r| world.comp::<Gang>(r));
    let fit = citysim::systems::gang::fit_headcount(world, gid);
    let rival_fit = rival.map_or(0, |r| citysim::systems::gang::fit_headcount(world, r));
    section(ui, &format!("{} ({} members)", gang.name, gang.members.len()), |ui| {
        ui.colored_label(
            colour,
            format!(
                "Order {} since day {} ({:.1} d)",
                gang.order,
                time::day(gang.order_since),
                days(now.saturating_sub(gang.order_since))
            ),
        );
        match gang.leader {
            Some(l) => {
                ui.horizontal(|ui| {
                    ui.label("Leader");
                    agent_link(ui, app, world, l);
                    if world.has::<Sentence>(l) {
                        ui.colored_label(RED, "jailed: no new orders");
                    }
                });
            }
            None => {
                ui.colored_label(RED, "No leader: no new orders");
            }
        }
        if let Some(t) = gang.raid_at {
            let when = if t > now { format!("in {} min", t - now) } else { "departed".to_string() };
            let what = if gang.order.target_is_jail() { "Breakout" } else { "Raid" };
            ui.colored_label(RED, format!("{what} musters {when} ({})", time::clock(t)));
        }
        let jailed = citysim::systems::gang::jailed_headcount(world, gid);
        if let Some(b) = gang.boss.filter(|&b| world.has::<Sentence>(b)) {
            ui.horizontal(|ui| {
                ui.colored_label(RED, "Boss inside:");
                agent_link(ui, app, world, b);
            });
        }
        if jailed > 0 {
            let cooldown = world.config.gangs.breakout_cooldown_days * TICKS_PER_DAY;
            let wait = gang.last_breakout_tick.map_or(0, |t| (t + cooldown).saturating_sub(now));
            let note = if wait > 0 { format!(" · breakout possible in {:.1} d", days(wait)) } else { String::new() };
            ui.label(format!("{jailed} in the Jail{note}"));
        }
        if gang.is_sacked(now) {
            let left = gang.sacked_until.map_or(0.0, |t| days(t.saturating_sub(now)));
            ui.colored_label(RED, format!("SACKED: {left:.1} days to recover"));
        }
        if let Some(t) = gang.retaliate_until.filter(|&t| t > now) {
            ui.label(format!("retaliating for {:.1} more days", days(t - now)));
        }
        let heat = citysim::systems::faction::heat(world, gid);
        let window = world.config.gangs.heat_days * TICKS_PER_DAY;
        let hot = gang.heat_log.iter().filter(|&&(t, _)| now.saturating_sub(t) < window).count();
        ui.label(format!("heat {heat:.2} ({hot} arrests or deaths in {} days)", world.config.gangs.heat_days));
        ui.label(format!(
            "{fit} fit vs {rival_fit} rival · treasury {} vs {}",
            gang.treasury,
            rival_gang.map_or(0, |g| g.treasury)
        ));
        stock_bar(ui, "fenced", b.stock_food, cap);
        if let Some(t) = gang.empty_since {
            ui.colored_label(RED, format!("empty for {:.1} days (disbands at 30)", days(now.saturating_sub(t))));
        }
    });
    section(ui, "Order trace", |ui| {
        if gang.order_trace.is_empty() {
            ui.label("no rescoring yet");
        }
        for s in gang.order_trace.iter().take(3) {
            egui::CollapsingHeader::new(format!("{}  {:.3}", s.order, s.score))
                .default_open(s.order == gang.order)
                .show(ui, |ui| {
                    egui::Grid::new(format!("order-{}", s.order)).striped(true).show(ui, |ui| {
                        for c in &s.considerations {
                            ui.label(&c.name);
                            ui.label(format!("{:.3}", c.input));
                            ui.label("->");
                            ui.label(format!("{:.3}", c.output));
                            ui.end_row();
                        }
                    });
                });
        }
    });
    section(ui, "Roster", |ui| {
        let mut members: Vec<(EntityId, u8, u64)> = gang
            .members
            .iter()
            .filter_map(|&m| world.comp::<GangMember>(m).map(|gm| (m, gm.rank, gm.joined_tick)))
            .collect();
        members.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
        egui::Grid::new("members").striped(true).show(ui, |ui| {
            ui.strong("name");
            ui.strong("rank");
            ui.strong("joined");
            ui.strong("coins");
            ui.strong("doing");
            ui.end_row();
            for (m, rank, joined) in members {
                agent_link(ui, app, world, m);
                let rank_name = if rank == 2 { "leader" } else { "grunt" };
                if rank == 2 {
                    ui.colored_label(GOLD, rank_name);
                } else {
                    ui.label(rank_name);
                }
                ui.label(format!("day {}", time::day(joined)));
                ui.label(format!("{}", world.comp::<Wallet>(m).map_or(0, |w| w.coins)));
                if world.has::<Sentence>(m) {
                    ui.colored_label(RED, "jailed");
                } else {
                    let following = world.comp::<Brain>(m).and_then(|b| b.following_order);
                    ui.label(match following {
                        Some(o) => format!("{o} ({})", lod_tag(world, m)),
                        None => format!("freelance ({})", lod_tag(world, m)),
                    });
                }
                ui.end_row();
            }
        });
    });
    let tribute = gang.territory.len() * 2;
    section(ui, &format!("Territory ({} homes, {tribute} coins/day)", gang.territory.len()), |ui| {
        if gang.territory.is_empty() {
            ui.label("none yet");
        }
        ui.horizontal_wrapped(|ui| {
            for &h in &gang.territory {
                if ui.link(RichText::new(world.name_of(h)).color(colour)).clicked() {
                    app.selected = Some(h);
                    app.follow = false;
                }
            }
        });
    });
}

fn warehouse(ui: &mut Ui, world: &World, b: &Building) {
    let cap = world.config.buildings.for_kind(b.kind).stock_cap;
    section(ui, "Reserve", |ui| {
        stock_bar(ui, "food", b.stock_food, cap);
        ui.label("Moved to the Market by the city panel's Release reserve lever.");
    });
}

/// Everyone employed here, by role.
fn staff(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, title: &str) {
    let staff = workers(world, id);
    section(ui, &format!("{title} ({})", staff.len()), |ui| {
        if staff.is_empty() {
            ui.label("none");
            return;
        }
        egui::Grid::new("staff").striped(true).show(ui, |ui| {
            for (w, role) in staff {
                agent_link(ui, app, world, w);
                ui.label(role.to_string());
                let here = world.comp::<Position>(w).is_some_and(|p| p.building == Some(id));
                let jailed = world.has::<Sentence>(w);
                ui.label(if jailed {
                    "jailed"
                } else if here {
                    "on site"
                } else {
                    "away"
                });
                let unpaid = world.comp::<Job>(w).map_or(0, |j| j.days_unpaid);
                if unpaid > 0 {
                    ui.colored_label(RED, format!("{unpaid}d unpaid"));
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
    });
}

/// Agents physically inside right now (all LOD tiers).
fn occupants(ui: &mut Ui, app: &mut App, world: &World, b: &Building) {
    section(ui, &format!("Inside now ({} / {})", b.occupants.len(), b.capacity), |ui| {
        if b.occupants.is_empty() {
            ui.label("nobody");
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for &o in &b.occupants {
                agent_link(ui, app, world, o);
                ui.small(lod_tag(world, o));
            }
        });
    });
}

fn buttons(ui: &mut Ui, app: &mut App, id: EntityId, b: &Building) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui.button("Centre camera").clicked() {
            app.camera.centre_on(b.door);
        }
        if b.kind == BuildingKind::Home && !b.demolished && ui.button("Demolish").clicked() {
            app.cmds.push(PlayerCommand::DemolishHome(id));
        }
        if ui.button("Close").clicked() {
            app.selected = None;
            app.follow = false;
        }
    });
    ui.add_space(4.0);
    ui.small(
        RichText::new("Click a name to inspect that agent; click a Home in the territory list to jump to it.")
            .color(GOLD),
    );
}
