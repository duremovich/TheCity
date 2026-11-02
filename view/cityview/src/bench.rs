//! Benches and shots (P3 gates: "tier M loads and streams; frame time and streaming hitches measured;
//! screenshots of the game view in D1 / D2 / D4 and the overview"). Ballpark numbers (shared box, §HONEST 4).
//!
//! --bench stream    the game camera flies a path through D1 -> D7 -> D2 -> D3 -> D4 -> D2 at --speed m/s, zoom
//!                   --zoom: frame times (p50 / p95 / p99 / max), frames over 33 / 50 ms, chunk build / insert times
//! --bench overview  the overview orbits the whole island once (every chunk loaded, coarse ground)
//! --bench hold      the view the args set up (--at, --zoom) held for 10 s after warm-up: frame times there
//!                   (P5 G: the generated sub-block at the game camera; CV_NO_GEN=1 is its massing-only control)
//! --bench shots     the review pictures (night + a few day-light structure shots) into --out
//! Results: <out>/<tag>_<bench>.json (+ png).

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::path::PathBuf;

use crate::cam::{Mode, View};
use crate::stream::Stream;
use glclient::city::{City, ReadyBuildings};

#[derive(Clone, Debug)]
pub struct Shot {
    pub name: String,
    pub mode: Mode,
    pub at: Vec2,
    pub zoom: f32,
    pub orbit: Option<(Vec3, f32, f32, f32)>,
    pub day: bool,
    /// Debug: massing hidden (the ground and the pinned block alone)
    pub hide_mass: bool,
    /// C2 (`--save --shots`): the sim tick the shot waits for (the sim thread holds there)
    pub tick: Option<u64>,
}

/// C2: the follow-cam's shots (`--bench one --save .. --shots T1,T2,..`): the sim thread's buffer and hold.
#[derive(Resource)]
pub struct Shots {
    pub shared: std::sync::Arc<crate::simthread::Shared>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Warm,
    Run,
    Settle,
    Done,
}

#[derive(serde::Serialize, Clone, Copy, Debug)]
struct Sample {
    /// wall time since the previous frame (Time's delta is clamped at 250 ms)
    ms: f32,
    /// seconds since the run started
    t: f32,
    inserted: usize,
    insert_ms: f32,
    pending: usize,
    loaded: usize,
    tris: usize,
    /// kit buildings spawned this frame (P8 B-A streaming), mesh assets alive
    kit: usize,
    meshes: usize,
    kit_ms: f32,
}

#[derive(Resource)]
pub struct Bench {
    pub kind: String,
    pub tag: String,
    pub out: PathBuf,
    pub speed: f32,
    pub zoom: f32,
    path: Vec<Vec2>,
    seg: usize,
    phase: Phase,
    frames: u32,
    t: f32,
    samples: Vec<Sample>,
    shots: Vec<Shot>,
    shot: usize,
    /// The last screenshot: 0 pending, 1 saved, 2 came back black (retaken; review p3 r1 saw a flaky black PNG)
    shot_state: std::sync::Arc<std::sync::atomic::AtomicU8>,
    retries: u32,
    /// The frame the shot's view was ready (idle, every building in) and the shadow allocation was reset
    ready_at: Option<u32>,
}

impl Bench {
    pub fn new(kind: &str, tag: &str, out: PathBuf, speed: f32, zoom: f32, s: &Stream, xway: Option<(Vec2, Vec2)>) -> Bench {
        let v = |d: &str| s.index.views.get(d).map(|p| Vec2::from(*p)).unwrap_or(Vec2::new(s.index.size_m[0] / 2.0, 800.0));
        // CV_BENCH_PATH="x,y;x,y;..." (city m) overrides the district tour (P6: a walk across one superblock)
        let path = match std::env::var("CV_BENCH_PATH") {
            Ok(p) => {
                let pts: Vec<Vec2> = p
                    .split(';')
                    .map(|xy| {
                        let v: Vec<f32> = xy.split(',').map(|t| t.trim().parse::<f32>()).collect::<Result<_, _>>().unwrap_or_default();
                        match v[..] {
                            [x, y] => Vec2::new(x, y),
                            _ => panic!("CV_BENCH_PATH: '{xy}' is not 'x,y' (the path is 'x,y;x,y;...')"),
                        }
                    })
                    .collect();
                assert!(pts.len() >= 2, "CV_BENCH_PATH: needs at least two points, got '{p}'");
                pts
            }
            Err(_) => vec![v("D1"), v("D7"), v("D2"), v("D3"), v("D4"), v("D2")],
        };
        let mut shots = vec![];
        if kind == "one" {
            // a single shot of the view the args set up (--at, --zoom, --overview, --day): debugging
            shots.push(Shot {
                name: tag.into(),
                mode: Mode::Game,
                at: Vec2::ZERO,
                zoom,
                orbit: None,
                day: false,
                hide_mass: false,
                tick: None,
            });
        }
        if kind == "shots" {
            let centre = Vec3::new(s.index.size_m[0] / 2.0, s.index.size_m[1] / 2.0, 60.0);
            let ov = |name: &str, c: Vec3, yaw: f32, pitch: f32, dist: f32, day: bool| Shot {
                name: name.into(),
                mode: Mode::Overview,
                at: c.truncate(),
                zoom: 60.0,
                orbit: Some((c, yaw, pitch, dist)),
                day,
                hide_mass: false,
                tick: None,
            };
            let g = |name: &str, at: Vec2, zoom: f32, day: bool| Shot {
                name: name.into(),
                mode: Mode::Game,
                at,
                zoom,
                orbit: None,
                day,
                hide_mass: false,
                tick: None,
            };
            let q = std::f32::consts::FRAC_PI_4;
            // the whole island in frame: the centre pulled S and E (the E end ran off the frame: review p3 r4)
            let c2 = centre + Vec3::new(350.0, -350.0, 0.0);
            shots.push(ov("overview", c2, q, 0.62, 5600.0, false));
            shots.push(ov("overview_day", c2, q, 0.62, 5600.0, true));
            let d2 = v("D2");
            shots.push(ov("overview_d2", Vec3::new(d2.x, d2.y + 300.0, 40.0), q, 0.4, 1600.0, false));
            for d in ["D1", "D4"] {
                shots.push(g(&format!("game_{d}_z60"), v(d), 60.0, false));
                shots.push(g(&format!("game_{d}_z140"), v(d), 140.0, false));
            }
            // the core from the game camera at a skyline zoom (at 60 / 140 one facade filled the frame: review p3 r2)
            shots.push(g("game_D2_z400", v("D2"), 400.0, false));
            shots.push(g("game_D2_z400_day", v("D2"), 400.0, true));
            shots.push(g("game_D2_z1000", v("D2"), 1000.0, false));
            shots.push(g("game_D3_z140", v("D3"), 140.0, false));
            shots.push(g("game_D6_z400", v("D6"), 400.0, false));
            shots.push(g("game_D4_z400", v("D4"), 400.0, false));
            if let Some(p) = s.index.pins.first() {
                // the block's middle, a little N of its frontage (its height rises up the frame)
                let at = Vec2::new(p.origin_m[0] + 16.0, p.origin_m[1] + 12.0);
                shots.push(g("game_pin_blk01_z40", at, 40.0, false));
                shots.push(g("game_pin_blk01_z40_day", at, 40.0, true));
                // the generated massing hidden (debug, not occlusion): blk01 on its lot, its street, its platform
                shots.push(Shot { hide_mass: true, ..g("game_pin_blk01_z40_nomass", at, 40.0, false) });
                shots.push(Shot { hide_mass: true, ..g("game_pin_blk01_z40_nomass_day", at, 40.0, true) });
            }
            // the expressway (the P2 decks from the game camera)
            if let Some((xs, xe)) = xway {
                // (no night shot of the bare deck: without P7's lamps it is an unreadable dark plane: review p3 r3)
                shots.push(g("game_xway_z140_day", xs, 140.0, true));
                shots.push(g("game_xway_curve_z140", xe, 140.0, false));
            }
        }
        Bench {
            kind: kind.into(),
            tag: tag.into(),
            out,
            speed,
            zoom,
            path,
            seg: 0,
            phase: Phase::Warm,
            frames: 0,
            ready_at: None,
            t: 0.0,
            samples: vec![],
            shots,
            shot: 0,
            shot_state: Default::default(),
            retries: 0,
        }
    }

    /// C2: one shot per sim tick in place of the single one (`<tag>_d<D>_<HHMM>.png`).
    pub fn at_ticks(&mut self, ticks: &[u64]) {
        if ticks.is_empty() || self.kind != "one" {
            return;
        }
        let one = self.shots[0].clone();
        self.shots = ticks
            .iter()
            .map(|&t| {
                let (d, m) = (t / citysim::TICKS_PER_DAY, t % citysim::TICKS_PER_DAY);
                Shot { name: format!("{}_d{d}_{:02}{:02}", one.name, m / 60, m % 60), tick: Some(t), ..one.clone() }
            })
            .collect();
    }

    fn apply_shot(&self, view: &mut View) {
        if self.kind == "one" {
            return;
        }
        let s = &self.shots[self.shot];
        view.mode = s.mode;
        view.at = s.at;
        view.zoom = s.zoom;
        // V10: a day shot forces the day; the rest follow the sim's clock (no sim: night)
        view.day = s.day.then_some(true);
        view.hide_mass = s.hide_mass;
        if let Some((c, yaw, pitch, dist)) = s.orbit {
            view.orbit_at = c;
            view.yaw = yaw;
            view.pitch = pitch;
            view.dist = dist;
        }
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

#[allow(clippy::too_many_arguments)]
pub fn run_bench(
    mut commands: Commands,
    mut b: ResMut<Bench>,
    mut view: ResMut<View>,
    s: Res<Stream>,
    time: Res<Time>,
    ready: Res<ReadyBuildings>,
    kit_stream: Option<Res<glclient::kit::KitStream>>,
    city: Res<City>,
    meshes: Res<Assets<Mesh>>,
    mut exit: MessageWriter<AppExit>,
    mut last: Local<Option<std::time::Instant>>,
    sim_shots: Option<Res<Shots>>,
    sim: Option<Res<crate::sim::Sim>>,
) {
    let now = std::time::Instant::now();
    let wall_ms = last.map_or(0.0, |l| (now - l).as_secs_f32() * 1000.0);
    *last = Some(now);
    let dt = time.delta_secs();
    b.frames += 1;
    view.scripted = true;
    match b.kind.as_str() {
        "stream" | "overview" | "hold" => match b.phase {
            Phase::Warm => {
                if b.frames == 1 {
                    if b.kind == "overview" {
                        view.mode = Mode::Overview;
                    } else if b.kind == "stream" {
                        view.mode = Mode::Game;
                        view.zoom = b.zoom;
                        view.at = b.path[0];
                    }
                }
                if s.idle() && b.frames > 120 {
                    b.phase = Phase::Run;
                    b.frames = 0;
                    info!("bench {}: warm, running", b.kind);
                }
            }
            Phase::Run => {
                let st = &s.stats;
                let t = b.samples.last().map_or(0.0, |x| x.t) + wall_ms / 1000.0;
                b.samples.push(Sample {
                    ms: wall_ms,
                    t,
                    inserted: st.inserted,
                    insert_ms: st.insert_ms,
                    pending: st.pending,
                    loaded: st.loaded,
                    tris: st.tris,
                    kit: kit_stream.as_ref().map_or(0, |k| k.stats.spawned_frame),
                    meshes: meshes.len(),
                    kit_ms: kit_stream.as_ref().map_or(0.0, |k| k.stats.main_ms),
                });
                if b.kind == "overview" {
                    b.t += dt;
                    view.yaw += dt * std::f32::consts::TAU / 20.0;
                    if b.t >= 20.0 {
                        b.phase = Phase::Done;
                    }
                } else if b.kind == "hold" {
                    b.t += dt;
                    if b.t >= 10.0 {
                        b.phase = Phase::Done;
                    }
                } else {
                    let mut step = b.speed * dt;
                    while step > 0.0 && b.seg + 1 < b.path.len() {
                        let target = b.path[b.seg + 1];
                        let d = target - view.at;
                        if d.length() <= step {
                            view.at = target;
                            step -= d.length();
                            b.seg += 1;
                        } else {
                            view.at += d.normalize() * step;
                            step = 0.0;
                        }
                    }
                    if b.seg + 1 >= b.path.len() {
                        b.phase = Phase::Done;
                    }
                }
            }
            Phase::Done | Phase::Settle => {
                let ms: Vec<f32> = b.samples.iter().map(|x| x.ms).collect();
                let mut worst: Vec<(usize, Sample)> = b.samples.iter().copied().enumerate().collect();
                worst.sort_by(|a, c| c.1.ms.total_cmp(&a.1.ms));
                worst.truncate(10);
                let st = &s.stats;
                // streaming's own cost: frames that inserted chunks (or follow one: the upload renders a frame
                // later) vs the rest; the box's stalls (an empty-scene control has them too) hit both alike
                let ins: Vec<bool> =
                    (0..b.samples.len()).map(|i| b.samples[i].inserted > 0 || (i > 0 && b.samples[i - 1].inserted > 0)).collect();
                let split = |want: bool| -> serde_json::Value {
                    let v: Vec<f32> = b.samples.iter().zip(&ins).filter(|(_, f)| **f == want).map(|(x, _)| x.ms).collect();
                    serde_json::json!({"frames": v.len(), "p50": pct(&v, 0.5), "p95": pct(&v, 0.95), "p99": pct(&v, 0.99),
                                       "max": pct(&v, 1.0), "over_33ms": v.iter().filter(|m| **m > 33.3).count()})
                };
                // the kit stream's cost: frames that spawned kit buildings (or follow one) vs frames that did neither
                let kitf: Vec<bool> = (0..b.samples.len()).map(|i| b.samples[i].kit > 0 || (i > 0 && b.samples[i - 1].kit > 0)).collect();
                let split_kit = |want: bool| -> serde_json::Value {
                    let v: Vec<f32> = b
                        .samples
                        .iter()
                        .zip(&kitf)
                        .zip(&ins)
                        .filter(|((_, k), c)| **k == want && (want || !**c))
                        .map(|((x, _), _)| x.ms)
                        .collect();
                    serde_json::json!({"frames": v.len(), "p50": pct(&v, 0.5), "p95": pct(&v, 0.95), "p99": pct(&v, 0.99),
                                       "max": pct(&v, 1.0), "over_33ms": v.iter().filter(|m| **m > 33.3).count()})
                };
                let insert_ms: Vec<f32> = b.samples.iter().filter(|x| x.inserted > 0).map(|x| x.insert_ms).collect();
                let res = serde_json::json!({
                    "insert_frames": split(true),
                    "other_frames": split(false),
                    "kit_frames": split_kit(true),
                    "kit_main_ms": {"p50": pct(&b.samples.iter().filter(|x| x.kit > 0).map(|x| x.kit_ms).collect::<Vec<_>>(), 0.5),
                                    "max": b.samples.iter().map(|x| x.kit_ms).fold(0.0f32, f32::max)},
                    "quiet_frames": split_kit(false),
                    "insert_main_thread_ms": {"p50": pct(&insert_ms, 0.5), "max": pct(&insert_ms, 1.0)},
                    "bench": b.kind, "tag": b.tag, "city": s.index.city, "seed": s.index.seed,
                    "zoom": b.zoom, "speed_m_s": b.speed, "frames": ms.len(),
                    "seconds": ms.iter().sum::<f32>() / 1000.0,
                    "ms": {"p50": pct(&ms, 0.5), "p95": pct(&ms, 0.95), "p99": pct(&ms, 0.99), "max": pct(&ms, 1.0),
                           "mean": ms.iter().sum::<f32>() / ms.len().max(1) as f32},
                    "frames_over_33ms": ms.iter().filter(|m| **m > 33.3).count(),
                    "frames_over_50ms": ms.iter().filter(|m| **m > 50.0).count(),
                    "stall_times_s": b.samples.iter().filter(|x| x.ms > 33.3).map(|x| (x.t * 10.0).round() / 10.0).collect::<Vec<_>>(),
                    "worst": worst.iter().map(|(i, x)| serde_json::json!({"frame": i, "sample": x})).collect::<Vec<_>>(),
                    "chunks_built": st.built,
                    // P8 B-A: kit chunks loaded / unloaded over the run, chunks + buildings resident at the end
                    "kit_stream": kit_stream.as_ref().map(|k| k.stats),
                    // mesh assets alive: peak over the run and at its end (a leak grows the end over repeated passes)
                    "meshes": {"peak": b.samples.iter().map(|x| x.meshes).max(), "end": b.samples.last().map(|x| x.meshes),
                               "every_10s": b.samples.iter().scan(-1.0f32, |n, x| { let k = (x.t / 10.0).floor(); if k > *n { *n = k; Some(Some(x.meshes)) } else { Some(None) } }).flatten().collect::<Vec<_>>()},
                    "build_ms_mean": st.build_ms_sum / st.built.max(1) as f64,
                    "build_ms_max": st.build_ms_max,
                    "unloaded": st.unloaded,
                    "loaded_max": b.samples.iter().map(|x| x.loaded).max(),
                    "tris_max": b.samples.iter().map(|x| x.tris).max(),
                    "note": "ballpark: shared box (CITY_SPEC §HONEST 4); frame = wall time per frame, vsync off",
                });
                std::fs::create_dir_all(&b.out).ok();
                let p = b.out.join(format!("{}_{}.json", b.tag, b.kind));
                std::fs::write(&p, serde_json::to_string_pretty(&res).unwrap()).unwrap();
                info!("bench {} -> {}: {}", b.kind, p.display(), res["ms"]);
                exit.write(AppExit::Success);
            }
        },
        "shots" | "one" => {
            if b.shot >= b.shots.len() {
                if b.frames > 20 {
                    exit.write(AppExit::Success);
                }
                return;
            }
            match b.phase {
                Phase::Warm => {
                    b.apply_shot(&mut view);
                    // C2: the sim thread runs to the shot's tick and holds there; V6: the shot at diary minute t is
                    // frame t + 1 (the state minute t left, citysim::view's module doc)
                    if let (Some(t), Some(sh)) = (b.shots[b.shot].tick, sim_shots.as_ref()) {
                        sh.shared.hold.store(t + 1, std::sync::atomic::Ordering::Relaxed);
                    }
                    b.phase = Phase::Run;
                    b.frames = 0;
                }
                Phase::Run => {
                    // C2: not before the sim's newest frame is the shot's tick (the wait and its timeout start then)
                    if let (Some(t), Some(sh)) = (b.shots[b.shot].tick, sim_shots.as_ref())
                        && sh.shared.pair().1.is_none_or(|p| p.frame.tick < t + 1)
                    {
                        b.frames = 0;
                        return;
                    }
                    // wait: streaming idle, the pinned glbs in, the camera settled on the ground
                    let glbs = glclient::kit::buildings_ready(&city, &ready, kit_stream.as_deref());
                    // CV_SHOT_MIN_FRAMES (default 60): a longer wait lets the camera's ease finish (glb vs kit
                    // comparisons: the kit is ready sooner, an early shot catches the camera still moving)
                    let min_frames: u32 = std::env::var("CV_SHOT_MIN_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
                    // once the view is ready (idle, every building and light in, the camera settled): one fresh shadow
                    // allocation (no load-order hysteresis in the frame), the shot 30 frames later
                    if b.ready_at.is_none() && s.idle() && glbs && b.frames > min_frames {
                        commands.insert_resource(glclient::lighting::AllocReset(true));
                        b.ready_at = Some(b.frames);
                    }
                    if b.ready_at.is_some_and(|r| b.frames >= r + 30) || b.frames > 1800.max(min_frames + 31) {
                        b.ready_at = None;
                        std::fs::create_dir_all(&b.out).ok();
                        let p = b.out.join(format!("{}.png", b.shots[b.shot].name));
                        info!("shot {} (idle {}, {} frames)", p.display(), s.idle(), b.frames);
                        if let Some(sim) = sim.as_ref() {
                            // the caption in the log too (the report quotes it beside the PNG)
                            info!("shot {}: {}", b.shots[b.shot].name, sim.hud.replace('\n', " | "));
                        }
                        let state = b.shot_state.clone();
                        state.store(0, std::sync::atomic::Ordering::SeqCst);
                        let mut save = save_to_disk(p);
                        commands.spawn(Screenshot::primary_window()).observe(move |ev: On<ScreenshotCaptured>| {
                            // BGRA / RGBA bytes: any colour byte lit (alpha skipped)
                            let lit = ev.image.data.as_ref().is_some_and(|d| d.iter().enumerate().any(|(i, v)| i % 4 != 3 && *v > 0));
                            if lit {
                                save(ev);
                                state.store(1, std::sync::atomic::Ordering::SeqCst);
                            } else {
                                state.store(2, std::sync::atomic::Ordering::SeqCst);
                            }
                        });
                        b.phase = Phase::Settle;
                        b.frames = 0;
                    }
                }
                Phase::Settle => {
                    let st = b.shot_state.load(std::sync::atomic::Ordering::SeqCst);
                    if st == 2 && b.retries < 3 {
                        warn!("shot {} came back black: retaking", b.shots[b.shot].name);
                        b.retries += 1;
                        b.phase = Phase::Run;
                        b.frames = 0;
                    } else if (st != 0 && b.frames > 10) || b.frames > 600 {
                        b.shot += 1;
                        b.retries = 0;
                        b.phase = Phase::Warm;
                        b.frames = 0;
                    }
                }
                Phase::Done => {}
            }
        }
        _ => panic!("unknown bench {}", b.kind),
    }
}

#[derive(Component)]
pub struct Hud;

pub fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::srgb(0.85, 0.95, 0.85)),
        Node { position_type: PositionType::Absolute, top: px(8), left: px(8), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Hud,
    ));
}

pub fn update_hud(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    s: Res<Stream>,
    view: Res<View>,
    bench: Option<Res<Bench>>,
    sim: Option<Res<crate::sim::Sim>>,
    mut hud: Query<(&mut Text, &mut Visibility), With<Hud>>,
    mut avg: Local<f32>,
) {
    let ms = time.delta_secs() * 1000.0;
    *avg = if *avg == 0.0 { ms } else { *avg * 0.95 + ms * 0.05 };
    for (mut t, mut v) in hud.iter_mut() {
        if keys.just_pressed(KeyCode::KeyH) {
            *v = if *v == Visibility::Hidden { Visibility::Inherited } else { Visibility::Hidden };
        }
        // the review shots go out without the overlay
        // (a sim replay keeps its caption: the frame's tick and counts)
        if bench.as_ref().is_some_and(|b| b.kind == "shots" || (b.kind == "one" && sim.is_none())) {
            *v = Visibility::Hidden;
        }
        let st = &s.stats;
        t.0 = format!(
            "{} {:.1} ms ({:.0} fps)  {:?} zoom {:.0} yaw {:.0} pitch {:.0}  light {}  at ({:.0}, {:.0}) z {:.1}\nchunks {} loaded / {} wanted / {} building, {:.2} Mtris  builds {} (mean {:.1} ms, max {:.1})  insert {:.2} ms\n{}",
            s.index.city,
            *avg,
            1000.0 / avg.max(0.01),
            view.mode,
            view.zoom,
            view.game_yaw.to_degrees(),
            view.game_pitch.to_degrees(),
            // V10: L's override or the clock's day fraction
            match view.day {
                Some(true) => "day (L)".to_string(),
                Some(false) => "night (L)".to_string(),
                None => format!("clock {:.2}", view.daylight),
            },
            view.at.x,
            view.at.y,
            view.ground_z,
            st.loaded,
            st.wanted,
            st.pending,
            st.tris as f32 / 1e6,
            st.built,
            st.build_ms_sum / st.built.max(1) as f64,
            st.build_ms_max,
            st.insert_ms,
            "WASD pan  Q/E rotate  Z/X pitch  wheel -/= zoom  RMB drag orbit  Tab overview  G cut  L light  M massing  H hud  P shot",
        );
        if let Some(sim) = sim.as_ref() {
            t.0.push('\n');
            t.0.push_str(&sim.hud);
        }
    }
}

pub fn screenshot_key(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut n: Local<u32>, b: Option<Res<Bench>>) {
    if keys.just_pressed(KeyCode::KeyP) {
        let dir = b.map(|b| b.out.clone()).unwrap_or_else(|| PathBuf::from("results"));
        std::fs::create_dir_all(&dir).ok();
        let p = dir.join(format!("shot_{:03}.png", *n));
        *n += 1;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(p));
    }
}
