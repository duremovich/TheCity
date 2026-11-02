//! City view (CITY_SPEC §PHASE P3, spec/city/P3_PLAN.md C): the generated city's chunks (pipeline/city/chunks.py:
//! ground tiles, massing boxes, the elevated layers) streamed under the game camera (SPEC §CAM 5.1) and a free
//! overview, at night (glclient's look); a pinned hand-built block (overrides.toml [[pin]], e.g. blk01) loads
//! through glclient (its glbs, gl_* extras, cutaway and lights).
//!
//! The game camera (the view milestone V4b, cam.rs): a perspective orbit follow-cam around the subject; the cutaway
//! (occ.rs + glclient::occlusion): everything on the camera's side of the subject above its feet is cut away in a
//! box, its own building cut to its floor while it is indoors.
//!
//! cargo run --release -p cityview -- [--city city/isle_m] [--at X,Y] [--zoom W] [--yaw DEG] [--pitch DEG] [--overview]
//!                        [--res WxH] [--no-player | --player] [--squad N] [--day] [--occ off]
//!                        [--bench stream|overview|hold|shots|one] [--speed M_S] [--tag T] [--out DIR]
//! (--bench one: a single screenshot of the view the args set up, named --tag; --zoom is the frame's width at the
//! subject in metres, --yaw the camera's bearing from it (0 = south, 90 = east), --pitch its angle above the
//! horizon; with --overview they set the overview orbit; --occ off: no cutaway, the A/B)
//! CV_NO_OCC=1 builds the pre-P4 pipeline (no cut, Opaque materials, no depth prepass): a baseline.
//! Generated buildings are P6 kit streams (pipeline/buildings/kit.py: out/bldg/kitplaced_*.json; interiors in the 3 x 3 chunk ring around the focus, CV_KIT_RING=off: all; CV_NO_GEN=1: massing only).
//! Keys: see cam.rs (WASD pan / walk, Q / E rotate, Z / X pitch, wheel or - / = zoom, right- or middle-drag orbit,
//! Tab overview, L day light, H hud, P screenshot) and occ.rs (F player on / off, G cutaway on / off).
//! --sim FILE [--sim-rate TICKS_PER_S] [--sim-frame N]: a sim replay (city-simview/0.1 from engine/simsnap) drawn as
//! placeholders on the city (sim.rs; spec/integration/SIM_VIEW_CONTRACT.md).
//! --sim REC.simrec.gz [--follow INDEX] [--rate ..] [--from ..] [--shots ..]: a shadow's recording (the view milestone
//! V6, OPEN-270, `citysim-cli shadow --record`, recplay.rs) played through the follow-cam: the pick's ring as the
//! shadow's world had it, so a diary line and the shot at its minute agree. `--shots T` (live or recorded) shows the
//! state the diary stamps T: frame T + 1 (`citysim::view`'s module doc); the HUD's caption leads with that minute.
//! --save FILE --follow INDEX [--rate TICKS_PER_S] [--from TICK] [--shots T1,T2,..]: the follow-cam (the city milestone
//! C2, simthread.rs): a sim thread runs the save (`citysim`, in process) and the camera follows entity INDEX; --rate
//! defaults to walk-matched (`[view] walk_m_s` over the `[exec]` pace). A tick is a number or `D:HH:MM`. With
//! `--bench one`, --shots takes one PNG at each tick (`<tag>_<tick>.png`). glclient's tram and traffic are off.
//! [--watch-vehicle INDEX]: the camera on that vehicle instead of the subject (the view milestone V4: an El train's
//! riders seen when the subject never rides; 0: the train with the most riders now; the subject still drives the ring
//! and its stand-in marks the camera).
//! [--config PATH] [--reload-at TICK]: config hot-reload (the view milestone V1, OPEN-262, simthread.rs): the R key,
//! a change of the file (default `assets/config.toml`) or the tick reloads it into the running world at a tick
//! boundary (`citysim::reload`: the live keys applied, the build-time keys kept and reported in the log and the HUD).

// Bevy systems take their params as arguments and query tuples as types: the two lints every Bevy crate allows.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod bench;
mod cam;
mod data;
mod lots;
mod mesh;
mod occ;
mod people;
mod recplay;
mod sim;
mod simthread;
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
    occlusion::{CutSettings, Cutaway, OccSet, OcclusionPlugin},
};
use std::path::{Path, PathBuf};

#[derive(Resource, Clone, Debug)]
struct Args {
    city: String,
    at: Option<Vec2>,
    zoom: f32,
    /// --yaw / --pitch (degrees): the game camera's orbit (default `[view] cam_yaw_deg / cam_pitch_deg`)
    yaw: Option<f32>,
    pitch: Option<f32>,
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
    /// --traffic-trace OUT: run the traffic sim headless around --at for --trace-secs and write its trace (P7 G)
    traffic_trace: Option<PathBuf>,
    trace_secs: f32,
    /// --occ off: no cutaway (the A/B)
    occ_on: bool,
    /// --sim FILE: sim frames (city-simview/0.1, engine/simsnap) drawn as placeholders (sim.rs)
    sim: Option<PathBuf>,
    sim_rate: f32,
    sim_frame: usize,
    /// --save FILE --follow INDEX: the follow-cam (simthread.rs)
    save: Option<PathBuf>,
    follow: Option<u32>,
    rate: Option<f64>,
    from: Option<u64>,
    shots: Vec<u64>,
    /// --config PATH: the file a reload reads (V1; `None`: `assets/config.toml` when it is there)
    config: Option<PathBuf>,
    /// --reload-at TICK: reload the config at that tick (V1; the bench's proof)
    reload_at: Option<u64>,
    /// V3: --capsules draws the old placeholders, not the people (people.rs)
    capsules: bool,
    /// V4: --watch-vehicle INDEX: the camera on that vehicle (an El train's riders), the subject still drives the ring
    watch_vehicle: Option<u32>,
}

/// A tick: a number, or `D:HH:MM` (day, hour, minute).
fn parse_tick(s: &str) -> u64 {
    let parts: Vec<u64> = s.split(':').map(|p| p.trim().parse().unwrap_or_else(|_| panic!("'{s}' is not a tick or D:HH:MM"))).collect();
    match parts[..] {
        [t] => t,
        [d, h, m] => {
            assert!(h < 24 && m < 60, "'{s}': hour or minute out of range");
            d * citysim::TICKS_PER_DAY + h * citysim::TICKS_PER_HOUR + m
        }
        _ => panic!("'{s}' is not a tick or D:HH:MM"),
    }
}

impl Args {
    fn parse() -> Args {
        let mut a = Args {
            city: "city/isle_m".into(),
            at: None,
            zoom: 60.0,
            yaw: None,
            pitch: None,
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
            traffic_trace: None,
            trace_secs: 300.0,
            occ_on: true,
            sim: None,
            sim_rate: 0.7,
            sim_frame: 0,
            save: None,
            follow: None,
            rate: None,
            from: None,
            shots: vec![],
            config: None,
            reload_at: None,
            capsules: false,
            watch_vehicle: None,
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
                "--yaw" => {
                    a.yaw = Some(next(i).parse().expect("--yaw DEG"));
                    i += 1;
                }
                "--pitch" => {
                    a.pitch = Some(next(i).parse().expect("--pitch DEG"));
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
                "--occ" => {
                    a.occ_on = next(i) != "off";
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
                "--capsules" => a.capsules = true,
                "--watch-vehicle" => {
                    a.watch_vehicle = Some(next(i).parse().expect("--watch-vehicle INDEX"));
                    i += 1;
                }
                "--save" => {
                    a.save = Some(PathBuf::from(next(i)));
                    i += 1;
                }
                "--follow" => {
                    a.follow = Some(next(i).parse().expect("--follow INDEX"));
                    i += 1;
                }
                "--rate" => {
                    let r: f64 = next(i).parse().expect("--rate TICKS_PER_S");
                    assert!(r >= 0.0, "--rate must be 0 or more");
                    a.rate = Some(r);
                    i += 1;
                }
                "--from" => {
                    a.from = Some(parse_tick(&next(i)));
                    i += 1;
                }
                "--shots" => {
                    a.shots = next(i).split(',').map(parse_tick).collect();
                    assert!(a.shots.windows(2).all(|w| w[0] < w[1]), "--shots: ticks in rising order");
                    i += 1;
                }
                "--config" => {
                    a.config = Some(PathBuf::from(next(i)));
                    i += 1;
                }
                "--reload-at" => {
                    a.reload_at = Some(parse_tick(&next(i)));
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
        // V6: a recording (`--sim REC.simrec.gz`) plays through the follow-cam: --follow, --rate, --from, --shots
        let rec = a.sim.as_deref().is_some_and(recplay::is_recording);
        assert!(a.save.is_none() || a.follow.is_some(), "--save needs --follow");
        assert!(a.follow.is_none() || a.save.is_some() || rec, "--follow needs --save or a recording (--sim REC.simrec.gz)");
        assert!(a.save.is_none() || a.sim.is_none(), "--save and --sim are exclusive");
        assert!(a.shots.is_empty() || a.bench.as_deref() == Some("one"), "--shots needs --bench one");
        assert!(a.shots.is_empty() || a.save.is_some() || rec, "--shots needs --save or a recording");
        assert!(a.watch_vehicle.is_none() || a.save.is_some(), "--watch-vehicle needs --save");
        assert!((a.config.is_none() && a.reload_at.is_none()) || a.save.is_some(), "--config and --reload-at need --save");
        if let Some(p) = &a.config {
            assert!(p.is_file(), "--config {}: no such file", p.display());
        }
        a
    }
}

/// TheCity's workspace root (view/cityview/../..).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).expect("view/cityview sits two below the root").to_path_buf()
}

/// TheCityGraphics' assets/ directory (the AssetPlugin root: building glbs, block files, the city outputs), generated
/// there and read here: `CITYVIEW_ASSETS`, default `<TheCity>/../TheCityGraphics/assets` (the two repos are siblings
/// on this box; a worktree sets the variable). A machine path, not a sim number: it stays out of assets/config.toml.
/// A missing root fails at startup with the path it tried.
fn asset_root() -> PathBuf {
    let (root, from) = match std::env::var_os("CITYVIEW_ASSETS") {
        Some(p) => (PathBuf::from(p), "CITYVIEW_ASSETS"),
        None => match workspace_root().parent() {
            Some(up) => (up.join("TheCityGraphics").join("assets"), "the default: CITYVIEW_ASSETS is unset"),
            None => (PathBuf::from("TheCityGraphics/assets"), "the default: CITYVIEW_ASSETS is unset"),
        },
    };
    if !root.is_dir() {
        eprintln!("cityview: no asset root at {} ({from}); set CITYVIEW_ASSETS to TheCityGraphics/assets", root.display());
        std::process::exit(2);
    }
    root
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
    // `[view]` from TheCity's assets/config.toml: the camera's and the cutaway's numbers (V4b)
    let vcfg = citysim::config::Config::find_assets_dir().map(|d| citysim::config::Config::load_from(&d).view).unwrap_or_default();
    let aspect = args.res.0 as f32 / args.res.1.max(1) as f32;
    let mut view = cam::View::new(
        start,
        args.zoom,
        Vec2::from(index.size_m),
        cam::CamCfg::from_view(&vcfg),
        args.yaw.unwrap_or(vcfg.cam_yaw_deg),
        args.pitch.unwrap_or(vcfg.cam_pitch_deg),
        aspect,
    );
    if args.overview {
        view.mode = cam::Mode::Overview;
        // C3's shot: --at frames the overview's orbit centre, --zoom its distance (m); --yaw / --pitch its angles
        if let Some(a) = args.at {
            view.orbit_at = Vec3::new(a.x, a.y, 60.0);
            view.dist = args.zoom.max(150.0);
        }
        if let Some(y) = args.yaw {
            view.yaw = y.to_radians();
        }
        if let Some(p) = args.pitch {
            view.pitch = p.to_radians();
        }
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
    // bench and screenshot output: --out DIR, default <TheCity>/target/cityview (gitignored)
    let out = args.out.clone().unwrap_or_else(|| workspace_root().join("target/cityview"));
    // the P3 benches / shots run without the player (their views are the city, not a player)
    let player_on =
        args.player.unwrap_or(args.follow.is_some() || !matches!(args.bench.as_deref(), Some("stream" | "overview" | "shots" | "one")));
    app.insert_resource(occ::Player {
        pos: start.extend(3.0),
        facing: 0.0,
        on: player_on,
        occ_on: args.occ_on,
        indoors: None,
        squad: args.squad,
        scripted: false,
        light: std::env::var("CV_NO_PLAYER_LIGHT").is_err(),
    })
    .insert_resource(Cutaway::new(
        CutSettings {
            width: vcfg.cut_width_m,
            behind: vcfg.cut_behind_m,
            above: vcfg.cut_above_m,
            band: vcfg.cut_band_m,
            rim_strength: vcfg.cut_rim,
            ceiling: vcfg.cut_ceiling_m,
            max_zoom: vcfg.cut_max_zoom_m,
            ..default()
        }
        .with_env(),
    ))
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
    .add_systems(PostUpdate, (occ::occ_switch.before(OccSet::Apply), windows::update_occ.after(OccSet::Apply)))
    .add_systems(
        Update,
        (
            cam::controls,
            occ::player_controls.after(cam::controls).before(stream::update_stream),
            occ::place_bodies.after(occ::player_controls),
            stream::update_stream.after(cam::controls),
            cam::ease_view.after(stream::update_stream).after(occ::player_controls),
            cam::apply_view.after(cam::ease_view),
            bench::update_hud,
            windows::update_haze.after(stream::update_stream),
            stream::massing_visibility,
            stream::gen_massing_visibility.after(stream::update_stream),
            bench::screenshot_key,
        ),
    );
    app.insert_resource(Pending { dir: dir.clone(), out: out.clone(), args: args.clone() });
    // V6: a recording plays through the follow-cam's buffer (recplay.rs), the JSON fixture through Sim::load
    let mut shared = None;
    if let Some(p) = &args.sim {
        if recplay::is_recording(p) {
            shared = Some(recplay::spawn(recplay::RecArgs {
                path: p.clone(),
                follow: args.follow,
                rate: args.rate,
                from: args.from,
                shots: args.shots.clone(),
            }));
        } else {
            app.insert_resource(sim::Sim::load(p, args.sim_rate, args.sim_frame, &dir))
                .add_systems(Startup, sim::setup_sim)
                .add_systems(Update, sim::update_sim.after(stream::update_stream));
        }
    }
    if let (Some(save), Some(follow)) = (&args.save, args.follow) {
        // the save reloads its name tables from TheCity's assets/ (CITYSIM_ASSETS wins when set)
        if std::env::var_os("CITYSIM_ASSETS").is_none() {
            // SAFETY: main, before any thread of ours starts (the sim thread and Bevy's come after)
            unsafe { std::env::set_var("CITYSIM_ASSETS", workspace_root().join("assets")) };
        }
        shared = Some(simthread::spawn(simthread::SimArgs {
            save: save.clone(),
            follow,
            rate: args.rate,
            from: args.from,
            shots: args.shots.clone(),
            // V1: the default is the repo root's config (the CLI runs from there), when it is there
            config: args.config.clone().or_else(|| Some(PathBuf::from("assets/config.toml")).filter(|p| p.is_file())),
            reload_at: args.reload_at,
        }));
    }
    if let Some(shared) = shared {
        let mut live = sim::Sim::live(shared.clone(), &dir);
        live.watch_vehicle = args.watch_vehicle;
        app.insert_resource(live)
            .insert_resource(glclient::SimDrives)
            .insert_resource(bench::Shots { shared })
            .add_systems(Startup, sim::setup_sim)
            .add_systems(
                Update,
                (sim::follow_subject.before(occ::player_controls), sim::update_sim.after(stream::update_stream), sim::reload_key),
            );
    }
    // V3: people drawn as people (people.rs)
    if args.sim.is_some() || args.save.is_some() {
        people::add(&mut app, args.capsules);
    }
    if args.bench.is_some() {
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
) {
    let ground_mat = gnd_mats.add(windows::ground_material(0.75));
    let window_mat = win_mats.add(windows::material(1.0));
    let s = stream::Stream::new(pending.dir.clone(), ground_mat, window_mat);
    let layers = data::Layers::load(&pending.dir);
    // the decks in pieces, the pylons / beams as one mesh (the cutaway cuts both)
    let (pieces, solid) = mesh::layer_parts(&layers);
    let deck_mat = layer_mats.add(windows::layer_material(0.6));
    let tris: usize = pieces.iter().map(|p| p.mb.tris()).sum::<usize>() + solid.tris();
    info!("layers: {} elements, {} deck pieces, {} tris", layers.elements.len(), pieces.len(), tris);
    for (i, p) in pieces.into_iter().enumerate() {
        commands.spawn((
            Mesh3d(meshes.add(p.mb.into_mesh())),
            MeshMaterial3d(deck_mat.clone()),
            Transform::default(),
            Name::new(format!("deck{i}")),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(solid.into_mesh())),
        MeshMaterial3d(layer_mats.add(windows::layer_material(0.6))),
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
        let mut b = bench::Bench::new(kind, &a.tag, pending.out.clone(), a.speed, a.zoom, &s, xw);
        b.at_ticks(&a.shots);
        commands.insert_resource(b);
    }
    commands.insert_resource(s);
}
