//! The Real economy phase 3b (docs/ECONOMY_V2.md § 4; plan 3b.6): Missions
//! on donations. Every meal here is one unit off a building's shelf and a
//! `needs::eat`; every donation coins moved from one integer purse into a
//! building's purse; the identity `total_coins + Σ outside − minted` holds.

use citysim::goap::ActionKind;
use citysim::systems::{charity, demography, lod, ownership, street};
use citysim::word::Deed;
use citysim::{
    Brain, Building, BuildingKind, Config, EntityId, Household, Job, Lod, Memory, Needs, Personality, Position, Role,
    Wallet, World, TICKS_PER_DAY,
};

fn world() -> World {
    World::new(42, Config::load())
}

/// L2's conservation identity (`tests/outside.rs`).
fn identity(w: &World) -> i64 {
    ownership::total_coins(w) + w.outside.treasuries() - w.outside.minted
}

fn mission(w: &World) -> EntityId {
    *w.buildings_of_kind(BuildingKind::Mission).first().expect("a seeded Mission")
}

fn purse(w: &World, m: EntityId) -> i64 {
    w.comp::<Building>(m).and_then(|b| b.charity.as_ref()).map_or(0, |c| c.purse)
}

fn stock(w: &World, b: EntityId) -> u32 {
    w.comp::<Building>(b).map_or(0, |bd| bd.stock_food)
}

fn set_coins(w: &mut World, id: EntityId, c: i64) {
    w.comp_mut::<Wallet>(id).expect("wallet").coins = c;
}

fn set_hunger(w: &mut World, id: EntityId, h: f32) {
    w.comp_mut::<Needs>(id).expect("needs").hunger = h;
}

fn hunger(w: &World, id: EntityId) -> f32 {
    w.comp::<Needs>(id).map_or(1.0, |n| n.hunger)
}

/// A housed, jobless adult with a Brain, in no gang.
fn civilian(w: &World, skip: &[EntityId]) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            !skip.contains(&a)
                && demography::is_adult(w, a)
                && w.has::<Brain>(a)
                && !w.has::<Job>(a)
                && w.gang_of(a).is_none()
                && w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
                && citysim::systems::life::exec_corp(w, a).is_none()
        })
        .expect("a civilian")
}

fn run_days(w: &mut World, days: u64) {
    for _ in 0..days * TICKS_PER_DAY {
        citysim::tick(w);
    }
}

/// E26: a Mission stands at seed in a Sump district on the city's deed
/// with its purse, two Volunteer vacancies and the Chapel has a kitchen.
#[test]
fn test_seed_mission_and_purist_chapel_has_a_kitchen() {
    let w = world();
    assert!(charity::on(&w));
    let m = mission(&w);
    let b = w.comp::<Building>(m).expect("building");
    assert_eq!(b.kind, BuildingKind::Mission);
    assert!(b.owner.is_none(), "the city's deed");
    assert!(b.charity.is_some());
    assert_eq!(w.map.zone(b.door), citysim::Zone::Sump);
    // The Volunteer posts wait for a funded larder (a week of staff meals);
    // an empty kitchen lets its Volunteers go (an unpaid Job draws no dole).
    assert!(!w.vacancies.contains_key(&m), "unfunded: no Volunteer post at seed");
    let mut w = w;
    charity::donate(&mut w, None, m, 100);
    charity::daily(&mut w);
    assert_eq!(w.vacancies.get(&m).map(|v| v.len()), Some(2));
    assert!(w.vacancies.get(&m).is_some_and(|v| v.iter().all(|&r| r == Role::Volunteer)));
    let v = civilian(&w, &[]);
    demography::hire(&mut w, v, m, Role::Volunteer);
    w.vacancies.remove(&m);
    if let Some(b) = w.comp_mut::<Building>(m) {
        b.stock_food = 0;
        if let Some(c) = b.charity.as_mut() {
            c.purse = 0;
        }
    }
    charity::daily(&mut w);
    assert!(!w.has::<Job>(v), "let go when the kitchen is empty");
    assert!(!w.vacancies.contains_key(&m));
    // The Chapel: The Unplugged's Hideout carries a `Charity`.
    let purist = w.gang_list().iter().copied().find(|&g| citysim::systems::creeds::is_purist(&w, g)).expect("purist");
    let chapel = w.hideout_of(purist).expect("chapel");
    assert!(w.comp::<Building>(chapel).is_some_and(|b| b.charity.is_some()));
    assert_eq!(charity::missions(&w).len(), 2);
    // With the switch off nothing stands.
    let mut cfg = Config::load();
    cfg.charity.enabled = false;
    let off = World::new(42, cfg);
    assert!(off.buildings_of_kind(BuildingKind::Mission).is_empty());
    assert!(charity::missions(&off).is_empty());
}

/// E27 (plan 3b.6): the kitchen's restock costs the purse exactly
/// `units × (wholesale + meal_markup)`, and a served meal takes one unit,
/// lifts the eater's hunger and writes the ledger-only `Alms` line.
#[test]
fn test_mission_meal_costs_exactly_its_purse() {
    let mut w = world();
    let m = mission(&w);
    let before = identity(&w);
    // The Treasury endows the kitchen (a city gift: a purse move, no mint).
    assert_eq!(charity::donate(&mut w, None, m, 100), 100);
    assert_eq!(purse(&w, m), 100);
    assert_eq!(identity(&w), before, "a donation moves coins, it makes none");
    let cost = charity::meal_cost(&w);
    assert_eq!(cost, w.config.corps.wholesale + w.config.charity.meal_markup);
    let p0 = purse(&w, m);
    charity::daily(&mut w);
    let bought = stock(&w, m);
    assert!(bought > 0, "the kitchen restocked");
    assert_eq!(p0 - purse(&w, m), i64::from(bought) * cost, "the purse paid the units exactly");
    assert_eq!(identity(&w), before);
    // A hungry, broke body at the kitchen eats (its coins to the Treasury,
    // so the identity still reads).
    let a = civilian(&w, &[]);
    let had = w.comp::<Wallet>(a).map_or(0, |x| x.coins);
    ownership::pay(&mut w, Some(a), None, had, ownership::Flow::Fine);
    set_hunger(&mut w, a, 0.2);
    w.enter_building(a, m);
    assert!(charity::can_start(&w, a, ActionKind::EatAlms, Some(m)));
    let alms0 = w.stats.current.econ.flow_alms;
    let meals0 = w.stats.current.econ.mission_meals;
    assert!(matches!(charity::on_eat_alms(&mut w, a, Some(m)), citysim::StepResult::Done));
    assert!(hunger(&w, a) > 0.2, "the meal fed them");
    assert_eq!(stock(&w, m), bought - 1);
    assert_eq!(w.stats.current.econ.flow_alms - alms0, cost, "Alms is the meal's value, ledger-only");
    assert_eq!(w.stats.current.econ.mission_meals - meals0, 1);
    assert_eq!(identity(&w), before, "a meal moves no coin");
    // Not hungry, or with the price of a meal: no alms.
    set_hunger(&mut w, a, 0.9);
    assert!(!charity::can_start(&w, a, ActionKind::EatAlms, Some(m)));
    set_hunger(&mut w, a, 0.2);
    set_coins(&mut w, a, 50);
    assert!(!charity::can_start(&w, a, ActionKind::EatAlms, Some(m)));
}

/// E27: the hourly cap: `meals_per_hour`, doubled by a Volunteer on shift.
#[test]
fn test_hourly_cap_doubles_with_a_volunteer() {
    let mut w = world();
    let m = mission(&w);
    charity::donate(&mut w, None, m, 500);
    charity::daily(&mut w);
    assert!(stock(&w, m) >= 30);
    let cap = w.config.charity.meals_per_hour;
    let mut eaters = Vec::new();
    for _ in 0..cap + 2 {
        let a = civilian(&w, &eaters);
        eaters.push(a);
        set_coins(&mut w, a, 0);
        set_hunger(&mut w, a, 0.1);
        w.enter_building(a, m);
    }
    let mut served = 0;
    for &a in &eaters {
        if charity::can_start(&w, a, ActionKind::EatAlms, Some(m)) {
            charity::on_eat_alms(&mut w, a, Some(m));
            served += 1;
        }
    }
    assert_eq!(served, cap, "the hour's cap holds");
    // A Volunteer on shift inside doubles it: hire one and put them on shift.
    let v = civilian(&w, &eaters);
    demography::hire(&mut w, v, m, Role::Volunteer);
    assert_eq!(w.comp::<Job>(v).map(|j| j.wage_per_day), Some(0), "unpaid");
    let start = w.comp::<Job>(v).map(|j| j.shifts[0].0).expect("shift");
    // Advance to the shift's start on the next hour boundary.
    let target = u64::from(start);
    while w.tick_of_day() as u64 != target {
        citysim::tick(&mut w);
    }
    w.enter_building(v, m);
    charity::donate(&mut w, None, m, 500);
    charity::daily(&mut w);
    let mut served = 0;
    for &a in &eaters {
        set_hunger(&mut w, a, 0.1);
        w.enter_building(a, m);
        if charity::can_start(&w, a, ActionKind::EatAlms, Some(m)) {
            charity::on_eat_alms(&mut w, a, Some(m));
            served += 1;
        }
    }
    assert_eq!(served, cap + 2, "every eater served under the doubled cap");
}

/// E28 (plan 3b.6): a donation moves coins from the donor into the purse
/// (`total_coins` unchanged); the god's gift is minted from the World and
/// the identity still holds; `Donated` is logged at 5.
#[test]
fn test_donate_moves_coins_into_purse_and_identity_holds() {
    let mut w = world();
    let m = mission(&w);
    let a = civilian(&w, &[]);
    set_coins(&mut w, a, 500);
    let (coins0, total0, id0) = (ownership::total_coins(&w), ownership::total_coins(&w), identity(&w));
    let _ = coins0;
    assert_eq!(charity::donate(&mut w, Some(a), m, 50), 50);
    assert_eq!(w.comp::<Wallet>(a).map(|x| x.coins), Some(450));
    assert_eq!(purse(&w, m), 50);
    assert_eq!(ownership::total_coins(&w), total0, "the purse is in total_coins");
    assert_eq!(identity(&w), id0);
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Donated && e.actors.contains(&a)));
    // Capped at the wallet.
    assert_eq!(charity::donate(&mut w, Some(a), m, 1_000), 450);
    assert_eq!(purse(&w, m), 500);
    assert_eq!(identity(&w), id0);
    // The god's gift: minted from the World account, crossed in.
    let minted0 = w.outside.minted;
    assert_eq!(charity::god_donate(&mut w, m, 1_000), Ok(1_000));
    assert_eq!(purse(&w, m), 1_500);
    assert_eq!(w.outside.minted - minted0, 1_000);
    assert_eq!(identity(&w), id0, "a god gift is an outside donor's: minted and crossed in");
    assert_eq!(w.stats.current.econ.mission_purse, charity::purses(&w));
    assert!(charity::god_donate(&mut w, a, 10).is_err(), "an agent is no Mission");
}

/// E28: the give test: a donor with coins above `give_floor_days` of rent
/// and meals gives `donate_frac` of the surplus when its keyed unit falls
/// under `donate_base × (lawfulness + loyalty) ÷ 2`; the body plan walks
/// to the Mission; one gift a day; the pure hash reads the same on a
/// second world.
#[test]
fn test_wants_to_give_is_keyed_and_once_a_day() {
    let mut w = world();
    // Force the test's probability to 1: donate_base 1.0 for a saint.
    w.config.charity.donate_base = 1.0;
    let a = civilian(&w, &[]);
    if let Some(p) = w.comp_mut::<Personality>(a) {
        p.lawfulness = 1.0;
        p.loyalty = 1.0;
    }
    set_coins(&mut w, a, 2_000);
    let amount = charity::wants_to_give(&w, a).expect("gives");
    let frac = w.config.charity.donate_frac;
    assert!(amount >= 1 && amount <= (2_000.0 * frac) as i64 + 1, "amount {amount}");
    let plan = charity::donate_plan(&mut w, a).expect("a plan");
    assert_eq!(plan.goal, citysim::GoalKind::Socialise);
    assert_eq!(plan.steps.last().map(|s| s.action), Some(ActionKind::Donate));
    // The nearest kitchen (the Mission or the Chapel).
    let m = plan.target.expect("a target");
    assert!(charity::missions(&w).contains(&m));
    // At the door: the gift.
    w.enter_building(a, m);
    assert!(charity::can_start(&w, a, ActionKind::Donate, Some(m)));
    assert!(matches!(charity::on_donate(&mut w, a, Some(m)), citysim::StepResult::Done));
    assert_eq!(purse(&w, m), amount);
    assert!(charity::wants_to_give(&w, a).is_none(), "one gift a day");
    // Below the floor: nothing.
    set_coins(&mut w, a, 5);
    assert!(charity::donate_plan(&mut w, a).is_none());
    // Keyed: a second world with the same seed reads the same unit.
    let seed = w.seed();
    let day = w.day();
    let u1 = citysim::systems::econ::hash_unit(seed, citysim::econ::PURPOSE_DONATE, day, u64::from(a.index));
    let w2 = world();
    let u2 = citysim::systems::econ::hash_unit(w2.seed(), citysim::econ::PURPOSE_DONATE, day, u64::from(a.index));
    assert_eq!(u1, u2);
}

/// E29 (plan 3b.6): a gift of `rep_gift_min` writes the `Gave` deed into
/// the donor's heard store and the district pool; a small one does not.
#[test]
fn test_gave_deed_written_for_large_gift() {
    let mut w = world();
    assert!(w.config.gossip.enabled);
    let m = mission(&w);
    let a = civilian(&w, &[]);
    set_coins(&mut w, a, 500);
    let min = w.config.charity.rep_gift_min;
    charity::donate(&mut w, Some(a), m, min - 1);
    let gave = |w: &World| {
        w.comp::<Memory>(a)
            .is_some_and(|mm| mm.heard.iter().any(|e| e.deed == Some(Deed::Gave) && e.subject == Some(a)))
    };
    assert!(!gave(&w), "a small gift leaves no deed");
    charity::donate(&mut w, Some(a), m, min);
    assert!(gave(&w), "the Gave deed is held first-hand");
    let d = w.district_of_building(m);
    assert!(w.rumours[d.index()].entries.iter().any(|e| e.deed == Deed::Gave && e.actor == Some(a)), "and posted");
    // The deed is harmless: no grudge on the donor from the gift.
    assert!(citysim::systems::memory::deed_of(a, w.comp::<Memory>(a).and_then(|mm| mm.heard.last()).expect("entry"))
        .is_some_and(|r| r.deed == Deed::Gave));
}

/// E27 (plan 3b.6): the 12:00 Statistical pass serves a Statistical adult
/// within `reach_tiles` who is hungry and broke, counts the meal into its
/// district's column, and leaves one out of reach alone.
#[test]
fn test_stat_daily_serves_statistical_poor_in_reach() {
    let mut w = world();
    let m = mission(&w);
    charity::donate(&mut w, None, m, 500);
    charity::daily(&mut w);
    assert!(stock(&w, m) > 0);
    let md = w.comp::<Building>(m).map(|b| b.door).expect("door");
    let reach = w.config.charity.reach_tiles;
    let near = w
        .citizens()
        .into_iter()
        .find(|&a| {
            demography::is_adult(&w, a)
                && w.has::<Brain>(a)
                && !w.has::<Job>(a)
                && w.comp::<Household>(a)
                    .and_then(|h| h.home)
                    .and_then(|h| w.comp::<Building>(h))
                    .is_some_and(|b| b.door.manhattan(md) <= reach)
        })
        .expect("a neighbour");
    let far = w
        .citizens()
        .into_iter()
        .find(|&a| {
            demography::is_adult(&w, a)
                && w.has::<Brain>(a)
                && w.comp::<Household>(a)
                    .and_then(|h| h.home)
                    .and_then(|h| w.comp::<Building>(h))
                    .is_some_and(|b| b.door.manhattan(md) > 3 * reach)
        })
        .expect("a stranger");
    for &a in &[near, far] {
        lod::set_lod(&mut w, a, Lod::Statistical);
        set_coins(&mut w, a, 0);
        set_hunger(&mut w, a, 0.2);
    }
    let d = w.district_of_building(w.comp::<Household>(near).and_then(|h| h.home).expect("home")).index();
    charity::stat_daily(&mut w);
    assert!(hunger(&w, near) > 0.2, "served");
    assert!((hunger(&w, far) - 0.2).abs() < 1e-6, "out of reach");
    assert!(w.stats.current.econ.mission_meals >= 1);
    assert_eq!(w.stats.current.econ.d_meals.get(d).copied().unwrap_or(0) >= 1, d < citysim::stats::DISTRICT_SLOTS);
}

/// E27: a Mission whose purse covers a cot is a Hotel at price 0: the
/// homeless neighbour checks in for nothing, the cot is counted and the
/// `Alms` line carries its value.
#[test]
fn test_mission_cot_is_free_and_counted() {
    let mut w = world();
    let m = mission(&w);
    charity::donate(&mut w, None, m, 50);
    let md = w.comp::<Building>(m).map(|b| b.door).expect("door");
    let a = w
        .citizens()
        .into_iter()
        .find(|&a| demography::is_adult(&w, a) && w.has::<Brain>(a) && !w.has::<Job>(a) && w.gang_of(a).is_none())
        .expect("an adult");
    w.set_home(a, None);
    w.stand_at_door(a, m);
    if let Some(p) = w.comp_mut::<Position>(a) {
        p.tile = md;
    }
    set_coins(&mut w, a, 0);
    assert!(street::is_hotel(&w, m));
    assert_eq!(street::hotel_price(&w, m), 0);
    assert_eq!(street::hotel_for(&w, a), Some(m), "the free cot comes first");
    w.enter_building(a, m);
    let alms0 = w.stats.current.econ.flow_alms;
    assert_eq!(street::check_in(&mut w, a, m), Ok(()));
    assert_eq!(w.comp::<Wallet>(a).map(|x| x.coins), Some(0), "nothing paid");
    assert_eq!(purse(&w, m), 50, "the purse pays nobody");
    assert_eq!(w.stats.current.econ.mission_cots, 1);
    assert_eq!(w.stats.current.econ.flow_alms - alms0, w.config.charity.cot_price);
    assert_eq!(street::booked_hotel(&w, a), Some(m));
}

/// E30 (plan 3b.6): a corp under Lobby founds a Mission in its district
/// when none stands and it holds `10 × found_cost.mission`, then gives
/// `lobby_gift` to it on the next term.
#[test]
fn test_lobby_corp_gives_or_founds() {
    let mut w = world();
    let corp = w
        .corps()
        .into_iter()
        .find(|&c| w.comp::<citysim::Corp>(c).is_some_and(|cc| cc.slot == Some(0)))
        .expect("corp 1");
    if let Some(c) = w.comp_mut::<citysim::Corp>(corp) {
        c.treasury = 20_000;
    }
    let n0 = charity::missions(&w).len();
    let id0 = identity(&w);
    charity::lobby(&mut w, corp);
    let missions = charity::missions(&w);
    assert_eq!(identity(&w), id0);
    // Either a Mission stood in the corp's district (a gift) or one was founded.
    if missions.len() == n0 + 1 {
        assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Founded && e.text.contains("philanthropy")));
        let m = *missions
            .iter()
            .find(|&&m| w.comp::<Building>(m).is_some_and(|b| b.kind == BuildingKind::Mission && b.owner.is_none()))
            .expect("founded");
        let _ = m;
    } else {
        assert_eq!(missions.len(), n0);
        assert!(missions.iter().any(|&m| purse(&w, m) >= w.config.charity.lobby_gift), "the gift landed");
    }
    // The next call finds a Mission in the district: the gift.
    let total0: i64 = missions.iter().map(|&m| purse(&w, m)).sum();
    charity::lobby(&mut w, corp);
    let total1: i64 = charity::missions(&w).iter().map(|&m| purse(&w, m)).sum();
    assert_eq!(total1 - total0, w.config.charity.lobby_gift);
}

/// E26: a lawful, humble `Register` founder may choose a Mission; a proud
/// or lawless one never does; the Mission's cost is priced only with
/// charities on.
#[test]
fn test_register_founder_rule_for_missions() {
    let mut w = world();
    let a = civilian(&w, &[]);
    if let Some(p) = w.comp_mut::<Personality>(a) {
        p.lawfulness = 0.9;
        p.pride = 0.1;
    }
    assert!(charity::founder_ok(&w, a));
    if let Some(p) = w.comp_mut::<Personality>(a) {
        p.pride = 0.8;
    }
    assert!(!charity::founder_ok(&w, a));
    assert_eq!(
        citysim::systems::founding::found_cost(&w, BuildingKind::Mission),
        Some(w.config.corps.found_cost.mission)
    );
    let mut cfg = Config::load();
    cfg.charity.enabled = false;
    let off = World::new(42, cfg);
    assert_eq!(citysim::systems::founding::found_cost(&off, BuildingKind::Mission), None);
}

/// E27, E34: over a few days with an endowed kitchen the Mission serves
/// (body and Statistical), the day's `MissionServed` is logged and the
/// columns move; the Volunteers were hired at wage 0.
#[test]
fn test_endowed_mission_serves_over_days() {
    let mut w = world();
    let m = mission(&w);
    charity::god_donate(&mut w, m, 2_000).expect("gift");
    // (The identity drifts over days at EC_BASE: immigrants, emigrants and
    // the fence are phase 1's census; a Mission's day moves no coin but
    // through `charity_in`/`charity_out`, checked above.)
    run_days(&mut w, 4);
    let meals: u32 = w.stats.history.iter().map(|r| r.econ.mission_meals).sum();
    assert!(meals > 0, "meals served over four days: {meals}");
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::MissionServed));
    let served: u32 = w
        .comp::<Building>(m)
        .and_then(|b| b.charity.as_ref())
        .map_or(0, |c| c.served.iter().map(|&x| u32::from(x)).sum());
    assert!(served > 0);
    let volunteers = ownership::staff_at(&w, m);
    assert!(!volunteers.is_empty(), "Volunteers hired");
    assert!(volunteers
        .iter()
        .all(|&v| w.comp::<Job>(v).is_some_and(|j| j.role == Role::Volunteer && j.wage_per_day == 0)));
    assert!(w.stats.history.iter().any(|r| r.econ.d_dregs.iter().sum::<u32>() > 0), "the Dreg column is live");
}
