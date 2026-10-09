//! M16a phase 1 (plan 1.16): contract records, escrow, settlement and
//! renege, visibility and matching, the acceptance score, Hunt hiring, the
//! ledger's pre-bound holes, the Locate stream, the Guard intercept, the
//! seeded Fixers and the identity of the chase refactor and the off
//! switch. Every record, hit, beat and bounty here is game state between
//! fictional agents: a struct with a price, matched by a score, resolved
//! by one seeded dice roll; escrow is coins held on a record.

use citysim::contract::{Broker, ContractKind, ContractStatus, Origin, Posting, Render, Target};
use citysim::systems::econ::identity;
use citysim::systems::{bind, contracts, demography, fviolence, grudges, hunt, law, lod, missions, ownership};
use citysim::word::{Deed, GrudgeCause, Grudges, HuntWhy, Reputation};
use citysim::{
    ActionKind, BuildingKind, Config, DeathCause, EntityId, GangMember, Job, Lod, MemoryKind, Personality, Position,
    RelKind, Role, Skills, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn world() -> World {
    World::new(42, Config::load())
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
        if out.iter().any(|&o| w.edge(o, a).is_some()) {
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

fn coins(w: &World, id: EntityId) -> i64 {
    w.purse(Some(id))
}

/// A gun: lawless, a strong fighter, jobless (accepts any record).
fn gun(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.lawfulness = 0.0;
        p.courage = 0.9;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 1.0;
        s.stealth = 1.0;
    }
    w.vacate_job(id);
}

/// The plan's worked target (fighting 0.4, courage 0.4, standing 0.2, no
/// expected allies: no Friend edges, no gang).
fn worked_target(w: &mut World, t: EntityId) {
    if let Some(s) = w.comp_mut::<Skills>(t) {
        s.fighting = 0.4;
    }
    if let Some(p) = w.comp_mut::<Personality>(t) {
        p.courage = 0.4;
    }
    let friends: Vec<EntityId> =
        w.neighbours(t).filter(|&o| w.edge(t, o).is_some_and(|e| e.kind == RelKind::Friend)).collect();
    for f in friends {
        w.remove_edge(t, f);
    }
    let rep = Reputation { standing: 0.2, ..Default::default() };
    w.insert(t, rep);
}

fn fixer(w: &World) -> EntityId {
    contracts::open_fixers(w)[0]
}

fn posting(buyer: EntityId, kind: ContractKind, target: EntityId, broker: Option<EntityId>) -> Posting {
    Posting {
        buyer: Some(buyer),
        agent: Some(buyer),
        kind,
        target: Target::Agent(target),
        broker,
        deadline_days: 10,
        origin: Origin::God,
        price: None,
    }
}

#[test]
fn test_brokered_post_escrows_and_identity_holds() {
    let mut w = world();
    let [buyer, t] = strangers(&w, 2)[..] else { unreachable!() };
    worked_target(&mut w, t);
    set_coins(&mut w, buyer, 1000);
    let f = fixer(&w);
    assert_eq!(contracts::quote(&w, ContractKind::Hit, &Target::Agent(t), Some(f)), 711, "the plan's worked quote");
    let (total, id0) = (ownership::total_coins(&w), identity(&w));
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, Some(f))).expect("posted");
    let c = &w.contracts[&id];
    assert_eq!((c.price, c.escrow, c.status), (711, 711, ContractStatus::Open));
    assert_eq!(coins(&w, buyer), 289);
    assert_eq!(w.escrow_held, 711);
    assert_eq!(ownership::total_coins(&w), total, "escrow is inside total_coins");
    assert_eq!(identity(&w), id0, "L2's identity");
    assert!(w.comp::<Broker>(f).is_some_and(|k| k.book.contains(&id)), "in the Fixer's book");
    // The placing agent knows first-hand.
    let heard = w.comp::<citysim::Memory>(buyer).is_some_and(|m| {
        m.heard.iter().any(|e| e.kind == MemoryKind::Rumour && e.deed == Some(Deed::Hired) && e.object == Some(t))
    });
    assert!(heard, "a first-hand Hired rumour in the buyer");
}

#[test]
fn test_fulfilled_pays_taker_and_fixer_at_cut() {
    let mut w = world();
    let [buyer, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
    worked_target(&mut w, t);
    gun(&mut w, taker);
    set_coins(&mut w, buyer, 1000);
    set_coins(&mut w, taker, 0);
    let f = fixer(&w);
    let owner = w.owner_of(f).expect("an owned Fixer");
    let (total, id0) = (ownership::total_coins(&w), identity(&w));
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, Some(f))).expect("posted");
    let (owner0, treasury0) = (w.purse(Some(owner)), w.purse(None));
    let rev0 = w.comp::<citysim::Building>(f).map_or(0, |b| b.revenue_today);
    assert!(contracts::accept(&mut w, id, taker, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    assert_eq!(coins(&w, taker), 569, "price less the cut");
    let tax = w.purse(None) - treasury0;
    assert_eq!(w.purse(Some(owner)) - owner0 + tax, 142, "the cut, tax withheld into the Treasury");
    assert!(tax >= 0);
    assert_eq!(w.comp::<citysim::Building>(f).map_or(0, |b| b.revenue_today) - rev0, 142, "credited to the Fixer");
    assert_eq!(w.contracts[&id].escrow, 0);
    assert_eq!(w.escrow_held, 0);
    assert_eq!(ownership::total_coins(&w), total);
    assert_eq!(identity(&w), id0);
}

#[test]
fn test_expired_and_failed_refund() {
    let mut w = world();
    let [buyer, t, a, b] = strangers(&w, 4)[..] else { unreachable!() };
    gun(&mut w, a);
    gun(&mut w, b);
    set_coins(&mut w, buyer, 2000);
    let f = fixer(&w);
    let total = ownership::total_coins(&w);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Beat, t, Some(f))).expect("posted");
    assert!(contracts::accept(&mut w, id, a, &[]));
    contracts::fail_attempt(&mut w, id, "test");
    assert_eq!(w.contracts[&id].status, ContractStatus::Open, "back on the board");
    assert_eq!(w.contracts[&id].attempts, 1);
    assert!(contracts::accept(&mut w, id, b, &[]));
    contracts::fail_attempt(&mut w, id, "test");
    assert_eq!(w.contracts[&id].status, ContractStatus::Failed, "max_attempts 2");
    assert_eq!(coins(&w, buyer), 2000, "every coin back");
    // The deadline.
    let id2 = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, Some(f))).expect("posted");
    assert!(coins(&w, buyer) < 2000);
    if let Some(c) = w.contracts.get_mut(&id2) {
        c.deadline = w.tick;
    }
    contracts::daily(&mut w);
    assert_eq!(w.contracts[&id2].status, ContractStatus::Expired);
    assert_eq!(coins(&w, buyer), 2000, "every coin back");
    assert_eq!(w.escrow_held, 0);
    assert_eq!(ownership::total_coins(&w), total);
}

#[test]
fn test_direct_renege_posts_betrayed_and_grudge() {
    let mut w = world();
    let [buyer, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, taker);
    set_coins(&mut w, buyer, 1000);
    set_coins(&mut w, taker, 0);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Beat, t, None)).expect("posted");
    assert_eq!(w.contracts[&id].escrow, 0, "a direct record escrows nothing");
    if let Some(c) = w.contracts.get_mut(&id) {
        c.renege = true;
    }
    assert!(contracts::accept(&mut w, id, taker, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Reneged);
    assert_eq!(coins(&w, taker), 0, "nothing paid");
    assert_eq!(coins(&w, buyer), 1000);
    let g = w.comp::<Grudges>(taker).and_then(|g| g.list.iter().find(|x| x.target == buyer).cloned());
    let g = g.expect("the taker's grudge on the buyer");
    assert_eq!(g.cause, GrudgeCause::Betrayed);
    assert!(g.weight >= w.config.contracts.renege_grudge - 1e-5);
    let told = w.comp::<citysim::Memory>(taker).is_some_and(|m| {
        m.heard.iter().any(|e| e.deed == Some(Deed::Betrayed) && e.subject == Some(buyer) && e.object == Some(taker))
    });
    assert!(told, "the taker holds the Betrayed deed first-hand");
}

#[test]
fn test_short_buyer_is_reneged() {
    let mut w = world();
    let [buyer, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, taker);
    set_coins(&mut w, buyer, 1000);
    set_coins(&mut w, taker, 0);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Beat, t, None)).expect("posted");
    if let Some(c) = w.contracts.get_mut(&id) {
        c.renege = false;
    }
    let price = w.contracts[&id].price;
    set_coins(&mut w, buyer, price / 2);
    assert!(contracts::accept(&mut w, id, taker, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Reneged, "the taker cannot tell malice from poverty");
    assert_eq!(coins(&w, taker), price / 2, "what the buyer had");
    assert!(grudges::holds(&w, taker, buyer, 0.5));
}

#[test]
fn test_visible_hides_brokered_from_non_regular_shows_direct_to_friend() {
    let mut w = world();
    let [buyer, t, friend, other] = strangers(&w, 4)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    let f = fixer(&w);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Beat, t, Some(f))).expect("posted");
    assert!(!contracts::sees(&w, other, id), "a stranger, no regular, no gang");
    let now = w.tick;
    if let Some(k) = w.comp_mut::<Broker>(f) {
        k.regulars.insert(other, now);
    }
    assert!(contracts::sees(&w, other, id), "a regular");
    assert!(contracts::visible(&w, other).contains(&id));
    assert!(!contracts::sees(&w, t, id), "never on oneself");
    let d = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, None)).expect("posted");
    w.edge_entry(buyer, friend).kind = RelKind::Friend;
    assert!(contracts::sees(&w, friend, d), "the buyer's Friend");
    if let Some(k) = w.comp_mut::<Broker>(f) {
        k.regulars.clear();
    }
    assert!(!contracts::sees(&w, other, d), "a stranger to the buyer");
}

#[test]
fn test_matching_never_offers_hit_to_target_kin() {
    let mut w = world();
    let [buyer, t, spouse, kin, friend] = strangers(&w, 5)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    w.set_spouse(t, spouse);
    w.edge_entry(t, kin).kind = RelKind::Family;
    w.edge_entry(t, friend).kind = RelKind::Friend;
    let f = fixer(&w);
    let now = w.tick;
    for g in [spouse, kin, friend] {
        gun(&mut w, g);
    }
    if let Some(k) = w.comp_mut::<Broker>(f) {
        k.regulars.clear();
        for g in [spouse, kin, friend] {
            k.regulars.insert(g, now);
        }
    }
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, Some(f))).expect("posted");
    contracts::match_day(&mut w);
    assert_eq!(w.contracts[&id].status, ContractStatus::Open, "no kin takes it");
    assert_eq!(w.contracts[&id].taker, None);
}

#[test]
fn test_matching_is_deterministic() {
    let pick = |rev: bool| {
        let mut w = world();
        let ids = strangers(&w, 12);
        let (buyer, t, cands) = (ids[0], ids[1], ids[2..].to_vec());
        set_coins(&mut w, buyer, 5000);
        let f = fixer(&w);
        let now = w.tick;
        for &c in &cands {
            gun(&mut w, c);
        }
        let mut order = cands.clone();
        if rev {
            order.reverse();
        }
        if let Some(k) = w.comp_mut::<Broker>(f) {
            k.regulars.clear();
            for c in order {
                k.regulars.insert(c, now);
            }
        }
        let id = contracts::post(&mut w, posting(buyer, ContractKind::Beat, t, Some(f))).expect("posted");
        contracts::match_day(&mut w);
        w.contracts[&id].taker
    };
    let (a, b) = (pick(false), pick(true));
    assert!(a.is_some(), "a gun took it");
    assert_eq!(a, b, "the same offers whatever the insertion order");
}

#[test]
fn test_accept_score_same_at_every_tier() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    let f = fixer(&w);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, Some(f))).expect("posted");
    lod::set_lod(&mut w, g, Lod::Full);
    let full = contracts::accept_score(&w, g, id);
    lod::set_lod(&mut w, g, Lod::Statistical);
    assert_eq!(w.comp::<citysim::Brain>(g).map(|b| b.lod), Some(Lod::Statistical));
    let stat = contracts::accept_score(&w, g, id);
    assert!((full - stat).abs() < 1e-6, "{full} vs {stat}");
    assert!(full > 0.0);
}

/// A holder with a `w` grudge on a stronger target, `coins` in hand.
fn weak_holder(w: &mut World, weight: f32, coins_: i64) -> (EntityId, EntityId) {
    let [h, t] = strangers(w, 2)[..] else { unreachable!() };
    if let Some(s) = w.comp_mut::<Skills>(h) {
        s.fighting = 0.1;
    }
    if let Some(p) = w.comp_mut::<Personality>(h) {
        p.lawfulness = 0.1;
        p.courage = 0.9;
    }
    if let Some(s) = w.comp_mut::<Skills>(t) {
        s.fighting = 0.9;
    }
    grudges::add(w, h, t, GrudgeCause::Assaulted, weight, 0);
    if let Some(g) = w.comp_mut::<Grudges>(h) {
        if let Some(x) = g.list.iter_mut().find(|x| x.target == t) {
            x.weight = weight;
        }
    }
    set_coins(w, h, coins_);
    (h, t)
}

#[test]
fn test_hunt_hires_below_hire_gap() {
    for (weight, kind) in [(0.9, ContractKind::Hit), (0.8, ContractKind::Beat)] {
        let mut w = world();
        let (h, t) = weak_holder(&mut w, weight, 5000);
        assert!(hunt::might_gap(&w, h, t) < w.config.contracts.hire_gap, "too weak to win");
        assert!(!hunt::adopt(&mut w, h, HuntWhy::Goal), "no Hunt: a hire");
        assert!(!w.hunts.contains_key(&h), "no HuntState");
        let c = w.contracts.values().find(|c| c.buyer == Some(h)).expect("a record posted");
        assert_eq!((c.kind, c.target, c.origin), (kind, Target::Agent(t), Origin::Hunt));
        assert!(w.comp::<citysim::Brain>(h).and_then(|b| b.cooldowns.get(&citysim::GoalKind::Hunt)).is_some());
    }
}

#[test]
fn test_hire_pass_posts_for_statistical_holder_without_promotion() {
    let mut w = world();
    let (h, t) = weak_holder(&mut w, 0.9, 5000);
    lod::set_lod(&mut w, h, Lod::Statistical);
    let mut posted = false;
    for _ in 0..9 {
        contracts::hire_pass(&mut w);
        if w.contracts.values().any(|c| c.buyer == Some(h) && c.target == Target::Agent(t)) {
            posted = true;
            break;
        }
        w.tick += TICKS_PER_DAY;
    }
    assert!(posted, "the hash picks one day in three");
    assert_eq!(w.comp::<citysim::Brain>(h).map(|b| b.lod), Some(Lod::Statistical), "never promoted for it");
}

/// A Statistical gun and target, a direct Hit taken on the ledger, the
/// draw forced (`strike_k` huge, the gun far stronger), resolved now.
fn ledger_hit(w: &mut World) -> (EntityId, EntityId, citysim::contract::ContractId) {
    w.config.missions.strike_k = 100.0;
    let [buyer, t, taker] = strangers(w, 3)[..] else { unreachable!() };
    gun(w, taker);
    if let Some(s) = w.comp_mut::<Skills>(t) {
        s.fighting = 0.0;
    }
    set_coins(w, buyer, 5000);
    for a in [t, taker] {
        lod::set_lod(w, a, Lod::Statistical);
    }
    let id = contracts::post(w, posting(buyer, ContractKind::Hit, t, None)).expect("posted");
    assert!(contracts::accept(w, id, taker, &[]));
    assert_eq!(w.contracts[&id].render, Render::Ledger, "both Statistical, off screen");
    contracts::resolve_ledger(w, id);
    (t, taker, id)
}

#[test]
fn test_ledger_hit_prebound_hole_binds_taker_same_witness_after_save_load() {
    let mut w = world();
    let (t, taker, id) = ledger_hit(&mut w);
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    assert!(w.has::<citysim::Corpse>(t), "the target is dead");
    let (&hid, hole) = w.holes.iter().find(|(_, h)| h.victim == t).expect("a hole");
    assert_eq!(hole.source, Some(citysim::ledger::ViolenceSource::Contract(id)));
    assert_eq!(hole.faction, Some(taker));
    let text = citysim::save::to_ron(&w);
    let mut w2 = citysim::save::from_ron(&text).expect("loads");
    let a = bind::bind(&mut w, hid);
    let b = bind::bind(&mut w2, hid);
    assert_eq!(a, Some(citysim::Bound::Actor(taker)), "bound to the taker");
    assert_eq!(a, b, "the same binding after save and load");
    let reports = |w: &World| -> Vec<(EntityId, Option<EntityId>)> {
        w.crime_reports().iter().filter(|r| r.suspect == taker).map(|r| (r.suspect, r.witness)).collect()
    };
    assert_eq!(reports(&w), reports(&w2), "the same witness");
    assert_eq!(w.stats.current.contract.contract_hole_wrong, 0);
    assert_eq!(w.stats.current.contract.contract_holes, 1);
}

#[test]
fn test_prebound_witness_offset_matches_ordinary_hole() {
    // The same hole id, bound to the same actor: a contract hole (two
    // draws discarded) and an ordinary faction hole whose only candidate
    // is that actor (an agent is its own faction; `p_unknown` 0) read the
    // witness roll at the same offset.
    let run = |contract: bool| {
        let mut w = world();
        w.config.bind.p_unknown = 0.0;
        w.config.bind.p_witness = 1.0;
        let [v, actor] = strangers(&w, 2)[..] else { unreachable!() };
        for a in [v, actor] {
            lod::set_lod(&mut w, a, Lod::Statistical);
        }
        // A weight above zero in the ordinary pool.
        if let Some(p) = w.comp_mut::<Personality>(actor) {
            p.lawfulness = 0.2;
        }
        // A day of traces for the pool.
        w.run_ticks(TICKS_PER_DAY);
        let tick = w.tick - 10;
        let tile = w.comp::<Position>(v).map(|p| p.tile).unwrap_or_default();
        let district = w.district_of(tile);
        let source = if contract {
            citysim::ledger::ViolenceSource::Contract(1)
        } else {
            citysim::ledger::ViolenceSource::Episode
        };
        let hole = citysim::Hole {
            id: citysim::hole_id(tick, v, citysim::HoleKind::Assaulted),
            kind: citysim::HoleKind::Assaulted,
            victim: v,
            zone: w.map.zone(tile),
            district,
            tick,
            event_id: 0,
            consequential: true,
            spouse: None,
            loot: 0,
            home: None,
            gang: None,
            source: Some(source),
            faction: Some(actor),
            riot: None,
        };
        let hid = bind::open_hole(&mut w, hole);
        let bound = bind::bind(&mut w, hid);
        let rep: Vec<(EntityId, Option<EntityId>)> =
            w.crime_reports().iter().filter(|r| r.suspect == actor).map(|r| (r.suspect, r.witness)).collect();
        (bound, rep)
    };
    let (a, ra) = run(true);
    let (b, rb) = run(false);
    assert_eq!(a, b, "both bind the actor");
    assert!(matches!(a, Some(citysim::Bound::Actor(_))));
    assert_eq!(ra, rb, "the same witness roll");
}

#[test]
fn test_ledger_kill_names_nobody_before_bind() {
    let mut w = world();
    w.config.missions.strike_k = 100.0;
    let [buyer, t, taker, spouse] = strangers(&w, 4)[..] else { unreachable!() };
    w.set_spouse(t, spouse);
    gun(&mut w, taker);
    if let Some(s) = w.comp_mut::<Skills>(t) {
        s.fighting = 0.0;
    }
    set_coins(&mut w, buyer, 5000);
    for a in [t, taker, spouse] {
        lod::set_lod(&mut w, a, Lod::Statistical);
    }
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, None)).expect("posted");
    assert!(contracts::accept(&mut w, id, taker, &[]));
    contracts::resolve_ledger(&mut w, id);
    assert!(w.has::<citysim::Corpse>(t));
    assert!(!grudges::holds(&w, spouse, taker, 0.0), "the widow holds no grudge on the taker before the hole binds");
}

#[test]
fn test_contract_killing_not_in_order_rates() {
    let mut w = world();
    let gangs = w.gang_list().to_vec();
    let (ga, gb) = (gangs[0], gangs[1]);
    let free = strangers(&w, 3);
    citysim::systems::gang::enlist(&mut w, free[0], ga);
    citysim::systems::gang::enlist(&mut w, free[1], gb);
    let (taker, t, buyer) = (free[0], free[1], free[2]);
    grudges::declare(&mut w, ga, gb, 1.0);
    assert!(fviolence::source_of(&w, taker, t).is_some(), "a vendetta act without a record");
    set_coins(&mut w, buyer, 5000);
    gun(&mut w, taker);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, None)).expect("posted");
    lod::set_lod(&mut w, taker, Lod::Coarse);
    assert!(contracts::accept(&mut w, id, taker, &[]));
    assert!(contracts::live_job_on(&w, taker, t), "a live run");
    assert_eq!(fviolence::source_of(&w, taker, t), None, "the record's job is no order's act");
}

#[test]
fn test_target_killed_elsewhere_cancels_and_refunds() {
    let mut w = world();
    let [buyer, t, taker, other] = strangers(&w, 4)[..] else { unreachable!() };
    gun(&mut w, taker);
    set_coins(&mut w, buyer, 3000);
    let f = fixer(&w);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, Some(f))).expect("posted");
    assert!(contracts::accept(&mut w, id, taker, &[]));
    w.kill_by(t, DeathCause::Violence, Some(other));
    assert_eq!(w.contracts[&id].status, ContractStatus::Cancelled);
    assert_eq!(coins(&w, buyer), 3000, "refunded");
    assert_eq!(w.escrow_held, 0);
    assert!(!w.contract_runs.contains_key(&taker), "the run ended");
}

#[test]
fn test_locate_pays_at_most_once_per_gap_and_caps() {
    let mut w = world();
    let [buyer, t, eye] = strangers(&w, 3)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    set_coins(&mut w, eye, 0);
    let f = fixer(&w);
    let mut p = posting(buyer, ContractKind::Locate, t, Some(f));
    p.price = Some(1000);
    let id = contracts::post(&mut w, p).expect("posted");
    let now = w.tick;
    if let Some(k) = w.comp_mut::<Broker>(f) {
        k.regulars.insert(eye, now);
    }
    assert!(contracts::locate_wants(&w, eye, t));
    let tile = w.comp::<Position>(t).map(|p| p.tile).unwrap_or_default();
    for _ in 0..12 {
        contracts::on_sighting(&mut w, eye, t, tile);
        w.tick += TICKS_PER_HOUR;
    }
    assert_eq!(w.contracts[&id].paid_sightings, 2, "one per six hours over twelve");
    assert_eq!(coins(&w, eye), 2 * w.config.bounty.per_sighting);
    assert!(w.db.get(&buyer).is_some_and(|db| db.sightings.iter().any(|s| s.who == t)), "the buyer's database");
    w.config.bounty.cap_sightings = 3;
    for _ in 0..48 {
        contracts::on_sighting(&mut w, eye, t, tile);
        w.tick += TICKS_PER_HOUR;
    }
    assert_eq!(w.contracts[&id].paid_sightings, 3, "the cap");
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled, "fulfilled at the cap");
    assert_eq!(w.escrow_held, 0, "the rest refunded or cut");
}

#[test]
fn test_guard_intercepts_attack_on_client() {
    let mut w = world();
    let [client, g, attacker] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    if let Some(s) = w.comp_mut::<Skills>(attacker) {
        s.fighting = 0.0;
    }
    set_coins(&mut w, client, 5000);
    for a in [client, g, attacker] {
        lod::set_lod(&mut w, a, Lod::Coarse);
    }
    // A Guard bought on oneself (C31: the client is the buyer).
    let id = contracts::post(&mut w, posting(client, ContractKind::Guard, client, None)).expect("posted");
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::on_guard_start(&mut w, g, Some(client));
    let tile = w.comp::<Position>(client).map(|p| p.tile).unwrap_or_default();
    for a in [g, attacker] {
        w.leave_building(a);
        if let Some(pp) = w.comp_mut::<Position>(a) {
            pp.tile = tile;
            pp.building = None;
        }
    }
    w.leave_building(client);
    if let Some(pp) = w.comp_mut::<Position>(client) {
        pp.building = None;
    }
    assert_eq!(contracts::guard_for(&w, attacker, client), Some(g), "on post beside the client");
    let before = w.events.len();
    let now = w.tick;
    citysim::exec::actions::on_complete(&mut w, attacker, ActionKind::Attack, Some(client), now, now);
    let new: Vec<&citysim::Event> = w.events.iter().skip(before).collect();
    assert!(
        new.iter().any(|e| e.actors.first() == Some(&attacker) && e.actors.get(1) == Some(&g)),
        "the attacker met the guard first"
    );
    let hit_client = new.iter().any(|e| {
        matches!(e.kind, citysim::EventKind::Assault | citysim::EventKind::Murder)
            && e.actors.first() == Some(&attacker)
            && e.actors.get(1) == Some(&client)
    });
    let guard_won = law::living(&w, g) && !new.iter().any(|e| e.kind == citysim::EventKind::Death);
    if guard_won {
        assert!(!hit_client, "the client untouched");
    }
}

#[test]
fn test_fixer_seeded_in_mid_east_and_sump_central_or_logged() {
    // C9 deviation: one seeded Fixer (Mid East); Sump Central's Lot is left
    // for M12 and the per-capita rule founds the second.
    let w = world();
    let fixers = w.buildings_of_kind(BuildingKind::Fixer).to_vec();
    assert_eq!(fixers.len(), 1, "one seeded Fixer (log: {:?})", w.contract_log);
    let names: Vec<String> = fixers.iter().map(|&f| w.district_name(w.district_of_building(f)).to_string()).collect();
    let sc = w.districts.iter().position(|x| x.name == "Sump Central").expect("Sump Central");
    let off = World::new(42, Config::load().contracts_off());
    let lots = |w: &World| {
        citysim::systems::founding::vacant_lots(w)
            .into_iter()
            .filter(|&l| w.district_of_building(l).index() == sc)
            .count()
    };
    assert_eq!(lots(&w), lots(&off), "Sump Central keeps its vacant Lots");
    for want in contracts::SEEDED_FIXERS {
        let d = w.districts.iter().position(|x| x.name == want).expect("the district");
        let mask = w.district_adjacent[d];
        let ok = fixers.iter().any(|&f| {
            let i = w.district_of_building(f).index();
            i == d || mask & (1 << i) != 0
        });
        assert!(ok, "a Fixer in or beside {want}: {names:?}");
    }
    for &f in &fixers {
        assert!(w.comp::<Broker>(f).is_some(), "a record book");
        let staffed =
            ownership::staff_at(&w, f).iter().any(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == Role::Fixer))
                || w.vacancies.get(&f).is_some_and(|v| v.contains(&Role::Fixer));
        assert!(staffed, "a Fixer vacancy (or its hire)");
        assert!(w.owner_of(f).is_some(), "an agent owner");
    }
    // No coin minted: the same seed with contracts off holds the same coins.
    assert_eq!(ownership::total_coins(&w), ownership::total_coins(&off));
}

#[test]
fn test_chase_refactor_keeps_hunt_identical() {
    // With no contract run, `hunt::chase` is the Hunt's own state and the
    // intel readers answer from it; a god Hunt over three days runs the
    // same twice (the byte identity against 519f9a8 is the CLI device, C41).
    let run = || {
        let mut w = World::new(42, Config::load().contracts_off());
        let [h, t] = strangers(&w, 2)[..] else { unreachable!() };
        hunt::god_hunt(&mut w, h, t).expect("a god Hunt");
        let mut seen = Vec::new();
        for _ in 0..(3 * 24) {
            w.run_ticks(TICKS_PER_HOUR);
            if let (Some(c), Some(s)) = (hunt::chase(&w, h), w.hunts.get(&h)) {
                assert_eq!((c.target, c.phase, c.venue, c.intel), (s.target, s.phase, s.venue, s.intel));
                assert_eq!((c.stakeout_until, c.deceived, c.liar), (s.stakeout_until, s.deceived, s.liar));
                seen.push((c.phase, c.intel.map(|i| i.tile)));
            }
        }
        (seen, w.stats.history.iter().map(|r| r.csv_row()).collect::<Vec<_>>())
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "deterministic");
}

#[test]
fn test_contracts_off_world_matches_l2() {
    // `contracts_off` vs a config file with no M16a key at all.
    let dir = Config::find_assets_dir().expect("assets");
    let text = std::fs::read_to_string(dir.join("config.toml")).expect("config");
    let cut = text.find("# M16a contracts (docs/M16_CONTRACTS.md").expect("the M16a block");
    let mut bare: Config = toml::from_str(&text[..cut]).expect("parses");
    bare.assets_dir = dir.clone();
    assert!(!bare.contracts.enabled, "an absent [contracts] is off");
    let rows = |cfg: Config| {
        let mut w = World::new(42, cfg);
        w.run_ticks(3 * TICKS_PER_DAY);
        w.stats.history.iter().map(|r| r.csv_row()).collect::<Vec<_>>()
    };
    // Real economy (plan E2): the bare file lacks `[economy2]` too, so the
    // L2-closing city is `contracts_off().econ_off()` (`--contracts-off
    // --econ-off`); `--contracts-off` alone keeps the market.
    assert_eq!(rows(Config::load().contracts_off().econ_off()), rows(bare));
}

/// Review fix: only a Guard may name a building; a Hit, Beat or Locate on
/// one is refused (it would sit Taken with no plan and no ledger row).
#[test]
fn test_building_target_only_for_guard() {
    let mut w = world();
    let [buyer] = strangers(&w, 1)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    let b = w.buildings_of_kind(BuildingKind::Bar)[0];
    for kind in [ContractKind::Hit, ContractKind::Beat, ContractKind::Locate] {
        let mut p = posting(buyer, kind, buyer, None);
        p.target = Target::Building(b);
        assert_eq!(contracts::post(&mut w, p), Err(citysim::contract::Refusal::NoTarget), "{kind:?}");
    }
    let mut p = posting(buyer, ContractKind::Guard, buyer, None);
    p.target = Target::Building(b);
    assert!(contracts::post(&mut w, p).is_ok(), "a Guard on a building");
}

/// Review fix: a failed attempt takes the departing taker out of
/// `by_party` (the index matches a rebuild from the records).
#[test]
fn test_failed_attempt_unindexes_departing_taker() {
    let mut w = world();
    let [buyer, t, a] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, a);
    set_coins(&mut w, buyer, 5000);
    let f = fixer(&w);
    let id = contracts::post(&mut w, posting(buyer, ContractKind::Beat, t, Some(f))).expect("posted");
    assert!(contracts::accept(&mut w, id, a, &[]));
    assert!(w.by_party.get(&a).is_some_and(|l| l.contains(&id)));
    contracts::fail_attempt(&mut w, id, "test");
    assert_eq!(w.contracts[&id].status, ContractStatus::Open);
    assert!(!w.by_party.get(&a).is_some_and(|l| l.contains(&id)), "the departing taker is unindexed");
    let live = w.by_party.clone();
    contracts::rebuild_index(&mut w);
    assert_eq!(w.by_party, live, "the index equals a rebuild");
    assert_eq!(contracts::escrow_stuck(&w), 0);
}

#[test]
fn test_strike_estimate_worked_numbers() {
    // The plan's Strike table, quote row: the median gun (0.625) against a
    // lone target (0.5): p_win 0.679.
    let mut w = world();
    let [t] = strangers(&w, 1)[..] else { unreachable!() };
    worked_target(&mut w, t);
    let p = missions::estimate(&w, &[missions::MEDIAN], &Target::Agent(t));
    assert!((p - 0.679).abs() < 1e-3, "{p}");
}

/// C5 (plan "Money model"): every phase-1 flow moves coins between purses
/// and escrow only: a brokered post and its fulfilment, a refund, a
/// renege, a direct short pay, bounties from escrow and from the Treasury,
/// and a ledger Hit (the body's loot stays counted): `total_coins` and
/// L2's identity never move, and `escrow_leak` stays 0. (No tick runs, so
/// the documented outside sources, immigrants and emigrants, do not move.)
#[test]
fn test_conservation_over_every_phase_one_flow() {
    let mut w = world();
    let ids = strangers(&w, 9);
    for &a in &ids {
        set_coins(&mut w, a, 3000);
    }
    let (total, id0) = (ownership::total_coins(&w), identity(&w));
    let check = |w: &World, what: &str| {
        assert_eq!(ownership::total_coins(w), total, "total_coins after {what}");
        assert_eq!(identity(w), id0, "the identity after {what}");
        let held: i64 = w.contracts.values().map(|c| c.escrow).sum();
        assert_eq!(held, w.escrow_held, "escrow_leak after {what}");
    };
    let f = fixer(&w);
    // A brokered Beat, fulfilled.
    gun(&mut w, ids[1]);
    let a = contracts::post(&mut w, posting(ids[0], ContractKind::Beat, ids[2], Some(f))).expect("posted");
    check(&w, "a brokered post");
    assert!(contracts::accept(&mut w, a, ids[1], &[]));
    contracts::settle(&mut w, a, contracts::Settle::Fulfilled, "");
    check(&w, "a fulfilment");
    // A brokered Hit refunded.
    let b = contracts::post(&mut w, posting(ids[3], ContractKind::Hit, ids[2], Some(f))).expect("posted");
    contracts::settle(&mut w, b, contracts::Settle::Cancelled, "test");
    check(&w, "a refund");
    // A direct record reneged, another paid short.
    gun(&mut w, ids[4]);
    let c = contracts::post(&mut w, posting(ids[0], ContractKind::Beat, ids[5], None)).expect("posted");
    if let Some(x) = w.contracts.get_mut(&c) {
        x.renege = true;
    }
    assert!(contracts::accept(&mut w, c, ids[4], &[]));
    contracts::settle(&mut w, c, contracts::Settle::Fulfilled, "");
    check(&w, "a renege");
    let d = contracts::post(&mut w, posting(ids[3], ContractKind::Beat, ids[5], None)).expect("posted");
    if let Some(x) = w.contracts.get_mut(&d) {
        x.renege = false;
    }
    set_coins(&mut w, ids[3], 10);
    let total2 = ownership::total_coins(&w);
    let id2 = identity(&w);
    gun(&mut w, ids[6]);
    assert!(contracts::accept(&mut w, d, ids[6], &[]));
    contracts::settle(&mut w, d, contracts::Settle::Fulfilled, "");
    assert_eq!(ownership::total_coins(&w), total2, "a short pay");
    assert_eq!(identity(&w), id2);
    // Bounties: from escrow (brokered) and from the Treasury (public).
    let now = w.tick;
    if let Some(k) = w.comp_mut::<Broker>(f) {
        k.regulars.insert(ids[7], now);
    }
    let mut lp = posting(ids[8], ContractKind::Locate, ids[5], Some(f));
    lp.price = Some(100);
    contracts::post(&mut w, lp).expect("posted");
    let tile = w.comp::<Position>(ids[5]).map(|p| p.tile).unwrap_or_default();
    let mut public = posting(ids[8], ContractKind::Locate, ids[2], None);
    public.buyer = None;
    public.agent = None;
    contracts::post(&mut w, public).expect("posted");
    let t2 = w.comp::<Position>(ids[2]).map(|p| p.tile).unwrap_or_default();
    let (total3, id3) = (ownership::total_coins(&w), identity(&w));
    contracts::on_sighting(&mut w, ids[7], ids[5], tile);
    contracts::on_sighting(&mut w, ids[7], ids[2], t2);
    assert!(w.stats.current.contract.bounties_paid >= 2, "both paid");
    assert_eq!(ownership::total_coins(&w), total3, "bounties");
    assert_eq!(identity(&w), id3);
    let held: i64 = w.contracts.values().map(|c| c.escrow).sum();
    assert_eq!(held, w.escrow_held);
    // A brokered ledger Hit: the target's coins stay counted on its body.
    w.config.missions.strike_k = 100.0;
    let more = strangers(&w, 12);
    let (buyer, t, taker) = (more[9], more[10], more[11]);
    gun(&mut w, taker);
    if let Some(s) = w.comp_mut::<Skills>(t) {
        s.fighting = 0.0;
    }
    set_coins(&mut w, buyer, 5000);
    for x in [t, taker] {
        lod::set_lod(&mut w, x, Lod::Statistical);
    }
    let (total4, id4) = (ownership::total_coins(&w), identity(&w));
    let h = contracts::post(&mut w, posting(buyer, ContractKind::Hit, t, Some(f))).expect("posted");
    assert!(contracts::accept(&mut w, h, taker, &[]));
    contracts::resolve_ledger(&mut w, h);
    assert_eq!(w.contracts[&h].status, ContractStatus::Fulfilled);
    assert!(w.has::<citysim::Corpse>(t));
    assert_eq!(ownership::total_coins(&w), total4, "a ledger Hit");
    assert_eq!(identity(&w), id4);
    let held: i64 = w.contracts.values().map(|c| c.escrow).sum();
    assert_eq!(held, w.escrow_held, "escrow_leak after a ledger Hit");
}
