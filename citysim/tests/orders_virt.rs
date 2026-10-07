//! M14 phase 3: orders and targets (plan 3.6). Every run here is a game
//! abstraction: a fictional runner's token on abstract nodes, each guarded
//! node one seeded dice contest (deck tier plus skill against ICE tier), as
//! `law::resolve_fight` resolves a brawl. Gangs and corps now issue those
//! attempts as orders from their utility brains, as they issue raids.

use citysim::systems::virt::LossRolls;
use citysim::systems::{assets, demography, faction, law, lod, raid, robots, virt};
use citysim::virt::{HackEffect, NodeId, Purpose, RunMode, RunOrder, RunOutcome, RunWhy, Track};
use citysim::{
    AssetKind, AssetLoc, Brain, Building, BuildingKind, Config, Corp, Crime, EntityId, Gang, GangMember, Kit, Lod,
    Order, Personality, Role, Shock, Skills, World,
};

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

/// An adult with a Brain, no gang, no deck, no chrome, not a guard; Coarse.
fn adult(w: &mut World, skip: &[EntityId]) -> EntityId {
    let a = w
        .citizens()
        .into_iter()
        .find(|&a| {
            w.has::<Brain>(a)
                && demography::is_adult(w, a)
                && !w.has::<GangMember>(a)
                && !skip.contains(&a)
                && w.comp::<Kit>(a).is_some_and(|k| k.deck.is_none() && !k.chrome)
                && !law::is_guard(w, a)
        })
        .expect("an adult");
    lod::set_lod(w, a, Lod::Coarse);
    a
}

fn arm(w: &mut World, a: EntityId, tier: u8, hacking: f32) {
    assets::grant(w, a, AssetKind::Deck, tier).expect("deck");
    w.comp_mut::<Skills>(a).expect("skills").hacking = hacking;
    assert_eq!(w.comp::<Kit>(a).map(|k| k.deck_tier), Some(tier));
}

/// A gang member armed with a deck.
fn gang_runner(w: &mut World, gang: EntityId, tier: u8, hacking: f32) -> EntityId {
    let a = adult(w, &[]);
    citysim::systems::gang::enlist(w, a, gang);
    arm(w, a, tier, hacking);
    a
}

fn set_ice(w: &mut World, n: NodeId, ice: u8) {
    virt::profile_mut(w, n).expect("profile").ice = ice;
    virt::bump_epoch(w);
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

/// A new asset of `kind` owned by `owner` and posted at `b` (granted to `via` first).
fn post(w: &mut World, via: EntityId, kind: AssetKind, tier: u8, owner: EntityId, b: EntityId) -> EntityId {
    let a = assets::grant(w, via, kind, tier).expect("grant");
    assets::set_owner(w, a, Some(owner));
    assets::set_keeper(w, a, None);
    assets::set_loc(w, a, AssetLoc::Posted(b));
    assets::rekit(w, via);
    a
}

/// A corp building of `kind` (lowest id).
fn corp_building(w: &World, kind: BuildingKind) -> (EntityId, EntityId) {
    let b = w
        .buildings_of_kind(kind)
        .iter()
        .copied()
        .find(|&b| w.corp_of_building(b).is_some() && virt::node_of_building(w, b).is_some())
        .expect("a corp building");
    (b, w.corp_of_building(b).expect("corp"))
}

/// Plan 3.6: a gang with a T2-deck runner at hacking 0.6 and a rival Lab
/// holding 600 at ICE 1: VirtRaid is gated open and scores above its 0.3
/// flat; under the order, at most `virt_runners` orders go out, chair the
/// Hideout, patron the gang.
#[test]
fn test_gang_virtraid_scores_and_orders_runners() {
    let mut cfg = Config::load();
    cfg.gangs.order_flat.virt_raid = 0.3;
    let mut w = World::new(42, cfg);
    let gang = w.gang_list()[0];
    let lab = lab_of(&w, corp_named(&w, "Zetatech"), Track::Chrome);
    let n = virt::node_of_building(&w, lab).expect("node");
    w.virt.node_mut(n).expect("node").store.units = [600, 0, 0];
    set_ice(&mut w, n, 1);
    let r1 = gang_runner(&mut w, gang, 2, 0.6);
    let _r2 = gang_runner(&mut w, gang, 2, 0.5);
    let _r3 = gang_runner(&mut w, gang, 2, 0.4);
    let i = faction::gather_inputs(&w, gang).expect("inputs");
    assert_eq!(i.runner, Some(r1), "the best hacker runs");
    assert!(i.virt_p >= w.config.virt.min_route_p && i.virt_ev > 0.0, "{i:?}");
    let scores = faction::score_orders(&i, &w.config.gangs);
    let vr = scores.iter().find(|s| s.order == Order::VirtRaid).expect("VirtRaid scored");
    assert!(vr.score > 0.3, "{}", vr.score);
    w.comp_mut::<Gang>(gang).expect("gang").order = Order::VirtRaid;
    virt::gang_daily(&mut w, gang);
    let hideout = w.hideout_of(gang).expect("hideout");
    let orders: Vec<&RunOrder> = w.run_orders.values().filter(|o| o.patron == Some(gang)).collect();
    assert!(!orders.is_empty() && orders.len() <= w.config.hack.virt_runners as usize, "{}", orders.len());
    assert!(orders.iter().all(|o| o.chair == hideout && o.why == RunWhy::GangOrder));
    assert!(w.run_orders.contains_key(&r1));
}

/// Plan 3.6 (V32, V33): a corp-raid target with ICE 1, a robot and a
/// contract gets a `Robot` order `door_lead_hours` before the muster; a
/// won Robot run turns it; a won Door run opens the doors until `raid_at +
/// door_hours`.
#[test]
fn test_raid_prelude_opens_door_in_window() {
    let mut w = world();
    let gang = w.gang_list()[0];
    let (b, corp) = corp_building(&w, BuildingKind::Farm);
    let n = virt::node_of_building(&w, b).expect("node");
    set_ice(&mut w, n, 1);
    let via = adult(&mut w, &[]);
    let robot = post(&mut w, via, AssetKind::Robot, 1, corp, b);
    let security = w.corps().into_iter().find(|&c| c != corp).expect("another corp");
    w.comp_mut::<Building>(b).expect("b").secured_by = Some(security);
    let runner = gang_runner(&mut w, gang, 2, 0.6);
    let raid_at = w.tick + 10 * 60;
    w.comp_mut::<Gang>(gang).expect("gang").raid_at = Some(raid_at);
    virt::raid_prelude(&mut w, gang, b, raid_at);
    let o = w.run_orders.get(&runner).cloned().expect("a prelude order");
    assert_eq!(o.purpose, Purpose::Robot(robot));
    assert_eq!(o.why, RunWhy::Prelude);
    assert_eq!(o.not_before, raid_at - 60 * u64::from(w.config.hack.door_lead_hours));
    // Forced success: the node unguarded, so no contest is rolled.
    set_ice(&mut w, n, 0);
    let hideout = w.hideout_of(gang).expect("hideout");
    seat(&mut w, runner, hideout);
    w.tick = o.not_before;
    virt::start_run(&mut w, runner).expect("run");
    finish(&mut w);
    assert_eq!(w.run_log.back().and_then(|r| r.outcome), Some(RunOutcome::Success));
    assert!(w.comp::<citysim::Asset>(robot).and_then(|x| x.turned).is_some_and(|(s, _)| s == gang));
    assert!(robots::powered_robot(&w, b).is_none(), "turned: no longer the owner's");
    // A Door run.
    let now = w.tick;
    w.run_orders.insert(
        runner,
        RunOrder {
            patron: Some(gang),
            purpose: Purpose::Door,
            target: n,
            chair: hideout,
            not_before: now,
            expires: raid_at,
            why: RunWhy::Prelude,
            mode: RunMode::Quiet,
        },
    );
    virt::start_run(&mut w, runner).expect("door run");
    finish(&mut w);
    let hacked = w.comp::<Building>(b).and_then(|x| x.hacked);
    let until = raid_at + 60 * u64::from(w.config.hack.door_hours);
    assert_eq!(hacked, Some((HackEffect::DoorOpen, until)));
    assert!(virt::sensors_off(&w, b));
}

/// Plan 3.6 (V32): robot 0.75, summoned guards 0.5 and 0.6, staff 0.3;
/// total 2.15, cap 1.075: the robot goes, the 0.5 guard stays (0.75 + 0.5
/// = 1.25 > 1.075), and the brawl keeps defenders.
#[test]
fn test_door_open_strips_at_most_half_the_defence() {
    let w = world();
    let ids = w.citizens();
    let (robot, g1, g2, staff) = (ids[0], ids[1], ids[2], ids[3]);
    let total = 0.75 + 0.5 + 0.6 + 0.3;
    let cands = [(robot, 0.75), (g1, 0.5), (g2, 0.6)];
    let removed = raid::door_strip(total, &cands, w.config.ice.door_strip_cap);
    assert_eq!(removed, vec![robot]);
    let left: Vec<EntityId> = [robot, g1, g2, staff].into_iter().filter(|x| !removed.contains(x)).collect();
    assert_eq!(left, vec![g1, g2, staff]);
    let removed_strength: f32 = 0.75;
    assert!(removed_strength <= w.config.ice.door_strip_cap * total);
}

/// Plan 3.6 (V33): a turned robot stands first on the raiders' side at its
/// building and is no longer the owner's defender.
#[test]
fn test_turned_robot_fights_for_patron() {
    let mut w = world();
    let gang = w.gang_list()[0];
    let (b, corp) = corp_building(&w, BuildingKind::Farm);
    let via = adult(&mut w, &[]);
    let robot = post(&mut w, via, AssetKind::Robot, 2, corp, b);
    assert_eq!(robots::powered_robot(&w, b), Some(robot));
    let until = w.tick + 600;
    w.comp_mut::<citysim::Asset>(robot).expect("robot").turned = Some((gang, until));
    assert!(robots::defenders_at(&w, b).is_empty());
    let member = gang_runner(&mut w, gang, 1, 0.3);
    let mut raiders = vec![member];
    raid::turned_join(&w, b, gang, &mut raiders);
    assert_eq!(raiders, vec![robot, member]);
    let other = w.gang_list()[1];
    let mut theirs = vec![member];
    raid::turned_join(&w, b, other, &mut theirs);
    assert_eq!(theirs, vec![member], "it fights for its patron only");
}

/// Plan 3.6 (V28, V33): a T3 camera against a tier-1 thief: a sighting in
/// the owner's database and a report with no witness; under Blind nothing.
#[test]
fn test_blind_skips_camera_and_camera_writes_sighting() {
    let mut w = world();
    let (b, corp) = corp_building(&w, BuildingKind::Market);
    let via = adult(&mut w, &[]);
    post(&mut w, via, AssetKind::Camera, 3, corp, b);
    assert!(virt::camera_at(&w, b).is_some());
    let thief = adult(&mut w, &[via]);
    w.comp_mut::<Skills>(thief).expect("skills").stealth = 0.0;
    seat(&mut w, thief, b);
    // Blind first: 40 thefts, no sighting.
    let until = w.tick + 600;
    w.comp_mut::<Building>(b).expect("b").hacked = Some((HackEffect::Blind, until));
    for _ in 0..40 {
        virt::camera_sense(&mut w, thief, Crime::Theft, b);
    }
    assert!(w.db.get(&corp).is_none_or(|d| d.sightings.is_empty()), "Blind: no sighting");
    assert!(!law::wanted(&w, thief));
    w.comp_mut::<Building>(b).expect("b").hacked = None;
    for _ in 0..40 {
        virt::camera_sense(&mut w, thief, Crime::Theft, b);
        if w.db.get(&corp).is_some_and(|d| !d.sightings.is_empty()) {
            break;
        }
    }
    let s = w.db.get(&corp).and_then(|d| d.sightings.back().copied()).expect("a sighting");
    assert_eq!(s.who, thief);
    assert!(s.confidence > 0.9, "{}", s.confidence);
    assert!(law::wanted(&w, thief), "a report with no witness");
    assert!(w.last_seen.contains_key(&thief), "the law reads the sighting");
}

/// Plan 3.6 (V27): two Labs short of their ICE target, a Virt loss on the
/// second: it is raised first.
#[test]
fn test_secure_raises_largest_gap_newest_virt_loss_first() {
    let mut cfg = Config::load();
    cfg.corps.secure_per_day = 1;
    let mut w = World::new(42, cfg);
    let z = corp_named(&w, "Zetatech");
    let (a, b) = (lab_of(&w, z, Track::Chrome), lab_of(&w, z, Track::Industry));
    let (na, nb) = (virt::node_of_building(&w, a).expect("a"), virt::node_of_building(&w, b).expect("b"));
    let (lo, hi) = if na < nb { (na, nb) } else { (nb, na) };
    w.virt.node_mut(lo).expect("lo").store.units = [2000, 2000, 2000];
    w.virt.node_mut(hi).expect("hi").store.units = [2000, 0, 0];
    set_ice(&mut w, lo, 0);
    set_ice(&mut w, hi, 0);
    w.comp_mut::<Corp>(z).expect("z").treasury = 50_000;
    let now = w.tick;
    w.comp_mut::<Corp>(z).expect("z").virt_losses.push_back((now, hi));
    virt::secure_ice(&mut w, z);
    assert_eq!(virt::profile(&w, hi).map(|p| p.ice), Some(1), "the robbed Lab first");
    assert_eq!(virt::profile(&w, lo).map(|p| p.ice), Some(0), "one a day");
}

/// Plan 3.6 (V27): ICE 3 on a node with `ice_target` 1 is lowered a tier.
#[test]
fn test_hunker_lowers_excess_ice() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    // Every other node of the corp at its target: no excess elsewhere.
    let nodes: Vec<NodeId> = (0..w.virt.nodes.len() as u16)
        .map(NodeId)
        .filter(|&n| w.virt.node(n).is_some_and(|x| x.alive && x.owner == Some(z)))
        .collect();
    for &n in &nodes {
        let t = virt::ice_target(&w, n);
        if let Some(p) = virt::profile_mut(&mut w, n) {
            p.ice = t;
        }
    }
    let lab = lab_of(&w, z, Track::Chrome);
    let n = virt::node_of_building(&w, lab).expect("node");
    w.virt.node_mut(n).expect("node").store.units = [100, 0, 0];
    assert_eq!(virt::ice_target(&w, n), 1);
    set_ice(&mut w, n, 3);
    virt::hunker_ice(&mut w, z);
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(2));
}

/// A day on: the Labs have hired their Researchers.
fn hired_world() -> World {
    let mut w = world();
    w.run_ticks(1440);
    w
}

/// Plan 3.6 (V31): Militech (Deck 2) against Arasaka (Deck 3), a fleet deck
/// at its Lab: lawfulness 0.2 orders a wipe, 0.8 a steal.
#[test]
fn test_corp_virtraid_wipes_when_behind_and_lawless() {
    let mut w = hired_world();
    let m = corp_named(&w, "Militech");
    let lab = lab_of(&w, m, Track::Deck);
    let staffed = w
        .workers(Role::Researcher)
        .iter()
        .any(|&r| w.comp::<citysim::Job>(r).and_then(|j| j.employer) == Some(lab) && !w.has::<citysim::Sentence>(r));
    if !staffed {
        let a = adult(&mut w, &[]);
        demography::hire(&mut w, a, lab, Role::Researcher);
    }
    let via = adult(&mut w, &[]);
    post(&mut w, via, AssetKind::Deck, 1, m, lab);
    w.comp_mut::<Corp>(m).expect("m").tech.focus = Track::Deck;
    let ar = corp_named(&w, "Arasaka");
    let alab = lab_of(&w, ar, Track::Deck);
    let an = virt::node_of_building(&w, alab).expect("node");
    w.virt.node_mut(an).expect("node").store.units = [0, 500, 0];
    let exec = w.comp::<Corp>(m).and_then(|c| c.exec).expect("exec");
    w.comp_mut::<Personality>(exec).expect("p").lawfulness = 0.2;
    let run = virt::corp_run(&w, m).expect("a run");
    assert_eq!(run.purpose, Purpose::Data { wipe: true });
    assert_eq!(run.target, an);
    w.comp_mut::<Personality>(exec).expect("p").lawfulness = 0.8;
    let run = virt::corp_run(&w, m).expect("a run");
    assert_eq!(run.purpose, Purpose::Data { wipe: false });
    virt::corp_virt_raid(&mut w, m);
    let o = w.run_orders.get(&run.runner).expect("an order");
    assert_eq!((o.chair, o.why, o.patron), (lab, RunWhy::CorpOrder, Some(m)));
    assert!(run.deck.is_some() && w.comp::<Kit>(run.runner).is_some_and(|k| k.deck == run.deck), "lent");
}

/// Plan 3.6 (V38): a Statistical adult with a deck on its day, the score
/// threshold at 0: promoted to Coarse with a `RunOrder { why: Stat }`.
#[test]
fn test_stat_pass_promotes_runner_to_coarse() {
    let mut cfg = Config::load();
    cfg.hack.stat_hack_min = 0.0;
    let mut w = World::new(42, cfg);
    let a = adult(&mut w, &[]);
    arm(&mut w, a, 2, 0.6);
    lod::set_lod(&mut w, a, Lod::Statistical);
    assert_eq!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Statistical));
    let day = (7 - u64::from(a.index) % 7) % 7 + 7;
    w.tick = day * 1440;
    virt::stat_pass(&mut w);
    assert_eq!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Coarse));
    assert_eq!(w.run_orders.get(&a).map(|o| o.why), Some(RunWhy::Stat));
}

/// Plan 3.6 (V15, V34): a gang runner traced by a rival gang's Hideout:
/// `Shock::Hacked { by: Some(gang) }`, a grudge, and the VirtRaid input.
#[test]
fn test_hacked_shock_targets_runners_gang() {
    let mut w = world();
    let (ga, gb) = (w.gang_list()[0], w.gang_list()[1]);
    let hideout = w.hideout_of(gb).expect("hideout");
    let n = virt::node_of_building(&w, hideout).expect("node");
    w.virt.node_mut(n).expect("node").store.units = [200, 0, 0];
    set_ice(&mut w, n, 1);
    let a = gang_runner(&mut w, ga, 2, 0.5);
    let chair = w.hideout_of(ga).expect("own hideout");
    let now = w.tick;
    w.run_orders.insert(
        a,
        RunOrder {
            patron: None,
            purpose: Purpose::Data { wipe: false },
            target: n,
            chair,
            not_before: now,
            expires: now + 1440,
            why: RunWhy::Freelance,
            mode: RunMode::Quiet,
        },
    );
    let id = virt::start_run(&mut w, a).expect("run");
    let r = w.runs[&id].clone();
    let gap = f32::from(virt::def(&w, n)) - r.att;
    w.comp_mut::<Gang>(gb).expect("gang").shocks.clear();
    virt::lose(&mut w, r, n, LossRolls { gap, fry: 1.0, dodge: 0.0, kill: 1.0, trace: 0.0 });
    let g = w.comp::<Gang>(gb).expect("gang");
    let shock = Shock::Hacked { by: Some(ga) };
    assert!(g.shocks.contains(&shock), "{:?}", g.shocks);
    assert!(shock.is_grudge());
    assert_eq!(g.hacked_by.map(|(by, _)| by), Some(ga));
    assert!(faction::gather_inputs(&w, gb).is_none_or(|i| i.hacked));
}
