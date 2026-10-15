//! M16b phase 3 (docs/M16_CONTRACTS.md § 5): the wounded, the Rescue goal
//! and the Trauma Team.
//!
//! A game abstraction between fictional agents of a simulated city: a
//! "wound" is an entry in `World::wounded` replacing a fight's death roll
//! (a second keyed draw, `wound_share × (1 − armour)`): the loser lies
//! where it fell (`Brain.cuffed_by` itself: it neither thinks nor walks,
//! and keeps a body), bleeding on a timer. A friend, a gang-mate, a
//! colleague, a fellow guard or a Trauma crew may carry it to a Clinic
//! (the M9 escort drag) and have it stabilised (`Flow::Treatment`); left
//! too long it dies (a Murder by its attacker, `BledOut`). A Trauma
//! subscription is a `Standing` contract record with a Security corp,
//! paid weekly (`Flow::Trauma`). Off screen the hourly table's killing is
//! split by the same share (`stat_kill_p`).
//!
//! `[wounded] enabled = false` (`CITYSIM_WOUNDED_OFF=1`) is the phase-2
//! city: no draw is made, nobody goes down. Rolls are keyed on
//! `ContractNs::Leverage` with the spec's purpose bytes (17 the wound, 20
//! the subscription).

use crate::components::{
    Brain, Building, BuildingKind, Identity, Job, Lod, Needs, Personality, Position, RelKind, Role, Sentence,
};
use crate::contract::{ContractKind, ContractStatus, Wounded};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{FailReason, StepResult};
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::ownership::{self, Flow};
use crate::systems::{contracts, law};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::word::Deed;
use crate::world::World;

/// Is phase 3 on?
pub fn on(world: &World) -> bool {
    world.config.wounded.enabled
}

/// The spec's purpose bytes on `ContractNs::Leverage`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Purpose {
    Wound = 17,
    Subscribe = 20,
}

fn draw(world: &World, p: Purpose, a: u64, b: u64) -> f32 {
    crate::systems::leverage::leverage_draw(world, p as u8, a, b)
}

/// Is `id` down?
pub fn is_down(world: &World, id: EntityId) -> bool {
    world.wounded.contains_key(&id)
}

// ---------------------------------------------------------------------------
// The wound
// ---------------------------------------------------------------------------

/// From `law::fight_inner` when the death roll hits: the second draw (keyed
/// by the loser and the tick) makes it a wound with `wound_share × (1 −
/// armour)`; at `wounded_quota` the would-be wound stays a death
/// (`wounds_capped`), so deaths only fall. True when the loser went down.
pub fn wound_instead(world: &mut World, loser: EntityId, by: EntityId, armour: f32, murder: bool) -> bool {
    if !on(world) || world.config.wounded.wound_share <= 0.0 || is_down(world, loser) {
        return false;
    }
    if !world.has::<Identity>(loser) || !world.has::<Brain>(loser) {
        return false;
    }
    let p = world.config.wounded.wound_share * (1.0 - armour.clamp(0.0, 1.0));
    if draw(world, Purpose::Wound, u64::from(loser.index), world.tick) >= p {
        return false;
    }
    // Review finding 6: the stabilised hold no slot.
    if world.wounded.values().filter(|w| !w.stable).count() >= world.config.wounded.wounded_quota {
        world.stats.current.wounded.wounds_capped += 1;
        return false;
    }
    down(world, loser, Some(by));
    if let Some(x) = world.wounded.get_mut(&loser) {
        x.murder = murder;
    }
    true
}

/// `id` goes down (by `by`): it lies where it is, a body, bleeding for
/// `bleed_hours`; the beacon (a sighting at conf 1.0) to its gang's and
/// employer's databases and, for a Trauma client, its Trauma corp's
/// (`TraumaCall`). `Wounded`.
pub fn down(world: &mut World, id: EntityId, by: Option<EntityId>) {
    let now = world.tick;
    let tile = world.comp::<Position>(id).map(|p| p.tile).unwrap_or_default();
    world.abort_plan(id);
    if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, id, Lod::Coarse);
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.cuffed_by = Some(id);
        b.current_goal = None;
    }
    let trauma = world.trauma_clients.get(&id).copied();
    let bleed = Tick::from(world.config.wounded.bleed_hours) * TICKS_PER_HOUR;
    world.wounded.insert(
        id,
        Wounded {
            by,
            since: now,
            bleed_until: now + bleed,
            tile,
            stable: false,
            rescuer: None,
            trauma,
            rise_at: None,
            murder: by.is_some(),
            clinic: clinic_near(world, tile),
            care_by: None,
        },
    );
    let s = crate::virt::Sighting { who: id, tile, tick: now, confidence: 1.0, relayed: false };
    let mut owners: Vec<EntityId> = Vec::new();
    owners.extend(world.gang_of(id));
    owners.extend(world.corp_of_agent(id));
    owners.extend(trauma);
    owners.dedup();
    for o in owners {
        crate::systems::virt::record_sighting(world, Some(o), s);
    }
    world.stats.current.wounded.downed += 1;
    let who = by.map_or_else(|| "unknown hands".to_string(), |b| world.name_of(b));
    let text = format!("{} went down, bleeding, at {}'s hands", world.name_of(id), who);
    world.push_event(EventKind::Wounded, &[id, by.unwrap_or(EntityId::NONE)], text);
    if let Some(c) = trauma {
        world.stats.current.wounded.trauma_calls += 1;
        let text = format!("{}'s Trauma Team called to {}", world.owner_label(Some(c)), world.name_of(id));
        world.push_event(EventKind::TraumaCall, &[c, id], text);
    }
}

/// A downed agent killed where it lies (`law::fight_inner`'s shortcut).
pub fn finish(world: &mut World, id: EntityId, by: EntityId) {
    world.wounded.remove(&id);
    world.stats.current.wounded.finished_off += 1;
    world.kill_by(id, crate::components::DeathCause::Violence, Some(by));
}

/// Unstabilised past `bleed_until`: dead by its attacker's hand (a Murder
/// under the Murder rules at the spot; `BledOut`).
fn bleed_out(world: &mut World, id: EntityId) {
    let Some(w) = world.wounded.remove(&id) else { return };
    let tile = world.comp::<Position>(id).map(|p| p.tile).unwrap_or(w.tile);
    world.kill_by(id, crate::components::DeathCause::Violence, w.by);
    world.stats.current.wounded.bled_out += 1;
    let who = w.by.map_or_else(|| "someone".to_string(), |b| world.name_of(b));
    let by = w.by.unwrap_or(EntityId::NONE);
    // Counted as the death would have been (a Murder when it was one).
    if w.murder {
        world.push_event(EventKind::Murder, &[by, id], format!("{who} killed {} (bled out)", world.name_of(id)));
        if let Some(b) = w.by.filter(|&b| law::living(world, b)) {
            law::raise_crime_on(world, b, None, Some(id), crate::components::Crime::Murder, tile);
        }
    }
    world.push_event(EventKind::BledOut, &[id, by], format!("{} bled out", world.name_of(id)));
}

/// Stabilised and rested: on its feet (energy and safety at 0.2).
fn rise(world: &mut World, id: EntityId) {
    world.wounded.remove(&id);
    if let Some(b) = world.comp_mut::<Brain>(id) {
        if b.cuffed_by == Some(id) {
            b.cuffed_by = None;
        }
    }
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.energy = 0.2;
        n.safety = 0.2;
    }
    world.stats.current.wounded.recovered += 1;
    world.push_event(EventKind::Recovered, &[id], format!("{} is back on their feet", world.name_of(id)));
}

// ---------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------

pub fn run(world: &mut World) {
    if !on(world) {
        return;
    }
    if !world.wounded.is_empty() {
        tick_down(world);
    }
    let tod = u64::from(crate::time::tick_of_day(world.tick));
    if tod == 0 {
        trauma_daily(world);
        care_daily(world);
    }
    if tod == TICKS_PER_DAY - 1 {
        world.stats.current.wounded.down_now = world.wounded.len() as u32;
        world.stats.current.wounded.trauma_subs = world.trauma_clients.len() as u32;
    }
}

/// Every tick, over the downed (at most `wounded_quota`): the dead and the
/// jailed leave the list; the bleeding die at `bleed_until`; the
/// stabilised rise; a downed agent whose carrier let go lies where it is.
fn tick_down(world: &mut World) {
    let now = world.tick;
    let all: Vec<(EntityId, Wounded)> = world.wounded.iter().map(|(&k, v)| (k, v.clone())).collect();
    for (id, w) in all {
        if !law::living(world, id) || world.has::<Sentence>(id) {
            world.wounded.remove(&id);
            continue;
        }
        if !w.stable && now >= w.bleed_until {
            bleed_out(world, id);
            continue;
        }
        if w.rise_at.is_some_and(|t| now >= t) {
            rise(world, id);
            continue;
        }
        // Review finding 8: the nearest Clinic, once a tick per bleeding
        // agent (not per thinker × downed in `pick`).
        if !w.stable {
            let at = world.comp::<Position>(id).map_or(w.tile, |p| p.tile);
            let c = clinic_near(world, at);
            if let Some(x) = world.wounded.get_mut(&id) {
                x.clinic = c;
            }
        }
        let cuff = world.comp::<Brain>(id).and_then(|b| b.cuffed_by);
        let carried = cuff
            .filter(|&c| c != id)
            .is_some_and(|c| law::living(world, c) && world.comp::<Brain>(c).is_some_and(|b| b.escorting == Some(id)));
        if !carried && cuff != Some(id) {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.cuffed_by = Some(id);
            }
            if let Some(x) = world.wounded.get_mut(&id) {
                x.rescuer = None;
            }
        }
        // A claim whose rescuer gave up (no Rescue plan) lapses.
        if let Some(r) = w.rescuer {
            let working = law::living(world, r)
                && world.comp::<Brain>(r).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::Rescue);
            if !working {
                if let Some(x) = world.wounded.get_mut(&id) {
                    x.rescuer = None;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The Rescue goal
// ---------------------------------------------------------------------------

/// A gang's temperament (spec § 5): Abandon for a Purist gang or one whose
/// leader's loyalty is under 0.3, else Loyal (computed, not stored).
pub fn gang_abandons(world: &World, gang: EntityId) -> bool {
    crate::systems::creeds::is_purist(world, gang)
        || world
            .comp::<crate::components::Gang>(gang)
            .and_then(|g| g.leader)
            .and_then(|l| world.comp::<Personality>(l))
            .is_some_and(|p| p.loyalty < 0.3)
}

/// What `a` is to the downed `w` for a rescue: the term (`rel_w` for kin,
/// the gang's temperament, 1.0 for a colleague, a fellow city guard or the
/// Trauma corp's guard) and whether it is a Trauma call (any distance).
/// `None`: no tie that carries.
fn tie(world: &World, a: EntityId, w: EntityId, rec: &Wounded) -> Option<(f32, bool, Tie)> {
    if let Some(c) = rec.trauma {
        let guard_of = world
            .comp::<Job>(a)
            .is_some_and(|j| j.role == Role::Guard && j.employer.and_then(|e| world.owner_of(e)) == Some(c));
        if guard_of {
            return Some((1.0, true, Tie::Trauma));
        }
    }
    if let Some((rw, _)) = crate::systems::grudges::relation(world, a, Some(w)).filter(|_| {
        world
            .edge(a, w)
            .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend))
    }) {
        return Some((rw, false, Tie::Kin));
    }
    if let (Some(ga), Some(gw)) = (world.gang_of(a), world.gang_of(w)) {
        if ga == gw {
            if gang_abandons(world, ga) {
                return None;
            }
            let loyalty = world.comp::<Personality>(a).map_or(0.5, |p| p.loyalty);
            return (loyalty >= world.config.wounded.rescue_loyalty).then_some((loyalty, false, Tie::Gang));
        }
    }
    if law::is_city_guard(world, a) && law::is_guard(world, w) && law::is_city_guard(world, w) {
        return Some((1.0, false, Tie::Guard));
    }
    let employer = |x: EntityId| world.comp::<Job>(x).and_then(|j| j.employer);
    if employer(a).is_some() && employer(a) == employer(w) {
        return Some((1.0, false, Tie::Colleague));
    }
    None
}

/// The tie a rescue was made on (the counters).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Tie {
    Kin,
    Gang,
    Colleague,
    Guard,
    Trauma,
}

/// The nearest Clinic to `tile` (open, not derelict).
fn clinic_near(world: &World, tile: crate::components::TilePos) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict) && !world.is_closed(b))
        .filter(|&b| doc_in(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(tile), b)))
        .min()
        .map(|(_, b)| b)
}

/// The downed `a` would go for: unstable, unclaimed (or its own claim),
/// tied to it, within `rescue_radius` (a Trauma call any distance), a
/// Clinic reachable before it bleeds out (a tile a tick); the best term,
/// ties the lower id. `(downed, term, trauma)`.
fn pick(world: &World, a: EntityId) -> Option<(EntityId, f32, bool, Tie)> {
    if world.wounded.is_empty() || is_down(world, a) {
        return None;
    }
    let me = world.comp::<Position>(a)?.tile;
    let r = world.config.wounded.rescue_radius;
    let now = world.tick;
    let mut best: Option<(f32, EntityId, bool, Tie)> = None;
    for (&w, rec) in &world.wounded {
        if w == a || rec.stable || rec.rescuer.is_some_and(|x| x != a) {
            continue;
        }
        let Some((term, trauma, how)) = tie(world, a, w, rec) else { continue };
        let at = world.comp::<Position>(w).map_or(rec.tile, |p| p.tile);
        let d = me.manhattan(at);
        if !trauma && law::chebyshev(me, at) > r {
            continue;
        }
        let Some(c) = rec.clinic else { continue };
        let to_clinic = world.comp::<Building>(c).map_or(u32::MAX, |b| b.door.manhattan(at));
        if now + Tick::from(d) + Tick::from(to_clinic) > rec.bleed_until {
            continue;
        }
        if best.is_none_or(|(bt, bw, _, _)| term > bt || (term == bt && w < bw)) {
            best = Some((term, w, trauma, how));
        }
    }
    best.map(|(t, w, tr, how)| (w, t, tr, how))
}

/// Spec § 5's Rescue considerations: GATE a downed ally to carry; the tie
/// (`rel_w` or the faction term) → Linear{0.8, 0.2}; `courage` →
/// Linear{0.4, 0.6}; `1 − danger` (its attacker within 4 tiles) →
/// Linear{0.5, 0.5}; flat `rescue_flat` (+ `trauma_flat` on a call).
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) {
        return None;
    }
    let (w, term, trauma, _) = pick(world, id)?;
    let rec = world.wounded.get(&w)?;
    let courage = world.comp::<Personality>(id).map_or(0.5, |p| p.courage);
    let at = world.comp::<Position>(w).map(|p| p.tile).unwrap_or(rec.tile);
    let danger = rec
        .by
        .filter(|&b| law::living(world, b))
        .and_then(|b| world.comp::<Position>(b))
        .is_some_and(|p| law::chebyshev(p.tile, at) <= 4);
    let cs = vec![
        Consideration::new("a downed ally", can(true), GATE),
        Consideration::new("the tie", term.clamp(0.0, 1.0), Curve::Linear { m: 0.8, b: 0.2 }),
        Consideration::new("courage", courage, Curve::Linear { m: 0.4, b: 0.6 }),
        Consideration::new("1-danger", if danger { 0.0 } else { 1.0 }, Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    let cfg = &world.config.wounded;
    Some((cs, cfg.rescue_flat + if trauma { cfg.trauma_flat } else { 0.0 }))
}

/// The Rescue's scripted plan: `GoTo(downed) → CarryWounded → GoTo(Clinic)
/// → Stabilise`, the downed claimed.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    let (w, _, _, _) = pick(world, id)?;
    let clinic = world.wounded.get(&w).and_then(|r| r.clinic)?;
    if let Some(x) = world.wounded.get_mut(&w) {
        x.rescuer = Some(id);
    }
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let steps = vec![
        step(ActionKind::GoTo(LocationKey::Victim), Some(w)),
        step(ActionKind::CarryWounded, Some(w)),
        step(ActionKind::GoTo(LocationKey::Clinic), Some(clinic)),
        step(ActionKind::Stabilise, Some(clinic)),
    ];
    Some(Plan { goal: crate::components::GoalKind::Rescue, target: Some(w), steps, started_tick: world.tick })
}

/// A Ripperdoc on `clinic`'s staff (plan deviation: on call; the spec's
/// "inside" left every night's downed to bleed out at a shut door).
fn doc_in(world: &World, clinic: EntityId) -> bool {
    world
        .staff_of(clinic)
        .iter()
        .any(|&o| law::living(world, o) && world.comp::<Job>(o).is_some_and(|j| j.role == Role::Ripperdoc))
}

fn at_building(world: &World, id: EntityId, b: EntityId) -> bool {
    world.comp::<Position>(id).is_some_and(|p| {
        p.building == Some(b)
            || world.comp::<Building>(b).is_some_and(|bd| law::chebyshev(p.tile, world.outside_door(bd)) <= 1)
    })
}

/// The Rescue's own start checks: beside the downed (lying, unstable) to
/// carry it; at the Clinic, carrying it, a Ripperdoc in, to stabilise; the
/// walks as any walk.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    match kind {
        ActionKind::CarryWounded => target.is_some_and(|w| {
            world.wounded.get(&w).is_some_and(|r| !r.stable)
                && world.comp::<Brain>(w).and_then(|b| b.cuffed_by) == Some(w)
                && law::near(world, id, w, 1)
        }),
        ActionKind::Stabilise => target.is_some_and(|c| {
            let carrying = world.comp::<Brain>(id).and_then(|b| b.escorting).is_some_and(|w| is_down(world, w));
            carrying && at_building(world, id, c) && doc_in(world, c)
        }),
        _ => crate::exec::actions::can_start(world, id, kind, target),
    }
}

/// `CarryWounded` done: the downed in tow (the M9 drag).
pub fn on_carry(world: &mut World, id: EntityId, target: Option<EntityId>) -> StepResult {
    let Some(w) = target.filter(|&w| is_down(world, w)) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    if let Some(b) = world.comp_mut::<Brain>(w) {
        b.cuffed_by = Some(id);
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.escorting = Some(w);
    }
    if let Some(x) = world.wounded.get_mut(&w) {
        x.rescuer = Some(id);
    }
    StepResult::Done
}

/// `Stabilise` done at the Clinic: `stabilise_price` to its owner
/// (`Flow::Treatment`) from the carrier's faction (a Trauma crew's: its
/// corp), else the downed's wallet, else the carrier's (what there is);
/// the downed stable, lying inside to rise after `recover_days`;
/// `Rescued` (deed and event), and `TraumaSave` for a crew's client.
pub fn on_stabilise(world: &mut World, id: EntityId, target: Option<EntityId>) -> StepResult {
    let Some(clinic) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let Some(w) = world.comp::<Brain>(id).and_then(|b| b.escorting).filter(|&w| is_down(world, w)) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    let Some(rec) = world.wounded.get(&w).cloned() else { return StepResult::Failed(FailReason::PreconditionLost) };
    let how = tie(world, id, w, &rec).map_or(Tie::Kin, |t| t.2);
    let price = world.config.wounded.stabilise_price;
    let faction = match how {
        Tie::Trauma => rec.trauma,
        Tie::Gang => world.gang_of(id),
        Tie::Colleague => world.corp_of_agent(id),
        _ => None,
    };
    let payer = [faction, Some(w), Some(id)].into_iter().flatten().find(|&p| world.purse(Some(p)) >= price);
    if let Some(x) = world.wounded.get_mut(&w) {
        x.care_by = payer.filter(|&p| p != w);
    }
    let owner = world.owner_of(clinic);
    if let Some(p) = payer {
        let paid = ownership::pay(world, Some(p), owner, price, Flow::Treatment);
        ownership::credit(world, clinic, paid);
    }
    let now = world.tick;
    if let Some(x) = world.wounded.get_mut(&w) {
        x.stable = true;
        x.rise_at = Some(now + Tick::from(world.config.wounded.recover_days) * TICKS_PER_DAY);
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.escorting = None;
    }
    if let Some(b) = world.comp_mut::<Brain>(w) {
        b.cuffed_by = Some(w);
    }
    // Jobs & lives phase 6 (§ 7): a unit off the Clinic's shelf (nothing off).
    crate::systems::gray::on_stabilise(world, clinic);
    let row = &mut world.stats.current.wounded;
    row.stabilised += 1;
    match how {
        Tie::Kin => row.rescued_kin += 1,
        Tie::Gang => row.rescued_gang += 1,
        Tie::Colleague => row.rescued_colleague += 1,
        Tie::Guard => row.rescued_guard += 1,
        Tie::Trauma => row.trauma_saves += 1,
    }
    let d = world.district_of_building(clinic);
    crate::systems::gossip::post_deed(world, d, Deed::Rescued, Some(id), Some(w));
    let text = format!("{} carried {} to {} in time", world.name_of(id), world.name_of(w), world.name_of(clinic));
    world.push_event(EventKind::Rescued, &[id, w, clinic], text);
    if let (Tie::Trauma, Some(c)) = (how, rec.trauma) {
        let text = format!("{}'s Trauma Team saved {}", world.owner_label(Some(c)), world.name_of(w));
        world.push_event(EventKind::TraumaSave, &[c, w], text);
    }
    StepResult::Done
}

/// Review finding 6, at midnight: each stabilised patient lying in care
/// eats off the nearest Market's shelf (`care_food` to its owner,
/// `Flow::Upkeep`, as a captive's meal), paid by its carer, else itself;
/// nobody able, it goes unfed. It rests (energy 1).
fn care_daily(world: &mut World) {
    let cfg = world.config.needs.clone();
    let price = world.config.wounded.care_food;
    let stable: Vec<(EntityId, Wounded)> =
        world.wounded.iter().filter(|(_, w)| w.stable).map(|(&k, v)| (k, v.clone())).collect();
    for (k, w) in stable {
        if !law::living(world, k) {
            continue;
        }
        if let Some(n) = world.comp_mut::<Needs>(k) {
            n.energy = 1.0;
        }
        let at = world.comp::<Position>(k).map_or(w.tile, |p| p.tile);
        let market = world.nearest_of_kind(BuildingKind::Market, at);
        let stocked = market.and_then(|m| world.comp::<Building>(m)).is_some_and(|b| b.stock_food > 0);
        let payer = [w.care_by, Some(k)].into_iter().flatten().find(|&p| world.purse(Some(p)) >= price);
        let (Some(m), Some(p), true) = (market, payer, stocked) else { continue };
        if let Some(b) = world.comp_mut::<Building>(m) {
            b.stock_food -= 1;
        }
        let owner = world.owner_of(m);
        let paid = ownership::pay(world, Some(p), owner, price, Flow::Upkeep);
        ownership::credit(world, m, paid);
        if let Some(n) = world.comp_mut::<Needs>(k) {
            crate::needs::eat(n, &cfg);
        }
        world.mark_day(k, crate::components::trace_flags::ATE);
    }
}

// ---------------------------------------------------------------------------
// The Statistical split
// ---------------------------------------------------------------------------

/// The hour's killing `p` for a Statistical agent with the split applied
/// (`p × (1 − wound_share × stat_saved)`, a Trauma client's further ×
/// `(1 − trauma_stat_save)`); `None` with the wounded off.
pub fn stat_kill_p(world: &World, id: EntityId, p: f32) -> Option<f32> {
    if !on(world) {
        return None;
    }
    let cfg = &world.config.wounded;
    let mut eff = p * (1.0 - cfg.wound_share * cfg.stat_saved).clamp(0.0, 1.0);
    if world.trauma_clients.contains_key(&id) {
        eff *= (1.0 - world.config.trauma.trauma_stat_save).clamp(0.0, 1.0);
    }
    Some(eff)
}

// ---------------------------------------------------------------------------
// The Trauma Team
// ---------------------------------------------------------------------------

/// `World::trauma_clients` from the `Standing` Trauma records (on load).
pub fn reindex(world: &mut World) {
    world.trauma_clients = world
        .contracts
        .values()
        .filter(|c| c.kind == ContractKind::Trauma && c.status == ContractStatus::Standing)
        .filter_map(|c| Some((c.target_agent()?, c.taker?)))
        .collect();
}

/// The Security corp a client would pick (spec: `corps::honour_seller`'s
/// rule with the Trauma fee as the price) and its weekly fee.
pub fn trauma_seller(world: &World) -> Option<(EntityId, i64)> {
    let fee = world.config.trauma.fee_weekly as f32;
    world
        .corps()
        .into_iter()
        .filter(|&c| {
            world
                .comp::<crate::components::Corp>(c)
                .is_some_and(|cc| cc.niches.contains(&crate::components::Niche::Security))
        })
        .map(|c| {
            let level =
                world.comp::<crate::components::Corp>(c).map_or(1.0, |cc| cc.level(crate::components::Niche::Security));
            let honour = crate::systems::reputation::rep(world, c).honour;
            (fee * level * (1.5 - honour), (fee * level).round() as i64, c)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)))
        .map(|(_, f, c)| (c, f.max(1)))
}

/// Subscribe `client` (paid by `buyer`) to `corp`. True when it stands.
pub fn subscribe(world: &mut World, buyer: EntityId, client: EntityId) -> bool {
    if !on(world) || world.trauma_clients.contains_key(&client) {
        return false;
    }
    let Some((corp, fee)) = trauma_seller(world) else { return false };
    if contracts::post_standing(world, buyer, client, corp, fee).is_none() {
        return false;
    }
    world.trauma_clients.insert(client, corp);
    true
}

/// M16b phase 4 (god `Subscribe`): [`subscribe`] with a named Security
/// corp, at its fee (`fee_weekly × its Security level`).
pub fn subscribe_with(world: &mut World, buyer: EntityId, client: EntityId, corp: EntityId) -> bool {
    if !on(world) || world.trauma_clients.contains_key(&client) || !law::living(world, client) {
        return false;
    }
    let Some(cc) = world.comp::<crate::components::Corp>(corp) else { return false };
    if !cc.niches.contains(&crate::components::Niche::Security) {
        return false;
    }
    let fee = ((world.config.trauma.fee_weekly as f32) * cc.level(crate::components::Niche::Security)).round() as i64;
    if contracts::post_standing(world, buyer, client, corp, fee.max(1)).is_none() {
        return false;
    }
    world.trauma_clients.insert(client, corp);
    true
}

/// Daily: the weekly fees (unpaid: the subscription lapses), then the new
/// subscribers: every corp's exec (paid by the corp), and an adult with
/// `subscribe_coins` and `heat ≥ subscribe_heat` or a Hunt or a live Hit
/// or Beat on it, with `p_subscribe` (purpose 20).
fn trauma_daily(world: &mut World) {
    let now = world.tick;
    let week = 7 * TICKS_PER_DAY;
    let due: Vec<crate::contract::ContractId> = world
        .contracts
        .values()
        .filter(|c| c.kind == ContractKind::Trauma && c.status == ContractStatus::Standing)
        .filter(|c| c.last_paid.is_none_or(|t| now >= t + week))
        .map(|c| c.id)
        .collect();
    for id in due {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        let client_alive = c.target_agent().is_some_and(|t| law::living(world, t));
        let payer_ok = c.buyer.is_some_and(|b| world.purse(Some(b)) >= c.price);
        if client_alive && payer_ok {
            ownership::pay(world, c.buyer, c.taker, c.price, Flow::Trauma);
            if let Some(x) = world.contracts.get_mut(&id) {
                x.last_paid = Some(now);
            }
        } else {
            if let Some(t) = c.target_agent() {
                world.trauma_clients.remove(&t);
            }
            world.stats.current.wounded.trauma_lapsed += 1;
            contracts::set_status(world, id, ContractStatus::Expired, "the subscription lapsed");
        }
    }
    for corp in world.corps() {
        if let Some(e) = contracts::head_of(world, corp) {
            if !world.trauma_clients.contains_key(&e) {
                subscribe(world, corp, e);
            }
        }
    }
    let cfg = world.config.trauma.clone();
    let day = world.day();
    // scan-ok: daily, the adults with the coins.
    for a in world.citizens() {
        if world.purse(Some(a)) < cfg.subscribe_coins || world.trauma_clients.contains_key(&a) {
            continue;
        }
        if !crate::systems::demography::is_adult(world, a) || !law::living(world, a) {
            continue;
        }
        let hot = crate::systems::reputation::rep(world, a).heat >= cfg.subscribe_heat;
        let hunted = world.hunted_by.contains_key(&a)
            || world.by_party.get(&a).is_some_and(|l| {
                l.iter().any(|id| {
                    world
                        .contracts
                        .get(id)
                        .is_some_and(|c| c.is_live() && c.kind.violent() && c.target_agent() == Some(a))
                })
            });
        if !hot && !hunted {
            continue;
        }
        if draw(world, Purpose::Subscribe, u64::from(a.index), day) < cfg.p_subscribe {
            subscribe(world, a, a);
        }
    }
}
