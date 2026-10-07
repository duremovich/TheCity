//! macroquad front end: squares and letters, a HUD, time controls, save/load.
//!
//! ```text
//! citysim-app [--seed N] [--map FILE] [--load FILE] [--fps] [--select INDEX | --select-kind Kind] [--select-name NAME] [--tab story|corp]
//!             [--overlay virt|word] [--select-node INDEX] [--select-runner] [--select-hunter]
//! ```
//!
//! `--map` overrides `[world] map` (the v2 256 x 192 map by default).
//! `--tab corp` with `--select-kind` opens the selected building's owning
//! corp's panel (M11 screenshots of the Corp panel).

mod camera;
mod input;
mod mission;
mod overlay;
mod render;
mod ui;
mod word_overlay;

use std::path::PathBuf;

use macroquad::prelude::*;

use citysim::{save, Config, EntityId, PlayerCommand, Speed, World};

use camera::Camera;

pub const WINDOW_W: i32 = 1280;
pub const WINDOW_H: i32 = 800;
/// Ticks per real second at 1×: one in-game day every three real minutes.
pub const BASE_TICKS_PER_SEC: f64 = 8.0;
/// Ticks dropped past this many per frame (1000× on a slow frame).
pub const MAX_TICKS_PER_FRAME: u32 = 4000;

/// Everything the renderer and UI need besides the world.
pub struct App {
    pub camera: Camera,
    pub paused: bool,
    pub speed: Speed,
    pub acc: f64,
    pub selected: Option<EntityId>,
    /// The inspector's open tab.
    pub inspector_tab: ui::inspector::InspectorTab,
    /// The agent whose open victim holes the inspector last bound.
    pub bound_for: Option<EntityId>,
    /// M15: the Known tab's holder scan (agent, day, rows), computed once
    /// per agent and day (UI only).
    pub known_cache: Option<ui::inspector::KnownCache>,
    pub follow: bool,
    pub show_fps: bool,
    pub fps_avg: f32,
    /// Smoothed sim rate: ticks run divided by the wall seconds they took.
    pub ticks_per_sec: f32,
    /// Frame the whole map on the first frame (`--fit`).
    pub fit_pending: bool,
    pub cmds: Vec<PlayerCommand>,
    pub saves_dir: PathBuf,
    pub status: String,
    pub status_until: f64,
    /// The pointer is over an egui panel (last frame): the map ignores it.
    pub ui_hover: bool,
    pub log: ui::log::LogState,
    pub city: ui::city::CityState,
    /// M12 D43: `B` draws the district borders; with it on, a click on a
    /// street tile selects the district.
    pub show_districts: bool,
    /// M12: the district shown in the right panel when nothing else is selected.
    pub selected_district: Option<citysim::DistrictId>,
    /// M12 D43: `L` draws the litter heat on street tiles.
    pub show_litter: bool,
    /// M13 D47: `K` heats districts by hooked adults (`A` pans).
    pub show_hooked: bool,
    /// M14 § 10: `N` draws the Virt plane over a ghosted city; a click
    /// selects a node.
    pub show_virt: bool,
    /// M15 § 10: `J` draws the word: each district's talk (Σ pool reach),
    /// hunters on a stake-out, red lines between factions in a vendetta.
    pub show_word: bool,
    /// M15: keep the City panel scrolled to its Word section (`--scroll-word`, screenshots).
    pub scroll_word: bool,
    /// M14: the node shown in the Node panel.
    pub selected_node: Option<citysim::virt::NodeId>,
    /// M14: the run shown in the Run panel (live in `world.runs`, else `run_log`).
    pub selected_run: Option<citysim::virt::RunId>,
    /// M14 § 7: the gang whose raid the Mission panel shows.
    pub selected_mission: Option<EntityId>,
}

impl App {
    pub fn new(show_fps: bool, map_w: usize, map_h: usize) -> App {
        App {
            camera: Camera::new(map_w, map_h),
            paused: false,
            speed: Speed::X1,
            acc: 0.0,
            selected: None,
            inspector_tab: ui::inspector::InspectorTab::default(),
            bound_for: None,
            known_cache: None,
            follow: false,
            show_fps,
            fps_avg: 60.0,
            ticks_per_sec: 0.0,
            fit_pending: false,
            cmds: Vec::new(),
            saves_dir: PathBuf::from("saves"),
            status: String::new(),
            status_until: 0.0,
            ui_hover: false,
            log: ui::log::LogState::default(),
            city: ui::city::CityState::default(),
            show_districts: false,
            selected_district: None,
            show_litter: false,
            show_hooked: false,
            show_virt: false,
            show_word: false,
            scroll_word: false,
            selected_node: None,
            selected_run: None,
            selected_mission: None,
        }
    }

    /// Clear every panel selection (Escape).
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_district = None;
        self.selected_node = None;
        self.selected_run = None;
        self.selected_mission = None;
        self.follow = false;
    }

    /// Show a line in the HUD for a few seconds.
    pub fn notify(&mut self, text: impl Into<String>) {
        self.status = text.into();
        self.status_until = get_time() + 4.0;
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "The City".to_string(),
        window_width: WINDOW_W,
        window_height: WINDOW_H,
        high_dpi: false,
        ..Default::default()
    }
}

struct Args {
    seed: u64,
    load: Option<PathBuf>,
    fps: bool,
    /// Render a few frames, write this PNG, exit. For smoke tests.
    screenshot: Option<PathBuf>,
    /// Run this many ticks before the first frame.
    start_tick: u64,
    /// Select this entity index at start (inspector smoke tests).
    select: Option<u32>,
    /// Select the citizen with this name at start (`--select-name "First Last"`).
    select_name: Option<String>,
    /// Select the first building of this kind at start (building panel smoke tests).
    select_kind: Option<citysim::BuildingKind>,
    /// Open the inspector on its Story tab (`--tab story`).
    story_tab: bool,
    /// M15: open the inspector on its Known tab (`--tab known`).
    known_tab: bool,
    /// Select the selected building's owning corp instead (`--tab corp`).
    corp_tab: bool,
    /// Do not bind the selected agent's open holes on open (screenshots of an unbound line).
    no_autobind: bool,
    /// Frame the whole map (screenshots of the full city).
    fit: bool,
    /// Map file overriding `[world] map` (made absolute).
    map: Option<PathBuf>,
    /// M12: start with the district borders on (`B`).
    districts: bool,
    /// M12: open the District panel on this district index at start.
    select_district: Option<u8>,
    /// M12: start with the litter heat on (`L`).
    litter: bool,
    /// M13: start with the hooked-adults heat on (`K`).
    hooked: bool,
    /// M14: `--overlay virt` starts with the Virt overlay on (`N`).
    overlay_virt: bool,
    /// M15: `--overlay word` starts with the Word overlay on (`J`).
    overlay_word: bool,
    /// M15: select the first hunter (a stake-out first) on its Known tab.
    select_hunter: bool,
    /// M15: scroll the City panel to its Word section.
    scroll_word: bool,
    /// M14: open the Node panel on this node index at start.
    select_node: Option<u16>,
    /// M14: select the first jacked-in agent and open its Run panel.
    select_runner: bool,
    /// M14: open the Mission panel of the first streamed raid (else the first raid).
    select_raid: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        seed: 42,
        load: None,
        fps: false,
        screenshot: None,
        start_tick: 0,
        select: None,
        select_kind: None,
        select_name: None,
        story_tab: false,
        known_tab: false,
        corp_tab: false,
        no_autobind: false,
        fit: false,
        map: None,
        districts: false,
        select_district: None,
        litter: false,
        hooked: false,
        overlay_virt: false,
        overlay_word: false,
        select_hunter: false,
        scroll_word: false,
        select_node: None,
        select_runner: false,
        select_raid: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => args.seed = it.next().and_then(|s| s.parse().ok()).expect("--seed <N>"),
            "--load" => args.load = Some(PathBuf::from(it.next().expect("--load <FILE>"))),
            "--fps" => args.fps = true,
            "--map" => {
                let p = PathBuf::from(it.next().expect("--map <FILE>"));
                args.map = Some(std::fs::canonicalize(&p).unwrap_or_else(|e| panic!("--map {}: {e}", p.display())));
            }
            "--screenshot" => args.screenshot = Some(PathBuf::from(it.next().expect("--screenshot <FILE>"))),
            "--start-tick" => args.start_tick = it.next().and_then(|s| s.parse().ok()).expect("--start-tick <N>"),
            "--select" => args.select = Some(it.next().and_then(|s| s.parse().ok()).expect("--select <INDEX>")),
            "--select-kind" => {
                args.select_kind = Some(
                    it.next().and_then(|s| citysim::BuildingKind::parse(&s)).expect("--select-kind <BuildingKind>"),
                )
            }
            "--select-name" => args.select_name = Some(it.next().expect("--select-name <NAME>")),
            "--fit" => args.fit = true,
            "--districts" => args.districts = true,
            "--litter" => args.litter = true,
            "--hooked" => args.hooked = true,
            "--overlay" => match it.next().as_deref() {
                Some("virt") => args.overlay_virt = true,
                Some("word") => args.overlay_word = true,
                other => panic!("--overlay virt|word, got {other:?}"),
            },
            "--select-hunter" => args.select_hunter = true,
            "--scroll-word" => args.scroll_word = true,
            "--select-node" => {
                args.select_node = Some(it.next().and_then(|s| s.parse().ok()).expect("--select-node <INDEX>"))
            }
            "--select-runner" => args.select_runner = true,
            "--select-raid" => args.select_raid = true,
            "--select-district" => {
                args.select_district = Some(it.next().and_then(|s| s.parse().ok()).expect("--select-district <INDEX>"))
            }
            "--no-autobind" => args.no_autobind = true,
            "--tab" => match it.next().as_deref() {
                Some("story") => args.story_tab = true,
                Some("known") => args.known_tab = true,
                Some("corp") => args.corp_tab = true,
                other => panic!("--tab story|known|corp, got {other:?}"),
            },
            other => panic!("unknown argument {other}"),
        }
    }
    args
}

#[macroquad::main(window_conf)]
async fn main() {
    let args = parse_args();
    let mut world = match &args.load {
        Some(path) => save::load_from_file(path).unwrap_or_else(|e| panic!("{e}")),
        None => {
            let mut config = Config::load();
            if let Some(map) = &args.map {
                config.world.map = map.to_string_lossy().into_owned();
            }
            World::new(args.seed, config)
        }
    };
    world.run_ticks(args.start_tick);
    let mut app = App::new(args.fps, world.map.w(), world.map.h());
    if let Some(index) = args.select {
        app.selected = world.citizens().into_iter().find(|id| id.index == index);
        if let Some(p) = app.selected.and_then(|id| world.comp::<citysim::Position>(id)) {
            app.camera.centre_on(p.tile);
        }
    }
    if let Some(name) = &args.select_name {
        // Names repeat: prefer the namesake with an open hole, then the first.
        let named: Vec<_> = world.citizens().into_iter().filter(|&id| world.name_of(id) == *name).collect();
        app.selected =
            named.iter().copied().find(|id| world.holes_by_agent.contains_key(id)).or(named.first().copied());
        // Centring the camera promotes the agent and binds their holes.
        if let (false, Some(p)) = (args.no_autobind, app.selected.and_then(|id| world.comp::<citysim::Position>(id))) {
            app.camera.centre_on(p.tile);
        }
    }
    if args.no_autobind {
        app.bound_for = app.selected;
    }
    if args.story_tab {
        app.inspector_tab = ui::inspector::InspectorTab::Story;
    }
    if args.known_tab {
        app.inspector_tab = ui::inspector::InspectorTab::Known;
    }
    if let Some(kind) = args.select_kind {
        app.selected = world.building_of_kind(kind);
        if let Some(b) = app.selected.and_then(|id| world.comp::<citysim::Building>(id)) {
            app.camera.centre_on(b.door);
        }
        if args.corp_tab {
            // The first building of the kind owned by a corp, then its corp.
            let owned = world.buildings_of_kind(kind).iter().copied().find(|&b| world.corp_of_building(b).is_some());
            if let Some(b) = owned.and_then(|b| world.comp::<citysim::Building>(b)) {
                app.camera.centre_on(b.door);
            }
            app.selected = owned.and_then(|b| world.corp_of_building(b));
        }
    }
    app.show_districts = args.districts;
    app.show_litter = args.litter;
    app.show_hooked = args.hooked;
    app.selected_district = args.select_district.map(citysim::DistrictId);
    if let Some(d) = app.selected_district.and_then(|d| world.districts.get(d.index())) {
        // Zoomed out so most of the district shows between the panels.
        app.camera.px_per_tile = camera::MIN_PX_PER_TILE;
        app.camera.centre_on(d.centroid);
    }
    app.show_virt = args.overlay_virt;
    app.show_word = args.overlay_word;
    app.scroll_word = args.scroll_word;
    if args.select_hunter {
        // A hunter on a stake-out first, else the first hunter (ascending id).
        let staking = |h: &EntityId| {
            world
                .comp::<citysim::Brain>(*h)
                .and_then(|b| b.current_step())
                .is_some_and(|s| s.action == citysim::ActionKind::StakeOut)
        };
        let pick = world.hunts.keys().copied().find(staking).or_else(|| world.hunts.keys().next().copied());
        if let Some(h) = pick {
            app.selected = Some(h);
            app.inspector_tab = ui::inspector::InspectorTab::Known;
            if let Some(p) = world.comp::<citysim::Position>(h) {
                app.camera.px_per_tile = 12.0;
                app.camera.centre_on(p.tile);
            }
        }
    }
    // A run lasts tens of ticks: freeze the frames the M14 screenshots are taken in.
    app.paused = args.select_runner || args.select_raid || args.select_node.is_some() || args.select_hunter;
    if let Some(i) = args.select_node.filter(|&i| usize::from(i) < world.virt.nodes.len()) {
        let n = citysim::virt::NodeId(i);
        app.selected_node = Some(n);
        if let Some(node) = world.virt.node(n) {
            app.camera.px_per_tile = 14.0;
            app.camera.centre_on(node.pos);
        }
    }
    if args.select_runner {
        // The first jacked-in agent (ascending id) and its run, else the newest logged run.
        let pick = world
            .runner_of
            .iter()
            .next()
            .map(|(&a, &r)| (a, r))
            .or_else(|| world.run_log.back().map(|r| (r.runner, r.id)));
        if let Some((agent, run)) = pick {
            app.selected = Some(agent);
            app.selected_run = Some(run);
            // A live run: centred on the middle of its route; a finished one on
            // its last contest. Zoomed to read it.
            let live = world.runs.contains_key(&run);
            if let Some(r) = world.runs.get(&run).or_else(|| world.run_log.iter().find(|r| r.id == run)) {
                let nodes: Vec<citysim::virt::NodeId> = match r.log.last() {
                    Some(&(_, n, _)) if !live => vec![n],
                    _ => r.route.to_vec(),
                };
                let pts: Vec<(f32, f32)> = nodes
                    .iter()
                    .filter_map(|&n| world.virt.node(n))
                    .map(|n| (f32::from(n.pos.x), f32::from(n.pos.y)))
                    .collect();
                if !pts.is_empty() {
                    let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.0), m.1.max(p.0)));
                    let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.1), m.1.max(p.1)));
                    app.camera.px_per_tile = 12.0;
                    app.camera.centre = vec2((x0 + x1) / 2.0 + 0.5, (y0 + y1) / 2.0 + 0.5);
                }
            }
        }
    }
    if args.select_raid {
        let streamed = world.gangs().into_iter().find(|&g| mission::build(&world, g).is_some());
        app.selected_mission = streamed.or_else(|| mission::raids(&world).first().copied());
        // Centred on the raid's target.
        let door = app
            .selected_mission
            .and_then(|g| mission::target(&world, g))
            .and_then(|b| world.comp::<citysim::Building>(b))
            .map(|b| b.door);
        if let Some(door) = door {
            app.camera.px_per_tile = 10.0;
            app.camera.centre_on(door);
        }
    }
    // Screenshots with no selection frame the whole map, as `--fit` does anywhere.
    let unselected = args.select.is_none()
        && args.select_name.is_none()
        && args.select_kind.is_none()
        && args.select_district.is_none()
        && args.select_node.is_none()
        && !(args.select_hunter && app.selected.is_some())
        && !(args.select_runner && app.selected_run.is_some())
        && !(args.select_raid && app.selected_mission.is_some());
    app.fit_pending = args.fit || (args.screenshot.is_some() && unselected);
    app.notify(format!(
        "seed {} · WASD/drag pan · wheel zoom · Space pause · 1-7 speed · B districts · L litter · K hooked · N virt · J word · F5 save · F9 load",
        world.seed()
    ));

    let mut frame = 0u32;

    loop {
        // Screen size is only known inside the loop; frame the map on the first pass.
        if std::mem::take(&mut app.fit_pending) {
            app.camera.fit();
        }
        let dt = f64::from(get_frame_time());
        app.fps_avg = app.fps_avg * 0.95 + (1.0 / get_frame_time().max(1e-4)) * 0.05;

        input::handle(&mut app, &mut world);

        if !app.paused {
            app.acc += dt * BASE_TICKS_PER_SEC * f64::from(app.speed.multiplier());
        }
        let mut n = app.acc.floor() as u32;
        app.acc -= f64::from(n);
        n = n.min(MAX_TICKS_PER_FRAME);

        world.set_view(Some(app.camera.view_rect()));
        world.push_commands(&mut app.cmds);
        let started = std::time::Instant::now();
        for _ in 0..n {
            citysim::tick(&mut world);
        }
        if n > 0 {
            let rate = n as f32 / started.elapsed().as_secs_f32().max(1e-6);
            app.ticks_per_sec = if app.ticks_per_sec == 0.0 { rate } else { app.ticks_per_sec * 0.95 + rate * 0.05 };
        }

        render::draw(&world, &app);
        ui::draw(&mut app, &world);

        frame += 1;
        if let (Some(path), true) = (&args.screenshot, frame == 3) {
            get_screen_data().export_png(&path.to_string_lossy());
            eprintln!("wrote {}", path.display());
            std::process::exit(0);
        }

        next_frame().await;
    }
}
