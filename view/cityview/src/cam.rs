//! Cameras: the game camera (SPEC §CAM 5.1: orthographic, looking NW 30 deg down; glclient `frame::view_dir`)
//! and a free overview (perspective orbit over the island). One camera entity; Tab swaps the projection.
//!
//! Keys: WASD / arrows pan (screen-aligned in the game view; the orbit centre in the overview), Q / E or the
//! wheel zoom (game: 26 / 60 / 140 / 400 / 1000 m wide; overview: distance), arrows in the overview orbit,
//! Tab game <-> overview, L night <-> day light (structure reading), M hide the massing (debug), P screenshot, H hud.

use bevy::{
    camera::{Hdr, ScalingMode},
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::MouseWheel,
    post_process::bloom::Bloom,
    prelude::*,
};
use glclient::frame::{Focus, view_dir};

use crate::mesh::bv;

pub const ZOOMS: [f32; 5] = [26.0, 60.0, 140.0, 400.0, 1000.0];

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
/// The game camera sits this far back along its view direction (ortho: only the clip range cares).
const BACK: f32 = 4000.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Game,
    Overview,
}

#[derive(Resource, Clone, Debug)]
pub struct View {
    pub mode: Mode,
    /// Game: the ground point at the view centre (world x, y); its z follows the ground (`ground_z`)
    pub at: Vec2,
    pub ground_z: f32,
    pub zoom: f32,
    /// Overview orbit: centre (world), yaw (rad, 0 = camera south of the centre), pitch (rad), distance (m)
    pub orbit_at: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub day: bool,
    /// Debug: hide every massing mesh (M)
    pub hide_mass: bool,
    /// Bench / shots drive the view: keys off
    pub scripted: bool,
    /// The game camera follows the player (occ.rs: WASD walks the player instead of panning)
    pub player: bool,
}

impl View {
    pub fn new(at: Vec2, zoom: f32, island: Vec2) -> View {
        View {
            mode: Mode::Game,
            at,
            ground_z: 3.0,
            zoom,
            orbit_at: Vec3::new(island.x / 2.0, island.y / 2.0, 60.0),
            // from the SE, like the game camera
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: 0.5,
            dist: 4800.0,
            day: false,
            hide_mass: false,
            scripted: false,
            player: false,
        }
    }

    /// Screen-up (NW) and screen-right (NE) directions on the ground (world xy) in the game view.
    pub fn pan_axes() -> (Vec2, Vec2) {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        (Vec2::new(-s, s), Vec2::new(s, s))
    }

    pub fn transform(&self) -> Transform {
        match self.mode {
            Mode::Game => {
                let d = view_dir();
                let target = bv(self.at.x, self.at.y, self.ground_z);
                Transform::from_translation(target - d * BACK).looking_to(d, Vec3::Y)
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

    pub fn projection(&self) -> Projection {
        match self.mode {
            Mode::Game => Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedHorizontal { viewport_width: self.zoom },
                near: 1.0,
                far: BACK * 2.5,
                ..OrthographicProjection::default_3d()
            }),
            Mode::Overview => {
                Projection::Perspective(PerspectiveProjection { fov: 40f32.to_radians(), near: 2.0, far: 30000.0, ..default() })
            }
        }
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
        // the cut runs in this camera's views only (P4)
        glclient::occlusion::OccCam,
        CityCam,
        // glclient's light rules (clusters, shadow resolutions) for the pinned block's lights (/code-review s22)
        glclient::frame::LightCam,
    ));
    // the occlusion silhouettes read the depth prepass; the pre-P4 baseline (CV_NO_OCC) has none
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

pub fn controls(keys: Res<ButtonInput<KeyCode>>, mut wheel: MessageReader<MouseWheel>, time: Res<Time>, mut view: ResMut<View>) {
    let scroll: f32 = wheel.read().map(|e| e.y.signum()).sum();
    if view.scripted {
        return;
    }
    let dt = time.delta_secs();
    let held = |k: &[KeyCode]| k.iter().any(|k| keys.pressed(*k));
    let mut zoom_step = 0i32;
    if keys.just_pressed(KeyCode::KeyQ) || scroll > 0.0 {
        zoom_step -= 1;
    }
    if keys.just_pressed(KeyCode::KeyE) || scroll < 0.0 {
        zoom_step += 1;
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
    match view.mode {
        Mode::Game if view.player => {
            if zoom_step != 0 {
                let k = ZOOMS.iter().position(|z| (*z - view.zoom).abs() < 0.5).unwrap_or(1) as i32;
                view.zoom = ZOOMS[(k + zoom_step).clamp(0, ZOOMS.len() as i32 - 1) as usize];
            }
        }
        Mode::Game => {
            let (up, right) = View::pan_axes();
            let mut d = Vec2::ZERO;
            if held(&[KeyCode::KeyW, KeyCode::ArrowUp]) {
                d += up;
            }
            if held(&[KeyCode::KeyS, KeyCode::ArrowDown]) {
                d -= up;
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
            if zoom_step != 0 {
                let k = ZOOMS.iter().position(|z| (*z - view.zoom).abs() < 0.5).unwrap_or(1) as i32;
                view.zoom = ZOOMS[(k + zoom_step).clamp(0, ZOOMS.len() as i32 - 1) as usize];
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
            if zoom_step != 0 {
                view.dist = (view.dist * if zoom_step < 0 { 0.8 } else { 1.25 }).clamp(150.0, 20000.0);
            }
        }
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
        // haze only in the perspective overview (an ortho view's fragments are all ~BACK m away)
        if view.mode == Mode::Overview && !view.day {
            commands.entity(ent).insert(DistanceFog {
                color: horizon(),
                falloff: FogFalloff::from_visibility_colors(24000.0, horizon(), horizon()),
                ..default()
            });
        } else {
            commands.entity(ent).remove::<DistanceFog>();
        }
        let np = view.projection();
        let same = matches!(
            (&*p, &np),
            (Projection::Orthographic(_), Projection::Orthographic(_)) | (Projection::Perspective(_), Projection::Perspective(_))
        );
        if !same {
            *p = np;
        } else if let (Projection::Orthographic(o), Mode::Game) = (p.as_mut(), view.mode) {
            o.scaling_mode = ScalingMode::FixedHorizontal { viewport_width: view.zoom };
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
    focus.at = bv(view.at.x, view.at.y, view.ground_z);
    focus.zoom = view.zoom;
}
