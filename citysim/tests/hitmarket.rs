//! The Hit market (docs/HIT_MARKET.md; plan `~/.claude/plans/hit-market.md`)
//! phase 1: the price (a gun's ask, the market price and its fallback), the
//! means (kin who share the grudge chip in or refuse), credit (half down,
//! the balance at fulfilment, the stiffed buyer's debt and the gun's
//! grudge), the brakes, and a buyer who keeps his own Hit quiet. Every
//! record, Hit and Beat here is game state between fictional agents: a
//! struct with a price, matched by a score, resolved by a seeded dice roll.

use citysim::contract::{ContractKind, ContractStatus, Motive, Origin, PostExtra, Posting, Target};
use citysim::systems::econ::identity;
use citysim::systems::{contracts, demography, grudges, hitmarket, law, ownership};
use citysim::word::{GrudgeCause, Grudges};
use citysim::{Config, EntityId, EventKind, GangMember, Personality, RelKind, Skills, World};

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

/// A lawless, brave, strong, jobless gun.
fn gun(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.lawfulness = 0.0;
        p.courage = 0.9;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 1.0;
    }
    w.vacate_job(id);
}

fn set_grudge(w: &mut World, h: EntityId, t: EntityId, weight: f32) {
    grudges::add(w, h, t, GrudgeCause::KilledKin(t), weight, 0);
    if let Some(g) = w.comp_mut::<Grudges>(h) {
        if let Some(x) = g.list.iter_mut().find(|x| x.target == t) {
            x.weight = weight;
        }
    }
}

/// A coward (H12) with a lethal grudge on a stranger, broke.
fn coward_holder(w: &mut World) -> (EntityId, EntityId) {
    let [h, t] = strangers(w, 2)[..] else { unreachable!() };
    if let Some(p) = w.comp_mut::<Personality>(h) {
        p.courage = 0.1;
        p.lawfulness = 0.2;
    }
    set_grudge(w, h, t, 0.95);
    set_coins(w, h, 0);
    (h, t)
}

/// A kin of `h` who shares the grudge on `t`, loyal and lawless (chips in).
fn sharing_kin(w: &mut World, h: EntityId, t: EntityId, coins_: i64) -> EntityId {
    let k = adults(w)
        .into_iter()
        .find(|&a| a != h && a != t && w.edge(h, a).is_none() && w.edge(t, a).is_none() && !w.has::<GangMember>(a))
        .expect("a kin");
    {
        let e = w.edge_entry(h, k);
        e.kind = RelKind::Family;
        e.affinity = 1.0;
    }
    if let Some(p) = w.comp_mut::<Personality>(k) {
        p.lawfulness = 0.0;
    }
    set_grudge(w, k, t, 0.6);
    set_coins(w, k, coins_);
    k
}

fn brokered(buyer: EntityId, kind: ContractKind, t: EntityId, broker: EntityId, price: i64) -> Posting {
    Posting {
        buyer: Some(buyer),
        agent: Some(buyer),
        kind,
        target: Target::Agent(t),
        broker: Some(broker),
        deadline_days: 10,
        origin: Origin::Hunt,
        price: Some(price),
    }
}

/// H2: a gun's ask is the exact boundary of matching's acceptance score.
#[test]
fn test_ask_is_the_acceptance_boundary() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 10_000);
    let f = contracts::open_fixers(&w)[0];
    let tgt = Target::Agent(t);
    let ask = contracts::ask(&w, g, ContractKind::Hit, &tgt, true, Some(buyer)).expect("a gun has a price");
    assert!(ask > 1, "a Hit is not free: {ask}");
    let min = w.config.contracts.accept_min;
    let id = contracts::post(&mut w, brokered(buyer, ContractKind::Hit, t, f, ask)).expect("posted");
    assert!(contracts::accept_score(&w, g, id) >= min, "accepts at the ask");
    contracts::settle(&mut w, id, contracts::Settle::Cancelled, "test");
    let id = contracts::post(&mut w, brokered(buyer, ContractKind::Hit, t, f, ask - 1)).expect("posted");
    assert!(contracts::accept_score(&w, g, id) < min, "refuses a coin under it");
    // A lawful coward will not at any price.
    let [p] = strangers(&w, 1)[..] else { unreachable!() };
    if let Some(x) = w.comp_mut::<Personality>(p) {
        x.lawfulness = 1.0;
        x.courage = 0.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(p) {
        s.fighting = 0.0;
    }
    assert_eq!(contracts::ask(&w, p, ContractKind::Hit, &tgt, true, Some(buyer)), None);
}

/// H1: the market price is the `ask_rank`-th cheapest ask of the broker's
/// eligible regulars, at least the floor; too few asks fall back to `quote`.
#[test]
fn test_market_price_rank_floor_and_fallback() {
    let mut w = world();
    let [buyer, t] = strangers(&w, 2)[..] else { unreachable!() };
    let f = contracts::open_fixers(&w)[0];
    let tgt = Target::Agent(t);
    let regulars: Vec<EntityId> =
        w.comp::<citysim::contract::Broker>(f).map(|k| k.regulars.keys().copied().collect()).unwrap_or_default();
    let probe = |w: &World| contracts::market_price(w, ContractKind::Hit, &tgt, Some(buyer), Some(buyer), Some(f));
    let p = probe(&w);
    let mut asks: Vec<i64> = regulars
        .iter()
        .filter(|&&a| a != t && a != buyer && contracts::free_adult(&w, a) && !law::is_guard(&w, a))
        .filter(|&&a| w.comp::<Personality>(a).is_some_and(|x| x.lawfulness <= w.config.fixers.gun_lawfulness.hit))
        .filter(|&&a| !contracts::holds_taken(&w, a))
        .filter(|&&a| {
            !w.edge(a, t).is_some_and(|e| {
                matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend)
            })
        })
        .filter_map(|&a| contracts::ask(&w, a, ContractKind::Hit, &tgt, true, Some(buyer)))
        .collect();
    asks.sort_unstable();
    let rank = w.config.hitmarket.ask_rank;
    if asks.len() >= rank {
        assert_eq!(p, Some(asks[rank - 1].max(w.config.hitmarket.price_floor.hit)));
    } else {
        assert_eq!(p, None);
    }
    // The floor binds.
    w.config.hitmarket.price_floor.hit = 1_000;
    if p.is_some() {
        assert!(probe(&w).is_some_and(|x| x >= 1_000));
    }
    // Nobody ranks that deep: `quote`.
    w.config.hitmarket.ask_rank = 100_000;
    assert_eq!(probe(&w), None);
    assert_eq!(
        contracts::organic_price(&w, ContractKind::Hit, &tgt, Some(buyer), Some(buyer), Some(f)),
        contracts::quote(&w, ContractKind::Hit, &tgt, Some(f))
    );
}

/// Decision 6: loyalty raises the chance to chip in, lawfulness lowers it.
#[test]
fn test_chip_p_reads_loyalty_and_lawfulness() {
    assert_eq!(hitmarket::chip_p(1.0, 0.0), 1.0);
    assert_eq!(hitmarket::chip_p(-1.0, 0.0), 0.0);
    assert!(hitmarket::chip_p(0.5, 0.2) > hitmarket::chip_p(0.5, 0.9));
    assert!(hitmarket::chip_p(0.8, 0.5) > hitmarket::chip_p(0.0, 0.5));
}

/// H3-H5, H12: a broke coward with a lethal grudge buys a Hit at the market
/// price with half down, the half from a relative who shares the grudge;
/// the relative knows; no coin is made or lost.
#[test]
fn test_revenge_pooled_from_kin_half_down() {
    let mut w = world();
    let (h, t) = coward_holder(&mut w);
    let k = sharing_kin(&mut w, h, t, 1_000);
    assert!(hitmarket::hires(&w, h, t), "a coward hires");
    let id0 = identity(&w);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95), "posted");
    let c = w.contracts.values().find(|c| c.buyer == Some(h)).expect("a record");
    assert_eq!((c.kind, c.motive, c.origin), (ContractKind::Hit, Motive::Revenge, Origin::Hunt));
    assert_eq!(c.deposit, hitmarket::deposit_of(&w, c.price), "half down");
    assert!(c.deposit < c.price);
    assert_eq!(coins(&w, k), 1_000 - c.deposit, "the kin put in the deposit");
    // Brokered or direct (P2 fix round), the deposit is held on the record.
    assert_eq!(c.escrow, c.deposit, "only the deposit is escrowed");
    assert_eq!(coins(&w, h), 0);
    assert!(c.known_by.contains(&k), "who paid knows");
    assert_eq!(identity(&w), id0, "the coins are conserved");
    assert!(w.stats.current.hit.hm_posted_revenge == 1);
}

/// H3: short of the deposit with no kin, nothing moves.
#[test]
fn test_revenge_short_of_deposit_posts_nothing() {
    let mut w = world();
    let (h, t) = coward_holder(&mut w);
    let k = sharing_kin(&mut w, h, t, 0);
    let before = w.contracts.len();
    assert!(!hitmarket::try_revenge(&mut w, h, t, 0.95));
    assert_eq!(w.contracts.len(), before);
    assert_eq!(coins(&w, k), 0);
    assert_eq!(w.stats.current.hit.hm_unaffordable, 1);
}

/// Decision 6: a relative who refuses (no loyalty) puts nothing in.
#[test]
fn test_disloyal_kin_refuses() {
    let mut w = world();
    let (h, t) = coward_holder(&mut w);
    let k = sharing_kin(&mut w, h, t, 1_000);
    w.edge_entry(h, k).affinity = -1.0;
    assert!(!hitmarket::try_revenge(&mut w, h, t, 0.95));
    assert_eq!(coins(&w, k), 1_000);
    assert_eq!(w.stats.current.hit.hm_refused, 1);
}

/// H11: no room under the brakes, no post.
#[test]
fn test_brakes_hold() {
    let mut w = world();
    let (h, t) = coward_holder(&mut w);
    set_coins(&mut w, h, 10_000);
    w.config.hitmarket.posts_per_day = 0;
    assert!(!hitmarket::try_revenge(&mut w, h, t, 0.95));
    w.config.hitmarket.posts_per_day = 3;
    w.config.hitmarket.max_open = 0;
    assert!(!hitmarket::try_revenge(&mut w, h, t, 0.95));
    w.config.hitmarket.max_open = 16;
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
}

/// D8 (phase 4 fix, from the buyers' diaries): a buyer does not hunt the
/// head it is paying for while its want or its record stands, even with the
/// Hunt's cooldown gone; once the record closes the Hunt is open again.
#[test]
fn test_buyer_does_not_hunt_the_head_it_paid_for() {
    use citysim::systems::hunt;
    use citysim::word::HuntWhy;
    let mut w = world();
    let (h, t) = coward_holder(&mut w);
    set_coins(&mut w, h, 1_000);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95), "posted or wanted");
    assert!(hitmarket::paying_for(&w, h, t));
    // The Hunt's cooldown (D8's) runs out; the record or the want stands.
    let cool = |w: &mut World| {
        if let Some(b) = w.comp_mut::<citysim::Brain>(h) {
            b.cooldowns.clear();
        }
    };
    cool(&mut w);
    assert!(hunt::considerations(&w, h).is_none(), "no Hunt scored on a paid head");
    assert!(!hunt::adopt(&mut w, h, HuntWhy::Goal), "no Hunt adopted on a paid head");
    assert!(!w.hunts.contains_key(&h));
    // Control: the want lapses and the record closes; the Hunt is open again.
    w.wants.remove(&h);
    for c in w.contracts.values_mut().filter(|c| c.buyer == Some(h)) {
        c.status = ContractStatus::Expired;
    }
    cool(&mut w);
    assert!(!hitmarket::paying_for(&w, h, t));
    assert!(hunt::considerations(&w, h).is_some(), "the Hunt scores once the record is gone");
}

/// H12: a brave, strong holder hunts himself (no hire).
#[test]
fn test_brave_strong_holder_does_not_hire() {
    let mut w = world();
    let (h, t) = coward_holder(&mut w);
    if let Some(p) = w.comp_mut::<Personality>(h) {
        p.courage = 0.9;
    }
    if let Some(s) = w.comp_mut::<Skills>(h) {
        s.fighting = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(t) {
        s.fighting = 0.0;
    }
    assert!(!hitmarket::hires(&w, h, t));
}

/// A brokered credit record on `t`, taken by gun `g`.
fn credit_record(w: &mut World, buyer: EntityId, t: EntityId, g: EntityId, price: i64) -> u64 {
    let f = contracts::open_fixers(w)[0];
    let extra = PostExtra { deposit: Some(price / 2), motive: Motive::Revenge, why: None };
    let id = contracts::post_with(w, brokered(buyer, ContractKind::Hit, t, f, price), extra).expect("posted");
    assert!(contracts::accept(w, id, g, &[]));
    id
}

/// H6: a buyer who cannot pay the balance: the job stands done, the gun is
/// paid what there is, the rest is an edge debt and a grudge; `Stiffed`.
#[test]
fn test_stiffed_buyer_owes_and_is_hated() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 50);
    set_coins(&mut w, g, 0);
    let id0 = identity(&w);
    let id = credit_record(&mut w, buyer, t, g, 100);
    assert_eq!(coins(&w, buyer), 0, "the deposit is down");
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    let c = &w.contracts[&id];
    assert_eq!(c.status, ContractStatus::Fulfilled);
    assert_eq!(c.escrow, 0);
    let cut = (50.0 * w.config.fixers.fixer_cut).round() as i64;
    assert_eq!(coins(&w, g), 50 - cut, "the deposit less the cut");
    // The lower id owes the higher id when `debt > 0`.
    let e = w.edge(buyer, g).expect("an edge");
    let owed = if buyer < g { e.debt } else { -e.debt };
    assert_eq!(owed, 50, "the balance is owed");
    let gr = grudges::grudge_on(&w, g, buyer).expect("the gun's grudge");
    // The first-hand `Betrayed` deed forms its own grudge first; the stiff
    // weight merges into it (`1 − (1 − a)(1 − b)`).
    assert!(gr.weight >= w.config.hitmarket.stiff_grudge - 1e-4, "{}", gr.weight);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Stiffed));
    assert_eq!(w.stats.current.hit.hm_stiffed, 1);
    assert_eq!(identity(&w), id0);
    assert_eq!(ownership::total_coins(&w) - w.escrow_held, ownership::total_coins(&w));
}

/// H6: a buyer who can pay the balance pays it all; nobody is stiffed.
#[test]
fn test_credit_buyer_pays_balance() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 500);
    set_coins(&mut w, g, 0);
    let id = credit_record(&mut w, buyer, t, g, 100);
    assert_eq!(coins(&w, buyer), 450);
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(coins(&w, buyer), 400, "the balance charged");
    let cut = (100.0 * w.config.fixers.fixer_cut).round() as i64;
    assert_eq!(coins(&w, g), 100 - cut);
    assert!(!w.events.iter().any(|e| e.kind == EventKind::Stiffed));
}

/// P5: a direct record on credit whose buyer is short is fulfilled and
/// stiffed, not reneged.
#[test]
fn test_direct_credit_short_is_stiffed_not_reneged() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    w.edge_entry(buyer, g).kind = RelKind::Friend;
    set_coins(&mut w, buyer, 60);
    let mut p = brokered(buyer, ContractKind::Beat, t, EntityId::NONE, 100);
    p.broker = None;
    let extra = PostExtra { deposit: Some(50), motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, p, extra).expect("posted");
    w.contracts.get_mut(&id).unwrap().renege = false;
    assert_eq!(coins(&w, buyer), 10, "the deposit is held on the record (P2 fix round)");
    assert_eq!(w.contracts[&id].escrow, 50);
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    assert_eq!(coins(&w, buyer), 0);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Stiffed));
}

/// H4 (P2 fix round): a direct buyer on credit cannot spend the deposit
/// before fulfilment: it is held on the record; the gun gets the deposit
/// whatever the buyer did with the rest of the purse, and only the balance
/// is owed. The coin identity holds.
#[test]
fn test_direct_deposit_is_held_not_spent() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    w.edge_entry(buyer, g).kind = RelKind::Friend;
    set_coins(&mut w, buyer, 50);
    set_coins(&mut w, g, 0);
    let id0 = identity(&w);
    let mut p = brokered(buyer, ContractKind::Beat, t, EntityId::NONE, 100);
    p.broker = None;
    let extra = PostExtra { deposit: Some(50), motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, p, extra).expect("posted");
    w.contracts.get_mut(&id).unwrap().renege = false;
    // The buyer spends everything he holds before the job is done.
    assert_eq!(coins(&w, buyer), 0, "the deposit left the purse at posting");
    assert_eq!(w.contracts[&id].escrow, 50);
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(coins(&w, g), 50, "the gun is paid the deposit");
    let e = w.edge(buyer, g).expect("an edge");
    let owed = if buyer < g { e.debt } else { -e.debt };
    assert_eq!(owed, 50, "only the balance is owed");
    assert_eq!(w.contracts[&id].escrow, 0);
    assert_eq!(identity(&w), id0);
    // Expired instead: the deposit goes back.
    let mut w = world();
    let [buyer, t, _] = strangers(&w, 3)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 50);
    let mut p = brokered(buyer, ContractKind::Beat, t, EntityId::NONE, 100);
    p.broker = None;
    let extra = PostExtra { deposit: Some(50), motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, p, extra).expect("posted");
    contracts::settle(&mut w, id, contracts::Settle::Expired, "test");
    assert_eq!(coins(&w, buyer), 50, "refunded");
}

/// A bought revenge beating answers the grudge (`beat_settle`), so the
/// buyer does not buy the same beating again.
#[test]
fn test_bought_beating_settles_part_of_the_grudge() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    set_grudge(&mut w, buyer, t, 0.8);
    set_coins(&mut w, buyer, 500);
    let f = contracts::open_fixers(&w)[0];
    let extra = PostExtra { deposit: None, motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, brokered(buyer, ContractKind::Beat, t, f, 40), extra).expect("posted");
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    let left = grudges::grudge_on(&w, buyer, t).map_or(0.0, |x| x.weight);
    assert!((left - (0.8 - w.config.grudges.beat_settle)).abs() < 1e-4, "{left}");
}

/// H20: with the market off, `try_hire` keeps M16a's price and rule.
#[test]
fn test_market_off_keeps_m16a_hire() {
    let mut w = world();
    w.config.hitmarket.enabled = false;
    let (h, t) = coward_holder(&mut w);
    let k = sharing_kin(&mut w, h, t, 1_000);
    // A coward who is not weaker: M16a does not hire.
    if let Some(s) = w.comp_mut::<Skills>(h) {
        s.fighting = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(t) {
        s.fighting = 0.0;
    }
    assert!(!hitmarket::hires(&w, h, t));
    assert!(!contracts::try_hire(&mut w, h, t, 0.95));
    assert_eq!(coins(&w, k), 1_000);
}

// ---------------------------------------------------------------------------
// Phase 3: the gangs' and corps' business (H18, H19, decision 4)
// ---------------------------------------------------------------------------

fn set_gang_treasury(w: &mut World, g: EntityId, coins_: i64) {
    if let Some(x) = w.comp_mut::<citysim::Gang>(g) {
        x.treasury = coins_;
    }
}

fn set_corp_treasury(w: &mut World, c: EntityId, coins_: i64) {
    if let Some(x) = w.comp_mut::<citysim::Corp>(c) {
        x.treasury = coins_;
    }
}

/// The first two gangs, each staffed with three strangers and led.
fn led_gangs(w: &mut World) -> Vec<EntityId> {
    use citysim::systems::gang;
    let gangs = w.gangs();
    let (a, b) = (gangs[0], gangs[1]);
    let people = strangers(w, 6);
    for (i, &m) in people.iter().enumerate() {
        gang::enlist(w, m, if i < 3 { a } else { b });
    }
    for g in [a, b] {
        gang::recompute_leader(w, g);
    }
    vec![a, b]
}

/// Two gangs with living leaders, in an open vendetta.
fn feuding_gangs(w: &mut World) -> (EntityId, EntityId) {
    let gangs = led_gangs(w);
    let (a, b) = (gangs[0], gangs[1]);
    let now = w.tick;
    w.vendettas.push(citysim::word::Vendetta { a, b, since: now, kills: [0, 0], w: [1.0, 1.0], declared: None });
    (a, b)
}

fn organic_by(w: &World, buyer: EntityId) -> Vec<citysim::contract::Contract> {
    w.contracts.values().filter(|c| c.buyer == Some(buyer) && !c.motive.is_none()).cloned().collect()
}

/// H19: a gang in a vendetta with money buys the rival leader's death.
#[test]
fn test_gang_buys_rival_leader_in_a_feud() {
    let mut w = world();
    let (a, b) = feuding_gangs(&mut w);
    set_gang_treasury(&mut w, a, 100_000);
    hitmarket::gang_pass(&mut w);
    let lb = w.comp::<citysim::Gang>(b).and_then(|x| x.leader).unwrap();
    let mine = organic_by(&w, a);
    assert_eq!(mine.len(), 1, "one head a day");
    assert_eq!((mine[0].kind, mine[0].target, mine[0].motive), (ContractKind::Hit, Target::Agent(lb), Motive::Rival));
    assert_eq!(mine[0].deposit, 0, "a faction pays in full");
    // The next day: already bought.
    hitmarket::gang_pass(&mut w);
    assert_eq!(organic_by(&w, a).len(), 1);
}

/// H19: a broke gang buys nothing.
#[test]
fn test_broke_gang_buys_nothing() {
    let mut w = world();
    let (a, _) = feuding_gangs(&mut w);
    set_gang_treasury(&mut w, a, 0);
    hitmarket::gang_pass(&mut w);
    assert!(organic_by(&w, a).is_empty());
}

/// H19: a gang pays for its leader's revenge on an outsider.
#[test]
fn test_gang_pays_for_its_leaders_revenge() {
    let mut w = world();
    let g = led_gangs(&mut w)[0];
    let leader = w.comp::<citysim::Gang>(g).and_then(|x| x.leader).unwrap();
    let t = adults(&w).into_iter().find(|&a| !w.has::<GangMember>(a) && a != leader && !law::is_guard(&w, a)).unwrap();
    set_grudge(&mut w, leader, t, 0.95);
    set_gang_treasury(&mut w, g, 100_000);
    hitmarket::gang_pass(&mut w);
    let mine = organic_by(&w, g);
    assert_eq!(mine.len(), 1);
    assert_eq!((mine[0].kind, mine[0].target, mine[0].motive), (ContractKind::Hit, Target::Agent(t), Motive::Revenge));
    assert_eq!(mine[0].agent, Some(leader));
}

/// Two corps with living execs.
fn two_corps(w: &World) -> (EntityId, EntityId, EntityId) {
    let cs: Vec<EntityId> = w
        .corps()
        .into_iter()
        .filter(|&c| w.comp::<citysim::Corp>(c).and_then(|x| x.exec).is_some_and(|e| law::living(w, e)))
        .collect();
    (cs[0], cs[1], cs[2])
}

fn set_exec_law(w: &mut World, c: EntityId, law_: f32) {
    let e = w.comp::<citysim::Corp>(c).and_then(|x| x.exec).unwrap();
    if let Some(p) = w.comp_mut::<Personality>(e) {
        p.lawfulness = law_;
    }
}

/// H18: a dirty exec wronged by a rival buys the rival exec's death; the
/// cooldown holds; a clean exec buys nothing.
#[test]
fn test_dirty_corp_buys_rival_exec_after_a_wrong() {
    let mut w = world();
    let (c, r, r2) = two_corps(&w);
    set_corp_treasury(&mut w, c, 1_000_000);
    set_exec_law(&mut w, c, 0.9);
    hitmarket::wronged(&mut w, c, r, citysim::contract::Wrong::Acquired);
    hitmarket::corp_pass(&mut w);
    assert!(organic_by(&w, c).is_empty(), "a clean exec buys nothing");
    set_exec_law(&mut w, c, 0.1);
    hitmarket::corp_pass(&mut w);
    let er = w.comp::<citysim::Corp>(r).and_then(|x| x.exec).unwrap();
    let mine = organic_by(&w, c);
    assert_eq!(mine.len(), 1);
    assert_eq!((mine[0].kind, mine[0].target, mine[0].motive), (ContractKind::Hit, Target::Agent(er), Motive::Rival));
    assert_eq!(w.hm_corp_hits.len(), 1);
    // The cooldown: a second wrong a day later buys nothing more.
    w.tick += citysim::TICKS_PER_DAY;
    hitmarket::wronged(&mut w, c, r2, citysim::contract::Wrong::Poached);
    hitmarket::corp_pass(&mut w);
    assert_eq!(organic_by(&w, c).len(), 1);
}

/// Decision 4: past the city-wide cap no corp buys an exec's death; an
/// exec between `dirty_lobby` and `dirty_beat` buys a beating instead.
#[test]
fn test_corp_hit_year_cap_and_beat_band() {
    let mut w = world();
    let (c, r, _) = two_corps(&w);
    set_corp_treasury(&mut w, c, 1_000_000);
    set_exec_law(&mut w, c, 0.1);
    w.config.hitmarket.corp_year_cap = 0;
    hitmarket::wronged(&mut w, c, r, citysim::contract::Wrong::Undercut);
    hitmarket::corp_pass(&mut w);
    assert!(organic_by(&w, c).is_empty(), "the cap holds");
    set_exec_law(&mut w, c, 0.35);
    hitmarket::corp_pass(&mut w);
    let mine = organic_by(&w, c);
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].kind, ContractKind::Beat);
}

/// H18: a wrong older than `rival_window_days` is forgotten.
#[test]
fn test_old_wrong_is_forgotten() {
    let mut w = world();
    let (c, r, _) = two_corps(&w);
    set_corp_treasury(&mut w, c, 1_000_000);
    set_exec_law(&mut w, c, 0.1);
    hitmarket::wronged(&mut w, c, r, citysim::contract::Wrong::Acquired);
    w.tick += u64::from(w.config.hitmarket.rival_window_days + 1) * citysim::TICKS_PER_DAY;
    hitmarket::corp_pass(&mut w);
    assert!(organic_by(&w, c).is_empty());
}

/// H18: Lobby's culprit record is priced at the market and labelled.
#[test]
fn test_lobby_culprit_at_market_price() {
    let mut w = world();
    let (c, _, _) = two_corps(&w);
    set_corp_treasury(&mut w, c, 1_000_000);
    set_exec_law(&mut w, c, 0.1);
    let g = led_gangs(&mut w)[0];
    let lg = w.comp::<citysim::Gang>(g).and_then(|x| x.leader).unwrap();
    let f = contracts::open_fixers(&w)[0];
    let at = w.comp::<citysim::Position>(lg).map(|p| p.tile).unwrap();
    let _ = at;
    let kind = ContractKind::Hit;
    let expect = contracts::organic_price(
        &w,
        kind,
        &Target::Agent(lg),
        Some(c),
        w.comp::<citysim::Corp>(c).and_then(|x| x.exec),
        Some(f),
    );
    assert!(contracts::post_lobby(&mut w, c, g));
    let rec = w.contracts.values().find(|x| x.buyer == Some(c)).cloned().unwrap();
    assert_eq!(rec.motive, Motive::Culprit);
    assert_eq!(rec.kind, kind);
    if rec.broker == Some(f) {
        assert_eq!(rec.price, expect, "priced at the market (or quote when nobody asks)");
    }
}

/// P3 follow-up: Lobby waits `repost_days` before buying the same head
/// again (the bribe goes ahead meanwhile).
#[test]
fn test_lobby_waits_before_rebuying_the_same_head() {
    let mut w = world();
    let (c, _, _) = two_corps(&w);
    set_corp_treasury(&mut w, c, 1_000_000);
    set_exec_law(&mut w, c, 0.1);
    let g = led_gangs(&mut w)[0];
    assert!(contracts::post_lobby(&mut w, c, g));
    let id = w.contracts.values().find(|x| x.buyer == Some(c)).map(|x| x.id).unwrap();
    assert!(contracts::post_lobby(&mut w, c, g), "a live record stands");
    contracts::settle(&mut w, id, contracts::Settle::Failed, "test");
    let n = w.contracts.len();
    assert!(!contracts::post_lobby(&mut w, c, g), "inside the wait: no record, the bribe");
    assert_eq!(w.contracts.len(), n);
    w.tick += u64::from(w.config.hitmarket.repost_days + 1) * citysim::TICKS_PER_DAY;
    assert!(contracts::post_lobby(&mut w, c, g), "after the wait: bought again");
    assert_eq!(w.contracts.len(), n + 1);
}

/// P3 follow-up: an exec's expected defenders count up to `exec_guards`
/// private guards of the corps protecting its corp (a Security corp's
/// own); none with the market off.
#[test]
fn test_exec_defended_by_corp_guards() {
    use citysim::systems::missions;
    let mut w = world();
    // The private guards are hired over the first days.
    w.run_ticks(3 * citysim::TICKS_PER_DAY);
    let sec = w
        .corps()
        .into_iter()
        .find(|&c| {
            w.comp::<citysim::Corp>(c).is_some_and(|x| {
                x.niches.contains(&citysim::Niche::Security) && x.exec.is_some_and(|e| law::living(&w, e))
            })
        })
        .expect("a Security corp with an exec");
    let e = w.comp::<citysim::Corp>(sec).and_then(|x| x.exec).unwrap();
    let guards = missions::exec_guards(&w, e);
    assert!(!guards.is_empty() && guards.len() <= w.config.hitmarket.exec_guards, "{}", guards.len());
    let t = Target::Agent(e);
    let defs = missions::expected_defenders(&w, &t);
    assert!(guards.iter().all(|g| defs.contains(g)));
    let s_on = missions::defender_strength(&w, &t);
    w.config.hitmarket.enabled = false;
    assert!(missions::exec_guards(&w, e).is_empty());
    assert!(missions::defender_strength(&w, &t) < s_on);
    // A nobody has none.
    w.config.hitmarket.enabled = true;
    let [p] = strangers(&w, 1)[..] else { unreachable!() };
    assert!(missions::exec_guards(&w, p).is_empty());
}

// ---------------------------------------------------------------------------
// Phase 2: the walk to the Fixer (H9), Silence (H16), Debt, Protect (D12),
// cold cases (H14), recanting (H15) and the buyer after (H17)
// ---------------------------------------------------------------------------

fn set_lod(w: &mut World, id: EntityId, lod: citysim::Lod) {
    if let Some(b) = w.comp_mut::<citysim::Brain>(id) {
        b.lod = lod;
    }
}

fn set_law(w: &mut World, id: EntityId, lawfulness: f32, courage: f32) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.lawfulness = lawfulness;
        p.courage = courage;
    }
}

fn open_report(w: &World, suspect: EntityId, witness: EntityId) -> bool {
    w.crime_reports().iter().any(|r| !r.resolved && r.suspect == suspect && r.witness == Some(witness))
}

fn day(w: &mut World, days: u64) {
    w.tick += days * citysim::TICKS_PER_DAY;
}

/// A lawless suspect (Statistical: posts at once) with an open `crime`
/// report filed by `wit`, rich; `known` gives them an edge (H16).
fn suspect_and_witness(w: &mut World, crime: citysim::Crime, known: bool) -> (EntityId, EntityId) {
    let [s, wit] = strangers(w, 2)[..] else { unreachable!() };
    set_law(w, s, 0.1, 0.5);
    set_lod(w, s, citysim::Lod::Statistical);
    set_coins(w, s, 10_000);
    if known {
        w.edge_entry(s, wit).kind = RelKind::Acquaintance;
    }
    law::file_report(w, crime, s, Some(wit));
    (s, wit)
}

/// Silence (H16): a lawless suspect who knows the witness to his assault
/// buys a Beat on her; one who does not know her buys nothing; a lawful one
/// buys nothing.
#[test]
fn test_silence_buys_a_known_witness_only() {
    let mut w = world();
    let (s, wit) = suspect_and_witness(&mut w, citysim::Crime::Assault, true);
    hitmarket::silence_pass(&mut w);
    let mine = organic_by(&w, s);
    assert_eq!(mine.len(), 1, "the suspect bought the witness");
    assert_eq!(
        (mine[0].kind, mine[0].target, mine[0].motive),
        (ContractKind::Beat, Target::Agent(wit), Motive::Silence)
    );
    // Control: an unknown witness is never targeted.
    let mut w = world();
    let (s, _) = suspect_and_witness(&mut w, citysim::Crime::Assault, false);
    hitmarket::silence_pass(&mut w);
    assert!(organic_by(&w, s).is_empty(), "an unknown witness was targeted");
    // Control: a lawful suspect does not buy.
    let mut w = world();
    let (s, _) = suspect_and_witness(&mut w, citysim::Crime::Assault, true);
    set_law(&mut w, s, 0.9, 0.5);
    hitmarket::silence_pass(&mut w);
    assert!(organic_by(&w, s).is_empty(), "a lawful suspect bought a beating");
}

/// Silence: a Murder's witness is bought dead (a Hit).
#[test]
fn test_silence_on_a_murder_is_a_hit() {
    let mut w = world();
    let (s, wit) = suspect_and_witness(&mut w, citysim::Crime::Murder, true);
    hitmarket::silence_pass(&mut w);
    let mine = organic_by(&w, s);
    assert_eq!(mine.len(), 1);
    assert_eq!((mine[0].kind, mine[0].target), (ContractKind::Hit, Target::Agent(wit)));
}

/// H16 (D9): the street tells: a gang member knows a witness who lives in a
/// district his gang holds, and not one who lives elsewhere.
#[test]
fn test_gang_member_knows_witness_in_held_district() {
    let mut w = world();
    let g = led_gangs(&mut w)[0];
    let m = w.comp::<citysim::Gang>(g).map(|x| x.members[0]).unwrap();
    let wit = adults(&w)
        .into_iter()
        .find(|&a| a != m && w.edge(m, a).is_none() && w.comp::<citysim::Household>(a).and_then(|h| h.home).is_some())
        .expect("a housed stranger");
    let home = w.comp::<citysim::Household>(wit).and_then(|h| h.home).unwrap();
    if let Some(x) = w.comp_mut::<citysim::Gang>(g) {
        x.territory.retain(|&b| b != home);
    }
    let held = |w: &World| {
        citysim::systems::gang::held_districts(w, g).iter().any(|&(d, _)| d == w.district_of_building(home))
    };
    if !held(&w) {
        assert!(!hitmarket::knows_witness(&w, m, wit), "a stranger elsewhere is unknown");
    }
    if let Some(x) = w.comp_mut::<citysim::Gang>(g) {
        x.territory.push(home);
    }
    assert!(hitmarket::knows_witness(&w, m, wit), "the street tells");
}

/// Debt from an edge: a lawless creditor owed `debt_min` buys nothing until
/// the debt is `debt_days` old, then a Beat on the debtor; a Debt Beat done
/// starts the Hit's clock: `debt_hit_days` later, the debt standing, a Hit.
/// A lawful creditor buys nothing.
#[test]
fn test_debt_beat_then_hit() {
    let run = |lawful: bool| -> (World, EntityId, EntityId) {
        let mut w = world();
        let [c, d, g] = strangers(&w, 3)[..] else { unreachable!() };
        set_law(&mut w, c, if lawful { 0.9 } else { 0.1 }, 0.5);
        set_lod(&mut w, c, citysim::Lod::Statistical);
        set_coins(&mut w, c, 10_000);
        gun(&mut w, g);
        let owed = w.config.hitmarket.debt_min;
        let (lo, _) = citysim::edge_key(c, d);
        let e = w.edge_entry(c, d);
        e.debt = if lo == d { owed } else { -owed };
        hitmarket::debt_pass(&mut w);
        assert!(organic_by(&w, c).is_empty(), "a fresh debt is no motive");
        let days = u64::from(w.config.hitmarket.debt_days);
        day(&mut w, days);
        hitmarket::debt_pass(&mut w);
        (w, c, d)
    };
    let (mut w, c2, d) = run(false);
    let mine = organic_by(&w, c2);
    assert_eq!(mine.len(), 1, "the creditor bought a beating");
    assert_eq!((mine[0].kind, mine[0].target, mine[0].motive), (ContractKind::Beat, Target::Agent(d), Motive::Debt));
    // The Beat is done: the clock starts; the record closes.
    let beat = mine[0].clone();
    hitmarket::on_fulfilled(&mut w, &beat);
    contracts::settle(&mut w, beat.id, contracts::Settle::Cancelled, "test");
    assert!(w.hm_debts[&(c2, d)].beat.is_some());
    hitmarket::debt_pass(&mut w);
    assert_eq!(organic_by(&w, c2).len(), 1, "no Hit before debt_hit_days");
    let days = u64::from(w.config.hitmarket.debt_hit_days);
    day(&mut w, days);
    hitmarket::debt_pass(&mut w);
    let hit: Vec<_> = organic_by(&w, c2).into_iter().filter(|x| x.is_live()).collect();
    assert_eq!(hit.len(), 1);
    assert_eq!((hit[0].kind, hit[0].target), (ContractKind::Hit, Target::Agent(d)));
    // Control: the lawful creditor.
    let (w, c, _) = run(true);
    assert!(organic_by(&w, c).is_empty(), "a lawful creditor bought a beating");
}

/// Debt from a finance plan: an agent lender whose asset sits on its owner
/// in arrears past `repo_days` buys a Beat on the owner after `debt_days`.
#[test]
fn test_debt_from_finance_arrears() {
    use citysim::systems::assets;
    let mut w = world();
    let [l, o] = strangers(&w, 2)[..] else { unreachable!() };
    set_law(&mut w, l, 0.1, 0.5);
    set_lod(&mut w, l, citysim::Lod::Statistical);
    set_coins(&mut w, l, 10_000);
    let spot = contracts::open_fixers(&w)[0];
    let a =
        assets::spawn_asset(&mut w, citysim::AssetKind::Motorcycle, 1, Some(o), citysim::AssetLoc::Parked(spot), 300);
    let repo = w.config.assets.repo_days;
    if let Some(x) = w.comp_mut::<citysim::Asset>(a) {
        x.finance = Some(citysim::Finance { lender: Some(l), remaining: 100, per_day: 5, arrears: repo });
    }
    hitmarket::debt_pass(&mut w);
    assert!(organic_by(&w, l).is_empty());
    let days = u64::from(w.config.hitmarket.debt_days);
    day(&mut w, days);
    hitmarket::debt_pass(&mut w);
    let mine = organic_by(&w, l);
    assert_eq!(mine.len(), 1, "the lender bought a beating");
    assert_eq!((mine[0].kind, mine[0].target, mine[0].motive), (ContractKind::Beat, Target::Agent(o), Motive::Debt));
    // Repaid (no arrears): the clock drops.
    if let Some(x) = w.comp_mut::<citysim::Asset>(a) {
        x.finance = None;
    }
    hitmarket::debt_pass(&mut w);
    assert!(!w.hm_debts.contains_key(&(l, o)));
}

/// Protect (D12): a gang buys the known witness of its member's assault,
/// direct to its own first; untaken a day later, the same record moves to
/// the open Fixer in a district the gang holds (escrowed from its purse).
#[test]
fn test_protect_direct_then_brokered() {
    let mut w = world();
    let g = led_gangs(&mut w)[0];
    set_gang_treasury(&mut w, g, 100_000);
    let leader = w.comp::<citysim::Gang>(g).and_then(|x| x.leader).unwrap();
    let m = w.comp::<citysim::Gang>(g).map(|x| x.members.clone()).unwrap().into_iter().find(|&x| x != leader).unwrap();
    let wit = adults(&w)
        .into_iter()
        .find(|&a| w.gang_of(a).is_none() && !law::is_guard(&w, a) && w.edge(m, a).is_none() && a != m)
        .expect("a witness");
    w.edge_entry(m, wit).kind = RelKind::Acquaintance;
    law::file_report(&mut w, citysim::Crime::Assault, m, Some(wit));
    // The gang holds the district of an open Fixer.
    let f = contracts::open_fixers(&w)[0];
    let d = w.district_of_building(f);
    let home =
        w.buildings_of_kind(citysim::BuildingKind::Home).iter().copied().find(|&h| w.district_of_building(h) == d);
    if let (Some(h), Some(x)) = (home, w.comp_mut::<citysim::Gang>(g)) {
        x.territory.push(h);
    }
    hitmarket::gang_pass(&mut w);
    let mine = organic_by(&w, g);
    assert_eq!(mine.len(), 1, "the gang bought its member's witness");
    let r = &mine[0];
    assert_eq!((r.kind, r.target, r.motive, r.broker), (ContractKind::Beat, Target::Agent(wit), Motive::Protect, None));
    let id0 = identity(&w);
    day(&mut w, 1);
    hitmarket::gang_pass(&mut w);
    let r = &w.contracts[&r.id];
    assert_eq!(r.broker, Some(f), "re-posted at the held district's Fixer");
    assert_eq!(r.escrow, r.price, "the price in escrow");
    assert_eq!(organic_by(&w, g).len(), 1, "one record, not two");
    assert_eq!(identity(&w), id0);
}

/// H14: a dead witness's case goes cold `cold_days` after the death (not
/// before); a case a city guard saw does not.
#[test]
fn test_dead_witness_case_goes_cold_unless_a_guard_saw() {
    let run = |guard_saw: bool| -> bool {
        let mut w = world();
        let [s, wit] = strangers(&w, 2)[..] else { unreachable!() };
        law::file_report(&mut w, citysim::Crime::Assault, s, Some(wit));
        if guard_saw {
            let gd = citysim::systems::law_brain::guards(&w)[0];
            w.remember_crime(gd, s, citysim::Crime::Assault, 1.0, None);
        }
        w.kill_by(wit, citysim::DeathCause::Violence, None);
        hitmarket::cold_cases(&mut w);
        assert!(open_report(&w, s, wit), "not cold the same day");
        let days = u64::from(w.config.hitmarket.cold_days);
        day(&mut w, days);
        hitmarket::cold_cases(&mut w);
        let cold = !open_report(&w, s, wit);
        assert_eq!(cold, w.events.iter().any(|e| e.kind == EventKind::CaseCold));
        cold
    };
    assert!(run(false), "no witness, no case");
    assert!(!run(true), "a guard saw it: the case stands");
}

/// A done Silence Beat on `wit` by a gun, the witness at `courage`, with
/// `recant_p` 1: did the report on `s` go?
fn beaten_witness(courage: f32) -> bool {
    let mut w = world();
    w.config.hitmarket.recant_p = 1.0;
    let (s, wit) = suspect_and_witness(&mut w, citysim::Crime::Assault, true);
    let [g] = strangers(&w, 1)[..] else { unreachable!() };
    gun(&mut w, g);
    set_law(&mut w, wit, 0.5, courage);
    let p = Posting {
        buyer: Some(s),
        agent: Some(s),
        kind: ContractKind::Beat,
        target: Target::Agent(wit),
        broker: None,
        deadline_days: 10,
        origin: Origin::Hunt,
        price: Some(30),
    };
    let extra = PostExtra { deposit: None, motive: Motive::Silence, why: None };
    let id = contracts::post_with(&mut w, p, extra).expect("posted");
    if let Some(c) = w.contracts.get_mut(&id) {
        c.renege = false;
    }
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    let gone = !open_report(&w, s, wit);
    assert_eq!(gone, w.events.iter().any(|e| e.kind == EventKind::Recanted));
    gone
}

/// H15: a beaten witness recants at `recant_p × (1 − courage)`: always at
/// courage 0 with `recant_p` 1, never at courage 1.
#[test]
fn test_beaten_witness_recants_by_courage() {
    assert!(beaten_witness(0.0), "a frightened witness recants");
    assert!(!beaten_witness(1.0), "a brave witness stands");
}

/// H15: a guard witness never recants.
#[test]
fn test_guard_witness_never_recants() {
    let mut w = world();
    w.config.hitmarket.recant_p = 1.0;
    let gd = citysim::systems::law_brain::guards(&w)[0];
    set_law(&mut w, gd, 0.5, 0.0);
    let [s] = strangers(&w, 1)[..] else { unreachable!() };
    law::file_report(&mut w, citysim::Crime::Assault, s, Some(gd));
    let [other] = strangers(&w, 2)[1..] else { unreachable!() };
    let mut c = organic_record(&mut w, s, other);
    c.target = Target::Agent(gd);
    hitmarket::on_fulfilled(&mut w, &c);
    assert!(open_report(&w, s, gd), "a guard recanted");
}

/// A live organic Beat by `buyer` on `t` (posted direct with the motive Silence).
fn organic_record(w: &mut World, buyer: EntityId, t: EntityId) -> citysim::contract::Contract {
    set_coins(w, buyer, 10_000);
    let p = Posting {
        buyer: Some(buyer),
        agent: Some(buyer),
        kind: ContractKind::Beat,
        target: Target::Agent(t),
        broker: None,
        deadline_days: 10,
        origin: Origin::Hunt,
        price: Some(30),
    };
    let extra = PostExtra { deposit: None, motive: Motive::Silence, why: None };
    let id = contracts::post_with(w, p, extra).expect("posted");
    w.contracts[&id].clone()
}

/// A coward with a lethal grudge, rich, housed with an open Fixer in reach,
/// at `lod`.
fn walking_buyer(w: &mut World, lod: citysim::Lod) -> (EntityId, EntityId) {
    w.config.fixers.hire_reach_tiles = 100_000;
    let all: Vec<EntityId> = adults(w)
        .into_iter()
        .filter(|&a| !w.has::<GangMember>(a) && !law::is_guard(w, a) && !w.has::<citysim::Sentence>(a))
        .collect();
    let h = all.iter().copied().find(|&a| contracts::nearest_fixer(w, a).is_some()).expect("a housed holder");
    let t = all.iter().copied().find(|&a| a != h && w.edge(h, a).is_none()).expect("a target");
    set_law(w, h, 0.2, 0.1);
    set_grudge(w, h, t, 0.95);
    set_coins(w, h, 10_000);
    set_lod(w, h, lod);
    (h, t)
}

/// H9: a buyer at Full with a Fixer in reach does not post from the daily
/// pass: it holds a want (no coin moves), the Commission goal scores, its
/// plan is `GoTo(Fixer) → Commission`, and the step at the Fixer posts the
/// record there.
#[test]
fn test_full_buyer_walks_to_the_fixer() {
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Full);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    assert!(organic_by(&w, h).is_empty(), "no record before the walk");
    assert_eq!(coins(&w, h), 10_000, "no coin moved");
    let want = w.wants.get(&h).cloned().expect("a want");
    assert_eq!((want.target, want.kind, want.motive), (t, ContractKind::Hit, Motive::Revenge));
    assert!(hitmarket::commission_considerations(&w, h).is_some());
    let plan = hitmarket::commission_plan(&mut w, h).expect("a plan");
    let steps: Vec<_> = plan.steps.iter().map(|s| (s.action, s.target)).collect();
    assert_eq!(
        steps,
        vec![
            (citysim::ActionKind::GoTo(citysim::LocationKey::Seller), Some(want.fixer)),
            (citysim::ActionKind::Commission, Some(want.fixer))
        ]
    );
    // D8: no Hunt on the target while the want stands.
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95), "the errand stands in for the Hunt");
    let _ = hitmarket::on_commission(&mut w, h, Some(want.fixer));
    let mine = organic_by(&w, h);
    assert_eq!(mine.len(), 1, "posted at the Fixer");
    assert_eq!((mine[0].broker, mine[0].motive), (Some(want.fixer), Motive::Revenge));
    assert!(w.wants.is_empty());
    assert_eq!(w.stats.current.hit.hm_commissions, 1);
}

/// H9, H10: with no Fixer in reach a Full buyer posts at once, no errand
/// (direct, or with too few asks of its own, at the nearest Fixer out of
/// reach: the gaps pass); a Statistical buyer posts at once.
#[test]
fn test_no_errand_without_a_fixer_or_a_body() {
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Full);
    w.config.fixers.hire_reach_tiles = 0;
    let far = contracts::reach_offer(&w, ContractKind::Hit, &Target::Agent(t), h).map(|x| x.0);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    let mine = organic_by(&w, h);
    assert_eq!((mine.len(), mine[0].broker), (1, far), "at once");
    assert!(w.wants.is_empty());
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Statistical);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    assert_eq!(organic_by(&w, h).len(), 1, "a Statistical buyer posts at once");
    assert!(w.wants.is_empty());
}

/// H8: a want lapses after `want_days`; a buyer demoted to Statistical
/// posts at once.
#[test]
fn test_want_sweep_lapses_or_posts() {
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Full);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    set_lod(&mut w, h, citysim::Lod::Statistical);
    hitmarket::want_sweep(&mut w);
    assert!(w.wants.is_empty());
    assert_eq!(organic_by(&w, h).len(), 1, "demoted: posted at once");
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Full);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    let days = u64::from(w.config.hitmarket.want_days);
    day(&mut w, days);
    hitmarket::want_sweep(&mut w);
    assert!(w.wants.is_empty(), "lapsed");
    assert!(organic_by(&w, h).is_empty());
}

/// H17: the buyer of a done Beat learns it first-hand (an `Assaulted` deed
/// with the taker as actor) and remembers it (`Commissioned`, relief for
/// Revenge).
#[test]
fn test_buyer_learns_and_remembers_the_deed() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 10_000);
    let f = contracts::open_fixers(&w)[0];
    let extra = PostExtra { deposit: None, motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, brokered(buyer, ContractKind::Beat, t, f, 40), extra).expect("posted");
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    let m = w.comp::<citysim::Memory>(buyer).expect("memory");
    let c = m
        .entries
        .iter()
        .find(|e| e.kind == citysim::MemoryKind::Commissioned && e.subject == Some(t))
        .expect("Commissioned");
    assert!((c.valence - w.config.hitmarket.closure_val).abs() < 1e-4, "{}", c.valence);
    let heard = m.heard.iter().chain(m.entries.iter()).any(|e| {
        e.deed == Some(citysim::word::Deed::Assaulted) && e.subject == Some(g) && e.object == Some(t) && e.hops == 0
    });
    assert!(heard, "the buyer knows who did it, first-hand");
}

/// H22: a want and a debt clock survive a save round trip.
#[test]
fn test_save_round_trip_with_a_want_and_a_debt_clock() {
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Full);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    let now = w.tick;
    w.hm_debts.insert((h, t), citysim::contract::DebtClock { seen: now, beat: Some(now) });
    // The gaps pass: a stiffed gun's grace (save 9).
    w.hm_stiffs.insert((t, h), (now, 0.8));
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(back.wants, w.wants);
    assert_eq!(back.hm_debts, w.hm_debts);
    assert_eq!(back.hm_stiffs, w.hm_stiffs);
}

/// P2 fix round: a direct Hit's viewer must pass the gun gate, as a
/// Fixer's regulars did to network: a friend with a well-paid day job does
/// not take it; the same friend jobless and broke does.
#[test]
fn test_direct_hit_skips_a_well_paid_viewer() {
    let run = |employed: bool| -> Option<EntityId> {
        let mut w = world();
        let [buyer, t] = strangers(&w, 2)[..] else { unreachable!() };
        let v = adults(&w)
            .into_iter()
            .find(|&a| {
                a != buyer
                    && a != t
                    && w.edge(a, t).is_none()
                    && w.edge(a, buyer).is_none()
                    && !w.has::<GangMember>(a)
                    && !law::is_guard(&w, a)
                    && w.comp::<citysim::Job>(a).is_some_and(|j| j.wage_per_day > 0)
            })
            .expect("an employed viewer");
        w.edge_entry(buyer, v).kind = RelKind::Friend;
        if let Some(p) = w.comp_mut::<Personality>(v) {
            p.lawfulness = 0.0;
            p.courage = 0.9;
        }
        if let Some(s) = w.comp_mut::<Skills>(v) {
            s.fighting = 1.0;
        }
        let wage = w.comp::<citysim::Job>(v).map(|j| j.wage_per_day).unwrap();
        w.median_wage = wage;
        if !employed {
            w.vacate_job(v);
            set_coins(&mut w, v, 0);
        }
        set_coins(&mut w, buyer, 10_000);
        let mut p = brokered(buyer, ContractKind::Hit, t, EntityId::NONE, 2_000);
        p.broker = None;
        let extra = PostExtra { deposit: None, motive: Motive::Revenge, why: None };
        let id = contracts::post_with(&mut w, p, extra).expect("posted");
        contracts::match_day(&mut w);
        w.contracts[&id].taker
    };
    assert!(run(false).is_some(), "the control: a broke, jobless friend takes it");
    assert_eq!(run(true), None, "a well-paid friend took a direct Hit");
}

// ---------------------------------------------------------------------------
// Review fixes (`/code-review high 8270896..HEAD`)
// ---------------------------------------------------------------------------

/// Review fix 1: a buyer on the way to a Fixer for one head reports no hire
/// on another (a want stands for its own head only, any motive); the want on
/// the first head is untouched.
#[test]
fn test_want_on_one_head_is_no_hire_on_another() {
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Full);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    let want = w.wants.get(&h).cloned().expect("a want");
    let t2 = adults(&w)
        .into_iter()
        .find(|&a| a != h && a != t && w.edge(h, a).is_none() && !law::is_guard(&w, a))
        .expect("a second head");
    set_grudge(&mut w, h, t2, 0.95);
    assert!(hitmarket::hires(&w, h, t2));
    assert!(!hitmarket::try_revenge(&mut w, h, t2, 0.95), "no hire happened on the second head");
    assert_eq!(w.wants.get(&h), Some(&want), "the first errand stands");
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95), "the errand stands for its own head");
    assert!(organic_by(&w, h).is_empty());
}

/// Review fix 2: a bought revenge Beat that takes the grudge to 0 or under
/// settles it (weight 0), as a Hunt's beating does.
#[test]
fn test_bought_beating_settles_a_light_grudge() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    let light = w.config.grudges.beat_settle * 0.6;
    set_grudge(&mut w, buyer, t, light);
    set_coins(&mut w, buyer, 500);
    let f = contracts::open_fixers(&w)[0];
    let extra = PostExtra { deposit: None, motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, brokered(buyer, ContractKind::Beat, t, f, 40), extra).expect("posted");
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    let x = w.comp::<Grudges>(buyer).and_then(|gs| gs.list.iter().find(|x| x.target == t).cloned()).expect("grudge");
    assert!(x.settled.is_some(), "answered: settled");
    assert_eq!(x.weight, 0.0);
    assert!(grudges::grudge_on(&w, buyer, t).is_none());
}

/// Review fix 3: Lobby's culprit record is organic, so the day's brake
/// holds it: past `posts_per_day` (or `max_open`) nothing is posted and the
/// bribe goes ahead.
#[test]
fn test_lobby_culprit_under_the_brake() {
    let mut w = world();
    let (c, _, _) = two_corps(&w);
    set_corp_treasury(&mut w, c, 1_000_000);
    set_exec_law(&mut w, c, 0.1);
    let g = led_gangs(&mut w)[0];
    w.config.hitmarket.posts_per_day = 0;
    let n = w.contracts.len();
    assert!(!contracts::post_lobby(&mut w, c, g), "the brake: the bribe goes ahead");
    assert_eq!(w.contracts.len(), n, "nothing posted");
    w.config.hitmarket.posts_per_day = 3;
    w.config.hitmarket.max_open = 0;
    assert!(!contracts::post_lobby(&mut w, c, g));
    assert_eq!(w.contracts.len(), n);
    w.config.hitmarket.max_open = 16;
    assert!(contracts::post_lobby(&mut w, c, g), "the control: room, posted");
    assert_eq!(w.contracts.len(), n + 1);
}

/// An edge debt of `debt_min` owed to a lawless rich Statistical creditor,
/// `debt_days` old.
fn aged_edge_debt(w: &mut World) -> (EntityId, EntityId) {
    let [c, d] = strangers(w, 2)[..] else { unreachable!() };
    set_law(w, c, 0.1, 0.5);
    set_lod(w, c, citysim::Lod::Statistical);
    set_coins(w, c, 10_000);
    let owed = w.config.hitmarket.debt_min;
    let (lo, _) = citysim::edge_key(c, d);
    let e = w.edge_entry(c, d);
    e.debt = if lo == d { owed } else { -owed };
    hitmarket::debt_pass(w);
    let days = u64::from(w.config.hitmarket.debt_days);
    day(w, days);
    (c, d)
}

/// Review fix 5: only a living free adult creditor buys a Debt beating: a
/// cuffed one buys nothing, a dead one is not even priced (no
/// `hm_unaffordable` tick a day).
#[test]
fn test_debt_creditor_must_be_free_and_living() {
    let mut w = world();
    let (c, _) = aged_edge_debt(&mut w);
    let gd = citysim::systems::law_brain::guards(&w)[0];
    if let Some(b) = w.comp_mut::<citysim::Brain>(c) {
        b.cuffed_by = Some(gd);
    }
    hitmarket::debt_pass(&mut w);
    assert!(organic_by(&w, c).is_empty(), "a cuffed creditor bought a beating");
    let mut w = world();
    let (c, _) = aged_edge_debt(&mut w);
    w.kill_by(c, citysim::DeathCause::Violence, None);
    let before = w.stats.current.hit.hm_unaffordable;
    hitmarket::debt_pass(&mut w);
    assert!(organic_by(&w, c).is_empty());
    assert_eq!(w.stats.current.hit.hm_unaffordable, before, "a dead creditor tried to buy");
    // Control: the same debt, a free creditor.
    let mut w = world();
    let (c, _) = aged_edge_debt(&mut w);
    hitmarket::debt_pass(&mut w);
    assert_eq!(organic_by(&w, c).len(), 1);
}

/// Review fix 6: a finance-arrears Debt record's text names what is in
/// arrears (`per_day × arrears`, at most what remains), not "0 owed".
#[test]
fn test_debt_from_arrears_says_what_is_owed() {
    use citysim::systems::assets;
    let mut w = world();
    let [l, o] = strangers(&w, 2)[..] else { unreachable!() };
    set_law(&mut w, l, 0.1, 0.5);
    set_lod(&mut w, l, citysim::Lod::Statistical);
    set_coins(&mut w, l, 10_000);
    let spot = contracts::open_fixers(&w)[0];
    let a =
        assets::spawn_asset(&mut w, citysim::AssetKind::Motorcycle, 1, Some(o), citysim::AssetLoc::Parked(spot), 300);
    let repo = w.config.assets.repo_days;
    if let Some(x) = w.comp_mut::<citysim::Asset>(a) {
        x.finance = Some(citysim::Finance { lender: Some(l), remaining: 100, per_day: 5, arrears: repo });
    }
    hitmarket::debt_pass(&mut w);
    let days = u64::from(w.config.hitmarket.debt_days);
    day(&mut w, days);
    hitmarket::debt_pass(&mut w);
    assert_eq!(organic_by(&w, l).len(), 1);
    let owed = (5 * i64::from(repo)).min(100);
    assert!(owed > 0);
    let phrase = format!("over {owed} owed");
    assert!(w.events.iter().any(|e| e.text.contains(&phrase)), "no event reads {phrase:?}");
    assert!(!w.events.iter().any(|e| e.text.contains("over 0 owed")));
}

/// Review fix 7 (the spec's Rival row): a dirty corp in an open vendetta
/// buys the feuding rival's exec before the one that last wronged it.
#[test]
fn test_corp_rival_vendetta_before_the_last_wrong() {
    let mut w = world();
    let (c, r, r2) = two_corps(&w);
    set_corp_treasury(&mut w, c, 1_000_000);
    set_exec_law(&mut w, c, 0.1);
    hitmarket::wronged(&mut w, c, r, citysim::contract::Wrong::Acquired);
    let now = w.tick;
    w.vendettas.push(citysim::word::Vendetta { a: c, b: r2, since: now, kills: [0, 0], w: [1.0, 1.0], declared: None });
    hitmarket::corp_pass(&mut w);
    let e2 = w.comp::<citysim::Corp>(r2).and_then(|x| x.exec).unwrap();
    let mine = organic_by(&w, c);
    assert_eq!(mine.len(), 1);
    assert_eq!((mine[0].target, mine[0].motive), (Target::Agent(e2), Motive::Rival), "the feud first");
}

// ---------------------------------------------------------------------------
// The gaps pass (docs/HIT_MARKET.md "Gaps pass")
// ---------------------------------------------------------------------------

/// Does `holder` hold a `deed` rumour with this actor and object?
fn holds_deed(w: &World, holder: EntityId, deed: citysim::word::Deed, actor: EntityId, object: EntityId) -> bool {
    w.comp::<citysim::Memory>(holder).is_some_and(|m| {
        m.entries
            .iter()
            .chain(m.heard.iter())
            .any(|e| e.deed == Some(deed) && e.subject == Some(actor) && e.object == Some(object))
    })
}

/// A teller who knows the city (no distortion) and holds nothing yet.
fn blank_teller(w: &mut World, id: EntityId) {
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.knowledge = 1.0;
    }
    if let Some(m) = w.comp_mut::<citysim::Memory>(id) {
        m.entries.clear();
        m.heard.clear();
    }
}

/// An off-reach Revenge buyer whose own people would not take it, two guns
/// on the Fixer's books, with or without two guns among its own friends.
fn off_reach_buyer(w: &mut World, own_guns: bool) -> (EntityId, EntityId, EntityId) {
    w.config.fixers.hire_reach_tiles = 0;
    let [b, t, g1, g2, f1, f2] = strangers(w, 6)[..] else { unreachable!() };
    set_law(w, b, 0.2, 0.1);
    set_lod(w, b, citysim::Lod::Statistical);
    set_coins(w, b, 10_000);
    let ns: Vec<EntityId> = w.neighbours(b).collect();
    for n in ns {
        set_law(w, n, 1.0, 0.0);
    }
    let f = contracts::open_fixers(w)[0];
    let now = w.tick;
    for g in [g1, g2] {
        gun(w, g);
        if let Some(k) = w.comp_mut::<citysim::contract::Broker>(f) {
            k.regulars.insert(g, now);
        }
    }
    if own_guns {
        for g in [f1, f2] {
            gun(w, g);
            w.edge_entry(b, g).kind = RelKind::Friend;
        }
    }
    (b, t, f)
}

/// Gaps pass (reach): an agent's direct record with fewer than `ask_rank`
/// asks among its own people goes to the nearest open Fixer (out of
/// walking reach) at that Fixer's market price plus the go-between's
/// premium (`reach_premium` 0.5), not the M16a quote; with two guns of its
/// own it stays direct at their price.
#[test]
fn test_off_reach_buyer_goes_to_the_nearest_fixer_at_a_premium() {
    let mut w = world();
    let (b, t, f) = off_reach_buyer(&mut w, false);
    let tgt = Target::Agent(t);
    assert!(contracts::market_price(&w, ContractKind::Hit, &tgt, Some(b), Some(b), None).is_none());
    let p =
        contracts::market_price(&w, ContractKind::Hit, &tgt, Some(b), Some(b), Some(f)).expect("the Fixer's guns ask");
    let quote = contracts::quote(&w, ContractKind::Hit, &tgt, None);
    assert!(hitmarket::agent_buy(&mut w, b, t, ContractKind::Hit, Motive::Revenge, 0.95, None));
    let mine = organic_by(&w, b);
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].broker, Some(f), "word went to the Fixer out of reach");
    assert_eq!(mine[0].price, (p as f32 * 1.5).ceil() as i64, "its market price and the premium");
    assert!(mine[0].price < quote, "not the M16a quote ({} vs {quote})", mine[0].price);
    // Control: two guns among the buyer's own friends price it direct.
    let mut w = world();
    let (b, t, _) = off_reach_buyer(&mut w, true);
    let tgt = Target::Agent(t);
    let direct = contracts::market_price(&w, ContractKind::Hit, &tgt, Some(b), Some(b), None).expect("own guns ask");
    assert!(hitmarket::agent_buy(&mut w, b, t, ContractKind::Hit, Motive::Revenge, 0.95, None));
    let mine = organic_by(&w, b);
    assert_eq!((mine.len(), mine[0].broker, mine[0].price), (1, None, direct), "direct at its own people's price");
}

/// Gaps pass: a lawful revenge buyer waits for the law: with his report on
/// the killer open (the killer wanted) or the killer held, he buys nothing;
/// with no case open he buys; a lawless one does not wait.
#[test]
fn test_lawful_buyer_waits_for_the_law() {
    let run = |law_: f32, case: u8| -> usize {
        let mut w = world();
        let [h, t] = strangers(&w, 2)[..] else { unreachable!() };
        set_law(&mut w, h, law_, 0.1);
        set_lod(&mut w, h, citysim::Lod::Statistical);
        set_grudge(&mut w, h, t, 0.95);
        set_coins(&mut w, h, 10_000);
        match case {
            1 => law::file_report(&mut w, citysim::Crime::Murder, t, Some(h)),
            2 => {
                let gd = citysim::systems::law_brain::guards(&w)[0];
                if let Some(b) = w.comp_mut::<citysim::Brain>(t) {
                    b.cuffed_by = Some(gd);
                }
            }
            _ => {}
        }
        hitmarket::try_revenge(&mut w, h, t, 0.95);
        organic_by(&w, h).len()
    };
    assert_eq!(run(0.8, 1), 0, "a lawful man waits while his report on the killer is open");
    assert_eq!(run(0.8, 2), 0, "and while the killer is held");
    assert_eq!(run(0.8, 0), 1, "no case open: he buys");
    assert_eq!(run(0.3, 1), 1, "a lawless one does not wait");
}

/// Gaps pass: a lawful buyer's want waits on the way to the Fixer: the
/// errand does not score while the law has the case, and the step at the
/// Fixer keeps the want and posts nothing.
#[test]
fn test_lawful_buyers_want_waits_for_the_law() {
    let mut w = world();
    let (h, t) = walking_buyer(&mut w, citysim::Lod::Full);
    set_law(&mut w, h, 0.8, 0.1);
    assert!(hitmarket::try_revenge(&mut w, h, t, 0.95));
    let want = w.wants.get(&h).cloned().expect("a want");
    law::file_report(&mut w, citysim::Crime::Murder, t, Some(h));
    assert!(hitmarket::waits_for_law(&w, h, t));
    assert!(hitmarket::commission_considerations(&w, h).is_none(), "the errand waits");
    let _ = hitmarket::on_commission(&mut w, h, Some(want.fixer));
    assert!(organic_by(&w, h).is_empty(), "nothing bought at the Fixer");
    assert!(w.wants.contains_key(&h), "the want waits");
}

/// Gaps pass: a Fixer's owner keeps her book quiet: she holds the buyer's
/// `Hired` first-hand and does not tell it; the gun who took it still does
/// (the leak stays through other holders).
#[test]
fn test_fixer_owner_keeps_her_book_quiet() {
    let mut w = world();
    let f = contracts::open_fixers(&w)[0];
    let owner = w.owner_of(f).expect("an owner");
    let people: Vec<EntityId> = strangers(&w, 7).into_iter().filter(|&a| a != owner).collect();
    let [buyer, t, g, listener, l2] = people[..5] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 5000);
    blank_teller(&mut w, owner);
    blank_teller(&mut w, g);
    w.edge_entry(listener, owner).trust = 1.0;
    w.edge_entry(l2, g).trust = 1.0;
    let id = contracts::post(&mut w, brokered(buyer, ContractKind::Hit, t, f, 100)).expect("posted");
    let hired = citysim::word::Deed::Hired;
    assert!(holds_deed(&w, owner, hired, buyer, t), "the broker knows first-hand");
    citysim::systems::gossip::exchange(&mut w, owner, listener, citysim::systems::gossip::Venue::Chat);
    assert!(!holds_deed(&w, listener, hired, buyer, t), "the owner told her book");
    assert!(contracts::accept(&mut w, id, g, &[]));
    citysim::systems::gossip::exchange(&mut w, g, l2, citysim::systems::gossip::Venue::Chat);
    assert!(holds_deed(&w, l2, hired, buyer, t), "the gun still talks");
}

/// Gaps pass: a buyer keeps the deed she bought to herself (H17's
/// first-hand `Assaulted`, actor the gun); a first-hand deed she did not
/// buy she still tells.
#[test]
fn test_buyer_keeps_the_bought_deed_to_herself() {
    let mut w = world();
    let [buyer, t, g, listener, x, y] = strangers(&w, 6)[..] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 10_000);
    let f = contracts::open_fixers(&w)[0];
    let extra = PostExtra { deposit: None, motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, brokered(buyer, ContractKind::Beat, t, f, 40), extra).expect("posted");
    assert!(contracts::accept(&mut w, id, g, &[]));
    blank_teller(&mut w, buyer);
    w.edge_entry(listener, buyer).trust = 1.0;
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    let assaulted = citysim::word::Deed::Assaulted;
    assert!(holds_deed(&w, buyer, assaulted, g, t), "H17: the buyer knows who did it");
    let sal = w.config.gossip.deed_sal.get(assaulted);
    let other = citysim::MemoryEntry {
        subject: Some(x),
        salience: sal,
        deed: Some(assaulted),
        object: Some(y),
        hops: 0,
        conf: 1.0,
        ..citysim::MemoryEntry::blank(citysim::MemoryKind::Rumour, w.tick)
    };
    citysim::systems::memory::hear_entry(&mut w, buyer, other);
    for _ in 0..2 {
        citysim::systems::gossip::exchange(&mut w, buyer, listener, citysim::systems::gossip::Venue::Chat);
    }
    assert!(!holds_deed(&w, listener, assaulted, g, t), "she told the beating she bought");
    assert!(holds_deed(&w, listener, assaulted, x, y), "she tells what she only saw");
}

/// Gaps pass (decision 3's grace): a stiffed gun's grudge counts toward no
/// Hunt while the balance is owed out of wages; `debt_days` on, unpaid, it
/// does.
#[test]
fn test_stiffed_gun_waits_out_the_grace() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 50);
    let id = credit_record(&mut w, buyer, t, g, 100);
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    let gr = grudges::grudge_on(&w, g, buyer).expect("the gun's grudge");
    assert!(gr.weight >= w.config.hunt.hunt_min, "{}", gr.weight);
    let hunted = |w: &World| citysim::systems::hunt::heaviest_eligible(w, g).map(|x| x.0) == Some(buyer);
    assert!(!hunted(&w), "owed out of wages first: no Hunt in the grace");
    let days = u64::from(w.config.hitmarket.debt_days);
    day(&mut w, days - 1);
    hitmarket::stiff_sweep(&mut w);
    assert!(!hunted(&w), "a day short");
    day(&mut w, 1);
    hitmarket::stiff_sweep(&mut w);
    assert!(hunted(&w), "unpaid past the grace: the grudge counts");
    assert!(w.hm_stiffs.is_empty());
}

/// Gaps pass: a stiffed buyer who pays the balance from wages inside the
/// grace is forgiven: the gun's `Betrayed` grudge settles and never hunts.
#[test]
fn test_stiffed_gun_paid_in_the_grace_lets_it_go() {
    let mut w = world();
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    set_coins(&mut w, buyer, 50);
    let id = credit_record(&mut w, buyer, t, g, 100);
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    set_coins(&mut w, buyer, 1_000);
    citysim::systems::social::repay_debts(&mut w, buyer);
    hitmarket::stiff_sweep(&mut w);
    assert!(grudges::grudge_on(&w, g, buyer).is_none(), "paid: the grudge settles");
    let days = u64::from(w.config.hitmarket.debt_days);
    day(&mut w, days);
    hitmarket::stiff_sweep(&mut w);
    assert!(citysim::systems::hunt::heaviest_eligible(&w, g).map(|x| x.0) != Some(buyer));
}

/// Gaps pass (takers who can't close): a taken organic Hit queued for a
/// live slot (every `max_missions` slot held) is worked on the ledger after
/// `queue_hours`, a solo gun's and a gang's job alike; a gang's ledger
/// strike is its crew's (never "the taker was not free").
#[test]
fn test_queued_record_goes_to_the_ledger() {
    for gang_job in [false, true] {
        let mut w = world();
        w.config.missions.max_missions = 0;
        // A day in, at 01:00 (an hour mark, past midnight's pass).
        w.run_ticks(citysim::TICKS_PER_DAY + citysim::TICKS_PER_HOUR);
        let gangs = if gang_job { led_gangs(&mut w) } else { Vec::new() };
        let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
        gun(&mut w, g);
        set_lod(&mut w, g, citysim::Lod::Full);
        set_coins(&mut w, buyer, 10_000);
        let f = contracts::open_fixers(&w)[0];
        let extra = PostExtra { deposit: None, motive: Motive::Revenge, why: None };
        let id = contracts::post_with(&mut w, brokered(buyer, ContractKind::Hit, t, f, 100), extra).expect("posted");
        let taken = if gang_job {
            let c = w.contracts[&id].clone();
            let crew = contracts::job_crew(&w, &c, gangs[0]);
            assert!(!crew.is_empty());
            contracts::accept(&mut w, id, gangs[0], &crew)
        } else {
            contracts::accept(&mut w, id, g, &[])
        };
        assert!(taken);
        assert_eq!(w.contracts[&id].render, citysim::contract::Render::Live, "queued for a live slot");
        // Fix round: only a record with no body on it goes to the ledger;
        // the gun (taken live as a body) has since gone off-screen.
        // The take is `queue_hours` old (backdated: a crew may be promoted
        // by the LOD pass over a real day, and stays queued then).
        let hours = u64::from(w.config.hitmarket.queue_hours);
        if let Some(x) = w.contracts.get_mut(&id) {
            x.taken = x.taken.map(|k| k.saturating_sub(hours * citysim::TICKS_PER_HOUR));
        }
        let c = w.contracts[&id].clone();
        let party: Vec<EntityId> = c.taker.into_iter().chain(c.crew.iter().copied()).chain([t]).collect();
        for p in party {
            set_lod(&mut w, p, citysim::Lod::Statistical);
        }
        // A crew member walking off-screen (Coarse) may strike from the
        // ledger; only the target as a body, or a party in front of the
        // player, keeps it live.
        if let Some(&m) = c.crew.first() {
            set_lod(&mut w, m, citysim::Lod::Coarse);
        }
        // The hourly queue pass (`contracts::run` at the hour).
        contracts::run(&mut w);
        let c = w.contracts[&id].clone();
        assert_eq!(
            (c.status, c.render),
            (ContractStatus::Taken, citysim::contract::Render::Ledger),
            "gang {gang_job}: still queued after {hours} h"
        );
        assert!(c.due.is_some_and(|d| d <= c.deadline), "a due inside the deadline");
        contracts::resolve_ledger(&mut w, id);
        let c = w.contracts[&id].clone();
        assert!(
            c.status != ContractStatus::Taken || c.attempts > 0 || c.holds > 0,
            "gang {gang_job}: the ledger did not work it"
        );
        let not_free = w.contract_log.iter().any(|(_, x, text)| *x == id && text.contains("was not free"));
        assert!(!not_free, "gang {gang_job}: the ledger read a gang as an unfree taker");
    }
}

/// Fix round (review 1): a stale-queued record whose target is a body
/// (pinned here; Full, Coarse or on screen alike) is never rolled on the
/// ledger, where `lod::stat_hit` would kill a body with no scene: it stays
/// live and, stale, starts past the `max_missions` run count.
#[test]
fn test_stale_queued_record_with_a_shown_target_stays_live() {
    let mut w = world();
    w.config.missions.max_missions = 0;
    // The gun (lawfulness 0) jumps every gate of the El: off screen its
    // errand rides (parity's `p_errand_ride`) had it arrested for Fare
    // Evasion on day 1 here, failing the attempt (the record back on the
    // board, `Render::Ledger` as every open record). This test reads the
    // queued record's render while its taker is free.
    if let Some(t) = w.stat_table.as_mut() {
        t.p_errand_ride = None;
    }
    let [buyer, t, g] = strangers(&w, 3)[..] else { unreachable!() };
    gun(&mut w, g);
    if let Some(b) = w.comp_mut::<citysim::Brain>(t) {
        b.pinned = true;
    }
    set_coins(&mut w, buyer, 10_000);
    let f = contracts::open_fixers(&w)[0];
    let extra = PostExtra { deposit: None, motive: Motive::Revenge, why: None };
    let id = contracts::post_with(&mut w, brokered(buyer, ContractKind::Hit, t, f, 100), extra).expect("posted");
    assert!(contracts::accept(&mut w, id, g, &[]));
    assert_eq!(w.contracts[&id].render, citysim::contract::Render::Live, "queued live");
    let hours = u64::from(w.config.hitmarket.queue_hours) + 1;
    w.run_ticks(hours * citysim::TICKS_PER_HOUR);
    let c = w.contracts[&id].clone();
    let off = w.contract_log.iter().any(|(_, x, text)| *x == id && text.contains("worked off-screen"));
    assert!(!off && c.render == citysim::contract::Render::Live, "a shown target went to the ledger");
    // ... and, stale, it jumps the run cap: worked live.
    assert!(c.started.is_some(), "a stale record with a body on it never started");
}

/// A gun stiffed by a buyer on a 100 record with 50 down and nothing more;
/// `murder_grudge`: the gun already held a killing grudge on the buyer.
fn stiffed_gun(w: &mut World, murder_grudge: bool) -> (EntityId, EntityId) {
    let [buyer, t, g] = strangers(w, 3)[..] else { unreachable!() };
    gun(w, g);
    if murder_grudge {
        set_grudge(w, g, buyer, 0.9);
    }
    set_coins(w, buyer, 50);
    let id = credit_record(w, buyer, t, g, 100);
    contracts::settle(w, id, contracts::Settle::Fulfilled, "");
    (buyer, g)
}

/// Fix round (review 2): the grace's end re-swears the grudge at the
/// stiffing's weight, not merged over the grudge that stood (no decay ran
/// here, so the weight is unchanged).
#[test]
fn test_grace_end_does_not_overweight_the_grudge() {
    let mut w = world();
    let (buyer, g) = stiffed_gun(&mut w, false);
    let before = grudges::grudge_on(&w, g, buyer).expect("grudge").weight;
    let days = u64::from(w.config.hitmarket.debt_days);
    day(&mut w, days);
    hitmarket::stiff_sweep(&mut w);
    let after = grudges::grudge_on(&w, g, buyer).expect("grudge").weight;
    assert!((after - before).abs() < 1e-5, "{before} -> {after}");
}

/// Fix round (review 2): the grace holds back only the betrayal: a gun who
/// already holds a killing grudge on the buyer still hunts him.
#[test]
fn test_grace_does_not_hide_a_killing_grudge() {
    let mut w = world();
    let (buyer, g) = stiffed_gun(&mut w, true);
    assert!(w.hm_stiffs.contains_key(&(g, buyer)));
    assert_eq!(citysim::systems::hunt::heaviest_eligible(&w, g).map(|x| x.0), Some(buyer));
}

/// Fix round (review 3): an edge pruned with the balance on it is no
/// repayment: the clock stands and the grudge is not settled.
#[test]
fn test_pruned_debt_edge_is_not_a_repayment() {
    let mut w = world();
    let (buyer, g) = stiffed_gun(&mut w, false);
    w.remove_edge(buyer, g);
    hitmarket::stiff_sweep(&mut w);
    assert!(w.hm_stiffs.contains_key(&(g, buyer)), "the clock stands");
    assert!(grudges::grudge_on(&w, g, buyer).is_some(), "not forgiven");
}

/// Review item 5 (`/code-review high 7324e60..dfaca2e`): the grace's end
/// restores the betrayal only. A killing grudge that stood at the stiffing
/// took the stiff in then and was never held back, so the grace's end does
/// not raise it again (it was raised to the stiffing's weight, whatever
/// it had decayed to).
#[test]
fn test_grace_end_leaves_a_killing_grudge_alone() {
    let mut w = world();
    let (buyer, g) = stiffed_gun(&mut w, true);
    let stiff_w = w.hm_stiffs[&(g, buyer)].1;
    // The killing grudge decayed through the grace below the stiff's weight.
    let decayed = stiff_w * 0.5;
    if let Some(x) = w.comp_mut::<Grudges>(g).and_then(|gr| gr.list.iter_mut().find(|x| x.target == buyer)) {
        assert!(matches!(x.cause, GrudgeCause::KilledKin(_)), "the killing grudge took the stiff");
        x.weight = decayed;
    }
    let days = u64::from(w.config.hitmarket.debt_days);
    day(&mut w, days);
    hitmarket::stiff_sweep(&mut w);
    assert!(!w.hm_stiffs.contains_key(&(g, buyer)), "the clock ran out");
    let x = grudges::grudge_on(&w, g, buyer).expect("grudge");
    assert!(matches!(x.cause, GrudgeCause::KilledKin(_)));
    assert!((x.weight - decayed).abs() < 1e-6, "left as it stood: {decayed} -> {}", x.weight);
}
