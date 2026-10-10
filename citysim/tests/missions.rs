//! M16a phase 2 (plan 2.6): missions, squads, the strike decision and its
//! political cost, `Trespass`, sell-outs and gang jobs. Every mission,
//! squad, strike and sell-out here is game state between fictional agents
//! of a simulated city: a crew list marching to a door, a deterministic
//! score over summed strengths, and the raid machinery's seeded dice
//! (`raid::fight_out`).

use citysim::contract::{ContractKind, ContractStatus, Decision, Origin, Posting, Render, Target};
use citysim::systems::econ::identity;
use citysim::systems::{contracts, demography, gang, hunt, law, lod, missions, raid};
use citysim::word::{Deed, Regard, Reputation};
use citysim::DeathCause;
use citysim::{
    Brain, Config, Controller, EntityId, EventKind, Gang, GangMember, GoalKind, Lod, Personality, Position, RelKind,
    Shock, Skills, TilePos, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};
use rand::Rng;

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

/// Fighting and courage (strength = fighting + 0.25 × courage, no Kit).
fn set_fighter(w: &mut World, id: EntityId, fighting: f32, courage: f32) {
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = fighting;
    }
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.courage = courage;
    }
    w.remove::<citysim::Kit>(id);
}

/// No Friend edges (no expected allies), standing 0.2.
fn lone(w: &mut World, t: EntityId) {
    let friends: Vec<EntityId> =
        w.neighbours(t).filter(|&o| w.edge(t, o).is_some_and(|e| e.kind == RelKind::Friend)).collect();
    for f in friends {
        w.remove_edge(t, f);
    }
    w.insert(t, Reputation { standing: 0.2, ..Default::default() });
}

fn lawless(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.lawfulness = 0.0;
        p.loyalty = 1.0;
    }
}

fn direct(buyer: EntityId, kind: ContractKind, target: EntityId, broker: Option<EntityId>) -> Posting {
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

fn tile_of(w: &World, id: EntityId) -> TilePos {
    w.comp::<Position>(id).map(|p| p.tile).unwrap_or_default()
}

/// The district at `tile` held by `g` at `share`.
fn hold_district(w: &mut World, tile: TilePos, g: EntityId, share: f32) {
    let d = w.district_of(tile).index();
    w.districts[d].control = Controller::Gang(g);
    w.districts[d].control_share = share;
}

/// A gang ready to take a job (the seeded gangs start empty): `n`
/// strangers enlisted, its leader ranked, a greedy lawless leader, every
/// member lawless and loyal, no raid mustering.
fn ready_gang(w: &mut World, g: EntityId, n: usize) -> Vec<EntityId> {
    for r in strangers(w, n) {
        gang::enlist(w, r, g);
    }
    gang::recompute_leader(w, g);
    let members = w.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
    for &m in &members {
        lawless(w, m);
    }
    if let Some(l) = w.comp::<Gang>(g).and_then(|x| x.leader) {
        if let Some(p) = w.comp_mut::<Personality>(l) {
            p.greed = 1.0;
        }
    }
    if let Some(x) = w.comp_mut::<Gang>(g) {
        x.raid_at = None;
        x.strike = None;
    }
    members
}

#[test]
fn test_strike_table() {
    // The plan's Strike table (C24): strike_k 3.0, loss_w 0.5, pol_w 1.0.
    let cfg = Config::load().missions;
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    let r = |value, fear| Regard { value, fear };
    // A: a solo gun (fi 0.6, co 0.5) vs a lone target (fi 0.4, co 0.4), City.
    let (p, pol, ev) = missions::strike_numbers(&cfg, 0.725, 0.5, None);
    assert!(near(p, 0.794) && near(pol, 0.0) && near(ev, 0.691), "A: {p} {pol} {ev}");
    // B: as A in a gang-held district (share 0.6, fear 0.5, value 0.2).
    let (p, pol, ev) = missions::strike_numbers(&cfg, 0.725, 0.5, Some((0.6, r(0.2, 0.5), false)));
    assert!(near(p, 0.794) && near(pol, 0.54) && near(ev, 0.151), "B: {p} {pol} {ev}");
    assert!(ev >= 0.0, "B strikes");
    // C: as B, the target a member of the controller: doubled, a Hold.
    let (p, pol, ev) = missions::strike_numbers(&cfg, 0.725, 0.5, Some((0.6, r(0.2, 0.5), true)));
    assert!(near(p, 0.794) && near(pol, 1.08) && near(ev, -0.389), "C: {p} {pol} {ev}");
    // D: a squad of 3 vs the target and two Friends (share 0.8, fear 0.7, value -0.3).
    let (p, pol, ev) = missions::strike_numbers(&cfg, 2.0, 1.6, Some((0.8, r(-0.3, 0.7), false)));
    assert!(near(p, 0.679) && near(pol, 0.68) && near(ev, -0.161), "D: {p} {pol} {ev}");
    // E: a weak gun (0.4, 0.3) vs a strong target (0.7, 0.8): a squad.
    let (p, _, _) = missions::strike_numbers(&cfg, 0.475, 0.9, None);
    assert!(near(p, 0.195), "E: {p}");
    assert!(p < cfg.squad_below);
    // The quote's row: the median gun (0.625) vs B's target, standing 0.2.
    let (p, _, _) = missions::strike_numbers(&cfg, missions::MEDIAN_STRENGTH, 0.5, None);
    assert!(near(p, 0.679) && near(1.0 - p, 0.321), "quote: {p}");
    let quote = (400.0 * (1.0 + 1.5 * (1.0 - p)) * (1.0 + 1.0 * 0.2)).round();
    assert_eq!(quote, 711.0);
}

#[test]
fn test_strike_in_rival_district_pushes_trespass_and_raided_deed() {
    let mut w = world();
    let g = w.gang_list()[0];
    let [buyer, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
    set_fighter(&mut w, taker, 1.0, 0.9);
    lawless(&mut w, taker);
    set_fighter(&mut w, t, 0.4, 0.4);
    lone(&mut w, t);
    set_coins(&mut w, buyer, 5000);
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Hit, t, None)).expect("posted");
    assert!(contracts::accept(&mut w, id, taker, &[]));
    let door = tile_of(&w, t);
    hold_district(&mut w, door, g, 0.6);
    let bf = missions::buyer_faction(&w, &w.contracts[&id]);
    assert_ne!(bf, g);
    w.regard.insert((g, bf), Regard { value: 0.2, fear: 0.5 });
    if let Some(x) = w.comp_mut::<Gang>(g) {
        x.shocks.clear();
    }
    let terms = missions::strike(&w, id, &[taker], door).expect("terms");
    assert!(terms.political > 0.0, "a foreign controller costs: {terms:?}");
    assert_eq!(terms.controller, 2, "a gang's district");
    assert_eq!(missions::decide(&mut w, id, &[taker], door), Decision::Strike);
    assert_eq!(w.contracts[&id].strike.map(|s| s.decision), Some(Decision::Strike));
    let shocked = w.comp::<Gang>(g).is_some_and(|x| x.shocks.contains(&Shock::Trespass { by: Some(bf) }));
    assert!(shocked, "Trespass on the controller");
    let d = w.district_of(door).index();
    let deed =
        w.rumours[d].entries.iter().any(|e| e.deed == Deed::Raided && e.actor == Some(bf) && e.object == Some(g));
    assert!(deed, "a Raided deed (actor the buyer's faction, object the controller) in the district's pool");
    // In a City district: no political term, no Trespass.
    let mut w = world();
    let [buyer, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
    set_fighter(&mut w, taker, 1.0, 0.9);
    set_fighter(&mut w, t, 0.4, 0.4);
    lone(&mut w, t);
    set_coins(&mut w, buyer, 5000);
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Hit, t, None)).expect("posted");
    assert!(contracts::accept(&mut w, id, taker, &[]));
    let door = tile_of(&w, t);
    let d = w.district_of(door).index();
    w.districts[d].control = Controller::City;
    let terms = missions::strike(&w, id, &[taker], door).expect("terms");
    assert_eq!(terms.political, 0.0);
    assert_eq!(terms.decision, Decision::Strike);
}

/// A taken Hit on `t` whose strike in `g`'s district must fail on its
/// political cost after its holds (share 1, fear 1, value 1: political 2).
fn doomed_hit(w: &mut World, g: EntityId, t: EntityId, brokered: bool) -> (citysim::contract::ContractId, TilePos) {
    let pick: Vec<EntityId> = strangers(w, 3).into_iter().filter(|&x| x != t).take(2).collect();
    let [buyer, taker] = pick[..] else { unreachable!() };
    set_coins(w, buyer, 5000);
    let broker = brokered.then(|| contracts::open_fixers(w)[0]);
    let id = contracts::post(w, direct(buyer, ContractKind::Hit, t, broker)).expect("posted");
    assert!(contracts::accept(w, id, taker, &[]));
    let door = tile_of(w, t);
    hold_district(w, door, g, 1.0);
    let bf = missions::buyer_faction(w, &w.contracts[&id]);
    w.regard.insert((g, bf), Regard { value: 1.0, fear: 1.0 });
    let hold = w.config.missions.hold_days;
    if let Some(c) = w.contracts.get_mut(&id) {
        c.holds = hold;
    }
    (id, door)
}

#[test]
fn test_gang_sells_out_non_member_refuses_member() {
    // A non-member target the gang dislikes: sold out at 1.5x, the brokered
    // buyer topping up the escrow.
    let mut w = world();
    let g = w.gang_list()[0];
    ready_gang(&mut w, g, 4);
    let [t] = strangers(&w, 1)[..] else { unreachable!() };
    let tf = w.gang_of(t).or_else(|| w.corp_of_agent(t)).unwrap_or(t);
    w.regard.insert((g, tf), Regard { value: -0.5, fear: 0.5 });
    let (id, door) = doomed_hit(&mut w, g, t, true);
    let c0 = w.contracts[&id].clone();
    let buyer = c0.buyer.expect("a buyer");
    let before = w.purse(Some(buyer));
    assert!(contracts::can_sell_out(&w, id, g));
    let taker = c0.taker.expect("a taker");
    assert_eq!(missions::decide(&mut w, id, &[taker], door), Decision::SellOut);
    let c = &w.contracts[&id];
    let price = (c0.price as f32 * 1.5).round() as i64;
    assert_eq!(c.taker, Some(g), "the gang took it over");
    assert!(c.sold_out);
    assert_eq!(c.price, price);
    assert_eq!(c.escrow, price, "the escrow topped up");
    assert_eq!(w.purse(Some(buyer)), before - (price - c0.price), "the buyer paid the premium");
    assert_eq!(c.status, ContractStatus::Taken);
    assert_eq!(c.render, Render::Live);
    assert!(!c.crew.is_empty(), "the gang's crew");
    assert_eq!(contracts::job_of(&w, g), Some(id));
    assert!(w.missions.contains_key(&id), "the job marches as a mission");
    assert_eq!(w.comp::<Gang>(g).map(|x| x.order), Some(citysim::Order::Job));
    assert_eq!(w.stats.current.contract.sold_out, 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::SoldOut));
    let held: i64 = w.contracts.values().map(|c| c.escrow).sum();
    assert_eq!(held, w.escrow_held, "escrow conserved");

    // A member of the gang: never sold out, the strike fails.
    let mut w = world();
    let g = w.gang_list()[0];
    let members = ready_gang(&mut w, g, 4);
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader);
    let t = members.iter().copied().find(|&m| Some(m) != leader).expect("a member");
    let (id, door) = doomed_hit(&mut w, g, t, false);
    assert!(!contracts::can_sell_out(&w, id, g), "no sell-out of a member");
    let taker = w.contracts[&id].taker.expect("a taker");
    assert_eq!(missions::decide(&mut w, id, &[taker], door), Decision::Fail);
    let c = &w.contracts[&id];
    assert_eq!(c.status, ContractStatus::Open, "the attempt failed");
    assert_eq!(c.attempts, 1);
    assert!(!c.sold_out);
    assert!(w.events.iter().any(|e| e.kind == EventKind::StrikeDeclined));
    assert_eq!(w.stats.current.contract.strikes_declined_pol, 1, "the political term decided it");

    // A brokered buyer short of the premium: the offer lapses.
    let mut w = world();
    let g = w.gang_list()[0];
    ready_gang(&mut w, g, 4);
    let [t] = strangers(&w, 1)[..] else { unreachable!() };
    let tf = w.gang_of(t).or_else(|| w.corp_of_agent(t)).unwrap_or(t);
    w.regard.insert((g, tf), Regard { value: -0.5, fear: 0.5 });
    let (id, door) = doomed_hit(&mut w, g, t, true);
    let buyer = w.contracts[&id].buyer.expect("a buyer");
    set_coins(&mut w, buyer, 0);
    let escrow = w.contracts[&id].escrow;
    let taker = w.contracts[&id].taker.expect("a taker");
    assert_eq!(missions::decide(&mut w, id, &[taker], door), Decision::Fail, "the offer lapsed");
    let c = &w.contracts[&id];
    assert_eq!(c.status, ContractStatus::Open);
    assert!(!c.sold_out);
    assert_eq!(c.escrow, escrow, "nothing topped up");
    assert_eq!(contracts::job_of(&w, g), None);
}

#[test]
fn test_squad_when_solo_estimate_below_squad_below() {
    let mut w = world();
    let g = w.gang_list()[0];
    let members = ready_gang(&mut w, g, 4);
    let [buyer, t] = strangers(&w, 2)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    set_fighter(&mut w, t, 1.0, 1.0); // strength 1.25
    lone(&mut w, t);
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader);
    let others: Vec<EntityId> = members.iter().copied().filter(|&m| Some(m) != leader).collect();
    let taker = others[0];
    set_fighter(&mut w, taker, 0.3, 0.3); // 0.375: solo p_win ~0.11
    for &m in &others[1..] {
        set_fighter(&mut w, m, 0.5, 0.5); // 0.625 each
    }
    if let Some(l) = leader {
        set_fighter(&mut w, l, 0.1, 0.1);
    }
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Hit, t, None)).expect("posted");
    let c = w.contracts[&id].clone();
    let below = w.config.missions.squad_below;
    assert!(missions::estimate(&w, &[taker], &c.target) < below, "a weak gun");
    let crew = contracts::squad_crew(&w, &c, taker);
    // 0.375 + 0.625 vs 1.25: 0.35; + 0.625: 0.71 ≥ 0.6, so two and no more.
    assert_eq!(crew.len(), 2, "the crew stops once the estimate clears: {crew:?}");
    let mut team = vec![taker];
    team.extend(crew.iter().copied());
    assert!(missions::estimate(&w, &team, &c.target) >= below);
    assert!(missions::estimate(&w, &team[..2], &c.target) < below);
    assert!(contracts::accept(&mut w, id, taker, &crew));
    let c = &w.contracts[&id];
    assert_eq!(c.render, Render::Live, "a squad is always live");
    let m = w.missions.get(&id).expect("a mission");
    assert_eq!(m.crew[0], taker, "the taker first");
    assert_eq!(m.crew.len(), 3);
    for a in &m.crew {
        assert_eq!(w.mission_of.get(a), Some(&id));
        assert_ne!(w.comp::<Brain>(*a).map(|b| b.lod), Some(Lod::Statistical), "a body to march");
    }
    assert_eq!(raid::expedition_of(&w, taker), Some(raid::Expedition::Mission(id)));
    assert_eq!(raid::departure(&w, taker), Some(m.raid_at));
    assert!(!raid::raid_pending(&w, taker), "never the Raid goal");
    assert!(!raid::raid_done(&w, taker));
    assert_eq!(raid::target_tile(&w, taker), Some(m.door));
    assert_eq!(raid::muster_point(&w, taker), Some(raid::MusterAt::Door(m.muster)));
    // M16a 5.1 (a): fulfilled, it counts as a squad Hit (0 of seeds 42-47 in 120 days).
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    assert_eq!((w.stats.current.contract.hits_done, w.stats.current.contract.hits_squad), (1, 1), "a squad Hit");
    // A strong gun goes alone.
    let mut w = world();
    let [buyer, t, gun] = strangers(&w, 3)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    set_fighter(&mut w, t, 0.4, 0.4);
    lone(&mut w, t);
    set_fighter(&mut w, gun, 1.0, 0.9);
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Hit, t, None)).expect("posted");
    assert!(contracts::squad_crew(&w, &w.contracts[&id], gun).is_empty(), "no squad for a strong gun");
}

#[test]
fn test_ledger_hold_then_sell_out() {
    let mut w = world();
    let g = w.gang_list()[0];
    ready_gang(&mut w, g, 4);
    let [buyer, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    for a in [t, taker] {
        lod::set_lod(&mut w, a, Lod::Statistical);
    }
    let tf = w.gang_of(t).or_else(|| w.corp_of_agent(t)).unwrap_or(t);
    w.regard.insert((g, tf), Regard { value: -0.5, fear: 0.5 });
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Hit, t, None)).expect("posted");
    assert!(contracts::accept(&mut w, id, taker, &[]));
    assert_eq!(w.contracts[&id].render, Render::Ledger);
    // The ledger's door: the target's habit now.
    let habit = hunt::habit(&w, t, w.tick);
    let door = habit.building.and_then(|b| missions::door_of(&w, b)).unwrap_or(habit.tile);
    hold_district(&mut w, door, g, 1.0);
    let bf = missions::buyer_faction(&w, &w.contracts[&id]);
    w.regard.insert((g, bf), Regard { value: 1.0, fear: 1.0 });
    let hold = w.config.missions.hold_days;
    for n in 1..=hold {
        let now = w.tick;
        contracts::resolve_ledger(&mut w, id);
        let c = &w.contracts[&id];
        assert_eq!(c.status, ContractStatus::Taken, "held, not failed");
        assert_eq!(c.holds, n);
        assert_eq!(c.due, Some(now + TICKS_PER_DAY), "a Hold looks again in a day");
        assert!(w.ledger_due.contains(&(now + TICKS_PER_DAY, id)));
        assert_eq!(c.strike.map(|s| s.decision), Some(Decision::Hold));
        assert!(law::living(&w, t), "no draw on a Hold");
    }
    contracts::resolve_ledger(&mut w, id);
    let c = &w.contracts[&id];
    assert_eq!(c.taker, Some(g), "sold out to the district's gang");
    assert!(c.sold_out);
    assert_eq!(c.render, Render::Live);
    assert!(!w.ledger_due.iter().any(|&(_, x)| x == id), "off the ledger");
    assert!(law::living(&w, t));
}

#[test]
fn test_no_mission_no_draw() {
    // No off switch exists since the 2026-10-09 streamlining, so the twin
    // is a day with no posting source (`max_open` 0: `post` refuses every
    // record, so no record, run or mission can exist) against the shipped
    // day on the same seed, which posts none on its first day either; the
    // mission hooks are also called on the no-source world every tick. The
    // world stream's next value and the day's row are the same.
    let mut a = world();
    let mut cfg = Config::load();
    cfg.contracts.max_open = 0;
    let mut b = World::new(42, cfg);
    let agents: Vec<EntityId> = a.citizens().into_iter().take(64).collect();
    for _ in 0..TICKS_PER_DAY {
        a.tick();
        b.tick();
        missions::check(&mut b);
        for &x in &agents {
            assert_eq!(raid::expedition_of(&b, x), raid::expedition_of(&a, x));
            assert!(contracts::strike_at_contact(&mut b, x));
        }
    }
    assert!(a.contracts.is_empty() && a.missions.is_empty(), "the shipped day posted nothing");
    assert!(b.contracts.is_empty() && b.missions.is_empty());
    assert_eq!(a.rng.world().random::<u64>(), b.rng.world().random::<u64>(), "the same world stream");
    let row = |w: &World| {
        w.stats.history.back().map(|r| {
            let mut r = r.clone();
            r.ticks_per_sec = 0.0;
            r.csv_row()
        })
    };
    assert_eq!(row(&a), row(&b), "the same day's row (outside ticks_per_sec)");
}

/// A Beat mission at a street door: three raiders (the taker and two crew,
/// goal Contract) against the target and two of its gang mates, everyone
/// standing on `door`, the strike decided.
fn staged_beat(w: &mut World) -> (citysim::contract::ContractId, EntityId, Vec<EntityId>, Vec<EntityId>, TilePos) {
    let gangs = w.gang_list().to_vec();
    let free = strangers(w, 7);
    let (buyer, t) = (free[0], free[1]);
    let raiders = vec![free[2], free[3], free[4]];
    let mates = vec![free[5], free[6]];
    gang::enlist(w, t, gangs[1]);
    for &m in &mates {
        gang::enlist(w, m, gangs[1]);
        set_fighter(w, m, 0.5, 0.8);
    }
    set_fighter(w, t, 0.5, 0.8);
    for &r in &raiders {
        lawless(w, r);
        set_fighter(w, r, 0.6, 0.6);
    }
    set_coins(w, buyer, 5000);
    let id = contracts::post(w, direct(buyer, ContractKind::Beat, t, None)).expect("posted");
    assert!(contracts::accept(w, id, raiders[0], &raiders[1..]));
    let door = w.comp::<Position>(t).map(|p| p.tile).unwrap_or_default();
    for &a in raiders.iter().chain(mates.iter()).chain(std::iter::once(&t)) {
        if let Some(p) = w.comp_mut::<Position>(a) {
            p.tile = door;
            p.building = None;
        }
    }
    for &r in &raiders {
        if let Some(b) = w.comp_mut::<Brain>(r) {
            b.current_goal = Some(GoalKind::Contract);
        }
    }
    let now = w.tick;
    if let Some(m) = w.missions.get_mut(&id) {
        m.door = door;
        m.door_building = None;
        m.raid_at = now.saturating_sub(1);
    }
    let terms = citysim::contract::StrikeTerms {
        p_win: 0.5,
        controller: 0,
        political: 0.0,
        ev: 0.0,
        decision: Decision::Strike,
    };
    if let Some(c) = w.contracts.get_mut(&id) {
        c.strike = Some(terms);
    }
    (id, t, raiders, mates, door)
}

#[test]
fn test_mission_fight_out_matches_gang_raid() {
    // The same raider and defender lists on the same seed: the gang arm's
    // call (`raid::fight_out` as a gang raid makes it) and the mission arm
    // (`missions::brawl` through `raid::resolve`) roll the same pairings.
    let mut a = world();
    let mut b = world();
    let (ida, ta, ra, _, door) = staged_beat(&mut a);
    let (idb, tb, rb, _, _) = staged_beat(&mut b);
    assert_eq!((ida, ta, &ra), (idb, tb, &rb), "the same stage");
    let cursor = |w: &World| w.events.back().map_or(0, |e| e.id);
    let (ca, cb) = (cursor(&a), cursor(&b));
    // The gang arm.
    let mut raiders = raid::gathered_for(&a, &ra, ra[0], door, GoalKind::Contract);
    let mut defenders = missions::defenders(&a, ida);
    assert!(defenders.contains(&ta), "the target at its door");
    assert_eq!(raiders.len(), 3);
    assert_eq!(defenders.len(), 3);
    let place = format!("a street in {}", a.district_name(a.district_of(door)));
    let tally = raid::fight_out(&mut a, &mut raiders, &mut defenders, door, &place, 1.0, None);
    // The mission arm.
    assert!(raid::resolve(&mut b, rb[0]).is_some(), "the mission's brawl ran");
    let fights = |w: &World, from: u64| -> Vec<(EventKind, String)> {
        w.events
            .iter()
            .filter(|e| e.id > from)
            .filter(|e| matches!(e.kind, EventKind::Murder | EventKind::Assault | EventKind::Crossfire))
            .map(|e| (e.kind, e.text.clone()))
            .collect()
    };
    let (fa, fb) = (fights(&a, ca), fights(&b, cb));
    assert!(!fa.is_empty(), "pairings were fought");
    assert_eq!(fa, fb, "identical pairings, deaths and texts");
    let dead = |w: &World, xs: &[EntityId]| xs.iter().filter(|&&x| !law::living(w, x)).count();
    let mates = |w: &World| w.comp::<Gang>(w.gang_list()[1]).map(|g| g.members.clone()).unwrap_or_default();
    assert_eq!(dead(&a, &ra), dead(&b, &rb), "the same raiders fell");
    assert_eq!(dead(&a, &[ta]), dead(&b, &[tb]), "the same target");
    assert_eq!(dead(&a, &mates(&a)), dead(&b, &mates(&b)), "the same defenders fell");
    assert!(tally.deaths <= fa.len(), "deaths are pairings'");
    // The Beat settled on the result; the mission is over.
    assert!(!b.missions.contains_key(&idb));
    let won = defenders.is_empty();
    let st = b.contracts[&idb].status;
    if won {
        assert_eq!(st, ContractStatus::Fulfilled, "a Beat won");
    } else {
        assert_eq!(st, ContractStatus::Open, "a lost brawl fails the attempt");
    }
    assert_eq!(a.rng.world().random::<u64>(), b.rng.world().random::<u64>(), "the same world stream after");
}

#[test]
fn test_squad_mission_marches_and_brawls() {
    // The mechanism end to end in a seeded world (missions are rare on
    // 42-47: no record reaches a squad there yet): a weak gun's squad,
    // its muster at 22:00, the strike, the march and the brawl at the
    // target's door; the record settles or fails an attempt, and the
    // mission is over by the end of the march window.
    let mut w = world();
    let g = w.gang_list()[0];
    let members = ready_gang(&mut w, g, 4);
    let [buyer, t] = strangers(&w, 2)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    set_fighter(&mut w, t, 0.9, 0.5);
    lone(&mut w, t);
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader);
    let others: Vec<EntityId> = members.iter().copied().filter(|&m| Some(m) != leader).collect();
    let taker = others[0];
    set_fighter(&mut w, taker, 0.3, 0.9);
    for &m in &others[1..] {
        set_fighter(&mut w, m, 0.8, 0.9);
    }
    // A gang's street crew: no shift to keep.
    for &m in &others {
        w.vacate_job(m);
    }
    // A City district at the door: no political term.
    for d in &mut w.districts {
        d.control = Controller::City;
    }
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Beat, t, None)).expect("posted");
    let c = w.contracts[&id].clone();
    let crew = contracts::squad_crew(&w, &c, taker);
    assert!(!crew.is_empty(), "a squad");
    assert!(contracts::accept(&mut w, id, taker, &crew));
    let raid_at = w.missions[&id].raid_at;
    let mut struck = false;
    let mut brawled = false;
    let until = raid_at + 7 * TICKS_PER_HOUR;
    while w.tick < until && w.contracts.get(&id).is_some_and(|c| c.status == ContractStatus::Taken) {
        w.tick();
        struck |= w.contracts.get(&id).and_then(|c| c.strike).is_some();
        brawled |= w.contract_log.iter().any(|(_, x, s)| *x == id && s.contains("crew hit"));
    }
    let c = &w.contracts[&id];
    assert!(struck, "the strike was decided at the muster");
    assert!(!w.missions.contains_key(&id), "the mission is over");
    assert!(
        brawled || c.status != ContractStatus::Taken,
        "the crew reached the door (or the record closed): {:?} {:?}",
        c.status,
        w.contract_log.iter().filter(|(_, x, _)| *x == id).collect::<Vec<_>>()
    );
    let held: i64 = w.contracts.values().map(|c| c.escrow).sum();
    assert_eq!(held, w.escrow_held);
}

/// A sell-out-ready brokered Hit in `g`'s district on a stranger the gang
/// dislikes (the strike must fail on its political cost after its holds).
fn sellable(w: &mut World, g: EntityId) -> (citysim::contract::ContractId, TilePos) {
    ready_gang(w, g, 4);
    let [t] = strangers(w, 1)[..] else { unreachable!() };
    let tf = w.gang_of(t).or_else(|| w.corp_of_agent(t)).unwrap_or(t);
    w.regard.insert((g, tf), Regard { value: -0.5, fear: 0.5 });
    doomed_hit(w, g, t, true)
}

#[test]
fn test_short_top_up_returns_only_its_take() {
    // Review fix: a brokered buyer one coin short of the premium. The
    // top-up it could pay goes back to it, the record keeps its own escrow
    // (so a later taker is paid), and the offer lapses.
    let mut w = world();
    let g = w.gang_list()[0];
    let (id, door) = sellable(&mut w, g);
    let c0 = w.contracts[&id].clone();
    let buyer = c0.buyer.expect("a buyer");
    let top = (c0.price as f32 * w.config.missions.sell_premium).round() as i64 - c0.price;
    set_coins(&mut w, buyer, top - 1);
    let before = identity(&w);
    let taker = c0.taker.expect("a taker");
    assert_eq!(missions::decide(&mut w, id, &[taker], door), Decision::Fail, "the offer lapsed");
    let c = w.contracts[&id].clone();
    assert_eq!(c.status, ContractStatus::Open, "the attempt failed");
    assert!(!c.sold_out);
    assert_eq!(c.escrow, c0.escrow, "the record's own escrow stays");
    assert_eq!(w.purse(Some(buyer)), top - 1, "the partial top-up went back");
    let held: i64 = w.contracts.values().map(|c| c.escrow).sum();
    assert_eq!(held, w.escrow_held);
    assert_eq!(identity(&w), before, "no coin made or lost");
    // A later taker is paid from it.
    let [next] = strangers(&w, 6)[5..] else { unreachable!() };
    assert!(contracts::accept(&mut w, id, next, &[]));
    let paid0 = w.purse(Some(next));
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert!(w.purse(Some(next)) > paid0, "the next taker is paid");
    assert_eq!(w.contracts[&id].escrow, 0);
    assert_eq!(identity(&w), before);
}

#[test]
fn test_mission_deaths_mid_march() {
    // A crew member, then the taker, then the target die mid-march: the
    // crew shrinks (the taker's place to the next of the crew), then the
    // target dead by other hands cancels the record (status table) and the
    // mission is gone with every index.
    let mut w = world();
    let (id, t, raiders, _, _) = staged_beat(&mut w);
    let mates: Vec<EntityId> = w.missions[&id].crew.to_vec();
    assert_eq!(mates, raiders);
    w.kill_by(raiders[1], DeathCause::Violence, None);
    let c = &w.contracts[&id];
    assert_eq!(c.status, ContractStatus::Taken);
    assert!(!c.crew.contains(&raiders[1]));
    assert!(!w.missions[&id].crew.contains(&raiders[1]));
    assert!(!w.mission_of.contains_key(&raiders[1]));
    w.kill_by(raiders[0], DeathCause::Violence, None);
    let c = &w.contracts[&id];
    assert_eq!(c.status, ContractStatus::Taken, "the crew marches on");
    assert_eq!(c.taker, Some(raiders[2]), "the taker's place to the next of the crew");
    assert_eq!(w.missions[&id].crew.to_vec(), vec![raiders[2]]);
    assert!(!w.mission_of.contains_key(&raiders[0]));
    w.kill_by(t, DeathCause::Violence, None);
    let c = &w.contracts[&id];
    assert_eq!(c.status, ContractStatus::Cancelled, "the target dead by other hands");
    assert_eq!(c.escrow, 0);
    assert!(w.missions.is_empty() && w.mission_of.is_empty(), "the mission and its index are clean");
    // The last of the crew out fails the attempt.
    let mut w = world();
    let (id, _, raiders, _, _) = staged_beat(&mut w);
    for &r in &raiders {
        w.kill_by(r, DeathCause::Violence, None);
    }
    let c = &w.contracts[&id];
    assert_eq!(c.status, ContractStatus::Open, "back on the board");
    assert_eq!(c.attempts, 1);
    assert!(c.taker.is_none() && c.crew.is_empty());
    assert!(w.missions.is_empty() && w.mission_of.is_empty());
}

#[test]
fn test_identity_across_sell_out_then_fail_or_settle() {
    // The coin identity (`econ::identity`) is constant across a brokered
    // sell-out (the top-up into escrow), a failed attempt after it (the
    // escrow stays), the refund, and a sell-out settled (the gang and its
    // crew paid, the Fixer's cut).
    let mut w = world();
    let g = w.gang_list()[0];
    let (id, door) = sellable(&mut w, g);
    let before = identity(&w);
    let taker = w.contracts[&id].taker.expect("a taker");
    assert_eq!(missions::decide(&mut w, id, &[taker], door), Decision::SellOut);
    assert_eq!(identity(&w), before, "the sell-out");
    let escrow = w.contracts[&id].escrow;
    contracts::fail_attempt(&mut w, id, "a test");
    assert_eq!(w.contracts[&id].status, ContractStatus::Open);
    assert_eq!(w.contracts[&id].escrow, escrow, "the escrow stays on the record");
    assert!(w.missions.is_empty() && contracts::job_of(&w, g).is_none());
    assert_eq!(identity(&w), before, "a failed attempt");
    contracts::settle(&mut w, id, contracts::Settle::Cancelled, "a test");
    assert_eq!(w.contracts[&id].escrow, 0);
    assert_eq!(identity(&w), before, "the refund");
    let mut w = world();
    let g = w.gang_list()[0];
    let (id, door) = sellable(&mut w, g);
    let before = identity(&w);
    let treasury = w.comp::<Gang>(g).map_or(0, |x| x.treasury);
    let taker = w.contracts[&id].taker.expect("a taker");
    assert_eq!(missions::decide(&mut w, id, &[taker], door), Decision::SellOut);
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    assert_eq!(w.contracts[&id].escrow, 0);
    assert!(w.comp::<Gang>(g).map_or(0, |x| x.treasury) > treasury, "the gang's share");
    assert_eq!(identity(&w), before, "a sell-out settled");
    let held: i64 = w.contracts.values().map(|c| c.escrow).sum();
    assert_eq!(held, w.escrow_held);
}

#[test]
fn test_gang_leader_never_takes_a_job_on_kin() {
    // Review fix: the leader who accepts is held to the relationship
    // exclusions as the crew are.
    let mut w = world();
    let g = w.gang_list()[0];
    ready_gang(&mut w, g, 4);
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("a leader");
    let [buyer, t] = strangers(&w, 2)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Beat, t, None)).expect("posted");
    let c = w.contracts[&id].clone();
    assert!(contracts::gang_eligible(&w, &c, g), "a stranger's job");
    w.edge_entry(leader, t).kind = RelKind::Friend;
    assert!(!contracts::gang_eligible(&w, &c, g), "not on the leader's Friend");
}

/// M16a review: a gang job queued without a mission (no crew to march) is
/// not offered as `Order::Job`: the gang never takes the order, so
/// `gang::run` never rethinks it every tick, and a small shock waits for
/// its rescore instead of being cleared.
#[test]
fn test_queued_gang_job_never_takes_order_job_and_keeps_shocks() {
    let mut w = world();
    let g = w.gang_list()[0];
    ready_gang(&mut w, g, 4);
    // Job wins whenever it is offered; no dwell holds the current order.
    w.config.gangs.order_flat.job = 10.0;
    w.config.life.order_dwell_ticks = 0;
    let [buyer, t] = strangers(&w, 2)[..] else { unreachable!() };
    set_coins(&mut w, buyer, 5000);
    let id = contracts::post(&mut w, direct(buyer, ContractKind::Hit, t, None)).expect("posted");
    assert!(contracts::accept(&mut w, id, g, &[]));
    assert_eq!(contracts::job_of(&w, g), Some(id), "the gang holds the job");
    assert!(!contracts::job_live(&w, g), "queued: no mission");
    let i = citysim::systems::faction::gather_inputs(&w, g).expect("inputs");
    assert!(i.job.is_none(), "a queued job is not offered");
    w.tick += 1;
    citysim::systems::faction::rescore(&mut w, g, 0.0);
    assert_ne!(w.comp::<Gang>(g).map(|x| x.order), Some(citysim::Order::Job));
    if let Some(x) = w.comp_mut::<Gang>(g) {
        x.shocks.clear();
        x.shocks.push(Shock::MemberArrested);
    }
    for _ in 0..30 {
        w.tick += 1;
        gang::run(&mut w);
    }
    let x = w.comp::<Gang>(g).expect("the gang");
    assert_ne!(x.order, citysim::Order::Job);
    assert!(x.shocks.contains(&Shock::MemberArrested), "the shock waits for the rescore");
}
