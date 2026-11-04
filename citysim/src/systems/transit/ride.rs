use super::*;

/// The plan step a ride belongs to: the plan's start tick and step index.
pub(super) fn plan_key(world: &World, id: EntityId) -> Option<(Tick, u8)> {
    let b = world.comp::<crate::components::Brain>(id)?;
    b.plan.as_ref().map(|p| (p.started_tick, b.plan_step))
}

/// A walk toward `target` from where the agent stands, at its tier (a Full
/// walk unreachable on foot falls back to a timed one).
fn walk_leg(world: &World, id: EntityId, target: GotoTarget, lod: Lod) -> ExecState {
    match lod {
        Lod::Full => crate::exec::walking_goto(world, id, target.clone())
            .unwrap_or_else(|| crate::exec::timed_goto(world, id, target)),
        _ => crate::exec::timed_goto(world, id, target),
    }
}

/// S1 (`OPEN-309`): stop `stop`'s platform, when it has one.
pub fn platform_of(world: &World, stop: u16) -> Option<&Platform> {
    world.transit.net.platforms.get(usize::from(stop))?.as_ref()
}

/// S1 (`OPEN-309`): a Full rider's walk to stop `stop`: with a platform, the
/// street to the stairs' (lift's) foot, up them (the portal's run, as any
/// deck's stairs are walked) and along the deck to the waiting place;
/// without, to the stop tile.
fn access_walk(world: &World, id: EntityId, stop: u16) -> Option<ExecState> {
    let tile = world.transit.net.stops[usize::from(stop)];
    let plat = platform_of(world, stop).filter(|_| world.big_paths());
    let Some(plat) = plat else {
        return crate::exec::walking_goto(world, id, GotoTarget { dest: LocationKey::Street, tile, building: None });
    };
    let foot = GotoTarget { dest: LocationKey::Street, tile: plat.foot, building: None };
    let mut state = crate::exec::walking_goto(world, id, foot)?;
    if let ExecState::Goto { target, path, levels, .. } = &mut state {
        // The path is reversed (its last element the next step): the deck's
        // steps go in at its front, the waiting place first.
        if levels.len() != path.len() {
            *levels = vec![0; path.len()];
        }
        for &(t, l) in &plat.way {
            path.insert(0, t);
            levels.insert(0, l);
        }
        target.tile = plat.wait().0;
    }
    Some(state)
}

/// S1 (`OPEN-309`): stand `id` at stop `stop`, on its platform's waiting
/// place when it has one (boarding or alighting there, level with the car),
/// else on the stop tile.
pub(super) fn stand_at_stop(world: &mut World, id: EntityId, stop: u16) {
    let (tile, level) = match platform_of(world, stop) {
        Some(p) => p.wait(),
        None => (world.transit.net.stops[usize::from(stop)], 0),
    };
    if let Some(p) = world.comp_mut::<Position>(id) {
        p.tile = tile;
        p.building = None;
        p.floor = level;
    }
}

/// A `GoTo` toward `target` starts on foot (no vehicle): ride if `choose`
/// says so. Files the `Ride` and returns the first leg's state (the walk to
/// the board stop); `None` walks as before. A no-op with transit off.
pub fn try_ride(world: &mut World, id: EntityId, target: &GotoTarget, lod: Lod) -> Option<ExecState> {
    if !enabled(world) || lod == Lod::Statistical {
        return None;
    }
    ensure_net(world);
    if world.transit.net.stops.is_empty() {
        return None;
    }
    let from = crate::exec::walk_origin(world, id)?;
    let route = match choose(world, id, from, target.tile, world.tick) {
        Ok(r) => r,
        Err(r) => {
            count_refusal(world, r);
            return None;
        }
    };
    let (plan_started, plan_step) = plan_key(world, id)?;
    let board_tile = world.transit.net.stops[usize::from(route.board)];
    let state = match lod {
        Lod::Full => access_walk(world, id, route.board)?,
        _ => crate::exec::timed_goto(
            world,
            id,
            GotoTarget { dest: LocationKey::Street, tile: board_tile, building: None },
        ),
    };
    // The island pass phase 6: the legs the route scheduled.
    let legs =
        if trains_on(world) { legs_of(world, route.board, route.alight).unwrap_or_default() } else { Vec::new() };
    let hop_ticks = if legs.is_empty() { route.hop.ticks } else { (route.arrive - route.depart) as u32 };
    let ride = Ride {
        target: target.clone(),
        board: route.board,
        alight: route.alight,
        line: route.hop.line,
        forward: route.hop.forward,
        hop_ticks,
        depart: route.depart,
        arrive: route.arrive,
        dodge: route.dodge,
        plan_started,
        plan_step,
        board_pos: Some(route.hop.pos),
        legs,
        leg: 0,
    };
    world.rides.insert(id, ride);
    Some(state)
}

/// A leg of a `GoTo` ended (`exec::step_agent`'s Done): at the board stop,
/// board; off the train, alight and walk on. `false` when the agent has no
/// ride for this step (an ordinary arrival; a stale ride is dropped).
pub fn leg_done(world: &mut World, id: EntityId, lod: Lod) -> bool {
    let Some(ride) = world.rides.get(&id).cloned() else { return false };
    if plan_key(world, id) != Some((ride.plan_started, ride.plan_step)) {
        world.rides.remove(&id);
        return false;
    }
    let aboard = matches!(world.comp::<crate::components::Brain>(id).map(|b| &b.exec), Some(ExecState::Ride { .. }));
    if let Some(e) = world.transit.evac.remove(&id).filter(|_| aboard) {
        // The island pass phase 6: at a cut line's stop, walking on.
        if let (Some(&at), Some(p)) = (e.way.last(), world.comp_mut::<Position>(id)) {
            p.tile = at;
        }
        abandon(world, id, &ride, lod);
    } else if aboard && usize::from(ride.leg) + 1 < ride.legs.len() {
        transfer(world, id, ride, lod);
    } else if aboard {
        alight(world, id, &ride, lod);
    } else {
        board(world, id, ride, lod);
    }
    true
}

/// Give a ride up where the agent stands and walk on.
pub(super) fn abandon(world: &mut World, id: EntityId, ride: &Ride, lod: Lod) {
    world.rides.remove(&id);
    world.stats.current.transit.ride_abandoned += 1;
    let state = walk_leg(world, id, ride.target.clone(), lod);
    if let Some(b) = world.comp_mut::<crate::components::Brain>(id) {
        b.exec = state;
    }
}

/// Jump the gate: a FareEvasion crime at the stop, to the witnesses and the
/// law as any crime (Dylan, 2026-10-10: a petty record from day one).
pub(super) fn dodge(world: &mut World, id: EntityId, tile: TilePos) {
    world.stats.current.transit.fare_dodges += 1;
    crate::systems::law::raise_crime(world, id, None, Crime::FareEvasion, tile);
}

/// On the platform: the departure (the planned one, or the next if the walk
/// ran late), the fare or the gate jumped, the boarding counted.
pub(super) fn board(world: &mut World, id: EntityId, mut ride: Ride, lod: Lod) {
    let now = world.tick;
    let depart = if now <= ride.depart {
        Some(ride.depart)
    } else {
        board_pos(world, &ride).and_then(|k| departure_at(world, ride.line, ride.forward, k, now))
    };
    // M17 phase 2 (a found gap, god_line_cut): a ride filed before a CutLine
    // does not board the cut line after it.
    let line_open = world.transit.line_closed.get(&ride.line).is_none_or(|&until| until <= now);
    let Some(depart) = depart.filter(|&d| line_open && in_service(world, d) && stop_open(world, ride.board, now))
    else {
        abandon(world, id, &ride, lod);
        return;
    };
    let tile = world.transit.net.stops[usize::from(ride.board)];
    let fare = world.config.transit.fare;
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    // Pressure pass round 2, R1: a rider banned since the ride was planned
    // walks, or jumps the gate in breach of the ban.
    let banned = crate::systems::fares::banned(world, id);
    let wary = crate::systems::fares::wary(world, id);
    if banned && (wary || !crate::systems::fares::breaks_ban(world, id)) {
        crate::systems::ownership::jobs2_row(world).fare_ban_walks += 1;
        abandon(world, id, &ride, lod);
        return;
    }
    if wary && !banned && coins < fare {
        abandon(world, id, &ride, lod);
        return;
    }
    if banned || (!wary && (ride.dodge || (coins < fare && law < world.config.transit.dodge_lawfulness))) {
        dodge(world, id, tile);
        // R1: caught and banned now, he is turned off the platform and walks
        // (a fined dodger rides on: the fine bought the ride).
        if crate::systems::fares::caught_now(world, id)
            && crate::systems::fares::banned(world, id)
            && plan_key(world, id) == Some((ride.plan_started, ride.plan_step))
        {
            crate::systems::ownership::jobs2_row(world).fare_ban_walks += 1;
            abandon(world, id, &ride, lod);
            return;
        }
    } else if coins >= fare {
        crate::systems::ownership::pay(world, Some(id), None, fare, crate::systems::ownership::Flow::Fare);
    } else {
        abandon(world, id, &ride, lod);
        return;
    }
    // The crime may have ended the plan (a guard on the platform).
    if plan_key(world, id) != Some((ride.plan_started, ride.plan_step)) {
        world.rides.remove(&id);
        return;
    }
    {
        let c = &mut world.stats.current.transit;
        match lod {
            Lod::Full => c.rides_full += 1,
            _ => c.rides_coarse += 1,
        }
    }
    count_boarding(world, ride.board, depart);
    // S1 (`OPEN-309`): the wait is on the platform. A Full rider walked up;
    // a Coarse one (a timed walk to the stop tile) or one whose way up was
    // lost stands there now.
    if platform_of(world, ride.board).is_some() && world.comp::<Position>(id).is_some_and(|p| p.floor == 0) {
        stand_at_stop(world, id, ride.board);
    }
    ride.depart = depart;
    ride.arrive = depart + Tick::from(ride.hop_ticks);
    let mut state = ExecState::Ride { target: ride.target.clone(), depart, arrive_tick: ride.arrive };
    // The island pass phase 6: the first leg's train, the whole ride
    // scheduled from its departure; the platform until it comes.
    if let Some(&leg) = ride.legs.first() {
        ride.arrive = schedule(world, &ride.legs, depart).map_or(ride.arrive, |x| x.1);
        state = ExecState::Ride { target: ride.target.clone(), depart, arrive_tick: depart + leg_ticks(world, leg) };
        world.transit.boarding.insert((depart, id));
    }
    world.rides.insert(id, ride);
    if let Some(b) = world.comp_mut::<crate::components::Brain>(id) {
        b.exec = state;
    }
}

/// A boarding at `stop` in the hour of `at`, into today's load ledger
/// (T17) and the day's peak.
fn count_boarding(world: &mut World, stop: u16, at: Tick) {
    let hour = ((at % TICKS_PER_DAY) / TICKS_PER_HOUR) as usize;
    let slot = world.transit.load.entry(stop).or_insert([0; 24]);
    slot[hour] = slot[hour].saturating_add(1);
    let peak = u32::from(slot[hour]);
    let c = &mut world.stats.current.transit;
    c.peak_stop_load = c.peak_stop_load.max(peak);
}

/// Off at the alight stop: stand on it and walk on to the `GoTo`'s target.
pub(super) fn alight(world: &mut World, id: EntityId, ride: &Ride, lod: Lod) {
    world.rides.remove(&id);
    crate::systems::motion::alight_now(world, id);
    // S1 (`OPEN-309`): onto the platform (the walk on goes down its stairs).
    stand_at_stop(world, id, ride.alight);
    if world.commute_start.contains_key(&id) {
        world.transit.rode.insert(id);
    }
    let state = walk_leg(world, id, ride.target.clone(), lod);
    if let Some(b) = world.comp_mut::<crate::components::Brain>(id) {
        b.exec = state;
    }
}

/// A ride ends with its plan (an abort, a death, an emigrant leaving).
pub fn end_ride(world: &mut World, id: EntityId) {
    if world.rides.is_empty() && world.transit.rode.is_empty() {
        return;
    }
    world.rides.remove(&id);
    world.transit.rode.remove(&id);
    if !world.transit.evac.is_empty() {
        world.transit.evac.remove(&id);
    }
}

// ---------------------------------------------------------------------------
// Off screen (phase 2: T13, plan D8)
// ---------------------------------------------------------------------------

/// T13: a Statistical worker's commute on a workday, at the shift's first
/// hour on (the caller has checked the shift is pending and today a
/// workday): the Home door to the work door as `choose` would ride it,
/// setting out a walk before the shift. A ride pays two fares (out and
/// back; a leg it can't pay is jumped and counted, no crime off screen) and
/// counts two boardings: the board stop at the departure's hour, the alight
/// stop at the shift's end; none on a no-show at `attend` (the role's
/// `p_attend`, review item 2). No movement, no RNG. Errands are not modelled
/// off screen. Plan D8's per-agent route cache is not built: the choice is
/// a few table lookups once a workday, and a cache left out of the save
/// would make a loaded world choose differently.
pub fn stat_commute(world: &mut World, id: EntityId, job: &crate::components::Job, attend: Option<f32>) {
    if !enabled(world) || world.transit.net.stops.is_empty() {
        return;
    }
    let tod = world.tick_of_day();
    let wraps = job.shifts.iter().any(|&(_, e)| e == 1440);
    let first_hour =
        |&&(s, e): &&(u16, u16)| tod >= s && tod < e && tod - s < TICKS_PER_HOUR as u16 && !(s == 0 && wraps);
    let Some(&(start, _)) = job.shifts.iter().find(first_hour) else { return };
    // Review item 2: a no-show on the role's attendance `attend` (no hour
    // of this shift worked, `lod::shift_attended`) makes no commute.
    if attend.is_some_and(|p| !crate::systems::lod::shift_attended(world, id, job, p)) {
        return;
    }
    let Some(work) = job.employer.and_then(|e| world.comp::<crate::components::Building>(e)).map(|b| b.door) else {
        return;
    };
    let Some(home) = crate::systems::contracts::home_door(world, id) else { return };
    let walk = crate::exec::timed_walk(world, id, home, work);
    let shift_start = world.tick - Tick::from(tod - start);
    world.stats.current.transit.stat_commutes += 1;
    let leave = shift_start.saturating_sub(walk);
    let model = choose(world, id, home, work, leave);
    // Parity (commute rides): the model's answer from the Home door,
    // corrected by the Full city's ride share per answer
    // (`StatTable.p_commute_ride`): a commute that sets out from elsewhere
    // may ride where the Home door's would walk.
    let cell = world.stat_table.as_ref().and_then(|t| t.p_commute_ride).map(|p| p[usize::from(model.is_ok())]);
    let r = match cell {
        None => model.ok(),
        Some(p) => {
            let u = crate::systems::econ::hash_unit(world.seed(), 0xfa2e_0002, u64::from(id.index), world.day());
            if u >= p {
                None
            } else {
                model.ok().or_else(|| {
                    let dodge = pays_or_dodges(world, id)?;
                    route(world, id, home, work, leave).map(|r| Route { dodge, ..r })
                })
            }
        }
    };
    let Some(r) = r else { return };
    world.stats.current.transit.stat_commute_rides += 1;
    let fare = world.config.transit.fare;
    let mut dodged = 0u64;
    for _ in 0..2 {
        let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
        if r.dodge || coins < fare {
            world.stats.current.transit.fare_dodges += 1;
            dodged += 1;
        } else {
            crate::systems::ownership::pay(world, Some(id), None, fare, crate::systems::ownership::Flow::Fare);
        }
    }
    world.stats.current.transit.rides_stat += 2;
    // Phase 3 (parity): a jumped leg is reported at the Full city's rate
    // (`StatTable.p_dodge_caught`, a pure hash of the agent and the day), and
    // the body comes on screen for the arrest, as an off-screen theft's does.
    let p = world.stat_table.as_ref().and_then(|t| t.p_dodge_caught).unwrap_or(0.0);
    let caught = (0..dodged).any(|leg| {
        crate::systems::econ::hash_unit(world.seed(), 0xfa2e_0001, u64::from(id.index), world.day() * 2 + leg) < p
    });
    if caught && crate::systems::fares::on(world) {
        // Pressure pass round 2, R1: the ladder, coarse (no body, no cell).
        crate::systems::fares::stat_caught(world, id);
    } else if caught {
        crate::systems::law::file_report(world, Crime::FareEvasion, id, None);
        crate::systems::lod::set_lod(world, id, Lod::Coarse);
    }
    let back = job.shift_end(world.tick).unwrap_or(world.tick);
    for (stop, at) in [(r.board, r.depart), (r.alight, back)] {
        count_boarding(world, stop, at);
    }
}

/// Parity (arrests): a Statistical adult's non-commute ride this hour, on
/// its row's `StatTable.p_errand_ride` (a pure hash of the agent and the
/// hour), when it would pay the fare or jump the gate (`pays_or_dodges`, the
/// rate's own denominator). The fare is paid as a body's; a jumped gate is
/// counted and reported at `p_dodge_caught`, and the body comes on screen
/// for the arrest (`stat_commute`'s rule). No movement, no stop load. True
/// when the agent rode.
pub fn stat_errand(world: &mut World, id: EntityId, row: usize) -> bool {
    if !enabled(world) || world.transit.net.stops.is_empty() {
        return false;
    }
    let Some(p) = world.stat_table.as_ref().and_then(|t| t.p_errand_ride).map(|p| p[row]) else { return false };
    let hour = world.tick / TICKS_PER_HOUR;
    if crate::systems::econ::hash_unit(world.seed(), 0xfa2e_0003, u64::from(id.index), hour) >= p {
        return false;
    }
    // Review item 3: gated as a body's boarding is (`can_board_here`).
    if !can_board_here(world, id) {
        return false;
    }
    let Some(dodge) = pays_or_dodges(world, id) else { return false };
    world.stats.current.transit.stat_errand_rides += 1;
    if !dodge {
        let fare = world.config.transit.fare;
        crate::systems::ownership::pay(world, Some(id), None, fare, crate::systems::ownership::Flow::Fare);
        return true;
    }
    world.stats.current.transit.fare_dodges += 1;
    let p = world.stat_table.as_ref().and_then(|t| t.p_dodge_caught).unwrap_or(0.0);
    let caught = crate::systems::econ::hash_unit(world.seed(), 0xfa2e_0004, u64::from(id.index), hour) < p;
    if caught && crate::systems::fares::on(world) {
        // Pressure pass round 2, R1: the ladder, coarse.
        crate::systems::fares::stat_caught(world, id);
    } else if caught {
        crate::systems::law::file_report(world, Crime::FareEvasion, id, None);
        crate::systems::lod::set_lod(world, id, Lod::Coarse);
    }
    true
}

/// Review item 3: could `id` board a train now from where it stands, as a
/// body's boarding is gated (`board`, `route`): the El in service now, and
/// among its nearest open stops (`candidates`: no god closure, no riot, no
/// night curfew there) one a running line calls at (a cut line's stops see
/// no train). `stat_errand`'s gate and `calibrate`'s denominator for
/// `p_errand_ride` (the hours an errand ride was possible).
pub fn can_board_here(world: &World, id: EntityId) -> bool {
    let now = world.tick;
    if !enabled(world) || !in_service(world, now) {
        return false;
    }
    let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) else { return false };
    let net = &world.transit.net;
    let served = |s: u16| {
        net.lines
            .iter()
            .enumerate()
            .any(|(l, line)| net.open.get(l).copied().unwrap_or(false) && line.stops.contains(&s))
    };
    candidates(world, tile, now).into_iter().any(served)
}

/// The app's lerp: where a rider aboard is on the track now, in tiles (a
/// point between two stops of its line, by the ride offsets); `None` on the
/// platform, off a ride, or with transit off.
pub fn aboard_at(world: &World, id: EntityId) -> Option<(f32, f32)> {
    let ride = world.rides.get(&id)?;
    let Some(ExecState::Ride { depart, arrive_tick, .. }) = world.comp::<crate::components::Brain>(id).map(|b| &b.exec)
    else {
        return None;
    };
    if world.tick < *depart || world.tick >= *arrive_tick {
        return None;
    }
    // The island pass phase 6: aboard a train, the train's tile (at the
    // departure tick, before the boarding, the train is at his stop).
    if trains_on(world) {
        let p = match crate::systems::motion::vehicle_of(world, id).and_then(|v| world.motions.get(&v)) {
            Some(m) => m.pos,
            None => world.comp::<Position>(id)?.tile,
        };
        return Some((f32::from(p.x), f32::from(p.y)));
    }
    let net = &world.transit.net;
    let line = net.lines.get(usize::from(ride.line))?;
    let k0 = usize::from(board_pos(world, ride)?);
    if line.stops.get(k0) != Some(&ride.board) {
        return None;
    }
    // The first leg's line from the board stop, in the ride's direction: the
    // stops it passes and their ticks from the board stop (a transfer's
    // second line is drawn as the end of the first: the lerp is cosmetic).
    let seq: Vec<(u16, u32)> = if ride.forward {
        line.stops[k0..].iter().zip(&line.offsets[k0..]).map(|(&s, &o)| (s, o - line.offsets[k0])).collect()
    } else {
        line.stops[..=k0].iter().zip(&line.offsets[..=k0]).rev().map(|(&s, &o)| (s, line.offsets[k0] - o)).collect()
    };
    let span = arrive_tick.saturating_sub(*depart).max(1) as f32;
    let total = seq.last().map_or(1, |&(_, o)| o.max(1)) as f32;
    let at = (world.tick - depart) as f32 / span * total.min(ride.hop_ticks.max(1) as f32);
    let pos = |s: u16| net.stops[usize::from(s)];
    for w in seq.windows(2) {
        let ((a, oa), (b, ob)) = (w[0], w[1]);
        if at <= ob as f32 {
            let f = ((at - oa as f32) / (ob - oa).max(1) as f32).clamp(0.0, 1.0);
            let (pa, pb) = (pos(a), pos(b));
            let lerp = |x: u16, y: u16| f32::from(x) + (f32::from(y) - f32::from(x)) * f;
            return Some((lerp(pa.x, pb.x), lerp(pa.y, pb.y)));
        }
    }
    seq.last().map(|&(s, _)| (f32::from(pos(s).x), f32::from(pos(s).y)))
}

// ---------------------------------------------------------------------------
// The day
// ---------------------------------------------------------------------------

/// Per tick, first thing (review item 8): a `CutLine` runs out at its
/// tick, any time of day, and the table must not wait for the next reader
/// that holds `&mut World` (`try_ride`): `commute_ticks` and the
/// `route` readers take `&World` and read the table as built. While any
/// line is cut, the table is checked against the open lines and rebuilt on
/// a change. Nothing to do (one map test) with no line cut.
pub fn reopen(world: &mut World) {
    if !world.transit.line_closed.is_empty() {
        ensure_net(world);
    }
}
