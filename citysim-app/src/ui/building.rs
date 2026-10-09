//! Building inspector (right panel): what a selected building is, who is
//! in it, and the per-kind state the world systems keep on it (pantry,
//! farm yield, market price, jail roster, gang treasury and territory).

use egui_macroquad::egui::{self, Color32, ProgressBar, RichText, Ui};

use citysim::{
    time, Brain, Building, BuildingKind, Child, Corp, Corpse, EntityId, Gang, GangMember, Household, Identity, Job,
    Lod, Market, PlayerCommand, Position, Role, Sentence, Wallet, World, TICKS_PER_DAY,
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
pub(super) fn stock_bar(ui: &mut Ui, label: &str, value: u32, cap: u32) {
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
            BuildingKind::SecurityOffice => staff(ui, app, world, id, "Guards"),
            BuildingKind::Lot => {
                ui.label(format!(
                    "{} · a vacant Lot: an NPC Registers a Bar or a Block here, a corp Grows one",
                    world.district_name(world.district_of(b.door))
                ));
            }
            BuildingKind::Hotel => hotel(ui, app, world, id),
            // M13: sales and stock are in the Assets section below.
            BuildingKind::Clinic | BuildingKind::Garage => staff(ui, app, world, id, "Staff"),
            // M14 V16: the Lab's Researchers (its panel is phase 4).
            BuildingKind::Lab => staff(ui, app, world, id, "Researchers"),
            // M15 § 10: the Feed panel, then its Reporters.
            BuildingKind::Feed => {
                super::feed::draw(ui, app, world, id);
                staff(ui, app, world, id, "Reporters");
            }
            // L2 § 8: a venue's price, 7-day visits, take, front gang, staff on shift.
            BuildingKind::Club
            | BuildingKind::Arcade
            | BuildingKind::NoodleBar
            | BuildingKind::FightPit
            | BuildingKind::Den
            | BuildingKind::Lounge => {
                if let Some(v) = &b.venue {
                    let week: u32 = v.visits.iter().map(|&x| u32::from(x)).sum();
                    ui.label(format!("price {}¢ · visits today {} · 7 days {week}", v.price, v.visits_today));
                    ui.label(format!("take today {}¢ · since the last Collect {}¢", v.take_today, v.take_week));
                    if let Some(g) = v.front_of {
                        ui.label(format!("a front of {}", world.owner_label(Some(g))));
                    }
                    if let Some(h) = v.heir {
                        ui.label(format!("passes to {} once it earns", world.owner_label(Some(h))));
                    }
                    if !v.bets.is_empty() {
                        let pot: i64 = v.bets.iter().map(|&(_, s, _)| s).sum();
                        ui.label(format!("tonight's bets {} ({pot}¢)", v.bets.len()));
                    }
                    if v.table_shut_day == Some(world.day()) {
                        ui.label("table shut today (the house could not pay)");
                    }
                    let tod = world.tick_of_day();
                    let on = citysim::systems::ownership::staff_at(world, id)
                        .into_iter()
                        .filter(|&s| world.comp::<citysim::Job>(s).is_some_and(|j| j.on_shift(tod)))
                        .count();
                    let open = if citysim::systems::leisure::open(world, id) { "open" } else { "closed" };
                    ui.label(format!("{open} · staff on shift {on}"));
                }
                staff(ui, app, world, id, "Staff");
            }
            BuildingKind::Fab => {
                // L2 § 8: the Fab's Parts made (city-wide today) and held.
                let held = b.stock_goods.get(citysim::Good::Parts as usize - 1).copied().unwrap_or(0);
                ui.label(format!("Parts held {held} · made today (all Fabs) {}", world.stats.current.living.fab_parts));
                staff(ui, app, world, id, "Fab Techs");
            }
            // M16a (plan C42; the full panel is phase 4): the record book's size.
            BuildingKind::Fixer => {
                if let Some(k) = world.comp::<citysim::contract::Broker>(id) {
                    ui.label(format!(
                        "book {} · regulars {} · cut {:.0}% · heat {:.2}",
                        k.book.len(),
                        k.regulars.len(),
                        k.cut * 100.0,
                        k.heat
                    ));
                }
                staff(ui, app, world, id, "Staff");
            }
            // Real economy E26 (plan 3b.5): the Mission's purse and service.
            BuildingKind::Mission => {
                if let Some(c) = &b.charity {
                    let served: u32 = c.served.iter().map(|&m| u32::from(m)).sum();
                    ui.label(format!(
                        "purse {} · stock {} · meals today {} · cots {} · meals 30 d {served}",
                        c.purse, b.stock_food, c.meals_today, c.cots_today
                    ));
                    let mut donors: Vec<(i64, EntityId)> = c.donors.iter().map(|(&d, &v)| (v, d)).collect();
                    donors.sort_unstable_by(|a, b| b.cmp(a));
                    for (v, d) in donors.into_iter().take(3) {
                        let who = if d == EntityId::NONE { "the World".to_string() } else { world.owner_label(Some(d)) };
                        ui.label(format!("  {who} gave {v} (30 d)"));
                    }
                }
                staff(ui, app, world, id, "Volunteers");
            }
            // Real economy E37: the camp's children and ledger.
            BuildingKind::Camp => {
                if let Some(c) = &b.camp {
                    let held = b.stock_goods.get(citysim::Good::Parts as usize - 1).copied().unwrap_or(0);
                    let state = if c.closed_by_law { "closed by the law" } else { "open" };
                    ui.label(format!(
                        "{state} · children {} · food {} · Parts held {held} · made today {} · unfed today {} · scandal days {}",
                        c.children.len(),
                        b.stock_food,
                        c.output_today,
                        c.unfed_today,
                        c.scandal_days
                    ));
                }
            }
        }
        super::asset::building_section(ui, app, world, id, b);
        derelict(ui, app, world, id, b);
        occupants(ui, app, world, b);
        buttons(ui, app, world, id, b);
    });
}

fn header(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    section(ui, "Building", |ui| {
        ui.heading(format!("{}#{}", b.kind.label(), id.index));
        ui.label(format!(
            "{}×{} at ({}, {}) · door {} · capacity {}",
            b.rect.w, b.rect.h, b.rect.x, b.rect.y, b.door, b.capacity
        ));
        ui.horizontal(|ui| {
            ui.label("Owner");
            match b.owner {
                Some(o) if world.has::<Corp>(o) => {
                    // The owner link opens the corp panel.
                    let i = world.corp_index(o);
                    let name = RichText::new(world.owner_label(Some(o))).color(crate::ui::corp_colour(i));
                    if ui.link(name).clicked() {
                        app.selected = Some(o);
                        app.follow = false;
                    }
                    if let Some(c) = world.comp::<Corp>(o) {
                        ui.label(format!("· {}¢ · {}", c.treasury, c.order));
                    }
                }
                Some(o) if world.has::<Gang>(o) => {
                    let name =
                        RichText::new(world.owner_label(Some(o))).color(crate::ui::gang_colour(world.gang_index(o)));
                    if ui.link(name).clicked() {
                        app.selected = world.hideout_of(o);
                        app.follow = false;
                    }
                }
                Some(o) => agent_link(ui, app, world, o),
                None => {
                    ui.label("the city");
                }
            }
            if b.demolished {
                ui.colored_label(RED, "Demolished");
            }
        });
        // M12: the district, a derelict flag, and a riot's closure.
        ui.horizontal(|ui| {
            ui.label(world.district_name(world.district_of(b.door)));
            if b.derelict {
                ui.colored_label(RED, "Derelict");
            }
            if let Some(t) = b.closed_until.filter(|&t| t > world.tick) {
                ui.colored_label(RED, format!("Closed (riot) until day {} {}", time::day(t), time::clock(t)));
            }
        });
        let tier = ["Sump", "Mid", "Spire"].get(usize::from(b.tier)).copied().unwrap_or("?");
        if b.kind == BuildingKind::Home {
            let cap = world.levers.rent_cap.map_or(String::new(), |c| format!(" (cap {c}¢)"));
            ui.label(format!("Rent {}¢/day{cap} · tier {} {tier}", b.rent_per_day, b.tier));
        } else {
            ui.label(format!("Tier {} {tier}", b.tier));
        }
        if !b.revenue.is_empty() || b.revenue_today != 0 {
            let week: i64 = b.revenue.iter().rev().take(7).sum();
            ui.label(format!(
                "Revenue {}¢ today, {}¢ over the last {} days",
                b.revenue_today,
                week,
                b.revenue.len().min(7)
            ));
        }
        security(ui, app, world, id, b);
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
                ui.label(format!("{coins}¢"));
                let job = world.comp::<Job>(r).map_or("no job".to_string(), |j| j.role.label().to_string());
                ui.label(job);
                let today = world.day();
                let (arrears, paid) = world.comp::<Household>(r).map_or((0, 0), |h| (h.arrears, h.rent_paid_7d(today)));
                ui.label(format!("rent {paid}¢/7d"));
                let behind = (arrears > 0).then(|| format!("{arrears} days behind"));
                let flags = [
                    world.has::<GangMember>(r).then(|| "gang".to_string()),
                    world.has::<Sentence>(r).then(|| "jailed".to_string()),
                    behind,
                ];
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
            ui.colored_label(PURPLE, "Pays 2¢/day tribute");
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
    staff(ui, app, world, id, "Vat Techs");
}

fn market(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    let cap = world.config.buildings.for_kind(b.kind).stock_cap;
    section(ui, "Stock and price", |ui| {
        stock_bar(ui, "food", b.stock_food, cap);
        if let Some(m) = world.comp::<Market>(id) {
            ui.label(format!("price {}¢ (city mean {}¢)", m.price_food, world.mean_price()));
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

fn law_section(ui: &mut Ui, app: &mut App, world: &World) {
    let Some(law) = world.law() else { return };
    let now = world.tick;
    let days = |t: u64| t as f32 / TICKS_PER_DAY as f32;
    section(ui, "The Law", |ui| {
        let target = law.target.and_then(|g| world.comp::<Gang>(g)).map(|g| g.name.clone());
        let on = target.map_or(String::new(), |t| format!(" on {t}"));
        ui.colored_label(
            egui::Color32::from_rgb(120, 170, 220),
            format!(
                "Posture {}{on} since day {} ({:.1} d)",
                law.posture,
                time::day(law.posture_since),
                days(now.saturating_sub(law.posture_since))
            ),
        );
        if let Some(p) = law.pinned {
            ui.colored_label(GOLD, format!("pinned to {p} by the player"));
        }
        ui.horizontal(|ui| match law.captain {
            Some(c) => {
                ui.label("Captain");
                agent_link(ui, app, world, c);
                if let Some(p) = world.comp::<citysim::Personality>(c) {
                    ui.label(format!(
                        "lawfulness {:.2} · greed {:.2} · courage {:.2}",
                        p.lawfulness, p.greed, p.courage
                    ));
                }
            }
            None => {
                ui.colored_label(RED, "No captain: no guards");
            }
        });
        let reports = citysim::systems::law_brain::reports_by_gang(world);
        let mut parts: Vec<String> = Vec::new();
        for (g, n) in &reports {
            if let Some(gg) = world.comp::<Gang>(*g) {
                parts.push(format!("{} {n}", gg.name));
            }
        }
        ui.label(format!(
            "reports in {} days: {}",
            world.config.law.window_days,
            if parts.is_empty() { "none".to_string() } else { parts.join(" · ") }
        ));
        if let Some(t) = law.last_breakout_tick {
            ui.colored_label(RED, format!("last jailbreak day {} ({:.1} d ago)", time::day(t), days(now - t)));
        }
        if let Some(t) = law.hardened_until.filter(|&t| t > now) {
            ui.label(format!("a refused bribe hardens the crackdown for {:.1} d", days(t - now)));
        }
        for gid in world.gangs() {
            if let Some(g) = world.comp::<Gang>(gid) {
                if let Some(t) = g.paid_until.filter(|&t| t > now) {
                    ui.label(format!("{}: no crackdown for {:.1} d (bribe)", g.name, days(t - now)));
                }
            }
        }
    });
    if !law.beats.is_empty() {
        // M12 D10: the patrol guards dealt to each district today.
        section(ui, "Beats", |ui| {
            egui::Grid::new("jail-beats").striped(true).show(ui, |ui| {
                for d in &world.districts {
                    let n = law.beats.values().filter(|&&b| b == d.id).count();
                    if n == 0 {
                        continue;
                    }
                    ui.label(&d.name);
                    ui.label(format!("{n}"));
                    ui.label(citysim::systems::law_brain::stance_label(world, d.stance));
                    ui.end_row();
                }
            });
        });
    }
    section(ui, "Posture trace", |ui| {
        if law.posture_trace.is_empty() {
            ui.label("no rescoring yet");
        }
        for s in &law.posture_trace {
            egui::CollapsingHeader::new(format!("{}  {:.3}", s.posture, s.score))
                .default_open(s.posture == law.posture)
                .show(ui, |ui| {
                    egui::Grid::new(format!("posture-{}", s.posture)).striped(true).show(ui, |ui| {
                        for c in &s.considerations {
                            ui.label(c.name.as_ref());
                            ui.label(format!("{:.3}", c.input));
                            ui.label("->");
                            ui.label(format!("{:.3}", c.output));
                            ui.end_row();
                        }
                    });
                });
        }
    });
}

fn jail(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    law_section(ui, app, world);
    let capacity = world.config.buildings.jail.capacity;
    let mut inmates: Vec<(EntityId, &Sentence)> =
        world.with::<Sentence>().into_iter().filter_map(|a| world.comp::<Sentence>(a).map(|s| (a, s))).collect();
    inmates.sort_by_key(|(_, s)| s.until_tick);
    let open_warrants = world.crime_reports().iter().filter(|r| !r.resolved).count();
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
                ui.label(s.crime.label());
                let left = s.until_tick.saturating_sub(world.tick) as f32 / TICKS_PER_DAY as f32;
                ui.label(format!("day {} ({left:.1}d)", time::day(s.until_tick)));
                match world.gang_of(a).and_then(|g| world.comp::<Gang>(g)) {
                    Some(g) => {
                        let boss = if g.boss == Some(a) { " (boss)" } else { "" };
                        ui.colored_label(PURPLE, format!("{}{boss}", g.name));
                    }
                    None => {
                        ui.label("");
                    }
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
    staff(ui, app, world, id, "Recycler Techs");
}

fn hall(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    section(ui, "Treasury", |ui| {
        let coins = world.treasury().map_or(0, |t| t.coins);
        if coins < 0 {
            ui.colored_label(RED, format!("{coins}¢: wages unpaid"));
        } else {
            ui.label(format!("{coins}¢"));
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
        // M12 D39: a corp building as the Raid's target; D36: lineage.
        if let Some(t) = citysim::systems::raid::corp_target(world, gid) {
            ui.label(format!("Raid target: {} ({})", world.name_of(t), world.owner_label(world.owner_of(t))));
        }
        if let Some(parent) = gang.split_from {
            let name = world.comp::<Gang>(parent).map_or_else(|| "a gang gone".to_string(), |g| g.name.clone());
            ui.label(format!("Split from {name}"));
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
        if citysim::systems::law::cracking_down_on(world, gid) {
            ui.colored_label(RED, "Under crackdown");
            match citysim::systems::faction::bribe_score(world, gid) {
                Some(cs) => {
                    let score: f32 = cs.iter().map(|c| c.output).product();
                    ui.label(format!(
                        "bribe {}¢: score {score:.2} (pays at {:.2})",
                        citysim::systems::faction::bribe_price(world),
                        world.config.law.bribe_threshold
                    ));
                }
                None => {
                    ui.label(format!(
                        "bribe {}¢: cannot (paid, broke or leaderless)",
                        citysim::systems::faction::bribe_price(world)
                    ));
                }
            }
        }
        if let Some(t) = gang.paid_until.filter(|&t| t > now) {
            ui.label(format!("the captain is bought for {:.1} more days", days(t - now)));
        } else if let Some(t) = gang.bribe_until.filter(|&t| t > now) {
            ui.label(format!("the captain refused; no new offer for {:.1} more days", days(t - now)));
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
        // M11 D33: a hoarding corp tilts Contest.
        let (hoard, hoard_corp) = citysim::systems::faction::hoard(world);
        if hoard > 0.0 {
            ui.label(format!(
                "hoard {} +{:.3} on Contest",
                world.owner_label(hoard_corp),
                world.config.corps.hoard_tilt * hoard
            ));
        }
        for s in gang.order_trace.iter().take(3) {
            egui::CollapsingHeader::new(format!("{}  {:.3}", s.order, s.score))
                .default_open(s.order == gang.order)
                .show(ui, |ui| {
                    egui::Grid::new(format!("order-{}", s.order)).striped(true).show(ui, |ui| {
                        for c in &s.considerations {
                            ui.label(c.name.as_ref());
                            ui.label(format!("{:.3}", c.input));
                            ui.label("->");
                            ui.label(format!("{:.3}", c.output));
                            ui.end_row();
                        }
                    });
                });
        }
    });
    // M15 § 10: the gang's axes, regard, vendettas and creed.
    super::word::faction(ui, app, world, gid);
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
            ui.strong("¢");
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
                ui.label(format!("{}¢", world.comp::<Wallet>(m).map_or(0, |w| w.coins)));
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
    section(ui, &format!("Territory ({} blocks, {tribute}¢/day)", gang.territory.len()), |ui| {
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
        ui.label("Moved to the Street Market by the city panel's Release reserve lever.");
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
                ui.label(role.label());
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

/// M12 D20: a Capsule Hotel: tonight's price, beds and guests.
fn hotel(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let price = citysim::systems::street::hotel_price(world, id);
    let free = citysim::systems::street::free_beds(world, id);
    let beds = world.comp::<Building>(id).map_or(0, |b| usize::from(b.capacity));
    let now = world.tick;
    let guests: Vec<EntityId> =
        world.hotel_beds.iter().filter(|(_, &(h, until))| h == id && until > now).map(|(&g, _)| g).collect();
    section(ui, &format!("Beds ({} of {beds} taken, {price} a night)", beds - free), |ui| {
        if guests.is_empty() {
            ui.label("no guests tonight");
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for g in guests {
                agent_link(ui, app, world, g);
                ui.small(lod_tag(world, g));
            }
        });
    });
}

/// M12 D25/D27: a derelict building and its squatters.
fn derelict(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    if !b.derelict {
        return;
    }
    let squatters = world.squatters_of(id).to_vec();
    section(ui, &format!("Derelict · squatters {} / {}", squatters.len(), b.capacity), |ui| {
        if let Some(t) = b.empty_since {
            ui.label(format!("derelict since day {}", t / citysim::time::TICKS_PER_DAY));
        }
        if let Some(c) = b.claim {
            let g = world.comp::<Gang>(c.gang).map_or_else(|| "a gang".to_string(), |g| g.name.clone());
            ui.label(format!("claimed by {g} ({}/3)", c.count));
        }
        ui.horizontal_wrapped(|ui| {
            for s in squatters {
                agent_link(ui, app, world, s);
                ui.small(lod_tag(world, s));
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

fn buttons(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui.button("Centre camera").clicked() {
            app.camera.centre_on(b.door);
        }
        // D44: only a city Block can be demolished; nationalise it first.
        if b.kind == BuildingKind::Home && !b.demolished && b.owner.is_none() && ui.button("Demolish").clicked() {
            app.cmds.push(PlayerCommand::DemolishHome(id));
        }
        let value = citysim::systems::ownership::value(world, b.kind);
        if b.owner.is_some() && !b.demolished && value > 0 {
            let afford = world.treasury().is_some_and(|t| t.coins >= value);
            let button = egui::Button::new(format!("Nationalise ({value}¢)"));
            if ui.add_enabled(afford, button).on_disabled_hover_text("the Treasury cannot pay").clicked() {
                app.cmds.push(PlayerCommand::Nationalise(id));
            }
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

/// D19: the Security corp guarding this building, its price, and its guards.
fn security(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &Building) {
    let Some(seller) = b.secured_by else { return };
    let Some(c) = world.comp::<Corp>(seller) else { return };
    let until = c.contracts.iter().find(|(client, _)| *client == id).map(|&(_, t)| t);
    ui.horizontal_wrapped(|ui| {
        ui.label("Secured by");
        let name = RichText::new(&c.name).color(crate::ui::corp_colour(world.corp_index(seller)));
        if ui.link(name).clicked() {
            app.selected = Some(seller);
            app.follow = false;
        }
        let price = citysim::systems::corps::contract_price(world, seller);
        let till = until.map_or(String::new(), |t| format!(" until day {}", time::day(t)));
        ui.label(format!("{price}¢/day{till}"));
    });
    // The seller's guards: the staff of its Security Offices.
    let guards: Vec<EntityId> = c
        .buildings
        .iter()
        .filter(|&&o| world.comp::<Building>(o).is_some_and(|bd| bd.kind == BuildingKind::SecurityOffice))
        .flat_map(|&o| workers(world, o).into_iter().map(|(g, _)| g))
        .collect();
    if !guards.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.small("guards");
            for g in guards {
                agent_link(ui, app, world, g);
            }
        });
    }
}
