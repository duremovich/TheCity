//! Chunk streaming (CITY_SPEC §PERF, P3_PLAN C): the wanted set = chunks whose box (their tiles x the index's
//! z range: ground .. the highest roof) projects into the game view (+ a margin), or every chunk in the overview
//! at the coarse ground LOD. Loading = file read + meshing on the async compute pool (never the main thread);
//! the main thread inserts at most a budget of finished chunks per frame; chunks well outside the view unload.

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
    window::PrimaryWindow,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::cam::{CityCam, Mode, View};
use crate::data::*;
use crate::mesh;
use crate::windows::{GroundMaterial, WindowMaterial};

/// Ground LOD by view: 0 = every tile, 1 = 4 x 4 tile cells (overview, wide zooms).
pub const LOD_STEP: [i32; 2] = [1, 4];
const MAX_TASKS: usize = 24;
/// Finished chunks inserted per frame at most, and a triangle budget per frame (mesh uploads)
const INSERT_MAX: usize = 6;
const INSERT_TRIS: usize = 250_000;

type Key = (i32, i32);

pub struct Built {
    pub lod: u8,
    /// The massing was built (a first load); on a LOD change only the ground is (/code-review s22: Tab re-meshed the
    /// island's massing)
    pub with_mass: bool,
    pub ground: Option<(Mesh, usize)>,
    pub mass: Option<(Mesh, usize)>,
    pub g: Option<Arc<Ground>>,
    /// The massing boxes (world, Bevy min / max): the occlusion rays' occluders
    pub boxes: Option<Arc<Vec<(Vec3, Vec3)>>>,
    /// The placed lots' massing (the kit stand-in) and its boxes
    pub standin: Option<(Mesh, usize)>,
    pub gen_boxes: Option<Arc<Vec<(Vec3, Vec3)>>>,
    pub ms: f32,
}

pub struct Loaded {
    pub lod: u8,
    pub ground: Option<(Entity, usize)>,
    pub mass: Option<(Entity, usize)>,
    pub standin: Option<(Entity, usize)>,
}

impl Loaded {
    pub fn tris(&self) -> usize {
        self.ground.map_or(0, |g| g.1) + self.mass.map_or(0, |m| m.1) + self.standin.map_or(0, |m| m.1)
    }
}

/// A chunk's placed-lot massing: the stand-in for its kit buildings (shown until the kit stream has the chunk).
#[derive(Component)]
pub struct GenMassing(pub Key);

/// The placed lots' massing shows while their chunk's kits are not in (a streamed chunk out of the kit radius,
/// or loading); without a kit stream every kit is in (P6: all at startup) and it never shows. The M key hides it
/// with the rest of the massing.
pub fn gen_massing_visibility(view: Res<View>, ks: Option<Res<glclient::kit::KitStream>>, mut q: Query<(&GenMassing, &mut Visibility)>) {
    for (g, mut v) in q.iter_mut() {
        let show = !view.hide_mass && ks.as_ref().is_some_and(|k| !k.chunk_ready(g.0));
        v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// A chunk's massing mesh (the M key / a shot can hide them all: a debug view of the ground and the pinned block,
/// not occlusion, which is P4).
#[derive(Component)]
pub struct Massing;

pub fn massing_visibility(view: Res<View>, mut q: Query<&mut Visibility, With<Massing>>) {
    if !view.is_changed() {
        return;
    }
    for mut v in q.iter_mut() {
        v.set_if_neq(if view.hide_mass { Visibility::Hidden } else { Visibility::Inherited });
    }
}

#[derive(Default, Clone, Debug, serde::Serialize)]
pub struct StreamStats {
    pub loaded: usize,
    pub pending: usize,
    pub wanted: usize,
    pub tris: usize,
    pub built: usize,
    pub build_ms_sum: f64,
    pub build_ms_max: f32,
    /// Main-thread time spent inserting this frame (ms)
    pub insert_ms: f32,
    pub inserted: usize,
    pub unloaded: usize,
}

#[derive(Resource)]
pub struct Stream {
    pub dir: PathBuf,
    pub index: Index,
    pub recs: HashMap<Key, ChunkRec>,
    pub loaded: HashMap<Key, Loaded>,
    pub tasks: HashMap<Key, (u8, Task<Built>)>,
    pub grounds: HashMap<Key, Arc<Ground>>,
    /// Loaded chunks' massing boxes (world, Bevy min / max) and their bounds
    pub boxes: HashMap<Key, (Vec3, Vec3, Arc<Vec<(Vec3, Vec3)>>)>,
    /// Loaded chunks' placed-lot boxes (occluders while the chunk's kits are not in)
    pub gen_boxes: HashMap<Key, (Vec3, Vec3, Arc<Vec<(Vec3, Vec3)>>)>,
    pub ground_mat: Handle<GroundMaterial>,
    pub window_mat: Handle<WindowMaterial>,
    pub stats: StreamStats,
    /// Wanted keys this frame (for the bench's "idle" test)
    pub wanted: HashMap<Key, u8>,
}

impl Stream {
    pub fn new(dir: PathBuf, ground_mat: Handle<GroundMaterial>, window_mat: Handle<WindowMaterial>) -> Stream {
        let index = Index::load(&dir);
        let recs = index.chunks.iter().map(|r| ((r.c[0], r.c[1]), r.clone())).collect();
        Stream {
            dir,
            index,
            recs,
            loaded: HashMap::new(),
            tasks: HashMap::new(),
            grounds: HashMap::new(),
            boxes: HashMap::new(),
            gen_boxes: HashMap::new(),
            ground_mat,
            window_mat,
            stats: StreamStats::default(),
            wanted: HashMap::new(),
        }
    }

    /// Everything wanted is loaded at its LOD and nothing is in flight.
    pub fn idle(&self) -> bool {
        self.tasks.is_empty() && self.wanted.iter().all(|(k, l)| self.loaded.get(k).is_some_and(|x| x.lod == *l))
    }

    pub fn ground_z(&self, x: f32, y: f32) -> Option<f32> {
        let cm = self.index.chunk_m;
        let key = ((x / cm).floor() as i32, (y / cm).floor() as i32);
        self.grounds.get(&key).and_then(|g| mesh::ground_z(g, self.index.tile, x, y))
    }
}

fn build(dir: PathBuf, rec: ChunkRec, lod: u8, with_mass: bool, tile: f32, apron: i32, chunk_m: f32) -> Built {
    let t0 = Instant::now();
    let (cx, cy) = (rec.c[0], rec.c[1]);
    let mut out = Built { lod, with_mass, ground: None, mass: None, g: None, boxes: None, standin: None, gen_boxes: None, ms: 0.0 };
    if rec.ground
        && let Some(g) = Ground::load(&dir.join(format!("{cx}_{cy}.ground.bin")), apron)
    {
        let mb = mesh::ground_mesh(&g, tile, LOD_STEP[lod as usize]);
        if !mb.is_empty() {
            let n = mb.tris();
            out.ground = Some((mb.into_mesh(), n));
        }
        out.g = Some(Arc::new(g));
    }
    if with_mass
        && rec.boxes > 0
        && let Some(m) = ChunkMass::load(&dir.join(format!("{cx}_{cy}.json")))
    {
        let origin = Vec2::new(cx as f32 * chunk_m, cy as f32 * chunk_m);
        out.boxes = Some(Arc::new(mesh::massing_boxes(&m.boxes)));
        let mb = mesh::massing_mesh(&m, &m.boxes, origin);
        if !mb.is_empty() {
            let n = mb.tris();
            out.mass = Some((mb.into_mesh(), n));
        }
        if !m.standin.is_empty() {
            out.gen_boxes = Some(Arc::new(mesh::massing_boxes(&m.standin)));
            let mb = mesh::massing_mesh(&m, &m.standin, origin);
            let n = mb.tris();
            out.standin = Some((mb.into_mesh(), n));
        }
    }
    out.ms = t0.elapsed().as_secs_f32() * 1000.0;
    out
}

/// Chunk keys + LOD the view wants; `keep` = the larger set loaded chunks may stay in (hysteresis).
fn wanted(s: &Stream, view: &View, cam: &Transform, aspect: f32) -> (HashMap<Key, u8>, HashMap<Key, u8>) {
    let mut want = HashMap::new();
    let mut keep = HashMap::new();
    match view.mode {
        Mode::Overview => {
            for k in s.recs.keys() {
                want.insert(*k, 1);
                keep.insert(*k, 1);
            }
        }
        Mode::Game => {
            // full ground detail up to the 400 m zoom (the 8 m cells read blocky there: review p3 r2)
            let lod = if view.zoom <= 450.0 { 0 } else { 1 };
            let (right, up, eye) = (cam.right().as_vec3(), cam.up().as_vec3(), cam.translation);
            let hw = view.zoom / 2.0;
            let hh = hw / aspect.max(0.1);
            let cm = s.index.chunk_m;
            for (k, r) in &s.recs {
                // the chunk's square, grown to its boxes' extent (a lot's boxes may overhang it: /code-review s22)
                let (mut x0, mut y0) = (k.0 as f32 * cm, k.1 as f32 * cm);
                let (mut x1, mut y1) = (x0 + cm, y0 + cm);
                if let Some(bb) = r.bb {
                    (x0, y0, x1, y1) = (x0.min(bb[0]), y0.min(bb[1]), x1.max(bb[2]), y1.max(bb[3]));
                }
                let (mut sx0, mut sx1, mut sy0, mut sy1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
                for &x in &[x0, x1] {
                    for &y in &[y0, y1] {
                        for &z in &[r.zr[0] - 2.0, r.zr[1] + 2.0] {
                            let p = mesh::bv(x, y, z) - eye;
                            let (a, b) = (p.dot(right), p.dot(up));
                            sx0 = sx0.min(a);
                            sx1 = sx1.max(a);
                            sy0 = sy0.min(b);
                            sy1 = sy1.max(b);
                        }
                    }
                }
                let hit = |m: f32, pad: f32| sx1 >= -hw * m - pad && sx0 <= hw * m + pad && sy1 >= -hh * m - pad && sy0 <= hh * m + pad;
                if hit(1.15, 16.0) {
                    want.insert(*k, lod);
                }
                if hit(1.6, 64.0) {
                    keep.insert(*k, lod);
                }
            }
        }
    }
    (want, keep)
}

#[allow(clippy::too_many_arguments)]
pub fn update_stream(
    mut commands: Commands,
    mut s: ResMut<Stream>,
    mut view: ResMut<View>,
    cam: Query<&Transform, With<CityCam>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time>,
) {
    let Ok(cam_t) = cam.single() else { return };
    // CV_EMPTY=1: stream nothing (bench isolation: the sea + layers only)
    if std::env::var("CV_EMPTY").is_ok() {
        return;
    }
    let aspect = window.single().map(|w| w.width() / w.height().max(1.0)).unwrap_or(16.0 / 9.0);
    let (want, keep) = wanted(&s, &view, cam_t, aspect);

    // unload: outside the keep set; stale tasks dropped (dropping a Task cancels it)
    let gone: Vec<Key> = s.loaded.keys().filter(|k| !keep.contains_key(k)).copied().collect();
    for k in &gone {
        if let Some(l) = s.loaded.remove(k) {
            for (e, _) in l.ground.into_iter().chain(l.mass).chain(l.standin) {
                commands.entity(e).despawn();
            }
            s.stats.unloaded += 1;
        }
        s.grounds.remove(k);
        s.boxes.remove(k);
        s.gen_boxes.remove(k);
    }
    s.tasks.retain(|k, (lod, _)| want.get(k) == Some(lod));

    // start builds, nearest the view centre first
    let focus = match view.mode {
        Mode::Game => view.at,
        Mode::Overview => view.orbit_at.truncate(),
    };
    let cm = s.index.chunk_m;
    let mut todo: Vec<(f32, Key, u8)> = want
        .iter()
        .filter(|(k, l)| s.loaded.get(*k).is_none_or(|x| x.lod != **l) && !s.tasks.contains_key(*k))
        .map(|(k, l)| {
            let c = Vec2::new((k.0 as f32 + 0.5) * cm, (k.1 as f32 + 0.5) * cm);
            (c.distance_squared(focus), *k, *l)
        })
        .collect();
    todo.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let pool = AsyncComputeTaskPool::get();
    for (_, k, lod) in todo.into_iter().take(MAX_TASKS.saturating_sub(s.tasks.len())) {
        let (dir, rec) = (s.dir.clone(), s.recs[&k].clone());
        let (tile, apron, chunk_m) = (s.index.tile, s.index.apron, s.index.chunk_m);
        let with_mass = !s.loaded.contains_key(&k);
        s.tasks.insert(k, (lod, pool.spawn(async move { build(dir, rec, lod, with_mass, tile, apron, chunk_m) })));
    }

    // insert finished builds within the per-frame budget
    let t0 = Instant::now();
    let mut done = vec![];
    for (k, (_, task)) in s.tasks.iter_mut() {
        if let Some(b) = check_ready(task) {
            done.push((*k, b));
        }
    }
    done.sort_by_key(|(k, _)| *k);
    let (mut n_ins, mut tris) = (0, 0);
    for (k, b) in done {
        s.tasks.remove(&k);
        if n_ins >= INSERT_MAX || tris >= INSERT_TRIS {
            // over budget: keep the result for the next frame
            s.tasks.insert(k, (b.lod, pool.spawn(async move { b })));
            continue;
        }
        let origin = mesh::bv(k.0 as f32 * cm, k.1 as f32 * cm, 0.0);
        let ground = b.ground.map(|(m, n)| {
            let e = commands
                .spawn((
                    Mesh3d(meshes.add(m)),
                    MeshMaterial3d(s.ground_mat.clone()),
                    Transform::from_translation(origin),
                    Name::new(format!("ground_{}_{}", k.0, k.1)),
                ))
                .id();
            (e, n)
        });
        let built_mass = b.mass.map(|(m, n)| {
            let vis = if view.hide_mass { Visibility::Hidden } else { Visibility::Inherited };
            let e = commands
                .spawn((
                    Mesh3d(meshes.add(m)),
                    MeshMaterial3d(s.window_mat.clone()),
                    Transform::from_translation(origin),
                    Name::new(format!("mass_{}_{}", k.0, k.1)),
                    Massing,
                    vis,
                ))
                .id();
            (e, n)
        });
        // hidden until gen_massing_visibility runs (a kit-ready chunk must not flash its stand-in)
        let built_gen = b.standin.map(|(m, n)| {
            let e = commands
                .spawn((
                    Mesh3d(meshes.add(m)),
                    MeshMaterial3d(s.window_mat.clone()),
                    Transform::from_translation(origin),
                    Name::new(format!("gen_{}_{}", k.0, k.1)),
                    GenMassing(k),
                    Visibility::Hidden,
                ))
                .id();
            (e, n)
        });
        let old = s.loaded.remove(&k);
        let (mut mass, mut standin) = (built_mass, built_gen);
        if let Some(o) = old {
            if let Some((e, _)) = o.ground {
                commands.entity(e).despawn();
            }
            // a LOD change keeps the massing it did not rebuild
            if b.with_mass {
                for (e, _) in o.mass.into_iter().chain(o.standin) {
                    commands.entity(e).despawn();
                }
            } else {
                mass = o.mass;
                standin = o.standin;
            }
        }
        let t = ground.map_or(0, |g| g.1) + built_mass.map_or(0, |m| m.1) + built_gen.map_or(0, |m| m.1);
        s.loaded.insert(k, Loaded { lod: b.lod, ground, mass, standin });
        if let Some(g) = b.g {
            s.grounds.insert(k, g);
        }
        if let Some(bx) = b.boxes {
            let lo = bx.iter().fold(Vec3::MAX, |a, b| a.min(b.0));
            let hi = bx.iter().fold(Vec3::MIN, |a, b| a.max(b.1));
            s.boxes.insert(k, (lo, hi, bx));
        }
        if let Some(bx) = b.gen_boxes {
            let lo = bx.iter().fold(Vec3::MAX, |a, b| a.min(b.0));
            let hi = bx.iter().fold(Vec3::MIN, |a, b| a.max(b.1));
            s.gen_boxes.insert(k, (lo, hi, bx));
        }
        s.stats.built += 1;
        s.stats.build_ms_sum += b.ms as f64;
        s.stats.build_ms_max = s.stats.build_ms_max.max(b.ms);
        n_ins += 1;
        tris += t;
    }
    s.stats.insert_ms = t0.elapsed().as_secs_f32() * 1000.0;
    s.stats.inserted = n_ins;
    s.stats.loaded = s.loaded.len();
    s.stats.pending = s.tasks.len();
    s.stats.wanted = want.len();
    s.stats.tris = s.loaded.values().map(|l| l.tris()).sum();
    s.wanted = want;

    // the game camera follows the ground under the view centre
    // (the player's own z drives it in player mode: decks, buildings)
    if view.mode == Mode::Game
        && !view.player
        && let Some(z) = s.ground_z(view.at.x, view.at.y)
        && z != view.ground_z
    {
        // ease, then snap the last 5 cm: stopping inside a dead band left the camera's resting height to frame
        // timing (shots of one view differed by ~2 px between runs)
        if (z - view.ground_z).abs() > 0.05 {
            let k = (time.delta_secs() * 6.0).min(1.0);
            view.ground_z += (z - view.ground_z) * k;
        } else {
            view.ground_z = z;
        }
    }
}
