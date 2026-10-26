//! Measurement only (the pre-view pass phase 5, P10's diagnosis): every
//! Full or Coarse `GoTo` leg's start and end, so a run can list the long
//! legs by who, where, how and why no ride. `World::leg_log` is `None` in
//! the sim (never saved, read by nothing in the sim); the CLI's `run
//! --legs FILE` turns it on.

use std::collections::BTreeMap;

use crate::components::{Lod, TilePos};
use crate::entity::EntityId;
use crate::exec::GotoTarget;
use crate::goap::LocationKey;
use crate::time::Tick;
use crate::world::World;

/// One open leg.
#[derive(Clone, Debug)]
pub struct LegStart {
    pub tick: Tick,
    pub from: TilePos,
    pub to: TilePos,
    pub key: LocationKey,
    pub lod: Lod,
    pub mode: &'static str,
    pub why: String,
    pub key_plan: Option<(Tick, u8)>,
    /// Segments (and other re-starts) after the first.
    pub hops: u32,
    /// An El leg's structural estimate at its start (`transit::commute_ticks`).
    pub est: Tick,
}

/// A finished leg at or over `min_ticks`.
#[derive(Clone, Debug)]
pub struct LegRow {
    pub id: EntityId,
    pub who: String,
    pub goal: String,
    pub start: LegStart,
    pub ticks: Tick,
    pub outcome: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct LegLog {
    pub min_ticks: Tick,
    pub open: BTreeMap<EntityId, LegStart>,
    pub rows: Vec<LegRow>,
    /// Every leg ended, by mode: (count, ticks summed).
    pub by_mode: BTreeMap<&'static str, (u64, u64)>,
    /// The 03:00 census by day: execs asleep at home, asleep away, awake;
    /// guards the same (`night_census`).
    pub nights: Vec<(u64, [u32; 6])>,
    /// The island pass phase 2 (the Hideout/Home ping-pong): by (night,
    /// leader), the ticks asleep, the sleeps begun, the bed switches (a
    /// sleep begun in another building than the night's last) and the ticks
    /// a leader with a body; the night runs 18:00 to 18:00 (`leader_tick`).
    pub leaders: BTreeMap<(u64, EntityId), [u32; 4]>,
    /// Each leader's last sleep: (night, building, asleep last tick).
    pub leader_last: BTreeMap<EntityId, (u64, Option<EntityId>, bool)>,
    /// The island pass phase 5 (OPEN-165's device, `ride_tick`): a body on
    /// a vehicle leg, last seen.
    pub ride_track: BTreeMap<EntityId, RideSeen>,
    /// By (day, body, mode, Full): [parked, jumped] on a vehicle leg that day.
    pub ride_days: BTreeMap<(u64, EntityId, &'static str, bool), [bool; 2]>,
    /// The island pass phase 6 (`transit::note_train`): riders left on a
    /// full train's platform, plans dropped between stops (carried on), riders
    /// set down by a cut line.
    pub trains: [u64; 3],
    /// The island pass phase 6's device: an El rider's ticks this ride on
    /// the platform (the transfers' too) and aboard (`ride_tick`).
    pub el_acc: BTreeMap<EntityId, [u32; 2]>,
    /// Every finished El ride's [platform, aboard] ticks.
    pub el_trips: Vec<[u32; 2]>,
    /// Every El leg done: its estimate at the start and its ticks.
    pub el_fit: Vec<(Tick, Tick)>,
    /// The island pass phase 9 (OPEN-181's device, `walk_tick`): each
    /// Coarse walker last tick.
    pub walk_track: BTreeMap<EntityId, WalkSeen>,
    /// Each walk's way, by walker (`exec::walk_route`, kept by its depart).
    pub walk_ways: BTreeMap<EntityId, KeptWay>,
    /// The device's sums (`#walk` rows).
    pub walks: WalkSums,
}

/// A walk's way kept by its walker: (depart, way, arrive).
pub type KeptWay = (Tick, Vec<TilePos>, Tick);

/// `walk_tick`'s walker now: (id, to, arrive, Position, (depart, arrive)
/// when the walk has a start).
type WalkerNow = (EntityId, TilePos, Tick, crate::components::Position, Option<(Tick, Tick)>);

/// `walk_tick`'s last sight of a Coarse walker.
#[derive(Clone, Copy, Debug)]
pub struct WalkSeen {
    /// The walk: its target tile and arrival.
    pub to: TilePos,
    pub arrive: Tick,
    /// Its `Position` when first seen on it (the base's start).
    pub start: TilePos,
    pub first: Tick,
    /// Where it is drawn (`exec::walk_place`), since when.
    pub tile: TilePos,
    pub since: Tick,
    pub parked: bool,
}

/// OPEN-181's device sums.
#[derive(Clone, Debug, Default)]
pub struct WalkSums {
    /// Walker-ticks seen on the street, and those sharing a tile with a
    /// walker bound elsewhere.
    pub ticks: u64,
    pub stacked: u64,
    /// The most walkers bound for at least two places on one tile, its tick
    /// and tile.
    pub stack_max: (u32, Tick, TilePos),
    /// The same, of walkers standing still there `WALK_STILL_TICKS` (the
    /// parked stack).
    pub still: u64,
    pub still_max: (u32, Tick, TilePos),
    /// Walks seen, and those whose body stood `RIDE_PARKED_TICKS` on one
    /// tile mid-walk.
    pub walks: u64,
    pub parked: u64,
    /// Promotions to Full mid-walk; of them, a body whose first Full tile is
    /// more than `RIDE_JUMP_TILES` from where it was drawn, and a body
    /// restarted at the walk's start past its first tick.
    pub promoted: u64,
    pub promo_jumps: u64,
    pub promo_restart: u64,
}

/// `ride_tick`'s last sight of a rider.
#[derive(Clone, Copy, Debug)]
pub struct RideSeen {
    pub tile: TilePos,
    pub since: Tick,
    pub last: Tick,
    pub mode: &'static str,
    pub full: bool,
}

/// The device's parked stretch: a body on a vehicle leg on one tile this
/// long (measurement only).
pub const RIDE_PARKED_TICKS: Tick = 60;
/// The device's jump: a body on a vehicle leg moved farther than this in
/// one tick (a car makes ~1-3 tiles a tick, a flyer ~3-4; measurement only).
pub const RIDE_JUMP_TILES: u32 = 12;
/// The walk device's still stack: walkers on one tile this long
/// (measurement only).
pub const WALK_STILL_TICKS: Tick = 4;

/// The vehicle legs' modes (`start`'s): the cab, a body driver's fare leg,
/// the exec's car, the El, a flyer, a drive.
fn vehicle_mode(mode: &str) -> bool {
    matches!(mode, "cab" | "fare_leg" | "exec_ride" | "el" | "fly" | "drive" | "escort")
}

/// Where a body that left its leg stands: inside a building, its door (the
/// step in to an interior slot is no jump); else its tile.
fn landed_at(world: &World, id: EntityId) -> Option<TilePos> {
    let p = world.comp::<crate::components::Position>(id)?;
    match p.building.and_then(|b| world.comp::<crate::components::Building>(b)) {
        Some(b) => Some(b.door),
        None => Some(p.tile),
    }
}

/// Every tick (`run --legs`, the island pass phase 5's device, OPEN-165):
/// each Full or Coarse body on a vehicle leg (a timed leg, a flight, an El
/// ride, a drive) is watched; a body-day is "parked" when it sat on one
/// tile `RIDE_PARKED_TICKS` or more while on the leg, "jumped" when it moved
/// more than `RIDE_JUMP_TILES` in one tick (the landing included: the tick
/// after the leg ended). `#ride` rows of the TSV. Measurement only.
pub fn ride_tick(world: &mut World) {
    let Some(log) = world.leg_log.as_ref() else { return };
    let tick = world.tick;
    let day = tick / crate::time::TICKS_PER_DAY;
    // (body, mode, Full, tile, a stand here counts as parked).
    let mut seen: Vec<(EntityId, &'static str, bool, TilePos, bool)> = Vec::new();
    let mut el_seen: Vec<(EntityId, bool)> = Vec::new();
    for (&id, s) in &log.open {
        if !vehicle_mode(s.mode) {
            continue;
        }
        let Some(b) = world.comp::<crate::components::Brain>(id) else { continue };
        if b.lod == Lod::Statistical {
            continue;
        }
        use crate::exec::ExecState;
        // The mode this tick: the El's ride, a flight, the vehicle he is
        // aboard (`motion`), else the leg's own.
        // The El's device (phase 6): aboard a train, or past the departure
        // without trains, is a ride tick; else the platform (a cut line's
        // walk is neither).
        if let ExecState::Ride { depart, .. } = &b.exec {
            if world.transit.evac.is_empty() || !world.transit.evac.contains_key(&id) {
                let aboard = if crate::systems::transit::trains_on(world) {
                    world.aboard.contains_key(&id)
                } else {
                    tick >= *depart
                };
                el_seen.push((id, aboard));
            }
        }
        let mode = match &b.exec {
            ExecState::Ride { .. } => "el",
            ExecState::Fly { .. } => "fly",
            ExecState::GotoTimed { .. } | ExecState::Goto { .. } => {
                match world.aboard.get(&id).and_then(|v| world.motions.get(v)) {
                    Some(m) => match m.ride {
                        crate::systems::motion::RideKind::Car => "drive",
                        crate::systems::motion::RideKind::Flyer => "fly",
                        crate::systems::motion::RideKind::Cab if m.driver == Some(id) => "fare_leg",
                        crate::systems::motion::RideKind::Cab => "cab",
                        crate::systems::motion::RideKind::Exec => "exec_ride",
                        crate::systems::motion::RideKind::Train => "el",
                        // The island pass phase 7: the escort's van, the
                        // escort led on foot.
                        crate::systems::motion::RideKind::Van => "van",
                        crate::systems::motion::RideKind::Foot => "escort",
                    },
                    // The island pass phase 6: the walk to or from the El
                    // (a Coarse walk is timed) is the walk's, not the El's.
                    None if s.mode == "el" => "el_walk",
                    None => s.mode,
                }
            }
            _ => continue,
        };
        let Some(p) = world.comp::<crate::components::Position>(id) else { continue };
        // A body set down on an El stop this tick (`transit::run` alights
        // after the step) is the El's: its jump is the El's (phase 6).
        let mode = if world.transit.net.stops.contains(&p.tile)
            && log.ride_track.get(&id).is_some_and(|r| r.tile != p.tile && r.mode == "el")
        {
            "el"
        } else {
            mode
        };
        // Inside a building, its door: the step out of (or in to) a deep
        // interior slot is no jump.
        let Some(at) = landed_at(world, id) else { continue };
        seen.push((id, mode, b.lod == Lod::Full, at, true));
        // The island pass phase 7's device: the escorted suspect (the
        // arrest's hand-off), aboard the van with the guard or led.
        if s.mode == "escort" {
            if let Some(sus) = b.escorting {
                let sb = world.comp::<crate::components::Brain>(sus);
                if let (Some(sb), Some(at)) = (sb, landed_at(world, sus)) {
                    if sb.lod != Lod::Statistical && sb.cuffed_by.is_some() {
                        let mode = match world.aboard.get(&sus).and_then(|v| world.motions.get(v)) {
                            Some(m) if m.ride == crate::systems::motion::RideKind::Van => "van",
                            _ => "arrest",
                        };
                        seen.push((sus, mode, sb.lod == Lod::Full, at, true));
                    }
                }
            }
        }
    }
    // The island pass phase 7's device: a body held in a Station's cells or
    // on a transfer van (the 04:00 transfer); a stand in the cells is no
    // parked ride.
    for &p in world.sentenced() {
        if seen.iter().any(|s| s.0 == p) {
            continue;
        }
        let Some(b) = world.comp::<crate::components::Brain>(p) else { continue };
        if b.lod == Lod::Statistical {
            continue;
        }
        let aboard = world.aboard.get(&p).is_some_and(|v| world.van_runs.contains_key(v));
        let in_cell = world
            .comp::<crate::components::Position>(p)
            .and_then(|pos| pos.building)
            .is_some_and(|bd| crate::systems::stations::is_station(world, bd));
        if !aboard && !in_cell {
            continue;
        }
        let Some(at) = landed_at(world, p) else { continue };
        seen.push((p, "transfer", b.lod == Lod::Full, at, aboard));
    }
    // The landing: a body watched last tick and off its leg now (a demotion
    // to Statistical snaps it off screen: no ride's jump).
    let gone: Vec<(EntityId, RideSeen, Option<TilePos>)> = log
        .ride_track
        .iter()
        .filter(|(id, r)| r.last + 1 == tick && !seen.iter().any(|s| s.0 == **id))
        .filter(|(id, _)| world.comp::<crate::components::Brain>(**id).is_some_and(|b| b.lod != Lod::Statistical))
        .map(|(&id, &r)| (id, r, landed_at(world, id)))
        .collect();
    let Some(log) = world.leg_log.as_mut() else { return };
    for &(id, aboard) in &el_seen {
        log.el_acc.entry(id).or_default()[usize::from(aboard)] += 1;
    }
    let done: Vec<EntityId> = log.el_acc.keys().filter(|id| !el_seen.iter().any(|s| s.0 == **id)).copied().collect();
    for id in done {
        if let Some(t) = log.el_acc.remove(&id) {
            log.el_trips.push(t);
        }
    }
    for (id, r, now) in gone {
        if now.is_some_and(|t| t.manhattan(r.tile) > RIDE_JUMP_TILES) {
            log.ride_days.entry((day, id, r.mode, r.full)).or_default()[1] = true;
        }
    }
    log.ride_track.retain(|_, r| r.last + 1 >= tick);
    for (id, mode, full, tile, park) in seen {
        let next = match log.ride_track.get(&id).copied() {
            Some(r) if r.last + 1 == tick && r.tile == tile && !park => {
                RideSeen { since: tick, last: tick, mode, full, ..r }
            }
            Some(r) if r.last + 1 == tick && r.tile == tile => {
                if tick - r.since >= RIDE_PARKED_TICKS {
                    log.ride_days.entry((day, id, mode, full)).or_default()[0] = true;
                }
                RideSeen { last: tick, mode, full, ..r }
            }
            Some(r) if r.last + 1 == tick => {
                // A jump is the mode it left (a landing: the El's when it
                // set him down on its stop; phase 6: a timed walk's onto the
                // platform is the walk's).
                if r.tile.manhattan(tile) > RIDE_JUMP_TILES {
                    log.ride_days.entry((day, id, r.mode, r.full)).or_default()[1] = true;
                }
                RideSeen { tile, since: tick, last: tick, mode, full }
            }
            _ => RideSeen { tile, since: tick, last: tick, mode, full },
        };
        log.ride_days.entry((day, id, mode, full)).or_default();
        log.ride_track.insert(id, next);
    }
    log.ride_track.retain(|_, r| r.last == tick);
}

/// Every tick (`run --legs`, the island pass phase 9's device, OPEN-181):
/// each Coarse body on a timed walk on foot (`GotoTimed`, aboard nothing)
/// is drawn where the view draws it (`exec::walk_place`; the base: its
/// `Position`, indoors not drawn). Sums: walker-ticks sharing a street tile
/// with a walker bound elsewhere and the largest such stack; walks parked
/// `RIDE_PARKED_TICKS` on one tile; promotions to Full mid-walk that jump
/// or restart at the walk's start. `#walk` rows. Measurement only.
pub fn walk_tick(world: &mut World) {
    use crate::exec::ExecState;
    let Some(log) = world.leg_log.as_ref() else { return };
    let tick = world.tick;
    // Promotions: a walker last tick, Full now.
    let mut promos: Vec<(WalkSeen, Option<TilePos>)> = Vec::new();
    for (&id, r) in &log.walk_track {
        if world.comp::<crate::components::Brain>(id).is_some_and(|b| b.lod == Lod::Full) {
            promos.push((*r, landed_at(world, id)));
        }
    }
    // The walkers now: (id, to, arrive, Position, the way to read when on a
    // walk with a start).
    let mut now: Vec<WalkerNow> = Vec::new();
    for &id in world.tier(Lod::Coarse) {
        let Some(b) = world.comp::<crate::components::Brain>(id) else { continue };
        let ExecState::GotoTimed { target, arrive_tick, walk, .. } = &b.exec else { continue };
        if world.aboard.contains_key(&id) || *arrive_tick == Tick::MAX {
            continue;
        }
        let Some(p) = world.comp::<crate::components::Position>(id) else { continue };
        now.push((id, target.tile, *arrive_tick, p.clone(), walk.map(|w| (w.depart, *arrive_tick))));
    }
    // The ways not yet kept (one road search a walk).
    let need: Vec<EntityId> = now
        .iter()
        .filter(|w| w.4.is_some_and(|(d, _)| log.walk_ways.get(&w.0).is_none_or(|k| k.0 != d)))
        .map(|w| w.0)
        .collect();
    let fresh: Vec<(EntityId, KeptWay)> = need
        .into_iter()
        .filter_map(|id| crate::exec::walk_route(world, id).map(|(way, d, a)| (id, (d, way, a))))
        .collect();
    let Some(log) = world.leg_log.as_mut() else { return };
    for (id, k) in fresh {
        log.walk_ways.insert(id, k);
    }
    let s = &mut log.walks;
    for (r, at) in promos {
        s.promoted += 1;
        if at.is_some_and(|t| t.manhattan(r.tile) > RIDE_JUMP_TILES) {
            s.promo_jumps += 1;
        }
        let far = r.start.manhattan(r.to) > 8;
        let late = (tick - r.first) * 4 >= r.arrive.saturating_sub(r.first);
        if far && late && at.is_some_and(|t| t.manhattan(r.start) <= 2) {
            s.promo_restart += 1;
        }
    }
    // Drawn: on the way, else the street tile (indoors is not drawn).
    let mut drawn: Vec<(EntityId, TilePos, TilePos, Tick, TilePos)> = Vec::new();
    for (id, to, arrive, p, walk) in now {
        let tile = match walk.and_then(|_| log.walk_ways.get(&id)) {
            Some((d, way, a)) => crate::systems::motion::timed_pos(way, *d, *a, tick),
            None if p.building.is_none() => p.tile,
            None => {
                // Indoors: still a walk (parked), not on the street.
                drawn.push((id, TilePos::new(u16::MAX, u16::MAX), to, arrive, p.tile));
                continue;
            }
        };
        drawn.push((id, tile, to, arrive, p.tile));
    }
    // Stacks: street tiles holding walkers bound for two places or more.
    // A still stack: its walkers have stood there `WALK_STILL_TICKS` (a
    // walk at 2 ticks a tile passes a tile in 1-2; a crowd off one door
    // walks one road together a while).
    let mut by_tile: BTreeMap<(bool, TilePos), (u32, TilePos, bool)> = BTreeMap::new();
    for &(id, tile, to, arrive, _) in &drawn {
        if tile.x == u16::MAX {
            continue;
        }
        let still = log
            .walk_track
            .get(&id)
            .is_some_and(|r| r.to == to && r.arrive == arrive && r.tile == tile && tick - r.since >= WALK_STILL_TICKS);
        let keys: &[bool] = if still { &[false, true] } else { &[false] };
        for &k in keys {
            let e = by_tile.entry((k, tile)).or_insert((0, to, false));
            e.0 += 1;
            e.2 |= e.1 != to;
        }
    }
    for (&(still, tile), &(n, _, mixed)) in &by_tile {
        if !mixed {
            continue;
        }
        if still {
            s.still += u64::from(n);
            if n > s.still_max.0 {
                s.still_max = (n, tick, tile);
            }
            continue;
        }
        s.stacked += u64::from(n);
        if n > s.stack_max.0 {
            s.stack_max = (n, tick, tile);
        }
    }
    s.ticks += drawn.iter().filter(|d| d.1.x != u16::MAX).count() as u64;
    let mut track: BTreeMap<EntityId, WalkSeen> = BTreeMap::new();
    for (id, tile, to, arrive, pos) in drawn {
        let next = match log.walk_track.get(&id) {
            Some(r) if r.to == to && r.arrive == arrive => {
                let mut r = *r;
                if r.tile != tile {
                    r.tile = tile;
                    r.since = tick;
                } else if !r.parked && tick - r.since >= RIDE_PARKED_TICKS {
                    r.parked = true;
                    s.parked += 1;
                }
                r
            }
            _ => {
                s.walks += 1;
                WalkSeen { to, arrive, start: pos, first: tick, tile, since: tick, parked: false }
            }
        };
        track.insert(id, next);
    }
    log.walk_track = track;
    let live = &log.walk_track;
    log.walk_ways.retain(|id, _| live.contains_key(id));
}

/// Who `id` is, for the table: the trade's key, the built-in role, a corp
/// exec, or none.
fn who(world: &World, id: EntityId) -> String {
    if let Some(job) = world.comp::<crate::components::Job>(id) {
        if let Some(r) = crate::systems::trades::row_of(world, job.role) {
            return r.id.clone();
        }
        return format!("{:?}", job.role).to_lowercase();
    }
    if crate::systems::life::exec_corp(world, id).is_some() {
        return "exec".into();
    }
    "none".into()
}

/// Why a walking leg did not ride the El: the refusal, or "stat"/"off".
fn why_walk(world: &World, id: EntityId, from: TilePos, to: TilePos, lod: Lod) -> String {
    if !crate::systems::transit::enabled(world) || world.transit.net.stops.is_empty() {
        return "el_off".into();
    }
    if lod == Lod::Statistical {
        return "stat".into();
    }
    let net = &world.transit.net;
    let near = |t: TilePos| net.near(t).iter().map(|&s| net.stops[usize::from(s)].manhattan(t)).min().unwrap_or(0);
    let (a, b) = (near(from), near(to));
    let r = match crate::systems::transit::choose(world, id, from, to, world.tick) {
        Ok(_) => "would_ride".to_string(),
        Err(e) => format!("{e:?}").to_lowercase(),
    };
    format!("{r}:{a}:{b}")
}

/// A leg starts (`exec::start_step`'s GoTo branch, after the mode is known).
pub fn start(world: &mut World, id: EntityId, target: &GotoTarget, mode: &'static str) {
    if world.leg_log.is_none() {
        return;
    }
    let key = crate::exec::legs::plan_key(world, id);
    if let Some(open) = world.leg_log.as_mut().and_then(|l| l.open.get_mut(&id)) {
        if open.key_plan == key {
            open.hops += 1;
            if mode != "walk" {
                open.mode = mode;
            }
            return;
        }
    }
    let Some(from) = crate::exec::walk_origin(world, id) else { return };
    let lod = world.comp::<crate::components::Brain>(id).map_or(Lod::Full, |b| b.lod);
    let why = if mode == "walk" { why_walk(world, id, from, target.tile, lod) } else { String::new() };
    let est = if mode == "el" { crate::systems::transit::commute_ticks(world, from, target.tile) } else { 0 };
    let s = LegStart {
        tick: world.tick,
        from,
        to: target.tile,
        key: target.dest,
        lod,
        mode,
        why,
        key_plan: key,
        hops: 0,
        est,
    };
    if let Some(log) = world.leg_log.as_mut() {
        log.open.insert(id, s);
    }
}

/// A leg ended (done, failed or the plan dropped).
pub fn end(world: &mut World, id: EntityId, outcome: &'static str) {
    let Some(log) = world.leg_log.as_ref() else { return };
    if !log.open.contains_key(&id) {
        return;
    }
    let min = log.min_ticks;
    let who = who(world, id);
    let goal = world
        .comp::<crate::components::Brain>(id)
        .and_then(|b| b.plan_goal())
        .map_or_else(|| "-".to_string(), |g| format!("{g:?}"));
    let tick = world.tick;
    let Some(log) = world.leg_log.as_mut() else { return };
    let Some(s) = log.open.remove(&id) else { return };
    let ticks = tick.saturating_sub(s.tick);
    if s.mode == "el" && outcome == "done" {
        log.el_fit.push((s.est, ticks));
    }
    let e = log.by_mode.entry(s.mode).or_default();
    e.0 += 1;
    e.1 += ticks;
    if ticks >= min {
        log.rows.push(LegRow { id, who, goal, start: s, ticks, outcome });
    }
}

/// The TSV header and rows (`run --legs`).
pub fn tsv(world: &World) -> String {
    let mut out = String::from(
        "start\tid\twho\tgoal\tkey\tlod\tmode\twhy\tfrom\tto\ttiles\tfrom_d\tto_d\tminutes\toutcome\thops\n",
    );
    let Some(log) = world.leg_log.as_ref() else { return out };
    for r in &log.rows {
        let s = &r.start;
        let d = |t: TilePos| world.district_name(world.district_of(t)).to_string();
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\t{},{}\t{},{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            s.tick,
            r.id.index,
            r.who,
            r.goal,
            s.key,
            s.lod,
            s.mode,
            s.why,
            s.from.x,
            s.from.y,
            s.to.x,
            s.to.y,
            s.from.manhattan(s.to),
            d(s.from),
            d(s.to),
            r.ticks,
            r.outcome,
            s.hops
        ));
    }
    for (d, r) in &log.nights {
        out.push_str(&format!("#night\t{d}\t{}\t{}\t{}\t{}\t{}\t{}\n", r[0], r[1], r[2], r[3], r[4], r[5]));
    }
    for (&(n, id), r) in &log.leaders {
        out.push_str(&format!(
            "#leader	{n}	{}	{}	{}	{}	{}
",
            id.index, r[0], r[1], r[2], r[3]
        ));
    }
    for (m, (n, t)) in &log.by_mode {
        out.push_str(&format!("#mode\t{m}\t{n}\t{t}\n"));
    }
    // The island pass phase 5 (OPEN-165's device): by mode and tier, the
    // body-days on a vehicle leg, parked, jumped, either.
    let mut rides: BTreeMap<(&'static str, bool), [u32; 4]> = BTreeMap::new();
    for (&(_, _, mode, full), f) in &log.ride_days {
        let r = rides.entry((mode, full)).or_default();
        r[0] += 1;
        r[1] += u32::from(f[0]);
        r[2] += u32::from(f[1]);
        r[3] += u32::from(f[0] || f[1]);
    }
    for ((m, full), r) in &rides {
        let tier = if *full { "full" } else { "coarse" };
        out.push_str(&format!("#ride\t{m}\t{tier}\t{}\t{}\t{}\t{}\n", r[0], r[1], r[2], r[3]));
    }
    // The island pass phase 9 (OPEN-181's device): the Coarse walks.
    let w = &log.walks;
    out.push_str(&format!(
        "#walk\tticks\t{}\tstacked\t{}\tstack_max\t{}\tat\t{}\t{},{}\tstill\t{}\tstill_max\t{}\tat\t{}\t{},{}\twalks\t{}\tparked\t{}\tpromoted\t{}\tjumps\t{}\trestarts\t{}\n",
        w.ticks,
        w.stacked,
        w.stack_max.0,
        w.stack_max.1,
        w.stack_max.2.x,
        w.stack_max.2.y,
        w.still,
        w.still_max.0,
        w.still_max.1,
        w.still_max.2.x,
        w.still_max.2.y,
        w.walks,
        w.parked,
        w.promoted,
        w.promo_jumps,
        w.promo_restart
    ));
    // The island pass phase 6's device: the El rides' platform and aboard
    // ticks (median, p90), the El legs' estimate against their ticks, the
    // trains' events, the structural commutes at 240 min or more.
    let q = |v: &mut Vec<u64>, f: f64| -> u64 {
        v.sort_unstable();
        v.get(((v.len() as f64 * f) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(0)
    };
    let mut wait: Vec<u64> = log.el_trips.iter().map(|t| u64::from(t[0])).collect();
    let mut ride: Vec<u64> = log.el_trips.iter().map(|t| u64::from(t[1])).collect();
    out.push_str(&format!(
        "#el\ttrips\t{}\twait_p50\t{}\twait_p90\t{}\tride_p50\t{}\tride_p90\t{}\n",
        log.el_trips.len(),
        q(&mut wait, 0.5),
        q(&mut wait, 0.9),
        q(&mut ride, 0.5),
        q(&mut ride, 0.9)
    ));
    let n = log.el_fit.len().max(1) as f64;
    let (est, act): (u64, u64) = log.el_fit.iter().fold((0, 0), |(a, b), &(e, t)| (a + e, b + t));
    let mut ratio: Vec<u64> = log.el_fit.iter().map(|&(e, t)| t * 100 / e.max(1)).collect();
    out.push_str(&format!(
        "#elfit\tlegs\t{}\test_mean\t{:.1}\tticks_mean\t{:.1}\tratio_pct_p50\t{}\tratio_pct_p90\t{}\n",
        log.el_fit.len(),
        est as f64 / n,
        act as f64 / n,
        q(&mut ratio, 0.5),
        q(&mut ratio, 0.9)
    ));
    out.push_str(&format!(
        "#trains\tfull_waits\t{}\theld_to_stop\t{}\tcut_set_down\t{}\n",
        log.trains[0], log.trains[1], log.trains[2]
    ));
    let mut far = (0u32, 0u32);
    // The pressure pass F1: the same by role (n, at 120+, at 240+).
    let mut by_role: BTreeMap<String, [u32; 3]> = BTreeMap::new();
    for a in world.citizens() {
        let Some(job) = world.comp::<crate::components::Job>(a) else { continue };
        let Some(w) = job.employer else { continue };
        let Some(wd) = world.comp::<crate::components::Building>(w).map(|b| b.door) else { continue };
        let Some(hd) = crate::systems::contracts::home_door(world, a) else { continue };
        let t = crate::systems::distance::travel(world, hd, wd);
        far.0 += 1;
        far.1 += u32::from(t >= 240);
        let r = by_role.entry(format!("{:?}", job.role)).or_default();
        r[0] += 1;
        r[1] += u32::from(t >= 120);
        r[2] += u32::from(t >= 240);
    }
    out.push_str(&format!("#commute\temployed_housed\t{}\tat_240_plus\t{}\n", far.0, far.1));
    for (role, r) in &by_role {
        out.push_str(&format!("#commute_role\t{role}\t{}\t{}\t{}\n", r[0], r[1], r[2]));
    }
    out
}

/// The 03:00 census (`run --legs`): of the corp execs and the guards with a
/// Home, how many sleep in it, how many sleep elsewhere (a Hotel, a cot, the
/// street), and how many are awake: `#night` rows of the TSV.
pub fn night_census(world: &mut World) {
    if world.leg_log.is_none() {
        return;
    }
    let mut row = [0u32; 6];
    for id in world.citizens() {
        let guard = world.comp::<crate::components::Job>(id).is_some_and(|j| j.role == crate::components::Role::Guard);
        let exec = !guard && crate::systems::life::exec_corp(world, id).is_some();
        if !guard && !exec {
            continue;
        }
        let Some(home) = crate::systems::missions::home_of(world, id) else { continue };
        let asleep = world.comp::<crate::components::Brain>(id).is_some_and(|b| {
            matches!(b.exec, crate::exec::ExecState::Use { kind: crate::goap::ActionKind::Sleep, .. })
        });
        let at_home = world.comp::<crate::components::Position>(id).is_some_and(|p| p.building == Some(home));
        let k = if exec { 0 } else { 3 }
            + if !asleep {
                2
            } else if at_home {
                0
            } else {
                1
            };
        row[k] += 1;
    }
    let day = world.tick / crate::time::TICKS_PER_DAY;
    if let Some(log) = world.leg_log.as_mut() {
        log.nights.push((day, row));
    }
}

/// Every tick (`run --legs`): each gang leader with a body, asleep or not,
/// and where; `#leader` rows of the TSV (night, id, minutes asleep, sleeps
/// begun, bed switches, minutes a leader with a body). Measurement only.
pub fn leader_tick(world: &mut World) {
    if world.leg_log.is_none() {
        return;
    }
    let night = (world.tick + 6 * crate::time::TICKS_PER_HOUR) / crate::time::TICKS_PER_DAY;
    let leaders: Vec<EntityId> = world
        .gang_list()
        .iter()
        .filter_map(|&g| world.comp::<crate::components::Gang>(g).and_then(|x| x.leader))
        .filter(|&l| crate::systems::presence::is_body(world, l))
        .collect();
    let mut seen = Vec::with_capacity(leaders.len());
    for l in leaders {
        let asleep = world.comp::<crate::components::Brain>(l).is_some_and(|b| {
            matches!(b.exec, crate::exec::ExecState::Use { kind: crate::goap::ActionKind::Sleep, .. })
        });
        let at = world.comp::<crate::components::Position>(l).and_then(|p| p.building);
        seen.push((l, asleep, at));
    }
    let Some(log) = world.leg_log.as_mut() else { return };
    for (l, asleep, at) in seen {
        let row = log.leaders.entry((night, l)).or_default();
        row[3] += 1;
        let last = log.leader_last.get(&l).copied();
        if asleep {
            row[0] += 1;
            let was_asleep = last.is_some_and(|(_, _, a)| a);
            if !was_asleep {
                row[1] += 1;
                if last.is_some_and(|(n, b, _)| n == night && b.is_some() && b != at) {
                    row[2] += 1;
                }
            }
            log.leader_last.insert(l, (night, at, true));
        } else if let Some((n, b, _)) = last {
            log.leader_last.insert(l, (n, b, false));
        }
    }
}
