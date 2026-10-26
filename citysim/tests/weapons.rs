//! M17 phase 4 (docs/M17_OUTSIDE.md § 3-4, "Revised 2026-10-12"): the
//! weapons that reach the ledger, and scrip. A parent's uplink is a node
//! behind a tier-3 Trunk from its branch's Ledger, its ICE capped by the
//! parent's Deck tier; a theft cuts the parent's edge, a wipe drops its tech
//! tier and caps its branches, a Ledger run siphons its treasury; edge
//! recovers only while every track holds `data_floor`; a sanction cuts
//! income and costs the sanctioner; a parent under its upkeep for
//! `death_days` dies, bankrupting its branches and wiping its scrip. Scrip
//! is a wage share whose coins go to the parent at issue, spent at the
//! issuer's buildings at par (at health below `scrip_par_min`), converted at
//! a Fixer, wiped with the parent. Every crossing keeps the coin identity:
//! scrip is not coins.

use citysim::outside::OutsideOrder;
use citysim::systems::{econ, ownership, parents, virt};
use citysim::virt::{LinkKind, NodeKind, Purpose, Track};
use citysim::{Config, Corp, EntityId, EventKind, PlayerCommand, Wallet, World, TICKS_PER_DAY};

fn account(w: &World, name: &str) -> citysim::outside::OutsideFaction {
    w.outside.factions.iter().find(|f| f.name == name).cloned().unwrap_or_else(|| panic!("no account {name}"))
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == name)).expect("corp")
}

fn city() -> World {
    let mut w = World::new(42, Config::load());
    w.run_ticks(2 * TICKS_PER_DAY + 30);
    w
}

fn god(w: &mut World, cmd: PlayerCommand) {
    w.push_command(cmd);
    w.apply_commands();
}

/// An adult employed by `corp`.
fn employee_of(w: &World, corp: EntityId) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| w.comp::<citysim::Job>(a).and_then(|j| j.employer).and_then(|b| w.owner_of(b)) == Some(corp))
        .expect("an employee")
}

#[test]
fn test_uplink_node_behind_a_tier3_trunk_with_capped_ice() {
    let w = city();
    let p = account(&w, "Militech International");
    let n = parents::uplink_node(&w, p.id).expect("an uplink");
    let node = w.virt.node(n).expect("node");
    assert_eq!(node.kind, NodeKind::Uplink(p.id));
    let militech = corp_named(&w, "Militech");
    let ledger = virt::ledger_of(&w, militech).expect("the branch's Ledger");
    let link = w
        .virt
        .links
        .iter()
        .find(|l| (l.a == n && l.b == ledger) || (l.a == ledger && l.b == n))
        .expect("a link to the branch's Ledger");
    assert_eq!(link.kind, LinkKind::Trunk);
    assert_eq!(link.tier, w.config.outside.parents.uplink_link_tier);
    let deck = p.parent.as_ref().expect("p").tech[Track::Deck.index()];
    assert_eq!(virt::ice_eff(&w, n), w.config.outside.parents.uplink_ice.min(deck));
    // The node's store is the parent's Data (a window, not the branch's holding).
    assert_eq!(node.store, p.parent.as_ref().expect("p").data);
    assert!(virt::holding(&w, militech, Track::Deck) < node.store.get(Track::Deck), "no branch holding");
}

#[test]
fn test_theft_cuts_edge_and_wipe_drops_the_tier_and_caps_the_branch() {
    let mut w = city();
    let p = account(&w, "Militech International");
    let zetatech = corp_named(&w, "Zetatech");
    let militech = corp_named(&w, "Militech");
    let edge0 = p.parent.as_ref().expect("p").edge;
    let data0 = p.parent.as_ref().expect("p").data.clone();
    god(
        &mut w,
        PlayerCommand::ForceUplinkRun { parent: p.id, purpose: Purpose::Data { wipe: false }, patron: Some(zetatech) },
    );
    let p1 = account(&w, "Militech International");
    let pp = p1.parent.as_ref().expect("p");
    let taken = data0.total() - pp.data.total();
    assert!(taken > 0, "Data taken");
    let expect = edge0 - w.config.outside.parents.edge_per_unit * taken as f32;
    assert!((pp.edge - expect).abs() < 1e-4, "edge {} vs {expect}", pp.edge);
    assert!(w.events.iter().any(|e| e.kind == EventKind::UplinkBreached));
    let deck0 = pp.tech[Track::Deck.index()];
    god(
        &mut w,
        PlayerCommand::ForceUplinkRun { parent: p.id, purpose: Purpose::Data { wipe: true }, patron: Some(zetatech) },
    );
    let pp = account(&w, "Militech International").parent.expect("p");
    assert_eq!(pp.data.get(Track::Deck), 0, "wiped");
    assert_eq!(pp.tech[Track::Deck.index()], deck0 - 1, "a tier lost abroad");
    assert!(virt::maker_tier(&w, militech, Track::Deck) <= pp.tech[Track::Deck.index()], "the branch is capped");
}

#[test]
fn test_ledger_siphon_crosses_in_to_the_patron() {
    let mut w = city();
    let p = account(&w, "Arasaka Global");
    let zetatech = corp_named(&w, "Zetatech");
    let base = econ::identity(&w);
    let (t0, z0) = (p.treasury, w.purse(Some(zetatech)));
    god(&mut w, PlayerCommand::ForceUplinkRun { parent: p.id, purpose: Purpose::Ledger, patron: Some(zetatech) });
    let t1 = account(&w, "Arasaka Global").treasury;
    let cfg = w.config.outside.parents.clone();
    let take = ((cfg.uplink_frac as f64 * t0 as f64).floor() as i64).min(cfg.uplink_cap);
    assert_eq!(t0 - t1, take);
    assert_eq!(w.purse(Some(zetatech)) - z0, take);
    assert_eq!(econ::identity(&w), base);
}

#[test]
fn test_income_applies_edge_and_the_sanction_cut() {
    // base 3000, market 1.0, edge 0.5, rep 0.5, no exposure, cut 0.08.
    assert_eq!(parents::parent_income(3000, 1.0, 0.5, 0.5, 0.0, 0.0, 0.0), 1500);
    assert_eq!(parents::parent_income(3000, 1.0, 0.5, 0.5, 0.0, 0.0, 0.08), 1380);
}

#[test]
fn test_edge_recovers_only_above_the_data_floor() {
    let mut w = city();
    let id = account(&w, "Arasaka Global").id;
    god(&mut w, PlayerCommand::SetParentEdge { parent: id, edge: 0.5 });
    w.run_ticks(TICKS_PER_DAY);
    let up = account(&w, "Arasaka Global").parent.expect("p").edge;
    assert!(up > 0.5, "recovering with Data abroad: {up}");
    if let Some(p) = w.outside.faction_mut(id).and_then(|f| f.parent.as_mut()) {
        p.data.units[Track::Chrome.index()] = 0;
        p.edge = 0.5;
    }
    w.run_ticks(TICKS_PER_DAY);
    assert_eq!(account(&w, "Arasaka Global").parent.expect("p").edge, 0.5, "a track under the floor: no recovery");
}

#[test]
fn test_sanction_cuts_and_costs_its_sanctioner() {
    let mut w = city();
    let a = account(&w, "Arasaka Global").id;
    let m = account(&w, "Militech International").id;
    // The control: the same world without the sanction.
    let mut c = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("a copy");
    let base = econ::identity(&w);
    parents::sanction(&mut w, Some(a), m, 10, 0.08);
    assert!((parents::sanction_cut(&w, m) - 0.08).abs() < 1e-6);
    assert_eq!(parents::sanction_cut(&w, a), 0.0);
    w.run_ticks(TICKS_PER_DAY);
    c.run_ticks(TICKS_PER_DAY);
    let cost = w.config.outside.parents.sanction_cost;
    assert_eq!(
        account(&c, "Arasaka Global").treasury - account(&w, "Arasaka Global").treasury,
        cost,
        "the sanction's cost"
    );
    let inc =
        |x: &World| account(x, "Militech International").parent.expect("p").ledger.income.back().copied().unwrap_or(0);
    assert!(inc(&w) < inc(&c), "the sanctioned income is cut: {} vs {}", inc(&w), inc(&c));
    assert_eq!(econ::identity(&w), base);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Sanctioned));
}

/// A parent under its upkeep dies on `death_days` failing, not a day before;
/// its corp branches go bankrupt, its treasury is absorbed, its scrip wiped.
#[test]
fn test_parent_dies_at_death_days_and_takes_its_branches() {
    let mut c = Config::load();
    c.outside.parents.factions[1].income = 0;
    let mut w = World::new(42, c);
    let cfg = w.config.outside.parents.clone();
    let id = account(&w, "Militech International").id;
    let base = econ::identity(&w);
    let holder = w.citizens()[0];
    w.outside.scrip.insert(holder, vec![(id, 40)]);
    // Its reserves run under its seed (the creditors' second condition).
    if let Some(f) = w.outside.faction_mut(id) {
        f.treasury_ref = f.treasury * 2;
    }
    let died_on = |w: &World| account(w, "Militech International").dead;
    w.run_ticks(TICKS_PER_DAY + 30);
    while account(&w, "Militech International").parent.expect("p").failing_since.is_none() {
        w.run_ticks(TICKS_PER_DAY);
        assert!(w.day() < 30, "never failing");
    }
    let since = account(&w, "Militech International").parent.expect("p").failing_since.expect("failing");
    let due_day = (since + u64::from(cfg.death_days) * TICKS_PER_DAY) / TICKS_PER_DAY;
    while w.day() < due_day {
        assert!(!died_on(&w), "dead on day {} before day {due_day}", w.day());
        w.run_ticks(TICKS_PER_DAY);
    }
    w.run_ticks(TICKS_PER_DAY);
    assert!(died_on(&w), "dead by day {due_day}");
    assert_eq!(account(&w, "Militech International").treasury, 0, "absorbed abroad");
    assert!(!w.corps().into_iter().any(|c| w.comp::<Corp>(c).is_some_and(|x| x.parent == Some(id))), "no branch left");
    assert!(w.events.iter().any(|e| e.kind == EventKind::ParentDied));
    assert!(w.events.iter().any(|e| e.kind == EventKind::ScripWiped));
    assert!(!w.outside.scrip.get(&holder).is_some_and(|v| v.iter().any(|&(i, _)| i == id)));
    assert_eq!(econ::identity(&w), base, "the absorbed treasury is booked");
}

/// A failing parent with its reserves above its `treasury_ref` is not called in.
/// (`death_days` 7 instead of 21 so the run is 20 days, not 40: the test
/// needs a failing streak past `death_days`, not the shipped length.)
#[test]
fn test_rich_failing_parent_lives() {
    let mut c = Config::load();
    c.outside.parents.factions[1].income = 0;
    c.outside.parents.death_days = 7;
    let mut w = World::new(42, c);
    let id = account(&w, "Militech International").id;
    if let Some(f) = w.outside.faction_mut(id) {
        f.treasury_ref = 1;
    }
    w.run_ticks(20 * TICKS_PER_DAY);
    let f = account(&w, "Militech International");
    let since = f.parent.as_ref().expect("p").failing_since.expect("failing");
    assert!(w.tick - since >= 7 * TICKS_PER_DAY, "failing past death_days ({} ticks)", w.tick - since);
    assert!(!f.dead, "its reserves hold");
}

/// A wage from a parented corp branch: `scrip_wage_share` in scrip, its coins
/// to the parent at issue; the identity holds (scrip is not coins).
#[test]
fn test_scrip_wage_split_keeps_the_identity() {
    let mut w = city();
    let militech = corp_named(&w, "Militech");
    let id = account(&w, "Militech International").id;
    let a = employee_of(&w, militech);
    let base = econ::identity(&w);
    let (c0, p0) = (w.purse(Some(a)), account(&w, "Militech International").treasury);
    let s0 = parents::scrip_of(&w, a, id);
    ownership::pay(&mut w, Some(militech), Some(a), 100, ownership::Flow::Wage);
    let share = (w.config.outside.parents.scrip_wage_share * 100.0).round() as i64;
    assert_eq!(parents::scrip_of(&w, a, id) - s0, share);
    assert_eq!(w.purse(Some(a)) - c0, 100 - share);
    assert_eq!(account(&w, "Militech International").treasury - p0, share);
    assert_eq!(econ::identity(&w), base);
}

/// Scrip spends at the issuer's buildings: at par while the parent's health
/// holds, at health below `scrip_par_min`.
#[test]
fn test_scrip_spends_at_par_then_at_health() {
    let mut w = city();
    let militech = corp_named(&w, "Militech");
    let id = account(&w, "Militech International").id;
    let a = w.citizens().into_iter().find(|&x| w.has::<Wallet>(x)).expect("an agent");
    w.comp_mut::<Wallet>(a).expect("w").coins = 0;
    w.outside.scrip.insert(a, vec![(id, 30)]);
    let base = econ::identity(&w);
    let paid = ownership::pay(&mut w, Some(a), Some(militech), 20, ownership::Flow::Rent);
    assert_eq!(paid, 20, "the scrip covered the rent");
    assert_eq!(parents::scrip_of(&w, a, id), 10, "at par");
    assert_eq!(econ::identity(&w), base);
    if let Some(p) = w.outside.faction_mut(id).and_then(|f| f.parent.as_mut()) {
        p.health = 0.2;
    }
    w.outside.scrip.insert(a, vec![(id, 100)]);
    let paid = ownership::pay(&mut w, Some(a), Some(militech), 10, ownership::Flow::Rent);
    assert_eq!(paid, 10);
    assert_eq!(parents::scrip_of(&w, a, id), 50, "10 coins at health 0.2 cost 50 scrip");
}

/// A broke holder converts at a Fixer: `scrip × health × (1 − fixer_cut)` to
/// the holder, the cut to the Fixer's owner, from the parent's treasury.
#[test]
fn test_conversion_at_a_fixer() {
    let mut w = city();
    let id = account(&w, "Militech International").id;
    let a = w.citizens().into_iter().find(|&x| w.has::<Wallet>(x)).expect("an agent");
    w.comp_mut::<Wallet>(a).expect("w").coins = 0;
    w.outside.scrip.insert(a, vec![(id, 100)]);
    let base = econ::identity(&w);
    let health = account(&w, "Militech International").parent.expect("p").health;
    parents::convert_pass(&mut w, true);
    let got = w.purse(Some(a));
    let expect = (100.0 * health * (1.0 - w.config.outside.parents.fixer_cut)).floor() as i64;
    assert_eq!(got, expect, "converted at health {health}");
    assert_eq!(parents::scrip_of(&w, a, id), 0);
    assert_eq!(econ::identity(&w), base);
}

/// `BanScrip`: holders redeemed at health, no issuance after.
#[test]
fn test_ban_scrip_redeems_and_stops_issuance() {
    let mut w = city();
    let militech = corp_named(&w, "Militech");
    let id = account(&w, "Militech International").id;
    let a = employee_of(&w, militech);
    w.outside.scrip.insert(a, vec![(id, 50)]);
    let c0 = w.purse(Some(a));
    let base = econ::identity(&w);
    god(&mut w, PlayerCommand::BanScrip(true));
    assert!(w.purse(Some(a)) > c0, "redeemed");
    assert_eq!(parents::scrip_of(&w, a, id), 0);
    ownership::pay(&mut w, Some(militech), Some(a), 100, ownership::Flow::Wage);
    assert_eq!(parents::scrip_of(&w, a, id), 0, "no issuance under the ban");
    assert_eq!(econ::identity(&w), base);
}

/// A branch whose tech upkeep lapses gets Data from its parent's store.
#[test]
fn test_data_shipped_to_a_lapsing_branch() {
    let mut w = city();
    let militech = corp_named(&w, "Militech");
    let lab = citysim::systems::tech::labs_of(&w, militech)[0];
    let n = virt::node_of_building(&w, lab).expect("the Lab's node");
    // Its Labs run dry: the upkeep would lapse for want of Data.
    for x in w.virt.nodes.iter_mut().filter(|x| x.owner == Some(militech)) {
        x.store = Default::default();
    }
    if let Some(c) = w.comp_mut::<Corp>(militech) {
        c.tech.tier[Track::Deck.index()] = c.tech.tier[Track::Deck.index()].max(2);
    }
    let before = w.virt.node(n).expect("n").store.total();
    let id = account(&w, "Militech International").id;
    let abroad = account(&w, "Militech International").parent.expect("p").data.total();
    parents::ship_data(&mut w, id);
    assert!(w.virt.node(n).expect("n").store.total() > before, "Data into the Lab");
    assert!(account(&w, "Militech International").parent.expect("p").data.total() < abroad, "out of the parent's");
    assert!(w.events.iter().any(|e| e.kind == EventKind::DataShipped));
}

/// A Campaign order acts on its rival (a posting or a run order).
#[test]
fn test_campaign_acts_on_the_rival() {
    let mut w = city();
    let a = account(&w, "Arasaka Global").id;
    let m = account(&w, "Militech International").id;
    if let Some(p) = w.outside.faction_mut(a).and_then(|f| f.parent.as_mut()) {
        p.rival = Some(m);
    }
    god(&mut w, PlayerCommand::SetOutsideOrder { id: a, order: OutsideOrder::Campaign, days: 5 });
    w.run_ticks(2 * TICKS_PER_DAY);
    assert!(w.events.iter().any(|e| e.kind == EventKind::CampaignPosted), "the campaign acted");
}
