//! Hunger as sickness (docs/HUNGER_SICKNESS.md; plan docs/plans/hunger-sickness.md):
//! synth and real meals on the one `Good::Food`, the malnutrition and
//! toxicity meters, the debuffs and sick days, the slower starvation.

use citysim::systems::diet::{self, Quality};
use citysim::systems::{charity, demography, economy, health, law};
use citysim::utility::goals;
use citysim::{
    Brain, Building, BuildingKind, Config, EntityId, GoalKind, Health, Inventory, Job, Lod, Market, MealKind, Needs,
    Wallet, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn sick_cfg() -> Config {
    let mut c = Config::load();
    c.sickness.enabled = true;
    c
}

fn world_on() -> World {
    World::new(42, sick_cfg())
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| law::living(w, a) && demography::is_adult(w, a) && w.comp::<Brain>(a).is_some())
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<citysim::Sentence>(a))
        .collect()
}

fn market(w: &World) -> EntityId {
    *w.buildings_of_kind(BuildingKind::Market).first().expect("a Market")
}

/// Pin `m`'s price to a whole `p` (no tenths dither).
fn set_price(w: &mut World, m: EntityId, p: i64) {
    let mk = w.comp_mut::<Market>(m).expect("market");
    mk.price_food = p;
    mk.price_tenths = p * 10;
}

fn health(w: &World, id: EntityId) -> Health {
    w.comp::<Health>(id).cloned().unwrap_or_default()
}

/// With `[sickness]` off (the shipped default) nothing of it is kept: no
/// `Health`, no synth subset, no health columns (the CSV identity against
/// d771073 is the commit's A/B).
#[test]
fn test_sickness_off_leaves_no_trace() {
    let mut w = World::new(42, Config::load());
    assert!(!w.config.sickness.enabled, "the shipped default is off until the flip");
    w.run_ticks(5 * TICKS_PER_DAY);
    assert!(w.citizens().iter().all(|&a| !w.has::<Health>(a)), "a Health with sickness off");
    assert!(w.citizens().iter().all(|&a| w.comp::<Inventory>(a).is_none_or(|i| i.synth == 0)));
    assert!(w.stats.history.iter().all(|r| r.health.is_zero()));
}

/// § 1: at price 4 a synth meal is 1 coin; three synth meals take one shelf
/// unit, a fourth opens the next; the meals join the carried synth subset.
#[test]
fn test_synth_meal_costs_a_third_and_drains_a_third() {
    let mut w = world_on();
    let m = market(&w);
    set_price(&mut w, m, 4);
    w.comp_mut::<Building>(m).expect("market").stock_food = 10;
    let a = adults(&w)[0];
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 3;
    *w.comp_mut::<Inventory>(a).expect("inv") = Inventory::default();
    assert_eq!(diet::synth_price(&w, 4), 1);
    assert_eq!(diet::quality(&w, a, 4), Quality::Synth);
    let q = economy::quality_at(&w, a, Some(m));
    let paid = economy::pay_for_food_q(&mut w, a, Some(m), 3, q);
    assert_eq!(paid, 3, "three synth meals at 1");
    assert!(economy::take_food_q(&mut w, a, Some(m), 3, paid, q));
    assert_eq!(w.comp::<Building>(m).expect("market").stock_food, 9, "one unit for three meals");
    let inv = w.comp::<Inventory>(a).expect("inv");
    assert_eq!((inv.food, inv.synth_held()), (3, 3));
    // A fourth meal opens a unit; the next two come out of it.
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 1;
    let paid = economy::pay_for_food_q(&mut w, a, Some(m), 1, Quality::Synth);
    assert!(economy::take_food_q(&mut w, a, Some(m), 1, paid, Quality::Synth));
    assert_eq!(w.comp::<Building>(m).expect("market").stock_food, 8);
    assert_eq!(w.comp::<Market>(m).expect("market").synth_frac, 1);
    assert_eq!(diet::meals_on_shelf(&w, m, 8, Quality::Synth), 8 * 3 + 2);
}

/// § 1: coins for `real_meal_reserve` real meals buy real (and so does a
/// malnourished agent with two meals' coins).
#[test]
fn test_real_bought_above_the_floor() {
    let mut w = world_on();
    let a = adults(&w)[0];
    let reserve = w.config.sickness.real_meal_reserve;
    w.comp_mut::<Wallet>(a).expect("wallet").coins = reserve * 4;
    assert_eq!(diet::quality(&w, a, 4), Quality::Real);
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 8;
    assert_eq!(diet::quality(&w, a, 4), Quality::Synth);
    w.insert(a, Health { malnutrition: 0.6, ..Health::default() });
    assert_eq!(diet::quality(&w, a, 4), Quality::Real, "malnourished with two meals' coins");
    // The diet off: always real, at the Market's price.
    let mut off = World::new(42, Config::load());
    let b = adults(&off)[0];
    off.comp_mut::<Wallet>(b).expect("wallet").coins = 1;
    assert_eq!(diet::quality(&off, b, 4), Quality::Real);
}

/// § 1 (H5): a Mission serves `synth_stretch` synth meals out of one unit.
#[test]
fn test_mission_serves_three_synth_meals_per_unit() {
    let mut w = world_on();
    let m = *w.buildings_of_kind(BuildingKind::Mission).first().expect("a Mission");
    w.comp_mut::<Building>(m).expect("mission").stock_food = 1;
    let eaters: Vec<EntityId> = adults(&w).into_iter().take(4).collect();
    let mut served = 0;
    for &e in &eaters {
        if matches!(charity::on_eat_alms(&mut w, e, Some(m)), citysim::StepResult::Done) {
            served += 1;
            assert_eq!(health(&w, e).today.alms, 1, "the meal is tallied as alms");
        }
    }
    assert_eq!(served, 3, "one unit, three meals");
    assert_eq!(w.comp::<Building>(m).expect("mission").stock_food, 0);
}

/// § 2: the daily update's arithmetic from the tally.
#[test]
fn test_meters_daily_from_the_tally() {
    let mut w = world_on();
    let cfg = w.config.sickness.clone();
    let a = adults(&w)[0];
    let b = adults(&w)[1];
    let c = adults(&w)[2];
    let tally = |real, synth, dive, hungry| citysim::MealTally { real, synth, dive, alms: 0, hungry_hours: hungry };
    w.insert(a, Health { today: tally(0, 3, 0, 0), ..Health::default() });
    w.insert(b, Health { today: tally(0, 0, 0, 24), ..Health::default() });
    w.insert(c, Health { malnutrition: 0.5, toxicity: 0.5, today: tally(3, 0, 1, 0), ..Health::default() });
    health::daily(&mut w);
    let (ha, hb, hc) = (health(&w, a), health(&w, b), health(&w, c));
    assert!((ha.malnutrition - cfg.mal_poor_diet).abs() < 1e-6, "an all-synth day: {}", ha.malnutrition);
    assert!((ha.toxicity - (3.0 * cfg.tox_synth - cfg.tox_clear)).abs() < 1e-6);
    assert!((hb.malnutrition - cfg.mal_hungry).abs() < 1e-6, "a hungry day: {}", hb.malnutrition);
    assert_eq!(hb.toxicity, 0.0, "toxicity clears to 0, never below");
    let want_m = 0.5 + cfg.mal_poor_diet * 0.25 - 2.0 * cfg.mal_real;
    assert!((hc.malnutrition - want_m).abs() < 1e-6, "real meals heal: {} vs {want_m}", hc.malnutrition);
    assert!((hc.toxicity - (0.5 + cfg.tox_dive - cfg.tox_clear)).abs() < 1e-6);
    assert_eq!(hc.today, citysim::MealTally::default(), "the tally resets");
}

/// § 2 (coordinator call on P2): a synth diet levels off at
/// `synth_malnut_cap`; hunger and bin scraps push past it.
#[test]
fn test_synth_diet_levels_off_at_the_cap() {
    let mut w = world_on();
    let cap = w.config.sickness.synth_malnut_cap;
    let (a, b, c) = (adults(&w)[0], adults(&w)[1], adults(&w)[2]);
    let tally = |synth, dive, hungry| citysim::MealTally { real: 0, synth, dive, alms: 0, hungry_hours: hungry };
    for _ in 0..200 {
        for (id, t) in [(a, tally(3, 0, 0)), (b, tally(0, 3, 0)), (c, tally(3, 0, 12))] {
            let mut h = health(&w, id);
            h.today = t;
            w.insert(id, h);
        }
        health::daily(&mut w);
    }
    let (ma, mb, mc) = (health(&w, a).malnutrition, health(&w, b).malnutrition, health(&w, c).malnutrition);
    eprintln!("200 days: synth {ma:.3}, scraps {mb:.3}, synth and hungry {mc:.3}");
    assert!((ma - cap).abs() < 1e-5, "an all-synth diet sits at the cap: {ma}");
    assert!(mb > cap + 0.4, "scraps push past the cap: {mb}");
    assert!(mc > cap + 0.4, "hunger pushes past the cap: {mc}");
    // Above the cap, a synth diet does not add (nor heal).
    let mut h = health(&w, a);
    h.malnutrition = 0.8;
    h.today = tally(3, 0, 0);
    w.insert(a, h);
    health::daily(&mut w);
    assert!((health(&w, a).malnutrition - 0.8).abs() < 1e-6);
}

/// § 2: the debuffs read through `health::effects`.
#[test]
fn test_effects_reach_energy_work_mood_fertility() {
    let mut w = world_on();
    let a = adults(&w)[0];
    assert_eq!(health::effects(&w, a), health::Effects::NONE);
    w.insert(a, Health { malnutrition: 1.0, toxicity: 1.0, ..Health::default() });
    let e = health::effects(&w, a);
    let cfg = &w.config.sickness;
    assert!((e.energy_mult - (1.0 + cfg.mal_energy + cfg.tox_energy)).abs() < 1e-6);
    assert!((e.work_mult - (1.0 - cfg.mal_work)).abs() < 1e-6);
    assert!(e.mood_bias < -0.3);
    assert!(!e.fertile, "no conception past fertile_mal_max");
    assert!(e.old_mult > 1.0 && e.frail_fight > 0.0);
    assert_eq!(health::energy_mult(&w, a, None), Some(e.energy_mult));
    assert_eq!(health::energy_mult(&w, a, Some(2.0)), Some(2.0 * e.energy_mult));
}

/// § 2: a sick day takes the shift on both tiers: the Full Work goal is
/// gated, and a Statistical farmer on shift adds no work.
#[test]
fn test_sick_day_skips_the_shift_on_both_tiers() {
    let mut w = world_on();
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let farmer = adults(&w)
        .into_iter()
        .find(|&a| w.comp::<Job>(a).is_some_and(|j| j.role == citysim::Role::Farmer && j.on_shift(w.tick_of_day())))
        .expect("a farmer on shift");
    w.insert(farmer, Health { sick_today: true, ..Health::default() });
    assert!(health::sick(&w, farmer));
    let (cs, _) = goals::considerations(&w, farmer, GoalKind::Work, false).expect("work considerations");
    assert!(cs.iter().any(|c| c.output <= 0.0), "a sick farmer's Work is gated");
    // Statistical: the farm's accrued work over an hour, sick vs well.
    let farm = w.comp::<Job>(farmer).and_then(|j| j.employer).expect("farm");
    let mut accrued = Vec::new();
    for sick in [false, true] {
        let mut v = w.clone();
        citysim::systems::lod::set_lod(&mut v, farmer, Lod::Statistical);
        v.comp_mut::<Health>(farmer).expect("health").sick_today = sick;
        // Every other farmer off sick too, so only this one can move the farm.
        let others: Vec<EntityId> = v
            .citizens()
            .into_iter()
            .filter(|&o| o != farmer && v.comp::<Job>(o).and_then(|j| j.employer) == Some(farm))
            .collect();
        for o in others {
            v.insert(o, Health { sick_today: true, ..Health::default() });
            citysim::systems::lod::set_lod(&mut v, o, Lod::Statistical);
        }
        let before = v.comp::<Building>(farm).map_or(0.0, |b| b.production_accum + b.stock_food as f32);
        v.run_ticks(TICKS_PER_HOUR);
        let after = v.comp::<Building>(farm).map_or(0.0, |b| b.production_accum + b.stock_food as f32);
        accrued.push(after - before);
    }
    eprintln!("farm output in an hour: well {:.3}, sick {:.3}", accrued[0], accrued[1]);
    assert!(accrued[0] > 0.0, "a well Statistical farmer works the hour");
    assert!(accrued[1] <= 0.0, "a sick one does not");
}

/// Decision 4: five sick days in fourteen cost the job.
#[test]
fn test_five_sick_days_in_fourteen_cost_the_job() {
    let mut cfg = sick_cfg();
    cfg.sickness.sick_mal = 100.0;
    let mut w = World::new(42, cfg);
    let a = adults(&w).into_iter().find(|&a| w.has::<Job>(a)).expect("an employed adult");
    w.insert(a, Health { malnutrition: 1.0, sick_days: 0b1111, ..Health::default() });
    health::daily(&mut w);
    assert!(health(&w, a).sick_today, "malnutrition 1.0 at sick_mal 100 is a sick day");
    assert!(!w.has::<Job>(a), "the fifth sick day in fourteen is the sack");
}

/// § 2: the Full and Statistical tiers tally a meal the same way.
#[test]
fn test_statistical_tally_matches_full() {
    let mut w = world_on();
    let a = adults(&w)[0];
    // Full: eat a carried synth meal.
    {
        let i = w.comp_mut::<Inventory>(a).expect("inv");
        i.food = 1;
        i.synth = 1;
        i.stolen_food = 0;
    }
    let t = w.tick;
    citysim::exec::actions::on_complete(&mut w, a, citysim::ActionKind::EatFromInventory, None, t, t);
    assert_eq!(health(&w, a).today.synth, 1);
    // Statistical: the same agent off screen, hungry, holding one synth meal.
    let mut v = World::new(42, sick_cfg());
    let b = adults(&v)[0];
    citysim::systems::lod::set_lod(&mut v, b, Lod::Statistical);
    {
        let i = v.comp_mut::<Inventory>(b).expect("inv");
        i.food = 1;
        i.synth = 1;
        i.stolen_food = 0;
    }
    v.comp_mut::<Needs>(b).expect("needs").hunger = 0.2;
    v.run_ticks(TICKS_PER_HOUR);
    assert_eq!(health(&v, b).today.synth, 1, "the off-screen meal is tallied as synth");
    assert_eq!(v.comp::<Inventory>(b).expect("inv").food, 0);
}

/// § 1: a store eats real first; synth keeps through spoilage.
#[test]
fn test_real_first_and_synth_keeps() {
    let mut inv = Inventory { food: 3, synth: 2, ..Inventory::default() };
    assert_eq!(health::eat_inventory_unit(&mut inv), MealKind::Real);
    assert_eq!(health::eat_inventory_unit(&mut inv), MealKind::Synth);
    assert_eq!((inv.food, inv.synth_held()), (1, 1));
    // A stale synth count is clamped, never trusted.
    let inv = Inventory { food: 1, synth: 5, ..Inventory::default() };
    assert_eq!(inv.synth_held(), 1);
}

/// § 7: with sickness on, starvation kills after `starve_grace_days` at
/// hunger 0, not three days.
#[test]
fn test_starvation_takes_seven_days_with_sickness() {
    let mut w = world_on();
    let a = adults(&w)[0];
    let tick = w.tick + 10 * TICKS_PER_DAY;
    w.tick = tick;
    {
        let n = w.comp_mut::<Needs>(a).expect("needs");
        n.hunger = 0.0;
        n.starving_since = Some(tick - 4 * TICKS_PER_DAY);
    }
    citysim::needs::starvation(&mut w, a);
    assert!(law::living(&w, a), "alive four days in (the old grace was three)");
    w.comp_mut::<Needs>(a).expect("needs").starving_since = Some(tick - 7 * TICKS_PER_DAY);
    citysim::needs::starvation(&mut w, a);
    assert!(!law::living(&w, a), "dead after seven days at hunger 0");
}

/// § 10: a mid-run save with the meters carries them.
#[test]
fn test_save_round_trip_with_health() {
    let mut w = world_on();
    w.run_ticks(2 * TICKS_PER_DAY + 3 * TICKS_PER_HOUR);
    let a = adults(&w)[0];
    w.insert(a, Health { malnutrition: 0.4, toxicity: 0.3, sick_days: 0b101, ..Health::default() });
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(back.comp::<Health>(a), w.comp::<Health>(a));
    w.run_ticks(TICKS_PER_DAY);
    back.run_ticks(TICKS_PER_DAY);
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w), "the loaded world runs on byte for byte");
}
