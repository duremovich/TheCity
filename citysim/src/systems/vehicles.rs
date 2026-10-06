//! M13 phase 2 vehicles (docs/M13_ASSETS.md § 2, plan D19-D27, D44):
//! trips and step durations, the flyer, haul trucks and fleets, crashes,
//! theft, the fence and the chop.
//!
//! A trip is a `World::trips` entry while a vehicle is `InUse` by its
//! driver: `begin_trip` (a `GoTo` step's start) inserts it before the
//! vehicle goes `InUse`, so the rekit reads `Kit.driving` from it;
//! `end_trip` parks the vehicle and removes it (arrival, an aborted plan,
//! a death, a demotion to Statistical). Nothing here scans agents per
//! tick: a Full driver's step reads its Kit and the tile it entered; the
//! rest is event-driven or daily.

use rand::Rng;

use crate::components::{
    AssetKind, AssetLoc, Body, Brain, Building, BuildingKind, Controller, Corp, Crime, DeathCause, Gang, GoalKind, Job,
    Kit, Lod, MemoryKind, Needs, Position, Role, Sentence, ShopPick, Skills, TileKind, TilePos, Trip,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{ExecState, GotoTarget};
use crate::goap::ActionKind;
use crate::systems::assets::{self, assets_at, assets_of};
use crate::systems::ownership::{self, Flow};
use crate::time::Tick;
use crate::world::World;

// ---------------------------------------------------------------------------
// Places
// ---------------------------------------------------------------------------

/// The building a parked vehicle stands at.
pub fn parked_at(world: &World, v: EntityId) -> Option<EntityId> {
    match world.comp::<crate::components::Asset>(v)?.loc {
        AssetLoc::Parked(b) => Some(b),
        _ => None,
    }
}

/// A powered robot is posted at `b` (`robots::powered_robot`, D41).
pub fn powered_robot(world: &World, b: EntityId) -> bool {
    crate::systems::robots::powered_robot(world, b).is_some()
}

/// Plan D27: parked at a building that is not a Garage and has no powered robot.
pub fn street_parked(world: &World, v: EntityId) -> bool {
    let Some(b) = parked_at(world, v) else { return false };
    world.comp::<Building>(b).is_some_and(|bd| bd.kind != BuildingKind::Garage) && !powered_robot(world, b)
}

/// `LocationKey::Vehicle`: the street tile outside the door a vehicle is
/// parked at (`None` for anything but a parked vehicle).
pub fn vehicle_stand(world: &World, v: EntityId) -> Option<TilePos> {
    let x = world.comp::<crate::components::Asset>(v)?;
    if !x.kind.is_vehicle() {
        return None;
    }
    let AssetLoc::Parked(b) = x.loc else { return None };
    world.comp::<Building>(b).map(|bd| world.outside_door(bd))
}

/// The building whose door is nearest `from` (standing, not a Lot), ties
/// the lower id: where a trip that ends on the street parks.
pub fn nearest_parking(world: &World, from: TilePos) -> Option<EntityId> {
    world
        .buildings_by_kind
        .iter()
        .filter(|(k, _)| **k != BuildingKind::Lot)
        .flat_map(|(_, v)| v.iter())
        .filter_map(|&b| world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// The Garage nearest `from` (standing, not derelict), ties the lower id.
pub fn nearest_garage(world: &World, from: TilePos) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .filter_map(|&g| {
            world.comp::<Building>(g).filter(|b| !b.demolished && !b.derelict).map(|b| (b.door.manhattan(from), g))
        })
        .min()
        .map(|(_, g)| g)
}

fn door_of(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

// ---------------------------------------------------------------------------
// Steps (plan D19, D21, D22)
// ---------------------------------------------------------------------------

/// `[vehicles] mult` column of a tile: road 0, farmland 2, anything else
/// (ground, a door) 1.
fn column(world: &World, tile: TilePos) -> usize {
    match world.map.tile_at(tile) {
        TileKind::Road => 0,
        TileKind::Farmland => 2,
        _ => 1,
    }
}

/// D19: the quarter ticks a step onto `tile` costs: `max(1, round(4 ×
/// move_ticks_full × mult))`, `mult` the road vehicle's column when
/// driving, else `Kit.walk_mult`. An unchromed walker is exactly `4 ×
/// move_ticks_full` (8): today's arithmetic. Litter is added by the caller.
pub fn step_q(world: &World, agent: EntityId, tile: TilePos) -> u32 {
    let base = 4 * world.config.exec.move_ticks_full as u32;
    let Some(k) = world.comp::<Kit>(agent) else { return base };
    let mult = match k.driving {
        Some(kind) if kind.is_road_vehicle() => world.config.vehicles.mult_of(kind, column(world, tile)),
        _ => k.walk_mult,
    };
    if mult == 1.0 {
        return base;
    }
    ((base as f32 * mult).round() as u32).max(1)
}

/// A driver entered `tile`: count the step, the Road tiles, and the
/// trip's midpoint (step `half`). Walkers are skipped by the caller.
pub fn note_step(world: &mut World, agent: EntityId, tile: TilePos) {
    let road = world.map.tile_at(tile) == TileKind::Road;
    if let Some(t) = world.trips.get_mut(&agent) {
        t.steps = t.steps.saturating_add(1);
        if road {
            t.road_tiles = t.road_tiles.saturating_add(1);
        }
        if t.mid.is_none() && t.steps == t.half {
            t.mid = Some(tile);
        }
    }
}

/// D21: a Coarse trip's multiplier: `road_share × mult_road + (1 −
/// road_share) × mult_ground` when driving a road vehicle, else
/// `Kit.walk_mult` (1.0 walking unchromed).
pub fn timed_mult(world: &World, agent: EntityId) -> f32 {
    let Some(k) = world.comp::<Kit>(agent) else { return 1.0 };
    match k.driving {
        Some(kind) if kind.is_road_vehicle() => timed_mult_of(world, kind),
        _ => k.walk_mult,
    }
}

/// D21 for one road vehicle kind.
pub fn timed_mult_of(world: &World, kind: AssetKind) -> f32 {
    let c = &world.config.vehicles;
    c.road_share * c.mult_of(kind, 0) + (1.0 - c.road_share) * c.mult_of(kind, 1)
}

/// D22: the flyer takes off from the street outside its building and lands
/// at the door after `max(1, ceil(chebyshev × flyer_ticks_per_tile))`.
pub fn fly(world: &mut World, agent: EntityId, target: GotoTarget) -> ExecState {
    let from = crate::exec::walk_origin(world, agent).unwrap_or(target.tile);
    world.leave_building(agent);
    if let Some(p) = world.comp_mut::<Position>(agent) {
        p.tile = from;
        p.building = None;
    }
    let d = crate::systems::law::chebyshev(from, target.tile) as f32;
    // Less a hair, so 30 tiles at 0.3 is 9 ticks, not f32's 9.0000003 → 10.
    let ticks = ((d * world.config.vehicles.flyer_ticks_per_tile - 1e-4).ceil().max(1.0)) as Tick;
    let depart = world.tick;
    ExecState::Fly { target, from, depart, arrive_tick: depart + ticks }
}

// ---------------------------------------------------------------------------
// Trips (plan D20, D24)
// ---------------------------------------------------------------------------

fn asset(world: &World, v: EntityId) -> Option<&crate::components::Asset> {
    world.comp::<crate::components::Asset>(v)
}

/// D20: the vehicle a trip from here would use: (1) one already `InUse`
/// by the agent (a stolen car, a claimed haul truck); (2) its own or kept
/// vehicle parked at the building it is in; (3) a fleet vehicle parked at
/// its employer or its gang's Hideout, owned by that building's owner,
/// with no keeper (lowest id). Phase 2 deviation: trucks never serve rule
/// (3) (they are the Farm's, claimed by `HaulToMarket`), nor do stolen
/// vehicles waiting for the chop.
pub fn vehicle_for_trip(world: &World, agent: EntityId) -> Option<EntityId> {
    let usable = |x: &crate::components::Asset| x.kind.is_vehicle() && x.condition > 0;
    if let Some(v) = assets_at(world, agent)
        .iter()
        .copied()
        .find(|&a| asset(world, a).is_some_and(|x| usable(x) && x.loc == AssetLoc::InUse(agent)))
    {
        return Some(v);
    }
    let here = world.comp::<Position>(agent).and_then(|p| p.building)?;
    let parked_here = |x: &crate::components::Asset| usable(x) && x.loc == AssetLoc::Parked(here);
    if let Some(v) = assets_of(world, Some(agent))
        .iter()
        .copied()
        .find(|&a| asset(world, a).is_some_and(|x| parked_here(x) && x.keeper.is_none_or(|k| k == agent)))
    {
        return Some(v);
    }
    let at_here = assets_at(world, here);
    if let Some(v) =
        at_here.iter().copied().find(|&a| asset(world, a).is_some_and(|x| parked_here(x) && x.keeper == Some(agent)))
    {
        return Some(v);
    }
    let employer = world.comp::<Job>(agent).and_then(|j| j.employer);
    let hideout = world.gang_of(agent).and_then(|g| world.hideout_of(g));
    if Some(here) != employer && Some(here) != hideout {
        return None;
    }
    let owner = world.owner_of(here)?;
    at_here.iter().copied().find(|&a| {
        asset(world, a).is_some_and(|x| {
            parked_here(x)
                && x.owner == Some(owner)
                && x.keeper.is_none()
                && !matches!(x.kind, AssetKind::Truck | AssetKind::Flyer)
                && !x.stolen
        })
    })
}

/// D25: the driver is chasing or being chased: fleeing (a Flee plan or a
/// HideFromLaw step ahead), a guard on an Arrest, or pinned by god `Chase`.
pub fn chase_now(world: &World, agent: EntityId) -> bool {
    fleeing(world, agent)
        || (crate::systems::law::is_guard(world, agent)
            && world.comp::<Brain>(agent).and_then(|b| b.plan_goal()) == Some(GoalKind::Arrest))
        || world.chase_pins.contains(&agent)
}

/// D25: a Flee plan, or a plan with a HideFromLaw step (a kill is Murder).
pub fn fleeing(world: &World, agent: EntityId) -> bool {
    world
        .comp::<Brain>(agent)
        .and_then(|b| b.plan.as_ref())
        .is_some_and(|p| p.goal == GoalKind::Flee || p.steps.iter().any(|s| s.action == ActionKind::HideFromLaw))
}

/// Insert (or restart) the agent's trip in `v` from `from` to `target`,
/// then put the vehicle `InUse` (the Trip first: the rekit reads it).
fn start_trip(world: &mut World, agent: EntityId, v: EntityId, from: TilePos, to: TilePos) {
    let half = u16::try_from(from.manhattan(to) / 2).unwrap_or(u16::MAX);
    let chase = chase_now(world, agent);
    let start = world.tick;
    world.trips.insert(agent, Trip { vehicle: v, start, from, road_tiles: 0, steps: 0, half, mid: None, chase });
    if asset(world, v).is_some_and(|x| x.loc == AssetLoc::InUse(agent)) {
        assets::rekit(world, agent);
    } else {
        assets::set_loc(world, v, AssetLoc::InUse(agent));
    }
}

/// D20: a `GoTo` step starts. Picks the vehicle (`vehicle_for_trip`, or the
/// one a running trip already holds), files the trip, keeps a fleet
/// vehicle for the driver (D24) and returns its kind; `None` walks.
pub fn begin_trip(world: &mut World, agent: EntityId, target: &GotoTarget) -> Option<AssetKind> {
    if !world.config.assets.enabled {
        return None;
    }
    let from = crate::exec::walk_origin(world, agent)?;
    let running = world.trips.get(&agent).map(|t| t.vehicle);
    let v = match running {
        Some(v) if asset(world, v).is_some_and(|x| x.loc == AssetLoc::InUse(agent) && x.condition > 0) => v,
        _ => {
            if running.is_some() {
                world.trips.remove(&agent);
            }
            vehicle_for_trip(world, agent)?
        }
    };
    let x = asset(world, v)?.clone();
    // D24 rule (3): a fleet vehicle is the driver's until it is home.
    if x.keeper.is_none()
        && x.owner != Some(agent)
        && x.owner.is_some_and(|o| world.has::<Corp>(o) || world.has::<Gang>(o))
    {
        assets::set_keeper(world, v, Some(agent));
    }
    start_trip(world, agent, v, from, target.tile);
    Some(x.kind)
}

/// D20: the trip ends. The vehicle parks in the building the driver is in
/// (the destination on arrival), else at the door nearest the driver; a
/// corp fleet vehicle's keeper is cleared at the keeper's workplace (phase
/// 2 deviation from "any building of its owner": a haul truck parked at the
/// corp's Market must stay kept so the hauler drives it home); a vehicle
/// flagged for repossession while driven is towed now (D11); an arrival
/// rolls the crash (D25).
pub fn end_trip(world: &mut World, agent: EntityId, arrived: bool) {
    let Some(trip) = world.trips.remove(&agent) else { return };
    // A god `Chase` pin lasts one trip, arrived or not.
    let pinned = world.chase_pins.remove(&agent);
    let v = trip.vehicle;
    let Some(x) = asset(world, v).cloned() else {
        assets::rekit(world, agent);
        return;
    };
    if x.loc != AssetLoc::InUse(agent) {
        assets::rekit(world, agent);
        return;
    }
    let pos = world.comp::<Position>(agent).map(|p| (p.tile, p.building));
    let park = pos.and_then(|(t, b)| b.or_else(|| nearest_parking(world, t)));
    let Some(park) = park else {
        assets::rekit(world, agent);
        return;
    };
    assets::set_loc(world, v, AssetLoc::Parked(park));
    if let Some(k) = x.keeper {
        // An exec's flyer (phase 5) stays the exec's.
        let corp_fleet = x.kind != AssetKind::Flyer && x.owner.is_some_and(|o| world.has::<Corp>(o));
        let workplace = world.comp::<Job>(k).and_then(|j| j.employer);
        // A trip aborted away from work keeps its keeper here; the soft
        // form of the abort-time keeper clear is in `fleet_recall` (phase
        // 5): a kept corp car parked away from its owner's buildings for 2
        // midnights is recalled. (The hard form, clearing at the abort,
        // shifted seed 42's regime: see the phase 2 fix commit.)
        if corp_fleet && workplace == Some(park) {
            assets::set_keeper(world, v, None);
        }
    }
    let door = door_of(world, park).unwrap_or(trip.from);
    if arrived && x.kind.is_road_vehicle() {
        crash_roll(world, agent, &trip, door, x.kind, pinned);
    }
    let repo_days = world.config.assets.repo_days;
    let flagged =
        asset(world, v).and_then(|x| x.finance.as_ref()).is_some_and(|f| f.arrears >= repo_days && f.lender.is_some());
    if flagged {
        assets::tow(world, v);
    }
}

/// D46: an agent demoted to Statistical: its trip ends, its own vehicles
/// parked away from Home go home with it, and a gang vehicle it keeps goes
/// back to the Hideout.
pub fn on_demoted(world: &mut World, agent: EntityId) {
    end_trip(world, agent, false);
    // A gang vehicle it keeps goes back to the Hideout (phase 2 addition: a
    // bike left at a Bar door was theft fodder until the keeper came back).
    let kept: Vec<(EntityId, EntityId)> = world
        .vehicles
        .iter()
        .copied()
        .filter_map(|v| {
            let x = asset(world, v).filter(|x| x.keeper == Some(agent) && matches!(x.loc, AssetLoc::Parked(_)))?;
            let h = x.owner.filter(|&o| world.has::<Gang>(o)).and_then(|g| world.hideout_of(g))?;
            (x.loc != AssetLoc::Parked(h)).then_some((v, h))
        })
        .collect();
    for (v, h) in kept {
        assets::set_loc(world, v, AssetLoc::Parked(h));
    }
    let Some(home) = world.comp::<crate::components::Household>(agent).and_then(|h| h.home) else { return };
    let own: Vec<EntityId> = assets_of(world, Some(agent))
        .iter()
        .copied()
        .filter(|&a| {
            asset(world, a).is_some_and(|x| x.kind.is_vehicle() && matches!(x.loc, AssetLoc::Parked(b) if b != home))
        })
        .collect();
    for v in own {
        assets::set_loc(world, v, AssetLoc::Parked(home));
    }
}

// ---------------------------------------------------------------------------
// Crashes (plan D25)
// ---------------------------------------------------------------------------

fn midpoint(a: TilePos, b: TilePos) -> TilePos {
    TilePos { x: ((u16::from(a.x) + u16::from(b.x)) / 2) as u8, y: ((u16::from(a.y) + u16::from(b.y)) / 2) as u8 }
}

/// Body reflex plus the Kit's (the dodge and the driver's term).
fn reflex(world: &World, id: EntityId) -> f32 {
    world.comp::<Body>(id).map_or(0.0, |b| b.reflex) + world.comp::<Kit>(id).map_or(0.0, |k| k.reflex)
}

/// D25 / D35: the chance a body dodges a car or a bullet.
pub fn p_dodge(world: &World, id: EntityId) -> f32 {
    let c = &world.config.vehicles;
    (c.dodge_w * reflex(world, id)).min(c.dodge_cap)
}

/// D25 at an arrival: `p = crash_per_tile × road_tiles × speed × (1 +
/// litter) × chase × (1 − 0.5 × min(1, reflex))` on the world stream.
fn crash_roll(world: &mut World, driver: EntityId, trip: &Trip, door: TilePos, kind: AssetKind, pinned: bool) {
    let cfg = world.config.vehicles.clone();
    let speed = *cfg.speed.get(kind);
    if speed <= 0.0 || cfg.crash_per_tile <= 0.0 {
        return;
    }
    let road = if trip.steps > 0 {
        f32::from(trip.road_tiles)
    } else {
        (cfg.road_share * trip.from.manhattan(door) as f32).round()
    };
    let tile = trip.mid.unwrap_or_else(|| midpoint(trip.from, door));
    let litter =
        if crate::systems::litter::enabled(world) { world.district(world.district_of(tile)).litter } else { 0.0 };
    // The chase is the trip's, decided when it started (fix round: not the
    // driver's plan at arrival), or a god pin.
    let chased = trip.chase || pinned;
    let chase = if chased { cfg.chase_mult } else { 1.0 };
    let p = cfg.crash_per_tile * road * speed * (1.0 + litter) * chase * (1.0 - 0.5 * reflex(world, driver).min(1.0));
    if p <= 0.0 {
        return;
    }
    let roll: f32 = world.rng.world().random();
    if roll >= p {
        return;
    }
    // A kill on a chase is Murder unless the driver was the law giving
    // chase (a guard on an Arrest); else Manslaughter.
    let murder = chased && !crate::systems::law::is_guard(world, driver);
    crash(world, driver, trip.vehicle, tile, murder);
}

/// A crash at `tile`: the nearest body within `crash_radius` on the street
/// that is not driving (else one Statistical adult of the district) may
/// dodge, die (`p_crash_kill × (1 − armour)`) or be hurt; the vehicle
/// loses `crash_wear`; litter; Manslaughter (Murder when fleeing) for a
/// kill; the `Crash` event.
pub fn crash(world: &mut World, driver: EntityId, v: EntityId, tile: TilePos, murder: bool) {
    let cfg = world.config.vehicles.clone();
    world.stats.current.crashes += 1;
    let near = world
        .bodies()
        .into_iter()
        .filter(|&b| b != driver && crate::systems::law::living(world, b) && !world.has::<Sentence>(b))
        .filter(|&b| world.comp::<Kit>(b).is_none_or(|k| k.driving.is_none()))
        .filter_map(|b| {
            let p = world.comp::<Position>(b).filter(|p| p.building.is_none())?;
            let d = crate::systems::law::chebyshev(p.tile, tile);
            (d <= cfg.crash_radius).then_some((d, b))
        })
        .min()
        .map(|(_, b)| b);
    let victim = near.or_else(|| {
        let d = world.district_of(tile);
        let pool: Vec<EntityId> = world
            .district(d)
            .residents
            .iter()
            .copied()
            .filter(|&a| {
                crate::systems::law::living(world, a)
                    && !world.has::<Sentence>(a)
                    && world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical)
                    && crate::systems::demography::is_adult(world, a)
            })
            .collect();
        if pool.is_empty() {
            return None;
        }
        let i = world.rng.world().random_range(0..pool.len());
        Some(pool[i])
    });
    let (dn, what, place) =
        (world.name_of(driver), world.name_of(v), world.district_name(world.district_of(tile)).to_string());
    let mut killed = false;
    let text = match victim {
        None => format!("{dn}'s {what} crashed in {place}"),
        Some(vic) => {
            let vn = world.name_of(vic);
            let dodge: f32 = world.rng.world().random();
            if dodge < p_dodge(world, vic) {
                format!("{dn}'s {what} missed {vn} in {place}")
            } else {
                let armour = world.comp::<Kit>(vic).map_or(0.0, |k| k.armour);
                let kill: f32 = world.rng.world().random();
                killed = kill < cfg.p_crash_kill * (1.0 - armour);
                if !killed {
                    world.remember(vic, MemoryKind::Crashed, Some(driver), 0.7, -0.7, false);
                    if let Some(n) = world.comp_mut::<Needs>(vic) {
                        n.safety = (n.safety - 0.6).max(0.0);
                        n.energy = (n.energy - 0.3).max(0.0);
                    }
                }
                format!("{dn}'s {what} hit {vn} in {place}{}", if killed { ": killed" } else { "" })
            }
        }
    };
    let actors = [driver, victim.unwrap_or(EntityId::NONE), v];
    world.push_event(EventKind::Crash, &actors, text);
    if let (true, Some(vic)) = (killed, victim) {
        world.kill_by(vic, DeathCause::Accident, Some(driver));
        if crate::systems::law::living(world, driver) {
            let crime = if murder { Crime::Murder } else { Crime::Manslaughter };
            crate::systems::law::raise_crime(world, driver, None, crime, tile);
        }
    }
    crate::systems::litter::deposit(world, tile, cfg.crash_litter, 1);
    let worn = asset(world, v).map(|x| x.condition.saturating_sub(cfg.crash_wear));
    if let Some(c) = worn {
        if let Some(m) = world.comp_mut::<crate::components::Asset>(v) {
            m.condition = c;
        }
        if c == 0 {
            assets::wreck(world, v, "crash");
        }
    }
}

// ---------------------------------------------------------------------------
// Hauls and fleets (plan D23, D24, D44)
// ---------------------------------------------------------------------------

/// D23: a Truck, else a Car, parked at the Farm, owned by its owner, no
/// keeper, lowest id.
pub fn fleet_vehicle_at(world: &World, farm: EntityId) -> Option<EntityId> {
    if !world.config.assets.enabled {
        return None;
    }
    let owner = world.owner_of(farm);
    let pick = |kind: AssetKind| {
        assets_at(world, farm).iter().copied().find(|&a| {
            asset(world, a).is_some_and(|x| {
                x.kind == kind
                    && x.loc == AssetLoc::Parked(farm)
                    && x.owner == owner
                    && owner.is_some()
                    && x.keeper.is_none()
                    && x.condition > 0
                    && !x.stolen
            })
        })
    };
    pick(AssetKind::Truck).or_else(|| pick(AssetKind::Car))
}

/// D23: `HaulToMarket` starts at the Farm: claim its fleet vehicle (the
/// hauler keeps it and the trip starts here, so an abort parks it) and
/// return the batch multiplier (1 on foot).
pub fn claim_haul(world: &mut World, hauler: EntityId, farm: EntityId) -> Option<u32> {
    let v = fleet_vehicle_at(world, farm)?;
    let kind = asset(world, v)?.kind;
    let from = door_of(world, farm).and_then(|_| crate::exec::walk_origin(world, hauler))?;
    assets::set_keeper(world, v, Some(hauler));
    start_trip(world, hauler, v, from, from);
    Some((*world.config.vehicles.haul.get(kind)).max(1))
}

/// D23 for a Statistical farmer's haul: the Farm's fleet vehicle carries
/// the batch without moving (the trip is off screen). Phase 2 deviation:
/// the plan is silent on Statistical hauls, and most farmers are
/// Statistical, so the truck share would otherwise read the Full few.
pub fn stat_haul_mult(world: &World, farm: EntityId) -> Option<u32> {
    let v = fleet_vehicle_at(world, farm)?;
    asset(world, v).map(|x| (*world.config.vehicles.haul.get(x.kind)).max(1))
}

/// A corp's vehicles of `kind` (ascending).
fn corp_vehicles(world: &World, corp: EntityId, kind: AssetKind) -> Vec<EntityId> {
    assets_of(world, Some(corp)).iter().copied().filter(|&a| asset(world, a).is_some_and(|x| x.kind == kind)).collect()
}

/// A corp's standing buildings of `kind`, ascending.
fn corp_buildings(world: &World, corp: EntityId, kind: BuildingKind) -> Vec<EntityId> {
    world
        .comp::<Corp>(corp)
        .map(|c| {
            c.buildings
                .iter()
                .copied()
                .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == kind && !bd.demolished))
                .collect()
        })
        .unwrap_or_default()
}

/// Buy `kind` T1 for `corp` at the Garage nearest `home` (a used one in its
/// stock first) and park it at `home`. Needs the list price in the purse.
fn corp_buy(world: &mut World, corp: EntityId, kind: AssetKind, home: EntityId, why: &str) -> bool {
    let Some(from) = door_of(world, home) else { return false };
    let Some(g) = nearest_garage(world, from) else { return false };
    let used = assets_at(world, g)
        .iter()
        .copied()
        .find(|&a| asset(world, a).is_some_and(|x| x.kind == kind && x.loc == AssetLoc::Stock(g) && x.condition > 0));
    let list = assets::list_price(world, kind, 1).unwrap_or(i64::MAX);
    let (closing, reserve) = world.comp::<Corp>(corp).map_or((0, 0), |c| (c.closing, c.treasury_ref / 4));
    if world.purse(Some(corp)).min(closing) < list.saturating_add(reserve) {
        return false;
    }
    let pick = ShopPick { kind, tier: 1, used };
    let note = format!("for {}", world.name_of(home));
    match assets::buy_noted(world, corp, g, &pick, Some(&note)) {
        Ok(v) => {
            assets::set_loc(world, v, AssetLoc::Parked(home));
            let _ = why;
            true
        }
        Err(_) => false,
    }
}

/// D44: a Food corp's truck for its first Farm with none parked there.
fn buy_truck(world: &mut World, corp: EntityId) -> bool {
    let farms = corp_buildings(world, corp, BuildingKind::Farm);
    let trucks = corp_vehicles(world, corp, AssetKind::Truck);
    if trucks.len() >= farms.len() {
        return false;
    }
    let has_truck = |f: EntityId| trucks.iter().any(|&t| asset(world, t).is_some_and(|x| x.loc == AssetLoc::Parked(f)));
    let Some(farm) = farms.iter().copied().find(|&f| !has_truck(f)) else { return false };
    corp_buy(world, corp, AssetKind::Truck, farm, "haul")
}

/// D44: a Security corp's car, one per three guards employed at an Office.
fn buy_office_car(world: &mut World, corp: EntityId) -> bool {
    let offices = corp_buildings(world, corp, BuildingKind::SecurityOffice);
    let cars = corp_vehicles(world, corp, AssetKind::Car);
    let want: usize = offices
        .iter()
        .map(|&o| {
            world
                .workers(Role::Guard)
                .iter()
                .filter(|&&g| world.comp::<Job>(g).and_then(|j| j.employer) == Some(o))
                .count()
                / 3
        })
        .sum();
    if cars.len() >= want {
        return false;
    }
    let parked =
        |o: EntityId| cars.iter().filter(|&&c| asset(world, c).is_some_and(|x| x.loc == AssetLoc::Parked(o))).count();
    let Some(office) = offices.iter().copied().min_by_key(|&o| (parked(o), o)) else { return false };
    corp_buy(world, corp, AssetKind::Car, office, "patrol")
}

/// D44 Hunker: the newest fleet vehicle not on the road goes to the nearest
/// Garage at `buyback_frac × value` (`Flow::Sale`), into its stock.
fn sell_newest(world: &mut World, corp: EntityId) -> bool {
    let newest = assets_of(world, Some(corp))
        .iter()
        .copied()
        .filter_map(|a| {
            asset(world, a)
                .filter(|x| x.kind.is_vehicle() && matches!(x.loc, AssetLoc::Parked(_)))
                .map(|x| (x.bought, a))
        })
        .max();
    let Some((_, v)) = newest else { return false };
    let Some(from) = assets::asset_tile(world, v) else { return false };
    let Some(g) = nearest_garage(world, from) else { return false };
    let buyer = world.owner_of(g);
    if buyer == Some(corp) {
        return false;
    }
    let value = asset(world, v).map_or(0, |x| x.value);
    let price = (world.config.chrome.buyback_frac * value as f32).round() as i64;
    if !matches!(ownership::owner_kind(world, buyer), ownership::OwnerKind::City | ownership::OwnerKind::Corp(_))
        && world.purse(buyer) < price
    {
        return false;
    }
    ownership::charge(world, buyer, Some(corp), price, Flow::Sale);
    assets::set_keeper(world, v, None);
    assets::set_owner(world, v, buyer);
    assets::set_loc(world, v, AssetLoc::Stock(g));
    let (who, what, at) = (world.owner_label(buyer), world.name_of(v), world.name_of(g));
    let seller = world.owner_label(Some(corp));
    world.push_event(
        EventKind::AssetBought,
        &[buyer.unwrap_or(EntityId::NONE), g, v],
        format!("{who} bought {seller}'s {what} at {at} for {price} (hunkering)"),
    );
    true
}

/// D44, daily from `corp_brain::run`: one purchase a day inside the
/// standing order. Grow(Food) (or any order but Hunker with
/// `fleet_any_order`): a truck; Grow(Security) (phase 2 deviation: also any
/// order but Hunker with `fleet_any_order`): an Office car; Hunker: sell
/// the newest fleet vehicle.
pub fn corp_fleet(world: &mut World, corp: EntityId) {
    if !world.config.assets.enabled {
        return;
    }
    let Some((order, niche, food, security)) = world.comp::<Corp>(corp).map(|c| {
        (
            c.order,
            c.order_niche,
            c.niches.contains(&crate::components::Niche::Food),
            c.niches.contains(&crate::components::Niche::Security),
        )
    }) else {
        return;
    };
    use crate::components::{CorpOrder, Niche};
    if order == CorpOrder::Hunker {
        // Phase 2 deviation: only a corp losing money over a week and short
        // of cash sells (its books' close, `Corp.closing`, under a quarter
        // of its treasury reference). Hunker is the quiet default, and a
        // truck's own price reads as a losing week: a truck bought under
        // Secure was sold back at a loss the next day. The close, not the
        // live treasury: at midnight a corp sits a day's upkeep lump below it.
        let poor = world.comp::<Corp>(corp).is_some_and(|c| {
            c.cashflow.len() >= 7 && c.cashflow.iter().sum::<i64>() < 0 && c.closing * 4 < c.treasury_ref
        });
        if poor {
            sell_newest(world, corp);
        }
        return;
    }
    // Outside Grow too (D44's `fleet_any_order`); `corp_buy` keeps a
    // quarter of the treasury reference in the books after the price (the
    // Hunker sale's bar, so a purchase never triggers one).
    let any = world.config.shop.fleet_any_order;
    if ((order == CorpOrder::Grow && niche == Some(Niche::Food)) || (any && food)) && buy_truck(world, corp) {
        return;
    }
    if ((order == CorpOrder::Grow && niche == Some(Niche::Security)) || (any && security))
        && buy_office_car(world, corp)
    {
        return;
    }
    if any || order == CorpOrder::Grow {
        exec_flyer(world, corp);
    }
}

/// Phase 5 (the acceptance's Spire execs flying over the Sump): a corp
/// whose exec has no vehicle hands it the corp's keeper-less flyer, or buys
/// one (cash only, as every flyer) when its treasury and books hold
/// `[shop] exec_flyer_cash_mult × price` plus the fleet reserve. Owned by
/// the corp (upkeep to its books), kept by the exec for good, parked at
/// the exec's Home.
fn exec_flyer(world: &mut World, corp: EntityId) -> bool {
    let mult = world.config.shop.exec_flyer_cash_mult;
    if mult <= 0.0 {
        return false;
    }
    let Some(exec) = world.comp::<Corp>(corp).and_then(|c| c.exec) else { return false };
    if !crate::systems::law::living(world, exec)
        || world.has::<crate::components::Sentence>(exec)
        || world.comp::<crate::components::Kit>(exec).is_some_and(|k| k.vehicle.is_some())
    {
        return false;
    }
    let Some(home) = world
        .comp::<crate::components::Household>(exec)
        .and_then(|h| h.home)
        .or_else(|| world.comp::<Job>(exec).and_then(|j| j.employer))
    else {
        return false;
    };
    let flyers = corp_vehicles(world, corp, AssetKind::Flyer);
    if let Some(&f) = flyers.iter().find(|&&f| asset(world, f).is_some_and(|x| x.keeper.is_none())) {
        assets::set_keeper(world, f, Some(exec));
        return true;
    }
    if !flyers.is_empty() {
        return false;
    }
    let list = assets::list_price(world, AssetKind::Flyer, 1).unwrap_or(i64::MAX);
    let (closing, reserve) = world.comp::<Corp>(corp).map_or((0, 0), |c| (c.closing, c.treasury_ref / 4));
    let need = (list as f32 * mult).round() as i64;
    if world.purse(Some(corp)).min(closing) < need.saturating_add(reserve) {
        return false;
    }
    let Some(g) = door_of(world, home).and_then(|d| nearest_garage(world, d)) else { return false };
    let pick = ShopPick { kind: AssetKind::Flyer, tier: 1, used: None };
    let note = format!("for {}", world.name_of(exec));
    match assets::buy_noted(world, corp, g, &pick, Some(&note)) {
        Ok(v) => {
            assets::set_loc(world, v, AssetLoc::Parked(home));
            assets::set_keeper(world, v, Some(exec));
            true
        }
        Err(_) => false,
    }
}

/// Daily (phase 2 addition): fleet vehicles stranded by an aborted trip go
/// home. A corp truck not parked at one of its Farms goes to its Farm with
/// the fewest trucks parked (ties lower id), keeper cleared; a keeper-less
/// corp car not at one of its Offices goes to the nearest; a keeper-less
/// gang vehicle goes to the Hideout.
pub fn fleet_recall(world: &mut World) {
    for v in world.vehicles.clone() {
        let Some(x) = asset(world, v).cloned() else { continue };
        let AssetLoc::Parked(at) = x.loc else { continue };
        let Some(owner) = x.owner else { continue };
        if world.has::<Corp>(owner) {
            // Fix round: a keeper who is dead, jailed or no longer works for
            // the owner keeps nothing.
            let stale = x.keeper.is_some_and(|k| {
                !crate::systems::law::living(world, k)
                    || world.has::<crate::components::Sentence>(k)
                    || world.comp::<Job>(k).and_then(|j| j.employer).and_then(|e| world.owner_of(e)) != Some(owner)
            });
            if stale {
                assets::set_keeper(world, v, None);
            }
            let mut keeper = if stale { None } else { x.keeper };
            // Phase 5 (the soft abort-time keeper clear): a kept car parked
            // away from every building of its owner for 2 midnights is
            // recalled like a keeper-less one.
            if x.kind == AssetKind::Car && keeper.is_some() {
                let away = world.owner_of(at) != Some(owner);
                let days = if away { x.away_days.saturating_add(1) } else { 0 };
                if let Some(m) = world.comp_mut::<crate::components::Asset>(v) {
                    m.away_days = days;
                }
                if days >= 2 {
                    assets::set_keeper(world, v, None);
                    if let Some(m) = world.comp_mut::<crate::components::Asset>(v) {
                        m.away_days = 0;
                    }
                    keeper = None;
                }
            }
            let (kind, home_kind) = match x.kind {
                AssetKind::Truck => (x.kind, BuildingKind::Farm),
                AssetKind::Car if keeper.is_none() => (x.kind, BuildingKind::SecurityOffice),
                _ => continue,
            };
            let homes = corp_buildings(world, owner, home_kind);
            if homes.contains(&at) || homes.is_empty() {
                continue;
            }
            let same = corp_vehicles(world, owner, kind);
            let parked = |h: EntityId| {
                same.iter().filter(|&&c| asset(world, c).is_some_and(|y| y.loc == AssetLoc::Parked(h))).count()
            };
            let from = door_of(world, at).unwrap_or_default();
            let home = if kind == AssetKind::Truck {
                homes.iter().copied().min_by_key(|&h| (parked(h), h))
            } else {
                homes.iter().copied().min_by_key(|&h| (door_of(world, h).map_or(u32::MAX, |d| d.manhattan(from)), h))
            };
            if let Some(h) = home {
                assets::set_keeper(world, v, None);
                assets::set_loc(world, v, AssetLoc::Parked(h));
            }
        } else if world.has::<Gang>(owner) && x.keeper.is_none() {
            if let Some(h) = world.hideout_of(owner).filter(|&h| h != at) {
                assets::set_loc(world, v, AssetLoc::Parked(h));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Theft, the fence, the chop (plan D26, D27)
// ---------------------------------------------------------------------------

/// Fix round: a member leaving its gang hands back the gang vehicles it
/// keeps: a trip in one ends where it stands, then it goes home to the
/// Hideout, keeper cleared.
pub fn return_gang_vehicles(world: &mut World, agent: EntityId, gang: EntityId) {
    let kept: Vec<EntityId> = world
        .vehicles
        .iter()
        .copied()
        .filter(|&v| asset(world, v).is_some_and(|x| x.keeper == Some(agent) && x.owner == Some(gang)))
        .collect();
    if kept.is_empty() {
        return;
    }
    if world.trips.get(&agent).is_some_and(|t| kept.contains(&t.vehicle)) {
        end_trip(world, agent, false);
    }
    let home = world.hideout_of(gang);
    for v in kept {
        assets::set_keeper(world, v, None);
        if let Some(h) = home {
            if asset(world, v).is_some_and(|x| matches!(x.loc, AssetLoc::Parked(_))) {
                assets::set_loc(world, v, AssetLoc::Parked(h));
            }
        }
    }
}

/// The agent's own vehicle, or its gang's, or one it keeps, or (phase 5)
/// its household's: a spouse or housemate parked at the same Home. Seed 42
/// had a wife steal her husband's bike from their door 9 times, each one
/// recovered home when no gang could pay the fence (37 thefts of one bike).
fn own_or_gang(world: &World, agent: EntityId, x: &crate::components::Asset) -> bool {
    if x.owner == Some(agent) || x.keeper == Some(agent) || (x.owner.is_some() && x.owner == world.gang_of(agent)) {
        return true;
    }
    let Some(o) = x.owner.filter(|&o| world.has::<crate::components::Household>(o)) else { return false };
    let home = |a: EntityId| world.comp::<crate::components::Household>(a).and_then(|h| h.home);
    world.spouse_of(agent) == Some(o) || home(agent).is_some_and(|h| home(o) == Some(h))
}

/// D26: may `agent` steal `v` now: a street-parked, working vehicle that is
/// not its own, its gang's or kept by it, within `steal_reach`, and not
/// already stolen (phase 2: an abandoned stolen car was stolen again and
/// again, 77 times in one run; `recover_abandoned` returns it instead).
pub fn may_steal(world: &World, agent: EntityId, v: EntityId) -> bool {
    let Some(pos) = world.comp::<Position>(agent).map(|p| p.tile) else { return false };
    stealable_at(world, agent, v, pos).is_some()
}

/// [`may_steal`]'s test from `pos`, returning the stand's distance. Phase 5
/// (throughput): the cheap tests and the reach first, the household and
/// robot lookups last; `steal_target` runs it over every vehicle per think
/// of a lawless adult (190 vehicles cost ~7 us an agent before).
fn stealable_at(world: &World, agent: EntityId, v: EntityId, pos: TilePos) -> Option<u32> {
    let x = asset(world, v)?;
    if !x.kind.is_vehicle() || x.condition == 0 || x.stolen {
        return None;
    }
    let AssetLoc::Parked(b) = x.loc else { return None };
    let bd = world.comp::<Building>(b)?;
    if bd.kind == BuildingKind::Garage {
        return None;
    }
    let d = world.outside_door(bd).manhattan(pos);
    if d > world.config.vehicles.steal_reach || own_or_gang(world, agent, x) || powered_robot(world, b) {
        return None;
    }
    Some(d)
}

/// D26: the nearest vehicle the agent may steal (ties lower id).
pub fn steal_target(world: &World, agent: EntityId) -> Option<EntityId> {
    if !world.config.assets.enabled {
        return None;
    }
    let pos = world.comp::<Position>(agent)?.tile;
    world
        .vehicles
        .iter()
        .copied()
        .filter_map(|v| stealable_at(world, agent, v, pos).map(|d| (d, v)))
        .min()
        .map(|(_, v)| v)
}

/// D26: would this agent plan a vehicle theft at all (an Earn binding)?
pub fn would_steal(world: &World, agent: EntityId) -> bool {
    world.config.assets.enabled
        && !crate::systems::law::is_guard(world, agent)
        && world.comp::<crate::components::Personality>(agent).is_some_and(|p| p.lawfulness < 0.4)
        && crate::systems::demography::is_adult(world, agent)
}

/// D26: the Earn goal has a theft to plan: an eligible thief, a vehicle in
/// reach, and a gang at its Hideout that can pay the fence (else the plan
/// is hopeless and the goal only cools).
pub fn theft_plannable(world: &World, agent: EntityId) -> bool {
    would_steal(world, agent) && steal_target(world, agent).is_some_and(|v| can_fence(world, agent, Some(v)))
}

/// A stolen vehicle the agent holds and has not fenced: kept by it and
/// driven by it, or parked where it stands.
pub fn stolen_held_by(world: &World, agent: EntityId) -> Option<EntityId> {
    if world.vehicles.is_empty() {
        return None;
    }
    let gang = world.gang_of(agent);
    let held = |a: &EntityId| {
        asset(world, *a).is_some_and(|x| {
            x.kind.is_vehicle() && x.stolen && x.keeper == Some(agent) && (x.owner.is_none() || x.owner != gang)
        })
    };
    let here = world.comp::<Position>(agent).and_then(|p| p.building);
    let mine = assets_at(world, agent).iter().copied().find(held);
    mine.or_else(|| here.and_then(|b| assets_at(world, b).iter().copied().find(held)))
}

fn fence_price(world: &World, v: EntityId) -> i64 {
    let value = asset(world, v).map_or(0, |x| x.value);
    (world.config.vehicles.fence_frac * value as f32).round() as i64
}

/// The gang that owns the Hideout `LocationKey::Hideout` means for this agent.
fn fence_gang(world: &World, agent: EntityId) -> Option<(EntityId, EntityId)> {
    let h = crate::systems::gang::hideout_for(world, agent)?;
    let g = world.owner_of(h).filter(|&g| world.has::<Gang>(g))?;
    Some((g, h))
}

/// D26: a gang at the agent's Hideout can pay for the vehicle it holds (or
/// the one bound to steal).
pub fn can_fence(world: &World, agent: EntityId, target: Option<EntityId>) -> bool {
    if !world.config.assets.enabled {
        return false;
    }
    let v = stolen_held_by(world, agent)
        .or_else(|| target.filter(|&t| asset(world, t).is_some_and(|x| x.kind.is_vehicle())));
    let (Some(v), Some((g, _))) = (v, fence_gang(world, agent)) else { return false };
    world.purse(Some(g)) >= fence_price(world, v)
}

/// D26 at `StealVehicle`'s completion: the lock contest (`thief_tier = 1 +
/// round(2 × stealth)` against the vehicle's tier); Grand Theft is raised
/// either way; a win drives off with it (stolen, kept by the thief, a trip
/// begun so an abort parks it).
pub fn steal(world: &mut World, thief: EntityId, v: EntityId) -> bool {
    let Some(x) = asset(world, v).cloned() else { return false };
    if !x.kind.is_vehicle() || x.condition == 0 || own_or_gang(world, thief, &x) || !street_parked(world, v) {
        return false;
    }
    let Some(door) = vehicle_stand(world, v) else { return false };
    let stealth =
        world.comp::<Skills>(thief).map_or(0.0, |s| s.stealth) + world.comp::<Kit>(thief).map_or(0.0, |k| k.stealth);
    let tier = (1.0 + (2.0 * stealth).round()).clamp(1.0, 255.0) as u8;
    let step = world.config.chrome.contest_step;
    let won = crate::systems::security::contest(tier, x.tier, step, world.rng.world());
    let victim = x.owner.filter(|&o| world.has::<Brain>(o));
    crate::systems::law::raise_crime(world, thief, victim, Crime::GrandTheft, door);
    if !won || !crate::systems::law::living(world, thief) {
        return false;
    }
    if let Some(m) = world.comp_mut::<crate::components::Asset>(v) {
        m.stolen = true;
    }
    assets::set_keeper(world, v, Some(thief));
    start_trip(world, thief, v, door, door);
    let (tn, on, what) = (world.name_of(thief), world.owner_label(x.owner), world.name_of(v));
    let place = world.district_name(world.district_of(door)).to_string();
    world.push_event(
        EventKind::VehicleStolen,
        &[thief, x.owner.unwrap_or(EntityId::NONE), v],
        format!("{tn} stole {on}'s {what} in {place}"),
    );
    world.stats.current.vehicle_thefts += 1;
    true
}

/// D26 `Fence` of a vehicle at a Hideout: its gang pays `fence_frac ×
/// value` (`Flow::Sale`) and takes it (owner the gang, parked there, still
/// stolen). Phase 2 deviation: a member is paid too (the "member's
/// arrival" handover rides its plan's Fence step).
pub fn fence(world: &mut World, agent: EntityId) -> bool {
    let Some(v) = stolen_held_by(world, agent) else { return false };
    let Some(h) = world.comp::<Position>(agent).and_then(|p| p.building) else { return false };
    let Some(g) = world
        .comp::<Building>(h)
        .filter(|b| b.kind == BuildingKind::Hideout)
        .and_then(|b| b.owner)
        .filter(|&g| world.has::<Gang>(g))
    else {
        return false;
    };
    let price = fence_price(world, v);
    if world.purse(Some(g)) < price {
        return false;
    }
    end_trip(world, agent, false);
    ownership::pay(world, Some(g), Some(agent), price, Flow::Sale);
    assets::set_keeper(world, v, None);
    assets::set_owner(world, v, Some(g));
    assets::set_loc(world, v, AssetLoc::Parked(h));
    if let Some(m) = world.comp_mut::<crate::components::Asset>(v) {
        m.finance = None;
        m.upkeep_arrears = 0;
    }
    let (gn, tn, what) = (world.owner_label(Some(g)), world.name_of(agent), world.name_of(v));
    world.push_event(EventKind::AssetBought, &[g, h, v], format!("{gn} fenced {tn}'s stolen {what} for {price}"));
    true
}

/// Daily, before the fleet recall and the theft roll: a stolen vehicle left
/// parked by a thief who never fenced it (an arrest, a plan given up) is
/// recovered: `stolen` and the keeper cleared (the Asset's "cleared by a
/// chop, a fence or recovery"); an agent's goes back to its Home, a corp's
/// to the recall. A gang's (fenced or stolen off screen) stays stolen
/// until chopped (D26).
pub fn recover_abandoned(world: &mut World) {
    for v in world.vehicles.clone() {
        let Some(x) = asset(world, v).cloned() else { continue };
        if !x.stolen || !matches!(x.loc, AssetLoc::Parked(_)) || x.owner.is_some_and(|o| world.has::<Gang>(o)) {
            continue;
        }
        if let Some(m) = world.comp_mut::<crate::components::Asset>(v) {
            m.stolen = false;
        }
        assets::set_keeper(world, v, None);
        let home = x.owner.and_then(|o| world.comp::<crate::components::Household>(o)).and_then(|h| h.home);
        if let Some(h) = home {
            assets::set_loc(world, v, AssetLoc::Parked(h));
        }
    }
}

/// The gang a vehicle stolen off screen at `tile` goes to: the district's
/// controller, else the gang whose Hideout door is nearest.
fn theft_gang(world: &World, tile: TilePos) -> Option<(EntityId, EntityId)> {
    if let Controller::Gang(g) = world.district(world.district_of(tile)).control {
        if let Some(h) = world.hideout_of(g).filter(|_| world.has::<Gang>(g)) {
            return Some((g, h));
        }
    }
    world
        .gangs()
        .into_iter()
        .filter_map(|g| {
            let h = world.hideout_of(g)?;
            Some((door_of(world, h)?.manhattan(tile), g, h))
        })
        .min()
        .map(|(_, g, h)| (g, h))
}

/// D27, daily from the assets pass: every parked vehicle not owned by a
/// Full or Coarse agent (nor a gang) rolls `vehicle_theft_base × (crime
/// rate ÷ the inhabited mean) × (2 − coverage)` (× `garage_mult` in a
/// Garage, none behind a powered robot) on the world stream; a hit goes to
/// the district's gang (else the nearest Hideout's), stolen, into its stock.
pub fn theft_daily(world: &mut World) {
    let cfg = world.config.vehicles.clone();
    if cfg.vehicle_theft_base <= 0.0 || world.gangs().is_empty() {
        return;
    }
    let mean = crate::systems::law_brain::mean_crime_rate(world);
    for v in world.vehicles.clone() {
        let Some(x) = asset(world, v).cloned() else { continue };
        let AssetLoc::Parked(b) = x.loc else { continue };
        if x.stolen || x.condition == 0 || powered_robot(world, b) {
            continue;
        }
        if let Some(o) = x.owner {
            if world.has::<Gang>(o) {
                continue;
            }
            if world.comp::<Brain>(o).is_some_and(|br| br.lod != Lod::Statistical) {
                continue;
            }
        }
        let Some(bd) = world.comp::<Building>(b) else { continue };
        let garage = bd.kind == BuildingKind::Garage;
        let door = bd.door;
        let d = world.district(world.district_of(door));
        let rate = if mean > 0.0 { d.crime_rate / mean } else { 1.0 };
        let p = cfg.vehicle_theft_base * rate * (2.0 - d.coverage) * if garage { cfg.garage_mult } else { 1.0 };
        if p <= 0.0 {
            continue;
        }
        let roll: f32 = world.rng.world().random();
        if roll >= p.min(1.0) {
            continue;
        }
        let Some((g, h)) = theft_gang(world, door) else { continue };
        assets::set_keeper(world, v, None);
        assets::set_owner(world, v, Some(g));
        assets::set_loc(world, v, AssetLoc::Stock(h));
        if let Some(m) = world.comp_mut::<crate::components::Asset>(v) {
            m.stolen = true;
            m.finance = None;
            m.upkeep_arrears = 0;
        }
        let (on, what, gn) = (world.owner_label(x.owner), world.name_of(v), world.owner_label(Some(g)));
        let place = world.district_name(world.district_of(door)).to_string();
        world.push_event(
            EventKind::VehicleStolen,
            &[EntityId::NONE, x.owner.unwrap_or(EntityId::NONE), v],
            format!("{gn} stole {on}'s {what} in {place}"),
        );
        world.stats.current.vehicle_thefts += 1;
        if let Some(o) = x.owner.filter(|&o| world.has::<Brain>(o)) {
            world.remember(o, MemoryKind::WasRobbed, None, 0.6, -0.6, false);
        }
    }
}

/// The gang's vehicle quota: `floor(bikes_max_share × members)`.
fn quota(world: &World, gang: EntityId) -> usize {
    let members = world.comp::<Gang>(gang).map_or(0, |g| g.members.len());
    (world.config.vehicles.bikes_max_share * members as f32).floor() as usize
}

/// The highest-ranked free member without a vehicle (`gang::leader_ranking`).
fn rider_wanted(world: &World, gang: EntityId) -> Option<EntityId> {
    crate::systems::gang::leader_ranking(world, gang)
        .into_iter()
        .find(|&m| world.comp::<Kit>(m).is_some_and(|k| k.vehicle.is_none()))
}

/// D27, daily in the gang pass: the gang keeps stolen bikes and cars while
/// its vehicles are under quota (each to its best-ranked member without a
/// vehicle, parked at the Hideout) and chops the rest, trucks and flyers
/// always: `parts_per` Parts into its Hideout stock, `Chopped`, despawned.
pub fn chop_daily(world: &mut World, gang: EntityId) {
    if !world.config.assets.enabled {
        return;
    }
    let Some(h) = world.hideout_of(gang) else { return };
    let mine: Vec<EntityId> = assets_of(world, Some(gang))
        .iter()
        .copied()
        .filter(|&a| asset(world, a).is_some_and(|x| x.kind.is_vehicle()))
        .collect();
    let stolen: Vec<EntityId> = mine.iter().copied().filter(|&a| asset(world, a).is_some_and(|x| x.stolen)).collect();
    if stolen.is_empty() {
        return;
    }
    let limit = quota(world, gang);
    let mut kept = mine.len() - stolen.len();
    for v in stolen {
        let Some(x) = asset(world, v).cloned() else { continue };
        if matches!(x.loc, AssetLoc::InUse(_)) {
            kept += 1;
            continue;
        }
        let keepable = matches!(x.kind, AssetKind::Motorcycle | AssetKind::Car) && x.condition > 0;
        // Kept only with a rider: one already, or a member without a vehicle.
        let rider = x.keeper.or_else(|| rider_wanted(world, gang));
        if keepable && kept < limit && rider.is_some() {
            kept += 1;
            if x.keeper.is_none() {
                assets::set_keeper(world, v, rider);
            }
            if matches!(x.loc, AssetLoc::Stock(_)) {
                assets::set_loc(world, v, AssetLoc::Parked(h));
            }
            continue;
        }
        let parts = *world.config.assets.parts_per.get(x.kind);
        world.add_stock(h, crate::components::Good::Parts, parts);
        let (gn, what) = (world.owner_label(Some(gang)), world.name_of(v));
        world.push_event(EventKind::Chopped, &[gang, v], format!("{gn} chopped a {what} for {parts} Parts"));
        world.stats.current.chops += 1;
        assets::despawn(world, v);
    }
}

/// D44, daily in the gang pass after the chop: with `gang_buy_floor` in the
/// treasury, one motorcycle for the best-ranked member without a vehicle,
/// up to quota: owned by the gang, kept by the member, parked at the Hideout.
pub fn gang_bikes(world: &mut World, gang: EntityId) {
    if !world.config.assets.enabled || world.purse(Some(gang)) < world.config.shop.gang_buy_floor {
        return;
    }
    let Some(h) = world.hideout_of(gang) else { return };
    let have =
        assets_of(world, Some(gang)).iter().filter(|&&a| asset(world, a).is_some_and(|x| x.kind.is_vehicle())).count();
    if have >= quota(world, gang) {
        return;
    }
    let Some(member) = rider_wanted(world, gang) else { return };
    let Some(from) = door_of(world, h) else { return };
    let Some(g) = nearest_garage(world, from) else { return };
    let pick = ShopPick { kind: AssetKind::Motorcycle, tier: 1, used: None };
    let note = format!("for {}", world.name_of(member));
    if let Ok(v) = assets::buy_noted(world, gang, g, &pick, Some(&note)) {
        assets::set_keeper(world, v, Some(member));
        assets::set_loc(world, v, AssetLoc::Parked(h));
    }
}
