//! Sim frames on the city (spec/integration/SIM_VIEW_CONTRACT.md, the `city-simview/0.1` file form): `--sim FILE`
//! reads the converter's frames (engine/simsnap) and draws placeholders on the real city: a capsule per agent out
//! on the street, a box per vehicle in motion and per El train, moved between frames at `--sim-rate` ticks a second
//! (default 0.7: one tick ~1.4 s, the sim's 2-tick, 4 m step at a 1.4 m/s walk; 0 holds `--sim-frame`).
//!
//! Colours (CV_SIM_COLOUR=lod switches): by action, walk green, use amber, idle slate, wait violet, ride / aboard
//! cyan, timed (a Coarse jump) magenta, fly white; by LOD, Full green, Coarse amber, Statistical red. An agent inside
//! a building is drawn on its storey (the city milestone C6a: the frame's `floor`, a 4 m ground and 3 m storeys over
//! the ground at its building's tile), seen where the occlusion cuts the building away; the HUD counts them. V3: it is
//! drawn only there (the subject's storey of its building, inside the open cut), else hidden; the bodies go to
//! people.rs as characters (`Sim::people`), the capsules stay behind `--capsules`.
//! Positions are tile centres plus a per-id jitter inside the 4 m tile, on the streamed ground; a train sits on the
//! rail deck (layers.json), the tram and the lines with no track here on the street (OPEN-158: drawn red, raised).
//!
//! C2, the follow-cam (`--save FILE --follow INDEX`, simthread.rs): the frames come live from the sim thread's
//! two-slot buffer instead of a file (`Sim::live`), lerped from the older to the newer over one tick at the rate; a
//! body or vehicle with an `ahead` (the way to the next tick's place) is lerped along it, not through the block; the
//! subject's place drives the player stand-in, so the camera and the occlusion follow it (`follow_subject`).
//!
//! The view milestone V4 (riders and climbs, `[view] el_*`, `lift_*`, `stair_radius_m`, `crowd_*`): an El train on a
//! rail deck is `el_cars` cars laid along the deck's centre line on its left track (`car_meshes`), its riders standing
//! along the walls (a slot per rider by its id), the subject's train roofless; a body indoors stands on the export's
//! lot at its storey's z (lots.rs: no more ground-plus-formula), a lift rider in a glass car in a shaft at the lot's
//! middle, a stairs climber walking round the core a turn a storey; the HUD says what the subject is doing (climb,
//! ride, storey, z, lot); a crowd standing on one street tile spreads over the walkable ground round it (`Sim::spread`).
//! Motion V2 (films v2_*): a body through a door inside a pair is one walk, along the street to the building's edge and
//! on inside to its indoor spot, or out the same way (`Sim::door_path`, `OPEN-308`), shown inside where the cut opens;
//! walkers between the same two tiles take lanes across their way (`[view] lane_gap_m`); the subject's street fight is
//! staged round it (`fight_gap_m`, `fight_circle_deg_s`, `OPEN-314`); the click way's drawn walk is held within a tile of
//! the sim's place and never behind the frames (`way_lead_tiles`, `OPEN-315`); a body's act picks its posture (asleep
//! lies, a sitting Use sits: `pose_of`).
//! `--watch-vehicle INDEX` (0: the fullest train) puts the camera on a vehicle while the subject drives the ring.

use crate::data::Layers;
use crate::mesh::bv;
use crate::stream::Stream;
use bevy::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone)]
pub struct Agent {
    pub id: u64,
    pub t: [f32; 2],
    /// The way to the next frame's place (C2; empty: a straight lerp)
    pub ahead: Vec<[f32; 2]>,
    pub indoors: bool,
    /// C6a: the storey it stands on indoors (0 the ground); C7: on the street its walkway deck's level, on a climb
    /// the storey reached (lerped between frames)
    pub floor: f32,
    pub lod: u8,
    pub act: u8,
    /// The vehicle it is aboard (its key)
    pub aboard: Option<u64>,
    pub facing: Option<f32>,
    /// V3: the building it is inside (its key), and the look's keys (people.rs): sex, age (years), dress, a gang's
    pub b: Option<u64>,
    pub female: bool,
    pub age: u32,
    pub dress: u8,
    pub gang: bool,
    /// V3: indoors, its building's rect (tiles `[x, y, w, h]`): drawn inside it, not at the door tile it stands on
    pub rect: Option<[f32; 4]>,
    /// V4: on a climb, (from, to, by the lift)
    pub climb: Option<(u8, u8, bool)>,
    /// Motion V2: the posture its act asks for (`POSE_STAND`, `POSE_SIT`: a sitting Use or jacked in, `POSE_LIE`:
    /// asleep)
    pub pose: u8,
}

/// Motion V2: an agent's posture (`Agent::pose`).
pub const POSE_STAND: u8 = 0;
pub const POSE_SIT: u8 = 1;
pub const POSE_LIE: u8 = 2;

/// Motion V2: the posture of a Use (the sim's `ActionKind`): asleep lies, these sit (on the floor: there are no chairs
/// in the view), the rest stand.
fn pose_of(act: &citysim::view::ViewAct) -> u8 {
    use citysim::goap::actions::ActionKind as K;
    match act {
        citysim::view::ViewAct::Use { kind: K::Sleep, .. } => POSE_LIE,
        citysim::view::ViewAct::Use { kind: K::Rest | K::Drink | K::EatAtHome | K::EatFromInventory | K::Beg | K::ServeTime, .. }
        | citysim::view::ViewAct::JackedIn { .. } => POSE_SIT,
        _ => POSE_STAND,
    }
}

#[derive(Clone)]
pub struct Vehicle {
    pub id: u64,
    pub t: [f32; 2],
    pub ahead: Vec<[f32; 2]>,
    /// 0 car / cab / exec, 1 van, 2 train, 3 flyer, 4 foot escort
    pub ride: u8,
    pub line: Option<u8>,
    pub riders: u16,
    /// V4: the riders' keys (drawn inside the car)
    pub rider_ids: Vec<u64>,
}

/// C6b: a business on its host's floors, its sign on the facade (live frames only).
#[derive(Clone)]
pub struct Unit {
    pub id: u64,
    /// The host's rect, tiles `[x, y, w, h]`, and the door the sign hangs by
    pub rect: [f32; 4],
    pub door: [f32; 2],
    pub floor: u8,
    /// The sign's colour (a hash of the kind)
    pub hue: u8,
    pub lit: bool,
    pub fresh: bool,
}

pub struct SimFrame {
    pub tick: u64,
    pub clock: String,
    pub agents: Vec<Agent>,
    pub vehicles: Vec<Vehicle>,
    pub events: usize,
    pub units: Vec<Unit>,
    /// C6b: the ring's centre (tiles): the follow-cam's place for a building subject (no body in the frame)
    pub centre: Option<[f32; 2]>,
}

#[derive(Resource)]
pub struct Sim {
    pub tile_m: f32,
    pub x0: f32,
    pub y1: f32,
    pub frames: Vec<SimFrame>,
    /// Line index -> on the rail deck (the Terrace Line), on the street (the tram), or no track here.
    pub line_track: Vec<u8>,
    pub rail: Vec<[f32; 3]>,
    /// C7: the walkway decks' points (footbridges, the escalator, the stations): a body on a deck stands at their z
    pub deck: Vec<[f32; 3]>,
    pub t: f32,
    pub rate: f32,
    pub by_lod: bool,
    pub ents: HashMap<(u8, u64), Entity>,
    pub mats: Vec<Handle<StandardMaterial>>,
    pub capsule: Handle<Mesh>,
    pub boxes: [Handle<Mesh>; 5],
    /// C6b: a unit's sign
    pub sign: Handle<Mesh>,
    pub hud: String,
    /// C2: the sim thread's buffer (`--save`), the last frame pair's sequence number, the subject's key and caption
    pub live: Option<std::sync::Arc<crate::simthread::Shared>>,
    pub live_seq: u64,
    pub subject: Option<u64>,
    pub caption: String,
    /// The subject's place this frame (feet, metres) and heading (rad, the player's convention) for the follow-cam
    pub subject_at: Option<(Vec3, f32)>,
    /// V3 (people.rs): the bodies to draw this frame (the subject apart: the player stand-in wears it), the last
    /// heading drawn per body (a standing body keeps it), `--capsules` (the old placeholders)
    pub people: Vec<crate::people::BodyDraw>,
    pub subject_body: Option<crate::people::BodyDraw>,
    pub yaw_of: HashMap<u64, f32>,
    pub capsules: bool,
    /// V4: the rail decks' centre lines (per element: its points, the cumulative plan length) a train's cars are laid
    /// along; the export's storeys per lot (lots.rs); `[view]` (TheCity's config at startup)
    pub rails: Vec<(Vec<[f32; 3]>, Vec<f32>)>,
    pub lots: crate::lots::Lots,
    pub cfg: citysim::config::ViewCfg,
    /// V4: the way each train last ran along its rail (+1 / -1); the lines' names; `--watch-vehicle` (the camera on a
    /// vehicle, not the subject: a proof of the El's riders when the subject never rides)
    pub dir_of: HashMap<u64, f32>,
    pub line_names: Vec<String>,
    pub watch_vehicle: Option<u32>,
    /// V4: the El car's body, lit floor and roof; the lift's plate, post and glass
    pub car: [Handle<Mesh>; 3],
    pub car_mats: [Handle<StandardMaterial>; 3],
    pub cab: [Handle<Mesh>; 3],
    pub cab_mats: [Handle<StandardMaterial>; 2],
    /// V4b: the subject indoors, its building's rect (world metres x0, y0, x1, y1): the cutaway cuts it at the
    /// subject's ceiling (occ.rs, glclient::occlusion)
    pub subject_indoors: Option<Vec4>,
    /// V10: the newest frame's tick (the light follows its hour: `clock_light`)
    pub clock_tick: Option<u64>,
    /// T2 (docs/TIME_MODEL.md § 5): the possessed body's click way (`simthread::WayPub`): its seq, the tiles, the
    /// tiles walked along it so far, the walk's tiles a tick; the subject walks it from the click at the clock's pace
    /// (`Shared::tick_rate`: stopped while the clock is), ahead of the frames by at most a tick, until the frames reach
    /// its end or leave it
    pub way: Option<(u64, Vec<[f32; 2]>, f32, f32)>,
    pub way_seen: u64,
    /// T2 (§ 4): this frame's bodies whose way ahead did not reach their newer place (drawn as a jump, not a slide
    /// through the block), for the HUD
    pub no_way: usize,
    /// The film (film.rs): the lerp this frame drew, the older frame's tick, the newer's and `u` between them
    pub lerp: Option<(u64, u64, f32)>,
    /// Motion V1: the newest frame came flat out (a hold, a script's line, `--from`, a load): drawn as jumps, the
    /// camera on the subject at once
    pub flat: bool,
    /// Motion V1 (motion.rs): the drawn bodies' tracks; a crowd's slots per (tile, count) on walkable ground; the
    /// click way as the drawn body walks it (`motion::rejoin`, by the way's seq)
    pub motion: crate::motion::Motion,
    pub slot_cache: HashMap<((i32, i32), usize), Vec<Vec2>>,
    pub way_path: Option<(u64, Vec<[f32; 2]>)>,
    /// Motion V2 (`OPEN-314`): each fighter's last mark (the body it struck or was struck by), faced between beats
    pub fight_mark: HashMap<u64, u64>,
    /// Motion V2: the view tick the subject's street fight was first drawn at (its circling's start)
    pub fight_t0: Option<f64>,
    /// Motion V2: the pair's strides (`simthread::Published::strides`), older and newer
    pub strides: [HashMap<u64, f32>; 2],
    /// R4: the pair's walkway hops under way (`simthread::Published::hops`), older and newer
    pub hops: [HashMap<u64, citysim::exec::HopSpan>; 2],
    /// Motion V2 (`OPEN-309`): where each El rider was drawn last (its seat), for the walk off the car
    pub seat_at: HashMap<u64, Vec3>,
    /// Motion V2: S2's `[combat] stir_tiles` (`simthread::Static`): a body standing this near the subject's fight
    /// watches it (faces it)
    pub stir_tiles: u16,
}

/// A 64-bit mix (splitmix64's finaliser): the per-id picks.
fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Two numbers in -0.5..0.5 fixed per id (V3's indoor place).
fn unit2(id: u64) -> (f32, f32) {
    let k = id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (((k >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5, ((k >> 40) & 0xFFFF) as f32 / 65535.0 - 0.5)
}

fn ekey(e: citysim::EntityId) -> u64 {
    (u64::from(e.generation) << 32) | u64::from(e.index)
}

fn tf(t: citysim::TilePos) -> [f32; 2] {
    [f32::from(t.x), f32::from(t.y)]
}

impl SimFrame {
    /// A live `ViewFrame` (C2) as the placeholders read it.
    pub fn from_view(f: &citysim::view::ViewFrame) -> SimFrame {
        use citysim::systems::motion::RideKind;
        use citysim::view::ViewAct;
        let agents = f
            .agents
            .iter()
            .map(|a| Agent {
                id: ekey(a.id),
                t: tf(a.t),
                ahead: a.ahead.iter().map(|t| tf(*t)).collect(),
                indoors: a.b.is_some(),
                floor: f32::from(a.floor),
                lod: match a.lod {
                    Some(citysim::Lod::Full) => 0,
                    Some(citysim::Lod::Coarse) => 1,
                    _ => 2,
                },
                act: match a.act {
                    ViewAct::Walk { .. } => 0,
                    ViewAct::Use { .. } => 1,
                    ViewAct::Idle => 2,
                    ViewAct::Wait { .. } => 3,
                    ViewAct::Ride { .. } => 4,
                    ViewAct::Timed { .. } => 5,
                    ViewAct::Fly { .. } => 6,
                    ViewAct::JackedIn { .. } => 7,
                    ViewAct::Child => 8,
                    ViewAct::Climb { .. } => 9,
                },
                aboard: a.aboard.map(ekey),
                facing: a.facing,
                b: a.b.map(ekey),
                female: a.sex == citysim::Sex::Female,
                age: a.age,
                dress: a.dress.unwrap_or(1),
                gang: a.colours.is_some(),
                rect: a.rect.map(|r| [f32::from(r.x), f32::from(r.y), f32::from(r.w), f32::from(r.h)]),
                climb: match a.act {
                    ViewAct::Climb { from, to, lift, .. } => Some((from, to, lift)),
                    _ => None,
                },
                pose: pose_of(&a.act),
            })
            .collect();
        let vehicles = f
            .vehicles
            .iter()
            .map(|v| Vehicle {
                id: ekey(v.id),
                t: tf(v.t),
                ahead: v.ahead.iter().map(|t| tf(*t)).collect(),
                ride: match v.ride {
                    RideKind::Van => 1,
                    RideKind::Train => 2,
                    RideKind::Flyer => 3,
                    RideKind::Foot => 4,
                    _ => 0,
                },
                line: match v.route {
                    citysim::view::ViewRoute::Line { line, .. } => Some(line),
                    _ => None,
                },
                riders: v.riders.len() as u16,
                rider_ids: v.riders.iter().map(|r| ekey(*r)).collect(),
            })
            .collect();
        let units = f
            .buildings
            .iter()
            .filter_map(|b| {
                let u = b.unit.as_ref()?;
                Some(Unit {
                    id: ekey(b.id),
                    rect: [f32::from(u.rect.x), f32::from(u.rect.y), f32::from(u.rect.w), f32::from(u.rect.h)],
                    door: tf(u.door),
                    floor: u.floor,
                    hue: (u.kind as u8).wrapping_mul(5) % SIGN_RGB.len() as u8,
                    lit: u.lit,
                    fresh: u.fresh,
                })
            })
            .collect();
        let centre = Some([f32::from(f.ring.x) + f32::from(f.ring.w) / 2.0, f32::from(f.ring.y) + f32::from(f.ring.h) / 2.0]);
        SimFrame { tick: f.tick, clock: f.clock.clone(), agents, vehicles, events: f.events.len(), units, centre }
    }
}

/// The place `u` (0..1) of the way from `from` to `to`: along `ahead` when it reaches `to` (the street's corners; T2:
/// the whole way when it ends there, a stitched frame's path over many ticks), else, a step apart, straight; else
/// (T2, docs/TIME_MODEL.md § 4: no slide through the block) a jump at the pair's middle, the sim's own (a placement),
/// counted in `.1`.
///
/// Motion V3: a way longer than `cap` tiles inside one pair (a repaired detour: `v2_audit_4_crossing_c_5x`, a body drawn
/// 38 m in a tick, 4.5x the walk at 5x for twelve frames) is drawn as the chord between its ends, a step the sim's
/// own displacement says; a chord past `cap` too is the sim's jump.
fn along(from: [f32; 2], ahead: &[[f32; 2]], to: [f32; 2], u: f32, cap: f32) -> ([f32; 2], bool) {
    let lerp = |a: [f32; 2], b: [f32; 2], k: f32| [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
    let end = if ahead.last() == Some(&to) { Some(ahead.len() - 1) } else { ahead.iter().position(|p| *p == to) };
    let Some(end) = end else {
        let step = (to[0] - from[0]).abs().max((to[1] - from[1]).abs());
        if step <= 1.5 {
            return (lerp(from, to, u), false);
        }
        return (if u < 0.5 { from } else { to }, true);
    };
    let way = &ahead[..=end];
    let len: f32 = std::iter::once(from)
        .chain(way.iter().copied())
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
        .sum();
    if len > cap {
        let chord = ((to[0] - from[0]).powi(2) + (to[1] - from[1]).powi(2)).sqrt();
        return if chord <= cap { (lerp(from, to, u), false) } else { (if u < 0.5 { from } else { to }, true) };
    }
    (walk_along(from, way, u, None), false)
}

/// Motion V2: a polyline's length and its point `s` along it (clamped).
fn poly_len(pts: &[Vec2]) -> f32 {
    pts.windows(2).map(|w| w[0].distance(w[1])).sum()
}

fn poly_at(pts: &[Vec2], s: f32) -> Vec2 {
    let mut left = s.max(0.0);
    for w in pts.windows(2) {
        let l = w[0].distance(w[1]);
        if left <= l && l > 0.0 {
            return w[0].lerp(w[1], left / l);
        }
        left -= l;
    }
    *pts.last().unwrap_or(&Vec2::ZERO)
}

/// Motion V2 (`OPEN-315`): how far along the polyline `pts` (tiles) the point nearest `p` lies.
fn arc_at(pts: &[[f32; 2]], p: [f32; 2]) -> f32 {
    let p = Vec2::from(p);
    let (mut best, mut acc) = ((f32::MAX, 0.0f32), 0.0f32);
    for w in pts.windows(2) {
        let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
        let ab = b - a;
        let l = ab.length();
        let k = if l > 0.0 { ((p - a).dot(ab) / (l * l)).clamp(0.0, 1.0) } else { 0.0 };
        let d = (a + ab * k).distance(p);
        if d < best.0 {
            best = (d, acc + k * l);
        }
        acc += l;
    }
    best.1
}

/// Motion V2 (`OPEN-308`): a rect in metres `[x0, y0, x1, y1]`: is `p` inside it, and the point of its edge nearest `p`.
fn in_area(r: [f32; 4], p: Vec2) -> bool {
    p.x >= r[0] && p.x <= r[2] && p.y >= r[1] && p.y <= r[3]
}

fn area_edge(r: [f32; 4], p: Vec2) -> Vec2 {
    if !in_area(r, p) {
        return Vec2::new(p.x.clamp(r[0], r[2]), p.y.clamp(r[1], r[3]));
    }
    let d = [p.x - r[0], r[2] - p.x, p.y - r[1], r[3] - p.y];
    let k = (0..4).min_by(|&i, &j| d[i].total_cmp(&d[j])).unwrap_or(0);
    match k {
        0 => Vec2::new(r[0], p.y),
        1 => Vec2::new(r[2], p.y),
        2 => Vec2::new(p.x, r[1]),
        _ => Vec2::new(p.x, r[3]),
    }
}

/// The point `u` (0..1) of the way `from` then `pts` (or, with `tiles`, that many tiles along it, clamped).
fn walk_along(from: [f32; 2], pts: &[[f32; 2]], u: f32, tiles: Option<f32>) -> [f32; 2] {
    let lerp = |a: [f32; 2], b: [f32; 2], k: f32| [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
    let pts: Vec<[f32; 2]> = std::iter::once(from).chain(pts.iter().copied()).collect();
    let len = |a: [f32; 2], b: [f32; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let total: f32 = pts.windows(2).map(|w| len(w[0], w[1])).sum();
    let to = *pts.last().unwrap_or(&from);
    if total <= 0.0 {
        return to;
    }
    let mut left = tiles.map_or(total * u.clamp(0.0, 1.0), |t| t.clamp(0.0, total));
    for w in pts.windows(2) {
        let l = len(w[0], w[1]);
        if left <= l && l > 0.0 {
            return lerp(w[0], w[1], left / l);
        }
        left -= l;
    }
    to
}

fn line_track_of(names: &[String]) -> Vec<u8> {
    names
        .iter()
        .map(|n| {
            if n == "Terrace Line" {
                TRACK_RAIL
            } else if n.starts_with("Tram") {
                TRACK_STREET
            } else {
                TRACK_NONE
            }
        })
        .collect()
}

const ACTS: [&str; 10] = ["walk", "use", "idle", "wait", "ride", "timed", "fly", "jacked_in", "child", "climb"];
const ACT_RGB: [[f32; 3]; 10] = [
    [0.25, 1.0, 0.35],
    [1.0, 0.7, 0.15],
    [0.45, 0.55, 0.7],
    [0.7, 0.4, 1.0],
    [0.2, 0.9, 1.0],
    [1.0, 0.2, 0.9],
    [1.0, 1.0, 1.0],
    [0.3, 0.3, 1.0],
    [0.9, 0.9, 0.5],
    // C7: a climb (the stairs, the lift, its lobby)
    [1.0, 1.0, 0.1],
];
const LOD_RGB: [[f32; 3]; 3] = [[0.25, 1.0, 0.35], [1.0, 0.7, 0.15], [1.0, 0.15, 0.1]];
/// C6b: the signs' colours (a kind's hue), lit while open, dim when shut; a unit founded today burns white.
const SIGN_RGB: [[f32; 3]; 8] = [
    [1.0, 0.2, 0.6],
    [0.2, 0.9, 1.0],
    [1.0, 0.8, 0.1],
    [0.5, 1.0, 0.2],
    [0.7, 0.3, 1.0],
    [1.0, 0.45, 0.1],
    [0.1, 0.5, 1.0],
    [1.0, 0.15, 0.15],
];
const VEH_RGB: [[f32; 3]; 2] = [[0.55, 0.75, 1.0], [1.0, 0.15, 0.1]];
const TRACK_RAIL: u8 = 0;
const TRACK_STREET: u8 = 1;
const TRACK_NONE: u8 = 2;

fn key(v: &Value) -> u64 {
    let a = v.as_array().expect("an id is [index, generation]");
    (a[1].as_u64().unwrap() << 32) | a[0].as_u64().unwrap()
}

fn t2(v: &Value) -> [f32; 2] {
    [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32]
}

impl Sim {
    pub fn load(path: &Path, rate: f32, start: usize, chunk_dir: &Path) -> Sim {
        let txt = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let d: Value = serde_json::from_str(&txt).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(d["schema"], "city-simview/0.1", "{}: not a city-simview/0.1 file", path.display());
        let fr = &d["frame"];
        let names: Vec<String> =
            d["lines"].as_array().map(|ls| ls.iter().map(|l| l["name"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
        let line_track = line_track_of(&names);
        let (rail, deck, rails) = rail_and_deck(chunk_dir);
        let mut frames = vec![];
        for f in d["frames"].as_array().expect("frames") {
            let agents = f["agents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| {
                    let act = a["act"].as_str().unwrap_or("idle");
                    let act = act.split(':').next().unwrap();
                    Agent {
                        id: key(&a["id"]),
                        t: t2(&a["t"]),
                        ahead: a["ahead"].as_array().map_or(vec![], |w| w.iter().map(t2).collect()),
                        indoors: !a["b"].is_null(),
                        floor: a["floor"].as_u64().map_or(0.0, |f| f.min(255) as f32),
                        lod: match a["lod"].as_str() {
                            Some("Full") => 0,
                            Some("Coarse") => 1,
                            _ => 2,
                        },
                        act: ACTS.iter().position(|s| *s == act).unwrap_or(2) as u8,
                        aboard: (!a["aboard"].is_null()).then(|| key(&a["aboard"])),
                        facing: a["facing"].as_f64().map(|h| h as f32),
                        b: (!a["b"].is_null()).then(|| key(&a["b"])),
                        female: a["sex"].as_str() == Some("Female"),
                        age: a["age"].as_u64().map_or(30, |v| v as u32),
                        dress: a["dress"].as_u64().map_or(1, |v| v as u8),
                        gang: !a["colours"].is_null(),
                        rect: None,
                        climb: (act == "climb").then(|| {
                            let x = &a["x"];
                            let n = |k: &str| x[k].as_u64().unwrap_or(0).min(255) as u8;
                            (n("from"), n("to"), x["lift"].as_bool().unwrap_or(false))
                        }),
                        pose: POSE_STAND,
                    }
                })
                .collect();
            let vehicles = f["vehicles"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| Vehicle {
                    id: key(&v["id"]),
                    t: t2(&v["t"]),
                    ahead: v["ahead"].as_array().map_or(vec![], |w| w.iter().map(t2).collect()),
                    ride: match v["ride"].as_str() {
                        Some("Van") => 1,
                        Some("Train") => 2,
                        Some("Flyer") => 3,
                        Some("Foot") => 4,
                        _ => 0,
                    },
                    line: v["route"]["line"].as_u64().map(|l| l as u8),
                    riders: v["riders"].as_array().map_or(0, |r| r.len() as u16),
                    rider_ids: v["riders"].as_array().map_or(vec![], |r| r.iter().map(key).collect()),
                })
                .collect();
            frames.push(SimFrame {
                tick: f["tick"].as_u64().unwrap(),
                clock: f["clock"].as_str().unwrap_or("").into(),
                agents,
                vehicles,
                events: f["events"].as_array().map_or(0, |e| e.len()),
                units: vec![],
                centre: None,
            });
        }
        assert!(!frames.is_empty(), "{}: no frames", path.display());
        info!("sim: {} frames from {} (ticks {}..{})", frames.len(), path.display(), frames[0].tick, frames.last().unwrap().tick);
        Sim {
            tile_m: fr["tile_m"].as_f64().unwrap() as f32,
            x0: fr["x0"].as_f64().unwrap() as f32,
            y1: fr["y1"].as_f64().unwrap() as f32,
            t: start.min(frames.len() - 1) as f32,
            frames,
            line_track,
            line_names: names,
            rail,
            deck,
            rate,
            by_lod: std::env::var("CV_SIM_COLOUR").is_ok_and(|v| v == "lod"),
            ents: HashMap::new(),
            mats: vec![],
            capsule: Handle::default(),
            sign: Handle::default(),
            boxes: Default::default(),
            hud: String::new(),
            live: None,
            live_seq: 0,
            subject: None,
            caption: String::new(),
            subject_at: None,
            people: vec![],
            subject_body: None,
            yaw_of: HashMap::new(),
            capsules: false,
            subject_indoors: None,
            clock_tick: None,
            way: None,
            way_seen: 0,
            no_way: 0,
            lerp: None,
            flat: false,
            motion: Default::default(),
            slot_cache: HashMap::new(),
            way_path: None,
            fight_mark: HashMap::new(),
            fight_t0: None,
            strides: Default::default(),
            hops: Default::default(),
            seat_at: HashMap::new(),
            stir_tiles: 8,
            rails,
            lots: crate::lots::Lots::load(chunk_dir),
            cfg: citysim::config::ViewCfg::default(),
            dir_of: HashMap::new(),
            watch_vehicle: None,
            car: Default::default(),
            car_mats: Default::default(),
            cab: Default::default(),
            cab_mats: Default::default(),
        }
    }

    /// C2: frames from the sim thread (`--save`); the frame (tile_m, y1) and the lines arrive with the first frame.
    pub fn live(shared: std::sync::Arc<crate::simthread::Shared>, chunk_dir: &Path) -> Sim {
        let (rail, deck, rails) = rail_and_deck(chunk_dir);
        Sim {
            tile_m: 4.0,
            x0: 0.0,
            y1: 3750.0,
            frames: vec![],
            line_track: vec![],
            rail,
            deck,
            t: 0.0,
            rate: 0.0,
            by_lod: std::env::var("CV_SIM_COLOUR").is_ok_and(|v| v == "lod"),
            ents: HashMap::new(),
            mats: vec![],
            capsule: Handle::default(),
            sign: Handle::default(),
            boxes: Default::default(),
            hud: "sim: loading the save".into(),
            live: Some(shared),
            live_seq: 0,
            subject: None,
            caption: String::new(),
            subject_at: None,
            people: vec![],
            subject_body: None,
            yaw_of: HashMap::new(),
            capsules: false,
            subject_indoors: None,
            clock_tick: None,
            way: None,
            way_seen: 0,
            no_way: 0,
            lerp: None,
            flat: false,
            motion: Default::default(),
            slot_cache: HashMap::new(),
            way_path: None,
            fight_mark: HashMap::new(),
            fight_t0: None,
            strides: Default::default(),
            hops: Default::default(),
            seat_at: HashMap::new(),
            stir_tiles: 8,
            rails,
            lots: crate::lots::Lots::load(chunk_dir),
            cfg: citysim::config::ViewCfg::default(),
            dir_of: HashMap::new(),
            line_names: vec![],
            watch_vehicle: None,
            car: Default::default(),
            car_mats: Default::default(),
            cab: Default::default(),
            cab_mats: Default::default(),
        }
    }

    /// C2: the newest frame pair from the sim thread; the lerp's `u` (0..1) from the real time since the newer one
    /// and the rate (1 while a bench holds the sim: the shot's tick exactly). `None` until the first frame.
    fn pull_live(&mut self) -> Option<f32> {
        let live = self.live.clone()?;
        let (prev, latest) = live.pair();
        let Some(latest) = latest else {
            self.hud = format!("sim: loading / running to the first frame (tick {})", live.tick.load(std::sync::atomic::Ordering::Relaxed));
            return None;
        };
        if self.live_seq != latest.seq {
            if self.line_track.is_empty()
                && let Some(st) = live.stat.lock().unwrap().clone()
            {
                self.line_track = line_track_of(&st.lines);
                self.line_names = st.lines.clone();
                self.tile_m = st.tile_m;
                self.y1 = st.north_m;
                self.stir_tiles = st.stir_tiles;
            }
            let older = prev.as_ref().unwrap_or(&latest);
            self.frames = vec![SimFrame::from_view(&older.frame), SimFrame::from_view(&latest.frame)];
            self.strides = [older.strides.clone(), latest.strides.clone()];
            self.hops = [older.hops.clone(), latest.hops.clone()];
            self.live_seq = latest.seq;
            self.subject = Some(ekey(latest.subject));
            self.caption = latest.caption.clone();
            // T2 (§ 4): the sim thread publishes the next frame once this one is taken
            live.taken.fetch_max(latest.seq, std::sync::atomic::Ordering::Relaxed);
            if std::env::var_os("CV_LERP_DEBUG").is_some() {
                // OPEN-304's teleport: before T2 the lerp ran at the rate alone, `u` = elapsed x rate, whatever the speed
                let old_u = prev.as_ref().map(|p| (latest.at - p.at).as_secs_f32() * live.rate());
                info!(
                    "lerp: frame {} from {} over {:.2} s; the old lerp (u = elapsed x rate) had reached u {:?} of the last pair when this one came",
                    latest.frame.tick,
                    latest.from_tick,
                    latest.span_s,
                    old_u.map(|u| (u * 1000.0).round() / 1000.0)
                );
            }
        }
        // T2 (§ 5): a click's way, begun at the view's game time now
        let way = live.way.lock().unwrap().clone();
        match way {
            Some(w) if w.seq != self.way_seen => {
                self.way_seen = w.seq;
                self.way = Some((w.seq, w.tiles.iter().map(|t| tf(*t)).collect(), 0.0, w.tiles_per_tick));
            }
            None => self.way = None,
            _ => {}
        }
        self.rate = live.rate();
        let u = live.lerp_u(&latest);
        self.lerp = Some((latest.from_tick, latest.frame.tick, u));
        self.flat = !latest.clocked && latest.span_s <= 0.0;
        Some(u)
    }

    /// A tile's centre in city metres, plus a jitter inside the tile fixed per id (bodies on one tile spread out).
    fn metres(&self, t: [f32; 2], id: u64, jitter: bool) -> Vec2 {
        let mut p = Vec2::new(self.x0 + (t[0] + 0.5) * self.tile_m, self.y1 - (t[1] + 0.5) * self.tile_m);
        if jitter {
            let h = id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let u = ((h >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5;
            let v = ((h >> 40) & 0xFFFF) as f32 / 65535.0 - 0.5;
            p += Vec2::new(u, v) * self.tile_m * 0.7;
        }
        p
    }

    /// V3: where a body indoors is drawn: inside its building's rect (a place fixed per id in its middle 60 %), not
    /// at the door tile its Position holds (that tile is on the facade: drawn there, a body stood on the wall).
    fn indoor_xy(&self, a: &Agent) -> Vec2 {
        let Some([x, y, w, h]) = a.rect else { return self.metres(a.t, a.id, true) };
        let k = a.id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let u = ((k >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5;
        let v = ((k >> 40) & 0xFFFF) as f32 / 65535.0 - 0.5;
        self.metres([x + w / 2.0 - 0.5 + u * w * 0.6, y + h / 2.0 - 0.5 + v * h * 0.6], a.id, false)
    }

    /// V4b: an indoor body's building rect in world metres (x0, y0, x1, y1); its tile without one.
    fn agent_rect_m(&self, a: &Agent) -> Vec4 {
        let [x, y, w, h] = a.rect.unwrap_or([a.t[0], a.t[1], 1.0, 1.0]);
        Vec4::new(self.x0 + x * self.tile_m, self.y1 - (y + h) * self.tile_m, self.x0 + (x + w) * self.tile_m, self.y1 - y * self.tile_m)
    }

    /// V4: a rect in tiles `[x, y, w, h]` as city metres `[x0, y0, x1, y1]`.
    fn rect_m(&self, r: [f32; 4]) -> [f32; 4] {
        let x0 = self.x0 + r[0] * self.tile_m;
        let y1 = self.y1 - r[1] * self.tile_m;
        [x0, y1 - r[3] * self.tile_m, x0 + r[2] * self.tile_m, y1]
    }

    /// V4: where a body inside its building stands on storey `floor` (a climb's fraction): on the export's lot that
    /// has the storey (lots.rs), at that storey's z, a place fixed per id in the middle 60 % of the lot's part of the
    /// building (V10: the part that storey's massing box covers, a tower's plate); `core` stands it at the core (the
    /// lift and the stairs: the top storey's middle). No lot (an old asset tree, a
    /// building off every lot): V3's place in the rect, the ground under it plus the 4 m + 3 m formula.
    fn indoor_place(&self, a: &Agent, floor: f32, core: bool, stream: &Stream) -> (Vec2, f32, Option<crate::lots::Placed>) {
        if let Some(r) = a.rect
            && let Some(pl) = self.lots.place(self.rect_m(r), floor, self.cfg.storey_slab_m)
        {
            let [x0, y0, x1, y1] = pl.area;
            let p = if core {
                Vec2::from(pl.core)
            } else {
                let (u, v) = unit2(a.id);
                Vec2::new((x0 + x1) / 2.0 + u * (x1 - x0) * 0.6, (y0 + y1) / 2.0 + v * (y1 - y0) * 0.6)
            };
            return (p, pl.z, Some(pl));
        }
        let p = self.indoor_xy(a);
        (p, stream.ground_z(p.x, p.y).unwrap_or(0.0) + Sim::storey_z(floor), None)
    }

    /// Motion V2 (`OPEN-308`): a body through a door between the pair's frames, drawn as one walk: going in, along
    /// the street way to the building's edge (the export lot's part on the ground storey, else the sim's rect) where
    /// the way first crosses it (else the edge nearest the last street place), then on inside to the spot it is drawn
    /// at indoors (`indoor_place`); coming out the same backwards, from its drawn indoor spot (the core after a climb).
    /// The sim's own interior tile (wherever the sim puts a body that came in: `OPEN-307`) is never walked to. The
    /// way in metres, the edge's index on it, the indoor z, the area.
    fn door_path(&self, older: &Agent, newer: &Agent, going_in: bool, stream: &Stream) -> (Vec<Vec2>, usize, f32, [f32; 4]) {
        let inner = if going_in { newer } else { older };
        let f = if going_in { 0.0 } else { inner.floor };
        let (spot, z_in, pl) = self.indoor_place(inner, f, !going_in && inner.climb.is_some(), stream);
        let area =
            pl.map_or_else(|| inner.rect.map_or([spot.x - 1.0, spot.y - 1.0, spot.x + 1.0, spot.y + 1.0], |r| self.rect_m(r)), |p| p.area);
        // the street way, tiles: the older place, its way ahead up to the newer one
        let mut way: Vec<[f32; 2]> = vec![older.t];
        let end = older.ahead.iter().position(|p| *p == newer.t);
        way.extend(older.ahead.iter().take(end.map_or(older.ahead.len(), |e| e + 1)).copied());
        if way.last() != Some(&newer.t) && !going_in {
            way.push(newer.t);
        }
        let way: Vec<Vec2> = way.iter().map(|t| self.metres(*t, 0, false)).collect();
        // a place of the way is the building's when it lies in the drawn area or the sim's rect (the sim's interior
        // tiles: the export's lot and the sim's rect disagree by a tile or more), the street's else
        let rect = inner.rect.map(|r| self.rect_m(r));
        let ins = |p: &Vec2| in_area(area, *p) || rect.is_some_and(|r| in_area(r, *p));
        if going_in {
            // the street part: up to the first place inside, then the area's edge nearest the last street place
            let k = way.iter().position(ins).unwrap_or(way.len()).max(1);
            let mut path = way[..k].to_vec();
            let last = *path.last().unwrap();
            path.push(area_edge(area, last));
            let edge = path.len() - 1;
            path.push(spot);
            (path, edge, z_in, area)
        } else {
            // the street part: from the first place outside on (the door's street tile), from the area's edge nearest it
            let k = way.iter().position(|p| !ins(p)).unwrap_or(way.len() - 1);
            let mut path = vec![spot, area_edge(area, way[k])];
            path.extend(way[k..].iter().copied());
            (path, 1, z_in, area)
        }
    }

    /// V4: the rail deck nearest `p` within 20 m: (its index, the plan distance along it).
    fn rail_frame(&self, p: Vec2) -> Option<(usize, f32)> {
        let mut best: Option<(f32, usize, f32)> = None;
        for (i, (pts, acc)) in self.rails.iter().enumerate() {
            for (k, w) in pts.windows(2).enumerate() {
                let (a, b) = (Vec2::new(w[0][0], w[0][1]), Vec2::new(w[1][0], w[1][1]));
                let ab = b - a;
                let l2 = ab.length_squared();
                let t = if l2 > 0.0 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
                let d2 = (a + ab * t - p).length_squared();
                if d2 < 400.0 && best.is_none_or(|x| d2 < x.0) {
                    best = Some((d2, i, acc[k] + (acc[k + 1] - acc[k]) * t));
                }
            }
        }
        best.map(|(_, i, s)| (i, s))
    }

    /// V4: the point (with the deck's z) and the unit plan tangent `s` metres along rail deck `i` (clamped to it).
    fn rail_point(&self, i: usize, s: f32) -> (Vec3, Vec2) {
        let (pts, acc) = &self.rails[i];
        let s = s.clamp(0.0, *acc.last().unwrap());
        let k = acc.partition_point(|x| *x <= s).clamp(1, acc.len() - 1);
        let (a, b) = (pts[k - 1], pts[k]);
        let l = acc[k] - acc[k - 1];
        let t = if l > 0.0 { (s - acc[k - 1]) / l } else { 0.0 };
        let p = Vec3::new(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t);
        (p, Vec2::new(b[0] - a[0], b[1] - a[1]).normalize_or(Vec2::X))
    }

    /// V4, Motion V1: each crowd standing on one ground-level street tile (`standing`: the tile's bodies, two or
    /// more) in slots of its own: the tile's slots (`slot_cands`), `[view] crowd_slots` of them (a bigger crowd: the
    /// next multiple, so a crowd of 9 or 10 does not reshuffle as it changes), each body
    /// (in id order) in the first free one from a slot a hash of its id picks, so two bodies never share a place and
    /// a crowd no bigger than the slots keeps its places as bodies come and go. The bodies' places, by id.
    fn slots(&mut self, stream: &Stream, standing: &HashMap<(i32, i32), Vec<u64>>) -> HashMap<u64, Vec2> {
        let mut out = HashMap::new();
        for (&tile, ids) in standing {
            if ids.len() < 2 {
                continue;
            }
            let k = usize::from(self.cfg.crowd_slots).max(1);
            let m = ids.len().div_ceil(k) * k;
            let cands = self.slot_cands(stream, tile, m);
            if cands.is_empty() {
                continue;
            }
            let pool = m.min(cands.len()) as u64;
            let mut used = vec![false; cands.len()];
            let mut ids = ids.clone();
            ids.sort_unstable();
            for id in ids {
                let pref = (mix64(id ^ 0x5107_5107) % pool) as usize;
                if let Some(k) = (0..cands.len()).map(|i| (pref + i) % cands.len()).find(|&k| !used[k]) {
                    used[k] = true;
                    out.insert(id, cands[k]);
                }
            }
        }
        out
    }

    /// Motion V1: up to `m` slots round a tile's centre, on a sunflower spiral `[view] crowd_gap_m` apart out to
    /// `crowd_spread_m`, on walkable ground at the centre's level (sidewalk, ped, open, stairs; not a carriageway, a
    /// lot's platform or a wall); cached per tile and count once the ground is streamed in.
    fn slot_cands(&mut self, stream: &Stream, tile: (i32, i32), m: usize) -> Vec<Vec2> {
        if let Some(c) = self.slot_cache.get(&(tile, m)) {
            return c.clone();
        }
        let c = self.metres([tile.0 as f32, tile.1 as f32], 0, false);
        let Some(gz) = stream.ground_z(c.x, c.y) else { return vec![] };
        let cap = self.cfg.crowd_spread_m.max(self.tile_m * 0.5);
        // a spiral's points sqrt(k) out at the golden angle sit ~1.77 x the radius step apart
        let step = self.cfg.crowd_gap_m.max(0.1) / 1.77;
        let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
        let mut out = vec![];
        for k in 0.. {
            let r = step * (k as f32 + 0.5).sqrt();
            if r > cap || out.len() >= m {
                break;
            }
            let ang = k as f32 * golden;
            let q = c + Vec2::new(ang.cos(), ang.sin()) * r;
            let walkable = matches!(
                stream.ground_class(q.x, q.y),
                Some(crate::data::SIDEWALK | crate::data::PED | crate::data::OPEN | crate::data::STAIRS)
            );
            if walkable && stream.ground_z(q.x, q.y).is_some_and(|z| (z - gz).abs() < 1.0) {
                out.push(q);
            }
        }
        if self.slot_cache.len() > 4096 {
            self.slot_cache.clear();
        }
        self.slot_cache.insert((tile, m), out.clone());
        out
    }

    /// C6a: a storey's floor over the ground, metres: the export's 4 m ground and 3 m storeys above it (C7: a
    /// climb's storey a fraction, the ground's first 4 m in proportion).
    fn storey_z(floor: f32) -> f32 {
        if floor <= 1.0 { 4.0 * floor.max(0.0) } else { 4.0 + 3.0 * (floor - 1.0) }
    }

    /// C7: a walkway deck's z at `p` (its nearest point within 12 m), else `None`.
    fn deck_z(&self, p: Vec2) -> Option<f32> {
        self.deck
            .iter()
            .map(|q| ((Vec2::new(q[0], q[1]) - p).length_squared(), q[2]))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .filter(|(d2, _)| *d2 < 144.0)
            .map(|(_, z)| z)
    }

    fn rail_z(&self, p: Vec2) -> Option<f32> {
        self.rail
            .iter()
            .map(|q| ((Vec2::new(q[0], q[1]) - p).length_squared(), q[2]))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .filter(|(d2, _)| *d2 < 400.0)
            .map(|(_, z)| z)
    }
}

/// The rail deck's points (`rail_z`), (C7) the walkway decks' (footbridges, the escalator, the stations: `deck_z`)
/// and (V4) each rail deck's centre line with its cumulative plan length (a train's cars laid along it).
type Polyline = (Vec<[f32; 3]>, Vec<f32>);
fn rail_and_deck(chunk_dir: &Path) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<Polyline>) {
    let (mut rail, mut deck, mut rails) = (vec![], vec![], vec![]);
    for e in Layers::load(chunk_dir).elements {
        match e.kind.as_str() {
            "rail" => {
                let mut acc = vec![0.0f32];
                for w in e.pts.windows(2) {
                    let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
                    acc.push(acc.last().unwrap() + d);
                }
                rail.extend(e.pts.iter().copied());
                rails.push((e.pts, acc));
            }
            "walkway" | "escalator" | "station" => deck.extend(e.pts),
            _ => {}
        }
    }
    (rail, deck, rails)
}

fn unlit(mats: &mut Assets<StandardMaterial>, c: [f32; 3], k: f32) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: Color::srgb(c[0] * 0.5, c[1] * 0.5, c[2] * 0.5),
        emissive: LinearRgba::rgb(c[0] * k, c[1] * k, c[2] * k),
        perceptual_roughness: 0.8,
        ..default()
    })
}

pub fn setup_sim(mut sim: ResMut<Sim>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    // placeholders a little over life size so they read at the game zoom (CV_SIM_SCALE)
    let k: f32 = std::env::var("CV_SIM_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.4);
    sim.capsule = meshes.add(Capsule3d::new(0.3 * k, 1.1 * k));
    // car, van, train car (one 20 m car stands for the train), flyer, foot escort marker
    sim.boxes = [
        meshes.add(Cuboid::new(1.9 * k, 1.5 * k, 4.4 * k)),
        meshes.add(Cuboid::new(2.1 * k, 2.3 * k, 5.2 * k)),
        meshes.add(Cuboid::new(3.0, 3.6, 20.0)),
        meshes.add(Cuboid::new(2.5 * k, 1.2 * k, 4.0 * k)),
        meshes.add(Cuboid::new(0.6 * k, 0.1, 0.6 * k)),
    ];
    let mut m = vec![];
    for c in ACT_RGB.iter().chain(LOD_RGB.iter()) {
        m.push(unlit(&mut mats, *c, 3.0));
    }
    // vehicles dimmer: a 20 m train car at the bodies' glow blooms over the street
    for c in VEH_RGB.iter() {
        m.push(unlit(&mut mats, *c, 0.35));
    }
    // C6b: the signs, lit then dim per hue, then the fresh unit's white
    for c in SIGN_RGB.iter() {
        m.push(unlit(&mut mats, *c, 2.2));
    }
    for c in SIGN_RGB.iter() {
        m.push(unlit(&mut mats, [c[0] * 0.25, c[1] * 0.25, c[2] * 0.25], 0.05));
    }
    m.push(unlit(&mut mats, [1.0, 1.0, 0.9], 10.0));
    sim.mats = m;
    // a shop sign: 3.2 m wide, 1.5 m tall, 0.3 m deep (its face along the wall)
    sim.sign = meshes.add(Cuboid::new(3.2, 1.5, 0.3));
    // V4: the El car (`[view] el_car_*`): dark steel with a faint blue glow, a warm lit floor, the roof
    let [body, floor, roof] = car_meshes(&sim.cfg);
    sim.car = [meshes.add(body), meshes.add(floor), meshes.add(roof)];
    let steel = |glow: [f32; 3]| StandardMaterial {
        base_color: Color::srgb(0.20, 0.22, 0.26),
        emissive: LinearRgba::rgb(glow[0], glow[1], glow[2]),
        metallic: 0.5,
        perceptual_roughness: 0.5,
        ..default()
    };
    sim.car_mats = [
        mats.add(steel([0.02, 0.05, 0.09])),
        mats.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.27, 0.22),
            emissive: LinearRgba::rgb(0.3, 0.25, 0.16),
            perceptual_roughness: 0.8,
            ..default()
        }),
        mats.add(steel([0.01, 0.02, 0.04])),
    ];
    // V4: the lift: a unit plate, a unit post (scaled: the car's corners, the shaft's), a unit glass box
    sim.cab = [meshes.add(Cuboid::new(1.0, 0.1, 1.0)), meshes.add(Cuboid::new(0.08, 1.0, 0.08)), meshes.add(Cuboid::new(1.0, 1.0, 1.0))];
    sim.cab_mats = [
        mats.add(steel([0.6, 0.4, 0.12])),
        mats.add(StandardMaterial {
            base_color: Color::srgba(0.6, 0.8, 1.0, 0.15),
            emissive: LinearRgba::rgb(0.03, 0.05, 0.08),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            ..default()
        }),
    ];
}

/// V4: an El car's meshes (local: x across, y up from the car's floor, z along): the body (the sills to 1 m, the
/// window pillars, the band over the windows, the end walls), the floor (lit: the riders read at night), the roof.
fn car_meshes(c: &citysim::config::ViewCfg) -> [Mesh; 3] {
    let (w, h, l) = (c.el_car_w_m, c.el_car_h_m.max(2.2), c.el_car_m.max(4.0));
    let cub = |s: Vec3, at: Vec3| Mesh::from(Cuboid::new(s.x, s.y, s.z)).transformed_by(Transform::from_translation(at));
    let t = 0.12;
    let mut body = cub(Vec3::new(w, h, t), Vec3::new(0.0, h / 2.0, l / 2.0 - t / 2.0));
    let mut add = |m: Mesh| body.merge(&m).expect("cuboids share their attributes");
    add(cub(Vec3::new(w, h, t), Vec3::new(0.0, h / 2.0, -l / 2.0 + t / 2.0)));
    let pillars = (l / 2.5).floor().max(1.0) as i32;
    for side in [-1.0f32, 1.0] {
        let x = side * (w / 2.0 - t / 2.0);
        add(cub(Vec3::new(t, 1.0, l), Vec3::new(x, 0.5, 0.0)));
        add(cub(Vec3::new(t, 0.6, l), Vec3::new(x, h - 0.3, 0.0)));
        for k in 1..pillars {
            let z = -l / 2.0 + k as f32 * l / pillars as f32;
            add(cub(Vec3::new(t, h - 1.6, 0.25), Vec3::new(x, 1.0 + (h - 1.6) / 2.0, z)));
        }
    }
    let floor = cub(Vec3::new(w, 0.25, l), Vec3::new(0.0, -0.125, 0.0));
    let roof = cub(Vec3::new(w + 0.1, 0.15, l + 0.05), Vec3::new(0.0, h + 0.075, 0.0));
    [body, floor, roof]
}

/// What the HUD says the subject is doing (V4: "@ street" read through a climb).
fn subject_line(a: &Agent, f: f32, z: f32, pl: Option<crate::lots::Placed>, lots: &crate::lots::Lots) -> String {
    let lot = pl.map_or(" (no export lot: the 4 m + 3 m formula)".to_string(), |p| {
        let l = &lots.lots[p.lot];
        format!(
            " (lot {}: platform {:.1}, {} storeys, roof {:.1}{})",
            l.id,
            l.z_m,
            l.floors(),
            l.roof(),
            if p.over { "; the sim's storey is past the lot's top" } else { "" }
        )
    });
    match a.climb {
        Some((from, to, lift)) => format!(
            "subject: climbing {} by the {}, floor {f:.1} ({from} -> {to}), z {z:.1}{lot}",
            if to >= from { "up" } else { "down" },
            if lift { "lift" } else { "stairs" }
        ),
        None => format!("subject: inside, floor {:.0}, z {z:.1}{lot}", f.round()),
    }
}

/// Advance the playback and place every placeholder: frame i and i + 1 lerped for the ids in both.
pub fn update_sim(
    mut commands: Commands,
    time: Res<Time>,
    stream: Res<Stream>,
    mut sim: ResMut<Sim>,
    mut q: Query<(&mut Transform, &mut Visibility)>,
    occ: Option<Res<glclient::occlusion::Cutaway>>,
) {
    let (i, j, u) = if sim.live.is_some() {
        let Some(u) = sim.pull_live() else { return };
        (0, 1, u)
    } else {
        let n = sim.frames.len();
        let rate = sim.rate;
        sim.t = (sim.t + time.delta_secs() * rate) % n as f32;
        let i = sim.t.floor() as usize;
        let j = if rate == 0.0 { i } else { (i + 1).min(n - 1) };
        (i, j, if rate == 0.0 { 0.0 } else { sim.t - i as f32 })
    };
    let subject = sim.subject;
    let mut subject_at: Option<(Vec3, f32)> = None;
    let mut subject_in: Option<u64> = None;
    let mut subj_line = String::new();
    // the newer frame's bodies, each lerped from its place in the older one along the older's way ahead (at u = 1
    // exactly the newer frame: a bench's shot shows its tick)
    let prev_a: HashMap<u64, ([f32; 2], Vec<[f32; 2]>)> = sim.frames[i].agents.iter().map(|a| (a.id, (a.t, a.ahead.clone()))).collect();
    let mut no_way = 0usize;
    // T2 (§ 5): the view's game time (the subject's click way is walked in it), the way, and whether it ends here
    let pace = sim.live.as_ref().map_or(0.0, |l| l.tick_rate());
    if let Some(w) = sim.way.as_mut() {
        w.2 += time.delta_secs() * w.3 * pace;
    }
    // Motion V1 (motion.rs): the tracks' frame at the play speed now; a new click way walked from where the subject is
    // drawn (`motion::rejoin`), not from the sim's place a tick ahead
    let speed_x = sim.live.as_ref().map_or(1.0, |l| l.tick_rate() / l.rate().max(1e-9));
    sim.motion.begin(time.delta_secs(), speed_x);
    let cfg = sim.cfg.clone();
    let flat = sim.flat;
    if let Some((seq, tiles, _, _)) = sim.way.as_ref()
        && sim.way_path.as_ref().map(|w| w.0) != Some(*seq)
    {
        let path = match subject.and_then(|s| sim.motion.base_of(s)) {
            Some(from) => crate::motion::rejoin(tiles, from, cfg.way_rejoin_tiles),
            None => tiles.clone(),
        };
        sim.way_path = Some((*seq, path));
    }
    let click_way = sim.way.clone();
    let way_path = sim.way_path.as_ref().map(|w| w.1.clone()).unwrap_or_default();
    let mut way_over = false;
    // C7: a climbing body's storey lerped from the older frame's
    let prev_floor: HashMap<u64, f32> = sim.frames[i].agents.iter().map(|a| (a.id, a.floor)).collect();
    let floor_of = |a: &Agent| prev_floor.get(&a.id).map_or(a.floor, |f| f + (a.floor - f) * u);
    let prev_v: HashMap<u64, ([f32; 2], Vec<[f32; 2]>)> = sim.frames[i].vehicles.iter().map(|v| (v.id, (v.t, v.ahead.clone()))).collect();
    let mut seen: Vec<(u8, u64)> = vec![];
    let mut place =
        |commands: &mut Commands, sim: &mut Sim, k: (u8, u64), mesh: Handle<Mesh>, mat: Handle<StandardMaterial>, tf: Transform| {
            seen.push(k);
            match sim.ents.get(&k).copied() {
                Some(e) => {
                    if let Ok((mut t, mut v)) = q.get_mut(e) {
                        *t = tf;
                        v.set_if_neq(Visibility::Inherited);
                    }
                }
                None => {
                    // V4b: signs and placeholders on cut storeys are cut too
                    let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), tf, Name::new("sim"), glclient::occlusion::CutProp)).id();
                    sim.ents.insert(k, e);
                }
            }
        };
    let at_yaw = |p: Vec3, yaw: f32| Transform::from_translation(p).with_rotation(Quat::from_rotation_y(yaw));
    let (mut street, mut indoors, mut aboard, mut lods) = (0, 0, 0, [0usize; 3]);
    // the most bodies standing on one street tile (a Coarse stack reads as one blob)
    let mut on_tile: HashMap<(i32, i32), usize> = HashMap::new();
    let fr_agents = sim.frames[j].agents.clone();
    // Motion V3: a body new to the pair (no older place: the film's first frames, a promotion) is standing when its act is
    // (Use, Idle, Wait, jacked in), so it takes its crowd slot at once and is drawn there from its first frame
    let still = |a: &Agent| prev_a.get(&a.id).map_or(matches!(a.act, 1 | 2 | 3 | 7), |(p, _)| *p == a.t);
    // V4: the bodies standing on each ground-level street tile: a crowd on one tile takes slots round it (`Sim::slots`)
    let mut standing: HashMap<(i32, i32), Vec<u64>> = HashMap::new();
    for a in fr_agents.iter().filter(|a| !a.indoors && a.aboard.is_none() && a.floor == 0.0 && still(a)) {
        standing.entry((a.t[0] as i32, a.t[1] as i32)).or_default().push(a.id);
    }
    let slots = sim.slots(&stream, &standing);
    // Motion V2: the older frame's bodies whole (a door's walk reads the older place's building and way)
    let older: HashMap<u64, Agent> = sim.frames[i].agents.iter().map(|a| (a.id, a.clone())).collect();
    // Motion V2: walkers stepping between the same two tiles (lockstep: V1's 7990 and 24801 inside each other for 39
    // frames) walk on lanes across their way `[view] lane_gap_m` apart, a lane each by id (a hash's pick, the first
    // free from it), at most `lane_count`; the lane eases in like a slot (motion.rs)
    let lanes: HashMap<u64, Vec2> = {
        let mut groups: HashMap<((i32, i32), (i32, i32)), Vec<u64>> = HashMap::new();
        for a in fr_agents.iter().filter(|a| !a.indoors && a.aboard.is_none() && !still(a)) {
            if let Some(p) = older.get(&a.id).filter(|p| !p.indoors && p.aboard.is_none()) {
                groups.entry(((p.t[0] as i32, p.t[1] as i32), (a.t[0] as i32, a.t[1] as i32))).or_default().push(a.id);
            }
        }
        let n = usize::from(cfg.lane_count.max(1));
        let mut out = HashMap::new();
        for ((from, to), mut ids) in groups {
            // metres: x east, y north (tile y runs south)
            let d = Vec2::new((to.0 - from.0) as f32, (from.1 - to.1) as f32).normalize_or(Vec2::X);
            let across = Vec2::new(-d.y, d.x);
            ids.sort_unstable();
            // more walkers than lanes: rows of them, each row a gap behind the one before
            let m = ids.len().div_ceil(n) * n;
            let mut used = vec![false; m];
            for id in ids {
                let pref = (mix64(id ^ 0x1A4E_5EED) % m as u64) as usize;
                let k = (0..m).map(|i| (pref + i) % m).find(|&k| !used[k]).unwrap_or(pref);
                used[k] = true;
                let (lane, row) = (k % n, k / n);
                if std::env::var("CV_MOTION_DEBUG").is_ok_and(|v| v.split(',').any(|x| x.parse() == Ok(id & 0xFFFF_FFFF))) {
                    eprintln!("lane {}: {from:?} -> {to:?}, lane {lane} of {n}, row {row}", id & 0xFFFF_FFFF);
                }
                out.insert(id, (across * (lane as f32 - (n as f32 - 1.0) / 2.0) - d * row as f32) * cfg.lane_gap_m);
            }
        }
        out
    };
    // V3: a body indoors is drawn only where the cut opens the walls: V4b, the subject's storey of the subject's
    // building while the cutaway runs (the subject indoors: its building is cut at its ceiling, the whole storey
    // shows), else inside the cut box. Every other indoor body stays hidden: drawn on its storey at its jittered tile, it stood on
    // (or through) the facades from the outside (the floating capsules).
    // V10: the kit's own cut (the building the feet are in, the floor it is cut to) for the HUD's subject line
    let own_cut = occ.as_ref().and_then(|o| o.own);
    let cut = occ.filter(|o| o.active());
    let opened = |p: Vec2| cut.as_ref().is_some_and(|c| c.indoors.is_some() || c.in_box(bv(p.x, p.y, 0.0)));
    let mut subject_indoors = None;
    // V4: the subject indoors stands on the export's lot at its storey's z (lots.rs); on a climb at the lot's core.
    let subj = fr_agents.iter().find(|a| Some(a.id) == subject && a.indoors).map(|a| {
        let f = floor_of(a);
        let (p, z, pl) = sim.indoor_place(a, f, a.climb.is_some(), &stream);
        (a.b, f.round(), p, z, pl)
    });
    let mut people = vec![];
    let mut subject_body = None;
    // C5: the bodies lying (knocked out, downed), the subject's fight (always drawn), the replay's poses
    let (down, fight, poses) = match sim.live.as_ref() {
        Some(sh) => (sh.down.lock().unwrap().clone(), sh.fight.lock().unwrap().clone(), sh.poses.lock().unwrap().clone()),
        None => (vec![], vec![], vec![]),
    };
    // V4: the subject's lift car (its place, z, lot)
    let mut cab: Option<(Vec2, f32, Option<crate::lots::Placed>)> = None;
    let (mut hidden_in, mut deck_miss, mut spread_n) = (0usize, 0usize, 0usize);
    // Motion V2 (`OPEN-314`): the subject's fight on the street, staged round the subject: each foe `fight_gap_m` off
    // it (striking range) on its own side, circling at `fight_circle_deg_s`, facing it; the subject faces its mark
    // (the last body it struck or was struck by, the replay's poses)
    if fight.is_empty() {
        sim.fight_mark.clear();
    } else {
        sim.fight_mark.retain(|k, _| fight.contains(k));
        for x in &poses {
            sim.fight_mark.insert(x.key, x.toward);
            if Some(x.toward) == subject {
                sim.fight_mark.entry(x.toward).or_insert(x.key);
            }
        }
    }
    let subj_base: Option<[f32; 2]> = subject
        .filter(|s| fight.iter().any(|k| k != s))
        .and_then(|s| fr_agents.iter().find(|a| a.id == s && !a.indoors && a.aboard.is_none()))
        .map(|a| prev_a.get(&a.id).map_or(a.t, |(p, ah)| along(*p, ah, a.t, u, cfg.way_pair_max_tiles).0));
    let circle = {
        let vt = sim.lerp.map_or(0.0, |(f, t, u)| f as f64 + f64::from(u) * t.saturating_sub(f) as f64);
        let t0 = *sim.fight_t0.get_or_insert(vt);
        if subj_base.is_none() {
            sim.fight_t0 = None;
        }
        // the game-real seconds (1x's) since the fight was first drawn
        let secs = (vt - t0).max(0.0) / f64::from(sim.rate.max(1e-6));
        (secs * f64::from(cfg.fight_circle_deg_s).to_radians()).rem_euclid(std::f64::consts::TAU) as f32
    };
    // each fighter's staged place and facing: round the subject on its own side (the bearing it comes from), the
    // bearings spread at least `360 / n` (at most 60) deg apart in order (two on one tile are not drawn in one place),
    // circling; a foe at `fight_gap_m` facing the subject, a friend a little wider facing its mark or the nearest foe
    let side = sim.live.as_ref().map(|l| l.fight_side.lock().unwrap().clone()).unwrap_or_default();
    let mut staged: HashMap<u64, (Vec2, f32)> = HashMap::new();
    if let Some(sb) = subj_base {
        let s_m = sim.metres(sb, 0, false);
        let mut ring: Vec<(f32, u64)> = fr_agents
            .iter()
            .filter(|a| fight.contains(&a.id) && Some(a.id) != subject && !a.indoors && a.aboard.is_none())
            .filter_map(|a| {
                let t = prev_a.get(&a.id).map_or(a.t, |(p, ah)| along(*p, ah, a.t, u, cfg.way_pair_max_tiles).0);
                let d = sim.metres(t, 0, false) - s_m;
                (d.length() < 6.0 * sim.tile_m).then(|| {
                    let ang = if d.length() > 0.2 { d.y.atan2(d.x) } else { (unit2(a.id ^ 0xF16).0 + 0.5) * std::f32::consts::TAU };
                    (ang, a.id)
                })
            })
            .collect();
        ring.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        let sep = (std::f32::consts::TAU / ring.len().max(1) as f32).min(60f32.to_radians());
        for k in 1..ring.len() {
            ring[k].0 = ring[k].0.max(ring[k - 1].0 + sep);
        }
        let foes: Vec<Vec2> = ring
            .iter()
            .filter(|(_, id)| !side.contains(id))
            .map(|(ang, _)| s_m + Vec2::from_angle(ang + circle) * cfg.fight_gap_m)
            .collect();
        for (ang, id) in &ring {
            let dir = Vec2::from_angle(ang + circle);
            let (at, look) = if side.contains(id) {
                let at = s_m + dir * cfg.fight_gap_m * 1.4;
                let mark = sim.fight_mark.get(id).and_then(|m| sim.motion.at_of(*m));
                let look = mark
                    .or_else(|| foes.iter().copied().min_by(|x, y| x.distance(at).total_cmp(&y.distance(at))))
                    .unwrap_or(s_m + dir * 3.0);
                (at, look)
            } else {
                (s_m + dir * cfg.fight_gap_m, s_m)
            };
            let d = look - at;
            staged.insert(*id, (at, -d.x.atan2(d.y)));
        }
    }
    let mut walked_set: Option<f32> = None;
    for a in &fr_agents {
        // Motion V2 (`OPEN-308`): through a door inside the pair: one walk along the street to the building's edge and
        // on inside to its indoor spot (or out from it), `door_path`; inside the walls it shows where the cut opens
        let door = older.get(&a.id).filter(|o| u < 1.0 && o.aboard.is_none() && a.aboard.is_none() && o.indoors != a.indoors);
        lods[a.lod as usize] += 1;
        // the way ahead's heading (compass degrees -> Bevy yaw about +y, heading 0 = -z), else the last one drawn
        let yaw =
            match a.facing {
                Some(h) => {
                    sim.yaw_of.insert(a.id, -h.to_radians());
                    -h.to_radians()
                }
                // never seen moving: a heading fixed per id (a crowd does not stand all one way)
                None => sim.yaw_of.get(&a.id).copied().unwrap_or_else(|| {
                    (a.id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f32 / (1u64 << 24) as f32 * std::f32::consts::TAU
                }),
            };
        // Motion V1: walking or not, and the clip's rate, are the drawn ground speed's (motion.rs)
        // Motion V2: the posture (people.rs `Posing`): asleep lies, a sitting Use sits, the subject's fight guards
        let in_fight = fight.contains(&a.id) || (Some(a.id) == subject && fight.iter().any(|k| Some(*k) != subject));
        let draw = |at: Vec3, d: crate::motion::Drawn| {
            let rest = !d.walking;
            crate::people::BodyDraw {
                id: a.id,
                at,
                yaw: d.yaw,
                walking: d.walking,
                clip_rate: d.clip_rate,
                female: a.female,
                age: a.age,
                dress: a.dress,
                gang: a.gang,
                lean: 0.0,
                nudge: Vec2::ZERO,
                lying: down.contains(&a.id) || (rest && a.pose == POSE_LIE),
                pose: crate::people::Posing {
                    sit: if rest && a.pose == POSE_SIT { 1.0 } else { 0.0 },
                    guard: if in_fight { 1.0 } else { 0.0 },
                    run: if in_fight { 0.0 } else { d.run },
                    ..Default::default()
                },
            }
        };
        let is_subject = Some(a.id) == subject;
        if is_subject {
            subject_in = a.aboard;
        }
        if let Some(o) = door {
            let going_in = a.indoors;
            street += 1;
            let (path, edge, z_in, area) = sim.door_path(o, a, going_in, &stream);
            let len = poly_len(&path);
            // a street part longer than twice the snap a tick of the pair (a promoted body sent from its stale place,
            // `OPEN-312`) is the sim's jump: drawn as one at the pair's middle, not a 30 m/s walk
            let ticks = sim.lerp.map_or(1, |(f, t, _)| t.saturating_sub(f).max(1)) as f32;
            let street_m = if going_in { poly_len(&path[..=edge]) } else { len - poly_len(&path[..=edge]) };
            let leap = street_m > 2.0 * cfg.motion_snap_m * ticks;
            let s = if leap { if u < 0.5 { 0.0 } else { len } } else { u * len };
            let p = poly_at(&path, s);
            let edge_s = poly_len(&path[..=edge]);
            let inside = if going_in { s > edge_s } else { s < edge_s };
            let g_edge = stream.ground_z(path[edge].x, path[edge].y).unwrap_or(0.0);
            let z = if !inside {
                stream.ground_z(p.x, p.y).unwrap_or(g_edge)
            } else if going_in {
                g_edge + (z_in - g_edge) * ((s - edge_s) / (len - edge_s).max(1e-3)).clamp(0.0, 1.0)
            } else {
                z_in + (g_edge - z_in) * (s / edge_s.max(1e-3)).clamp(0.0, 1.0)
            };
            let dir = poly_at(&path, s + 0.3) - poly_at(&path, s - 0.3);
            let yaw = if dir.length_squared() > 1e-6 { -dir.x.atan2(dir.y) } else { yaw };
            sim.yaw_of.insert(a.id, yaw);
            let t = [(p.x - sim.x0) / sim.tile_m - 0.5, (sim.y1 - p.y) / sim.tile_m - 0.5];
            let d = sim.motion.draw(a.id, p, t, Vec2::ZERO, yaw, leap || flat, false, cfg.motion_offset_m_s, &cfg);
            no_way += usize::from(leap);
            let at = Vec3::new(d.at.x, d.at.y, z);
            if is_subject {
                way_over = true;
                if inside {
                    subject_indoors = Some(Vec4::from(area));
                }
                subject_at = Some((at, d.yaw));
                subject_body = Some(draw(at, d));
                subj_line = format!(
                    "subject: through a door ({}), {}",
                    if going_in { "in" } else { "out" },
                    if inside { "inside" } else { "outside" }
                );
            } else if inside && !opened(d.at) {
                hidden_in += 1;
            } else if sim.capsules {
                let mat = if sim.by_lod { sim.mats[ACTS.len() + a.lod as usize].clone() } else { sim.mats[a.act as usize].clone() };
                let cap = sim.capsule.clone();
                place(&mut commands, &mut sim, (0, a.id), cap, mat, at_yaw(bv(at.x, at.y, at.z + 0.85 * 1.4), d.yaw));
            } else {
                people.push(draw(at, d));
            }
            continue;
        }
        if a.indoors {
            // T2: the click way ends at a door gone through (or a ride)
            way_over |= is_subject;
            // C6a: on its floor, seen only in the cut (above)
            indoors += 1;
            let f = floor_of(a);
            let (mut p, z, mut yaw) = match (&subj, is_subject) {
                (Some(s), true) => (s.2, s.3, yaw),
                // C5 (`OPEN-297`): a body in the subject's fight is always drawn (a joiner on another storey too)
                (Some(s), false) if s.0.is_some() && s.0 == a.b && fight.contains(&a.id) => {
                    // round the subject at `fight_ring_m`, a place fixed per id, facing it
                    let ang = (unit2(a.id).0 + 0.5) * std::f32::consts::TAU;
                    let p = s.2 + Vec2::new(ang.cos(), ang.sin()) * sim.cfg.fight_ring_m;
                    let d = s.2 - p;
                    (p, s.3, -d.x.atan2(d.y))
                }
                (Some(s), false) if s.0.is_some() && s.0 == a.b && f.round() == s.1 => {
                    let (p, z, _) = sim.indoor_place(a, f, false, &stream);
                    if !opened(p) {
                        hidden_in += 1;
                        continue;
                    }
                    (p, z, yaw)
                }
                _ => {
                    hidden_in += 1;
                    continue;
                }
            };
            if is_subject {
                let pl = subj.as_ref().and_then(|s| s.4);
                // V4b's cutaway rect: the export lot's part of the building when there is one (the sim rect spills past the massing)
                subject_indoors = Some(pl.map(|l| Vec4::from(l.area)).unwrap_or_else(|| sim.agent_rect_m(a)));
                if let Some((from, to, lift)) = a.climb {
                    if lift {
                        cab = Some((p, z, pl));
                    } else {
                        // the stairs: round the core, a turn a storey, facing the way round
                        let ang = f * std::f32::consts::TAU;
                        let sign = if to >= from { 1.0 } else { -1.0 };
                        p += Vec2::new(ang.cos(), ang.sin()) * sim.cfg.stair_radius_m;
                        let d = Vec2::new(-ang.sin(), ang.cos()) * sign;
                        yaw = -d.x.atan2(d.y);
                    }
                }
                subj_line = subject_line(a, f, z, pl, &sim.lots);
                subj_line += &own_cut.map_or(", kit cut: none".to_string(), |(b, fl)| format!(", kit cut: building {b} at its floor {fl}"));
            }
            let d = sim.motion.draw(a.id, p, a.t, Vec2::ZERO, yaw, flat, false, cfg.motion_offset_m_s, &cfg);
            let p = if cab.is_some() && is_subject { p } else { d.at };
            if is_subject {
                subject_at = Some((Vec3::new(p.x, p.y, z), d.yaw));
                subject_body = Some(draw(Vec3::new(p.x, p.y, z), d));
            }
            if sim.capsules {
                let mat = if sim.by_lod { sim.mats[ACTS.len() + a.lod as usize].clone() } else { sim.mats[a.act as usize].clone() };
                let cap = sim.capsule.clone();
                place(&mut commands, &mut sim, (0, a.id), cap, mat, at_yaw(bv(p.x, p.y, z + 0.85 * 1.4), 0.0));
            } else if !is_subject {
                people.push(draw(Vec3::new(p.x, p.y, z), d));
            }
            continue;
        }
        if a.aboard.is_some() {
            way_over |= is_subject;
            // drawn inside its vehicle (below)
            aboard += 1;
            continue;
        }
        street += 1;
        *on_tile.entry((a.t[0] as i32, a.t[1] as i32)).or_default() += 1;
        let (mut t, jump) = prev_a.get(&a.id).map_or((a.t, false), |(p, ah)| along(*p, ah, a.t, u, cfg.way_pair_max_tiles));
        no_way += usize::from(jump);
        // Motion V2 (S1's stride, `OPEN-317`): each frame's place is its tile plus the way through the next step, so
        // the pair covers the same ground every tick (the tiles alone ran 4, 3, 4 a tick: a 25 % surge)
        // R4 (`OPEN-319`): a walkway hop (stairs, a lift, a deck's span) in either frame of the pair: each frame's place
        // is the hop's near end plus its share gone toward the far end (the sim stands the body on the near end the
        // hop's time and steps it onto the far end when the share completes), and its height the same share between
        // the two ends' (`hop_z` below); the pair lerps from one frame's place to the other's
        let hop_of = |k: usize, g: Option<&Agent>| -> Option<([f32; 2], f32, f32)> {
            let g = g?;
            match sim.hops[k].get(&g.id) {
                Some(h) => {
                    let s = h.share.clamp(0.0, 1.0);
                    let (f, e) = ([f32::from(h.from.x), f32::from(h.from.y)], [f32::from(h.to.x), f32::from(h.to.y)]);
                    Some(([f[0] + (e[0] - f[0]) * s, f[1] + (e[1] - f[1]) * s], f32::from(h.from_level), s))
                }
                None => Some((g.t, g.floor, -1.0)),
            }
        };
        let hopping = sim.hops[0].contains_key(&a.id) || sim.hops[1].contains_key(&a.id);
        let mut hop_z: Option<[(f32, [f32; 2], f32); 2]> = None;
        if hopping && let (Some(o), Some(n)) = (hop_of(0, older.get(&a.id)), hop_of(1, Some(a))) {
            t = [o.0[0] + (n.0[0] - o.0[0]) * u, o.0[1] + (n.0[1] - o.0[1]) * u];
            hop_z = Some([(o.1, o.0, o.2), (n.1, n.0, n.2)]);
        }
        if !jump && !hopping {
            let off = |s: &HashMap<u64, f32>, g: Option<&Agent>| {
                g.and_then(|g| {
                    let k = *s.get(&g.id)?;
                    let n = *g.ahead.first()?;
                    let d = [n[0] - g.t[0], n[1] - g.t[1]];
                    (d[0].abs().max(d[1].abs()) <= 1.5).then_some([d[0] * k, d[1] * k])
                })
                .unwrap_or([0.0, 0.0])
            };
            let (o, n) = (off(&sim.strides[0], older.get(&a.id)), off(&sim.strides[1], Some(a)));
            t = [t[0] + o[0] + (n[0] - o[0]) * u, t[1] + o[1] + (n[1] - o[1]) * u];
        }
        // T2 (§ 5): the subject on its click way walks it from the click, until the frames reach its end or leave it
        let mut yaw = yaw;
        if is_subject && let Some((_, way, walked, _)) = click_way.as_ref() {
            let on_way = way.contains(&a.t);
            let end = *way.last().unwrap_or(&a.t);
            // Motion V1: over once both the frames and the drawn walk (from where the body was drawn at the click,
            // `way_path`) have reached its end: the drawn body waits there for the frames, or walks on to it
            let path = if way_path.is_empty() { way } else { &way_path };
            let len: f32 = path.windows(2).map(|w| Vec2::from(w[0]).distance(Vec2::from(w[1]))).sum();
            let done = (t[0] - end[0]).abs() < 0.05 && (t[1] - end[1]).abs() < 0.05 && *walked >= len - 1e-3;
            if !on_way || done {
                way_over = true;
            } else {
                // Motion V2 (`OPEN-315`): the drawn walk never behind the frames' own lerp (a sim run on ahead, flat
                // out to a script's line, takes it along) nor more than `way_lead_tiles` past the sim's newer tile (a
                // sim holding the body at a segment's end or a door holds the drawn one)
                let lo = arc_at(path, t);
                let hi = arc_at(path, a.t) + cfg.way_lead_tiles;
                let d = walked.min(hi).max(lo);
                walked_set = Some(d);
                let w = walk_along(path[0], &path[1..], 0.0, Some(d));
                let ahead = walk_along(path[0], &path[1..], 0.0, Some(d + 0.5));
                let (dx, dy) = (ahead[0] - w[0], w[1] - ahead[1]);
                if dx.abs() + dy.abs() > 1e-3 {
                    yaw = -dx.atan2(dy);
                    sim.yaw_of.insert(a.id, yaw);
                }
                t = w;
            }
        }
        // V4: a crowd standing on one tile in its slots round it (`Sim::slots`), else a lane's jitter in the tile;
        // Motion V1: the offset eased, the drawn place kept from jumping (motion.rs)
        let base = sim.metres(t, a.id, false);
        let mut want = match (slots.get(&a.id), lanes.get(&a.id)) {
            (Some(q), _) => {
                spread_n += 1;
                *q - base
            }
            // Motion V2: a walker on its lane
            (None, Some(l)) => *l,
            (None, None) => sim.metres(t, a.id, true) - base,
        };
        // Motion V2 (`OPEN-314`): the subject's fight staged round the subject (above): a foe at striking range on its
        // side, circling, facing the subject; the subject on its tile, facing its mark
        let mut face = false;
        let mut ease = cfg.motion_offset_m_s;
        if let Some(sb) = subj_base.filter(|_| is_subject || fight.contains(&a.id)) {
            let s_m = sim.metres(sb, 0, false);
            if is_subject {
                want = Vec2::ZERO;
                // its mark's staged place, else the nearest foe's (the staged ring, then where the foe is drawn)
                // V3: only a staged foe is faced (a mark that is not in the staged ring, a body struck in passing or
                // one that walked off, is drawn elsewhere and the subject turned its back on the fight to face it)
                let near = |k: &u64| staged.get(k).map(|s| s.0);
                let mark = sim.fight_mark.get(&a.id).filter(|m| !side.contains(m)).and_then(near).or_else(|| {
                    fight
                        .iter()
                        .filter(|k| **k != a.id && !side.contains(k))
                        .filter_map(near)
                        .min_by(|x, y| x.distance(s_m).total_cmp(&y.distance(s_m)))
                });
                if let Some(m) = mark.filter(|m| m.distance(s_m) > 0.05) {
                    let d = m - s_m;
                    yaw = -d.x.atan2(d.y);
                    face = true;
                }
            } else if let Some((at, look)) = staged.get(&a.id) {
                want = *at - base;
                yaw = *look;
                ease = cfg.fight_close_m_s;
                face = true;
            }
        } else if let Some(sb) = subj_base.filter(|_| still(a))
            && Vec2::from(sb).distance(Vec2::from(a.t)) <= f32::from(sim.stir_tiles)
        {
            // Motion V2 (S2's stir): a body standing near the subject's fight watches it
            let d = sim.metres(sb, 0, false) - (base + want);
            if d.length() > 0.3 {
                yaw = -d.x.atan2(d.y);
                face = true;
            }
        }
        // Motion V2 (`OPEN-309`): off the El inside the pair: from its seat in the car to its place on the platform
        let seat = sim.seat_at.get(&a.id).copied().filter(|_| u < 1.0 && older.get(&a.id).is_some_and(|o| o.aboard.is_some()));
        let (base, want) = match seat {
            Some(s) => (s.truncate().lerp(base + want, u), Vec2::ZERO),
            None => (base, want),
        };
        let d = sim.motion.draw(a.id, base, t, want, yaw, (jump && seat.is_none()) || flat, face, ease, &cfg);
        let (p, yaw) = (d.at, d.yaw);
        let g = stream.ground_z(p.x, p.y).unwrap_or(0.0);
        // C7: a body on a walkway deck stands on it (no deck point within 12 m: at the storey's height, counted)
        let deck = if a.floor > 0.0 { sim.deck_z(p) } else { None };
        deck_miss += usize::from(a.floor > 0.0 && deck.is_none());
        let z = if a.floor > 0.0 { deck.unwrap_or(g + Sim::storey_z(a.floor)) } else { g };
        // R4: a hop's height: each frame's (a hop's share between its near end's and its far end's), lerped
        let z = match hop_z {
            Some(fr) => {
                let at = |t: [f32; 2], fl: f32| {
                    let m = sim.metres(t, 0, false);
                    let g = stream.ground_z(m.x, m.y).unwrap_or(0.0);
                    if fl > 0.0 { sim.deck_z(m).unwrap_or(g + Sim::storey_z(fl)) } else { g }
                };
                let zk = |k: usize, (fl, pl, s): (f32, [f32; 2], f32)| match sim.hops[k].get(&a.id) {
                    Some(h) if s >= 0.0 => {
                        let near = at([f32::from(h.from.x), f32::from(h.from.y)], f32::from(h.from_level));
                        let far = at([f32::from(h.to.x), f32::from(h.to.y)], f32::from(h.to_level));
                        near + (far - near) * s
                    }
                    _ => at(pl, fl),
                };
                let (z0, z1) = (zk(0, fr[0]), zk(1, fr[1]));
                z0 + (z1 - z0) * u
            }
            None => z,
        };
        let z = seat.map_or(z, |s| s.z + (z - s.z) * u);
        if is_subject {
            subject_at = Some((Vec3::new(p.x, p.y, z), yaw));
            subject_body = Some(draw(Vec3::new(p.x, p.y, z), d));
            subj_line = if a.floor > 0.0 {
                format!("subject: on a deck (level {:.0}), z {z:.1}", a.floor)
            } else {
                format!("subject: on the street, z {z:.1}")
            };
        }
        if sim.capsules {
            let mat = if sim.by_lod { sim.mats[ACTS.len() + a.lod as usize].clone() } else { sim.mats[a.act as usize].clone() };
            let cap = sim.capsule.clone();
            place(&mut commands, &mut sim, (0, a.id), cap, mat, at_yaw(bv(p.x, p.y, z + 0.85 * 1.4), yaw));
        } else if !is_subject {
            people.push(draw(Vec3::new(p.x, p.y, z), d));
        }
    }
    sim.motion.end();
    // V4: the subject's lift car in its shaft (the lot's middle, platform to roof): two plates, four posts, glass
    if let Some((p, z, pl)) = cab {
        let (s, h) = (sim.cfg.lift_cab_m, sim.cfg.lift_cab_h_m);
        let [plate, post, glass] = sim.cab.clone();
        let [steel, glass_m] = sim.cab_mats.clone();
        let sc = |m: Vec3, at: Vec3| Transform::from_translation(at).with_scale(m);
        place(&mut commands, &mut sim, (8, 0), plate.clone(), steel.clone(), sc(Vec3::new(s, 1.0, s), bv(p.x, p.y, z - 0.05)));
        place(&mut commands, &mut sim, (8, 1), plate, steel.clone(), sc(Vec3::new(s, 1.0, s), bv(p.x, p.y, z + h)));
        place(&mut commands, &mut sim, (8, 2), glass, glass_m, sc(Vec3::new(s, h, s), bv(p.x, p.y, z + h / 2.0)));
        for (k, (dx, dy)) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].into_iter().enumerate() {
            let c = Vec2::new(p.x + dx * s / 2.0, p.y + dy * s / 2.0);
            place(
                &mut commands,
                &mut sim,
                (8, 3 + k as u64),
                post.clone(),
                steel.clone(),
                sc(Vec3::new(1.0, h, 1.0), bv(c.x, c.y, z + h / 2.0)),
            );
            if let Some(pl) = pl {
                let l = &sim.lots.lots[pl.lot];
                let (z0, z1) = (l.z_m, l.roof());
                let c = Vec2::new(p.x + dx * (s / 2.0 + 0.2), p.y + dy * (s / 2.0 + 0.2));
                let m = sc(Vec3::new(1.0, (z1 - z0).max(0.1), 1.0), bv(c.x, c.y, (z0 + z1) / 2.0));
                place(&mut commands, &mut sim, (9, k as u64), post.clone(), steel.clone(), m);
            }
        }
    }
    sim.subject_indoors = subject_indoors;
    // Motion V2 (`OPEN-315`): the click way's drawn walk as clamped to the frames this frame
    if let (Some(d), Some(w)) = (walked_set, sim.way.as_mut())
        && click_way.as_ref().map(|c| c.0) == Some(w.0)
    {
        w.2 = d;
    }
    if way_over && sim.way.as_ref().map(|w| w.0) == click_way.as_ref().map(|w| w.0) {
        sim.way = None;
    }
    let fr_veh: Vec<Vehicle> = sim.frames[j].vehicles.clone();
    // V4: the riders' looks (a rider whose vehicle is outside the ring is not in the frame: a look by its id)
    let looks: HashMap<u64, Agent> = fr_agents.iter().filter(|a| a.aboard.is_some()).map(|a| (a.id, a.clone())).collect();
    // `--watch-vehicle 0`: the train with the most riders now
    let watch = sim.watch_vehicle.and_then(|w| match w {
        0 => fr_veh.iter().filter(|v| v.ride == 2 && v.riders > 0).max_by_key(|v| (v.riders, std::cmp::Reverse(v.id))).map(|v| v.id),
        w => fr_veh.iter().find(|v| (v.id & 0xFFFF_FFFF) as u32 == w).map(|v| v.id),
    });
    let mut watch_at: Option<(Vec3, f32)> = None;
    let (mut by_ride, mut riders, mut riders_drawn) = ([0usize; 5], 0usize, 0usize);
    // Motion V2 (`OPEN-309`): the riders' seats this frame (a rider getting off inside the pair keeps the last one)
    let mut seats: HashMap<u64, Vec3> = sim
        .seat_at
        .iter()
        .filter(|(k, _)| u < 1.0 && fr_agents.iter().any(|a| a.id == **k && a.aboard.is_none()))
        .map(|(k, v)| (*k, *v))
        .collect();
    for v in &fr_veh {
        by_ride[v.ride as usize] += 1;
        riders += usize::from(v.riders);
        let (t, jump) = prev_v.get(&v.id).map_or((v.t, false), |(q, ah)| along(*q, ah, v.t, u, f32::MAX));
        no_way += usize::from(jump);
        let p = sim.metres(t, v.id, false);
        // heading: from the older place to the newer, else along the newer's way ahead
        let to = |a: [f32; 2]| sim.metres(a, v.id, false);
        let motion = prev_v.get(&v.id).map(|(q, _)| to(v.t) - to(*q)).filter(|d| d.length() > 0.01);
        let ahead = motion.or_else(|| v.ahead.first().map(|n| to(*n) - to(v.t)).filter(|d| d.length() > 0.01));
        let yaw = ahead.map_or(0.0, |d| -d.x.atan2(d.y));
        let track = v.line.and_then(|l| sim.line_track.get(l as usize).copied());
        // V4: an El train on a rail deck: `el_cars` cars laid along the deck's centre line on its left track, riders
        // inside along the walls; the subject's (and the watched) train roofless so the camera sees in
        if v.ride == 2
            && track != Some(TRACK_STREET)
            && let Some((ri, s)) = sim.rail_frame(p)
        {
            let (_, tan) = sim.rail_point(ri, s);
            let sign = match motion {
                Some(m) => {
                    if m.dot(tan) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                None => sim.dir_of.get(&v.id).copied().unwrap_or(1.0),
            };
            sim.dir_of.insert(v.id, sign);
            let open = subject_in == Some(v.id) || watch == Some(v.id);
            let c = sim.cfg.clone();
            let n = usize::from(c.el_cars.max(1));
            let pitch = c.el_car_m + c.el_car_gap_m;
            let [body, floor, roof] = sim.car.clone();
            let [body_m, floor_m, roof_m] = sim.car_mats.clone();
            let mut cars: Vec<(Vec2, f32, Vec2)> = vec![];
            for k in 0..n {
                let (q, tk) = sim.rail_point(ri, s + (k as f32 - (n as f32 - 1.0) / 2.0) * pitch * sign);
                let at = q.truncate() + Vec2::new(-tk.y, tk.x) * sign * c.el_track_m;
                let z = q.z + c.el_floor_m;
                cars.push((at, z, tk));
                let tf = at_yaw(bv(at.x, at.y, z), tk.x.atan2(-tk.y));
                let key = v.id ^ ((k as u64 + 1) << 56);
                place(&mut commands, &mut sim, (5, key), body.clone(), body_m.clone(), tf);
                place(&mut commands, &mut sim, (6, key), floor.clone(), floor_m.clone(), tf);
                if !open {
                    place(&mut commands, &mut sim, (7, key), roof.clone(), roof_m.clone(), tf);
                }
            }
            // the riders' places: along both walls of every car, a slot per rider picked by its id (probing past a
            // taken one), the overflow in the aisle
            let rows = ((c.el_car_m - 3.0) / c.el_slot_m.max(0.3)).floor().max(1.0) as usize;
            let per = 2 * rows;
            let total = per * n;
            let mut taken = vec![false; total];
            let mut ids = v.rider_ids.clone();
            ids.sort_unstable();
            for id in ids {
                let h = mix64(id);
                let mut k = (h % total as u64) as usize;
                let mut tries = 0;
                while taken[k] && tries < total {
                    k = (k + 1) % total;
                    tries += 1;
                }
                let (car, lane, row) = if tries < total {
                    taken[k] = true;
                    (k / per, (k % per) / rows, k % rows)
                } else {
                    ((h >> 8) as usize % n, 2, (h >> 24) as usize % rows)
                };
                let (at, z, tk) = cars[car];
                let nrm = Vec2::new(-tk.y, tk.x);
                let x = match lane {
                    0 => -(c.el_car_w_m / 2.0 - 0.55),
                    1 => c.el_car_w_m / 2.0 - 0.55,
                    _ => 0.0,
                };
                let along_m = -c.el_car_m / 2.0 + 1.5 + (row as f32 + 0.5) * (c.el_car_m - 3.0) / rows as f32;
                let pos = at + nrm * x + tk * along_m;
                // facing in across the car; in the aisle along it
                let face = if lane == 2 { tk } else { nrm * -x.signum() };
                let yaw_b = -face.x.atan2(face.y);
                let look = looks.get(&id);
                let bd = crate::people::BodyDraw {
                    id,
                    at: pos.extend(z),
                    yaw: yaw_b,
                    walking: false,
                    clip_rate: 1.0,
                    female: look.map_or(h & 1 == 0, |a| a.female),
                    age: look.map_or(30, |a| a.age),
                    dress: look.map_or(1, |a| a.dress),
                    gang: look.is_some_and(|a| a.gang),
                    lean: 0.0,
                    nudge: Vec2::ZERO,
                    lying: false,
                    // Motion V2: riders stand (the car has no seats to sit on)
                    pose: Default::default(),
                };
                riders_drawn += 1;
                seats.insert(id, pos.extend(z));
                // Motion V2 (`OPEN-309`): on the El inside the pair: a walk from its place on the platform to the seat
                let mut bd = bd;
                if u < 1.0
                    && let Some(o) = older.get(&id).filter(|o| o.aboard.is_none() && !o.indoors)
                {
                    let from = sim.metres(o.t, id, false);
                    let fz = if o.floor > 0.0 { sim.deck_z(from) } else { stream.ground_z(from.x, from.y) }.unwrap_or(z);
                    let at = from.extend(fz).lerp(bd.at, u);
                    let dir = bd.at.truncate() - from;
                    if dir.length() > 0.5 {
                        bd.yaw = -dir.x.atan2(dir.y);
                        bd.walking = true;
                        bd.clip_rate = 1.5;
                    }
                    bd.at = at;
                }
                if Some(id) == subject {
                    subject_at = Some((bd.at, bd.yaw));
                    subject_body = Some(bd);
                    let line = v.line.and_then(|l| sim.line_names.get(usize::from(l))).map_or("El", |s| s.as_str());
                    subj_line = format!(
                        "subject: aboard the {line} (train #{}), car {} of {n}, {} riders, floor z {z:.1}",
                        v.id & 0xFFFF_FFFF,
                        car + 1,
                        v.riders
                    );
                } else if !sim.capsules {
                    people.push(bd);
                }
            }
            if watch == Some(v.id) {
                let (at, z, tk) = cars[n / 2];
                watch_at = Some((at.extend(z), -tk.x.atan2(tk.y)));
                let line = v.line.and_then(|l| sim.line_names.get(usize::from(l))).map_or("El", |s| s.as_str());
                subj_line = format!("watching the {line} train #{} ({} riders), floor z {z:.1}", v.id & 0xFFFF_FFFF, v.riders);
            }
            continue;
        }
        let ground = stream.ground_z(p.x, p.y).unwrap_or(0.0);
        let (z, red) = match (v.ride, track) {
            (2, Some(TRACK_RAIL)) => (sim.rail_z(p).map_or(ground + 12.0, |z| z + 2.6), false),
            (2, Some(TRACK_STREET)) => (ground + 1.8, false),
            // C3: a routed El line on its deck (rail.json); a line with no track here (OPEN-158): raised and red
            (2, _) => sim.rail_z(p).map_or((ground + 10.0, true), |z| (z + 2.6, false)),
            _ => (ground + 0.9, false),
        };
        let mat = sim.mats[ACTS.len() + LOD_RGB.len() + usize::from(red)].clone();
        if subject_in == Some(v.id) {
            // a road vehicle: the subject sits low in its box
            subject_at = Some((Vec3::new(p.x, p.y, ground + 0.2), yaw));
            subj_line = format!("subject: aboard a vehicle (ride {}), {} riders", v.ride, v.riders);
        }
        if watch == Some(v.id) {
            watch_at = Some((Vec3::new(p.x, p.y, z), yaw));
        }
        let mesh = sim.boxes[v.ride as usize].clone();
        place(&mut commands, &mut sim, (1, v.id), mesh, mat, at_yaw(bv(p.x, p.y, z), yaw));
    }
    sim.seat_at = seats;
    // C5: the replay's poses: a lunge at the struck, a stagger away from the striker (and the striker faces its mark)
    if !poses.is_empty() {
        // Motion V1: on the pacing's clock (play.rs `replay` lays the poses on it; a film's own while one runs)
        let now = sim.live.as_ref().map_or_else(std::time::Instant::now, |l| l.now());
        let (dur, reach, deg) = (sim.cfg.lunge_s.max(0.05), sim.cfg.lunge_m, sim.cfg.lunge_deg.to_radians());
        let at_of = |k: u64, people: &Vec<crate::people::BodyDraw>, sb: &Option<crate::people::BodyDraw>| {
            sb.as_ref().filter(|b| b.id == k).or_else(|| people.iter().find(|b| b.id == k)).map(|b| b.at)
        };
        for x in &poses {
            if now < x.at {
                continue;
            }
            let u = (now - x.at).as_secs_f32() / dur;
            if u >= 1.0 {
                continue;
            }
            let (Some(me), Some(them)) = (at_of(x.key, &people, &subject_body), at_of(x.toward, &people, &subject_body)) else { continue };
            let d = (them - me).truncate();
            if d.length_squared() < 1e-6 {
                continue;
            }
            let dir = d.normalize();
            // R4: a beat's two swings lunge both bodies at once: each closes at most half the room left over
            // `lunge_sep_m`, so the pair never meets through one another (the staged gap is 1 m, two lunges 0.9)
            let reach = reach.min(((d.length() - sim.cfg.lunge_sep_m) * 0.5).max(0.0));
            let k = (u * std::f32::consts::PI).sin();
            let b = if subject_body.as_ref().is_some_and(|b| b.id == x.key) {
                subject_body.as_mut()
            } else {
                people.iter_mut().find(|b| b.id == x.key)
            };
            let Some(b) = b else { continue };
            if b.lying {
                continue;
            }
            // Motion V2: and on the rig, a strike (the right arm at the mark, the spine in) or a stagger (`Posing`)
            if x.lunge {
                b.nudge = dir * reach * k;
                b.lean = deg * k;
                b.yaw = -dir.x.atan2(dir.y);
                b.pose.strike = k;
            } else {
                b.nudge = -dir * reach * 0.8 * k;
                b.lean = -deg * 0.8 * k;
                b.pose.stagger = k;
            }
        }
    }
    sim.people = people;
    sim.subject_body = subject_body;
    // `--watch-vehicle`: the camera on that vehicle (the subject still drives the ring)
    if watch_at.is_some() {
        subject_at = watch_at;
    }
    // C6b: a sign per unit on its host's facade by its door, on its storey; units sharing a door and a storey
    // spread along the wall. Lit while open, dim when shut, white the day it was founded.
    let units: Vec<Unit> = sim.frames[j].units.clone();
    let mut by_spot: HashMap<(i32, i32, u8), Vec<u64>> = HashMap::new();
    for un in &units {
        by_spot.entry((un.door[0] as i32, un.door[1] as i32, un.floor)).or_default().push(un.id);
    }
    let (mut signs, mut lit_n, mut fresh_n) = (0usize, 0usize, 0usize);
    let base = ACTS.len() + LOD_RGB.len() + VEH_RGB.len();
    for un in &units {
        let [x, y, w, _] = un.rect;
        // the wall the door is on: its outward normal in tiles (west, east, north, south)
        let (nx, ny) = if un.door[0] <= x {
            (-1.0, 0.0)
        } else if un.door[0] >= x + w - 1.0 {
            (1.0, 0.0)
        } else if un.door[1] <= y {
            (0.0, -1.0)
        } else {
            (0.0, 1.0)
        };
        let spot = &by_spot[&(un.door[0] as i32, un.door[1] as i32, un.floor)];
        let k = spot.iter().position(|&i| i == un.id).unwrap_or(0) as f32 - (spot.len() as f32 - 1.0) / 2.0;
        let along = 3.6 / sim.tile_m;
        // tiles: out of the wall by half a tile, along it by the unit's slot (tile y runs south)
        let t = [un.door[0] + nx * 0.55 - ny * k * along, un.door[1] + ny * 0.55 + nx * k * along];
        let p = sim.metres(t, un.id, false);
        let z = stream.ground_z(p.x, p.y).unwrap_or(0.0) + Sim::storey_z(f32::from(un.floor)) + if un.floor == 0 { 3.0 } else { 1.6 };
        let yaw = if nx != 0.0 { std::f32::consts::FRAC_PI_2 } else { 0.0 };
        // a placeholder keeps the material it spawned with: each state (lit, dim, founded today) is its own entity
        let (state, mat) = if un.fresh {
            fresh_n += 1;
            (4, sim.mats[base + 2 * SIGN_RGB.len()].clone())
        } else if un.lit {
            lit_n += 1;
            (2, sim.mats[base + un.hue as usize].clone())
        } else {
            (3, sim.mats[base + SIGN_RGB.len() + un.hue as usize].clone())
        };
        signs += 1;
        let mesh = sim.sign.clone();
        place(&mut commands, &mut sim, (state, un.id), mesh, mat, at_yaw(bv(p.x, p.y, z), yaw));
    }
    // hide what this frame does not hold
    let seen: std::collections::HashSet<(u8, u64)> = seen.into_iter().collect();
    for (k, e) in sim.ents.iter() {
        if !seen.contains(k)
            && let Ok((_, mut v)) = q.get_mut(*e)
        {
            v.set_if_neq(Visibility::Hidden);
        }
    }
    // C6b: a building subject (`--follow` a building): no body matched, the camera stands at the ring's centre
    if subject_at.is_none()
        && subject.is_some()
        && let Some(c) = sim.frames[j].centre
        && sim.frames[j].agents.iter().all(|a| Some(a.id) != subject)
    {
        let p = sim.metres(c, 0, false);
        subject_at = Some((Vec3::new(p.x, p.y, stream.ground_z(p.x, p.y).unwrap_or(0.0)), 0.0));
    }
    sim.subject_at = subject_at;
    sim.no_way = no_way;
    let stack = on_tile.values().copied().max().unwrap_or(0);
    let f = &sim.frames[j];
    let follow = if sim.live.is_some() { format!("\nfollowing {}  (R reload config)", sim.caption) } else { String::new() };
    // V1: the last config reload's line
    let follow = match sim.live.as_ref().map(|l| l.reload_line.lock().unwrap().clone()).filter(|l| !l.is_empty()) {
        Some(l) => format!("{follow}\n{l}"),
        None => follow,
    };
    let follow = if signs > 0 { format!("{follow}\nsigns {signs} (lit {lit_n}, founded today {fresh_n})") } else { follow };
    // V4: what the subject is doing (a climb, a ride, its storey's z), the riders drawn, the crowd spread
    let follow = if subj_line.is_empty() { follow } else { format!("{follow}\n{subj_line}") };
    let follow = format!(
        "{follow}\nriders drawn {riders_drawn}, bodies in a crowd's slots {spread_n}, jumps (no way) {no_way}, drawn jumps eased {} snapped {}",
        sim.motion.absorbed, sim.motion.snapped
    );
    let tick_now = f.tick;
    sim.hud = format!(
        "sim tick {} {}  agents {} (street {street}, most on a tile {stack}, indoors {indoors} ({hidden_in} hidden), aboard {aboard}, off a deck {deck_miss}; Full {} Coarse {} Stat {})  vehicles car {} van {} train {} flyer {} foot {} ({riders} aboard)  events {}",
        f.tick,
        f.clock,
        f.agents.len(),
        lods[0],
        lods[1],
        lods[2],
        by_ride[0],
        by_ride[1],
        by_ride[2],
        by_ride[3],
        by_ride[4],
        f.events
    ) + &follow;
    sim.clock_tick = Some(tick_now);
}

/// V10: the light follows the sim's clock (cam.rs `View::daylight`, `[view] light_*`); a config reload's `[view]`
/// reaches the camera here (the light's and the camera's numbers live).
pub fn clock_light(sim: Res<Sim>, mut view: ResMut<crate::cam::View>) {
    if let Some(c) = sim.live.as_ref().and_then(|l| l.view_cfg.lock().unwrap().take()) {
        view.cfg = crate::cam::CamCfg::from_view(&c);
    }
    // Motion V1: the play speed now, for the follow's ease (cam.rs `View::ease`)
    // (flat out the camera is on the subject at once: no ease left to catch up when paced play begins)
    let speed_x = sim.live.as_ref().map_or(1.0, |l| l.tick_rate() / l.rate().max(1e-9)).max(1.0);
    view.bypass_change_detection().speed_x = if sim.flat { 1e6 } else { speed_x };
    let tpd = citysim::TICKS_PER_DAY;
    let d = sim.clock_tick.map_or(0.0, |t| view.cfg.light.daylight((t % tpd) as f32 * 24.0 / tpd as f32));
    if (view.daylight - d).abs() > 1e-4 {
        view.daylight = d;
    }
}

/// V1 (OPEN-262): the R key asks the sim thread for a config reload at its next tick boundary.
pub fn reload_key(keys: Res<ButtonInput<KeyCode>>, sim: Res<Sim>) {
    if keys.just_pressed(KeyCode::KeyR)
        && let Some(live) = sim.live.as_ref()
    {
        live.reload_req.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// C2: the follow-cam: the player stand-in (the camera's and the occlusion's focus, occ.rs) stands where the subject
/// is drawn. Motion V1: after `update_sim` (this frame's place), moving the view to it too (`occ::player_controls`
/// ran on the last frame's, for the stream).
pub fn follow_subject(sim: Res<Sim>, mut player: ResMut<crate::occ::Player>, mut view: ResMut<crate::cam::View>) {
    if let Some((p, yaw)) = sim.subject_at {
        player.on = true;
        player.scripted = true;
        player.pos = p;
        if view.mode == crate::cam::Mode::Game && (view.at != p.truncate() || view.ground_z != p.z) {
            view.at = p.truncate();
            view.ground_z = p.z;
        }
        player.facing = yaw;
        player.indoors = sim.subject_indoors;
        // C5: the subject's pose (a lunge, a stagger, lying knocked out)
        match sim.subject_body.as_ref() {
            Some(b) => {
                // Motion V2: the stand-in is rigged (its lean is in its spine), the capsule is not
                player.tilt = if sim.capsules { b.tilt() } else { b.rig_tilt() };
                player.nudge = b.feet() - b.at;
                player.facing = b.yaw;
            }
            None => {
                player.tilt = Quat::IDENTITY;
                player.nudge = Vec3::ZERO;
            }
        }
    } else {
        player.indoors = None;
    }
}
