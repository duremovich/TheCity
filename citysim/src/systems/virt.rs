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
/// other building's `ownership::value`. A Public node risks nothing.
///
/// Plan deviation (phase 3 fix round): the plan adds the value of the
/// assets posted at an "other" building. Runs only target Labs, Hideouts,
/// Ledgers and robots, so a Farm's posted truck lifted its target to 2 and
/// Secure spent 600 on ICE nobody contests: the Food corps' truck money in
/// days 0-30 (the M13 gate's truck bullets). A robbed building is still
/// raised through the loss term of [`ice_target`].
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
            ownership::value(world, bd.kind)
        }
    }
}

/// V27: one tier per `ice_value_steps` step the value at risk reaches,
/// plus (plan deviation, phase 3) one per Virt loss on the node in the
/// owner's last 14 days, at most two, capped at 3: a robbed Lab already at
/// its value target still gets raised (phase 5's "raised within 7 days").
pub fn ice_target(world: &World, n: NodeId) -> u8 {
    let v = value_at_risk(world, n);
    let base = world.config.ice.ice_value_steps.iter().filter(|&&s| v >= s).count();
    let horizon = world.tick.saturating_sub(14 * TICKS_PER_DAY);
    let robbed = owner_of(world, n)
        .and_then(|o| world.comp::<Corp>(o))
        .map_or(0, |c| c.virt_losses.iter().filter(|&&(t, x)| x == n && t >= horizon).count());
    (base + robbed.min(2)).min(3) as u8
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
/// V26: what raising `n` one tier costs `owner`: `(cost, payee, maker,
/// flow, tier)`; `None` at ICE 3, without a profile, or with no seller.
#[allow(clippy::type_complexity)]
fn ice_quote(
    world: &World,
    owner: Option<EntityId>,
    n: NodeId,
) -> Option<(i64, Option<EntityId>, Option<EntityId>, Flow, u8)> {
    let cur = profile(world, n).map(|p| p.ice)?;
    if cur >= 3 {
        return None;
    }
    let tier = cur + 1;
    let base = world.config.ice.price(tier)?;
    let seller = ice_seller(world, tier);
    let own_deck = owner.and_then(|o| world.comp::<Corp>(o)).map_or(0, |c| c.tech.tier_of(Track::Deck));
    if own_deck >= tier {
        let cheapest = seller.map_or(base, |(p, _)| p);
        let cost = (cheapest as f32 * world.config.ice.self_install_frac).round() as i64;
        Some((cost, None, owner, Flow::Import, tier))
    } else {
        let (price, s) = seller?;
        Some((price, Some(s), Some(s), Flow::Contract, tier))
    }
}

pub fn install_ice(world: &mut World, owner: Option<EntityId>, n: NodeId) -> bool {
    if !enabled(world) {
        return false;
    }
    let Some((cost, payee, maker, flow, tier)) = ice_quote(world, owner, n) else { return false };
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
    hack_offer(world, id, false)
}

/// V11 (phase 3): a gang, corp, prelude, overwatch or Stat order due within
/// the hour (`now ≥ not_before − 60`), not yet run: the agent's one order.
pub fn standing_order(world: &World, id: EntityId) -> Option<&RunOrder> {
    world
        .run_orders
        .get(&id)
        .filter(|o| o.why != RunWhy::Freelance)
        .filter(|o| world.tick + lead_of(o.why) >= o.not_before && o.expires > world.tick)
        .filter(|_| !world.runner_of.contains_key(&id))
}

/// How long before `not_before` an order is taken up: an hour (V11), the
/// Raid prelude four (plan deviation: a runner living across the map walked
/// to the Hideout chair after the muster had left).
fn lead_of(why: RunWhy) -> Tick {
    if why == RunWhy::Prelude {
        4 * TICKS_PER_HOUR
    } else {
        TICKS_PER_HOUR
    }
}

/// [`hack_choice`]; `stat` lets the Statistical pass (V38) score an agent
/// off screen. An agent holding a standing order (V11) scores the single
/// `order_score` instead of the freelance considerations.
pub fn hack_offer(world: &World, id: EntityId, stat: bool) -> Option<HackOffer> {
    use crate::utility::curves::{can, urgency, Curve, GATE};
    use crate::utility::Consideration;
    if !enabled(world) {
        return None;
    }
    let kit = world.comp::<Kit>(id)?;
    let deck = kit.deck?;
    let brain = world.comp::<Brain>(id)?;
    if (brain.lod == crate::components::Lod::Statistical && !stat)
        || world.has::<crate::components::Sentence>(id)
        || !crate::systems::demography::is_adult(world, id)
    {
        return None;
    }
    if let Some(o) = standing_order(world, id) {
        let considerations = vec![
            Consideration::new("ordered", can(true), GATE),
            Consideration::new("order", 1.0, Curve::Linear { m: world.config.hack.order_score, b: 0.0 }),
        ];
        let building = o.chair;
        return Some(HackOffer {
            sell: false,
            lab: None,
            portal: Some(Portal { building, node: portal(world, building, id, o.patron) }),
            target: o.target,
            purpose: o.purpose,
            ev: 0,
            route: None,
            mode: o.mode,
            considerations,
        });
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
    // Phase 3: a standing order keeps its own `RunOrder`.
    if standing_order(world, id).is_none() {
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
    }
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
    let from = portal(world, order.chair, runner, order.patron);
    let o = odds_of(world, runner, deck_eff, order.patron, order.mode);
    // V37: an Overwatch run stays on its portal (no route, no contest) and
    // ends when the raid it streams resolves.
    let overwatch = matches!(order.purpose, Purpose::Overwatch(_));
    let route = if overwatch {
        Route { nodes: SmallVec::from_elem(from, 1), p_success: 1.0, p_fry: 0.0, p_trace: 0.0 }
    } else {
        let tree = routes_from(world, from, deck_eff, o.att, order.patron);
        route_to(world, &tree, from, order.target, &o).ok_or(FailReason::PreconditionLost)?
    };
    let free = matches!(order.why, RunWhy::Freelance | RunWhy::Stat);
    if free && route.p_success < world.config.virt.min_route_p {
        return Err(FailReason::PreconditionLost);
    }
    // V32: a prelude whose raid is over (resolved, fizzled, called off) is moot.
    if order.why == RunWhy::Prelude && !raid_pending_for(world, order.patron) {
        world.run_orders.remove(&runner);
        return Err(FailReason::PreconditionLost);
    }
    let now = world.tick;
    let id = run_id(now, runner);
    let at_target = route.nodes.len() == 1 && !overwatch;
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
        phase: if overwatch {
            RunPhase::Out
        } else if at_target {
            RunPhase::BreakIn
        } else {
            RunPhase::Hop
        },
        next_at: if overwatch {
            now + TICKS_PER_HOUR
        } else if at_target {
            now
        } else {
            now + hop_ticks(world, deck_eff)
        },
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
            // V37: an Overwatch run checks hourly whether its raid is over
            // (resolved, called off, or past the march window).
            if let Purpose::Overwatch(g) = r.purpose {
                let live = world
                    .comp::<Gang>(g)
                    .and_then(|x| x.raid_at)
                    .is_some_and(|t| now < t + crate::systems::faction::RAID_MARCH_TICKS);
                if live {
                    r.next_at = now + TICKS_PER_HOUR;
                    world.run_queue.insert((r.next_at, id));
                    world.runs.insert(id, r);
                    return;
                }
            }
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
                // Phase 3 (V30): the VirtRaid `hacked` input and its wipe target.
                if let (Some(b), Some(gg)) = (by, world.comp_mut::<Gang>(g)) {
                    gg.hacked_by = Some((b, now));
                }
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
            note_virt_loss(world, r.target, value, r.runner);
            let track = Track::ALL.into_iter().max_by_key(|t| (payload[t.index()], std::cmp::Reverse(*t)));
            let for_whom = r.patron.map(|p| format!(" for {}", world.owner_label(Some(p)))).unwrap_or_default();
            let text =
                format!("{runner_name} lifted {total} {} Data from {label}{for_whom}", track.unwrap_or_default());
            // Phase 3: the node's building as a fourth actor (the ICE-after-loss report reads it).
            let building = loss_building(world, r.target).unwrap_or(EntityId::NONE);
            let actors = [r.runner, owner.unwrap_or(EntityId::NONE), r.patron.unwrap_or(EntityId::NONE), building];
            world.push_event(EventKind::DataStolen, &actors, text);
        }
        // V19: a wipe by a run.
        Purpose::Data { wipe: true } => {
            let value =
                world.virt.node(r.target).map_or(0, |x| i64::from(x.store.total())) * world.config.data.data_price;
            crate::systems::tech::wipe_store(world, r.target, Some(r.runner));
            note_virt_loss(world, r.target, value, r.runner);
        }
        // V32: the doors stand open until `raid_at + door_hours` (one per
        // `door_cooldown_days` per building).
        Purpose::Door => {
            let Some(b) = loss_building(world, r.target) else { return };
            // V32: the doors open for a raid still to come.
            if !raid_pending_for(world, r.patron) {
                return;
            }
            let now = world.tick;
            let cool = Tick::from(world.config.ice.door_cooldown_days) * TICKS_PER_DAY;
            if world.comp::<Building>(b).and_then(|x| x.last_door_open).is_some_and(|t| now.saturating_sub(t) < cool) {
                return;
            }
            let gang = r.patron.filter(|&p| world.has::<Gang>(p));
            let raid_at = gang.and_then(|g| world.comp::<Gang>(g)).and_then(|g| g.raid_at).unwrap_or(now);
            let until = raid_at.max(now) + Tick::from(world.config.hack.door_hours) * TICKS_PER_HOUR;
            set_hack(world, r.target, b, crate::virt::HackEffect::DoorOpen, until);
            if let Some(x) = world.comp_mut::<Building>(b) {
                x.last_door_open = Some(now);
            }
            world.stats.current.virt.doors_hacked += 1;
            let loss = world.config.data.door_loss;
            note_virt_loss(world, r.target, loss, r.runner);
            let gname = world.owner_label(gang);
            let text = format!(
                "{runner_name} opened {}'s doors for {gname} until {:02}:{:02}",
                world.name_of(b),
                (until % TICKS_PER_DAY) / TICKS_PER_HOUR,
                until % TICKS_PER_HOUR
            );
            world.push_event(EventKind::DoorHacked, &[r.runner, gang.unwrap_or(EntityId::NONE), b], text);
        }
        // V33: the robot fights for the patron for `hack_hours`.
        Purpose::Robot(a) => {
            let Some(b) = loss_building(world, r.target) else { return };
            let until = world.tick + Tick::from(world.config.robots.hack_hours) * TICKS_PER_HOUR;
            let side = r.patron.unwrap_or(r.runner);
            if let Some(x) = world.comp_mut::<Asset>(a) {
                x.turned = Some((side, until));
            }
            set_hack(world, r.target, b, crate::virt::HackEffect::RobotTurned(a), until);
            world.stats.current.virt.robots_turned += 1;
            let text = format!("{runner_name} turned the robot at {}", world.name_of(b));
            world.push_event(EventKind::RobotTurned, &[r.runner, b], text);
        }
        // V33: the building's cameras and robot sensors skip their contests.
        Purpose::Camera(_) => {
            let Some(b) = loss_building(world, r.target) else { return };
            let until = world.tick + Tick::from(world.config.hack.blind_hours) * TICKS_PER_HOUR;
            set_hack(world, r.target, b, crate::virt::HackEffect::Blind, until);
            world.stats.current.virt.blinded += 1;
            let text = format!("{runner_name} blinded {}'s sensors", world.name_of(b));
            world.push_event(EventKind::Blinded, &[r.runner, b], text);
        }
        // V37: the stream ends with the raid.
        Purpose::Overwatch(g) => {
            if let Some(x) = world.comp_mut::<Gang>(g) {
                if x.stream_by == Some(r.runner) {
                    x.stream_by = None;
                }
            }
            return;
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
            note_virt_loss(world, r.target, moved, r.runner);
            let text = format!("{runner_name} took {moved} from {}'s ledger", world.owner_label(corp));
            world.push_event(EventKind::LedgerHacked, &[r.runner, corp.unwrap_or(EntityId::NONE)], text);
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

/// V27 (phase 3): a Virt loss of `coins` on node `n`: a corp owner books it
/// (`ownership::note_loss` at the node's building: `loss_log` and the
/// `Robbed` shock) and marks the node in `Corp.virt_losses` (14 days); a
/// gang owner's members remember it (`MemoryKind::Hacked`).
fn note_virt_loss(world: &mut World, n: NodeId, coins: i64, runner: EntityId) {
    let owner = owner_of(world, n);
    match world.virt.node(n).map(|x| x.owner_kind) {
        Some(OwnerTag::Corp) => {
            let Some(corp) = owner else { return };
            if let Some(b) = loss_building(world, n) {
                ownership::note_loss(world, b, coins, Some(runner));
            }
            let now = world.tick;
            let horizon = now.saturating_sub(14 * TICKS_PER_DAY);
            if let Some(c) = world.comp_mut::<Corp>(corp) {
                c.virt_losses.push_back((now, n));
                while c.virt_losses.front().is_some_and(|&(t, _)| t < horizon) {
                    c.virt_losses.pop_front();
                }
            }
        }
        Some(OwnerTag::Gang) => {
            let members = owner.and_then(|g| world.comp::<Gang>(g)).map(|g| g.members.clone()).unwrap_or_default();
            for m in members {
                world.remember(m, crate::components::MemoryKind::Hacked, None, 0.5, -0.4, false);
            }
        }
        _ => {}
    }
}

/// V19: a sacked Hideout's store moves to `winner`'s Hideout node.
pub fn move_store(world: &mut World, from: EntityId, winner: EntityId) {
    if !enabled(world) {
        return;
    }
    let (Some(a), Some(b)) =
        (node_of_building(world, from), world.hideout_of(winner).and_then(|h| node_of_building(world, h)))
    else {
        return;
    };
    let cap = world.config.data.store_cap;
    let Some(units) = world.virt.node_mut(a).map(|x| std::mem::take(&mut x.store.units)) else { return };
    if let Some(x) = world.virt.node_mut(b) {
        for (slot, u) in x.store.units.iter_mut().zip(units) {
            *slot = slot.saturating_add(u).min(cap.max(*slot));
        }
    }
}

/// V32: `gang` has a raid scheduled (`raid_at` set) that has not resolved.
fn raid_pending_for(world: &World, gang: Option<EntityId>) -> bool {
    gang.and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| g.raid_at.is_some())
}

/// V32/V33: a hack on building `b` and its node until `until`.
fn set_hack(world: &mut World, n: NodeId, b: EntityId, effect: crate::virt::HackEffect, until: Tick) {
    if let Some(x) = world.comp_mut::<Building>(b) {
        x.hacked = Some((effect, until));
    }
    if let Some(x) = world.virt.node_mut(n) {
        x.hacked = Some((effect, until));
    }
    bump_epoch(world);
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
    // V37 (plan deviation): an Overwatch stream is no run for the counters
    // or the drift (no contest happens on it).
    let watch = matches!(r.purpose, Purpose::Overwatch(_));
    let v = &mut world.stats.current.virt;
    if !watch {
        v.runs += 1;
        match outcome {
            RunOutcome::Success => v.runs_ok += 1,
            RunOutcome::Bounced => v.runs_bounced += 1,
            RunOutcome::Captured => v.runs_captured += 1,
            RunOutcome::Dumped => v.runs_dumped += 1,
            RunOutcome::Traced | RunOutcome::Fried | RunOutcome::Flatlined => {}
        }
    }
    let base = if watch { 0.0 } else { world.config.decks.hack_drift };
    let drift = base * if outcome == RunOutcome::Success { 2.0 } else { 1.0 };
    if let Some(s) = world.comp_mut::<Skills>(r.runner) {
        s.hacking = (s.hacking.max(0.0) + drift).min(1.0);
    }
    let days = if outcome == RunOutcome::Fried {
        world.config.hack.fried_cooldown_days
    } else {
        world.config.hack.hack_cooldown_days
    };
    let cool = now + Tick::from(days) * TICKS_PER_DAY;
    let free = matches!(r.why, RunWhy::Freelance | RunWhy::Stat);
    let data = free && outcome == RunOutcome::Success && deck_data(world, r.runner) > 0;
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
    let lent = world.run_orders.remove(&r.runner).is_some_and(|o| o.why == RunWhy::CorpOrder);
    if lent {
        // V31: the fleet deck goes back to the Lab.
        crate::systems::assets::rekit(world, r.runner);
    }
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

// ---------------------------------------------------------------------------
// Phase 3: orders and targets (plan V13, V27, V28, V30-V33, V37, V38)
// ---------------------------------------------------------------------------

/// Days a traced run that named someone keeps a gang's `hacked` input up.
const HACKED_DAYS: Tick = 7;

/// V13: write `order` for `runner`, promote a Statistical runner to Coarse
/// at once, and re-kit (a corp order lends the Lab's fleet deck).
pub fn give_order(world: &mut World, runner: EntityId, order: RunOrder) {
    world.run_orders.insert(runner, order);
    if world.comp::<Brain>(runner).is_some_and(|b| b.lod == crate::components::Lod::Statistical) {
        crate::systems::lod::set_lod(world, runner, crate::components::Lod::Coarse);
    }
    crate::systems::assets::rekit(world, runner);
}

/// V30: a gang's members able to run, best hacking first (ties the lower
/// id): a deck in the Kit, `hacking ≥ hack_min`, free, not on a run.
pub fn gang_runners(world: &World, gang: EntityId) -> Vec<EntityId> {
    let min = world.config.hack.hack_min;
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let mut v: Vec<(f32, EntityId)> = g
        .members
        .iter()
        .copied()
        .filter(|&m| crate::systems::law::living(world, m) && !world.has::<crate::components::Sentence>(m))
        .filter(|&m| !world.runner_of.contains_key(&m))
        .filter(|&m| world.comp::<Kit>(m).is_some_and(|k| k.deck.is_some() && k.deck_tier > 0))
        .filter_map(|m| world.comp::<Skills>(m).map(|s| (s.hacking, m)))
        .filter(|&(h, _)| h >= min)
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    v.into_iter().map(|(_, m)| m).collect()
}

/// V30: who a traced run on the gang's nodes named lately (`Some(None)`: a
/// pending `Shock::Hacked` naming nobody).
fn gang_hacked(world: &World, gang: EntityId) -> Option<Option<EntityId>> {
    let g = world.comp::<Gang>(gang)?;
    let window = HACKED_DAYS * TICKS_PER_DAY;
    if let Some((by, _)) = g.hacked_by.filter(|&(_, t)| world.tick.saturating_sub(t) < window) {
        return Some(Some(by));
    }
    g.shocks.iter().any(|s| matches!(s, crate::components::Shock::Hacked { .. })).then_some(None)
}

/// V30: a gang runner's candidates: rival Hideout stores and corp Labs
/// (steal), the hoarding corp's Ledger (a deck at the Trunk tier), the
/// robot posted at the corp-raid prize building, and a wipe of whoever a
/// trace named (a corp's richest Lab, a gang's Hideout store).
fn gang_candidates(
    world: &World,
    gang: EntityId,
    deck_eff: u8,
    mode: RunMode,
    prize: Option<EntityId>,
) -> SmallVec<[(NodeId, Purpose, i64); 32]> {
    let mut out: SmallVec<[(NodeId, Purpose, i64); 32]> = SmallVec::new();
    let (take, _, _) = mode_terms(world, mode);
    let price = world.config.data.data_price;
    let units = steal_units(world, deck_eff);
    for (i, n) in world.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive && n.owner != Some(gang)) {
        let NodeKind::Building(b) = n.kind else { continue };
        let kind = world.comp::<Building>(b).map(|bd| bd.kind);
        let fits = match n.owner_kind {
            OwnerTag::Gang => kind == Some(BuildingKind::Hideout),
            OwnerTag::Corp => kind == Some(BuildingKind::Lab),
            OwnerTag::City => false,
        };
        if !fits || n.store.total() == 0 {
            continue;
        }
        let got = (n.store.total().min(units) as f32 * take).round() as i64;
        if got > 0 {
            out.push((NodeId(i as u16), Purpose::Data { wipe: false }, got * price));
        }
    }
    if deck_eff >= world.config.virt.trunk_links_tier {
        if let Some(c) = crate::systems::faction::hoard(world).1 {
            if let Some(l) = ledger_of(world, c) {
                let got = (ledger_take(world, world.purse(Some(c)), deck_eff) as f32 * take).round() as i64;
                if got > 0 {
                    out.push((l, Purpose::Ledger, got));
                }
            }
        }
    }
    if let Some(b) = prize {
        if let (Some(r), Some(n)) = (crate::systems::robots::powered_robot(world, b), node_of_building(world, b)) {
            let v = world.comp::<Asset>(r).map_or(0, |x| x.value) / 2;
            out.push((n, Purpose::Robot(r), v.max(1)));
        }
    }
    match gang_hacked(world, gang) {
        Some(Some(by)) if world.has::<Corp>(by) => {
            let lab = crate::systems::tech::labs_of(world, by)
                .into_iter()
                .filter_map(|l| node_of_building(world, l))
                .filter_map(|n| world.virt.node(n).map(|x| (x.store.total(), std::cmp::Reverse(n))))
                .max()
                .filter(|&(u, _)| u > 0);
            if let Some((u, std::cmp::Reverse(n))) = lab {
                out.push((n, Purpose::Data { wipe: true }, i64::from(u) * price));
            }
        }
        Some(Some(by)) if world.has::<Gang>(by) && by != gang => {
            if let Some(n) = world.hideout_of(by).and_then(|h| node_of_building(world, h)) {
                let u = world.virt.node(n).map_or(0, |x| x.store.total());
                if u > 0 {
                    out.push((n, Purpose::Data { wipe: true }, i64::from(u) * price));
                }
            }
        }
        _ => {}
    }
    out
}

/// One ranked target: `(EV × p, node, purpose, EV, p_success)`.
type Ranked = (f32, NodeId, Purpose, i64, f32);

/// V6/V30: the candidates read off one search from `from`, at `p_success ≥
/// min_route_p`, best `EV × p` first (ties the lower `NodeId`).
fn rank(world: &World, from: NodeId, o: &Odds, deck_eff: u8, cands: &[(NodeId, Purpose, i64)]) -> Vec<Ranked> {
    let tree = routes_from(world, from, deck_eff, o.att, o.patron);
    let min = world.config.virt.min_route_p;
    let mut v: Vec<Ranked> = cands
        .iter()
        .filter(|&&(n, _, _)| n != from)
        .filter_map(|&(n, p, ev)| route_to(world, &tree, from, n, o).map(|r| (n, p, ev, r.p_success)))
        .filter(|&(_, _, _, ps)| ps >= min)
        .map(|(n, p, ev, ps)| (ev as f32 * ps, n, p, ev, ps))
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    v
}

/// V30 inputs: `(runner, EV ÷ hoard_heat, p_success, hacked)` for the
/// best target of the gang's best runner from its Hideout node.
pub fn gang_virt_inputs(world: &World, gang: EntityId, prize: Option<EntityId>) -> (Option<EntityId>, f32, f32, bool) {
    if !enabled(world) {
        return (None, 0.0, 0.0, false);
    }
    let hacked = gang_hacked(world, gang).is_some();
    let Some(runner) = gang_runners(world, gang).first().copied() else { return (None, 0.0, 0.0, hacked) };
    let Some(from) = world.hideout_of(gang).and_then(|h| node_of_building(world, h)) else {
        return (Some(runner), 0.0, 0.0, hacked);
    };
    let deck_eff = world.comp::<Kit>(runner).map_or(0, |k| k.deck_tier);
    let mode = mode_of(world, runner);
    let o = odds_of(world, runner, deck_eff, Some(gang), mode);
    let cands = gang_candidates(world, gang, deck_eff, mode, prize);
    let best = rank(world, from, &o, deck_eff, &cands).into_iter().next();
    let heat = world.config.corps.hoard_heat.max(1) as f32;
    match best {
        Some((_, _, _, ev, p)) => (Some(runner), (ev as f32 / heat).clamp(0.0, 1.0), p, hacked),
        None => (Some(runner), 0.0, 0.0, hacked),
    }
}

/// V30, daily in the gang economy: tier-1 ICE on the Hideout node while its
/// store is worth `ice_value_steps[0]`; then, under `VirtRaid`, up to
/// `virt_runners` runners each get a `RunOrder` (chair the Hideout, patron
/// the gang) on a distinct target, best first.
pub fn gang_daily(world: &mut World, gang: EntityId) {
    if !enabled(world) {
        return;
    }
    let Some(hideout) = world.hideout_of(gang) else { return };
    let Some(from) = node_of_building(world, hideout) else { return };
    let worth = value_at_risk(world, from);
    let step = world.config.ice.ice_value_steps.first().copied().unwrap_or(i64::MAX);
    if worth >= step && profile(world, from).is_some_and(|p| p.ice == 0) {
        install_ice(world, Some(gang), from);
    }
    if world.comp::<Gang>(gang).is_none_or(|g| g.order != crate::components::Order::VirtRaid) {
        return;
    }
    let prize = crate::systems::raid::corp_target(world, gang);
    let now = world.tick;
    let mut taken: Vec<NodeId> = Vec::new();
    let max = world.config.hack.virt_runners as usize;
    let runners: Vec<EntityId> =
        gang_runners(world, gang).into_iter().filter(|r| !world.run_orders.contains_key(r)).take(max).collect();
    for runner in runners {
        let deck_eff = world.comp::<Kit>(runner).map_or(0, |k| k.deck_tier);
        let mode = mode_of(world, runner);
        let o = odds_of(world, runner, deck_eff, Some(gang), mode);
        let cands = gang_candidates(world, gang, deck_eff, mode, prize);
        let pick = rank(world, from, &o, deck_eff, &cands).into_iter().find(|t| !taken.contains(&t.1));
        let Some((_, target, purpose, _, _)) = pick else { continue };
        taken.push(target);
        let order = RunOrder {
            patron: Some(gang),
            purpose,
            target,
            chair: hideout,
            not_before: now,
            expires: now + TICKS_PER_DAY,
            why: RunWhy::GangOrder,
            mode,
        };
        give_order(world, runner, order);
    }
}

/// V32, when M12's Raid names a corp building and the muster: the gang's
/// best runner gets a `Robot` run (a powered robot posted there) or a `Door`
/// run (ICE ≥ 1 or a contract; not within `door_cooldown_days` of the last
/// DoorOpen), from the Hideout chair, `door_lead_hours` before the muster.
pub fn raid_prelude(world: &mut World, gang: EntityId, target: EntityId, raid_at: Tick) {
    if !enabled(world) {
        return;
    }
    let Some(n) = node_of_building(world, target) else { return };
    let Some(hideout) = world.hideout_of(gang) else { return };
    let robot = crate::systems::robots::powered_robot(world, target);
    let (ice, contract, last) = world
        .comp::<Building>(target)
        .map_or((0, false, None), |b| (b.security.ice, b.secured_by.is_some(), b.last_door_open));
    let cool = Tick::from(world.config.ice.door_cooldown_days) * TICKS_PER_DAY;
    let door_ok = (ice >= 1 || contract) && last.is_none_or(|t| world.tick.saturating_sub(t) >= cool);
    let purpose = match robot {
        Some(r) => Purpose::Robot(r),
        None if door_ok => Purpose::Door,
        None => return,
    };
    let Some(runner) = gang_runners(world, gang).first().copied() else { return };
    let lead = Tick::from(world.config.hack.door_lead_hours) * TICKS_PER_HOUR;
    let order = RunOrder {
        patron: Some(gang),
        purpose,
        target: n,
        chair: hideout,
        not_before: raid_at.saturating_sub(lead).max(world.tick),
        expires: raid_at + Tick::from(world.config.hack.door_hours) * TICKS_PER_HOUR,
        why: RunWhy::Prelude,
        mode: RunMode::Quiet,
    };
    give_order(world, runner, order);
}

/// V37, the first member out of the muster: a member with a deck and
/// `hacking ≥ stream_min` inside the Hideout and not marching takes an
/// `Overwatch` run; `Gang.stream_by`. A corp raid leaving through an open
/// door logs it (the V32 gate pair).
pub fn on_raid_departed(world: &mut World, gang: EntityId) {
    if !enabled(world) {
        return;
    }
    let now = world.tick;
    if let Some(t) = crate::systems::raid::corp_target(world, gang) {
        if hack_live(world, t, crate::virt::HackEffect::DoorOpen) {
            let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
            let text = format!("{gname}'s raid departs for {} through open doors", world.name_of(t));
            world.push_event(EventKind::Raid, &[gang, t], text);
        }
    }
    let Some(hideout) = world.hideout_of(gang) else { return };
    let min = world.config.hack.stream_min;
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let streamer = members.into_iter().find(|&m| {
        crate::systems::law::living(world, m)
            && !world.runner_of.contains_key(&m)
            && !world.run_orders.contains_key(&m)
            && world.comp::<Kit>(m).is_some_and(|k| k.deck.is_some())
            && world.comp::<Skills>(m).is_some_and(|s| s.hacking >= min)
            && world.comp::<Position>(m).is_some_and(|p| p.building == Some(hideout))
            && world.comp::<Brain>(m).is_some_and(|b| b.current_goal != Some(GoalKind::Raid))
    });
    let Some(m) = streamer else { return };
    let order = RunOrder {
        patron: Some(gang),
        purpose: Purpose::Overwatch(gang),
        target: node_of_building(world, hideout).unwrap_or_default(),
        chair: hideout,
        not_before: now,
        expires: now + crate::systems::faction::RAID_MARCH_TICKS,
        why: RunWhy::Overwatch,
        mode: RunMode::Quiet,
    };
    give_order(world, m, order);
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.stream_by = Some(m);
    }
}

/// The tier of the fleet deck a corp's VirtRaid buys when its Lab has none
/// (plan deviation: phase 2's tier-1 fleet decks cannot clear `min_route_p`
/// against a rival Lab at ICE 2).
const RAID_DECK_TIER: u8 = 2;

/// A corp's run for the day (V31).
#[derive(Clone, Debug)]
pub struct CorpRun {
    pub lab: EntityId,
    /// The fleet deck posted at the Lab; `None`: the act buys one first.
    pub deck: Option<EntityId>,
    pub runner: EntityId,
    pub target: NodeId,
    pub purpose: Purpose,
    pub p: f32,
}

/// The fleet deck posted at `lab` (condition > 0), the lowest id.
pub fn fleet_deck_at(world: &World, lab: EntityId) -> Option<EntityId> {
    crate::systems::assets::assets_at(world, lab).iter().copied().find(|&a| {
        world
            .comp::<Asset>(a)
            .is_some_and(|x| x.kind == AssetKind::Deck && x.loc == AssetLoc::Posted(lab) && x.condition > 0)
    })
}

/// V31: the corp's run for today, if it has a Lab with a fleet deck, a
/// Researcher there and a niche rival holding Data in its focus (the rival
/// with the most, its Lab with the most in that track): a wipe when the
/// rival's tier there exceeds its own and `1 − lawfulness ≥ wipe_min`, else
/// a steal. Plan deviation: the runner is the best Researcher employed at
/// the Lab, on shift or not (the act runs at midnight).
pub fn corp_run(world: &World, corp: EntityId) -> Option<CorpRun> {
    if !enabled(world) {
        return None;
    }
    let c = world.comp::<Corp>(corp)?;
    let focus = c.tech.focus;
    let staff = |lab: EntityId| {
        world
            .workers(crate::components::Role::Researcher)
            .iter()
            .copied()
            .filter(|&r| world.comp::<crate::components::Job>(r).and_then(|j| j.employer) == Some(lab))
            .filter(|&r| crate::systems::law::living(world, r) && !world.has::<crate::components::Sentence>(r))
            .filter(|&r| !world.runner_of.contains_key(&r))
            .filter_map(|r| world.comp::<Skills>(r).map(|s| (s.hacking, r)))
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(_, r)| r)
    };
    // A Lab with a fleet deck first, else any staffed Lab (ascending).
    let labs = crate::systems::tech::labs_of(world, corp);
    let (lab, deck, runner) = labs
        .iter()
        .find_map(|&l| Some((l, Some(fleet_deck_at(world, l)?), staff(l)?)))
        .or_else(|| labs.iter().find_map(|&l| Some((l, None, staff(l)?))))?;
    let from = node_of_building(world, lab)?;
    let rival = world
        .corps()
        .into_iter()
        .filter(|&r| r != corp)
        .filter(|&r| world.comp::<Corp>(r).is_some_and(|rc| rc.niches.iter().any(|n| c.niches.contains(n))))
        .map(|r| (holding(world, r, focus), std::cmp::Reverse(r)))
        .filter(|&(h, _)| h > 0)
        .max()
        .map(|(_, std::cmp::Reverse(r))| r)?;
    let target = crate::systems::tech::labs_of(world, rival)
        .into_iter()
        .filter_map(|l| node_of_building(world, l))
        .filter_map(|n| world.virt.node(n).map(|x| (x.store.get(focus), std::cmp::Reverse(n))))
        .filter(|&(u, _)| u > 0)
        .max()
        .map(|(_, std::cmp::Reverse(n))| n)?;
    let law = c.exec.and_then(|e| world.comp::<Personality>(e)).map_or(0.5, |p| p.lawfulness);
    let behind = maker_tier(world, rival, focus) > c.tech.tier_of(focus);
    let wipe = behind && 1.0 - law >= world.config.hack.wipe_min;
    let deck_eff = deck
        .and_then(|d| world.comp::<Asset>(d))
        .map_or(RAID_DECK_TIER, |x| crate::systems::assets::eff_tier_of(world, x).max(1));
    let mode = if wipe { RunMode::Loud } else { RunMode::Quiet };
    let o = odds_of(world, runner, deck_eff, Some(corp), mode);
    let tree = routes_from(world, from, deck_eff, o.att, Some(corp));
    let p = route_to(world, &tree, from, target, &o).map_or(0.0, |r| r.p_success);
    Some(CorpRun { lab, deck, runner, target, purpose: Purpose::Data { wipe }, p })
}

/// V31, daily under `VirtRaid`: one `RunOrder` from the Lab chair (the
/// fleet deck is lent through the Kit while the order stands). The order's
/// one spend: a fleet deck up to `RAID_DECK_TIER` for a Lab without one,
/// paid from above the fleet reserve (`treasury_ref / 4`, as Secure's ICE
/// and cameras), logged "for {Lab} (VirtRaid)".
pub fn corp_virt_raid(world: &mut World, corp: EntityId) {
    let Some(run) = corp_run(world, corp) else { return };
    if world.run_orders.contains_key(&run.runner) {
        return;
    }
    // The order equips its Lab: a fleet deck up to `RAID_DECK_TIER`.
    if run.deck.is_none() {
        let from = world.comp::<Building>(run.lab).map(|b| b.door).unwrap_or_default();
        let reserve = world.comp::<Corp>(corp).map_or(0, |c| c.treasury_ref / 4);
        let budget = world.purse(Some(corp)) - reserve;
        let Some((seller, tier, _)) = deck_offer(world, from, budget, RAID_DECK_TIER) else { return };
        let pick = crate::components::ShopPick { kind: AssetKind::Deck, tier, used: None, upgrade: false };
        let note = format!("for {} (VirtRaid)", world.name_of(run.lab));
        match crate::systems::assets::buy_noted(world, corp, seller, &pick, Some(&note)) {
            Ok(a) => crate::systems::assets::set_loc(world, a, AssetLoc::Posted(run.lab)),
            Err(_) => return,
        }
    }
    let now = world.tick;
    let wipe = matches!(run.purpose, Purpose::Data { wipe: true });
    let order = RunOrder {
        patron: Some(corp),
        purpose: run.purpose,
        target: run.target,
        chair: run.lab,
        not_before: now,
        expires: now + TICKS_PER_DAY,
        why: RunWhy::CorpOrder,
        mode: if wipe { RunMode::Loud } else { RunMode::Quiet },
    };
    give_order(world, run.runner, order);
}

/// The corp's alive nodes (building nodes and its Ledger), ascending.
fn corp_nodes(world: &World, corp: EntityId) -> Vec<NodeId> {
    world
        .virt
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.alive && n.owner == Some(corp) && !matches!(n.kind, NodeKind::Public(_)))
        .map(|(i, _)| NodeId(i as u16))
        .collect()
}

/// Phase 3 fix round: can `corp` pay `cost` for a Secure spend (ICE, a
/// camera) while its purse holds `mult × cost` and keeps M13's fleet-buy
/// reserve (`treasury_ref / 4`, the reserve the Data budget keeps) after
/// it? Without the reserve Secure's ICE and cameras ate the Food corps'
/// truck money in days 0-30 (the M13 gate's truck bullets).
pub fn secure_affords(world: &World, corp: EntityId, cost: i64, mult: f32) -> bool {
    let purse = world.purse(Some(corp));
    let reserve = world.comp::<Corp>(corp).map_or(0, |c| c.treasury_ref / 4);
    purse as f32 >= mult * cost as f32 && purse - cost >= reserve
}

/// V27 Secure (after the contract and robot pass): raise ICE one tier on
/// up to `secure_per_day` owned nodes with `ice_target − ice_eff > 0`:
/// nodes with a Virt loss in 14 days first (newest first), then the
/// largest gap (ties the lower id). Nodes the corp cannot afford
/// ([`secure_affords`]) are skipped before the `secure_per_day` take.
pub fn secure_ice(world: &mut World, corp: EntityId) {
    if !enabled(world) {
        return;
    }
    let per_day = world.config.corps.secure_per_day;
    let horizon = world.tick.saturating_sub(14 * TICKS_PER_DAY);
    let recent: Vec<NodeId> = world
        .comp::<Corp>(corp)
        .map(|c| c.virt_losses.iter().rev().filter(|&&(t, _)| t >= horizon).map(|&(_, n)| n).collect())
        .unwrap_or_default();
    let gap = |w: &World, n: NodeId| i32::from(ice_target(w, n)) - i32::from(ice_eff(w, n));
    let mut order: Vec<NodeId> = Vec::new();
    for n in recent {
        if !order.contains(&n) && owner_of(world, n) == Some(corp) && gap(world, n) > 0 {
            order.push(n);
        }
    }
    let mut rest: Vec<(std::cmp::Reverse<i32>, NodeId)> = corp_nodes(world, corp)
        .into_iter()
        .filter(|n| !order.contains(n))
        .map(|n| (std::cmp::Reverse(gap(world, n)), n))
        .filter(|&(std::cmp::Reverse(g), _)| g > 0)
        .collect();
    rest.sort_unstable();
    order.extend(rest.into_iter().map(|(_, n)| n));
    // Plan deviation: M13 D42's cash rule for Secure's robots holds for its
    // ICE (the treasury keeps `robot_cash_mult` x the price): with the loss
    // term in `ice_target`, Militech bought ICE 3 for its robbed Lab from
    // Arasaka (2,000) on day 7 and went bankrupt on day 11 (HEAD: 89).
    let mult = world.config.robots.robot_cash_mult;
    let mut raised = 0;
    for n in order {
        if raised >= per_day {
            break;
        }
        let Some((cost, ..)) = ice_quote(world, Some(corp), n) else { continue };
        if !secure_affords(world, corp, cost, mult) {
            continue;
        }
        if install_ice(world, Some(corp), n) {
            raised += 1;
        }
    }
}

/// A camera posted at `b` (condition > 0, upkeep paid), the lowest id.
pub fn camera_at(world: &World, b: EntityId) -> Option<EntityId> {
    crate::systems::assets::assets_at(world, b).iter().copied().find(|&a| {
        world.comp::<Asset>(a).is_some_and(|x| {
            x.kind == AssetKind::Camera && x.loc == AssetLoc::Posted(b) && x.condition > 0 && x.upkeep_arrears == 0
        })
    })
}

/// V27/V28 Secure: a tier-1 camera for the owned building (lowest id) with
/// no sensor (no robot, no camera) and `ice_target ≥ 1`, from the cheapest
/// Security Office that can sell it, while the treasury holds twice the
/// price and keeps the fleet reserve after it ([`secure_affords`]).
pub fn buy_camera(world: &mut World, corp: EntityId) {
    if !enabled(world) || !world.config.assets.enabled {
        return;
    }
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let want = c.buildings.iter().copied().find(|&b| {
        world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
            && crate::systems::robots::posted_robot(world, b).is_none()
            && camera_at(world, b).is_none()
            && node_of_building(world, b).is_some_and(|n| ice_target(world, n) >= 1)
    });
    let Some(b) = want else { return };
    let Some(list) = crate::systems::assets::list_price(world, AssetKind::Camera, 1) else { return };
    let office = world
        .buildings_of_kind(BuildingKind::SecurityOffice)
        .iter()
        .copied()
        .filter(|&o| crate::systems::assets::can_sell(world, o, AssetKind::Camera, 1))
        .map(|o| ((list as f32 * crate::systems::assets::seller_level(world, o)).round() as i64, o))
        .min();
    let Some((price, office)) = office else { return };
    if !secure_affords(world, corp, price, 2.0) {
        return;
    }
    let pick = crate::components::ShopPick { kind: AssetKind::Camera, tier: 1, used: None, upgrade: false };
    let note = format!("for Secure ({})", world.name_of(b));
    if let Ok(a) = crate::systems::assets::buy_noted(world, corp, office, &pick, Some(&note)) {
        crate::systems::assets::set_loc(world, a, AssetLoc::Posted(b));
    }
}

/// Days of upkeep a Hunker shed must save to repay re-buying the tier
/// (the corp's `ice_spend` window).
const HUNKER_PAYBACK_DAYS: i64 = 30;

/// V27 Hunker: while the purse is below the fleet reserve (`treasury_ref /
/// 4`), lower ICE one tier on an owned node whose tier saves, over
/// `HUNKER_PAYBACK_DAYS`, at least what re-installing it would cost
/// (`ice_price × self_install_frac`, the cheapest rebuy): the most excess
/// over `ice_target` first, then the largest saving, ties the lower id.
///
/// Plan deviation (phase 3 fix round): the plan sheds the node with the
/// most excess over `ice_target`, cash or not. With the loss term in
/// `ice_target` (14 days) and a Ledger's target following its purse, a
/// target fell, Hunker shed the tier and Secure bought it back days later:
/// on seed 42 Greenline's seeded Ledger ICE 2 was shed on day 1 (saving 1
/// coin a day), re-bought for 600 on day 5 and shed again on day 8, the
/// money its day-5 truck took at HEAD. `ice_target` now governs buys only;
/// at today's `ice_upkeep` [0, 0, 1, 2] no shed repays itself, so Hunker
/// keeps its ICE until phase 5 gives upkeep teeth.
pub fn hunker_ice(world: &mut World, corp: EntityId) {
    if !enabled(world) {
        return;
    }
    let reserve = world.comp::<Corp>(corp).map_or(0, |c| c.treasury_ref / 4);
    if world.purse(Some(corp)) >= reserve {
        return;
    }
    let cfg = &world.config.ice;
    let pick = corp_nodes(world, corp)
        .into_iter()
        .filter_map(|n| {
            let ice = profile(world, n).map(|p| p.ice).filter(|&i| i > 0)?;
            let saving = cfg.upkeep(ice) - cfg.upkeep(ice - 1);
            let rebuy = (cfg.price(ice).unwrap_or(0) as f32 * cfg.self_install_frac).round() as i64;
            let excess = i32::from(ice) - i32::from(ice_target(world, n));
            (saving > 0 && saving * HUNKER_PAYBACK_DAYS >= rebuy).then_some((excess, saving, std::cmp::Reverse(n)))
        })
        .max();
    if let Some((_, _, std::cmp::Reverse(n))) = pick {
        lower_ice(world, n);
    }
}

/// V33: a live hack of `effect` on building `b`.
pub fn hack_live(world: &World, b: EntityId, effect: crate::virt::HackEffect) -> bool {
    world.comp::<Building>(b).and_then(|bd| bd.hacked).is_some_and(|(e, until)| e == effect && until > world.tick)
}

/// V32/V33: the building's robot sensors are off (Blind, or DoorOpen: a
/// robot posted at an open door neither defends nor senses).
pub fn robot_sensors_off(world: &World, b: EntityId) -> bool {
    enabled(world)
        && (hack_live(world, b, crate::virt::HackEffect::Blind)
            || hack_live(world, b, crate::virt::HackEffect::DoorOpen))
}

/// V33: the building's cameras are off (Blind only; DoorOpen opens the
/// door and leaves the cameras on).
pub fn cameras_off(world: &World, b: EntityId) -> bool {
    enabled(world) && hack_live(world, b, crate::virt::HackEffect::Blind)
}

/// V28: a sighting into `owner`'s database (`None` = the city), capped at
/// `db_cap`; a corp's or the city's sighting of an agent with an open
/// report refreshes `last_seen` (the law reads it).
pub fn record_sighting(world: &mut World, owner: Option<EntityId>, s: crate::virt::Sighting) {
    let cap = world.config.db.db_cap.max(1);
    let db = world.db.entry(owner.unwrap_or(EntityId::NONE)).or_default();
    db.sightings.push_back(s);
    while db.sightings.len() > cap {
        db.sightings.pop_front();
    }
    world.stats.current.virt.sightings += 1;
    let lawful = owner.is_none_or(|o| world.has::<Corp>(o));
    if lawful && crate::systems::law::wanted(world, s.who) {
        let newer = world.last_seen.get(&s.who).is_none_or(|&(_, t)| t <= s.tick);
        if newer {
            world.last_seen.insert(s.who, (s.tile, s.tick));
        }
    }
}

/// V28, from `law::raise_crime`: a Theft, Shakedown or Grand Theft inside a
/// building with a camera (not Blind) rolls the thief's tier against the
/// camera's effective tier on the world stream (`security::contest_p`, one
/// draw), the camera's side scaled by the owner's `alertness_mult` (the M16
/// hook every detection roll reads; at 1.0 the draw is `security::contest`'s).
/// The camera wins: a sighting (confidence the camera's odds) and, for a
/// corp or the city, a report with no witness.
pub fn camera_sense(world: &mut World, actor: EntityId, crime: crate::components::Crime, b: EntityId) {
    use crate::components::Crime;
    if !enabled(world) || !matches!(crime, Crime::Theft | Crime::Extortion | Crime::GrandTheft) {
        return;
    }
    let Some(cam) = camera_at(world, b) else { return };
    if cameras_off(world, b) {
        return;
    }
    let cam_tier = world.comp::<Asset>(cam).map_or(1, |x| crate::systems::assets::eff_tier_of(world, x));
    let thief = crate::systems::security::thief_tier(crate::systems::law::stealth(world, actor));
    let step = contest_step(world);
    let owner = world.owner_of(b);
    let p_thief = crate::systems::security::contest_p(f32::from(thief), f32::from(cam_tier), step);
    let alert = alertness_mult(world, owner);
    // The thief slips by on `roll < p_thief` (M13's draw); alertness widens
    // or narrows the camera's side from the top. At exactly 1.0 the compare
    // is against `p_thief` itself, so `1 − (1 − p)` cannot differ from M13's
    // draw by an ulp.
    let roll: f32 = world.rng.world().random();
    #[allow(clippy::float_cmp)]
    let slips = if alert == 1.0 { roll < p_thief } else { roll < 1.0 - ((1.0 - p_thief) * alert).clamp(0.0, 1.0) };
    if slips {
        return;
    }
    let confidence = 1.0 - p_thief;
    let tile = world.comp::<Position>(actor).map(|p| p.tile).unwrap_or_default();
    if owner.is_none_or(|o| world.has::<Corp>(o)) {
        crate::systems::law::file_report(world, crime, actor, None);
    }
    let tick = world.tick;
    record_sighting(world, owner, crate::virt::Sighting { who: actor, tile, tick, confidence });
}

/// V38, daily in `tech::run`: Statistical agents carrying a deck, on
/// `(index + day) % 7 == 0`, ascending, score the Hack goal with the
/// freelance considerations; a score at `stat_hack_min` writes a
/// `RunOrder { why: Stat }` and promotes them to Coarse.
pub fn stat_pass(world: &mut World) {
    if !enabled(world) {
        return;
    }
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
        let now = world.tick;
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
