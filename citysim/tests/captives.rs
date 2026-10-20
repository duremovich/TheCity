//! M16b phase 2 (docs/M16_CONTRACTS.md § 4, § 2): captives, the Private
//! Prison and the new kinds, one test per mechanism in the shipped seed-42
//! city: a hold (fed by the holder, the cuff, the quota), a ransom paid, a
//! hold's end (killed or let go), the refusal's Rescue and Escort, the
//! Rescue mission's door and win, an Abduct's live plan and hand-over, a
//! Harvest hand-over, the gang's and the corp's hostage postings (the
//! Prison built), an Escort kept, a Steal (the theft and the heist), a
//! Deliver (posted for an unburied corpse, carried and buried), a
//! fabricated Spin and its exposure, and a save mid-hold. Every captive,
//! ransom and story here is game state between fictional agents: a record
//! on a struct and a seeded dice roll. With `[hostage] enabled = false` the
//! mechanism tests fail (the switch gates every entry point).

use citysim::contract::{
    Cargo, CoercionStatus, CoercionWhy, ContractKind, ContractStatus, Demand, Hold, Origin, Posting, Target,
};
use citysim::systems::econ::identity;
use citysim::systems::{captives, contracts, demography, gang, law};
use citysim::word::Deed;
use citysim::{
    AssetKind, Brain, Building, BuildingKind, Config, Corpse, Crime, EntityId, EventKind, Gang, GangMember, Lod, Needs,
    Personality, Position, RelKind, Skills, World, TICKS_PER_DAY,
};

/// The shipped city with the outside's parents off (M17 phase 4): these
/// seeded-world tests read the M16b city's day-40 gangs and records (a
/// direct record's keyed renege draw is by its id, which the parents' runs
/// and scrip move).
fn world() -> World {
    let mut c = Config::load();
    c.outside.enabled = false;
    // Jobs & lives phase 1: and without the industries (they take the Lots
    // and the jobs that set this city's gangs and businesses).
    c.jobs2.enabled = false;
    // Lives at work phase 6: nor the careers and lieutenants (their seeded
    // jobs and ranks move the gangs this file's scenes are cut from).
    c.strip_careers();
    c.gangs.strip_lieutenants();
    World::new(42, c)
}

/// The seed-42 city once two gangs are led and one holds a district.
fn gang_world() -> World {
    static SAVE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let text = SAVE.get_or_init(|| {
        let mut w = world();
        for _ in 0..40 {
            w.run_ticks(TICKS_PER_DAY);
            let led = w.gangs().into_iter().filter(|&g| w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some())).count();
            let held = w.gangs().into_iter().any(|g| !gang::held_districts(&w, g).is_empty());
            if led >= 2 && held {
                break;
            }
        }
        citysim::save::to_ron(&w)
    });
    citysim::save::from_ron(text).expect("loads")
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
}

/// `n` free adults with no edge between any two, no gang, no guard's job.
fn strangers(w: &World, n: usize) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = Vec::new();
    for a in adults(w) {
        if w.has::<GangMember>(a) || law::is_guard(w, a) || w.has::<citysim::Sentence>(a) {
            continue;
        }
        if w.spouse_of(a).is_some() || out.iter().any(|&o| w.edge(o, a).is_some()) {
            continue;
        }
        if w.comp::<Brain>(a).is_none_or(|b| b.cuffed_by.is_some()) {
            continue;
        }
        out.push(a);
        if out.len() == n {
            return out;
        }
    }
    panic!("{n} strangers");
}

fn set_coins(w: &mut World, id: EntityId, coins: i64) {
    if let Some(x) = w.comp_mut::<citysim::Wallet>(id) {
        x.coins = coins;
    }
}

fn heavy(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.lawfulness = 0.0;
        p.courage = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 1.0;
        s.intimidation = 1.0;
        s.persuasion = 1.0;
        s.deception = 1.0;
    }
}

fn soft(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.courage = 0.0;
        p.greed = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 0.0;
        s.knowledge = 0.0;
    }
}

fn until_ok(w: &mut World, mut f: impl FnMut(&mut World) -> bool) -> bool {
    for _ in 0..300 {
        w.lev_tried.clear();
        if f(w) {
            return true;
        }
        w.tick += 1;
    }
    false
}

fn has_event(w: &World, kind: EventKind, who: EntityId) -> bool {
    w.events.iter().any(|e| e.kind == kind && e.actors.contains(&who))
}

/// A led gang with a Hideout: (gang, leader, hideout).
fn a_gang(w: &mut World) -> (EntityId, EntityId, EntityId) {
    for g in w.gangs() {
        let Some(l) = w.comp::<Gang>(g).and_then(|x| x.leader) else { continue };
        let Some(h) = w.hideout_of(g) else { continue };
        heavy(w, l);
        w.comp_mut::<Gang>(g).unwrap().treasury = 5_000;
        return (g, l, h);
    }
    panic!("a led gang with a Hideout");
}

/// A hold: the captive inside, cuffed to the building, a body; the
/// hostage coercion open over the default answerer; the holder pays its
/// meal at midnight; released, it walks out.
#[test]
fn test_hold_feeds_and_releases() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    // Shadow V4 batch 3: the premise's jobless stranger (on the moved city
    // the first stranger held a post, and the employer answered).
    let pool = strangers(&w, 12);
    let v = *pool.iter().find(|&&x| !w.has::<citysim::Job>(x)).expect("a jobless stranger");
    let kin = *pool.iter().find(|&&x| x != v).expect("a kin");
    w.edge_entry(v, kin).kind = RelKind::Family;
    let before = identity(&w);
    let demand = Demand::Coins { amount: 200, weekly: false };
    assert!(captives::take(&mut w, v, Some(g), l, h, Some(demand), None), "held");
    assert!(w.comp::<Brain>(v).is_some_and(|b| b.cuffed_by == Some(h) && b.lod != Lod::Statistical));
    assert_eq!(w.comp::<Position>(v).and_then(|p| p.building), Some(h));
    let cid = w.captives[&v].coercion.expect("a hostage coercion");
    assert_eq!(w.coercions[&cid].why, CoercionWhy::Hostage);
    assert_eq!(w.coercions[&cid].over, kin, "the kin answers for a jobless stranger");
    if let Some(n) = w.comp_mut::<Needs>(v) {
        n.hunger = 0.2;
    }
    let treasury = w.purse(Some(g));
    captives::feed(&mut w);
    assert!(w.comp::<Needs>(v).is_some_and(|n| n.hunger > 0.2), "fed");
    assert_eq!(w.purse(Some(g)), treasury - w.config.hostage.hostage_food, "the holder paid");
    assert_eq!(identity(&w), before, "no coin made or lost");
    captives::release(&mut w, v, captives::Freed::Expired);
    assert!(!captives::is_captive(&w, v));
    assert!(w.comp::<Brain>(v).is_some_and(|b| b.cuffed_by.is_none()));
    assert!(has_event(&w, EventKind::HostageFreed, v));
    assert_eq!(w.coercions[&cid].status, CoercionStatus::Broken);
}

/// The quota: past `captive_quota` no taking is made.
#[test]
fn test_quota_blocks_a_taking() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    let [v] = strangers(&w, 1)[..] else { unreachable!() };
    w.config.hostage.captive_quota = 0;
    assert!(!captives::take(&mut w, v, Some(g), l, h, None, None));
    assert!(w.comp::<Brain>(v).is_some_and(|b| b.cuffed_by.is_none()));
}

/// A ransom: the holder's head leans on the payer (a roll a day); the
/// coins move holder-ward, the captive walks (`Ransomed`), the coercion
/// complied; coins conserved.
#[test]
fn test_ransom_paid_frees_the_captive() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    let [v, over] = strangers(&w, 2)[..] else { unreachable!() };
    w.edge_entry(v, over).kind = RelKind::Spouse;
    soft(&mut w, over);
    set_coins(&mut w, over, 5_000);
    let before = identity(&w);
    let treasury = w.purse(Some(g));
    let demand = Demand::Coins { amount: 200, weekly: false };
    assert!(captives::take(&mut w, v, Some(g), l, h, Some(demand), Some(over)));
    let cid = w.captives[&v].coercion.unwrap();
    assert!(until_ok(&mut w, |w| captives::roll_hostage(w, v)), "paid within the tries");
    assert!(!captives::is_captive(&w, v));
    assert_eq!(w.coercions[&cid].status, CoercionStatus::Complied);
    assert_eq!(w.purse(Some(g)), treasury + 200);
    assert_eq!(w.stats.current.captive.ransomed, 1);
    assert_eq!(identity(&w), before);
}

/// The hold's end: a lawless holder kills at `p_hostage_kill` 1 (a Murder,
/// `HostageKilled`); at 0 the captive walks.
#[test]
fn test_hold_end_kills_or_frees() {
    for (p, killed) in [(1.0, true), (0.0, false)] {
        let mut w = gang_world();
        let (g, l, h) = a_gang(&mut w);
        let [v] = strangers(&w, 1)[..] else { unreachable!() };
        w.config.hostage.p_hostage_kill = p;
        assert!(captives::take(&mut w, v, Some(g), l, h, Some(Demand::Coins { amount: 10, weekly: false }), None));
        captives::end_hold(&mut w, v);
        assert!(!captives::is_captive(&w, v));
        assert_eq!(!law::living(&w, v), killed, "p {p}");
        if killed {
            assert!(has_event(&w, EventKind::HostageKilled, v));
        } else {
            assert!(has_event(&w, EventKind::HostageFreed, v));
        }
    }
}

/// A refusal sends the coerced for help: a Rescue on the captive and an
/// Escort on itself; the Rescue marches on the hold (a mission whose door
/// is the holding building's).
#[test]
fn test_refusal_posts_rescue_and_escort() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    let [v, over] = strangers(&w, 2)[..] else { unreachable!() };
    w.edge_entry(v, over).kind = RelKind::Spouse;
    set_coins(&mut w, over, 5_000);
    assert!(captives::take(&mut w, v, Some(g), l, h, Some(Demand::Coins { amount: 200, weekly: false }), Some(over)));
    captives::seek_help(&mut w, v, over, None);
    let rescue = w.contracts.values().find(|c| c.kind == ContractKind::Rescue).expect("a Rescue").clone();
    assert_eq!(rescue.target, Target::Agent(v));
    assert!(w.contracts.values().any(|c| c.kind == ContractKind::Escort && c.target == Target::Agent(over)));
    let (door, building) = contracts::mission_door(&w, rescue.id, w.tick);
    assert_eq!(building, Some(h), "the mission marches on the hold");
    assert_eq!(Some(door), citysim::systems::missions::door_of(&w, h));
}

/// A Rescue won frees every captive of that holder at the hold (`Rescued`
/// deed told).
#[test]
fn test_rescue_win_frees_the_holders_captives() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    let [a, b, crew] = strangers(&w, 3)[..] else { unreachable!() };
    let d = Some(Demand::Coins { amount: 10, weekly: false });
    assert!(captives::take(&mut w, a, Some(g), l, h, d, None));
    assert!(captives::take(&mut w, b, Some(g), l, h, d, None));
    assert_eq!(captives::guards_at(&w, a).len(), captives::guards_at(&w, b).len());
    captives::rescue_win(&mut w, a, &[crew]);
    assert!(!captives::is_captive(&w, a) && !captives::is_captive(&w, b));
    assert_eq!(w.stats.current.captive.rescued_captives, 2);
    let dist = w.district_of_building(h);
    assert!(w.rumours[dist.index()].entries.iter().any(|e| e.deed == Deed::Rescued && e.actor == Some(crew)));
}

/// An Abduct's live plan: the chase, the grab, the walk to the hold, the
/// hand-over; at the hold with the abductee in tow the hand-over holds
/// it for the buyer and fulfils the record.
#[test]
fn test_abduct_plan_and_hand_over() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    let [taker, v] = strangers(&w, 2)[..] else { unreachable!() };
    heavy(&mut w, taker);
    let hold = Hold { at: h, demand: Some(Demand::Coins { amount: 100, weekly: false }), over: None };
    let id = captives::post_abduct(
        &mut w,
        g,
        Some(l),
        v,
        None,
        Origin::GangOrder(citysim::Order::Retaliate),
        Some(50),
        hold,
    )
    .expect("posted");
    // Shadow V4 batch 2: on the moved city the buyer's keyed renege roll
    // came up; this test is the hand-over, so the buyer pays.
    w.contracts.get_mut(&id).expect("record").renege = false;
    // Shadow V4 batch 4: room for the run (on the moved city the city's
    // live runs filled the room, `max_missions` or the bodies' quota, and the
    // record stayed queued with no run to plan from).
    w.config.missions.max_missions = 1_000;
    w.config.lod.contract_quota = 10_000;
    contracts::god_take(&mut w, id, taker).expect("taken");
    assert_eq!(w.contracts[&id].render, citysim::contract::Render::Live, "an Abduct is never ledger");
    let plan = contracts::plan(&mut w, taker).expect("a plan");
    let acts: Vec<_> = plan.steps.iter().map(|s| s.action).collect();
    assert!(acts.contains(&citysim::ActionKind::Abduct) && acts.last() == Some(&citysim::ActionKind::HandOver));
    // The grab done: the victim in tow, the taker at the hold.
    if let Some(b) = w.comp_mut::<Brain>(v) {
        b.cuffed_by = Some(taker);
        b.abducted_by = Some(taker);
    }
    if let Some(b) = w.comp_mut::<Brain>(taker) {
        b.escorting = Some(v);
    }
    w.enter_building(taker, h);
    let r = captives::on_hand_over(&mut w, taker, Some(h));
    assert_eq!(r, citysim::StepResult::Done);
    assert!(captives::is_captive(&w, v));
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    assert!(has_event(&w, EventKind::HostageTaken, v));
}

/// A gang under Retaliate, weaker than its rival gang, posts an Abduct of
/// the rival leader's Spouse, held at its Hideout for a ransom from the
/// rival leader (direct, to its own).
#[test]
fn test_weak_gang_under_retaliate_takes_a_hostage() {
    let mut w = gang_world();
    let gs: Vec<EntityId> =
        w.gangs().into_iter().filter(|&x| w.comp::<Gang>(x).is_some_and(|y| y.leader.is_some())).collect();
    // The rival the bigger gang (the weaker side's ratio under `hire_ratio`).
    let r = *gs.iter().max_by_key(|&&x| (w.comp::<Gang>(x).unwrap().members.len(), std::cmp::Reverse(x))).unwrap();
    let g = *gs.iter().find(|&&x| x != r).unwrap();
    assert!(w.comp::<Gang>(r).unwrap().members.len() >= 2, "a rival of two or more");
    w.comp_mut::<Gang>(g).unwrap().order = citysim::Order::Retaliate;
    w.comp_mut::<Gang>(g).unwrap().retaliate_on = Some(r);
    w.comp_mut::<Gang>(g).unwrap().treasury = 5_000;
    // Make g the weaker: trim its roster to one.
    let keep: Vec<EntityId> =
        w.comp::<Gang>(g).unwrap().members.iter().copied().filter(|&m| contracts::free_adult(&w, m)).take(1).collect();
    w.comp_mut::<Gang>(g).unwrap().members = keep;
    let rl = w.comp::<Gang>(r).unwrap().leader.unwrap();
    let [s] = strangers(&w, 1)[..] else { unreachable!() };
    if w.spouse_of(rl).is_none() {
        w.set_spouse(rl, s);
    }
    let spouse = w.spouse_of(rl).unwrap();
    captives::gang_hostages(&mut w);
    let c = w.contracts.values().find(|c| c.kind == ContractKind::Abduct).expect("an Abduct");
    assert_eq!((c.buyer, c.target, c.broker), (Some(g), Target::Agent(spouse), None));
    let hold = c.hold.expect("a hold");
    assert_eq!(hold.at, w.hideout_of(g).unwrap());
    assert_eq!(hold.over, Some(rl));
}

/// A corp refused twice by a seller, its exec dirty, builds a Private
/// Prison (none yet) and posts an Abduct of the seller exec's Spouse for
/// the building's sale.
#[test]
fn test_corp_refused_twice_takes_a_hostage_for_the_sale() {
    let mut w = gang_world();
    let corps = w.corps();
    let (corp, seller) = (corps[0], corps[1]);
    w.comp_mut::<citysim::Corp>(corp).unwrap().treasury = 50_000;
    let exec = w.comp::<citysim::Corp>(corp).and_then(|c| c.exec).unwrap();
    if let Some(p) = w.comp_mut::<Personality>(exec) {
        p.lawfulness = 0.0;
    }
    let sexec = w.comp::<citysim::Corp>(seller).and_then(|c| c.exec).unwrap();
    let [s] = strangers(&w, 1)[..] else { unreachable!() };
    if w.spouse_of(sexec).is_none() {
        w.set_spouse(sexec, s);
    }
    let building = w.comp::<citysim::Corp>(seller).unwrap().buildings[0];
    captives::on_refused(&mut w, corp, seller, building);
    assert!(!w.contracts.values().any(|c| c.kind == ContractKind::Abduct), "one refusal is not enough");
    captives::on_refused(&mut w, corp, seller, building);
    let c = w.contracts.values().find(|c| c.kind == ContractKind::Abduct).expect("an Abduct");
    let hold = c.hold.expect("a hold");
    assert_eq!(hold.demand, Some(Demand::Sell(building)));
    assert_eq!(w.comp::<Building>(hold.at).map(|b| b.kind), Some(BuildingKind::Prison));
    assert_eq!(w.owner_of(hold.at), Some(corp));
}

/// An Escort is a Guard on a person: kept (the client alive at the
/// deadline) it is fulfilled.
#[test]
fn test_escort_kept_is_fulfilled() {
    let mut w = world();
    let [client, taker] = strangers(&w, 2)[..] else { unreachable!() };
    set_coins(&mut w, client, 1_000);
    let p = Posting {
        buyer: Some(client),
        agent: Some(client),
        kind: ContractKind::Escort,
        target: Target::Agent(client),
        broker: None,
        deadline_days: 3,
        origin: Origin::God,
        price: Some(50),
    };
    let id = contracts::post(&mut w, p).expect("an Escort on oneself posts");
    contracts::god_take(&mut w, id, taker).expect("taken");
    assert_eq!(w.contracts[&id].render, citysim::contract::Render::Live);
    let plan = contracts::plan(&mut w, taker).expect("a plan");
    assert!(plan.steps.iter().any(|s| s.action == citysim::ActionKind::Guard));
    let now = w.tick;
    let c = w.contracts.get_mut(&id).unwrap();
    c.started = Some(now);
    c.deadline = now;
    contracts::daily(&mut w);
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
}

/// A Steal on a building (no heist): won, a share of the owner's purse
/// (at most `steal_max`) goes to the buyer; coins conserved.
#[test]
fn test_steal_takes_a_share() {
    let mut w = world();
    let [buyer, owner, thief] = strangers(&w, 3)[..] else { unreachable!() };
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    w.comp_mut::<Building>(bar).unwrap().owner = Some(owner);
    set_coins(&mut w, owner, 1_000);
    set_coins(&mut w, buyer, 1_000);
    heavy(&mut w, thief);
    let p = Posting {
        buyer: Some(buyer),
        agent: Some(buyer),
        kind: ContractKind::Steal,
        target: Target::Building(bar),
        broker: None,
        deadline_days: 7,
        origin: Origin::God,
        price: Some(50),
    };
    let id = contracts::post(&mut w, p).expect("a Steal on a building posts");
    w.config.contracts.max_attempts = 200;
    let before = identity(&w);
    let buyer_before = w.purse(Some(buyer));
    let done = until_ok(&mut w, |w| {
        if w.contracts[&id].status == ContractStatus::Open {
            contracts::god_take(w, id, thief).expect("taken");
        }
        captives::resolve_steal(w, id, thief);
        w.contracts[&id].status == ContractStatus::Fulfilled
    });
    assert!(done);
    let take = (1_000.0 * w.config.hostage.steal_share) as i64;
    assert_eq!(w.purse(Some(owner)), 1_000 - take);
    assert_eq!(w.purse(Some(buyer)), buyer_before + take - 50, "the take in, the price out");
    assert_eq!(identity(&w), before);
}

/// A heist: a Steal on a Lab with a node runs as a run order; its Data
/// goes to the buyer's Lab and the record is fulfilled.
#[test]
fn test_heist_data_goes_to_the_buyer() {
    let mut w = world();
    let labs: Vec<EntityId> = w.buildings_of_kind(BuildingKind::Lab).to_vec();
    let lab = labs
        .iter()
        .copied()
        .find(|&b| {
            citysim::systems::virt::node_of_building(&w, b).is_some()
                && w.owner_of(b).is_some_and(|o| w.has::<citysim::Corp>(o))
        })
        .expect("a corp Lab with a node");
    let victim = w.owner_of(lab).unwrap();
    let buyer = w
        .corps()
        .into_iter()
        .find(|&c| c != victim && citysim::systems::tech::data_lab_for(&w, c, citysim::virt::Track::ALL[0]).is_some())
        .expect("another corp with a Lab");
    let [runner] = strangers(&w, 1)[..] else { unreachable!() };
    let deck = citysim::systems::assets::grant(&mut w, runner, AssetKind::Deck, 2).expect("a deck");
    let p = Posting {
        buyer: Some(buyer),
        agent: None,
        kind: ContractKind::Steal,
        target: Target::Building(lab),
        broker: None,
        deadline_days: 7,
        origin: Origin::God,
        price: Some(50),
    };
    let id = contracts::post(&mut w, p).expect("posted");
    contracts::god_take(&mut w, id, runner).expect("taken");
    assert_eq!(w.steal_runs.get(&runner), Some(&id), "the heist is a run order");
    w.comp_mut::<citysim::Asset>(deck).unwrap().data = [5, 0, 0];
    let blab = citysim::systems::tech::data_lab_for(&w, buyer, citysim::virt::Track::ALL[0]).unwrap();
    let node = citysim::systems::virt::node_of_building(&w, blab).unwrap();
    let before = w.virt.node(node).unwrap().store.units[0];
    captives::on_heist_end(&mut w, runner, true);
    assert_eq!(w.virt.node(node).unwrap().store.units[0], before + 5);
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
}

/// A Deliver: kin of an unburied corpse (all off screen) with the price
/// post it; carried, the corpse is buried and the record fulfilled.
#[test]
fn test_deliver_posted_for_a_corpse_and_buried() {
    let mut w = world();
    let [dead, kin, taker] = strangers(&w, 3)[..] else { unreachable!() };
    w.edge_entry(dead, kin).kind = RelKind::Family;
    set_coins(&mut w, kin, 1_000);
    citysim::systems::lod::set_lod(&mut w, kin, Lod::Statistical);
    w.kill_by(dead, citysim::DeathCause::Violence, None);
    w.tick += 2 * TICKS_PER_DAY;
    captives::bury_posts(&mut w);
    let (&id, c) = w.contracts.iter().find(|(_, c)| c.kind == ContractKind::Deliver).expect("a Deliver");
    assert!(matches!(c.target, Target::Carry { what: Cargo::Corpse(x), .. } if x == dead));
    assert_eq!(c.origin, Origin::Bury);
    if c.status == ContractStatus::Open {
        contracts::god_take(&mut w, id, taker).expect("taken");
    }
    captives::resolve_deliver(&mut w, id, taker);
    assert!(w.comp::<Corpse>(dead).is_some_and(|k| k.buried));
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
}

/// A fabricated Spin: a dirty exec with no true misdeed buys a story that
/// never happened (the Feed's reporter fooled); named, its actor exposes
/// it (`Exposed`, Fraud filed on the exec, the Feed's reach cut).
#[test]
fn test_fabricated_spin_and_exposure() {
    let mut w = world();
    let feed = citysim::systems::news::feeds(&w).into_iter().find(|&f| w.owner_of(f).is_some()).expect("an owned Feed");
    let owner = w.owner_of(feed).unwrap();
    let corp = w
        .corps()
        .into_iter()
        .find(|&c| c != owner && citysim::systems::captives::fabricate_target(&w, c).is_some())
        .expect("a corp with a rival");
    w.comp_mut::<citysim::Corp>(corp).unwrap().treasury = 50_000;
    let exec = w.comp::<citysim::Corp>(corp).and_then(|c| c.exec).unwrap();
    heavy(&mut w, exec);
    for r in w.workers(citysim::Role::Reporter).to_vec() {
        if let Some(s) = w.comp_mut::<Skills>(r) {
            s.knowledge = 0.0;
        }
    }
    let e = until_ok(&mut w, |w| captives::fabricate(w, corp, feed, 0.5).is_some());
    assert!(e, "planted");
    assert!(w.contracts.values().any(|c| c.kind == ContractKind::Spin && c.status == ContractStatus::Fulfilled));
    let actor = captives::fabricate_target(&w, corp).unwrap();
    w.stories.push_back(citysim::word::Story {
        byline: None,
        id: 77,
        feed,
        deed: Deed::Betrayed,
        actor,
        source: None,
        object: Some(corp),
        tick: w.tick,
        slant: -1.0,
        paid_by: Some(corp),
        fabricated: true,
        exposed: false,
    });
    w.config.leverage.expose_p = 1.0;
    captives::expose(&mut w);
    assert!(w.stories.iter().any(|s| s.id == 77 && s.exposed));
    assert!(has_event(&w, EventKind::Exposed, corp));
    assert!(w.crime_reports().iter().any(|r| r.crime == Crime::Fraud && r.suspect == exec));
    assert!(w.feed_shamed.contains_key(&feed));
}

/// A save mid-hold runs on byte for byte.
#[test]
fn test_save_mid_hold_round_trips() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    let [v] = strangers(&w, 1)[..] else { unreachable!() };
    assert!(captives::take(&mut w, v, Some(g), l, h, Some(Demand::Coins { amount: 100, weekly: false }), None));
    w.run_ticks(30);
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text);
    w.run_ticks(TICKS_PER_DAY);
    back.run_ticks(TICKS_PER_DAY);
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w));
}

/// M16b review finding 2: a god hold with no holder has no hand-over
/// agent: the captive is not its own abductor, nobody rolls its demand and
/// nobody kills it at the hold's end.
#[test]
fn test_holderless_god_hostage_never_rolls_or_kills() {
    let mut w = gang_world();
    let (_, _, h) = a_gang(&mut w);
    let [v, over] = strangers(&w, 2)[..] else { unreachable!() };
    w.edge_entry(v, over).kind = RelKind::Spouse;
    w.config.hostage.p_hostage_kill = 1.0;
    w.push_command(citysim::PlayerCommand::Hostage {
        captive: v,
        holder: None,
        at: h,
        demand: Some(Demand::Coins { amount: 50, weekly: false }),
    });
    w.apply_commands();
    assert!(captives::is_captive(&w, v), "held");
    assert_ne!(w.captives[&v].agent, v, "not its own abductor");
    assert_ne!(w.comp::<Brain>(v).and_then(|b| b.abducted_by), Some(v));
    for _ in 0..20 {
        assert!(!captives::roll_hostage(&mut w, v), "nobody rolls");
        w.tick += 1;
    }
    captives::end_hold(&mut w, v);
    assert!(law::living(&w, v), "nobody kills");
    assert!(has_event(&w, EventKind::HostageFreed, v));
}

/// M16b review finding 4: a captive let go stands its Rescue and the
/// coerced party's Escort down (cancelled, the escrow refunded).
#[test]
fn test_release_stands_the_help_down() {
    let mut w = gang_world();
    let (g, l, h) = a_gang(&mut w);
    let [v, over] = strangers(&w, 2)[..] else { unreachable!() };
    w.edge_entry(v, over).kind = RelKind::Spouse;
    set_coins(&mut w, over, 5_000);
    assert!(captives::take(&mut w, v, Some(g), l, h, Some(Demand::Coins { amount: 200, weekly: false }), Some(over)));
    let escrow = w.escrow_held;
    captives::seek_help(&mut w, v, over, None);
    let live = |w: &World, k: ContractKind| w.contracts.values().filter(|c| c.kind == k && c.is_live()).count();
    assert_eq!((live(&w, ContractKind::Rescue), live(&w, ContractKind::Escort)), (1, 1));
    let before = identity(&w);
    captives::end_hold(&mut w, v);
    assert!(!captives::is_captive(&w, v));
    assert_eq!((live(&w, ContractKind::Rescue), live(&w, ContractKind::Escort)), (0, 0), "the help stood down");
    assert_eq!(w.escrow_held, escrow, "the escrow refunded");
    assert_eq!(identity(&w), before);
}
