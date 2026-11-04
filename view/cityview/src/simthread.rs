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
    /// T2 (docs/TIME_MODEL.md § 4): the older frame's tick (the lerp runs from it to this one's, however many ticks
    /// passed: the older frame's `ahead` carries the whole way walked between them) and the real seconds the lerp
    /// takes (the ticks' own pacing).
    pub from_tick: u64,
    pub span_s: f32,
    /// Motion V1: the pair is drawn on the view's clock (`Shared::vt`, a paced tick); `false`: a flat-out frame (u 1 at
    /// once) or a fight's beat (over `span_s` from `at`, the combat clock).
    pub clocked: bool,
    /// Motion V2 (S1's `citysim::exec::stride`, `OPEN-317`): each Full walker's way through its next step at this
    /// frame's tick (0..1, by body key; none: not on a Full walk), drawn at `tile + stride x (next - tile)`
    pub strides: std::collections::HashMap<u64, f32>,
}

/// T2 (§ 5): the possessed body's way from a click, planned between two ticks (`player::way_now`): the tiles from
/// its place, and the sim's walk pace (tiles a tick) the view walks it at, in the view's game time.
#[derive(Clone, Debug)]
pub struct WayPub {
    pub seq: u64,
    pub tiles: Vec<TilePos>,
    pub tiles_per_tick: f32,
}

/// The save's fixed facts: the El lines' names (the map's `L` lines, by index) and the tile -> metre frame.
#[derive(Clone)]
pub struct Static {
    pub lines: Vec<String>,
    pub tile_m: f32,
    pub north_m: f32,
    /// V7: the map's marked crossings (lights.rs draws their signals)
    pub crossings: Vec<citysim::map::MapCrossing>,
    /// Motion V2: S2's `[combat] stir_tiles` (a fight's bystanders this near watch or run): a watcher faces the fight
    pub stir_tiles: u16,
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
    /// Motion V2: the subject's own side in its fight now (the rest of `fight` its foes, or gone from it)
    pub fight_side: Mutex<Vec<u64>>,
    /// M18 P5: the start menu's state (the nobodies, the player saves, the load's line, the auto-pause's reason).
    pub slice: Mutex<SlicePub>,
    /// M18 P5: the auto-pause on (the HUD's toggle; `[view] autopause` at the start), and the human's last key or
    /// click (away past `[view] away_s`).
    pub autopause: AtomicBool,
    pub input_at: Mutex<Instant>,
    /// T2 (docs/TIME_MODEL.md § 3): auto-speed on (the HUD's and the Esc menu's toggle; `[view] auto_speed_on` at the
    /// start); why auto runs, for the HUD ("asleep, 600x"; empty: the human's speed).
    pub auto_on: AtomicBool,
    pub auto_line: Mutex<String>,
    /// T2 (§ 5): the clock's pace now, ticks a real second (f64 bits; 0 while it is stopped or held): the click way's.
    pub tick_rate_bits: AtomicU64,
    /// T2 (§ 4): the newest frame seq the render thread has taken (the sim thread publishes the next once it has, and
    /// carries the ways of the ticks between).
    pub taken: AtomicU64,
    /// T2 (§ 5): the possessed body's way from the last click (`None`: none, or a Stop).
    pub way: Mutex<Option<WayPub>>,
    /// The film (film.rs): the pacing's clock (`Shared::now`), and the film time the sim thread has published every
    /// tick due by (f64 bits; -1 none yet).
    pub clock: Mutex<Clock>,
    pub film_ready_bits: AtomicU64,
    /// Motion V1: the view's clock (`ViewClock`): the lerp's `u` and the sim's next tick both read it.
    pub view_clock: Mutex<ViewClock>,
}

/// Motion V1 (docs/TIME_MODEL.md § 4): the view's game time, ticks, one clock for the lerp and the pacing: `vt` at
/// `at` (the pacing's clock, `Shared::now`) running on at `pace` ticks a real second (0 while stopped, held or flat
/// out). The drawn pair is (newest - 1 .. newest) at `u` = `vt` between their ticks, and the sim thread runs the next
/// tick when `vt` reaches the newest one (the view has drawn the pair out), so a new pair starts where the last one
/// ended: a speed change speeds the drawn walk at once (no pair cut short, no forward snap), a pause freezes the
/// bodies mid-step, and after a hold, a load or `--from` (`vt` set to the newest tick) the next tick runs at once
/// (P1's films: every NPC stood 10 s at 1x after a hold, then jumped).
#[derive(Clone, Copy)]
pub struct ViewClock {
    pub at: Instant,
    pub vt: f64,
    pub pace: f64,
}

/// The film (film.rs): the clock the sim's pacing and the lerp read. Outside a film it is the wall clock (less what
/// the films took: no jump after one); while filming it is the film's own, advanced 1/FPS a captured frame by the
/// render thread, so the frames stand for even game-real time however slow the capture.
#[derive(Default)]
pub struct Clock {
    /// Filming: the clock at the film's start and the film time since (s).
    pub film: Option<(Instant, f64)>,
    /// The film's frame step (s, 1/FPS).
    pub step: f64,
    /// The wall clock's lead over this one (the films' wall time past their film time).
    pub offset: Duration,
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
    /// P5b: whose street the camera rests over while nobody is possessed (the start menu says so).
    pub backdrop: String,
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
    /// M18 P5h: a `--play` line's `bld near hostel|corp|city`: the nearest capsule hostel with a bed, corp letting
    /// desk with a place, or Civic Hall (`housing::script_target`); its menu.
    BldNear(String),
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
    /// M18 P5h: the tenure (`housing::tenure_line`: sleeps rough, a capsule at ..., a room at ..., a flat at ...).
    pub tenure: String,
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

    /// T2: the clock's pace now, ticks a real second (0 while stopped or held).
    pub fn tick_rate(&self) -> f32 {
        f64::from_bits(self.tick_rate_bits.load(Ordering::Relaxed)) as f32
    }

    /// T2 (§ 4): the lerp's `u` (0..1) from the older frame to `latest`. Motion V1: a paced pair's from the view's
    /// clock (`vt` between the pair's ticks); a flat-out frame's 1 (a bench's shot shows its tick exactly); a fight's
    /// beat's the real time since it was published over its span.
    pub fn lerp_u(&self, latest: &Published) -> f32 {
        let ticks = latest.frame.tick.saturating_sub(latest.from_tick);
        if latest.clocked && ticks > 0 {
            return ((self.vt() - latest.from_tick as f64) / ticks as f64).clamp(0.0, 1.0) as f32;
        }
        if latest.span_s <= 0.0 {
            return 1.0;
        }
        (self.now().saturating_duration_since(latest.at).as_secs_f32() / latest.span_s).clamp(0.0, 1.0)
    }

    /// Motion V1: the view's clock now, ticks (`ViewClock`).
    pub fn vt(&self) -> f64 {
        let now = self.now();
        let c = *self.view_clock.lock().unwrap();
        c.vt + now.saturating_duration_since(c.at).as_secs_f64() * c.pace
    }

    /// Motion V1: the view's clock runs on at `pace` ticks a real second from now, no further than tick `cap` (the
    /// newest tick run: a sim slower than the pace holds the drawn bodies at their newest place, never past it).
    pub fn vt_pace(&self, pace: f64, cap: f64) {
        let vt = self.vt().min(cap);
        let now = self.now();
        *self.view_clock.lock().unwrap() = ViewClock { at: now, vt, pace };
    }

    /// Motion V1: the view's clock set to tick `vt` (a flat-out frame, a fight's beat, a load: the view is there).
    pub fn vt_set(&self, vt: f64) {
        let now = self.now();
        let mut c = self.view_clock.lock().unwrap();
        c.at = now;
        c.vt = vt;
    }

    /// The film: the pacing's clock (`Clock`): the wall clock outside a film, the film's while one runs.
    pub fn now(&self) -> Instant {
        let c = self.clock.lock().unwrap();
        match c.film {
            Some((base, t)) => base + Duration::from_secs_f64(t),
            None => {
                let now = Instant::now();
                now.checked_sub(c.offset).unwrap_or(now)
            }
        }
    }

    /// The film's time (s) while one runs.
    pub fn film_t(&self) -> Option<f64> {
        self.clock.lock().unwrap().film.map(|f| f.1)
    }

    /// The film starts: the clock stops following the wall at its reading now; it steps `step` seconds a frame.
    pub fn film_begin(&self, step: f64) {
        let base = self.now();
        self.film_ready_bits.store((-1f64).to_bits(), Ordering::Relaxed);
        let mut c = self.clock.lock().unwrap();
        c.film = Some((base, 0.0));
        c.step = step;
    }

    /// The film's clock on by `dt` seconds (a captured frame's step).
    pub fn film_advance(&self, dt: f64) {
        if let Some(f) = self.clock.lock().unwrap().film.as_mut() {
            f.1 += dt;
        }
    }

    /// The film is over: the clock follows the wall again from where the film left it (no jump).
    pub fn film_end(&self) {
        let mut c = self.clock.lock().unwrap();
        if let Some((base, t)) = c.film.take() {
            let at = base + Duration::from_secs_f64(t);
            c.offset = Instant::now().saturating_duration_since(at);
        }
    }

    /// The film: the clock now and, while filming, the film time it reads (under one lock).
    pub fn now_film(&self) -> (Instant, Option<f64>) {
        let c = self.clock.lock().unwrap();
        match c.film {
            Some((base, t)) => (base + Duration::from_secs_f64(t), Some(t)),
            None => {
                let now = Instant::now();
                (now.checked_sub(c.offset).unwrap_or(now), None)
            }
        }
    }

    /// The film time by which the sim thread has published every tick due (-1 none yet).
    pub fn film_ready(&self) -> f64 {
        f64::from_bits(self.film_ready_bits.load(Ordering::Relaxed))
    }

    /// T2: the view's game time, ticks: the older frame's tick and `u` of the way to the newer's.
    pub fn view_tick(&self) -> Option<f64> {
        let (_, latest) = self.pair();
        let l = latest?;
        let u = f64::from(self.lerp_u(&l));
        Some(l.from_tick as f64 + u * (l.frame.tick.saturating_sub(l.from_tick)) as f64)
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
            fight_side: Mutex::new(Vec::new()),
            slice: Mutex::new(SlicePub::default()),
            autopause: AtomicBool::new(true),
            input_at: Mutex::new(Instant::now()),
            auto_on: AtomicBool::new(true),
            auto_line: Mutex::new(String::new()),
            tick_rate_bits: AtomicU64::new(0f64.to_bits()),
            taken: AtomicU64::new(0),
            way: Mutex::new(None),
            clock: Mutex::new(Clock::default()),
            film_ready_bits: AtomicU64::new((-1f64).to_bits()),
            view_clock: Mutex::new(ViewClock { at: Instant::now(), vt: 0.0, pace: 0.0 }),
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
    // S2 (`OPEN-149`): the ring runs on the new subject now, so the first frame after a load or a start draws no body
    // in the camera Statistical or Coarse (it used to be drawn before the ring ever ran)
    if let Some(at) = view::subject_tile(world, to) {
        world.view_now(Some(view::ring_about(at, world.config.view.ring_half_tiles, world.map.w(), world.map.h())));
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

/// T2: the client's `[view]` from the `--config` file. A save carries the config it was made with; the view's keys are
/// the client's (the speeds, auto-speed), while the pace the walk-matched rate reads (`[exec]`) stays the save's.
fn view_keys(world: &mut World, config: Option<&Path>) {
    let Some(p) = config else { return };
    let parsed =
        std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| citysim::Config::from_toml(&t, &world.config.assets_dir));
    match parsed {
        Ok(c) => world.config.view = c.view,
        Err(e) => eprintln!("cityview: [view] from {}: {e}; the save's stays", p.display()),
    }
}

/// T2 (docs/TIME_MODEL.md § 4): one body's (or vehicle's) way since the last published frame: its place in the last
/// frame seen, that frame's way ahead, and the path walked since the published one.
struct Track {
    at: TilePos,
    ahead: Vec<TilePos>,
    walked: Vec<TilePos>,
}

/// T2 (§ 4): the ways walked over the ticks since the last published frame (the render thread had not taken it), so
/// the next published pair's older frame carries the whole path, however many ticks passed. `gap`: a tick ran with
/// no frame seen (flat out to a hold): no stitching across it.
#[derive(Default)]
struct Trail {
    ticks: u32,
    span_s: f64,
    gap: bool,
    agents: std::collections::HashMap<EntityId, Track>,
    vehicles: std::collections::HashMap<EntityId, Track>,
    /// The last stitch's searched ways and the ones that found none (`CV_LERP_DEBUG`).
    repaired: (u32, u32),
}

/// T2 (§ 4): the way from `from` to `to` along `ahead` when it reaches `to`; else, within `[view]
/// trail_repair_tiles`, the street way between them searched (a walk begun inside the tick, a door left: the frame
/// before had no way ahead); else `[to]`, a jump in the sim (a placement), drawn as one.
fn seg(world: &World, from: TilePos, ahead: &[TilePos], to: TilePos, repair: u16, n: &mut (u32, u32)) -> Vec<TilePos> {
    if from == to {
        return Vec::new();
    }
    if let Some(e) = ahead.iter().position(|p| *p == to) {
        return ahead[..=e].to_vec();
    }
    let far = citysim::systems::law::chebyshev(from, to);
    if repair > 0 && far > 1 && far <= u32::from(repair) && world.map.walkable(to) {
        n.0 += 1;
        let p = if world.big_paths() {
            world.big_path(from, to)
        } else {
            citysim::exec::pathfind::astar(&world.map, from, to, world.config.exec.astar_max_expansions)
        };
        if let Some(p) = p.filter(|p| p.last() == Some(&to)) {
            return p;
        }
        n.1 += 1;
    }
    vec![to]
}

impl Trail {
    /// A frame published: the tracks start again at it.
    fn start(&mut self, f: &ViewFrame) {
        self.ticks = 0;
        self.span_s = 0.0;
        self.gap = false;
        self.agents = f.agents.iter().map(|a| (a.id, Track { at: a.t, ahead: a.ahead.clone(), walked: Vec::new() })).collect();
        self.vehicles = f.vehicles.iter().map(|v| (v.id, Track { at: v.t, ahead: v.ahead.clone(), walked: Vec::new() })).collect();
    }

    /// A tick's frame not published: each track walks on to its place in it.
    fn extend(&mut self, world: &World, f: &ViewFrame, repair: u16) {
        let mut n = (0, 0);
        for a in &f.agents {
            let t = self.agents.entry(a.id).or_insert_with(|| Track { at: a.t, ahead: Vec::new(), walked: Vec::new() });
            let s = seg(world, t.at, &t.ahead, a.t, repair, &mut n);
            t.walked.extend(s);
            t.at = a.t;
            t.ahead = a.ahead.clone();
        }
        for v in &f.vehicles {
            let t = self.vehicles.entry(v.id).or_insert_with(|| Track { at: v.t, ahead: Vec::new(), walked: Vec::new() });
            let s = seg(world, t.at, &t.ahead, v.t, 0, &mut n);
            t.walked.extend(s);
            t.at = v.t;
            t.ahead = v.ahead.clone();
        }
        self.repaired.0 += n.0;
        self.repaired.1 += n.1;
    }

    /// The older frame of the pair (`base`, `newer`) with each way ahead the whole path walked to the newer place.
    fn stitch(&mut self, world: &World, base: &ViewFrame, newer: &ViewFrame, repair: u16) -> ViewFrame {
        let mut n = (0, 0);
        let mut out = base.clone();
        let to_a: std::collections::HashMap<EntityId, TilePos> = newer.agents.iter().map(|a| (a.id, a.t)).collect();
        for a in out.agents.iter_mut() {
            let (Some(tr), Some(&to)) = (self.agents.get(&a.id), to_a.get(&a.id)) else { continue };
            let mut way = tr.walked.clone();
            way.extend(seg(world, tr.at, &tr.ahead, to, repair, &mut n));
            a.ahead = way;
        }
        let to_v: std::collections::HashMap<EntityId, TilePos> = newer.vehicles.iter().map(|v| (v.id, v.t)).collect();
        for v in out.vehicles.iter_mut() {
            let (Some(tr), Some(&to)) = (self.vehicles.get(&v.id), to_v.get(&v.id)) else { continue };
            let mut way = tr.walked.clone();
            way.extend(seg(world, tr.at, &tr.ahead, to, 0, &mut n));
            v.ahead = way;
        }
        self.repaired.0 += n.0;
        self.repaired.1 += n.1;
        out
    }
}

/// T2 (docs/TIME_MODEL.md § 3): auto-speed on the sim thread.
struct Auto {
    /// The stretch it runs over now (`None`: the human's speed), and a stretch the human took back (a key, a click,
    /// someone came for the body): no auto over it until it changes.
    on: Option<&'static str>,
    latched: Option<&'static str>,
    /// The ramp: from `from` at `at` to `to` (multiples of the rate).
    from: f64,
    at: Instant,
    to: f64,
    /// The human's last key or click seen.
    input: Instant,
    /// Ticks run since `.1`, and the speed they made (what the sim thread manages: the HUD's).
    window: (u32, Instant),
    managed: f64,
}

impl Auto {
    /// `now`: the pacing's clock (`Shared::now`; a film's while one runs); the human's input stays on the wall's.
    fn new(speed: f64, now: Instant) -> Auto {
        Auto { on: None, latched: None, from: speed, at: now, to: speed, input: Instant::now(), window: (0, now), managed: 0.0 }
    }

    /// The speed now: geometric from `from` to `to` over `ramp_s` real seconds (1x to 600x reads as a steady rise).
    fn speed(&self, ramp_s: f64, now: Instant) -> f64 {
        let k = if ramp_s <= 0.0 { 1.0 } else { (now.saturating_duration_since(self.at).as_secs_f64() / ramp_s).min(1.0) };
        let (a, b) = (self.from.max(1e-6), self.to.max(1e-6));
        a * (b / a).powf(k)
    }

    fn aim(&mut self, to: f64, ramp_s: f64, now: Instant) {
        if (to - self.to).abs() > 1e-9 {
            self.from = self.speed(ramp_s, now);
            self.at = now;
            self.to = to;
        }
    }
}

/// T2 (§ 3): someone coming for the body or speaking to it, near it (its building, or within `tiles` on the street):
/// a new event of another body's naming it, or a body whose step under way targets it. The name and what.
fn coming(world: &World, subject: EntityId, new: &[&citysim::events::Event], tiles: u16) -> Option<String> {
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
    for e in new {
        if let Some(&x) = e.actors.first()
            && x != subject
            && e.actors.contains(&subject)
            && world.has::<Identity>(x)
            && near(x)
        {
            return Some(format!("{} ({:?})", world.name_of(x), e.kind));
        }
    }
    // scan-ok: the view's, a tick while auto-speed runs (not the sim's)
    world
        .entities()
        .find(|&x| {
            x != subject && world.comp::<Brain>(x).and_then(|b| b.current_step()).is_some_and(|s| s.target == Some(subject)) && near(x)
        })
        .map(|x| {
            let what = world.comp::<Brain>(x).and_then(|b| b.current_step()).map(|s| s.action);
            format!("{} ({what:?})", world.name_of(x))
        })
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
    view_keys(&mut world, a.config.as_deref());
    *sh.signals.lock().unwrap() = world.config.crossings.clone();
    // T2 (docs/TIME_MODEL.md § 2): 1x is walk-matched on the running world's own pace (a save's `[exec]`)
    let mut rate = a.rate.unwrap_or_else(|| view::walk_matched_rate(&world));
    sh.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
    *sh.stat.lock().unwrap() = Some(Static {
        lines: world.map.lines.iter().map(|l| l.name.clone()).collect(),
        tile_m: world.config.view.tile_m as f32,
        north_m: world.config.view.north_m as f32,
        crossings: world.map.crossings.clone(),
        stir_tiles: world.config.combat.stir_tiles,
    });
    eprintln!("cityview: following {} at {rate:.4} ticks/s", who(&world, subject));
    take(&mut world, None, subject, possessed);
    let mut cfg = world.config.view.clone();
    sh.autopause.store(cfg.autopause, Ordering::Relaxed);
    sh.auto_on.store(cfg.auto_speed_on, Ordering::Relaxed);
    // T2: auto-speed (§ 3), the ways walked since the last published frame (§ 4), the real time since the last tick
    // (the pair's lerp span; it starts again on a pause), the click ways published (§ 5)
    let mut auto = Auto::new(sh.speed(), sh.now());
    let mut trail = Trail::default();
    // the film (film.rs): the pacing reads `Shared::now`, the wall clock outside a film
    let mut last_tick_at = sh.now();
    let mut film_tick_at: Option<Instant> = None;
    let mut way_seq = 0u64;
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
    // P5b: an autopilot order given but not yet run (the next tick applies it): the auto-pause's reason reads it
    let mut auto_asked: Option<bool> = None;
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
            *sh.way.lock().unwrap() = None;
            trail.gap = true;
            {
                let mut s = sh.slice.lock().unwrap();
                s.epoch = epoch;
                s.started = possessed;
                s.alert = None;
                s.backdrop = match (possessed, last_tile) {
                    (false, Some(t)) => format!("The street behind this menu: {}, no one's in particular.", pl::area_of(&world, t)),
                    _ => String::new(),
                };
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
            view_keys(&mut world, a.config.as_deref());
            *sh.signals.lock().unwrap() = world.config.crossings.clone();
            cfg = world.config.view.clone();
            if a.rate.is_none() {
                rate = view::walk_matched_rate(&world);
                sh.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
            }
            (w, h) = (world.map.w(), world.map.h());
            ways = view::WayCache::default();
            trail = Trail { gap: true, ..Trail::default() };
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
            let piloted =
                auto_asked.unwrap_or_else(|| world.comp::<Brain>(subject).and_then(|b| b.player.as_ref()).is_some_and(|p| p.autopilot));
            let away = sh.input_at.lock().unwrap().elapsed().as_secs_f64() >= cfg.away_s;
            let dead = news.as_ref().is_some_and(|n| n.pubd.death.is_some());
            if possessed && !dead && sh.autopause.load(Ordering::Relaxed) && (piloted || away) {
                let why = if piloted { "on autopilot" } else { "you were away" };
                let line = format!("the clock stopped ({why}): {line}");
                eprintln!("autopause {}: {line}", stamp(world.tick));
                if let Some(n) = news.as_mut() {
                    n.push(&world, true, format!("!! {line}"));
                    n.sent = n.pubd.seq;
                    *sh.news.lock().unwrap() = NewsPub { epoch, ..n.pubd.clone() };
                }
                sh.paused.store(true, Ordering::Relaxed);
                // T2: the clock stopped is no stretch: auto-speed goes back to the human's speed with it
                if let Some(st) = auto.on.take() {
                    auto.latched = Some(st);
                }
                sh.auto_line.lock().unwrap().clear();
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
                UiCmd::BldNear(what) => {
                    let b = citysim::systems::housing::script_target(&world, subject, &what);
                    eprintln!("cityview: tick {} bld near {what}: {:?}", world.tick, b.map(|b| world.name_of(b)));
                    let door = b.and_then(|b| world.comp::<citysim::Building>(b)).map(|x| x.door);
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
                UiCmd::Autopilot(on) => {
                    auto_asked = Some(on);
                    citysim::PlayerCommand::SetAutopilot(subject, on)
                }
                UiCmd::Fight(order) => citysim::PlayerCommand::Fight { agent: subject, order },
            };
            if !possessed {
                continue;
            }
            eprintln!("cityview: tick {} order {cmd:?}", world.tick);
            // T2 (docs/TIME_MODEL.md § 5): a walk's order (a click, a menu row, a goal, Stop) lands at once, between two
            // ticks, and its way goes to the view now: the body moves at the click, not at the next tick (10 real
            // seconds at 1x). A fight's order and the autopilot's wait for the tick as before.
            use citysim::PlayerCommand as Pc;
            let at_once = matches!(cmd, Pc::Steps { .. } | Pc::Act { .. } | Pc::Goal { .. } | Pc::Stop(_))
                && !citysim::systems::combat::engaged(&world, subject);
            world.push_command(cmd);
            if at_once {
                world.apply_commands();
                let tiles = pl::way_now(&world, subject);
                pl::start_now(&mut world, subject);
                let e = &world.config.exec;
                let tiles_per_tick = (e.walk_tiles_per_tick as f64 / e.move_ticks_full.max(1) as f64) as f32;
                way_seq += 1;
                eprintln!(
                    "cityview: tick {} way now: {}",
                    world.tick,
                    tiles.as_ref().map_or("none (no walk on foot under way)".to_string(), |t| format!(
                        "{} tiles from ({}, {}) to ({}, {}), walked at {tiles_per_tick} a tick",
                        t.len().saturating_sub(1),
                        t[0].x,
                        t[0].y,
                        t[t.len() - 1].x,
                        t[t.len() - 1].y
                    ))
                );
                *sh.way.lock().unwrap() = tiles.map(|tiles| WayPub { seq: way_seq, tiles, tiles_per_tick });
                *sh.player.lock().unwrap() = Some(player_hud(&world, subject));
            }
        }
        // M18 P5: a fight begun between two ticks (a script's provoke) stops the clock before its first beat
        if possessed && let Some(line) = alarm(&world, subject, &mut seen_fights, &mut alarmed_by, cfg.autopause_tiles) {
            autopause!(line);
        }
        // M18 P5: a frame owed with the clock stopped (the menu's look, a start, a load)
        if std::mem::take(&mut republish)
            && let Some(at) = view::subject_tile(&world, subject).or(last_tile)
        {
            publish(
                &world,
                &sh,
                subject,
                at,
                &cfg,
                &mut ways,
                &mut seq,
                possessed,
                news.as_mut(),
                &mut last_logged,
                &mut trail,
                (0.0, sh.now(), false, false),
            );
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
        // the film (film.rs): the clock now and the film's time it reads (one lock: a frame's step may land between)
        let (now_v, film_t) = sh.now_film();
        // nothing is due: every tick due by the film's time is published, the render thread may capture it
        let ready = || {
            if let Some(t) = film_t {
                sh.film_ready_bits.store(t.to_bits(), Ordering::Relaxed);
            }
        };
        if world.tick >= hold {
            ready();
            sh.tick_rate_bits.store(0f64.to_bits(), Ordering::Relaxed);
            sh.vt_pace(0.0, world.tick as f64);
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
            ready();
            std::thread::sleep(Duration::from_millis(20));
            next_at = Instant::now();
            last_tick_at = sh.now();
            sh.tick_rate_bits.store(0f64.to_bits(), Ordering::Relaxed);
            // Motion V1: the view's clock stops with it (the bodies freeze mid-step, and walk on at once after)
            sh.vt_pace(0.0, world.tick as f64);
            continue;
        }
        // paced: free running past --from; flat out to --from and to a bench's hold
        let paced = (hold == u64::MAX || fighting || stepped > 0) && world.tick >= from;
        let bpt = world.config.combat.beats_per_tick.max(1);
        // T2 (docs/TIME_MODEL.md § 3): the speed now, the human's or auto-speed's over a stretch, ramped
        let eff = auto_speed(&world, &sh, &cfg, &mut auto, subject, possessed && !fighting);
        // Motion V2 (OPEN-313): one clock for the fight and the street. A beat lands every `[view] beat_s` real seconds
        // or the play speed's own beat (a tick's real time over `beats_per_tick`), the slower: at 1x a tick is 10 s and
        // a beat 1.7 s, the street walking on at 1x; faster, the fight slows the whole clock to a tick every
        // `beats_per_tick` x `beat_s`, the street walking on in slow motion. Never a frozen street, never a faster one.
        let beat_s = {
            let own = if rate * eff > 0.0 { 1.0 / (rate * eff * f64::from(bpt)) } else { 0.0 };
            cfg.beat_s.max(0.05).max(own)
        };
        let fight_clock = fighting || stepped > 0;
        let tick_rate = if fight_clock { 1.0 / beat_s } else { rate * eff };
        let walking_rate = if paced && !fight_clock {
            tick_rate
        } else if paced {
            // the real time left before the beats' tick runs (the beats to come, or the last one's wait): the view's
            // clock reaches the newest tick as it runs, so the street's walk through the pair never stops for the fight
            let wait = next_at.saturating_duration_since(now_v).as_secs_f64().min(beat_s);
            let left = if fighting && stepped < bpt { f64::from(bpt - stepped) * beat_s } else { wait }.max(0.25 * beat_s);
            let gap = (world.tick as f64 - sh.vt()).max(0.0);
            (gap / left).min((1.0 / (beat_s * f64::from(bpt))).max(rate * eff) * 1.5)
        } else {
            0.0
        };
        sh.tick_rate_bits.store(walking_rate.to_bits(), Ordering::Relaxed);
        // Motion V1: the view's clock runs at the walk's pace (no further than a tick past the newest run; V2: in a
        // fight no further than the newest, whose next tick waits for the beats)
        sh.vt_pace(walking_rate, world.tick as f64 + if fight_clock { 0.0 } else { 1.0 });
        let mut beat_now: Option<Vec<citysim::systems::combat::BeatLog>> = None;
        // Motion V2: the beats on the pacing's clock (a film's own while one runs: a fight films at its true pace)
        let nap = Duration::from_millis(if film_t.is_some() { 1 } else { 20 });
        if fighting && stepped < bpt {
            let now = now_v;
            // a beat at most `beat_s` off (a flat-out stretch before the fight leaves `next_at` anywhere)
            next_at = next_at.min(now + Duration::from_secs_f64(beat_s));
            if now < next_at {
                ready();
                std::thread::sleep((next_at - now).min(nap));
                continue;
            }
            next_at = next_at.max(now) + Duration::from_secs_f64(beat_s);
            if let Some((logs, over)) = citysim::systems::combat::beat(&mut world, subject) {
                stepped = if over { bpt } else { stepped + 1 };
                beat_now = Some(logs);
            }
        } else if stepped > 0 {
            // the stepped beats' tick: after the last beat's second, at once
            let now = now_v;
            next_at = next_at.min(now + Duration::from_secs_f64(beat_s));
            if now < next_at {
                ready();
                std::thread::sleep((next_at - now).min(nap));
                continue;
            }
        } else if paced && rate > 0.0 {
            // V1: the wait in slices (a tick is 10 real seconds at 1x), so a reload is seen between them. Motion V1:
            // the next tick is due when the view's clock reaches the newest tick (the drawn pair is played out: a
            // speed change paces the wait at once, a hold, a load or a pause owes no wait); a sim slower than that
            // runs flat out (auto-speed's cap)
            let now = now_v;
            let mut vt = sh.vt();
            if vt < world.tick as f64 - 1.5 {
                // the world jumped past the view (a frame not published on the clock): the view is at the newest
                sh.vt_set(world.tick as f64);
                vt = world.tick as f64;
            }
            if vt < world.tick as f64 {
                ready();
                // filming, the render thread steps the clock a frame at a time: look again soon
                let slice = if film_t.is_some() { Duration::from_millis(1) } else { Duration::from_millis(20) };
                let wait = Duration::from_secs_f64(((world.tick as f64 - vt) / tick_rate).min(1.0));
                std::thread::sleep(wait.min(slice));
                continue;
            }
            if film_t.is_some() {
                // the film: a tick waits for the render thread to take the last frame, so each is drawn in turn
                // (outside a film the ways of the ticks between ride the next frame, `Trail`)
                if sh.taken.load(Ordering::Relaxed) < seq {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                film_tick_at = Some(now);
            }
            next_at = Instant::now() + Duration::from_secs_f64(1.0 / tick_rate);
        } else if paced {
            // --rate 0: hold the frame
            ready();
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
        // the pacing's clock at the tick (a film's due time)
        let t_tick_v = film_tick_at.take().unwrap_or_else(|| sh.now());
        let stepping = beat_now.is_some();
        // T2 (§ 4): the real time this tick covers (since the last began, at least its interval at the speed now): the
        // published pair's lerp span
        let dt =
            if stepping { beat_s } else { t_tick_v.saturating_duration_since(last_tick_at).as_secs_f64().max(1.0 / tick_rate.max(1e-9)) };
        if !stepping {
            last_tick_at = t_tick_v;
            auto.window.0 += 1;
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
            // T2 (docs/TIME_MODEL.md § 3): someone coming for the body or speaking to it takes auto-speed back to the
            // human's speed (the auto-pause, below, stops the clock whatever the speed)
            if let Some(s) = auto.on
                && let Some(who) = coming(&world, subject, &new, cfg.autopause_tiles)
            {
                auto.latched = Some(s);
                auto.on = None;
                let line = format!("the clock came back ({s}): {who}");
                eprintln!("auto-speed {}: {line}", stamp(world.tick));
                n.push(&world, false, line);
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
        auto_asked = None;
        if !stepping {
            acc = (acc.0 + 1, acc.1 + t_tick.elapsed().as_secs_f64());
        }
        sh.tick.store(world.tick, Ordering::Relaxed);
        // publish every paced tick; flat out, only the two ticks before the hold (or --from) the lerp needs (C4: and
        // every tick with beats of a fight in the ring). T2 (docs/TIME_MODEL.md § 4): paced, a frame goes once the
        // render thread has taken the last one (or after `[view] trail_max_ticks`), the ticks between carrying their
        // ways on it, so the pair's way covers the whole path walked however fast the clock runs
        let target = if hold == u64::MAX { from } else { hold };
        if !stepping {
            trail.ticks += 1;
            trail.span_s += dt;
        }
        let beats_now = std::mem::take(&mut shown_beats);
        let behind = paced && !stepping && sh.taken.load(Ordering::Relaxed) < seq && trail.ticks < cfg.trail_max_ticks.max(1);
        if (paced && !behind) || stepping || beats_now || world.tick + 1 >= target {
            // flat out (to a hold, to --from) a frame is drawn at once
            let span = if stepping {
                beat_s
            } else if paced {
                trail.span_s
            } else {
                0.0
            };
            let line = publish(
                &world,
                &sh,
                subject,
                at,
                &cfg,
                &mut ways,
                &mut seq,
                possessed,
                news.as_mut(),
                &mut last_logged,
                &mut trail,
                (span, if stepping { sh.now() } else { t_tick_v }, paced && !stepping, stepping && paced),
            );
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
        } else if paced {
            // T2: the render thread has not drawn the last frame yet: this tick's ways go on the next one
            let ring = view::ring_about(view::subject_tile(&world, subject).unwrap_or(at), cfg.ring_half_tiles, w, h);
            let f = view::snapshot_many_cached(&world, &[ring], Some(&mut ways)).pop().expect("one ring, one frame");
            trail.extend(&world, &f, cfg.trail_repair_tiles);
        } else {
            trail.gap = true;
        }
    }
}

/// T2 (docs/TIME_MODEL.md § 3): the clock's speed now (a multiple of the rate): auto-speed's over a stretch of the
/// possessed body's (`player::stretch`) while the toggle is on and the human has not taken that stretch back (a key, a
/// click, someone came: `Auto::latched`), else the human's; ramped over `[view] auto_ramp_s`. Published with the
/// HUD's why ("asleep, 600x").
fn auto_speed(world: &World, sh: &Shared, cfg: &citysim::config::ViewCfg, auto: &mut Auto, subject: EntityId, live: bool) -> f64 {
    let human = sh.speed().max(1e-6);
    let input = *sh.input_at.lock().unwrap();
    if input > auto.input {
        auto.input = input;
        if let Some(s) = auto.on {
            auto.latched = Some(s);
            auto.on = None;
            eprintln!("auto-speed {}: back to {human}x ({s}): a key or a click", stamp(world.tick));
        }
    }
    let st = if live && sh.auto_on.load(Ordering::Relaxed) { pl::stretch(world, subject) } else { None };
    if auto.latched.is_some() && st != auto.latched {
        auto.latched = None;
    }
    let on = st.filter(|s| auto.latched != Some(*s));
    if on != auto.on {
        match (auto.on, on) {
            (_, Some(s)) => eprintln!("auto-speed {}: {s}, {:.0}x", stamp(world.tick), cfg.auto_speed.max(human)),
            (Some(s), None) => eprintln!("auto-speed {}: back to {human}x: {s} ended", stamp(world.tick)),
            _ => {}
        }
        auto.on = on;
    }
    let now = sh.now();
    auto.aim(if on.is_some() { cfg.auto_speed.max(human) } else { human }, cfg.auto_ramp_s, now);
    let eff = auto.speed(cfg.auto_ramp_s, now);
    // what the sim thread manages (the ticks of the last second)
    let secs = now.saturating_duration_since(auto.window.1).as_secs_f64();
    if secs >= 1.0 {
        let r = f64::from_bits(sh.rate_bits.load(Ordering::Relaxed)).max(1e-9);
        auto.managed = f64::from(auto.window.0) / secs / r;
        auto.window = (0, now);
    }
    let line = match on {
        Some(s) => {
            let settled = now.saturating_duration_since(auto.at).as_secs_f64() > cfg.auto_ramp_s + 1.0;
            let cap = if settled && auto.managed > 0.0 && auto.managed < 0.85 * eff {
                format!(" (the sim manages {:.0}x)", auto.managed)
            } else {
                String::new()
            };
            format!("{s}, {eff:.0}x{cap}")
        }
        None if eff > human * 1.05 => format!("slowing to {human:.0}x ({eff:.0}x)"),
        None => String::new(),
    };
    let mut l = sh.auto_line.lock().unwrap();
    if *l != line {
        *l = line;
    }
    eff
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
    trail: &mut Trail,
    // the pair's lerp span (s) and when it starts on the pacing's clock (the tick's time); Motion V1: drawn on the
    // view's clock (a paced tick; else the view's clock is set to this frame's tick)
    (span_s, at_v, clocked, beat): (f64, Instant, bool, bool),
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
    let side: Vec<u64> = world
        .combat
        .engaged
        .get(&subject)
        .and_then(|i| world.combat.engagements.get(i))
        .and_then(|e| e.sides.iter().find(|s| s.contains(&subject)))
        .map(|s| s.iter().copied().map(key).collect())
        .unwrap_or_default();
    *sh.fight_side.lock().unwrap() = side;
    // T2 (docs/TIME_MODEL.md § 4): the older frame's ways stitched to the whole path walked since it (the ticks the
    // render thread did not take, a walk begun inside the tick, a door left); flat out or after a gap, as it was
    let base = sh.slots.lock().unwrap().latest.clone();
    // Motion V2 (OPEN-313): a fight's beat inside a paced tick keeps the drawn pair (the older frame and the view's
    // clock through it): the newer frame is the beat's (the fight's state; every place is the tick's own), so the
    // street walks on through the pair while the beats land
    if beat && let Some(b) = base.as_ref().filter(|b| b.subject == subject && b.clocked && b.frame.tick == frame.tick) {
        trail.start(&frame);
        let p = Arc::new(Published {
            frame,
            subject,
            caption,
            at: b.at,
            seq: *seq,
            from_tick: b.from_tick,
            span_s: b.span_s,
            clocked: true,
            strides: b.strides.clone(),
        });
        sh.slots.lock().unwrap().latest = Some(p);
        return line;
    }
    let stitch = span_s > 0.0 && !trail.gap && base.as_ref().is_some_and(|b| b.subject == subject);
    trail.repaired = (0, 0);
    let prev = match base {
        Some(b) if stitch => Some(Arc::new(Published {
            frame: trail.stitch(world, &b.frame, &frame, cfg.trail_repair_tiles),
            subject: b.subject,
            caption: b.caption.clone(),
            at: b.at,
            seq: b.seq,
            from_tick: b.from_tick,
            span_s: b.span_s,
            clocked: b.clocked,
            strides: b.strides.clone(),
        })),
        b => b,
    };
    let from_tick = prev.as_ref().map_or(frame.tick.saturating_sub(1), |p| p.frame.tick);
    if std::env::var_os("CV_LERP_DEBUG").is_some() {
        let old = prev.as_ref().and_then(|p| p.frame.agent(subject));
        let to = frame.agent(subject).map(|a| a.t);
        let reach = old.zip(to).map(|(a, t)| a.t == t || a.ahead.last() == Some(&t));
        eprintln!(
            "lerp {}: pair {from_tick}->{} ({} ticks, {span_s:.2} s{}), ways searched {} (none {}); the subject {:?} -> {:?}, its way {} tiles reaches: {reach:?}",
            stamp(frame.tick.saturating_sub(1)),
            frame.tick,
            frame.tick.saturating_sub(from_tick),
            if stitch { ", stitched" } else { "" },
            trail.repaired.0,
            trail.repaired.1,
            old.map(|a| a.t),
            to,
            old.map_or(0, |a| a.ahead.len())
        );
    }
    trail.start(&frame);
    if !clocked {
        sh.vt_set(frame.tick as f64);
    }
    let strides = frame.agents.iter().filter_map(|a| citysim::exec::stride(world, a.id).map(|s| (key(a.id), s))).collect();
    let p = Arc::new(Published { frame, subject, caption, at: at_v, seq: *seq, from_tick, span_s: span_s as f32, clocked, strides });
    let mut s = sh.slots.lock().unwrap();
    s.prev = prev;
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
        Some(b) => format!("in {}, floor {}", citysim::systems::player::building_phrase(world, b), p.floor),
        None => format!("on a street in {}", citysim::systems::player::area_of(world, p.tile)),
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
        tenure: citysim::systems::housing::tenure_line(world, id),
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
            out = Some(plain_failure(f.split_once(": ").map_or(f.as_str(), |x| x.1)));
        }
        last.1 = hud.failure.clone();
    }
    if hud.autopilot != last.2 {
        eprintln!("player {at}: autopilot {}", if hud.autopilot { "on" } else { "off" });
        last.2 = hud.autopilot;
    }
    out
}

/// P5b: `EatFromInventory` as words: "eat what you carry"; a few by hand, the rest the name's camel case spaced.
fn plain_step(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(k) = raw.strip_prefix("GoTo(").and_then(|r| r.strip_suffix(')')) {
        return format!("go to the {}", camel_words(k));
    }
    match raw {
        "EatFromInventory" => "eat what you carry".to_string(),
        "BuyFood" => "buy food".to_string(),
        _ => camel_words(raw.split('(').next().unwrap_or(raw)),
    }
}

fn camel_words(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// P5b: a plan failure ("Some(Treat) failed at GoTo(Clinic): BuildingFull") in plain words ("the clinic was full
/// (going for: treat)"); a line that is not that shape (a refusal) goes through.
pub fn plain_failure(raw: &str) -> String {
    let Some((goal, rest)) = raw.split_once(" failed at ") else { return raw.to_string() };
    let (step, why) = rest.split_once(": ").unwrap_or((rest, ""));
    let reason = why.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("");
    let goal = goal.trim_start_matches("Some(").trim_end_matches(')');
    let place = step.strip_prefix("GoTo(").and_then(|r| r.strip_suffix(')')).map(camel_words);
    let what = place.as_ref().map_or_else(|| plain_step(step), |p| format!("the {p}"));
    let said = match (reason, place.is_some()) {
        ("BuildingFull", true) => format!("{what} was full"),
        ("NoSuchPlace", true) => format!("there was no {what} to be found"),
        ("StockGone", _) => format!("{what} had run out"),
        ("PartnerLeft", _) => format!("{what}: they left"),
        ("Timeout", _) => format!("{what} took too long"),
        ("PreconditionLost", _) => format!("{what} could not be done any more"),
        _ => format!("{what} failed"),
    };
    if goal == "None" { said } else { format!("{said} (going for: {})", camel_words(goal)) }
}

/// P5b: an event as one line of the player's story: its own deeds without the kind tag or the raw plan,
/// the buildings in plain words.
fn plain_event(world: &World, e: &citysim::events::Event) -> String {
    use citysim::events::EventKind as K;
    let line = if matches!(e.kind, K::PlayerAction | K::PlayerActionFailed) {
        // "Name: the rest": the name is the player's own
        let t = e.text.split_once(": ").map_or(e.text.as_str(), |x| x.1);
        let list = |steps: &str| steps.split(", ").map(plain_step).collect::<Vec<_>>().join(", then ");
        if let Some(steps) = t.strip_prefix("directed [").and_then(|r| r.strip_suffix(']')) {
            format!("you {}", list(steps))
        } else if let Some((goal, steps)) = t.split_once(" planned [") {
            format!("you set about {}: {}", camel_words(goal), list(steps.trim_end_matches(']')))
        } else if t.starts_with("possessed") {
            "you take the wheel".to_string()
        } else if t.starts_with("autopilot on") {
            "autopilot on: the body looks after itself".to_string()
        } else if t.starts_with("autopilot off") {
            "autopilot off: it is yours".to_string()
        } else if t == "stopped" {
            "you stop".to_string()
        } else if t.ends_with(']')
            && let Some((label, _steps)) = t.rsplit_once(" [")
        {
            // "Eat what you carry (1) [EatFromInventory]": the label says it, the raw steps stay in the stderr log
            label.to_string()
        } else {
            t.to_string()
        }
    } else {
        format!("[{:?}] {}", e.kind, e.text)
    };
    citysim::systems::player::plain_buildings(world, &line)
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
        let line = citysim::systems::player::plain_buildings(world, &line);
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
        let line = plain_event(world, e);
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
