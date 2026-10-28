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
//!   takes it, the E38 path); on a weekday a child of `school_from_years`
//!   or more → `School` at the nearest School with room on its roll
//!   (`school_places_per_teacher` a teacher) within `school_trip_ticks` of
//!   the Home door, walked or by the El (round 4 T2, OPEN-232), whose fee
//!   (and the El's fares) the household can spare (the fee waived by the
//!   `public_school` lever) (the fee `Flow::Contract` to its owner,
//!   `edu_days`); why not is `Child.school_miss`; on a weekday a parent with a
//!   Pitch → `Chores` (at the pitch); else `Play`. At 18 the school days add
//!   `edu_gain` each to the education (`Schooling`), and a street childhood
//!   takes stealth up and lawfulness down.
//! - **The seeded children** (the pressure pass S6, `OPEN-224`): at the
//!   seed's end each couple sharing a Home has `seed_child_slots` chances at
//!   a child on `seed_child_p`, aged under 18 and kept when the younger
//!   parent was `seed_child_gap_years` old at the birth (a materialised
//!   Ambient couple brings its own the same way); pure hashes, no world draw.
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
    Building, BuildingKind, Child, Household, Identity, Job, Personality, Routine, SchoolMiss, Sentence, Skills, Slot,
    TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::Tick;

use crate::world::World;

const PURPOSE_BEG: u64 = 0x4147_0001;
const PURPOSE_THEFT: u64 = 0x4147_0002;
const PURPOSE_KIN: u64 = 0x4147_0003;
const PURPOSE_MAIM: u64 = 0x4147_0004;
/// The pressure pass S6 (OPEN-224): a seeded couple's child slot (kept on
/// `seed_child_p`), the child's age, and the child's own stream.
const PURPOSE_SEED_CHILD: u64 = 0x4147_0005;
const PURPOSE_SEED_AGE: u64 = 0x4147_0006;
const PURPOSE_SEED_STREAM: u64 = 0x4147_0007;

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

/// The pressure pass S6 (Dylan 2026-10-27, a hard rule): `id` is under
/// `protected_under_years` (18), or a `Child`: never killed, hurt or harmed,
/// never a victim, a hire, a vice worker or client, a bettor. Every site that
/// picks a victim or deals harm skips them in the pick (`World::kill_by`
/// refuses as the backstop and counts `harm_blocked`). Not under `on`.
pub fn is_protected(world: &World, id: EntityId) -> bool {
    if world.has::<Child>(id) {
        return true;
    }
    let under = world.config.ages.protected_under_years * crate::time::DAYS_PER_YEAR as u32;
    world.comp::<Identity>(id).is_some_and(|i| i.age_days < under)
}

/// Phase 7's state on `Jobs2State` (empty, and not saved, off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgesState {
    /// The maimed: their level (1-3) and the slot (none for a cancer).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub maimed: BTreeMap<EntityId, (u8, Option<Slot>)>,
    /// Round 4 T2 (OPEN-232): the children on each School's roll today (the
    /// midnight's pick), and the day it was taken: a teacher's shift at a
    /// School with none on it is no workday ([`teacher_idle`]).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub rolls: BTreeMap<EntityId, u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll_day: Option<u64>,
    /// Round 5 U1 (OPEN-232): the school days running each district (by
    /// index) has had `[ages] found_full_min` or more of its children
    /// turned away full; at `found_full_days` a School is founded there.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub full_run: BTreeMap<u8, u32>,
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

/// Round 4 T2 (OPEN-232): the school run from a Home door `from` to a
/// School door `to`: the walk (Manhattan at `[exec] move_ticks_full`), or
/// the El's structural estimate (`distance::travel`) when it is quicker;
/// `(ticks, by the El)`. The child rides on the fares its household pays
/// (`school_el_fares`), never on a dodge: no gate, no guard, no beating.
pub fn school_trip(world: &World, from: TilePos, to: TilePos) -> (Tick, bool) {
    let walk = Tick::from(from.manhattan(to)) * world.config.exec.move_ticks_full;
    let el = crate::systems::distance::travel(world, from, to);
    if el < walk {
        (el, true)
    } else {
        (walk, false)
    }
}

/// T2: the standing Schools within `[ages] school_trip_ticks` of `from`
/// by [`school_trip`], nearest first (ties the lower id): `(ticks, by the
/// El, School)`.
pub fn schools_in_reach(world: &World, from: TilePos) -> Vec<(Tick, bool, EntityId)> {
    let limit = world.config.ages.school_trip_ticks;
    let mut v: Vec<(Tick, bool, EntityId)> = world
        .buildings_of_kind(BuildingKind::School)
        .iter()
        .copied()
        .filter_map(|b| {
            let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict)?;
            let (t, el) = school_trip(world, from, bd.door);
            (t <= limit).then_some((t, el, b))
        })
        .collect();
    v.sort_by_key(|&(t, _, b)| (t, b));
    v
}

/// The nearest School within the trip limit of `from` (no roll, no fee:
/// the relocation's `commute::school_kept`).
pub(crate) fn school_near(world: &World, from: TilePos) -> Option<EntityId> {
    schools_in_reach(world, from).first().map(|&(_, _, b)| b)
}

/// T2: a School's roll: `school_places_per_teacher` for each living
/// teacher on its staff (a School with none takes no one).
pub fn roll_cap(world: &World, school: EntityId) -> u32 {
    let teachers = world
        .staff_of(school)
        .iter()
        .filter(|&&t| world.is_alive(t) && world.comp::<Job>(t).is_some_and(|j| j.employer == Some(school)))
        .count() as u32;
    teachers * world.config.ages.school_places_per_teacher
}

/// T2 (OPEN-232): `job` is a post at a School whose roll for `shift_key`'s
/// day is empty (a rest day, or no child in reach): no class, no
/// ClerkWork, no shift and no wage (`exec::routine::workday_of`, both
/// tiers). `false` before the first midnight's roll, for a shift on another
/// day than the roll's, or off.
pub fn teacher_idle(world: &World, job: &Job, shift_key: i64) -> bool {
    let Some(s) = job.employer else { return false };
    if !on(world) || world.jobs2.ages.roll_day.is_none_or(|d| d as i64 != shift_key) {
        return false;
    }
    if world.comp::<Building>(s).is_none_or(|b| b.kind != BuildingKind::School) {
        return false;
    }
    !world.jobs2.ages.rolls.contains_key(&s)
}

/// T2: tonight's pick for a school-age child at Home `h` (door `door`) on
/// a school day: the nearest School in reach with room on its roll whose
/// fee (and the El's fares past the walk) the household can spare, paid
/// here; else why not, with the School that turned it away or was too dear.
struct Pick {
    school: Option<EntityId>,
    miss: Option<SchoolMiss>,
}

fn pick_school(
    world: &mut World,
    head: Option<EntityId>,
    cands: &[(Tick, bool, EntityId)],
    rolls: &mut BTreeMap<EntityId, u32>,
    caps: &mut BTreeMap<EntityId, u32>,
) -> Pick {
    let cfg = &world.config.ages;
    let fee = if world.levers.public_school { 0 } else { cfg.school_fee };
    let el_fare = world.config.transit.fare * cfg.school_el_fares;
    let purse = head.map_or(0, |p| crate::systems::leisure::spendable(world, p));
    let mut miss = Pick { school: None, miss: Some(if cands.is_empty() { SchoolMiss::Far } else { SchoolMiss::Full }) };
    for &(_, el, s) in cands {
        let cap = *caps.entry(s).or_insert_with(|| roll_cap(world, s));
        let on_roll = rolls.get(&s).copied().unwrap_or(0);
        if on_roll >= cap {
            if miss.school.is_none() {
                miss.school = Some(s);
            }
            continue;
        }
        let cost = fee + if el { el_fare } else { 0 };
        if cost > 0 && (head.is_none() || purse < cost) {
            if miss.miss != Some(SchoolMiss::Purse) {
                miss = Pick { school: Some(s), miss: Some(SchoolMiss::Purse) };
            }
            continue;
        }
        if el && el_fare > 0 {
            // The fares, the head's, to the Treasury as every fare.
            ownership::pay(world, head, None, el_fare, Flow::Fare);
            ownership::jobs2_row(world).school_el += 1;
        }
        if fee > 0 {
            let owner = world.owner_of(s);
            let moved = ownership::pay(world, head, owner, fee, Flow::Contract);
            ownership::credit(world, s, moved);
            ownership::jobs2_row(world).school_fees += moved;
        }
        *rolls.entry(s).or_insert(0) += 1;
        return Pick { school: Some(s), miss: None };
    }
    miss
}

/// The household's payer: the living adult parent sharing the child's home
/// with the most to spare (round 4 T2: whichever parent holds the money
/// pays; S6 read the lower id's purse alone), ties the lower id; else any
/// adult resident of it (the lowest id).
fn head_of(world: &World, child: EntityId, home: EntityId) -> Option<EntityId> {
    let parents = crate::systems::camp::parents_of(world, child);
    parents
        .into_iter()
        .filter(|&p| world.comp::<Household>(p).and_then(|h| h.home) == Some(home))
        .map(|p| (std::cmp::Reverse(crate::systems::leisure::spendable(world, p)), p))
        .min()
        .map(|(_, p)| p)
        .or_else(|| {
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
        // T2 (OPEN-232): the School whose roll it is on (a routine set by
        // hand, as a probe's, reads the nearest standing School by the trip).
        Routine::School => match c.school {
            Some(s) => Place::School(s),
            None => {
                let from = home.and_then(|h| world.comp::<Building>(h)).map(|b| b.door)?;
                let s = world
                    .buildings_of_kind(BuildingKind::School)
                    .iter()
                    .copied()
                    .filter_map(|b| {
                        let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict)?;
                        Some((school_trip(world, from, bd.door).0, b))
                    })
                    .min()?;
                Place::School(s.1)
            }
        },
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

/// S6 (OPEN-224): a School child is out of its Home now (its routine is
/// `School` and the hour is inside `school_hours`); `household::home_term`
/// leaves it out of the company at home.
pub fn at_school_now(world: &World, c: &Child) -> bool {
    if c.routine != Routine::School {
        return false;
    }
    let [from, to] = world.config.ages.school_hours;
    let tod = world.tick_of_day();
    (from..to).contains(&tod)
}

/// S6 (OPEN-224): the children's food a Home's pantry keeps: `ceil(
/// CHILD_FOOD_PER_DAY × the children at home × child_pantry_days)` (a Camp
/// or Street child eats elsewhere); 0 off.
pub fn children_food_need(world: &World, home: EntityId) -> u32 {
    if !on(world) {
        return 0;
    }
    let kids = world
        .residents_of(home)
        .iter()
        .filter(|&&r| world.comp::<Child>(r).is_some_and(|c| !matches!(c.routine, Routine::Camp | Routine::Street)))
        .count() as f32;
    (crate::systems::demography::CHILD_FOOD_PER_DAY * kids * world.config.ages.child_pantry_days).ceil() as u32
}

/// S6: `id` is a parent with a child of theirs living in their Home (the
/// jobless parent's grocery run, `routine::grocery_plan`, `lod`'s mirror).
pub fn feeds_children(world: &World, id: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else { return false };
    crate::systems::demography::children_of_agent(world, id)
        .into_iter()
        .any(|c| world.has::<Child>(c) && world.comp::<Household>(c).and_then(|h| h.home) == Some(home))
}

/// Midnight: each child's routine and its day's effect.
fn children(world: &mut World) {
    let cfg = world.config.ages.clone();
    let day = world.day();
    let wd = weekday(world);
    let kids: Vec<EntityId> = world.with::<Child>();
    let mut takes: Vec<EntityId> = Vec::new();
    // Round 4 T2 (OPEN-232): tonight's rolls fill in child order; a Home's
    // Schools in reach are read once.
    let mut rolls: BTreeMap<EntityId, u32> = BTreeMap::new();
    let mut caps: BTreeMap<EntityId, u32> = BTreeMap::new();
    let mut reach: BTreeMap<EntityId, Vec<(Tick, bool, EntityId)>> = BTreeMap::new();
    // Round 5 U1: the children turned away full tonight, by their Home's district.
    let mut full: FullDay = BTreeMap::new();
    for k in kids {
        // A child has no Brain (`law::living` reads one): alive, not a corpse.
        if !world.is_alive(k) || world.has::<crate::components::Corpse>(k) {
            continue;
        }
        let home = world.comp::<Household>(k).and_then(|h| h.home);
        let in_camp = home.is_some_and(|h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp));
        let mut pick = Pick { school: None, miss: None };
        let routine = if in_camp {
            Routine::Camp
        } else if let Some(h) = home {
            let door = world.comp::<Building>(h).map(|b| b.door);
            // S6 (OPEN-224): School from `school_from_years` (an infant on
            // a paid fee was a "pupil" before).
            let school_age = years(world, k) >= cfg.school_from_years;
            let head = head_of(world, k, h);
            if let (true, true, Some(d)) = (wd, school_age, door) {
                let cands = reach.entry(h).or_insert_with(|| schools_in_reach(world, d)).clone();
                pick = pick_school(world, head, &cands, &mut rolls, &mut caps);
                if pick.miss == Some(SchoolMiss::Full) {
                    let e = full.entry(world.district_of(d).0).or_default();
                    e.0.push(d);
                    e.1.extend(cands.iter().map(|c| c.2));
                    if let Some(s) = pick.school {
                        *e.2.entry(s).or_insert(0) += 1;
                    }
                }
            }
            let paid = pick.school.is_some() && pick.miss.is_none();
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
            c.school = pick.school;
            c.school_miss = pick.miss;
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
        match pick.miss {
            Some(SchoolMiss::Far) => row.school_far += 1,
            Some(SchoolMiss::Full) => row.school_full += 1,
            Some(SchoolMiss::Purse) => row.school_unpaid += 1,
            None => {}
        }
        if routine == Routine::School {
            row.school_days += 1;
        }
    }
    for k in takes {
        if crate::systems::camp::take(world, k).is_some() {
            ownership::jobs2_row(world).street_takes += 1;
        }
    }
    // T2: a School with teachers and no child on a school day's roll says so
    // (the teachers have no class, no shift and no wage today).
    if wd {
        let idle: Vec<EntityId> = world
            .buildings_of_kind(BuildingKind::School)
            .iter()
            .copied()
            .filter(|s| !rolls.contains_key(s) && world.comp::<Building>(*s).is_some_and(|b| !b.demolished))
            .collect();
        for s in idle {
            let staff: Vec<EntityId> = world
                .staff_of(s)
                .iter()
                .copied()
                .filter(|&t| world.is_alive(t) && world.comp::<Job>(t).is_some_and(|j| j.employer == Some(s)))
                .collect();
            if staff.is_empty() {
                continue;
            }
            let names: Vec<String> = staff.iter().map(|&t| world.name_of(t)).collect();
            let text = format!(
                "No pupil on the roll at {} today: no class for {} (no shift, no wage)",
                world.name_of(s),
                names.join(", ")
            );
            let mut who = staff;
            who.push(s);
            world.push_event(EventKind::Served, &who, text);
        }
    }
    world.jobs2.ages.rolls = rolls;
    world.jobs2.ages.roll_day = Some(day);
    if wd {
        found_on_demand(world, full);
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
    let kids = bring_children(world, e, h);
    ownership::jobs2_row(world).elders_taken_in += 1;
    let text = format!("{} moved in with family at {}{kids}", world.name_of(e), world.name_of(h));
    world.push_event(EventKind::Housed, &[e, h], text);
}

/// Round 5 U2 (OPEN-231): `e` moves into `h` and his children living
/// under his old roof come with him (`camp::send_home`: dependants, never
/// counted against the flat). An elder taken in by a grown child, or a
/// retiree's spouse, left the young ones behind: 54 children on day 1 and
/// ~6 a day after at 20k, unfed at a Block with no parent and on the
/// service's ladder. The line's tail names them.
fn bring_children(world: &mut World, e: EntityId, h: EntityId) -> String {
    let old = world.comp::<Household>(e).and_then(|x| x.home);
    world.set_home(e, Some(h));
    let Some(old) = old.filter(|&o| o != h) else { return String::new() };
    let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, e)
        .into_iter()
        .filter(|&c| world.has::<Child>(c) && world.comp::<Household>(c).and_then(|x| x.home) == Some(old))
        .collect();
    let mut names = Vec::new();
    for c in kids {
        if crate::systems::camp::send_home(world, c, e).is_some() {
            names.push(world.name_of(c));
        }
    }
    if names.is_empty() {
        String::new()
    } else {
        format!(", with {}", names.join(", "))
    }
}

/// Review fix 4 (`careers::retire`): the retiree's co-resident spouse moves
/// into the same Home with him (a `Housed` event; not an elder's take-in,
/// so `elders_taken_in` counts the retiree only).
pub fn move_in_with(world: &mut World, e: EntityId, h: EntityId) {
    let kids = bring_children(world, e, h);
    let text = format!("{} moved in with family at {}{kids}", world.name_of(e), world.name_of(h));
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
    let n = world.lot_copies() * world.config.ages.schools as usize;
    let mut homes = HomeGrid::new(world);
    for _ in 0..n {
        let Some(lot) = school_lot(world, &homes) else { break };
        if crate::systems::vice::seed_venue_on(world, lot, BuildingKind::School, None).is_none() {
            break;
        }
        if let Some(d) = world.comp::<Building>(lot).map(|b| b.door) {
            homes.cover(d, u32::MAX);
        }
    }
    seed_kin(world);
}

/// Round 4 T2 (OPEN-232): the seed's Homes (door, residents) in cells of
/// `HOME_CELL` tiles, for the Schools by reach. A Home is covered once a
/// seeded School's door lies within the walk of `school_trip_ticks`.
/// Round 5 U1: the weight is any count by door (the school-age children,
/// or the children turned away full), and a School covers up to its seats,
/// nearest first (T2's residents: every Home in the walk, uncapped).
struct HomeGrid {
    radius: u32,
    cells: HomeCells,
}

const HOME_CELL: u32 = 8;

/// A cell of the grid: its Homes (door, the weight not yet covered).
type HomeCells = BTreeMap<(u32, u32), Vec<(TilePos, u32)>>;

impl HomeGrid {
    fn new(world: &World) -> Self {
        let homes = world.buildings_of_kind(BuildingKind::Home).iter().filter_map(|&h| {
            let bd = world.comp::<Building>(h).filter(|b| !b.demolished)?;
            Some((bd.door, world.residents_of(h).len() as u32))
        });
        Self::of(world, homes)
    }

    /// U1: the grid of `(door, weight)` (a door may repeat).
    fn of(world: &World, doors: impl IntoIterator<Item = (TilePos, u32)>) -> Self {
        let step = world.config.exec.move_ticks_full.max(1);
        let radius = (world.config.ages.school_trip_ticks / step).min(u64::from(u32::MAX / 4)) as u32;
        let mut cells: HomeCells = BTreeMap::new();
        for (d, n) in doors {
            if n == 0 {
                continue;
            }
            cells.entry((u32::from(d.x) / HOME_CELL, u32::from(d.y) / HOME_CELL)).or_default().push((d, n));
        }
        HomeGrid { radius, cells }
    }

    /// The cells' column range and row range around `at`.
    fn span(&self, at: TilePos) -> ([u32; 2], [u32; 2]) {
        let r = self.radius;
        let (x, y) = (u32::from(at.x), u32::from(at.y));
        (
            [x.saturating_sub(r) / HOME_CELL, x.saturating_add(r) / HOME_CELL],
            [y.saturating_sub(r) / HOME_CELL, y.saturating_add(r) / HOME_CELL],
        )
    }

    /// The uncovered residents within the walk radius of `at`.
    fn count(&self, at: TilePos) -> u32 {
        let ([x0, x1], [y0, y1]) = self.span(at);
        let mut sum = 0;
        for cx in x0..=x1 {
            for (_, v) in self.cells.range((cx, y0)..=(cx, y1)) {
                sum += v.iter().filter(|e| e.0.manhattan(at) <= self.radius).map(|e| e.1).sum::<u32>();
            }
        }
        sum
    }

    /// Covers up to `seats` of the weight within the walk radius of `at`,
    /// nearest door first (ties the cell order); `u32::MAX`: all of it.
    fn cover(&mut self, at: TilePos, seats: u32) {
        let ([x0, x1], [y0, y1]) = self.span(at);
        let r = self.radius;
        let mut near: Vec<(u32, (u32, u32), usize)> = Vec::new();
        for cx in x0..=x1 {
            for (&k, v) in self.cells.range((cx, y0)..=(cx, y1)) {
                for (i, e) in v.iter().enumerate() {
                    let d = e.0.manhattan(at);
                    if d <= r && e.1 > 0 {
                        near.push((d, k, i));
                    }
                }
            }
        }
        near.sort_unstable();
        let mut left = seats;
        for (_, k, i) in near {
            if left == 0 {
                break;
            }
            let Some(e) = self.cells.get_mut(&k).and_then(|v| v.get_mut(i)) else { continue };
            let take = e.1.min(left);
            e.1 -= take;
            left -= take;
        }
    }
}

/// T2: the next seeded School's Lot: of `jobs::seed_lots` (the tier, the
/// interior), the one whose door has the most uncovered Home residents
/// within the walk of `school_trip_ticks`, then `seed_lot`'s order (fewest
/// in the district, nearest its centroid, the lower id). S6's seed took the
/// centroid each time: 20 Schools at five centroids, 15 % of the island's
/// school-age children within an hour of one.
fn school_lot(world: &World, homes: &HomeGrid) -> Option<EntityId> {
    if !world.config.ages.school_by_reach {
        return crate::systems::jobs::seed_lot(world, BuildingKind::School);
    }
    crate::systems::jobs::seed_lots(world, BuildingKind::School)
        .into_iter()
        .filter_map(|(copies, centroid, l)| {
            let door = world.comp::<Building>(l)?.door;
            Some((std::cmp::Reverse(homes.count(door)), copies, centroid, l))
        })
        .min()
        .map(|(_, _, _, l)| l)
}

/// Round 5 U1 (OPEN-232): where the next School stands for the weight in
/// `grid` (children by Home door): of the vacant Lots of `jobs::seed_lots`
/// (the tier, the interior) and the derelict Blocks of a School's tier
/// (refitted, as `jobs::seed_derelict`'s are), the one whose door has the
/// most of it within the walk (up to `seats`, then the most; a Lot before
/// a Block, then `seed_lot`'s order, the lower id). `None` when no site
/// reaches any of it. `(site, refit)`.
fn school_site(world: &World, grid: &HomeGrid, seats: u32) -> Option<(EntityId, bool)> {
    use std::cmp::Reverse;
    let lot = crate::systems::jobs::seed_lots(world, BuildingKind::School)
        .into_iter()
        .filter_map(|(copies, centroid, l)| {
            let n = grid.count(world.comp::<Building>(l)?.door);
            (n > 0).then_some((Reverse(n.min(seats)), Reverse(n), copies, centroid, l))
        })
        .min()
        .map(|t| ((t.0, t.1), t.4));
    let block = crate::systems::street::derelicts(world)
        .into_iter()
        .filter_map(|d| {
            let bd = world.comp::<Building>(d)?;
            if bd.kind != BuildingKind::Home
                || bd.demolished
                || !crate::systems::founding::tier_ok(BuildingKind::School, bd.tier)
            {
                return None;
            }
            let n = grid.count(bd.door);
            (n > 0).then_some((Reverse(n.min(seats)), Reverse(n), d))
        })
        .min()
        .map(|t| ((t.0, t.1), t.2));
    match (lot, block) {
        (Some(l), Some(b)) if b.0 < l.0 => Some((b.1, true)),
        (Some(l), _) => Some((l.1, false)),
        (None, b) => b.map(|b| (b.1, true)),
    }
}

/// U1: a School on `site` on the city's deed, unpaid as the seed's (a
/// refit's squatters put out, as every refit's); its teacher posts open as
/// vacancies (`founding::convert`), the hire staffs it.
fn open_school(world: &mut World, site: EntityId, refit: bool) -> Option<EntityId> {
    let built = if refit {
        crate::systems::founding::refit_with(world, site, BuildingKind::School, None, false)
    } else {
        crate::systems::founding::build_on_lot(world, site, BuildingKind::School, None)
    };
    built.ok()
}

fn district_name(world: &World, b: EntityId) -> String {
    let d = world.district_of_building(b);
    world.districts.get(d.index()).map_or_else(|| "?".to_string(), |x| x.name.clone())
}

/// U1: the school-age children (`school_from_years` and up, not in a Camp)
/// by their Home's door.
fn school_age_doors(world: &World) -> Vec<(TilePos, u32)> {
    let from = world.config.ages.school_from_years;
    let mut by: BTreeMap<EntityId, u32> = BTreeMap::new();
    for k in world.with::<Child>() {
        if !world.is_alive(k) || years(world, k) < from {
            continue;
        }
        let Some(h) = world.comp::<Household>(k).and_then(|h| h.home) else { continue };
        if world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Home && !b.demolished) {
            *by.entry(h).or_insert(0) += 1;
        }
    }
    by.into_iter().filter_map(|(h, n)| Some((world.comp::<Building>(h)?.door, n))).collect()
}

/// Round 5 U1 (OPEN-232, Dylan 2026-10-28: Schools by the child count):
/// `World::new`, after `seed_children`: the Schools topped up to one for
/// each `school_age_per_school` school-age children (T2's per-copy seed
/// the floor). The standing Schools cover their rolls of the nearest
/// children first; each new one stands where the most are left
/// ([`school_site`]); its teacher posts open as vacancies, filled by the
/// first night's hire (a job search here hired into every open post of the
/// seed: the probes' staffing moved). No RNG, no coins. Returns the Schools
/// built.
pub fn seed_by_children(world: &mut World) -> u32 {
    let per = world.config.ages.school_age_per_school;
    if !on(world) || per == 0 {
        return 0;
    }
    let doors = school_age_doors(world);
    let target = doors.iter().map(|d| d.1).sum::<u32>().div_ceil(per) as usize;
    let standing: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::School)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect();
    let mut grid = HomeGrid::of(world, doors);
    for &b in &standing {
        let Some(d) = world.comp::<Building>(b).map(|bd| bd.door) else { continue };
        let cap = roll_cap(world, b);
        grid.cover(d, if cap > 0 { cap } else { per });
    }
    let mut built = 0usize;
    while standing.len() + built < target {
        let Some((site, refit)) = school_site(world, &grid, per) else { break };
        let Some(door) = world.comp::<Building>(site).map(|b| b.door) else { break };
        let near = grid.count(door);
        let Some(s) = open_school(world, site, refit) else { break };
        grid.cover(door, per);
        let how = if refit {
            format!("a derelict Block refitted, {}", world.name_of(s))
        } else {
            format!("Lot {}", site.index)
        };
        let text = format!(
            "The city opened a School in {} (seeded for the children: {near} school-age without a seat within the walk; {how})",
            district_name(world, s)
        );
        world.push_event(EventKind::Founded, &[s], text);
        built += 1;
    }
    built as u32
}

/// U1: a school day's children turned away full, by their Home's district:
/// their Home doors, the Schools in their reach, the count each full School
/// turned away.
type FullDay = BTreeMap<u8, (Vec<TilePos>, std::collections::BTreeSet<EntityId>, BTreeMap<EntityId, u32>)>;

/// U1 (OPEN-232, Dylan 2026-10-28: Schools on demand), a school day's
/// midnight after the pick: a district with `found_full_min` or more of
/// its children turned away full on `found_full_days` school days running
/// gets a School on the city's deed where most of them are within the
/// walk ([`school_site`]), staffed by the hire; not while a School in
/// their reach still waits for its teachers (a roll of 0).
fn found_on_demand(world: &mut World, full: FullDay) {
    let (days, min, per) =
        (world.config.ages.found_full_days, world.config.ages.found_full_min, world.config.ages.school_age_per_school);
    let old = std::mem::take(&mut world.jobs2.ages.full_run);
    if days == 0 {
        return;
    }
    let mut run: BTreeMap<u8, u32> = BTreeMap::new();
    for (d, (doors, near, by)) in full {
        let turned = doors.len() as u32;
        if turned < min.max(1) {
            continue;
        }
        let n = old.get(&d).copied().unwrap_or(0) + 1;
        if n < days || near.iter().any(|&s| roll_cap(world, s) == 0) {
            run.insert(d, n);
            continue;
        }
        let grid = HomeGrid::of(world, doors.iter().map(|&p| (p, 1)));
        let Some((site, refit)) = school_site(world, &grid, per.max(1)) else { continue };
        let Some(s) = open_school(world, site, refit) else { continue };
        let full_school = by.iter().max_by_key(|&(&b, &c)| (c, std::cmp::Reverse(b))).map_or(s, |(&b, _)| b);
        let how = if refit {
            format!("a derelict Block refitted as {}", world.name_of(s))
        } else {
            format!("{} on Lot {}", world.name_of(s), site.index)
        };
        let text = format!(
            "The city opened a School in {} ({how}): {turned} of the district's children turned away full, {n} school days running ({} full); its teachers to hire",
            world.districts.get(usize::from(d)).map_or("?", |x| x.name.as_str()),
            world.name_of(full_school)
        );
        let mut who = vec![s];
        if full_school != s {
            who.push(full_school);
        }
        world.push_event(EventKind::Founded, &who, text);
        ownership::jobs2_row(world).schools_founded += 1;
    }
    world.jobs2.ages.full_run = run;
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
        world.note_kin(e, c);
    }
}

/// The pressure pass S6 (OPEN-224, `OPEN-150`'s starting condition):
/// `World::new`, last (the households settled: the pre-roll, the company
/// blocks): every couple sharing a Home gets its children
/// ([`children_for_couple`]). Pure hashes and keyed streams, no world draw.
/// Returns the children seeded.
pub fn seed_children(world: &mut World) -> u32 {
    if !on(world) || world.config.ages.seed_child_slots == 0 {
        return 0;
    }
    let pairs: Vec<(EntityId, EntityId)> = world.spouses.iter().filter(|(a, b)| a < b).map(|(&a, &b)| (a, b)).collect();
    let mut n = 0;
    for (a, b) in pairs {
        let ha = world.comp::<Household>(a).and_then(|h| h.home);
        let Some(home) = ha.filter(|_| ha == world.comp::<Household>(b).and_then(|h| h.home)) else { continue };
        n += children_for_couple(world, a, b, home, key(a) ^ key(b).rotate_left(17)).len() as u32;
    }
    n
}

/// S6: the children of the couple `a`, `b` in `home` (a Home), keyed on
/// `k`: `seed_child_slots` slots, each kept on `seed_child_p`, aged
/// uniformly under 18 and kept when the younger parent was
/// `seed_child_gap_years` old at the birth; oldest first, spawned as a
/// migrant's child is (`demography::spawn_child_aged`). The seed's couples
/// and a materialised Ambient couple (`ambient::materialise`).
pub fn children_for_couple(world: &mut World, a: EntityId, b: EntityId, home: EntityId, k: u64) -> Vec<EntityId> {
    let cfg = world.config.ages.clone();
    if !on(world) || cfg.seed_child_slots == 0 {
        return Vec::new();
    }
    if world.comp::<Building>(home).is_none_or(|bd| bd.kind != BuildingKind::Home || bd.demolished) {
        return Vec::new();
    }
    let year = crate::time::DAYS_PER_YEAR as f32;
    let adult = crate::systems::demography::ADULT_AGE_DAYS;
    let younger = |w: &World| {
        let d = |x: EntityId| w.comp::<Identity>(x).map_or(0, |i| i.age_days);
        d(a).min(d(b)) as f32 / year
    };
    let parent = younger(world);
    let mut ages: Vec<u32> = (0..cfg.seed_child_slots)
        .filter(|&i| hash(world, PURPOSE_SEED_CHILD, k, u64::from(i)) < cfg.seed_child_p)
        .filter_map(|i| {
            let years = hash(world, PURPOSE_SEED_AGE, k, u64::from(i)) * (adult as f32 / year);
            let gap = parent - years;
            (gap >= cfg.seed_child_gap_years[0] && gap <= cfg.seed_child_gap_years[1])
                .then(|| ((years * year) as u32).min(adult - 1))
        })
        .collect();
    ages.sort_unstable_by(|x, y| y.cmp(x));
    let female =
        |w: &World, x: EntityId| w.comp::<Identity>(x).is_some_and(|i| i.sex == crate::components::Sex::Female);
    let (mother, father) = if female(world, b) && !female(world, a) { (b, a) } else { (a, b) };
    let mut out = Vec::new();
    for (i, age) in ages.into_iter().enumerate() {
        let mut r = world.rng.keyed(PURPOSE_SEED_STREAM ^ k.rotate_left(7) ^ ((i as u64) << 56));
        out.push(crate::systems::demography::spawn_child_aged(world, mother, father, home, age, &mut r));
    }
    out
}

/// The day's end (`systems::stats`): the life stages' snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let mut by = [0u32; 5];
    // Round 6 V1 (OPEN-231): a warning standing past `warn_stale_days`.
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    let (stale, window) = (u32::from(world.config.camp.warn_stale_days), u32::from(world.config.camp.warn_repeat_days));
    let mut warned_old = 0u32;
    for k in world.with::<Child>() {
        if let Some(c) = world.comp::<Child>(k) {
            by[c.routine as usize] += 1;
            let age = c.warned_day.map(|w| day.saturating_sub(w));
            if c.routine != crate::components::Routine::Camp && age.is_some_and(|a| a > stale && a <= window) {
                warned_old += 1;
            }
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
    row.ch_warned_old = warned_old;
    row.elders = elders;
    row.elders_employed = employed;
    row.elders_on_cots = cots;
    row.maimed = maimed;
    row.schools = schools;
}
