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
    // The jobs round's keys at their off values: these tests pin phase 2's
    // rules (the round's own tests turn each key on).
    cfg.economy2.staff_ceiling_mult = 1.0;
    cfg.economy2.capital_hi_days = 0.0;
    cfg.economy2.capital_lo_days = 0.0;
    cfg.economy2.fleet_floor_days = 0.0;
    cfg.economy2.asset_import_per_day = 0;
    cfg.economy2.overflow_paid_only = false;
    cfg.economy2.emigrate_cost = 0;
    cfg.world_market.cap_wages = [0; 3];
    cfg
}

fn city() -> World {
    let w = World::new(42, config());
    assert!(wages::on(&w), "wages and the market are on for the tests");
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
    assert!(!wages::on(&w));
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

// ---------------------------------------------------------------------------
// The jobs round (after phase 2's stop rule): staffing past full_staff from
// revenue, counter-cyclical capital, the overflow leg paid only, the asset
// import budget, priced emigration.
// ---------------------------------------------------------------------------

/// A phase-2 city with the jobs round's keys at their off values (each test
/// turns on the one it pins).
fn round_city(f: impl FnOnce(&mut Config)) -> World {
    let mut cfg = config();
    cfg.economy2.staff_ceiling_mult = 1.0;
    cfg.economy2.capital_hi_days = 0.0;
    cfg.economy2.capital_lo_days = 0.0;
    cfg.economy2.fleet_floor_days = 0.0;
    cfg.economy2.asset_import_per_day = 0;
    cfg.economy2.overflow_paid_only = false;
    cfg.economy2.emigrate_cost = 0;
    cfg.world_market.cap_wages = [0; 3];
    f(&mut cfg);
    let w = World::new(42, cfg);
    assert!(wages::on(&w));
    w
}

/// Staffing from revenue: under its margin a corp posts past `full_staff`
/// up to `ceil(full_staff × staff_ceiling_mult)` while `hire_below × P* −
/// P` covers the marginal hire; with the room short of a wage it stops at
/// full staff, and at mult 1.0 the cap is phase 2's.
#[test]
fn test_staffing_past_full_staff_only_when_revenue_covers_the_hire() {
    let places = |w: &World, b: EntityId| {
        let role = w.comp::<Building>(b).and_then(|bd| ownership::role_for(bd.kind)).expect("a staffed kind");
        ownership::staff_at(w, b).into_iter().filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.role == role)).count()
            + w.vacancies.get(&b).map_or(0, |v| v.iter().filter(|&&r| r == role).count())
    };
    let run = |mult: f32, rev: i64, pay: i64| -> (World, Vec<EntityId>) {
        let mut w = round_city(|c| c.economy2.staff_ceiling_mult = mult);
        let corp = corp_named(&w, "Nutrix");
        pin_food_demand(&mut w, corp);
        let markets = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Market);
        assert!(!markets.is_empty());
        for _ in 0..60 {
            pin_windows(&mut w, corp, rev, pay);
            wages::staff(&mut w);
        }
        (w, markets)
    };
    let full = |w: &World| corp_brain::full_staff(w, BuildingKind::Market);
    // Room to spare (target 0.6 × 3,000; payroll 100): every Market to the ceiling.
    let (w, markets) = run(2.0, 3000, 100);
    let ceiling = wages::staff_ceiling(&w, BuildingKind::Market);
    assert_eq!(ceiling, (full(&w) as f32 * 2.0).ceil() as usize);
    assert!(ceiling > full(&w));
    for &m in &markets {
        assert_eq!(places(&w, m), ceiling, "Market {m:?}: posted to the ceiling");
    }
    // A room under one wage (0.85 × 0.6 × 200 − 100 = 2 a day): full staff only.
    let (w, markets) = run(2.0, 200, 100);
    for &m in &markets {
        assert_eq!(places(&w, m), full(&w), "Market {m:?}: no room for the marginal hire");
    }
    // mult 1.0: phase 2's cap whatever the room.
    let (w, markets) = run(1.0, 3000, 100);
    assert_eq!(wages::staff_ceiling(&w, BuildingKind::Market), full(&w));
    for &m in &markets {
        assert_eq!(places(&w, m), full(&w));
    }
}

/// Counter-cyclical capital: a purse above `capital_hi_days × max(R, P)`
/// adds the excess ÷ `capital_payout_days` a day to `P*`; inside the band
/// (or with the key 0) the target is phase 2's `labour_share × R`; the wage
/// rule climbs on the payout.
#[test]
fn test_capital_above_the_band_raises_the_payroll_target() {
    let mut w = round_city(|c| {
        c.economy2.capital_hi_days = 14.0;
        c.economy2.capital_payout_days = 30.0;
    });
    let corp = corp_named(&w, "Nutrix");
    let share = wages::labour_share(&w, corp);
    pin_windows(&mut w, corp, 200, 100);
    let purse = w.purse(Some(corp));
    let band = 14.0 * 200.0;
    assert!(purse as f32 > band, "Nutrix opens above a 14-day band ({purse})");
    let expect = share * 200.0 + (purse as f32 - band) / 30.0;
    let t = wages::target(&w, corp).expect("full windows");
    assert!((t - expect).abs() < 1e-2, "P* {t} = share × R + excess / 30 ({expect})");
    assert!((wages::capital_payout(&w, corp, 200.0, 100.0) - (purse as f32 - band) / 30.0).abs() < 1e-2);
    // Inside the band: no payout.
    let take = purse - 1000;
    ownership::pay(&mut w, Some(corp), None, take, Flow::Subsidy);
    assert_eq!(wages::capital_payout(&w, corp, 200.0, 100.0), 0.0);
    assert!((wages::target(&w, corp).expect("windows") - share * 200.0).abs() < 1e-3);
    // The band reads max(R, P): a payroll over revenue widens it.
    ownership::pay(&mut w, None, Some(corp), take, Flow::Subsidy);
    let wide = wages::capital_payout(&w, corp, 200.0, 2_000.0);
    assert!(wide < wages::capital_payout(&w, corp, 200.0, 100.0));
    // The rule pays it out: payroll 100 against a target well over it.
    let wr0 = w.comp::<Corp>(corp).map_or(1.0, |c| c.wage_rev);
    for _ in 0..5 {
        pin_windows(&mut w, corp, 200, 100);
        wages::daily(&mut w);
    }
    assert!(w.comp::<Corp>(corp).map_or(1.0, |c| c.wage_rev) > wr0, "wages rise on the payout");
    // Off (0 days): phase 2's target.
    let mut w = round_city(|_| {});
    let corp = corp_named(&w, "Nutrix");
    pin_windows(&mut w, corp, 200, 100);
    assert_eq!(wages::capital_payout(&w, corp, 200.0, 100.0), 0.0);
    assert!((wages::target(&w, corp).expect("windows") - wages::labour_share(&w, corp) * 200.0).abs() < 1e-3);
}

/// The overflow leg paid only: with the Treasury in the red the Reserve
/// buys nothing from a corp's Farm and the surplus stays at the Farm (phase
/// 2 charged a negative Treasury); with coins it buys what they cover.
#[test]
fn test_overflow_buys_only_from_a_positive_treasury() {
    let setup = |paid_only: bool, treasury: i64| -> (World, EntityId) {
        let mut w = round_city(|c| c.economy2.overflow_paid_only = paid_only);
        let corp = corp_named(&w, "Nutrix");
        // The Farm's own Markets full and the World closed: the haul's
        // surplus goes to the Reserve leg.
        w.econ.world_closed = true;
        let cap = w.config.buildings.market.stock_cap;
        for m in ownership::owned_of_kind(&w, Some(corp), BuildingKind::Market) {
            w.comp_mut::<Building>(m).expect("market").stock_food = cap;
        }
        let farm = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm)[0];
        w.comp_mut::<Building>(farm).expect("farm").stock_food = 200;
        w.treasury_mut().expect("treasury").coins = treasury;
        (w, farm)
    };
    let batch = Config::load().economy.haul_batch;
    let wholesale = Config::load().corps.wholesale;
    // Phase 2: a negative Treasury still pays (the magic money).
    let (mut w, farm) = setup(false, -500);
    let moved = citysim::systems::economy::haul(&mut w, farm, None);
    assert_eq!(moved, batch);
    assert_eq!(w.stats.current.flow_overflow, i64::from(batch) * wholesale);
    // (the Wholesale's withheld tax comes back to the Treasury)
    assert!(w.treasury().map_or(0, |t| t.coins) < -500, "the red Treasury paid");
    // Paid only, in the red: nothing bought, nothing lost, the Treasury untouched.
    let (mut w, farm) = setup(true, -500);
    let moved = citysim::systems::economy::haul(&mut w, farm, None);
    assert_eq!(moved, 0);
    assert_eq!(w.comp::<Building>(farm).map(|b| b.stock_food), Some(200), "the surplus stays at the Farm");
    assert_eq!(w.stats.current.flow_overflow, 0);
    assert_eq!(w.treasury().map(|t| t.coins), Some(-500));
    // Paid only, 10 units' worth of coins: 10 bought, the rest stays.
    let (mut w, farm) = setup(true, 10 * wholesale);
    let moved = citysim::systems::economy::haul(&mut w, farm, None);
    assert_eq!(moved, 10);
    assert_eq!(w.comp::<Building>(farm).map(|b| b.stock_food), Some(190));
    assert_eq!(w.stats.current.flow_overflow, 10 * wholesale);
    assert!(w.treasury().map_or(-1, |t| t.coins) >= 0, "never into the red");
}

/// The asset import budget: a corp fronts at most `asset_import_per_day` of
/// asset imports a day (the sale past it is refused, the buyer keeps the
/// coins); the midnight roll resets it.
#[test]
fn test_asset_import_budget_refuses_past_the_day_cap() {
    use citysim::systems::assets;
    use citysim::ShopPick;
    let mut w = round_city(|_| {});
    let g = w
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .copied()
        .find(|&b| w.corp_of_building(b).is_some())
        .expect("a corp Garage");
    let corp = w.corp_of_building(g).expect("its corp");
    let parts = w.stock(g, Good::Parts);
    w.take_stock(g, Good::Parts, parts);
    let buyers: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| w.has::<citysim::Brain>(a) && w.has::<Wallet>(a))
        .filter(|&a| citysim::systems::demography::is_adult(&w, a))
        .take(3)
        .collect();
    for &b in &buyers {
        w.comp_mut::<Wallet>(b).expect("wallet").coins = 5_000;
    }
    let pick = ShopPick { kind: AssetKind::Motorcycle, tier: 1, used: None, upgrade: false };
    // Unlimited (0): the import is counted.
    assets::buy(&mut w, buyers[0], g, &pick).expect("a sale");
    let one = w.comp::<Corp>(corp).map_or(0, |c| c.import_today);
    assert!(one > 0, "the sale fronted an import");
    // The cap at one sale's import: the second sale today is refused.
    w.config.economy2.asset_import_per_day = one;
    let coins = w.comp::<Wallet>(buyers[1]).map_or(0, |x| x.coins);
    let err = assets::buy(&mut w, buyers[1], g, &pick).expect_err("past the day's import budget");
    assert!(err.contains("import budget"), "{err}");
    assert_eq!(w.comp::<Wallet>(buyers[1]).map(|x| x.coins), Some(coins), "the buyer keeps the coins");
    assert_eq!(w.comp::<Corp>(corp).map(|c| c.import_today), Some(one));
    // Review fix: a corp's business-fleet purchase is neither capped nor counted.
    let fleet_buyer = w.corps().into_iter().find(|&c| c != corp).expect("another corp");
    let parts = w.stock(g, Good::Parts);
    w.take_stock(g, Good::Parts, parts);
    assets::buy(&mut w, fleet_buyer, g, &pick).expect("a corp's fleet purchase past the households' cap");
    assert_eq!(w.comp::<Corp>(corp).map(|c| c.import_today), Some(one), "not counted");
    // The next midnight: the budget is back (a sentinel shows the roll; the
    // midnight's own fleet purchases may front a new day's import).
    w.comp_mut::<Corp>(corp).expect("corp").import_today = 1_000_000;
    w.run_ticks(TICKS_PER_DAY - w.tick_of_day() as u64);
    assert!(w.comp::<Corp>(corp).map_or(0, |c| c.import_today) < 1_000_000, "the roll reset it");
    w.comp_mut::<Corp>(corp).expect("corp").import_today = 0;
    let parts = w.stock(g, Good::Parts);
    w.take_stock(g, Good::Parts, parts);
    w.comp_mut::<Wallet>(buyers[1]).expect("wallet").coins = 5_000;
    assets::buy(&mut w, buyers[1], g, &pick).expect("a sale the next day");
}

/// E35 `emigrate_cost` (the jobs round, wages on): an agent with fewer
/// coins cannot start emigrating; 0 or wages off, everyone may.
#[test]
fn test_emigrate_cost_keeps_the_broke_home() {
    use citysim::systems::demography::can_buy_passage;
    let mut w = round_city(|c| c.economy2.emigrate_cost = 20);
    let a = w.citizens().into_iter().find(|&a| w.has::<citysim::Brain>(a) && w.has::<Wallet>(a)).expect("an agent");
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 19;
    assert!(!can_buy_passage(&w, a), "19 coins buy no passage");
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 20;
    assert!(can_buy_passage(&w, a));
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    w.config.economy2.emigrate_cost = 0;
    assert!(can_buy_passage(&w, a), "0: off");
    w.config.economy2.emigrate_cost = 20;
    w.config.economy2.wages = false;
    assert!(can_buy_passage(&w, a), "wages off: off");
}

/// The band's floor: a corp under `capital_lo_days × max(R, P)` posts
/// nothing under its margin, closes its open past-cap places at once and,
/// over `fire_above × P*` for `HIRE_DAYS` (the debounce), lays its newest
/// off, one a day (phase 2 waits seven days and the wage floor).
#[test]
fn test_lean_corp_debounced_layoff_and_posts_nothing() {
    let mut w = round_city(|c| {
        c.economy2.capital_hi_days = 7.0;
        c.economy2.capital_lo_days = 3.0;
        c.economy2.staff_ceiling_mult = 2.0;
    });
    let corp = corp_named(&w, "Nutrix");
    pin_food_demand(&mut w, corp);
    // Lean: the purse at 1,000 against 3 × max(R, P) = 3,000.
    let purse = w.purse(Some(corp));
    ownership::pay(&mut w, Some(corp), None, purse - 1_000, Flow::Subsidy);
    assert!(wages::lean(&w, corp, 100.0, 1_000.0));
    assert!(!wages::lean(&w, corp, 100.0, 100.0), "1,000 covers 3 days of 100");
    // Open places past a Market's cap close on the first lean day.
    let m = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Market)[0];
    let role = ownership::role_for(BuildingKind::Market).expect("a role");
    let full = corp_brain::full_staff(&w, BuildingKind::Market);
    let working = ownership::staff_at(&w, m).len();
    let open_m = |w: &World| w.vacancies.get(&m).map_or(0, |v| v.iter().filter(|&&r| r == role).count());
    let add = full + 3 - working - open_m(&w);
    w.vacancies.entry(m).or_default().extend(std::iter::repeat_n(role, add));
    let staff = |w: &World| ownership::employees_of(w, corp).len();
    let n0 = staff(&w);
    for day in 1..=2 {
        pin_windows(&mut w, corp, 100, 1000);
        wages::staff(&mut w);
        assert_eq!(staff(&w), n0, "day {day}: the debounce holds the layoff");
        assert!(working + open_m(&w) <= full, "day {day}: past-cap places closed");
    }
    pin_windows(&mut w, corp, 100, 1000);
    wages::staff(&mut w);
    assert_eq!(staff(&w), n0 - 1, "day 3: one laid off");
    pin_windows(&mut w, corp, 100, 1000);
    wages::staff(&mut w);
    assert_eq!(staff(&w), n0 - 2, "day 4: one more, one a day");
    assert!(w.comp::<Corp>(corp).map_or(0.0, |c| c.wage_rev) > w.config.economy2.wage_floor_mult);
    // Under the margin but lean: no posting.
    let open = |w: &World| -> usize {
        let c = w.comp::<Corp>(corp).expect("corp");
        c.buildings.iter().map(|b| w.vacancies.get(b).map_or(0, |v| v.len())).sum()
    };
    let before = open(&w);
    for _ in 0..5 {
        pin_windows(&mut w, corp, 3000, 1000);
        wages::staff(&mut w);
    }
    assert!(wages::lean(&w, corp, 3000.0, 1000.0));
    assert_eq!(open(&w), before, "a lean corp posts nothing");
    // Off (capital_lo_days 0): never lean.
    let w = round_city(|c| c.economy2.capital_hi_days = 7.0);
    assert!(!wages::lean(&w, corp_named(&w, "Nutrix"), 1e6, 1e6));
}

/// Jobless adults, ascending.
fn jobless(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<citysim::Brain>(a) && w.has::<Wallet>(a) && !w.has::<Job>(a))
        .filter(|&a| citysim::systems::demography::is_adult(w, a))
        .collect()
}

/// `shed_extra`: a corp over `fire_above × P*` sheds its past-cap hires
/// from the third day (`fire_days ≥ 3`), the newest at each building past
/// its cap, one per building a day, never its exec, and no `lay_off` of a
/// core hire the same day.
#[test]
fn test_shed_extra_sheds_past_cap_hires_first() {
    let mut w = round_city(|c| c.economy2.staff_ceiling_mult = 2.0);
    let corp = corp_named(&w, "Nutrix");
    let markets = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Market);
    let m = markets[0];
    let role = ownership::role_for(BuildingKind::Market).expect("a role");
    let full = corp_brain::full_staff(&w, BuildingKind::Market);
    let exec = w.comp::<Corp>(corp).and_then(|c| c.exec).expect("an exec");
    if let Some(v) = w.vacancies.get_mut(&m) {
        v.retain(|&r| r != role);
    }
    // Fill the Market to full staff plus three, the exec among them as the
    // newest hire (the rule must skip it).
    let have = ownership::staff_at(&w, m).len();
    for a in jobless(&w).into_iter().filter(|&a| a != exec).take(full + 3 - have) {
        citysim::systems::demography::hire(&mut w, a, m, role);
    }
    w.remove::<Job>(exec);
    citysim::systems::demography::hire(&mut w, exec, m, role);
    w.comp_mut::<Job>(exec).expect("job").hired_tick = 1_000_000;
    let count = |w: &World| ownership::staff_at(w, m).len();
    assert_eq!(count(&w), full + 4, "full + 3 hires + the exec");
    let n0 = ownership::employees_of(&w, corp).len();
    for day in 1..=2 {
        pin_windows(&mut w, corp, 100, 3000);
        wages::staff(&mut w);
        assert_eq!(ownership::employees_of(&w, corp).len(), n0, "day {day}: nothing shed before fire_days 3");
    }
    pin_windows(&mut w, corp, 100, 3000);
    wages::staff(&mut w);
    assert_eq!(count(&w), full + 3, "day 3: one past-cap hire shed at the Market");
    assert_eq!(ownership::employees_of(&w, corp).len(), n0 - 1, "one per building, no lay_off the same day");
    assert!(w.has::<Job>(exec), "never the exec");
    assert!(w.events.iter().any(|e| e.kind == EventKind::LaidOff && e.text.contains("past full staff")));
    for _ in 0..5 {
        pin_windows(&mut w, corp, 100, 3000);
        wages::staff(&mut w);
    }
    assert_eq!(count(&w), full + 1, "shed down to the cap (the exec not counted)");
    assert!(w.has::<Job>(exec));
}

/// Review fix (finding 1): the 7-day payroll lags the past-cap hires, so a
/// corp's posting room pays for every open past-cap place and every
/// past-cap hire of the last week; with P lagging (the windows unchanged
/// while the hires come in) the past-cap payroll never passes the room
/// the target left (+ one marginal wage).
#[test]
fn test_past_cap_posting_respects_room_with_lagging_payroll() {
    let mut w = round_city(|c| c.economy2.staff_ceiling_mult = 3.0);
    let corp = corp_named(&w, "Nutrix");
    pin_food_demand(&mut w, corp);
    let (rev, pay) = (1_000i64, 300i64);
    let cfg = w.config.economy2.clone();
    let room0 = cfg.hire_below * wages::labour_share(&w, corp) * rev as f32 - pay as f32;
    let rev_mult = w.comp::<Corp>(corp).map_or(1.0, |c| c.wage_mult * c.wage_rev);
    let buildings = w.comp::<Corp>(corp).expect("corp").buildings.clone();
    let exec = w.comp::<Corp>(corp).and_then(|c| c.exec);
    let mut max_wage = 0.0f32;
    for _ in 0..12 {
        pin_windows(&mut w, corp, rev, pay);
        wages::staff(&mut w);
        // Every open place filled at once (the hires P has not seen yet).
        let mut pool = jobless(&w).into_iter();
        for &b in &buildings {
            let roles: Vec<citysim::components::Role> = w.vacancies.get(&b).cloned().unwrap_or_default();
            w.vacancies.remove(&b);
            for r in roles {
                if let Some(a) = pool.next() {
                    citysim::systems::demography::hire(&mut w, a, b, r);
                }
            }
        }
    }
    let mut past_cap = 0.0f32;
    let overtime = 0usize;
    for &b in &buildings {
        let Some(kind) = w.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict).map(|bd| bd.kind) else {
            continue;
        };
        let Some(role) = ownership::role_for(kind) else { continue };
        let staff = ownership::staff_at(&w, b)
            .into_iter()
            .filter(|&a| Some(a) != exec && w.comp::<Job>(a).is_some_and(|j| j.role == role))
            .count();
        let cap = corp_brain::full_staff(&w, kind) + overtime;
        let wage = w.config.economy.wage(role) as f32 * rev_mult;
        max_wage = max_wage.max(wage);
        past_cap += staff.saturating_sub(cap) as f32 * wage;
    }
    assert!(past_cap > 0.0, "the room bought some past-cap shifts");
    assert!(past_cap <= room0 + max_wage, "past-cap payroll {past_cap} within the room {room0} (+ one wage)");
}

/// E35 with the identity: gated emigrants stay, the wallets of those who
/// leave cross out, and `coin_identity` holds to the coin every day either
/// way (20 days, no dole).
#[test]
fn test_emigrate_cost_keeps_the_identity() {
    let run = |cost: i64| -> (Vec<i64>, i64) {
        let mut w = round_city(|c| c.economy2.emigrate_cost = cost);
        w.levers.dole_per_day = 0;
        w.run_ticks(20 * TICKS_PER_DAY);
        let ids: Vec<i64> = w.stats.history.iter().map(|r| r.econ.coin_identity).collect();
        let emigrants: i64 = w.stats.history.iter().map(|r| i64::from(r.emigrants)).sum();
        (ids, emigrants)
    };
    let (free_ids, free) = run(0);
    let (gated_ids, gated) = run(20);
    assert!(free_ids.windows(2).all(|p| p[0] == p[1]), "free: the identity holds: {free_ids:?}");
    assert!(gated_ids.windows(2).all(|p| p[0] == p[1]), "gated: the identity holds: {gated_ids:?}");
    eprintln!("emigrants in 20 days: free {free}, gated {gated}");
    assert!(free > 0, "the no-dole city sheds people by day 20");
    assert!(gated < free, "passage keeps the broke home ({gated} vs {free})");
}

/// Export demand in a wages city: `[world_market] cap_wages` replaces a
/// good's daily cap with wages on (0 keeps the good's own); wages off reads
/// the plain cap.
#[test]
fn test_cap_wages_sets_the_worlds_cap_with_wages_on_only() {
    let mut w = round_city(|c| c.world_market.cap_wages = [0, 100, 30]);
    let app = |w: &World, g: ExportGood| wm::appetite(w, g);
    let expect = |cap: u32, a: f32| (cap as f32 * a).round() as u32;
    let food_cap = w.config.world_market.food.cap;
    assert_eq!(wm::cap_today(&w, ExportGood::Parts), expect(100, app(&w, ExportGood::Parts)));
    assert_eq!(wm::cap_today(&w, ExportGood::Data), expect(30, app(&w, ExportGood::Data)));
    assert_eq!(wm::cap_today(&w, ExportGood::Food), expect(food_cap, app(&w, ExportGood::Food)), "0: the good's own");
    w.config.economy2.wages = false;
    let parts_cap = w.config.world_market.parts.cap;
    assert_eq!(wm::cap_today(&w, ExportGood::Parts), expect(parts_cap, app(&w, ExportGood::Parts)), "wages off");
}

/// Review fix (item 7): a corp's business-fleet purchase is outside the
/// per-day import cap (never counted) but paid only from coins above
/// `fleet_floor_days × max(R, P)`: a purchase that would leave the buyer
/// under the floor is refused, one that leaves it at the floor buys.
#[test]
fn test_fleet_purchase_needs_coins_above_the_floor() {
    use citysim::systems::assets;
    use citysim::ShopPick;
    let mut w = round_city(|c| {
        c.economy2.fleet_floor_days = 7.0;
        c.economy2.asset_import_per_day = 1;
    });
    let g = w
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .copied()
        .find(|&b| w.corp_of_building(b).is_some())
        .expect("a corp Garage");
    let seller = w.corp_of_building(g).expect("its corp");
    let buyer = w.corps().into_iter().find(|&c| c != seller).expect("another corp");
    pin_windows(&mut w, buyer, 100, 100);
    let floor = wages::fleet_floor(&w, buyer);
    assert!(floor >= 700, "7 × max(R, P) with P at least the wage bill ({floor})");
    let pick = ShopPick { kind: AssetKind::Motorcycle, tier: 1, used: None, upgrade: false };
    let set_purse = |w: &mut World, coins: i64| {
        let have = w.purse(Some(buyer));
        if coins > have {
            ownership::charge(w, None, Some(buyer), coins - have, Flow::Subsidy);
        } else {
            ownership::pay(w, Some(buyer), None, have - coins, Flow::Subsidy);
        }
        assert_eq!(w.purse(Some(buyer)), coins);
    };
    // Rich: it buys (the price measured), the seller's import not counted
    // although the cap is 1.
    set_purse(&mut w, 100_000);
    let parts = w.stock(g, Good::Parts);
    w.take_stock(g, Good::Parts, parts);
    assets::buy(&mut w, buyer, g, &pick).expect("a fleet purchase over the floor");
    let price = 100_000 - w.purse(Some(buyer));
    assert!(price > 0);
    assert_eq!(w.comp::<Corp>(seller).map(|c| c.import_today), Some(0), "outside the cap: not counted");
    // One coin short of the floor after the purchase: refused, nothing paid.
    set_purse(&mut w, floor + price - 1);
    let parts = w.stock(g, Good::Parts);
    w.take_stock(g, Good::Parts, parts);
    let err = assets::buy(&mut w, buyer, g, &pick).expect_err("under the floor");
    assert!(err.contains("fleet floor"), "{err}");
    assert_eq!(w.purse(Some(buyer)), floor + price - 1);
    // At the floor after the purchase: it buys.
    set_purse(&mut w, floor + price);
    assets::buy(&mut w, buyer, g, &pick).expect("leaves exactly the floor");
    assert_eq!(w.purse(Some(buyer)), floor);
    assert_eq!(w.comp::<Corp>(seller).map(|c| c.import_today), Some(0));
}
