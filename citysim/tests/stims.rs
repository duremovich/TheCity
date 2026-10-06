//! M13 phase 4: Stims, addiction and the drug trade (docs/M13_ASSETS.md § 4,
//! plan 4.6): the cook, a dealer's sale, a witnessed deal and the
//! confiscation, addiction and withdrawal, Detox, the overdose threshold and
//! legal stims at a Market.

use citysim::systems::{assets, demography, gang, law, stims};
use citysim::{
    Body, Brain, Building, BuildingKind, Config, Corpse, Crime, DeathCause, EntityId, EventKind, Gang, Good, Inventory,
    Job, Personality, Position, Skills, Wallet, World, TICKS_PER_HOUR,
};

fn city() -> World {
    World::new(42, Config::load())
}

/// Living adults with a Brain and a Wallet, not in a gang and without a job,
/// ascending: free to be placed and enlisted.
fn free(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && w.has::<Wallet>(a) && demography::is_adult(w, a))
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<Job>(a))
        .collect()
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |x| x.coins)
}

fn set_coins(w: &mut World, a: EntityId, c: i64) {
    w.comp_mut::<Wallet>(a).expect("wallet").coins = c;
}

fn stims_of(w: &World, a: EntityId) -> u16 {
    w.comp::<Inventory>(a).map_or(0, |i| i.stims)
}

fn set_stims(w: &mut World, a: EntityId, n: u16) {
    w.comp_mut::<Inventory>(a).expect("inventory").stims = n;
}

fn treasury(w: &World, g: EntityId) -> i64 {
    w.comp::<Gang>(g).expect("gang").treasury
}

fn set_body(w: &mut World, a: EntityId, addiction: f32) {
    let b = w.comp_mut::<Body>(a).expect("body");
    b.addiction = addiction;
    b.last_use = None;
}

/// Put `a` inside building `b`.
fn put_in(w: &mut World, a: EntityId, b: EntityId) {
    w.remove_from_building(a);
    w.enter_building(a, b);
    assert_eq!(w.comp::<Position>(a).and_then(|p| p.building), Some(b));
}

/// The first standing Bar.
fn a_bar(w: &World) -> EntityId {
    w.buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .find(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .expect("a Bar")
}

/// A gang of the given members (enlisted into the first seeded gang), its
/// treasury and Hideout stock zeroed, the leader's greed at 0.5.
fn gang_of(w: &mut World, members: &[EntityId]) -> EntityId {
    let g = w.gangs()[0];
    for &m in members {
        gang::enlist(w, m, g);
    }
    w.comp_mut::<Gang>(g).expect("gang").treasury = 0;
    let h = w.hideout_of(g).expect("hideout");
    let s = w.stock(h, Good::Stims);
    w.take_stock(h, Good::Stims, s);
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("a leader");
    w.comp_mut::<Personality>(leader).expect("p").greed = 0.5;
    g
}

#[test]
fn test_cook_spends_treasury_into_hideout_stock() {
    let mut w = city();
    let people = free(&w);
    let g = gang_of(&mut w, &people[..10]);
    assert_eq!(w.comp::<Gang>(g).expect("gang").members.len(), 10);
    w.comp_mut::<Gang>(g).expect("gang").treasury = 15;
    let h = w.hideout_of(g).expect("hideout");
    let import = w.stats.current.flow_import;
    stims::cook(&mut w, g);
    // min(2 x 10 members, 200 - 0, 15 / 1) = 15.
    assert_eq!(w.stock(h, Good::Stims), 15, "cooked into the Hideout");
    assert_eq!(treasury(&w, g), 0, "paid from the treasury");
    assert_eq!(w.stats.current.flow_import - import, 15, "as Flow::Import");
}

#[test]
fn test_deal_pays_gang_and_dealer() {
    let mut w = city();
    w.config.stims.deal_price = 5; // the plan's (phase 5 calibration: 6)
    let people = free(&w);
    let (dealer, buyer) = (people[0], people[1]);
    let g = gang_of(&mut w, &[dealer]);
    let bar = a_bar(&w);
    put_in(&mut w, dealer, bar);
    put_in(&mut w, buyer, bar);
    set_stims(&mut w, dealer, 10);
    set_stims(&mut w, buyer, 0);
    set_coins(&mut w, buyer, 100);
    let dealer_coins = coins(&w, dealer);
    stims::start_deal(&mut w, dealer, bar);
    assert_eq!(stims::stim_source(&w, buyer), Some(bar), "the Bar is a source while the dealer deals");
    // Greed 0.5: round(5 x (0.8 + 0.2)) = 5 a dose.
    assert_eq!(stims::source_price(&w, bar), Some(5));
    let income = w.stats.current.gang_income_dealing;
    assert_eq!(stims::buy_stims(&mut w, buyer, bar), 2, "two doses");
    assert_eq!(coins(&w, buyer), 90);
    assert_eq!(treasury(&w, g), 8, "the gang keeps 5 x 2 less the cut");
    assert_eq!(coins(&w, dealer) - dealer_coins, 2, "the dealer's cut, 1 a dose");
    assert_eq!(stims_of(&w, dealer), 8);
    assert_eq!(stims_of(&w, buyer), 2);
    assert_eq!(w.stats.current.stims_dealt, 2);
    assert_eq!(w.stats.current.gang_income_dealing - income, 10, "dealing income, gross");
    stims::end_deal(&mut w, dealer);
    assert!(w.dealers.is_empty(), "unregistered");
    assert_eq!(stims::stim_source(&w, buyer), None);
}

#[test]
fn test_witnessed_deal_reported_and_arrest_confiscates() {
    let mut w = city();
    w.config.crime.witness_base = 1.0;
    let people = free(&w);
    let (dealer, buyer) = (people[0], people[1]);
    gang_of(&mut w, &[dealer]);
    let bar = a_bar(&w);
    let guard = w.guards()[0];
    put_in(&mut w, dealer, bar);
    put_in(&mut w, buyer, bar);
    put_in(&mut w, guard, bar);
    w.comp_mut::<Skills>(dealer).expect("skills").stealth = 0.0;
    set_stims(&mut w, dealer, 10);
    set_coins(&mut w, buyer, 100);
    stims::start_deal(&mut w, dealer, bar);
    assert_eq!(stims::buy_stims(&mut w, buyer, bar), 2);
    let report = w
        .crime_reports()
        .iter()
        .find(|r| r.suspect == dealer && r.crime == Crime::Dealing && !r.resolved)
        .cloned()
        .expect("a Dealing report");
    assert!(report.witness.is_some(), "a witness filed it");
    assert!(w.stats.current.dealing_reports >= 1);
    assert!(law::wanted(&w, dealer));
    // The guard brings the dealer in: the doses go.
    w.comp_mut::<Brain>(dealer).expect("brain").cuffed_by = Some(guard);
    w.comp_mut::<Brain>(guard).expect("brain").escorting = Some(dealer);
    law::jail_suspect(&mut w, guard, dealer);
    assert_eq!(stims_of(&w, dealer), 0, "confiscated");
}

#[test]
fn test_addiction_rises_per_use_and_withdrawal_from_last_use() {
    let mut w = city();
    w.config.stims.addict_per_use = 0.06; // the plan's (phase 5 calibration: 0.1)
    let a = free(&w)[0];
    w.comp_mut::<Personality>(a).expect("p").lawfulness = 0.5;
    set_body(&mut w, a, 0.0);
    set_stims(&mut w, a, 20);
    assert!(stims::use_stim(&mut w, a));
    let after_one = w.comp::<Body>(a).expect("body").addiction;
    assert!((after_one - 0.06).abs() < 1e-6, "+0.06 x (1.5 - 0.5): {after_one}");
    for _ in 0..8 {
        assert!(stims::use_stim(&mut w, a));
    }
    assert!(stims::is_hooked(&w, a), "nine doses: hooked");
    assert!(stims::is_high(&w, a));
    assert!(!stims::in_withdrawal(&w, a), "just used");
    w.tick += 24 * TICKS_PER_HOUR;
    assert!(stims::in_withdrawal(&w, a), "24 h after the last dose");
    assert!(!stims::is_high(&w, a));
    assert!(stims::use_stim(&mut w, a));
    assert!(!stims::in_withdrawal(&w, a), "one more dose ends it");
}

#[test]
fn test_detox_cuts_addiction() {
    let mut w = city();
    let a = free(&w)[0];
    set_body(&mut w, a, 0.8);
    set_coins(&mut w, a, 500);
    let clinic = w.buildings_of_kind(BuildingKind::Clinic)[0];
    let price = stims::detox_price(&w, clinic);
    let detoxes = w.stats.current.detoxes;
    assert!(stims::detox(&mut w, a, clinic));
    let addiction = w.comp::<Body>(a).expect("body").addiction;
    assert!((addiction - 0.24).abs() < 1e-6, "0.8 x 0.3 = {addiction}");
    assert_eq!(coins(&w, a), 500 - price);
    assert_eq!(w.stats.current.detoxes, detoxes + 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Treated && e.text.contains("Detox")));
}

#[test]
fn test_overdose_only_above_point_eight() {
    let mut w = city();
    w.config.stims.p_overdose = 1.0;
    let people = free(&w);
    let (a, b) = (people[0], people[1]);
    set_body(&mut w, a, 0.79);
    set_body(&mut w, b, 0.8);
    set_stims(&mut w, a, 1);
    set_stims(&mut w, b, 1);
    assert!(stims::use_stim(&mut w, a));
    assert!(law::living(&w, a), "0.79 rolls no overdose");
    assert!(stims::use_stim(&mut w, b));
    assert!(!law::living(&w, b), "0.8 overdoses at p = 1");
    assert_eq!(w.comp::<Corpse>(b).map(|c| c.cause), Some(DeathCause::Overdose));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Overdose && e.actors.first() == Some(&b)));
    assert_eq!(w.stats.current.overdoses, 1);
}

#[test]
fn test_legal_stims_sold_at_market() {
    let mut w = city();
    w.levers.stims_legal = true;
    stims::restock_legal(&mut w);
    let market =
        w.buildings_of_kind(BuildingKind::Market).iter().copied().find(|&m| !w.is_closed(m)).expect("an open Market");
    assert_eq!(w.stock(market, Good::Stims), 300, "restocked to the floor");
    let buyer = free(&w)[0];
    put_in(&mut w, buyer, market);
    set_coins(&mut w, buyer, 100);
    set_stims(&mut w, buyer, 0);
    let level = assets::seller_level(&w, market);
    let price = (6.0 * level).round() as i64;
    assert_eq!(stims::source_price(&w, market), Some(price));
    let reports = w.stats.current.dealing_reports;
    let report_count = w.crime_reports().len();
    assert_eq!(stims::buy_stims(&mut w, buyer, market), 2);
    assert_eq!(coins(&w, buyer), 100 - 2 * price);
    assert_eq!(w.stock(market, Good::Stims), 298);
    assert_eq!(w.stats.current.dealing_reports, reports, "no Dealing raised");
    assert_eq!(w.crime_reports().len(), report_count);
    assert_eq!(w.stats.current.stims_dealt, 0, "not a dealer's sale");
}
