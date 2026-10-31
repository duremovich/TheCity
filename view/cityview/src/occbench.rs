//! The occlusion bench (CITY_SPEC §CAM acceptance, spec/city/P4_PLAN.md C): the player walks the scripted paths
//! of `out/chunks/cam_paths.json` (pipeline/city/cam_paths.py) at 1.4 m/s with the game camera following, then
//! stands on the path's open -> occluded boundary swaying +-0.15 m for 6 s.
//!
//! Per frame: the player's visible fraction = its pixels in an id buffer WITH the world / WITHOUT it (two probe
//! cameras with the main camera's view render a flat green copy of the player: RenderLayers 1, the world 0, the
//! real bodies and their silhouettes 2), read back each frame; the occlusion state, radius and target flips (the
//! flicker metric: flips per second while standing still MUST be 0); frame wall time; with `--gpu` the per-pass
//! GPU times (RenderDiagnosticsPlugin; run without probes so only the game camera's passes count).
//!
//! --bench occ [--path NAME,..] [--occ off] [--no-probes] [--gpu] [--record FILE.mp4] [--tag T] [--out DIR]
//! Results: <out>/<tag>_occ.json. Virtual time runs at a fixed 1/60 s per frame (1/30 when recording), so a run
//! walks the same frames on any box.

use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    diagnostic::DiagnosticsStore,
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    world_serialization::WorldInstanceReady,
};
use glclient::{
    city::{City, ReadyBuildings},
    occlusion::Occlusion,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::cam::{CityCam, View};
use crate::occ::{Body, PLAYER_GLB, Player};
use crate::stream::Stream;

const SPEED: f32 = 1.4;
const STAND_S: f32 = 6.0;
const SETTLE_S: f32 = 0.5;
const SWAY: f32 = 0.15;
/// A path is walked from this far before its stand point to this far after it (m)
const BEFORE_M: f32 = 25.0;
const AFTER_M: f32 = 45.0;
pub const PROBE: (u32, u32) = (960, 540);

#[derive(Deserialize, Clone, Debug)]
pub struct CamPath {
    pub name: String,
    pub desc: String,
    pub pts: Vec<[f32; 3]>,
    pub stand: usize,
}

#[derive(Deserialize)]
struct CamPaths {
    paths: Vec<CamPath>,
}

/// The flat-colour copy of the player the probes count.
#[derive(Component)]
pub struct FlatBody;

/// Probe camera 0 = the world + the flat player (visible), 1 = the flat player alone, 2 = the world + the flat player +
/// its silhouette twins in the same colour (readable: visible or drawn as a silhouette, §CAM requirement 1).
#[derive(Component)]
pub struct ProbeCam(#[allow(dead_code)] pub usize);

#[derive(Resource, Clone, Default)]
pub struct ProbeCounts(pub Arc<Mutex<[u32; 3]>>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Warm,
    Walk,
    Stand,
    Done,
}

#[derive(serde::Serialize, Clone, Debug)]
struct Sample {
    t: f32,
    walk: bool,
    s: f32,
    occluded: bool,
    radius: f32,
    flips: u32,
    narrow: usize,
    wide: usize,
    with: u32,
    alone: u32,
    /// visible or silhouetted pixels
    readable: u32,
    ms: f32,
    gpu: BTreeMap<String, f64>,
    /// The squad's volumes (slots 1..): occluded, flips
    squad: Vec<(bool, u32)>,
}

#[derive(Resource)]
pub struct OccBench {
    paths: Vec<CamPath>,
    /// The walked part of each path (world xyz) and its stand index in that part
    cur: Vec<Vec3>,
    stand: usize,
    idx: usize,
    phase: Phase,
    frames: u32,
    s: f32,
    t: f32,
    pub dt: f32,
    samples: Vec<Sample>,
    results: Vec<serde_json::Value>,
    pub tag: String,
    pub out: PathBuf,
    pub probes: bool,
    pub gpu: bool,
    pub occ_on: bool,
    record: Option<Arc<Mutex<std::process::Child>>>,
    rec_pending: Arc<Mutex<u32>>,
}

pub struct OccArgs {
    pub paths: Option<Vec<String>>,
    pub occ_on: bool,
    pub probes: bool,
    pub gpu: bool,
    pub record: Option<PathBuf>,
    pub tag: String,
    pub out: PathBuf,
    pub res: (u32, u32),
}

impl OccBench {
    pub fn new(dir: &std::path::Path, a: &OccArgs) -> OccBench {
        let p = dir.join("cam_paths.json");
        let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e} (run python -m pipeline.city.cam_paths)", p.display()));
        let all: CamPaths = serde_json::from_str(&txt).unwrap();
        let paths: Vec<CamPath> = all.paths.into_iter().filter(|q| a.paths.as_ref().is_none_or(|w| w.contains(&q.name))).collect();
        assert!(!paths.is_empty(), "no such path");
        // (the window's swapchain is BGRA: the screenshot bytes come back in that order)
        let record = a.record.as_ref().map(|f| {
            let child = std::process::Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "bgra", "-s"])
                .arg(format!("{}x{}", a.res.0, a.res.1))
                .args(["-r", "30", "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20"])
                .arg(f)
                .stdin(std::process::Stdio::piped())
                .spawn()
                .expect("ffmpeg");
            Arc::new(Mutex::new(child))
        });
        let dt = if record.is_some() { 1.0 / 30.0 } else { 1.0 / 60.0 };
        OccBench {
            paths,
            cur: vec![],
            stand: 0,
            idx: 0,
            phase: Phase::Warm,
            frames: 0,
            s: 0.0,
            t: 0.0,
            dt,
            samples: vec![],
            results: vec![],
            tag: a.tag.clone(),
            out: a.out.clone(),
            probes: a.probes,
            gpu: a.gpu,
            occ_on: a.occ_on,
            record,
            rec_pending: Arc::new(Mutex::new(0)),
        }
    }

    /// The walked window of path `i`: BEFORE_M before its stand point to AFTER_M after it.
    fn window(&mut self) {
        let p = &self.paths[self.idx];
        let pts: Vec<Vec3> = p.pts.iter().map(|q| Vec3::from_array(*q)).collect();
        let mut acc = vec![0.0f32];
        for k in 1..pts.len() {
            acc.push(acc[k - 1] + pts[k].distance(pts[k - 1]));
        }
        let s0 = acc[p.stand] - BEFORE_M;
        let s1 = acc[p.stand] + AFTER_M;
        let i0 = acc.iter().position(|a| *a >= s0).unwrap_or(0);
        let i1 = acc.iter().rposition(|a| *a <= s1).unwrap_or(pts.len() - 1);
        self.cur = pts[i0..=i1].to_vec();
        self.stand = p.stand - i0;
    }

    /// The point `s` m along the current window, and the walking direction there.
    fn at(&self, s: f32) -> (Vec3, Vec2) {
        let mut left = s.max(0.0);
        for k in 1..self.cur.len() {
            let (a, b) = (self.cur[k - 1], self.cur[k]);
            let d = a.distance(b);
            if left <= d || k + 1 == self.cur.len() {
                let t = if d > 0.0 { (left / d).min(1.0) } else { 0.0 };
                return (a.lerp(b, t), (b - a).truncate().normalize_or(Vec2::Y));
            }
            left -= d;
        }
        (self.cur[0], Vec2::Y)
    }

    fn length(&self) -> f32 {
        self.cur.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    fn stand_s(&self) -> f32 {
        self.cur.windows(2).take(self.stand).map(|w| w[0].distance(w[1])).sum()
    }
}

fn pct(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s[((s.len() - 1) as f32 * p).round() as usize]
}

/// Per-pass GPU times of the latest frame (render/<pass>/elapsed_gpu, top-level spans only).
fn gpu_passes(d: &DiagnosticsStore) -> BTreeMap<String, f64> {
    let mut latest = None;
    let mut items = vec![];
    for diag in d.iter() {
        let parts: Vec<&str> = diag.path().as_str().split('/').collect();
        if parts.len() == 3
            && parts[0] == "render"
            && parts[2] == "elapsed_gpu"
            && let Some(m) = diag.measurement()
        {
            latest = latest.max(Some(m.time));
            items.push((parts[1].to_string(), m.time, m.value));
        }
    }
    let mut m = BTreeMap::new();
    let Some(latest) = latest else { return m };
    for (name, t, v) in items {
        if latest.duration_since(t).as_secs_f64() <= 0.002 {
            *m.entry(name).or_insert(0.0) += v;
        }
    }
    m
}

/// Startup (probe runs): the flat player, the two probe cameras and their readbacks, the main camera's layers.
pub fn setup_probes(
    mut commands: Commands,
    b: Res<OccBench>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    cam: Query<Entity, With<CityCam>>,
) {
    if !b.probes {
        return;
    }
    let counts = ProbeCounts::default();
    for k in 0..3 {
        let mut im = Image::new_target_texture(PROBE.0, PROBE.1, TextureFormat::Rgba8UnormSrgb, None);
        // (a readback copies out of the texture)
        im.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let img = images.add(im);
        let layers = match k {
            0 => RenderLayers::from_layers(&[0, 1]),
            1 => RenderLayers::layer(1),
            _ => RenderLayers::from_layers(&[0, 1, 3]),
        };
        let mut pc = commands.spawn((
            Camera3d::default(),
            Camera { order: -1 - k as isize, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
            RenderTarget::Image(img.clone().into()),
            Tonemapping::None,
            Msaa::Off,
            layers,
            ProbeCam(k),
            Projection::Orthographic(OrthographicProjection::default_3d()),
        ));
        // the game camera's prepass too: a cut missing from the prepass (§CAM trap 1) must show in the id buffer
        if glclient::occlusion::built() {
            pc.insert(bevy::core_pipeline::prepass::DepthPrepass);
        }
        let c = counts.clone();
        commands.spawn(Readback::texture(img)).observe(move |ev: On<ReadbackComplete>| {
            // the flat player is pure green (unlit, no tonemapping, sRGB 0 / 255 / 0)
            let n = ev.data.chunks_exact(4).filter(|p| p[1] > 240 && p[0] < 16 && p[2] < 16).count() as u32;
            c.0.lock().unwrap()[k] = n;
        });
    }
    commands.insert_resource(counts);
    for e in cam.iter() {
        commands.entity(e).insert(RenderLayers::from_layers(&[0, 2]));
    }
    commands
        .spawn((
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(PLAYER_GLB))),
            Transform::default(),
            FlatBody,
            Name::new("flat_body"),
        ))
        .observe(flat_ready);
}

/// The flat copy: unlit green, the standing pose.
fn flat_ready(
    ev: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    meshes: Query<(&Mesh3d, Option<&bevy::mesh::skinning::SkinnedMesh>)>,
    mut q: Query<(&Name, &mut Transform)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut sils: ResMut<Assets<glclient::occlusion::SilhouetteMaterial>>,
) {
    let green = mats.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() });
    // the silhouette rule (glclient's margin) in the probe colour, opaque, no edge boost
    let sil = sils.add(glclient::occlusion::SilhouetteMaterial {
        colour: Vec4::new(0.0, 1.0, 0.0, 1.0),
        misc: Vec4::new(glclient::occlusion::SIL_MARGIN, 0.0, 0.0, 0.0),
        rim: Vec4::ZERO,
    });
    for e in children.iter_descendants(ev.entity).collect::<Vec<_>>() {
        if let Ok((m, skin)) = meshes.get(e) {
            commands.entity(e).insert((MeshMaterial3d(green.clone()), RenderLayers::layer(1)));
            let mut t = commands.spawn((
                Mesh3d(m.0.clone()),
                MeshMaterial3d(sil.clone()),
                Transform::default(),
                RenderLayers::layer(3),
                ChildOf(e),
            ));
            if let Some(sk) = skin {
                t.insert(sk.clone());
            }
        }
        if let Ok((n, mut t)) = q.get_mut(e) {
            let a = match n.as_str() {
                "mixamorig:LeftArm" => -1.25,
                "mixamorig:RightArm" => 1.25,
                _ => continue,
            };
            t.rotation *= Quat::from_rotation_z(a);
        }
    }
}

/// Bodies' meshes (and their silhouette twins) on layer 2 in probe runs, so the probes see only the flat copy.
pub fn body_layers(
    mut commands: Commands,
    added: Query<Entity, (Added<Mesh3d>, Without<RenderLayers>)>,
    parents: Query<&ChildOf>,
    bodies: Query<(), With<Body>>,
) {
    for e in added.iter() {
        if parents.iter_ancestors(e).any(|a| bodies.get(a).is_ok()) {
            commands.entity(e).insert(RenderLayers::layer(2));
        }
    }
}

/// The probes and the flat copy follow the main camera / the player.
#[allow(clippy::type_complexity)]
pub fn sync_probes(
    main: Query<(&Transform, &Projection), (With<CityCam>, Without<ProbeCam>)>,
    mut probes: Query<(&mut Transform, &mut Projection), (With<ProbeCam>, Without<CityCam>, Without<FlatBody>)>,
    body: Query<(&Transform, &Body), (Without<FlatBody>, Without<CityCam>, Without<ProbeCam>)>,
    mut flat: Query<&mut Transform, (With<FlatBody>, Without<Body>, Without<CityCam>, Without<ProbeCam>)>,
) {
    let Ok((mt, mp)) = main.single() else { return };
    for (mut t, mut p) in probes.iter_mut() {
        *t = *mt;
        *p = mp.clone();
    }
    // the player (slot 0), not the first body: query order is storage order, and each glb's instancing moves its
    // root to another table, so with a squad the first body was whichever loaded first (s28's squad failures, s29)
    if let Some((bt, _)) = body.iter().find(|(_, b)| b.slot == 0) {
        for mut f in flat.iter_mut() {
            *f = *bt;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_occ_bench(
    mut commands: Commands,
    mut b: ResMut<OccBench>,
    mut player: ResMut<Player>,
    mut view: ResMut<View>,
    s: Res<Stream>,
    occ: Res<Occlusion>,
    counts: Option<Res<ProbeCounts>>,
    ready: Res<ReadyBuildings>,
    kit_stream: Option<Res<glclient::kit::KitStream>>,
    city: Res<City>,
    diag: Option<Res<DiagnosticsStore>>,
    mut exit: MessageWriter<AppExit>,
    mut last: Local<Option<std::time::Instant>>,
) {
    let now = std::time::Instant::now();
    let wall_ms = last.map_or(0.0, |l| (now - l).as_secs_f32() * 1000.0);
    *last = Some(now);
    view.scripted = true;
    player.scripted = true;
    player.on = true;
    player.occ_on = b.occ_on;
    let dt = b.dt;
    b.frames += 1;
    match b.phase {
        Phase::Warm => {
            if b.frames == 1 {
                b.window();
                let (p, d) = b.at(0.0);
                player.pos = p;
                player.facing = (-d.x).atan2(d.y);
                info!("occ bench: path {} ({}), {:.0} m", b.paths[b.idx].name, b.paths[b.idx].desc, b.length());
            }
            let glbs = glclient::kit::buildings_ready(&city, &ready, kit_stream.as_deref());
            // CV_OCC_WARM (frames, default 90): the warm-up's minimum
            let warm: u32 = std::env::var("CV_OCC_WARM").ok().and_then(|v| v.parse().ok()).unwrap_or(90);
            if (s.idle() && glbs && b.frames > warm) || b.frames > 3000.max(warm + 1) {
                b.phase = Phase::Walk;
                b.frames = 0;
                b.s = 0.0;
                b.t = 0.0;
                b.samples.clear();
            }
        }
        Phase::Walk | Phase::Stand => {
            let walk = b.phase == Phase::Walk;
            b.t += dt;
            if walk {
                b.s += SPEED * dt;
                let (p, d) = b.at(b.s);
                player.pos = p;
                player.facing = (-d.x).atan2(d.y);
                if b.s >= b.length() {
                    b.phase = Phase::Stand;
                    b.t = 0.0;
                }
            } else {
                // still on the boundary, swaying along the path (an idle shuffle)
                let s0 = b.stand_s();
                let (p, d) = b.at(s0 + SWAY * (b.t * std::f32::consts::PI).sin());
                player.pos = p;
                player.facing = (-d.x).atan2(d.y);
                if b.t >= STAND_S {
                    b.phase = Phase::Done;
                }
            }
            let v = occ.vols.first().cloned().unwrap_or_default();
            let c = counts.as_ref().map(|c| *c.0.lock().unwrap()).unwrap_or([0, 0, 0]);
            let gpu = if b.gpu { diag.as_ref().map(|d| gpu_passes(d)).unwrap_or_default() } else { BTreeMap::new() };
            let smp = Sample {
                t: b.t,
                walk,
                s: b.s,
                occluded: v.occluded,
                radius: v.radius,
                flips: v.flips,
                narrow: v.narrow,
                wide: v.wide,
                with: c[0],
                alone: c[1],
                readable: c[2],
                ms: wall_ms,
                gpu,
                squad: occ.vols.iter().skip(1).map(|v| (v.occluded, v.flips)).collect(),
            };
            b.samples.push(smp);
            if let Some(rec) = b.record.clone() {
                let pend = b.rec_pending.clone();
                *pend.lock().unwrap() += 1;
                commands.spawn(Screenshot::primary_window()).observe(move |ev: On<ScreenshotCaptured>| {
                    if let Some(d) = ev.image.data.as_ref()
                        && let Some(stdin) = rec.lock().unwrap().stdin.as_mut()
                    {
                        stdin.write_all(d).ok();
                    }
                    *pend.lock().unwrap() -= 1;
                });
            }
        }
        Phase::Done => {
            // the results are written: an extra frame before the exit lands does nothing
            if b.idx >= b.paths.len() {
                return;
            }
            // let the recorder drain before the next path / exit
            if *b.rec_pending.lock().unwrap() > 0 && b.frames < 600 {
                return;
            }
            let res = summarise(&b);
            info!("occ bench {}: {}", b.paths[b.idx].name, res);
            b.results.push(res);
            b.idx += 1;
            b.frames = 0;
            if b.idx < b.paths.len() {
                b.phase = Phase::Warm;
                return;
            }
            std::fs::create_dir_all(&b.out).ok();
            let p = b.out.join(format!("{}_occ.json", b.tag));
            let doc = serde_json::json!({
                "bench": "occ", "tag": b.tag, "occ_on": b.occ_on, "probes": b.probes, "gpu": b.gpu,
                "built": glclient::occlusion::built(), "dt": b.dt, "speed_m_s": SPEED, "stand_s": STAND_S,
                "settings": format!("{:?}", occ.settings),
                "note": "ballpark (shared box, CITY_SPEC §HONEST 4); visible fraction = probe pixels with / without the world",
                "paths": b.results,
            });
            std::fs::write(&p, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
            info!("occ bench -> {}", p.display());
            if let Some(rec) = b.record.take() {
                let mut child = rec.lock().unwrap();
                drop(child.stdin.take());
                child.wait().ok();
            }
            b.phase = Phase::Done;
            b.idx = b.paths.len();
            exit.write(AppExit::Success);
        }
    }
}

fn summarise(b: &OccBench) -> serde_json::Value {
    let p = &b.paths[b.idx];
    // (the probes' two readbacks can land a frame apart: skip the frames right after a teleport, i.e. the walk's
    // first SETTLE_S and the stand's first SETTLE_S, for the visible fraction)
    let walk: Vec<&Sample> = b.samples.iter().filter(|x| x.walk).collect();
    let stand: Vec<&Sample> = b.samples.iter().filter(|x| !x.walk && x.t >= SETTLE_S).collect();
    // visible fraction where the player is on screen at all (alone > 50 px); the readback lags a few frames
    let frac = |x: &&Sample| if x.alone > 50 { Some(x.with as f32 / x.alone as f32) } else { None };
    let fw: Vec<(f32, &Sample)> = walk.iter().filter(|x| x.t >= SETTLE_S).filter_map(|x| frac(x).map(|f| (f, *x))).collect();
    let below: Vec<&(f32, &Sample)> = fw.iter().filter(|(f, _)| *f < 0.6).collect();
    let full_r = b.samples.iter().map(|x| x.radius).fold(0.0, f32::max);
    let below_steady = below.iter().filter(|(_, x)| !(x.occluded && x.radius < full_r * 0.99)).count();
    let fs: Vec<f32> = stand.iter().filter_map(|x| frac(x)).collect();
    // flicker: target flips while standing, after the settle
    let settled: Vec<&&Sample> = stand.iter().collect();
    let flips = match (settled.first(), settled.last()) {
        (Some(a), Some(z)) => z.flips - a.flips,
        _ => 0,
    };
    let span = settled.last().map_or(0.0, |z| z.t) - settled.first().map_or(0.0, |a| a.t);
    let ms: Vec<f32> = b.samples.iter().map(|x| x.ms).collect();
    let mut gpu: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for x in &b.samples {
        for (k, v) in &x.gpu {
            gpu.entry(k.clone()).or_default().push(*v);
        }
    }
    let gpu_mean: BTreeMap<String, f64> = gpu.iter().map(|(k, v)| (k.clone(), v.iter().sum::<f64>() / v.len().max(1) as f64)).collect();
    let fmin = fw.iter().map(|x| x.0).fold(f32::INFINITY, f32::min);
    // readable = visible or drawn as a silhouette (§CAM requirement 1)
    let rd: Vec<f32> =
        walk.iter().filter(|x| x.t >= SETTLE_S && x.alone > 50).map(|x| (x.readable as f32 / x.alone as f32).min(1.0)).collect();
    serde_json::json!({
        "name": p.name, "desc": p.desc, "walk_m": b.length(), "frames": b.samples.len(),
        "occluded_share": walk.iter().filter(|x| x.occluded).count() as f32 / walk.len().max(1) as f32,
        "visible": {
            "frames_measured": fw.len(),
            "min": if fw.is_empty() { serde_json::Value::Null } else { serde_json::json!(fmin) },
            "p05": pct(&fw.iter().map(|x| x.0).collect::<Vec<_>>(), 0.05),
            "mean": fw.iter().map(|x| x.0).sum::<f32>() / fw.len().max(1) as f32,
            "seconds_below_60": below.len() as f32 * b.dt,
            "seconds_below_60_steady": below_steady as f32 * b.dt,
            "seconds_below_60_transition": (below.len() - below_steady) as f32 * b.dt,
            "stand_min": if fs.is_empty() { serde_json::Value::Null } else { serde_json::json!(fs.iter().copied().fold(f32::INFINITY, f32::min)) },
        },
        "readable": {
            "frames_measured": rd.len(),
            "min": if rd.is_empty() { serde_json::Value::Null } else { serde_json::json!(rd.iter().copied().fold(f32::INFINITY, f32::min)) },
            "seconds_below_60": rd.iter().filter(|f| **f < 0.6).count() as f32 * b.dt,
            // the longest continuous stretch under 60 % (a door jamb passed at 0.2 m is a few frames)
            "longest_below_60_s": rd.iter().fold((0u32, 0u32), |(cur, best), f| if *f < 0.6 { (cur + 1, best.max(cur + 1)) } else { (0, best) }).1 as f32 * b.dt,
        },
        // where it dipped (diagnosis): walk metres, fraction, the state (first 60 such frames)
        "below_60_at": below.iter().take(60).map(|(f, x)| serde_json::json!([(x.s * 10.0).round() / 10.0, (f * 100.0).round() / 100.0, x.occluded, (x.radius * 100.0).round() / 100.0, x.with, x.alone])).collect::<Vec<_>>(),
        "flicker": {"stand_flips": flips, "stand_seconds": span, "flips_per_s": if span > 0.0 { flips as f32 / span } else { 0.0 },
                    "walk_flips": walk.last().map_or(0, |z| z.flips) - walk.first().map_or(0, |a| a.flips)},
        "frame_ms": {"p50": pct(&ms, 0.5), "p95": pct(&ms, 0.95), "max": pct(&ms, 1.0)},
        "gpu_ms_mean": gpu_mean,
        "gpu_ms_total_mean": gpu_mean.values().sum::<f64>(),
        // per squad member (--squad N): the walk's occluded share, target flips while standing (after the settle)
        "squad": (0..b.samples.iter().map(|x| x.squad.len()).max().unwrap_or(0)).map(|k| serde_json::json!({
            "occluded_share": walk.iter().filter(|x| x.squad.get(k).is_some_and(|s| s.0)).count() as f32 / walk.len().max(1) as f32,
            "stand_flips": match (settled.first().and_then(|a| a.squad.get(k)), settled.last().and_then(|z| z.squad.get(k))) {
                (Some(a), Some(z)) => z.1 - a.1,
                _ => 0,
            },
        })).collect::<Vec<_>>(),
    })
}

/// The HUD caption in recordings: the path and the switch.
pub fn caption(b: &OccBench) -> String {
    match b.paths.get(b.idx) {
        Some(p) => format!("P4 camera occlusion  |  {}: {}  |  occlusion {}", p.name, p.desc, if b.occ_on { "ON" } else { "OFF" }),
        None => String::new(),
    }
}
