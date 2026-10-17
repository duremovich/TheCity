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

const PURPOSE_OPEN: u64 = 0x5713_0001;
const PURPOSE_PICK: u64 = 0x5713_0002;
const PURPOSE_CORNER: u64 = 0x5713_0003;
const PURPOSE_STREET: u64 = 0x5713_00F1;
const PURPOSE_CAB: u64 = 0x5713_0004;
const PURPOSE_TINKER: u64 = 0x5713_0005;
const PURPOSE_SWEEP: u64 = 0x5713_0006;
const PURPOSE_STAT_PITCH: u64 = 0x5713_0007;

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
    if !on(world) || world.tick_of_day() != 0 {
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
            Some((g, f)) => (Some(g), f),
            None => (gang, cfg.cut_frac),
        };
        if let Some(g) = gang.filter(|_| p.take_today > 0) {
            let cut = (p.take_today as f32 * frac).round() as i64;
            let paid = ownership::pay(world, Some(o), Some(g), cut, Flow::StreetCut);
            ownership::jobs2_row(world).street_cut += paid;
            if pimp.is_some() {
                ownership::jobs2_row(world).pimp_cut += paid;
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
        if count.get(d.index()).is_none_or(|&c| c >= cfg.pitches_per_district) {
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
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    restock(world);
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
    true
}

// ---------------------------------------------------------------------------
// The labour corner (§ 3)
// ---------------------------------------------------------------------------

/// The corners: per district with an open Construction or Freight Yard,
/// the lowest-id one's door (the street outside it), ascending by district.
pub fn corners(world: &World) -> Vec<(DistrictId, TilePos)> {
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
            from.and_then(|t| corner_near(&corners, t, cfg.corner_stat_reach))
                .filter(|_| hash(world, PURPOSE_CORNER, u64::from(a.index), day) < cfg.p_corner)
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
        let best = queues[i].iter().copied().filter(|&a| !world.has::<Job>(a)).max_by(|&x, &y| {
            let known = |a: EntityId| foreman.is_some_and(|f| world.edge(a, f).is_some());
            let f = |a: EntityId| world.comp::<Skills>(a).map_or(0.0, |s| s.fighting);
            known(x).cmp(&known(y)).then(f(x).total_cmp(&f(y))).then(y.cmp(&x))
        });
        let Some(a) = best else { continue };
        queues[i].retain(|&x| x != a);
        crate::systems::demography::hire(world, a, b, role);
        if let Some(j) = world.comp_mut::<Job>(a) {
            j.until = Some(until);
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
    world
        .staff_of(yard)
        .iter()
        .filter(|&&s| {
            world
                .comp::<Job>(s)
                .and_then(|j| crate::systems::trades::row_of(world, j.role))
                .is_some_and(|r| r.model == TradeModel::Contract && r.duty == TradeDuty::Round)
        })
        .count()
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
        let yard = yards
            .iter()
            .copied()
            .filter(|y| room.get(y).is_some_and(|&r| r > 0))
            .min_by_key(|&y| (world.comp::<Building>(y).map_or(u32::MAX, |b| b.door.manhattan(from)), y));
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
        let near: Vec<EntityId> = standing
            .into_iter()
            .filter(|&b| world.comp::<Building>(b).is_some_and(|x| x.door.manhattan(ydoor) <= cfg.site_reach))
            .take(6)
            .collect();
        world.jobs2.parcel_stops.entry(y).or_default().extend(near);
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
    if !first {
        return false;
    }
    let cfg = world.config.jobs2.clone();
    if hash(world, PURPOSE_CAB, u64::from(id.index), world.day()) >= cfg.p_cab {
        return false;
    }
    let Some(work) = job.employer.and_then(|e| world.comp::<Building>(e)).map(|b| b.door) else { return false };
    let Some(home) = crate::systems::contracts::home_door(world, id) else { return false };
    let tiles = home.manhattan(work);
    if tiles < cfg.cab_min_tiles {
        return false;
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
            return false;
        }
        let paid = ownership::pay(world, Some(id), Some(r), f, Flow::CabFare);
        if let Some(p) = world.jobs2.pitches.get_mut(&r) {
            p.jobs_today = p.jobs_today.saturating_add(1);
            p.take_today += paid;
        }
        let row = ownership::jobs2_row(world);
        row.rickshaw_trips += 1;
        row.rickshaw_fares += paid;
        return true;
    }
    if purse < fare + cfg.cab_reserve {
        return false;
    }
    let garage = world
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .copied()
        .filter(|&g| crate::systems::industry::open(world, g))
        .filter(|&g| world.jobs2.cab_trips.get(&g).copied().unwrap_or(0) < drivers(world, g) * cfg.rides_per_driver)
        .min_by_key(|&g| (world.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(home)), g));
    let Some(g) = garage else { return false };
    let owner = world.owner_of(g);
    let paid = ownership::pay(world, Some(id), owner, fare, Flow::CabFare);
    ownership::credit(world, g, paid);
    *world.jobs2.cab_trips.entry(g).or_default() += 1;
    let row = ownership::jobs2_row(world);
    row.cab_trips += 1;
    row.cab_fares += paid;
    true
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
    corner_near(&cs, from, world.config.jobs2.corner_reach).map(|i| cs[i].1)
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
