use super::*;

// ---------------------------------------------------------------------------
// Lives at work phase 0 (`shadow --lives`; docs/LIVES_AT_WORK.md § 9 row 0)
// ---------------------------------------------------------------------------

/// The generated city's window (V4's 20k recipe: `--seed 42 --start-day 20
/// --days 7`): every archetype pinned together in one world, this many each.
const LIVES_BIG_SEED: u64 = 42;
const LIVES_BIG_DAY: u64 = 20;
pub(super) const LIVES_COUNT_BIG: usize = 5;

/// The lives baseline: the 2k windows (`--assert`'s first window over every
/// pinnable archetype, the seeds pooled) or, with `population`, one window of
/// the generated city. Returns the `PROBE lives` lines.
pub fn lives(
    out: Option<&std::path::Path>,
    population: Option<u32>,
    map: Option<&std::path::Path>,
) -> Result<String, String> {
    let archs: Vec<&str> = ARCHETYPES.iter().copied().filter(|&a| a != "child").collect();
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let (city, window) = match population {
        Some(n) => {
            // Shadow V7 (tool): `--map` is read here too (the island at 20k).
            let mut world = start_world(LIVES_BIG_SEED, LIVES_BIG_DAY, Some(n), map)?;
            let list = archs.join(",");
            let picked = pick(&world, LIVES_BIG_SEED, LIVES_BIG_DAY, Some(&list), LIVES_COUNT_BIG, &[]);
            if picked.is_empty() {
                return Err("lives: no candidates in the generated city".into());
            }
            // As `shadow --pick`: the pins sit inside `[lod] max_full`, grown by the pick count.
            world.config.lod.max_full += picked.len();
            let mut tracks = follow(&mut world, &picked, BEHAVIOUR_DAYS, true, false);
            if let Some(dir) = out {
                let dir = dir.join(format!("lives_{n}_d{LIVES_BIG_DAY}"));
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                let end = world.tick;
                for t in &mut tracks {
                    write_diary(&world, t, &dir, end)?;
                }
            }
            for t in &tracks {
                rows.entry(t.arch.clone()).or_default().add(t);
            }
            let stem = map.and_then(|m| m.file_stem()).map(|m| format!("@{}", m.to_string_lossy()));
            (
                format!("{}k{}", n / 1000, stem.unwrap_or_default()),
                format!(
                    "seed {LIVES_BIG_SEED}, day {LIVES_BIG_DAY}, {BEHAVIOUR_DAYS} days, {} picks of {} archetypes pinned \
                     together in one world (up to {LIVES_COUNT_BIG} each)",
                    picked.len(),
                    archs.len()
                ),
            )
        }
        None => {
            let day = BEHAVIOUR_WINDOWS[0].0;
            // One seed at a time: a clone of the city per archetype is ~48 worlds.
            for &seed in &BEHAVIOUR_SEEDS {
                for r in window_runs(seed, day, &archs, out) {
                    if let (_, _, arch, Some(row)) = r? {
                        rows.entry(arch).or_default().extend(&row);
                    }
                }
            }
            (
                "2k".to_string(),
                format!(
                    "seeds {BEHAVIOUR_SEEDS:?} pooled, day {day}, {BEHAVIOUR_DAYS} days, up to {BEHAVIOUR_SAMPLE} free \
                     agents per archetype and seed, each archetype in its own clone (pinned at Full)"
                ),
            )
        }
    };
    let mut text = String::new();
    let _ = writeln!(
        text,
        "PROBE lives {city}: {window}; rates per agent-day or agent-week pooled over the row's agents; a spouse row \
         reads the married picks only; shifts are whole free workday shifts at a building; rows under \
         {BEHAVIOUR_MIN_AGENTS} agents are one person's week; `all` pools the rows (an agent picked for two \
         archetypes counts in both)"
    );
    let mut all = Row::default();
    for (arch, row) in &rows {
        all.extend(row);
        lives_line(&mut text, &city, arch, &row.pooled);
    }
    lives_line(&mut text, &city, "all", &all.pooled);
    Ok(text)
}

/// One `PROBE lives` row: the archetype's pooled numbers.
fn lives_line(text: &mut String, city: &str, label: &str, a: &Agg) {
    let days = a.agent_days();
    let per_day = |n: f64| if days > 0.0 { n / days } else { 0.0 };
    let share = |n: u32, d: u32| if d > 0 { f64::from(n) / f64::from(d) } else { 0.0 };
    let married_days = a.married_min as f64 / TICKS_PER_DAY as f64;
    let per_married_week = |n: f64| if married_days > 0.0 { n / married_days * 7.0 } else { 0.0 };
    let mut kinds: Vec<(&String, &u32)> = a.source_kinds.iter().collect();
    kinds.sort_by(|x, y| y.1.cmp(x.1).then(x.0.cmp(y.0)));
    let kinds: Vec<String> = kinds.iter().take(3).map(|(k, n)| format!("{k} {n}")).collect();
    let _ = writeln!(
        text,
        "PROBE lives {city} {label}: n {} agent-days {days:.1} | idle_h/d {:.2} short_rest/d {:.2} | married {} never_named {} never_coloc {} \
         named/wk {:.2} coloc_h/wk {:.2} (stat-spouse {:.2}) | hang/wk {:.2} empty {:.2} ({}/{}) | shifts {} servedable \
         {:.2} any {:.2} bodies/h {:.2} venue_visits/shift {:.2} met {:.2} enc/shift {:.2} | travel_h/wk {:.2}          spire {}/{} | kinds: {}",
        a.agents,
        per_day(a.idle_min as f64 / 60.0),
        per_day(f64::from(a.short_rests)),
        a.married,
        a.never_named,
        a.never_coloc,
        per_married_week(f64::from(a.spouse_named_hours)),
        per_married_week(f64::from(a.spouse_coloc)),
        share(a.spouse_coloc_stat, a.spouse_coloc),
        per_day(f64::from(a.hangouts)) * 7.0,
        share(a.hang_empty, a.hangouts),
        a.hang_empty,
        a.hangouts,
        a.served_shifts,
        share(a.servedable, a.served_shifts),
        share(a.servedable_any, a.served_shifts),
        if a.shift_samples > 0 { a.nonstaff as f64 / a.shift_samples as f64 } else { 0.0 },
        if a.served_shifts > 0 { a.venue_visits as f64 / f64::from(a.served_shifts) } else { 0.0 },
        share(a.shifts_met, a.served_shifts),
        if a.served_shifts > 0 { a.encounters as f64 / f64::from(a.served_shifts) } else { 0.0 },
        per_day(a.travel_min as f64 / 60.0) * 7.0,
        a.spire,
        a.housed,
        if kinds.is_empty() { "none".to_string() } else { kinds.join(", ") }
    );
}

/// One printed row of the behaviour table (every value, the floors aside).
pub(super) fn row_line(text: &mut String, day: u64, label: &str, a: &Row) {
    let _ = write!(text, "d{day:<3} {label:<14} {:>2}", a.agents.len());
    for m in METRICS {
        match a.get(m, false) {
            Some(v) => {
                let _ = write!(text, " {v:>9.2}");
            }
            None => {
                let _ = write!(text, " {:>9}", "-");
            }
        }
    }
    let _ = writeln!(text);
}

pub(super) fn short(m: Metric) -> &'static str {
    match m {
        Metric::WalkHDay => "walk_h/d",
        Metric::SleepHDay => "sleep_h/d",
        Metric::LongestSleepH => "longest_h",
        Metric::Energy0HWeek => "e0_h/wk",
        Metric::WorkHDay => "work_h/d",
        Metric::ShiftsWorkedShare => "worked",
        Metric::PaidOfWorked => "paid/wkd",
        Metric::WagedWorkdays => "waged_wd",
        Metric::HangoutsWeek => "hang/wk",
        Metric::KnownMetWeek => "known/wk",
        Metric::RefundsWeek => "refund/wk",
        Metric::CommuteAbortsDay => "abort/d",
        Metric::BoutsAttended => "bouts_at",
        Metric::StarveHWeek => "starve_h/wk",
        Metric::SynthShare => "synth%",
        Metric::DivesWeek => "dive/wk",
    }
}

/// Hunger as sickness: the meters on one line (the header and meal lines).
pub(super) fn health_line(h: &citysim::Health) -> String {
    let cancer = h.cancer.map_or(String::from("no cancer"), |c| {
        format!("cancer stage {:.2}{}", c.stage, if c.diagnosed { " (diagnosed)" } else { " (silent)" })
    });
    format!(
        "malnutrition {:.2} | toxicity {:.2} | {cancer} | sick days {}/14{}",
        h.malnutrition,
        h.toxicity,
        h.sick_days.count_ones(),
        if h.sick_today { " | OFF SICK TODAY" } else { "" }
    )
}
