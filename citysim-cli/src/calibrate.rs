use super::*;

#[derive(clap::Args)]
pub(super) struct CalibrateArgs {
    #[arg(long, default_value_t = 30)]
    days: u64,
    /// Residents (`[world] population` when absent). The recalibration
    /// (2026-10-09) moved the default from 500: a 500-resident city on the
    /// 2,000 map has a quarter of the police per street and four times the
    /// corps' places per resident (see `Config::calibration_city`).
    #[arg(long)]
    agents: Option<u32>,
    /// The map the table is learned on. The view milestone V9 (OPEN-255):
    /// where bodies ride (the 20k island, `assets/maps/isle_m_k2.txt`, with
    /// `--agents 20000`): on the 2,000 map nobody rides and the table's
    /// ride rates came out 0.
    #[arg(long, default_value = "assets/map.txt")]
    map: PathBuf,
    /// Ambient flats per household slot on a map without `flats=`, as
    /// `run --ambient-flats` (`[ambient] flats_per_slot`).
    #[arg(long, value_name = "R")]
    ambient_flats: Option<f32>,
    #[arg(long, default_value = "assets/stat_table.toml")]
    out: PathBuf,
    /// Cities run (seeds 1000, 1001, ...), tallied together.
    #[arg(long, default_value_t = 3)]
    seeds: u64,
    /// The first seed (`--rows-csv` runs split the seeds over processes).
    #[arg(long, default_value_t = 1000)]
    first_seed: u64,
    /// Print each city's day-end health (price, Treasury, stocks, thefts).
    #[arg(long)]
    health: bool,
    /// Timed straight-line walks (the M7 shortcut). Off by default since M10:
    /// on the 256 x 192 map they starve the city and cut its crime threefold.
    #[arg(long)]
    straight_lines: bool,
    /// Also write one CSV row per Full agent-hour (the extended features at
    /// the hour's start, the outcome and the rolls that fired): the training
    /// data of the learned-policy experiment (`systems::stat_policy`).
    #[arg(long, value_name = "FILE")]
    rows_csv: Option<PathBuf>,
}

/// Build a Full-only world (seed 1000, straight-line walks, gangless, the
/// city scaled to `--agents`), run it, and write the v2 table: per row
/// (phase x lawfulness bucket x hunger bucket, at the start of each
/// agent-hour) the frequency of the dominant exec category, and of hours in
/// which the agent stole, flirted (or proposed), was robbed, assaulted or
/// killed.
pub(super) fn calibrate(args: CalibrateArgs) -> Result<(), String> {
    use citysim::{
        Brain, Corpse, DeathCause, EventKind, MemoryKind, Needs, Personality, StatRow, StatTable, STAT_ROWS,
    };
    use std::collections::BTreeMap;
    let loaded = Config::load();
    let agents = args.agents.unwrap_or(loaded.world.population);
    let mut config = loaded.calibration_city(agents);
    config.world.map = map_path(&args.map)?;
    if let Some(r) = args.ambient_flats {
        config.ambient.flats_per_slot = r;
    }
    config.lod.force = Some(Lod::Full);
    config.exec.straight_line_paths = args.straight_lines;
    // Only the bucket edges are read from this header.
    let header = StatTable {
        version: 2,
        seed: 1000,
        days: args.days,
        agents,
        lawfulness_edges: [0.3, 0.7],
        hunger_edge: 0.4,
        p_dole_day: 1.0,
        p_theft_caught: None,
        p_scavenge: None,
        p_beg: None,
        p_scavenge_short: None,
        p_beg_short: None,
        p_desperate: None,
        p_steal_broke: None,
        p_dive: None,
        p_meal: None,
        p_attend: None,
        p_wage_ratio: None,
        p_dodge_caught: None,
        p_commute_ride: None,
        p_errand_ride: None,
        p_pitch: None,
        p_venue: None,
        pantry_per_resident: None,
        rows: Vec::new(),
    };
    // Jobs & lives phase 7b: the adult-days (read at the evening's tick)
    // and the Full tier's paid visits per venue kind (`p_venue`).
    let mut adult_days = 0u64;
    let mut venue_paid: BTreeMap<String, u64> = BTreeMap::new();
    // Jobs & lives (R5): the Full pitch-hours due and those worked (`p_pitch`).
    let (mut pitch_due, mut pitch_worked) = (0u64, 0u64);
    let mut counts = [[0u64; 5]; STAT_ROWS];
    let mut extra = [[0u64; 5]; STAT_ROWS];
    let mut denom = [0u64; STAT_ROWS];
    let mut court_denom = [0u64; STAT_ROWS];
    // New non-co-worker edge ends per row, for `p_meet`.
    let mut met = [0u64; STAT_ROWS];
    let mut chats = [[0u64; 2]; STAT_ROWS];
    let (mut dole_days, mut dole_taken) = (0u64, 0u64);
    // Every Theft, every Arrest, and the arrests of a thief (`p_theft_caught`).
    let (mut thefts_all, mut arrests, mut theft_arrests) = (0u64, 0u64, 0u64);
    // The no-net income path (flip readiness): written only in a no-net
    // city, so the dole city's table is unchanged. Per row: earning
    // agent-hours with a Scavenge, with a Beg, and all earning agent-hours;
    // per lawfulness bucket: thefts by a hungry thief below the meal price,
    // and the hungry agent-hours below the meal price.
    // The recalibration (2026-10-09): the income path and p_desperate are
    // learned in every city (the dole city's off-screen broke scavenge, beg
    // and go hungry as its Full ones do).
    let (mut scav, mut beg, mut scav_denom) = ([0u64; STAT_ROWS], [0u64; STAT_ROWS], [0u64; STAT_ROWS]);
    let (mut scav_s, mut beg_s, mut short_denom) = ([0u64; STAT_ROWS], [0u64; STAT_ROWS], [0u64; STAT_ROWS]);
    let (mut dive, mut dive_denom) = ([0u64; STAT_ROWS], [0u64; STAT_ROWS]);
    // Parity (the law1 actor share): thefts (not `p_desperate`'s) by a thief
    // broke at the hour's start, and the broke agent-hours (`p_steal_broke`).
    let (mut steal_broke, mut broke_denom) = ([0u64; STAT_ROWS], [0u64; STAT_ROWS]);
    // Parity (arrests): non-commute boardings per ride-able agent-hour
    // (`p_errand_ride`); (commute rides) the first commutes of a workday by
    // the Home-to-work model's answer at the start, and those that rode
    // (`p_commute_ride`).
    let (mut errand, mut errand_denom) = ([0u64; STAT_ROWS], [0u64; STAT_ROWS]);
    let mut commute_cells = [[0u64; 2]; 2];
    // P5f: per role, free healthy workday shift minutes, and those at the work.
    let mut attend: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    // P5f: wages owed (a free, healthy workday shift's wage at its start) and paid.
    let (mut wage_owed, mut wage_paid) = (0i64, 0i64);
    // Transit phase 3 (parity): the gates jumped and the Fare Evasion
    // sentences they drew (`p_dodge_caught`).
    let (mut dodges, mut dodges_caught) = (0u64, 0u64);
    let mut theft_arrested: std::collections::BTreeSet<citysim::EntityId> = std::collections::BTreeSet::new();
    // Diet parity: the pantry meals per resident, a daily mean.
    let (mut pantry_sum, mut pantry_days) = (0f64, 0u64);
    let (mut eat_h, mut eat_h_denom) =
        ([0u64; citysim::world::MEAL_CELLS_SPLIT], [0u64; citysim::world::MEAL_CELLS_SPLIT]);
    let (mut desp, mut desp_denom) = ([0u64; 3], [0u64; 3]);
    let with_rows = args.rows_csv.is_some();
    let mut rows_out = match &args.rows_csv {
        Some(path) => {
            use std::io::Write;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut w = std::io::BufWriter::new(f);
            let names = citysim::systems::stat_policy::FEATURE_NAMES.join(",");
            writeln!(
                w,
                "seed,tick,agent,row,{names},outcome,courting,steal,flirt,robbed,assaulted,killed,met,chats,chats_home"
            )
            .map_err(|e| e.to_string())?;
            Some(w)
        }
        None => None,
    };
    let open_hour = |world: &World, hour: &mut BTreeMap<citysim::EntityId, HourTally>| {
        hour.clear();
        for id in world.citizens() {
            if !world.has::<Brain>(id) {
                continue;
            }
            let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
            let hunger = world.comp::<Needs>(id).map_or(1.0, |n| n.hunger);
            let courting = citysim::systems::social::known_candidate(world, id, 0.3).is_some();
            let features = with_rows.then(|| citysim::systems::stat_policy::features(world, id));
            let adult = citysim::systems::demography::is_adult(world, id);
            // Parity (the bin meals red): the Full Scavenge's and Beg's gate
            // (`life::broke`, the planner's `ctx.poor`), as `lod::stat_earn` reads it.
            let earn = adult && !world.has::<citysim::Job>(id) && citysim::systems::life::broke(world, id);
            let short =
                adult && !world.has::<citysim::Job>(id) && !earn && citysim::systems::life::earn_short(world, id);
            let broke = adult
                && world.comp::<citysim::Wallet>(id).map_or(0, |w| w.coins)
                    < citysim::systems::life::meal_price(world, id);
            let dive_ok = citysim::systems::bins::dive_target(world, id).is_some();
            // Review item 3: the hours a body could board (`can_board_here`,
            // the off-screen errand ride's gate).
            let ride_ok = adult
                && world.config.transit.enabled
                && citysim::systems::transit::pays_or_dodges(world, id).is_some()
                && citysim::systems::transit::can_board_here(world, id);
            hour.insert(
                id,
                HourTally {
                    row: header.index(world.phase(), law, hunger),
                    courting,
                    features,
                    earn,
                    short,
                    broke,
                    dive_ok,
                    ride_ok,
                    dive_meals: world.comp::<citysim::Health>(id).map_or(0, |h| h.today.dive),
                    meal_seen: citysim::systems::lod::has_means(world, id),
                    meal_cell: header.meal_index_split(
                        world.phase(),
                        hunger,
                        world.comp::<citysim::Job>(id).is_some_and(|j| j.on_shift(world.tick_of_day())),
                    ),
                    last_hunger: hunger,
                    ..Default::default()
                },
            );
        }
    };
    let total_ticks = args.days * TICKS_PER_DAY;
    let t0 = Instant::now();
    // Several seeds, one tally: a single 30-day city varies by a quarter in
    // its theft rate from seed to seed (M10).
    for seed in args.first_seed..args.first_seed + args.seeds {
        let mut world = World::new(seed, config.clone());
        theft_arrested.clear();
        let mut hour: BTreeMap<citysim::EntityId, HourTally> = BTreeMap::new();
        let mut hour_start = world.tick;
        let mut poor_today: (u64, Vec<citysim::EntityId>) = (u64::MAX, Vec::new());
        let mut el = ElTally::default();
        let mut cursor = world.next_event_id;
        open_hour(&world, &mut hour);
        let mut known: std::collections::BTreeSet<_> = world.edges.keys().copied().collect();
        for _ in 0..total_ticks {
            citysim::tick(&mut world);
            if world.config.transit.enabled {
                for id in el.step(&world, &mut commute_cells) {
                    if let Some(t) = hour.get_mut(&id) {
                        t.errands += 1;
                    }
                }
            }
            let night = world.phase() == citysim::DayPhase::Night;
            let just = world.tick - 1;
            // Jobs & lives phase 7b: the evening's eligible agents (the
            // Statistical evening's tick, `leisure::stat_daily`).
            if just % TICKS_PER_DAY == 1260 {
                // Lives at work gate (OPEN-106): the adults the Statistical
                // evening holds, without the bodies (`lod::kept_body`).
                for id in world.citizens() {
                    if world.has::<Brain>(id)
                        && citysim::systems::demography::is_adult(&world, id)
                        && !citysim::systems::lod::kept_body(&world, id)
                    {
                        adult_days += 1;
                    }
                }
            }
            // P5f: the shift minutes and those at the work (`p_attend`).
            {
                let tod = world.tick_of_day();
                if tod == 0 {
                    wage_paid += world.stats.history.back().map_or(0, |r| r.flow_wages);
                    dodges += world.stats.history.back().map_or(0, |r| u64::from(r.transit.fare_dodges));
                    let (mut st, mut rs) = (0u64, 0u64);
                    for &h in world.buildings_of_kind(citysim::BuildingKind::Home) {
                        let r = world.residents_of(h).len() as u64;
                        if r > 0 {
                            rs += r;
                            st += world.comp::<citysim::Building>(h).map_or(0, |b| u64::from(b.stock_food));
                        }
                    }
                    pantry_sum += st as f64 / rs.max(1) as f64;
                    pantry_days += 1;
                }
                for &id in hour.keys() {
                    let Some(j) = world.comp::<citysim::Job>(id) else { continue };
                    if !j.on_shift(tod)
                        || !citysim::exec::routine::workday_of(&world, id, j, j.shift_key_at(world.tick))
                        || world.has::<citysim::Sentence>(id)
                        || citysim::systems::health::sick(&world, id)
                    {
                        continue;
                    }
                    // Once a shift: a night shift split at midnight
                    // ((1260, 1440), (0, 360)) starts once, at 21:00 (review
                    // fix: its 00:00 segment counted the wage twice).
                    let wraps = j.shifts.iter().any(|&(_, e)| e == 1440);
                    if j.shifts.iter().any(|&(s, _)| s == tod && !(s == 0 && wraps)) {
                        wage_owed += j.wage_per_day;
                    }
                    let Some(b) = world.comp::<Brain>(id) else { continue };
                    // At the work: a work step under Work, or any step of a
                    // guard's Patrol or Arrest (the beat is walked).
                    let at_work = match b.current_goal {
                        Some(citysim::GoalKind::Work) => matches!(b.exec, citysim::ExecState::Use { .. }),
                        Some(citysim::GoalKind::Patrol | citysim::GoalKind::Arrest) => true,
                        _ => false,
                    };
                    let e = attend.entry(format!("{:?}", j.role)).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += u64::from(at_work);
                    // Transit T14 (plan D9): the same minutes by commute band
                    // (`"{role:?}/b{band}"`, rows with 600+ minutes kept).
                    if let Some(b) = citysim::systems::transit::commute_band(&world, id) {
                        let e = attend.entry(format!("{:?}/b{b}", j.role)).or_insert((0, 0));
                        e.0 += 1;
                        e.1 += u64::from(at_work);
                    }
                }
            }
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
                if t.earn || t.short {
                    match brain.exec {
                        citysim::ExecState::Use { kind: citysim::ActionKind::Scavenge, started, .. }
                            if started == just =>
                        {
                            t.scav += 1
                        }
                        citysim::ExecState::Use { kind: citysim::ActionKind::Beg, started, .. } if started == just => {
                            t.beg += 1
                        }
                        _ => {}
                    }
                }
                if t.meal_seen {
                    let h = world.comp::<Needs>(id).map_or(1.0, |n| n.hunger);
                    if h > t.last_hunger + 0.05 {
                        t.ate = true;
                    }
                    t.last_hunger = h;
                }
                // Diet parity: a dive counts when it fed the diver (a bin
                // meal in the agent's day tally), as the off-screen roll's
                // `bins::dive` eats at once (a begun dive can lose the bin).
                if t.dive_ok {
                    let d = world.comp::<citysim::Health>(id).map_or(0, |h| h.today.dive);
                    if d > t.dive_meals {
                        t.dives += 1;
                    }
                    t.dive_meals = d;
                }
                if t.broke && !t.hungry {
                    t.hungry = world.comp::<Needs>(id).is_some_and(|n| n.hunger < header.hunger_edge);
                }
                if let citysim::ExecState::Use { kind: citysim::ActionKind::Chat, started, .. } = brain.exec {
                    if started == just {
                        let home = |x| world.comp::<citysim::Household>(x).and_then(|h| h.home);
                        let partner = brain.current_step().and_then(|s| s.target);
                        t.chats += 1;
                        t.chats_home += u16::from(home(id).is_some() && partner.is_some_and(|p| home(p) == home(id)));
                    }
                }
            }
            // New ring entries since the cursor.
            let first = world.events.front().map_or(0, |e| e.id);
            for e in world.events.iter().skip(cursor.saturating_sub(first) as usize) {
                if e.kind == EventKind::Sentence {
                    let thief = e.actors.first().is_some_and(|a| theft_arrested.remove(a));
                    if e.text.contains("for Fare Evasion") {
                        dodges_caught += 1;
                        // A thief taken at the gate was sentenced for the gate,
                        // not the theft: not `p_theft_caught`'s arrest (the first
                        // El table read 1.0 against 0.82 without the El).
                        theft_arrests -= u64::from(thief);
                    }
                }
                // Pressure pass round 2, R1: a guard's catch at the gate (the
                // fine or the beating, `systems::fares`) is the jumped gate caught.
                if e.kind == EventKind::Fare && e.actors.len() > 1 && e.text.contains("for Fare Evasion") {
                    dodges_caught += 1;
                }
                let hit = |k: usize, who: Option<&citysim::EntityId>, hour: &mut BTreeMap<_, HourTally>| {
                    if let Some(t) = who.and_then(|w| hour.get_mut(w)) {
                        t.extra[k] += 1;
                    }
                };
                match e.kind {
                    // Every theft but a starving one (the Statistical eat path's
                    // desperation theft models those; M10, replaces D29's
                    // lawless-and-hungry rows).
                    EventKind::Arrest => {
                        arrests += 1;
                        let suspect = e.actors.get(1).copied();
                        let thief = world
                            .crime_reports()
                            .iter()
                            .any(|r| Some(r.suspect) == suspect && r.crime == citysim::Crime::Theft);
                        theft_arrests += u64::from(thief);
                        if let (true, Some(s)) = (thief, suspect) {
                            theft_arrested.insert(s);
                        }
                    }
                    EventKind::Theft => {
                        thefts_all += 1;
                        let thief = e.actors.first().copied();
                        let hunger = thief.and_then(|a| world.comp::<Needs>(a)).map(|n| n.hunger);
                        // A no-net city: a theft for the meal the thief cannot
                        // buy (below the meal price at the hour's start,
                        // hungry now) is `p_desperate`'s, not `p_steal`'s.
                        let need = thief.and_then(|a| hour.get(&a)).is_some_and(|t| t.broke)
                            && hunger.is_some_and(|h| h < header.hunger_edge);
                        if need {
                            if let Some(t) = thief.and_then(|a| hour.get_mut(&a)) {
                                t.desperate += 1;
                            }
                        } else if hunger.is_some_and(|h| h > 0.0) {
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
                // Jobs & lives (R5): the hour's pitches due and worked.
                for p in world.jobs2.pitches.values() {
                    if citysim::systems::streetwork::due(&world, p) {
                        pitch_due += 1;
                        if citysim::systems::streetwork::working(&world, p) {
                            pitch_worked += 1;
                        }
                    }
                }
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
                // The hour's first meetings: edges that were not there at its
                // start. Co-workers are left out (the Statistical shift
                // drifts its co-workers itself).
                let employer = |id| world.comp::<citysim::Job>(id).and_then(|j| j.employer);
                for &(a, b) in world.edges.keys().filter(|k| !known.contains(k)) {
                    if employer(a).is_some() && employer(a) == employer(b) {
                        continue;
                    }
                    for x in [a, b] {
                        if let Some(t) = hour.get_mut(&x) {
                            t.met += 1;
                        }
                    }
                }
                known = world.edges.keys().copied().collect();
                for (&id, t) in hour.iter() {
                    let c = t.ticks;
                    let dominant =
                        if c[0] > 0 { 0 } else { (1..5).max_by_key(|&k| (c[k], std::cmp::Reverse(k))).unwrap_or(4) };
                    if let (Some(w), Some(f)) = (rows_out.as_mut(), t.features.as_ref()) {
                        use std::io::Write;
                        let mut line = format!("{seed},{hour_start},{},{}", id.index, t.row);
                        for v in f {
                            line.push_str(&format!(",{v}"));
                        }
                        let x = t.extra.map(|n| n.min(1));
                        line.push_str(&format!(
                            ",{dominant},{},{},{},{},{},{},{},{},{}\n",
                            u8::from(t.courting),
                            x[0],
                            x[1],
                            x[2],
                            x[3],
                            x[4],
                            t.met,
                            t.chats,
                            t.chats_home
                        ));
                        w.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
                    }
                    counts[t.row][dominant] += 1;
                    denom[t.row] += 1;
                    if t.earn {
                        scav[t.row] += u64::from(t.scav.min(1));
                        beg[t.row] += u64::from(t.beg.min(1));
                        scav_denom[t.row] += 1;
                    }
                    if t.short {
                        scav_s[t.row] += u64::from(t.scav.min(1));
                        beg_s[t.row] += u64::from(t.beg.min(1));
                        short_denom[t.row] += 1;
                    }
                    if t.broke {
                        steal_broke[t.row] += u64::from(t.extra[0].min(1));
                        broke_denom[t.row] += 1;
                    }
                    if t.ride_ok {
                        errand[t.row] += u64::from(t.errands.min(1));
                        errand_denom[t.row] += 1;
                    }
                    if t.dive_ok {
                        dive[t.row] += u64::from(t.dives.min(1));
                        dive_denom[t.row] += 1;
                    }
                    if t.meal_seen {
                        eat_h[t.meal_cell] += u64::from(t.ate);
                        eat_h_denom[t.meal_cell] += 1;
                    }
                    if t.broke && t.hungry {
                        desp[(t.row % 6) / 2] += u64::from(t.desperate.min(1));
                        desp_denom[(t.row % 6) / 2] += 1;
                    }
                    court_denom[t.row] += u64::from(t.courting);
                    met[t.row] += u64::from(t.met);
                    chats[t.row][0] += u64::from(t.chats);
                    chats[t.row][1] += u64::from(t.chats_home);
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
        // Lives at work gate (OPEN-106): the evening's own visits, without
        // the bodies' (they go out on screen in either city, on top of the
        // rate) and the world-side passes both tiers run (`status_daily`):
        // `CoinProbe::venue_unmirrored`.
        for (k, v) in &world.probe.venue_paid {
            let out = world.probe.venue_unmirrored.get(k).copied().unwrap_or(0);
            *venue_paid.entry(k.clone()).or_default() += v.0.saturating_sub(out);
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
                // Each Statistical meeting gives both agents an edge: half the
                // ends per agent-hour, pooled over hunger.
                p_meet: {
                    let (a, b) = (i & !1, i | 1);
                    (met[a] + met[b]) as f32 / 2.0 / pn
                },
                p_chat: {
                    let (a, b) = (i & !1, i | 1);
                    (chats[a][0] + chats[b][0]) as f32 / pn
                },
                p_chat_home: {
                    let (a, b) = (i & !1, i | 1);
                    let all = chats[a][0] + chats[b][0];
                    if all == 0 {
                        0.0
                    } else {
                        (chats[a][1] + chats[b][1]) as f32 / all as f32
                    }
                },
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
    if let Some(mut w) = rows_out {
        use std::io::Write;
        w.flush().map_err(|e| e.to_string())?;
    }
    let p_dole_day = (dole_taken as f32 / dole_days.max(1) as f32).min(1.0);
    let p_theft_caught = Some((theft_arrests as f32 / thefts_all.max(1) as f32).min(1.0));
    eprintln!("thefts {thefts_all}, arrests {arrests}, of a thief {theft_arrests}");
    let rate = |n: u64, d: u64| (n as f32 / d.max(1) as f32).min(1.0);
    eprintln!("desperate {desp:?} of {desp_denom:?} hungry broke hours");
    let p_desperate = Some(std::array::from_fn(|i| rate(desp[i], desp_denom[i])));
    eprintln!("scavenge {scav:?} beg {beg:?} of {scav_denom:?} earning hours");
    let p_scavenge = Some(std::array::from_fn(|i| rate(scav[i], scav_denom[i])));
    let p_beg = Some(std::array::from_fn(|i| rate(beg[i], scav_denom[i])));
    eprintln!("short: scavenge {scav_s:?} beg {beg_s:?} of {short_denom:?} hours");
    let p_scavenge_short = Some(std::array::from_fn(|i| rate(scav_s[i], short_denom[i])));
    let p_beg_short = Some(std::array::from_fn(|i| rate(beg_s[i], short_denom[i])));
    // Parity (the law1 actor share): `p_steal_broke`, pooled over the two
    // hunger buckets as the rows' `p_steal`; a pair under 200 broke hours
    // takes the mean over all rows.
    eprintln!("thefts by the broke {steal_broke:?} of {broke_denom:?} broke hours");
    let p_steal_broke = {
        let (n, d): (u64, u64) = (steal_broke.iter().sum(), broke_denom.iter().sum());
        let mean = (n as f32 / d.max(1) as f32).min(1.0);
        Some(std::array::from_fn(|i| {
            let (a, b) = (i & !1, i | 1);
            let (n, d) = (steal_broke[a] + steal_broke[b], broke_denom[a] + broke_denom[b]);
            if d < 200 {
                mean
            } else {
                (n as f32 / d as f32).min(1.0)
            }
        }))
    };
    // Parity (arrests, commute rides): the El's two rates, with it on.
    let p_errand_ride = config.transit.enabled.then(|| {
        eprintln!("El: non-commute boardings {errand:?} of {errand_denom:?} ride-able hours");
        std::array::from_fn(|i| rate(errand[i], errand_denom[i]))
    });
    let p_commute_ride = config.transit.enabled.then(|| {
        eprintln!(
            "El: first commutes [walked, rode] by the model's walk {:?} ride {:?}",
            commute_cells[0], commute_cells[1]
        );
        std::array::from_fn(|m| rate(commute_cells[m][1], commute_cells[m][0] + commute_cells[m][1]))
    });
    // Hunger as sickness § 5: `p_dive` with the bins on. A row with under
    // 200 eligible hours takes the mean over all rows (plan risk: noisy rows).
    let p_dive = (config.sickness.diet && config.sickness.bins).then(|| {
        eprintln!("bins: dives {dive:?} of {dive_denom:?} eligible hours");
        let (n, d): (u64, u64) = (dive.iter().sum(), dive_denom.iter().sum());
        let mean = (n as f32 / d.max(1) as f32).min(1.0);
        std::array::from_fn(
            |i| if dive_denom[i] < 200 { mean } else { (dive[i] as f32 / dive_denom[i] as f32).min(1.0) },
        )
    });
    // Parity: the hungry hour's meal, every city (a row under 200 hungry
    // hours takes the all-row mean).
    eprintln!("hungry hours with a meal {eat_h:?} of {eat_h_denom:?}");
    let p_meal = {
        let (n, d): (u64, u64) = (eat_h.iter().sum(), eat_h_denom.iter().sum());
        let mean = (n as f32 / d.max(1) as f32).min(1.0);
        Some(
            (0..citysim::world::MEAL_CELLS_SPLIT)
                .map(|i| {
                    // A thin cell takes its pooled (unsplit) cell's rate, else the mean.
                    let j = i % citysim::world::MEAL_CELLS;
                    let (pn, pd) = (
                        eat_h[j] + eat_h[j + citysim::world::MEAL_CELLS],
                        eat_h_denom[j] + eat_h_denom[j + citysim::world::MEAL_CELLS],
                    );
                    if eat_h_denom[i] >= 200 {
                        (eat_h[i] as f32 / eat_h_denom[i] as f32).min(1.0)
                    } else if pd >= 200 {
                        (pn as f32 / pd as f32).min(1.0)
                    } else {
                        mean
                    }
                })
                .collect::<Vec<f32>>(),
        )
    };

    eprintln!("attendance (role: worked of shift minutes) {attend:?}");
    // P5f: Full pays a cut shift past half pro rata and a shift worked whole
    // in full, so the wage share runs above the attendance share: the ratio
    // of the two scales the Statistical pro rata wage.
    // The roles' own rows only (the banded rows count the same minutes again).
    let (worked, minutes) =
        attend.iter().filter(|(k, _)| !k.contains('/')).fold((0u64, 0u64), |(w, n), (_, &(n2, w2))| (w + w2, n + n2));
    let attend_all = worked as f32 / minutes.max(1) as f32;
    let paid_share = wage_paid as f32 / wage_owed.max(1) as f32;
    let p_wage_ratio = Some(if attend_all > 0.0 { paid_share / attend_all } else { 1.0 });
    eprintln!("wages paid {wage_paid} of owed {wage_owed} ({paid_share:.3}); attendance {attend_all:.3}; ratio {p_wage_ratio:?}");
    let p_attend = Some(
        attend
            .iter()
            // Transit T14: a banded cell needs a hundred 9-hour shifts' minutes
            // (54,000; a role's own row 600): the first banded table read
            // Reporter/b2 0.0 off 20,520 minutes, about one reporter.
            .filter(|(k, &(n, _))| n >= if k.contains('/') { 54_000 } else { 600 })
            .map(|(r, &(n, w))| (r.clone(), (w as f32 / n as f32).min(1.0)))
            .collect::<BTreeMap<String, f32>>()
            .into(),
    );
    let table = StatTable {
        rows,
        p_dole_day,
        p_theft_caught,
        p_scavenge,
        p_beg,
        p_scavenge_short,
        p_beg_short,
        p_desperate,
        p_steal_broke,
        p_dive,
        p_meal,
        p_attend,
        p_wage_ratio,
        p_commute_ride,
        p_errand_ride,
        p_dodge_caught: (dodges > 0).then(|| (dodges_caught as f64 / dodges as f64).min(1.0) as f32),
        p_pitch: (pitch_due > 0).then(|| (pitch_worked as f64 / pitch_due as f64).min(1.0) as f32),
        p_venue: (adult_days > 0)
            .then(|| venue_paid.iter().map(|(k, &n)| (k.clone(), (n as f64 / adult_days as f64) as f32)).collect()),
        pantry_per_resident: (pantry_days > 0).then(|| (pantry_sum / pantry_days as f64) as f32),
        ..header
    };
    let body = toml::to_string(&table).map_err(|e| e.to_string())?;
    let text = format!(
        "# generated by calibrate v2: seeds {}..={}, {} days, {} agents, map {}, gangless, riot- and asset-free, {} walks, DO NOT EDIT\n{body}",
        args.first_seed,
        args.first_seed + args.seeds - 1,
        args.days,
        agents,
        args.map.display(),
        if args.straight_lines { "straight-line" } else { "pathed" }
    );
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&args.out, text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    eprintln!(
        "calibrated {} agents over {} days in {:.1}s -> {} (agent-hours per row: {:?})",
        agents,
        args.days,
        t0.elapsed().as_secs_f32(),
        args.out.display(),
        denom
    );
    eprintln!("new non-co-worker edge ends per row {met:?}");
    eprintln!("chats (all, with a housemate) per row {chats:?}");
    Ok(())
}

/// The perf pass phase 10 (ISLAND_PASS I9): the mean and the largest
/// Manhattan distance, Home door to Hq door (`life::hq_of`), over the living
/// corp execs who have both; `(0, 0)` with none.
pub(super) fn exec_hq_spread(world: &citysim::World) -> (f64, u32) {
    use citysim::systems::{classes, life, missions};
    let mut d: Vec<u32> = Vec::new();
    for e in classes::exec_set(world) {
        let Some(corp) = life::exec_corp(world, e) else { continue };
        let Some(hq) = life::hq_of(world, corp, e).and_then(|b| world.comp::<citysim::components::Building>(b)) else {
            continue;
        };
        let Some(home) = missions::home_of(world, e).and_then(|h| world.comp::<citysim::components::Building>(h))
        else {
            continue;
        };
        d.push(home.door.manhattan(hq.door));
    }
    if d.is_empty() {
        return (0.0, 0);
    }
    (d.iter().map(|&x| f64::from(x)).sum::<f64>() / d.len() as f64, d.iter().copied().max().unwrap_or(0))
}
