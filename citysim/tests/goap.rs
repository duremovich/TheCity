//! M3: the planner, plan queue, cooldowns and reservations.

use citysim::goap::planner::{self, Limits};
use citysim::systems::plan;
use citysim::{
    ActionKind, Brain, Building, BuildingKind, Config, GoalKind, Inventory, Job, Key, LocationKey, Needs, Personality,
    PlanCtx, Position, Skills, StealSource, TilePos, Wallet, World, WorldState,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().dole_city().v1_profile())
}

fn limits(w: &World) -> Limits {
    Limits { max_expansions: w.config.brain.plan_max_expansions, max_len: w.config.brain.plan_max_len }
}

/// Put an agent on a street tile `d` tiles (Manhattan) from the Market door
/// so `GoTo(Market)` costs `2 + d/16`, with the given wallet, hunger and traits.
fn scenario(w: &mut World, d: u8, coins: i64, lawfulness: f32, pride: f32, sociability: f32) -> citysim::EntityId {
    let id = w.citizens().into_iter().find(|&id| !w.has::<Job>(id)).expect("unemployed");
    w.leave_building(id);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    let door = w.comp::<Building>(market).expect("market").door;
    // walk straight up the vertical road west of the Market: (door.x - 1?)... use the road at x = 39
    let tile = TilePos { x: 39, y: door.y.saturating_sub(u16::from(d) - (door.x - 39)) };
    assert_eq!(tile.manhattan(door), u32::from(d), "scenario tile distance");
    w.comp_mut::<Position>(id).expect("pos").tile = tile;
    w.comp_mut::<Wallet>(id).expect("wallet").coins = coins;
    w.comp_mut::<Inventory>(id).expect("inv").food = 0;
    let home = w.comp::<citysim::Household>(id).expect("hh").home.expect("home");
    w.comp_mut::<Building>(home).expect("home").stock_food = 0;
    let p = w.comp_mut::<Personality>(id).expect("p");
    p.lawfulness = lawfulness;
    p.pride = pride;
    p.sociability = sociability;
    w.comp_mut::<Skills>(id).expect("s").stealth = 0.2;
    w.comp_mut::<Needs>(id).expect("n").hunger = 0.1;
    w.comp_mut::<Building>(market).expect("market").stock_food = 400;
    // daytime, no guards nearby: move every guard into the Jail
    w.tick = 700;
    for g in w.citizens() {
        if w.comp::<Job>(g).is_some_and(|j| j.role == citysim::Role::Guard) {
            let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
            w.leave_building(g);
            w.enter_building(g, jail);
        }
    }
    id
}

#[test]
fn test_planner_finds_buy_and_eat() {
    let mut w = world(1);
    let id = scenario(&mut w, 14, 30, 0.8, 0.5, 0.5);
    let ctx = PlanCtx::build(&w, id, None);
    let start = WorldState::observe(&w, id, None);
    let found = planner::plan(&ctx, start, &vec![(Key::HungerSatisfied, true)], limits(&w)).expect("plan");
    assert_eq!(
        found.steps,
        vec![ActionKind::GoTo(LocationKey::Market), ActionKind::BuyFood, ActionKind::EatFromInventory],
        "cost {}",
        found.cost
    );
}

#[test]
fn test_planner_respects_limits() {
    let mut w = world(1);
    let id = scenario(&mut w, 14, 30, 0.8, 0.5, 0.5);
    let ctx = PlanCtx::build(&w, id, None);
    let start = WorldState::observe(&w, id, None);
    // Nothing implemented produces a jailed suspect: unsatisfiable.
    let err = planner::plan(&ctx, start, &vec![(Key::SuspectJailed, true)], limits(&w)).expect_err("unsatisfiable");
    assert!(err.1 <= 200, "{err:?}");
    // A reachable goal never exceeds 6 steps.
    let found = planner::plan(&ctx, start, &vec![(Key::HungerSatisfied, true)], limits(&w)).expect("plan");
    assert!(found.steps.len() <= 6);
    assert!(found.expansions <= 200);
}

#[test]
fn test_hungry_broke_low_lawfulness_steals() {
    // Worked trace 1: coins 0, hunger 0.1, lawfulness 0.3, stealth 0.2, pride 0.5,
    // sociability 0.5, daytime, Spring, Market stock 400, no guard within 8.
    let mut w = world(1);
    let id = scenario(&mut w, 14, 0, 0.3, 0.5, 0.5);
    let ctx = PlanCtx::build(&w, id, None);
    assert!(!ctx.guard8, "no guard within 8");
    assert!(ctx.starving && !ctx.dark);
    let start = WorldState::observe(&w, id, None);
    let found = planner::plan(&ctx, start, &vec![(Key::HungerSatisfied, true)], limits(&w)).expect("plan");
    assert!(found.steps.contains(&ActionKind::StealFood(StealSource::Market)), "{:?}", found.steps);
    assert_eq!(
        found.steps,
        vec![
            ActionKind::GoTo(LocationKey::Market),
            ActionKind::StealFood(StealSource::Market),
            ActionKind::EatFromInventory
        ]
    );
    // GoTo 2 + 14/16 = 2.875; StealFood 8 + 0.3×20 − 4 − 0.8 = 9.2; Eat 1.0 → 13.075
    assert!((found.cost - 13.1).abs() <= 0.1, "cost {}", found.cost);
}

#[test]
fn test_hungry_broke_high_lawfulness_forages() {
    // Worked trace 2: lawfulness 0.9, pride 0.4, sociability 0.6, Autumn.
    let mut w = world(1);
    let id = scenario(&mut w, 14, 0, 0.9, 0.4, 0.6);
    w.tick = 60 * citysim::TICKS_PER_DAY + 700; // day 60: Autumn, daytime
                                                // the trace predates the dole (review note 3): today's is already collected
    w.comp_mut::<Brain>(id).expect("brain").last_dole_day = Some(w.day());
    let ctx = PlanCtx::build(&w, id, None);
    assert_eq!(ctx.season, citysim::Season::Autumn);
    let start = WorldState::observe(&w, id, None);
    assert!(start.forage_available);
    let found = planner::plan(&ctx, start, &vec![(Key::HungerSatisfied, true)], limits(&w)).expect("plan");
    assert!(found.steps.contains(&ActionKind::Forage), "{:?}", found.steps);
    assert!(!found.steps.iter().any(|s| matches!(s, ActionKind::StealFood(_))), "{:?}", found.steps);
    assert_eq!(found.steps[0], ActionKind::GoTo(LocationKey::Farm));
}

#[test]
fn test_failed_goal_cools_for_120_ticks() {
    let mut w = world(2);
    let id = w.citizens()[0];
    w.tick = 500;
    let goal = GoalKind::Socialise;
    w.goal_failed(id, goal);
    assert!(!w.comp::<Brain>(id).expect("brain").cooldowns.contains_key(&goal), "first failure: replan");
    w.goal_failed(id, goal);
    let until = *w.comp::<Brain>(id).expect("brain").cooldowns.get(&goal).expect("cooled");
    assert_eq!(until, 500 + w.config.brain.goal_cooldown_ticks);
    // the cooled goal is skipped by utility until then
    w.comp_mut::<Needs>(id).expect("needs").belonging = 0.0;
    let (_, trace) = citysim::utility::think(&w, id).expect("think");
    assert!(trace.goals.iter().all(|g| g.goal != goal), "{trace:?}");
    w.tick = until;
    let (_, trace) = citysim::utility::think(&w, id).expect("think");
    assert!(trace.goals.iter().any(|g| g.goal == goal), "available again at {until}");
}

#[test]
fn test_plan_budget_12_per_tick() {
    let mut w = world(3);
    let ids: Vec<_> = w.citizens().into_iter().take(100).collect();
    for &id in &ids {
        let b = w.comp_mut::<Brain>(id).expect("brain");
        b.current_goal = Some(GoalKind::Eat);
        b.clear_plan();
        w.comp_mut::<Needs>(id).expect("needs").hunger = 0.2;
        plan::enqueue(&mut w, id, 0.5);
    }
    assert_eq!(w.plan_queue.len(), 100);
    plan::run(&mut w);
    let planned = ids.iter().filter(|&&id| w.comp::<Brain>(id).expect("brain").plan.is_some()).count();
    assert_eq!(planned, w.config.brain.plan_budget_per_tick);
    assert_eq!(w.plan_queue.len(), 88);
}

#[test]
fn test_reservation_prevents_overcommit() {
    let mut w = world(4);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    // Hunger as sickness (always on since P10): a buyer with `real_meal_reserve`
    // real meals' coins buys real, that many shelf units (a synth buyer's
    // reservation counts meals, not units: see docs/P10_POLICY.md).
    let reserve = u32::try_from(w.config.sickness.real_meal_reserve).expect("a small reserve");
    w.comp_mut::<Building>(market).expect("market").stock_food = 2 * reserve;
    // nobody's pantry counts as a source
    for h in w.buildings_by_kind[&BuildingKind::Home].clone() {
        w.comp_mut::<Building>(h).expect("home").stock_food = 0;
    }
    let ids: Vec<_> = w.citizens().into_iter().filter(|&id| !w.has::<Job>(id)).take(3).collect();
    for &id in &ids {
        // one buyer's worth each: BuyFood reserves what it will actually buy
        let price = w.price_for(market, id);
        w.comp_mut::<Wallet>(id).expect("wallet").coins = price * i64::from(reserve);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 0.2;
        w.comp_mut::<Brain>(id).expect("brain").current_goal = Some(GoalKind::Eat);
    }
    assert!(WorldState::observe(&w, ids[0], None).food_source_available);
    plan::plan_for(&mut w, ids[0], GoalKind::Eat);
    assert!(w.comp::<Brain>(ids[0]).expect("b").plan.is_some());
    assert!(WorldState::observe(&w, ids[1], None).food_source_available, "one buyer's worth left");
    plan::plan_for(&mut w, ids[1], GoalKind::Eat);
    assert!(w.comp::<Brain>(ids[1]).expect("b").plan.is_some());
    assert!(!WorldState::observe(&w, ids[2], None).food_source_available, "both buyers' units reserved");
    assert_eq!(WorldState::reserved_units(&w, market), 2 * reserve);
}

/// A synth buyer reserves the shelf units its meals take, not one unit a
/// meal: three synth meals are one unit at `synth_stretch` 3, so a shelf
/// of two units serves two such buyers before it reads reserved.
#[test]
fn test_synth_reservation_counts_shelf_units_not_meals() {
    let mut w = world(4);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    let stretch = w.config.sickness.synth_stretch;
    assert!(stretch >= 2, "the case needs a synth unit of several meals");
    w.comp_mut::<Building>(market).expect("market").stock_food = 2;
    w.comp_mut::<citysim::Market>(market).expect("market").synth_frac = 0; // no open unit
    for h in w.buildings_by_kind[&BuildingKind::Home].clone() {
        w.comp_mut::<Building>(h).expect("home").stock_food = 0;
    }
    let ids: Vec<_> = w.citizens().into_iter().filter(|&id| !w.has::<Job>(id)).take(3).collect();
    for &id in &ids {
        // a unit's worth of synth meals' coins, short of the real reserve
        let synth = citysim::systems::diet::synth_price(&w, w.price_for(market, id));
        w.comp_mut::<Wallet>(id).expect("wallet").coins = synth * i64::from(stretch);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 0.2;
        w.comp_mut::<Brain>(id).expect("brain").current_goal = Some(GoalKind::Eat);
        let (q, _) = citysim::systems::diet::offer(&w, id, market);
        assert_eq!(q, citysim::systems::diet::Quality::Synth, "a synth buyer");
    }
    assert_eq!(
        citysim::systems::economy::buy_quantity(&w, ids[0], Some(market)),
        stretch.min(3),
        "the buyer takes a unit's meals"
    );
    plan::plan_for(&mut w, ids[0], GoalKind::Eat);
    assert!(w.comp::<Brain>(ids[0]).expect("b").plan.is_some());
    assert_eq!(WorldState::reserved_units(&w, market), 1, "three synth meals hold one shelf unit");
    assert!(WorldState::observe(&w, ids[1], None).food_source_available, "one unit left");
    plan::plan_for(&mut w, ids[1], GoalKind::Eat);
    assert!(w.comp::<Brain>(ids[1]).expect("b").plan.is_some());
    assert!(!WorldState::observe(&w, ids[2], None).food_source_available, "both units reserved");
    assert_eq!(WorldState::reserved_units(&w, market), 2);
}

/// Review item 4 (`/code-review high 7324e60..dfaca2e`): two synth buyers
/// planning at once each counted the same open unit's meals and held no
/// shelf unit between them. The first holds the open meals; the second
/// holds the shelf unit its meals need past them, so a third finds the
/// shelf reserved.
#[test]
fn test_synth_reservation_does_not_count_open_meals_twice() {
    let mut w = world(4);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    let stretch = w.config.sickness.synth_stretch;
    assert!((2..=4).contains(&stretch), "the case needs an open unit of 1-3 meals");
    w.comp_mut::<Building>(market).expect("market").stock_food = 1;
    // One meal cut from the open unit: `stretch - 1` meals left in it.
    w.comp_mut::<citysim::Market>(market).expect("market").synth_frac = 1;
    let open = citysim::systems::diet::open_meals(&w, market);
    assert_eq!(open, stretch - 1);
    for h in w.buildings_by_kind[&BuildingKind::Home].clone() {
        w.comp_mut::<Building>(h).expect("home").stock_food = 0;
    }
    let ids: Vec<_> = w.citizens().into_iter().filter(|&id| !w.has::<Job>(id)).take(3).collect();
    for &id in &ids {
        // the open unit's meals' coins, short of the real reserve
        let synth = citysim::systems::diet::synth_price(&w, w.price_for(market, id));
        w.comp_mut::<Wallet>(id).expect("wallet").coins = synth * i64::from(open);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 0.2;
        w.comp_mut::<Brain>(id).expect("brain").current_goal = Some(GoalKind::Eat);
        let (q, _) = citysim::systems::diet::offer(&w, id, market);
        assert_eq!(q, citysim::systems::diet::Quality::Synth, "a synth buyer");
    }
    assert_eq!(citysim::systems::economy::buy_quantity(&w, ids[0], Some(market)), open, "the open unit's meals");
    plan::plan_for(&mut w, ids[0], GoalKind::Eat);
    assert!(w.comp::<Brain>(ids[0]).expect("b").plan.is_some());
    assert_eq!(WorldState::reserved_units(&w, market), 0, "the open unit serves the first buyer");
    assert!(WorldState::observe(&w, ids[1], None).food_source_available, "the shelf unit is free");
    plan::plan_for(&mut w, ids[1], GoalKind::Eat);
    assert!(w.comp::<Brain>(ids[1]).expect("b").plan.is_some());
    assert_eq!(WorldState::reserved_units(&w, market), 1, "the second buyer's meals need the shelf unit");
    assert!(!WorldState::observe(&w, ids[2], None).food_source_available, "the shelf unit is reserved");
}

#[test]
fn test_idle_with_stolen_food_can_still_store() {
    use citysim::exec::routine;
    let mut w = world(5);
    let id = w.citizens().into_iter().find(|&id| !w.has::<Job>(id)).expect("unemployed");
    let inv = w.comp_mut::<Inventory>(id).expect("inv");
    inv.food = 3;
    inv.stolen_food = 1;
    let plan = routine::idle_plan(&w, id).expect("idle plan");
    assert_eq!(plan.steps[0].action, ActionKind::StoreFood);
    let ctx = PlanCtx::build_light(&w, id, None);
    let ws = WorldState::observe(&w, id, None);
    assert!(ActionKind::StoreFood.preconditions(&ws, &ctx), "the symbolic check must agree with the executor");
}

#[test]
fn test_work_plans_the_shift_when_the_wage_trip_is_blocked() {
    let mut w = world(6);
    let farmer = w
        .citizens()
        .into_iter()
        .find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == citysim::Role::Farmer))
        .expect("farmer");
    let farm = w.comp::<Job>(farmer).expect("job").employer.expect("farm");
    w.tick = 600; // on shift
    w.leave_building(farmer);
    w.enter_building(farmer, farm);
    w.comp_mut::<Job>(farmer).expect("job").last_wage_attempt_day = Some(0); // short-paid this morning
    w.comp_mut::<Brain>(farmer).expect("brain").current_goal = Some(GoalKind::Work);
    plan::plan_for(&mut w, farmer, GoalKind::Work);
    let brain = w.comp::<Brain>(farmer).expect("brain");
    let steps: Vec<_> = brain.plan.as_ref().expect("a plan").steps.iter().map(|s| s.action).collect();
    assert_eq!(steps, vec![ActionKind::FarmWork], "{steps:?}");
    assert!(!brain.cooldowns.contains_key(&GoalKind::Work));
}

#[test]
fn test_empty_market_with_stocked_pantry_eats_at_home() {
    let mut w = world(8);
    let id = scenario(&mut w, 14, 30, 0.8, 0.5, 0.5);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    w.comp_mut::<Building>(market).expect("market").stock_food = 0;
    let home = w.comp::<citysim::Household>(id).expect("hh").home.expect("home");
    w.comp_mut::<Building>(home).expect("home").stock_food = 2;
    let ctx = PlanCtx::build(&w, id, None);
    let start = WorldState::observe(&w, id, None);
    let found = planner::plan(&ctx, start, &vec![(Key::HungerSatisfied, true)], limits(&w)).expect("plan");
    assert!(!found.steps.contains(&ActionKind::BuyFood), "{:?}", found.steps);
    assert!(found.steps.contains(&ActionKind::EatAtHome), "{:?}", found.steps);
}

#[test]
fn test_beg_does_not_promise_a_meal_at_price_three() {
    let mut w = world(9);
    let id = scenario(&mut w, 14, 0, 0.9, 0.4, 0.6);
    // Hunger as sickness (always on since P10): a broke agent's meal is synth
    // at a third of the shelf price, so a meal at 3 is a shelf at 9.
    let stretch = i64::from(citysim::systems::diet::stretch(&w));
    w.comp_mut::<citysim::Market>(w.building_of_kind(BuildingKind::Market).expect("market"))
        .expect("market")
        .price_food = 3 * stretch;
    let ctx = PlanCtx::build(&w, id, None);
    let mut ws = WorldState::observe(&w, id, None);
    ws.at = LocationKey::Market;
    let after = ActionKind::Beg.apply(&ws, &ctx);
    assert!(!after.has_coins);
    w.comp_mut::<citysim::Market>(w.building_of_kind(BuildingKind::Market).expect("market"))
        .expect("market")
        .price_food = 2;
    let ctx = PlanCtx::build(&w, id, None);
    let after = ActionKind::Beg.apply(&ws, &ctx);
    assert!(after.has_coins);
}
