//! M16b phase 3 (docs/M16_CONTRACTS.md § 5): the wounded, one test per
//! mechanism in the shipped seed-42 city: deaths with `wound_share > 0`
//! never exceed deaths with it 0 (the fight path, the same seed), a wound
//! that bleeds out is a Murder, the quota keeps the death, a downed agent
//! is finished where it lies, kin carry it to a Clinic and it rises, an
//! Abandon gang leaves its own, the Trauma subscription (weekly, lapsing),
//! its call and save, the Statistical split, and a save mid-wound. Every
//! wound here is game state between fictional agents: a record on a struct
//! and a seeded dice roll. With `[wounded] enabled = false` the mechanism tests
//! fail (the switch gates every entry point).

mod common;

use std::sync::OnceLock;

use citysim::contract::{ContractKind, ContractStatus};
use citysim::systems::econ::identity;
use citysim::systems::{demography, law, wounded};
use citysim::{
    ActionKind, Brain, BuildingKind, EntityId, EventKind, GangMember, Job, Personality, Position, RelKind, Role, World,
    TICKS_PER_DAY,
};

/// The shipped seed-42 city, built once; every test starts from a clone.
static BASE: OnceLock<World> = OnceLock::new();

fn world() -> World {
    BASE.get_or_init(common::world).clone()
}

fn strangers(w: &World, n: usize) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = Vec::new();
    for a in w.citizens() {
        if !law::living(w, a) || !demography::is_adult(w, a) {
            continue;
        }
        if w.has::<GangMember>(a) || law::is_guard(w, a) || w.has::<citysim::Sentence>(a) {
            continue;
        }
        if w.comp::<Brain>(a).is_none_or(|b| b.cuffed_by.is_some()) || w.spouse_of(a).is_some() {
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

fn place_beside(w: &mut World, a: EntityId, b: EntityId) {
    let t = w.comp::<Position>(b).map(|p| p.tile).unwrap();
    w.remove_from_building(a);
    if let Some(p) = w.comp_mut::<Position>(a) {
        p.tile = t;
        p.building = None;
    }
}

fn has_event(w: &World, kind: EventKind, who: EntityId) -> bool {
    w.events.iter().any(|e| e.kind == kind && e.actors.contains(&who))
}

/// Spec § 11: deaths with `wound_share > 0` never exceed deaths with it 0
/// for the same seed, over the fight path: the same fights in two copies
/// of the city, the death roll the same, the wound only ever replacing it.
#[test]
fn test_wounds_never_add_deaths() {
    let base = world();
    let [a, b] = strangers(&base, 2)[..] else { unreachable!() };
    let (mut d0, mut d6, mut wounds) = (0, 0, 0);
    for t in 0..200u64 {
        let mut w0 = base.clone();
        let mut w6 = base.clone();
        w0.config.wounded.wound_share = 0.0;
        w6.config.wounded.wound_share = 0.6;
        for w in [&mut w0, &mut w6] {
            w.tick += t;
        }
        let mods = law::FightMods { kill_p: Some(0.5), ..Default::default() };
        let r0 = law::resolve_fight_mods(&mut w0, a, b, mods);
        let r6 = law::resolve_fight_mods(&mut w6, a, b, mods);
        assert_eq!((r0.0, r0.1), (r6.0, r6.1), "the same fight");
        assert!(!r6.2 || r0.2, "a death only where the woundless city had one");
        d0 += u32::from(r0.2);
        d6 += u32::from(r6.2);
        wounds += w6.wounded.len();
    }
    assert!(d6 <= d0, "deaths {d6} with wounds vs {d0} without");
    assert!(wounds > 0 && d6 < d0, "some deaths became wounds ({wounds})");
}

/// A downed agent left alone dies at `bleed_until`: a Murder by its
/// attacker (counted exactly, `BledOut` beside it).
#[test]
fn test_bleed_out_is_a_murder() {
    let mut w = world();
    let [v, by] = strangers(&w, 2)[..] else { unreachable!() };
    wounded::down(&mut w, v, Some(by));
    assert!(wounded::is_down(&w, v));
    assert_eq!(w.comp::<Brain>(v).and_then(|b| b.cuffed_by), Some(v), "lies where it fell");
    let murders = w.events.iter().filter(|e| e.kind == EventKind::Murder).count();
    let left = w.wounded[&v].bleed_until - w.tick;
    w.run_ticks(left + 2);
    assert!(!law::living(&w, v));
    assert!(has_event(&w, EventKind::BledOut, v));
    assert_eq!(w.events.iter().filter(|e| e.kind == EventKind::Murder).count(), murders + 1);
    assert_eq!(w.stats.current.wounded.bled_out + w.stats.history.back().map_or(0, |r| r.wounded.bled_out), 1);
}

/// At `wounded_quota` a would-be wound stays a death (`wounds_capped`).
#[test]
fn test_quota_keeps_the_death() {
    let mut w = world();
    let [v, by] = strangers(&w, 2)[..] else { unreachable!() };
    w.config.wounded.wounded_quota = 0;
    w.config.wounded.wound_share = 1.0;
    assert!(!wounded::wound_instead(&mut w, v, by, 0.0, true));
    assert_eq!(w.stats.current.wounded.wounds_capped, 1);
}

/// A downed agent attacked is finished where it lies.
#[test]
fn test_downed_is_finished_off() {
    let mut w = world();
    let [v, by] = strangers(&w, 2)[..] else { unreachable!() };
    wounded::down(&mut w, v, Some(by));
    let (winner, loser, died) = law::resolve_fight(&mut w, by, v);
    assert_eq!((winner, loser, died), (by, v, true));
    assert_eq!(w.stats.current.wounded.finished_off, 1);
}

/// Kin carry the downed to a Clinic: the Rescue goal opens, the plan is
/// the spec's four steps; carried and stabilised (paid, coins conserved),
/// it rises after `recover_days` (`Rescued`, `Recovered`).
#[test]
fn test_kin_carry_the_downed_to_a_clinic() {
    let mut w = world();
    let [v, kin, by] = strangers(&w, 3)[..] else { unreachable!() };
    w.edge_entry(v, kin).kind = RelKind::Spouse;
    place_beside(&mut w, kin, v);
    citysim::systems::lod::set_lod(&mut w, kin, citysim::Lod::Full);
    let clinic = w
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .find(|&c| w.staff_of(c).iter().any(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == Role::Ripperdoc)))
        .expect("a staffed Clinic");
    let before = identity(&w);
    wounded::down(&mut w, v, Some(by));
    if let Some(x) = w.wounded.get_mut(&v) {
        x.bleed_until = w.tick + 100 * TICKS_PER_DAY;
    }
    assert!(wounded::considerations(&w, kin).is_some(), "the goal opens");
    let plan = wounded::plan(&mut w, kin).expect("a plan");
    let acts: Vec<ActionKind> = plan.steps.iter().map(|s| s.action).collect();
    assert_eq!(acts[1], ActionKind::CarryWounded);
    assert_eq!(acts[3], ActionKind::Stabilise);
    assert!(wounded::can_start(&w, kin, ActionKind::CarryWounded, Some(v)));
    assert_eq!(wounded::on_carry(&mut w, kin, Some(v)), citysim::StepResult::Done);
    w.enter_building(kin, clinic);
    citysim::systems::law::follow_guard(&mut w, kin);
    assert_eq!(wounded::on_stabilise(&mut w, kin, Some(clinic)), citysim::StepResult::Done);
    assert!(w.wounded[&v].stable);
    assert!(has_event(&w, EventKind::Rescued, v));
    assert_eq!(w.stats.current.wounded.rescued_kin, 1);
    assert_eq!(identity(&w), before);
    w.run_ticks(w.config.wounded.recover_days as u64 * TICKS_PER_DAY + 2);
    assert!(!wounded::is_down(&w, v) && law::living(&w, v));
    assert!(has_event(&w, EventKind::Recovered, v));
}

/// An Abandon gang (its leader's loyalty under 0.3) leaves its downed: a
/// loyal gang-mate has no Rescue to make; the same gang Loyal, it has.
#[test]
fn test_abandon_gang_leaves_its_own() {
    let mut w = world();
    let mut found = None;
    for _ in 0..30 {
        w.run_ticks(TICKS_PER_DAY);
        found = w.gangs().into_iter().find(|&g| {
            w.comp::<citysim::Gang>(g).is_some_and(|x| x.leader.is_some() && x.members.len() >= 3)
                && !citysim::systems::creeds::is_purist(&w, g)
        });
        if found.is_some() {
            break;
        }
    }
    let g = found.expect("a led gang of three");
    let leader = w.comp::<citysim::Gang>(g).unwrap().leader.unwrap();
    let members: Vec<EntityId> =
        w.comp::<citysim::Gang>(g).unwrap().members.iter().copied().filter(|&m| m != leader).collect();
    let (v, mate) = (members[0], members[1]);
    if let Some(p) = w.comp_mut::<Personality>(mate) {
        p.loyalty = 1.0;
    }
    place_beside(&mut w, mate, v);
    // The gang tie alone (a Friend carries whatever the gang's way).
    w.remove_edge(v, mate);
    if let Some(j) = w.comp_mut::<Job>(mate) {
        j.employer = None;
    }
    wounded::down(&mut w, v, None);
    if let Some(x) = w.wounded.get_mut(&v) {
        x.bleed_until = w.tick + 100 * TICKS_PER_DAY;
    }
    if let Some(p) = w.comp_mut::<Personality>(leader) {
        p.loyalty = 0.9;
    }
    assert!(!wounded::gang_abandons(&w, g));
    assert!(wounded::considerations(&w, mate).is_some(), "Loyal: the mate goes");
    if let Some(p) = w.comp_mut::<Personality>(leader) {
        p.loyalty = 0.1;
    }
    assert!(wounded::gang_abandons(&w, g));
    assert!(wounded::considerations(&w, mate).is_none(), "Abandon: the mate leaves it");
}

/// The Trauma subscription: posted `Standing` with the cheapest honest
/// Security corp, the first week paid; a week on with an empty purse it
/// lapses.
#[test]
fn test_trauma_subscription_weekly_and_lapses() {
    let mut w = world();
    let [c] = strangers(&w, 1)[..] else { unreachable!() };
    if let Some(x) = w.comp_mut::<citysim::Wallet>(c) {
        x.coins = 1_000;
    }
    let before = identity(&w);
    assert!(wounded::subscribe(&mut w, c, c));
    let rec = w.contracts.values().find(|x| x.kind == ContractKind::Trauma).expect("a record").clone();
    assert_eq!(rec.status, ContractStatus::Standing);
    assert_eq!(w.purse(Some(c)), 1_000 - rec.price, "the first week");
    assert_eq!(identity(&w), before);
    if let Some(x) = w.comp_mut::<citysim::Wallet>(c) {
        x.coins = 0;
    }
    w.tick = (w.tick / TICKS_PER_DAY + 8) * TICKS_PER_DAY;
    w.tick += u64::from(citysim::systems::midnight::tick_of(&w, citysim::systems::midnight::Mid::Tail));
    wounded::run(&mut w);
    assert_eq!(w.contracts[&rec.id].status, ContractStatus::Expired, "lapsed");
    assert!(!w.trauma_clients.contains_key(&c));
}

/// A client's wound calls its Trauma corp (`TraumaCall`); that corp's guard
/// carries it in from anywhere and the save is a `TraumaSave`.
#[test]
fn test_trauma_call_and_save() {
    let mut w = world();
    let [c, by] = strangers(&w, 2)[..] else { unreachable!() };
    if let Some(x) = w.comp_mut::<citysim::Wallet>(c) {
        x.coins = 1_000;
    }
    assert!(wounded::subscribe(&mut w, c, c));
    let corp = w.trauma_clients[&c];
    let guard = w
        .workers(Role::Guard)
        .iter()
        .copied()
        .find(|&g| w.comp::<Job>(g).and_then(|j| j.employer).and_then(|e| w.owner_of(e)) == Some(corp))
        .expect("a guard of the Trauma corp");
    wounded::down(&mut w, c, Some(by));
    assert!(has_event(&w, EventKind::TraumaCall, c));
    if let Some(x) = w.wounded.get_mut(&c) {
        x.bleed_until = w.tick + 100 * TICKS_PER_DAY;
    }
    let (_, flat) = wounded::considerations(&w, guard).expect("the call reaches the guard");
    assert!(flat >= w.config.wounded.trauma_flat);
    let clinic = wounded::plan(&mut w, guard).expect("a plan").steps[3].target.unwrap();
    place_beside(&mut w, guard, c);
    assert_eq!(wounded::on_carry(&mut w, guard, Some(c)), citysim::StepResult::Done);
    w.enter_building(guard, clinic);
    citysim::systems::law::follow_guard(&mut w, guard);
    assert_eq!(wounded::on_stabilise(&mut w, guard, Some(clinic)), citysim::StepResult::Done);
    assert!(has_event(&w, EventKind::TraumaSave, c));
    assert_eq!(w.stats.current.wounded.trauma_saves, 1);
}

/// Off screen the hour's killing is split: `wound_share × stat_saved`
/// survives, a Trauma client's further by `trauma_stat_save`.
#[test]
fn test_statistical_split() {
    let mut w = world();
    let [a, c] = strangers(&w, 2)[..] else { unreachable!() };
    let p = 0.01;
    let eff = wounded::stat_kill_p(&w, a, p).expect("on");
    let cfg = &w.config.wounded;
    assert!((eff - p * (1.0 - cfg.wound_share * cfg.stat_saved)).abs() < 1e-7);
    if let Some(x) = w.comp_mut::<citysim::Wallet>(c) {
        x.coins = 1_000;
    }
    assert!(wounded::subscribe(&mut w, c, c));
    assert!(wounded::stat_kill_p(&w, c, p).unwrap() < eff, "a client's is lower");
}

/// A save mid-wound runs on byte for byte.
#[test]
fn test_save_mid_wound_round_trips() {
    let mut w = world();
    let [v, by] = strangers(&w, 2)[..] else { unreachable!() };
    wounded::down(&mut w, v, Some(by));
    w.run_ticks(30);
    let text = citysim::save::to_ron(&w);
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text);
    w.run_ticks(TICKS_PER_DAY);
    back.run_ticks(TICKS_PER_DAY);
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w));
}

/// M16b review finding 6: a stabilised patient frees its quota slot and is
/// fed at midnight (its carer pays), not left three days unfed.
#[test]
fn test_stabilised_patient_fed_and_off_the_quota() {
    let mut w = world();
    let [v, kin, by] = strangers(&w, 3)[..] else { unreachable!() };
    w.edge_entry(v, kin).kind = RelKind::Spouse;
    place_beside(&mut w, kin, v);
    citysim::systems::lod::set_lod(&mut w, kin, citysim::Lod::Full);
    if let Some(x) = w.comp_mut::<citysim::Wallet>(kin) {
        x.coins = 1_000;
    }
    let clinic = w
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .find(|&c| w.staff_of(c).iter().any(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == Role::Ripperdoc)))
        .expect("a staffed Clinic");
    wounded::down(&mut w, v, Some(by));
    assert!(w.wounded[&v].clinic.is_some(), "the nearest Clinic cached");
    if let Some(x) = w.wounded.get_mut(&v) {
        x.bleed_until = w.tick + 100 * TICKS_PER_DAY;
    }
    assert_eq!(wounded::on_carry(&mut w, kin, Some(v)), citysim::StepResult::Done);
    w.enter_building(kin, clinic);
    citysim::systems::law::follow_guard(&mut w, kin);
    assert_eq!(wounded::on_stabilise(&mut w, kin, Some(clinic)), citysim::StepResult::Done);
    // The quota is full of the bleeding only: one slot, and the patient holds none.
    w.config.wounded.wounded_quota = 1;
    let [x] = strangers(&w, 1)[..] else { unreachable!() };
    let armour = 0.0;
    let mut downed = false;
    for t in 0..200u64 {
        w.tick += t;
        if wounded::wound_instead(&mut w, x, by, armour, true) {
            downed = true;
            break;
        }
    }
    assert!(downed, "a fresh wound found the slot");
    if let Some(n) = w.comp_mut::<citysim::Needs>(v) {
        n.hunger = 0.2;
    }
    let to_midnight = TICKS_PER_DAY - w.tick % TICKS_PER_DAY;
    w.run_ticks(to_midnight + citysim::systems::midnight::window(&w));
    if wounded::is_down(&w, v) {
        assert!(w.comp::<citysim::Needs>(v).unwrap().hunger > 0.2, "fed in care");
    }
}
