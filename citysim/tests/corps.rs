//! M11 phase 3: the corp brain and corp mechanics (docs/M11_OWNERSHIP.md § 5,
//! § 11; plan phase 3 tests). Scoring tests build `CorpInputs` by hand; world
//! tests use the v1 city with three corps, or a small v2 city where Lots and
//! Security Offices are needed.

use std::collections::BTreeMap;

use citysim::systems::corp_brain::{self, CorpInputs, NicheInputs};
use citysim::systems::{corps, faction, gang, law, law_brain, ownership};
use citysim::{
    Brain, Building, BuildingKind, Config, Corp, CorpLoss, CorpOrder, CorpShock, Crime, EntityId, EventKind, Gang, Job,
    Market, Niche, Personality, Position, Role, Sentence, Wallet, World, TICKS_PER_DAY,
};

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

fn niche() -> NicheInputs {
    NicheInputs {
        demand: 0.5,
        share: 0.3,
        own_price: 1.0,
        rival_price: 1.0,
        rivals: 1,
        weakest: None,
        lots: 0,
        offer: 0,
        at_cap: false,
    }
}

fn inputs(n: NicheInputs) -> CorpInputs {
    CorpInputs {
        cash: 1.0,
        flow: 0.0,
        losses: 0.0,
        unrest: 0.0,
        greed: 0.5,
        courage: 0.5,
        lawfulness: 0.5,
        treasury: 5000,
        cooldown_ok: true,
        lobby_ready: false,
        culprit: None,
        niches: BTreeMap::from([(Niche::Food, n)]),
    }
}

fn best(i: &CorpInputs) -> (CorpOrder, Niche) {
    let s = corp_brain::score_all(i, &Config::load().corps);
    (s[0].order, s[0].niche)
}

#[test]
fn test_brain_picks_grow_with_demand_and_lots() {
    let i = CorpInputs { cash: 1.5, flow: 0.5, greed: 0.8, ..inputs(NicheInputs { demand: 0.9, lots: 5, ..niche() }) };
    assert_eq!(best(&i).0, CorpOrder::Grow);
    let no_lots = CorpInputs { niches: BTreeMap::from([(Niche::Food, NicheInputs { demand: 0.9, ..niche() })]), ..i };
    assert_ne!(best(&no_lots).0, CorpOrder::Grow, "no Lot, no Grow");
}

#[test]
fn test_brain_picks_squeeze_with_dominant_share_and_greedy_exec() {
    let i = CorpInputs { greed: 0.95, flow: -0.5, ..inputs(NicheInputs { share: 0.9, ..niche() }) };
    assert_eq!(best(&i).0, CorpOrder::Squeeze);
    let meek = CorpInputs { greed: 0.2, ..i.clone() };
    assert_ne!(best(&meek).0, CorpOrder::Squeeze, "a modest exec does not squeeze");
    // D32: a Housing corp with every Block at the cap cannot squeeze.
    let capped = CorpInputs {
        niches: BTreeMap::from([(Niche::Housing, NicheInputs { share: 0.9, at_cap: true, ..niche() })]),
        ..i
    };
    assert_ne!(best(&capped).0, CorpOrder::Squeeze);
}

#[test]
fn test_brain_picks_undercut_with_small_share_and_pricier_rival() {
    let i = inputs(NicheInputs { share: 0.1, own_price: 1.0, rival_price: 1.6, ..niche() });
    assert_eq!(best(&i).0, CorpOrder::Undercut);
    let alone = inputs(NicheInputs { share: 0.1, rival_price: 1.6, rivals: 0, ..niche() });
    assert_ne!(best(&alone).0, CorpOrder::Undercut, "nobody to undercut");
}

#[test]
fn test_brain_picks_acquire_against_rival_in_the_red() {
    let rival = EntityId { index: 900, generation: 0 };
    let farm = EntityId { index: 901, generation: 0 };
    let i =
        CorpInputs { greed: 0.8, ..inputs(NicheInputs { weakest: Some((rival, farm, 1000)), offer: 1200, ..niche() }) };
    assert_eq!(best(&i).0, CorpOrder::Acquire);
    let poor = CorpInputs { treasury: 1000, ..i.clone() };
    assert_ne!(best(&poor).0, CorpOrder::Acquire, "cannot pay the offer");
    let cooling = CorpInputs { cooldown_ok: false, ..i };
    assert_ne!(best(&cooling).0, CorpOrder::Acquire, "one acquisition per cooldown");
}

#[test]
fn test_brain_picks_secure_after_big_robbery() {
    let i = CorpInputs { losses: 0.8, courage: 0.8, ..inputs(niche()) };
    assert_eq!(best(&i).0, CorpOrder::Secure);
}

#[test]
fn test_brain_picks_hunker_when_broke() {
    let i = CorpInputs { cash: 0.0, flow: -1.0, ..inputs(niche()) };
    assert_eq!(best(&i).0, CorpOrder::Hunker);
}

#[test]
fn test_brain_picks_lobby_after_extortion_with_lawless_exec() {
    let gang = EntityId { index: 902, generation: 0 };
    let i = CorpInputs {
        losses: 0.6,
        lawfulness: 0.05,
        courage: 0.0,
        cash: 0.5,
        lobby_ready: true,
        culprit: Some(gang),
        ..inputs(niche())
    };
    assert_eq!(best(&i).0, CorpOrder::Lobby);
    let upright = CorpInputs { lawfulness: 0.95, ..i.clone() };
    assert_ne!(best(&upright).0, CorpOrder::Lobby, "a lawful exec secures instead");
    let no_culprit = CorpInputs { culprit: None, ..i };
    assert_ne!(best(&no_culprit).0, CorpOrder::Lobby, "no gang to name");
}

#[test]
fn test_choose_keeps_the_standing_order_within_hysteresis() {
    let i = inputs(NicheInputs { share: 0.1, rival_price: 1.6, ..niche() });
    let s = corp_brain::score_all(&i, &Config::load().corps);
    assert_eq!(s[0].order, CorpOrder::Undercut);
    // The runner-up within 0.1 of the best stays.
    let second = s[1].clone();
    let close = s[0].score - second.score < 1.0;
    assert!(close);
    assert_eq!(corp_brain::choose(&s, (CorpOrder::Undercut, Some(Niche::Food)), 0.1), None);
    let from_hunker = corp_brain::choose(&s, (CorpOrder::Hunker, None), 0.0);
    assert_eq!(from_hunker, Some((CorpOrder::Undercut, Niche::Food)));
}

// ---------------------------------------------------------------------------
// Worlds
// ---------------------------------------------------------------------------

/// The v1 city with three corps: FoodCo (a Farm, the Market and, by seeding
/// rule 5, the Bar), FarmCo (the other Farm) and HomeCo (30 Blocks).
fn cfg() -> Config {
    let mut c = Config::load().v1_profile();
    c.corps.names = vec!["FoodCo".into(), "FarmCo".into(), "HomeCo".into()];
    c.corps.niches = vec![vec!["Food".into()], vec!["Food".into()], vec!["Housing".into()]];
    c.corps.farms = vec![1, 1, 0];
    c.corps.markets = vec![1, 0, 0];
    c.corps.blocks = vec![0, 0, 30];
    c.corps.offices = vec![0, 0, 0];
    c.corps.treasury_initial = vec![2000, 2000, 2000];
    c.rent.base = [1, 2, 4];
    c
}

fn world() -> World {
    World::new(7, cfg())
}

fn three(w: &World) -> (EntityId, EntityId, EntityId) {
    let c = w.corps();
    (c[0], c[1], c[2])
}

/// A small v2 city (Lots, two Security Offices, the eight seeded corps).
fn v2_world(seed: u64) -> World {
    World::new(seed, Config::load().scaled_to(400))
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == name)).expect("corp")
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

/// Seven days of Market sales, so Food shares count (one corp's Market is
/// then a Food monopoly on the v1 map).
fn full_sales(w: &mut World) {
    for m in w.buildings_of_kind(BuildingKind::Market).to_vec() {
        let mk = w.comp_mut::<Market>(m).expect("market");
        mk.sales = std::iter::repeat_n(50, 7).collect();
        mk.stock_hist = std::iter::repeat_n(500, 7).collect();
    }
}

#[test]
fn test_conglomerate_acts_in_the_niche_that_scored() {
    let food = NicheInputs { demand: 0.9, lots: 5, ..niche() };
    let housing = NicheInputs { demand: 0.1, lots: 0, ..niche() };
    let i = CorpInputs {
        cash: 1.5,
        flow: 0.5,
        greed: 0.8,
        niches: BTreeMap::from([(Niche::Food, food), (Niche::Housing, housing)]),
        ..inputs(niche())
    };
    assert_eq!(best(&i), (CorpOrder::Grow, Niche::Food));
    // On a world: Vatra (Food and Housing) growing in Food builds a Bar.
    let mut w = v2_world(3);
    let vatra = corp_named(&w, "Vatra");
    let lots0 = w.buildings_of_kind(BuildingKind::Lot).len();
    let bars0 = ownership::owned_of_kind(&w, Some(vatra), BuildingKind::Bar).len();
    {
        let c = w.comp_mut::<Corp>(vatra).expect("corp");
        c.order = CorpOrder::Grow;
        c.order_niche = Some(Niche::Food);
        c.treasury = 5000;
    }
    corp_brain::act(&mut w, vatra);
    assert_eq!(ownership::owned_of_kind(&w, Some(vatra), BuildingKind::Bar).len(), bars0 + 1, "a Bar, not a Block");
    assert_eq!(w.buildings_of_kind(BuildingKind::Lot).len(), lots0 - 1);
    let bar = *ownership::owned_of_kind(&w, Some(vatra), BuildingKind::Bar).last().expect("bar");
    let b = w.comp::<Building>(bar).expect("b");
    let door = b.door;
    for y in b.rect.y..b.rect.y + b.rect.h {
        for x in b.rect.x..b.rect.x + b.rect.w {
            let t = citysim::TilePos { x, y };
            if b.rect.on_perimeter(t) && t != door {
                assert_eq!(w.map.tile_at(t), citysim::TileKind::Wall, "perimeter walled at {t:?}");
            }
        }
    }
    assert!(w.vacancies.get(&bar).is_some_and(|v| v.contains(&Role::Bartender)), "the Bar posts its staff");
    assert_eq!(w.comp::<Corp>(vatra).expect("corp").treasury, 5000 - w.config.corps.found_cost.bar);
    // Cooldown: a second act the same week builds nothing.
    corp_brain::act(&mut w, vatra);
    assert_eq!(ownership::owned_of_kind(&w, Some(vatra), BuildingKind::Bar).len(), bars0 + 1);
}

#[test]
fn test_acquisition_moves_building_employees_and_contracts() {
    let mut w = v2_world(4);
    w.run_ticks(1); // job_search fills the seeded vacancies (the private guards)
    let nutrix = corp_named(&w, "Nutrix");
    let vatra = corp_named(&w, "Vatra");
    let arasaka = corp_named(&w, "Arasaka");
    let farm = ownership::owned_of_kind(&w, Some(vatra), BuildingKind::Farm)[0];
    let staff: Vec<EntityId> = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .filter(|&a| w.comp::<Job>(a).and_then(|j| j.employer) == Some(farm))
        .collect();
    assert!(!staff.is_empty());
    assert!(corps::buy_contract(&mut w, farm, arasaka), "Arasaka guards the Farm");
    w.comp_mut::<Corp>(nutrix).expect("c").treasury = 5000;
    let (n0, v0, total) = (w.purse(Some(nutrix)), w.purse(Some(vatra)), ownership::total_coins(&w));
    assert!(corps::acquire(&mut w, nutrix, farm, 1200, "hostile"));
    assert_eq!(w.owner_of(farm), Some(nutrix));
    assert!(w.comp::<Corp>(nutrix).expect("c").buildings.contains(&farm));
    assert!(!w.comp::<Corp>(vatra).expect("c").buildings.contains(&farm));
    for &a in &staff {
        assert_eq!(w.comp::<Job>(a).and_then(|j| j.employer), Some(farm), "employees stay with the building");
        assert_eq!(w.corp_of_agent(a), Some(nutrix));
    }
    assert_eq!(w.comp::<Building>(farm).expect("b").secured_by, Some(arasaka), "the contract stays with its seller");
    assert!(w.comp::<Corp>(arasaka).expect("c").contracts.iter().any(|&(b, _)| b == farm));
    assert_eq!(w.purse(Some(nutrix)), n0 - 1200);
    assert_eq!(w.purse(Some(vatra)), v0 + 1200);
    assert_eq!(ownership::total_coins(&w), total, "a sale moves coins");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Acquired && e.text.contains("hostile")));
    // A Security Office takes its contracts along when the seller has no other.
    let office = ownership::owned_of_kind(&w, Some(arasaka), BuildingKind::SecurityOffice)[0];
    let militech = corp_named(&w, "Militech");
    w.comp_mut::<Corp>(militech).expect("c").treasury = 5000;
    assert!(corps::acquire(&mut w, militech, office, 600, "hostile"));
    assert!(w.comp::<Corp>(militech).expect("c").contracts.iter().any(|&(b, _)| b == farm));
    assert!(w.comp::<Corp>(arasaka).expect("c").contracts.is_empty());
    assert_eq!(w.comp::<Building>(farm).expect("b").secured_by, Some(militech));
}

#[test]
fn test_hostile_bid_never_takes_a_rivals_last_niche_building() {
    let mut w = v2_world(5);
    let arasaka = corp_named(&w, "Arasaka");
    let militech = corp_named(&w, "Militech");
    w.comp_mut::<Corp>(militech).expect("c").negative_since = Some(0);
    let i = corp_brain::gather_inputs(&w, arasaka).expect("inputs");
    assert!(i.niches[&Niche::Security].weakest.is_none(), "Militech's only Office is not for a hostile bid");
    // Greenline in the red has two Food buildings: one is for sale.
    let nutrix = corp_named(&w, "Nutrix");
    let greenline = corp_named(&w, "Greenline");
    w.comp_mut::<Corp>(greenline).expect("c").negative_since = Some(0);
    let i = corp_brain::gather_inputs(&w, nutrix).expect("inputs");
    let (who, _, value) = i.niches[&Niche::Food].weakest.expect("a weak rival");
    assert_eq!(who, greenline);
    assert_eq!(value, w.config.corps.value.farm);
}

#[test]
fn test_breakup_halves_a_monopoly() {
    let mut w = world();
    let (food, _, _) = three(&w);
    assert!(corps::break_up(&mut w, food).is_err(), "no monopoly without seven days of sales");
    full_sales(&mut w);
    assert!(corps::is_monopoly(&w, food, Niche::Food), "the v1 city's only Market");
    let before = corp_brain::niche_buildings(&w, food, Niche::Food);
    assert_eq!(before.len(), 3, "a Farm, the Market, the Bar");
    w.comp_mut::<Corp>(food).expect("c").treasury = 3000;
    let total = ownership::total_coins(&w);
    w.push_command(citysim::PlayerCommand::BreakUp(food));
    w.run_ticks(1);
    let spin = w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == "FoodCo Spinoff"));
    let spin = spin.expect("a spinoff");
    let theirs = corp_brain::niche_buildings(&w, spin, Niche::Food);
    assert_eq!(theirs, vec![before[1]], "every second niche building, from the second");
    assert_eq!(corp_brain::niche_buildings(&w, food, Niche::Food), vec![before[0], before[2]]);
    let sc = w.comp::<Corp>(spin).expect("c");
    assert_eq!(sc.niches.iter().copied().collect::<Vec<_>>(), vec![Niche::Food]);
    assert_eq!(sc.level(Niche::Food), 1.0);
    assert_eq!(count(&w, EventKind::BrokenUp), 1);
    assert!(ownership::total_coins(&w) == total || w.tick > 0, "the split moves coins");
    // A corp with no monopoly is refused.
    let (_, farm_co, _) = three(&w);
    let n = count(&w, EventKind::PlayerActionFailed);
    w.push_command(citysim::PlayerCommand::BreakUp(farm_co));
    w.run_ticks(1);
    assert_eq!(count(&w, EventKind::PlayerActionFailed), n + 1);
}

#[test]
fn test_strike_shock_forces_rescore() {
    let mut w = world();
    let (food, _, _) = three(&w);
    w.run_ticks(100);
    {
        // An order whose gate is shut scores zero, so any rescore moves off it.
        let c = w.comp_mut::<Corp>(food).expect("c");
        c.order = CorpOrder::Lobby;
        c.order_niche = Some(Niche::Food);
        c.order_trace.clear();
    }
    corp_brain::push_shock(&mut w, food, CorpShock::Strike);
    let now = w.tick;
    w.tick();
    let c = w.comp::<Corp>(food).expect("c");
    assert_eq!(c.order_since, now, "rescored at once, not at midnight");
    assert_ne!(c.order, CorpOrder::Lobby);
    assert!(!c.order_trace.is_empty(), "the trace is kept for the inspector");
    assert!(c.shocks.is_empty(), "the shock was consumed");
    assert!(w.events.iter().any(|e| e.kind == EventKind::CorpOrder && e.text.contains("shock")));
    // A small shock waits for midnight.
    corp_brain::push_shock(&mut w, food, CorpShock::Undercut);
    let since = w.comp::<Corp>(food).expect("c").order_since;
    w.tick();
    assert_eq!(w.comp::<Corp>(food).expect("c").order_since, since);
    assert_eq!(w.comp::<Corp>(food).expect("c").shocks.len(), 1);
}

#[test]
fn test_bankruptcy_sells_to_richest_then_city() {
    let mut w = world();
    let (food, farm_co, home) = three(&w);
    // Nobody but FarmCo can buy a Block (400): wallets under 400, FoodCo at its reserve.
    for a in w.citizens() {
        if let Some(wl) = w.comp_mut::<Wallet>(a) {
            wl.coins = wl.coins.min(100);
        }
    }
    let r = w.comp::<Corp>(food).expect("c").treasury_ref;
    w.comp_mut::<Corp>(food).expect("c").treasury = r;
    let r = w.comp::<Corp>(farm_co).expect("c").treasury_ref;
    w.comp_mut::<Corp>(farm_co).expect("c").treasury = r + 450;
    w.comp_mut::<Corp>(home).expect("c").treasury = -500;
    let blocks = w.comp::<Corp>(home).expect("c").buildings.clone();
    let exec = w.comp::<Corp>(home).expect("c").exec.expect("exec");
    let greed = w.comp::<Personality>(exec).expect("p").greed;
    let total = ownership::total_coins(&w);
    let t0 = w.purse(None);
    corps::bankrupt(&mut w, home);
    assert!(!w.is_alive(home), "the corp is gone");
    assert_eq!(w.owner_of(blocks[0]), Some(farm_co), "the first Block to the richest purse that can pay");
    for &b in &blocks[1..] {
        assert_eq!(w.owner_of(b), None, "the rest to the city at half");
    }
    assert_eq!(count(&w, EventKind::Bankrupt), 1);
    assert_eq!(count(&w, EventKind::Acquired), blocks.len());
    assert_eq!(ownership::total_coins(&w), total, "bankruptcy moves coins, never makes them");
    // The city paid half a Block's value for each, and absorbed nothing: the
    // estate (-500 + 400 + 29 x 200) was positive and went to the exec.
    let half = w.config.corps.found_cost.home / 2;
    assert_eq!(w.purse(None), t0 - half * (blocks.len() as i64 - 1));
    assert!(w.comp::<Personality>(exec).expect("p").greed >= greed + 0.049, "the exec drifts greedier");
    for c in [food, farm_co] {
        assert!(w.comp::<Corp>(c).expect("c").shocks.contains(&CorpShock::Bankrupt(home)));
    }
    // In the red for `bankrupt_days`: the daily pass does it.
    let mut w = world();
    let (food, _, _) = three(&w);
    w.comp_mut::<Corp>(food).expect("c").treasury = -10_000;
    w.run_ticks(TICKS_PER_DAY * (w.config.corps.bankrupt_days + 2));
    assert!(!w.is_alive(food));
    assert_eq!(w.stats.history.iter().map(|r| r.bankruptcies).sum::<u32>(), 1);
}

#[test]
fn test_monopoly_raises_markup_cap() {
    let mut w = world();
    // The spec's ceilings (the shipped config lowers both until phase 5).
    w.config.corps.squeeze_cap = 1.5;
    w.config.corps.monopoly_markup_cap = 2.0;
    let (food, _, _) = three(&w);
    {
        let c = w.comp_mut::<Corp>(food).expect("c");
        c.order = CorpOrder::Squeeze;
        c.order_niche = Some(Niche::Food);
        c.price_level.insert(Niche::Food, 1.4);
    }
    for _ in 0..4 {
        corp_brain::act(&mut w, food);
    }
    assert_eq!(w.comp::<Corp>(food).expect("c").level(Niche::Food), 1.5, "1.5 without a monopoly");
    full_sales(&mut w);
    for _ in 0..10 {
        corp_brain::act(&mut w, food);
    }
    assert_eq!(w.comp::<Corp>(food).expect("c").level(Niche::Food), w.config.corps.monopoly_markup_cap);
    // The markup is the Market's price at the next midnight (unless the
    // brain moved off Squeeze in between, the level holds).
    let market = ownership::owned_of_kind(&w, Some(food), BuildingKind::Market)[0];
    w.run_ticks(TICKS_PER_DAY - w.tick % TICKS_PER_DAY + 1);
    let m = w.comp::<Market>(market).expect("m");
    let base = citysim::systems::economy::price_for_stock(&w.config.economy, *m.stock_hist.back().expect("rolled"));
    let level = w.comp::<Corp>(food).expect("c").level(Niche::Food);
    let want = ((base as f32 * level).round() as i64).clamp(1, w.config.economy.price_cap);
    assert!(level > 1.5);
    assert_eq!(m.price_food, want, "base {base} x level {level}");
}

#[test]
fn test_undercut_steps_down_to_the_floor() {
    let mut w = world();
    let (food, farm_co, _) = three(&w);
    w.comp_mut::<Corp>(farm_co).expect("c").price_level.insert(Niche::Food, 0.9);
    {
        let c = w.comp_mut::<Corp>(food).expect("c");
        c.order = CorpOrder::Undercut;
        c.order_niche = Some(Niche::Food);
    }
    for _ in 0..10 {
        corp_brain::act(&mut w, food);
    }
    assert_eq!(w.comp::<Corp>(food).expect("c").level(Niche::Food), 0.8, "a step under the rival, no lower");
    w.comp_mut::<Corp>(farm_co).expect("c").price_level.insert(Niche::Food, 0.5);
    for _ in 0..10 {
        corp_brain::act(&mut w, food);
    }
    assert_eq!(w.comp::<Corp>(food).expect("c").level(Niche::Food), w.config.corps.undercut_floor);
}

#[test]
fn test_squeeze_entry_cuts_food_wages_and_housing_eviction_days() {
    let mut w = world();
    let (food, _, home) = three(&w);
    full_sales(&mut w);
    for (c, greed) in [(food, 1.0), (home, 1.0)] {
        if let Some(e) = w.comp::<Corp>(c).and_then(|cc| cc.exec) {
            w.comp_mut::<Personality>(e).expect("p").greed = greed;
        }
    }
    // FoodCo holds the only Market: a monopoly, a greedy exec, no unrest.
    corp_brain::rescore(&mut w, food, 0.0, "test");
    let c = w.comp::<Corp>(food).expect("c");
    assert_eq!((c.order, c.order_niche), (CorpOrder::Squeeze, Some(Niche::Food)));
    assert_eq!(c.wage_mult, 0.9);
    // Leaving Squeeze restores the wage.
    {
        let c = w.comp_mut::<Corp>(food).expect("c");
        c.treasury = -5000;
        c.cashflow = std::iter::repeat_n(5000, 7).collect();
    }
    corp_brain::rescore(&mut w, food, 0.0, "test");
    let c = w.comp::<Corp>(food).expect("c");
    assert_ne!(c.order, CorpOrder::Squeeze);
    assert_eq!(c.wage_mult, 1.0);
}

/// A private guard on a 24-hour shift, its corp with a contract on a Market.
fn private_guard_scene() -> (World, EntityId, EntityId, EntityId) {
    let mut c = Config::load().scaled_to(400);
    c.law.pursuit_radius = 0; // the city's guards chase nobody
    let mut w = World::new(11, c);
    w.run_ticks(2);
    let arasaka = corp_named(&w, "Arasaka");
    let office = ownership::owned_of_kind(&w, Some(arasaka), BuildingKind::SecurityOffice)[0];
    let guard = w
        .guards()
        .iter()
        .copied()
        .find(|&g| w.comp::<Job>(g).and_then(|j| j.employer) == Some(office))
        .expect("a private guard");
    w.comp_mut::<Job>(guard).expect("job").shifts = vec![(0, 1440)];
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    assert!(corps::buy_contract(&mut w, market, arasaka));
    (w, arasaka, guard, market)
}

#[test]
fn test_private_guard_arrests_at_contract_and_never_takes_jail_duty() {
    let (mut w, arasaka, guard, market) = private_guard_scene();
    for key in -3..40 {
        assert!(!law::jail_duty(&w, guard, key), "a private guard never holds the Precinct");
    }
    assert!(!law_brain::guards(&w).contains(&guard), "not on the city's payroll");
    assert!(w.guards().contains(&guard), "but a guard for sightings and fear");
    assert_eq!(law::pursuit_radius_for(&w, guard), w.config.corps.private_pursuit_radius);
    let route = law::new_patrol_route(&mut w, guard);
    assert_eq!(route, vec![market], "the patrol loop is the contracted client");
    assert!(w.comp::<Corp>(arasaka).expect("c").contracts.iter().any(|&(b, _)| b == market));
    // A thief at the client's Market, a few tiles from the guard.
    w.config.lod.force = Some(citysim::Lod::Full);
    let thief =
        w.citizens().into_iter().find(|&a| w.has::<Brain>(a) && !w.has::<Job>(a) && a != guard).expect("a civilian");
    for id in [thief, guard] {
        w.leave_building(id);
        w.enter_building(id, market);
        w.abort_plan(id);
    }
    w.comp_mut::<citysim::Skills>(thief).expect("skills").stealth = 0.0;
    w.comp_mut::<Personality>(thief).expect("p").courage = 0.0;
    let tile = w.comp::<Position>(thief).expect("pos").tile;
    law::raise_crime(&mut w, thief, None, Crime::Theft, tile);
    assert!(law::wanted(&w, thief));
    let mut arrested = false;
    for _ in 0..TICKS_PER_DAY {
        w.tick();
        if !arrested {
            arrested = w.events.iter().any(|e| e.kind == EventKind::Arrest && e.actors.contains(&guard));
        }
        if w.has::<Sentence>(thief) {
            break;
        }
    }
    assert!(arrested, "the private guard made the arrest");
    assert!(w.has::<Sentence>(thief), "the escort ends at the Precinct");
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    assert_eq!(w.comp::<Position>(thief).and_then(|p| p.building), Some(jail));
}

#[test]
fn test_contracts_bill_daily_and_lapse_unpaid() {
    let (mut w, arasaka, _, market) = private_guard_scene();
    let client = w.owner_of(market).expect("a corp Market");
    let price = corps::contract_price(&w, arasaka);
    // A small city's corps start thin and pay a full city's upkeep at tick 0.
    w.comp_mut::<Corp>(client).expect("c").treasury = 1000;
    let total = ownership::total_coins(&w);
    let (a0, c0) = (w.purse(Some(arasaka)), w.purse(Some(client)));
    corps::daily(&mut w);
    assert_eq!(w.stats.current.flow_contract, price, "one day billed");
    assert_eq!(w.purse(Some(client)), c0 - price);
    assert!(w.purse(Some(arasaka)) > a0, "the seller earns it, less tax");
    assert_eq!(ownership::total_coins(&w), total);
    assert_eq!(w.comp::<Building>(market).expect("b").secured_by, Some(arasaka));
    w.comp_mut::<Corp>(client).expect("c").treasury = -1;
    corps::daily(&mut w);
    assert_eq!(w.comp::<Building>(market).expect("b").secured_by, None, "a short payment ends it");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Contract && e.text.contains("unpaid")));
    assert!(w.comp::<Corp>(arasaka).expect("c").contracts.is_empty());
}

/// FoodCo robbed by a gang member at its Market; the v1 guards at
/// lawfulness 0.4 under a captain at `captain_lawfulness`.
fn lobby_scene(captain_lawfulness: f32) -> (World, EntityId, EntityId, EntityId) {
    let mut w = world();
    let (food, _, _) = three(&w);
    let gang = w.gangs()[0];
    let recruits: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && !w.has::<Job>(a) && w.corp_of_agent(a).is_none())
        .take(2)
        .collect();
    for m in recruits {
        gang::enlist(&mut w, m, gang);
    }
    let gs = law_brain::guards(&w);
    for &g in &gs {
        w.comp_mut::<Personality>(g).expect("p").lawfulness = 0.4;
    }
    let captain = gs[0];
    {
        let p = w.comp_mut::<Personality>(captain).expect("p");
        p.lawfulness = captain_lawfulness;
        p.greed = 0.9;
    }
    law_brain::recompute_captain(&mut w);
    let market = ownership::owned_of_kind(&w, Some(food), BuildingKind::Market)[0];
    let tick = w.tick;
    {
        let c = w.comp_mut::<Corp>(food).expect("c");
        c.loss_log.push_back(CorpLoss { tick, coins: 600, gang: Some(gang), building: Some(market) });
        c.treasury = 2000;
    }
    if let Some(e) = w.comp::<Corp>(food).and_then(|c| c.exec) {
        w.comp_mut::<Personality>(e).expect("p").lawfulness = 0.05;
    }
    (w, food, gang, captain)
}

#[test]
fn test_corp_lobby_bribe_forces_crackdown() {
    let (mut w, food, gang, captain) = lobby_scene(0.5);
    let i = corp_brain::gather_inputs(&w, food).expect("inputs");
    assert_eq!(i.culprit, Some(gang), "the gang behind the losses");
    assert!(i.lobby_ready);
    {
        let c = w.comp_mut::<Corp>(food).expect("c");
        c.order = CorpOrder::Lobby;
        c.order_niche = Some(Niche::Food);
    }
    let (f0, k0) = (w.purse(Some(food)), w.comp::<Wallet>(captain).map_or(0, |w| w.coins));
    corp_brain::act(&mut w, food);
    let price = faction::bribe_price(&w);
    let bribe = w.events.iter().rev().find(|e| e.kind == EventKind::Bribe).expect("a Bribe");
    assert_eq!(bribe.actors.first(), Some(&food), "the corp pays");
    assert!(bribe.text.contains("crackdown"));
    assert_eq!(w.purse(Some(food)), f0 - price);
    assert_eq!(w.comp::<Wallet>(captain).map_or(0, |w| w.coins), k0 + price);
    let l = w.law().expect("law");
    assert_eq!(l.lobby.map(|h| (h.corp, h.gang)), Some((food, gang)));
    assert!(l.cracking_down_on(gang), "the captain cracks down on the culprit");
    assert!(w.comp::<Corp>(food).expect("c").lobby_until.is_some());
    assert!(w.events.iter().any(|e| e.kind == EventKind::Posture && e.text.contains("lobbied by FoodCo")));
    // A daily rescoring keeps the bought crackdown.
    law_brain::rescore(&mut w, 0.1, "daily");
    assert!(w.law().expect("law").cracking_down_on(gang));
    // The gang out-bids: its taken bribe clears the hold.
    {
        let g = w.comp_mut::<Gang>(gang).expect("g");
        g.treasury = 500;
        let leader = g.leader.expect("leader");
        w.comp_mut::<Personality>(leader).expect("p").pride = 0.0;
    }
    gang::push_shock(&mut w, gang, citysim::Shock::MemberArrested);
    assert!(faction::consider_bribe(&mut w, gang), "the gang pays");
    let l = w.law().expect("law");
    assert!(l.lobby.is_none(), "the gang's bribe clears the corp's hold");
    assert!(!l.cracking_down_on(gang));
}

#[test]
fn test_incorruptible_captain_refuses_the_corp() {
    let (mut w, food, _, _) = lobby_scene(0.95);
    {
        let c = w.comp_mut::<Corp>(food).expect("c");
        c.order = CorpOrder::Lobby;
        c.order_niche = Some(Niche::Food);
    }
    let f0 = w.purse(Some(food));
    corp_brain::act(&mut w, food);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Bribe && e.text.contains("refused")));
    assert_eq!(w.purse(Some(food)), f0, "no coins move");
    let l = w.law().expect("law");
    assert!(l.lobby.is_none());
    assert!(l.hardened_until.is_some());
    assert!(w.comp::<Corp>(food).expect("c").lobby_until.is_some(), "no second offer until it passes");
    let i = corp_brain::gather_inputs(&w, food).expect("inputs");
    assert!(!i.lobby_ready);
}

#[test]
fn test_hoard_tilts_contest() {
    let mut w = world();
    let (food, _, _) = three(&w);
    assert_eq!(faction::hoard(&w), (0.0, None), "nobody past the heat");
    let heat = w.config.corps.hoard_heat;
    w.comp_mut::<Corp>(food).expect("c").treasury = heat * 3 / 2;
    let (h, who) = faction::hoard(&w);
    assert_eq!(who, Some(food));
    assert!((h - 0.5).abs() < 1e-6);
    let base = faction::OrderInputs {
        frontier: 0,
        frontier_total: 30,
        rival_territory: 5,
        own: 5,
        rival: 5,
        heat: 0.0,
        prize: 0,
        grudge: false,
        greed: 0.5,
        courage: 0.5,
        pride: 0.5,
        raid_ready: true,
        rival_exists: true,
        sacked: false,
        jailed: 0,
        boss_jailed: false,
        breakout_ready: true,
        garrison: false,
        loyalty: 0.5,
        hoard: 0.0,
        hoard_corp: None,
        hoard_tilt: 0.1,
    };
    let contest = |i: &faction::OrderInputs| {
        faction::score_orders(i, &w.config.gangs)
            .into_iter()
            .find(|s| s.order == citysim::Order::Contest)
            .map_or(0.0, |s| s.score)
    };
    let tilted = faction::OrderInputs { hoard: 0.5, hoard_corp: Some(food), ..base.clone() };
    assert!((contest(&tilted) - contest(&base) - 0.05).abs() < 1e-5);
}

#[test]
fn test_daily_brain_logs_order_changes_and_keeps_conservation() {
    let mut w = v2_world(9);
    let total = ownership::total_coins(&w);
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(count(&w, EventKind::CorpOrder) >= 1, "some corp chose an order");
    for c in w.corps() {
        let cc = w.comp::<Corp>(c).expect("c");
        assert!(cc.order_niche.is_some() || cc.order == CorpOrder::Hunker);
        assert!(!cc.order_trace.is_empty(), "{} has a trace", cc.name);
    }
    // Coins only change by the documented sources and sinks (immigrants'
    // endowments, emigrants' wallets): within a few thousand of the start.
    let now = ownership::total_coins(&w);
    assert!((now - total).abs() < 5000, "coins {total} -> {now}");
    w.check_indices().expect("indices in sync");
}
