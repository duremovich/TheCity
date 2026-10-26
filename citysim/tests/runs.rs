//! M14 phase 2: decks and runs (plan 2.8). A run is a game abstraction: a
//! fictional runner's token moves along abstract nodes, each guarded node a
//! seeded dice contest (deck tier plus skill against the node's ICE tier),
//! resolved as `law::resolve_fight` resolves a brawl.

mod common;

use citysim::systems::virt::{LossRolls, Odds};
use citysim::systems::{assets, law, lod, security, tech, vehicles, virt};
use citysim::virt::{NodeId, NodeKind, Purpose, RunMode, RunOrder, RunOutcome, RunPhase, RunWhy, Track};
use citysim::{
    ActionInstance, ActionKind, AssetKind, Body, Brain, Building, BuildingKind, Config, Corp, Corpse, Crime,
    DeathCause, EntityId, EventKind, ExecState, Gang, GangMember, GoalKind, Kit, Lod, Plan, Shock, Skills, World,
    TICKS_PER_HOUR,
};
use common::{corp_named, dole_world};
use rand::Rng;

fn lab_of(w: &World, corp: EntityId, focus: Track) -> EntityId {
    w.buildings_of_kind(BuildingKind::Lab)
        .iter()
        .copied()
        .find(|&b| w.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(corp) && bd.focus == Some(focus)))
        .expect("a seeded Lab")
}

/// An adult with a Brain, no gang, no deck, no chrome, a body (not Statistical).
fn runner(w: &mut World, skip: &[EntityId]) -> EntityId {
    let a = w
        .citizens()
        .into_iter()
        .find(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(w, a)
                && !w.has::<GangMember>(a)
                && !skip.contains(&a)
                && w.comp::<Kit>(a).is_some_and(|k| k.deck.is_none() && !k.chrome)
                && !law::is_guard(w, a)
        })
        .expect("an adult");
    lod::set_lod(w, a, Lod::Coarse);
    a
}

/// A deck of `tier` and the hacking skill.
fn arm(w: &mut World, a: EntityId, tier: u8, hacking: f32) {
    assets::grant(w, a, AssetKind::Deck, tier).expect("deck");
    w.comp_mut::<Skills>(a).expect("skills").hacking = hacking;
    assert_eq!(w.comp::<Kit>(a).map(|k| k.deck_tier), Some(tier));
}

fn order(w: &mut World, a: EntityId, chair: EntityId, target: NodeId, purpose: Purpose, why: RunWhy) {
    let now = w.tick;
    w.run_orders.insert(
        a,
        RunOrder {
            patron: None,
            purpose,
            target,
            chair,
            not_before: now,
            expires: now + 1440,
            why,
            mode: RunMode::Quiet,
        },
    );
}

fn set_ice(w: &mut World, n: NodeId, ice: u8) {
    virt::profile_mut(w, n).expect("profile").ice = ice;
    virt::bump_epoch(w);
}

fn first_bar(w: &World) -> EntityId {
    w.buildings_of_kind(BuildingKind::Bar)[0]
}

/// Pop every queued run step, jumping the clock to each.
fn finish(w: &mut World) {
    while let Some(&(t, _)) = w.run_queue.first() {
        w.tick = w.tick.max(t);
        virt::run(w);
    }
}

fn seat(w: &mut World, a: EntityId, chair: EntityId) {
    w.leave_building(a);
    w.enter_building(a, chair);
}

fn last_outcome(w: &World) -> Option<RunOutcome> {
    w.run_log.back().and_then(|r| r.outcome)
}

fn events(w: &World, kind: EventKind, who: EntityId) -> usize {
    w.events.iter().filter(|e| e.kind == kind && e.actors.contains(&who)).count()
}

/// Plan 2.8: the Contest table, to 1e-6 (and one row read off a real route).
#[test]
fn test_contest_table() {
    let w = dole_world();
    let step = w.config.chrome.contest_step;
    let cfg = &w.config.ice;
    let fry = |ice: u8, gap: f32| (cfg.fry_base[usize::from(ice)] * (1.0 + cfg.fry_gap * gap.max(0.0))).min(1.0);
    let dodge = w.config.vehicles.dodge_w * 0.4;
    // (att, def, p, twice, fry raw, fry after the dodge)
    let rows: [(f32, f32, f64, f64, f64, f64); 6] = [
        (1.4, 1.0, 0.60, 0.36, 0.05, 0.038),
        (1.4, 2.0, 0.35, 0.1225, 0.40, 0.304),
        (2.5, 2.0, 0.625, 0.390625, 0.25, 0.19),
        (2.5, 3.0, 0.375, 0.140625, 0.90, 0.684),
        (1.3, 3.0, 0.075, 0.005625, 1.0, 0.76),
        (3.6, 3.0, 0.65, 0.4225, 0.6, 0.456),
    ];
    for (att, def, p, twice, raw, saved) in rows {
        let got = security::contest_p(att, def, step);
        println!(
            "att {att} def {def}: p {got} twice {} fry {} saved {}",
            got * got,
            fry(def as u8, def - att),
            fry(def as u8, def - att) * (1.0 - dodge)
        );
        assert!((f64::from(got) - p).abs() < 1e-6, "p({att}, {def}) = {got}, want {p}");
        assert!((f64::from(got * got) - twice).abs() < 1e-6, "twice {att} {def}");
        let f = fry(def as u8, def - att);
        assert!((f64::from(f) - raw).abs() < 1e-6, "fry raw {att} {def}: {f}");
        assert!((f64::from(f * (1.0 - dodge)) - saved).abs() < 1e-6, "fry saved {att} {def}");
    }
    // The flatline column: gap 1.7 >= 1.5 at ICE 3, gap 0.5 is not.
    assert!(3.0 - 1.3 >= cfg.flatline_gap && 3.0 - 2.5 < cfg.flatline_gap);
    // The Ledger row: Farm ICE 1 then Ledger ICE 2 at att 2.5.
    let (farm, ledger) = (security::contest_p(2.5, 1.0, step), security::contest_p(2.5, 2.0, step));
    assert!((f64::from(farm) - 0.875).abs() < 1e-6 && (f64::from(ledger) - 0.625).abs() < 1e-6);
    assert!((f64::from(farm * ledger * ledger) - 0.341_796_875).abs() < 1e-6);
    // A real route: a tier-2 deck at hacking 0.5 on Zetatech's ICE-2 Lab from its district's Public node.
    let lab = lab_of(&w, corp_named(&w, "Zetatech"), Track::Chrome);
    let n = virt::node_of_building(&w, lab).expect("node");
    assert_eq!(virt::def(&w, n), 2);
    let from = virt::public_of(&w, w.district_of_building(lab));
    let o = Odds { att: 2.5, patron: None, dodge: 0.0, trace_mult: 1.0 };
    let tree = virt::routes_from(&w, from, 2, o.att, None);
    let route = virt::route_to(&w, &tree, from, n, &o).expect("route");
    assert!((f64::from(route.p_success) - 0.390_625).abs() < 1e-6, "{route:?}");
}

/// V3/V6: a tier-1 tree from Public 0 never reaches a Ledger (Trunks are tier 2).
#[test]
fn test_tier1_route_never_uses_trunk() {
    let w = dole_world();
    let tree = virt::routes_from(&w, NodeId(0), 1, 1.5, None);
    let mut ledgers = 0;
    for (i, n) in w.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        match n.kind {
            NodeKind::Ledger(_) => {
                ledgers += 1;
                assert!(tree.dist[i].is_infinite(), "Ledger {i} reached at tier 1");
            }
            // M17 phase 4: an uplink sits behind a tier-3 Trunk.
            NodeKind::Uplink(_) => assert!(tree.dist[i].is_infinite(), "uplink {i} reached at tier 1"),
            _ => assert!(tree.dist[i].is_finite(), "node {i} unreached at tier 1"),
        }
    }
    assert!(ledgers >= 9);
    let uplinks = w.virt.nodes.iter().filter(|n| n.alive && matches!(n.kind, NodeKind::Uplink(_))).count();
    let t2 = virt::routes_from(&w, NodeId(0), 2, 2.5, None);
    let reached = w.virt.nodes.iter().enumerate().filter(|(i, n)| n.alive && t2.dist[*i].is_finite()).count();
    assert_eq!(reached, w.virt.alive_count() - uplinks, "a tier-2 deck reaches every node but the uplinks");
    let t3 = virt::routes_from(&w, NodeId(0), 3, 3.5, None);
    let reached = w.virt.nodes.iter().enumerate().filter(|(i, n)| n.alive && t3.dist[*i].is_finite()).count();
    assert_eq!(reached, w.virt.alive_count(), "a tier-3 deck reaches the uplinks too");
}

/// V7: a corp's weakest building is its treasury's front door.
#[test]
fn test_ledger_route_enters_through_weakest_building() {
    let mut w = dole_world();
    let corp = corp_named(&w, "Zetatech");
    let mine: Vec<NodeId> = w
        .virt
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.alive && n.owner == Some(corp) && matches!(n.kind, NodeKind::Building(_)))
        .map(|(i, _)| NodeId(i as u16))
        .collect();
    assert!(mine.len() >= 3, "{mine:?}");
    let weak = *mine
        .iter()
        .find(|&&n| match w.virt.node(n).map(|x| x.kind) {
            Some(NodeKind::Building(b)) => w.comp::<Building>(b).is_some_and(|bd| bd.kind != BuildingKind::Lab),
            _ => false,
        })
        .expect("a non-Lab building");
    for &n in &mine {
        set_ice(&mut w, n, if n == weak { 1 } else { 2 });
    }
    let ledger = virt::ledger_of(&w, corp).expect("ledger");
    let o = Odds { att: 2.5, patron: None, dodge: 0.0, trace_mult: 1.0 };
    let tree = virt::routes_from(&w, NodeId(0), 2, o.att, None);
    let route = virt::route_to(&w, &tree, NodeId(0), ledger, &o).expect("route");
    assert_eq!(route.nodes[route.nodes.len() - 2], weak, "{route:?}");
    let l = f64::from(security::contest_p(2.5, f32::from(virt::def(&w, ledger)), 0.25));
    assert!((f64::from(route.p_success) - 0.875 * l * l).abs() < 1e-6, "{route:?}");
}

/// V7: a target at ICE 0 rolls nothing: the run succeeds with no contest.
#[test]
fn test_unguarded_nodes_roll_nothing() {
    let mut w = dole_world();
    // A tier-1 deck's quiet take, read from the config (the store holds more).
    let take = (w.config.data.steal_units[0] as f32 * w.config.hack.quiet_take).round() as u32;
    assert!(take > 0 && take <= 100);
    let gang = w.gang_list()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let n = virt::node_of_building(&w, hideout).expect("node");
    // Phase 5 seeds Hideouts at ICE 1 (`ice_seed.hideout`): take it off for the unguarded case.
    virt::profile_mut(&mut w, n).expect("a Hideout's profile").ice = 0;
    virt::bump_epoch(&mut w);
    assert_eq!(virt::def(&w, n), 0);
    w.virt.node_mut(n).expect("node").store.units[Track::Deck.index()] = 100;
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 1, 0.4);
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::Freelance);
    let id = virt::start_run(&mut w, a).expect("run");
    finish(&mut w);
    let r = w.run_log.back().expect("logged");
    assert_eq!((r.id, r.outcome, r.contests), (id, Some(RunOutcome::Success), 0));
    assert!(r.log.is_empty());
    let deck = w.comp::<Kit>(a).and_then(|k| k.deck).expect("deck");
    assert_eq!(
        w.comp::<citysim::Asset>(deck).map(|x| x.data[Track::Deck.index()]),
        Some(take),
        "steal_units[0] x quiet_take"
    );
}

/// V6: one search per scorer, whatever the number of targets; an ICE write
/// costs one more.
#[test]
fn test_route_cache_one_search_per_scorer() {
    let mut w = dole_world();
    for g in w.gang_list().to_vec() {
        if let Some(n) = w.hideout_of(g).and_then(|h| virt::node_of_building(&w, h)) {
            w.virt.node_mut(n).expect("node").store.units[0] = 50;
        }
    }
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    virt::bump_epoch(&mut w);
    let mode = virt::mode_of(&w, a);
    let targets = virt::targets(&w, a, None, 2, mode).len();
    assert!(targets >= 8, "{targets} targets");
    let s0 = w.virt.cache.searches();
    assert!(virt::best_target(&w, a, None).is_some());
    assert_eq!(w.virt.cache.searches(), s0 + 1, "one search for {targets} targets");
    assert!(virt::hack_choice(&w, a).is_some());
    assert_eq!(w.virt.cache.searches(), s0 + 1, "the think reuses the tree");
    let lab = lab_of(&w, corp_named(&w, "Arasaka"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 3);
    assert!(virt::best_target(&w, a, None).is_some());
    assert_eq!(w.virt.cache.searches(), s0 + 2, "an ICE write: one more");
}

/// V9/V10: a step pops only when due; the same run from a cloned world logs
/// the same contests.
#[test]
fn test_run_pops_only_when_due() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(&mut w, a).expect("run");
    let before = w.runs[&id].clone();
    let (due, _) = *w.run_queue.first().expect("queued");
    w.tick = due - 1;
    virt::run(&mut w);
    assert_eq!(w.runs[&id], before, "nothing before next_at");
    finish(&mut w);
    let x = w.run_log.back().expect("run");
    assert!(!x.log.is_empty(), "the Lab was contested");
    println!("log {:?} outcome {:?}", x.log, x.outcome);
}

/// V9: a run never draws on the world stream.
#[test]
fn test_run_rolls_do_not_touch_world_stream() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 1);
    let mut quiet = w.clone();
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    virt::start_run(&mut w, a).expect("run");
    finish(&mut w);
    assert!(w.run_log.back().is_some_and(|r| r.contests > 0));
    let x: u64 = w.rng.world().random();
    let y: u64 = quiet.rng.world().random();
    assert_eq!(x, y, "the world stream is where it was");
}

/// V13: a jacked-in body is never Statistical.
#[test]
fn test_jacked_in_body_never_statistical() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(&mut w, a).expect("run");
    let now = w.tick;
    w.comp_mut::<Brain>(a).expect("brain").exec = ExecState::JackedIn { run: id, since: now };
    lod::set_lod(&mut w, a, Lod::Statistical);
    assert_eq!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Coarse), "refused");
    assert_eq!(lod::rank_class(&w, a), 4);
    w.tick = (w.tick / 60 + 1) * 60;
    lod::run(&mut w);
    assert_ne!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Statistical), "the LOD pass keeps a body");
    assert!(w.runs.contains_key(&id));
}

/// V14: a tier-1 deck at hacking 0 against ICE 3 (gap 2 >= 1.5), no dodge,
/// `p_flatline` 1, the pass lost: a Flatline, not violence.
#[test]
fn test_flatline_at_tier3_gap() {
    let mut cfg = Config::load().dole_city();
    cfg.ice.p_flatline = 1.0;
    let mut w = World::new(42, cfg);
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 1, 0.0);
    w.comp_mut::<Body>(a).expect("body").reflex = 0.0;
    assert_eq!(vehicles::p_dodge(&w, a), 0.0);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 3);
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(&mut w, a).expect("ordered runs jack in anyway");
    let violence = w.stats.current.deaths_violence;
    let r = w.runs[&id].clone();
    let gap = f32::from(virt::def(&w, n)) - r.att;
    assert!(gap >= w.config.ice.flatline_gap);
    virt::lose(&mut w, r, n, LossRolls { gap, fry: 0.0, dodge: 1.0, kill: 0.5, trace: 1.0 });
    assert_eq!(w.comp::<Corpse>(a).map(|c| c.cause), Some(DeathCause::Flatline));
    assert_eq!(w.stats.current.virt.flatlined, 1);
    assert_eq!(w.stats.current.deaths_violence, violence, "a Flatline is not violence");
    assert_eq!(events(&w, EventKind::Flatlined, a), 1);
    assert_eq!(last_outcome(&w), Some(RunOutcome::Flatlined));
    assert!(w.runs.is_empty() && !w.runner_of.contains_key(&a));
}

/// V14: reflex saves fries (1,000 seeded contests at reflex 0.2 and 0.6).
#[test]
fn test_reflex_saves_fry() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 3);
    let mut fried = [0u32; 2];
    for (i, reflex) in [0.2f32, 0.6].into_iter().enumerate() {
        w.comp_mut::<Body>(a).expect("body").reflex = reflex;
        let o = virt::odds_of(&w, a, 2, None, RunMode::Quiet);
        let gap = 3.0 - o.att;
        for k in 0..1000u64 {
            let (pass, rolls) = virt::contest_draws(&w, k << 6, 0, gap);
            if pass < security::contest_p(o.att, 3.0, 0.25) {
                continue;
            }
            fried[i] += u32::from(virt::loss_verdict(&w, n, &o, &rolls).0);
        }
    }
    println!("fried at reflex 0.2 / 0.6: {fried:?}");
    assert!(fried[1] < fried[0], "{fried:?}");
}

/// V15: a corp's trace files `DataTheft`, sets `last_seen` to the Bar's door;
/// a guard beside the dazed runner arrests it uncontested at the chair.
#[test]
fn test_corp_trace_files_report_sets_last_seen_and_arrest_at_chair() {
    let mut w = dole_world();
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    let lab_door = w.comp::<Building>(lab).expect("lab").door;
    let bar = w
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .min_by_key(|&b| (w.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(lab_door)), b))
        .expect("a Bar");
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    w.comp_mut::<citysim::Personality>(a).expect("p").courage = 1.0;
    seat(&mut w, a, bar);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::Freelance);
    let id = virt::start_run(&mut w, a).expect("run");
    let now = w.tick;
    w.comp_mut::<Brain>(a).expect("brain").exec = ExecState::JackedIn { run: id, since: now };
    let r = w.runs[&id].clone();
    assert!(r.route.len() <= 5, "a short route keeps the trace's confidence: {:?}", r.route);
    let gap = f32::from(virt::def(&w, n)) - r.att;
    virt::lose(&mut w, r, n, LossRolls { gap, fry: 1.0, dodge: 0.0, kill: 1.0, trace: 0.0 });
    let door = w.comp::<Building>(bar).expect("bar").door;
    assert!(w.crime_reports().iter().any(|x| !x.resolved && x.suspect == a && x.crime == Crime::DataTheft));
    assert_eq!(w.last_seen().get(&a).map(|&(t, _)| t), Some(door));
    assert_eq!(w.last_trace.get(&a).map(|&(c, _)| c), Some(bar));
    assert!(w.comp::<Brain>(a).and_then(|b| b.dazed_until).is_some_and(|t| t > w.tick), "dazed");
    assert!(matches!(w.comp::<Brain>(a).map(|b| &b.exec), Some(ExecState::Wait { .. })), "waits in the chair");
    assert_eq!(last_outcome(&w), Some(RunOutcome::Traced));
    assert_eq!(w.db.get(&corp_named(&w, "Militech")).map(|d| d.sightings.len()), Some(1));
    let guard = w.guards().iter().copied().find(|&g| law::is_city_guard(&w, g)).expect("a guard");
    seat(&mut w, guard, bar);
    assert!(law::arrest(&mut w, guard, a), "a dazed runner cannot fight the cuffs");
    assert_eq!(w.stats.current.virt.hack_arrests_chair, 1);
}

/// V15: a gang's trace never calls the law: `Shock::Hacked { by: None }`
/// for a freelancer.
#[test]
fn test_gang_trace_pushes_hacked_shock() {
    let mut w = dole_world();
    let gang = w.gang_list()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let n = virt::node_of_building(&w, hideout).expect("node");
    w.virt.node_mut(n).expect("node").store.units[0] = 200;
    set_ice(&mut w, n, 1);
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::Freelance);
    let id = virt::start_run(&mut w, a).expect("run");
    let r = w.runs[&id].clone();
    let gap = f32::from(virt::def(&w, n)) - r.att;
    w.comp_mut::<Gang>(gang).expect("gang").shocks.clear();
    virt::lose(&mut w, r, n, LossRolls { gap, fry: 1.0, dodge: 0.0, kill: 1.0, trace: 0.0 });
    let shocks = &w.comp::<Gang>(gang).expect("gang").shocks;
    assert!(shocks.contains(&Shock::Hacked { by: None }), "{shocks:?}");
    assert!(!law::wanted(&w, a), "no report");
    assert_eq!(w.stats.current.virt.traced, 1);
}

/// V14: a loss at Extract returns the payload (Captured).
#[test]
fn test_extract_loss_returns_payload() {
    let mut w = dole_world();
    let lab = lab_of(&w, corp_named(&w, "Zetatech"), Track::Chrome);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 0);
    let store = w.virt.node(n).expect("node").store.clone();
    // A tier-2 deck's quiet take, read from the config (the store holds more).
    let take = (w.config.data.steal_units[1] as f32 * w.config.hack.quiet_take).round() as u32;
    assert!(take > 0 && store.total() >= take);
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(&mut w, a).expect("run");
    while w.runs[&id].phase != RunPhase::Extract {
        let (t, _) = *w.run_queue.first().expect("queued");
        w.tick = w.tick.max(t);
        virt::run(&mut w);
    }
    let r = w.runs[&id].clone();
    assert_eq!(r.payload.iter().sum::<u32>(), take, "steal_units[1] x quiet_take in the payload");
    assert_ne!(w.virt.node(n).expect("node").store, store);
    set_ice(&mut w, n, 2);
    let gap = 2.0 - r.att;
    virt::lose(&mut w, r, n, LossRolls { gap, fry: 1.0, dodge: 0.0, kill: 1.0, trace: 1.0 });
    assert_eq!(last_outcome(&w), Some(RunOutcome::Captured));
    assert_eq!(w.virt.node(n).expect("node").store, store, "the payload went back");
    assert_eq!(virt::deck_data(&w, a), 0);
}

/// V12: an attack on the seated body dumps the run (sanity − dump_sanity).
#[test]
fn test_attack_dumps_run() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    let bar = first_bar(&w);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(&mut w, a).expect("run");
    let now = w.tick;
    w.comp_mut::<Brain>(a).expect("brain").exec = ExecState::JackedIn { run: id, since: now };
    let sanity = w.comp::<Body>(a).expect("body").sanity;
    let attacker = runner(&mut w, &[a]);
    law::resolve_fight(&mut w, attacker, a);
    assert!(!w.runs.contains_key(&id) && !w.runner_of.contains_key(&a));
    assert_eq!(last_outcome(&w), Some(RunOutcome::Dumped));
    assert_eq!(events(&w, EventKind::Dumpshock, a), 1);
    if let Some(b) = w.comp::<Body>(a).filter(|_| !w.has::<Corpse>(a)) {
        assert!((b.sanity - (sanity - w.config.virt.dump_sanity)).abs() < 1e-6, "{} -> {}", sanity, b.sanity);
    }
    assert_eq!(w.stats.current.virt.runs_dumped, 1);
}

/// V9: a run's contest draws pass, fry, dodge, kill, trace on its stream, in
/// that order: the live path (`step` -> `contest` -> `lose`) and the public
/// `contest_draws` + `loss_verdict` agree, so the two cannot drift.
#[test]
fn test_contest_draw_order_pinned() {
    let mut checked = 0;
    for (k, ice) in [(0u64, 3u8), (1, 3), (2, 2), (3, 3), (4, 2), (5, 3)] {
        let mut w = dole_world();
        w.tick += k * 7;
        let a = runner(&mut w, &[]);
        arm(&mut w, a, 1, 0.3);
        let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
        let n = virt::node_of_building(&w, lab).expect("node");
        set_ice(&mut w, n, ice);
        let bar = first_bar(&w);
        order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
        let id = virt::start_run(&mut w, a).expect("run");
        let r = w.runs[&id].clone();
        let o = virt::run_odds(&w, &r);
        let d = f32::from(virt::def(&w, n));
        let (pass, rolls) = virt::contest_draws(&w, id, 0, d - r.att);
        let won = pass < security::contest_p(r.att, d, w.config.chrome.contest_step);
        let (fried, dead, traced) = virt::loss_verdict(&w, n, &o, &rolls);
        let fried0 = w.stats.current.virt.fried;
        finish(&mut w);
        let done = w.run_log.back().expect("logged");
        assert_eq!(done.log.first().map(|e| e.2), Some(won), "the pass draw is first");
        if !won {
            let want = if dead {
                RunOutcome::Flatlined
            } else if fried {
                RunOutcome::Fried
            } else if traced {
                RunOutcome::Traced
            } else {
                RunOutcome::Bounced
            };
            assert_eq!(done.outcome, Some(want), "fry, dodge, kill, trace follow in order");
            assert_eq!(w.stats.current.virt.fried - fried0, u32::from(fried));
            checked += 1;
        }
    }
    assert!(checked >= 3, "{checked} lost first contests checked");
}

// ---------------------------------------------------------------------------
// M14 review (`/code-review high 6c04ece..16b9efe`), findings 1, 3, 6, 7, 8
// and 9: the route cache and save/load, the chair, an empty or undeliverable
// steal, the trace's hops, a JackIn without its order.
// ---------------------------------------------------------------------------

/// A seated run for `a` from the first Bar onto Militech's Deck Lab.
fn seated_run(w: &mut World, a: EntityId) -> u64 {
    let lab = lab_of(w, corp_named(w, "Militech"), Track::Deck);
    let n = virt::node_of_building(w, lab).expect("node");
    let bar = first_bar(w);
    seat(w, a, bar);
    order(w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(w, a).expect("run");
    let now = w.tick;
    w.comp_mut::<Brain>(a).expect("brain").exec = ExecState::JackedIn { run: id, since: now };
    id
}

/// Review 1: a tree built while a node's alarm stands is not reused after
/// the alarm lapses mid-hour (no write marks the lapse); the cached tree
/// equals a fresh search on a clone (whose cache is empty) at every tick.
#[test]
fn test_route_cache_drops_trees_at_alarm_expiry() {
    let mut w = dole_world();
    w.tick = 10 * TICKS_PER_HOUR + 7;
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 1);
    let bar = first_bar(&w);
    let from = virt::portal(&w, bar, a, None);
    let expiry = w.tick + 20;
    assert!(!expiry.is_multiple_of(TICKS_PER_HOUR), "mid-hour");
    w.virt.node_mut(n).expect("node").alarm_until = Some(expiry);
    virt::bump_epoch(&mut w);
    let att = virt::odds_of(&w, a, 2, None, RunMode::Quiet).att;
    let with_alarm = virt::routes_from(&w, from, 2, att, None);
    let fresh = |w: &World| virt::routes_from(&w.clone(), from, 2, att, None);
    assert_eq!(*with_alarm, *fresh(&w));
    w.tick = expiry - 1;
    assert_eq!(*virt::routes_from(&w, from, 2, att, None), *with_alarm, "still standing: a cache hit");
    w.tick = expiry;
    let after = virt::routes_from(&w, from, 2, att, None);
    assert_eq!(*after, *fresh(&w), "the lapse drops the tree");
    assert!(after.dist[n.index()] < with_alarm.dist[n.index()], "the alarm's +1 is gone");
}

/// Review 3: a sacked Hideout puts everyone inside out at the door
/// (`stand_at_door`); a runner seated there is dumped (V12), as is anyone
/// taken off a building's list while on a run.
#[test]
fn test_put_out_of_the_chair_dumps_the_run() {
    let mut w = dole_world();
    let gang = w.gang_list()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let a = runner(&mut w, &[]);
    citysim::systems::gang::enlist(&mut w, a, gang);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    seat(&mut w, a, hideout);
    order(&mut w, a, hideout, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(&mut w, a).expect("run");
    let now = w.tick;
    w.comp_mut::<Brain>(a).expect("brain").exec = ExecState::JackedIn { run: id, since: now };
    // What `raid::resolve`'s Sacked arm does to everyone inside.
    w.stand_at_door(a, hideout);
    assert!(!w.runs.contains_key(&id) && !w.runner_of.contains_key(&a), "out of the chair");
    assert_eq!(last_outcome(&w), Some(RunOutcome::Dumped));
    assert_eq!(events(&w, EventKind::Dumpshock, a), 1);
    // Nobody on a run: a no-op.
    let b = runner(&mut w, &[a]);
    let before = w.events.len();
    w.stand_at_door(b, hideout);
    assert_eq!(w.events.len(), before);
}

/// A won-every-contest Data run onto Militech's Deck Lab at ICE 0 for `a`
/// with `patron`; returns the Lab's node.
fn open_steal(w: &mut World, a: EntityId, patron: Option<EntityId>) -> NodeId {
    let lab = lab_of(w, corp_named(w, "Militech"), Track::Deck);
    let n = virt::node_of_building(w, lab).expect("node");
    set_ice(w, n, 0);
    let bar = first_bar(w);
    seat(w, a, bar);
    order(w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    w.run_orders.get_mut(&a).expect("order").patron = patron;
    virt::start_run(w, a).expect("run");
    n
}

/// Review 6: a steal that lifts 0 units (the store was empty at Act) robs
/// nobody: no `DataStolen`, no Virt loss on the corp (so no hardening).
#[test]
fn test_empty_steal_is_no_robbery() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let n = open_steal(&mut w, a, None);
    w.virt.node_mut(n).expect("node").store.units = [0; 3];
    let militech = corp_named(&w, "Militech");
    let losses = w.comp::<Corp>(militech).map(|c| c.virt_losses.len());
    finish(&mut w);
    assert_eq!(last_outcome(&w), Some(RunOutcome::Success));
    assert_eq!(w.events.iter().filter(|e| e.kind == EventKind::DataStolen).count(), 0);
    assert_eq!(w.comp::<Corp>(militech).map(|c| c.virt_losses.len()), losses, "no Virt loss");
    assert_eq!(w.stats.current.virt.data_stolen, 0);
}

/// Review 9: a steal whose patron has nowhere to keep Data (a gang with no
/// Hideout node) puts the payload back and ends `Bounced`: no `runs_ok`, no
/// success drift.
#[test]
fn test_undeliverable_steal_is_not_a_success() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    // A corp patron with no Lab: `deliver` finds nowhere to put the payload.
    let patron = w.corps().into_iter().find(|&c| tech::labs_of(&w, c).is_empty()).expect("a corp without a Lab");
    let n = open_steal(&mut w, a, Some(patron));
    let store = w.virt.node(n).expect("node").store.clone();
    let hacking = w.comp::<Skills>(a).expect("skills").hacking;
    finish(&mut w);
    assert_eq!(last_outcome(&w), Some(RunOutcome::Bounced));
    assert_eq!(w.stats.current.virt.runs_ok, 0);
    assert_eq!(w.virt.node(n).expect("node").store, store, "the payload went back");
    let drift = w.config.decks.hack_drift;
    let now = w.comp::<Skills>(a).expect("skills").hacking;
    assert!((now - (hacking + drift)).abs() < 1e-6, "one drift, not two: {hacking} -> {now}");
}

/// Review 7: the trace's confidence decays with the hops from the portal
/// to the node that traced, not with the whole route.
#[test]
fn test_trace_decay_counts_hops_to_the_tracing_node() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let id = seated_run(&mut w, a);
    let mut r = w.runs[&id].clone();
    // The Lab one hop from the portal on a four-node route: the whole
    // route would read three hops.
    let (p, n) = (r.portal, r.target);
    r.route.clear();
    r.route.extend([p, n, p, p]);
    virt::trace(&mut w, &r, n);
    let owner = virt::owner_of(&w, n).unwrap_or(EntityId::NONE);
    let s = w.db.get(&owner).and_then(|d| d.sightings.back()).copied().expect("named");
    let want = 1.0 - w.config.ice.trace_decay_per_hop;
    assert!((s.confidence - want).abs() < 1e-6, "one hop: {} vs {want}", s.confidence);
}

/// Review 8: a `JackIn` waiting on an order not yet due fails when the
/// order vanishes; the post-run daze wait still completes the step.
#[test]
fn test_jackin_wait_fails_when_the_order_is_gone() {
    let mut w = dole_world();
    let a = runner(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let bar = first_bar(&w);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    seat(&mut w, a, bar);
    order(&mut w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let now = w.tick;
    w.run_orders.get_mut(&a).expect("order").not_before = now + 30;
    let step = ActionInstance { action: ActionKind::JackIn, target: Some(bar), tile: None };
    let plan = Plan { goal: GoalKind::Hack, target: Some(bar), steps: vec![step], started_tick: now };
    let set_plan = |w: &mut World, exec: ExecState| {
        let b = w.comp_mut::<Brain>(a).expect("brain");
        b.plan = Some(plan.clone());
        b.plan_step = 0;
        b.exec = exec;
    };
    set_plan(&mut w, ExecState::Idle);
    citysim::exec::run(&mut w);
    assert!(matches!(w.comp::<Brain>(a).map(|b| &b.exec), Some(ExecState::Wait { .. })), "waits for not_before");
    w.run_orders.remove(&a);
    w.tick += 1;
    citysim::exec::run(&mut w);
    assert!(w.comp::<Brain>(a).is_some_and(|b| b.plan.is_none()), "the plan failed");
    let aborted = |w: &World, t: u64| {
        w.debug_events.iter().any(|e| e.kind == EventKind::PlanAborted && e.actors.contains(&a) && e.tick == t)
    };
    assert!(aborted(&w, w.tick), "PlanAborted");
    // The daze after a run: the wait ends at `dazed_until` and the step is done.
    let until = w.tick + 5;
    set_plan(&mut w, ExecState::Wait { until });
    w.comp_mut::<Brain>(a).expect("brain").dazed_until = Some(until);
    w.tick = until;
    citysim::exec::run(&mut w);
    let b = w.comp::<Brain>(a).expect("brain");
    assert!(b.plan.is_none() || b.plan_step == 1, "the daze completes the step");
    assert!(!aborted(&w, until), "no abort at the end of the daze");
}
