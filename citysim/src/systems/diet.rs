//! Hunger as sickness § 1 (docs/HUNGER_SICKNESS.md): synth and real meals on
//! the one `Good::Food`. A shelf unit sold **real** is one meal at the
//! Market's price `P`; sold **synth** it is cut with vat filler into
//! `synth_stretch` meals at `max(1, round(P / stretch))` each, the shelf
//! drained a unit per `stretch` meals through `Market.synth_frac`. One
//! choice function serves both LOD tiers.

use crate::components::{BuildingKind, Market};
use crate::entity::EntityId;
use crate::world::World;

/// Synth and real meals, the meters and the debuffs (`[sickness] diet`;
/// always on since P10 but for the part switch).
pub fn on(world: &World) -> bool {
    world.config.sickness.diet
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Quality {
    Real,
    Synth,
}

/// Meals a shelf unit is cut into (at least 1).
pub fn stretch(world: &World) -> u32 {
    world.config.sickness.synth_stretch.max(1)
}

/// A synth meal's price at real price `p`.
pub fn synth_price(world: &World, p: i64) -> i64 {
    ((p as f64 / f64::from(stretch(world))).round() as i64).max(1)
}

/// The meal `id` would buy at real price `p` (§ 1): real with
/// `real_meal_reserve` real meals' coins, or malnourished past
/// `mal_seek_real` with two; else synth. Real with the diet off.
pub fn quality(world: &World, id: EntityId, p: i64) -> Quality {
    if !on(world) {
        return Quality::Real;
    }
    let cfg = &world.config.sickness;
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
    let mal = crate::systems::health::malnutrition(world, id);
    if coins >= cfg.real_meal_reserve.saturating_mul(p) || (mal >= cfg.mal_seek_real && coins >= 2 * p) {
        Quality::Real
    } else {
        Quality::Synth
    }
}

/// The quality and the per-meal price `id` would pay at `market`.
pub fn offer(world: &World, id: EntityId, market: EntityId) -> (Quality, i64) {
    let p = world.price_for(market, id);
    match quality(world, id, p) {
        Quality::Real => (Quality::Real, p),
        Quality::Synth => (Quality::Synth, synth_price(world, p)),
    }
}

/// The per-meal price `id` pays at `market` (`World::price_for` with the
/// diet off).
pub fn price_at(world: &World, id: EntityId, market: EntityId) -> i64 {
    if !on(world) {
        return world.price_for(market, id);
    }
    offer(world, id, market).1
}

/// Synth meals still in the unit `market` last opened.
pub fn open_meals(world: &World, market: EntityId) -> u32 {
    let frac = world.comp::<Market>(market).map_or(0, |m| m.synth_frac);
    if frac == 0 {
        0
    } else {
        stretch(world).saturating_sub(frac)
    }
}

/// Meals of `q` the shelf of `market` holding `stock` units can sell.
pub fn meals_on_shelf(world: &World, market: EntityId, stock: u32, q: Quality) -> u32 {
    match q {
        Quality::Real => stock,
        Quality::Synth => stock.saturating_mul(stretch(world)).saturating_add(open_meals(world, market)),
    }
}

/// Shelf units `meals` synth meals take from `market` (the open unit's
/// meals first), and the new `synth_frac`.
pub fn synth_units(world: &World, market: EntityId, meals: u32) -> (u32, u32) {
    let s = stretch(world);
    let frac = world.comp::<Market>(market).map_or(0, |m| m.synth_frac);
    let open = open_meals(world, market);
    let rest = meals.saturating_sub(open);
    let units = rest.div_ceil(s);
    (units, (frac + meals) % s)
}

/// The local Market's per-meal price for `id` (3 with no Market), as
/// `life::meal_price` reads it with the diet on.
pub fn meal_price(world: &World, id: EntityId) -> i64 {
    world.local(id, BuildingKind::Market).map_or(3, |m| price_at(world, id, m)).max(1)
}
