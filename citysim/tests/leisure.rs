//! Life pass L2 phase 2 (docs/LIFE_L2.md § 2, § 10; plan phase 2): the fun
//! need at every tier, the rung ladder, the venues' money and doors,
//! HangOut's spot scoring, the Statistical evening, fronts and the leader's
//! Collect, and the keyed streams.

mod common;

use citysim::exec::actions;
use citysim::goap::ActionKind;
use citysim::systems::{demography, leisure, lod, ownership};
use citysim::{
    Brain, Building, BuildingKind, Config, EntityId, Gang, Household, Job, Lod, Needs, Personality, Position, Role,
    World, TICKS_PER_DAY,
};
use common::{coins, day_workers, jobless_housed, set_coins};
use rand::Rng;

fn standing(w: &World, kind: BuildingKind) -> Vec<EntityId> {
    w.buildings_of_kind(kind)
        .iter()
        .copied()
        .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect()
}

/// A housed, jobless adult who is no exec and in no gang.
fn civilian(w: &World, skip: &[EntityId]) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            !skip.contains(&a)
                && demography::is_adult(w, a)
                && !w.has::<Job>(a)
                && w.gang_of(a).is_none()
                && w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
                && citysim::systems::life::exec_corp(w, a).is_none()
                && citysim::systems::classes::class_of(w, a) == citysim::Class::Street
        })
        .expect("a civilian")
}

/// § 1: an Enjoy charges the venue's price at start; coins are conserved
/// throughout.
#[test]
fn test_enjoy_charges_price_and_keeps_it_on_abort() {
    // L2 shadow fixes item 4: a started purchase is atomic (an abandoned
    // entry is not refunded: 12 food flows for 6 meals).
    let mut w = World::new(42, Config::load().dole_city());
    let club = standing(&w, BuildingKind::Arcade)[0];
    let a = civilian(&w, &[]);
    set_coins(&mut w, a, 100);
    w.enter_building(a, club);
    let price = leisure::price_of(&w, club);
    assert!(price > 0);
    let total = ownership::total_coins(&w);
    assert!(actions::can_start(&w, a, ActionKind::Enjoy, Some(club)));
    actions::on_start(&mut w, a, ActionKind::Enjoy, Some(club));
    assert_eq!(coins(&w, a), 100 - price, "the entry is paid at start");
    assert_eq!(ownership::total_coins(&w), total, "conserved at start");
    let now = w.tick;
    actions::on_abort(&mut w, a, ActionKind::Enjoy, now);
    assert_eq!(coins(&w, a), 100 - price, "a started entry is not refunded");
    assert_eq!(ownership::total_coins(&w), total, "conserved");
    // Broke: no start, nothing moves.
    set_coins(&mut w, a, 0);
    assert!(!actions::can_start(&w, a, ActionKind::Enjoy, Some(club)));
}

/// L18: table games conserve coins; a house that cannot pay a win shuts
/// its table for the day.
#[test]
fn test_gamble_conserves_coins_and_short_house_shuts_table() {
    let mut w = World::new(42, Config::load().dole_city());
    let den = standing(&w, BuildingKind::Den)[0];
    let gambler = civilian(&w, &[]);
    let house = civilian(&w, &[gambler]);
    ownership::transfer_building(&mut w, den, Some(house));
    set_coins(&mut w, house, 0);
    set_coins(&mut w, gambler, 60);
    w.enter_building(gambler, den);
    let mut shut = false;
    for _ in 0..200 {
        // A fresh purse each round (the house stays broke: it never wins
        // enough to pay a stake back).
        set_coins(&mut w, gambler, 40);
        set_coins(&mut w, house, 0);
        if !actions::can_start(&w, gambler, ActionKind::Gamble, Some(den)) {
            shut = true;
            break;
        }
        let total = ownership::total_coins(&w);
        actions::on_start(&mut w, gambler, ActionKind::Gamble, Some(den));
        assert_eq!(ownership::total_coins(&w), total, "coins conserved");
        w.tick += 1;
    }
    let today = w.day();
    let v = w.comp::<Building>(den).and_then(|b| b.venue.clone()).expect("venue");
    assert!(shut && v.table_shut_day == Some(today), "a broke house shut its table: {v:?}");
}

/// L19: a Lounge refuses a Dreg; an exec (Corp, dressed) gets in.
#[test]
fn test_lounge_refuses_dreg() {
    let mut w = World::new(42, Config::load().dole_city());
    let lounge = standing(&w, BuildingKind::Lounge)[0];
    let a = civilian(&w, &[]);
    assert!(!leisure::door_ok(&w, a, lounge), "a Street dress-1 civilian is refused");
    w.comp_mut::<Household>(a).expect("household").home = None;
    assert_eq!(citysim::systems::classes::class_of(&w, a), citysim::Class::Dreg);
    assert!(!leisure::door_ok(&w, a, lounge), "a Dreg is refused");
    let exec = w.corps().into_iter().find_map(|c| w.comp::<citysim::Corp>(c).and_then(|cc| cc.exec)).expect("an exec");
    assert!(leisure::door_ok(&w, exec, lounge), "an exec gets in");
}

/// L19: a Club's door Host refuses a hot visitor when braver than the heat.
#[test]
fn test_club_door_refuses_heat() {
    let mut w = World::new(42, Config::load().dole_city());
    let club = standing(&w, BuildingKind::Club)[0];
    let a = civilian(&w, &[]);
    let host = civilian(&w, &[a]);
    demography::hire(&mut w, host, club, Role::Host);
    w.comp_mut::<Job>(host).expect("job").shifts = vec![(0, 1440)];
    w.comp_mut::<Personality>(host).expect("p").courage = 1.0;
    assert!(leisure::door_ok(&w, a, club), "a cold visitor gets in");
    let rep = citysim::word::Reputation { heat: 0.9, ..Default::default() };
    let i = a.index as usize;
    if w.reputation.len() <= i {
        w.reputation.resize(i + 1, None);
    }
    w.reputation[i] = Some(rep);
    assert!(!leisure::door_ok(&w, a, club), "heat 0.9 against a Host of courage 1.0");
    w.comp_mut::<Personality>(host).expect("p").courage = 0.5;
    assert!(leisure::door_ok(&w, a, club), "a timid Host lets heat through");
}

/// L16: a Friend whose habit is a Bar (on shift there) makes that Bar's
/// door beat a nearer spot with nobody expected.
#[test]
fn test_hangout_picks_spot_where_friend_habit_is() {
    // The pressure pass R2: the v1 pace (`common::v1_pace`).
    let mut cfg = Config::load().dole_city();
    common::v1_pace(&mut cfg);
    let mut w = World::new(42, cfg);
    let a = civilian(&w, &[]);
    let friend = civilian(&w, &[a]);
    // A Bar's door spot and a street tile 4-6 tiles off it.
    let mut setup = None;
    for bar in standing(&w, BuildingKind::Bar) {
        let bd = w.comp::<Building>(bar).expect("bar");
        let door = w.outside_door(bd);
        let near = (-6i32..=6)
            .flat_map(|dx| (-6i32..=6).map(move |dy| (dx, dy)))
            .filter_map(|(dx, dy)| {
                let (x, y) = (i32::from(door.x) + dx, i32::from(door.y) + dy);
                ((0..256).contains(&x) && (0..192).contains(&y))
                    .then_some(citysim::TilePos { x: x as u16, y: y as u16 })
            })
            .find(|&t| (4..=6).contains(&t.manhattan(door)) && w.is_street(t));
        if let Some(t) = near {
            setup = Some((bar, door, t));
            break;
        }
    }
    let (bar, bar_spot, near) = setup.expect("a Bar and a street tile near it");
    let spots = vec![
        citysim::living::Spot { tile: bar_spot, kind: citysim::living::SpotKind::Door(bar) },
        citysim::living::Spot { tile: near, kind: citysim::living::SpotKind::Barrel },
    ];
    w.spots = vec![spots; w.districts.len()];
    // The friend works the Bar all day; `a` stands on the other spot.
    demography::hire(&mut w, friend, bar, Role::Bartender);
    w.comp_mut::<Job>(friend).expect("job").shifts = vec![(0, 1440)];
    w.edge_entry(a, friend).affinity = 0.8;
    w.edge_entry(a, friend).kind = citysim::RelKind::Friend;
    w.leave_building(a);
    w.comp_mut::<Position>(a).expect("pos").tile = near;
    let (t, social) = leisure::best_spot(&w, a, near).expect("a spot");
    assert_eq!(t, bar_spot, "the Friend's Bar door wins (social {social})");
    // Without the friend the nearer spot wins.
    w.edge_entry(a, friend).affinity = 0.0;
    let (t2, _) = leisure::best_spot(&w, a, near).expect("a spot");
    assert_eq!(t2, near, "with nobody expected, the nearest");
}

/// § 2: a broke agent's only rung is free; with coins a paid one.
#[test]
fn test_unwind_skips_paid_rungs_when_broke() {
    let mut w = World::new(42, Config::load().dole_city());
    leisure::spots_daily(&mut w);
    let a = civilian(&w, &[]);
    w.comp_mut::<Needs>(a).expect("needs").fun = 0.2;
    w.comp_mut::<Needs>(a).expect("needs").hunger = 1.0;
    set_coins(&mut w, a, 0);
    let pick = leisure::choice(&w, a, true).expect("a free rung");
    assert_eq!(pick.rung, citysim::living::Rung::Free, "{pick:?}");
    // Lives at work phase 2: a Free pick may carry a Home as its venue (the
    // home spot, § 3.1), never a paid venue.
    let home = pick.venue.and_then(|v| w.comp::<Building>(v).map(|b| b.kind));
    assert!(home.is_none_or(|k| k == BuildingKind::Home), "{pick:?}");
    set_coins(&mut w, a, 300);
    // Day gaps 2: a venue in reach (here, at the Arcade's door).
    let arcade = standing(&w, BuildingKind::Arcade)[0];
    w.enter_building(a, arcade);
    let pick = leisure::choice(&w, a, true).expect("a rung");
    assert_ne!(pick.rung, citysim::living::Rung::Free, "{pick:?}");
    assert!(pick.venue.is_some());
}

/// L17: the Statistical evening pays a venue, raises fun, adds a visit.
/// Jobs & lives phase 7b: the table's `p_venue` (a kind's Full visits per
/// adult-day, shared over the adults eligible for it) decides who goes, so
/// one agent's evening is a roll; here every kind's rate is 1 a day, past
/// any one adult's share, so an eligible agent goes. The best-rung path (a
/// table without `p_venue`) is `test_stat_daily_best_rung_without_rates`.
#[test]
fn test_stat_daily_pays_venue_and_raises_fun() {
    let mut w = World::new(42, Config::load().dole_city());
    if let Some(t) = w.stat_table.as_mut() {
        let rates =
            t.p_venue.as_ref().expect("the shipped table learns p_venue").keys().map(|k| (k.clone(), 1.0)).collect();
        t.p_venue = Some(rates);
    }
    stat_evening_pays(w, &[]);
}

/// L17 without `p_venue` (a table from before Jobs & lives phase 7b): every
/// fun-starved Statistical adult takes the best rung.
#[test]
fn test_stat_daily_best_rung_without_rates() {
    let mut w = World::new(42, Config::load().dole_city());
    if let Some(t) = w.stat_table.as_mut() {
        t.p_venue = None;
    }
    // A Club, Arcade or Den in reach (a Bar's or a NoodleBar's walk-weighted
    // gain is under an hour's HangOut, the free rung).
    stat_evening_pays(w, &[BuildingKind::Club, BuildingKind::Arcade, BuildingKind::Den]);
}

fn stat_evening_pays(mut w: World, kinds: &[BuildingKind]) {
    // Phase 7b: at the evening's hour (the nearest venue in reach may be
    // shut at midnight), a civilian with a venue within the walk cap of its
    // day (the first civilian's Home reaches none since the evening mirrors
    // the Full ladder's cap).
    w.run_ticks(1259);
    // The 21:00 evening's own visit (the pre-view pass phase 3 draws a visit
    // to a venue with a counter body into the body's shift: tests/shift.rs).
    if let Some(s) = w.config.shift.as_mut() {
        s.counter = None;
    }
    let mut skip = Vec::new();
    let a = loop {
        let a = civilian(&w, &skip);
        let mut probe = w.clone();
        set_coins(&mut probe, a, 200);
        if leisure::evening_kinds(&probe, a).iter().any(|k| kinds.is_empty() || kinds.contains(k)) {
            break a;
        }
        skip.push(a);
    };
    lod::set_lod(&mut w, a, Lod::Statistical);
    assert_eq!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Statistical));
    w.comp_mut::<Needs>(a).expect("needs").fun = 0.1;
    w.comp_mut::<Needs>(a).expect("needs").hunger = 1.0;
    set_coins(&mut w, a, 200);
    let total = ownership::total_coins(&w);
    let visits = |w: &World| -> u32 {
        BuildingKind::LEISURE
            .iter()
            .flat_map(|&k| w.buildings_of_kind(k).iter().copied())
            .filter_map(|b| w.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).map(|v| u32::from(v.visits_today)))
            .sum()
    };
    let before = visits(&w);
    leisure::stat_daily(&mut w);
    let fun = w.comp::<Needs>(a).expect("needs").fun;
    assert!(fun > 0.25, "fun rose: {fun}");
    assert_ne!(coins(&w, a), 200, "a venue was paid (or a table won)");
    assert!(visits(&w) > before, "a visit");
    assert_eq!(ownership::total_coins(&w), total, "conserved");
}

/// L20: the leader's Collect pays the week's share to at least 3 members.
#[test]
fn test_collect_pays_three_members_tribute() {
    let mut w = World::new(42, Config::load().dole_city());
    let gang = w.gangs()[0];
    let mut skip = Vec::new();
    for _ in 0..3 {
        let m = civilian(&w, &skip);
        citysim::systems::gang::enlist(&mut w, m, gang);
        skip.push(m);
    }
    citysim::systems::gang::recompute_leader(&mut w, gang);
    let leader = w.comp::<Gang>(gang).and_then(|g| g.leader);
    {
        let g = w.comp_mut::<Gang>(gang).expect("gang");
        g.tribute_week = 100;
        g.treasury = 1000;
    }
    let members = leisure::members(&w, gang);
    let before: Vec<i64> = members.iter().map(|&m| coins(&w, m)).collect();
    let total = ownership::total_coins(&w);
    let paid = leisure::payout(&mut w, gang, leader);
    assert!(paid >= 3, "paid {paid}");
    let gained = members.iter().zip(&before).filter(|(&m, &b)| coins(&w, m) > b).count();
    assert!(gained >= 3);
    assert_eq!(ownership::total_coins(&w), total);
    assert_eq!(w.comp::<Gang>(gang).map(|g| g.tribute_week), Some(0));
    assert_eq!(w.stats.current.living.flow_tribute, 50, "half the week's 100");
}

/// L20: a gang under Expand with the treasury builds a front in a district
/// it holds.
#[test]
fn test_front_built_under_expand_in_held_district() {
    let mut w = World::new(42, Config::load().dole_city());
    // A Lot a Den may stand on, and a Home of its district held by the gang.
    let lot = citysim::systems::founding::vacant_lots(&w)
        .into_iter()
        .find(|&l| {
            let door = w.comp::<Building>(l).expect("lot").door;
            citysim::systems::founding::tier_ok(BuildingKind::Den, citysim::systems::founding::door_tier(&w, door))
        })
        .expect("a Lot for a Den");
    let d = w.district_of_building(lot);
    let home = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .find(|&h| w.district_of_building(h) == d && w.comp::<Building>(h).is_some_and(|b| !b.derelict))
        .expect("a Home there");
    let gang = w.gangs()[0];
    let m = civilian(&w, &[]);
    citysim::systems::gang::enlist(&mut w, m, gang);
    {
        let g = w.comp_mut::<Gang>(gang).expect("gang");
        g.order = citysim::Order::Expand;
        g.treasury = 5000;
        g.territory.push(home);
    }
    // Review fix: the cap counts the seeded fronts on the city's deed that
    // name the gang too; room for one more above them.
    let seeded = [BuildingKind::FightPit, BuildingKind::Den, BuildingKind::Club]
        .iter()
        .flat_map(|&k| w.buildings_of_kind(k).iter().copied())
        .filter(|&b| w.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).is_some_and(|v| v.front_of == Some(gang)))
        .count() as u32;
    w.config.leisure.fronts_max = seeded + 1;
    let total = ownership::total_coins(&w);
    let before = leisure::fronts_of(&w, gang);
    leisure::fronts_daily(&mut w, gang);
    let after = leisure::fronts_of(&w, gang);
    let new = after.iter().find(|b| !before.contains(b)).copied().expect("a new front");
    assert_eq!(w.district_of_building(new), d, "in the held district");
    let bd = w.comp::<Building>(new).expect("front");
    assert_eq!(bd.owner, Some(gang));
    assert_eq!(bd.venue.as_ref().and_then(|v| v.front_of), Some(gang));
    assert!(w.comp::<Gang>(gang).is_some_and(|g| g.treasury < 5000), "the gang paid");
    assert_eq!(ownership::total_coins(&w), total, "the cost went to the Treasury");
}

/// L20, the negative case (the core_sanity "a gang front" bullet is a
/// printed finding since P5i; docs/TESTING.md): with a Lot and a held
/// district, a gang not under Expand, or under Expand with an empty
/// treasury, builds no front.
#[test]
fn test_no_front_without_expand_or_treasury() {
    for (order, treasury) in [(citysim::Order::LieLow, 5000), (citysim::Order::Expand, 0)] {
        let mut w = World::new(42, Config::load().dole_city());
        let lot = citysim::systems::founding::vacant_lots(&w)
            .into_iter()
            .find(|&l| {
                let door = w.comp::<Building>(l).expect("lot").door;
                citysim::systems::founding::tier_ok(BuildingKind::Den, citysim::systems::founding::door_tier(&w, door))
            })
            .expect("a Lot for a Den");
        let d = w.district_of_building(lot);
        let home = w
            .buildings_of_kind(BuildingKind::Home)
            .iter()
            .copied()
            .find(|&h| w.district_of_building(h) == d && w.comp::<Building>(h).is_some_and(|b| !b.derelict))
            .expect("a Home there");
        let gang = w.gangs()[0];
        let m = civilian(&w, &[]);
        citysim::systems::gang::enlist(&mut w, m, gang);
        {
            let g = w.comp_mut::<Gang>(gang).expect("gang");
            g.order = order;
            g.treasury = treasury;
            g.territory.push(home);
        }
        w.config.leisure.fronts_max = 99;
        let before = leisure::fronts_of(&w, gang);
        leisure::fronts_daily(&mut w, gang);
        assert_eq!(leisure::fronts_of(&w, gang), before, "a front under {order:?} with treasury {treasury}");
    }
}

/// L13: a Full and a Statistical agent lose the same fun in 24 hours (the
/// same agent in a forced-Full and a forced-Statistical city, the gains off).
#[test]
fn test_fun_decays_at_every_tier_equally() {
    let run = |force: Lod| -> Vec<(EntityId, f32, citysim::Class)> {
        let mut cfg = Config::load().dole_city().scaled_to(300);
        cfg.lod.force = Some(force);
        // Jobs and room P3: no trades (their hires shrink the jobless pool compared here).
        cfg.trades.clear();
        // Jobs & lives phase 7: nor the life stages (the seeded kin move a tier's day).
        cfg.ages.enabled = false;
        cfg.reset_trade_cache();
        cfg.leisure.gain = citysim::config::FunGainCfg {
            club: 0.0,
            club_spire: 0.0,
            arcade: 0.0,
            noodle_bar: 0.0,
            fight_pit: 0.0,
            den: 0.0,
            lounge: 0.0,
            drink: 0.0,
            hangout_hour: 0.0,
            watch: 0.0,
            free_stat: 0.0,
        };
        let mut w = World::new(7, cfg);
        // L2 phase 5 seeds a spread of opening fun; this test compares the decay alone from a full need
        // (below 1.0 a tier's small gains, which 1.0 clamped away, show).
        for a in w.citizens() {
            if let Some(n) = w.comp_mut::<Needs>(a) {
                n.fun = 1.0;
            }
        }
        w.run_ticks(TICKS_PER_DAY);
        // The jobless off corp payrolls keep one class all day.
        w.citizens()
            .into_iter()
            .filter(|&a| !w.has::<Job>(a) && citysim::systems::life::exec_corp(&w, a).is_none())
            .filter_map(|a| Some((a, w.comp::<Needs>(a)?.fun, citysim::systems::classes::class_of(&w, a))))
            .collect()
    };
    let full = run(Lod::Full);
    for other in [Lod::Coarse, Lod::Statistical] {
        let tier = run(other);
        let mut compared = 0;
        for (a, f, c) in &full {
            if let Some((_, s, c2)) = tier.iter().find(|(b, _, _)| b == a) {
                if c == c2 {
                    assert!((f - s).abs() < 1e-5, "{a:?}: Full {f} vs {other:?} {s}");
                    compared += 1;
                }
            }
        }
        assert!(compared > 50, "{other:?}: compared {compared}");
    }
    assert!(full.iter().any(|(_, f, _)| *f < 0.9), "fun decayed");
    // A held prisoner (`[lod] budget` on): a day settled at once by
    // `law::settle_held` equals 24 hourly decays.
    let mut w = World::new(7, Config::load().dole_city().scaled_to(300));
    let a = civilian(&w, &[]);
    let execs = citysim::systems::classes::exec_set(&w);
    let rate = leisure::fun_per_hour(&w, a, &execs);
    let mut expect = w.comp::<Needs>(a).expect("needs").clone();
    for _ in 0..24 {
        leisure::decay_fun(&mut expect, rate, 1.0);
    }
    let now = w.tick + TICKS_PER_DAY;
    w.tick = now;
    w.held_since.insert(a, now - TICKS_PER_DAY);
    citysim::systems::law::settle_held(&mut w, a);
    let held = w.comp::<Needs>(a).expect("needs").fun;
    assert!((held - expect.fun).abs() < 1e-5, "held {held} vs 24 hourly {}", expect.fun);
    assert!(held < 1.0, "the hold decays fun");
}

/// L31: the bouts, a table game and the Statistical evening draw only on
/// keyed streams: the world and agent streams are where they were.
#[test]
fn test_l2_streams_untouch_world_and_agent() {
    let mk = || World::new(42, Config::load().dole_city());
    let (mut a, mut b) = (mk(), mk());
    // A bout needs two Fighters on shift and a bet.
    let pit = standing(&a, BuildingKind::FightPit)[0];
    let f1 = civilian(&a, &[]);
    let f2 = civilian(&a, &[f1]);
    let bettor = civilian(&a, &[f1, f2]);
    for f in [f1, f2] {
        demography::hire(&mut a, f, pit, Role::Fighter);
        a.comp_mut::<Job>(f).expect("job").shifts = vec![(0, 1440)];
    }
    if let Some(v) = a.comp_mut::<Building>(pit).and_then(|bd| bd.venue.as_mut()) {
        v.bets.push((bettor, 3, false));
    }
    leisure::bouts(&mut a);
    // A table game.
    let den = standing(&a, BuildingKind::Den)[0];
    set_coins(&mut a, bettor, 50);
    a.enter_building(bettor, den);
    actions::on_start(&mut a, bettor, ActionKind::Gamble, Some(den));
    // The Statistical evening.
    let s = civilian(&a, &[f1, f2, bettor]);
    lod::set_lod(&mut a, s, Lod::Statistical);
    a.comp_mut::<Needs>(s).expect("needs").fun = 0.1;
    set_coins(&mut a, s, 100);
    leisure::stat_daily(&mut a);
    // `hire` and `set_lod` touch no stream either; compare the next draws.
    let wa: u64 = a.rng.world().random();
    let wb: u64 = b.rng.world().random();
    assert_eq!(wa, wb, "the world stream");
    for id in [f1, f2, bettor, s] {
        let xa: u64 = a.rng.agent(id).random();
        let xb: u64 = b.rng.agent(id).random();
        assert_eq!(xa, xb, "{id:?}'s stream");
    }
}

/// The "off hours" gate's output in `id`'s Unwind considerations.
fn off_hours_gate(w: &World, id: EntityId) -> Option<f32> {
    leisure::considerations(w, id).map(|(cs, _)| cs.iter().find(|c| c.name == "off hours").expect("the gate").output)
}

/// Day gaps 1 (2026-10-11): off hours are the agent's own: a night crew's
/// afternoon and a rest day are off (the wall clock's Evening/Night gate
/// left them at Unwind x 0.3 < Idle, at home all day), the hour before a
/// shift is not, and a workday's shift is not.
#[test]
fn test_off_hours_follow_the_agents_own_shift() {
    let mut w = World::new(42, Config::load());
    leisure::spots_daily(&mut w);
    let a = civilian(&w, &[]);
    // The Bar nearest the Home: the walk to work is short (the rule reads it).
    let home = w.comp::<Household>(a).and_then(|h| h.home).expect("housed");
    let home_door = w.comp::<Building>(home).expect("home").door;
    let bar = standing(&w, BuildingKind::Bar)
        .into_iter()
        .min_by_key(|&b| (w.comp::<Building>(b).expect("bar").door.manhattan(home_door), b))
        .expect("a Bar");
    demography::hire(&mut w, a, bar, Role::Bartender);
    {
        let n = w.comp_mut::<Needs>(a).expect("needs");
        n.fun = 0.2;
        n.hunger = 1.0;
        n.energy = 1.0;
    }
    set_coins(&mut w, a, 300);
    // A night crew (21:00-06:00) on day 1, a workday.
    w.comp_mut::<Job>(a).expect("job").shifts = vec![(1260, 1440), (0, 360)];
    w.tick = TICKS_PER_DAY + 13 * 60;
    assert_eq!(off_hours_gate(&w, a), Some(1.0), "a night crew's afternoon is off hours");
    w.tick = TICKS_PER_DAY + 20 * 60;
    assert_eq!(off_hours_gate(&w, a), Some(0.3), "an hour before the shift is not");
    // A day shift: day 6 is the rest day, day 5 a workday.
    w.comp_mut::<Job>(a).expect("job").shifts = vec![(540, 1080)];
    w.tick = 6 * TICKS_PER_DAY + 12 * 60;
    assert_eq!(off_hours_gate(&w, a), Some(1.0), "a rest day's noon is off hours");
    w.tick = 5 * TICKS_PER_DAY + 12 * 60;
    assert!(off_hours_gate(&w, a).is_none_or(|g| g < 1.0), "a workday's shift is not");
    w.tick = 5 * TICKS_PER_DAY + 20 * 60;
    assert_eq!(off_hours_gate(&w, a), Some(1.0), "the evening after it is");
}

/// Day gaps 2 (2026-10-11): a paid venue past `[leisure]
/// venue_walk_cap_ticks` is not on the ladder (fabbers walked 171 min to a
/// Gambling Den after a 9 h shift, 195 back); the free rungs or Idle win.
#[test]
fn test_unwind_paid_venue_within_walk_cap() {
    let mut w = World::new(42, Config::load());
    leisure::spots_daily(&mut w);
    w.tick = 19 * 60;
    let cap = w.config.leisure.venue_walk_cap_ticks;
    let adults: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| demography::is_adult(&w, a) && w.comp::<Household>(a).is_some_and(|h| h.home.is_some()))
        .take(400)
        .collect();
    let (mut paid, mut far) = (0, Vec::new());
    for a in adults {
        {
            let n = w.comp_mut::<Needs>(a).expect("needs");
            n.fun = 0.2;
            n.hunger = 1.0;
        }
        set_coins(&mut w, a, 300);
        let Some(v) = leisure::choice(&w, a, false).and_then(|p| p.venue) else { continue };
        paid += 1;
        let from = citysim::exec::walk_origin(&w, a).expect("an origin");
        let door = w.comp::<Building>(v).expect("a venue").door;
        let walk = citysim::exec::timed_walk(&w, a, from, door);
        if walk > cap {
            far.push((a, v, walk));
        }
    }
    assert!(paid > 0, "someone takes a paid rung");
    assert!(far.is_empty(), "{} of {paid} paid picks past the cap: {:?}", far.len(), &far[..far.len().min(5)]);
}

/// The "phase" gate's output in `id`'s Socialise considerations.
fn socialise_phase(w: &World, id: EntityId) -> Option<f32> {
    citysim::utility::goals::considerations(w, id, citysim::GoalKind::Socialise, false)
        .map(|(cs, _)| cs.iter().find(|c| c.name == "phase").expect("the gate").output)
}

/// Day gaps fix round (2026-10-11): Socialise's evening is the job
/// holder's own (off hours, `leisure::off_hours`, outside the small
/// hours): a night crew's afternoon and an evening-shift Fighter's morning
/// are its evening; a shift being worked in the wall clock's Evening is
/// not. The jobless keep the wall clock.
#[test]
fn test_socialise_evening_follows_the_agents_own_shift() {
    let mut w = World::new(42, Config::load());
    let a = civilian(&w, &[]);
    let b = civilian(&w, &[a]);
    for id in [a, b] {
        let n = w.comp_mut::<Needs>(id).expect("needs");
        n.hunger = 1.0;
        n.energy = 1.0;
        n.belonging = 0.2;
    }
    let home = w.comp::<Household>(a).and_then(|h| h.home).expect("housed");
    let home_door = w.comp::<Building>(home).expect("home").door;
    let bar = standing(&w, BuildingKind::Bar)
        .into_iter()
        .min_by_key(|&x| (w.comp::<Building>(x).expect("bar").door.manhattan(home_door), x))
        .expect("a Bar");
    demography::hire(&mut w, a, bar, Role::Bartender);
    w.comp_mut::<Job>(a).expect("job").shifts = vec![(1260, 1440), (0, 360)];
    // A night crew's afternoon on day 1 (a workday) is its evening.
    w.tick = TICKS_PER_DAY + 13 * 60;
    assert_eq!(socialise_phase(&w, a), Some(1.0), "a night crew's afternoon");
    assert_eq!(socialise_phase(&w, b), Some(0.4), "the jobless keep the wall clock's Evening");
    // An evening shift (18:00-24:00): the morning is free, the shift is not.
    w.comp_mut::<Job>(a).expect("job").shifts = vec![(1080, 1440)];
    w.tick = TICKS_PER_DAY + 10 * 60;
    assert_eq!(socialise_phase(&w, a), Some(1.0), "an evening shift's morning");
    w.tick = TICKS_PER_DAY + 20 * 60;
    assert_eq!(socialise_phase(&w, a), Some(0.4), "an evening shift being worked");
    assert_eq!(socialise_phase(&w, b), Some(1.0), "the jobless evening");
    // A day worker's small hours stay the Night.
    w.comp_mut::<Job>(a).expect("job").shifts = vec![(540, 1080)];
    w.tick = 2 * TICKS_PER_DAY + 2 * 60;
    assert_eq!(socialise_phase(&w, a), Some(0.4), "a day worker's 02:00");
}

/// Phase-8 review finding 2: the learned off-screen evening reads the Full
/// ladder's gates: with every venue shut (`closed_until`), nobody's evening
/// has a paid rung (on d2eea01 the closed venues still took visits).
#[test]
fn test_learned_evening_skips_closed_venues() {
    // The pressure pass R2: the v1 pace (`common::v1_pace`).
    let mut cfg = Config::load().dole_city();
    common::v1_pace(&mut cfg);
    let mut w = World::new(42, cfg);
    w.run_ticks(1259);
    let mut skip = Vec::new();
    let a = loop {
        let a = civilian(&w, &skip);
        let mut probe = w.clone();
        set_coins(&mut probe, a, 200);
        if !leisure::evening_kinds(&probe, a).is_empty() {
            break a;
        }
        skip.push(a);
    };
    set_coins(&mut w, a, 200);
    let until = w.tick + TICKS_PER_DAY;
    for k in BuildingKind::LEISURE.iter().copied().chain([BuildingKind::Bar]) {
        for b in w.buildings_of_kind(k).to_vec() {
            if let Some(bd) = w.comp_mut::<Building>(b) {
                bd.closed_until = Some(until);
            }
        }
    }
    let left = leisure::evening_kinds(&w, a);
    assert!(left.is_empty(), "a shut venue on the evening: {left:?}");
}

// ---------------------------------------------------------------------------
// Shadow V4 bugs 2, 17 and 18 (docs/SHADOW_V4.md): a day worker's night, the
// doze's cap, the pit's card going round, no leisure at one's own venue.
// ---------------------------------------------------------------------------

/// Bug 2 (Unwind): at 03:00 a day worker's leisure is not "off hours" (it
/// is at 20:00): woken at 23:30 it stood on a corner in the small hours.
#[test]
fn test_bug2_day_worker_night_is_not_off_hours() {
    let mut w = common::world();
    let day = 3 * TICKS_PER_DAY;
    let off = |w: &World, a: EntityId| {
        leisure::considerations(w, a)
            .and_then(|(cs, _)| cs.into_iter().find(|c| c.name == "off hours"))
            .map(|c| c.input)
    };
    w.tick = day + 20 * 60;
    let mut checked = 0;
    for a in day_workers(&w) {
        if let Some(n) = w.comp_mut::<Needs>(a) {
            n.fun = 0.1;
            n.hunger = 1.0;
            n.energy = 1.0;
        }
        w.tick = day + 20 * 60;
        if off(&w, a) != Some(1.0) {
            continue;
        }
        w.tick = day + 3 * 60;
        assert!(citysim::exec::routine::day_worker_night(&w, a));
        assert_eq!(off(&w, a), None, "03:00 is no day worker's leisure hour");
        checked += 1;
        if checked == 5 {
            break;
        }
    }
    assert!(checked > 0, "a day worker with a pick at 20:00");
}

/// Bug 2 (Rest): a doze restores energy only up to `rest_energy_cap`
/// (+0.1 a Rest outran the decay, so a Rest loop never slept).
#[test]
fn test_bug2_rest_restores_only_to_the_cap() {
    let mut w = common::world();
    let a = *day_workers(&w).first().expect("an agent");
    let cap = w.config.life.rest_energy_cap;
    let rest = |w: &mut World, e: f32| {
        w.comp_mut::<Needs>(a).expect("needs").energy = e;
        let now = w.tick;
        actions::on_complete(w, a, ActionKind::Rest, None, now, now);
        w.comp::<Needs>(a).expect("needs").energy
    };
    assert!((rest(&mut w, 0.85) - 0.85).abs() < 1e-6, "no gain above the cap");
    assert!((rest(&mut w, 0.3) - 0.4).abs() < 1e-6, "the tired still doze");
    assert!((rest(&mut w, cap - 0.05) - cap).abs() < 1e-6, "up to the cap");
}

/// Bug 17: a pit's bouts go round its fighters (longest since the last
/// bout first), not the same two fittest every night.
#[test]
fn test_bug17_bouts_go_round_the_fighters() {
    let mut w = common::world();
    let pit = w.buildings_of_kind(BuildingKind::FightPit)[0];
    // The pit's own Fighters off the card; three of ours on it, all day.
    for f in w.staff_of(pit).to_vec() {
        if let Some(j) = w.comp_mut::<Job>(f) {
            j.shifts = vec![];
        }
    }
    let crew: Vec<EntityId> = jobless_housed(&w).into_iter().take(3).collect();
    for (i, &f) in crew.iter().enumerate() {
        demography::hire(&mut w, f, pit, Role::Fighter);
        w.comp_mut::<Job>(f).expect("job").shifts = vec![(0, 1440)];
        w.comp_mut::<citysim::Skills>(f).expect("s").fighting = [0.9, 0.8, 0.1][i];
        lod::set_lod(&mut w, f, Lod::Statistical);
    }
    let mut fought = std::collections::BTreeSet::new();
    for d in 1..=7 {
        w.tick = d * TICKS_PER_DAY + 22 * 60;
        let before = w.events.len();
        leisure::bouts(&mut w);
        for e in w.events.iter().skip(before).filter(|e| e.kind == citysim::EventKind::Bout) {
            fought.extend(e.actors.iter().take(2).copied());
        }
    }
    assert!(fought.contains(&crew[2]), "the weakest fighter gets a bout in a week");
}

/// Bug 18: a venue's staff do not take their leisure at their own venue
/// (then wait there up to 10 h for the shift).
#[test]
fn test_bug18_no_leisure_at_ones_own_venue() {
    let mut w = common::world();
    let mut checked = 0;
    for kind in [BuildingKind::Brothel, BuildingKind::FightPit, BuildingKind::Lounge] {
        for &b in w.buildings_of_kind(kind).to_vec().iter() {
            let Some(&s) = w.staff_of(b).first() else { continue };
            w.leave_building(s);
            w.enter_building(s, b);
            set_coins(&mut w, s, 2000);
            if let Some(n) = w.comp_mut::<Needs>(s) {
                n.fun = 0.05;
                n.hunger = 1.0;
            }
            let pick = leisure::choice(&w, s, false);
            assert!(pick.is_none_or(|p| p.venue != Some(b)), "{kind:?} staff unwinds at its own venue");
            checked += 1;
        }
    }
    assert!(checked > 0, "a venue with staff");
}
