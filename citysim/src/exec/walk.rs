use super::*;

/// C7: the level `id` stands on in the street (a walkway's deck above 0;
/// 0 indoors, where the walk starts from the door).
pub fn street_level(world: &World, id: EntityId) -> u8 {
    world.comp::<Position>(id).filter(|p| p.building.is_none()).map_or(0, |p| p.floor)
}

/// C7: is `id` on foot (not at a wheel)? A walker's path takes the portals.
pub fn on_foot(world: &World, id: EntityId) -> bool {
    !world.comp::<crate::components::Kit>(id).is_some_and(|k| k.driving.is_some())
}

/// A Coarse Goto: arrive after Manhattan distance × move ticks; M12 D18: ×
/// `1 + timed_mult × litter` of the target's district, rounded. M13 D21:
/// × the vehicle's `timed_mult` when driving (else `Kit.walk_mult`); at
/// exactly 1.0 the integer formula runs unchanged (a branch, not a multiply).
pub fn timed_goto(world: &World, id: EntityId, target: GotoTarget) -> ExecState {
    let from = walk_origin(world, id).unwrap_or(target.tile);
    let mut ticks = timed_walk(world, id, from, target.tile);
    // The city milestone C7: the climbs down out of the building left and
    // up to the target's storey (a move inside one structure only climbs).
    let climb = timed_climbs(world, id, &target, ticks);
    if climb.as_ref().is_some_and(|c| c.inside) {
        ticks = 0;
    }
    let walk_ticks = ticks;
    if let Some(c) = &climb {
        ticks += c.down_ticks() + c.up_ticks();
    }
    let arrive_tick = world.tick + ticks;
    // The island pass phase 9 (OPEN-181): the walk keeps its start, so its
    // body has a place on its way at every tick (`walk_place`).
    let walk = (walks_on(world) && from != target.tile && walk_ticks > 0).then_some(Walk { from, depart: world.tick });
    ExecState::GotoTimed { target, arrive_tick, blocked_since: None, walk, climb }
}

/// C7: a timed leg's climbs ([`TimedClimb`]): down from the storey `id`
/// stands on to the door of the building he is leaving, and up from the
/// target's door to the storey he will stand on there (`floors::floor_in`),
/// the lift's queue read at the tick he reaches its lobby; a move between
/// two buildings of one structure (a span on either) climbs straight between
/// the storeys. `None`: no climb (`[levels]` off, on the ground both ends).
fn timed_climbs(world: &World, id: EntityId, target: &GotoTarget, walk_ticks: Tick) -> Option<TimedClimb> {
    use crate::systems::levels;
    if !levels::on(world) {
        return None;
    }
    let p = world.comp::<Position>(id)?;
    let tick = world.tick;
    if let (Some(h), Some(t)) = (p.building, target.building) {
        if h == t {
            return None;
        }
        let core = levels::core_of(world, h);
        let spans = [h, t].iter().any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.span.is_some()));
        if spans && levels::core_of(world, t) == core {
            let to = crate::systems::floors::floor_in(world, id, t);
            let plan = levels::plan(world, core, p.floor, to, tick, Some(id))?;
            let up = ClimbLeg { core, building: t, from: p.floor, to, plan };
            return Some(TimedClimb { down: None, up: Some(up), booked: false, depart: tick, inside: true });
        }
    }
    let down = p.building.filter(|_| p.floor > 0).and_then(|h| {
        let core = levels::core_of(world, h);
        levels::plan(world, core, p.floor, 0, tick, Some(id)).map(|plan| ClimbLeg {
            core,
            building: h,
            from: p.floor,
            to: 0,
            plan,
        })
    });
    let reach = tick + down.map_or(0, |d| d.plan.ticks) + walk_ticks;
    let up = target.building.and_then(|t| {
        let to = crate::systems::floors::floor_in(world, id, t);
        if to == 0 {
            return None;
        }
        let core = levels::core_of(world, t);
        levels::plan(world, core, 0, to, reach, Some(id)).map(|plan| ClimbLeg { core, building: t, from: 0, to, plan })
    });
    (down.is_some() || up.is_some()).then_some(TimedClimb { down, up, booked: false, depart: tick, inside: false })
}

/// C7: book a timed leg's climbs (`levels::book`): the climb down at its
/// start, the climb up at the tick it reaches the lobby.
pub(super) fn book_timed(world: &mut World, id: EntityId) {
    let Some(ExecState::GotoTimed { climb: Some(c), arrive_tick, .. }) =
        world.comp::<Brain>(id).map(|b| b.exec.clone())
    else {
        return;
    };
    if let Some(d) = c.down {
        crate::systems::levels::book(world, d.core, d.plan, d.from.max(d.to), c.depart);
    }
    if let Some(u) = c.up {
        crate::systems::levels::book(world, u.core, u.plan, u.from.max(u.to), arrive_tick.saturating_sub(u.plan.ticks));
    }
    if let Some(ExecState::GotoTimed { climb: Some(c), .. }) = world.comp_mut::<Brain>(id).map(|b| &mut b.exec) {
        c.booked = true;
    }
}

/// C7: where a body on a climb is (its building, the tile it stands on and
/// its storey as a fraction): a Full body's `ExecState::Climb`, a Coarse
/// timed leg's climb down (before its walk) or up (after it). `None`: on no
/// climb now.
pub fn climb_place(world: &World, id: EntityId) -> Option<(TilePos, EntityId, f32)> {
    use crate::systems::levels::storey_at;
    let b = world.comp::<Brain>(id)?;
    let tick = world.tick;
    match &b.exec {
        ExecState::Climb { from, to, depart, board, arrive_tick, .. } => {
            let p = world.comp::<Position>(id)?;
            Some((p.tile, p.building?, storey_at(*from, *to, *depart, *board, *arrive_tick, tick)))
        }
        ExecState::GotoTimed { target, arrive_tick, climb: Some(c), .. } => {
            if let Some(d) = c.down {
                let end = c.depart + d.plan.ticks;
                if tick < end {
                    let p = world.comp::<Position>(id)?;
                    let f = storey_at(d.from, d.to, c.depart, c.depart + d.plan.wait, end, tick);
                    return Some((p.tile, d.building, f));
                }
            }
            let u = c.up?;
            let start = arrive_tick.saturating_sub(u.plan.ticks);
            if tick < start || tick >= *arrive_tick {
                return None;
            }
            let f = storey_at(u.from, u.to, start, start + u.plan.wait, *arrive_tick, tick);
            Some((target.tile, u.building, f))
        }
        // V8 (`OPEN-264`): a raider's climb is his own `Climb` above.
        _ => None,
    }
}

/// `[motion.walks]` on (the island pass phase 9).
pub fn walks_on(world: &World) -> bool {
    world.config.motion.as_ref().is_some_and(|m| m.walks.is_some())
}

/// The island pass phase 9 (OPEN-181): a Coarse timed walk on foot in
/// progress: its way (`way[0]` the start, the last the target's tile, the
/// door for a building), its start and its arrival. `None`: `id` is on no
/// such walk (aboard a vehicle, a ride, the door queue, `[motion.walks]`
/// off). The way costs one road search (the Full walk's A*; an L with
/// `road_route` false): a reader drawing it every tick keeps it by
/// `(id, depart)`, as it is fixed for the walk.
pub fn walk_route(world: &World, id: EntityId) -> Option<(Vec<TilePos>, Tick, Tick)> {
    let b = world.comp::<Brain>(id)?;
    let ExecState::GotoTimed { target, arrive_tick, walk: Some(w), climb, .. } = &b.exec else { return None };
    if crate::systems::motion::vehicle_of(world, id).is_some() {
        return None;
    }
    let road = world.config.motion.as_ref().and_then(|m| m.walks.as_ref()).is_some_and(|w| w.road_route);
    let way = crate::systems::motion::walk_way(world, w.from, target.tile, road);
    // C7: the walk between the climbs.
    let (depart, arrive) = match climb {
        Some(c) => (w.depart + c.down_ticks(), arrive_tick.saturating_sub(c.up_ticks())),
        None => (w.depart, *arrive_tick),
    };
    Some((way, depart, arrive.max(depart)))
}

/// The island pass phase 9 (OPEN-181): where a body is now, for the view
/// and the device: a Coarse walker mid-walk is on his way (the point at the
/// walk's pace, out of the building he left); anyone else is at his
/// `Position`. The sim's own `Position` of a Coarse walker stays at the
/// walk's start until he arrives (or a promotion or an interrupt sets him
/// down on his way, `settle_walk`): nothing is moved per tick.
pub fn walk_place(world: &World, id: EntityId) -> Option<Position> {
    // C7: on a climb, at the door of the building climbed, on the storey reached.
    if let Some((tile, b, f)) = climb_place(world, id) {
        return Some(Position { tile, building: Some(b), entered: world.tick, floor: f.round() as u8 });
    }
    if let Some((way, depart, arrive)) = walk_route(world, id) {
        let tile = crate::systems::motion::timed_pos(&way, depart, arrive, world.tick);
        return Some(Position { tile, building: None, entered: depart, floor: 0 });
    }
    world.comp::<Position>(id).cloned()
}

/// The island pass phase 9 (OPEN-181): a Coarse walker leaves his walk
/// mid-way (a promotion to Full, an interrupt): his body is set down where
/// the walk has him now, out of any building, and the walk forgets its
/// start. A no-op off a walk, on its first tick, and for a cuffed body (the
/// guard holds him where he stands).
pub fn settle_walk(world: &mut World, id: EntityId) {
    let Some(b) = world.comp::<Brain>(id) else { return };
    let ExecState::GotoTimed { walk: Some(w), arrive_tick, .. } = &b.exec else { return };
    if world.tick <= w.depart || world.tick >= *arrive_tick || b.cuffed_by.is_some() {
        return;
    }
    // C7: still on the climb down, he is still in the building he left.
    if climb_place(world, id)
        .is_some_and(|(_, bd, _)| world.comp::<Position>(id).is_some_and(|p| p.building == Some(bd)))
    {
        return;
    }
    let Some((way, depart, arrive)) = walk_route(world, id) else { return };
    let tile = crate::systems::motion::timed_pos(&way, depart, arrive, world.tick);
    if world.comp::<Position>(id).is_some_and(|p| p.building.is_some()) {
        world.remove_from_building(id);
    }
    if let Some(p) = world.comp_mut::<Position>(id) {
        p.tile = tile;
        p.building = None;
        p.floor = 0;
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        if let ExecState::GotoTimed { walk, .. } = &mut b.exec {
            *walk = None;
        }
    }
}

/// `settle_walk` for an interrupt (`abort_plan`, `plan::install`), when
/// `[motion.walks] settle_interrupt` is on.
pub fn settle_interrupted(world: &mut World, id: EntityId) {
    let on = world.config.motion.as_ref().and_then(|m| m.walks.as_ref()).is_some_and(|w| w.settle_interrupt);
    if on {
        settle_walk(world, id);
    }
}

/// The ticks of a timed walk from `from` to `to` (`timed_goto`'s; the L2
/// shadow fixes' commute estimate, `fixes::walk_estimate`).
pub fn timed_walk(world: &World, id: EntityId, from: TilePos, to: TilePos) -> Tick {
    let tiles = Tick::from(from.manhattan(to));
    let m = crate::systems::vehicles::timed_mult(world, id);
    let mut litter = 1.0;
    if crate::systems::litter::enabled(world) {
        let dirt = world.district(world.district_of(to)).litter;
        litter = 1.0 + world.config.litter.timed_mult * dirt;
    }
    // The view milestone V7: on foot, the signals' expected waits at the
    // crossings the walk is likely to make (`[crossings]`), rounded.
    let signals = match &world.config.crossings {
        Some(c) if on_foot(world, id) => crate::signals::timed_wait_ticks(c, tiles).round() as Tick,
        _ => 0,
    };
    // Round 9 Y4: the walk at the pace (`[exec] walk_ticks`), scaled
    // before the rounding, so a car's share is not lost to a short walk's
    // few ticks.
    signals
        + if m == 1.0 {
            if litter > 1.0 {
                world.config.exec.walk_ticks_f(tiles, litter)
            } else {
                world.config.exec.walk_ticks(tiles)
            }
        } else {
            world.config.exec.walk_ticks_f(tiles, m * litter.max(1.0))
        }
}

/// The perf pass phase 2: the most Manhattan tiles apart two tiles can be
/// and still have `timed_walk` for `id` at most `cap` (every pair further
/// apart walks longer), or `u32::MAX` when a pair `limit` apart still does.
/// The floor under `timed_walk` at `d` tiles is its litter-free value
/// (litter only lengthens a walk: its factor is used only above 1), which
/// never falls as `d` grows and is 0 at 0 tiles, so the answer is a
/// bisection over `0..=limit`.
pub fn walk_reach_tiles(world: &World, id: EntityId, cap: Tick, limit: u32) -> u32 {
    let m = crate::systems::vehicles::timed_mult(world, id);
    let floor = |d: u32| {
        if m == 1.0 {
            world.config.exec.walk_ticks(Tick::from(d))
        } else {
            world.config.exec.walk_ticks_f(Tick::from(d), m)
        }
    };
    if floor(limit) <= cap {
        return u32::MAX;
    }
    // floor(lo) <= cap < floor(hi).
    let (mut lo, mut hi) = (0u32, limit);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if floor(mid) <= cap {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// S1 (`OPEN-317`), for the view: how far a Full walker is through the step
/// to the next tile of its way, at the tick boundary just run (`World::tick`
/// read after the tick, as the view reads it), in `[0, 1]`. The executor
/// steps whole tiles at sub-tick times (`[exec] units_per_tick`, 3.5 tiles a
/// tick at the T1 pace: 4, 3, 4, ...), so a body's tile alone surges 25 %
/// tick on tick; drawn at `tile + stride × (next − tile)`, `next` the first
/// tile of its way ahead, it covers the same ground every tick. 0 standing
/// (a kerb's wait, a door's queue), 1 a step landing on the boundary.
/// `None`: not a Full walk.
pub fn stride(world: &World, id: EntityId) -> Option<f32> {
    let b = world.comp::<Brain>(id)?;
    let ExecState::Goto { next_move_tick, carry_q, blocked_since, hop, .. } = &b.exec else { return None };
    if blocked_since.is_some() {
        return Some(0.0);
    }
    let upt = world.config.exec.units_per_tick();
    // S3 (`OPEN-319`): on a walkway hop the step is the whole hop, begun at `hop`.
    if let Some(depart) = hop {
        let (from_q, to_q) = (depart * upt, next_move_tick * upt + u64::from(*carry_q));
        let span = to_q.saturating_sub(from_q).max(1);
        return Some(((world.tick * upt).saturating_sub(from_q) as f32 / span as f32).clamp(0.0, 1.0));
    }
    let tile = world.comp::<Position>(id)?.tile;
    let step = u64::from(crate::systems::vehicles::step_q(world, id, tile)).max(1);
    let next_q = next_move_tick * upt + u64::from(*carry_q);
    let now_q = world.tick * upt;
    let left = next_q.saturating_sub(now_q);
    Some((1.0 - left as f32 / step as f32).clamp(0.0, 1.0))
}

/// M12 D18: the extra step units before a Full mover's next step off
/// `tile`, by the tile's litter band (`[litter] step_share`, a share of a
/// walker step at the pace, `[exec] step_units`; S1, `OPEN-317`: whole
/// steps before, so a heap's walker crawled at a third of the pace; round 9
/// Y4, ticks before it, when a step was a tick). One byte read on a step
/// already taken; flow fields never change.
pub fn litter_delay(world: &World, tile: TilePos) -> Tick {
    if !crate::systems::litter::enabled(world) {
        return 0;
    }
    let band = crate::systems::litter::band(crate::systems::litter::at(world, tile));
    let share = world.config.litter.step_share.get(band).copied().unwrap_or(0.0).max(0.0);
    (share * world.config.exec.step_units() as f32).round() as Tick
}

/// A Full Goto: flow field for buildings, A* otherwise. `None` if unreachable.
pub fn walking_goto(world: &World, id: EntityId, target: GotoTarget) -> Option<ExecState> {
    if world.config.exec.straight_line_paths {
        return Some(timed_goto(world, id, target));
    }
    // Scale S3a: on a big map a building's path is found on the first step
    // (`advance_goto`, from outside the building being left).
    let (path, levels) = if target.building.is_some() {
        (Vec::new(), Vec::new())
    } else if world.big_paths() {
        let from = walk_origin(world, id)?;
        let (mut p, mut l) = world.big_route(from, street_level(world, id), target.tile, on_foot(world, id))?;
        p.reverse();
        l.reverse();
        (p, l)
    } else {
        let from = walk_origin(world, id)?;
        let mut p = pathfind::astar(&world.map, from, target.tile, world.config.exec.astar_max_expansions)?;
        p.reverse();
        (p, Vec::new())
    };
    Some(ExecState::Goto {
        target,
        path,
        next_move_tick: world.tick,
        blocked_since: None,
        carry_q: 0,
        levels,
        hop: None,
    })
}
