//! Jobs and room P2 (docs/JOBS_V2.md § 2.2-2.3; plan J5-J8): the new Lots,
//! floors multiplying places and seats, `AddFloor` and its trigger, and a
//! save round trip.

use std::collections::VecDeque;

use citysim::map::Map;
use citysim::systems::ownership;
use citysim::systems::{founding, jobs, wages};
use citysim::{save, Building, BuildingKind, Config, Corp, EntityId, EventKind, Job, Role, World};

/// The wages city (`[economy2] wages` is off by default until the flip).
fn wages_city() -> World {
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    let w = World::new(42, cfg);
    assert!(wages::on(&w));
    w
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == name)).expect(name)
}

fn set_floors(w: &mut World, b: EntityId, floors: u8) {
    w.comp_mut::<Building>(b).expect("building").floors = floors;
}

fn floors(w: &World, b: EntityId) -> u8 {
    w.comp::<Building>(b).expect("building").floors
}

/// `role` staff plus open `role` vacancies at `b`.
fn places_held(w: &World, b: EntityId, role: Role) -> usize {
    ownership::staff_at(w, b).into_iter().filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.role == role)).count()
        + w.vacancies.get(&b).map_or(0, |v| v.iter().filter(|&&r| r == role).count())
}

/// J5: the shipped map carries 160 Lots (60 + 100 on the free ground), 20 of
/// them double-wide Vats yards, every building one floor; the generator's
/// BFS proved every door reachable.
#[test]
fn test_map_has_the_new_lots_on_one_floor() {
    let w = World::new(42, Config::load());
    assert_eq!(w.map.count_kind(BuildingKind::Lot), 160);
    let wide = w.map.buildings.iter().filter(|b| b.kind == BuildingKind::Lot && b.rect.w == 15).count();
    assert_eq!(wide, 20, "the Vats' double-wide yards");
    assert!(w.map.buildings.iter().all(|b| b.floors == 1));
}

/// J6: the map line's optional eighth field is the floor count (1 when
/// absent); a Block seeded on three floors seats three floors' residents.
#[test]
fn test_map_eighth_field_is_floors() {
    let text = std::fs::read_to_string(Config::load().assets_dir.join("map.txt")).expect("map");
    let mut done = false;
    let edited: String = text
        .lines()
        .map(|l| {
            if !done && l.starts_with("B Home ") {
                done = true;
                format!("{l} 3\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    let map = Map::parse(&edited);
    let homes: Vec<_> = map.buildings.iter().filter(|b| b.kind == BuildingKind::Home).collect();
    assert_eq!(homes[0].floors, 3);
    assert!(homes[1..].iter().all(|b| b.floors == 1));
    assert_eq!(founding::with_floors(BuildingKind::Home, 5, 3), 15);
}

/// J7: a two-floor Farm's places are twice a one-floor Farm's, and under the
/// margin rule a corp staffs it to twice the places (overtime alike).
#[test]
fn test_two_floor_farm_posts_twice_the_places() {
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg.economy2.staff_ceiling_mult = 1.0;
    cfg.economy2.capital_hi_days = 0.0;
    cfg.economy2.capital_lo_days = 0.0;
    let mut w = World::new(42, cfg);
    let corp = corp_named(&w, "Nutrix");
    let farms = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm);
    assert!(farms.len() >= 2, "Nutrix holds two Farms");
    let (two, one) = (farms[0], farms[1]);
    set_floors(&mut w, two, 2);
    let full = citysim::systems::corp_brain::full_staff(&w, BuildingKind::Farm);
    assert_eq!(jobs::places_of(&w, one, Role::Farmer), full);
    assert_eq!(jobs::places_of(&w, two, Role::Farmer), 2 * full);
    assert_eq!(jobs::places_of(&w, two, Role::Clerk), 0, "a role the kind does not employ");
    assert_eq!(wages::ceiling_at(&w, two, Role::Farmer), 2 * full);
    // Food demand read full (seven days of full sell-through), as tests/wages.rs pins it.
    for m in ownership::owned_of_kind(&w, Some(corp), BuildingKind::Market) {
        if let Some(mk) = w.comp_mut::<citysim::Market>(m) {
            mk.sales = VecDeque::from(vec![100; 7]);
            mk.stock_hist = VecDeque::from(vec![100; 7]);
        }
    }
    assert!(citysim::systems::corp_brain::demand_of(&w, corp, citysim::Niche::Food).0 >= 0.99);
    for _ in 0..60 {
        let c = w.comp_mut::<Corp>(corp).expect("corp");
        c.rev = VecDeque::from(vec![3000; wages::WINDOW_DAYS]);
        c.pay = VecDeque::from(vec![100; wages::WINDOW_DAYS]);
        wages::staff(&mut w);
    }
    let (a, b) = (places_held(&w, two, Role::Farmer), places_held(&w, one, Role::Farmer));
    assert!(b >= full, "the one-floor Farm staffed to full ({b} of {full})");
    assert_eq!(a, b + full, "the second floor adds one floor's places");
}

/// J7: seats multiply for Blocks, Hotels and venues only, clamped at 255; a
/// Lot of two floors converted into a Club seats and posts two floors' worth.
#[test]
fn test_capacity_multiplies_and_clamps() {
    assert_eq!(founding::with_floors(BuildingKind::Home, 5, 2), 10);
    assert_eq!(founding::with_floors(BuildingKind::Hotel, 12, 3), 36);
    assert_eq!(founding::with_floors(BuildingKind::Club, 40, 6), 240);
    assert_eq!(founding::with_floors(BuildingKind::Club, 100, 3), 255, "clamped at u8::MAX");
    assert_eq!(founding::with_floors(BuildingKind::Farm, 16, 3), 16, "a Farm's room is not seats");
    assert_eq!(founding::with_floors(BuildingKind::Market, 20, 2), 20);

    let mut w = World::new(42, Config::load());
    let lot = founding::vacant_lots(&w)
        .into_iter()
        .find(|&l| {
            let bd = w.comp::<Building>(l).expect("lot");
            founding::tier_ok(BuildingKind::Club, bd.tier) && founding::door_tier(&w, bd.door) >= 1
        })
        .expect("a Lot a Club may take");
    let mut one = World::new(42, Config::load());
    founding::build_on_lot(&mut one, lot, BuildingKind::Club, None).expect("one floor");
    set_floors(&mut w, lot, 2);
    founding::build_on_lot(&mut w, lot, BuildingKind::Club, None).expect("two floors");
    let (c1, c2) = (one.comp::<Building>(lot).expect("club").capacity, w.comp::<Building>(lot).expect("club").capacity);
    assert_eq!(usize::from(c2), (2 * usize::from(c1)).min(255));
    let posted = |w: &World| w.vacancies.get(&lot).map_or(0, |v| v.iter().filter(|&&r| r == Role::Host).count());
    assert_eq!(posted(&w), 2 * posted(&one));
    assert!(posted(&one) > 0);
}

/// J8: `add_floor` charges `floor_cost_frac × found_cost` to the Treasury
/// (`Flow::Found`, coins conserved), grows the seats by a floor, logs
/// `FloorAdded`, and stops at `floors_max`; an empty purse is refused.
#[test]
fn test_add_floor_charges_caps_and_logs() {
    let mut w = wages_city();
    let corp = corp_named(&w, "Nutrix");
    let farm = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm)[0];
    let cost = founding::floor_cost(&w, BuildingKind::Farm).expect("a Farm may build up");
    assert_eq!(cost, (w.config.floors.floor_cost_frac * w.config.economy2.farm_found_cost as f32).round() as i64);
    assert!(cost > 0);
    w.comp_mut::<Corp>(corp).expect("corp").treasury = 10 * cost;
    let coins = ownership::total_coins(&w);
    let treasury = w.treasury().expect("treasury").coins;
    founding::add_floor(&mut w, farm, Some(corp)).expect("a second floor");
    assert_eq!(floors(&w, farm), 2);
    assert_eq!(w.purse(Some(corp)), 9 * cost);
    assert_eq!(w.treasury().expect("treasury").coins, treasury + cost);
    assert_eq!(ownership::total_coins(&w), coins, "coins conserved");
    assert!(w.events.iter().any(|e| e.kind == EventKind::FloorAdded
        && e.actors.as_slice() == [corp, farm]
        && e.text.contains("2 floors")));
    let max = w.config.floors.max_for(BuildingKind::Farm);
    assert_eq!(max, 3);
    founding::add_floor(&mut w, farm, Some(corp)).expect("a third floor");
    assert_eq!(floors(&w, farm), 3);
    assert!(founding::add_floor(&mut w, farm, Some(corp)).is_err(), "floors_max holds");
    assert_eq!(floors(&w, farm), 3);
    assert_eq!(w.purse(Some(corp)), 8 * cost, "a refused floor costs nothing");
    // A Block's seats grow a floor at a time.
    let home = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .find(|&h| w.comp::<Building>(h).is_some_and(|bd| !bd.derelict && !bd.demolished))
        .expect("a Block");
    let seats = w.comp::<Building>(home).expect("home").capacity;
    w.comp_mut::<Corp>(corp).expect("corp").treasury = 0;
    assert!(founding::add_floor(&mut w, home, Some(corp)).is_err(), "an empty purse is refused");
    founding::add_floor(&mut w, home, None).expect("the city builds up its own Block");
    assert_eq!(w.comp::<Building>(home).expect("home").capacity, 2 * seats);
    // A kind no one founds stays on one floor.
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    assert!(founding::add_floor(&mut w, market, None).is_err());
}

/// J8 trigger: a corp's Farm at its ceiling with room for another floor's
/// staff adds a floor on the `floor_days`-th midnight, not before, and not
/// while a vacant Lot lies within `lot_reach`.
#[test]
fn test_floor_trigger_after_floor_days_with_no_lot_near() {
    let setup = |clear_lots: bool| -> (World, EntityId, EntityId) {
        let mut w = wages_city();
        let corp = corp_named(&w, "Nutrix");
        let farm = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm)[0];
        if clear_lots {
            for l in founding::vacant_lots(&w) {
                w.comp_mut::<Building>(l).expect("lot").demolished = true;
            }
        } else {
            // A vacant Lot by the Farm's door.
            let door = w.comp::<Building>(farm).expect("farm").door;
            let near = founding::vacant_lots(&w)
                .into_iter()
                .any(|l| w.comp::<Building>(l).is_some_and(|bd| bd.door.manhattan(door) <= w.config.floors.lot_reach));
            assert!(near, "the shipped map has a Lot within reach of Nutrix's first Farm");
        }
        w.comp_mut::<Corp>(corp).expect("corp").treasury = 1_000_000;
        w.comp_mut::<Corp>(corp).expect("corp").last_build_tick = None;
        // The Farm at its ceiling: the open places make up the difference.
        let ceiling = wages::ceiling_at(&w, farm, Role::Farmer);
        let held = places_held(&w, farm, Role::Farmer);
        w.vacancies.entry(farm).or_default().extend(std::iter::repeat_n(Role::Farmer, ceiling - held));
        (w, corp, farm)
    };
    let room = 10_000.0;
    let days = w_days();
    let (mut w, corp, farm) = setup(true);
    for d in 1..days {
        assert!(!wages::floor_pass(&mut w, corp, 0.0, room, 1000.0, 500.0), "no floor on midnight {d}");
    }
    assert_eq!(floors(&w, farm), 1);
    assert!(wages::floor_pass(&mut w, corp, 0.0, room, 1000.0, 500.0), "the floor_days-th midnight");
    assert_eq!(floors(&w, farm), 2);
    assert!(w.events.iter().any(|e| e.kind == EventKind::FloorAdded));
    assert!(w.comp::<Corp>(corp).expect("corp").last_build_tick.is_some(), "the build cooldown runs");
    // Room under one floor's wages resets the count.
    let (mut w, corp, farm) = setup(true);
    for _ in 0..days + 2 {
        assert!(!wages::floor_pass(&mut w, corp, 0.0, 1.0, 1000.0, 500.0));
    }
    assert_eq!(w.comp::<Building>(farm).expect("farm").floor_days, 0);
    // A Lot within reach: Grow's job, not a floor.
    let (mut w, corp, farm) = setup(false);
    for _ in 0..days + 2 {
        assert!(!wages::floor_pass(&mut w, corp, 0.0, room, 1000.0, 500.0));
    }
    assert_eq!(floors(&w, farm), 1);
}

fn w_days() -> u8 {
    Config::load().floors.floor_days
}

/// J6, J32: floors survive a save round trip byte for byte.
#[test]
fn test_save_round_trip_keeps_floors() {
    let mut w = World::new(42, Config::load());
    let home = w.buildings_of_kind(BuildingKind::Home)[0];
    founding::add_floor(&mut w, home, None).expect("a floor");
    w.comp_mut::<Building>(home).expect("home").floor_days = 3;
    w.run_ticks(10);
    let text = save::to_ron(&w);
    let mut back = save::from_ron(&text).expect("load");
    assert_eq!(floors(&back, home), 2);
    assert_eq!(back.comp::<Building>(home).expect("home").floor_days, 3);
    assert_eq!(save::to_ron(&back), text);
    w.run_ticks(60);
    back.run_ticks(60);
    assert_eq!(save::to_ron(&w), save::to_ron(&back));
}

/// J7 (review): floors count in the wages-off staffing too: under Hunker a
/// two-floor Farm emptied of its Vat Techs keeps half of both floors'
/// places open (not half a floor's), and a two-floor Market staffed to one
/// floor's full staff is not over half and lays nobody off.
#[test]
fn test_hunker_keeps_half_of_every_floor() {
    use citysim::systems::corp_brain;
    use citysim::{CorpOrder, Niche};
    let mut w = World::new(42, Config::load());
    // A day in: the opening vacancies filled.
    w.run_ticks(citysim::TICKS_PER_DAY);
    let corp = corp_named(&w, "Nutrix");
    let farm = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm)[0];
    let market = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Market)[0];
    set_floors(&mut w, farm, 2);
    set_floors(&mut w, market, 2);
    for a in w.citizens() {
        if w.comp::<Job>(a).and_then(|j| j.employer) == Some(farm) {
            w.vacate_job(a);
        }
    }
    w.vacancies.remove(&market);
    let full_market = corp_brain::full_staff(&w, BuildingKind::Market);
    let staff = |w: &World| ownership::staff_at(w, market).len();
    // The Market staffed to one floor's full staff.
    while staff(&w) < full_market {
        let id = citysim::systems::demography::hire_candidate(&w, market, Role::Clerk).expect("a candidate");
        citysim::systems::demography::hire(&mut w, id, market, Role::Clerk);
    }
    let before = staff(&w);
    assert!(before > full_market / 2 && before <= full_market, "the Market over half a floor ({before})");
    {
        let c = w.comp_mut::<Corp>(corp).expect("corp");
        c.order = CorpOrder::Hunker;
        c.order_niche = Some(Niche::Food);
        c.cashflow.clear();
    }
    corp_brain::act(&mut w, corp);
    let half_farm = jobs::places_of(&w, farm, Role::Farmer).div_ceil(2);
    assert_eq!(half_farm, corp_brain::full_staff(&w, BuildingKind::Farm));
    assert_eq!(w.vacancies.get(&farm).map_or(0, Vec::len), half_farm, "half of both floors, not half of one");
    assert_eq!(staff(&w), before, "a two-floor Market at one floor's staff is not over half");
}

/// J8 through `wages::staff` (review): the real room and lean wiring. A
/// corp with revenue to spare, capital far above its floor and no vacant Lot
/// anywhere, whose Farms stand at their ceilings, adds a floor on the
/// `floor_days`-th midnight and not before.
#[test]
fn test_wages_staff_adds_a_floor_on_the_seventh_midnight() {
    let mut w = wages_city();
    let corp = corp_named(&w, "Nutrix");
    for l in founding::vacant_lots(&w) {
        w.comp_mut::<Building>(l).expect("lot").demolished = true;
    }
    {
        let c = w.comp_mut::<Corp>(corp).expect("corp");
        c.treasury = 1_000_000;
        c.last_build_tick = None;
    }
    let farms = ownership::owned_of_kind(&w, Some(corp), BuildingKind::Farm);
    let order = wages::food_order(&w);
    for &f in &farms {
        let ceiling = wages::ceiling_at(&w, f, Role::Farmer) + 1 + order as usize;
        let held = places_held(&w, f, Role::Farmer);
        w.vacancies.entry(f).or_default().extend(std::iter::repeat_n(Role::Farmer, ceiling.saturating_sub(held)));
    }
    let added = |w: &World| w.events.iter().filter(|e| e.kind == EventKind::FloorAdded).count();
    let days = w_days();
    for d in 1..=days {
        let c = w.comp_mut::<Corp>(corp).expect("corp");
        c.rev = VecDeque::from(vec![30_000; wages::WINDOW_DAYS]);
        c.pay = VecDeque::from(vec![100; wages::WINDOW_DAYS]);
        wages::staff(&mut w);
        if d < days {
            assert_eq!(added(&w), 0, "no floor on midnight {d}");
        }
    }
    assert_eq!(added(&w), 1, "one floor on the floor_days-th midnight");
    let floored: Vec<EntityId> = farms.iter().copied().filter(|&f| floors(&w, f) == 2).collect();
    assert_eq!(floored.len(), 1);
    assert!(w.comp::<Corp>(corp).expect("corp").treasury < 1_000_000, "the corp paid for it");
}
