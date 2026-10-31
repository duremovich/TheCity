//! Frames and the game camera. Block / building frame: x east, y north, z up (metres); Bevy / glTF: +Y up, so a
//! point (x, y, z) is (x, z, -y). SPEC 5.1: orthographic, Blender rotation (60, 0, 45) = looking north-west,
//! 30 deg down; ortho width 26 for one 16 m lot.

use bevy::{camera::ScalingMode, prelude::*};

/// Block / building frame -> Bevy.
pub fn to_bevy(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.z, -p.y)
}

/// Bevy -> block / building frame.
pub fn from_bevy(p: Vec3) -> Vec3 {
    Vec3::new(p.x, -p.z, p.y)
}

#[derive(Resource, Clone, Debug)]
pub struct Focus {
    /// Ground point at the view centre (Bevy).
    pub at: Vec3,
    /// Ortho width in metres (SPEC 5.1: 26 for one 16 m lot).
    pub zoom: f32,
}

/// The bench street's camera: `sync_camera` drives it from `Focus`.
#[derive(Component)]
pub struct MainCam;

/// The camera the light rules apply to (`lighting::apply_features`: clusters, SSAO, fog, shadow resolutions). The
/// bench street's MainCam carries it; a host that drives its own camera (engine/cityview) adds only this.
#[derive(Component)]
pub struct LightCam;

/// SPEC 5.1 view direction (Bevy).
pub fn view_dir() -> Vec3 {
    let (s45, c60, s60) = (std::f32::consts::FRAC_1_SQRT_2, 0.5_f32, 3f32.sqrt() / 2.0);
    // Blender: (-s60 s45, s60 s45, -c60) -> Bevy (x, z, -y)
    Vec3::new(-s60 * s45, -c60, -s60 * s45).normalize()
}

pub fn cam_transform(f: &Focus) -> Transform {
    let d = view_dir();
    // aim a little above the street so a 15 m frontage sits in frame
    let target = f.at + Vec3::Y * 4.0;
    Transform::from_translation(target - d * 150.0).looking_to(d, Vec3::Y)
}

/// The camera follows `Focus` (pan / zoom).
pub fn sync_camera(focus: Res<Focus>, mut cam: Query<(&mut Transform, &mut Projection), With<MainCam>>) {
    if !focus.is_changed() {
        return;
    }
    for (mut t, mut p) in cam.iter_mut() {
        *t = cam_transform(&focus);
        if let Projection::Orthographic(o) = p.as_mut() {
            o.scaling_mode = ScalingMode::FixedHorizontal { viewport_width: focus.zoom };
        }
    }
}
