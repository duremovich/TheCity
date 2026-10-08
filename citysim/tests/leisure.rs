//! Life pass L2 phase 2 (docs/LIFE_L2.md § 2, § 10; plan phase 2): the fun
//! need at every tier, the rung ladder, the venues' money and doors,
//! HangOut's spot scoring, the Statistical evening, fronts and the leader's
//! Collect, and the keyed streams.

use citysim::exec::actions;
use citysim::goap::ActionKind;
use citysim::systems::{demography, leisure, lod, ownership};
use citysim::{
    Brain, Building, BuildingKind, Config, EntityId, Gang, Household, Job, Lod, Needs, Personality, Position, Role,
    Wallet, World, TICKS_PER_DAY,
};
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

fn set_coins(w: &mut World, id: EntityId, c: i64) {
    w.comp_mut::<Wallet>(id).expect("wallet").coins = c;
}

fn coins(w: &World, id: EntityId) -> i64 {
    w.comp::<Wallet>(id).map_or(0, |x| x.coins)
}

/// § 1: an Enjoy charges the venue's price at start and an abandoned one is
/// refunded; coins are conserved throughout.
#[test]
fn test_enjoy_charges_price_and_refunds_on_abort() {
    let mut w = World::new(42, Config::load());
    assert!(leisure::on(&w));
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
    assert_eq!(coins(&w, a), 100, "refunded on abort");
    assert_eq!(ownership::total_coins(&w), total, "conserved after the refund");
    // Broke: no start, nothing moves.
    set_coins(&mut w, a, 0);
    assert!(!actions::can_start(&w, a, ActionKind::Enjoy, Some(club)));
}

/// L18: table games conserve coins; a house that cannot pay a win shuts
/// its table for the day.
#[test]
fn test_gamble_conserves_coins_and_short_house_shuts_table() {
    let mut w = World::new(42, Config::load());
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
    let mut w = World::new(42, Config::load());
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
    let mut w = World::new(42, Config::load());
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
    let mut w = World::new(42, Config::load());
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
                ((0..256).contains(&x) && (0..192).contains(&y)).then_some(citysim::TilePos { x: x as u8, y: y as u8 })
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
    let mut w = World::new(42, Config::load());
    leisure::spots_daily(&mut w);
    let a = civilian(&w, &[]);
    w.comp_mut::<Needs>(a).expect("needs").fun = 0.2;
    w.comp_mut::<Needs>(a).expect("needs").hunger = 1.0;
    set_coins(&mut w, a, 0);
    let pick = leisure::choice(&w, a, true).expect("a free rung");
    assert_eq!(pick.rung, citysim::living::Rung::Free, "{pick:?}");
    assert!(pick.venue.is_none());
    set_coins(&mut w, a, 300);
    let pick = leisure::choice(&w, a, true).expect("a rung");
    assert_ne!(pick.rung, citysim::living::Rung::Free, "{pick:?}");
    assert!(pick.venue.is_some());
}

/// L17: the Statistical evening pays a venue, raises fun, adds a visit.
#[test]
fn test_stat_daily_pays_venue_and_raises_fun() {
    let mut w = World::new(42, Config::load());
    let a = civilian(&w, &[]);
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
    let mut w = World::new(42, Config::load());
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
    let mut w = World::new(42, Config::load());
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

/// L13: a Full and a Statistical agent lose the same fun in 24 hours (the
/// same agent in a forced-Full and a forced-Statistical city, the gains off).
#[test]
fn test_fun_decays_at_every_tier_equally() {
    let run = |force: Lod| -> Vec<(EntityId, f32, citysim::Class)> {
        let mut cfg = Config::load().scaled_to(300);
        cfg.lod.force = Some(force);
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
    let mut w = World::new(7, Config::load().scaled_to(300));
    assert!(lod::budget_on(&w));
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
    let mk = || World::new(42, Config::load());
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
