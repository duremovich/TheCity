//! The city fleet (P8 V-B, spec/city/P8_PLAN.md): `pipeline/vehicles/fleet.py` builds one glb per model variant
//! (the vehicle generator: vertex colours, palette materials `pal_base` / `pal_glass` / `pal_glow_<slot>`, pivots
//! `pivot_wheel_*` / `pivot_thruster_*`) + `assets/vehicles/fleet/out/fleet.json`. This module loads every glb
//! once, flattens it into parts (mesh + material + a transform in the car's frame) and hands traffic / tram the
//! parts to stamp: no scene spawn per car, so every car of a variant shares its meshes and materials (Bevy
//! batches them).
//!
//! Frames: the glTF front is -Z (Blender +Y), the right side +X, origin on the ground at the footprint centre.
//! A traffic car's local frame is Bevy with +X forward (traffic.rs: the city heading, `rotation_y(h)`) and +Z
//! its right side, so every part gets `rotation_y(-90 deg)` (-Z -> +X, +X -> +Z).
//!
//! Materials are rebuilt from the palette names, not taken from bevy_gltf: `pal_base` is white (the vertex
//! colours carry the paint), glass keeps its colour, glows keep their hue at a client scale per slot (AgX turns
//! a bright emissive white, s30; the generator's Blender strengths are for Eevee previews).

use bevy::{
    camera::primitives::Aabb,
    gltf::{Gltf, GltfMaterial, GltfMesh, GltfNode},
    prelude::*,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

#[derive(Deserialize, Clone, Debug)]
pub struct ModelRec {
    pub id: String,
    pub model: String,
    pub kind: String,
    pub weight: f32,
    pub glb: String,
    /// (length, width, height) m
    pub size: [f32; 3],
    #[serde(default)]
    pub hover: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct FleetFile {
    pub models: Vec<ModelRec>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Body,
    /// A wheel: spins about its axle (the part's local +Z after the axis fix, the glTF X)
    Wheel {
        front: bool,
    },
    Thruster,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub mesh: Handle<Mesh>,
    pub mat: Handle<StandardMaterial>,
    pub aabb: Aabb,
    /// The part in the car frame (for a wheel: its pivot; the spin goes on top)
    pub tf: Transform,
    pub role: Role,
    /// Wheel radius (m), from the mesh bounds
    pub radius: f32,
    /// The material's palette slot (`base`, `glass`, or the glow slot: `neon_red`, `window_warm`, ...)
    pub slot: String,
}

impl Model {
    /// Stamp the parts under `root` (one entity per part, in `parts` order).
    pub fn spawn_parts(&self, commands: &mut Commands, root: Entity) -> Vec<Entity> {
        self.spawn_parts_with(commands, root, |m| m.clone())
    }

    /// `spawn_parts` with each part's material passed through `mat` (a parked car's lamps switched off).
    pub fn spawn_parts_with(
        &self,
        commands: &mut Commands,
        root: Entity,
        mat: impl Fn(&Handle<StandardMaterial>) -> Handle<StandardMaterial>,
    ) -> Vec<Entity> {
        self.parts
            .iter()
            .map(|p| commands.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(mat(&p.mat)), p.aabb, p.tf, ChildOf(root))).id())
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Model {
    pub rec: ModelRec,
    pub parts: Vec<Part>,
    /// Light sockets in the car frame: the head lamps' (socket_light_0) and the underglow's (socket_light_2)
    pub head: Option<Vec3>,
    pub under: Option<Vec3>,
    /// The underglow's colour (its glow slot's), when the model has one
    pub under_color: Option<Color>,
}

/// The loaded fleet. `ready` once every glb is in; `models` then holds the flattened parts.
#[derive(Resource, Default)]
pub struct Fleet {
    pub ready: bool,
    pub models: Vec<Model>,
    /// Variants that failed `check` (logged as "fleet check")
    pub bad: usize,
    /// The tail lamps' material -> its brake version (brighter), shared by the fleet
    pub brake: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
    /// The head / tail lamps' material -> its switched-off version (a parked car with its lamps off)
    pub dark: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
    pending: Vec<(ModelRec, Handle<Gltf>)>,
}

impl Fleet {
    /// The variants of a kind with their weights.
    pub fn of_kind(&self, kind: &str) -> Vec<(usize, f32)> {
        self.models.iter().enumerate().filter(|(_, m)| m.rec.kind == kind).map(|(i, m)| (i, m.rec.weight)).collect()
    }

    /// A deterministic weighted pick for `key` (a car id) among `kind`'s variants.
    pub fn pick(&self, kind: &str, key: u64) -> Option<usize> {
        let opts = self.of_kind(kind);
        let total: f32 = opts.iter().map(|o| o.1).sum();
        if opts.is_empty() || total <= 0.0 {
            return None;
        }
        // splitmix64 of the key -> [0, 1)
        let mut z = key.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let mut r = (z >> 40) as f32 / (1u64 << 24) as f32 * total;
        for (i, w) in &opts {
            if r < *w {
                return Some(*i);
            }
            r -= w;
        }
        opts.last().map(|o| o.0)
    }
}

/// Client emissive scale per glow slot (the colour is normalised to its brightest channel first).
fn glow_scale(slot: &str) -> f32 {
    match slot {
        "window_warm" => 6.0, // head lamps (traffic.rs boxes: 6.0)
        "neon_red" => 3.0,    // tail lamps
        "cabin_lit" => 0.15,  // lit saloons (0.35, the box bus windows, washed out white on the models)
        "sodium" => 1.0,      // destination boards
        _ => 1.2,             // neon strips, pods, roof signs
    }
}

pub fn load(root: &std::path::Path) -> Option<FleetFile> {
    let p = root.join("vehicles/fleet/out/fleet.json");
    let t = std::fs::read_to_string(&p).ok()?;
    serde_json::from_str(&t).map_err(|e| warn!("{}: {e}", p.display())).ok()
}

/// The hand-built hero sedan's glb (assets/-relative)
pub const HERO_GLB: &str = "vehicles/hover_sedan_02/model.glb";

/// Start loading the fleet (once; does nothing without fleet.json or with CV_BOX_TRAFFIC).
fn start(mut commands: Commands, city: Option<Res<crate::city::City>>, fleet: Option<Res<Fleet>>, assets: Res<AssetServer>) {
    if fleet.is_some() {
        return;
    }
    let Some(city) = city else { return };
    let mut f = Fleet::default();
    if std::env::var("CV_BOX_TRAFFIC").is_ok() {
        info!("fleet: CV_BOX_TRAFFIC, box vehicles");
        f.ready = true;
    } else if let Some(file) = load(&city.root) {
        for mut m in file.models {
            m.glb = format!("vehicles/fleet/out/{}", m.glb);
            let h = assets.load::<Gltf>(m.glb.clone());
            f.pending.push((m, h));
        }
        // the hand-built hero sedan (pipeline/blender/handbuilt/export_sedan.py): not a traffic kind (its disc pods
        // reach 2.9 m, past the lane graph's 1.9 m body), parked next to the player
        if city.root.join(HERO_GLB).exists() {
            let m = ModelRec {
                id: "hero_sedan".into(),
                model: "hover_sedan_02".into(),
                kind: "hero".into(),
                weight: 0.0,
                glb: HERO_GLB.into(),
                size: [0.0; 3],
                hover: 0.45,
            };
            let h = assets.load::<Gltf>(m.glb.clone());
            f.pending.push((m, h));
        }
        info!("fleet: loading {} variants", f.pending.len());
    } else {
        warn!("fleet: no vehicles/fleet/out/fleet.json (run pipeline/vehicles/fleet.py), box vehicles");
        f.ready = true;
    }
    commands.insert_resource(f);
}

/// Flatten the loaded glbs into parts once all are in.
#[allow(clippy::too_many_arguments)]
fn finish(
    fleet: Option<ResMut<Fleet>>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gmeshes: Res<Assets<GltfMesh>>,
    gmats: Res<Assets<GltfMaterial>>,
    meshes: Res<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut fleet) = fleet else { return };
    if fleet.ready {
        return;
    }
    for (rec, h) in &fleet.pending {
        if let Some(bevy::asset::LoadState::Failed(e)) = assets.get_load_state(h) {
            warn!("fleet: {} failed to load: {e}", rec.glb);
        }
    }
    if !fleet.pending.iter().all(|(_, h)| gltfs.contains(h) || matches!(assets.get_load_state(h), Some(bevy::asset::LoadState::Failed(_))))
    {
        return;
    }
    let pending = std::mem::take(&mut fleet.pending);
    // one StandardMaterial per palette material name, shared by the whole fleet
    let mut by_name: HashMap<String, (Handle<StandardMaterial>, String)> = HashMap::new();
    let axis = Transform::from_rotation(Quat::from_rotation_y(-FRAC_PI_2));
    for (rec, h) in pending {
        let Some(g) = gltfs.get(&h) else { continue };
        let mat_names: HashMap<AssetId<GltfMaterial>, String> = g.named_materials.iter().map(|(n, m)| (m.id(), n.to_string())).collect();
        // roots: nodes no other node lists as a child
        let children: std::collections::HashSet<AssetId<GltfNode>> =
            g.nodes.iter().filter_map(|n| nodes.get(n)).flat_map(|n| n.children.iter().map(|c| c.id())).collect();
        let mut parts = vec![];
        let (mut head, mut under) = (None, None);
        let mut stack: Vec<(Handle<GltfNode>, Transform, Role)> =
            g.nodes.iter().filter(|n| !children.contains(&n.id())).map(|n| (n.clone(), axis, Role::Body)).collect();
        while let Some((nh, parent, role)) = stack.pop() {
            let Some(node) = nodes.get(&nh) else { continue };
            let tf = parent * node.transform;
            match node.name.as_str() {
                "socket_light_0" => head = Some(tf.translation),
                "socket_light_2" => under = Some(tf.translation),
                _ => {}
            }
            let role = if node.name.starts_with("pivot_wheel_") {
                Role::Wheel { front: node.name["pivot_wheel_".len()..].starts_with('f') }
            } else if node.name.starts_with("pivot_thruster_") {
                Role::Thruster
            } else {
                role
            };
            if let Some(gm) = node.mesh.as_ref().and_then(|m| gmeshes.get(m)) {
                for p in &gm.primitives {
                    let Some(mesh) = meshes.get(&p.mesh) else { continue };
                    let Some(aabb) = mesh_aabb(mesh) else { continue };
                    let name = p.material.as_ref().and_then(|m| mat_names.get(&m.id())).cloned().unwrap_or_else(|| "pal_base".into());
                    let gmat = p.material.as_ref().and_then(|m| gmats.get(m));
                    let extras = p.material_extras.as_ref().and_then(|e| serde_json::from_str::<serde_json::Value>(&e.value).ok());
                    let (mat, slot) = by_name
                        .entry(name.clone())
                        .or_insert_with(|| {
                            let (m, slot) = if name.starts_with("pal_") || extras.is_none() {
                                material(&name, gmat)
                            } else {
                                recipe(&name, gmat, extras.as_ref().unwrap())
                            };
                            (mats.add(m), slot)
                        })
                        .clone();
                    // a wheel / pod mesh hangs under its pivot: keep the pivot as the part's frame and fold the
                    // mesh node's own (identity in practice) offset into it
                    let radius = aabb.half_extents.y;
                    parts.push(Part { mesh: p.mesh.clone(), mat, aabb, tf, role, radius, slot });
                }
            }
            for c in &node.children {
                stack.push((c.clone(), tf, role));
            }
        }
        // stable order (the node walk is a stack)
        parts.sort_by(|a, b| {
            format!("{:?}{}", a.role, a.slot)
                .cmp(&format!("{:?}{}", b.role, b.slot))
                .then(a.tf.translation.x.total_cmp(&b.tf.translation.x))
                .then(a.tf.translation.z.total_cmp(&b.tf.translation.z))
        });
        if parts.is_empty() {
            warn!("fleet: {} has no meshes", rec.id);
            continue;
        }
        let mut rec = rec;
        if rec.kind == "hero" {
            // no fleet.json size: measured here (the parity check then holds its lamps / ground)
            let (lo, hi) = extent(&parts);
            rec.size = [hi.x - lo.x, hi.z - lo.z, hi.y - lo.y];
            info!("fleet: hero {} {:.2} x {:.2} x {:.2}", rec.id, rec.size[0], rec.size[1], rec.size[2]);
        }
        let problems = if rec.kind == "hero" { vec![] } else { check(&rec, &parts) };
        for e in &problems {
            warn!("fleet check: {}: {e}", rec.id);
        }
        fleet.bad += !problems.is_empty() as usize;
        // the underglow's colour: the glow part nearest the underglow socket under the body
        let under_color = under.and_then(|u| {
            parts
                .iter()
                .filter(|p| p.role == Role::Body && !matches!(p.slot.as_str(), "base" | "glass" | "window_warm" | "neon_red"))
                .min_by(|a, b| {
                    a.tf.transform_point(Vec3::from(a.aabb.center))
                        .distance(u)
                        .total_cmp(&b.tf.transform_point(Vec3::from(b.aabb.center)).distance(u))
                })
                .and_then(|p| mats.get(&p.mat).map(|m| Color::from(m.emissive)))
        });
        fleet.models.push(Model { rec, parts, head, under, under_color });
    }
    // brake lamps: the tail material x 3
    let tails: Vec<Handle<StandardMaterial>> =
        fleet.models.iter().flat_map(|m| m.parts.iter()).filter(|p| p.slot == "neon_red").map(|p| p.mat.clone()).collect();
    for t in tails {
        if fleet.brake.contains_key(&t.id()) {
            continue;
        }
        let Some(m) = mats.get(&t).cloned() else { continue };
        let b = mats.add(StandardMaterial { emissive: m.emissive * 3.0, ..m });
        fleet.brake.insert(t.id(), b);
    }
    // lamps off (parked): head lenses a dull grey, tail lenses a faint red (lit at 4 %)
    let lamps: Vec<(Handle<StandardMaterial>, bool)> = fleet
        .models
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter(|p| matches!(p.slot.as_str(), "window_warm" | "neon_red"))
        .map(|p| (p.mat.clone(), p.slot == "neon_red"))
        .collect();
    for (l, tail) in lamps {
        if fleet.dark.contains_key(&l.id()) {
            continue;
        }
        let Some(m) = mats.get(&l).cloned() else { continue };
        let d = if tail {
            StandardMaterial { emissive: m.emissive * 0.04, ..m }
        } else {
            StandardMaterial { base_color: Color::srgb(0.12, 0.12, 0.13), emissive: LinearRgba::BLACK, ..m }
        };
        fleet.dark.insert(l.id(), mats.add(d));
    }
    fleet.ready = true;
    info!("fleet check: {} ok, {} bad", fleet.models.len() - fleet.bad, fleet.bad);
    let tris: usize = fleet
        .models
        .iter()
        .map(|m| m.parts.iter().filter_map(|p| meshes.get(&p.mesh)).map(|m| m.indices().map_or(0, |i| i.len() / 3)).sum::<usize>())
        .sum();
    info!("fleet: {} variants ready ({} materials, {} tris over all variants)", fleet.models.len(), by_name.len(), tris);
}

/// The parity gate (V-B): in the car frame (+X forward, +Y up, +Z right) the model's extent equals fleet.json's
/// size (the glb as the generator measured it) and sits on the ground, head lamps (`window_warm`) are in front of
/// the middle and tail lamps (`neon_red`) behind it: an axis or handedness slip in the loader fails it.
pub fn check(rec: &ModelRec, parts: &[Part]) -> Vec<String> {
    let mut out = vec![];
    let lamp = |slot: &str| -> Option<f32> {
        let xs: Vec<f32> = parts.iter().filter(|p| p.slot == slot).map(|p| p.tf.transform_point(Vec3::from(p.aabb.center)).x).collect();
        (!xs.is_empty()).then(|| xs.iter().sum::<f32>() / xs.len() as f32)
    };
    let (head, tail) = (lamp("window_warm"), lamp("neon_red"));
    let (lo, hi) = extent(parts);
    let size = hi - lo;
    let [l, w, h] = rec.size;
    if (size.x - l).abs() > 0.02 || (size.z - w).abs() > 0.02 || (size.y - h).abs() > 0.02 {
        out.push(format!("extent {:.2} x {:.2} x {:.2} (along x across) != fleet.json {l:.2} x {w:.2} x {h:.2}", size.x, size.z, size.y));
    }
    if lo.y.abs() > 0.02 {
        out.push(format!("lowest point y {:.3}, not on the ground", lo.y));
    }
    let mid = (lo.x + hi.x) * 0.5;
    if let Some(x) = head {
        if x <= mid {
            out.push(format!("head lamps at x {x:.2} behind the middle {mid:.2}"));
        }
    }
    if let Some(x) = tail {
        if x >= mid {
            out.push(format!("tail lamps at x {x:.2} ahead of the middle {mid:.2}"));
        }
    }
    out
}

/// The parts' extent in the car frame (min, max).
pub fn extent(parts: &[Part]) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in parts {
        let (c, h) = (Vec3::from(p.aabb.center), Vec3::from(p.aabb.half_extents));
        for k in 0..8 {
            let corner = c + h * Vec3::new(
                if k & 1 == 0 { -1.0 } else { 1.0 },
                if k & 2 == 0 { -1.0 } else { 1.0 },
                if k & 4 == 0 { -1.0 } else { 1.0 },
            );
            let w = p.tf.transform_point(corner);
            lo = lo.min(w);
            hi = hi.max(w);
        }
    }
    (lo, hi)
}

/// A hand-built material from its recipe extras (export_sedan.py: gl_kind, gl_tint, gl_rough, gl_metal, gl_glow,
/// gl_emit, gl_alpha) + the glTF material's texture. Two-sided (the export's doubleSided; s27: bevy_gltf).
fn recipe(name: &str, g: Option<&GltfMaterial>, x: &serde_json::Value) -> (StandardMaterial, String) {
    let f = |k: &str, d: f32| x.get(k).and_then(|v| v.as_f64()).map_or(d, |v| v as f32);
    let rgb = |k: &str| {
        x.get(k).and_then(|v| v.as_array()).map(|a| {
            LinearRgba::rgb(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32)
        })
    };
    let kind = x.get("gl_kind").and_then(|v| v.as_str()).unwrap_or("flat");
    let tex = g.and_then(|g| g.base_color_texture.clone());
    let mut m = StandardMaterial {
        perceptual_roughness: f("gl_rough", 0.6),
        metallic: f("gl_metal", 0.0),
        double_sided: true,
        cull_mode: None,
        ..default()
    };
    match kind {
        "emit" => {
            let e = rgb("gl_emit").unwrap_or(LinearRgba::WHITE);
            let mx = e.red.max(e.green).max(e.blue).max(1e-6);
            let k = match name {
                n if n.starts_with("head_") => 6.0,
                "tail_red" => 3.0,
                "vent_cyan" => 0.6,
                _ => 1.2,
            } / mx;
            m.base_color = Color::BLACK;
            m.emissive = LinearRgba::rgb(e.red * k, e.green * k, e.blue * k);
            let slot = if name.starts_with("head_") {
                "window_warm"
            } else if name == "tail_red" {
                "neon_red"
            } else {
                name
            };
            return (m, slot.into());
        }
        "glass" => {
            let t = rgb("gl_tint").unwrap_or(LinearRgba::rgb(0.01, 0.015, 0.02));
            m.base_color = Color::LinearRgba(LinearRgba { alpha: f("gl_alpha", 0.3), ..t });
            m.alpha_mode = AlphaMode::Blend;
            m.reflectance = 0.6;
            return (m, "glass".into());
        }
        "atlas" => m.base_color_texture = tex.clone(),
        "hw" => {
            m.base_color = Color::LinearRgba(rgb("gl_tint").unwrap_or(LinearRgba::WHITE));
            m.base_color_texture = tex.clone();
        }
        _ => m.base_color = Color::LinearRgba(rgb("gl_tint").unwrap_or(LinearRgba::gray(0.2))),
    }
    if let Some(gw) = x.get("gl_glow").and_then(|v| v.as_f64()) {
        // a faint self-light on interiors (the review renders' "glow"), through the same texture
        let t = rgb("gl_tint").unwrap_or(LinearRgba::WHITE);
        m.emissive = LinearRgba::rgb(t.red, t.green, t.blue) * (gw as f32 * 0.4);
        m.emissive_texture = tex;
    }
    (m, "base".into())
}

fn mesh_aabb(m: &Mesh) -> Option<Aabb> {
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(ps)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { return None };
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for p in ps {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
    }
    (lo.x <= hi.x).then(|| Aabb::from_min_max(lo, hi))
}

/// A palette material -> (StandardMaterial, slot).
fn material(name: &str, g: Option<&GltfMaterial>) -> (StandardMaterial, String) {
    if let Some(slot) = name.strip_prefix("pal_glow_") {
        let e = g.map(|g| g.emissive).unwrap_or(LinearRgba::WHITE);
        let m = e.red.max(e.green).max(e.blue).max(1e-6);
        let k = glow_scale(slot) / m;
        let em = LinearRgba::rgb(e.red * k, e.green * k, e.blue * k);
        return (StandardMaterial { base_color: Color::BLACK, emissive: em, perceptual_roughness: 0.5, ..default() }, slot.to_string());
    }
    if name == "pal_glass" {
        // white: the vertex colours carry the glass colour too (bevy_gltf's base colour on top would square it)
        return (StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.12, reflectance: 0.6, ..default() }, "glass".into());
    }
    // pal_base: the vertex colours (COLOR_0, linear) carry the paint
    (StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.6, ..default() }, "base".into())
}

// ------------------------------------------------------------------------------------------------ parked cars

/// Parked cars (P8 V-F): `out/street/parked.json` (pipeline/city/street/parked.py: cars along the lanes' kerbs,
/// `[x, y, z, heading, kind, id]`, city frame), drawn with the fleet's models inside the street ring, landed (a
/// parked hover car rests on the ground), no lights.
#[derive(Resource, Default)]
pub struct ParkedCars {
    cars: Vec<(Vec3, f32, String, String)>,
    live: HashMap<usize, Entity>,
    last: Option<Vec2>,
    pub shown: usize,
    /// The slot the hero sedan takes (the hero slot nearest the focus at startup): the car index it replaces and
    /// the hero's own pose (position, heading)
    pub hero: Option<(usize, Vec3, f32)>,
}

#[derive(Deserialize)]
struct ParkedFile {
    cars: Vec<(f32, f32, f32, f32, String, String)>,
    /// The car slots the hero sedan fits (parked.py hero_slots: its 2.9 m plan on the lane, same ids as the cars)
    #[serde(default)]
    hero: Vec<(f32, f32, f32, f32, String, String)>,
}

#[derive(Component)]
pub struct ParkedCar;

/// The share of parked cars with their lamps lit (idling); the rest are switched off (checkpoint V).
const LIT_SHARE: f32 = 0.1;

fn parked(
    mut commands: Commands,
    street: Option<Res<crate::street::Street>>,
    fleet: Option<Res<Fleet>>,
    focus: Option<Res<crate::frame::Focus>>,
    mut pk: Option<ResMut<ParkedCars>>,
) {
    let (Some(st), Some(fleet)) = (street, fleet) else { return };
    if !fleet.ready || fleet.models.is_empty() {
        return;
    }
    let Some(pk) = pk.as_mut() else {
        let mut p = ParkedCars::default();
        if std::env::var("CV_NO_PARKED").is_err() {
            let path = st.dir.join("parked.json");
            if let Some(f) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str::<ParkedFile>(&t).map_err(|e| warn!("{}: {e}", path.display())).ok())
            {
                p.cars = f.cars.into_iter().map(|(x, y, z, h, k, id)| (Vec3::new(x, y, z), h, k, id)).collect();
                let heroes: Vec<(Vec3, f32, String)> = f.hero.into_iter().map(|(x, y, z, h, _, id)| (Vec3::new(x, y, z), h, id)).collect();
                // the hero sedan: the car slot nearest where the view starts (CV_NO_HERO: none)
                if fleet.models.iter().any(|m| m.rec.kind == "hero") && std::env::var("CV_NO_HERO").is_err() {
                    let f0 = focus.as_ref().map(|f| crate::frame::from_bevy(f.at).truncate()).unwrap_or(st.focus);
                    let best = heroes.iter().min_by(|a, b| a.0.truncate().distance(f0).total_cmp(&b.0.truncate().distance(f0)));
                    p.hero = best.and_then(|(at, h, id)| p.cars.iter().position(|c| &c.3 == id).map(|i| (i, *at, *h)));
                }
                info!("parked: {} cars (hero at {:?})", p.cars.len(), p.hero.map(|h| h.1));
            }
        }
        commands.insert_resource(p);
        return;
    };
    if pk.last == Some(st.focus) {
        return;
    }
    pk.last = Some(st.focus);
    let half = (st.ring as f32 + 0.5) * st.chunk_m;
    let pk = &mut **pk;
    for (i, (p, h, kind, id)) in pk.cars.iter().enumerate() {
        let near = (p.truncate() - st.focus).abs().max_element() < half;
        match (near, pk.live.get(&i)) {
            (true, None) => {
                let key = id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |a, b| (a ^ b as u64).wrapping_mul(0x100_0000_01b3));
                let slot = pk.hero.filter(|h| h.0 == i);
                let hero = slot.and_then(|_| fleet.models.iter().position(|m| m.rec.kind == "hero"));
                let Some(mi) = hero.or_else(|| fleet.pick(kind, key)) else { continue };
                // the hero idles at its hover height (a parked fleet hover car rests on the ground)
                let lift = if hero.is_some() { fleet.models[mi].rec.hover } else { 0.0 };
                let (at, hd) = slot.filter(|_| hero.is_some()).map_or((*p, *h), |s| (s.1, s.2));
                let tf = Transform::from_translation(crate::frame::to_bevy(at) + Vec3::Y * lift).with_rotation(Quat::from_rotation_y(hd));
                let root = commands.spawn((tf, Visibility::Inherited, ParkedCar, Name::new(id.clone()))).id();
                // lamps: off on most parked cars, LIT_SHARE of them idling (the hero always lit)
                if hero.is_none() && (key >> 33) % 1000 >= (LIT_SHARE * 1000.0) as u64 {
                    fleet.models[mi]
                        .spawn_parts_with(&mut commands, root, |m| fleet.dark.get(&m.id()).cloned().unwrap_or_else(|| m.clone()));
                } else {
                    fleet.models[mi].spawn_parts(&mut commands, root);
                }
                pk.live.insert(i, root);
            }
            (false, Some(e)) => {
                commands.entity(*e).despawn();
                pk.live.remove(&i);
            }
            _ => {}
        }
    }
    pk.shown = pk.live.len();
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (start, finish, parked).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(slot: &str, c: Vec3, h: Vec3, tf: Transform) -> Part {
        Part {
            mesh: Handle::default(),
            mat: Handle::default(),
            aabb: Aabb { center: c.into(), half_extents: h.into() },
            tf,
            role: Role::Body,
            radius: 0.0,
            slot: slot.into(),
        }
    }

    fn car(tf: Transform) -> Vec<Part> {
        vec![
            part("base", Vec3::new(0.0, 0.7, 0.0), Vec3::new(2.0, 0.7, 0.9), tf),
            part("window_warm", Vec3::new(1.9, 0.6, 0.0), Vec3::splat(0.05), tf),
            part("neon_red", Vec3::new(-1.9, 0.6, 0.0), Vec3::splat(0.05), tf),
        ]
    }

    #[test]
    fn check_passes_and_controls_fail() {
        let rec = ModelRec {
            id: "t".into(),
            model: "t".into(),
            kind: "car".into(),
            weight: 1.0,
            glb: String::new(),
            size: [4.0, 1.8, 1.4],
            hover: 0.0,
        };
        assert!(check(&rec, &car(Transform::IDENTITY)).is_empty());
        // control: turned round (the head lamps behind)
        assert!(!check(&rec, &car(Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)))).is_empty());
        // control: a quarter turn (the glTF -Z front left unmapped: length across)
        assert!(!check(&rec, &car(Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2)))).is_empty());
        // control: floating
        assert!(!check(&rec, &car(Transform::from_xyz(0.0, 0.3, 0.0))).is_empty());
    }
}
