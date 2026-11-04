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
//!
//! Payments are all-or-nothing (phase 2, from the phase 1 review): a day's
//! upkeep or finance payment an agent or gang cannot cover in full is not
//! taken at all (the wallet is left alone) and the arrears grow; a corp or
//! the city always pays (`charge` lets them go negative). A partial sweep
//! used to empty a short payer's wallet and count the arrears anyway.
//!
//! Phase 2 adds the sellers (plan D16-D18, D29, D43): [`seller_open`],
//! [`shop_choice`] (the Shop goal's offer), the Statistical shop, the
//! Garage rent, and [`seed_sellers`].

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{
    Appearance, Asset, AssetKind, AssetLoc, Body, Brain, Building, BuildingKind, Class, Controller, Corp, Corpse,
    Finance, Gang, Good, Identity, Inventory, Job, Kit, Lod, MemoryKind, Position, Sentence, ShopPick, Slot, TilePos,
    Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{Tick, TICKS_PER_HOUR};
use crate::world::World;

mod estate;
mod kit;
mod registry;
mod sellers;
mod shop;
mod upkeep;

pub use estate::*;
pub use kit::*;
pub use registry::*;
pub use sellers::*;
pub use shop::*;
pub use upkeep::*;

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
/// `price_level` for the building's niche (`Tech` for a Clinic or Garage),
/// 1.0 for an agent or the city.
pub fn seller_level(world: &World, seller: EntityId) -> f32 {
    let Some(b) = world.comp::<Building>(seller) else { return 1.0 };
    let niche = match b.kind {
        BuildingKind::Market => crate::components::Niche::Food,
        BuildingKind::SecurityOffice => crate::components::Niche::Security,
        BuildingKind::Clinic | BuildingKind::Garage => crate::components::Niche::Tech,
        _ => return 1.0,
    };
    world.corp_of_building(seller).and_then(|c| world.comp::<Corp>(c)).map_or(1.0, |c| c.level(niche))
}

// ---------------------------------------------------------------------------
// Tech gates and effective tiers (M14 V22, V23)
// ---------------------------------------------------------------------------

/// M14 V23: the tier an asset works at: `tier` when it has no maker (a god
/// grant); `min(tier, orphan_cap)`
/// when the maker corp is gone; else `min(tier, maker's tier in the
/// kind's track)`. Prices, upkeep, value and the sanity `load` keep the
/// nominal tier.
pub fn eff_tier(world: &World, a: EntityId) -> u8 {
    world.comp::<Asset>(a).map_or(0, |x| eff_tier_of(world, x))
}

/// [`eff_tier`] of an asset in hand.
pub fn eff_tier_of(world: &World, x: &Asset) -> u8 {
    let Some(m) = x.maker else { return x.tier };
    x.tier.min(crate::systems::virt::maker_tier(world, m, crate::systems::tech::track_of(x.kind)))
}

/// M14 V22: the tier a seller building's owner sells at in `kind`'s track:
/// a corp's own tier, else (an agent or the city) the street tier; and the
/// corp recorded as the maker of what it sells.
fn seller_tech(world: &World, seller: EntityId, kind: AssetKind) -> (u8, Option<EntityId>) {
    let track = crate::systems::tech::track_of(kind);
    match world.corp_of_building(seller).and_then(|c| world.comp::<Corp>(c).map(|cc| (c, cc))) {
        Some((c, cc)) if !cc.tech.is_unset() => (cc.tech.tier_of(track), Some(c)),
        _ => crate::systems::tech::street_tier(world, track),
    }
}

/// M14 V22: may `seller` sell a new `kind` at `tier`? While its owner's tier
/// in the kind's track is at least `[tech] requires[kind][tier - 1]` (a tier
/// past the list is not gated). Always with the plane off. Used and
/// repossessed stock is exempt (made already): callers gate new stock only.
pub fn can_sell(world: &World, seller: EntityId, kind: AssetKind, tier: u8) -> bool {
    let Some(&need) = world.config.tech.requires.of(kind).get(usize::from(tier.saturating_sub(1))) else {
        return true;
    };
    seller_tech(world, seller, kind).0 >= need
}

/// M14 V22: the maker a new asset from `seller` records (`None` with the plane off).
pub fn maker_for(world: &World, seller: EntityId, kind: AssetKind) -> Option<EntityId> {
    seller_tech(world, seller, kind).1
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
        // M14 V23: a pack carries at its effective tier.
        .filter_map(|x| cfg.pack.get(usize::from(eff_tier_of(world, x).saturating_sub(1))).copied())
        .max()
        .unwrap_or(0);
    cfg.carry_base + carry + pack
}

// ---------------------------------------------------------------------------
// The daily pass (plan D9)
// ---------------------------------------------------------------------------

/// Midnight, right after ownership's pass, while `[assets] enabled`:
/// upkeep, finance, repossession and impound, wear and wrecks, repairs,
/// the corpse window and the scavs, the parts market, appearance, and the
/// 7-day sales roll. Nothing per tick.
pub fn run(world: &mut World) {
    if !world.config.assets.enabled {
        return;
    }
    // Phase 3 (D33): episodes end on the hour.
    if world.tick.is_multiple_of(TICKS_PER_HOUR) && !world.episodes.is_empty() {
        crate::systems::chrome::episodes_hourly(world);
    }
    if world.tick_of_day() != 0 {
        return;
    }
    let unpaid = upkeep(world);
    finance(world);
    repossess(world);
    wear(world, &unpaid);
    repairs(world);
    garage_rent(world);
    crate::systems::chrome::sanity_daily(world);
    settle_window(world);
    scav_strip(world);
    parts_market(world);
    crate::systems::vehicles::recover_abandoned(world);
    crate::systems::vehicles::fleet_recall(world);
    crate::systems::vehicles::theft_daily(world);
    // Phase 4 (D39): addiction decay and the Statistical addict pass.
    crate::systems::stims::addiction_daily(world);
    stat_shop(world);
    appearance(world);
    roll_sales(world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Market;
    use crate::time::TICKS_PER_DAY;

    const SELLERS: [BuildingKind; 4] =
        [BuildingKind::Garage, BuildingKind::Clinic, BuildingKind::SecurityOffice, BuildingKind::Market];

    /// `f` with the indexed reads switched to their scans.
    fn scan<R>(f: impl FnOnce() -> R) -> R {
        crate::spatial::SCANS.with(|s| s.set(true));
        let r = f();
        crate::spatial::SCANS.with(|s| s.set(false));
        r
    }

    /// Every price-keyed read the cached way first (on the memo the last
    /// check left), then by the scan: `market_by_price` and `nearest_seller`
    /// (open and not) from every seller's door and a sample of the
    /// citizens' tiles, `lowest_level` per seller kind and `mean_price`.
    /// Returns how many Market picks were not the nearest Market (the price
    /// and the shelf at work).
    fn check(w: &World, label: &str) -> usize {
        let mut tiles: Vec<TilePos> = SELLERS
            .iter()
            .flat_map(|&k| w.buildings_of_kind(k).iter())
            .filter_map(|&b| w.comp::<Building>(b).map(|bd| bd.door))
            .collect();
        // scan-ok: a unit test.
        tiles.extend(w.citizens().iter().step_by(5).filter_map(|&a| w.comp::<Position>(a).map(|p| p.tile)));
        tiles.push(TilePos::default());
        let mut priced = 0;
        for &t in &tiles {
            let got = w.market_by_price(t);
            assert_eq!(got, scan(|| w.market_by_price(t)), "{label}: market_by_price from {t:?}");
            priced += usize::from(got.is_some() && got != w.nearest_of_kind(BuildingKind::Market, t));
            for kind in SELLERS {
                for open in [false, true] {
                    let got = nearest_seller(w, kind, t, open);
                    let want = scan(|| nearest_seller(w, kind, t, open));
                    assert_eq!(got, want, "{label}: nearest_seller {kind:?} open {open} from {t:?}");
                }
            }
        }
        for kind in SELLERS {
            assert_eq!(lowest_level(w, kind), scan(|| lowest_level(w, kind)), "{label}: lowest_level {kind:?}");
        }
        assert_eq!(w.mean_price(), scan(|| w.mean_price()), "{label}: mean_price");
        priced
    }

    /// The perf pass phase 4: the price-keyed caches are the scans they
    /// replaced on the shipped city through each of their invalidation
    /// points, read at once after each: the midnight price move, a price and
    /// a corp's level written mid-day, a seller re-owned, a Market founded,
    /// one closed (re-kinded), a Garage demolished, a Market shut by a riot
    /// and an empty shelf. Without `shop_gen` (the Building, Corp and Market
    /// stores' writes) or `kinds_gen` a read returns the stale answer.
    #[test]
    fn test_price_keyed_caches_match_the_scans() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 9 * TICKS_PER_HOUR);
        let mut priced = check(&w, "day 1 09:00");
        // Midnight: the prices move.
        let tenths = |w: &World| -> Vec<i64> {
            w.buildings_of_kind(BuildingKind::Market).iter().map(|&m| w.price_tenths_at(m)).collect()
        };
        let before = tenths(&w);
        w.run_ticks(TICKS_PER_DAY);
        assert_ne!(tenths(&w), before, "no price moved at midnight");
        priced += check(&w, "day 2 09:00");
        // A price written mid-day: the dearest Market the cheapest.
        let markets = w.buildings_of_kind(BuildingKind::Market).to_vec();
        let dear = *markets.iter().max_by_key(|&&m| (w.price_tenths_at(m), m)).expect("a Market");
        let mean = w.mean_price();
        if let Some(m) = w.comp_mut::<Market>(dear) {
            m.price_food = 1;
            m.price_tenths = 10;
        }
        assert_ne!(scan(|| w.mean_price()), mean, "the write moved no mean");
        priced += check(&w, "a price written");
        // A corp's Tech level written: the cheapest Garage cheaper still.
        let garages = w.buildings_of_kind(BuildingKind::Garage).to_vec();
        let (g, corp) = garages.iter().find_map(|&g| w.corp_of_building(g).map(|c| (g, c))).expect("a corp's Garage");
        let low = lowest_level(&w, BuildingKind::Garage);
        if let Some(c) = w.comp_mut::<Corp>(corp) {
            c.price_level.insert(crate::components::Niche::Tech, 0.25);
        }
        assert_ne!(scan(|| lowest_level(&w, BuildingKind::Garage)), low, "the level moved nothing");
        priced += check(&w, "a corp's level written");
        // That Garage re-owned by the city (level 1.0).
        ownership::transfer_building(&mut w, g, None);
        priced += check(&w, "a Garage re-owned");
        // A Market founded on the empty Lot nearest a sample agent.
        // scan-ok: a unit test.
        let people = w.citizens();
        let a = people[people.len() / 3];
        let at = w.comp::<Position>(a).expect("a tile").tile;
        let lot = w
            .buildings_of_kind(BuildingKind::Lot)
            .iter()
            .copied()
            .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished && bd.occupants.is_empty()))
            .min_by_key(|&b| (w.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(at)), b))
            .expect("a Lot");
        crate::systems::founding::convert(&mut w, lot, BuildingKind::Market, None, 1);
        assert!(w.buildings_of_kind(BuildingKind::Market).contains(&lot), "not founded");
        priced += check(&w, "a Market founded");
        // The Market nearest another agent closed (re-kinded a Home).
        let c = people[2 * people.len() / 3];
        let cat = w.comp::<Position>(c).expect("a tile").tile;
        let shut = w.nearest_of_kind(BuildingKind::Market, cat).expect("a Market");
        crate::systems::founding::convert(&mut w, shut, BuildingKind::Home, None, 0);
        assert!(!w.buildings_of_kind(BuildingKind::Market).contains(&shut), "not closed");
        priced += check(&w, "a Market closed");
        // A Garage demolished, the next Market shut by a riot, the one after
        // with an empty shelf (query-time gates).
        if let Some(bd) = garages.iter().find(|&&x| x != g).and_then(|&x| w.comp_mut::<Building>(x)) {
            bd.demolished = true;
        }
        let riot = w.market_by_price(cat).expect("a Market");
        let until = w.tick + TICKS_PER_HOUR;
        if let Some(bd) = w.comp_mut::<Building>(riot) {
            bd.closed_until = Some(until);
        }
        let bare = w.market_by_price(cat).expect("a Market");
        assert_ne!(bare, riot, "the riot closed nothing");
        if let Some(bd) = w.comp_mut::<Building>(bare) {
            bd.stock_food = 0;
        }
        priced += check(&w, "demolished, shut and emptied");
        // Ten minutes of the city on.
        w.run_ticks(10);
        priced += check(&w, "ten minutes on");
        assert!(priced > 0, "every pick the nearest Market: the price term is not exercised");
    }
}
