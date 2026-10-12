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
/// the first leg's line, direction and board position on that line (the
/// departure the rider waits for). Review item 7: a line that calls at a
/// stop twice (a loop) departs from each call at its own offset, and the
/// table's path starts from one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hop {
    pub ticks: u32,
    pub line: u8,
    pub forward: bool,
    /// The board stop's index in the line's `stops` the path starts from.
    pub pos: u16,
}

const NO_HOP: Hop = Hop { ticks: UNREACHABLE, line: 0, forward: true, pos: 0 };

/// The derived network (not saved).
#[derive(Clone, Debug, Default)]
pub struct Net {
    pub stops: Vec<TilePos>,
    pub lines: Vec<NetLine>,
    /// The lines open when `table` was built.
    pub open: Vec<bool>,
    /// `table[s × n + t]`.
    pub table: Vec<Hop>,
    /// Phase 2: each map tile's `NEAR` nearest stops by `(Manhattan, index)`
    /// (row-major, `u16::MAX` padding), so a stop shortlist is a slice read,
    /// not a scan (`candidates`, `commute_ticks`). Derived with the stops.
    pub near: Vec<u16>,
    pub map_w: usize,
}

/// Stops kept per tile in `Net::near`: `stop_candidates` plus room for a
/// few closed ones before `candidates` falls back to the scan.
pub const NEAR: usize = 8;

impl Net {
    pub fn hop(&self, s: u16, t: u16) -> Hop {
        let n = self.stops.len();
        self.table.get(usize::from(s) * n + usize::from(t)).copied().unwrap_or(NO_HOP)
    }

    /// `tile`'s nearest stops (up to `NEAR`, nearest first, ties the lower
    /// index); empty off the map or before `build`.
    pub fn near(&self, tile: TilePos) -> &[u16] {
        let i = (usize::from(tile.y) * self.map_w + usize::from(tile.x)) * NEAR;
        match self.near.get(i..i + NEAR) {
            Some(v) => &v[..v.iter().position(|&s| s == u16::MAX).unwrap_or(NEAR)],
            None => &[],
        }
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
    /// Phase 3, the money circuit: the El's maintenance crews, Sanitation
    /// posts on the city's payroll funded from the line's fares
    /// (`staff_target`); `districts::reconcile_sanitation` hires them on
    /// top of the civic headcount.
    pub staff: u16,
}

impl TransitState {
    pub fn is_idle(&self) -> bool {
        self.line_closed.is_empty()
            && self.stop_closed.is_empty()
            && self.load.is_empty()
            && self.rode.is_empty()
            && self.staff == 0
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
    /// Review item 7: the board stop's position on the line (`Hop::pos`);
    /// `None` in a save from before it (the line's first call there).
    #[serde(default)]
    pub board_pos: Option<u16>,
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
    world.transit.net.near = near_table(world, &stops);
    world.transit.net.map_w = world.map.w();
    world.transit.net.lines = lines;
    world.transit.net.stops = stops;
    let open = open_lines(world);
    rebuild_table(world, open);
}

/// Every tile's `NEAR` nearest stops by `(Manhattan, index)`.
fn near_table(world: &World, stops: &[TilePos]) -> Vec<u16> {
    let (w, h) = (world.map.w(), world.map.h());
    let mut out = vec![u16::MAX; w * h * NEAR];
    let k = NEAR.min(stops.len());
    if k == 0 {
        return out;
    }
    let mut v: Vec<(u32, u16)> = Vec::with_capacity(stops.len());
    for y in 0..h {
        for x in 0..w {
            let t = TilePos::new(x as u8, y as u8);
            v.clear();
            v.extend(stops.iter().enumerate().map(|(i, s)| (s.manhattan(t), u16::try_from(i).unwrap_or(u16::MAX))));
            if k < v.len() {
                v.select_nth_unstable(k - 1);
            }
            v[..k].sort_unstable();
            let base = (y * w + x) * NEAR;
            for (j, &(_, i)) in v[..k].iter().enumerate() {
                out[base + j] = i;
            }
        }
    }
    out
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

/// A path's first leg in `rebuild_table`: line, direction, board position.
type FirstLeg = (u8, bool, u16);

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
        let mut dist: Vec<(u32, Option<FirstLeg>)> = vec![(UNREACHABLE, None); nodes.len()];
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
            let mut relax = |to: usize, cost: u32, first: Option<FirstLeg>, heap: &mut BinaryHeap<_>| {
                let nd = d.saturating_add(cost);
                if nd < dist[to].0 {
                    dist[to] = (nd, first);
                    heap.push(std::cmp::Reverse((nd, to)));
                }
            };
            let lu = u8::try_from(l).unwrap_or(u8::MAX);
            let ku = u16::try_from(k).unwrap_or(u16::MAX);
            if k + 1 < line.stops.len() {
                let to = node_of[&(l, k + 1)];
                relax(to, line.offsets[k + 1] - line.offsets[k], first.or(Some((lu, true, ku))), &mut heap);
            }
            if k > 0 {
                let to = node_of[&(l, k - 1)];
                relax(to, line.offsets[k] - line.offsets[k - 1], first.or(Some((lu, false, ku))), &mut heap);
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
            if let Some((ticks, (line, forward, pos))) = best {
                table[src * n + t] = Hop { ticks, line, forward, pos };
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
fn board_pos(world: &World, ride: &Ride) -> Option<u16> {
    ride.board_pos.or_else(|| {
        let l = world.transit.net.lines.get(usize::from(ride.line))?;
        u16::try_from(l.stops.iter().position(|&s| s == ride.board)?).ok()
    })
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
    // Phase 2: the tile's precomputed shortlist, in the scan's order; the
    // scan below only when closures leave the shortlist short.
    let net = &world.transit.net;
    let near = net.near(tile);
    let open: Vec<u16> = near.iter().copied().filter(|&i| stop_open(world, i, t)).take(k).collect();
    if open.len() == k || near.len() == net.stops.len() {
        return open;
    }
    let mut v: Vec<(u32, u16)> = world
        .transit
        .net
        .stops
        .iter()
        .enumerate()
        .filter_map(|(i, &s)| u16::try_from(i).ok().map(|i| (s.manhattan(tile), i)))
        .collect();
    v.sort_unstable();
    // The open test (a district lookup, the riots) only for the nearest.
    v.into_iter().filter(|&(_, i)| stop_open(world, i, t)).take(k).map(|(_, i)| i).collect()
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
            let Some(depart) = departure_at(world, hop.line, hop.forward, hop.pos, t + w1) else { continue };
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

/// T10: how `who` would board now: `Some(false)` pays the fare,
/// `Some(true)` jumps the gate, `None` can't pay and won't jump it. Phase 3:
/// fresh from the cells, a rider with the fare pays it. Pure.
pub fn pays_or_dodges(world: &World, who: EntityId) -> Option<bool> {
    let c = &world.config.transit;
    let coins = world.comp::<Wallet>(who).map_or(0, |w| w.coins);
    let law = world.comp::<Personality>(who).map_or(0.5, |p| p.lawfulness);
    let since = world.tick.saturating_sub(u64::from(c.dodge_deterred_days) * TICKS_PER_DAY);
    let deterred = coins >= c.fare
        && world.comp::<crate::components::Memory>(who).is_some_and(|m| {
            m.entries.iter().any(|e| e.kind == crate::components::MemoryKind::WasArrested && e.tick >= since)
        });
    let dodge = !deterred && (law < c.dodge_always || (coins < c.fare && law < c.dodge_lawfulness));
    (dodge || coins >= c.fare).then_some(dodge)
}

/// T8: walk or ride, for `who` from `from` to `to` leaving at `t`. Pure.
pub fn choose(world: &World, who: EntityId, from: TilePos, to: TilePos, t: Tick) -> Result<Route, Refusal> {
    let c = &world.config.transit;
    if from.manhattan(to) < c.min_ride_tiles {
        return Err(Refusal::Short);
    }
    let mut r = route(world, who, from, to, t).ok_or(Refusal::NoRoute)?;
    let coins = world.comp::<Wallet>(who).map_or(0, |w| w.coins);
    let courage = world.comp::<Personality>(who).map_or(0.5, |p| p.courage);
    let dodge = pays_or_dodges(world, who).ok_or(Refusal::Fare)?;
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
// The shared distance and the estimate (phase 2: T16, plan § 6)
// ---------------------------------------------------------------------------

/// The one measure of a commute between two doors, in travel ticks: the
/// walk (Manhattan × `[exec] move_ticks_full`, `life::commute_cost`'s
/// formula), or with the El on the best ride over the doors' nearest stops
/// (`stop_candidates` each, the walks at the same rate, half a `headway`
/// of wait, the table's ride) when it saves `min_saving_ticks` plus a
/// fare's `ticks_per_coin`, as `choose` asks of a rider with the coins.
/// No agent, no clock: the structural commute the hiring band, relocation
/// and the housing demand read. God closures count (a closed stop is
/// skipped); a riot's or a curfew's do not (they pass). With transit off it
/// is the walk exactly.
pub fn commute_ticks(world: &World, from: TilePos, to: TilePos) -> Tick {
    let step = world.config.exec.move_ticks_full;
    let tiles = from.manhattan(to);
    let walk = Tick::from(tiles) * step;
    let c = &world.config.transit;
    if !c.enabled || tiles < c.min_ride_tiles {
        return walk;
    }
    let net = &world.transit.net;
    let k = c.stop_candidates.max(1);
    let shut = |s: u16| world.transit.stop_closed.get(&s).is_some_and(|&until| until > world.tick);
    let mut best = UNREACHABLE as Tick;
    for &s in net.near(from).iter().filter(|&&s| !shut(s)).take(k) {
        let w1 = Tick::from(net.stops[usize::from(s)].manhattan(from)) * step;
        for &e in net.near(to).iter().filter(|&&e| !shut(e)).take(k) {
            if s == e {
                continue;
            }
            let hop = net.hop(s, e);
            if hop.ticks == UNREACHABLE {
                continue;
            }
            let w2 = Tick::from(net.stops[usize::from(e)].manhattan(to)) * step;
            best = best.min(w1 + Tick::from(c.headway / 2) + Tick::from(hop.ticks) + w2);
        }
    }
    let need = Tick::from(c.min_saving_ticks) + c.fare.max(0) as Tick * Tick::from(c.ticks_per_coin);
    if best.saturating_add(need) <= walk {
        best
    } else {
        walk
    }
}

/// T14 (phase 3, plan D9): the commute band's upper bounds in ticks: `< 30`,
/// `30-60`, `60-120`, `120-180`, `>= 180` (bands 0-4).
pub const BANDS: [u64; 4] = [30, 60, 120, 180];

/// T14: `id`'s commute band, from its Home door to its work door by
/// `life::commute_cost` (the walk, or the ride where it wins: the same
/// measure on both tiers, read by `calibrate` and `lod::stat_work`). `None`
/// homeless or jobless.
pub fn commute_band(world: &World, id: EntityId) -> Option<usize> {
    let job = world.comp::<crate::components::Job>(id)?;
    let work = world.comp::<crate::components::Building>(job.employer?)?.door;
    let home = crate::systems::contracts::home_door(world, id)?;
    let ticks = crate::systems::life::commute_cost(world, home, work);
    Some(BANDS.iter().position(|&b| ticks < b).unwrap_or(BANDS.len()))
}

/// T16: how long `who` takes from `from` to `to` setting out at `t`: the
/// ride `choose` would take (door to door, its exact wait), else the walk
/// (`exec::timed_walk`). With transit off, or under `min_ride_tiles`, the
/// walk exactly (no route is computed).
pub fn estimate_from(world: &World, who: EntityId, from: TilePos, to: TilePos, t: Tick) -> Tick {
    let walk = crate::exec::timed_walk(world, who, from, to);
    if !may_ride(world, who, from, to) {
        return walk;
    }
    choose(world, who, from, to, t).map_or(walk, |r| r.total.min(walk))
}

/// Could this trip ride at all: the El on and built, the trip past
/// `min_ride_tiles`, no vehicle to hand (a driver never asks, T8).
fn may_ride(world: &World, who: EntityId, from: TilePos, to: TilePos) -> bool {
    enabled(world)
        && from.manhattan(to) >= world.config.transit.min_ride_tiles
        && !world.transit.net.stops.is_empty()
        && !(world.config.assets.enabled && crate::systems::vehicles::vehicle_for_trip(world, who).is_some())
}

/// T16, the departure gate's estimate: the trip to arrive at `to` by
/// `arrive_by`, as `choose` would make it setting out at the time that
/// arrival needs (the structural ride's leaving time, then once more from
/// the leaving time the first answer gives). It reads the deadline, not the
/// clock, so it holds still while a body waits to leave: an estimate read
/// at "now" fell when the El's service began at 05:00 and turned riders
/// back from commutes their walk's gate had opened (the behaviour tier's
/// commute aborts). The walk exactly where `estimate_from` is.
pub fn estimate_by(world: &World, who: EntityId, from: TilePos, to: TilePos, arrive_by: Tick) -> Tick {
    let walk = crate::exec::timed_walk(world, who, from, to);
    if !may_ride(world, who, from, to) {
        return walk;
    }
    let ride = |t: Tick| choose(world, who, from, to, t).map_or(walk, |r| r.total.min(walk));
    let first = ride(arrive_by.saturating_sub(commute_ticks(world, from, to).min(walk)));
    ride(arrive_by.saturating_sub(first))
}

/// T16, the trip gate shared by the departure gate
/// (`routine::must_leave_for_work`, `lead` = its margin) and the shift lead
/// before an outing (`leisure::off_hours`, `lead` = `shift_lead_ticks`):
/// is it time to set out for `door`, `until` ticks off, to arrive `lead`
/// ahead of it? The trip from where `who` stands as it would be made to
/// arrive by then (`estimate_by`: the deadline's trip, not the clock's, so
/// the gate holds still while the body waits). The estimate is never above
/// the walk, so the route is only computed once the walk's gate is open
/// (the departure gate runs every tick for a body resting before its shift).
pub fn trip_due(world: &World, who: EntityId, door: TilePos, until: Tick, lead: Tick) -> bool {
    let from = crate::exec::walk_origin(world, who).unwrap_or(door);
    let walk = crate::exec::timed_walk(world, who, from, door);
    if until > walk + lead {
        return false;
    }
    let arrive_by = world.tick + until - lead.min(until);
    until <= estimate_by(world, who, from, door, arrive_by) + lead
}

/// T16: `estimate_from` where `who` stands (outside its building's door),
/// setting out now: the departure gate's and every "how long to get there"
/// reader's (`fixes::walk_estimate`).
pub fn estimate(world: &World, who: EntityId, to: TilePos) -> Tick {
    let from = crate::exec::walk_origin(world, who).unwrap_or(to);
    estimate_from(world, who, from, to, world.tick)
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
        board_pos: Some(route.hop.pos),
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
        board_pos(world, &ride).and_then(|k| departure_at(world, ride.line, ride.forward, k, now))
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
    count_boarding(world, ride.board, depart);
    ride.depart = depart;
    ride.arrive = depart + Tick::from(ride.hop_ticks);
    let state = ExecState::Ride { target: ride.target.clone(), depart, arrive_tick: ride.arrive };
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
    if caught {
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
    if crate::systems::econ::hash_unit(world.seed(), 0xfa2e_0004, u64::from(id.index), hour) < p {
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
            let lerp = |x: u8, y: u8| f32::from(x) + (f32::from(y) - f32::from(x)) * f;
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

/// Per tick: at midnight, the closures that ran out, the power, the day's
/// load ledger. A no-op with transit off.
pub fn run(world: &mut World) {
    if !enabled(world) {
        return;
    }
    // Phase 3: a ride whose plan step was replaced without an abort (a goal
    // interrupt mid-walk writes a new plan) is dropped now, not at a next
    // leg that may never come (core_sanity's stale-ride check, seed 43).
    if !world.rides.is_empty() {
        let stale: Vec<EntityId> = world
            .rides
            .iter()
            .filter(|(&id, r)| plan_key(world, id) != Some((r.plan_started, r.plan_step)))
            .map(|(&id, _)| id)
            .collect();
        for id in stale {
            world.rides.remove(&id);
        }
    }
    if world.tick_of_day() != 0 || world.tick == 0 {
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
    // The operator spends its fares in the city: the crews it can pay.
    let want = staff_target(world);
    world.transit.staff = want.max(world.transit.staff.saturating_sub(u16::from(world.config.treasury.civic_pace)));
    world.stats.current.transit.el_staff = u32::from(world.transit.staff);
}

/// Phase 3, the money circuit (Dylan 2026-10-10: the city owns the El; fares
/// to the Treasury, power from it): the maintenance crews the line's fares
/// pay for: the mean fares of the last `[treasury] receipts_days` days less
/// a day's power, over a Sanitation post's wage (`treasury::sweeper_wage`).
/// Parked in the Treasury, whose posts are capped (police at its ceiling,
/// Sanitation at `sanitation_max`) and whose tax already sits at its floor,
/// the fares left the city's purses 15-17 % lower (seeds 42-47); spent as
/// wages they go back round. Station posts (clerks, platform guards) wait
/// for station buildings (the Interiors milestone, decision 5).
pub fn staff_target(world: &World) -> u16 {
    let days = world.config.treasury.receipts_days.max(1) as usize;
    let rows: Vec<i64> = world.stats.history.iter().rev().take(days).map(|r| r.transit.fares).collect();
    if rows.is_empty() {
        return 0;
    }
    let mean = rows.iter().sum::<i64>() / rows.len() as i64;
    let open = open_lines(world).iter().filter(|&&o| o).count() as i64;
    let net = mean - open * world.config.transit.power_per_line_day;
    u16::try_from((net.max(0)) / crate::systems::treasury::sweeper_wage(world)).unwrap_or(u16::MAX)
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
