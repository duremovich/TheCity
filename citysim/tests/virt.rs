//! M14 phase 1: the Virt plane (plan 1.10). The plane is a game abstraction:
//! abstract nodes and links over the city map, contested by seeded dice.

use std::collections::{BTreeSet, VecDeque};

use citysim::systems::{corps, ownership, security, street, tech, virt};
use citysim::virt::{LinkKind, NodeId, NodeKind, SecurityProfile, Track};
use citysim::{Building, BuildingKind, Config, Corp, EntityId, World};
use rand::{Rng, SeedableRng};

fn world() -> World {
    World::new(42, Config::load())
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == name)).expect("seeded corp")
}

fn lab_of(w: &World, corp: EntityId, focus: Track) -> EntityId {
    w.buildings_of_kind(BuildingKind::Lab)
        .iter()
        .copied()
        .find(|&b| w.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(corp) && bd.focus == Some(focus)))
        .expect("a seeded Lab")
}

/// Plan 1.10: the seeded plane's shape and the Seeding table's Labs.
#[test]
fn test_seed_plane_shape() {
    let w = world();
    let p = &w.virt;
    let alive = p.alive_count();
    assert!((40..=60).contains(&alive), "{alive} nodes at seed");
    let districts = w.districts.len();
    for d in 0..districts {
        assert_eq!(p.nodes[d].kind, NodeKind::Public(citysim::DistrictId(d as u8)), "NodeId({d}) = Public({d})");
        let streets = p.adj[d].iter().filter(|&&(_, l)| p.links[usize::from(l)].kind == LinkKind::Street).count();
        assert!(streets >= 1, "Public {d} has a Street link");
    }
    // A tier-1 search from Public 0 reaches every alive non-Ledger node.
    let mut seen = BTreeSet::from([NodeId(0)]);
    let mut queue = VecDeque::from([NodeId(0)]);
    while let Some(n) = queue.pop_front() {
        for &(m, l) in &p.adj[n.index()] {
            if p.links[usize::from(l)].tier <= 1 && seen.insert(m) {
                queue.push_back(m);
            }
        }
    }
    for (i, n) in p.nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        let id = NodeId(i as u16);
        let kinds: Vec<LinkKind> = p.adj[i].iter().map(|&(_, l)| p.links[usize::from(l)].kind).collect();
        match n.kind {
            NodeKind::Ledger(_) => {
                assert!(!seen.contains(&id), "a tier-1 deck never reaches a Ledger");
                assert!(kinds.contains(&LinkKind::Trunk), "Ledger {i} has a Trunk");
                assert!(!kinds.contains(&LinkKind::Access), "Ledger {i} has no Access");
            }
            _ => assert!(seen.contains(&id), "node {i} ({:?}) reachable at tier 1", n.kind),
        }
    }
    // The Seeding table: kind, owner, focus, district.
    let mut labs: Vec<(String, Track, String)> = w
        .buildings_of_kind(BuildingKind::Lab)
        .iter()
        .map(|&b| {
            let bd = w.comp::<Building>(b).expect("lab");
            let owner = w.owner_label(bd.owner);
            let district = w.district(w.district_of_building(b)).name.clone();
            println!("Lab {} {:?} {owner} {:?} {district}", b.index, bd.rect, bd.focus);
            assert!(virt::node_of_building(&w, b).is_some(), "every Lab has a node");
            (owner, bd.focus.expect("focus"), district)
        })
        .collect();
    labs.sort();
    let mut want = vec![
        ("Arasaka".to_string(), Track::Deck, "Vats".to_string()),
        ("Militech".to_string(), Track::Deck, "Mid West".to_string()),
        ("Zetatech".to_string(), Track::Chrome, "Spire".to_string()),
        ("Zetatech".to_string(), Track::Industry, "Civic".to_string()),
    ];
    want.sort();
    assert_eq!(labs, want);
    println!("nodes {alive}, links {}", p.links.len());
}

/// V4: an `Acquired` keeps the node, its ICE and its store; the Trunks follow the new owner.
#[test]
fn test_relink_keeps_ice_and_store_across_acquire() {
    let mut w = world();
    let (militech, arasaka) = (corp_named(&w, "Militech"), corp_named(&w, "Arasaka"));
    let lab = lab_of(&w, militech, Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    w.comp_mut::<Building>(lab).expect("lab").security.ice = 2;
    w.virt.node_mut(n).expect("node").store.units[Track::Deck.index()] = 500;
    w.comp_mut::<Corp>(arasaka).expect("corp").treasury = 50_000;
    assert!(corps::acquire(&mut w, arasaka, lab, 1200, "test"));
    virt::relink(&mut w);
    assert_eq!(virt::node_of_building(&w, lab), Some(n), "same NodeId");
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(2), "ICE kept");
    assert_eq!(w.virt.node(n).expect("node").store.get(Track::Deck), 500, "store kept");
    assert_eq!(virt::owner_of(&w, n), Some(arasaka));
    let (la, lm) = (virt::ledger_of(&w, arasaka).expect("ledger"), virt::ledger_of(&w, militech).expect("ledger"));
    let trunk_to = |l: NodeId| {
        w.virt.adj[n.index()].iter().any(|&(m, k)| m == l && w.virt.links[usize::from(k)].kind == LinkKind::Trunk)
    };
    assert!(trunk_to(la) && !trunk_to(lm), "the Trunk re-pointed to the buyer's Ledger");
}

/// V1: a node whose building goes derelict dies with its id kept; re-owned, it revives.
#[test]
fn test_node_ids_stable_and_revived() {
    let mut w = world();
    let nutrix = corp_named(&w, "Nutrix");
    let bar = ownership::owned_of_kind(&w, Some(nutrix), BuildingKind::Bar)[0];
    let n = virt::node_of_building(&w, bar).expect("a corp Bar has a node");
    w.comp_mut::<Building>(bar).expect("bar").security.ice = 1;
    let count = w.virt.nodes.len();
    assert!(street::make_derelict(&mut w, bar, "test"));
    virt::run(&mut w);
    assert!(!w.virt.nodes[n.index()].alive, "dead while derelict");
    assert_eq!(virt::node_of_building(&w, bar), None);
    assert!(street::restore(&mut w, bar, Some(nutrix), "test"));
    virt::run(&mut w);
    assert!(w.virt.nodes[n.index()].alive, "revived");
    assert_eq!(virt::node_of_building(&w, bar), Some(n), "same id");
    assert_eq!(w.virt.nodes.len(), count, "no node appended");
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(1), "its ICE kept");
}

/// V4: the midnight relink changes nothing when nothing changed.
#[test]
fn test_midnight_relink_is_noop() {
    let mut w = world();
    virt::relink(&mut w);
    let (nodes, links) = (w.virt.nodes.clone(), w.virt.links.clone());
    virt::relink(&mut w);
    assert_eq!(w.virt.nodes, nodes);
    assert_eq!(w.virt.links, links);
}

/// V25: the maker's Deck tier caps ICE; the city's own is uncapped; arrears switch it off.
#[test]
fn test_ice_eff_caps_and_arrears() {
    let mut w = world();
    let (arasaka, militech) = (corp_named(&w, "Arasaka"), corp_named(&w, "Militech"));
    let lab = lab_of(&w, arasaka, Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    assert_eq!(w.comp::<Corp>(militech).expect("corp").tech.tier_of(Track::Deck), 2);
    w.comp_mut::<Building>(lab).expect("lab").security =
        SecurityProfile { ice: 3, ice_maker: Some(militech), ice_arrears: 0 };
    assert_eq!(virt::ice_eff(&w, n), 2, "capped by Militech's Deck 2");
    w.comp_mut::<Building>(lab).expect("lab").security.ice_maker = None;
    assert_eq!(virt::ice_eff(&w, n), 3, "the city's own ICE is uncapped");
    w.comp_mut::<Building>(lab).expect("lab").security.ice_arrears = 2;
    assert_eq!(virt::ice_eff(&w, n), 0, "two days unpaid switch it off");
    assert_eq!(virt::def(&w, n), 0);
}

/// V8: `contest_f` over integer tiers is M13's contest: the same p and the same draw.
#[test]
fn test_contest_f_equals_m13_contest() {
    let step = 0.25f32;
    for a in 0u8..=3 {
        for b in 0u8..=3 {
            let m13_p = (0.5 + step * (f32::from(a) - f32::from(b))).clamp(0.05, 0.95);
            assert_eq!(security::contest_p(f32::from(a), f32::from(b), step), m13_p, "p({a}, {b})");
            let mut r1 = rand_chacha::ChaCha8Rng::seed_from_u64(u64::from(a) * 10 + u64::from(b));
            let mut r2 = r1.clone();
            for _ in 0..64 {
                let roll: f32 = r1.random();
                let m13 = roll < m13_p;
                assert_eq!(security::contest(a, b, step, &mut r2), m13, "draw ({a}, {b})");
            }
        }
    }
}

/// V26: ICE upkeep and installs move coins between purses only, and the
/// ledger column is the sum of the upkeep table over the ICE'd nodes.
#[test]
fn test_ice_upkeep_and_install_flows_conserve() {
    let mut w = world();
    let total = ownership::total_coins(&w);
    let expected: i64 = (0..w.virt.nodes.len())
        .filter(|&i| w.virt.nodes[i].alive)
        .filter_map(|i| virt::profile(&w, NodeId(i as u16)).map(|p| p.ice))
        .map(|ice| w.config.ice.upkeep(ice))
        .sum();
    let before = w.stats.current.virt.flow_ice_upkeep;
    tech::ice_upkeep(&mut w);
    assert_eq!(w.stats.current.virt.flow_ice_upkeep - before, expected, "flow_ice_upkeep = the upkeep table");
    assert_eq!(ownership::total_coins(&w), total, "upkeep conserves coins");
    // An install bought from a Security corp, and a self-install.
    let nutrix = corp_named(&w, "Nutrix");
    let market = ownership::owned_of_kind(&w, Some(nutrix), BuildingKind::Market)[0];
    let n = virt::node_of_building(&w, market).expect("node");
    let ice = virt::profile(&w, n).map(|p| p.ice).expect("profile");
    assert!(virt::install_ice(&mut w, Some(nutrix), n));
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(ice + 1));
    let arasaka = corp_named(&w, "Arasaka");
    let lab = virt::node_of_building(&w, lab_of(&w, arasaka, Track::Deck)).expect("node");
    assert!(virt::install_ice(&mut w, Some(arasaka), lab), "Arasaka self-installs at Deck 3");
    assert_eq!(virt::profile(&w, lab).and_then(|p| p.ice_maker), Some(arasaka));
    assert_eq!(ownership::total_coins(&w), total, "installs conserve coins");
    assert!(w.stats.current.virt.ice_raised >= 2);
}

/// V35: the hacking draws are keyed: the world stream is the same with the plane on and off.
#[test]
fn test_hacking_draw_untouches_world_stream() {
    let mut on = World::new(42, Config::load());
    let mut off = World::new(42, Config::load().virt_off());
    let (a, b): (u64, u64) = (on.rng.world().random(), off.rng.world().random());
    assert_eq!(a, b);
    let h: Vec<f32> = on.citizens().iter().filter_map(|&c| on.comp::<citysim::Skills>(c)).map(|s| s.hacking).collect();
    assert!(h.iter().all(|&x| (0.0..=0.8).contains(&x)), "hacking in 0..=hack_seed_scale");
    let clear = h.iter().filter(|&&x| x >= 0.4).count() as f32 / h.len() as f32;
    assert!((0.12..=0.30).contains(&clear), "about 21 % clear deck_shop_min: {clear}");
}
