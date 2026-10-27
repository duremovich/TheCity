//! Jobs & lives phase 2, the street (docs/JOBS_AND_LIVES.md § 3, § 1's
//! Pitch, D3): Pitches (vendors, rickshaws, tinkers), the labour corner and
//! its day posts, couriers, cabs and rickshaws, tinkers, the gang's street
//! cut and the sweeps. Under `[jobs2] street` (itself under `[jobs2]
//! enabled`): off, every entry point is a no-op and no state is written.
//!
//! - **Pitches** (the overflow rung under the workplace room cap): at
//!   midnight a jobless adult holding `pitch_stake` coins, `pitch_after_days`
//!   without a post, opens one on `p_pitch_open` where the district is under
//!   its per-capita cap (a world-side choice, every tier: D17). A body
//!   stands it through its hours (`GoalKind::Pitch`, the `Vend` step); off
//!   screen the owner counts as there. It closes on a hire, a jail or
//!   `pitch_fail_days` without a take; the holding gang takes `cut_frac`
//!   of each day's take (`Flow::StreetCut`); a `Sweep` district fines it.
//! - **Vendors** (`resale`): at the shift's first hour, `vendor_batch` shelf
//!   units of synth from the nearest Market (`Flow::Wholesale`); an
//!   off-screen synth buyer in the district takes a meal from an open stall
//!   on `p_vendor_pick` at `vendor_markup` (`lod::stat_eat`'s Market leg).
//! - **The labour corner**: outside each district's first Yard. At
//!   `corner_hour` the day's posts (a Construction Yard's open sites, a
//!   Freight Yard's backlog, the Farms in the harvest seasons) are filled
//!   from the broke jobless standing there (a body within `corner_tiles`;
//!   off screen, living within `corner_stat_reach`, on `p_corner`), the
//!   employer's foreman's known contacts first, then the strongest
//!   (`fighting`). A day post (`Job.until`) ends after its shift, paid as
//!   any post at its end (`industry::end_day_posts`).
//! - **Couriers**: a corp with two or more buildings sends
//!   `parcels_per_building` a day through the nearest Freight Yard whose
//!   couriers can carry them (`parcel_fee` each, `Flow::Handling`); the
//!   couriers' round walks the senders' buildings near the Yard.
//! - **Cabs and rickshaws**: off screen, a commuter of `cab_min_tiles` or
//!   more with the fare and `cab_reserve` takes a ride on `p_cab` in the
//!   shift's first hour: a rickshaw Pitch of the home district first, else
//!   a Garage's cab drivers (`Flow::CabFare`).
//! - **Tinkers**: an agent's vehicle due a repair near an open tinker is
//!   fixed at `tinker_discount` of the Garage's price, or wrecked on
//!   `tinker_fail_p` (`assets::repairs`).
//!
//! Every coin moves between purses; nothing is minted. Hourly work is
//! O(pitches + yards); the dawn pick and the midnight pass are O(adults)
//! once a day.

use crate::components::ActionInstance;
use crate::components::{
    Brain, Building, BuildingKind, Corp, DistrictId, GoalKind, Job, Lod, Personality, Pitch, Position, Role, Sentence,
    Skills, TilePos, TradeId, Wallet,
};
use crate::config::{TradeCfg, TradeDuty, TradeModel, TradeSkill};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, urgency, Curve, GATE, IDENTITY};
use crate::utility::Consideration;
use crate::world::World;
use serde::{Deserialize, Serialize};

const PURPOSE_OPEN: u64 = 0x5713_0001;
const PURPOSE_PICK: u64 = 0x5713_0002;
/// The pressure pass phase 2: a body's hail (one a day).
const PURPOSE_HAIL: u64 = 0x5713_00a1;
const PURPOSE_CORNER: u64 = 0x5713_0003;
const PURPOSE_STREET: u64 = 0x5713_00F1;
const PURPOSE_CAB: u64 = 0x5713_0004;
/// R6a: the ride home's roll keys the day apart from the commute's.
const PURPOSE_CAB_HOME: u64 = 0x5713_00c4;
const PURPOSE_TINKER: u64 = 0x5713_0005;
const PURPOSE_SWEEP: u64 = 0x5713_0006;
const PURPOSE_STAT_PITCH: u64 = 0x5713_0007;
/// Lives at work phase 4 (§ 2.3): the driver a booked ride goes to (D6).
const PURPOSE_FARE: u64 = 0x5713_0008;
/// Lives at work phase 4 (§ 2.3): a parcel's sender, recipient building and
/// recipient (one draw each, keyed by the yard, the day and the parcel).
const PURPOSE_PARCEL: u64 = 0x5713_0009;
/// Round 3 S5: the cab's rank for the wait.
const PURPOSE_RANK: u64 = 0x5713_000a;

/// Lives at work phase 4 (§ 2.3): who takes a booked ride.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RideBy {
    /// A working rickshaw Pitch of the rider's home district (its owner).
    Rickshaw(EntityId),
    /// The Garage nearest home with drivers and rides left.
    Cab(EntityId),
}

/// Lives at work phase 4 (§ 2.3): the cab roll's outcome, one source for
/// both tiers (`cab_roll`; the coins move in `pay_ride`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ride {
    pub from: TilePos,
    pub to: TilePos,
    pub fare: i64,
    pub by: RideBy,
}

/// Lives at work phase 4 (§ 2.3): a booked ride a driver's body drives
/// (`Jobs2State.cab_rides`, saved while pending, cleared at midnight):
/// stage 0 to the rider's door, 1 to the rider's work door, then the Garage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CabRide {
    pub rider: EntityId,
    pub from: TilePos,
    pub to: TilePos,
    pub driver: EntityId,
    pub fare: i64,
    pub tick: Tick,
    pub stage: u8,
    /// The pre-view pass phase 5 (the `fares` shape): the rider's body waits
    /// at `from` and rides along (`Jobs2State.passengers`); paid on arrival.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub body: bool,
}

/// Lives at work phase 4 (§ 2.3): a parcel, a named item (not an asset):
/// from the sender building to `to`, a staff member of `at`, a recipient
/// building of the same corp, through a Yard (`Jobs2State.parcels`, saved
/// while pending, cleared at midnight).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parcel {
    pub from: EntityId,
    pub to: EntityId,
    pub at: EntityId,
    pub yard: EntityId,
    pub fee: i64,
    pub day: u64,
}

/// The street's sub-switch (`[jobs2] street`, under the master switch).
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.jobs2.street
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

// ---------------------------------------------------------------------------
// The Pitch trades
// ---------------------------------------------------------------------------

/// What a Pitch row works (from its duty and skill: no row is keyed by name).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PitchKind {
    Vendor,
    Rickshaw,
    Tinker,
    /// Jobs & lives phase 4 (§ 5.1): a `vice` row's night pitch.
    Street,
}

/// A self-employed row (no workplace) and what it works.
pub fn pitch_kind(row: &TradeCfg) -> Option<PitchKind> {
    if !row.workplace.is_empty() {
        return None;
    }
    if row.vice {
        return Some(PitchKind::Street);
    }
    match row.duty {
        TradeDuty::Sell => Some(PitchKind::Vendor),
        TradeDuty::Fare => Some(PitchKind::Rickshaw),
        TradeDuty::Post if row.skill == TradeSkill::Knowledge => Some(PitchKind::Tinker),
        _ => None,
    }
}

/// The first live row of `kind`.
fn pitch_row(world: &World, kind: PitchKind) -> Option<TradeId> {
    world.config.trade_roles().into_iter().enumerate().find_map(|(i, r)| {
        let row = world.config.trades.get(i)?;
        (pitch_kind(row) == Some(kind) && crate::systems::classpay::trade_live(world, row)).then(|| r.trade())?
    })
}

fn kind_of_pitch(world: &World, p: &Pitch) -> Option<PitchKind> {
    world.config.trade(p.trade).and_then(pitch_kind)
}

/// The `day` model's row (the labour corner's posts).
fn day_role(world: &World) -> Option<Role> {
    world.config.trade_roles().into_iter().enumerate().find_map(|(i, r)| {
        let row = world.config.trades.get(i)?;
        (row.model == TradeModel::Day && crate::systems::classpay::trade_live(world, row)).then_some(r)
    })
}

/// A Pitch's hours now: its row's shift for its owner, not swept.
fn in_hours(world: &World, p: &Pitch) -> bool {
    if p.closed_until.is_some_and(|t| t > world.tick) {
        return false;
    }
    let Some(row) = world.config.trade(p.trade) else { return false };
    let tod = world.tick_of_day();
    crate::systems::trades::shifts_for(world, row, p.owner).iter().any(|&(s, e)| tod >= s && tod < e)
}

/// A pitch-hour due: in its hours, the owner free and well (`calibrate`'s
/// `p_pitch` denominator).
/// R6a: was the pitch in its hours at the time of day `tod` (its owner
/// free and well now)? The pimp's hour at an off-screen worker's pitch
/// counts from its start.
pub fn due_at(world: &World, p: &Pitch, tod: u16) -> bool {
    let Some(row) = world.config.trade(p.trade) else { return false };
    p.closed_until.is_none_or(|t| t <= world.tick)
        && crate::systems::trades::shifts_for(world, row, p.owner).iter().any(|&(s, e)| tod >= s && tod < e)
        && !world.has::<Sentence>(p.owner)
        && !crate::systems::health::sick(world, p.owner)
}

pub fn due(world: &World, p: &Pitch) -> bool {
    in_hours(world, p) && !world.has::<Sentence>(p.owner) && !crate::systems::health::sick(world, p.owner)
}

/// Is the owner at his Pitch now (in its hours, free, well; a body within
/// `pitch_reach` of the spot; off screen on the table's `p_pitch` for the
/// hour, every hour with a table without it)?
pub fn working(world: &World, p: &Pitch) -> bool {
    if !due(world, p) {
        return false;
    }
    match world.comp::<Brain>(p.owner) {
        Some(b) if b.lod == Lod::Statistical => world.stat_table.as_ref().and_then(|t| t.p_pitch).is_none_or(|q| {
            let hour = world.tick / crate::time::TICKS_PER_HOUR;
            crate::systems::econ::hash_unit(world.seed(), PURPOSE_STAT_PITCH, u64::from(p.owner.index), hour) < q
        }),
        Some(_) => world
            .comp::<Position>(p.owner)
            .is_some_and(|pos| pos.building.is_none() && pos.tile.manhattan(p.spot) <= world.config.jobs2.pitch_reach),
        None => false,
    }
}

fn district_of_agent(world: &World, id: EntityId) -> Option<DistrictId> {
    crate::systems::contracts::home_door(world, id)
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
        .map(|t| world.district_of(t))
}

/// A free adult with a Brain (not a child, not held, not leaving).
fn free_adult(world: &World, id: EntityId) -> bool {
    crate::systems::demography::is_adult(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Brain>(id).is_some_and(|b| !b.emigrating)
}

fn coins(world: &World, id: EntityId) -> i64 {
    world.comp::<Wallet>(id).map_or(0, |w| w.coins)
}

// ---------------------------------------------------------------------------
// The midnight pass: idle days, the cut, sweeps, closings, openings
// ---------------------------------------------------------------------------

/// Midnight (from `living::run`): the jobless' idle days, each Pitch's
/// cut, sweep and day roll, the closings, the openings, the couriers'
/// parcels and the cabs' new day.
pub fn daily(world: &mut World) {
    // Lives at work phase 4 (§ 2.3): yesterday's rides, rolls and parcels
    // are done (empty off: nothing moves).
    release_all(world);
    world.jobs2.cab_rides.clear();
    world.jobs2.cab_rolled.clear();
    world.jobs2.exec_home_rolled.clear();
    world.jobs2.hailed.clear();
    world.jobs2.fares_shift.clear();
    world.jobs2.fare_cuts.clear();
    // A parcel no courier's round reached went back to the Yard.
    let left = std::mem::take(&mut world.jobs2.parcels).len() as u32;
    if left > 0 {
        ownership::jobs2_row(world).parcels_returned += left;
    }
    if !on(world) || !crate::systems::midnight::due(world, crate::systems::midnight::Mid::LivingEarly) {
        return;
    }
    idle_days(world);
    close_the_day(world);
    closings(world);
    openings(world);
    parcels(world);
    world.jobs2.cab_trips.clear();
    for v in world.jobs2.yard_units.values_mut() {
        *v = (0, v.0);
    }
}

/// The jobless free adults' first day without a post (`idle_since`).
fn idle_days(world: &mut World) {
    let today = world.day();
    // scan-ok: daily: the jobless' idle days
    let adults: Vec<EntityId> = world.citizens().into_iter().filter(|&a| world.has::<Brain>(a)).collect();
    let mut keep = std::collections::BTreeSet::new();
    for a in adults {
        // Shadow V4 bug 5: an exec draws a salary and is not idle (a
        // 68-year-old exec opened a solicitation pitch).
        if !free_adult(world, a) || world.has::<Job>(a) || crate::systems::founding::is_exec(world, a) {
            continue;
        }
        keep.insert(a);
        world.jobs2.idle_since.entry(a).or_insert(today);
    }
    world.jobs2.idle_since.retain(|a, _| keep.contains(a));
}

/// Each Pitch's day closes: the holding gang's cut of the take, a sweep in
/// a `Sweep` district (capped a day), the 7-day take and the dry days.
fn close_the_day(world: &mut World) {
    use crate::components::{Controller, Stance};
    let cfg = world.config.jobs2.clone();
    let (now, day) = (world.tick, world.day());
    let mut swept = 0u16;
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        let d = world.districts.get(p.district.index());
        let gang = d.and_then(|d| match d.control {
            Controller::Gang(g) => Some(g),
            _ => None,
        });
        let sweep_stance = d.is_some_and(|d| d.stance == Stance::Sweep);
        let mut cut_to = p.cut_to;
        // Jobs & lives phase 4 (§ 5.1): a street pitch under a pimp pays his
        // gang `pimp_cut` instead.
        let pimp = crate::systems::vice::street_cut(world, &p);
        let (gang, frac) = match pimp {
            Some(c) => (Some(c.gang), c.frac),
            None => (gang, cfg.cut_frac),
        };
        // The pre-view pass phase 4 (OPEN-119): a pimp's cut with a body on
        // either side was taken at his collections in person, not here.
        let at_close = pimp.is_none_or(|c| crate::systems::vice::street_cut_at_close(world, &p, &c));
        if let Some(g) = gang.filter(|_| p.take_today > 0 && at_close) {
            let cut = (p.take_today as f32 * frac).round() as i64;
            // Lives at work phase 4b (§ 2.3): the cut lands in the pimp's
            // purse; he passes `pimp_pass_up` of it to the gang.
            let to = pimp.and_then(|c| c.pimp).unwrap_or(g);
            let paid = ownership::pay(world, Some(o), Some(to), cut, Flow::StreetCut);
            ownership::jobs2_row(world).street_cut += paid;
            if pimp.is_some() {
                ownership::jobs2_row(world).pimp_cut += paid;
            }
            if to != g {
                crate::systems::vice::pass_up(world, to, g, paid);
            }
            cut_to = Some(g);
        }
        // Jobs & lives phase 4: a street pitch's sweep is the vice's
        // (`Solicitation`, never with `street_legal` on).
        let street = crate::systems::vice::on(world) && crate::systems::vice::is_street_pitch(world, &p);
        let sweep = if street {
            swept < cfg.sweeps_per_day && crate::systems::vice::sweep_street(world, &p)
        } else {
            sweep_stance
                && swept < cfg.sweeps_per_day
                && hash(world, PURPOSE_SWEEP, u64::from(o.index), day) < cfg.p_pitch_sweep
        };
        let mut closed_until = p.closed_until.filter(|&t| t > now);
        if sweep {
            swept += 1;
            let fine = ownership::pay(world, Some(o), None, cfg.pitch_fine, Flow::Fine);
            closed_until = Some(now + Tick::from(cfg.sweep_close_days.max(1)) * TICKS_PER_DAY);
            ownership::jobs2_row(world).swept += 1;
            let (who, at) = (world.name_of(o), world.district_name(p.district).to_string());
            world.push_event(
                EventKind::PitchSwept,
                &[o],
                format!("{who}'s pitch in {at} was swept (fined {fine}, closed a day)"),
            );
        }
        if let Some(x) = world.jobs2.pitches.get_mut(&o) {
            x.cut_to = cut_to;
            x.closed_until = closed_until;
            x.take_week = x.take_week - x.take_week / 7 + x.take_today;
            x.dry_days = if x.take_today > 0 { 0 } else { x.dry_days.saturating_add(1) };
            x.take_today = 0;
            x.cut_paid = 0;
            x.jobs_today = 0;
        }
    }
}

/// A Pitch closes when its owner is gone, hired, held or dry for
/// `pitch_fail_days` (`PitchClosed`; a gone owner's silently).
fn closings(world: &mut World) {
    let fail = world.config.jobs2.pitch_fail_days;
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let why = if !crate::systems::law::living(world, o) {
            world.jobs2.pitches.remove(&o);
            continue;
        } else if world.has::<Job>(o) {
            "hired"
        } else if world.has::<Sentence>(o) {
            "jailed"
        } else if world.jobs2.pitches.get(&o).is_some_and(|p| p.dry_days >= fail) {
            "no take"
        } else {
            continue;
        };
        let Some(p) = world.jobs2.pitches.remove(&o) else { continue };
        ownership::jobs2_row(world).pitch_closed += 1;
        let what = world.config.trade(p.trade).map_or("pitch", |r| r.label.as_str()).to_string();
        let who = world.name_of(o);
        world.push_event(EventKind::PitchClosed, &[o], format!("{who} closed a {what} pitch ({why})"));
    }
}

/// The midnight openings: per district under its cap, the idle jobless
/// with the stake, in id order on `p_pitch_open` (a pure hash of the day
/// and the agent); the trade by skill (knowledge a tinker, fighting a
/// rickshaw, else a vendor); the spot a HangOut spot of the district no
/// Pitch holds, else its centroid.
fn openings(world: &mut World) {
    let cfg = world.config.jobs2.clone();
    let (today, now) = (world.day(), world.tick);
    let n = world.districts.len();
    let mut count = vec![0u16; n];
    for p in world.jobs2.pitches.values() {
        if let Some(c) = count.get_mut(p.district.index()) {
            *c += 1;
        }
    }
    let cands: Vec<EntityId> = world
        .jobs2
        .idle_since
        .iter()
        .filter(|&(_, &since)| today.saturating_sub(since) >= u64::from(cfg.pitch_after_days))
        .map(|(&a, _)| a)
        .collect();
    for a in cands {
        if world.jobs2.pitches.contains_key(&a) || !free_adult(world, a) || world.has::<Job>(a) {
            continue;
        }
        if coins(world, a) < cfg.pitch_stake || hash(world, PURPOSE_OPEN, u64::from(a.index), today) >= cfg.p_pitch_open
        {
            continue;
        }
        let Some(d) = district_of_agent(world, a) else { continue };
        // The pressure pass F1: the spot is chosen below from the home's
        // reach, not the district the opener stood in at midnight.
        if !crate::systems::distance::on(world) && count.get(d.index()).is_none_or(|&c| c >= cfg.pitches_per_district) {
            continue;
        }
        let skills = world.comp::<Skills>(a).cloned();
        // Shadow V4 bug 6: an elder (past `[ages] frail_from_years`) vends;
        // the rickshaw and the street are not his rows (3 of 4 street
        // workers were 64-68).
        let elder = crate::systems::ages::on(world)
            && world.comp::<crate::components::Identity>(a).map_or(0, |i| i.age_years())
                >= world.config.ages.frail_from_years;
        let kind = match skills {
            Some(s) if s.knowledge >= cfg.tinker_skill => PitchKind::Tinker,
            Some(s) if s.fighting >= cfg.rickshaw_skill && !elder => PitchKind::Rickshaw,
            _ => PitchKind::Vendor,
        };
        // Jobs & lives phase 4 (§ 5.1): with `[vice]` on, a lawless opener
        // works a street pitch on `p_street` (a pure hash of the day).
        let street = !elder
            && crate::systems::vice::on(world)
            && world.comp::<Personality>(a).is_some_and(|p| p.lawfulness < world.config.vice.street_lawful)
            && hash(world, PURPOSE_STREET, u64::from(a.index), today) < world.config.vice.p_street;
        let kind = if street && pitch_row(world, PitchKind::Street).is_some() { PitchKind::Street } else { kind };
        let Some(trade) = pitch_row(world, kind).or_else(|| pitch_row(world, PitchKind::Vendor)) else { continue };
        let used: Vec<TilePos> = world.jobs2.pitches.values().map(|p| p.spot).collect();
        // Shadow V4 bug 6: the free spot nearest the opener's home (the
        // district's first free one had vendors walking 47-168 min a way).
        let from = crate::systems::contracts::home_door(world, a)
            .or_else(|| world.comp::<crate::components::Position>(a).map(|p| p.tile));
        // The pressure pass F1 (`[distance]` on): the free spot nearest the
        // home door in any district under its cap, and none past `[distance]
        // job_cap_ticks` (a vendor whose midnight found him in the Sump
        // pitched 670 tiles from his Mid-Levels home, 9 h a way).
        let (spot, d) = if let (Some(cap), Some(f)) = (crate::systems::distance::job_cap(world), from) {
            let near = world
                .spots
                .iter()
                .enumerate()
                .filter(|&(i, _)| count.get(i).is_some_and(|&c| c < cfg.pitches_per_district))
                .flat_map(|(i, s)| s.iter().map(move |x| (x.tile, i)))
                .filter(|(t, _)| !used.contains(t))
                .min_by_key(|&(t, i)| (f.manhattan(t), t.x, t.y, i));
            match near.filter(|&(t, _)| crate::systems::distance::travel(world, f, t) <= cap) {
                Some((t, i)) => (Some(t), crate::components::DistrictId(i as u8)),
                None => continue,
            }
        } else {
            let spot = world
                .spots
                .get(d.index())
                .and_then(|s| {
                    s.iter()
                        .map(|x| x.tile)
                        .filter(|t| !used.contains(t))
                        .min_by_key(|t| (from.map_or(0, |f| f.manhattan(*t)), t.x, t.y))
                })
                .or_else(|| world.districts.get(d.index()).map(|x| x.centroid));
            (spot, d)
        };
        let Some(spot) = spot else { continue };
        let p = Pitch {
            spot,
            district: d,
            trade,
            owner: a,
            since: now,
            take_today: 0,
            take_week: 0,
            cut_to: None,
            stock: 0,
            closed_until: None,
            dry_days: 0,
            jobs_today: 0,
            cut_paid: 0,
        };
        world.jobs2.pitches.insert(a, p);
        count[d.index()] += 1;
        ownership::jobs2_row(world).pitch_opened += 1;
        let what = world.config.trade(trade).map_or("pitch", |r| r.label.as_str()).to_string();
        let (who, at) = (world.name_of(a), world.district_name(d).to_string());
        world.push_event(EventKind::PitchOpened, &[a], format!("{who} opened a {what} pitch in {at}"));
    }
}

// ---------------------------------------------------------------------------
// The hourly pass: vendors' stock, day posts' end, the dawn pick
// ---------------------------------------------------------------------------

/// Hourly (from `living::run`): the vendors' morning stock, the day posts
/// ended, and at `corner_hour` the dawn pick.
pub fn hourly(world: &mut World) {
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if crate::systems::service::street_on(world) {
        drop_stale_rides(world);
    }
    if !on(world) {
        return;
    }
    restock(world);
    vendor_passers(world);
    let ended = crate::systems::industry::end_day_posts(world);
    ownership::jobs2_row(world).day_posts_ended += ended;
    if world.tick_of_day() == world.config.jobs2.corner_hour {
        dawn_pick(world);
    }
}

/// A working vendor short of a batch buys `vendor_batch` shelf units at
/// the nearest stocked Market (the real unit price, `Flow::Wholesale`),
/// `synth_stretch` meals each, as far as his wallet goes.
fn restock(world: &mut World) {
    let batch = world.config.jobs2.vendor_batch;
    let stretch = crate::systems::diet::stretch(world);
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        if kind_of_pitch(world, &p) != Some(PitchKind::Vendor) || p.stock >= batch * stretch / 2 || !working(world, &p)
        {
            continue;
        }
        let market = world
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .copied()
            .filter(|&m| world.comp::<Building>(m).is_some_and(|b| !b.demolished && b.stock_food > 0))
            .filter(|&m| !world.is_closed(m))
            .min_by_key(|&m| (world.comp::<Building>(m).map_or(u32::MAX, |b| b.door.manhattan(p.spot)), m));
        let Some(m) = market else { continue };
        let price = world.comp::<crate::components::Market>(m).map_or(0, |x| x.price_food).max(1);
        let stock = world.comp::<Building>(m).map_or(0, |b| b.stock_food);
        let units = batch.min(stock).min((coins(world, o).max(0) / price).min(i64::from(u32::MAX)) as u32);
        if units == 0 {
            continue;
        }
        let owner = world.owner_of(m);
        let paid = ownership::pay(world, Some(o), owner, i64::from(units) * price, Flow::Wholesale);
        ownership::credit(world, m, paid);
        if let Some(b) = world.comp_mut::<Building>(m) {
            b.stock_food -= units;
        }
        if let Some(x) = world.jobs2.pitches.get_mut(&o) {
            x.stock += units * stretch;
        }
    }
}

/// `lod::stat_eat`'s Market leg: an off-screen synth buyer takes a meal
/// from an open vendor's stall in his district on `p_vendor_pick` (a pure
/// hash of the hour and the buyer) at `vendor_markup` × the synth meal's
/// price, paid to the vendor (`Flow::Food`). `true` when he ate there.
pub fn stat_vendor_meal(world: &mut World, id: EntityId, market: EntityId) -> bool {
    if !on(world) || world.jobs2.pitches.is_empty() {
        return false;
    }
    let (q, synth) = crate::systems::diet::offer(world, id, market);
    if q != crate::systems::diet::Quality::Synth {
        return false;
    }
    let cfg = &world.config.jobs2;
    if hash(world, PURPOSE_PICK, u64::from(id.index), world.tick / TICKS_PER_HOUR) >= cfg.p_vendor_pick {
        return false;
    }
    let price = ((synth as f32 * cfg.vendor_markup).ceil() as i64).max(1);
    if coins(world, id) < price {
        return false;
    }
    let Some(d) = district_of_agent(world, id) else { return false };
    let vendor = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.district == d && p.owner != id && p.stock > 0)
        .filter(|p| kind_of_pitch(world, p) == Some(PitchKind::Vendor) && working(world, p))
        .map(|p| p.owner)
        .min_by_key(|o| (hash(world, PURPOSE_PICK, u64::from(o.index), u64::from(id.index)).to_bits(), *o));
    let Some(v) = vendor else { return false };
    let paid = ownership::pay(world, Some(id), Some(v), price, Flow::Food);
    if paid <= 0 {
        return false;
    }
    if let Some(p) = world.jobs2.pitches.get_mut(&v) {
        p.stock -= 1;
        p.take_today += paid;
    }
    let cfg = world.config.needs.clone();
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    crate::systems::health::ate(world, id, crate::components::MealKind::Synth);
    world.mark_day(id, crate::components::trace_flags::ATE);
    let row = ownership::jobs2_row(world);
    row.vendor_meals += 1;
    row.vendor_take += paid;
    // Lives at work phase 4b (§ 2.3): the vendor's regulars (the stall is a tile).
    if crate::systems::service::street_on(world) {
        crate::systems::service::encounter(world, v, id, None, d, crate::components::ServeKind::Buyer, paid);
    }
    true
}

/// The pressure pass phase 2 (OPEN-176, `[customers]`): a working
/// vendor's stall sells, each hour, up to `vendor_per_hour` meals to the
/// hungry bodies passing it (Full or Coarse, out on the street within
/// `vendor_tiles`, hunger under `vendor_hunger`, the price in hand; the
/// lowest hash of the hour first), at the off-screen buyer's price
/// (`vendor_markup` × the nearest Market's synth meal): the meal eaten
/// there, the coins to the vendor, the Buyer encounter on both sides. The
/// off-screen buyer is `stat_vendor_meal`.
fn vendor_passers(world: &mut World) {
    let Some(c) = crate::systems::customers::cfg(world).cloned() else { return };
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    let hour = world.tick / TICKS_PER_HOUR;
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        if kind_of_pitch(world, &p) != Some(PitchKind::Vendor) || p.stock == 0 || !working(world, &p) {
            continue;
        }
        let market = world
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .copied()
            .filter(|&m| world.comp::<Building>(m).is_some_and(|b| !b.demolished))
            .min_by_key(|&m| (world.comp::<Building>(m).map_or(u32::MAX, |b| b.door.manhattan(p.spot)), m));
        let Some(m) = market else { continue };
        let mut buyers: Vec<(u32, EntityId)> = world
            .agents_near(p.spot, c.vendor_tiles, None)
            .into_iter()
            .filter(|&id| {
                id != o
                    && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
                    && world.comp::<Position>(id).is_some_and(|x| x.building.is_none())
                    && crate::systems::demography::is_adult(world, id)
                    && !world.has::<Sentence>(id)
                    && crate::systems::law::living(world, id)
                    && world.comp::<crate::components::Needs>(id).is_some_and(|n| n.hunger < c.vendor_hunger)
            })
            .map(|id| (hash(world, PURPOSE_PICK, u64::from(id.index) ^ u64::from(o.index), hour).to_bits(), id))
            .collect();
        buyers.sort_unstable();
        let mut sold = 0u8;
        for (_, id) in buyers {
            if sold >= c.vendor_per_hour {
                break;
            }
            if world.jobs2.pitches.get(&o).is_none_or(|x| x.stock == 0) {
                break;
            }
            let (_, synth) = crate::systems::diet::offer(world, id, m);
            let price = ((synth as f32 * world.config.jobs2.vendor_markup).ceil() as i64).max(1);
            if coins(world, id) < price {
                continue;
            }
            let paid = ownership::pay(world, Some(id), Some(o), price, Flow::Food);
            if paid <= 0 {
                continue;
            }
            if let Some(x) = world.jobs2.pitches.get_mut(&o) {
                x.stock -= 1;
                x.take_today += paid;
            }
            let cfg = world.config.needs.clone();
            if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                crate::needs::eat(n, &cfg);
            }
            crate::systems::health::ate(world, id, crate::components::MealKind::Synth);
            world.mark_day(id, crate::components::trace_flags::ATE);
            world.remember(id, crate::components::MemoryKind::Ate, None, 0.2, 0.3, false);
            let row = ownership::jobs2_row(world);
            row.vendor_meals += 1;
            row.vendor_take += paid;
            row.vendor_bodies += 1;
            crate::systems::service::encounter(
                world,
                o,
                id,
                None,
                p.district,
                crate::components::ServeKind::Buyer,
                paid,
            );
            sold += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// The labour corner (§ 3)
// ---------------------------------------------------------------------------

/// The corners: per district with an open Construction or Freight Yard,
/// the lowest-id one's door (the street outside it), ascending by district.
pub fn corners(world: &World) -> Vec<(DistrictId, TilePos)> {
    if world.config.island_trades.as_ref().is_some_and(|c| c.corner_per_yard) {
        return yard_corners(world);
    }
    let mut out: Vec<(DistrictId, EntityId)> = Vec::new();
    for kind in [BuildingKind::ConstructionYard, BuildingKind::FreightYard] {
        for &y in world.buildings_of_kind(kind) {
            if !crate::systems::industry::open(world, y) {
                continue;
            }
            let d = world.district_of_building(y);
            match out.iter_mut().find(|(x, _)| *x == d) {
                Some((_, b)) if y < *b => *b = y,
                Some(_) => {}
                None => out.push((d, y)),
            }
        }
    }
    out.sort();
    out.into_iter().filter_map(|(d, y)| world.comp::<Building>(y).map(|b| (d, world.outside_door(b)))).collect()
}

/// The island pass phase 8 (`[island_trades] corner_per_yard`; OPEN-168):
/// a corner outside every open Construction and Freight Yard, in id order,
/// one within `[jobs2] corner_tiles` of a corner already stood sharing it;
/// ascending by (district, tile). The base's one a district stood the
/// island's Vats corner 380 tiles from its Construction Yards, so their day
/// posts drew on a queue of five.
fn yard_corners(world: &World) -> Vec<(DistrictId, TilePos)> {
    let reach = world.config.jobs2.corner_tiles;
    let mut yards: Vec<EntityId> = [BuildingKind::ConstructionYard, BuildingKind::FreightYard]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|&y| crate::systems::industry::open(world, y))
        .collect();
    yards.sort_unstable();
    let mut out: Vec<(DistrictId, TilePos)> = Vec::new();
    for y in yards {
        let Some(b) = world.comp::<Building>(y) else { continue };
        let t = world.outside_door(b);
        if out.iter().any(|&(_, c)| c.manhattan(t) <= reach) {
            continue;
        }
        out.push((world.district_of_building(y), t));
    }
    out.sort();
    out
}

/// The island pass phase 8 (`[island_trades] corner_stat_ticks`;
/// OPEN-168): past `corner_stat_reach` tiles of every corner, the corner
/// nearest `from` by travel (`distance::travel`, the El where it wins)
/// within the cap, ties the lower index; `None` off. The island's Yards sit
/// at the Vats' edge, and the Statistical broke within 160 tiles of them
/// queued five a dawn.
fn corner_by_travel(world: &World, corners: &[(DistrictId, TilePos)], from: TilePos) -> Option<usize> {
    let cap = world.config.island_trades.as_ref()?.corner_stat_ticks;
    corners
        .iter()
        .enumerate()
        .map(|(i, &(_, t))| (crate::systems::distance::travel(world, from, t), i))
        .filter(|&(d, _)| d <= cap)
        .min()
        .map(|(_, i)| i)
}

/// The nearest corner to `from` within `reach` tiles.
fn corner_near(corners: &[(DistrictId, TilePos)], from: TilePos, reach: u32) -> Option<usize> {
    corners
        .iter()
        .enumerate()
        .map(|(i, &(_, t))| (t.manhattan(from), i))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, i)| i)
}

/// One who may stand at the corner: a free jobless adult, broke, lawful enough.
fn corner_ok(world: &World, id: EntityId) -> bool {
    let cfg = &world.config.jobs2;
    free_adult(world, id)
        && !world.has::<Job>(id)
        && coins(world, id) < cfg.corner_broke
        && world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= cfg.corner_lawful)
}

/// The day's posts by employer: a Construction Yard's `day_per_site` a
/// site, a Freight Yard that handled `backlog_units` yesterday its
/// `yard_posts`, each Farm in a harvest season its `harvest_posts`; an
/// owner posts only what its purse pays a day.
fn day_posts(world: &World, wage: i64) -> Vec<EntityId> {
    let cfg = &world.config.jobs2;
    let mut out: Vec<EntityId> = Vec::new();
    for y in world.buildings_of_kind(BuildingKind::ConstructionYard).to_vec() {
        if crate::systems::industry::open(world, y) {
            let n = usize::from(cfg.day_per_site) * crate::systems::industry::sites_of(world, y).len();
            out.extend(std::iter::repeat_n(y, n));
        }
    }
    for y in world.buildings_of_kind(BuildingKind::FreightYard).to_vec() {
        let yesterday = world.jobs2.yard_units.get(&y).map_or(0, |v| v.1);
        if crate::systems::industry::open(world, y) && yesterday >= cfg.backlog_units {
            out.extend(std::iter::repeat_n(y, usize::from(cfg.yard_posts)));
        }
    }
    // The pressure pass phase 2 (OPEN-176): every open Yard's foreman
    // takes `shape_up_posts` hands at the dawn shape-up, sites or none
    // (the corner's posts dried up with the sites: two picks in a week).
    if let Some(n) = crate::systems::customers::cfg(world).map(|c| usize::from(c.shape_up_posts)) {
        for k in [BuildingKind::ConstructionYard, BuildingKind::FreightYard] {
            for y in world.buildings_of_kind(k).to_vec() {
                if crate::systems::industry::open(world, y) {
                    out.extend(std::iter::repeat_n(y, n));
                }
            }
        }
    }
    if cfg.harvest_seasons.contains(&world.season()) {
        for f in world.buildings_of_kind(BuildingKind::Farm).to_vec() {
            if world.comp::<Building>(f).is_some_and(|b| !b.demolished && !b.derelict) {
                out.extend(std::iter::repeat_n(f, usize::from(cfg.harvest_posts)));
            }
        }
    }
    // An owner posts what its purse pays.
    let mut spent: std::collections::BTreeMap<Option<EntityId>, i64> = Default::default();
    out.retain(|&b| {
        let o = world.owner_of(b);
        let s = spent.entry(o).or_insert(0);
        *s += wage;
        world.purse(o) >= *s
    });
    out
}

/// § 3, at `corner_hour`: the day's posts filled from the corner's queue
/// (bodies standing there; off screen, the broke living near on
/// `p_corner`), each post from the nearest corner's queue: the
/// employer's foreman's known contacts first, then the strongest
/// (`fighting`), then id. A pick is hired into the `day` row with
/// `Job.until` two hours past the shift's end (`DawnPick` per corner).
fn dawn_pick(world: &mut World) {
    let Some(role) = day_role(world) else { return };
    // Round 3 S5 (OPEN-217): no shape-up on the rest day: the post's shift
    // is a workday's (`routine::workday_of`), so a hand picked then held a
    // post with no shift and wandered all day (`day_labourer_75212 d27`).
    if world.config.verbs.is_some() && !crate::exec::routine::is_workday(world.day() as i64) {
        return;
    }
    let cfg = world.config.jobs2.clone();
    let corners = corners(world);
    if corners.is_empty() {
        return;
    }
    let wage = world.config.wage(role);
    let posts = day_posts(world, wage);
    ownership::jobs2_row(world).day_posts_offered += posts.len() as u32;
    world.jobs2.last_posts = posts.len() as u32;
    // The queues, one per corner.
    let day = world.day();
    let mut queues: Vec<Vec<EntityId>> = vec![Vec::new(); corners.len()];
    // scan-ok: daily: the dawn pick's queue
    for a in world.citizens() {
        if !world.has::<Brain>(a) || !corner_ok(world, a) {
            continue;
        }
        let body = world.comp::<Brain>(a).is_some_and(|b| b.lod != Lod::Statistical);
        let at = if body {
            world
                .comp::<Position>(a)
                .filter(|p| p.building.is_none())
                .and_then(|p| corner_near(&corners, p.tile, cfg.corner_tiles))
        } else {
            let from =
                crate::systems::contracts::home_door(world, a).or_else(|| world.comp::<Position>(a).map(|p| p.tile));
            // The roll first: the travel fallback is the dear part.
            let rolled = hash(world, PURPOSE_CORNER, u64::from(a.index), day) < cfg.p_corner;
            from.filter(|_| rolled).and_then(|t| {
                corner_near(&corners, t, cfg.corner_stat_reach).or_else(|| corner_by_travel(world, &corners, t))
            })
        };
        if let Some(i) = at {
            queues[i].push(a);
        }
    }
    let queued: u32 = queues.iter().map(|q| q.len() as u32).sum();
    ownership::jobs2_row(world).corner_queue += queued;
    let mut picked = vec![0u32; corners.len()];
    let shift_end = world.config.world.shift_day.iter().map(|&(_, e)| e).max().unwrap_or(1080);
    let until = world.tick - Tick::from(world.tick_of_day()) + Tick::from(shift_end) + 2 * TICKS_PER_HOUR;
    for b in posts {
        let Some(door) = world.comp::<Building>(b).map(|x| x.door) else { continue };
        let Some(i) = corner_near(&corners, door, u32::MAX) else { continue };
        let foreman = crate::systems::industry::foreman(world, b, None);
        // Lives at work phase 6 (§ 4.1): the corner's band (every one off).
        let band = crate::systems::careers::hire_band(world, role);
        let order = |&x: &EntityId, &y: &EntityId| {
            let known = |a: EntityId| foreman.is_some_and(|f| world.edge(a, f).is_some());
            let f = |a: EntityId| world.comp::<Skills>(a).map_or(0.0, |s| s.fighting);
            known(x).cmp(&known(y)).then(f(x).total_cmp(&f(y))).then(y.cmp(&x))
        };
        let admitted = queues[i]
            .iter()
            .copied()
            .filter(|&a| !world.has::<Job>(a) && crate::systems::careers::admits(world, a, band));
        let best = match world.config.distance.as_ref() {
            // The island pass phase 1 (OPEN-163): in the foreman's order, a
            // labourer past `[distance] job_cap_ticks` from the post (from
            // where he stands, else his Home door) is passed over while one
            // within it is left, else the nearest.
            Some(cfg) => {
                let mut ranked: Vec<EntityId> = admitted.collect();
                ranked.sort_by(|x, y| order(y, x));
                crate::systems::distance::pick(
                    ranked.iter().enumerate().map(|(r, &a)| {
                        let from = world
                            .comp::<crate::components::Position>(a)
                            .map(|p| p.tile)
                            .or_else(|| crate::systems::distance::set_out(world, a));
                        let t = from.map_or(Tick::MAX, |t| crate::systems::distance::travel(world, t, door));
                        (t, r as i64, (r, a))
                    }),
                    cfg.job_cap_ticks,
                )
                .map(|(_, a)| a)
            }
            None => admitted.max_by(order),
        };
        let Some(a) = best else { continue };
        queues[i].retain(|&x| x != a);
        crate::systems::demography::hire(world, a, b, role);
        if let Some(j) = world.comp_mut::<Job>(a) {
            j.until = Some(until);
        }
        // The pressure pass phase 2: the shape-up is a meeting (both diaries).
        if let Some(f) = foreman.filter(|_| crate::systems::customers::cfg(world).is_some()) {
            let sal = world.config.service.served_salience;
            world.remember(f, crate::components::MemoryKind::Served, Some(a), sal, 0.1, false);
            world.remember(a, crate::components::MemoryKind::Served, Some(f), sal, 0.2, false);
        }
        picked[i] += 1;
        ownership::jobs2_row(world).day_posts_filled += 1;
    }
    for (i, &(d, _)) in corners.iter().enumerate() {
        let left = queues[i].len();
        if picked[i] == 0 && left == 0 {
            continue;
        }
        let at = world.district_name(d).to_string();
        world.push_event(
            EventKind::DawnPick,
            &[],
            format!("the labour corner in {at}: {} picked at dawn, {left} sent away", picked[i]),
        );
    }
}

// ---------------------------------------------------------------------------
// Couriers (§ 3)
// ---------------------------------------------------------------------------

/// A Yard's couriers (its `contract` staff on a `round` duty).
fn couriers(world: &World, yard: EntityId) -> usize {
    courier_staff(world, yard).count()
}

fn courier_staff(world: &World, yard: EntityId) -> impl Iterator<Item = EntityId> + '_ {
    world.staff_of(yard).iter().copied().filter(move |&s| {
        world
            .comp::<Job>(s)
            .and_then(|j| crate::systems::trades::row_of(world, j.role))
            .is_some_and(|r| r.model == TradeModel::Contract && r.duty == TradeDuty::Round)
    })
}

/// The pre-view pass phase 3 (OPEN-141): `courier` works his parcels as
/// `haul` trips (a Freight Yard's courier whose row's shape is `haul`).
pub fn haul_courier(world: &World, courier: EntityId) -> bool {
    let Some(j) = world.comp::<Job>(courier) else { return false };
    j.employer.and_then(|y| world.comp::<Building>(y)).is_some_and(|b| b.kind == BuildingKind::FreightYard)
        && crate::systems::trades::on_round(world, courier)
        && crate::systems::shift::shape_of(world, j.role) == crate::config::ShiftShape::Haul
}

/// The pre-view pass phase 3 (OPEN-141): a haul courier's addresses: the
/// distinct buildings his Yard's pending parcels go to, each dealt to one
/// of the Yard's couriers by a hash of the address (stable as parcels are
/// handed over; the round's rank rotation wrapped, so a Statistical
/// colleague's first-hour round took every parcel before a body's first
/// leg arrived). Ascending; none off.
pub fn courier_addresses(world: &World, courier: EntityId) -> Vec<EntityId> {
    let Some(yard) = world.comp::<Job>(courier).and_then(|j| j.employer) else { return Vec::new() };
    // The couriers working today (a rest day's addresses go to the others).
    let staff: Vec<EntityId> = courier_staff(world, yard)
        .filter(|&s| {
            s == courier
                || world
                    .comp::<Job>(s)
                    .is_some_and(|j| crate::exec::routine::workday_of(world, s, j, j.shift_key_at(world.tick)))
        })
        .collect();
    let Some(rank) = staff.iter().position(|&s| s == courier) else { return Vec::new() };
    let n = staff.len() as u64;
    let mut v: Vec<EntityId> = world
        .jobs2
        .parcels
        .iter()
        .filter(|p| p.yard == yard)
        .map(|p| p.at)
        .filter(|at| {
            crate::rng::splitmix64(PURPOSE_PARCEL ^ u64::from(at.index) << 20 ^ u64::from(yard.index)) % n
                == rank as u64
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The pre-view pass phase 3 (OPEN-141): the parcels pending for `b` from
/// `courier`'s Yard.
pub fn parcels_for(world: &World, courier: EntityId, b: EntityId) -> usize {
    let Some(yard) = world.comp::<Job>(courier).and_then(|j| j.employer) else { return 0 };
    world.jobs2.parcels.iter().filter(|p| p.yard == yard && p.at == b).count()
}

/// Midnight: each corp with two or more standing buildings sends
/// `parcels_per_building` a building through the nearest open Freight Yard
/// with couriers and room on their day (`parcels_per_courier` each):
/// `parcel_fee` a parcel to the Yard's owner (`Flow::Handling`); the
/// sender's buildings within `site_reach` of the Yard join its couriers'
/// round.
fn parcels(world: &mut World) {
    let cfg = world.config.jobs2.clone();
    world.jobs2.parcel_stops.clear();
    // The pre-view pass phase 3 (OPEN-141): a parcel left from an earlier
    // day (its courier off, or its address on no round) goes back to the
    // Yard at midnight; it lay pending for good before (2k seed 42 x 30:
    // 4,320 sent, 952 handed over, 172 returned).
    if crate::systems::shift::haul_on(world) {
        let today = world.day();
        let before = world.jobs2.parcels.len();
        world.jobs2.parcels.retain(|p| p.day >= today);
        let back = (before - world.jobs2.parcels.len()) as u32;
        if back > 0 {
            ownership::jobs2_row(world).parcels_returned += back;
        }
    }
    let yards: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::FreightYard)
        .iter()
        .copied()
        .filter(|&y| crate::systems::industry::open(world, y))
        .collect();
    let mut room: std::collections::BTreeMap<EntityId, u32> =
        yards.iter().map(|&y| (y, couriers(world, y) as u32 * cfg.parcels_per_courier)).collect();
    if room.values().all(|&r| r == 0) {
        return;
    }
    for c in world.corps() {
        let Some(bs) = world.comp::<Corp>(c).map(|x| x.buildings.clone()) else { continue };
        let standing: Vec<EntityId> = bs
            .into_iter()
            .filter(|&b| world.comp::<Building>(b).is_some_and(|x| !x.demolished && !x.derelict))
            .collect();
        if standing.len() < 2 {
            continue;
        }
        let want = (standing.len() as f32 * cfg.parcels_per_building).round() as u32;
        let from = world.comp::<Building>(standing[0]).map(|b| b.door).unwrap_or_default();
        // The island pass phase 8 (OPEN-168): the Yard with the most of the
        // corp's staffed buildings in travel reach, those its recipients.
        let reach = parcel_reach(world, &standing, &yards, &room);
        let yard = reach.as_ref().map(|(y, _)| *y).or_else(|| {
            yards
                .iter()
                .copied()
                .filter(|y| room.get(y).is_some_and(|&r| r > 0))
                .min_by_key(|&y| (world.comp::<Building>(y).map_or(u32::MAX, |b| b.door.manhattan(from)), y))
        });
        let Some(y) = yard else { break };
        let n = want.min(room[&y]);
        if n == 0 {
            continue;
        }
        *room.get_mut(&y).expect("a yard") -= n;
        let owner = world.owner_of(y);
        let paid = ownership::charge(world, Some(c), owner, i64::from(n) * cfg.parcel_fee, Flow::Handling);
        ownership::credit(world, y, paid);
        let row = ownership::jobs2_row(world);
        row.parcels += n;
        row.parcel_fees += paid;
        let ydoor = world.comp::<Building>(y).map(|b| b.door).unwrap_or_default();
        let near: Vec<EntityId> = match reach {
            Some((_, near)) => near,
            None => standing
                .iter()
                .copied()
                .filter(|&b| world.comp::<Building>(b).is_some_and(|x| x.door.manhattan(ydoor) <= cfg.site_reach))
                .take(6)
                .collect(),
        };
        // Lives at work phase 4 (§ 2.3): the parcels are named, and the
        // round's stops are their recipients' buildings (with staff to sign).
        let stops =
            if crate::systems::service::street_on(world) { name_parcels(world, y, &standing, &near, n) } else { near };
        world.jobs2.parcel_stops.entry(y).or_default().extend(stops);
    }
}

/// The island pass phase 8 (OPEN-168; `[island_trades] parcel_cap_ticks`):
/// the open Yard with room holding the most of `standing`'s staffed
/// buildings within the cap's travel of its door (ties the lower id), and
/// those buildings (ascending, the first `parcel_stops_max`); `None` off or
/// with none in reach of any Yard (the base's pick). The base took the Yard
/// nearest the corp's first building and its buildings within `site_reach`
/// tiles: on the island, where the Yards cluster at the Vats' edge, that
/// left most corps' parcels with no address. No random draw.
fn parcel_reach(
    world: &World,
    standing: &[EntityId],
    yards: &[EntityId],
    room: &std::collections::BTreeMap<EntityId, u32>,
) -> Option<(EntityId, Vec<EntityId>)> {
    let cfg = world.config.island_trades.as_ref()?;
    let staffed: Vec<(EntityId, TilePos)> = standing
        .iter()
        .copied()
        .filter(|&b| !world.staff_of(b).is_empty())
        .filter_map(|b| world.comp::<Building>(b).map(|x| (b, x.door)))
        .collect();
    let mut best: Option<(usize, EntityId, Vec<EntityId>)> = None;
    for &y in yards {
        if !room.get(&y).is_some_and(|&r| r > 0) {
            continue;
        }
        let Some(ydoor) = world.comp::<Building>(y).map(|b| b.door) else { continue };
        // Not the Yard's own door (the Yard, its company floors): no trip.
        let near: Vec<EntityId> = staffed
            .iter()
            .filter(|&&(_, d)| d != ydoor && crate::systems::distance::travel(world, ydoor, d) <= cfg.parcel_cap_ticks)
            .map(|&(b, _)| b)
            .collect();
        // Two at least: a parcel is from one of the corp's buildings to another.
        if near.len() >= 2 && best.as_ref().is_none_or(|(n, _, _)| near.len() > *n) {
            best = Some((near.len(), y, near));
        }
    }
    best.map(|(_, y, mut near)| {
        near.truncate(usize::from(cfg.parcel_stops_max));
        (y, near)
    })
}

/// Lives at work phase 4 (§ 2.3): `n` parcels of one corp through `yard`:
/// each from a sender among `standing` (hashed) to a staff member (hashed)
/// of a recipient among `near` with staff, not the sender (hashed). Returns
/// the recipient buildings (the couriers' stops); none with no recipient.
fn name_parcels(world: &mut World, yard: EntityId, standing: &[EntityId], near: &[EntityId], n: u32) -> Vec<EntityId> {
    let (day, fee) = (world.day(), world.config.jobs2.parcel_fee);
    let to_staff: Vec<EntityId> = near.iter().copied().filter(|&b| !world.staff_of(b).is_empty()).collect();
    let mut stops = Vec::new();
    for k in 0..u64::from(n) {
        let pick = |w: &World, salt: u64, len: usize| {
            let u = hash(w, PURPOSE_PARCEL, u64::from(yard.index) << 20 | day, k * 4 + salt);
            ((u * len as f32) as usize).min(len - 1)
        };
        let from = standing[pick(world, 0, standing.len())];
        let at: Vec<EntityId> = to_staff.iter().copied().filter(|&b| b != from).collect();
        if at.is_empty() {
            continue;
        }
        let at = at[pick(world, 1, at.len())];
        let staff = world.staff_of(at).to_vec();
        let to = staff[pick(world, 2, staff.len())];
        world.jobs2.parcels.push(Parcel { from, to, at, yard, fee, day });
        stops.push(at);
    }
    stops
}

/// Lives at work phase 4 (§ 2.3, the courier's stop): a courier of `yard`
/// at `b` (a body standing in it on his round, `trades::round_plan`; a
/// Statistical courier credited at his shift's first hour, `lod::stat_work`)
/// hands each of the yard's parcels for `b` to its recipient when present
/// (`service::present`), else to a present staff member (hashed, D6): a
/// `Parcel` encounter; with nobody present it stays for the shift's end.
pub fn courier_stop(world: &mut World, courier: EntityId, b: EntityId) {
    if !crate::systems::service::street_on(world) {
        return;
    }
    let Some(yard) = world.comp::<Job>(courier).and_then(|j| j.employer) else { return };
    let mine: Vec<usize> = (0..world.jobs2.parcels.len())
        .filter(|&i| world.jobs2.parcels[i].yard == yard && world.jobs2.parcels[i].at == b)
        .collect();
    let mut done = Vec::new();
    for i in mine {
        let p = world.jobs2.parcels[i];
        let hand = if crate::systems::service::present(world, p.to, b) {
            Some(p.to)
        } else {
            let hour = world.tick / TICKS_PER_HOUR;
            world
                .staff_of(b)
                .iter()
                .copied()
                .filter(|&s| s != courier && crate::systems::service::present(world, s, b))
                .map(|s| (hash(world, PURPOSE_PARCEL, u64::from(p.to.index) ^ hour, u64::from(s.index)).to_bits(), s))
                .min()
                .map(|(_, s)| s)
        };
        let Some(r) = hand else { continue };
        crate::systems::service::serve(world, courier, r, b, crate::components::ServeKind::Parcel, 0);
        ownership::jobs2_row(world).parcels_delivered += 1;
        done.push(i);
    }
    for i in done.into_iter().rev() {
        world.jobs2.parcels.remove(i);
    }
}

/// Lives at work phase 4 (§ 2.3): a courier's shift end (`service::
/// shift_end`, both tiers): the yard's parcels still pending at the stops of
/// his round go back to the Yard (`parcels_returned`).
pub fn parcels_back(world: &mut World, courier: EntityId) {
    if !crate::systems::service::street_on(world) || world.jobs2.parcels.is_empty() {
        return;
    }
    let Some(job) = world.comp::<Job>(courier) else { return };
    let Some(yard) = job.employer else { return };
    if !crate::systems::trades::on_round(world, courier) {
        return;
    }
    // The pre-view pass phase 3 (OPEN-141): a haul courier's own addresses.
    let route = if haul_courier(world, courier) {
        courier_addresses(world, courier)
    } else {
        crate::systems::trades::round_for(world, courier, job.shift_key_at(world.tick))
    };
    let before = world.jobs2.parcels.len();
    world.jobs2.parcels.retain(|p| p.yard != yard || !route.contains(&p.at));
    let back = (before - world.jobs2.parcels.len()) as u32;
    if back > 0 {
        ownership::jobs2_row(world).parcels_returned += back;
    }
}

/// The couriers' round stops at `yard` today (`trades::new_round`).
pub fn parcel_stops(world: &World, yard: EntityId) -> Vec<EntityId> {
    let mut v = world.jobs2.parcel_stops.get(&yard).cloned().unwrap_or_default();
    v.sort();
    v.dedup();
    v
}

// ---------------------------------------------------------------------------
// Cabs and rickshaws (§ 3)
// ---------------------------------------------------------------------------

/// A Garage's cab drivers (its `contract` staff on a `fare` duty).
fn drivers(world: &World, g: EntityId) -> u32 {
    world
        .staff_of(g)
        .iter()
        .filter(|&&s| {
            world
                .comp::<Job>(s)
                .and_then(|j| crate::systems::trades::row_of(world, j.role))
                .is_some_and(|r| r.duty == TradeDuty::Fare)
        })
        .count() as u32
}

/// `lod::stat_work`, the shift's first hour (before the El's choice): an
/// off-screen commuter of `cab_min_tiles` or more holding the fare and
/// `cab_reserve` takes a ride on `p_cab` (a pure hash of the day and the
/// agent): an open rickshaw of his home district with rides left first
/// (`rickshaw_mult` × the fare to its owner), else the Garage nearest home
/// with cab drivers and rides left (`Flow::CabFare` to its owner). `true`
/// when he rode (the El's leg is skipped).
pub fn stat_cab(world: &mut World, id: EntityId, job: &Job) -> bool {
    if !on(world) {
        return false;
    }
    let tod = world.tick_of_day();
    let first = job.shifts.iter().any(|&(s, e)| tod >= s && tod < e && tod - s < TICKS_PER_HOUR as u16 && s != 0);
    // R6a: the shift's last hour rides home (`cab_roll_home`).
    // A night shift's 24:00 is not its end (it runs on from 00:00).
    let wraps = job.shifts.iter().any(|&(s, _)| s == 0);
    let last = !first
        && job
            .shifts
            .iter()
            .any(|&(s, e)| tod >= s && tod < e && e - tod <= TICKS_PER_HOUR as u16 && !(e == 1440 && wraps));
    if !first && !last {
        return false;
    }
    let ride = if first { cab_roll(world, id, job) } else { cab_roll_home(world, id, job) };
    let Some(ride) = ride else { return false };
    pay_ride(world, id, &ride);
    // Lives at work phase 4 (§ 2.3, D3): the ride booked to a driver's
    // body (the same rule as a Full rider's) and its encounter. Nothing off.
    let driver = match ride.by {
        RideBy::Cab(g) => book_cab(world, id, g, ride.from, &ride).map(|(s, _)| s),
        RideBy::Rickshaw(_) => None,
    };
    let d = world.district_of(ride.from);
    ride_encounter(world, id, driver, &ride, d);
    true
}

/// Lives at work phase 4 (§ 2.3): the cab roll, one source for both tiers
/// (a Full commuter rolls the same hash and gets the same pick as a
/// Statistical one; the coins move in `pay_ride`). The roll `stat_cab`
/// made inline until phase 4, unchanged: `p_cab` on a hash of the agent
/// and the day, `cab_min_tiles`, the fare, a working rickshaw of the home
/// district with rides left first (its fare, or no ride when the purse is
/// short), else the Garage nearest home with drivers and rides left.
pub fn cab_roll(world: &World, id: EntityId, job: &Job) -> Option<Ride> {
    roll(world, id, job, false)
}

/// The pressure pass R6a (OPEN-217): the commute's roll the other way, the
/// ride home off a shift (`stat_cab`'s last hour, `full_cab`'s GoTo home
/// from the workplace): the same rule (the purse, `cab_min_tiles`, the El
/// not shortening it or the `p_cab` roll on its own hash), picked up at the
/// work door. The commute's cab rode only in: every rider came at a shift's
/// start, so an evening driver (16-24) stood by through no rush at all.
/// `None` with `[trade_verbs]` absent.
pub fn cab_roll_home(world: &World, id: EntityId, job: &Job) -> Option<Ride> {
    world.config.trade_verbs.as_ref()?;
    roll(world, id, job, true)
}

fn roll(world: &World, id: EntityId, job: &Job, home_bound: bool) -> Option<Ride> {
    if !on(world) {
        return None;
    }
    let cfg = &world.config.jobs2;
    let day_key = if home_bound { world.day() ^ PURPOSE_CAB_HOME } else { world.day() };
    let rolled = hash(world, PURPOSE_CAB, u64::from(id.index), day_key) < cfg.p_cab;
    let el_missed = world.config.legs.as_ref().is_some_and(|l| l.fares_el_missed);
    if !rolled && !el_missed {
        return None;
    }
    let work = job.employer.and_then(|e| world.comp::<Building>(e)).map(|b| b.door)?;
    let home = crate::systems::contracts::home_door(world, id)?;
    let tiles = home.manhattan(work);
    if tiles < cfg.cab_min_tiles {
        return None;
    }
    // The pre-view pass phase 5 (the `fares` shape): a commute the El does
    // not shorten (`transit::commute_ticks` is the walk) takes the cab
    // without the roll, both tiers (the one source).
    if !rolled {
        let walk = Tick::from(tiles) * world.config.exec.move_ticks_full;
        if crate::systems::transit::commute_ticks(world, home, work) < walk {
            return None;
        }
    }
    let fare = cfg.fare_base + (tiles as f32 * cfg.fare_per_tile).round() as i64;
    let purse = coins(world, id);
    let d = world.district_of(home);
    let rickshaw = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.district == d && p.owner != id && u32::from(p.jobs_today) < cfg.rides_per_rickshaw)
        .filter(|p| kind_of_pitch(world, p) == Some(PitchKind::Rickshaw) && working(world, p))
        .map(|p| p.owner)
        .next();
    if let Some(r) = rickshaw {
        let f = ((fare as f32 * cfg.rickshaw_mult).round() as i64).max(1);
        if purse < f + cfg.cab_reserve {
            return None;
        }
        let (from, to) = if home_bound { (work, home) } else { (home, work) };
        return Some(Ride { from, to, fare: f, by: RideBy::Rickshaw(r) });
    }
    if purse < fare + cfg.cab_reserve {
        return None;
    }
    let (from, to) = if home_bound { (work, home) } else { (home, work) };
    let open = open_garages(world);
    let garage = if world.config.trade_verbs.is_some() {
        garage_for(world, &open, from, id)?
    } else {
        open.iter()
            .copied()
            .min_by_key(|&g| (world.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(from)), g))?
    };
    Some(Ride { from, to, fare, by: RideBy::Cab(garage) })
}

/// The open Garages with cab drivers and rides left today.
fn open_garages(world: &World) -> Vec<EntityId> {
    let per = world.config.jobs2.rides_per_driver;
    world
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .copied()
        .filter(|&g| crate::systems::industry::open(world, g))
        .filter(|&g| world.jobs2.cab_trips.get(&g).copied().unwrap_or(0) < drivers(world, g) * per)
        .collect()
}

/// The ticks a rider waits at the door for a driver's body.
fn fare_wait(world: &World) -> Tick {
    match world.config.legs.as_ref() {
        Some(l) => Tick::from(l.fare_wait_ticks),
        None => Tick::from(world.config.service.cab_wait_ticks),
    }
}

/// R6a: a driver's fares this shift (`fares_shift`).
fn fares_of(world: &World, s: EntityId) -> u16 {
    world.jobs2.fares_shift.get(&s).copied().unwrap_or(0)
}

/// The pressure pass R6a (OPEN-217): the wait for `rider`: an off-screen
/// rider's is `[trade_verbs] stat_fare_wait_ticks` (nobody sees him at his
/// door; a Garage keeps one driver a shift, so at the body's wait most
/// rides fell to the off-screen cab: 20k, 3 days, 240 bookings with no
/// driver on shift and most of the rest with the one driver an hour or more
/// away), a body's `fare_wait`.
fn fare_wait_for(world: &World, rider: EntityId) -> Tick {
    match world.config.trade_verbs.as_ref() {
        Some(c) if world.comp::<Brain>(rider).is_none_or(|b| b.lod == Lod::Statistical) => {
            c.stat_fare_wait_ticks.max(fare_wait(world))
        }
        _ => fare_wait(world),
    }
}

/// The pressure pass R6a (OPEN-217): the Garage a ride picked up at `at`
/// goes to: the one whose standing-by driver's body can come soonest
/// within the rider's wait (any Garage), else the nearest (its off-screen
/// cab). The commute's roll took the Garage nearest the rider's Home, so a
/// followed driver whose Garage was not the nearest drove nobody while the
/// nearest Garage's off-screen cab took the fare (20k seed 42 x 30: 396
/// rides a body drove, 3,637 unmanned; `fares_no_body` 2,096).
fn garage_for(world: &World, open: &[EntityId], at: TilePos, rider: EntityId) -> Option<EntityId> {
    let wait = fare_wait_for(world, rider);
    // The dispatch: the free body within the wait with the fewest fares this
    // shift, then the soonest (a rank's turn: the soonest alone gave the
    // driver nearest the riders every fare and his neighbours none).
    // The day's cap (`rides_per_driver`) is the off-screen cab's: a Garage
    // past it still sends its driver's body when he stands by (the cab's
    // unmanned rides filled a busy Garage's cap by the morning rush, and its
    // driver's body stood by for the rest of the shift with nothing booked).
    let all: Vec<EntityId> = if world.config.trade_verbs.is_some() {
        world
            .buildings_of_kind(BuildingKind::Garage)
            .iter()
            .copied()
            .filter(|&g| crate::systems::industry::open(world, g))
            .collect()
    } else {
        open.to_vec()
    };
    let body = all
        .iter()
        .filter_map(|&g| {
            drivers_free(world, g, at)
                .into_iter()
                .filter(|&(_, eta)| eta <= wait)
                .map(|(s, eta)| (if world.config.trade_verbs.is_some() { fares_of(world, s) } else { 0 }, eta, g))
                .min()
        })
        .min()
        .map(|(_, _, g)| g);
    body.or_else(|| {
        open.iter().copied().min_by_key(|&g| (world.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(at)), g))
    })
}

/// Lives at work phase 4 (§ 2.3): the ride's coins and counters (the
/// rickshaw's or the Garage's `Flow::CabFare`, the pitch's or the Garage's
/// rides today). Returns the coins paid.
pub fn pay_ride(world: &mut World, id: EntityId, ride: &Ride) -> i64 {
    match ride.by {
        RideBy::Rickshaw(r) => {
            let paid = ownership::pay(world, Some(id), Some(r), ride.fare, Flow::CabFare);
            if let Some(p) = world.jobs2.pitches.get_mut(&r) {
                p.jobs_today = p.jobs_today.saturating_add(1);
                p.take_today += paid;
            }
            let row = ownership::jobs2_row(world);
            row.rickshaw_trips += 1;
            row.rickshaw_fares += paid;
            paid
        }
        RideBy::Cab(g) => {
            let owner = world.owner_of(g);
            let paid = ownership::pay(world, Some(id), owner, ride.fare, Flow::CabFare);
            ownership::credit(world, g, paid);
            *world.jobs2.cab_trips.entry(g).or_default() += 1;
            let row = ownership::jobs2_row(world);
            row.cab_trips += 1;
            row.cab_fares += paid;
            paid
        }
    }
}

// ---------------------------------------------------------------------------
// Lives at work phase 4 (§ 2.3): the Full commuter's cab and the driver's body
// ---------------------------------------------------------------------------

/// The ticks a cab takes from `from` to `to`: the walk's ticks × the Car's
/// `timed_mult` (`[vehicles] mult`), at least one.
pub fn cab_ticks(world: &World, from: TilePos, to: TilePos) -> Tick {
    let base = Tick::from(from.manhattan(to)) * world.config.exec.move_ticks_full;
    let m = crate::systems::vehicles::timed_mult_of(world, crate::components::AssetKind::Car);
    ((base as f32 * m).round() as Tick).max(1)
}

/// The on-duty cab drivers of `garage` with a body (Full or Coarse) and no
/// ride pending, with the ticks each needs to reach `at`.
fn drivers_free(world: &World, garage: EntityId, at: TilePos) -> Vec<(EntityId, Tick)> {
    world
        .staff_of(garage)
        .iter()
        .copied()
        .filter(|&s| {
            standing_by(world, s)
                && !world.has::<Sentence>(s)
                && crate::systems::law::living(world, s)
                && world.comp::<Brain>(s).is_some_and(|b| b.lod != Lod::Statistical)
        })
        .filter_map(|s| {
            let from = crate::exec::walk_origin(world, s)?;
            Some((s, cab_ticks(world, from, at)))
        })
        .collect()
}

/// `exec::start_step`, a Full or Coarse commuter's `GoTo` to its workplace
/// (before the vehicle, the El and the walk): the cab roll shared with the
/// Statistical tier, once a day a rider (`Jobs2State.cab_rolled`). A ride
/// rolled: the fare paid (`pay_ride`; a rider `transit::pays_or_dodges`
/// says dodges rides `Short`: no coins, a `FareShort` event, the driver's
/// grudge), the rider's leg timed at the cab's speed, and the Garage's
/// on-duty driver with a body who can reach the rider within
/// `cab_wait_ticks` and the lowest hash (D6) takes the ride: a `CabRide`
/// his body drives (`fare_plan`), the `Fare` encounter at the pick-up.
/// With no such driver the Garage's off-screen cab takes it (deviation:
/// the rider never waits, the shift matters more than the ride). Nothing
/// off.
pub fn full_cab(world: &mut World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    // Review fix 10: the cheap checks on a borrow; the Job cloned only for the roll.
    let job = world.comp::<Job>(id)?;
    // R6a (OPEN-217): off a shift, the GoTo home from the workplace is the
    // ride home's roll, once a day (`exec_home_rolled`: an exec holds no Job,
    // so the set is free for a worker's ride home).
    if world.config.trade_verbs.is_some()
        && target.building.is_some()
        && target.building == crate::systems::missions::home_of(world, id)
        && job.employer.is_some()
        && world.comp::<Position>(id).is_some_and(|p| p.building == job.employer)
        && !job.on_shift(world.tick_of_day())
        && !world.jobs2.exec_home_rolled.contains(&id)
    {
        let job = job.clone();
        world.jobs2.exec_home_rolled.insert(id);
        let ride = cab_roll_home(world, id, &job)?;
        let here = crate::exec::walk_origin(world, id)?;
        return take_ride(world, id, target, ride, here);
    }
    if target.building.is_none() || target.building != job.employer || world.jobs2.cab_rolled.contains(&id) {
        return None;
    }
    let job = job.clone();
    world.jobs2.cab_rolled.insert(id);
    let ride = cab_roll(world, id, &job)?;
    let here = crate::exec::walk_origin(world, id)?;
    take_ride(world, id, target, ride, here)
}

/// The pressure pass phase 2 (OPEN-176, `[customers]`): a body's GoTo to a
/// building `hail_min_tiles` or more away, not to work (the commute's roll
/// is `full_cab`'s), hails a cab once a day when rich (`hail_rich_coins`)
/// or hurried (its goal is `Treat`, `Flee` or `Rescue`, or a shift starting
/// within the walk), holding the fare and `cab_reserve`, on `p_hail` (a
/// hash of the rider and the day): the Garage nearest the rider with
/// drivers and rides left sends a driver's body (`book_cab`, the pick-up
/// and the drop-off legs) or its off-screen cab, as the commuter's ride.
pub fn hail_cab(world: &mut World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    let c = crate::systems::customers::cfg(world)?.clone();
    if !crate::systems::service::street_on(world) || world.jobs2.hailed.contains(&id) {
        return None;
    }
    let to_b = target.building?;
    let here = crate::exec::walk_origin(world, id)?;
    let to = world.comp::<Building>(to_b)?.door;
    let tiles = here.manhattan(to);
    if tiles < c.hail_min_tiles {
        return None;
    }
    // Late for a shift: the walk is longer than the time left before it.
    let walk = Tick::from(tiles) * world.config.exec.move_ticks_full;
    let late = world
        .comp::<Job>(id)
        .is_some_and(|j| j.employer == Some(to_b) && Tick::from(j.ticks_until_shift(world.tick_of_day())) < walk);
    if world.comp::<Job>(id).is_some_and(|j| j.employer == Some(to_b)) && !late {
        return None;
    }
    let cfg = &world.config.jobs2;
    let fare = cfg.fare_base + (tiles as f32 * cfg.fare_per_tile).round() as i64;
    let purse = coins(world, id);
    if purse < fare + cfg.cab_reserve {
        return None;
    }
    let goal = world.comp::<Brain>(id).and_then(|b| b.current_goal);
    let hurried = late || matches!(goal, Some(GoalKind::Treat | GoalKind::Flee | GoalKind::Rescue));
    if purse < c.hail_rich_coins && !hurried {
        return None;
    }
    if hash(world, PURPOSE_HAIL, u64::from(id.index), world.day()) >= c.p_hail {
        return None;
    }
    // A hailed cab is the one a driver's body can bring soonest within the
    // rider's wait, from any Garage; else the nearest Garage's (off screen).
    let open = open_garages(world);
    let garage = garage_for(world, &open, here, id)?;
    world.jobs2.hailed.insert(id);
    ownership::jobs2_row(world).cab_hails += 1;
    let why = if late {
        "late for a shift"
    } else if hurried {
        "in a hurry"
    } else {
        "flush"
    };
    let text = format!(
        "{} hailed a cab ({why}) from {} to {}",
        world.name_of(id),
        world.district_name(world.district_of(here)),
        world.name_of(to_b)
    );
    world.push_event(EventKind::Served, &[id, garage, to_b], text);
    let ride = Ride { from: here, to, fare, by: RideBy::Cab(garage) };
    take_ride(world, id, target, ride, here)
}

/// A ride taken now from `here` (the commuter's and the hailer's): the
/// driver booked, the fare paid (or dodged), the leg timed or carried.
fn take_ride(
    world: &mut World,
    id: EntityId,
    target: &crate::exec::GotoTarget,
    ride: Ride,
    here: TilePos,
) -> Option<crate::exec::ExecState> {
    let now = world.tick;
    // `pays_or_dodges` is `Some(dodge)`: `Some(true)` rides without paying.
    let dodges = crate::systems::transit::pays_or_dodges(world, id) == Some(true);
    let mut arrive = now + cab_ticks(world, here, ride.to);
    let driver = match ride.by {
        RideBy::Cab(garage) => book_cab(world, id, garage, here, &ride),
        RideBy::Rickshaw(_) => None,
    };
    if let Some((_, eta)) = driver {
        arrive += eta;
    }
    // The pre-view pass phase 5 (the `fares` shape, OPEN-115): a driver's
    // body comes for him: he waits at his door and rides along (`carry`),
    // the fare paid on arrival and the encounter at the pick-up
    // (`fare_plan`). A dodger's short fare is as before.
    if let (Some((s, _)), RideBy::Cab(g)) = (driver.filter(|_| fares_on(world)), ride.by) {
        if let Some(r) = world.jobs2.cab_rides.iter_mut().rev().find(|r| r.rider == id && r.driver == s) {
            r.body = true;
            if dodges {
                r.fare = 0;
            }
        }
        if dodges {
            ownership::jobs2_row(world).fares_short += 1;
            let (who, at) = (world.name_of(id), world.district_name(world.district_of(here)).to_string());
            let w = world.config.service.tune.fare_dodge_grudge;
            crate::systems::grudges::add(world, s, id, crate::word::GrudgeCause::Robbed, w, 0);
            let text = format!("{who} rode {}'s cab from {at} and skipped the {} fare", world.name_of(s), ride.fare);
            world.push_event(EventKind::FareShort, &[id, s, g], text);
        }
        world.jobs2.passengers.insert(id, s);
        return Some(crate::exec::ExecState::GotoTimed {
            target: target.clone(),
            arrive_tick: Tick::MAX,
            blocked_since: None,
            walk: None,
        });
    }
    if fares_on(world) {
        ownership::jobs2_row(world).fares_unmanned += 1;
    }
    if dodges {
        ownership::jobs2_row(world).fares_short += 1;
        let (who, at) = (world.name_of(id), world.district_name(world.district_of(here)).to_string());
        match (driver, ride.by) {
            (Some((s, _)), RideBy::Cab(g)) => {
                let w = world.config.service.tune.fare_dodge_grudge;
                crate::systems::grudges::add(world, s, id, crate::word::GrudgeCause::Robbed, w, 0);
                let text =
                    format!("{who} rode {}'s cab from {at} and skipped the {} fare", world.name_of(s), ride.fare);
                world.push_event(EventKind::FareShort, &[id, s, g], text);
            }
            (_, RideBy::Cab(g)) => {
                let text = format!("{who} skipped a {} cab fare from {at}", ride.fare);
                world.push_event(EventKind::FareShort, &[id, g], text);
            }
            (_, RideBy::Rickshaw(r)) => {
                let text = format!("{who} skipped {}'s {} rickshaw fare from {at}", world.name_of(r), ride.fare);
                world.push_event(EventKind::FareShort, &[id, r], text);
            }
        }
    } else {
        pay_ride(world, id, &ride);
    }
    let d = world.district_of(here);
    ride_encounter(world, id, driver.map(|(s, _)| s), &ride, d);
    Some(crate::exec::ExecState::GotoTimed {
        target: target.clone(),
        arrive_tick: arrive,
        blocked_since: None,
        walk: None,
    })
}

/// 8d (OPEN-129, Dylan 2026-10-19): `exec::start_step`, an exec's `GoTo`
/// to his corp's Hq (`life::hq_of`), before the vehicle, the El and the
/// walk. With `[service] exec_ride`, the street on and the exec housed at
/// `[life] exec_home_tier` or better (the Spire) in a district other than
/// the Hq's (with `[routes]`, the island fix round F1: `exec_ride_min_tiles`
/// or more from it, a label not being space): the leg is timed at the Car's speed (`cab_ticks`, 4a's cab
/// leg). The corp's own road vehicle drives him when it owns one (no fare;
/// deviation: the vehicle's asset is not moved, as the off-screen cab's
/// is not), else the Garage nearest his Home with rides left sends its
/// off-screen cab and the corp pays the fare (`Flow::CabFare`, the
/// Garage's `cab_trips`). No Garage, or a corp short of the fare: he
/// walks. Once a day (`cab_rolled`). Both tiers: a Statistical exec's
/// office trip is the same `GoTo` (timed), so the mirror is the rule
/// itself. Nothing off.
pub fn exec_ride(world: &mut World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    if !crate::systems::service::street_on(world) || !world.config.service.exec_ride {
        return None;
    }
    let tier = crate::systems::life::exec_home_tier(world);
    if tier == 0 || world.has::<Job>(id) {
        return None;
    }
    let to = target.building?;
    // The pre-view pass phase 5 (OPEN-140's remainder): the ride home, from
    // anywhere outside the Home's district, once a day (`exec_home_rolled`).
    let home_ride = world.config.legs.as_ref().is_some_and(|l| l.exec_ride_home)
        && crate::systems::missions::home_of(world, id) == Some(to);
    if home_ride {
        if world.jobs2.exec_home_rolled.contains(&id) {
            return None;
        }
    } else {
        if world.jobs2.cab_rolled.contains(&id) {
            return None;
        }
        // Review fix 10: a GoTo to a corp's building whose exec he is, before the
        // scan of every corp (`Corp.buildings` and `Building.owner` agree:
        // `ownership::transfer_building`).
        let owner = world.corp_of_building(to)?;
        if world.comp::<Corp>(owner).and_then(|c| c.exec) != Some(id) {
            return None;
        }
    }
    let corp = crate::systems::life::exec_corp(world, id)?;
    let hq = crate::systems::life::office_of(world, corp, id)?;
    if !home_ride && target.building != Some(hq) {
        return None;
    }
    let home = crate::systems::missions::home_of(world, id)?;
    let hb = world.comp::<Building>(home)?;
    // The pressure pass F1: an exec not yet housed at the tier (no free
    // Spire flat near his Hq: `life::house_exec`) is driven too; the tier
    // gate walked a CEO 764 tiles across the Mid-Levels to his Lab.
    if near_by_label(world, home, hb.door, hq) {
        return None;
    }
    let home_door = hb.door;
    let here = crate::exec::walk_origin(world, id)?;
    // The pressure pass F1: the car is timed at its Coarse road share (0.88
    // min a tile against the El's 0.25), so across the island it is slower
    // than the train: a ride the El beats is not taken, and the GoTo rides
    // the El (`transit::try_ride`) or walks. A Spire exec sat 601 min in a
    // car to his Sump Hq.
    if crate::systems::transit::commute_ticks(world, here, target.tile) < cab_ticks(world, here, target.tile) {
        return None;
    }
    if home_ride {
        if near_home(world, here, home, home_door) {
            return None;
        }
        world.jobs2.exec_home_rolled.insert(id);
    } else {
        world.jobs2.cab_rolled.insert(id);
    }
    let road = |x: &crate::components::Asset| {
        x.kind.is_road_vehicle() && x.kind != crate::components::AssetKind::Truck && x.condition > 0 && !x.stolen
    };
    let corp_car = crate::systems::assets::assets_of(world, Some(corp))
        .iter()
        .any(|&v| world.comp::<crate::components::Asset>(v).is_some_and(road));
    if !corp_car {
        let cfg = &world.config.jobs2;
        let fare = cfg.fare_base + (here.manhattan(target.tile) as f32 * cfg.fare_per_tile).round() as i64;
        let garage = world
            .buildings_of_kind(BuildingKind::Garage)
            .iter()
            .copied()
            .filter(|&g| crate::systems::industry::open(world, g))
            .filter(|&g| world.jobs2.cab_trips.get(&g).copied().unwrap_or(0) < drivers(world, g) * cfg.rides_per_driver)
            .min_by_key(|&g| (world.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(home_door)), g))?;
        if world.purse(Some(corp)) < fare {
            return None;
        }
        let owner = world.owner_of(garage);
        let paid = ownership::pay(world, Some(corp), owner, fare, Flow::CabFare);
        ownership::credit(world, garage, paid);
        *world.jobs2.cab_trips.entry(garage).or_default() += 1;
        let row = ownership::jobs2_row(world);
        row.cab_trips += 1;
        row.cab_fares += paid;
        row.cab_rides_unmanned += 1;
    }
    let row = ownership::jobs2_row(world);
    row.exec_rides += 1;
    if corp_car {
        row.exec_rides_corp += 1;
    }
    if home_ride {
        row.exec_rides_home += 1;
    }
    let arrive = world.tick + cab_ticks(world, here, target.tile);
    Some(crate::exec::ExecState::GotoTimed {
        target: target.clone(),
        arrive_tick: arrive,
        blocked_since: None,
        walk: None,
    })
}

/// The pre-view pass phase 5 (OPEN-140's remainder): the ticks of the car
/// ride home `exec_ride` would give `id` from `from` to his Home's `door`
/// (an exec housed at `exec_home_tier` whose Hq stands in another district,
/// `from` outside the Home's district), the Car's speed; `None` otherwise or
/// with `[legs] exec_ride_home` off. Pure (the fare and the Garage are the
/// ride's, not the bed's).
pub fn ride_home_ticks(world: &World, id: EntityId, from: TilePos, door: TilePos) -> Option<Tick> {
    if !crate::systems::service::street_on(world)
        || !world.config.service.exec_ride
        || !world.config.legs.as_ref().is_some_and(|l| l.exec_ride_home)
    {
        return None;
    }
    let tier = crate::systems::life::exec_home_tier(world);
    if tier == 0 || world.has::<Job>(id) {
        return None;
    }
    let corp = crate::systems::life::exec_corp(world, id)?;
    let hq = crate::systems::life::office_of(world, corp, id)?;
    let home = crate::systems::missions::home_of(world, id)?;
    let hb = world.comp::<Building>(home)?;
    if near_by_label(world, home, hb.door, hq) || near_home(world, from, home, hb.door) {
        return None;
    }
    // The pressure pass F1: as `exec_ride`, no car ride the El beats.
    let car = cab_ticks(world, from, door);
    (crate::systems::transit::commute_ticks(world, from, door) >= car).then_some(car)
}

/// The exec's car's test of a Home and an Hq too near to drive: with
/// `[routes]` (the island fix round F1, OPEN-188), under `exec_ride_min_tiles`
/// door to door; off, the base's one district label (a Spire CEO 300 tiles
/// from his Spire Hq walked).
fn near_by_label(world: &World, home: EntityId, home_door: TilePos, hq: EntityId) -> bool {
    match world.config.routes.as_ref() {
        Some(r) => world.comp::<Building>(hq).is_none_or(|b| b.door.manhattan(home_door) < r.exec_ride_min_tiles),
        None => world.district_of_building(home) == world.district_of_building(hq),
    }
}

/// The ride home's test of `from` too near the Home: with `[routes]`, under
/// `exec_ride_min_tiles` of its door; off, inside its district.
fn near_home(world: &World, from: TilePos, home: EntityId, home_door: TilePos) -> bool {
    match world.config.routes.as_ref() {
        Some(r) => from.manhattan(home_door) < r.exec_ride_min_tiles,
        None => world.district_of(from) == world.district_of_building(home),
    }
}

/// Lives at work phase 4 (§ 2.3, D3, D6): a cab ride from `garage` booked
/// to a driver's body: the standing-by driver who can reach `from` within
/// `cab_wait_ticks` at cab speed with the lowest `hash(PURPOSE_FARE, rider,
/// driver)` takes a `CabRide` (`cab_rides_driven`); none, the Garage's
/// off-screen cab (`cab_rides_unmanned`). Both tiers' riders (`full_cab`,
/// `stat_cab`): the ride is world-side, the driver's body animates it.
/// Returns the driver and his ticks to the pick-up. Nothing off.
fn book_cab(
    world: &mut World,
    rider: EntityId,
    garage: EntityId,
    from: TilePos,
    ride: &Ride,
) -> Option<(EntityId, Tick)> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    let wait = fare_wait_for(world, rider);
    let free = drivers_free(world, garage, from);
    // The pre-view pass phase 5 (OPEN-115's diagnosis): why no body drives.
    if fares_on(world) && !free.iter().any(|&(_, eta)| eta <= wait) {
        let row = ownership::jobs2_row(world);
        if free.is_empty() {
            row.fares_no_body += 1;
        } else {
            row.fares_too_far += 1;
        }
    }
    // R6a: the fewest fares this shift first (`garage_for`'s dispatch), then the hash.
    let fewest = world.config.trade_verbs.is_some();
    let driver = free
        .into_iter()
        .filter(|&(_, eta)| eta <= wait)
        .map(|(s, eta)| {
            let n = if fewest { fares_of(world, s) } else { 0 };
            (n, hash(world, PURPOSE_FARE, u64::from(rider.index), u64::from(s.index)).to_bits(), s, eta)
        })
        .min()
        .map(|(_, _, s, eta)| (s, eta));
    match driver {
        Some((s, _)) => {
            let (to, fare, tick) = (ride.to, ride.fare, world.tick);
            world.jobs2.cab_rides.push(CabRide { rider, from, to, driver: s, fare, tick, stage: 0, body: false });
            ownership::jobs2_row(world).cab_rides_driven += 1;
        }
        None => ownership::jobs2_row(world).cab_rides_unmanned += 1,
    }
    driver
}

/// Lives at work phase 4 (§ 2.3): the ride's `Fare` encounter at the
/// pick-up: the booked driver's at the Garage, the rickshaw puller's in the
/// rider's district (his pitch is a tile); an unmanned cab, none. Nothing off.
fn ride_encounter(world: &mut World, rider: EntityId, driver: Option<EntityId>, ride: &Ride, d: DistrictId) {
    if !crate::systems::service::street_on(world) {
        return;
    }
    let kind = crate::components::ServeKind::Fare;
    match (driver, ride.by) {
        (Some(s), RideBy::Cab(g)) => {
            *world.jobs2.fares_shift.entry(s).or_default() += 1;
            driver_cut(world, s, g, ride.fare);
            crate::systems::service::serve(world, s, rider, g, kind, 0)
        }
        (_, RideBy::Rickshaw(r)) => crate::systems::service::encounter(world, r, rider, None, d, kind, 0),
        _ => {}
    }
}

/// Round 3 S5 (OPEN-217): the driver's body takes `[verbs] cab_fare_cut`
/// of the `fare` he drives in hand, from the Garage's owner (who took the
/// fare: `pay_ride`), told in his diary as a money line and tallied for
/// the shift's line (`fare_cuts`). A dodged fare (0) pays no cut. The
/// fare went to the corp and the shift's wage, scaled by the fares, paid
/// 0c and wrote "Paid" on a fare-less shift (`cabbie_82150`, 0c all week).
fn driver_cut(world: &mut World, s: EntityId, g: EntityId, fare: i64) {
    let Some(share) = world.config.verbs.as_ref().map(|v| v.cab_fare_cut).filter(|&c| c > 0.0) else { return };
    let cut = (fare as f32 * share).round() as i64;
    if cut <= 0 {
        return;
    }
    let owner = world.owner_of(g);
    let paid = ownership::pay(world, owner, Some(s), cut, Flow::CabFare);
    if paid > 0 {
        *world.jobs2.fare_cuts.entry(s).or_default() += paid;
        ownership::jobs2_row(world).cab_cuts += paid;
    }
}

/// The ride `id` drives now, if any (the first pending with him as
/// driver, while he can still drive it: [`can_drive`]).
fn ride_of(world: &World, id: EntityId) -> Option<usize> {
    if !can_drive(world, id) {
        return None;
    }
    world.jobs2.cab_rides.iter().position(|r| r.driver == id)
}

/// Review fix 2: `id` can still drive a booked ride: alive, free, a cab
/// driver's Job (a `fare` duty) on its shift. A ride whose driver cannot is
/// not his ([`ride_of`]) and the hour drops it ([`drop_stale_rides`]); the
/// rider's leg was timed at the booking, so he arrives all the same (the
/// off-screen cab's ride).
fn can_drive(world: &World, id: EntityId) -> bool {
    crate::systems::law::living(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Job>(id).is_some_and(|j| fare_duty(world, j) && j.on_shift(world.tick_of_day()))
        // The pre-view pass phase 5: a body drives it (a driver gone off
        // screen leaves his rider to the off-screen cab).
        && (!fares_on(world) || world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical))
}

/// Review fix 2 (`hourly`): drop the booked rides whose driver can no
/// longer drive them (the shift over, the post lost, jailed or dead).
fn drop_stale_rides(world: &mut World) {
    if world.jobs2.cab_rides.is_empty() {
        return;
    }
    let keep: Vec<bool> = world.jobs2.cab_rides.iter().map(|r| can_drive(world, r.driver)).collect();
    let gone: Vec<CabRide> = world.jobs2.cab_rides.iter().zip(&keep).filter(|(_, &k)| !k).map(|(r, _)| *r).collect();
    let mut k = keep.into_iter();
    world.jobs2.cab_rides.retain(|_| k.next().unwrap_or(true));
    for r in gone {
        release(world, &r, false);
    }
}

/// The pre-view pass phase 5 (the `fares` shape, OPEN-115): `[legs]` on.
pub fn fares_on(world: &World) -> bool {
    world.config.legs.is_some()
}

/// The `fares` shape: a body ride's rider steps off. `arrived`: at the
/// destination with the driver (the fare paid now, `pay_ride`, and the
/// rider's leg ends there); else the driver could not finish (his shift
/// over, jailed, gone off screen, the day's end): the Garage's off-screen
/// cab takes the rider on from where he stands, timed at the cab's speed,
/// paid now. No-op for a ride without a body rider.
fn release(world: &mut World, r: &CabRide, arrived: bool) {
    if !r.body || world.jobs2.passengers.remove(&r.rider).is_none() {
        return;
    }
    let target = match world.comp::<Brain>(r.rider).map(|b| &b.exec) {
        Some(crate::exec::ExecState::GotoTimed { target, arrive_tick, .. }) if *arrive_tick == Tick::MAX => {
            target.clone()
        }
        _ => return,
    };
    let garage = world.comp::<Job>(r.driver).and_then(|j| j.employer);
    if r.fare > 0 {
        if let Some(g) = garage {
            let ride = Ride { from: r.from, to: r.to, fare: r.fare, by: RideBy::Cab(g) };
            pay_ride(world, r.rider, &ride);
        }
    }
    let arrive = if arrived {
        ownership::jobs2_row(world).fares_driven += 1;
        world.tick
    } else {
        ownership::jobs2_row(world).fares_unmanned += 1;
        let here = crate::exec::walk_origin(world, r.rider).unwrap_or(r.from);
        world.tick + cab_ticks(world, here, target.tile)
    };
    let to = target.tile;
    if let Some(b) = world.comp_mut::<Brain>(r.rider) {
        b.exec = crate::exec::ExecState::GotoTimed { target, arrive_tick: arrive, blocked_since: None, walk: None };
    }
    // The island pass phase 5 (I3): the off-screen cab takes him on in the
    // vehicle he is aboard (else one comes to his door), from where it is.
    if !arrived && crate::systems::motion::on(world) {
        use crate::systems::motion::{self, RideKind};
        let (car, now, reuse) = (crate::components::AssetKind::Car, world.tick, motion::vehicle_of(world, r.rider));
        motion::board_timed(world, r.rider, RideKind::Cab, car, None, reuse, to, now, arrive, false);
    }
}

/// The island pass phase 5 (I3): the cab a body driver's next fare leg
/// drives: the one he is at the wheel of, else the one his fare is aboard.
pub fn fare_vehicle(world: &World, driver: EntityId) -> Option<EntityId> {
    if let Some(v) = crate::systems::motion::vehicle_of(world, driver) {
        return Some(v);
    }
    let r = world.jobs2.cab_rides.iter().find(|r| r.driver == driver && r.body)?;
    crate::systems::motion::vehicle_of(world, r.rider)
}

/// The `fares` shape: the driver `driver` moved; a rider aboard his ride
/// (stage 1, picked up) takes his tile, out on the street (`exec::run`
/// after the driver's step, as an escort's suspect follows the guard).
pub fn carry(world: &mut World, driver: EntityId) {
    let Some(r) = world.jobs2.cab_rides.iter().find(|r| r.driver == driver && r.body && r.stage >= 1).copied() else {
        return;
    };
    if world.jobs2.passengers.get(&r.rider) != Some(&driver) {
        return;
    }
    // The island pass phase 5 (I3): the fare climbs into the cab his driver
    // is at the wheel of; the vehicle carries him (`motion::advance`).
    if let Some(v) = crate::systems::motion::vehicle_of(world, driver) {
        crate::systems::motion::join(world, r.rider, v);
        return;
    }
    let Some(tile) = world.comp::<Position>(driver).filter(|p| p.building.is_none()).map(|p| p.tile) else { return };
    if world.comp::<Position>(r.rider).is_some_and(|p| p.building.is_some()) {
        world.leave_building(r.rider);
    }
    if let Some(p) = world.comp_mut::<Position>(r.rider) {
        p.tile = tile;
    }
}

/// The `fares` shape: `rider`'s plan was dropped while he waited or rode:
/// he leaves the cab where he stands, and the driver's ride is over (no fare).
pub fn passenger_off(world: &mut World, rider: EntityId) {
    if world.jobs2.passengers.remove(&rider).is_none() {
        return;
    }
    world.jobs2.cab_rides.retain(|r| !(r.body && r.rider == rider));
}

/// The `fares` shape: the day's end (`daily`): every rider still waiting or
/// aboard goes on by the off-screen cab before the day's rides are cleared.
fn release_all(world: &mut World) {
    if world.jobs2.passengers.is_empty() {
        return;
    }
    let rides: Vec<CabRide> = world.jobs2.cab_rides.iter().filter(|r| r.body).copied().collect();
    for r in rides {
        release(world, &r, false);
    }
    world.jobs2.passengers.clear();
}

/// `leisure::spot_of` and the stand-by Rest (`exec::actions`): `id` is on a
/// fare in his Work plan (a ride of his pending and the plan the fare's;
/// another goal's plan that pre-empted it reads its own spot).
pub fn driving(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(GoalKind::Work) && on_fare(world, id)
}

/// Lives at work phase 4: is `id` a cab driver with a ride pending (the
/// shadow tool's and `leisure::spot_of`'s question)?
pub fn on_fare(world: &World, id: EntityId) -> bool {
    crate::systems::service::street_on(world) && ride_of(world, id).is_some()
}

/// The tile of the driver's current leg: the rider's door, then the work door.
pub fn fare_tile(world: &World, id: EntityId) -> Option<TilePos> {
    let r = world.jobs2.cab_rides.get(ride_of(world, id)?)?;
    Some(if r.stage == 0 { r.from } else { r.to })
}

/// `exec::start_step`: a driver's leg is a drive, timed at the cab's speed.
pub fn fare_leg(world: &World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    // Round 3 S5: the drive to the rank is the cab's too.
    let to_rank = target.building.is_none() && rank_spot(world, id) == Some(target.tile);
    if !on_fare(world, id) && !cab_home(world, id, target) && !to_rank {
        return None;
    }
    let here = crate::exec::walk_origin(world, id)?;
    let arrive_tick = world.tick + cab_ticks(world, here, target.tile);
    Some(crate::exec::ExecState::GotoTimed { target: target.clone(), arrive_tick, blocked_since: None, walk: None })
}

/// The pressure pass R6a (OPEN-217): a cab driver on shift heading back to
/// his Garage from the street drives it (the cab is his): after a drop-off
/// he walked back (`cabbie_10678 d15`: 224 min for 227 tiles, a fare's
/// worth of the shift), standing by far from the riders.
fn cab_home(world: &World, id: EntityId, target: &crate::exec::GotoTarget) -> bool {
    let Some(job) = world.comp::<Job>(id) else { return false };
    target.building.is_some()
        && target.building == job.employer
        && fare_duty(world, job)
        && job.on_shift(world.tick_of_day())
        && crate::systems::service::street_on(world)
        && world.comp::<Position>(id).is_some_and(|p| p.building.is_none())
}

/// `plan::plan_for`'s Work arm (after the round): a cab driver on shift
/// with a ride pending drives it, one leg a plan: `GoTo(Spot)` to the
/// rider's door (stage 0), then to the rider's work door (stage 1); the
/// legs read the driver's position as `round_plan` reads a stop. With no
/// ride he stands by at the Garage (`GoTo(Workplace) → Rest`, the Rest
/// ending when a ride is his: `exec::actions`), and the shift's last
/// `ROUND_CUTOFF` is the post's (the work step and the wage, as the
/// round's). No work `ActionKind` is cut short (§ 1.3). The M13 trip roll
/// (crashes) is skipped: the Garage posts no vehicle asset for the cab
/// (deviation, § 14).
pub fn fare_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    let job = world.comp::<Job>(id)?;
    if !fare_duty(world, job) || !job.on_shift(world.tick_of_day()) {
        return None;
    }
    let garage = job.employer?;
    if let Some(i) = ride_of(world, id) {
        let here = world.comp::<Position>(id).filter(|p| p.building.is_none()).map(|p| p.tile);
        let r = world.jobs2.cab_rides[i];
        let done = if r.stage == 0 && here == Some(r.from) {
            world.jobs2.cab_rides[i].stage = 1;
            // The pre-view pass phase 5 (the `fares` shape): the pick-up,
            // both bodies at the door (P3): the encounter now, and he climbs in.
            if r.body {
                let d = world.district_of(r.from);
                let ride = Ride { from: r.from, to: r.to, fare: r.fare, by: RideBy::Cab(garage) };
                ride_encounter(world, r.rider, Some(id), &ride, d);
                carry(world, id);
            }
            false
        } else {
            r.stage >= 1 && here == Some(r.to)
        };
        if done && r.body {
            world.jobs2.cab_rides.remove(i);
            release(world, &r, true);
            return fare_plan(world, id);
        }
        if !done {
            let step = ActionInstance { action: ActionKind::GoTo(LocationKey::Spot), target: None, tile: None };
            return Some(Plan {
                goal: GoalKind::Work,
                target: Some(garage),
                steps: vec![step],
                started_tick: world.tick,
            });
        }
        world.jobs2.cab_rides.remove(i);
    }
    if !standing_by(world, id) {
        return None;
    }
    // Round 3 S5 (OPEN-217): no fare, he waits at a rank (`rank_tile`),
    // a Rest on the street there, and cruises to the next when the wait is
    // up (`rank_wait_over`). He stood by 9.8 h resting inside the Garage
    // (`cabbie_82150 d23`), the dispatch measuring every rider from there.
    if let Some(rank) = rank_tile(world, id) {
        let here = world.comp::<Position>(id).map(|p| (p.building, p.tile));
        let action = if here == Some((None, rank)) { ActionKind::Rest } else { ActionKind::GoTo(LocationKey::Spot) };
        let steps = vec![ActionInstance { action, target: None, tile: None }];
        return Some(Plan { goal: GoalKind::Work, target: Some(garage), steps, started_tick: world.tick });
    }
    let steps = vec![
        ActionInstance { action: ActionKind::GoTo(LocationKey::Workplace), target: Some(garage), tile: None },
        ActionInstance { action: ActionKind::Rest, target: None, tile: None },
    ];
    Some(Plan { goal: GoalKind::Work, target: Some(garage), steps, started_tick: world.tick })
}

/// Round 3 S5 (OPEN-217): the rank a cab driver standing by waits at: the
/// door of a building of `[verbs] cab_rank_kinds` within `cab_rank_tiles`
/// of his Garage's door, picked on a hash of the driver and the wait
/// (`cab_rank_ticks`: each wait a fresh rank, the cruise). `None` off, for
/// anyone not on a fare trade, or with no such door.
pub fn rank_tile(world: &World, id: EntityId) -> Option<TilePos> {
    let v = world.config.verbs.as_ref()?;
    if v.cab_rank_tiles == 0 || v.cab_rank_kinds.is_empty() {
        return None;
    }
    let job = world.comp::<Job>(id)?;
    if !fare_duty(world, job) {
        return None;
    }
    let door = world.comp::<Building>(job.employer?)?.door;
    let r = v.cab_rank_tiles;
    let mut doors: Vec<TilePos> = Vec::new();
    for &k in &v.cab_rank_kinds {
        for b in world.kind_grid(k).near(door, r) {
            if let Some(bd) = world.comp::<Building>(b).filter(|bd| !bd.demolished && bd.door.manhattan(door) <= r) {
                doors.push(bd.door);
            }
        }
    }
    doors.sort_unstable_by_key(|t| (t.x, t.y));
    doors.dedup();
    if doors.is_empty() {
        return None;
    }
    let slot = world.tick / v.cab_rank_ticks.max(1);
    let u = hash(world, PURPOSE_RANK, u64::from(id.index), slot);
    Some(doors[((u * doors.len() as f32) as usize).min(doors.len() - 1)])
}

/// Round 3 S5: a cab driver resting at his rank (on the street, standing
/// by) since `started` whose wait is up (`cab_rank_ticks`): he cruises on
/// (`exec::actions::finishes_early`). False off.
pub fn rank_wait_over(world: &World, id: EntityId, started: Tick) -> bool {
    let Some(t) = world.config.verbs.as_ref().map(|v| v.cab_rank_ticks.max(1)) else { return false };
    world.comp::<Position>(id).is_some_and(|p| p.building.is_none())
        && started / t != world.tick / t
        && standing_by(world, id)
        && rank_tile(world, id).is_some()
}

/// Round 3 S5: is `id` a cab driver standing by at his rank's tile (the
/// street Rest's start check, `exec::start_step`)?
pub fn at_rank(world: &World, id: EntityId) -> bool {
    standing_by(world, id)
        && world.comp::<Position>(id).is_some_and(|p| p.building.is_none() && rank_tile(world, id) == Some(p.tile))
}

/// Round 3 S5: the rank's tile as the Work plan's `GoTo(Spot)` reads it
/// (standing by, in the Work plan). `leisure::spot_of`.
pub fn rank_spot(world: &World, id: EntityId) -> Option<TilePos> {
    if world.comp::<Brain>(id).and_then(|b| b.plan_goal()) != Some(GoalKind::Work) || !standing_by(world, id) {
        return None;
    }
    rank_tile(world, id)
}

/// A `fare` duty trade's job (the cab driver).
/// The island pass phase 8 (`[island_trades] cab_quota`; OPEN-168): each
/// Garage's first `cab_quota` cab drivers by id who are on shift or within
/// the hour before it (the commute a body's), free and alive: the LOD's
/// class-2 set (`lod::budget_sets`). At 20k no cab driver ranked into the
/// body budget, so `book_cab` never found a body and the Garage's
/// off-screen cab took every fare. Empty off.
pub fn cab_body_set(world: &World) -> Vec<EntityId> {
    let Some(quota) = world.config.island_trades.as_ref().map(|c| usize::from(c.cab_quota)) else {
        return Vec::new();
    };
    if quota == 0 || !crate::systems::service::street_on(world) {
        return Vec::new();
    }
    let tod = world.tick_of_day();
    let lead = ((u64::from(tod) + TICKS_PER_HOUR) % TICKS_PER_DAY) as u16;
    let mut out = Vec::new();
    for &g in world.buildings_of_kind(BuildingKind::Garage) {
        let mut staff: Vec<EntityId> = world
            .staff_of(g)
            .iter()
            .copied()
            .filter(|&s| {
                world.comp::<Job>(s).is_some_and(|j| fare_duty(world, j) && (j.on_shift(tod) || j.on_shift(lead)))
                    && !world.has::<Sentence>(s)
                    && crate::systems::law::living(world, s)
            })
            .collect();
        staff.sort_unstable();
        out.extend(staff.into_iter().take(quota));
    }
    out
}

fn fare_duty(world: &World, job: &Job) -> bool {
    crate::systems::trades::row_of(world, job.role).is_some_and(|r| r.duty == TradeDuty::Fare)
}

/// Lives at work phase 4 (§ 2.3): is `id` a cab driver standing by for a
/// ride: on his shift's workday, before its last `ROUND_CUTOFF`, the shift
/// not closed, no ride pending (`exec::actions`' Rest runs on while true;
/// `drivers_free` reads it)? False off.
pub fn standing_by(world: &World, id: EntityId) -> bool {
    if !crate::systems::service::street_on(world) {
        return false;
    }
    let Some(job) = world.comp::<Job>(id) else { return false };
    if !fare_duty(world, job) || !job.on_shift(world.tick_of_day()) {
        return false;
    }
    let key = job.shift_key_at(world.tick);
    job.last_shift_day != Some(key)
        && crate::exec::routine::workday_of(world, id, job, key)
        && job
            .shift_end(world.tick)
            .is_some_and(|e| e.saturating_sub(world.tick) > crate::systems::trades::ROUND_CUTOFF)
        && ride_of(world, id).is_none()
}

// ---------------------------------------------------------------------------
// Tinkers (§ 3)
// ---------------------------------------------------------------------------

/// `assets::repairs`: an agent's asset at `from` due a repair costing the
/// Garage `cost`: the nearest working tinker within `tinker_reach` with
/// jobs left fixes it at `tinker_discount` × `cost` (`Flow::Asset` to the
/// tinker), or wrecks it on `tinker_fail_p` (`TinkerBotched`). `Some(true)`
/// fixed, `Some(false)` wrecked, `None` no tinker (the Garage's path).
pub fn tinker_repair(world: &mut World, asset: EntityId, owner: EntityId, from: TilePos, cost: i64) -> Option<bool> {
    if !on(world) || world.has::<Corp>(owner) || !world.has::<Wallet>(owner) {
        return None;
    }
    let cfg = world.config.jobs2.clone();
    let tinker = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.owner != owner && p.jobs_today < cfg.tinker_jobs && p.spot.manhattan(from) <= cfg.tinker_reach)
        .filter(|p| kind_of_pitch(world, p) == Some(PitchKind::Tinker) && working(world, p))
        .min_by_key(|p| (p.spot.manhattan(from), p.owner))
        .map(|p| p.owner)?;
    let price = ((cost as f32 * cfg.tinker_discount).round() as i64).max(1);
    if coins(world, owner) < price {
        return None;
    }
    let paid = ownership::pay(world, Some(owner), Some(tinker), price, Flow::Asset);
    if let Some(p) = world.jobs2.pitches.get_mut(&tinker) {
        p.jobs_today = p.jobs_today.saturating_add(1);
        p.take_today += paid;
    }
    let row = ownership::jobs2_row(world);
    row.tinker_repairs += 1;
    row.tinker_take += paid;
    // Lives at work phase 4b (§ 2.3): the tinker's regulars (the pitch is a tile).
    if crate::systems::service::street_on(world) && world.has::<crate::components::Brain>(owner) {
        if let Some(d) = world.jobs2.pitches.get(&tinker).map(|p| p.district) {
            let kind = crate::components::ServeKind::Buyer;
            crate::systems::service::encounter(world, tinker, owner, None, d, kind, paid);
        }
    }
    let botched = hash(world, PURPOSE_TINKER, u64::from(asset.index), world.day()) < cfg.tinker_fail_p;
    if botched {
        ownership::jobs2_row(world).tinker_botched += 1;
        let (who, whose) = (world.name_of(tinker), world.name_of(owner));
        world.push_event(
            EventKind::TinkerBotched,
            &[tinker, owner],
            format!("{who} botched a street repair on {whose}'s vehicle (wrecked)"),
        );
        // Phase-8 review: the vehicle is wrecked as any wreck is (its Parts,
        // the event, the despawn); setting its condition to 0 left a zombie
        // vehicle no repair pass touches again.
        if world.has::<crate::components::Asset>(asset) {
            crate::systems::assets::wreck(world, asset, "a botched street repair");
        }
    }
    Some(!botched)
}

// ---------------------------------------------------------------------------
// GoalKind::Pitch (bodies)
// ---------------------------------------------------------------------------

/// The tile a body's Pitch goal walks to: its own Pitch's spot, else (in
/// the dawn window) the nearest corner within `corner_reach`.
pub fn street_tile(world: &World, id: EntityId) -> Option<TilePos> {
    if let Some(p) = world.jobs2.pitches.get(&id) {
        return Some(p.spot);
    }
    corner_for(world, id)
}

/// The dawn window: the two hours before `corner_hour`.
fn dawn(world: &World) -> bool {
    let h = world.config.jobs2.corner_hour;
    let tod = world.tick_of_day();
    tod < h && tod + 2 * TICKS_PER_HOUR as u16 >= h
}

fn corner_for(world: &World, id: EntityId) -> Option<TilePos> {
    if !dawn(world) || !corner_ok(world, id) {
        return None;
    }
    // The parity diagnosis: only after a dawn that offered posts (and
    // `corner_reach` is short, 40 tiles). With 100 tiles every broke jobless
    // body walked across the Vats before dawn, most were sent away, and the
    // all-Full parity city's hunger-days rose 1.06 -> 1.15 (a `p_corner`
    // roll on the walk fixed parity but crowded the Spire's HangOuts and
    // cost the CEOs their known contacts in the behaviour tier).
    if world.jobs2.last_posts == 0 {
        return None;
    }
    let from = world.comp::<Position>(id).map(|p| p.tile)?;
    let cs = corners(world);
    // The pressure pass phase 2: the jobless walk farther to a shape-up.
    let reach = crate::systems::customers::cfg(world).map_or(world.config.jobs2.corner_reach, |c| c.shape_up_reach);
    corner_near(&cs, from, reach).map(|i| cs[i].1)
}

/// The Pitch goal's considerations: one's own Pitch in its hours (as Work
/// scores a shift), or the broke jobless' dawn walk to the corner (as Earn
/// scores the wallet). `None` off, or with neither.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) || world.has::<Sentence>(id) || world.has::<Job>(id) {
        return None;
    }
    let n = world.comp::<crate::components::Needs>(id)?;
    if let Some(p) = world.jobs2.pitches.get(&id) {
        let hours = in_hours(world, p) && !crate::systems::health::sick(world, id);
        return Some((
            vec![
                Consideration::new("pitch hours", can(hours), GATE),
                Consideration::new("energy", n.energy, IDENTITY),
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Logistic { k: 8.0, mid: 0.5 }),
            ],
            0.0,
        ));
    }
    corner_for(world, id)?;
    Some((
        vec![
            Consideration::new("the corner at dawn", 1.0, GATE),
            // A hungry body eats before it queues.
            Consideration::new("hunger", n.hunger, IDENTITY),
            Consideration::new("energy", n.energy, IDENTITY),
            Consideration::new("U(wealth)", urgency(n.wealth), Curve::Logistic { k: 8.0, mid: 0.4 }),
        ],
        0.0,
    ))
}

/// The Pitch goal's scripted plan: `GoTo(Spot)` (the street tile) then `Vend`.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    street_tile(world, id)?;
    let step = |action| ActionInstance { action, target: None, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot)), step(ActionKind::Vend)];
    Some(Plan { goal: GoalKind::Pitch, target: None, steps, started_tick: world.tick })
}

/// A Pitch step may start: `Vend` on the street within `pitch_reach` of the tile.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind) -> bool {
    match kind {
        ActionKind::GoTo(_) => street_tile(world, id).is_some(),
        ActionKind::Vend => {
            let Some(t) = street_tile(world, id) else { return false };
            world
                .comp::<Position>(id)
                .is_some_and(|p| p.building.is_none() && p.tile.manhattan(t) <= world.config.jobs2.pitch_reach)
        }
        _ => false,
    }
}

/// `Vend`'s ticks: an hour at the Pitch; at the corner, to the dawn pick.
pub fn vend_ticks(world: &World, id: EntityId) -> Tick {
    if world.jobs2.pitches.contains_key(&id) {
        return TICKS_PER_HOUR;
    }
    let h = world.config.jobs2.corner_hour;
    Tick::from(h.saturating_sub(world.tick_of_day()).max(1))
}

// ---------------------------------------------------------------------------
// The snapshot
// ---------------------------------------------------------------------------

/// The day's snapshot columns: the Pitches open by kind, the day labourers.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let mut by = [0u32; 3];
    for p in world.jobs2.pitches.values() {
        match kind_of_pitch(world, p) {
            Some(PitchKind::Vendor) => by[0] += 1,
            Some(PitchKind::Rickshaw) => by[1] += 1,
            Some(PitchKind::Tinker) => by[2] += 1,
            // Jobs & lives phase 4: the vice's own column (`street_pitches`).
            Some(PitchKind::Street) | None => {}
        }
    }
    let day = day_role(world).map_or(0, |r| world.workers(r).len() as u32);
    let row = ownership::jobs2_row(world);
    row.pitches = by.iter().sum();
    row.pitches_vendor = by[0];
    row.pitches_rickshaw = by[1];
    row.pitches_tinker = by[2];
    row.day_labourers = day;
}

/// The Pitch an agent owns (the inspector, the probes).
pub fn pitch_of(world: &World, id: EntityId) -> Option<&Pitch> {
    world.jobs2.pitches.get(&id)
}
