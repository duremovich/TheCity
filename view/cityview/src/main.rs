//! City view (CITY_SPEC §PHASE P3, spec/city/P3_PLAN.md C): the generated city's chunks (pipeline/city/chunks.py:
//! ground tiles, massing boxes, the elevated layers) streamed under the game camera (SPEC §CAM 5.1) and a free
//! overview, at night (glclient's look); a pinned hand-built block (overrides.toml [[pin]], e.g. blk01) loads
//! through glclient (its glbs, gl_* extras, cutaway and lights).
//!
//! Camera occlusion (P4, occ.rs + glclient::occlusion): a player stand-in the camera follows; massing, pylons and
//! deck pieces dissolve around it when they hide it.
//!
//! cargo run --release -- [--city city/isle_m] [--at X,Y] [--zoom W] [--overview] [--res WxH] [--no-player | --player]
//!                        [--squad N] [--day] [--bench stream|overview|shots|one] [--speed M_S] [--tag T] [--out DIR]
//! (--bench one: a single screenshot of the view the args set up, named --tag)
//! --bench occ [--path a,b] [--occ off] [--no-probes] [--gpu] [--record FILE.mp4]: the P4 acceptance paths
//! (occbench.rs); --bench shadow: a dissolved occluder keeps its shadow (shadowbench.rs); CV_NO_OCC=1 builds the pre-P4 pipeline (no cut, Opaque materials, no depth prepass): a baseline.
//! Generated buildings are P6 kit streams (pipeline/buildings/kit.py: out/bldg/kitplaced_*.json; interiors in the 3 x 3 chunk ring around the focus, CV_KIT_RING=off: all; CV_NO_GEN=1: massing only).
//! Keys: see cam.rs (WASD pan / walk, Q / E zoom, Tab overview, L day light, H hud, P screenshot) and occ.rs
//! (F player on / off, G occlusion on / off).
//! --sim FILE [--sim-rate TICKS_PER_S] [--sim-frame N]: a sim replay (city-simview/0.1 from engine/simsnap) drawn as
//! placeholders on the city (sim.rs; spec/integration/SIM_VIEW_CONTRACT.md).

mod bench;
mod cam;
mod data;
mod mesh;
mod occ;
mod occbench;
mod shadowbench;
mod sim;
mod stream;
mod windows;

use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    window::{PresentMode, WindowResolution},
    winit::{UpdateMode, WinitSettings},
};
use glclient::{
    GlClientPlugin,
    city::City,
    frame::Focus,
    lighting::LightSettings,
    occlusion::{OccSet, OcclusionPlugin},
};
use std::path::{Path, PathBuf};

#[derive(Resource, Clone, Debug)]
struct Args {
    city: String,
    at: Option<Vec2>,
    zoom: f32,
    overview: bool,
    day: bool,
    /// Start with the massing hidden (the M key): screenshots of placed buildings behind other blocks
    hide_mass: bool,
    res: (u32, u32),
    bench: Option<String>,
    speed: f32,
    tag: String,
    out: Option<PathBuf>,
    /// None = the default (on, except in the P3 benches / shots)
    player: Option<bool>,
    squad: usize,
    paths: Option<Vec<String>>,
    /// --traffic-trace OUT: run the traffic sim headless around --at for --trace-secs and write its trace (P7 G)
    traffic_trace: Option<PathBuf>,
    trace_secs: f32,
    occ_on: bool,
    probes: bool,
    gpu: bool,
    record: Option<PathBuf>,
    /// --sim FILE: sim frames (city-simview/0.1, engine/simsnap) drawn as placeholders (sim.rs)
    sim: Option<PathBuf>,
    sim_rate: f32,
    sim_frame: usize,
}

impl Args {
    fn parse() -> Args {
        let mut a = Args {
            city: "city/isle_m".into(),
            at: None,
            zoom: 60.0,
            overview: false,
            day: false,
            hide_mass: false,
            res: (1920, 1080),
            bench: None,
            speed: 30.0,
            tag: "run".into(),
            out: None,
            player: None,
            squad: 0,
            paths: None,
            traffic_trace: None,
            trace_secs: 300.0,
            occ_on: true,
            probes: true,
            gpu: false,
            record: None,
            sim: None,
            sim_rate: 0.7,
            sim_frame: 0,
        };
        let v: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < v.len() {
            let next = |i: usize| v.get(i + 1).cloned().unwrap_or_else(|| panic!("{} needs a value", v[i]));
            match v[i].as_str() {
                "--city" => {
                    a.city = next(i);
                    i += 1;
                }
                "--at" => {
                    let s = next(i);
                    let (x, y) = s.split_once(',').expect("--at X,Y");
                    a.at = Some(Vec2::new(x.parse().unwrap(), y.parse().unwrap()));
                    i += 1;
                }
                "--zoom" => {
                    a.zoom = next(i).parse().unwrap();
                    i += 1;
                }
                "--overview" => a.overview = true,
                "--day" => a.day = true,
                "--hide-mass" => a.hide_mass = true,
                "--res" => {
                    let s = next(i);
                    let (w, h) = s.split_once('x').expect("--res WxH");
                    a.res = (w.parse().unwrap(), h.parse().unwrap());
                    i += 1;
                }
                "--bench" => {
                    a.bench = Some(next(i));
                    i += 1;
                }
                "--speed" => {
                    a.speed = next(i).parse().unwrap();
                    i += 1;
                }
                "--tag" => {
                    a.tag = next(i);
                    i += 1;
                }
                "--out" => {
                    a.out = Some(PathBuf::from(next(i)));
                    i += 1;
                }
                "--no-player" => a.player = Some(false),
                "--player" => a.player = Some(true),
                "--squad" => {
                    a.squad = next(i).parse().unwrap();
                    i += 1;
                }
                "--path" => {
                    a.paths = Some(next(i).split(',').map(String::from).collect());
                    i += 1;
                }
                "--occ" => {
                    a.occ_on = next(i) != "off";
                    i += 1;
                }
                "--no-probes" => a.probes = false,
                "--gpu" => a.gpu = true,
                "--record" => {
                    a.record = Some(PathBuf::from(next(i)));
                    i += 1;
                }
                "--traffic-trace" => {
                    a.traffic_trace = Some(PathBuf::from(next(i)));
                    i += 1;
                }
                "--sim" => {
                    a.sim = Some(PathBuf::from(next(i)));
                    i += 1;
                }
                "--sim-rate" => {
                    a.sim_rate = next(i).parse().unwrap();
                    i += 1;
                }
                "--sim-frame" => {
                    a.sim_frame = next(i).parse().unwrap();
                    i += 1;
                }
                "--trace-secs" => {
                    a.trace_secs = next(i).parse().unwrap();
                    i += 1;
                }
                o => panic!("unknown arg {o}"),
            }
            i += 1;
        }
        a
    }
}

/// The repo's assets/ directory (the AssetPlugin root: building glbs, block files, the city outputs).
fn asset_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn main() {
    let args = Args::parse();
    if std::env::var("CV_NO_OCC").is_ok_and(|v| v != "0") {
        glclient::occlusion::BUILT.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    let root = asset_root();
    if let Some(out) = &args.traffic_trace {
        // headless: the traffic sim around --at (the street ring's half size), CV_TRAFFIC_NO_GAP / _FREEZE controls
        let faults = glclient::traffic::Faults {
            no_gap: std::env::var("CV_TRAFFIC_NO_GAP").is_ok(),
            freeze: std::env::var("CV_TRAFFIC_FREEZE").is_ok(),
        };
        let at = args.at.expect("--traffic-trace needs --at X,Y");
        let src = root.join(&args.city).join("out/street/traffic.json");
        glclient::traffic::trace(&src, out, at, 2.5 * 128.0, args.trace_secs, faults).expect("traffic trace");
        return;
    }
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("cityview {}", args.city),
                    resolution: WindowResolution::from(args.res).with_scale_factor_override(1.0),
                    present_mode: PresentMode::AutoNoVsync,
                    resizable: false,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin { file_path: root.to_string_lossy().into_owned(), ..default() }),
        FrameTimeDiagnosticsPlugin::default(),
        windows::WindowsPlugin,
    ))
    // benches run unfocused: winit's default throttles an unfocused window to 60 Hz
    .insert_resource(WinitSettings { focused_mode: UpdateMode::Continuous, unfocused_mode: UpdateMode::Continuous });

    let dir = root.join(&args.city).join("out/chunks");
    let index = data::Index::load(&dir);
    let start = args.at.unwrap_or_else(|| index.views.get("D2").map(|p| Vec2::from(*p)).unwrap_or(Vec2::new(3000.0, 600.0)));
    let mut view = cam::View::new(start, args.zoom, Vec2::from(index.size_m));
    if args.overview {
        view.mode = cam::Mode::Overview;
    }
    view.day = args.day;
    view.hide_mass = args.hide_mass;
    // a pinned block via glclient (P3: the first pin; glclient holds one City)
    // CV_NO_PIN=1: no pinned block (bench isolation)
    let pins = if std::env::var("CV_NO_PIN").is_ok() { &[][..] } else { &index.pins[..] };
    let city = match pins.first() {
        Some(p) => {
            if pins.len() > 1 {
                warn!("{} pins: only the first loads (glclient holds one City)", pins.len());
            }
            City::at(root.clone(), &p.block, mesh::bv(p.origin_m[0], p.origin_m[1], p.origin_m[2]))
        }
        None => City { root: root.clone(), kinds: vec![], bldgs: vec![], blocks: vec![], cut_gen: 0, standalone: false },
    };
    // generated buildings (P6: kit.py writes out/bldg/kitplaced_<block>.json; the s28 checkpoint retired the
    // P5 baked glbs for generated lots); CV_NO_GEN=1: massing only
    let mut city = city;
    let no_gen = std::env::var("CV_NO_GEN").is_ok();
    let mut kits = if no_gen { vec![] } else { data::kit_placed(&root.join(&args.city)) };
    // CV_KIT_ONLY=PREFIX: only the kit lots whose id starts with it
    if let Ok(pre) = std::env::var("CV_KIT_ONLY") {
        kits.retain(|k| k.lot.starts_with(&pre));
    }
    let kit_lots: std::collections::HashSet<String> = kits.iter().map(|k| k.lot.clone()).collect();
    let tf_of = |at: [f32; 3], yaw: f32| {
        Transform::from_translation(mesh::bv(at[0], at[1], at[2])).with_rotation(Quat::from_rotation_y(yaw.to_radians()))
    };
    // lazy interiors (P6): interiors inside the 3 x 3 chunk ring around the focus, shells outside (glclient
    // KitRing re-stamps as the focus moves); CV_KIT_RING=off keeps every interior
    let ring = std::env::var("CV_KIT_RING").map_or(true, |v| v != "off");
    let chunk = |x: f32, y: f32| ((x / index.chunk_m).floor() as i32, (y / index.chunk_m).floor() as i32);
    let fc = chunk(start.x, start.y);
    // P8 B-A: kits stream per chunk (a building in its lot centre's chunk, as the massing) within CV_KIT_RADIUS
    // chunks of the focus (default 2: 5 x 5), unloading past radius + 1; the placed lots' massing stands in
    // elsewhere. CV_KIT_STREAM=off: every kit at startup (P6; the A/B control)
    let stream_on = std::env::var("CV_KIT_STREAM").map_or(true, |v| v != "off");
    let mut streamed = vec![];
    for k in kits.iter() {
        let c = chunk(k.at[0], k.at[1]);
        let near = (c.0 - fc.0).abs() <= 1 && (c.1 - fc.1).abs() <= 1;
        let kr = glclient::kit::KitRef { kit: k.kit.clone(), block: k.block.clone(), interiors: !ring || near };
        let idx = city.place_kit(&k.id, kr, k.floors, tf_of(k.at, k.yaw), k.x[0], k.x[1]);
        streamed.push((idx, k.chunk(index.chunk_m)));
    }
    if !kits.is_empty() {
        info!("{} generated buildings placed (kit{})", kits.len(), if stream_on { ", streamed" } else { "" });
        if kits.iter().any(|k| k.rect.is_none()) {
            warn!("kitplaced without lot rects (kit.py before P8 B-A): chunks keyed by the root corner, re-run kit.py");
        }
    }
    if stream_on && !kits.is_empty() {
        let radius: i32 = std::env::var("CV_KIT_RADIUS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
        app.insert_resource(glclient::kit::KitStream::new(index.chunk_m, radius, radius + 1, 64, streamed));
    }
    if ring && !kits.is_empty() {
        app.insert_resource(glclient::kit::KitRing {
            chunk_m: index.chunk_m,
            ring: 1,
            // CV_KIT_LIGHT_M: the light radius around the focus (m)
            light_m: std::env::var("CV_KIT_LIGHT_M").ok().and_then(|v| v.parse().ok()).unwrap_or(96.0),
        });
    }
    data::PLACED.set(kit_lots).ok();
    // P7 street dressing (pipeline/city/street -> out/street/): lamps + lights + zebra paint around the focus;
    // CV_NO_STREET=1: none (bench comparisons); CV_STREET_LIGHT_M: the lamp-light radius (m, default the kit's 96)
    if std::env::var("CV_NO_STREET").is_err()
        && let Some(st) = glclient::street::Street::load(
            root.join(&args.city).join("out/street"),
            2,
            std::env::var("CV_STREET_LIGHT_M").ok().and_then(|v| v.parse().ok()).unwrap_or(96.0),
        )
    {
        app.insert_resource(st);
    }
    let out = args.out.clone().unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("results"));
    // the P3 benches / shots run without the player (their views are the city, not a player)
    let player_on = args.player.unwrap_or(!matches!(args.bench.as_deref(), Some("stream" | "overview" | "shots" | "one")));
    app.insert_resource(occ::Player {
        pos: start.extend(3.0),
        facing: 0.0,
        on: player_on,
        occ_on: true,
        squad: args.squad,
        scripted: false,
        light: std::env::var("CV_NO_PLAYER_LIGHT").is_err(),
    })
    .init_resource::<occ::LayerOccluders>()
    .insert_resource(city)
    .insert_resource(Focus { at: mesh::bv(start.x, start.y, 3.0), zoom: args.zoom })
    // no volumetric street fog at city scale (glclient's volume spans its bench street)
    // CV_SHADOWS=none: no shadow maps (a cost split for benches)
    .insert_resource(LightSettings {
        volumetric: false,
        shadows: if std::env::var("CV_SHADOWS").is_ok_and(|v| v == "none") {
            glclient::lighting::ShadowPolicy::None
        } else {
            LightSettings::default().shadows
        },
        ..default()
    })
    .insert_resource(view)
    .add_plugins((GlClientPlugin, OcclusionPlugin))
    .add_systems(Startup, (cam::setup_camera, setup_world, bench::setup_hud, occ::spawn_bodies))
    .add_systems(
        PostUpdate,
        (occ::occ_switch.before(OccSet::Prepare), occ::occ_cast.in_set(OccSet::Cast), windows::update_occ.after(OccSet::Apply)),
    )
    .add_systems(
        Update,
        (
            cam::controls,
            occ::player_controls.after(cam::controls).before(stream::update_stream),
            occ::place_bodies.after(occ::player_controls),
            stream::update_stream.after(cam::controls),
            cam::apply_view.after(stream::update_stream),
            bench::update_hud,
            windows::update_haze.after(stream::update_stream),
            stream::massing_visibility,
            stream::gen_massing_visibility.after(stream::update_stream),
            bench::screenshot_key,
        ),
    );
    app.insert_resource(Pending { dir: dir.clone(), out: out.clone(), args: args.clone() });
    if let Some(p) = &args.sim {
        app.insert_resource(sim::Sim::load(p, args.sim_rate, args.sim_frame, &dir))
            .add_systems(Startup, sim::setup_sim)
            .add_systems(Update, sim::update_sim.after(stream::update_stream));
    }
    if args.bench.as_deref() == Some("occ") {
        let oa = occbench::OccArgs {
            paths: args.paths.clone(),
            occ_on: args.occ_on,
            probes: args.probes && !args.gpu,
            gpu: args.gpu,
            record: args.record.clone(),
            tag: args.tag.clone(),
            out: out.clone(),
            res: args.res,
        };
        let ob = occbench::OccBench::new(&dir, &oa);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(ob.dt)))
            .insert_resource(ob)
            .add_systems(Startup, occbench::setup_probes.after(cam::setup_camera))
            .add_systems(
                Update,
                (
                    occbench::run_occ_bench.before(cam::controls).before(occ::player_controls),
                    occbench::sync_probes.after(cam::apply_view),
                    occbench::body_layers.run_if(|b: Res<occbench::OccBench>| b.probes),
                ),
            );
        if args.gpu {
            app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
        }
    } else if args.bench.as_deref() == Some("shadow") {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
            .insert_resource(shadowbench::ShadowBench::new(&dir, &args.tag, out.clone()))
            .add_systems(Startup, shadowbench::setup_spot)
            .add_systems(Update, shadowbench::run_shadow_bench.before(cam::controls).before(occ::player_controls));
    } else if args.bench.is_some() {
        app.add_systems(Update, bench::run_bench.before(cam::controls).run_if(resource_exists::<bench::Bench>));
    }
    app.run();
}

#[derive(Resource)]
struct Pending {
    dir: PathBuf,
    out: PathBuf,
    args: Args,
}

/// The stream (materials), the elevated layers (one mesh), the sea; the bench.
fn setup_world(
    mut commands: Commands,
    pending: Res<Pending>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mut gnd_mats: ResMut<Assets<windows::GroundMaterial>>,
    mut win_mats: ResMut<Assets<windows::WindowMaterial>>,
    mut layer_mats: ResMut<Assets<windows::LayerMaterial>>,
    mut occluders: ResMut<occ::LayerOccluders>,
) {
    let ground_mat = gnd_mats.add(windows::ground_material(0.75));
    let window_mat = win_mats.add(windows::material(1.0));
    let s = stream::Stream::new(pending.dir.clone(), ground_mat, window_mat);
    let layers = data::Layers::load(&pending.dir);
    // the decks in pieces (L3 fades a piece as a whole), the pylons / beams as one mesh (L2 cuts them)
    let (pieces, solid) = mesh::layer_parts(&layers);
    let deck_mat = layer_mats.add(windows::layer_material(0.6, windows::DECKS));
    let tris: usize = pieces.iter().map(|p| p.mb.tris()).sum::<usize>() + solid.tris();
    info!("layers: {} elements, {} deck pieces, {} tris", layers.elements.len(), pieces.len(), tris);
    for (i, p) in pieces.into_iter().enumerate() {
        let e = commands.spawn(occ::deck_bundle(Handle::default(), deck_mat.clone(), i)).id();
        occluders.add_piece(e, &p);
        commands.entity(e).insert(Mesh3d(meshes.add(p.mb.into_mesh())));
    }
    occluders.boxes = mesh::layer_boxes(&layers);
    commands.spawn((
        Mesh3d(meshes.add(solid.into_mesh())),
        MeshMaterial3d(layer_mats.add(windows::layer_material(0.6, windows::PYLONS))),
        Transform::default(),
        Name::new("layers_solid"),
    ));
    let [w, h] = s.index.size_m;
    let sea = StandardMaterial {
        base_color: Color::srgb_u8(0x0A, 0x10, 0x18),
        // rough enough that the moon is a sheen, not a hot spot (it bloomed into a disc in the overview)
        perceptual_roughness: 0.45,
        reflectance: 0.25,
        ..default()
    };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(w + 60000.0, h + 60000.0))),
        MeshMaterial3d(std_mats.add(sea)),
        Transform::from_translation(mesh::bv(w / 2.0, h / 2.0, 0.0)),
        Name::new("sea"),
    ));
    if let Some(kind) = &pending.args.bench {
        let a = &pending.args;
        // the expressway where the camera sees it (open water / low sheds in front; review p3 r1: the D1 / D2 strip
        // south of the trunk hid it): the S leg's point nearest x 4300 and the SE curve
        let xw = layers.elements.iter().find(|e| e.kind == "expressway").map(|e| {
            let near = |x: f32, y: f32| {
                let p = e
                    .pts
                    .iter()
                    .min_by(|a, b| ((a[0] - x).powi(2) + (a[1] - y).powi(2)).total_cmp(&((b[0] - x).powi(2) + (b[1] - y).powi(2))))
                    .unwrap();
                Vec2::new(p[0], p[1])
            };
            (near(4300.0, 392.0), near(5850.0, 750.0))
        });
        commands.insert_resource(bench::Bench::new(kind, &a.tag, pending.out.clone(), a.speed, a.zoom, &s, xw));
    }
    commands.insert_resource(s);
}
