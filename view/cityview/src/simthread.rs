//! The follow-cam's sim (the city milestone C2; SIM_VIEW_CONTRACT.md § 2 "Transport"): `--save FILE --follow INDEX`.
//! A thread owns a `citysim::World` loaded from the save, ticks it at the view's rate and, after each tick,
//! publishes the camera ring's `ViewFrame` (`citysim::view::snapshot`) behind an `Arc` into a two-slot buffer; the
//! render thread lerps between the two (sim.rs). Each tick the ring is re-centred on the subject, the sim's view is
//! set to it (`PlayerCommand::SetView`); the sim's camera ring (`citysim::systems::ring`, the view milestone V5,
//! OPEN-149) holds everyone in it Full and a band around it Coarse on its own budget, each tick. The subject is
//! pinned. The ring's counts (in camera Full / Coarse / Stat, held, over budget) go to the HUD's caption and, at a
//! bench's holds, to the log with the sim's ticks/s since the last hold. `CV_RING_DEBUG=1` also logs, at each
//! published frame, the Statistical bodies in the camera (with why the ring left them) and bodies the ring promoted
//! and placed inside their workplace (their schedule's building beside their home and shift).
//!
//! The rate (`--rate`, ticks a real second) defaults to walk-matched (`view::walk_matched_rate`: `[view] walk_m_s`
//! over the `[exec]` pace; 1/60 since Y4's 21 tiles a minute). `--from TICK` runs flat out to that tick first.
//! A bench (`--bench one --shots T1,T2,..`) holds the sim at each shot's tick (`Shared::hold`): the thread runs flat
//! out to it, publishes its last two ticks and waits for the bench to move the hold on.
//!
//! Config hot-reload (the view milestone V1, `OPEN-262`): between two ticks the thread reloads `--config` (default
//! `assets/config.toml`) on the R key (`Shared::reload_req`), when the file's mtime moves (looked at every `[view]
//! reload_poll_s` real seconds) and at `--reload-at TICK`; `citysim::reload` applies the live keys and keeps the
//! build-time ones. The reload's line goes to the log and the HUD (`Shared::reload_line`); the walk-matched rate
//! follows a pace change unless `--rate` pinned it.

use citysim::components::{Brain, Identity};
use citysim::view::{self, ViewFrame};
use citysim::{EntityId, World, save};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One published tick.
pub struct Published {
    pub frame: ViewFrame,
    pub subject: EntityId,
    /// The subject's name and what it is doing, for the HUD.
    pub caption: String,
    /// When it was published (the render thread's lerp clock).
    pub at: Instant,
    pub seq: u64,
}

/// The save's fixed facts: the El lines' names (the map's `L` lines, by index) and the tile -> metre frame.
#[derive(Clone)]
pub struct Static {
    pub lines: Vec<String>,
    pub tile_m: f32,
    pub north_m: f32,
    /// V7: the map's marked crossings (lights.rs draws their signals)
    pub crossings: Vec<citysim::map::MapCrossing>,
}

#[derive(Default)]
pub struct Slots {
    pub prev: Option<Arc<Published>>,
    pub latest: Option<Arc<Published>>,
}

/// What the sim thread and the render thread share.
pub struct Shared {
    pub slots: Mutex<Slots>,
    /// The sim runs no further than this tick (u64::MAX: free running at the rate).
    pub hold: AtomicU64,
    /// The rate in use, f64 bits (walk-matched unless `--rate`), set once the save is loaded.
    pub rate_bits: AtomicU64,
    /// The save's fixed facts, set once it is loaded.
    pub stat: Mutex<Option<Static>>,
    /// The world's tick now (the HUD while it fast-forwards).
    pub tick: AtomicU64,
    /// V1: reload the config at the next tick boundary (the R key).
    pub reload_req: AtomicBool,
    /// V1: the last reload's line (empty before the first), for the HUD.
    pub reload_line: Mutex<String>,
    /// V7: the world's `[crossings]` (the signals' cycle; follows a reload), for lights.rs.
    pub signals: Mutex<Option<citysim::config::CrossingsCfg>>,
    /// V10: a reload's `[view]`, taken by the render thread (sim.rs `clock_light`: the camera's and the light's numbers).
    pub view_cfg: Mutex<Option<citysim::config::ViewCfg>>,
}

impl Shared {
    pub fn rate(&self) -> f32 {
        f64::from_bits(self.rate_bits.load(Ordering::Relaxed)) as f32
    }

    pub fn pair(&self) -> (Option<Arc<Published>>, Option<Arc<Published>>) {
        let s = self.slots.lock().unwrap();
        (s.prev.clone(), s.latest.clone())
    }
}

pub struct SimArgs {
    pub save: PathBuf,
    pub follow: u32,
    pub rate: Option<f64>,
    pub from: Option<u64>,
    pub shots: Vec<u64>,
    /// V1: the config file a reload reads (`--config`; `None`: the default was not found, nothing to reload).
    pub config: Option<PathBuf>,
    /// V1: reload at this tick (`--reload-at`).
    pub reload_at: Option<u64>,
}

/// V5: the frame's bodies per tier (the camera's: Full, Coarse, Statistical, children) and the sim's ring counts.
fn ring_line(world: &World, frame: &ViewFrame) -> String {
    let mut n = [0usize; 4];
    for a in &frame.agents {
        n[match a.lod {
            Some(citysim::Lod::Full) => 0,
            Some(citysim::Lod::Coarse) => 1,
            Some(citysim::Lod::Statistical) => 2,
            None => 3,
        }] += 1;
    }
    let c = &world.ring.counts;
    format!(
        "ring: in camera Full {} Coarse {} Stat {} (child {}); held Full {} Coarse {} (over budget {} / {}); this tick taken {} let go {} moved {} replaced {}, lead {} tiles, {:.1} ms",
        n[0],
        n[1],
        n[2],
        n[3],
        c.full,
        c.coarse,
        c.over_full,
        c.over_coarse,
        c.taken,
        c.released,
        c.moved,
        c.replaced,
        c.lead,
        c.micros as f64 / 1000.0
    )
}

/// The subject's name for the log (the HUD's caption is `view::caption`, from each frame).
fn who(world: &World, e: EntityId) -> String {
    format!("{} #{}", world.name_of(e), e.index)
}

/// V6: the frame's bodies per tier (in camera Full / Coarse / Stat / child), for a recording's HUD line (the
/// recorded world's ring counts are not in the file).
pub fn camera_line(frame: &ViewFrame, view: bool) -> String {
    let mut n = [0usize; 4];
    for a in &frame.agents {
        n[match a.lod {
            Some(citysim::Lod::Full) => 0,
            Some(citysim::Lod::Coarse) => 1,
            Some(citysim::Lod::Statistical) => 2,
            None => 3,
        }] += 1;
    }
    let how = if view { "the shadow's view on the pick" } else { "no view set" };
    format!("ring (recorded: the shadow's world, {how}): in camera Full {} Coarse {} Stat {} (child {})", n[0], n[1], n[2], n[3])
}

impl Shared {
    /// A buffer held at `hold` (`u64::MAX`: free running).
    pub fn new(hold: u64) -> Shared {
        Shared {
            slots: Mutex::new(Slots::default()),
            hold: AtomicU64::new(hold),
            rate_bits: AtomicU64::new(0f64.to_bits()),
            stat: Mutex::new(None),
            tick: AtomicU64::new(0),
            reload_req: AtomicBool::new(false),
            reload_line: Mutex::new(String::new()),
            signals: Mutex::new(None),
            view_cfg: Mutex::new(None),
        }
    }
}

/// V6: the first hold of a shots list: the frame after the first shot's minute (`citysim::view`'s diary rule).
pub fn first_hold(shots: &[u64]) -> u64 {
    shots.first().map_or(u64::MAX, |t| t + 1)
}

/// Load the save and start the thread. A save that fails to load or a subject that is not alive is fatal (exit 2):
/// the tool asserts its preconditions.
pub fn spawn(a: SimArgs) -> Arc<Shared> {
    let shared = Arc::new(Shared::new(first_hold(&a.shots)));
    let sh = shared.clone();
    std::thread::Builder::new().name("sim".into()).spawn(move || run(a, sh)).expect("spawn the sim thread");
    shared
}

/// V1: the config file's mtime (`None`: unreadable now; a reappearing file reads as a change).
fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

pub fn fatal(msg: String) -> ! {
    eprintln!("cityview: {msg}");
    std::process::exit(2);
}

fn run(a: SimArgs, sh: Arc<Shared>) {
    let t0 = Instant::now();
    let mut world = save::load_from_file(&a.save).unwrap_or_else(|e| fatal(format!("--save: {e}")));
    eprintln!("cityview: loaded {} at tick {} ({:.1} s)", a.save.display(), world.tick, t0.elapsed().as_secs_f64());
    let subject = world
        .entities()
        .find(|e| e.index == a.follow)
        .unwrap_or_else(|| fatal(format!("--follow {}: no living entity with that index at tick {}", a.follow, world.tick)));
    let building = world.has::<citysim::Building>(subject);
    if !(world.has::<Identity>(subject) || building) || view::subject_tile(&world, subject).is_none() {
        fatal(format!("--follow {}: {subject:?} is no body with a place nor a building", a.follow));
    }
    if let Some(&t) = a.shots.iter().chain(a.from.iter()).chain(a.reload_at.iter()).find(|&&t| t < world.tick) {
        fatal(format!("tick {t} (--shots / --from / --reload-at) is before the save's tick {}", world.tick));
    }
    // V7: a save from before the crossings walks under the map file's and the --config's (lights.rs)
    crate::lights::graft(&mut world, a.config.as_deref());
    *sh.signals.lock().unwrap() = world.config.crossings.clone();
    let mut rate = a.rate.unwrap_or_else(|| view::walk_matched_rate(&world));
    sh.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
    *sh.stat.lock().unwrap() = Some(Static {
        lines: world.map.lines.iter().map(|l| l.name.clone()).collect(),
        tile_m: world.config.view.tile_m as f32,
        north_m: world.config.view.north_m as f32,
        crossings: world.map.crossings.clone(),
    });
    eprintln!("cityview: following {} at {rate:.4} ticks/s", who(&world, subject));
    if let Some(b) = world.comp_mut::<Brain>(subject) {
        b.pinned = true;
    }
    // V7: the subject's crossings to the log (a shot's tick is found from them)
    world.crossing_watch = Some([(subject, Vec::new())].into());
    let mut cfg = world.config.view.clone();
    let mut watch = a.config.as_deref().map(|p| (mtime(p), Instant::now()));
    let (w, h) = (world.map.w(), world.map.h());
    // V5: ticks run and their wall time since the last hold (the bench's sim ticks/s)
    let mut acc = (0u64, 0f64);
    let mut seq = 0u64;
    // V6: the walkers' ways kept from one published frame to the next (one road search a walk, not a frame)
    let mut ways = view::WayCache::default();
    let from = a.from.unwrap_or(world.tick);
    let mut next_at = Instant::now();
    let mut reloaded_at = false;
    loop {
        // V1: a reload between two ticks: the R key, --reload-at, the file's mtime moved
        let mut why = None;
        if sh.reload_req.swap(false, Ordering::Relaxed) {
            why = Some("the R key");
        } else if a.reload_at == Some(world.tick) && !reloaded_at {
            reloaded_at = true;
            why = Some("--reload-at");
        } else if let (Some(p), Some((seen, at))) = (a.config.as_deref(), watch.as_mut())
            && at.elapsed().as_secs_f64() >= cfg.reload_poll_s
        {
            *at = Instant::now();
            let now = mtime(p);
            if now != *seen {
                *seen = now;
                why = Some("the file changed");
            }
        }
        if let Some(why) = why {
            let line = match a.config.as_deref() {
                None => "reload: no config file (--config PATH; assets/config.toml is not here)".to_string(),
                Some(p) => match std::fs::read_to_string(p) {
                    Err(e) => format!("reload: {}: {e}; the old config stays", p.display()),
                    Ok(text) => match citysim::reload::reload(&mut world, &text) {
                        Err(e) => format!("reload: {}: {e}; the old config stays", p.display()),
                        Ok(r) => {
                            cfg = world.config.view.clone();
                            *sh.signals.lock().unwrap() = world.config.crossings.clone();
                            *sh.view_cfg.lock().unwrap() = Some(cfg.clone());
                            if a.rate.is_none() {
                                rate = view::walk_matched_rate(&world);
                                sh.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
                            }
                            r.line()
                        }
                    },
                },
            };
            eprintln!("cityview: tick {} ({why}): {line}; rate {rate:.4} ticks/s", world.tick);
            *sh.reload_line.lock().unwrap() = line;
        }
        let hold = sh.hold.load(Ordering::Relaxed);
        if world.tick >= hold {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        // paced: free running past --from; flat out to --from and to a bench's hold
        let paced = hold == u64::MAX && world.tick >= from;
        if paced && rate > 0.0 {
            // V1: the wait in slices (a tick is a real minute walk-matched), so a reload is seen between them
            let now = Instant::now();
            if now < next_at {
                std::thread::sleep((next_at - now).min(Duration::from_millis(20)));
                continue;
            }
            next_at = next_at.max(now) + Duration::from_secs_f64(1.0 / rate);
        } else if paced {
            // --rate 0: hold the frame
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let Some(at) = view::subject_tile(&world, subject) else {
            fatal(format!("the subject {subject:?} is gone at tick {}", world.tick))
        };
        let ring = view::ring_about(at, cfg.ring_half_tiles, w, h);
        world.set_view(Some(ring));
        let t_tick = Instant::now();
        citysim::tick(&mut world);
        crate::lights::log_crossings(&mut world, subject);
        acc = (acc.0 + 1, acc.1 + t_tick.elapsed().as_secs_f64());
        sh.tick.store(world.tick, Ordering::Relaxed);
        // publish every paced tick; flat out, only the two ticks before the hold (or --from) the lerp needs
        let target = if hold == u64::MAX { from } else { hold };
        if paced || world.tick + 1 >= target {
            let at = view::subject_tile(&world, subject).unwrap_or(at);
            let ring = view::ring_about(at, cfg.ring_half_tiles, w, h);
            let frame = view::snapshot_many_cached(&world, &[ring], Some(&mut ways)).pop().expect("one ring, one frame");
            let line = ring_line(&world, &frame);
            if std::env::var_os("CV_RING_DEBUG").is_some() {
                // promoted this tick from Statistical: where it stands and what its schedule says
                let fresh = world
                    .ring
                    .held
                    .iter()
                    .filter(|(id, h)| {
                        h.base == citysim::Lod::Statistical
                            && world
                                .comp::<Brain>(**id)
                                .is_some_and(|b| world.comp::<citysim::Position>(**id).is_some_and(|p| p.entered == b.lod_since))
                    })
                    .filter(|(id, _)| {
                        // the ones placed at work (a home placement reads the same way)
                        let work = world.comp::<citysim::components::Job>(**id).and_then(|j| j.employer);
                        work.is_some() && work == world.comp::<citysim::Position>(**id).and_then(|p| p.building)
                    })
                    .take(6)
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>();
                let kind = |b: Option<EntityId>| {
                    b.and_then(|b| world.comp::<citysim::Building>(b).map(|x| format!("{:?}#{} door {:?}", x.kind, b.index, x.door)))
                };
                for id in fresh {
                    let p = world.comp::<citysim::Position>(id);
                    let job = world.comp::<citysim::components::Job>(id);
                    eprintln!(
                        "ring-promoted (lod since {:?}) {} {:?} at {:?} in {:?} floor {:?}; home {:?}; work {:?} on shift {:?}; schedule {:?}",
                        world.comp::<Brain>(id).map(|b| b.lod_since),
                        world.name_of(id),
                        id,
                        p.map(|p| p.tile),
                        kind(p.and_then(|p| p.building)),
                        p.map(|p| p.floor),
                        kind(world.comp::<citysim::components::Household>(id).and_then(|h| h.home)),
                        kind(job.and_then(|j| j.employer)),
                        job.map(|j| j.shifts.clone()),
                        kind(citysim::systems::ring::schedule_door(&world, id))
                    );
                }
                for ag in frame.agents.iter().filter(|a| a.lod == Some(citysim::Lod::Statistical)).take(8) {
                    let p = world.comp::<citysim::Position>(ag.id);
                    eprintln!(
                        "ring-debug {:?} t {:?} pos {:?} held {:?} sentence {} sched {:?} brain lod_since {:?}",
                        ag.id,
                        ag.t,
                        p.map(|p| (p.tile, p.building)),
                        world.ring.held.get(&ag.id),
                        world.has::<citysim::components::Sentence>(ag.id),
                        citysim::systems::ring::schedule_door(&world, ag.id)
                            .map(|b| (b, world.comp::<citysim::Building>(b).map(|x| x.door))),
                        world.comp::<Brain>(ag.id).map(|b| b.lod_since)
                    );
                }
            }
            if !paced {
                let (n, secs) = acc;
                let tps = n as f64 / secs.max(1e-9);
                eprintln!(
                    "cityview: tick {} subject at ({}, {}); {line}; sim {} ticks in {secs:.1} s ({tps:.1} ticks/s)",
                    world.tick, at.x, at.y, n
                );
                if world.tick >= target {
                    acc = (0, 0.0);
                }
            }
            seq += 1;
            let caption = format!("{}\n{line}", view::caption(&world, &frame, subject));
            let p = Arc::new(Published { frame, subject, caption, at: Instant::now(), seq });
            let mut s = sh.slots.lock().unwrap();
            s.prev = s.latest.take();
            s.latest = Some(p);
            next_at = next_at.max(Instant::now());
        }
    }
}
