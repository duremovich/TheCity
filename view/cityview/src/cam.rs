//! Cameras (the view milestone V4b, Dylan 2026-11-01: an orbit follow-cam replaces SPEC §CAM 5.1's locked
//! orthographic NW view). The game camera is a perspective orbit at a narrow FOV around its subject (the followed
//! body, the player stand-in, or the free view's ground point): free yaw, pitch clamped, zoom by distance. The
//! zoom is the frame's width at the subject in metres, so `--zoom` keeps its meaning (the distance follows from
//! it and the FOV). The aim eases toward the subject's place, z included: a body climbing to floor 30 takes the
//! camera up. Tab swaps to a free perspective overview orbit over the island. Numbers: `[view] cam_*`.
//!
//! Yaw is the camera's bearing from the subject (0 = south of it, 90 deg = east; 45 deg = the old view, looking
//! NW); pitch is the angle above the horizon.
//!
//! Keys: WASD / arrows pan (yaw-aligned; the overview's centre; with the player on, WASD walks it), Q / E rotate,
//! Z / X pitch down / up, the wheel or - / = zoom, right- or middle-drag rotate and pitch, Tab game <-> overview,
//! L night <-> day light (structure reading), M hide the massing (debug), P screenshot, H hud.

use bevy::{
    camera::Hdr,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::{AccumulatedMouseMotion, MouseWheel},
    post_process::bloom::Bloom,
    prelude::*,
};
use glclient::frame::Focus;

use crate::mesh::bv;

/// `[view] cam_*` (assets/config.toml documents each).
#[derive(Clone, Debug)]
pub struct CamCfg {
    /// Vertical FOV (rad)
    pub fov: f32,
    pub pitch_min: f32,
    pub pitch_max: f32,
    /// The zoom's range: the frame's width at the subject (m)
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub half_life_s: f32,
    pub snap_m: f32,
    pub look_up_m: f32,
    /// Q / E (rad/s), Z / X (rad/s), a dragged pixel (rad)
    pub rotate_s: f32,
    pub pitch_s: f32,
    pub drag_px: f32,
    /// One wheel notch / key press multiplies the zoom by this (or divides)
    pub zoom_step: f32,
    pub far_m: f32,
}

impl CamCfg {
    pub fn from_view(v: &citysim::config::ViewCfg) -> CamCfg {
        CamCfg {
            fov: v.cam_fov_deg.to_radians(),
            pitch_min: v.cam_pitch_min_deg.to_radians(),
            pitch_max: v.cam_pitch_max_deg.to_radians(),
            zoom_min: v.cam_zoom_min_m,
            zoom_max: v.cam_zoom_max_m,
            half_life_s: v.cam_follow_half_life_s,
            snap_m: v.cam_snap_m,
            look_up_m: v.cam_look_up_m,
            rotate_s: v.cam_rotate_deg_s.to_radians(),
            pitch_s: v.cam_pitch_deg_s.to_radians(),
            drag_px: v.cam_drag_deg_px.to_radians(),
            zoom_step: v.cam_zoom_step,
            far_m: v.cam_far_m,
        }
    }
}

/// Night rig of the city view. glclient's (SPEC §LGT: world 0.05, moon 0.08) assumes a lamp-lit street; before
/// P7 the city had no lamps, so massing and ground were black: a sky glow (an ambient in Grade A's fog hue, the
/// light-polluted sky) and a stronger moon from the NW give the silhouettes. With the P7 street lamps loaded
/// (`lamps`) the rig drops to 80 / 150 so their pools read (s30: at 250 / 400 the ambient flattened them).
/// Env-tunable for look rounds (CV_SKY, CV_MOON).
fn night(lamps: bool) -> (f32, f32) {
    let f = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    if lamps { (f("CV_SKY", 80.0), f("CV_MOON", 150.0)) } else { (f("CV_SKY", 250.0), f("CV_MOON", 400.0)) }
}
const SKY: Color = Color::srgb(0.40, 0.38, 0.48);
const MOON: Color = Color::srgb(0.6, 0.65, 1.0);
/// The overview's sky at the horizon: a lighter Grade A (#2A2040) so the ridge and coast silhouette. The clear
/// colour and the fog are pre-tonemapping linear values that AgX crushes in the toe: x7 lands near #2A2040 on
/// screen (x4 read black, x12 saturated blue). CV_HORIZON overrides the scale (look rounds).
fn horizon() -> Color {
    let k = std::env::var("CV_HORIZON").ok().and_then(|v| v.parse().ok()).unwrap_or(7.0f32);
    let c = LinearRgba::from(Color::srgb(0.17, 0.14, 0.22));
    Color::LinearRgba(LinearRgba::rgb(c.red * k, c.green * k, c.blue * k))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Game,
    Overview,
}

#[derive(Resource, Clone, Debug)]
pub struct View {
    pub mode: Mode,
    /// Game: the subject's ground point (world x, y) and its z (the followed body's own z: a storey, a deck)
    pub at: Vec2,
    pub ground_z: f32,
    /// Game: the frame's width at the subject (m)
    pub zoom: f32,
    /// Game: the orbit's yaw and pitch (rad)
    pub game_yaw: f32,
    pub game_pitch: f32,
    /// Game: the eased aim (world), None until the first frame (it snaps there)
    pub aim: Option<Vec3>,
    /// Overview orbit: centre (world), yaw (rad, 0 = camera south of the centre), pitch (rad), distance (m)
    pub orbit_at: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub day: bool,
    /// Debug: hide every massing mesh (M)
    pub hide_mass: bool,
    /// Bench / shots drive the view: keys off, the aim snaps (a shot shows the tick's place, not the ease)
    pub scripted: bool,
    /// The game camera follows the player (occ.rs: WASD walks the player instead of panning)
    pub player: bool,
    /// The window's width over its height (the zoom's distance needs the horizontal FOV)
    pub aspect: f32,
    pub cfg: CamCfg,
}

impl View {
    pub fn new(at: Vec2, zoom: f32, island: Vec2, cfg: CamCfg, yaw_deg: f32, pitch_deg: f32, aspect: f32) -> View {
        let mut v = View {
            mode: Mode::Game,
            at,
            ground_z: 3.0,
            zoom: 0.0,
            game_yaw: yaw_deg.to_radians(),
            game_pitch: 0.0,
            aim: None,
            orbit_at: Vec3::new(island.x / 2.0, island.y / 2.0, 60.0),
            // from the SE, like the game camera
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: 0.5,
            dist: 4800.0,
            day: false,
            hide_mass: false,
            scripted: false,
            player: false,
            aspect,
            cfg,
        };
        v.set_zoom(zoom);
        v.set_pitch(pitch_deg.to_radians());
        v
    }

    pub fn set_zoom(&mut self, z: f32) {
        self.zoom = z.clamp(self.cfg.zoom_min, self.cfg.zoom_max);
    }

    pub fn set_pitch(&mut self, p: f32) {
        self.game_pitch = p.clamp(self.cfg.pitch_min, self.cfg.pitch_max);
    }

    /// The game camera's distance from its aim: the zoom's width across the horizontal FOV.
    pub fn game_dist(&self) -> f32 {
        let tan_h = (self.cfg.fov * 0.5).tan() * self.aspect;
        self.zoom * 0.5 / tan_h.max(1e-3)
    }

    /// The point the game camera eases toward (world): the subject's place, `look_up_m` over the feet.
    pub fn goal(&self) -> Vec3 {
        self.at.extend(self.ground_z + self.cfg.look_up_m)
    }

    /// Forward (away from the camera) and right on the ground (world xy) for the game camera's yaw.
    pub fn pan_axes(&self) -> (Vec2, Vec2) {
        let (s, c) = self.game_yaw.sin_cos();
        (Vec2::new(-s, c), Vec2::new(c, s))
    }

    /// The game camera's eye and aim (world).
    pub fn game_eye(&self) -> (Vec3, Vec3) {
        let aim = self.aim.unwrap_or_else(|| self.goal());
        let (s, c) = self.game_yaw.sin_cos();
        let (ps, pc) = self.game_pitch.sin_cos();
        (aim + Vec3::new(s * pc, -c * pc, ps) * self.game_dist(), aim)
    }

    pub fn transform(&self) -> Transform {
        match self.mode {
            Mode::Game => {
                let (eye, aim) = self.game_eye();
                Transform::from_translation(bv(eye.x, eye.y, eye.z)).looking_at(bv(aim.x, aim.y, aim.z), Vec3::Y)
            }
            Mode::Overview => {
                let c = self.orbit_at;
                // camera offset in world: south-east of the centre at yaw 45 deg
                let off = Vec3::new(self.yaw.sin() * self.pitch.cos(), -self.yaw.cos() * self.pitch.cos(), self.pitch.sin()) * self.dist;
                let eye = bv(c.x + off.x, c.y + off.y, c.z + off.z);
                Transform::from_translation(eye).looking_at(bv(c.x, c.y, c.z), Vec3::Y)
            }
        }
    }

    /// (fov, near, far) of the mode.
    fn lens(&self) -> (f32, f32, f32) {
        match self.mode {
            Mode::Game => (self.cfg.fov, 0.5, self.cfg.far_m),
            Mode::Overview => (40f32.to_radians(), 2.0, 30000.0),
        }
    }

    pub fn projection(&self) -> Projection {
        let (fov, near, far) = self.lens();
        Projection::Perspective(PerspectiveProjection { fov, near, far, aspect_ratio: self.aspect, ..default() })
    }

    /// The aim eases toward the goal (an exponential ease of `half_life_s`); a jump past `snap_m`, the first
    /// frame and a scripted view snap.
    pub fn ease(&mut self, dt: f32) -> bool {
        let goal = self.goal();
        let next = match self.aim {
            Some(a) if !self.scripted && a.distance(goal) < self.cfg.snap_m => {
                let k = 1.0 - (-dt * std::f32::consts::LN_2 / self.cfg.half_life_s.max(1e-3)).exp();
                let n = a + (goal - a) * k;
                if n.distance(goal) < 0.005 { goal } else { n }
            }
            _ => goal,
        };
        let moved = self.aim != Some(next);
        self.aim = Some(next);
        moved
    }
}

#[derive(Component)]
pub struct CityCam;

#[derive(Component)]
pub struct Moon;

/// The city's own glow: a low warm fill from the S (light bounced off the lit streets and the cloud base), so
/// the S and E facades the camera sees read as volumes, the S brighter than the E (review p3 r2: facades were a
/// uniform void, 93-97 % of pixels under 0.15).
#[derive(Component)]
pub struct Glow;

fn glow() -> f32 {
    std::env::var("CV_GLOW").ok().and_then(|v| v.parse().ok()).unwrap_or(600.0)
}
const GLOW: Color = Color::srgb(1.0, 0.78, 0.6);

pub fn setup_camera(mut commands: Commands, view: Res<View>, street: Option<Res<glclient::street::Street>>) {
    let mut cam = commands.spawn((
        Camera3d::default(),
        view.projection(),
        view.transform(),
        Hdr,
        Tonemapping::AgX,
        // glclient's look: the Blender renders' +0.6 EV over Bevy's EV100 9.7
        bevy::camera::Exposure { ev100: 9.1 },
        Bloom::NATURAL,
        Msaa::Off,
        // the cutaway runs in this camera's views only
        glclient::occlusion::OccCam,
        CityCam,
        // glclient's light rules (clusters, shadow resolutions) for the pinned block's lights (/code-review s22)
        glclient::frame::LightCam,
    ));
    // the silhouettes read the depth prepass; the pre-P4 baseline (CV_NO_OCC) has none
    if glclient::occlusion::built() {
        cam.insert(bevy::core_pipeline::prepass::DepthPrepass);
    }
    // night (glclient / SPEC §LGT): a weak blue moon from the NW, near-zero ambient
    let (sky, moon) = night(street.is_some());
    commands.spawn((
        DirectionalLight { illuminance: moon, color: MOON, shadow_maps_enabled: false, ..default() },
        // from the NW (SPEC §LGT), high: lights the roofs and the NW faces a little
        Transform::from_xyz(0.0, 0.0, 0.0).looking_to(Vec3::new(0.55, -0.75, 0.35), Vec3::Y),
        Moon,
    ));
    commands.spawn((
        DirectionalLight { illuminance: glow(), color: GLOW, shadow_maps_enabled: false, ..default() },
        // from azimuth ~175 deg (S by E), 12 deg up: the S faces catch it, the E faces barely (two face values)
        Transform::default().looking_to(Vec3::new(-0.09, -0.21, -0.97), Vec3::Y),
        Glow,
    ));
    commands.insert_resource(GlobalAmbientLight { color: SKY, brightness: sky, ..default() });
    commands.insert_resource(ClearColor(Color::srgb(0.005, 0.005, 0.012)));
}

pub fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    time: Res<Time>,
    mut view: ResMut<View>,
) {
    let scroll: f32 = wheel.read().map(|e| e.y.signum()).sum();
    if view.scripted {
        return;
    }
    let dt = time.delta_secs();
    let held = |k: &[KeyCode]| k.iter().any(|k| keys.pressed(*k));
    // + = closer
    let mut zoom_in = scroll;
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        zoom_in += 1.0;
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        zoom_in -= 1.0;
    }
    if keys.just_pressed(KeyCode::Tab) {
        view.mode = if view.mode == Mode::Game { Mode::Overview } else { Mode::Game };
    }
    if keys.just_pressed(KeyCode::KeyL) {
        view.day = !view.day;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        view.hide_mass = !view.hide_mass;
    }
    // rotate / pitch: keys and a right- or middle-drag
    let mut d_yaw = 0.0;
    let mut d_pitch = 0.0;
    if held(&[KeyCode::KeyQ]) {
        d_yaw -= view.cfg.rotate_s * dt;
    }
    if held(&[KeyCode::KeyE]) {
        d_yaw += view.cfg.rotate_s * dt;
    }
    if held(&[KeyCode::KeyZ]) {
        d_pitch -= view.cfg.pitch_s * dt;
    }
    if held(&[KeyCode::KeyX]) {
        d_pitch += view.cfg.pitch_s * dt;
    }
    if (buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle)) && motion.delta != Vec2::ZERO {
        d_yaw += motion.delta.x * view.cfg.drag_px;
        d_pitch += motion.delta.y * view.cfg.drag_px;
    }
    let step = view.cfg.zoom_step;
    match view.mode {
        Mode::Game => {
            if d_yaw != 0.0 {
                view.game_yaw = (view.game_yaw + d_yaw).rem_euclid(std::f32::consts::TAU);
            }
            if d_pitch != 0.0 {
                let p = view.game_pitch + d_pitch;
                view.set_pitch(p);
            }
            if zoom_in != 0.0 {
                let z = view.zoom / step.powf(zoom_in);
                view.set_zoom(z);
            }
            if !view.player {
                let (fwd, right) = view.pan_axes();
                let mut d = Vec2::ZERO;
                if held(&[KeyCode::KeyW, KeyCode::ArrowUp]) {
                    d += fwd;
                }
                if held(&[KeyCode::KeyS, KeyCode::ArrowDown]) {
                    d -= fwd;
                }
                if held(&[KeyCode::KeyD, KeyCode::ArrowRight]) {
                    d += right;
                }
                if held(&[KeyCode::KeyA, KeyCode::ArrowLeft]) {
                    d -= right;
                }
                let speed = view.zoom * if keys.pressed(KeyCode::ShiftLeft) { 3.0 } else { 0.8 };
                if d != Vec2::ZERO {
                    view.at += d.normalize() * speed * dt;
                }
            }
        }
        Mode::Overview => {
            if held(&[KeyCode::ArrowLeft]) {
                view.yaw -= dt * 0.8;
            }
            if held(&[KeyCode::ArrowRight]) {
                view.yaw += dt * 0.8;
            }
            if held(&[KeyCode::ArrowUp]) {
                view.pitch = (view.pitch + dt * 0.5).min(1.5);
            }
            if held(&[KeyCode::ArrowDown]) {
                view.pitch = (view.pitch - dt * 0.5).max(0.05);
            }
            view.yaw += d_yaw;
            view.pitch = (view.pitch + d_pitch).clamp(0.05, 1.5);
            let (s, c) = view.yaw.sin_cos();
            // forward = from the camera toward the centre, on the ground
            let fwd = Vec2::new(-s, c);
            let right = Vec2::new(c, s);
            let mut d = Vec2::ZERO;
            if keys.pressed(KeyCode::KeyW) {
                d += fwd;
            }
            if keys.pressed(KeyCode::KeyS) {
                d -= fwd;
            }
            if keys.pressed(KeyCode::KeyD) {
                d += right;
            }
            if keys.pressed(KeyCode::KeyA) {
                d -= right;
            }
            let sp = view.dist * 0.4 * dt;
            view.orbit_at += (d * sp).extend(0.0);
            if zoom_in != 0.0 {
                view.dist = (view.dist / step.powf(zoom_in)).clamp(150.0, 20000.0);
            }
        }
    }
}

/// The game camera's aim eases toward the subject (after the subject's place and the ground z are set).
pub fn ease_view(time: Res<Time>, mut view: ResMut<View>) {
    let dt = time.delta_secs();
    if view.bypass_change_detection().ease(dt) {
        view.set_changed();
    }
}

/// View -> the camera, the light rig and glclient's `Focus` (its light allocator scores around it).
pub fn apply_view(
    mut commands: Commands,
    view: Res<View>,
    mut cam: Query<(Entity, &mut Transform, &mut Projection, &mut bevy::camera::Exposure), With<CityCam>>,
    mut moon: Query<&mut DirectionalLight, (With<Moon>, Without<Glow>)>,
    mut glow_q: Query<&mut DirectionalLight, (With<Glow>, Without<Moon>)>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut focus: ResMut<Focus>,
    street: Option<Res<glclient::street::Street>>,
) {
    if !view.is_changed() {
        return;
    }
    let lamps = street.is_some();
    for (ent, mut t, mut p, mut e) in cam.iter_mut() {
        *t = view.transform();
        // haze only in the overview (the game view has the windows' height haze)
        if view.mode == Mode::Overview && !view.day {
            commands.entity(ent).insert(DistanceFog {
                color: horizon(),
                falloff: FogFalloff::from_visibility_colors(24000.0, horizon(), horizon()),
                ..default()
            });
        } else {
            commands.entity(ent).remove::<DistanceFog>();
        }
        // in place: Bevy keeps the aspect ratio up to date on the projection
        let (fov, near, far) = view.lens();
        if let Projection::Perspective(pp) = p.as_mut() {
            if pp.fov != fov || pp.near != near || pp.far != far {
                pp.fov = fov;
                pp.near = near;
                pp.far = far;
            }
        } else {
            *p = view.projection();
        }
        // day: a debug sun to read the structure (not the look)
        e.ev100 = if view.day { 13.0 } else { 9.1 };
    }
    for mut m in moon.iter_mut() {
        if view.day {
            m.illuminance = 12000.0;
            m.color = Color::srgb(1.0, 0.96, 0.9);
        } else {
            m.illuminance = night(lamps && view.mode == Mode::Game).1;
            m.color = MOON;
        }
    }
    for mut g in glow_q.iter_mut() {
        g.illuminance = if view.day { 0.0 } else { glow() };
    }
    ambient.brightness = if view.day { 1500.0 } else { night(lamps && view.mode == Mode::Game).0 };
    ambient.color = if view.day { Color::srgb(0.75, 0.8, 0.9) } else { SKY };
    // the overview's horizon fades into the haze, not into black
    clear.0 = if view.mode == Mode::Overview && !view.day { horizon() } else { Color::srgb(0.005, 0.005, 0.012) };
    let a = view.aim.unwrap_or_else(|| view.goal());
    focus.at = bv(a.x, a.y, a.z - view.cfg.look_up_m);
    focus.zoom = view.zoom;
}
