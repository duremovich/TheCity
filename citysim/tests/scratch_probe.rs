//! Ignored probes for by-hand investigation. Run one with
//! `cargo test --release -p citysim --test scratch_probe -- --ignored --nocapture <name>`.

use std::time::Instant;

use citysim::{Config, World, TICKS_PER_DAY};

/// Per-system wall time over one day after a long warm-up: where the ticks go.
#[test]
#[ignore]
fn probe_system_timing() {
    let warm_days: u64 = std::env::var("WARM_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(50);
    let mut w = World::new(42, Config::load());
    let t0 = Instant::now();
    w.run_ticks(warm_days * TICKS_PER_DAY);
    eprintln!("warm-up {warm_days} days in {:.1}s, edges {}", t0.elapsed().as_secs_f32(), w.edges.len());
    let names = [
        "commands", "lod", "needs", "memory", "mood", "think", "plan", "exec", "economy", "law", "social", "gang",
        "stats",
    ];
    let mut acc = [0f64; 13];
    for _ in 0..TICKS_PER_DAY {
        let steps: [&dyn Fn(&mut World); 13] = [
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
    let mut w = World::new(43, Config::load());
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
    let mut w = World::new(43, Config::load());
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
    let mut w = World::new(42, Config::load());
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
