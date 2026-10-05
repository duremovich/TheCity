//! Draws the world. Takes `&World` only and never mutates it.

#![deny(clippy::needless_pass_by_ref_mut)]

use macroquad::prelude::*;

use citysim::{time, Brain, Building, Corpse, Gang, Job, Lod, Position, Role, TileKind, TilePos, World};

use crate::App;

const C_GROUND: u32 = 0x2e2a24;
const C_ROAD: u32 = 0x4a4440;
const C_WALL: u32 = 0x6b6660;
const C_DOOR: u32 = 0x9a7b4f;
const C_FARMLAND: u32 = 0x3f5a2a;
const C_WATER: u32 = 0x27485e;
const C_OUTLINE: u32 = 0xc8c0b0;
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
const SIGHT_TILES: f32 = 6.0;

fn hex(c: u32) -> Color {
    Color::from_hex(c)
}

/// A gang's map colour, from the shared table in `ui`.
fn gang_colour(index: usize) -> Color {
    hex(crate::ui::gang_hex(index))
}

fn tile_colour(kind: TileKind) -> Color {
    hex(match kind {
        TileKind::Ground => C_GROUND,
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
    let view = cam.view_rect();

    // 1. tiles, culled to the view
    for y in view.y..view.y + view.h {
        for x in view.x..view.x + view.w {
            let p = cam.tile_to_screen(vec2(f32::from(x), f32::from(y)));
            draw_rectangle(p.x, p.y, ppt + 0.5, ppt + 0.5, tile_colour(world.map.tile(usize::from(x), usize::from(y))));
        }
    }

    // One pass over citizens: badge counts for non-Full agents inside
    // buildings, and the list of Full agents to draw as squares.
    let mut inside = vec![0u16; world.building.len()];
    let mut full: Vec<(citysim::EntityId, TilePos)> = Vec::new();
    for id in world.citizens() {
        let (Some(pos), Some(brain)) = (world.comp::<Position>(id), world.comp::<Brain>(id)) else { continue };
        if brain.lod == Lod::Full {
            if view.contains(pos.tile) {
                full.push((id, pos.tile));
            }
        } else if let Some(n) = pos.building.and_then(|b| inside.get_mut(b.index as usize)) {
            *n += 1;
        }
    }

    // 2 + 3. buildings: outline, letter, badge
    for id in world.with::<Building>() {
        let Some(b) = world.comp::<Building>(id) else { continue };
        if !b.rect.overlaps(&view) {
            continue;
        }
        let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
        let (w, h) = (f32::from(b.rect.w) * ppt, f32::from(b.rect.h) * ppt);
        if b.demolished {
            dashed_rect(tl.x, tl.y, w, h, hex(C_OUTLINE));
        } else {
            draw_rectangle_lines(tl.x, tl.y, w, h, 2.0, hex(C_OUTLINE));
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
            if !b.rect.overlaps(&view) {
                continue;
            }
            let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
            let (w, h) = (f32::from(b.rect.w) * ppt, f32::from(b.rect.h) * ppt);
            draw_rectangle_lines(tl.x + 3.0, tl.y + 3.0, w - 6.0, h - 6.0, 1.5, colour);
        }
        if g.is_sacked(world.tick) {
            if let Some(b) = world.comp::<Building>(g.hideout).filter(|b| b.rect.overlaps(&view)) {
                let tl = cam.tile_to_screen(vec2(f32::from(b.rect.x), f32::from(b.rect.y)));
                dashed_rect(tl.x, tl.y, f32::from(b.rect.w) * ppt, f32::from(b.rect.h) * ppt, colour);
            }
        }
    }

    // 4. corpses
    for id in world.with::<Corpse>() {
        let Some(pos) = world.comp::<Position>(id) else { continue };
        if !view.contains(pos.tile) {
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

    // 6. selection: a Full agent's square and sight radius, else the building
    // the selection is (or is inside)
    if let Some(sel) = app.selected {
        if let Some(b) = world.comp::<Building>(sel) {
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

    // 7. day/night overlay
    let alpha = night_alpha(world.tick);
    if alpha > 0.0 {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color { a: alpha, ..hex(C_NIGHT) });
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
        E::Goto { .. } | E::GotoTimed { .. } => match brain.current_step().map(|s| s.action) {
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
        | A::CollectDole => C_AGENT_WORKING,
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
