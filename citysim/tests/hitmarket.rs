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
    if c.brokered() {
        assert_eq!(c.escrow, c.deposit, "only the deposit is escrowed");
        assert_eq!(coins(&w, h), 0);
    } else {
        assert_eq!(coins(&w, h), c.deposit, "a direct buyer holds it");
    }
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
    assert_eq!(coins(&w, buyer), 60, "a direct record moves nothing at posting");
    assert!(contracts::accept(&mut w, id, g, &[]));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    assert_eq!(coins(&w, buyer), 0);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Stiffed));
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
