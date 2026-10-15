//! Jobs & lives phase 1 (docs/JOBS_AND_LIVES.md §§ 1-2, § 12's unit tests):
//! the master switch, the five `TradeModel`s that had no code, the
//! industries, the civic three, the Hq and the foreman. Each test is the
//! shipped seed-42 city (the switch on, as shipped) and one mechanism,
//! driven by calling the system directly (most of these fire rarely or
//! need a set-up a run reaches only by chance).

use citysim::outside::ExportGood;
use citysim::systems::{econ, industry, jobs, law, ownership, world_market};
use citysim::{Building, BuildingKind, Config, EntityId, EventKind, Good, Job, Role, World};

fn on_world() -> World {
    let mut c = Config::load();
    c.jobs2.enabled = true;
    // Phase 1's mechanisms as written (the street's couriers move the
    // seed's opening staffing and the housing pre-roll's Lots; phase 3's
    // Gunsmith takes a Lot; phase 5's BdStudio takes the Lot near the Yard).
    c.jobs2.street = false;
    c.weapons.enabled = false;
    c.vice.bd = false;
    World::new(42, c)
}

fn trade(w: &World, key: &str) -> Role {
    w.config.trade_role(key).unwrap_or_else(|| panic!("trade {key} configured"))
}

fn first(w: &World, kind: BuildingKind) -> EntityId {
    *w.buildings_of_kind(kind).first().unwrap_or_else(|| panic!("a seeded {kind}"))
}

fn j2(w: &World) -> citysim::stats::Jobs2Cols {
    w.stats.current.jobs2.clone().unwrap_or_default()
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

/// A jobless free adult (lowest id).
fn jobless(w: &World) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            law::living(w, a)
                && citysim::systems::demography::is_adult(w, a)
                && !w.has::<Job>(a)
                && !w.has::<citysim::Sentence>(a)
                && w.comp::<citysim::Brain>(a).is_some_and(|b| !b.emigrating)
        })
        .expect("a jobless adult")
}

/// § 1: the switch off seeds no new kind, keeps no state, makes no new
/// book and prints no new column; on, each industry and the megacorps'
/// Hqs stand, staffed, with the books for the appended goods.
#[test]
fn test_switch_off_is_the_old_city_on_seeds_the_industries() {
    let mut c = Config::load();
    c.jobs2.enabled = false;
    let off = World::new(42, c);
    for k in BuildingKind::JOBS2 {
        assert!(off.buildings_of_kind(k).is_empty(), "{k} stands with the switch off");
    }
    assert!(off.jobs2.is_default());
    assert!(world_market::book(&off, ExportGood::Rations).is_none());
    assert!(!citysim::systems::stats::csv_header(&off).contains("flow_handling"));
    let civic = trade(&off, "jailer");
    assert!(ownership::roles_for(&off, BuildingKind::Jail).iter().all(|&(r, _)| r != civic), "a jobs2 row is live off");

    let on = on_world();
    for k in [
        BuildingKind::PackingPlant,
        BuildingKind::FreightYard,
        BuildingKind::UtilityPlant,
        BuildingKind::ConstructionYard,
    ] {
        assert!(!on.buildings_of_kind(k).is_empty(), "no seeded {k}");
    }
    let hqs = on.buildings_of_kind(BuildingKind::Hq).len();
    let megas = on.config.corps.megacorp.iter().filter(|&&m| m).count();
    assert!(hqs >= megas, "an Hq per megacorp copy ({hqs} < {megas})");
    let plant = first(&on, BuildingKind::PackingPlant);
    assert_eq!(on.comp::<Building>(plant).map(|b| b.floors), Some(2), "seeded at floors_seed");
    assert_eq!(jobs::places_of(&on, plant, trade(&on, "packer")), 16);
    assert!(world_market::book(&on, ExportGood::Rations).is_some());
    assert!(citysim::systems::stats::csv_header(&on).contains("flow_handling"));
}

/// § 2 (`handling`): a sale to the World pays `handling_per_unit` a unit
/// once to the nearest staffed Yard; with every Yard unstaffed the fee
/// leaks to the World; with no Yard standing nothing is charged. The coin
/// identity holds throughout.
#[test]
fn test_handling_fee_once_per_unit_leak_and_none() {
    let mut w = on_world();
    let id0 = econ::identity(&w);
    let yard = first(&w, BuildingKind::FreightYard);
    let farm =
        *w.buildings_of_kind(BuildingKind::Farm).iter().find(|&&f| w.owner_of(f) != w.owner_of(yard)).expect("a farm");
    let owner = w.owner_of(farm);
    let now = w.tick;
    w.jobs2.yard_seen.insert(yard, now);
    let yard_before = w.purse(w.owner_of(yard));
    let (n, _) = world_market::sell(&mut w, Some(farm), owner, ExportGood::Food, 8);
    assert_eq!(n, 8);
    let fee = (8.0 * w.config.jobs2.handling_per_unit).round() as i64;
    assert_eq!(j2(&w).flow_handling, fee, "one fee per unit sold");
    assert!(w.purse(w.owner_of(yard)) > yard_before, "the Yard's owner took it");
    assert_eq!(j2(&w).handling_leak, 0);
    // Unstaffed: the fee crosses to the World.
    w.jobs2.yard_seen.clear();
    world_market::sell(&mut w, Some(farm), owner, ExportGood::Food, 8);
    assert_eq!(j2(&w).flow_handling, fee);
    assert_eq!(j2(&w).handling_leak, fee);
    // No Yard standing: nothing.
    for y in w.buildings_of_kind(BuildingKind::FreightYard).to_vec() {
        w.comp_mut::<Building>(y).expect("yard").demolished = true;
    }
    world_market::sell(&mut w, Some(farm), owner, ExportGood::Food, 8);
    assert_eq!(j2(&w).handling_leak, fee);
    assert_eq!(econ::identity(&w), id0, "coins made or lost");
}

/// § 2: an import pays `handling_rate` of its coins to a staffed Yard; a
/// cross-owner haul pays the Drivers' `haul_per_unit`.
#[test]
fn test_import_and_haul_fees() {
    let mut w = on_world();
    let id0 = econ::identity(&w);
    let yard = first(&w, BuildingKind::FreightYard);
    let now = w.tick;
    w.jobs2.yard_seen.insert(yard, now);
    let corp = w.corps().into_iter().find(|&c| w.owner_of(yard) != Some(c)).expect("a corp");
    let moved = ownership::import(&mut w, Some(corp), 100, ownership::ImportWhy::Asset);
    assert_eq!(moved, 100);
    let fee = (100.0 * w.config.jobs2.handling_rate).round() as i64;
    assert_eq!(j2(&w).flow_handling, fee);
    let farm =
        *w.buildings_of_kind(BuildingKind::Farm).iter().find(|&&f| w.owner_of(f) != w.owner_of(yard)).expect("a farm");
    let owner = w.owner_of(farm);
    industry::haul_fee(&mut w, farm, owner, 12);
    assert_eq!(j2(&w).flow_handling, fee + (12.0 * w.config.jobs2.haul_per_unit).round() as i64);
    assert_eq!(econ::identity(&w), id0);
}

/// § 2 (`produce`): Packers turn a plant's Food into Rations at
/// `pack_ratio` (as far as the Food goes); the midnight sale sells them to
/// the World's Rations book at `packed_premium` × the Food bid, capped.
#[test]
fn test_packing_makes_rations_and_sells_at_the_premium() {
    let mut w = on_world();
    let id0 = econ::identity(&w);
    let plant = first(&w, BuildingKind::PackingPlant);
    let worker = jobless(&w);
    w.comp_mut::<Building>(plant).expect("plant").stock_food = 5;
    // Eight hours of packing want ~16+ units; five Food make five Rations.
    industry::accrue_packing(&mut w, worker, plant, 8 * citysim::TICKS_PER_HOUR);
    assert_eq!(w.stock(plant, Good::Rations), 5);
    assert_eq!(w.comp::<Building>(plant).map(|b| b.stock_food), Some(0));
    assert_eq!(j2(&w).food_packed, 5);
    assert_eq!(j2(&w).rations_made, 5);
    let premium = w.config.jobs2.packed_premium as f64;
    let (r, f) = (world_market::bid(&w, ExportGood::Rations, 0), world_market::bid(&w, ExportGood::Food, 0));
    let food_appetite = world_market::appetite(&w, ExportGood::Food) as f64;
    let ration_appetite = world_market::appetite(&w, ExportGood::Rations) as f64;
    assert!((r / ration_appetite - premium * f / food_appetite).abs() < 1e-6, "Rations bid {r} vs Food {f}");
    industry::daily(&mut w);
    assert_eq!(w.stock(plant, Good::Rations), 0, "sold at midnight");
    assert_eq!(j2(&w).rations_sold, 5);
    assert!(j2(&w).rations_paid > 0);
    assert_eq!(econ::identity(&w), id0);
}

/// § 2 (`contract`, power): a building's power goes to the nearest lit
/// Utility Plant's owner and the plant owes `fuel_share` to the World; an
/// unlit plant passes it to the World as E13.
#[test]
fn test_power_to_the_plant_fuel_to_the_world() {
    let mut w = on_world();
    let id0 = econ::identity(&w);
    citysim::systems::wages::upkeep(&mut w);
    let (power, fuel) = (j2(&w).flow_power, j2(&w).power_fuel);
    assert!(power > 0, "no power paid to the plant");
    assert!(fuel > 0 && fuel <= power, "fuel {fuel} of power {power}");
    w.jobs2.plant_seen.clear();
    citysim::systems::wages::upkeep(&mut w);
    assert_eq!(j2(&w).flow_power, power, "an unlit plant takes no power");
    assert_eq!(econ::identity(&w), id0);
}

/// § 2: a plant with no operator on shift for `outage_hours` darkens the
/// districts it serves (`PowerOut`): their ICE is off and their night
/// venues closed; an operator back lights them (`PowerBack`).
#[test]
fn test_outage_darkens_districts_and_comes_back() {
    let mut w = on_world();
    assert!(w.districts.iter().all(|d| d.powered()));
    // Every operator gone (a Statistical one on shift counts as there).
    let plant = first(&w, BuildingKind::UtilityPlant);
    for s in w.staff_of(plant).to_vec() {
        citysim::systems::economy::dismiss(&mut w, s, Some(plant), "test".into());
    }
    w.jobs2.plant_seen.clear();
    industry::hourly(&mut w);
    assert!(w.districts.iter().any(|d| d.dark), "no district went dark");
    assert!(count(&w, EventKind::PowerOut) > 0);
    let club = w
        .buildings_of_kind(BuildingKind::Club)
        .iter()
        .copied()
        .find(|&b| industry::dark_at(&w, b))
        .expect("a Club in a dark district");
    let visitor = jobless(&w);
    assert!(!citysim::systems::leisure::door_ok(&w, visitor, club), "a dark Club let a visitor in");
    let now = w.tick;
    w.jobs2.plant_seen.insert(plant, now);
    industry::hourly(&mut w);
    assert!(w.districts.iter().all(|d| d.powered()));
    assert!(count(&w, EventKind::PowerBack) > 0);
}

/// § 2 (`contract`, construction): a build paid with a Yard open pays the
/// Yard (`Flow::Build`), the site closes (no places, no door) and opens
/// with its staff posted when its time is up (`BuildDone`); with no Yard,
/// the Treasury as M11.
#[test]
fn test_build_pays_the_yard_closes_then_opens() {
    let mut w = on_world();
    let id0 = econ::identity(&w);
    let corp = w.corps()[0];
    let lot = citysim::systems::founding::vacant_lots(&w).into_iter().next().expect("a vacant Lot");
    citysim::systems::founding::build_on_lot(&mut w, lot, BuildingKind::Bar, Some(corp)).expect("built");
    let paid = industry::pay_build(&mut w, Some(corp), 200, Some(lot), true);
    assert_eq!(paid, 200);
    assert_eq!(j2(&w).flow_build, 200);
    assert!(industry::closed_site(&w, lot));
    assert_eq!(jobs::places_of(&w, lot, Role::Bartender), 0, "a closed site has places");
    assert!(w.vacancies.get(&lot).is_none_or(|v| v.is_empty()));
    let yard = first(&w, BuildingKind::ConstructionYard);
    assert_eq!(industry::sites_of(&w, yard), vec![lot], "the site is on the Builders' round");
    let visitor = jobless(&w);
    assert!(!citysim::systems::leisure::door_ok(&w, visitor, lot));
    // Its time up, the next hourly pass opens it.
    let now = w.tick;
    w.comp_mut::<Building>(lot).expect("site").under_construction_until = Some(now);
    industry::hourly(&mut w);
    assert!(!industry::closed_site(&w, lot));
    assert!(industry::sites_of(&w, yard).is_empty());
    assert_eq!(count(&w, EventKind::BuildDone), 1);
    let posted = w.vacancies.get(&lot).map_or(0, |v| v.iter().filter(|&&r| r == Role::Bartender).count());
    assert!(posted > 0, "the opened Bar posted no staff");
    assert!(jobs::places_of(&w, lot, Role::Bartender) > 0);
    // No Yard: the Treasury (M11's path).
    for y in w.buildings_of_kind(BuildingKind::ConstructionYard).to_vec() {
        w.comp_mut::<Building>(y).expect("yard").demolished = true;
    }
    let treasury = w.purse(None);
    industry::pay_build(&mut w, Some(corp), 50, None, false);
    assert_eq!(w.purse(None), treasury + 50);
    assert_eq!(econ::identity(&w), id0);
}

/// § 2: a Builder's round walks only the Yard's sites within `site_reach`
/// (a site across the city is built from the Yard: the round ate the shift).
#[test]
fn test_builders_round_keeps_to_sites_in_reach() {
    let mut w = on_world();
    let yard = first(&w, BuildingKind::ConstructionYard);
    let corp = w.corps()[0];
    let door = |w: &World, b: EntityId| w.comp::<Building>(b).map(|bd| bd.door).expect("door");
    let reach = w.config.jobs2.site_reach;
    let lots = citysim::systems::founding::vacant_lots(&w);
    let near =
        lots.iter().copied().find(|&l| door(&w, l).manhattan(door(&w, yard)) <= reach).expect("a Lot near the Yard");
    let far = lots.iter().copied().find(|&l| door(&w, l).manhattan(door(&w, yard)) > reach).expect("a Lot far off");
    for lot in [near, far] {
        citysim::systems::founding::build_on_lot(&mut w, lot, BuildingKind::Bar, Some(corp)).expect("built");
        industry::pay_build(&mut w, Some(corp), 100, Some(lot), true);
    }
    let mut both = vec![near, far];
    both.sort();
    assert_eq!(industry::sites_of(&w, yard), both, "both sites are the Yard's");
    assert_eq!(industry::round_sites(&w, yard), vec![near], "only the near site is walked");
}

/// § 2 (`civic`): the budget's coins after police and Sanitation fund the
/// Jailers first, then the Clerks, then the Depot Hands; the places follow.
#[test]
fn test_civic_allocation_order_and_places() {
    let mut w = on_world();
    let (jailer, clerk, hand) = (trade(&w, "jailer"), trade(&w, "civic_clerk"), trade(&w, "depot_hand"));
    let wage = |r: Role| w.config.wage(r);
    let held = w.sentenced().len() as u32;
    let jailers = held.div_ceil(w.config.jobs2.held_per_jailer).max(1) as u16;
    // Jailers' coins and two Clerks', nothing for the Depot.
    let mut left = i64::from(jailers) * wage(jailer) + 2 * wage(clerk);
    if held == 0 {
        // No prisoner: no Jailer is wanted, the Clerks take it all.
        left = 2 * wage(clerk);
    }
    industry::civic_allocate(&mut w, left);
    let want_j = if held == 0 { 0 } else { jailers };
    assert_eq!(w.jobs2.civic, [want_j, 2, 0]);
    let places = |w: &World, kind: BuildingKind, r: Role| -> usize {
        w.buildings_of_kind(kind).iter().map(|&b| jobs::places_of(w, b, r)).sum()
    };
    assert_eq!(places(&w, BuildingKind::Jail, jailer), usize::from(want_j));
    assert_eq!(places(&w, BuildingKind::Hall, clerk), 2);
    assert_eq!(places(&w, BuildingKind::Warehouse, hand), 0);
    // A fat budget fills all three to their caps.
    industry::civic_allocate(&mut w, 1_000_000);
    assert_eq!(w.jobs2.civic[1], w.config.jobs2.clerks_max);
    assert_eq!(w.jobs2.civic[2], w.config.jobs2.depot_hands_max);
}

/// § 2: the foreman is the longest-serving worker; the hire's event names him.
#[test]
fn test_foreman_is_the_longest_serving() {
    let mut w = on_world();
    let plant = first(&w, BuildingKind::PackingPlant);
    let packer = trade(&w, "packer");
    let a = jobless(&w);
    citysim::systems::demography::hire(&mut w, a, plant, packer);
    w.comp_mut::<Job>(a).expect("job").hired_tick = 0;
    w.run_ticks(5);
    let b = jobless(&w);
    citysim::systems::demography::hire(&mut w, b, plant, packer);
    let old =
        w.staff_of(plant).iter().copied().min_by_key(|&s| (w.comp::<Job>(s).map_or(u64::MAX, |j| j.hired_tick), s));
    assert_eq!(industry::foreman(&w, plant, Some(b)), old);
    let hire =
        w.events.iter().rev().find(|e| e.kind == EventKind::Hire && e.actors.first() == Some(&b)).expect("a Hire");
    assert!(hire.text.contains("the foreman"), "the hire names no foreman: {}", hire.text);
}

/// § 2 (the Hq, JOBS_V2 J30): an Hq's office places are its corp's admin
/// budget (`admin_share` × the 7-day revenue) at the row's wage, and its
/// staff add the admin term to the corp's competence.
#[test]
fn test_hq_places_follow_the_admin_budget_and_raise_competence() {
    let mut w = on_world();
    let hq = first(&w, BuildingKind::Hq);
    let corp = w.corp_of_building(hq).expect("a corp's Hq");
    let ow = trade(&w, "office_worker");
    let wage = w.config.wage(ow);
    w.comp_mut::<citysim::Corp>(corp).expect("corp").rev = std::iter::repeat_n(1000, 7).collect();
    let budget = industry::admin_budget(&w, corp).expect("wages on");
    assert_eq!(budget, (w.config.jobs2.admin_share * 1000.0).floor() as i64);
    let cap = 6 * usize::from(w.comp::<Building>(hq).expect("hq").floors);
    assert_eq!(jobs::places_of(&w, hq, ow), ((budget / wage) as usize).min(cap));
    w.comp_mut::<citysim::Corp>(corp).expect("corp").rev = std::iter::repeat_n(0, 7).collect();
    assert_eq!(jobs::places_of(&w, hq, ow), 0, "no revenue, no office");
    assert_eq!(industry::admin_term(&w, corp), 0.0);
    let a = w
        .citizens()
        .into_iter()
        .find(|&x| {
            law::living(&w, x)
                && citysim::systems::demography::is_adult(&w, x)
                && !w.has::<Job>(x)
                && w.comp::<citysim::Skills>(x).is_some_and(|s| s.knowledge > 0.3)
        })
        .expect("a jobless adult who knows things");
    citysim::systems::demography::hire(&mut w, a, hq, ow);
    assert!(industry::admin_term(&w, corp) > 0.0, "an office worker adds no admin term");
}

/// § 1 (`day`): a one-day post ends at its `until`, no layoff event.
#[test]
fn test_day_post_ends_at_its_until() {
    let mut c = Config::load();
    c.jobs2.enabled = true;
    let mut row = c.trades.iter().find(|r| r.id == "builder").expect("builder").clone();
    row.id = "day_hand".into();
    row.label = "Day Hand".into();
    row.model = citysim::config::TradeModel::Day;
    row.duty = citysim::config::TradeDuty::Corner;
    c.trades.push(row);
    c.reset_trade_cache();
    let mut w = World::new(42, c);
    let role = trade(&w, "day_hand");
    let yard = first(&w, BuildingKind::ConstructionYard);
    let a = jobless(&w);
    citysim::systems::demography::hire(&mut w, a, yard, role);
    let until = w.tick + 10;
    w.comp_mut::<Job>(a).expect("job").until = Some(until);
    assert_eq!(industry::end_day_posts(&mut w), 0, "ended before its until");
    let fired = count(&w, EventKind::Fire) + count(&w, EventKind::LaidOff);
    w.run_ticks(10);
    assert_eq!(industry::end_day_posts(&mut w), 1);
    assert!(!w.has::<Job>(a));
    assert_eq!(count(&w, EventKind::Fire) + count(&w, EventKind::LaidOff), fired, "a day post's end is no layoff");
}

/// § 1 (`resale`): a stall's purchase at a Market's price and its sale at
/// a markup balance: coins and food are conserved.
#[test]
fn test_resale_buy_and_sell_balance() {
    let mut w = on_world();
    let id0 = econ::identity(&w);
    let market = first(&w, BuildingKind::Market);
    let vendor = jobless(&w);
    let buyer =
        w.citizens().into_iter().rev().find(|&a| a != vendor && w.purse(Some(a)) > 50).expect("a buyer with coins");
    w.comp_mut::<citysim::Wallet>(vendor).expect("wallet").coins += 100;
    let food = |w: &World, a: EntityId| w.comp::<citysim::Inventory>(a).map_or(0, |i| i.food);
    let (f0, s0) = (food(&w, vendor) + food(&w, buyer), w.comp::<Building>(market).map_or(0, |b| b.stock_food));
    let (n, paid) = industry::resale_buy(&mut w, vendor, market, 3);
    assert_eq!(n, 3);
    assert!(paid > 0);
    let price = paid / 3;
    let got = industry::resale_sell(&mut w, vendor, buyer, price, 1.5);
    assert!(got >= price, "sold at no markup");
    assert_eq!(food(&w, vendor) + food(&w, buyer), f0 + 3);
    assert_eq!(w.comp::<Building>(market).map_or(0, |b| b.stock_food), s0 - 3);
    w.comp_mut::<citysim::Wallet>(vendor).expect("wallet").coins -= 100;
    assert_eq!(econ::identity(&w), id0, "resale moved coins out of thin air");
}

/// § 2: a Jail short of Jailers is an easier breach; staffed, it is not.
#[test]
fn test_jailers_short_reads_the_held() {
    let mut w = on_world();
    if w.sentenced().is_empty() {
        // Nobody held: nobody is needed.
        assert!(!industry::jailers_short(&w));
        return;
    }
    assert!(industry::jailers_short(&w), "no Jailer and prisoners: short");
    let jail = first(&w, BuildingKind::Jail);
    let role = trade(&w, "jailer");
    let need = (w.sentenced().len() as u32).div_ceil(w.config.jobs2.held_per_jailer);
    for _ in 0..need {
        let a = jobless(&w);
        citysim::systems::demography::hire(&mut w, a, jail, role);
    }
    assert!(!industry::jailers_short(&w));
}

/// Saves: a world with every phase 1 state (a dark district, an open site,
/// fuel owed, the civic targets) runs on byte for byte after a round trip.
#[test]
fn test_save_round_trip_with_phase1_state() {
    let mut w = on_world();
    w.run_ticks(citysim::TICKS_PER_HOUR * 3);
    w.jobs2.plant_seen.clear();
    let corp = w.corps()[0];
    if let Some(lot) = citysim::systems::founding::vacant_lots(&w).into_iter().next() {
        citysim::systems::founding::build_on_lot(&mut w, lot, BuildingKind::Bar, Some(corp)).expect("built");
        industry::pay_build(&mut w, Some(corp), 100, Some(lot), true);
    }
    industry::civic_allocate(&mut w, 100);
    w.run_ticks(citysim::TICKS_PER_HOUR * 2);
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text, "the round trip lost state");
    w.run_ticks(citysim::TICKS_PER_HOUR * 4);
    back.run_ticks(citysim::TICKS_PER_HOUR * 4);
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w), "a loaded world ran on differently");
}
