//! M13 assets (docs/M13_ASSETS.md § 1, plan phase 1, D1-D15, D45): owned
//! things with upkeep, finance, repossession, impound, wear, repairs, loot
//! and inheritance; the derived per-agent `Kit`; goods stock helpers; the
//! Recycler's Parts and the parts market.
//!
//! An asset is an entity carrying only an `Asset`. Its `loc` and `owner`
//! are written only by [`set_loc`] and [`set_owner`], which keep the
//! indices on `World` (`assets_at`, `assets_by_owner`, `vehicles`, `limbo`)
//! and re-kit every agent whose `Kit` can change (plan D2). Nothing here
//! runs per tick: [`run`] is the daily pass at midnight, right after
//! ownership's (plan D9); the rest is event-driven.

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{
    Appearance, Asset, AssetKind, AssetLoc, Body, Brain, Building, BuildingKind, Class, Controller, Corp, Corpse,
    Finance, Gang, Good, Identity, Inventory, Job, Kit, MemoryKind, Position, ShopPick, Slot, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{Tick, TICKS_PER_HOUR};
use crate::world::World;

/// Plan D4: the Body draws' stream key (`SimRng::keyed`), so neither the
/// world stream nor any agent's is touched.
pub const BODY_KEY: u64 = 0xB0D7 << 44;

/// The food carry cap (v1): `economy::buy_quantity` keeps it (plan D7).
pub const FOOD_CAP: u32 = 20;

/// Days of `Building.asset_sales` kept.
const SALES_DAYS: usize = 7;

type List = SmallVec<[EntityId; 4]>;

fn list_insert(list: &mut List, id: EntityId) {
    if let Err(i) = list.binary_search(&id) {
        list.insert(i, id);
    }
}

fn list_remove(list: &mut List, id: EntityId) {
    if let Ok(i) = list.binary_search(&id) {
        list.remove(i);
    }
}

// ---------------------------------------------------------------------------
// Bodies (plan D4)
// ---------------------------------------------------------------------------

/// The keyed stream for `id`'s Body at `tick`.
fn body_rng(world: &World, id: EntityId, tick: Tick) -> rand_chacha::ChaCha8Rng {
    world.rng.keyed(BODY_KEY ^ (u64::from(id.index) << 20) ^ tick)
}

/// A fresh Body: strength and reflex `U(0.2, 0.6)`, sanity 1.
pub fn new_body(world: &World, id: EntityId) -> Body {
    let mut r = body_rng(world, id, world.tick);
    let strength = r.random_range(0.2f32..0.6);
    let reflex = r.random_range(0.2f32..0.6);
    Body { strength, reflex, sanity: 1.0, addiction: 0.0, last_use: None, episode_until: None, last_shop: None }
}

/// A newborn's Body: the parents' mean ± `U(-0.05, 0.05)`, clamped to 0.2..=0.6.
pub fn child_body(world: &World, id: EntityId, mother: EntityId, father: EntityId) -> Body {
    let parents: Vec<&Body> = [mother, father].iter().filter_map(|&p| world.comp::<Body>(p)).collect();
    if parents.is_empty() {
        return new_body(world, id);
    }
    let n = parents.len() as f32;
    let (s, x) =
        (parents.iter().map(|b| b.strength).sum::<f32>() / n, parents.iter().map(|b| b.reflex).sum::<f32>() / n);
    let mut r = body_rng(world, id, world.tick);
    let strength = (s + r.random_range(-0.05f32..0.05)).clamp(0.2, 0.6);
    let reflex = (x + r.random_range(-0.05f32..0.05)).clamp(0.2, 0.6);
    Body { strength, reflex, sanity: 1.0, addiction: 0.0, last_use: None, episode_until: None, last_shop: None }
}

/// Give an agent its Body and an empty Kit.
pub fn give_body(world: &mut World, id: EntityId, body: Body) {
    world.insert(id, body);
    world.insert(id, Kit::default());
}

// ---------------------------------------------------------------------------
// Prices and config reads
// ---------------------------------------------------------------------------

/// `[assets] price[kind][tier - 1]`; `None` = not sold at that tier.
pub fn list_price(world: &World, kind: AssetKind, tier: u8) -> Option<i64> {
    let i = usize::from(tier.checked_sub(1)?);
    world.config.assets.price.get(kind).get(i).copied()
}

/// `[assets] upkeep[kind][tier - 1]` (0 when the tier has no entry).
pub fn upkeep_for(world: &World, kind: AssetKind, tier: u8) -> i64 {
    let i = usize::from(tier.saturating_sub(1));
    world.config.assets.upkeep.get(kind).get(i).copied().unwrap_or(0)
}

/// An asset's daily upkeep with the class's `asset_tax` lever applied.
pub fn upkeep_of(world: &World, a: EntityId) -> i64 {
    let Some(asset) = world.comp::<Asset>(a) else { return 0 };
    let rate = world.levers.asset_tax.get(asset.kind.class().index()).copied().unwrap_or(0.0);
    if rate == 0.0 {
        return asset.upkeep_per_day;
    }
    (asset.upkeep_per_day as f32 * (1.0 + rate)).round() as i64
}

/// The price level a seller building's owner sets (plan D29): a corp's
/// `price_level` for the building's niche, 1.0 for an agent or the city.
/// `Niche::Tech` (Clinics, Garages) arrives in phase 2; until then 1.0.
pub fn seller_level(world: &World, seller: EntityId) -> f32 {
    let Some(b) = world.comp::<Building>(seller) else { return 1.0 };
    let niche = match b.kind {
        BuildingKind::Market => crate::components::Niche::Food,
        BuildingKind::SecurityOffice => crate::components::Niche::Security,
        _ => return 1.0,
    };
    world.corp_of_building(seller).and_then(|c| world.comp::<Corp>(c)).map_or(1.0, |c| c.level(niche))
}

// ---------------------------------------------------------------------------
// Capacity (plan D7)
// ---------------------------------------------------------------------------

/// Carry load: food 1, stims 1, parts 3.
pub fn load(inv: &Inventory) -> u32 {
    inv.food + u32::from(inv.stims) + 3 * u32::from(inv.parts)
}

/// `carry_base + round(carry_per_strength × (Body.strength + Kit.strength))
/// + pack[tier]`; 20 with assets off or no Body.
pub fn capacity(world: &World, agent: EntityId) -> u32 {
    let cfg = &world.config.assets;
    let Some(body) = world.comp::<Body>(agent).filter(|_| cfg.enabled) else { return FOOD_CAP };
    let kit_strength = world.comp::<Kit>(agent).map_or(0.0, |k| k.strength);
    let carry = (cfg.carry_per_strength * (body.strength + kit_strength)).round().max(0.0) as u32;
    let pack = world
        .assets_at
        .get(&agent)
        .into_iter()
        .flatten()
        .filter_map(|&a| world.comp::<Asset>(a))
        .filter(|x| x.kind == AssetKind::Pack && x.loc == AssetLoc::Carried(agent) && x.condition > 0)
        .filter_map(|x| cfg.pack.get(usize::from(x.tier.saturating_sub(1))).copied())
        .max()
        .unwrap_or(0);
    cfg.carry_base + carry + pack
}

// ---------------------------------------------------------------------------
// Indices and the two writers (plan D2)
// ---------------------------------------------------------------------------

fn owner_key(owner: Option<EntityId>) -> EntityId {
    owner.unwrap_or(EntityId::NONE)
}

/// File an asset in every index from its current `loc` and `owner`.
fn file(world: &mut World, a: EntityId) {
    let Some((loc, owner, vehicle)) = world.comp::<Asset>(a).map(|x| (x.loc, x.owner, x.kind.is_vehicle())) else {
        return;
    };
    file_loc(world, a, loc);
    list_insert(world.assets_by_owner.entry(owner_key(owner)).or_default(), a);
    if vehicle {
        if let Err(i) = world.vehicles.binary_search(&a) {
            world.vehicles.insert(i, a);
        }
    }
}

fn file_loc(world: &mut World, a: EntityId, loc: AssetLoc) {
    match loc {
        AssetLoc::Limbo(h) => list_insert(world.limbo.entry(h).or_default(), a),
        other => {
            if let Some(holder) = other.holder() {
                list_insert(world.assets_at.entry(holder).or_default(), a);
            }
        }
    }
}

fn unfile_loc(world: &mut World, a: EntityId, loc: AssetLoc) {
    match loc {
        AssetLoc::Limbo(h) => {
            if let Some(l) = world.limbo.get_mut(&h) {
                list_remove(l, a);
                if l.is_empty() {
                    world.limbo.remove(&h);
                }
            }
        }
        other => {
            if let Some(holder) = other.holder() {
                if let Some(l) = world.assets_at.get_mut(&holder) {
                    list_remove(l, a);
                    if l.is_empty() {
                        world.assets_at.remove(&holder);
                    }
                }
            }
        }
    }
}

fn unfile_owner(world: &mut World, a: EntityId, owner: Option<EntityId>) {
    let key = owner_key(owner);
    if let Some(l) = world.assets_by_owner.get_mut(&key) {
        list_remove(l, a);
        if l.is_empty() {
            world.assets_by_owner.remove(&key);
        }
    }
}

/// Re-kit the agents an asset touches: its holder, owner and keeper.
fn rekit_touched(world: &mut World, ids: &[Option<EntityId>]) {
    let mut seen: SmallVec<[EntityId; 6]> = SmallVec::new();
    for id in ids.iter().flatten() {
        if !seen.contains(id) {
            seen.push(*id);
            rekit(world, *id);
        }
    }
}

fn touched(world: &World, a: EntityId) -> [Option<EntityId>; 3] {
    world.comp::<Asset>(a).map_or([None; 3], |x| [x.loc.holder(), x.owner, x.keeper])
}

/// The one writer of `Asset.loc`.
pub fn set_loc(world: &mut World, a: EntityId, loc: AssetLoc) {
    let Some(old) = world.comp::<Asset>(a).map(|x| x.loc) else { return };
    let before = touched(world, a);
    unfile_loc(world, a, old);
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.loc = loc;
    }
    file_loc(world, a, loc);
    let after = touched(world, a);
    rekit_touched(world, &[before[0], before[1], before[2], after[0]]);
}

/// The one writer of `Asset.owner` (`None` = the city).
pub fn set_owner(world: &mut World, a: EntityId, owner: Option<EntityId>) {
    let Some(old) = world.comp::<Asset>(a).map(|x| x.owner) else { return };
    let before = touched(world, a);
    unfile_owner(world, a, old);
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.owner = owner;
    }
    list_insert(world.assets_by_owner.entry(owner_key(owner)).or_default(), a);
    rekit_touched(world, &[before[0], before[1], before[2], owner]);
}

/// Set (or clear) a fleet vehicle's keeper (plan D24).
pub fn set_keeper(world: &mut World, a: EntityId, keeper: Option<EntityId>) {
    let Some(old) = world.comp::<Asset>(a).map(|x| x.keeper) else { return };
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.keeper = keeper;
    }
    rekit_touched(world, &[old, keeper]);
}

/// A new asset: condition 100, value = `list`, upkeep from the config.
pub fn spawn_asset(
    world: &mut World,
    kind: AssetKind,
    tier: u8,
    owner: Option<EntityId>,
    loc: AssetLoc,
    list: i64,
) -> EntityId {
    let upkeep_per_day = upkeep_for(world, kind, tier);
    let id = world.spawn();
    let bought = world.tick;
    world.insert(
        id,
        Asset {
            kind,
            tier,
            owner,
            loc,
            condition: 100,
            value: list,
            upkeep_per_day,
            upkeep_arrears: 0,
            finance: None,
            bricked: false,
            stolen: false,
            bought,
            keeper: None,
            list,
        },
    );
    file(world, id);
    let t = touched(world, id);
    rekit_touched(world, &t);
    id
}

/// Unfile, re-kit and free an asset.
pub fn despawn(world: &mut World, a: EntityId) {
    let Some((loc, owner, vehicle)) = world.comp::<Asset>(a).map(|x| (x.loc, x.owner, x.kind.is_vehicle())) else {
        return;
    };
    let t = touched(world, a);
    unfile_loc(world, a, loc);
    unfile_owner(world, a, owner);
    if vehicle {
        if let Ok(i) = world.vehicles.binary_search(&a) {
            world.vehicles.remove(i);
        }
    }
    world.despawn(a);
    rekit_touched(world, &t);
}

/// The assets at a building or agent (its `loc` names it), ascending.
pub fn assets_at(world: &World, holder: EntityId) -> &[EntityId] {
    world.assets_at.get(&holder).map_or(&[], |l| l.as_slice())
}

/// The assets an owner holds (`None` = the city), ascending.
pub fn assets_of(world: &World, owner: Option<EntityId>) -> &[EntityId] {
    world.assets_by_owner.get(&owner_key(owner)).map_or(&[], |l| l.as_slice())
}

/// Every asset, ascending (each is filed under exactly one owner).
pub fn all_assets(world: &World) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = world.assets_by_owner.values().flatten().copied().collect();
    v.sort_unstable();
    v
}

/// The D2 indices rebuilt from the stores, for `rebuild` and `check`.
#[allow(clippy::type_complexity)]
fn indices_from_stores(
    world: &World,
) -> (
    std::collections::BTreeMap<EntityId, List>,
    std::collections::BTreeMap<EntityId, List>,
    Vec<EntityId>,
    std::collections::BTreeMap<crate::components::HoleId, List>,
    Vec<EntityId>,
) {
    let mut at: std::collections::BTreeMap<EntityId, List> = Default::default();
    let mut by_owner: std::collections::BTreeMap<EntityId, List> = Default::default();
    let mut vehicles = Vec::new();
    let mut limbo: std::collections::BTreeMap<crate::components::HoleId, List> = Default::default();
    let mut loot = Vec::new();
    for id in world.entities() {
        if let Some(x) = world.comp::<Asset>(id) {
            match x.loc {
                AssetLoc::Limbo(h) => limbo.entry(h).or_default().push(id),
                other => {
                    if let Some(h) = other.holder() {
                        at.entry(h).or_default().push(id);
                    }
                }
            }
            by_owner.entry(owner_key(x.owner)).or_default().push(id);
            if x.kind.is_vehicle() {
                vehicles.push(id);
            }
        }
        if world.comp::<Corpse>(id).is_some_and(|c| !c.settled) {
            loot.push(id);
        }
    }
    (at, by_owner, vehicles, limbo, loot)
}

/// After a load (plan D2, D3, D50): the indices from the stores, a Kit on
/// every agent with a Body, each rebuilt. Tolerates stores shorter than the
/// arena (a pre-M13 save before `migrate_legacy` resizes them).
pub fn rebuild(world: &mut World) {
    let n = world.generations.len();
    if world.kit.len() < n {
        world.kit.resize_with(n, || None);
    }
    let (at, by_owner, vehicles, limbo, loot) = indices_from_stores(world);
    world.assets_at = at;
    world.assets_by_owner = by_owner;
    world.vehicles = vehicles;
    world.limbo = limbo;
    world.loot_corpses = loot;
    let bodies: Vec<EntityId> = world.entities().filter(|&id| world.has::<Body>(id)).collect();
    for id in bodies {
        world.kit[id.index as usize] = Some(Kit::default());
        rekit(world, id);
    }
}

/// Compare the incremental indices with a rebuild from the stores.
pub fn check(world: &World) -> Result<(), String> {
    let (at, by_owner, vehicles, limbo, loot) = indices_from_stores(world);
    if at != world.assets_at || by_owner != world.assets_by_owner {
        return Err("assets_at / assets_by_owner out of sync with the Asset store".to_string());
    }
    if vehicles != world.vehicles || limbo != world.limbo {
        return Err("vehicles / limbo out of sync with the Asset store".to_string());
    }
    if loot != world.loot_corpses {
        return Err(format!("loot_corpses out of sync: {:?}", world.loot_corpses));
    }
    for id in world.entities() {
        if let Some(k) = world.comp::<Kit>(id) {
            if *k != compute_kit(world, id) {
                return Err(format!("Kit of {id} is stale"));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The Kit (plan D3, Kit tables)
// ---------------------------------------------------------------------------

/// The agent's own vehicle (lowest id, not wrecked, not in stock or limbo),
/// else the vehicle it keeps (plan D24).
pub fn vehicle_of(world: &World, agent: EntityId) -> Option<EntityId> {
    let usable =
        |x: &Asset| x.kind.is_vehicle() && x.condition > 0 && !matches!(x.loc, AssetLoc::Stock(_) | AssetLoc::Limbo(_));
    assets_of(world, Some(agent)).iter().copied().find(|&a| world.comp::<Asset>(a).is_some_and(usable)).or_else(|| {
        world
            .vehicles
            .iter()
            .copied()
            .find(|&a| world.comp::<Asset>(a).is_some_and(|x| x.keeper == Some(agent) && usable(x)))
    })
}

/// The Kit `agent` would have now, from its assets.
pub fn compute_kit(world: &World, agent: EntityId) -> Kit {
    let cfg = &world.config.chrome;
    let mut k = Kit::default();
    for &a in assets_at(world, agent) {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        let AssetKind::Implant(slot) = x.kind else { continue };
        if x.loc != AssetLoc::Installed(agent) {
            continue;
        }
        let t = x.tier.max(1);
        let tf = f32::from(t);
        k.chrome = true;
        k.load += cfg.sanity_cost.get(usize::from(t - 1)).copied().unwrap_or(0.0);
        k.chrome_value += x.value;
        if matches!(slot, Slot::Arms | Slot::Eyes | Slot::Skin) {
            k.visible = k.visible.saturating_add(t);
        }
        if x.bricked || x.condition == 0 {
            continue;
        }
        match slot {
            Slot::Arms => {
                k.fighting += cfg.arms_fighting * tf;
                k.strength += cfg.arms_strength * tf;
            }
            Slot::Legs => {
                k.walk_mult = k.walk_mult.min(cfg.legs_walk.get(usize::from(t - 1)).copied().unwrap_or(1.0));
            }
            Slot::Nerves => k.reflex += cfg.nerves_reflex * tf,
            Slot::Eyes => {
                k.sight = k.sight.saturating_add(cfg.eyes_sight.saturating_mul(t));
                k.stealth += cfg.eyes_stealth * tf;
            }
            Slot::Skin => k.armour = k.armour.max(cfg.skin_armour * tf),
        }
    }
    k.vehicle = vehicle_of(world, agent);
    k.driving = world.trips.get(&agent).and_then(|t| world.comp::<Asset>(t.vehicle)).map(|x| x.kind);
    let vehicle_tier =
        k.vehicle.and_then(|v| world.comp::<Asset>(v)).map_or(
            0,
            |x| {
                if x.kind == AssetKind::Flyer {
                    3
                } else {
                    x.tier
                }
            },
        );
    k.flash = ((f32::from(k.visible) + f32::from(vehicle_tier)) / 9.0).min(1.0);
    k
}

/// Recompute an agent's Kit (a no-op for anything without one).
pub fn rekit(world: &mut World, agent: EntityId) {
    if !world.has::<Kit>(agent) {
        return;
    }
    let k = compute_kit(world, agent);
    if let Some(slot) = world.comp_mut::<Kit>(agent) {
        *slot = k;
    }
}

/// The agent's Kit, or the bare default.
pub fn kit(world: &World, agent: EntityId) -> Kit {
    world.comp::<Kit>(agent).cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Places
// ---------------------------------------------------------------------------

/// Where an asset stands: its building's door, or its holder agent's tile.
pub fn asset_tile(world: &World, a: EntityId) -> Option<TilePos> {
    let holder = world.comp::<Asset>(a)?.loc.holder()?;
    world.comp::<Building>(holder).map(|b| b.door).or_else(|| world.comp::<Position>(holder).map(|p| p.tile))
}

/// The building of `kind` nearest `from` (door Manhattan, ties lower id),
/// standing and not derelict, optionally owned by `owner` only.
fn nearest_building(
    world: &World,
    kind: BuildingKind,
    from: TilePos,
    owner: Option<Option<EntityId>>,
) -> Option<EntityId> {
    world
        .buildings_of_kind(kind)
        .iter()
        .filter_map(|&b| {
            let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict)?;
            if owner.is_some_and(|o| bd.owner != o) {
                return None;
            }
            Some((bd.door.manhattan(from), b))
        })
        .min()
        .map(|(_, b)| b)
}

/// The building whose door is nearest `from` (any kind, standing).
fn nearest_any_building(world: &World, from: TilePos) -> Option<EntityId> {
    world
        .buildings_by_kind
        .values()
        .flatten()
        .filter_map(|&b| world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// An owner whose purse is live: the city, a corp, a gang or a living agent.
fn live_owner(world: &World, owner: Option<EntityId>) -> bool {
    match owner {
        None => true,
        Some(o) => world.has::<Corp>(o) || world.has::<Gang>(o) || world.has::<Wallet>(o),
    }
}

fn is_agent(world: &World, owner: Option<EntityId>) -> Option<EntityId> {
    owner.filter(|&o| world.has::<Identity>(o) && world.has::<Wallet>(o))
}

/// What a purse can pay now: a corp or the city anything, an agent or gang its coins.
fn can_pay(world: &World, owner: Option<EntityId>, amount: i64) -> bool {
    match ownership::owner_kind(world, owner) {
        OwnerKind::City | OwnerKind::Corp(_) => true,
        OwnerKind::Gang(_) | OwnerKind::Agent(_) => world.purse(owner) >= amount,
    }
}

// ---------------------------------------------------------------------------
// Goods stock (plan D6)
// ---------------------------------------------------------------------------

impl World {
    /// A building's cap for `good`: Food as today (the kind's `stock_cap`);
    /// Stims and Parts at least `[assets] goods_cap`.
    pub fn goods_cap(&self, b: EntityId, good: Good) -> u32 {
        let Some(bd) = self.comp::<Building>(b) else { return 0 };
        let cap = self.config.buildings.for_kind(bd.kind).stock_cap;
        match good {
            Good::Food => cap,
            Good::Stims | Good::Parts => cap.max(self.config.assets.goods_cap),
        }
    }

    /// Units of `good` a building holds.
    pub fn stock(&self, b: EntityId, good: Good) -> u32 {
        self.comp::<Building>(b).map_or(0, |bd| match good {
            Good::Food => bd.stock_food,
            Good::Stims | Good::Parts => bd.stock_goods[good as usize - 1],
        })
    }

    /// Add up to `n` units, capped at `goods_cap`; returns the units added.
    pub fn add_stock(&mut self, b: EntityId, good: Good, n: u32) -> u32 {
        let cap = self.goods_cap(b, good);
        let Some(bd) = self.comp_mut::<Building>(b) else { return 0 };
        let slot = match good {
            Good::Food => &mut bd.stock_food,
            Good::Stims | Good::Parts => &mut bd.stock_goods[good as usize - 1],
        };
        let added = n.min(cap.saturating_sub(*slot));
        *slot += added;
        added
    }

    /// Take up to `n` units; returns the units taken.
    pub fn take_stock(&mut self, b: EntityId, good: Good, n: u32) -> u32 {
        let Some(bd) = self.comp_mut::<Building>(b) else { return 0 };
        let slot = match good {
            Good::Food => &mut bd.stock_food,
            Good::Stims | Good::Parts => &mut bd.stock_goods[good as usize - 1],
        };
        let taken = n.min(*slot);
        *slot -= taken;
        taken
    }
}

// ---------------------------------------------------------------------------
// The daily pass (plan D9)
// ---------------------------------------------------------------------------

/// Midnight, right after ownership's pass, while `[assets] enabled`:
/// upkeep, finance, repossession and impound, wear and wrecks, repairs,
/// the corpse window and the scavs, the parts market, appearance, and the
/// 7-day sales roll. Nothing per tick.
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 || !world.config.assets.enabled {
        return;
    }
    let unpaid = upkeep(world);
    finance(world);
    repossess(world);
    wear(world, &unpaid);
    repairs(world);
    settle_window(world);
    scav_strip(world);
    parts_market(world);
    appearance(world);
    roll_sales(world);
}

/// Each asset with a live, non-city owner pays its upkeep (`Flow::AssetUpkeep`
/// to the Treasury); short: `upkeep_arrears += 1`. Returns the unpaid ids.
fn upkeep(world: &mut World) -> Vec<EntityId> {
    let mut unpaid = Vec::new();
    for a in all_assets(world) {
        let Some(owner) = world.comp::<Asset>(a).map(|x| x.owner) else { continue };
        if owner.is_none() || !live_owner(world, owner) {
            continue;
        }
        let cost = upkeep_of(world, a);
        let paid = if cost > 0 { ownership::charge(world, owner, None, cost, Flow::AssetUpkeep) } else { 0 };
        let Some(x) = world.comp_mut::<Asset>(a) else { continue };
        if paid >= cost {
            x.upkeep_arrears = 0;
        } else {
            x.upkeep_arrears = x.upkeep_arrears.saturating_add(1);
            unpaid.push(a);
        }
    }
    unpaid
}

/// Each finance plan takes the day's payment and any missed days, capped
/// at what remains (`Flow::Finance`); short: `arrears += 1`, paid in full:
/// arrears 0. A plan at 0 remaining ends.
fn finance(world: &mut World) {
    for a in all_assets(world) {
        let Some((owner, f)) = world.comp::<Asset>(a).and_then(|x| x.finance.clone().map(|f| (x.owner, f))) else {
            continue;
        };
        if !live_owner(world, owner) {
            continue;
        }
        let due = (f.per_day * (1 + i64::from(f.arrears))).min(f.remaining).max(0);
        let paid = if owner == f.lender { due } else { ownership::charge(world, owner, f.lender, due, Flow::Finance) };
        let Some(x) = world.comp_mut::<Asset>(a) else { continue };
        let Some(fin) = x.finance.as_mut() else { continue };
        fin.remaining -= paid;
        if paid >= due {
            fin.arrears = 0;
        } else {
            fin.arrears = fin.arrears.saturating_add(1);
        }
        if fin.remaining <= 0 {
            x.finance = None;
        }
    }
}

/// Plan D11, D12.
fn repossess(world: &mut World) {
    let cfg = world.config.assets.clone();
    for a in all_assets(world) {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        if let Some(f) = x.finance.clone() {
            // Arrears cleared: the lender unbricks.
            if x.kind.is_implant() && x.bricked && f.arrears == 0 {
                if let Some(m) = world.comp_mut::<Asset>(a) {
                    m.bricked = false;
                }
                rekit_touched(world, &[x.loc.holder()]);
                continue;
            }
            let Some(lender) = f.lender.filter(|&l| live_owner(world, Some(l))) else { continue };
            if f.arrears < cfg.repo_days {
                continue;
            }
            match x.kind {
                k if k.is_vehicle() => {
                    if matches!(x.loc, AssetLoc::InUse(_)) {
                        // Towed when it parks (phase 2's `end_trip`); retried daily.
                        continue;
                    }
                    let from = asset_tile(world, a).unwrap_or_default();
                    let to = nearest_building(world, BuildingKind::Garage, from, Some(Some(lender)))
                        .or_else(|| world.building_of_kind(BuildingKind::Jail));
                    take_back(world, a, lender);
                    if let Some(b) = to {
                        set_loc(world, a, AssetLoc::Stock(b));
                    }
                    repossessed(world, &x, Some(lender), a, "towed");
                    world.stats.current.repos += 1;
                }
                AssetKind::Implant(_) => {
                    if !x.bricked {
                        if let Some(m) = world.comp_mut::<Asset>(a) {
                            m.bricked = true;
                        }
                        rekit_touched(world, &[x.loc.holder()]);
                        repossessed(world, &x, Some(lender), a, "bricked");
                        world.stats.current.repos += 1;
                    } else if u32::from(f.arrears) >= u32::from(cfg.repo_days) + u32::from(cfg.brick_repo_days) {
                        // Dead metal: the lender owns it in place.
                        take_back(world, a, lender);
                        let (owner, what) = (world.owner_label(x.owner), world.name_of(a));
                        let text = format!("{} took {owner}'s {what}", world.owner_label(Some(lender)));
                        let actors = [x.owner.unwrap_or(EntityId::NONE), lender, a];
                        world.push_event(EventKind::Repossessed, &actors, text);
                    }
                }
                _ => {
                    let from = asset_tile(world, a).unwrap_or_default();
                    let seller_kind = match x.kind {
                        AssetKind::Robot => BuildingKind::SecurityOffice,
                        AssetKind::Pack => BuildingKind::Market,
                        _ => BuildingKind::Garage,
                    };
                    let to = nearest_building(world, seller_kind, from, Some(Some(lender)));
                    take_back(world, a, lender);
                    if let Some(b) = to {
                        set_loc(world, a, AssetLoc::Stock(b));
                    }
                    repossessed(world, &x, Some(lender), a, "took");
                    world.stats.current.repos += 1;
                }
            }
        }
    }
    impound(world);
}

/// The lender becomes the owner; the plan and the arrears end.
fn take_back(world: &mut World, a: EntityId, lender: EntityId) {
    set_owner(world, a, Some(lender));
    if let Some(m) = world.comp_mut::<Asset>(a) {
        m.finance = None;
        m.upkeep_arrears = 0;
    }
}

/// The `Repossessed` event, memory and counter for a tow, brick or take.
fn repossessed(world: &mut World, before: &Asset, lender: Option<EntityId>, a: EntityId, how: &str) {
    let owner = before.owner;
    let what = world.name_of(a);
    let (o, l) = (world.owner_label(owner), world.owner_label(lender));
    let text = match how {
        "bricked" => format!("{l} bricked {o}'s {what}"),
        "towed" => format!("{l} towed {o}'s {what}"),
        "impounded" => format!("the city impounded {o}'s {what}"),
        _ => format!("{l} took {o}'s {what}"),
    };
    let actors = [owner.unwrap_or(EntityId::NONE), lender.unwrap_or(EntityId::NONE), a];
    world.push_event(EventKind::Repossessed, &actors, text);
    if let Some(agent) = is_agent(world, owner) {
        let subject = lender.filter(|&x| world.has::<Identity>(x));
        world.remember(agent, MemoryKind::Repossessed, subject, 0.6, -0.6, false);
    }
}

/// Plan D12: vehicles a non-city owner left unregistered `impound_days`
/// go to the Precinct; every vehicle in Precinct stock is offered to the
/// nearest Garage whose owner can pay `impound_frac × value`.
fn impound(world: &mut World) {
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return };
    let days = world.config.assets.impound_days;
    if world.levers.impound {
        for a in world.vehicles.clone() {
            let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
            if x.owner.is_none()
                || x.upkeep_arrears < days
                || matches!(x.loc, AssetLoc::InUse(_) | AssetLoc::Stock(_) | AssetLoc::Limbo(_))
            {
                continue;
            }
            set_owner(world, a, None);
            set_loc(world, a, AssetLoc::Stock(precinct));
            if let Some(m) = world.comp_mut::<Asset>(a) {
                m.finance = None;
                m.upkeep_arrears = 0;
            }
            repossessed(world, &x, None, a, "impounded");
            world.stats.current.impounds += 1;
        }
    }
    // The impound sale: the proceeds to the vehicle's owner (the city, or a
    // lender whose tow found no Garage of its own).
    let door = world.comp::<Building>(precinct).map_or_else(TilePos::default, |b| b.door);
    let frac = world.config.assets.impound_frac;
    for a in assets_at(world, precinct).to_vec() {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        if !x.kind.is_vehicle() || x.loc != AssetLoc::Stock(precinct) {
            continue;
        }
        let price = (frac * x.value as f32).round() as i64;
        let mut garages: Vec<(u32, EntityId)> = world
            .buildings_of_kind(BuildingKind::Garage)
            .iter()
            .filter_map(|&g| {
                world.comp::<Building>(g).filter(|b| !b.demolished && !b.derelict).map(|b| (b.door.manhattan(door), g))
            })
            .collect();
        garages.sort_unstable();
        let Some(g) = garages.into_iter().map(|(_, g)| g).find(|&g| {
            let o = world.owner_of(g);
            o != x.owner && world.purse(o) >= price
        }) else {
            continue;
        };
        let buyer = world.owner_of(g);
        ownership::pay(world, buyer, x.owner, price, Flow::Asset);
        set_owner(world, a, buyer);
        set_loc(world, a, AssetLoc::Stock(g));
        let (what, who) = (world.name_of(a), world.owner_label(buyer));
        world.push_event(
            EventKind::AssetBought,
            &[buyer.unwrap_or(EntityId::NONE), g, a],
            format!("{who} bought the impounded {what} for {price}"),
        );
    }
}

/// Plan D9 step 4: `condition −= wear` (× `unpaid_wear_mult` unpaid), value
/// from condition; at 0 a vehicle or robot is wrecked, an implant fails.
/// Stock and limbo do not wear.
fn wear(world: &mut World, unpaid: &[EntityId]) {
    let mult = world.config.assets.unpaid_wear_mult.max(1);
    let mut gone = Vec::new();
    for a in all_assets(world) {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        let w = *world.config.assets.wear.get(x.kind);
        let idle = matches!(x.loc, AssetLoc::Stock(_) | AssetLoc::Limbo(_));
        let mut was = x.condition;
        if w > 0 && !idle && x.condition > 0 {
            let w = if unpaid.binary_search(&a).is_ok() { w.saturating_mul(mult) } else { w };
            let c = x.condition.saturating_sub(w);
            if let Some(m) = world.comp_mut::<Asset>(a) {
                m.condition = c;
            }
            if c == 0 {
                gone.push(a);
            }
            was = c;
        }
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.value = m.list * i64::from(was) / 100;
        }
    }
    for a in gone {
        wreck(world, a, "worn out");
    }
}

/// A vehicle, robot, pack or bridge is wrecked: `Wrecked` event, half its
/// `parts_per` into the stock of the building it stood at (a driver's or
/// carrier's nearest), despawned. An implant fails instead: condition 0,
/// sanity − `fail_shock`, never despawned.
pub fn wreck(world: &mut World, a: EntityId, why: &str) {
    let Some(x) = world.comp::<Asset>(a).cloned() else { return };
    if x.kind.is_implant() {
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.condition = 0;
            m.value = 0;
        }
        if let AssetLoc::Installed(body) = x.loc {
            let shock = world.config.assets.fail_shock;
            if let Some(b) = world.comp_mut::<Body>(body) {
                b.sanity = (b.sanity - shock).clamp(0.0, 1.0);
            }
        }
        rekit_touched(world, &[x.loc.holder()]);
        let (what, owner) = (world.name_of(a), world.owner_label(x.owner));
        world.push_event(
            EventKind::Wrecked,
            &[a, x.owner.unwrap_or(EntityId::NONE)],
            format!("{owner}'s {what} failed ({why})"),
        );
        return;
    }
    let at = match x.loc {
        AssetLoc::Parked(b) | AssetLoc::Posted(b) | AssetLoc::Stock(b) => Some(b),
        AssetLoc::InUse(_) | AssetLoc::Carried(_) | AssetLoc::Installed(_) => {
            asset_tile(world, a).and_then(|t| nearest_any_building(world, t))
        }
        AssetLoc::Limbo(_) => None,
    };
    let parts = *world.config.assets.parts_per.get(x.kind) / 2;
    if let Some(b) = at {
        world.add_stock(b, Good::Parts, parts);
    }
    let (what, owner) = (world.name_of(a), world.owner_label(x.owner));
    world.push_event(
        EventKind::Wrecked,
        &[a, x.owner.unwrap_or(EntityId::NONE)],
        format!("{owner}'s {what} was wrecked ({why})"),
    );
    despawn(world, a);
}

/// Owners of vehicles below `repair_below` buy the points back to 100 from
/// the nearest Garage at `repair_price × level` a point (`Flow::Asset`),
/// one Part per 25 points from its stock (a missing Part costs the Garage
/// `part_credit` as `Import`). An agent or gang pays only in full.
fn repairs(world: &mut World) {
    let cfg = world.config.assets.clone();
    for a in world.vehicles.clone() {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        if x.condition >= cfg.repair_below
            || x.condition == 0
            || x.owner.is_none()
            || !live_owner(world, x.owner)
            || matches!(x.loc, AssetLoc::Stock(_) | AssetLoc::Limbo(_))
        {
            continue;
        }
        let Some(from) = asset_tile(world, a) else { continue };
        let Some(g) = nearest_building(world, BuildingKind::Garage, from, None) else { continue };
        let points = 100 - i64::from(x.condition);
        let cost = (cfg.repair_price as f32 * seller_level(world, g) * points as f32).round() as i64;
        if !can_pay(world, x.owner, cost) {
            continue;
        }
        let garage_owner = world.owner_of(g);
        let paid = ownership::charge(world, x.owner, garage_owner, cost, Flow::Asset);
        ownership::credit(world, g, paid);
        let need = (points as u32).div_ceil(25);
        let have = world.take_stock(g, Good::Parts, need);
        let missing = i64::from(need - have);
        if missing > 0 {
            ownership::charge(world, garage_owner, None, missing * cfg.part_credit, Flow::Import);
        }
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.condition = 100;
            m.value = m.list;
        }
    }
}

// ---------------------------------------------------------------------------
// Corpses as loot (plan D13-D15)
// ---------------------------------------------------------------------------

/// `kill_by` has put the Corpse on: file it for settlement, and settle at
/// once when the window is 0 or assets are off (M12: inheritance at death).
pub fn on_corpse(world: &mut World, c: EntityId) {
    if let Err(i) = world.loot_corpses.binary_search(&c) {
        world.loot_corpses.insert(i, c);
    }
    if !world.config.assets.enabled || world.config.assets.loot_window_hours == 0 {
        settle_corpse(world, c);
    }
}

/// Inheritance on what a corpse still carries (plan D13): its coins by the
/// M11 rule, carried packs to the coin heir (else the Recycler's stock,
/// the city's), goods into the heir's inventory up to capacity (the rest
/// is lost). Then `settled`; dropped from `loot_corpses`.
pub fn settle_corpse(world: &mut World, c: EntityId) {
    if let Ok(i) = world.loot_corpses.binary_search(&c) {
        world.loot_corpses.remove(i);
    }
    let Some(corpse) = world.comp_mut::<Corpse>(c) else { return };
    if corpse.settled {
        return;
    }
    corpse.settled = true;
    let loot = std::mem::take(&mut corpse.loot);
    if loot.coins > 0 {
        crate::systems::demography::inherit_coins(world, c, loot.coins);
    }
    let heir = crate::systems::demography::coin_heirs(world, c).first().copied().filter(|&h| world.has::<Brain>(h));
    let packs: Vec<EntityId> = assets_at(world, c)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.loc == AssetLoc::Carried(c)))
        .collect();
    for p in packs {
        match heir {
            Some(h) => {
                set_owner(world, p, Some(h));
                set_loc(world, p, AssetLoc::Carried(h));
            }
            None => {
                set_owner(world, p, None);
                match world.building_of_kind(BuildingKind::Cemetery) {
                    Some(r) => set_loc(world, p, AssetLoc::Stock(r)),
                    None => despawn(world, p),
                }
            }
        }
    }
    if let Some(h) = heir {
        give_goods(world, h, loot.food, loot.stims, loot.parts);
    }
}

/// Goods into an agent's inventory up to its capacity (food also to
/// `FOOD_CAP`); the rest is lost.
fn give_goods(world: &mut World, agent: EntityId, food: u32, stims: u16, parts: u16) {
    let cap = capacity(world, agent);
    let Some(inv) = world.comp_mut::<Inventory>(agent) else { return };
    let mut room = cap.saturating_sub(load(inv));
    let f = food.min(room).min(FOOD_CAP.saturating_sub(inv.food));
    inv.food += f;
    room -= f;
    let s = u32::from(stims).min(room);
    inv.stims = inv.stims.saturating_add(s as u16);
    room -= s;
    let p = u32::from(parts).min(room / 3);
    inv.parts = inv.parts.saturating_add(p as u16);
}

/// Plan D14: the chrome in a buried body goes to the Recycler as Parts.
pub fn recycle_implants(world: &mut World, c: EntityId) {
    let implants: Vec<EntityId> = assets_at(world, c)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind.is_implant() && x.loc == AssetLoc::Installed(c)))
        .collect();
    if implants.is_empty() {
        return;
    }
    let per = world.config.assets.parts_per.implant;
    let recycler = world.building_of_kind(BuildingKind::Cemetery);
    for a in implants {
        if let Some(r) = recycler {
            world.add_stock(r, Good::Parts, per);
        }
        despawn(world, a);
    }
}

/// `demography::bury`: settle the body, then recycle its chrome.
pub fn on_buried(world: &mut World, c: EntityId) {
    settle_corpse(world, c);
    recycle_implants(world, c);
}

/// `World::remove_agent` (a freed corpse, an emigrant): an unsettled corpse
/// settles first; whatever is still at the entity goes with it.
pub fn on_removed(world: &mut World, id: EntityId) {
    if world.comp::<Corpse>(id).is_some_and(|c| !c.settled) {
        settle_corpse(world, id);
    }
    for a in assets_at(world, id).to_vec() {
        despawn(world, a);
    }
    world.trips.remove(&id);
    world.chase_pins.remove(&id);
}

/// Daily: corpses older than the window settle (so the window is 12–36 h:
/// daily granularity).
fn settle_window(world: &mut World) {
    let window = u64::from(world.config.assets.loot_window_hours) * TICKS_PER_HOUR;
    let now = world.tick;
    for c in world.loot_corpses.clone() {
        let old = world.comp::<Corpse>(c).is_some_and(|k| now.saturating_sub(k.died_tick) >= window);
        if old {
            settle_corpse(world, c);
        }
    }
}

/// Plan D15: every unsettled, unstripped, unburied corpse rolls
/// `scav_strip_base × (2 − coverage)` on the world stream (ascending).
fn scav_strip(world: &mut World) {
    let base = world.config.assets.scav_strip_base;
    for c in world.loot_corpses.clone() {
        let Some(k) = world.comp::<Corpse>(c) else { continue };
        if k.stripped || k.buried || k.settled {
            continue;
        }
        let Some(tile) = world.comp::<Position>(c).map(|p| p.tile) else { continue };
        let d = world.district_of(tile);
        let cover = world.district(d).coverage;
        let p = f64::from((base * (2.0 - cover)).clamp(0.0, 1.0));
        if world.rng.world().random_bool(p) {
            strip_offscreen(world, c);
        }
    }
}

/// An off-screen strip: in a gang-held district the loot goes to that
/// gang (coins to its treasury, goods and chrome to its Hideout, chrome
/// marked stolen); anywhere else goods and chrome are destroyed and the
/// coins go to the Treasury (destroying them would break M11 D3).
pub fn strip_offscreen(world: &mut World, c: EntityId) {
    let Some(tile) = world.comp::<Position>(c).map(|p| p.tile) else { return };
    let gang = match world.district(world.district_of(tile)).control {
        Controller::Gang(g) if world.has::<Gang>(g) => Some(g),
        _ => None,
    };
    let hideout = gang.and_then(|g| world.hideout_of(g));
    let Some(corpse) = world.comp_mut::<Corpse>(c) else { return };
    let loot = std::mem::take(&mut corpse.loot);
    corpse.stripped = true;
    match (gang, hideout) {
        (Some(g), Some(h)) => {
            loot_to_gang(world, g, loot.coins);
            world.add_stock(h, Good::Food, loot.food);
            world.add_stock(h, Good::Stims, u32::from(loot.stims));
            world.add_stock(h, Good::Parts, u32::from(loot.parts));
            for a in assets_at(world, c).to_vec() {
                set_owner(world, a, Some(g));
                set_loc(world, a, AssetLoc::Stock(h));
                if let Some(m) = world.comp_mut::<Asset>(a) {
                    m.stolen = true;
                    m.finance = None;
                }
            }
        }
        _ => {
            if loot.coins > 0 {
                world.purse_add(None, loot.coins);
            }
            for a in assets_at(world, c).to_vec() {
                despawn(world, a);
            }
        }
    }
    let actor = gang.unwrap_or(EntityId::NONE);
    let (dead, by) = (world.name_of(c), gang.map_or_else(|| "scavengers".to_string(), |g| world.owner_label(Some(g))));
    world.push_event(
        EventKind::Stripped,
        &[actor, c],
        format!("{by} stripped the body of {dead} ({} coins)", loot.coins),
    );
    world.stats.current.stripped += 1;
    settle_corpse(world, c);
}

/// Loot coins (held by no purse) into a gang's treasury.
pub fn loot_to_gang(world: &mut World, gang: EntityId, coins: i64) {
    if coins > 0 {
        world.purse_add(Some(gang), coins);
    }
}

// ---------------------------------------------------------------------------
// The parts market (plan D14)
// ---------------------------------------------------------------------------

/// Every Clinic and Garage below `parts_floor` buys up to `parts_batch` at
/// `parts_price` (`Flow::Parts`), from gang Hideouts first (ascending gang
/// id), then the Recycler (revenue to the City).
fn parts_market(world: &mut World) {
    let (floor, batch, price) =
        (world.config.assets.parts_floor, world.config.assets.parts_batch, world.config.assets.parts_price.max(1));
    let mut buyers: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .chain(world.buildings_of_kind(BuildingKind::Garage))
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect();
    buyers.sort_unstable();
    for b in buyers {
        if world.stock(b, Good::Parts) >= floor {
            continue;
        }
        let buyer = world.owner_of(b);
        let room = world.goods_cap(b, Good::Parts).saturating_sub(world.stock(b, Good::Parts));
        let mut want = batch.min(room);
        let mut sources: Vec<(Option<EntityId>, EntityId)> =
            world.gangs().into_iter().filter_map(|g| world.hideout_of(g).map(|h| (Some(g), h))).collect();
        if let Some(r) = world.building_of_kind(BuildingKind::Cemetery) {
            sources.push((None, r));
        }
        for (seller, src) in sources {
            if want == 0 {
                break;
            }
            if seller == buyer && seller.is_some() {
                continue;
            }
            let have = world.stock(src, Good::Parts);
            let afford = match ownership::owner_kind(world, buyer) {
                OwnerKind::City | OwnerKind::Corp(_) => u32::MAX,
                _ => u32::try_from(world.purse(buyer).max(0) / price).unwrap_or(u32::MAX),
            };
            let n = want.min(have).min(afford);
            if n == 0 {
                continue;
            }
            ownership::charge(world, buyer, seller, i64::from(n) * price, Flow::Parts);
            world.take_stock(src, Good::Parts, n);
            world.add_stock(b, Good::Parts, n);
            want -= n;
        }
    }
}

// ---------------------------------------------------------------------------
// Appearance and the sales roll (plan D5, D17)
// ---------------------------------------------------------------------------

/// Plan D5, daily for every adult with a Body: dress from class and wealth,
/// chrome from `Kit.visible`, colours from the gang or the employer's corp.
fn appearance(world: &mut World) {
    let execs = crate::systems::classes::exec_set(world);
    // scan-ok: daily: appearance
    for id in world.citizens() {
        if !world.has::<Body>(id) || !world.has::<Brain>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let class = crate::systems::classes::class_in(world, id, &execs);
        let dress = match class {
            Class::Dreg => 0,
            Class::Corp => 3,
            Class::Street => {
                let spire = world
                    .comp::<crate::components::Household>(id)
                    .and_then(|h| h.home)
                    .and_then(|h| world.comp::<Building>(h))
                    .is_some_and(|b| b.tier >= 2);
                let rich = world.comp::<crate::components::Needs>(id).is_some_and(|n| n.wealth >= 0.8);
                if spire || rich {
                    2
                } else {
                    1
                }
            }
        };
        let chrome = world.comp::<Kit>(id).map_or(0, |k| k.visible);
        let colours = world
            .gang_of(id)
            .or_else(|| world.comp::<Job>(id).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e)));
        let a = Appearance { dress, chrome, colours };
        if world.comp::<Appearance>(id) != Some(&a) {
            world.insert(id, a);
        }
    }
}

/// Roll each building's `asset_sales_today` into its 7-day window.
fn roll_sales(world: &mut World) {
    for b in world.with::<Building>() {
        let Some(bd) = world.comp_mut::<Building>(b) else { continue };
        if bd.asset_sales_today == 0 && bd.asset_sales.is_empty() {
            continue;
        }
        let today = std::mem::take(&mut bd.asset_sales_today);
        bd.asset_sales.push_back(today);
        while bd.asset_sales.len() > SALES_DAYS {
            bd.asset_sales.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// Owners gone (plan D10, D45)
// ---------------------------------------------------------------------------

/// An owner died, left, dissolved or disbanded. An agent's parked vehicles,
/// posted robots and stock pass to its D45 heir (spouse, else first living
/// adult child, else the city); a financed asset in arrears goes to its
/// lender; carried and installed assets stay on the body (D13). A corp's or
/// gang's go to the city. Plans it lent on are re-pointed at the heir (an
/// agent) or the city. The vehicles it kept go home.
pub fn on_owner_gone(world: &mut World, gone: EntityId) {
    let agent = world.has::<Identity>(gone);
    let heir = if agent { agent_heir(world, gone) } else { None };
    for a in assets_of(world, Some(gone)).to_vec() {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        if agent && matches!(x.loc, AssetLoc::Carried(h) | AssetLoc::Installed(h) | AssetLoc::InUse(h) if h == gone) {
            continue;
        }
        let lender = x.finance.as_ref().filter(|f| f.arrears > 0).and_then(|f| f.lender);
        match lender.filter(|&l| l != gone && live_owner(world, Some(l))) {
            Some(l) => take_back(world, a, l),
            None => set_owner(world, a, heir),
        }
    }
    let mut lent: Vec<EntityId> = Vec::new();
    let mut kept: Vec<EntityId> = Vec::new();
    for a in all_assets(world) {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        if x.finance.as_ref().is_some_and(|f| f.lender == Some(gone)) {
            lent.push(a);
        }
        if x.keeper == Some(gone) {
            kept.push(a);
        }
    }
    for a in lent {
        if let Some(f) = world.comp_mut::<Asset>(a).and_then(|x| x.finance.as_mut()) {
            f.lender = heir;
        }
    }
    for a in kept {
        set_keeper(world, a, None);
        let home = world.comp::<Asset>(a).and_then(|x| x.owner).and_then(|o| world.hideout_of(o));
        let driven = world.comp::<Asset>(a).is_some_and(|x| matches!(x.loc, AssetLoc::InUse(_)));
        if let (Some(h), false) = (home, driven) {
            set_loc(world, a, AssetLoc::Parked(h));
        }
    }
}

/// M11 D45's heir: the living spouse, else the first living adult child.
fn agent_heir(world: &World, agent: EntityId) -> Option<EntityId> {
    let alive = |w: &World, id: EntityId| w.has::<Wallet>(id) && w.has::<Brain>(id) && !w.has::<Corpse>(id);
    world.spouse_of(agent).filter(|&s| s != agent && alive(world, s)).or_else(|| {
        crate::systems::demography::children_of_agent(world, agent)
            .into_iter()
            .filter(|&c| c != agent && alive(world, c) && crate::systems::demography::is_adult(world, c))
            .min()
    })
}

// ---------------------------------------------------------------------------
// Buying (plan D29, D30) and god grants
// ---------------------------------------------------------------------------

/// Where a new asset of `kind` goes for `buyer` at `seller`.
fn new_loc(world: &World, kind: AssetKind, buyer: EntityId, seller: EntityId) -> AssetLoc {
    let agent = world.has::<Identity>(buyer);
    match kind {
        k if k.is_vehicle() => AssetLoc::Parked(seller),
        AssetKind::Implant(_) if agent => AssetLoc::Installed(buyer),
        AssetKind::Pack | AssetKind::Bridge if agent => AssetLoc::Carried(buyer),
        // TODO(M13 ph4): a robot is posted at the building `Secure` names.
        _ => AssetLoc::Stock(seller),
    }
}

/// `buyer` (an agent, gang or corp) buys `pick` at `seller` (plan D29):
/// price `round(list × level)` (a used asset `used_frac × list ×
/// condition / 100`), in full or `down_frac` down with a `Finance` to the
/// seller's owner; a new asset's import is paid by the seller's owner
/// first (refused: no sale). `Flow::Asset`, the `AssetBought` event, the
/// seller's credit and sales count, the buyer's `last_shop`.
pub fn buy(world: &mut World, buyer: EntityId, seller: EntityId, pick: &ShopPick) -> Result<EntityId, String> {
    let cfg = world.config.assets.clone();
    if !world.has::<Building>(seller) {
        return Err("no such seller".into());
    }
    let seller_owner = world.owner_of(seller);
    let used = match pick.used {
        Some(u) => {
            let x = world.comp::<Asset>(u).ok_or("the used asset is gone")?;
            if x.loc != AssetLoc::Stock(seller) {
                return Err("the used asset is not in this seller's stock".into());
            }
            Some((u, x.list, x.condition, x.kind, x.tier))
        }
        None => None,
    };
    let (kind, tier) = used.map_or((pick.kind, pick.tier), |(_, _, _, k, t)| (k, t));
    let list = match used {
        Some((_, l, _, _, _)) => l,
        None => list_price(world, kind, tier).ok_or_else(|| format!("no {} T{tier} for sale", kind.label()))?,
    };
    let price = match used {
        Some((_, l, c, _, _)) => (world.config.chrome.used_frac * l as f32 * f32::from(c) / 100.0).round() as i64,
        None => (list as f32 * seller_level(world, seller)).round() as i64,
    };
    let coins = world.purse(Some(buyer));
    let financed = coins < price;
    let down = if financed { (cfg.down_frac * price as f32).round() as i64 } else { price };
    if coins < down || matches!(ownership::owner_kind(world, Some(buyer)), OwnerKind::City) {
        return Err(format!("{} cannot afford {price}", world.owner_label(Some(buyer))));
    }
    // D30: the import, new assets only, paid first by the seller's owner.
    if used.is_none() {
        let half = *cfg.parts_per.get(kind) / 2;
        let parts = world.stock(seller, Good::Parts).min(half);
        let import = ((cfg.import_frac * list as f32).round() as i64 - cfg.part_credit * i64::from(parts)).max(0);
        if !can_pay(world, seller_owner, import) {
            return Err(format!("{} cannot pay the import", world.owner_label(seller_owner)));
        }
        world.take_stock(seller, Good::Parts, parts);
        ownership::charge(world, seller_owner, None, import, Flow::Import);
    }
    let paid = ownership::charge(world, Some(buyer), seller_owner, down, Flow::Asset);
    ownership::credit(world, seller, paid);
    let loc = new_loc(world, kind, buyer, seller);
    let a = match used {
        Some((u, ..)) => {
            set_owner(world, u, Some(buyer));
            set_loc(world, u, loc);
            u
        }
        None => spawn_asset(world, kind, tier, Some(buyer), loc, list),
    };
    if financed {
        let remaining = price - down;
        // Interest in whole coins first, so float noise never adds a coin a day.
        let total = remaining + (remaining as f64 * f64::from(cfg.interest)).round() as i64;
        let term = i64::from(cfg.term_days.max(1));
        let per_day = (total + term - 1) / term;
        let now = world.tick;
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.finance = Some(Finance { lender: seller_owner, remaining, per_day, arrears: 0 });
            m.bought = now;
        }
    }
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Body>(buyer) {
        b.last_shop = Some(now);
    }
    if let Some(bd) = world.comp_mut::<Building>(seller) {
        bd.asset_sales_today = bd.asset_sales_today.saturating_add(1);
    }
    let (who, what, at) = (world.owner_label(Some(buyer)), world.name_of(a), world.name_of(seller));
    let text = format!("{who} bought a {what} at {at} for {price}{}", if financed { " on finance" } else { "" });
    world.push_event(EventKind::AssetBought, &[buyer, seller, a], text);
    Ok(a)
}

/// God (plan D48, phase 1): give `agent` a new asset, free, no import. A
/// vehicle parks at its Home (else the building it is in, else the nearest
/// door), an implant is installed (its slot must be free), a pack or bridge
/// is carried, a robot is posted at its Home.
pub fn grant(world: &mut World, agent: EntityId, kind: AssetKind, tier: u8) -> Result<EntityId, String> {
    if !world.has::<Identity>(agent) || world.has::<Corpse>(agent) {
        return Err("no such living agent".into());
    }
    let list = list_price(world, kind, tier).ok_or_else(|| format!("no {} at tier {tier}", kind.label()))?;
    let home = world.comp::<crate::components::Household>(agent).and_then(|h| h.home);
    let here = world.comp::<Position>(agent).and_then(|p| p.building);
    let near = world.comp::<Position>(agent).and_then(|p| nearest_any_building(world, p.tile));
    let base = home.or(here).or(near);
    let loc = match kind {
        k if k.is_vehicle() => AssetLoc::Parked(base.ok_or("nowhere to park")?),
        AssetKind::Implant(slot) => {
            let taken = assets_at(world, agent).iter().any(|&a| {
                world
                    .comp::<Asset>(a)
                    .is_some_and(|x| x.kind == AssetKind::Implant(slot) && x.loc == AssetLoc::Installed(agent))
            });
            if taken {
                return Err(format!("{} already has {} chrome", world.name_of(agent), slot.label()));
            }
            AssetLoc::Installed(agent)
        }
        AssetKind::Robot => AssetLoc::Posted(base.ok_or("nowhere to post it")?),
        _ => AssetLoc::Carried(agent),
    };
    Ok(spawn_asset(world, kind, tier, Some(agent), loc, list))
}

/// Living agents with a chromed Kit, and the mean sanity of living adults
/// with a Body (the D49 snapshots).
pub fn snapshot(world: &World) -> (u32, f32, u32, [u32; 4]) {
    let mut chrome = 0;
    let (mut sanity, mut n) = (0.0f32, 0u32);
    for &id in world
        .tier(crate::components::Lod::Full)
        .iter()
        .chain(world.tier(crate::components::Lod::Coarse))
        .chain(world.tier(crate::components::Lod::Statistical))
    {
        if world.comp::<Kit>(id).is_some_and(|k| k.chrome) {
            chrome += 1;
        }
        if let Some(b) = world.comp::<Body>(id) {
            if crate::systems::demography::is_adult(world, id) {
                sanity += b.sanity;
                n += 1;
            }
        }
    }
    let mut vehicles = [0u32; 4];
    for &v in &world.vehicles {
        if let Some(x) = world.comp::<Asset>(v) {
            let i = match x.kind {
                AssetKind::Motorcycle => 0,
                AssetKind::Car => 1,
                AssetKind::Truck => 2,
                _ => 3,
            };
            vehicles[i] += 1;
        }
    }
    let robots = all_assets(world)
        .into_iter()
        .filter(|&a| {
            world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Robot && matches!(x.loc, AssetLoc::Posted(_)))
        })
        .count() as u32;
    (chrome, if n > 0 { sanity / n as f32 } else { 0.0 }, robots, vehicles)
}
