//! M13 § 9: the asset panel (right), opened from any asset link: what the
//! thing is, who owns it, where it stands, what it is worth and costs, and
//! its finance and flags. Also the shared asset link and summary helpers.

use egui_macroquad::egui::{self, Color32, RichText, Ui};

use citysim::{time, Asset, AssetLoc, EntityId, World};

use crate::App;

const RED: Color32 = Color32::from_rgb(220, 60, 60);
const CYAN: Color32 = Color32::from_rgb(0x40, 0xc8, 0xe0);

/// A clickable asset, `Car T2 (cond 80)`; clicking opens the asset panel.
pub fn asset_link(ui: &mut Ui, app: &mut App, world: &World, a: EntityId) {
    let Some(x) = world.comp::<Asset>(a) else {
        ui.label(format!("#{}", a.index));
        return;
    };
    let mut text = RichText::new(format!("{} T{} · {}%", x.kind.label(), x.tier, x.condition)).color(CYAN);
    if x.condition == 0 || x.bricked || x.stolen {
        text = text.color(RED);
    }
    if ui.link(text).clicked() {
        app.selected = Some(a);
        app.follow = false;
    }
}

/// A clickable agent or building by id (anything with a name).
pub fn holder_link(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    if world.is_alive(id) {
        if ui.link(world.name_of(id)).clicked() {
            app.selected = Some(id);
            app.follow = false;
        }
    } else {
        ui.label(format!("#{} (gone)", id.index));
    }
}

/// M13 § 9, in the building panel: assets parked, posted or in stock here
/// (as links), a Clinic's installs or a Garage's sales over 7 days, a
/// Hideout's or Market's Stims and Parts, a Bar's registered dealers.
pub fn building_section(ui: &mut Ui, app: &mut App, world: &World, id: EntityId, b: &citysim::Building) {
    use citysim::systems::assets;
    use citysim::{BuildingKind, Good};
    let here = assets::assets_at(world, id);
    let pick = |f: fn(&AssetLoc) -> bool| -> Vec<EntityId> {
        here.iter().copied().filter(|&a| world.comp::<Asset>(a).is_some_and(|x| f(&x.loc))).collect()
    };
    let groups: [(&str, Vec<EntityId>); 3] = [
        ("Parked", pick(|l| matches!(l, AssetLoc::Parked(_)))),
        ("Posted", pick(|l| matches!(l, AssetLoc::Posted(_)))),
        ("In stock", pick(|l| matches!(l, AssetLoc::Stock(_)))),
    ];
    let seller = matches!(b.kind, BuildingKind::Clinic | BuildingKind::Garage);
    let any = groups.iter().any(|(_, v)| !v.is_empty());
    let goods = matches!(b.kind, BuildingKind::Hideout | BuildingKind::Market);
    let dealers = world.dealers.get(&id).filter(|d| !d.is_empty());
    if !any && !seller && !goods && dealers.is_none() {
        return;
    }
    egui::CollapsingHeader::new("Assets").default_open(true).show(ui, |ui| {
        if seller {
            let week: u32 = b.asset_sales.iter().map(|&n| u32::from(n)).sum::<u32>() + u32::from(b.asset_sales_today);
            let what = if b.kind == BuildingKind::Clinic { "installs" } else { "sales" };
            ui.label(format!("{what} over 7 days: {week} ({} today)", b.asset_sales_today));
            if b.kind == BuildingKind::Clinic {
                let week: u32 = world.stats.history.iter().rev().take(7).map(|r| r.treatments).sum::<u32>()
                    + world.stats.current.treatments;
                ui.label(format!("treatments, city-wide, 7 days: {week}"));
            }
        }
        for (title, list) in &groups {
            if list.is_empty() {
                continue;
            }
            ui.label(format!("{title} ({})", list.len()));
            ui.horizontal_wrapped(|ui| {
                for &a in list.iter().take(24) {
                    asset_link(ui, app, world, a);
                }
                if list.len() > 24 {
                    ui.label(format!("and {} more", list.len() - 24));
                }
            });
        }
        if goods {
            for g in [Good::Stims, Good::Parts] {
                let (n, cap) = (world.stock(id, g), world.goods_cap(id, g));
                if cap > 0 {
                    super::building::stock_bar(ui, g.label(), n, cap);
                } else {
                    ui.label(format!("{} {n} (no stock room)", g.label()));
                }
            }
        }
        if let Some(d) = dealers {
            ui.label("dealers registered:");
            ui.horizontal_wrapped(|ui| {
                for &a in d {
                    holder_link(ui, app, world, a);
                }
            });
        }
    });
}

/// Where an asset is, in words, with the holder as a link.
fn loc_line(ui: &mut Ui, app: &mut App, world: &World, loc: AssetLoc) {
    ui.horizontal_wrapped(|ui| {
        let word = match loc {
            AssetLoc::Parked(_) => "parked at",
            AssetLoc::InUse(_) => "driven by",
            AssetLoc::Carried(_) => "carried by",
            AssetLoc::Installed(_) => "installed in",
            AssetLoc::Posted(_) => "posted at",
            AssetLoc::Stock(_) => "for sale at",
            AssetLoc::Limbo(_) => "in limbo (an Abducted hole)",
        };
        ui.label(word);
        if let Some(h) = loc.holder() {
            holder_link(ui, app, world, h);
        }
    });
}

pub fn draw(ui: &mut Ui, app: &mut App, world: &World, id: EntityId) {
    let Some(a) = world.comp::<Asset>(id) else {
        ui.label("Selection is gone.");
        app.selected = None;
        return;
    };
    let a = a.clone();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(format!("{} T{}", a.kind.label(), a.tier));
        ui.label(format!("asset id {id} · class {:?}", a.kind.class()));
        ui.horizontal_wrapped(|ui| {
            ui.label("owner");
            super::inspector::owner_link(ui, app, world, a.owner);
        });
        loc_line(ui, app, world, a.loc);
        if let Some(k) = a.keeper {
            ui.horizontal_wrapped(|ui| {
                ui.label("keeper");
                holder_link(ui, app, world, k);
            });
        }
        let cond = format!("condition {}/100", a.condition);
        if a.condition == 0 {
            ui.colored_label(RED, format!("{cond} · {}", if a.kind.is_implant() { "failed" } else { "wrecked" }));
        } else {
            ui.label(cond);
        }
        ui.label(format!("value {}¢ (list {}¢) · upkeep {}¢/day", a.value, a.list, a.upkeep_per_day));
        if a.upkeep_arrears > 0 {
            ui.colored_label(RED, format!("upkeep in arrears {} days", a.upkeep_arrears));
        }
        match &a.finance {
            Some(f) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("finance: {}¢ left at {}¢/day · lender", f.remaining, f.per_day));
                    match f.lender {
                        Some(l) => super::inspector::owner_link(ui, app, world, Some(l)),
                        None => {
                            ui.label("the city");
                        }
                    }
                });
                if f.arrears > 0 {
                    ui.colored_label(RED, format!("finance arrears {} days", f.arrears));
                }
            }
            None => {
                ui.label("no finance");
            }
        }
        ui.label(format!("bought day {}", time::day(a.bought)));
        ui.horizontal_wrapped(|ui| {
            if a.stolen {
                ui.colored_label(RED, "stolen");
            }
            if a.bricked {
                ui.colored_label(RED, "bricked (locked by its lender)");
            }
            if !a.stolen && !a.bricked {
                ui.label("clean title");
            }
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui.button("Centre camera").clicked() {
                if let Some(t) = citysim::systems::assets::asset_tile(world, id) {
                    app.camera.centre_on(t);
                }
            }
            if ui.button("Wreck").clicked() {
                app.cmds.push(citysim::PlayerCommand::Wreck(id));
            }
            if ui.button("Close").clicked() {
                app.selected = None;
            }
        });
    });
}
