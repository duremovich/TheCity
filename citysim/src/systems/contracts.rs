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
    Broker, Contract, ContractId, ContractKind, ContractNs, ContractRun, ContractStatus, Origin, Posting, Refusal,
    Render, Target, Terms,
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
        let list = world.by_party.entry(p).or_default();
        if !list.contains(&id) {
            list.push(id);
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
// Fixers
// ---------------------------------------------------------------------------

/// An open Fixer's office: standing, a `Broker`, not closed by the law.
pub fn fixer_open(world: &World, b: EntityId) -> bool {
    let now = world.tick;
    world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Fixer && !bd.demolished && !bd.derelict)
        && world.comp::<Broker>(b).is_some_and(|k| k.closed_until.is_none_or(|t| t <= now))
}

/// Every open Fixer, ascending.
pub fn open_fixers(world: &World) -> Vec<EntityId> {
    world.buildings_of_kind(BuildingKind::Fixer).iter().copied().filter(|&b| fixer_open(world, b)).collect()
}

/// A home door, else where the agent stands.
fn home_door(world: &World, id: EntityId) -> Option<TilePos> {
    world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
}

/// C20: the open Fixer whose door is nearest `id`'s Home door within
/// `hire_reach_tiles` (Manhattan; ties the lower id).
pub fn nearest_fixer(world: &World, id: EntityId) -> Option<EntityId> {
    let from = home_door(world, id)?;
    let reach = world.config.fixers.hire_reach_tiles;
    world
        .buildings_of_kind(BuildingKind::Fixer)
        .iter()
        .copied()
        .filter(|&b| fixer_open(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, b)| b)
}

/// The open Fixer nearest a tile (any distance; ties the lower id).
fn nearest_fixer_to(world: &World, from: TilePos) -> Option<EntityId> {
    open_fixers(world)
        .into_iter()
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// Is `id` a regular of any open Fixer whose stamp holds past `now + slack`?
fn is_regular(world: &World, id: EntityId, slack: Tick) -> bool {
    let keep = Tick::from(world.config.fixers.regular_days) * TICKS_PER_DAY;
    let now = world.tick;
    world.buildings_of_kind(BuildingKind::Fixer).iter().any(|&f| {
        fixer_open(world, f)
            && world.comp::<Broker>(f).and_then(|k| k.regulars.get(&id)).is_some_and(|&t| t + keep > now + slack)
    })
}

/// C9: the districts that get a seeded Fixer, in order.
pub const SEEDED_FIXERS: [&str; 1] = ["Mid East"];

/// C9 (deviation, coordinator 2026-10-08): one seeded Fixer, Mid East's,
/// on the Lots left after L2's venues (no RNG); the second is founded by an
/// NPC through `Register` under the per-capita cap (`fixers_per_pop`), so
/// Sump Central keeps its one vacant Lot for M12 splits and Registers.
/// Per district, the vacant Lot whose door
/// is nearest the district's centroid (ties the lower id), else one in an
/// adjacent district (ascending index), else a derelict refitted free
/// (the district, then the adjacent ones), else skipped and logged. The
/// owner is the jobless adult who is no exec, no gang leader and owns no
/// building, lowest lawfulness first (ties: Home door nearest the Lot's,
/// then the lower id). No coin is minted.
pub fn seed_fixers(world: &mut World) -> Vec<EntityId> {
    let mut built = Vec::new();
    for name in SEEDED_FIXERS {
        let Some(d) = world.districts.iter().position(|x| x.name == name) else {
            log(world, 0, format!("no {name} district: Fixer not seeded"));
            continue;
        };
        let adjacent: Vec<usize> = {
            let mask = world.district_adjacent.get(d).copied().unwrap_or(0);
            (0..world.districts.len()).filter(|&i| i != d && mask & (1 << i) != 0).collect()
        };
        let order: Vec<usize> = std::iter::once(d).chain(adjacent).collect();
        let centroid = world.districts[d].centroid;
        let lot = order.iter().find_map(|&di| {
            crate::systems::founding::vacant_lots(world)
                .into_iter()
                .filter(|&l| world.district_of_building(l).index() == di)
                .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(centroid), l)))
                .min()
                .map(|(_, l)| l)
        });
        let derelict = || {
            order.iter().find_map(|&di| {
                crate::systems::street::derelicts(world)
                    .into_iter()
                    .filter(|&x| world.district_of_building(x).index() == di)
                    .filter_map(|x| {
                        let bd = world.comp::<Building>(x).filter(|bd| bd.kind == BuildingKind::Home)?;
                        Some((bd.door.manhattan(centroid), x))
                    })
                    .min()
                    .map(|(_, x)| x)
            })
        };
        let site = match lot {
            Some(l) => Some((l, true)),
            None => derelict().map(|x| (x, false)),
        };
        let Some((site, on_lot)) = site else {
            log(world, 0, format!("no Lot or derelict in or beside {name}: Fixer not seeded"));
            continue;
        };
        let door = world.comp::<Building>(site).map(|b| b.door).unwrap_or_default();
        let owner = seed_owner(world, door);
        let made = if on_lot {
            crate::systems::founding::build_on_lot(world, site, BuildingKind::Fixer, owner)
        } else {
            crate::systems::founding::refit_with(world, site, BuildingKind::Fixer, owner, false)
        };
        if made.is_err() {
            log(world, 0, format!("{name}: the Fixer could not be built"));
            continue;
        }
        let at = world.district_name(world.district_of_building(site)).to_string();
        let who = world.owner_label(owner);
        let how = if on_lot { "Lot" } else { "refit" };
        let text = format!("{who} opened a Fixer in {at} (seeded; {how} {})", site.index);
        let mut actors: Vec<EntityId> = owner.into_iter().collect();
        actors.push(site);
        world.push_event(EventKind::Founded, &actors, text.clone());
        log(world, 0, text);
        built.push(site);
    }
    world.fixers_seeded = !built.is_empty();
    built
}

/// C9: a seeded Fixer's owner.
fn seed_owner(world: &World, door: TilePos) -> Option<EntityId> {
    let owners: std::collections::BTreeSet<EntityId> =
        crate::systems::ownership::holdings(world).keys().flatten().copied().collect();
    let leaders: std::collections::BTreeSet<EntityId> =
        world.gangs().iter().filter_map(|&g| world.comp::<crate::components::Gang>(g).and_then(|x| x.leader)).collect();
    world
        // scan-ok: twice at seed (and at a deferred seeding).
        .citizens()
        .into_iter()
        .filter(|&a| crate::systems::demography::is_adult(world, a) && world.has::<Brain>(a))
        .filter(|&a| !world.has::<Job>(a) && !world.has::<Sentence>(a))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| !b.emigrating))
        .filter(|&a| !owners.contains(&a) && !leaders.contains(&a))
        .filter(|&a| !crate::systems::founding::is_exec(world, a))
        .filter_map(|a| {
            let l = world.comp::<Personality>(a)?.lawfulness;
            let dist = home_door(world, a).map_or(u32::MAX, |t| t.manhattan(door));
            Some((ordered_float::OrderedFloat(l), dist, a))
        })
        .min()
        .map(|(_, _, a)| a)
}

// ---------------------------------------------------------------------------
// Price, posting, visibility
// ---------------------------------------------------------------------------

/// The risk a record carries: `1 − estimate(&[MEDIAN], target)` for the
/// violent kinds, 0.1 for Guard and Locate (C7).
pub fn risk_of(world: &World, kind: ContractKind, target: &Target) -> f32 {
    if kind.violent() {
        1.0 - missions::estimate(world, &[missions::MEDIAN], target)
    } else {
        0.1
    }
}

/// The standing the price reads: the target's, a building's owner's (0.3
/// for the city).
fn standing_of(world: &World, target: &Target) -> f32 {
    match *target {
        Target::Agent(t) => crate::systems::reputation::rep(world, t).standing,
        Target::Building(b) => match world.owner_of(b) {
            Some(o) => crate::systems::reputation::rep(world, o).standing,
            None => 0.3,
        },
    }
}

/// C7: `price_base[kind] × (1 + risk_w × risk) × (1 + standing_w ×
/// standing) × price_level_b` (1.0 for every broker), rounded.
pub fn quote(world: &World, kind: ContractKind, target: &Target, broker: Option<EntityId>) -> i64 {
    let _ = broker;
    let c = &world.config.contracts;
    let base = c.price_base.get(kind) as f32;
    let risk = risk_of(world, kind, target);
    let standing = standing_of(world, target);
    let level = 1.0;
    (base * (1.0 + c.risk_w * risk) * (1.0 + c.standing_w * standing) * level).round().max(1.0) as i64
}

fn target_ok(world: &World, target: &Target) -> bool {
    match *target {
        Target::Agent(t) => world.has::<Identity>(t) && crate::systems::law::living(world, t) && world.has::<Brain>(t),
        Target::Building(b) => world.comp::<Building>(b).is_some_and(|bd| !bd.demolished),
    }
}

/// A buyer that can still hold coins: the city, a corp, a gang or a living agent.
fn buyer_ok(world: &World, buyer: Option<EntityId>) -> bool {
    match buyer {
        None => true,
        Some(b) => {
            world.has::<crate::components::Corp>(b)
                || world.has::<crate::components::Gang>(b)
                || world.has::<crate::components::Wallet>(b)
        }
    }
}

/// The renege draw (C6): direct records placed by an agent only.
fn renege_draw(world: &World, id: ContractId, agent: Option<EntityId>) -> bool {
    let Some(a) = agent else { return false };
    let law = world.comp::<Personality>(a).map_or(0.5, |p| p.lawfulness);
    let honour = crate::systems::reputation::rep(world, a).honour;
    let p = world.config.contracts.renege_base * (1.0 - law) * (1.0 - honour);
    let mut rng = world.rng.contract(ContractNs::Renege, id, 0);
    rng.random::<f32>() < p
}

/// C21: a first-hand `Hired` rumour (actor the buyer, object the target)
/// in `holder`, who joins `known_by`.
fn hear_hired(world: &mut World, id: ContractId, holder: EntityId) {
    let Some(c) = world.contracts.get(&id) else { return };
    if c.kind == ContractKind::Guard {
        return;
    }
    let (actor, object) = (c.buyer.or(c.agent), c.target.id());
    if let Some(c) = world.contracts.get_mut(&id) {
        if !c.known_by.contains(&holder) {
            c.known_by.push(holder);
        }
    }
    if !world.has::<Memory>(holder) {
        return;
    }
    let sal = world.config.gossip.deed_sal.get(Deed::Hired);
    let e = MemoryEntry {
        subject: actor,
        salience: sal,
        valence: -world.config.gossip.deed_sev.get(Deed::Hired) * sal,
        deed: Some(Deed::Hired),
        object: Some(object),
        hops: 0,
        conf: 1.0,
        ..MemoryEntry::blank(MemoryKind::Rumour, world.tick)
    };
    crate::systems::memory::hear_entry(world, holder, e);
}

/// The one entry point (C1, spec § 1): brains, agents, god commands and
/// (M18) the player post through it. Checks, the price (quoted unless a
/// god's override), escrow for a brokered record (a direct one checks the
/// purse only), the renege draw, the `Hired` first-hand writes, the
/// Fixer's book, the log and `ContractPosted`.
pub fn post(world: &mut World, p: Posting) -> Result<ContractId, Refusal> {
    let live = world.contracts.values().filter(|c| c.is_live()).count();
    if live >= world.config.contracts.max_open {
        return Err(Refusal::MaxOpen);
    }
    // A Guard may be bought on oneself (C31); any other kind may not.
    let on_self = Some(p.target.id()) == p.buyer && p.kind != ContractKind::Guard;
    // Review fix: only a Guard may name a building (a Hit, Beat or Locate
    // on one has no plan and no ledger row: it would sit Taken to its deadline).
    let building = matches!(p.target, Target::Building(_)) && p.kind != ContractKind::Guard;
    if !target_ok(world, &p.target) || on_self || building || !buyer_ok(world, p.buyer) {
        return Err(Refusal::NoTarget);
    }
    if world.contracts.values().any(|c| c.is_live() && c.buyer == p.buyer && c.kind == p.kind && c.target == p.target) {
        return Err(Refusal::Duplicate);
    }
    if let Some(b) = p.broker {
        if !fixer_open(world, b) {
            return Err(Refusal::BrokerClosed);
        }
    }
    let price = p.price.unwrap_or_else(|| quote(world, p.kind, &p.target, p.broker)).max(1);
    if world.purse(p.buyer) < price {
        return Err(Refusal::CannotPay);
    }
    let now = world.tick;
    let id = world.next_contract.max(1);
    world.next_contract = id + 1;
    let renege = p.broker.is_none() && renege_draw(world, id, p.agent);
    let deadline = now + Tick::from(p.deadline_days.max(1)) * TICKS_PER_DAY;
    let risk = risk_of(world, p.kind, &p.target);
    world.contracts.insert(
        id,
        Contract {
            id,
            buyer: p.buyer,
            agent: p.agent,
            kind: p.kind,
            target: p.target,
            terms: Terms::Pay { price },
            escrow: 0,
            broker: p.broker,
            posted: now,
            deadline,
            origin: p.origin,
            status: ContractStatus::Open,
            taker: None,
            crew: SmallVec::new(),
            attempts: 0,
            risk,
            political: 0.0,
            renege,
            due: None,
            render: Render::Ledger,
            known_by: SmallVec::new(),
            price,
            taken: None,
            closed: None,
            paid_sightings: 0,
            last_paid: None,
            strike: None,
            holds: 0,
            sold_out: false,
            charged: false,
            interrogated: SmallVec::new(),
            guard_since: None,
        },
    );
    if p.broker.is_some() {
        let moved = ownership::escrow_in(world, p.buyer, id, price);
        debug_assert_eq!(moved, price, "the purse was checked");
    }
    index_parties(world, id);
    if let Some(b) = p.broker {
        if let Some(k) = world.comp_mut::<Broker>(b) {
            k.book.push(id);
        }
    }
    if p.kind == ContractKind::Locate {
        if let Target::Agent(t) = p.target {
            world.locate_targets.entry(t).or_default().push(id);
        }
    }
    // C21: first-hand into the placing agent and, brokered, the Fixer's owner.
    if let Some(a) = p.agent {
        hear_hired(world, id, a);
    }
    if let Some(owner) = p.broker.and_then(|b| world.owner_of(b)) {
        if world.has::<Identity>(owner) {
            hear_hired(world, id, owner);
        }
    }
    let row = &mut world.stats.current.contract;
    row.contracts_posted += 1;
    row.k_posted[p.kind.index()] += 1;
    let who = world.owner_label(p.buyer);
    let on = world.name_of(p.target.id());
    let at = p.broker.map(|b| format!(" at {}", world.name_of(b))).unwrap_or_default();
    let text = format!("{who} posted a {} on {on} for {price}{at}", p.kind.label());
    let actors = [p.agent.unwrap_or(EntityId::NONE), p.target.id(), p.broker.unwrap_or(EntityId::NONE)];
    world.push_event(EventKind::ContractPosted, &actors, text.clone());
    log(world, id, text);
    Ok(id)
}

/// Is `viewer` a member of the gang or corp `f`, or `f` itself?
fn member(world: &World, viewer: EntityId, f: EntityId) -> bool {
    crate::systems::grudges::member_of(world, viewer, f)
}

/// C11: does `viewer` see record `id` on the board (on demand, never
/// cached)? A brokered record: the Fixer's regulars and members of gangs
/// that networked there or hold (or border) its district; a direct one: the
/// buyer, its edges but Enemies, its gang's members and its corp's staff;
/// a public one (no buyer): everyone; a Guard also Security corps' staff.
/// Nobody sees a record on themselves.
pub fn sees(world: &World, viewer: EntityId, id: ContractId) -> bool {
    let Some(c) = world.contracts.get(&id) else { return false };
    if c.target.id() == viewer {
        return false;
    }
    if c.kind == ContractKind::Guard {
        let security = world
            .corp_of_agent(viewer)
            .and_then(|corp| world.comp::<crate::components::Corp>(corp))
            .is_some_and(|corp| corp.niches.contains(&crate::components::Niche::Security));
        if security {
            return true;
        }
    }
    if let Some(b) = c.broker {
        let Some(k) = world.comp::<Broker>(b) else { return false };
        if k.regulars.contains_key(&viewer) {
            return true;
        }
        let Some(g) = world.gang_of(viewer) else { return false };
        if k.gangs.contains(&g) {
            return true;
        }
        let d = world.district_of_building(b).index();
        let mask = world.district_adjacent.get(d).copied().unwrap_or(0);
        return crate::systems::gang::held_districts(world, g)
            .iter()
            .any(|&(hd, _)| hd.index() == d || mask & (1 << hd.index()) != 0);
    }
    let Some(buyer) = c.buyer else { return true };
    if viewer == buyer || member(world, viewer, buyer) {
        return true;
    }
    if world.edge(buyer, viewer).is_some_and(|e| e.kind != RelKind::Enemy) {
        return true;
    }
    world.gang_of(buyer).is_some_and(|g| world.gang_of(viewer) == Some(g))
}

/// C11: the live records `viewer` sees (the Board panel, the Contract goal,
/// M18's quest log).
pub fn visible(world: &World, viewer: EntityId) -> Vec<ContractId> {
    world.contracts.iter().filter(|(_, c)| c.is_live()).map(|(&i, _)| i).filter(|&i| sees(world, viewer, i)).collect()
}

// ---------------------------------------------------------------------------
// Matching and acceptance
// ---------------------------------------------------------------------------

/// Does `id` hold a Taken record (as taker or crew)?
pub fn holds_taken(world: &World, id: EntityId) -> bool {
    world.by_party.get(&id).is_some_and(|l| {
        l.iter().any(|cid| {
            world
                .contracts
                .get(cid)
                .is_some_and(|c| c.status == ContractStatus::Taken && (c.taker == Some(id) || c.crew.contains(&id)))
        })
    })
}

/// A free living adult: no Sentence, not cuffed, not leaving.
pub fn free_adult(world: &World, id: EntityId) -> bool {
    crate::systems::law::living(world, id)
        && crate::systems::demography::is_adult(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Brain>(id).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
}

/// C12: may `cand` be offered record `c`: not its target, buyer or placing
/// agent, nor the target's Spouse, Family, Parent or Friend; a free adult
/// (and no guard: a shift is a shift); under `gun_lawfulness` for an
/// illegal kind; holding no Taken record.
pub fn eligible(world: &World, c: &Contract, cand: EntityId) -> bool {
    let t = c.target.id();
    if cand == t || Some(cand) == c.buyer || Some(cand) == c.agent {
        return false;
    }
    if !free_adult(world, cand) || crate::systems::law::is_guard(world, cand) {
        return false;
    }
    if world
        .edge(cand, t)
        .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend))
    {
        return false;
    }
    if c.kind.illegal() {
        let law = world.comp::<Personality>(cand).map_or(1.0, |p| p.lawfulness);
        if law > world.config.fixers.gun_lawfulness.get(c.kind) {
            return false;
        }
    }
    !holds_taken(world, cand)
}

/// C12: `risk_for(cand)` against a target whose defenders sum to `s_def`.
fn risk_against(world: &World, kind: ContractKind, cand: EntityId, s_def: f32) -> f32 {
    if kind.violent() {
        1.0 - missions::estimate_against(world, &[cand], s_def)
    } else {
        0.1
    }
}

/// C12: `risk_for(cand)` on record `c`.
pub fn risk_for(world: &World, c: &Contract, cand: EntityId) -> f32 {
    if c.kind.violent() {
        1.0 - missions::estimate(world, &[cand], &c.target)
    } else {
        0.1
    }
}

fn skill_for(world: &World, kind: ContractKind, cand: EntityId) -> f32 {
    match kind {
        ContractKind::Hit | ContractKind::Beat | ContractKind::Guard => crate::systems::law::fighting(world, cand),
        ContractKind::Locate => world.comp::<Skills>(cand).map_or(0.0, |s| s.stealth),
    }
}

/// C12: `skill × (1 − risk_for) × (0.5 + 0.5 × honour)`.
pub fn fit(world: &World, c: &Contract, cand: EntityId, s_def: f32) -> f32 {
    let honour = crate::systems::reputation::rep(world, cand).honour;
    skill_for(world, c.kind, cand) * (1.0 - risk_against(world, c.kind, cand, s_def)) * (0.5 + 0.5 * honour)
}

/// `U(x) = x ÷ (1 + x)` (plan C13: the spec writes U without defining it).
fn u_of(x: f32) -> f32 {
    let x = x.max(0.0);
    x / (1.0 + x)
}

/// C13 mode A's considerations for `who` on record `c` (without the gate);
/// `urgency` is `1 − time_left ÷ total` (1.0 for `accept_score`).
fn mode_a(world: &World, who: EntityId, c: &Contract, risk: f32, urgency_v: f32) -> Vec<Consideration> {
    let wage = world.comp::<Job>(who).map_or(0, |j| j.wage_per_day);
    let week_wage = (7 * wage).max(50) as f32;
    let p = world.comp::<Personality>(who);
    let mut cs = vec![
        Consideration::new(
            "U(price/week wage)",
            u_of(c.price as f32 / week_wage),
            Curve::Logistic { k: 8.0, mid: 0.3 },
        ),
        Consideration::new("1-risk", 1.0 - risk, Curve::Linear { m: 0.7, b: 0.3 }),
    ];
    if !c.brokered() {
        let honour = c.buyer.map_or(0.5, |b| crate::systems::reputation::rep(world, b).honour);
        cs.push(Consideration::new("buyer honour", honour, Curve::Linear { m: 0.5, b: 0.5 }));
    }
    if c.kind.illegal() {
        let law = p.map_or(0.5, |p| p.lawfulness);
        cs.push(Consideration::new("1-lawfulness", 1.0 - law, Curve::Linear { m: 0.6, b: 0.4 }));
    }
    if c.kind.violent() {
        let courage = p.map_or(0.5, |p| p.courage);
        cs.push(Consideration::new("courage", courage, Curve::Linear { m: 0.5, b: 0.5 }));
    }
    cs.push(Consideration::new("urgency", urgency_v, Curve::Linear { m: 0.5, b: 0.5 }));
    cs
}

/// C13: mode A's raw product without the gate, urgency 1.0: what a
/// candidate's acceptance reads at any tier (`Personality`, `Skills`,
/// `Kit`, `Job`, reputation), so a Statistical taker accepts as a Full one.
pub fn accept_score(world: &World, who: EntityId, id: ContractId) -> f32 {
    let Some(c) = world.contracts.get(&id) else { return 0.0 };
    let risk = risk_for(world, c, who);
    mode_a(world, who, c, risk, 1.0).iter().map(|x| x.output).product()
}

/// C17: `Live` when the target or the taker is a body, pinned or on
/// screen, or the record is a Guard taken by an agent; else `Ledger`.
pub fn render_for(world: &World, id: ContractId) -> Render {
    let Some(c) = world.contracts.get(&id) else { return Render::Ledger };
    if c.kind == ContractKind::Guard && c.taker.is_some_and(|t| world.has::<Identity>(t)) {
        return Render::Live;
    }
    // Phase 2 (C22, C25, C26): a squad or a gang's job is always live.
    if is_mission(world, c) {
        return Render::Live;
    }
    let shown = |e: EntityId| {
        world.comp::<Brain>(e).is_some_and(|b| b.lod != Lod::Statistical || b.pinned)
            || world.view_rect.zip(world.comp::<Position>(e)).is_some_and(|(r, p)| r.contains(p.tile))
    };
    let mut ids: SmallVec<[EntityId; 6]> = SmallVec::new();
    ids.extend(c.taker);
    ids.extend(c.crew.iter().copied());
    if let Target::Agent(t) = c.target {
        ids.push(t);
    }
    if ids.into_iter().any(shown) {
        Render::Live
    } else {
        Render::Ledger
    }
}

/// Live records being worked now (each one run or one mission, C17;
/// phase 2: `max_missions` counts both).
fn live_count(world: &World) -> usize {
    world.contract_runs.len() + world.missions.len()
}

/// C17: start a live record's run: the taker promoted to Coarse, its
/// chase opened (`Ask` for Hit, Beat and Locate; `Watch` at the client for
/// a Guard). `false` when `max_missions` runs are going (it stays queued).
fn start_live(world: &mut World, id: ContractId) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    // Phase 2 (C22): a squad or a gang's job marches as a mission.
    if is_mission(world, &c) {
        return open_mission(world, id);
    }
    let Some(taker) = c.taker.filter(|&t| world.has::<Identity>(t)) else { return false };
    if world.contract_runs.contains_key(&taker) {
        return true;
    }
    if live_count(world) >= world.config.missions.max_missions {
        return false;
    }
    let now = world.tick;
    if let Some(x) = world.contracts.get_mut(&id) {
        x.render = Render::Live;
        x.due = None;
    }
    let (phase, intel) = match (c.kind, c.target) {
        (ContractKind::Guard, Target::Building(b)) => (
            HuntPhase::Watch,
            world.comp::<Building>(b).map(|bd| Intel { building: Some(b), tile: bd.door, source: IntelSource::Habit }),
        ),
        (ContractKind::Guard, Target::Agent(a)) => (HuntPhase::Watch, Some(crate::systems::hunt::habit(world, a, now))),
        _ => (HuntPhase::Ask, None),
    };
    world.contract_runs.insert(
        taker,
        ContractRun {
            contract: id,
            target: c.target.id(),
            since: now,
            phase,
            venue: None,
            intel,
            stakeout_until: None,
            deceived: false,
            liar: None,
        },
    );
    if world.comp::<Brain>(taker).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, taker, Lod::Coarse);
    }
    crate::systems::hunt::reindex(world);
    true
}

/// C12, C17: `taker` (and `crew`) take record `id`: `Taken`, `known_by`
/// and the `Hired` first-hand copies, the render, a ledger `due` (drawn on
/// `ContractNs::Due`) or the live run. `ContractTaken`.
pub fn accept(world: &mut World, id: ContractId, taker: EntityId, crew: &[EntityId]) -> bool {
    let Some(c) = world.contracts.get(&id) else { return false };
    if c.status != ContractStatus::Open {
        return false;
    }
    let now = world.tick;
    if let Some(c) = world.contracts.get_mut(&id) {
        c.status = ContractStatus::Taken;
        c.taker = Some(taker);
        c.crew = crew.iter().copied().collect();
        c.taken = Some(now);
        if c.kind == ContractKind::Guard {
            c.guard_since = Some(now);
        }
    }
    index_parties(world, id);
    hear_hired(world, id, taker);
    for &m in crew {
        hear_hired(world, id, m);
    }
    let render = render_for(world, id);
    let tag = match render {
        Render::Live => {
            if let Some(c) = world.contracts.get_mut(&id) {
                c.render = Render::Live;
            }
            if start_live(world, id) {
                "LIVE"
            } else {
                "QUEUED"
            }
        }
        _ => {
            let (taken, deadline) = world.contracts.get(&id).map_or((now, now), |c| (now, c.deadline));
            let span = (deadline.saturating_sub(taken) / 2).max(60);
            let u: f64 = world.rng.contract(ContractNs::Due, id, 0).random();
            let due = taken + (u * span as f64).floor() as Tick;
            if let Some(c) = world.contracts.get_mut(&id) {
                c.render = Render::Ledger;
                c.due = Some(due);
            }
            world.ledger_due.insert((due, id));
            "LEDGER"
        }
    };
    // Phase 1 sends every Hit solo; a weak solo gun is counted (C25).
    if let Some(c) = world.contracts.get(&id) {
        if c.kind == ContractKind::Hit
            && c.crew.is_empty()
            && missions::estimate(world, &[taker], &c.target) < world.config.missions.squad_below
        {
            world.stats.current.contract.hits_solo_weak += 1;
        }
    }
    let (kind, target) =
        world.contracts.get(&id).map(|c| (c.kind, c.target.id())).unwrap_or((ContractKind::Hit, taker));
    let crew_txt = if crew.is_empty() { String::new() } else { format!(", crew {}", crew.len()) };
    let text = format!(
        "{} took the {} on {} ({tag}{crew_txt})",
        world.owner_label(Some(taker)),
        kind.label(),
        world.name_of(target)
    );
    world.push_event(EventKind::ContractTaken, &[taker, target], text.clone());
    log(world, id, text);
    true
}

/// C12: one record's offers: its candidates by `fit` descending (ties the
/// lower id), offered in turn until one accepts (`accept_score ≥
/// accept_min`). A refusal costs nothing; one acceptance per candidate per
/// day (`taken_today`).
fn offer(
    world: &mut World,
    id: ContractId,
    cands: Vec<EntityId>,
    gangs: Vec<EntityId>,
    taken_today: &mut std::collections::BTreeSet<EntityId>,
) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let s_def = missions::defender_strength(world, &c.target);
    let mut scored: Vec<(f32, EntityId)> = cands
        .into_iter()
        .filter(|a| !taken_today.contains(a))
        .filter(|&a| eligible(world, &c, a))
        .map(|a| (fit(world, &c, a, s_def), a))
        .collect();
    // Phase 2 (C12, C26): a gang is a candidate for a Hit or a Beat, scored
    // by its best member.
    for g in gangs {
        if taken_today.contains(&g) || !gang_eligible(world, &c, g) {
            continue;
        }
        let best = job_crew(world, &c, g).iter().map(|&m| fit(world, &c, m, s_def)).fold(0.0f32, f32::max);
        scored.push((best, g));
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let min = world.config.contracts.accept_min;
    for (_, a) in scored {
        if world.has::<crate::components::Gang>(a) {
            if leader_accepts(world, a, missions::buyer_faction(world, &c)) {
                taken_today.insert(a);
                return accept_gang(world, id, a);
            }
            continue;
        }
        let risk = risk_against(world, c.kind, a, s_def);
        let score: f32 = mode_a(world, a, &c, risk, 1.0).iter().map(|x| x.output).product();
        if score >= min {
            // Phase 2 (C25): a weak gun brings a squad.
            let crew = squad_crew(world, &c, a);
            taken_today.insert(a);
            taken_today.extend(crew.iter().copied());
            return accept(world, id, a, &crew);
        }
    }
    false
}

/// C12, daily at `match_hour`: (1) each open Fixer (by id) ranks its book's
/// Open records by `price × (1 + age_days ÷ deadline_days)` (ties the
/// lower id) and works the top `offers_per_day` against its regulars; (2)
/// every Open direct record (by id) against its viewers; (3) public
/// Locates are never matched (a stream, C27). Gangs and Security corps as
/// candidates are phases 2 and 3.
pub fn match_day(world: &mut World) {
    let now = world.tick;
    let mut taken_today = std::collections::BTreeSet::new();
    for f in open_fixers(world) {
        let Some(k) = world.comp::<Broker>(f) else { continue };
        let regulars: Vec<EntityId> = k.regulars.keys().copied().collect();
        let mut book: Vec<(f64, ContractId)> = k
            .book
            .iter()
            .filter_map(|id| world.contracts.get(id))
            .filter(|c| c.is_open())
            .map(|c| {
                let age = now.saturating_sub(c.posted) as f64 / TICKS_PER_DAY as f64;
                let days = (c.deadline.saturating_sub(c.posted) as f64 / TICKS_PER_DAY as f64).max(1.0);
                (c.price as f64 * (1.0 + age / days), c.id)
            })
            .collect();
        book.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let gangs: Vec<EntityId> = k.gangs.iter().copied().collect();
        for &(_, id) in book.iter().take(usize::from(world.config.fixers.offers_per_day)) {
            offer(world, id, regulars.clone(), gangs.clone(), &mut taken_today);
        }
    }
    let direct: Vec<ContractId> = world
        .contracts
        .values()
        .filter(|c| c.is_open() && c.broker.is_none())
        .filter(|c| !(c.buyer.is_none() && c.kind == ContractKind::Locate))
        .map(|c| c.id)
        .collect();
    for id in direct {
        let cands = direct_candidates(world, id);
        // Phase 2 (C26): a direct record's gang candidate is the buyer's gang.
        let gangs: Vec<EntityId> = world
            .contracts
            .get(&id)
            .and_then(|c| c.buyer)
            .and_then(|b| if world.has::<crate::components::Gang>(b) { Some(b) } else { world.gang_of(b) })
            .into_iter()
            .collect();
        offer(world, id, cands, gangs, &mut taken_today);
    }
}

/// C11: a direct record's viewers who could take it: the buyer's edges but
/// Enemies, its gang's members, its corp's staff (a public record: every
/// adult).
fn direct_candidates(world: &World, id: ContractId) -> Vec<EntityId> {
    let Some(c) = world.contracts.get(&id) else { return Vec::new() };
    let Some(buyer) = c.buyer else {
        // scan-ok: daily, a public record (god only in phase 1).
        return world.citizens();
    };
    let mut out: Vec<EntityId> = Vec::new();
    if world.has::<Identity>(buyer) {
        out.extend(world.neighbours(buyer).filter(|&o| world.edge(buyer, o).is_some_and(|e| e.kind != RelKind::Enemy)));
    }
    let gang = if world.has::<crate::components::Gang>(buyer) { Some(buyer) } else { world.gang_of(buyer) };
    if let Some(g) = gang.and_then(|g| world.comp::<crate::components::Gang>(g)) {
        out.extend(g.members.iter().copied());
    }
    if world.has::<crate::components::Corp>(buyer) {
        out.extend(crate::systems::ownership::employees_of(world, buyer));
    }
    out.sort();
    out.dedup();
    out.retain(|&a| sees(world, a, id));
    out
}

// ---------------------------------------------------------------------------
// Settlement and the status machine
// ---------------------------------------------------------------------------

/// The one status writer (C16): the closing tick, the indices (the
/// ledger, the Locate targets, the Fixer's book, the live run and its
/// plan), the counters, the log and the event.
pub fn set_status(world: &mut World, id: ContractId, status: ContractStatus, why: &str) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let now = world.tick;
    if let Some(x) = world.contracts.get_mut(&id) {
        x.status = status;
        if !status.is_live() {
            x.closed = Some(now);
        }
    }
    if c.status == ContractStatus::Taken && status != ContractStatus::Taken {
        if let Some(d) = c.due {
            world.ledger_due.remove(&(d, id));
        }
        if let Some(t) = c.taker {
            end_run(world, t, id);
        }
        for &m in &c.crew {
            end_run(world, m, id);
        }
        // Phase 2 (C22): the mission ends with the record's Taken state.
        end_mission(world, id);
    }
    if !status.is_live() {
        if let Target::Agent(t) = c.target {
            if let Some(l) = world.locate_targets.get_mut(&t) {
                l.retain(|x| *x != id);
                if l.is_empty() {
                    world.locate_targets.remove(&t);
                }
            }
        }
        if let Some(b) = c.broker {
            if let Some(k) = world.comp_mut::<Broker>(b) {
                k.book.retain(|&x| x != id);
            }
        }
    }
    let kind = c.kind;
    // Phase 2 (C25): a squad of two or more.
    let squad = squad_size(world, &c) >= 2;
    let row = &mut world.stats.current.contract;
    match status {
        ContractStatus::Fulfilled => {
            row.contracts_fulfilled += 1;
            row.k_done[kind.index()] += 1;
            if kind == ContractKind::Hit {
                row.hits_done += 1;
                row.contract_murders += 1;
                if squad {
                    row.hits_squad += 1;
                }
            }
        }
        ContractStatus::Reneged => {
            row.reneged += 1;
            if kind == ContractKind::Hit {
                row.hits_done += 1;
                row.contract_murders += 1;
            }
        }
        ContractStatus::Failed => row.contracts_failed += 1,
        ContractStatus::Expired => row.contracts_expired += 1,
        ContractStatus::Cancelled => row.contracts_cancelled += 1,
        ContractStatus::Open | ContractStatus::Taken => {}
    }
    let target = c.target.id();
    let tname = world.name_of(target);
    let taker = c.taker.unwrap_or(EntityId::NONE);
    let (ev, text) = match status {
        ContractStatus::Fulfilled if why.is_empty() => (
            EventKind::ContractFulfilled,
            format!("{} on {tname}: done by {}", kind.label(), world.owner_label(c.taker)),
        ),
        ContractStatus::Fulfilled => (
            EventKind::ContractFulfilled,
            format!("{} on {tname}: done by {} ({why})", kind.label(), world.owner_label(c.taker)),
        ),
        ContractStatus::Failed => (EventKind::ContractFailed, format!("{} on {tname}: failed ({why})", kind.label())),
        ContractStatus::Expired => (EventKind::ContractExpired, format!("{} on {tname}: expired", kind.label())),
        ContractStatus::Cancelled => {
            (EventKind::ContractFailed, format!("{} on {tname}: cancelled ({why})", kind.label()))
        }
        ContractStatus::Reneged => (
            EventKind::Reneged,
            format!("{} stiffed {} on the {}", world.owner_label(c.buyer), world.owner_label(c.taker), kind.label()),
        ),
        ContractStatus::Open | ContractStatus::Taken => return,
    };
    let actors =
        if status == ContractStatus::Reneged { [c.buyer.unwrap_or(EntityId::NONE), taker] } else { [taker, target] };
    world.push_event(ev, &actors, text.clone());
    log(world, id, text);
}

/// Drop `who`'s run on record `id` (and its Contract plan).
fn end_run(world: &mut World, who: EntityId, id: ContractId) {
    if world.contract_runs.get(&who).is_some_and(|r| r.contract == id) {
        world.contract_runs.remove(&who);
        crate::systems::hunt::reindex(world);
        if world.comp::<Brain>(who).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
            world.abort_plan(who);
        }
        for list in world.contract_guards.values_mut() {
            list.retain(|g| *g != who);
        }
        world.contract_guards.retain(|_, l| !l.is_empty());
    }
}

/// The purse a refund goes to: the buyer while it can hold coins, else
/// the Treasury.
fn refund_to(world: &World, buyer: Option<EntityId>) -> Option<EntityId> {
    buyer.filter(|&b| buyer_ok(world, Some(b)))
}

/// C16: every coin left in escrow back to the buyer (`Flow::Escrow`).
pub fn refund(world: &mut World, id: ContractId) -> i64 {
    let Some(c) = world.contracts.get(&id) else { return 0 };
    let (to, held) = (refund_to(world, c.buyer), c.escrow);
    ownership::escrow_out(world, id, to, held, Flow::Escrow)
}

/// The payees of a fulfilment: the taker first, then the crew.
fn payees(c: &Contract) -> SmallVec<[EntityId; 6]> {
    let mut v: SmallVec<[EntityId; 6]> = SmallVec::new();
    v.extend(c.taker);
    v.extend(c.crew.iter().copied());
    v
}

/// `total` split equally over `n`, the remainder coin to the first.
fn shares(total: i64, n: usize) -> Vec<i64> {
    let n = n.max(1) as i64;
    let each = total / n;
    let mut v = vec![each; n as usize];
    v[0] += total - each * n;
    v
}

/// C16: the one money path out of a live record.
pub fn settle(world: &mut World, id: ContractId, how: Settle, why: &str) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if !c.is_live() {
        return;
    }
    match how {
        Settle::Fulfilled => fulfil(world, id, &c, why),
        Settle::Failed => {
            refund(world, id);
            set_status(world, id, ContractStatus::Failed, why);
        }
        Settle::Expired => {
            refund(world, id);
            set_status(world, id, ContractStatus::Expired, why);
        }
        Settle::Cancelled => {
            refund(world, id);
            set_status(world, id, ContractStatus::Cancelled, why);
        }
    }
    debug_assert!(
        world.contracts.get(&id).is_none_or(|x| x.is_live() || x.escrow == 0),
        "a closed record holds no escrow"
    );
}

fn fulfil(world: &mut World, id: ContractId, c: &Contract, why: &str) {
    let price = c.price;
    if c.kind == ContractKind::Locate {
        // The sightings were paid as they came (C27): the Fixer's cut of
        // the price from what is left, the rest back to the buyer.
        if let Some(b) = c.broker {
            let cut = ((price as f32) * c_cut(world, b)).round() as i64;
            let owner = world.owner_of(b);
            let paid = ownership::escrow_out(world, id, owner, cut, Flow::FixerCut);
            note_cut(world, b, paid);
        }
        refund(world, id);
        set_status(world, id, ContractStatus::Fulfilled, "");
        return;
    }
    let pay_to = payees(c);
    if let Some(b) = c.broker {
        let held = world.contracts.get(&id).map_or(0, |x| x.escrow);
        let cut = ((held as f32) * c_cut(world, b)).round() as i64;
        let owner = world.owner_of(b);
        let to_takers = held - cut;
        for (who, share) in pay_to.iter().zip(shares(to_takers, pay_to.len())) {
            ownership::escrow_out(world, id, Some(*who), share, Flow::Payout);
        }
        let paid = ownership::escrow_out(world, id, owner, cut, Flow::FixerCut);
        note_cut(world, b, paid);
        refund(world, id);
        set_status(world, id, ContractStatus::Fulfilled, why);
        return;
    }
    // C6: a direct record pays from the buyer's purse, unless it reneges or
    // runs short (the taker cannot tell malice from poverty).
    if c.renege {
        renege(world, id, c);
        return;
    }
    let mut moved = 0;
    for (who, share) in pay_to.iter().zip(shares(price, pay_to.len())) {
        moved += if c.buyer.is_none() {
            ownership::charge(world, None, Some(*who), share, Flow::Payout)
        } else {
            ownership::pay(world, c.buyer, Some(*who), share, Flow::Payout)
        };
    }
    if moved < price {
        renege(world, id, c);
    } else {
        set_status(world, id, ContractStatus::Fulfilled, why);
    }
}

fn c_cut(world: &World, fixer: EntityId) -> f32 {
    world.comp::<Broker>(fixer).map_or(world.config.fixers.fixer_cut, |k| k.cut)
}

/// The Fixer's revenue: the building's credit and today's income.
fn note_cut(world: &mut World, fixer: EntityId, paid: i64) {
    if paid <= 0 {
        return;
    }
    ownership::credit(world, fixer, paid);
    if let Some(k) = world.comp_mut::<Broker>(fixer) {
        k.income_today += paid;
    }
}

/// C6: the buyer did not pay in full: `Reneged`, a `Betrayed` deed (actor
/// the buyer, object the taker) in the taker's talk district and first
/// hand in the taker, and the taker's `Betrayed` grudge on the buyer.
fn renege(world: &mut World, id: ContractId, c: &Contract) {
    set_status(world, id, ContractStatus::Reneged, "");
    let Some(taker) = c.taker else { return };
    let Some(buyer) = c.buyer.or(c.agent) else { return };
    let d = crate::systems::gossip::home_district(world, taker)
        .or_else(|| world.comp::<Position>(taker).map(|p| world.district_of(p.tile)));
    if let Some(d) = d {
        crate::systems::gossip::post_deed(world, d, Deed::Betrayed, Some(buyer), Some(taker));
    }
    if crate::systems::law::living(world, taker) && world.has::<Memory>(taker) {
        let sal = world.config.gossip.deed_sal.get(Deed::Betrayed);
        let e = MemoryEntry {
            subject: Some(buyer),
            salience: sal,
            valence: -world.config.gossip.deed_sev.get(Deed::Betrayed) * sal,
            deed: Some(Deed::Betrayed),
            object: Some(taker),
            ..MemoryEntry::blank(MemoryKind::Rumour, world.tick)
        };
        crate::systems::memory::hear_entry(world, taker, e);
    }
    let w = world.config.contracts.renege_grudge;
    if crate::systems::law::living(world, taker) {
        crate::systems::grudges::add(world, taker, buyer, crate::word::GrudgeCause::Betrayed, w, 0);
    }
}

/// C16: a failed attempt (taker dead, jailed or beaten; a strike `Fail`):
/// back to `Open` with `attempts + 1`, taker and crew cleared, until
/// `max_attempts`, then `Failed` with the refund. `ContractFailed`.
pub fn fail_attempt(world: &mut World, id: ContractId, why: &str) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if c.status != ContractStatus::Taken {
        return;
    }
    let attempts = c.attempts.saturating_add(1);
    if let Some(x) = world.contracts.get_mut(&id) {
        x.attempts = attempts;
    }
    if attempts >= world.config.contracts.max_attempts {
        settle(world, id, Settle::Failed, why);
        return;
    }
    if let Some(d) = c.due {
        world.ledger_due.remove(&(d, id));
    }
    if let Some(t) = c.taker {
        end_run(world, t, id);
    }
    for &m in &c.crew {
        end_run(world, m, id);
    }
    // Phase 2 (C22): the mission ends; the record goes back on the board.
    end_mission(world, id);
    // Review fix: the departing taker and crew are no longer parties.
    for p in c.taker.into_iter().chain(c.crew.iter().copied()) {
        let still = Some(p) == c.buyer || Some(p) == c.agent || p == c.target.id();
        if still {
            continue;
        }
        if let Some(list) = world.by_party.get_mut(&p) {
            list.retain(|x| *x != id);
            if list.is_empty() {
                world.by_party.remove(&p);
            }
        }
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.status = ContractStatus::Open;
        x.taker = None;
        x.crew.clear();
        x.taken = None;
        x.due = None;
        x.render = Render::Ledger;
        x.guard_since = None;
        x.holds = 0;
    }
    let text = format!("{} on {}: attempt {attempts} failed ({why})", c.kind.label(), world.name_of(c.target.id()));
    world.push_event(EventKind::ContractFailed, &[c.taker.unwrap_or(EntityId::NONE), c.target.id()], text.clone());
    log(world, id, text);
}

/// C16, from `World::kill_by` (and `remove_agent`), before the dead is
/// unlinked: a target killed by its taker fulfils a Hit or Beat; by other
/// hands (or not by violence) cancels it (a Locate with a paid sighting is
/// fulfilled); a Guard's client dead fails it; a dead taker fails the
/// attempt; a dead buyer cancels (the refund lands in its wallet, its
/// estate's).
pub fn on_death(world: &mut World, dead: EntityId, killer: Option<EntityId>) {
    if world.contracts.is_empty() {
        return;
    }
    let Some(ids) = world.by_party.get(&dead).cloned() else { return };
    for id in ids {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        if !c.is_live() {
            continue;
        }
        let by_taker = killer.is_some_and(|k| c.taker == Some(k) || c.crew.contains(&k));
        if c.target.id() == dead {
            match c.kind {
                ContractKind::Hit | ContractKind::Beat if c.status == ContractStatus::Taken && by_taker => {
                    settle(world, id, Settle::Fulfilled, "");
                }
                ContractKind::Guard if c.status == ContractStatus::Taken => {
                    settle(world, id, Settle::Failed, "the client died");
                }
                ContractKind::Locate if c.paid_sightings > 0 => settle(world, id, Settle::Fulfilled, ""),
                _ => settle(world, id, Settle::Cancelled, "the target is dead"),
            }
        } else if c.status == ContractStatus::Taken && (c.taker == Some(dead) || c.crew.contains(&dead)) {
            // Phase 2 (C22): a mission loses one of its crew (the last one
            // out fails the attempt); a solo taker's death fails it.
            if world.missions.contains_key(&id) {
                drop_crew(world, id, dead);
            } else {
                fail_attempt(world, id, "the taker died");
            }
        } else if c.buyer == Some(dead) {
            settle(world, id, Settle::Cancelled, "the buyer is gone");
        }
    }
}

/// C19: does `actor` work a live record on `victim` (its run's target)?
pub fn live_job_on(world: &World, actor: EntityId, victim: EntityId) -> bool {
    // Phase 2 (C19, C23): a mission's crew working it (its target, or
    // whoever stands at its door in the brawl) does the record's own violence.
    if let Some(m) = world.mission_of.get(&actor).filter(|_| !world.missions.is_empty()) {
        let on_target = world.contracts.get(m).and_then(|c| c.target_agent()) == Some(victim);
        if on_target || world.comp::<Brain>(actor).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
            return true;
        }
    }
    !world.contract_runs.is_empty() && world.contract_runs.get(&actor).is_some_and(|r| r.target == victim)
}

/// C15: a contract Hit's strike kills at `hunt_kill_p`; a Beat and every
/// other strike at the base rate.
pub fn strike_kill_p(world: &World, taker: EntityId, victim: EntityId) -> Option<f32> {
    let r = world.contract_runs.get(&taker).filter(|r| r.target == victim)?;
    let c = world.contracts.get(&r.contract)?;
    (c.kind == ContractKind::Hit).then_some(world.config.hunt.hunt_kill_p)
}

/// C15, from `hunt::on_strike` (the `Attack` completion): a Beat won is
/// fulfilled, a strike lost fails the attempt; a Hit won whose target
/// lives sends the run back to asking (a death settled at `on_death`).
pub fn on_strike(world: &mut World, taker: EntityId, victim: EntityId, winner: EntityId, died: bool) {
    let Some(r) = world.contract_runs.get(&taker).filter(|r| r.target == victim).cloned() else { return };
    let Some(c) = world.contracts.get(&r.contract).cloned() else { return };
    if c.status != ContractStatus::Taken {
        return;
    }
    if winner != taker {
        if !died {
            fail_attempt(world, c.id, "the taker was beaten");
        }
        return;
    }
    match c.kind {
        ContractKind::Beat => settle(world, c.id, Settle::Fulfilled, ""),
        ContractKind::Hit if !died => {
            if let Some(mut x) = crate::systems::hunt::chase(world, taker) {
                x.phase = HuntPhase::Ask;
                x.intel = None;
                x.venue = None;
                x.stakeout_until = None;
                crate::systems::hunt::set_chase(world, taker, x);
                crate::systems::hunt::reindex(world);
            }
        }
        _ => {}
    }
}

/// C32: an on-post Guard taker within 2 tiles of `victim` that an attack
/// on it meets first (the lowest id), not the attacker.
pub fn guard_for(world: &World, attacker: EntityId, victim: EntityId) -> Option<EntityId> {
    if world.contract_guards.is_empty() {
        return None;
    }
    world
        .contract_guards
        .get(&victim)?
        .iter()
        .copied()
        .filter(|&g| g != attacker && crate::systems::law::living(world, g))
        .find(|&g| crate::systems::law::near(world, g, victim, 2))
}

// ---------------------------------------------------------------------------
// The ledger (C18)
// ---------------------------------------------------------------------------

/// C18: a ledger record at `due`: the strike's `p_win` (the solo taker
/// against the target's expected defenders), one draw on
/// `ContractNs::Outcome` (key `id, attempts`). A Hit won settles and opens
/// a pre-bound `Killed` hole through `lod::stat_hit` (the binder names the
/// taker); a Beat won an `Assaulted` hole; a Locate pays `cap_sightings ÷
/// 2` sightings at the target's habit tile. A loss fails the attempt and,
/// below `ledger_death_p`, may cost the taker its life on
/// `ContractNs::TakerDeath` (a hole pre-bound to the target). Phase 2
/// adds the strike decision's Hold and SellOut.
pub fn resolve_ledger(world: &mut World, id: ContractId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if c.status != ContractStatus::Taken || c.render != Render::Ledger {
        return;
    }
    let Some(taker) = c.taker else { return };
    if world.contract_runs.contains_key(&taker) {
        return;
    }
    if !free_adult(world, taker) {
        fail_attempt(world, id, "the taker was not free");
        return;
    }
    let Target::Agent(t) = c.target else {
        // A ledger Guard cannot occur (a Guard is always live).
        return;
    };
    if !crate::systems::law::living(world, t) {
        settle(world, id, Settle::Cancelled, "the target is dead");
        return;
    }
    let now = world.tick;
    let habit = crate::systems::hunt::habit(world, t, now);
    let district = habit.building.map_or_else(|| world.district_of(habit.tile), |b| world.district_of_building(b));
    if c.kind == ContractKind::Locate {
        let n = world.config.bounty.cap_sightings / 2;
        for _ in 0..n {
            if !pay_sighting(world, id, taker, t, habit.tile, true) {
                break;
            }
        }
        if let Some(x) = world.contracts.get_mut(&id) {
            x.due = None;
        }
        return;
    }
    // Phase 2 (C18, C24): the strike decision against the habit's district
    // first: a Hold looks again in a day, a SellOut hands the record to the
    // district's gang (live), a Fail is a failed attempt.
    let door = habit.building.and_then(|b| missions::door_of(world, b)).unwrap_or(habit.tile);
    match missions::decide(world, id, &[taker], door) {
        crate::contract::Decision::Strike => {}
        crate::contract::Decision::Hold => {
            let due = missions::hold_until(now);
            if let Some(old) = world.contracts.get_mut(&id).and_then(|x| x.due.replace(due)) {
                world.ledger_due.remove(&(old, id));
            }
            world.ledger_due.insert((due, id));
            return;
        }
        _ => return,
    }
    let p_win = world
        .contracts
        .get(&id)
        .and_then(|x| x.strike)
        .map_or_else(|| missions::estimate(world, &[taker], &c.target), |s| s.p_win);
    let attempts = u64::from(c.attempts);
    let win = world.rng.contract(ContractNs::Outcome, id, attempts).random::<f32>() < p_win;
    let src = |faction: EntityId| crate::ledger::ActiveSource {
        source: crate::ledger::ViolenceSource::Contract(id),
        district,
        faction: Some(faction),
        riot: None,
        episode: None,
    };
    if win {
        match c.kind {
            ContractKind::Hit => {
                // Settled first, so the death's `on_death` finds it closed.
                settle(world, id, Settle::Fulfilled, "");
                crate::systems::lod::stat_hit(world, t, crate::components::HoleKind::Killed, Some(&src(taker)));
            }
            _ => {
                crate::systems::lod::stat_hit(world, t, crate::components::HoleKind::Assaulted, Some(&src(taker)));
                settle(world, id, Settle::Fulfilled, "");
            }
        }
        return;
    }
    let lethal = p_win < world.config.missions.ledger_death_p
        && world.rng.contract(ContractNs::TakerDeath, id, attempts).random::<f32>() < 1.0 - p_win;
    fail_attempt(world, id, "the strike was lost");
    if lethal && crate::systems::law::living(world, taker) {
        crate::systems::lod::stat_hit(world, taker, crate::components::HoleKind::Killed, Some(&src(t)));
    }
}

// ---------------------------------------------------------------------------
// Locate (C27)
// ---------------------------------------------------------------------------

/// C27: `gossip::wants_sighting`'s clause: `who` is the target of a live
/// Locate that `observer` sees.
pub fn locate_wants(world: &World, observer: EntityId, who: EntityId) -> bool {
    if world.locate_targets.is_empty() {
        return false;
    }
    world.locate_targets.get(&who).is_some_and(|l| l.iter().any(|&id| sees(world, observer, id)))
}

/// One paid sighting of `who` at `tile` by `writer` under record `id`
/// (`force` skips the gap: a ledger tracker's half-cap): the sighting into
/// the buyer's database (the law's under `EntityId::NONE`), `per_sighting`
/// from escrow (brokered), the buyer's purse (direct, capped) or the
/// Treasury (public, which also sets `World::last_seen`). `BountyPaid`;
/// the cap fulfils the record.
fn pay_sighting(
    world: &mut World,
    id: ContractId,
    writer: EntityId,
    who: EntityId,
    tile: TilePos,
    force: bool,
) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    if !c.is_live() || c.paid_sightings >= world.config.bounty.cap_sightings {
        return false;
    }
    let now = world.tick;
    let gap = Tick::from(world.config.bounty.min_gap_hours) * TICKS_PER_HOUR;
    if !force && c.last_paid.is_some_and(|t| now < t + gap) {
        return false;
    }
    if Some(writer) == c.buyer || Some(writer) == c.agent || writer == who {
        return false;
    }
    let s = crate::virt::Sighting { who, tile, tick: now, confidence: 1.0, relayed: true };
    let owner = c.buyer.unwrap_or(EntityId::NONE);
    crate::systems::gossip::relay_sighting(world, owner, s);
    if c.buyer.is_none() {
        world.last_seen.insert(who, (tile, now));
    }
    let per = world.config.bounty.per_sighting;
    let paid = if c.brokered() {
        ownership::escrow_out(world, id, Some(writer), per, Flow::Payout)
    } else if c.buyer.is_none() {
        ownership::charge(world, None, Some(writer), per, Flow::Payout)
    } else {
        ownership::pay(world, c.buyer, Some(writer), per, Flow::Payout)
    };
    // Nothing left to pay with: the sighting went in, unpaid.
    if paid <= 0 {
        return false;
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.paid_sightings = x.paid_sightings.saturating_add(1);
        x.last_paid = Some(now);
    }
    world.stats.current.contract.bounties_paid += 1;
    let text = format!("{} sold a sighting of {} for {paid}", world.name_of(writer), world.name_of(who));
    world.push_event(EventKind::BountyPaid, &[writer, who], text);
    if world.contracts.get(&id).is_some_and(|x| x.paid_sightings >= world.config.bounty.cap_sightings) {
        settle(world, id, Settle::Fulfilled, "");
    }
    true
}

/// C27, from `gossip::maybe_sight` after a body's sighting is written: each
/// live Locate on `who` that `observer` sees pays it (at most one per
/// `min_gap_hours`, `cap_sightings` in all).
pub fn on_sighting(world: &mut World, observer: EntityId, who: EntityId, tile: TilePos) {
    if world.locate_targets.is_empty() {
        return;
    }
    let Some(ids) = world.locate_targets.get(&who).cloned() else { return };
    for id in ids {
        if sees(world, observer, id) {
            pay_sighting(world, id, observer, who, tile, false);
        }
    }
}

/// C27, a Locate tracker's `StakeOut` contact: a paid sighting, and the
/// run goes back to asking (the tracker keeps tracking until the cap or
/// the deadline). No-op for any other run.
pub fn on_contact(world: &mut World, taker: EntityId) {
    let Some(r) = world.contract_runs.get(&taker).cloned() else { return };
    let Some(c) = world.contracts.get(&r.contract) else { return };
    if c.kind != ContractKind::Locate || c.status != ContractStatus::Taken {
        return;
    }
    let tile = world.comp::<Position>(r.target).map_or_else(Default::default, |p| p.tile);
    pay_sighting(world, r.contract, taker, r.target, tile, false);
    if let Some(mut x) = crate::systems::hunt::chase(world, taker) {
        x.phase = HuntPhase::Ask;
        x.intel = None;
        x.venue = None;
        x.stakeout_until = None;
        crate::systems::hunt::set_chase(world, taker, x);
        crate::systems::hunt::reindex(world);
    }
}

// ---------------------------------------------------------------------------
// Hunt hiring (C20)
// ---------------------------------------------------------------------------

/// C20: a Hunt holder too weak to win (`might_gap < hire_gap`) who can pay
/// posts a Hit (grudge `weight ≥ lethal_min`) or a Beat on its grudge
/// target instead of hunting: brokered with the nearest open Fixer within
/// `hire_reach_tiles` of its Home door, else direct; the Hunt cools for
/// `hunt_cooldown_days`. Tier-neutral: a Statistical holder posts at once.
pub fn try_hire(world: &mut World, holder: EntityId, target: EntityId, weight: f32) -> bool {
    if crate::systems::hunt::might_gap(world, holder, target) >= world.config.contracts.hire_gap {
        return false;
    }
    let dup = world.by_party.get(&holder).is_some_and(|l| {
        l.iter().any(|id| {
            world.contracts.get(id).is_some_and(|c| {
                c.is_live() && c.origin == Origin::Hunt && c.buyer == Some(holder) && c.target == Target::Agent(target)
            })
        })
    });
    if dup {
        return false;
    }
    let kind = if weight >= world.config.hunt.lethal_min { ContractKind::Hit } else { ContractKind::Beat };
    let tgt = Target::Agent(target);
    let broker = nearest_fixer(world, holder);
    if world.purse(Some(holder)) < quote(world, kind, &tgt, broker) {
        return false;
    }
    let days = world.config.contracts.deadline_days.get(kind);
    let posting = Posting {
        buyer: Some(holder),
        agent: Some(holder),
        kind,
        target: tgt,
        broker,
        deadline_days: days,
        origin: Origin::Hunt,
        price: None,
    };
    if post(world, posting).is_err() {
        return false;
    }
    let until = world.tick + Tick::from(world.config.hunt.hunt_cooldown_days) * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(holder) {
        b.cooldowns.insert(GoalKind::Hunt, until);
    }
    true
}

/// C20, daily: the grudge holders the Hunt score never reaches (every
/// tier, not hunting, not cooled), each on one hash-picked day in three,
/// whose heaviest eligible grudge passes the gap test and whose Hunt
/// considerations without the might term score `hire_min` or more, hire.
pub fn hire_pass(world: &mut World) {
    let day = world.day();
    let seed = world.seed();
    let now = world.tick;
    let min = world.config.contracts.hire_min;
    let mut queue: Vec<(EntityId, EntityId, f32)> = Vec::new();
    // scan-ok: daily, the grudge store.
    for id in world.with::<crate::word::Grudges>() {
        if world.hunts.contains_key(&id) {
            continue;
        }
        if !splitmix64(seed ^ u64::from(id.index) ^ day ^ HIRE_SALT).is_multiple_of(3) {
            continue;
        }
        if world.comp::<Brain>(id).and_then(|b| b.cooldowns.get(&GoalKind::Hunt)).is_some_and(|&t| t > now) {
            continue;
        }
        if !crate::systems::hunt::hunter_ok(world, id) {
            continue;
        }
        let Some((t, w, _)) = crate::systems::hunt::heaviest_eligible(world, id) else { continue };
        if crate::systems::hunt::might_gap(world, id, t) >= world.config.contracts.hire_gap {
            continue;
        }
        let Some((cs, flat)) = crate::systems::hunt::hire_considerations(world, id, t, w) else { continue };
        let Some(s) = crate::utility::score_goal(GoalKind::Hunt, cs, None, 0.0, flat) else { continue };
        if s.score >= min {
            queue.push((id, t, w));
        }
    }
    for (id, t, w) in queue {
        try_hire(world, id, t, w);
    }
}

// ---------------------------------------------------------------------------
// The Contract goal and its plans (C13, C15)
// ---------------------------------------------------------------------------

/// C13's gun gate: a free adult, jobless or paid below 0.7 × the median
/// wage, whose best of fighting, stealth, hacking, persuasion and
/// intimidation reaches `gun_skill_min`.
pub fn gun_gate(world: &World, id: EntityId) -> bool {
    // Cheapest first: this runs in every body's think (perf).
    let Some(s) = world.comp::<Skills>(id) else { return false };
    let best = s.fighting.max(s.stealth).max(s.hacking).max(s.persuasion).max(s.intimidation);
    if best < world.config.fixers.gun_skill_min {
        return false;
    }
    let wage = world.comp::<Job>(id).map(|j| j.wage_per_day);
    if wage.is_some_and(|w| w as f32 >= 0.7 * world.median_wage as f32) {
        return false;
    }
    free_adult(world, id)
}

/// C13: the Contract goal. Mode A (work a live record: the taker holds a
/// run) and mode B (network at a Fixer within reach: not a regular, or
/// lapsing within 2 days, and through the gun gate).
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    // Phase 2 (C22): a mission's crew member works its record as a taker does.
    let mission = if world.missions.is_empty() { None } else { world.mission_of.get(&id).copied() };
    // The crew goes about its day until the gather window (the Raid goal's
    // `raid_gather_hours`) opens before the muster.
    let window = Tick::from(world.config.gangs.raid_gather_hours) * TICKS_PER_HOUR;
    if mission.and_then(|m| world.missions.get(&m)).is_some_and(|m| world.tick + window < m.raid_at) {
        return None;
    }
    if let Some(cid) = world.contract_runs.get(&id).map(|r| r.contract).or(mission) {
        let c = world.contracts.get(&cid)?;
        if c.status != ContractStatus::Taken || !free_adult(world, id) {
            return None;
        }
        let total = c.deadline.saturating_sub(c.taken.unwrap_or(c.posted)).max(1) as f32;
        let left = c.deadline.saturating_sub(world.tick) as f32;
        let risk = match mission.and_then(|m| world.missions.get(&m)) {
            Some(m) => 1.0 - missions::estimate(world, &m.crew, &c.target),
            None => risk_for(world, c, id),
        };
        // Phase 2 (deviation): a mission's crew reads its muster, not the
        // deadline: inside the gather window the muster is called (urgency
        // 1, the Raid goal's "muster called").
        let urgency_v = if mission.is_some() { 1.0 } else { (1.0 - left / total).clamp(0.0, 1.0) };
        let mut cs = vec![Consideration::new("holds a taken contract", can(true), GATE)];
        cs.extend(mode_a(world, id, c, risk, urgency_v));
        return Some((cs, world.config.contracts.contract_flat));
    }
    if world.buildings_of_kind(BuildingKind::Fixer).is_empty() || !gun_gate(world, id) {
        return None;
    }
    if is_regular(world, id, 2 * TICKS_PER_DAY) {
        return None;
    }
    nearest_fixer(world, id)?;
    let p = world.comp::<Personality>(id)?;
    let n = world.comp::<Needs>(id)?;
    let cs = vec![
        Consideration::new("can network", can(true), GATE),
        Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    Some((cs, world.config.fixers.network_flat))
}

/// C13, C15: the Contract goal's scripted plan (`plan::plan_for`'s bypass;
/// `None` cools the goal). A run: Hit and Beat `GoTo(Intel) → [AskAround
/// → GoTo(Intel)] → StakeOut → Attack` (a fresh sighting, the taker's or
/// its gang's or the buyer's database, skips the asking), a Locate the
/// same without the Attack, a Guard `GoTo(client) → Guard`. No run:
/// `GoTo(Fixer) → Network`.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    // Phase 2 (C22): a crew member marches with its mission.
    if !world.missions.is_empty() && world.mission_of.contains_key(&id) {
        return missions::plan(world, id);
    }
    let step = |action, target| ActionInstance { action, target, tile: None };
    let now = world.tick;
    let Some(r) = world.contract_runs.get(&id).cloned() else {
        let f = nearest_fixer(world, id)?;
        let steps = vec![step(ActionKind::GoTo(LocationKey::Seller), Some(f)), step(ActionKind::Network, Some(f))];
        return Some(Plan { goal: GoalKind::Contract, target: Some(f), steps, started_tick: now });
    };
    let c = world.contracts.get(&r.contract)?.clone();
    if c.status != ContractStatus::Taken {
        return None;
    }
    if c.kind == ContractKind::Guard {
        let steps = match c.target {
            Target::Building(b) => {
                vec![step(ActionKind::GoTo(LocationKey::Seller), Some(b)), step(ActionKind::Guard, Some(b))]
            }
            Target::Agent(a) => {
                let i = crate::systems::hunt::habit(world, a, now);
                if let Some(mut x) = crate::systems::hunt::chase(world, id) {
                    x.phase = HuntPhase::Watch;
                    x.intel = Some(i);
                    crate::systems::hunt::set_chase(world, id, x);
                }
                vec![step(ActionKind::GoTo(LocationKey::Intel), None), step(ActionKind::Guard, Some(a))]
            }
        };
        crate::systems::hunt::reindex(world);
        return Some(Plan { goal: GoalKind::Contract, target: Some(c.target.id()), steps, started_tick: now });
    }
    let t = r.target;
    if !crate::systems::hunt::target_ok(world, t) {
        return None;
    }
    let mut next = crate::systems::hunt::chase(world, id)?;
    if next.phase == HuntPhase::Ask {
        let fresh = crate::systems::hunt::fresh_intel(world, id, t).or_else(|| buyer_intel(world, &c, t));
        if let Some(i) = fresh {
            next.phase = HuntPhase::Watch;
            next.intel = Some(i);
        } else {
            match crate::systems::hunt::last_known_district(world, id, t)
                .and_then(|d| crate::systems::hunt::busiest_bar(world, d))
            {
                Some(v) => next.venue = Some(v),
                None => {
                    next.phase = HuntPhase::Watch;
                    next.intel = Some(crate::systems::hunt::habit(world, t, now));
                }
            }
        }
    }
    if next.phase == HuntPhase::Watch && next.intel.is_none() {
        next.intel = Some(crate::systems::hunt::habit(world, t, now));
    }
    let phase = next.phase;
    crate::systems::hunt::set_chase(world, id, next);
    crate::systems::hunt::reindex(world);
    let tt = Some(t);
    let mut steps = vec![step(ActionKind::GoTo(LocationKey::Intel), None)];
    if phase == HuntPhase::Ask {
        steps.push(step(ActionKind::AskAround, tt));
        steps.push(step(ActionKind::GoTo(LocationKey::Intel), None));
    }
    steps.push(step(ActionKind::StakeOut, tt));
    if c.kind.violent() {
        steps.push(step(ActionKind::Attack, tt));
    }
    Some(Plan { goal: GoalKind::Contract, target: tt, steps, started_tick: now })
}

/// A sighting of `t` in the buyer's database younger than
/// `fresh_sighting_hours` (C15: the buyer's intel skips the asking).
fn buyer_intel(world: &World, c: &Contract, t: EntityId) -> Option<Intel> {
    let owner = c.buyer.unwrap_or(EntityId::NONE);
    let horizon = world.tick.saturating_sub(Tick::from(world.config.hunt.fresh_sighting_hours) * TICKS_PER_HOUR);
    world
        .db
        .get(&owner)?
        .sightings
        .iter()
        .filter(|s| s.who == t && s.tick >= horizon)
        .max_by_key(|s| s.tick)
        .map(|s| Intel { building: None, tile: s.tile, source: IntelSource::Sighting })
}

/// Inside `b`, or within a tile of its outside door.
fn at_building(world: &World, id: EntityId, b: EntityId) -> bool {
    world.comp::<Position>(id).is_some_and(|p| {
        p.building == Some(b)
            || world
                .comp::<Building>(b)
                .is_some_and(|bd| crate::systems::law::chebyshev(p.tile, world.outside_door(bd)) <= 1)
    })
}

/// The scripted steps' start check (`exec::start_step`'s scripted branch
/// for `GoalKind::Contract`, and `actions::can_start` for the two
/// contract steps): `Network` at an open Fixer, `Guard` at its client
/// while the record is taken; the chase steps through `hunt::can_start`.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    match kind {
        ActionKind::Network => target.is_some_and(|f| fixer_open(world, f) && at_building(world, id, f)),
        ActionKind::Guard => {
            let Some(r) = world.contract_runs.get(&id) else { return false };
            let taken = world.contracts.get(&r.contract).is_some_and(|c| c.status == ContractStatus::Taken);
            taken
                && target.is_some_and(|t| {
                    if world.has::<Building>(t) {
                        at_building(world, id, t)
                    } else {
                        crate::systems::law::living(world, t)
                            && (crate::systems::law::near(world, id, t, 3)
                                || r.intel.is_some_and(|i| match (i.building, world.comp::<Position>(id)) {
                                    (Some(b), _) => at_building(world, id, b),
                                    (None, Some(p)) => crate::systems::law::chebyshev(p.tile, i.tile) <= 1,
                                    _ => false,
                                }))
                    }
                })
        }
        // Phase 2 (C22): the mission's muster and brawl.
        ActionKind::Muster | ActionKind::Brawl => missions::can_start(world, id, kind),
        _ => crate::systems::hunt::can_start(world, id, kind, target),
    }
}

/// C10: `Network` completes: a regular of the Fixer from now (a gang
/// member's gang networked there too).
pub fn on_network(world: &mut World, id: EntityId, fixer: Option<EntityId>) -> StepResult {
    let Some(f) = fixer.filter(|&f| fixer_open(world, f)) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    let now = world.tick;
    let gang = world.gang_of(id);
    if let Some(k) = world.comp_mut::<Broker>(f) {
        k.regulars.insert(id, now);
        if let Some(g) = gang {
            k.gangs.insert(g);
        }
    }
    StepResult::Done
}

/// C32: a Guard post starts: the taker stands over its client.
pub fn on_guard_start(world: &mut World, id: EntityId, client: Option<EntityId>) {
    if let Some(c) = client {
        let list = world.contract_guards.entry(c).or_default();
        if !list.contains(&id) {
            list.push(id);
        }
    }
}

/// C32: a Guard post ends (done or abandoned).
pub fn on_guard_end(world: &mut World, id: EntityId, client: Option<EntityId>) {
    if let Some(c) = client {
        if let Some(list) = world.contract_guards.get_mut(&c) {
            list.retain(|g| *g != id);
            if list.is_empty() {
                world.contract_guards.remove(&c);
            }
        }
    }
}

/// C15: a Guard post completes: off post, the goal cooled until the next
/// day's post (once a day, taking to deadline).
pub fn on_guard_done(world: &mut World, id: EntityId, client: Option<EntityId>) -> StepResult {
    on_guard_end(world, id, client);
    let rest = (24u64.saturating_sub(u64::from(world.config.fixers.guard_hours))).max(1) * TICKS_PER_HOUR;
    let until = world.tick + rest;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.cooldowns.insert(GoalKind::Contract, until);
    }
    StepResult::Done
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
    if tod == 0 {
        daily(world);
    }
}

/// C17: queued live records start, by price (ties the lower id), while
/// runs are under `max_missions`.
fn start_queued(world: &mut World) {
    if live_count(world) >= world.config.missions.max_missions {
        return;
    }
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
    for (_, id) in queued {
        if !start_live(world, id) {
            break;
        }
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
        if live_count(world) >= world.config.missions.max_missions {
            return;
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
            (ContractKind::Guard, ContractStatus::Taken) => {
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
    prune_regulars(world);
    stat_regulars(world);
    hire_pass(world);
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
    match c.target {
        Target::Agent(a) => crate::systems::law::living(world, a),
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

// ---------------------------------------------------------------------------
// The day's snapshot (C38)
// ---------------------------------------------------------------------------

/// Review fix (probe): coins still held in escrow by records that are
/// neither Open nor Taken (M16b's Standing counts as live then); `settle`
/// empties every closing record, so this must read 0. `escrow_leak` cannot
/// see it (`escrow_held` moves with every record's escrow).
pub fn escrow_stuck(world: &World) -> i64 {
    world.contracts.values().filter(|c| !c.is_live()).map(|c| c.escrow).sum()
}

/// C38's snapshot columns, from `stats::snapshot`.
pub fn snapshot(world: &mut World) {
    let open = world.contracts.values().filter(|c| c.is_live()).count() as u32;
    let fixers: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Fixer).to_vec();
    let regulars: u32 = fixers.iter().filter_map(|&f| world.comp::<Broker>(f)).map(|k| k.regulars.len() as u32).sum();
    let mut parties: std::collections::BTreeSet<EntityId> = world.contract_runs.keys().copied().collect();
    parties.extend(world.contract_runs.values().filter_map(|r| world.has::<Identity>(r.target).then_some(r.target)));
    // Phase 2 (2.5): the missions' crews and targets.
    for (id, m) in &world.missions {
        parties.extend(m.crew.iter().copied());
        parties.extend(world.contracts.get(id).and_then(|c| c.target_agent()));
    }
    let queued = world
        .contracts
        .values()
        .filter(|c| c.status == ContractStatus::Taken && c.render == Render::Live)
        .filter(|c| !running(world, c))
        .count() as u32;
    let escrow: i64 = world.contracts.values().map(|c| c.escrow).sum();
    let held = world.escrow_held;
    let stuck = escrow_stuck(world);
    let mut heat = [0.0f32; crate::contract::FIXER_SLOTS];
    let mut income = [0i64; crate::contract::FIXER_SLOTS];
    for (i, &f) in fixers.iter().take(crate::contract::FIXER_SLOTS).enumerate() {
        if let Some(k) = world.comp::<Broker>(f) {
            heat[i] = k.heat;
            income[i] = k.income_today;
        }
    }
    let row = &mut world.stats.current.contract;
    row.contracts_open = open;
    row.regulars = regulars;
    row.live_parties = parties.len() as u32;
    row.live_queued = queued;
    row.escrow_held = held;
    row.escrow_leak = escrow - held;
    row.escrow_stuck = stuck;
    row.f_heat = heat;
    row.f_income = income;
}

// ---------------------------------------------------------------------------
// God commands (C39)
// ---------------------------------------------------------------------------

/// God `PostContract`: a record by `buyer` (`None` the city), the price
/// escrowed (brokered with the open Fixer nearest the target) or checked
/// (direct).
pub fn god_post(
    world: &mut World,
    buyer: Option<EntityId>,
    kind: ContractKind,
    target: Target,
    price: i64,
    brokered: bool,
    deadline_days: u16,
) -> Result<ContractId, String> {
    let broker = if brokered {
        let from = match target {
            Target::Agent(a) => world.comp::<Position>(a).map(|p| p.tile),
            Target::Building(b) => world.comp::<Building>(b).map(|bd| bd.door),
        }
        .ok_or("no such target")?;
        Some(nearest_fixer_to(world, from).ok_or("no open Fixer")?)
    } else {
        None
    };
    let agent = buyer.filter(|&b| world.has::<Identity>(b));
    let posting = Posting {
        buyer,
        agent,
        kind,
        target,
        broker,
        deadline_days: if deadline_days == 0 { world.config.contracts.deadline_days.get(kind) } else { deadline_days },
        origin: Origin::God,
        price: (price > 0).then_some(price),
    };
    post(world, posting).map_err(|r| format!("refused: {r:?}"))
}

/// God `TakeContract` (and M18's player call): `taker` takes the open
/// record now, past the matching (a living agent; gang and corp takers are
/// phase 2).
pub fn god_take(world: &mut World, id: ContractId, taker: EntityId) -> Result<(), String> {
    let c = world.contracts.get(&id).ok_or("no such contract")?;
    if !c.is_open() {
        return Err("the contract is not open".into());
    }
    // Phase 2 (C26): a gang takes it as a job.
    if world.has::<crate::components::Gang>(taker) {
        let c = c.clone();
        if !gang_eligible(world, &c, taker) {
            return Err("the gang cannot take it".into());
        }
        return if accept_gang(world, id, taker) { Ok(()) } else { Err("not accepted".into()) };
    }
    if !free_adult(world, taker) || c.target.id() == taker {
        return Err("the taker cannot take it".into());
    }
    if holds_taken(world, taker) {
        return Err("the taker holds a contract".into());
    }
    if accept(world, id, taker, &[]) {
        Ok(())
    } else {
        Err("not accepted".into())
    }
}

/// The record a body works (for the Inspector and tests).
pub fn run_of(world: &World, id: EntityId) -> Option<&ContractRun> {
    world.contract_runs.get(&id)
}

// ---------------------------------------------------------------------------
// Missions, squads, gang takers and sell-outs (phase 2: C22-C26)
// ---------------------------------------------------------------------------

/// Is `id` a gang (a faction taker)?
fn is_gang(world: &World, id: EntityId) -> bool {
    world.has::<crate::components::Gang>(id)
}

/// C22, C25, C26: a record worked as a mission: a gang's job, or a squad
/// (an agent taker with a crew).
fn is_mission(world: &World, c: &Contract) -> bool {
    c.taker.is_some_and(|t| is_gang(world, t)) || !c.crew.is_empty()
}

/// A Taken live record already being worked (a run or a mission).
fn running(world: &World, c: &Contract) -> bool {
    world.missions.contains_key(&c.id)
        || c.taker.is_some_and(|t| world.contract_runs.get(&t).is_some_and(|r| r.contract == c.id))
}

/// The people on a record: an agent taker and the crew (`hits_squad`
/// counts fulfilled Hits with two or more, C25).
fn squad_size(world: &World, c: &Contract) -> usize {
    usize::from(c.taker.is_some_and(|t| !is_gang(world, t))) + c.crew.len()
}

/// C26: the record a gang holds as its job (`Taken`, the gang the taker).
pub fn job_of(world: &World, gang: EntityId) -> Option<ContractId> {
    world
        .by_party
        .get(&gang)?
        .iter()
        .copied()
        .find(|id| world.contracts.get(id).is_some_and(|c| c.status == ContractStatus::Taken && c.taker == Some(gang)))
}

/// C26 (review fix): the gang's job is marching (its mission open): the
/// window `faction::rescore` holds `Order::Job` in (a job queued past
/// `max_missions` does not freeze the gang's order).
pub fn job_live(world: &World, gang: EntityId) -> bool {
    !world.missions.is_empty() && job_of(world, gang).is_some_and(|id| world.missions.contains_key(&id))
}

/// May `m` march on record `c` as crew: eligible as a taker would be (C12),
/// not in a live riot, not on another mission, and not of a gang mustering
/// its own raid.
fn crew_ok(world: &World, c: &Contract, m: EntityId) -> bool {
    eligible(world, c, m)
        && !world.rioter_of.contains_key(&m)
        && !world.mission_of.contains_key(&m)
        && world.gang_of(m).and_then(|g| world.comp::<crate::components::Gang>(g)).is_none_or(|g| g.raid_at.is_none())
}

/// C25: a weak gun's crew. When the solo estimate is below `squad_below`:
/// up to `crew_max − 1` of the taker's gang members who may march (else, a
/// gangless taker or none fit, its Friends who are regulars of the
/// record's Fixer), strongest first (ties the lower id), stopping once the
/// estimate clears `squad_below`. Empty for a strong gun, a Guard or a
/// Locate.
pub fn squad_crew(world: &World, c: &Contract, taker: EntityId) -> SmallVec<[EntityId; 4]> {
    let mut crew: SmallVec<[EntityId; 4]> = SmallVec::new();
    let below = world.config.missions.squad_below;
    if !c.kind.violent() || missions::estimate(world, &[taker], &c.target) >= below {
        return crew;
    }
    let ok = |m: EntityId| m != taker && crew_ok(world, c, m);
    let mut pool: Vec<EntityId> = world
        .gang_of(taker)
        .and_then(|g| world.comp::<crate::components::Gang>(g))
        .map(|g| g.members.iter().copied().filter(|&m| ok(m)).collect())
        .unwrap_or_default();
    if pool.is_empty() {
        let regulars = c.broker.and_then(|b| world.comp::<Broker>(b));
        pool = world
            .neighbours(taker)
            .filter(|&o| world.edge(taker, o).is_some_and(|e| e.kind == RelKind::Friend))
            .filter(|o| regulars.is_some_and(|k| k.regulars.contains_key(o)))
            .filter(|&o| ok(o))
            .collect();
    }
    crate::systems::raid::by_strength(world, &mut pool);
    let max = usize::from(world.config.fixers.crew_max.saturating_sub(1));
    let mut team: Vec<EntityId> = vec![taker];
    for m in pool {
        if crew.len() >= max {
            break;
        }
        crew.push(m);
        team.push(m);
        if missions::estimate(world, &team, &c.target) >= below {
            break;
        }
    }
    crew
}

/// C26: a gang's crew for a job: its fittest members who may march (C12's
/// eligibility), strongest first, up to `crew_max`, the leader only if
/// fewer than two others.
pub fn job_crew(world: &World, c: &Contract, gang: EntityId) -> SmallVec<[EntityId; 4]> {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return SmallVec::new() };
    let leader = g.leader;
    let mut pool: Vec<EntityId> =
        g.members.iter().copied().filter(|&m| Some(m) != leader && crew_ok(world, c, m)).collect();
    crate::systems::raid::by_strength(world, &mut pool);
    let max = usize::from(world.config.fixers.crew_max.max(1));
    let mut crew: SmallVec<[EntityId; 4]> = pool.into_iter().take(max).collect();
    if crew.len() < 2 && crew.len() < max {
        if let Some(l) = leader.filter(|&l| crew_ok(world, c, l)) {
            crew.push(l);
        }
    }
    crew
}

/// C26: may `gang` take record `c` as a job: a Hit or a Beat, not the
/// buyer, not the target's own gang, a free leader, no job, no raid
/// mustering, no god strike pinned, and a crew to send.
pub fn gang_eligible(world: &World, c: &Contract, gang: EntityId) -> bool {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return false };
    let now = world.tick;
    let t = c.target.id();
    // Review fix: the leader who accepts is held to `eligible`'s
    // relationship exclusions (no job on its Spouse, Family, Parent or
    // Friend), as the crew are.
    let kin = |l: EntityId| {
        l == t
            || Some(l) == c.buyer
            || Some(l) == c.agent
            || world.edge(l, t).is_some_and(|e| {
                matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend)
            })
    };
    c.kind.violent()
        && c.buyer != Some(gang)
        && !crate::systems::grudges::member_of(world, t, gang)
        && g.leader.is_some_and(|l| !world.has::<Sentence>(l) && !kin(l))
        && job_of(world, gang).is_none()
        && g.raid_at.is_none()
        && g.strike.is_none_or(|(_, until)| until <= now)
        && !job_crew(world, c, gang).is_empty()
}

/// C26: a gang's leader takes a job: `greed` → Linear{0.6, 0.4} × `1 −
/// lawfulness` → Linear{0.5, 0.5} × `Regard(gang, buyer's faction).fear` →
/// Linear{0.5, 0.5} ≥ `accept_min` (the spec's sell-out rule, also the
/// matched gang's: one rule for a gang's acceptance).
pub fn leader_accepts(world: &World, gang: EntityId, buyer_faction: EntityId) -> bool {
    let Some(p) = world
        .comp::<crate::components::Gang>(gang)
        .and_then(|g| g.leader)
        .filter(|&l| !world.has::<Sentence>(l))
        .and_then(|l| world.comp::<Personality>(l))
    else {
        return false;
    };
    let fear = crate::systems::reputation::regard(world, gang, buyer_faction).fear;
    let score = Consideration::new("greed", p.greed, Curve::Linear { m: 0.6, b: 0.4 }).output
        * Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }).output
        * Consideration::new("fear of the buyer", fear, Curve::Linear { m: 0.5, b: 0.5 }).output;
    score >= world.config.contracts.accept_min
}

/// C26: `gang` takes Open record `id` as a job: its crew (`job_crew`), the
/// record Taken and live, the mission opened (or queued); the gang's brain
/// pins `Order::Job` once the mission opens (`open_mission`).
pub fn accept_gang(world: &mut World, id: ContractId, gang: EntityId) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let crew = job_crew(world, &c, gang);
    !crew.is_empty() && accept(world, id, gang, &crew)
}

/// C22: the door a mission marches to at `at`: the freshest intel (the
/// taker's or its first crew member's own and gang's sightings, then the
/// buyer's database), else the target's habit at `at`; a building's door
/// is the street tile outside it.
pub fn mission_door(world: &World, id: ContractId, at: Tick) -> (TilePos, Option<EntityId>) {
    let Some(c) = world.contracts.get(&id) else { return (TilePos::default(), None) };
    let Some(t) = c.target_agent() else { return (TilePos::default(), None) };
    let lead = c.taker.filter(|&x| !is_gang(world, x)).or_else(|| c.crew.first().copied());
    let intel = lead
        .and_then(|l| crate::systems::hunt::fresh_intel(world, l, t))
        .or_else(|| buyer_intel(world, c, t))
        .unwrap_or_else(|| crate::systems::hunt::habit(world, t, at));
    match intel.building.and_then(|b| missions::door_of(world, b).map(|d| (d, b))) {
        Some((d, b)) => (d, Some(b)),
        None => (intel.tile, None),
    }
}

/// C22: where a mission gathers: outside the taker's gang's held Home
/// nearest the door within `muster_near_tiles`, else outside the record's
/// Fixer, else outside the taker's Home (a gang's Hideout), else where the
/// lead stands.
fn mission_muster(world: &World, c: &Contract, door: TilePos) -> TilePos {
    let near = world.config.gangs.muster_near_tiles;
    if let Some(g) = missions::taker_gang(world, c).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        let best = g
            .territory
            .iter()
            .filter_map(|&h| {
                world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(door), h, b))
            })
            .filter(|&(d, _, _)| d <= near)
            .min_by_key(|&(d, h, _)| (d, h));
        if let Some((_, _, b)) = best {
            return world.outside_door(b);
        }
    }
    if let Some(d) = c.broker.and_then(|b| missions::door_of(world, b)) {
        return d;
    }
    let taker = c.taker.unwrap_or(EntityId::NONE);
    let home = if is_gang(world, taker) { world.hideout_of(taker) } else { missions::home_of(world, taker) };
    if let Some(d) = home.and_then(|h| missions::door_of(world, h)) {
        return d;
    }
    let lead = c.crew.first().copied().unwrap_or(taker);
    world.comp::<Position>(lead).map_or(door, |p| p.tile)
}

/// C22: open record `id`'s mission (a squad's or a gang's): the crew (an
/// agent taker first), the muster, the door, `raid_at` at the next
/// `raid_muster_hour`, `mission_of`, the record live with a fresh strike;
/// the crew promoted to bodies. `false` when `max_missions` live records
/// are going (it stays queued, C17).
pub fn open_mission(world: &mut World, id: ContractId) -> bool {
    if world.missions.contains_key(&id) {
        return true;
    }
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    if c.status != ContractStatus::Taken || c.target_agent().is_none() {
        return false;
    }
    if live_count(world) >= world.config.missions.max_missions {
        return false;
    }
    let now = world.tick;
    let mut crew: SmallVec<[EntityId; 6]> = SmallVec::new();
    crew.extend(c.taker.filter(|&t| !is_gang(world, t)));
    crew.extend(c.crew.iter().copied());
    crew.retain(|a| crate::systems::law::living(world, *a));
    if crew.is_empty() {
        return false;
    }
    let raid_at = crate::systems::faction::next_muster(now, world.config.gangs.raid_muster_hour);
    let (door, door_building) = mission_door(world, id, raid_at);
    let muster = mission_muster(world, &c, door);
    let hint = missions::expected_defenders(world, &c.target).len().saturating_sub(1).min(255) as u8;
    for &a in &crew {
        world.mission_of.insert(a, id);
    }
    world.missions.insert(
        id,
        crate::contract::Mission {
            contract: id,
            crew: crew.clone(),
            muster,
            door,
            door_building,
            raid_at,
            stream_by: None,
            defenders_hint: hint,
        },
    );
    if let Some(x) = world.contracts.get_mut(&id) {
        x.render = Render::Live;
        x.due = None;
        x.strike = None;
        x.holds = 0;
    }
    if let Some(d) = c.due {
        world.ledger_due.remove(&(d, id));
    }
    for &a in &crew {
        if world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical) {
            crate::systems::lod::set_lod(world, a, Lod::Coarse);
        }
    }
    // C26: a gang's job marches: its brain pins `Order::Job` at once (at
    // acceptance, a sell-out, or when a queued job finds room).
    if let Some(g) = c.taker.filter(|&t| is_gang(world, t)) {
        crate::systems::faction::rethink(world, g);
    }
    true
}

/// C22: a mission ends (its record left Taken, failed an attempt or was
/// sold out): the crew released, `mission_of` cleaned, their Contract plans
/// dropped.
pub fn end_mission(world: &mut World, id: ContractId) {
    let Some(m) = world.missions.remove(&id) else { return };
    for &a in &m.crew {
        if world.mission_of.get(&a) == Some(&id) {
            world.mission_of.remove(&a);
        }
        if world.comp::<Brain>(a).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
            world.abort_plan(a);
        }
    }
}

/// C22: `who` leaves record `id`'s mission (dead, jailed, cuffed, gone):
/// out of the crew and the record (an agent taker's place goes to the
/// first of the crew), no longer a party; the last one out fails the
/// attempt.
pub fn drop_crew(world: &mut World, id: ContractId, who: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if let Some(m) = world.missions.get_mut(&id) {
        m.crew.retain(|a| *a != who);
    }
    if world.mission_of.get(&who) == Some(&id) {
        world.mission_of.remove(&who);
    }
    if world.comp::<Brain>(who).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
        world.abort_plan(who);
    }
    let empty = world.missions.get(&id).is_none_or(|m| m.crew.is_empty());
    if empty {
        fail_attempt(world, id, "the crew is gone");
        return;
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.crew.retain(|a| *a != who);
        if x.taker == Some(who) {
            x.taker = if x.crew.is_empty() { None } else { Some(x.crew.remove(0)) };
        }
    }
    let still = Some(who) == c.buyer || Some(who) == c.agent || who == c.target.id();
    if !still {
        if let Some(list) = world.by_party.get_mut(&who) {
            list.retain(|x| *x != id);
            if list.is_empty() {
                world.by_party.remove(&who);
            }
        }
    }
}

/// C26: would `gang` (the door district's controller) take record `id`
/// over: a Taken Hit or Beat whose target is not its member, the gang's
/// `Regard` of the target's faction (its gang, else its corp, else the
/// target) below `sell_regard`, the gang not its taker or buyer and free to
/// take a job, its leader accepting (`leader_accepts`). The buyer's purse
/// is `sell_out`'s check (a short buyer lapses the offer).
pub fn can_sell_out(world: &World, id: ContractId, gang: EntityId) -> bool {
    let Some(c) = world.contracts.get(&id) else { return false };
    let Some(t) = c.target_agent() else { return false };
    if c.status != ContractStatus::Taken || c.taker == Some(gang) || missions::taker_gang(world, c) == Some(gang) {
        return false;
    }
    let tf = world.gang_of(t).or_else(|| world.corp_of_agent(t)).unwrap_or(t);
    gang_eligible(world, c, gang)
        && crate::systems::reputation::regard(world, gang, tf).value < world.config.missions.sell_regard
        && leader_accepts(world, gang, missions::buyer_faction(world, c))
}

/// C26: sell record `id` out to `gang` (the same record, one id): the price
/// raised by `sell_premium` (a brokered buyer tops up the escrow, a direct
/// buyer's purse is checked; short, the offer lapses: `false`), the old
/// taker and crew released, the gang the taker with its crew, `sold_out`,
/// `SoldOut`, the mission opened (or queued) and the gang's brain rethinks.
pub fn sell_out(world: &mut World, id: ContractId, gang: EntityId) -> bool {
    if !can_sell_out(world, id, gang) {
        return false;
    }
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let new_price = ((c.price as f32) * world.config.missions.sell_premium).round().max(c.price as f32) as i64;
    let top = new_price - c.price;
    if c.brokered() {
        // Review fix: a short top-up returns only what it took (the record's
        // own escrow stays for its next taker) and the offer lapses.
        let moved = if top > 0 { ownership::escrow_in(world, c.buyer, id, top) } else { 0 };
        if moved < top {
            if moved > 0 {
                ownership::escrow_out(world, id, refund_to(world, c.buyer), moved, Flow::Escrow);
            }
            return false;
        }
    } else if world.purse(c.buyer) < new_price {
        return false;
    }
    let crew = job_crew(world, &c, gang);
    let prev = c.taker;
    if let Some(t) = prev {
        end_run(world, t, id);
    }
    for &m in &c.crew {
        end_run(world, m, id);
    }
    end_mission(world, id);
    if let Some(d) = c.due {
        world.ledger_due.remove(&(d, id));
    }
    for p in prev.into_iter().chain(c.crew.iter().copied()) {
        let still = Some(p) == c.buyer || Some(p) == c.agent || p == c.target.id();
        if still {
            continue;
        }
        if let Some(list) = world.by_party.get_mut(&p) {
            list.retain(|x| *x != id);
            if list.is_empty() {
                world.by_party.remove(&p);
            }
        }
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.taker = Some(gang);
        x.crew = crew.clone();
        x.price = new_price;
        x.terms = Terms::Pay { price: new_price };
        x.sold_out = true;
        x.holds = 0;
        x.due = None;
        x.render = Render::Live;
    }
    index_parties(world, id);
    for &m in &crew {
        hear_hired(world, id, m);
    }
    world.stats.current.contract.sold_out += 1;
    let t = c.target.id();
    let text = format!(
        "{} took over the job on {} from {}",
        world.owner_label(Some(gang)),
        world.name_of(t),
        world.owner_label(prev)
    );
    world.push_event(EventKind::SoldOut, &[gang, t, prev.unwrap_or(EntityId::NONE)], text.clone());
    log(world, id, text);
    start_live(world, id);
    true
}

/// C24, from `StakeOut`'s contact (exec): a live solo Hit or Beat decides
/// its strike there (the taker against the target's expected defenders,
/// the target's district's controller). `true` strikes (the `Attack`
/// follows; also for anything that is not a contract strike); a Hold sends
/// the run back to asking and cools the goal a day; a SellOut or a Fail
/// ends this attempt. No draw.
pub fn strike_at_contact(world: &mut World, taker: EntityId) -> bool {
    if world.contract_runs.is_empty() {
        return true;
    }
    let Some(r) = world.contract_runs.get(&taker).cloned() else { return true };
    let violent = world
        .contracts
        .get(&r.contract)
        .is_some_and(|c| c.status == ContractStatus::Taken && c.kind.violent() && c.target_agent().is_some());
    if !violent {
        return true;
    }
    let door = world
        .comp::<Position>(r.target)
        .or_else(|| world.comp::<Position>(taker))
        .map_or_else(TilePos::default, |p| p.tile);
    match missions::decide(world, r.contract, &[taker], door) {
        crate::contract::Decision::Strike => true,
        crate::contract::Decision::Hold => {
            if let Some(mut x) = crate::systems::hunt::chase(world, taker) {
                x.phase = HuntPhase::Ask;
                x.intel = None;
                x.venue = None;
                x.stakeout_until = None;
                crate::systems::hunt::set_chase(world, taker, x);
                crate::systems::hunt::reindex(world);
            }
            let until = missions::hold_until(world.tick);
            if let Some(b) = world.comp_mut::<Brain>(taker) {
                b.cooldowns.insert(GoalKind::Contract, until);
            }
            false
        }
        _ => false,
    }
}
