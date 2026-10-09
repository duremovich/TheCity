//! M14 review fixes (`/code-review high 6c04ece..16b9efe`). Every run here
//! is a game abstraction: a fictional runner's token on abstract nodes of a
//! second board, each guarded node one seeded dice contest (deck tier plus
//! skill against the node's ICE tier), as `law::resolve_fight` resolves a
//! brawl; effects are timed flags on buildings and units of Data moved
//! between ledgers.

use citysim::systems::{assets, corps, demography, law, lod, tech, virt};
use citysim::virt::{NodeId, NodeKind, Purpose, RunMode, RunOrder, RunOutcome, RunWhy, Track};
use citysim::{
    ActionInstance, ActionKind, AssetKind, Brain, Building, BuildingKind, Config, Corp, EntityId, EventKind, ExecState,
    Gang, GangMember, GoalKind, Kit, Lod, Plan, Skills, World, TICKS_PER_DAY, TICKS_PER_HOUR,
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
            expires: now + TICKS_PER_DAY,
            why,
            mode: RunMode::Quiet,
        },
    );
}

fn set_ice(w: &mut World, n: NodeId, ice: u8) {
    virt::profile_mut(w, n).expect("profile").ice = ice;
    virt::bump_epoch(w);
}

fn seat(w: &mut World, a: EntityId, chair: EntityId) {
    w.leave_building(a);
    w.enter_building(a, chair);
}

/// Pop every queued run step, jumping the clock to each.
fn finish(w: &mut World) {
    while let Some(&(t, _)) = w.run_queue.first() {
        w.tick = w.tick.max(t);
        virt::run(w);
    }
}

fn last_outcome(w: &World) -> Option<RunOutcome> {
    w.run_log.back().and_then(|r| r.outcome)
}

fn events(w: &World, kind: EventKind, who: EntityId) -> usize {
    w.events.iter().filter(|e| e.kind == kind && e.actors.contains(&who)).count()
}

/// A seated run for `a` from the first Bar onto Militech's Deck Lab.
fn seated_run(w: &mut World, a: EntityId) -> u64 {
    let lab = lab_of(w, corp_named(w, "Militech"), Track::Deck);
    let n = virt::node_of_building(w, lab).expect("node");
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    seat(w, a, bar);
    order(w, a, bar, n, Purpose::Data { wipe: false }, RunWhy::God);
    let id = virt::start_run(w, a).expect("run");
    let now = w.tick;
    w.comp_mut::<Brain>(a).expect("brain").exec = ExecState::JackedIn { run: id, since: now };
    id
}

// ---------------------------------------------------------------------------
// 1. The route cache and save/load
// ---------------------------------------------------------------------------

/// Review 1: a tree built while a node's alarm stands is not reused after
/// the alarm lapses mid-hour (no write marks the lapse); the cached tree
/// equals a fresh search on a clone (whose cache is empty) at every tick.
#[test]
fn test_route_cache_drops_trees_at_alarm_expiry() {
    let mut w = world();
    w.tick = 10 * TICKS_PER_HOUR + 7;
    let a = adult(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    set_ice(&mut w, n, 1);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
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

/// Review 1: a save taken mid-hour after an alarm lapsed loads and runs on
/// exactly as the uninterrupted world, whose cache held trees built while
/// the alarms stood: every tree both read after the save is the same, so
/// are the runs they plan, and so is the world a day on.
#[test]
fn test_save_mid_hour_after_alarm_expiry_matches_uninterrupted_run() {
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 9 * TICKS_PER_HOUR + 5);
    // Thirty freelancers with decks of tiers 1-2 and hacking 0.3-0.97, so
    // the Hack goal reads the trees.
    let mut armed = Vec::new();
    for _ in 0..30 {
        let a = adult(&mut w, &armed);
        let i = armed.len();
        arm(&mut w, a, 1 + (i % 2) as u8, 0.3 + 0.67 * (i as f32 / 29.0));
        armed.push(a);
    }
    // Alarms lapsing mid-hour on every other district's Public node (an
    // alarm makes an unguarded node contested).
    let expiry = w.tick + 2;
    let alarmed: Vec<NodeId> = (0..w.virt.nodes.len())
        .map(|i| NodeId(i as u16))
        .filter(|&n| matches!(w.virt.node(n).map(|x| x.kind), Some(NodeKind::Public(d)) if d.0 % 2 == 0))
        .collect();
    assert!(!alarmed.is_empty());
    for &n in &alarmed {
        w.virt.node_mut(n).expect("node").alarm_until = Some(expiry);
    }
    virt::bump_epoch(&mut w);
    // Warm the cache with trees built while the alarms stand.
    for &a in &armed {
        let _ = virt::hack_choice(&w, a);
    }
    w.run_ticks(3);
    assert!(w.tick > expiry && !w.tick.is_multiple_of(TICKS_PER_HOUR), "mid-hour, after the lapse");
    let mut loaded = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("load");
    // The trees each freelancer's think reads are the same in both worlds.
    for &a in &armed {
        let Some(chair) = virt::chair_for(&w, a, None) else { continue };
        let deck = w.comp::<Kit>(a).map_or(0, |k| k.deck_tier);
        let att = virt::odds_of(&w, a, deck, None, virt::mode_of(&w, a)).att;
        let x = virt::routes_from(&w, chair.node, deck, att, None);
        let y = virt::routes_from(&loaded, chair.node, deck, att, None);
        assert_eq!(*x, *y, "a stale tree after the save");
    }
    // Both worlds plan their freelancers' runs at once (the Hack goal's
    // bind), then run on for a day.
    for world in [&mut w, &mut loaded] {
        for &a in &armed {
            let _ = virt::hack_plan(world, a);
        }
    }
    assert_eq!(w.run_orders, loaded.run_orders, "the same runs planned");
    w.run_ticks(TICKS_PER_DAY);
    loaded.run_ticks(TICKS_PER_DAY);
    let a = blake3::hash(citysim::save::to_ron(&w).as_bytes());
    let b = blake3::hash(citysim::save::to_ron(&loaded).as_bytes());
    assert_eq!(a, b, "the continued runs diverged");
}

// ---------------------------------------------------------------------------
// 2. A pending order is not replaced by a freelance one
// ---------------------------------------------------------------------------

/// Review 2: an agent holding a Prelude order not yet due (so not a
/// standing order) gets no freelance run: `hack_plan` leaves the order.
#[test]
fn test_hack_plan_keeps_a_pending_order() {
    let mut w = world();
    for g in w.gang_list().to_vec() {
        if let Some(n) = w.hideout_of(g).and_then(|h| virt::node_of_building(&w, h)) {
            w.virt.node_mut(n).expect("node").store.units[0] = 200;
        }
    }
    let a = adult(&mut w, &[]);
    arm(&mut w, a, 2, 0.9);
    assert!(virt::best_target(&w, a, None).is_some(), "a freelance run is on offer");
    let lab = lab_of(&w, corp_named(&w, "Militech"), Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    let now = w.tick;
    let pending = RunOrder {
        patron: None,
        purpose: Purpose::Door,
        target: n,
        chair: w.buildings_of_kind(BuildingKind::Bar)[0],
        not_before: now + 10 * TICKS_PER_HOUR,
        expires: now + TICKS_PER_DAY,
        why: RunWhy::Prelude,
        mode: RunMode::Quiet,
    };
    w.run_orders.insert(a, pending.clone());
    assert!(virt::standing_order(&w, a).is_none(), "not due yet");
    let _ = virt::hack_plan(&mut w, a);
    assert_eq!(w.run_orders.get(&a), Some(&pending), "the pending order stands");
    // A stale freelance order is still replaced.
    let mut stale = pending.clone();
    stale.why = RunWhy::Freelance;
    w.run_orders.insert(a, stale.clone());
    assert!(virt::hack_plan(&mut w, a).is_some());
    let got = w.run_orders.get(&a).expect("order");
    assert_eq!(got.why, RunWhy::Freelance);
    assert_ne!(got, &stale, "a fresh freelance order");
}

// ---------------------------------------------------------------------------
// 3. Put out of the chair building = dumped
// ---------------------------------------------------------------------------

/// Review 3: a sacked Hideout puts everyone inside out at the door
/// (`stand_at_door`); a runner seated there is dumped (V12), as is anyone
/// taken off a building's list while on a run.
#[test]
fn test_put_out_of_the_chair_dumps_the_run() {
    let mut w = world();
    let gang = w.gang_list()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let a = adult(&mut w, &[]);
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
    let b = adult(&mut w, &[a]);
    let before = w.events.len();
    w.stand_at_door(b, hideout);
    assert_eq!(w.events.len(), before);
}

// ---------------------------------------------------------------------------
// 4. The plane switched off under runs in progress
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 5. Gang.stream_by
// ---------------------------------------------------------------------------

/// Review 5: an Overwatch run that ends any way (here a dump) clears its
/// gang's `stream_by`, and so does an Overwatch order that lapses untaken.
#[test]
fn test_stream_by_cleared_on_dump_and_expiry() {
    let mut w = world();
    let gang = w.gang_list()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let node = virt::node_of_building(&w, hideout).expect("node");
    let a = adult(&mut w, &[]);
    citysim::systems::gang::enlist(&mut w, a, gang);
    arm(&mut w, a, 1, 0.5);
    seat(&mut w, a, hideout);
    order(&mut w, a, hideout, node, Purpose::Overwatch(gang), RunWhy::Overwatch);
    w.comp_mut::<Gang>(gang).expect("gang").stream_by = Some(a);
    virt::start_run(&mut w, a).expect("run");
    virt::dump(&mut w, a, "test");
    assert_eq!(w.comp::<Gang>(gang).and_then(|g| g.stream_by), None, "dumped");
    // An order never taken up, lapsing at midnight.
    let b = adult(&mut w, &[a]);
    citysim::systems::gang::enlist(&mut w, b, gang);
    let now = w.tick;
    order(&mut w, b, hideout, node, Purpose::Overwatch(gang), RunWhy::Overwatch);
    w.run_orders.get_mut(&b).expect("order").expires = now + 5;
    w.comp_mut::<Gang>(gang).expect("gang").stream_by = Some(b);
    w.tick = (now / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    tech::run(&mut w);
    assert!(!w.run_orders.contains_key(&b));
    assert_eq!(w.comp::<Gang>(gang).and_then(|g| g.stream_by), None, "lapsed");
}

// ---------------------------------------------------------------------------
// 6, 9. Out: an empty steal, an undeliverable steal
// ---------------------------------------------------------------------------

/// A won-every-contest Data run onto Militech's Deck Lab at ICE 0 for `a`
/// with `patron`; returns the Lab's node.
fn open_steal(w: &mut World, a: EntityId, patron: Option<EntityId>) -> NodeId {
    let lab = lab_of(w, corp_named(w, "Militech"), Track::Deck);
    let n = virt::node_of_building(w, lab).expect("node");
    set_ice(w, n, 0);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
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
    let mut w = world();
    let a = adult(&mut w, &[]);
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
    let mut w = world();
    let a = adult(&mut w, &[]);
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

// ---------------------------------------------------------------------------
// 7. Trace attribution by the hops to the tracing node
// ---------------------------------------------------------------------------

/// Review 7: the trace's confidence decays with the hops from the portal
/// to the node that traced, not with the whole route.
#[test]
fn test_trace_decay_counts_hops_to_the_tracing_node() {
    let mut w = world();
    let a = adult(&mut w, &[]);
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

// ---------------------------------------------------------------------------
// 8. JackIn without its order
// ---------------------------------------------------------------------------

/// Review 8: a `JackIn` waiting on an order not yet due fails when the
/// order vanishes; the post-run daze wait still completes the step.
#[test]
fn test_jackin_wait_fails_when_the_order_is_gone() {
    let mut w = world();
    let a = adult(&mut w, &[]);
    arm(&mut w, a, 2, 0.5);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
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

// ---------------------------------------------------------------------------
// 10, 12. The VirtRaid fleet deck; the reserve helper
// ---------------------------------------------------------------------------

/// Review 10: when only a tier below `RAID_DECK_TIER` fits above the fleet
/// reserve, `corp_virt_raid` buys nothing and gives no order.
#[test]
fn test_corp_virt_raid_buys_no_lower_tier_deck() {
    let mut w = world();
    // Arasaka's Deck Lab, staffed by one Researcher, against Militech's
    // Deck Lab holding Data (the rival `corp_run` reads).
    let arasaka = corp_named(&w, "Arasaka");
    let lab = lab_of(&w, arasaka, Track::Deck);
    for c in w.corps() {
        if let Some(cc) = w.comp_mut::<Corp>(c) {
            cc.tech.focus = Track::Deck;
        }
    }
    let r = adult(&mut w, &[]);
    demography::hire(&mut w, r, lab, citysim::Role::Researcher);
    let run = virt::corp_run(&w, arasaka).expect("a corp run");
    assert!(run.deck.is_none(), "no fleet deck at the Lab yet");
    // The purse: the reserve plus the cheapest tier-1 deck, short of any tier 2.
    let from = w.comp::<Building>(lab).expect("lab").door;
    let (_, _, p1) = virt::deck_offer(&w, from, i64::MAX / 4, 1).expect("a tier-1 deck on sale");
    let (_, t2, p2) = virt::deck_offer(&w, from, i64::MAX / 4, 2).expect("a deck on sale");
    assert!(t2 == 2 && p2 > p1, "tier 2 dearer: {p1} vs {p2}");
    let reserve = corps::fleet_reserve(&w, arasaka);
    w.comp_mut::<Corp>(arasaka).expect("corp").treasury = reserve + p1;
    assert_eq!(virt::deck_offer(&w, from, p1, 2).map(|(_, t, _)| t), Some(1), "only tier 1 fits");
    let decks = virt::decks_owned(&w);
    virt::corp_virt_raid(&mut w, arasaka);
    assert_eq!(virt::decks_owned(&w), decks, "no deck bought");
    assert!(!w.run_orders.contains_key(&run.runner), "no order");
}

/// Review 12: the fleet reserve is `treasury_ref / 4`.
#[test]
fn test_fleet_reserve_helper() {
    let w = world();
    for c in w.corps() {
        let r = w.comp::<Corp>(c).expect("corp").treasury_ref / 4;
        assert_eq!(corps::fleet_reserve(&w, c), r);
    }
    assert_eq!(corps::fleet_reserve(&w, EntityId::NONE), 0);
}

// ---------------------------------------------------------------------------
// 16. A hack grudge brings the wipe
// ---------------------------------------------------------------------------

/// Review 16: a gang whose node traced a rival gang's runner (`hacked_by`)
/// and whose best runner can reach the rival's Hideout store holds a
/// grudge: VirtRaid's flat gains `order_flat.virt_grudge`, and under the
/// order the wipe goes out first, ahead of a richer steal.
#[test]
fn test_hack_grudge_lifts_virtraid_and_orders_the_wipe_first() {
    use citysim::systems::faction::{self, OrderInputs};
    let mut w = world();
    let gangs = w.gang_list().to_vec();
    let (gang, rival) = (gangs[0], gangs[1]);
    let rival_node = w.hideout_of(rival).and_then(|h| virt::node_of_building(&w, h)).expect("rival node");
    w.virt.node_mut(rival_node).expect("node").store.units = [0, 40, 0];
    // A richer steal elsewhere: Zetatech's Chrome Lab with 600 units.
    let lab = lab_of(&w, corp_named(&w, "Zetatech"), Track::Chrome);
    let lab_node = virt::node_of_building(&w, lab).expect("node");
    w.virt.node_mut(lab_node).expect("node").store.units = [600, 0, 0];
    let r = adult(&mut w, &[]);
    citysim::systems::gang::enlist(&mut w, r, gang);
    arm(&mut w, r, 2, 0.6);
    virt::bump_epoch(&mut w);
    assert!(!virt::gang_virt_inputs(&w, gang, None).grudge, "no grudge yet");
    let now = w.tick;
    w.comp_mut::<Gang>(gang).expect("gang").hacked_by = Some((rival, now));
    let i = faction::gather_inputs(&w, gang).expect("inputs");
    assert!(i.hacked && i.virt_grudge, "{i:?}");
    let cfg = w.config.gangs.clone();
    let score = |i: &OrderInputs| {
        faction::score_orders(i, &cfg)
            .into_iter()
            .find(|s| s.order == citysim::Order::VirtRaid)
            .map(|s| s.score)
            .expect("VirtRaid scored")
    };
    let calm = OrderInputs { virt_grudge: false, ..i.clone() };
    let lift = score(&i) - score(&calm);
    assert!(lift > 0.0 && (lift - cfg.order_flat.virt_grudge).abs() < 1e-6, "lift {lift}");
    w.comp_mut::<Gang>(gang).expect("gang").order = citysim::Order::VirtRaid;
    virt::gang_daily(&mut w, gang);
    let o = w.run_orders.get(&r).expect("an order");
    assert_eq!((o.purpose, o.target), (Purpose::Data { wipe: true }, rival_node), "the wipe first");
}
