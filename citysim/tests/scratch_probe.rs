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
            ExecState::Fly { .. } => "Fly".into(),
            ExecState::Wait { .. } => "Wait".into(),
            ExecState::JackedIn { .. } => "JackedIn".into(),
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
    let mut duty: Vec<(bool, f32, f32)> = Vec::new();
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
                if j.last_shift_day == Some(key) {
                    credited += 1;
                } else {
                    let before: BTreeMap<_, String> = [(g, doing)].into_iter().collect();
                    let shift_goals: BTreeMap<_, BTreeMap<String, u32>> = [(g, goals)].into_iter().collect();
                    uncredited += 1;
                    *at_end.entry(before.get(&g).cloned().unwrap_or_default()).or_default() += 1;
                    let goals = shift_goals.get(&g).cloned().unwrap_or_default();
                    let top = goals.iter().max_by_key(|(_, &n)| n).map(|(k, _)| k.clone()).unwrap_or_default();
                    *over.entry(top).or_default() += 1;
                    let total: u32 = goals.values().sum();
                    let law: u32 = goals
                        .iter()
                        .filter(|(k, _)| k.contains("Patrol") || k.contains("Arrest") || k.contains("Work"))
                        .map(|(_, n)| n)
                        .sum();
                    let sleep: u32 = goals.iter().filter(|(k, _)| k.contains("Sleep")).map(|(_, n)| n).sum();
                    let night = j.shifts[0].0 > 1000;
                    duty.push((night, law as f32 / total.max(1) as f32, sleep as f32 / total.max(1) as f32));
                }
            }
        }
    }
    eprintln!("shifts credited {credited} uncredited {uncredited} offday {offday}");
    let mut v: Vec<_> = at_end.into_iter().collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    eprintln!("doing at shift end (uncredited): {:#?}", &v[..v.len().min(15)]);
    eprintln!("dominant goal over uncredited shifts: {over:?}");
    for night in [false, true] {
        let v: Vec<_> = duty.iter().filter(|d| d.0 == night).collect();
        let mut hist = [0u32; 5];
        for d in &v {
            hist[((d.1 * 5.0) as usize).min(4)] += 1;
        }
        let sleep = v.iter().map(|d| d.2).sum::<f32>() / v.len().max(1) as f32;
        eprintln!("uncredited {} shifts: {} ; law-duty share histogram (0-20%..80-100%) {hist:?}; mean sleep share {sleep:.2}", if night { "night" } else { "day" }, v.len());
    }
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
                            w.comp::<Job>(id).map(|j| (j.role, j.days_unpaid, j.shifts[0].0)),
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

/// M10 5c: guards who quit unpaid: how many days owed, and the Unpaid memories, an hour before.
#[test]
#[ignore]
fn probe_guard_quits() {
    use citysim::{Brain, Job, Memory, MemoryKind, Role, TICKS_PER_HOUR};
    use std::collections::BTreeMap;
    let mut w = World::new(42, Config::load());
    let mut last: BTreeMap<_, String> = BTreeMap::new();
    for _ in 0..60 * 24 {
        let guards = w.guards().to_vec();
        w.run_ticks(TICKS_PER_HOUR);
        for g in guards {
            if w.comp::<Job>(g).is_some_and(|j| j.role == Role::Guard) {
                let j = w.comp::<Job>(g).expect("job");
                let unpaid = w
                    .comp::<Memory>(g)
                    .map_or(0, |m| m.entries.iter().filter(|e| e.kind == MemoryKind::Unpaid).count());
                let goal = w.comp::<Brain>(g).and_then(|b| b.current_goal);
                last.insert(
                    g,
                    format!(
                        "days_unpaid {} last_attempt {:?} unpaid_mem {unpaid} goal {goal:?} shift {}",
                        j.days_unpaid, j.last_wage_attempt_day, j.shifts[0].0
                    ),
                );
            } else if let Some(s) = last.remove(&g) {
                if w.comp::<Brain>(g).is_some() {
                    eprintln!("day {} {g:?} left the watch; an hour before: {s}", w.day());
                }
            }
        }
    }
}

/// M11: who carries rent arrears, by tier, owner and job; wallets at midnight.
#[test]
#[ignore]
fn probe_m11_rent() {
    use citysim::{Brain, Building, Household, Job, Lod, Wallet};
    use std::collections::BTreeMap;
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(24);
    let mut w = World::new(42, Config::load());
    for d in 0..days {
        w.run_ticks(TICKS_PER_DAY);
        if d % 4 != 3 {
            continue;
        }
        // just after midnight's pass (tick of day 1)
        w.run_ticks(1);
        let mut by: BTreeMap<String, (u32, u32, i64)> = BTreeMap::new();
        for id in w.citizens() {
            let Some(h) = w.comp::<Household>(id) else { continue };
            let Some(home) = h.home else { continue };
            if !w.has::<Brain>(id) || !citysim::systems::demography::is_adult(&w, id) {
                continue;
            }
            let b = w.comp::<Building>(home).expect("home");
            let owner = if b.owner.is_none() { "city" } else { "corp" };
            let job = if w.has::<Job>(id) { "job" } else { "dole" };
            let lod = w.comp::<Brain>(id).map(|b| b.lod).unwrap_or(Lod::Statistical);
            let key = format!("t{} {owner} {job} {lod:?}", b.tier);
            let e = by.entry(key).or_default();
            e.0 += 1;
            e.1 += u32::from(h.arrears > 0);
            e.2 += w.comp::<Wallet>(id).map_or(0, |w| w.coins);
        }
        eprintln!("--- day {} (adults, in arrears, mean coins at 00:01)", w.day());
        for (k, (n, a, c)) in by {
            eprintln!("{k:32} {n:5} {a:5} {:6.1}", c as f64 / f64::from(n.max(1)));
        }
    }
}

/// M11: per-system wall time over one day of the default city (bind and
/// ownership included) after `WARM_DAYS`; `CITYSIM_ASSETS` picks the config.
#[test]
#[ignore]
fn probe_m11_timing() {
    let warm_days: u64 = std::env::var("WARM_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(45);
    let mut w = World::new(42, Config::load());
    w.run_ticks(warm_days * TICKS_PER_DAY);
    let names = [
        "lod",
        "needs",
        "memory",
        "mood",
        "think",
        "plan",
        "exec",
        "ownership",
        "economy",
        "bind",
        "law",
        "social",
        "gang",
        "demography",
        "stats",
    ];
    let mut acc = [0f64; 15];
    for _ in 0..TICKS_PER_DAY {
        w.apply_commands();
        let steps: [&dyn Fn(&mut World); 15] = [
            &citysim::systems::lod::run,
            &citysim::needs::run,
            &citysim::systems::memory::run,
            &citysim::mood::run,
            &citysim::systems::think::run,
            &citysim::systems::plan::run,
            &citysim::exec::run,
            &citysim::systems::ownership::run,
            &citysim::systems::economy::run,
            &citysim::systems::bind::run,
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
        eprintln!("{name:>10} {:7.1} ms  {:5.1}%", secs * 1e3, secs / total * 100.0);
    }
    let gang: usize = w.gangs().iter().filter_map(|&g| w.comp::<citysim::Gang>(g)).map(|g| g.members.len()).sum();
    eprintln!(
        "day total {:.0} ms -> {:.0} ticks/s; gang members {gang}, guards {}",
        total * 1e3,
        TICKS_PER_DAY as f64 / total,
        w.guards().len()
    );
}

/// M11: who is evicted: jailed, employed, gang, tier, coins, last dole.
#[test]
#[ignore]
fn probe_m11_evictions() {
    use citysim::{Brain, Building, EventKind, GangMember, Household, Job, Sentence, Wallet};
    use std::collections::BTreeMap;
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
    let mut w = World::new(42, Config::load());
    let mut tally: BTreeMap<String, u32> = BTreeMap::new();
    let mut next = 0u64;
    let mut snap: BTreeMap<citysim::EntityId, String> = BTreeMap::new();
    for _ in 0..days * 24 {
        // Snapshot the state an hour before; evictions happen at midnight.
        if w.tick_of_day() == 23 * 60 {
            snap = {
                w.citizens()
                    .into_iter()
                    .filter(|&a| w.comp::<Household>(a).is_some_and(|h| h.arrears >= 6 && h.home.is_some()))
                    .map(|a| {
                        let tier = w
                            .comp::<Household>(a)
                            .and_then(|h| h.home)
                            .and_then(|h| w.comp::<Building>(h))
                            .map_or(9, |b| b.tier);
                        let lod = w.comp::<Brain>(a).map(|b| format!("{:?}", b.lod)).unwrap_or_default();
                        let dole = w.comp::<Brain>(a).and_then(|b| b.last_dole_day).map_or(-1, |d| d as i64);
                        let s = format!(
                            "jail {} job {} gang {} t{tier} {lod} coins {} dole_age {}",
                            w.has::<Sentence>(a),
                            w.has::<Job>(a),
                            w.has::<GangMember>(a),
                            w.comp::<Wallet>(a).map_or(0, |w| w.coins),
                            w.day() as i64 - dole
                        );
                        (a, s)
                    })
                    .collect()
            };
        }
        w.run_ticks(60);
        for e in w.events.iter().filter(|e| e.id >= next && e.kind == EventKind::Evicted) {
            let key = snap.get(&e.actors[0]).cloned().unwrap_or_else(|| "spouse/unknown".into());
            let short: String = key.split(" coins").next().unwrap_or("").to_string();
            *tally.entry(short).or_default() += 1;
            eprintln!("day {} {}", w.day(), key);
        }
        next = w.events.back().map_or(0, |e| e.id + 1);
    }
    for (k, n) in tally {
        eprintln!("{n:4} {k}");
    }
}

/// M11: plan_for wall time by group and goal over one day at day 45.
#[test]
#[ignore]
fn probe_m11_plan_time() {
    use citysim::{Brain, GangMember, Job, Role};
    use std::collections::BTreeMap;
    let mut w = World::new(42, Config::load());
    w.run_ticks(45 * TICKS_PER_DAY);
    let group = |w: &World, id: citysim::EntityId| -> &'static str {
        if w.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard) {
            if citysim::systems::law::is_private_guard(w, id) {
                "private"
            } else {
                "guard"
            }
        } else if w.has::<GangMember>(id) {
            "gang"
        } else {
            "other"
        }
    };
    let mut acc: BTreeMap<String, (u32, f64)> = BTreeMap::new();
    for _ in 0..TICKS_PER_DAY {
        w.apply_commands();
        citysim::systems::lod::run(&mut w);
        citysim::needs::run(&mut w);
        citysim::systems::memory::run(&mut w);
        citysim::mood::run(&mut w);
        citysim::systems::think::run(&mut w);
        // plan::run, timed per call
        let tick = w.tick;
        let (mut planned, mut expansions) = (0usize, 0usize);
        while planned < w.config.brain.plan_budget_per_tick
            && expansions < w.config.brain.plan_expansion_budget_per_tick
        {
            let Some((&(u, id), &enq)) = w.plan_queue.iter().next() else { break };
            w.plan_queue.remove(&(u, id));
            if let Some(b) = w.comp_mut::<Brain>(id) {
                b.plan_queued = false;
            }
            if tick.saturating_sub(enq) > citysim::systems::plan::PLAN_QUEUE_MAX_AGE {
                continue;
            }
            let Some(goal) = w.comp::<Brain>(id).filter(|b| b.plan.is_none()).and_then(|b| b.current_goal) else {
                continue;
            };
            planned += 1;
            let g = group(&w, id);
            let t = Instant::now();
            expansions += citysim::systems::plan::plan_for(&mut w, id, goal);
            let e = acc.entry(format!("{g:8} {goal:?}")).or_default();
            e.0 += 1;
            e.1 += t.elapsed().as_secs_f64();
        }
        citysim::exec::run(&mut w);
        citysim::systems::ownership::run(&mut w);
        citysim::systems::economy::run(&mut w);
        citysim::systems::bind::run(&mut w);
        citysim::systems::law::run(&mut w);
        citysim::systems::social::run(&mut w);
        citysim::systems::gang::run(&mut w);
        citysim::systems::demography::run(&mut w);
        citysim::systems::stats::run(&mut w);
        w.tick += 1;
    }
    let mut v: Vec<_> = acc.into_iter().collect();
    v.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
    let total: f64 = v.iter().map(|x| x.1 .1).sum();
    eprintln!("plan total {:.1} ms", total * 1e3);
    for (k, (n, s)) in v.iter().take(20) {
        eprintln!("{:7.1} ms {n:6} calls {:6.1} us/call  {k}", s * 1e3, s * 1e6 / f64::from(*n));
    }
}

/// M11: starvation deaths over a run, children (under 18) and adults.
#[test]
#[ignore]
fn probe_m11_starvation_by_age() {
    use citysim::{EventKind, Identity};
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let mut w = World::new(42, Config::load());
    let (mut kids, mut adults, mut next) = (0, 0, 0u64);
    for _ in 0..days * 24 {
        w.run_ticks(60);
        for e in
            w.events.iter().filter(|e| e.id >= next && e.kind == EventKind::Death && e.text.ends_with("Starvation"))
        {
            let child = w.comp::<Identity>(e.actors[0]).is_some_and(|i| i.age_years() < 18);
            if child {
                kids += 1;
            } else {
                adults += 1;
            }
        }
        next = w.events.back().map_or(0, |e| e.id + 1);
    }
    eprintln!("starvation over {days} days: {kids} children, {adults} adults");
}

/// M11 phase 3: seed 42, 120 days on the default config. Per Market: owner,
/// price and sales per day (price min/mean/max, sales share by 10 days); per
/// corp: buildings at day 0/60/119 and Food price level while it Undercuts.
#[test]
#[ignore]
fn probe_m11_corps() {
    use citysim::{BuildingKind, Corp, CorpOrder, Market, Niche};
    let mut w = World::new(42, Config::load());
    let markets = w.buildings_of_kind(BuildingKind::Market).to_vec();
    let name = |w: &World, c: Option<citysim::EntityId>| w.owner_label(c);
    let mut prices: Vec<Vec<i64>> = vec![Vec::new(); markets.len()];
    let mut sales: Vec<Vec<u32>> = vec![Vec::new(); markets.len()];
    let mut owners: Vec<Vec<String>> = vec![Vec::new(); markets.len()];
    let mut undercut_days: Vec<(u64, String, f32)> = Vec::new();
    let snapshot = |w: &World| -> Vec<(String, usize, i64)> {
        w.corps()
            .into_iter()
            .filter_map(|c| w.comp::<Corp>(c).map(|cc| (cc.name.clone(), cc.buildings.len(), cc.treasury)))
            .collect()
    };
    let mut snaps = vec![(0u64, snapshot(&w))];
    for day in 0..120u64 {
        w.run_ticks(TICKS_PER_DAY);
        for (i, &m) in markets.iter().enumerate() {
            let mk = w.comp::<Market>(m).expect("market");
            prices[i].push(mk.price_food);
            sales[i].push(mk.sales.back().copied().unwrap_or(0));
            owners[i].push(name(&w, w.owner_of(m)));
        }
        for c in w.corps() {
            let cc = w.comp::<Corp>(c).expect("corp");
            if cc.order == CorpOrder::Undercut && cc.order_niche == Some(Niche::Food) {
                undercut_days.push((day, cc.name.clone(), cc.level(Niche::Food)));
            }
        }
        if day == 59 || day == 119 {
            snaps.push((day + 1, snapshot(&w)));
        }
    }
    for (i, &m) in markets.iter().enumerate() {
        let p = &prices[i];
        let mean = p.iter().sum::<i64>() as f64 / p.len() as f64;
        let mut owner_runs: Vec<(usize, String)> = Vec::new();
        for (d, o) in owners[i].iter().enumerate() {
            if owner_runs.last().is_none_or(|(_, last)| last != o) {
                owner_runs.push((d, o.clone()));
            }
        }
        eprintln!(
            "Market#{} price min {} mean {mean:.2} max {} owners {:?}",
            m.index,
            p.iter().min().unwrap_or(&0),
            p.iter().max().unwrap_or(&0),
            owner_runs
        );
    }
    for d in (0..120).step_by(10) {
        let tot: Vec<u32> = (0..markets.len()).map(|i| sales[i][d..d + 10].iter().sum()).collect();
        let all: u32 = tot.iter().sum::<u32>().max(1);
        let share: Vec<String> = tot.iter().map(|&t| format!("{:.2}", f64::from(t) / f64::from(all))).collect();
        let price: Vec<String> = (0..markets.len())
            .map(|i| format!("{:.1}", prices[i][d..d + 10].iter().sum::<i64>() as f64 / 10.0))
            .collect();
        eprintln!("days {d:3}-{:3}: sales share {share:?} mean price {price:?}", d + 9);
    }
    let mut runs: Vec<(String, u64, u64, f32)> = Vec::new();
    for (d, n, l) in undercut_days {
        match runs.last_mut() {
            Some(r) if r.0 == n && r.2 + 1 == d => {
                r.2 = d;
                r.3 = l;
            }
            _ => runs.push((n, d, d, l)),
        }
    }
    eprintln!("Food Undercut held (corp, from, to, level at end): {runs:?}");
    for (d, s) in snaps {
        eprintln!("day {d}: {s:?}");
    }
}

/// M13 phase 5: who can buy what. Wallets, incomes, treasuries, the assets
/// by owner kind and finance state, gang chrome, and the Shop offers, on a
/// few days of seed 42 (`DAYS`, `SEED`).
#[test]
#[ignore]
fn probe_m13_economy() {
    use citysim::systems::{assets, demography, ownership};
    use citysim::{Asset, AssetKind, AssetLoc, Corp, Gang, GangMember, Job, Kit, Wallet};
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(42);
    let mut w = World::new(seed, Config::load());
    let marks = [1u64, 5, 10, 20, 30, 45, 60, 90, 120];
    for d in 1..=days {
        w.run_ticks(TICKS_PER_DAY);
        if !marks.contains(&d) {
            continue;
        }
        let adults: Vec<_> = w.citizens().into_iter().filter(|&a| demography::is_adult(&w, a)).collect();
        let mut coins: Vec<i64> = adults.iter().map(|&a| w.comp::<Wallet>(a).map_or(0, |x| x.coins)).collect();
        coins.sort_unstable();
        let q = |p: f64| coins[((coins.len() - 1) as f64 * p) as usize];
        let ge = |n: i64| coins.iter().filter(|&&c| c >= n).count();
        let mut wages = std::collections::BTreeMap::new();
        for &a in &adults {
            if let Some(j) = w.comp::<Job>(a) {
                *wages.entry(j.wage_per_day).or_insert(0) += 1;
            }
        }
        eprintln!(
            "day {d}: adults {} coins p50 {} p90 {} p99 {} max {}; >=75 {} >=150 {} >=300 {} >=1000 {}; wages {wages:?}",
            adults.len(),
            q(0.5),
            q(0.9),
            q(0.99),
            coins.last().unwrap(),
            ge(75),
            ge(150),
            ge(300),
            ge(1000)
        );
        for (what, sel) in [("employed", 0), ("members", 1)] {
            let mut c: Vec<i64> = adults
                .iter()
                .filter(|&&a| if sel == 0 { w.has::<Job>(a) } else { w.has::<GangMember>(a) })
                .map(|&a| w.comp::<Wallet>(a).map_or(0, |x| x.coins))
                .collect();
            c.sort_unstable();
            if !c.is_empty() {
                let q = |p: f64| c[((c.len() - 1) as f64 * p) as usize];
                eprintln!(
                    "  {what} {}: p25 {} p50 {} p75 {} p90 {} max {}",
                    c.len(),
                    q(0.25),
                    q(0.5),
                    q(0.75),
                    q(0.9),
                    c[c.len() - 1]
                );
            }
        }
        let gangs: Vec<String> = w
            .gangs()
            .iter()
            .filter_map(|&g| w.comp::<Gang>(g).map(|x| format!("{}:{}m/{}c", x.name, x.members.len(), x.treasury)))
            .collect();
        let corps: Vec<String> =
            w.corps().iter().filter_map(|&c| w.comp::<Corp>(c).map(|x| format!("{}:{}", x.name, x.treasury))).collect();
        eprintln!("  gangs {gangs:?}\n  corps {corps:?}");
        let kinds: Vec<_> = [200i64, 250, 300, 400, 1000]
            .iter()
            .map(|&c| (c, citysim::systems::founding::choose_kind(&w, c)))
            .collect();
        eprintln!("  choose_kind {kinds:?}");
        // Assets by owner kind.
        let mut by = std::collections::BTreeMap::new();
        let (mut fin, mut cash, mut bricked, mut arrears) = (0, 0, 0, 0);
        for a in assets::all_assets(&w) {
            let Some(x) = w.comp::<Asset>(a) else { continue };
            let ok = match ownership::owner_kind(&w, x.owner) {
                ownership::OwnerKind::City => "city",
                ownership::OwnerKind::Corp(_) => "corp",
                ownership::OwnerKind::Gang(_) => "gang",
                ownership::OwnerKind::Agent(_) => "agent",
            };
            let k = match x.kind {
                AssetKind::Implant(_) => "implant",
                k => k.label(),
            };
            let stock = matches!(x.loc, AssetLoc::Stock(_));
            *by.entry((k, ok, stock)).or_insert(0) += 1;
            if x.kind.is_implant() && matches!(x.loc, AssetLoc::Installed(_)) {
                if x.finance.is_some() {
                    fin += 1;
                } else {
                    cash += 1;
                }
                bricked += i32::from(x.bricked);
                arrears += i32::from(x.finance.as_ref().is_some_and(|f| f.arrears > 0));
                if x.finance.as_ref().is_some_and(|f| f.arrears > 0) {
                    let o = x.owner.unwrap_or(citysim::EntityId::NONE);
                    let k = if w.has::<GangMember>(o) {
                        "member"
                    } else if w.has::<Job>(o) {
                        "employed"
                    } else {
                        "dole"
                    };
                    *by.entry(("ARREARS", k, false)).or_insert(0) += 1;
                }
                if x.finance.is_some() {
                    let o = x.owner.unwrap_or(citysim::EntityId::NONE);
                    let k = if w.has::<GangMember>(o) {
                        "member"
                    } else if w.has::<Job>(o) {
                        "employed"
                    } else {
                        "dole"
                    };
                    *by.entry(("FINANCED", k, false)).or_insert(0) += 1;
                }
            }
        }
        eprintln!("  assets (kind, owner, in stock): {by:?}");
        eprintln!("  installed implants financed {fin} cash {cash} bricked {bricked} in arrears {arrears}");
        let members: Vec<_> = w.citizens().into_iter().filter(|&a| w.has::<GangMember>(a)).collect();
        let chromed = members.iter().filter(|&&m| w.comp::<Kit>(m).is_some_and(|k| k.chrome)).count();
        eprintln!("  gang members {} chromed {chromed}", members.len());
        let mut loads = std::collections::BTreeMap::new();
        for &a in &adults {
            if let Some(k) = w.comp::<Kit>(a).filter(|k| k.chrome) {
                *loads.entry((k.load * 100.0).round() as i32).or_insert(0) += 1;
            }
        }
        let mut sanity = std::collections::BTreeMap::new();
        for &a in &adults {
            if let Some(b) = w.comp::<citysim::Body>(a) {
                *sanity.entry((b.sanity * 10.0).floor() as i32).or_insert(0) += 1;
            }
        }
        eprintln!(
            "  loads x100 {loads:?}
  sanity deciles {sanity:?}"
        );
        let (mut offers, mut good) = (std::collections::BTreeMap::new(), 0);
        for &a in &adults {
            if let Some(o) = assets::shop_choice(&w, a, false) {
                *offers.entry(format!("{:?}{}", o.category, if o.financed { "/fin" } else { "" })).or_insert(0) += 1;
                good += usize::from(o.score >= w.config.shop.stat_shop_min);
            }
        }
        eprintln!("  shop offers {offers:?}, score >= stat_shop_min {good}");
    }
}

/// M13 phase 5: per-system wall time, the full tick order and the default
/// (2,000, assets on) config, per day over the whole run (`DAYS`, `SEED`):
/// the mean share per system and the slowest days' breakdown.
#[test]
#[ignore]
fn probe_system_timing_m13() {
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(42);
    let mut w = World::new(seed, Config::load());
    let names = [
        "commands",
        "lod",
        "needs",
        "memory",
        "mood",
        "think",
        "plan",
        "exec",
        "ownership",
        "assets",
        "classes",
        "districts",
        "economy",
        "bind",
        "law",
        "social",
        "gang",
        "corp_brain",
        "demography",
        "stats",
    ];
    const N: usize = 20;
    let mut total = [0f64; N];
    let mut per_day: Vec<(f64, [f64; N])> = Vec::new();
    for _ in 0..days {
        let mut acc = [0f64; N];
        for _ in 0..TICKS_PER_DAY {
            let steps: [&dyn Fn(&mut World); N] = [
                &|w| w.apply_commands(),
                &citysim::systems::lod::run,
                &citysim::needs::run,
                &citysim::systems::memory::run,
                &citysim::mood::run,
                &citysim::systems::think::run,
                &citysim::systems::plan::run,
                &citysim::exec::run,
                &citysim::systems::ownership::run,
                &citysim::systems::assets::run,
                &citysim::systems::classes::run,
                &citysim::systems::districts::run,
                &citysim::systems::economy::run,
                &citysim::systems::bind::run,
                &citysim::systems::law::run,
                &citysim::systems::social::run,
                &citysim::systems::gang::run,
                &citysim::systems::corp_brain::run,
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
        let sum: f64 = acc.iter().sum();
        for i in 0..N {
            total[i] += acc[i];
        }
        per_day.push((sum, acc));
    }
    let all: f64 = total.iter().sum();
    eprintln!("seed {seed}, {days} days: {:.0} ticks/s overall", (days * TICKS_PER_DAY) as f64 / all);
    for (name, secs) in names.iter().zip(total) {
        eprintln!("{name:>11} {:8.1} ms/day {:5.1}%", secs * 1e3 / days as f64, secs / all * 100.0);
    }
    let mut idx: Vec<usize> = (0..per_day.len()).collect();
    idx.sort_by(|&a, &b| per_day[b].0.total_cmp(&per_day[a].0));
    for &d in idx.iter().take(6) {
        let (sum, acc) = &per_day[d];
        let mut top: Vec<(f64, &str)> = acc.iter().copied().zip(names).collect();
        top.sort_by(|a, b| b.0.total_cmp(&a.0));
        let top: Vec<String> = top.iter().take(5).map(|(s, n)| format!("{n} {:.0}", s * 1e3)).collect();
        eprintln!("slow day {d}: {:.0} ticks/s; ms: {}", TICKS_PER_DAY as f64 / sum, top.join(", "));
    }
}

/// M13 phase 5 (H2): why is the law garrisoned? Daily means over the run of
/// the jailed by crime and how many of them are gang members, the posture
/// days, jailbreaks, robot detentions (`SEED`, `DAYS`).
#[test]
#[ignore]
fn probe_m13_law() {
    use citysim::{EventKind, GangMember, Posture, Sentence};
    use std::collections::BTreeMap;
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(42);
    let mut w = World::new(seed, Config::load());
    let mut by_crime: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut posture: BTreeMap<String, u32> = BTreeMap::new();
    let (mut breaks, mut detained, mut next) = (0u32, 0u32, 0u64);
    for _ in 0..days {
        w.run_ticks(TICKS_PER_DAY);
        for e in w.events.iter().filter(|e| e.id >= next) {
            match e.kind {
                EventKind::Jailbreak => breaks += 1,
                EventKind::Arrest if e.text.contains("detained") => detained += 1,
                _ => {}
            }
        }
        next = w.events.back().map_or(next, |e| e.id + 1);
        for p in w.with::<Sentence>() {
            let c = w.comp::<Sentence>(p).map(|s| format!("{:?}", s.crime)).unwrap_or_default();
            let e = by_crime.entry(c).or_default();
            e.0 += 1;
            e.1 += u64::from(w.has::<GangMember>(p));
        }
        let p = w.law().map_or(Posture::Patrol, |l| l.posture);
        *posture.entry(format!("{p:?}")).or_default() += 1;
    }
    eprintln!("seed {seed}: postures {posture:?}, jailbreaks {breaks}, robot detentions {detained}");
    for (c, (n, g)) in by_crime {
        eprintln!(
            "  jailed for {c:<14} mean {:6.1} a day, gang members {:6.1}",
            n as f64 / days as f64,
            g as f64 / days as f64
        );
    }
}

/// M13 phase 5 throughput: the per-think M13 helpers timed over every Full
/// and Coarse agent of seed 42 at day `DAYS` (100), 20 passes each.
#[test]
#[ignore]
fn probe_m13_think_fns() {
    use citysim::systems::{assets, chrome, stims, vehicles};
    use citysim::Lod;
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(100);
    let mut w = World::new(42, Config::load());
    w.run_ticks(days * TICKS_PER_DAY + 600);
    let agents: Vec<_> = w.tier(Lod::Full).iter().chain(w.tier(Lod::Coarse)).copied().collect();
    eprintln!("{} Full+Coarse agents, {} vehicles", agents.len(), w.vehicles.len());
    let time = |name: &str, f: &dyn Fn(citysim::EntityId) -> bool| {
        let t = Instant::now();
        let mut hits = 0;
        for _ in 0..20 {
            for &a in &agents {
                hits += usize::from(f(a));
            }
        }
        let per = t.elapsed().as_secs_f64() / 20.0;
        eprintln!("{name:>18}: {:8.1} us per pass over all, hits {}", per * 1e6, hits / 20);
    };
    time("shop_choice", &|a| assets::shop_choice(&w, a, true).is_some());
    time("theft_plannable", &|a| vehicles::theft_plannable(&w, a));
    time("would_steal", &|a| vehicles::would_steal(&w, a));
    time("steal_target", &|a| vehicles::steal_target(&w, a).is_some());
    time("wants_high", &|a| stims::wants_high(&w, a));
    time("stim_source", &|a| stims::stim_source(&w, a).is_some());
    time("wants_detox", &|a| stims::wants_detox(&w, a));
    time("wants_treatment", &|a| chrome::wants_treatment(&w, a));
    time("loot_target", &|a| chrome::loot_target(&w, a).is_some());
    time("think", &|a| citysim::utility::think(&w, a).is_some());
}

/// M13 phase 5 throughput: `plan::run` re-implemented with a clock around
/// each `plan_for`, by goal, over days `FROM`..120 of seed 42; also each
/// goal's `bind_target` alone.
#[test]
#[ignore]
fn probe_m13_plan_by_goal() {
    use citysim::systems::plan;
    use citysim::Brain;
    use std::collections::BTreeMap;
    let from: u64 = std::env::var("FROM").ok().and_then(|s| s.parse().ok()).unwrap_or(90);
    let mut w = World::new(42, Config::load());
    w.run_ticks(from * TICKS_PER_DAY);
    let mut by: BTreeMap<String, (u32, f64, usize, f64)> = BTreeMap::new();
    for _ in from * TICKS_PER_DAY..120 * TICKS_PER_DAY {
        w.apply_commands();
        citysim::systems::lod::run(&mut w);
        citysim::needs::run(&mut w);
        citysim::systems::memory::run(&mut w);
        citysim::mood::run(&mut w);
        citysim::systems::think::run(&mut w);
        // plan::run, timed.
        let tick = w.tick;
        let (max_plans, max_exp) = (w.config.brain.plan_budget_per_tick, w.config.brain.plan_expansion_budget_per_tick);
        let (mut planned, mut expansions) = (0usize, 0usize);
        while planned < max_plans && expansions < max_exp {
            let Some((&(urgency, id), &enqueued)) = w.plan_queue.iter().next() else { break };
            w.plan_queue.remove(&(urgency, id));
            if let Some(b) = w.comp_mut::<Brain>(id) {
                b.plan_queued = false;
            }
            if tick.saturating_sub(enqueued) > plan::PLAN_QUEUE_MAX_AGE {
                continue;
            }
            let Some(goal) = w.comp::<Brain>(id).filter(|b| b.plan.is_none()).and_then(|b| b.current_goal) else {
                continue;
            };
            planned += 1;
            let t = Instant::now();
            let _ = plan::bind_target(&w, id, goal);
            let tb = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let n = plan::plan_for(&mut w, id, goal);
            let failed = w.comp::<Brain>(id).is_some_and(|b| b.plan.is_none());
            let mut key = format!("{goal:?}{}", if failed { " FAIL" } else { "" });
            if failed && goal == citysim::GoalKind::GangWork {
                let dealer = citysim::systems::stims::dealer_target(&w, id).is_some();
                let carrying = w.comp::<citysim::Inventory>(id).is_some_and(|i| i.stims > 0);
                let order = citysim::systems::gang::following_order(&w, id);
                let target = citysim::systems::gang::gang_work_target(&w, id)
                    .map(|t| w.name_of(t.0).split('#').next().unwrap_or("").to_string());
                let here = w
                    .comp::<citysim::Position>(id)
                    .and_then(|p| p.building)
                    .map(|b| w.name_of(b).split('#').next().unwrap_or("").to_string());
                key =
                    format!("{key} dealer {dealer} carrying {carrying} order {order:?} target {target:?} in {here:?}");
            }
            let e = by.entry(key).or_default();
            e.0 += 1;
            e.1 += t.elapsed().as_secs_f64();
            e.2 += n;
            e.3 += tb;
            expansions += n;
        }
        citysim::exec::run(&mut w);
        citysim::systems::ownership::run(&mut w);
        citysim::systems::assets::run(&mut w);
        citysim::systems::classes::run(&mut w);
        citysim::systems::districts::run(&mut w);
        citysim::systems::economy::run(&mut w);
        citysim::systems::bind::run(&mut w);
        citysim::systems::law::run(&mut w);
        citysim::systems::social::run(&mut w);
        citysim::systems::gang::run(&mut w);
        citysim::systems::corp_brain::run(&mut w);
        citysim::systems::demography::run(&mut w);
        citysim::systems::stats::run(&mut w);
        w.tick += 1;
    }
    let days = (120 - from) as f64;
    let mut rows: Vec<_> = by.into_iter().collect();
    rows.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
    for (g, (n, t, exp, tb)) in rows {
        eprintln!(
            "{g:>12}: {:7.0} plans/day {:7.2} ms/day ({:5.1} us each, bind {:5.1} us), {:6.1} expansions each",
            f64::from(n) / days,
            t * 1e3 / days,
            t * 1e6 / f64::from(n.max(1)),
            tb * 1e6 / f64::from(n.max(1)),
            exp as f64 / f64::from(n.max(1))
        );
    }
}

/// M13 phase 5: why a dealer's GangWork plan exhausts the planner.
#[test]
#[ignore]
fn probe_m13_dealer_plan() {
    use citysim::goap::{self, planner, ActionKind, Limits, PlanCtx, WorldState};
    use citysim::{GangMember, GoalKind};
    let mut w = World::new(42, Config::load());
    w.run_ticks(100 * TICKS_PER_DAY + 600);
    let mut shown = 0;
    for id in w.citizens() {
        if !w.has::<GangMember>(id) || shown >= 6 {
            continue;
        }
        let bar = citysim::systems::stims::dealer_target(&w, id);
        if bar.is_some() == (std::env::var("EXTORT").is_ok()) {
            continue;
        }
        let target = citysim::systems::plan::bind_target(&w, id, GoalKind::GangWork);
        let ctx = PlanCtx::build(&w, id, target);
        let start = WorldState::observe(&w, id, target);
        let gs = goap::goal_state(GoalKind::GangWork).unwrap();
        let r = planner::plan(&ctx, start, &gs, Limits { max_expansions: 5000, max_len: 12 });
        let feas: Vec<ActionKind> = goap::actions::PLANNABLE.iter().copied().filter(|a| a.feasible(&ctx)).collect();
        eprintln!(
            "{} bar {:?} target {:?} dealer {} hideout_stims {} batch {} dist keys {:?}\n  feasible {:?}\n  result {:?}",
            w.name_of(id),
            bar,
            target,
            ctx.dealer,
            ctx.hideout_stims,
            ctx.deal_batch,
            ctx.dist.keys().collect::<Vec<_>>(),
            feas,
            r.as_ref().map(|f| (f.steps.clone(), f.expansions)).map_err(|e| format!("{e:?}"))
        );
        shown += 1;
    }
}

/// M13 phase 5 throughput: `think`'s cost by goal (already_satisfied plus
/// considerations) over the Full and Coarse agents at day `DAYS` (100).
#[test]
#[ignore]
fn probe_m13_think_by_goal() {
    use citysim::utility::goals;
    use citysim::Lod;
    let days: u64 = std::env::var("DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(100);
    let mut w = World::new(42, Config::load());
    w.run_ticks(days * TICKS_PER_DAY + 600);
    let agents: Vec<_> = w.tier(Lod::Full).iter().chain(w.tier(Lod::Coarse)).copied().collect();
    let mut rows = Vec::new();
    for goal in goals::GOAL_ORDER {
        let t = Instant::now();
        for _ in 0..20 {
            for &a in &agents {
                let sp = goals::has_spouse(&w, a);
                if !goals::already_satisfied(&w, a, goal, sp) {
                    let _ = goals::considerations(&w, a, goal, sp);
                }
            }
        }
        rows.push((t.elapsed().as_secs_f64() / 20.0, goal));
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (s, g) in rows.iter().take(12) {
        eprintln!("{g:?}: {:.1} us per pass over {} agents", s * 1e6, agents.len());
    }
}
