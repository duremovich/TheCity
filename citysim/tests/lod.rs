//! M7: level of detail, the Statistical tick, replay and throughput.

use citysim::systems::lod;
use citysim::{
    Brain, Config, ExecState, Household, Lod, Needs, PlayerCommand, Position, Sentence, World, TICKS_PER_DAY,
    TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

fn counts(w: &World) -> (usize, usize, usize) {
    let mut n = (0, 0, 0);
    for id in w.citizens() {
        match w.comp::<Brain>(id).map(|b| b.lod) {
            Some(Lod::Full) => n.0 += 1,
            Some(Lod::Coarse) => n.1 += 1,
            Some(Lod::Statistical) => n.2 += 1,
            None => {}
        }
    }
    n
}

#[test]
fn test_lod_assignment_counts() {
    let mut w = world(61);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let (full, coarse, stat) = counts(&w);
    let jailed = w.citizens().into_iter().filter(|&id| w.has::<Sentence>(id)).count();
    assert!(full <= w.config.lod.max_full, "full {full}");
    assert!(coarse <= w.config.lod.max_coarse + jailed, "coarse {coarse}");
    assert!(stat > 0, "nobody statistical");
    assert_eq!(full + coarse + stat, w.citizens().into_iter().filter(|&id| w.has::<Brain>(id)).count());
}

#[test]
fn test_pinned_agent_is_full() {
    let mut w = world(62);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let id = w
        .citizens()
        .into_iter()
        .find(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical))
        .expect("a statistical agent");
    w.push_command(PlayerCommand::Pin(id, true));
    w.run_ticks(TICKS_PER_HOUR);
    assert_eq!(w.comp::<Brain>(id).map(|b| b.lod), Some(Lod::Full));
    assert!(w.comp::<Brain>(id).is_some_and(|b| b.pinned));
}

#[test]
fn test_demote_full_to_coarse_keeps_plan() {
    let mut w = world(63);
    // Find a Full agent mid-walk.
    let mut found = None;
    for _ in 0..(4 * TICKS_PER_HOUR) {
        w.tick();
        found = w.citizens().into_iter().find(|&id| {
            w.comp::<Brain>(id)
                .is_some_and(|b| b.lod == Lod::Full && b.plan.is_some() && matches!(b.exec, ExecState::Goto { .. }))
        });
        if found.is_some() {
            break;
        }
    }
    let id = found.expect("a walking Full agent");
    let (plan, step) = {
        let b = w.comp::<Brain>(id).expect("brain");
        (b.plan.clone(), b.plan_step)
    };
    lod::set_lod(&mut w, id, Lod::Coarse);
    let b = w.comp::<Brain>(id).expect("brain");
    assert_eq!(b.lod, Lod::Coarse);
    assert_eq!(b.plan, plan, "plan kept");
    assert_eq!(b.plan_step, step, "step kept");
    assert!(matches!(b.exec, ExecState::GotoTimed { .. }), "walk became a timed arrival: {:?}", b.exec);
}

#[test]
fn test_demote_to_statistical_snaps_home_at_night() {
    let mut w = world(64);
    w.tick = 100; // 01:40, Night
    let id = w.citizens().into_iter().find(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Coarse)).expect("c");
    let home = w.comp::<Household>(id).and_then(|h| h.home).expect("home");
    let door = w.comp::<citysim::Building>(home).expect("b").door;
    // Put them somewhere else first.
    w.leave_building(id);
    w.comp_mut::<Position>(id).expect("pos").tile = citysim::TilePos { x: 3, y: 3 };
    lod::set_lod(&mut w, id, Lod::Statistical);
    let pos = w.comp::<Position>(id).expect("pos");
    assert_eq!(pos.tile, door);
    assert_eq!(pos.building, None);
    assert_eq!(w.comp::<Brain>(id).map(|b| b.lod), Some(Lod::Statistical));
}

#[test]
fn test_statistical_hourly_decay_equals_60_ticks() {
    let cfg = Config::load().needs;
    let ctx =
        citysim::needs::DecayCtx { sociability: 0.5, season_energy_mult: 1.0, ..citysim::needs::DecayCtx::plain() };
    let start = Needs {
        hunger: 0.8,
        energy: 0.7,
        safety: 0.6,
        wealth: 0.3,
        belonging: 0.5,
        intimacy: 0.4,
        starving_since: None,
    };
    let mut hourly = start.clone();
    citysim::needs::decay(&mut hourly, &cfg, &ctx, TICKS_PER_HOUR as u32);
    let mut stepped = start;
    for _ in 0..TICKS_PER_HOUR {
        citysim::needs::decay(&mut stepped, &cfg, &ctx, 1);
    }
    for (a, b) in [
        (hourly.hunger, stepped.hunger),
        (hourly.energy, stepped.energy),
        (hourly.safety, stepped.safety),
        (hourly.belonging, stepped.belonging),
        (hourly.intimacy, stepped.intimacy),
    ] {
        assert!((a - b).abs() < 1e-5, "{a} vs {b}");
    }
}

/// Parity v2 (M10 § 7, D37): Full vs Statistical on the 2,000 map's city
/// scaled to 500, gangless, 30 days, seeds 2000-2005 pooled. Thefts, hunger-days, arrests,
/// assaults, violent deaths and marriages per 100 agent-days within 15%
/// (floor 0.5); then, with every hole bound, the share of attributed crimes
/// per actor lawfulness bucket within `max(0.15 x share, 0.05)`. Run with
/// `--ignored`.
#[test]
#[ignore]
fn test_full_vs_statistical_within_15pct() {
    use citysim::systems::bind;
    use citysim::{EventKind, Personality};
    const AGENTS: u32 = 500;
    const DAYS: u64 = 30;
    const SEEDS: [u64; 6] = [2000, 2001, 2002, 2003, 2004, 2005];
    struct Run {
        rates: [f64; 6],
        actors: [f64; 3],
    }
    fn bucket(w: &World, id: citysim::EntityId) -> Option<usize> {
        let l = w.comp::<Personality>(id)?.lawfulness;
        Some(if l < 0.3 {
            0
        } else if l < 0.7 {
            1
        } else {
            2
        })
    }
    fn run(force: Lod, seed: u64) -> Run {
        // The city `calibrate` measured: gangless, full Warehouse.
        let mut cfg = Config::load().calibration_city(AGENTS);
        cfg.lod.force = Some(force);
        cfg.lod.stat_violence_mult = 1.0;
        // The learned-policy experiment: `CITYSIM_STAT_POLICY=mlp`.
        if let Ok(p) = std::env::var("CITYSIM_STAT_POLICY") {
            cfg.lod.policy = p;
        }
        let mut w = World::new(seed, cfg);
        let mut hunger_days = 0u64;
        let (mut assaults, mut marriages) = (0u64, 0u64);
        let mut actors = [0u64; 3];
        let mut cursor = 0u64;
        // Actor buckets are read when the event is seen (end of its day).
        let mut split = [[0u64; 3]; 2];
        let mut scan = |w: &World, cursor: &mut u64, assaults: &mut u64, marriages: &mut u64, actors: &mut [u64; 3]| {
            for e in w.events.iter().filter(|e| e.id >= *cursor) {
                match e.kind {
                    EventKind::Assault | EventKind::Assaulted => *assaults += 1,
                    EventKind::Marriage => *marriages += 1,
                    _ => {}
                }
                let actor_side = match force {
                    Lod::Full => matches!(e.kind, EventKind::Theft | EventKind::Assault | EventKind::Murder),
                    // A Full robbery is one Theft by its thief. Off screen
                    // the thief's own `p_steal` roll is that Theft, and the
                    // victim's `p_robbed` hole is the same robbery seen from
                    // the other side: counting its bound actor too counted
                    // every robbery twice, at the binder's (1 - l)^2 weights
                    // (M10 phase 5b, once the fed calibration city's thefts
                    // became mostly robberies).
                    //
                    // Assaults and murders count on both sides: a forced
                    // Statistical city keeps guards, gravediggers and the
                    // wanted as bodies, and their fights are Full events with
                    // a named actor (an off-screen victim's Murder names
                    // nobody, so `bucket` skips it). Counting only thefts
                    // here dropped the bodies' violence from this side alone
                    // (M10 phase 5c).
                    _ => {
                        matches!(e.kind, EventKind::Theft | EventKind::Assault | EventKind::Murder)
                            || (e.kind == EventKind::Attributed
                                && e.actors.len() == 2
                                && !e.text.contains(citysim::HoleKind::Robbed.noun()))
                    }
                };
                if actor_side {
                    if let Some(b) = e.actors.first().and_then(|&a| bucket(w, a)) {
                        actors[b] += 1;
                        // Thefts apart from assaults, murders and bound holes, for the diagnosis.
                        let kind = usize::from(e.kind != EventKind::Theft);
                        split[kind][b] += 1;
                    }
                }
            }
            *cursor = w.next_event_id;
        };
        for _ in 0..DAYS {
            w.run_ticks(TICKS_PER_DAY);
            hunger_days +=
                w.citizens().into_iter().filter(|&id| w.comp::<Needs>(id).is_some_and(|n| n.hunger < 0.2)).count()
                    as u64;
            scan(&w, &mut cursor, &mut assaults, &mut marriages, &mut actors);
        }
        bind::bind_all(&mut w);
        scan(&w, &mut cursor, &mut assaults, &mut marriages, &mut actors);
        let pop: [usize; 3] =
            std::array::from_fn(|b| w.citizens().into_iter().filter(|&id| bucket(&w, id) == Some(b)).count());
        eprintln!(
            "{force:?} seed {seed}: thefts by bucket {:?}, violence/attributed by bucket {:?}, population {pop:?}",
            split[0], split[1]
        );
        let per = f64::from(AGENTS) * DAYS as f64 / 100.0;
        let sum = |f: fn(&citysim::DayRow) -> u32| w.stats.history.iter().map(f).sum::<u32>() as f64 / per;
        Run {
            rates: [
                sum(|r| r.thefts),
                hunger_days as f64 / per,
                sum(|r| r.arrests),
                assaults as f64 / per,
                sum(|r| r.deaths_violence),
                marriages as f64 / per,
            ],
            actors: actors.map(|n| n as f64),
        }
    }
    // Six cities per tier, pooled: one 30-day city's theft rate varies by a
    // quarter from seed to seed (seed 2000 alone runs 4.1 against 2.9-3.6),
    // and the actor shares, which a few repeat thieves dominate, moved 0.04
    // on the Full side alone between seeds 2000-2002 and 2003-2005 (law0
    // 0.204 vs 0.242) against a 0.05 tolerance (M10 review fixes).
    let pooled = |force: Lod| {
        let runs: Vec<Run> = SEEDS.iter().map(|&s| run(force, s)).collect();
        let n = runs.len() as f64;
        let rates = std::array::from_fn(|i| runs.iter().map(|r| r.rates[i]).sum::<f64>() / n);
        let counts: [f64; 3] = std::array::from_fn(|b| runs.iter().map(|r| r.actors[b]).sum::<f64>());
        let total = counts.iter().sum::<f64>().max(1.0);
        Run { rates, actors: counts.map(|c| c / total) }
    };
    let full = pooled(Lod::Full);
    let stat = pooled(Lod::Statistical);
    let names = ["thefts", "hunger-days", "arrests", "assaults", "violent deaths", "marriages"];
    let mut failures = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let (f, s) = (full.rates[i], stat.rates[i]);
        let tolerance = (0.15 * f).max(0.5);
        eprintln!("{name:15} Full {f:7.3}  Statistical {s:7.3}  per 100 agent-days (tolerance {tolerance:.2})");
        if (f - s).abs() > tolerance {
            failures.push(format!("{name}: Full {f:.3} vs Statistical {s:.3}"));
        }
    }
    for b in 0..3 {
        let (f, s) = (full.actors[b], stat.actors[b]);
        let tolerance = (0.15 * f).max(0.05);
        eprintln!("actor share law{b}  Full {f:.3}  Statistical {s:.3}  (tolerance {tolerance:.3})");
        if (f - s).abs() > tolerance {
            failures.push(format!("actor share law{b}: Full {f:.3} vs Statistical {s:.3}"));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn test_headless_throughput_8000_tps() {
    if cfg!(debug_assertions) {
        eprintln!("release only");
        return;
    }
    let mut w = world(42);
    let t0 = std::time::Instant::now();
    w.run_ticks(10 * TICKS_PER_DAY);
    let tps = (10 * TICKS_PER_DAY) as f64 / t0.elapsed().as_secs_f64();
    assert!(tps >= 8000.0, "{tps:.0} ticks/s");
}

#[test]
fn test_command_log_replay_matches() {
    let mut a = world(65);
    let who = a.citizens().into_iter().find(|&id| a.has::<Brain>(id)).expect("agent");
    let cmds = [
        (100, PlayerCommand::SetTaxRate(0.1)),
        (500, PlayerCommand::GrantCoins { agent: who, amount: 20 }),
        (900, PlayerCommand::ReleaseReserve { amount: 100 }),
        (1300, PlayerCommand::Pin(who, true)),
        (1700, PlayerCommand::SetDolePerDay(4)),
    ];
    let end = 2 * TICKS_PER_DAY;
    for (t, cmd) in &cmds {
        while a.tick < *t {
            a.tick();
        }
        a.push_command(cmd.clone());
    }
    while a.tick < end {
        a.tick();
    }
    assert_eq!(a.command_log.len(), 5);
    let saved = citysim::save::to_ron(&a);

    let mut b = World::replay(65, Config::load().v1_profile(), &a.command_log, end);
    assert_eq!(b.tick, end);
    assert_eq!(citysim::save::to_ron(&b), saved, "replay diverged");
    b.tick();
}

/// The learned-policy experiment: microseconds per agent-hour row, table vs
/// MLP (features + both forward passes), over the 2,000 city's agents after a
/// day. Needs `assets/stat_mlp.toml`. Run with `--ignored`.
#[test]
#[ignore]
fn bench_stat_policy_row() {
    use citysim::systems::stat_policy::{features, MlpPolicy, StatMlp, StatPolicy, TablePolicy};
    let mut w = World::new(42, Config::load());
    w.run_ticks(TICKS_PER_DAY + 7 * TICKS_PER_HOUR);
    let ids: Vec<_> = w.citizens().into_iter().filter(|&id| w.has::<Brain>(id)).collect();
    let mlp = StatMlp::load(&w.config);
    let table = w.stat_table.clone().expect("table");
    const REPS: usize = 200;
    let time = |name: &str, f: &dyn Fn(citysim::EntityId) -> f32| {
        let t0 = std::time::Instant::now();
        let mut acc = 0.0f32;
        for _ in 0..REPS {
            for &id in &ids {
                acc += f(id);
            }
        }
        let us = t0.elapsed().as_secs_f64() * 1e6 / (REPS * ids.len()) as f64;
        eprintln!("{name:22} {us:.3} us per agent-hour ({} agents, checksum {acc:.1})", ids.len());
    };
    time("table", &|id| TablePolicy(&table).row(&w, id).p_eat);
    time("features only", &|id| features(&w, id)[6]);
    time("mlp (features + nets)", &|id| MlpPolicy(&mlp).row(&w, id).p_eat);
    let x: Vec<_> = ids.iter().map(|&id| features(&w, id)).collect();
    let t0 = std::time::Instant::now();
    let mut acc = 0.0f32;
    for _ in 0..REPS {
        for f in &x {
            acc += mlp.row_for(f).p_steal;
        }
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / (REPS * x.len()) as f64;
    eprintln!("{:22} {us:.3} us per agent-hour (checksum {acc:.3})", "mlp nets only");
}
