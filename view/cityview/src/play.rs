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
//! `goal NAME`, `stop`, `auto on|off`, `pause`, `speed I`, `shot NAME` (a PNG with the HUD and the menu into `--out`)
//! or `quit`.

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
}

/// `"T action; T action"`: the ticks a number or `D:HH:MM`, in rising order.
pub fn parse_script(s: &str, tick: impl Fn(&str) -> u64) -> Vec<Step> {
    let steps: Vec<Step> = s
        .split(';')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (t, a) = l.split_once(' ').unwrap_or_else(|| panic!("--play '{l}': 'TICK ACTION'"));
            Step { tick: tick(t), action: a.trim().to_string() }
        })
        .collect();
    assert!(steps.windows(2).all(|w| w[0].tick <= w[1].tick), "--play: ticks in rising order");
    steps
}

pub fn add(app: &mut App, shared: Arc<Shared>, vcfg: &citysim::config::ViewCfg, script: Vec<Step>, out: PathBuf) {
    let speeds = if vcfg.play_speeds.is_empty() { vec![1.0] } else { vcfg.play_speeds.clone() };
    app.add_plugins(EguiPlugin::default())
        .insert_resource(Play {
            shared,
            speeds,
            speed_i: 0,
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
        })
        .add_systems(Update, (keys, click, run_script))
        .add_systems(EguiPrimaryContextPass, (hud, menu, board));
}

fn set_speed(p: &mut Play, i: usize) {
    p.speed_i = i.min(p.speeds.len() - 1);
    p.shared.speed_bits.store(p.speeds[p.speed_i].to_bits(), Ordering::Relaxed);
}

fn autopilot_now(p: &Play) -> bool {
    p.shared.player.lock().unwrap().as_ref().is_some_and(|h| h.autopilot)
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut p: ResMut<Play>) {
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
    if keys.just_pressed(KeyCode::Escape) {
        p.menu_open = None;
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
    if !buttons.just_pressed(MouseButton::Left) || egui.is_some_and(|e| e.wants_any_pointer_input()) {
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
            let (x, y) = arg.split_once(',').expect("click TX,TY");
            let t: [f32; 2] = [x.trim().parse().expect("click TX"), y.trim().parse().expect("click TY")];
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
        // M18 P3: a building's menu by its entity (the menu a click on it opens), at the screen's centre
        "bld" => {
            let index: u32 = arg.parse().expect("bld INDEX");
            p.menu_open = None;
            p.click_px = Vec2::new(480.0, 300.0);
            let asked = p.shared.menu.lock().unwrap().as_ref().map_or(0, |m| m.seq);
            p.menu_asked = asked;
            p.shared.send(UiCmd::Bld(citysim::EntityId { index, generation: 0 }));
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
                    "play shot {}: d{} {} {} | goal {} | plan {} | {} | coins {} | fail {:?}",
                    path.display(),
                    h.day,
                    h.clock,
                    h.name,
                    h.goal,
                    h.plan,
                    h.place,
                    h.coins,
                    h.failure
                );
            }
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        }
        "quit" => p.quit_in = Some(30),
        o => panic!("--play: unknown action '{o}'"),
    }
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
    let h = p.shared.player.lock().unwrap().clone();
    let paused = p.shared.paused.load(Ordering::Relaxed);
    let speed = p.speeds[p.speed_i];
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
            ui.label(format!(
                "day {} {}   {}   x{speed}{}",
                h.day,
                h.clock,
                if h.autopilot { "AUTOPILOT" } else { "you drive" },
                if paused { "  PAUSED" } else { "" }
            ));
            ui.label(format!("{}c   {} food   {}", h.coins, h.food, h.job));
            ui.label(&h.place);
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
            ui.label(format!("last failure: {}", h.failure.as_deref().unwrap_or("-")));
            ui.separator();
            for row in &h.rows {
                if row_button(ui, row) {
                    p.shared.send(UiCmd::Act(row.1));
                }
            }
            if !h.offers.is_empty() {
                ui.colored_label(egui::Color32::YELLOW, "an offer waits (the board)");
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
                if ui.button("<<").clicked() {
                    speed_to = Some(p.speed_i.saturating_sub(1));
                }
                if ui.button(">>").clicked() {
                    speed_to = Some(p.speed_i + 1);
                }
            });
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
    let Some(h) = p.shared.player.lock().unwrap().clone() else { return Ok(()) };
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
