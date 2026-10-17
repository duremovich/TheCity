//! Jobs & lives phase 5, braindance (docs/JOBS_AND_LIVES.md § 5.3). Under
//! `[vice] bd` (itself under `[vice] enabled`): off, every entry point is a
//! no-op, no BdStudio stands, no Recorder exists and no state is written.
//!
//! A goods chain in numbers only: units made, shelved, sold and counted.
//!
//! - **Legal units.** A BdStudio's produce-model staff (the `bd` rows) make
//!   `bd_rate × (0.5 + the row's skill)` `Good::Braindance` units an hour
//!   on shift (`trades::accrue`). At midnight an Arcade below `arcade_bd`
//!   buys units from the nearest studio above `bd_floor` at `bd_wholesale`
//!   (`Flow::Wholesale`); an Arcade visit uses one off its shelf (the legal
//!   sale to residents rides the visit's price). The units a studio still
//!   holds above its floor sell to the World's Braindance book (capped per
//!   capita, `world_market::sell`, printed: R2).
//! - **The Recorder.** A gang of `recorder_members` or more keeps one
//!   `AssetKind::Recorder`, bought abroad (`Flow::Import`, its list price)
//!   and carried by its member of lowest fighting (ties the lower id);
//!   alive at most `recorders_per_1000` per 1,000 residents (R4). A fight
//!   won by a gang member within `crime.sight` of its gang's carrier is a
//!   recorded deed.
//! - **Black units.** At midnight a gang's recorded deeds become black
//!   units in its stock. A Bar drinker with a dealer of that gang present
//!   (the phase-3 dealer: registered at the Bar, else the Statistical
//!   evening's last) buys one on `p_black_buy` at `bd_wholesale ×
//!   black_bd_price_mult` (`Flow::Gray` to the gang, the stims dealer's cut
//!   back to the dealer) and `Dealing` is raised against the dealer.
//!
//! Every coin moves between purses or across the city's edge (the World's
//! book, a Recorder bought abroad). Hourly work is the accrual on shift;
//! the midnight pass is O(Arcades × studios + gangs).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::components::{
    Asset, AssetKind, AssetLoc, Building, BuildingKind, Gang, Good, Position, Sentence, Skills, Wallet,
};
use crate::config::TradeSkill;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::ExportGood;
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_HOUR;
use crate::world::World;

const PURPOSE_BLACK: u64 = 0x4244_0001;

/// Braindance is live: `[vice] enabled` and `[vice] bd`.
pub fn on(world: &World) -> bool {
    crate::systems::vice::on(world) && world.config.vice.bd
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// Phase 5's braindance state on `Jobs2State` (empty, and not saved, off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BdState {
    /// Gang -> black units in its stock.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub black: BTreeMap<EntityId, u32>,
    /// Gang -> recorded deeds not yet made into units.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub recorded: BTreeMap<EntityId, u32>,
    /// The Recorders alive (rebuilt at midnight; a lookup index).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub recorders: BTreeSet<EntityId>,
    /// Phase-8 review: agent -> the day of its black-unit roll (one roll an
    /// agent-day on both tiers; a Full drinker rolled at every drink).
    /// Kept to today at midnight.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub rolled: BTreeMap<EntityId, u64>,
}

impl BdState {
    pub fn is_default(&self) -> bool {
        *self == BdState::default()
    }
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

// ---------------------------------------------------------------------------
// Legal units
// ---------------------------------------------------------------------------

/// `trades::accrue`'s BdStudio arm: a `bd` row's `ticks` on shift make
/// `bd_rate × (0.5 + the row's skill)` units an hour, as far as the
/// studio's Braindance stock has room.
pub fn accrue_studio(world: &mut World, worker: EntityId, b: EntityId, ticks: u64) {
    if !on(world) {
        return;
    }
    let skill = crate::systems::trades::job_row(world, worker).map_or(TradeSkill::None, |r| r.skill);
    let s = world.comp::<Skills>(worker).map_or(0.0, |s| match skill {
        TradeSkill::Persuasion => s.persuasion,
        TradeSkill::Knowledge => s.knowledge,
        TradeSkill::Fighting => s.fighting,
        TradeSkill::Farming => s.farming,
        TradeSkill::Hacking => s.hacking,
        TradeSkill::None => 0.0,
    });
    let units = world.config.vice.bd_rate * (0.5 + s) * ticks as f32 / TICKS_PER_HOUR as f32;
    let whole = {
        let Some(bd) = world.comp_mut::<Building>(b) else { return };
        bd.production_accum += units;
        let whole = bd.production_accum.floor().max(0.0);
        bd.production_accum -= whole;
        whole as u32
    };
    if whole == 0 {
        return;
    }
    let room = world.goods_cap(b, Good::Braindance).saturating_sub(world.stock(b, Good::Braindance));
    let added = world.add_stock(b, Good::Braindance, whole.min(room));
    ownership::jobs2_row(world).bd_made += added;
}

/// An Arcade visit (`vice::on_visit`): one unit off its shelf, if any.
pub fn on_arcade_visit(world: &mut World, b: EntityId) {
    if !on(world) || world.stock(b, Good::Braindance) == 0 {
        return;
    }
    world.take_stock(b, Good::Braindance, 1);
    ownership::jobs2_row(world).bd_viewed += 1;
}

/// The open studios, ascending.
fn studios(world: &World) -> Vec<EntityId> {
    world.buildings_of_kind(BuildingKind::BdStudio).iter().copied().filter(|&b| standing(world, b)).collect()
}

/// Midnight: each standing Arcade below `arcade_bd` (ascending) buys from
/// the nearest studio above its floor (door Manhattan, ties the lower id)
/// at `bd_wholesale` a unit, what its owner's purse covers.
fn wholesale(world: &mut World, studios: &[EntityId]) {
    let cfg = world.config.vice.clone();
    let price = cfg.bd_wholesale.max(1);
    for a in world.buildings_of_kind(BuildingKind::Arcade).to_vec() {
        if !standing(world, a) {
            continue;
        }
        let want = cfg.arcade_bd.saturating_sub(world.stock(a, Good::Braindance));
        if want == 0 {
            continue;
        }
        let Some(door) = world.comp::<Building>(a).map(|bd| bd.door) else { continue };
        let Some(s) = studios
            .iter()
            .copied()
            .filter(|&s| world.stock(s, Good::Braindance) > cfg.bd_floor)
            .filter_map(|s| world.comp::<Building>(s).map(|bd| (bd.door.manhattan(door), s)))
            .min()
            .map(|(_, s)| s)
        else {
            continue;
        };
        let buyer = world.owner_of(a);
        let afford = u32::try_from(world.purse(buyer).max(0) / price).unwrap_or(u32::MAX);
        let n = want.min(world.stock(s, Good::Braindance) - cfg.bd_floor).min(afford);
        if n == 0 {
            continue;
        }
        let seller = world.owner_of(s);
        let paid = ownership::pay(world, buyer, seller, i64::from(n) * price, Flow::Wholesale);
        ownership::credit(world, s, paid);
        world.take_stock(s, Good::Braindance, n);
        world.add_stock(a, Good::Braindance, n);
        ownership::jobs2_row(world).bd_wholesale += n;
    }
}

/// Midnight: each studio's units above its floor to the World's book
/// (capped per capita; nothing sells with the World closed).
fn export(world: &mut World, studios: &[EntityId]) {
    if !crate::systems::world_market::open(world) {
        return;
    }
    let floor = world.config.vice.bd_floor;
    for &s in studios {
        let have = world.stock(s, Good::Braindance).saturating_sub(floor);
        if have == 0 {
            continue;
        }
        let owner = world.owner_of(s);
        let (n, paid) = crate::systems::world_market::sell(world, Some(s), owner, ExportGood::Braindance, have);
        world.take_stock(s, Good::Braindance, n);
        let row = ownership::jobs2_row(world);
        row.bd_exported += n;
        row.bd_export_paid += paid;
    }
}

// ---------------------------------------------------------------------------
// The Recorder and black units
// ---------------------------------------------------------------------------

fn recorder_cap(world: &World) -> usize {
    // scan-ok: daily (the Recorder cap at midnight), the head count only
    let pop = world.citizens().len() as f32;
    (world.config.vice.recorders_per_1000 * pop / 1000.0).floor() as usize
}

/// The member of `gang` carrying a Recorder (the lowest id).
pub fn carrier_of(world: &World, gang: EntityId) -> Option<EntityId> {
    world
        .jobs2
        .bd
        .recorders
        .iter()
        .filter_map(|&a| match world.comp::<Asset>(a).map(|x| x.loc) {
            Some(AssetLoc::Carried(m)) => Some(m),
            _ => None,
        })
        .filter(|&m| world.gang_of(m) == Some(gang) && crate::systems::law::living(world, m))
        .min()
}

/// A fight won (`law::resolve_fight`'s end): a gang winner within
/// `crime.sight` of its gang's carrier is a recorded deed.
pub fn after_fight(world: &mut World, winner: EntityId) {
    if !on(world) || world.jobs2.bd.recorders.is_empty() {
        return;
    }
    let Some(g) = world.gang_of(winner) else { return };
    let Some(c) = carrier_of(world, g) else { return };
    let (Some(a), Some(b)) = (world.comp::<Position>(winner), world.comp::<Position>(c)) else { return };
    if crate::systems::law::chebyshev(a.tile, b.tile) > world.config.crime.sight {
        return;
    }
    *world.jobs2.bd.recorded.entry(g).or_default() += 1;
    ownership::jobs2_row(world).recorded += 1;
}

/// Midnight: the index rebuilt; a gang of `recorder_members` with no
/// carrier buys a Recorder abroad (under the cap) for its member of lowest
/// fighting (free, ties the lower id); the recorded deeds become black units.
fn recorders_daily(world: &mut World) {
    // Phase-8 review: the asset store's index and the Recorder index, not a
    // scan of every entity (ascending, as the scan was).
    let mut all: Vec<EntityId> = crate::systems::assets::all_assets(world);
    all.extend(world.jobs2.bd.recorders.iter().copied());
    all.sort_unstable();
    all.dedup();
    all.retain(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Recorder));
    // A Recorder on a body no longer living (or set down) is gone with it.
    let mut index: BTreeSet<EntityId> = BTreeSet::new();
    for a in all {
        let carried = match world.comp::<Asset>(a).map(|x| x.loc) {
            Some(AssetLoc::Carried(m)) => crate::systems::law::living(world, m),
            _ => false,
        };
        if carried {
            index.insert(a);
        } else {
            crate::systems::assets::despawn(world, a);
        }
    }
    world.jobs2.bd.recorders = index;
    let cap = recorder_cap(world);
    let price = crate::systems::assets::list_price(world, AssetKind::Recorder, 1).unwrap_or(0).max(1);
    for g in world.gangs() {
        let Some(members) = world.comp::<Gang>(g).map(|x| x.members.clone()) else { continue };
        if members.len() < world.config.vice.recorder_members
            || carrier_of(world, g).is_some()
            || world.jobs2.bd.recorders.len() >= cap
            || world.purse(Some(g)) < price
        {
            continue;
        }
        let Some(m) = members
            .iter()
            .copied()
            .filter(|&m| crate::systems::law::living(world, m) && !world.has::<Sentence>(m))
            .map(|m| (world.comp::<Skills>(m).map_or(0.0, |s| s.fighting), m))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, m)| m)
        else {
            continue;
        };
        let paid = ownership::cross_out(world, Some(g), crate::outside::WORLD_ACCOUNT, price, Flow::Import, false);
        if paid < price {
            continue;
        }
        let a =
            crate::systems::assets::spawn_asset(world, AssetKind::Recorder, 1, Some(m), AssetLoc::Carried(m), price);
        world.jobs2.bd.recorders.insert(a);
        let text = format!("{} carries a Recorder for {}", world.name_of(m), world.owner_label(Some(g)));
        world.push_event(EventKind::AssetBought, &[m, a], text);
    }
    let made: Vec<(EntityId, u32)> = std::mem::take(&mut world.jobs2.bd.recorded).into_iter().collect();
    for (g, n) in made {
        if !world.has::<Gang>(g) {
            continue;
        }
        *world.jobs2.bd.black.entry(g).or_default() += n;
        ownership::jobs2_row(world).black_made += n;
    }
    // A gang gone takes its stock with it.
    let gone: Vec<EntityId> = world.jobs2.bd.black.keys().copied().filter(|&g| !world.has::<Gang>(g)).collect();
    for g in gone {
        world.jobs2.bd.black.remove(&g);
    }
}

/// The dealer at `bar` whose gang holds black units: a registered dealer
/// alive and inside (`present`), else (the Statistical evening) the bar's
/// last dealer, alive and free.
fn black_dealer(world: &World, bar: EntityId, present: bool) -> Option<(EntityId, EntityId)> {
    let has = |g: EntityId| world.jobs2.bd.black.get(&g).is_some_and(|&n| n > 0);
    let here = world.dealers.get(&bar).and_then(|list| {
        list.iter().copied().find_map(|d| {
            let g = world.gang_of(d)?;
            (crate::systems::law::living(world, d)
                && world.comp::<Position>(d).is_some_and(|p| p.building == Some(bar))
                && has(g))
            .then_some((d, g))
        })
    });
    if here.is_some() || present {
        return here;
    }
    let &(d, _) = world.deal_log.get(&bar)?;
    let g = world.gang_of(d)?;
    (crate::systems::law::living(world, d) && !world.has::<Sentence>(d) && has(g)).then_some((d, g))
}

/// A drink done at `bar` (a body there, or the Statistical evening's): with
/// a dealer of a gang holding black units, on `p_black_buy` the drinker
/// buys one if it can spare the price; `Dealing` against the dealer.
pub fn bar_visit(world: &mut World, id: EntityId, bar: EntityId, present: bool) {
    if !on(world) || world.jobs2.bd.black.is_empty() {
        return;
    }
    let Some((dealer, gang)) = black_dealer(world, bar, present) else { return };
    if dealer == id || world.gang_of(id) == Some(gang) {
        return;
    }
    // One roll an agent-day (the hash is keyed on the day; a Full drinker's
    // second drink read the same draw and bought again).
    let day = world.day();
    if world.jobs2.bd.rolled.insert(id, day) == Some(day) {
        return;
    }
    if hash(world, PURPOSE_BLACK, key(id), day) >= world.config.vice.p_black_buy {
        return;
    }
    let cfg = &world.config.vice;
    let price = ((cfg.bd_wholesale as f32 * cfg.black_bd_price_mult).round() as i64).max(1);
    if crate::systems::leisure::spendable(world, id) < price || world.comp::<Wallet>(id).is_none_or(|w| w.coins < price)
    {
        return;
    }
    let paid = ownership::pay(world, Some(id), Some(gang), price, Flow::Gray);
    let cut = world.config.stims.dealer_cut.min(paid);
    if cut > 0 {
        ownership::pay(world, Some(gang), Some(dealer), cut, Flow::Gray);
    }
    if let Some(n) = world.jobs2.bd.black.get_mut(&gang) {
        *n = n.saturating_sub(1);
    }
    let row = ownership::jobs2_row(world);
    row.black_sold += 1;
    row.black_revenue += paid;
    let tile = world.comp::<Position>(dealer).map_or_else(Default::default, |p| p.tile);
    crate::systems::law::raise_crime_except(
        world,
        dealer,
        None,
        None,
        crate::components::Crime::Dealing,
        tile,
        Some(id),
    );
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Midnight (from `living::run`): the Arcades' wholesale, the World's
/// book, the Recorders and the black units.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("braindance::daily");
    let today = world.day();
    world.jobs2.bd.rolled.retain(|_, d| *d == today);
    let s = studios(world);
    wholesale(world, &s);
    export(world, &s);
    recorders_daily(world);
}

/// `World::new`, after the vice's venues: `bd_studios` BdStudios per copy
/// of each `[vice] corp_rows` row on the Lots left. No RNG, no coins.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let n = world.config.vice.bd_studios;
    let rows = world.config.vice.corp_rows.clone();
    for j in 0..world.seed_copies() {
        for &r in &rows {
            let Some(t) = world.config.corps.row_template(r) else { continue };
            let Some(&c) = world.corps_of_template(t).get(j) else { continue };
            for _ in 0..n {
                crate::systems::vice::seed_venue(world, BuildingKind::BdStudio, Some(c));
            }
        }
    }
}

/// The day's end (`systems::stats`): the braindance snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let n = studios(world).len() as u32;
    let recorders = world.jobs2.bd.recorders.len() as u32;
    let row = ownership::jobs2_row(world);
    row.bd_studios = n;
    row.recorders = recorders;
}
