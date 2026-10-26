//! The Real economy phase 3a (docs/ECONOMY_V2.md § 2; plan 3a.5): no safety
//! net. In the shipped policy (P10: the market wage law, no net) the dole is 0, public
//! works are never posted, the Reserve's free release is gone, school meals
//! are off, scavenging is paid from the Recycler's till alone, and the
//! Treasury's tax band moves the tax rate with hysteresis and a pin.

use citysim::components::{Brain, Child, Good, Identity};
use citysim::exec::actions;
use citysim::goap::ActionKind;
use citysim::systems::ownership;
use citysim::systems::{assets, demography, econ, treasury, world_market};
use citysim::{
    Building, BuildingKind, Config, Corp, EntityId, EventKind, Job, Market, PlayerCommand, StepResult, Wallet, World,
    TICKS_PER_DAY,
};

/// The 3a city: wages on and no safety net.
fn config() -> Config {
    let mut cfg = Config::load().dole_city();
    cfg.policy = citysim::config::PolicyCfg::default();
    cfg
}

fn city() -> World {
    let w = World::new(42, config());
    assert!(econ::wages_on(&w) && !citysim::systems::policy::net_enacted(&w), "the 3a city has wages on and no net");
    w
}

/// Jobless adults with a Brain, free, not in a gang.
fn jobless(w: &World, n: usize) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&id| !w.has::<Job>(id) && w.has::<Brain>(id) && w.gang_of(id).is_none())
        .filter(|&id| demography::is_adult(w, id))
        .take(n)
        .collect()
}

fn treasury_coins(w: &World) -> i64 {
    w.treasury().map_or(0, |t| t.coins)
}

/// A corp-owned Market left stranded at the next midnight: an empty shelf at
/// price 30, its owner with no coins for the Reserve or the World.
fn strand_a_market(w: &mut World) -> EntityId {
    let mk = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .find(|&m| w.owner_of(m).is_some_and(|o| w.has::<Corp>(o)))
        .expect("a corp Market");
    let owner = w.owner_of(mk).expect("owner");
    if let Some(c) = w.comp_mut::<Corp>(owner) {
        c.treasury = 0;
        c.closing = 0;
    }
    w.comp_mut::<Building>(mk).expect("market").stock_food = 0;
    w.comp_mut::<Market>(mk).expect("market").price_food = 30;
    mk
}

fn released(w: &World) -> bool {
    w.events.iter().any(|e| e.kind == EventKind::Restock && e.text.contains("The city released"))
}

/// E22 (a)-(c), E32: ten days with no dole paid, no public works, no
/// release; the dole lever reads 0 at seed but `SetDolePerDay` still sets
/// it (a god scenario). The release itself is pinned on a stranded Market:
/// the net-on twin releases, the no-net city does not.
#[test]
fn test_no_dole_no_works_no_release_with_no_net() {
    let mut w = city();
    assert_eq!(w.levers.dole_per_day, 0, "no dole at seed");
    assert!(!w.levers.public_works, "no public works at seed");
    w.run_ticks(10 * TICKS_PER_DAY);
    assert!(w.stats.history.iter().all(|r| r.flow_dole == 0), "flow_dole 0 every day");
    assert!(w.stats.history.iter().all(|r| r.living.works_jobs == 0), "no works jobs");
    assert!(w.jobs_book.works.is_empty(), "jobs_book.works empty");
    assert_eq!(w.works_vacancies, 0, "no works vacancies posted");
    assert!(!released(&w), "no release in ten days");
    w.push_command(PlayerCommand::SetDolePerDay(4));
    w.apply_commands();
    assert_eq!(w.levers.dole_per_day, 4, "the god lever still sets the dole");

    // The release, pinned: one midnight with a stranded Market.
    let cfg_net = Config::load().dole_city();
    let mut twin = World::new(42, cfg_net);
    assert!(twin.levers.reserve_release, "the dole city enacts the release");
    let mut nn = city();
    for x in [&mut twin, &mut nn] {
        x.run_ticks(TICKS_PER_DAY - 1);
        strand_a_market(x);
        x.run_ticks(2);
    }
    assert!(released(&twin), "with the net the stranded Market gets the Reserve's release");
    assert!(!released(&nn), "with no_net the release returns at once");
}

/// E21: above `hi` the rate falls a step every `band_hold_days` days held,
/// below `lo` it rises, clamped to `[tax_min, tax_max]`; `SetTaxRate` pins it
/// against the band until `SetTaxAuto`. The pass runs in `living::run`'s
/// midnight in place of the L2 budget band.
#[test]
fn test_tax_band_steps_with_hysteresis_and_pins() {
    let mut w = city();
    assert!(treasury::on(&w));
    let cfg = w.config.treasury.clone();
    let hold = w.config.budget.band_hold_days;
    assert_eq!(hold, 3);
    let moved = |w: &World| w.events.iter().filter(|e| e.kind == EventKind::TaxMoved).count();
    let set_coins = |w: &mut World, c: i64| w.treasury_mut().expect("the Treasury").coins = c;
    w.levers.tax_rate = 0.12;
    set_coins(&mut w, cfg.band[1] + 50_000);
    treasury::daily(&mut w);
    treasury::daily(&mut w);
    assert_eq!(w.levers.tax_rate, 0.12, "no step before the hold");
    treasury::daily(&mut w);
    assert!((w.levers.tax_rate - (0.12 - cfg.tax_step)).abs() < 1e-6, "a step down after {hold} days above hi");
    assert_eq!(moved(&w), 1, "a TaxMoved event");
    for _ in 0..3 {
        treasury::daily(&mut w);
    }
    assert!((w.levers.tax_rate - (0.12 - 2.0 * cfg.tax_step)).abs() < 1e-6, "the next step a hold later");
    // A pin holds against the band.
    w.push_command(PlayerCommand::SetTaxRate(0.15));
    w.apply_commands();
    assert!(w.econ.tax_pinned, "SetTaxRate pins with no_net");
    for _ in 0..9 {
        treasury::daily(&mut w);
    }
    assert_eq!(w.levers.tax_rate, 0.15, "pinned: the band does not move it");
    w.push_command(PlayerCommand::SetTaxAuto);
    w.apply_commands();
    assert!(!w.econ.tax_pinned, "SetTaxAuto releases the pin");
    for _ in 0..3 {
        treasury::daily(&mut w);
    }
    assert!(w.levers.tax_rate < 0.15, "released, the band moves it again");
    // Inside the band: nothing moves.
    let r = w.levers.tax_rate;
    set_coins(&mut w, (cfg.band[0] + cfg.band[1]) / 2);
    for _ in 0..6 {
        treasury::daily(&mut w);
    }
    assert_eq!(w.levers.tax_rate, r, "inside the band the rate holds");
    // Below lo: up, clamped at tax_max.
    w.levers.tax_rate = cfg.tax_max - cfg.tax_step;
    set_coins(&mut w, cfg.band[0] - 1_000);
    for _ in 0..12 {
        treasury::daily(&mut w);
    }
    assert!((w.levers.tax_rate - cfg.tax_max).abs() < 1e-6, "clamped at tax_max: {}", w.levers.tax_rate);
    // With no surplus credit, clamped at tax_min above hi.
    w.config.treasury.credit_max = 0.0;
    w.levers.tax_rate = cfg.tax_min;
    set_coins(&mut w, cfg.band[1] + 50_000);
    let before = moved(&w);
    for _ in 0..6 {
        treasury::daily(&mut w);
    }
    assert!((w.levers.tax_rate - cfg.tax_min).abs() < 1e-6, "clamped at tax_min");
    assert_eq!(moved(&w), before, "no event without a move");

    // In the tick order: a Treasury far above the band moves the rate within
    // a week, and no public works are posted (the L2 band does not run).
    let mut w = city();
    let start = w.levers.tax_rate;
    w.treasury_mut().expect("t").coins = 200_000;
    w.run_ticks(7 * TICKS_PER_DAY + 1);
    assert!(w.levers.tax_rate < start, "the band moved the rate in the tick order ({start} -> {})", w.levers.tax_rate);
    assert!(w.events.iter().any(|e| e.kind == EventKind::TaxMoved));
    assert!(!w.events.iter().any(|e| e.kind == EventKind::WorksPosted), "no public works");
    // Under the levy (the dole city: the band off) a SetTaxRate does not pin.
    let cfg_net = Config::load().dole_city();
    let mut twin = World::new(42, cfg_net);
    assert!(!treasury::on(&twin));
    twin.push_command(PlayerCommand::SetTaxRate(0.1));
    twin.apply_commands();
    assert!(!twin.econ.tax_pinned, "no pin without the band");
}

/// The surplus credit: a Treasury held above `hi` with the tax at its floor
/// takes the rate past `tax_min`, a step a hold, down to `-credit_max`;
/// back inside the band a credit eases a step a hold toward `tax_min` (the
/// Treasury settles at `hi`, not drained to `lo`); under `lo` it stops at
/// once (the civic posts come first). The shipped city runs it.
#[test]
fn test_surplus_takes_the_rate_below_the_floor() {
    let mut w = city();
    assert!(w.config.treasury.credit_max > 0.0, "the shipped city returns its surplus");
    w.config.treasury.credit_max = 0.03;
    let cfg = w.config.treasury.clone();
    let set_coins = |w: &mut World, c: i64| w.treasury_mut().expect("the Treasury").coins = c;
    let near = |a: f32, b: f32| (a - b).abs() < 1e-6;
    w.levers.tax_rate = cfg.tax_min;
    set_coins(&mut w, cfg.band[1] + 50_000);
    for _ in 0..3 {
        treasury::daily(&mut w);
    }
    assert!(near(w.levers.tax_rate, cfg.tax_min - cfg.tax_step), "a step past the floor: {}", w.levers.tax_rate);
    for _ in 0..30 {
        treasury::daily(&mut w);
    }
    assert!(near(w.levers.tax_rate, -cfg.credit_max), "clamped at -credit_max: {}", w.levers.tax_rate);
    // Inside the band: the credit eases toward the floor and stops there.
    set_coins(&mut w, (cfg.band[0] + cfg.band[1]) / 2);
    for _ in 0..3 {
        treasury::daily(&mut w);
    }
    assert!(near(w.levers.tax_rate, -cfg.credit_max + cfg.tax_step), "eased a step: {}", w.levers.tax_rate);
    for _ in 0..30 {
        treasury::daily(&mut w);
    }
    assert!(near(w.levers.tax_rate, cfg.tax_min), "eased to the floor and held: {}", w.levers.tax_rate);
    // Under lo: a credit stops at once.
    w.levers.tax_rate = -cfg.credit_max;
    set_coins(&mut w, cfg.band[0] - 1);
    treasury::daily(&mut w);
    assert!(near(w.levers.tax_rate, cfg.tax_min), "under lo the credit stops at once: {}", w.levers.tax_rate);
}

/// A corp's worker on the market law: its wage, its days unpaid set to 1.
fn corp_worker(w: &mut World, wage: i64) -> (EntityId, EntityId) {
    let (worker, corp) = w
        .citizens()
        .into_iter()
        .find_map(|a| {
            let corp = w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.owner_of(e))?;
            w.has::<Corp>(corp).then_some((a, corp))
        })
        .expect("a corp's worker");
    let j = w.comp_mut::<Job>(worker).expect("job");
    j.wage_per_day = wage;
    j.days_unpaid = 1;
    j.tax_accum = 0.0;
    j.premium = 1.0;
    if let Some(c) = w.comp_mut::<Corp>(corp) {
        c.wage_mult = 1.0;
        c.wage_rev = 1.0;
        c.treasury = 1_000;
    }
    (worker, corp)
}

/// The surplus credit at the wage desk: a negative rate pays the worker the
/// gross plus the credit; the employer pays the gross, the Treasury the
/// credit (`tax_credit`, `flow_tax` untouched), from its coins above `lo`
/// only. Coins are conserved.
#[test]
fn test_negative_rate_pays_a_wage_credit_from_the_surplus() {
    use citysim::systems::economy;
    let mut w = city();
    // The wage alone (no rent settled from it).
    w.config.rent.pay_from_income = false;
    let lo = w.config.treasury.band[0];
    let wallet = |w: &World, a: EntityId| w.comp::<Wallet>(a).map_or(0, |x| x.coins);
    let (worker, corp) = corp_worker(&mut w, 10);
    w.levers.tax_rate = -0.5;
    w.treasury_mut().expect("t").coins = lo + 100;
    let total = ownership::total_coins(&w);
    let (c0, p0, t0) = (wallet(&w, worker), w.purse(Some(corp)), treasury_coins(&w));
    let tax0 = w.stats.current.flow_tax;
    economy::collect_wage(&mut w, worker);
    assert_eq!(wallet(&w, worker) - c0, 15, "the gross 10 and a credit of 5");
    assert_eq!(p0 - w.purse(Some(corp)), 10, "the employer pays the gross, no tax");
    assert_eq!(t0 - treasury_coins(&w), 5, "the Treasury pays the credit");
    assert_eq!(w.stats.current.jobs.tax_credit, 5);
    assert_eq!(w.stats.current.flow_tax, tax0, "the receipts stay the gross tax");
    assert_eq!(ownership::total_coins(&w), total, "conserved");
    // Only the coins above lo are a surplus.
    let (worker, _) = corp_worker(&mut w, 10);
    w.treasury_mut().expect("t").coins = lo + 2;
    let c0 = wallet(&w, worker);
    economy::collect_wage(&mut w, worker);
    assert_eq!(wallet(&w, worker) - c0, 12, "the credit stops at the surplus");
    assert_eq!(treasury_coins(&w), lo);
    let (worker, _) = corp_worker(&mut w, 10);
    let c0 = wallet(&w, worker);
    economy::collect_wage(&mut w, worker);
    assert_eq!(wallet(&w, worker) - c0, 10, "no surplus, no credit");
}

/// E33: a find is paid from the Recycler's till alone: an empty till pays
/// 0 (the find is still scrap), the Treasury is never touched; the
/// Recycler's Parts sales fill the till, and the till is in `total_coins`.
#[test]
fn test_scavenge_pays_from_till_only() {
    let mut w = city();
    w.config.life.scavenge_p = 1.0;
    let a = jobless(&w, 1)[0];
    w.leave_building(a);
    let wallet = |w: &World| w.comp::<Wallet>(a).map_or(0, |x| x.coins);
    // An empty till pays nothing; the find is still scrap.
    w.econ.recycler_till = 0;
    let (c0, t0, s0) = (wallet(&w), treasury_coins(&w), w.scrap);
    let t = w.tick;
    let r = actions::on_complete(&mut w, a, ActionKind::Scavenge, None, t, t);
    assert_ne!(r, StepResult::Running);
    assert_eq!(wallet(&w), c0, "an empty till pays 0");
    assert_eq!(treasury_coins(&w), t0, "the Treasury is untouched");
    assert_eq!(w.scrap, s0 + 1, "the find is still scrap");
    // A till with coins pays the find from the till.
    w.econ.recycler_till = 10;
    let total = ownership::total_coins(&w);
    let price = match w.config.treasury.scrap_coins {
        c if c > 0 => c,
        _ => w.config.life.scavenge_coins,
    };
    let (c0, t0) = (wallet(&w), treasury_coins(&w));
    let t = w.tick;
    let r = actions::on_complete(&mut w, a, ActionKind::Scavenge, None, t, t);
    assert_eq!(r, StepResult::Done);
    assert_eq!(wallet(&w), c0 + price, "the find paid {price}");
    assert_eq!(w.econ.recycler_till, 10 - price, "from the till");
    assert_eq!(treasury_coins(&w), t0, "the Treasury is untouched");
    assert_eq!(ownership::total_coins(&w), total, "a purse move: the till is in total_coins");
    // The Recycler's sale proceeds (taken by the Treasury as the city's
    // seller) move into the till, conserving.
    let before = (w.econ.recycler_till, treasury_coins(&w));
    let total = ownership::total_coins(&w);
    citysim::systems::treasury::to_till(&mut w, 30);
    assert_eq!(w.econ.recycler_till, before.0 + 30);
    assert_eq!(treasury_coins(&w), before.1 - 30);
    assert_eq!(ownership::total_coins(&w), total, "the till move conserves");
    // Under the levy (the dole city) the till is never written: the Treasury pays the find.
    let cfg_net = Config::load().dole_city();
    let mut twin = World::new(42, cfg_net);
    twin.config.life.scavenge_p = 1.0;
    let b = jobless(&twin, 1)[0];
    twin.leave_building(b);
    let t0 = treasury_coins(&twin);
    let t = twin.tick;
    actions::on_complete(&mut twin, b, ActionKind::Scavenge, None, t, t);
    assert_eq!(twin.econ.recycler_till, 0, "no till under the levy");
    assert!(treasury_coins(&twin) < t0, "the Treasury paid the find");
    citysim::systems::treasury::to_till(&mut twin, 30);
    assert_eq!(twin.econ.recycler_till, 0, "no till move under the levy");
}

/// E36: with no_net school meals are off at seed: a child whose pantry is
/// empty goes a day unfed (the net-on twin feeds it from the Reserve).
#[test]
fn test_school_meals_off_with_no_net() {
    let cfg_net = Config::load().dole_city();
    assert!(cfg_net.policy.school_meals, "school meals on in the dole city");
    let mut twin = World::new(42, cfg_net);
    let mut nn = city();
    assert!(!nn.levers.school_meals, "the shipped policy: no school meals");
    let mut unfed = Vec::new();
    for x in [&mut twin, &mut nn] {
        x.run_ticks(TICKS_PER_DAY - 1);
        // The seed has no children: one born to two adult residents of a Home.
        let homes = x.buildings_of_kind(BuildingKind::Home).to_vec();
        let (home, adults) = homes
            .into_iter()
            .find_map(|h| {
                let a: Vec<EntityId> = x
                    .residents_of(h)
                    .iter()
                    .copied()
                    .filter(|&a| demography::is_adult(x, a) && x.has::<Identity>(a))
                    .collect();
                (a.len() >= 2).then_some((h, a))
            })
            .expect("a Home with two adults");
        let kid = demography::spawn_child(x, adults[0], adults[1], home);
        // The perf pass phase 7b: to day 1's child pass (its midnight slot),
        // the pantry bare as it opens.
        use citysim::systems::midnight::{slot, Mid};
        x.run_ticks(1 + u64::from(slot(Mid::DemoEarly) * x.config.perf.stagger_midnight));
        let b = x.comp_mut::<Building>(home).expect("home");
        b.stock_food = 0;
        b.child_food_debt = 0.9;
        x.run_ticks(1);
        unfed.push(x.comp::<Child>(kid).map_or(0, |c| c.hunger_days));
    }
    assert_eq!(unfed[0], 0, "with the net the school meal fed the child");
    assert_eq!(unfed[1], 1, "with no_net the child went a day unfed");
}

/// Every building's Parts emptied except the Recycler's `n`, every Clinic
/// and Garage filled to its floor except `buyer` (empty): the Recycler is
/// the only source and `buyer` the only buyer.
fn only_recycler_sells(w: &mut World, n: u32, buyer: Option<EntityId>) -> EntityId {
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("the Recycler");
    let floor = w.config.assets.parts_floor;
    for b in w.with::<Building>() {
        let have = w.stock(b, Good::Parts);
        w.take_stock(b, Good::Parts, have);
        let kind = w.comp::<Building>(b).map(|bd| bd.kind);
        // Jobs & lives phase 3: a Gunsmith is a Parts buyer too.
        if matches!(kind, Some(BuildingKind::Clinic | BuildingKind::Garage | BuildingKind::Gunsmith))
            && Some(b) != buyer
        {
            w.add_stock(b, Good::Parts, floor);
        }
    }
    w.add_stock(recycler, Good::Parts, n);
    recycler
}

/// E33 (review fix): a Recycler sale to a paying buyer moves exactly the
/// buyer's coins into the till (the Treasury ends where it started); a
/// city-owned Clinic buying from the city's Recycler moves no coin, so the
/// till does not rise (the phantom drain the review found).
#[test]
fn test_parts_market_recycler_sale_fills_till_exactly() {
    // A corp-owned Clinic pays.
    let mut w = city();
    let clinic = w
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .find(|&c| w.owner_of(c).is_some_and(|o| w.has::<Corp>(o)))
        .expect("a corp Clinic");
    let owner = w.owner_of(clinic).expect("owner");
    let recycler = only_recycler_sells(&mut w, 40, Some(clinic));
    let (t0, till0, p0, total) =
        (treasury_coins(&w), w.econ.recycler_till, w.purse(Some(owner)), ownership::total_coins(&w));
    assets::parts_market(&mut w);
    let bought = 40 - w.stock(recycler, Good::Parts);
    assert!(bought > 0, "the Clinic bought from the Recycler");
    let paid = p0 - w.purse(Some(owner));
    assert_eq!(paid, i64::from(bought) * w.config.assets.parts_price, "the buyer paid parts_price a unit");
    assert_eq!(w.econ.recycler_till - till0, paid, "the till rose by exactly the buyer's coins");
    assert_eq!(treasury_coins(&w), t0, "the Treasury ends where it started");
    assert_eq!(ownership::total_coins(&w), total, "conserved");

    // A city-owned Clinic: no coin moves, the till holds.
    let mut w = city();
    let clinic = w.buildings_of_kind(BuildingKind::Clinic)[0];
    ownership::transfer_building(&mut w, clinic, None);
    assert!(w.owner_of(clinic).is_none());
    let recycler = only_recycler_sells(&mut w, 40, Some(clinic));
    let (t0, till0, total) = (treasury_coins(&w), w.econ.recycler_till, ownership::total_coins(&w));
    assets::parts_market(&mut w);
    assert!(w.stock(recycler, Good::Parts) < 40, "the city's Clinic took the Recycler's Parts");
    assert_eq!(w.econ.recycler_till, till0, "no phantom drain into the till");
    assert_eq!(treasury_coins(&w), t0, "the Treasury is untouched");
    assert_eq!(ownership::total_coins(&w), total, "conserved");
}

/// E33: the Recycler's Parts sold to the World at midnight fill the till by
/// exactly what the crossing paid the Treasury (tax 0, so no other seller's
/// sale touches the Treasury); the identity holds through the pass.
#[test]
fn test_world_market_recycler_export_fills_till() {
    let mut w = city();
    w.levers.tax_rate = 0.0;
    let floor = w.config.export.parts_floor;
    let recycler = only_recycler_sells(&mut w, floor + 30, None);
    let (t0, till0, id0) = (treasury_coins(&w), w.econ.recycler_till, econ::identity(&w));
    world_market::daily(&mut w);
    let sold = floor + 30 - w.stock(recycler, Good::Parts);
    assert!(sold > 0, "the Recycler sold to the World");
    assert!(w.econ.recycler_till > till0, "the till rose");
    assert_eq!(treasury_coins(&w), t0, "the crossing went on to the till: the Treasury ends where it started");
    assert_eq!(econ::identity(&w), id0, "the identity holds through the pass");
}

/// The coin identity is constant to the coin over a short no-net run (the
/// till inside `total_coins`).
#[test]
fn test_identity_constant_with_no_net() {
    let mut w = city();
    let id0 = econ::identity(&w);
    w.run_ticks(6 * TICKS_PER_DAY);
    assert!(w.stats.history.len() >= 5);
    for r in &w.stats.history {
        assert_eq!(r.econ.coin_identity, id0, "day {}: the identity moved", r.day);
    }
    assert_eq!(econ::identity(&w), id0);
}
