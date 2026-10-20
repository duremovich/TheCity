//! M16b phase 2 (docs/M16_CONTRACTS.md § 4 "Hostages and the private
//! prison", § 2's new kinds): captives, the Private Prison, the hostage
//! coercion, and the kinds `Abduct`, `Rescue`, `Escort`, `Steal`,
//! `Deliver` and `Spin`'s fabricated stories.
//!
//! A game abstraction between fictional agents of a simulated city: a
//! "captive" is an entry in `World::captives` and a flag on the held
//! agent's `Brain` (`cuffed_by` its holding building, so it neither thinks
//! nor walks; `abducted_by`, so it keeps a body); a "hostage" is a
//! `Coercion` with `Leverage::Hostage` whose demand is one seeded
//! Intimidate roll a day; every coin moves through `ownership` inside the
//! coin identity; meals come off a Market's shelf.
//!
//! `[hostage] enabled = false` is the phase-1
//! city: nothing here posts, holds or rolls. Rolls are keyed on
//! `ContractNs::Leverage` with this module's purpose bytes (`Purpose`).

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{Brain, Building, BuildingKind, Corpse, Identity, Lod, Needs, Personality, RelKind, Sentence};
use crate::contract::{
    Captive, Cargo, CoercionStatus, CoercionWhy, ContractId, ContractKind, ContractNs, ContractStatus, Demand, Hold,
    Leverage, Origin, PostExtra, Target,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{FailReason, StepResult};
use crate::systems::ownership::{self, Flow};
use crate::systems::{contracts, law, leverage};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::word::Deed;
use crate::world::World;

/// Is phase 2 on (it rides on leverage's switch too)?
pub fn on(world: &World) -> bool {
    world.config.hostage.enabled && world.config.leverage.enabled
}

/// The purpose bytes of this module's `ContractNs::Leverage` draws.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Purpose {
    /// A hostage killed at its hold's end, key `(coercion, day)`.
    Kill = 16,
    /// A fabricated story exposed, key `(story, day)`.
    Expose = 21,
}

fn draw(world: &World, p: Purpose, a: u64, b: u64) -> f32 {
    crate::systems::leverage::leverage_draw(world, p as u8, a, b)
}

// ---------------------------------------------------------------------------
// Holding buildings
// ---------------------------------------------------------------------------

/// Captives held city-wide may not pass this (`captive_quota`, per capita).
pub fn quota_ok(world: &World) -> bool {
    world.captives.len() < world.config.hostage.captive_quota
}

/// Is `id` held?
pub fn is_captive(world: &World, id: EntityId) -> bool {
    world.captives.contains_key(&id)
}

/// Captives held at `b`.
pub fn held_at(world: &World, b: EntityId) -> usize {
    world.captives.values().filter(|c| c.held_at == b).count()
}

/// A corp's Private Prisons, ascending.
pub fn prisons_of(world: &World, corp: EntityId) -> Vec<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Prison)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(corp) && !bd.demolished))
        .collect()
}

/// Where `holder` keeps a captive: a gang's Hideout, a corp's Prison with
/// room (spec: "Holding buildings: a gang's Hideout, a squat, and the
/// Prison"; a squat is not one here).
pub fn holding_for(world: &World, holder: EntityId) -> Option<EntityId> {
    if world.has::<crate::components::Gang>(holder) {
        return world.hideout_of(holder);
    }
    prisons_of(world, holder)
        .into_iter()
        .find(|&b| held_at(world, b) < usize::from(world.comp::<Building>(b).map_or(0, |bd| bd.capacity)))
}

/// Spec § 4: a corp with a hostage to take and nowhere to keep it builds a
/// Prison on the vacant Lot nearest its first building, paying
/// `prison_cost` to the Treasury (`Flow::Found`; nothing minted). `None`
/// with no Lot or no coins.
pub fn build_prison(world: &mut World, corp: EntityId) -> Option<EntityId> {
    // M16b phase 4: `SetPrisonPermit(false)` forbids new Prisons.
    if !world.levers.prison_permit {
        return None;
    }
    let cost = world.config.hostage.prison_cost;
    if world.purse(Some(corp)) < cost {
        return None;
    }
    let from = world
        .comp::<crate::components::Corp>(corp)
        .and_then(|c| c.buildings.first().copied())
        .and_then(|b| world.comp::<Building>(b).map(|bd| bd.door))?;
    let nearest = |v: Vec<EntityId>, w: &World| {
        v.into_iter().filter_map(|l| w.comp::<Building>(l).map(|b| (b.door.manhattan(from), l))).min().map(|(_, l)| l)
    };
    // A vacant Lot, else a derelict Block refitted (the Fixer's fallback, M16a C9).
    let lot = match nearest(crate::systems::founding::vacant_lots(world), world) {
        Some(l) => {
            crate::systems::founding::build_on_lot(world, l, BuildingKind::Prison, Some(corp)).ok()?;
            l
        }
        None => {
            let homes: Vec<EntityId> = crate::systems::street::derelicts(world)
                .into_iter()
                .filter(|&x| world.comp::<Building>(x).is_some_and(|b| b.kind == BuildingKind::Home))
                .collect();
            let d = nearest(homes, world)?;
            crate::systems::founding::refit_with(world, d, BuildingKind::Prison, Some(corp), false).ok()?;
            d
        }
    };
    crate::systems::industry::pay_build(world, Some(corp), cost, Some(lot), true);
    let text = format!("{} built a Private Prison ({})", world.owner_label(Some(corp)), world.name_of(lot));
    world.push_event(EventKind::Founded, &[corp, lot], text);
    Some(lot)
}

// ---------------------------------------------------------------------------
// Taking and letting go
// ---------------------------------------------------------------------------

/// Who answers for a captive (spec § 4): the captive's employer's exec, its
/// gang's leader, else the living kin with the largest purse; never the
/// holder's own head. `None` when nobody is left.
pub fn over_for(world: &World, captive: EntityId, holder: Option<EntityId>) -> Option<EntityId> {
    let ok = |e: EntityId| {
        e != captive && law::living(world, e) && !world.has::<Sentence>(e) && holder.is_none_or(|h| e != h)
    };
    let head_of_holder = holder.and_then(|h| contracts::head_of(world, h));
    let exec = world.corp_of_agent(captive).filter(|&c| Some(c) != holder).and_then(|c| contracts::head_of(world, c));
    if let Some(e) = exec.filter(|&e| ok(e) && Some(e) != head_of_holder) {
        return Some(e);
    }
    let leader = world.gang_of(captive).filter(|&g| Some(g) != holder).and_then(|g| contracts::head_of(world, g));
    if let Some(l) = leader.filter(|&l| ok(l) && Some(l) != head_of_holder) {
        return Some(l);
    }
    world
        .neighbours(captive)
        .filter(|&o| {
            world
                .edge(captive, o)
                .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent))
        })
        .filter(|&o| ok(o) && world.has::<Identity>(o))
        .map(|o| (world.purse(Some(o)), o))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, o)| o)
}

/// The purse a demand on `over` is paid from: the faction it heads (its
/// corp's treasury, its gang's), else its own wallet.
fn payer_for(world: &World, over: EntityId) -> Option<EntityId> {
    let corp = world.corp_of_agent(over).filter(|&c| contracts::head_of(world, c) == Some(over));
    let gang = world.gang_of(over).filter(|&g| contracts::head_of(world, g) == Some(over));
    // The richer of the faction's purse and the head's own wallet.
    corp.or(gang).filter(|&f| world.purse(Some(f)) >= world.purse(Some(over)))
}

/// Hold `victim` at `at` for `holder` (`agent` handed it over), with a
/// hostage coercion over `over` (or the default answerer) when `demand` is
/// given. `HostageTaken`. False when the quota or the building's room is
/// spent (the taking is not made, spec's LOD rule).
#[allow(clippy::too_many_arguments)]
pub fn take(
    world: &mut World,
    victim: EntityId,
    holder: Option<EntityId>,
    agent: EntityId,
    at: EntityId,
    demand: Option<Demand>,
    over: Option<EntityId>,
) -> bool {
    if !on(world) {
        return false;
    }
    let cap = usize::from(world.comp::<Building>(at).map_or(0, |bd| bd.capacity));
    // M16b phase 4: without the permit a Prison admits no new captive.
    if !world.levers.prison_permit && world.comp::<Building>(at).is_some_and(|bd| bd.kind == BuildingKind::Prison) {
        return false;
    }
    if !quota_ok(world) || held_at(world, at) >= cap.max(1) || is_captive(world, victim) || !law::living(world, victim)
    {
        return false;
    }
    let now = world.tick;
    let until = now + Tick::from(world.config.hostage.hostage_days) * TICKS_PER_DAY;
    world.abort_plan(victim);
    if let Some(b) = world.comp_mut::<Brain>(agent) {
        if b.escorting == Some(victim) {
            b.escorting = None;
        }
    }
    // A body first (a promotion places the agent), then into the hold.
    if world.comp::<Brain>(victim).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, victim, Lod::Coarse);
    }
    world.remove_from_building(victim);
    world.enter_building(victim, at);
    if let Some(b) = world.comp_mut::<Brain>(victim) {
        b.cuffed_by = Some(at);
        b.abducted_by = Some(agent);
        b.current_goal = None;
    }
    let mut coercion = None;
    if let Some(d) = demand {
        if let Some(o) = over.filter(|&o| law::living(world, o)).or_else(|| over_for(world, victim, holder)) {
            let payer = payer_for(world, o);
            // A ransom is set at what the payer can raise (half its purse,
            // at least 10), up to `ransom`: the holder asks what it can get.
            let d = match d {
                Demand::Coins { amount, weekly } => {
                    let can = (world.purse(Some(payer.unwrap_or(o))).max(0) / 2).max(10);
                    Demand::Coins { amount: amount.min(can), weekly }
                }
                other => other,
            };
            let id = leverage::open(
                world,
                holder,
                agent,
                o,
                Leverage::Hostage { captive: victim },
                d,
                CoercionWhy::Hostage,
                payer,
            );
            coercion = Some(id);
        }
    }
    world.captives.insert(victim, Captive { holder, agent, held_at: at, since: now, coercion, until });
    world.stats.current.captive.hostages_taken += 1;
    let text = format!(
        "{} handed {} over at {} for {}",
        world.name_of(agent),
        world.name_of(victim),
        world.name_of(at),
        world.owner_label(holder)
    );
    world.push_event(EventKind::HostageTaken, &[victim, agent, at], text);
    true
}

/// How a captive's hold ended.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Freed {
    /// Its demand met.
    Ransomed,
    /// A Rescue's win.
    Rescued,
    /// The hold gone (the holder, the building, a broken cuff).
    Escaped,
    /// Let go at the hold's end.
    Expired,
}

/// Let `captive` go at the door (`HostageFreed`); its coercion closes.
pub fn release(world: &mut World, captive: EntityId, how: Freed) {
    let Some(c) = world.captives.remove(&captive) else { return };
    if let Some(b) = world.comp_mut::<Brain>(captive) {
        if b.cuffed_by == Some(c.held_at) {
            b.cuffed_by = None;
        }
        b.abducted_by = None;
    }
    if world.comp::<crate::components::Position>(captive).and_then(|p| p.building) == Some(c.held_at) {
        world.leave_building(captive);
    }
    if let Some(id) = c.coercion {
        let status = if how == Freed::Ransomed { CoercionStatus::Complied } else { CoercionStatus::Broken };
        leverage::finish(world, id, status, "the captive is out");
    }
    let row = &mut world.stats.current.captive;
    row.hostages_freed += 1;
    match how {
        Freed::Ransomed => row.ransomed += 1,
        Freed::Rescued => row.rescued_captives += 1,
        Freed::Escaped => row.escaped += 1,
        Freed::Expired => {}
    }
    if law::living(world, captive) {
        let by = Some(c.agent).filter(|&a| a != EntityId::NONE);
        world.remember(captive, crate::components::MemoryKind::Abducted, by, 1.0, -1.0, false);
    }
    // Review finding 4: the help sought for it stands down (a Rescue that
    // won is settled by its own mission).
    close_help(world, captive, c.coercion, how == Freed::Rescued);
    let why = match how {
        Freed::Ransomed => "the demand was met",
        Freed::Rescued => "rescued",
        Freed::Escaped => "the hold was gone",
        Freed::Expired => "let go",
    };
    let text = format!("{} walked out of {} ({why})", world.name_of(captive), world.name_of(c.held_at));
    world.push_event(EventKind::HostageFreed, &[captive, c.held_at], text);
}

/// The help a hold drew, closed with the hold: a live Rescue on `captive`
/// (unless `keep_rescue`: the winning mission settles its own) and the
/// coerced party's Escort on itself, cancelled with the escrow refunded.
fn close_help(world: &mut World, captive: EntityId, coercion: Option<crate::contract::CoercionId>, keep_rescue: bool) {
    let over = coercion.and_then(|id| world.coercions.get(&id)).map(|c| c.over);
    let mut ids: Vec<crate::contract::ContractId> = Vec::new();
    for (t, kind) in [(Some(captive), ContractKind::Rescue), (over, ContractKind::Escort)] {
        let Some(t) = t else { continue };
        if kind == ContractKind::Rescue && keep_rescue {
            continue;
        }
        let live = world.by_party.get(&t).map(|l| l.to_vec()).unwrap_or_default();
        ids.extend(live.into_iter().filter(|id| {
            world.contracts.get(id).is_some_and(|c| c.is_live() && c.kind == kind && c.target == Target::Agent(t))
        }));
    }
    for id in ids {
        contracts::settle(world, id, contracts::Settle::Cancelled, "the captive is out");
    }
}

/// The hold's end with no deal: the holder's agent kills the captive with
/// `p_hostage_kill × (1 − lawfulness)` (a Murder, `HostageKilled`), else
/// lets it go.
fn expire(world: &mut World, captive: EntityId) {
    let Some(c) = world.captives.get(&captive).cloned() else { return };
    let law_a = world.comp::<Personality>(c.agent).map_or(0.5, |p| p.lawfulness);
    let p = world.config.hostage.p_hostage_kill * (1.0 - law_a);
    let key = c.coercion.unwrap_or(u64::from(captive.index));
    let killer_ok = law::living(world, c.agent);
    if killer_ok && draw(world, Purpose::Kill, key, world.day()) < p {
        world.captives.remove(&captive);
        if let Some(id) = c.coercion {
            leverage::finish(world, id, CoercionStatus::Broken, "the captive was killed");
        }
        close_help(world, captive, c.coercion, false);
        let tile = world.comp::<Building>(c.held_at).map(|b| b.door).unwrap_or_default();
        world.kill_by(captive, crate::components::DeathCause::Violence, Some(c.agent));
        law::raise_crime_on(world, c.agent, None, Some(captive), crate::components::Crime::Murder, tile);
        world.stats.current.captive.hostages_killed += 1;
        let text =
            format!("{} killed {} at {}", world.name_of(c.agent), world.name_of(captive), world.name_of(c.held_at));
        world.push_event(EventKind::HostageKilled, &[c.agent, captive], text);
        return;
    }
    release(world, captive, Freed::Expired);
}

// ---------------------------------------------------------------------------
// The Abduct plan's hand-over
// ---------------------------------------------------------------------------

/// `HandOver` (an Abduct plan's last step) at `target`, the record's
/// holding building, with the abductee in tow: held for the buyer (a
/// hostage, with its coercion), or a Harvest's take ripped for the buyer
/// gang; the record fulfilled.
pub fn on_hand_over(world: &mut World, id: EntityId, target: Option<EntityId>) -> StepResult {
    let Some(at) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let Some(run) = world.contract_runs.get(&id).map(|r| r.contract) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    let Some(c) = world.contracts.get(&run).cloned() else { return StepResult::Failed(FailReason::PreconditionLost) };
    let Some(victim) = crate::systems::chrome::dragging(world, id) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    let hold = c.hold.unwrap_or(Hold { at, demand: None, over: None });
    match hold.demand {
        Some(d) => {
            if !take(world, victim, c.buyer, id, at, Some(d), hold.over) {
                // The taking is not made (the quota, the room): the abductee
                // goes at the door and the attempt fails.
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.escorting = None;
                }
                if let Some(b) = world.comp_mut::<Brain>(victim) {
                    b.cuffed_by = None;
                    b.abducted_by = None;
                }
                contracts::fail_attempt(world, run, "no room to hold the captive");
                return StepResult::Failed(FailReason::PreconditionLost);
            }
        }
        None => harvest(world, id, victim, c.buyer),
    }
    contracts::settle(world, run, contracts::Settle::Fulfilled, "handed over");
    StepResult::Done
}

/// A Harvest's take at the buyer gang's Hideout: its implants into the
/// gang's stock (M13's rip, for the gang), then the abductee killed on the
/// table or let go.
fn harvest(world: &mut World, agent: EntityId, victim: EntityId, buyer: Option<EntityId>) {
    let gang = buyer.filter(|&g| world.has::<crate::components::Gang>(g));
    crate::systems::chrome::rip_for(world, agent, victim, gang);
    world.stats.current.captive.harvest_handovers += 1;
}

// ---------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------

pub fn run(world: &mut World) {
    if !on(world) {
        return;
    }
    let tod = u64::from(crate::time::tick_of_day(world.tick));
    if tod == 0 {
        daily(world);
    }
    if tod == TICKS_PER_DAY - 1 {
        snapshot(world);
    }
}

fn snapshot(world: &mut World) {
    let prisons: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Prison).to_vec();
    let pop = world.captives.values().filter(|c| prisons.contains(&c.held_at)).count() as u32;
    let row = &mut world.stats.current.captive;
    row.captives_held = world.captives.len() as u32;
    row.prison_pop = pop;
    row.prisons = prisons.len() as u32;
}

fn daily(world: &mut World) {
    check_holds(world);
    feed(world);
    hostage_rolls(world);
    prison_upkeep(world);
    gang_hostages(world);
    harvest_posts(world);
    bury_posts(world);
    research_steals(world);
    expose(world);
}

/// The holds still standing: a dead captive's entry goes; a cuff broken,
/// the building gone, or the holder gone frees the captive (`Escaped`).
fn check_holds(world: &mut World) {
    let all: Vec<(EntityId, Captive)> = world.captives.iter().map(|(&k, v)| (k, v.clone())).collect();
    for (k, c) in all {
        if !law::living(world, k) {
            world.captives.remove(&k);
            if let Some(id) = c.coercion {
                leverage::finish(world, id, CoercionStatus::Broken, "the captive is dead");
            }
            continue;
        }
        let cuffed = world.comp::<Brain>(k).is_some_and(|b| b.cuffed_by == Some(c.held_at));
        let standing = world.comp::<Building>(c.held_at).is_some_and(|b| !b.demolished && !b.derelict);
        let holder = c.holder.is_none_or(|h| {
            world.has::<crate::components::Corp>(h)
                || world.comp::<crate::components::Gang>(h).is_some_and(|g| !g.members.is_empty())
        });
        if !cuffed || !standing || !holder {
            release(world, k, Freed::Escaped);
        }
    }
}

/// A captive's day (spec: the holder pays `hostage_food`): one meal off
/// the shelf of the Market nearest the holding building, the holder paying
/// the Market's owner (`Flow::Upkeep`); a night's sleep. No coins or no
/// stock: no meal (`captives_unfed`).
pub fn feed(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.needs.clone();
    let price = world.config.hostage.hostage_food;
    let all: Vec<(EntityId, Captive)> = world.captives.iter().map(|(&k, v)| (k, v.clone())).collect();
    for (k, c) in all {
        let door = world.comp::<Building>(c.held_at).map(|b| b.door).unwrap_or_default();
        let market = world.nearest_of_kind(BuildingKind::Market, door);
        let can_pay = c.holder.is_none_or(|h| world.purse(Some(h)) >= price);
        let stocked = market.and_then(|m| world.comp::<Building>(m)).is_some_and(|b| b.stock_food > 0);
        if let Some(n) = world.comp_mut::<Needs>(k) {
            n.energy = 1.0;
        }
        if !can_pay || !stocked {
            world.stats.current.captive.captives_unfed += 1;
            continue;
        }
        let m = market.expect("stocked");
        if let Some(b) = world.comp_mut::<Building>(m) {
            b.stock_food -= 1;
        }
        let owner = world.owner_of(m);
        let paid = ownership::pay(world, c.holder, owner, price, Flow::Upkeep);
        ownership::credit(world, m, paid);
        if let Some(n) = world.comp_mut::<Needs>(k) {
            crate::needs::eat(n, &cfg);
        }
        world.mark_day(k, crate::components::trace_flags::ATE);
    }
}

/// A corp pays each Prison's upkeep to the Treasury (`Flow::Upkeep`).
fn prison_upkeep(world: &mut World) {
    let fee = world.config.hostage.prison_upkeep;
    for b in world.buildings_of_kind(BuildingKind::Prison).to_vec() {
        let owner = world.owner_of(b);
        if owner.is_some() && fee > 0 {
            ownership::charge(world, owner, None, fee, Flow::Upkeep);
        }
    }
}

/// The daily roll of every hostage's demand (spec: "re-rolled daily until
/// `until`"): the holder's head (else the agent who handed it over) leans
/// on the coerced with `Leverage::Hostage`; a success meets the demand and
/// the captive walks; the first refusal sends the coerced for help (a
/// `Rescue` record) and a bodyguard (an `Escort`); past `until` the hold
/// ends ([`expire`]).
fn hostage_rolls(world: &mut World) {
    let keys: Vec<EntityId> = world.captives.keys().copied().collect();
    for k in keys {
        roll_hostage(world, k);
    }
}

/// One captive's day of [`hostage_rolls`]. True when it walked (ransomed).
pub fn roll_hostage(world: &mut World, k: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(c) = world.captives.get(&k).cloned() else { return false };
    if world.tick >= c.until {
        expire(world, k);
        return false;
    }
    let Some(cid) = c.coercion else { return false };
    let Some(co) = world.coercions.get(&cid).cloned().filter(|x| x.is_active()) else { return false };
    if !law::living(world, co.over) || world.has::<Sentence>(co.over) {
        return false;
    }
    let head = c.holder.and_then(|h| contracts::head_of(world, h)).unwrap_or(c.agent);
    if !law::living(world, head) {
        return false;
    }
    let out = leverage::roll(world, head, co.over, co.payer, &co.leverage, &co.demand);
    if out.success && comply(world, &co) {
        release(world, k, Freed::Ransomed);
        return true;
    }
    // Refused, or willing and unable: the coerced looks for help.
    seek_help(world, k, co.over, co.payer);
    false
}

/// The hold's end, now (tests; the daily pass reaches it past `until`).
pub fn end_hold(world: &mut World, captive: EntityId) {
    if on(world) {
        expire(world, captive);
    }
}

/// The demand met. Coins: from the payer to the holder (`Flow::Coerced`);
/// a sale: the holder buys the building at its value. False when it could
/// not be met (the payer short, the sale refused).
fn comply(world: &mut World, co: &crate::contract::Coercion) -> bool {
    match co.demand {
        Demand::Coins { amount, .. } => {
            let payer = co.payer.unwrap_or(co.over);
            if world.purse(Some(payer)) < amount {
                return false;
            }
            let paid = ownership::pay(world, Some(payer), co.holder, amount, Flow::Coerced);
            world.stats.current.captive.flow_ransom += paid;
            paid > 0
        }
        Demand::Sell(b) => {
            let Some(h) = co.holder else { return false };
            let price = world.comp::<Building>(b).map_or(0, |bd| ownership::value(world, bd.kind));
            crate::systems::corps::acquire(world, h, b, price, "for a hostage")
        }
        // M16b phase 4 (spec § 11): the captain lets a convict out only
        // when the law is weak and the captain bent (`Law.competence < 0.4`,
        // lawfulness < 0.5).
        Demand::Release(convict) => {
            let comp = world.law().map_or(1.0, |l| l.competence);
            let law = world.comp::<crate::components::Personality>(co.over).map_or(1.0, |p| p.lawfulness);
            if comp >= 0.4 || law >= 0.5 || !world.has::<Sentence>(convict) {
                return false;
            }
            law::release(world, convict, false);
            true
        }
        _ => false,
    }
}

/// The coerced will not pay: once per captive it posts a `Rescue` on the
/// captive (its own gang's job when it leads one, else brokered at the
/// nearest open Fixer) and, with the coins left, an `Escort` on itself.
pub fn seek_help(world: &mut World, captive: EntityId, over: EntityId, payer: Option<EntityId>) {
    if !on(world) {
        return;
    }
    let buyer = payer.unwrap_or(over);
    let posted = |w: &World, kind: ContractKind, t: EntityId| {
        w.by_party.get(&t).is_some_and(|l| {
            l.iter().any(|id| {
                w.contracts.get(id).is_some_and(|c| c.is_live() && c.kind == kind && c.target == Target::Agent(t))
            })
        })
    };
    if !posted(world, ContractKind::Rescue, captive) {
        let gang = world.has::<crate::components::Gang>(buyer).then_some(buyer);
        let broker = if gang.is_some() { None } else { contracts::nearest_fixer(world, over) };
        let broker = broker.filter(|&f| contracts::fixer_open(world, f));
        let origin = match gang {
            Some(g) => Origin::GangOrder(world.comp::<crate::components::Gang>(g).map(|x| x.order).unwrap_or_default()),
            None => Origin::God,
        };
        let tgt = Target::Agent(captive);
        let price = contracts::organic_price(world, ContractKind::Rescue, &tgt, Some(buyer), Some(over), broker);
        let r = contracts::brain_post_with(
            world,
            Some(buyer),
            Some(over),
            ContractKind::Rescue,
            tgt,
            broker,
            origin,
            Some(price),
            PostExtra::default(),
        );
        if let (Ok(id), Some(g)) = (r, gang) {
            // The gang works its own: the job at once when it can muster.
            let _ = contracts::accept_gang(world, id, g);
        }
    }
    if !posted(world, ContractKind::Escort, over) && world.has::<Identity>(over) {
        let broker = contracts::nearest_fixer(world, over).filter(|&f| contracts::fixer_open(world, f));
        let _ = contracts::brain_post_with(
            world,
            Some(buyer),
            Some(over),
            ContractKind::Escort,
            Target::Agent(over),
            broker,
            Origin::God,
            None,
            PostExtra::default(),
        );
    }
}

// ---------------------------------------------------------------------------
// Rescue: the mission's defenders and its win
// ---------------------------------------------------------------------------

/// The defenders of a captive's holding building: the holder gang's
/// members inside, the building's posted guards (strongest first; the
/// robots first). Empty when `captive` is not held.
pub fn guards_at(world: &World, captive: EntityId) -> Vec<EntityId> {
    let Some(c) = world.captives.get(&captive) else { return Vec::new() };
    let mut out: Vec<EntityId> = Vec::new();
    if let Some(g) = c.holder.and_then(|h| world.comp::<crate::components::Gang>(h)) {
        out.extend(g.members.iter().copied().filter(|&m| {
            law::living(world, m)
                && !world.has::<Sentence>(m)
                && world.comp::<crate::components::Position>(m).and_then(|p| p.building) == Some(c.held_at)
        }));
    }
    out.extend(crate::systems::raid::posted_guards(world, c.held_at));
    out.sort_unstable();
    out.dedup();
    crate::systems::raid::by_strength(world, &mut out);
    crate::systems::raid::robots_first(world, c.held_at, &mut out);
    out
}

/// A captive's holding building's door (the Rescue mission's).
pub fn hold_of(world: &World, captive: EntityId) -> Option<EntityId> {
    world.captives.get(&captive).map(|c| c.held_at)
}

/// A Rescue won at `building`: every captive of `captive`'s holder held
/// there walks out (`Rescued` deed, the crew the actors).
pub fn rescue_win(world: &mut World, captive: EntityId, crew: &[EntityId]) {
    if !on(world) {
        return;
    }
    let Some(c) = world.captives.get(&captive).cloned() else { return };
    let freed: Vec<EntityId> = world
        .captives
        .iter()
        .filter(|(_, x)| x.held_at == c.held_at && x.holder == c.holder)
        .map(|(&k, _)| k)
        .collect();
    let d = world.district_of_building(c.held_at);
    for k in freed {
        release(world, k, Freed::Rescued);
        // The others freed with it: their own Rescues stand down.
        if k != captive {
            close_help(world, k, None, false);
        }
        if let Some(&a) = crew.first() {
            crate::systems::gossip::post_deed(world, d, Deed::Rescued, Some(a), Some(k));
        }
    }
}

// ---------------------------------------------------------------------------
// Who takes hostages, and the other postings
// ---------------------------------------------------------------------------

/// A live Abduct the buyer placed.
pub(crate) fn abducting(world: &World, buyer: EntityId) -> bool {
    world.by_party.get(&buyer).is_some_and(|l| {
        l.iter().any(|id| world.contracts.get(id).is_some_and(|c| c.is_live() && c.kind == ContractKind::Abduct))
    })
}

/// Post an Abduct record with its hold (direct when `broker` is `None`).
#[allow(clippy::too_many_arguments)]
pub fn post_abduct(
    world: &mut World,
    buyer: EntityId,
    agent: Option<EntityId>,
    victim: EntityId,
    broker: Option<EntityId>,
    origin: Origin,
    price: Option<i64>,
    hold: Hold,
) -> Option<ContractId> {
    if !on(world) || !quota_ok(world) || is_captive(world, victim) {
        return None;
    }
    let id = contracts::brain_post_with(
        world,
        Some(buyer),
        agent,
        ContractKind::Abduct,
        Target::Agent(victim),
        broker,
        origin,
        price,
        PostExtra::default(),
    )
    .ok()?;
    if let Some(c) = world.contracts.get_mut(&id) {
        c.hold = Some(hold);
    }
    Some(id)
}

/// The rival leader's spouse, else a child (living, free, not held).
fn kin_of_leader(world: &World, rival: EntityId) -> Option<EntityId> {
    let l = contracts::head_of(world, rival)?;
    let ok = |k: EntityId| law::living(world, k) && !world.has::<Sentence>(k) && !is_captive(world, k);
    if let Some(s) = world.spouse_of(l).filter(|&s| ok(s)) {
        return Some(s);
    }
    crate::systems::demography::children_of_agent(world, l).into_iter().filter(|&c| ok(c)).min()
}

/// Spec § 4 "Who takes hostages": a gang under Retaliate or Job weaker
/// than its rival gang (members `own ÷ rival < hire_ratio`) takes the rival
/// leader's Spouse or child, held at its Hideout for `ransom` coins from
/// the rival's leader: a direct Abduct to its own members. One at a time.
pub fn gang_hostages(world: &mut World) {
    if !on(world) {
        return;
    }
    let ratio = world.config.contracts.hire_ratio;
    for g in world.gangs() {
        let Some(x) = world.comp::<crate::components::Gang>(g) else { continue };
        let (order, own) = (x.order, x.members.len());
        if !matches!(order, crate::components::Order::Retaliate | crate::components::Order::Job) {
            continue;
        }
        let Some(rival) =
            crate::systems::raid::expedition_rival(world, g).filter(|&r| world.has::<crate::components::Gang>(r))
        else {
            continue;
        };
        let theirs = world.comp::<crate::components::Gang>(rival).map_or(0, |r| r.members.len());
        if theirs == 0 || own as f32 / theirs as f32 >= ratio {
            continue;
        }
        if abducting(world, g) || world.captives.values().any(|c| c.holder == Some(g)) {
            continue;
        }
        let (Some(victim), Some(at)) = (kin_of_leader(world, rival), holding_for(world, g)) else { continue };
        let leader = contracts::head_of(world, g);
        let over = contracts::head_of(world, rival);
        // Spec § 4: "by the holder's members": the leader's order to its
        // strongest free member (unpaid; a gang's treasury is no purse for
        // its own work).
        let Some(member) = order_taker(world, g, victim) else { continue };
        let demand = Demand::Coins { amount: world.config.hostage.ransom, weekly: false };
        let hold = Hold { at, demand: Some(demand), over };
        let p = crate::contract::Posting {
            buyer: Some(g),
            agent: leader,
            kind: ContractKind::Abduct,
            target: Target::Agent(victim),
            broker: None,
            deadline_days: world.config.contracts.deadline_days.get(ContractKind::Abduct),
            origin: Origin::GangOrder(order),
            price: None,
        };
        if !quota_ok(world) {
            return;
        }
        if let Some(id) = contracts::post_order(world, p, PostExtra::default(), member) {
            if let Some(c) = world.contracts.get_mut(&id) {
                c.hold = Some(hold);
            }
        }
    }
}

/// Jobs & lives phase 5 (§ 5.2): a gang's Dollhouse front orders its
/// strongest free member to abduct `victim`, held at `at` for a ransom put
/// to the victim's kin (the `gang_hostages` shape). False when no order is
/// posted (the quota, no member, already abducting).
pub fn front_abduct(world: &mut World, g: EntityId, victim: EntityId, at: EntityId) -> bool {
    if !on(world) || !quota_ok(world) || abducting(world, g) || is_captive(world, victim) {
        return false;
    }
    let Some(order) = world.comp::<crate::components::Gang>(g).map(|x| x.order) else { return false };
    let Some(member) = order_taker(world, g, victim) else { return false };
    let demand = Demand::Coins { amount: world.config.hostage.ransom, weekly: false };
    let hold = Hold { at, demand: Some(demand), over: None };
    let p = crate::contract::Posting {
        buyer: Some(g),
        agent: contracts::head_of(world, g),
        kind: ContractKind::Abduct,
        target: Target::Agent(victim),
        broker: None,
        deadline_days: world.config.contracts.deadline_days.get(ContractKind::Abduct),
        origin: Origin::GangOrder(order),
        price: None,
    };
    let Some(id) = contracts::post_order(world, p, PostExtra::default(), member) else { return false };
    if let Some(c) = world.contracts.get_mut(&id) {
        c.hold = Some(hold);
    }
    true
}

/// A gang's member for an order on `target`: free, not on a run or a
/// mission, no record held, no tie to the target; the strongest (ties the
/// lower id).
fn order_taker(world: &World, g: EntityId, target: EntityId) -> Option<EntityId> {
    let members = world.comp::<crate::components::Gang>(g)?.members.clone();
    members
        .into_iter()
        .filter(|&m| m != target && contracts::free_adult(world, m) && !contracts::holds_taken(world, m))
        .filter(|&m| !world.contract_runs.contains_key(&m) && !world.mission_of.contains_key(&m))
        .filter(|&m| {
            world.edge(m, target).is_none_or(|e| !matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent))
        })
        .map(|m| (crate::systems::raid::strength(world, m), m))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, m)| m)
}

/// Spec § 1 "Gang Harvest": a gang under Harvest with fewer than three fit
/// members buys the abduction of its Harvest target, priced `abduct_share ×
/// chrome_value`, brokered at an open Fixer in a held district, else
/// direct; the take goes to its Hideout.
pub fn harvest_posts(world: &mut World) {
    if !on(world) {
        return;
    }
    for g in world.gangs() {
        let Some(x) = world.comp::<crate::components::Gang>(g) else { continue };
        if x.order != crate::components::Order::Harvest {
            continue;
        }
        let fit = x.members.iter().filter(|&&m| law::living(world, m) && contracts::free_adult(world, m)).count();
        if fit >= 3 || abducting(world, g) {
            continue;
        }
        let Some(victim) = crate::systems::chrome::gang_harvest_target(world, g) else { continue };
        let Some(at) = world.hideout_of(g) else { continue };
        let value = world.comp::<crate::components::Kit>(victim).map_or(0, |k| k.chrome_value);
        let price = ((world.config.hostage.abduct_share * value as f32).round() as i64).max(1);
        if world.purse(Some(g)) < price {
            continue;
        }
        let broker = contracts::fixer_in_held(world, g);
        let leader = contracts::head_of(world, g);
        post_abduct(
            world,
            g,
            leader,
            victim,
            broker,
            Origin::GangOrder(x.order),
            Some(price),
            Hold { at, demand: None, over: None },
        );
    }
}

/// Spec § 1 "Bury": a corpse unburied `bury_hours` with no living adult kin
/// who keeps a body (Full or Coarse), and a kin who can pay the quote,
/// gets a `Deliver` of the corpse to the Recycler (direct, or brokered at
/// the kin's nearest open Fixer).
pub fn bury_posts(world: &mut World) {
    if !on(world) {
        return;
    }
    let Some(recycler) = world.building_of_kind(BuildingKind::Cemetery) else { return };
    let age = Tick::from(world.config.hostage.bury_hours) * crate::time::TICKS_PER_HOUR;
    let now = world.tick;
    // scan-ok: daily, the corpses.
    let corpses: Vec<EntityId> = world
        .with::<Corpse>()
        .into_iter()
        .filter(|&c| world.comp::<Corpse>(c).is_some_and(|k| !k.buried && now.saturating_sub(k.died_tick) >= age))
        .collect();
    for c in corpses {
        let target = Target::Carry { what: Cargo::Corpse(c), to: recycler };
        if world.by_party.get(&c).is_some_and(|l| !l.is_empty()) {
            continue;
        }
        let kin: Vec<EntityId> = world
            .neighbours(c)
            .filter(|&o| {
                world.edge(c, o).is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent))
            })
            .filter(|&o| law::living(world, o) && crate::systems::demography::is_adult(world, o))
            .collect();
        if kin.is_empty() || kin.iter().any(|&k| world.comp::<Brain>(k).is_some_and(|b| b.lod != Lod::Statistical)) {
            continue;
        }
        let price = contracts::quote(world, ContractKind::Deliver, &target, None);
        let Some(&buyer) = kin
            .iter()
            .filter(|&&k| world.purse(Some(k)) >= price)
            .max_by_key(|&&k| (world.purse(Some(k)), std::cmp::Reverse(k)))
        else {
            continue;
        };
        let broker = contracts::nearest_fixer(world, buyer).filter(|&f| contracts::fixer_open(world, f));
        let _ = contracts::brain_post_with(
            world,
            Some(buyer),
            Some(buyer),
            ContractKind::Deliver,
            target,
            broker,
            Origin::Bury,
            Some(price),
            PostExtra::default(),
        );
    }
}

/// Spec § 1 "Corp Research": a corp under Research with no runner of its
/// own on a run and a rival Lab holding Data buys a heist on that Lab (a
/// `Steal` on the building; its node's run), brokered at the exec's
/// nearest open Fixer. One at a time.
pub fn research_steals(world: &mut World) {
    if !on(world) {
        return;
    }
    for corp in world.corps() {
        let Some(c) = world.comp::<crate::components::Corp>(corp) else { continue };
        if c.order != crate::components::CorpOrder::Research {
            continue;
        }
        let Some(exec) = contracts::head_of(world, corp) else { continue };
        let own_run = world.run_orders.values().any(|o| o.patron == Some(corp));
        let stealing = world.by_party.get(&corp).is_some_and(|l| {
            l.iter().any(|id| world.contracts.get(id).is_some_and(|x| x.is_live() && x.kind == ContractKind::Steal))
        });
        if own_run || stealing {
            continue;
        }
        let lab = world
            .buildings_of_kind(BuildingKind::Lab)
            .iter()
            .copied()
            .filter(|&b| world.owner_of(b).is_some_and(|o| o != corp && world.has::<crate::components::Corp>(o)))
            .filter_map(|b| {
                let n = crate::systems::virt::node_of_building(world, b)?;
                let units: u32 = world.virt.node(n).map_or(0, |x| x.store.units.iter().sum());
                (units > 0).then_some((units, b))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(_, b)| b);
        let Some(lab) = lab else { continue };
        let broker = contracts::nearest_fixer(world, exec).filter(|&f| contracts::fixer_open(world, f));
        let _ = contracts::brain_post_with(
            world,
            Some(corp),
            Some(exec),
            ContractKind::Steal,
            Target::Building(lab),
            broker,
            Origin::CorpOrder(crate::components::CorpOrder::Research),
            None,
            PostExtra::default(),
        );
    }
}

/// From `corp_brain::acquire` when a seller refuses (M15 W33): the
/// refusal counted; at the second, a buyer whose exec is under
/// `dirty_lobby` takes the seller exec's Spouse or child into its Private
/// Prison (built when it has none) for the building's sale: an Abduct
/// brokered at the exec's nearest open Fixer, else direct.
pub fn on_refused(world: &mut World, corp: EntityId, seller: EntityId, building: EntityId) {
    if !on(world) {
        return;
    }
    let n = world.refusals.entry((corp, seller)).or_insert(0);
    *n = n.saturating_add(1);
    if *n < 2 {
        return;
    }
    let Some(exec) = contracts::head_of(world, corp) else { return };
    if world.comp::<Personality>(exec).is_none_or(|p| p.lawfulness >= world.config.contracts.dirty_lobby) {
        return;
    }
    if abducting(world, corp) || world.captives.values().any(|c| c.holder == Some(corp)) {
        return;
    }
    let Some(victim) = kin_of_leader(world, seller) else { return };
    let at = match holding_for(world, corp) {
        Some(b) => b,
        None => match build_prison(world, corp) {
            Some(b) => b,
            None => return,
        },
    };
    let broker = contracts::nearest_fixer(world, exec).filter(|&f| contracts::fixer_open(world, f));
    let over = contracts::head_of(world, seller);
    let hold = Hold { at, demand: Some(Demand::Sell(building)), over };
    let price =
        contracts::organic_price(world, ContractKind::Abduct, &Target::Agent(victim), Some(corp), Some(exec), broker);
    let origin = Origin::CorpOrder(crate::components::CorpOrder::Acquire);
    post_abduct(world, corp, Some(exec), victim, broker, origin, Some(price), hold);
    world.refusals.remove(&(corp, seller));
}

// ---------------------------------------------------------------------------
// Steal and Deliver: the ledger rows
// ---------------------------------------------------------------------------

/// A Steal on a building at its ledger due (no heist run going): the
/// strike's `p_win` against the building's defenders; a win takes
/// `steal_share` of the owner's purse (at most `steal_max`) to the buyer
/// (`Flow::Robbery`, the owner's loss booked, a `Robbed` deed told).
pub fn resolve_steal(world: &mut World, id: ContractId, lead: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let Target::Building(b) = c.target else { return };
    let p_win = crate::systems::missions::estimate(world, &[lead], &c.target);
    let win = world.rng.contract(ContractNs::Outcome, id, u64::from(c.attempts)).random::<f32>() < p_win;
    if !win {
        contracts::fail_attempt(world, id, "the theft was foiled");
        return;
    }
    let owner = world.owner_of(b);
    let cfg = &world.config.hostage;
    let take = ((world.purse(owner).max(0) as f32 * cfg.steal_share) as i64).min(cfg.steal_max);
    let got = if take > 0 { ownership::pay(world, owner, c.buyer, take, Flow::Robbery) } else { 0 };
    if got > 0 {
        ownership::note_loss(world, b, got, Some(lead));
    }
    let d = world.district_of_building(b);
    crate::systems::gossip::post_deed(world, d, Deed::Robbed, Some(lead), owner);
    contracts::settle(world, id, contracts::Settle::Fulfilled, &format!("{got} taken"));
}

/// A Deliver at its ledger due: the corpse carried and buried by the taker
/// (`demography::bury`); one already buried cancels the record.
pub fn resolve_deliver(world: &mut World, id: ContractId, lead: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let Target::Carry { what: Cargo::Corpse(corpse), .. } = c.target else {
        contracts::settle(world, id, contracts::Settle::Cancelled, "nothing to carry");
        return;
    };
    if crate::systems::demography::bury(world, lead, corpse) {
        contracts::settle(world, id, contracts::Settle::Fulfilled, "");
    } else {
        contracts::settle(world, id, contracts::Settle::Cancelled, "the body is gone");
    }
}

/// A Steal on a Lab with a node, at acceptance: the taker gets the heist
/// as an M14 run order (`patron` the buyer, `RunWhy::Contract`) when it has
/// a deck and a chair; `true` when given (the record then waits on the run).
pub fn start_heist(world: &mut World, id: ContractId, taker: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let Target::Building(lab) = c.target else { return false };
    let Some(node) = crate::systems::virt::node_of_building(world, lab) else { return false };
    if !world.comp::<crate::components::Kit>(taker).is_some_and(|k| k.deck.is_some())
        || world.run_orders.contains_key(&taker)
    {
        return false;
    }
    let Some(chair) = crate::systems::virt::chair_for(world, taker, c.buyer) else { return false };
    let now = world.tick;
    let order = crate::virt::RunOrder {
        patron: c.buyer,
        purpose: crate::virt::Purpose::Data { wipe: false },
        target: node,
        chair: chair.building,
        not_before: now,
        expires: c.deadline,
        why: crate::virt::RunWhy::Contract,
        mode: crate::virt::RunMode::Quiet,
    };
    crate::systems::virt::give_order(world, taker, order);
    world.steal_runs.insert(taker, id);
    if world.comp::<Brain>(taker).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, taker, Lod::Coarse);
    }
    world.stats.current.captive.steal_runs += 1;
    true
}

/// From `virt::end_run` for a `RunWhy::Contract` run: a success with Data
/// on the deck moves it to the buyer's Data Lab and fulfils the record;
/// anything else fails the attempt.
pub fn on_heist_end(world: &mut World, runner: EntityId, success: bool) {
    let Some(id) = world.steal_runs.remove(&runner) else { return };
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if c.status != ContractStatus::Taken {
        return;
    }
    let deck = world.comp::<crate::components::Kit>(runner).and_then(|k| k.deck);
    let data = deck.and_then(|d| world.comp::<crate::components::Asset>(d)).map(|a| a.data).unwrap_or([0; 3]);
    if !success || data.iter().sum::<u32>() == 0 {
        contracts::fail_attempt(world, id, "the heist came back empty");
        return;
    }
    let buyer = c.buyer.unwrap_or(EntityId::NONE);
    for t in crate::virt::Track::ALL {
        let n = data[t.index()];
        if n == 0 {
            continue;
        }
        let lab = crate::systems::tech::data_lab_for(world, buyer, t);
        if let Some(node) = lab.and_then(|b| crate::systems::virt::node_of_building(world, b)) {
            if let Some(x) = world.virt.node_mut(node) {
                x.store.units[t.index()] = x.store.units[t.index()].saturating_add(n);
            }
        }
    }
    if let Some(a) = deck.and_then(|d| world.comp_mut::<crate::components::Asset>(d)) {
        a.data = [0; 3];
    }
    contracts::settle(world, id, contracts::Settle::Fulfilled, "the Data delivered");
}

// ---------------------------------------------------------------------------
// Spin: fabricated stories and their exposure
// ---------------------------------------------------------------------------

/// Spec § 4 Fraud: a spinning corp with no true rival misdeed to plant and
/// an exec under `fabricate_lawfulness` buys a fabricated story on `feed`:
/// a `Spin` record (direct, price `plant_price × reach × fabricate_mult`)
/// naming its rival's exec as having betrayed it, taken by the Feed's
/// owner; the exec's Deceive against the Feed's sharpest reporter decides
/// (a Feed that sees through it refuses: the record fails and the move
/// posts `Betrayed` against the exec). Returns the deed to plant.
pub fn fabricate(world: &mut World, corp: EntityId, feed: EntityId, reach: f32) -> Option<crate::word::PoolEntry> {
    if !on(world) {
        return None;
    }
    let exec = contracts::head_of(world, corp)?;
    if world.comp::<Personality>(exec).is_none_or(|p| p.lawfulness >= world.config.leverage.fabricate_lawfulness) {
        return None;
    }
    let actor = fabricate_target(world, corp)?;
    // One try a corp per `retry_days` (a Feed that saw through it is wary).
    if leverage::tried_recently(world, corp, actor, leverage::tried::FABRICATE) {
        return None;
    }
    leverage::mark(world, corp, actor, leverage::tried::FABRICATE);
    let taker = world.owner_of(feed)?;
    if taker == corp {
        return None;
    }
    let price =
        ((world.config.news.plant_price as f32 * reach * world.config.leverage.fabricate_mult).round() as i64).max(1);
    if world.purse(Some(corp)) < price {
        return None;
    }
    let target = Target::Story { deed: Deed::Betrayed, actor, object: Some(corp), fabricated: true };
    let id = contracts::brain_post_with(
        world,
        Some(corp),
        Some(exec),
        ContractKind::Spin,
        target,
        None,
        Origin::Spin,
        Some(price),
        PostExtra::default(),
    )
    .ok()?;
    if !contracts::accept(world, id, taker, &[]) {
        contracts::settle(world, id, contracts::Settle::Cancelled, "the Feed would not take it");
        return None;
    }
    let reporter = crate::systems::news::sharpest_reporter(world, feed);
    let seen_through = reporter.is_some_and(|r| {
        let m = crate::word::SocialMove {
            actor: exec,
            target: r,
            kind: crate::word::MoveKind::Deceive,
            stake: crate::word::Stake::Coins(price),
        };
        !crate::systems::moves::resolve(world, &m).success
    });
    if seen_through {
        contracts::settle(world, id, contracts::Settle::Failed, "the Feed saw through it");
        return None;
    }
    contracts::settle(world, id, contracts::Settle::Fulfilled, "planted");
    world.stats.current.captive.fabricated += 1;
    let d = world.district_of_building(feed);
    Some(crate::word::PoolEntry {
        deed: Deed::Betrayed,
        actor: Some(actor),
        object: Some(corp),
        tick: world.tick,
        hops: 1,
        reach: 0.0,
        hole: None,
        story: None,
        kin: SmallVec::new(),
        told: SmallVec::new(),
        district: d,
        press: 0,
    })
}

/// Who a fabrication names: the exec of the corp's first rival corp
/// (`news::plant_targets`), living.
pub fn fabricate_target(world: &World, corp: EntityId) -> Option<EntityId> {
    let rival = crate::systems::news::plant_targets(world, corp)
        .into_iter()
        .find(|&t| world.has::<crate::components::Corp>(t))?;
    contracts::head_of(world, rival)
}

/// Daily: each fabricated story of the last 30 days not yet exposed is
/// found out with `expose_p` by the one who knows it false (the actor it
/// names, alive): `Exposed`, a `Betrayed` deed against the corp that paid
/// at full reach, `Crime::Fraud` filed on its exec, and the Feed's reach
/// cut (× 0.8) for 14 days.
pub fn expose(world: &mut World) {
    if !on(world) {
        return;
    }
    let now = world.tick;
    let day = world.day();
    let horizon = now.saturating_sub(30 * TICKS_PER_DAY);
    let p = world.config.leverage.expose_p;
    let cands: Vec<(usize, u32, EntityId, Option<EntityId>, EntityId)> = world
        .stories
        .iter()
        .enumerate()
        .filter(|(_, s)| s.fabricated && !s.exposed && s.tick >= horizon)
        .map(|(i, s)| (i, s.id, s.deed_actor(), s.paid_by, s.feed))
        .collect();
    for (i, sid, actor, paid_by, feed) in cands {
        if !law::living(world, actor) || draw(world, Purpose::Expose, u64::from(sid), day) >= p {
            continue;
        }
        if let Some(s) = world.stories.get_mut(i) {
            s.exposed = true;
        }
        world.stats.current.captive.exposed += 1;
        let Some(corp) = paid_by else { continue };
        let d = world.district_of_building(feed);
        let r0 = world.config.gossip.reach0.get(Deed::Betrayed).max(0.01);
        crate::systems::gossip::post_deed_at(world, d, Deed::Betrayed, Some(corp), Some(actor), 1.0 / r0);
        if let Some(exec) = contracts::head_of(world, corp) {
            law::file_report(world, crate::components::Crime::Fraud, exec, Some(actor));
        }
        world.feed_shamed.insert(feed, now + 14 * TICKS_PER_DAY);
        let text = format!(
            "{} exposed the story {} paid {} to run",
            world.name_of(actor),
            world.owner_label(Some(corp)),
            crate::systems::news::feed_name(world, feed)
        );
        world.push_event(EventKind::Exposed, &[corp, feed], text);
    }
    world.feed_shamed.retain(|_, &mut t| t > now);
}
