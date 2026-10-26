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
        status: 1.0,
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

/// Jobs and room P2 fix (seed 44's wages-city wave: 345 Statistical agents
/// starved beside an empty Market while the other two held 1,200 each): a
/// hungry Statistical agent whose Market is empty buys, or steals when
/// starving and broke, at a stocked one, and the price pick passes the empty
/// shelf over. Only the Statistical pass runs, for one hour (one turn each).
#[test]
fn test_statistical_agent_eats_at_a_stocked_market_when_its_own_is_empty() {
    use citysim::{Building, BuildingKind, EventKind, Inventory, Job, Wallet};
    for broke in [false, true] {
        let mut w = World::new(42, Config::load().dole_city());
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
        // The meal's mechanics, not its delay: the M10 rule (every hungry
        // hour eats), not the table's learned `p_meal`.
        if let Some(t) = w.stat_table.as_mut() {
            t.p_meal = None;
        }
        // Homeless: no pantry a housemate could refill within the hour.
        w.set_home(id, None);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Wallet>(id).expect("wallet").coins = if broke { 0 } else { 100 };
        if broke {
            // No dole to buy with: the starving agent's only meal is a theft.
            w.levers.dole_per_day = 0;
            // The M10 rule (a table without `p_desperate`: a starving adult
            // steals whatever its lawfulness); the table's `p_desperate`
            // roll is the no-net tests' below.
            if let Some(t) = w.stat_table.as_mut() {
                t.p_desperate = None;
                // Nor a scavenged or begged coin to buy with (the earn path; on
                // the transit phase 3 table this agent scavenged its meal).
                t.p_scavenge = None;
                t.p_beg = None;
            }
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
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    assert!(citysim::systems::econ::wages_on(&w) && !citysim::systems::policy::net_enacted(&w));
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

/// Parity (the bin meals red): the off-screen income path has the Full
/// Scavenge's and Beg's gate (the planner's `ctx.poor`), not the Earn
/// goal's savings line: a Full jobless adult with a meal's coins (and,
/// homeless, a Hotel night's) neither scavenged nor begged (seed 2000,
/// all-Full: 0.196 scavenges per hour at 0 coins, 0.012 at 1, under 0.01
/// above), so off screen it does not either; a coin short of the line, it
/// does. Shadow V4 bug 1: the gate is `life::earn_short` (with no dole, under
/// `[life] earn_meals_no_net` meals' coins too), the line its edge.
#[test]
fn test_statistical_scavenge_only_when_broke() {
    use citysim::Wallet;
    for short in [false, true] {
        let (mut w, id) = no_net_stat_agent();
        w.config.life.scavenge_p = 1.0;
        w.econ.recycler_till = 1_000_000;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 1.0;
        let price = w.local(id, citysim::BuildingKind::Market).map_or(1, |m| w.price_for(m, id));
        let line = (citysim::systems::life::meal_price(&w, id) + w.config.street.night_price)
            .max(w.config.life.earn_meals_no_net * price);
        assert!(line < citysim::goap::world_state::SAVINGS_DAYS * price, "below the savings line either way");
        w.comp_mut::<Wallet>(id).expect("wallet").coins = if short { line - 1 } else { line };
        assert_eq!(citysim::systems::life::earn_short(&w, id), short);
        if let Some(t) = w.stat_table.as_mut() {
            t.p_scavenge = Some([1.0; citysim::STAT_ROWS]);
            // The near-broke's own rate (Shadow V4 bug 1's table field).
            t.p_scavenge_short = Some([1.0; citysim::STAT_ROWS]);
        }
        let till = w.econ.recycler_till;
        stat_hour(&mut w);
        let after = w.comp::<Wallet>(id).map_or(0, |x| x.coins);
        if short {
            assert!(after >= line - 1 + w.config.treasury.scrap_coins, "broke: the hour's find ({after})");
            assert!(w.econ.recycler_till < till, "paid from the till");
        } else {
            assert!(after <= line, "a meal and a night in hand: no scavenging ({after})");
        }
    }
}

/// Parity (the law1 actor share): with the table's `p_steal_broke` the
/// off-screen hour's theft roll is the broke adult's alone, as a Full agent
/// with a meal's coins all but never steals (seed 2000, all-Full: 1 theft
/// of ~240 by a thief who could buy); the rows' `p_steal` is not read. A
/// table without it keeps the rows' roll for everyone.
#[test]
fn test_statistical_theft_roll_is_the_broke_adults() {
    use citysim::{EventKind, Wallet};
    for (learned, broke) in [(true, true), (true, false), (false, false)] {
        let (mut w, id) = no_net_stat_agent();
        w.comp_mut::<Needs>(id).expect("needs").hunger = 1.0;
        let price = citysim::systems::life::meal_price(&w, id);
        w.comp_mut::<Wallet>(id).expect("wallet").coins = if broke { price - 1 } else { 50 * price };
        if let Some(t) = w.stat_table.as_mut() {
            for r in &mut t.rows {
                r.p_steal = 1.0;
            }
            t.p_steal_broke = learned.then_some([1.0; citysim::STAT_ROWS]);
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        stat_hour(&mut w);
        let stole =
            w.events.iter().any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.first() == Some(&id));
        assert_eq!(stole, broke || !learned, "learned {learned}, broke {broke}");
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
    let cfg = Config::load();
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
        let cfg = Config::load();
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

/// Hunger as sickness P5h (a harness change): the all-Full calibration and
/// parity cities scale the planner budgets by n / 500 (never below the
/// shipped values), so a 2,000-agent Full city is not a planner-starved one.
#[test]
fn test_calibration_city_scales_the_planner_budget() {
    let base = Config::load();
    let (p, e) = (base.brain.plan_budget_per_tick, base.brain.plan_expansion_budget_per_tick);
    let at = |n: u32| {
        let c = Config::load().calibration_city(n);
        (c.brain.plan_budget_per_tick, c.brain.plan_expansion_budget_per_tick)
    };
    assert_eq!(at(500), (p, e));
    assert_eq!(at(250), (p, e), "never below the shipped budget");
    assert_eq!(at(2000), (4 * p, 4 * e));
}

/// Hunger as sickness P5h, a printed finding (not an assert): the planner
/// queue in the shipped mixed-LOD city on seed 42 (does a busy on-screen
/// crowd starve the planner in the real game too?). Max and mean queue
/// length sampled hourly, and the stale drops, over 3 days.
#[test]
fn finding_planner_queue_shipped_city() {
    let mut w = World::new(42, Config::load());
    let mut lens = Vec::new();
    let mut full_max = 0;
    for _ in 0..(3 * 24) {
        w.run_ticks(TICKS_PER_HOUR);
        lens.push(w.plan_queue.len());
        full_max = full_max.max(counts(&w).0);
    }
    let max = lens.iter().copied().max().unwrap_or(0);
    let mean = lens.iter().sum::<usize>() as f64 / lens.len().max(1) as f64;
    eprintln!(
        "FINDING planner queue (shipped, seed 42, 3 days): budget {}/tick; queue max {max} mean {mean:.1}; stale drops {}; Full agents max {full_max}",
        w.config.brain.plan_budget_per_tick, w.plan_stale_drops
    );
}

/// Diet parity (hunger fix item 11's off-screen side): an employed
/// Statistical agent off shift, with a meal's coins and an empty household
/// pantry, stocks the pantry in its hour (the Full worker's grocery run);
/// with item 11 off it does not.
#[test]
fn test_statistical_worker_off_shift_stocks_the_pantry() {
    use citysim::{Household, Inventory, Job, Needs, Wallet};
    for on in [true, false] {
        let mut cfg = Config::load();
        cfg.life.hf_shift_meals = on;
        let mut w = World::new(42, cfg);
        w.run_ticks(TICKS_PER_DAY + 20 * TICKS_PER_HOUR);
        let tod = w.tick_of_day();
        let id = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| {
                citysim::systems::demography::is_adult(&w, a)
                    && !w.has::<Sentence>(a)
                    && w.comp::<Job>(a).is_some_and(|j| j.employer.is_some() && !j.on_shift(tod))
                    && w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
            })
            .expect("an employed Statistical adult off shift");
        let home = w.comp::<Household>(id).and_then(|h| h.home).unwrap();
        for r in w.residents_of(home).to_vec() {
            if let Some(i) = w.comp_mut::<Inventory>(r) {
                i.food = 0;
                i.synth = 0;
                i.stolen_food = 0;
            }
            if let Some(n) = w.comp_mut::<Needs>(r) {
                n.hunger = 0.95;
            }
        }
        if let Some(b) = w.comp_mut::<citysim::Building>(home) {
            b.stock_food = 0;
            b.synth_food = 0;
        }
        w.comp_mut::<Wallet>(id).expect("wallet").coins = 60;
        stat_hour(&mut w);
        let stock = w.comp::<citysim::Building>(home).map_or(0, |b| b.stock_food);
        eprintln!("item 11 {on}: pantry after the hour {stock}");
        assert_eq!(stock > 0, on, "the off-shift worker's pantry run (item 11 {on})");
    }
}

/// Diet parity: a meal at hand (carried, or in the Home pantry) is eaten
/// before a bin is: `bins::dive_target` has none for an agent holding one,
/// in both tiers.
#[test]
fn test_no_bin_with_a_meal_at_hand() {
    use citysim::{Household, Inventory, Needs, Wallet};
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    // A housed broke adult, starving, with a bin in reach once its food is gone.
    let found = w.citizens().into_iter().find_map(|a| {
        if !citysim::systems::demography::is_adult(&w, a) || w.has::<Sentence>(a) {
            return None;
        }
        let home = w.comp::<Household>(a).and_then(|h| h.home)?;
        let mut c = w.clone();
        c.comp_mut::<Wallet>(a)?.coins = 0;
        c.comp_mut::<Needs>(a)?.hunger = 0.05;
        if let Some(i) = c.comp_mut::<Inventory>(a) {
            i.food = 0;
            i.synth = 0;
            i.stolen_food = 0;
        }
        c.comp_mut::<citysim::Building>(home)?.stock_food = 0;
        citysim::systems::bins::dive_target(&c, a).map(|_| (a, home, c))
    });
    let Some((a, home, mut c)) = found else { panic!("no broke adult with a bin in reach") };
    c.comp_mut::<citysim::Building>(home).unwrap().stock_food = 2;
    assert!(citysim::systems::bins::dive_target(&c, a).is_none(), "a bin with a pantry meal at home");
    c.comp_mut::<citysim::Building>(home).unwrap().stock_food = 0;
    c.comp_mut::<Inventory>(a).unwrap().food = 1;
    assert!(citysim::systems::bins::dive_target(&c, a).is_none(), "a bin with a meal in hand");
}

/// Scale S1: `crew_above`, a Statistical worker's co-workers above it for
/// the hour's co-work. `crew` 0 and a group of at most `crew + 1` are every
/// co-worker above it (the pre-S1 rule); a bigger group is the `crew / 2`
/// nearest on each side in id order, wrapping, so every member drives at
/// most `crew` pairs in all and each pair is driven once (membership is
/// symmetric); other roles at the employer are passed over.
#[test]
fn test_crew_above_bounds_coworker_pairs() {
    let ids = |r: std::ops::Range<u32>| -> Vec<citysim::EntityId> {
        r.map(|index| citysim::EntityId { index, generation: 0 }).collect()
    };
    let all = |_: citysim::EntityId| true;
    let small = ids(0..10);
    for crew in [0, 9, 16] {
        assert_eq!(lod::crew_above(&small, small[3], crew, all), small[4..].to_vec(), "crew {crew}");
    }
    assert_eq!(lod::crew_above(&small, small[3], 4, all), vec![small[4], small[5]]);
    // Wrapping: the last member's crew holds the first ones, driven by them.
    assert_eq!(lod::crew_above(&small, small[9], 4, all), Vec::<citysim::EntityId>::new());
    assert_eq!(lod::crew_above(&small, small[0], 4, all), vec![small[1], small[2], small[8], small[9]]);
    let big = ids(0..100);
    for crew in [2usize, 8, 32] {
        let mut pairs = std::collections::BTreeSet::new();
        for &a in &big {
            for b in lod::crew_above(&big, a, crew, all) {
                assert!(b > a);
                assert!(pairs.insert((a, b)), "pair driven twice");
            }
        }
        assert_eq!(pairs.len(), big.len() * crew / 2, "crew {crew}: each member in `crew` pairs");
    }
    // Another role in between is skipped, not counted.
    let even = |e: citysim::EntityId| e.index.is_multiple_of(2);
    assert_eq!(lod::crew_above(&big, big[10], 4, even), vec![big[12], big[14]]);
}

/// Scale S1: `[lod] coworker_crew` bounds the hour's co-work: with a crew of
/// 2, the co-workers of a big group are tied to their two neighbours, not to
/// the whole group (the pre-S1 rule, `coworker_crew` 0, ties every pair).
#[test]
fn test_coworker_crew_bounds_the_coworker_edges() {
    let tied = |crew: usize| -> (usize, usize) {
        let mut cfg = Config::load();
        cfg.lod.coworker_crew = crew;
        let mut w = World::new(42, cfg);
        w.run_ticks(2 * TICKS_PER_DAY);
        // The staff of every workplace's biggest role, at least 20 strong.
        let (mut tied, mut groups) = (0, 0);
        for staff in w.employers.values() {
            let mut by_role: std::collections::BTreeMap<String, Vec<citysim::EntityId>> = Default::default();
            for &c in staff {
                if let Some(j) = w.comp::<citysim::Job>(c) {
                    by_role.entry(j.role.label().to_string()).or_default().push(c);
                }
            }
            for g in by_role.values().filter(|g| g.len() >= 20) {
                groups += 1;
                for (i, &a) in g.iter().enumerate() {
                    tied += g[i + 1..].iter().filter(|&&b| w.edge(a, b).is_some()).count();
                }
            }
        }
        (tied, groups)
    };
    let (all, groups) = tied(0);
    let (crew, _) = tied(2);
    eprintln!("co-worker pairs tied in {groups} groups of 20+: every pair {all}, crew 2 {crew}");
    assert!(groups > 0, "no workplace has 20 of a role");
    assert!(crew * 3 < all, "crew 2 tied {crew} pairs, every co-worker {all}");
}
