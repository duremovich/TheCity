//! Jobs & lives phase 7, life stages (docs/JOBS_AND_LIVES.md § 6; D11-D13;
//! Dylan's answers 4 and 5). Under `[ages] enabled` (itself under `[jobs2]
//! enabled`): off, every entry point is a no-op, no School stands, every
//! child's routine stays `Play` (not written), the seeded ages are `[world]`'s
//! and nobody is maimed.
//!
//! Counters, rolls and coins:
//!
//! - **Children** (no Brain). At midnight each child's routine is chosen
//!   from its household: a camp's child → `Camp`; a homeless household's →
//!   `Street` (it begs `child_beg_coins` from a resident of its district, a
//!   gift; steals on `child_theft_p`; after `street_take_days` the camp
//!   takes it, the E38 path); on a weekday a School in reach whose fee the
//!   household can spare (or the `public_school` lever) → `School` (the fee
//!   `Flow::Contract` to its owner, `edu_days`); on a weekday a parent with a
//!   Pitch → `Chores` (at the pitch); else `Play`. At 18 the school days add
//!   `edu_gain` each to the education (`Schooling`), and a street childhood
//!   takes stealth up and lawfulness down.
//! - **The elderly** (no pension). From `frail_from_years` work output
//!   falls `frail_slope` a year over (`scale_ticks` on the Farm, Fab and
//!   trade accruals, both tiers); the sick-day firing comes at
//!   `elder_fire_mult` of the days. A jobless elder with an adult child
//!   housed elsewhere moves in on `kin_take_in_p` a day (room permitting);
//!   else the Mission cot and the street, as anyone homeless.
//! - **Disability.** A level (1-3) and a slot: a stabilised survivor on
//!   `p_maimed`, a living victim's ripped slot, a cancer in remission (no
//!   slot), a packer's press on `p_press_maim` an hour. Work output × (1 −
//!   `maimed_work` × level); a beggar's odds × `maimed_beg_mult`. With the
//!   coins, a prosthetic (a T1 implant in the slot, bought and installed at
//!   the nearest Clinic) clears it and its load applies.
//!
//! Every coin moves between purses. The midnight pass is O(children +
//! elders + maimed).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{
    Building, BuildingKind, Child, Household, Identity, Job, Personality, Routine, Sentence, Skills, Slot, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};

use crate::world::World;

const PURPOSE_BEG: u64 = 0x4147_0001;
const PURPOSE_THEFT: u64 = 0x4147_0002;
const PURPOSE_KIN: u64 = 0x4147_0003;
const PURPOSE_MAIM: u64 = 0x4147_0004;

/// The life stages are live: `[jobs2] enabled` and `[ages] enabled`.
pub fn on(world: &World) -> bool {
    on_cfg(&world.config)
}

/// [`on`] from a config (the seeding reads it before the world stands).
pub fn on_cfg(cfg: &crate::config::Config) -> bool {
    cfg.jobs2.enabled && cfg.ages.enabled
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// Phase 7's state on `Jobs2State` (empty, and not saved, off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgesState {
    /// The maimed: their level (1-3) and the slot (none for a cancer).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub maimed: BTreeMap<EntityId, (u8, Option<Slot>)>,
}

impl AgesState {
    pub fn is_default(&self) -> bool {
        *self == AgesState::default()
    }
}

pub fn years(world: &World, id: EntityId) -> u32 {
    world.comp::<Identity>(id).map_or(0, |i| i.age_years())
}

fn weekday(world: &World) -> bool {
    world.day() % 7 < 5
}

// ---------------------------------------------------------------------------
// Work, firing, begging
// ---------------------------------------------------------------------------

/// The worker's output multiplier: frailty past `frail_from_years`, and the
/// maimed level (1 off, and for the whole and young).
pub fn work_mult(world: &World, id: EntityId) -> f32 {
    if !on(world) {
        return 1.0;
    }
    let cfg = &world.config.ages;
    let over = years(world, id).saturating_sub(cfg.frail_from_years) as f32;
    let frail = (1.0 - cfg.frail_slope * over).max(0.2);
    let maimed =
        world.jobs2.ages.maimed.get(&id).map_or(1.0, |&(l, _)| (1.0 - cfg.maimed_work * f32::from(l)).max(0.2));
    frail * maimed
}

/// A work accrual's ticks scaled by [`work_mult`] (unchanged at 1).
pub fn scale_ticks(world: &World, id: EntityId, ticks: u64) -> u64 {
    let m = work_mult(world, id);
    if m >= 1.0 {
        return ticks;
    }
    (ticks as f32 * m).round() as u64
}

/// `health`'s sick-day firing threshold for `id`: `elder_fire_mult` of the
/// days for an elder (rounded up), else the days.
pub fn fire_days(world: &World, id: EntityId, days: u32) -> u32 {
    if !on(world) || years(world, id) < world.config.ages.frail_from_years {
        return days;
    }
    ((days as f32 * world.config.ages.elder_fire_mult).ceil() as u32).max(1)
}

/// A beggar's odds multiplier (`maimed_beg_mult` for the maimed, else 1).
pub fn beg_mult(world: &World, id: EntityId) -> f32 {
    if on(world) && world.jobs2.ages.maimed.contains_key(&id) {
        world.config.ages.maimed_beg_mult
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------
// Disability
// ---------------------------------------------------------------------------

/// Maim `id` one level (to 3) in `slot` (`why` for the event).
pub fn maim(world: &mut World, id: EntityId, slot: Option<Slot>, why: &str) {
    if !on(world) || !crate::systems::law::living(world, id) {
        return;
    }
    // Phase-8 review: a behaviour chip ripped out is no limb lost (its
    // "prosthetic" was a behaviour chip): a maim with no slot.
    let slot = slot.filter(|&s| s != Slot::Behaviour);
    let e = world.jobs2.ages.maimed.entry(id).or_insert((0, slot));
    e.0 = (e.0 + 1).min(3);
    if e.1.is_none() {
        e.1 = slot;
    }
    ownership::jobs2_row(world).maimed_new += 1;
    let text = format!("{} was left maimed ({why})", world.name_of(id));
    world.push_event(EventKind::Wounded, &[id], text);
}

/// `wounded::rise`: a stabilised survivor maimed on `p_maimed` (the legs or
/// the arms by a hash).
pub fn on_rise(world: &mut World, id: EntityId) {
    if !on(world) {
        return;
    }
    let u = hash(world, PURPOSE_MAIM, key(id), world.tick);
    if u < world.config.ages.p_maimed {
        let slot = if u < world.config.ages.p_maimed / 2.0 { Slot::Legs } else { Slot::Arms };
        maim(world, id, Some(slot), "a wound that healed badly");
    }
}

/// `trades::accrue`'s press: a packer's hour on `p_press_maim`.
pub fn press(world: &mut World, worker: EntityId, ticks: u64) {
    if !on(world) {
        return;
    }
    let p = world.config.ages.p_press_maim * ticks as f32 / crate::time::TICKS_PER_HOUR as f32;
    if hash(world, PURPOSE_MAIM, key(worker), world.tick ^ 0x5052) < p {
        maim(world, worker, Some(Slot::Arms), "the press");
    }
}

/// Midnight: a maimed agent with a slot and the coins buys a T1 prosthetic
/// in it at the nearest Clinic (`chrome::buy_install`): the level clears.
fn prosthetics(world: &mut World) {
    let list: Vec<(EntityId, Slot)> = world
        .jobs2
        .ages
        .maimed
        .iter()
        .filter_map(|(&a, &(_, s))| s.filter(|&s| s != Slot::Behaviour).map(|s| (a, s)))
        .collect();
    for (a, slot) in list {
        if !crate::systems::law::living(world, a) || world.has::<Sentence>(a) {
            continue;
        }
        let kind = crate::components::AssetKind::Implant(slot);
        let Some(price) = crate::systems::assets::list_price(world, kind, 1) else { continue };
        if world.purse(Some(a)) < price + crate::systems::chrome::install_fee(world, 1) {
            continue;
        }
        let Some(from) = world.comp::<crate::components::Position>(a).map(|p| p.tile) else { continue };
        let Some(clinic) = world.nearest_of_kind(BuildingKind::Clinic, from) else { continue };
        let pick = crate::components::ShopPick { kind, tier: 1, used: None, upgrade: false };
        if crate::systems::chrome::buy_install(world, a, clinic, &pick).is_ok() {
            world.jobs2.ages.maimed.remove(&a);
            ownership::jobs2_row(world).prosthetics += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Children
// ---------------------------------------------------------------------------

/// A School within `school_reach` of `from` (door Manhattan, ties the lower id).
pub(crate) fn school_near(world: &World, from: crate::components::TilePos) -> Option<EntityId> {
    let reach = world.config.ages.school_reach;
    world
        .buildings_of_kind(BuildingKind::School)
        .iter()
        .copied()
        .filter_map(|b| {
            let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict)?;
            let d = bd.door.manhattan(from);
            (d <= reach).then_some((d, b))
        })
        .min()
        .map(|(_, b)| b)
}

/// The household's payer: a living adult parent sharing the child's home,
/// else any adult resident of it (the lowest id).
fn head_of(world: &World, child: EntityId, home: EntityId) -> Option<EntityId> {
    let parents = crate::systems::camp::parents_of(world, child);
    parents.into_iter().filter(|&p| world.comp::<Household>(p).and_then(|h| h.home) == Some(home)).min().or_else(|| {
        world.residents_of(home).iter().copied().filter(|&r| crate::systems::demography::is_adult(world, r)).min()
    })
}

/// Where `child` is today (the § 12 probe reads it): the School, a
/// parent's Pitch tile, home, the street, the camp.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Place {
    School(EntityId),
    Pitch(crate::components::TilePos),
    Home,
    Street,
    Camp,
}

/// The child's place for its routine today.
pub fn place_of(world: &World, child: EntityId) -> Option<Place> {
    let c = world.comp::<Child>(child)?;
    let home = world.comp::<Household>(child).and_then(|h| h.home);
    Some(match c.routine {
        Routine::School => {
            let from = home.and_then(|h| world.comp::<Building>(h)).map(|b| b.door)?;
            Place::School(school_near(world, from)?)
        }
        Routine::Chores => {
            let p = crate::systems::camp::parents_of(world, child)
                .into_iter()
                .find_map(|p| world.jobs2.pitches.get(&p).map(|x| x.spot))?;
            Place::Pitch(p)
        }
        Routine::Play => Place::Home,
        Routine::Street => Place::Street,
        Routine::Camp => Place::Camp,
    })
}

/// Midnight: each child's routine and its day's effect.
fn children(world: &mut World) {
    let cfg = world.config.ages.clone();
    let day = world.day();
    let wd = weekday(world);
    let kids: Vec<EntityId> = world.with::<Child>();
    let mut takes: Vec<EntityId> = Vec::new();
    for k in kids {
        // A child has no Brain (`law::living` reads one): alive, not a corpse.
        if !world.is_alive(k) || world.has::<crate::components::Corpse>(k) {
            continue;
        }
        let home = world.comp::<Household>(k).and_then(|h| h.home);
        let in_camp = home.is_some_and(|h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp));
        let routine = if in_camp {
            Routine::Camp
        } else if let Some(h) = home {
            let door = world.comp::<Building>(h).map(|b| b.door);
            let school = if wd { door.and_then(|d| school_near(world, d)) } else { None };
            let head = head_of(world, k, h);
            let paid = school.is_some_and(|s| {
                if world.levers.public_school {
                    return true;
                }
                let Some(p) = head.filter(|&p| crate::systems::leisure::spendable(world, p) >= cfg.school_fee) else {
                    return false;
                };
                let owner = world.owner_of(s);
                let moved = ownership::pay(world, Some(p), owner, cfg.school_fee, Flow::Contract);
                ownership::credit(world, s, moved);
                ownership::jobs2_row(world).school_fees += moved;
                moved > 0
            });
            let pitch =
                crate::systems::camp::parents_of(world, k).into_iter().any(|p| world.jobs2.pitches.contains_key(&p));
            if paid {
                Routine::School
            } else if wd && pitch {
                Routine::Chores
            } else {
                Routine::Play
            }
        } else {
            Routine::Street
        };
        if routine == Routine::Street {
            street_day(world, k, day);
        }
        let take = {
            let Some(c) = world.comp_mut::<Child>(k) else { continue };
            c.routine = routine;
            if routine == Routine::School {
                c.edu_days = c.edu_days.saturating_add(1);
            }
            if routine == Routine::Street {
                c.street_days = c.street_days.saturating_add(1);
            } else {
                c.street_days = 0;
            }
            c.street_days >= cfg.street_take_days
        };
        if take {
            takes.push(k);
        }
        let row = ownership::jobs2_row(world);
        row.child_days += 1;
        if routine == Routine::School {
            row.school_days += 1;
        }
    }
    for k in takes {
        if crate::systems::camp::take(world, k).is_some() {
            ownership::jobs2_row(world).street_takes += 1;
        }
    }
}

/// A Street child's day: a gift of `child_beg_coins` from a resident of its
/// district (the first by a hash with coins over 10), and on `child_theft_p`
/// a resident's coins (up to 3).
fn street_day(world: &mut World, k: EntityId, day: u64) {
    let cfg = world.config.ages.clone();
    let Some(t) = world.comp::<crate::components::Position>(k).map(|p| p.tile) else { return };
    let d = world.district_of(t);
    let res = world.districts.get(d.index()).map(|x| x.residents.clone()).unwrap_or_default();
    if res.is_empty() {
        return;
    }
    let pick = |world: &World, purpose: u64| -> Option<EntityId> {
        let u = hash(world, purpose, key(k), day);
        let start = ((u * res.len() as f32) as usize).min(res.len() - 1);
        (0..res.len().min(8))
            .map(|i| res[(start + i) % res.len()])
            .find(|&r| r != k && world.comp::<Wallet>(r).is_some_and(|w| w.coins > 10))
    };
    if let Some(g) = pick(world, PURPOSE_BEG) {
        let n = ownership::pay(world, Some(g), Some(k), cfg.child_beg_coins, Flow::Donate);
        ownership::jobs2_row(world).child_begged += n;
    }
    if hash(world, PURPOSE_THEFT, key(k), day) < cfg.child_theft_p {
        if let Some(v) = pick(world, PURPOSE_THEFT ^ 1) {
            let n = world.comp::<Wallet>(v).map_or(0, |w| w.coins.min(3));
            if let Some(w) = world.comp_mut::<Wallet>(v) {
                w.coins -= n;
            }
            if let Some(w) = world.comp_mut::<Wallet>(k) {
                w.coins += n;
            }
            ownership::jobs2_row(world).child_thefts += 1;
        }
    }
}

/// `demography::mature`: the childhood's mark: `edu_gain` education a school
/// day (`Schooling`), a street childhood's stealth up and lawfulness down.
pub fn on_mature(world: &mut World, id: EntityId, c: &Child) {
    if !on(world) {
        return;
    }
    // Lives at work phase 3 (§ 2.2, teacher): under `[service] civic` the
    // education is the days taught (a School day with no teacher on duty
    // teaches nothing); off, the midnight attendance count as before.
    let days = if crate::systems::service::civic_on(world) { c.edu_taught } else { f32::from(c.edu_days) };
    let gain = world.config.ages.edu_gain * days;
    if gain > 0.0 {
        if let Some(s) = world.comp_mut::<crate::components::Schooling>(id) {
            s.education = (s.education + gain).min(1.0);
        }
    }
    if c.routine == Routine::Street {
        if let Some(s) = world.comp_mut::<Skills>(id) {
            s.stealth = (s.stealth + 0.1).min(1.0);
        }
        if let Some(p) = world.comp_mut::<Personality>(id) {
            p.lawfulness = (p.lawfulness - 0.1).max(0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// The elderly
// ---------------------------------------------------------------------------

/// Midnight: a jobless elder (not housed with an adult child) whose adult
/// child has a Home with room moves in on `kin_take_in_p` (the child of
/// the lowest id).
fn kin(world: &mut World) {
    let cfg = world.config.ages.clone();
    let day = world.day();
    let elders: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&a| years(world, a) >= cfg.frail_from_years)
        .filter(|&a| !world.has::<Job>(a) && !world.has::<Sentence>(a) && world.gang_of(a).is_none())
        .collect();
    // OPEN-140: a corp exec works (no `Job`, an Hq): the take-in moved him
    // in with family and the relocation moved him back every midnight.
    let execs = crate::systems::classes::exec_set(world);
    for e in elders {
        if execs.contains(&e) || crate::systems::life::housed_now(world, e) {
            continue;
        }
        let Some(h) = kin_home(world, e) else { continue };
        if hash(world, PURPOSE_KIN, key(e), day) >= cfg.kin_take_in_p {
            continue;
        }
        move_in(world, e, h);
    }
}

/// [`kin`]'s target for `e`: the Home (with room) of an adult living child
/// of `e`'s housed elsewhere (the child of the lowest id); `None` while `e`
/// is already under a grown child's roof. Lives at work phase 6 (§ 4.4):
/// the retirement to kin reads it too.
pub fn kin_home(world: &World, e: EntityId) -> Option<EntityId> {
    let home = world.comp::<Household>(e).and_then(|h| h.home);
    let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, e)
        .into_iter()
        .filter(|&c| crate::systems::demography::is_adult(world, c) && crate::systems::law::living(world, c))
        .collect();
    // Already under a grown child's roof: stays.
    if home.is_some() && kids.iter().any(|&c| world.comp::<Household>(c).and_then(|h| h.home) == home) {
        return None;
    }
    kids.iter().copied().filter_map(|c| world.comp::<Household>(c).and_then(|h| h.home)).find(|&h| {
        Some(h) != home
            // The island fix round F2 (OPEN-189): a relative's plain Home.
            && crate::systems::beds::kin_ok(world, h)
            && world.comp::<Building>(h).is_some_and(|b| {
                b.kind == BuildingKind::Home
                    && !b.demolished
                    && world.residents_of(h).len() < usize::from(b.capacity.max(1))
            })
    })
}

/// `e` lives under an adult living child's roof (Lives at work phase 6,
/// § 4.4: a retired elder kept by kin is not sent back to work).
pub fn with_kin(world: &World, e: EntityId) -> bool {
    let Some(home) = world.comp::<Household>(e).and_then(|h| h.home) else { return false };
    crate::systems::demography::children_of_agent(world, e).into_iter().any(|c| {
        crate::systems::demography::is_adult(world, c)
            && crate::systems::law::living(world, c)
            && world.comp::<Household>(c).and_then(|h| h.home) == Some(home)
    })
}

/// [`kin`]'s take-in: `e` moves into `h` (a `Housed` event, `elders_taken_in`).
pub fn move_in(world: &mut World, e: EntityId, h: EntityId) {
    world.set_home(e, Some(h));
    ownership::jobs2_row(world).elders_taken_in += 1;
    let text = format!("{} moved in with family at {}", world.name_of(e), world.name_of(h));
    world.push_event(EventKind::Housed, &[e, h], text);
}

/// Review fix 4 (`careers::retire`): the retiree's co-resident spouse moves
/// into the same Home with him (a `Housed` event; not an elder's take-in,
/// so `elders_taken_in` counts the retiree only).
pub fn move_in_with(world: &mut World, e: EntityId, h: EntityId) {
    world.set_home(e, Some(h));
    let text = format!("{} moved in with family at {}", world.name_of(e), world.name_of(h));
    world.push_event(EventKind::Housed, &[e, h], text);
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Midnight (from `living::run`): the children's routines, the kin, the
/// prosthetics; the dead dropped from the maimed.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("ages::daily");
    children(world);
    kin(world);
    prosthetics(world);
    let gone: Vec<EntityId> =
        world.jobs2.ages.maimed.keys().copied().filter(|&a| !crate::systems::law::living(world, a)).collect();
    for a in gone {
        world.jobs2.ages.maimed.remove(&a);
    }
}

/// `World::new`, after the vice's venues: `schools` Schools per copy on
/// the Lots left (Mid and up), on the city's deed. No RNG, no coins.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    for _ in 0..world.seed_copies() {
        for _ in 0..world.config.ages.schools {
            crate::systems::vice::seed_venue(world, BuildingKind::School, None);
        }
    }
    seed_kin(world);
}

/// The seeded city has no grown children (every resident arrives an adult):
/// under the switch each seeded adult of `kin_from_years` or more (oldest
/// first, ties the lower id) on `seed_kin_p` (a pure hash) gets a Parent
/// edge to the first unclaimed adult 20 to 35 years younger in another
/// household (ascending by age, then id). No RNG.
fn seed_kin(world: &mut World) {
    let cfg = world.config.ages.clone();
    let mut adults: Vec<(u32, EntityId)> = world
        .citizens()
        .into_iter()
        .filter(|&a| crate::systems::demography::is_adult(world, a))
        .map(|a| (world.comp::<Identity>(a).map_or(0, |i| i.age_days), a))
        .collect();
    adults.sort_unstable();
    let year = crate::time::DAYS_PER_YEAR as u32;
    let mut claimed: std::collections::BTreeSet<EntityId> = std::collections::BTreeSet::new();
    let elders: Vec<(u32, EntityId)> =
        adults.iter().rev().copied().filter(|&(d, _)| d >= cfg.kin_from_years * year).collect();
    for (d, e) in elders {
        if claimed.contains(&e) || hash(world, PURPOSE_KIN ^ 0xff, key(e), 0) >= cfg.seed_kin_p {
            continue;
        }
        let (lo, hi) = (d.saturating_sub(35 * year), d.saturating_sub(20 * year));
        let home = world.comp::<Household>(e).and_then(|h| h.home);
        let start = adults.partition_point(|&(x, _)| x < lo);
        let pick = adults[start..]
            .iter()
            .take_while(|&&(x, _)| x <= hi)
            .map(|&(_, a)| a)
            .find(|&a| a != e && !claimed.contains(&a) && world.comp::<Household>(a).and_then(|h| h.home) != home);
        let Some(c) = pick else { continue };
        claimed.insert(c);
        let edge = world.edge_entry(e, c);
        edge.kind = crate::components::RelKind::Parent;
        edge.affinity = 0.5;
        edge.trust = 0.6;
    }
}

/// The day's end (`systems::stats`): the life stages' snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let mut by = [0u32; 5];
    for k in world.with::<Child>() {
        if let Some(c) = world.comp::<Child>(k) {
            by[c.routine as usize] += 1;
        }
    }
    let from = world.config.ages.frail_from_years;
    let (mut elders, mut employed, mut cots) = (0u32, 0u32, 0u32);
    for a in world.citizens() {
        if world.has::<Child>(a) || world.comp::<Identity>(a).is_none_or(|i| i.age_years() < from) {
            continue;
        }
        elders += 1;
        employed += u32::from(world.has::<Job>(a));
        let homeless = world.comp::<Household>(a).is_some_and(|h| h.home.is_none());
        if homeless && world.hotel_beds.get(&a).is_some_and(|&(b, _)| crate::systems::charity::is_cot_house(world, b)) {
            cots += 1;
        }
    }
    let maimed = world.jobs2.ages.maimed.len() as u32;
    let schools = world.buildings_of_kind(BuildingKind::School).len() as u32;
    let row = ownership::jobs2_row(world);
    row.ch_play = by[0];
    row.ch_school = by[1];
    row.ch_chores = by[2];
    row.ch_street = by[3];
    row.ch_camp = by[4];
    row.elders = elders;
    row.elders_employed = employed;
    row.elders_on_cots = cots;
    row.maimed = maimed;
    row.schools = schools;
}
