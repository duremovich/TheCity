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

use smallvec::SmallVec;

use crate::components::{AssetLoc, Building, BuildingKind, Corp, DistrictId, Gang, Niche, TilePos};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::virt::{DataStore, Link, LinkKind, Node, NodeId, NodeKind, OwnerTag, SecurityProfile, Track};
use crate::world::World;

/// Is the plane on (`[virt] enabled`)? Every M14 branch starts here.
pub fn enabled(world: &World) -> bool {
    world.config.virt.enabled
}

/// Per tick: relink when a hook marked the plane dirty. Phase 2 pops the
/// due run steps here.
pub fn run(world: &mut World) {
    if !enabled(world) {
        return;
    }
    if world.virt_dirty {
        relink(world);
    }
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
