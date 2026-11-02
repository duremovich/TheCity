//! The cutaway (the view milestone V4b, Dylan 2026-11-01; replaces P4's see-through capsule, CITY_SPEC §CAM's
//! L2 / L3 and their ray casts): the rules any host shares.
//!
//! - **The box**: every occluder fragment on the camera's side of the subject (from `behind` m behind its feet to
//!   past the camera), within `width / 2` of the camera-to-subject line on the ground, and higher than the feet +
//!   `above` is discarded, with a screen-door dither band `band` m deep at the box's faces and a faint emissive rim
//!   just under the cut height (a section line). Behind and beside the box everything keeps its full height (the
//!   skyline). The box is built from the camera's position each frame (it follows the yaw): one uniform, one
//!   world-space clip in each occluder material's fragment (`occlusion.wgsl`), no per-building CPU work.
//! - **The subject's own building** (indoors only): the host's rect of it (the sim's building) is cut at the
//!   subject's ceiling (feet + `ceiling`) in the same clip, so the room shows whatever the building is (a kit, a
//!   massing box); and (L1) the placed building whose node boxes hold the feet is cut to the subject's floor through
//!   the buildings' own cutaway (`City::bldgs[b].cut`, GAME_DATA.md: the floors above hidden, that floor's walls as
//!   stubs, its room lights on), looked up every `OWN_PERIOD_S`.
//! - **The cut plane**: a kit building shows its own floor slabs and rooms; a massing box is drawn double-sided and
//!   its inside (only ever seen through the cut) is painted as a floor plate under the subject's floor (the far
//!   walls under that height read as the cap they hide) and as the room's walls above it (`occ_inside`).
//! - Only in the game camera's views: shadow views stay whole (a cut building keeps its shadow), and the prepass
//!   runs the same discard as the main pass (§CAM trap 1: a hole leaves no depth behind).
//! - **Materials**: `OccStd` replaces every StandardMaterial of a building (glb or kit) and of a `CutProp` mesh
//!   (street lamps, signs, props: the host marks them); hosts embed `OccUniform` at binding 101 in their own
//!   occluder materials and copy `Cutaway::uniform` into them after `OccSet::Apply`.
//! - **Silhouettes** (L4): `SilhouetteSource` glbs get a silhouette twin per mesh (needs `DepthPrepass` on the
//!   camera): a body still hidden (a canopy beside the box) shows as a flat outline.
//! - Controls (env, `CutSettings::with_env`): CV_OCC_SHADOW_CUT=1 cuts in shadow views too, CV_OCC_NO_PREPASS_CUT=1
//!   keeps the prepass whole.

use bevy::{
    asset::embedded_asset,
    camera::primitives::Aabb,
    light::NotShadowCaster,
    mesh::{MeshVertexBufferLayoutRef, skinning::SkinnedMesh},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError},
    shader::{ShaderRef, load_shader_library},
    transform::TransformSystems,
    world_serialization::WorldInstanceReady,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::city::{Bldg, City, GlNode};

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
    if !built()
        && let Some(f) = descriptor.fragment.as_mut()
    {
        f.shader_defs.push("OCC_OFF".into());
    }
}

/// The shader's `Occ` (occlusion.wgsl), the same in every occluder material.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct OccUniform {
    /// xyz the game camera's position (Bevy; a view elsewhere is a shadow view: no cut), w 1 = the cut runs
    pub cam: Vec4,
    /// xyz the subject's feet (Bevy), w the cut's height (Bevy y)
    pub subject: Vec4,
    /// xy the ground direction from the subject toward the camera (Bevy x, z), z the box's half width, w how far
    /// behind the feet it starts (m)
    pub axis: Vec4,
    /// x the dither band (m), y flags (1 = cut in every view [control], 2 = no prepass cut [control]), z the rim's
    /// strength, w its height (m)
    pub misc: Vec4,
    /// rgb the rim's hue (linear), w the subject's ceiling (Bevy y: its own building is cut above it)
    pub rim: Vec4,
    /// The subject's own building (indoors): min x, min z, max x, max z (Bevy); empty (max <= min) = none
    pub own: Vec4,
}

impl Default for OccUniform {
    fn default() -> Self {
        OccUniform { cam: Vec4::ZERO, subject: Vec4::ZERO, axis: Vec4::ZERO, misc: Vec4::ZERO, rim: Vec4::ZERO, own: Vec4::ZERO }
    }
}

/// The cutaway's numbers (`[view] cut_*` in TheCity's config; the host fills them).
#[derive(Clone, Debug)]
pub struct CutSettings {
    pub width: f32,
    pub behind: f32,
    pub above: f32,
    pub band: f32,
    pub rim_strength: f32,
    /// The subject's own building is cut this far over its feet (its ceiling)
    pub ceiling: f32,
    /// Zooms (the frame's width at the subject, m) wider than this cut nothing
    pub max_zoom: f32,
    /// The rim's hue (linear rgb) and height (m)
    pub rim: Vec3,
    pub rim_m: f32,
    /// Controls (see the module doc)
    pub cut_in_shadows: bool,
    pub no_prepass_cut: bool,
}

impl Default for CutSettings {
    fn default() -> Self {
        let c = LinearRgba::from(Color::srgb_u8(0x22, 0xE4, 0xFF));
        CutSettings {
            width: 36.0,
            behind: 1.5,
            above: 1.8,
            band: 1.2,
            rim_strength: 0.6,
            ceiling: 2.8,
            max_zoom: 250.0,
            rim: Vec3::new(c.red, c.green, c.blue),
            rim_m: 0.12,
            cut_in_shadows: false,
            no_prepass_cut: false,
        }
    }
}

impl CutSettings {
    pub fn with_env(self) -> CutSettings {
        let on = |k: &str| std::env::var(k).is_ok_and(|v| v != "0");
        CutSettings { cut_in_shadows: on("CV_OCC_SHADOW_CUT"), no_prepass_cut: on("CV_OCC_NO_PREPASS_CUT"), ..self }
    }
}

/// The game camera: the cut runs in its views only (any other view, a shadow view, stays whole).
#[derive(Component)]
pub struct OccCam;

/// A mesh (or a root whose descendant meshes) the cutaway cuts: its StandardMaterial becomes an `OccStd`.
#[derive(Component)]
pub struct CutProp;

#[derive(Resource)]
pub struct Cutaway {
    /// The host's switch (the game view, a subject to follow, the user's key)
    pub enabled: bool,
    pub settings: CutSettings,
    /// The subject's feet (Bevy) and, indoors, its building's rect (Bevy min x, min z, max x, max z); the host sets
    /// both
    pub subject: Option<Vec3>,
    pub indoors: Option<Vec4>,
    /// The game view's zoom (the frame's width at the subject, m)
    pub zoom: f32,
    pub uniform: OccUniform,
    /// The placed building (City index) and floor the subject's own cut holds (L1)
    pub own: Option<(usize, i32)>,
    timer: f32,
}

/// How often the subject's own building is looked up (s).
const OWN_PERIOD_S: f32 = 0.25;

impl Cutaway {
    pub fn new(settings: CutSettings) -> Cutaway {
        Cutaway { enabled: true, settings, subject: None, indoors: None, zoom: 60.0, uniform: OccUniform::default(), own: None, timer: 0.0 }
    }

    /// The cut runs this frame.
    pub fn active(&self) -> bool {
        self.uniform.cam.w > 0.5
    }

    /// A world point (Bevy) inside the box's footprint (on the camera's side, within its width): the host draws
    /// the indoor bodies there.
    pub fn in_box(&self, p: Vec3) -> bool {
        if !self.active() {
            return false;
        }
        let u = &self.uniform;
        let d = Vec2::new(p.x - u.subject.x, p.z - u.subject.z);
        let a = Vec2::new(u.axis.x, u.axis.y);
        d.dot(a) + u.axis.w > 0.0 && d.perp_dot(a).abs() < u.axis.z
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum OccSet {
    /// The uniform, the materials, the own building's cut (PostUpdate, after transform propagation)
    Apply,
}

/// Buildings' and props' materials with the cut; `OccStdBlend` for a blended source material (com01's glass),
/// which keeps its own alpha mode (Mask would turn it into opaque panes).
pub type OccStd = ExtendedMaterial<StandardMaterial, OccExt>;
pub type OccStdBlend = ExtendedMaterial<StandardMaterial, OccExtBlend>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct OccExt {
    #[uniform(101)]
    pub occ: OccUniform,
}

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
        // after transform propagation: the uniform uses this frame's camera (a frame-old camera position would
        // read as a shadow view and switch the cut off while the camera moves)
        .configure_sets(PostUpdate, OccSet::Apply.after(TransformSystems::Propagate))
        .init_resource::<PropMats>()
        .add_systems(Startup, setup_silhouette)
        .add_systems(Update, swap_prop_materials)
        .add_systems(PostUpdate, (cut_apply, occ_std_materials, own_cut).chain().in_set(OccSet::Apply))
        .add_observer(swap_building_materials::<WorldInstanceReady>)
        .add_observer(swap_building_materials::<crate::kit::KitReady>)
        .add_observer(make_silhouettes);
        if !app.world().contains_resource::<Cutaway>() {
            app.insert_resource(Cutaway::new(CutSettings::default().with_env()));
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

/// The uniform from the camera and the subject: the box's axis follows the camera's bearing from the subject.
pub fn cut_apply(mut cut: ResMut<Cutaway>, cam: Query<&GlobalTransform, With<OccCam>>) {
    let s = cut.settings.clone();
    let mut u = OccUniform::default();
    if let (Ok(cam_t), Some(feet)) = (cam.single(), cut.subject) {
        let eye = cam_t.translation();
        let to_cam = Vec2::new(eye.x - feet.x, eye.z - feet.z);
        // straight overhead: the camera's forward on the ground stands in
        let axis = if to_cam.length() > 0.1 {
            to_cam.normalize()
        } else {
            let f = cam_t.forward().as_vec3();
            -Vec2::new(f.x, f.z).normalize_or(Vec2::Y)
        };
        let on = cut.enabled && cut.zoom <= s.max_zoom;
        u.cam = eye.extend(if on { 1.0 } else { 0.0 });
        u.subject = feet.extend(feet.y + s.above);
        u.axis = Vec4::new(axis.x, axis.y, s.width * 0.5, s.behind);
        u.own = cut.indoors.unwrap_or(Vec4::ZERO);
        u.rim.w = feet.y + s.ceiling;
    }
    let flags = (s.cut_in_shadows as u8 as f32) + 2.0 * (s.no_prepass_cut as u8 as f32);
    u.misc = Vec4::new(s.band, flags, s.rim_strength, s.rim_m);
    u.rim = s.rim.extend(u.rim.w);
    if cut.uniform != u {
        cut.uniform = u;
    }
}

/// Copy the uniform into the buildings' and props' materials (they are few: one per source material) when it
/// changed or a material was added (a touched material re-uploads its bind group).
pub fn occ_std_materials(
    cut: Res<Cutaway>,
    mut mats: ResMut<Assets<OccStd>>,
    mut blend: ResMut<Assets<OccStdBlend>>,
    mut last: Local<Option<(OccUniform, usize)>>,
) {
    let u = cut.uniform;
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

/// L1: the subject indoors cuts its own building to its floor (the buildings' own cutaway: the floors above
/// hidden, its walls as stubs, its room lights on); outdoors, or a building that is not placed (massing), none.
/// The building: the one whose LOD0 node boxes hold the feet; the floor: the highest node floor whose box starts
/// at or under the feet (+0.5 m: a storey's slab).
pub fn own_cut(
    time: Res<Time>,
    mut cut: ResMut<Cutaway>,
    city: Option<ResMut<City>>,
    nodes: Query<(&GlNode, &Children)>,
    prims: Query<(&Aabb, &GlobalTransform), With<Mesh3d>>,
) {
    let Some(mut city) = city else { return };
    cut.timer += time.delta_secs();
    if cut.timer < OWN_PERIOD_S {
        return;
    }
    cut.timer = 0.0;
    let want = match cut.subject.filter(|_| cut.indoors.is_some() && cut.active()) {
        None => None,
        Some(feet) => {
            // per building: holds the feet (xz inside a node box, the feet within its height); its floors' bottoms
            let mut floors: HashMap<usize, (bool, i32)> = HashMap::new();
            for (n, kids) in nodes.iter() {
                if n.data || n.lod != 0 {
                    continue;
                }
                for (aabb, gt) in prims.iter_many(kids) {
                    let (c, he) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
                    // buildings only translate and turn by quarter turns: the corners' min / max is the world box
                    let (a, b) = (gt.transform_point(c - he), gt.transform_point(c + he));
                    let (lo, hi) = (a.min(b), a.max(b));
                    if feet.x < lo.x || feet.x > hi.x || feet.z < lo.z || feet.z > hi.z {
                        continue;
                    }
                    let e = floors.entry(n.bldg).or_insert((false, i32::MIN));
                    if feet.y >= lo.y - 0.5 && feet.y <= hi.y + 0.5 {
                        e.0 = true;
                    }
                    if lo.y <= feet.y + 0.5 {
                        e.1 = e.1.max(n.floor);
                    }
                }
            }
            floors.into_iter().filter(|(_, (holds, f))| *holds && *f > i32::MIN).map(|(b, (_, f))| (b, f)).min_by_key(|(b, _)| *b)
        }
    };
    if want == cut.own {
        return;
    }
    if let Some((b, _)) = cut.own
        && b < city.bldgs.len()
    {
        city.bldgs[b].cut = None;
    }
    if let Some((b, f)) = want
        && b < city.bldgs.len()
    {
        city.bldgs[b].cut = Some(f);
    }
    city.cut_gen += 1;
    cut.own = want;
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
    mut cache: ResMut<PropMats>,
) {
    let root = ev.event_target();
    if bldgs.get(root).is_err() || !built() {
        return;
    }
    for e in children.iter_descendants(root) {
        let Ok(m) = std_q.get(e) else { continue };
        let h = cache.get(&m.0, &std_mats, &mut occ_mats, &mut blend_mats);
        swap(&mut commands, e, h);
    }
}

/// One `OccStd` per source StandardMaterial (bounded: a glb's few, a kit block's four, the street's twenty).
#[derive(Resource, Default)]
pub struct PropMats(HashMap<AssetId<StandardMaterial>, Result<Handle<OccStd>, Handle<OccStdBlend>>>);

impl PropMats {
    fn get(
        &mut self,
        m: &Handle<StandardMaterial>,
        std_mats: &Assets<StandardMaterial>,
        occ_mats: &mut Assets<OccStd>,
        blend_mats: &mut Assets<OccStdBlend>,
    ) -> Result<Handle<OccStd>, Handle<OccStdBlend>> {
        self.0
            .entry(m.id())
            .or_insert_with(|| {
                let base = std_mats.get(m).cloned().unwrap_or_default();
                let occ = OccUniform::default();
                if matches!(base.alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_)) {
                    Ok(occ_mats.add(ExtendedMaterial { base, extension: OccExt { occ } }))
                } else {
                    Err(blend_mats.add(ExtendedMaterial { base, extension: OccExtBlend { occ } }))
                }
            })
            .clone()
    }
}

fn swap(commands: &mut Commands, e: Entity, h: Result<Handle<OccStd>, Handle<OccStdBlend>>) {
    let mut c = commands.entity(e);
    c.remove::<MeshMaterial3d<StandardMaterial>>();
    match h {
        Ok(h) => c.insert(MeshMaterial3d(h)),
        Err(h) => c.insert(MeshMaterial3d(h)),
    };
}

/// A `CutProp` mesh's StandardMaterial -> OccStd (the root itself, else its descendants once they are there).
fn swap_prop_materials(
    mut commands: Commands,
    props: Query<Entity, With<CutProp>>,
    added: Query<Entity, Added<CutProp>>,
    children: Query<&Children>,
    std_q: Query<&MeshMaterial3d<StandardMaterial>>,
    std_mats: Res<Assets<StandardMaterial>>,
    mut occ_mats: ResMut<Assets<OccStd>>,
    mut blend_mats: ResMut<Assets<OccStdBlend>>,
    mut cache: ResMut<PropMats>,
) {
    if !built() || props.is_empty() {
        return;
    }
    // a newly marked root and the descendants of every marked root that still wear a StandardMaterial (a glb's
    // children arrive after its root)
    let mut todo: Vec<Entity> = added.iter().collect();
    for r in props.iter() {
        todo.extend(children.iter_descendants(r).filter(|e| std_q.get(*e).is_ok()));
    }
    for e in todo {
        let Ok(m) = std_q.get(e) else { continue };
        let h = cache.get(&m.0, &std_mats, &mut occ_mats, &mut blend_mats);
        swap(&mut commands, e, h);
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
