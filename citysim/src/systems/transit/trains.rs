use super::*;

// ---------------------------------------------------------------------------
// The timetable
// ---------------------------------------------------------------------------

fn in_window(tod: u16, from: u16, to: u16) -> bool {
    if from <= to {
        tod >= from && tod < to
    } else {
        tod >= from || tod < to
    }
}

fn tod_of(t: Tick) -> u16 {
    (t % TICKS_PER_DAY) as u16
}

/// Is the El running at `t` (`service_from..service_to`)?
pub fn in_service(world: &World, t: Tick) -> bool {
    let c = &world.config.transit;
    in_window(tod_of(t), c.service_from, c.service_to)
}

/// Is `t` night for the danger test?
pub fn is_night(world: &World, t: Tick) -> bool {
    let c = &world.config.transit;
    in_window(tod_of(t), c.night_from, c.night_to)
}

/// T6: the first departure from `stop` on `line` in a direction at or after
/// `t`, at the line's first call there. `None` when the line does not call
/// there. A ride departs from the call its route boards at
/// ([`departure_at`]).
pub fn next_departure(world: &World, line: u8, forward: bool, stop: u16, t: Tick) -> Option<Tick> {
    let l = world.transit.net.lines.get(usize::from(line))?;
    let k = l.stops.iter().position(|&s| s == stop)?;
    departure_at(world, line, forward, u16::try_from(k).ok()?, t)
}

/// T6, review item 7: the first departure at or after `t` from the line's
/// `pos`-th call (`NetLine::stops[pos]`) in a direction: a loop line calls
/// at a stop more than once, each at its own offset. `None` past the line.
pub fn departure_at(world: &World, line: u8, forward: bool, pos: u16, t: Tick) -> Option<Tick> {
    let l = world.transit.net.lines.get(usize::from(line))?;
    let k = usize::from(pos);
    let off = if forward { *l.offsets.get(k)? } else { l.total - *l.offsets.get(k)? };
    let h = i64::from(world.config.transit.headway.max(1));
    let wait = (i64::from(off) - t as i64).rem_euclid(h);
    Some(t + wait as Tick)
}

/// The board stop's call on the ride's line: the route's (`Ride::board_pos`),
/// else (a save from before it) the line's first call there.
pub(super) fn board_pos(world: &World, ride: &Ride) -> Option<u16> {
    ride.board_pos.or_else(|| {
        let l = world.transit.net.lines.get(usize::from(ride.line))?;
        u16::try_from(l.stops.iter().position(|&s| s == ride.board)?).ok()
    })
}

// ---------------------------------------------------------------------------
// Trains (the island pass phase 6, ISLAND_PASS I3, OPEN-165)
// ---------------------------------------------------------------------------
//
// The timetable above, run by trains. A line's departures leave each
// terminus every `headway` ticks (slot `s` at tick `s × headway`); a train
// that leaves the first terminus at slot `s` reaches the far one at
// `total` ticks and leaves it again at slot `s + m`, `m` = `total` over the
// headway rounded up, so `2m` trains run a line and train `j` takes the
// forward slots `≡ j` and the backward slots `≡ j + m` (mod `2m`). A train's
// place is a pure function of the tick (`train_at`): travelling a stretch
// between two calls at the line's speed (`seg`), or dwelling at a call
// (`dwell_ticks` before each departure), or laid over at a terminus. Trains
// run around the clock (the service hours gate boardings, not trains).
//
// Each train is a persistent `Motion` (`RideKind::Train`, `Route::Line`)
// spawned once (`ensure_trains`) and never despawned; `motion::advance`
// moves it and its riders. A rider on the platform waits for his
// departure (`TransitState::boarding`), boards the train that makes it
// (`board_trains`; a full one leaves him for the next), rides aboard (his
// position is the train's), and at his leg's end alights at the stop: the
// last leg walks on, a transfer walks `transfer_ticks` across the platform
// and waits for the next line's train. A plan dropped between two stops
// keeps him aboard to the next stop (`motion::alight`, `held`). A cut line
// stops its trains where they are; their riders walk the track to the
// nearest stop (`evacuate`) and walk on from there.
//
// The ride's ticks are the timetable's, as before: the wait for the
// departure, the ride, now the transfer's real wait. `route` schedules
// every leg exactly (`schedule`), and the table's transfer edge counts half
// a headway of wait, so `commute_ticks` (the structural estimate) is the
// ride's expected value. The Statistical mirror is the timed ride itself
// (`stat_commute`): no body, the same fares and boardings.

/// `[motion.trains]` on (with `[motion]` and the El).
pub fn trains_on(world: &World) -> bool {
    world.config.transit.enabled && world.config.motion.as_ref().is_some_and(|m| m.trains.is_some())
}

/// A line's `m`: its one-way trip in headways, rounded up (at least 1).
fn line_m(world: &World, line: &NetLine) -> u64 {
    let h = u64::from(world.config.transit.headway.max(1));
    u64::from(line.total).div_ceil(h).max(1)
}

/// The ticks of one leg (its calls' offsets apart).
pub(super) fn leg_ticks(world: &World, leg: TrainLeg) -> Tick {
    let Some(l) = world.transit.net.lines.get(usize::from(leg.line)) else { return 0 };
    let (a, b) = (usize::from(leg.from), usize::from(leg.to));
    match (l.offsets.get(a), l.offsets.get(b)) {
        (Some(&x), Some(&y)) => Tick::from(x.abs_diff(y)),
        _ => 0,
    }
}

/// The legs of a ride from stop `s` to stop `e` over the open lines: the
/// table's first leg (to its `off` call), then the table's ride from there,
/// a line each. `None` with no ride or a path the table does not close.
pub fn legs_of(world: &World, s: u16, e: u16) -> Option<Vec<TrainLeg>> {
    let net = &world.transit.net;
    let mut out = Vec::new();
    let mut at = s;
    for _ in 0..8 {
        let hop = net.hop(at, e);
        if hop.ticks == UNREACHABLE {
            return None;
        }
        out.push(TrainLeg { line: hop.line, forward: hop.forward, from: hop.pos, to: hop.off });
        let stop = *net.lines.get(usize::from(hop.line))?.stops.get(usize::from(hop.off))?;
        if stop == e {
            return Some(out);
        }
        if stop == at {
            return None;
        }
        at = stop;
    }
    None
}

/// The exact departure of the first leg and the arrival of the last for a
/// rider on the first platform at `t`: each leg's next departure, its ride,
/// the `transfer_ticks` walk to the next platform. `None` past a line.
pub fn schedule(world: &World, legs: &[TrainLeg], t: Tick) -> Option<(Tick, Tick)> {
    let transfer = Tick::from(world.config.transit.transfer_ticks);
    let (mut at, mut first, mut arrive) = (t, None, t);
    for &leg in legs {
        let dep = departure_at(world, leg.line, leg.forward, leg.from, at)?;
        first.get_or_insert(dep);
        arrive = dep + leg_ticks(world, leg);
        at = arrive + transfer;
    }
    Some((first?, arrive))
}

/// Where a train is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TrainAt {
    /// At the line's `k`-th call (dwelling, departing or laid over).
    Stop(u16),
    /// Between call `seg` and `seg + 1`, `done` tiles along `NetLine::seg[seg]`
    /// from call `seg`.
    Track { seg: u16, done: u64 },
}

/// Train `j` of line `line` at `tick` (the timetable's pure function).
pub fn train_at(world: &World, line: u8, j: u64, tick: Tick) -> Option<TrainAt> {
    let l = world.transit.net.lines.get(usize::from(line))?;
    let last = l.stops.len().checked_sub(1)?;
    let h = u64::from(world.config.transit.headway.max(1));
    let m = line_m(world, l);
    let cycle = 2 * m * h;
    let phase = (i128::from(tick) - i128::from(j * h)).rem_euclid(i128::from(cycle)) as u64;
    let (forward, e) = if phase < m * h { (true, phase) } else { (false, phase - m * h) };
    let total = u64::from(l.total);
    if e >= total {
        return Some(TrainAt::Stop(if forward { last as u16 } else { 0 }));
    }
    let off = |k: usize| u64::from(l.offsets[k]);
    let dwell = world.config.transit.dwell_ticks.round().max(0.0) as u64;
    let len = |k: usize| l.seg.get(k).map_or(0, |w| crate::systems::motion::way_len(w));
    // The stretch's progress: `x` of `span` ticks done from call `k`.
    let track = |k: usize, x: u64, span: u64| TrainAt::Track { seg: k as u16, done: len(k) * x / span.max(1) };
    if forward {
        // Departed call k at off(k); at k + 1 from off(k + 1) - dwell.
        let k = l.offsets.partition_point(|&o| u64::from(o) <= e) - 1;
        if e == off(k) || k == last {
            return Some(TrainAt::Stop(k as u16));
        }
        let arr = off(k + 1).saturating_sub(dwell).max(off(k) + 1);
        if e >= arr {
            return Some(TrainAt::Stop((k + 1) as u16));
        }
        Some(track(k, e - off(k), arr - off(k)))
    } else {
        // The forward clock run back: departed call i at e' = off(i), at
        // i - 1 from e' = off(i - 1) + dwell.
        let r = total - e;
        let i = l.offsets.partition_point(|&o| u64::from(o) < r);
        if i > last || off(i) == r || i == 0 {
            return Some(TrainAt::Stop(i.min(last) as u16));
        }
        let arr = (off(i - 1) + dwell).min(off(i) - 1);
        if r <= arr {
            return Some(TrainAt::Stop((i - 1) as u16));
        }
        Some(track(i - 1, r - arr, off(i) - arr))
    }
}

/// The tile of a train's place.
pub fn train_tile(world: &World, line: u8, at: TrainAt) -> Option<TilePos> {
    let l = world.transit.net.lines.get(usize::from(line))?;
    match at {
        TrainAt::Stop(k) => l.stops.get(usize::from(k)).map(|&s| world.transit.net.stops[usize::from(s)]),
        TrainAt::Track { seg, done } => l.seg.get(usize::from(seg)).map(|w| crate::systems::motion::point_at(w, done)),
    }
}

/// The train that makes the departure from the line's `pos`-th call at
/// `dep` in a direction (a departure `departure_at` gives).
pub fn train_for(world: &World, line: u8, forward: bool, pos: u16, dep: Tick) -> Option<EntityId> {
    let l = world.transit.net.lines.get(usize::from(line))?;
    let k = usize::from(pos);
    let off = Tick::from(if forward { *l.offsets.get(k)? } else { l.total - *l.offsets.get(k)? });
    let h = u64::from(world.config.transit.headway.max(1));
    let slot = dep.checked_sub(off)? / h;
    let m = line_m(world, l);
    let j = if forward { slot % (2 * m) } else { (slot + m) % (2 * m) };
    world.transit.trains.get(usize::from(line))?.get(j as usize).copied()
}

/// Spawn every line's trains once (`[motion.trains]` on, the network built):
/// a persistent `Motion` each, at its place now.
pub fn ensure_trains(world: &mut World) {
    if !trains_on(world) || !world.transit.trains.is_empty() {
        return;
    }
    ensure_net(world);
    let lines = world.transit.net.lines.len();
    if lines == 0 {
        return;
    }
    let mut all = Vec::with_capacity(lines);
    for l in 0..lines {
        let n = 2 * line_m(world, &world.transit.net.lines[l]);
        let lu = u8::try_from(l).unwrap_or(u8::MAX);
        let mut ids = Vec::with_capacity(n as usize);
        for j in 0..n {
            let v = world.spawn();
            let pos = train_at(world, lu, j, world.tick).and_then(|at| train_tile(world, lu, at)).unwrap_or_default();
            crate::systems::motion::add_train(world, v, lu, j as u16, pos);
            ids.push(v);
        }
        all.push(ids);
    }
    world.transit.trains = all;
}

/// Is `line` cut now?
pub fn line_cut(world: &World, line: u8) -> bool {
    world.transit.line_closed.get(&line).is_some_and(|&until| until > world.tick)
}

/// Note a train event in the leg log (measurement only): 0 a rider left on
/// a full train's platform, 1 a dropped plan carried to the next stop, 2 a
/// rider set down by a cut.
pub fn note_train(world: &mut World, what: usize) {
    if let Some(l) = world.leg_log.as_mut() {
        l.trains[what] += 1;
    }
}

/// Every tick, before the vehicles advance (`exec::run`): the trains exist,
/// the riders due on a platform board, the cut lines' riders walk the
/// track.
pub fn trains_tick(world: &mut World) {
    if !trains_on(world) {
        return;
    }
    ensure_trains(world);
    let now = world.tick;
    while let Some(&(t, id)) = world.transit.boarding.first() {
        if t > now {
            break;
        }
        world.transit.boarding.pop_first();
        board_train(world, id, t);
    }
    if !world.transit.evac.is_empty() {
        let walking: Vec<(EntityId, TilePos)> = world
            .transit
            .evac
            .iter()
            .map(|(&id, e)| (id, crate::systems::motion::timed_pos(&e.way, e.depart, e.arrive, now)))
            .collect();
        for (id, at) in walking {
            let ok = world.is_alive(id)
                && matches!(world.comp::<crate::components::Brain>(id).map(|b| &b.exec), Some(ExecState::Ride { .. }))
                && world.comp::<Position>(id).is_some_and(|p| p.building.is_none());
            if !ok {
                world.transit.evac.remove(&id);
                continue;
            }
            if let Some(p) = world.comp_mut::<Position>(id) {
                p.tile = at;
            }
        }
    }
}

/// `id` on the platform for the departure at `t`: aboard the train that
/// makes it, or (full) on to the next; a cut line's rider walks on.
fn board_train(world: &mut World, id: EntityId, t: Tick) {
    let Some(mut ride) = world.rides.get(&id).cloned() else { return };
    if plan_key(world, id) != Some((ride.plan_started, ride.plan_step)) {
        return;
    }
    let Some(lod) = world.comp::<crate::components::Brain>(id).map(|b| b.lod) else { return };
    let Some(ExecState::Ride { target, depart, arrive_tick }) =
        world.comp::<crate::components::Brain>(id).map(|b| b.exec.clone())
    else {
        return;
    };
    if depart != t || crate::systems::motion::vehicle_of(world, id).is_some() {
        return;
    }
    let Some(&leg) = ride.legs.get(usize::from(ride.leg)) else { return };
    let train = if line_cut(world, leg.line) { None } else { train_for(world, leg.line, leg.forward, leg.from, t) };
    let Some(v) = train else {
        abandon(world, id, &ride, lod);
        return;
    };
    let cap = world.config.motion.as_ref().and_then(|m| m.trains.as_ref()).map_or(u32::MAX, |c| c.capacity);
    if world.motions.get(&v).is_some_and(|m| m.riders.len() >= cap as usize) {
        let h = Tick::from(world.config.transit.headway.max(1));
        ride.depart = t + h;
        ride.arrive += h;
        world.rides.insert(id, ride);
        if let Some(b) = world.comp_mut::<crate::components::Brain>(id) {
            b.exec = ExecState::Ride { target, depart: t + h, arrive_tick: arrive_tick + h };
        }
        world.transit.boarding.insert((t + h, id));
        note_train(world, 0);
        return;
    }
    crate::systems::motion::board_train(world, id, v);
}

/// A transfer: off the train at the leg's last call, `transfer_ticks` across
/// the platform, the next leg's departure; a cut line's rider walks on.
pub(super) fn transfer(world: &mut World, id: EntityId, mut ride: Ride, lod: Lod) {
    crate::systems::motion::alight_now(world, id);
    let next = usize::from(ride.leg) + 1;
    let Some(&leg) = ride.legs.get(next) else {
        abandon(world, id, &ride, lod);
        return;
    };
    let net = &world.transit.net;
    let Some(stop) = net.lines.get(usize::from(leg.line)).and_then(|l| l.stops.get(usize::from(leg.from))).copied()
    else {
        abandon(world, id, &ride, lod);
        return;
    };
    // S1 (`OPEN-309`): across to the next train's platform.
    stand_at_stop(world, id, stop);
    let ready = world.tick + Tick::from(world.config.transit.transfer_ticks);
    let depart = departure_at(world, leg.line, leg.forward, leg.from, ready).filter(|_| !line_cut(world, leg.line));
    let Some(depart) = depart else {
        abandon(world, id, &ride, lod);
        return;
    };
    ride.leg = next as u8;
    ride.line = leg.line;
    ride.forward = leg.forward;
    ride.board = stop;
    ride.board_pos = Some(leg.from);
    ride.depart = depart;
    ride.arrive = schedule(world, &ride.legs[next..], depart).map_or(depart, |x| x.1);
    let state = ExecState::Ride { target: ride.target.clone(), depart, arrive_tick: depart + leg_ticks(world, leg) };
    world.rides.insert(id, ride);
    if let Some(b) = world.comp_mut::<crate::components::Brain>(id) {
        b.exec = state;
    }
    world.transit.boarding.insert((depart, id));
}

/// A cut line (`cut_line`): its trains stand where they are; each rider
/// aboard walks the track to the nearest stop (ties: the one ahead), his
/// ride's leg now that walk (`leg_done` walks on from the stop).
fn evacuate(world: &mut World, line: u8) {
    if !trains_on(world) {
        return;
    }
    let Some(trains) = world.transit.trains.get(usize::from(line)).cloned() else { return };
    let now = world.tick;
    for (j, v) in trains.into_iter().enumerate() {
        let Some(riders) = world.motions.get(&v).map(|m| m.riders.clone()) else { continue };
        if riders.is_empty() {
            continue;
        }
        let Some(at) = train_at(world, line, j as u64, now) else { continue };
        let l = &world.transit.net.lines[usize::from(line)];
        // The way to the nearest call: a stop's tile, or along the stretch.
        let way: Vec<TilePos> = match at {
            TrainAt::Stop(k) => vec![world.transit.net.stops[usize::from(l.stops[usize::from(k)])]],
            TrainAt::Track { seg, done } => {
                let w = &l.seg[usize::from(seg)];
                let len = crate::systems::motion::way_len(w);
                let forward = train_forward(world, line, j as u64, now);
                let to_end = match (len - done.min(len)).cmp(&done) {
                    std::cmp::Ordering::Less => true,
                    std::cmp::Ordering::Greater => false,
                    std::cmp::Ordering::Equal => forward,
                };
                crate::systems::motion::split_way(w, done, to_end)
            }
        };
        let tiles = crate::systems::motion::way_len(&way);
        for r in riders {
            if world.transit.held.contains(&r) {
                crate::systems::motion::alight_now(world, r);
                continue;
            }
            crate::systems::motion::alight_now(world, r);
            let Some(target) = world.rides.get(&r).map(|x| x.target.clone()) else { continue };
            let arrive = now + world.config.exec.walk_ticks(tiles).max(1);
            if let Some(b) = world.comp_mut::<crate::components::Brain>(r) {
                b.exec = ExecState::Ride { target, depart: now, arrive_tick: arrive };
            }
            world.transit.evac.insert(r, Evac { way: way.clone(), depart: now, arrive });
            note_train(world, 2);
        }
    }
}

/// Is train `j` of `line` on its forward run at `tick`?
fn train_forward(world: &World, line: u8, j: u64, tick: Tick) -> bool {
    let Some(l) = world.transit.net.lines.get(usize::from(line)) else { return true };
    let h = u64::from(world.config.transit.headway.max(1));
    let m = line_m(world, l);
    let phase = (i128::from(tick) - i128::from(j * h)).rem_euclid(i128::from(2 * m * h)) as u64;
    phase < m * h
}

// ---------------------------------------------------------------------------
// Closures (T15)
// ---------------------------------------------------------------------------

/// Is `stop` open at `t`: not closed by god, no riot live in its district,
/// no curfew there at night.
pub fn stop_open(world: &World, stop: u16, t: Tick) -> bool {
    if world.transit.stop_closed.get(&stop).is_some_and(|&until| until > world.tick) {
        return false;
    }
    let Some(&tile) = world.transit.net.stops.get(usize::from(stop)) else { return false };
    let d = world.district_of(tile);
    if crate::systems::riot::riot_in(world, d) {
        return false;
    }
    !(world.district(d).curfew && is_night(world, t))
}

/// God `CutLine`: close line `line` for `days` (0 reopens it).
pub fn cut_line(world: &mut World, line: u8, days: u32) -> Result<String, String> {
    let Some(name) = world.config.transit.lines.get(usize::from(line)).map(|l| l.name.clone()) else {
        return Err(format!("CutLine: no line {line}"));
    };
    if days == 0 {
        world.transit.line_closed.remove(&line);
        ensure_net(world);
        return Ok(format!("reopened the {name}"));
    }
    let until = world.tick + u64::from(days) * TICKS_PER_DAY;
    world.transit.line_closed.insert(line, until);
    // Phase 2: the table now, so the structural commute (`commute_ticks`,
    // read without `&mut`) sees the cut at once.
    ensure_net(world);
    // The island pass phase 6: the line's trains stop; their riders walk
    // the track to the nearest stop.
    evacuate(world, line);
    Ok(format!("cut the {name} for {days} days"))
}

/// God `CloseStop`: close stop `stop` for `days` (0 reopens it).
pub fn close_stop(world: &mut World, stop: u16, days: u32) -> Result<String, String> {
    ensure_net(world);
    let Some(&tile) = world.transit.net.stops.get(usize::from(stop)) else {
        return Err(format!("CloseStop: no stop {stop}"));
    };
    if days == 0 {
        world.transit.stop_closed.remove(&stop);
        return Ok(format!("reopened the stop at ({}, {})", tile.x, tile.y));
    }
    let until = world.tick + u64::from(days) * TICKS_PER_DAY;
    world.transit.stop_closed.insert(stop, until);
    Ok(format!("closed the stop at ({}, {}) for {days} days", tile.x, tile.y))
}
