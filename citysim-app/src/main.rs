//! macroquad front end: squares and letters, a HUD, time controls, save/load.
//!
//! ```text
//! citysim-app [--seed N] [--load FILE] [--fps]
//! ```

mod camera;
mod input;
mod render;
mod ui;

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
    pub follow: bool,
    pub show_fps: bool,
    pub fps_avg: f32,
    pub cmds: Vec<PlayerCommand>,
    pub saves_dir: PathBuf,
    pub status: String,
    pub status_until: f64,
}

impl App {
    pub fn new(show_fps: bool) -> App {
        App {
            camera: Camera::default(),
            paused: false,
            speed: Speed::X1,
            acc: 0.0,
            selected: None,
            follow: false,
            show_fps,
            fps_avg: 60.0,
            cmds: Vec::new(),
            saves_dir: PathBuf::from("saves"),
            status: String::new(),
            status_until: 0.0,
        }
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
}

fn parse_args() -> Args {
    let mut args = Args { seed: 42, load: None, fps: false, screenshot: None, start_tick: 0 };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => args.seed = it.next().and_then(|s| s.parse().ok()).expect("--seed <N>"),
            "--load" => args.load = Some(PathBuf::from(it.next().expect("--load <FILE>"))),
            "--fps" => args.fps = true,
            "--screenshot" => args.screenshot = Some(PathBuf::from(it.next().expect("--screenshot <FILE>"))),
            "--start-tick" => args.start_tick = it.next().and_then(|s| s.parse().ok()).expect("--start-tick <N>"),
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
        None => World::new(args.seed, Config::load()),
    };
    world.run_ticks(args.start_tick);
    let mut app = App::new(args.fps);
    app.notify(format!(
        "seed {} · WASD/drag pan · wheel zoom · Space pause · 1-7 speed · F5 save · F9 load",
        world.seed()
    ));

    let mut frame = 0u32;

    loop {
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
        for _ in 0..n {
            citysim::tick(&mut world);
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
