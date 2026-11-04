//! Jobs & lives phase 3, weapons (docs/JOBS_AND_LIVES.md § 4; Dylan's
//! answer 1: the generic owned-item store). Under `[weapons] enabled`
//! (itself under `[jobs2] enabled`): off, every entry point is a no-op, no
//! weapon exists and no state is written.
//!
//! A game abstraction, a board game's counters: a weapon is an owned item
//! (an `Asset` of `AssetKind::Weapon`) with a tier (1-3), a condition and a
//! provenance, carried on a body, kept at home, or stocked at a Gunsmith,
//! a Hideout's armoury or the Precinct. A fight is the seeded dice roll of
//! `law::fight_inner` with one more term read from the tiers. Nothing here
//! describes how anything is done: tiers, coins, rolls and reports.
//!
//! - **The store** (`AssetKind::is_item`): the M13 asset store with its
//!   indices and writers; `Asset.prov` records the last change of hands.
//!   Weapon entities alive are capped at `weapons_per_1000` per 1,000
//!   residents (R4): past the cap nothing new is made, landed or seeded.
//! - **The fight**: `Kit.weapon` is the carried tier; a city guard draws
//!   the Precinct's stock and a private guard his Office's (a stable share
//!   of the guards, the stock over the headcount). With either side armed
//!   `p_win += weapon_w × (tier_a − tier_b)` and the loser's death chance ×
//!   `(1 + kill_w × the winner's tier)`; an unarmed fight rolls exactly as
//!   before (a branch). The winner takes the loser's carried weapons (a
//!   guard seizes them for the Precinct). A raid's pairing strength and a
//!   Hunt's might add `might_w` per tier (so the Hit's quote reads it).
//! - **Gunsmiths** (`gunsmith` trade, `produce`): Parts into weapons at
//!   the owner corp's Industry tier; sold to the Treasury's arms line (the
//!   Precinct), to Security corps for their Offices, and to licence holders
//!   through the Shop goal. Gangs never buy there.
//! - **Armouries**: a gang's stock at its Hideout, filled by crates, fences
//!   and loot; issued to its members as they march (`raid::depart`) and
//!   returned at midnight.
//! - **Dealers** (the stims dealer generalised over goods): a dealer
//!   registered at its Bar also sells the armoury's surplus (beyond
//!   `armoury_per_member`) to a buyer the Shop goal sends there
//!   (`Flow::Arms`, `Crime::ArmsDealing` against the witnesses).
//! - **Smuggling**: at `crate_hour` a gang short of its armoury target pays
//!   `crate_price` abroad (its syndicate, else the World) for a crate at the
//!   Freight Yard nearest its Hideout; a Loader on shift takes `bribe`
//!   (`Flow::Bribe`); customs seizes it on `p_seize` (× `bribe_mult` when
//!   bribed) into the Precinct's stock and files `Smuggling` on the gang's
//!   leader.
//! - **Licences**: the Civic Clerk sells one for `licence_fee`
//!   (`Flow::Licence`, the Treasury) to a lawful adult who wants a weapon;
//!   guards and Security staff are licensed by their posts.
//! - **Carry sightings**: hourly, a guard who sees an unlicensed carrier
//!   files `UnlicensedArms` after a concealment contest (`security::
//!   contest_p`; the carrier's stealth tier and pack against the guard's
//!   sight, the weapon's tier against the carrier), doubled where the law
//!   cracks down on the carrier's gang; capped a day per capita (R3).
//! - **Petty theft** (Dylan's answer 1, on the existing theft path): a
//!   Market thief also lifts a carried item from a body nearby or empties a
//!   stall's till; a Home thief takes an item kept there. Stolen items are
//!   fenced at midnight through the gang fence (`fence_frac` of the value,
//!   `Flow::Fence`) into the gang's armoury. Capped a day per capita.
//!
//! Every coin moves between purses or across the city's edge (a crate);
//! nothing is minted. Hourly work is O(weapons + guards × carriers), the
//! midnight pass O(adults) once a day.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::components::{
    Asset, AssetKind, AssetLoc, Brain, Building, BuildingKind, Class, Corpse, Gang, Household, Identity, Job, Kit,
    MemoryKind, Personality, Position, ProvKind, Provenance, Sentence, ShopPick, Skills, Wallet,
};
use crate::config::TradeModel;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::assets;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

mod carry;
mod crates;
mod market;
mod trade;

pub use carry::*;
pub use crates::*;
pub use market::*;
pub use trade::*;

const PURPOSE_GUARD: u64 = 0x5745_0001;
const PURPOSE_CARRY: u64 = 0x5745_0002;
const PURPOSE_WANT: u64 = 0x5745_0003;
const PURPOSE_SIGHT: u64 = 0x5745_0004;
const PURPOSE_CRATE: u64 = 0x5745_0005;
const PURPOSE_PETTY: u64 = 0x5745_0006;
/// R6b: the gangs' gun trade day (`gang_arms_trade`).
const PURPOSE_TRADE: u64 = 0x5745_0007;

/// Phase 3 is live: `[jobs2] enabled` and `[weapons] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.weapons.enabled
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// Phase 3's state on `Jobs2State` (empty, and not saved, with the switch off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArmsState {
    /// Every weapon entity (spawns add; the midnight pass rebuilds it from
    /// the store, so a wreck or a removal drops out by the next day).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub index: BTreeSet<EntityId>,
    /// The Civic Clerk's register of licence holders.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub licences: BTreeSet<EntityId>,
    /// Adults who want a weapon of their own (the midnight pass; the Shop
    /// goal reads it).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub wants: BTreeSet<EntityId>,
    /// Gang -> the tick its last crate landed or was seized.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub last_crate: BTreeMap<EntityId, Tick>,
    /// The guards' armed shares and tiers: the Precinct's (city guards),
    /// and each Security Office's (its private guards). Daily.
    #[serde(skip_serializing_if = "is_zero_share")]
    pub law: (f32, u8),
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub offices: BTreeMap<EntityId, (f32, u8)>,
    /// Round 6 V3 (`OPEN-238`): a stall owner -> the tick its till was last
    /// emptied (pruned at midnight past `stall_rob_gap_hours`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub till_robbed: BTreeMap<EntityId, Tick>,
    /// Today's capped counts (reset at midnight).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub petty_today: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub ua_today: u32,
    /// The pressure pass phase 2 (OPEN-176): a gang short of its armoury's
    /// target sends a member (its quartermaster) to another gang's dealer;
    /// member -> his gang, booked at midnight, cleared by the buy.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub quarter: BTreeMap<EntityId, EntityId>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn is_zero_share(v: &(f32, u8)) -> bool {
    v.1 == 0 && v.0 == 0.0
}

impl ArmsState {
    pub fn is_default(&self) -> bool {
        *self == ArmsState::default()
    }
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/// R4: weapon entities alive at most (`weapons_per_1000` per 1,000 residents).
pub fn cap(world: &World) -> usize {
    let n = f64::from(world.config.world.population) * f64::from(world.config.weapons.weapons_per_1000) / 1000.0;
    n.round().max(0.0) as usize
}

/// Weapon entities alive (the index, live entries only).
pub fn alive(world: &World) -> usize {
    world.jobs2.arms.index.iter().filter(|&&a| is_weapon(world, a)).count()
}

fn is_weapon(world: &World, a: EntityId) -> bool {
    world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Weapon)
}

/// A new weapon at `tier` (its list value), unless the store is at its
/// cap (`force`: a god's flood ignores it). Indexed, its provenance set.
pub fn spawn_weapon(
    world: &mut World,
    tier: u8,
    owner: Option<EntityId>,
    loc: AssetLoc,
    prov: Provenance,
    force: bool,
) -> Option<EntityId> {
    if !force && alive(world) >= cap(world) {
        return None;
    }
    let tier = tier.clamp(1, 3);
    let list = assets::list_price(world, AssetKind::Weapon, tier).unwrap_or(0);
    let a = assets::spawn_asset(world, AssetKind::Weapon, tier, owner, loc, list);
    // The combat model C1 (§ 4): its kind, by its tier and its holder's class.
    let class = crate::systems::combat::class_of(world, owner, loc);
    let wkind = crate::systems::combat::map_kind(world, tier, class, key(a));
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.prov = Some(prov);
        x.wkind = wkind;
    }
    if let AssetLoc::Carried(h) = loc {
        assets::rekit(world, h);
    }
    world.jobs2.arms.index.insert(a);
    Some(a)
}

fn prov(world: &World, how: ProvKind, by: Option<EntityId>) -> Provenance {
    Provenance { how, by, at: world.tick }
}

/// Move a weapon to a new holder and owner (the title reset, the
/// provenance written; `stolen` as given).
fn hand_over(world: &mut World, a: EntityId, owner: Option<EntityId>, loc: AssetLoc, p: Provenance, stolen: bool) {
    assets::reset_title(world, a);
    assets::set_owner(world, a, owner);
    assets::set_loc(world, a, loc);
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.prov = Some(p);
        x.stolen = stolen;
    }
}

/// The weapons an agent carries, ascending.
fn carried_by(world: &World, agent: EntityId) -> Vec<EntityId> {
    assets::assets_at(world, agent)
        .iter()
        .copied()
        .filter(|&a| {
            world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Weapon && x.loc == AssetLoc::Carried(agent))
        })
        .collect()
}

/// The weapons in a building's stock, ascending.
pub fn stock_at(world: &World, b: EntityId) -> Vec<EntityId> {
    assets::assets_at(world, b)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Weapon && x.loc == AssetLoc::Stock(b)))
        .collect()
}

/// The items kept at a Home (an owner's, parked there: weapons and, phase
/// 4, clothing), ascending.
fn kept_at(world: &World, home: EntityId) -> Vec<EntityId> {
    assets::assets_at(world, home)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind.is_item() && x.loc == AssetLoc::Parked(home)))
        .collect()
}

/// The best of `list` (the highest effective tier, then condition, then
/// the lower id).
fn best_of(world: &World, list: &[EntityId]) -> Option<EntityId> {
    list.iter()
        .copied()
        .filter_map(|a| world.comp::<Asset>(a).map(|x| (assets::eff_tier_of(world, x), x.condition, a)))
        .max_by(|p, q| p.0.cmp(&q.0).then(p.1.cmp(&q.1)).then(q.2.cmp(&p.2)))
        .map(|(_, _, a)| a)
}

/// The Precinct (the law's building, its armoury).
fn precinct(world: &World) -> Option<EntityId> {
    world.building_of_kind(BuildingKind::Jail)
}

// ---------------------------------------------------------------------------
// The fight (§ 4)
// ---------------------------------------------------------------------------

/// The tier `id` fights at: its carried weapon's, or a guard's draw from
/// his armoury (a stable share of the guards: the stock over the
/// headcount); 0 with the switch off.
pub fn tier(world: &World, id: EntityId) -> u8 {
    if !on(world) {
        return 0;
    }
    let own = world.comp::<Kit>(id).map_or(0, |k| k.weapon);
    let drawn = guard_draw(world, id);
    own.max(drawn)
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Hourly (from `living::run`): the crates at `crate_hour`, the carry
/// sightings. O(gangs + weapons + guards × carriers).
pub fn hourly(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let _t = crate::prof::scope!("weapons::hourly");
    if world.tick_of_day() == world.config.weapons.crate_hour {
        crates(world);
    }
    sight_carriers(world);
}

/// Midnight (from `living::run`, after the street's): the index rebuilt,
/// issued weapons back to the armouries, the fence, the owners' carry
/// roll, the wants and the licences, the guards' restock and shares.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("weapons::daily");
    world.jobs2.arms.petty_today = 0;
    let gap = u64::from(world.config.weapons.stall_rob_gap_hours) * crate::time::TICKS_PER_HOUR;
    let now = world.tick;
    world.jobs2.arms.till_robbed.retain(|_, t| now < *t + gap);
    world.jobs2.arms.ua_today = 0;
    let index: BTreeSet<EntityId> = world.jobs2.arms.index.iter().copied().filter(|&a| is_weapon(world, a)).collect();
    world.jobs2.arms.index = index;
    let gone: Vec<EntityId> = world
        .jobs2
        .arms
        .licences
        .iter()
        .copied()
        .filter(|&l| !world.has::<Brain>(l) || world.has::<Corpse>(l))
        .collect();
    for g in gone {
        world.jobs2.arms.licences.remove(&g);
    }
    let dead_gangs: Vec<EntityId> =
        world.jobs2.arms.last_crate.keys().copied().filter(|&g| !world.has::<Gang>(g)).collect();
    for g in dead_gangs {
        world.jobs2.arms.last_crate.remove(&g);
    }
    return_issued(world);
    fence_daily(world);
    carry_roll(world);
    wants_and_licences(world);
    quartermasters(world);
    gang_arms_trade(world);
    restock_guards(world);
    guard_shares(world);
}
