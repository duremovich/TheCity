use super::*;

/// One tick of Full-LOD movement. M13 D19, round 9 Y4: steps are counted
/// in step units, `next_q = next_move_tick × units_per_tick + carry_q`
/// (never earlier than now; `[exec] units_per_tick` is `step_res ×
/// walk_tiles_per_tick`): the call keeps stepping while `next_q` is inside
/// this tick, a walker's step `step_units` (`step_res × move_ticks_full`),
/// so a walker crosses `walk_tiles_per_tick` tiles every `move_ticks_full`
/// ticks, a driver up to `[vehicles] max_steps_per_tick` steps. Every
/// sub-tick step is taken tile by tile: the litter on the tile stepped
/// onto, a driver's step count and crash tile (`vehicles::note_step`), the
/// path's re-find on a big map (`big_step`) all read the tile they happen
/// on. The burst goes in at the door (S1, `OPEN-317`: it stopped before it,
/// the door and its queue the next call's, and the walker stood a tick at
/// every door) and stops at the path's end (the arrival is the next call's;
/// a segment's end is its arrival now, `OPEN-311`). Stepping out of a
/// building walks the floor to the door, a tile a step, then two steps
/// through it, and the walk goes on in the same tick (`OPEN-307`).
#[allow(clippy::too_many_arguments)]
pub(super) fn advance_goto(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    mut path: Vec<TilePos>,
    mut levels: Vec<u8>,
    next_move_tick: Tick,
    blocked_since: Option<Tick>,
    carry_q: u32,
) -> StepResult {
    let tick = world.tick;
    if tick < next_move_tick {
        return StepResult::Running;
    }
    let upt = world.config.exec.units_per_tick();
    let Some(pos) = world.comp::<Position>(id).cloned() else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let mut next_q = (next_move_tick * upt + u64::from(carry_q)).max(tick * upt);
    let end_q = (tick + 1) * upt;

    // Inside some other building: step out onto the street first (one step).
    if let Some(here) = pos.building {
        if Some(here) == target.building {
            return StepResult::Done;
        }
        // The city milestone C7: down to the door first, or straight to the
        // target's storey inside the same structure.
        if let Some(r) = climb_out(world, id, &target, here, pos.floor, &path, &levels) {
            return r;
        }
        // S1 (`OPEN-307`): across the floor to the door at the walk, a tile
        // a step (a station spot behind the counter is a walk from the
        // door), then through it onto the street: two steps, the door's
        // tile and the street's. The leave put the body outside the door
        // from anywhere on the plate in one step.
        let walk = if on_foot(world, id) {
            u64::from(crate::systems::vehicles::step_q(world, id, pos.tile))
        } else {
            world.config.exec.step_units()
        };
        let mut tile = pos.tile;
        let mut out_of_time = false;
        if let Some(bd) = world.comp::<Building>(here).filter(|bd| bd.rect.contains(pos.tile)) {
            while let Some(n) = bd.step_to_door(tile) {
                tile = n;
                next_q += walk;
                if next_q >= end_q {
                    out_of_time = true;
                    break;
                }
            }
        }
        if tile != pos.tile {
            move_to(world, id, tile);
        }
        if out_of_time {
            return set_goto(world, id, target, path, levels, next_q / upt, None, (next_q % upt) as u32);
        }
        world.leave_building(id);
        next_q += 2 * walk;
        if next_q >= end_q {
            return set_goto(world, id, target, path, levels, next_q / upt, None, (next_q % upt) as u32);
        }
    }
    let Some((mut here, mut lvl)) =
        world.comp::<Position>(id).map(|p| (p.tile, if p.building.is_some() { 0 } else { p.floor }))
    else {
        return StepResult::Failed(FailReason::NoSuchPlace);
    };

    let driving = world.comp::<crate::components::Kit>(id).is_some_and(|k| k.driving.is_some());
    let max_steps = if driving { u32::from(world.config.vehicles.max_steps_per_tick.max(1)) } else { u32::MAX };
    // A walker's step is the same on every tile (its `Kit.walk_mult`); a
    // driver's reads the tile's column.
    let walk_q = (!driving).then(|| crate::systems::vehicles::step_q(world, id, here));
    let big_map = world.big_paths();
    let big = target.building.is_some() && big_map;
    let mut steps = 0u32;
    loop {
        let (next, nl) = match target.building {
            Some(b) => {
                let door = world.comp::<Building>(b).map(|bd| bd.door);
                if door == Some(here) && lvl == 0 {
                    return try_enter(world, id, target, path, levels, blocked_since);
                }
                let step = match (big, door) {
                    (false, _) => world.flow_step(b, here).map(|n| (n, 0)),
                    (true, None) => None,
                    (true, Some(d)) => big_step(world, id, &mut path, &mut levels, (here, lvl), d, true),
                };
                match step {
                    Some((n, 0)) if door == Some(n) => {
                        return try_enter(world, id, target, path, levels, blocked_since);
                    }
                    Some(s) => s,
                    // The field never reached this tile: there is no way there.
                    None if steps == 0 => return StepResult::Failed(FailReason::NoSuchPlace),
                    None => break,
                }
            }
            None => {
                // C7: a walk over the walkways re-checks its next hop.
                // S1 (`OPEN-309`): at the way's end on a deck (an El
                // platform's waiting place) the walk is there; the re-find
                // below would take it down to the ground under it.
                let s = if path.is_empty() && here == target.tile {
                    None
                } else if big_map && (!levels.is_empty() || lvl > 0) {
                    big_step(world, id, &mut path, &mut levels, (here, lvl), target.tile, false)
                } else {
                    path.pop().map(|n| (n, 0))
                };
                match s {
                    Some(n) => n,
                    None if steps == 0 && here == target.tile => return StepResult::Done,
                    None if steps == 0 => return StepResult::Failed(FailReason::NoSuchPlace),
                    None => break,
                }
            }
        };
        // C7: a step onto or along a deck, or a jump across one, is a portal's hop.
        let hop =
            if nl != lvl || next.manhattan(here) != 1 { world.portal_edge((here, lvl), (next, nl)) } else { None };
        // The view milestone V7: a walker reaching a marked crossing waits at
        // its kerb for the walk phase (the signal at its sub-tick time); a
        // wait past this tick stands it at the kerb, the step the next call's.
        if !driving && big_map && hop.is_none() && nl == 0 && lvl == 0 {
            if let Some(wait) = crossing_wait_q(world, id, here, next, next_q, upt) {
                next_q += wait;
                if next_q >= end_q {
                    path.push(next);
                    if !levels.is_empty() {
                        levels.push(nl);
                    }
                    break;
                }
            }
        }
        move_to(world, id, next);
        if let Some(p) = world.comp_mut::<Position>(id) {
            p.floor = nl;
        }
        if let Some(e) = &hop {
            note_hop(world, e, lvl, nl);
        }
        here = next;
        lvl = nl;
        steps += 1;
        if driving {
            crate::systems::vehicles::note_step(world, id, next);
        }
        let q = walk_q.unwrap_or_else(|| crate::systems::vehicles::step_q(world, id, next));
        next_q += match &hop {
            Some(e) => u64::from(q) * u64::from(e.steps.max(1)),
            None => u64::from(q) + litter_delay(world, next),
        };
        if next_q >= end_q || steps >= max_steps || (target.building.is_none() && path.is_empty()) {
            break;
        }
    }
    let carry = (next_q % upt) as u32;
    // S1 (`OPEN-311`): a segment's end reached with time left in the tick
    // is its arrival now (the step re-starts at once, `step_agent`, the
    // time left carried into the next segment); the next call's arrival
    // stood the walker two ticks mid-street at every segment end.
    let segment_end = target.building.is_none()
        && path.is_empty()
        && here == target.tile
        && next_q < end_q
        && !world.segments.is_empty()
        && world.segments.contains_key(&id);
    let r = set_goto(world, id, target, path, levels, next_q / upt, None, carry);
    if segment_end {
        return StepResult::Done;
    }
    r
}

/// The view milestone V7: a Full walker stepping from `here` onto `next`
/// at step unit `q` (`upt` a tick): stepping onto a marked crossing from
/// off it, the units it waits at the kerb for its junction's walk phase
/// (`signals::wait_ms`, rounded up so the step lands in the walk phase),
/// counted with the day's crossings; a step onto a carriageway off every
/// crossing from off one counted as a jaywalk. `None`: no wait.
fn crossing_wait_q(world: &mut World, id: EntityId, here: TilePos, next: TilePos, q: u64, upt: u64) -> Option<u64> {
    let cfg = world.config.crossings.clone()?;
    let portals = world.flow_fields.portals();
    let ped = &portals.ped;
    if ped.is_empty() {
        return None;
    }
    let (hi, ni) = (world.map.index(here), world.map.index(next));
    let row = crate::systems::ownership::jobs2_row(world);
    row.walk_steps += 1;
    if ped.jaywalk_at(ni) && !ped.jaywalk_at(hi) && ped.crossing_at(hi).is_none() {
        row.jaywalks += 1;
        let tick = world.tick;
        if let Some(v) = world.crossing_watch.as_mut().and_then(|w| w.get_mut(&id)) {
            v.push((tick, next, next, u32::MAX));
        }
        return None;
    }
    let k = ped.crossing_at(ni)?;
    if ped.crossing_at(hi).is_some() {
        return None;
    }
    let junction = world.map.crossings.get(usize::from(k))?.junction;
    let t_ms = q * crate::signals::MS_PER_TICK / upt.max(1);
    let wait_ms = crate::signals::wait_ms(&cfg, junction, t_ms);
    let wait = (wait_ms * upt).div_ceil(crate::signals::MS_PER_TICK);
    // A walker stood at the kerb past the last tick steps on at the walk
    // phase's first sub-tick unit: counted when its wait began, not again.
    if wait == 0 && crate::signals::phase_ms(&cfg, junction, t_ms) <= crate::signals::MS_PER_TICK / upt.max(1) + 2 {
        return None;
    }
    let row = crate::systems::ownership::jobs2_row(world);
    row.crossings_walked += 1;
    row.crossing_wait_tenths += (wait_ms / 6_000) as u32;
    let tick = world.tick;
    if let Some(v) = world.crossing_watch.as_mut().and_then(|w| w.get_mut(&id)) {
        v.push((tick, next, junction, wait_ms as u32));
    }
    (wait > 0).then_some(wait)
}

/// C7: count a walker's portal hop (the day's crossings).
fn note_hop(world: &mut World, e: &bigpath::Edge, from_level: u8, to_level: u8) {
    use crate::map::PortalKind as K;
    let kind = world.flow_fields.portals().kind(e);
    let row = crate::systems::ownership::jobs2_row(world);
    match kind {
        Some(k) if k.footbridge() => row.hops_footbridge += 1,
        Some(K::Escalator) => row.hops_escalator += 1,
        Some(_) => row.hops_stairs += 1,
        None => {}
    }
    if from_level == 0 && to_level > 0 {
        row.deck_entries += 1;
        if kind.is_some_and(|k| k.footbridge()) {
            row.footbridge_entries += 1;
        }
    }
}

/// Scale S3a: the next step toward `goal` on a big map, off the walk's own
/// path (reversed, the door its first element when `keep_goal`: the door
/// stays on it until `try_enter` lets the walker in). The path is found
/// here when the walk has none, or when its next step is no longer a step
/// away or no longer walkable (a wall went up, rubble fell, the body was
/// moved), or (C7) no longer a portal's hop that runs now (the escalator
/// turned). C7: each step carries its level (`levels`, reversed alike;
/// empty on the ground).
fn big_step(
    world: &World,
    id: EntityId,
    path: &mut Vec<TilePos>,
    levels: &mut Vec<u8>,
    here: (TilePos, u8),
    goal: TilePos,
    keep_goal: bool,
) -> Option<(TilePos, u8)> {
    let next_level = |levels: &Vec<u8>| levels.last().copied().unwrap_or(0);
    let usable = path.last().is_some_and(|&n| {
        let nl = next_level(levels);
        if nl == 0 && here.1 == 0 && n.manhattan(here.0) == 1 {
            world.map.walkable(n)
        } else {
            world.portal_edge(here, (n, nl)).is_some()
        }
    });
    if !usable {
        let (mut p, mut l) = world.big_route(here.0, here.1, goal, on_foot(world, id))?;
        p.reverse();
        l.reverse();
        *path = p;
        *levels = l;
    }
    let n = *path.last()?;
    let nl = next_level(levels);
    if !(keep_goal && n == goal && nl == 0) {
        path.pop();
        levels.pop();
    }
    Some((n, nl))
}

/// C7: a Full body leaving `here` from storey `floor`: inside the same
/// structure (a span on either, the target not full) it moves to the
/// target's storey by one climb (or at once on the same storey) and the GoTo
/// is done at its end; else from a storey above the ground it climbs down to
/// the door first (`ExecState::Climb` with `out`, the walk on after it).
/// `None`: walk out now.
fn climb_out(
    world: &mut World,
    id: EntityId,
    target: &GotoTarget,
    here: EntityId,
    floor: u8,
    path: &[TilePos],
    levels: &[u8],
) -> Option<StepResult> {
    use crate::systems::levels;
    if !levels::on(world) {
        return None;
    }
    let tick = world.tick;
    let core = levels::core_of(world, here);
    if let Some(t) = target.building {
        let spans = [here, t].iter().any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.span.is_some()));
        let full = world.comp::<Building>(t).is_some_and(|bd| bd.is_full()) && !world.capacity_exempt(id, t);
        if spans && !full && levels::core_of(world, t) == core {
            world.leave_building(id);
            world.enter_building(id, t);
            let to = world.comp::<Position>(id).map_or(0, |p| p.floor);
            let Some(p) = levels::climb(world, core, floor, to, id) else { return Some(StepResult::Done) };
            if let Some(pos) = world.comp_mut::<Position>(id) {
                pos.floor = floor;
            }
            let state = ExecState::Climb {
                target: target.clone(),
                core,
                from: floor,
                to,
                depart: tick,
                board: tick + p.wait,
                arrive_tick: tick + p.ticks,
                lift: p.lift,
                out: false,
                path: Vec::new(),
                levels: Vec::new(),
            };
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.exec = state;
            }
            return Some(StepResult::Running);
        }
    }
    if floor == 0 {
        return None;
    }
    let p = levels::climb(world, core, floor, 0, id)?;
    let state = ExecState::Climb {
        target: target.clone(),
        core,
        from: floor,
        to: 0,
        depart: tick,
        board: tick + p.wait,
        arrive_tick: tick + p.ticks,
        lift: p.lift,
        out: true,
        path: path.to_vec(),
        levels: levels.to_vec(),
    };
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.exec = state;
    }
    Some(StepResult::Running)
}

/// C7: a Full body just in `b` at its door: up to the storey it entered on
/// (`Position.floor`, set by `enter_building`) by a climb (`ExecState::Climb`,
/// the GoTo done at its end), standing in the lobby on the ground until then.
/// `Done` at once on the ground (or with `[levels]` off).
fn climb_in(world: &mut World, id: EntityId, target: &GotoTarget, b: EntityId) -> StepResult {
    use crate::systems::levels;
    let floor = world.comp::<Position>(id).map_or(0, |p| p.floor);
    if floor == 0 || !levels::on(world) {
        return StepResult::Done;
    }
    let core = levels::core_of(world, b);
    let Some(p) = levels::climb(world, core, 0, floor, id) else { return StepResult::Done };
    let tick = world.tick;
    if let Some(pos) = world.comp_mut::<Position>(id) {
        pos.floor = 0;
    }
    let state = ExecState::Climb {
        target: target.clone(),
        core,
        from: 0,
        to: floor,
        depart: tick,
        board: tick + p.wait,
        arrive_tick: tick + p.ticks,
        lift: p.lift,
        out: false,
        path: Vec::new(),
        levels: Vec::new(),
    };
    if let Some(br) = world.comp_mut::<Brain>(id) {
        br.exec = state;
    }
    StepResult::Running
}

fn move_to(world: &mut World, id: EntityId, tile: TilePos) {
    if let Some(p) = world.comp_mut::<Position>(id) {
        p.tile = tile;
    }
}

/// Step onto the door and into the building, or queue outside it.
fn try_enter(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    path: Vec<TilePos>,
    levels: Vec<u8>,
    blocked_since: Option<Tick>,
) -> StepResult {
    let tick = world.tick;
    let Some(b) = target.building else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else {
        return StepResult::Failed(FailReason::NoSuchPlace);
    };
    let cap = world.config.exec.door_capacity_per_tick;
    let slots = world.door_queue.get(&door).copied().unwrap_or(0);
    let full = world.comp::<Building>(b).is_some_and(|bd| bd.is_full()) && !world.capacity_exempt(id, b);
    // Round 4 T3 (OPEN-222): with `[exec] staff_door` the building's own
    // staff go in by the staff door, past the public door's queue: a shift
    // starting at once locked itself out (20k seed 42: 66 guards into the
    // Precinct at 08:10-08:44 held its door at 2 a tick and the 67th timed
    // out; V12's `runner_90375` timed out on a 0-tile GoTo 25 times in 6 days).
    let staff =
        world.config.exec.staff_door && world.comp::<crate::components::Job>(id).is_some_and(|j| j.employer == Some(b));
    if staff && !full {
        world.enter_building(id, b);
        return climb_in(world, id, &target, b);
    }
    if slots < cap && !full {
        *world.door_queue.entry(door).or_insert(0) += 1;
        world.enter_building(id, b);
        return climb_in(world, id, &target, b);
    }
    // Pressure pass round 9, Z1: a hunter (or a contract's taker) at a full
    // ask venue or stake-out waits at its door (`hunt::can_start` reads the
    // door): V18's 20k `bereaved_21178` failed BuildingFull at the Bar six
    // times in three days and the hunt cooled each time.
    if full && target.dest == LocationKey::Intel {
        return StepResult::Done;
    }
    let since = blocked_since.unwrap_or(tick);
    if tick - since >= world.config.exec.door_queue_max_ticks {
        if full {
            note_refused(world, id, b);
            return StepResult::Failed(FailReason::BuildingFull);
        }
        return StepResult::Failed(FailReason::Timeout);
    }
    set_goto(world, id, target, path, levels, tick + 1, Some(since), 0)
}

/// Workplace room: remember the full building that refused `id` (`[exec]
/// refused_ticks`; `World::local` passes it over until then).
fn note_refused(world: &mut World, id: EntityId, b: EntityId) {
    if world.config.exec.refused_ticks == 0 {
        return;
    }
    let tick = world.tick;
    if let Some(brain) = world.comp_mut::<Brain>(id) {
        brain.refused = Some((b, tick));
    }
}

#[allow(clippy::too_many_arguments)]
fn set_goto(
    world: &mut World,
    id: EntityId,
    target: GotoTarget,
    path: Vec<TilePos>,
    levels: Vec<u8>,
    next_move_tick: Tick,
    blocked_since: Option<Tick>,
    carry_q: u32,
) -> StepResult {
    if let Some(brain) = world.comp_mut::<Brain>(id) {
        brain.exec = ExecState::Goto { target, path, next_move_tick, blocked_since, carry_q, levels };
    }
    StepResult::Running
}

/// Coarse arrival: leave wherever we are and appear at the destination. A
/// full building is queued at for `door_queue_max_ticks`, as on foot.
pub(super) fn arrive(world: &mut World, id: EntityId, target: GotoTarget, blocked_since: Option<Tick>) -> StepResult {
    let tick = world.tick;
    match target.building {
        Some(b) => {
            if world.comp::<Position>(id).is_some_and(|p| p.building == Some(b)) {
                // The pre-view pass phase 2: across the room to the spot.
                if target.dest == LocationKey::Station {
                    // The pre-view pass phase 3: a spot another occupant
                    // stands on (the night crew's one stock tile) gives the
                    // nearest free interior tile (one body a tile).
                    let mut tile = target.tile;
                    if crate::systems::shift::haul_on(world) {
                        if let Some(bd) = world.comp::<Building>(b) {
                            let taken: Vec<TilePos> = bd
                                .occupants
                                .iter()
                                .filter(|&&o| o != id)
                                .filter_map(|&o| world.comp::<Position>(o).map(|p| p.tile))
                                .collect();
                            if taken.contains(&tile) {
                                if let Some(t) = bd
                                    .interior()
                                    .filter(|t| !taken.contains(t) && world.map.walkable(*t))
                                    .min_by_key(|&t| (t.manhattan(target.tile), t))
                                {
                                    tile = t;
                                }
                            }
                        }
                    }
                    if let Some(p) = world.comp_mut::<Position>(id) {
                        p.tile = tile;
                    }
                }
                return StepResult::Done;
            }
            if world.comp::<Building>(b).is_some_and(|bd| bd.is_full()) && !world.capacity_exempt(id, b) {
                // Round 9 Z1: a full ask venue or stake-out is waited at
                // from its door (as on foot, `try_enter`).
                if target.dest == LocationKey::Intel {
                    let outside = world.comp::<Building>(b).map(|bd| world.outside_door(bd));
                    world.leave_building(id);
                    if let (Some(t), Some(p)) = (outside, world.comp_mut::<Position>(id)) {
                        p.tile = t;
                    }
                    return StepResult::Done;
                }
                let since = blocked_since.unwrap_or(tick);
                if tick - since >= world.config.exec.door_queue_max_ticks {
                    note_refused(world, id, b);
                    return StepResult::Failed(FailReason::BuildingFull);
                }
                if let Some(brain) = world.comp_mut::<Brain>(id) {
                    brain.exec = ExecState::GotoTimed {
                        target,
                        arrive_tick: tick + 1,
                        blocked_since: Some(since),
                        walk: None,
                        climb: None,
                    };
                }
                return StepResult::Running;
            }
            world.leave_building(id);
            world.enter_building(id, b);
            StepResult::Done
        }
        None => {
            world.leave_building(id);
            move_to(world, id, target.tile);
            // S1: a timed walk ends on the ground (one set out from an El
            // platform's deck kept its level).
            if let Some(p) = world.comp_mut::<Position>(id) {
                p.floor = 0;
            }
            StepResult::Done
        }
    }
}
