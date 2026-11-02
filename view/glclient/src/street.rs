//! Street dressing at city scale (P7, spec/city/P7_PLAN.md): `pipeline/city/street` writes `out/street/<cx>_<cy>.json`
//! per 128 m chunk (the city frame: x east, y north, z up): lamps (pole / median / wall / post / deck), crossings
//! (zebra bars with the road's z) and props (stops, bollards, bins, railings, vending, road signs: boxes with a
//! material name).
//!
//! With a `Street` resource the client keeps the chunks within `ring` chunks of the focus loaded: each is read and
//! stamped off the main thread into one mesh per material (metal, the two lamp-head emissives, road paint), so a
//! chunk costs a few entities whatever its lamp count. Lamp LIGHTS are lazier (decision 2, P7_PLAN): a head gets
//! its `SpotLight` + `GlLight` (blk01's lamp: role unshadowed, its shadow priority, the allocator decides shadows)
//! only while it is within `light_m` of the focus (+ `LIGHT_HYST` m before it goes); the heads stay emissive
//! everywhere their chunk is loaded.

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::block::hex;
use crate::city::{GlLight, LM_PER_W, Role};
use crate::frame::{Focus, from_bevy, to_bevy};

#[derive(Deserialize, Clone, Debug)]
pub struct LampRec {
    pub id: String,
    pub kind: String,
    pub foot: [f32; 3],
    pub heads: Vec<[f32; 3]>,
    pub colour: String,
    pub watts: f32,
    pub cone_deg: f32,
    pub blend: f32,
    pub range: f32,
    pub radius: f32,
    pub shadow_priority: f32,
    pub size: f32,
    #[serde(default)]
    pub pole_h: f32,
    /// A bracket's wall side (N / S / E / W); the camera never sees N / W faces: light only, no mesh
    #[serde(default)]
    pub face: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CrossingRec {
    pub id: String,
    pub kind: String,
    /// zebra bars: [x0, y0, x1, y1, z]
    #[serde(default)]
    pub stripes: Vec<[f32; 5]>,
}

/// A prop (P7 C: stops, bollards, bins, railings, vending, name plates, give-way signs, gantries): boxes in the
/// city frame, each [x0, y0, z0, x1, y1, z1, material name] (pipeline/city/street/props.py).
#[derive(Deserialize, Clone, Debug)]
pub struct PropRec {
    pub kind: String,
    pub boxes: Vec<(f32, f32, f32, f32, f32, f32, String)>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct StreetFile {
    #[serde(default)]
    pub lamps: Vec<LampRec>,
    #[serde(default)]
    pub crossings: Vec<CrossingRec>,
    #[serde(default)]
    pub props: Vec<PropRec>,
    /// The tram's static boxes (P7 F: rails, the contact wire, the poles' cross arms): [x0..z1, material]
    #[serde(default)]
    pub tram: Vec<(f32, f32, f32, f32, f32, f32, String)>,
}

#[derive(Deserialize, Clone, Debug)]
struct IndexFile {
    chunk_m: f32,
    chunks: Vec<[i32; 2]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
enum Mat {
    Metal,
    Sodium,
    Cold,
    Paint,
    Rail,
    Bollard,
    Bin,
    Roof,
    Glass,
    Bench,
    Ad,
    SignRed,
    SignGreen,
    SignName,
    VendBody,
    Vend,
    RailSteel,
    Wire,
    Platform,
}

impl Mat {
    /// props.py's material names
    fn of(name: &str) -> Mat {
        match name {
            "rail" => Mat::Rail,
            "bollard" => Mat::Bollard,
            "bin" => Mat::Bin,
            "shelter_roof" => Mat::Roof,
            "glass" => Mat::Glass,
            "bench" => Mat::Bench,
            "emit_ad" => Mat::Ad,
            "sign_red" => Mat::SignRed,
            "sign_green" => Mat::SignGreen,
            "sign_name" => Mat::SignName,
            "vend_body" => Mat::VendBody,
            "emit_vend" => Mat::Vend,
            "rail_steel" => Mat::RailSteel,
            "wire" => Mat::Wire,
            "platform" => Mat::Platform,
            _ => Mat::Metal,
        }
    }
}

type Key = (i32, i32);

struct Built {
    meshes: Vec<(Mat, Mesh, Aabb)>,
    lamps: Arc<Vec<LampRec>>,
}

struct Loaded {
    entities: Vec<Entity>,
    lamps: Arc<Vec<LampRec>>,
}

/// Head lights within this much past `light_m` stay (no flicker at the edge while the focus wobbles).
const LIGHT_HYST: f32 = 16.0;
/// Lamp-head emissive scale (blk01's sodium head: x 2000)
const HEAD_EMIT: f32 = 2000.0;

#[derive(Resource)]
pub struct Street {
    pub dir: PathBuf,
    /// The focus chunk's centre (city frame): the loaded square is it +- (ring + 0.5) chunks (tram.rs / traffic.rs
    /// show their cars in that square only: the rails, the wire and the lamps are there)
    pub focus: Vec2,
    pub chunk_m: f32,
    /// Chunks around the focus chunk kept loaded (2: 5 x 5 chunks, 640 m)
    pub ring: i32,
    pub light_m: f32,
    chunks: HashSet<Key>,
    loaded: HashMap<Key, Loaded>,
    tasks: HashMap<Key, Task<Built>>,
    /// (chunk, lamp index, head index) -> the light entity
    lights: HashMap<(Key, usize, usize), Entity>,
    mats: HashMap<Mat, Handle<StandardMaterial>>,
    last_focus: Option<Vec2>,
}

/// Street stats for the hud / benches.
#[derive(Resource, Default, Debug, Clone, serde::Serialize)]
pub struct StreetStats {
    pub chunks: usize,
    pub lamps: usize,
    pub lights: usize,
    pub entities: usize,
}

impl Street {
    /// The street dir's index, or None when the city has no street pass (`out/street/index.json`).
    pub fn load(dir: PathBuf, ring: i32, light_m: f32) -> Option<Street> {
        let txt = std::fs::read_to_string(dir.join("index.json")).ok()?;
        let idx: IndexFile = serde_json::from_str(&txt).ok()?;
        Some(Street {
            dir,
            focus: Vec2::ZERO,
            chunk_m: idx.chunk_m,
            ring,
            light_m,
            chunks: idx.chunks.iter().map(|c| (c[0], c[1])).collect(),
            loaded: HashMap::new(),
            tasks: HashMap::new(),
            lights: HashMap::new(),
            mats: HashMap::new(),
            last_focus: None,
        })
    }
}

#[derive(Default)]
pub(crate) struct Acc {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    idx: Vec<u32>,
    lo: Vec3,
    hi: Vec3,
}

impl Acc {
    /// An axis-aligned box in the city frame (min, max), emitted in Bevy coordinates.
    pub(crate) fn cuboid(&mut self, a: Vec3, b: Vec3) {
        let (lo, hi) = (a.min(b), a.max(b));
        let c = |x: f32, y: f32, z: f32| to_bevy(Vec3::new(x, y, z));
        // faces: -x +x -y +y -z +z (city frame); each a quad CCW seen from outside
        let faces: [([Vec3; 4], Vec3); 6] = [
            ([c(lo.x, hi.y, lo.z), c(lo.x, lo.y, lo.z), c(lo.x, lo.y, hi.z), c(lo.x, hi.y, hi.z)], Vec3::NEG_X),
            ([c(hi.x, lo.y, lo.z), c(hi.x, hi.y, lo.z), c(hi.x, hi.y, hi.z), c(hi.x, lo.y, hi.z)], Vec3::X),
            ([c(lo.x, lo.y, lo.z), c(hi.x, lo.y, lo.z), c(hi.x, lo.y, hi.z), c(lo.x, lo.y, hi.z)], Vec3::NEG_Y),
            ([c(hi.x, hi.y, lo.z), c(lo.x, hi.y, lo.z), c(lo.x, hi.y, hi.z), c(hi.x, hi.y, hi.z)], Vec3::Y),
            ([c(lo.x, hi.y, lo.z), c(hi.x, hi.y, lo.z), c(hi.x, lo.y, lo.z), c(lo.x, lo.y, lo.z)], Vec3::NEG_Z),
            ([c(lo.x, lo.y, hi.z), c(hi.x, lo.y, hi.z), c(hi.x, hi.y, hi.z), c(lo.x, hi.y, hi.z)], Vec3::Z),
        ];
        for (q, n) in faces {
            let nb = to_bevy(n);
            let base = self.pos.len() as u32;
            for p in q {
                if self.pos.is_empty() {
                    (self.lo, self.hi) = (p, p);
                }
                self.lo = self.lo.min(p);
                self.hi = self.hi.max(p);
                self.pos.push(p.to_array());
                self.nrm.push(nb.to_array());
            }
            self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    /// A box around a segment from `a` to `b` (horizontal or vertical) with a square section `w`.
    pub(crate) fn beam(&mut self, a: Vec3, b: Vec3, w: f32) {
        let h = Vec3::splat(w / 2.0);
        self.cuboid(a.min(b) - h, a.max(b) + h);
    }

    pub(crate) fn mesh(self) -> Option<(Mesh, Aabb)> {
        if self.idx.is_empty() {
            return None;
        }
        let aabb = Aabb::from_min_max(self.lo, self.hi);
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm);
        m.insert_indices(Indices::U32(self.idx));
        Some((m, aabb))
    }
}

fn v(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// A chunk's meshes (blk01's street materials: rust poles, its paint): pole / arm / bracket in metal, the heads in their colour's emissive, the zebra bars in paint.
fn stamp(f: &StreetFile) -> Vec<(Mat, Mesh, Aabb)> {
    let mut acc: HashMap<Mat, Acc> = HashMap::new();
    for l in &f.lamps {
        let foot = v(l.foot);
        let head_mat = if l.colour.eq_ignore_ascii_case("#FFB23E") { Mat::Sodium } else { Mat::Cold };
        match l.kind.as_str() {
            "pole" | "median" | "post" => {
                let top = foot + Vec3::Z * l.pole_h;
                acc.entry(Mat::Metal).or_default().beam(foot, top, l.size);
                for h in &l.heads {
                    let h = v(*h);
                    if l.kind == "post" {
                        // a lantern on the post's top
                        acc.entry(head_mat).or_default().cuboid(h - Vec3::new(0.14, 0.14, 0.2), h + Vec3::new(0.14, 0.14, 0.12));
                    } else {
                        // an arm at the pole's top out to the head, the head hanging under its end
                        let arm_end = Vec3::new(h.x, h.y, top.z - 0.04);
                        acc.entry(Mat::Metal).or_default().beam(Vec3::new(foot.x, foot.y, top.z - 0.04), arm_end, 0.08);
                        acc.entry(head_mat).or_default().cuboid(h - Vec3::new(0.18, 0.18, 0.05), h + Vec3::new(0.18, 0.18, 0.05));
                    }
                }
            }
            "wall" if l.face == "N" || l.face == "W" => {}
            "wall" => {
                for h in &l.heads {
                    let h = v(*h);
                    acc.entry(Mat::Metal).or_default().beam(Vec3::new(foot.x, foot.y, h.z + 0.08), Vec3::new(h.x, h.y, h.z + 0.08), 0.06);
                    acc.entry(head_mat).or_default().cuboid(h - Vec3::new(0.14, 0.14, 0.04), h + Vec3::new(0.14, 0.14, 0.04));
                }
            }
            "deck" => {
                for h in &l.heads {
                    // a 1.2 m fluorescent fitting under the deck
                    let h = v(*h);
                    acc.entry(head_mat).or_default().cuboid(h - Vec3::new(0.6, 0.1, 0.04), h + Vec3::new(0.6, 0.1, 0.04));
                }
            }
            _ => {}
        }
    }
    for p in &f.props {
        for b in &p.boxes {
            acc.entry(Mat::of(&b.6)).or_default().cuboid(Vec3::new(b.0, b.1, b.2), Vec3::new(b.3, b.4, b.5));
        }
    }
    for b in &f.tram {
        acc.entry(Mat::of(&b.6)).or_default().cuboid(Vec3::new(b.0, b.1, b.2), Vec3::new(b.3, b.4, b.5));
    }
    for c in &f.crossings {
        for s in &c.stripes {
            // 4 mm proud of the road (the ground mesh is the road surface)
            acc.entry(Mat::Paint).or_default().cuboid(Vec3::new(s[0], s[1], s[4] + 0.002), Vec3::new(s[2], s[3], s[4] + 0.006));
        }
    }
    let mut keys: Vec<Mat> = acc.keys().copied().collect();
    keys.sort();
    keys.into_iter().filter_map(|k| acc.remove(&k).unwrap().mesh().map(|(m, a)| (k, m, a))).collect()
}

fn build(path: PathBuf) -> Built {
    let f: StreetFile = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).map_err(|e| warn!("{}: {e}", path.display())).ok())
        .unwrap_or_default();
    let meshes = stamp(&f);
    Built { meshes, lamps: Arc::new(f.lamps) }
}

fn materials(mats: &mut Assets<StandardMaterial>) -> HashMap<Mat, Handle<StandardMaterial>> {
    let emit = |c: Color| LinearRgba::from(c) * HEAD_EMIT;
    HashMap::from([
        (Mat::Metal, mats.add(StandardMaterial { base_color: Color::srgb(0.25, 0.12, 0.07), perceptual_roughness: 0.7, ..default() })),
        (Mat::Sodium, mats.add(StandardMaterial { base_color: Color::BLACK, emissive: emit(hex("#FFB23E")), ..default() })),
        (Mat::Cold, mats.add(StandardMaterial { base_color: Color::BLACK, emissive: emit(hex("#E8F4FF")), ..default() })),
        (Mat::Paint, mats.add(StandardMaterial { base_color: Color::srgb(0.42, 0.42, 0.44), perceptual_roughness: 0.7, ..default() })),
        // props: HK street furniture colours (railing grey-green, safety-yellow bollards, green bins, KMB red,
        // the direction signs' green, the street-name blue); the ad panel and the vending front glow
        (Mat::Rail, mats.add(StandardMaterial { base_color: Color::srgb(0.18, 0.24, 0.2), perceptual_roughness: 0.6, ..default() })),
        (Mat::Bollard, mats.add(StandardMaterial { base_color: Color::srgb(0.55, 0.45, 0.08), perceptual_roughness: 0.6, ..default() })),
        (Mat::Bin, mats.add(StandardMaterial { base_color: Color::srgb(0.08, 0.2, 0.11), perceptual_roughness: 0.7, ..default() })),
        (Mat::Roof, mats.add(StandardMaterial { base_color: Color::srgb(0.3, 0.32, 0.34), perceptual_roughness: 0.5, ..default() })),
        (Mat::Glass, mats.add(StandardMaterial { base_color: Color::srgb(0.06, 0.09, 0.1), perceptual_roughness: 0.1, ..default() })),
        (Mat::Bench, mats.add(StandardMaterial { base_color: Color::srgb(0.22, 0.14, 0.09), perceptual_roughness: 0.8, ..default() })),
        (Mat::Ad, mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::from(hex("#E8F4FF")) * 400.0, ..default() })),
        (Mat::SignRed, mats.add(StandardMaterial { base_color: Color::srgb(0.5, 0.06, 0.05), perceptual_roughness: 0.5, ..default() })),
        (Mat::SignGreen, mats.add(StandardMaterial { base_color: Color::srgb(0.04, 0.3, 0.14), perceptual_roughness: 0.5, ..default() })),
        (Mat::SignName, mats.add(StandardMaterial { base_color: Color::srgb(0.06, 0.12, 0.42), perceptual_roughness: 0.5, ..default() })),
        (Mat::VendBody, mats.add(StandardMaterial { base_color: Color::srgb(0.1, 0.11, 0.13), perceptual_roughness: 0.4, ..default() })),
        (
            Mat::Vend,
            mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::from(hex("#22E4FF")) * 600.0, ..default() }),
        ),
        // the tram: polished rail heads, the dark contact wire, the island's concrete
        (
            Mat::RailSteel,
            mats.add(StandardMaterial { base_color: Color::srgb(0.5, 0.5, 0.52), metallic: 0.8, perceptual_roughness: 0.3, ..default() }),
        ),
        (Mat::Wire, mats.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.05), perceptual_roughness: 0.6, ..default() })),
        (Mat::Platform, mats.add(StandardMaterial { base_color: Color::srgb(0.32, 0.31, 0.3), perceptual_roughness: 0.85, ..default() })),
    ])
}

/// Marks the street's mesh entities.
#[derive(Component)]
pub struct StreetMesh;

/// Marks a street lamp's light.
#[derive(Component)]
pub struct StreetLight;

#[allow(clippy::too_many_arguments)]
pub fn update_street(
    mut commands: Commands,
    street: Option<ResMut<Street>>,
    focus: Res<Focus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<StreetStats>,
) {
    let Some(mut st) = street else { return };
    if st.mats.is_empty() {
        st.mats = materials(&mut mats);
    }
    let f = from_bevy(focus.at);
    let fc = (f.x / st.chunk_m).floor() as i32;
    let fr = (f.y / st.chunk_m).floor() as i32;
    st.focus = Vec2::new((fc as f32 + 0.5) * st.chunk_m, (fr as f32 + 0.5) * st.chunk_m);
    let ring = st.ring;
    let want: HashSet<Key> =
        (-ring..=ring).flat_map(|i| (-ring..=ring).map(move |j| (fc + i, fr + j))).filter(|k| st.chunks.contains(k)).collect();
    // unload past the ring + 1 (hysteresis); its lights go with it
    let gone: Vec<Key> = st.loaded.keys().filter(|k| (k.0 - fc).abs() > ring + 1 || (k.1 - fr).abs() > ring + 1).copied().collect();
    for k in gone {
        if let Some(l) = st.loaded.remove(&k) {
            for e in l.entities {
                commands.entity(e).despawn();
            }
        }
        let ls: Vec<_> = st.lights.keys().filter(|x| x.0 == k).copied().collect();
        for x in ls {
            if let Some(e) = st.lights.remove(&x) {
                commands.entity(e).despawn();
            }
        }
    }
    let pool = AsyncComputeTaskPool::get();
    let mut todo: Vec<Key> = want.iter().filter(|k| !st.loaded.contains_key(*k) && !st.tasks.contains_key(*k)).copied().collect();
    todo.sort_by_key(|k| ((k.0 - fc).abs() + (k.1 - fr).abs(), *k));
    for k in todo {
        let path = st.dir.join(format!("{}_{}.json", k.0, k.1));
        st.tasks.insert(k, pool.spawn(async move { build(path) }));
    }
    // finished chunks
    let mut done = vec![];
    for (k, t) in st.tasks.iter_mut() {
        if let Some(b) = block_on(poll_once(t)) {
            done.push((*k, b));
        }
    }
    let mut moved = !done.is_empty();
    for (k, b) in done {
        st.tasks.remove(&k);
        let mut ents = vec![];
        for (mk, m, aabb) in b.meshes {
            ents.push(
                // V4b: lamps and props inside the cut box are cut with the buildings
                commands
                    .spawn((
                        Mesh3d(meshes.add(m)),
                        MeshMaterial3d(st.mats[&mk].clone()),
                        aabb,
                        Transform::IDENTITY,
                        StreetMesh,
                        crate::occlusion::CutProp,
                    ))
                    .id(),
            );
        }
        st.loaded.insert(k, Loaded { entities: ents, lamps: b.lamps });
    }
    // lights: re-checked when the focus moved 4 m or a chunk arrived
    let f2 = Vec2::new(f.x, f.y);
    if st.last_focus.is_none_or(|p| p.distance(f2) > 4.0) {
        moved = true;
    }
    if moved {
        st.last_focus = Some(f2);
        let (on, off) = (st.light_m, st.light_m + LIGHT_HYST);
        let mut want_l: Vec<((Key, usize, usize), LampRec, Vec3)> = vec![];
        let mut keep: HashSet<(Key, usize, usize)> = HashSet::new();
        for (k, l) in &st.loaded {
            for (i, lp) in l.lamps.iter().enumerate() {
                for (j, h) in lp.heads.iter().enumerate() {
                    let d = Vec2::new(h[0], h[1]).distance(f2);
                    let key = (*k, i, j);
                    if d < off && st.lights.contains_key(&key) {
                        keep.insert(key);
                    } else if d < on {
                        want_l.push((key, lp.clone(), v(*h)));
                    }
                }
            }
        }
        let stale: Vec<_> = st.lights.keys().filter(|x| !keep.contains(x)).copied().collect();
        for x in stale {
            if let Some(e) = st.lights.remove(&x) {
                commands.entity(e).despawn();
            }
        }
        for (key, lp, head) in want_l {
            if st.lights.contains_key(&key) {
                continue;
            }
            let outer = (lp.cone_deg / 2.0).to_radians();
            let inner = outer * (1.0 - lp.blend);
            let colour = hex(&lp.colour);
            let lm = lp.watts * LM_PER_W;
            let at = to_bevy(head);
            let e = commands
                .spawn((
                    SpotLight {
                        color: colour,
                        intensity: lm,
                        range: lp.range,
                        radius: lp.radius,
                        outer_angle: outer,
                        inner_angle: inner,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(at).looking_at(at - Vec3::Y, Vec3::Z),
                    GlLight {
                        // not a building's light: the index is only read for interior lights (lighting::lit)
                        bldg: usize::MAX,
                        name: format!("street_{}_{}", lp.id, key.2),
                        role: Role::Unshadowed,
                        export_shadow: true,
                        floor: 0,
                        interior_floor: -1,
                        energy_w: lp.watts,
                        color: colour,
                        intensity: lm,
                        range: lp.range,
                        radius: lp.radius,
                        spot: Some((inner, outer)),
                        priority: lp.shadow_priority,
                        down: true,
                        cone: None,
                    },
                    StreetLight,
                ))
                .id();
            st.lights.insert(key, e);
        }
    }
    stats.chunks = st.loaded.len();
    stats.lamps = st.loaded.values().map(|l| l.lamps.len()).sum();
    stats.lights = st.lights.len();
    stats.entities = st.loaded.values().map(|l| l.entities.len()).sum::<usize>() + st.lights.len();
}

pub fn plugin(app: &mut App) {
    app.init_resource::<StreetStats>().add_systems(Update, update_street.before(crate::lighting::apply_lights));
}
