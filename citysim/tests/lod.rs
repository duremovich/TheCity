//! M7: level of detail, the Statistical tick and replay (the ticks/s floor is
//! `core_sanity`'s, tests/core.rs).

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
        fun: 1.0,
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
/// `--ignored` (nightly, docs/TESTING.md: the one parity test kept).
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

/// Jobs and room P2 fix (seed 44's wages-city wave: 345 Statistical agents
/// starved beside an empty Market while the other two held 1,200 each): a
/// hungry Statistical agent whose Market is empty buys, or steals when
/// starving and broke, at a stocked one, and the price pick passes the empty
/// shelf over. Only the Statistical pass runs, for one hour (one turn each).
#[test]
fn test_statistical_agent_eats_at_a_stocked_market_when_its_own_is_empty() {
    use citysim::{Building, BuildingKind, EventKind, Inventory, Job, Wallet};
    for broke in [false, true] {
        let mut w = World::new(42, Config::load());
        w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
        let id = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| {
                citysim::systems::demography::is_adult(&w, a)
                    && !w.has::<Sentence>(a)
                    && w.comp::<Job>(a).and_then(|j| j.employer).is_none()
            })
            .expect("a jobless Statistical adult");
        let empty = w.local(id, BuildingKind::Market).expect("a Market");
        w.comp_mut::<Building>(empty).expect("market").stock_food = 0;
        let other = w.local(id, BuildingKind::Market).expect("a Market");
        assert_ne!(other, empty, "the price pick passes the empty shelf over");
        let stock = w.comp::<Building>(other).map_or(0, |b| b.stock_food);
        assert!(stock > 0);
        assert_eq!(w.food_market(id), Some(other));
        // Homeless: no pantry a housemate could refill within the hour.
        w.set_home(id, None);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Wallet>(id).expect("wallet").coins = if broke { 0 } else { 100 };
        if broke {
            // No dole to buy with: the starving agent's only meal is a theft.
            w.levers.dole_per_day = 0;
        }
        {
            let n = w.comp_mut::<Needs>(id).expect("needs");
            n.hunger = if broke { 0.0 } else { 0.2 };
            n.starving_since = None;
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        for _ in 0..TICKS_PER_HOUR {
            lod::run_statistical(&mut w);
            w.tick += 1;
        }
        assert!(w.comp::<Needs>(id).is_some_and(|n| n.hunger > 0.2), "broke {broke}: the agent ate");
        assert_eq!(w.comp::<Building>(empty).map(|b| b.stock_food), Some(0));
        let left = w.comp::<Building>(other).map_or(0, |b| b.stock_food);
        assert!(left < stock, "broke {broke}: the food came from the stocked Market");
        if broke {
            let theft = w
                .events
                .iter()
                .any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.as_slice() == [id, other]);
            assert!(theft, "the starving thief stole at the stocked Market");
        }
    }
}

/// The flip-readiness round's no-net city at day 1, 10:00, and a jobless
/// homeless Statistical adult in it with nothing to eat and no coins (no
/// pantry a housemate could refill within the hour). The table's no-net
/// rates are set by each test.
fn no_net_stat_agent() -> (World, citysim::EntityId) {
    use citysim::{Inventory, Job, Wallet};
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg.economy2.no_safety_net = true;
    let mut w = World::new(42, cfg);
    assert!(citysim::systems::econ::no_net(&w));
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let id = w
        .tier(Lod::Statistical)
        .iter()
        .copied()
        .find(|&a| {
            citysim::systems::demography::is_adult(&w, a)
                && !w.has::<Sentence>(a)
                && w.comp::<Job>(a).and_then(|j| j.employer).is_none()
        })
        .expect("a jobless Statistical adult");
    w.set_home(id, None);
    w.comp_mut::<Inventory>(id).expect("inv").food = 0;
    w.comp_mut::<Inventory>(id).expect("inv").stolen_food = 0;
    w.comp_mut::<Wallet>(id).expect("wallet").coins = 0;
    if let Some(t) = w.stat_table.as_mut() {
        t.p_scavenge = Some([0.0; citysim::STAT_ROWS]);
        t.p_beg = Some([0.0; citysim::STAT_ROWS]);
        t.p_desperate = Some([0.0; 3]);
    }
    (w, id)
}

/// One hour of the Statistical pass alone (each agent's one turn).
fn stat_hour(w: &mut World) {
    for _ in 0..TICKS_PER_HOUR {
        lod::run_statistical(w);
        w.tick += 1;
    }
}

/// Flip readiness, the no-net income path: a broke jobless Statistical
/// adult scavenges on its row's `p_scavenge` and the find is paid from the
/// Recycler's till (`jobs::scavenge_find`, Full's Scavenge); a table without
/// the rate (the dole city's) never scavenges off screen.
#[test]
fn test_no_net_statistical_scavenge_is_paid_from_the_till() {
    use citysim::Wallet;
    for rated in [true, false] {
        let (mut w, id) = no_net_stat_agent();
        w.config.life.scavenge_p = 1.0;
        // Every broke jobless Statistical adult scavenges this hour: a till
        // deep enough for all of them.
        w.econ.recycler_till = 1_000_000;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 1.0;
        if let Some(t) = w.stat_table.as_mut() {
            t.p_scavenge = rated.then_some([1.0; citysim::STAT_ROWS]);
        }
        let till = w.econ.recycler_till;
        stat_hour(&mut w);
        let coins = w.comp::<Wallet>(id).map_or(0, |x| x.coins);
        let paid = w.config.treasury.scrap_coins;
        if rated {
            assert_eq!(coins, paid, "the hour's find paid {paid}");
            assert!(w.econ.recycler_till <= till - paid, "from the till");
        } else {
            assert_eq!(coins, 0, "no rate, no off-screen scavenging");
        }
    }
}

/// Flip readiness: with no net a hungry Statistical agent who cannot buy
/// steals by its lawfulness bucket's `p_desperate` (the lawful go hungry, as
/// the Full planner's theft cost has them), and a theft takes a Full Market
/// theft's two units (one eaten at once).
#[test]
fn test_no_net_need_theft_follows_lawfulness() {
    use citysim::{EventKind, Inventory, Personality};
    for lawful in [false, true] {
        let (mut w, id) = no_net_stat_agent();
        w.comp_mut::<Personality>(id).expect("personality").lawfulness = if lawful { 0.9 } else { 0.1 };
        {
            let n = w.comp_mut::<Needs>(id).expect("needs");
            n.hunger = 0.2;
            n.starving_since = None;
        }
        if let Some(t) = w.stat_table.as_mut() {
            t.p_desperate = Some([1.0, 0.0, 0.0]);
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        stat_hour(&mut w);
        let stole =
            w.events.iter().any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.first() == Some(&id));
        if lawful {
            assert!(!stole, "a lawful agent goes hungry rather than steal");
            assert_eq!(w.comp::<Inventory>(id).map(|i| i.food), Some(0));
        } else {
            assert!(stole, "the lawless steal the meal they cannot buy");
            assert!(w.comp::<Needs>(id).is_some_and(|n| n.hunger > 0.2), "and eat it");
            assert_eq!(w.comp::<Inventory>(id).map(|i| (i.food, i.stolen_food)), Some((1, 1)), "two taken, one eaten");
        }
    }
}

/// Flip readiness (night porter): with wages on, a worker promoted on its
/// own shift stands at its workplace whatever the phase (a night shift
/// starts at the Hotel, not across town), and a homeless worker off shift
/// at night stands at its workplace rather than the cheapest Market.
#[test]
fn test_night_shift_worker_promoted_at_the_workplace() {
    use citysim::{Building, Job, Position, TilePos};
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg.economy2.no_safety_net = true;
    let mut w = World::new(42, cfg);
    let role = w.config.trade_role("night_porter").expect("the Night Porter trade");
    // Day 1 (a workday), 00:30: the night shift is on.
    w.run_ticks(TICKS_PER_DAY + 30);
    let p = w
        .workers(role)
        .iter()
        .copied()
        .find(|&a| {
            !w.has::<Sentence>(a)
                && w.comp::<Job>(a).is_some_and(|j| {
                    j.on_shift(w.tick_of_day()) && citysim::exec::routine::workday_of(&w, a, j, j.shift_key_at(w.tick))
                })
        })
        .expect("a porter on shift");
    let hotel = w.comp::<Job>(p).and_then(|j| j.employer).expect("its Hotel");
    let door = w.comp::<Building>(hotel).expect("hotel").door;
    let far = TilePos { x: 3, y: 3 };
    for homeless in [false, true] {
        if homeless {
            w.set_home(p, None);
        }
        lod::set_lod(&mut w, p, Lod::Statistical);
        w.leave_building(p);
        w.comp_mut::<Position>(p).expect("pos").tile = far;
        lod::set_lod(&mut w, p, Lod::Full);
        assert_eq!(w.comp::<Position>(p).map(|x| x.tile), Some(door), "homeless {homeless}: at the Hotel on shift");
    }
    // Off shift at night (a day worker's hours), a homeless worker sleeps near the work.
    if let Some(j) = w.comp_mut::<Job>(p) {
        j.shifts = vec![(600, 1080)];
    }
    lod::set_lod(&mut w, p, Lod::Statistical);
    assert_eq!(w.comp::<Position>(p).map(|x| x.tile), Some(door), "a homeless worker's night at the workplace");
}

/// Flip readiness (review): `p_desperate` is learned from adults, so a
/// Statistical minor keeps the M10 rule: hungry is no theft, starving is.
#[test]
fn test_no_net_child_keeps_the_starving_theft_rule() {
    use citysim::{EventKind, Inventory, Wallet};
    for starving in [false, true] {
        let mut cfg = Config::load();
        cfg.economy2.wages = true;
        cfg.economy2.no_safety_net = true;
        let mut w = World::new(42, cfg);
        w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
        let id = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| citysim::systems::demography::is_adult(&w, a) && !w.has::<Sentence>(a))
            .expect("a Statistical agent");
        // Under age (a minor with a Brain: the off-screen eat path's child).
        w.comp_mut::<citysim::Identity>(id).expect("identity").age_days =
            citysim::systems::demography::ADULT_AGE_DAYS - 1;
        assert!(!citysim::systems::demography::is_adult(&w, id));
        w.set_home(id, None);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Inventory>(id).expect("inv").stolen_food = 0;
        if let Some(x) = w.comp_mut::<Wallet>(id) {
            x.coins = 0;
        }
        if let Some(t) = w.stat_table.as_mut() {
            t.p_scavenge = Some([0.0; citysim::STAT_ROWS]);
            t.p_beg = Some([0.0; citysim::STAT_ROWS]);
            t.p_desperate = Some([1.0; 3]);
        }
        {
            let n = w.comp_mut::<Needs>(id).expect("needs");
            n.hunger = if starving { 0.0 } else { 0.2 };
            n.starving_since = None;
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        stat_hour(&mut w);
        let stole =
            w.events.iter().any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.first() == Some(&id));
        assert_eq!(stole, starving, "starving {starving}: a child steals only when starving");
    }
}
