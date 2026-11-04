//! M18 P1, the seam, and P2, the verbs (docs/M18_PLAYER.md): a human drives the followed body (`--save FILE
//! --possess INDEX`).
//!
//! The HUD is egui (`bevy_egui`), bottom right (`[view] hud_width_px`; P1's top-right panel ate the far field's
//! clicks): the clock, the needs, the coins, the job, where the body is, its goal and current plan (the step under
//! way marked `>`, the menu row it came from) and the plan's last failure, with the rows the sim says the body can
//! do where it stands (`citysim::systems::player::menu_self`: eat what you carry, eat at home, sleep, work), Stop,
//! autopilot, pause and the speed.
//!
//! A left click picks a drawn body (its chest within `[view] pick_body_px`, people.rs' bodies), else casts the camera's
//! ray against the ground and the lots' massing boxes (lots.rs; the cutaway's box around the subject is passed
//! through) and sends the tile it hits. The sim answers a body or a building with a menu of only its legal rows
//! (`player::menu_body`, `menu_building`: the door, then the verbs), drawn at the click; a row's hover lists the will
//! terms the verb waives. The street is walked to. Keys: T autopilot, Space pause, `,` / `.` the play speed
//! (`[view] play_speeds`), Backspace stop, Escape closes the menu.
//!
//! `--play "T action; T action; .."` scripts a session through the same paths (a play session for a proof): at the
//! first frame whose tick is T or later (`D:HH:MM` or a tick) it runs `click TX,TY` (the tile's top projected to the
//! screen, then the click's own pick), `body INDEX|adult|child` (that drawn body, or the nearest adult or child to the
//! subject: its chest projected, then the click's pick), `bld INDEX` (M18 P3: a building's menu, as a click on it),
//! `await` (the click's menu open), `pick TEXT` (the open
//! menu's first row containing TEXT; it waits for the menu), `do TEXT` (a HUD row), `close` (the menu), `zoom M`, `pitch DEG`, `yaw DEG`,
//! `goal NAME`, `stop`, `auto on|off`, `pause`, `speed I`, `dismiss` (M18 P4: the day-end summary closed), `shot NAME` (a
//! PNG with the HUD and the menu into `--out`) or `quit`.
//!
//! M18 P4 (consequences): the player's own log (top right: the body's events, its failures, and the city's doings to
//! it in yellow: a grudge, a Hunt or a record on it formed, word of a deed of its reaching more people), "who knows
//! about you" under it (`citysim::systems::player_news`: the bodies whose memories name the player, how they heard,
//! the talk, who is after it), the day-end summary at the top centre (coins, who now knows you, who is after you, the
//! street's read, your block) and the death screen from the body's `Life`.
//!
//! C4 (the player's fight, docs/COMBAT.md § 5): the fight HUD (top left, while the body is in a live fight: the sides
//! and their stances, what their wounds show, your stance and orders as `UiCmd::Fight`, your wounds, blood and
//! fatigue, the morale hints, the chrome to hand, an Attack row a foe with its odds, a Help row a downed body), the
//! wounds panel in its place outside a fight (days to heal, what a patch-up costs and where, "Get patched up"), the
//! beat replay (`replay`: each tick's beats a `beat_s` apart as words over the bodies, a flash on a telling blow)
//! and the HUD's stance and holster for the next fight. Script: `order stance allout|defensive|subdue|holdback`,
//! `order attack INDEX`, `order draw|holster|cover|flee|yield`, `order help INDEX`.
//!
//! C5 (the feel pass): the fight's beats are stepped one a `[view] beat_s` (simthread), so the replay shows each beat
//! as it lands; a swing is a lunge at the struck body and a hit a stagger away from the striker (`Pose`, drawn by
//! sim.rs on the existing rig: a lean and a step, `[view] lunge_s` / `lunge_m` / `lunge_deg`); the Attack rows read
//! the duel's odds and the whole fight's; Take cover shows only with a gun in the fight, Holster only with a weapon in
//! hand. Script: `wait SECS` (the sim runs on that long: a fight's beats a second apart).
//!
//! M18 P5 (start and loop, `--slice`): the egui start menu (New: pick a nobody or arrive fresh; Load; Quit), the
//! Esc menu in play (Resume, Save, Load, Quit to menu, Quit), the death screen's "pick another nobody" into the same
//! running city, the auto-pause's tick box and banner, and the cold-start help line. Script: `menu main|pick|load|esc|
//! game`, `nobody I` (play the I-th nobody of the list), `look I`, `fresh`, `newcity`, `save NAME`, `load NAME`,
//! `provoke [lethal]` (the adult near you with the best odds goes for you: a proof's cheat), `unpause`, `autopause
//! on|off`, `await idle|player`; a line's tick `*` is the line before's (the next frame).
//!
//! T2 (docs/TIME_MODEL.md, the clock): the speed row reads multiples of the walk-matched rate (1x a body walks
//! `[view] walk_m_s`), on one line, and while auto-speed runs why ("auto: asleep, 600x"); auto-speed's tick box sits
//! by the auto-pause's in the HUD and in the Esc menu. Script: `live SECS` (the sim runs paced at the speed for that
//! many real seconds, auto-speed and all, where the other lines hold it to their ticks), `autospeed on|off`, `click
//! ~DX,DY` (the tile that far from the subject's).
//!
//! Films (film.rs, docs/TESTING.md "Filming motion"): `film NAME SECS [FPS]` runs the sim live at the play speed for
//! SECS film seconds, like `live`, writing FPS (10) frames a second to `<out>/NAME/frame_00001.png` .. (the `shot`
//! capture, HUD and all) on a fixed-step clock (each frame 1/FPS of game-real time, however slow the capture), with the
//! motion trace `trace.csv` and `meta.json`; the next line runs once the frames are written.

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::PrimaryWindow,
};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, input::EguiWantsInput};
use citysim::components::{GoalKind, TilePos};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::simthread::{Shared, UiCmd};

/// One `--play` line.
#[derive(Clone, Debug)]
pub struct Step {
    pub tick: u64,
    pub action: String,
}

/// A script `pick` gives up on its menu after this many frames.
const PICK_WAIT_FRAMES: u32 = 600;

#[derive(Resource)]
pub struct Play {
    pub shared: Arc<Shared>,
    pub speeds: Vec<f64>,
    pub speed_i: usize,
    start_speed_i: usize,
    pub reach_m: f32,
    pub step_m: f32,
    /// The cutaway's box (`[view] cut_width_m`, `cut_above_m`): a click's ray passes through it.
    pub cut_width_m: f32,
    pub cut_above_m: f32,
    pub cut_max_zoom_m: f32,
    pub script: Vec<Step>,
    pub next: usize,
    pub out: PathBuf,
    /// A quit waits this many frames (the last shot's write).
    quit_in: Option<u32>,
    /// M18 P2: a click this near a body's chest (px) picks the body; the HUD's width.
    pub pick_body_px: f32,
    pub hud_width_px: f32,
    /// M18 P2: the last click's pixel (its menu opens there) and the menu seq published before it.
    click_px: Vec2,
    menu_asked: u64,
    /// M18 P2: the open menu (its seq, where it is drawn); `None` closed.
    menu_open: Option<(u64, Vec2)>,
    /// M18 P2: frames a script `pick` has waited for its menu.
    wait: u32,
    /// M18 P4: the day-end summaries closed so far (the newest past it is drawn).
    days_seen: usize,
    /// C4: the replay's cues, the beats seq taken, how long a cue floats.
    cues: Vec<Cue>,
    beats_seq: u64,
    cue_s: f32,
    /// C5: a script's `wait SECS`: the sim runs on (a fight's beats a `beat_s` apart) until then.
    wait_until: Option<(Instant, u64)>,
    /// C5: the procedural poses of the beat replay (a lunge, a stagger) by body key, from `at`.
    pub poses: Vec<Pose>,
    lunge_s: f32,
    /// M18 P5: the start menu (`--slice`), the screen up, the save's name field, the clock's state when Esc opened,
    /// the possession whose panels show (a new one closes the old day-ends), the player saves' directory.
    pub slice: bool,
    pub screen: Screen,
    save_name: String,
    esc_was_paused: bool,
    epoch_seen: u64,
    /// M18 P5: a load was sent: once done, play goes on if the save held a body, else the pick.
    pending_load: bool,
    saves_dir: PathBuf,
    /// T2: a script's `live SECS`: paced play until then.
    live_until: Option<Instant>,
}

/// C5: a body's procedural pose on the existing rig: a lunge at `toward` or a stagger away from it.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub key: u64,
    pub toward: u64,
    pub lunge: bool,
    pub at: Instant,
}

/// `"T action; T action"`: the ticks a number or `D:HH:MM`, in rising order.
pub fn parse_script(s: &str, tick: impl Fn(&str) -> u64) -> Vec<Step> {
    let steps: Vec<Step> = s
        .split(';')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (t, a) = l.split_once(' ').unwrap_or_else(|| panic!("--play '{l}': 'TICK ACTION'"));
            // M18 P5: `*` is the line before's tick (the next frame)
            Step { tick: if t == "*" { u64::MAX } else { tick(t) }, action: a.trim().to_string() }
        })
        .collect();
    let mut steps = steps;
    let mut last = 0;
    for st in steps.iter_mut() {
        if st.tick == u64::MAX {
            st.tick = last;
        }
        last = st.tick;
    }
    assert!(steps.windows(2).all(|w| w[0].tick <= w[1].tick), "--play: ticks in rising order");
    steps
}

pub fn add(app: &mut App, shared: Arc<Shared>, vcfg: &citysim::config::ViewCfg, script: Vec<Step>, out: PathBuf, slice: Option<PathBuf>) {
    let speeds = if vcfg.play_speeds.is_empty() { vec![1.0] } else { vcfg.play_speeds.clone() };
    app.add_plugins(EguiPlugin::default())
        .insert_resource(Play {
            shared,
            speeds,
            speed_i: 0,
            start_speed_i: vcfg.start_speed_i,
            reach_m: vcfg.pick_reach_m,
            step_m: vcfg.pick_step_m.max(0.05),
            cut_width_m: vcfg.cut_width_m,
            cut_above_m: vcfg.cut_above_m,
            cut_max_zoom_m: vcfg.cut_max_zoom_m,
            script,
            next: 0,
            out,
            quit_in: None,
            pick_body_px: vcfg.pick_body_px,
            hud_width_px: vcfg.hud_width_px,
            click_px: Vec2::ZERO,
            menu_asked: 0,
            menu_open: None,
            wait: 0,
            days_seen: 0,
            cues: Vec::new(),
            beats_seq: 0,
            cue_s: vcfg.cue_s,
            wait_until: None,
            poses: Vec::new(),
            lunge_s: vcfg.lunge_s,
            slice: slice.is_some(),
            screen: if slice.is_some() { Screen::Main } else { Screen::Game },
            save_name: String::new(),
            esc_was_paused: false,
            epoch_seen: 0,
            pending_load: false,
            saves_dir: slice.unwrap_or_default(),
            live_until: None,
        })
        .add_systems(Update, (keys, click, run_script))
        .add_systems(EguiPrimaryContextPass, (hud, menu, board, news, day_end, death, fight, wounds, replay, start_menu, help));
    app.add_systems(Update, hide_debug.run_if(|p: Res<Play>| p.slice));
}

fn set_speed(p: &mut Play, i: usize) {
    p.speed_i = i.min(p.speeds.len() - 1);
    p.shared.speed_bits.store(p.speeds[p.speed_i].to_bits(), Ordering::Relaxed);
}

fn autopilot_now(p: &Play) -> bool {
    p.shared.player.lock().unwrap().as_ref().is_some_and(|h| h.autopilot)
}

fn keys(keys: Res<ButtonInput<KeyCode>>, buttons: Res<ButtonInput<MouseButton>>, mut p: ResMut<Play>) {
    // M18 P5: the human is here (the auto-pause's "away")
    if keys.get_just_pressed().next().is_some() || buttons.get_just_pressed().next().is_some() {
        *p.shared.input_at.lock().unwrap() = Instant::now();
    }
    // M18 P5: Esc closes the click's menu, else opens or closes the Esc menu (the slice)
    if keys.just_pressed(KeyCode::Escape) {
        if p.menu_open.is_some() {
            p.menu_open = None;
        } else if p.slice && p.screen == Screen::Game && p.shared.slice.lock().unwrap().started {
            open_esc(&mut p);
        } else if p.screen == Screen::Esc {
            close_esc(&mut p);
        }
        return;
    }
    if p.screen != Screen::Game {
        return;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        let on = !autopilot_now(&p);
        p.shared.send(UiCmd::Autopilot(on));
    }
    if keys.just_pressed(KeyCode::Space) {
        let was = p.shared.paused.load(Ordering::Relaxed);
        p.shared.paused.store(!was, Ordering::Relaxed);
    }
    if keys.just_pressed(KeyCode::Comma) {
        let i = p.speed_i.saturating_sub(1);
        set_speed(&mut p, i);
    }
    if keys.just_pressed(KeyCode::Period) {
        let i = p.speed_i + 1;
        set_speed(&mut p, i);
    }
    if keys.just_pressed(KeyCode::Backspace) {
        p.shared.send(UiCmd::Stop);
    }
}

/// Bevy (x, z, -y) -> city metres (x east, y north, z up).
fn city(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y)
}

/// The camera's ray through `px` against the ground and the lots' boxes: the city point it hits (metres) and
/// whether it is a building's (a lot box) rather than the ground.
fn pick(
    px: Vec2,
    cam: (&Camera, &GlobalTransform),
    p: &Play,
    sim: &crate::sim::Sim,
    stream: &crate::stream::Stream,
    zoom: f32,
) -> Option<(Vec3, bool)> {
    let ray = cam.0.viewport_to_world(cam.1, px).ok()?;
    let (o, d) = (city(ray.origin), city(*ray.direction));
    let eye = Vec2::new(o.x, o.y);
    // the cutaway's box (occ.rs / glclient::occlusion): from the subject toward the eye, cut_width wide, over the
    // subject's feet + cut_above; a ray passing there passes through walls that are not drawn
    let feet = sim.subject_at.map(|(f, _)| f);
    let in_cut = |q: Vec3| {
        let Some(f) = feet.filter(|_| zoom <= p.cut_max_zoom_m) else { return false };
        if q.z < f.z + p.cut_above_m {
            return false;
        }
        let (a, b) = (Vec2::new(f.x, f.y), eye);
        let ab = b - a;
        let u = ((Vec2::new(q.x, q.y) - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        u > 0.0 && (a + ab * u).distance(Vec2::new(q.x, q.y)) <= p.cut_width_m / 2.0
    };
    let mut t = 0.0;
    while t < p.reach_m {
        let q = o + d * t;
        if let Some(g) = stream.ground_z(q.x, q.y)
            && q.z <= g
        {
            return Some((Vec3::new(q.x, q.y, g), false));
        }
        if !in_cut(q) {
            for (i, _, _) in sim.lots.under([q.x - 0.01, q.y - 0.01, q.x + 0.01, q.y + 0.01]) {
                let l = &sim.lots.lots[i];
                let hit = if l.boxes.is_empty() {
                    q.z >= l.z_m && q.z <= l.roof()
                } else {
                    l.boxes.iter().any(|b| q.x >= b[0] && q.x <= b[2] && q.y >= b[1] && q.y <= b[3] && q.z >= b[4] && q.z <= b[5])
                };
                if hit {
                    return Some((q, true));
                }
            }
        }
        t += p.step_m;
    }
    None
}

/// City metres -> the sim's tile (sim.rs `metres`' inverse).
fn tile_of(sim: &crate::sim::Sim, q: Vec3) -> Option<TilePos> {
    let x = ((q.x - sim.x0) / sim.tile_m).floor();
    let y = ((sim.y1 - q.y) / sim.tile_m).floor();
    (x >= 0.0 && y >= 0.0 && x < 65535.0 && y < 65535.0).then_some(TilePos { x: x as u16, y: y as u16 })
}

#[allow(clippy::too_many_arguments)]
fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<crate::cam::CityCam>>,
    egui: Option<Res<EguiWantsInput>>,
    mut p: ResMut<Play>,
    sim: Res<crate::sim::Sim>,
    stream: Res<crate::stream::Stream>,
    view: Res<crate::cam::View>,
) {
    if !buttons.just_pressed(MouseButton::Left) || egui.is_some_and(|e| e.wants_any_pointer_input()) || p.screen != Screen::Game {
        return;
    }
    let Some(px) = windows.single().ok().and_then(|w| w.cursor_position()) else { return };
    let Ok(cam) = cams.single() else { return };
    send_pick(px, cam, &mut p, &sim, &stream, view.zoom, "click");
}

/// M18 P2: the drawn body whose chest projects nearest `px`, within `[view] pick_body_px`.
fn body_at(px: Vec2, cam: (&Camera, &GlobalTransform), p: &Play, sim: &crate::sim::Sim) -> Option<(u64, f32)> {
    sim.people
        .iter()
        .filter_map(|b| {
            let q = cam.0.world_to_viewport(cam.1, chest(b)).ok()?;
            let d = q.distance(px);
            (d <= p.pick_body_px).then_some((b.id, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// A body's chest, Bevy coordinates.
fn chest(b: &crate::people::BodyDraw) -> Vec3 {
    crate::mesh::bv(b.at.x, b.at.y, b.at.z + 1.2)
}

/// The view's body key (sim.rs `ekey`) -> the entity.
fn entity_of(key: u64) -> citysim::EntityId {
    citysim::EntityId { index: (key & 0xffff_ffff) as u32, generation: (key >> 32) as u32 }
}

fn send_pick(
    px: Vec2,
    cam: (&Camera, &GlobalTransform),
    p: &mut Play,
    sim: &crate::sim::Sim,
    stream: &crate::stream::Stream,
    zoom: f32,
    why: &str,
) {
    // a click closes the open menu; a body or a building asks for a new one, opened at the click
    p.menu_open = None;
    p.click_px = px;
    p.menu_asked = p.shared.menu.lock().unwrap().as_ref().map_or(0, |m| m.seq);
    if let Some((key, d)) = body_at(px, cam, p, sim) {
        let e = entity_of(key);
        info!("play {why} at px ({:.0}, {:.0}): a body #{} ({d:.0} px off)", px.x, px.y, e.index);
        p.shared.send(UiCmd::Body(e));
        return;
    }
    match pick(px, cam, p, sim, stream, zoom).and_then(|(q, roof)| tile_of(sim, q).map(|t| (q, roof, t))) {
        Some((q, roof, t)) => {
            let what = if roof { "a building" } else { "the ground" };
            info!(
                "play {why} at px ({:.0}, {:.0}): hit {what} at ({:.1}, {:.1}, {:.1}) m, tile ({}, {})",
                px.x, px.y, q.x, q.y, q.z, t.x, t.y
            );
            p.shared.send(UiCmd::Click(t, roof));
        }
        None => info!("play {why} at px ({:.0}, {:.0}): the ray hit nothing", px.x, px.y),
    }
}

fn goal_of(name: &str) -> Option<GoalKind> {
    serde_json::from_str(&format!("\"{name}\"")).ok()
}

/// M18 P2: the open menu's (or the HUD's) first row whose label holds `text`.
fn row_with(rows: &[crate::simthread::MenuRow], text: &str) -> Option<citysim::systems::player::Verb> {
    rows.iter().find(|r| r.0.contains(text) && r.3.is_none()).map(|r| r.1)
}

#[allow(clippy::too_many_arguments)]
fn run_script(
    mut commands: Commands,
    mut p: ResMut<Play>,
    cams: Query<(&Camera, &GlobalTransform), With<crate::cam::CityCam>>,
    sim: Res<crate::sim::Sim>,
    stream: Res<crate::stream::Stream>,
    mut view: ResMut<crate::cam::View>,
    mut exit: MessageWriter<AppExit>,
    mut film: Option<ResMut<crate::film::Film>>,
) {
    if let Some(n) = p.quit_in.as_mut() {
        *n = n.saturating_sub(1);
        if *n == 0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    // the frame's tick: the HUD's state was published with it
    let Some(tick) = sim.clock_tick else { return };
    // a `film`: paced play on the film's clock until its frames are written
    if film.as_ref().is_some_and(|f| f.busy()) {
        p.shared.hold.store(u64::MAX, Ordering::Relaxed);
        return;
    }
    // T2: a `live SECS`: paced play (no hold) until then, auto-speed and all
    if let Some(u) = p.live_until {
        if Instant::now() < u {
            p.shared.hold.store(u64::MAX, Ordering::Relaxed);
            return;
        }
        p.live_until = None;
        info!("play script live: over at frame {tick}");
    }
    // C5: a `wait SECS`: the sim runs on to the next later line's tick (a fight's beats a `beat_s` apart) until then
    if let Some((u, later)) = p.wait_until {
        if Instant::now() < u {
            p.shared.hold.store(later, Ordering::Relaxed);
            return;
        }
        p.wait_until = None;
    }
    // M18 P2: the sim is held at the next line's tick (`Shared::hold`, the bench's hold): a line runs at its own tick,
    // not wherever the sim had got to by the frame (at the fast speeds that was tens of minutes late), and the sim runs
    // flat out between lines. Without a script the hold is never touched.
    if !p.script.is_empty() {
        let hold = p.script.get(p.next).map_or(u64::MAX, |s| s.tick);
        p.shared.hold.store(hold, Ordering::Relaxed);
    }
    if p.next >= p.script.len() || p.script[p.next].tick > tick {
        return;
    }
    let step = p.script[p.next].clone();
    p.next += 1;
    let (verb, arg) = step.action.split_once(' ').map_or((step.action.as_str(), ""), |(v, a)| (v, a.trim()));
    if p.wait == 0 {
        info!("play script tick {} (frame {tick}): {}", step.tick, step.action);
    }
    match verb {
        "click" => {
            // T2: `click ~DX,DY`: that far from the subject's tile
            let (rel, arg) = arg.strip_prefix('~').map_or((false, arg), |a| (true, a));
            let (x, y) = arg.split_once(',').expect("click TX,TY");
            let mut t: [f32; 2] = [x.trim().parse().expect("click TX"), y.trim().parse().expect("click TY")];
            if rel {
                let Some(me) = sim.subject.and_then(|k| sim.frames.last()?.agents.iter().find(|a| a.id == k).map(|a| a.t)) else {
                    warn!("play script click ~{arg}: no subject drawn");
                    return;
                };
                t = [me[0] + t[0], me[1] + t[1]];
                info!("play script click ~{arg}: the tile ({}, {})", t[0], t[1]);
            }
            let m = Vec2::new(sim.x0 + (t[0] + 0.5) * sim.tile_m, sim.y1 - (t[1] + 0.5) * sim.tile_m);
            // the top of what stands there (a lot's box, a roof), else the ground: what a player clicks on
            let ground = stream.ground_z(m.x, m.y).unwrap_or(view.ground_z);
            let z = sim
                .lots
                .under([m.x - 0.01, m.y - 0.01, m.x + 0.01, m.y + 0.01])
                .iter()
                .map(|(i, _, _)| &sim.lots.lots[*i])
                .flat_map(|l| {
                    let roof = l.roof();
                    l.boxes
                        .iter()
                        .filter(|b| m.x >= b[0] && m.x <= b[2] && m.y >= b[1] && m.y <= b[3])
                        .map(|b| b[5])
                        .chain(l.boxes.is_empty().then_some(roof))
                        .collect::<Vec<_>>()
                })
                .fold(ground, f32::max);
            let Ok(cam) = cams.single() else { return };
            // a pixel off the window is no click a player could make
            let size = cam.0.logical_viewport_size().unwrap_or(Vec2::splat(f32::MAX));
            let on = |q: &Vec2| q.x >= 0.0 && q.y >= 0.0 && q.x < size.x && q.y < size.y;
            match cam.0.world_to_viewport(cam.1, crate::mesh::bv(m.x, m.y, z)).ok().filter(on).ok_or("off the window") {
                Ok(px) => send_pick(px, cam, &mut p, &sim, &stream, view.zoom, "script click"),
                Err(e) => info!("play script click ({}, {}): not on the screen ({e:?})", t[0], t[1]),
            }
        }
        // M18 P5h: `bld near hostel|corp|city`: the nearest such desk's menu (`housing::script_target`)
        "bld" if arg.starts_with("near") => {
            p.menu_open = None;
            p.click_px = Vec2::new(480.0, 300.0);
            let asked = p.shared.menu.lock().unwrap().as_ref().map_or(0, |m| m.seq);
            p.menu_asked = asked;
            p.shared.send(UiCmd::BldNear(arg.trim_start_matches("near").trim().to_string()));
        }
        // M18 P3: a building's menu by its entity (the menu a click on it opens), at the screen's centre
        "bld" => {
            let index: u32 = arg.parse().expect("bld INDEX");
            p.menu_open = None;
            p.click_px = Vec2::new(480.0, 300.0);
            let asked = p.shared.menu.lock().unwrap().as_ref().map_or(0, |m| m.seq);
            p.menu_asked = asked;
            p.shared.send(UiCmd::Bld(citysim::EntityId { index, generation: 0 }));
        }
        // C4: `body best friends|weak|strong` (C5: `brave`, `out`): a target picked by the sim's odds (simthread `best_body`)
        "body" if arg.starts_with("best") => {
            p.menu_open = None;
            p.click_px = Vec2::new(480.0, 300.0);
            let asked = p.shared.menu.lock().unwrap().as_ref().map_or(0, |m| m.seq);
            p.menu_asked = asked;
            p.shared.send(UiCmd::BodyBest(arg.trim_start_matches("best").trim().to_string()));
        }
        // C4: `body #INDEX`: that body's menu wherever it is (a target off the screen; the Attack walks there)
        "body" if arg.starts_with('#') => {
            let index: u32 = arg[1..].parse().expect("body #INDEX");
            p.menu_open = None;
            p.click_px = Vec2::new(480.0, 300.0);
            let asked = p.shared.menu.lock().unwrap().as_ref().map_or(0, |m| m.seq);
            p.menu_asked = asked;
            p.shared.send(UiCmd::BodyIndex(index));
        }
        "body" => {
            // INDEX, or `adult` / `child`: the drawn adult or child nearest the subject
            let Ok(cam) = cams.single() else { return };
            let feet = sim.subject_at.map_or(Vec3::ZERO, |(f, _)| f);
            let near = |child: bool| {
                sim.people.iter().filter(|b| (b.age < 18) == child).min_by(|a, b| a.at.distance(feet).total_cmp(&b.at.distance(feet)))
            };
            let body = match arg {
                "adult" => near(false),
                "child" => near(true),
                n => {
                    let index: u32 = n.parse().expect("body INDEX|adult|child");
                    sim.people.iter().find(|b| entity_of(b.id).index == index)
                }
            };
            let px = body.and_then(|b| cam.0.world_to_viewport(cam.1, chest(b)).ok());
            match px {
                Some(px) => send_pick(px, cam, &mut p, &sim, &stream, view.zoom, "script body"),
                None => info!("play script body {arg}: not drawn on the screen"),
            }
        }
        "pick" | "do" => {
            let found = if verb == "do" {
                // M18 P3: the HUD's rows, then the board's and the offers'
                p.shared
                    .player
                    .lock()
                    .unwrap()
                    .as_ref()
                    .and_then(|h| row_with(&h.rows, arg).or_else(|| row_with(&h.offers, arg)).or_else(|| row_with(&h.board, arg)))
            } else {
                let m = p.shared.menu.lock().unwrap().clone();
                m.filter(|m| p.menu_open.is_some_and(|(s, _)| s == m.seq)).and_then(|m| row_with(&m.rows, arg))
            };
            match found {
                Some(v) => {
                    info!("play script {verb} '{arg}': {v:?}");
                    p.shared.send(UiCmd::Act(v));
                    p.menu_open = None;
                    p.wait = 0;
                }
                None if p.wait < PICK_WAIT_FRAMES => {
                    // the menu (or the row) is a frame or two behind the click: this line again next frame
                    p.wait += 1;
                    p.next -= 1;
                }
                None => {
                    warn!("play script {verb} '{arg}': no such row after {PICK_WAIT_FRAMES} frames");
                    p.wait = 0;
                }
            }
        }
        // M18 P5: `await idle` (no load or save under way), `await player` (a body possessed and its HUD up)
        "await" if arg == "idle" || arg == "player" => {
            let ready = if arg == "idle" {
                !p.shared.slice.lock().unwrap().busy && p.shared.cmds.lock().unwrap().is_empty()
            } else {
                p.shared.player.lock().unwrap().is_some() && p.shared.slice.lock().unwrap().started
            };
            if !ready && p.wait < PICK_WAIT_FRAMES * 10 {
                p.wait += 1;
                p.next -= 1;
            } else {
                if !ready {
                    warn!("play script await {arg}: not after {} frames", PICK_WAIT_FRAMES * 10);
                }
                p.wait = 0;
            }
        }
        // the click's menu open (a shot of it next)
        "await" => {
            if p.menu_open.is_none() && p.wait < PICK_WAIT_FRAMES {
                p.wait += 1;
                p.next -= 1;
            } else {
                if p.menu_open.is_none() {
                    warn!("play script await: no menu after {PICK_WAIT_FRAMES} frames");
                }
                p.wait = 0;
            }
        }
        "close" => p.menu_open = None,
        // M18 P4: the day-end summary closed
        "dismiss" => {
            let n = p.shared.news.lock().unwrap().days.len();
            p.days_seen = n;
        }
        "zoom" => {
            let z: f32 = arg.parse().expect("zoom METRES");
            view.set_zoom(z);
        }
        // M18 P2: the camera's pitch and yaw, degrees (Z/X and Q/E): a near top-down look clicks the lot it means
        "pitch" => view.set_pitch(arg.parse::<f32>().expect("pitch DEG").to_radians()),
        "yaw" => view.game_yaw = arg.parse::<f32>().expect("yaw DEG").to_radians().rem_euclid(std::f32::consts::TAU),
        "goal" => match goal_of(arg) {
            Some(g) => p.shared.send(UiCmd::Goal(g)),
            None => warn!("play script: no goal '{arg}'"),
        },
        // C4: a fight order (`order stance subdue`, `order attack INDEX`, `order flee`, `order help INDEX`)
        "order" => {
            use citysim::systems::combat::{Order, Stance};
            let (what, rest) = arg.split_once(' ').map_or((arg, ""), |(a, b)| (a, b.trim()));
            let hud = p.shared.player.lock().unwrap().clone();
            // a body by its index: one in the fight (or downed in it), else one drawn
            let body = |s: &str| -> Option<citysim::EntityId> {
                let index: u32 = s.parse().ok()?;
                let f = hud.as_ref().and_then(|h| h.fight.as_ref());
                f.and_then(|f| f.sides.iter().flatten().map(|m| m.id).chain(f.downed.iter().map(|d| d.0)).find(|e| e.index == index))
                    .or_else(|| sim.people.iter().map(|b| entity_of(b.id)).find(|e| e.index == index))
            };
            let o = match what {
                "stance" => match rest {
                    "allout" => Some(Order::Stance(Stance::AllOut)),
                    "defensive" => Some(Order::Stance(Stance::Defensive)),
                    "subdue" => Some(Order::Stance(Stance::Subdue)),
                    "holdback" => Some(Order::Stance(Stance::HoldBack)),
                    _ => None,
                },
                "attack" => body(rest).map(Order::Attack),
                "help" => body(rest).map(Order::Help),
                "draw" => Some(Order::Draw),
                "holster" => Some(Order::Holster),
                "cover" => Some(Order::Cover),
                "flee" => Some(Order::Flee),
                "yield" => Some(Order::Yield),
                _ => None,
            };
            match o {
                Some(o) => {
                    info!("play script order {arg}: {o:?}");
                    p.shared.send(UiCmd::Fight(o));
                }
                None => warn!("play script order '{arg}': no such order or body"),
            }
        }
        "stop" => p.shared.send(UiCmd::Stop),
        "auto" => p.shared.send(UiCmd::Autopilot(arg == "on")),
        "pause" => {
            let was = p.shared.paused.load(Ordering::Relaxed);
            p.shared.paused.store(!was, Ordering::Relaxed);
        }
        "speed" => {
            let i = arg.parse().expect("speed I");
            set_speed(&mut p, i);
        }
        "shot" => {
            std::fs::create_dir_all(&p.out).ok();
            let path = p.out.join(format!("{arg}.png"));
            if let Some(h) = p.shared.player.lock().unwrap().as_ref() {
                info!(
                    "play shot {}: d{} {} {} | goal {} | plan {} | {} | {} | coins {} | fail {:?}",
                    path.display(),
                    h.day,
                    h.clock,
                    h.name,
                    h.goal,
                    h.plan,
                    h.place,
                    h.tenure,
                    h.coins,
                    h.failure
                );
            }
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        }
        // C5: let the sim run on this many real seconds (a fight's beats land a `beat_s` apart)
        "wait" => {
            let secs: f32 = arg.parse().expect("wait SECS");
            // the hold: the next later line's tick as the wait begins
            let later = p.script[p.next..].iter().map(|s| s.tick).find(|&t| t > tick).unwrap_or(u64::MAX);
            p.wait_until = Some((Instant::now() + Duration::from_secs_f32(secs.max(0.0)), later));
        }
        "quit" => p.quit_in = Some(30),
        // M18 P5: the start menu's screens, a nobody played or looked at, a fresh arrival, the pre-run city again
        "menu" => {
            p.screen = match arg {
                "main" => Screen::Main,
                "pick" => {
                    p.shared.send(UiCmd::Nobodies);
                    Screen::Pick
                }
                "load" => Screen::Load,
                "esc" => {
                    open_esc(&mut p);
                    Screen::Esc
                }
                _ => {
                    if p.screen == Screen::Esc {
                        close_esc(&mut p);
                    }
                    Screen::Game
                }
            };
        }
        "nobody" | "look" => {
            let i: usize = arg.parse().expect("nobody I");
            let id = p.shared.slice.lock().unwrap().nobodies.get(i).map(|n| n.0);
            match id {
                Some(id) => {
                    p.wait = 0;
                    if verb == "nobody" {
                        info!("play script nobody {i}: {}", p.shared.slice.lock().unwrap().nobodies[i].2);
                        p.shared.send(UiCmd::Start(id));
                        p.screen = Screen::Game;
                    } else {
                        p.shared.send(UiCmd::Look(id));
                    }
                }
                None if p.wait < PICK_WAIT_FRAMES => {
                    p.wait += 1;
                    p.next -= 1;
                }
                None => {
                    warn!("play script {verb} {i}: no such nobody");
                    p.wait = 0;
                }
            }
        }
        "fresh" => {
            p.shared.send(UiCmd::Fresh);
            p.screen = Screen::Game;
        }
        "newcity" => p.shared.send_busy(UiCmd::NewCity),
        "save" => p.shared.send_busy(UiCmd::Save(arg.to_string())),
        "load" => {
            let path = p.saves_dir.join(format!("{arg}.ron"));
            p.shared.send_busy(UiCmd::Load(path));
            p.pending_load = true;
            p.screen = Screen::Game;
        }
        "provoke" => p.shared.send(UiCmd::Provoke(arg == "lethal")),
        "unpause" => p.shared.paused.store(false, Ordering::Relaxed),
        "autopause" => p.shared.autopause.store(arg == "on", Ordering::Relaxed),
        // T2: auto-speed's toggle; paced play for SECS real seconds
        "autospeed" => p.shared.auto_on.store(arg == "on", Ordering::Relaxed),
        "film" => match film.as_mut() {
            Some(f) => f.start(crate::film::FilmReq::parse(arg), &sim),
            None => warn!("play script film {arg}: no live sim to film"),
        },
        "live" => {
            let secs: f32 = arg.parse().expect("live SECS");
            p.live_until = Some(Instant::now() + Duration::from_secs_f32(secs.max(0.0)));
            p.shared.hold.store(u64::MAX, Ordering::Relaxed);
        }
        o => panic!("--play: unknown action '{o}'"),
    }
}

/// T2 (docs/TIME_MODEL.md § 2): a play speed as a multiple of the walk-matched rate (1x: a body walks at a walk).
fn speed_label(s: f64) -> String {
    format!("{s:.0}x")
}

fn bar(ui: &mut bevy_egui::egui::Ui, name: &str, v: f32) {
    ui.horizontal(|ui| {
        ui.label(format!("{name:<9}"));
        ui.add(bevy_egui::egui::ProgressBar::new(v.clamp(0.0, 1.0)).desired_width(110.0).text(format!("{v:.2}")));
    });
}

/// A menu row's button, its hover the will terms the verb waives; M18 P3: a row that cannot be done now is greyed,
/// its reason beside it and its hover.
fn row_button(ui: &mut bevy_egui::egui::Ui, row: &crate::simthread::MenuRow) -> bool {
    let (label, _, waived, why_not) = row;
    if let Some(why) = why_not {
        let text = format!("{label}  ({why})");
        ui.add_enabled(false, bevy_egui::egui::Button::new(text)).on_disabled_hover_text(why.as_str());
        return false;
    }
    let b = ui.button(label);
    let b = if waived.is_empty() { b } else { b.on_hover_text(format!("waived (the human's will):\n{}", waived.join("\n"))) };
    b.clicked()
}

fn hud(mut contexts: EguiContexts, mut p: ResMut<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    if !in_game(&p) {
        return Ok(());
    }
    let mut h = p.shared.player.lock().unwrap().clone();
    // P5b: a dead body has no wheel: no rows, no offers, no fight
    let dead = p.shared.news.lock().unwrap().death.is_some();
    if dead && let Some(h) = h.as_mut() {
        h.rows.clear();
        h.offers.clear();
        h.fight = None;
        h.wounds = None;
    }
    let paused = p.shared.paused.load(Ordering::Relaxed);
    let alert = p.shared.slice.lock().unwrap().alert.clone();
    let mut autopause = p.shared.autopause.load(Ordering::Relaxed);
    let mut autospeed = p.shared.auto_on.load(Ordering::Relaxed);
    let auto_line = p.shared.auto_line.lock().unwrap().clone();
    let speeds = p.speeds.clone();
    let speed_i = p.speed_i;
    let mut speed_to = None;
    let w = p.hud_width_px;
    egui::Window::new("player").anchor(egui::Align2::RIGHT_BOTTOM, [-8.0, -8.0]).resizable(false).default_width(w).max_width(w).show(
        ctx,
        |ui| {
            let Some(h) = h else {
                ui.label("waiting for the city to reach --from");
                return;
            };
            ui.strong(format!("{} #{}", h.name, h.index));
            // P5b: a dead body shows no wheel: the death screen has the way on
            if dead {
                ui.label(format!("day {} {}   dead", h.day, h.clock));
                return;
            }
            ui.label(format!(
                "day {} {}   {}{}",
                h.day,
                h.clock,
                if h.autopilot { "AUTOPILOT" } else { "you drive" },
                if paused { "   PAUSED" } else { "" }
            ));
            // P5b: the clock's speed, every step a button; T2: multiples on one line, and auto-speed's why
            ui.horizontal(|ui| {
                ui.label("speed:");
                for (i, s) in speeds.iter().enumerate() {
                    if ui.selectable_label(i == speed_i && auto_line.is_empty(), speed_label(*s)).clicked() {
                        speed_to = Some(i);
                    }
                }
            });
            if !auto_line.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(120, 200, 255), format!("auto: {auto_line}"));
            }
            // M18 P5: the auto-pause's reason while the clock it stopped is stopped
            if let Some(a) = &alert {
                ui.colored_label(egui::Color32::from_rgb(255, 120, 90), format!("PAUSED: {a} (Space goes on)"));
            }
            ui.label(format!("{}c   {} food   {}", h.coins, h.food, h.job));
            ui.label(&h.place);
            ui.label(&h.tenure);
            // M18 P4: held (cuffs, a sentence's days left)
            if let Some(held) = &h.held {
                ui.colored_label(egui::Color32::from_rgb(255, 120, 90), format!("held: {held}"));
            }
            for (n, v) in ["hunger", "energy", "safety", "wealth", "belonging"].iter().zip(h.needs) {
                bar(ui, n, v);
            }
            ui.separator();
            if let Some(d) = &h.doing {
                ui.label(format!("doing: {d}"));
            } else {
                ui.label(format!("goal: {}", h.goal));
            }
            ui.label(format!("plan: {}", h.plan));
            ui.label(format!(
                "last failure: {}",
                h.failure.as_deref().map_or("-".to_string(), |f| crate::simthread::plain_failure(f.split_once(": ").map_or(f, |x| x.1)))
            ));
            ui.separator();
            for row in h.rows.iter() {
                if row_button(ui, row) {
                    p.shared.send(UiCmd::Act(row.1));
                }
            }
            if !h.offers.is_empty() {
                ui.colored_label(egui::Color32::YELLOW, "an offer waits (the board)");
            }
            // C4: the stance and the weapon for the next fight (in one: the fight HUD)
            if h.fight.is_none() && !dead {
                use citysim::systems::combat::{Order, Stance};
                ui.horizontal_wrapped(|ui| {
                    ui.small("in a fight:");
                    for s in Stance::ALL {
                        if ui.selectable_label(h.stance == Some(s), s.order_label()).clicked() {
                            p.shared.send(UiCmd::Fight(Order::Stance(s)));
                        }
                    }
                    // C5: Holster only with a weapon to hand
                    let (l, o) = if h.holstered { ("draw", Order::Draw) } else { ("holster", Order::Holster) };
                    if (h.armed || h.holstered) && ui.small_button(l).clicked() {
                        p.shared.send(UiCmd::Fight(o));
                    }
                });
                if !h.chrome.is_empty() {
                    ui.small(h.chrome.join(", "));
                }
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Stop").clicked() {
                    p.shared.send(UiCmd::Stop);
                }
                if ui.button(if h.autopilot { "Take the wheel (T)" } else { "Autopilot (T)" }).clicked() {
                    p.shared.send(UiCmd::Autopilot(!h.autopilot));
                }
                if ui.button(if paused { "Resume" } else { "Pause" }).clicked() {
                    p.shared.paused.store(!paused, Ordering::Relaxed);
                }
            });
            // M18 P5: the auto-pause's toggle
            if ui
                .checkbox(&mut autopause, "stop the clock when someone goes for me")
                .on_hover_text("on autopilot, or when you have not touched a key for a while")
                .changed()
            {
                p.shared.autopause.store(autopause, Ordering::Relaxed);
            }
            // T2: auto-speed's toggle
            if ui
                .checkbox(&mut autospeed, "run the clock fast while nothing happens")
                .on_hover_text("asleep, at work, riding, queued, serving time: back to your speed when it ends or someone comes")
                .changed()
            {
                p.shared.auto_on.store(autospeed, Ordering::Relaxed);
            }
            ui.small("click: a person or a building for its menu, the street to walk");
        },
    );
    if let Some(i) = speed_to {
        set_speed(&mut p, i);
    }
    Ok(())
}

/// M18 P2: the click's menu, drawn at the click (only the rows the sim found legal; none: "nothing to do here").
fn menu(mut contexts: EguiContexts, mut p: ResMut<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    let m = p.shared.menu.lock().unwrap().clone();
    // a menu newer than the click opens at it
    if let Some(m) = m.as_ref().filter(|m| m.seq > p.menu_asked && p.menu_open.is_none_or(|(s, _)| s < m.seq)) {
        p.menu_open = Some((m.seq, p.click_px));
        p.menu_asked = m.seq;
    }
    let (Some((seq, at)), Some(m)) = (p.menu_open, m) else { return Ok(()) };
    if m.seq != seq {
        return Ok(());
    }
    let mut picked = None;
    let mut close = false;
    egui::Area::new(egui::Id::new("verb_menu")).fixed_pos(egui::pos2(at.x + 6.0, at.y + 6.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.strong(&m.title);
            ui.separator();
            if m.rows.is_empty() {
                ui.label("nothing to do here");
            }
            for row in &m.rows {
                if row_button(ui, row) {
                    picked = Some(row.1);
                }
            }
            ui.separator();
            close = ui.small_button("close (Esc)").clicked();
        });
    });
    if let Some(v) = picked {
        p.shared.send(UiCmd::Act(v));
        close = true;
    }
    if close {
        p.menu_open = None;
    }
    Ok(())
}

/// M18 P3: the board, bottom left: the city's offers (take or turn down), the job held, the open records the body sees
/// (a Fixer's regulars, the buyer's people, the public bounties; what it cannot take greyed with why) and the Fixers
/// to get known at. Hidden with nobody possessed.
fn board(mut contexts: EguiContexts, p: Res<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    let Some(h) = hud_of(&p) else { return Ok(()) };
    let w = p.hud_width_px * 1.6;
    egui::Window::new("the board").anchor(egui::Align2::LEFT_BOTTOM, [8.0, -8.0]).resizable(false).default_width(w).max_width(w).show(
        ctx,
        |ui| {
            if !h.offers.is_empty() {
                ui.strong("offers");
                for row in &h.offers {
                    if row_button(ui, row) {
                        p.shared.send(UiCmd::Act(row.1));
                    }
                }
                ui.separator();
            }
            ui.strong("jobs you see");
            if h.board.is_empty() {
                ui.label("nothing on any board you know");
            }
            for row in &h.board {
                if row_button(ui, row) {
                    p.shared.send(UiCmd::Act(row.1));
                }
            }
        },
    );
    Ok(())
}

/// M18 P4: the player's own log (top right, newest at the bottom; the city's doings to you in yellow) and, under it,
/// who knows about you (memories, talk, grudges naming you, with how they heard) and who is after you.
fn news(mut contexts: EguiContexts, p: Res<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    if hud_of(&p).is_none() {
        return Ok(());
    }
    let n = p.shared.news.lock().unwrap().clone();
    let w = p.hud_width_px * 1.55;
    egui::Window::new("your story").anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0]).resizable(false).default_width(w).max_width(w).show(
        ctx,
        |ui| {
            egui::ScrollArea::vertical().max_height(230.0).stick_to_bottom(true).show(ui, |ui| {
                if n.log.is_empty() {
                    ui.label("nothing yet");
                }
                for (important, line) in &n.log {
                    if *important {
                        ui.colored_label(egui::Color32::from_rgb(250, 220, 120), line);
                    } else {
                        ui.label(line);
                    }
                }
            });
        },
    );
    egui::Window::new("who knows about you")
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 290.0])
        .resizable(false)
        .default_width(w)
        .max_width(w)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                ui.strong("after you");
                if n.after.is_empty() {
                    ui.label("nobody");
                }
                for a in &n.after {
                    ui.colored_label(egui::Color32::from_rgb(255, 120, 90), a);
                }
                for t in &n.talk {
                    ui.label(format!("the talk: {t}"));
                }
                ui.small(&n.rep);
                ui.separator();
                ui.strong(format!("{} know of you ({} of a deed of yours)", n.knowing.1, n.knowing.0));
                for (name, what, how) in &n.knowers {
                    ui.label(format!("{name} {what}")).on_hover_text(how.as_str());
                    ui.small(format!("    {how}"));
                }
            });
        });
    Ok(())
}

/// M18 P4: the day-end summary (top centre) until it is closed (or a script's `dismiss`).
fn day_end(mut contexts: EguiContexts, mut p: ResMut<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    // M18 P5: a new possession's panels start again
    let (days, epoch) = {
        let n = p.shared.news.lock().unwrap();
        (n.days.clone(), n.epoch)
    };
    if epoch != p.epoch_seen {
        p.epoch_seen = epoch;
        p.days_seen = days.len();
        // P5b: a new game starts at a speed where life visibly moves
        if p.slice {
            let i = p.start_speed_i;
            set_speed(&mut p, i);
        }
    }
    if days.len() <= p.days_seen || !in_game(&p) {
        return Ok(());
    }
    let Some((day, lines)) = days.last() else { return Ok(()) };
    let mut close = false;
    egui::Window::new(format!("the end of day {day}"))
        .anchor(egui::Align2::CENTER_TOP, [0.0, 8.0])
        .resizable(false)
        .collapsible(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            for l in lines {
                if l.starts_with("after you") {
                    ui.colored_label(egui::Color32::from_rgb(255, 120, 90), l);
                } else {
                    ui.label(l);
                }
            }
            close = ui.button("go on").clicked();
        });
    if close {
        p.days_seen = days.len();
    }
    Ok(())
}

/// M18 P4: the death screen, from the player's `Life` (the sim thread pauses the clock at the death).
fn death(mut contexts: EguiContexts, mut p: ResMut<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    if !in_game(&p) {
        return Ok(());
    }
    let Some((line, life)) = p.shared.news.lock().unwrap().death.clone() else { return Ok(()) };
    let mut go = None;
    egui::Window::new("dead")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .collapsible(false)
        .default_width(600.0)
        .show(ctx, |ui| {
            ui.heading(&line);
            ui.label("the city goes on without you");
            ui.separator();
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for l in &life {
                    ui.label(l);
                }
            });
            // M18 P5: permadeath: the same city goes on with another nobody
            if p.slice {
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Pick another nobody (the same city)").clicked() {
                        go = Some(0);
                    }
                    if ui.button("Arrive fresh").clicked() {
                        go = Some(1);
                    }
                    if ui.button("Quit to the menu").clicked() {
                        go = Some(2);
                    }
                });
            }
        });
    match go {
        Some(0) => {
            p.shared.send(UiCmd::Nobodies);
            p.screen = Screen::Pick;
        }
        Some(1) => p.shared.send(UiCmd::Fresh),
        Some(2) => p.screen = Screen::Main,
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// C4 (docs/COMBAT.md § 5): the player's fight: the replay's cues, the fight HUD, the wounds panel
// ---------------------------------------------------------------------------

/// C4: one cue of the beat replay: a word over a body (and a flash on a telling blow), from `at` for `[view] cue_s`.
pub struct Cue {
    at: Instant,
    key: u64,
    text: String,
    color: [u8; 3],
    big: bool,
    flash: bool,
}

/// The sim's body key (sim.rs `ekey`) of an entity.
fn key_of(e: citysim::EntityId) -> u64 {
    u64::from(e.index) | (u64::from(e.generation) << 32)
}

/// The cues of one blow: the swing over the striker, the dodge or the hit over the struck, a break, a joiner.
fn cues_of(b: &citysim::systems::combat::BeatLog, at: Instant, out: &mut Vec<Cue>) {
    use citysim::components::Severity;
    use citysim::systems::combat::{Blow, End};
    let mut cue = |e: citysim::EntityId, text: String, color: [u8; 3], big: bool, flash: bool| {
        out.push(Cue { at, key: key_of(e), text, color, big, flash });
    };
    match b.blow {
        Blow::Miss => {
            cue(b.by, "swing".into(), [200, 200, 200], false, false);
            cue(b.on, "dodge".into(), [140, 200, 255], false, false);
        }
        Blow::Hit(r, v) => {
            cue(b.by, "swing".into(), [200, 200, 200], false, false);
            match v {
                Severity::Graze => cue(b.on, format!("graze: {}", r.label()), [250, 220, 90], false, false),
                Severity::Wound => cue(b.on, format!("HIT {} (staggers)", r.label()), [255, 150, 60], true, true),
                Severity::Grave => cue(b.on, format!("GRAVE: {}", r.label()), [255, 60, 50], true, true),
            }
        }
        Blow::Broke(e) => {
            let (t, c) = match e {
                End::Fled => ("RUNS", [255, 255, 255]),
                End::Yielded => ("YIELDS", [255, 255, 255]),
                End::KnockedOut => ("KNOCKED OUT", [255, 90, 60]),
                End::Downed => ("DOWN", [255, 60, 50]),
                End::Dead => ("DEAD", [255, 30, 30]),
                End::Wrecked => ("WRECKED", [255, 90, 60]),
                End::Won => ("WINS", [120, 255, 120]),
            };
            cue(b.by, t.into(), c, true, true);
        }
        Blow::Joined => cue(b.by, "JOINS IN".into(), [210, 130, 255], true, true),
        Blow::Cover(c) => cue(b.by, format!("cover {c}%"), [120, 230, 230], false, false),
        Blow::Stuck => cue(b.by, "can't get away".into(), [255, 200, 120], false, false),
        Blow::Refused => cue(b.on, "no quarter".into(), [255, 120, 90], true, false),
        Blow::Drew(d) => cue(b.by, if d { "draws" } else { "holsters" }.into(), [200, 200, 200], false, false),
        Blow::Stance(s) => cue(b.by, s.order_label().into(), [200, 200, 200], false, false),
    }
}

/// C5: the procedural poses of one blow: the striker lunges at the struck (a swing, hit or miss), the struck staggers
/// away on a hit of a Wound or worse.
fn poses_of(b: &citysim::systems::combat::BeatLog, at: Instant, out: &mut Vec<Pose>) {
    use citysim::components::Severity;
    use citysim::systems::combat::Blow;
    let (by, on) = (key_of(b.by), key_of(b.on));
    match b.blow {
        Blow::Miss => out.push(Pose { key: by, toward: on, lunge: true, at }),
        Blow::Hit(_, v) => {
            out.push(Pose { key: by, toward: on, lunge: true, at });
            if v >= Severity::Wound {
                out.push(Pose { key: on, toward: by, lunge: false, at });
            }
        }
        _ => {}
    }
}

/// C4, the beat replay: each new tick of beats (`Shared::beats`) is laid out a beat every `beat_s`, and each cue is
/// drawn over its body (rising, fading) with a flash on a telling blow.
pub fn replay(
    mut contexts: EguiContexts,
    mut p: ResMut<Play>,
    sim: Res<crate::sim::Sim>,
    cams: Query<(&Camera, &GlobalTransform), With<crate::cam::CityCam>>,
) -> Result {
    use bevy_egui::egui;
    let bp = p.shared.beats.lock().unwrap().clone();
    if bp.seq != p.beats_seq {
        p.beats_seq = bp.seq;
        let now = Instant::now();
        let mut fresh = Vec::new();
        let mut poses = Vec::new();
        for (g, b) in &bp.beats {
            let at = now + Duration::from_secs_f32(*g as f32 * bp.beat_s);
            cues_of(b, at, &mut fresh);
            poses_of(b, at, &mut poses);
        }
        p.cues.extend(fresh);
        p.poses.extend(poses);
    }
    let lunge = Duration::from_secs_f32(p.lunge_s.max(0.05));
    let now = Instant::now();
    p.poses.retain(|x| now < x.at + lunge);
    let poses = p.poses.clone();
    if let Ok(mut l) = p.shared.poses.lock() {
        *l = poses;
    }
    let now = Instant::now();
    let life = Duration::from_secs_f32(p.cue_s.max(0.1));
    p.cues.retain(|c| now < c.at + life);
    if p.cues.is_empty() {
        return Ok(());
    }
    let Ok(cam) = cams.single() else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("beat_cues")));
    // the cues stacked per body (a swing and a hit on one body in one beat sit one above the other)
    let mut stack: std::collections::HashMap<u64, f32> = std::collections::HashMap::new();
    for c in p.cues.iter().filter(|c| c.at <= now) {
        let at = if sim.subject == Some(c.key) {
            sim.subject_at.map(|(f, _)| f)
        } else {
            sim.people.iter().find(|b| b.id == c.key).map(|b| b.at)
        };
        let Some(at) = at else { continue };
        let Ok(px) = cam.0.world_to_viewport(cam.1, crate::mesh::bv(at.x, at.y, at.z + 2.1)) else { continue };
        let age = (now - c.at).as_secs_f32() / life.as_secs_f32();
        let a = (1.0 - age).clamp(0.0, 1.0);
        let up = stack.entry(c.key).or_insert(0.0);
        let pos = egui::pos2(px.x, px.y - 26.0 * age - *up);
        *up += if c.big { 20.0 } else { 15.0 };
        let col = egui::Color32::from_rgba_unmultiplied(c.color[0], c.color[1], c.color[2], (255.0 * a) as u8);
        if c.flash {
            let fl = egui::Color32::from_rgba_unmultiplied(c.color[0], c.color[1], c.color[2], (110.0 * a) as u8);
            painter.circle_filled(egui::pos2(px.x, px.y + 14.0), 10.0 + 18.0 * age, fl);
        }
        let font = egui::FontId::proportional(if c.big { 18.0 } else { 13.0 });
        painter.text(
            pos + egui::vec2(1.0, 1.0),
            egui::Align2::CENTER_BOTTOM,
            &c.text,
            font.clone(),
            egui::Color32::from_black_alpha((200.0 * a) as u8),
        );
        painter.text(pos, egui::Align2::CENTER_BOTTOM, &c.text, font, col);
    }
    Ok(())
}

fn order_button(ui: &mut bevy_egui::egui::Ui, p: &Play, label: &str, o: citysim::systems::combat::Order) {
    if ui.button(label).clicked() {
        p.shared.send(UiCmd::Fight(o));
    }
}

/// C4: the fight HUD (top left) while the body is in a live fight: the sides with their stances and what their
/// wounds show, your stance and orders, your wounds exactly, the blood and fatigue, the morale hints, the chrome to
/// hand, an Attack row a foe with its odds, a Help row a downed body.
pub fn fight(mut contexts: EguiContexts, p: Res<Play>) -> Result {
    use bevy_egui::egui;
    use citysim::systems::combat::{Order, Stance};
    let ctx = contexts.ctx_mut()?;
    let Some(h) = hud_of(&p) else { return Ok(()) };
    if dead(&p) {
        return Ok(());
    }
    let Some(f) = h.fight else { return Ok(()) };
    let w = p.hud_width_px * 1.4;
    egui::Window::new("the fight")
        .anchor(egui::Align2::LEFT_TOP, [8.0, 200.0])
        .fade_in(false)
        .resizable(false)
        .default_width(w)
        .max_width(w)
        .show(ctx, |ui| {
            ui.label(format!(
                "beat {}   {}",
                f.beats,
                if p.shared.paused.load(Ordering::Relaxed) { "PAUSED (Space)" } else { "Space pauses" }
            ));
            let orange = egui::Color32::from_rgb(255, 170, 80);
            for hint in &f.hints {
                ui.colored_label(orange, hint);
            }
            ui.separator();
            for (k, title) in [(f.mine, "your side"), (1 - f.mine, "their side")] {
                ui.strong(title);
                for m in &f.sides[k] {
                    let front = if m.front { " [in the duel]" } else { "" };
                    let shows = if m.shows.is_empty() { String::new() } else { format!(", {}", m.shows) };
                    let wav = if m.wavering && k != f.mine { ", wavering" } else { "" };
                    ui.label(format!("  {} ({}){shows}{wav}{front}", m.name, m.stance));
                }
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label("stance:");
                for s in Stance::ALL {
                    if ui.selectable_label(f.stance == Some(s), s.order_label()).clicked() {
                        p.shared.send(UiCmd::Fight(Order::Stance(s)));
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{}{}", f.weapon, if f.holstered { " (holstered)" } else { "" }));
                // C5: Holster only with something in the hand, Take cover only with a gun in the fight (melee
                // ignores cover)
                if f.holstered {
                    order_button(ui, &p, "Draw", Order::Draw);
                } else if !f.weapon.starts_with("bare hands") {
                    order_button(ui, &p, "Holster", Order::Holster);
                }
                if f.guns {
                    order_button(ui, &p, &format!("Take cover ({:.0}% here)", f.cover * 100.0), Order::Cover);
                }
            });
            ui.horizontal_wrapped(|ui| {
                order_button(ui, &p, "Flee", Order::Flee);
                order_button(ui, &p, "Yield", Order::Yield);
            });
            // C5: the duel's odds (you and that one) and the whole fight's (their side as it stands)
            let more = f.sides[1 - f.mine].len() > 1 || f.sides[f.mine].len() > 1;
            for (id, name, duel, whole) in &f.odds {
                let label = if more {
                    format!("Attack {name} (the duel {:.0}%; the fight {:.0}%)", duel * 100.0, whole * 100.0)
                } else {
                    format!("Attack {name} ({:.0}%)", duel * 100.0)
                };
                order_button(ui, &p, &label, Order::Attack(*id));
            }
            for (id, name) in &f.downed {
                order_button(ui, &p, &format!("Help {name} (drag clear)"), Order::Help(*id));
            }
            if !f.pending.is_empty() {
                ui.small(format!("at the next beat: {}", f.pending.join(", ")));
            }
            ui.separator();
            bar(ui, "blood", f.blood);
            bar(ui, "fatigue", f.fatigue);
            for wl in &f.wounds {
                ui.small(wl);
            }
            for c in &f.chrome {
                ui.colored_label(egui::Color32::from_rgb(120, 230, 230), c);
            }
        });
    Ok(())
}

/// C4: the wounds panel (top left) outside a fight: each wound, its days to heal, seen to or not, the blood, what a
/// patch-up costs and where (the Treat goal's pick, or why nobody will see to it), and the button that goes.
pub fn wounds(mut contexts: EguiContexts, p: Res<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    let Some(h) = hud_of(&p) else { return Ok(()) };
    if h.fight.is_some() || dead(&p) {
        return Ok(());
    }
    let Some(wv) = h.wounds else { return Ok(()) };
    let w = p.hud_width_px * 1.4;
    egui::Window::new("your wounds")
        .anchor(egui::Align2::LEFT_TOP, [8.0, 200.0])
        .fade_in(false)
        .resizable(false)
        .default_width(w)
        .max_width(w)
        .show(ctx, |ui| {
            bar(ui, "blood", wv.blood);
            for l in &wv.wounds {
                ui.label(l);
            }
            ui.separator();
            for l in &wv.care {
                ui.small(l);
            }
            if wv.untreated && ui.button("Get patched up (the Treat goal)").clicked() {
                p.shared.send(UiCmd::Goal(GoalKind::Treat));
            }
        });
    Ok(())
}

// ---------------------------------------------------------------------------
// M18 P5 (docs/M18_PLAYER.md, start and loop): the start menu, the Esc menu, the death screen's way on, the help
// line
// ---------------------------------------------------------------------------

/// M18 P5: which screen is up (`--slice`; `--possess` plays with `Game` only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The city, the HUD and its panels.
    Game,
    /// New (pick a nobody, arrive fresh), Load, Quit.
    Main,
    /// "Pick a nobody": a handful of broke adults, each with its one-line read.
    Pick,
    /// The player saves (and the pre-run city).
    Load,
    /// Esc in play: Resume, Save, Load, Quit to menu, Quit.
    Esc,
}

/// The game's panels show (the HUD, the board, the log, the fight): in play, or under the Esc menu.
fn in_game(p: &Play) -> bool {
    matches!(p.screen, Screen::Game | Screen::Esc)
}

/// The possessed body's HUD while the game's panels show.
fn hud_of(p: &Play) -> Option<crate::simthread::PlayerHud> {
    if !in_game(p) {
        return None;
    }
    p.shared.player.lock().unwrap().clone()
}

/// P5b: the possessed body is dead (the death screen is up): its fight, wounds and action panels are gone.
fn dead(p: &Play) -> bool {
    p.shared.news.lock().unwrap().death.is_some()
}

/// A save's name from the body and the clock: `Name_Surname_d23_1840`.
fn save_name(h: Option<&crate::simthread::PlayerHud>) -> String {
    match h {
        Some(h) => {
            let n: String = h.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            format!("{n}_d{}_{}", h.day, h.clock.replace(':', ""))
        }
        None => "city".into(),
    }
}

/// M18 P5: the Esc menu opens over a stopped clock; closing it lets the clock go if it was going.
fn open_esc(p: &mut Play) {
    p.esc_was_paused = p.shared.paused.swap(true, Ordering::Relaxed);
    let h = p.shared.player.lock().unwrap().clone();
    p.save_name = save_name(h.as_ref());
    p.screen = Screen::Esc;
}

fn close_esc(p: &mut Play) {
    p.screen = Screen::Game;
    if !p.esc_was_paused {
        p.shared.paused.store(false, Ordering::Relaxed);
    }
}

/// M18 P5: New from the main menu: a game already played in this city starts from the pre-run city again.
fn new_city_if_played(p: &Play) {
    if p.shared.slice.lock().unwrap().started {
        p.shared.send_busy(UiCmd::NewCity);
    }
}

/// M18 P5: the start menu (`--slice`): New (pick a nobody, arrive fresh), Load, Quit; "pick a nobody"; the saves; the
/// Esc menu in play (Resume, Save, Load, Quit to menu, Quit).
fn start_menu(mut contexts: EguiContexts, mut p: ResMut<Play>, mut exit: MessageWriter<AppExit>) -> Result {
    use bevy_egui::egui;
    if !p.slice {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let sl = p.shared.slice.lock().unwrap().clone();
    // a load ends in play when the save holds a body, else at the pick
    if p.pending_load && !sl.busy {
        p.pending_load = false;
        if !sl.started && p.screen == Screen::Game {
            p.screen = Screen::Pick;
        }
    }
    // the clock the auto-pause stopped goes again: its banner goes
    if sl.alert.is_some() && !p.shared.paused.load(Ordering::Relaxed) {
        p.shared.slice.lock().unwrap().alert = None;
    }
    let busy = |ui: &mut egui::Ui| {
        if !sl.status.is_empty() {
            if sl.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&sl.status);
                });
            } else {
                ui.small(&sl.status);
            }
        }
    };
    let center = egui::Align2::CENTER_CENTER;
    match p.screen {
        Screen::Game => {}
        Screen::Main => {
            let mut go = None;
            egui::Window::new("TheCity").anchor(center, [0.0, 0.0]).resizable(false).collapsible(false).default_width(520.0).show(
                ctx,
                |ui| {
                    ui.heading("A nobody in the Sump");
                    ui.label("A city of 20,000 that lives without you. You take one broke adult and live their days.");
                    if !sl.backdrop.is_empty() {
                        ui.small(&sl.backdrop);
                    }
                    ui.separator();
                    if let Some(m) = &sl.missing {
                        ui.colored_label(egui::Color32::from_rgb(255, 140, 100), m);
                        ui.separator();
                    }
                    let ready = sl.missing.is_none() && !sl.busy;
                    let dead = p.shared.news.lock().unwrap().death.is_some();
                    if sl.started && !dead && ui.add_enabled(ready, egui::Button::new("Resume")).clicked() {
                        go = Some(0);
                    }
                    if ui
                        .add_enabled(ready, egui::Button::new("New: pick a nobody"))
                        .on_hover_text("a broke adult of the city, with a past")
                        .clicked()
                    {
                        go = Some(1);
                    }
                    if ui
                        .add_enabled(ready, egui::Button::new("New: arrive fresh"))
                        .on_hover_text("an immigrant onto a Sump street: a few coins, nobody knows you")
                        .clicked()
                    {
                        go = Some(2);
                    }
                    if ui.add_enabled(!sl.busy, egui::Button::new("Load")).clicked() {
                        go = Some(3);
                    }
                    if ui.button("Quit").clicked() {
                        go = Some(4);
                    }
                    ui.separator();
                    busy(ui);
                },
            );
            match go {
                Some(0) => p.screen = Screen::Game,
                Some(1) => {
                    new_city_if_played(&p);
                    p.shared.send(UiCmd::Nobodies);
                    p.screen = Screen::Pick;
                }
                Some(2) => {
                    new_city_if_played(&p);
                    p.shared.send(UiCmd::Fresh);
                    p.screen = Screen::Game;
                }
                Some(3) => p.screen = Screen::Load,
                Some(4) => {
                    exit.write(AppExit::Success);
                }
                _ => {}
            }
        }
        Screen::Pick => {
            let mut back = false;
            let mut fresh = false;
            egui::Window::new("pick a nobody").anchor(center, [0.0, 0.0]).resizable(false).collapsible(false).default_width(760.0).show(
                ctx,
                |ui| {
                    ui.label("Each is a real body of the running city. Look: the camera goes to them. Play: you are them.");
                    ui.separator();
                    if sl.nobodies.is_empty() {
                        ui.label(if sl.busy { "the city is loading" } else { "nobody fits (all doing well, or all held)" });
                    }
                    egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                        for (id, kind, line) in &sl.nobodies {
                            ui.horizontal(|ui| {
                                if ui.add_enabled(!sl.busy, egui::Button::new("Look")).clicked() {
                                    p.shared.send(UiCmd::Look(*id));
                                }
                                if ui.add_enabled(!sl.busy, egui::Button::new("Play")).clicked() {
                                    p.shared.send(UiCmd::Start(*id));
                                    p.screen = Screen::Game;
                                }
                                ui.strong(kind);
                            });
                            ui.label(line);
                            ui.add_space(4.0);
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        fresh = ui.add_enabled(!sl.busy, egui::Button::new("Arrive fresh instead")).clicked();
                        back = ui.button("Back").clicked();
                    });
                    busy(ui);
                },
            );
            if fresh {
                p.shared.send(UiCmd::Fresh);
                p.screen = Screen::Game;
            }
            if back {
                p.screen = Screen::Main;
            }
        }
        Screen::Load => {
            let mut back = false;
            egui::Window::new("load").anchor(center, [0.0, 0.0]).resizable(false).collapsible(false).default_width(560.0).show(ctx, |ui| {
                if ui.add_enabled(!sl.busy && sl.missing.is_none(), egui::Button::new("the pre-run city (day 21, nobody yet)")).clicked() {
                    p.shared.send_busy(UiCmd::NewCity);
                    p.screen = Screen::Pick;
                }
                ui.separator();
                if sl.saves.is_empty() {
                    ui.label("no saves yet (Esc in play: Save)");
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for (path, line) in &sl.saves {
                        if ui.add_enabled(!sl.busy, egui::Button::new(line)).clicked() {
                            p.shared.send_busy(UiCmd::Load(path.clone()));
                            p.pending_load = true;
                            p.screen = Screen::Game;
                        }
                    }
                });
                ui.separator();
                back = ui.button("Back").clicked();
                busy(ui);
            });
            if back {
                p.screen = if sl.started { Screen::Esc } else { Screen::Main };
            }
        }
        Screen::Esc => {
            let mut go = None;
            let mut name = p.save_name.clone();
            egui::Window::new("menu").anchor(center, [0.0, 0.0]).resizable(false).collapsible(false).default_width(420.0).show(ctx, |ui| {
                if ui.button("Resume (Esc)").clicked() {
                    go = Some(0);
                }
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(240.0));
                    if ui.add_enabled(!sl.busy && !name.trim().is_empty(), egui::Button::new("Save")).clicked() {
                        go = Some(1);
                    }
                });
                if ui.add_enabled(!sl.busy, egui::Button::new("Load")).clicked() {
                    go = Some(2);
                }
                if ui.button("Quit to the menu").clicked() {
                    go = Some(3);
                }
                if ui.button("Quit the game").clicked() {
                    go = Some(4);
                }
                // T2: the clock's two tick boxes (the HUD has them too)
                ui.separator();
                let mut ap = p.shared.autopause.load(Ordering::Relaxed);
                if ui.checkbox(&mut ap, "stop the clock when someone goes for me").changed() {
                    p.shared.autopause.store(ap, Ordering::Relaxed);
                }
                let mut au = p.shared.auto_on.load(Ordering::Relaxed);
                if ui.checkbox(&mut au, "run the clock fast while nothing happens").changed() {
                    p.shared.auto_on.store(au, Ordering::Relaxed);
                }
                ui.separator();
                ui.small("Death is for good: the city goes on, and you pick another nobody. Save to come back.");
                busy(ui);
            });
            p.save_name = name;
            match go {
                Some(0) => close_esc(&mut p),
                Some(1) => {
                    let n = p.save_name.trim().to_string();
                    p.shared.send_busy(UiCmd::Save(n));
                }
                Some(2) => p.screen = Screen::Load,
                Some(3) => {
                    p.shared.paused.store(true, Ordering::Relaxed);
                    p.screen = Screen::Main;
                }
                Some(4) => {
                    exit.write(AppExit::Success);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// M18 P5: the cold-start help line (bottom centre, in play): the keys, what a click does, the clock.
fn help(mut contexts: EguiContexts, p: Res<Play>) -> Result {
    use bevy_egui::egui;
    if !in_game(&p) || p.shared.player.lock().unwrap().is_none() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    egui::Area::new(egui::Id::new("help_line")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -6.0]).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.small(
                "Click a person or a building: what you can do there.  Click the street: walk there.  \
                 Space: pause / go on.   , and . : slower / faster.   T: autopilot (keeps you alive, decides nothing).",
            );
            ui.small(
                "Backspace: stop.   Esc: menu (save, load, quit).   Right-drag or Q / E: turn the camera, wheel: zoom.   \
                 The clock stops by itself when someone goes for you, and runs fast while you sleep or work (the HUD's tick boxes).",
            );
        });
    });
    Ok(())
}

/// M18 P5: the slice starts with the debug overlay (top left) hidden: H shows it.
fn hide_debug(mut q: Query<&mut Visibility, With<crate::bench::Hud>>, mut done: Local<bool>) {
    if *done {
        return;
    }
    for mut v in q.iter_mut() {
        *v = Visibility::Hidden;
        *done = true;
    }
}
