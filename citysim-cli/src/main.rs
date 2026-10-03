//! Headless runner and calibration.
//!
//! ```text
//! citysim-cli run --days N --seed S [--speed 1000] [--report]
//!                 [--lever "day=90:release_reserve=1500"]... [--save-at T]
//!                 [--load FILE] [--force-lod full|coarse|stat]
//! citysim-cli calibrate [--days 30] [--agents 200] [--out assets/stat_table.toml]
//! ```
//!
//! Exit code 0 on completion, 101 on panic.

use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};

use citysim::{save, stats, Config, Lod, PlayerCommand, World, TICKS_PER_DAY};

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
}

#[derive(clap::Args)]
struct RunArgs {
    /// In-game days to simulate.
    #[arg(long)]
    days: u64,
    /// World seed.
    #[arg(long)]
    seed: u64,
    /// Accepted for parity with the app; headless runs always go flat out.
    #[arg(long, default_value_t = 1000)]
    speed: u32,
    /// Print DailyStats as CSV, one row per day, header first.
    #[arg(long)]
    report: bool,
    /// Schedule a lever: `day=<D>:<lever>=<value>`. Repeatable.
    #[arg(long = "lever", value_name = "SPEC")]
    levers: Vec<String>,
    /// Write `saves/<seed>-<tick>.ron` when this tick is reached.
    #[arg(long, value_name = "TICK")]
    save_at: Option<u64>,
    /// Resume from a save instead of a fresh world.
    #[arg(long, value_name = "FILE")]
    load: Option<PathBuf>,
    /// Force every agent to one LOD.
    #[arg(long, value_enum)]
    force_lod: Option<ForceLod>,
    /// Directory for `--save-at`.
    #[arg(long, default_value = "saves")]
    saves_dir: PathBuf,
}

#[derive(Copy, Clone, ValueEnum)]
enum ForceLod {
    Full,
    Coarse,
    Stat,
}

impl From<ForceLod> for Lod {
    fn from(f: ForceLod) -> Lod {
        match f {
            ForceLod::Full => Lod::Full,
            ForceLod::Coarse => Lod::Coarse,
            ForceLod::Stat => Lod::Statistical,
        }
    }
}

#[derive(clap::Args)]
struct CalibrateArgs {
    #[arg(long, default_value_t = 30)]
    days: u64,
    #[arg(long, default_value_t = 200)]
    agents: u32,
    #[arg(long, default_value = "assets/stat_table.toml")]
    out: PathBuf,
}

/// `day=90:release_reserve=1500` → `(tick, command)`.
fn parse_lever(spec: &str) -> Result<(u64, PlayerCommand), String> {
    let (day_part, cmd_part) =
        spec.split_once(':').ok_or_else(|| format!("{spec}: expected day=<D>:<lever>=<value>"))?;
    let day: u64 = day_part
        .strip_prefix("day=")
        .ok_or_else(|| format!("{spec}: expected day=<D>"))?
        .parse()
        .map_err(|e| format!("{spec}: bad day: {e}"))?;
    let (name, value) = cmd_part.split_once('=').ok_or_else(|| format!("{spec}: expected <lever>=<value>"))?;
    let num = |what: &str| value.parse::<f64>().map_err(|e| format!("{spec}: bad {what}: {e}"));
    let cmd = match name {
        "release_reserve" => PlayerCommand::ReleaseReserve { amount: num("amount")? as u32 },
        "tax_rate" => PlayerCommand::SetTaxRate(num("rate")? as f32),
        "sentence_mult" => PlayerCommand::SetSentenceMult(num("multiplier")? as f32),
        "guard_count" => PlayerCommand::SetGuardCount(num("count")? as u8),
        "immigration_per_week" => PlayerCommand::SetImmigrationPerWeek(num("count")? as u8),
        "dole_per_day" => PlayerCommand::SetDolePerDay(num("coins")? as u8),
        other => return Err(format!("{spec}: unknown lever {other}")),
    };
    Ok((day * TICKS_PER_DAY, cmd))
}

fn run(args: RunArgs) -> Result<(), String> {
    let mut levers: Vec<(u64, PlayerCommand)> = args.levers.iter().map(|s| parse_lever(s)).collect::<Result<_, _>>()?;
    levers.sort_by_key(|(t, _)| *t);

    let mut config = Config::load();
    config.lod.force = args.force_lod.map(Lod::from);
    let mut world = match &args.load {
        Some(path) => {
            let mut w = save::load_from_file(path)?;
            w.config.lod.force = config.lod.force;
            w.config.assets_dir = config.assets_dir;
            w
        }
        None => World::new(args.seed, config),
    };
    let _ = args.speed;

    // Levers are absolute ticks. After a `--load`, anything already in the
    // past is in the save's command log; firing it again would double-apply.
    let start_tick = world.tick;
    let end_tick = start_tick + args.days * TICKS_PER_DAY;
    let (stale, live): (Vec<_>, Vec<_>) = levers.into_iter().partition(|(t, _)| *t < start_tick);
    for (t, cmd) in &stale {
        eprintln!("warning: lever {cmd:?} at tick {t} is before the start tick {start_tick}; skipped");
    }
    let levers = live;
    if let Some(t) = args.save_at {
        if t < start_tick || t > end_tick {
            return Err(format!("--save-at {t} is outside this run's tick range {start_tick}..={end_tick}"));
        }
    }
    let save_if_due = |world: &World| -> Result<(), String> {
        if args.save_at == Some(world.tick) {
            let path = save::save_path(&args.saves_dir, world.seed(), world.tick);
            save::save_to_file(world, &path).map_err(|e| format!("save {}: {e}", path.display()))?;
            eprintln!("saved {}", path.display());
        }
        Ok(())
    };

    if args.report {
        println!("{}", stats::CSV_HEADER);
    }

    let mut next_lever = 0;
    save_if_due(&world)?;
    while world.tick < end_tick {
        let day_start = Instant::now();
        // Chunk to the next day boundary so a mid-day `--load` still reports its first day.
        let day_end = ((world.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY).min(end_tick);
        let chunk_ticks = day_end - world.tick;
        while world.tick < day_end {
            while next_lever < levers.len() && levers[next_lever].0 <= world.tick {
                world.push_command(levers[next_lever].1.clone());
                next_lever += 1;
            }
            citysim::tick(&mut world);
            save_if_due(&world)?;
        }
        let secs = day_start.elapsed().as_secs_f32().max(1e-9);
        if args.report && world.tick % TICKS_PER_DAY == 0 {
            if let Some(row) = world.stats.history.back_mut() {
                row.ticks_per_sec = chunk_ticks as f32 / secs;
                println!("{}", row.csv_row());
            }
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(args) => run(args),
        Command::Calibrate(args) => Err(format!(
            "calibrate is not implemented until M7 (requested {} days, {} agents → {})",
            args.days,
            args.agents,
            args.out.display()
        )),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(2);
    }
}
