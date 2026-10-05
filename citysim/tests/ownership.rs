//! M11 phase 2: owners, purses, rent and eviction (docs/M11_OWNERSHIP.md § 3-4,
//! § 8, § 11; plan phase 2 tests).

use citysim::systems::{economy, ownership};
use citysim::{
    ActionKind, Brain, Building, BuildingKind, Config, Corp, EntityId, EventKind, GoalKind, Household, Job, LifeKind,
    LocationKey, Lod, Market, PlanCtx, PlayerCommand, Position, Posture, Role, TilePos, Wallet, World, WorldState,
    TICKS_PER_DAY,
};

/// The v1 city with two corps: FoodCo (both Farms, the Market and, by
/// seeding rule 5, the Bar) and HomeCo (30 Blocks); rent [1, 2, 4].
fn cfg() -> Config {
    let mut c = Config::load().v1_profile();
    c.corps.names = vec!["FoodCo".into(), "HomeCo".into()];
    c.corps.niches = vec![vec!["Food".into()], vec!["Housing".into()]];
    c.corps.farms = vec![2, 0];
    c.corps.markets = vec![1, 0];
    c.corps.blocks = vec![0, 30];
    c.corps.offices = vec![0, 0];
    c.corps.treasury_initial = vec![2000, 2000];
    c.corps.bar_owner_count = 1;
    c.rent.base = [1, 2, 4];
    c
}

fn world() -> World {
    World::new(7, cfg())
}

/// `(FoodCo, HomeCo)`.
fn corps(w: &World) -> (EntityId, EntityId) {
    let c = w.corps();
    (c[0], c[1])
}

fn treasury(w: &World) -> i64 {
    w.purse(None)
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |w| w.coins)
}

fn set_coins(w: &mut World, a: EntityId, c: i64) {
    w.comp_mut::<Wallet>(a).expect("wallet").coins = c;
}

/// An unemployed adult with a Brain, not the excluded ones.
fn jobless_adult(w: &World, not: &[EntityId]) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            w.has::<Brain>(a)
                && !w.has::<Job>(a)
                && citysim::systems::demography::is_adult(w, a)
                && !not.contains(&a)
                && w.comp::<Position>(a).is_some()
        })
        .expect("a jobless adult")
}

/// Run the daily ownership pass at the next midnight (the clock jumps a day).
fn midnight(w: &mut World) {
    ownership::run(w);
    w.tick += TICKS_PER_DAY;
}

fn events(w: &World, kind: EventKind, who: EntityId) -> usize {
    w.events.iter().filter(|e| e.kind == kind && e.actors.first() == Some(&who)).count()
}

#[test]
fn test_purchase_pays_market_owner() {
    let mut w = world();
    let (food, _) = corps(&w);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    assert_eq!(w.owner_of(market), Some(food));
    let buyer = jobless_adult(&w, &[]);
    set_coins(&mut w, buyer, 100);
    w.comp_mut::<Market>(market).expect("m").price_food = 20;
    let (t0, f0, total) = (treasury(&w), w.purse(Some(food)), ownership::total_coins(&w));
    let paid = economy::pay_for_food(&mut w, buyer, Some(market), 3);
    assert_eq!(paid, 60);
    assert_eq!(coins(&w, buyer), 40);
    let tax = treasury(&w) - t0;
    assert_eq!(tax, 3, "5 % of 60 withheld into the Treasury");
    assert_eq!(w.purse(Some(food)) - f0, 60 - tax, "the Market's owner takes the rest");
    assert_eq!(ownership::total_coins(&w), total, "a purchase moves coins, never makes them");
    assert_eq!(w.stats.current.flow_food, 60);
    // A purchase whose stock is gone is refunded in full.
    w.comp_mut::<Building>(market).expect("m").stock_food = 0;
    assert!(!economy::take_food(&mut w, buyer, Some(market), 3, paid));
    assert_eq!(coins(&w, buyer), 100);
    assert_eq!(ownership::total_coins(&w), total);
}

#[test]
fn test_drink_pays_bar_owner() {
    let mut w = world();
    let (food, _) = corps(&w);
    let bar = w.building_of_kind(BuildingKind::Bar).expect("bar");
    assert_eq!(w.owner_of(bar), Some(food), "seeding rule 5: the lead Food corp takes the Bar");
    let drinker = jobless_adult(&w, &[]);
    set_coins(&mut w, drinker, 10);
    w.leave_building(drinker);
    w.enter_building(drinker, bar);
    let f0 = w.purse(Some(food));
    citysim::exec::actions::on_start(&mut w, drinker, ActionKind::Drink, None);
    assert_eq!(coins(&w, drinker), 8);
    assert_eq!(w.purse(Some(food)) - f0, 2, "2 coins to the Bar's corp (tax rounds down to 0)");
    // An agent owner earns the same way.
    let owner = jobless_adult(&w, &[drinker]);
    ownership::transfer_building(&mut w, bar, Some(owner));
    let o0 = coins(&w, owner);
    citysim::exec::actions::on_start(&mut w, drinker, ActionKind::Drink, None);
    assert_eq!(coins(&w, owner) - o0, 2);
    assert_eq!(coins(&w, drinker), 6);
    assert!(!w.comp::<Corp>(food).expect("corp").buildings.contains(&bar), "the corp's list follows the owner");
}

#[test]
fn test_negative_corp_leaves_worker_unpaid_and_they_quit() {
    let mut w = world();
    let (food, _) = corps(&w);
    let farmer = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .find(|&f| w.comp::<Job>(f).and_then(|j| j.employer).and_then(|e| w.owner_of(e)) == Some(food))
        .expect("a FoodCo farmer");
    w.comp_mut::<Corp>(food).expect("corp").treasury = -100;
    let c0 = coins(&w, farmer);
    let mut quit = false;
    for _ in 0..7 {
        if let Some(j) = w.comp_mut::<Job>(farmer) {
            j.days_unpaid += 1;
            j.last_wage_attempt_day = None;
        }
        economy::collect_wage(&mut w, farmer);
        if !w.has::<Job>(farmer) {
            quit = true;
            break;
        }
        w.tick += TICKS_PER_DAY;
    }
    assert!(quit, "a week unpaid by a broke corp ends the job");
    assert_eq!(events(&w, EventKind::Quit, farmer), 1);
    assert_eq!(coins(&w, farmer), c0, "nobody pays from a negative purse");
    assert_eq!(w.purse(Some(food)), -100);
}

/// Two HomeCo Blocks with one adult each: A broke, B with 50 coins. Everyone
/// else in them moves out penniless (so the re-housing pass cannot bring them back).
fn two_tenants(w: &mut World) -> (EntityId, EntityId, EntityId, EntityId) {
    let (_, home_co) = corps(w);
    let blocks = w.comp::<Corp>(home_co).expect("corp").buildings.clone();
    let (h1, h2) = (blocks[0], blocks[1]);
    let mut keep = Vec::new();
    for h in [h1, h2] {
        let rs = w.residents_of(h).to_vec();
        keep.push(rs[0]);
        for &r in &rs[1..] {
            set_coins(w, r, 0);
            w.set_home(r, None);
        }
    }
    set_coins(w, keep[0], 0);
    set_coins(w, keep[1], 50);
    (keep[0], h1, keep[1], h2)
}

#[test]
fn test_rent_moves_coins_and_short_rent_counts_arrears() {
    let mut w = world();
    let (_, home_co) = corps(&w);
    let (a, h1, b, h2) = two_tenants(&mut w);
    assert_eq!(w.comp::<Building>(h1).expect("b").tier, 1);
    midnight(&mut w); // accrues 2 each (one adult per Block)
    assert_eq!(w.comp::<Household>(a).expect("h").arrears, 0, "nothing was due yet");
    assert_eq!(w.comp::<Building>(h1).expect("b").rent_per_day, 2);
    let hc0 = w.purse(Some(home_co));
    midnight(&mut w);
    assert_eq!(w.comp::<Household>(a).expect("h").arrears, 1, "the due rent went unpaid");
    assert_eq!(events(&w, EventKind::RentShort, a), 1);
    assert_eq!(coins(&w, b), 48, "B paid the 2 due");
    assert_eq!(w.comp::<Household>(b).expect("h").arrears, 0);
    assert_eq!(w.comp::<Building>(h2).expect("b").revenue.back(), Some(&2), "rent reaches the landlord's Block");
    assert!(w.purse(Some(home_co)) != hc0, "HomeCo's purse moved");
    midnight(&mut w);
    assert_eq!(w.comp::<Household>(a).expect("h").arrears, 2);
    assert_eq!(events(&w, EventKind::RentShort, a), 1, "RentShort only on the first short day of a run");
}

#[test]
fn test_eviction_frees_slot_and_evictee_rehouses_when_able() {
    let mut w = world();
    let (_, home_co) = corps(&w);
    let (a, h1, _, _) = two_tenants(&mut w);
    let evict_days = w.config.rent.evict_days;
    for _ in 0..=evict_days {
        midnight(&mut w);
    }
    assert_eq!(events(&w, EventKind::Evicted, a), 1);
    assert_eq!(w.comp::<Household>(a).expect("h").home, None);
    assert!(w.residents_of(h1).is_empty(), "the slot is free");
    assert!(w.comp::<citysim::Life>(a).expect("life").events.iter().any(|e| e.kind == LifeKind::Evicted));
    assert!(w.stats.current.evictions >= 1 || w.eviction_log.len() == 1);
    // Broke, they stay out; with coins they move in at the next midnight,
    // and not with the landlord who evicted them.
    midnight(&mut w);
    assert_eq!(w.comp::<Household>(a).expect("h").home, None);
    set_coins(&mut w, a, 20);
    midnight(&mut w);
    let home = w.comp::<Household>(a).expect("h").home.expect("re-housed");
    assert_eq!(events(&w, EventKind::Housed, a), 1);
    assert_ne!(w.owner_of(home), Some(home_co), "HomeCo refuses its evictee for 30 days");
}

#[test]
fn test_spouse_and_children_evicted_together() {
    let mut w = world();
    let (_, home_co) = corps(&w);
    let h = w.comp::<Corp>(home_co).expect("corp").buildings[2];
    let rs = w.residents_of(h).to_vec();
    let (a, b) = (rs[0], rs[1]);
    if w.spouse_of(a) != Some(b) {
        w.set_spouse(a, b);
    }
    let kid = citysim::systems::demography::spawn_child(&mut w, a, b, h);
    let stay = rs[2];
    ownership::evict(&mut w, a, "test");
    for who in [a, b, kid] {
        assert_eq!(w.comp::<Household>(who).expect("h").home, None, "{who:?} went with the evictee");
    }
    assert_eq!(w.comp::<Household>(stay).expect("h").home, Some(h), "a housemate stays");
    assert_eq!(events(&w, EventKind::Evicted, a) + events(&w, EventKind::Evicted, b), 2, "one Evicted each");
    assert_eq!(events(&w, EventKind::Evicted, kid), 0);
}

#[test]
fn test_wage_collected_at_workplace_for_corp_job() {
    let mut w = world();
    let (food, _) = corps(&w);
    w.tick = 1200; // 20:00: every shift is over
    let farmer = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .find(|&f| w.comp::<Job>(f).and_then(|j| j.employer).and_then(|e| w.owner_of(e)) == Some(food))
        .expect("a FoodCo farmer");
    let guard = w.guards()[0];
    let farm = w.comp::<Job>(farmer).and_then(|j| j.employer).expect("farm");
    assert_eq!(w.wage_desk(farmer), Some(farm));
    assert_eq!(w.wage_desk(guard), w.building_of_kind(BuildingKind::Hall), "the Precinct is the city's");
    let street = w.edge_roads[0];
    for who in [farmer, guard] {
        let tick = w.tick;
        if let Some(j) = w.comp_mut::<Job>(who) {
            j.days_unpaid = 1;
            j.last_wage_attempt_day = None;
            j.last_shift_day = Some(j.shift_key_at(tick));
        }
        w.leave_building(who);
        w.comp_mut::<Position>(who).expect("pos").tile = street;
    }
    let plan = |w: &World, who: EntityId| -> Vec<ActionKind> {
        let ctx = PlanCtx::build(w, who, None);
        let start = WorldState::observe(w, who, None);
        let goal = citysim::goap::goal_state(GoalKind::Work).expect("goal");
        let limits = citysim::goap::Limits { max_expansions: 400, max_len: 6 };
        citysim::goap::planner::plan(&ctx, start, &goal, limits).expect("a plan").steps
    };
    assert_eq!(PlanCtx::build(&w, farmer, None).wage_at, LocationKey::Farm);
    assert_eq!(PlanCtx::build(&w, guard, None).wage_at, LocationKey::Hall);
    let fp = plan(&w, farmer);
    assert_eq!(fp, vec![ActionKind::GoTo(LocationKey::Farm), ActionKind::CollectWage], "farmer: {fp:?}");
    let gp = plan(&w, guard);
    assert_eq!(gp, vec![ActionKind::GoTo(LocationKey::Hall), ActionKind::CollectWage], "guard: {gp:?}");
    // At the Farm the wage is paid there, by FoodCo.
    w.enter_building(farmer, farm);
    assert!(citysim::exec::actions::can_start(&w, farmer, ActionKind::CollectWage, None));
    let f0 = w.purse(Some(food));
    economy::collect_wage(&mut w, farmer);
    assert!(w.purse(Some(food)) < f0, "FoodCo paid the wage");
}

#[test]
fn test_seeding_matches_table() {
    let w = World::new(42, Config::load());
    let corps = w.corps();
    assert_eq!(corps.len(), 8);
    let name = |c: EntityId| w.comp::<Corp>(c).expect("corp").name.clone();
    let names: Vec<String> = corps.iter().map(|&c| name(c)).collect();
    assert_eq!(names, ["Nutrix", "Vatra", "Greenline", "Habitat", "Stackwell", "Kessler", "Arasaka", "Militech"]);
    let owned = |c: Option<EntityId>, k: BuildingKind| ownership::owned_of_kind(&w, c, k);
    let at = |b: EntityId| w.comp::<Building>(b).map(|b| (b.rect.x, b.rect.y)).expect("b");
    let [nutrix, vatra, greenline, habitat, stackwell, kessler, arasaka, militech] =
        <[EntityId; 8]>::try_from(corps.clone()).expect("eight");
    assert_eq!(owned(Some(nutrix), BuildingKind::Farm).len(), 6);
    assert_eq!(owned(Some(vatra), BuildingKind::Farm).len(), 4);
    assert_eq!(owned(Some(greenline), BuildingKind::Farm).len(), 2);
    assert_eq!(owned(Some(nutrix), BuildingKind::Market).iter().map(|&m| at(m)).collect::<Vec<_>>(), [(96, 57)]);
    assert_eq!(owned(Some(vatra), BuildingKind::Market).iter().map(|&m| at(m)).collect::<Vec<_>>(), [(128, 89)]);
    assert_eq!(owned(Some(greenline), BuildingKind::Market).iter().map(|&m| at(m)).collect::<Vec<_>>(), [(16, 57)]);
    assert_eq!(owned(Some(nutrix), BuildingKind::Bar).iter().map(|&b| at(b)).collect::<Vec<_>>(), [(144, 57)]);
    let tiers = |c: Option<EntityId>| {
        let mut t = [0; 3];
        for h in owned(c, BuildingKind::Home) {
            t[usize::from(w.comp::<Building>(h).expect("h").tier)] += 1;
        }
        t
    };
    assert_eq!(tiers(Some(habitat)), [0, 60, 60], "all 60 Spire and the first 60 Mid");
    assert_eq!(tiers(Some(vatra)), [0, 60, 0]);
    assert_eq!(tiers(Some(stackwell)), [40, 20, 0]);
    assert_eq!(tiers(Some(militech)), [40, 0, 0]);
    assert_eq!(tiers(Some(kessler)), [20, 0, 0]);
    assert_eq!(tiers(None), [100, 0, 0], "the city keeps 100 Sump Blocks");
    assert_eq!(
        owned(Some(arasaka), BuildingKind::SecurityOffice).iter().map(|&o| at(o)).collect::<Vec<_>>(),
        [(240, 49)]
    );
    assert_eq!(
        owned(Some(militech), BuildingKind::SecurityOffice).iter().map(|&o| at(o)).collect::<Vec<_>>(),
        [(208, 65)]
    );
    let agent_bars: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .filter_map(|&b| w.owner_of(b))
        .filter(|&o| w.has::<Wallet>(o))
        .collect();
    assert_eq!(agent_bars.len(), 2, "two Bars belong to agents");
    for o in agent_bars {
        assert_eq!(coins(&w, o), 200);
        assert!(!w.has::<Job>(o));
    }
    for g in w.gangs() {
        let h = w.hideout_of(g).expect("hideout");
        assert_eq!(w.owner_of(h), Some(g), "a Hideout belongs to its gang");
    }
    for (i, &c) in corps.iter().enumerate() {
        let cc = w.comp::<Corp>(c).expect("corp");
        assert_eq!(cc.slot, Some(i as u8));
        assert!(cc.exec.is_some(), "{} has an exec", cc.name);
        assert_eq!(cc.order, citysim::CorpOrder::Hunker);
        assert_eq!(cc.governance, citysim::Governance::Dictator);
        assert_eq!(cc.parent.is_some(), i >= 6, "Arasaka and Militech are branches");
        // M11 review: an outside parent's id is its own namespace, not the slot.
        if let Some(p) = cc.parent {
            assert!(p >= citysim::OUTSIDE_PARENT_BASE, "parent {p} reads as a slot");
        }
    }
    assert_eq!(w.vacancies.get(&owned(Some(arasaka), BuildingKind::SecurityOffice)[0]).map(Vec::len), Some(6));
}

#[test]
fn test_daily_ownership_pass_conserves_coins() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(3 * TICKS_PER_DAY);
    assert_eq!(w.tick_of_day(), 0);
    let before = ownership::total_coins(&w);
    let rent0 = w.stats.current.rent_paid;
    let upkeep0 = w.stats.current.flow_upkeep;
    ownership::run(&mut w);
    assert!(w.stats.current.rent_paid > rent0, "rent moved");
    assert!(w.stats.current.flow_upkeep > upkeep0, "upkeep moved");
    assert_eq!(ownership::total_coins(&w), before, "wallets + gangs + corps + Treasury unchanged");
}

#[test]
fn test_restock_capped_by_owner_purse() {
    let mut w = world();
    let (food, _) = corps(&w);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    let wholesale = w.config.corps.wholesale;
    w.comp_mut::<Building>(market).expect("m").stock_food = 0;
    // The restock reads the closing balance (phase 5): the treasury before
    // tonight's upkeep, which `ownership::run` records.
    let c = w.comp_mut::<Corp>(food).expect("corp");
    c.treasury = 50 * wholesale;
    c.closing = 50 * wholesale;
    let t0 = treasury(&w);
    economy::run(&mut w); // tick 0: restock, price, spoilage
    assert_eq!(w.comp::<Building>(market).expect("m").stock_food, 50, "50 units is all FoodCo could pay for");
    assert_eq!(w.purse(Some(food)), 0);
    assert_eq!(treasury(&w) - t0, 50 * wholesale, "the Reserve is the city's to sell");
}

#[test]
fn test_cheaper_market_wins_customers_across_tiers() {
    let mut w = World::new(42, Config::load());
    let markets = w.buildings_of_kind(BuildingKind::Market).to_vec();
    let door = |w: &World, m: EntityId| w.comp::<Building>(m).expect("m").door;
    let (a, b, c) = (markets[0], markets[1], markets[2]);
    // A street tile where B is farther than A by less than 24 tiles and C is far off.
    let (da, db, dc) = (door(&w, a), door(&w, b), door(&w, c));
    let mut spot = None;
    'find: for y in 0..w.map.h().min(256) {
        for x in 0..w.map.w().min(256) {
            let t = TilePos::new(x as u8, y as u8);
            if w.map.tile_at(t) != citysim::TileKind::Road {
                continue;
            }
            let (ma, mb, mc) = (t.manhattan(da), t.manhattan(db), t.manhattan(dc));
            if mb > ma && mb - ma <= 20 && mc > mb + 40 {
                spot = Some(t);
                break 'find;
            }
        }
    }
    let spot = spot.expect("a tile between two Markets");
    let shopper = jobless_adult(&w, &[]);
    for lod in [Lod::Coarse, Lod::Statistical] {
        citysim::systems::lod::set_lod(&mut w, shopper, lod);
        w.leave_building(shopper);
        w.comp_mut::<Position>(shopper).expect("pos").tile = spot;
        for m in [a, b, c] {
            w.comp_mut::<Market>(m).expect("m").price_food = 5;
        }
        assert_eq!(w.local(shopper, BuildingKind::Market), Some(a), "{lod:?}: equal prices, the nearer");
        w.comp_mut::<Market>(b).expect("m").price_food = 3;
        assert_eq!(w.local(shopper, BuildingKind::Market), Some(b), "{lod:?}: 2 coins is worth 24 tiles");
    }
}

#[test]
fn test_upkeep_can_push_corp_negative_and_sets_negative_since() {
    let mut w = world();
    let (food, _) = corps(&w);
    w.comp_mut::<Corp>(food).expect("corp").treasury = 10;
    let t0 = treasury(&w);
    ownership::run(&mut w);
    let c = w.comp::<Corp>(food).expect("corp");
    assert!(c.treasury < 0, "upkeep is charged in full: {}", c.treasury);
    assert_eq!(c.closing, 10, "the books closed before the upkeep");
    assert_eq!(c.negative_since, None, "solvent at the close: the upkeep lump is no day in the red");
    assert!(treasury(&w) > t0);
    // In the red at the close: the clock starts.
    w.comp_mut::<Corp>(food).expect("corp").treasury = -10;
    w.tick += TICKS_PER_DAY;
    ownership::run(&mut w);
    assert_eq!(w.comp::<Corp>(food).expect("corp").negative_since, Some(TICKS_PER_DAY));
    w.comp_mut::<Corp>(food).expect("corp").treasury = 100_000;
    w.tick += TICKS_PER_DAY;
    ownership::run(&mut w);
    assert_eq!(w.comp::<Corp>(food).expect("corp").negative_since, None, "cleared on recovery");
}

#[test]
fn test_nationalise_and_subsidise_levers() {
    let mut w = world();
    let (food, _) = corps(&w);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    let value = ownership::value(&w, BuildingKind::Market);
    let (t0, f0) = (treasury(&w), w.purse(Some(food)));
    w.push_command(PlayerCommand::Nationalise(market));
    w.apply_commands();
    assert_eq!(w.owner_of(market), None);
    assert_eq!(t0 - treasury(&w), value);
    assert_eq!(w.purse(Some(food)) - f0, value);
    // Refused: the Treasury cannot pay; the Farm stays FoodCo's.
    let farm = w.building_of_kind(BuildingKind::Farm).expect("farm");
    w.push_command(PlayerCommand::SetTreasury(10));
    w.push_command(PlayerCommand::Nationalise(farm));
    w.push_command(PlayerCommand::Subsidise { corp: food, amount: 500 });
    w.apply_commands();
    assert_eq!(w.owner_of(farm), Some(food));
    assert_eq!(w.events.iter().filter(|e| e.kind == EventKind::PlayerActionFailed).count(), 2);
    let f1 = w.purse(Some(food));
    w.push_command(PlayerCommand::SetTreasury(1000));
    w.push_command(PlayerCommand::Subsidise { corp: food, amount: 500 });
    w.apply_commands();
    assert_eq!(w.purse(Some(food)) - f1, 500);
    assert_eq!(treasury(&w), 500);
    // City rent, the rent cap, no city evictions.
    w.push_command(PlayerCommand::SetCityRent([0, 50, 3]));
    w.push_command(PlayerCommand::SetRentCap(Some(1)));
    w.push_command(PlayerCommand::NoCityEvictions(true));
    w.apply_commands();
    assert_eq!(w.levers.city_rent, [0, 20, 3]);
    assert_eq!(w.levers.rent_cap, Some(1));
    assert!(w.levers.no_city_evictions);
    let city_home = ownership::owned_of_kind(&w, None, BuildingKind::Home)[0];
    assert_eq!(ownership::rent_for(&w, city_home), 1, "the cap holds the city's 20");
    // DemolishHome refuses a corp's Block.
    let (_, home_co) = corps(&w);
    let block = w.comp::<Corp>(home_co).expect("corp").buildings[0];
    w.push_command(PlayerCommand::DemolishHome(block));
    w.apply_commands();
    assert!(!w.comp::<Building>(block).expect("b").demolished);
}

#[test]
fn test_private_guard_never_takes_jail_duty() {
    let mut w = World::new(42, Config::load());
    w.tick(); // tick 0: job search fills the Security Offices
    let private: Vec<EntityId> =
        w.guards().iter().copied().filter(|&g| citysim::systems::law::is_private_guard(&w, g)).collect();
    assert!(private.len() >= 6, "{} private guards", private.len());
    let roster = citysim::systems::law_brain::guards(&w);
    assert!(private.iter().all(|g| !roster.contains(g)), "the city's roster is the Precinct's");
    w.law_mut().expect("law").posture = Posture::Garrison;
    for &g in &private {
        for key in 0..3 {
            assert!(!citysim::systems::law::jail_duty(&w, g, key), "Garrison holds the Jail with city guards only");
        }
    }
    assert!(citysim::systems::law::jail_duty(&w, roster[0], 0));
    // Their loop is their corp's own buildings, office first.
    let g = private[0];
    let office = w.comp::<Job>(g).and_then(|j| j.employer).expect("office");
    let corp = w.corp_of_building(office).expect("corp");
    let route = citysim::systems::law::new_patrol_route(&mut w, g);
    assert_eq!(route.first(), Some(&office));
    assert!(route.iter().all(|&b| w.owner_of(b) == Some(corp)));
    assert_eq!(citysim::systems::law::pursuit_radius_for(&w, g), w.config.corps.private_pursuit_radius);
}

#[test]
fn test_owner_death_passes_buildings_to_heir() {
    let mut w = World::new(42, Config::load());
    let bars: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .filter(|&b| w.owner_of(b).is_some_and(|o| w.has::<Wallet>(o)))
        .collect();
    let (b1, b2) = (bars[0], bars[1]);
    let (o1, o2) = (w.owner_of(b1).expect("o"), w.owner_of(b2).expect("o"));
    // The first owner's spouse inherits; the second has none left, so the city does.
    let spouse = jobless_adult(&w, &[o1, o2]);
    if let Some(s) = w.spouse_of(o1) {
        let _ = s;
    } else {
        w.set_spouse(o1, spouse);
    }
    let heir = w.spouse_of(o1).expect("spouse");
    w.kill_by(o1, citysim::DeathCause::OldAge, None);
    assert_eq!(w.owner_of(b1), Some(heir));
    if let Some(s) = w.spouse_of(o2) {
        w.kill_by(s, citysim::DeathCause::OldAge, None);
    }
    for c in citysim::systems::demography::children_of_agent(&w, o2) {
        w.kill_by(c, citysim::DeathCause::OldAge, None);
    }
    w.remove_agent(o2); // emigration
    assert_eq!(w.owner_of(b2), None, "nobody left: the city");
    assert_eq!(w.events.iter().filter(|e| e.kind == EventKind::Acquired).count(), 2);
}

// ---------------------------------------------------------------------------
// M11 phase 5 fix pass (findings 2, 4-9, 29)
// ---------------------------------------------------------------------------

#[test]
fn test_zero_rent_and_lost_home_clear_arrears() {
    let mut w = world();
    let (a, _, _, _) = two_tenants(&mut w);
    midnight(&mut w);
    midnight(&mut w);
    assert_eq!(w.comp::<Household>(a).expect("h").arrears, 1);
    // A rent cap of 0: what was owed is forgiven, the arrears end.
    w.levers.rent_cap = Some(0);
    midnight(&mut w);
    let h = w.comp::<Household>(a).expect("h");
    assert_eq!((h.arrears, h.rent_due), (0, 0.0), "rent 0 forgives the debt");
    // A Home lost any way (DemolishHome) ends its rent and arrears.
    w.levers.rent_cap = None;
    w.comp_mut::<Household>(a).expect("h").arrears = 3;
    w.comp_mut::<Household>(a).expect("h").rent_due = 4.0;
    w.set_home(a, None);
    let h = w.comp::<Household>(a).expect("h");
    assert_eq!((h.arrears, h.rent_due), (0, 0.0), "a homeless adult owes no rent and can be re-housed");
}

#[test]
fn test_owner_in_own_home_pays_no_rent() {
    let mut w = world();
    let (a, h1, _, _) = two_tenants(&mut w);
    ownership::transfer_building(&mut w, h1, Some(a));
    for _ in 0..4 {
        midnight(&mut w);
    }
    let h = w.comp::<Household>(a).expect("h");
    assert_eq!((h.arrears, h.rent_due), (0, 0.0), "no rent to oneself, no arrears, never evicted");
    assert_eq!(events(&w, EventKind::RentShort, a), 0);
    assert_eq!(w.comp::<Household>(a).expect("h").home, Some(h1));
}

#[test]
fn test_wage_and_tax_capped_together_by_corp_purse() {
    let mut w = world();
    let (food, _) = corps(&w);
    let farmer = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .find(|&f| w.comp::<Job>(f).and_then(|j| j.employer).and_then(|e| w.owner_of(e)) == Some(food))
        .expect("a FoodCo farmer");
    w.levers.tax_rate = 0.5;
    w.comp_mut::<Corp>(food).expect("corp").treasury = 5;
    let j = w.comp_mut::<Job>(farmer).expect("job");
    j.wage_per_day = 6;
    j.days_unpaid = 1;
    j.tax_accum = 0.0;
    let (c0, t0) = (coins(&w, farmer), treasury(&w));
    economy::collect_wage(&mut w, farmer);
    // 5 coins cover 5 of the 6 gross: tax 2.5 -> 2 to the Treasury, 3 net.
    assert_eq!(w.purse(Some(food)), 0, "the corp paid what it had");
    assert_eq!(coins(&w, farmer) - c0, 3);
    assert_eq!(treasury(&w) - t0, 2, "the Treasury got the tax on what was paid, not shorted");
    let j = w.comp::<Job>(farmer).expect("job");
    assert!((j.tax_accum - 0.5).abs() < 1e-6, "only the paid gross accrues tax: {}", j.tax_accum);
    assert_eq!(j.days_unpaid, 1, "the unpaid coin is a day still owed");
}

#[test]
fn test_refund_reverses_the_tax_exactly() {
    let mut w = world();
    let (food, _) = corps(&w);
    let buyer = jobless_adult(&w, &[]);
    set_coins(&mut w, buyer, 30);
    w.tax_accum.insert(food, 0.9);
    let (b0, f0, t0) = (coins(&w, buyer), w.purse(Some(food)), treasury(&w));
    let paid = ownership::pay(&mut w, Some(buyer), Some(food), 3, ownership::Flow::Food);
    assert_eq!(treasury(&w) - t0, 1, "0.9 + 0.15 withheld a whole coin");
    ownership::refund(&mut w, buyer, Some(food), paid, ownership::Flow::Food);
    assert_eq!((coins(&w, buyer), w.purse(Some(food)), treasury(&w)), (b0, f0, t0), "everyone where they started");
    assert!((w.tax_accum[&food] - 0.9).abs() < 1e-5);
}

#[test]
fn test_rehouse_uses_each_home_capacity() {
    let mut w = world();
    let (a, _, _, _) = two_tenants(&mut w);
    w.set_home(a, None);
    set_coins(&mut w, a, 1000);
    let homes = w.buildings_of_kind(BuildingKind::Home).to_vec();
    for &h in &homes {
        let n = w.residents_of(h).len() as u8;
        w.comp_mut::<Building>(h).expect("b").capacity = n;
    }
    midnight(&mut w);
    assert_eq!(w.comp::<Household>(a).expect("h").home, None, "every Home is full at its own capacity");
    let roomy = homes[5];
    let n = w.residents_of(roomy).len() as u8;
    w.comp_mut::<Building>(roomy).expect("b").capacity = n + 4;
    midnight(&mut w);
    assert_eq!(w.comp::<Household>(a).expect("h").home, Some(roomy));
}

#[test]
fn test_tax_remainder_of_gone_payee_dropped() {
    let mut w = world();
    let ghost = w.spawn();
    w.tax_accum.insert(ghost, 0.7);
    let (food, _) = corps(&w);
    w.tax_accum.insert(food, 0.3);
    midnight(&mut w);
    assert!(!w.tax_accum.contains_key(&ghost), "a despawned payee leaves the save");
    assert!(w.tax_accum.contains_key(&food));
}

#[test]
fn test_full_day_of_flows_conserves_coins() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(3 * TICKS_PER_DAY);
    assert_eq!(w.tick_of_day(), 0);
    let mut before = ownership::total_coins(&w);
    // Midnight: rent, upkeep (closing balance), exec wages; restock and price.
    ownership::run(&mut w);
    economy::run(&mut w);
    // A purchase at every Market (with tenths), the first refunded.
    let markets = w.buildings_of_kind(BuildingKind::Market).to_vec();
    let mut shoppers = Vec::new();
    for &m in &markets {
        let a = jobless_adult(&w, &shoppers);
        shoppers.push(a);
        before += 50 - coins(&w, a);
        set_coins(&mut w, a, 50);
        let paid = economy::pay_for_food(&mut w, a, Some(m), 3);
        assert!(paid > 0);
        if m == markets[0] {
            economy::refund_food(&mut w, a, Some(m), paid);
        } else {
            assert!(economy::take_food(&mut w, a, Some(m), 3, paid));
        }
    }
    // A drink, a wage with tax, a haul that overflows into the Reserve.
    if let Some(bar) = w.buildings_of_kind(BuildingKind::Bar).first().copied() {
        let d = jobless_adult(&w, &shoppers);
        before += 10 - coins(&w, d);
        set_coins(&mut w, d, 10);
        w.leave_building(d);
        w.enter_building(d, bar);
        citysim::exec::actions::on_start(&mut w, d, ActionKind::Drink, None);
    }
    let worker = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .find(|&f| w.comp::<Job>(f).and_then(|j| j.employer).and_then(|e| w.owner_of(e)).is_some())
        .expect("a corp farmer");
    w.comp_mut::<Job>(worker).expect("job").days_unpaid = 3;
    economy::collect_wage(&mut w, worker);
    let farm = w.comp::<Job>(worker).and_then(|j| j.employer).expect("farm");
    w.comp_mut::<Building>(farm).expect("farm").stock_food = 400;
    for m in &markets {
        w.comp_mut::<Building>(*m).expect("m").stock_food = 2000;
    }
    let overflow0 = w.stats.current.flow_overflow;
    assert!(economy::haul(&mut w, farm) > 0);
    assert!(w.stats.current.flow_overflow > overflow0, "the Market was full: the Reserve bought the haul");
    assert_eq!(ownership::total_coins(&w), before, "wallets + gangs + corps + Treasury unchanged");
}

#[test]
fn test_evictee_waits_on_the_street() {
    let mut w = world();
    w.config.rent.rehouse_wait_days = 3;
    let (a, _, _, _) = two_tenants(&mut w);
    let evict_days = w.config.rent.evict_days;
    for _ in 0..=evict_days {
        midnight(&mut w);
    }
    assert_eq!(events(&w, EventKind::Evicted, a), 1);
    set_coins(&mut w, a, 1000);
    for night in 0..2 {
        midnight(&mut w);
        assert_eq!(w.comp::<Household>(a).expect("h").home, None, "night {night} on the street");
        assert_ne!(citysim::systems::classes::class_of(&w, a), citysim::Class::Street, "homeless: a Dreg (or Corp)");
    }
    midnight(&mut w);
    assert!(w.comp::<Household>(a).expect("h").home.is_some(), "re-housed once the wait is over");
}

// ---------------------------------------------------------------------------
// M11 review regressions
// ---------------------------------------------------------------------------

/// Item 1: a Security Office in the city's hands posts no guards and lets
/// its own go (they were paid by the Treasury, held no Jail and patrolled
/// their own door); one in an agent's hands likewise.
#[test]
fn test_office_without_a_corp_has_no_guards() {
    let mut w = World::new(42, Config::load());
    w.tick(); // tick 0: job search fills the Security Offices
    let offices: Vec<EntityId> = w.buildings_of_kind(BuildingKind::SecurityOffice).to_vec();
    let staff = |w: &World, o: EntityId| ownership::staff_at(w, o);
    assert!(offices.len() >= 2 && offices.iter().all(|&o| !staff(&w, o).is_empty()), "every Office staffed");
    let (to_city, to_agent) = (offices[0], offices[1]);
    let guards = staff(&w, to_city);
    let roster = citysim::systems::law_brain::guards(&w).len();
    w.push_command(PlayerCommand::SetTreasury(1_000_000));
    w.push_command(PlayerCommand::Nationalise(to_city));
    w.apply_commands();
    assert_eq!(w.owner_of(to_city), None);
    let agent = jobless_adult(&w, &[]);
    citysim::systems::corps::move_building(&mut w, to_agent, Some(agent));
    for o in [to_city, to_agent] {
        assert!(staff(&w, o).is_empty(), "no guard stays at an Office no corp owns");
        assert!(!w.vacancies.contains_key(&o), "and none is posted");
    }
    for &g in &guards {
        assert!(!w.has::<Job>(g), "laid off");
        assert!(!citysim::systems::law::is_private_guard(&w, g));
    }
    assert_eq!(citysim::systems::law_brain::guards(&w).len(), roster, "the city's payroll is unchanged");
    // A day later nobody has been hired there.
    w.run_ticks(TICKS_PER_DAY);
    assert!(staff(&w, to_city).is_empty() && staff(&w, to_agent).is_empty());
}

/// Item 3: a corp's replacement exec is never another corp's exec.
#[test]
fn test_replacement_exec_is_not_another_corps_exec() {
    let mut w = world();
    let (food, home_co) = corps(&w);
    let farm = w.building_of_kind(BuildingKind::Farm).expect("farm");
    let staff = ownership::staff_at(&w, farm);
    assert!(staff.len() >= 2, "two farmers");
    let (taken, next) = (staff[0], staff[1]);
    for (a, g) in [(taken, 1.0), (next, 0.99)] {
        w.comp_mut::<citysim::Personality>(a).expect("p").greed = g;
    }
    for &a in &staff[2..] {
        w.comp_mut::<citysim::Personality>(a).expect("p").greed = 0.0;
    }
    w.comp_mut::<Corp>(home_co).expect("c").exec = Some(taken);
    w.comp_mut::<Corp>(food).expect("c").exec = None;
    midnight(&mut w);
    let exec = w.comp::<Corp>(food).expect("c").exec;
    assert_eq!(exec, Some(next), "the greediest employee who is not HomeCo's exec");
}

/// Item 5: an owner living in their own Block is not one of the adults its
/// rent is split over: the tenant owes the whole rent.
#[test]
fn test_owner_resident_rent_split_over_tenants() {
    let mut w = world();
    let (a, h1, b, _) = two_tenants(&mut w);
    set_coins(&mut w, a, 0);
    let owner = jobless_adult(&w, &[a, b]);
    ownership::transfer_building(&mut w, h1, Some(owner));
    w.set_home(owner, Some(h1));
    midnight(&mut w);
    let rent = w.comp::<Building>(h1).expect("b").rent_per_day;
    assert!(rent > 0);
    assert_eq!(w.comp::<Household>(a).expect("h").rent_due, rent as f32, "the tenant owes the whole rent");
    assert_eq!(w.comp::<Household>(owner).expect("h").rent_due, 0.0);
    // Paid in full the next night: the Block yields its whole rent.
    set_coins(&mut w, a, 100);
    midnight(&mut w);
    assert_eq!(coins(&w, a), 100 - rent);
    assert_eq!(w.comp::<Building>(h1).expect("b").revenue.back(), Some(&rent), "the whole rent reaches the owner");
}

/// Item 7: "paid N in 7 days" is a seven-day window, not a decaying figure.
#[test]
fn test_rent_paid_is_a_seven_day_window() {
    let mut h = Household::new(None);
    for day in 0..10u64 {
        h.note_rent_paid(day, 2);
    }
    assert_eq!(h.rent_paid_7d(9), 14, "days 3-9");
    assert_eq!(h.rent_paid_7d(16), 0, "nothing paid in the last week");
    assert!(h.rent_paid_log.len() <= 7);
}
