//! The view milestone V3: people drawn as people. Every body `sim.rs` places (`Sim::people`) is one of
//! TheCityGraphics' street NPCs (`characters/base_body/street/round_06/glb`, 20 baked looks, ~3k tris, one atlas,
//! the Mixamo rig) instead of a capsule.
//!
//! - **The look** is keyed to the body's identity, never to a name: a child gets a street kid (scaled by age, `[view]
//!   child_scale_min` .. `child_full_age`: there is no child mesh), a gang member (colours) a heavy or a guard, else the
//!   dress (0 rags, 1 street, 2-3 smart, the fashion pass's `Appearance.dress` from class and wealth) picks a pool;
//!   within the pool and the body's sex, a stable pick by entity id (`look_of`).
//! - **The clips**: `npc_clips.glb` (TheCityGraphics `pipeline/blender/handbuilt/npc_clips.py`) holds the Mixamo
//!   `walk` and `idle` on the baked rig (same node names and rest rotations as every NPC glb). The NPC glbs carry no
//!   animation, so the loader puts no animation targets on them: `rig_on_ready` adds the `AnimationPlayer` on the
//!   `base_body_rig` node and an `AnimationTargetId` (the name path from it) on every node below.
//! - **LOD** (`[view] people_anim_m`, `people_anim_max`): the nearest bodies to the subject are the skinned glb,
//!   walking when they move and idling when they stand; past that, one static mesh per look and pose (the idle's
//!   first frame, the walk's mid-stride), CPU-skinned once at startup from a posed template (`bake_poses`) and drawn
//!   as a plain mesh, so Bevy batches every body of a look into one instanced draw. No body casts a shadow; the atlas
//!   glows a little (`[view] people_glow`), or the night street loses the crowd.
//! - **Poses** (Motion V2; Dylan 2026-11-04: procedural poses in code until real clips): `pose_bones` lays a `Posing`
//!   over the clip on each rig's bones, between the animation and the transform propagation: sitting on the floor
//!   (a sitting Use, jacked in: the hips at `[view] sit_hips_m`, legs out, hands on the thighs), the fight's guard,
//!   a strike and a stagger; lying (asleep, knocked out, downed) is the whole body laid flat. The static LOD bakes the
//!   sit beside the idle and the walk (`bake_poses`).
//! - **The subject** is the occlusion's player stand-in (occ.rs: its cut and silhouette follow that entity); it
//!   takes the subject's look (`dress_standin`) and its walk / idle, and `Sim::people` leaves it out.
//! - The glbs face +z (south at heading 0): drawn turned half round. Which bodies are drawn at all (the indoor rule:
//!   only in the open cut) is sim.rs's.
//! - `npc_clips.glb` is generated (not in TheCityGraphics' git): missing, the people never get ready and nothing is
//!   drawn ("people: loading" stays on the caption); the command is in TheCityGraphics `pipeline/DECISIONS.md`, V3.
//!
//! `--capsules` draws the old capsules (sim.rs) instead: a frame-time A/B on one build.

use bevy::animation::{AnimatedBy, AnimationTargetId};
use bevy::gltf::Gltf;
use bevy::light::NotShadowCaster;
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use std::collections::HashMap;

use crate::mesh::bv;
use crate::occ::Body;
use crate::sim::Sim;

const DIR: &str = "characters/base_body/street/round_06/glb";
const CLIPS: &str = "characters/base_body/street/round_06/glb/npc_clips.glb";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kit {
    Kid,
    Drone,
    Courier,
    Medic,
    Heavy,
    Runner,
    Vendor,
    Guard,
}

/// The round's NPCs: file stem, female, archetype (round_06/specs.json).
const NPCS: [(&str, bool, Kit); 20] = [
    ("npc01_street_kid", false, Kit::Kid),
    ("npc02_office_drone", true, Kit::Drone),
    ("npc03_courier", true, Kit::Courier),
    ("npc04_street_medic", true, Kit::Medic),
    ("npc05_gang_heavy", false, Kit::Heavy),
    ("npc06_netrunner", true, Kit::Runner),
    ("npc07_vendor", false, Kit::Vendor),
    ("npc08_franchise_guard", true, Kit::Guard),
    ("npc09_street_kid", false, Kit::Kid),
    ("npc10_office_drone", false, Kit::Drone),
    ("npc11_courier", true, Kit::Courier),
    ("npc12_street_medic", false, Kit::Medic),
    ("npc13_gang_heavy", false, Kit::Heavy),
    ("npc14_netrunner", false, Kit::Runner),
    ("npc15_vendor", true, Kit::Vendor),
    ("npc16_franchise_guard", true, Kit::Guard),
    ("npc17_street_kid", false, Kit::Kid),
    ("npc18_office_drone", false, Kit::Drone),
    ("npc19_courier", false, Kit::Courier),
    ("npc20_street_medic", false, Kit::Medic),
];

/// Poses of the static LOD: 0 the idle's first frame, 1 the walk mid-stride, 2 (Motion V2) sitting on the floor (the
/// idle's first frame with `pose_bones`' sit); the clip each is seeked in and the seek time (s).
const POSE_SEEK: [f32; 3] = [0.0, 0.27, 0.0];
const POSE_CLIP: [usize; 3] = [IDLE, WALK, IDLE];
/// The sim's child (under 18: no Brain, CLAUDE.md's hard rule) wears a street kid's look
const ADULT_AGE: u32 = 18;
const IDLE: usize = 0;
const WALK: usize = 1;
const SIT: usize = 2;

/// Motion V2: the rig's bones the procedural poses move (`pose_bones`), and each one's parent among them (`None`: the
/// rig node, `base_body_rig`). Mixamo's: a bone's +y runs along it; the character faces +z, its left is +x.
const BONES: [&str; 15] = [
    "mixamorig:Hips",
    "mixamorig:Spine",
    "mixamorig:Spine1",
    "mixamorig:Spine2",
    "mixamorig:LeftShoulder",
    "mixamorig:LeftArm",
    "mixamorig:LeftForeArm",
    "mixamorig:RightShoulder",
    "mixamorig:RightArm",
    "mixamorig:RightForeArm",
    "mixamorig:LeftUpLeg",
    "mixamorig:LeftLeg",
    "mixamorig:RightUpLeg",
    "mixamorig:RightLeg",
    "mixamorig:Neck",
];
const BONE_PARENT: [Option<usize>; 15] =
    [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), Some(3), Some(7), Some(8), Some(0), Some(10), Some(0), Some(12), Some(3)];
const HIPS: usize = 0;
const SPINE: usize = 1;
const L_ARM: usize = 5;
const L_FORE: usize = 6;
const R_ARM: usize = 8;
const R_FORE: usize = 9;
const L_THIGH: usize = 10;
const L_SHIN: usize = 11;
const R_THIGH: usize = 12;
const R_SHIN: usize = 13;

/// Motion V2: on a rigged NPC (its `base_body_rig` node): the bones `pose_bones` moves (`BONES`, found by name).
#[derive(Component)]
pub struct Bones([Option<Entity>; 15]);

/// Motion V2 (Dylan 2026-11-04: procedural poses in code until real clips): the pose laid over the clip playing,
/// each a weight 0..1: sitting on the floor (`[view] sit_hips_m`), the fight's guard (fists up), a strike (the right
/// arm out, the spine leaning in by `lunge_deg`) and a stagger (the spine back, the arms thrown out). Lying is the
/// whole body's turn (`BodyDraw::tilt`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Posing {
    pub sit: f32,
    pub guard: f32,
    pub strike: f32,
    pub stagger: f32,
    pub lean: f32,
}

impl Posing {
    fn any(&self) -> bool {
        self.sit + self.guard + self.strike + self.stagger > 1e-3
    }
}

/// One body to draw this frame (sim.rs fills `Sim::people`).
#[derive(Clone, Debug, Default)]
pub struct BodyDraw {
    pub id: u64,
    /// Feet, city metres (x east, y north, z up)
    pub at: Vec3,
    /// Bevy yaw about +y (the capsules' convention: heading 0 = north = -z)
    pub yaw: f32,
    pub walking: bool,
    /// Motion V1 (motion.rs): the walk clip's rate (the drawn ground speed over the clip's stride; 1 idling)
    pub clip_rate: f32,
    pub female: bool,
    pub age: u32,
    pub dress: u8,
    pub gang: bool,
    /// C5: the replay's procedural pose on the rig: a lean (rad, forward +), a step (city metres), lying flat (knocked
    /// out, downed)
    pub lean: f32,
    pub nudge: Vec2,
    pub lying: bool,
    /// Motion V2: the procedural pose on the rig's bones (`Posing`: sitting, the fight's guard, a strike, a stagger)
    pub pose: Posing,
}

impl BodyDraw {
    /// C5: the pose's turn on the body's own axes (after its yaw): a lean about its right-hand axis, or flat on its back.
    /// Motion V2: a static (unrigged) body's; a rigged one leans in its spine (`rig_tilt`).
    pub fn tilt(&self) -> Quat {
        if self.lying { Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) } else { Quat::from_rotation_x(self.lean) }
    }

    /// Motion V2: a rigged body's whole turn: lying flat only (a lunge's lean is the spine's, `pose_bones`).
    pub fn rig_tilt(&self) -> Quat {
        if self.lying { Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY }
    }

    /// Motion V2: the bones' pose (`Posing`), a lying body's none (its whole turn lays it down as it stands).
    pub fn posing(&self) -> Posing {
        if self.lying { Posing::default() } else { Posing { lean: self.lean, ..self.pose } }
    }

    /// C5: the feet with the pose's step (and a lying body raised off the floor).
    pub fn feet(&self) -> Vec3 {
        self.at + self.nudge.extend(if self.lying { 0.15 } else { 0.0 })
    }
}

/// `[view]`'s V3 numbers (assets/config.toml, read at startup).
#[derive(Clone, Debug)]
pub struct PeopleCfg {
    pub anim_m: f32,
    pub anim_max: usize,
    pub child_scale_min: f32,
    pub child_full_age: u32,
    pub glow: f32,
    /// Motion V2: a body sitting on the floor has its hips this high (m)
    pub sit_hips_m: f32,
}

impl PeopleCfg {
    pub fn from_view(v: &citysim::config::ViewCfg) -> PeopleCfg {
        PeopleCfg {
            anim_m: v.people_anim_m,
            anim_max: usize::from(v.people_anim_max),
            child_scale_min: v.child_scale_min,
            child_full_age: v.child_full_age.max(1),
            glow: v.people_glow,
            sit_hips_m: v.sit_hips_m,
        }
    }
}

struct Near {
    e: Entity,
    /// The clip it plays (None until rigged and started)
    playing: Option<usize>,
}

#[derive(Resource)]
pub struct People {
    pub cfg: PeopleCfg,
    gltfs: Vec<Handle<Gltf>>,
    clips: Handle<Gltf>,
    graph: Option<(Handle<AnimationGraph>, [AnimationNodeIndex; 2])>,
    /// Per look and pose: the posed static mesh; per look: its material
    posed: Vec<[Option<Handle<Mesh>>; 3]>,
    mats: Vec<Option<Handle<StandardMaterial>>>,
    /// The glb's atlas material -> its glowing copy (the animated bodies swap theirs on rigging)
    glow: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
    templates: bool,
    pub ready: bool,
    far: HashMap<u64, (Entity, usize, usize)>,
    near: HashMap<u64, Near>,
    /// The stand-in's look once dressed
    standin: Option<usize>,
    pub stats: String,
}

impl People {
    pub fn new(cfg: PeopleCfg) -> People {
        People {
            cfg,
            gltfs: vec![],
            clips: Handle::default(),
            graph: None,
            posed: vec![[None, None, None]; NPCS.len()],
            mats: vec![None; NPCS.len()],
            glow: HashMap::new(),
            templates: false,
            ready: false,
            far: HashMap::new(),
            near: HashMap::new(),
            standin: None,
            stats: "people: loading".into(),
        }
    }

    /// The film's trace (film.rs): how body `id` is drawn now, its LOD (`skinned`, `static`, `none`) and its clip
    /// (`walk` / `idle` playing on the rig, `unrigged` before it is, the static mesh's `walk_pose` / `idle_pose`).
    pub fn drawn_state(&self, id: u64) -> (&'static str, &'static str) {
        if let Some(n) = self.near.get(&id) {
            return (
                "skinned",
                match n.playing {
                    Some(WALK) => "walk",
                    Some(_) => "idle",
                    None => "unrigged",
                },
            );
        }
        match self.far.get(&id) {
            Some((_, _, WALK)) => ("static", "walk_pose"),
            Some((_, _, SIT)) => ("static", "sit_pose"),
            Some(_) => ("static", "idle_pose"),
            None => ("none", "none"),
        }
    }

    fn scale(&self, age: u32) -> f32 {
        let c = &self.cfg;
        if age >= c.child_full_age { 1.0 } else { c.child_scale_min + (1.0 - c.child_scale_min) * age as f32 / c.child_full_age as f32 }
    }
}

fn mix(id: u64) -> u64 {
    let mut z = id.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The look (an index into `NPCS`) of a body: its pool by age, gang and dress, then its sex, then a stable pick by id.
fn look_of(b: &BodyDraw) -> usize {
    let (pool, fallback): (&[Kit], &[Kit]) = if b.age < ADULT_AGE {
        (&[Kit::Kid], &[Kit::Vendor, Kit::Runner])
    } else if b.gang {
        (&[Kit::Heavy, Kit::Guard], &[Kit::Courier])
    } else {
        match b.dress {
            0 => (&[Kit::Kid, Kit::Vendor], &[Kit::Medic]),
            1 => (&[Kit::Courier, Kit::Vendor, Kit::Medic, Kit::Runner], &[]),
            _ => (&[Kit::Drone, Kit::Runner], &[]),
        }
    };
    let of = |kits: &[Kit]| -> Vec<usize> { (0..NPCS.len()).filter(|&i| NPCS[i].1 == b.female && kits.contains(&NPCS[i].2)).collect() };
    let mut c = of(pool);
    if c.is_empty() {
        c = of(fallback);
    }
    if c.is_empty() {
        c = (0..NPCS.len()).filter(|&i| NPCS[i].1 == b.female).collect();
    }
    c[(mix(b.id) % c.len() as u64) as usize]
}

/// A template: one look in one pose, posed off-world until its static mesh is baked.
#[derive(Component)]
pub struct Template {
    look: usize,
    pose: usize,
    frames: u32,
}

/// On an NPC glb root once rigged: its `base_body_rig` node (the AnimationPlayer's)
#[derive(Component)]
pub struct Rigged(pub Entity);

pub fn setup_people(mut people: ResMut<People>, assets: Res<AssetServer>) {
    people.gltfs = NPCS.iter().map(|(n, _, _)| assets.load(format!("{DIR}/{n}_lod1.glb"))).collect();
    people.clips = assets.load(CLIPS);
}

/// Animation targets on an NPC glb instance (the glb has no clips of its own): the player on `base_body_rig`, an id
/// per node below it from the name path (the clips glb's paths: the same rig).
pub fn rig_on_ready(
    ev: On<WorldInstanceReady>,
    mut commands: Commands,
    people: Res<People>,
    children: Query<&Children>,
    names: Query<&Name>,
    meshes: Query<Option<&MeshMaterial3d<StandardMaterial>>, With<Mesh3d>>,
) {
    let Some((graph, _)) = people.graph.clone() else { return };
    let Some(rig) = children.iter_descendants(ev.entity).find(|e| names.get(*e).is_ok_and(|n| n.as_str() == "base_body_rig")) else {
        return;
    };
    let mut stack = vec![(rig, vec![names.get(rig).unwrap().clone()])];
    let mut bones = [None; BONES.len()];
    while let Some((e, path)) = stack.pop() {
        if let Some(i) = path.last().and_then(|n| BONES.iter().position(|b| *b == n.as_str())) {
            bones[i] = Some(e);
        }
        commands.entity(e).insert((AnimationTargetId::from_names(path.iter()), AnimatedBy(rig)));
        for c in children.get(e).into_iter().flatten() {
            if let Ok(m) = meshes.get(*c) {
                commands.entity(*c).insert(NotShadowCaster);
                if let Some(g) = m.and_then(|m| people.glow.get(&m.0.id())) {
                    commands.entity(*c).insert(MeshMaterial3d(g.clone()));
                }
            }
            if let Ok(n) = names.get(*c) {
                let mut p = path.clone();
                p.push(n.clone());
                stack.push((*c, p));
            }
        }
    }
    commands.entity(rig).insert((AnimationPlayer::default(), AnimationGraphHandle(graph), Bones(bones), Posing::default()));
    commands.entity(ev.entity).insert(Rigged(rig));
}

/// Motion V2: the procedural poses (`Posing`) laid on each rig's bones after the clip has set them this frame and
/// before the transforms propagate (PostUpdate, between `AnimationSystems` and `TransformSystems::Propagate`). Each
/// bone is turned in the character's frame (the rig node's, +y up, facing +z) and written back as its local
/// rotation: an aimed bone (a thigh forward, an arm out) turns its +y toward the aim by the pose's weight, a leaning
/// one turns about the character's left-right axis. The rig's own `Transform` and the hips' bone are read as the
/// clip left them, so the pose rides the idle's breathing and the walk's stride.
pub fn pose_bones(rigs: Query<(Entity, &Bones, &Posing)>, mut tfs: Query<&mut Transform>, cfg: Res<People>) {
    for (rig, bones, pose) in rigs.iter() {
        if !pose.any() && pose.lean.abs() < 1e-4 {
            continue;
        }
        let Ok(rt) = tfs.get(rig).map(|t| *t) else { continue };
        let Some(es) = bones.0.iter().copied().collect::<Option<Vec<Entity>>>() else { continue };
        let mut local: Vec<Transform> = es.iter().map(|e| tfs.get(*e).map(|t| *t).unwrap_or_default()).collect();
        // the bones' rotations in the character's frame, parents first (BONES is in that order)
        let global = |local: &[Transform]| {
            let mut g = [Quat::IDENTITY; BONES.len()];
            for i in 0..BONES.len() {
                g[i] = BONE_PARENT[i].map_or(rt.rotation, |p| g[p]) * local[i].rotation;
            }
            g
        };
        let parent = |g: &[Quat; BONES.len()], i: usize| BONE_PARENT[i].map_or(rt.rotation, |p| g[p]);
        // bone `i`'s +y turned toward `w` (the character's frame) by `k`
        let aim = |local: &mut [Transform], i: usize, w: Vec3, k: f32| {
            if k <= 1e-4 {
                return;
            }
            let g = global(local);
            let gi = g[i];
            let d = gi * Vec3::Y;
            let to = d.lerp(w.normalize(), k.min(1.0)).normalize_or(d);
            let q = Quat::from_rotation_arc(d, to);
            local[i].rotation = (parent(&g, i).inverse() * (q * gi)).normalize();
        };
        // bone `i` turned about the character's left-right axis (+: its top forward)
        let lean = |local: &mut [Transform], i: usize, a: f32| {
            if a.abs() <= 1e-4 {
                return;
            }
            let g = global(local);
            local[i].rotation = (parent(&g, i).inverse() * (Quat::from_rotation_x(a) * g[i])).normalize();
        };
        let k = pose.sit.min(1.0);
        if k > 0.0 {
            // the hips lowered to `sit_hips_m` (the rig node's frame: its turn and scale), the legs out in front, the
            // knees up a little, the hands on the thighs
            let s = rt.scale.x.max(1e-6);
            let up = (rt.rotation * (local[HIPS].translation * s)).y;
            let drop = (up - cfg.cfg.sit_hips_m).max(0.0) * k;
            local[HIPS].translation += rt.rotation.inverse() * Vec3::new(0.0, -drop, 0.0) / s;
            lean(&mut local, SPINE, -0.12 * k);
            aim(&mut local, L_THIGH, Vec3::new(0.18, 0.28, 1.0), k);
            aim(&mut local, R_THIGH, Vec3::new(-0.18, 0.28, 1.0), k);
            aim(&mut local, L_SHIN, Vec3::new(0.1, -0.4, 1.0), k);
            aim(&mut local, R_SHIN, Vec3::new(-0.1, -0.4, 1.0), k);
            aim(&mut local, L_ARM, Vec3::new(0.25, -1.0, 0.45), k);
            aim(&mut local, R_ARM, Vec3::new(-0.25, -1.0, 0.45), k);
            aim(&mut local, L_FORE, Vec3::new(0.05, -0.35, 1.0), k);
            aim(&mut local, R_FORE, Vec3::new(-0.05, -0.35, 1.0), k);
        }
        // the fight: the guard (fists up before the face), a strike (the right arm straight at the mark, the spine in
        // by the lean), a stagger (the spine back, the arms thrown out)
        let g = pose.guard.min(1.0) * (1.0 - pose.strike.min(1.0)) * (1.0 - pose.stagger.min(1.0));
        aim(&mut local, L_ARM, Vec3::new(0.35, -0.55, 0.75), pose.guard.min(1.0) * (1.0 - pose.stagger.min(1.0)));
        aim(&mut local, L_FORE, Vec3::new(-0.35, 0.9, 0.35), pose.guard.min(1.0) * (1.0 - pose.stagger.min(1.0)));
        aim(&mut local, R_ARM, Vec3::new(-0.35, -0.55, 0.75), g);
        aim(&mut local, R_FORE, Vec3::new(0.35, 0.9, 0.35), g);
        if pose.strike > 0.0 {
            lean(&mut local, SPINE, pose.lean.max(0.0));
            aim(&mut local, R_ARM, Vec3::new(0.12, 0.12, 1.0), pose.strike);
            aim(&mut local, R_FORE, Vec3::new(0.12, 0.12, 1.0), pose.strike);
        }
        if pose.stagger > 0.0 {
            lean(&mut local, SPINE, pose.lean.min(0.0));
            aim(&mut local, L_ARM, Vec3::new(1.0, -0.2, -0.3), pose.stagger);
            aim(&mut local, R_ARM, Vec3::new(-1.0, -0.2, -0.3), pose.stagger);
        }
        for (e, t) in es.iter().zip(local) {
            if let Ok(mut m) = tfs.get_mut(*e)
                && *m != t
            {
                *m = t;
            }
        }
    }
}

/// Once the glbs are in: the clips' graph, then a template per look and pose (off-world, under the ground).
pub fn prepare_people(
    mut commands: Commands,
    mut people: ResMut<People>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if people.templates || people.gltfs.is_empty() {
        return;
    }
    let Some(clips) = gltfs.get(&people.clips) else { return };
    if people.gltfs.iter().any(|h| gltfs.get(h).is_none()) {
        return;
    }
    let clip = |n: &str| clips.named_animations.get(n).cloned().unwrap_or_else(|| panic!("{CLIPS}: no `{n}` clip (npc_clips.py)"));
    let (graph, idx) = AnimationGraph::from_clips([clip("idle"), clip("walk")]);
    people.graph = Some((graphs.add(graph), [idx[0], idx[1]]));
    for look in 0..NPCS.len() {
        let scene = gltfs.get(&people.gltfs[look]).unwrap().scenes[0].clone();
        for pose in [IDLE, WALK, SIT] {
            commands
                .spawn((
                    WorldAssetRoot(scene.clone()),
                    Transform::from_xyz(look as f32 * 3.0, -400.0 - pose as f32 * 3.0, 0.0),
                    Template { look, pose, frames: 0 },
                    Name::new("people_template"),
                ))
                .observe(rig_on_ready);
        }
    }
    people.templates = true;
}

/// Pose the templates, then CPU-skin each one's mesh into a static mesh in its root's frame (`People::posed`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn bake_poses(
    mut commands: Commands,
    mut people: ResMut<People>,
    mut templates: Query<(Entity, &mut Template, &Rigged, &GlobalTransform)>,
    mut players: Query<(&mut AnimationPlayer, &mut Posing)>,
    children: Query<&Children>,
    skinned: Query<(&SkinnedMesh, &Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    globals: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    binds: Res<Assets<SkinnedMeshInverseBindposes>>,
) {
    if people.ready {
        return;
    }
    let Some((_, idx)) = people.graph.clone() else { return };
    for (e, mut t, rigged, root) in templates.iter_mut() {
        t.frames += 1;
        if let Ok((mut p, mut posing)) = players.get_mut(rigged.0)
            && !p.is_playing_animation(idx[POSE_CLIP[t.pose]])
        {
            p.play(idx[POSE_CLIP[t.pose]]).set_speed(0.0).seek_to(POSE_SEEK[t.pose]);
            // Motion V2: the sitting pose is the idle's frame with the bones posed (`pose_bones`)
            posing.sit = if t.pose == SIT { 1.0 } else { 0.0 };
            t.frames = 0;
        }
        // the clip applies in PostUpdate and the transforms propagate after it: a few frames is plenty
        if t.frames < 4 {
            continue;
        }
        let Some((sk, mesh, mat)) = children.iter_descendants(e).find_map(|c| skinned.get(c).ok()) else { continue };
        let (Some(src), Some(ib)) = (meshes.get(&mesh.0), binds.get(&sk.inverse_bindposes)) else { continue };
        let inv_root = root.affine().inverse();
        let joints: Vec<Mat4> = sk
            .joints
            .iter()
            .zip(ib.iter())
            .map(|(j, b)| Mat4::from(inv_root) * globals.get(*j).map_or(Mat4::IDENTITY, |g| g.to_matrix()) * *b)
            .collect();
        let posed = skin(src, &joints);
        let h = meshes.add(posed);
        let look = t.look;
        people.posed[look][t.pose] = Some(h);
        if people.mats[look].is_none() {
            // the look's atlas lit a little from within (`[view] people_glow`): the night street reads the crowd
            let mut m = mats.get(&mat.0).cloned().unwrap_or_default();
            let k = people.cfg.glow;
            m.emissive = LinearRgba::rgb(k, k, k);
            m.emissive_texture = m.base_color_texture.clone();
            let g = mats.add(m);
            people.glow.insert(mat.0.id(), g.clone());
            people.mats[look] = Some(g);
        }
        commands.entity(e).despawn();
    }
    if people.posed.iter().all(|p| p.iter().all(Option::is_some)) {
        people.ready = true;
        info!("people: {} looks posed (idle, walk, sit)", NPCS.len());
    }
}

/// The mesh `src` skinned by `joints` (joint x inverse bind, in the root's frame), without its skin attributes.
fn skin(src: &Mesh, joints: &[Mat4]) -> Mesh {
    let mut m = src.clone();
    let pos = match src.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
        _ => panic!("people: an NPC mesh without positions"),
    };
    let nrm = match src.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
        _ => vec![[0.0, 1.0, 0.0]; pos.len()],
    };
    let ji: Vec<[u16; 4]> = match src.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) {
        Some(VertexAttributeValues::Uint16x4(v)) => v.clone(),
        Some(VertexAttributeValues::Uint8x4(v)) => v.iter().map(|a| a.map(u16::from)).collect(),
        _ => panic!("people: an NPC mesh without joint indices"),
    };
    let jw = match src.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT) {
        Some(VertexAttributeValues::Float32x4(v)) => v.clone(),
        _ => panic!("people: an NPC mesh without joint weights"),
    };
    let (mut p2, mut n2) = (Vec::with_capacity(pos.len()), Vec::with_capacity(pos.len()));
    for i in 0..pos.len() {
        let mut k = Mat4::ZERO;
        for c in 0..4 {
            if jw[i][c] > 0.0 {
                k += joints[usize::from(ji[i][c])] * jw[i][c];
            }
        }
        p2.push(k.transform_point3(Vec3::from(pos[i])).to_array());
        n2.push(k.transform_vector3(Vec3::from(nrm[i])).normalize_or_zero().to_array());
    }
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, p2);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n2);
    m.remove_attribute(Mesh::ATTRIBUTE_JOINT_INDEX);
    m.remove_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT);
    m.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
    m
}

/// The clip `want` on the rig at `rate` (Motion V1: the walk's the drawn ground speed's, motion.rs).
fn play(p: &mut AnimationPlayer, idx: &[AnimationNodeIndex; 2], want: usize, id: u64, playing: &mut Option<usize>, rate: f32) {
    if *playing != Some(want) {
        p.stop_all();
        // a phase per body, so a crowd does not step in lockstep
        let phase = (mix(id ^ 0x5EED) % 1000) as f32 / 1000.0;
        p.play(idx[want]).repeat().seek_to(phase * if want == WALK { 1.03 } else { 8.7 });
        *playing = Some(want);
    }
    if let Some(a) = p.animation_mut(idx[want])
        && (a.speed() - rate).abs() > 1e-3
    {
        a.set_speed(rate);
    }
}

/// Draw `Sim::people`: the nearest animated (the glb, walk / idle), the rest a static posed mesh per look.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw_people(
    mut commands: Commands,
    mut people: ResMut<People>,
    mut sim: ResMut<Sim>,
    gltfs: Res<Assets<Gltf>>,
    rigged: Query<&Rigged>,
    mut players: Query<(&mut AnimationPlayer, &mut Posing)>,
    mut q: Query<(&mut Transform, &mut Visibility, Option<&mut Mesh3d>), Without<Template>>,
) {
    if sim.capsules {
        return;
    }
    if !people.ready {
        sim.hud = format!("{}\n{}", sim.hud, people.stats);
        return;
    }
    let Some((_, idx)) = people.graph.clone() else { return };
    let people: &mut People = &mut people;
    let focus = sim.subject_at.map_or(Vec3::ZERO, |(p, _)| p);
    // the nearest `anim_max` within `anim_m` are animated
    let mut order: Vec<(f32, usize)> = sim.people.iter().enumerate().map(|(i, b)| (b.at.distance_squared(focus), i)).collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let r2 = people.cfg.anim_m * people.cfg.anim_m;
    let mut near_now = vec![false; sim.people.len()];
    for (d2, i) in order.iter().take(people.cfg.anim_max) {
        if *d2 <= r2 {
            near_now[*i] = true;
        }
    }
    let mut seen_far: Vec<u64> = vec![];
    let mut seen_near: Vec<u64> = vec![];
    let (mut n_near, mut n_far, mut n_kids) = (0usize, 0usize, 0usize);
    for (i, b) in sim.people.iter().enumerate() {
        let look = look_of(b);
        let s = people.scale(b.age);
        n_kids += usize::from(s < 1.0);
        let f = b.feet();
        let at = Transform::from_translation(bv(f.x, f.y, f.z)).with_scale(Vec3::splat(s));
        let turn = Quat::from_rotation_y(b.yaw + std::f32::consts::PI);
        let tf = at.with_rotation(turn * b.tilt());
        let clip = if b.walking { WALK } else { IDLE };
        // Motion V2: a static body sitting wears the sit's baked mesh
        let pose = if b.pose.sit >= 0.5 && !b.walking && !b.lying { SIT } else { clip };
        if near_now[i] {
            n_near += 1;
            seen_near.push(b.id);
            let n = people.near.entry(b.id).or_insert_with(|| {
                let scene = gltfs.get(&people.gltfs[look]).unwrap().scenes[0].clone();
                let e = commands.spawn((WorldAssetRoot(scene), tf, Name::new("person"))).observe(rig_on_ready).id();
                Near { e, playing: None }
            });
            if let Ok((mut t, _, _)) = q.get_mut(n.e) {
                // Motion V2: a rigged body leans in its spine (`pose_bones`), not from its feet
                *t = at.with_rotation(turn * b.rig_tilt());
            }
            if let Ok(r) = rigged.get(n.e)
                && let Ok((mut p, mut posing)) = players.get_mut(r.0)
            {
                play(&mut p, &idx, clip, b.id, &mut n.playing, b.clip_rate);
                let want = b.posing();
                if *posing != want {
                    *posing = want;
                }
            }
            continue;
        }
        n_far += 1;
        seen_far.push(b.id);
        let mesh = people.posed[look][pose].clone().unwrap();
        match people.far.get(&b.id).copied() {
            // the look follows the dress (the fashion pass, daily): a changed look respawns
            Some((e, l, _)) if l != look => {
                commands.entity(e).despawn();
                let mat = people.mats[look].clone().unwrap();
                let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), tf, NotShadowCaster, Name::new("person_far"))).id();
                people.far.insert(b.id, (e, look, pose));
            }
            Some((e, _, p)) => {
                if let Ok((mut t, mut v, m)) = q.get_mut(e) {
                    *t = tf;
                    v.set_if_neq(Visibility::Inherited);
                    if p != pose
                        && let Some(mut m) = m
                    {
                        m.0 = mesh;
                    }
                }
                people.far.insert(b.id, (e, look, pose));
            }
            None => {
                let mat = people.mats[look].clone().unwrap();
                let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), tf, NotShadowCaster, Name::new("person_far"))).id();
                people.far.insert(b.id, (e, look, pose));
            }
        }
    }
    // a static body not drawn far this frame hides; an animated one not near any more goes (its static takes over)
    let seen_far: std::collections::HashSet<u64> = seen_far.into_iter().collect();
    for (id, (e, _, _)) in people.far.iter() {
        if !seen_far.contains(id)
            && let Ok((_, mut v, _)) = q.get_mut(*e)
        {
            v.set_if_neq(Visibility::Hidden);
        }
    }
    let seen_near: std::collections::HashSet<u64> = seen_near.into_iter().collect();
    let gone: Vec<u64> = people.near.keys().filter(|id| !seen_near.contains(id)).copied().collect();
    for id in gone {
        if let Some(n) = people.near.remove(&id) {
            commands.entity(n.e).despawn();
        }
    }
    people.stats = format!(
        "people: animated {n_near} (within {:.0} m, max {}), static {n_far}, children {n_kids}",
        people.cfg.anim_m, people.cfg.anim_max
    );
    sim.hud = format!("{}\n{}", sim.hud, people.stats);
}

/// The occlusion's player stand-in (occ.rs slot 0) wears the subject's look and plays its walk / idle.
#[allow(clippy::too_many_arguments)]
pub fn dress_standin(
    mut commands: Commands,
    mut people: ResMut<People>,
    sim: Res<Sim>,
    gltfs: Res<Assets<Gltf>>,
    bodies: Query<(Entity, &Body, Option<&Rigged>)>,
    mut players: Query<(&mut AnimationPlayer, &mut Posing)>,
    mut playing: Local<Option<usize>>,
    mut observed: Local<bool>,
) {
    if !people.ready || sim.capsules {
        return;
    }
    let Some(b) = sim.subject_body.as_ref() else { return };
    let Some((_, idx)) = people.graph.clone() else { return };
    for (e, body, rig) in bodies.iter() {
        if body.slot != 0 {
            continue;
        }
        if !*observed {
            commands.entity(e).observe(rig_on_ready);
            *observed = true;
        }
        let look = look_of(b);
        if people.standin != Some(look) {
            let scene = gltfs.get(&people.gltfs[look]).unwrap().scenes[0].clone();
            commands.entity(e).insert(WorldAssetRoot(scene)).remove::<Rigged>();
            people.standin = Some(look);
            *playing = None;
            continue;
        }
        if let Some(r) = rig
            && let Ok((mut p, mut posing)) = players.get_mut(r.0)
        {
            play(&mut p, &idx, if b.walking { WALK } else { IDLE }, b.id, &mut playing, b.clip_rate);
            let want = b.posing();
            if *posing != want {
                *posing = want;
            }
        }
    }
}

/// V3's wiring (main.rs, with a `Sim`): `[view]` from TheCity's assets/config.toml (a save's own copy predates the
/// V3 keys), the people systems, `--capsules`.
pub fn add(app: &mut App, capsules: bool) {
    let view = citysim::config::Config::find_assets_dir().map(|d| citysim::config::Config::load_from(&d).view).unwrap_or_default();
    {
        let mut sim = app.world_mut().resource_mut::<Sim>();
        sim.capsules = capsules;
        sim.cfg = view.clone();
    }
    app.insert_resource(People::new(PeopleCfg::from_view(&view)))
        .add_systems(Startup, setup_people)
        .add_systems(
            Update,
            (prepare_people, bake_poses, (draw_people, dress_standin).after(crate::sim::update_sim), frame_stats.after(draw_people)),
        )
        .add_systems(
            PostUpdate,
            pose_bones.after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate),
        );
}

/// The frame time's median and 90th percentile over the last 240 frames, on the sim's caption (a bench's shot line
/// logs it): the HUD's moving mean swings with one slow frame.
pub fn frame_stats(time: Res<Time>, mut sim: ResMut<Sim>, mut ring: Local<std::collections::VecDeque<f32>>) {
    ring.push_back(time.delta_secs() * 1000.0);
    if ring.len() > 240 {
        ring.pop_front();
    }
    let mut v: Vec<f32> = ring.iter().copied().collect();
    v.sort_by(f32::total_cmp);
    let at = |q: f32| v[((v.len() - 1) as f32 * q) as usize];
    sim.hud = format!("{}\nframe p50 {:.1} ms, p90 {:.1} ms (last {} frames)", sim.hud, at(0.5), at(0.9), v.len());
}
