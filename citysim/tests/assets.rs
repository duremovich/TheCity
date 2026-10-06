//! M13 phase 1: the asset model (docs/M13_ASSETS.md § 1, plan 1.9): buying
//! and finance, repossession and impound, wear and wrecks, capacity, corpses
//! as loot, the Recycler's Parts, the scavs, the parts market, the Kit and
//! the keyed Body draws.

use rand::Rng;

use citysim::systems::{assets, demography, economy, founding, ownership};
use citysim::{
    Asset, AssetKind, AssetLoc, Body, Brain, Building, BuildingKind, Config, Controller, Corpse, DeathCause, EntityId,
    EventKind, Finance, Gang, Good, Inventory, Kit, ShopPick, Slot, Wallet, World, TICKS_PER_DAY,
};

fn city() -> World {
    World::new(42, Config::load())
}

/// Living adults with a Brain and a Wallet, ascending.
fn adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && w.has::<Wallet>(a) && demography::is_adult(w, a))
        .collect()
}

fn set_coins(w: &mut World, a: EntityId, coins: i64) {
    w.comp_mut::<Wallet>(a).expect("wallet").coins = coins;
}

/// Phase 2 seeds two Garages and phase 3 three Clinics (D18); a phase 1
/// test that needs a city with none (an impound nobody buys, one Garage at
/// the parts market) sets them aside: a demolished building buys and sells
/// nothing.
fn set_seeded_garages_aside(w: &mut World) {
    let sellers: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .chain(w.buildings_of_kind(BuildingKind::Clinic))
        .copied()
        .collect();
    for g in sellers {
        w.comp_mut::<Building>(g).expect("garage").demolished = true;
    }
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |x| x.coins)
}

/// A Tech corp with `treasury` and a Garage it built on the first vacant Lot.
fn garage(w: &mut World, treasury: i64) -> (EntityId, EntityId) {
    let corp = ownership::spawn_corp(w, "Tech".into(), Default::default(), treasury, None);
    let lot = founding::vacant_lots(w)[0];
    let g = founding::build_on_lot(w, lot, BuildingKind::Garage, Some(corp)).expect("a Garage on a Lot");
    (corp, g)
}

fn asset(w: &World, a: EntityId) -> &Asset {
    w.comp::<Asset>(a).expect("asset")
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

/// A married pair of living adults (seeded spouses).
fn couple(w: &World) -> (EntityId, EntityId) {
    adults(w)
        .into_iter()
        .find_map(|a| w.spouse_of(a).filter(|&s| w.has::<Brain>(s)).map(|s| (a, s)))
        .expect("a married couple")
}

#[test]
fn test_purchase_pays_seller_imports_and_conserves() {
    let mut w = city();
    // The plan's car and finance terms (phase 5 calibration moved them).
    w.config.assets.price.car = vec![800, 1400];
    w.config.assets.down_frac = 0.25;
    w.config.assets.term_days = 60;
    let (corp, g) = garage(&mut w, 5000);
    let people = adults(&w);
    let (rich, poor) = (people[0], people[1]);
    set_coins(&mut w, rich, 1000);
    let total = ownership::total_coins(&w);
    let (corp0, city0, tax0) = (w.purse(Some(corp)), w.purse(None), w.stats.current.flow_tax);
    let pick = ShopPick { kind: AssetKind::Car, tier: 1, used: None };
    let car = assets::buy(&mut w, rich, g, &pick).expect("a cash purchase");
    let tax = w.stats.current.flow_tax - tax0;
    assert!(tax > 0, "the sale is taxed");
    assert_eq!(coins(&w, rich), 200);
    assert_eq!(w.purse(Some(corp)) - corp0, 800 - tax - 480, "the price less the tax and the 0.6 import");
    assert_eq!(w.purse(None) - city0, tax + 480, "the tax and the import reach the Treasury");
    assert_eq!(ownership::total_coins(&w), total, "money is conserved");
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc, x.condition, x.list), (Some(rich), AssetLoc::Parked(g), 100, 800));
    assert!(x.finance.is_none());
    assert_eq!(w.stats.current.flow_asset, 800);
    assert_eq!(w.stats.current.flow_import, 480);
    assert_eq!(w.comp::<Kit>(rich).expect("kit").vehicle, Some(car));
    assert_eq!(count(&w, EventKind::AssetBought), 1);
    // Financed: 300 coins buys the 200 down payment.
    set_coins(&mut w, poor, 300);
    let total = ownership::total_coins(&w);
    let car2 = assets::buy(&mut w, poor, g, &pick).expect("a financed purchase");
    let f = asset(&w, car2).finance.clone().expect("a finance plan");
    assert_eq!(f, Finance { lender: Some(corp), remaining: 600, per_day: 12, arrears: 0 });
    assert_eq!(coins(&w, poor), 100);
    assert_eq!(ownership::total_coins(&w), total);
    // One daily pass with upkeep and finance.
    assets::run(&mut w);
    assert_eq!(ownership::total_coins(&w), total, "upkeep and finance move coins, never make them");
    assert_eq!(asset(&w, car2).finance.as_ref().expect("plan").remaining, 588);
    // Phase 2: parked in the Garage, it pays the Garage its rent (1).
    assert_eq!(coins(&w, poor), 100 - 2 - 12 - 1, "upkeep 2, the day's 12, the Garage's rent");
    assert!(w.stats.current.flow_asset_upkeep >= 4 && w.stats.current.flow_finance == 12);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_finance_arrears_tows_vehicle_to_lenders_garage() {
    let mut w = city();
    let (corp, g) = garage(&mut w, 5000);
    let buyer = adults(&w)[3];
    set_coins(&mut w, buyer, 300);
    let car = assets::buy(&mut w, buyer, g, &ShopPick { kind: AssetKind::Car, tier: 1, used: None }).expect("buy");
    // The buyer drives it home.
    let home = w.comp::<citysim::Household>(buyer).and_then(|h| h.home).expect("a home");
    assets::set_loc(&mut w, car, AssetLoc::Parked(home));
    set_coins(&mut w, buyer, 0);
    let repo_days = w.config.assets.repo_days;
    for day in 1..=repo_days {
        assets::run(&mut w);
        let towed = asset(&w, car).owner == Some(corp);
        assert_eq!(towed, day == repo_days, "towed on day {day} exactly");
    }
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc), (Some(corp), AssetLoc::Stock(g)));
    assert!(x.finance.is_none());
    assert_eq!(count(&w, EventKind::Repossessed), 1);
    assert_eq!(w.stats.current.repos, 1);
    assert_eq!(w.comp::<Kit>(buyer).expect("kit").vehicle, None, "the Kit lost the car");
    w.check_indices().expect("indices in step");
}

#[test]
fn test_financed_implant_bricks_then_transfers() {
    let mut w = city();
    let (corp, _) = garage(&mut w, 5000);
    let who = adults(&w)[5];
    let arm = assets::grant(&mut w, who, AssetKind::Implant(Slot::Arms), 1).expect("granted");
    w.comp_mut::<Asset>(arm).expect("asset").finance =
        Some(Finance { lender: Some(corp), remaining: 1000, per_day: 10, arrears: 0 });
    set_coins(&mut w, who, 0);
    let (repo, brick) = (u32::from(w.config.assets.repo_days), u32::from(w.config.assets.brick_repo_days));
    for _ in 0..repo {
        assets::run(&mut w);
    }
    assert!(asset(&w, arm).bricked, "bricked at {repo} days of arrears");
    let k = w.comp::<Kit>(who).expect("kit").clone();
    assert_eq!(k.fighting, 0.0, "no modifiers while bricked");
    assert!((k.load - 0.08).abs() < 1e-6, "the load stays");
    assert_eq!(count(&w, EventKind::Repossessed), 1);
    // The arrears cleared: unbricked.
    set_coins(&mut w, who, 1000);
    assets::run(&mut w);
    assert!(!asset(&w, arm).bricked);
    assert_eq!(asset(&w, arm).finance.as_ref().expect("plan").arrears, 0);
    assert!((w.comp::<Kit>(who).expect("kit").fighting - 0.1).abs() < 1e-6);
    // Never cleared again: bricked at repo days, the lender's at repo + brick.
    set_coins(&mut w, who, 0);
    for day in 1..=repo + brick {
        assets::run(&mut w);
        let x = asset(&w, arm);
        assert_eq!(x.bricked, day >= repo, "day {day}");
        assert_eq!(x.owner == Some(corp), day == repo + brick, "day {day}");
    }
    let x = asset(&w, arm);
    assert_eq!(x.loc, AssetLoc::Installed(who), "dead metal stays in the body");
    assert!(x.bricked && x.finance.is_none());
    assert!((w.comp::<Kit>(who).expect("kit").load - 0.08).abs() < 1e-6);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_unpaid_upkeep_impounds_and_sells_to_garage() {
    let mut w = city();
    set_seeded_garages_aside(&mut w);
    let who = adults(&w)[7];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("granted");
    set_coins(&mut w, who, 0);
    let precinct = w.building_of_kind(BuildingKind::Jail).expect("precinct");
    let days = w.config.assets.impound_days;
    for _ in 0..days {
        assets::run(&mut w);
    }
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc), (None, AssetLoc::Stock(precinct)), "impounded");
    assert_eq!(w.stats.current.impounds, 1);
    // Nobody to sell to: it stays. Then a Garage opens and buys it.
    assets::run(&mut w);
    assert_eq!(asset(&w, car).loc, AssetLoc::Stock(precinct));
    let (corp, g) = garage(&mut w, 5000);
    let value = asset(&w, car).value;
    assert!(value > 0 && value < 800, "worn at the unpaid rate: {value}");
    let price = (w.config.assets.impound_frac * value as f32).round() as i64;
    let (corp0, city0) = (w.purse(Some(corp)), w.purse(None));
    assets::run(&mut w);
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc), (Some(corp), AssetLoc::Stock(g)), "sold to the Garage");
    assert_eq!(corp0 - w.purse(Some(corp)), price);
    assert_eq!(w.purse(None) - city0, price);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_wear_wrecks_into_parts() {
    let mut w = city();
    let who = adults(&w)[9];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("granted");
    let AssetLoc::Parked(at) = asset(&w, car).loc else { panic!("parked") };
    w.comp_mut::<Asset>(car).expect("asset").condition = 1;
    let parts = w.stock(at, Good::Parts);
    assets::run(&mut w);
    assert!(!w.has::<Asset>(car), "wrecked and despawned");
    assert_eq!(w.stock(at, Good::Parts), parts + w.config.assets.parts_per.car / 2);
    assert_eq!(w.stock(at, Good::Parts), parts + 4);
    assert_eq!(count(&w, EventKind::Wrecked), 1);
    assert_eq!(w.comp::<Kit>(who).expect("kit").vehicle, None);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_capacity_with_strength_and_pack() {
    let mut w = city();
    let who = adults(&w)[11];
    w.comp_mut::<Body>(who).expect("body").strength = 0.4;
    assert_eq!(assets::capacity(&w, who), 24);
    // No stims or parts: buy_quantity is the M12 one.
    let market = w.building_of_kind(BuildingKind::Market);
    for (food, coins) in [(0, 100), (18, 100), (20, 100), (5, 4), (0, 0)] {
        w.comp_mut::<Inventory>(who).expect("inv").food = food;
        set_coins(&mut w, who, coins);
        let price = w.price_for(market.expect("market"), who);
        let stock = w.comp::<Building>(market.expect("m")).expect("b").stock_food;
        let m12 = ((coins / price).clamp(0, 3) as u32).min(20 - food).min(stock);
        assert_eq!(economy::buy_quantity(&w, who, market), m12, "food {food}, coins {coins}");
    }
    assets::grant(&mut w, who, AssetKind::Pack, 2).expect("a pack");
    assert_eq!(assets::capacity(&w, who), 49, "24 + the T2 pack's 25");
    let inv = w.comp_mut::<Inventory>(who).expect("inv");
    inv.food = 0;
    inv.parts = 5;
    assert_eq!(assets::load(w.comp::<Inventory>(who).expect("inv")), 15, "a Part loads 3");
    // Assets off: the v1 cap.
    w.config.assets.enabled = false;
    assert_eq!(assets::capacity(&w, who), 20);
}

#[test]
fn test_corpse_keeps_loot_until_window_then_inherits() {
    let mut w = city();
    w.config.assets.loot_window_hours = 12;
    w.config.assets.scav_strip_base = 0.0;
    let (dead, spouse) = couple(&w);
    set_coins(&mut w, dead, 100);
    let total = ownership::total_coins(&w);
    let before = coins(&w, spouse);
    w.kill(dead, DeathCause::Violence);
    let c = w.comp::<Corpse>(dead).expect("corpse");
    assert_eq!(c.loot.coins, 100, "the coins stay on the body");
    assert!(!c.settled);
    assert!(!w.has::<Wallet>(dead), "a corpse keeps no Wallet");
    assert_eq!(coins(&w, spouse), before, "nothing inherited yet");
    assert_eq!(ownership::total_coins(&w), total, "the loot is counted");
    assert_eq!(w.loot_corpses, vec![dead]);
    // The first midnight 12 h or more after the death.
    w.tick += TICKS_PER_DAY;
    assets::run(&mut w);
    assert_eq!(coins(&w, spouse), before + 100, "inherited at the window");
    assert!(w.comp::<Corpse>(dead).expect("corpse").settled);
    assert!(w.loot_corpses.is_empty());
    assert_eq!(ownership::total_coins(&w), total);
    // Window 0 (phase 1): inheritance at death, as M12.
    let mut w = city();
    w.config.assets.loot_window_hours = 0;
    let (dead, spouse) = couple(&w);
    set_coins(&mut w, dead, 100);
    let before = coins(&w, spouse);
    w.kill(dead, DeathCause::Violence);
    assert_eq!(coins(&w, spouse), before + 100);
    let c = w.comp::<Corpse>(dead).expect("corpse");
    assert!(c.settled && c.loot.coins == 0);
}

#[test]
fn test_buried_implant_becomes_recycler_parts() {
    let mut w = city();
    let people = adults(&w);
    let (who, digger) = (people[13], people[14]);
    let arm = assets::grant(&mut w, who, AssetKind::Implant(Slot::Arms), 1).expect("granted");
    w.kill(who, DeathCause::Violence);
    assert_eq!(asset(&w, arm).loc, AssetLoc::Installed(who), "the chrome stays in the body");
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("recycler");
    let parts = w.stock(recycler, Good::Parts);
    assert!(demography::bury(&mut w, digger, who));
    assert!(!w.has::<Asset>(arm), "recycled");
    assert_eq!(w.stock(recycler, Good::Parts), parts + 3);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_scav_strip_to_controlling_gang() {
    let mut w = city();
    w.config.assets.loot_window_hours = 12;
    w.config.assets.scav_strip_base = 1.0;
    let g = w.gangs()[0];
    let who = adults(&w).into_iter().find(|&a| w.gang_of(a).is_none()).expect("a civilian");
    set_coins(&mut w, who, 50);
    let tile = w.comp::<citysim::Position>(who).expect("pos").tile;
    let d = w.district_of(tile);
    w.district_mut(d).control = Controller::Gang(g);
    w.kill(who, DeathCause::Violence);
    let gang0 = w.comp::<Gang>(g).expect("gang").treasury;
    let total = ownership::total_coins(&w);
    assets::run(&mut w);
    assert_eq!(w.comp::<Gang>(g).expect("gang").treasury, gang0 + 50, "the coins to the gang");
    let c = w.comp::<Corpse>(who).expect("corpse");
    assert!(c.stripped && c.settled && c.loot.coins == 0);
    assert_eq!(count(&w, EventKind::Stripped), 1);
    assert_eq!(w.stats.current.stripped, 1);
    assert_eq!(ownership::total_coins(&w), total);
}

#[test]
fn test_parts_market_buys_from_gang_first() {
    let mut w = city();
    set_seeded_garages_aside(&mut w);
    let (corp, g) = garage(&mut w, 5000);
    let gang = w.gangs()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("recycler");
    w.comp_mut::<Building>(hideout).expect("b").stock_goods[1] = 30;
    w.comp_mut::<Building>(recycler).expect("b").stock_goods[1] = 30;
    let (gang0, corp0) = (w.comp::<Gang>(gang).expect("g").treasury, w.purse(Some(corp)));
    let tax0 = w.stats.current.flow_tax;
    assets::run(&mut w);
    assert_eq!(w.stock(g, Good::Parts), 20);
    assert_eq!(w.stock(hideout, Good::Parts), 10, "the chop shop's buyer: the gang first");
    assert_eq!(w.stock(recycler, Good::Parts), 30);
    assert_eq!(w.stats.current.flow_parts, 300, "20 at 15");
    let tax = w.stats.current.flow_tax - tax0;
    assert_eq!(w.comp::<Gang>(gang).expect("g").treasury - gang0 + tax, 300);
    assert!(corp0 - w.purse(Some(corp)) >= 300);
}

#[test]
fn test_kit_rebuilt_on_change_and_load() {
    let mut w = World::new(42, Config::load().scaled_to(300));
    let who = adults(&w)[2];
    assert!(w.comp::<Kit>(who).expect("kit").is_bare());
    let arm = assets::grant(&mut w, who, AssetKind::Implant(Slot::Arms), 2).expect("granted");
    let k = w.comp::<Kit>(who).expect("kit").clone();
    assert!((k.fighting - 0.2).abs() < 1e-6 && (k.strength - 0.5).abs() < 1e-6);
    assert_eq!(k.visible, 2);
    assert!(k.chrome && !k.is_bare());
    let back = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("load");
    assert_eq!(back.comp::<Kit>(who), Some(&k), "the Kit is rebuilt on load");
    back.check_indices().expect("indices rebuilt");
    assets::despawn(&mut w, arm);
    assert!(w.comp::<Kit>(who).expect("kit").is_bare());
    w.check_indices().expect("indices in step");
}

#[test]
fn test_body_draws_do_not_touch_world_stream() {
    let mut on = Config::load().scaled_to(300);
    on.assets.enabled = true;
    let mut off = on.clone();
    off.assets.enabled = false;
    let mut a = World::new(42, on);
    let mut b = World::new(42, off);
    assert_eq!(a.rng.world().random::<u64>(), b.rng.world().random::<u64>());
    let who = adults(&a)[0];
    let body = a.comp::<Body>(who).expect("a Body on every agent").clone();
    assert!((0.2..0.6).contains(&body.strength) && (0.2..0.6).contains(&body.reflex));
    assert_eq!(body.sanity, 1.0);
    assert_eq!(b.comp::<Body>(who), Some(&body), "keyed: the same Body either way");
    // Phase 2 review carry-over: a child's and an immigrant's Body draws
    // leave the world stream and every agent's stream where they were too.
    let (mother, father) = couple(&a);
    let home = a.comp::<citysim::Household>(mother).and_then(|h| h.home).expect("a home");
    let mut spawned = Vec::new();
    for w in [&mut a, &mut b] {
        let child = demography::spawn_child(w, mother, father, home);
        let immigrant = demography::spawn_immigrant(w);
        spawned.push((child, immigrant));
    }
    assert_eq!(spawned[0], spawned[1], "the same ids either way");
    let (child, immigrant) = spawned[0];
    for id in [child, immigrant] {
        assert!(a.has::<Body>(id) && b.has::<Body>(id), "a Body either way");
        assert_eq!(a.comp::<Body>(id), b.comp::<Body>(id), "keyed draws");
    }
    assert_eq!(a.rng.world().random::<u64>(), b.rng.world().random::<u64>(), "the world stream untouched");
    for id in [mother, father, child, immigrant] {
        assert_eq!(a.rng.agent(id).random::<u64>(), b.rng.agent(id).random::<u64>(), "agent streams untouched");
    }
}

// ---------------------------------------------------------------------------
// M13 phase 4: the security robot (plan 4.6, D41, D42)
// ---------------------------------------------------------------------------

/// A civilian adult (no gang, no job) and the Home it lives in.
fn homeowner(w: &World) -> (EntityId, EntityId) {
    adults(w)
        .into_iter()
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<citysim::Job>(a))
        .find_map(|a| w.comp::<citysim::Household>(a).and_then(|h| h.home).map(|h| (a, h)))
        .expect("a housed civilian")
}

/// Adults other than `not`, outside any gang and job, ascending.
fn others(w: &World, not: EntityId) -> Vec<EntityId> {
    adults(w).into_iter().filter(|&a| a != not && w.gang_of(a).is_none() && !w.has::<citysim::Job>(a)).collect()
}

fn put_in(w: &mut World, a: EntityId, b: EntityId) {
    w.remove_from_building(a);
    w.enter_building(a, b);
}

#[test]
fn test_robot_defends_in_brawl_and_wrecks_on_loss() {
    use citysim::systems::{law, raid, robots};
    let mut w = city();
    w.config.crime.fight_death_p = 0.0;
    w.config.riots.p_crossfire = 0.0;
    w.config.robots.robot_fighting = vec![0.0, 0.0, 0.0];
    let (owner, home) = homeowner(&w);
    let pool = others(&w, owner);
    let (raider, neighbour) = (pool[0], pool[1]);
    w.comp_mut::<citysim::Skills>(raider).expect("skills").fighting = 1.0;
    w.comp_mut::<citysim::Personality>(raider).expect("p").courage = 1.0;
    let door = w.comp::<Building>(home).expect("home").door;
    let mut wrecked = false;
    for _ in 0..60 {
        let r = assets::grant(&mut w, owner, AssetKind::Robot, 1).expect("a robot");
        assert_eq!(asset(&w, r).loc, AssetLoc::Posted(home));
        // A powered robot stands first among the defenders.
        let mut defenders = vec![neighbour];
        raid::robots_first(&w, home, &mut defenders);
        assert_eq!(defenders, vec![r, neighbour]);
        let parts = w.stock(home, Good::Parts);
        let wrecks = count(&w, EventKind::Wrecked);
        let mut raiders = vec![raider];
        let mut held = vec![r];
        let t = raid::fight_out(&mut w, &mut raiders, &mut held, door, "the test door", 1.0, None);
        assert_eq!(t.deaths, 0, "nobody dies: a robot is never killed");
        assert!(law::living(&w, raider));
        if !w.has::<Asset>(r) {
            assert!(held.is_empty(), "the robot lost");
            assert!(!w.has::<Corpse>(r), "wrecked, not a corpse");
            assert_eq!(count(&w, EventKind::Wrecked), wrecks + 1);
            assert_eq!(w.stock(home, Good::Parts), parts + 5, "half its 10 Parts at the door");
            assert!(!w.crime_reports().iter().any(|x| x.crime == citysim::Crime::Murder && x.suspect == raider));
            wrecked = true;
            break;
        }
        // The robot won this one: the raider was beaten, the robot stands.
        assert!(raiders.is_empty() && held == vec![r]);
        assets::despawn(&mut w, r);
    }
    assert!(wrecked, "a T1 robot at fighting 0 loses to a fighter at 1.0 in 60 tries");
    assert!(robots::posted_robot(&w, home).is_none());
    w.check_indices().expect("indices in step");
}

#[test]
fn test_robot_detains_thief_who_loses_contest() {
    use citysim::systems::{law, robots};
    let mut w = city();
    w.config.crime.fight_death_p = 0.0;
    let (owner, home) = homeowner(&w);
    let mut robot = assets::grant(&mut w, owner, AssetKind::Robot, 3).expect("a robot");
    let door = w.comp::<Building>(home).expect("home").door;
    let mut detained = None;
    for thief in others(&w, owner).into_iter().take(40) {
        if !w.has::<Asset>(robot) {
            robot = assets::grant(&mut w, owner, AssetKind::Robot, 3).expect("a robot");
        }
        assert_eq!(robots::powered_robot(&w, home), Some(robot));
        put_in(&mut w, thief, home);
        w.comp_mut::<citysim::Skills>(thief).expect("skills").stealth = 0.0;
        w.comp_mut::<citysim::Skills>(thief).expect("skills").fighting = 0.0;
        w.comp_mut::<citysim::Personality>(thief).expect("p").courage = 0.0;
        law::raise_crime(&mut w, thief, None, citysim::Crime::Theft, door);
        if w.has::<citysim::Sentence>(thief) {
            detained = Some(thief);
            break;
        }
    }
    let thief = detained.expect("a T3 robot detains a tier-1 thief within 40 tries");
    assert_eq!(w.comp::<citysim::Sentence>(thief).map(|s| s.crime), Some(citysim::Crime::Theft));
    assert!(
        w.crime_reports().iter().any(|r| r.suspect == thief && r.crime == citysim::Crime::Theft && r.witness.is_none()),
        "the robot's report names no witness"
    );
    assert!(w.events.iter().any(|e| e.kind == EventKind::Arrest && e.text.contains("detained")));
}

#[test]
fn test_unpowered_robot_neither_fights_nor_senses() {
    use citysim::systems::{law, raid, robots};
    let mut w = city();
    w.config.crime.fight_death_p = 0.0;
    let (owner, home) = homeowner(&w);
    let r = assets::grant(&mut w, owner, AssetKind::Robot, 3).expect("a robot");
    w.comp_mut::<Asset>(r).expect("robot").upkeep_arrears = 1;
    assert_eq!(robots::posted_robot(&w, home), Some(r), "still posted");
    assert_eq!(robots::powered_robot(&w, home), None, "but powered down");
    let pool = others(&w, owner);
    let mut defenders = vec![pool[0]];
    raid::robots_first(&w, home, &mut defenders);
    assert_eq!(defenders, vec![pool[0]], "it does not stand at the door");
    let door = w.comp::<Building>(home).expect("home").door;
    for &thief in pool.iter().skip(1).take(30) {
        put_in(&mut w, thief, home);
        w.comp_mut::<citysim::Skills>(thief).expect("skills").stealth = 0.0;
        law::raise_crime(&mut w, thief, None, citysim::Crime::Theft, door);
        assert!(!w.has::<citysim::Sentence>(thief), "nobody detained");
        assert!(
            !w.crime_reports().iter().any(|x| x.suspect == thief && x.witness.is_none()),
            "no report from the robot"
        );
    }
    assert!(w.has::<Asset>(r), "never fought");
}

#[test]
fn test_secure_buys_robot_when_cheaper_over_horizon() {
    use citysim::systems::robots;
    let mut w = city();
    for c in w.corps() {
        let cc = w.comp_mut::<citysim::Corp>(c).expect("corp");
        if cc.niches.contains(&citysim::Niche::Security) {
            cc.price_level.insert(citysim::Niche::Security, 1.0);
        }
    }
    let corp = ownership::spawn_corp(&mut w, "Holdings".into(), Default::default(), 5000, None);
    let lots = founding::vacant_lots(&w);
    let b1 = founding::build_on_lot(&mut w, lots[0], BuildingKind::Home, Some(corp)).expect("a Block");
    let b2 = founding::build_on_lot(&mut w, lots[1], BuildingKind::Home, Some(corp)).expect("a Block");
    assert_eq!(robots::robot_tier_for(&w, BuildingKind::Home), 1);
    // Contract 20 a day x 60 = 1,200 against a T1 robot 800 + 4 x 60 = 1,040.
    w.config.corps.contract_per_guard_day = 20;
    assert!(robots::consider_robot(&mut w, corp, b1), "the robot is cheaper");
    let r = robots::posted_robot(&w, b1).expect("posted");
    assert_eq!(asset(&w, r).owner, Some(corp));
    assert_eq!(asset(&w, r).tier, 1);
    assert_eq!(w.comp::<citysim::Corp>(corp).expect("corp").treasury, 5000 - 800);
    assert!(w.events.iter().any(|e| e.kind == EventKind::AssetBought && e.text.contains("for Secure")));
    assert!(!robots::consider_robot(&mut w, corp, b1), "one robot a building");
    // At 15 a day the contract (900) wins.
    w.config.corps.contract_per_guard_day = 15;
    assert!(!robots::consider_robot(&mut w, corp, b2), "the contract is cheaper");
    assert!(robots::posted_robot(&w, b2).is_none());
    w.check_indices().expect("indices in step");
}

/// Plan 5.1 (D48): the M13 levers and god commands do what they say.
#[test]
fn test_d48_levers_and_god_commands() {
    use citysim::{AssetClass, DistrictId, PlayerCommand};
    let mut w = city();
    let (corp, _) = garage(&mut w, 5000);
    w.push_command(PlayerCommand::SetAssetTax { kind: AssetClass::Car, rate: 0.5 });
    w.push_command(PlayerCommand::SetImpound(false));
    w.apply_commands();
    assert_eq!(w.levers.asset_tax[AssetClass::Car.index()], 0.5);
    assert!(!w.levers.impound);
    let who = adults(&w)[5];
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("granted");
    let base = assets::upkeep_for(&w, AssetKind::Car, 1);
    assert_eq!(assets::upkeep_of(&w, car), (base as f32 * 1.5).round() as i64, "taxed upkeep");

    // Brick: the lender's financed implants, at once.
    let arm = assets::grant(&mut w, who, AssetKind::Implant(Slot::Arms), 1).expect("granted");
    w.comp_mut::<Asset>(arm).expect("asset").finance =
        Some(Finance { lender: Some(corp), remaining: 1000, per_day: 10, arrears: 0 });
    w.push_command(PlayerCommand::Brick(corp));
    w.apply_commands();
    assert!(asset(&w, arm).bricked);
    assert_eq!(w.comp::<Kit>(who).expect("kit").fighting, 0.0, "the Kit is rebuilt");
    set_coins(&mut w, who, 0);
    assets::run(&mut w);
    assert!(asset(&w, arm).bricked, "a called loan stays bricked");

    // ChromeEveryone: Arms (kept where filled) and Nerves for every adult.
    w.push_command(PlayerCommand::ChromeEveryone { tier: 2 });
    w.apply_commands();
    for &a in adults(&w).iter().take(50) {
        let k = w.comp::<Kit>(a).expect("kit");
        assert!(k.chrome && k.reflex > 0.0, "{a:?} chromed");
    }
    assert_eq!(asset(&w, arm).tier, 1, "a filled slot is kept");

    // FloodStims: into the Hideouts of the gangs dealing or holding there.
    let g = w.gangs()[0];
    let h = w.hideout_of(g).expect("hideout");
    let d = w.district_of_building(h);
    let before = w.stock(h, Good::Stims);
    w.push_command(PlayerCommand::FloodStims { district: d, n: 50 });
    w.apply_commands();
    assert_eq!(w.stock(h, Good::Stims), (before + 50).min(w.goods_cap(h, Good::Stims)));
    w.push_command(PlayerCommand::FloodStims { district: DistrictId(200), n: 50 });
    w.apply_commands();
    assert_eq!(w.events.back().expect("event").kind, EventKind::PlayerActionFailed);

    // Chase: pinned until the next trip ends.
    w.push_command(PlayerCommand::Chase(who));
    w.apply_commands();
    assert!(w.chase_pins.contains(&who));
    w.check_indices().expect("indices in step");
}

// ---------------------------------------------------------------------------
// M13 review fixes
// ---------------------------------------------------------------------------

/// Review finding 1: a thief the robot kills in the detain fight is charged
/// with nothing: no report on the corpse, no witness, no victim memory.
#[test]
fn test_thief_killed_by_robot_is_not_charged() {
    use citysim::systems::{law, robots};
    let mut w = city();
    // Any fight the thief loses kills it.
    w.config.crime.fight_death_p = 1.0;
    let (owner, home) = homeowner(&w);
    let mut robot = assets::grant(&mut w, owner, AssetKind::Robot, 3).expect("a robot");
    let door = w.comp::<Building>(home).expect("home").door;
    let mut dead = None;
    for thief in others(&w, owner).into_iter().take(60) {
        if !w.has::<Asset>(robot) {
            robot = assets::grant(&mut w, owner, AssetKind::Robot, 3).expect("a robot");
        }
        assert_eq!(robots::powered_robot(&w, home), Some(robot));
        put_in(&mut w, thief, home);
        w.comp_mut::<citysim::Skills>(thief).expect("skills").stealth = 0.0;
        w.comp_mut::<citysim::Skills>(thief).expect("skills").fighting = 0.0;
        w.comp_mut::<citysim::Personality>(thief).expect("p").courage = 0.0;
        let witnessed = count(&w, EventKind::Witness);
        law::raise_crime(&mut w, thief, Some(owner), citysim::Crime::Theft, door);
        if !law::living(&w, thief) {
            assert_eq!(count(&w, EventKind::Witness), witnessed, "nobody witnesses a corpse's crime");
            dead = Some(thief);
            break;
        }
    }
    let thief = dead.expect("a T3 robot kills a tier-1 thief within 60 tries");
    assert!(!w.crime_reports().iter().any(|r| r.suspect == thief), "no report stands against the dead");
    assert!(!law::wanted(&w, thief));
    assert!(!w.has::<citysim::Sentence>(thief));
    let robbed = w.comp::<citysim::Memory>(owner).is_some_and(|m| {
        m.entries.iter().any(|e| e.kind == citysim::MemoryKind::WasRobbed && e.subject == Some(thief))
    });
    assert!(!robbed, "the raise ended at the death");
}

/// Review finding 5: the impound clears the keeper, and a used resale
/// clears the last life's keeper, theft, arrears and finance.
#[test]
fn test_impound_and_used_resale_reset_the_title() {
    let mut w = city();
    set_seeded_garages_aside(&mut w);
    let people = adults(&w);
    let (who, keeper, buyer) = (people[7], people[8], people[9]);
    let car = assets::grant(&mut w, who, AssetKind::Car, 1).expect("granted");
    assets::set_keeper(&mut w, car, Some(keeper));
    set_coins(&mut w, who, 0);
    let precinct = w.building_of_kind(BuildingKind::Jail).expect("precinct");
    for _ in 0..w.config.assets.impound_days {
        assets::run(&mut w);
    }
    let x = asset(&w, car);
    assert_eq!((x.owner, x.loc, x.keeper), (None, AssetLoc::Stock(precinct), None), "impounded, keeper cleared");
    assert_eq!(w.comp::<Kit>(keeper).expect("kit").vehicle, None);
    // Sold to a Garage, then resold used after a hard life in its stock.
    let (_, g) = garage(&mut w, 5000);
    assets::run(&mut w);
    assert_eq!(asset(&w, car).loc, AssetLoc::Stock(g), "sold to the Garage");
    {
        let m = w.comp_mut::<Asset>(car).expect("asset");
        m.stolen = true;
        m.upkeep_arrears = 4;
        m.away_days = 1;
        m.finance = Some(Finance { lender: None, remaining: 100, per_day: 5, arrears: 3 });
    }
    assets::set_keeper(&mut w, car, Some(keeper));
    set_coins(&mut w, buyer, 5000);
    let bought =
        assets::buy(&mut w, buyer, g, &ShopPick { kind: AssetKind::Car, tier: 1, used: Some(car) }).expect("bought");
    assert_eq!(bought, car);
    let x = asset(&w, car);
    assert_eq!((x.owner, x.keeper, x.stolen, x.upkeep_arrears, x.away_days), (Some(buyer), None, false, 0, 0));
    assert!(x.finance.is_none(), "paid in full: no plan of the last life's");
    assert_eq!(w.comp::<Kit>(buyer).expect("kit").vehicle, Some(car));
    assert_eq!(w.comp::<Kit>(keeper).expect("kit").vehicle, None);
    w.check_indices().expect("indices in step");
}
