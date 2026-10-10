//! Transit: the El (docs/TRANSIT.md, plan `~/.claude/plans/transit.md`
//! phase 1). City-owned elevated lines over the road grid (Dylan,
//! 2026-10-10): fares go to the Treasury, power is paid from it to the
//! World.
//!
//! A line is a timetable, not a train: departures leave each terminus every
//! `headway` ticks, and a stop's departure is the terminus's plus the ride
//! ticks to it. A ride is a lookup. The stops, the lines and the all-pairs
//! ride table are derived from the config and the map ([`build`], at world
//! creation and on load) and never saved.
//!
//! One pure function decides walk or ride ([`choose`]: no RNG). A Full or
//! Coarse `GoTo` that rides runs three legs inside the one plan step: the
//! walk to the board stop, [`ExecState::Ride`] (the platform until the
//! departure, then aboard), and the walk on from the alight stop.
//! `World::rides` holds the itinerary between the legs. The Statistical
//! tier does not ride in phase 1.
//!
//! Nothing here runs per agent per tick: the choice runs at a `GoTo`'s
//! start, the legs at their ends, the power and the day's counters at
//! midnight. With `[transit] enabled = false` nothing in this module
//! touches the world.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use serde::{Deserialize, Serialize};

use crate::components::{Crime, Lod, Personality, Position, TileKind, TilePos, Wallet};
use crate::entity::EntityId;
use crate::exec::{ExecState, GotoTarget};
use crate::goap::LocationKey;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// Tiles a nominal stop may move to find a walkable Road tile.
const SNAP_RADIUS: i32 = 8;
/// No route.
const UNREACHABLE: u32 = u32::MAX;

/// `[transit] enabled`.
pub fn enabled(world: &World) -> bool {
    world.config.transit.enabled
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// One line of the derived network: its stops in track order and each
/// stop's ride ticks from the first terminus.
#[derive(Clone, Debug, Default)]
pub struct NetLine {
    pub name: String,
    pub stops: Vec<u16>,
    pub offsets: Vec<u32>,
    pub total: u32,
}

/// The fastest ride between two stops over the open lines: its ticks and
/// the first leg's line and direction (the departure the rider waits for).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hop {
    pub ticks: u32,
    pub line: u8,
    pub forward: bool,
}

const NO_HOP: Hop = Hop { ticks: UNREACHABLE, line: 0, forward: true };

/// The derived network (not saved).
#[derive(Clone, Debug, Default)]
pub struct Net {
    pub stops: Vec<TilePos>,
    pub lines: Vec<NetLine>,
    /// The lines open when `table` was built.
    pub open: Vec<bool>,
    /// `table[s × n + t]`.
    pub table: Vec<Hop>,
}

impl Net {
    pub fn hop(&self, s: u16, t: u16) -> Hop {
        let n = self.stops.len();
        self.table.get(usize::from(s) * n + usize::from(t)).copied().unwrap_or(NO_HOP)
    }
}

/// `World::transit`: the derived network and the saved closures and day
/// counters. Not written into a save while idle (everything empty), so a
/// world with transit off saves as it did before transit existed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TransitState {
    #[serde(skip)]
    pub net: Net,
    /// God `CutLine`: a line's index -> closed until this tick.
    pub line_closed: BTreeMap<u8, Tick>,
    /// God `CloseStop`: a stop's index -> closed until this tick.
    pub stop_closed: BTreeMap<u16, Tick>,
    /// Today's boardings per stop and hour (T17's ledger).
    pub load: BTreeMap<u16, [u16; 24]>,
    /// Agents whose commute in progress rode (for `commute_tpt_ride`).
    pub rode: BTreeSet<EntityId>,
}

impl TransitState {
    pub fn is_idle(&self) -> bool {
        self.line_closed.is_empty() && self.stop_closed.is_empty() && self.load.is_empty() && self.rode.is_empty()
    }
}

/// A ride in progress (`World::rides`): the itinerary between the legs.
/// `plan_started`/`plan_step` name the plan step it belongs to, so a ride
/// left behind by a replaced plan is dropped, never resumed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ride {
    /// The `GoTo`'s own destination (the third leg's).
    pub target: GotoTarget,
    pub board: u16,
    pub alight: u16,
    pub line: u8,
    pub forward: bool,
    pub hop_ticks: u32,
    /// The planned departure and arrival at the alight stop.
    pub depart: Tick,
    pub arrive: Tick,
    /// Jumps the gate (decided with the ride, re-checked at the platform).
    pub dodge: bool,
    pub plan_started: Tick,
    pub plan_step: u8,
}

/// Why a trip walks (the refusal columns).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Under `min_ride_tiles` (not counted).
    Short,
    /// No open stop pair connects, or no service at the departure.
    NoRoute,
    /// Can't pay and won't jump the gate.
    Fare,
    /// The ride doesn't save enough time for its fare.
    Slow,
    /// A night stop in a district too dangerous for this rider.
    Danger,
}

/// A chosen ride (`choose`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub board: u16,
    pub alight: u16,
    pub hop: Hop,
    pub depart: Tick,
    pub arrive: Tick,
    /// Door to door, from the trip's start.
    pub total: Tick,
    pub walk: Tick,
    pub dodge: bool,
}

// ---------------------------------------------------------------------------
// The network
// ---------------------------------------------------------------------------

/// The tiles of a polyline, waypoint to waypoint (an L when a pair is not
/// axis-aligned: x first), the first waypoint included.
fn track(waypoints: &[[u8; 2]]) -> Vec<TilePos> {
    let mut out = Vec::new();
    let Some(&[x0, y0]) = waypoints.first() else { return out };
    let (mut x, mut y) = (i32::from(x0), i32::from(y0));
    out.push(TilePos::new(x0, y0));
    for &[bx, by] in &waypoints[1..] {
        let (bx, by) = (i32::from(bx), i32::from(by));
        while x != bx {
            x += (bx - x).signum();
            out.push(TilePos::new(x as u8, y as u8));
        }
        while y != by {
            y += (by - y).signum();
            out.push(TilePos::new(x as u8, y as u8));
        }
    }
    out
}

/// The nearest walkable Road tile to `p` within `SNAP_RADIUS` (Manhattan),
/// ties the lower `(y, x)`; `p` itself when it is one.
fn snap(world: &World, p: TilePos) -> Option<TilePos> {
    let (px, py) = (i32::from(p.x), i32::from(p.y));
    for r in 0..=SNAP_RADIUS {
        let mut best: Option<TilePos> = None;
        for dy in -r..=r {
            let rest = r - dy.abs();
            for dx in [-rest, rest] {
                let (x, y) = (px + dx, py + dy);
                if !world.map.in_bounds(x, y) {
                    continue;
                }
                let t = TilePos::new(x as u8, y as u8);
                if world.map.tile_at(t) != TileKind::Road {
                    continue;
                }
                if best.is_none_or(|b| (t.y, t.x) < (b.y, b.x)) {
                    best = Some(t);
                }
                if rest == 0 {
                    break;
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}

/// Derive the stops and lines from `[transit]` and the map (T3, T5), then
/// the table over the open lines. A no-op with transit off.
pub fn build(world: &mut World) {
    world.transit.net = Net::default();
    if !enabled(world) {
        return;
    }
    let cfg = world.config.transit.clone();
    let tracks: Vec<Vec<TilePos>> = cfg.lines.iter().map(|l| track(&l.waypoints)).collect();
    let mut on_lines: BTreeMap<TilePos, BTreeSet<usize>> = BTreeMap::new();
    for (l, t) in tracks.iter().enumerate() {
        for &p in t {
            on_lines.entry(p).or_default().insert(l);
        }
    }
    let spacing = cfg.stop_spacing.max(1) as usize;
    let mut index: BTreeMap<TilePos, u16> = BTreeMap::new();
    let mut stops: Vec<TilePos> = Vec::new();
    let mut lines = Vec::new();
    for (l, (row, t)) in cfg.lines.iter().zip(&tracks).enumerate() {
        let waypoints: BTreeSet<TilePos> = row.waypoints.iter().map(|&[x, y]| TilePos::new(x, y)).collect();
        let mut line = NetLine { name: row.name.clone(), ..NetLine::default() };
        let mut last_pos: Option<usize> = None;
        for (i, &p) in t.iter().enumerate() {
            let shared = on_lines.get(&p).is_some_and(|s| s.len() > 1 || !s.contains(&l));
            let is_stop = i % spacing == 0 || i + 1 == t.len() || waypoints.contains(&p) || shared;
            if !is_stop {
                continue;
            }
            let Some(at) = snap(world, p) else { continue };
            let s = *index.entry(at).or_insert_with(|| {
                stops.push(at);
                u16::try_from(stops.len() - 1).unwrap_or(u16::MAX)
            });
            if line.stops.last() == Some(&s) {
                continue;
            }
            let off = match (last_pos, line.offsets.last()) {
                (Some(j), Some(&prev)) => {
                    let tiles = (i - j) as f32;
                    prev + (tiles * cfg.ride_ticks_per_tile).ceil() as u32 + cfg.dwell_ticks
                }
                _ => 0,
            };
            line.stops.push(s);
            line.offsets.push(off);
            last_pos = Some(i);
        }
        line.total = line.offsets.last().copied().unwrap_or(0);
        lines.push(line);
    }
    world.transit.net.stops = stops;
    world.transit.net.lines = lines;
    let open = open_lines(world);
    rebuild_table(world, open);
}

/// The lines open now (god `CutLine` is the only closure of a whole line).
fn open_lines(world: &World) -> Vec<bool> {
    let tick = world.tick;
    (0..world.transit.net.lines.len())
        .map(|l| {
            u8::try_from(l).ok().is_none_or(|l| world.transit.line_closed.get(&l).is_none_or(|&until| until <= tick))
        })
        .collect()
}

/// The all-pairs table over the open lines: Dijkstra from every stop over
/// `(line, position)` nodes, ride edges along a line, `transfer_ticks`
/// between two lines' nodes at one stop. Ties keep the first path found
/// (the heap orders by `(ticks, node)`: deterministic).
fn rebuild_table(world: &mut World, open: Vec<bool>) {
    let transfer = world.config.transit.transfer_ticks;
    let net = &world.transit.net;
    let n = net.stops.len();
    // Nodes: (line, position) of the open lines.
    let mut nodes: Vec<(usize, usize)> = Vec::new();
    let mut at_stop: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut node_of: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (l, line) in net.lines.iter().enumerate() {
        if !open.get(l).copied().unwrap_or(false) {
            continue;
        }
        for (k, &s) in line.stops.iter().enumerate() {
            node_of.insert((l, k), nodes.len());
            at_stop[usize::from(s)].push(nodes.len());
            nodes.push((l, k));
        }
    }
    let mut table = vec![NO_HOP; n * n];
    for src in 0..n {
        let mut dist: Vec<(u32, Option<(u8, bool)>)> = vec![(UNREACHABLE, None); nodes.len()];
        let mut heap = BinaryHeap::new();
        for &v in &at_stop[src] {
            dist[v] = (0, None);
            heap.push(std::cmp::Reverse((0u32, v)));
        }
        while let Some(std::cmp::Reverse((d, v))) = heap.pop() {
            if d > dist[v].0 {
                continue;
            }
            let first = dist[v].1;
            let (l, k) = nodes[v];
            let line = &net.lines[l];
            let mut relax = |to: usize, cost: u32, first: Option<(u8, bool)>, heap: &mut BinaryHeap<_>| {
                let nd = d.saturating_add(cost);
                if nd < dist[to].0 {
                    dist[to] = (nd, first);
                    heap.push(std::cmp::Reverse((nd, to)));
                }
            };
            let lu = u8::try_from(l).unwrap_or(u8::MAX);
            if k + 1 < line.stops.len() {
                let to = node_of[&(l, k + 1)];
                relax(to, line.offsets[k + 1] - line.offsets[k], first.or(Some((lu, true))), &mut heap);
            }
            if k > 0 {
                let to = node_of[&(l, k - 1)];
                relax(to, line.offsets[k] - line.offsets[k - 1], first.or(Some((lu, false))), &mut heap);
            }
            // A transfer only once riding (at the source it changes nothing).
            if first.is_some() {
                let s = usize::from(line.stops[k]);
                for &w in &at_stop[s] {
                    if w != v {
                        relax(w, transfer, first, &mut heap);
                    }
                }
            }
        }
        for t in 0..n {
            if t == src {
                continue;
            }
            let best = at_stop[t].iter().filter_map(|&v| dist[v].1.map(|f| (dist[v].0, f))).min_by_key(|&(d, _)| d);
            if let Some((ticks, (line, forward))) = best {
                table[src * n + t] = Hop { ticks, line, forward };
            }
        }
    }
    world.transit.net.table = table;
    world.transit.net.open = open;
}

/// Build the network if it is missing, and rebuild the table when a line
/// opened or closed since it was built.
pub fn ensure_net(world: &mut World) {
    if !enabled(world) {
        return;
    }
    if world.transit.net.stops.is_empty() && !world.config.transit.lines.is_empty() {
        build(world);
        return;
    }
    let open = open_lines(world);
    if open != world.transit.net.open {
        rebuild_table(world, open);
    }
}

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
/// `t`. `None` when the line does not call there.
pub fn next_departure(world: &World, line: u8, forward: bool, stop: u16, t: Tick) -> Option<Tick> {
    let l = world.transit.net.lines.get(usize::from(line))?;
    let k = l.stops.iter().position(|&s| s == stop)?;
    let off = if forward { l.offsets[k] } else { l.total - l.offsets[k] };
    let h = i64::from(world.config.transit.headway.max(1));
    let wait = (i64::from(off) - t as i64).rem_euclid(h);
    Some(t + wait as Tick)
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
        return Ok(format!("reopened the {name}"));
    }
    let until = world.tick + u64::from(days) * TICKS_PER_DAY;
    world.transit.line_closed.insert(line, until);
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

// ---------------------------------------------------------------------------
// The route and the choice (T7, T8)
// ---------------------------------------------------------------------------

/// The `stop_candidates` open stops nearest `tile` at `t`, by Manhattan,
/// ties the lower index.
fn candidates(world: &World, tile: TilePos, t: Tick) -> Vec<u16> {
    let k = world.config.transit.stop_candidates.max(1);
    let mut v: Vec<(u32, u16)> = world
        .transit
        .net
        .stops
        .iter()
        .enumerate()
        .filter_map(|(i, &s)| u16::try_from(i).ok().map(|i| (s.manhattan(tile), i)))
        .filter(|&(_, i)| stop_open(world, i, t))
        .collect();
    v.sort_unstable();
    v.truncate(k);
    v.into_iter().map(|(_, i)| i).collect()
}

/// T7: the fastest ride from `from` to `to` for `who` leaving at `t`, door to
/// door: walk to a candidate board stop, the exact next departure, the
/// table's ride, the walk from the alight stop. `None` without one.
pub fn route(world: &World, who: EntityId, from: TilePos, to: TilePos, t: Tick) -> Option<Route> {
    let board = candidates(world, from, t);
    let alight = candidates(world, to, t);
    let net = &world.transit.net;
    let mut best: Option<Route> = None;
    for &s in &board {
        let w1 = crate::exec::timed_walk(world, who, from, net.stops[usize::from(s)]);
        for &e in &alight {
            if s == e {
                continue;
            }
            let hop = net.hop(s, e);
            if hop.ticks == UNREACHABLE {
                continue;
            }
            let Some(depart) = next_departure(world, hop.line, hop.forward, s, t + w1) else { continue };
            if !in_service(world, depart) {
                continue;
            }
            let arrive = depart + Tick::from(hop.ticks);
            let w2 = crate::exec::timed_walk(world, who, net.stops[usize::from(e)], to);
            let total = arrive + w2 - t;
            if best.is_none_or(|b| total < b.total) {
                best = Some(Route { board: s, alight: e, hop, depart, arrive, total, walk: 0, dodge: false });
            }
        }
    }
    best
}

/// The coins a meal costs (the poor test).
fn meal_price(world: &World) -> i64 {
    world.mean_price().max(1)
}

/// T8: walk or ride, for `who` from `from` to `to` leaving at `t`. Pure.
pub fn choose(world: &World, who: EntityId, from: TilePos, to: TilePos, t: Tick) -> Result<Route, Refusal> {
    let c = &world.config.transit;
    if from.manhattan(to) < c.min_ride_tiles {
        return Err(Refusal::Short);
    }
    let mut r = route(world, who, from, to, t).ok_or(Refusal::NoRoute)?;
    let coins = world.comp::<Wallet>(who).map_or(0, |w| w.coins);
    let (law, courage) = world.comp::<Personality>(who).map_or((0.5, 0.5), |p| (p.lawfulness, p.courage));
    // T10: pay, or jump the gate.
    let dodge = law < c.dodge_always || (coins < c.fare && law < c.dodge_lawfulness);
    if !dodge && coins < c.fare {
        return Err(Refusal::Fare);
    }
    let fare = if dodge { 0 } else { c.fare };
    // Time vs fare: a coin is worth more to the poor.
    let poor = coins < c.poor_meals.saturating_mul(meal_price(world));
    let per_coin = if poor { c.poor_ticks_per_coin } else { c.ticks_per_coin };
    let walk = crate::exec::timed_walk(world, who, from, to);
    let need = u64::from(c.min_saving_ticks) + fare.max(0) as u64 * u64::from(per_coin);
    if walk < r.total || walk - r.total < need {
        return Err(Refusal::Slow);
    }
    // Danger: a night stop in a district past the rider's nerve.
    if is_night(world, r.depart) || is_night(world, r.arrive) {
        let net = &world.transit.net;
        let crime = [r.board, r.alight]
            .iter()
            .map(|&s| world.district(world.district_of(net.stops[usize::from(s)])).crime_rate)
            .fold(0.0f32, f32::max);
        if crime > c.danger_crime * (0.5 + courage) {
            return Err(Refusal::Danger);
        }
    }
    r.walk = walk;
    r.dodge = dodge;
    Ok(r)
}

// ---------------------------------------------------------------------------
// The legs (T12, T13)
// ---------------------------------------------------------------------------

fn count_refusal(world: &mut World, r: Refusal) {
    let c = &mut world.stats.current.transit;
    match r {
        Refusal::Short => {}
        Refusal::NoRoute => c.refused_no_route += 1,
        Refusal::Fare => c.refused_fare += 1,
        Refusal::Slow => c.refused_slow += 1,
        Refusal::Danger => c.refused_danger += 1,
    }
}

/// The plan step a ride belongs to: the plan's start tick and step index.
fn plan_key(world: &World, id: EntityId) -> Option<(Tick, u8)> {
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
    let access = GotoTarget { dest: LocationKey::Street, tile: board_tile, building: None };
    let state = match lod {
        Lod::Full => crate::exec::walking_goto(world, id, access)?,
        _ => crate::exec::timed_goto(world, id, access),
    };
    let ride = Ride {
        target: target.clone(),
        board: route.board,
        alight: route.alight,
        line: route.hop.line,
        forward: route.hop.forward,
        hop_ticks: route.hop.ticks,
        depart: route.depart,
        arrive: route.arrive,
        dodge: route.dodge,
        plan_started,
        plan_step,
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
    if aboard {
        alight(world, id, &ride, lod);
    } else {
        board(world, id, ride, lod);
    }
    true
}

/// Give a ride up where the agent stands and walk on.
fn abandon(world: &mut World, id: EntityId, ride: &Ride, lod: Lod) {
    world.rides.remove(&id);
    world.stats.current.transit.ride_abandoned += 1;
    let state = walk_leg(world, id, ride.target.clone(), lod);
    if let Some(b) = world.comp_mut::<crate::components::Brain>(id) {
        b.exec = state;
    }
}

/// Jump the gate: a FareEvasion crime at the stop, to the witnesses and the
/// law as any crime (Dylan, 2026-10-10: a petty record from day one).
fn dodge(world: &mut World, id: EntityId, tile: TilePos) {
    world.stats.current.transit.fare_dodges += 1;
    crate::systems::law::raise_crime(world, id, None, Crime::FareEvasion, tile);
}

/// On the platform: the departure (the planned one, or the next if the walk
/// ran late), the fare or the gate jumped, the boarding counted.
fn board(world: &mut World, id: EntityId, mut ride: Ride, lod: Lod) {
    let now = world.tick;
    let depart = if now <= ride.depart {
        Some(ride.depart)
    } else {
        next_departure(world, ride.line, ride.forward, ride.board, now)
    };
    let Some(depart) = depart.filter(|&d| in_service(world, d) && stop_open(world, ride.board, now)) else {
        abandon(world, id, &ride, lod);
        return;
    };
    let tile = world.transit.net.stops[usize::from(ride.board)];
    let fare = world.config.transit.fare;
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    if ride.dodge || (coins < fare && law < world.config.transit.dodge_lawfulness) {
        dodge(world, id, tile);
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
    let hour = ((depart % TICKS_PER_DAY) / TICKS_PER_HOUR) as usize;
    let slot = world.transit.load.entry(ride.board).or_insert([0; 24]);
    slot[hour] = slot[hour].saturating_add(1);
    let peak = u32::from(slot[hour]);
    let c = &mut world.stats.current.transit;
    c.peak_stop_load = c.peak_stop_load.max(peak);
    ride.depart = depart;
    ride.arrive = depart + Tick::from(ride.hop_ticks);
    let state = ExecState::Ride { target: ride.target.clone(), depart, arrive_tick: ride.arrive };
    world.rides.insert(id, ride);
    if let Some(b) = world.comp_mut::<crate::components::Brain>(id) {
        b.exec = state;
    }
}

/// Off at the alight stop: stand on it and walk on to the `GoTo`'s target.
fn alight(world: &mut World, id: EntityId, ride: &Ride, lod: Lod) {
    world.rides.remove(&id);
    let tile = world.transit.net.stops[usize::from(ride.alight)];
    if let Some(p) = world.comp_mut::<Position>(id) {
        p.tile = tile;
        p.building = None;
    }
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
}

// ---------------------------------------------------------------------------
// The day
// ---------------------------------------------------------------------------

/// Per tick: at midnight, the closures that ran out, the power, the day's
/// load ledger. A no-op with transit off.
pub fn run(world: &mut World) {
    if !enabled(world) || world.tick_of_day() != 0 || world.tick == 0 {
        return;
    }
    ensure_net(world);
    let tick = world.tick;
    world.transit.line_closed.retain(|_, &mut until| until > tick);
    world.transit.stop_closed.retain(|_, &mut until| until > tick);
    let open = open_lines(world).iter().filter(|&&o| o).count() as i64;
    let power = open * world.config.transit.power_per_line_day;
    if power > 0 {
        let paid = crate::systems::wages::inputs(world, None, power);
        world.stats.current.transit.transit_power += paid;
    }
    world.transit.load.clear();
    let starts = &world.commute_start;
    world.transit.rode.retain(|a| starts.contains_key(a));
}

/// The network as text (stops, lines, a sample of rides): the
/// `transit-report` subcommand's body.
pub fn describe(world: &mut World) -> String {
    ensure_net(world);
    let net = &world.transit.net;
    let mut out = format!("{} stops, {} lines\n", net.stops.len(), net.lines.len());
    for (l, line) in net.lines.iter().enumerate() {
        let tiles: Vec<String> = line
            .stops
            .iter()
            .zip(&line.offsets)
            .map(|(&s, &o)| {
                let t = net.stops[usize::from(s)];
                format!("#{s}({},{})+{o}", t.x, t.y)
            })
            .collect();
        out.push_str(&format!(
            "line {l} {}: {} stops, {} ticks end to end\n  {}\n",
            line.name,
            line.stops.len(),
            line.total,
            tiles.join(" ")
        ));
    }
    out
}
