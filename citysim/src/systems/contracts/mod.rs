//! M16a § 1-2 (plan C2-C21, C27, C32, C36; phase 1): the contract board.
//!
//! A game abstraction over records: a `Contract` is a record in
//! `World::contracts` (buyer, kind, target, price, deadline, broker,
//! taker, status); a Fixer is a building whose owner keeps a record book
//! (`Broker`); matching is a deterministic score over candidates; a live
//! record is worked by the taker's scripted plan (the Hunt's chase, the
//! ordinary `Attack` roll), a ledger record by one keyed dice draw at
//! `due` (`SimRng::contract`) that opens a pre-bound hole through
//! `lod::stat_hit`; escrow is coins held on the record
//! (`ownership::escrow_in`/`escrow_out`, inside `total_coins`). Nothing
//! here runs without `on(world)`, so a city with `[contracts]` off draws
//! and decides exactly as the L2-closing city.
//!
//! Phase 1 lands the record, posting, escrow, the board and matching for
//! solo agent takers, the Contract goal (work a live record; network at a
//! Fixer), settlement with renege, the ledger, Hunt hiring and the Locate
//! stream. Missions, squads, gang and corp takers are phase 2; the law's
//! view, heat and the brains' postings phase 3; the LOD quotas phase 4.

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, GoalKind, Household, Identity, Job, Lod, Memory, MemoryEntry,
    MemoryKind, Needs, Personality, Position, RelKind, Sentence, Skills, TilePos, Trace,
};
use crate::contract::{
    Broker, Contract, ContractId, ContractKind, ContractNs, ContractRun, ContractStatus, Motive, Origin, PostExtra,
    Posting, Refusal, Render, Target, Terms,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{FailReason, StepResult};
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::rng::splitmix64;
use crate::systems::missions;
use crate::systems::ownership::{self, Flow};
use crate::time::{self, Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, urgency, Curve, GATE};
use crate::utility::Consideration;
use crate::word::{Deed, HuntPhase, Intel, IntelSource};
use crate::world::World;

mod desk;
mod fixers;
mod heat;
mod ledger;
mod locate;
mod offers;
mod plan;
mod post;
mod price;
mod settle;
mod squads;
mod take;
mod word;

pub use desk::*;
pub use fixers::*;
pub use heat::*;
pub use ledger::*;
pub use locate::*;
pub use offers::*;
pub use plan::*;
pub use post::*;
pub use price::*;
pub use settle::*;
pub use squads::*;
pub use take::*;
pub use word::*;

/// C10: the Statistical regular day's hash salt ("REGULAR!").
const REGULAR_SALT: u64 = 0x5245_4755_4C41_5221;
/// C20: the hire pass day's hash salt ("HIREPASS").
const HIRE_SALT: u64 = 0x4849_5245_5041_5353;
/// C2: `World::contract_log`'s cap.
const LOG_CAP: usize = 512;

/// How a live record closes (`settle`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Settle {
    Fulfilled,
    Failed,
    Expired,
    Cancelled,
}

// ---------------------------------------------------------------------------
// Indices
// ---------------------------------------------------------------------------

fn parties(c: &Contract) -> SmallVec<[EntityId; 8]> {
    let mut v: SmallVec<[EntityId; 8]> = SmallVec::new();
    let mut add = |e: Option<EntityId>| {
        if let Some(e) = e {
            if !v.contains(&e) {
                v.push(e);
            }
        }
    };
    add(c.buyer);
    add(c.agent);
    add(c.taker);
    for &m in &c.crew {
        add(Some(m));
    }
    add(Some(c.target.id()));
    v
}

fn index_parties(world: &mut World, id: ContractId) {
    let Some(c) = world.contracts.get(&id) else { return };
    for p in parties(c) {
        // OPEN-112: kept ascending, as the load's rebuild (over the
        // records in id order) builds it: a party joining a record later
        // must not reorder what a live world and a loaded one walk.
        let list = world.by_party.entry(p).or_default();
        if let Err(i) = list.binary_search(&id) {
            list.insert(i, id);
        }
    }
}

fn unindex(world: &mut World, id: ContractId) {
    let Some(c) = world.contracts.get(&id) else { return };
    for p in parties(c) {
        if let Some(list) = world.by_party.get_mut(&p) {
            list.retain(|x| *x != id);
            if list.is_empty() {
                world.by_party.remove(&p);
            }
        }
    }
}

/// C2: every `#[serde(skip)]` index from the saved records (after a load;
/// `World::rebuild_indices`).
pub fn rebuild_index(world: &mut World) {
    world.by_party.clear();
    world.ledger_due.clear();
    world.mission_of.clear();
    world.locate_targets.clear();
    world.on_take.clear();
    world.contract_guards.clear();
    let ids: Vec<ContractId> = world.contracts.keys().copied().collect();
    for id in ids {
        index_parties(world, id);
        let Some(c) = world.contracts.get(&id) else { continue };
        if c.status == ContractStatus::Taken && c.render == Render::Ledger {
            if let Some(d) = c.due {
                world.ledger_due.insert((d, id));
            }
        }
        if c.kind == ContractKind::Locate && c.is_live() {
            if let Target::Agent(t) = c.target {
                world.locate_targets.entry(t).or_default().push(id);
            }
        }
    }
    for (&m, mission) in &world.missions {
        for &a in &mission.crew {
            world.mission_of.insert(a, m);
        }
    }
    // C32: the bodies standing a Guard post now.
    let posts: Vec<(EntityId, EntityId)> = world
        .contract_runs
        .iter()
        .filter(|(&t, _)| {
            world
                .comp::<Brain>(t)
                .is_some_and(|b| matches!(b.exec, crate::exec::ExecState::Use { kind: ActionKind::Guard, .. }))
        })
        .map(|(&t, r)| (r.target, t))
        .collect();
    for (client, guard) in posts {
        world.contract_guards.entry(client).or_default().push(guard);
    }
    // C31: the guards on the take.
    reindex_take(world);
    // M16b phase 3: the Trauma clients.
    crate::systems::wounded::reindex(world);
}

/// M16b phase 3 (spec § 5 "Trauma Team"): a standing Trauma subscription
/// of `client` with the Security corp `corp`, paid by `buyer` (the client,
/// or its corp for an exec): `Standing` (never matched, never on the
/// ledger), its first week charged now (`last_paid`). `None` when the
/// payer is short.
pub fn post_standing(
    world: &mut World,
    buyer: EntityId,
    client: EntityId,
    corp: EntityId,
    fee: i64,
) -> Option<ContractId> {
    if world.purse(Some(buyer)) < fee {
        return None;
    }
    let now = world.tick;
    let id = world.next_contract.max(1);
    world.next_contract = id + 1;
    let mut rec = record_of(
        id,
        Some(buyer),
        Some(client),
        ContractKind::Trauma,
        Target::Agent(client),
        None,
        Origin::Subscription,
        now,
        fee,
    );
    rec.status = ContractStatus::Standing;
    rec.taker = Some(corp);
    rec.taken = Some(now);
    rec.deadline = Tick::MAX / 2;
    rec.last_paid = Some(now);
    world.contracts.insert(id, rec);
    index_parties(world, id);
    ownership::pay(world, Some(buyer), Some(corp), fee, Flow::Trauma);
    world.stats.current.contract.contracts_posted += 1;
    note_kind_posted(world, ContractKind::Trauma);
    let text = format!(
        "{} subscribed {} to {}'s Trauma Team for {fee} a week",
        world.owner_label(Some(buyer)),
        world.name_of(client),
        world.owner_label(Some(corp))
    );
    log(world, id, text);
    Some(id)
}

fn log(world: &mut World, id: ContractId, text: String) {
    let now = world.tick;
    world.contract_log.push_back((now, id, text));
    while world.contract_log.len() > LOG_CAP {
        world.contract_log.pop_front();
    }
}

/// A line in `World::contract_log` (the Board's history; phase 2's brawl
/// and strike lines).
pub fn note(world: &mut World, id: ContractId, text: String) {
    log(world, id, text);
}

// ---------------------------------------------------------------------------
// The schedule (C36)
// ---------------------------------------------------------------------------

/// C36: the tick slot after `living`: the ledger's due records, the live
/// runs' validity (a dead, jailed or cuffed taker), the queued live
/// records hourly, the matching at `match_hour`, the midnight pass.
pub fn run(world: &mut World) {
    let now = world.tick;
    while let Some(&(t, id)) = world.ledger_due.first() {
        if t > now {
            break;
        }
        world.ledger_due.remove(&(t, id));
        resolve_ledger(world, id);
    }
    if !world.contract_runs.is_empty() {
        // scan-ok: the live runs only (≤ `max_missions`).
        let stale: Vec<(EntityId, ContractId)> =
            world.contract_runs.iter().filter(|(&t, _)| !free_adult(world, t)).map(|(&t, r)| (t, r.contract)).collect();
        for (_, id) in stale {
            fail_attempt(world, id, "the taker was taken off the street");
        }
    }
    // Phase 2 (C22): the missions' crews and march windows.
    missions::check(world);
    let tod = time::tick_of_day(now);
    if u64::from(tod) % TICKS_PER_HOUR == 0 {
        start_queued(world);
    }
    if u64::from(tod) == u64::from(world.config.fixers.match_hour) * TICKS_PER_HOUR {
        match_day(world);
    }
    if crate::systems::midnight::due(world, crate::systems::midnight::Mid::Tail) {
        daily(world);
    }
}

/// C17: queued live records start, by price (ties the lower id), while
/// runs are under `max_missions`.
fn start_queued(world: &mut World) {
    ledger_stale_queue(world);
    stalled_runs(world);
    let mut queued: Vec<(i64, ContractId)> = world
        .contracts
        .values()
        .filter(|c| c.status == ContractStatus::Taken && c.render == Render::Live)
        .filter(|c| !running(world, c))
        .map(|c| (-c.price, c.id))
        .collect();
    if queued.is_empty() {
        return;
    }
    queued.sort();
    // Phase 4 review: a record that cannot start (its bodies over the
    // quota, a lost taker or crew) is passed over, not a wall for the
    // cheaper ones behind it; only the run count stops the pass (the Hit
    // market's stale records with a body on them excepted: `jumps_queue`).
    for (_, id) in queued {
        let full = live_count(world) >= world.config.missions.max_missions;
        if full && !world.contracts.get(&id).is_some_and(|c| jumps_queue(world, c)) {
            continue;
        }
        start_live(world, id);
    }
}

/// The Hit market's gaps pass (fix round): a solo taker's organic Hit or
/// Beat queued `queue_hours` or more that may not go to the ledger
/// (`ledger_ok`: its target a body, or a party in front of the player)
/// starts live past the `max_missions` run count; the bodies'
/// `contract_quota` still holds, and `[hitmarket] max_open` bounds how many
/// such records exist at once. A gang's or a squad's job does not jump (on
/// 42-47 letting the missions jump too raised Murders 509 → 632: door
/// brawls); it waits for a slot or for its party to leave the screen.
fn jumps_queue(world: &World, c: &Contract) -> bool {
    world.config.hitmarket.enabled
        && c.status == ContractStatus::Taken
        && !c.motive.is_none()
        && c.kind.violent()
        && !is_mission(world, c)
        && c.taken.is_some_and(|t| {
            world.tick.saturating_sub(t) >= u64::from(world.config.hitmarket.queue_hours) * TICKS_PER_HOUR
        })
        && !ledger_ok(world, c)
}

/// The Hit market's gaps pass (takers who can't close), hourly before the
/// queue starts: a taken organic Hit or Beat still waiting for a live slot
/// `queue_hours` after the take (the `max_missions` slots held by
/// week-long Locate runs and Guards: 42-47 had records taken on day one
/// and queued to their deadline) is worked on the ledger instead: a `due`
/// drawn over half the time left (`ContractNs::Due`, key `1 + attempts`;
/// the take's draw is key 0), the strike rolled by `resolve_ledger` (a
/// gang's or a squad's crew strikes as one party).
fn ledger_stale_queue(world: &mut World) {
    if !world.config.hitmarket.enabled {
        return;
    }
    let now = world.tick;
    let wait = u64::from(world.config.hitmarket.queue_hours) * TICKS_PER_HOUR;
    let stale: Vec<ContractId> = world
        .contracts
        .values()
        .filter(|c| c.status == ContractStatus::Taken && c.render == Render::Live)
        .filter(|c| !c.motive.is_none() && c.kind.violent() && c.target_agent().is_some())
        .filter(|c| c.taken.is_some_and(|t| now.saturating_sub(t) >= wait))
        .filter(|c| !running(world, c))
        // Fix round: a body for a target, or a party in front of the
        // player, keeps it live in the queue (`ledger_ok`).
        .filter(|c| ledger_ok(world, c))
        .map(|c| c.id)
        .collect();
    for id in stale {
        let Some((deadline, attempts)) = world.contracts.get(&id).map(|c| (c.deadline, c.attempts)) else { continue };
        let span = (deadline.saturating_sub(now) / 2).max(60);
        let u: f64 = world.rng.contract(ContractNs::Due, id, 1 + u64::from(attempts)).random();
        let due = now + (u * span as f64).floor() as Tick;
        if let Some(c) = world.contracts.get_mut(&id) {
            c.render = Render::Ledger;
            c.due = Some(due);
        }
        world.ledger_due.insert((due, id));
        let hours = world.config.hitmarket.queue_hours;
        log(world, id, format!("no live slot in {hours} h: worked off-screen"));
    }
}

/// Round 8 X2 (`OPEN-247`), hourly: a solo taker's live Hit or Beat run
/// with no stakeout at the mark's door in `[hitmarket] stall_hours` (since
/// the take, or since the last stakeout) and not under way now is let go:
/// an attempt failed with the line (how often he set out, the post he kept
/// to), the record back on the board. 20k seed 42: a hit taken on d1 by a
/// gun hired as Club Staff at the next midnight was planned daily, cut by
/// her shift and her sleep every time, and sat unsaid until its buyer was
/// gone on d9.
fn stalled_runs(world: &mut World) {
    let hours = world.config.hitmarket.stall_hours;
    if hours == 0 || world.contract_runs.is_empty() {
        return;
    }
    let now = world.tick;
    let wait = u64::from(hours) * TICKS_PER_HOUR;
    // scan-ok: the live runs only (≤ `max_missions`).
    let stale: Vec<(EntityId, ContractId)> = world
        .contract_runs
        .iter()
        .filter(|(_, r)| now.saturating_sub(r.since.max(r.worked)) >= wait)
        .filter(|(&t, _)| world.comp::<Brain>(t).and_then(|b| b.plan_goal()) != Some(GoalKind::Contract))
        .filter(|(_, r)| {
            world.contracts.get(&r.contract).is_some_and(|c| c.kind.violent() && c.status == ContractStatus::Taken)
        })
        .map(|(&t, r)| (t, r.contract))
        .collect();
    for (t, id) in stale {
        let who = world.name_of(t);
        let tries = world.contract_runs.get(&t).map_or(0, |r| r.tries);
        let mark = world.contracts.get(&id).map(|c| world.name_of(c.target.id())).unwrap_or_default();
        let went = match tries {
            0 => "never set out".to_string(),
            1 => format!("set out once and never stood at {mark}'s door"),
            n => format!("set out {n} times and never stood at {mark}'s door"),
        };
        let post = world
            .comp::<Job>(t)
            .map(|j| format!(", kept to {} post as {}", crate::systems::grief::his(world, t), j.role.label()))
            .unwrap_or_default();
        fail_attempt(world, id, &format!("{who} let it go: {went} in {hours} h{post}"));
    }
}

/// C17: `lod::set_lod`'s promotion branch: a ledger record whose taker or
/// target became a body before `due` turns live (room permitting).
pub fn on_promoted(world: &mut World, id: EntityId) {
    if world.ledger_due.is_empty() {
        return;
    }
    let Some(ids) = world.by_party.get(&id).cloned() else { return };
    for cid in ids {
        let Some(c) = world.contracts.get(&cid) else { continue };
        if c.status != ContractStatus::Taken || c.render != Render::Ledger {
            continue;
        }
        let Some(due) = c.due else { continue };
        if c.taker != Some(id) && c.target.id() != id {
            continue;
        }
        // Round 4 T3 (OPEN-222): a ledger kind stays on the ledger.
        if ledger_kind(world, c) {
            continue;
        }
        // C35 (phase 4): only into a set with room (else it stays a ledger record).
        if live_count(world) >= world.config.missions.max_missions {
            return;
        }
        if !has_room(world, c) {
            continue;
        }
        world.ledger_due.remove(&(due, cid));
        if let Some(x) = world.contracts.get_mut(&cid) {
            x.render = Render::Live;
            x.due = None;
        }
        start_live(world, cid);
    }
}

/// The median `Job` wage now (0 with nobody employed).
fn median_wage(world: &World) -> i64 {
    // scan-ok: daily: the job holders.
    let mut w: Vec<i64> =
        world.with::<Job>().iter().filter_map(|&a| world.comp::<Job>(a)).map(|j| j.wage_per_day).collect();
    if w.is_empty() {
        return 0;
    }
    w.sort_unstable();
    w[w.len() / 2]
}

/// C36 at midnight: deferred seeding, the median wage, deadlines and
/// buyers gone, regulars pruned and the Statistical regular pass, the hire
/// pass, the Fixers' income roll, closed records dropped.
pub fn daily(world: &mut World) {
    if world.fixers_due && !seed_fixers(world).is_empty() {
        world.fixers_due = false;
        crate::systems::virt::relink(world);
    }
    world.median_wage = median_wage(world);
    let now = world.tick;
    let live: Vec<ContractId> = world.contracts.values().filter(|c| c.is_live()).map(|c| c.id).collect();
    for id in live {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        if c.buyer.is_some() && !buyer_ok(world, c.buyer) {
            settle(world, id, Settle::Cancelled, "the buyer is gone");
            continue;
        }
        let gang_gone =
            c.buyer.and_then(|b| world.comp::<crate::components::Gang>(b)).is_some_and(|g| g.members.is_empty());
        if gang_gone {
            settle(world, id, Settle::Cancelled, "the buyer is gone");
            continue;
        }
        if c.deadline > now {
            continue;
        }
        match (c.kind, c.status) {
            // Phase 4 review: a Guard still queued at its deadline never
            // stood its post: expired (the buyer refunded, the taker unpaid).
            (ContractKind::Guard | ContractKind::Escort, ContractStatus::Taken) if c.started.is_none() => {
                settle(world, id, Settle::Expired, "the post was never stood")
            }
            (ContractKind::Guard | ContractKind::Escort, ContractStatus::Taken) => {
                if guard_kept(world, &c) {
                    settle(world, id, Settle::Fulfilled, "");
                } else {
                    settle(world, id, Settle::Failed, "the client was hit");
                }
            }
            (ContractKind::Locate, _) if c.paid_sightings > 0 => settle(world, id, Settle::Fulfilled, ""),
            _ => settle(world, id, Settle::Expired, "the deadline passed"),
        }
    }
    // Round 6 V3 (`OPEN-238`): the law's bust before the night's posting
    // passes (hire, the Hit market's), so a busted office takes no post in
    // the minute it closes (V14 `fixer_40483 d26 00:09`: a beat posted on
    // Wisp Varley's book and cancelled in the same minute).
    heat_daily(world);
    prune_regulars(world);
    stat_regulars(world);
    // The Hit market (phase 2): stale wants lapse first (H8); the gaps
    // pass: the stiffed guns' grace (decision 3).
    crate::systems::hitmarket::want_sweep(world);
    crate::systems::hitmarket::stiff_sweep(world);
    hire_pass(world);
    // The Hit market: Silence and Debt (phase 2), the gangs' and corps'
    // business (H18, H19, Protect), and the cold cases (H14).
    crate::systems::hitmarket::agent_passes(world);
    crate::systems::hitmarket::gang_pass(world);
    crate::systems::hitmarket::corp_pass(world);
    crate::systems::hitmarket::cold_cases(world);
    // Phase 3 (C30, C31, C33, C34).
    law_and_street_daily(world);
    for f in world.buildings_of_kind(BuildingKind::Fixer).to_vec() {
        if let Some(k) = world.comp_mut::<Broker>(f) {
            let today = k.income_today;
            k.income.push_back(today);
            while k.income.len() > 14 {
                k.income.pop_front();
            }
            k.income_today = 0;
        }
    }
    drop_closed(world);
}

/// C32: a Guard's client is safe at the deadline: an agent alive, a
/// building with no corp loss logged since the taking.
fn guard_kept(world: &World, c: &Contract) -> bool {
    // Phase 4 review: a post never stood is never kept.
    if c.started.is_none() {
        return false;
    }
    match c.target {
        Target::Agent(a) => crate::systems::law::living(world, a),
        Target::Carry { .. } | Target::Story { .. } => false,
        Target::Building(b) => {
            let since = c.guard_since.unwrap_or(c.posted);
            world
                .corp_of_building(b)
                .and_then(|corp| world.comp::<crate::components::Corp>(corp))
                .is_none_or(|corp| !corp.loss_log.iter().any(|l| l.building == Some(b) && l.tick >= since))
        }
    }
}

/// C10: stamps older than `regular_days` go.
fn prune_regulars(world: &mut World) {
    let keep = Tick::from(world.config.fixers.regular_days) * TICKS_PER_DAY;
    let now = world.tick;
    for f in world.buildings_of_kind(BuildingKind::Fixer).to_vec() {
        if let Some(k) = world.comp_mut::<Broker>(f) {
            k.regulars.retain(|_, &mut t| t + keep > now);
        }
    }
}

/// C10: Statistical adults through the gun gate whose Trace yesterday put
/// them in an open Fixer's district become its regulars on one hash-picked
/// day in seven (`REGULAR_SALT`).
fn stat_regulars(world: &mut World) {
    let fixers = open_fixers(world);
    if fixers.is_empty() {
        return;
    }
    let day = world.day();
    let seed = world.seed();
    let now = world.tick;
    let yesterday = day.saturating_sub(1);
    let by_district: Vec<(usize, EntityId)> =
        fixers.iter().map(|&f| (world.district_of_building(f).index(), f)).collect();
    let mut stamps: Vec<(EntityId, EntityId)> = Vec::new();
    // scan-ok: daily, the Statistical tier.
    for &id in world.tier(Lod::Statistical) {
        if !splitmix64(seed ^ u64::from(id.index) ^ day ^ REGULAR_SALT).is_multiple_of(7) {
            continue;
        }
        let Some(d) = world.comp::<Trace>(id).and_then(|t| t.on_day(yesterday)).map(|t| t.district) else { continue };
        if d.is_unset() {
            continue;
        }
        let Some(&(_, f)) = by_district.iter().find(|&&(fd, _)| fd == d.index()) else { continue };
        if gun_gate(world, id) {
            stamps.push((id, f));
        }
    }
    for (id, f) in stamps {
        let gang = world.gang_of(id);
        if let Some(k) = world.comp_mut::<Broker>(f) {
            k.regulars.insert(id, now);
            if let Some(g) = gang {
                k.gangs.insert(g);
            }
        }
    }
}

/// C2: closed records drop `closed_keep_days` after closing; a fulfilled
/// Hit or Beat is kept `max(closed_keep_days, accessory_days)`.
fn drop_closed(world: &mut World) {
    let now = world.tick;
    let keep = u64::from(world.config.contracts.closed_keep_days) * TICKS_PER_DAY;
    let acc = keep.max(u64::from(world.config.law.accessory_days) * TICKS_PER_DAY);
    let old: Vec<ContractId> = world
        .contracts
        .values()
        .filter(|c| !c.is_live())
        .filter(|c| {
            let k = if c.kind.violent() && matches!(c.status, ContractStatus::Fulfilled | ContractStatus::Reneged) {
                acc
            } else {
                keep
            };
            c.closed.is_some_and(|t| t + k <= now)
        })
        .map(|c| c.id)
        .collect();
    for id in old {
        unindex(world, id);
        world.contracts.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ask by the pre-review bisection: a fresh mode-A list per step.
    fn ask_reference(
        world: &World,
        gun: EntityId,
        kind: ContractKind,
        brokered: bool,
        buyer: Option<EntityId>,
        s_def: f32,
    ) -> Option<i64> {
        let min = world.config.contracts.accept_min;
        let risk = risk_against(world, kind, gun, s_def);
        let score = |price: i64| -> f32 {
            mode_a_at(world, gun, kind, brokered, buyer, price, risk, 1.0).iter().map(|x| x.output).product()
        };
        if score(ASK_CAP) < min {
            return None;
        }
        if score(1) >= min {
            return Some(1);
        }
        let (mut lo, mut hi) = (1i64, ASK_CAP);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if score(mid) >= min {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Some(hi)
    }

    /// Review fix (perf): the hoisted ask equals the old bisection's on a
    /// spread of candidates, kinds, broker modes and defender strengths.
    #[test]
    fn test_hoisted_ask_equals_the_bisection() {
        let w = World::new(42, crate::config::Config::load());
        // scan-ok: a unit test, once.
        let people: Vec<EntityId> = w.citizens().into_iter().filter(|&a| crate::systems::law::living(&w, a)).collect();
        let buyer = people.first().copied();
        let (mut some, mut none) = (0, 0);
        for &g in people.iter().step_by(5) {
            for kind in [ContractKind::Hit, ContractKind::Beat, ContractKind::Locate] {
                for brokered in [true, false] {
                    for s_def in [0.1, 0.625, 1.5, 4.0] {
                        let new = ask_against(&w, g, kind, brokered, buyer, s_def);
                        assert_eq!(new, ask_reference(&w, g, kind, brokered, buyer, s_def), "{g:?} {kind:?}");
                        if new.is_some() {
                            some += 1;
                        } else {
                            none += 1;
                        }
                    }
                }
            }
        }
        assert!(some > 100 && none > 100, "a spread: {some} priced, {none} not for hire");
    }
}
