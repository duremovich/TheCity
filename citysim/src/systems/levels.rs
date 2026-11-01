//! The city milestone C7 (docs/CITY_MILESTONE.md § C7): levels, portals and
//! pathing, the climb's half (the portals' graph is `exec::bigpath::Portals`).
//!
//! - **The climb.** A body entering a building on a storey above the ground
//!   climbs to it from the door, and leaving one climbs down to the door
//!   first: a timed leg inside the structure, [`plan`]. By the stairs,
//!   `[levels] stair_ticks_per_floor` a storey up (`stair_down_mult` of it
//!   down); by the lift past `lift_min_floors` storeys, when the core has one
//!   that is not cut and its wait and ride beat the stairs. A Full body's
//!   climb is an `ExecState::Climb` arm at the door (between the street path
//!   and the `Use`); a Coarse body's is added to its timed walk
//!   (`GotoTimed.climb`), placed by `exec::walk_place` at the door with the
//!   floor interpolated.
//! - **The core.** A building climbs its own lift (`Building.lift`, the
//!   map's `lift=`, the export's lift cores) or else its host's; every
//!   structure has its stairs.
//! - **The lift is a place** (Dylan 2026-10-30, item 5): a lift core keeps
//!   its lobby's calls by the tick ([`LiftState`], saved), and the queue at a
//!   tick is the fluid queue those calls make, served at the cars' rate
//!   (`lift_capacity` a car each cycle, the cycle up to the top and back plus
//!   the dwells): `Q(t) = max_s (calls in (s, t] − rate × (t − s))`. A call
//!   waits `lift_wait_ticks` plus a cycle (over the cars) for every full car
//!   ahead of it, so the evening rush in a 40-storey tower waits, and once
//!   the stairs are quicker the crowd takes them. A call is booked at the
//!   tick it is made (a Coarse body's at its walk's start, for the tick it
//!   reaches the lobby). **C8** (`systems::vertical`): `LiftState.cut` (no
//!   car runs: the stairs, until `cut_until`) and `held_by` (a gang holding
//!   the lobby: its members ride first, anyone else waits more).
//! - **Statistical mirror:** none (a Statistical agent does not walk or
//!   climb; promoted, he stands on his floor, `floors::floor_in`).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::components::Building;
use crate::config::LevelsCfg;
use crate::entity::EntityId;
use crate::map::{MapPortal, PortalKind};
use crate::time::Tick;
use crate::world::World;

/// Calls older than this are dropped from a lobby's book (the queue they
/// made is long served: a cycle is minutes).
const BOOK_TICKS: Tick = 180;

/// A lift core's lobby: its calls by the tick and C8's hooks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LiftState {
    /// `(tick, calls)` in tick order: the bodies that called the car then.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub calls: VecDeque<(Tick, u16)>,
    /// C8's hook: the power cut (a landlord who stops paying, a sabotage):
    /// no car runs, every climb takes the stairs.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut: bool,
    /// C8: the gang holding the lobby (`vertical::lift_holds`): its members
    /// ride first, anyone else waits `[vertical] held_wait_ticks` more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_by: Option<EntityId>,
    /// C8: the cut runs until this tick (`vertical::cut_lift`), then the cars
    /// run again; `None` with `cut`: until someone mends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_until: Option<Tick>,
}

/// A climb planned: its ticks in all, the lift's wait among them (0 by the
/// stairs), and whether it rides the lift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClimbPlan {
    pub ticks: Tick,
    pub wait: Tick,
    pub lift: bool,
}

pub fn on(world: &World) -> bool {
    world.config.levels.is_some()
}

/// The building whose cores `b`'s climb uses: its own lift's, else its host's.
pub fn core_of(world: &World, b: EntityId) -> EntityId {
    match world.comp::<Building>(b) {
        Some(bd) if bd.lift.is_none() => bd.within.unwrap_or(b),
        _ => b,
    }
}

/// A lift's cycle in ticks: up to its top and back plus the two dwells.
fn cycle(c: &LevelsCfg, top: u8) -> f64 {
    (2.0 * f64::from(top) * f64::from(c.lift_ticks_per_floor) + 2.0 * f64::from(c.lift_dwell_ticks)).max(0.1)
}

/// The fluid queue `calls` make at `at` (the calls at `at` itself
/// included), served at `rate` bodies a tick.
fn queue_at(calls: &VecDeque<(Tick, u16)>, at: Tick, rate: f64) -> f64 {
    let mut a = 0.0;
    let mut q: f64 = 0.0;
    for &(t, n) in calls.iter().rev() {
        if t > at {
            continue;
        }
        a += f64::from(n);
        q = q.max(a - rate * (at - t) as f64);
    }
    q.max(0.0)
}

/// The lobby queue of `core`'s lift at `at` (bodies; 0 without a lift).
pub fn queue(world: &World, core: EntityId, at: Tick) -> f64 {
    let (Some(c), Some((cars, top))) =
        (world.config.levels.as_ref(), world.comp::<Building>(core).and_then(|b| b.lift))
    else {
        return 0.0;
    };
    let rate = f64::from(cars.max(1)) * f64::from(c.lift_capacity.max(1)) / cycle(c, top);
    world.lifts.get(&core).map_or(0.0, |s| queue_at(&s.calls, at, rate))
}

/// The climb from storey `from` to `to` by the stairs (C8: the raid's column
/// past the lift's choke). `None`: no climb.
pub fn stairs(world: &World, from: u8, to: u8) -> Option<ClimbPlan> {
    let c = world.config.levels.as_ref()?;
    if from == to {
        return None;
    }
    let rate = f64::from(c.stair_ticks_per_floor) * if to > from { 1.0 } else { f64::from(c.stair_down_mult) };
    let t = f64::from(from.abs_diff(to)) * rate;
    Some(ClimbPlan { ticks: (t.ceil() as Tick).max(1), wait: 0, lift: false })
}

/// The climb from storey `from` to `to` inside `core` for a call at `at` by
/// `who` (pure: nothing booked). `None`: no climb (the same storey,
/// `[levels]` off). C8: a lift a gang holds (`LiftState.held_by`) lets its
/// members ride first and makes anyone else wait `[vertical]
/// held_wait_ticks` more (`vertical::held_wait`).
pub fn plan(world: &World, core: EntityId, from: u8, to: u8, at: Tick, who: Option<EntityId>) -> Option<ClimbPlan> {
    let c = world.config.levels.as_ref()?;
    if from == to {
        return None;
    }
    let up = to > from;
    let floors = f64::from(from.abs_diff(to));
    let stair_rate = f64::from(c.stair_ticks_per_floor) * if up { 1.0 } else { f64::from(c.stair_down_mult) };
    let stairs = floors * stair_rate;
    let round = |t: f64| (t.ceil() as Tick).max(1);
    let by_stairs = ClimbPlan { ticks: round(stairs), wait: 0, lift: false };
    let Some((cars, top)) = world.comp::<Building>(core).and_then(|b| b.lift) else { return Some(by_stairs) };
    if from.abs_diff(to) <= c.lift_min_floors || world.lifts.get(&core).is_some_and(|s| s.cut) {
        return Some(by_stairs);
    }
    // The lift serves the ground to `top`; the storeys past it by the stairs.
    let (lo, hi) = (from.min(to), from.max(to));
    let lifted = f64::from(hi.min(top).saturating_sub(lo));
    if lifted <= f64::from(c.lift_min_floors) {
        return Some(by_stairs);
    }
    let cyc = cycle(c, top);
    let cap = f64::from(c.lift_capacity.max(1));
    let rate = f64::from(cars.max(1)) * cap / cyc;
    let held = crate::systems::vertical::held_wait(world, core, who);
    let ahead =
        if held == Some(0.0) { 0.0 } else { world.lifts.get(&core).map_or(0.0, |s| queue_at(&s.calls, at, rate)) };
    let wait =
        f64::from(c.lift_wait_ticks) + (ahead / cap).floor() * cyc / f64::from(cars.max(1)) + held.unwrap_or(0.0);
    let ride = lifted * f64::from(c.lift_ticks_per_floor) + (floors - lifted) * stair_rate;
    if wait + ride >= stairs {
        return Some(by_stairs);
    }
    Some(ClimbPlan { ticks: round(wait + ride), wait: wait.round() as Tick, lift: true })
}

/// Book a planned climb: a lift's call into its lobby at `at`, and the
/// day's counters (`to` the storey climbed to, for the high floors').
pub fn book(world: &mut World, core: EntityId, p: ClimbPlan, to: u8, at: Tick) {
    let ticks = u32::try_from(p.ticks).unwrap_or(u32::MAX);
    let row = crate::systems::ownership::jobs2_row(world);
    row.climbs += 1;
    row.climb_ticks = row.climb_ticks.saturating_add(ticks);
    if to >= HIGH_FLOOR {
        row.climbs_high += 1;
        row.climb_ticks_high = row.climb_ticks_high.saturating_add(ticks);
    }
    if !p.lift {
        return;
    }
    row.lift_rides += 1;
    row.lift_wait_ticks = row.lift_wait_ticks.saturating_add(u32::try_from(p.wait).unwrap_or(u32::MAX));
    let q = queue(world, core, at);
    let row = crate::systems::ownership::jobs2_row(world);
    row.lift_queue_max = row.lift_queue_max.max(q.round() as u32);
    let now = world.tick;
    let st = world.lifts.entry(core).or_default();
    while st.calls.front().is_some_and(|&(t, _)| t + BOOK_TICKS < now) {
        st.calls.pop_front();
    }
    let i = st.calls.partition_point(|&(t, _)| t <= at);
    match i.checked_sub(1).and_then(|j| st.calls.get_mut(j)) {
        Some((t, n)) if *t == at => *n = n.saturating_add(1),
        _ => st.calls.insert(i, (at, 1)),
    }
}

/// A storey from which a climb counts as a high floor's (the report's).
pub const HIGH_FLOOR: u8 = 20;

/// `plan` then `book` at the world's tick, for `who`.
pub fn climb(world: &mut World, core: EntityId, from: u8, to: u8, who: EntityId) -> Option<ClimbPlan> {
    let at = world.tick;
    let p = plan(world, core, from, to, at, Some(who))?;
    book(world, core, p, to, at);
    Some(p)
}

/// The storey a climb from `from` to `to` has reached at `tick` of a climb
/// departing at `depart`, boarding at `board` (the lift's wait spent at the
/// lower end: the lobby going up, the landing going down) and arriving at
/// `arrive`, as a fraction (the view's).
pub fn storey_at(from: u8, to: u8, depart: Tick, board: Tick, arrive: Tick, tick: Tick) -> f32 {
    let (from_f, to_f) = (f32::from(from), f32::from(to));
    if tick <= board.max(depart) {
        return from_f;
    }
    if tick >= arrive || arrive <= board {
        return to_f;
    }
    let u = (tick - board) as f32 / (arrive - board) as f32;
    from_f + (to_f - from_f) * u
}

/// A portal's walker steps (`exec::bigpath::Portals::build`'s): its run,
/// plus a lift portal's call and ride, or the stairs' (and a landing's down
/// through its lot) `portal_stair_ticks_per_floor` a storey of the ends'
/// levels apart; the escalator's run at `escalator_speed` x the walk (a
/// moving stair walked; it lifts you). Ticks to steps at the pace (`[exec]
/// walk_tiles_per_tick` a `move_ticks_full`). `[levels]` off: the run.
pub fn portal_steps(world: &World, p: &MapPortal) -> u32 {
    let run = u32::from(p.tiles);
    let Some(c) = world.config.levels.as_ref() else { return run };
    let e = &world.config.exec;
    let per_tick = e.walk_tiles_per_tick.max(1) as f64 / e.move_ticks_full.max(1) as f64;
    let storeys = f64::from(p.a.1.abs_diff(p.b.1));
    let ticks = match p.kind {
        PortalKind::Escalator => {
            return ((f64::from(run) / f64::from(c.escalator_speed).max(0.1)).round() as u32).max(1);
        }
        PortalKind::Lift => f64::from(c.portal_lift_wait_ticks) + storeys * f64::from(c.lift_ticks_per_floor),
        _ => storeys * f64::from(c.portal_stair_ticks_per_floor),
    };
    run + (ticks * per_tick).round() as u32
}

/// The run report's `#levels` lines: the lift hosts and lobbies booked, the
/// portal graph, the big-map searches (`bigpath::searched`: per walk) and
/// the landmarks' measuring time.
pub fn report(world: &World) -> Vec<String> {
    let lifts = world
        .with::<Building>()
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.lift.is_some()))
        .count();
    let portals = world.flow_fields.portals();
    let (finds, expanded) = crate::exec::bigpath::searched();
    vec![
        format!(
            "#levels\tlift_hosts\t{lifts}\tlobbies_booked\t{}\tportals\t{}\tdeck_nodes\t{}",
            world.lifts.len(),
            portals.list.len(),
            portals.nodes.len()
        ),
        format!(
            "#bigpath\tfinds\t{finds}\texpanded\t{expanded}\tper_find\t{:.0}\tlandmark_ms\t{:.0}\tfailed\t{}",
            expanded as f64 / finds.max(1) as f64,
            crate::exec::bigpath::landmark_us() as f64 / 1000.0,
            crate::exec::bigpath::failed().0
        ),
    ]
}

/// Build the world's portal graph from its map (at the build and the load).
pub fn init_portals(world: &mut World) {
    let portals = if world.map.portals.is_empty() {
        crate::exec::bigpath::Portals::none()
    } else {
        let w: &World = world;
        crate::exec::bigpath::Portals::build(&w.map, &|p| portal_steps(w, p))
    };
    world.flow_fields.set_portals(portals);
}
