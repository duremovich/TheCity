use super::*;

#[derive(clap::Args)]
pub(super) struct RunArgs {
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
    /// With `--events`: a daily `Diag` line per Market (owner, price in
    /// tenths, yesterday's sales, stock) and per corp (treasury, closing,
    /// buildings, niches, levels, order, sold contracts).
    #[arg(long)]
    diag: bool,
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
    /// With `--population`: scale the configured city to it
    /// (`Config::scaled_to`: jobs, stocks, treasuries, levers), not only the
    /// head count (Scale S1's probe).
    #[arg(long, requires = "population")]
    scaled: bool,
    /// Override `[world] residents_per_home` (ignored with `--load`).
    #[arg(long, value_name = "N")]
    rph: Option<u32>,
    /// Map file, overriding `[world] map` (ignored with `--load`).
    #[arg(long, value_name = "FILE")]
    map: Option<PathBuf>,
    /// The pre-view pass phase 5 (measurement): write every Full or Coarse
    /// GoTo leg of `--legs-min` minutes or more to FILE (TSV, `exec::leglog`).
    #[arg(long, value_name = "FILE")]
    legs: Option<PathBuf>,
    #[arg(long, default_value_t = 120)]
    legs_min: u64,
    /// Run the 300-resident v1 city (`Config::v1_profile`) instead of the
    /// configured one (ignored with `--load`).
    #[arg(long)]
    v1_profile: bool,
    /// Start under the dole city's policy (`Config::dole_city`, P10: the
    /// levy and the safety net enacted from day 0; ignored with `--load`).
    #[arg(long)]
    dole_city: bool,
    /// The Statistical tier's policy, overriding `[lod] policy`: `table` or
    /// `mlp` (experiment; ignored with `--load`).
    #[arg(long, value_name = "POLICY")]
    stat_policy: Option<String>,
    /// Scale S5: Ambient flats per household slot on a map without `flats=`
    /// (`[ambient] flats_per_slot`; ignored with `--load`).
    #[arg(long, value_name = "R")]
    ambient_flats: Option<f32>,
    /// Scale S5: no Ambient tier (`[ambient] enabled = false`; ignored with `--load`).
    #[arg(long)]
    no_ambient: bool,
    /// Scale S5: `[ambient] hire_far` on (ignored with `--load`).
    #[arg(long)]
    ambient_hire_far: bool,
    /// Scale S5 (measurement): a `MEM <day> <peak working set MB> <working
    /// set MB>` line on stderr at each day's end (Windows; 0 elsewhere).
    #[arg(long)]
    mem: bool,
    /// With the `prof` feature: zero the profile at the start of this day,
    /// so the printed profile is the run's tail (Scale S1).
    #[arg(long, value_name = "DAY")]
    prof_from: Option<u64>,
    /// With the `prof` feature: profile each day's midnight tick alone, a
    /// `PROFDAY <day> <tick of day>` line then its `PROF` lines (the perf
    /// pass phase 7; phase 7b: every tick of the staggered midnight, each
    /// alone; the run's own profile then holds only the last of them).
    #[arg(long)]
    prof_midnight: bool,
    /// With the `prof` feature: profile this tick of day alone, each day, a
    /// `PROFDAY <day> <tick of day>` line then its `PROF` lines (the view
    /// milestone V2: the 21:00 tick is `--prof-tick 1260`).
    #[arg(long, value_name = "TICK_OF_DAY")]
    prof_tick: Option<u64>,
    /// The view milestone V2: a `SLOW <day> <tick of day> <ms>` line on
    /// stderr for every `citysim::tick` longer than this many wall ms (the
    /// view stalls on one; `max_tick_ms` shows only the day's longest).
    #[arg(long, value_name = "MS")]
    slow_ticks: Option<f64>,
    /// The perf pass phase 8: threads scoring the think slice, overriding
    /// `[perf] threads` (a host setting, not saved: kept with `--load`).
    #[arg(long, value_name = "N")]
    threads: Option<usize>,
}

/// An absolute map path for `config.world.map` (`Config::asset` joins it onto
/// the assets directory, which an absolute path replaces).
/// Scale S3b: `gen:<residents>[:<seed>]` passes through (the generator).
pub(super) fn map_path(p: &std::path::Path) -> Result<String, String> {
    if let Some(spec) = p.to_str().filter(|s| s.starts_with("gen:")) {
        return Ok(spec.to_string());
    }
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

pub(super) fn run(args: RunArgs) -> Result<(), String> {
    let mut levers: Vec<(u64, Lever)> = args.levers.iter().map(|s| parse_lever(s)).collect::<Result<_, _>>()?;
    levers.sort_by_key(|(t, _)| *t);

    let mut config = Config::load();
    if args.v1_profile {
        config = config.v1_profile();
    }
    if args.dole_city {
        config = config.dole_city();
    }
    config.lod.force = args.force_lod.map(Lod::from);
    if let Some(n) = args.population {
        if args.scaled {
            config = config.scaled_to(n);
        } else {
            config.world.population = n;
        }
    }
    if let Some(r) = args.rph {
        config.world.residents_per_home = r;
    }
    if let Some(m) = &args.map {
        config.world.map = map_path(m)?;
    }
    if let Some(p) = &args.stat_policy {
        config.lod.policy = p.clone();
    }
    if let Some(r) = args.ambient_flats {
        config.ambient.flats_per_slot = r;
    }
    if args.no_ambient {
        config.ambient.enabled = false;
    }
    if args.ambient_hire_far {
        config.ambient.hire_far = true;
    }
    if let Some(n) = args.threads {
        if n == 0 {
            return Err("--threads must be 1 or more".into());
        }
        config.perf.threads = n;
    }
    let mut world = match &args.load {
        Some(path) => {
            let mut w = save::load_from_file(path)?;
            w.config.lod.force = config.lod.force;
            // The host's threads; the midnight stagger (phase 7b) is the
            // save's own.
            w.config.perf.threads = config.perf.threads;
            w.config.assets_dir = config.assets_dir;
            w
        }
        None => {
            let t0 = Instant::now();
            let w = World::new(args.seed, config);
            if cfg!(feature = "prof") {
                eprintln!("PROF world_new {:.2}s", t0.elapsed().as_secs_f64());
            }
            w
        }
    };
    let _ = args.speed;
    if args.prof_tick.is_some_and(|t| t >= TICKS_PER_DAY) {
        return Err(format!(
            "--prof-tick {} is past the day's last tick {}",
            args.prof_tick.unwrap_or(0),
            TICKS_PER_DAY - 1
        ));
    }
    if args.legs.is_some() {
        world.leg_log = Some(citysim::exec::leglog::LegLog { min_ticks: args.legs_min, ..Default::default() });
    }

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
    let save_if_due = |world: &mut World| -> Result<(), String> {
        if args.save_at == Some(world.tick) {
            // C3a: the live fights end before the save (engagements are not saved).
            citysim::systems::combat::settle_all(world);
            let path = save::save_path(&args.saves_dir, world.seed(), world.tick);
            save::save_to_file(world, &path).map_err(|e| format!("save {}: {e}", path.display()))?;
            eprintln!("saved {}", path.display());
        }
        Ok(())
    };

    if args.report {
        // Perf pass phase 5: two wall-clock columns after `ticks_per_sec`, the
        // day's longest single tick (ms) and its tick of day. CLI-only (not in
        // `DailyStats`, not saved); like `ticks_per_sec`, noise to every
        // identity comparison (`tools/phase_measure.py` masks them).
        println!(
            "{},max_tick_ms,max_tick_at,edges,edges_new,coworker_pairs,friends,exec_hq_tiles,exec_hq_max",
            citysim::systems::stats::csv_header(&world)
        );
    }

    // The perf pass phase 6 (the slope): the social graph's size at the
    // day's end, the edges made and the Statistical co-worker pair-hours
    // bonded that day (`World::edges_made`, `World::coworker_pairs`, not
    // saved: a loaded world counts from its load, as these do). Phase 7c
    // (the crew cap, OPEN-205): `friends`, the Friend edges at the day's end
    // (a walk of the edges once a day; CLI-only like the rest).
    let mut made_at = (world.edges_made, world.coworker_pairs);
    let mut next_lever = 0;
    let mut last_event_tick: Option<u64> = None;
    let mut role_tally: std::collections::BTreeMap<(String, String), u32> = std::collections::BTreeMap::new();
    // M12 fix pass (throughput): `--events` writes through one buffer flushed
    // once a day; an unbuffered line per event (1,000-1,500 a day, mostly
    // PlanAborted) cost a fifth of a day's ticks and made the dip days.
    use std::io::Write;
    let mut events_out = std::io::BufWriter::with_capacity(1 << 16, std::io::stderr());
    save_if_due(&mut world)?;
    while world.tick < end_tick {
        if args.prof_from.is_some_and(|d| d * TICKS_PER_DAY == world.tick) {
            citysim::prof::reset();
        }
        let day_start = Instant::now();
        // The day's longest `citysim::tick` (wall ms) and its tick of day.
        let mut max_tick = (0.0f64, 0u64);
        // Chunk to the next day boundary so a mid-day `--load` still reports its first day.
        let day_end = ((world.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY).min(end_tick);
        let chunk_ticks = day_end - world.tick;
        while world.tick < day_end {
            while next_lever < levers.len() && levers[next_lever].0 <= world.tick {
                match levers[next_lever].1.resolve_all(&world) {
                    Ok(cmds) => {
                        for cmd in cmds {
                            world.push_command(cmd);
                        }
                    }
                    Err(e) => {
                        eprintln!("warning: lever {:?} at tick {}: {e}; skipped", levers[next_lever].1, world.tick)
                    }
                }
                next_lever += 1;
            }
            let tick_of_day = world.tick % TICKS_PER_DAY;
            let tick_start = (args.report || args.slow_ticks.is_some()).then(Instant::now);
            // The perf pass phase 7b: each tick of the staggered midnight alone
            // (tick 0 alone with the stagger off), its tick of day after the day.
            let prof_this = (args.prof_midnight && tick_of_day < citysim::systems::midnight::window(&world))
                || args.prof_tick == Some(tick_of_day);
            if prof_this {
                citysim::prof::reset();
            }
            citysim::tick(&mut world);
            if prof_this {
                eprintln!("PROFDAY {} {tick_of_day}", world.tick / TICKS_PER_DAY + 1);
                citysim::prof::dump();
            }
            if let Some(t) = tick_start {
                let ms = t.elapsed().as_secs_f64() * 1e3;
                if args.slow_ticks.is_some_and(|s| ms > s) {
                    eprintln!("SLOW {} {tick_of_day} {ms:.0}", world.tick / TICKS_PER_DAY + 1);
                }
                if ms > max_tick.0 {
                    max_tick = (ms, tick_of_day);
                }
            }
            citysim::exec::leglog::leader_tick(&mut world);
            citysim::exec::leglog::ride_tick(&mut world);
            citysim::exec::leglog::walk_tick(&mut world);
            if world.leg_log.is_some() && world.tick % TICKS_PER_DAY == 180 {
                citysim::exec::leglog::night_census(&mut world);
            }
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
                // Round 7 W4 (OPEN-245): the install's doer and the hire's
                // foreman by role (diagnostic; the `#role` lines).
                for e in &both {
                    let who = match e.kind {
                        citysim::EventKind::Installed if e.text.contains(" installed at ") => {
                            Some(e.actors.get(3).copied())
                        }
                        citysim::EventKind::Hire => Some(e.actors.get(2).copied()),
                        // Round 8 X3 (OPEN-247): the thief's role (a body's
                        // theft; the gang's off-screen one names nobody).
                        citysim::EventKind::VehicleStolen => {
                            e.actors.first().copied().filter(|&t| t != citysim::EntityId::NONE).map(Some)
                        }
                        _ => None,
                    };
                    // Round 8 X3: a hire with a record (`Misery.record_at`),
                    // by the post's role.
                    if e.kind == citysim::EventKind::Hire {
                        let h = e.actors.first().copied().unwrap_or(citysim::EntityId::NONE);
                        if world.comp::<citysim::components::Misery>(h).is_some_and(|m| m.record_at > 0) {
                            let label = world
                                .comp::<citysim::components::Job>(h)
                                .map_or_else(|| "no job".to_string(), |j| j.role.label().to_string());
                            *role_tally.entry(("HireWithRecord".to_string(), label)).or_insert(0u32) += 1;
                        }
                    }
                    if let Some(who) = who {
                        let label = match who {
                            None => "none".to_string(),
                            Some(w) => world
                                .comp::<citysim::components::Job>(w)
                                .map_or_else(|| "no job".to_string(), |j| j.role.label().to_string()),
                        };
                        *role_tally.entry((format!("{:?}", e.kind), label)).or_insert(0u32) += 1;
                    }
                }
                let fresh: Vec<_> = both.iter().map(|e| (e.tick, e.kind, e.text.clone())).collect();
                for (tick, kind, text) in fresh.into_iter().rev() {
                    let _ = writeln!(events_out, "{tick}	{kind:?}	{text}");
                    last_event_tick = Some(tick);
                }
            }
            save_if_due(&mut world)?;
        }
        let _ = events_out.flush();
        if args.diag && args.events && world.tick % TICKS_PER_DAY == 0 {
            print_diag(&world);
        }
        let secs = day_start.elapsed().as_secs_f32().max(1e-9);
        if args.mem && world.tick % TICKS_PER_DAY == 0 {
            let (peak, now) = working_set_mb();
            eprintln!("MEM	{}	{peak:.0}	{now:.0}", world.tick / TICKS_PER_DAY);
        }
        if args.report && world.tick % TICKS_PER_DAY == 0 {
            if let Some(row) = world.stats.history.back_mut() {
                row.ticks_per_sec = chunk_ticks as f32 / secs;
                // The perf pass phase 10 (I9): how far the execs live from
                // their Hq (`exec_hq_spread`; CLI-only like the rest).
                let (hq_mean, hq_max) = exec_hq_spread(&world);
                let row = world.stats.history.back().expect("the row above");
                println!(
                    "{},{:.1},{},{},{},{},{},{hq_mean:.1},{hq_max}",
                    row.csv_row(),
                    max_tick.0,
                    max_tick.1,
                    world.edges.len(),
                    world.edges_made - made_at.0,
                    world.coworker_pairs - made_at.1,
                    world.edges.values().filter(|e| e.kind == citysim::RelKind::Friend).count()
                );
                made_at = (world.edges_made, world.coworker_pairs);
            }
        }
    }
    citysim::prof::dump();
    // The pressure pass phase 5b: where the gangs' coins went (diagnostic).
    if args.report {
        for ((kind, role), n) in &role_tally {
            let _ = writeln!(events_out, "#role	{kind}	{role}	{n}");
        }
        // The city milestone C6b: the vacant units by district at the run's end.
        for line in citysim::systems::floors::report(&world)
            .into_iter()
            .chain(citysim::systems::levels::report(&world))
            .chain(citysim::systems::vertical::report(&world))
        {
            let _ = writeln!(events_out, "{line}");
        }
        for (k, v) in &world.probe.gang_in {
            let _ = writeln!(events_out, "#gang_flow	in	{k}	{v}");
        }
        for (k, v) in &world.probe.gang_out {
            let _ = writeln!(events_out, "#gang_flow	out	{k}	{v}");
        }
        for g in world.gangs() {
            if let Some(x) = world.comp::<citysim::components::Gang>(g) {
                let _ = writeln!(
                    events_out,
                    "#gang_book	{}	{} members	{}c	{:?}	in {}",
                    x.name,
                    x.members.len(),
                    x.treasury,
                    x.order,
                    world.probe.gang_in_by.get(&g).copied().unwrap_or(0)
                );
                for ((_, k), v) in world.probe.gang_out_by.range((g, String::new())..).take_while(|((x, _), _)| *x == g)
                {
                    let _ = writeln!(events_out, "#gang_book	{}	out	{k}	{v}", x.name);
                }
            }
        }
        let _ = events_out.flush();
    }
    if let Some(p) = &args.legs {
        std::fs::write(p, citysim::exec::leglog::tsv(&world)).map_err(|e| format!("--legs {}: {e}", p.display()))?;
    }
    Ok(())
}

/// `--mem`: the process's peak and current working set, MB (Windows; 0
/// elsewhere), as `tests/big_island.rs` reads it.
#[cfg(windows)]
fn working_set_mb() -> (f64, f64) {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_non_paged: usize,
        quota_non_paged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(process: *mut std::ffi::c_void, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Counters::default() };
    // SAFETY: a plain Win32 query into a correctly sized, owned struct.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok == 0 {
        (0.0, 0.0)
    } else {
        (c.peak_working_set as f64 / 1_048_576.0, c.working_set as f64 / 1_048_576.0)
    }
}

#[cfg(not(windows))]
fn working_set_mb() -> (f64, f64) {
    (0.0, 0.0)
}

/// `--diag`: one `Diag` event line per Market and per corp at the day's start.
fn print_diag(world: &World) {
    let tick = world.tick;
    for &m in world.buildings_of_kind(citysim::BuildingKind::Market) {
        let owner = world.owner_label(world.owner_of(m));
        let sold = world.comp::<citysim::Market>(m).and_then(|mk| mk.sales.back().copied()).unwrap_or(0);
        let stock = world.comp::<citysim::Building>(m).map_or(0, |b| b.stock_food);
        eprintln!(
            "{tick}	Diag	market {} owner={owner} tenths={} sold={sold} stock={stock}",
            m.index,
            world.price_tenths_at(m)
        );
    }
    for c in world.corps() {
        let Some(cc) = world.comp::<citysim::Corp>(c) else { continue };
        let niches: Vec<String> = cc.niches.iter().map(|n| format!("{n}:{:.2}", cc.level(*n))).collect();
        let sold = citysim::systems::corp_brain::sold_contracts(world, cc, c);
        eprintln!(
            "{tick}	Diag	corp {} treasury={} closing={} buildings={} niches={} order={:?} contracts={sold}/{}",
            cc.name.replace(' ', "_"),
            cc.treasury,
            cc.closing,
            cc.buildings.len(),
            niches.join(","),
            cc.order,
            cc.contracts.len()
        );
    }
}

/// Which of the five Statistical outcomes an executor state counts toward.
pub(super) fn exec_category(exec: &citysim::ExecState, night: bool) -> usize {
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
pub(super) fn below_savings(world: &World, id: citysim::EntityId) -> bool {
    let price = world.local(id, citysim::BuildingKind::Market).map_or(1, |m| world.price_for(m, id));
    world
        .comp::<citysim::Wallet>(id)
        .is_some_and(|w| w.coins < citysim::goap::world_state::SAVINGS_DAYS.saturating_mul(price))
}

/// Parity (arrests, commute rides): the El's boardings in the all-Full
/// calibration city, read tick by tick.
#[derive(Default)]
pub(super) struct ElTally {
    /// Aboard (`ExecState::Ride`) at the last tick.
    riding: std::collections::BTreeSet<citysim::EntityId>,
    /// `(agent, day)`: rode to work today, and the ride Home after it not yet seen.
    to_work: std::collections::BTreeSet<(citysim::EntityId, u64)>,
    back_due: std::collections::BTreeSet<(citysim::EntityId, u64)>,
    /// A commute under way (`World::commute_start`): the Home-to-work
    /// model's answer at its start (`None` homeless), and whether it rode.
    commuting: std::collections::BTreeMap<citysim::EntityId, (Option<bool>, bool)>,
    /// `(agent, day)` with its first commute arrival seen.
    arrived: std::collections::BTreeSet<(citysim::EntityId, u64)>,
}

impl ElTally {
    /// One tick: the commutes' model answers and arrivals into `cells`
    /// (`[model walks, model rides][walked, rode]`); returns the agents who
    /// boarded on a non-commute ride (anything but the first ride to work of
    /// the day and the first ride Home after it).
    pub(super) fn step(&mut self, world: &World, cells: &mut [[u64; 2]; 2]) -> Vec<citysim::EntityId> {
        use citysim::{Building, Household, Job};
        let day = world.day();
        let employer = |id| world.comp::<Job>(id).and_then(|j| j.employer);
        let home = |id| world.comp::<Household>(id).and_then(|h| h.home);
        let door = |b: Option<citysim::EntityId>| b.and_then(|b| world.comp::<Building>(b)).map(|b| b.door);
        let now: std::collections::BTreeSet<citysim::EntityId> = world
            .rides
            .keys()
            .copied()
            .filter(|&id| {
                world.comp::<citysim::Brain>(id).is_some_and(|b| matches!(b.exec, citysim::ExecState::Ride { .. }))
            })
            .collect();
        let mut errands = Vec::new();
        for &id in now.difference(&self.riding) {
            let target = world.rides[&id].target.building;
            if target.is_some() && target == employer(id) {
                if let Some(c) = self.commuting.get_mut(&id) {
                    c.1 = true;
                }
                if self.to_work.insert((id, day)) {
                    self.back_due.insert((id, day));
                    continue;
                }
            } else if target.is_some() && target == home(id) && self.back_due.remove(&(id, day)) {
                continue;
            }
            errands.push(id);
        }
        self.riding = now;
        // Commutes: started (the model's answer from the Home door now) and
        // arrived (at the employer, the day's first).
        for &id in world.commute_start.keys() {
            self.commuting.entry(id).or_insert_with(|| {
                let model = match (door(home(id)), door(employer(id))) {
                    (Some(h), Some(w)) => Some(citysim::systems::transit::choose(world, id, h, w, world.tick).is_ok()),
                    _ => None,
                };
                (model, false)
            });
        }
        let ended: Vec<citysim::EntityId> =
            self.commuting.keys().copied().filter(|id| !world.commute_start.contains_key(id)).collect();
        for id in ended {
            let Some((model, rode)) = self.commuting.remove(&id) else { continue };
            let at = world.comp::<citysim::Position>(id).and_then(|p| p.building);
            if at.is_some() && at == employer(id) && self.arrived.insert((id, day)) {
                if let Some(m) = model {
                    cells[usize::from(m)][usize::from(rode)] += 1;
                }
            }
        }
        errands
    }
}

/// Per-agent tallies for the hour in progress.
#[derive(Default, Clone)]
pub(super) struct HourTally {
    /// The table row the agent's hour falls in (lawfulness and hunger at the hour's start).
    pub(super) row: usize,
    /// Ticks per exec category.
    pub(super) ticks: [u16; 5],
    /// steal, flirt, robbed, assaulted, killed.
    pub(super) extra: [u16; 5],
    /// Had a courtship candidate at the hour's start (`p_flirt` is per such hour).
    pub(super) courting: bool,
    /// New edges this hour with anyone but a co-worker (`p_meet`).
    pub(super) met: u16,
    /// Chats begun this hour, and those with a housemate (`p_chat_home`).
    pub(super) chats: u16,
    pub(super) chats_home: u16,
    /// `stat_policy::features` at the hour's start (`--rows-csv` only).
    pub(super) features: Option<[f32; citysim::systems::stat_policy::N_FEATURES]>,
    /// The no-net income path: a jobless adult who is broke
    /// (`life::broke`) at the hour's start (`p_scavenge`'s and `p_beg`'s
    /// hours), and the Scavenges and Begs it began this hour.
    pub(super) earn: bool,
    /// Shadow V4 bug 1: short (`life::earn_short`) but not broke: the
    /// `p_scavenge_short` / `p_beg_short` hours.
    pub(super) short: bool,
    pub(super) scav: u16,
    pub(super) beg: u16,
    /// An adult below the meal price at the hour's start who was hungry
    /// (below the table's hunger edge) at some tick of the hour
    /// (`p_desperate`'s hours), and its thefts while hungry.
    pub(super) broke: bool,
    pub(super) hungry: bool,
    pub(super) desperate: u16,
    /// Hunger as sickness § 5 (`[sickness] bins` on): an adult who cannot
    /// buy a meal with a bin in reach at the hour's start (`bins::dive_target`;
    /// `p_dive`'s hours), and the dives it began this hour.
    pub(super) dive_ok: bool,
    pub(super) dives: u16,
    /// Parity (arrests): an adult who could pay the El's fare or would jump
    /// the gate at the hour's start (`p_errand_ride`'s hours), and its
    /// non-commute boardings this hour.
    pub(super) ride_ok: bool,
    pub(super) errands: u16,
    /// The agent's bin meals today at the last tick (`Health.today.dive`).
    pub(super) dive_meals: u8,
    /// Parity (`p_meal`): hungry at the hour's start (below the
    /// table's hunger edge), the hunger last seen, and a meal this hour.
    pub(super) meal_seen: bool,
    pub(super) meal_cell: usize,
    pub(super) last_hunger: f32,
    pub(super) ate: bool,
}
