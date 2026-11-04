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

mod choose;
mod net;
mod ride;
mod trains;

pub use choose::*;
pub use net::*;
pub use ride::*;
pub use trains::*;

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
    /// The island pass phase 6 (trains): the way from each call to the next
    /// (`seg[k]`: `stops[k]` to `stops[k + 1]`), the stop tiles at its ends
    /// and the track's corners between (derived with the line).
    pub seg: Vec<Vec<TilePos>>,
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
    /// The island pass phase 6: the first leg's last call (its index in the
    /// line's `stops`): the alight stop, or the first transfer's.
    pub off: u16,
}

const NO_HOP: Hop = Hop { ticks: UNREACHABLE, line: 0, forward: true, pos: 0, off: 0 };

/// The derived network (not saved).
#[derive(Clone, Debug, Default)]
pub struct Net {
    pub stops: Vec<TilePos>,
    pub lines: Vec<NetLine>,
    /// The lines open when `table` was built.
    pub open: Vec<bool>,
    /// `table[s × n + t]`.
    pub table: Vec<Hop>,
    /// Phase 2, Scale S3a: per `NEAR_CELL` × `NEAR_CELL` cell of the map
    /// (row-major), the stops that can be among the `NEAR` nearest of some
    /// tile in it, ascending: `near_cands[near_cells[c]..near_cells[c + 1]]`.
    /// A tile's shortlist is ranked from its cell's few candidates
    /// (`Net::near`), not scanned over every stop. Phase 2 kept every tile's
    /// shortlist (`w × h × 8` u16: 90 MB at 3,000 × 1,875, built in O(tiles
    /// × stops) on every load); the cells are ~1/256 of the tiles and built
    /// in O(cells × stops). Derived with the stops.
    near_cands: Vec<u16>,
    near_cells: Vec<u32>,
    cells_w: usize,
    map_w: usize,
    map_h: usize,
}

/// Stops kept per tile in `Net::near`: `stop_candidates` plus room for a
/// few closed ones before `candidates` falls back to the scan.
pub const NEAR: usize = 8;

/// The side of a `Net::near` candidate cell, in tiles.
const NEAR_CELL: usize = 16;

/// A tile's shortlist from `Net::near`: up to `NEAR` stop indices, nearest
/// first (a slice by `Deref`).
#[derive(Clone, Copy, Debug, Default)]
pub struct NearStops {
    ids: [u16; NEAR],
    len: u8,
}

impl std::ops::Deref for NearStops {
    type Target = [u16];
    fn deref(&self) -> &[u16] {
        &self.ids[..usize::from(self.len)]
    }
}

impl Net {
    pub fn hop(&self, s: u16, t: u16) -> Hop {
        let n = self.stops.len();
        self.table.get(usize::from(s) * n + usize::from(t)).copied().unwrap_or(NO_HOP)
    }

    /// `tile`'s nearest stops (up to `NEAR`, nearest first, ties the lower
    /// index: the order of a scan sorted by `(Manhattan, index)`); empty off
    /// the map or before `build`.
    pub fn near(&self, tile: TilePos) -> NearStops {
        let mut out = NearStops::default();
        let (x, y) = (usize::from(tile.x), usize::from(tile.y));
        if x >= self.map_w || y >= self.map_h || self.near_cells.is_empty() {
            return out;
        }
        let c = (y / NEAR_CELL) * self.cells_w + x / NEAR_CELL;
        let (a, b) = (self.near_cells[c] as usize, self.near_cells[c + 1] as usize);
        let k = NEAR.min(self.stops.len());
        let mut best = [(u32::MAX, u16::MAX); NEAR];
        let mut n = 0usize;
        for &s in &self.near_cands[a..b] {
            let key = (self.stops[usize::from(s)].manhattan(tile), s);
            if n == k && key >= best[k - 1] {
                continue;
            }
            // Insert in order, dropping the k-th when full.
            let mut j = n.min(k - 1);
            while j > 0 && best[j - 1] > key {
                best[j] = best[j - 1];
                j -= 1;
            }
            best[j] = key;
            n = (n + 1).min(k);
        }
        for (slot, &(_, s)) in out.ids.iter_mut().zip(&best[..n]) {
            *slot = s;
        }
        out.len = n as u8;
        out
    }

    /// The shortlist index's bytes (the memory test's read).
    pub fn near_bytes(&self) -> usize {
        self.near_cands.len() * 2 + self.near_cells.len() * 4
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
    /// The island pass phase 6 (§ Trains): each line's trains, by slot
    /// (`train_for`); spawned once (`ensure_trains`), never despawned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trains: Vec<Vec<EntityId>>,
    /// Riders on a platform, by the departure they board: (tick, rider).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub boarding: BTreeSet<(Tick, EntityId)>,
    /// Riders whose plan ended between two stops: aboard to the next stop,
    /// off there (`motion::alight`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub held: BTreeSet<EntityId>,
    /// Riders set down by a cut line, walking the track to the nearest stop.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub evac: BTreeMap<EntityId, Evac>,
}

impl TransitState {
    pub fn is_idle(&self) -> bool {
        self.line_closed.is_empty()
            && self.stop_closed.is_empty()
            && self.load.is_empty()
            && self.rode.is_empty()
            && self.staff == 0
            && self.trains.is_empty()
            && self.boarding.is_empty()
            && self.held.is_empty()
            && self.evac.is_empty()
    }
}

/// The island pass phase 6: one leg of a train ride, on one line in one
/// direction, from the `from`-th call of the line to the `to`-th.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainLeg {
    pub line: u8,
    pub forward: bool,
    pub from: u16,
    pub to: u16,
}

/// A rider walking the track of a cut line to its nearest stop: along
/// `way` (its end the stop) over `depart..=arrive`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evac {
    pub way: Vec<TilePos>,
    pub depart: Tick,
    pub arrive: Tick,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
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
    /// The island pass phase 6 (trains): the ride's legs, a line each, in
    /// order; empty without trains. `line`, `forward`, `board` and
    /// `board_pos` name the leg in progress, `depart` its departure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legs: Vec<TrainLeg>,
    /// The leg in progress (an index into `legs`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub leg: u8,
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
/// a day's power, over a Sanitation post's wage (`treasury::sweeper_wage`),
/// capped at `[transit] staff_per_line` a line open. Uncapped, the fares
/// paid ~250 crews on seed 42 (~14 % of jobs, a jobs program hiding the
/// Treasury's hoard); the fares past the power and the crews' wages are the
/// Treasury's income like any other, counted in the civic receipts
/// (`net_fares`) and returned by the surplus credit (`treasury::daily`).
/// Station posts (clerks, platform guards) wait for station buildings (the
/// Interiors milestone, decision 5).
pub fn staff_target(world: &World) -> u16 {
    let days = world.config.treasury.receipts_days.max(1) as usize;
    let rows: Vec<i64> = world.stats.history.iter().rev().take(days).map(|r| r.transit.fares).collect();
    if rows.is_empty() {
        return 0;
    }
    let mean = rows.iter().sum::<i64>() / rows.len() as i64;
    let open = open_lines(world).iter().filter(|&&o| o).count() as i64;
    let net = mean - open * world.config.transit.power_per_line_day;
    let paid = (net.max(0)) / crate::systems::treasury::sweeper_wage(world);
    let cap = open * i64::from(world.config.transit.staff_per_line);
    u16::try_from(paid.min(cap)).unwrap_or(u16::MAX)
}

/// Review finding 2: a day's fares left after the El paid its way: the
/// day's power (`transit_power`) and its crews' wages (`el_staff` x
/// `treasury::sweeper_wage`, the post `staff_target` funds), at least 0.
/// The civic receipts count this, not the gross (the gross paid the crews
/// and then the police and sweepers again).
pub fn net_fares(world: &World, row: &crate::stats::DayRow) -> i64 {
    let t = &row.transit;
    let crews = i64::from(t.el_staff) * crate::systems::treasury::sweeper_wage(world);
    (t.fares - t.transit_power - crews).max(0)
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
