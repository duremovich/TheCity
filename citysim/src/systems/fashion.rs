//! Jobs & lives phase 4, fashion (docs/JOBS_AND_LIVES.md § 9, Dylan's
//! answer 1). Under `[fashion] enabled` (itself under `[jobs2] enabled`):
//! off, every entry point is a no-op, no Boutique or clothing item exists,
//! `status` stays 1.0 and no state is written.
//!
//! - **Clothing** is an item kind of phase 3's owned-item store
//!   (`AssetKind::Clothing`: tier 1-3, condition, provenance). The best
//!   item an agent carries is worn; the daily appearance pass sets
//!   `Appearance.dress = max(derived from class and wealth, the worn item's
//!   tier × condition ÷ 100, rounded)`, so a worn item fades as it wears
//!   (`[assets] wear.clothing` a day). Items alive are capped per capita (R4).
//! - **The Boutique** (tailors and barbers) is a venue on the leisure
//!   ladder, weighted by `1 + status_pull × (1 − status)`: a visit pays the
//!   grooming price (`Flow::Leisure`) and raises `status`; a visitor whose
//!   worn dress is under the Boutique's tier (1 + its district tier) buys
//!   the dearest tier above it he can pay, at its list price (`Flow::Asset`),
//!   worn at once, the old one kept at home. Two are seeded per copy of a
//!   `boutique_rows` row: the Spire's (the rich's high rung) and the Mid's. A High-rung evening raises `status` too.
//! - **`Needs.status`** decays `status_decay × (0.5 + pride) × class_w` a day
//!   and feeds mood as `status_w × (status − 0.5)`.
//! - **The readers**: the Lounge's door (dress ≥ 2, unchanged code reading
//!   the new dress), a trade row's `min_dress` at hiring (the Hq's office
//!   posts, the Lounge's companion), and the carry sighting (a dress-0 body
//!   in the Spire noticed × `spire_dress_mult`).
//! - **Theft**: a fight's winner (no guard) takes the loser's worn item on
//!   `p_mug_clothes`, under the petty cap; a Home's kept items go with its
//!   thief (`weapons::home_petty`); both fence through the gang fence, and a
//!   gang dresses its members from the clothing in its stock at midnight.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::components::{
    Asset, AssetKind, AssetLoc, Building, BuildingKind, Class, Corpse, Gang, Household, Identity, Needs, Personality,
    ProvKind, Provenance, Role, Sentence,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::assets;
use crate::systems::ownership::{self, Flow};
use crate::world::World;

// Phase-8 review: its own purpose (it shared 0x4641_0001 with faith's attend).
const PURPOSE_MUG: u64 = 0x4653_0001;

/// Phase 4's fashion is live: `[jobs2] enabled` and `[fashion] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.fashion.enabled
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// Phase 4's fashion state on `Jobs2State` (empty, and not saved, off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FashionState {
    /// Every clothing item (spawns add; the midnight pass rebuilds it).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub index: BTreeSet<EntityId>,
    /// Clothing taken in fights today (the petty cap).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub mugged_today: u32,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl FashionState {
    pub fn is_default(&self) -> bool {
        *self == FashionState::default()
    }
}

// ---------------------------------------------------------------------------
// The items
// ---------------------------------------------------------------------------

/// R4: clothing items alive at most.
pub fn cap(world: &World) -> usize {
    let n = f64::from(world.config.world.population) * f64::from(world.config.fashion.clothing_per_1000) / 1000.0;
    n.round().max(0.0) as usize
}

fn alive(world: &World) -> usize {
    world.jobs2.fashion.index.iter().filter(|&&a| is_clothing(world, a)).count()
}

fn is_clothing(world: &World, a: EntityId) -> bool {
    world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Clothing)
}

/// A worn item's dress: `round(tier × condition ÷ 100)`.
fn item_dress(x: &Asset) -> u8 {
    (f32::from(x.tier) * f32::from(x.condition) / 100.0).round() as u8
}

/// The item `id` wears: the carried clothing of the best dress (ties the
/// lower id).
pub fn worn(world: &World, id: EntityId) -> Option<EntityId> {
    assets::assets_at(world, id)
        .iter()
        .copied()
        .filter_map(|a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter(|(_, x)| x.kind == AssetKind::Clothing && x.loc == AssetLoc::Carried(id) && x.condition > 0)
        .max_by(|(a, x), (b, y)| item_dress(x).cmp(&item_dress(y)).then(b.cmp(a)))
        .map(|(a, _)| a)
}

/// The worn item's dress (0 without one, or with fashion off).
pub fn worn_dress(world: &World, id: EntityId) -> u8 {
    if !on(world) {
        return 0;
    }
    worn(world, id).and_then(|a| world.comp::<Asset>(a)).map_or(0, item_dress)
}

/// Clothing items exist only with the asset store on (`[assets]
/// enabled`): the calibration and parity city, run without assets (its
/// Statistical rows must not hold a Kit or an item), sells none.
fn items_on(world: &World) -> bool {
    on(world) && world.config.assets.enabled
}

/// A new clothing item at `tier`, unless the store is at its cap (or off).
fn spawn_clothing(
    world: &mut World,
    tier: u8,
    owner: Option<EntityId>,
    loc: AssetLoc,
    p: Provenance,
) -> Option<EntityId> {
    if !items_on(world) || alive(world) >= cap(world) {
        return None;
    }
    let tier = tier.clamp(1, 3);
    let list = assets::list_price(world, AssetKind::Clothing, tier).unwrap_or(0);
    let a = assets::spawn_asset(world, AssetKind::Clothing, tier, owner, loc, list);
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.prov = Some(p);
    }
    world.jobs2.fashion.index.insert(a);
    Some(a)
}

/// A clothing item at `tier` worn by `id` (a god's grant; tests), past
/// nothing but the store's cap.
pub fn grant(world: &mut World, id: EntityId, tier: u8) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let p = Provenance { how: ProvKind::Granted, by: None, at: world.tick };
    let a = spawn_clothing(world, tier, Some(id), AssetLoc::Carried(id), p)?;
    wear_new(world, id, a);
    Some(a)
}

/// Put on `a` (carried by `id`): any other worn item goes home (or, with
/// no home, stays carried and is simply not the best).
fn wear_new(world: &mut World, id: EntityId, a: EntityId) {
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let others: Vec<EntityId> = assets::assets_at(world, id)
        .iter()
        .copied()
        .filter(|&o| {
            o != a
                && world
                    .comp::<Asset>(o)
                    .is_some_and(|x| x.kind == AssetKind::Clothing && x.loc == AssetLoc::Carried(id))
        })
        .collect();
    if let Some(h) = home {
        for o in others {
            assets::set_loc(world, o, AssetLoc::Parked(h));
        }
    }
}

/// § 9's hiring reader: a trade row's `min_dress` (fashion on, with the
/// clothing items: without the asset store no one could ever dress up, so
/// the gate would shut the posts for good; the calibration city).
pub fn dress_ok(world: &World, id: EntityId, role: Role) -> bool {
    if !items_on(world) {
        return true;
    }
    let min = crate::systems::trades::row_of(world, role).map_or(0, |r| r.min_dress);
    min == 0 || crate::systems::reputation::appearance_of(world, id).dress >= min
}

/// The carry sighting's dress term: a dress-0 body in the Spire is noticed
/// × `spire_dress_mult` (1 otherwise, or off).
pub fn sighting_mult(world: &World, id: EntityId) -> f32 {
    if !on(world) {
        return 1.0;
    }
    let spire = world
        .comp::<crate::components::Position>(id)
        .is_some_and(|p| world.map.zone(p.tile) == crate::components::Zone::Spire);
    if spire && crate::systems::reputation::appearance_of(world, id).dress == 0 {
        world.config.fashion.spire_dress_mult
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------
// The Boutique and status
// ---------------------------------------------------------------------------

/// A Boutique visit's grooming price by tier (`venue_price`'s base).
pub fn groom_price(world: &World, tier: u8) -> i64 {
    world.config.fashion.groom_price[usize::from(tier.min(2))]
}

/// The tier a Boutique sells: 1 + its district tier.
fn sells_tier(world: &World, b: EntityId) -> u8 {
    1 + world.comp::<Building>(b).map_or(0, |bd| bd.tier.min(2))
}

/// The ladder's weight on a Boutique: `1 + status_pull × (1 − status)`.
pub fn score_mult(world: &World, id: EntityId, kind: BuildingKind) -> f32 {
    if kind != BuildingKind::Boutique {
        return 1.0;
    }
    let s = world.comp::<Needs>(id).map_or(0.5, |n| n.status);
    1.0 + world.config.fashion.status_pull * (1.0 - s).clamp(0.0, 1.0)
}

fn add_status(world: &mut World, id: EntityId, gain: f32) {
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.status = (n.status + gain).min(1.0);
    }
}

/// A paid venue visit done: at a Boutique `status_gain` and, under its
/// tier, an item bought and worn; at a High-rung venue `status_high`.
pub fn on_visit(world: &mut World, visitor: EntityId, b: EntityId, high: bool) {
    if !on(world) {
        return;
    }
    let cfg = world.config.fashion.clone();
    if high {
        add_status(world, visitor, cfg.status_high);
    }
    if world.comp::<Building>(b).map(|bd| bd.kind) != Some(BuildingKind::Boutique) {
        return;
    }
    add_status(world, visitor, cfg.status_gain);
    ownership::jobs2_row(world).boutique_visits += 1;
    // The dearest tier it sells above the worn dress that the visitor can pay.
    let worn = worn_dress(world, visitor);
    // Clothes come after food: the purchase leaves `clothes_reserve_meals`
    // meals' coins on top of the evening's own reserve.
    let keep = world.config.fashion.clothes_reserve_meals * crate::systems::life::meal_price(world, visitor);
    let spend = crate::systems::leisure::spendable(world, visitor) - keep;
    let level = assets::seller_level(world, b);
    let pick = (worn + 1..=sells_tier(world, b)).rev().find_map(|t| {
        let list = assets::list_price(world, AssetKind::Clothing, t)?;
        let price = (list as f32 * level).round().max(1.0) as i64;
        (price <= spend).then_some((t, price))
    });
    let Some((t, price)) = pick else { return };
    if !items_on(world) || alive(world) >= cap(world) {
        return;
    }
    let owner = world.owner_of(b);
    let paid = ownership::pay(world, Some(visitor), owner, price, Flow::Asset);
    if paid < price {
        return;
    }
    ownership::credit(world, b, paid);
    let p = Provenance { how: ProvKind::Sold, by: Some(b), at: world.tick };
    let Some(a) = spawn_clothing(world, t, Some(visitor), AssetLoc::Carried(visitor), p) else { return };
    wear_new(world, visitor, a);
    ownership::jobs2_row(world).clothes_sold += 1;
    let text = format!("{} bought a Clothing T{t} at {} for {price}", world.name_of(visitor), world.name_of(b));
    world.push_event(EventKind::AssetBought, &[visitor, b, a], text);
}

/// Mood's `status_w × (status − 0.5)` (0 with fashion off).
pub fn mood_bias(world: &World, id: EntityId) -> f32 {
    if !on(world) {
        return 0.0;
    }
    world.comp::<Needs>(id).map_or(0.0, |n| world.config.fashion.status_w * (n.status - 0.5))
}

/// A fight's end (`weapons::after_fight`'s caller, fashion on): a winner who
/// is no guard takes the loser's worn item on `p_mug_clothes`, under the
/// petty cap (stolen; worn by him when it dresses him better).
pub fn after_fight(world: &mut World, winner: EntityId, loser: EntityId) {
    if !on(world) || !world.has::<crate::components::Brain>(winner) || crate::systems::law::is_guard(world, winner) {
        return;
    }
    let cap = crate::systems::weapons::per_capita_day(world, world.config.weapons.petty_per_1000_day);
    if world.jobs2.fashion.mugged_today >= cap {
        return;
    }
    let Some(a) = worn(world, loser) else { return };
    let u = crate::systems::econ::hash_unit(world.seed(), PURPOSE_MUG, key(winner), world.tick);
    if u >= world.config.fashion.p_mug_clothes {
        return;
    }
    assets::reset_title(world, a);
    assets::set_owner(world, a, Some(winner));
    assets::set_loc(world, a, AssetLoc::Carried(winner));
    let pv = Provenance { how: ProvKind::Stolen, by: Some(winner), at: world.tick };
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.prov = Some(pv);
        x.stolen = true;
    }
    if worn(world, winner) == Some(a) {
        wear_new(world, winner, a);
    }
    world.jobs2.fashion.mugged_today += 1;
    ownership::jobs2_row(world).clothes_stolen += 1;
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Midnight (from `living::run`): the index rebuilt, `status`'s decay, a
/// gang's stock dressing its members, the cap reset.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("fashion::daily");
    world.jobs2.fashion.mugged_today = 0;
    let index: BTreeSet<EntityId> =
        world.jobs2.fashion.index.iter().copied().filter(|&a| is_clothing(world, a)).collect();
    world.jobs2.fashion.index = index;
    let cfg = world.config.fashion.clone();
    let execs = crate::systems::classes::exec_set(world);
    // scan-ok: daily: status decay
    for id in world.with::<Identity>() {
        if world.has::<Corpse>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let pride = world.comp::<Personality>(id).map_or(0.5, |p| p.pride);
        let w = cfg.class_w[class_index(crate::systems::classes::class_in(world, id, &execs))];
        let d = cfg.status_decay * (0.5 + pride) * w;
        if let Some(n) = world.comp_mut::<Needs>(id) {
            n.status = (n.status - d).max(0.0);
        }
    }
    dress_gangs(world);
}

fn class_index(c: Class) -> usize {
    match c {
        Class::Corp => 0,
        Class::Street => 1,
        Class::Dreg => 2,
    }
}

/// A gang's stocked clothing (fenced) dresses its free members whose worn
/// dress is under the item's (the best items to the lowest ids).
fn dress_gangs(world: &mut World) {
    for g in world.gangs() {
        let Some(h) = world.hideout_of(g) else { continue };
        let mut stock: Vec<EntityId> = assets::assets_at(world, h)
            .iter()
            .copied()
            .filter(|&a| {
                world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Clothing && x.loc == AssetLoc::Stock(h))
            })
            .collect();
        if stock.is_empty() {
            continue;
        }
        stock.sort_by_key(|&a| (std::cmp::Reverse(world.comp::<Asset>(a).map_or(0, item_dress)), a));
        let members = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
        for m in members {
            let Some(&a) = stock.first() else { break };
            if world.has::<Sentence>(m) || !crate::systems::law::living(world, m) {
                continue;
            }
            let d = world.comp::<Asset>(a).map_or(0, item_dress);
            if worn_dress(world, m) >= d {
                continue;
            }
            stock.remove(0);
            assets::reset_title(world, a);
            assets::set_owner(world, a, Some(m));
            assets::set_loc(world, a, AssetLoc::Carried(m));
            wear_new(world, m, a);
        }
    }
}

/// `World::new`: two Boutiques per copy of each `[fashion] boutique_rows`
/// row on the Lots left, one in the Spire and one in the Mid
/// (`industry::seed_kind_in`). No RNG, no coins. None without the asset
/// store (a Boutique sells the clothing items; the calibration city).
pub fn seed(world: &mut World) {
    if !items_on(world) {
        return;
    }
    let rows = world.config.fashion.boutique_rows.clone();
    let copies = world.lot_copies();
    for j in 0..copies {
        for &r in &rows {
            let Some(t) = world.config.corps.row_template(r) else { continue };
            let Some(&c) = world.corps_of_template(t).get(j) else { continue };
            // One in the Spire (the rich's high rung), one in the Mid.
            for zone in [crate::components::Zone::Spire, crate::components::Zone::Mid] {
                crate::systems::industry::seed_kind_in(world, BuildingKind::Boutique, Some(c), zone);
            }
        }
    }
}

/// The day's end (`systems::stats`): the fashion group's snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let boutiques = world
        .buildings_of_kind(BuildingKind::Boutique)
        .iter()
        .filter(|&&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .count() as u32;
    let mut clothing = 0u32;
    let mut wearers: BTreeSet<EntityId> = BTreeSet::new();
    for &a in &world.jobs2.fashion.index {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        if x.kind != AssetKind::Clothing {
            continue;
        }
        clothing += 1;
        if let AssetLoc::Carried(h) = x.loc {
            if world.has::<Identity>(h) {
                wearers.insert(h);
            }
        }
    }
    let execs = crate::systems::classes::exec_set(world);
    let mut sum = [0.0f32; 3];
    let mut n = [0u32; 3];
    for id in world.with::<Identity>() {
        if world.has::<Corpse>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let Some(s) = world.comp::<Needs>(id).map(|x| x.status) else { continue };
        let i = class_index(crate::systems::classes::class_in(world, id, &execs));
        sum[i] += s;
        n[i] += 1;
    }
    let mean = |i: usize| if n[i] > 0 { sum[i] / n[i] as f32 } else { 0.0 };
    let (sc, ss, sd) = (mean(0), mean(1), mean(2));
    let row = ownership::jobs2_row(world);
    row.boutiques = boutiques;
    row.clothing = clothing;
    row.clothes_worn = wearers.len() as u32;
    row.status_corp = sc;
    row.status_street = ss;
    row.status_dreg = sd;
}
