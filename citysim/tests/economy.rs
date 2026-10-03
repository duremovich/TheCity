//! M1: price, production, wages, needs.

use citysim::systems::economy;
use citysim::{needs, Building, BuildingKind, Config, Job, Needs, Role, Skills, Wallet, World, TICKS_PER_DAY};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
}

#[test]
fn test_price_formula_table() {
    let cfg = Config::load();
    for (stock, price) in [(7, 28), (50, 10), (600, 3), (2400, 2), (10000, 1)] {
        assert_eq!(economy::price_for_stock(&cfg.economy, stock), price, "stock {stock}");
    }
    assert_eq!(economy::price_for_stock(&cfg.economy, 0), 28, "stock below the floor is clamped to 7");
}

#[test]
fn test_price_rises_when_stock_falls() {
    let mut w = world(1);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    assert_eq!(w.market().expect("market").price_food, 3);
    w.comp_mut::<Building>(market).expect("market").stock_food = 100;
    // next daily tick is tick_of_day == 0 of day 1
    w.run_ticks(TICKS_PER_DAY + 1);
    let price = w.market().expect("market").price_food;
    assert!(price > 3, "price {price}");
    assert_eq!(price, economy::price_for_stock(&w.config.economy, w.comp::<Building>(market).expect("m").stock_food));
    assert_eq!(w.market().expect("market").price_history.len(), 2, "day 0 and day 1");
}

#[test]
fn test_farmer_produces_food() {
    let mut w = world(42);
    w.config.economy.haul_enabled = false;
    let farmers: Vec<_> =
        w.citizens().into_iter().filter(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Farmer)).collect();
    assert_eq!(farmers.len(), 24);
    for &id in &farmers {
        w.comp_mut::<Skills>(id).expect("skills").farming = 0.5;
    }
    let farm_stock = |w: &World| -> u32 {
        w.buildings_by_kind[&BuildingKind::Farm].iter().map(|&f| w.comp::<Building>(f).expect("farm").stock_food).sum()
    };
    assert_eq!(farm_stock(&w), 0);
    w.run_ticks(TICKS_PER_DAY);
    let produced = farm_stock(&w) as i64;
    // 24 farmers × 9 h × farm_yield_base × (0.6 + 0.8 × 0.5) × Spring 1.0
    // (302 in the spec at yield 1.4; the yield is tuned in config.toml)
    let e = &w.config.economy;
    let expected = (24.0 * 9.0 * e.farm_yield_base * (e.farm_skill_floor + e.farm_skill_slope * 0.5) * e.season_mult[0])
        .round() as i64;
    assert!((produced - expected).abs() <= 10, "produced {produced}, expected {expected}");
    // production.accum keeps the fraction; skill ticked up once
    for &id in &farmers {
        let s = w.comp::<Skills>(id).expect("skills");
        assert!((s.farming - 0.502).abs() < 1e-4, "farming {}", s.farming);
        assert_eq!(w.comp::<Job>(id).expect("job").last_shift_day, Some(0));
    }
}

#[test]
fn test_wage_taxed_to_treasury() {
    let mut w = world(7);
    let worker = w
        .citizens()
        .into_iter()
        .find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Farmer))
        .expect("a farmer");
    w.comp_mut::<Job>(worker).expect("job").days_unpaid = 7;
    let due = 7 * 6;
    let tax = (due as f32 * 0.05).floor() as i64; // 2
    let treasury_before = w.treasury().expect("treasury").coins;
    let wallet_before = w.comp::<Wallet>(worker).expect("wallet").coins;

    let paid = economy::collect_wage(&mut w, worker);

    assert_eq!(paid, due - tax);
    assert_eq!(w.treasury().expect("treasury").coins, treasury_before - (due - tax));
    assert_eq!(w.comp::<Wallet>(worker).expect("wallet").coins, wallet_before + due - tax);
    let job = w.comp::<Job>(worker).expect("job");
    assert_eq!(job.days_unpaid, 0);
    assert!((job.tax_accum - 0.1).abs() < 1e-4, "fraction carried: {}", job.tax_accum);
}

#[test]
fn test_wage_partially_paid_when_treasury_short() {
    let mut w = world(7);
    let worker =
        w.citizens().into_iter().find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)).expect("a guard");
    w.comp_mut::<Job>(worker).expect("job").days_unpaid = 3; // due 24, tax 1, net 23
    w.treasury_mut().expect("treasury").coins = 10;
    let paid = economy::collect_wage(&mut w, worker);
    assert_eq!(paid, 10);
    assert_eq!(w.treasury().expect("treasury").coins, 0);
    // 13 coins outstanding at 8/day → 2 days still owed
    assert_eq!(w.comp::<Job>(worker).expect("job").days_unpaid, 2);
}

#[test]
fn test_hunger_decays_and_eating_restores() {
    let cfg = Config::load();
    let mut n = Needs {
        hunger: 1.0,
        energy: 1.0,
        safety: 1.0,
        wealth: 0.0,
        belonging: 1.0,
        intimacy: 1.0,
        starving_since: None,
    };
    let ctx = needs::DecayCtx::plain();
    for _ in 0..TICKS_PER_DAY {
        needs::decay(&mut n, &cfg.needs, &ctx, 1);
    }
    assert!((n.hunger - 0.5).abs() < 0.01, "hunger after a day: {}", n.hunger);
    needs::eat(&mut n, &cfg.needs);
    assert!((n.hunger - 1.0).abs() < 0.01, "one meal: {}", n.hunger);
    needs::eat(&mut n, &cfg.needs);
    assert_eq!(n.hunger, 1.0, "clamped");
    // 60 ticks in one step equals 60 single steps
    let mut a = n.clone();
    let mut b = n.clone();
    needs::decay(&mut a, &cfg.needs, &ctx, 60);
    for _ in 0..60 {
        needs::decay(&mut b, &cfg.needs, &ctx, 1);
    }
    assert!((a.hunger - b.hunger).abs() < 1e-4);
}

#[test]
fn test_starvation_kills_after_grace() {
    let mut w = world(3);
    let victim = w.citizens()[0];
    // no food anywhere for this agent: empty wallet, inventory and pantry
    w.comp_mut::<Wallet>(victim).expect("wallet").coins = 0;
    w.comp_mut::<citysim::Inventory>(victim).expect("inv").food = 0;
    let home = w.comp::<citysim::Household>(victim).expect("hh").home.expect("home");
    w.comp_mut::<Building>(home).expect("home").stock_food = 0;
    w.comp_mut::<Needs>(victim).expect("needs").hunger = 0.0;
    w.levers.dole_per_day = 0; // no coins from the Hall
    w.comp_mut::<citysim::Household>(victim).expect("hh").home = None; // no housemates' pantry
    w.leave_building(victim);
    let grace = w.config.needs.starvation_grace_ticks;
    w.run_ticks(grace + 2);
    assert!(w.has::<citysim::Corpse>(victim), "starved");
    assert!(!w.citizens().contains(&victim));
    assert_eq!(w.population(), 299);
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Death && e.actors.contains(&victim)));
    assert!(w.comp::<citysim::Identity>(victim).is_some(), "identity survives death");
}
