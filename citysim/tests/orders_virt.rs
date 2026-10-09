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
    assert!(virt::robot_sensors_off(&w, b));
    assert!(!virt::cameras_off(&w, b), "DoorOpen leaves the cameras on");
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

/// Phase 5 (the gate's robot bullet became a finding): a god `RunNow` Door
/// on a robot-guarded corp building is a Robot run, as the Raid prelude's,
/// and at ICE 0 (no contest, so no dice) its Out turns the robot for the
/// runner: `RobotTurned`, `Asset.turned`, the counter.
#[test]
fn test_god_door_run_on_robot_building_turns_the_robot() {
    use citysim::PlayerCommand;
    let mut w = world();
    let (b, corp) = corp_building(&w, BuildingKind::Farm);
    let via = adult(&mut w, &[]);
    let robot = post(&mut w, via, AssetKind::Robot, 2, corp, b);
    assert_eq!(robots::powered_robot(&w, b), Some(robot));
    let n = virt::node_of_building(&w, b).expect("node");
    set_ice(&mut w, n, 0);
    let a = adult(&mut w, &[via]);
    arm(&mut w, a, 3, 0.9);
    w.push_command(PlayerCommand::RunNow { agent: a, target: b, purpose: Purpose::Door });
    w.apply_commands();
    let o = w.run_orders.get(&a).cloned().expect("a RunNow order");
    assert_eq!((o.purpose, o.target, o.why), (Purpose::Robot(robot), n, RunWhy::God));
    seat(&mut w, a, o.chair);
    virt::start_run(&mut w, a).expect("the run starts");
    finish(&mut w);
    let r = w.run_log.back().expect("logged");
    assert_eq!(r.outcome, Some(RunOutcome::Success));
    assert_eq!(w.comp::<citysim::Asset>(robot).and_then(|x| x.turned).map(|(side, _)| side), Some(a));
    assert_eq!(w.stats.current.virt.robots_turned, 1);
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::RobotTurned));
}

/// Plan 3.6 (V28, V33): a T3 camera against a tier-1 thief: a sighting in
/// the owner's database and a report with no witness; under Blind nothing,
/// under DoorOpen the camera still contests.
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
    // DoorOpen opens the door and leaves the cameras on (plan V32/V33).
    w.comp_mut::<Building>(b).expect("b").hacked = Some((HackEffect::DoorOpen, until));
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

/// Plan 3.6 (V27): ICE 3 on a node with `ice_target` 1 is lowered a tier
/// once the corp is below its fleet reserve (and kept above it) when the
/// upkeep saved repays the rebuy; at the shipped upkeep it is kept.
#[test]
fn test_hunker_lowers_excess_ice() {
    let mut cfg = Config::load();
    // Upkeep with teeth: a shed of tier 3 repays its rebuy within 30 days.
    cfg.ice.ice_upkeep = vec![0, 0, 10, 60];
    let mut w = World::new(42, cfg);
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
    // Phase 3 fix round: a solvent corp keeps the ICE it paid for (no churn
    // when a target falls); below the fleet reserve it sheds a tier.
    w.comp_mut::<Corp>(z).expect("z").treasury = 50_000;
    virt::hunker_ice(&mut w, z);
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(3), "above the reserve: kept");
    w.comp_mut::<Corp>(z).expect("z").treasury = 0;
    // The Ledger's target follows the purse: put the others back at target.
    for &m in nodes.iter().filter(|&&m| m != n) {
        let t = virt::ice_target(&w, m);
        if let Some(p) = virt::profile_mut(&mut w, m) {
            p.ice = t;
        }
    }
    virt::hunker_ice(&mut w, z);
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(2));
    // At the shipped upkeep no shed repays its rebuy: the tier stays.
    w.config.ice.ice_upkeep = Config::load().ice.ice_upkeep;
    set_ice(&mut w, n, 3);
    virt::hunker_ice(&mut w, z);
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(3), "no paid churn");
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

/// V34: three gangs. A runner of the third gang is traced on the first
/// gang's Hideout node: the Retaliate the first gang chooses on that shock
/// fights the third gang (the traced run's owner), not `rival_of` (the
/// second); a Retaliate on a grudge that names nobody fights the rival.
#[test]
fn test_retaliate_targets_the_traced_runners_gang() {
    let mut w = world();
    w.config.gangs.order_flat.retaliate = 10.0;
    let (ga, gb) = (w.gang_list()[0], w.gang_list()[1]);
    let third_base = w.buildings_of_kind(BuildingKind::Home)[0];
    // M15 W31: The Unplugged is seeded too (no territory, never the rival here).
    let before = w.gang_list().len();
    let gc = w.spawn();
    w.insert(gc, Gang::new("Third".into(), third_base, 0));
    assert_eq!(w.gang_list().len(), before + 1);
    assert_eq!(w.rival_of(ga), Some(gb), "the rival is not the hacker");
    // Enough fit members for a Retaliate.
    for _ in 0..3 {
        let m = adult(&mut w, &[]);
        citysim::systems::gang::enlist(&mut w, m, ga);
    }
    let hideout = w.hideout_of(ga).expect("hideout");
    let n = virt::node_of_building(&w, hideout).expect("node");
    w.virt.node_mut(n).expect("node").store.units = [200, 0, 0];
    set_ice(&mut w, n, 1);
    let a = gang_runner(&mut w, gc, 2, 0.5);
    let chair = w.buildings_of_kind(BuildingKind::Bar)[0];
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
    w.comp_mut::<Gang>(ga).expect("gang").shocks.clear();
    virt::lose(&mut w, r, n, LossRolls { gap, fry: 1.0, dodge: 0.0, kill: 1.0, trace: 0.0 });
    assert!(w.comp::<Gang>(ga).expect("gang").shocks.contains(&Shock::Hacked { by: Some(gc) }));
    assert_eq!(raid::hack_grudge(&w, ga), Some(gc));
    assert!(faction::rescore(&mut w, ga, 0.0));
    let g = w.comp::<Gang>(ga).expect("gang");
    assert_eq!(g.order, Order::Retaliate, "{:?}", g.order_trace);
    assert_eq!(g.retaliate_on, Some(gc));
    assert_eq!(raid::raid_rival(&w, ga), Some(gc));
    assert_eq!(raid::gang_target(&w, ga), w.hideout_of(gc), "the march goes to the hacker");
    // A grudge naming nobody: the Retaliate fights the rival.
    {
        let g = w.comp_mut::<Gang>(ga).expect("gang");
        g.order = Order::Expand;
        g.retaliate_on = None;
        g.shocks.clear();
        g.shocks.push(Shock::Raided);
    }
    assert!(faction::rescore(&mut w, ga, 0.0));
    let g = w.comp::<Gang>(ga).expect("gang");
    assert_eq!(g.order, Order::Retaliate, "{:?}", g.order_trace);
    assert_eq!(g.retaliate_on, None);
    assert_eq!(raid::raid_rival(&w, ga), Some(gb));
}

/// Phase 3 fix round: Secure spends (ICE, cameras) need `mult × cost` in
/// the purse and keep the fleet reserve (`treasury_ref / 4`) after paying.
#[test]
fn test_secure_affords_keeps_the_fleet_reserve() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    {
        let c = w.comp_mut::<Corp>(z).expect("z");
        c.treasury_ref = 4000;
        c.treasury = 1200;
    }
    assert!(virt::secure_affords(&w, z, 150, 2.0), "1,050 left over a 1,000 reserve");
    assert!(!virt::secure_affords(&w, z, 150, 9.0), "the purse holds less than 9 x the price");
    w.comp_mut::<Corp>(z).expect("z").treasury = 1100;
    assert!(!virt::secure_affords(&w, z, 150, 2.0), "950 left: under the reserve");
}

/// Phase 3 fix round: a vehicle buy skips a nearer Garage that cannot sell
/// the kind (V22's tech gate) for a farther one that can; with the plane off
/// every Garage sells, so the nearest is picked.
#[test]
fn test_garage_selling_skips_garages_that_cannot_sell() {
    let mut w = world();
    // Zetatech's Garages lose the truck tier (Industry 1); another corp keeps
    // Industry 2, so the street (an NPC Garage) still sells trucks.
    let z = corp_named(&w, "Zetatech");
    w.comp_mut::<Corp>(z).expect("z").tech.tier[Track::Industry.index()] = 1;
    let other = w.corps().into_iter().find(|&c| c != z).expect("another corp");
    w.comp_mut::<Corp>(other).expect("corp").tech.tier[Track::Industry.index()] = 2;
    let garages: Vec<EntityId> = w.buildings_of_kind(BuildingKind::Garage).to_vec();
    let blocked = garages
        .iter()
        .copied()
        .find(|&g| !assets::can_sell(&w, g, AssetKind::Truck, 1))
        .expect("a Garage without the truck tier");
    let from = w.comp::<Building>(blocked).expect("garage").door;
    let pick = citysim::systems::vehicles::garage_selling(&w, from, AssetKind::Truck).expect("a truck seller");
    assert_ne!(pick, blocked);
    assert!(assets::can_sell(&w, pick, AssetKind::Truck, 1));
    let dist = |g: EntityId| w.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(from));
    assert!(dist(pick) > dist(blocked), "the nearer one was skipped");
}

/// Phase 3 fix round (deviation A): a Farm's value at risk is its building
/// value only (a posted asset adds nothing); a Lab's is its store.
#[test]
fn test_value_at_risk_ignores_posted_assets() {
    let mut w = world();
    let (farm, corp) = corp_building(&w, BuildingKind::Farm);
    let n = virt::node_of_building(&w, farm).expect("node");
    let before = virt::value_at_risk(&w, n);
    assert_eq!(before, citysim::systems::ownership::value(&w, BuildingKind::Farm));
    let via = adult(&mut w, &[]);
    post(&mut w, via, AssetKind::Robot, 1, corp, farm);
    assert_eq!(virt::value_at_risk(&w, n), before, "the posted robot adds nothing");
    let z = corp_named(&w, "Zetatech");
    let lab = lab_of(&w, z, Track::Chrome);
    let ln = virt::node_of_building(&w, lab).expect("lab node");
    w.virt.node_mut(ln).expect("node").store.units = [100, 0, 0];
    assert_eq!(virt::value_at_risk(&w, ln), 100 * w.config.data.data_price);
}

/// Plan 4.3 (V37): a departing raid with a deck member at the Hideout sets
/// `Gang.stream_by` and writes an `Overwatch` order; the stream runs on its
/// portal (no contest) and clears the flag when the raid is over.
#[test]
fn test_stream_flag_set_on_departure() {
    let mut w = world();
    let gang = w.gang_list()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let m = gang_runner(&mut w, gang, 2, 0.9);
    assert!(w.config.hack.stream_min <= 0.9);
    seat(&mut w, m, hideout);
    let now = w.tick;
    w.comp_mut::<Gang>(gang).expect("gang").raid_at = Some(now);
    assert_eq!(w.comp::<Gang>(gang).expect("gang").stream_by, None);
    virt::on_raid_departed(&mut w, gang);
    assert_eq!(w.comp::<Gang>(gang).expect("gang").stream_by, Some(m), "the streamer is set at departure");
    let o = w.run_orders.get(&m).cloned().expect("an Overwatch order");
    assert_eq!((o.purpose, o.why, o.chair), (Purpose::Overwatch(gang), RunWhy::Overwatch, hideout));
    let id = virt::start_run(&mut w, m).expect("the stream starts");
    assert_eq!(w.runs.get(&id).map(|r| r.contests), Some(0));
    // The raid resolves (raid_at cleared): the hourly check ends the stream.
    w.comp_mut::<Gang>(gang).expect("gang").raid_at = None;
    finish(&mut w);
    assert_eq!(w.comp::<Gang>(gang).expect("gang").stream_by, None, "cleared at resolve");
    assert!(!w.runner_of.contains_key(&m));
}

/// Plan 4.1 (V42): the remaining god commands and levers act: `GrantDeck`,
/// `SetIce`, `SetHackSentence` (read by `sentence_ticks`), `RunNow` (a God
/// order) and `Fry` (a seated runner loses its contest with no save).
#[test]
fn test_god_virt_commands_act() {
    use citysim::PlayerCommand;
    let mut w = world();
    let lab = lab_of(&w, corp_named(&w, "Zetatech"), Track::Chrome);
    let n = virt::node_of_building(&w, lab).expect("node");
    let a = adult(&mut w, &[]);
    w.push_command(PlayerCommand::GrantDeck { agent: a, tier: 2 });
    w.push_command(PlayerCommand::SetIce { building: lab, tier: 3 });
    w.push_command(PlayerCommand::SetHackSentence { crime: Crime::DataTheft, days: 40 });
    w.apply_commands();
    assert_eq!(w.comp::<Kit>(a).map(|k| k.deck_tier), Some(2));
    assert_eq!(virt::profile(&w, n).map(|p| p.ice), Some(3));
    assert_eq!(law::sentence_ticks(&w, Crime::DataTheft), 40 * 1440 * w.levers.sentence_mult.ceil() as u64);
    // RunNow writes a God order the Hack goal takes up.
    w.comp_mut::<Skills>(a).expect("skills").hacking = 0.5;
    w.push_command(PlayerCommand::RunNow { agent: a, target: lab, purpose: Purpose::Data { wipe: false } });
    w.apply_commands();
    let o = w.run_orders.get(&a).cloned().expect("a RunNow order");
    assert_eq!((o.why, o.target), (RunWhy::God, n));
    // Fry: seated, mid-run, the contest is lost with no save.
    let chair = o.chair;
    seat(&mut w, a, chair);
    virt::start_run(&mut w, a).expect("the run starts");
    w.push_command(PlayerCommand::Fry(a));
    w.apply_commands();
    assert!(!w.runner_of.contains_key(&a), "the run ended");
    let outcome = w.run_log.back().and_then(|r| r.outcome);
    assert!(matches!(outcome, Some(RunOutcome::Fried | RunOutcome::Flatlined)), "{outcome:?}");
    assert!(w.stats.current.virt.fried >= 1);
    assert!(w.run_log.back().is_some_and(|r| matches!(r.log.last(), Some(&(_, _, false)))), "logged as a lost contest");
    // Fry on an agent who is not jacked in is refused.
    let b = adult(&mut w, &[a]);
    w.push_command(PlayerCommand::Fry(b));
    w.apply_commands();
    assert!(w.events.iter().any(|e| e.text.contains("God: Fry: not jacked in")));
}
