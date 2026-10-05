//! City panel (left, 300 px): population, food, price sparkline, treasury,
//! the last 30 days of deaths, crimes, births and migration, and the lever
//! widgets.

use egui_macroquad::egui::{self, Color32, Ui};

use citysim::{Building, BuildingKind, EventKind, PlayerCommand, Rect, World};

use crate::App;

pub const CITY_W: f32 = 300.0;

/// Lever widget state: sliders edit these and a button commits them, so a
/// drag does not spam the command log.
#[derive(Clone, Debug)]
pub struct CityState {
    pub tax_rate: f32,
    pub sentence_mult: f32,
    pub guard_count: u8,
    pub immigration_per_week: u8,
    pub dole_per_day: u8,
    /// M9: `None` = the captain decides.
    pub law_pin: Option<citysim::Posture>,
    pub release_amount: u32,
    pub build_rect: Rect,
    pub synced: bool,
}

impl Default for CityState {
    fn default() -> Self {
        CityState {
            tax_rate: 0.05,
            sentence_mult: 1.0,
            guard_count: 10,
            immigration_per_week: 2,
            dole_per_day: 3,
            law_pin: None,
            release_amount: 500,
            build_rect: Rect { x: 40, y: 40, w: 5, h: 4 },
            synced: false,
        }
    }
}

fn sum_last_30(world: &World, f: impl Fn(&citysim::DayRow) -> u32) -> u32 {
    let f = &f;
    world.stats.history.iter().rev().take(30).map(f).sum::<u32>() + f(&world.stats.current)
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World) {
    if !app.city.synced {
        app.city.tax_rate = world.levers.tax_rate;
        app.city.sentence_mult = world.levers.sentence_mult;
        app.city.guard_count = world.levers.guard_count;
        app.city.immigration_per_week = world.levers.immigration_per_week;
        app.city.dole_per_day = world.levers.dole_per_day;
        app.city.law_pin = world.law().and_then(|l| l.pinned);
        app.city.synced = true;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("City");
        let s = &world.stats.current;
        egui::Grid::new("city_pop").striped(true).show(ui, |ui| {
            ui.label("Population");
            ui.label(format!("{}", world.population()));
            ui.end_row();
            ui.label("Employed");
            ui.label(format!("{}", s.employed));
            ui.end_row();
            ui.label("Homeless");
            ui.label(format!("{}", s.homeless));
            ui.end_row();
            ui.label("Jailed");
            ui.label(format!("{}", s.jailed));
            ui.end_row();
            ui.label("Gang");
            ui.label(format!("{}", s.gang_members));
            ui.end_row();
            ui.label("Tiers");
            ui.label(format!(
                "Full {} · Coarse {} · Stat {}",
                world.tier(citysim::Lod::Full).len(),
                world.tier(citysim::Lod::Coarse).len(),
                world.tier(citysim::Lod::Statistical).len()
            ));
            ui.end_row();
            ui.label("Open holes");
            ui.label(format!("{}", world.holes.len()));
            ui.end_row();
            ui.label("Ticks/s");
            ui.label(format!("{:.0}", app.ticks_per_sec));
            ui.end_row();
        });

        ui.separator();
        ui.strong("Gangs");
        egui::Grid::new("city_gangs").striped(true).show(ui, |ui| {
            ui.strong("gang");
            ui.strong("heads");
            ui.strong("blocks");
            ui.strong("¢");
            ui.strong("order");
            ui.end_row();
            for (i, gid) in world.gangs().into_iter().enumerate() {
                let Some(g) = world.comp::<citysim::Gang>(gid) else { continue };
                if ui.link(egui::RichText::new(&g.name).color(crate::ui::gang_colour(i))).clicked() {
                    app.selected = Some(g.hideout);
                    app.follow = false;
                }
                ui.label(format!("{}", g.members.len()));
                ui.label(format!("{}", g.territory.len()));
                ui.label(format!("{}¢", g.treasury));
                let order = if g.is_sacked(world.tick) { "sacked".to_string() } else { g.order.to_string() };
                ui.label(order);
                ui.end_row();
            }
            if let Some(law) = world.law() {
                if ui.link(egui::RichText::new("The Law").color(Color32::from_rgb(120, 170, 220))).clicked() {
                    app.selected = world.building_of_kind(BuildingKind::Jail);
                    app.follow = false;
                }
                let guards = citysim::systems::law_brain::guards(world).len();
                ui.label(format!("{guards}"));
                ui.label("");
                ui.label(format!("{}¢", world.treasury().map_or(0, |t| t.coins)));
                let on = law
                    .target
                    .and_then(|g| world.comp::<citysim::Gang>(g))
                    .map_or(String::new(), |g| format!(" on {}", g.name));
                let pin = if law.pinned.is_some() { " (pinned)" } else { "" };
                ui.label(format!("{}{on}{pin}", law.posture));
                ui.end_row();
            }
        });

        ui.separator();
        ui.strong("Food");
        let (mut market, mut warehouse, mut pantry) = (0u32, 0u32, 0u32);
        for id in world.with::<Building>() {
            let Some(b) = world.comp::<Building>(id) else { continue };
            match b.kind {
                BuildingKind::Market => market += b.stock_food,
                BuildingKind::Warehouse => warehouse += b.stock_food,
                BuildingKind::Home => pantry += b.stock_food,
                _ => {}
            }
        }
        ui.label(format!(
            "Street Markets {market} · Reserve Depot {warehouse} · Pantries {pantry} · Sum {}",
            market + warehouse + pantry
        ));
        let price = world.mean_price();
        ui.label(format!("Price {price}¢ · Treasury {}¢", world.treasury().map_or(0, |t| t.coins)));
        sparkline(ui, world);

        ui.separator();
        ui.strong("Last 30 days");
        egui::Grid::new("city_30").striped(true).show(ui, |ui| {
            ui.label("Deaths");
            ui.label(format!(
                "starvation {} · old age {} · violence {}",
                sum_last_30(world, |r| r.deaths_starvation),
                sum_last_30(world, |r| r.deaths_old_age),
                sum_last_30(world, |r| r.deaths_violence)
            ));
            ui.end_row();
            ui.label("Crimes");
            ui.label(format!(
                "thefts {} · arrests {} · {}",
                sum_last_30(world, |r| r.thefts),
                sum_last_30(world, |r| r.arrests),
                crimes_by_kind(world)
            ));
            ui.end_row();
            ui.label("Births");
            ui.label(format!("{}", sum_last_30(world, |r| r.births)));
            ui.end_row();
            ui.label("Migration");
            ui.label(format!(
                "in {} · out {}",
                sum_last_30(world, |r| r.immigrants),
                sum_last_30(world, |r| r.emigrants)
            ));
            ui.end_row();
        });

        ui.separator();
        ui.strong("Levers");
        let c = &mut app.city;
        ui.add(egui::Slider::new(&mut c.tax_rate, 0.0..=0.3).text("tax rate"));
        ui.add(egui::Slider::new(&mut c.sentence_mult, 0.5..=3.0).text("sentence ×"));
        ui.add(egui::Slider::new(&mut c.guard_count, 0..=60).text("guards"));
        ui.add(egui::Slider::new(&mut c.immigration_per_week, 0..=30).text("immigrants / week"));
        ui.add(egui::Slider::new(&mut c.dole_per_day, 0..=10).text("dole / day"));
        ui.horizontal(|ui| {
            ui.label("law posture");
            let name = |p: Option<citysim::Posture>| p.map_or("Auto (captain)".to_string(), |p| p.to_string());
            egui::ComboBox::from_id_salt("law_pin").selected_text(name(c.law_pin)).show_ui(ui, |ui| {
                ui.selectable_value(&mut c.law_pin, None, name(None));
                for p in citysim::Posture::ALL {
                    ui.selectable_value(&mut c.law_pin, Some(p), name(Some(p)));
                }
            });
        });
        if ui.button("Apply levers").clicked() {
            let l = &world.levers;
            if (c.tax_rate - l.tax_rate).abs() > 1e-4 {
                app.cmds.push(PlayerCommand::SetTaxRate(c.tax_rate));
            }
            if (c.sentence_mult - l.sentence_mult).abs() > 1e-4 {
                app.cmds.push(PlayerCommand::SetSentenceMult(c.sentence_mult));
            }
            if c.guard_count != l.guard_count {
                app.cmds.push(PlayerCommand::SetGuardCount(c.guard_count));
            }
            if c.immigration_per_week != l.immigration_per_week {
                app.cmds.push(PlayerCommand::SetImmigrationPerWeek(c.immigration_per_week));
            }
            if c.dole_per_day != l.dole_per_day {
                app.cmds.push(PlayerCommand::SetDolePerDay(c.dole_per_day));
            }
            if c.law_pin != world.law().and_then(|l| l.pinned) {
                app.cmds.push(PlayerCommand::SetLawPosture(c.law_pin));
            }
        }
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut c.release_amount).range(1..=3000));
            if ui.button("Release reserve").clicked() {
                app.cmds.push(PlayerCommand::ReleaseReserve { amount: c.release_amount });
            }
        });
        ui.horizontal(|ui| {
            ui.label("Block at");
            ui.add(egui::DragValue::new(&mut c.build_rect.x).range(0..=90));
            ui.add(egui::DragValue::new(&mut c.build_rect.y).range(0..=60));
            ui.add(egui::DragValue::new(&mut c.build_rect.w).range(4..=6));
            ui.add(egui::DragValue::new(&mut c.build_rect.h).range(4..=6));
            if ui.button("Build (200)").clicked() {
                app.cmds.push(PlayerCommand::BuildHome { rect: c.build_rect });
            }
        });
        if let Some(sel) = app.selected {
            let own_home = world.comp::<Building>(sel).filter(|b| b.kind == BuildingKind::Home).map(|_| sel);
            if let Some(home) = own_home.or_else(|| world.comp::<citysim::Household>(sel).and_then(|h| h.home)) {
                if ui.button(format!("Demolish Block#{}", home.index)).clicked() {
                    app.cmds.push(PlayerCommand::DemolishHome(home));
                }
            }
        }
    });
}

/// Price over the last 30 days, drawn with the painter.
fn sparkline(ui: &mut Ui, world: &World) {
    let prices: Vec<f32> = world.stats.history.iter().rev().take(30).map(|r| r.price as f32).collect::<Vec<_>>();
    let prices: Vec<f32> = prices.into_iter().rev().collect();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(260.0, 40.0), egui::Sense::hover());
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
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.5_f32, Color32::from_rgb(217, 162, 61))));
}

fn crimes_by_kind(world: &World) -> String {
    let since = world.tick.saturating_sub(30 * citysim::TICKS_PER_DAY);
    let mut n = [0u32; 4];
    for e in world.events.iter().filter(|e| e.tick >= since) {
        match e.kind {
            EventKind::Theft => n[0] += 1,
            EventKind::Extortion => n[1] += 1,
            EventKind::Assault => n[2] += 1,
            EventKind::Murder => n[3] += 1,
            _ => {}
        }
    }
    format!("logged: theft {} extortion {} assault {} murder {}", n[0], n[1], n[2], n[3])
}
