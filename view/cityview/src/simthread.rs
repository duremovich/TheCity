//! The follow-cam's sim (the city milestone C2; SIM_VIEW_CONTRACT.md § 2 "Transport"): `--save FILE --follow INDEX`.
//! A thread owns a `citysim::World` loaded from the save, ticks it at the view's rate and, after each tick,
//! publishes the camera ring's `ViewFrame` (`citysim::view::snapshot`) behind an `Arc` into a two-slot buffer; the
//! render thread lerps between the two (sim.rs). Each tick the ring is re-centred on the subject, the sim's view is
//! set to it (`PlayerCommand::SetView`) and every body standing in it is pinned (`view::pin_ring`: OPEN-149's
//! stopgap, as simsnap's; the ring budget and promotion before visibility stay the view milestone's).
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

use citysim::components::{Brain, Identity, Rect, TilePos};
use citysim::view::{self, ViewFrame};
use citysim::{EntityId, World, save};
use std::collections::BTreeSet;
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

/// The ring grown by `m` tiles on every side (clipped at the map's top-left; the far edge is past the map anyway).
fn grow(r: Rect, m: u16) -> Rect {
    let (x, y) = (r.x.saturating_sub(m), r.y.saturating_sub(m));
    Rect { x, y, w: r.w.saturating_add(r.x - x).saturating_add(m), h: r.h.saturating_add(r.y - y).saturating_add(m) }
}

/// The subject's place: its vehicle's, else its walk's (a Coarse walker on his way), else its `Position`.
fn subject_tile(world: &World, e: EntityId) -> Option<TilePos> {
    // C6b: a building as the subject (a shop-house's units): the camera stands at its door
    if let Some(b) = world.comp::<citysim::Building>(e) {
        return Some(b.door);
    }
    if let Some(v) = citysim::systems::motion::vehicle_of(world, e)
        && let Some(m) = world.motions.get(&v)
    {
        return Some(m.pos);
    }
    citysim::exec::walk_place(world, e).map(|p| p.tile)
}

fn caption(world: &World, e: EntityId) -> String {
    if world.has::<citysim::Building>(e) {
        return format!("{} (its door)", world.name_of(e));
    }
    let name = world.comp::<Identity>(e).map_or("?".into(), |i| i.name.clone());
    let b = world.comp::<Brain>(e);
    let goal = b.and_then(|b| b.current_goal).map_or("-".into(), |g| format!("{g:?}"));
    let step = b.and_then(|b| b.current_step()).map_or("-".into(), |s| format!("{:?}", s.action));
    let lod = b.map_or("child".into(), |b| format!("{:?}", b.lod));
    let at = match world.comp::<citysim::Position>(e).and_then(|p| p.building) {
        Some(bd) => world.comp::<citysim::Building>(bd).map_or(format!("#{}", bd.index), |b| format!("{:?}#{}", b.kind, bd.index)),
        None => "street".into(),
    };
    format!("{name} #{} ({lod}): {goal} / {step} @ {at}", e.index)
}

/// Load the save and start the thread. A save that fails to load or a subject that is not alive is fatal (exit 2):
/// the tool asserts its preconditions.
pub fn spawn(a: SimArgs) -> Arc<Shared> {
    let shared = Arc::new(Shared {
        slots: Mutex::new(Slots::default()),
        hold: AtomicU64::new(if a.shots.is_empty() { u64::MAX } else { a.shots[0] }),
        rate_bits: AtomicU64::new(0f64.to_bits()),
        stat: Mutex::new(None),
        tick: AtomicU64::new(0),
        reload_req: AtomicBool::new(false),
        reload_line: Mutex::new(String::new()),
    });
    let sh = shared.clone();
    std::thread::Builder::new().name("sim".into()).spawn(move || run(a, sh)).expect("spawn the sim thread");
    shared
}

/// V1: the config file's mtime (`None`: unreadable now; a reappearing file reads as a change).
fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn fatal(msg: String) -> ! {
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
    if !(world.has::<Identity>(subject) || building) || subject_tile(&world, subject).is_none() {
        fatal(format!("--follow {}: {subject:?} is no body with a place nor a building", a.follow));
    }
    if let Some(&t) = a.shots.iter().chain(a.from.iter()).chain(a.reload_at.iter()).find(|&&t| t < world.tick) {
        fatal(format!("tick {t} (--shots / --from / --reload-at) is before the save's tick {}", world.tick));
    }
    let mut rate = a.rate.unwrap_or_else(|| view::walk_matched_rate(&world));
    sh.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
    *sh.stat.lock().unwrap() = Some(Static {
        lines: world.map.lines.iter().map(|l| l.name.clone()).collect(),
        tile_m: world.config.view.tile_m as f32,
        north_m: world.config.view.north_m as f32,
    });
    eprintln!("cityview: following {} at {rate:.4} ticks/s", caption(&world, subject));
    if let Some(b) = world.comp_mut::<Brain>(subject) {
        b.pinned = true;
    }
    let mut cfg = world.config.view.clone();
    let mut watch = a.config.as_deref().map(|p| (mtime(p), Instant::now()));
    let (w, h) = (world.map.w(), world.map.h());
    let mut pinned = BTreeSet::new();
    let mut seq = 0u64;
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
        let Some(at) = subject_tile(&world, subject) else { fatal(format!("the subject {subject:?} is gone at tick {}", world.tick)) };
        let ring = view::ring_about(at, cfg.ring_half_tiles, w, h);
        world.set_view(Some(ring));
        view::pin_ring(&mut world, ring, grow(ring, cfg.pin_margin_tiles), &mut pinned);
        citysim::tick(&mut world);
        sh.tick.store(world.tick, Ordering::Relaxed);
        // publish every paced tick; flat out, only the two ticks before the hold (or --from) the lerp needs
        let target = if hold == u64::MAX { from } else { hold };
        if paced || world.tick + 1 >= target {
            let at = subject_tile(&world, subject).unwrap_or(at);
            if !paced {
                eprintln!("cityview: tick {} subject at ({}, {})", world.tick, at.x, at.y);
            }
            let frame = view::snapshot(&world, view::ring_about(at, cfg.ring_half_tiles, w, h));
            seq += 1;
            let p = Arc::new(Published { frame, subject, caption: caption(&world, subject), at: Instant::now(), seq });
            let mut s = sh.slots.lock().unwrap();
            s.prev = s.latest.take();
            s.latest = Some(p);
            next_at = next_at.max(Instant::now());
        }
    }
}
