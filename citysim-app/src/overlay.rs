//! M14 § 10 and the addendum's "The overlay": the Virt plane drawn over a
//! ghosted real city (`N`). A second board of abstract nodes and links,
//! positioned at the buildings they belong to, so a runner's route reads
//! against the streets it crosses. Takes `&World` only; it never mutates it.
//!
//! Nodes are coloured by effective ICE (0 grey, 1 teal, 2 amber, 3 red) and
//! outlined in their owner's colour; a Ledger is a square beside its
//! building (it shares the door); a Data glyph is sized by the store; links
//! are lines (Trunks 2 px, a firewalled link red); a live run is a dot
//! moving along its route, and a run that lost a contest leaves a red dot
//! at the node for a while.

#![deny(clippy::needless_pass_by_ref_mut)]

use macroquad::prelude::*;

use citysim::systems::virt;
use citysim::virt::{LinkKind, Node, NodeId, NodeKind, OwnerTag, Purpose, Run};
use citysim::World;

use crate::camera::Camera;
use crate::App;

/// Effective ICE 0..=3: grey, teal, amber, red.
pub const C_ICE: [u32; 4] = [0x8a949c, 0x1abc9c, 0xf1c40f, 0xe74c3c];
const C_DATA: u32 = 0x7fe0ff;
const C_RUN: u32 = 0xffffff;
const C_RUN_LOST: u32 = 0xff2020;
const C_ROUTE: u32 = 0xb48cff;
const C_SELECTION: u32 = 0xffd700;
/// The real city's dimming under the overlay.
const GHOST_ALPHA: f32 = 0.68;
const C_GHOST: u32 = 0x05070d;
/// A lost contest's red dot lingers this many ticks (an hour).
const LOST_DOT_TICKS: u64 = 60;
/// A Ledger sits this many pixels up and right of its door, joined by a stub.
const LEDGER_OFF: Vec2 = Vec2::new(11.0, -11.0);

fn hex(c: u32) -> Color {
    Color::from_hex(c)
}

/// A node's radius in pixels at the current zoom.
fn radius(node: &Node, ppt: f32) -> f32 {
    match node.kind {
        NodeKind::Public(_) => (ppt * 0.9).clamp(6.0, 14.0),
        NodeKind::Ledger(_) => (ppt * 0.5).clamp(4.0, 9.0),
        NodeKind::Building(_) => (ppt * 0.45).clamp(4.0, 9.0),
    }
}

/// Where a node is drawn: its door, a Ledger nudged off it.
pub fn screen_pos(cam: &Camera, node: &Node) -> Vec2 {
    let p = cam.tile_to_screen(vec2(f32::from(node.pos.x) + 0.5, f32::from(node.pos.y) + 0.5));
    match node.kind {
        NodeKind::Ledger(_) => p + LEDGER_OFF,
        _ => p,
    }
}

fn owner_colour(world: &World, node: &Node) -> Color {
    match (node.owner_kind, node.owner) {
        (OwnerTag::Corp, Some(c)) => hex(crate::ui::corp_hex(world.corp_index(c))),
        (OwnerTag::Gang, Some(g)) => hex(crate::ui::gang_hex(world.gang_index(g))),
        _ => hex(0xc8c0b0),
    }
}

/// The dot of a live run, in screen pixels: along its route by `next_at`
/// while hopping, else on the target (an Overwatch stream on its portal).
pub fn run_pos(world: &World, cam: &Camera, r: &Run) -> Option<Vec2> {
    let at = |n: NodeId| world.virt.node(n).map(|x| screen_pos(cam, x));
    if r.phase == citysim::virt::RunPhase::Hop {
        let a = at(*r.route.get(usize::from(r.at))?)?;
        let Some(b) = r.route.get(usize::from(r.at) + 1).and_then(|&n| at(n)) else { return Some(a) };
        let h = &world.config.virt.hop_ticks;
        let span = h.get(usize::from(r.deck_eff.saturating_sub(1))).or(h.last()).copied().unwrap_or(8).max(1) as f32;
        let left = r.next_at.saturating_sub(world.tick) as f32;
        let t = (1.0 - left / span).clamp(0.0, 1.0);
        return Some(a + (b - a) * t);
    }
    match r.purpose {
        Purpose::Overwatch(_) => at(r.portal),
        _ => at(r.target),
    }
}

/// The alive node nearest the mouse within a few pixels of its glyph.
pub fn node_at(world: &World, cam: &Camera, mouse: Vec2) -> Option<NodeId> {
    let ppt = cam.px_per_tile;
    world
        .virt
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.alive)
        .map(|(i, n)| (screen_pos(cam, n).distance(mouse) - radius(n, ppt), i))
        .filter(|&(d, _)| d <= 3.0)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, i)| NodeId(i as u16))
}

/// The live run whose dot is under the mouse.
pub fn run_at(world: &World, cam: &Camera, mouse: Vec2) -> Option<citysim::virt::RunId> {
    world
        .runs
        .values()
        .filter_map(|r| run_pos(world, cam, r).map(|p| (p.distance(mouse), r.id)))
        .filter(|&(d, _)| d <= 8.0)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

pub fn draw(world: &World, app: &App) {
    // The real city, ghosted.
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color { a: GHOST_ALPHA, ..hex(C_GHOST) });
    let cam = &app.camera;
    let ppt = cam.px_per_tile;
    let nodes = &world.virt.nodes;
    let pos = |n: NodeId| nodes.get(n.index()).filter(|x| x.alive).map(|x| screen_pos(cam, x));

    // Links, Trunks thicker; a firewalled link red. (No link kind is a
    // bridge until M16 plants them; they will be drawn dashed here.)
    for l in &world.virt.links {
        let (Some(a), Some(b)) = (pos(l.a), pos(l.b)) else { continue };
        let (w, base) = match l.kind {
            LinkKind::Street => (1.0, Color { a: 0.40, ..hex(0x5b7f9e) }),
            LinkKind::Access => (1.0, Color { a: 0.50, ..hex(0x7f9fb4) }),
            LinkKind::Trunk => (2.0, Color { a: 0.85, ..hex(0xa8d0ff) }),
        };
        let colour = if l.firewall > 0 { Color { a: 0.9, ..hex(C_ICE[3]) } } else { base };
        draw_line(a.x, a.y, b.x, b.y, w, colour);
        if l.firewall > 0 {
            let m = (a + b) * 0.5;
            draw_rectangle(m.x - 2.5, m.y - 2.5, 5.0, 5.0, hex(C_ICE[3]));
        }
    }

    // The selected run's route.
    if let Some(r) =
        app.selected_run.and_then(|id| world.runs.get(&id).or_else(|| world.run_log.iter().find(|r| r.id == id)))
    {
        for w in r.route.windows(2) {
            if let (Some(a), Some(b)) = (pos(w[0]), pos(w[1])) {
                draw_line(a.x, a.y, b.x, b.y, 3.0, Color { a: 0.9, ..hex(C_ROUTE) });
            }
        }
    }

    // Nodes: Public rings, then Ledgers, then buildings on top.
    let mut order: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].alive).collect();
    order.sort_by_key(|&i| match nodes[i].kind {
        NodeKind::Public(_) => 0,
        NodeKind::Ledger(_) => 1,
        NodeKind::Building(_) => 2,
    });
    for i in order {
        let n = &nodes[i];
        let id = NodeId(i as u16);
        let p = screen_pos(cam, n);
        let door = cam.tile_to_screen(vec2(f32::from(n.pos.x) + 0.5, f32::from(n.pos.y) + 0.5));
        let r = radius(n, ppt);
        let ice = usize::from(virt::ice_eff(world, id).min(3));
        let fill = hex(C_ICE[ice]);
        let ring = owner_colour(world, n);
        match n.kind {
            NodeKind::Public(_) => {
                draw_circle(p.x, p.y, r, Color { a: 0.35, ..fill });
                draw_circle_lines(p.x, p.y, r, 2.0, fill);
            }
            NodeKind::Ledger(_) => {
                draw_line(door.x, door.y, p.x, p.y, 1.0, Color { a: 0.6, ..ring });
                draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 2.0, fill);
                draw_rectangle_lines(p.x - r, p.y - r, r * 2.0, r * 2.0, 2.0, ring);
            }
            NodeKind::Building(_) => {
                draw_circle(p.x, p.y, r, fill);
                draw_circle_lines(p.x, p.y, r + 1.0, 1.5, ring);
            }
        }
        if n.hacked.is_some_and(|(_, until)| until > world.tick) {
            // A live hack effect: a violet halo.
            draw_circle_lines(p.x, p.y, r + 4.0, 2.0, hex(C_ROUTE));
        }
        if n.alarm_until.is_some_and(|t| t > world.tick) {
            draw_circle_lines(p.x, p.y, r + 7.0, 1.5, hex(C_RUN_LOST));
        }
        let total = n.store.total();
        if total > 0 {
            let cap = world.config.data.store_cap.max(1) as f32;
            let s = 3.0 + 9.0 * (total as f32 / cap).sqrt().min(1.0);
            let (gx, gy) = (p.x + r + 1.0, p.y - r - s);
            draw_rectangle(gx, gy, s, s, hex(C_DATA));
            draw_rectangle_lines(gx, gy, s, s, 1.0, BLACK);
        }
        if ppt >= 10.0 {
            let t = ice.to_string();
            let d = measure_text(&t, None, 14, 1.0);
            draw_text(&t, p.x - d.width / 2.0, p.y + d.height / 2.0, 14.0, BLACK);
        }
        if app.selected_node == Some(id) {
            draw_circle_lines(p.x, p.y, r + 5.0, 3.0, hex(C_SELECTION));
        }
    }

    // Runs that lost a contest recently: a red dot where it happened.
    for r in world.run_log.iter().rev() {
        let Some(&(t, n, false)) = r.log.last() else { continue };
        if world.tick.saturating_sub(t) > LOST_DOT_TICKS {
            continue;
        }
        let Some(p) = pos(n) else { continue };
        let fade = 1.0 - world.tick.saturating_sub(t) as f32 / LOST_DOT_TICKS as f32;
        draw_circle(p.x, p.y, 7.0, Color { a: 0.35 + 0.65 * fade, ..hex(C_RUN_LOST) });
        draw_circle_lines(p.x, p.y, 9.0, 2.0, hex(C_RUN_LOST));
    }

    // Live runs: a dot along the route; a stream rings its portal.
    for r in world.runs.values() {
        let Some(p) = run_pos(world, cam, r) else { continue };
        if matches!(r.purpose, Purpose::Overwatch(_)) {
            draw_circle_lines(p.x, p.y, 10.0, 2.0, hex(C_ROUTE));
            continue;
        }
        draw_circle(p.x, p.y, 5.0, hex(C_RUN));
        draw_circle_lines(p.x, p.y, 5.5, 2.0, if app.selected_run == Some(r.id) { hex(C_SELECTION) } else { BLACK });
    }

    // Legend, clear of the side panels.
    let (lx, ly) = (crate::ui::city::CITY_W + 10.0, crate::ui::HUD_H + 28.0);
    draw_rectangle(lx, ly, 330.0, 38.0, Color::new(0.0, 0.0, 0.0, 0.7));
    draw_text("VIRT  ICE", lx + 6.0, ly + 14.0, 14.0, WHITE);
    for (i, c) in C_ICE.iter().enumerate() {
        let x = lx + 78.0 + i as f32 * 34.0;
        draw_circle(x, ly + 10.0, 5.0, hex(*c));
        draw_text(i.to_string(), x + 8.0, ly + 14.0, 14.0, WHITE);
    }
    draw_text(
        "square ledger · box Data · dot run · red lost",
        lx + 6.0,
        ly + 31.0,
        13.0,
        Color::new(0.85, 0.85, 0.9, 1.0),
    );
}
