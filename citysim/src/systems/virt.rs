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
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::virt::{
    DataStore, Link, LinkKind, Node, NodeId, NodeKind, OwnerTag, Portal, Purpose, Route, RouteKey, RouteTree, Run,
    RunId, RunMode, RunOrder, RunOutcome, RunPhase, RunWhy, SecurityProfile, Track,
};
use crate::world::World;

/// Is the plane on (`[virt] enabled`)? Every M14 branch starts here.
pub fn enabled(world: &World) -> bool {
    world.config.virt.enabled
}

/// Per tick: relink when a hook marked the plane dirty, then pop the run
/// steps due (V10; nothing else per tick).
pub fn run(world: &mut World) {
    if !enabled(world) {
        return;
    }
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
    let mut buildings: Vec<EntityId> = world.buildings_by_kind.values().flatten().copied().collect();
    buildings.sort_unstable();
    for b in buildings {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if wants_node(world, b, bd, &hideouts) {
            out.push((NodeKind::Building(b), building_owner(world, b, &hideouts), bd.door));
        }
    }
    out
}

/// V1-V4: rebuild the node set and the links from the world. Surviving
/// nodes keep their id and state (ICE lives on the building or the corp,
/// stores on the node); a node no longer wanted is marked dead, a wanted
/// one with no node is appended; a node whose owner changed drops its
/// breaches and alarm. Links are rebuilt from scratch. Bumps `epoch`.
pub fn relink(world: &mut World) {
    if !enabled(world) {
        world.virt_dirty = false;
        return;
    }
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
            if db.index() < 16 && bits & (1 << db.index()) != 0 {
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

/// V44: a save from before M14 (enabled, no nodes): relink and seed the
/// ICE. No Labs (the Research order builds them), empty stores, no runs.
pub fn migrate(world: &mut World) {
    if enabled(world) && world.virt.nodes.is_empty() {
        relink(world);
        seed_ice(world);
    }
}

// ---------------------------------------------------------------------------
// Seeding (the plan's Seeding table, V26, V47)
// ---------------------------------------------------------------------------

/// The seeded Labs: owner corp by name, district by name, focus.
const SEED_LABS: [(&str, &str, Track); 4] = [
    ("Zetatech", "Spire", Track::Chrome),
    ("Zetatech", "Civic", Track::Industry),
    ("Arasaka", "Vats", Track::Deck),
    ("Militech", "Mid West", Track::Deck),
];

/// Last in `World::new`, after `assets::seed_sellers`: each row's Lab on
/// the vacant Lot of its district nearest the centroid (Lot door
/// Manhattan, ties lower id), owned by the row's corp, `focus` set, four
/// Researcher vacancies posted (`build_on_lot`), `seed_lab_grant` to the
/// owner out of nothing (V47), then a relink and `seed_store` Data in the
/// focus track. Returns the Labs built, in row order. A no-op with the
/// plane off.
pub fn seed_labs(world: &mut World) -> Vec<EntityId> {
    let mut built = Vec::new();
    if !enabled(world) {
        return built;
    }
    for (corp_name, district, focus) in SEED_LABS {
        let Some(corp) =
            world.corps().into_iter().find(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.name == corp_name))
        else {
            continue;
        };
        let Some((d, centroid)) = world.districts.iter().find(|x| x.name == district).map(|x| (x.id, x.centroid))
        else {
            continue;
        };
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter(|&l| world.district_of_building(l) == d)
            .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(centroid), l)))
            .min()
            .map(|(_, l)| l);
        let Some(lot) = lot else { continue };
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Lab, Some(corp)).is_err() {
            continue;
        }
        if let Some(bd) = world.comp_mut::<Building>(lot) {
            bd.focus = Some(focus);
        }
        let grant = world.config.tech.seed_lab_grant;
        if let Some(c) = world.comp_mut::<Corp>(corp) {
            c.treasury += grant;
            c.closing = c.treasury;
        }
        built.push(lot);
        let rect = world.comp::<Building>(lot).map(|b| b.rect);
        let text = format!(
            "{corp_name} opened {} ({focus}) in {district} (seeded; Lot {} {rect:?})",
            world.name_of(lot),
            lot.index
        );
        world.push_event(EventKind::Founded, &[corp, lot], text);
    }
    relink(world);
    let store = world.config.data.seed_store;
    for &lab in &built {
        let focus = world.comp::<Building>(lab).and_then(|b| b.focus).unwrap_or_default();
        if let Some(n) = node_of_building(world, lab) {
            if let Some(node) = world.virt.node_mut(n) {
                node.store.units[focus.index()] = store;
            }
        }
    }
    built
}

/// V26: the opening ICE (`ice_maker = None`, the city's own): building
/// nodes `ice_seed[kind]`, corp Ledgers one tier per `ice_value_steps`
/// step their treasury reaches, the Treasury and the Precinct
/// `levers.city_ice`. Not an event (`ice_raised` counts installs only).
pub fn seed_ice(world: &mut World) {
    if !enabled(world) {
        return;
    }
    let jail = world.building_of_kind(BuildingKind::Jail);
    let hall = world.building_of_kind(BuildingKind::Hall);
    let city_ice = world.levers.city_ice;
    let steps = world.config.ice.ice_value_steps.clone();
    for i in 0..world.virt.nodes.len() {
        let n = NodeId(i as u16);
        let Some(node) = world.virt.node(n).filter(|x| x.alive) else { continue };
        let ice = match node.kind {
            NodeKind::Public(_) => continue,
            NodeKind::Building(b) if Some(b) == jail => city_ice,
            NodeKind::Building(b) => {
                world.comp::<Building>(b).map_or(0, |bd| world.config.ice.ice_seed.for_kind(bd.kind))
            }
            NodeKind::Ledger(c) if Some(c) == hall => city_ice,
            NodeKind::Ledger(c) => {
                let t = world.purse(Some(c));
                steps.iter().filter(|&&s| t >= s).count() as u8
            }
        };
        if let Some(p) = profile_mut(world, n) {
            *p = SecurityProfile { ice: ice.min(3), ice_maker: None, ice_arrears: 0 };
        }
    }
    bump_epoch(world);
}

// ---------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------

/// The alive node of building `b` (a Lab, a Hideout, a faction building, the Precinct).
pub fn node_of_building(world: &World, b: EntityId) -> Option<NodeId> {
    world.virt.of_building.get(&b).copied()
}

/// District `d`'s Public node.
pub fn public_of(world: &World, d: DistrictId) -> NodeId {
    world.virt.public_of.get(d.index()).copied().unwrap_or_default()
}

/// A faction's Ledger node (a corp; the Hall for the Treasury).
pub fn ledger_of(world: &World, owner: EntityId) -> Option<NodeId> {
    world.virt.ledger_of.get(&owner).copied()
}

/// The node's faction (`None` = the city).
pub fn owner_of(world: &World, n: NodeId) -> Option<EntityId> {
    world.virt.node(n).and_then(|x| x.owner)
}

/// The building a node's ICE lives on: its building, or the Hall for the
/// Treasury. `None` for a Public node or a corp Ledger.
fn profile_building(world: &World, n: NodeId) -> Option<EntityId> {
    match world.virt.node(n)?.kind {
        NodeKind::Public(_) => None,
        NodeKind::Building(b) => Some(b),
        NodeKind::Ledger(c) => (!world.has::<Corp>(c)).then_some(c),
    }
}

/// V25: where a node's ICE lives (a building, `Corp.ledger_ice`, the Hall).
pub fn profile(world: &World, n: NodeId) -> Option<&SecurityProfile> {
    match world.virt.node(n)?.kind {
        NodeKind::Public(_) => None,
        NodeKind::Ledger(c) if world.has::<Corp>(c) => world.comp::<Corp>(c).map(|x| &x.ledger_ice),
        _ => profile_building(world, n).and_then(|b| world.comp::<Building>(b)).map(|bd| &bd.security),
    }
}

pub fn profile_mut(world: &mut World, n: NodeId) -> Option<&mut SecurityProfile> {
    match world.virt.node(n)?.kind {
        NodeKind::Public(_) => None,
        NodeKind::Ledger(c) if world.has::<Corp>(c) => world.comp_mut::<Corp>(c).map(|x| &mut x.ledger_ice),
        _ => {
            let b = profile_building(world, n)?;
            world.comp_mut::<Building>(b).map(|bd| &mut bd.security)
        }
    }
}

/// A corp's tier in a track; a maker that is gone reads `orphan_cap`, an
/// unset tree (a pre-M14 save before migration) reads 3 (uncapped).
pub fn maker_tier(world: &World, maker: EntityId, t: Track) -> u8 {
    match world.comp::<Corp>(maker) {
        Some(c) if c.tech.is_unset() => 3,
        Some(c) => c.tech.tier_of(t),
        None => world.config.tech.orphan_cap,
    }
}

/// V25: 0 when its upkeep is `ice_off_days` behind or a `DoorOpen` hack is
/// live on its building; else `min(ice, the maker's Deck tier)` (the city's
/// own ICE, `ice_maker = None`, is uncapped).
pub fn ice_eff(world: &World, n: NodeId) -> u8 {
    let Some(p) = profile(world, n) else { return 0 };
    if p.ice == 0 || p.ice_arrears >= world.config.ice.ice_off_days {
        return 0;
    }
    let door_open = profile_building(world, n)
        .and_then(|b| world.comp::<Building>(b))
        .and_then(|bd| bd.hacked)
        .is_some_and(|(e, until)| e == crate::virt::HackEffect::DoorOpen && until > world.tick);
    if door_open {
        return 0;
    }
    match p.ice_maker {
        None => p.ice,
        Some(m) => p.ice.min(maker_tier(world, m, Track::Deck)),
    }
}

/// V7: what a runner meets: `ice_eff + 1` while the node's alarm stands.
pub fn def(world: &World, n: NodeId) -> u8 {
    let alarm = world.virt.node(n).and_then(|x| x.alarm_until).is_some_and(|t| t > world.tick);
    ice_eff(world, n) + u8::from(alarm)
}

/// V27: a Lab's or Hideout's `Σ store × data_price`, a Ledger's purse, any
/// other building's `ownership::value` plus the value of the assets posted
/// there. A Public node risks nothing.
pub fn value_at_risk(world: &World, n: NodeId) -> i64 {
    let Some(node) = world.virt.node(n) else { return 0 };
    match node.kind {
        NodeKind::Public(_) => 0,
        NodeKind::Ledger(c) if world.has::<Corp>(c) => world.purse(Some(c)),
        NodeKind::Ledger(_) => world.purse(None),
        NodeKind::Building(b) => {
            let Some(bd) = world.comp::<Building>(b) else { return 0 };
            if matches!(bd.kind, BuildingKind::Lab | BuildingKind::Hideout) {
                return i64::from(node.store.total()) * world.config.data.data_price;
            }
            let posted: i64 = crate::systems::assets::assets_at(world, b)
                .iter()
                .filter_map(|&a| world.comp::<crate::components::Asset>(a))
                .filter(|x| x.loc == AssetLoc::Posted(b))
                .map(|x| x.value)
                .sum();
            ownership::value(world, bd.kind) + posted
        }
    }
}

/// V27: one tier per `ice_value_steps` step the value at risk reaches.
pub fn ice_target(world: &World, n: NodeId) -> u8 {
    let v = value_at_risk(world, n);
    world.config.ice.ice_value_steps.iter().filter(|&&s| v >= s).count() as u8
}

/// A faction's holding in a track: the sum of `store[t]` over its alive nodes.
pub fn holding(world: &World, faction: EntityId, t: Track) -> u32 {
    world.virt.nodes.iter().filter(|n| n.alive && n.owner == Some(faction)).map(|n| n.store.get(t)).sum()
}

// ---------------------------------------------------------------------------
// ICE installs (V26)
// ---------------------------------------------------------------------------

/// The Security corp selling ICE at `tier`: Deck tier at least `tier`, the
/// lowest `ice_price[tier] × level(Security)`, ties the lower id.
fn ice_seller(world: &World, tier: u8) -> Option<(i64, EntityId)> {
    let base = world.config.ice.price(tier)?;
    world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).map(|cc| (c, cc)))
        .filter(|(_, cc)| cc.niches.contains(&Niche::Security) && !cc.tech.is_unset())
        .filter(|(_, cc)| cc.tech.tier_of(Track::Deck) >= tier)
        .map(|(c, cc)| ((base as f32 * cc.level(Niche::Security)).round() as i64, c))
        .min()
}

/// V26: raise `n`'s ICE one tier, paid by `owner`: self-installed at
/// `self_install_frac` of the cheapest seller's price (to the City,
/// `Flow::Import`) when the owner is a corp whose own Deck tier suffices,
/// else bought from the cheapest Security corp that can sell the tier
/// (`Flow::Contract`). `ice_maker` is the installer. A gang or an agent
/// pays in full or not at all. `IceRaised`, `Corp.ice_spend_today`,
/// `stats.ice_spend`. False when nothing was installed.
pub fn install_ice(world: &mut World, owner: Option<EntityId>, n: NodeId) -> bool {
    if !enabled(world) {
        return false;
    }
    let Some(cur) = profile(world, n).map(|p| p.ice) else { return false };
    if cur >= 3 {
        return false;
    }
    let tier = cur + 1;
    let Some(base) = world.config.ice.price(tier) else { return false };
    let seller = ice_seller(world, tier);
    let own_deck = owner.and_then(|o| world.comp::<Corp>(o)).map_or(0, |c| c.tech.tier_of(Track::Deck));
    let (cost, payee, maker, flow) = if own_deck >= tier {
        let cheapest = seller.map_or(base, |(p, _)| p);
        let cost = (cheapest as f32 * world.config.ice.self_install_frac).round() as i64;
        (cost, None, owner, Flow::Import)
    } else {
        let Some((price, s)) = seller else { return false };
        (price, Some(s), Some(s), Flow::Contract)
    };
    let full = matches!(ownership::owner_kind(world, owner), OwnerKind::City | OwnerKind::Corp(_));
    if !full && world.purse(owner) < cost {
        return false;
    }
    ownership::charge(world, owner, payee, cost, flow);
    if let Some(p) = profile_mut(world, n) {
        p.ice = tier;
        p.ice_maker = maker;
        p.ice_arrears = 0;
    }
    if let Some(c) = owner.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.ice_spend_today += cost;
    }
    world.stats.current.virt.ice_spend += cost;
    world.stats.current.virt.ice_raised += 1;
    bump_epoch(world);
    let what = node_label(world, n);
    let (who, by) = (world.owner_label(owner), world.owner_label(maker));
    let building = profile_building(world, n).unwrap_or(EntityId::NONE);
    world.push_event(
        EventKind::IceRaised,
        &[owner.unwrap_or(EntityId::NONE), building],
        format!("{who} raised {what} to ICE {tier} ({by})"),
    );
    true
}

/// V27 Hunker: lower `n`'s ICE one tier (no refund). `IceLowered`.
pub fn lower_ice(world: &mut World, n: NodeId) -> bool {
    if !enabled(world) {
        return false;
    }
    let Some(p) = profile_mut(world, n).filter(|p| p.ice > 0) else { return false };
    p.ice -= 1;
    let tier = p.ice;
    world.stats.current.virt.ice_lowered += 1;
    bump_epoch(world);
    let owner = owner_of(world, n);
    let what = node_label(world, n);
    let who = world.owner_label(owner);
    let building = profile_building(world, n).unwrap_or(EntityId::NONE);
    world.push_event(
        EventKind::IceLowered,
        &[owner.unwrap_or(EntityId::NONE), building],
        format!("{who} lowered {what} to ICE {tier}"),
    );
    true
}

/// "Vat Farm#12", "Zetatech's Ledger", "the Treasury", "Spire Public".
pub fn node_label(world: &World, n: NodeId) -> String {
    let Some(node) = world.virt.node(n) else { return format!("node {}", n.0) };
    match node.kind {
        NodeKind::Public(d) => format!("{} Public", world.district(d).name),
        NodeKind::Building(b) => world.name_of(b),
        NodeKind::Ledger(c) if world.has::<Corp>(c) => format!("{}'s Ledger", world.owner_label(Some(c))),
        NodeKind::Ledger(_) => "the Treasury".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: routing (plan V5-V7), the run as an event chain (V9-V15, V20),
// the freelance Hack goal (V29), decks (V30, V36)
// ---------------------------------------------------------------------------
//
// A run is a short chain of seeded dice contests between a fictional runner
// (deck tier plus skill) and a node (its ICE tier), resolved exactly as
// `law::resolve_fight` resolves a brawl: one draw against one probability.

/// M16 addendum hook: an owner's alertness multiplies every detection roll
/// a run meets (the trace). 1.0 until M16 defines it.
pub fn alertness_mult(_world: &World, _owner: Option<EntityId>) -> f32 {
    1.0
}

/// V8: `[chrome] contest_step` (one value for locks, sensors and ICE).
fn contest_step(world: &World) -> f32 {
    world.config.chrome.contest_step
}

/// V7: a runner for `patron` contests `n` when the node is guarded (`def >
/// 0`) and not the patron's own (a freelancer: every owner).
pub fn contested(world: &World, n: NodeId, patron: Option<EntityId>) -> bool {
    def(world, n) > 0 && (patron.is_none() || owner_of(world, n) != patron)
}

/// What a runner brings to every contest of a route (V6, V8, V14, V66).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Odds {
    /// `deck_eff + hack_w × hacking` (+ the mode's `*_att`).
    pub att: f32,
    pub patron: Option<EntityId>,
    /// M13's `p_dodge` (the fry's save).
    pub dodge: f32,
    /// `(1 − stealth_w × stealth) × the mode's trace factor`.
    pub trace_mult: f32,
}

/// V66: a freelancer runs loud when `(1 − lawfulness) × courage ≥ loud_min`.
pub fn mode_of(world: &World, agent: EntityId) -> RunMode {
    let p = world.comp::<Personality>(agent);
    let bold = p.map_or(0.0, |p| (1.0 - p.lawfulness) * p.courage);
    if bold >= world.config.hack.loud_min {
        RunMode::Loud
    } else {
        RunMode::Quiet
    }
}

/// V66: `(take, trace factor, att shift)` of a mode.
fn mode_terms(world: &World, mode: RunMode) -> (f32, f32, f32) {
    let h = &world.config.hack;
    match mode {
        RunMode::Quiet => (h.quiet_take, h.quiet_trace, h.quiet_att),
        RunMode::Loud => (h.loud_take, h.loud_trace, h.loud_att),
    }
}

/// The runner's odds with a deck working at `deck_eff`.
pub fn odds_of(world: &World, runner: EntityId, deck_eff: u8, patron: Option<EntityId>, mode: RunMode) -> Odds {
    let hacking = world.comp::<Skills>(runner).map_or(0.0, |s| s.hacking.max(0.0));
    let (_, trace, shift) = mode_terms(world, mode);
    let stealth = crate::systems::law::stealth(world, runner);
    Odds {
        att: f32::from(deck_eff) + world.config.ice.hack_w * hacking + shift,
        patron,
        dodge: crate::systems::vehicles::p_dodge(world, runner),
        trace_mult: (1.0 - world.config.ice.stealth_w * stealth).max(0.0) * trace,
    }
}

/// V14: a lost contest's raw fry chance and trace chance at node `n`, the
/// gap being `def − att`: `fry_base[ice] × (1 + fry_gap × max(gap, 0))` and
/// `trace_p[ice] × trace_mult × alertness`, each capped at 1.
fn loss_odds(world: &World, n: NodeId, gap: f32, o: &Odds) -> (f32, f32) {
    let cfg = &world.config.ice;
    let ice = ice_eff(world, n);
    let fry = (IceCfg::at(&cfg.fry_base, ice) * (1.0 + cfg.fry_gap * gap.max(0.0))).clamp(0.0, 1.0);
    let alert = alertness_mult(world, owner_of(world, n));
    let trace = (IceCfg::at(&cfg.trace_p, ice) * o.trace_mult * alert).clamp(0.0, 1.0);
    (fry, trace)
}

/// V6: one Dijkstra from `from` over alive nodes, links above `deck_eff`
/// skipped; entering `n` costs `hop_eps` plus `−ln p_pass(att, def(n))`
/// when `n` is contested for `patron`. Ties `(cost, NodeId)`. Cached per
/// `RouteKey` for the epoch-hour (`att` quantized to 1/20, the search runs
/// on the quantized value so a tree is a pure function of its key). Takes
/// `&World`: the cache is interior (a think scores through it).
pub fn routes_from(world: &World, from: NodeId, deck_eff: u8, att: f32, patron: Option<EntityId>) -> Arc<RouteTree> {
    let att_q = (att * 20.0).round().clamp(0.0, f32::from(u16::MAX)) as u16;
    let key = RouteKey { from, deck_eff, att_q, patron: patron.unwrap_or(EntityId::NONE) };
    let hour = world.tick / TICKS_PER_HOUR;
    world.virt.cache.get_or_build(hour, key, || search(world, from, deck_eff, f32::from(att_q) / 20.0, patron))
}

fn search(world: &World, from: NodeId, deck_eff: u8, att: f32, patron: Option<EntityId>) -> RouteTree {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let p = &world.virt;
    let len = p.nodes.len();
    let mut tree = RouteTree { dist: vec![f32::INFINITY; len], pred: vec![None; len], hops: vec![0; len] };
    if p.nodes.get(from.index()).is_none_or(|n| !n.alive) {
        return tree;
    }
    let (eps, step) = (world.config.virt.hop_eps, contest_step(world));
    let enter: Vec<f32> = (0..len)
        .map(|i| {
            let m = NodeId(i as u16);
            let toll = if p.nodes[i].alive && contested(world, m, patron) {
                -crate::systems::security::contest_p(att, f32::from(def(world, m)), step).ln()
            } else {
                0.0
            };
            eps + toll
        })
        .collect();
    let mut done = vec![false; len];
    let mut heap = BinaryHeap::new();
    tree.dist[from.index()] = 0.0;
    heap.push(Reverse((ordered_float::OrderedFloat(0.0f32), from)));
    while let Some(Reverse((ordered_float::OrderedFloat(d), n))) = heap.pop() {
        if done[n.index()] {
            continue;
        }
        done[n.index()] = true;
        for &(m, l) in &p.adj[n.index()] {
            if p.links.get(usize::from(l)).is_none_or(|k| k.tier > deck_eff) || done[m.index()] {
                continue;
            }
            let nd = d + enter[m.index()];
            if nd < tree.dist[m.index()] {
                tree.dist[m.index()] = nd;
                tree.pred[m.index()] = Some(n);
                tree.hops[m.index()] = tree.hops[n.index()].saturating_add(1);
                heap.push(Reverse((ordered_float::OrderedFloat(nd), m)));
            }
        }
    }
    tree
}

/// V6: the route to `to` read off `tree` (walking `pred`), with its odds:
/// `p_success` = Π over contested intermediates `p` × `p(to)²` (the target
/// twice, when contested); `p_fry` and `p_trace` = `1 − Π(1 − q)` over the
/// same contests, `q = (1 − p) × fry × (1 − dodge)` and `(1 − p) × trace`.
/// `None` when unreachable or longer than 8 nodes.
pub fn route_to(world: &World, tree: &RouteTree, from: NodeId, to: NodeId, o: &Odds) -> Option<Route> {
    if !tree.dist.get(to.index()).is_some_and(|d| d.is_finite()) {
        return None;
    }
    let mut nodes: SmallVec<[NodeId; 8]> = SmallVec::new();
    let mut cur = to;
    loop {
        nodes.push(cur);
        if nodes.len() > 8 {
            return None;
        }
        if cur == from {
            break;
        }
        cur = tree.pred.get(cur.index()).copied().flatten()?;
    }
    nodes.reverse();
    let step = contest_step(world);
    let (mut ok, mut no_fry, mut no_trace) = (1.0f32, 1.0f32, 1.0f32);
    for &n in nodes.iter().skip(1) {
        if !contested(world, n, o.patron) {
            continue;
        }
        let d = f32::from(def(world, n));
        let p = crate::systems::security::contest_p(o.att, d, step);
        let (fry, trace) = loss_odds(world, n, d - o.att, o);
        for _ in 0..if n == to { 2 } else { 1 } {
            ok *= p;
            no_fry *= 1.0 - (1.0 - p) * fry * (1.0 - o.dodge);
            no_trace *= 1.0 - (1.0 - p) * trace;
        }
    }
    Some(Route { nodes, p_success: ok, p_fry: 1.0 - no_fry, p_trace: 1.0 - no_trace })
}

/// V5: the node a run starts from: the chair building's own node when it
/// has one and its owner is the patron or the runner's employer corp, else
/// the Public node of the chair's district.
pub fn portal(world: &World, chair: EntityId, runner: EntityId, patron: Option<EntityId>) -> NodeId {
    let employer =
        world.comp::<crate::components::Job>(runner).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e));
    if let Some(n) = node_of_building(world, chair) {
        let owner = owner_of(world, n);
        if owner.is_some() && (owner == patron || owner == employer) {
            return n;
        }
    }
    public_of(world, world.district_of_building(chair))
}

/// V5: the agent's chair: the nearest (Manhattan from its tile, ties the
/// lower id) of its Home, its gang's Hideout, its workplace when that is a
/// Lab, and every open Bar and Hotel (a public terminal).
pub fn chair_for(world: &World, agent: EntityId, patron: Option<EntityId>) -> Option<Portal> {
    let tile = world.comp::<Position>(agent)?.tile;
    let usable = |b: EntityId| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict);
    let mut cands: SmallVec<[EntityId; 8]> = SmallVec::new();
    cands.extend(world.comp::<crate::components::Household>(agent).and_then(|h| h.home));
    cands.extend(world.gang_of(agent).and_then(|g| world.hideout_of(g)));
    cands.extend(
        world
            .comp::<crate::components::Job>(agent)
            .and_then(|j| j.employer)
            .filter(|&e| world.comp::<Building>(e).is_some_and(|b| b.kind == BuildingKind::Lab)),
    );
    let terminals =
        world.buildings_of_kind(BuildingKind::Bar).iter().chain(world.buildings_of_kind(BuildingKind::Hotel));
    let chair = cands
        .into_iter()
        .chain(terminals.copied().filter(|&b| !world.is_closed(b)))
        .filter(|&b| usable(b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(tile), b)))
        .min()
        .map(|(_, b)| b)?;
    Some(Portal { building: chair, node: portal(world, chair, agent, patron) })
}

/// A public terminal: a Bar or a Hotel.
fn is_terminal(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| matches!(bd.kind, BuildingKind::Bar | BuildingKind::Hotel))
}

/// V29: `[data] steal_units[deck_eff − 1]` (0 past the table).
fn steal_units(world: &World, deck_eff: u8) -> u32 {
    world.config.data.steal_units.get(usize::from(deck_eff.saturating_sub(1))).copied().unwrap_or(0)
}

/// V20: `min(ledger_frac × purse, ledger_cap[deck_eff − 1])`.
fn ledger_take(world: &World, purse: i64, deck_eff: u8) -> i64 {
    let cap = world.config.data.ledger_cap.get(usize::from(deck_eff.saturating_sub(1))).copied().unwrap_or(0);
    ((world.config.data.ledger_frac * purse.max(0) as f32).floor() as i64).min(cap).max(0)
}

/// V29: what a runner could go after with a deck at `deck_eff`, and its
/// value in coins: Data on alive Lab and Hideout nodes with a store, not of
/// the agent's employer corp, its own gang, the patron or a corp it runs; a Ledger (a corp's
/// or the Treasury) with a tier-2 deck. Values carry the mode's take (V66).
pub fn targets(
    world: &World,
    agent: EntityId,
    patron: Option<EntityId>,
    deck_eff: u8,
    mode: RunMode,
) -> SmallVec<[(NodeId, Purpose, i64); 32]> {
    let mut out = SmallVec::new();
    let employer =
        world.comp::<crate::components::Job>(agent).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e));
    let gang = world.gang_of(agent);
    // Plan addition: nor the corp the agent runs as exec (no robbing oneself).
    let own = |o: EntityId| world.comp::<Corp>(o).is_some_and(|c| c.exec == Some(agent));
    let mine = |o: Option<EntityId>| o.is_some() && (o == employer || o == gang || o == patron || o.is_some_and(own));
    let (take, _, _) = mode_terms(world, mode);
    let price = world.config.data.data_price;
    let units = steal_units(world, deck_eff);
    let ledger_ok = deck_eff >= world.config.virt.trunk_links_tier;
    for (i, n) in world.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive && !mine(n.owner)) {
        let id = NodeId(i as u16);
        match n.kind {
            NodeKind::Building(b) => {
                let kind = world.comp::<Building>(b).map(|bd| bd.kind);
                if !matches!(kind, Some(BuildingKind::Lab | BuildingKind::Hideout)) || n.store.total() == 0 {
                    continue;
                }
                let got = (n.store.total().min(units) as f32 * take).round() as i64;
                if got > 0 {
                    out.push((id, Purpose::Data { wipe: false }, got * price));
                }
            }
            NodeKind::Ledger(c) if ledger_ok => {
                let purse = if world.has::<Corp>(c) { world.purse(Some(c)) } else { world.purse(None) };
                let got = (ledger_take(world, purse, deck_eff) as f32 * take).round() as i64;
                if got > 0 {
                    out.push((id, Purpose::Ledger, got));
                }
            }
            _ => {}
        }
    }
    out
}

/// V29 (one `routes_from` per scorer): the best target by `EV × p_success`
/// (ties the lower `NodeId`) among those at `p_success ≥ min_route_p`.
pub fn best_target(
    world: &World,
    agent: EntityId,
    patron: Option<EntityId>,
) -> Option<(Portal, NodeId, Purpose, i64, Route)> {
    let deck_eff = world.comp::<Kit>(agent).filter(|k| k.deck.is_some()).map(|k| k.deck_tier)?;
    if deck_eff == 0 {
        return None;
    }
    let chair = chair_for(world, agent, patron)?;
    let mode = mode_of(world, agent);
    let o = odds_of(world, agent, deck_eff, patron, mode);
    let tree = routes_from(world, chair.node, deck_eff, o.att, patron);
    let min = world.config.virt.min_route_p;
    let mut best: Option<(f32, NodeId, Purpose, i64, Route)> = None;
    for (n, purpose, ev) in targets(world, agent, patron, deck_eff, mode) {
        if n == chair.node {
            continue;
        }
        let Some(route) = route_to(world, &tree, chair.node, n, &o) else { continue };
        if route.p_success < min {
            continue;
        }
        let score = ev as f32 * route.p_success;
        if best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, n, purpose, ev, route));
        }
    }
    best.map(|(_, n, purpose, ev, route)| (chair, n, purpose, ev, route))
}

/// The Hack goal's offer (V29): a run, or the sale of the Data a deck holds.
#[derive(Clone, Debug)]
pub struct HackOffer {
    /// `GoTo(DataBuyer) → SellData` (the Lab in `lab`).
    pub sell: bool,
    pub lab: Option<EntityId>,
    pub portal: Option<Portal>,
    pub target: NodeId,
    pub purpose: Purpose,
    pub ev: i64,
    pub route: Option<Route>,
    pub mode: RunMode,
    pub considerations: Vec<crate::utility::Consideration>,
}

/// The Data on the agent's deck (`Kit.deck`'s `Asset.data`), all tracks.
pub fn deck_data(world: &World, agent: EntityId) -> u32 {
    world
        .comp::<Kit>(agent)
        .and_then(|k| k.deck)
        .and_then(|d| world.comp::<Asset>(d))
        .map_or(0, |a| a.data.iter().sum())
}

/// Is the Hack goal cooled for `agent` (a run's or a fry's cooldown, or a
/// failed plan)?
pub fn hack_cooled(world: &World, agent: EntityId) -> bool {
    world.comp::<Brain>(agent).and_then(|b| b.cooldowns.get(&GoalKind::Hack)).is_some_and(|&until| until > world.tick)
}

/// V29: the Hack goal's gate and considerations, for a Full or Coarse adult
/// with a deck, no Sentence: the sale of its deck's Data first (no
/// cooldown: the run already happened); else, off cooldown, at `hacking ≥
/// hack_min`, with a chair and a target at `p_success ≥ min_route_p`, the
/// best run. `None` with the plane off.
pub fn hack_choice(world: &World, id: EntityId) -> Option<HackOffer> {
    use crate::utility::curves::{can, urgency, Curve, GATE};
    use crate::utility::Consideration;
    if !enabled(world) {
        return None;
    }
    let kit = world.comp::<Kit>(id)?;
    let deck = kit.deck?;
    let brain = world.comp::<Brain>(id)?;
    if brain.lod == crate::components::Lod::Statistical
        || world.has::<crate::components::Sentence>(id)
        || !crate::systems::demography::is_adult(world, id)
    {
        return None;
    }
    let wealth = world.comp::<crate::components::Needs>(id).map_or(0.5, |n| urgency(n.wealth));
    let held = world.comp::<Asset>(deck).map_or(0, |a| a.data.iter().sum::<u32>());
    // With no buyer able to pay, the Data waits on the deck and the runner may run again.
    let buyer = if held > 0 { crate::systems::tech::data_buyer_lab(world, id) } else { None };
    if let Some(lab) = buyer {
        let value = i64::from(held) * world.config.data.data_price;
        let x = (value as f32 / world.config.hack.hack_ref.max(1) as f32).min(1.0);
        let considerations = vec![
            Consideration::new("can sell", can(true), GATE),
            Consideration::new("U(value)", x, Curve::Linear { m: 0.5, b: 0.5 }),
            Consideration::new("U(wealth)", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ];
        return Some(HackOffer {
            sell: true,
            lab: Some(lab),
            portal: None,
            target: NodeId::default(),
            purpose: Purpose::Data { wipe: false },
            ev: value,
            route: None,
            mode: RunMode::Quiet,
            considerations,
        });
    }
    if hack_cooled(world, id) || world.runner_of.contains_key(&id) {
        return None;
    }
    let hacking = world.comp::<Skills>(id).map_or(0.0, |s| s.hacking);
    if hacking < world.config.hack.hack_min {
        return None;
    }
    let (portal, target, purpose, ev, route) = best_target(world, id, None)?;
    let p = world.comp::<Personality>(id);
    let (law, greed, courage) = p.map_or((0.5, 0.5, 0.5), |p| (p.lawfulness, p.greed, p.courage));
    let x = (ev as f32 / world.config.hack.hack_ref.max(1) as f32).min(1.0);
    let considerations = vec![
        Consideration::new("can hack", can(true), GATE),
        Consideration::new("U(EV)", x, Curve::Logistic { k: 8.0, mid: 0.3 }),
        Consideration::new("1-lawfulness", 1.0 - law, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("greed", greed, Curve::Linear { m: 0.5, b: 0.5 }),
        // Plan deviation (as M12's Squat): `U(wealth)`, not the spec's
        // `1 − U(wealth)`, so the poor run more, not less.
        Consideration::new("U(wealth)", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new(
            "1-p_fry x (1-courage)",
            1.0 - route.p_fry * (1.0 - courage),
            Curve::Linear { m: 0.6, b: 0.4 },
        ),
    ];
    Some(HackOffer {
        sell: false,
        lab: None,
        portal: Some(portal),
        target,
        purpose,
        ev,
        route: Some(route),
        mode: mode_of(world, id),
        considerations,
    })
}

/// V29: the Hack goal's plan, built directly (`plan.rs` calls it at bind):
/// `[GoTo(DataBuyer)] → SellData` for a deck holding Data; else the
/// freelance `RunOrder` is written and the plan is `[GoTo(Chair)] → JackIn`.
pub fn hack_plan(world: &mut World, id: EntityId) -> Option<crate::goap::Plan> {
    use crate::components::ActionInstance;
    use crate::goap::{ActionKind, LocationKey};
    let offer = hack_choice(world, id)?;
    let tick = world.tick;
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let step = |action: ActionKind, target: Option<EntityId>| ActionInstance { action, target, tile: None };
    if offer.sell {
        let lab = offer.lab?;
        let mut steps = Vec::new();
        if here != Some(lab) {
            steps.push(step(ActionKind::GoTo(LocationKey::DataBuyer), Some(lab)));
        }
        steps.push(step(ActionKind::SellData, Some(lab)));
        return Some(crate::goap::Plan { goal: GoalKind::Hack, target: Some(lab), steps, started_tick: tick });
    }
    let portal = offer.portal?;
    world.run_orders.insert(
        id,
        RunOrder {
            patron: None,
            purpose: offer.purpose,
            target: offer.target,
            chair: portal.building,
            not_before: tick,
            expires: tick + TICKS_PER_DAY,
            why: RunWhy::Freelance,
            mode: offer.mode,
        },
    );
    let mut steps = Vec::new();
    if here != Some(portal.building) {
        steps.push(step(ActionKind::GoTo(LocationKey::Chair), Some(portal.building)));
    }
    steps.push(step(ActionKind::JackIn, Some(portal.building)));
    Some(crate::goap::Plan { goal: GoalKind::Hack, target: Some(portal.building), steps, started_tick: tick })
}

/// V5: `JackIn`'s start at a public terminal: `terminal_fee` to the Bar's
/// or Hotel's owner (`Flow::Terminal`, taxed).
pub fn pay_terminal(world: &mut World, runner: EntityId) {
    if !enabled(world) {
        return;
    }
    let Some(chair) = world.run_orders.get(&runner).map(|o| o.chair) else { return };
    if !is_terminal(world, chair) || world.comp::<Position>(runner).and_then(|p| p.building) != Some(chair) {
        return;
    }
    let owner = world.owner_of(chair);
    if owner == Some(runner) {
        return;
    }
    let fee = world.config.virt.terminal_fee;
    let paid = ownership::pay(world, Some(runner), owner, fee, Flow::Terminal);
    ownership::credit(world, chair, paid);
}

/// `hop_ticks[deck_eff − 1]`.
fn hop_ticks(world: &World, deck_eff: u8) -> Tick {
    let h = &world.config.virt.hop_ticks;
    Tick::from(h.get(usize::from(deck_eff.saturating_sub(1))).or(h.last()).copied().unwrap_or(8))
}

fn contest_ticks(world: &World) -> Tick {
    Tick::from(world.config.virt.contest_ticks)
}

fn act_ticks(world: &World, purpose: Purpose) -> Tick {
    let a = &world.config.ice.act_ticks;
    Tick::from(match purpose {
        Purpose::Data { .. } | Purpose::Overwatch(_) => a.data,
        Purpose::Ledger => a.ledger,
        Purpose::Door => a.door,
        Purpose::Robot(_) => a.robot,
        Purpose::Camera(_) => a.camera,
    })
}

/// V10: `(start_tick << 22) | runner.index`.
pub fn run_id(tick: Tick, runner: EntityId) -> RunId {
    debug_assert!(runner.index < (1 << 22), "a runner index fits 22 bits");
    debug_assert!(tick < (1 << 28), "a RunId fits 50 bits");
    (tick << 22) | u64::from(runner.index)
}

/// V10/V11: `JackIn` completes: the agent's `RunOrder` becomes a `Run`,
/// re-routed from the portal with the current ICE. A freelancer whose route
/// is gone or below `min_route_p` loses the step (`PreconditionLost`); an
/// ordered runner jacks in anyway. The run is queued, `runner_of` set, the
/// `JackedIn` event pushed.
pub fn start_run(world: &mut World, runner: EntityId) -> Result<RunId, crate::exec::FailReason> {
    use crate::exec::FailReason;
    if !enabled(world) || world.runner_of.contains_key(&runner) {
        return Err(FailReason::PreconditionLost);
    }
    let order = world.run_orders.get(&runner).cloned().ok_or(FailReason::PreconditionLost)?;
    let (deck, deck_eff) = world
        .comp::<Kit>(runner)
        .and_then(|k| k.deck.map(|d| (d, k.deck_tier)))
        .filter(|&(_, t)| t > 0)
        .ok_or(FailReason::PreconditionLost)?;
    // Phase 3 builds Overwatch runs (they take no route).
    if matches!(order.purpose, Purpose::Overwatch(_)) {
        return Err(FailReason::PreconditionLost);
    }
    let from = portal(world, order.chair, runner, order.patron);
    let o = odds_of(world, runner, deck_eff, order.patron, order.mode);
    let tree = routes_from(world, from, deck_eff, o.att, order.patron);
    let route = route_to(world, &tree, from, order.target, &o).ok_or(FailReason::PreconditionLost)?;
    if order.why == RunWhy::Freelance && route.p_success < world.config.virt.min_route_p {
        return Err(FailReason::PreconditionLost);
    }
    let now = world.tick;
    let id = run_id(now, runner);
    let at_target = route.nodes.len() == 1;
    let run = Run {
        id,
        runner,
        deck,
        chair: order.chair,
        patron: order.patron,
        purpose: order.purpose,
        target: order.target,
        route: route.nodes.clone(),
        at: 0,
        phase: if at_target { RunPhase::BreakIn } else { RunPhase::Hop },
        next_at: if at_target { now } else { now + hop_ticks(world, deck_eff) },
        payload: [0; 3],
        stream: false,
        log: SmallVec::new(),
        portal: from,
        att: o.att,
        deck_eff,
        contests: 0,
        outcome: None,
        source: None,
        mode: order.mode,
        why: order.why,
    };
    world.run_queue.insert((run.next_at, id));
    world.runner_of.insert(runner, id);
    world.runs.insert(id, run);
    let what = match order.purpose {
        Purpose::Data { wipe: true } => "a wipe",
        Purpose::Data { wipe: false } => "Data",
        Purpose::Ledger => "a ledger",
        Purpose::Door => "a door",
        Purpose::Robot(_) => "a robot",
        Purpose::Camera(_) => "a camera",
        Purpose::Overwatch(_) => "overwatch",
    };
    let text = format!(
        "{} jacked in at {} ({what} on {})",
        world.name_of(runner),
        world.name_of(order.chair),
        node_label(world, order.target)
    );
    world.push_event(EventKind::JackedIn, &[runner, order.chair], text);
    Ok(id)
}

/// V10: the per-tick pop: every run step due by now, in `(tick, id)` order
/// (a step due at once is handled in the same tick).
fn pop_due(world: &mut World) {
    let now = world.tick;
    while let Some(&(t, id)) = world.run_queue.first() {
        if t > now {
            break;
        }
        world.run_queue.remove(&(t, id));
        step(world, id);
    }
}

/// The run sequence (plan 2.4): Hop → BreakIn → Act → Extract → Out; a lost
/// contest ends it ([`lose`]). Copy the run out, mutate, write it back.
pub fn step(world: &mut World, id: RunId) {
    let Some(mut r) = world.runs.get(&id).cloned() else { return };
    let now = world.tick;
    match r.phase {
        RunPhase::Hop => {
            r.at = r.at.saturating_add(1);
            let Some(&n) = r.route.get(usize::from(r.at)) else {
                return end_run(world, r, RunOutcome::Bounced);
            };
            if n == r.target {
                r.phase = RunPhase::BreakIn;
                r.next_at = now;
            } else if contested(world, n, r.patron) {
                if let Some(rolls) = contest(world, &mut r, n) {
                    return lose(world, r, n, rolls);
                }
                r.next_at = now + hop_ticks(world, r.deck_eff) + contest_ticks(world);
            } else {
                r.next_at = now + hop_ticks(world, r.deck_eff);
            }
        }
        RunPhase::BreakIn => {
            let n = r.target;
            if contested(world, n, r.patron) {
                if let Some(rolls) = contest(world, &mut r, n) {
                    return lose(world, r, n, rolls);
                }
            }
            act_begin(world, &mut r);
            r.phase = RunPhase::Act;
            r.next_at = now + act_ticks(world, r.purpose);
        }
        RunPhase::Act => {
            r.phase = RunPhase::Extract;
            r.next_at = now + contest_ticks(world);
        }
        RunPhase::Extract => {
            let n = r.target;
            if contested(world, n, r.patron) {
                if let Some(rolls) = contest(world, &mut r, n) {
                    return lose(world, r, n, rolls);
                }
            }
            r.phase = RunPhase::Out;
            r.next_at = now;
        }
        RunPhase::Out => {
            apply_out(world, &mut r);
            return end_run(world, r, RunOutcome::Success);
        }
    }
    world.run_queue.insert((r.next_at, id));
    world.runs.insert(id, r);
}

/// The draws of a lost contest, in the stream's fixed order (V9).
#[derive(Copy, Clone, Debug)]
pub struct LossRolls {
    pub gap: f32,
    pub fry: f32,
    pub dodge: f32,
    pub kill: f32,
    pub trace: f32,
}

/// V8/V9: one contest at `n` on the run's own stream `rng.run(id,
/// contests)`: the pass draw, logged; on a loss the fry, dodge, kill and
/// trace draws follow on the same stream. `None` = won.
fn contest(world: &World, r: &mut Run, n: NodeId) -> Option<LossRolls> {
    let mut rng = world.rng.run(r.id, r.contests);
    r.contests = r.contests.saturating_add(1);
    let d = f32::from(def(world, n));
    let won = crate::systems::security::contest_f(r.att, d, contest_step(world), &mut rng);
    r.log.push((world.tick, n, won));
    if won {
        return None;
    }
    Some(LossRolls { gap: d - r.att, fry: rng.random(), dodge: rng.random(), kill: rng.random(), trace: rng.random() })
}

/// V14: what a lost contest at `n` does, from its draws: `(fried, dead,
/// traced)`. Fried when the fry draw is under `fry_base[ice] x (1 + fry_gap
/// x max(gap, 0))` and the dodge draw is not under `p_dodge`; dead when
/// fried and the kill draw is under `p_flatline` (ICE 3 at `gap >=
/// flatline_gap`) or `p_fry_kill[ice]`; traced (independently) when the
/// trace draw is under the trace odds.
pub fn loss_verdict(world: &World, n: NodeId, o: &Odds, rolls: &LossRolls) -> (bool, bool, bool) {
    let cfg = &world.config.ice;
    let (fry_p, trace_p) = loss_odds(world, n, rolls.gap, o);
    let ice = ice_eff(world, n);
    let fried = rolls.fry < fry_p && rolls.dodge >= o.dodge;
    let kill_p =
        if ice >= 3 && rolls.gap >= cfg.flatline_gap { cfg.p_flatline } else { IceCfg::at(&cfg.p_fry_kill, ice) };
    (fried, fried && rolls.kill < kill_p, rolls.trace < trace_p)
}

/// V9: a contest's draws on its stream, in their fixed order: the pass
/// draw, then the fry, dodge, kill and trace draws a loss reads.
pub fn contest_draws(world: &World, run: RunId, contest: u8, gap: f32) -> (f32, LossRolls) {
    let mut rng = world.rng.run(run, contest);
    let pass: f32 = rng.random();
    (pass, LossRolls { gap, fry: rng.random(), dodge: rng.random(), kill: rng.random(), trace: rng.random() })
}

/// V66: a run's trace factor and take as `Odds` for a live runner.
pub fn run_odds(world: &World, r: &Run) -> Odds {
    let (_, trace, _) = mode_terms(world, r.mode);
    let stealth = crate::systems::law::stealth(world, r.runner);
    Odds {
        att: r.att,
        patron: r.patron,
        dodge: crate::systems::vehicles::p_dodge(world, r.runner),
        trace_mult: (1.0 - world.config.ice.stealth_w * stealth).max(0.0) * trace,
    }
}

/// V14, in order: the fry (saved by `p_dodge`): `Fried`, sanity and energy
/// hits, the `Fried` memory, deck wear; then the kill (`p_fry_kill[ice]`, or
/// `p_flatline` at ICE 3 with `gap ≥ flatline_gap`). The trace rolls
/// independently. An Extract loss returns the payload (`Captured`). The
/// recorded outcome is the worst of Flatlined > Fried > Traced > Captured >
/// Bounced. A survivor is dazed `daze_ticks` in the chair.
pub fn lose(world: &mut World, mut r: Run, n: NodeId, rolls: LossRolls) {
    let cfg = world.config.ice.clone();
    let o = run_odds(world, &r);
    let (fried, dead, traced) = loss_verdict(world, n, &o, &rolls);
    let owner = owner_of(world, n);
    let runner = r.runner;
    if fried {
        world.stats.current.virt.fried += 1;
        let text = format!("{}'s ICE fried {}", node_label(world, n), world.name_of(runner));
        world.push_event(EventKind::Fried, &[runner, owner.unwrap_or(EntityId::NONE)], text);
        if let Some(b) = world.comp_mut::<crate::components::Body>(runner) {
            b.sanity = (b.sanity - cfg.fry_sanity).max(0.0);
        }
        if let Some(nd) = world.comp_mut::<crate::components::Needs>(runner) {
            nd.energy = (nd.energy - cfg.fry_energy).max(0.0);
        }
        world.remember(runner, crate::components::MemoryKind::Fried, None, 0.9, -0.8, false);
        if let Some(a) = world.comp_mut::<Asset>(r.deck) {
            a.condition = a.condition.saturating_sub(cfg.fry_wear);
        }
        crate::systems::assets::rekit(world, runner);
    }
    let extract = r.phase == RunPhase::Extract;
    if extract {
        return_payload(world, &mut r);
    }
    let outcome = if dead {
        RunOutcome::Flatlined
    } else if fried {
        RunOutcome::Fried
    } else if traced {
        RunOutcome::Traced
    } else if extract {
        RunOutcome::Captured
    } else {
        RunOutcome::Bounced
    };
    if !dead {
        let until = world.tick + Tick::from(cfg.daze_ticks);
        if let Some(b) = world.comp_mut::<Brain>(runner) {
            b.dazed_until = Some(until);
        }
    }
    let snapshot = r.clone();
    end_run(world, r, outcome);
    if dead {
        let text = format!("{} flatlined in {}'s ICE", world.name_of(runner), node_label(world, n));
        world.push_event(EventKind::Flatlined, &[runner, owner.unwrap_or(EntityId::NONE)], text);
        world.kill_by(runner, crate::components::DeathCause::Flatline, None);
    }
    if traced {
        trace(world, &snapshot, n);
    }
}

/// V15 with V65's decay: the owner learns who sat where. Confidence `(1 −
/// trace_decay_per_hop)^hops`; at or above `trace_floor` a `Sighting` (the
/// chair's door) goes into the owner's `FactionDb`, and a corp or the city
/// files `DataTheft` (Data, Ledger) or `Intrusion` and sets `last_seen` to
/// the chair's door (the guards come; `last_trace` for the chair arrest);
/// a gang pushes `Shock::Hacked` naming the patron, else the runner's gang.
/// Below the floor the trace names nobody. A loud run (V66) raises the
/// node's alarm (`alarm_hours`).
pub fn trace(world: &mut World, r: &Run, n: NodeId) {
    let now = world.tick;
    let owner = owner_of(world, n);
    let tag = world.virt.node(n).map_or(OwnerTag::City, |x| x.owner_kind);
    let hops = i32::try_from(r.route.len().saturating_sub(1)).unwrap_or(i32::MAX);
    let confidence = (1.0 - world.config.ice.trace_decay_per_hop).max(0.0).powi(hops);
    let named = confidence >= world.config.ice.trace_floor;
    let door = world.comp::<Building>(r.chair).map(|b| b.door).unwrap_or_default();
    world.stats.current.virt.traced += 1;
    if named {
        let cap = world.config.db.db_cap.max(1);
        let db = world.db.entry(owner.unwrap_or(EntityId::NONE)).or_default();
        db.sightings.push_back(crate::virt::Sighting { who: r.runner, tile: door, tick: now, confidence });
        while db.sightings.len() > cap {
            db.sightings.pop_front();
        }
        world.stats.current.virt.sightings += 1;
    }
    if r.mode == RunMode::Loud {
        let until = now + Tick::from(world.config.ice.alarm_hours) * TICKS_PER_HOUR;
        if let Some(x) = world.virt.node_mut(n) {
            x.alarm_until = Some(until);
        }
        bump_epoch(world);
    }
    let alive = crate::systems::law::living(world, r.runner);
    match tag {
        OwnerTag::Gang => {
            let by = if named { r.patron.or_else(|| world.gang_of(r.runner)) } else { None };
            if let Some(g) = owner {
                crate::systems::gang::push_shock(world, g, crate::components::Shock::Hacked { by });
            }
        }
        OwnerTag::City | OwnerTag::Corp => {
            if named && alive {
                let crime = match r.purpose {
                    Purpose::Data { .. } | Purpose::Ledger => crate::components::Crime::DataTheft,
                    _ => crate::components::Crime::Intrusion,
                };
                crate::systems::law::file_report(world, crime, r.runner, None);
                world.last_seen.insert(r.runner, (door, now));
                world.last_trace.insert(r.runner, (r.chair, now));
            }
        }
    }
    let who = if named { world.name_of(r.runner) } else { "an unknown runner".to_string() };
    let text = format!("{}'s ICE traced {who} to {}", world.owner_label(owner), world.name_of(r.chair));
    let actors = [if named { r.runner } else { EntityId::NONE }, owner.unwrap_or(EntityId::NONE), r.chair];
    world.push_event(EventKind::Traced, &actors, text);
}

/// The payload back into the store it came from (Captured, Dumped).
fn return_payload(world: &mut World, r: &mut Run) {
    let payload = std::mem::take(&mut r.payload);
    if payload == [0; 3] {
        return;
    }
    if let Some(node) = r.source.and_then(|s| world.virt.node_mut(s)) {
        for (slot, p) in node.store.units.iter_mut().zip(payload) {
            *slot = slot.saturating_add(p);
        }
    }
}

/// Act (V29): a Data steal takes `steal_units[deck_eff − 1] × take` from
/// the target's store, the largest track first, into the payload. Wipes,
/// Ledgers, doors, robots and cameras take nothing here.
fn act_begin(world: &mut World, r: &mut Run) {
    if r.purpose != (Purpose::Data { wipe: false }) {
        return;
    }
    let (take, _, _) = mode_terms(world, r.mode);
    let mut budget = (steal_units(world, r.deck_eff) as f32 * take).round() as u32;
    let Some(node) = world.virt.node_mut(r.target) else { return };
    let mut order: SmallVec<[(std::cmp::Reverse<u32>, usize); 3]> =
        node.store.units.iter().enumerate().map(|(i, &u)| (std::cmp::Reverse(u), i)).collect();
    order.sort_unstable();
    for (_, i) in order {
        let got = node.store.units[i].min(budget);
        node.store.units[i] -= got;
        r.payload[i] += got;
        budget -= got;
    }
    r.source = Some(r.target);
}

/// The building a node's loss lands on for `note_loss`: its building, or a
/// corp Ledger's front (the owner's building with the highest value).
fn loss_building(world: &World, n: NodeId) -> Option<EntityId> {
    match world.virt.node(n)?.kind {
        NodeKind::Building(b) => Some(b),
        NodeKind::Ledger(c) => world
            .comp::<Corp>(c)?
            .buildings
            .iter()
            .filter_map(|&b| world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (b, bd.kind)))
            .map(|(b, kind)| (std::cmp::Reverse(ownership::value(world, kind)), b))
            .min()
            .map(|(_, b)| b),
        NodeKind::Public(_) => None,
    }
}

/// Out (the Purposes table, phase 2 rows): a Data steal's payload to the
/// patron's store (a gang's Hideout node, a corp's Lab of that focus) or the
/// deck (`Asset.data`), `DataStolen`, the owner's `loss_log`; a Ledger run
/// moves V20's take through `ownership::charge` (`Flow::Hack`, untaxed) to
/// the patron's purse or the runner's wallet, `LedgerHacked`, the loss. Each
/// success rings the node's breaches. Wipe, Door, Robot, Camera and
/// Overwatch are phase 3: nothing writes their orders before it, so their
/// arms do nothing here.
fn apply_out(world: &mut World, r: &mut Run) {
    let owner = owner_of(world, r.target);
    let label = node_label(world, r.target);
    let runner_name = world.name_of(r.runner);
    match r.purpose {
        Purpose::Data { wipe: false } => {
            let payload = std::mem::take(&mut r.payload);
            let total: u32 = payload.iter().sum();
            if total > 0 && !deliver(world, r, payload) {
                r.payload = payload;
                return_payload(world, r);
                return;
            }
            world.stats.current.virt.data_stolen += total;
            let value = i64::from(total) * world.config.data.data_price;
            if let Some(b) = loss_building(world, r.target).filter(|_| owner.is_some_and(|o| world.has::<Corp>(o))) {
                ownership::note_loss(world, b, value, Some(r.runner));
            }
            let track = Track::ALL.into_iter().max_by_key(|t| (payload[t.index()], std::cmp::Reverse(*t)));
            let for_whom = r.patron.map(|p| format!(" for {}", world.owner_label(Some(p)))).unwrap_or_default();
            let text =
                format!("{runner_name} lifted {total} {} Data from {label}{for_whom}", track.unwrap_or_default());
            let actors = [r.runner, owner.unwrap_or(EntityId::NONE), r.patron.unwrap_or(EntityId::NONE)];
            world.push_event(EventKind::DataStolen, &actors, text);
        }
        Purpose::Ledger => {
            let (take, _, _) = mode_terms(world, r.mode);
            let corp = owner.filter(|&o| world.has::<Corp>(o));
            let purse = world.purse(corp);
            let amount = (ledger_take(world, purse, r.deck_eff) as f32 * take).round() as i64;
            let to = r.patron.or(Some(r.runner));
            let moved = ownership::charge(world, corp, to, amount, Flow::Hack);
            if moved <= 0 {
                return;
            }
            world.stats.current.virt.ledger_hacks += 1;
            if let Some(b) = corp.and_then(|_| loss_building(world, r.target)) {
                ownership::note_loss(world, b, moved, Some(r.runner));
            }
            let text = format!("{runner_name} took {moved} from {}'s ledger", world.owner_label(corp));
            world.push_event(EventKind::LedgerHacked, &[r.runner, corp.unwrap_or(EntityId::NONE)], text);
        }
        // Phase 3 (V19, V32, V33, V37): never reached before its orders exist.
        Purpose::Data { wipe: true }
        | Purpose::Door
        | Purpose::Robot(_)
        | Purpose::Camera(_)
        | Purpose::Overwatch(_) => {
            return;
        }
    }
    let now = world.tick;
    if let Some(node) = world.virt.node_mut(r.target) {
        node.breaches.push_back(now);
        while node.breaches.len() > 8 {
            node.breaches.pop_front();
        }
    }
}

/// A steal's payload to where the run's patron keeps Data: a gang's Hideout
/// node, a corp's Lab of the richest track's focus (else its first Lab),
/// a freelancer's deck. False when there is nowhere to put it.
fn deliver(world: &mut World, r: &Run, payload: [u32; 3]) -> bool {
    let into_node = |world: &mut World, n: NodeId| {
        let Some(node) = world.virt.node_mut(n) else { return false };
        for (slot, p) in node.store.units.iter_mut().zip(payload) {
            *slot = slot.saturating_add(p);
        }
        true
    };
    match r.patron {
        Some(g) if world.has::<Gang>(g) => match world.hideout_of(g).and_then(|h| node_of_building(world, h)) {
            Some(n) => into_node(world, n),
            None => false,
        },
        Some(c) if world.has::<Corp>(c) => {
            let best = Track::ALL.into_iter().max_by_key(|t| (payload[t.index()], std::cmp::Reverse(*t)));
            let labs = crate::systems::tech::labs_of(world, c);
            let lab = labs
                .iter()
                .copied()
                .find(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.focus == best))
                .or_else(|| labs.first().copied());
            match lab.and_then(|b| node_of_building(world, b)) {
                Some(n) => into_node(world, n),
                None => false,
            }
        }
        _ => {
            let Some(a) = world.comp_mut::<Asset>(r.deck) else { return false };
            for (slot, p) in a.data.iter_mut().zip(payload) {
                *slot = slot.saturating_add(p);
            }
            true
        }
    }
}

/// V12: the seated body is pulled out (attacked, arrested, killed, gone):
/// `Dumpshock`, sanity − `dump_sanity`, the payload back to its store, the
/// run ends `Dumped`. A no-op for an agent not on a run.
pub fn dump(world: &mut World, runner: EntityId, why: &str) {
    let Some(id) = world.runner_of.get(&runner).copied() else { return };
    let Some(mut r) = world.runs.get(&id).cloned() else {
        world.runner_of.remove(&runner);
        return;
    };
    return_payload(world, &mut r);
    let hit = world.config.virt.dump_sanity;
    if let Some(b) = world.comp_mut::<crate::components::Body>(runner) {
        b.sanity = (b.sanity - hit).max(0.0);
    }
    let text = format!("{} was dumped from the plane ({why})", world.name_of(runner));
    world.push_event(EventKind::Dumpshock, &[runner], text);
    end_run(world, r, RunOutcome::Dumped);
}

/// V10/V35/V29: the run is over: out of `runs`, the queue and `runner_of`,
/// into `run_log` (64); the counters; hacking drift for a survivor (+
/// `hack_drift`, twice on a success); the Hack cooldown (`fried_cooldown_days`
/// after a fry, else `hack_cooldown_days`); a dazed body waits in the chair;
/// a freelancer's deck holding Data gets `GoTo(DataBuyer) → SellData`
/// appended; the `RunOrder` is dropped.
pub fn end_run(world: &mut World, mut r: Run, outcome: RunOutcome) {
    world.runs.remove(&r.id);
    world.run_queue.remove(&(r.next_at, r.id));
    if world.runner_of.get(&r.runner) == Some(&r.id) {
        world.runner_of.remove(&r.runner);
    }
    r.outcome = Some(outcome);
    let now = world.tick;
    let v = &mut world.stats.current.virt;
    v.runs += 1;
    match outcome {
        RunOutcome::Success => v.runs_ok += 1,
        RunOutcome::Bounced => v.runs_bounced += 1,
        RunOutcome::Captured => v.runs_captured += 1,
        RunOutcome::Dumped => v.runs_dumped += 1,
        RunOutcome::Traced | RunOutcome::Fried | RunOutcome::Flatlined => {}
    }
    let drift = world.config.decks.hack_drift * if outcome == RunOutcome::Success { 2.0 } else { 1.0 };
    if let Some(s) = world.comp_mut::<Skills>(r.runner) {
        s.hacking = (s.hacking.max(0.0) + drift).min(1.0);
    }
    let days = if outcome == RunOutcome::Fried {
        world.config.hack.fried_cooldown_days
    } else {
        world.config.hack.hack_cooldown_days
    };
    let cool = now + Tick::from(days) * TICKS_PER_DAY;
    let data = r.why == RunWhy::Freelance && outcome == RunOutcome::Success && deck_data(world, r.runner) > 0;
    let lab = if data { crate::systems::tech::data_buyer_lab(world, r.runner) } else { None };
    if let Some(b) = world.comp_mut::<Brain>(r.runner) {
        b.cooldowns.insert(GoalKind::Hack, cool);
        let seated = matches!(b.exec, crate::exec::ExecState::JackedIn { run, .. } if run == r.id);
        if seated {
            if let Some(until) = b.dazed_until.filter(|&t| t > now) {
                b.exec = crate::exec::ExecState::Wait { until };
            }
        }
        // V29: the sell-append (M13 D23's plan edit).
        if let (Some(lab), Some(plan)) = (lab, b.plan.as_mut().filter(|p| p.goal == GoalKind::Hack)) {
            use crate::goap::{ActionKind, LocationKey};
            let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
            plan.steps.push(step(ActionKind::GoTo(LocationKey::DataBuyer), Some(lab)));
            plan.steps.push(step(ActionKind::SellData, Some(lab)));
        }
    }
    world.run_orders.remove(&r.runner);
    world.run_log.push_back(r);
    while world.run_log.len() > 64 {
        world.run_log.pop_front();
    }
}

// ---------------------------------------------------------------------------
// Decks: gang buys (V30), corp fleet decks (V31), the snapshot (V43)
// ---------------------------------------------------------------------------

/// V24: the sellers of decks (Clinics and Security Offices), ascending.
pub fn deck_sellers(world: &World) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .chain(world.buildings_of_kind(BuildingKind::SecurityOffice))
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect();
    v.sort_unstable();
    v
}

/// The dearest new deck up to `max_tier` within `budget` across the deck
/// sellers (`can_sell`, priced at the seller's level), ties the cheaper
/// price, then nearest to `from`, then the lower id: `(seller, tier, price)`.
pub fn deck_offer(world: &World, from: TilePos, budget: i64, max_tier: u8) -> Option<(EntityId, u8, i64)> {
    type Key = (u8, std::cmp::Reverse<i64>, std::cmp::Reverse<u32>, std::cmp::Reverse<EntityId>);
    let mut best: Option<Key> = None;
    for s in deck_sellers(world) {
        let level = crate::systems::assets::seller_level(world, s);
        let dist = world.comp::<Building>(s).map_or(u32::MAX, |b| b.door.manhattan(from));
        for tier in 1..=max_tier.min(3) {
            let Some(list) = crate::systems::assets::list_price(world, AssetKind::Deck, tier) else { continue };
            let price = (list as f32 * level).round() as i64;
            if price > budget || !crate::systems::assets::can_sell(world, s, AssetKind::Deck, tier) {
                continue;
            }
            let key = (tier, std::cmp::Reverse(price), std::cmp::Reverse(dist), std::cmp::Reverse(s));
            if best.is_none_or(|b| key > b) {
                best = Some(key);
            }
        }
    }
    best.map(|(tier, std::cmp::Reverse(price), _, std::cmp::Reverse(s))| (s, tier, price))
}

/// V30, the gang buy step (after the bike and the Arms): one deck a day
/// while the treasury holds `gang_deck_floor` and the gang owns fewer than
/// `gang_decks_max`: the dearest tier priced within `treasury − floor`, for
/// the best-hacking member without one (at `hack_min` or better; ties the
/// lower id), owned by the gang, carried and kept by the member.
pub fn gang_deck(world: &mut World, gang: EntityId) {
    if !enabled(world) || !world.config.assets.enabled {
        return;
    }
    let floor = world.config.decks.gang_deck_floor;
    let treasury = world.purse(Some(gang));
    if treasury < floor {
        return;
    }
    let owned = crate::systems::assets::assets_of(world, Some(gang))
        .iter()
        .filter(|&&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Deck))
        .count();
    if owned >= world.config.decks.gang_decks_max as usize {
        return;
    }
    let min = world.config.hack.hack_min;
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let member = members
        .iter()
        .copied()
        .filter(|&m| world.has::<Brain>(m) && !world.has::<crate::components::Sentence>(m))
        .filter(|&m| world.comp::<Kit>(m).is_some_and(|k| k.deck.is_none()))
        .filter_map(|m| world.comp::<Skills>(m).map(|s| (s.hacking, m)))
        .filter(|&(h, _)| h >= min)
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, m)| m);
    let Some(member) = member else { return };
    let from = world.hideout_of(gang).and_then(|h| world.comp::<Building>(h)).map(|b| b.door).unwrap_or_default();
    let Some((seller, tier, _)) = deck_offer(world, from, treasury - floor, 3) else { return };
    let pick = crate::components::ShopPick { kind: AssetKind::Deck, tier, used: None, upgrade: false };
    let note = format!("for {}", world.name_of(member));
    if let Ok(a) = crate::systems::assets::buy_noted(world, gang, seller, &pick, Some(&note)) {
        crate::systems::assets::set_keeper(world, a, Some(member));
        crate::systems::assets::set_loc(world, a, AssetLoc::Carried(member));
    }
}

/// V31 (phase 2 part), daily under `Research`: one tier-1 fleet deck for
/// each of the corp's Labs with none posted (plan deviation: the tier is
/// left to phase 3's `VirtRaid`; a dearer pick spent 1,080 of Zetatech's day
/// 0 on two T3 decks, it then could not pay its Garage's truck imports and
/// the M13 gate's Farm trucks fell from 8 to 4), posted at the Lab
/// (`Posted`, not `Parked`: M13's street theft never sees it). Phase 3's `VirtRaid` lends it.
pub fn fleet_decks(world: &mut World, corp: EntityId) {
    if !enabled(world) || !world.config.assets.enabled {
        return;
    }
    for lab in crate::systems::tech::labs_of(world, corp) {
        let has = crate::systems::assets::assets_at(world, lab).iter().any(|&a| {
            world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Deck && x.loc == AssetLoc::Posted(lab))
        });
        if has {
            continue;
        }
        let budget = world.purse(Some(corp));
        let from = world.comp::<Building>(lab).map(|b| b.door).unwrap_or_default();
        let Some((seller, tier, _)) = deck_offer(world, from, budget, 1) else { return };
        let pick = crate::components::ShopPick { kind: AssetKind::Deck, tier, used: None, upgrade: false };
        let note = format!("for {}", world.name_of(lab));
        match crate::systems::assets::buy_noted(world, corp, seller, &pick, Some(&note)) {
            Ok(a) => crate::systems::assets::set_loc(world, a, AssetLoc::Posted(lab)),
            Err(_) => return,
        }
    }
}

/// V43: decks owned (any owner).
pub fn decks_owned(world: &World) -> u32 {
    world
        .assets_by_owner
        .values()
        .flatten()
        .filter(|&&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Deck))
        .count() as u32
}

/// V18: an arrest confiscates the Data on the suspect's deck (destroyed).
pub fn confiscate_data(world: &mut World, agent: EntityId) {
    if let Some(d) = world.comp::<Kit>(agent).and_then(|k| k.deck) {
        if let Some(a) = world.comp_mut::<Asset>(d) {
            a.data = [0; 3];
        }
    }
}
