//! Jobs & lives phase 2, the street (docs/JOBS_AND_LIVES.md § 3, § 12's
//! unit tests): Pitches (opening under the district cap, closing on a hire,
//! the gang's cut, sweeps), vendors, the labour corner's dawn pick and its
//! day posts, couriers, cabs and rickshaws, tinkers and a body's Pitch
//! goal. Each test is the shipped seed-42 city (the street on, as shipped)
//! and one mechanism, called directly with its chance forced where it is a
//! roll (most fire rarely).

mod common;

use citysim::components::{Controller, Pitch, Stance};
use citysim::systems::{econ, industry, streetwork};
use citysim::{Brain, BuildingKind, Config, EntityId, EventKind, GoalKind, Job, Role, World, TICKS_PER_HOUR};
use common::{jobless_housed, set_coins};

fn world_with(f: impl FnOnce(&mut Config)) -> World {
    let mut c = Config::load();
    c.jobs2.enabled = true;
    c.jobs2.street = true;
    f(&mut c);
    World::new(42, c)
}

fn world() -> World {
    world_with(|_| {})
}

fn trade(w: &World, key: &str) -> Role {
    w.config.trade_role(key).unwrap_or_else(|| panic!("trade {key} configured"))
}

fn j2(w: &World) -> citysim::stats::Jobs2Cols {
    w.stats.current.jobs2.clone().unwrap_or_default()
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

/// Jobless free adults, ascending.
fn jobless(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            citysim::systems::law::living(w, a)
                && citysim::systems::demography::is_adult(w, a)
                && !w.has::<Job>(a)
                && !w.has::<citysim::Sentence>(a)
                && w.comp::<Brain>(a).is_some_and(|b| !b.emigrating)
        })
        .collect()
}

/// A vendor's Pitch for `a` at its home district's first spot (by hand).
fn vendor_pitch(w: &mut World, a: EntityId) -> Pitch {
    let home = citysim::systems::contracts::home_door(w, a)
        .or_else(|| w.comp::<citysim::Position>(a).map(|p| p.tile))
        .expect("a place");
    let d = w.district_of(home);
    let spot = w.spots.get(d.index()).and_then(|s| s.first()).map_or(home, |s| s.tile);
    let trade = trade(w, "vendor").trade().expect("a trade");
    let p = Pitch {
        spot,
        district: d,
        trade,
        owner: a,
        since: w.tick,
        take_today: 0,
        take_week: 0,
        cut_to: None,
        stock: 0,
        closed_until: None,
        dry_days: 0,
        jobs_today: 0,
        cut_paid: 0,
    };
    w.jobs2.pitches.insert(a, p.clone());
    p
}

/// Run to the next tick of day `tod`.
fn run_to_tod(w: &mut World, tod: u64) {
    let day = citysim::TICKS_PER_DAY;
    let now = w.tick;
    let target = (now / day) * day + tod;
    let target = if target <= now { target + day } else { target };
    w.run_ticks(target - now);
}

/// § 1: the street off posts no street row and opens no Pitch.
#[test]
fn test_street_off_is_phase_one() {
    let mut w = world_with(|c| c.jobs2.street = false);
    assert!(!streetwork::on(&w));
    let yard = w.buildings_of_kind(BuildingKind::FreightYard)[0];
    assert_eq!(citysim::systems::jobs::places_of(&w, yard, trade(&w, "courier")), 0);
    w.run_ticks(citysim::TICKS_PER_DAY * 2);
    assert!(w.jobs2.pitches.is_empty() && w.jobs2.idle_since.is_empty());
    assert!(w.stats.history.iter().all(|r| r.jobs2.as_ref().is_none_or(|j| j.pitches == 0 && j.day_posts_filled == 0)));
}

/// § 3: the midnight openings: an idle jobless adult with the stake opens a
/// Pitch in his district (on `p_pitch_open` forced to 1), up to the cap;
/// one under the stake does not.
#[test]
fn test_pitch_opens_under_the_cap_with_the_stake() {
    let mut w = world_with(|c| {
        c.jobs2.p_pitch_open = 1.0;
        c.jobs2.pitch_after_days = 0;
        c.jobs2.pitches_per_district = 1;
    });
    let idle = jobless(&w);
    for &a in &idle {
        set_coins(&mut w, a, 0);
    }
    let (rich, poor) = (idle[0], idle[1]);
    set_coins(&mut w, rich, 50);
    run_to_tod(&mut w, 0);
    w.run_ticks(1);
    assert!(w.jobs2.pitches.contains_key(&rich), "the staked idle adult opened none");
    assert!(!w.jobs2.pitches.contains_key(&poor), "a broke adult opened one");
    let mut per: std::collections::BTreeMap<usize, usize> = Default::default();
    for p in w.jobs2.pitches.values() {
        *per.entry(p.district.index()).or_default() += 1;
    }
    assert!(per.values().all(|&n| n <= 1), "the district cap: {per:?}");
    assert!(count(&w, EventKind::PitchOpened) >= 1);
}

/// § 3: a Pitch closes at the midnight after its owner is hired.
#[test]
fn test_pitch_closes_on_a_hire() {
    let mut w = world();
    let a = jobless(&w)[0];
    vendor_pitch(&mut w, a);
    let farm = w.buildings_of_kind(BuildingKind::Farm)[0];
    citysim::systems::demography::hire(&mut w, a, farm, Role::Farmer);
    run_to_tod(&mut w, 0);
    w.run_ticks(1);
    assert!(!w.jobs2.pitches.contains_key(&a));
    assert!(w.events.iter().any(|e| e.kind == EventKind::PitchClosed && e.text.contains("hired")));
}

/// § 3: the gang holding the district takes `cut_frac` of the day's take
/// (`StreetCut`); a `Sweep` district fines the Pitch and closes it a day.
#[test]
fn test_street_cut_and_sweep() {
    let mut w = world_with(|c| c.jobs2.p_pitch_sweep = 1.0);
    let a = jobless(&w)[0];
    let p = vendor_pitch(&mut w, a);
    let g = w.gangs()[0];
    let d = p.district.index();
    w.districts[d].control = Controller::Gang(g);
    w.districts[d].stance = Stance::Sweep;
    w.jobs2.pitches.get_mut(&a).expect("pitch").take_today = 100;
    set_coins(&mut w, a, 200);
    let gang0 = w.comp::<citysim::Gang>(g).expect("gang").treasury;
    let treasury0 = w.purse(None);
    let id0 = econ::identity(&w);
    // The midnight pass itself (the districts' own midnight would recount
    // the control set here first): the world stands at tick 0.
    streetwork::daily(&mut w);
    let cut = (100.0 * w.config.jobs2.cut_frac).round() as i64;
    let gang1 = w.comp::<citysim::Gang>(g).expect("gang").treasury;
    assert!(gang1 >= gang0 + cut, "the cut {cut} did not reach the gang ({gang0} -> {gang1})");
    let x = w.jobs2.pitches.get(&a).expect("still open");
    assert_eq!(x.cut_to, Some(g));
    assert!(x.closed_until.is_some_and(|t| t > w.tick), "swept: closed a day");
    assert!(count(&w, EventKind::PitchSwept) >= 1);
    assert!(w.purse(None) >= treasury0, "the fine went to the Treasury");
    assert_eq!(econ::identity(&w), id0);
}

/// § 3 (`resale`): a working vendor buys his batch at the Market; an
/// off-screen synth buyer of his district takes a meal at the markup; the
/// coins and the meals balance.
#[test]
fn test_vendor_restock_and_sale_balance() {
    let mut w = world_with(|c| c.jobs2.p_vendor_pick = 1.0);
    let a = jobless(&w)[0];
    let p = vendor_pitch(&mut w, a);
    set_coins(&mut w, a, 100);
    run_to_tod(&mut w, 9 * TICKS_PER_HOUR);
    // At the stall when the hour strikes.
    if let Some(pos) = w.comp_mut::<citysim::Position>(a) {
        pos.tile = p.spot;
        pos.building = None;
    }
    w.comp_mut::<Brain>(a).expect("brain").lod = citysim::Lod::Statistical;
    w.run_ticks(1);
    let stock = w.jobs2.pitches.get(&a).expect("pitch").stock;
    assert!(stock > 0, "the vendor bought no batch");
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let buyer = w
        .citizens()
        .into_iter()
        .find(|&b| {
            b != a
                && citysim::systems::demography::is_adult(&w, b)
                && citysim::systems::contracts::home_door(&w, b).is_some_and(|t| w.district_of(t) == p.district)
        })
        .expect("a neighbour");
    set_coins(&mut w, buyer, 3);
    let id0 = econ::identity(&w);
    let (coins_a, coins_b) = (w.purse(Some(a)), w.purse(Some(buyer)));
    assert!(streetwork::stat_vendor_meal(&mut w, buyer, market), "no street meal");
    let paid = coins_b - w.purse(Some(buyer));
    assert!(paid > 0);
    let took = w.purse(Some(a)) - coins_a;
    assert!(took > 0 && took <= paid, "the vendor took the price less any tax ({took} of {paid})");
    assert_eq!(w.jobs2.pitches.get(&a).expect("pitch").stock, stock - 1);
    assert_eq!(j2(&w).vendor_meals, 1);
    assert_eq!(econ::identity(&w), id0);
}

/// § 3 (the labour corner, `day`): the dawn pick fills a Construction
/// Yard's day post from the corner's queue, the foreman's known contact
/// first; the post ends after its shift with no layoff.
#[test]
fn test_dawn_pick_contacts_first_and_the_day_post_ends() {
    let mut w = world_with(|c| {
        c.jobs2.p_corner = 1.0;
        c.jobs2.corner_stat_reach = 10_000;
        // The contact's rank alone: the queue spans the map (the reach), so
        // the island pass phase 1's job cap (a labourer within it first) is off.
        c.distance = None;
    });
    let yard = w.buildings_of_kind(BuildingKind::ConstructionYard)[0];
    let corp = w.corps()[0];
    let lot = citysim::systems::founding::vacant_lots(&w).into_iter().next().expect("a Lot");
    citysim::systems::founding::build_on_lot(&mut w, lot, BuildingKind::Bar, Some(corp)).expect("built");
    industry::pay_build(&mut w, Some(corp), 100, Some(lot), true);
    let foreman = industry::foreman(&w, yard, None).expect("the Yard has a foreman");
    let queue: Vec<EntityId> = jobless(&w)
        .into_iter()
        .filter(|&a| w.comp::<citysim::Personality>(a).is_some_and(|p| p.lawfulness >= 0.2))
        .take(40)
        .collect();
    for &a in &queue {
        set_coins(&mut w, a, 0);
    }
    let friend = *queue.last().expect("a queue");
    w.edge_entry(friend, foreman).affinity = 0.5;
    let hour = u64::from(w.config.jobs2.corner_hour);
    run_to_tod(&mut w, hour);
    w.run_ticks(1);
    let role = trade(&w, "day_labourer");
    let hired: Vec<EntityId> = w.workers(role).to_vec();
    assert!(!hired.is_empty(), "no day post filled");
    assert!(hired.contains(&friend), "the foreman's contact was not picked first");
    let until = w.comp::<Job>(friend).and_then(|j| j.until).expect("a day post");
    assert!(count(&w, EventKind::DawnPick) >= 1);
    let fired = count(&w, EventKind::Fire) + count(&w, EventKind::LaidOff);
    w.run_ticks(until - w.tick + TICKS_PER_HOUR);
    assert!(!w.has::<Job>(friend) || w.comp::<Job>(friend).is_some_and(|j| j.role != role), "the day post did not end");
    assert_eq!(count(&w, EventKind::Fire) + count(&w, EventKind::LaidOff), fired, "a day post's end is no layoff");
}

/// § 3 (couriers): a corp's parcels pay the Yard `parcel_fee` each up to
/// its couriers' day; its buildings near the Yard join their round.
#[test]
fn test_couriers_carry_parcels_for_a_fee() {
    let mut w = world();
    let id0 = econ::identity(&w);
    let yard = w.buildings_of_kind(BuildingKind::FreightYard)[0];
    let courier = trade(&w, "courier");
    for a in jobless(&w).into_iter().take(2) {
        citysim::systems::demography::hire(&mut w, a, yard, courier);
    }
    run_to_tod(&mut w, 0);
    w.run_ticks(1);
    let row = w.stats.history.back().and_then(|r| r.jobs2.clone()).unwrap_or_default();
    let today = j2(&w);
    let parcels = today.parcels.max(row.parcels);
    assert!(parcels > 0, "no parcel carried");
    assert_eq!(today.parcel_fees.max(row.parcel_fees), i64::from(parcels) * w.config.jobs2.parcel_fee);
    assert_eq!(econ::identity(&w), id0);
}

/// § 3 (cabs and rickshaws): in the shift's first hour, a long commuter
/// with the fare takes the home district's rickshaw first, else a cab.
#[test]
fn test_stat_cab_rickshaw_first_then_cab() {
    let mut w = world_with(|c| {
        c.jobs2.p_cab = 1.0;
        c.jobs2.cab_min_tiles = 0;
    });
    let worker = w
        .citizens()
        .into_iter()
        .find(|&a| {
            w.comp::<Job>(a).is_some_and(|j| j.shifts.first().is_some_and(|s| s.0 == 540) && j.employer.is_some())
                && citysim::systems::contracts::home_door(&w, a).is_some()
        })
        .expect("a day worker with a home");
    set_coins(&mut w, worker, 500);
    let job = w.comp::<Job>(worker).cloned().expect("job");
    // A rickshaw in his home district, working.
    let puller = jobless(&w)[0];
    let home = citysim::systems::contracts::home_door(&w, worker).expect("home");
    let mut p = vendor_pitch(&mut w, puller);
    p.trade = trade(&w, "rickshaw").trade().expect("trade");
    p.district = w.district_of(home);
    w.comp_mut::<Brain>(puller).expect("brain").lod = citysim::Lod::Statistical;
    w.jobs2.pitches.insert(puller, p);
    run_to_tod(&mut w, 540);
    // Lives at work phase 4a: the city's Full commuters ride too (the same
    // roll at `p_cab` 1): read the deltas.
    let (before, r0) = (w.purse(Some(puller)), j2(&w).rickshaw_trips);
    assert!(streetwork::stat_cab(&mut w, worker, &job), "no ride");
    assert!(w.purse(Some(puller)) > before, "the rickshaw took no fare");
    assert_eq!(j2(&w).rickshaw_trips - r0, 1);
    // No rickshaw: a cab at a Garage with drivers.
    w.jobs2.pitches.clear();
    let g = w.buildings_of_kind(BuildingKind::Garage)[0];
    let driver = jobless(&w)[0];
    let cab = trade(&w, "cab_driver");
    citysim::systems::demography::hire(&mut w, driver, g, cab);
    let c0 = j2(&w).cab_trips;
    assert!(streetwork::stat_cab(&mut w, worker, &job), "no cab");
    assert_eq!(j2(&w).cab_trips - c0, 1);
}

/// § 3 (tinkers): an agent's vehicle near a working tinker is fixed at the
/// discount or wrecked on `tinker_fail_p`.
#[test]
fn test_tinker_fixes_cheap_or_wrecks() {
    for (fail, fixed) in [(0.0, true), (1.0, false)] {
        let mut w = world_with(|c| c.jobs2.tinker_fail_p = fail);
        let tinker = jobless(&w)[0];
        let mut p = vendor_pitch(&mut w, tinker);
        p.trade = trade(&w, "tinker").trade().expect("trade");
        w.jobs2.pitches.insert(tinker, p.clone());
        w.comp_mut::<Brain>(tinker).expect("brain").lod = citysim::Lod::Statistical;
        run_to_tod(&mut w, 10 * TICKS_PER_HOUR);
        let owner = jobless(&w)[1];
        set_coins(&mut w, owner, 500);
        let id0 = econ::identity(&w);
        let before = w.purse(Some(tinker));
        let r = streetwork::tinker_repair(&mut w, EntityId::NONE, owner, p.spot, 100);
        assert_eq!(r, Some(fixed));
        let price = (100.0 * w.config.jobs2.tinker_discount).round() as i64;
        let took = w.purse(Some(tinker)) - before;
        assert!(took > 0 && took <= price, "the tinker took the discounted price less tax ({took} of {price})");
        assert_eq!(count(&w, EventKind::TinkerBotched), usize::from(!fixed));
        assert_eq!(econ::identity(&w), id0);
    }
}

/// Phase-8 review finding 4: a botched street repair wrecks the vehicle
/// (gone from the world and the vehicle index, its Parts left); on d2eea01
/// it stayed a zombie at condition 0.
#[test]
fn test_botched_repair_wrecks_the_vehicle() {
    let mut w = world_with(|c| c.jobs2.tinker_fail_p = 1.0);
    let tinker = jobless(&w)[0];
    let mut p = vendor_pitch(&mut w, tinker);
    p.trade = trade(&w, "tinker").trade().expect("trade");
    w.jobs2.pitches.insert(tinker, p.clone());
    w.comp_mut::<Brain>(tinker).expect("brain").lod = citysim::Lod::Statistical;
    run_to_tod(&mut w, 10 * TICKS_PER_HOUR);
    let owner = jobless(&w)[1];
    set_coins(&mut w, owner, 500);
    let v = citysim::systems::assets::spawn_asset(
        &mut w,
        citysim::AssetKind::Motorcycle,
        1,
        Some(owner),
        citysim::AssetLoc::Carried(owner),
        60,
    );
    assert!(w.has::<citysim::components::Asset>(v));
    let r = streetwork::tinker_repair(&mut w, v, owner, p.spot, 100);
    assert_eq!(r, Some(false), "botched");
    assert!(!w.has::<citysim::components::Asset>(v), "the vehicle is wrecked, not left at condition 0");
    assert!(!w.vehicles.contains(&v));
    assert_eq!(count(&w, EventKind::Wrecked), 1);
}

/// § 3 (`GoalKind::Pitch`): a body with a Pitch walks to it in its hours
/// and stands it (`Vend`).
#[test]
fn test_a_body_stands_its_pitch() {
    let mut w = world();
    let a = jobless(&w)[0];
    let p = vendor_pitch(&mut w, a);
    set_coins(&mut w, a, 60);
    run_to_tod(&mut w, 9 * TICKS_PER_HOUR);
    let mut stood = false;
    let mut goal_seen = false;
    for _ in 0..(4 * TICKS_PER_HOUR) {
        w.comp_mut::<Brain>(a).expect("brain").pinned = true;
        if let Some(n) = w.comp_mut::<citysim::Needs>(a) {
            n.hunger = 1.0;
            n.energy = 1.0;
        }
        w.run_ticks(1);
        goal_seen |= w.comp::<Brain>(a).and_then(|b| b.plan_goal()) == Some(GoalKind::Pitch);
        let here = w.comp::<citysim::Position>(a).map(|x| (x.building, x.tile));
        if here.is_some_and(|(b, t)| b.is_none() && t.manhattan(p.spot) <= w.config.jobs2.pitch_reach) {
            stood = true;
            break;
        }
    }
    assert!(goal_seen, "the Pitch goal never won");
    assert!(stood, "never stood at the Pitch");
}

/// The carry-over: a corp's Utility Plant dark on `seize_after_days` days
/// running passes to the city (the Nationalise path: the Treasury pays its
/// value); a plant lit again resets the count.
#[test]
fn test_a_plant_dark_for_days_is_seized_by_the_city() {
    let mut w = world();
    let id0 = econ::identity(&w);
    let p = w.buildings_of_kind(BuildingKind::UtilityPlant)[0];
    let owner = w.owner_of(p).expect("a corp's plant");
    let limit = w.config.jobs2.seize_after_days;
    w.jobs2.plant_dark_days.insert(p, limit - 1);
    w.jobs2.plant_dark_today.insert(p);
    let (corp0, city0) = (w.purse(Some(owner)), w.purse(None));
    industry::daily(&mut w);
    assert_eq!(w.owner_of(p), None, "the city took the plant");
    let value = citysim::systems::ownership::value(&w, BuildingKind::UtilityPlant);
    assert!(w.purse(Some(owner)) >= corp0 + value - 1 && w.purse(None) <= city0 - value + 1);
    assert_eq!(j2(&w).plants_seized, 1);
    assert_eq!(econ::identity(&w), id0);
    // A lit day resets the count.
    w.jobs2.plant_dark_days.insert(p, 3);
    industry::daily(&mut w);
    assert!(!w.jobs2.plant_dark_days.contains_key(&p));
}

/// Saves: a world with Pitches, idle days and the day's parcels runs on
/// byte for byte after a round trip.
#[test]
fn test_save_round_trip_with_the_street() {
    let mut w = world_with(|c| {
        c.jobs2.p_pitch_open = 1.0;
        c.jobs2.pitch_after_days = 0;
    });
    let a = jobless(&w)[0];
    vendor_pitch(&mut w, a);
    w.run_ticks(citysim::TICKS_PER_DAY + 7 * TICKS_PER_HOUR);
    assert!(!w.jobs2.pitches.is_empty());
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text);
    w.run_ticks(3 * TICKS_PER_HOUR);
    back.run_ticks(3 * TICKS_PER_HOUR);
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w));
}

// ---------------------------------------------------------------------------
// Shadow V4 bug 6 (docs/SHADOW_V4.md): the elder's pitch at the nearest spot.
// ---------------------------------------------------------------------------

/// Bug 6 (the pitch): an elder opener with a rickshaw-puller's arms vends
/// (3 of 4 street workers were 64-68), at the free spot of his district
/// nearest his home (the first free one meant 47-168 min walks).
#[test]
fn test_bug6_elder_opens_a_vendor_pitch_at_the_nearest_spot() {
    let mut w = world_with(|c| {
        c.jobs2.p_pitch_open = 1.0;
        c.jobs2.pitch_after_days = 0;
        c.jobs2.pitches_per_district = 50;
    });
    // The HangOut spots (the pitches' sites) are laid out in the first day.
    w.run_ticks(citysim::TICKS_PER_DAY);
    let idle = jobless_housed(&w);
    // No other opener (housed or not) holds the stake.
    for a in w.citizens() {
        if w.has::<citysim::Wallet>(a) && !w.has::<Job>(a) {
            set_coins(&mut w, a, 0);
        }
    }
    // One whose home district has spots (two or more free).
    let used: Vec<citysim::TilePos> = w.jobs2.pitches.values().map(|p| p.spot).collect();
    let a = *idle
        .iter()
        .find(|&&x| {
            citysim::systems::contracts::home_door(&w, x).is_some_and(|h| {
                w.spots
                    .get(w.district_of(h).index())
                    .is_some_and(|s| s.iter().filter(|t| !used.contains(&t.tile)).count() >= 2)
            })
        })
        .expect("an opener with spots at home");
    set_coins(&mut w, a, 50);
    w.comp_mut::<citysim::Skills>(a).expect("skills").fighting = 0.95;
    w.comp_mut::<citysim::Skills>(a).expect("skills").knowledge = 0.0;
    let elder = (w.config.ages.frail_from_years + 6) * citysim::time::DAYS_PER_YEAR as u32;
    w.comp_mut::<citysim::Identity>(a).expect("id").age_days = elder;
    let home = citysim::systems::contracts::home_door(&w, a).expect("a home");
    let d = w.district_of(home);
    let nearest = w.spots[d.index()]
        .iter()
        .map(|x| x.tile)
        .filter(|t| !used.contains(t))
        .map(|t| t.manhattan(home))
        .min()
        .expect("a free spot");
    streetwork::daily(&mut w);
    let p = w.jobs2.pitches.get(&a).expect("the elder opened a pitch");
    let kind = w.config.trade(p.trade).and_then(streetwork::pitch_kind);
    assert_eq!(kind, Some(streetwork::PitchKind::Vendor), "an elder vends");
    assert_eq!(p.spot.manhattan(home), nearest, "the free spot nearest home");
}

// ---------------------------------------------------------------------------
// The island pass phase 8 (docs/ISLAND_PASS.md I4; OPEN-168): the dead
// trades' bugs. Each test runs the mechanism with `[island_trades]` and
// shows the base (the section dropped, `CITYSIM_TRADES_OFF=1`) failing it.
// ---------------------------------------------------------------------------

/// The world with `[island_trades]` (`on`) or without it (the base).
fn trades_world(on: bool, f: impl FnOnce(&mut Config)) -> World {
    world_with(|c| {
        if !on {
            c.island_trades = None;
        }
        f(c);
    })
}

fn door_of(w: &World, b: EntityId) -> citysim::TilePos {
    w.comp::<citysim::Building>(b).expect("a building").door
}

/// Phase 8 (OPEN-168, the courier): with `[jobs2] site_reach` 0 every corp's
/// buildings lie past the tile radius of every Yard (the island's Yards sit
/// 100-1,000 tiles from what they serve), and the base names a parcel only
/// to a building at the Yard's own door (the Yard, its company floors): the
/// couriers carry nothing off the Yard. The fix routes a corp's parcels
/// through the Yard with its staffed buildings within `parcel_cap_ticks` of
/// travel, and every named parcel's address lies within the cap.
#[test]
fn test_parcels_go_to_recipients_in_travel_reach() {
    let named = |on: bool| {
        let mut w = trades_world(on, |c| c.jobs2.site_reach = 0);
        w.jobs2.parcels.clear();
        streetwork::daily(&mut w);
        let cap = w.config.island_trades.as_ref().map_or(u64::MAX, |c| c.parcel_cap_ticks);
        for p in &w.jobs2.parcels {
            let t = citysim::systems::distance::travel(&w, door_of(&w, p.yard), door_of(&w, p.at));
            assert!(t <= cap, "parcel to {} is {t} ticks from its Yard (cap {cap})", p.at.index);
            assert!(!w.staff_of(p.at).is_empty(), "a recipient building with staff to sign");
        }
        // (named off the Yard's door, named)
        let away = w.jobs2.parcels.iter().filter(|p| door_of(&w, p.at) != door_of(&w, p.yard)).count();
        (away, w.jobs2.parcels.len())
    };
    let (on, off) = (named(true), named(false));
    eprintln!("parcels off the Yard's door: on {on:?}, base {off:?}");
    assert!(on.0 > 0 && on.0 == on.1, "every parcel named to a building off the Yard: {on:?}");
    assert_eq!(off.0, 0, "the base names none off the Yard past site_reach (the test's precondition): {off:?}");
}

/// Phase 8 (OPEN-168, the builder): a Construction Yard whose open sites all
/// lie past `[shift.haul] haul_reach` ran every trip inside the Yard (the
/// island's nearest Yard is 170-1,100 tiles from a site). With `site_day`
/// the Builder carries a load to the nearest site, sets it down there and
/// works the rest of the shift on it; the shift closes and is paid.
#[test]
fn test_builder_works_a_far_site_for_the_day() {
    let builder_role = |w: &World| trade(w, "builder");
    let setup = |on: bool| {
        let mut w = trades_world(on, |_| {});
        w.run_ticks(citysim::TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
        let role = builder_role(&w);
        let reach = w.config.shift.as_ref().and_then(|c| c.haul.as_ref()).map_or(0, |h| h.haul_reach);
        let (a, yard) = w
            .citizens()
            .into_iter()
            .filter(|&a| citysim::systems::law::living(&w, a) && !w.has::<citysim::Sentence>(a))
            .find_map(|a| {
                let j = w.comp::<Job>(a)?;
                let e = j.employer?;
                let key = j.next_shift_key(w.tick);
                (j.role == role
                    && j.shifts.len() == 1
                    && j.shifts.iter().all(|&(s, e)| s >= 360 && e <= 1200)
                    && citysim::exec::routine::workday_of(&w, a, j, key)
                    && w.comp::<citysim::Household>(a).is_some_and(|h| h.home.is_some()))
                .then_some((a, e))
            })
            .expect("a Builder on a day shift");
        // One open site, past haul_reach and an hour or two away.
        let yd = door_of(&w, yard);
        let site = w
            .buildings_by_kind
            .get(&BuildingKind::Home)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&b| w.comp::<citysim::Building>(b).is_some_and(|x| !x.demolished && !x.derelict))
            .map(|b| (citysim::systems::distance::travel(&w, yd, door_of(&w, b)), b))
            .filter(|&(t, b)| (60..=120).contains(&t) && door_of(&w, b).manhattan(yd) > reach)
            .min()
            .map(|(_, b)| b)
            .expect("a Block an hour or two from the Yard");
        let tick = w.tick;
        w.jobs2.sites.retain(|_, &mut (y, _)| y != yard);
        w.jobs2.sites.insert(site, (yard, tick));
        (w, a, yard, site)
    };
    // The base: no destination in reach, the trips stay inside the Yard.
    let (w, a, yard, _) = setup(false);
    assert!(citysim::systems::shift::haul_dests(&w, a, yard).is_empty(), "the base has no far site");
    let (mut w, a, yard, site) = setup(true);
    assert_eq!(citysim::systems::shift::haul_dests(&w, a, yard), vec![site], "the far site is the day's destination");
    citysim::systems::lod::set_lod(&mut w, a, citysim::Lod::Full);
    let key = w.comp::<Job>(a).expect("job").next_shift_key(w.tick);
    let before = j2(&w).site_days;
    let (mut unload, mut work) = (false, false);
    let end = w.tick + 16 * TICKS_PER_HOUR;
    while w.tick < end {
        w.comp_mut::<Brain>(a).expect("brain").pinned = true;
        w.run_ticks(1);
        let here = w.comp::<citysim::Position>(a).and_then(|p| p.building);
        if let citysim::ExecState::Use { kind, .. } = w.comp::<Brain>(a).expect("brain").exec {
            unload |= here == Some(site) && kind == citysim::ActionKind::Unload;
            work |= here == Some(site) && kind.is_work();
        }
    }
    eprintln!("builder {} yard {} site {}: unload {unload}, work {work}", a.index, yard.index, site.index);
    assert!(unload, "the load set down at the site");
    assert!(work, "the rest of the shift worked at the site");
    assert!(j2(&w).site_days > before, "the site day counted");
    assert_eq!(w.comp::<Job>(a).expect("job").last_shift_day, Some(key), "the shift closed (paid) at the site");
}
