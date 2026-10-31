//! Sim frames on the city (spec/integration/SIM_VIEW_CONTRACT.md, the `city-simview/0.1` file form): `--sim FILE`
//! reads the converter's frames (engine/simsnap) and draws placeholders on the real city: a capsule per agent out
//! on the street, a box per vehicle in motion and per El train, moved between frames at `--sim-rate` ticks a second
//! (default 0.7: one tick ~1.4 s, the sim's 2-tick, 4 m step at a 1.4 m/s walk; 0 holds `--sim-frame`).
//!
//! Colours (CV_SIM_COLOUR=lod switches): by action, walk green, use amber, idle slate, wait violet, ride / aboard
//! cyan, timed (a Coarse jump) magenta, fly white; by LOD, Full green, Coarse amber, Statistical red. Agents inside a
//! building are not drawn (the sim has no floor for them: the contract's missing `floor`); the HUD counts them.
//! Positions are tile centres plus a per-id jitter inside the 4 m tile, on the streamed ground; a train sits on the
//! rail deck (layers.json), the tram and the lines with no track here on the street (OPEN-158: drawn red, raised).

use crate::data::Layers;
use crate::mesh::bv;
use crate::stream::Stream;
use bevy::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy)]
pub struct Agent {
    pub id: u64,
    pub t: [f32; 2],
    pub indoors: bool,
    pub lod: u8,
    pub act: u8,
    pub aboard: bool,
    pub facing: Option<f32>,
}

#[derive(Clone, Copy)]
pub struct Vehicle {
    pub id: u64,
    pub t: [f32; 2],
    /// 0 car / cab / exec, 1 van, 2 train, 3 flyer, 4 foot escort
    pub ride: u8,
    pub line: Option<u8>,
    pub riders: u16,
}

pub struct SimFrame {
    pub tick: u64,
    pub clock: String,
    pub agents: Vec<Agent>,
    pub vehicles: Vec<Vehicle>,
    pub events: usize,
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
    pub t: f32,
    pub rate: f32,
    pub by_lod: bool,
    pub ents: HashMap<(u8, u64), Entity>,
    pub mats: Vec<Handle<StandardMaterial>>,
    pub capsule: Handle<Mesh>,
    pub boxes: [Handle<Mesh>; 5],
    pub hud: String,
}

const ACTS: [&str; 9] = ["walk", "use", "idle", "wait", "ride", "timed", "fly", "jacked_in", "child"];
const ACT_RGB: [[f32; 3]; 9] = [
    [0.25, 1.0, 0.35],
    [1.0, 0.7, 0.15],
    [0.45, 0.55, 0.7],
    [0.7, 0.4, 1.0],
    [0.2, 0.9, 1.0],
    [1.0, 0.2, 0.9],
    [1.0, 1.0, 1.0],
    [0.3, 0.3, 1.0],
    [0.9, 0.9, 0.5],
];
const LOD_RGB: [[f32; 3]; 3] = [[0.25, 1.0, 0.35], [1.0, 0.7, 0.15], [1.0, 0.15, 0.1]];
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
        let line_track = d["lines"]
            .as_array()
            .map(|ls| {
                ls.iter()
                    .map(|l| {
                        let n = l["name"].as_str().unwrap_or("");
                        if n == "Terrace Line" {
                            TRACK_RAIL
                        } else if n.starts_with("Tram") {
                            TRACK_STREET
                        } else {
                            TRACK_NONE
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let rail = Layers::load(chunk_dir).elements.into_iter().filter(|e| e.kind == "rail").flat_map(|e| e.pts).collect();
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
                        indoors: !a["b"].is_null(),
                        lod: match a["lod"].as_str() {
                            Some("Full") => 0,
                            Some("Coarse") => 1,
                            _ => 2,
                        },
                        act: ACTS.iter().position(|s| *s == act).unwrap_or(2) as u8,
                        aboard: !a["aboard"].is_null(),
                        facing: a["facing"].as_f64().map(|h| h as f32),
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
                    ride: match v["ride"].as_str() {
                        Some("Van") => 1,
                        Some("Train") => 2,
                        Some("Flyer") => 3,
                        Some("Foot") => 4,
                        _ => 0,
                    },
                    line: v["route"]["line"].as_u64().map(|l| l as u8),
                    riders: v["riders"].as_array().map_or(0, |r| r.len() as u16),
                })
                .collect();
            frames.push(SimFrame {
                tick: f["tick"].as_u64().unwrap(),
                clock: f["clock"].as_str().unwrap_or("").into(),
                agents,
                vehicles,
                events: f["events"].as_array().map_or(0, |e| e.len()),
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
            rail,
            rate,
            by_lod: std::env::var("CV_SIM_COLOUR").is_ok_and(|v| v == "lod"),
            ents: HashMap::new(),
            mats: vec![],
            capsule: Handle::default(),
            boxes: Default::default(),
            hud: String::new(),
        }
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

    fn rail_z(&self, p: Vec2) -> Option<f32> {
        self.rail
            .iter()
            .map(|q| ((Vec2::new(q[0], q[1]) - p).length_squared(), q[2]))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .filter(|(d2, _)| *d2 < 400.0)
            .map(|(_, z)| z)
    }
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
    sim.mats = m;
}

/// Advance the playback and place every placeholder: frame i and i + 1 lerped for the ids in both.
pub fn update_sim(
    mut commands: Commands,
    time: Res<Time>,
    stream: Res<Stream>,
    mut sim: ResMut<Sim>,
    mut q: Query<(&mut Transform, &mut Visibility)>,
) {
    let n = sim.frames.len();
    let rate = sim.rate;
    sim.t = (sim.t + time.delta_secs() * rate) % n as f32;
    let i = sim.t.floor() as usize;
    let j = (i + 1).min(n - 1);
    let u = if rate == 0.0 { 0.0 } else { sim.t - i as f32 };
    let next_a: HashMap<u64, [f32; 2]> = sim.frames[j].agents.iter().map(|a| (a.id, a.t)).collect();
    let next_v: HashMap<u64, [f32; 2]> = sim.frames[j].vehicles.iter().map(|v| (v.id, v.t)).collect();
    let mut seen: Vec<(u8, u64)> = vec![];
    let mut place =
        |commands: &mut Commands, sim: &mut Sim, k: (u8, u64), mesh: Handle<Mesh>, mat: Handle<StandardMaterial>, at: Vec3, yaw: f32| {
            seen.push(k);
            let tf = Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw));
            match sim.ents.get(&k).copied() {
                Some(e) => {
                    if let Ok((mut t, mut v)) = q.get_mut(e) {
                        *t = tf;
                        v.set_if_neq(Visibility::Inherited);
                    }
                }
                None => {
                    let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), tf, Name::new("sim"))).id();
                    sim.ents.insert(k, e);
                }
            }
        };
    let (mut street, mut indoors, mut aboard, mut lods) = (0, 0, 0, [0usize; 3]);
    let fr_agents = sim.frames[i].agents.clone();
    for a in &fr_agents {
        lods[a.lod as usize] += 1;
        if a.indoors {
            indoors += 1;
            continue;
        }
        if a.aboard {
            // drawn inside its vehicle
            aboard += 1;
            continue;
        }
        street += 1;
        let t = next_a.get(&a.id).map_or(a.t, |n| [a.t[0] + (n[0] - a.t[0]) * u, a.t[1] + (n[1] - a.t[1]) * u]);
        let p = sim.metres(t, a.id, true);
        let z = stream.ground_z(p.x, p.y).unwrap_or(0.0);
        let mat = if sim.by_lod { sim.mats[ACTS.len() + a.lod as usize].clone() } else { sim.mats[a.act as usize].clone() };
        // compass degrees (clockwise from north) -> Bevy yaw about +y (heading 0 = -z)
        let yaw = a.facing.map_or(0.0, |h| -h.to_radians());
        let cap = sim.capsule.clone();
        place(&mut commands, &mut sim, (0, a.id), cap, mat, bv(p.x, p.y, z + 0.85 * 1.4), yaw);
    }
    let fr_veh: Vec<Vehicle> = sim.frames[i].vehicles.clone();
    let (mut by_ride, mut riders) = ([0usize; 5], 0usize);
    for v in &fr_veh {
        by_ride[v.ride as usize] += 1;
        riders += usize::from(v.riders);
        let t = next_v.get(&v.id).map_or(v.t, |n| [v.t[0] + (n[0] - v.t[0]) * u, v.t[1] + (n[1] - v.t[1]) * u]);
        let p = sim.metres(t, v.id, false);
        let ahead = next_v.get(&v.id).map(|n| sim.metres(*n, v.id, false) - sim.metres(v.t, v.id, false)).filter(|d| d.length() > 0.01);
        let yaw = ahead.map_or(0.0, |d| -d.x.atan2(d.y));
        let track = v.line.and_then(|l| sim.line_track.get(l as usize).copied());
        let ground = stream.ground_z(p.x, p.y).unwrap_or(0.0);
        let (z, red) = match (v.ride, track) {
            (2, Some(TRACK_RAIL)) => (sim.rail_z(p).map_or(ground + 12.0, |z| z + 2.6), false),
            (2, Some(TRACK_STREET)) => (ground + 1.8, false),
            // a line with no track here (OPEN-158): raised and red
            (2, _) => (ground + 10.0, true),
            _ => (ground + 0.9, false),
        };
        let mat = sim.mats[ACTS.len() + LOD_RGB.len() + usize::from(red)].clone();
        let mesh = sim.boxes[v.ride as usize].clone();
        place(&mut commands, &mut sim, (1, v.id), mesh, mat, bv(p.x, p.y, z), yaw);
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
    let f = &sim.frames[i];
    sim.hud = format!(
        "sim tick {} {}  agents {} (street {street}, indoors {indoors}, aboard {aboard}; Full {} Coarse {} Stat {})  vehicles car {} van {} train {} flyer {} foot {} ({riders} aboard)  events {}",
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
    );
}
