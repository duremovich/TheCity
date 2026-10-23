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

/// The vehicle legs' modes (`start`'s): the cab, a body driver's fare leg,
/// the exec's car, the El, a flyer, a drive.
fn vehicle_mode(mode: &str) -> bool {
    matches!(mode, "cab" | "fare_leg" | "exec_ride" | "el" | "fly" | "drive")
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
    let mut seen: Vec<(EntityId, &'static str, bool, TilePos)> = Vec::new();
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
                    },
                    None => s.mode,
                }
            }
            _ => continue,
        };
        let Some(p) = world.comp::<crate::components::Position>(id) else { continue };
        // A body set down on an El stop this tick (`transit::run` alights
        // after the step) is the El's: its jump is the El's (phase 6).
        let mode =
            if world.transit.net.stops.contains(&p.tile) && log.ride_track.get(&id).is_some_and(|r| r.tile != p.tile) {
                "el"
            } else {
                mode
            };
        seen.push((id, mode, b.lod == Lod::Full, p.tile));
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
    for (id, r, now) in gone {
        if now.is_some_and(|t| t.manhattan(r.tile) > RIDE_JUMP_TILES) {
            log.ride_days.entry((day, id, r.mode, r.full)).or_default()[1] = true;
        }
    }
    log.ride_track.retain(|_, r| r.last + 1 >= tick);
    for (id, mode, full, tile) in seen {
        let next = match log.ride_track.get(&id).copied() {
            Some(r) if r.last + 1 == tick && r.tile == tile => {
                if tick - r.since >= RIDE_PARKED_TICKS {
                    log.ride_days.entry((day, id, mode, full)).or_default()[0] = true;
                }
                RideSeen { last: tick, mode, full, ..r }
            }
            Some(r) if r.last + 1 == tick => {
                // A jump is the mode it left (a landing), or the El's when
                // it set him down on its stop.
                if r.tile.manhattan(tile) > RIDE_JUMP_TILES {
                    let m = if mode == "el" { mode } else { r.mode };
                    log.ride_days.entry((day, id, m, r.full)).or_default()[1] = true;
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
    let s =
        LegStart { tick: world.tick, from, to: target.tile, key: target.dest, lod, mode, why, key_plan: key, hops: 0 };
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
