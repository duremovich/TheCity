//! Films (docs/TESTING.md, "Filming motion"): a `--play` script's `film NAME SECS [FPS]` or the CLI's `--film
//! NAME:SECS[:FPS]` (with `--save F --follow N [--from T] [--play-speed X]`: an NPC watched, nobody possessed) records
//! SECS seconds of the live view as `<out>/NAME/frame_00001.png` .. (the window, HUD and all, the `shot` path) at FPS
//! (default 10), a motion trace beside them (`trace.csv`: a row per frame per drawn body within `[view] film_trace_m`
//! of the subject) and `meta.json`. `tools/film.py DIR` makes the mp4, the contact sheets and the motion analysis.
//!
//! **The clock is a fixed step**: while a film runs, the pacing's clock (`simthread::Clock`, `Shared::now`) is the
//! film's own. The render thread steps it 1/FPS a captured frame, the sim thread ticks each tick at its due time on
//! it and publishes every tick (it waits for the render thread to take the last frame: the film draws the pairs a tick
//! apart, never the stitched trail), Bevy's time steps 1/FPS a captured frame too (`TimeUpdateStrategy`: the clips,
//! the camera's ease) and 0 on the frames between, so the frames stand for even game-real time however slow the
//! capture, and the mp4 plays back at true speed. A frame is captured once the sim thread has published every tick due
//! by its film time (`Shared::film_ready`), checked before the frame is drawn; one whose sim never gets there in
//! `READY_WAIT_S` wall seconds is captured late and counted (`meta.json` `frames_late`).

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    time::TimeUpdateStrategy,
    transform::TransformSystems,
};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::simthread::Shared;

/// A frame whose sim is not ready after this many wall seconds is captured anyway (a stuck sim thread must not hang
/// the film), and counted late.
const READY_WAIT_S: f64 = 5.0;
/// A finished film waits at most this long for its PNGs to be written.
const WRITE_WAIT_S: f64 = 60.0;

/// A film asked for: its name (the directory under `--out`), its length (film seconds) and frames a second.
#[derive(Clone, Debug)]
pub struct FilmReq {
    pub name: String,
    pub secs: f32,
    pub fps: f32,
}

impl FilmReq {
    /// `NAME SECS [FPS]` (a script's) or `NAME:SECS[:FPS]` (the CLI's).
    pub fn parse(s: &str) -> FilmReq {
        let parts: Vec<&str> = s.split([' ', ':']).filter(|p| !p.is_empty()).collect();
        assert!((2..=3).contains(&parts.len()), "film '{s}': NAME SECS [FPS]");
        let secs: f32 = parts[1].parse().unwrap_or_else(|_| panic!("film '{s}': SECS a number"));
        let fps: f32 = parts.get(2).map_or(10.0, |f| f.parse().unwrap_or_else(|_| panic!("film '{s}': FPS a number")));
        assert!(secs > 0.0 && fps > 0.0, "film '{s}': SECS and FPS above 0");
        FilmReq { name: parts[0].to_string(), secs, fps }
    }
}

struct Run {
    req: FilmReq,
    dir: PathBuf,
    total: u32,
    /// Frames captured.
    n: u32,
    /// This frame's film time was ready when the frame began (`film_pre`).
    ready_now: bool,
    late_now: bool,
    late: u32,
    /// Wall clock: the film's start, the current frame's wait.
    wall0: Instant,
    wait0: Instant,
    trace: std::io::BufWriter<std::fs::File>,
    written: Arc<AtomicU32>,
    ticks: (Option<u64>, u64),
    speeds: Vec<f64>,
    clock0: String,
}

/// A film done, its PNGs being written: then the script goes on (or the CLI's run exits).
struct Writing {
    dir: PathBuf,
    total: u32,
    written: Arc<AtomicU32>,
    since: Instant,
}

#[derive(Resource)]
pub struct Film {
    shared: Arc<Shared>,
    out: PathBuf,
    trace_m: f32,
    walk_m_s: f64,
    save: String,
    subject: Option<u32>,
    /// The CLI's film: started once the sim reaches `from` (taken then), and the app exits when it is written.
    cli: Option<(FilmReq, Option<u64>)>,
    exit_when_written: bool,
    run: Option<Run>,
    writing: Option<Writing>,
}

impl Film {
    /// A film running or still writing (a script waits on it).
    pub fn busy(&self) -> bool {
        self.run.is_some() || self.writing.is_some()
    }

    /// A script's `film`: starts at once.
    pub fn start(&mut self, req: FilmReq, sim: &crate::sim::Sim) {
        assert!(self.run.is_none(), "film {}: a film is already running", req.name);
        let dir = self.out.join(&req.name);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("film {}: {}: {e}", req.name, dir.display()));
        // a film's directory holds this film only (an old take's frames would mix into the sheets)
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if (n.starts_with("frame_") && n.ends_with(".png")) || n == "trace.csv" || n == "meta.json" {
                    std::fs::remove_file(e.path()).ok();
                }
            }
        }
        let f = std::fs::File::create(dir.join("trace.csv")).expect("film: trace.csv");
        let mut trace = std::io::BufWriter::new(f);
        writeln!(
            trace,
            "frame,film_t,wall_s,tick_from,tick_to,u,view_tick,speed_x,body,is_subject,x,y,z,yaw_deg,tile_x,tile_y,prev_x,prev_y,\
floor,indoors,aboard,act,tier,walking,lod,clip,clip_rate,lying,lean,sx,sy"
        )
        .unwrap();
        let total = (req.secs * req.fps).round().max(1.0) as u32;
        let clock0 = sim.clock_tick.map_or(String::new(), |t| {
            let m = t % citysim::TICKS_PER_DAY;
            format!("d{} {:02}:{:02}", t / citysim::TICKS_PER_DAY, m / 60, m % 60)
        });
        info!("film {}: {total} frames at {} fps into {} (the fixed-step clock)", req.name, req.fps, dir.display());
        self.shared.hold.store(u64::MAX, Ordering::Relaxed);
        self.shared.film_begin(1.0 / f64::from(req.fps));
        let now = Instant::now();
        self.run = Some(Run {
            req,
            dir,
            total,
            n: 0,
            ready_now: false,
            late_now: false,
            late: 0,
            wall0: now,
            wait0: now,
            trace,
            written: Arc::new(AtomicU32::new(0)),
            ticks: (None, 0),
            speeds: vec![],
            clock0,
        });
    }
}

/// main.rs: the film's systems, with a live sim (`--save`).
#[allow(clippy::too_many_arguments)]
pub fn add(
    app: &mut App,
    shared: Arc<Shared>,
    vcfg: &citysim::config::ViewCfg,
    out: PathBuf,
    save: String,
    subject: Option<u32>,
    cli: Option<FilmReq>,
    from: Option<u64>,
) {
    app.insert_resource(Film {
        shared,
        out,
        trace_m: vcfg.film_trace_m,
        walk_m_s: vcfg.walk_m_s,
        save,
        subject,
        exit_when_written: cli.is_some(),
        cli: cli.map(|c| (c, from)),
        run: None,
        writing: None,
    })
    .add_systems(Update, film_pre.before(crate::sim::update_sim))
    .add_systems(PostUpdate, film_post.after(TransformSystems::Propagate));
}

/// Before the frame is drawn: whether the sim has published every tick due by the film's time (then this frame is
/// captured), and the CLI's film started once the sim reaches `--from`.
fn film_pre(mut film: ResMut<Film>, sim: Res<crate::sim::Sim>) {
    if film.cli.as_ref().is_some_and(|(_, from)| sim.clock_tick.is_some_and(|t| from.is_none_or(|f| t >= f)))
        && let Some((req, _)) = film.cli.take()
    {
        film.start(req, &sim);
    }
    let shared = film.shared.clone();
    let Some(r) = film.run.as_mut() else { return };
    let t = shared.film_t().unwrap_or(0.0);
    r.ready_now = shared.film_ready() >= t - 1e-9;
    r.late_now = !r.ready_now && r.wait0.elapsed().as_secs_f64() > READY_WAIT_S;
}

/// After the frame's transforms: a ready frame captured (the PNG, its trace rows), the clock stepped for the next
/// one; the film's end (the meta, the clock back to the wall, the CLI's exit once the PNGs are written).
#[allow(clippy::too_many_arguments)]
fn film_post(
    mut commands: Commands,
    mut film: ResMut<Film>,
    sim: Res<crate::sim::Sim>,
    people: Option<Res<crate::people::People>>,
    cams: Query<(&Camera, &GlobalTransform), With<crate::cam::CityCam>>,
    windows: Query<&Window>,
    mut strategy: ResMut<TimeUpdateStrategy>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(w) = film.writing.as_ref() {
        let n = w.written.load(Ordering::Relaxed);
        let timed_out = w.since.elapsed().as_secs_f64() > WRITE_WAIT_S;
        if n >= w.total || timed_out {
            if timed_out {
                warn!("film {}: {n} of {} frames written after {WRITE_WAIT_S} s", w.dir.display(), w.total);
            }
            info!("film {}: written", w.dir.display());
            film.writing = None;
            if film.exit_when_written {
                exit.write(AppExit::Success);
            }
        }
        return;
    }
    let shared = film.shared.clone();
    let (trace_m, walk_m_s, save_name, subject_ix) = (film.trace_m, film.walk_m_s, film.save.clone(), film.subject);
    let Some(r) = film.run.as_mut() else { return };
    let dt = 1.0 / f64::from(r.req.fps);
    if !(r.ready_now || r.late_now) {
        // not ready: the frame is drawn, not captured, and Bevy's time stands
        *strategy = TimeUpdateStrategy::ManualDuration(Duration::ZERO);
        return;
    }
    if r.late_now {
        r.late += 1;
        warn!("film {}: frame {} captured late (the sim thread not ready in {READY_WAIT_S} s)", r.req.name, r.n + 1);
    }
    let film_t = shared.film_t().unwrap_or(0.0);
    r.n += 1;
    let path = r.dir.join(format!("frame_{:05}.png", r.n));
    let mut save = save_to_disk(path);
    let written = r.written.clone();
    commands.spawn(Screenshot::primary_window()).observe(move |c: On<ScreenshotCaptured>| {
        save(c);
        written.fetch_add(1, Ordering::Relaxed);
    });
    // the trace: the subject and every drawn body within trace_m of it
    let (from_tick, to_tick, u) = sim.lerp.unwrap_or((0, 0, 1.0));
    let view_tick = from_tick as f64 + f64::from(u) * to_tick.saturating_sub(from_tick) as f64;
    let rate = f64::from(shared.rate()).max(1e-9);
    let speed_x = f64::from(shared.tick_rate()) / rate;
    if !r.speeds.iter().any(|s| (s - speed_x).abs() < 1e-6) {
        r.speeds.push(speed_x);
    }
    r.ticks = (r.ticks.0.or(Some(to_tick)), to_tick);
    let cam = cams.single().ok();
    let screen = |p: Vec3| {
        cam.and_then(|(c, g)| c.world_to_viewport(g, crate::mesh::bv(p.x, p.y, p.z + 1.2)).ok())
            .map_or((f32::NAN, f32::NAN), |v| (v.x, v.y))
    };
    let newer: std::collections::HashMap<u64, &crate::sim::Agent> =
        sim.frames.last().map(|f| f.agents.iter().map(|a| (a.id, a)).collect()).unwrap_or_default();
    let older: std::collections::HashMap<u64, [f32; 2]> =
        sim.frames.first().map(|f| f.agents.iter().map(|a| (a.id, a.t)).collect()).unwrap_or_default();
    let focus = sim.subject_at.map(|(p, _)| p);
    let mut rows: Vec<(&crate::people::BodyDraw, bool)> = vec![];
    if let Some(b) = sim.subject_body.as_ref() {
        rows.push((b, true));
    }
    for b in &sim.people {
        if Some(b.id) != sim.subject && focus.is_some_and(|f| b.at.distance(f) <= trace_m) {
            rows.push((b, false));
        }
    }
    let wall = r.wall0.elapsed().as_secs_f64();
    for (b, is_subject) in rows {
        let a = newer.get(&b.id);
        let (lod, mut clip) = if is_subject {
            ("standin", if b.walking { "walk" } else { "idle" })
        } else {
            people.as_ref().map_or(("none", "none"), |p| p.drawn_state(b.id))
        };
        if b.lying {
            clip = "lying";
        } else if b.lean.abs() > 1e-4 || b.nudge.length_squared() > 1e-8 {
            clip = "fight_pose";
        }
        let clip_rate = if matches!(clip, "walk" | "idle") { 1.0 } else { 0.0 };
        let f = b.feet();
        let (sx, sy) = screen(f);
        let tier = match a.map(|a| a.lod) {
            Some(0) => "Full",
            Some(1) => "Coarse",
            Some(2) => "Stat",
            _ => "",
        };
        let (tx, ty) = a.map_or((f32::NAN, f32::NAN), |a| (a.t[0], a.t[1]));
        let (px, py) = older.get(&b.id).map_or((f32::NAN, f32::NAN), |t| (t[0], t[1]));
        writeln!(
            r.trace,
            "{},{film_t:.4},{wall:.3},{from_tick},{to_tick},{u:.4},{view_tick:.4},{speed_x:.3},{},{},{:.3},{:.3},{:.3},{:.2},{tx},{ty},{px},{py},{},{},{},{},{tier},{},{lod},{clip},{clip_rate},{},{:.3},{sx:.1},{sy:.1}",
            r.n,
            b.id & 0xffff_ffff,
            u8::from(is_subject),
            f.x,
            f.y,
            f.z,
            b.yaw.to_degrees(),
            a.map_or(0.0, |a| a.floor),
            a.map_or(0, |a| u8::from(a.indoors)),
            a.map_or(0, |a| u8::from(a.aboard.is_some())),
            a.map_or(255, |a| a.act),
            u8::from(b.walking),
            u8::from(b.lying),
            b.lean,
        )
        .unwrap();
    }
    if r.n < r.total {
        // the next frame: the clock a step on, Bevy's time with it
        shared.film_advance(dt);
        *strategy = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(dt));
        r.wait0 = Instant::now();
        return;
    }
    // the end: the clock back to the wall's pace (no jump), the meta, the PNGs awaited
    shared.film_end();
    *strategy = TimeUpdateStrategy::Automatic;
    r.trace.flush().ok();
    let res = windows.iter().next().map_or([0.0, 0.0], |w| [w.width(), w.height()]);
    let tile_m = f64::from(sim.tile_m);
    let meta = serde_json::json!({
        "name": r.req.name,
        "clock": "fixed-step",
        "fps": r.req.fps,
        "secs": r.req.secs,
        "frames": r.n,
        "frames_late": r.late,
        "wall_s": r.wall0.elapsed().as_secs_f64(),
        "speed_x": r.speeds,
        "human_speed_x": shared.speed(),
        "rate_1x_ticks_s": rate,
        "walk_m_s": walk_m_s,
        "tile_m": tile_m,
        "tile_x0_m": sim.x0,
        "tile_y1_m": sim.y1,
        "tiles_per_tick": walk_m_s / (tile_m * rate),
        "trace_m": trace_m,
        "save": save_name,
        "subject": subject_ix,
        "subject_key": sim.subject.map(|k| k & 0xffff_ffff),
        "tick_first": r.ticks.0,
        "tick_last": r.ticks.1,
        "clock_start": r.clock0,
        "res": res,
    });
    std::fs::write(r.dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap()).expect("film: meta.json");
    info!("film {}: {} frames captured ({} late) in {:.1} s wall", r.req.name, r.n, r.late, r.wall0.elapsed().as_secs_f64());
    let w = Writing { dir: r.dir.clone(), total: r.n, written: r.written.clone(), since: Instant::now() };
    film.run = None;
    film.writing = Some(w);
}
