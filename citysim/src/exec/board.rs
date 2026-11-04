use super::*;

/// The pre-view pass phase 5 (P12): an escort to the cells walks, at
/// walking speed (a Full guard on foot with the suspect in tow, a Coarse
/// one timed at the walk), up to `[legs] escort_walk_tiles`; past it the
/// guard calls the van: `van_call_ticks` for it to come, then the drive
/// with both aboard at the Car's speed (`[vehicles] mult`). No escort
/// covers ground faster than its mode. `None` with `[legs]` off (the old
/// timed leg capped at `escort_van_ticks`). Statistical mirror: none (an
/// arrest is made by a body; a Statistical guard's arrests are the law's
/// off-screen roll, with no leg).
pub(super) fn escort_leg(world: &mut World, id: EntityId, target: &GotoTarget, lod: Lod) -> Option<ExecState> {
    let cfg = world.config.legs.clone()?;
    let from = walk_origin(world, id)?;
    let tiles = from.manhattan(target.tile);
    let (state, ticks) = if tiles <= cfg.escort_walk_tiles {
        let state = match lod {
            Lod::Full => {
                walking_goto(world, id, target.clone()).unwrap_or_else(|| timed_goto(world, id, target.clone()))
            }
            _ => timed_goto(world, id, target.clone()),
        };
        crate::systems::ownership::jobs2_row(world).escort_walks += 1;
        // The island pass phase 7 (I3): a timed walk leads the suspect
        // along the road (no statue, no landing).
        if let ExecState::GotoTimed { arrive_tick, .. } = state {
            escort_aboard(world, id, crate::systems::motion::RideKind::Foot, target.tile, world.tick, arrive_tick);
        }
        (state, timed_walk(world, id, from, target.tile))
    } else {
        // The island pass phase 7: the van drives the road (the island's
        // bays make it far longer than the Manhattan line), so the drive is
        // timed over the road's length.
        // The pressure pass R2: a road-measured way at the road's speed.
        let drive = if crate::systems::motion::vans_on(world) {
            let road = crate::systems::motion::road_len(world, from, target.tile);
            crate::systems::vehicles::road_ticks(world, road.max(tiles))
        } else {
            crate::systems::vehicles::car_ticks(world, tiles)
        };
        let call = Tick::from(cfg.van_call_ticks);
        let ticks = call + drive;
        crate::systems::ownership::jobs2_row(world).escort_vans += 1;
        // The island pass phase 7 (I3): the van pulls up at the kerb, waits
        // the call, and the guard drives it, his suspect aboard.
        let now = world.tick;
        escort_aboard(world, id, crate::systems::motion::RideKind::Van, target.tile, now + call, now + ticks);
        (
            ExecState::GotoTimed {
                target: target.clone(),
                arrive_tick: world.tick + ticks,
                blocked_since: None,
                walk: None,
                climb: None,
            },
            ticks,
        )
    };
    let tpm = u32::try_from(u64::from(tiles) * 100 / ticks.max(1)).unwrap_or(u32::MAX);
    let row = crate::systems::ownership::jobs2_row(world);
    row.escort_tpm_max = row.escort_tpm_max.max(tpm);
    Some(state)
}

/// The island pass phase 7 (I3, OPEN-166): the escort's van or timed walk
/// in motion, the guard leading, his cuffed suspect aboard. Nothing with
/// `[motion.vans]` off.
fn escort_aboard(
    world: &mut World,
    guard: EntityId,
    ride: crate::systems::motion::RideKind,
    to: TilePos,
    depart: Tick,
    arrive: Tick,
) {
    let Some(suspect) = world.comp::<Brain>(guard).and_then(|b| b.escorting) else { return };
    crate::systems::motion::board_escort(world, guard, suspect, ride, to, depart, arrive);
}

/// `lod::set_lod`: a Full guard walking an escort demoted to Coarse (the
/// walk now timed): he leads the suspect along the road on the timed leg.
pub fn escort_to_timed(world: &mut World, guard: EntityId) {
    if !crate::systems::motion::vans_on(world) || crate::systems::motion::vehicle_of(world, guard).is_some() {
        return;
    }
    let Some(b) = world.comp::<Brain>(guard) else { return };
    if b.escorting.is_none() || !b.current_step().is_some_and(|s| s.action == ActionKind::Escort) {
        return;
    }
    let ExecState::GotoTimed { target, arrive_tick, .. } = &b.exec else { return };
    let (to, arrive) = (target.tile, *arrive_tick);
    let now = world.tick;
    escort_aboard(world, guard, crate::systems::motion::RideKind::Foot, to, now, arrive);
}

/// The island pass phase 5 (I3): a street leg (`start_step`) is a vehicle
/// in motion: the cab (`full_cab`, the Garage's off-screen cab, the rider
/// aboard), the exec's car (`exec_ride`), a body driver's fare leg (he at
/// the wheel of the cab his fare is aboard, else a fresh one). A body
/// fare's rider (`arrive_tick` never: he waits at his door) climbs in at
/// the pick-up (`streetwork::carry`). Nothing with `[motion]` off.
pub(super) fn board_street(
    world: &mut World,
    id: EntityId,
    target: &GotoTarget,
    state: &ExecState,
    mode: &'static str,
) {
    use crate::systems::motion::{self, RideKind};
    if !motion::on(world) {
        return;
    }
    let ExecState::GotoTimed { arrive_tick, .. } = *state else { return };
    if arrive_tick == Tick::MAX {
        return;
    }
    let now = world.tick;
    let from = walk_origin(world, id).unwrap_or(target.tile);
    let ride_ticks = crate::systems::streetwork::cab_ticks(world, from, target.tile);
    let depart = arrive_tick.saturating_sub(ride_ticks).max(now);
    let car = crate::components::AssetKind::Car;
    let to = target.tile;
    match mode {
        "fare_leg" => {
            let reuse = crate::systems::streetwork::fare_vehicle(world, id);
            motion::board_timed(world, id, RideKind::Cab, car, None, reuse, to, now, arrive_tick, true);
        }
        "cab" => {
            motion::board_timed(world, id, RideKind::Cab, car, None, None, to, depart, arrive_tick, false);
        }
        _ => {
            motion::board_timed(world, id, RideKind::Exec, car, None, None, to, depart, arrive_tick, false);
        }
    }
}

/// The island pass phase 5 (I3): a trip's vehicle is in motion: a flyer
/// flies its straight way over the flight's ticks, a Coarse driver's road
/// vehicle runs his timed leg along the road, a Full driver steers his.
/// Nothing with `[motion]` off.
pub(super) fn board_trip(world: &mut World, id: EntityId, kind: crate::components::AssetKind, state: &ExecState) {
    use crate::systems::motion::{self, RideKind};
    if !motion::on(world) {
        return;
    }
    let Some(v) = world.trips.get(&id).map(|t| t.vehicle) else { return };
    match state {
        ExecState::Fly { target, depart, arrive_tick, .. } => {
            let (to, d, a) = (target.tile, *depart, *arrive_tick);
            motion::board_timed(world, id, RideKind::Flyer, kind, Some(v), None, to, d, a, true);
        }
        ExecState::GotoTimed { target, arrive_tick, .. } => {
            let (to, d, a) = (target.tile, world.tick, *arrive_tick);
            motion::board_timed(world, id, RideKind::Car, kind, Some(v), None, to, d, a, true);
        }
        ExecState::Goto { .. } => {
            motion::board_driven(world, id, v, kind);
        }
        _ => {}
    }
}

/// The pre-view pass phase 5 (the `fares` shape): a cab driver's leg past
/// `[legs] leg_replan_tiles` drives in segments too, each timed at the
/// cab's speed (the car seen en route, the rider aboard with it).
pub(super) fn fare_segment(world: &mut World, id: EntityId, target: &GotoTarget) -> Option<ExecState> {
    // The island pass phase 5 (I3): with the vehicle in motion the cab is
    // seen along its whole way; one leg.
    if !legs::on(world) || crate::systems::motion::on(world) || !crate::systems::streetwork::on_fare(world, id) {
        return None;
    }
    let from = walk_origin(world, id)?;
    let end = legs::segment_end(world, from, target.tile)?;
    let key = legs::plan_key(world, id)?;
    let arrive_tick = world.tick + crate::systems::streetwork::cab_ticks(world, from, end);
    world.segments.insert(id, key);
    Some(ExecState::GotoTimed {
        target: GotoTarget { dest: LocationKey::Street, tile: end, building: None },
        arrive_tick,
        blocked_since: None,
        walk: None,
        climb: None,
    })
}

/// The pre-view pass phase 5 (OPEN-139): a walk past `[legs]
/// leg_replan_tiles` toward `target` walks to the next segment's end first
/// (`legs::segment_end`; a Full body on foot, a Coarse one timed), filed in
/// `World::segments` against the plan step. `None`: one leg as before.
pub(super) fn segment(world: &mut World, id: EntityId, target: &GotoTarget, lod: Lod) -> Option<ExecState> {
    let from = walk_origin(world, id)?;
    let key = legs::plan_key(world, id)?;
    let state = if lod == Lod::Full && !world.config.exec.straight_line_paths {
        let (lvl, foot) = (street_level(world, id), on_foot(world, id));
        full_segment(world, from, lvl, foot, target)?
    } else {
        let end = legs::segment_end(world, from, target.tile)?;
        timed_goto(world, id, GotoTarget { dest: LocationKey::Street, tile: end, building: None })
    };
    world.segments.insert(id, key);
    Some(state)
}

/// A Full walk's segment follows the walk's own route: the tile `[legs]
/// leg_replan_tiles` steps along the path the one leg would take (the
/// door's flow field, the big map's A*, a street target's A*), so the body
/// walks the streets it walked before, re-checking its ride at each end.
/// The straight line's Road snap took the walker off its route: a guard's
/// patrol leg left the street its beat passed down, and the thief that
/// beat sighted was never seen again (`law::test_theft_witnessed…`).
/// `None`: the route is no longer than a segment, or there is none.
pub(super) fn full_segment(
    world: &mut World,
    from: TilePos,
    lvl: u8,
    foot: bool,
    target: &GotoTarget,
) -> Option<ExecState> {
    let len = world.config.legs.as_ref()?.leg_replan_tiles.max(1) as usize;
    let mut route_levels: Vec<u8> = Vec::new();
    let route: Vec<TilePos> = match target.building {
        Some(b) if !world.big_paths() => {
            let door = world.comp::<Building>(b)?.door;
            let mut route = Vec::with_capacity(len + 1);
            let mut here = from;
            while route.len() <= len && here != door {
                here = world.flow_step(b, here)?;
                route.push(here);
            }
            route
        }
        Some(b) => {
            let door = world.comp::<Building>(b)?.door;
            let (r, l) = world.big_route(from, lvl, door, foot)?;
            route_levels = l;
            r
        }
        None if world.big_paths() => {
            let (r, l) = world.big_route(from, lvl, target.tile, foot)?;
            route_levels = l;
            r
        }
        None => pathfind::astar(&world.map, from, target.tile, world.config.exec.astar_max_expansions)?,
    };
    // C7: a segment ends on the ground (never on a deck).
    let mut len = len;
    while route_levels.get(len.saturating_sub(1)).is_some_and(|&l| l > 0) {
        len += 1;
    }
    if route.len() <= len {
        return None;
    }
    let end = route[len - 1];
    let mut path = route[..len].to_vec();
    path.reverse();
    let mut levels: Vec<u8> = route_levels.get(..len).map(<[u8]>::to_vec).unwrap_or_default();
    if levels.iter().all(|&l| l == 0) {
        levels.clear();
    }
    levels.reverse();
    let leg = GotoTarget { dest: LocationKey::Street, tile: end, building: None };
    Some(ExecState::Goto {
        target: leg,
        path,
        next_move_tick: world.tick,
        blocked_since: None,
        carry_q: 0,
        levels,
        hop: None,
    })
}
