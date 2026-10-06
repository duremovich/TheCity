//! M14 phase 2: decks and runs (plan 2.8). A run is a game abstraction: a
//! fictional runner's token moves along abstract nodes, each guarded node a
//! seeded dice contest (deck tier plus skill against the node's ICE tier),
//! resolved as `law::resolve_fight` resolves a brawl.

use citysim::systems::virt::{LossRolls, Odds};
use citysim::systems::{assets, law, lod, security, vehicles, virt};
use citysim::virt::{NodeId, NodeKind, Purpose, RunMode, RunOrder, RunOutcome, RunPhase, RunWhy, Track};
use citysim::{
    AssetKind, Body, Brain, Building, BuildingKind, Config, Corp, Corpse, Crime, DeathCause, EntityId, EventKind,
    ExecState, Gang, GangMember, Kit, Lod, Shock, Skills, World,
};
use rand::Rng;

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
    let w = world();
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
    let w = world();
    let tree = virt::routes_from(&w, NodeId(0), 1, 1.5, None);
    let mut ledgers = 0;
    for (i, n) in w.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        match n.kind {
            NodeKind::Ledger(_) => {
                ledgers += 1;
                assert!(tree.dist[i].is_infinite(), "Ledger {i} reached at tier 1");
            }
            _ => assert!(tree.dist[i].is_finite(), "node {i} unreached at tier 1"),
        }
    }
    assert!(ledgers >= 9);
    let t2 = virt::routes_from(&w, NodeId(0), 2, 2.5, None);
    let reached = w.virt.nodes.iter().enumerate().filter(|(i, n)| n.alive && t2.dist[*i].is_finite()).count();
    assert_eq!(reached, w.virt.alive_count(), "a tier-2 deck reaches every node");
}

/// V7: a corp's weakest building is its treasury's front door.
#[test]
fn test_ledger_route_enters_through_weakest_building() {
    let mut w = world();
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
    let mut w = world();
    let gang = w.gang_list()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let n = virt::node_of_building(&w, hideout).expect("node");
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
    assert_eq!(w.comp::<citysim::Asset>(deck).map(|x| x.data[Track::Deck.index()]), Some(30), "60 x quiet 0.5");
}

/// V6: one search per scorer, whatever the number of targets; an ICE write
/// costs one more.
#[test]
fn test_route_cache_one_search_per_scorer() {
    let mut w = world();
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
fn test_run_pops_only_when_due_and_is_deterministic() {
    let mut w = world();
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
    let mut twin = w.clone();
    finish(&mut w);
    finish(&mut twin);
    let (x, y) = (w.run_log.back().expect("run"), twin.run_log.back().expect("run"));
    assert!(!x.log.is_empty(), "the Lab was contested");
    assert_eq!(x.log, y.log);
    assert_eq!(x.outcome, y.outcome);
    println!("log {:?} outcome {:?}", x.log, x.outcome);
}

/// V9: a run never draws on the world stream.
#[test]
fn test_run_rolls_do_not_touch_world_stream() {
    let mut w = world();
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
    let mut w = world();
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
    let mut cfg = Config::load();
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
    let mut w = world();
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
    let mut w = world();
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
    assert_eq!(w.last_seen.get(&a).map(|&(t, _)| t), Some(door));
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
    let mut w = world();
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
    let mut w = world();
    let lab = lab_of(&w, corp_named(&w, "Zetatech"), Track::Chrome);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 0);
    let store = w.virt.node(n).expect("node").store.clone();
    assert!(store.total() > 0);
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
    assert_eq!(r.payload.iter().sum::<u32>(), 75, "150 x quiet 0.5 in the payload");
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
    let mut w = world();
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
        let mut w = world();
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
