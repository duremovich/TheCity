//! Ignored probes for by-hand investigation. Run one with
//! `cargo test --release -p citysim --test scratch_probe -- --ignored --nocapture <name>`.

use std::time::Instant;

use citysim::{Config, World, TICKS_PER_DAY};

/// Per-system wall time over one day after a long warm-up: where the ticks go.
#[test]
#[ignore]
fn probe_system_timing() {
    let warm_days: u64 = std::env::var("WARM_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(50);
    let mut w = World::new(42, Config::load().v1_profile());
    let t0 = Instant::now();
    w.run_ticks(warm_days * TICKS_PER_DAY);
    eprintln!("warm-up {warm_days} days in {:.1}s, edges {}", t0.elapsed().as_secs_f32(), w.edges.len());
    let names = [
        "commands",
        "lod",
        "needs",
        "memory",
        "mood",
        "think",
        "plan",
        "exec",
        "economy",
        "law",
        "social",
        "gang",
        "demography",
        "stats",
    ];
    let mut acc = [0f64; 14];
    for _ in 0..TICKS_PER_DAY {
        let steps: [&dyn Fn(&mut World); 14] = [
            &|w| w.apply_commands(),
            &citysim::systems::lod::run,
            &citysim::needs::run,
            &citysim::systems::memory::run,
            &citysim::mood::run,
            &citysim::systems::think::run,
            &citysim::systems::plan::run,
            &citysim::exec::run,
            &citysim::systems::economy::run,
            &citysim::systems::law::run,
            &citysim::systems::social::run,
            &citysim::systems::gang::run,
            &citysim::systems::demography::run,
            &citysim::systems::stats::run,
        ];
        for (i, step) in steps.iter().enumerate() {
            let t = Instant::now();
            step(&mut w);
            acc[i] += t.elapsed().as_secs_f64();
        }
        w.tick += 1;
    }
    let total: f64 = acc.iter().sum();
    for (name, secs) in names.iter().zip(acc) {
        eprintln!("{name:>9} {:7.1} ms  {:5.1}%", secs * 1e3, secs / total * 100.0);
    }
    eprintln!("day total {:.0} ms -> {:.0} ticks/s", total * 1e3, TICKS_PER_DAY as f64 / total);
}

/// Why does a corpse at home not get buried? Events and the digger's thinking.
#[test]
#[ignore]
fn probe_burial() {
    use citysim::{Brain, DeathCause, EventKind, Household, Job, Role};
    let mut w = World::new(43, Config::load().v1_profile());
    let id = w.citizens().into_iter().find(|&id| !w.has::<Job>(id) && w.has::<Brain>(id)).expect("civilian");
    let diggers: Vec<_> =
        w.citizens().into_iter().filter(|&d| w.comp::<Job>(d).is_some_and(|j| j.role == Role::Gravedigger)).collect();
    eprintln!("corpse {id:?} home {:?}; diggers {diggers:?}", w.comp::<Household>(id).and_then(|h| h.home));
    w.kill(id, DeathCause::Violence);
    for day in 0..2 {
        for _ in 0..TICKS_PER_DAY {
            w.tick();
            let t = w.tick;
            if t.is_multiple_of(240) {
                for &d in &diggers {
                    let b = w.comp::<Brain>(d).expect("brain");
                    let top: Vec<String> = b
                        .last_think
                        .as_ref()
                        .map(|tr| tr.goals.iter().take(4).map(|g| format!("{:?}={:.2}", g.goal, g.score)).collect())
                        .unwrap_or_default();
                    eprintln!(
                        "t{t} digger {} goal {:?} plan {:?} carrying {:?} top {top:?}",
                        d.index,
                        b.current_goal,
                        b.plan.as_ref().map(|p| p.steps.iter().map(|s| format!("{:?}", s.action)).collect::<Vec<_>>()),
                        b.carrying_corpse
                    );
                }
            }
        }
        let _ = day;
    }
    for e in w.events.iter().filter(|e| {
        diggers.iter().any(|d| e.actors.contains(d))
            && (e.kind == EventKind::PlanAborted || e.kind == EventKind::Burial)
    }) {
        eprintln!("{} {:?} {}", e.tick, e.kind, e.text);
    }
    let mem = w.comp::<citysim::Memory>(diggers[0]).expect("mem");
    eprintln!(
        "digger memories: {:?}",
        mem.entries.iter().map(|m| (m.kind, m.subject.map(|s| s.index))).collect::<Vec<_>>()
    );
}

/// Is BuryCorpse feasible for the gravedigger with the corpse bound?
#[test]
#[ignore]
fn probe_bury_feasible() {
    use citysim::{ActionKind, Brain, DeathCause, Job, LocationKey, PlanCtx, Role, WorldState};
    let mut w = World::new(43, Config::load().v1_profile());
    let id = w.citizens().into_iter().find(|&id| !w.has::<Job>(id) && w.has::<Brain>(id)).expect("civilian");
    let digger =
        w.citizens().into_iter().find(|&d| w.comp::<Job>(d).is_some_and(|j| j.role == Role::Gravedigger)).expect("d");
    w.kill(id, DeathCause::Violence);
    w.tick();
    let ctx = PlanCtx::build(&w, digger, Some(id));
    let ws = WorldState::observe(&w, digger, Some(id));
    eprintln!(
        "corpse_target {} may_bury {} adult {} target {:?}",
        ctx.corpse_target, ctx.may_bury, ctx.adult, ctx.target
    );
    eprintln!("dist keys {:?}", ctx.dist.keys().collect::<Vec<_>>());
    for a in [
        ActionKind::GoTo(LocationKey::CorpseTile),
        ActionKind::CarryCorpse,
        ActionKind::GoTo(LocationKey::Cemetery),
        ActionKind::BuryCorpse,
    ] {
        eprintln!(
            "{a:?}: allowed {} feasible {} precond {} produces(CorpseBuried) {}",
            a.allowed(&ctx),
            a.feasible(&ctx),
            a.preconditions(&ws, &ctx),
            a.produces(citysim::Key::CorpseBuried, true, &ctx)
        );
    }
    eprintln!(
        "ws.at {:?} known_corpse {} carrying {} corpse_buried {}",
        ws.at, ws.known_corpse, ws.carrying_corpse, ws.corpse_buried
    );
}

/// Which birth precondition fails? Spouse pairs at day 60, filter by filter.
#[test]
#[ignore]
fn probe_births() {
    use citysim::{Household, Identity, Mood, Needs};
    let mut w = World::new(42, Config::load().v1_profile());
    w.run_ticks(60 * TICKS_PER_DAY);
    let pairs: Vec<(citysim::EntityId, citysim::EntityId)> =
        w.spouses.iter().filter(|(a, b)| a < b).map(|(&a, &b)| (a, b)).collect();
    let age = |id| w.comp::<Identity>(id).map_or(0, |i| i.age_days);
    let fertile_age = |id| (2160..=5400).contains(&age(id));
    let same_home = |a, b| {
        let ha = w.comp::<Household>(a).and_then(|h| h.home);
        ha.is_some() && ha == w.comp::<Household>(b).and_then(|h| h.home)
    };
    let intimacy = |id| w.comp::<Needs>(id).map_or(-1.0, |n| n.intimacy);
    let mood = |id| w.comp::<Mood>(id).map_or(0.0, |m| m.value);
    let n_age = pairs.iter().filter(|&&(a, b)| fertile_age(a) && fertile_age(b)).count();
    let n_home = pairs.iter().filter(|&&(a, b)| fertile_age(a) && fertile_age(b) && same_home(a, b)).count();
    let n_int = pairs
        .iter()
        .filter(|&&(a, b)| {
            fertile_age(a) && fertile_age(b) && same_home(a, b) && intimacy(a) >= 0.6 && intimacy(b) >= 0.6
        })
        .count();
    let mean_int: f32 =
        pairs.iter().map(|&(a, b)| (intimacy(a) + intimacy(b)) / 2.0).sum::<f32>() / pairs.len().max(1) as f32;
    let mean_mood: f32 = pairs.iter().map(|&(a, b)| (mood(a) + mood(b)) / 2.0).sum::<f32>() / pairs.len().max(1) as f32;
    let ages: Vec<u32> = w.citizens().into_iter().map(age).collect();
    let young = ages.iter().filter(|&&a| (2160..=5400).contains(&a)).count();
    eprintln!(
        "pairs {} | both 18-45: {n_age} | +same home: {n_home} | +intimacy>=0.6: {n_int} | mean intimacy {mean_int:.2} mood {mean_mood:.2} | citizens 18-45: {young}/{}",
        pairs.len(),
        ages.len()
    );
}

/// What are Full agents doing at 02:00? (Calibration sanity.)
#[test]
#[ignore]
fn probe_night_states() {
    use citysim::{Brain, ExecState, Lod};
    let mut cfg = Config::load().v1_profile();
    cfg.lod.force = Some(Lod::Full);
    cfg.world.population = 200;
    let mut w = World::new(1000, cfg);
    w.run_ticks(2 * TICKS_PER_DAY + 120);
    let mut hist: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for id in w.citizens() {
        let Some(b) = w.comp::<Brain>(id) else { continue };
        let key = match &b.exec {
            ExecState::Use { kind, .. } => format!("Use({kind:?})"),
            ExecState::Goto { .. } => "Goto".into(),
            ExecState::GotoTimed { .. } => "GotoTimed".into(),
            ExecState::Wait { .. } => "Wait".into(),
            ExecState::Idle => format!("Idle goal={:?} plan={}", b.current_goal, b.plan.is_some()),
        };
        *hist.entry(key).or_default() += 1;
    }
    eprintln!("phase {:?} tick {}", w.phase(), w.tick);
    for (k, n) in hist {
        eprintln!("{n:4} {k}");
    }
}

/// Whose mood never moved in ten days, by LOD?
#[test]
#[ignore]
fn probe_still_moods() {
    use citysim::{Brain, Mood};
    let mut w = World::new(5, Config::load().v1_profile());
    w.run_ticks(10 * TICKS_PER_DAY);
    let mut hist: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for id in w.citizens() {
        let Some(m) = w.comp::<Mood>(id) else { continue };
        if m.value == 0.0 {
            let lod = w.comp::<Brain>(id).map(|b| format!("{:?}", b.lod)).unwrap_or("none".into());
            *hist.entry(lod).or_default() += 1;
        }
    }
    eprintln!("still moods by lod: {hist:?}");
    let id = w.citizens().into_iter().find(|&id| w.comp::<Mood>(id).is_some_and(|m| m.value == 0.0)).expect("one");
    eprintln!(
        "example needs {:?} last_computed {:?}",
        w.comp::<citysim::Needs>(id),
        w.comp::<Mood>(id).map(|m| m.last_computed)
    );
}

/// Every GangJoin on seed 7: does the recruit hold a MetInJail memory, and of a member?
#[test]
#[ignore]
fn probe_gang_join_citations() {
    use citysim::{EventKind, Gang, Memory, MemoryKind};
    let mut w = World::new(7, Config::load().v1_profile());
    let mut seen = 0;
    for _ in 0..120 {
        w.run_ticks(TICKS_PER_DAY);
        let joins: Vec<_> = w
            .events
            .iter()
            .filter(|e| e.tick >= seen && e.kind == EventKind::GangJoin)
            .map(|e| (e.tick, e.actors[0], e.text.clone()))
            .collect();
        for (tick, id, text) in joins {
            let members = w
                .gangs()
                .first()
                .copied()
                .and_then(|g| w.comp::<Gang>(g))
                .map(|g| g.members.clone())
                .unwrap_or_default();
            let met: Vec<String> = w
                .comp::<Memory>(id)
                .map(|m| {
                    m.entries
                        .iter()
                        .filter(|e| e.kind == MemoryKind::MetInJail)
                        .map(|e| {
                            format!(
                                "{:?}{}",
                                e.subject.map(|s| s.index),
                                if e.subject.is_some_and(|s| members.contains(&s)) { "*" } else { "" }
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            eprintln!("t{tick} {text} | members {} | MetInJail {met:?}", members.len());
        }
        seen = w.tick;
    }
    let jailed_days: u32 = w.stats.history.iter().map(|r| r.jailed).sum();
    eprintln!("jailed agent-days {jailed_days}");
}

/// Seed 7: why does the gang stop at one member? Daily treasury, members,
/// contacts and funded eligibility.
#[test]
#[ignore]
fn probe_gang_growth() {
    use citysim::systems::gang;
    use citysim::{Brain, Gang, GangMember, Job, Role, Sentence};
    let mut w = World::new(7, Config::load().v1_profile());
    for day in 0..120 {
        w.run_ticks(TICKS_PER_DAY);
        if day % 6 != 5 {
            continue;
        }
        let Some(g) = w.gangs().first().copied().and_then(|g| w.comp::<Gang>(g)).cloned() else { continue };
        let thr = w.config.social.join_gang_affinity;
        let contacts = w
            .citizens()
            .into_iter()
            .filter(|&id| w.has::<Brain>(id) && !w.has::<GangMember>(id))
            .filter(|&id| g.members.iter().any(|&m| w.edge(id, m).is_some_and(|e| e.affinity >= thr)))
            .count();
        let eligible = w.citizens().into_iter().filter(|&id| w.has::<Brain>(id) && gang::eligible(&w, id)).count();
        let jailed_members = g.members.iter().filter(|&&m| w.has::<Sentence>(m)).count();
        let guards =
            w.citizens().into_iter().filter(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)).count();
        eprintln!(
            "day {day}: members {} (jailed {jailed_members}) treasury {} territory {} contacts>={thr} {contacts} eligible {eligible} guards {guards}",
            g.members.len(),
            g.treasury,
            g.territory.len()
        );
    }
}

/// Cohabiting fertile couples at day 60: LOD pair and intimacy.
#[test]
#[ignore]
fn probe_couple_intimacy() {
    use citysim::{Brain, Household, Identity, Needs};
    let mut w = World::new(42, Config::load().v1_profile());
    w.run_ticks(60 * TICKS_PER_DAY);
    let mut hist: std::collections::BTreeMap<String, (usize, f32)> = std::collections::BTreeMap::new();
    for (&a, &b) in w.spouses.iter().filter(|(a, b)| a < b) {
        let ha = w.comp::<Household>(a).and_then(|h| h.home);
        if ha.is_none() || ha != w.comp::<Household>(b).and_then(|h| h.home) {
            continue;
        }
        let age = |id| w.comp::<Identity>(id).map_or(0, |i| i.age_days);
        if !(2160..=5400).contains(&age(a)) || !(2160..=5400).contains(&age(b)) {
            continue;
        }
        let lod = |id| w.comp::<Brain>(id).map(|b| format!("{:?}", b.lod)).unwrap_or("-".into());
        let int = |id| w.comp::<Needs>(id).map_or(-1.0, |n| n.intimacy);
        let key = format!("{}+{}", lod(a), lod(b));
        let e = hist.entry(key).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += (int(a) + int(b)) / 2.0;
    }
    for (k, (n, sum)) in hist {
        eprintln!("{k}: {n} couples, mean intimacy {:.2}", sum / n as f32);
    }
}

/// Save/load round trip on a day-30 world (spec: < 200 ms).
#[test]
#[ignore]
fn probe_save_roundtrip_time() {
    let mut w = World::new(42, Config::load().v1_profile());
    w.run_ticks(30 * TICKS_PER_DAY);
    let t0 = Instant::now();
    let text = citysim::save::to_ron(&w);
    let t1 = Instant::now();
    let back = citysim::save::from_ron(&text).expect("load");
    let t2 = Instant::now();
    eprintln!(
        "save {:.0} ms ({} KB), load {:.0} ms, tick {}",
        (t1 - t0).as_secs_f64() * 1e3,
        text.len() / 1024,
        (t2 - t1).as_secs_f64() * 1e3,
        back.tick
    );
}

/// M8: one line per gang per day: order, headcounts, heat, treasury,
/// territory, and the top order scores. Seed 42, 120 days.
#[test]
#[ignore]
fn probe_faction_brain() {
    use citysim::systems::faction;
    use citysim::Gang;
    let mut w = World::new(42, Config::load().v1_profile());
    let gangs = w.gangs();
    for day in 0..120 {
        w.run_ticks(TICKS_PER_DAY);
        for &g in &gangs {
            let Some(gang) = w.comp::<Gang>(g) else { continue };
            let fit = citysim::systems::gang::fit_headcount(&w, g);
            let heat = faction::heat(&w, g);
            let inputs = faction::gather_inputs(&w, g);
            let trace: Vec<String> = gang.order_trace.iter().map(|s| format!("{}={:.2}", s.order, s.score)).collect();
            let (frontier, prize) = inputs.as_ref().map_or((0, 0), |i| (i.frontier, i.prize));
            eprintln!(
                "d{day:>3} {:<10} {:<9} n{:>2} fit{:>2} heat{heat:.2} $ {:>4} terr{:>2} front{frontier:>2} prize{prize:>4} raid_at{:?} {}",
                gang.name, gang.order.to_string(), gang.members.len(), fit, gang.treasury, gang.territory.len(),
                gang.raid_at.map(|t| t / TICKS_PER_DAY), trace.join(" ")
            );
        }
    }
}

/// M8: who is fighting whom? Assaults over 120 days on seed 42, classified
/// by the gang membership of attacker and victim at the day's end.
#[test]
#[ignore]
fn probe_assault_sources() {
    use citysim::EventKind;
    let mut w = World::new(42, Config::load().v1_profile());
    let (mut cross, mut civ_on_gang, mut gang_on_civ, mut civ_on_civ, mut extortions) = (0, 0, 0, 0, 0);
    let mut seen = 0;
    for _ in 0..120 {
        w.run_ticks(TICKS_PER_DAY);
        for e in w.events.iter().filter(|e| e.tick >= seen) {
            match e.kind {
                EventKind::Extortion => extortions += 1,
                EventKind::Assault | EventKind::Murder => {
                    let a = e.actors.first().and_then(|&a| w.gang_of(a));
                    let v = e.actors.get(1).and_then(|&v| w.gang_of(v));
                    match (a, v) {
                        (Some(x), Some(y)) if x != y => cross += 1,
                        (Some(_), _) => gang_on_civ += 1,
                        (None, Some(_)) => civ_on_gang += 1,
                        (None, None) => civ_on_civ += 1,
                    }
                }
                _ => {}
            }
        }
        seen = w.tick;
    }
    eprintln!("extortions {extortions} | assaults: cross-gang {cross} civilian-on-member {civ_on_gang} member-on-civilian {gang_on_civ} civilian-on-civilian {civ_on_civ}");
}

/// M10 5c: for every guard shift that ends uncredited, what was the guard doing
/// at the shift's last tick, and what did it do over the shift?
#[test]
#[ignore]
fn probe_guard_shift_end() {
    use citysim::{Brain, Job, Role};
    use std::collections::BTreeMap;
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(20);
    let mut w = World::new(42, Config::load());
    let guards: Vec<_> =
        w.citizens().into_iter().filter(|&g| w.comp::<Job>(g).is_some_and(|j| j.role == Role::Guard)).collect();
    let mut was_on: BTreeMap<_, bool> = BTreeMap::new();
    let mut shift_goals: BTreeMap<_, BTreeMap<String, u32>> = BTreeMap::new();
    let mut at_end: BTreeMap<String, u32> = BTreeMap::new();
    let mut over: BTreeMap<String, u32> = BTreeMap::new();
    let (mut credited, mut uncredited, mut offday) = (0, 0, 0);
    let mut pending = Vec::new();
    for _ in 0..days * TICKS_PER_DAY {
        let before: BTreeMap<_, String> = guards
            .iter()
            .filter_map(|&g| {
                let b = w.comp::<Brain>(g)?;
                let step = b
                    .plan
                    .as_ref()
                    .and_then(|p| p.steps.get(usize::from(b.plan_step)))
                    .map_or("none".to_string(), |s| format!("{:?}", s.action));
                Some((g, format!("{:?}/{}", b.current_goal, step)))
            })
            .collect();
        w.tick();
        for &g in &guards {
            let Some(j) = w.comp::<Job>(g) else { continue };
            let on = j.on_shift(w.tick_of_day());
            let prev = was_on.insert(g, on).unwrap_or(false);
            if on {
                let goal = w.comp::<Brain>(g).map_or("-".into(), |b| format!("{:?}", b.current_goal));
                *shift_goals.entry(g).or_default().entry(goal).or_default() += 1;
            }
            if prev && !on {
                let key = j.shift_key_at(w.tick - 1);
                if !citysim::exec::routine::is_workday(key) {
                    offday += 1;
                } else {
                    pending.push((
                        w.tick + 90,
                        g,
                        key,
                        before.get(&g).cloned().unwrap_or_default(),
                        shift_goals.get(&g).cloned().unwrap_or_default(),
                    ));
                }
                shift_goals.remove(&g);
            }
        }
        let now = w.tick;
        let (due, rest): (Vec<_>, Vec<_>) = pending.drain(..).partition(|p| p.0 <= now);
        pending = rest;
        for (_, g, key, doing, goals) in due {
            let Some(j) = w.comp::<Job>(g) else { continue };
            {
                if j.last_shift_day == Some(key) || j.shift_credited == Some(key) {
                    credited += 1;
                } else {
                    let before: BTreeMap<_, String> = [(g, doing)].into_iter().collect();
                    let shift_goals: BTreeMap<_, BTreeMap<String, u32>> = [(g, goals)].into_iter().collect();
                    uncredited += 1;
                    *at_end.entry(before.get(&g).cloned().unwrap_or_default()).or_default() += 1;
                    let goals = shift_goals.get(&g).cloned().unwrap_or_default();
                    let top = goals.iter().max_by_key(|(_, &n)| n).map(|(k, _)| k.clone()).unwrap_or_default();
                    *over.entry(top).or_default() += 1;
                }
            }
        }
    }
    eprintln!("shifts credited {credited} uncredited {uncredited} offday {offday}");
    let mut v: Vec<_> = at_end.into_iter().collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    eprintln!("doing at shift end (uncredited): {:#?}", &v[..v.len().min(15)]);
    eprintln!("dominant goal over uncredited shifts: {over:?}");
}

/// M10 5c: guard hardship (days 1-60) and starvation deaths by age (120 days).
/// `V1=1` runs the v1 profile.
#[test]
#[ignore]
fn probe_starvation() {
    use citysim::{Corpse, DeathCause, Identity, Job, Needs, Role, Wallet, TICKS_PER_HOUR};
    use std::collections::BTreeSet;
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(42);
    let cfg = if std::env::var("V1").is_ok() { Config::load().v1_profile() } else { Config::load() };
    let mut w = World::new(seed, cfg);
    let pop0 = w.citizens().len();
    let mut ever_guard = BTreeSet::new();
    let mut broke = BTreeSet::new();
    let mut starving_guard_days = 0u32;
    let mut dead = BTreeSet::new();
    let (mut adult, mut child, mut adult_guard) = (0u32, 0u32, 0u32);
    let mut ages = std::collections::BTreeMap::new();
    let mut guard_now = BTreeSet::new();
    let mut last_seen = std::collections::BTreeMap::new();
    for day in 0..days {
        let mut starving_today = BTreeSet::new();
        for h in 0..24 {
            for _ in 0..TICKS_PER_HOUR {
                w.tick();
            }
            let living: BTreeSet<_> = w.citizens().into_iter().collect();
            for &id in ages.keys().filter(|id| !living.contains(id)) {
                if let Some(c) = w.comp::<Corpse>(id) {
                    if dead.insert(id) && c.cause == DeathCause::Starvation {
                        let age: u32 = *ages.get(&id).unwrap_or(&99999);
                        if age >= citysim::systems::demography::ADULT_AGE_DAYS {
                            adult += 1;
                            if guard_now.contains(&id) {
                                adult_guard += 1;
                                eprintln!("  day {day} guard {id:?} starved; last seen {:?}", last_seen.get(&id));
                            }
                        } else {
                            child += 1;
                        }
                    }
                }
            }
            for &id in &living {
                if w.comp::<Needs>(id).is_some_and(|n| n.starving_since.is_some()) {
                    let b = w.comp::<citysim::Brain>(id);
                    last_seen.insert(
                        id,
                        (
                            w.comp::<Wallet>(id).map(|x| x.coins),
                            w.comp::<citysim::Inventory>(id).map(|x| x.food),
                            b.and_then(|b| b.current_goal),
                            w.comp::<Job>(id).map(|j| (j.role, j.days_unpaid)),
                            w.has::<citysim::Sentence>(id),
                            b.and_then(|b| b.cuffed_by).is_some(),
                        ),
                    );
                }
                if let Some(i) = w.comp::<Identity>(id) {
                    ages.insert(id, i.age_days);
                }
                let is_guard = w.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard);
                if is_guard {
                    guard_now.insert(id);
                } else if h == 0 {
                    guard_now.remove(&id);
                }
                if day < 60 && is_guard {
                    ever_guard.insert(id);
                    if w.comp::<Wallet>(id).is_some_and(|x| x.coins < 3) {
                        broke.insert(id);
                    }
                    if w.comp::<Needs>(id).is_some_and(|n| n.starving_since.is_some()) {
                        starving_today.insert(id);
                    }
                }
            }
        }
        starving_guard_days += starving_today.len() as u32;
        if day == 59 {
            eprintln!(
                "days 1-60: guards ever {} broke {} starving guard-days {starving_guard_days}",
                ever_guard.len(),
                broke.len()
            );
        }
    }
    eprintln!(
        "{days} days, start pop {pop0}: starvation deaths adult {adult} (guards {adult_guard}) child {child}; per 100 residents {:.2}",
        f64::from(adult + child) * 100.0 / pop0 as f64
    );
}

/// M10 5c: the law's posture per day on seed 42 (2,000), with the Posture events.
#[test]
#[ignore]
fn probe_posture_history() {
    use citysim::{EventKind, Posture};
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let mut w = World::new(42, Config::load());
    let mut line = String::new();
    let mut crack = 0;
    let mut seen = 0;
    for day in 0..days {
        w.run_ticks(TICKS_PER_DAY);
        for e in w.events.iter().filter(|e| e.tick >= seen && e.kind == EventKind::Posture) {
            eprintln!("  d{day} {}", e.text);
        }
        seen = w.tick;
        let (p, t) = w.law().map_or((Posture::Patrol, None), |l| (l.posture, l.target));
        if p == Posture::Crackdown {
            crack += 1;
        }
        line.push(match p {
            Posture::Patrol => 'P',
            Posture::Crackdown => 'C',
            Posture::Garrison => 'G',
        });
        let _ = t;
    }
    eprintln!("{line}\nCrackdown {crack}/{days}");
}
