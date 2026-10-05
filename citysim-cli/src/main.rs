//! Headless runner and calibration.
//!
//! ```text
//! citysim-cli run --days N --seed S [--speed 1000] [--report]
//!                 [--lever "day=90:release_reserve=1500"]... [--save-at T]
//!                 [--load FILE] [--force-lod full|coarse|stat] [--population N]
//!                 [--map FILE]
//! citysim-cli calibrate [--days 30] [--agents 500] [--seeds 3] [--map assets/map.txt]
//!                       [--out assets/stat_table.toml] [--straight-lines]
//! ```
//!
//! `--map` overrides `[world] map` (the M10 256 x 192 map by default).
//! `calibrate` runs the city scaled to `--agents` (`Config::scaled_to`) with
//! no gangs, so a 500-agent world on the 2,000 map keeps its proportions, and
//! writes the v2 table (M10): 24 rows by phase, lawfulness and hunger bucket,
//! with the hourly outcome mix and the actor- and victim-side crime rolls.
//!
//! Exit code 0 on completion, 101 on panic.

use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};

use citysim::{save, stats, Config, Lod, PlayerCommand, Posture, World, TICKS_PER_DAY};

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
    /// Print every event as `tick<TAB>kind<TAB>text` to stderr.
    #[arg(long)]
    events: bool,
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
    /// Starting population, overriding the config (ignored with `--load`).
    #[arg(long)]
    population: Option<u32>,
    /// Map file, overriding `[world] map` (ignored with `--load`).
    #[arg(long, value_name = "FILE")]
    map: Option<PathBuf>,
    /// Run the 300-resident v1 city (`Config::v1_profile`) instead of the
    /// configured one (ignored with `--load`).
    #[arg(long)]
    v1_profile: bool,
}

/// An absolute map path for `config.world.map` (`Config::asset` joins it onto
/// the assets directory, which an absolute path replaces).
fn map_path(p: &std::path::Path) -> Result<String, String> {
    std::fs::canonicalize(p)
        .map(|a| a.to_string_lossy().into_owned())
        .map_err(|e| format!("--map {}: {e}", p.display()))
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
    #[arg(long, default_value_t = 500)]
    agents: u32,
    #[arg(long, default_value = "assets/map.txt")]
    map: PathBuf,
    #[arg(long, default_value = "assets/stat_table.toml")]
    out: PathBuf,
    /// Cities run (seeds 1000, 1001, ...), tallied together.
    #[arg(long, default_value_t = 3)]
    seeds: u64,
    /// Print each city's day-end health (price, Treasury, stocks, thefts).
    #[arg(long)]
    health: bool,
    /// Timed straight-line walks (the M7 shortcut). Off by default since M10:
    /// on the 256 x 192 map they starve the city and cut its crime threefold.
    #[arg(long)]
    straight_lines: bool,
}

/// A scheduled lever: a ready command, or a god command naming a gang by its
/// index in `World::gangs()`, resolved to entity ids when it fires (the
/// command log records the resolved command, so replays need no resolving).
#[derive(Clone, Debug)]
enum Lever {
    Cmd(PlayerCommand),
    KillLeader(usize),
    JailGang(usize, u32),
    KillGang(usize),
    FundGang(usize, i64),
    SeizeGang(usize),
}

impl Lever {
    fn resolve(&self, world: &World) -> Result<PlayerCommand, String> {
        let gang = |i: usize| world.gangs().get(i).copied().ok_or_else(|| format!("no gang {i}"));
        Ok(match *self {
            Lever::Cmd(ref c) => c.clone(),
            Lever::KillLeader(i) => {
                let g = gang(i)?;
                let leader = world.comp::<citysim::Gang>(g).and_then(|g| g.leader);
                PlayerCommand::KillAgent(leader.ok_or_else(|| format!("gang {i} has no leader"))?)
            }
            Lever::JailGang(i, days) => PlayerCommand::JailGang { gang: gang(i)?, days },
            Lever::KillGang(i) => PlayerCommand::KillGang(gang(i)?),
            Lever::FundGang(i, amount) => PlayerCommand::FundGang { gang: gang(i)?, amount },
            Lever::SeizeGang(i) => PlayerCommand::SeizeGangTreasury(gang(i)?),
        })
    }
}

/// `day=90:release_reserve=1500` → `(tick, lever)`. God levers name a gang
/// by index: `kill_leader=0`, `jail_gang=0:60`, `kill_gang=0`,
/// `fund_gang=1:10000`, `seize_gang=0`, `fire_guards=1`, `treasury=-50000`.
fn parse_lever(spec: &str) -> Result<(u64, Lever), String> {
    let (day_part, cmd_part) =
        spec.split_once(':').ok_or_else(|| format!("{spec}: expected day=<D>:<lever>=<value>"))?;
    let day: u64 = day_part
        .strip_prefix("day=")
        .ok_or_else(|| format!("{spec}: expected day=<D>"))?
        .parse()
        .map_err(|e| format!("{spec}: bad day: {e}"))?;
    let (name, value) = cmd_part.split_once('=').ok_or_else(|| format!("{spec}: expected <lever>=<value>"))?;
    let num = |what: &str| value.parse::<f64>().map_err(|e| format!("{spec}: bad {what}: {e}"));
    let idx = |v: &str| v.parse::<usize>().map_err(|e| format!("{spec}: bad gang index: {e}"));
    // `<gang index>:<n>`
    let pair = || -> Result<(usize, i64), String> {
        let (g, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <gang index>:<n>"))?;
        Ok((idx(g)?, n.parse::<i64>().map_err(|e| format!("{spec}: bad number: {e}"))?))
    };
    let god = match name {
        "kill_leader" => Some(Lever::KillLeader(idx(value)?)),
        "kill_gang" => Some(Lever::KillGang(idx(value)?)),
        "seize_gang" => Some(Lever::SeizeGang(idx(value)?)),
        "jail_gang" => {
            let (g, d) = pair()?;
            Some(Lever::JailGang(g, u32::try_from(d).map_err(|e| format!("{spec}: bad days: {e}"))?))
        }
        "fund_gang" => {
            let (g, n) = pair()?;
            Some(Lever::FundGang(g, n))
        }
        _ => None,
    };
    if let Some(l) = god {
        return Ok((day * TICKS_PER_DAY, l));
    }
    let cmd = match name {
        "release_reserve" => PlayerCommand::ReleaseReserve { amount: num("amount")? as u32 },
        "tax_rate" => PlayerCommand::SetTaxRate(num("rate")? as f32),
        "sentence_mult" => PlayerCommand::SetSentenceMult(num("multiplier")? as f32),
        "guard_count" => PlayerCommand::SetGuardCount(num("count")? as u8),
        "immigration_per_week" => PlayerCommand::SetImmigrationPerWeek(num("count")? as u8),
        "dole_per_day" => PlayerCommand::SetDolePerDay(num("coins")? as u8),
        "law_posture" => PlayerCommand::SetLawPosture(match value.to_ascii_lowercase().as_str() {
            "auto" => None,
            "patrol" => Some(Posture::Patrol),
            "crackdown" => Some(Posture::Crackdown),
            "garrison" => Some(Posture::Garrison),
            _ => return Err(format!("{spec}: law_posture must be Auto|Patrol|Crackdown|Garrison")),
        }),
        "fire_guards" => PlayerCommand::FireAllGuards,
        "treasury" => PlayerCommand::SetTreasury(value.parse::<i64>().map_err(|e| format!("{spec}: bad coins: {e}"))?),
        other => return Err(format!("{spec}: unknown lever {other}")),
    };
    Ok((day * TICKS_PER_DAY, Lever::Cmd(cmd)))
}

fn run(args: RunArgs) -> Result<(), String> {
    let mut levers: Vec<(u64, Lever)> = args.levers.iter().map(|s| parse_lever(s)).collect::<Result<_, _>>()?;
    levers.sort_by_key(|(t, _)| *t);

    let mut config = Config::load();
    if args.v1_profile {
        config = config.v1_profile();
    }
    config.lod.force = args.force_lod.map(Lod::from);
    if let Some(n) = args.population {
        config.world.population = n;
    }
    if let Some(m) = &args.map {
        config.world.map = map_path(m)?;
    }
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
    let mut last_event_tick: Option<u64> = None;
    save_if_due(&world)?;
    while world.tick < end_tick {
        let day_start = Instant::now();
        // Chunk to the next day boundary so a mid-day `--load` still reports its first day.
        let day_end = ((world.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY).min(end_tick);
        let chunk_ticks = day_end - world.tick;
        while world.tick < day_end {
            while next_lever < levers.len() && levers[next_lever].0 <= world.tick {
                match levers[next_lever].1.resolve(&world) {
                    Ok(cmd) => world.push_command(cmd),
                    Err(e) => {
                        eprintln!("warning: lever {:?} at tick {}: {e}; skipped", levers[next_lever].1, world.tick)
                    }
                }
                next_lever += 1;
            }
            citysim::tick(&mut world);
            if args.events {
                // The story ring and the plan-abort debug ring, merged by tick.
                let fresh_of = |ring: &'_ std::collections::VecDeque<citysim::Event>| {
                    ring.iter()
                        .rev()
                        .take_while(|e| last_event_tick.is_none_or(|t| e.tick > t))
                        .cloned()
                        .collect::<Vec<_>>()
                };
                let mut both = fresh_of(&world.events);
                both.extend(fresh_of(&world.debug_events));
                both.sort_by_key(|e| std::cmp::Reverse(e.tick));
                let fresh: Vec<_> = both.iter().map(|e| (e.tick, e.kind, e.text.clone())).collect();
                for (tick, kind, text) in fresh.into_iter().rev() {
                    eprintln!("{tick}	{kind:?}	{text}");
                    last_event_tick = Some(tick);
                }
            }
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

/// Which of the five Statistical outcomes an executor state counts toward.
fn exec_category(exec: &citysim::ExecState, night: bool) -> usize {
    use citysim::ActionKind as A;
    match exec {
        citysim::ExecState::Use { kind, .. } => match kind {
            A::EatFromInventory | A::EatAtHome | A::BuyFood | A::StealFood(_) | A::Forage | A::Beg => 0,
            k if k.is_work() => 1,
            A::HaulToMarket | A::CollectWage | A::CollectDole => 1,
            A::Chat | A::Drink | A::Flirt | A::Propose => 2,
            A::Sleep => 3,
            // Resting through the night is sleep for the table's purposes
            // (energy is restored); by day Rest is the idle filler.
            A::Rest if night => 3,
            _ => 4,
        },
        _ => 4,
    }
}

/// Below the Full Earn goal's savings line (`SAVINGS_DAYS` meals of coins).
fn below_savings(world: &World, id: citysim::EntityId) -> bool {
    let price = world.local(id, citysim::BuildingKind::Market).map_or(1, |m| world.price_at(m));
    world
        .comp::<citysim::Wallet>(id)
        .is_some_and(|w| w.coins < citysim::goap::world_state::SAVINGS_DAYS.saturating_mul(price))
}

/// Per-agent tallies for the hour in progress.
#[derive(Default, Clone)]
struct HourTally {
    /// The table row the agent's hour falls in (lawfulness and hunger at the hour's start).
    row: usize,
    /// Ticks per exec category.
    ticks: [u16; 5],
    /// steal, flirt, robbed, assaulted, killed.
    extra: [u16; 5],
    /// Had a courtship candidate at the hour's start (`p_flirt` is per such hour).
    courting: bool,
}

/// Build a Full-only world (seed 1000, straight-line walks, gangless, the
/// city scaled to `--agents`), run it, and write the v2 table: per row
/// (phase x lawfulness bucket x hunger bucket, at the start of each
/// agent-hour) the frequency of the dominant exec category, and of hours in
/// which the agent stole, flirted (or proposed), was robbed, assaulted or
/// killed.
fn calibrate(args: CalibrateArgs) -> Result<(), String> {
    use citysim::{
        Brain, Corpse, DeathCause, EventKind, MemoryKind, Needs, Personality, StatRow, StatTable, STAT_ROWS,
    };
    use std::collections::BTreeMap;
    let mut config = Config::load().calibration_city(args.agents);
    config.world.map = map_path(&args.map)?;
    config.lod.force = Some(Lod::Full);
    config.exec.straight_line_paths = args.straight_lines;
    // Only the bucket edges are read from this header.
    let header = StatTable {
        version: 2,
        seed: 1000,
        days: args.days,
        agents: args.agents,
        lawfulness_edges: [0.3, 0.7],
        hunger_edge: 0.4,
        p_dole_day: 1.0,
        rows: Vec::new(),
    };
    let mut counts = [[0u64; 5]; STAT_ROWS];
    let mut extra = [[0u64; 5]; STAT_ROWS];
    let mut denom = [0u64; STAT_ROWS];
    let mut court_denom = [0u64; STAT_ROWS];
    let (mut dole_days, mut dole_taken) = (0u64, 0u64);
    let open_hour = |world: &World, hour: &mut BTreeMap<citysim::EntityId, HourTally>| {
        hour.clear();
        for id in world.citizens() {
            if !world.has::<Brain>(id) {
                continue;
            }
            let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
            let hunger = world.comp::<Needs>(id).map_or(1.0, |n| n.hunger);
            let courting = citysim::systems::social::known_candidate(world, id, 0.3).is_some();
            hour.insert(
                id,
                HourTally { row: header.index(world.phase(), law, hunger), courting, ..Default::default() },
            );
        }
    };
    let total_ticks = args.days * TICKS_PER_DAY;
    let t0 = Instant::now();
    // Several seeds, one tally: a single 30-day city varies by a quarter in
    // its theft rate from seed to seed (M10).
    for seed in 1000..1000 + args.seeds {
        let mut world = World::new(seed, config.clone());
        let mut hour: BTreeMap<citysim::EntityId, HourTally> = BTreeMap::new();
        let mut hour_start = world.tick;
        let mut poor_today: (u64, Vec<citysim::EntityId>) = (u64::MAX, Vec::new());
        let mut cursor = world.next_event_id;
        open_hour(&world, &mut hour);
        for _ in 0..total_ticks {
            citysim::tick(&mut world);
            let night = world.phase() == citysim::DayPhase::Night;
            let just = world.tick - 1;
            for (&id, t) in hour.iter_mut() {
                let Some(brain) = world.comp::<Brain>(id) else { continue };
                t.ticks[exec_category(&brain.exec, night)] += 1;
                if let citysim::ExecState::Use {
                    kind: citysim::ActionKind::Flirt | citysim::ActionKind::Propose,
                    started,
                    ..
                } = brain.exec
                {
                    if started == just {
                        t.extra[1] += 1;
                    }
                }
            }
            // New ring entries since the cursor.
            let first = world.events.front().map_or(0, |e| e.id);
            for e in world.events.iter().skip(cursor.saturating_sub(first) as usize) {
                let hit = |k: usize, who: Option<&citysim::EntityId>, hour: &mut BTreeMap<_, HourTally>| {
                    if let Some(t) = who.and_then(|w| hour.get_mut(w)) {
                        t.extra[k] += 1;
                    }
                };
                match e.kind {
                    // Every theft but a starving one (the Statistical eat path's
                    // desperation theft models those; M10, replaces D29's
                    // lawless-and-hungry rows).
                    EventKind::Theft => {
                        let thief = e.actors.first().copied();
                        if thief.is_some_and(|a| world.comp::<Needs>(a).is_some_and(|n| n.hunger > 0.0)) {
                            hit(0, thief.as_ref(), &mut hour);
                        }
                    }
                    EventKind::Assault => hit(3, e.actors.get(1), &mut hour),
                    EventKind::Death => {
                        let violent = e
                            .actors
                            .first()
                            .and_then(|&a| world.comp::<Corpse>(a))
                            .is_some_and(|c| c.cause == DeathCause::Violence);
                        if violent {
                            hit(4, e.actors.first(), &mut hour);
                        }
                    }
                    _ => {}
                }
            }
            cursor = world.next_event_id;
            if world.tick.is_multiple_of(citysim::TICKS_PER_HOUR) {
                // Close the hour: first-hand robberies, then every agent of the
                // hour (the dead included) into its row.
                for (&id, t) in hour.iter_mut() {
                    let robbed = world.comp::<citysim::Memory>(id).is_some_and(|m| {
                        m.entries
                            .iter()
                            .any(|e| e.kind == MemoryKind::WasRobbed && !e.second_hand && e.tick >= hour_start)
                    });
                    if robbed {
                        t.extra[2] += 1;
                    }
                }
                for t in hour.values() {
                    let c = t.ticks;
                    let dominant =
                        if c[0] > 0 { 0 } else { (1..5).max_by_key(|&k| (c[k], std::cmp::Reverse(k))).unwrap_or(4) };
                    counts[t.row][dominant] += 1;
                    denom[t.row] += 1;
                    court_denom[t.row] += u64::from(t.courting);
                    for (k, (sum, &n)) in extra[t.row].iter_mut().zip(&t.extra).enumerate() {
                        // A flirt counts in the hours it had a candidate for.
                        if k != 1 || t.courting {
                            *sum += u64::from(n.min(1));
                        }
                    }
                }
                hour_start = world.tick;
                open_hour(&world, &mut hour);
                // The Work phase opens: which jobless adults are below their
                // savings line (the Full Earn goal's gate) today?
                if world.phase() == citysim::DayPhase::Work && poor_today.0 != world.day() {
                    poor_today = (world.day(), Vec::new());
                    for id in world.citizens() {
                        if world.has::<Brain>(id)
                            && !world.has::<citysim::Job>(id)
                            && citysim::systems::demography::is_adult(&world, id)
                            && below_savings(&world, id)
                        {
                            poor_today.1.push(id);
                        }
                    }
                }
            }
            // Day end: did the poor collect the dole?
            if let Some(r) =
                world.stats.history.back().filter(|_| args.health && world.tick.is_multiple_of(TICKS_PER_DAY))
            {
                let homes = world
                    .buildings_of_kind(citysim::BuildingKind::Home)
                    .iter()
                    .filter(|&&h| {
                        world.comp::<citysim::Building>(h).is_some_and(|b| {
                            b.occupants
                                .iter()
                                .any(|&o| world.comp::<citysim::Household>(o).is_some_and(|hh| hh.home == Some(h)))
                        })
                    })
                    .count();
                let farms: u32 = world
                    .buildings_of_kind(citysim::BuildingKind::Farm)
                    .iter()
                    .filter_map(|&f| world.comp::<citysim::Building>(f))
                    .map(|b| b.stock_food)
                    .sum();
                eprintln!(
                    "seed {seed} day {}: price {} treasury {} warehouse {} market {} pantry {} farms {farms} employed {} thefts {} starved {} occupied homes {homes}",
                    r.day, r.price, r.treasury, r.food_warehouse, r.food_market, r.food_pantry, r.employed, r.thefts, r.deaths_starvation
                );
            }
            if world.tick.is_multiple_of(TICKS_PER_DAY) {
                let day = world.day() - 1;
                if poor_today.0 == day {
                    for &id in &poor_today.1 {
                        dole_days += 1;
                        dole_taken += u64::from(world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day == Some(day)));
                    }
                }
            }
        }
    }
    // The five independent rolls are pooled over the two hunger buckets of a
    // phase and lawfulness: a Statistical agent eats as soon as it is hungry
    // and spends a sixth of a Full agent's hours in the hungry bucket, so
    // hunger-conditioned crime rates would not transfer.
    // `p_flirt` is per hour with a courtship candidate (a Full agent only
    // flirts when it has one, and the Statistical roll needs one too).
    let pooled = |i: usize| -> ([u64; 5], u64, u64) {
        let (a, b) = (i & !1, i | 1);
        let x: [u64; 5] = std::array::from_fn(|k| extra[a][k] + extra[b][k]);
        (x, denom[a] + denom[b], court_denom[a] + court_denom[b])
    };
    let mut rows: Vec<StatRow> = (0..STAT_ROWS)
        .map(|i| {
            let n = denom[i].max(1) as f32;
            let c = counts[i];
            let (x, pn, cn) = pooled(i);
            let (pn, cn) = (pn.max(1) as f32, cn.max(1) as f32);
            StatRow {
                label: StatTable::label(i),
                p_eat: c[0] as f32 / n,
                p_work: c[1] as f32 / n,
                p_social: c[2] as f32 / n,
                p_sleep: c[3] as f32 / n,
                p_steal: x[0] as f32 / pn,
                p_flirt: x[1] as f32 / cn,
                p_robbed: x[2] as f32 / pn,
                p_assaulted: x[3] as f32 / pn,
                p_killed: x[4] as f32 / pn,
            }
        })
        .collect();
    // An empty row borrows the same phase and lawfulness row's other hunger bucket.
    for i in 0..STAT_ROWS {
        if denom[i] == 0 {
            let twin = i ^ 1;
            rows[i] = if denom[twin] > 0 {
                StatRow { label: StatTable::label(i), ..rows[twin].clone() }
            } else {
                StatRow { label: StatTable::label(i), ..Default::default() }
            };
        }
    }
    let p_dole_day = (dole_taken as f32 / dole_days.max(1) as f32).min(1.0);
    let table = StatTable { rows, p_dole_day, ..header };
    let body = toml::to_string(&table).map_err(|e| e.to_string())?;
    let text = format!(
        "# generated by calibrate v2: seeds 1000..={}, {} days, {} agents, map {}, gangless, {} walks, DO NOT EDIT\n{body}",
        1000 + args.seeds - 1,
        args.days,
        args.agents,
        args.map.display(),
        if args.straight_lines { "straight-line" } else { "pathed" }
    );
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&args.out, text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    eprintln!(
        "calibrated {} agents over {} days in {:.1}s -> {} (agent-hours per row: {:?})",
        args.agents,
        args.days,
        t0.elapsed().as_secs_f32(),
        args.out.display(),
        denom
    );
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(args) => run(args),
        Command::Calibrate(args) => calibrate(args),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(2);
    }
}
