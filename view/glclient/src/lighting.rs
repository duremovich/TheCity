//! The session-16 light rules over the managed lights (`GlLight`: the glbs' lights + the block files' lamps):
//! which get shadow maps, point vs spot fixtures, the interior rule, volumetric shafts, SSAO, contact shadows,
//! shadow map resolution. `LightSettings::default()` = the chosen rules (DECISIONS "Session 16", GAME_DATA.md
//! "Shadow allocation"): a per-frame allocator of 16 shadow views around the focus, downward fixtures as spots,
//! room fills only on the cut floor, shafts on the shadowed set. The other policies stay for the bench.

use bevy::{
    core_pipeline::prepass::{DepthPrepass, NormalPrepass},
    light::{
        DirectionalLightShadowMap, FogVolume, PointLightShadowMap, VolumetricFog, VolumetricLight,
        cluster::{ClusterConfig, ClusterZConfig},
    },
    pbr::{ContactShadows, ScreenSpaceAmbientOcclusion},
    prelude::*,
};
use std::collections::{HashMap, HashSet};

use crate::city::{City, GlLight, GradeLight, Role};
use crate::frame::{Focus, LightCam};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ShadowPolicy {
    /// gl_shadow from the export (<= 4 per building)
    Export,
    None,
    /// The K most powerful lit lights of each building
    PerBldg(usize),
    /// Every lit light
    All,
    /// A budget of N shadow *views* (spot 1, point 6) for the lights nearest the focus, weighted by priority x
    /// power (the shadow LOD allocator)
    Alloc(usize),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Fixture {
    /// As exported: point lights (cubemap shadow = 6 views)
    Point,
    /// Downward point fixtures become spots (1 view); exported spots stay spots
    Spot,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Interior {
    /// Room fills only on the cut floor (session 15)
    Rule,
    /// All fills on, shadowless (they leak through walls)
    AllOn,
    /// All fills on with shadow maps (no leak, the cost question)
    AllOnShadowed,
}

#[derive(Resource, Clone, PartialEq, Debug)]
pub struct LightSettings {
    pub shadows: ShadowPolicy,
    pub fixture: Fixture,
    pub interior: Interior,
    /// Synthetic shadowless stress lights around the focus (a bench's; the crate does not spawn them)
    pub extras: usize,
    pub point_shadow_res: usize,
    pub spot_shadow_res: usize,
    /// Volumetric fog over the street; shafts from the shadowed set
    pub volumetric: bool,
    /// Shafts only on this many of the shadowed lights (the best-scored; GAME_DATA: 4 mid-range, 16 high-end)
    pub fog_top: usize,
    pub ssao: bool,
    pub contact: bool,
    /// The block files' grade lights (rect lights standing in for off-screen signage)
    pub grade: bool,
    pub z_slices: u32,
}

impl Default for LightSettings {
    /// The chosen rules (session 16).
    fn default() -> Self {
        LightSettings {
            shadows: ShadowPolicy::Alloc(16),
            fixture: Fixture::Spot,
            interior: Interior::Rule,
            extras: 0,
            point_shadow_res: 1024,
            spot_shadow_res: 2048,
            volumetric: true,
            fog_top: 4,
            ssao: false,
            contact: false,
            grade: true,
            z_slices: 24,
        }
    }
}

impl LightSettings {
    /// Session 16's baseline (the export's fixed roles, point fixtures, no fog): the bench's reference.
    pub fn export_baseline() -> Self {
        LightSettings { shadows: ShadowPolicy::Export, fixture: Fixture::Point, volumetric: false, fog_top: usize::MAX, ..default() }
    }
}

/// Hard cap on shadow views before anything is applied (a policy asking for more gets none): 1,500 views
/// allocated ~6 GB of shadow maps and 816 lost the device once (session 16).
#[derive(Resource, Clone, Copy, Debug)]
pub struct ViewCap(pub usize);

impl Default for ViewCap {
    fn default() -> Self {
        ViewCap(600)
    }
}

/// Spot twin of a downward point fixture (a sibling under the same glTF node: same transform, faces -Z = down).
#[derive(Component)]
pub struct Twin(pub Entity);

/// A light on something that moves (P8: car head lamps, underglows): lit by the rules like any GlLight but never a
/// shadow candidate under any policy, and left out of the light dumps (its set depends on frame timing).
#[derive(Component)]
pub struct MovingLight;

#[derive(Component)]
pub struct TwinSpot;

/// What a light is currently set to; `apply_lights` only touches lights whose state changes.
#[derive(Component, Clone, Copy, PartialEq, Debug, Default)]
pub struct Applied {
    pub on: bool,
    pub spot: bool,
    pub shadow: bool,
    pub volumetric: bool,
    pub contact: bool,
}

#[derive(Component)]
pub struct StreetFog;

/// Live counts for an overlay / bench.
#[derive(Resource, Default, Clone, Debug, serde::Serialize)]
pub struct LightStats {
    pub lights_on: usize,
    pub shadowed_point: usize,
    pub shadowed_spot: usize,
    pub shadow_views: usize,
    pub extras: usize,
    /// Shadow views the policy asked for when it was over the `ViewCap` (then no light gets a shadow)
    pub capped_views: usize,
}

/// Fixture spot cone for converted points when the glb has no gl_fixture_cone_deg (export: 34 / 69 deg).
const FIX_INNER: f32 = 0.6;
const FIX_OUTER: f32 = 1.2;

/// Give every downward point light a hidden spot twin once.
pub fn make_twins(
    mut commands: Commands,
    lights: Query<(Entity, &GlLight, Option<&ChildOf>, &Transform), (Added<GlLight>, With<PointLight>)>,
) {
    for (e, l, parent, tf) in lights.iter().filter(|(_, l, ..)| l.down) {
        // glb lights hang under their node (the twin is a sibling, same transform); a free-standing light (a block
        // prop) gets a free-standing twin at its place, facing down. Never a child: apply_lights hides the light
        // while its twin is the active fixture, and a hidden parent would hide the twin too
        let twin = commands
            .spawn((
                SpotLight {
                    color: l.color,
                    intensity: l.intensity,
                    range: l.range,
                    radius: l.radius,
                    inner_angle: l.cone.map_or(FIX_INNER, |c| c.0),
                    outer_angle: l.cone.map_or(FIX_OUTER, |c| c.1),
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::IDENTITY,
                Visibility::Hidden,
                TwinSpot,
            ))
            .id();
        match parent {
            Some(p) => {
                commands.entity(twin).insert(ChildOf(p.parent()));
            }
            None => {
                commands.entity(twin).insert(Transform::from_translation(tf.translation).looking_to(Vec3::NEG_Y, Vec3::Z));
            }
        }
        commands.entity(e).insert(Twin(twin));
    }
}

fn lit(l: &GlLight, s: &LightSettings, city: &City) -> bool {
    if l.role != Role::Interior {
        return true;
    }
    match s.interior {
        Interior::Rule => city.bldgs[l.bldg].cut == Some(l.interior_floor),
        _ => true,
    }
}

/// Set by a host before a comparison shot: the next allocation drops the hysteresis (the shadow set becomes a
/// function of the lights and the focus only, not of the order lights loaded in). Cleared when applied.
#[derive(Resource, Default)]
pub struct AllocReset(pub bool);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn apply_lights(
    mut commands: Commands,
    settings: Res<LightSettings>,
    city: Res<City>,
    focus: Res<Focus>,
    cap: Option<Res<ViewCap>>,
    mut stats: ResMut<LightStats>,
    mut points: Query<
        (Entity, &GlLight, &GlobalTransform, Option<&Twin>, Option<&Applied>, Option<&mut PointLight>, Option<&mut SpotLight>, &mut Visibility),
        Without<TwinSpot>,
    >,
    mut twins: Query<(&mut SpotLight, &mut Visibility), With<TwinSpot>>,
    added: Query<(), Added<Twin>>,
    added_lights: Query<(), Added<GlLight>>,
    moving: Query<(), With<MovingLight>>,
    mut last_gen: Local<u32>,
    mut alloc_set: Local<Vec<Entity>>,
    mut frame: Local<u32>,
    mut reset: ResMut<AllocReset>,
) {
    // the allocator re-scores every 10 frames (and whenever the focus moves)
    *frame = frame.wrapping_add(1);
    let alloc = matches!(settings.shadows, ShadowPolicy::Alloc(_));
    if reset.0 {
        alloc_set.clear();
        reset.0 = false;
    } else if !settings.is_changed()
        && added.is_empty()
        && added_lights.is_empty()
        && *last_gen == city.cut_gen
        && !(alloc && (focus.is_changed() || *frame % 10 == 0))
    {
        return;
    }
    *last_gen = city.cut_gen;
    let max_views = cap.map_or(ViewCap::default().0, |c| c.0);

    // which lights are lit, and the shadow set; a light's shadow cost in views (spot 1, cube 6)
    let mut lit_lights: Vec<(Entity, usize, f32, Vec3, Role)> = Vec::new();
    let mut view_cost: HashMap<Entity, usize> = HashMap::new();
    for (e, l, gt, twin, _, _, sl, _) in points.iter() {
        if lit(l, &settings, &city) && !moving.contains(e) {
            lit_lights.push((e, l.bldg, l.priority * l.energy_w, gt.translation(), l.role));
            let spot = sl.is_some() || (settings.fixture == Fixture::Spot && twin.is_some());
            view_cost.insert(e, if spot { 1 } else { 6 });
        }
    }
    let mut shadowed: HashSet<Entity> = Default::default();
    match settings.shadows {
        ShadowPolicy::Export => {
            for (e, l, ..) in points.iter() {
                if l.export_shadow && lit(l, &settings, &city) {
                    shadowed.insert(e);
                }
            }
        }
        ShadowPolicy::None => {}
        ShadowPolicy::All => shadowed.extend(lit_lights.iter().map(|x| x.0)),
        ShadowPolicy::PerBldg(k) => {
            let mut per: HashMap<usize, Vec<(Entity, f32)>> = HashMap::new();
            for &(e, b, w, _, _) in &lit_lights {
                per.entry(b).or_default().push((e, w));
            }
            for (_, mut v) in per {
                v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                shadowed.extend(v.iter().take(k).map(|x| x.0));
            }
        }
        ShadowPolicy::Alloc(n) => {
            // score = power falling off with distance from the focus (8 m scale); hysteresis keeps the
            // current set unless a newcomer beats it by 25 % (no shadow popping while panning)
            let score = |w: f32, p: Vec3| w / (1.0 + (p - focus.at).length_squared() / 64.0);
            let mut v: Vec<(Entity, f32)> = lit_lights
                .iter()
                .map(|&(e, _, w, p, _)| (e, score(w, p) * if alloc_set.contains(&e) { 1.25 } else { 1.0 }))
                .collect();
            // ties (twin buildings, mirrored lights) break by the light's name (GL_BLD_<building>_...: unique), not by
            // entity id or building index: the shadow set must not depend on load order or on how the host numbered
            // its buildings (P6: kit buildings spawn in another order, and after the glbs, than the baked ones)
            let key: HashMap<Entity, &str> = points.iter().map(|(e, l, ..)| (e, l.name.as_str())).collect();
            v.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| key[&a.0].cmp(&key[&b.0])).then(a.0.cmp(&b.0)));
            // greedy fill of the view budget: a cube (6 views) only if it still fits
            let mut left = n;
            alloc_set.clear();
            for (e, _) in v {
                let c = view_cost[&e];
                if c <= left {
                    left -= c;
                    alloc_set.push(e);
                }
                if left == 0 {
                    break;
                }
            }
            shadowed.extend(alloc_set.iter().copied());
        }
    }
    if settings.interior == Interior::AllOnShadowed {
        shadowed.extend(lit_lights.iter().filter(|x| x.4 == Role::Interior).map(|x| x.0));
    }

    // shafts: the fog_top best-scored of the shadowed set (score as the allocator's, around the focus)
    let fog: HashSet<Entity> = if settings.volumetric {
        let mut v: Vec<(Entity, f32)> = lit_lights
            .iter()
            .filter(|x| shadowed.contains(&x.0))
            .map(|&(e, _, w, p, _)| (e, w / (1.0 + (p - focus.at).length_squared() / 64.0)))
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        v.into_iter().take(settings.fog_top).map(|x| x.0).collect()
    } else {
        HashSet::new()
    };

    let views: usize = shadowed.iter().map(|e| view_cost.get(e).copied().unwrap_or(0)).sum();
    let mut capped_views = 0;
    if views > max_views {
        capped_views = views;
        shadowed.clear();
    }

    let mut st = LightStats::default();
    for (e, l, _, twin, applied, pl, sl, mut vis) in points.iter_mut() {
        let on = lit(l, &settings, &city);
        let is_spot = sl.is_some();
        let spot = is_spot || (settings.fixture == Fixture::Spot && twin.is_some());
        let shadow = on && shadowed.contains(&e);
        let want = Applied { on, spot, shadow, volumetric: shadow && fog.contains(&e), contact: settings.contact };
        if on {
            st.lights_on += 1;
            if shadow {
                if spot {
                    st.shadowed_spot += 1;
                    st.shadow_views += 1;
                } else {
                    st.shadowed_point += 1;
                    st.shadow_views += 6;
                }
            }
        }
        if applied == Some(&want) {
            continue;
        }
        // the active entity: the light itself, or its spot twin
        let active = if spot && !is_spot { twin.map(|t| t.0) } else { Some(e) };
        let self_on = on && active == Some(e);
        vis.set_if_neq(if self_on { Visibility::Inherited } else { Visibility::Hidden });
        if let Some(mut p) = pl {
            p.shadow_maps_enabled = shadow;
            p.contact_shadows_enabled = want.contact;
            p.radius = l.radius;
        }
        if let Some(mut s) = sl {
            s.shadow_maps_enabled = shadow;
            s.contact_shadows_enabled = want.contact;
            s.radius = l.radius;
        }
        if let Some(t) = twin {
            if let Ok((mut s, mut tv)) = twins.get_mut(t.0) {
                let twin_on = on && active == Some(t.0);
                tv.set_if_neq(if twin_on { Visibility::Inherited } else { Visibility::Hidden });
                s.shadow_maps_enabled = shadow;
                s.contact_shadows_enabled = want.contact;
            }
            if want.volumetric {
                commands.entity(t.0).insert(VolumetricLight);
            } else {
                commands.entity(t.0).remove::<VolumetricLight>();
            }
        }
        if want.volumetric {
            commands.entity(e).insert(VolumetricLight);
        } else {
            commands.entity(e).remove::<VolumetricLight>();
        }
        commands.entity(e).insert(want);
    }
    st.extras = stats.extras;
    st.capped_views = capped_views;
    *stats = st;
}

fn env_f32(k: &str, d: f32) -> f32 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// Camera / global features from the settings.
#[allow(clippy::too_many_arguments)]
pub fn apply_features(
    mut commands: Commands,
    settings: Res<LightSettings>,
    city: Res<City>,
    cam: Query<(Entity, Has<crate::occlusion::OccCam>), With<LightCam>>,
    fog: Query<Entity, With<StreetFog>>,
    mut grade: Query<&mut Visibility, With<GradeLight>>,
    mut point_res: ResMut<PointLightShadowMap>,
    mut spot_res: ResMut<DirectionalLightShadowMap>,
) {
    if !settings.is_changed() {
        return;
    }
    let Ok((cam, occ_cam)) = cam.single() else { return };
    {
        let mut c = commands.entity(cam);
        if settings.ssao {
            c.insert(ScreenSpaceAmbientOcclusion::default());
        } else {
            c.remove::<ScreenSpaceAmbientOcclusion>();
        }
        if settings.contact {
            c.insert(ContactShadows::default());
        } else {
            c.remove::<ContactShadows>();
        }
        if !settings.ssao {
            c.remove::<NormalPrepass>();
        }
        // (the occlusion camera keeps its depth prepass: the silhouettes read it)
        if !settings.ssao && !settings.contact && !occ_cam {
            c.remove::<DepthPrepass>();
        }
        if settings.volumetric {
            c.insert(VolumetricFog { ambient_intensity: env_f32("LT_FOG_AMBIENT", 0.0), step_count: 48, ..default() });
        } else {
            c.remove::<VolumetricFog>();
        }
        c.insert(ClusterConfig::FixedZ {
            total: 4096,
            z_slices: settings.z_slices,
            z_config: ClusterZConfig::default(),
            dynamic_resizing: true,
        });
    }
    if settings.volumetric {
        if fog.is_empty() {
            // one volume over the street and the lots' first metres: x over the placed blocks (+ a lot each end)
            let (x0, x1) = city.extent();
            let lot = city.blocks.first().map_or(16.0, |b| b.file.block.lot_w);
            let len = x1 - x0 + 2.0 * lot;
            commands.spawn((
                FogVolume {
                    fog_color: Color::srgb_u8(0x8A, 0x80, 0xA8),
                    density_factor: env_f32("LT_FOG_DENSITY", 0.05),
                    light_intensity: env_f32("LT_FOG_LIGHT", 20.0),
                    ..default()
                },
                Transform::from_xyz((x0 + x1) / 2.0, 10.0, 6.0).with_scale(Vec3::new(len, 20.0, 40.0)),
                StreetFog,
            ));
        }
    } else {
        for e in fog.iter() {
            commands.entity(e).despawn();
        }
    }
    for mut v in grade.iter_mut() {
        v.set_if_neq(if settings.grade { Visibility::Inherited } else { Visibility::Hidden });
    }
    if point_res.size != settings.point_shadow_res {
        point_res.size = settings.point_shadow_res;
    }
    if spot_res.size != settings.spot_shadow_res {
        spot_res.size = settings.spot_shadow_res;
    }
}
