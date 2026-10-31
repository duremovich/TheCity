//! Placed blocks: the lots' building glbs, the street (sidewalks, road, kerbs, crossings, the far rail), lamps,
//! props and grade lights from each block file; the glTF `gl_*` extras tagged on load; the cutaway.
//!
//! A city is a row of block copies along x (blocks tile edge to edge), optionally with a mirrored row of the
//! same buildings across the road (a bench prop: their backs face the camera, so they sit cut to the ground
//! floor, with no nav). Building i = `City::bldgs[i]`; `GlLight::bldg` / `GlNode::bldg` index it.

use bevy::{
    camera::{Hdr, ScalingMode},
    core_pipeline::tonemapping::Tonemapping,
    gltf::GltfExtras,
    light::RectLight,
    post_process::bloom::Bloom,
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::block::{BlockFile, BlockNav, hex};
use crate::frame::{Focus, MainCam, cam_transform, to_bevy};

/// One building glb (shared by every lot that places it).
pub struct Kind {
    pub id: String,
    /// assets/-relative
    pub glb: String,
    pub lights: String,
    /// Floors incl. the roof
    pub floors: i32,
    /// A kit building (P6, kit.rs): assembled from its kit stream instead of `glb` / `lights`.
    pub kit: Option<crate::kit::KitRef>,
}

/// A placed building.
pub struct Placed {
    pub kind: usize,
    /// Which block copy and lot (None for the mirrored row)
    pub block: usize,
    pub lot: Option<usize>,
    pub transform: Transform,
    pub south: bool,
    /// Cutaway floor (None = normal view)
    pub cut: Option<i32>,
    /// World x range of its lot (Bevy x)
    pub x0: f32,
    pub x1: f32,
}

/// A placed block copy: the file, its navmesh, where its block frame sits in the world.
pub struct PlacedBlock {
    pub file: BlockFile,
    pub nav: BlockNav,
    /// Block frame origin (Bevy); copies only translate along x
    pub origin: Vec3,
    /// City building index of each lot
    pub bldgs: Vec<usize>,
}

impl PlacedBlock {
    /// Block frame point -> Bevy world.
    pub fn world(&self, p: Vec3) -> Vec3 {
        self.origin + to_bevy(p)
    }
}

#[derive(Resource)]
pub struct City {
    /// The assets/ directory (block files, lights.json; the AssetPlugin's root too)
    pub root: PathBuf,
    pub kinds: Vec<Kind>,
    pub bldgs: Vec<Placed>,
    pub blocks: Vec<PlacedBlock>,
    /// Bumped whenever a cut changes so the visibility pass re-runs
    pub cut_gen: u32,
    /// The bench street: the street, the moon / ambient and the camera come with the blocks. False when a host
    /// (engine/cityview) places the blocks in its own world and owns those.
    pub standalone: bool,
}

impl City {
    /// `copies` of the block along x; `south`: the same buildings mirrored across the road, cut to F00.
    pub fn row(root: PathBuf, block: &str, copies: usize, south: bool) -> City {
        let file = BlockFile::load(&root.join(format!("blocks/{block}/block.toml")));
        let mut city = City { root, kinds: vec![], bldgs: vec![], blocks: vec![], cut_gen: 0, standalone: true };
        let len = file.block.length;
        let across = 2.0 * file.block.sidewalk + file.block.road;
        let nav0 = BlockNav::load(&city.root.join(&file.block.nav));
        for k in 0..copies.max(1) {
            let origin = Vec3::new(k as f32 * len, 0.0, 0.0);
            let nav = nav0.clone();
            let mut ids = vec![];
            for (li, lot) in file.lots.iter().enumerate() {
                let kind = city.kind(lot);
                ids.push(city.bldgs.len());
                city.bldgs.push(Placed {
                    kind,
                    block: k,
                    lot: Some(li),
                    transform: Transform::from_translation(origin + Vec3::new(lot.x, 0.0, 0.0)),
                    south: false,
                    cut: None,
                    x0: origin.x + lot.x,
                    x1: origin.x + lot.x + file.block.lot_w,
                });
            }
            city.blocks.push(PlacedBlock { file: file.clone(), nav, origin, bldgs: ids });
        }
        if south {
            let n = file.lots.len();
            for k in 0..copies.max(1) {
                for (li, lot) in file.lots.iter().enumerate() {
                    // the next lot's building, so the two rows do not mirror each other
                    let other = &file.lots[(li + 1) % n];
                    let kind = city.kind(other);
                    let x = k as f32 * len + lot.x;
                    city.bldgs.push(Placed {
                        kind,
                        block: k,
                        lot: None,
                        transform: Transform::from_xyz(x + file.block.lot_w, 0.0, across)
                            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                        south: true,
                        cut: Some(0),
                        x0: x,
                        x1: x + file.block.lot_w,
                    });
                }
            }
        }
        city
    }

    /// One copy of a block with its frame origin at `origin` (Bevy), buildings only: no street, moon or camera
    /// (the host's world has them). A city view's pinned block (overrides.toml [[pin]]).
    pub fn at(root: PathBuf, block: &str, origin: Vec3) -> City {
        let mut city = City::row(root, block, 1, false);
        city.standalone = false;
        let shift = origin - city.blocks[0].origin;
        city.blocks[0].origin = origin;
        for b in city.bldgs.iter_mut() {
            b.transform.translation += shift;
            b.x0 += shift.x;
            b.x1 += shift.x;
        }
        city
    }

    /// A kit building (P6): its stream + its block's atlas (assets-relative); one kind per building.
    pub fn place_kit(&mut self, id: &str, kit: crate::kit::KitRef, floors: i32, transform: Transform, x0: f32, x1: f32) -> usize {
        self.kinds.push(Kind { id: id.to_string(), glb: String::new(), lights: String::new(), floors, kit: Some(kit) });
        self.bldgs.push(Placed { kind: self.kinds.len() - 1, block: 0, lot: None, transform, south: false, cut: None, x0, x1 });
        self.bldgs.len() - 1
    }

    fn kind(&mut self, lot: &crate::block::LotDef) -> usize {
        if let Some(i) = self.kinds.iter().position(|k| k.glb == lot.glb) {
            return i;
        }
        self.kinds.push(Kind { id: lot.bldg.clone(), glb: lot.glb.clone(), lights: lot.lights.clone(), floors: lot.floors, kit: None });
        self.kinds.len() - 1
    }

    /// World x extent of the placed blocks (Bevy).
    pub fn extent(&self) -> (f32, f32) {
        let x0 = self.blocks.iter().map(|b| b.origin.x).fold(f32::MAX, f32::min);
        let x1 = self.blocks.iter().map(|b| b.origin.x + b.file.block.length).fold(f32::MIN, f32::max);
        (x0, x1)
    }

    /// The north-row building whose lot contains world x.
    pub fn north_at(&self, x: f32) -> Option<usize> {
        self.bldgs.iter().position(|b| !b.south && x >= b.x0 && x < b.x1)
    }

    /// Step the cut of the north building at world x: +1 up (None after the roof), -1 down.
    pub fn step_cut(&mut self, x: f32, step: i32) {
        let Some(i) = self.north_at(x) else { return };
        let floors = self.kinds[self.bldgs[i].kind].floors;
        let b = &mut self.bldgs[i];
        b.cut = match (b.cut, step) {
            (None, s) if s < 0 => Some(floors - 1),
            (None, _) => None,
            (Some(c), s) if s > 0 && c + 1 >= floors => None,
            (Some(c), s) => Some((c + s).max(0)),
        };
        self.cut_gen += 1;
    }
}

/// Building root.
#[derive(Component)]
pub struct Bldg {
    pub idx: usize,
}

/// A glTF node with gl_* extras (mesh nodes; light nodes get `GlLight` on their light child instead).
#[derive(Component)]
pub struct GlNode {
    pub bldg: usize,
    pub floor: i32,
    pub cut: CutKind,
    pub lod: i32,
    pub data: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CutKind {
    Main,
    HideOnCut,
    CutOnly,
    None,
}

/// A light the rules manage: a building light from the glb (with the export's data) or a street lamp / prop
/// light from the block file.
#[allow(dead_code)]
#[derive(Component, Clone)]
pub struct GlLight {
    pub bldg: usize,
    pub name: String,
    pub role: Role,
    pub export_shadow: bool,
    pub floor: i32,
    pub interior_floor: i32,
    pub energy_w: f32,
    /// The light as loaded (intensity in lumens, range, colour) and the corrected source radius.
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
    pub radius: f32,
    /// A spot (exported area -> 150 deg spot, or a lamp): inner / outer angle.
    pub spot: Option<(f32, f32)>,
    /// gl_shadow_priority (allocator weight; 1.0 when the glb predates it)
    pub priority: f32,
    /// gl_fixture != "omni": a point the client may render as a downward spot (all points if absent)
    pub down: bool,
    /// gl_fixture_cone_deg (inner, outer) in radians for the downward spot
    pub cone: Option<(f32, f32)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Shadowed,
    Unshadowed,
    Interior,
}

/// The block files' grade lights (rect lights standing in for off-screen signage).
#[derive(Component)]
pub struct GradeLight;

/// Street lamp / prop meshes and lights (not building nodes).
#[derive(Component)]
pub struct StreetThing;

/// Buildings whose glb instance is ready (a bench waits for all of them).
#[derive(Resource, Default)]
pub struct ReadyBuildings(pub usize);

/// lights.json per kind: name -> (energy W, radius).
#[derive(Resource, Default)]
pub struct LightTable(pub Vec<HashMap<String, (f32, f32)>>);

/// Bevy's RectLight reads ~4x hotter than the Blender context's area lights (session 16).
pub const GRADE_SCALE: f32 = 0.25;
/// Blender watts -> as-if-point lumens.
pub const LM_PER_W: f32 = 683.0;

pub fn setup_city(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    city: Res<City>,
    focus: Res<Focus>,
    mut kit_blocks: ResMut<crate::kit::KitBlocks>,
    mut kit_tasks: ResMut<crate::kit::KitTasks>,
    kit_stream: Option<Res<crate::kit::KitStream>>,
) {
    // lights.json for radius + power (the glb has candela, the loader drops the source radius); kit buildings
    // carry their lights in the stream (an empty table keeps the kind indices)
    let mut table = LightTable::default();
    for k in city.kinds.iter() {
        if k.kit.is_some() {
            table.0.push(HashMap::new());
            continue;
        }
        let path = city.root.join(&k.lights);
        let txt = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let v: Value = serde_json::from_str(&txt).unwrap();
        let mut m = HashMap::new();
        for l in v["lights"].as_array().unwrap() {
            m.insert(
                l["name"].as_str().unwrap().to_string(),
                (l["energy_w"].as_f64().unwrap() as f32, l["radius"].as_f64().unwrap_or(0.0) as f32),
            );
        }
        table.0.push(m);
    }
    commands.insert_resource(table);

    for (idx, p) in city.bldgs.iter().enumerate() {
        let k = &city.kinds[p.kind];
        if let Some(kr) = k.kit.as_ref() {
            // a streamed building spawns with its chunk (kit_stream)
            if kit_stream.as_ref().is_some_and(|s| s.manages(idx)) {
                continue;
            }
            crate::kit::spawn_building(
                &mut commands,
                &asset_server,
                &mut materials,
                &mut kit_blocks,
                &mut kit_tasks,
                &city.root,
                idx,
                p.transform,
                format!("bldg{idx}_{}", k.id),
                kr,
            );
            continue;
        }
        commands
            .spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(k.glb.clone()))),
                p.transform,
                Bldg { idx },
                Name::new(format!("bldg{idx}_{}", k.id)),
            ))
            .observe(on_building_ready);
    }

    if !city.standalone {
        return;
    }
    street(&mut commands, &mut meshes, &mut materials, &city);

    // night: a weak blue moon (Blender sun 0.08 W/m2 ~ 55 lux), near-zero world ambient (#0A0A14 x 0.05)
    commands.spawn((
        DirectionalLight { illuminance: 55.0, color: Color::srgb_u8(0x6E, 0x7B, 0xFF), shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(0.0, 30.0, 0.0).looking_at(Vec3::new(-16.0, 0.0, 16.0), Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb_u8(0x0A, 0x0A, 0x14), brightness: 20.0, ..default() });
    commands.insert_resource(ClearColor(Color::srgb(0.005, 0.005, 0.012)));

    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedHorizontal { viewport_width: focus.zoom },
            near: 0.1,
            far: 400.0,
            ..OrthographicProjection::default_3d()
        }),
        cam_transform(&focus),
        Hdr,
        Tonemapping::AgX,
        // the Blender look renders at exposure +0.6 EV (render.py) over Bevy's Blender-default EV100 9.7
        bevy::camera::Exposure { ev100: 9.1 },
        Bloom::NATURAL,
        Msaa::Off,
        MainCam,
        crate::frame::LightCam,
    ));
}

/// The ground, kerbs, crossings, rail, lamps, props, grade lights of every block copy.
fn street(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, city: &City) {
    let mat = |m: &mut Assets<StandardMaterial>, c: Color, rough: f32| {
        m.add(StandardMaterial { base_color: c, perceptual_roughness: rough, ..default() })
    };
    // sidewalks (dry-ish concrete), the road (wet asphalt), curbs, the lots behind
    let sidewalk = mat(materials, Color::srgb(0.16, 0.16, 0.18), 0.55);
    let road = mat(materials, Color::srgb(0.05, 0.05, 0.06), 0.18);
    let lotm = mat(materials, Color::srgb(0.09, 0.09, 0.1), 0.8);
    let curb = mat(materials, Color::srgb(0.22, 0.22, 0.24), 0.6);
    let paint = mat(materials, Color::srgb(0.42, 0.42, 0.44), 0.7);
    let rust = mat(materials, Color::srgb(0.25, 0.12, 0.07), 0.7);
    let south = city.bldgs.iter().any(|b| b.south);
    let Some(first) = city.blocks.first() else { return };
    let hd = &first.file.block;
    let (sw, rd, kerb, lot_w) = (hd.sidewalk, hd.road, hd.kerb, hd.lot_w);
    let (x0, x1) = city.extent();
    let (gx0, gx1) = (x0 - lot_w, x1 + lot_w);
    // kerbs and the sidewalks' kerb strips are broken at the crossings (dropped kerb ramps there)
    let mut gaps: Vec<(f32, f32)> = vec![];
    let mut ramp = 1.0f32;
    for b in &city.blocks {
        for c in &b.file.crossings {
            gaps.push((b.origin.x + c.x0, b.origin.x + c.x1));
            ramp = c.ramp;
        }
    }
    gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut runs = vec![];
    let mut at = gx0;
    for (g0, g1) in &gaps {
        runs.push((at, *g0));
        at = *g1;
    }
    runs.push((at, gx1));
    // bands in Bevy z (= -y): north sidewalk 0..sw, road sw..sw+rd, south sidewalk ..2sw+rd
    let mut slab = |xa: f32, xb: f32, z0: f32, z1: f32, y: f32, m: &Handle<StandardMaterial>| {
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(xb - xa, z1 - z0))),
            MeshMaterial3d(m.clone()),
            Transform::from_xyz((xa + xb) / 2.0, y, (z0 + z1) / 2.0),
        ));
    };
    slab(gx0, gx1, -60.0, 0.0, -0.005, &lotm);
    slab(gx0, gx1, 0.0, sw - ramp, 0.0, &sidewalk);
    slab(gx0, gx1, sw, sw + rd, -kerb, &road);
    slab(gx0, gx1, sw + rd + ramp, 2.0 * sw + rd, 0.0, &sidewalk);
    slab(gx0, gx1, 2.0 * sw + rd, 2.0 * sw + rd + 60.0, -0.005, &lotm);
    for &(a, b) in &runs {
        if b - a > 0.01 {
            slab(a, b, sw - ramp, sw, 0.0, &sidewalk);
            slab(a, b, sw + rd, sw + rd + ramp, 0.0, &sidewalk);
        }
    }
    for z in [sw, sw + rd] {
        for &(a, b) in &runs {
            if b - a > 0.01 {
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(b - a, kerb, 0.2))),
                    MeshMaterial3d(curb.clone()),
                    Transform::from_xyz((a + b) / 2.0, -kerb / 2.0, z),
                ));
            }
        }
    }
    let lamp_head = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::from(Color::srgb_u8(0xFF, 0xB2, 0x3E)) * 2000.0,
        ..default()
    });
    for (bi, b) in city.blocks.iter().enumerate() {
        let f = &b.file;
        // crossings: zebra bars on the road, a ramp each side (a tilted slab from the kerb line to RAMP in)
        for c in &f.crossings {
            let w = c.x1 - c.x0;
            let n = (w / 0.5) as i32;
            for k in (0..n).step_by(2) {
                let x = b.origin.x + c.x0 + (k as f32 + 0.5) * 0.5;
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.5, 0.004, rd - 0.6))),
                    MeshMaterial3d(paint.clone()),
                    Transform::from_xyz(x, -kerb + 0.002, sw + rd / 2.0),
                ));
            }
            let tilt = (kerb / c.ramp).atan();
            for (z_kerb, dir) in [(sw, -1.0f32), (sw + rd, 1.0)] {
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(w, 0.02, (c.ramp * c.ramp + kerb * kerb).sqrt()))),
                    MeshMaterial3d(sidewalk.clone()),
                    Transform::from_xyz(b.origin.x + (c.x0 + c.x1) / 2.0, -kerb / 2.0 - 0.01, z_kerb + dir * c.ramp / 2.0)
                        .with_rotation(Quat::from_rotation_x(-dir * tilt)),
                ));
            }
        }
        // the far edge's low rail (not with a mirrored row standing there)
        if !south {
            let [t, h] = f.block.rail;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(f.block.length, h, t))),
                MeshMaterial3d(rust.clone()),
                Transform::from_translation(b.world(Vec3::new(f.block.length / 2.0, -2.0 * sw - rd + t / 2.0, h / 2.0))),
            ));
        }
        // lamps: pole, arm to the head over the kerb line, the head, a downward spot (a street light the policies
        // treat like the buildings' lights: role unshadowed, export shadow on, its priority)
        for lp in &f.lamps {
            let pole = b.world(Vec3::new(lp.pole[0], lp.pole[1], lp.pole_h / 2.0));
            let head = b.world(Vec3::new(lp.head[0], lp.head[1], lp.head[2]));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(lp.pole_size, lp.pole_h, lp.pole_size))),
                MeshMaterial3d(rust.clone()),
                Transform::from_translation(pole),
                StreetThing,
            ));
            let arm_len = (head.z - pole.z).abs() + 0.1;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.1, 0.08, arm_len))),
                MeshMaterial3d(rust.clone()),
                Transform::from_xyz(head.x, lp.pole_h - 0.04, (head.z + pole.z) / 2.0),
                StreetThing,
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.36, 0.1, 0.24))),
                MeshMaterial3d(lamp_head.clone()),
                Transform::from_xyz(head.x, head.y + 0.07, head.z),
                StreetThing,
            ));
            let outer = (lp.cone_deg / 2.0).to_radians();
            let inner = outer * (1.0 - lp.blend);
            let colour = hex(&lp.colour);
            let lm = lp.watts * LM_PER_W;
            let bldg = city.north_at(head.x).unwrap_or(b.bldgs.first().copied().unwrap_or(0));
            commands.spawn((
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
                Transform::from_translation(head).looking_at(head - Vec3::Y, Vec3::Z),
                GlLight {
                    bldg,
                    name: format!("{}_{}_{bi}", f.block.id, lp.id),
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
                    down: lp.fixture == "down",
                    cone: None,
                },
                StreetThing,
            ));
        }
        // props: a box per kind; a vending machine's lit front + a small shadowless glow that lights passers-by
        for p in &f.props {
            let [rx0, ry0, rx1, ry1] = p.rect;
            let c = b.world(Vec3::new((rx0 + rx1) / 2.0, (ry0 + ry1) / 2.0, p.h / 2.0));
            let body = match p.kind.as_str() {
                "vending" => Color::srgb(0.12, 0.13, 0.16),
                "bin" => Color::srgb(0.10, 0.16, 0.12),
                "bench" => Color::srgb(0.22, 0.16, 0.12),
                _ => Color::srgb(0.2, 0.2, 0.22),
            };
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(rx1 - rx0, p.h, ry1 - ry0))),
                MeshMaterial3d(mat(materials, body, 0.6)),
                Transform::from_translation(c),
                StreetThing,
            ));
            if let Some(e) = &p.emissive {
                // the face toward the road: the props stand against the far rail, so the face at y1 (north)
                let col = hex(e);
                let face = b.world(Vec3::new((rx0 + rx1) / 2.0, ry1 + 0.005, p.h * 0.55));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new((rx1 - rx0) * 0.8, p.h * 0.6, 0.01))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::BLACK,
                        emissive: LinearRgba::from(col) * 60.0,
                        ..default()
                    })),
                    Transform::from_translation(face),
                    StreetThing,
                ));
                let Some(w) = p.glow_w else { continue };
                let lm = w * LM_PER_W;
                let glow = b.world(Vec3::new((rx0 + rx1) / 2.0, ry1 + 0.4, p.h * 0.6));
                commands.spawn((
                    PointLight { color: col, intensity: lm, range: 5.0, radius: 0.3, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(glow),
                    GlLight {
                        bldg: city.north_at(glow.x).unwrap_or(0),
                        name: format!("{}_{}_{bi}", f.block.id, p.id),
                        role: Role::Unshadowed,
                        export_shadow: false,
                        floor: 0,
                        interior_floor: -1,
                        energy_w: w,
                        color: col,
                        intensity: lm,
                        range: 5.0,
                        radius: 0.3,
                        spot: None,
                        priority: 0.2,
                        down: false,
                        cone: None,
                    },
                    StreetThing,
                ));
            }
        }
        // street sign panels: the emissive panel, a rect light toward the street, a wash back onto the wall
        // (Blender area watts as the grade lights: x 683 x GRADE_SCALE)
        for sg in &f.signs {
            let col = hex(&sg.colour);
            let (cx, cz) = ((sg.x[0] + sg.x[1]) / 2.0, (sg.z[0] + sg.z[1]) / 2.0);
            let (w, h) = (sg.x[1] - sg.x[0], sg.z[1] - sg.z[0]);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(w, h, 0.05))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: LinearRgba::from(col) * sg.strength * 4.0,
                    ..default()
                })),
                Transform::from_translation(b.world(Vec3::new(cx, sg.y - 0.025, cz))),
                StreetThing,
            ));
            let size = w.max(h) / 3.0;
            for (watts, from, to) in [
                (sg.light_w, Vec3::new(cx, sg.y - 0.75, cz), Vec3::new(cx, -5.0, 0.5)),
                (sg.wash_w, Vec3::new(cx, sg.y - 0.7, cz), Vec3::new(cx, sg.y + 0.15, cz - 0.6)),
            ] {
                let at = b.world(from);
                commands.spawn((
                    RectLight { color: col, intensity: watts * LM_PER_W * GRADE_SCALE, range: 12.0, width: size, height: size },
                    Transform::from_translation(at).looking_at(b.world(to), Vec3::Y),
                    Visibility::Inherited,
                    StreetThing,
                ));
            }
        }
        // grade lights
        for g in &f.grade {
            let at = b.world(Vec3::from_array(g.at));
            let aim = b.world(Vec3::from_array(g.aim));
            commands.spawn((
                RectLight {
                    color: hex(&g.colour),
                    intensity: g.watts * LM_PER_W * GRADE_SCALE,
                    range: 40.0,
                    width: g.size,
                    height: g.size,
                },
                Transform::from_translation(at).looking_at(aim, Vec3::Y),
                Visibility::Inherited,
                GradeLight,
            ));
        }
    }
}

fn extras(e: &GltfExtras) -> Option<Value> {
    serde_json::from_str(&e.value).ok()
}

/// Tags every gl_* node of a freshly spawned building.
#[allow(clippy::too_many_arguments)]
fn on_building_ready(
    ev: On<WorldInstanceReady>,
    mut commands: Commands,
    bldgs: Query<&Bldg>,
    children: Query<&Children>,
    q: Query<(Option<&GltfExtras>, Option<&Name>, Option<&PointLight>, Option<&SpotLight>)>,
    city: Res<City>,
    table: Res<LightTable>,
    mut ready: ResMut<ReadyBuildings>,
) {
    let Ok(b) = bldgs.get(ev.entity) else { return };
    ready.0 += 1;
    let kind = city.bldgs[b.idx].kind;
    let mut n_lights = 0;
    for e in children.iter_descendants(ev.entity) {
        let Ok((Some(ex), name, _, _)) = q.get(e) else { continue };
        let Some(v) = extras(ex) else { continue };
        let floor = v["gl_floor"].as_i64().unwrap_or(0) as i32;
        if let Some(role) = v["gl_light_role"].as_str() {
            // the light is a child of the node (bevy_gltf); node extras stay on the node
            let name = name.map(|n| n.as_str().to_string()).unwrap_or_default();
            // radius from lights.json (the loader drops it); power = the exported point-equivalent watts
            let (table_w, radius) = table.0[kind].get(&name).copied().unwrap_or((0.0, 0.0));
            let energy_w = v["gl_energy_w"].as_f64().map(|w| w as f32).unwrap_or(table_w);
            let cone = v["gl_fixture_cone_deg"].as_array().and_then(|a| Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
            for c in children.iter_descendants(e) {
                let Ok((_, _, pl, sl)) = q.get(c) else { continue };
                let (color, intensity, range, spot) = if let Some(p) = pl {
                    (p.color, p.intensity, p.range, None)
                } else if let Some(s) = sl {
                    (s.color, s.intensity, s.range, Some((s.inner_angle, s.outer_angle)))
                } else {
                    continue;
                };
                commands.entity(c).insert(GlLight {
                    bldg: b.idx,
                    name: name.clone(),
                    role: match role {
                        "shadowed" => Role::Shadowed,
                        "interior" => Role::Interior,
                        _ => Role::Unshadowed,
                    },
                    export_shadow: v["gl_shadow"].as_bool().unwrap_or(false),
                    floor,
                    interior_floor: v["gl_interior_floor"].as_i64().unwrap_or(-1) as i32,
                    energy_w,
                    color,
                    intensity,
                    range,
                    // bevy_gltf 0.19.1 sets a spot's radius to its range (loader/mod.rs): use lights.json
                    radius,
                    spot,
                    priority: v["gl_shadow_priority"].as_f64().unwrap_or(1.0) as f32,
                    down: v["gl_fixture"].as_str().is_none_or(|f| f != "omni"),
                    cone: cone.map(|(i, o)| (i.to_radians(), o.to_radians())),
                });
                n_lights += 1;
            }
            continue;
        }
        let cut = match v["gl_cut"].as_str() {
            Some("main") => CutKind::Main,
            Some("hide_on_cut") => CutKind::HideOnCut,
            Some("cut_only") => CutKind::CutOnly,
            _ => CutKind::None,
        };
        commands.entity(e).insert(GlNode {
            bldg: b.idx,
            floor,
            cut,
            lod: v["gl_lod"].as_i64().unwrap_or(0) as i32,
            data: v["gl_role"].is_string(),
        });
    }
    info!("building {} ({}) ready: {} lights", b.idx, city.kinds[kind].id, n_lights);
}

/// The cutaway rule (GAME_DATA.md): normal view = main + hide_on_cut; cut at C: floors < C normal, C = main +
/// cut_only, floors > C hidden. LOD1 / LOD2 and the nav / collision data meshes never render here.
pub fn apply_cutaway(
    city: Res<City>,
    mut nodes: Query<(&GlNode, &mut Visibility)>,
    added: Query<(), Added<GlNode>>,
    mut last_gen: Local<u32>,
) {
    if added.is_empty() && *last_gen == city.cut_gen {
        return;
    }
    *last_gen = city.cut_gen;
    for (n, mut vis) in nodes.iter_mut() {
        let show = if n.data || n.lod != 0 {
            false
        } else {
            match city.bldgs[n.bldg].cut {
                None => n.cut != CutKind::CutOnly,
                Some(c) if n.floor < c => n.cut != CutKind::CutOnly,
                Some(c) if n.floor == c => n.cut != CutKind::HideOnCut,
                Some(_) => false,
            }
        };
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// Keyboard: arrows pan, Z zoom, PageUp / PageDown cut the building under the focus.
pub fn camera_controls(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut focus: ResMut<Focus>, mut city: ResMut<City>) {
    let mut d = Vec3::ZERO;
    if keys.pressed(KeyCode::ArrowLeft) {
        d.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        d.x += 1.0;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        d.z -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        d.z += 1.0;
    }
    if d != Vec3::ZERO {
        focus.at += d * 12.0 * time.delta_secs();
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        focus.zoom = if focus.zoom < 30.0 {
            60.0
        } else if focus.zoom < 100.0 {
            140.0
        } else {
            26.0
        };
    }
    if keys.just_pressed(KeyCode::PageUp) {
        let x = focus.at.x;
        city.step_cut(x, 1);
    }
    if keys.just_pressed(KeyCode::PageDown) {
        let x = focus.at.x;
        city.step_cut(x, -1);
    }
}
