//! The player and the cutaway in the city view: the player stand-in (a street NPC glb, static pose) and its squad,
//! the game camera following the player (or the followed sim subject, sim.rs), and the cutaway's switch and subject
//! (glclient::occlusion, the view milestone V4b: the box on the camera's side of the subject, its own building cut
//! to its floor while it is indoors).
//!
//! Keys (game view): F player mode on / off (WASD walks the player; off = WASD pans the camera), G the cutaway on /
//! off (A/B), K the player light on / off.
//!
//! The player light (s30, the night streets): an unshadowed point light a little above the player, nudged toward
//! the camera, so the body reads in the dark without lifting the scene (a pool about a lamp's size, not a torch).

use bevy::{prelude::*, world_serialization::WorldInstanceReady};
use glclient::occlusion::{Cutaway, SilhouetteSource};

use crate::cam::{Mode, View};
use crate::mesh::bv;
use crate::stream::Stream;

pub const PLAYER_GLB: &str = "characters/base_body/street/round_06/glb/npc03_courier_lod1.glb";
pub const SQUAD_GLBS: [&str; 3] = [
    "characters/base_body/street/round_06/glb/npc06_netrunner_lod1.glb",
    "characters/base_body/street/round_06/glb/npc05_gang_heavy_lod1.glb",
    "characters/base_body/street/round_06/glb/npc04_street_medic_lod1.glb",
];
/// Squad offsets behind the player (world m, in the player's frame: x right, y forward)
const SQUAD_AT: [Vec2; 3] = [Vec2::new(-1.2, -1.6), Vec2::new(1.2, -1.6), Vec2::new(0.0, -3.0)];

/// The player light: warm white, about 70 W (lm), a few metres of reach; above the feet, toward the camera (SE)
pub const LIGHT_W: f32 = 70.0;
pub const LIGHT_RANGE: f32 = 8.0;
pub const LIGHT_AT: Vec3 = Vec3::new(0.7, -0.7, 2.4);

#[derive(Component)]
pub struct PlayerLight;

/// A body: slot 0 = the player, 1.. = the squad.
#[derive(Component)]
pub struct Body {
    pub slot: usize,
}

#[derive(Resource, Clone, Debug)]
pub struct Player {
    /// World (x east, y north, z up) feet position
    pub pos: Vec3,
    /// Heading (rad, 0 = north, counter-clockwise)
    pub facing: f32,
    /// The player walks and the camera follows (F); off = the free camera
    pub on: bool,
    /// The user's cutaway switch (G); the cut runs only in the game view
    pub occ_on: bool,
    /// The followed subject indoors (sim.rs): its building's rect (world metres x0, y0, x1, y1), cut at its ceiling
    pub indoors: Option<Vec4>,
    pub squad: usize,
    /// Scripted (bench): keys off, z given
    pub scripted: bool,
    /// The player light (K; CV_NO_PLAYER_LIGHT=1 starts it off)
    pub light: bool,
    /// C5: the followed subject's pose on the rig (sim.rs `BodyDraw::tilt`, `feet`'s step): slot 0 wears it
    pub tilt: Quat,
    pub nudge: Vec3,
}

pub fn spawn_bodies(mut commands: Commands, assets: Res<AssetServer>, player: Res<Player>) {
    for slot in 0..=player.squad.min(3) {
        let glb = if slot == 0 { PLAYER_GLB } else { SQUAD_GLBS[slot - 1] };
        commands
            .spawn((
                WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(glb))),
                Transform::from_translation(bv(player.pos.x, player.pos.y, player.pos.z)),
                Body { slot },
                SilhouetteSource,
                Name::new(format!("body{slot}")),
            ))
            .observe(arms_down);
    }
    commands.spawn((
        PointLight {
            color: Color::srgb_u8(0xFF, 0xE8, 0xD0),
            intensity: LIGHT_W * glclient::city::LM_PER_W,
            range: LIGHT_RANGE,
            radius: 0.2,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(bv(player.pos.x, player.pos.y, player.pos.z)),
        Visibility::Hidden,
        PlayerLight,
        Name::new("player_light"),
    ));
}

/// The NPC glbs are in a T-pose; lower the arms (a standing pose for the stand-in; no walk clip in P4).
fn arms_down(ev: On<WorldInstanceReady>, children: Query<&Children>, mut q: Query<(&Name, &mut Transform)>) {
    for e in children.iter_descendants(ev.entity).collect::<Vec<_>>() {
        let Ok((n, mut t)) = q.get_mut(e) else { continue };
        let a = match n.as_str() {
            "mixamorig:LeftArm" => -1.25,
            "mixamorig:RightArm" => 1.25,
            _ => continue,
        };
        t.rotation *= Quat::from_rotation_z(a);
    }
}

/// WASD walks the player (screen-aligned, like the camera pan), the camera follows; F / G switches.
pub fn player_controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player: ResMut<Player>,
    mut view: ResMut<View>,
    stream: Res<Stream>,
) {
    if !player.scripted && !view.scripted {
        if keys.just_pressed(KeyCode::KeyF) {
            player.on = !player.on;
            if player.on {
                // the player drops in at the view centre
                let z = stream.ground_z(view.at.x, view.at.y).unwrap_or(view.ground_z);
                player.pos = view.at.extend(z);
            }
        }
        if keys.just_pressed(KeyCode::KeyG) {
            player.occ_on = !player.occ_on;
        }
        if keys.just_pressed(KeyCode::KeyK) {
            player.light = !player.light;
        }
    }
    view.player = player.on;
    if !player.on || view.mode != Mode::Game {
        return;
    }
    if !player.scripted {
        let (up, right) = view.pan_axes();
        let held = |k: &[KeyCode]| k.iter().any(|k| keys.pressed(*k));
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
        if d != Vec2::ZERO {
            let speed = if keys.pressed(KeyCode::ShiftLeft) { 6.0 } else { 1.6 };
            let n = d.normalize();
            let p = player.pos.truncate() + n * speed * time.delta_secs();
            player.facing = (-n.x).atan2(n.y);
            if let Some(z) = stream.ground_z(p.x, p.y) {
                player.pos = p.extend(z);
            }
        }
    }
    if !player.scripted
        && let Some(z) = stream.ground_z(player.pos.x, player.pos.y)
    {
        // on the ground (the spawn point loads after the player: snap once it does)
        player.pos.z = z;
    }
    view.at = player.pos.truncate();
    view.ground_z = player.pos.z;
}

/// Bodies follow the player state (the squad trails it on the same surface).
pub fn place_bodies(
    player: Res<Player>,
    stream: Res<Stream>,
    view: Res<View>,
    mut q: Query<(&Body, &mut Transform, &mut Visibility), Without<PlayerLight>>,
    mut light: Query<(&mut Transform, &mut Visibility), With<PlayerLight>>,
) {
    for (mut t, mut vis) in light.iter_mut() {
        // night only, game view (the day rig does not need it)
        let on = player.on && player.light && view.light_t() < 0.5 && view.mode == Mode::Game;
        vis.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
        let p = player.pos + LIGHT_AT;
        t.translation = bv(p.x, p.y, p.z);
    }
    // V3: the NPC glbs face +z (Blender's -Y), south at heading 0: turned half round to face the heading
    let rot = Quat::from_rotation_y(player.facing + std::f32::consts::PI);
    for (b, mut t, mut vis) in q.iter_mut() {
        vis.set_if_neq(if player.on { Visibility::Inherited } else { Visibility::Hidden });
        let mut p = player.pos;
        if b.slot > 0 {
            let o = SQUAD_AT[b.slot - 1];
            let (s, c) = player.facing.sin_cos();
            // heading 0 = north (+y); right = east at heading 0
            let fwd = Vec2::new(-s, c);
            let right = Vec2::new(c, s);
            let xy = p.truncate() + right * o.x + fwd * o.y;
            // on the ground the squad stands on its own ground; on a deck / in a building at the player's level
            let z = stream.ground_z(xy.x, xy.y).filter(|z| (z - p.z).abs() < 0.6).unwrap_or(p.z);
            p = xy.extend(z);
        }
        if b.slot == 0 {
            p += player.nudge;
            t.translation = bv(p.x, p.y, p.z);
            t.rotation = rot * player.tilt;
            continue;
        }
        t.translation = bv(p.x, p.y, p.z);
        t.rotation = rot;
    }
}

/// The cutaway runs in the game view while the player (or the followed subject) is on and the user has it on; its
/// subject is the player's feet, its zoom the view's.
pub fn occ_switch(player: Res<Player>, view: Res<View>, mut cut: ResMut<Cutaway>) {
    // the pre-P4 baseline (CV_NO_OCC) has no cut built
    let want = player.on && player.occ_on && view.mode == Mode::Game && glclient::occlusion::built();
    let subject = player.on.then(|| bv(player.pos.x, player.pos.y, player.pos.z));
    // the cut follows the camera's eased aim, not the feet: a floor change eases the cut height with the camera
    let subject = match (subject, view.aim) {
        (Some(_), Some(a)) if view.mode == Mode::Game => Some(bv(a.x, a.y, a.z - view.cfg.look_up_m)),
        (s, _) => s,
    };
    // the rect in Bevy's ground plane (x, z = -north): min x, min z, max x, max z
    let own = player.indoors.map(|r| Vec4::new(r.x, -r.w, r.z, -r.y));
    let c = cut.bypass_change_detection();
    let changed = c.enabled != want || c.subject != subject || c.indoors != own || c.zoom != view.zoom;
    if changed {
        c.enabled = want;
        c.subject = subject;
        c.indoors = own;
        c.zoom = view.zoom;
        cut.set_changed();
    }
}
