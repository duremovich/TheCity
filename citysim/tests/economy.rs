//! M1: price, production, wages, needs.

use citysim::systems::economy;
use citysim::{needs, Building, BuildingKind, Config, Job, Needs, Role, Skills, Wallet, World, TICKS_PER_DAY};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

#[test]
fn test_price_formula_table() {
    // The M1 table at price_base 3 (the v1 economy; L1b prices the 2,000 city at 4).
    let cfg = Config::load().v1_profile();
    // price_ref_stock 1500: round(3 * sqrt(1500 / max(stock, 7))) clamped to 1..=30
    for (stock, price) in [(7, 30), (50, 16), (400, 6), (600, 5), (1500, 3), (2400, 2), (10000, 1)] {
        assert_eq!(economy::price_for_stock(&cfg.economy, stock), price, "stock {stock}");
    }
    assert_eq!(economy::price_for_stock(&cfg.economy, 0), 30, "stock below the floor is clamped to 7");
}

#[test]
fn test_price_rises_when_stock_falls() {
    let mut w = world(1);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    assert_eq!(w.mean_price(), 3);
    // drain the shelf just before the day-1 price tick (farms haul in during the day)
    w.run_ticks(TICKS_PER_DAY - 1);
    w.comp_mut::<Building>(market).expect("market").stock_food = 100;
    w.run_ticks(2);
    let price = w.mean_price();
    assert!(price > 3, "price {price}");
    assert_eq!(price, economy::price_for_stock(&w.config.economy, w.comp::<Building>(market).expect("m").stock_food));
    assert_eq!(
        w.comp::<citysim::Market>(w.building_of_kind(BuildingKind::Market).expect("market"))
            .expect("market")
            .price_history
            .len(),
        2,
        "day 0 and day 1"
    );
}

#[test]
fn test_farmer_produces_food() {
    let mut w = world(42);
    w.config.lod.force = Some(citysim::Lod::Full); // an arbitrary agent must be simulated in full
    w.config.economy.haul_min_stock = u32::MAX; // no hauling: measure raw farm output
    let farmers: Vec<_> =
        w.citizens().into_iter().filter(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Farmer)).collect();
    assert_eq!(farmers.len(), 24);
    for &id in &farmers {
        w.comp_mut::<Skills>(id).expect("skills").farming = 0.5;
        // broke: Work outscores everything for a full shift (a yield test, not a motivation test)
        w.comp_mut::<Wallet>(id).expect("wallet").coins = 0;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 1.0;
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
        fun: 1.0,
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
fn test_wage_visit_blocked_only_after_short_payment() {
    let mut w = world(8);
    let worker =
        w.citizens().into_iter().find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Clerk)).expect("a clerk");
    w.comp_mut::<Job>(worker).expect("job").days_unpaid = 1;
    assert!(w.comp::<Job>(worker).expect("job").wage_collectable(w.day()));
    economy::collect_wage(&mut w, worker);
    let job = w.comp::<Job>(worker).expect("job");
    assert_eq!(job.days_unpaid, 0);
    assert_eq!(job.last_wage_attempt_day, None, "a full payment does not block a later visit today");

    w.comp_mut::<Job>(worker).expect("job").days_unpaid = 2;
    w.treasury_mut().expect("treasury").coins = 1;
    economy::collect_wage(&mut w, worker);
    let job = w.comp::<Job>(worker).expect("job");
    assert!(job.days_unpaid >= 1);
    assert_eq!(job.last_wage_attempt_day, Some(w.day()), "a short payment blocks a second visit today");
    assert!(!job.wage_collectable(w.day()));
    assert!(job.wage_collectable(w.day() + 1));
}
