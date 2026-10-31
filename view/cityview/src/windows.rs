//! The city view's materials: the massing (StandardMaterial + procedural emissive windows, windows.wgsl) and the
//! ground / decks (StandardMaterial + the shared height haze, ground.wgsl). `update_haze` keeps the haze's
//! reference height on the view (game camera: the ground under the view centre + HAZE_ABOVE).
//! The massing and the elevated layers are occluders (P4, glclient::occlusion): their extensions carry the
//! occlusion uniform at binding 101, run as AlphaMode::Mask with glclient's prepass fragment (the same discard);
//! `update_occ` copies the shared state into them each frame. The ground never cuts (it stays Opaque).

use bevy::{
    asset::embedded_asset,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::ShaderRef,
};

use glclient::occlusion::{OccFlags, OccUniform, Occlusion};

use crate::cam::{Mode, View};

pub type WindowMaterial = ExtendedMaterial<StandardMaterial, Windows>;
pub type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundHaze>;
pub type LayerMaterial = ExtendedMaterial<StandardMaterial, LayerOcc>;

/// The massing cuts (L2); deck pieces fade as wholes (L3); pylons / beams cut.
pub const MASSING: OccFlags = OccFlags { keep_up: false, fade: false, cut: true, gated: false };
pub const DECKS: OccFlags = OccFlags { keep_up: false, fade: true, cut: false, gated: false };
pub const PYLONS: OccFlags = OccFlags { keep_up: false, fade: false, cut: true, gated: false };


/// The haze reaches its maximum this far under the reference height (m), which sits this far over the ground
/// under the view centre: the street gets ~0.3 of it, roofs at 60 m none.
const HAZE_RANGE: f32 = 160.0;
const HAZE_ABOVE: f32 = 60.0;
const HAZE_MAX: f32 = 0.65;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Windows {
    /// rgb window_warm (linear), a = emissive strength scale
    #[uniform(100)]
    pub warm: Vec4,
    /// rgb fluoro_cold (linear)
    #[uniform(100)]
    pub cold: Vec4,
    /// rgb glass_dark (linear), a = its roughness
    #[uniform(100)]
    pub glass: Vec4,
    /// y lit-share multiplier, z wall albedo scale
    #[uniform(100)]
    pub misc: Vec4,
    /// rgb crown key hue (linear), a = strength
    #[uniform(100)]
    pub key: Vec4,
    /// rgb haze (linear), a = max amount
    #[uniform(100)]
    pub haze: Vec4,
    /// x reference z, y range
    #[uniform(100)]
    pub haze_z: Vec4,
    #[uniform(101)]
    pub occ: OccUniform,
}

impl MaterialExtension for Windows {
    fn fragment_shader() -> ShaderRef {
        "embedded://cityview/windows.wgsl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        glclient::occlusion::occ_prepass_shader()
    }
    fn alpha_mode() -> Option<AlphaMode> {
        glclient::occlusion::occ_alpha_mode()
    }
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        glclient::occlusion::occ_specialize(descriptor);
        Ok(())
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GroundHaze {
    #[uniform(100)]
    pub haze: Vec4,
    #[uniform(100)]
    pub haze_z: Vec4,
}

impl MaterialExtension for GroundHaze {
    fn fragment_shader() -> ShaderRef {
        "embedded://cityview/ground.wgsl".into()
    }
}

/// The layers' material: the ground's haze + the occlusion uniform.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct LayerOcc {
    #[uniform(100)]
    pub haze: Vec4,
    #[uniform(100)]
    pub haze_z: Vec4,
    #[uniform(101)]
    pub occ: OccUniform,
    /// Which occlusion this instance does (decks / pylons); not a shader field
    #[reflect(ignore)]
    pub flags: Option<OccFlags>,
}

impl MaterialExtension for LayerOcc {
    fn fragment_shader() -> ShaderRef {
        "embedded://cityview/layers.wgsl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        glclient::occlusion::occ_prepass_shader()
    }
    fn alpha_mode() -> Option<AlphaMode> {
        glclient::occlusion::occ_alpha_mode()
    }
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        glclient::occlusion::occ_specialize(descriptor);
        Ok(())
    }
}

pub struct WindowsPlugin;

impl Plugin for WindowsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "windows.wgsl");
        embedded_asset!(app, "ground.wgsl");
        embedded_asset!(app, "layers.wgsl");
        app.add_plugins((
            MaterialPlugin::<WindowMaterial>::default(),
            MaterialPlugin::<GroundMaterial>::default(),
            MaterialPlugin::<LayerMaterial>::default(),
        ));
    }
}

fn lin(hex: u32) -> Vec3 {
    let c = LinearRgba::from(Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8));
    Vec3::new(c.red, c.green, c.blue)
}

fn haze_colour() -> Vec3 {
    lin(0x1B1430)
}

/// SPEC §PAL: window_warm #FFD9A0 / fluoro_cold #E8F4FF (on screen), glass_dark #0E1218, Grade A key magenta #FF2E88.
/// Strength 0.6-1.4 x `strength`: Blender's 1.5-4 went white through AgX at EV 9.1 (review p3 r1: warm windows
/// at 11 % saturation; the MUST is no pure-white emissive).
pub fn material(strength: f32) -> WindowMaterial {
    ExtendedMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.8, ..default() },
        extension: Windows {
            // fed more saturated than the palette tokens: AgX desaturates them back toward #FFD9A0 / #E8F4FF
            // (review p3 r2 measured warm at #C8BDA8, 0.16 saturation, cold neutral grey)
            warm: lin(0xFFB060).extend(strength),
            cold: lin(0x8CD2FF).extend(1.0),          // review p3 r3: #D8F0FF came out grey paper
            glass: lin(0x0E1218).extend(0.15),
            misc: Vec4::new(0.0, 1.0, 1.0, 0.0),
            key: lin(0xFF2E88).extend(2.5),
            haze: haze_colour().extend(0.0),
            haze_z: Vec4::new(0.0, HAZE_RANGE, 0.0, 0.0),
            occ: OccUniform::default(),
        },
    }
}

pub fn ground_material(roughness: f32) -> GroundMaterial {
    ExtendedMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: roughness, ..default() },
        extension: GroundHaze { haze: haze_colour().extend(0.0), haze_z: Vec4::new(0.0, HAZE_RANGE, 0.0, 0.0) },
    }
}

pub fn layer_material(roughness: f32, flags: OccFlags) -> LayerMaterial {
    ExtendedMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: roughness, ..default() },
        extension: LayerOcc {
            haze: haze_colour().extend(0.0),
            haze_z: Vec4::new(0.0, HAZE_RANGE, 0.0, 0.0),
            occ: OccUniform::default(),
            flags: Some(flags),
        },
    }
}

/// The occlusion state -> the occluder materials (a handful of assets: one window material, two layer ones).
/// Only when the uniform changed or a material was added (`Occlusion` is written every frame: its change flag says
/// nothing, and a touched material re-uploads its bind group).
pub fn update_occ(
    occ: Res<Occlusion>,
    mut wm: ResMut<Assets<WindowMaterial>>,
    mut lm: ResMut<Assets<LayerMaterial>>,
    mut last: Local<Option<(OccUniform, usize)>>,
) {
    let key = Some((occ.uniform, wm.len() + lm.len()));
    if *last == key {
        return;
    }
    *last = key;
    let ids: Vec<_> = wm.ids().collect();
    for id in ids {
        if let Some(mut m) = wm.get_mut(id) {
            m.extension.occ = occ.uniform(MASSING);
        }
    }
    let ids: Vec<_> = lm.ids().collect();
    for id in ids {
        if let Some(mut m) = lm.get_mut(id) {
            let f = m.extension.flags.unwrap_or(PYLONS);
            m.extension.occ = occ.uniform(f);
        }
    }
}

/// The haze follows the game view (off in the overview, which has distance fog, and in the day light).
pub fn update_haze(
    view: Res<View>,
    mut wm: ResMut<Assets<WindowMaterial>>,
    mut gm: ResMut<Assets<GroundMaterial>>,
    mut lm: ResMut<Assets<LayerMaterial>>,
    mut last: Local<Vec2>,
) {
    let amount = if view.mode == Mode::Game && !view.day { HAZE_MAX } else { 0.0 };
    let z = view.ground_z + HAZE_ABOVE;
    // only when it moved (a material change re-uploads its uniform)
    if (last.x - z).abs() < 0.5 && last.y == amount {
        return;
    }
    *last = Vec2::new(z, amount);
    for (_, m) in wm.iter_mut() {
        m.extension.haze.w = amount;
        m.extension.haze_z.x = z;
    }
    for (_, m) in gm.iter_mut() {
        m.extension.haze.w = amount;
        m.extension.haze_z.x = z;
    }
    for (_, m) in lm.iter_mut() {
        m.extension.haze.w = amount;
        m.extension.haze_z.x = z;
    }
}
