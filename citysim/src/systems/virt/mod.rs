//! M14 the Virt plane (docs/M14_VIRT.md § 1, § 3 ICE; plan phase 1.4, V1-V4,
//! V25-V27, V47): relink, the indices, the seeded Labs and ICE, effective
//! ICE, the value at risk, installs and holdings.
//!
//! The plane is a game abstraction: abstract nodes and links over the city
//! map, where a fictional runner's attempt on a node is a seeded dice
//! contest (phase 2). Nothing here runs per agent per tick: [`run`] is one
//! flag test a tick (relink when a hook marked the plane dirty); everything
//! else is daily (`tech::run`) or event-driven. Every pass iterates nodes in
//! `NodeId` order and buildings and corps ascending.

use std::collections::BTreeMap;
use std::sync::Arc;

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{
    Asset, AssetKind, AssetLoc, Brain, Building, BuildingKind, Corp, DistrictId, Gang, GoalKind, Kit, Niche,
    Personality, Position, Skills, TilePos,
};
use crate::config::IceCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::corps::fleet_reserve;
use crate::systems::districts::Seat;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::virt::{
    DataStore, Link, LinkKind, Node, NodeId, NodeKind, OwnerTag, Portal, Purpose, Route, RouteKey, RouteTree, Run,
    RunId, RunMode, RunOrder, RunOutcome, RunPhase, RunWhy, SecurityProfile, Track,
};
use crate::world::World;

mod gangs;
mod hack;
mod ice;
mod routes;
mod runs;
mod security;

pub use gangs::*;
pub use hack::*;
pub use ice::*;
pub use routes::*;
pub use runs::*;
pub use security::*;

/// Per tick: relink when a hook marked the plane dirty, then pop the run
/// steps due (V10; nothing else per tick).
pub fn run(world: &mut World) {
    if world.virt_dirty {
        relink(world);
    }
    pop_due(world);
}

/// Mark the plane for a relink (plan V4's hooks). Cheap and side-effect
/// free: with the plane off nothing reads it.
pub fn mark_dirty(world: &mut World) {
    world.virt_dirty = true;
}

// ---------------------------------------------------------------------------
// Relink (plan V1-V4)
// ---------------------------------------------------------------------------

/// The owner a building node stands for: the gang whose Hideout it is,
/// else the building's owner.
fn building_owner(world: &World, b: EntityId, hideouts: &BTreeMap<EntityId, EntityId>) -> Option<EntityId> {
    hideouts.get(&b).copied().or_else(|| world.owner_of(b))
}

fn tag_of(world: &World, owner: Option<EntityId>) -> OwnerTag {
    match owner {
        Some(o) if world.has::<Corp>(o) => OwnerTag::Corp,
        Some(o) if world.has::<Gang>(o) => OwnerTag::Gang,
        _ => OwnerTag::City,
    }
}

/// Plan V2: does building `b` get its own node? A Lab, a gang's Hideout,
/// or a corp- or gang-owned standing building that is not a Block or a Lot
/// (the Precinct is added on its own).
fn wants_node(world: &World, b: EntityId, bd: &Building, hideouts: &BTreeMap<EntityId, EntityId>) -> bool {
    if bd.demolished || bd.derelict || matches!(bd.kind, BuildingKind::Home | BuildingKind::Lot | BuildingKind::Jail) {
        return false;
    }
    if bd.kind == BuildingKind::Lab || hideouts.contains_key(&b) {
        return true;
    }
    bd.owner.is_some_and(|o| world.has::<Corp>(o) || world.has::<Gang>(o))
}

/// Plan V3: a Ledger's position: the owner's building with the highest
/// `ownership::value` (ties lower id), else the Hall's door.
fn ledger_pos(world: &World, corp: EntityId, hall_door: TilePos) -> TilePos {
    let Some(c) = world.comp::<Corp>(corp) else { return hall_door };
    c.buildings
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (b, bd)))
        .map(|(b, bd)| (std::cmp::Reverse(ownership::value(world, bd.kind)), b, bd.door))
        .min()
        .map_or(hall_door, |(_, _, door)| door)
}

/// One wanted node: kind, owner, position.
type Wanted = (NodeKind, Option<EntityId>, TilePos);

/// The node set the world wants now, in first-build order (plan V1):
/// Public per district, the Treasury, the Precinct, corp Ledgers by corp
/// id, then building nodes by building id.
fn wanted(world: &World) -> Vec<Wanted> {
    let mut out = Vec::new();
    for d in &world.districts {
        out.push((NodeKind::Public(d.id), None, d.centroid));
    }
    let hall = world.building_of_kind(BuildingKind::Hall);
    let hall_door = hall.and_then(|h| world.comp::<Building>(h)).map(|b| b.door).unwrap_or_default();
    if let Some(h) = hall {
        out.push((NodeKind::Ledger(h), None, hall_door));
    }
    if let Some(j) = world.building_of_kind(BuildingKind::Jail) {
        let door = world.comp::<Building>(j).map(|b| b.door).unwrap_or_default();
        out.push((NodeKind::Building(j), None, door));
    }
    for c in world.corps() {
        out.push((NodeKind::Ledger(c), Some(c), ledger_pos(world, c, hall_door)));
    }
    let hideouts: BTreeMap<EntityId, EntityId> =
        world.gang_list().iter().filter_map(|&g| world.comp::<Gang>(g).map(|x| (x.hideout, g))).collect();
    let mut buildings: Vec<EntityId> = world.buildings_by_kind().values().flatten().copied().collect();
    buildings.sort_unstable();
    for b in buildings {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if wants_node(world, b, bd, &hideouts) {
            out.push((NodeKind::Building(b), building_owner(world, b, &hideouts), bd.door));
        }
    }
    // M17 phase 4: each living megacorp parent's uplink, at its first corp
    // branch's Ledger (no owner: the parent's Data is no city holding).
    out.extend(
        crate::systems::parents::wanted_uplinks(world)
            .into_iter()
            .map(|(id, branch)| (NodeKind::Uplink(id), None, ledger_pos(world, branch, hall_door))),
    );
    out
}

/// V1-V4: rebuild the node set and the links from the world. Surviving
/// nodes keep their id and state (ICE lives on the building or the corp,
/// stores on the node); a node no longer wanted is marked dead, a wanted
/// one with no node is appended; a node whose owner changed drops its
/// breaches and alarm. Links are rebuilt from scratch. Bumps `epoch`.
pub fn relink(world: &mut World) {
    let want = wanted(world);
    let mut index: BTreeMap<NodeKind, NodeId> = BTreeMap::new();
    for (i, n) in world.virt.nodes.iter().enumerate() {
        index.insert(n.kind, NodeId(i as u16));
    }
    let mut keep = vec![false; world.virt.nodes.len()];
    let tags: Vec<OwnerTag> = want.iter().map(|&(_, o, _)| tag_of(world, o)).collect();
    for (&(kind, owner, pos), &tag) in want.iter().zip(&tags) {
        match index.get(&kind) {
            Some(&id) => {
                keep[id.index()] = true;
                let n = &mut world.virt.nodes[id.index()];
                if n.owner != owner {
                    n.breaches.clear();
                    n.alarm_until = None;
                }
                n.owner = owner;
                n.owner_kind = tag;
                n.pos = pos;
                n.alive = true;
            }
            None => {
                debug_assert!(world.virt.nodes.len() < usize::from(u16::MAX), "the plane is sized for 250 nodes");
                let id = NodeId(world.virt.nodes.len() as u16);
                index.insert(kind, id);
                keep.push(true);
                world.virt.nodes.push(Node {
                    kind,
                    owner,
                    owner_kind: tag,
                    alive: true,
                    pos,
                    alarm_until: None,
                    hacked: None,
                    store: DataStore::default(),
                    breaches: Default::default(),
                });
            }
        }
    }
    for (i, k) in keep.iter().enumerate() {
        if !k {
            world.virt.nodes[i].alive = false;
        }
    }
    world.virt.links = links(world);
    rebuild_index(world);
    bump_epoch(world);
    world.virt_dirty = false;
}

/// Plan V3: Street between adjacent districts' Public nodes, Access from
/// every building node to its district's Public node, Trunk from each corp
/// building node to its owner's Ledger and to each of its owner's Labs, the
/// Precinct to the Treasury. A corp with no building node of its own (a
/// landlord of Blocks only) gets one Trunk from its Ledger to the Public
/// node of its Ledger's district (plan deviation: its Blocks ride on that
/// node, and every Ledger stays reachable through a Trunk only).
fn links(world: &World) -> Vec<Link> {
    let cfg = &world.config.virt;
    let nodes = &world.virt.nodes;
    let mut public: BTreeMap<DistrictId, NodeId> = BTreeMap::new();
    let mut ledger: BTreeMap<EntityId, NodeId> = BTreeMap::new();
    let mut labs: BTreeMap<EntityId, Vec<NodeId>> = BTreeMap::new();
    for (i, n) in nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        let id = NodeId(i as u16);
        match n.kind {
            NodeKind::Public(d) => {
                public.insert(d, id);
            }
            NodeKind::Ledger(c) => {
                ledger.insert(c, id);
            }
            NodeKind::Building(b) => {
                let lab = world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lab);
                if let (true, Some(o)) = (lab, n.owner) {
                    labs.entry(o).or_default().push(id);
                }
            }
            NodeKind::Uplink(_) => {}
        }
    }
    let link = |a: NodeId, b: NodeId, kind: LinkKind, tier: u8| {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        Link { a, b, kind, tier, firewall: 0 }
    };
    let mut out = Vec::new();
    // Street: a < b once.
    for (&da, &na) in &public {
        let bits = world.district_adjacent.get(da.index()).copied().unwrap_or(0);
        for (&db, &nb) in public.range(DistrictId(da.0.saturating_add(1))..) {
            if db.index() < crate::components::MAX_DISTRICTS && bits & (1 << db.index()) != 0 {
                out.push(link(na, nb, LinkKind::Street, cfg.public_links_tier));
            }
        }
    }
    let jail = world.building_of_kind(BuildingKind::Jail);
    let hall = world.building_of_kind(BuildingKind::Hall);
    let mut trunked: std::collections::BTreeSet<EntityId> = Default::default();
    for (i, n) in nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        let id = NodeId(i as u16);
        let NodeKind::Building(b) = n.kind else { continue };
        if let Some(&p) = public.get(&world.district_of_building(b)) {
            out.push(link(id, p, LinkKind::Access, cfg.access_links_tier));
        }
        if Some(b) == jail {
            if let Some(&t) = hall.and_then(|h| ledger.get(&h)) {
                out.push(link(id, t, LinkKind::Trunk, cfg.trunk_links_tier));
            }
            continue;
        }
        let Some(owner) = n.owner.filter(|_| n.owner_kind == OwnerTag::Corp) else { continue };
        if let Some(&l) = ledger.get(&owner) {
            out.push(link(id, l, LinkKind::Trunk, cfg.trunk_links_tier));
            trunked.insert(owner);
        }
        let is_lab = world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lab);
        for &lab in labs.get(&owner).map_or(&[][..], |v| v.as_slice()) {
            // A Lab to a Lab once (the lower id drives).
            if lab != id && (!is_lab || id < lab) {
                out.push(link(id, lab, LinkKind::Trunk, cfg.trunk_links_tier));
            }
        }
    }
    for (&c, &l) in &ledger {
        if Some(c) == hall || trunked.contains(&c) || !world.has::<Corp>(c) {
            continue;
        }
        let pos = nodes[l.index()].pos;
        if let Some(&p) = public.get(&world.district_of(pos)) {
            out.push(link(l, p, LinkKind::Trunk, cfg.trunk_links_tier));
        }
    }
    // M17 phase 4: an uplink's one Trunk, from its branch's Ledger.
    let tier = world.config.outside.parents.uplink_link_tier;
    for (i, n) in nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        let NodeKind::Uplink(p) = n.kind else { continue };
        let branch = crate::systems::parents::wanted_uplinks(world).into_iter().find(|&(x, _)| x == p).map(|(_, b)| b);
        if let Some(&l) = branch.and_then(|b| ledger.get(&b)) {
            out.push(link(NodeId(i as u16), l, LinkKind::Trunk, tier));
        }
    }
    out.sort_by_key(|k| (k.a, k.b, k.kind as u8));
    out
}

/// The derived indices (`adj`, `of_building`, `ledger_of`, `public_of`)
/// from `nodes` and `links`. Called by every relink and from
/// `World::rebuild_indices` on load.
pub fn rebuild_index(world: &mut World) {
    let p = &mut world.virt;
    p.adj = vec![SmallVec::new(); p.nodes.len()];
    p.of_building.clear();
    p.ledger_of.clear();
    p.public_of.clear();
    for (i, n) in p.nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        let id = NodeId(i as u16);
        match n.kind {
            NodeKind::Public(d) => {
                if p.public_of.len() <= d.index() {
                    p.public_of.resize(d.index() + 1, NodeId::default());
                }
                p.public_of[d.index()] = id;
            }
            NodeKind::Building(b) => {
                p.of_building.insert(b, id);
            }
            NodeKind::Ledger(c) => {
                p.ledger_of.insert(c, id);
            }
            NodeKind::Uplink(_) => {}
        }
    }
    for (k, l) in p.links.iter().enumerate() {
        let alive = |n: NodeId| p.nodes.get(n.index()).is_some_and(|x| x.alive);
        if !(alive(l.a) && alive(l.b)) {
            continue;
        }
        p.adj[l.a.index()].push((l.b, k as u16));
        p.adj[l.b.index()].push((l.a, k as u16));
    }
    p.cache.clear();
}

/// Any ICE, alarm or hack write, a relink, a tier change: route trees go stale.
pub fn bump_epoch(world: &mut World) {
    world.virt.epoch += 1;
    world.virt.cache.clear();
}

/// V38, daily in `tech::run`: Statistical agents carrying a deck, on
/// `(index + day) % 7 == 0`, ascending, score the Hack goal with the
/// freelance considerations; a score at `stat_hack_min` writes a
/// `RunOrder { why: Stat }` and promotes them to Coarse.
pub fn stat_pass(world: &mut World) {
    let day = world.day();
    let mut who: Vec<EntityId> = crate::systems::assets::all_assets(world)
        .into_iter()
        .filter_map(|a| world.comp::<Asset>(a).filter(|x| x.kind == AssetKind::Deck).map(|x| x.loc))
        .filter_map(|loc| match loc {
            AssetLoc::Carried(h) => Some(h),
            _ => None,
        })
        .filter(|h| (u64::from(h.index) + day).is_multiple_of(7))
        .filter(|&h| world.comp::<Brain>(h).is_some_and(|b| b.lod == crate::components::Lod::Statistical))
        // L2 (L21): no run from the cells.
        .filter(|&h| !world.has::<crate::components::Sentence>(h))
        .collect();
    who.sort_unstable();
    who.dedup();
    let min = world.config.hack.stat_hack_min;
    for id in who {
        if world.run_orders.contains_key(&id) {
            continue;
        }
        let Some(offer) = hack_offer(world, id, true).filter(|o| !o.sell) else { continue };
        let score = offer.considerations.iter().map(|c| c.output).product::<f32>() + world.config.hack.hack_flat;
        if score < min {
            continue;
        }
        let Some(portal) = offer.portal else { continue };
        let now = crate::systems::midnight::stamp(world);
        let order = RunOrder {
            patron: None,
            purpose: offer.purpose,
            target: offer.target,
            chair: portal.building,
            not_before: now,
            expires: now + TICKS_PER_DAY,
            why: RunWhy::Stat,
            mode: offer.mode,
        };
        give_order(world, id, order);
    }
}
