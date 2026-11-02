//! The view milestone V7 (`OPEN-260`): the junctions' signals at the marked crossings around the subject. The zebra
//! paint is glclient's street layer (out/street/, the same zebras the import made the map's `X` lines from); this
//! module stands a pole at each kerb of every crossing within `RANGE_M` of the subject, with a pedestrian head (the
//! red man over the green man, facing across the road) and a vehicle head (red, amber, green, facing the main road's
//! traffic). Both are lit from `citysim::signals` at the view's game time (the newer frame's tick less one plus the
//! lerp's `u`: the tick's steps happen inside it), the same pure function the sim's walkers wait on, and follow a
//! config reload (`Shared::signals`). The vehicle head is OPEN-275's hook drawn early: nothing drives under it yet.
//!
//! `graft`: a save written before V7 (its map has no crossings, its config no `[crossings]`) takes the map file's
//! carriageways and crossings (`[world] map`, the same 1,500 x 938 island) and the `--config` file's `[crossings]`,
//! and its walkers' graph is rebuilt, so the walkers in the ring use the crossings too.

use bevy::prelude::*;
use citysim::components::TilePos;
use citysim::config::CrossingsCfg;
use citysim::map::MapCrossing;
use glclient::frame::to_bevy;
use std::collections::HashMap;
use std::path::Path;

use crate::sim::Sim;
use crate::stream::Stream;

/// Crossings within this many metres of the subject get their poles.
const RANGE_M: f32 = 160.0;
/// The pole, the heads and the lamps, metres.
const POLE_H: f32 = 3.6;
const PED_Z: f32 = 2.3;
const VEH_Z: f32 = 3.0;
const LAMP: f32 = 0.22;
/// Amber: the main road's last seconds of green.
const AMBER_MS: u64 = 3000;

pub fn add(app: &mut App) {
    app.insert_resource(Lights::default())
        .add_systems(Startup, setup_lights)
        .add_systems(Update, update_lights.after(crate::sim::update_sim));
}

/// A save from before V7: the map file's carriageways and crossings, the config file's `[crossings]`, the walkers'
/// graph rebuilt. A no-op on a V7 save.
pub fn graft(world: &mut citysim::World, config: Option<&Path>) {
    let mut changed = false;
    if world.map.crossings.is_empty()
        && let citysim::map::MapSource::File(saved) = citysim::map::MapSource::from_config(&world.config)
    {
        // this checkout's map of the same name first (a save names the absolute path it was built from)
        let here = saved.file_name().map(|n| Path::new("assets/maps").join(n));
        let grafted = here.into_iter().chain([saved.clone()]).filter(|p| p.is_file()).find_map(|p| {
            let m = citysim::map::Map::load(&p);
            world.map.graft_crossings(&m).then(|| (p, m.crossings.len()))
        });
        match grafted {
            Some((p, n)) => {
                eprintln!("cityview: V7 graft: {n} crossings and the carriageways from {}", p.display());
                changed = true;
            }
            None => eprintln!("cityview: V7 graft: no map with crossings for {} (the save walks without)", saved.display()),
        }
    }
    if world.config.crossings.is_none()
        && let Some(p) = config
        && let Ok(text) = std::fs::read_to_string(p)
        && let Ok(c) = citysim::Config::from_toml(&text, &world.config.assets_dir)
        && c.crossings.is_some()
    {
        world.config.crossings = c.crossings;
        eprintln!("cityview: V7 graft: [crossings] from {}", p.display());
        changed = true;
    }
    if changed {
        citysim::systems::levels::init_portals(world);
    }
}

/// The subject's crossings this tick to the log (`World::crossing_watch`, drained).
pub fn log_crossings(world: &mut citysim::World, subject: citysim::EntityId) {
    let Some(v) = world.crossing_watch.as_mut().and_then(|w| w.get_mut(&subject)) else { return };
    for (tick, tile, j, wait) in v.drain(..) {
        if wait == u32::MAX {
            eprintln!("cityview: V7 tick {tick}: the subject jaywalks at ({}, {})", tile.x, tile.y);
        } else {
            eprintln!(
                "cityview: V7 tick {tick}: the subject crosses at ({}, {}) under junction ({}, {}), {} ms at the kerb",
                tile.x, tile.y, j.x, j.y, wait
            );
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lamp {
    RedMan,
    GreenMan,
    Red,
    Amber,
    Green,
}

#[derive(Resource, Default)]
pub struct Lights {
    crossings: Vec<MapCrossing>,
    /// crossing index -> (its entities, the lamps among them with their role and junction)
    spawned: HashMap<usize, (Vec<Entity>, Vec<(Entity, Lamp)>)>,
    pole: Handle<Mesh>,
    head: Handle<Mesh>,
    lamp: Handle<Mesh>,
    metal: Handle<StandardMaterial>,
    /// lit then dark, by `Lamp` order
    lit: Vec<Handle<StandardMaterial>>,
    dark: Vec<Handle<StandardMaterial>>,
    pub walk_now: usize,
    pub shown: usize,
}

fn glow(mats: &mut Assets<StandardMaterial>, c: [f32; 3], k: f32) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: Color::srgb(c[0] * 0.4, c[1] * 0.4, c[2] * 0.4),
        emissive: LinearRgba::rgb(c[0] * k, c[1] * k, c[2] * k),
        perceptual_roughness: 0.6,
        ..default()
    })
}

fn setup_lights(mut l: ResMut<Lights>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    l.pole = meshes.add(Cuboid::new(0.14, POLE_H, 0.14));
    l.head = meshes.add(Cuboid::new(0.36, 0.95, 0.36));
    // the lamp sticks out of the head on both faces (seen from either kerb or either way along the road)
    l.lamp = meshes.add(Cuboid::new(LAMP, LAMP, 0.44));
    l.metal = mats.add(StandardMaterial { base_color: Color::srgb(0.12, 0.13, 0.12), perceptual_roughness: 0.5, ..default() });
    let rgb = [[1.0, 0.12, 0.08], [0.2, 1.0, 0.35], [1.0, 0.1, 0.06], [1.0, 0.6, 0.05], [0.15, 1.0, 0.3]];
    l.lit = rgb.iter().map(|c| glow(&mut mats, *c, 40.0)).collect();
    l.dark = rgb.iter().map(|c| glow(&mut mats, [c[0] * 0.15, c[1] * 0.15, c[2] * 0.15], 0.0)).collect();
}

/// A crossing's two kerb poles, city metres: the pole and the axis its pedestrian head faces (walk_x: x).
fn poles(c: &MapCrossing, tm: f32, x0: f32, y1: f32) -> [Vec2; 2] {
    let (cx0, cx1) = (x0 + f32::from(c.a.x) * tm, x0 + f32::from(c.b.x + 1) * tm);
    let (cy0, cy1) = (y1 - f32::from(c.b.y + 1) * tm, y1 - f32::from(c.a.y) * tm);
    if c.walk_x {
        // kerbs west and east of the crossing, the poles off its south and north edges
        [Vec2::new(cx0 - 0.6, cy0 - 0.3), Vec2::new(cx1 + 0.6, cy1 + 0.3)]
    } else {
        [Vec2::new(cx0 - 0.3, cy1 + 0.6), Vec2::new(cx1 + 0.3, cy0 - 0.6)]
    }
}

/// The view's game time, ms: the newer frame's tick less one plus the lerp's `u` (1 while a bench holds).
fn game_ms(sim: &Sim) -> Option<u64> {
    let live = sim.live.as_ref()?;
    let (_, latest) = live.pair();
    let latest = latest?;
    let held = live.hold.load(std::sync::atomic::Ordering::Relaxed) != u64::MAX;
    let u = if held { 1.0 } else { (latest.at.elapsed().as_secs_f64() * f64::from(live.rate())).clamp(0.0, 1.0) };
    Some(((latest.frame.tick.saturating_sub(1) as f64 + u) * citysim::signals::MS_PER_TICK as f64) as u64)
}

fn lamp_lit(cfg: &CrossingsCfg, j: TilePos, t: u64, lamp: Lamp) -> bool {
    use citysim::signals as s;
    let walk = s::walk(cfg, j, t);
    let main_go = s::vehicle_go(cfg, j, true, t);
    let amber = main_go && s::left_ms(cfg, j, t) <= AMBER_MS;
    match lamp {
        Lamp::RedMan => !walk,
        Lamp::GreenMan => walk,
        Lamp::Red => !main_go,
        Lamp::Amber => amber,
        Lamp::Green => main_go && !amber,
    }
}

fn update_lights(
    mut commands: Commands,
    sim: Res<Sim>,
    stream: Res<Stream>,
    mut l: ResMut<Lights>,
    mut q: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    let Some(live) = sim.live.clone() else { return };
    if l.crossings.is_empty()
        && let Some(st) = live.stat.lock().unwrap().as_ref()
    {
        l.crossings = st.crossings.clone();
    }
    let cfg = live.signals.lock().unwrap().clone();
    let (Some(cfg), Some(t), Some((at, _))) = (cfg, game_ms(&sim), sim.subject_at) else { return };
    let (tm, x0, y1) = (sim.tile_m, sim.x0, sim.y1);
    let focus = Vec2::new(at.x, at.y);
    // spawn the near crossings' poles (on the streamed ground), despawn the far ones
    let near: Vec<usize> = l
        .crossings
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            let m = Vec2::new(x0 + (f32::from(c.a.x) + 0.5) * tm, y1 - (f32::from(c.a.y) + 0.5) * tm);
            m.distance(focus) < RANGE_M
        })
        .map(|(i, _)| i)
        .collect();
    let gone: Vec<usize> = l.spawned.keys().copied().filter(|k| !near.contains(k)).collect();
    for k in gone {
        if let Some((es, _)) = l.spawned.remove(&k) {
            for e in es {
                commands.entity(e).despawn();
            }
        }
    }
    for &k in &near {
        if l.spawned.contains_key(&k) {
            continue;
        }
        let c = l.crossings[k];
        let ps = poles(&c, tm, x0, y1);
        // the ground must be streamed in under both poles first
        let Some(zs) = ps.iter().map(|p| stream.ground_z(p.x, p.y)).collect::<Option<Vec<f32>>>() else { continue };
        let (mut es, mut lamps) = (vec![], vec![]);
        for (p, g) in ps.iter().zip(zs) {
            let base = Vec3::new(p.x, p.y, g);
            es.push(
                commands
                    .spawn((
                        Mesh3d(l.pole.clone()),
                        MeshMaterial3d(l.metal.clone()),
                        Transform::from_translation(to_bevy(base + Vec3::Z * POLE_H / 2.0)),
                    ))
                    .id(),
            );
            // the pedestrian head faces across the road (along the walk), the vehicle head along it
            let across = if c.walk_x { Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
            let along = if c.walk_x { Quat::IDENTITY } else { Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) };
            for (z, rot, set) in
                [(PED_Z, across, &[Lamp::RedMan, Lamp::GreenMan][..]), (VEH_Z + 0.35, along, &[Lamp::Red, Lamp::Amber, Lamp::Green][..])]
            {
                let side = if c.walk_x { Vec3::Y * 0.25 } else { Vec3::X * 0.25 };
                let hc = base + side + Vec3::Z * z;
                es.push(
                    commands
                        .spawn((
                            Mesh3d(l.head.clone()),
                            MeshMaterial3d(l.metal.clone()),
                            Transform::from_translation(to_bevy(hc)).with_rotation(rot),
                        ))
                        .id(),
                );
                let n = set.len() as f32;
                for (i, &lamp) in set.iter().enumerate() {
                    let dz = (n - 1.0) / 2.0 * 0.28 - i as f32 * 0.28;
                    let e = commands
                        .spawn((
                            Mesh3d(l.lamp.clone()),
                            MeshMaterial3d(l.dark[lamp as usize].clone()),
                            Transform::from_translation(to_bevy(hc + Vec3::Z * dz)).with_rotation(rot),
                        ))
                        .id();
                    es.push(e);
                    lamps.push((e, lamp));
                }
            }
        }
        l.spawned.insert(k, (es, lamps));
    }
    // light them from the signal function at the view's time
    let mut walk_now = 0;
    for (&k, (_, lamps)) in &l.spawned {
        let j = l.crossings[k].junction;
        walk_now += usize::from(citysim::signals::walk(&cfg, j, t));
        for &(e, lamp) in lamps {
            if let Ok(mut m) = q.get_mut(e) {
                let want = if lamp_lit(&cfg, j, t, lamp) { &l.lit[lamp as usize] } else { &l.dark[lamp as usize] };
                if m.0 != *want {
                    m.0 = want.clone();
                }
            }
        }
    }
    if l.spawned.len() != l.shown {
        eprintln!(
            "cityview: V7 lights: {} crossings' poles within {RANGE_M} m of the subject, {walk_now} on the green man at {:.1} min",
            l.spawned.len(),
            t as f64 / 60_000.0
        );
    }
    l.shown = l.spawned.len();
    l.walk_now = walk_now;
}
