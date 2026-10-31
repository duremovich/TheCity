//! Camera occlusion (CITY_SPEC §CAM, spec/city/P4_PLAN.md A): the rules any host shares.
//!
//! - **Volumes** (`Occluded`, <= 4: the player, the squad): each gets a see-through capsule (L2, `occlusion.wgsl`)
//!   whose radius eases in when the volume is blocked and out when it is clear. Blocked = rays from sample points
//!   on the body toward the camera hit something: enter on the narrow set, leave only when the wide set (the
//!   points pushed out sideways and up: spatial hysteresis) has been clear for `exit_hold_s` (time hysteresis).
//! - **L3 fades** (`OccFade` + `MeshTag`): objects a narrow ray hit fade (screen-door) to a floor, with hysteresis.
//! - **Casting**: every `cast_period_s`, `occ_prepare` lays out the rays (`OccCast`) and tests the building glbs'
//!   visible nodes (their boxes); the host tests its own geometry in `OccSet::Cast` (PostUpdate, after transform
//!   propagation; cityview: massing boxes, deck pieces), then `occ_apply` runs the state machines and fills the
//!   uniform (hosts copy it into their materials after `OccSet::Apply`).
//! - **Materials**: `OccStd` replaces every StandardMaterial of a building glb (walkable faces kept); hosts embed
//!   `OccUniform` at binding 101 in their own occluder materials and copy `Occlusion::uniform(flags)` into them.
//! - **Per-building keep** (s28): a building that is part of the scene around a body (it holds one of the context
//!   points: the vertical plane through the feet, the ground in front) stays whole (`KEEP_BIT` on its meshes'
//!   `MeshTag`) unless a body's ray hit it within `exit_hold_s + ease_s` of the last hit: a neighbour whose front
//!   juts toward the camera inside the capsule is not cut. Everything else cuts as before (a deny list: an allow
//!   list missed occluders between the rays, the squad gate s28). Host occluders are never kept.
//! - **L1**: the first volume inside a pinned building (its block nav) cuts that building at its level.
//! - **L4**: `SilhouetteSource` glbs get a silhouette twin per mesh (needs `DepthPrepass` on the camera).
//! - Controls (env, `OccSettings::from_env`): CV_OCC_SHADOW_CUT=1 cuts in shadow views too, CV_OCC_NO_HYST=1 drops
//!   both hystereses, CV_OCC_NO_PREPASS_CUT=1 keeps the prepass whole, CV_OCC_NO_GLB_RAYS=1 skips the glbs' ray
//!   test (outside a building only the host's geometry hides a body: the pre-review behaviour).

use bevy::{
    asset::embedded_asset,
    camera::primitives::Aabb,
    light::NotShadowCaster,
    mesh::{MeshTag, MeshVertexBufferLayoutRef, skinning::SkinnedMesh},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError},
    shader::{ShaderRef, load_shader_library},
    transform::TransformSystems,
    world_serialization::WorldInstanceReady,
};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::city::{Bldg, City, GlNode};

pub const MAX_VOLUMES: usize = 4;

/// Whether the occluder materials are built for the cut (AlphaMode::Mask + the prepass discard). A host clears it
/// before building the app to get the pre-P4 pipeline (Opaque, early-Z) as a GPU-cost baseline.
pub static BUILT: AtomicBool = AtomicBool::new(true);

pub fn built() -> bool {
    BUILT.load(Ordering::Relaxed)
}

/// The occluder materials' alpha mode / prepass shader (Mask + glclient's prepass while built).
pub fn occ_alpha_mode() -> Option<AlphaMode> {
    built().then_some(AlphaMode::Mask(0.5))
}

pub fn occ_prepass_shader() -> ShaderRef {
    if built() { "embedded://glclient/occ_prepass.wgsl".into() } else { ShaderRef::Default }
}

/// Every occluder material's `specialize`: the pre-P4 baseline compiles `occ_apply` out (OCC_OFF: no discard in
/// the main pass, so early-Z is what it was before P4).
pub fn occ_specialize(descriptor: &mut RenderPipelineDescriptor) {
    if !built() {
        if let Some(f) = descriptor.fragment.as_mut() {
            f.shader_defs.push("OCC_OFF".into());
        }
    }
}

/// The shader's `Occ` (occlusion.wgsl). The global part comes from `Occlusion::uniform`, `mat` per material.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct OccUniform {
    pub view_from_world: Mat4,
    pub cam: Vec4,
    pub feet: [Vec4; MAX_VOLUMES],
    pub head: [Vec4; MAX_VOLUMES],
    pub misc: Vec4,
    pub rim: Vec4,
    /// x keep_up, y L3 fade by MeshTag, z L2 cut, w no L2 where the MeshTag has KEEP_BIT
    pub mat: Vec4,
}

/// The `MeshTag` bit that keeps a mesh of a gated material (`OccFlags::gated`: the buildings) whole.
pub const KEEP_BIT: u32 = 0x100;

impl Default for OccUniform {
    fn default() -> Self {
        OccUniform {
            view_from_world: Mat4::IDENTITY,
            cam: Vec4::ZERO,
            feet: [Vec4::ZERO; MAX_VOLUMES],
            head: [Vec4::ZERO; MAX_VOLUMES],
            misc: Vec4::ZERO,
            rim: Vec4::ZERO,
            mat: Vec4::ZERO,
        }
    }
}

/// Per-material switches.
#[derive(Clone, Copy, Debug)]
pub struct OccFlags {
    pub keep_up: bool,
    pub fade: bool,
    pub cut: bool,
    /// No L2 on meshes tagged `KEEP_BIT`
    pub gated: bool,
}

impl OccFlags {
    fn vec(self) -> Vec4 {
        Vec4::new(self.keep_up as u8 as f32, self.fade as u8 as f32, self.cut as u8 as f32, self.gated as u8 as f32)
    }
}

#[derive(Clone, Debug)]
pub struct OccSettings {
    /// Capsule radius at full (view-space m): 6 m since P6 (the user, s26: 2.4 m showed the body, not where it is)
    pub radius: f32,
    /// Radius ease time 0 -> full (s; §CAM MUST >= 0.15)
    pub ease_s: f32,
    /// Clear time before the radius eases out (s)
    pub exit_hold_s: f32,
    /// The wide set's push (m): sideways and above the head
    pub wide_m: f32,
    /// Only fragments this much nearer the camera than the body centre cut (m)
    pub margin: f32,
    pub cast_period_s: f32,
    /// L3: the share of a faded object kept, fade time, release hold
    pub fade_keep: f32,
    pub fade_s: f32,
    pub fade_hold_s: f32,
    /// Rim: linear rgb, strength, width (m)
    pub rim: Vec3,
    pub rim_strength: f32,
    pub rim_m: f32,
    /// Controls (see the module doc)
    pub hysteresis: bool,
    pub cut_in_shadows: bool,
    pub no_prepass_cut: bool,
    pub no_glb_rays: bool,
    /// Diagnostics (s28): no per-building gate (every building cuts); the pre-s28 view-depth front test
    pub no_gate: bool,
    pub view_depth: bool,
}

impl Default for OccSettings {
    fn default() -> Self {
        let c = LinearRgba::from(Color::srgb_u8(0x22, 0xE4, 0xFF));
        OccSettings {
            radius: 6.0,
            ease_s: 0.25,
            exit_hold_s: 0.35,
            wide_m: 0.5,
            margin: 1.0,
            cast_period_s: 0.1,
            fade_keep: 0.25,
            fade_s: 0.3,
            fade_hold_s: 0.5,
            rim: Vec3::new(c.red, c.green, c.blue),
            rim_strength: 1.2,
            rim_m: 0.15,
            hysteresis: true,
            cut_in_shadows: false,
            no_prepass_cut: false,
            no_glb_rays: false,
            no_gate: false,
            view_depth: false,
        }
    }
}

impl OccSettings {
    pub fn from_env() -> OccSettings {
        let on = |k: &str| std::env::var(k).is_ok_and(|v| v != "0");
        let f = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let d = OccSettings::default();
        OccSettings {
            // look-round overrides
            margin: f("CV_OCC_MARGIN", d.margin),
            radius: f("CV_OCC_RADIUS", d.radius),
            hysteresis: !on("CV_OCC_NO_HYST"),
            cut_in_shadows: on("CV_OCC_SHADOW_CUT"),
            no_prepass_cut: on("CV_OCC_NO_PREPASS_CUT"),
            no_glb_rays: on("CV_OCC_NO_GLB_RAYS"),
            no_gate: on("CV_OCC_NO_GATE"),
            view_depth: on("CV_OCC_VIEW_DEPTH"),
            ..default()
        }
    }
}

/// A body the camera keeps readable (feet = its GlobalTransform translation; `height` m). Volumes are ordered by
/// `slot`: slot 0 is the player (L1's floor cut follows it), the squad after (Entity order is not spawn order: the
/// allocator reuses freed ids).
#[derive(Component, Clone, Copy)]
pub struct Occluded {
    pub height: f32,
    pub slot: u8,
}

/// The game camera (ortho): the cut runs in its space; any other view (shadow views) stays whole.
#[derive(Component)]
pub struct OccCam;

/// An L3 object (needs a `MeshTag`; its low byte becomes the fade level). The host sets `hit` during a cast.
#[derive(Component, Default)]
pub struct OccFade {
    pub hit: bool,
    pub target: bool,
    pub level: f32,
    pub clear_for: f32,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Volume {
    #[serde(skip)]
    pub entity: Option<Entity>,
    pub feet: Vec3,
    pub head: Vec3,
    /// Hits of the last cast: narrow set, wide set
    pub narrow: usize,
    pub wide: usize,
    pub n_narrow: usize,
    pub occluded: bool,
    pub clear_for: f32,
    pub radius: f32,
    /// Target flips since the start (the flicker metric counts them over a window)
    pub flips: u32,
}

#[derive(Resource)]
pub struct Occlusion {
    /// Off: no cut, no fades (the A/B key; the overview)
    pub enabled: bool,
    pub settings: OccSettings,
    pub vols: Vec<Volume>,
    pub uniform: OccUniform,
    /// Building index -> seconds since a body's ray last hit it (held `exit_hold_s + ease_s + cast_period_s`)
    pub hit_bldgs: HashMap<usize, f32>,
    /// The buildings kept whole: in the scene around a body (the last cast) and not in `hit_bldgs`
    pub keep_bldgs: HashSet<usize>,
    timer: f32,
}

impl Occlusion {
    pub fn new(settings: OccSettings) -> Occlusion {
        Occlusion {
            enabled: true,
            settings,
            vols: vec![],
            uniform: OccUniform::default(),
            hit_bldgs: HashMap::new(),
            keep_bldgs: HashSet::new(),
            timer: 0.0,
        }
    }

    /// The uniform for a material with these flags.
    pub fn uniform(&self, f: OccFlags) -> OccUniform {
        OccUniform { mat: f.vec(), ..self.uniform }
    }
}

/// One cast's rays (world, Bevy frame): toward the camera from sample points; `hit` filled by glclient (glbs) and
/// the host.
pub struct OccRay {
    pub vol: usize,
    pub wide: bool,
    pub origin: Vec3,
    pub dir: Vec3,
    pub hit: bool,
}

#[derive(Resource, Default)]
pub struct OccCast {
    /// This frame casts (every `cast_period_s`)
    pub due: bool,
    pub rays: Vec<OccRay>,
    /// How far a ray goes (m)
    pub max_t: f32,
    /// The buildings this cast's rays hit (any ray, narrow or wide) or a body stands in
    pub bldgs: Vec<usize>,
    /// The buildings holding a context point of an open volume (the scene its capsule shows)
    pub scene: HashSet<usize>,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum OccSet {
    Prepare,
    /// The host's ray tests
    Cast,
    Apply,
}

/// glb buildings' materials with the cut (walkable faces kept); `OccStdBlend` for a blended source material
/// (com01's glass), which keeps its own alpha mode (Mask would turn it into opaque panes).
pub type OccStd = ExtendedMaterial<StandardMaterial, OccExt>;
pub type OccStdBlend = ExtendedMaterial<StandardMaterial, OccExtBlend>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct OccExt {
    #[uniform(101)]
    pub occ: OccUniform,
}

pub const KEEP_UP: OccFlags = OccFlags { keep_up: true, fade: false, cut: true, gated: true };

impl MaterialExtension for OccExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://glclient/occ_std.wgsl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        occ_prepass_shader()
    }
    fn alpha_mode() -> Option<AlphaMode> {
        occ_alpha_mode()
    }
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        occ_specialize(descriptor);
        Ok(())
    }
}

/// `OccExt` for a blended base: the base's alpha mode stands (blended meshes have no depth prepass).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct OccExtBlend {
    #[uniform(101)]
    pub occ: OccUniform,
}

impl MaterialExtension for OccExtBlend {
    fn fragment_shader() -> ShaderRef {
        "embedded://glclient/occ_std.wgsl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        occ_prepass_shader()
    }
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        occ_specialize(descriptor);
        Ok(())
    }
}

/// L4: a flat colour where an occluder (> margin in front) still hides the body.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SilhouetteMaterial {
    #[uniform(0)]
    pub colour: Vec4,
    /// x margin m, y edge boost
    #[uniform(0)]
    pub misc: Vec4,
    /// The visible body's fresnel rim: rgb linear colour, w opacity at a grazing edge (0 = off)
    #[uniform(0)]
    pub rim: Vec4,
}

impl Material for SilhouetteMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://glclient/silhouette.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_write_enabled = Some(false);
            ds.depth_compare = Some(CompareFunction::Always);
        }
        Ok(())
    }
}

/// The silhouette's self-occlusion margin (m, view depth): an occluder nearer than this to the body does not
/// count (the character's own arms in front of its torso); a partition brushed in a doorway (0.3 m) does.
pub const SIL_MARGIN: f32 = 0.25;

/// The visible body's rim opacity at a grazing edge (s30: keeps the player's outline in the dark streets).
pub const RIM_ALPHA: f32 = 1.0;

/// A glb root whose meshes get silhouette twins (the player, the squad).
#[derive(Component)]
pub struct SilhouetteSource;

/// A silhouette twin (hidden with the source, never in a shadow).
#[derive(Component)]
pub struct SilhouetteTwin;

#[derive(Resource)]
pub struct SilhouetteHandle(pub Handle<SilhouetteMaterial>);

pub struct OcclusionPlugin;

impl Plugin for OcclusionPlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "occlusion.wgsl");
        embedded_asset!(app, "occ_std.wgsl");
        embedded_asset!(app, "occ_prepass.wgsl");
        embedded_asset!(app, "silhouette.wgsl");
        app.add_plugins((
            MaterialPlugin::<OccStd>::default(),
            MaterialPlugin::<OccStdBlend>::default(),
            MaterialPlugin::<SilhouetteMaterial>::default(),
        ))
        .init_resource::<OccCast>()
        // after transform propagation: the uniform and the rays use this frame's camera and bodies (the cut's
        // camera test compares positions, so a frame-old camera would switch the cut off while panning)
        .configure_sets(PostUpdate, (OccSet::Prepare, OccSet::Cast, OccSet::Apply).chain().after(TransformSystems::Propagate))
        .add_systems(Startup, setup_silhouette)
        .add_systems(
            PostUpdate,
            (
                occ_prepare.in_set(OccSet::Prepare),
                (occ_apply, occ_std_materials, tag_kept_buildings, floor_cut).chain().in_set(OccSet::Apply),
            ),
        )
        .add_observer(swap_building_materials::<WorldInstanceReady>)
        .add_observer(swap_building_materials::<crate::kit::KitReady>)
        .add_observer(make_silhouettes);
        if !app.world().contains_resource::<Occlusion>() {
            app.insert_resource(Occlusion::new(OccSettings::from_env()));
        }
    }
}

fn setup_silhouette(mut commands: Commands, mut mats: ResMut<Assets<SilhouetteMaterial>>) {
    let c = LinearRgba::from(Color::srgb_u8(0x22, 0xE4, 0xFF));
    // the rim: a cool moonlight edge, faint (CV_NO_RIM=1 turns it off)
    let r = LinearRgba::from(Color::srgb_u8(0xA8, 0xC8, 0xFF));
    let rim_a = if std::env::var("CV_NO_RIM").is_ok() { 0.0 } else { RIM_ALPHA };
    let h = mats.add(SilhouetteMaterial {
        colour: Vec4::new(c.red * 0.6, c.green * 0.6, c.blue * 0.6, 0.55),
        misc: Vec4::new(SIL_MARGIN, 1.5, 0.0, 0.0),
        rim: Vec4::new(r.red * 0.35, r.green * 0.35, r.blue * 0.35, rim_a),
    });
    commands.insert_resource(SilhouetteHandle(h));
}

/// Sample points on a body (Bevy frame; feet on the ground, `right` = the camera's screen right): the narrow set
/// (shins to head, the shoulders) or the wide set (the same pushed `w` out sideways, the top `w` higher).
pub fn sample_points(feet: Vec3, height: f32, right: Vec3, wide: bool, w: f32) -> Vec<Vec3> {
    let k = height / 1.8;
    let side = if wide { 0.25 + w } else { 0.25 };
    let top = if wide { 1.65 * k + w } else { 1.65 * k };
    let up = Vec3::Y;
    let mut v = vec![
        feet + up * (0.2 * k),
        feet + up * (0.6 * k),
        feet + up * (1.0 * k),
        feet + up * (1.35 * k),
        feet + up * top,
        feet + up * (1.35 * k) + right * side,
        feet + up * (1.35 * k) - right * side,
    ];
    if wide {
        v.push(feet + up * (0.6 * k) + right * side);
        v.push(feet + up * (0.6 * k) - right * side);
    }
    v
}

/// The inside test's hysteresis (m): > the bench's 0.15 m sway each way, so entering and leaving need 0.5 m apart
const INSIDE_HYST: f32 = 0.25;

/// Ray vs axis-aligned box (Bevy frame): entry distance if it hits within (0, max_t).
pub fn ray_aabb(o: Vec3, d: Vec3, lo: Vec3, hi: Vec3, max_t: f32) -> Option<f32> {
    let inv = d.recip();
    let t0 = (lo - o) * inv;
    let t1 = (hi - o) * inv;
    let tmin = t0.min(t1).max_element();
    let tmax = t0.max(t1).min_element();
    if tmax >= tmin.max(0.0) && tmin < max_t { Some(tmin.max(0.0)) } else { None }
}

/// Volumes from the `Occluded` entities; on a cast frame, the rays and the glb buildings' hits.
#[allow(clippy::type_complexity)]
pub fn occ_prepare(
    time: Res<Time>,
    mut occ: ResMut<Occlusion>,
    mut cast: ResMut<OccCast>,
    bodies: Query<(Entity, &GlobalTransform, &Occluded, &InheritedVisibility)>,
    cam: Query<&GlobalTransform, With<OccCam>>,
    nodes: Query<(&GlNode, &Children)>,
    prims: Query<(&Aabb, &GlobalTransform, &InheritedVisibility), With<Mesh3d>>,
    city: Option<Res<City>>,
) {
    let mut vols: Vec<Volume> = vec![];
    for (e, t, o, vis) in bodies.iter().sort_by::<&Occluded>(|a, b| a.slot.cmp(&b.slot)) {
        if !vis.get() || vols.len() >= MAX_VOLUMES {
            continue;
        }
        let feet = t.translation();
        let prev = occ.vols.iter().find(|v| v.entity == Some(e)).cloned().unwrap_or_default();
        vols.push(Volume { entity: Some(e), feet, head: feet + Vec3::Y * o.height, ..prev });
    }
    occ.vols = vols;
    occ.timer += time.delta_secs();
    cast.due = false;
    cast.rays.clear();
    cast.bldgs.clear();
    cast.scene.clear();
    let Ok(cam_t) = cam.single() else { return };
    if !occ.enabled || occ.timer < occ.settings.cast_period_s {
        return;
    }
    occ.timer = 0.0;
    cast.due = true;
    cast.max_t = 1500.0;
    let to_cam = -cam_t.forward().as_vec3();
    let right = cam_t.right().as_vec3();
    for (i, v) in occ.vols.iter().enumerate() {
        let h = v.head.y - v.feet.y;
        for wide in [false, true] {
            for p in sample_points(v.feet, h, right, wide, occ.settings.wide_m) {
                cast.rays.push(OccRay { vol: i, wide, origin: p, dir: to_cam, hit: false });
            }
        }
    }
    // glb buildings: the boxes of their visible LOD0 nodes' primitives (bevy_gltf puts the Aabb on a node's
    // primitive children, our GlNode on the node itself), except the floor the player is cut to (it is the room
    // they stand in; floors above are hidden by the cut, floors below are behind the ray)
    let Some(city) = city else { return };
    // a body inside a building (on its nav) is occluded: the cut floor's 1.2 m wall stubs, its rails and flights
    // hide the legs from the outside, and the node boxes cannot see that (they hold the body)
    for (i, v) in occ.vols.iter().enumerate() {
        if let Some((b, _)) = inside(&city, v.feet) {
            for r in cast.rays.iter_mut().filter(|r| r.vol == i) {
                r.hit = true;
            }
            cast.bldgs.push(b);
        }
    }
    let max_t = cast.max_t;
    if occ.settings.no_glb_rays {
        return;
    }
    // the context points (open volumes only): what the capsule shows around the body. On the vertical plane
    // through the feet (screen right x height) and on the ground in front of it (the capsule's lower half). A
    // building holding one is the scene there (a neighbour whose front juts toward the camera hides only itself)
    let rr = occ.settings.radius;
    let fwd = {
        let f = cam_t.forward().as_vec3();
        Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
    };
    let mut ctx: Vec<Vec3> = vec![];
    for v in occ.vols.iter().filter(|v| v.occluded || v.radius > 0.0) {
        for a in [-0.8, -0.4, 0.0, 0.4, 0.8] {
            for h in [0.5, 0.5 * rr, rr] {
                ctx.push(v.feet + right * (a * rr) + Vec3::Y * h);
            }
        }
        for a in [-0.6, 0.0, 0.6] {
            for d in [0.5, 1.0] {
                ctx.push(v.feet + right * (a * rr) - fwd * (d * rr) + Vec3::Y * 0.3);
            }
        }
        ctx.push(v.feet - fwd * (1.4 * rr) + Vec3::Y * 0.3);
    }
    let mut scene: HashSet<usize> = HashSet::new();
    for (n, kids) in nodes.iter() {
        if n.data || n.lod != 0 || city.bldgs[n.bldg].cut.is_some_and(|c| n.floor >= c) {
            continue;
        }
        for (aabb, gt, vis) in prims.iter_many(kids) {
            if !vis.get() {
                continue;
            }
            let (c, he) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            // the boxes are axis-aligned in the building frame; buildings only translate (the glTF axis
            // conversion turns by quarter turns), so the corners' min / max is the world box
            let (lo, hi) = (gt.transform_point(c - he), gt.transform_point(c + he));
            let (lo, hi) = (lo.min(hi), lo.max(hi));
            // every ray (a hit one too): the keep list needs each building a ray passes through
            let mut any = false;
            for r in cast.rays.iter_mut() {
                // a ray starting inside a box (the player in a room of that floor) does not count as a hit, but
                // that building may hide the body (an overhang, a merged shell's box): it is never kept whole.
                // The inside test has the sets' spatial hysteresis too (s28: a kit facade's dressing box reaches
                // 1.5 m into a 2 m alley; a body swaying across its edge flipped): the narrow set (enter) sees the
                // box INSIDE_HYST bigger, the wide set (stay) that much smaller
                // horizontal only: the sway is horizontal, and a box floor near the feet must not turn the wide
                // feet point into a permanent hit (/code-review s28)
                let h = if r.wide { -INSIDE_HYST } else { INSIDE_HYST };
                let m = Vec3::new(h, 0.0, h);
                if r.origin.cmpge(lo - m).all() && r.origin.cmple(hi + m).all() {
                    any = true;
                    continue;
                }
                if ray_aabb(r.origin, r.dir, lo, hi, max_t).is_some() {
                    r.hit = true;
                    any = true;
                }
            }
            if any {
                cast.bldgs.push(n.bldg);
            }
            if !scene.contains(&n.bldg) && ctx.iter().any(|o| o.cmpge(lo).all() && o.cmple(hi).all()) {
                scene.insert(n.bldg);
            }
        }
    }
    cast.scene = scene;
}

/// The state machines (volumes, L3 fades) and the uniform's global part.
pub fn occ_apply(
    time: Res<Time>,
    mut occ: ResMut<Occlusion>,
    cast: Res<OccCast>,
    cam: Query<(&GlobalTransform, &Projection), With<OccCam>>,
    mut fades: Query<(&mut OccFade, &mut MeshTag)>,
) {
    let dt = time.delta_secs();
    let s = occ.settings.clone();
    let enabled = occ.enabled;
    let ortho = cam.single().is_ok_and(|(_, p)| matches!(p, Projection::Orthographic(_)));
    if cast.due {
        let n = occ.vols.len();
        for i in 0..n {
            // narrow = the narrow set's hits; wide = every hit (the wide set covers the narrow one's ground too)
            let nn = cast.rays.iter().filter(|r| r.vol == i && r.hit && !r.wide).count();
            let ww = cast.rays.iter().filter(|r| r.vol == i && r.hit).count();
            let v = &mut occ.vols[i];
            v.narrow = nn;
            v.wide = ww;
            v.n_narrow = cast.rays.iter().filter(|r| r.vol == i && !r.wide).count();
        }
    }
    for v in occ.vols.iter_mut() {
        let was = v.occluded;
        if !enabled || !ortho {
            v.occluded = false;
        } else if v.narrow > 0 {
            v.occluded = true;
            v.clear_for = 0.0;
        } else if v.occluded {
            let clear = if s.hysteresis { v.wide == 0 } else { true };
            if clear {
                v.clear_for += dt;
                if !s.hysteresis || v.clear_for >= s.exit_hold_s {
                    v.occluded = false;
                }
            } else {
                v.clear_for = 0.0;
            }
        }
        if v.occluded != was && enabled {
            v.flips += 1;
        }
        let target = if v.occluded { s.radius } else { 0.0 };
        let step = s.radius / s.ease_s.max(1e-3) * dt;
        v.radius += (target - v.radius).clamp(-step, step);
    }
    // the keep list: a hit building stays cuttable until the capsule could have closed (exit hold + ease out + a
    // cast); a scene building nobody's ray hit stays whole
    let hold = s.exit_hold_s + s.ease_s + s.cast_period_s;
    for age in occ.hit_bldgs.values_mut() {
        *age += dt;
    }
    occ.hit_bldgs.retain(|_, age| *age <= hold);
    if cast.due {
        if enabled && ortho {
            for &b in cast.bldgs.iter() {
                occ.hit_bldgs.insert(b, 0.0);
            }
        }
        let keep: HashSet<usize> = cast.scene.iter().copied().filter(|b| !occ.hit_bldgs.contains_key(b)).collect();
        occ.keep_bldgs = keep;
    }
    for (mut f, mut tag) in fades.iter_mut() {
        if cast.due {
            if f.hit && enabled && ortho {
                f.target = true;
                f.clear_for = 0.0;
            }
            f.hit = false;
        }
        if f.target && (!enabled || !ortho) {
            f.target = false;
        }
        if f.target && enabled {
            f.clear_for += dt;
            // re-armed by every hit (cast every period): released after the hold with no hit
            if f.clear_for >= s.fade_hold_s + s.cast_period_s || !s.hysteresis && f.clear_for > s.cast_period_s * 1.5 {
                f.target = false;
            }
        }
        let goal = if f.target { 1.0 } else { 0.0 };
        let step = dt / s.fade_s.max(1e-3);
        f.level += (goal - f.level).clamp(-step, step);
        let t = (f.level * 255.0).round() as u32;
        if tag.0 & 0xff != t {
            tag.0 = (tag.0 & !0xff) | t;
        }
    }
    // the uniform's global part
    let mut u = OccUniform::default();
    if let Ok((cam_t, _)) = cam.single() {
        u.view_from_world = cam_t.to_matrix().inverse();
        u.cam = cam_t.translation().extend(if enabled && ortho { 1.0 } else { 0.0 });
    }
    for (k, v) in occ.vols.iter().enumerate().take(MAX_VOLUMES) {
        let centre = (v.feet + v.head) * 0.5;
        let vz = (u.view_from_world * centre.extend(1.0)).z;
        u.feet[k] = v.feet.extend(v.radius);
        u.head[k] = (v.head + Vec3::Y * 0.3).extend(vz);
    }
    let flags = (s.cut_in_shadows as u8 as f32) + 2.0 * (s.no_prepass_cut as u8 as f32) + 4.0 * (s.view_depth as u8 as f32);
    u.misc = Vec4::new(s.margin, flags, s.fade_keep, s.rim_strength);
    u.rim = s.rim.extend(s.rim_m);
    occ.uniform = u;
}

/// Copy the uniform into the buildings' materials (they are few: one per source material) when it changed or a
/// material was added (`occ_prepare` writes `Occlusion` every frame, so its change flag says nothing, and a touched
/// material re-uploads its bind group).
pub fn occ_std_materials(
    occ: Res<Occlusion>,
    mut mats: ResMut<Assets<OccStd>>,
    mut blend: ResMut<Assets<OccStdBlend>>,
    mut last: Local<Option<(OccUniform, usize)>>,
) {
    let u = occ.uniform(OccFlags { gated: !occ.settings.no_gate, ..KEEP_UP });
    let key = Some((u, mats.len() + blend.len()));
    if *last == key {
        return;
    }
    *last = key;
    let ids: Vec<_> = mats.ids().collect();
    for id in ids {
        if let Some(mut m) = mats.get_mut(id) {
            m.extension.occ = u;
        }
    }
    let ids: Vec<_> = blend.ids().collect();
    for id in ids {
        if let Some(mut m) = blend.get_mut(id) {
            m.extension.occ = u;
        }
    }
}

/// The keep tags: `KEEP_BIT` on the meshes of the buildings in `Occlusion::keep_bldgs`, cleared elsewhere, on cast
/// frames; a mesh spawned under a kept building's node (a kit re-stamp) is tagged the frame it appears (it would
/// be cut until the next cast: /code-review s28).
#[allow(clippy::type_complexity)]
pub fn tag_kept_buildings(
    mut commands: Commands,
    occ: Res<Occlusion>,
    cast: Res<OccCast>,
    nodes: Query<(&GlNode, &Children)>,
    node_of: Query<&GlNode>,
    added: Query<(Entity, &ChildOf), Added<Mesh3d>>,
    mut tags: Query<(Entity, Option<&mut MeshTag>), With<Mesh3d>>,
    mut prev: Local<HashSet<usize>>,
) {
    if !occ.keep_bldgs.is_empty() {
        for (e, parent) in added.iter() {
            if node_of.get(parent.parent()).is_ok_and(|n| occ.keep_bldgs.contains(&n.bldg)) {
                match tags.get_mut(e) {
                    Ok((_, Some(mut t))) => t.0 |= KEEP_BIT,
                    _ => {
                        commands.entity(e).insert(MeshTag(KEEP_BIT));
                    }
                }
            }
        }
    }
    if !cast.due {
        return;
    }
    for (n, kids) in nodes.iter() {
        let want = occ.keep_bldgs.contains(&n.bldg);
        if !want && !prev.contains(&n.bldg) {
            continue;
        }
        let mut it = tags.iter_many_mut(kids);
        while let Some((e, tag)) = it.fetch_next() {
            match tag {
                Some(mut t) => {
                    let v = if want { t.0 | KEEP_BIT } else { t.0 & !KEEP_BIT };
                    if t.0 != v {
                        t.0 = v;
                    }
                }
                None if want => {
                    commands.entity(e).insert(MeshTag(KEEP_BIT));
                }
                None => {}
            }
        }
    }
    *prev = occ.keep_bldgs.clone();
}

/// A building's StandardMaterials -> OccStd (one per source material): a glb (`WorldInstanceReady`) or a kit
/// building (`kit::KitReady`, P6).
fn swap_building_materials<E: EntityEvent>(
    ev: On<E>,
    mut commands: Commands,
    bldgs: Query<(), With<Bldg>>,
    children: Query<&Children>,
    std_q: Query<&MeshMaterial3d<StandardMaterial>>,
    std_mats: Res<Assets<StandardMaterial>>,
    mut occ_mats: ResMut<Assets<OccStd>>,
    mut blend_mats: ResMut<Assets<OccStdBlend>>,
    // bounded: one entry per source material (a glb's few; a kit block's four)
    mut cache: Local<HashMap<AssetId<StandardMaterial>, Result<Handle<OccStd>, Handle<OccStdBlend>>>>,
) {
    let root = ev.event_target();
    if bldgs.get(root).is_err() || !built() {
        return;
    }
    for e in children.iter_descendants(root) {
        let Ok(m) = std_q.get(e) else { continue };
        let h = cache
            .entry(m.0.id())
            .or_insert_with(|| {
                let base = std_mats.get(&m.0).cloned().unwrap_or_default();
                let occ = OccUniform { mat: KEEP_UP.vec(), ..default() };
                if matches!(base.alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_)) {
                    Ok(occ_mats.add(ExtendedMaterial { base, extension: OccExt { occ } }))
                } else {
                    Err(blend_mats.add(ExtendedMaterial { base, extension: OccExtBlend { occ } }))
                }
            })
            .clone();
        let mut c = commands.entity(e);
        c.remove::<MeshMaterial3d<StandardMaterial>>();
        match h {
            Ok(h) => c.insert(MeshMaterial3d(h)),
            Err(h) => c.insert(MeshMaterial3d(h)),
        };
    }
}

/// Silhouette twins for a `SilhouetteSource` glb.
fn make_silhouettes(
    ev: On<WorldInstanceReady>,
    mut commands: Commands,
    src: Query<(), With<SilhouetteSource>>,
    children: Query<&Children>,
    meshes: Query<(&Mesh3d, Option<&SkinnedMesh>, Option<&bevy::camera::visibility::RenderLayers>)>,
    sil: Res<SilhouetteHandle>,
) {
    if src.get(ev.entity).is_err() {
        return;
    }
    for e in children.iter_descendants(ev.entity).collect::<Vec<_>>() {
        let Ok((m, skin, layers)) = meshes.get(e) else { continue };
        let mut c = commands.spawn((
            Mesh3d(m.0.clone()),
            MeshMaterial3d(sil.0.clone()),
            Transform::default(),
            NotShadowCaster,
            SilhouetteTwin,
            ChildOf(e),
        ));
        if let Some(s) = skin {
            c.insert(s.clone());
        }
        if let Some(l) = layers {
            c.insert(l.clone());
        }
    }
}

/// L1: the first volume inside a pinned building cuts it at its nav level; leaving restores it.
pub fn floor_cut(occ: Res<Occlusion>, city: Option<ResMut<City>>, mut set_by_us: Local<Option<usize>>) {
    let Some(mut city) = city else { return };
    let want = occ.vols.first().filter(|_| occ.enabled).and_then(|v| inside(&city, v.feet));
    let cur = *set_by_us;
    match (cur, want) {
        (Some(b), Some((b2, lvl))) if b == b2 => {
            if city.bldgs[b].cut != Some(lvl) {
                city.bldgs[b].cut = Some(lvl);
                city.cut_gen += 1;
            }
        }
        (cur, want) => {
            if let Some(b) = cur {
                city.bldgs[b].cut = None;
                city.cut_gen += 1;
            }
            *set_by_us = None;
            if let Some((b, lvl)) = want {
                city.bldgs[b].cut = Some(lvl);
                city.cut_gen += 1;
                *set_by_us = Some(b);
            }
        }
    }
}

/// The building and nav level under a world point (Bevy), if it stands on a building poly of a placed block.
pub fn inside(city: &City, p: Vec3) -> Option<(usize, i32)> {
    let mut best: Option<(f32, usize, i32)> = None;
    for b in &city.blocks {
        let local = crate::frame::from_bevy(p - b.origin);
        let q = local.truncate();
        for (i, poly) in b.nav.polys.iter().enumerate() {
            let Some(lot) = poly.lot else { continue };
            if !point_in_poly(q, &poly.verts) {
                continue;
            }
            let dz = (b.nav.z_at(i, q) - local.z).abs();
            if dz < 1.5 && best.is_none_or(|x| dz < x.0) {
                best = Some((dz, b.bldgs[lot], poly.level));
            }
        }
    }
    best.map(|(_, b, l)| (b, l))
}

fn point_in_poly(p: Vec2, v: &[Vec3]) -> bool {
    let n = v.len();
    let mut sign = 0.0f32;
    for k in 0..n {
        let (a, b) = (v[k].truncate(), v[(k + 1) % n].truncate());
        let c = (b - a).perp_dot(p - a);
        if c.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}
