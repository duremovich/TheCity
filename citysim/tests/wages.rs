//! The Real economy phase 2 (docs/ECONOMY_V2.md § 5; plan 2.9): wages from
//! revenue: the wage rule's convergence and clamps, hiring and layoffs on
//! the margin, Farm overtime from the World's unfilled order, inputs and the
//! property rate in place of upkeep, the working capital at seed, a Grow
//! refit on World demand, the revenue window's exclusions, and the off path.

use std::collections::VecDeque;

use citysim::components::{AssetKind, AssetLoc, Good};
use citysim::outside::{ExportGood, WORLD_ACCOUNT};
use citysim::systems::ownership::{self, Flow};
use citysim::systems::{corp_brain, econ, wages, world_market as wm};
use citysim::{
    Building, BuildingKind, Config, Corp, CorpOrder, EntityId, EventKind, Job, Market, Niche, PlayerCommand, Wallet,
    World, TICKS_PER_DAY,
};

/// The phase-2 city: `[economy2] wages` is off by default until the stop
/// rule is answered, so the tests force it on.
fn config() -> Config {
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg
}

fn city() -> World {
    let w = World::new(42, config());
    assert!(wages::on(&w) && econ::market_on(&w), "wages and the market are on for the tests");
    w
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == name)).expect(name)
}

fn pin_windows(w: &mut World, corp: EntityId, rev: i64, pay: i64) {
    let c = w.comp_mut::<Corp>(corp).expect("corp");
    c.rev = VecDeque::from(vec![rev; wages::WINDOW_DAYS]);
    c.pay = VecDeque::from(vec![pay; wages::WINDOW_DAYS]);
}

fn world_treasury(w: &World) -> i64 {
    w.outside.faction(WORLD_ACCOUNT).map_or(0, |f| f.treasury)
}

/// A corp's Markets read demand 1.0 (seven days of full sell-through).
fn pin_food_demand(w: &mut World, corp: EntityId) {
    for m in ownership::owned_of_kind(w, Some(corp), BuildingKind::Market) {
        if let Some(mk) = w.comp_mut::<Market>(m) {
            mk.sales = VecDeque::from(vec![100; 7]);
            mk.stock_hist = VecDeque::from(vec![100; 7]);
        }
    }
    assert!(corp_brain::demand_of(w, corp, Niche::Food).0 >= 0.99);
}

/// The World's Food rings: `caps` and `bought` for seven days.
fn pin_food_book(w: &mut World, cap: u32, bought: u32) {
    let f = w.outside.faction_mut(WORLD_ACCOUNT).expect("the World account");
    let b = f.books.get_mut(&ExportGood::Food).expect("the Food book");
    b.caps = VecDeque::from(vec![cap; 7]);
    b.bought = VecDeque::from(vec![bought; 7]);
}

/// E17: with revenue pinned the payroll lands within 10 % of `labour_share
/// × R` in ≤ 20 days, never leaves its clamps; the Squeeze 0.9 multiplies
/// on top of `wage_rev`.
#[test]
fn test_wage_rule_converges_on_fixed_revenue() {
    let mut w = city();
    let cfg = w.config.economy2.clone();
    let corp = corp_named(&w, "Nutrix");
    let share = wages::labour_share(&w, corp);
    assert!((0.59..0.61).contains(&share), "a Food-only corp's share is labour_share.food ({share})");
    let (rev, base_pay) = (1500i64, 600i64);
    let target = share * rev as f32;
    let mut converged = None;
    for day in 1..=20 {
        let wr = w.comp::<Corp>(corp).map_or(1.0, |c| c.wage_rev);
        pin_windows(&mut w, corp, rev, (base_pay as f32 * wr).round() as i64);
        wages::daily(&mut w);
        let wr = w.comp::<Corp>(corp).map_or(1.0, |c| c.wage_rev);
        assert!((cfg.wage_floor_mult..=cfg.wage_cap_mult).contains(&wr), "day {day}: {wr} inside the clamps");
        let p = base_pay as f32 * wr;
        if (p - target).abs() <= 0.1 * target && converged.is_none() {
            converged = Some(day);
        }
    }
    let wr = w.comp::<Corp>(corp).map_or(1.0, |c| c.wage_rev);
    eprintln!(
        "converged on day {converged:?}: wage_rev {wr} (target {target:.0}, payroll {:.0})",
        base_pay as f32 * wr
    );
    assert!(converged.is_some(), "within 10 % of P* in 20 days (wage_rev {wr})");
    assert!(w.events.iter().any(|e| e.kind == EventKind::WageMoved), "a WageMoved event on a 0.1 step");
    // The clamps: a corp with no revenue falls to the floor; one with a
    // week of empty payroll against revenue climbs to the cap, never past.
    for _ in 0..100 {
        pin_windows(&mut w, corp, 0, 500);
        wages::daily(&mut w);
    }
    assert_eq!(w.comp::<Corp>(corp).map(|c| c.wage_rev), Some(cfg.wage_floor_mult));
    for _ in 0..100 {
        pin_windows(&mut w, corp, 100_000, 10);
        wages::daily(&mut w);
    }
    assert_eq!(w.comp::<Corp>(corp).map(|c| c.wage_rev), Some(cfg.wage_cap_mult));
    // The Squeeze multiplies on top (E15): the gross wage is round(wage × 0.9 × wage_rev).
    let worker = ownership::employees_of(&w, corp).into_iter().find(|&a| w.has::<Job>(a)).expect("a worker");
    let base = w.comp::<Job>(worker).map_or(0, |j| j.wage_per_day);
    if let Some(c) = w.comp_mut::<Corp>(corp) {
        c.wage_mult = 0.9;
    }
    let job = w.comp::<Job>(worker).cloned().expect("job");
    let mult = 0.9f32 * cfg.wage_cap_mult;
    assert_eq!(wages::gross_wage(&w, &job), (base as f32 * mult).round() as i64);
}

/// E18: three days under `hire_below × P*` with demand post a vacancy a
/// building a day; seven days over `fire_above × P*` lay the newest off
/// only once `wage_rev` sits at the floor.
#[test]
fn test_hiring_posts_below_and_lays_off_above_at_floor() {
    let mut w = city();
    let cfg = w.config.economy2.clone();
    let corp = corp_named(&w, "Nutrix");
    pin_food_demand(&mut w, corp);
    let open = |w: &World| -> usize {
        let c = w.comp::<Corp>(corp).expect("corp");
        c.buildings.iter().map(|b| w.vacancies.get(b).map_or(0, |v| v.len())).sum()
    };
    // Under the target (payroll 100 against a target of 1,800): nothing on
    // days 1 and 2, a posting on day 3.
    let before = open(&w);
    for day in 1..=3 {
        pin_windows(&mut w, corp, 3000, 100);
        wages::staff(&mut w);
        let now = open(&w);
        if day < 3 {
            assert_eq!(now, before, "day {day}: no posting before {} days", 3);
        } else {
            assert!(now > before, "day 3: the margin rule posted ({before} -> {now})");
            assert!(!w.econ.vacancy_since.is_empty(), "the postings are stamped");
        }
    }
    // Without demand nothing posts.
    let mut w2 = city();
    let corp2 = corp_named(&w2, "Nutrix");
    let before2 = open(&w2);
    for _ in 0..5 {
        pin_windows(&mut w2, corp2, 3000, 100);
        wages::staff(&mut w2);
    }
    assert_eq!(open(&w2), before2, "no niche demand: no posting");
    // Over the target for seven days: no layoff while wage_rev is above the
    // floor; at the floor the newest non-Farm hire goes, one a day.
    let mut w = city();
    let corp = corp_named(&w, "Nutrix");
    let staff = |w: &World| ownership::employees_of(w, corp).len();
    let n0 = staff(&w);
    for _ in 0..8 {
        pin_windows(&mut w, corp, 100, 1000);
        wages::staff(&mut w);
    }
    assert_eq!(staff(&w), n0, "wage_rev 1.0 > floor: no layoff yet");
    if let Some(c) = w.comp_mut::<Corp>(corp) {
        c.wage_rev = cfg.wage_floor_mult;
    }
    let exec = w.comp::<Corp>(corp).and_then(|c| c.exec);
    let spare_farms = w.config.corps.hunker_spares_farms;
    let expected = ownership::employees_of(&w, corp)
        .into_iter()
        .filter(|&a| Some(a) != exec)
        .filter(|&a| {
            let b = w.comp::<Job>(a).and_then(|j| j.employer).expect("employer");
            !(spare_farms && w.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Farm))
        })
        .max_by_key(|&a| (w.comp::<Job>(a).map_or(0, |j| j.hired_tick), a))
        .expect("a candidate");
    pin_windows(&mut w, corp, 100, 1000);
    wages::staff(&mut w);
    assert_eq!(staff(&w), n0 - 1, "one laid off");
    assert!(!w.has::<Job>(expected), "the newest (ties the higher id) went");
    assert!(w.events.iter().any(|e| e.kind == EventKind::LaidOff && e.actors.first() == Some(&expected)));
    assert_eq!(w.stats.current.econ.laid_off, 1);
    assert!(wages::over_margin(&w, corp), "the corp replaces no quitter while over the margin");
}

/// E18: the unfilled World Food order (7-day mean of `cap − bought`) lifts
/// a corp's Farm staff cap by `export_staff` per 100 units, split across
/// its Farms.
#[test]
fn test_farm_overtime_from_unfilled_world_order() {
    let mut w = city();
    assert_eq!(wages::food_order(&w), 0.0, "no rings at seed");
    pin_food_book(&mut w, 350, 50);
    assert!((wages::food_order(&w) - 300.0).abs() < 1e-3);
    let corp = corp_named(&w, "Nutrix");
    pin_food_demand(&mut w, corp);
    let farms = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm);
    assert!(!farms.is_empty());
    let full = corp_brain::full_staff(&w, BuildingKind::Farm);
    let extra = (w.config.economy2.export_staff * 300.0 / 100.0 / farms.len() as f32).round() as usize;
    assert!(extra >= 1, "the order is worth a place per Farm ({extra})");
    let places = |w: &World, f: EntityId| ownership::staff_at(w, f).len() + w.vacancies.get(&f).map_or(0, |v| v.len());
    for _ in 0..(full + extra + 3) {
        pin_windows(&mut w, corp, 3000, 100);
        wages::staff(&mut w);
    }
    for &f in &farms {
        assert_eq!(places(&w, f), full + extra, "Farm {f:?}: full staff plus the overtime");
    }
    // Without an order the cap is the full staff: nothing past it.
    let mut w = city();
    let corp = corp_named(&w, "Nutrix");
    pin_food_demand(&mut w, corp);
    for _ in 0..(full + 3) {
        pin_windows(&mut w, corp, 3000, 100);
        wages::staff(&mut w);
    }
    for f in ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm) {
        assert_eq!(places(&w, f), full);
    }
}

/// E13: a producer's inputs cross out per unit (the fraction carried), the
/// property rate goes to the Treasury and `[corps] upkeep` is not charged;
/// the identity holds across the crossings.
#[test]
fn test_upkeep_becomes_property_and_inputs() {
    let mut w = city();
    let corp = corp_named(&w, "Nutrix");
    let farm = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm)[0];
    let per = w.config.economy2.input_per_food;
    let (purse, world_t, ident) = (w.purse(Some(corp)), world_treasury(&w), econ::identity(&w));
    wages::produce_inputs(&mut w, farm, 10, per);
    let expect = (10.0 * per).floor() as i64;
    assert_eq!(w.purse(Some(corp)), purse - expect, "10 units × input_per_food crossed out");
    assert_eq!(world_treasury(&w), world_t + expect);
    assert_eq!(w.stats.current.econ.flow_inputs, expect);
    assert_eq!(econ::identity(&w), ident);
    // Three more units carry the fraction: 13 × 0.5 = 6.5 -> 6 crossed, 0.5 carried.
    wages::produce_inputs(&mut w, farm, 3, per);
    let carried = w.comp::<Building>(farm).map_or(0.0, |b| b.input_accum);
    assert_eq!(w.stats.current.econ.flow_inputs, (13.0 * per).floor() as i64);
    assert!((carried - (13.0 * per - (13.0 * per).floor())).abs() < 1e-5);
    // A Farm's work pays as it produces.
    let farmer = ownership::staff_at(&w, farm).into_iter().next().expect("a farmer");
    let (stock, before) = (w.comp::<Building>(farm).map_or(0, |b| b.stock_food), w.stats.current.econ.flow_inputs);
    citysim::systems::economy::accrue_farm_work(&mut w, farmer, farm, 6 * 60);
    let made = w.comp::<Building>(farm).map_or(0, |b| b.stock_food) - stock;
    assert!(made > 0, "six hours of work made food");
    assert!(w.stats.current.econ.flow_inputs >= before + (made as f32 * per).floor() as i64 - 1);
    // The first midnight: the property rate, the power, no upkeep.
    let expect_property: i64 = w
        .with::<Building>()
        .into_iter()
        .filter_map(|b| w.comp::<Building>(b))
        .filter(|bd| !bd.demolished && !bd.derelict && bd.owner.is_some())
        .map(|bd| w.config.treasury.property_rate.for_building(bd.kind, bd.tier))
        .sum();
    let expect_power: i64 = w
        .with::<Building>()
        .into_iter()
        .filter_map(|b| w.comp::<Building>(b))
        .filter(|bd| !bd.demolished && !bd.derelict)
        .map(|bd| w.config.economy2.power.for_kind(bd.kind))
        .sum();
    let inputs_before = w.stats.current.econ.flow_inputs;
    w.run_ticks(1);
    let row = &w.stats.current;
    assert_eq!(row.flow_upkeep, 0, "[corps] upkeep is not charged with wages on");
    assert_eq!(row.econ.flow_property, expect_property, "the property rate per non-city standing building");
    assert!(row.econ.flow_inputs >= inputs_before + expect_power, "the daily power crossed out");
    assert!(expect_property > 0 && expect_power > 0);
    // Ten days: the identity constant to the coin every day.
    w.run_ticks(10 * TICKS_PER_DAY);
    let ids: Vec<i64> = w.stats.history.iter().map(|r| r.econ.coin_identity).collect();
    assert!(ids.windows(2).all(|p| p[0] == p[1]), "the identity holds: {ids:?}");
    assert!(w.stats.history.iter().all(|r| r.flow_upkeep == 0));
}

/// E14: the hoard moves to the corps pro rata to the day-0 wage bill; the
/// Treasury opens at `[treasury] treasury_initial`; coins are conserved.
#[test]
fn test_seed_capital_moves_hoard_pro_rata_and_conserves() {
    let w = city();
    let cfg = &w.config;
    let moved = cfg.world.treasury_initial - cfg.treasury.treasury_initial;
    // The same seed with wages off: the Treasury less what seeding spent.
    let mut off = Config::load();
    off.economy2.wages = false;
    let w_off = World::new(42, off);
    let t_off = w_off.treasury().map_or(0, |t| t.coins);
    assert!(t_off <= cfg.world.treasury_initial && t_off > cfg.world.treasury_initial - 1_000, "seeding spends little");
    assert_eq!(w.treasury().map(|t| t.coins), Some(t_off - moved), "the Treasury's working balance");
    let corps_total = |w: &World| w.corps().iter().filter_map(|&c| w.comp::<Corp>(c)).map(|c| c.treasury).sum::<i64>();
    // The corps hold what the wages-off seed gave them plus the capital
    // (seeding trades: the same seed is the base).
    assert_eq!(corps_total(&w), corps_total(&w_off) + moved, "Σ corps = the off seed's + the hoard moved");
    // Pro rata: the share order follows the wage bill order; Σ shares = moved.
    let mut by_bill: Vec<(i64, i64, String)> = w
        .corps()
        .into_iter()
        .filter_map(|c| {
            let cc = w.comp::<Corp>(c)?;
            let off = w_off.comp::<Corp>(c)?;
            Some((corp_brain::wage_bill(&w, c), cc.treasury - off.treasury, cc.name.clone()))
        })
        .collect();
    assert_eq!(by_bill.iter().map(|&(_, s, _)| s).sum::<i64>(), moved);
    by_bill.sort();
    assert!(by_bill.windows(2).all(|p| p[0].1 <= p[1].1 + 1), "shares follow the bills: {by_bill:?}");
    for c in w.corps() {
        let cc = w.comp::<Corp>(c).expect("corp");
        let off = w_off.comp::<Corp>(c).expect("corp");
        assert_eq!(
            cc.treasury_ref,
            off.treasury_ref + (cc.treasury - off.treasury),
            "treasury_ref rose with the share"
        );
        assert_eq!(cc.cashflow_today, off.cashflow_today, "a capital move is not a day of trading");
    }
    eprintln!("working capital: {by_bill:?}");
    // Conservation: the same seed with wages off holds the same coins.
    assert_eq!(ownership::total_coins(&w), ownership::total_coins(&w_off), "the move conserves coins");
    assert_eq!(w.stats.current.flow_other, moved, "a Subsidy ledger line");
}

/// E19: no Lot left, a derelict Block standing, the World's Food fill at
/// 0.9: a Food corp under Grow refits the derelict as a Farm and
/// `grow_world` counts it.
#[test]
fn test_grow_refits_when_no_lot_and_world_demand() {
    let mut w = city();
    let corp = corp_named(&w, "Nutrix");
    w.push_command(PlayerCommand::SetCorpOrder { corp, order: CorpOrder::Grow, niche: Some(Niche::Food), days: 30 });
    w.run_ticks(1);
    // Tick 0's Grow may have built on a Lot: the cooldown is cleared.
    if let Some(c) = w.comp_mut::<Corp>(corp) {
        c.last_build_tick = None;
    }
    for lot in citysim::systems::founding::vacant_lots(&w) {
        if let Some(b) = w.comp_mut::<Building>(lot) {
            b.demolished = true;
        }
    }
    assert!(citysim::systems::founding::vacant_lots(&w).is_empty());
    let derelicts = citysim::systems::street::derelicts(&w).len();
    assert!(derelicts > 0, "a derelict stands at seed");
    pin_food_book(&mut w, 100, 90);
    let (d, from_world) = corp_brain::demand_of(&w, corp, Niche::Food);
    assert!(from_world && (d - 0.9).abs() < 1e-3, "the demand input came from the World ({d}, {from_world})");
    let i = corp_brain::gather_inputs(&w, corp).expect("inputs");
    let ni = &i.niches[&Niche::Food];
    assert!(ni.world_demand && ni.lots == 1, "a refit target counts as a Lot: {ni:?}");
    let farms = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm).len();
    let purse = w.purse(Some(corp));
    assert_eq!(w.comp::<Corp>(corp).map(|c| c.order), Some(CorpOrder::Grow));
    corp_brain::act(&mut w, corp);
    assert_eq!(ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm).len(), farms + 1, "a Farm refitted");
    assert_eq!(citysim::systems::street::derelicts(&w).len(), derelicts - 1);
    let cost = (w.config.jobs.refit_frac * w.config.economy2.farm_found_cost as f32).round() as i64;
    assert_eq!(w.purse(Some(corp)), purse - cost, "refit_frac × farm_found_cost paid");
    assert_eq!(w.stats.current.econ.grow_world, 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Refit && e.actors.first() == Some(&corp)));
    assert!(w
        .events
        .iter()
        .any(|e| e.kind == EventKind::Founded && e.text.contains("World demand") && e.actors.first() == Some(&corp)));
}

/// E16: a corp's revenue window counts taxed inflows net of tax and
/// exports; never a capital flow, a crossing other than Export, or a
/// transfer to itself.
#[test]
fn test_revenue_window_skips_capital_and_self_transfers() {
    let mut w = city();
    let corp = corp_named(&w, "Nutrix");
    // Seeding itself trades (the NoodleBars' opening restock is wholesale
    // revenue to Nutrix's Markets): the window is read from there.
    let r0 = w.comp::<Corp>(corp).map_or(0, |c| c.rev_today);
    let rev = |w: &World| w.comp::<Corp>(corp).map_or(0, |c| c.rev_today) - r0;
    assert_eq!(rev(&w), 0);
    let agent = w.with::<Wallet>().into_iter().find(|&a| w.has::<citysim::Brain>(a)).expect("an agent");
    if let Some(wl) = w.comp_mut::<Wallet>(agent) {
        wl.coins = 1_000;
    }
    let rate = w.levers.tax_rate;
    // The payee's tax remainder from seeding is taken into account.
    let acc0 = w.tax_accum.get(&corp).copied().unwrap_or(0.0);
    ownership::pay(&mut w, Some(agent), Some(corp), 100, Flow::Food);
    let tax = (acc0 + 100.0 * rate).floor() as i64;
    assert_eq!(rev(&w), 100 - tax, "a taxed inflow net of tax");
    ownership::pay(&mut w, Some(corp), Some(corp), 50, Flow::Food);
    assert_eq!(rev(&w), 100 - tax, "a transfer to itself is no revenue");
    ownership::pay(&mut w, None, Some(corp), 500, Flow::Subsidy);
    assert_eq!(rev(&w), 100 - tax, "a capital flow is no revenue");
    ownership::cross_in(&mut w, WORLD_ACCOUNT, Some(corp), 200, Flow::Migrant);
    assert_eq!(rev(&w), 100 - tax, "a crossing other than Export is no revenue");
    ownership::pay(&mut w, Some(agent), Some(corp), 40, Flow::Wage);
    assert_eq!(rev(&w), 100 - tax, "an untaxed flow is no revenue");
    let before = rev(&w);
    let (units, paid) = wm::sell(&mut w, None, Some(corp), ExportGood::Food, 20);
    assert!(units == 20 && paid > 0);
    let export_tax = (rate * paid as f32).round() as i64;
    assert_eq!(rev(&w), before + paid - export_tax, "an export counts net of its tax");
    // Payroll: a wage paid by the corp lands in `pay_today` gross.
    let worker = ownership::employees_of(&w, corp).into_iter().find(|&a| w.has::<Job>(a)).expect("a worker");
    if let Some(j) = w.comp_mut::<Job>(worker) {
        j.days_unpaid = 1;
    }
    let gross = w.comp::<Job>(worker).map(|j| wages::gross_wage(&w, j)).expect("gross");
    citysim::systems::economy::collect_wage(&mut w, worker);
    assert_eq!(w.comp::<Corp>(corp).map(|c| c.pay_today), Some(gross), "the gross wage is payroll");
    // Tick 0's roll takes the day's counters and pushes nothing (as
    // `cashflow`, not a day of trading); the next midnight rolls both into
    // the windows.
    w.run_ticks(TICKS_PER_DAY + 1);
    let c = w.comp::<Corp>(corp).expect("corp");
    assert_eq!(c.rev.len(), 1);
    assert_eq!(c.pay.len(), 1);
    // (`rev_today` is not 0 here: the midnight restocks trade after the roll.)
}

/// E10 (phase 2): a Clinic's chrome in stock beyond `asset_floor` sells to
/// the World as `parts_per` Parts each at the Parts bid and despawns.
#[test]
fn test_clinic_chrome_sells_as_parts_equivalents() {
    let mut w = city();
    let clinic = w
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .find(|&b| w.corp_of_building(b).is_some())
        .expect("a corp Clinic");
    let owner = w.owner_of(clinic);
    let floor = w.config.world_market.asset_floor as usize;
    let kind = AssetKind::Implant(citysim::components::Slot::Arms);
    let per = *w.config.assets.parts_per.get(kind);
    let in_stock = |w: &World| {
        citysim::systems::assets::assets_at(w, clinic)
            .iter()
            .filter(|&&a| {
                w.comp::<citysim::components::Asset>(a)
                    .is_some_and(|x| x.kind == kind && x.loc == AssetLoc::Stock(clinic))
            })
            .count()
    };
    let have = in_stock(&w);
    let want = floor + 3;
    for _ in have..want {
        citysim::systems::assets::spawn_asset(&mut w, kind, 1, owner, AssetLoc::Stock(clinic), 60);
    }
    assert_eq!(in_stock(&w), want);
    let (purse, bought) = (w.purse(owner), wm::book(&w, ExportGood::Parts).map_or(0, |b| b.bought_today));
    w.run_ticks(TICKS_PER_DAY - w.tick_of_day() as u64);
    assert_eq!(in_stock(&w), floor, "the stock beyond the floor sold: {}", in_stock(&w));
    let sold = wm::book(&w, ExportGood::Parts).map_or(0, |b| b.bought_today)
        + wm::book(&w, ExportGood::Parts).map_or(0, |b| b.bought.back().copied().unwrap_or(0));
    assert!(sold >= bought + 3 * per, "3 × parts_per Parts bought by the World ({sold})");
    assert!(w.purse(owner) > purse - 2_000, "the owner was paid (purse {} -> {})", purse, w.purse(owner));
    let _ = Good::Parts;
}

/// The off path: with `[economy2] wages = false` nothing of phase 2 runs in
/// three days (the byte identity against the phase-1 CLI is the device).
#[test]
fn test_wages_off_identical() {
    let mut cfg = Config::load();
    cfg.economy2.wages = false;
    let mut w = World::new(42, cfg);
    assert!(!wages::on(&w) && econ::market_on(&w));
    assert!(w.treasury().map_or(0, |t| t.coins) > w.config.world.treasury_initial - 1_000, "no capital moved");
    w.run_ticks(3 * TICKS_PER_DAY);
    for r in &w.stats.history {
        assert_eq!(r.econ.flow_property, 0);
        assert_eq!(r.econ.flow_inputs, 0);
        assert!(r.flow_upkeep > 0, "[corps] upkeep as before");
        assert_eq!(r.econ.wage_mult_mean, 0.0);
        assert_eq!(r.econ.vacancies_open, 0);
        assert_eq!(r.econ.laid_off, 0);
        assert_eq!(r.econ.pop_inflow_wages, 0);
        assert_eq!(r.econ.pop_inflow_other, 0);
        assert_eq!(r.econ.grow_world, 0);
    }
    for c in w.corps() {
        let cc = w.comp::<Corp>(c).expect("corp");
        assert!(cc.rev.is_empty() && cc.pay.is_empty() && cc.wage_rev == 1.0 && cc.rev_today == 0 && cc.pay_today == 0);
        assert!(cc.hire_days == 0 && cc.fire_days == 0);
    }
    assert!(w.econ.vacancy_since.is_empty());
    assert!(w.with::<Building>().into_iter().all(|b| w.comp::<Building>(b).is_some_and(|bd| bd.input_accum == 0.0)));
    assert!(!w.events.iter().any(|e| matches!(e.kind, EventKind::WageMoved | EventKind::LaidOff)));
    assert!(!w.events.iter().any(|e| e.text.contains("World demand")));
}
