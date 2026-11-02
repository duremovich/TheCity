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

/// The place `u` (0..1) of the way from `from` to `to`: along `ahead` when it reaches `to` (the street's corners),
/// else straight.
fn along(from: [f32; 2], ahead: &[[f32; 2]], to: [f32; 2], u: f32) -> [f32; 2] {
    let lerp = |a: [f32; 2], b: [f32; 2], k: f32| [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
    let Some(end) = ahead.iter().position(|p| *p == to) else { return lerp(from, to, u) };
    let pts: Vec<[f32; 2]> = std::iter::once(from).chain(ahead[..=end].iter().copied()).collect();
    let len = |a: [f32; 2], b: [f32; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let total: f32 = pts.windows(2).map(|w| len(w[0], w[1])).sum();
    if total <= 0.0 {
        return to;
    }
    let mut left = total * u.clamp(0.0, 1.0);
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
            }
            let older = prev.as_ref().unwrap_or(&latest);
            self.frames = vec![SimFrame::from_view(&older.frame), SimFrame::from_view(&latest.frame)];
            self.live_seq = latest.seq;
            self.subject = Some(ekey(latest.subject));
            self.caption = latest.caption.clone();
        }
        self.rate = live.rate();
        let held = live.hold.load(std::sync::atomic::Ordering::Relaxed) != u64::MAX;
        Some(if held { 1.0 } else { (latest.at.elapsed().as_secs_f32() * self.rate).clamp(0.0, 1.0) })
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

    /// V4: a body standing on a tile shared with `n - 1` others: a place fixed per id in a disc round the tile's
    /// centre (`[view] crowd_gap_m` apart, at most `crowd_spread_m` out) on walkable ground at the centre's level
    /// (sidewalk, ped, open, stairs; not a carriageway, a lot's platform or a wall), the disc widening on a miss;
    /// `None`: no such place found.
    fn spread(&self, stream: &Stream, c: Vec2, id: u64, n: usize) -> Option<Vec2> {
        let gz = stream.ground_z(c.x, c.y)?;
        // a disc about 2.5 times the crowd's own area (some of it is wall or road), growing on each miss
        let r0 = self.cfg.crowd_gap_m * (n as f32 * 2.5 / std::f32::consts::PI).sqrt();
        let cap = self.cfg.crowd_spread_m.max(self.tile_m * 0.5);
        for k in 0..16u64 {
            let r = (r0 * (1.0 + k as f32 / 4.0)).clamp(self.tile_m * 0.5, cap);
            let h = mix64(id ^ k.wrapping_mul(0xA24B_AED4_963E_E407));
            let ang = (h & 0xFFFF) as f32 / 65536.0 * std::f32::consts::TAU;
            let rad = r * (((h >> 16) & 0xFFFF) as f32 / 65535.0).sqrt();
            let q = c + Vec2::new(ang.cos(), ang.sin()) * rad;
            let walkable = matches!(
                stream.ground_class(q.x, q.y),
                Some(crate::data::SIDEWALK | crate::data::PED | crate::data::OPEN | crate::data::STAIRS)
            );
            if walkable && stream.ground_z(q.x, q.y).is_some_and(|z| (z - gz).abs() < 1.0) {
                return Some(q);
            }
        }
        None
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
    let still = |a: &Agent| prev_a.get(&a.id).is_some_and(|(p, _)| *p == a.t);
    // V4: the bodies standing on each ground-level street tile: a crowd on one tile spreads out (`Sim::spread`)
    let mut standing: HashMap<(i32, i32), usize> = HashMap::new();
    for a in fr_agents.iter().filter(|a| !a.indoors && a.aboard.is_none() && a.floor == 0.0 && still(a)) {
        *standing.entry((a.t[0] as i32, a.t[1] as i32)).or_default() += 1;
    }
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
    // V4: the subject's lift car (its place, z, lot)
    let mut cab: Option<(Vec2, f32, Option<crate::lots::Placed>)> = None;
    let (mut hidden_in, mut deck_miss, mut spread_n) = (0usize, 0usize, 0usize);
    for a in &fr_agents {
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
        // a lift rider stands in the car; a stairs climber walks
        let lift = a.climb.is_some_and(|c| c.2);
        let walking = matches!(a.act, 0 | 5) || (a.act == 9 && !lift) || prev_a.get(&a.id).is_some_and(|(p, _)| *p != a.t);
        let draw = |at: Vec3, yaw: f32| crate::people::BodyDraw {
            id: a.id,
            at,
            yaw,
            walking,
            female: a.female,
            age: a.age,
            dress: a.dress,
            gang: a.gang,
        };
        let is_subject = Some(a.id) == subject;
        if is_subject {
            subject_in = a.aboard;
        }
        if a.indoors {
            // C6a: on its floor, seen only in the cut (above)
            indoors += 1;
            let f = floor_of(a);
            let (mut p, z, mut yaw) = match (&subj, is_subject) {
                (Some(s), true) => (s.2, s.3, yaw),
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
                subject_at = Some((Vec3::new(p.x, p.y, z), yaw));
                subject_body = Some(draw(Vec3::new(p.x, p.y, z), yaw));
            }
            if sim.capsules {
                let mat = if sim.by_lod { sim.mats[ACTS.len() + a.lod as usize].clone() } else { sim.mats[a.act as usize].clone() };
                let cap = sim.capsule.clone();
                place(&mut commands, &mut sim, (0, a.id), cap, mat, at_yaw(bv(p.x, p.y, z + 0.85 * 1.4), 0.0));
            } else if !is_subject {
                people.push(draw(Vec3::new(p.x, p.y, z), yaw));
            }
            continue;
        }
        if a.aboard.is_some() {
            // drawn inside its vehicle (below)
            aboard += 1;
            continue;
        }
        street += 1;
        *on_tile.entry((a.t[0] as i32, a.t[1] as i32)).or_default() += 1;
        let t = prev_a.get(&a.id).map_or(a.t, |(p, ah)| along(*p, ah, a.t, u));
        let mut p = sim.metres(t, a.id, true);
        // V4: a crowd standing on one tile spreads over the walkable ground round it, a place fixed per id
        let n = standing.get(&(a.t[0] as i32, a.t[1] as i32)).copied().unwrap_or(0);
        if n >= 2
            && a.floor == 0.0
            && still(a)
            && let Some(sp) = sim.spread(&stream, sim.metres(a.t, a.id, false), a.id, n)
        {
            p = sp;
            spread_n += 1;
        }
        let g = stream.ground_z(p.x, p.y).unwrap_or(0.0);
        // C7: a body on a walkway deck stands on it (no deck point within 12 m: at the storey's height, counted)
        let deck = if a.floor > 0.0 { sim.deck_z(p) } else { None };
        deck_miss += usize::from(a.floor > 0.0 && deck.is_none());
        let z = if a.floor > 0.0 { deck.unwrap_or(g + Sim::storey_z(a.floor)) } else { g };
        if is_subject {
            subject_at = Some((Vec3::new(p.x, p.y, z), yaw));
            subject_body = Some(draw(Vec3::new(p.x, p.y, z), yaw));
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
            people.push(draw(Vec3::new(p.x, p.y, z), yaw));
        }
    }
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
    for v in &fr_veh {
        by_ride[v.ride as usize] += 1;
        riders += usize::from(v.riders);
        let t = prev_v.get(&v.id).map_or(v.t, |(q, ah)| along(*q, ah, v.t, u));
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
                    female: look.map_or(h & 1 == 0, |a| a.female),
                    age: look.map_or(30, |a| a.age),
                    dress: look.map_or(1, |a| a.dress),
                    gang: look.is_some_and(|a| a.gang),
                };
                riders_drawn += 1;
                if Some(id) == subject {
                    subject_at = Some((pos.extend(z), yaw_b));
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
    let follow = format!("{follow}\nriders drawn {riders_drawn}, bodies spread off a shared tile {spread_n}");
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
/// is drawn. Before `occ::player_controls`, which moves the view to it.
pub fn follow_subject(sim: Res<Sim>, mut player: ResMut<crate::occ::Player>) {
    if let Some((p, yaw)) = sim.subject_at {
        player.on = true;
        player.scripted = true;
        player.pos = p;
        player.facing = yaw;
        player.indoors = sim.subject_indoors;
    } else {
        player.indoors = None;
    }
}
