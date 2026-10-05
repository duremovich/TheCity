//! M7: level of detail, the Statistical tick, replay and throughput.

use citysim::systems::lod;
use citysim::{
    Brain, Config, ExecState, Household, Lod, Needs, PlayerCommand, Position, Sentence, World, TICKS_PER_DAY,
    TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
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

/// Thefts, hunger-days and arrests per 100 agent-days, Statistical within
/// 15% of Full (floor 0.5). Run with `--ignored`.
#[test]
#[ignore]
fn test_full_vs_statistical_within_15pct() {
    fn metrics(force: Lod) -> (f64, f64, f64) {
        let mut cfg = Config::load();
        cfg.world.population = 200;
        cfg.lod.force = Some(force);
        // The table never modelled gang actions, and a forced-Statistical
        // world has no gang at all: compare the tiers on gangless cities.
        cfg.gangs.max_members = 0;
        let mut w = World::new(2000, cfg);
        let mut hunger_days = 0u64;
        for _ in 0..30 {
            w.run_ticks(TICKS_PER_DAY);
            hunger_days +=
                w.citizens().into_iter().filter(|&id| w.comp::<Needs>(id).is_some_and(|n| n.hunger < 0.2)).count()
                    as u64;
        }
        let agent_days = 200.0 * 30.0 / 100.0;
        let thefts: u32 = w.stats.history.iter().map(|r| r.thefts).sum();
        let arrests: u32 = w.stats.history.iter().map(|r| r.arrests).sum();
        (f64::from(thefts) / agent_days, hunger_days as f64 / agent_days, f64::from(arrests) / agent_days)
    }
    let full = metrics(Lod::Full);
    let stat = metrics(Lod::Statistical);
    for (name, f, s) in [("thefts", full.0, stat.0), ("hunger-days", full.1, stat.1), ("arrests", full.2, stat.2)] {
        let tolerance = (0.15 * f).max(0.5);
        assert!((f - s).abs() <= tolerance, "{name}: Full {f:.2} vs Statistical {s:.2} per 100 agent-days");
    }
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

    let mut b = World::replay(65, Config::load(), &a.command_log, end);
    assert_eq!(b.tick, end);
    assert_eq!(citysim::save::to_ron(&b), saved, "replay diverged");
    b.tick();
}
