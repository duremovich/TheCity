//! M5: social graph, gangs, gossip.

use citysim::systems::{gang, law, social};
use citysim::{
    Brain, BuildingKind, Config, Crime, Gang, GangMember, Memory, MemoryKind, Needs, Personality, RelKind, Wallet,
    World, TICKS_PER_DAY,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

/// Two jobless adults who are not each other's spouse.
fn two_civilians(w: &World) -> (citysim::EntityId, citysim::EntityId) {
    let mut it = w.citizens().into_iter().filter(|&id| !w.has::<citysim::Job>(id) && w.has::<Brain>(id));
    let a = it.next().expect("a");
    let b = it.find(|&b| w.spouse_of(a) != Some(b) && w.edge(a, b).is_none()).expect("b");
    (a, b)
}

#[test]
fn test_jail_cellmates_gain_affinity_and_met_in_jail() {
    let mut w = world(31);
    let (a, b) = two_civilians(&w);
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let until = w.tick + 2 * TICKS_PER_DAY;
    law::sentence(&mut w, a, Crime::Theft, until, jail);
    law::sentence(&mut w, b, Crime::Theft, until, jail);
    w.run_ticks(2 * TICKS_PER_DAY - 10);
    let e = w.edge(a, b).expect("cellmates have an edge");
    assert!(e.affinity >= 0.4, "affinity {}", e.affinity);
    for who in [a, b] {
        let other = if who == a { b } else { a };
        let met = w
            .comp::<Memory>(who)
            .expect("mem")
            .entries
            .iter()
            .any(|m| m.kind == MemoryKind::MetInJail && m.subject == Some(other));
        assert!(met, "no MetInJail memory");
    }
}

#[test]
fn test_colocation_at_bar_creates_edge() {
    let mut w = world(32);
    let (a, b) = two_civilians(&w);
    let bar = w.building_of_kind(BuildingKind::Bar).expect("bar");
    for id in [a, b] {
        w.abort_plan(id);
        w.leave_building(id);
        w.enter_building(id, bar);
    }
    // Drive the social system alone so nobody wanders off mid-test.
    let together = |w: &mut World, ticks: u64| {
        for _ in 0..ticks {
            social::run(w);
            w.tick += 1;
        }
    };
    together(&mut w, 29);
    assert!(w.edge(a, b).is_none(), "no edge before 30 ticks");
    together(&mut w, 2);
    let e = w.edge(a, b).expect("edge after 30 ticks together");
    assert_eq!(e.kind, RelKind::Acquaintance);
    assert!(e.affinity.abs() <= 0.15);
    assert!((e.trust - 0.3).abs() < 1e-6);
    assert!(w.neighbours(a).any(|o| o == b));
}

#[test]
fn test_propose_requires_thresholds() {
    let mut w = world(33);
    let (a, b) = two_civilians(&w);
    {
        let e = w.edge_entry(a, b);
        e.affinity = 0.6;
        e.trust = 0.5;
    }
    assert!(social::propose_allowed(&w, a, b));
    {
        let e = w.edge_entry(a, b);
        e.affinity = 0.5;
    }
    assert!(!social::propose_allowed(&w, a, b));
    assert!(!social::propose(&mut w, a, b));
    assert!(w.spouse_of(a).is_none());
    let rejected = w.comp::<Memory>(a).expect("mem").entries.iter().any(|m| m.kind == MemoryKind::Rejected);
    assert!(rejected, "a failed proposal leaves a Rejected memory");
}

#[test]
fn test_join_gang_requires_contact_or_desperation() {
    let mut w = world(34);
    let (recruit, member) = two_civilians(&w);
    let gang_id = w.gangs()[0];
    // one member so the empty-gang bootstrap is off
    w.insert(member, GangMember { gang: gang_id, rank: 0, joined_tick: 0 });
    w.comp_mut::<Gang>(gang_id).expect("gang").members = vec![member];
    w.comp_mut::<Personality>(recruit).expect("p").lawfulness = 0.2;
    w.comp_mut::<Needs>(recruit).expect("n").hunger = 0.9;
    assert!(!gang::eligible(&w, recruit), "fed and unconnected: not eligible");
    // The spec's example is an edge at 0.25 against a 0.2 threshold; the
    // threshold is 0.4 here (see config.toml), so the test tracks the config.
    let contact = w.config.social.join_gang_affinity + 0.05;
    {
        let e = w.edge_entry(recruit, member);
        e.affinity = contact;
    }
    assert!(gang::eligible(&w, recruit), "an edge at {contact} to a member is contact enough");
    assert!(gang::join(&mut w, recruit));
    assert!(w.has::<GangMember>(recruit));
    assert!(w.comp::<Gang>(gang_id).expect("gang").members.contains(&recruit));
}

#[test]
fn test_extort_moves_coins_and_adds_memory() {
    let mut w = world(35);
    let actor = two_civilians(&w).0;
    let g0 = w.gangs()[0];
    gang::enlist(&mut w, actor, g0);
    let actor_home = w.comp::<citysim::Household>(actor).and_then(|h| h.home);
    let home = w.buildings_by_kind[&BuildingKind::Home]
        .iter()
        .copied()
        .find(|&h| Some(h) != actor_home && w.comp::<citysim::Building>(h).is_some_and(|b| b.occupants.len() >= 2))
        .expect("an occupied home");
    let victims: Vec<_> = w.comp::<citysim::Building>(home).expect("b").occupants.clone();
    let before: i64 = victims.iter().map(|&v| w.comp::<Wallet>(v).map_or(0, |x| x.coins)).sum();
    let actor_before = w.comp::<Wallet>(actor).expect("w").coins;
    let taken = gang::extort(&mut w, actor, home);
    assert_eq!(taken, 5);
    let after: i64 = victims.iter().map(|&v| w.comp::<Wallet>(v).map_or(0, |x| x.coins)).sum();
    assert_eq!(before - after, 5);
    assert_eq!(w.comp::<Wallet>(actor).expect("w").coins, actor_before + 5);
    for v in victims {
        let robbed = w
            .comp::<Memory>(v)
            .expect("mem")
            .entries
            .iter()
            .any(|m| m.kind == MemoryKind::WasRobbed && m.subject == Some(actor));
        assert!(robbed, "victim lacks WasRobbed");
        assert_eq!(w.edge(v, actor).expect("edge").kind, RelKind::Enemy);
    }
    assert_eq!(w.comp::<citysim::Building>(home).expect("b").claim, Some(citysim::Claim { gang: g0, count: 1 }));
}

#[test]
fn test_gossip_copies_second_hand() {
    let mut w = world(36);
    let (teller, listener) = two_civilians(&w);
    let thief = w.citizens().into_iter().find(|&c| c != teller && c != listener).expect("thief");
    w.remember_crime(teller, thief, Crime::Theft, 0.8);
    social::gossip(&mut w, teller, listener);
    let copy = w
        .comp::<Memory>(listener)
        .expect("mem")
        .entries
        .iter()
        .find(|m| m.kind == MemoryKind::SawCrime && m.subject == Some(thief))
        .expect("listener heard about it");
    assert!(copy.second_hand);
    assert!((copy.salience - 0.48).abs() < 1e-6, "salience {}", copy.salience);
    assert_eq!(copy.crime, Some(Crime::Theft));
    assert!(w.edge(listener, thief).is_some_and(|e| e.affinity < 0.0), "gossip costs the subject affinity");
}

#[test]
fn test_edges_deterministic_order() {
    let mut a = world(42);
    let mut b = world(42);
    a.run_ticks(3 * TICKS_PER_DAY);
    b.run_ticks(3 * TICKS_PER_DAY);
    assert!(!a.edges.is_empty());
    let ka: Vec<_> = a.edges.iter().map(|(k, e)| (*k, e.kind, e.affinity.to_bits(), e.trust.to_bits())).collect();
    let kb: Vec<_> = b.edges.iter().map(|(k, e)| (*k, e.kind, e.affinity.to_bits(), e.trust.to_bits())).collect();
    assert_eq!(ka, kb);
    assert_eq!(a.neighbours.len(), b.neighbours.len());
    assert_eq!(a.enemies, b.enemies);
}

#[test]
fn test_widow_can_remarry_and_grieves() {
    let mut w = world(37);
    let (a, b) = two_civilians(&w);
    w.set_spouse(a, b);
    assert!(social::has_spouse(&w, a));
    w.kill(b, citysim::DeathCause::Violence);
    assert!(!social::has_spouse(&w, a), "the widow is free to remarry");
    assert_eq!(w.edge(a, b).map(|e| e.kind), Some(RelKind::Spouse), "the edge itself stays");
    let grief =
        w.comp::<Memory>(a).expect("mem").entries.iter().any(|m| m.kind == MemoryKind::Grief && m.subject == Some(b));
    assert!(grief, "no Grief memory");
    assert!(w.enemies_of(b).next().is_none());
}

#[test]
fn test_own_partner_reservation_does_not_hide_partner() {
    let mut w = world(38);
    let (a, b) = two_civilians(&w);
    let bar = w.building_of_kind(BuildingKind::Bar).expect("bar");
    for id in [a, b] {
        w.abort_plan(id);
        w.leave_building(id);
        w.enter_building(id, bar);
    }
    assert_eq!(social::best_colocated_partner(&w, a, -1.0, false), Some(b));
    let expires = w.tick + 100;
    w.reserve(a, citysim::exec::ReservationKind::Partner { other: b }, expires);
    assert_eq!(social::best_colocated_partner(&w, a, -1.0, false), Some(b), "my own reservation is not a block");
    let c = w.citizens().into_iter().find(|&c| c != a && c != b && w.has::<Brain>(c)).expect("c");
    w.abort_plan(c);
    w.leave_building(c);
    w.enter_building(c, bar);
    assert_ne!(social::best_colocated_partner(&w, c, -1.0, false), Some(b), "someone else's reservation is");
}

#[test]
fn test_indices_rebuilt_on_load() {
    let mut w = world(39);
    w.run_ticks(2 * TICKS_PER_DAY);
    let text = citysim::save::to_ron(&w);
    let loaded = citysim::save::from_ron(&text).expect("load");
    assert_eq!(loaded.neighbours, w.neighbours);
    assert_eq!(loaded.spouses, w.spouses);
    assert_eq!(loaded.enemies, w.enemies);
    assert!(!loaded.spouses.is_empty());
}
