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

mod cabs;
mod couriers;
mod dawn;
mod fares;
mod pitches;
mod plan;
mod vendors;

pub use cabs::*;
pub use couriers::*;
use dawn::*;
pub use fares::*;
use pitches::*;
pub use plan::*;
pub use vendors::*;

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
    let now = world.tick;
    world.jobs2.riding.retain(|_, &mut t| t > now);
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
    // Round 4 T5 (OPEN-217): the searchers' Friends and next doors.
    crate::systems::search::hourly(world);
}
