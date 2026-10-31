//! The shadow check (CITY_SPEC §CAM requirement 3, P4_PLAN C): a dissolved occluder still casts its shadow.
//!
//! --bench shadow [--tag T] [--out DIR]: at two sites from cam_paths.json (`deck` = the expressway path, the
//! deck fades as a whole: L3; `tower` = the tower path, the tower is cut: L2) the player stands where the occluder
//! hides it, and a shadowed test spot sits on the camera's own line of sight 70 m behind the player, aimed at it:
//! the occluder's shadow falls exactly on what the cut reveals. Four renders per site:
//!   cut   occlusion on, the light on (the measurement)
//!   lit   the occluder hidden (no shadow: the lit reference)
//!   dark  the light off (the dark reference)
//! and the ground patch beside the player's feet (1.2 m to screen right, inside the cut) is read back: the median
//! luma of 9 x 9 px. Kept = (cut - dark) <= 0.25 (lit - dark). Control: CV_OCC_SHADOW_CUT=1 (the cut also runs in
//! shadow views) MUST fail it. Results: <out>/<tag>_shadow.json.

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use glclient::city::Bldg;
use glclient::occlusion::{OccFade, Occlusion};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::cam::{CityCam, View};
use crate::mesh::bv;
use crate::occ::Player;
use crate::stream::Stream;

const STATES: [&str; 3] = ["cut", "lit", "dark"];

#[derive(Component)]
pub struct TestSpot;

pub struct Site {
    name: String,
    feet: Vec3,
    facing: f32,
    deck: bool,
}

#[derive(Resource)]
pub struct ShadowBench {
    sites: Vec<Site>,
    site: usize,
    state: usize,
    frames: u32,
    shot: bool,
    lumas: Arc<Mutex<Vec<(String, String, f32, [f32; 2])>>>,
    pending: Arc<Mutex<u32>>,
    tag: String,
    out: PathBuf,
}

#[derive(serde::Deserialize)]
struct P {
    name: String,
    pts: Vec<[f32; 3]>,
    stand: usize,
    occluded: Vec<bool>,
}

#[derive(serde::Deserialize)]
struct Ps {
    paths: Vec<P>,
}

impl ShadowBench {
    pub fn new(dir: &std::path::Path, tag: &str, out: PathBuf) -> ShadowBench {
        std::fs::create_dir_all(&out).ok();
        let txt = std::fs::read_to_string(dir.join("cam_paths.json")).expect("cam_paths.json");
        let ps: Ps = serde_json::from_str(&txt).unwrap();
        let mut sites = vec![];
        for (path, site, deck) in [("xway", "deck", true), ("tower", "tower", false)] {
            let p = ps.paths.iter().find(|p| p.name == path).expect("path");
            // a few metres into the occluded run past the boundary
            let mut i = p.stand;
            for _ in 0..6 {
                if i + 1 < p.pts.len() && p.occluded[i + 1] {
                    i += 1;
                }
            }
            let (a, b) = (Vec3::from_array(p.pts[i.saturating_sub(1)]), Vec3::from_array(p.pts[(i + 1).min(p.pts.len() - 1)]));
            let d = (b - a).truncate().normalize_or(Vec2::Y);
            sites.push(Site { name: site.into(), feet: Vec3::from_array(p.pts[i]), facing: (-d.x).atan2(d.y), deck });
        }
        ShadowBench {
            sites,
            site: 0,
            state: 0,
            frames: 0,
            shot: false,
            lumas: Default::default(),
            pending: Default::default(),
            tag: tag.into(),
            out,
        }
    }
}

pub fn setup_spot(mut commands: Commands) {
    commands.spawn((
        SpotLight {
            color: Color::WHITE,
            intensity: 4.0e8,
            range: 400.0,
            radius: 0.0,
            outer_angle: 0.12,
            inner_angle: 0.08,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default(),
        Visibility::Hidden,
        TestSpot,
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn run_shadow_bench(
    mut commands: Commands,
    mut b: ResMut<ShadowBench>,
    mut player: ResMut<Player>,
    mut view: ResMut<View>,
    s: Res<Stream>,
    occ: Res<Occlusion>,
    mut spot: Query<(&mut Transform, &mut Visibility), (With<TestSpot>, Without<OccFade>)>,
    mut decks: Query<&mut Visibility, (With<OccFade>, Without<TestSpot>)>,
    mut bldgs: Query<&mut Visibility, (With<Bldg>, Without<OccFade>, Without<TestSpot>)>,
    cam: Query<(&Camera, &GlobalTransform), With<CityCam>>,
    mut exit: MessageWriter<AppExit>,
) {
    view.scripted = true;
    player.scripted = true;
    player.on = true;
    b.frames += 1;
    if b.site >= b.sites.len() {
        if *b.pending.lock().unwrap() == 0 || b.frames > 600 {
            let rows = b.lumas.lock().unwrap().clone();
            let mut sites = vec![];
            for site in &b.sites {
                let get = |st: &str| rows.iter().find(|r| r.0 == site.name && r.1 == st).map(|r| r.2);
                let (cut, lit, dark) = (get("cut"), get("lit"), get("dark"));
                // kept = None when the reference is invalid (no lit / dark contrast at the patch, or a state
                // unread): neither the check nor its control may pass on it
                let kept = match (cut, lit, dark) {
                    (Some(c), Some(l), Some(d)) if l - d > 0.02 => Some((c - d) <= 0.25 * (l - d)),
                    _ => None,
                };
                let px = rows.iter().find(|r| r.0 == site.name).map(|r| r.3);
                sites.push(serde_json::json!({"site": site.name, "cut": cut, "lit": lit, "dark": dark,
                    "shadow_share_left": match (cut, lit, dark) { (Some(c), Some(l), Some(d)) if l > d => Some((c - d) / (l - d)), _ => None },
                    "valid": kept.is_some(), "kept": kept, "patch_px": px}));
            }
            std::fs::create_dir_all(&b.out).ok();
            let p = b.out.join(format!("{}_shadow.json", b.tag));
            let doc = serde_json::json!({"bench": "shadow", "tag": b.tag, "settings": format!("{:?}", occ.settings),
                "rule": "kept = (cut - dark) <= 0.25 (lit - dark), lit - dark > 0.02; luma = median of 9x9 px beside the feet",
                "sites": sites});
            std::fs::write(&p, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
            info!("shadow bench -> {}: {}", p.display(), doc["sites"]);
            exit.write(AppExit::Success);
        }
        return;
    }
    let (feet, facing, deck, name) = {
        let st = &b.sites[b.site];
        (st.feet, st.facing, st.deck, st.name.clone())
    };
    let state = STATES[b.state];
    if b.frames == 1 {
        player.pos = feet;
        player.facing = facing;
        player.occ_on = true;
        view.hide_mass = state == "lit" && !deck;
        // kit buildings too: since P8 the cut can pass through a kit building on the spot's line, whose shadow
        // would otherwise darken the lit reference (s32: lit == dark at the tower site)
        for mut v in bldgs.iter_mut() {
            v.set_if_neq(if state == "lit" && !deck { Visibility::Hidden } else { Visibility::Inherited });
        }
        for mut v in decks.iter_mut() {
            v.set_if_neq(if state == "lit" && deck { Visibility::Hidden } else { Visibility::Inherited });
        }
        // the spot on the camera's line of sight, 70 m back from the player's chest, aimed at it
        if let Ok((mut t, mut v)) = spot.single_mut() {
            let chest = bv(feet.x, feet.y, feet.z + 1.0);
            let to_cam = -glclient::frame::view_dir();
            *t = Transform::from_translation(chest + to_cam * 70.0).looking_at(chest, Vec3::Y);
            *v = if state == "dark" { Visibility::Hidden } else { Visibility::Inherited };
        }
        b.shot = false;
        return;
    }
    // wait: streaming idle, the cut eased in, the fades settled
    let settled = occ.vols.first().is_some_and(|v| v.radius >= occ.settings.radius * 0.99 || state == "lit");
    if !b.shot && ((s.idle() && settled && b.frames > 90) || b.frames > 1800) {
        let Ok((c, gt)) = cam.single() else { return };
        let right = gt.right().as_vec3();
        let patch = bv(feet.x, feet.y, feet.z) + right * 1.2;
        let px = c.world_to_viewport(gt, patch).map(|p| [p.x, p.y]).unwrap_or([-1.0, -1.0]);
        let rows = b.lumas.clone();
        let pend = b.pending.clone();
        *pend.lock().unwrap() += 1;
        let (site, st) = (name.clone(), state.to_string());
        let path = b.out.join(format!("{}_shadow_{}_{}.png", b.tag, name, state));
        let mut save = bevy::render::view::screenshot::save_to_disk(path);
        commands.spawn(Screenshot::primary_window()).observe(move |ev: On<ScreenshotCaptured>| {
            let img = &ev.image;
            let (w, h) = (img.width() as i32, img.height() as i32);
            let mut v = vec![];
            if let Some(d) = img.data.as_ref() {
                for dy in -4..=4 {
                    for dx in -4..=4 {
                        let (x, y) = (px[0] as i32 + dx, px[1] as i32 + dy);
                        if x < 0 || y < 0 || x >= w || y >= h {
                            continue;
                        }
                        let o = ((y * w + x) * 4) as usize;
                        // BGRA (the swapchain), sRGB 8-bit -> luma
                        let (bb, g, r) = (d[o] as f32, d[o + 1] as f32, d[o + 2] as f32);
                        v.push((0.2126 * r + 0.7152 * g + 0.0722 * bb) / 255.0);
                    }
                }
            }
            v.sort_by(|a, c| a.total_cmp(c));
            let med = v.get(v.len() / 2).copied().unwrap_or(-1.0);
            rows.lock().unwrap().push((site.clone(), st.clone(), med, px));
            save(ev);
            *pend.lock().unwrap() -= 1;
        });
        b.shot = true;
        b.frames = 2;
    }
    if b.shot && b.frames > 30 {
        b.state += 1;
        b.frames = 0;
        if b.state >= STATES.len() {
            b.state = 0;
            b.site += 1;
            view.hide_mass = false;
            for mut v in bldgs.iter_mut() {
                v.set_if_neq(Visibility::Inherited);
            }
            for mut v in decks.iter_mut() {
                v.set_if_neq(Visibility::Inherited);
            }
        }
    }
}
