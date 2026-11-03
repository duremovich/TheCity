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
//!
//! M18 P1 (the seam, docs/M18_PLAYER.md): `--possess INDEX` drives the subject. The render thread has no world, so
//! its orders (`UiCmd`: a click's tile, a goal, stop, autopilot) go into `Shared::cmds`; the thread drains them
//! between two ticks into the world's command queue as `PlayerCommand`s (applied at the next tick's start, like the
//! R key's `reload_req`), and publishes the body's HUD state (`Shared::player`) with each frame. Pause and the play
//! speed (a multiple of the rate, `[view] play_speeds`) are atomics read at the pacing.

use citysim::components::{Brain, GoalKind, Identity, TilePos};
use citysim::systems::player::{self as pl, Verb};
use citysim::systems::player_news as pl_news;
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
    /// M18 P1: the render thread's orders for the possessed body, drained between two ticks.
    pub cmds: Mutex<Vec<UiCmd>>,
    /// M18 P1: the clock stopped (Space); the play speed, f64 bits, a multiple of the rate.
    pub paused: AtomicBool,
    pub speed_bits: AtomicU64,
    /// M18 P1: the possessed body's state for the HUD (`None`: nobody possessed).
    pub player: Mutex<Option<PlayerHud>>,
    /// M18 P2: the last click's menu (`None`: none asked yet; the render thread closes it on its side).
    pub menu: Mutex<Option<MenuPub>>,
    /// M18 P4: what the city knows and does about the possessed body (the log, who knows, who is after it, the
    /// day-end summaries, the death).
    pub news: Mutex<NewsPub>,
}

/// M18 P4: the player's panels (`citysim::systems::player_news`, read at `[view] news_scan_minutes`; the log from
/// the body's events as they happen).
#[derive(Clone, Debug, Default)]
pub struct NewsPub {
    /// Rises with each change.
    pub seq: u64,
    /// The player's own lines, oldest first (`[view] news_log_lines`): (important, "dD HH:MM text").
    pub log: Vec<(bool, String)>,
    /// Who knows about you: (name, what, how and when).
    pub knowers: Vec<(String, String, String)>,
    /// Knowers of a deed of yours, of anything.
    pub knowing: (usize, usize),
    /// Who is after you.
    pub after: Vec<String>,
    /// The district talk naming you.
    pub talk: Vec<String>,
    /// The street's read (the daily reputation).
    pub rep: String,
    /// The day-end summaries, newest last: (day, lines).
    pub days: Vec<(u64, Vec<String>)>,
    /// The end: the line and the life, newest first.
    pub death: Option<(String, Vec<String>)>,
}

/// M18 P1: an order from the render thread (a click, a key, a HUD button, a `--play` line).
#[derive(Clone, Debug)]
pub enum UiCmd {
    /// Walk there: into the building that holds the tile (a roof or a wall hit, `true`: the nearest within `[view]
    /// pick_snap_tiles`), else to the street tile.
    Click(TilePos, bool),
    Goal(GoalKind),
    Stop,
    Autopilot(bool),
    /// M18 P2: a click on a body (its entity): the people menu.
    Body(EntityId),
    /// M18 P2: a menu row picked.
    Act(Verb),
    /// M18 P3: a building's menu by its entity (a `--play` line's `bld INDEX`: the menu a click on it opens).
    Bld(EntityId),
}

/// A menu row for the render thread: the label, the verb, the will terms it waives (`Verb::waived`, the hover), and
/// (M18 P3) why it cannot be done now (`Some`: drawn greyed, the reason its hover).
pub type MenuRow = (String, Verb, Vec<String>, Option<String>);

/// M18 P2: a menu the sim built for a click (a building's or a body's), only its legal rows, for the render thread.
#[derive(Clone, Debug, Default)]
pub struct MenuPub {
    /// Rises with each menu (the render thread places a new one at the click).
    pub seq: u64,
    pub title: String,
    /// The row's label, its verb, the will terms it waives (`Verb::waived`, the row's hover).
    pub rows: Vec<MenuRow>,
}

/// M18 P1: what the HUD shows of the possessed body.
#[derive(Clone, Debug, Default)]
pub struct PlayerHud {
    pub name: String,
    pub index: u32,
    pub day: u64,
    pub clock: String,
    pub autopilot: bool,
    /// hunger, energy, safety, wealth, belonging (1 = met)
    pub needs: [f32; 5],
    pub coins: i64,
    pub job: String,
    pub place: String,
    pub goal: String,
    pub plan: String,
    /// The last failure or refusal: its clock and line.
    pub failure: Option<String>,
    /// M18 P2: the menu row the plan came from (`PlayerControlled::doing`).
    pub doing: Option<String>,
    /// M18 P2: the HUD's own rows (`player::menu_self`: eat, sleep, work where the body stands), only the legal ones.
    pub rows: Vec<MenuRow>,
    /// M18 P3: the board (`player::menu_board`: the job held, the records the body sees, the Fixers to get known
    /// at; the ones it cannot take greyed) and the city's offers (`player::menu_offers`).
    pub board: Vec<MenuRow>,
    pub offers: Vec<MenuRow>,
    /// M18 P2: carried food.
    pub food: u32,
    /// M18 P4: why the body cannot be ordered now, when held ("in cuffs", a sentence's days left).
    pub held: Option<String>,
}

impl Shared {
    /// M18 P1: an order for the possessed body, applied at the next tick.
    pub fn send(&self, c: UiCmd) {
        self.cmds.lock().unwrap().push(c);
    }

    pub fn speed(&self) -> f64 {
        f64::from_bits(self.speed_bits.load(Ordering::Relaxed))
    }

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
    /// M18 P1: possess the subject (`--possess`) once the world reaches `--from` (the save's tick without it).
    pub possess: bool,
    /// M18 P1: the first play speed (`[view] play_speeds[0]`).
    pub speed: f64,
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
            cmds: Mutex::new(Vec::new()),
            paused: AtomicBool::new(false),
            speed_bits: AtomicU64::new(1f64.to_bits()),
            player: Mutex::new(None),
            menu: Mutex::new(None),
            news: Mutex::new(NewsPub::default()),
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
    shared.speed_bits.store(a.speed.to_bits(), Ordering::Relaxed);
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
    // M18 P1: nobody under 18 is ever possessed (CLAUDE.md, 2026-10-27; the sim refuses it too)
    if a.possess && !citysim::systems::demography::is_adult(&world, subject) {
        fatal(format!("--possess {}: {} is no adult (nobody under 18 can be possessed)", a.follow, who(&world, subject)));
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
    let mut possessed = false;
    let mut last_logged = (String::new(), None, false);
    let mut menu_seq = 0u64;
    let mut last_event = world.events.back().map_or(0, |e| e.id);
    // M18 P4: the player's panels (read from the possession on)
    let mut news: Option<News> = None;
    let mut last_tile = view::subject_tile(&world, subject);
    loop {
        // M18 P1: possessed at --from (the city pre-run to it); the render thread's orders, between two ticks
        if a.possess && !possessed && world.tick >= from {
            possessed = true;
            world.push_command(citysim::PlayerCommand::Possess(subject, true));
            eprintln!("cityview: tick {} possess {}", world.tick, who(&world, subject));
        }
        let cmds = std::mem::take(&mut *sh.cmds.lock().unwrap());
        for c in cmds {
            let cmd = match c {
                // M18 P2: a click on a building (a roof, a wall, a floor in the cutaway) opens its menu; the
                // street walks there (P1's walk, never into a building: the doors are the menu's)
                UiCmd::Click(t, roof) => {
                    let snap = world.config.view.pick_snap_tiles;
                    if let Some(m) = pl::menu_at(&world, subject, t, roof, snap) {
                        publish_menu(&sh, &world, m, &mut menu_seq);
                        continue;
                    }
                    let steps = pl::walk_to(&world, t, false, snap);
                    eprintln!("cityview: tick {} click ({}, {}): walk to the street", world.tick, t.x, t.y);
                    citysim::PlayerCommand::Steps { agent: subject, steps }
                }
                UiCmd::Bld(b) => {
                    let door = world.comp::<citysim::Building>(b).map(|x| x.door);
                    let snap = world.config.view.pick_snap_tiles;
                    if let Some(m) = door.and_then(|t| pl::menu_at(&world, subject, t, true, snap)) {
                        publish_menu(&sh, &world, m, &mut menu_seq);
                    }
                    continue;
                }
                UiCmd::Body(o) => {
                    publish_menu(&sh, &world, pl::menu_body(&world, subject, o), &mut menu_seq);
                    continue;
                }
                UiCmd::Act(v) => citysim::PlayerCommand::Act { agent: subject, verb: v },
                UiCmd::Goal(g) => citysim::PlayerCommand::Goal { agent: subject, goal: g },
                UiCmd::Stop => citysim::PlayerCommand::Stop(subject),
                UiCmd::Autopilot(on) => citysim::PlayerCommand::SetAutopilot(subject, on),
            };
            eprintln!("cityview: tick {} order {cmd:?}", world.tick);
            world.push_command(cmd);
        }
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
        // M18 P1: Space stops the clock (paced play only; a bench's run to its hold goes on)
        if hold == u64::MAX && world.tick >= from && sh.paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(20));
            next_at = Instant::now();
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
            next_at = next_at.max(now) + Duration::from_secs_f64(1.0 / (rate * sh.speed().max(1e-6)));
        } else if paced {
            // --rate 0: hold the frame
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        // M18 P4: a dead player's corpse can go (buried, rotted): the camera stays where it fell
        let dead = news.as_ref().is_some_and(|n| n.pubd.death.is_some());
        let Some(at) = view::subject_tile(&world, subject).or(last_tile.filter(|_| dead)) else {
            fatal(format!("the subject {subject:?} is gone at tick {}", world.tick))
        };
        last_tile = Some(at);
        let ring = view::ring_about(at, cfg.ring_half_tiles, w, h);
        world.set_view(Some(ring));
        let t_tick = Instant::now();
        citysim::tick(&mut world);
        crate::lights::log_crossings(&mut world, subject);
        // M18 P2: the possessed body's events to the log (the play session's record: a hire, a theft, an arrest)
        if possessed {
            let n = news.get_or_insert_with(|| News::new(&world, subject));
            let new: Vec<&citysim::events::Event> = world.events.iter().rev().take_while(|e| e.id > last_event).collect();
            for e in new.iter().rev() {
                if e.actors.contains(&subject) {
                    let (d, c) = (e.tick / citysim::TICKS_PER_DAY, citysim::time::clock(e.tick));
                    eprintln!("event d{d} {c}: [{:?}] {}", e.kind, e.text);
                    n.event(&world, e);
                } else {
                    n.block_event(&world, e);
                }
            }
            // M18 P4: who knows, who is after you, the day's end, the death
            n.step(&world);
            if n.pubd.death.is_some() && !n.dead_paused {
                n.dead_paused = true;
                sh.paused.store(true, Ordering::Relaxed);
            }
            if n.pubd.seq != n.sent {
                n.sent = n.pubd.seq;
                *sh.news.lock().unwrap() = n.pubd.clone();
            }
        }
        last_event = world.events.back().map_or(last_event, |e| e.id);
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
            if possessed {
                let hud = player_hud(&world, subject);
                // the log's record of the body's day: each new plan, failure and autopilot switch
                if let (Some(line), Some(n)) = (log_player(&hud, &mut last_logged), news.as_mut()) {
                    n.push(&world, false, line);
                    n.sent = n.pubd.seq;
                    *sh.news.lock().unwrap() = n.pubd.clone();
                }
                *sh.player.lock().unwrap() = Some(hud);
            }
            let p = Arc::new(Published { frame, subject, caption, at: Instant::now(), seq });
            let mut s = sh.slots.lock().unwrap();
            s.prev = s.latest.take();
            s.latest = Some(p);
            next_at = next_at.max(Instant::now());
        }
    }
}

/// M18 P1: the possessed body's HUD state.
fn player_hud(world: &World, id: EntityId) -> PlayerHud {
    use citysim::components::{Job, Needs, Wallet};
    let b = world.comp::<Brain>(id);
    let n = world.comp::<Needs>(id);
    let job = world.comp::<Job>(id).map_or("no job".to_string(), |j| {
        let at = j.employer.map_or("-".to_string(), |e| world.name_of(e));
        let shifts: Vec<String> =
            j.shifts.iter().map(|(a, b)| format!("{:02}:{:02}-{:02}:{:02}", a / 60, a % 60, b / 60, b % 60)).collect();
        format!("{:?} at {at}, {}c a day, {}", j.role, j.wage_per_day, shifts.join(" "))
    });
    let place = world.comp::<citysim::Position>(id).map_or("-".to_string(), |p| match p.building {
        Some(b) => format!("in {} floor {}", world.name_of(b), p.floor),
        None => format!("on the street at ({}, {})", p.tile.x, p.tile.y),
    });
    let pc = b.and_then(|b| b.player.as_ref());
    PlayerHud {
        name: world.name_of(id),
        index: id.index,
        day: world.tick / citysim::TICKS_PER_DAY,
        clock: citysim::time::clock(world.tick),
        autopilot: pc.is_some_and(|p| p.autopilot),
        needs: n.map_or([0.0; 5], |n| [n.hunger, n.energy, n.safety, n.wealth, n.belonging]),
        coins: world.comp::<Wallet>(id).map_or(0, |w| w.coins),
        job,
        place,
        goal: b.and_then(|b| b.current_goal).map_or("-".to_string(), |g| format!("{g:?}")),
        plan: citysim::systems::player::plan_line(world, id),
        failure: pc
            .and_then(|p| p.last_failure.as_ref())
            .map(|(t, l)| format!("d{} {}: {l}", t / citysim::TICKS_PER_DAY, citysim::time::clock(*t))),
        doing: pc.and_then(|p| p.doing.clone()),
        rows: menu_rows(&pl::menu_self(world, id)),
        board: menu_rows(&pl::menu_board(world, id)),
        offers: menu_rows(&pl::menu_offers(world, id)),
        food: world.comp::<citysim::components::Inventory>(id).map_or(0, |i| i.food),
        held: held_line(world, id),
    }
}

/// M18 P4: why the body cannot be ordered now (`think::held`), a sentence with its days left (P2's "in cuffs / N
/// days").
fn held_line(world: &World, id: EntityId) -> Option<String> {
    let why = citysim::systems::think::held(world, id)?;
    Some(match world.comp::<citysim::components::Sentence>(id) {
        Some(s) => {
            let left = s.until_tick.saturating_sub(world.tick);
            let (d, h) = (left / citysim::TICKS_PER_DAY, (left % citysim::TICKS_PER_DAY) / 60);
            format!("{why} ({:?}): {d} days {h} h left", s.crime)
        }
        None => why.to_string(),
    })
}

fn menu_rows(m: &pl::Menu) -> Vec<MenuRow> {
    m.rows.iter().map(|r| (r.label.clone(), r.verb, r.verb.waived().iter().map(|s| s.to_string()).collect(), r.why_not.clone())).collect()
}

/// M18 P2: a click's menu to the render thread, and to the log (the play session's record).
fn publish_menu(sh: &Shared, world: &World, m: pl::Menu, seq: &mut u64) {
    *seq += 1;
    let rows = menu_rows(&m);
    let labels: Vec<String> = rows.iter().map(|r| r.3.as_ref().map_or(r.0.clone(), |w| format!("({} -- {w})", r.0))).collect();
    eprintln!(
        "player d{} {}: menu {}: [{}]",
        world.tick / citysim::TICKS_PER_DAY,
        citysim::time::clock(world.tick),
        m.title,
        labels.join(" | ")
    );
    *sh.menu.lock().unwrap() = Some(MenuPub { seq: *seq, title: m.title, rows });
}

/// M18 P1: the possessed body's plan, failures and autopilot to the log when they change (the play session's record);
/// M18 P4: a new failure is the player's own log line too (returned).
fn log_player(hud: &PlayerHud, last: &mut (String, Option<String>, bool)) -> Option<String> {
    let mut out = None;
    let at = format!("d{} {}", hud.day, hud.clock);
    if hud.plan != last.0 {
        eprintln!(
            "player {at}: plan {}{} | {} | coins {} food {} hunger {:.2} energy {:.2} | {}",
            hud.plan,
            hud.doing.as_ref().map_or(String::new(), |d| format!(" ({d})")),
            hud.place,
            hud.coins,
            hud.food,
            hud.needs[0],
            hud.needs[1],
            hud.job
        );
        last.0 = hud.plan.clone();
    }
    if hud.failure != last.1 {
        if let Some(f) = &hud.failure {
            eprintln!("player {at}: failed {f}");
            out = Some(format!("failed: {f}"));
        }
        last.1 = hud.failure.clone();
    }
    if hud.autopilot != last.2 {
        eprintln!("player {at}: autopilot {}", if hud.autopilot { "on" } else { "off" });
        last.2 = hud.autopilot;
    }
    out
}

/// M18 P4: the event kinds that make "your block" at the day's end (what happened to the neighbours).
const BLOCK_KINDS: &[citysim::events::EventKind] = {
    use citysim::events::EventKind as K;
    &[
        K::Murder,
        K::Death,
        K::Assault,
        K::Theft,
        K::Robbed,
        K::Extortion,
        K::Arrest,
        K::Evicted,
        K::Housed,
        K::Founded,
        K::Bankrupt,
        K::Raid,
        K::Riot,
        K::Looted,
        K::Crossfire,
        K::Squatted,
        K::HuntStarted,
        K::Avenged,
        K::Birth,
        K::LaidOff,
        K::Fire,
    ]
};

/// M18 P4: the sim thread's side of the player's panels.
struct News {
    subject: EntityId,
    pubd: NewsPub,
    /// The next scan's tick, and the day the last day-end closed.
    next_scan: u64,
    day: u64,
    /// The day's start: coins; the knowers then (index set); the block's lines today.
    day_coins: i64,
    day_knowers: std::collections::BTreeSet<u32>,
    block: Vec<String>,
    /// The last scan's "after" keys and deed-knower count (the log's diff).
    after_keys: std::collections::BTreeMap<(u8, u64), String>,
    deed_knowers: usize,
    /// The deed knowers (index) at the last "word travels" line, and its tick.
    knowers: std::collections::BTreeSet<u32>,
    word_at: u64,
    dead_paused: bool,
    /// The last seq sent to the render thread.
    sent: u64,
}

fn stamp(tick: u64) -> String {
    format!("d{} {}", tick / citysim::TICKS_PER_DAY, citysim::time::clock(tick))
}

impl News {
    fn new(world: &World, subject: EntityId) -> News {
        let k = pl_news::known(world, subject, usize::MAX);
        let knowers: std::collections::BTreeSet<u32> = k.knowers.iter().map(|x| x.who.index).collect();
        let mut n = News {
            subject,
            pubd: NewsPub::default(),
            next_scan: world.tick,
            day: world.tick / citysim::TICKS_PER_DAY,
            day_coins: coins(world, subject),
            day_knowers: knowers.clone(),
            block: Vec::new(),
            after_keys: k.after.iter().map(|a| (a.key, a.line.clone())).collect(),
            deed_knowers: k.knowers.iter().filter(|x| x.deed).count(),
            knowers,
            word_at: 0,
            dead_paused: false,
            sent: u64::MAX,
        };
        n.fill(world, &k);
        n
    }

    /// A line in the player's own log (to the stderr log too when it is the panels' own).
    fn push(&mut self, world: &World, important: bool, line: String) {
        let cap = world.config.view.news_log_lines.max(1);
        self.pubd.log.push((important, format!("{} {line}", stamp(world.tick))));
        if self.pubd.log.len() > cap {
            let cut = self.pubd.log.len() - cap;
            self.pubd.log.drain(..cut);
        }
        self.pubd.seq += 1;
    }

    /// The body's own event: its line (the plan's chatter, "the job goes on", stays in the stderr log only).
    fn event(&mut self, world: &World, e: &citysim::events::Event) {
        use citysim::events::EventKind as K;
        if e.kind == K::PlanAborted || (e.kind == K::PlayerAction && e.text.contains(" goes on [")) {
            return;
        }
        let important = !matches!(e.kind, K::PlayerAction | K::PlayerActionFailed);
        let line = format!("[{:?}] {}", e.kind, e.text);
        let cap = world.config.view.news_log_lines.max(1);
        self.pubd.log.push((important, format!("{} {line}", stamp(e.tick))));
        if self.pubd.log.len() > cap {
            let cut = self.pubd.log.len() - cap;
            self.pubd.log.drain(..cut);
        }
        self.pubd.seq += 1;
    }

    /// Another body's event: kept for the day-end's "your block" when it happened within `[view] block_tiles` of
    /// the player's home door (where the body stands, homeless).
    fn block_event(&mut self, world: &World, e: &citysim::events::Event) {
        // in the cells the block is the yard's, not the player's street
        if !BLOCK_KINDS.contains(&e.kind) || self.block.len() >= 400 || world.has::<citysim::components::Sentence>(self.subject) {
            return;
        }
        let home = world
            .comp::<citysim::components::Household>(self.subject)
            .and_then(|h| h.home)
            .and_then(|b| world.comp::<citysim::Building>(b).map(|x| x.door));
        let Some(anchor) = home.or_else(|| view::subject_tile(world, self.subject)) else { return };
        let r = u32::from(world.config.view.block_tiles);
        let near = e.actors.iter().filter(|a| **a != EntityId::NONE).any(|&a| {
            let t = world.comp::<citysim::Building>(a).map(|b| b.door).or_else(|| world.comp::<citysim::Position>(a).map(|p| p.tile));
            t.is_some_and(|t| u32::from(t.x.abs_diff(anchor.x)).max(u32::from(t.y.abs_diff(anchor.y))) <= r)
        });
        if near {
            self.block.push(format!("{} [{:?}] {}", citysim::time::clock(e.tick), e.kind, e.text));
        }
    }

    /// The scan (at `[view] news_scan_minutes`), the day's end and the death; `true` when the panels changed.
    fn step(&mut self, world: &World) -> bool {
        let mut changed = false;
        if self.pubd.death.is_none() {
            if let Some(d) = pl_news::death(world, self.subject) {
                eprintln!("death {}: {}", stamp(world.tick), d.line);
                for l in d.life.iter().take(12) {
                    eprintln!("death life: {l}");
                }
                self.push(world, true, format!("YOU DIED: {}", d.line));
                self.pubd.death = Some((d.line, d.life));
                return true;
            }
        } else {
            return false;
        }
        let day = world.tick / citysim::TICKS_PER_DAY;
        if world.tick >= self.next_scan || day != self.day {
            self.next_scan = world.tick + u64::from(world.config.view.news_scan_minutes.max(1));
            let k = pl_news::known(world, self.subject, usize::MAX);
            self.diff(world, &k);
            if day != self.day {
                self.day_end(world, &k);
                self.day = day;
            }
            self.fill(world, &k);
            changed = true;
        }
        changed
    }

    /// The log's lines for what changed since the last scan: a grudge, a Hunt or a record on you formed or ended;
    /// word of a deed of yours reached more people.
    fn diff(&mut self, world: &World, k: &pl_news::Known) {
        let now: std::collections::BTreeMap<(u8, u64), String> = k.after.iter().map(|a| (a.key, a.line.clone())).collect();
        for (key, line) in &now {
            if !self.after_keys.contains_key(key) {
                eprintln!("news {}: !! {line}", stamp(world.tick));
                self.push(world, true, format!("!! {line}"));
            }
        }
        for (key, line) in &self.after_keys.clone() {
            if !now.contains_key(key) {
                eprintln!("news {}: over: {line}", stamp(world.tick));
                self.push(world, true, format!("over: {line}"));
            }
        }
        self.after_keys = now;
        // word travels: the deed knowers new since the last such line, at most once in `[view] news_word_minutes`
        let deeds: Vec<&pl_news::Knower> = k.knowers.iter().filter(|x| x.deed).collect();
        let fresh: Vec<&&pl_news::Knower> = deeds.iter().filter(|x| !self.knowers.contains(&x.who.index)).collect();
        let due = self.word_at == 0 || world.tick >= self.word_at + u64::from(world.config.view.news_word_minutes);
        if due && deeds.len() > self.deed_knowers && !fresh.is_empty() {
            let names: Vec<String> = fresh.iter().take(3).map(|x| format!("{} ({}: {})", x.name, x.line, x.how.label())).collect();
            let line = format!("word travels: {} now know a deed of yours (+{}): {}", deeds.len(), fresh.len(), names.join("; "));
            eprintln!("news {}: {line}", stamp(world.tick));
            self.push(world, true, line);
            self.word_at = world.tick;
            self.deed_knowers = deeds.len();
            self.knowers = deeds.iter().map(|x| x.who.index).collect();
        } else if deeds.len() < self.deed_knowers {
            // forgetting: the base follows the count down, so the next line counts from here
            self.deed_knowers = deeds.len();
        }
    }

    /// The day-end summary: coins, who now knows you, who is after you, the street's read, your block.
    fn day_end(&mut self, world: &World, k: &pl_news::Known) {
        let mut lines = Vec::new();
        let c = coins(world, self.subject);
        lines.push(format!("coins {} -> {c} ({:+})", self.day_coins, c - self.day_coins));
        let new: Vec<&pl_news::Knower> = k.knowers.iter().filter(|x| !self.day_knowers.contains(&x.who.index)).collect();
        lines.push(format!(
            "who knows you: {} ({} of a deed of yours); {} new today",
            k.knowers.len(),
            k.knowers.iter().filter(|x| x.deed).count(),
            new.len()
        ));
        for x in new.iter().take(6) {
            lines.push(format!("  {} {} ({})", x.name, x.line, x.how.label()));
        }
        if k.after.is_empty() {
            lines.push("nobody is after you".into());
        }
        for a in &k.after {
            lines.push(format!("after you: {}", a.line));
        }
        for t in k.talk.iter().take(3) {
            lines.push(format!("the talk: {t}"));
        }
        lines.push(rep_line(k));
        if self.block.is_empty() && world.has::<citysim::components::Sentence>(self.subject) {
            lines.push("your block: you were inside".into());
        } else if self.block.is_empty() {
            lines.push("your block: a quiet day".into());
        } else {
            lines.push(format!("your block ({} things):", self.block.len()));
            for b in self.block.iter().rev().take(8) {
                lines.push(format!("  {b}"));
            }
        }
        for l in &lines {
            eprintln!("dayend d{}: {l}", self.day);
        }
        self.pubd.days.push((self.day, lines));
        self.day_coins = c;
        self.day_knowers = k.knowers.iter().map(|x| x.who.index).collect();
        self.block.clear();
    }

    fn fill(&mut self, world: &World, k: &pl_news::Known) {
        let cap = world.config.view.news_knowers_max;
        self.pubd.knowers = k
            .knowers
            .iter()
            .take(cap)
            .map(|x| {
                let more = if x.count > 1 { format!(", {} things", x.count) } else { String::new() };
                (x.name.clone(), x.line.clone(), format!("{}, {}{more}", x.how.label(), stamp(x.tick)))
            })
            .collect();
        self.pubd.knowing = (k.knowers.iter().filter(|x| x.deed).count(), k.knowers.len());
        self.pubd.after = k.after.iter().map(|a| format!("{} (since {})", a.line, stamp(a.since))).collect();
        self.pubd.talk = k.talk.clone();
        self.pubd.rep = rep_line(k);
        self.pubd.seq += 1;
    }
}

fn coins(world: &World, id: EntityId) -> i64 {
    world.comp::<citysim::components::Wallet>(id).map_or(0, |w| w.coins)
}

fn rep_line(k: &pl_news::Known) -> String {
    format!(
        "the street's read: dread {:.2} standing {:.2} honour {:.2} heat {:.2}, known by {}",
        k.rep[0], k.rep[1], k.rep[2], k.rep[3], k.known_by
    )
}
