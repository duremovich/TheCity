//! The violence fixes (2026-10-09, the wages-on diagnosis; `[life]
//! violence_fixes`): the ledger's Street class, short sources rolled for
//! their live hours, and the chrome humanity cap. Everything here is
//! counters, seeded dice rolls and hole records between fictional agents of
//! a simulated city.

use citysim::ledger::{ActiveSource, VictimClass, ViolenceSource};
use citysim::systems::econ::identity;
use citysim::systems::{assets, demography, fixes, fviolence, law, lod};
use citysim::{
    ActionInstance, ActionKind, AssetKind, Brain, Config, DeathCause, DistrictId, EntityId, Gang, GoalKind, HoleKind,
    Household, Job, Kit, Lod, Order, Personality, Plan, Position, Role, Slot, Wallet, World, TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    let w = World::new(seed, Config::load().scaled_to(300));
    assert!(fixes::violence_on(&w), "[life] violence_fixes on in the shipped config");
    w
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&a| law::living(w, a) && demography::is_adult(w, a)).collect()
}

/// Free civilian adults (no gang, no guard's job, not sentenced).
fn civilians(w: &World) -> Vec<EntityId> {
    adults(w)
        .into_iter()
        .filter(|&a| w.gang_of(a).is_none() && !law::is_guard(w, a) && !w.has::<citysim::Sentence>(a))
        .collect()
}

fn housed(w: &World, a: EntityId) -> bool {
    w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
}

fn unhome(w: &mut World, a: EntityId) {
    w.comp_mut::<Household>(a).expect("household").home = None;
}

// ---------------------------------------------------------------------------
// Fix 1: the Street class
// ---------------------------------------------------------------------------

#[test]
fn test_street_class_is_the_homeless_civilian_only_with_the_fix() {
    let mut w = world(42);
    let civ: Vec<EntityId> = civilians(&w).into_iter().filter(|&a| housed(&w, a)).collect();
    let (a, b) = (civ[0], civ[1]);
    assert_eq!(fviolence::victim_class(&w, a), VictimClass::Civilian, "housed: Civilian");
    unhome(&mut w, b);
    assert_eq!(fviolence::victim_class(&w, b), VictimClass::Street, "no Home: Street");
    // A homeless gang member stays a Member.
    let g = w.gangs()[0];
    citysim::systems::gang::enlist(&mut w, b, g);
    assert_eq!(fviolence::victim_class(&w, b), VictimClass::Member);
    // Fix off: the homeless are Civilians again (the 644eb10 ledger).
    let c = civ[2];
    unhome(&mut w, c);
    w.config.life.violence_fixes = false;
    assert_eq!(fviolence::victim_class(&w, c), VictimClass::Civilian);
    w.config.life.violence_fixes = true;
    w.config.life.vf_street_class = false;
    assert_eq!(fviolence::victim_class(&w, c), VictimClass::Civilian, "the sub-switch alone");
}

#[test]
fn test_evictee_killing_is_learned_as_street_not_civilian() {
    let mut w = world(42);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let g = w.gangs()[0];
    let recruits: Vec<EntityId> = civilians(&w).into_iter().rev().take(8).collect();
    for &m in &recruits {
        citysim::systems::gang::enlist(&mut w, m, g);
    }
    let m = w.comp::<Gang>(g).and_then(|x| x.members.first().copied()).expect("a member");
    w.comp_mut::<Gang>(g).expect("gang").order = Order::Contest;
    if let Some(p) = w.comp_mut::<Personality>(m) {
        p.loyalty = 1.0;
    }
    let now = w.tick;
    let br = w.comp_mut::<Brain>(m).expect("brain");
    br.plan = Some(Plan {
        goal: GoalKind::GangWork,
        target: None,
        steps: vec![ActionInstance { action: ActionKind::Extort, target: None, tile: None }],
        started_tick: now,
    });
    br.plan_step = 0;
    let v = civilians(&w).into_iter().find(|&a| housed(&w, a)).expect("a civilian");
    unhome(&mut w, v);
    for a in [m, v] {
        lod::set_lod(&mut w, a, Lod::Coarse);
    }
    let d = w.district_of(w.comp::<Position>(v).expect("pos").tile);
    w.kill_by(v, DeathCause::Violence, Some(m));
    let key = |c| (ViolenceSource::Order(Order::Contest), d, c);
    let street = w.order_rates.cells.get(&key(VictimClass::Street)).map(|c| c.victims_today[0]);
    let civ = w.order_rates.cells.get(&key(VictimClass::Civilian)).map_or(0, |c| c.victims_today[0]);
    assert_eq!(street, Some(1), "the evictee's death is the Street cell's");
    assert_eq!(civ, 0, "and never the housed civilians'");
    // The parity tallies still count it as a civilian victim.
    assert_eq!(w.fv_tally.body_civ_victims, 1);
}

#[test]
fn test_street_prior_is_the_civilian_prior_times_the_mult() {
    let mut w = world(43);
    w.order_rates.cells.clear();
    w.config.fviolence.fv_mult = 1.0;
    w.config.life.vf_street_prior_mult = 3.0;
    let s = ViolenceSource::Order(Order::Contest);
    let civ = fviolence::cell_rates(&w, s, DistrictId(2), VictimClass::Civilian);
    let street = fviolence::cell_rates(&w, s, DistrictId(2), VictimClass::Street);
    for k in 0..4 {
        assert!((street[k] - 3.0 * civ[k]).abs() < 1e-7, "kind {k}: {} vs 3 x {}", street[k], civ[k]);
    }
    assert!(street[0] > 0.0);
}

/// A district with many Statistical housed free civilians (their Homes').
fn stat_district(w: &World) -> DistrictId {
    let mut count = std::collections::BTreeMap::<DistrictId, usize>::new();
    for &a in w.tier(Lod::Statistical) {
        if w.gang_of(a).is_none() && demography::is_adult(w, a) {
            if let Some(h) = w.comp::<Household>(a).and_then(|h| h.home) {
                *count.entry(w.district_of_building(h)).or_default() += 1;
            }
        }
    }
    count.into_iter().max_by_key(|&(d, n)| (n, std::cmp::Reverse(d))).map(|(d, _)| d).expect("a district")
}

#[test]
fn test_street_rates_are_not_applied_to_the_housed() {
    let run = |fix: bool| {
        let mut w = world(44);
        w.run_ticks(TICKS_PER_HOUR + 1);
        let g = w.gangs()[0];
        let d = stat_district(&w);
        w.push_command(citysim::PlayerCommand::FactionStrike { gang: g, district: d, days: 3 });
        w.run_ticks(1);
        w.config.life.violence_fixes = fix;
        // No prior: only what the ledger learned acts.
        w.config.fviolence.prior.order = [0.0; 4];
        w.config.fviolence.fv_mult = 1.0;
        w.config.fviolence.day_cap.killed = 10_000;
        // The ledger learned a deadly street: the evictees' cell (Street with
        // the fix, Civilian without) full of killings.
        let class = if fix { VictimClass::Street } else { VictimClass::Civilian };
        let cell = w.order_rates.cell_mut((ViolenceSource::Order(Order::Contest), d, class));
        cell.victims_today = [500, 0, 0, 0];
        cell.exposure_today = 24 * 500;
        cell.roll(14);
        fviolence::daily(&mut w);
        // The hole keeps the victim's Home at the killing (the dead are unlinked).
        let killed: Vec<bool> = w
            .holes
            .values()
            .filter(|h| h.kind == HoleKind::Killed && h.source.is_some())
            .map(|h| h.home.is_some())
            .collect();
        (killed.len(), killed.iter().filter(|&&h| h).count())
    };
    let (off_all, off_housed) = run(false);
    assert!(off_housed > 10, "without the fix the evictees' rate kills the housed ({off_housed} of {off_all})");
    let (_, on_housed) = run(true);
    assert_eq!(on_housed, 0, "with the fix no housed civilian reads the Street cell");
}

// ---------------------------------------------------------------------------
// Fix 2: short sources roll for their live hours
// ---------------------------------------------------------------------------

#[test]
fn test_live_share_is_hours_over_24() {
    let s = ActiveSource {
        source: ViolenceSource::Riot,
        district: DistrictId(1),
        faction: None,
        riot: Some(7),
        episode: None,
    };
    let other = ActiveSource { riot: Some(8), ..s };
    assert!((fviolence::live_share(&[(s, 6)], &s) - 0.25).abs() < 1e-7);
    assert!((fviolence::live_share(&[(s, 6)], &other) - 1.0 / 24.0).abs() < 1e-7, "untallied: one hour");
    assert!((fviolence::live_share(&[(s, 30)], &s) - 1.0).abs() < 1e-7, "at most a day");
}

/// A riot live at `hours` hourly tallies in the biggest Statistical
/// district, a Killed riot prior of 0.9 and no cap: the Statistical
/// adults the midnight pass kills.
fn riot_kills(fix: bool, hours: u32) -> (u32, u32) {
    use citysim::{Riot, RiotResponse, RiotTarget};
    let mut w = world(48);
    w.config.life.violence_fixes = fix;
    w.run_ticks(citysim::TICKS_PER_DAY + 6 * TICKS_PER_HOUR);
    let d = stat_district(&w);
    let rioters: Vec<EntityId> = civilians(&w)
        .into_iter()
        .filter(|&a| w.comp::<Brain>(a).is_some_and(|b| b.lod != Lod::Statistical))
        .take(10)
        .collect();
    let jail = w.buildings_of_kind(citysim::BuildingKind::Jail)[0];
    w.riots.push(Riot {
        id: 91,
        district: d,
        target: jail,
        kind: RiotTarget::Precinct,
        muster: jail,
        muster_at: w.tick,
        rioters: rioters.clone(),
        response: RiotResponse::Contain,
        started: w.tick,
        departed: None,
    });
    for &r in &rioters {
        w.rioter_of.insert(r, 91);
    }
    for _ in 0..hours {
        fviolence::tally_exposure(&mut w);
    }
    let tallied = w.fv_tally.source_hours.iter().find(|(s, _)| s.riot == Some(91)).map_or(0, |&(_, h)| h);
    w.riots.clear();
    for r in &rioters {
        w.rioter_of.remove(r);
    }
    w.config.fviolence.prior = Default::default();
    w.config.fviolence.prior.order = [0.0; 4];
    w.config.fviolence.prior.vendetta = [0.0; 4];
    w.config.fviolence.prior.episode = [0.0; 4];
    w.config.fviolence.prior.riot = [0.9, 0.0, 0.0, 0.0];
    w.config.fviolence.prior_weight = 1e6;
    w.config.fviolence.fv_mult = 1.0;
    w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    w.config.fviolence.day_cap.killed = 10_000;
    fviolence::daily(&mut w);
    assert!(w.fv_tally.source_hours.is_empty(), "the day's hours are cleared");
    (w.stats.current.living.fv_killed, tallied)
}

#[test]
fn test_short_riot_rolls_its_hours_not_a_day() {
    let (off, off_hours) = riot_kills(false, 6);
    assert_eq!(off_hours, 0, "the fix off tallies no hours");
    let (on6, on_hours) = riot_kills(true, 6);
    assert_eq!(on_hours, 6, "six hourly tallies");
    let (on24, _) = riot_kills(true, 24);
    assert!(off > 15, "a full day's 0.9 kills many ({off})");
    // A 24-hour riot is the old full day (same rolls).
    assert_eq!(on24, off, "a day-long riot rolls as before");
    // A 6-hour riot: a quarter of the rate (0.225 vs 0.9): well under half.
    assert!(on6 * 2 < off && on6 > 0, "six hours {on6} vs a day {off}");
}

// ---------------------------------------------------------------------------
// Fix 3: the chrome humanity cap
// ---------------------------------------------------------------------------

fn grant(w: &mut World, a: EntityId, slot: Slot, tier: u8) {
    assets::grant(w, a, AssetKind::Implant(slot), tier).expect("chrome");
}

#[test]
fn test_chrome_max_tier_keeps_target_sanity_over_psycho() {
    let mut w = world(42);
    assert!((w.config.chrome.psycho - 0.8).abs() < 1e-6, "the shipped psycho line");
    let civ: Vec<EntityId> = civilians(&w).into_iter().filter(|&a| !w.has::<Job>(a)).collect();
    let (a, b, c) = (civ[0], civ[1], civ[2]);
    assert_eq!(assets::chrome_max_tier(&w, a), 2, "a bare body: T2 (0.15) fits under 0.2, T3 (0.25) does not");
    grant(&mut w, b, Slot::Legs, 1);
    assert_eq!(assets::chrome_max_tier(&w, b), 1, "after a T1 (0.08): another T1 only");
    grant(&mut w, b, Slot::Eyes, 1);
    assert!((w.comp::<Kit>(b).expect("kit").load - 0.16).abs() < 1e-6);
    assert_eq!(assets::chrome_max_tier(&w, b), 0, "two T1s: the third would cross 0.8");
    grant(&mut w, c, Slot::Legs, 2);
    assert_eq!(assets::chrome_max_tier(&w, c), 0, "a T2 (0.15): nothing more");
    // A gang member is not capped.
    let g = w.gangs()[0];
    citysim::systems::gang::enlist(&mut w, b, g);
    assert_eq!(assets::chrome_max_tier(&w, b), 3, "a gang member buys past the line");
    // Fix off: no cap.
    w.config.life.violence_fixes = false;
    assert_eq!(assets::chrome_max_tier(&w, c), 3);
}

#[test]
fn test_courage_alone_no_longer_unlocks_the_fighter_list() {
    let mut w = world(42);
    let civ: Vec<EntityId> = civilians(&w).into_iter().filter(|&a| !w.has::<Job>(a)).collect();
    let a = civ[0];
    w.comp_mut::<Personality>(a).expect("personality").courage = 0.95;
    assert_eq!(assets::chrome_slot(&w, a), Some(Slot::Legs), "a brave civilian: the civilian list");
    w.config.life.violence_fixes = false;
    assert_eq!(assets::chrome_slot(&w, a), Some(Slot::Arms), "fix off: courage unlocked the five slots");
    w.config.life.violence_fixes = true;
    // A pit Fighter (by role) gets the five-slot list.
    let worker = adults(&w)
        .into_iter()
        .find(|&x| w.comp::<Job>(x).is_some_and(|j| j.role != Role::Guard) && w.gang_of(x).is_none())
        .expect("a worker");
    w.comp_mut::<Personality>(worker).expect("personality").courage = 0.1;
    w.comp_mut::<Job>(worker).expect("job").role = Role::Fighter;
    assert_eq!(assets::chrome_slot(&w, worker), Some(Slot::Arms), "a Fighter: the fighter list");
    assert_eq!(assets::chrome_max_tier(&w, worker), 3, "and no cap");
}

#[test]
fn test_capped_civilian_gets_no_chrome_offer() {
    let offers = |fix: bool| {
        let mut w = world(42);
        w.config.life.violence_fixes = fix;
        w.tick = 600;
        let civ: Vec<EntityId> = civilians(&w)
            .into_iter()
            .filter(|&a| {
                w.comp::<Job>(a).is_some_and(|j| j.role != Role::Fighter)
                    && housed(&w, a)
                    && w.comp::<Brain>(a).is_some()
            })
            .take(40)
            .collect();
        let mut chrome = 0;
        for a in civ {
            // A T2 Legs (load 0.15), a car and money: the Eyes would be the next buy.
            grant(&mut w, a, Slot::Legs, 2);
            assets::grant(&mut w, a, AssetKind::Car, 1).expect("car");
            w.comp_mut::<Wallet>(a).expect("wallet").coins = 100_000;
            // A wage that carries the upkeep (the burden room).
            w.comp_mut::<Job>(a).expect("job").wage_per_day = 1_000;
            // Timid: the civilian list either way (the cap alone is tested).
            w.comp_mut::<Personality>(a).expect("personality").courage = 0.3;
            if let Some(n) = w.comp_mut::<citysim::Needs>(a) {
                n.wealth = 0.0;
            }
            if let Some(o) = assets::shop_choice(&w, a, false) {
                if o.category == assets::ShopCategory::Chrome {
                    assert_eq!(o.pick.kind, AssetKind::Implant(Slot::Eyes));
                    chrome += 1;
                }
            }
        }
        chrome
    };
    let off = offers(false);
    assert!(off > 0, "fix off: a T2 civilian still buys Eyes");
    assert_eq!(offers(true), 0, "fix on: a T2 civilian is at the line");
}

// ---------------------------------------------------------------------------
// Fix 4: rent from income leaves a meal
// ---------------------------------------------------------------------------

/// A housed civilian (not its own landlord) owing 7 whole coins with 7 (its
/// wage) in its purse and `arrears` days short: (coins after, rent_due
/// after, the meal's price).
fn rent_from_wage(fix: bool, arrears: u8) -> (i64, f32, i64) {
    let mut w = world(42);
    w.config.life.violence_fixes = fix;
    w.config.rent.pay_from_income = true;
    let a = civilians(&w)
        .into_iter()
        .find(|&a| w.comp::<Household>(a).and_then(|h| h.home).is_some_and(|h| w.owner_of(h) != Some(a)))
        .expect("a tenant");
    let h = w.comp_mut::<Household>(a).expect("household");
    h.rent_due = 7.0;
    h.arrears = arrears;
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 7;
    citysim::systems::ownership::pay_rent_from_income(&mut w, a);
    let coins = w.comp::<Wallet>(a).expect("wallet").coins;
    (coins, w.comp::<Household>(a).expect("household").rent_due, w.mean_price())
}

#[test]
fn test_rent_from_wage_leaves_a_meal_in_arrears() {
    let (coins, due, meal) = rent_from_wage(true, 1);
    assert!((1..7).contains(&meal), "a meal costs less than the wage ({meal})");
    assert_eq!(coins, meal, "a meal's worth stays in the purse");
    assert!((due - meal as f32).abs() < 1e-6, "the rest paid: {due} owed");
    // No arrears: the whole due is paid, as before.
    let (coins, due, _) = rent_from_wage(true, 0);
    assert_eq!((coins, due), (0, 0.0));
    // The switch off: the wage goes to the arrears whole (644eb10).
    let (coins, due, _) = rent_from_wage(false, 1);
    assert_eq!((coins, due), (0, 0.0));
}

// ---------------------------------------------------------------------------
// Fix 5: an immigrant arrives with a seeded adult's savings
// ---------------------------------------------------------------------------

/// Spawn immigrants until one lands in a Home of `tier` (or homeless with
/// `None`): its coins, and the coin identity before and after.
fn immigrant_coins(fix: bool, tier: Option<u8>) -> Option<(i64, i64, i64)> {
    let mut w = world(42);
    w.config.life.violence_fixes = fix;
    for _ in 0..200 {
        let before = identity(&w);
        let id = demography::spawn_immigrant(&mut w);
        let after = identity(&w);
        let t = w
            .comp::<Household>(id)
            .and_then(|h| h.home)
            .and_then(|h| w.comp::<citysim::Building>(h))
            .map(|b| b.tier.min(2));
        if t == tier {
            return Some((w.comp::<Wallet>(id).expect("wallet").coins, before, after));
        }
    }
    None
}

#[test]
fn test_immigrant_arrives_with_a_seeded_adults_savings() {
    // [world] initial_coins 10..=40 (mean 25) x coins_by_tier [0.5, 1, 3].
    let mut seen = 0;
    for (tier, want) in [(Some(0u8), 15i64), (Some(1), 25), (Some(2), 75)] {
        let Some((coins, before, after)) = immigrant_coins(true, tier) else { continue };
        seen += 1;
        assert_eq!(coins, want, "tier {tier:?}: a seeded adult's savings (Sump keeps the 15)");
        assert_eq!(before, after, "the savings cross in from the World: the identity holds");
        let (off, _, _) = immigrant_coins(false, tier).expect("the same arrival with the fix off");
        assert_eq!(off, 15, "fix off: the flat 15");
    }
    assert!(seen >= 2, "immigrants housed in at least two tiers");
}
