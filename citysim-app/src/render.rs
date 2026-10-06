//! Draws the world. Takes `&World` only and never mutates it.

#![deny(clippy::needless_pass_by_ref_mut)]

use macroquad::prelude::*;

use citysim::{
    time, Brain, Building, BuildingKind, Corp, Corpse, Gang, Job, Lod, Position, Role, TileKind, TilePos, Wallet,
    World, Zone,
};

use crate::App;

const C_GROUND: u32 = 0x2e2a24;
const C_ROAD: u32 = 0x4a4440;
const C_WALL: u32 = 0x6b6660;
const C_DOOR: u32 = 0x9a7b4f;
const C_FARMLAND: u32 = 0x3f5a2a;
const C_WATER: u32 = 0x27485e;
const C_OUTLINE: u32 = 0xc8c0b0;
/// M10: Lots (open plots) and Security Offices.
const C_LOT: u32 = 0x7a7466;
const C_SECURITY: u32 = 0x5fb3c8;
/// Ground tint per zone (Spire, Civic, Vats, Mid, Sump); Mid is the v1 ground.
const C_ZONE_GROUND: [u32; 5] = [0x2c2f3a, 0x33302a, 0x26301f, 0x2e2a24, 0x2a2420];
const C_CORPSE: u32 = 0x1a1a1a;
const C_AGENT_IDLE: u32 = 0x8a8a8a;
const C_AGENT_WORKING: u32 = 0x3d7bd9;
const C_AGENT_EATING: u32 = 0x4caf50;
const C_AGENT_SLEEPING: u32 = 0x1f2f6b;
const C_AGENT_CRIME: u32 = 0xd92f2f;
const C_AGENT_FLEEING: u32 = 0xf08c1e;
const C_AGENT_GUARDING: u32 = 0xf5f5f5;
const C_AGENT_SOCIAL: u32 = 0xd9a23d;
const C_AGENT_JAILED: u32 = 0x555555;
const C_SELECTION: u32 = 0xffd700;
const C_NIGHT: u32 = 0x0a0f2a;
const NIGHT_ALPHA: f32 = 0.35;
const NIGHT_RAMP_TICKS: f32 = 60.0;
const LABEL_MIN_PX: f32 = 12.0;
/// M12 D43: litter heat by band (littered, trashed, heaped); clean draws nothing.
const C_LITTER: [u32; 3] = [0xbdb76b, 0x8b5a2b, 0x8b1a1a];
/// M12: a derelict's crack, a Hotel's bed.
const C_DERELICT: u32 = 0x9a5a3a;
const C_BED: u32 = 0xd8cfe8;
/// M13 (plan 2.8): vehicles by kind (bike, car, truck, flyer).
const C_VEHICLE: [u32; 4] = [0xe0c040, 0x40c8e0, 0xc87840, 0xe040c0];
/// M12 D43: the district overlay's controller fill (the City's grey fainter).
const DISTRICT_ALPHA: f32 = 0.22;
const DISTRICT_CITY_ALPHA: f32 = 0.12;
const SIGHT_TILES: f32 = 6.0;

fn hex(c: u32) -> Color {
    Color::from_hex(c)
}

/// A gang's map colour, from the shared table in `ui`.
fn gang_colour(index: usize) -> Color {
    hex(crate::ui::gang_hex(index))
}

fn tile_colour(kind: TileKind, zone: Zone) -> Color {
    hex(match kind {
        TileKind::Ground => C_ZONE_GROUND[zone.index()],
        TileKind::Road => C_ROAD,
        TileKind::Wall => C_WALL,
        TileKind::Door => C_DOOR,
        TileKind::Farmland => C_FARMLAND,
        TileKind::Water => C_WATER,
    })
}

pub fn draw(world: &World, app: &App) {
    clear_background(hex(C_GROUND));
    let cam = &app.camera;
    let ppt = cam.px_per_tile;
    // Culling uses the u16 bounds so a whole-map frame reaches column 255.
    let (vx0, vy0, x1, y1) = cam.view_bounds();
    let in_view =
        |t: TilePos| u16::from(t.x) >= vx0 && u16::from(t.x) < x1 && u16::from(t.y) >= vy0 && u16::from(t.y) < y1;
    let rect_in_view = |r: &citysim::Rect| {
        u16::from(r.x) < x1
            && vx0 < u16::from(r.x) + u16::from(r.w)
            && u16::from(r.y) < y1
            && vy0 < u16::from(r.y) + u16::from(r.h)
    };

    // 1. tiles, culled to the view
    for y in vy0..y1 {
        for x in vx0..x1 {
            let p = cam.tile_to_screen(vec2(f32::from(x), f32::from(y)));
            let t = TilePos { x: x as u8, y: y as u8 };
            draw_rectangle(p.x, p.y, ppt + 0.5, ppt + 0.5, tile_colour(world.map.tile_at(t), world.map.zone(t)));
        }
    }

    // 1b. M12 D43: litter heat (`L`): alpha = litter / 255 in the band's colour.
    if app.show_litter && !world.litter.is_empty() {
        let w = world.map.w();
        for y in vy0..y1.min(world.map.h() as u16) {
            for x in vx0..x1.min(w as u16) {
                let v = world.litter.get(usize::from(y) * w + usize::from(x)).copied().unwrap_or(0);
                let band = citysim::systems::litter::band(v);
                if band == 0 {
                    continue;
                }
                let p = cam.tile_to_screen(vec2(f32::from(x), f32::from(y)));
                let c = Color { a: (f32::from(v) / 255.0).max(0.25), ..hex(C_LITTER[band - 1]) };
                draw_rectangle(p.x, p.y, ppt + 0.5, ppt + 0.5, c);
            }
        }
    }

    // 1c. M12 D43: with the borders on (`B`), each district filled in its
    // controller's colour, translucent so the map reads; Contested hatched at
    // 45° (a tile's anti-diagonal on every third diagonal, so they join up).
    if app.show_districts && !world.districts.is_empty() {
        let corps = world.corps();
        let fills: Vec<Option<Color>> = world
            .districts
            .iter()
            .map(|d| {
                let c = match d.control {
                    citysim::Controller::Gang(g) => crate::ui::gang_hex(world.gang_index(g)),
                    citysim::Controller::Corp(k) => {
                        crate::ui::corp_hex(corps.iter().position(|&c| c == k).unwrap_or(0))
                    }
                    citysim::Controller::City => C_OUTLINE,
                    citysim::Controller::Contested => return None,
                };
                let alpha = if d.control == citysim::Controller::City { DISTRICT_CITY_ALPHA } else { DISTRICT_ALPHA };
                Some(Color { a: alpha, ..hex(c) })
            })
            .collect();
        let (w, h) = (world.map.w() as u16, world.map.h() as u16);
        for y in vy0..y1.min(h) {
            for x in vx0..x1.min(w) {
                let d = world.district_of(TilePos { x: x as u8, y: y as u8 });
                let p = cam.tile_to_screen(vec2(f32::from(x), f32::from(y)));
                match fills.get(d.index()) {
                    Some(Some(c)) => draw_rectangle(p.x, p.y, ppt + 0.5, ppt + 0.5, *c),
                    Some(None) if (x + y) % 3 == 0 => {
                        draw_line(p.x, p.y + ppt, p.x + ppt, p.y, 1.0, Color { a: 0.45, ..WHITE });
                    }
                    _ => {}
                }
            }
        }
    }

    // One pass over citizens: badge counts for non-Full agents inside
    // buildings, and the list of Full agents to draw as squares.
    let mut inside = vec![0u16; world.building.len()];
    let mut full: Vec<(citysim::EntityId, TilePos)> = Vec::new();
    for id in world.citizens() {
        let (Some(pos), Some(brain)) = (world.comp::<Position>(id), world.comp::<Brain>(id)) else { continue };
        if brain.lod == Lod::Full {
            if in_view(pos.tile) {
                full.push((id, pos.tile));
            }
        } else if let Some(n) = pos.building.and_then(|b| inside.get_mut(b.index as usize)) {
            *n += 1;
        }
    }

    // 2 + 3. buildings: outline, letter, badge. M11 D43: the outline is the
    // owner's colour (corp, gang, an agent white, the city grey).
    let corps = world.corps();
    for id in world.with::<Building>() {
        let Some(b) = world.comp::<Building>(id) else { continue };
        if !rect_in_view(&b.rect) {
            continue;
        }
        let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
        let (w, h) = (f32::from(b.rect.w) * ppt, f32::from(b.rect.h) * ppt);
        if b.demolished {
            dashed_rect(tl.x, tl.y, w, h, hex(C_OUTLINE));
        } else if b.kind == BuildingKind::Lot {
            // An open plot: dashed, no letter (inert until M11).
            dashed_rect(tl.x, tl.y, w, h, hex(C_LOT));
        } else {
            let city = if b.kind == BuildingKind::SecurityOffice { C_SECURITY } else { C_OUTLINE };
            let outline = match b.owner {
                Some(o) if world.has::<Corp>(o) => crate::ui::corp_hex(corps.iter().position(|&c| c == o).unwrap_or(0)),
                Some(o) if world.has::<Gang>(o) => crate::ui::gang_hex(world.gang_index(o)),
                Some(o) if world.has::<Wallet>(o) => 0xffffff,
                _ => city,
            };
            let mut colour = hex(outline);
            // Spire Blocks a shade brighter.
            if b.kind == BuildingKind::Home && b.tier >= 2 {
                colour =
                    Color::new((colour.r * 1.2).min(1.0), (colour.g * 1.2).min(1.0), (colour.b * 1.2).min(1.0), 1.0);
            }
            draw_rectangle_lines(tl.x, tl.y, w, h, 2.0, colour);
            // M12: a derelict is cracked corner to corner; a Hotel has a bed.
            if b.derelict {
                draw_line(tl.x, tl.y, tl.x + w, tl.y + h, 1.5, hex(C_DERELICT));
                draw_line(tl.x + w, tl.y, tl.x, tl.y + h, 1.5, hex(C_DERELICT));
            }
            if b.kind == BuildingKind::Hotel && ppt >= LABEL_MIN_PX / 2.0 {
                let (bw, bh) = (w * 0.5, (h * 0.12).max(2.0));
                let (bx, by) = (tl.x + (w - bw) / 2.0, tl.y + h * 0.78);
                draw_rectangle(bx, by, bw, bh, hex(C_BED));
                draw_rectangle(bx, by - bh, bw * 0.25, bh, hex(C_BED));
            }
            if ppt >= LABEL_MIN_PX {
                let size = (ppt * 1.4).clamp(14.0, 40.0);
                let text = b.kind.letter().to_string();
                let dims = measure_text(&text, None, size as u16, 1.0);
                draw_text(
                    &text,
                    tl.x + w / 2.0 - dims.width / 2.0,
                    tl.y + h / 2.0 + dims.height / 2.0,
                    size,
                    hex(C_OUTLINE),
                );
            }
        }
        let n = inside.get(id.index as usize).copied().unwrap_or(0);
        if n > 0 {
            let text = n.to_string();
            let size = 14.0;
            let dims = measure_text(&text, None, size as u16, 1.0);
            let (pw, ph) = (dims.width + 8.0, 16.0);
            let (px, py) = (tl.x + w - pw - 1.0, tl.y + 1.0);
            draw_rectangle(px, py, pw, ph, Color::new(0.0, 0.0, 0.0, 0.8));
            draw_text(&text, px + 4.0, py + 12.0, size, WHITE);
        }
    }

    // 3b. territory outlines in the holder's colour; a sacked Hideout is dashed.
    for (i, gid) in world.gangs().into_iter().enumerate() {
        let Some(g) = world.comp::<Gang>(gid) else { continue };
        let colour = gang_colour(i);
        for &h in &g.territory {
            let Some(b) = world.comp::<Building>(h) else { continue };
            if !rect_in_view(&b.rect) {
                continue;
            }
            let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
            let (w, h) = (f32::from(b.rect.w) * ppt, f32::from(b.rect.h) * ppt);
            draw_rectangle_lines(tl.x + 3.0, tl.y + 3.0, w - 6.0, h - 6.0, 1.5, colour);
        }
        if g.is_sacked(world.tick) {
            if let Some(b) = world.comp::<Building>(g.hideout).filter(|b| rect_in_view(&b.rect)) {
                let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
                dashed_rect(tl.x, tl.y, f32::from(b.rect.w) * ppt, f32::from(b.rect.h) * ppt, colour);
            }
        }
    }

    // 4. corpses
    for id in world.with::<Corpse>() {
        let Some(pos) = world.comp::<Position>(id) else { continue };
        if !in_view(pos.tile) {
            continue;
        }
        let p = cam.tile_to_screen(vec2(f32::from(pos.tile.x), f32::from(pos.tile.y)));
        draw_line(p.x + 1.0, p.y + 1.0, p.x + ppt - 1.0, p.y + ppt - 1.0, 2.0, hex(C_CORPSE));
        draw_line(p.x + ppt - 1.0, p.y + 1.0, p.x + 1.0, p.y + ppt - 1.0, 2.0, hex(C_CORPSE));
    }

    // 5. Full agents, coloured by current action (M0: everyone is idle)
    for (id, tile) in full {
        let p = cam.tile_to_screen(vec2(f32::from(tile.x), f32::from(tile.y)));
        let colour = agent_colour(world, id);
        draw_rectangle(p.x + 1.0, p.y + 1.0, ppt - 2.0, ppt - 2.0, colour);
        if let Some(g) = world.gang_of(id) {
            draw_rectangle_lines(p.x + 1.0, p.y + 1.0, ppt - 2.0, ppt - 2.0, 2.0, gang_colour(world.gang_index(g)));
        } else if world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard) {
            draw_rectangle_lines(p.x + 1.0, p.y + 1.0, ppt - 2.0, ppt - 2.0, 1.0, WHITE);
        }
    }

    // 5b. M13 (plan 2.8): parked vehicles as small rects along the door's
    // top edge (at most four a door), drivers' glyphs over their squares,
    // and flyers lerped from where they took off to the door, over walls.
    draw_vehicles(world, app, &in_view);

    // 6. selection: a Full agent's square and sight radius, else the building
    // the selection is (or is inside)
    if let Some(sel) = app.selected {
        if let Some(c) = world.comp::<Corp>(sel) {
            // A selected corp marks every building it owns.
            for &id in &c.buildings {
                let Some(b) = world.comp::<Building>(id).filter(|b| rect_in_view(&b.rect)) else { continue };
                let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
                let (w, h) = (f32::from(b.rect.w) * ppt, f32::from(b.rect.h) * ppt);
                draw_rectangle_lines(tl.x - 2.0, tl.y - 2.0, w + 4.0, h + 4.0, 2.0, hex(C_SELECTION));
            }
        } else if let Some(b) = world.comp::<Building>(sel) {
            let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
            draw_rectangle_lines(
                tl.x,
                tl.y,
                f32::from(b.rect.w) * ppt,
                f32::from(b.rect.h) * ppt,
                3.0,
                hex(C_SELECTION),
            );
        } else if let (Some(pos), Some(brain)) = (world.comp::<Position>(sel), world.comp::<Brain>(sel)) {
            if brain.lod == Lod::Full {
                let p = cam.tile_to_screen(vec2(f32::from(pos.tile.x), f32::from(pos.tile.y)));
                draw_rectangle_lines(p.x, p.y, ppt, ppt, 2.0, hex(C_SELECTION));
                let c = p + vec2(ppt, ppt) * 0.5;
                draw_circle_lines(c.x, c.y, SIGHT_TILES * ppt, 1.0, Color { a: 0.4, ..hex(C_SELECTION) });
            } else if let Some(b) = pos.building.and_then(|b| world.comp::<Building>(b)) {
                let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
                draw_rectangle_lines(
                    tl.x,
                    tl.y,
                    f32::from(b.rect.w) * ppt,
                    f32::from(b.rect.h) * ppt,
                    2.0,
                    hex(C_SELECTION),
                );
            }
        }
    }

    // 6b. M12 D43: district borders, a line on every tile edge between two
    // districts; the selected district's in the selection colour.
    if app.show_districts {
        let (w, h) = (world.map.w() as u16, world.map.h() as u16);
        let sel = app.selected_district;
        for y in vy0..y1.min(h) {
            for x in vx0..x1.min(w) {
                let t = TilePos { x: x as u8, y: y as u8 };
                let d = world.district_of(t);
                let p = cam.tile_to_screen(vec2(f32::from(x), f32::from(y)));
                let colour = |o: citysim::DistrictId| {
                    if sel == Some(d) || sel == Some(o) {
                        hex(C_SELECTION)
                    } else {
                        Color { a: 0.85, ..WHITE }
                    }
                };
                if x + 1 < w {
                    let e = world.district_of(TilePos { x: (x + 1) as u8, y: y as u8 });
                    if e != d {
                        draw_line(p.x + ppt, p.y, p.x + ppt, p.y + ppt, 2.0, colour(e));
                    }
                }
                if y + 1 < h {
                    let s = world.district_of(TilePos { x: x as u8, y: (y + 1) as u8 });
                    if s != d {
                        draw_line(p.x, p.y + ppt, p.x + ppt, p.y + ppt, 2.0, colour(s));
                    }
                }
            }
        }
        // Each district's name at its centroid.
        for dist in &world.districts {
            if !in_view(dist.centroid) {
                continue;
            }
            let p = cam.tile_to_screen(vec2(f32::from(dist.centroid.x), f32::from(dist.centroid.y)));
            let dims = measure_text(&dist.name, None, 18, 1.0);
            // A dark backdrop so the name reads over a fill or the hatch.
            draw_rectangle(
                p.x - dims.width / 2.0 - 4.0,
                p.y - dims.offset_y - 3.0,
                dims.width + 8.0,
                dims.height + 6.0,
                Color::new(0.0, 0.0, 0.0, 0.6),
            );
            draw_text(&dist.name, p.x - dims.width / 2.0, p.y, 18.0, WHITE);
        }
    }

    // 7. day/night overlay
    let alpha = night_alpha(world.tick);
    if alpha > 0.0 {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color { a: alpha, ..hex(C_NIGHT) });
    }
}

fn vehicle_colour(kind: citysim::AssetKind) -> Color {
    hex(match kind {
        citysim::AssetKind::Motorcycle => C_VEHICLE[0],
        citysim::AssetKind::Car => C_VEHICLE[1],
        citysim::AssetKind::Truck => C_VEHICLE[2],
        _ => C_VEHICLE[3],
    })
}

fn vehicle_glyph(kind: citysim::AssetKind) -> &'static str {
    match kind {
        citysim::AssetKind::Motorcycle => "b",
        citysim::AssetKind::Car => "c",
        citysim::AssetKind::Truck => "t",
        _ => "f",
    }
}

/// Plan 2.8: parked vehicles, drivers and flyers.
fn draw_vehicles(world: &World, app: &App, in_view: &dyn Fn(TilePos) -> bool) {
    use citysim::{Asset, AssetLoc, ExecState};
    let cam = &app.camera;
    let ppt = cam.px_per_tile;
    // Parked: up to four per door, in a 2 x 2 grid over the door tile.
    let mut per_door: std::collections::BTreeMap<(u8, u8), u8> = std::collections::BTreeMap::new();
    for &v in &world.vehicles {
        let Some(x) = world.comp::<Asset>(v) else { continue };
        let AssetLoc::Parked(b) = x.loc else { continue };
        let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else { continue };
        if !in_view(door) {
            continue;
        }
        let n = per_door.entry((door.x, door.y)).or_insert(0);
        if *n >= 4 {
            continue;
        }
        let p = cam.tile_to_screen(vec2(f32::from(door.x), f32::from(door.y)));
        let w = (ppt / 2.0).max(3.0);
        let (sx, sy) = (p.x + f32::from(*n % 2) * w, p.y + f32::from(*n / 2) * w);
        draw_rectangle(sx, sy, w - 1.0, w - 1.0, vehicle_colour(x.kind));
        draw_rectangle_lines(sx, sy, w - 1.0, w - 1.0, 1.0, BLACK);
        *n += 1;
    }
    // M13 D41 (plan 4.5): a posted robot is a filled square on its door,
    // steel blue while powered, grey when its upkeep is unpaid.
    for a in world.assets_by_owner.values().flatten() {
        let Some(x) = world.comp::<Asset>(*a).filter(|x| x.kind == citysim::AssetKind::Robot) else { continue };
        let AssetLoc::Posted(b) = x.loc else { continue };
        let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else { continue };
        if !in_view(door) {
            continue;
        }
        let p = cam.tile_to_screen(vec2(f32::from(door.x), f32::from(door.y)));
        let s = (ppt * 0.6).max(3.0);
        let powered = x.condition > 0 && x.upkeep_arrears == 0 && !x.bricked;
        let colour = if powered { Color::from_rgba(0x5A, 0x8C, 0xC8, 255) } else { GRAY };
        let off = (ppt - s) / 2.0;
        draw_rectangle(p.x + off, p.y + off, s, s, colour);
        draw_rectangle_lines(p.x + off, p.y + off, s, s, 1.0, BLACK);
    }
    // Driving and flying, whatever the tier (the trips map is small).
    for (&agent, trip) in &world.trips {
        let Some(kind) = world.comp::<Asset>(trip.vehicle).map(|x| x.kind) else { continue };
        let Some(brain) = world.comp::<Brain>(agent) else { continue };
        let at = match &brain.exec {
            ExecState::Fly { target, from, depart, arrive_tick } => {
                let span = arrive_tick.saturating_sub(*depart).max(1) as f32;
                let t = (world.tick.saturating_sub(*depart) as f32 / span).clamp(0.0, 1.0);
                let (fx, fy) = (f32::from(from.x), f32::from(from.y));
                let (tx, ty) = (f32::from(target.tile.x), f32::from(target.tile.y));
                Some(vec2(fx + (tx - fx) * t, fy + (ty - fy) * t))
            }
            _ if brain.lod == Lod::Full => world
                .comp::<Position>(agent)
                .filter(|p| p.building.is_none())
                .map(|p| vec2(f32::from(p.tile.x), f32::from(p.tile.y))),
            _ => None,
        };
        let Some(at) = at else { continue };
        if !in_view(TilePos { x: at.x as u8, y: at.y as u8 }) {
            continue;
        }
        let p = cam.tile_to_screen(at);
        if kind == citysim::AssetKind::Flyer {
            draw_circle(p.x + ppt / 2.0, p.y + ppt / 2.0, (ppt * 0.45).max(3.0), vehicle_colour(kind));
        } else {
            draw_rectangle_lines(p.x, p.y, ppt, ppt, 2.0, vehicle_colour(kind));
        }
        if ppt >= LABEL_MIN_PX / 2.0 {
            let size = (ppt * 0.9).clamp(10.0, 28.0);
            draw_text(vehicle_glyph(kind), p.x + ppt * 0.25, p.y + ppt * 0.8, size, BLACK);
        }
    }
}

/// 0.35 while dark (21:00–06:00), ramping linearly over 60 ticks each side.
fn night_alpha(tick: u64) -> f32 {
    let t = f32::from(time::tick_of_day(tick));
    let dusk = 1260.0; // 21:00
    let dawn = 360.0; // 06:00
    let k = if t >= dusk || t < dawn {
        1.0
    } else if t < dawn + NIGHT_RAMP_TICKS {
        1.0 - (t - dawn) / NIGHT_RAMP_TICKS
    } else if t >= dusk - NIGHT_RAMP_TICKS {
        (t - (dusk - NIGHT_RAMP_TICKS)) / NIGHT_RAMP_TICKS
    } else {
        0.0
    };
    NIGHT_ALPHA * k
}

/// Agent square colour by current action (spec › Draw order and colours).
fn agent_colour(world: &World, id: citysim::EntityId) -> Color {
    use citysim::{ActionKind as A, ExecState as E};
    if world.has::<citysim::Sentence>(id) {
        return Color { a: 0.6, ..hex(C_AGENT_JAILED) };
    }
    let Some(brain) = world.comp::<Brain>(id) else { return hex(C_AGENT_IDLE) };
    let kind = match &brain.exec {
        E::Use { kind, .. } => *kind,
        E::Goto { .. } | E::GotoTimed { .. } | E::Fly { .. } => match brain.current_step().map(|s| s.action) {
            // en route: colour by the step after the walk, so a guard heading to an arrest reads as guarding
            Some(_) => brain
                .plan
                .as_ref()
                .and_then(|p| p.steps.get(usize::from(brain.plan_step) + 1))
                .map_or(A::Wander, |s| s.action),
            None => A::Wander,
        },
        E::Idle | E::Wait { .. } => A::Wander,
    };
    hex(match kind {
        A::FarmWork
        | A::ClerkWork
        | A::BartendWork
        | A::TendGraves
        | A::HaulToMarket
        | A::CollectWage
        | A::CollectDole
        | A::Register => C_AGENT_WORKING,
        A::EatFromInventory | A::EatAtHome | A::BuyFood | A::Forage | A::StoreFood => C_AGENT_EATING,
        A::Sleep | A::Rest => C_AGENT_SLEEPING,
        A::StealFood(_) | A::Extort | A::Attack | A::Fence | A::SplitLoot => C_AGENT_CRIME,
        A::FleeToHome | A::HideFromLaw => C_AGENT_FLEEING,
        A::GuardJail | A::PatrolLeg | A::Arrest | A::Escort => C_AGENT_GUARDING,
        A::Chat | A::Drink | A::Flirt | A::Propose | A::JoinGang => C_AGENT_SOCIAL,
        _ => C_AGENT_IDLE,
    })
}

fn dashed_rect(x: f32, y: f32, w: f32, h: f32, colour: Color) {
    let dash = 6.0;
    let mut t = 0.0;
    while t < w {
        let e = (t + dash).min(w);
        draw_line(x + t, y, x + e, y, 2.0, colour);
        draw_line(x + t, y + h, x + e, y + h, 2.0, colour);
        t += dash * 2.0;
    }
    t = 0.0;
    while t < h {
        let e = (t + dash).min(h);
        draw_line(x, y + t, x, y + e, 2.0, colour);
        draw_line(x + w, y + t, x + w, y + e, 2.0, colour);
        t += dash * 2.0;
    }
}
