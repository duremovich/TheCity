//! M13 phase 4: Stims, addiction and the drug trade (docs/M13_ASSETS.md § 4,
//! plan D38-D40).
//!
//! A gang cooks doses into its Hideout's stock; a share of its loyal members
//! deal them at a Bar; buyers come to the dealer (or, with the legal lever,
//! to a Market) and use them; addiction rises per dose and falls on a clean
//! day; the hooked go into withdrawal, a body term read on demand from
//! `Body.last_use` (no per-tick counter); Detox at a Clinic cuts it. Every
//! sale is a dice roll against the M9 witnesses (`Crime::Dealing`).
//!
//! Everything here is a state change between fictional agents in a city
//! simulation. Nothing runs per agent per tick: the cook is the gang's daily
//! pass, the addict pass and the decay are daily (`assets::run`, plan D9),
//! the legal restock is the economy's midnight, the rest is event-driven by
//! actions. Rolls (overdose, the Statistical detox) draw from the world
//! stream at those fixed points, in ascending id order.

use rand::Rng;

use crate::components::{
    Body, Brain, Building, BuildingKind, Crime, DeathCause, Gang, Good, Inventory, Lod, Market, Needs, Personality,
    Position, Sentence, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::assets;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// Phase 4 is on with `[assets] enabled` (plan D50).
pub fn on(world: &World) -> bool {
    world.config.assets.enabled
}

/// A dose within this many hours of a stim multiplies the episode roll
/// (plan 4.3: "within 12 h of a stim").
pub const EPISODE_STIM_HOURS: u64 = 12;

// ---------------------------------------------------------------------------
// The body (D39)
// ---------------------------------------------------------------------------

/// The last dose was within `hours`.
pub fn used_within(world: &World, agent: EntityId, hours: u64) -> bool {
    let now = world.tick;
    world.comp::<Body>(agent).and_then(|b| b.last_use).is_some_and(|t| now.saturating_sub(t) < hours * TICKS_PER_HOUR)
}

/// High: a dose within `stim_hours`.
pub fn is_high(world: &World, agent: EntityId) -> bool {
    on(world) && used_within(world, agent, u64::from(world.config.stims.stim_hours))
}

/// Hooked: `addiction ≥ hooked`.
pub fn is_hooked(world: &World, agent: EntityId) -> bool {
    on(world) && world.comp::<Body>(agent).is_some_and(|b| b.addiction >= world.config.stims.hooked)
}

/// D39, derived on read: hooked, and no dose for `withdrawal_hours`.
pub fn in_withdrawal(world: &World, agent: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let cfg = &world.config.stims;
    let Some(b) = world.comp::<Body>(agent) else { return false };
    if b.addiction < cfg.hooked {
        return false;
    }
    let window = u64::from(cfg.withdrawal_hours) * TICKS_PER_HOUR;
    b.last_use.is_none_or(|t| world.tick.saturating_sub(t) >= window)
}

/// The GetHigh goal's craving: `addiction × clamp(hours since the last
/// dose ÷ withdrawal_hours, 0, 1)` (never used: the full gap).
pub fn craving(world: &World, agent: EntityId) -> f32 {
    let Some(b) = world.comp::<Body>(agent) else { return 0.0 };
    if b.addiction <= 0.0 {
        return 0.0;
    }
    let window = (u64::from(world.config.stims.withdrawal_hours) * TICKS_PER_HOUR).max(1) as f32;
    let gap = b.last_use.map_or(1.0, |t| (world.tick.saturating_sub(t) as f32 / window).clamp(0.0, 1.0));
    b.addiction * gap
}

/// D33/D39: the stim terms of the body's mood bias: `+stim_mood` while
/// high, `−withdrawal_mood` in withdrawal; 0 otherwise.
pub fn mood_terms(world: &World, agent: EntityId) -> f32 {
    if !on(world) {
        return 0.0;
    }
    let cfg = &world.config.stims;
    if is_high(world, agent) {
        cfg.stim_mood
    } else if in_withdrawal(world, agent) {
        -cfg.withdrawal_mood
    } else {
        0.0
    }
}

/// D39: energy decay × `withdrawal_energy` in withdrawal (`None`: today's arithmetic).
pub fn energy_mult(world: &World, agent: EntityId) -> Option<f32> {
    in_withdrawal(world, agent).then_some(world.config.stims.withdrawal_energy)
}

/// One dose's body effects (D39): energy + `stim_energy`, addiction +
/// `addict_per_use × (1.5 − lawfulness)`, `last_use = now`; a dose taken at
/// `addiction ≥ 0.8` (before it) rolls `p_overdose` on the world stream.
/// Returns false when it killed.
fn dose(world: &mut World, agent: EntityId) -> bool {
    let cfg = world.config.stims.clone();
    let now = world.tick;
    let lawfulness = world.comp::<Personality>(agent).map_or(0.5, |p| p.lawfulness);
    let Some(b) = world.comp_mut::<Body>(agent) else { return true };
    let was = b.addiction;
    b.addiction = (b.addiction + cfg.addict_per_use * (1.5 - lawfulness)).clamp(0.0, 1.0);
    b.last_use = Some(now);
    if let Some(n) = world.comp_mut::<Needs>(agent) {
        n.energy = (n.energy + cfg.stim_energy).min(1.0);
    }
    if was < 0.8 {
        return true;
    }
    let p = f64::from(cfg.p_overdose.clamp(0.0, 1.0));
    if !world.rng.world().random_bool(p) {
        return true;
    }
    let d = world.comp::<Position>(agent).map(|p| world.district_of(p.tile));
    let place = d.map_or_else(|| "the city".to_string(), |d| world.district_name(d).to_string());
    let text = format!("{} overdosed in {place}", world.name_of(agent));
    world.push_event(EventKind::Overdose, &[agent], text);
    world.kill_by(agent, DeathCause::Overdose, None);
    false
}

/// `UseStim`: a dose from the inventory. Returns false with none to take.
pub fn use_stim(world: &mut World, agent: EntityId) -> bool {
    let Some(inv) = world.comp_mut::<Inventory>(agent) else { return false };
    if inv.stims == 0 {
        return false;
    }
    inv.stims -= 1;
    dose(world, agent);
    true
}

// ---------------------------------------------------------------------------
// Supply: the cook (D38)
// ---------------------------------------------------------------------------

/// D38, daily in `gang::daily_economy` before the chop and the purchases:
/// `n = min(cook_per_member × members, cook_target − stock, treasury ÷
/// cook_cost)` doses into the Hideout, paid as `Flow::Import`.
pub fn cook(world: &mut World, gang: EntityId) {
    if !on(world) {
        return;
    }
    let cfg = world.config.stims.clone();
    let Some(h) = world.hideout_of(gang) else { return };
    let members = world.comp::<Gang>(gang).map_or(0, |g| g.members.len()) as u32;
    let stock = world.stock(h, Good::Stims);
    let treasury = world.purse(Some(gang)).max(0);
    let cost = cfg.cook_cost.max(1);
    let afford = u32::try_from(treasury / cost).unwrap_or(u32::MAX);
    let n = (cfg.cook_per_member * members).min(cfg.cook_target.saturating_sub(stock)).min(afford);
    if n == 0 {
        return;
    }
    let paid = ownership::pay(world, Some(gang), None, i64::from(n) * cost, Flow::Import);
    let n = u32::try_from(paid / cost).unwrap_or(0);
    world.add_stock(h, Good::Stims, n);
}

// ---------------------------------------------------------------------------
// Dealing (D38)
// ---------------------------------------------------------------------------

/// A stable 0..100 share of an entity (splitmix64 of its id), so the same
/// members deal whatever the day.
fn share_of(id: EntityId) -> u64 {
    let mut z = (u64::from(id.index) << 32 | u64::from(id.generation)).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) % 100
}

/// The gang leader's greed (0.5 without a leader).
fn leader_greed(world: &World, gang: EntityId) -> f32 {
    world.comp::<Gang>(gang).and_then(|g| g.leader).and_then(|l| world.comp::<Personality>(l)).map_or(0.5, |p| p.greed)
}

/// D38: the member falls in the leader's `deal_share = 0.3 + 0.4 × greed`
/// of the roster by a stable hash of its id.
pub fn is_dealer(world: &World, member: EntityId) -> bool {
    let Some(gang) = world.gang_of(member) else { return false };
    let share = (100.0 * (0.3 + 0.4 * leader_greed(world, gang))).round() as u64;
    share_of(member) < share
}

/// D38: the Bar whose door is nearest the centroid of the gang's most-held
/// district (ties the lower id; no Homes held: its Hideout's district).
pub fn deal_bar(world: &World, gang: EntityId) -> Option<EntityId> {
    let held = crate::systems::gang::held_districts(world, gang);
    let d = match held.iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))) {
        Some(&(d, _)) => d,
        None => world.district_of_building(world.hideout_of(gang)?),
    };
    // L2 L20: with leisure on, the gang's open front (or a Club there) first.
    if let Some(b) = crate::systems::leisure::deal_venue(world, gang, d) {
        return Some(b);
    }
    let centroid = world.district(d).centroid;
    world
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict) && !world.is_closed(b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(centroid), b)))
        .min()
        .map(|(_, b)| b)
}

/// D38, for `gang::gang_work_target`: the deal Bar when this member deals
/// today: loyal (`following_order`), a dealer by the hash, and the gang's
/// stock holds a batch (or the member still carries doses).
pub fn dealer_target(world: &World, agent: EntityId) -> Option<EntityId> {
    if !on(world) || crate::systems::gang::following_order(world, agent).is_none() || !is_dealer(world, agent) {
        return None;
    }
    let gang = world.gang_of(agent)?;
    let stock = world.hideout_of(gang).map_or(0, |h| world.stock(h, Good::Stims));
    let carrying = world.comp::<Inventory>(agent).is_some_and(|i| i.stims > 0);
    if stock < world.config.stims.deal_batch && !carrying {
        return None;
    }
    deal_bar(world, gang)
}

/// `PickUp` at the Hideout: `deal_batch` doses into the inventory, as many
/// as the capacity allows. Returns the doses taken.
pub fn pick_up(world: &mut World, agent: EntityId) -> u32 {
    let Some(h) = world.gang_of(agent).and_then(|g| world.hideout_of(g)) else { return 0 };
    let room = world
        .comp::<Inventory>(agent)
        .map_or(0, |i| assets::capacity(world, agent).saturating_sub(assets::load(i)))
        .min(u32::from(u16::MAX));
    let want = world.config.stims.deal_batch.min(room);
    let n = world.take_stock(h, Good::Stims, want);
    if let Some(i) = world.comp_mut::<Inventory>(agent) {
        i.stims = i.stims.saturating_add(n as u16);
    }
    n
}

/// `Deal` starts: the dealer is registered at the Bar (and logged for the
/// Statistical pass).
pub fn start_deal(world: &mut World, dealer: EntityId, bar: EntityId) {
    let list = world.dealers.entry(bar).or_default();
    if let Err(i) = list.binary_search(&dealer) {
        list.insert(i, dealer);
    }
    let day = world.day();
    world.deal_log.insert(bar, (dealer, day));
}

/// `Deal` ends (completion, abort, death, arrest): unregistered everywhere.
pub fn end_deal(world: &mut World, dealer: EntityId) {
    if world.dealers.is_empty() {
        return;
    }
    for list in world.dealers.values_mut() {
        list.retain(|&d| d != dealer);
    }
    world.dealers.retain(|_, l| !l.is_empty());
}

/// The registered dealer at `bar` holding doses (lowest id), alive and in it.
pub fn bar_dealer(world: &World, bar: EntityId) -> Option<EntityId> {
    world.dealers.get(&bar)?.iter().copied().find(|&d| {
        crate::systems::law::living(world, d)
            && world.comp::<Inventory>(d).is_some_and(|i| i.stims > 0)
            && world.comp::<Position>(d).is_some_and(|p| p.building == Some(bar))
    })
}

/// A dealer's dose: `round(deal_price × (0.8 + 0.4 × leader greed))`.
pub fn dose_price(world: &World, gang: EntityId) -> i64 {
    let p = world.config.stims.deal_price as f32 * (0.8 + 0.4 * leader_greed(world, gang));
    (p.round() as i64).max(1)
}

/// D40: a legal Market's dose: `round(stim_price × the owner's Food price_level)`.
pub fn legal_price(world: &World, market: EntityId) -> i64 {
    let p = world.config.stims.stim_price as f32 * assets::seller_level(world, market);
    (p.round() as i64).max(1)
}

/// A legal Market with Stims on the shelf, open.
fn legal_market(world: &World, b: EntityId) -> bool {
    world.levers.stims_legal
        && world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Market && !bd.demolished)
        && !world.is_closed(b)
        && world.stock(b, Good::Stims) > 0
}

/// A dose's price at `b` when `b` is a Stims source now: a Bar with a
/// dealer holding doses, or a legal Market with stock.
pub fn source_price(world: &World, b: EntityId) -> Option<i64> {
    if let Some(d) = bar_dealer(world, b) {
        return world.gang_of(d).map(|g| dose_price(world, g));
    }
    legal_market(world, b).then(|| legal_price(world, b))
}

/// D38/D40 (`LocationKey::StimSource`): the nearest Bar with a dealer
/// holding doses (door Manhattan from the agent, ties the lower id), else
/// with the legal lever the nearest Market with stock.
pub fn stim_source(world: &World, agent: EntityId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let from = world.comp::<Position>(agent)?.tile;
    let door = |b: EntityId| world.comp::<Building>(b).map(|bd| bd.door.manhattan(from));
    let bar = world
        .dealers
        .keys()
        .copied()
        .filter(|&b| bar_dealer(world, b).is_some_and(|d| d != agent))
        .filter_map(|b| door(b).map(|d| (d, b)))
        .min()
        .map(|(_, b)| b);
    if bar.is_some() || !world.levers.stims_legal {
        return bar;
    }
    world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .filter(|&b| legal_market(world, b))
        .filter_map(|b| door(b).map(|d| (d, b)))
        .min()
        .map(|(_, b)| b)
}

/// The agent's source is a legal Market (GetHigh's lawfulness term is
/// for illegal stims only).
pub fn legal_reach(world: &World, agent: EntityId) -> bool {
    stim_source(world, agent)
        .is_some_and(|b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Market))
}

/// GetHigh's gate (D39): an adult with a Body, not high, with a dose in
/// hand or a source and the coins for one.
pub fn wants_high(world: &World, agent: EntityId) -> bool {
    if !on(world) || !world.has::<Body>(agent) || is_high(world, agent) {
        return false;
    }
    if !crate::systems::demography::is_adult(world, agent) || world.has::<Sentence>(agent) {
        return false;
    }
    if world.comp::<Inventory>(agent).is_some_and(|i| i.stims > 0) {
        return true;
    }
    let coins = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
    stim_source(world, agent).and_then(|b| source_price(world, b)).is_some_and(|p| coins >= p)
}

/// `BuyStims` at `source` (D38/D40): `min(2, afford, capacity room, stock)`
/// doses. From a dealer: `dose_price` each to the gang (`Flow::Stims`), the
/// dealer's cut back to it, one `Dealing` raised against the dealer per
/// sale. From a legal Market: `legal_price` each to its owner
/// (`Flow::StimsLegal`, taxed), no crime. Returns the doses bought.
pub fn buy_stims(world: &mut World, buyer: EntityId, source: EntityId) -> u32 {
    let coins = world.comp::<Wallet>(buyer).map_or(0, |w| w.coins).max(0);
    let room =
        world.comp::<Inventory>(buyer).map_or(0, |i| assets::capacity(world, buyer).saturating_sub(assets::load(i)));
    if let Some(dealer) = bar_dealer(world, source).filter(|&d| d != buyer) {
        let Some(gang) = world.gang_of(dealer) else { return 0 };
        let price = dose_price(world, gang);
        let have = world.comp::<Inventory>(dealer).map_or(0, |i| u32::from(i.stims));
        let afford = u32::try_from(coins / price).unwrap_or(u32::MAX);
        let units = 2u32.min(afford).min(room).min(have);
        if units == 0 {
            return 0;
        }
        if let Some(i) = world.comp_mut::<Inventory>(dealer) {
            i.stims -= units as u16;
        }
        if let Some(i) = world.comp_mut::<Inventory>(buyer) {
            i.stims = i.stims.saturating_add(units as u16);
        }
        let n = i64::from(units);
        ownership::pay(world, Some(buyer), Some(gang), n * price, Flow::Stims);
        ownership::pay(world, Some(gang), Some(dealer), n * world.config.stims.dealer_cut, Flow::Stims);
        world.stats.current.stims_dealt += units;
        let tile = world.comp::<Position>(dealer).map_or_else(Default::default, |p| p.tile);
        crate::systems::law::raise_crime(world, dealer, None, Crime::Dealing, tile);
        return units;
    }
    if !legal_market(world, source) {
        return 0;
    }
    let price = legal_price(world, source);
    let afford = u32::try_from(coins / price).unwrap_or(u32::MAX);
    let units = 2u32.min(afford).min(room).min(world.stock(source, Good::Stims));
    if units == 0 {
        return 0;
    }
    world.take_stock(source, Good::Stims, units);
    if let Some(i) = world.comp_mut::<Inventory>(buyer) {
        i.stims = i.stims.saturating_add(units as u16);
    }
    let owner = world.owner_of(source);
    let paid = ownership::pay(world, Some(buyer), owner, i64::from(units) * price, Flow::StimsLegal);
    ownership::credit(world, source, paid);
    if let Some(m) = world.comp_mut::<Market>(source) {
        m.stim_sales_today += units;
    }
    units
}

/// D38: an arrest confiscates the suspect's doses (destroyed); legal
/// stims are kept.
pub fn confiscate(world: &mut World, suspect: EntityId) {
    if !on(world) || world.levers.stims_legal {
        return;
    }
    if let Some(i) = world.comp_mut::<Inventory>(suspect) {
        i.stims = 0;
    }
}

/// D38: a won raid moves the loser building's Stims and Parts into the
/// winner gang's Hideout (up to its cap; the rest is lost).
pub fn loot_goods(world: &mut World, from: EntityId, to_gang: EntityId) {
    if !on(world) {
        return;
    }
    let Some(to) = world.hideout_of(to_gang) else { return };
    if to == from {
        return;
    }
    for g in [Good::Stims, Good::Parts] {
        let n = world.stock(from, g);
        if n > 0 {
            world.take_stock(from, g, n);
            world.add_stock(to, g, n);
        }
    }
}

// ---------------------------------------------------------------------------
// Detox (D39)
// ---------------------------------------------------------------------------

/// Detox's price at `clinic`: `round(detox_price × level)` (as Therapy's).
pub fn detox_price(world: &World, clinic: EntityId) -> i64 {
    (world.config.stims.detox_price as f32 * assets::seller_level(world, clinic)).round() as i64
}

/// The addiction term of Treat (`addiction − hooked + 0.2`) is above 0.
pub fn wants_detox(world: &World, agent: EntityId) -> bool {
    on(world) && world.comp::<Body>(agent).is_some_and(|b| b.addiction - world.config.stims.hooked + 0.2 > 0.0)
}

/// D39: the addiction term drives the Treat score (above the sanity term).
pub fn detox_drives(world: &World, agent: EntityId) -> bool {
    if !wants_detox(world, agent) {
        return false;
    }
    let Some(b) = world.comp::<Body>(agent) else { return false };
    let addiction = b.addiction - world.config.stims.hooked + 0.2;
    addiction > world.config.chrome.edgy - b.sanity
}

/// `Detox` at `clinic`: the price in full (`Flow::Treatment`), addiction ×
/// `detox_mult`, the `Treated` event.
pub fn detox(world: &mut World, agent: EntityId, clinic: EntityId) -> bool {
    let price = detox_price(world, clinic);
    if world.purse(Some(agent)) < price {
        return false;
    }
    let to = world.owner_of(clinic);
    let paid = ownership::pay(world, Some(agent), to, price, Flow::Treatment);
    ownership::credit(world, clinic, paid);
    let mult = world.config.stims.detox_mult;
    if let Some(b) = world.comp_mut::<Body>(agent) {
        b.addiction = (b.addiction * mult).clamp(0.0, 1.0);
    }
    world.stats.current.detoxes += 1;
    let (who, at) = (world.name_of(agent), world.name_of(clinic));
    world.push_event(EventKind::Treated, &[agent, clinic], format!("{who} took Detox at {at}"));
    true
}

// ---------------------------------------------------------------------------
// The daily passes (D39, D40)
// ---------------------------------------------------------------------------

/// D39, daily from `assets::run`: a body clean for a day loses
/// `addiction_decay`; then the Statistical addict pass.
pub fn addiction_daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let decay = world.config.stims.addiction_decay;
    let now = world.tick;
    // scan-ok: daily: the addiction decay
    for id in world.with::<Body>() {
        let Some(b) = world.comp_mut::<Body>(id) else { continue };
        if b.addiction <= 0.0 || b.last_use.is_some_and(|t| now.saturating_sub(t) < TICKS_PER_DAY) {
            continue;
        }
        b.addiction = (b.addiction - decay).max(0.0);
    }
    stat_addicts(world);
}

/// D39: hooked Statistical adults (ascending) buy `uses_per_day` doses each
/// from yesterday's dealers (the dealer Bar nearest the Home, its gang's
/// Hideout stock, paid as a dealer sale with the cut to that dealer), else
/// a legal Market, and use them; short of both, they go without (in
/// withdrawal). One with `2 × detox_price` and lawfulness ≥ 0.5 detoxes at
/// the nearest Clinic with `p_stat_detox`.
fn stat_addicts(world: &mut World) {
    let cfg = world.config.stims.clone();
    let today = world.day();
    let due: Vec<EntityId> = world
        .tier(Lod::Statistical)
        .iter()
        .copied()
        .filter(|&id| is_hooked(world, id) && !world.has::<Sentence>(id))
        .filter(|&id| crate::systems::demography::is_adult(world, id))
        .collect();
    if due.is_empty() {
        return;
    }
    // Yesterday's (and today's) dealers: `(bar door, bar, dealer, gang)`.
    let dealers: Vec<(crate::components::TilePos, EntityId, EntityId, EntityId)> = world
        .deal_log
        .iter()
        .filter(|(_, &(_, day))| day + 1 >= today)
        .filter_map(|(&bar, &(dealer, _))| {
            let gang = world.gang_of(dealer)?;
            let door = world.comp::<Building>(bar)?.door;
            Some((door, bar, dealer, gang))
        })
        .collect();
    for id in due {
        let from = world
            .comp::<crate::components::Household>(id)
            .and_then(|h| h.home)
            .and_then(|h| world.comp::<Building>(h))
            .map(|b| b.door)
            .or_else(|| world.comp::<Position>(id).map(|p| p.tile));
        let Some(from) = from else { continue };
        for _ in 0..cfg.uses_per_day {
            if !stat_buy(world, id, from, &dealers) {
                break;
            }
            if !dose(world, id) {
                break;
            }
        }
        if !crate::systems::law::living(world, id) {
            continue;
        }
        let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
        let lawful = world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.5);
        if coins >= 2 * cfg.detox_price && lawful {
            let p = f64::from(cfg.p_stat_detox.clamp(0.0, 1.0));
            if world.rng.world().random_bool(p) {
                if let Some(c) =
                    crate::systems::assets::nearest_seller(world, crate::components::BuildingKind::Clinic, from, false)
                {
                    detox(world, id, c);
                }
            }
        }
    }
}

/// One off-screen dose bought: from the dealer Bar nearest `from` whose
/// gang's Hideout holds stock, else a legal Market. False when none is
/// within means.
fn stat_buy(
    world: &mut World,
    id: EntityId,
    from: crate::components::TilePos,
    dealers: &[(crate::components::TilePos, EntityId, EntityId, EntityId)],
) -> bool {
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins).max(0);
    let pick = dealers
        .iter()
        .filter(|&&(_, _, _, g)| world.hideout_of(g).is_some_and(|h| world.stock(h, Good::Stims) > 0))
        .map(|&(door, bar, dealer, gang)| (door.manhattan(from), bar, dealer, gang))
        .min();
    if let Some((_, _, dealer, gang)) = pick {
        let price = dose_price(world, gang);
        if coins < price {
            return false;
        }
        let Some(h) = world.hideout_of(gang) else { return false };
        world.take_stock(h, Good::Stims, 1);
        ownership::pay(world, Some(id), Some(gang), price, Flow::Stims);
        if world.has::<Wallet>(dealer) && world.gang_of(dealer) == Some(gang) {
            ownership::pay(world, Some(gang), Some(dealer), world.config.stims.dealer_cut, Flow::Stims);
        }
        world.stats.current.stims_dealt += 1;
        return true;
    }
    if !world.levers.stims_legal {
        return false;
    }
    let market = world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .filter(|&b| legal_market(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b);
    let Some(m) = market else { return false };
    let price = legal_price(world, m);
    if coins < price {
        return false;
    }
    world.take_stock(m, Good::Stims, 1);
    let owner = world.owner_of(m);
    let paid = ownership::pay(world, Some(id), owner, price, Flow::StimsLegal);
    ownership::credit(world, m, paid);
    if let Some(mk) = world.comp_mut::<Market>(m) {
        mk.stim_sales_today += 1;
    }
    true
}

/// D40, in `economy::daily_restock` while `levers.stims_legal`: each open
/// Market tops its Stims up to `stim_restock_floor` from the Reserve, its
/// owner paying `cook_cost` a dose to the City (`Flow::Import`; an agent
/// or gang owner only as far as its purse goes).
pub fn restock_legal(world: &mut World) {
    if !on(world) || !world.levers.stims_legal {
        return;
    }
    let floor = world.config.stims.stim_restock_floor;
    let cost = world.config.stims.cook_cost.max(1);
    for m in world.buildings_of_kind(BuildingKind::Market).to_vec() {
        if world.comp::<Building>(m).is_none_or(|b| b.demolished) || world.is_closed(m) {
            continue;
        }
        let mut n = floor.saturating_sub(world.stock(m, Good::Stims));
        let owner = world.owner_of(m);
        if matches!(ownership::owner_kind(world, owner), OwnerKind::Agent(_) | OwnerKind::Gang(_)) {
            n = n.min(u32::try_from(world.purse(owner).max(0) / cost).unwrap_or(u32::MAX));
        }
        if n == 0 {
            continue;
        }
        ownership::charge(world, owner, None, i64::from(n) * cost, Flow::Import);
        world.add_stock(m, Good::Stims, n);
    }
}

/// Midnight: every Market's legal sales counter starts over.
pub fn roll_sales(world: &mut World) {
    for m in world.buildings_of_kind(BuildingKind::Market).to_vec() {
        if let Some(mk) = world.comp_mut::<Market>(m) {
            mk.stim_sales_today = 0;
        }
    }
}

/// D49 snapshot: living adults at `addiction ≥ hooked`.
pub fn hooked_count(world: &World) -> u32 {
    if !on(world) {
        return 0;
    }
    let hooked = world.config.stims.hooked;
    world
        .with::<Body>()
        .into_iter()
        .filter(|&id| world.has::<Brain>(id) && crate::systems::demography::is_adult(world, id))
        .filter(|&id| world.comp::<Body>(id).is_some_and(|b| b.addiction >= hooked))
        .count() as u32
}
