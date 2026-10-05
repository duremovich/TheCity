//! M8: factions. The brain's orders, claims and flips, the raid resolver.

use citysim::systems::faction::{self, OrderInputs};
use citysim::systems::gang;
use citysim::{
    Brain, Building, BuildingKind, Claim, Config, EntityId, EventKind, Gang, Order, Personality, Shock, World,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
}

/// Two jobless adults who are not each other's spouse.
fn two_civilians(w: &World) -> (EntityId, EntityId) {
    let mut it = w.citizens().into_iter().filter(|&id| !w.has::<citysim::Job>(id) && w.has::<Brain>(id));
    let a = it.next().expect("a");
    let b = it.find(|&b| w.spouse_of(a) != Some(b) && w.edge(a, b).is_none()).expect("b");
    (a, b)
}

fn gangs(w: &World) -> (EntityId, EntityId) {
    let g = w.gangs();
    (g[0], g[1])
}

/// A leader with middling traits, so the tests pin the curves, not the dice.
fn temper(w: &mut World, id: EntityId) {
    let p = w.comp_mut::<Personality>(id).expect("p");
    p.greed = 0.5;
    p.courage = 0.5;
    p.pride = 0.5;
    p.loyalty = 0.9;
}

fn inputs() -> OrderInputs {
    OrderInputs {
        frontier: 20,
        frontier_total: 30,
        rival_territory: 0,
        own: 5,
        rival: 5,
        heat: 0.0,
        prize: 0,
        grudge: false,
        greed: 0.5,
        courage: 0.5,
        pride: 0.5,
        raid_ready: true,
        rival_exists: true,
        sacked: false,
    }
}

fn best(i: &OrderInputs) -> Order {
    faction::score_orders(i, &Config::load().gangs)[0].order
}

#[test]
fn test_order_expand_while_the_frontier_is_open() {
    assert_eq!(best(&inputs()), Order::Expand);
}

#[test]
fn test_order_contest_at_parity_once_the_frontier_is_claimed() {
    let i = OrderInputs { frontier: 0, rival_territory: 5, ..inputs() };
    assert_eq!(best(&i), Order::Contest);
}

#[test]
fn test_order_raid_with_a_strength_edge_and_a_prize() {
    let i = OrderInputs { frontier: 0, rival_territory: 5, own: 10, rival: 5, prize: 200, courage: 0.8, ..inputs() };
    assert_eq!(best(&i), Order::Raid);
    let broke = OrderInputs { prize: 5, ..i };
    assert_ne!(best(&broke), Order::Raid, "no prize, no raid");
}

#[test]
fn test_order_retaliate_on_a_grudge_regardless_of_ratio() {
    let i = OrderInputs { grudge: true, own: 2, rival: 8, ..inputs() };
    assert_eq!(best(&i), Order::Retaliate);
    let sacked = OrderInputs { sacked: true, raid_ready: false, ..i };
    assert_ne!(best(&sacked), Order::Retaliate, "a sacked gang cannot muster");
    let cooling = OrderInputs { raid_ready: false, ..i };
    assert_ne!(best(&cooling), Order::Retaliate, "one raid per cooldown, grudge or not");
}

#[test]
fn test_order_lie_low_under_heat() {
    let i = OrderInputs { heat: 0.6, ..inputs() };
    assert_eq!(best(&i), Order::LieLow);
}

#[test]
fn test_choose_respects_hysteresis() {
    let cfg = Config::load().gangs;
    let scores = faction::score_orders(&inputs(), &cfg);
    // Expand leads comfortably here: a current Expand holds, a current LieLow does not.
    assert_eq!(faction::choose(&scores, Order::Expand, 0.1), None);
    assert_eq!(faction::choose(&scores, Order::LieLow, 0.1), Some(Order::Expand));
    // Hand-built: a lead inside the band holds at the daily rescoring and switches on a shock.
    let close = vec![
        citysim::OrderScore { order: Order::Contest, score: 0.50, considerations: vec![] },
        citysim::OrderScore { order: Order::Expand, score: 0.45, considerations: vec![] },
    ];
    assert_eq!(faction::choose(&close, Order::Expand, 0.1), None);
    assert_eq!(faction::choose(&close, Order::Expand, 0.0), Some(Order::Contest));
}

#[test]
fn test_next_muster_is_at_least_two_hours_away() {
    assert_eq!(faction::next_muster(0, 22), 1320);
    assert_eq!(faction::next_muster(1200, 22), 1320);
    assert_eq!(faction::next_muster(1300, 22), 1320 + 1440);
}

#[test]
fn test_two_gangs_seeded_with_their_own_hideouts() {
    let w = world(42);
    let (g0, g1) = gangs(&w);
    let h0 = w.hideout_of(g0).expect("h0");
    let h1 = w.hideout_of(g1).expect("h1");
    assert_ne!(h0, h1);
    for h in [h0, h1] {
        assert_eq!(w.comp::<Building>(h).expect("b").kind, BuildingKind::Hideout);
    }
    assert_eq!(w.comp::<Gang>(g0).expect("g").name, "The Hollow");
    assert_eq!(w.comp::<Gang>(g1).expect("g").name, "Ninefold");
    assert_eq!(w.rival_of(g0), Some(g1));
    assert_eq!(w.rival_of(g1), Some(g0));
}

#[test]
fn test_shocks_force_a_midday_rescore() {
    let mut w = world(43);
    let (g0, _) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    assert_eq!(w.comp::<Gang>(g0).expect("g").leader, Some(a));
    w.tick = 700; // noon: not the daily rescoring
    gang::push_shock(&mut w, g0, Shock::MemberKilled { by_rival: true });
    gang::run(&mut w);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.order, Order::Retaliate);
    assert!(g.shocks.is_empty(), "shocks are consumed");
    assert_eq!(g.raid_at, Some(1320), "musters at 22:00 tonight");
    assert!(g.retaliate_until.is_some());
    assert_eq!(g.order_trace.first().map(|s| s.order), Some(Order::Retaliate));
    assert!(w.events.iter().any(|e| e.kind == EventKind::OrderChanged && e.text.contains("shock")));
}

#[test]
fn test_no_leader_no_new_orders() {
    let mut w = world(44);
    let (g0, _) = gangs(&w);
    w.tick = 700;
    gang::push_shock(&mut w, g0, Shock::Sacked);
    gang::run(&mut w);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.order, Order::Expand);
    assert!(g.order_trace.is_empty());
    assert!(!w.events.iter().any(|e| e.kind == EventKind::OrderChanged));
}

#[test]
fn test_claim_resets_on_a_rival_blow_and_flips_at_three() {
    let mut w = world(45);
    let (g0, g1) = gangs(&w);
    let (a, b) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    gang::enlist(&mut w, b, g1);
    let homes = [a, b].map(|x| w.comp::<citysim::Household>(x).and_then(|h| h.home));
    let home = w.buildings_by_kind[&BuildingKind::Home]
        .iter()
        .copied()
        .find(|&h| !homes.contains(&Some(h)) && w.comp::<Building>(h).is_some_and(|bd| bd.occupants.len() >= 2))
        .expect("an occupied home");
    for _ in 0..3 {
        gang::extort(&mut w, a, home);
    }
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g0, count: 3 }));
    assert!(w.comp::<Gang>(g0).expect("g").territory.contains(&home));
    assert!(!w.events.iter().any(|e| e.kind == EventKind::TerritoryFlipped), "a first claim is not a flip");

    gang::extort(&mut w, b, home);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g1, count: 1 }), "a rival blow resets");
    assert!(w.comp::<Gang>(g0).expect("g").territory.contains(&home), "still held until the third blow");
    assert_eq!(w.edge(a, b).map(|e| e.kind), Some(citysim::RelKind::Enemy), "the holders hate the intruder");

    gang::extort(&mut w, b, home);
    gang::extort(&mut w, b, home);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g1, count: 3 }));
    assert!(w.comp::<Gang>(g1).expect("g").territory.contains(&home));
    assert!(!w.comp::<Gang>(g0).expect("g").territory.contains(&home));
    assert!(w.events.iter().any(|e| e.kind == EventKind::TerritoryFlipped && e.text.contains("from The Hollow")));
    assert!(w.comp::<Gang>(g0).expect("g").shocks.contains(&Shock::HomeFlippedAgainst));
}

#[test]
fn test_gang_work_target_follows_the_order() {
    let mut w = world(46);
    let (g0, _) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let (_, following) = gang::gang_work_target(&w, a).expect("an unclaimed Home to expand into");
    assert_eq!(following, Some(Order::Expand));
    w.comp_mut::<Gang>(g0).expect("g").order = Order::Contest;
    assert!(gang::gang_work_target(&w, a).is_none(), "nothing of the rival's to contest yet");
    w.comp_mut::<Gang>(g0).expect("g").order = Order::LieLow;
    assert!(gang::gang_work_target(&w, a).is_none(), "lying low");
    w.comp_mut::<Personality>(a).expect("p").loyalty = 0.1;
    let (_, following) = gang::gang_work_target(&w, a).expect("a freelancer works anyway");
    assert_eq!(following, None);
}

#[test]
fn test_sacked_gang_does_not_recruit() {
    let mut w = world(47);
    let (g0, _) = gangs(&w);
    let (recruit, member) = two_civilians(&w);
    gang::enlist(&mut w, member, g0);
    let contact = w.config.social.join_gang_affinity + 0.05;
    w.edge_entry(recruit, member).affinity = contact;
    assert_eq!(gang::recruit_gang(&w, recruit), Some(g0));
    w.comp_mut::<Gang>(g0).expect("g").sacked_until = Some(w.tick + 1000);
    assert_eq!(gang::recruit_gang(&w, recruit), None);
}

#[test]
fn test_legacy_save_migrates_hideout_and_claims() {
    let mut w = world(48);
    let (g0, _) = gangs(&w);
    let hideout = w.hideout_of(g0).expect("h");
    let home = w.buildings_by_kind[&BuildingKind::Home][0];
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.hideout = EntityId::NONE;
        g.territory = vec![hideout, home];
        g.territory.sort();
    }
    w.migrate_legacy();
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.hideout, hideout);
    assert_eq!(g.territory, vec![home]);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g0, count: 3 }));
}

// ---------------------------------------------------------------------------
// Raids
// ---------------------------------------------------------------------------

use citysim::systems::raid::{self, Outcome};
use citysim::{GoalKind, Skills};

/// Put a member on a street tile with the Raid goal, as a raider who has marched.
fn stage_raider(w: &mut World, id: EntityId, tile: citysim::TilePos) {
    w.abort_plan(id);
    w.leave_building(id);
    let p = w.comp_mut::<citysim::Position>(id).expect("pos");
    p.tile = tile;
    p.building = None;
    w.comp_mut::<Brain>(id).expect("b").current_goal = Some(GoalKind::Raid);
}

fn set_fighter(w: &mut World, id: EntityId, fighting: f32, courage: f32) {
    w.comp_mut::<Skills>(id).expect("s").fighting = fighting;
    w.comp_mut::<Personality>(id).expect("p").courage = courage;
}

fn civilians(w: &World, n: usize) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| !w.has::<citysim::Job>(id) && w.has::<Brain>(id)).take(n).collect()
}

#[test]
fn test_brawl_with_no_defenders_sacks_the_hideout() {
    let mut w = world(49);
    w.config.crime.fight_death_p = 0.0;
    let (g0, g1) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let h1 = w.hideout_of(g1).expect("h1");
    w.comp_mut::<Gang>(g1).expect("g").treasury = 100;
    w.comp_mut::<Building>(h1).expect("b").stock_food = 20;
    let door_tile = raid::rival_hideout_tile(&w, a).expect("tile");
    stage_raider(&mut w, a, door_tile);
    w.comp_mut::<Gang>(g0).expect("g").raid_at = Some(w.tick);
    assert_eq!(raid::brawl(&mut w, a), Some(Outcome::Sacked));
    assert_eq!(w.comp::<Gang>(g0).expect("g").treasury, 150);
    assert_eq!(w.comp::<Gang>(g1).expect("g").treasury, 0);
    assert!(w.comp::<Gang>(g1).expect("g").is_sacked(w.tick));
    assert!(w.comp::<Gang>(g1).expect("g").shocks.contains(&Shock::Sacked));
    assert_eq!(w.comp::<Building>(h1).expect("b").stock_food, 0);
    assert_eq!(w.comp::<Building>(w.hideout_of(g0).expect("h0")).expect("b").stock_food, 20);
    assert_eq!(w.comp::<Gang>(g0).expect("g").raid_at, None);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Sacked));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Raid && e.text.contains("Sacked")));
    assert_eq!(raid::brawl(&mut w, a), None, "a late raider finds it resolved");
}

#[test]
fn test_brawl_three_on_one_takes_the_treasury() {
    let mut w = world(50);
    w.config.crime.fight_death_p = 0.0;
    let (g0, g1) = gangs(&w);
    let civ = civilians(&w, 4);
    let (r1, r2, r3, d) = (civ[0], civ[1], civ[2], civ[3]);
    for r in [r1, r2, r3] {
        gang::enlist(&mut w, r, g0);
        set_fighter(&mut w, r, 0.9, 0.9);
    }
    temper(&mut w, r1);
    gang::enlist(&mut w, d, g1);
    set_fighter(&mut w, d, 0.1, 0.1);
    let h1 = w.hideout_of(g1).expect("h1");
    w.leave_building(d);
    w.enter_building(d, h1);
    w.comp_mut::<Gang>(g1).expect("g").treasury = 100;
    let door_tile = raid::rival_hideout_tile(&w, r1).expect("tile");
    for r in [r1, r2, r3] {
        stage_raider(&mut w, r, door_tile);
    }
    w.comp_mut::<Gang>(g0).expect("g").raid_at = Some(w.tick);
    let out = raid::brawl(&mut w, r1).expect("resolved");
    assert!(matches!(out, Outcome::Won | Outcome::Sacked), "{out:?}");
    assert!(w.comp::<Gang>(g0).expect("g").treasury >= 100, "half the prize at least");
    assert!(w.comp::<Gang>(g1).expect("g").treasury <= 50);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Assault), "every pairing is an Assault");
}

#[test]
fn test_brawl_one_on_three_loses_and_the_leader_retaliates() {
    let mut w = world(51);
    w.config.crime.fight_death_p = 0.0;
    let (g0, g1) = gangs(&w);
    let civ = civilians(&w, 4);
    let (r, d1, d2, d3) = (civ[0], civ[1], civ[2], civ[3]);
    gang::enlist(&mut w, r, g0);
    temper(&mut w, r);
    set_fighter(&mut w, r, 0.1, 0.5);
    let h1 = w.hideout_of(g1).expect("h1");
    for d in [d1, d2, d3] {
        gang::enlist(&mut w, d, g1);
        set_fighter(&mut w, d, 0.9, 0.9);
        w.leave_building(d);
        w.enter_building(d, h1);
    }
    w.comp_mut::<Gang>(g1).expect("g").treasury = 100;
    let door_tile = raid::rival_hideout_tile(&w, r).expect("tile");
    stage_raider(&mut w, r, door_tile);
    w.tick = 700;
    w.comp_mut::<Gang>(g0).expect("g").raid_at = Some(w.tick);
    assert_eq!(raid::brawl(&mut w, r), Some(Outcome::Lost));
    assert_eq!(w.comp::<Gang>(g1).expect("g").treasury, 100);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.order, Order::Retaliate, "a lost raid is a grudge");
    assert_eq!(g.raid_at, Some(1320), "the counter-raid musters tonight");
}

#[test]
fn test_raid_goal_opens_in_the_gather_window() {
    let mut w = world(52);
    let (g0, _) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    w.tick = 600;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::Raid;
        g.raid_at = Some(1320);
    }
    assert!(!raid::raid_pending(&w, a), "22:00 is twelve hours off");
    assert!(!raid::raid_done(&w, a));
    w.tick = 1200;
    assert!(raid::raid_pending(&w, a), "two hours off: muster");
    assert!(!raid::mustered(&w, a));
    w.tick = 1320;
    assert!(raid::mustered(&w, a));
    assert!(raid::depart(&mut w, a));
    assert_eq!(w.comp::<Gang>(g0).expect("g").last_raid_tick, Some(1320));
    w.comp_mut::<Gang>(g0).expect("g").raid_at = None;
    assert!(raid::raid_done(&w, a));
    assert!(!raid::depart(&mut w, a), "no muster to depart from");
}

#[test]
fn test_gang_members_are_never_statistical() {
    let mut w = world(53);
    let (g0, _) = gangs(&w);
    let members = civilians(&w, 3);
    for &m in &members {
        gang::enlist(&mut w, m, g0);
    }
    w.tick = 60;
    citysim::systems::lod::run(&mut w);
    let stat = w
        .citizens()
        .into_iter()
        .filter(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == citysim::Lod::Statistical))
        .count();
    assert!(stat > 100, "most of the city is Statistical ({stat})");
    for m in members {
        assert_ne!(w.comp::<Brain>(m).expect("b").lod, citysim::Lod::Statistical);
    }
}

#[test]
fn test_contest_keeps_working_a_home_after_the_first_blow() {
    let mut w = world(54);
    let (g0, g1) = gangs(&w);
    let (a, b) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    gang::enlist(&mut w, b, g1);
    temper(&mut w, a);
    let homes = [a, b].map(|x| w.comp::<citysim::Household>(x).and_then(|h| h.home));
    // The only rival-held Home: g1 holds it outright.
    let home = w.buildings_by_kind[&BuildingKind::Home]
        .iter()
        .copied()
        .find(|&h| !homes.contains(&Some(h)) && w.comp::<Building>(h).is_some_and(|bd| bd.occupants.len() >= 2))
        .expect("an occupied home");
    for _ in 0..3 {
        gang::extort(&mut w, b, home);
    }
    assert_eq!(gang::holder_of(&w, home), Some(g1));
    w.comp_mut::<Gang>(g0).expect("g").order = Order::Contest;
    // Guards may stand near it; the target picker is what is under test, so clear them.
    for g in w.citizens().into_iter().filter(|&g| citysim::systems::law::is_guard(&w, g)).collect::<Vec<_>>() {
        w.comp_mut::<citysim::Position>(g).expect("p").tile = citysim::TilePos { x: 95, y: 0 };
    }
    assert_eq!(gang::gang_work_target(&w, a), Some((home, Some(Order::Contest))));
    gang::extort(&mut w, a, home);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g0, count: 1 }));
    assert_eq!(gang::gang_work_target(&w, a), Some((home, Some(Order::Contest))), "still the rival's until it flips");
    gang::extort(&mut w, a, home);
    gang::extort(&mut w, a, home);
    assert_eq!(gang::holder_of(&w, home), Some(g0));
    assert!(gang::gang_work_target(&w, a).is_none(), "nothing of the rival's left to contest");
}
