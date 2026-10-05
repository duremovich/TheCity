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
    let broke = OrderInputs { prize: 10, ..i };
    assert_ne!(best(&broke), Order::Raid, "no prize, no raid");
}

#[test]
fn test_order_retaliate_on_a_grudge_regardless_of_ratio() {
    let i = OrderInputs { grudge: true, own: 2, rival: 8, ..inputs() };
    assert_eq!(best(&i), Order::Retaliate);
    let sacked = OrderInputs { sacked: true, ..i };
    assert_ne!(best(&sacked), Order::Retaliate, "a sacked gang cannot muster");
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
