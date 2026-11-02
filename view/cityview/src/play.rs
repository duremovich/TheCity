//! M18 P1, the seam (docs/M18_PLAYER.md): a human drives the followed body (`--save FILE --possess INDEX`).
//!
//! The HUD is egui (`bevy_egui`): the clock, the needs, the coins, the job, where the body is, its goal and current
//! plan (the step under way marked `>`) and the plan's last failure, with buttons for the goals the planner makes a
//! plan for (`PlayerCommand::Goal`, `plan::plan_for`: Eat, Sleep, Work, Shop) and Stop. A left click on the city casts
//! the camera's ray against the ground and the lots' massing boxes (lots.rs; the cutaway's box around the subject is
//! passed through, it is not drawn) and sends the tile it hits: the body walks there, into the building that holds it
//! (`citysim::systems::player::walk_to`). Keys: T autopilot (the body's own brain on / off), Space pause, `,` / `.`
//! the play speed (`[view] play_speeds`, multiples of the walk-matched rate), Backspace stop.
//!
//! `--play "T action; T action; .."` scripts a session through the same paths (a play session for a proof): at the
//! first frame whose tick is T or later (`D:HH:MM` or a tick) it runs `click TX,TY` (the tile's ground point projected
//! to the screen, then the click's own ray), `zoom M` (the frame's width, as the wheel), `goal NAME`, `stop`, `auto on|off`, `pause`, `speed I` (an index into the
//! speeds), `shot NAME` (a PNG with the HUD into `--out`) or `quit`.

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
        })
        .add_systems(Update, (keys, click, run_script))
        .add_systems(EguiPrimaryContextPass, hud);
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

fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<crate::cam::CityCam>>,
    egui: Option<Res<EguiWantsInput>>,
    p: Res<Play>,
    sim: Res<crate::sim::Sim>,
    stream: Res<crate::stream::Stream>,
    view: Res<crate::cam::View>,
) {
    if !buttons.just_pressed(MouseButton::Left) || egui.is_some_and(|e| e.wants_any_pointer_input()) {
        return;
    }
    let Some(px) = windows.single().ok().and_then(|w| w.cursor_position()) else { return };
    let Ok(cam) = cams.single() else { return };
    send_pick(px, cam, &p, &sim, &stream, view.zoom, "click");
}

fn send_pick(
    px: Vec2,
    cam: (&Camera, &GlobalTransform),
    p: &Play,
    sim: &crate::sim::Sim,
    stream: &crate::stream::Stream,
    zoom: f32,
    why: &str,
) {
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
    if p.next >= p.script.len() || p.script[p.next].tick > tick {
        return;
    }
    let step = p.script[p.next].clone();
    p.next += 1;
    let (verb, arg) = step.action.split_once(' ').map_or((step.action.as_str(), ""), |(v, a)| (v, a.trim()));
    info!("play script tick {} (frame {tick}): {}", step.tick, step.action);
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
            match cam.0.world_to_viewport(cam.1, crate::mesh::bv(m.x, m.y, z)) {
                Ok(px) => send_pick(px, cam, &p, &sim, &stream, view.zoom, "script click"),
                Err(e) => info!("play script click ({}, {}): not on the screen ({e:?})", t[0], t[1]),
            }
        }
        "zoom" => {
            let z: f32 = arg.parse().expect("zoom METRES");
            view.set_zoom(z);
        }
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
        ui.add(bevy_egui::egui::ProgressBar::new(v.clamp(0.0, 1.0)).desired_width(140.0).text(format!("{v:.2}")));
    });
}

fn hud(mut contexts: EguiContexts, mut p: ResMut<Play>) -> Result {
    use bevy_egui::egui;
    let ctx = contexts.ctx_mut()?;
    let h = p.shared.player.lock().unwrap().clone();
    let paused = p.shared.paused.load(Ordering::Relaxed);
    let speed = p.speeds[p.speed_i];
    let mut speed_to = None;
    egui::Window::new("player").anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0]).resizable(false).default_width(380.0).show(ctx, |ui| {
        let Some(h) = h else {
            ui.label("waiting for the city to reach --from");
            return;
        };
        ui.heading(format!("{} #{}", h.name, h.index));
        ui.label(format!(
            "day {} {}   {}   x{speed}{}",
            h.day,
            h.clock,
            if h.autopilot { "AUTOPILOT" } else { "you drive" },
            if paused { "  PAUSED" } else { "" }
        ));
        ui.label(format!("{}c   {}", h.coins, h.job));
        ui.label(&h.place);
        ui.separator();
        for (n, v) in ["hunger", "energy", "safety", "wealth", "belonging"].iter().zip(h.needs) {
            bar(ui, n, v);
        }
        ui.separator();
        ui.label(format!("goal: {}", h.goal));
        ui.label(format!("plan: {}", h.plan));
        ui.label(format!("last failure: {}", h.failure.as_deref().unwrap_or("-")));
        ui.separator();
        ui.horizontal(|ui| {
            for g in [GoalKind::Eat, GoalKind::Sleep, GoalKind::Work, GoalKind::Shop] {
                if ui.button(format!("{g:?}")).clicked() {
                    p.shared.send(UiCmd::Goal(g));
                }
            }
            if ui.button("Stop").clicked() {
                p.shared.send(UiCmd::Stop);
            }
        });
        ui.horizontal(|ui| {
            if ui.button(if h.autopilot { "Take the wheel (T)" } else { "Autopilot (T)" }).clicked() {
                p.shared.send(UiCmd::Autopilot(!h.autopilot));
            }
            if ui.button(if paused { "Resume (Space)" } else { "Pause (Space)" }).clicked() {
                p.shared.paused.store(!paused, Ordering::Relaxed);
            }
            if ui.button("slower (,)").clicked() {
                speed_to = Some(p.speed_i.saturating_sub(1));
            }
            if ui.button("faster (.)").clicked() {
                speed_to = Some(p.speed_i + 1);
            }
        });
        ui.small("left click: walk there (into the building)");
    });
    if let Some(i) = speed_to {
        set_speed(&mut p, i);
    }
    Ok(())
}
