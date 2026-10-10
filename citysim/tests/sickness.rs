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

// ---------------------------------------------------------------------------
// P3: the bins (§ 4)
// ---------------------------------------------------------------------------

fn bin_of(w: &World, b: EntityId) -> u16 {
    w.comp::<Building>(b).map_or(0, |x| x.bin)
}

/// A Market's spoiled units fill its bin at `bin_meals_per_unit`, capped,
/// then the bin rots by `bin_rot`; the midnight pass does both.
#[test]
fn test_market_bin_fills_from_spoilage_and_rots() {
    let mut w = world_on();
    let m = market(&w);
    citysim::systems::bins::spoiled(&mut w, m, 10);
    assert_eq!(bin_of(&w, m), 20, "10 units, 2 meals a unit");
    assert!(w.bins.contains(&m));
    citysim::systems::bins::spoiled(&mut w, m, 100);
    assert_eq!(bin_of(&w, m), w.config.sickness.bin_cap, "capped");
    citysim::systems::bins::rot(&mut w);
    assert_eq!(bin_of(&w, m), 20, "half rots");
    // The midnight pass: a stocked shelf spoils into the bin.
    let mut v = world_on();
    let m = market(&v);
    v.comp_mut::<Building>(m).expect("market").stock_food = 1500;
    v.run_ticks(TICKS_PER_DAY - v.tick_of_day() as u64);
    assert!(bin_of(&v, m) > 0, "the shelf's spoilage reached the bin");
}

/// A Home's pantry spoilage fills its own bin (household waste).
#[test]
fn test_home_bin_from_pantry_spoilage() {
    let mut w = world_on();
    let h = *w.buildings_of_kind(BuildingKind::Home).first().expect("a Home");
    w.comp_mut::<Building>(h).expect("home").stock_food = 40;
    w.run_ticks(TICKS_PER_DAY - w.tick_of_day() as u64);
    assert!(bin_of(&w, h) > 0, "the pantry's spoiled units reached the bin");
}

/// A broke, hungry adult with no Mission in reach and a stocked bin 20
/// tiles off dives and eats within six hours (a person probe).
#[test]
fn probe_broke_with_no_mission_dives() {
    let mut w = world_on();
    w.run_ticks(2 * TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    for m in w.buildings_of_kind(BuildingKind::Mission).to_vec() {
        w.comp_mut::<Building>(m).expect("mission").stock_food = 0;
    }
    let m = market(&w);
    w.comp_mut::<Building>(m).expect("market").bin = 30;
    w.bins.insert(m);
    let tile = citysim::systems::bins::bin_tile(&w, m).expect("bin tile");
    let picks: Vec<EntityId> = adults(&w)
        .into_iter()
        .filter(|&a| w.comp::<Job>(a).is_none())
        .filter(|&a| w.comp::<citysim::Position>(a).is_some_and(|p| (8..=25).contains(&p.tile.manhattan(tile))))
        .take(6)
        .collect();
    assert!(!picks.is_empty(), "an adult 8-25 tiles from the bin");
    let mut dove = 0;
    for &a in &picks {
        let mut v = w.clone();
        v.comp_mut::<Brain>(a).expect("brain").pinned = true;
        v.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
        *v.comp_mut::<Inventory>(a).expect("inv") = Inventory::default();
        if let Some(h) = v.comp::<citysim::Household>(a).and_then(|h| h.home) {
            v.comp_mut::<Building>(h).expect("home").stock_food = 0;
        }
        v.comp_mut::<Needs>(a).expect("needs").hunger = 0.15;
        if let Some(p) = v.comp_mut::<citysim::Personality>(a) {
            p.pride = 0.2;
        }
        let end = v.tick + 6 * TICKS_PER_HOUR;
        while v.tick < end {
            v.comp_mut::<Brain>(a).expect("brain").pinned = true;
            v.run_ticks(1);
        }
        let d = health(&v, a).today.dive;
        eprintln!("agent {}: dove {d}", a.index);
        dove += usize::from(d > 0);
    }
    assert!(dove * 2 >= picks.len(), "only {dove} of {} broke hungry adults dove", picks.len());
}

/// An established regular present at the bin puts an Intimidate to a
/// newcomer: over many pairs both outcomes happen, and a loss is a
/// `BinContest` that drifts the pair apart.
#[test]
fn test_regular_wins_or_loses_by_intimidate() {
    use citysim::systems::bins::{contest, Contest};
    let w = world_on();
    let m = market(&w);
    let day = w.day();
    let people = adults(&w);
    let (mut lost, mut clear) = (0, 0);
    for pair in people.chunks(2).take(40) {
        let [reg, new] = pair else { continue };
        let mut v = w.clone();
        citysim::systems::lod::set_lod(&mut v, *reg, Lod::Statistical);
        citysim::systems::lod::set_lod(&mut v, *new, Lod::Statistical);
        v.comp_mut::<Building>(m).expect("market").bin_regular =
            Some(citysim::BinRegular { who: *reg, days: 0b111, last_day: day });
        match contest(&mut v, *new, m) {
            Contest::LostToRegular(r) => {
                assert_eq!(r, *reg);
                assert!(v.edge(*new, *reg).is_some_and(|e| e.affinity < 0.0), "the loser resents the regular");
                assert_eq!(v.stats.current.health.bin_contests, 1);
                lost += 1;
            }
            Contest::Clear => clear += 1,
            Contest::Shaken => {}
        }
    }
    eprintln!("newcomers seen off {lost}, ate {clear}");
    assert!(lost > 0 && clear > 0, "both outcomes: lost {lost}, clear {clear}");
}

/// A gang's turf (a held Home within `bin_turf_tiles`): a non-member pays
/// the toll (once a day), one with nothing to pay goes without (off
/// screen), a member dives free.
#[test]
fn test_gang_turf_shakes_down_a_non_member() {
    use citysim::systems::bins::{contest, turf, Contest};
    let mut cfg = sick_cfg();
    cfg.sickness.bin_shake_p = 1.0;
    let mut w = World::new(42, cfg);
    w.run_ticks(5 * TICKS_PER_DAY);
    let g = w
        .gangs()
        .into_iter()
        .find(|&g| w.comp::<citysim::Gang>(g).is_some_and(|x| !x.members.is_empty()))
        .expect("a gang with members");
    // A Home held by the gang; its own bin is on its turf.
    let h = *w.buildings_of_kind(BuildingKind::Home).first().expect("a Home");
    w.comp_mut::<Building>(h).expect("home").claim = Some(citysim::Claim { gang: g, count: 3 });
    assert_eq!(turf(&w, h), Some(g));
    let diver = adults(&w).into_iter().find(|&a| w.gang_of(a).is_none()).expect("a civilian");
    citysim::systems::lod::set_lod(&mut w, diver, Lod::Statistical);
    w.insert(diver, Health::default());
    w.comp_mut::<Wallet>(diver).expect("wallet").coins = 5;
    let gang_before = w.comp::<citysim::Gang>(g).expect("gang").treasury;
    assert_eq!(contest(&mut w, diver, h), Contest::Clear, "the toll paid, the meal had");
    assert_eq!(w.comp::<Wallet>(diver).expect("wallet").coins, 5 - w.config.sickness.bin_toll);
    assert!(w.comp::<citysim::Gang>(g).expect("gang").treasury > gang_before, "the toll reached the gang");
    assert_eq!(w.stats.current.health.bin_shakedowns, 1);
    assert_eq!(contest(&mut w, diver, h), Contest::Clear, "once a day");
    assert_eq!(w.stats.current.health.bin_shakedowns, 1);
    // Broke: run off (off screen, no body to fight).
    let broke = adults(&w).into_iter().filter(|&a| w.gang_of(a).is_none() && a != diver).nth(1).expect("another");
    citysim::systems::lod::set_lod(&mut w, broke, Lod::Statistical);
    w.comp_mut::<Wallet>(broke).expect("wallet").coins = 0;
    assert_eq!(contest(&mut w, broke, h), Contest::Shaken);
    assert!(!citysim::systems::bins::stat_dive(&mut w, broke, 1.0), "stat_dive goes through the same contest");
    // A member dives free.
    let member = w.comp::<citysim::Gang>(g).expect("gang").members[0];
    assert_eq!(contest(&mut w, member, h), Contest::Clear);
}

/// One diver at a time: a bin held by another diver is neither a target
/// nor startable.
#[test]
fn test_one_diver_per_bin() {
    let mut w = world_on();
    let m = market(&w);
    w.comp_mut::<Building>(m).expect("market").bin = 10;
    w.bins.insert(m);
    let (a, b) = (adults(&w)[0], adults(&w)[1]);
    let t = w.tick;
    citysim::systems::bins::on_start(&mut w, a, Some(m), t + 20);
    let tile = citysim::systems::bins::bin_tile(&w, m).expect("tile");
    if let Some(p) = w.comp_mut::<citysim::Position>(b) {
        p.tile = tile;
        p.building = None;
    }
    assert!(!citysim::systems::bins::can_start(&w, b, Some(m)), "held by a");
    citysim::systems::bins::release(&mut w, a, Some(m));
    assert!(citysim::systems::bins::can_start(&w, b, Some(m)));
}

// ---------------------------------------------------------------------------
// P4: cancer and the Clinic (§ 3)
// ---------------------------------------------------------------------------

fn cancer(w: &World, id: EntityId) -> Option<citysim::Cancer> {
    w.comp::<Health>(id).and_then(|h| h.cancer)
}

fn clinic(w: &World) -> EntityId {
    *w.buildings_of_kind(BuildingKind::Clinic).first().expect("a Clinic")
}

fn diagnosed(stage: f32) -> Health {
    Health { cancer: Some(citysim::Cancer { stage, since: 0, diagnosed: true }), ..Health::default() }
}

/// The onset rolls only at toxicity >= `cancer_tox_min`, near `cancer_p` a
/// day at toxicity 1.0.
#[test]
fn test_cancer_rolls_only_above_tox_min() {
    let mut w = world_on();
    let people = adults(&w);
    let (hot, cool): (Vec<EntityId>, Vec<EntityId>) = people.iter().take(400).partition(|a| a.index % 2 == 0);
    let days = 30;
    for _ in 0..days {
        for &a in &hot {
            let mut h = health(&w, a);
            if h.cancer.is_none() {
                h.toxicity = 1.0;
                w.insert(a, h);
            }
        }
        for &a in &cool {
            let mut h = health(&w, a);
            h.toxicity = 0.55;
            w.insert(a, h);
        }
        health::daily(&mut w);
    }
    let n_hot = hot.iter().filter(|&&a| cancer(&w, a).is_some()).count();
    let n_cool = cool.iter().filter(|&&a| cancer(&w, a).is_some()).count();
    let want = w.config.sickness.cancer_p as f64 * hot.len() as f64 * days as f64;
    eprintln!("cancers at toxicity 1.0: {n_hot} (expected ~{want:.1}); at 0.55: {n_cool}");
    assert_eq!(n_cool, 0, "no onset under cancer_tox_min");
    assert!(n_hot > 0 && (n_hot as f64) < want * 3.0 + 3.0, "{n_hot} vs ~{want:.1}");
}

/// The stage grows `cancer_growth` a day, turns symptomatic (diagnosed: the
/// event and the memory) at `cancer_symptom`, and kills at 1.0 as Disease.
#[test]
fn test_cancer_course_diagnosis_and_death() {
    let mut w = world_on();
    let (a, b) = (adults(&w)[0], adults(&w)[1]);
    let symptom = w.config.sickness.cancer_symptom;
    let growth = w.config.sickness.cancer_growth;
    w.insert(
        a,
        Health {
            cancer: Some(citysim::Cancer { stage: symptom - growth / 2.0, since: 0, diagnosed: false }),
            ..Health::default()
        },
    );
    w.insert(b, diagnosed(1.0 - growth / 2.0));
    let before = w.next_event_id;
    health::daily(&mut w);
    assert!(cancer(&w, a).is_some_and(|c| c.diagnosed), "diagnosed at the symptom stage");
    assert!(w.events.iter().any(|e| e.id >= before && e.kind == citysim::EventKind::Diagnosed && e.actors[0] == a));
    assert!(w
        .comp::<citysim::Memory>(a)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == citysim::MemoryKind::Diagnosed)));
    assert!(!law::living(&w, b), "dead at stage 1");
    assert_eq!(w.comp::<citysim::Corpse>(b).map(|c| c.cause), Some(citysim::DeathCause::Disease));
    assert_eq!(w.stats.current.health.deaths_disease, 1);
}

/// Treatment below `cure_stage` cures (toxicity halved); above it buys a
/// remission; the price leaves the wallet for the Clinic's owner and no
/// coin is made or lost.
#[test]
fn test_treatment_cures_below_cure_stage_and_remits_above() {
    let mut w = world_on();
    let c = clinic(&w);
    let price = w.config.sickness.cancer_treat_price;
    let (a, b) = (adults(&w)[0], adults(&w)[1]);
    for &x in &[a, b] {
        w.comp_mut::<Wallet>(x).expect("wallet").coins = price + 10;
    }
    w.insert(a, Health { toxicity: 0.8, ..diagnosed(0.3) });
    w.insert(b, diagnosed(0.8));
    let id0 = citysim::systems::econ::identity(&w);
    assert!(health::oncology(&mut w, a, c), "treated");
    assert!(cancer(&w, a).is_none(), "cured below cure_stage");
    assert!((health(&w, a).toxicity - 0.4).abs() < 1e-6, "toxicity halved");
    assert_eq!(w.comp::<Wallet>(a).expect("wallet").coins, 10);
    assert!(health::oncology(&mut w, b, c));
    let left = cancer(&w, b).expect("still ill").stage;
    assert!((left - (0.8 - w.config.sickness.remission)).abs() < 1e-6, "a remission: {left}");
    assert_eq!(citysim::systems::econ::identity(&w), id0, "the coin identity holds");
    assert_eq!(w.stats.current.health.cancer_cured, 1);
}

/// Decision 3: the wallet first, then the spouse and kin, then the gang.
#[test]
fn test_pool_pays_wallet_then_kin_then_gang() {
    let mut w = world_on();
    w.run_ticks(5 * TICKS_PER_DAY);
    let price = w.config.sickness.cancer_treat_price;
    let a = adults(&w).into_iter().find(|&a| w.spouse_of(a).is_some()).expect("a married adult");
    let s = w.spouse_of(a).expect("spouse");
    w.comp_mut::<Wallet>(a).expect("wallet").coins = price / 2;
    w.comp_mut::<Wallet>(s).expect("wallet").coins = price;
    w.insert(a, diagnosed(0.3));
    assert!(!health::wants_oncology(&w, s));
    assert!(health::pool(&w, a) >= price, "the spouse's coins make up the price");
    let c = clinic(&w);
    assert!(health::oncology(&mut w, a, c));
    assert_eq!(w.comp::<Wallet>(a).expect("wallet").coins, 0, "the patient pays first");
    assert_eq!(w.comp::<Wallet>(s).expect("wallet").coins, price / 2, "the spouse the rest");
    // A gang member with an empty wallet: the gang pays.
    let g = w
        .gangs()
        .into_iter()
        .find(|&g| w.comp::<citysim::Gang>(g).is_some_and(|x| !x.members.is_empty()))
        .expect("a gang with members");
    let m = w.comp::<citysim::Gang>(g).expect("gang").members[0];
    w.comp_mut::<Wallet>(m).expect("wallet").coins = 0;
    w.comp_mut::<citysim::Gang>(g).expect("gang").treasury = price * 3;
    w.insert(m, diagnosed(0.3));
    assert!(health::payers(&w, m).last().is_some_and(|&(p, _)| p == g), "the gang last");
}

/// A Full patient, diagnosed with the coins, goes to a Clinic and is
/// treated within two days (a person probe).
#[test]
fn probe_diagnosed_patient_with_coins_seeks_the_clinic() {
    let mut w = world_on();
    w.run_ticks(2 * TICKS_PER_DAY + 9 * TICKS_PER_HOUR);
    let price = w.config.sickness.cancer_treat_price;
    let picks: Vec<EntityId> = adults(&w).into_iter().take(5).collect();
    let mut treated = 0;
    for &a in &picks {
        let mut v = w.clone();
        v.comp_mut::<Wallet>(a).expect("wallet").coins = price + 60;
        v.insert(a, diagnosed(0.3));
        let end = v.tick + 2 * TICKS_PER_DAY;
        while v.tick < end {
            v.comp_mut::<Brain>(a).expect("brain").pinned = true;
            v.run_ticks(1);
            if cancer(&v, a).is_none() {
                break;
            }
        }
        eprintln!("patient {}: cancer {:?}", a.index, cancer(&v, a));
        treated += usize::from(cancer(&v, a).is_none());
    }
    assert!(treated * 5 >= picks.len() * 4, "only {treated} of {} treated in two days", picks.len());
}

/// Off screen: a diagnosed patient whose pool pays treats on `p_stat_treat`.
#[test]
fn test_stat_patient_treats_on_p_stat_treat() {
    let mut cfg = sick_cfg();
    cfg.sickness.p_stat_treat = 1.0;
    let mut w = World::new(42, cfg);
    let a = adults(&w)[0];
    citysim::systems::lod::set_lod(&mut w, a, Lod::Statistical);
    w.comp_mut::<Wallet>(a).expect("wallet").coins = w.config.sickness.cancer_treat_price;
    w.insert(a, diagnosed(0.3));
    health::daily(&mut w);
    assert!(cancer(&w, a).is_none(), "treated off screen");
}

/// A diagnosed patient whose pool falls short saves for the treatment:
/// Earn is not satisfied at a few days' meals.
#[test]
fn test_saving_for_treatment_raises_earn() {
    let mut w = world_on();
    let a = adults(&w).into_iter().find(|&a| w.comp::<Job>(a).is_none()).expect("a jobless adult");
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 60;
    assert!(goals::already_satisfied(&w, a, GoalKind::Earn, false), "60 coins is savings enough");
    w.insert(a, diagnosed(0.3));
    assert!(!goals::already_satisfied(&w, a, GoalKind::Earn, false), "but not for a 240-coin treatment");
}

/// Decision 6: a child carries malnutrition, never a cancer.
#[test]
fn test_children_roll_no_cancer() {
    let mut w = world_on();
    w.run_ticks(TICKS_PER_DAY);
    let Some(k) = w.citizens().into_iter().find(|&c| w.has::<citysim::Child>(c)) else {
        eprintln!("no child at day 1 on seed 42: nothing to check");
        return;
    };
    for _ in 0..200 {
        w.insert(k, Health { toxicity: 1.0, ..health(&w, k) });
        health::daily(&mut w);
    }
    assert!(cancer(&w, k).is_none());
}

/// P5 (a): the bins fill from what is sold and eaten too: a Market's sale
/// leaves `market_waste` a unit, a pantry meal `home_scrap`, and a small
/// pantry's spoilage carries its fraction until a unit spoils into the bin.
#[test]
fn test_bins_fill_from_sales_meals_and_carried_spoilage() {
    let mut w = world_on();
    let m = market(&w);
    citysim::systems::bins::market_sale(&mut w, m, 10);
    assert_eq!(bin_of(&w, m), (10.0 * w.config.sickness.market_waste).floor() as u16);
    let h = *w.buildings_of_kind(BuildingKind::Home).first().expect("a Home");
    for _ in 0..5 {
        citysim::systems::bins::home_meal(&mut w, h);
    }
    assert_eq!(bin_of(&w, h), (5.0 * w.config.sickness.home_scrap + 1e-4).floor() as u16);
    // A 4-unit pantry (5 % a day, 7.5 % in the Sump) spoils a unit within
    // a fortnight with the carry, never without it.
    let mut v = world_on();
    let h = *v.buildings_of_kind(BuildingKind::Home).first().expect("a Home");
    let mut spoiled = false;
    for _ in 0..14 {
        v.comp_mut::<Building>(h).expect("home").stock_food = 4;
        v.run_ticks(TICKS_PER_DAY);
        spoiled |= v.comp::<Building>(h).is_some_and(|b| b.stock_food < 4 || b.bin > 0);
    }
    assert!(spoiled, "the carried fraction spoiled a unit");
}

// ---------------------------------------------------------------------------
// P4b: the rungs grow (Dylan on J25)
// ---------------------------------------------------------------------------

fn rungs_cfg() -> Config {
    let mut c = sick_cfg();
    c.street.shanties = true;
    c
}

/// Put `n` adults out on the street (no home), Statistical.
fn evict_n(w: &mut World, n: usize) -> Vec<EntityId> {
    let out: Vec<EntityId> = adults(w).into_iter().take(n).collect();
    for &a in &out {
        w.comp_mut::<citysim::Household>(a).expect("household").home = None;
    }
    out
}

/// Rough sleepers outnumbering the free squat slots raise shanties on vacant
/// Lots (at most `shanties_per_night` a night, `shanty_lots_max` standing);
/// the night's assignment settles the off-screen homeless there.
#[test]
fn test_shanties_rise_with_rough_sleepers_and_are_capped() {
    use citysim::systems::street;
    let mut cfg = rungs_cfg();
    cfg.street.shanty_lots_max = 2;
    let mut w = World::new(42, cfg);
    let lots_before = citysim::systems::founding::vacant_lots(&w).len();
    let out = evict_n(&mut w, 200);
    for &a in &out {
        citysim::systems::lod::set_lod(&mut w, a, Lod::Statistical);
    }
    street::grow_shanties(&mut w);
    let shanties: Vec<EntityId> =
        w.buildings_of_kind(BuildingKind::Lot).iter().copied().filter(|&l| street::is_shanty(&w, l)).collect();
    assert_eq!(shanties.len(), 2, "capped at shanty_lots_max");
    assert_eq!(citysim::systems::founding::vacant_lots(&w).len(), lots_before, "a shanty stays a buildable Lot");
    street::grow_shanties(&mut w);
    let n = w.buildings_of_kind(BuildingKind::Lot).iter().filter(|&&l| street::is_shanty(&w, l)).count();
    assert_eq!(n, 2, "no more past the cap");
    street::assign_statistical_squats(&mut w);
    let in_shanty = out
        .iter()
        .filter(|&&a| w.comp::<citysim::Squatter>(a).is_some_and(|s| street::is_shanty(&w, s.building)))
        .count();
    assert!(in_shanty > 0, "the off-screen homeless settle in the shanties");
    // Off, nothing rises.
    let mut v = World::new(42, sick_cfg());
    evict_n(&mut v, 200);
    street::grow_shanties(&mut v);
    assert!(!v.buildings_of_kind(BuildingKind::Lot).iter().any(|&l| street::is_shanty(&v, l)));
}

/// Building on a shanty clears it: its squatters are put out (an eviction
/// story), the Lot stands as the new building.
#[test]
fn test_building_on_a_shanty_puts_its_squatters_out() {
    use citysim::systems::street;
    let mut w = World::new(42, rungs_cfg());
    let out = evict_n(&mut w, 60);
    street::grow_shanties(&mut w);
    let lot =
        w.buildings_of_kind(BuildingKind::Lot).iter().copied().find(|&l| street::is_shanty(&w, l)).expect("a shanty");
    for &a in out.iter().take(3) {
        street::occupy(&mut w, a, lot).expect("a slot");
    }
    assert_eq!(w.squatters_of(lot).len(), 3);
    citysim::systems::founding::convert(&mut w, lot, BuildingKind::Bar, None, 0);
    assert!(w.squatters_of(lot).is_empty(), "cleared to build");
    assert!(out.iter().take(3).all(|&a| !w.has::<citysim::Squatter>(a)));
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::SquatEvicted && e.text.contains("cleared to build")));
    assert!(!w.comp::<Building>(lot).expect("bar").derelict);
}

/// A Hotel booked full adds a floor of beds its owner pays for.
#[test]
fn test_full_hotel_adds_a_floor_of_beds() {
    use citysim::systems::street;
    let mut w = World::new(42, rungs_cfg());
    let Some(h) = w.buildings_of_kind(BuildingKind::Hotel).iter().copied().find(|&h| {
        !citysim::systems::charity::is_cot_house(&w, h) && w.comp::<Building>(h).is_some_and(|b| !b.derelict)
    }) else {
        eprintln!("no Hotel on seed 42: nothing to check");
        return;
    };
    let beds = usize::from(w.comp::<Building>(h).expect("hotel").capacity);
    let guests = evict_n(&mut w, beds);
    for &a in &guests {
        w.comp_mut::<Wallet>(a).expect("wallet").coins = 50;
        w.hotel_beds.insert(a, (h, w.tick + TICKS_PER_DAY));
    }
    assert_eq!(street::free_beds(&w, h), 0);
    if let Some(o) = w.comp::<Building>(h).and_then(|b| b.owner) {
        if let Some(x) = w.comp_mut::<Wallet>(o) {
            x.coins = 100_000;
        }
    }
    street::grow_hotels(&mut w);
    let b = w.comp::<Building>(h).expect("hotel");
    assert_eq!(b.floors, 2);
    assert!(usize::from(b.capacity) > beds, "a floor of beds");
}
