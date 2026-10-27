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
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.prov = Some(prov);
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

fn guard_draw(world: &World, id: EntityId) -> u8 {
    let Some(job) = world.comp::<Job>(id).filter(|j| j.role == crate::components::Role::Guard) else { return 0 };
    let (share, t) = if crate::systems::law::job_is_private_guard(world, job) {
        match job.employer.and_then(|e| world.jobs2.arms.offices.get(&e)) {
            Some(&x) => x,
            None => return 0,
        }
    } else {
        world.jobs2.arms.law
    };
    if t == 0 || share <= 0.0 {
        return 0;
    }
    if hash(world, PURPOSE_GUARD, key(id), 0) < share {
        t
    } else {
        0
    }
}

/// The fight's tiers `(a, b)` when either side is armed (`None`: the
/// unarmed roll, exactly as before).
pub fn fight_tiers(world: &World, a: EntityId, b: EntityId) -> Option<(u8, u8)> {
    if !on(world) {
        return None;
    }
    let (ta, tb) = (tier(world, a), tier(world, b));
    (ta > 0 || tb > 0).then_some((ta, tb))
}

/// `weapon_w × (tier_a − tier_b)`: the term `p_win` gains.
pub fn p_win_term(cfg: &crate::config::WeaponsCfg, ta: u8, tb: u8) -> f32 {
    cfg.weapon_w * (f32::from(ta) - f32::from(tb))
}

/// `1 + kill_w × tier`: the loser's death chance multiplier.
pub fn kill_mult(cfg: &crate::config::WeaponsCfg, winner_tier: u8) -> f32 {
    1.0 + cfg.kill_w * f32::from(winner_tier)
}

/// A raid's pairing strength and a Hunt's might per tier (0 unarmed).
pub fn might(world: &World, id: EntityId) -> f32 {
    if !on(world) {
        return 0.0;
    }
    let t = tier(world, id);
    if t == 0 {
        0.0
    } else {
        world.config.weapons.might_w * f32::from(t)
    }
}

/// After a fight (switch on): the counts, the winner's weapon's wear, and
/// the loser's carried weapons to the winner (a guard seizes them for the
/// Precinct; a robot takes nothing).
pub fn after_fight(world: &mut World, winner: EntityId, loser: EntityId, died: bool, tiers: Option<(u8, u8)>) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("weapons::after_fight");
    let row = ownership::jobs2_row(world);
    row.fights += 1;
    if died {
        row.fight_deaths += 1;
    }
    if tiers.is_some() {
        row.armed_fights += 1;
        if died {
            row.armed_deaths += 1;
        }
    }
    if tiers.is_none() {
        return;
    }
    // The winner's weapon wears.
    let wear = world.config.weapons.wear_per_fight;
    if wear > 0 {
        if let Some(w) = best_of(world, &carried_by(world, winner)) {
            let c = world.comp::<Asset>(w).map_or(0, |x| x.condition.saturating_sub(wear));
            if let Some(x) = world.comp_mut::<Asset>(w) {
                x.condition = c;
            }
            if c == 0 {
                assets::wreck(world, w, "worn out in a fight");
                world.jobs2.arms.index.remove(&w);
            }
        }
    }
    let taken = carried_by(world, loser);
    if taken.is_empty() || !world.has::<Brain>(winner) {
        return;
    }
    if crate::systems::law::is_guard(world, winner) {
        seize(world, loser, &taken);
        return;
    }
    for a in taken {
        let p = prov(world, ProvKind::Stolen, Some(winner));
        hand_over(world, a, Some(winner), AssetLoc::Carried(winner), p, true);
    }
}

/// The law takes `items` from `from` into the Precinct's stock (the city's).
fn seize(world: &mut World, from: EntityId, items: &[EntityId]) {
    let Some(p) = precinct(world) else { return };
    for &a in items {
        let pv = prov(world, ProvKind::Seized, Some(p));
        hand_over(world, a, None, AssetLoc::Stock(p), pv, false);
        ownership::jobs2_row(world).seized += 1;
    }
    world.jobs2.arms.licences.remove(&from);
}

/// `law::jail_suspect`: an arrest takes every weapon the suspect carries.
pub fn confiscate(world: &mut World, suspect: EntityId) {
    if !on(world) {
        return;
    }
    let items = carried_by(world, suspect);
    if !items.is_empty() {
        seize(world, suspect, &items);
    }
}

// ---------------------------------------------------------------------------
// Carrying, licences and the sightings (§ 4)
// ---------------------------------------------------------------------------

/// Licensed by the post (guards, Security staff) or the Clerk's register.
pub fn licensed(world: &World, id: EntityId) -> bool {
    if world.jobs2.arms.licences.contains(&id) {
        return true;
    }
    world.comp::<Job>(id).is_some_and(|j| {
        j.role == crate::components::Role::Guard
            || j.employer
                .and_then(|e| world.comp::<Building>(e))
                .is_some_and(|b| matches!(b.kind, BuildingKind::SecurityOffice | BuildingKind::Gunsmith))
    })
}

/// A reason to want and carry a weapon: lawless, hunted or hunting, on a
/// contract's run, working a Pitch, robbed or beaten within `robbed_days`.
fn has_reason(world: &World, id: EntityId) -> bool {
    let cfg = &world.config.weapons;
    if world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < cfg.want_lawless) {
        return true;
    }
    if world.hunted_by.contains_key(&id) || world.hunts.contains_key(&id) || world.contract_runs.contains_key(&id) {
        return true;
    }
    if world.jobs2.pitches.contains_key(&id) {
        return true;
    }
    // The pressure pass phase 2 (OPEN-176): the frightened (low safety).
    if crate::systems::customers::frightened(world, id) {
        return true;
    }
    let since = world.tick.saturating_sub(u64::from(cfg.robbed_days) * TICKS_PER_DAY);
    world.comp::<crate::components::Memory>(id).is_some_and(|m| {
        m.entries.iter().any(|e| e.tick >= since && matches!(e.kind, MemoryKind::WasRobbed | MemoryKind::Lost))
    })
}

fn class_index(world: &World, id: EntityId) -> usize {
    match crate::systems::classes::class_of(world, id) {
        Class::Corp => 0,
        Class::Street => 1,
        Class::Dreg => 2,
    }
}

/// A free, living adult with a Body (no corpse, no sentence).
fn free_adult(world: &World, id: EntityId) -> bool {
    world.has::<Identity>(id)
        && world.has::<Brain>(id)
        && !world.has::<Corpse>(id)
        && !world.has::<Sentence>(id)
        && crate::systems::demography::is_adult(world, id)
}

/// The concealment contest: the guard's chance to notice a carrier whose
/// stealth tier and pack tier hide a weapon of `weapon_tier` from a guard
/// of `sight_tier`: `1 − contest_p(stealth + pack − (weapon − 1), sight, step)`.
pub fn notice_p(stealth: f32, pack: u8, weapon_tier: u8, sight_tier: u8, step: f32) -> f32 {
    let hide = f32::from(crate::systems::security::thief_tier(stealth)) + f32::from(pack)
        - (f32::from(weapon_tier.max(1)) - 1.0);
    1.0 - crate::systems::security::contest_p(hide, f32::from(sight_tier.max(1)), step)
}

/// A guard's sight tier: 1, plus his Eyes' tier.
fn sight_tier(world: &World, g: EntityId) -> u8 {
    let per = world.config.chrome.eyes_sight.max(1);
    let extra = world.comp::<Kit>(g).map_or(0, |k| k.sight) / per;
    1 + extra.min(2)
}

fn pack_tier(world: &World, id: EntityId) -> u8 {
    assets::assets_at(world, id)
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a))
        .filter(|x| x.kind == AssetKind::Pack && x.loc == AssetLoc::Carried(id) && x.condition > 0)
        .map(|x| x.tier)
        .max()
        .unwrap_or(0)
}

/// Hourly: every unlicensed carrier a guard sees rolls the concealment
/// contest (`crackdown_rolls` times where the law cracks down on the
/// carrier's gang); a notice files `UnlicensedArms` while under the day's cap.
pub fn sight_carriers(world: &mut World) {
    let cap = per_capita_day(world, world.config.weapons.ua_per_1000_day);
    if world.jobs2.arms.ua_today >= cap {
        return;
    }
    let mut carriers: Vec<EntityId> = world
        .jobs2
        .arms
        .index
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a))
        .filter_map(|x| match x.loc {
            AssetLoc::Carried(h) => Some(h),
            _ => None,
        })
        .collect();
    carriers.sort_unstable();
    carriers.dedup();
    if carriers.is_empty() {
        return;
    }
    let sight = world.config.crime.sight;
    let guards: Vec<(EntityId, crate::components::TilePos, Option<EntityId>, u32)> = world
        .guards()
        .iter()
        .filter_map(|&g| {
            let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(g).map(|p| (g, p.tile, p.building, extra))
        })
        .collect();
    let hour = world.tick / TICKS_PER_HOUR;
    let step = world.config.weapons.conceal_step;
    for c in carriers {
        if world.jobs2.arms.ua_today >= cap {
            break;
        }
        if !free_adult(world, c) || licensed(world, c) {
            continue;
        }
        let t = world.comp::<Kit>(c).map_or(0, |k| k.weapon);
        if t == 0 {
            continue;
        }
        let Some(pc) = world.comp::<Position>(c) else { continue };
        let (tile, building) = (pc.tile, pc.building);
        let Some(&(g, ..)) = guards.iter().find(|&&(g, gt, gb, extra)| {
            g != c && ((gb.is_some() && gb == building) || crate::systems::law::chebyshev(gt, tile) <= sight + extra)
        }) else {
            continue;
        };
        let stealth = crate::systems::law::stealth(world, c);
        let p = notice_p(stealth, pack_tier(world, c), t, sight_tier(world, g), step);
        // Jobs & lives phase 4 (§ 9): a dress-0 body in the Spire (1 off).
        let m = crate::systems::fashion::sighting_mult(world, c);
        let p = if m != 1.0 { (p * m).min(1.0) } else { p };
        let rolls = match world.gang_of(c) {
            Some(gang) if crate::systems::law::cracking_down_on(world, gang) => {
                world.config.weapons.crackdown_rolls.max(1)
            }
            _ => 1,
        };
        let seen = (0..rolls).any(|r| hash(world, PURPOSE_SIGHT, key(c), hour * 4 + u64::from(r)) < p);
        if !seen {
            continue;
        }
        world.jobs2.arms.ua_today += 1;
        ownership::jobs2_row(world).ua_reports += 1;
        crate::systems::law::file_report(world, crate::components::Crime::UnlicensedArms, c, Some(g));
    }
}

/// `per_1000 × population ÷ 1,000`, at least 1.
pub(crate) fn per_capita_day(world: &World, per_1000: f32) -> u32 {
    ((f64::from(world.config.world.population) * f64::from(per_1000) / 1000.0).round() as u32).max(1)
}

// ---------------------------------------------------------------------------
// Buying: the Shop goal's weapon offer (§ 4)
// ---------------------------------------------------------------------------

/// A weapon's price at a Gunsmith: `list × the seller's price level`.
fn legal_price(world: &World, seller: EntityId, tier: u8) -> Option<i64> {
    let list = assets::list_price(world, AssetKind::Weapon, tier)?;
    Some((list as f32 * assets::seller_level(world, seller)).round().max(1.0) as i64)
}

/// A dealer's price: `list × dealer_markup`.
fn dealer_price(world: &World, tier: u8) -> Option<i64> {
    let list = assets::list_price(world, AssetKind::Weapon, tier)?;
    Some((list as f32 * world.config.weapons.dealer_markup).round().max(1.0) as i64)
}

/// A Gunsmith sells while one of its gunsmiths is on shift (or, off
/// screen, on the books) and it has stock.
fn gunsmith_open(world: &World, b: EntityId) -> bool {
    if !crate::systems::industry::open(world, b) || world.is_closed(b) {
        return false;
    }
    let tod = world.tick_of_day();
    world.staff_of(b).iter().any(|&s| world.comp::<Job>(s).is_some_and(|j| j.on_shift(tod)))
}

/// A gang's armoury target: `armoury_per_member × members` (rounded up;
/// the muster's stock, kept back from the dealer).
fn armoury_target(world: &World, gang: EntityId) -> usize {
    let n = world.comp::<Gang>(gang).map_or(0, |g| g.members.len()) as f32;
    (n * world.config.weapons.armoury_per_member).ceil() as usize
}

/// Is there an armoury surplus for `gang`'s dealer to sell (the shadow
/// tool's arms-dealer pick)?
pub fn has_surplus(world: &World, gang: EntityId) -> bool {
    !surplus(world, gang).is_empty()
}

/// The armoury's weapons beyond its target (the dealer's to sell), the
/// cheapest first.
fn surplus(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(h) = world.hideout_of(gang) else { return Vec::new() };
    let mut stock = stock_at(world, h);
    let keep = armoury_target(world, gang);
    if stock.len() <= keep {
        return Vec::new();
    }
    // The best stay for the members; the rest are for sale, cheapest first.
    stock.sort_by_key(|&a| {
        let t = world.comp::<Asset>(a).map_or(0, |x| x.tier);
        (t, a)
    });
    let n = stock.len() - keep;
    stock.truncate(n);
    stock
}

/// The dealer selling at `bar` whose gang has a surplus, with that gang:
/// a registered dealer alive and in it (`present`, a body walking in), else
/// (off screen, the Statistical shop) the bar's last dealer, alive and free.
fn bar_arms_dealer(world: &World, bar: EntityId, present: bool) -> Option<(EntityId, EntityId)> {
    let here = world.dealers.get(&bar).and_then(|list| {
        list.iter().copied().find_map(|d| {
            let in_it = crate::systems::law::living(world, d)
                && world.comp::<Position>(d).is_some_and(|p| p.building == Some(bar));
            let gang = world.gang_of(d)?;
            (in_it && !surplus(world, gang).is_empty()).then_some((d, gang))
        })
    });
    if here.is_some() || present {
        return here;
    }
    let &(d, _) = world.deal_log.get(&bar)?;
    let gang = world.gang_of(d)?;
    (crate::systems::law::living(world, d) && !world.has::<Sentence>(d) && !surplus(world, gang).is_empty())
        .then_some((d, gang))
}

/// The Bars a dealer sells at (registered now, or logged).
fn deal_bars(world: &World, present: bool) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = world.dealers.keys().copied().collect();
    if !present {
        v.extend(world.deal_log.keys().copied());
        v.sort_unstable();
        v.dedup();
    }
    v
}

/// Does `id` want a weapon of its own (the midnight pass's set)?
pub fn wants(world: &World, id: EntityId) -> bool {
    on(world) && world.jobs2.arms.wants.contains(&id)
}

/// The Shop goal's weapon offer for `id`: `(seller, tier, price, legal)`:
/// the dearest affordable at the nearest Gunsmith with stock (open now
/// when `require_open`) for a licence holder, else the cheapest at the
/// nearest dealer's Bar. `None` without the want, the coins, or a seller.
pub fn offer(
    world: &World,
    id: EntityId,
    from: crate::components::TilePos,
    require_open: bool,
) -> Option<(EntityId, u8, i64, bool)> {
    // The pressure pass phase 2: a quartermaster buys for his gang's
    // armoury with its purse, from another gang's dealer only.
    let qm = world.jobs2.arms.quarter.get(&id).copied();
    if !wants(world, id) || (qm.is_none() && world.comp::<Kit>(id).is_some_and(|k| k.weapon > 0)) {
        return None;
    }
    let _t = crate::prof::scope!("weapons::offer");
    let coins = match qm {
        Some(g) => world.purse(Some(g)) - world.config.weapons.coin_reserve,
        None => world.comp::<Wallet>(id).map_or(0, |w| w.coins) - world.config.weapons.coin_reserve,
    };
    if coins <= 0 {
        return None;
    }
    let door = |b: EntityId| world.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(from));
    if qm.is_none() && licensed(world, id) {
        let shop = world
            .buildings_of_kind(BuildingKind::Gunsmith)
            .iter()
            .copied()
            .filter(|&b| crate::systems::industry::open(world, b) && (!require_open || gunsmith_open(world, b)))
            .filter(|&b| !stock_at(world, b).is_empty())
            .min_by_key(|&b| (door(b), b));
        if let Some(b) = shop {
            let pick = stock_at(world, b)
                .into_iter()
                .filter_map(|a| world.comp::<Asset>(a).map(|x| x.tier))
                .filter_map(|t| legal_price(world, b, t).map(|p| (t, p)))
                .filter(|&(_, p)| p <= coins)
                .max_by_key(|&(t, p)| (p, t));
            if let Some((t, p)) = pick {
                return Some((b, t, p, true));
            }
        }
    }
    let other = |b: EntityId| bar_arms_dealer(world, b, require_open).is_some_and(|(_, g)| Some(g) != qm);
    let bar = deal_bars(world, require_open).into_iter().filter(|&b| other(b)).min_by_key(|&b| (door(b), b))?;
    let (_, gang) = bar_arms_dealer(world, bar, require_open)?;
    let t = surplus(world, gang).first().and_then(|&a| world.comp::<Asset>(a)).map(|x| x.tier)?;
    let p = dealer_price(world, t)?;
    (p <= coins).then_some((bar, t, p, false))
}

/// The pre-view pass phase 4 (P3, OPEN-119): an off-screen buyer's sale
/// from a dealer who is a body, booked at midnight and made when the
/// dealer's body is at the Bar (`World::arms_due`, saved).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmsMeet {
    pub buyer: EntityId,
    pub bar: EntityId,
    pub pick: ShopPick,
    /// The meeting lapses after this tick (`[presence] arms_wait_hours`).
    pub until: Tick,
}

/// The pre-view pass phase 4 (`assets::stat_shop`, midnight): a
/// Statistical buyer's weapon from a dealer's Bar. With `[presence]` on and
/// the Bar's dealer a body, the sale is booked for the meeting (`true`);
/// else (off, a Gunsmith, a Statistical dealer: the mirror) `false`, and
/// the caller sells off screen now (`arms_midnight` counts the dealer's).
pub fn book_meeting(world: &mut World, buyer: EntityId, seller: EntityId, pick: &ShopPick) -> bool {
    let Some(cfg) = world.config.presence.as_ref() else { return false };
    if world.comp::<Building>(seller).is_none_or(|b| b.kind == BuildingKind::Gunsmith) {
        return false;
    }
    let Some((dealer, _)) = bar_arms_dealer(world, seller, false) else { return false };
    if !crate::systems::presence::is_body(world, dealer) {
        ownership::jobs2_row(world).arms_midnight += 1;
        return false;
    }
    let until = world.tick + Tick::from(cfg.arms_wait_hours) * TICKS_PER_HOUR;
    if !world.arms_due.iter().any(|m| m.buyer == buyer) {
        world.arms_due.push(ArmsMeet { buyer, bar: seller, pick: pick.clone(), until });
    }
    true
}

/// The pre-view pass phase 4 (P3, OPEN-119), hourly: each booked arms sale
/// whose dealer's body stands in the Bar now is made there (`buy` with the
/// dealer present: the coins, the hand-over, `ArmsDealing` raised at his
/// tile, the witnesses the bodies present); lapsed meetings and buyers gone
/// are dropped (`arms_missed`), the rest wait.
pub fn arms_meetings(world: &mut World) {
    if world.arms_due.is_empty() || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let now = world.tick;
    let due = std::mem::take(&mut world.arms_due);
    let mut keep = Vec::with_capacity(due.len());
    for m in due {
        let gone = !crate::systems::law::living(world, m.buyer) || world.has::<Sentence>(m.buyer);
        if gone || now > m.until {
            ownership::jobs2_row(world).arms_missed += 1;
            continue;
        }
        if bar_arms_dealer(world, m.bar, true).is_none() {
            keep.push(m);
            continue;
        }
        if buy(world, m.buyer, m.bar, &m.pick, true).is_ok() {
            ownership::jobs2_row(world).arms_meet += 1;
        } else {
            ownership::jobs2_row(world).arms_missed += 1;
        }
    }
    keep.append(&mut world.arms_due);
    world.arms_due = keep;
}

/// May `BuyAsset` run for a weapon at `b` now (an open Gunsmith with
/// stock, or a Bar with an arms dealer)?
pub fn sale_open(world: &World, b: EntityId) -> bool {
    match world.comp::<Building>(b).map(|bd| bd.kind) {
        Some(BuildingKind::Gunsmith) => gunsmith_open(world, b) && !stock_at(world, b).is_empty(),
        Some(_) => bar_arms_dealer(world, b, true).is_some(),
        None => false,
    }
}

/// `BuyAsset` for a weapon at `seller`: from a Gunsmith's stock to a
/// licence holder (`Flow::Asset`, taxed), or from a dealer's armoury
/// surplus (`Flow::Arms` to the gang, the dealer's cut, `ArmsDealing`
/// raised against the dealer with the buyer no witness). `present`: a
/// body at the counter (the Statistical shop buys off screen).
pub fn buy(
    world: &mut World,
    buyer: EntityId,
    seller: EntityId,
    pick: &ShopPick,
    present: bool,
) -> Result<EntityId, String> {
    if !on(world) {
        return Err("weapons are off".into());
    }
    let coins = world.purse(Some(buyer));
    let kind = world.comp::<Building>(seller).map(|b| b.kind).ok_or("no such seller")?;
    let now = world.tick;
    if kind == BuildingKind::Gunsmith {
        if !licensed(world, buyer) {
            return Err("no licence".into());
        }
        let a = stock_at(world, seller)
            .into_iter()
            .find(|&a| world.comp::<Asset>(a).is_some_and(|x| x.tier == pick.tier))
            .ok_or("sold out")?;
        let price = legal_price(world, seller, pick.tier).ok_or("no price")?;
        if coins < price {
            return Err("cannot afford".into());
        }
        let owner = world.owner_of(seller);
        let paid = ownership::pay(world, Some(buyer), owner, price, Flow::Asset);
        ownership::credit(world, seller, paid);
        let p = prov(world, ProvKind::Sold, Some(seller));
        hand_over(world, a, Some(buyer), AssetLoc::Carried(buyer), p, false);
        ownership::jobs2_row(world).sold_legal += 1;
        if let Some(b) = world.comp_mut::<crate::components::Body>(buyer) {
            b.last_shop = Some(now);
        }
        let text =
            format!("{} bought a Weapon T{} at {} for {price}", world.name_of(buyer), pick.tier, world.name_of(seller));
        world.push_event(EventKind::AssetBought, &[buyer, seller, a], text);
        world.jobs2.arms.wants.remove(&buyer);
        return Ok(a);
    }
    let (dealer, gang) = bar_arms_dealer(world, seller, present).ok_or("no dealer")?;
    let qm = world.jobs2.arms.quarter.get(&buyer).copied();
    if qm == Some(gang) {
        return Err("his own gang's dealer".into());
    }
    let coins = qm.map_or(coins, |g| world.purse(Some(g)));
    let a = surplus(world, gang).into_iter().next().ok_or("sold out")?;
    let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
    let price = dealer_price(world, tier).ok_or("no price")?;
    if coins < price {
        return Err("cannot afford".into());
    }
    ownership::pay(world, Some(qm.unwrap_or(buyer)), Some(gang), price, Flow::Arms);
    let cut = world.config.weapons.dealer_cut.min(price);
    ownership::pay(world, Some(gang), Some(dealer), cut, Flow::Arms);
    let p = prov(world, ProvKind::Dealt, Some(dealer));
    match qm.and_then(|g| world.hideout_of(g).map(|h| (g, h))) {
        Some((g, h)) => {
            hand_over(world, a, Some(g), AssetLoc::Stock(h), p, false);
            world.jobs2.arms.quarter.remove(&buyer);
            ownership::jobs2_row(world).gang_arms_buys += 1;
        }
        None => hand_over(world, a, Some(buyer), AssetLoc::Carried(buyer), p, false),
    }
    ownership::jobs2_row(world).sold_dealer += 1;
    if let Some(b) = world.comp_mut::<crate::components::Body>(buyer) {
        b.last_shop = Some(now);
    }
    let tile = world.comp::<Position>(dealer).map_or_else(Default::default, |p| p.tile);
    crate::systems::law::raise_crime_except(
        world,
        dealer,
        None,
        None,
        crate::components::Crime::ArmsDealing,
        tile,
        Some(buyer),
    );
    let text = match qm {
        Some(g) => format!(
            "{} bought a Weapon T{tier} from {} at {} for {price}, for {}'s armoury",
            world.name_of(buyer),
            world.name_of(dealer),
            world.name_of(seller),
            world.owner_label(Some(g))
        ),
        None => format!(
            "{} bought a Weapon T{tier} from {} at {} for {price}",
            world.name_of(buyer),
            world.name_of(dealer),
            world.name_of(seller)
        ),
    };
    world.push_event(EventKind::AssetBought, &[buyer, seller, a, dealer], text);
    // The pressure pass phase 2: the dealer's side of it (a body's diary).
    let sal = world.config.service.served_salience;
    world.remember(dealer, MemoryKind::Served, Some(buyer), sal, 0.1, false);
    world.remember(buyer, MemoryKind::Served, Some(dealer), sal, 0.1, false);
    world.jobs2.arms.wants.remove(&buyer);
    Ok(a)
}

// ---------------------------------------------------------------------------
// Gunsmiths (§ 4)
// ---------------------------------------------------------------------------

/// `trades::accrue`'s Gunsmith arm: a gunsmith's `ticks` on shift make
/// `make_rate × (0.5 + knowledge)` weapons an hour from `parts_per_weapon`
/// Parts each, at the owner corp's Industry tier (its maker), up to the
/// shop's stock cap and the store's.
pub fn accrue_gunsmith(world: &mut World, worker: EntityId, b: EntityId, ticks: u64) {
    if !on(world) || !crate::systems::industry::open(world, b) {
        return;
    }
    let cfg = world.config.weapons.clone();
    let knowledge = world.comp::<Skills>(worker).map_or(0.0, |s| s.knowledge);
    let units = cfg.make_rate * (0.5 + knowledge) * ticks as f32 / TICKS_PER_HOUR as f32;
    let whole = {
        let Some(bd) = world.comp_mut::<Building>(b) else { return };
        bd.production_accum += units;
        let whole = bd.production_accum.floor().max(0.0);
        bd.production_accum -= whole;
        whole as u32
    };
    let per = cfg.parts_per_weapon.max(1);
    for _ in 0..whole {
        if stock_at(world, b).len() >= cfg.gunsmith_stock as usize
            || world.stock(b, crate::components::Good::Parts) < per
        {
            break;
        }
        let owner = world.owner_of(b);
        let maker = assets::maker_for(world, b, AssetKind::Weapon);
        let t = maker
            .and_then(|m| world.comp::<crate::components::Corp>(m))
            .filter(|c| !c.tech.is_unset())
            .map_or(1, |c| c.tech.tier_of(crate::virt::Track::Industry))
            .clamp(1, 3);
        let p = prov(world, ProvKind::Made, Some(b));
        let Some(a) = spawn_weapon(world, t, owner, AssetLoc::Stock(b), p, false) else { break };
        world.take_stock(b, crate::components::Good::Parts, per);
        if let Some(x) = world.comp_mut::<Asset>(a) {
            x.maker = maker;
        }
        ownership::jobs2_row(world).weapons_made += 1;
    }
}

/// Sell a Gunsmith's weapon to a building's owner for its stock (the
/// Treasury for the Precinct, a corp for its Office): `Flow::Asset`.
/// Returns the coins paid.
fn sell_to_stock(world: &mut World, shop: EntityId, a: EntityId, buyer: Option<EntityId>, to: EntityId) -> i64 {
    let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
    let Some(price) = legal_price(world, shop, tier) else { return 0 };
    let seller = world.owner_of(shop);
    let paid = if seller == buyer { 0 } else { ownership::charge(world, buyer, seller, price, Flow::Asset) };
    ownership::credit(world, shop, paid);
    let p = prov(world, ProvKind::Sold, Some(shop));
    hand_over(world, a, buyer, AssetLoc::Stock(to), p, false);
    ownership::jobs2_row(world).sold_legal += 1;
    paid.max(price * i64::from(seller == buyer))
}

/// The nearest Gunsmith to `from` with stock, with its cheapest weapon.
fn nearest_stock(world: &World, from: crate::components::TilePos) -> Option<(EntityId, EntityId)> {
    world
        .buildings_of_kind(BuildingKind::Gunsmith)
        .iter()
        .copied()
        .filter(|&b| crate::systems::industry::open(world, b))
        .filter_map(|b| {
            let mut s = stock_at(world, b);
            s.sort_by_key(|&a| (world.comp::<Asset>(a).map_or(0, |x| x.tier), a));
            let a = *s.first()?;
            world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b, a))
        })
        .min()
        .map(|(_, b, a)| (b, a))
}

/// Midnight: the Treasury's arms line buys for the Precinct up to its
/// target (`stock_per_guard` per city guard) within `arms_budget`, and the
/// seized surplus past the target is destroyed (the worst first); each
/// Security corp buys for its Offices (per private guard) while it holds
/// the coins. From the Gunsmiths' stock, the nearest first.
fn restock_guards(world: &mut World) {
    let cfg = world.config.weapons.clone();
    if let Some(p) = precinct(world) {
        let city: usize = world.guards().iter().filter(|&&g| crate::systems::law::is_city_guard(world, g)).count();
        let target = (city as f32 * cfg.stock_per_guard).ceil() as usize;
        let mut budget = cfg.arms_budget;
        let door = world.comp::<Building>(p).map(|b| b.door).unwrap_or_default();
        while stock_at(world, p).len() < target && budget > 0 {
            let Some((shop, a)) = nearest_stock(world, door) else { break };
            let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
            let Some(price) = legal_price(world, shop, tier) else { break };
            if price > budget || world.purse(None) < price {
                break;
            }
            budget -= sell_to_stock(world, shop, a, None, p);
        }
        // The seized surplus past the target is destroyed (the worst first).
        let mut s = stock_at(world, p);
        if s.len() > target {
            let over = s.len() - target;
            s.sort_by_key(|&a| (world.comp::<Asset>(a).map_or(0, |x| assets::eff_tier_of(world, x)), a));
            for a in s.into_iter().take(over) {
                assets::despawn(world, a);
                world.jobs2.arms.index.remove(&a);
            }
        }
    }
    let offices: Vec<EntityId> = world.buildings_of_kind(BuildingKind::SecurityOffice).to_vec();
    for o in offices {
        let Some(corp) = world.corp_of_building(o) else { continue };
        let guards =
            world.guards().iter().filter(|&&g| world.comp::<Job>(g).is_some_and(|j| j.employer == Some(o))).count();
        let target = (guards as f32 * cfg.stock_per_guard).ceil() as usize;
        let door = world.comp::<Building>(o).map(|b| b.door).unwrap_or_default();
        while stock_at(world, o).len() < target {
            let Some((shop, a)) = nearest_stock(world, door) else { break };
            let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
            let Some(price) = legal_price(world, shop, tier) else { break };
            if world.purse(Some(corp)) < price * 2 {
                break;
            }
            sell_to_stock(world, shop, a, Some(corp), o);
        }
    }
}

/// The guards' armed shares: the stock over the headcount, and the best
/// tier in it.
fn guard_shares(world: &mut World) {
    let tier_of = |w: &World, b: EntityId| -> (usize, u8) {
        let s = stock_at(w, b);
        let t = s.iter().filter_map(|&a| w.comp::<Asset>(a)).map(|x| assets::eff_tier_of(w, x).max(1)).max();
        (s.len(), t.unwrap_or(0))
    };
    let law = match precinct(world) {
        Some(p) => {
            let (n, t) = tier_of(world, p);
            let city = world.guards().iter().filter(|&&g| crate::systems::law::is_city_guard(world, g)).count();
            ((n as f32 / city.max(1) as f32).min(1.0), t)
        }
        None => (0.0, 0),
    };
    let mut offices = BTreeMap::new();
    for o in world.buildings_of_kind(BuildingKind::SecurityOffice).to_vec() {
        let (n, t) = tier_of(world, o);
        if n == 0 {
            continue;
        }
        let guards =
            world.guards().iter().filter(|&&g| world.comp::<Job>(g).is_some_and(|j| j.employer == Some(o))).count();
        offices.insert(o, ((n as f32 / guards.max(1) as f32).min(1.0), t));
    }
    world.jobs2.arms.law = law;
    world.jobs2.arms.offices = offices;
}

// ---------------------------------------------------------------------------
// Armouries and crates (§ 4)
// ---------------------------------------------------------------------------

/// `raid::depart`: a member marching out draws the armoury's best weapon
/// (the gang's still; returned at midnight).
pub fn issue(world: &mut World, member: EntityId) {
    if !on(world) || world.comp::<Kit>(member).is_some_and(|k| k.weapon > 0) {
        return;
    }
    let Some(gang) = world.gang_of(member) else { return };
    let Some(h) = world.hideout_of(gang) else { return };
    let Some(a) = best_of(world, &stock_at(world, h)) else { return };
    assets::set_loc(world, a, AssetLoc::Carried(member));
    ownership::jobs2_row(world).issued += 1;
}

/// Midnight: every gang weapon carried by a member goes back to the
/// armoury (a jailed or dead member's stays where the law or the street
/// left it).
fn return_issued(world: &mut World) {
    let out: Vec<(EntityId, EntityId)> = world
        .jobs2
        .arms
        .index
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter_map(|(a, x)| match (x.owner, x.loc) {
            (Some(g), AssetLoc::Carried(m)) if world.has::<Gang>(g) && m != g => Some((a, g)),
            _ => None,
        })
        .collect();
    for (a, g) in out {
        let Some(h) = world.hideout_of(g) else { continue };
        let holder = world.comp::<Asset>(a).and_then(|x| x.loc.holder());
        if holder.is_some_and(|m| world.has::<Sentence>(m) || world.has::<Corpse>(m)) {
            continue;
        }
        assets::set_loc(world, a, AssetLoc::Stock(h));
    }
}

/// At `crate_hour`: a gang with members, short of its armoury target plus
/// its dealer's float (`deal_float`), with no crate within `crate_days` and
/// a standing Freight Yard, ships a crate of up to `crate_size` at
/// `crate_unit` each: a syndicate's branch's crate is the parent's, paid
/// from its treasury abroad (in kind, `ParentFund`'s shape); a gang on its
/// own pays the World from the coins above its payroll
/// (`crate_reserve_days` of stipends for the roster and one recruit). A Loader on shift at
/// the Yard (the lowest id) takes the bribe; customs seizes on `p_seize`
/// (× `bribe_mult` bribed) into the Precinct, filing `Smuggling` on the
/// leader; else the crate lands in the armoury.
pub fn crates(world: &mut World) {
    let cfg = world.config.weapons.clone();
    let now = world.tick;
    let day = world.day();
    for gang in world.gangs() {
        let Some(h) = world.hideout_of(gang) else { continue };
        let members = world.comp::<Gang>(gang).map_or(0, |g| g.members.len());
        if members == 0 {
            continue;
        }
        let want = (armoury_target(world, gang) + cfg.deal_float as usize).saturating_sub(stock_at(world, h).len());
        if want == 0 {
            continue;
        }
        if world.jobs2.arms.last_crate.get(&gang).is_some_and(|&t| now < t + u64::from(cfg.crate_days) * TICKS_PER_DAY)
        {
            continue;
        }
        let room = cap(world).saturating_sub(alive(world));
        // A syndicate's branch: the parent ships the crate in kind (its
        // treasury abroad pays, `ParentFund`'s shape); a gang on its own
        // pays from the coins above `crate_reserve_days` of stipends for the
        // roster and one recruit (the payroll first).
        let unit = cfg.crate_unit.max(1);
        let parent = crate::systems::parents::syndicate_id(world, gang);
        let budget = match parent {
            Some(id) => crate::systems::parents::treasury_abroad(world, id),
            None => {
                let reserve = world.config.social.gang_stipend.max(0)
                    * (members as i64 + 1)
                    * i64::from(cfg.crate_reserve_days.max(1));
                world.purse(Some(gang)) - reserve
            }
        };
        let afford = ((budget - cfg.bribe).max(0) / unit) as usize;
        let n = want.min(cfg.crate_size as usize).min(afford).min(room);
        if n == 0 || world.purse(Some(gang)) < cfg.bribe {
            continue;
        }
        let price = unit * n as i64;
        let from = world.comp::<Building>(h).map(|b| b.door).unwrap_or_default();
        let yard = world
            .buildings_of_kind(BuildingKind::FreightYard)
            .iter()
            .copied()
            .filter(|&y| crate::systems::industry::open(world, y))
            .filter_map(|y| world.comp::<Building>(y).map(|b| (b.door.manhattan(from), y)))
            .min()
            .map(|(_, y)| y);
        let Some(yard) = yard else { continue };
        let paid = match parent {
            Some(id) => {
                if crate::systems::parents::spend_abroad(world, id, price) {
                    price
                } else {
                    0
                }
            }
            None => ownership::cross_out(world, Some(gang), crate::outside::WORLD_ACCOUNT, price, Flow::Import, false),
        };
        if paid < price {
            continue;
        }
        world.jobs2.arms.last_crate.insert(gang, now);
        let tod = world.tick_of_day();
        let loader = world.staff_of(yard).iter().copied().find(|&s| {
            world.comp::<Job>(s).is_some_and(|j| {
                j.on_shift(tod)
                    && crate::systems::trades::row_of(world, j.role).is_some_and(|r| r.model == TradeModel::Handling)
            }) && !world.has::<Sentence>(s)
        });
        let mut p = cfg.p_seize;
        if let Some(l) = loader {
            if ownership::pay(world, Some(gang), Some(l), cfg.bribe, Flow::Bribe) > 0 {
                p *= cfg.bribe_mult;
            }
        }
        let gname = world.owner_label(Some(gang));
        let yname = world.name_of(yard);
        if hash(world, PURPOSE_CRATE, key(gang), day) < p {
            let Some(pre) = precinct(world) else { continue };
            for _ in 0..n {
                let pv = prov(world, ProvKind::Seized, Some(pre));
                spawn_weapon(world, cfg.crate_tier, None, AssetLoc::Stock(pre), pv, false);
            }
            ownership::jobs2_row(world).crates_seized += 1;
            let leader = world.comp::<Gang>(gang).and_then(|g| g.leader.or_else(|| g.members.first().copied()));
            if let Some(l) = leader {
                crate::systems::law::file_report(world, crate::components::Crime::Smuggling, l, None);
            }
            world.push_event(
                EventKind::CrateSeized,
                &[gang, yard],
                format!("customs seized {gname}'s crate at {yname}"),
            );
        } else {
            let mut landed = 0;
            for _ in 0..n {
                let pv = prov(world, ProvKind::Smuggled, Some(gang));
                if spawn_weapon(world, cfg.crate_tier, Some(gang), AssetLoc::Stock(h), pv, false).is_some() {
                    landed += 1;
                }
            }
            ownership::jobs2_row(world).crates_landed += 1;
            world.push_event(
                EventKind::CrateLanded,
                &[gang, yard],
                format!("a crate of {landed} weapons for {gname} passed {yname}"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Petty theft and the fence (Dylan's answer 1)
// ---------------------------------------------------------------------------

/// Room for one more petty item theft today (R3's cap).
fn petty_room(world: &World) -> bool {
    world.jobs2.arms.petty_today < per_capita_day(world, world.config.weapons.petty_per_1000_day)
}

/// `exec::steal_food` at a Market: on `p_pickpocket` the thief also lifts
/// the best item a body within `petty_reach` carries (a concealment
/// contest: the victim's sight against the thief's stealth); with no
/// carrier near, the till of a stall in the Market's district (up to
/// `stall_take_max` of its owner's coins). A Theft each, the victim's.
pub fn market_petty(world: &mut World, thief: EntityId) {
    if !on(world) || !petty_room(world) {
        return;
    }
    let cfg = world.config.weapons.clone();
    let Some(tile) = world.comp::<Position>(thief).map(|p| p.tile) else { return };
    let tick = world.tick;
    if hash(world, PURPOSE_PETTY, key(thief), tick) >= cfg.p_pickpocket {
        return;
    }
    let near = world.agents_near(tile, cfg.petty_reach, None);
    // The pressure pass S6 (Dylan 2026-10-27): nobody under 18 is a victim.
    let victim = near.into_iter().filter(|&o| o != thief && !crate::systems::ages::is_protected(world, o)).find(|&o| {
        world.has::<Identity>(o) && !world.has::<Corpse>(o) && world.comp::<Kit>(o).is_some_and(|k| k.weapon > 0)
    });
    if let Some(v) = victim {
        let stealth = crate::systems::law::stealth(world, thief);
        let step = cfg.conceal_step;
        let p_lift = crate::systems::security::contest_p(
            f32::from(crate::systems::security::thief_tier(stealth)),
            f32::from(sight_tier(world, v)),
            step,
        );
        if hash(world, PURPOSE_PETTY, key(v), tick) >= p_lift {
            return;
        }
        let Some(a) = best_of(world, &carried_by(world, v)) else { return };
        let p = prov(world, ProvKind::Stolen, Some(thief));
        hand_over(world, a, Some(thief), AssetLoc::Carried(thief), p, true);
        world.jobs2.arms.petty_today += 1;
        ownership::jobs2_row(world).pickpockets += 1;
        let text = format!("{} lifted a Weapon from {}", world.name_of(thief), world.name_of(v));
        world.push_event(EventKind::Theft, &[thief, v], text);
        crate::systems::law::raise_crime(world, thief, Some(v), crate::components::Crime::Theft, tile);
        return;
    }
    // A stall of the Market's district with a take today (the till on the
    // thief's way out; the lowest owner id).
    let d = world.district_of(tile);
    let stall = world
        .jobs2
        .pitches
        .iter()
        // Jobs & lives phase 4: a street worker's pitch is no stall.
        .find(|(&o, p)| {
            o != thief && p.take_today > 0 && p.district == d && !crate::systems::vice::is_street_pitch(world, p)
        })
        .map(|(&o, p)| (o, p.take_today));
    let Some((owner, take)) = stall else { return };
    let coins = take.min(cfg.stall_take_max).min(world.comp::<Wallet>(owner).map_or(0, |w| w.coins.max(0)));
    if coins <= 0 {
        return;
    }
    if let Some(w) = world.comp_mut::<Wallet>(owner) {
        w.coins -= coins;
    }
    if let Some(w) = world.comp_mut::<Wallet>(thief) {
        w.coins += coins;
    }
    if let Some(p) = world.jobs2.pitches.get_mut(&owner) {
        p.take_today -= coins;
    }
    world.jobs2.arms.petty_today += 1;
    ownership::jobs2_row(world).stall_thefts += 1;
    let text = format!("{} emptied the till of {}'s stall ({coins} coins)", world.name_of(thief), world.name_of(owner));
    world.push_event(EventKind::Theft, &[thief, owner], text);
    crate::systems::law::raise_crime(world, thief, Some(owner), crate::components::Crime::Theft, tile);
}

/// `exec::steal_food` at a Home: the thief also takes the best item kept
/// there (stolen; the Theft is the food's).
pub fn home_petty(world: &mut World, thief: EntityId, home: EntityId) {
    if !on(world) || !petty_room(world) {
        return;
    }
    let Some(a) = best_of(world, &kept_at(world, home)) else { return };
    let p = prov(world, ProvKind::Stolen, Some(thief));
    hand_over(world, a, Some(thief), AssetLoc::Carried(thief), p, true);
    world.jobs2.arms.petty_today += 1;
    let clothing = world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Clothing);
    let row = ownership::jobs2_row(world);
    row.home_thefts += 1;
    if clothing {
        row.clothes_stolen += 1;
    }
    let what = if clothing { "Clothing" } else { "Weapon" };
    let text = format!("{} took a {what} from {}", world.name_of(thief), world.name_of(home));
    world.push_event(EventKind::Theft, &[thief, home], text);
}

/// The gang fence for items: the stolen items `thief` carries, sold to
/// `gang` at `fence_frac` of their value (`Flow::Fence`, while the gang
/// can pay) into its armoury. Returns the items fenced.
pub fn fence_items(world: &mut World, thief: EntityId, gang: EntityId) -> u32 {
    if !on(world) {
        return 0;
    }
    let Some(h) = world.hideout_of(gang) else { return 0 };
    let frac = world.config.weapons.fence_frac;
    let mut n = 0;
    // Jobs & lives phase 4: the stolen clothing carried too.
    let items: Vec<EntityId> = assets::assets_at(world, thief)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind.is_item() && x.loc == AssetLoc::Carried(thief)))
        .collect();
    for a in items {
        let Some((stolen, value, owner)) = world.comp::<Asset>(a).map(|x| (x.stolen, x.value, x.owner)) else {
            continue;
        };
        if !stolen || owner != Some(thief) {
            continue;
        }
        let pay = ((value as f32 * frac).round() as i64).max(1);
        if world.purse(Some(gang)) < pay {
            break;
        }
        ownership::pay(world, Some(gang), Some(thief), pay, Flow::Fence);
        assets::into_gang_stock(world, a, gang, h);
        let pv = prov(world, ProvKind::Stolen, Some(thief));
        if let Some(x) = world.comp_mut::<Asset>(a) {
            x.prov = Some(pv);
        }
        ownership::jobs2_row(world).fenced += 1;
        n += 1;
    }
    n
}

/// Midnight: a thief still carrying stolen items fences them through his
/// gang, else the gang holding his district (a free agent keeps them).
fn fence_daily(world: &mut World) {
    let thieves: Vec<EntityId> = {
        let mut v: Vec<EntityId> = world
            .jobs2
            .arms
            .index
            .iter()
            .chain(world.jobs2.fashion.index.iter())
            .filter_map(|&a| world.comp::<Asset>(a))
            .filter(|x| x.stolen)
            .filter_map(|x| match x.loc {
                AssetLoc::Carried(h) if x.owner == Some(h) => Some(h),
                _ => None,
            })
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    for t in thieves {
        if !free_adult(world, t) {
            continue;
        }
        let gang = world.gang_of(t).or_else(|| {
            let tile = world.comp::<Position>(t)?.tile;
            match world.district(world.district_of(tile)).control {
                crate::components::Controller::Gang(g) if world.has::<Gang>(g) => Some(g),
                _ => None,
            }
        });
        if let Some(g) = gang {
            fence_items(world, t, g);
        }
    }
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

/// The pressure pass phase 2 (OPEN-176; `[customers]`): a gang whose
/// armoury is under its target and whose purse holds a dealer's price
/// sends a member (a body first, else its lowest id; free, no dealer) to
/// buy from another gang's dealer with a surplus, before it ships abroad:
/// the quartermaster wants a weapon (the Shop goal walks him to the Bar),
/// the gang's purse pays, the weapon goes to its Hideout. One a gang a day;
/// a booking unspent by the next midnight is dropped.
fn quartermasters(world: &mut World) {
    world.jobs2.arms.quarter.clear();
    if crate::systems::customers::cfg(world).is_none() {
        return;
    }
    let price = match dealer_price(world, 1) {
        Some(p) => p,
        None => return,
    };
    let sellers: BTreeSet<EntityId> =
        deal_bars(world, false).into_iter().filter_map(|b| bar_arms_dealer(world, b, false).map(|(_, g)| g)).collect();
    if sellers.is_empty() {
        return;
    }
    for g in world.gangs() {
        let Some(h) = world.hideout_of(g) else { continue };
        if stock_at(world, h).len() >= armoury_target(world, g) || !sellers.iter().any(|&x| x != g) {
            continue;
        }
        if world.purse(Some(g)) < price + world.config.weapons.coin_reserve {
            continue;
        }
        let Some(members) = world.comp::<Gang>(g).map(|x| x.members.clone()) else { continue };
        let fit = |w: &World, m: EntityId| {
            free_adult(w, m) && !crate::systems::stims::is_dealer(w, m) && !w.jobs2.arms.wants.contains(&m)
        };
        let body =
            |w: &World, m: EntityId| w.comp::<Brain>(m).is_some_and(|b| b.lod != crate::components::Lod::Statistical);
        let pick = members
            .iter()
            .copied()
            .find(|&m| fit(world, m) && body(world, m))
            .or_else(|| members.iter().copied().find(|&m| fit(world, m)));
        if let Some(m) = pick {
            world.jobs2.arms.quarter.insert(m, g);
            world.jobs2.arms.wants.insert(m);
        }
    }
}

/// The pressure pass round 2, R6b (OPEN-217; `[verbs]`): the gangs' gun
/// trade, hideout to hideout. On its trade day (`trade_days`, a hash of
/// the gang: no saved state) a gang under its armoury target whose purse
/// holds a weapon's price above `trade_reserve_days` of its members'
/// stipends buys up to `trade_lot` from the other gang with the largest
/// surplus (its Hideout nearest, then the lower id) at `trade_price_share`
/// of the dealer's T1 price a weapon: the seller's dealer (a registered
/// one, else its leader) makes the deal with the buyer's leader, the gang
/// purses pay (`Flow::Arms`), the weapons move armoury to armoury. The
/// quartermaster's walk-in (phase 2) buys one at the full price; this is
/// the lot a purse affords.
fn gang_arms_trade(world: &mut World) {
    let Some(cfg) = world.config.verbs.clone() else { return };
    if cfg.trade_lot == 0 || cfg.trade_days == 0 {
        return;
    }
    let Some(list) = dealer_price(world, 1) else { return };
    let unit = ((list as f32 * cfg.trade_price_share).round() as i64).max(1);
    let day = world.day();
    let stipend = world.config.social.gang_stipend.max(0);
    for g in world.gangs() {
        // The gang's trade day: the hash's slot in the cycle.
        let slot = (hash(world, PURPOSE_TRADE, key(g), 0) * f32::from(cfg.trade_days)) as u64;
        if day % u64::from(cfg.trade_days) != slot.min(u64::from(cfg.trade_days) - 1) {
            continue;
        }
        let Some(h) = world.hideout_of(g) else { continue };
        let members = world.comp::<Gang>(g).map_or(0, |x| x.members.len());
        if members == 0 {
            continue;
        }
        let want = armoury_target(world, g).saturating_sub(stock_at(world, h).len());
        if want == 0 {
            continue;
        }
        let reserve = stipend * members as i64 * i64::from(cfg.trade_reserve_days);
        let budget = world.purse(Some(g)) - reserve;
        let afford = (budget.max(0) / unit) as usize;
        let n = want.min(usize::from(cfg.trade_lot)).min(afford);
        if n == 0 {
            continue;
        }
        let from = world.comp::<Building>(h).map(|b| b.door).unwrap_or_default();
        let seller = world
            .gangs()
            .into_iter()
            .filter(|&s| s != g)
            .filter_map(|s| {
                let left = surplus(world, s).len();
                let door = world.hideout_of(s).and_then(|x| world.comp::<Building>(x)).map(|b| b.door)?;
                (left > 0).then(|| (std::cmp::Reverse(left), door.manhattan(from), s))
            })
            .min()
            .map(|(_, _, s)| s);
        let Some(s) = seller else { continue };
        let lot: Vec<EntityId> = surplus(world, s).into_iter().take(n).collect();
        let price = unit * lot.len() as i64;
        if lot.is_empty() || ownership::pay(world, Some(g), Some(s), price, Flow::Arms) < price {
            continue;
        }
        let dealer = world
            .dealers
            .values()
            .flatten()
            .copied()
            .find(|&d| world.gang_of(d) == Some(s) && crate::systems::law::living(world, d))
            .or_else(|| world.comp::<Gang>(s).and_then(|x| x.leader));
        let buyer = world.comp::<Gang>(g).and_then(|x| x.leader);
        for &a in &lot {
            let p = prov(world, ProvKind::Dealt, dealer.or(Some(s)));
            hand_over(world, a, Some(g), AssetLoc::Stock(h), p, false);
        }
        ownership::jobs2_row(world).gang_arms_buys += 1;
        let (gn, sn) = (world.owner_label(Some(g)), world.owner_label(Some(s)));
        let who = |w: &World, e: Option<EntityId>| e.map_or_else(|| "nobody".to_string(), |x| w.name_of(x));
        let what = if lot.len() == 1 { "a weapon".to_string() } else { format!("{} weapons", lot.len()) };
        let text = format!(
            "{sn} sold {gn} {what} for {price} ({unit} each): {} dealt with {}; the lot moved hideout to hideout",
            who(world, dealer),
            who(world, buyer)
        );
        let mut actors = vec![g, s];
        actors.extend(dealer);
        actors.extend(buyer);
        world.push_event(EventKind::AssetBought, &actors, text);
    }
}

/// Each weapon an agent owns: carried today when the owner has a reason
/// (a gang member, a licence for a guard or Security post, the reasons to
/// want one), else on `carry_p_by_class`; else kept at home.
fn carry_roll(world: &mut World) {
    let day = world.day();
    let items: Vec<(EntityId, EntityId, AssetLoc)> = world
        .jobs2
        .arms
        .index
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter_map(|(a, x)| {
            let o = x.owner?;
            matches!(x.loc, AssetLoc::Carried(h) | AssetLoc::Parked(h) if h == o || world.comp::<Household>(o).and_then(|hh| hh.home) == Some(h))
                .then_some((a, o, x.loc))
        })
        .collect();
    for (a, o, loc) in items {
        if !free_adult(world, o) {
            continue;
        }
        let home = world.comp::<Household>(o).and_then(|h| h.home);
        let reason = world.gang_of(o).is_some() || licensed(world, o) || has_reason(world, o);
        let carry = reason
            || hash(world, PURPOSE_CARRY, key(o), day) < world.config.weapons.carry_p_by_class[class_index(world, o)];
        let want = match (carry, home) {
            (true, _) | (false, None) => AssetLoc::Carried(o),
            (false, Some(h)) => AssetLoc::Parked(h),
        };
        if want != loc {
            assets::set_loc(world, a, want);
        }
    }
}

/// The adults who want a weapon (no weapon owned, the coins for the
/// cheapest, and a reason or the class's stable share), and the licences
/// the lawful among them buy from the Civic Clerk while one is on the books.
fn wants_and_licences(world: &mut World) {
    let cfg = world.config.weapons.clone();
    let cheapest = assets::list_price(world, AssetKind::Weapon, 1).unwrap_or(i64::MAX);
    let owners: BTreeSet<EntityId> =
        world.jobs2.arms.index.iter().filter_map(|&a| world.comp::<Asset>(a)).filter_map(|x| x.owner).collect();
    let mut wants = BTreeSet::new();
    let ids: Vec<EntityId> = world.with::<Identity>();
    for id in ids {
        if !free_adult(world, id) || owners.contains(&id) || world.gang_of(id).is_some() {
            continue;
        }
        if world.comp::<Job>(id).is_some_and(|j| j.role == crate::components::Role::Guard) {
            continue;
        }
        let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
        if coins < cheapest + cfg.coin_reserve {
            continue;
        }
        let share = cfg.want_p_by_class[class_index(world, id)];
        if has_reason(world, id) || hash(world, PURPOSE_WANT, key(id), 0) < share {
            wants.insert(id);
        }
    }
    let fear = wants.iter().filter(|&&id| crate::systems::customers::frightened(world, id)).count() as u32;
    ownership::jobs2_row(world).fear_wants += fear;
    world.jobs2.arms.wants = wants;
    if crate::systems::industry::clerks_on_books(world) == 0 {
        return;
    }
    let buyers: Vec<EntityId> = world
        .jobs2
        .arms
        .wants
        .iter()
        .copied()
        .filter(|&id| !licensed(world, id))
        .filter(|&id| world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.5))
        .filter(|&id| {
            world.comp::<Wallet>(id).is_some_and(|w| w.coins >= cfg.licence_fee + cheapest + cfg.coin_reserve)
        })
        .collect();
    for id in buyers {
        let paid = ownership::pay(world, Some(id), None, cfg.licence_fee, Flow::Licence);
        if paid < cfg.licence_fee {
            continue;
        }
        world.jobs2.arms.licences.insert(id);
        ownership::jobs2_row(world).licences_sold += 1;
        // Lives at work phase 5 (§ 2.2, civic clerk): the clerk named.
        let clerk = crate::systems::service::licence_clerk(world, id);
        let from = clerk.map(|(c, _)| format!(" from {}", world.name_of(c))).unwrap_or_default();
        let text = format!("{} bought a weapons licence for {}{from}", world.name_of(id), cfg.licence_fee);
        let actors: Vec<EntityId> = std::iter::once(id).chain(clerk.map(|(c, _)| c)).collect();
        world.push_event(EventKind::LicenceSold, &actors, text);
        if let Some((c, hall)) = clerk {
            crate::systems::service::licence_sold(world, c, hall, id, cfg.licence_fee);
        }
    }
}

// ---------------------------------------------------------------------------
// Seeding and the day's snapshot
// ---------------------------------------------------------------------------

/// `World::new`, after the industries: a Gunsmith per copy of each
/// `gunsmith_rows` corp, and the opening stocks (no RNG; no coins: the
/// seeded city's stock is unpaid, as its buildings are): the Precinct at
/// `stock_per_guard` per city guard, each gang's armoury at half its
/// target, each Gunsmith at half its cap (tier 1).
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.weapons.clone();
    let copies = world.lot_copies();
    for j in 0..copies {
        for &r in &cfg.gunsmith_rows {
            let Some(t) = world.config.corps.row_template(r) else { continue };
            let Some(&c) = world.corps_of_template(t).get(j) else { continue };
            crate::systems::industry::seed_kind(world, BuildingKind::Gunsmith, Some(c));
        }
    }
    let now = world.tick;
    if let Some(p) = precinct(world) {
        let city = world.guards().iter().filter(|&&g| crate::systems::law::is_city_guard(world, g)).count();
        let n = (city as f32 * cfg.stock_per_guard).ceil() as usize;
        for _ in 0..n {
            let pv = Provenance { how: ProvKind::Seized, by: Some(p), at: now };
            spawn_weapon(world, cfg.guard_tier, None, AssetLoc::Stock(p), pv, false);
        }
    }
    for gang in world.gangs() {
        let Some(h) = world.hideout_of(gang) else { continue };
        let n = armoury_target(world, gang) / 2;
        for _ in 0..n {
            let pv = Provenance { how: ProvKind::Smuggled, by: Some(gang), at: now };
            spawn_weapon(world, 1, Some(gang), AssetLoc::Stock(h), pv, false);
        }
    }
    for b in world.buildings_of_kind(BuildingKind::Gunsmith).to_vec() {
        let owner = world.owner_of(b);
        for _ in 0..cfg.gunsmith_stock / 2 {
            let pv = Provenance { how: ProvKind::Made, by: Some(b), at: now };
            spawn_weapon(world, 1, owner, AssetLoc::Stock(b), pv, false);
        }
    }
    guard_shares(world);
}

/// The day's end (`systems::stats`): the snapshots of the weapons group.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let mut alive = 0u32;
    let mut carriers: BTreeSet<EntityId> = BTreeSet::new();
    let (mut armoury, mut precinct_n, mut shops) = (0u32, 0u32, 0u32);
    let pre = precinct(world);
    for &a in &world.jobs2.arms.index {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        if x.kind != AssetKind::Weapon {
            continue;
        }
        alive += 1;
        match x.loc {
            AssetLoc::Carried(h) => {
                carriers.insert(h);
            }
            AssetLoc::Stock(b) => {
                match world.comp::<Building>(b).map(|bd| bd.kind) {
                    Some(BuildingKind::Hideout) => armoury += 1,
                    Some(BuildingKind::Gunsmith) => shops += 1,
                    _ => {}
                }
                if Some(b) == pre {
                    precinct_n += 1;
                }
            }
            _ => {}
        }
    }
    let licences = world.jobs2.arms.licences.len() as u32;
    let wants = world.jobs2.arms.wants.len() as u32;
    let row = ownership::jobs2_row(world);
    row.weapons = alive;
    row.carriers = carriers.len() as u32;
    row.armoury = armoury;
    row.precinct_arms = precinct_n;
    row.gunsmith_stock = shops;
    row.licences = licences;
    row.weapon_wants = wants;
}

/// God (`FloodGuns`): `n` weapons at `tier` into a gang's armoury, past the
/// store's cap (the god scenario's lever). Returns the weapons made.
pub fn flood(world: &mut World, gang: EntityId, n: u32, tier: u8) -> Result<u32, String> {
    if !on(world) {
        return Err("weapons are off".into());
    }
    let h = world.hideout_of(gang).ok_or("no such gang with a Hideout")?;
    let now = world.tick;
    let mut made = 0;
    for _ in 0..n {
        let pv = Provenance { how: ProvKind::Granted, by: None, at: now };
        if spawn_weapon(world, tier, Some(gang), AssetLoc::Stock(h), pv, true).is_some() {
            made += 1;
        }
    }
    Ok(made)
}
