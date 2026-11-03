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
//!
//! C4 (docs/COMBAT.md § 5, the player's fight): while the possessed body is in a live engagement the thread runs at
//! `[view] combat_speed` ticks a real second (the combat clock: the whole sim, a beat about a second; a script's hold
//! still stops it), and after each tick publishes the tick's beats of the fights in the ring (`Shared::beats`,
//! `combat::last_beats`) for the replay's cues (play.rs) and the player's fight's beats as story-log lines
//! (`combat::blow_line`); the HUD carries the fight (`combat::fight_view`) and the wounds panel
//! (`combat::wounds_view`).

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
    /// C4: the last tick's beats of the fights in the ring, for the replay.
    pub beats: Mutex<BeatsPub>,
    /// C5: the replay's procedural poses (play.rs `replay` writes, sim.rs draws), the bodies lying in the ring (knocked
    /// out, downed) and the bodies of the subject's fight (always drawn, indoors too), by body key.
    pub poses: Mutex<Vec<crate::play::Pose>>,
    pub down: Mutex<Vec<u64>>,
    pub fight: Mutex<Vec<u64>>,
    /// M18 P5: the start menu's state (the nobodies, the player saves, the load's line, the auto-pause's reason).
    pub slice: Mutex<SlicePub>,
    /// M18 P5: the auto-pause on (the HUD's toggle; `[view] autopause` at the start), and the human's last key or
    /// click (away past `[view] away_s`).
    pub autopause: AtomicBool,
    pub input_at: Mutex<Instant>,
}

/// M18 P5 (docs/M18_PLAYER.md, start and loop): what the start menu shows.
#[derive(Clone, Debug, Default)]
pub struct SlicePub {
    /// Rises with each change.
    pub seq: u64,
    /// The last load's or save's line ("loaded ... in 9.8 s"), and a load or a save under way.
    pub status: String,
    pub busy: bool,
    /// The pre-run save is missing: how to make it.
    pub missing: Option<String>,
    /// "Pick a nobody": (the body, its kind, the one-line read).
    pub nobodies: Vec<(EntityId, String, String)>,
    /// The player saves, newest first: (the file, its line).
    pub saves: Vec<(PathBuf, String)>,
    /// A body is possessed (the game is on); the possession's generation (the panels start again on it).
    pub started: bool,
    pub epoch: u64,
    /// The auto-pause's reason while the clock it stopped is stopped.
    pub alert: Option<String>,
}

/// M18 P5: the command that makes the slice's pre-run save (docs/M18_PLAYER.md), shown when it is missing.
pub const PRERUN_CMD: &str = "cargo run --release -p citysim-cli -- run --days 21 --seed 42 --population 20000 --scaled --ambient-flats 50 --map assets/maps/isle_m_k2.txt --save-at 30240 --saves-dir saves/slice\n(then rename saves/slice/42-30240.ron to saves/slice/20k_s42_d21.ron)";

/// C4: one tick's beats for the replay: each fight's blows in order (a beat's blows share its `group`, the beats
/// are `beat_s` real seconds apart).
#[derive(Clone, Debug, Default)]
pub struct BeatsPub {
    pub seq: u64,
    pub beat_s: f32,
    /// (the group within its fight, the blow)
    pub beats: Vec<(u32, citysim::systems::combat::BeatLog)>,
}

/// M18 P4: the player's panels (`citysim::systems::player_news`, read at `[view] news_scan_minutes`; the log from
/// the body's events as they happen).
#[derive(Clone, Debug, Default)]
pub struct NewsPub {
    /// Rises with each change.
    pub seq: u64,
    /// M18 P5: the possession it belongs to (`SlicePub::epoch`).
    pub epoch: u64,
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
    /// C4: a fight order (a stance, Attack, Draw or Holster, Take cover, Flee, Yield, Help).
    Fight(citysim::systems::combat::Order),
    /// C4: a body's menu by its index wherever it is (a `--play` line's `body #INDEX`: a target off the screen).
    BodyIndex(u32),
    /// C4: a `--play` line's `body best friends|weak|strong`: of the adults within `[combat] join_tiles` of the
    /// subject, the one with the most likely joiners, the best odds or the worst (`combat::odds`); its menu.
    BodyBest(String),
    /// M18 P5, the start menu: the camera on a body (not possessed, the clock stopped); take a body; a fresh arrival
    /// in the Sump; the nobodies again (the death screen's pick); a save under a name; a player save loaded; the
    /// pre-run city loaded again (New after a game).
    Look(EntityId),
    Start(EntityId),
    Fresh,
    Nobodies,
    Save(String),
    Load(PathBuf),
    NewCity,
    /// M18 P5, a script's: the adult near the body with the best odds goes for it (`true`: to kill, the blackmail
    /// silencing's `bm_kill_mult`).
    Provoke(bool),
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
    /// C4: the body's live fight (`None`: not in one) and, outside one, its wounds panel.
    pub fight: Option<citysim::systems::combat::FightView>,
    pub wounds: Option<citysim::systems::combat::WoundsView>,
    /// C4: the stance and the weapon held for the next fight; the chrome carried.
    pub stance: Option<citysim::systems::combat::Stance>,
    pub holstered: bool,
    pub chrome: Vec<String>,
    /// C5: a weapon to hand (Holster shows only then)
    pub armed: bool,
}

impl Shared {
    /// M18 P1: an order for the possessed body, applied at the next tick.
    pub fn send(&self, c: UiCmd) {
        self.cmds.lock().unwrap().push(c);
    }

    /// M18 P5: a load or a save: the menu reads busy from the order on (the sim thread clears it when done).
    pub fn send_busy(&self, c: UiCmd) {
        self.slice.lock().unwrap().busy = true;
        self.send(c);
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
    /// The subject (`--follow`); `None`: the slice's (a player save's body, else a nobody to pick).
    pub follow: Option<u32>,
    /// M18 P5: the slice (`--slice`): the start menu, the player saves in this directory; `save` is the pre-run city.
    pub slice: Option<PathBuf>,
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
            beats: Mutex::new(BeatsPub::default()),
            poses: Mutex::new(Vec::new()),
            down: Mutex::new(Vec::new()),
            fight: Mutex::new(Vec::new()),
            slice: Mutex::new(SlicePub::default()),
            autopause: AtomicBool::new(true),
            input_at: Mutex::new(Instant::now()),
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

/// M18 P5: the start menu's line (a load, a save) and whether the thread is busy with it.
fn status(sh: &Shared, line: String, busy: bool) {
    eprintln!("cityview: {line}");
    let mut s = sh.slice.lock().unwrap();
    s.status = line;
    s.busy = busy;
    s.seq += 1;
}

/// M18 P5: a save loaded, timed (the menu says how long).
fn load_world(sh: &Shared, path: &Path) -> Result<World, String> {
    status(sh, format!("loading {} ...", path.display()), true);
    let t0 = Instant::now();
    let w = save::load_from_file(path).inspect_err(|e| status(sh, format!("could not load {}: {e}", path.display()), false))?;
    let secs = t0.elapsed().as_secs_f64();
    status(
        sh,
        format!(
            "loaded {} (day {} {}) in {secs:.1} s",
            path.file_name().map_or(String::new(), |f| f.to_string_lossy().into_owned()),
            w.tick / citysim::TICKS_PER_DAY,
            citysim::time::clock(w.tick)
        ),
        false,
    );
    Ok(w)
}

/// M18 P5: the body a player save holds (possessed when it was saved).
fn player_in(world: &World) -> Option<EntityId> {
    world.entities().find(|&e| world.comp::<Brain>(e).is_some_and(|b| b.player.is_some()) && world.has::<Identity>(e))
}

/// M18 P5: the start menu's nobodies (`player::nobodies`) and the player saves (`saves_dir`, newest first).
fn publish_lists(sh: &Shared, world: Option<&World>, dir: &Path) {
    let nobodies = world.map(|w| pl::nobodies(w).into_iter().map(|n| (n.id, n.kind.to_string(), n.line)).collect());
    let mut saves: Vec<(std::time::SystemTime, PathBuf, String)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?;
            let when = m.modified().ok()?;
            let name = p.file_stem()?.to_string_lossy().into_owned();
            Some((when, p, format!("{name} ({:.0} MB)", m.len() as f64 / 1e6)))
        })
        .collect();
    saves.sort_by_key(|a| std::cmp::Reverse(a.0));
    let mut s = sh.slice.lock().unwrap();
    if let Some(n) = nobodies {
        s.nobodies = n;
    }
    s.saves = saves.into_iter().map(|(_, p, l)| (p, l)).collect();
    s.seq += 1;
}

/// M18 P5: the camera (and with `possess` the human) moves from `old` (in this world; `None` after a load) to `to`.
/// The body left behind goes back to its brain.
fn take(world: &mut World, old: Option<EntityId>, to: EntityId, possess: bool) {
    if let Some(old) = old.filter(|&o| o != to) {
        if let Some(b) = world.comp_mut::<Brain>(old) {
            b.pinned = false;
        }
        if world.comp::<Brain>(old).is_some_and(|b| b.player.is_some()) && world.has::<Identity>(old) {
            pl::possess(world, old, false);
        }
    }
    if let Some(b) = world.comp_mut::<Brain>(to) {
        b.pinned = true;
    }
    world.crossing_watch = Some([(to, Vec::new())].into());
    if possess {
        if world.comp::<Brain>(to).is_some_and(|b| b.player.is_none()) {
            pl::possess(world, to, true);
        }
        world.combat.stepper = Some(to);
    } else {
        world.combat.stepper = None;
    }
}

/// M18 P5: a fight new this tick that should stop the clock (the auto-pause): the body attacked (on the struck side
/// of a fight it did not start), or a fight within `[view] autopause_tiles` of it; the line.
fn alarm(
    world: &World,
    subject: EntityId,
    seen: &mut std::collections::BTreeSet<u32>,
    by_seen: &mut std::collections::BTreeSet<EntityId>,
    tiles: u16,
) -> Option<String> {
    let me = world.comp::<citysim::Position>(subject)?;
    let near = |x: EntityId| {
        world.comp::<citysim::Position>(x).is_some_and(|q| match (q.building, me.building) {
            (Some(a), Some(b)) => a == b,
            (None, None) => {
                (i32::from(q.tile.x) - i32::from(me.tile.x)).abs() <= i32::from(tiles)
                    && (i32::from(q.tile.y) - i32::from(me.tile.y)).abs() <= i32::from(tiles)
            }
            _ => false,
        })
    };
    let mut out = None;
    for (k, e) in &world.combat.engagements {
        if !seen.insert(*k) || out.is_some() || e.sides[0].contains(&subject) {
            continue;
        }
        let by = e.sides[0].first().map_or("someone".to_string(), |&x| world.name_of(x));
        if e.sides[1].contains(&subject) {
            by_seen.extend(e.sides[0].first().copied());
            out = Some(format!("{by} went for you"));
        } else if e.sides.iter().flatten().any(|&x| near(x)) {
            let on = e.sides[1].first().map_or("someone".to_string(), |&x| world.name_of(x));
            out = Some(format!("a fight broke out near you: {by} went for {on}"));
        }
    }
    seen.retain(|k| world.combat.engagements.contains_key(k));
    out
}

fn run(a: SimArgs, sh: Arc<Shared>) {
    let slice = a.slice.clone();
    // M18 P5: the slice's pre-run save; missing, the menu says how to make it and waits for a load
    let mut world = loop {
        if slice.is_none() || a.save.is_file() {
            match load_world(&sh, &a.save) {
                Ok(w) => break w,
                Err(e) if slice.is_none() => fatal(format!("--save: {e}")),
                Err(_) => {}
            }
        } else {
            sh.slice.lock().unwrap().missing = Some(format!(
                "There is no pre-run city at {}. Make it once from the repo root (a few minutes):\n{}",
                a.save.display(),
                PRERUN_CMD
            ));
        }
        if let Some(dir) = slice.as_deref() {
            publish_lists(&sh, None, dir);
        }
        // a player save picked from the menu instead
        let load = loop {
            let cmds = std::mem::take(&mut *sh.cmds.lock().unwrap());
            if let Some(p) = cmds.into_iter().find_map(|c| if let UiCmd::Load(p) = c { Some(p) } else { None }) {
                break p;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        if let Ok(w) = load_world(&sh, &load) {
            sh.slice.lock().unwrap().missing = None;
            break w;
        }
    };
    // the subject: --follow; the slice: a player save's body, else the first nobody (looked at, the clock stopped)
    let mut possessed = false;
    let subject = match (a.follow, slice.as_ref()) {
        (Some(i), _) => world
            .entities()
            .find(|e| e.index == i)
            .unwrap_or_else(|| fatal(format!("--follow {i}: no living entity with that index at tick {}", world.tick))),
        (None, _) => match player_in(&world) {
            Some(p) => {
                possessed = true;
                p
            }
            None => pl::nobodies(&world)
                .first()
                .map(|n| n.id)
                .unwrap_or_else(|| fatal(format!("{}: no free adult to look at", a.save.display()))),
        },
    };
    let mut subject = subject;
    let building = world.has::<citysim::Building>(subject);
    if !(world.has::<Identity>(subject) || building) || view::subject_tile(&world, subject).is_none() {
        fatal(format!("--follow {}: {subject:?} is no body with a place nor a building", subject.index));
    }
    // M18 P1: nobody under 18 is ever possessed (CLAUDE.md, 2026-10-27; the sim refuses it too)
    if a.possess && !citysim::systems::demography::is_adult(&world, subject) {
        fatal(format!("--possess {}: {} is no adult (nobody under 18 can be possessed)", subject.index, who(&world, subject)));
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
    take(&mut world, None, subject, possessed);
    let mut cfg = world.config.view.clone();
    sh.autopause.store(cfg.autopause, Ordering::Relaxed);
    let mut watch = a.config.as_deref().map(|p| (mtime(p), Instant::now()));
    let (mut w, mut h) = (world.map.w(), world.map.h());
    // V5: ticks run and their wall time since the last hold (the bench's sim ticks/s)
    let mut acc = (0u64, 0f64);
    let mut seq = 0u64;
    // V6: the walkers' ways kept from one published frame to the next (one road search a walk, not a frame)
    let mut ways = view::WayCache::default();
    let mut from = a.from.unwrap_or(world.tick);
    let mut next_at = Instant::now();
    let mut reloaded_at = false;
    let mut last_logged = (String::new(), None, false);
    let mut menu_seq = 0u64;
    let mut last_event = world.events.back().map_or(0, |e| e.id);
    // M18 P4: the player's panels (read from the possession on)
    let mut news: Option<News> = None;
    let mut last_tile = view::subject_tile(&world, subject);
    // C4: a tick's beats were published (its frame goes with them)
    let mut shown_beats = false;
    // C5, the combat clock: the beats of the subject's fight stepped since the last tick, and the log entries already
    // told (a tick's `last_beats` repeats them)
    let mut stepped = 0u8;
    let mut told: Vec<citysim::systems::combat::BeatLog> = Vec::new();
    // M18 P5: a frame owed with no tick (a look, a start, a load while the clock is stopped); the fights seen (the
    // auto-pause's); the possession's generation (the render thread's panels reset on it)
    let mut republish = true;
    let mut seen_fights: std::collections::BTreeSet<u32> = world.combat.engagements.keys().copied().collect();
    let mut epoch = 0u64;
    // M18 P5: the attackers the auto-pause already told of (their assault's event at the fight's end is the same)
    let mut alarmed_by: std::collections::BTreeSet<EntityId> = Default::default();
    if let Some(dir) = slice.as_deref() {
        publish_lists(&sh, Some(&world), dir);
        sh.paused.store(!possessed, Ordering::Relaxed);
        sh.slice.lock().unwrap().started = possessed;
    }
    // M18 P5: the camera (and the human) on another body: the per-body state starts again
    macro_rules! begin {
        ($old:expr, $to:expr, $possess:expr) => {{
            let to: EntityId = $to;
            take(&mut world, $old, to, $possess);
            subject = to;
            possessed = $possess;
            news = None;
            last_logged = (String::new(), None, false);
            last_tile = view::subject_tile(&world, subject);
            stepped = 0;
            told.clear();
            seen_fights = world.combat.engagements.keys().copied().collect();
            republish = true;
            epoch += 1;
            *sh.news.lock().unwrap() = NewsPub { epoch, ..NewsPub::default() };
            *sh.player.lock().unwrap() = None;
            *sh.menu.lock().unwrap() = None;
            {
                let mut s = sh.slice.lock().unwrap();
                s.epoch = epoch;
                s.started = possessed;
                s.alert = None;
                s.seq += 1;
            }
            eprintln!("cityview: tick {} {} {}", world.tick, if possessed { "possess" } else { "look at" }, who(&world, subject));
        }};
    }
    // M18 P5: a loaded world in place of the running one
    macro_rules! swap_world {
        ($new:expr) => {{
            world = $new;
            crate::lights::graft(&mut world, a.config.as_deref());
            *sh.signals.lock().unwrap() = world.config.crossings.clone();
            cfg = world.config.view.clone();
            if a.rate.is_none() {
                rate = view::walk_matched_rate(&world);
                sh.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
            }
            (w, h) = (world.map.w(), world.map.h());
            ways = view::WayCache::default();
            last_event = world.events.back().map_or(0, |e| e.id);
            from = world.tick;
            acc = (0, 0.0);
            {
                let mut s = sh.slots.lock().unwrap();
                s.prev = None;
                s.latest = None;
            }
            sh.tick.store(world.tick, Ordering::Relaxed);
            match player_in(&world) {
                Some(p) => {
                    begin!(None, p, true);
                    sh.paused.store(false, Ordering::Relaxed);
                }
                None => {
                    if let Some(n) = pl::nobodies(&world).first().map(|n| n.id) {
                        begin!(None, n, false);
                    }
                    sh.paused.store(true, Ordering::Relaxed);
                }
            }
            if let Some(dir) = slice.as_deref() {
                publish_lists(&sh, Some(&world), dir);
            }
        }};
    }
    // M18 P5, the auto-pause: the clock stops on `line` (the body attacked, a fight near) when the toggle is on and
    // the body is on autopilot or the human away
    macro_rules! autopause {
        ($line:expr) => {{
            let line: String = $line;
            let auto = world.comp::<Brain>(subject).and_then(|b| b.player.as_ref()).is_some_and(|p| p.autopilot);
            let away = sh.input_at.lock().unwrap().elapsed().as_secs_f64() >= cfg.away_s;
            let dead = news.as_ref().is_some_and(|n| n.pubd.death.is_some());
            if possessed && !dead && sh.autopause.load(Ordering::Relaxed) && (auto || away) {
                let why = if auto { "on autopilot" } else { "you were away" };
                let line = format!("the clock stopped ({why}): {line}");
                eprintln!("autopause {}: {line}", stamp(world.tick));
                if let Some(n) = news.as_mut() {
                    n.push(&world, true, format!("!! {line}"));
                    n.sent = n.pubd.seq;
                    *sh.news.lock().unwrap() = NewsPub { epoch, ..n.pubd.clone() };
                }
                sh.paused.store(true, Ordering::Relaxed);
                let mut s = sh.slice.lock().unwrap();
                s.alert = Some(line);
                s.seq += 1;
            }
        }};
    }
    loop {
        // M18 P1: possessed at --from (the city pre-run to it); the render thread's orders, between two ticks
        if a.possess && !possessed && world.tick >= from {
            possessed = true;
            world.push_command(citysim::PlayerCommand::Possess(subject, true));
            // C5: the view steps the subject's fight a beat at a time (`combat::beat`); the tick leaves it alone
            world.combat.stepper = Some(subject);
            eprintln!("cityview: tick {} possess {}", world.tick, who(&world, subject));
        }
        let cmds = std::mem::take(&mut *sh.cmds.lock().unwrap());
        for c in cmds {
            let cmd = match c {
                // M18 P5: the start menu
                UiCmd::Look(to) => {
                    if !possessed && world.is_alive(to) && view::subject_tile(&world, to).is_some() {
                        begin!(Some(subject), to, false);
                    }
                    continue;
                }
                UiCmd::Start(to) => {
                    if world.is_alive(to) && citysim::systems::demography::is_adult(&world, to) {
                        begin!(Some(subject), to, true);
                        sh.paused.store(false, Ordering::Relaxed);
                    }
                    continue;
                }
                UiCmd::Fresh => {
                    let to = pl::arrive_fresh(&mut world);
                    eprintln!("cityview: tick {} arrive fresh: {}", world.tick, pl::nobody_line(&world, to));
                    begin!(Some(subject), to, true);
                    sh.paused.store(false, Ordering::Relaxed);
                    continue;
                }
                UiCmd::Nobodies => {
                    if let Some(dir) = slice.as_deref() {
                        publish_lists(&sh, Some(&world), dir);
                    }
                    continue;
                }
                UiCmd::Save(name) => {
                    let Some(dir) = slice.as_deref() else { continue };
                    if citysim::systems::combat::engaged(&world, subject) {
                        status(&sh, "not in the middle of a fight: save when it is over".into(), false);
                        continue;
                    }
                    let path = dir.join(format!("{name}.ron"));
                    status(&sh, format!("saving {} ...", path.display()), true);
                    let t0 = Instant::now();
                    citysim::systems::combat::settle_all(&mut world);
                    match save::save_to_file(&world, &path) {
                        Ok(()) => status(&sh, format!("saved {name} in {:.1} s", t0.elapsed().as_secs_f64()), false),
                        Err(e) => status(&sh, format!("could not save {}: {e}", path.display()), false),
                    }
                    publish_lists(&sh, Some(&world), dir);
                    continue;
                }
                UiCmd::Load(p) => {
                    if let Ok(nw) = load_world(&sh, &p) {
                        swap_world!(nw);
                    }
                    continue;
                }
                UiCmd::NewCity => {
                    if let Ok(nw) = load_world(&sh, &a.save) {
                        swap_world!(nw);
                    }
                    continue;
                }
                UiCmd::Provoke(lethal) => {
                    if let Some(x) = best_body(&world, subject, "strong") {
                        let (kill_p, why) = if lethal {
                            (Some(cfg.provoke_kill_p), citysim::systems::combat::Purpose::Raid)
                        } else {
                            (None, citysim::systems::combat::Purpose::Brawl)
                        };
                        let mods = citysim::systems::law::FightMods { kill_p, why, ..Default::default() };
                        eprintln!(
                            "cityview: tick {} provoke: {} goes for {} (kill_p {kill_p:?})",
                            world.tick,
                            who(&world, x),
                            who(&world, subject)
                        );
                        citysim::systems::combat::fight(
                            &mut world,
                            x,
                            subject,
                            mods,
                            citysim::systems::combat::Tail::Attack { victim: subject },
                        );
                    }
                    continue;
                }
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
                UiCmd::BodyBest(how) => {
                    if let Some(o) = best_body(&world, subject, &how) {
                        publish_menu(&sh, &world, pl::menu_body(&world, subject, o), &mut menu_seq);
                    }
                    continue;
                }
                UiCmd::BodyIndex(i) => {
                    if let Some(o) = world.entities().find(|e| e.index == i) {
                        publish_menu(&sh, &world, pl::menu_body(&world, subject, o), &mut menu_seq);
                    }
                    continue;
                }
                UiCmd::Act(v) => citysim::PlayerCommand::Act { agent: subject, verb: v },
                UiCmd::Goal(g) => citysim::PlayerCommand::Goal { agent: subject, goal: g },
                UiCmd::Stop => citysim::PlayerCommand::Stop(subject),
                UiCmd::Autopilot(on) => citysim::PlayerCommand::SetAutopilot(subject, on),
                UiCmd::Fight(order) => citysim::PlayerCommand::Fight { agent: subject, order },
            };
            if !possessed {
                continue;
            }
            eprintln!("cityview: tick {} order {cmd:?}", world.tick);
            world.push_command(cmd);
        }
        // M18 P5: a fight begun between two ticks (a script's provoke) stops the clock before its first beat
        if possessed && let Some(line) = alarm(&world, subject, &mut seen_fights, &mut alarmed_by, cfg.autopause_tiles) {
            autopause!(line);
        }
        // M18 P5: a frame owed with the clock stopped (the menu's look, a start, a load)
        if std::mem::take(&mut republish)
            && let Some(at) = view::subject_tile(&world, subject).or(last_tile)
        {
            publish(&world, &sh, subject, at, &cfg, &mut ways, &mut seq, possessed, news.as_mut(), &mut last_logged);
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
        // C5, the combat clock: the possessed body in a live fight has its engagement stepped a beat every `[view]
        // beat_s` inside the tick (`combat::beat`, orders landing between beats), `beats_per_tick` of them, then the
        // rest of the tick runs at once: the fight's game time is an NPC's. Under a script's hold too (the hold still
        // stops it).
        let fighting = possessed && citysim::systems::combat::engaged(&world, subject);
        if !fighting && stepped == 0 {
            told.clear();
        }
        // M18 P1: Space stops the clock (paced play only; a bench's run to its hold goes on). M18 P5: the slice's
        // clock stops under a script's hold too (the menu, the auto-pause)
        if (hold == u64::MAX || fighting || slice.is_some()) && world.tick >= from && sh.paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(20));
            next_at = Instant::now();
            continue;
        }
        // paced: free running past --from; flat out to --from and to a bench's hold
        let paced = (hold == u64::MAX || fighting || stepped > 0) && world.tick >= from;
        let beat_s = cfg.beat_s.max(0.05);
        let bpt = world.config.combat.beats_per_tick.max(1);
        let tick_rate = if fighting || stepped > 0 { 1.0 / beat_s } else { rate * sh.speed().max(1e-6) };
        let mut beat_now: Option<Vec<citysim::systems::combat::BeatLog>> = None;
        if fighting && stepped < bpt {
            let now = Instant::now();
            // a beat at most `beat_s` off (a flat-out stretch before the fight leaves `next_at` anywhere)
            next_at = next_at.min(now + Duration::from_secs_f64(beat_s));
            if now < next_at {
                std::thread::sleep((next_at - now).min(Duration::from_millis(20)));
                continue;
            }
            next_at = next_at.max(now) + Duration::from_secs_f64(beat_s);
            if let Some((logs, over)) = citysim::systems::combat::beat(&mut world, subject) {
                stepped = if over { bpt } else { stepped + 1 };
                beat_now = Some(logs);
            }
        } else if stepped > 0 {
            // the stepped beats' tick: after the last beat's second, at once
            let now = Instant::now();
            next_at = next_at.min(now + Duration::from_secs_f64(beat_s));
            if now < next_at {
                std::thread::sleep((next_at - now).min(Duration::from_millis(20)));
                continue;
            }
        } else if paced && rate > 0.0 {
            // V1: the wait in slices (a tick is a real minute walk-matched), so a reload is seen between them
            let now = Instant::now();
            if now < next_at {
                std::thread::sleep((next_at - now).min(Duration::from_millis(20)));
                continue;
            }
            next_at = next_at.max(now) + Duration::from_secs_f64(1.0 / tick_rate);
        } else if paced {
            // --rate 0: hold the frame
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        // M18 P4: a dead player's corpse can go (buried, rotted): the camera stays where it fell
        let dead = news.as_ref().is_some_and(|n| n.pubd.death.is_some());
        let at = match view::subject_tile(&world, subject).or(last_tile.filter(|_| dead || slice.is_some())) {
            Some(at) => at,
            None => fatal(format!("the subject {subject:?} is gone at tick {}", world.tick)),
        };
        last_tile = Some(at);
        let ring = view::ring_about(at, cfg.ring_half_tiles, w, h);
        world.set_view(Some(ring));
        let t_tick = Instant::now();
        let stepping = beat_now.is_some();
        if !stepping {
            citysim::tick(&mut world);
            crate::lights::log_crossings(&mut world, subject);
            stepped = 0;
        }
        // M18 P2: the possessed body's events to the log (the play session's record: a hire, a theft, an arrest)
        if possessed {
            let n = news.get_or_insert_with(|| News::new(&world, subject));
            let new: Vec<&citysim::events::Event> = world.events.iter().rev().take_while(|e| e.id > last_event).collect();
            // M18 P5: an assault on the body (a fight over inside the tick leaves no engagement to see)
            let mut assaulted = None;
            for e in new.iter().rev() {
                if e.kind == citysim::events::EventKind::Assault
                    && e.actors.get(1) == Some(&subject)
                    && e.actors[0] != subject
                    && !alarmed_by.contains(&e.actors[0])
                {
                    assaulted = Some(format!("{} went for you", world.name_of(e.actors[0])));
                }
                if e.actors.contains(&subject) {
                    let (d, c) = (e.tick / citysim::TICKS_PER_DAY, citysim::time::clock(e.tick));
                    eprintln!("event d{d} {c}: [{:?}] {}", e.kind, e.text);
                    n.event(&world, e);
                } else {
                    n.block_event(&world, e);
                }
            }
            // C4: the tick's beats: the player's fight's to the story log, every fight's in the ring to the replay. C5:
            // a stepped beat's own entries at once; the tick's repeat of them dropped
            let mut beats = match beat_now.take() {
                Some(logs) => vec![(vec![subject], logs)],
                None => citysim::systems::combat::last_beats(&world),
            };
            for (seen, log) in beats.iter_mut() {
                if seen.contains(&subject) {
                    if stepping {
                        told.extend(log.iter().copied());
                    } else {
                        log.retain(|b| !told.contains(b));
                    }
                }
            }
            beats.retain(|(_, log)| !log.is_empty());
            if !beats.is_empty() {
                let ring_r = i32::from(cfg.ring_half_tiles);
                let me = view::subject_tile(&world, subject);
                let in_ring = |x: EntityId| {
                    let t = world.comp::<citysim::Position>(x).map(|p| p.tile);
                    match (t, me) {
                        (Some(t), Some(m)) => {
                            (i32::from(t.x) - i32::from(m.x)).abs() <= ring_r && (i32::from(t.y) - i32::from(m.y)).abs() <= ring_r
                        }
                        _ => false,
                    }
                };
                // C5: another fight's tick of beats plays a beat a `beat_s` beside the subject's stepped one
                let beat_s =
                    if fighting || stepped > 0 || stepping { beat_s as f32 } else { ((1.0 / tick_rate) / f64::from(bpt)).min(1.5) as f32 };
                let mut out = Vec::new();
                for (seen, log) in &beats {
                    if !seen.iter().any(|&x| x == subject || in_ring(x)) {
                        continue;
                    }
                    let mine = seen.contains(&subject);
                    let (mut group, mut last) = (0u32, None);
                    for b in log {
                        if last.is_some_and(|l| l != b.beat) {
                            group += 1;
                        }
                        last = Some(b.beat);
                        out.push((group, *b));
                        if mine {
                            let (line, turn) = citysim::systems::combat::blow_line(&world, b, subject);
                            eprintln!("beat {}: {line}", stamp(b.tick));
                            n.push(&world, turn, line);
                        }
                    }
                }
                if !out.is_empty() {
                    let mut bp = sh.beats.lock().unwrap();
                    bp.seq += 1;
                    bp.beat_s = beat_s;
                    bp.beats = out;
                    drop(bp);
                    shown_beats = true;
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
                *sh.news.lock().unwrap() = NewsPub { epoch, ..n.pubd.clone() };
            }
            // M18 P5, the auto-pause: attacked, or a fight near, on autopilot or with the human away (a fight the
            // body started never stops it: `alarm` skips the attacking side)
            let near = alarm(&world, subject, &mut seen_fights, &mut alarmed_by, cfg.autopause_tiles);
            if let Some(line) = near.or(assaulted) {
                autopause!(line);
            }
        }
        last_event = world.events.back().map_or(last_event, |e| e.id);
        if !stepping {
            acc = (acc.0 + 1, acc.1 + t_tick.elapsed().as_secs_f64());
        }
        sh.tick.store(world.tick, Ordering::Relaxed);
        // publish every paced tick; flat out, only the two ticks before the hold (or --from) the lerp needs (C4: and
        // every tick with beats of a fight in the ring)
        let target = if hold == u64::MAX { from } else { hold };
        if paced || stepping || std::mem::take(&mut shown_beats) || world.tick + 1 >= target {
            let line = publish(&world, &sh, subject, at, &cfg, &mut ways, &mut seq, possessed, news.as_mut(), &mut last_logged);
            if !paced && !stepping {
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
            if !stepping {
                next_at = next_at.max(Instant::now());
            }
        }
    }
}

/// One frame to the render thread: the ring's snapshot, the caption, the possessed body's HUD (and its log's plan
/// lines), who lies in the ring and who is in the subject's fight. The ring's counts line, returned.
#[allow(clippy::too_many_arguments)]
fn publish(
    world: &World,
    sh: &Shared,
    subject: EntityId,
    at: TilePos,
    cfg: &citysim::config::ViewCfg,
    ways: &mut view::WayCache,
    seq: &mut u64,
    possessed: bool,
    news: Option<&mut News>,
    last_logged: &mut (String, Option<String>, bool),
) -> String {
    let (w, h) = (world.map.w(), world.map.h());
    let at = view::subject_tile(world, subject).unwrap_or(at);
    let ring = view::ring_about(at, cfg.ring_half_tiles, w, h);
    let frame = view::snapshot_many_cached(world, &[ring], Some(ways)).pop().expect("one ring, one frame");
    let line = ring_line(world, &frame);
    if std::env::var_os("CV_RING_DEBUG").is_some() {
        ring_debug(world, &frame);
    }
    *seq += 1;
    let caption = format!("{}\n{line}", view::caption(world, &frame, subject));
    if possessed {
        let hud = player_hud(world, subject);
        // the log's record of the body's day: each new plan, failure and autopilot switch
        if let (Some(l), Some(n)) = (log_player(&hud, last_logged), news) {
            n.push(world, false, l);
            n.sent = n.pubd.seq;
            let epoch = sh.news.lock().unwrap().epoch;
            *sh.news.lock().unwrap() = NewsPub { epoch, ..n.pubd.clone() };
        }
        *sh.player.lock().unwrap() = Some(hud);
    }
    // C5: who lies in the ring (knocked out, downed) and who is in the subject's fight
    let key = |e: EntityId| u64::from(e.index) | (u64::from(e.generation) << 32);
    let down: Vec<u64> = frame
        .agents
        .iter()
        .map(|a| a.id)
        .filter(|&x| citysim::systems::combat::knocked_out(world, x) || citysim::systems::wounded::is_down(world, x))
        .map(key)
        .collect();
    *sh.down.lock().unwrap() = down;
    let fight: Vec<u64> = world
        .combat
        .engaged
        .get(&subject)
        .and_then(|i| world.combat.engagements.get(i))
        .map(|e| e.seen.iter().copied().map(key).collect())
        .unwrap_or_default();
    *sh.fight.lock().unwrap() = fight;
    let p = Arc::new(Published { frame, subject, caption, at: Instant::now(), seq: *seq });
    let mut s = sh.slots.lock().unwrap();
    s.prev = s.latest.take();
    s.latest = Some(p);
    line
}

/// V5 (`CV_RING_DEBUG=1`): the bodies promoted this tick from Statistical and placed at work, and the Statistical
/// bodies in the camera, to the log.
fn ring_debug(world: &World, frame: &ViewFrame) {
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
            kind(citysim::systems::ring::schedule_door(world, id))
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
            citysim::systems::ring::schedule_door(world, ag.id).map(|b| (b, world.comp::<citysim::Building>(b).map(|x| x.door))),
            world.comp::<Brain>(ag.id).map(|b| b.lod_since)
        );
    }
}

/// C4: a script's target: of the adults within `[combat] join_tiles` of `me` (or in its building), the one with the
/// most likely joiners (`friends`), the best odds (`weak`) or the worst (`strong`); the table to the log.
fn best_body(world: &World, me: EntityId, how: &str) -> Option<EntityId> {
    let p = world.comp::<citysim::Position>(me)?;
    let r = i32::from(world.config.combat.join_tiles as u16);
    let mut rows = Vec::new();
    for x in world.entities() {
        if x == me || !world.has::<Brain>(x) || !citysim::systems::demography::is_adult(world, x) {
            continue;
        }
        let Some(q) = world.comp::<citysim::Position>(x) else { continue };
        let near = match (q.building, p.building) {
            (Some(a), Some(b)) => a == b,
            (None, None) => {
                (i32::from(q.tile.x) - i32::from(p.tile.x)).abs() <= r && (i32::from(q.tile.y) - i32::from(p.tile.y)).abs() <= r
            }
            _ => false,
        };
        if !near || citysim::systems::wounded::is_down(world, x) || world.has::<citysim::components::Sentence>(x) {
            continue;
        }
        // C5: `out`: a body knocked out at hand (its menu: Go through the pockets)
        if how == "out" {
            if citysim::systems::combat::knocked_out(world, x) {
                eprintln!("best out: {} #{}", world.name_of(x), x.index);
                return Some(x);
            }
            continue;
        }
        rows.push((x, citysim::systems::combat::odds(world, me, x)));
    }
    // C5: `brave`: alone (no likely joiners), the odds at least even, the most courage (one who stands and is beaten)
    let courage = |x: EntityId| citysim::systems::law::courage(world, x);
    let key = |x: EntityId, o: &citysim::systems::combat::Odds| match how {
        "weak" => o.p,
        "strong" => -o.p,
        "brave" if o.p >= 0.5 && o.theirs < 1.2 => courage(x),
        "brave" => -1.0,
        _ => o.theirs - 0.5 * o.p,
    };
    rows.sort_by(|a, b| key(b.0, &b.1).total_cmp(&key(a.0, &a.1)));
    for (x, o) in rows.iter().take(8) {
        eprintln!("best {how}: {} #{} odds {:.0}% theirs {:.2}", world.name_of(*x), x.index, o.p * 100.0, o.theirs);
    }
    rows.first().map(|r| r.0)
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
        fight: citysim::systems::combat::fight_view(world, id),
        wounds: citysim::systems::combat::wounds_view(world, id),
        stance: citysim::systems::combat::stance_pref(world, id),
        holstered: citysim::systems::combat::holstered(world, id),
        armed: citysim::systems::combat::kind_of(world, id) > 0,
        chrome: citysim::systems::combat::chrome_lines(world, id),
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
        // C5: knocked out, the minutes to coming to
        None if citysim::systems::combat::knocked_out(world, id) => {
            let left = world.combat.out.get(&id).map_or(0, |o| o.until.saturating_sub(world.tick));
            format!("{why}: you come to in {left} min")
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
    /// C5 (`OPEN-297`): the witnesses of one deed folded into one log line: (the tick, the deed, how many, the line).
    witness: Option<(u64, String, u32, String)>,
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
            witness: None,
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
        // C5: a flood of witnesses to one deed is one line ("12 saw Solene Achebe commit Assault on ..."), counted up
        if e.kind == K::Witness
            && let Some((_, deed)) = e.text.split_once(" saw ")
        {
            if let Some((t, d, n, last)) = self.witness.as_mut()
                && *t == e.tick
                && d == deed
                && let Some(l) = self.pubd.log.iter_mut().rev().take(60).find(|l| l.1 == *last)
            {
                *n += 1;
                l.1 = format!("{} [Witness] {n} saw {deed}", stamp(e.tick));
                *last = l.1.clone();
                self.pubd.seq += 1;
                return;
            }
            let line = format!("{} [Witness] 1 saw {deed} ({})", stamp(e.tick), e.text.split(" saw ").next().unwrap_or(""));
            self.witness = Some((e.tick, deed.to_string(), 1, line.clone()));
            self.pubd.log.push((true, line));
            self.pubd.seq += 1;
            return;
        }
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
