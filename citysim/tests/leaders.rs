//! The pre-view pass phase 6 (docs/PREVIEW_PASS.md § 0 P9, P13-P15; OPEN-118,
//! OPEN-128; `systems/leaders.rs`): the leader's visible half. Each test is
//! a seeded world (the shipped seed-42 city) and one mechanism, with
//! `[leaders]` on and its control off (the base) where the base differs: a
//! body leader's protection is collected on his round, in person, only from
//! an owner there (the base paid it at midnight); an order chosen waits for
//! a lieutenant at court, is walked to one past the wait, and goes by word
//! past the cap (a raid slips, it never vanishes); a Statistical leader's
//! order is in force at once (the mirror); a member's take is handed to the
//! leader at court.

mod common;

use citysim::components::{GangMember, Lod, Order, PendingOrder, Position};
use citysim::contract::{CoercionStatus, CoercionWhy, Demand, Leverage};
use citysim::systems::econ::identity;
use citysim::systems::{demography, law, leaders, leverage, lod, ownership};
use citysim::{ActionKind, Brain, BuildingKind, Config, EntityId, EventKind, ExecState, Gang, LocationKey, World};
use citysim::{TICKS_PER_DAY, TICKS_PER_HOUR};
use common::set_coins;

/// The shipped seed-42 city on the first day a gang has a free leader and a
/// second free member, at 10:00 (nobody's court due), run once per switch
/// and shared as a clone.
fn city(on: bool) -> World {
    static ON: std::sync::OnceLock<World> = std::sync::OnceLock::new();
    static OFF: std::sync::OnceLock<World> = std::sync::OnceLock::new();
    let cell = if on { &ON } else { &OFF };
    cell.get_or_init(|| {
        let mut cfg = Config::load();
        if !on {
            cfg.leaders = None;
        }
        let mut w = World::new(42, cfg);
        w.run_ticks(10 * TICKS_PER_HOUR);
        for _ in 0..30 {
            if led(&w).is_some() {
                return w;
            }
            w.run_ticks(TICKS_PER_DAY);
        }
        panic!("no gang with a free leader and a second free member in 30 days");
    })
    .clone()
}

/// The first gang with a free leader, a second free member and a Hideout.
fn led(w: &World) -> Option<(EntityId, EntityId, EntityId)> {
    w.gangs().into_iter().find_map(|g| {
        let x = w.comp::<Gang>(g)?;
        let l = x.leader.filter(|&l| free(w, l))?;
        let lt = x.members.iter().copied().find(|&m| m != l && free(w, m))?;
        w.hideout_of(g).map(|_| (g, l, lt))
    })
}

fn jobs2(w: &World) -> citysim::stats::Jobs2Cols {
    w.stats.current.jobs2.clone().unwrap_or_default()
}

fn free(w: &World, a: EntityId) -> bool {
    law::living(w, a) && !w.has::<citysim::Sentence>(a) && demography::is_adult(w, a) && w.has::<Brain>(a)
}

/// A gang with a free leader and one other free member made its only
/// lieutenant (the rest rank 0), both bodies (Full).
fn led_gang(w: &mut World) -> (EntityId, EntityId, EntityId) {
    let (g, l, lt) = led(w).expect("a led gang (the city's premise)");
    let members = w.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
    for m in members {
        if m != l {
            w.comp_mut::<GangMember>(m).expect("member").rank = u8::from(m == lt);
        }
    }
    lod::set_lod(w, l, Lod::Full);
    lod::set_lod(w, lt, Lod::Full);
    (g, l, lt)
}

/// Stand `who` out on the street at `tile` (no building), a body.
fn stand(w: &mut World, who: EntityId, tile: citysim::TilePos) {
    w.remove_from_building(who);
    let p = w.comp_mut::<Position>(who).expect("position");
    p.tile = tile;
    p.building = None;
}

/// A protected business in a district `g` holds (one of its Homes there
/// made its territory if none), deeded to a free adult outside any gang, a
/// body with coins.
fn business(w: &mut World, g: EntityId) -> (EntityId, EntityId) {
    let b = leverage::PROTECTED
        .iter()
        .flat_map(|&k| w.buildings_of_kind(k).to_vec())
        .find(|&b| {
            w.comp::<citysim::Building>(b).is_some_and(|bd| !bd.derelict && !bd.demolished && bd.venue.is_none())
        })
        .expect("a protected business");
    let d = w.district_of_building(b);
    let held = citysim::systems::gang::held_districts(w, g).iter().any(|&(x, _)| x == d);
    if !held {
        let home = w
            .buildings_of_kind(BuildingKind::Home)
            .iter()
            .copied()
            .find(|&h| w.district_of_building(h) == d && citysim::systems::gang::holder_of(w, h).is_none())
            .expect("a Home in the business's district");
        let t = &mut w.comp_mut::<Gang>(g).expect("gang").territory;
        let at = t.binary_search(&home).unwrap_or_else(|i| i);
        t.insert(at, home);
    }
    let owner = common::free_adults(w)
        .into_iter()
        .find(|&a| !w.has::<GangMember>(a) && !law::is_guard(w, a))
        .expect("a free adult outside the gangs");
    ownership::transfer_building(w, b, Some(owner));
    set_coins(w, owner, 10_000);
    lod::set_lod(w, owner, Lod::Full);
    (b, owner)
}

/// P13: a body leader's protection is not paid at midnight (the base paid
/// it there); his round's stop collects it only from the owner inside the
/// business, and the ledger holds.
#[test]
fn test_round_collects_protection_in_person_only_from_the_owner_there() {
    let midnight_paid = |on: bool| -> (World, EntityId, EntityId, EntityId, EntityId, u64) {
        let mut w = city(on);
        let (g, l, _) = led_gang(&mut w);
        let (b, owner) = business(&mut w, g);
        let weekly = w.config.leverage.protection_weekly;
        let id = leverage::force(
            &mut w,
            Some(g),
            l,
            owner,
            Leverage::Threat,
            Demand::Coins { amount: weekly, weekly: true },
            CoercionWhy::Protection,
            Some(owner),
            Some(b),
        );
        assert_eq!(w.coercions[&id].paid, 1, "the forced first week at once");
        w.coercions.get_mut(&id).expect("coercion").next_due = w.tick;
        // Straight to the midnight tick (no round in between).
        w.tick = (w.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
        w.run_ticks(1);
        (w, g, l, b, owner, id)
    };
    let (w, _, _, _, _, id) = midnight_paid(false);
    assert_eq!(w.coercions[&id].paid, 2, "base: the week paid at midnight");
    let (mut w, g, l, b, owner, id) = midnight_paid(true);
    assert!(leaders::collects(&w, g), "a body leader collects in person");
    assert_eq!(w.coercions[&id].paid, 1, "on: midnight pays no body leader's protection");
    assert_eq!(w.coercions[&id].status, CoercionStatus::Active, "the week stays owed");
    // The LOD pass ran at midnight: the owner a body again.
    lod::set_lod(&mut w, owner, Lod::Full);
    let before = identity(&w);
    // The owner out: nothing collected, the week owed.
    let door = w.comp::<citysim::Building>(b).expect("business").door;
    stand(&mut w, owner, citysim::TilePos { x: door.x, y: door.y.saturating_add(8) });
    w.enter_building(l, b);
    leaders::round_stop(&mut w, l, g, b);
    assert_eq!(w.coercions[&id].paid, 1, "the owner out: nothing collected");
    assert_eq!(jobs2(&w).lead_owner_out, 1);
    // The owner in and awake: the week collected in person.
    w.enter_building(owner, b);
    w.comp_mut::<Brain>(owner).expect("brain").exec = ExecState::Idle;
    leaders::round_stop(&mut w, l, g, b);
    assert_eq!(w.coercions[&id].paid, 2, "the owner there: the week collected");
    assert_eq!(jobs2(&w).lead_collected, 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Protected && e.actors.as_slice() == [l, owner, b]));
    assert_eq!(identity(&w), before, "no coin made or lost");
    // Collected for the week: no longer owed, so not a round stop of debt.
    assert!(w.coercions[&id].next_due > w.tick);
}

fn raid_order(decided: u64) -> PendingOrder {
    PendingOrder { order: Order::Raid, decided, corp_target: None, retaliate_on: None, why: "test".into() }
}

/// P15: an order held for the giving leaves the members on the old one; at
/// court, with the lieutenant there, it is given in person and in force
/// (the raid's muster timed from then). Off, nothing is held (the base).
#[test]
fn test_order_waits_for_a_lieutenant_at_court() {
    let mut off = city(false);
    let (g0, _, _) = led_gang(&mut off);
    assert!(!leaders::in_person(&off, g0), "off: an order is in force the moment it is chosen");
    let mut w = city(true);
    let (g, l, lt) = led_gang(&mut w);
    assert!(leaders::in_person(&w, g), "a body leader with a lieutenant gives orders in person");
    let old = w.comp::<Gang>(g).expect("gang").order;
    assert_ne!(old, Order::Raid, "precondition: not raiding already");
    let now = w.tick;
    leaders::hold(&mut w, g, raid_order(now));
    w.run_ticks(1);
    assert_eq!(w.comp::<Gang>(g).expect("gang").order, old, "the members keep the old order till it is given");
    assert!(w.comp::<Gang>(g).expect("gang").pending_order.is_some());
    // Court: the leader holding it at the Hideout, the lieutenant there.
    let hideout = w.hideout_of(g).expect("hideout");
    w.enter_building(l, hideout);
    w.enter_building(lt, hideout);
    let t = w.tick;
    w.comp_mut::<Brain>(l).expect("brain").exec = ExecState::Use { kind: ActionKind::Court, until: t + 60, started: t };
    leaders::run(&mut w);
    let x = w.comp::<Gang>(g).expect("gang");
    assert_eq!(x.order, Order::Raid, "given at court");
    assert!(x.raid_at.is_some_and(|m| m > t), "the muster timed from the giving");
    assert!(x.pending_order.is_none());
    assert_eq!(jobs2(&w).orders_court, 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::OrderGiven && e.actors.as_slice() == [l, lt, g]));
}

/// P15: past the wait the leader walks to the lieutenant and gives it
/// beside him; past the cap it goes by word: a raid slips, never vanishes.
#[test]
fn test_order_walked_to_the_lieutenant_then_by_word() {
    let mut w = city(true);
    let (g, l, lt) = led_gang(&mut w);
    let c = w.config.leaders.clone().expect("[leaders]");
    let hideout = w.hideout_of(g).expect("hideout");
    // Past the wait: the Lead is due and walks to the lieutenant.
    let now = w.tick;
    leaders::hold(&mut w, g, raid_order(now - c.order_wait_ticks));
    assert!(leaders::due(&w, g, l), "the order is walked to a lieutenant past the wait");
    let plan = leaders::plan(&mut w, l, g, hideout).expect("a Lead plan");
    let kinds: Vec<(ActionKind, Option<EntityId>)> = plan.steps.iter().map(|s| (s.action, s.target)).collect();
    assert_eq!(kinds, vec![(ActionKind::GoTo(LocationKey::Victim), Some(lt)), (ActionKind::Court, Some(lt))]);
    // Not beside him: not given.
    let tile = w.comp::<Position>(lt).expect("pos").tile;
    stand(&mut w, lt, tile);
    stand(&mut w, l, citysim::TilePos { x: tile.x, y: tile.y.saturating_add(c.order_reach as u16 + 4) });
    assert!(!leaders::can_court(&w, l, Some(lt)), "too far to give it");
    stand(&mut w, l, citysim::TilePos { x: tile.x.saturating_add(1), y: tile.y });
    assert!(leaders::can_court(&w, l, Some(lt)), "beside him");
    leaders::court_start(&mut w, l, Some(lt));
    assert_eq!(w.comp::<Gang>(g).expect("gang").order, Order::Raid, "given where he was");
    assert_eq!(jobs2(&w).orders_walked, 1);
    // A second order past the cap goes by word.
    let now = w.tick;
    let mut p = raid_order(now - c.order_wait_ticks - c.order_word_ticks);
    p.order = Order::Retaliate;
    leaders::hold(&mut w, g, p);
    leaders::run(&mut w);
    let x = w.comp::<Gang>(g).expect("gang");
    assert_eq!(x.order, Order::Retaliate, "past the cap, by word");
    assert!(x.raid_at.is_some(), "the raid still musters");
    assert_eq!(jobs2(&w).orders_word, 1);
}

/// The Statistical mirror: a leader without a body gives no order in
/// person; one held for him is put in force at once.
#[test]
fn test_statistical_leader_order_in_force_at_once() {
    let mut w = city(true);
    let (g, l, _) = led_gang(&mut w);
    let now = w.tick;
    leaders::hold(&mut w, g, raid_order(now));
    lod::set_lod(&mut w, l, Lod::Statistical);
    assert!(!leaders::in_person(&w, g) && !leaders::collects(&w, g), "off camera: the old path");
    leaders::run(&mut w);
    assert_eq!(w.comp::<Gang>(g).expect("gang").order, Order::Raid, "in force at once");
}

/// P14: a member's SplitLoot at the Hideout while the leader holds court is
/// handed to him in person.
#[test]
fn test_take_handed_to_the_leader_at_court() {
    let mut w = city(true);
    let (g, l, m) = led_gang(&mut w);
    let hideout = w.hideout_of(g).expect("hideout");
    w.enter_building(m, hideout);
    leaders::handed(&mut w, m, 12);
    assert_eq!(jobs2(&w).court_handovers, 0, "no court: no hand-over");
    w.enter_building(l, hideout);
    let t = w.tick;
    w.comp_mut::<Brain>(l).expect("brain").exec = ExecState::Use { kind: ActionKind::Court, until: t + 60, started: t };
    leaders::handed(&mut w, m, 12);
    assert_eq!(jobs2(&w).court_handovers, 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::TakeHanded && e.actors.as_slice() == [m, l, g]));
}
