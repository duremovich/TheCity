//! Headless runner and calibration.
//!
//! ```text
//! citysim-cli run --days N --seed S [--speed 1000] [--report]
//!                 [--lever "day=90:release_reserve=1500"]... [--save-at T]
//!                 [--load FILE] [--force-lod full|coarse|stat] [--population N]
//!                 [--map FILE] [--ambient-flats R] [--no-ambient] [--ambient-hire-far]
//! citysim-cli calibrate [--days 30] [--agents N] [--seeds 3] [--map assets/map.txt]
//!                       [--out assets/stat_table.toml] [--straight-lines]
//! citysim-cli gen-map --residents N [--seed S] --out FILE
//! citysim-cli import-graphics --src DIR --out FILE [--downsample k] [--residents N] [--crop x0,y0,x1,y1]
//! citysim-cli export-view --save FILE --out FILE [--ticks 60] [--keep N] [--at X,Y | --follow INDEX] [--half T] [--no-pin]
//! ```
//!
//! `--map` overrides `[world] map` (the M10 256 x 192 map by default; Scale
//! S3b: `--map gen:<residents>[:<seed>]` generates one, `citysim::mapgen`).
//! `calibrate` runs the city scaled to `--agents` (`Config::scaled_to`) with
//! no gangs, so a 500-agent world on the 2,000 map keeps its proportions, and
//! writes the v2 table (M10): 24 rows by phase, lawfulness and hunger bucket,
//! with the hourly outcome mix and the actor- and victim-side crime rolls.
//!
//! Exit code 0 on completion, 101 on panic.

use std::path::PathBuf;
use std::time::Instant;

mod export_view;
mod import_graphics;
mod record;
mod shadow;

use clap::{Parser, Subcommand, ValueEnum};

use citysim::{save, Config, Lod, PlayerCommand, Posture, World, TICKS_PER_DAY};

mod calibrate;
mod lever;
mod lever_parse;
mod run;

use calibrate::*;
use lever::*;
use lever_parse::*;
use run::*;

#[derive(Parser)]
#[command(name = "citysim-cli", about = "Living City Simulator headless runner")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a city headless for N days.
    Run(RunArgs),
    /// Calibrate the Statistical-LOD table (M7).
    Calibrate(CalibrateArgs),
    /// Follow individual residents through their days (diaries).
    Shadow(shadow::ShadowArgs),
    /// Generate a city map sized for N residents (Scale S3b, `citysim::mapgen`).
    GenMap(GenMapArgs),
    /// Import TheCityGraphics' generated city into the sim's map format (Scale S3b).
    ImportGraphics(import_graphics::ImportArgs),
    /// Write the camera ring's view frames from a save (`city-simview/0.1`, the city milestone C2).
    ExportView(export_view::ExportViewArgs),
}

#[derive(clap::Args)]
struct GenMapArgs {
    /// Residents the map is sized for (one 2,000-resident borough each).
    #[arg(long)]
    residents: u32,
    /// Generator seed (rotates which free cells take the Blocks and Lots).
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Output map file.
    #[arg(long)]
    out: PathBuf,
}

/// `gen-map`: write the generated map and print its building mix.
fn gen_map(args: GenMapArgs) -> Result<(), String> {
    let g = citysim::mapgen::generate(args.residents, args.seed);
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&args.out, &g.text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    println!(
        "{}: {} x {} tiles, {} boroughs, {} districts, {} El lines, {} bytes",
        args.out.display(),
        g.w,
        g.h,
        g.boroughs,
        g.districts,
        g.lines,
        g.text.len()
    );
    let per = 1000.0 / f64::from(args.residents.max(1));
    for (k, n) in &g.counts {
        println!("  {:<15} {:>6}  {:>7.2} per 1,000", format!("{k:?}"), n, *n as f64 * per);
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(args) => run(args),
        Command::Calibrate(args) => calibrate(args),
        Command::Shadow(args) => shadow::shadow(args),
        Command::GenMap(args) => gen_map(args),
        Command::ImportGraphics(args) => import_graphics::import(args),
        Command::ExportView(args) => export_view::export_view(args),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(2);
    }
}
