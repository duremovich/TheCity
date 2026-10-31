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

use citysim::components::{Brain, Identity, Rect, TilePos};
use citysim::view::{self, ViewFrame};
use citysim::{EntityId, World, save};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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
}

/// The ring grown by `m` tiles on every side (clipped at the map's top-left; the far edge is past the map anyway).
fn grow(r: Rect, m: u16) -> Rect {
    let (x, y) = (r.x.saturating_sub(m), r.y.saturating_sub(m));
    Rect { x, y, w: r.w.saturating_add(r.x - x).saturating_add(m), h: r.h.saturating_add(r.y - y).saturating_add(m) }
}

/// The subject's place: its vehicle's, else its walk's (a Coarse walker on his way), else its `Position`.
fn subject_tile(world: &World, e: EntityId) -> Option<TilePos> {
    if let Some(v) = citysim::systems::motion::vehicle_of(world, e)
        && let Some(m) = world.motions.get(&v)
    {
        return Some(m.pos);
    }
    citysim::exec::walk_place(world, e).map(|p| p.tile)
}

fn caption(world: &World, e: EntityId) -> String {
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
    });
    let sh = shared.clone();
    std::thread::Builder::new().name("sim".into()).spawn(move || run(a, sh)).expect("spawn the sim thread");
    shared
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
    if !world.has::<Identity>(subject) || subject_tile(&world, subject).is_none() {
        fatal(format!("--follow {}: {subject:?} is no body with a place", a.follow));
    }
    if let Some(&t) = a.shots.iter().chain(a.from.iter()).find(|&&t| t < world.tick) {
        fatal(format!("tick {t} (--shots / --from) is before the save's tick {}", world.tick));
    }
    let rate = a.rate.unwrap_or_else(|| view::walk_matched_rate(&world));
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
    let cfg = world.config.view.clone();
    let (w, h) = (world.map.w(), world.map.h());
    let mut pinned = BTreeSet::new();
    let mut seq = 0u64;
    let from = a.from.unwrap_or(world.tick);
    let mut next_at = Instant::now();
    loop {
        let hold = sh.hold.load(Ordering::Relaxed);
        if world.tick >= hold {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        // paced: free running past --from; flat out to --from and to a bench's hold
        let paced = hold == u64::MAX && world.tick >= from;
        if paced && rate > 0.0 {
            let now = Instant::now();
            if now < next_at {
                std::thread::sleep(next_at - now);
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
