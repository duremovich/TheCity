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
    /// M11 levers: city rent per tier, the rent cap, no city evictions, and
    /// the corp panel's Subsidise amount.
    pub city_rent: [i64; 3],
    pub rent_cap_on: bool,
    pub rent_cap: i64,
    pub no_city_evictions: bool,
    pub subsidise_amount: i64,
    /// M12 D42: Sanitation headcount, the riot response pin (`None` = the
    /// captain), and the District panel's per-district weights.
    pub sanitation_count: u8,
    pub riot_response: Option<citysim::RiotResponse>,
    pub guard_weight: [f32; citysim::MAX_DISTRICTS],
    pub sanitation_weight: [f32; citysim::MAX_DISTRICTS],
    /// M13 D48: the Assets levers.
    pub stims_legal: bool,
    pub impound: bool,
    pub asset_tax: [f32; citysim::AssetClass::ALL.len()],
    /// M14 V42: the Virt levers (city ICE 0-3, the Data tax, the hack sentences in days).
    pub city_ice: u8,
    pub data_tax: f32,
    pub hack_sentence_days: [u16; 2],
    /// M15 W42: the news tax slider.
    pub news_tax: f32,
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
            city_rent: [1, 2, 4],
            rent_cap_on: false,
            rent_cap: 4,
            no_city_evictions: false,
            subsidise_amount: 1000,
            sanitation_count: 0,
            riot_response: None,
            guard_weight: [1.0; citysim::MAX_DISTRICTS],
            sanitation_weight: [1.0; citysim::MAX_DISTRICTS],
            stims_legal: false,
            impound: true,
            asset_tax: [0.0; citysim::AssetClass::ALL.len()],
            city_ice: 2,
            data_tax: 0.0,
            hack_sentence_days: [2, 5],
            news_tax: 0.0,
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
        app.city.city_rent = world.levers.city_rent;
        app.city.rent_cap_on = world.levers.rent_cap.is_some();
        app.city.rent_cap = world.levers.rent_cap.unwrap_or(4);
        app.city.no_city_evictions = world.levers.no_city_evictions;
        app.city.sanitation_count = world.levers.sanitation_count;
        app.city.riot_response = world.levers.riot_response;
        app.city.guard_weight = world.levers.guard_weight;
        app.city.sanitation_weight = world.levers.sanitation_weight;
        app.city.stims_legal = world.levers.stims_legal;
        app.city.impound = world.levers.impound;
        app.city.asset_tax = world.levers.asset_tax;
        app.city.city_ice = world.levers.city_ice;
        app.city.data_tax = world.levers.data_tax;
        app.city.news_tax = world.levers.news_tax;
        let ext = &world.config.crime.sentence_days_ext;
        app.city.hack_sentence_days = [
            world.levers.hack_sentence_days[0].unwrap_or(ext.intrusion as u16),
            world.levers.hack_sentence_days[1].unwrap_or(ext.data_theft as u16),
        ];
        app.city.synced = true;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("City");
        // Employed, homeless, jailed and gang are end-of-day snapshots: read
        // the last closed day (today's row is zero until midnight).
        let s = world.stats.history.back().unwrap_or(&world.stats.current);
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

        corps_section(ui, app, world);
        living_section(ui, world);
        classes_section(ui, world);
        districts_section(ui, app, world);
        assets_section(ui, app, world);
        virt_section(ui, app, world);
        super::word::city(ui, app, world);

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
        // M11 § 8: rent on city Blocks per tier, a cap on every Block, and
        // the city's own evictions.
        ui.horizontal(|ui| {
            ui.label("city rent");
            for (i, tier) in ["Sump", "Mid", "Spire"].iter().enumerate() {
                ui.add(egui::DragValue::new(&mut c.city_rent[i]).range(0..=20).prefix(format!("{tier} ")));
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.rent_cap_on, "rent cap");
            ui.add_enabled(c.rent_cap_on, egui::Slider::new(&mut c.rent_cap, 0..=20).suffix("¢"));
        });
        ui.checkbox(&mut c.no_city_evictions, "the city never evicts");
        // M12 D42: Sanitation headcount and the riot response pin.
        ui.add(egui::Slider::new(&mut c.sanitation_count, 0..=60).text("sanitation"));
        ui.horizontal(|ui| {
            ui.label("riot response");
            let combo =
                egui::ComboBox::from_id_salt("riot_response").selected_text(riot_response_label(c.riot_response));
            combo.show_ui(ui, |ui| {
                for r in [
                    None,
                    Some(citysim::RiotResponse::Contain),
                    Some(citysim::RiotResponse::Disperse),
                    Some(citysim::RiotResponse::Crush),
                ] {
                    ui.selectable_value(&mut c.riot_response, r, riot_response_label(r));
                }
            });
        });
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
            if c.city_rent != l.city_rent {
                app.cmds.push(PlayerCommand::SetCityRent(c.city_rent));
            }
            let cap = c.rent_cap_on.then_some(c.rent_cap);
            if cap != l.rent_cap {
                app.cmds.push(PlayerCommand::SetRentCap(cap));
            }
            if c.no_city_evictions != l.no_city_evictions {
                app.cmds.push(PlayerCommand::NoCityEvictions(c.no_city_evictions));
            }
            if c.sanitation_count != l.sanitation_count {
                app.cmds.push(PlayerCommand::SetSanitation(c.sanitation_count));
            }
            if c.riot_response != l.riot_response {
                app.cmds.push(PlayerCommand::SetRiotResponse(c.riot_response));
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
            // Nationalise the selected building (Subsidise and BreakUp live
            // on the corp panel).
            let owned = world.comp::<Building>(sel).filter(|b| b.owner.is_some() && !b.demolished);
            if let Some(b) = owned {
                let price = citysim::systems::ownership::value(world, b.kind);
                if price > 0 && ui.button(format!("Nationalise {} ({price}¢)", world.name_of(sel))).clicked() {
                    app.cmds.push(PlayerCommand::Nationalise(sel));
                }
            }
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

/// One row per corp (name, niches, order, treasury, price levels, building
/// count) and a share bar per niche (D16, `corp_brain::shares`).
fn corps_section(ui: &mut Ui, app: &mut App, world: &World) {
    use citysim::{Corp, Niche};
    let corps = world.corps();
    ui.separator();
    ui.strong(format!("Corps ({})", corps.len()));
    egui::Grid::new("city_corps").striped(true).show(ui, |ui| {
        for h in ["corp", "order", "¢", "price", "bld"] {
            ui.strong(h);
        }
        ui.end_row();
        for (i, &cid) in corps.iter().enumerate() {
            let Some(c) = world.comp::<Corp>(cid) else { continue };
            let niches: Vec<&str> = c.niches.iter().map(|n| n.label()).collect();
            let link = ui.link(egui::RichText::new(&c.name).color(crate::ui::corp_colour(i)));
            if link.on_hover_text(niches.join(" + ")).clicked() {
                app.selected = Some(cid);
                app.follow = false;
            }
            ui.label(c.order.to_string());
            if c.treasury < 0 {
                ui.colored_label(Color32::from_rgb(220, 60, 60), format!("{}", c.treasury));
            } else {
                ui.label(format!("{}", c.treasury));
            }
            let levels: Vec<String> = c.niches.iter().map(|&n| format!("{:.1}", c.level(n))).collect();
            ui.label(levels.join("/"));
            ui.label(format!("{}", c.buildings.len()));
            ui.end_row();
        }
    });
    ui.small("price: level per niche · hover a name for its niches");
    for niche in Niche::ALL {
        let shares = citysim::systems::corp_brain::shares(world, niche);
        ui.horizontal(|ui| {
            ui.label(format!("{:<8}", niche.label()));
            share_bar(ui, world, &corps, &shares);
        });
    }
}

/// A stacked bar: each corp's share in its colour, the rest (city, agents,
/// gangs) grey.
fn share_bar(
    ui: &mut Ui,
    world: &World,
    corps: &[citysim::EntityId],
    shares: &std::collections::BTreeMap<citysim::EntityId, f32>,
) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(200.0, 12.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, Color32::from_gray(60));
    let mut x = rect.left();
    let mut tip: Vec<String> = Vec::new();
    for (i, cid) in corps.iter().enumerate() {
        let Some(&s) = shares.get(cid).filter(|&&s| s > 0.0) else { continue };
        let w = s.clamp(0.0, 1.0) * rect.width();
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x, rect.top()), egui::pos2(x + w, rect.bottom())),
            0.0,
            crate::ui::corp_colour(i),
        );
        x += w;
        tip.push(format!("{} {:.0}%", world.owner_label(Some(*cid)), s * 100.0));
    }
    if shares.is_empty() {
        resp.on_hover_text("no market yet");
    } else {
        resp.on_hover_text(tip.join(" · "));
    }
}

/// Corp, Street and Dreg: count, happiness, loyalty, submission, unrest
/// (§ 7, computed at midnight).
fn classes_section(ui: &mut Ui, world: &World) {
    ui.separator();
    ui.strong("Classes");
    egui::Grid::new("city_classes").striped(true).show(ui, |ui| {
        for h in ["class", "n", "happy", "loyal", "submit", "unrest"] {
            ui.strong(h);
        }
        ui.end_row();
        for class in citysim::Class::ALL {
            let a = &world.classes[class.index()];
            ui.label(class.label());
            ui.label(format!("{}", a.count));
            ui.label(format!("{:.2}", a.happiness));
            ui.label(format!("{:.2}", a.loyalty));
            ui.label(format!("{:.2}", a.submission));
            let hot = class == citysim::Class::Street && a.unrest > world.config.classes.strike_threshold;
            if hot {
                ui.colored_label(Color32::from_rgb(220, 60, 60), format!("{:.2}", a.unrest));
            } else {
                ui.label(format!("{:.2}", a.unrest));
            }
            ui.end_row();
        }
    });
    if let Some(t) = world.last_strike {
        ui.small(format!("last strike day {}", citysim::time::day(t)));
    }
}

/// M13 § 9: vehicles by kind, chromed agents, mean sanity, hooked adults,
/// dealer and legal sales today, robots posted; then the levers.
fn assets_section(ui: &mut Ui, app: &mut App, world: &World) {
    use citysim::systems::{assets, stims};
    ui.separator();
    ui.strong("Assets");
    let (chromed, sanity, robots, v) = assets::snapshot(world);
    let hooked = stims::hooked_count(world);
    let today = &world.stats.current;
    egui::Grid::new("city_assets").striped(true).show(ui, |ui| {
        ui.label("Vehicles");
        ui.label(format!("bikes {} · cars {} · trucks {} · flyers {}", v[0], v[1], v[2], v[3]));
        ui.end_row();
        ui.label("Chromed agents");
        ui.label(format!("{chromed}"));
        ui.end_row();
        ui.label("Mean sanity");
        ui.label(format!("{sanity:.2}"));
        ui.end_row();
        ui.label("Hooked adults");
        ui.label(format!("{hooked}"));
        ui.end_row();
        ui.label("Stims sold today");
        ui.label(format!("dealer {} · legal {}", today.stims_dealt, today.stims_legal));
        ui.end_row();
        ui.label("Robots posted");
        ui.label(format!("{robots}"));
        ui.end_row();
    });
    ui.small("Levers");
    if ui.checkbox(&mut app.city.stims_legal, "Stims legal (Markets stock and sell them)").changed() {
        app.cmds.push(PlayerCommand::SetStimsLegal(app.city.stims_legal));
    }
    if ui.checkbox(&mut app.city.impound, "Impound vehicles with unpaid upkeep").changed() {
        app.cmds.push(PlayerCommand::SetImpound(app.city.impound));
    }
    for class in citysim::AssetClass::ALL {
        let slot = &mut app.city.asset_tax[class.index()];
        let r = ui.add(egui::Slider::new(slot, 0.0..=5.0).text(format!("{class:?} tax")));
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            app.cmds.push(PlayerCommand::SetAssetTax { kind: class, rate: *slot });
        }
    }
}

/// M14 § 10: runs, Data and the dead today and yesterday, the mean ICE by
/// owner type, the raids (LIVE when streamed), then the levers.
fn virt_section(ui: &mut Ui, app: &mut App, world: &World) {
    use citysim::systems::virt;
    use citysim::virt::{NodeId, NodeKind, OwnerTag};
    if !virt::enabled(world) {
        return;
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.strong("Virt");
        ui.small(if app.show_virt { "overlay on (N)" } else { "N draws the plane" });
    });
    let today = &world.stats.current.virt;
    let yday = world.stats.history.back().map(|r| r.virt.clone()).unwrap_or_default();
    let share = |v: &citysim::stats::VirtCols| {
        if v.runs == 0 {
            "-".to_string()
        } else {
            format!("{:.0}%", 100.0 * v.runs_ok as f32 / v.runs as f32)
        }
    };
    egui::Grid::new("city_virt").striped(true).show(ui, |ui| {
        ui.strong("");
        ui.strong("today");
        ui.strong("yesterday");
        ui.end_row();
        let rows: [(&str, String, String); 6] = [
            ("Runs", today.runs.to_string(), yday.runs.to_string()),
            ("Success share", share(today), share(&yday)),
            (
                "Data made / stolen",
                format!("{} / {}", today.data_made, today.data_stolen),
                format!("{} / {}", yday.data_made, yday.data_stolen),
            ),
            ("Data sold", today.data_sold.to_string(), yday.data_sold.to_string()),
            ("Fried", today.fried.to_string(), yday.fried.to_string()),
            ("Flatlined", today.flatlined.to_string(), yday.flatlined.to_string()),
        ];
        for (k, a, b) in rows {
            ui.label(k);
            ui.label(a);
            ui.label(b);
            ui.end_row();
        }
    });
    // Mean effective ICE over the alive guardable nodes, by owner type.
    let mut sum = [0u32; 3];
    let mut count = [0u32; 3];
    for (i, n) in world.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        if matches!(n.kind, NodeKind::Public(_)) {
            continue;
        }
        let k = match n.owner_kind {
            OwnerTag::City => 0,
            OwnerTag::Corp => 1,
            OwnerTag::Gang => 2,
        };
        sum[k] += u32::from(virt::ice_eff(world, NodeId(i as u16)));
        count[k] += 1;
    }
    let mean =
        |k: usize| if count[k] == 0 { "-".to_string() } else { format!("{:.2}", sum[k] as f32 / count[k] as f32) };
    ui.label(format!(
        "Mean ICE: city {} · corps {} · gangs {} ({} nodes)",
        mean(0),
        mean(1),
        mean(2),
        world.virt.alive_count()
    ));
    let raids = crate::mission::raids(world);
    if !raids.is_empty() {
        ui.small("Raids");
        for g in raids {
            ui.horizontal(|ui| {
                let name = world.comp::<citysim::Gang>(g).map_or(String::new(), |x| x.name.clone());
                if ui.link(name).clicked() {
                    app.selected = None;
                    app.selected_run = None;
                    app.selected_mission = Some(g);
                }
                if crate::mission::build(world, g).is_some() {
                    ui.colored_label(Color32::from_rgb(80, 170, 90), "LIVE");
                }
            });
        }
    }
    ui.small("Levers");
    let r = ui.add(egui::Slider::new(&mut app.city.city_ice, 0..=3).text("City ICE (Treasury, Precinct)"));
    if r.drag_stopped() || (r.changed() && !r.dragged()) {
        app.cmds.push(PlayerCommand::SetCityIce(app.city.city_ice));
    }
    let r = ui.add(egui::Slider::new(&mut app.city.data_tax, 0.0..=1.0).text("Data tax"));
    if r.drag_stopped() || (r.changed() && !r.dragged()) {
        app.cmds.push(PlayerCommand::SetDataTax(app.city.data_tax));
    }
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.city.hack_sentence_days[0]).range(1..=365).suffix(" d"));
        ui.label("Intrusion");
        ui.add(egui::DragValue::new(&mut app.city.hack_sentence_days[1]).range(1..=365).suffix(" d"));
        ui.label("Data Theft");
        if ui.button("Set").clicked() {
            // Only a changed sentence is sent (an untouched crime stays unpinned).
            let ext = &world.config.crime.sentence_days_ext;
            let current = [
                world.levers.hack_sentence_days[0].unwrap_or(ext.intrusion as u16),
                world.levers.hack_sentence_days[1].unwrap_or(ext.data_theft as u16),
            ];
            for (i, crime) in [citysim::Crime::Intrusion, citysim::Crime::DataTheft].into_iter().enumerate() {
                if app.city.hack_sentence_days[i] != current[i] {
                    app.cmds.push(PlayerCommand::SetHackSentence { crime, days: app.city.hack_sentence_days[i] });
                }
            }
        }
    });
}

/// The riot response combo's text.
pub fn riot_response_label(r: Option<citysim::RiotResponse>) -> String {
    r.map_or("Auto (captain)".to_string(), |r| format!("{r:?}"))
}

/// M12 § 8: one row per district (controller, guards, unrest, litter, crime
/// rate, a live riot); a row click opens the District panel.
fn districts_section(ui: &mut Ui, app: &mut App, world: &World) {
    const RED: Color32 = Color32::from_rgb(220, 60, 60);
    ui.separator();
    ui.strong("Districts");
    let hot = world.config.riots.riot_threshold;
    egui::Grid::new("city_districts").striped(true).spacing([6.0, 2.0]).show(ui, |ui| {
        for h in ["district", "control", "grd", "unrest", "litter", "crime"] {
            ui.label(egui::RichText::new(h).small().strong());
        }
        ui.end_row();
        for d in &world.districts {
            let sel = app.selected_district == Some(d.id) && app.selected.is_none();
            let riot = world.riots.iter().any(|r| r.district == d.id);
            let mut name = egui::RichText::new(if riot { format!("{} !", d.name) } else { d.name.clone() }).small();
            if riot {
                name = name.color(RED);
            }
            let row = ui.selectable_label(sel, name);
            if row.on_hover_text(if riot { "a riot under way" } else { "open the District panel" }).clicked() {
                app.selected = None;
                app.selected_district = Some(d.id);
                app.follow = false;
            }
            ui.label(crate::ui::district::controller_text(world, d.control).small());
            ui.small(format!("{}", d.guards));
            let unrest = egui::RichText::new(format!("{:.2}", d.unrest)).small();
            ui.label(if d.unrest > hot { unrest.color(RED) } else { unrest });
            ui.small(format!("{:.2}", d.litter));
            ui.small(format!("{:.1}", d.crime_rate));
            ui.end_row();
        }
    });
    ui.small("! a riot under way · unrest red over the riot threshold");
}

/// L2 § 8: the Economy section (wages, dole, their ratio, employed share,
/// the Treasury against its band, `upkeep_mult`, public works, export) and
/// the Leisure section (fun, satisfied share, visits by kind, the week's
/// biggest win). From the last closed day.
fn living_section(ui: &mut Ui, world: &World) {
    if !world.config.living.enabled {
        return;
    }
    let s = world.stats.history.back().unwrap_or(&world.stats.current);
    let l = &s.living;
    ui.separator();
    ui.strong("Economy");
    egui::Grid::new("city_economy").striped(true).show(ui, |ui| {
        ui.label("Wages / dole");
        ui.label(format!("{}¢ / {}¢ ({:.2})", s.flow_wages, s.flow_dole, l.wage_dole_ratio));
        ui.end_row();
        ui.label("Employed share");
        ui.label(format!("{:.0} %", 100.0 * l.employed_share));
        ui.end_row();
        let band = world.config.budget.band;
        ui.label("Treasury (band)");
        ui.label(format!("{}¢ ({}-{})", world.treasury().map_or(0, |t| t.coins), band[0], band[1]));
        ui.end_row();
        ui.label("Upkeep mult");
        ui.label(format!("{:.2}", l.upkeep_mult));
        ui.end_row();
        ui.label("Public works");
        ui.label(format!("{} jobs, {}¢ a day", l.works_jobs, l.flow_public_works));
        ui.end_row();
        ui.label("Export in / minted");
        ui.label(format!("{}¢ / {}¢", l.outside_inbound, l.outside_minted));
        ui.end_row();
    });
    if !citysim::systems::leisure::on(world) {
        return;
    }
    ui.strong("Leisure");
    egui::Grid::new("city_leisure").striped(true).show(ui, |ui| {
        ui.label("Fun (mean, satisfied)");
        ui.label(format!("{:.2}, {:.0} %", l.fun_mean, 100.0 * l.fun_satisfied_share));
        ui.end_row();
        ui.label("Spent");
        ui.label(format!(
            "{}¢ on venues, {}¢ net to the tables, {}¢ in tribute",
            l.flow_leisure, l.flow_gamble, l.flow_tribute
        ));
        ui.end_row();
        for (i, k) in citysim::stats::LEISURE_SLOTS.iter().enumerate() {
            ui.label(format!("visits {k}"));
            ui.label(format!("{} ({} standing)", l.visits[i], l.venues[i]));
            ui.end_row();
        }
        ui.label("HangOuts");
        ui.label(format!("{} (known faces {:.2})", l.hangouts, l.hangout_contacts_mean));
        ui.end_row();
        let week = world.tick.saturating_sub(7 * citysim::TICKS_PER_DAY);
        let best = world
            .events
            .iter()
            .filter(|e| e.kind == citysim::EventKind::Gambled && e.tick >= week)
            .max_by_key(|e| e.text.split_whitespace().find_map(|w| w.parse::<i64>().ok()).unwrap_or(0));
        ui.label("Biggest win (7 days)");
        ui.label(best.map_or("-".to_string(), |e| e.text.clone()));
        ui.end_row();
    });
}
