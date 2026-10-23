//! The island pass phase 5 (docs/ISLAND_PASS.md I3; OPEN-165): a vehicle
//! in motion is an entity with its own position, and its riders are aboard.
//!
//! A vehicle leg (a private or fleet car, an exec's flyer, the cab, the
//! exec's car) is a [`Motion`] in `World::motions`, keyed by the vehicle:
//! the `Asset` entity when there is one (the car, the flyer), else an entity
//! spawned for the ride and despawned when its last rider is off (the
//! Garage's cab and the exec's car post no asset; `OPEN-149`: at 20k no cab
//! driver is a body, so the cab is driven off screen). Each rider's aboard
//! link is `World::aboard` (rider -> vehicle) and the vehicle's `riders`.
//!
//! - **Position.** `Motion::pos` is where the vehicle is. A [`Route::Timed`]
//!   vehicle advances along its way (the corners of the road path from the
//!   walk origin to the target's door, or a straight line for a flyer) over
//!   the leg's own ticks, `depart..=arrive`: the same arrival the timed leg
//!   had, so the leg's duration (and the Statistical mirror) is unchanged.
//!   A [`Route::Driven`] vehicle is steered by a Full driver's walk (his
//!   `Goto` at the vehicle's step); its position is his tile after his step.
//!   [`advance`] (the end of `exec::run`) moves every vehicle and writes
//!   each rider's `Position` from it: no body is left on its boarding tile,
//!   none is moved into a building it has not reached.
//! - **Alighting.** On arrival ([`alight`] at the timed leg's arrival tick,
//!   `vehicles::end_trip` for a drive) the rider steps off at the way's end,
//!   the target's door, then walks in. **An interrupt** (a dropped plan: an
//!   urgent Eat, the plan timeout, a demotion, an arrest) steps him off
//!   where the vehicle stands now, and the new plan walks from there (a car
//!   pulls over and parks at the nearest door: `end_trip`). A rider never
//!   acts aboard. [`advance`] drops any rider whose exec is no longer a leg
//!   (a plan replaced without an abort) at the vehicle's tile.
//! - **LOD.** Full and Coarse bodies ride. A Coarse rider promoted to Full
//!   stays aboard (his timed leg runs on); a Full driver demoted to Coarse
//!   turns his drive into a timed one from where he is ([`to_timed`]); a
//!   demotion to Statistical drops the plan, so he steps off where the
//!   vehicle is (then the off-screen snap). The Statistical tier holds no
//!   leg, so no promotion lands mid-ride.
//! - **Statistical mirror:** the timed abstraction itself. Off screen there
//!   is no body to carry; a ride's ticks are the timed leg's, which the
//!   vehicle's arrival keeps, so the parity rule reads the same durations.
//!
//! `CITYSIM_VEHICLES_MOVE_OFF=1` (`[motion]` absent): nothing here runs and
//! the legs are the timed ones (the body on its boarding tile, then at the
//! door). Phase 6 (the El) and 7 (the van) ride the same model: a train is
//! a [`Route::Timed`] vehicle with many riders on a timetable's way, a van
//! one with the guard as driver and the prisoner aboard.

use serde::{Deserialize, Serialize};

use crate::components::{AssetKind, Brain, Position, TilePos};
use crate::entity::EntityId;
use crate::exec::ExecState;
use crate::time::Tick;
use crate::world::World;

/// What kind of ride a vehicle is on (the view's sprite, the device's
/// split). Phase 6 adds the train, phase 7 the van.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RideKind {
    /// A private or fleet road vehicle, its owner or keeper at the wheel.
    Car,
    /// A flyer (an exec's, or a private one).
    Flyer,
    /// The cab: the Garage's, driven by a body or off screen.
    Cab,
    /// An exec's car to his Hq or home (`streetwork::exec_ride`).
    Exec,
    /// The island pass phase 6: an El train (`Route::Line`, persistent).
    Train,
    /// The island pass phase 7 (OPEN-166): the law's van. An escort's (the
    /// guard at the wheel, his cuffed suspect aboard) or a transfer's (no
    /// body at the wheel; the Station's prisoners aboard, `World::van_runs`;
    /// kept until its arrival, empty too).
    Van,
    /// The island pass phase 7: an escort on foot, timed (a Coarse guard
    /// leading his cuffed suspect along the road at the walk's pace). No
    /// vehicle: the view draws the two bodies walking.
    Foot,
}

/// How a vehicle advances.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    /// Steered by a Full driver's walk: the position is his tile.
    Driven,
    /// Along `way` (its corners; `way[0]` the start, the last the target's
    /// door) at a steady pace, at `way[0]` until `depart` and at the end
    /// from `arrive`.
    Timed { way: Vec<TilePos>, depart: Tick, arrive: Tick },
    /// The island pass phase 6: train `train` of El line `line`, at the
    /// timetable's place (`transit::train_at`; a cut line's train stands).
    /// Never despawned.
    Line { line: u8, train: u16 },
}

/// A vehicle in motion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Motion {
    pub ride: RideKind,
    /// The vehicle's kind (a cab and an exec's car are a `Car`).
    pub kind: AssetKind,
    /// The key is an `Asset` entity (driven: `AssetLoc::InUse`); false: an
    /// entity spawned for the ride, despawned with the motion.
    pub asset: bool,
    /// The body at the wheel; `None`: driven off screen (the Garage's cab,
    /// the exec's car, a cab whose body driver stepped off).
    pub driver: Option<EntityId>,
    /// Everyone aboard, the driver included, in boarding order.
    pub riders: Vec<EntityId>,
    /// Where the vehicle is now.
    pub pos: TilePos,
    pub route: Route,
}

/// `[motion]` on.
pub fn on(world: &World) -> bool {
    world.config.motion.is_some()
}

/// `[motion.vans]` on (the island pass phase 7).
pub fn vans_on(world: &World) -> bool {
    world.config.motion.as_ref().is_some_and(|m| m.vans.is_some())
}

/// The vehicle `rider` is aboard.
pub fn vehicle_of(world: &World, rider: EntityId) -> Option<EntityId> {
    if world.aboard.is_empty() {
        return None;
    }
    world.aboard.get(&rider).copied()
}

/// The position along `way` at `done` tiles (Chebyshev per segment) from
/// its start; integer arithmetic (deterministic on every box).
pub fn point_at(way: &[TilePos], done: u64) -> TilePos {
    let mut left = done;
    for w in way.windows(2) {
        let l = u64::from(crate::systems::law::chebyshev(w[0], w[1]));
        if left <= l {
            if l == 0 {
                return w[1];
            }
            let f = |a: u16, b: u16| -> u16 {
                let (a, b) = (i64::from(a), i64::from(b));
                (a + (b - a) * left as i64 / l as i64) as u16
            };
            return TilePos::new(f(w[0].x, w[1].x), f(w[0].y, w[1].y));
        }
        left -= l;
    }
    way.last().copied().unwrap_or_default()
}

/// The way's length in tiles (Chebyshev per segment).
pub fn way_len(way: &[TilePos]) -> u64 {
    way.windows(2).map(|w| u64::from(crate::systems::law::chebyshev(w[0], w[1]))).sum()
}

/// The rest of `way` from `done` tiles along it: on to its end (`to_end`),
/// else back to its start; the first tile the point at `done`.
pub fn split_way(way: &[TilePos], done: u64, to_end: bool) -> Vec<TilePos> {
    let p = point_at(way, done);
    let (mut before, mut after) = (Vec::new(), Vec::new());
    let mut acc = 0u64;
    for (i, &c) in way.iter().enumerate() {
        if i > 0 {
            acc += u64::from(crate::systems::law::chebyshev(way[i - 1], c));
        }
        if acc < done {
            before.push(c);
        } else if acc > done {
            after.push(c);
        }
    }
    let mut out = vec![p];
    if to_end {
        out.extend(after);
    } else {
        out.extend(before.into_iter().rev());
    }
    let end = if to_end { way.last() } else { way.first() };
    if let Some(&e) = end {
        if out.last() != Some(&e) {
            out.push(e);
        }
    }
    out
}

/// A timed vehicle's position at `tick`.
pub fn timed_pos(way: &[TilePos], depart: Tick, arrive: Tick, tick: Tick) -> TilePos {
    let (Some(&first), Some(&last)) = (way.first(), way.last()) else { return TilePos::default() };
    if tick >= arrive || arrive <= depart {
        return last;
    }
    if tick <= depart {
        return first;
    }
    let total = way_len(way);
    point_at(way, total * (tick - depart) / (arrive - depart))
}

/// The way from `from` to `to`: the corners of the road path (`[motion]
/// road_route`; the big map's A*, else the grid A*), else an L (along x,
/// then y). A flyer flies `straight`.
fn way(world: &World, from: TilePos, to: TilePos, straight: bool) -> Vec<TilePos> {
    if straight || from == to {
        return vec![from, to];
    }
    let road = world.config.motion.as_ref().is_some_and(|m| m.road_route);
    let path = if !road {
        None
    } else if world.big_paths() {
        world.big_path(from, to)
    } else {
        crate::exec::pathfind::astar(&world.map, from, to, world.config.exec.astar_max_expansions)
    };
    match path {
        Some(p) if p.last() == Some(&to) => corners(from, &p),
        _ => {
            let mut w = vec![from];
            let bend = TilePos::new(to.x, from.y);
            if bend != from && bend != to {
                w.push(bend);
            }
            w.push(to);
            w
        }
    }
}

/// `from`, then the tiles of `path` where it turns, then its end.
fn corners(from: TilePos, path: &[TilePos]) -> Vec<TilePos> {
    let mut out = vec![from];
    let dir = |a: TilePos, b: TilePos| {
        ((i32::from(b.x) - i32::from(a.x)).signum(), (i32::from(b.y) - i32::from(a.y)).signum())
    };
    let mut prev = from;
    for (i, &t) in path.iter().enumerate() {
        if let Some(&next) = path.get(i + 1) {
            if dir(prev, t) != dir(t, next) {
                out.push(t);
            }
        } else {
            out.push(t);
        }
        prev = t;
    }
    out
}

/// Put `rider` aboard `v` at the vehicle's tile, out of any building.
fn seat(world: &mut World, rider: EntityId, v: EntityId, at: TilePos) {
    if let Some(old) = vehicle_of(world, rider) {
        if old != v {
            alight(world, rider);
        }
    }
    world.leave_building(rider);
    if let Some(p) = world.comp_mut::<Position>(rider) {
        p.tile = at;
        p.building = None;
    }
    world.aboard.insert(rider, v);
    if let Some(m) = world.motions.get_mut(&v) {
        if !m.riders.contains(&rider) {
            m.riders.push(rider);
        }
    }
}

/// A timed vehicle leg starts: `rider` (at the wheel when `drives`) boards
/// `vehicle` (an `Asset`; else the vehicle `reuse` names when it is in
/// motion, else a fresh one) at his walk origin, and it runs to `to` over
/// `depart..=arrive`. A vehicle already in motion is re-routed from where
/// it stands. Returns the vehicle; `None` with `[motion]` off.
#[allow(clippy::too_many_arguments)]
pub fn board_timed(
    world: &mut World,
    rider: EntityId,
    ride: RideKind,
    kind: AssetKind,
    vehicle: Option<EntityId>,
    reuse: Option<EntityId>,
    to: TilePos,
    depart: Tick,
    arrive: Tick,
    drives: bool,
) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let reuse = reuse.filter(|v| world.motions.contains_key(v));
    let from = match reuse.and_then(|v| world.motions.get(&v)) {
        Some(m) => m.pos,
        None => crate::exec::walk_origin(world, rider)?,
    };
    let straight = kind == AssetKind::Flyer;
    let route = Route::Timed { way: way(world, from, to, straight), depart, arrive };
    let v = match (vehicle, reuse) {
        (Some(v), _) => v,
        (None, Some(v)) => v,
        (None, None) => world.spawn(),
    };
    match world.motions.get_mut(&v) {
        Some(m) => {
            m.route = route;
            m.ride = ride;
            m.pos = from;
            m.driver = if drives { Some(rider) } else { m.driver.filter(|d| m.riders.contains(d)) };
        }
        None => {
            let m = Motion {
                ride,
                kind,
                asset: vehicle.is_some(),
                driver: drives.then_some(rider),
                riders: Vec::new(),
                pos: from,
                route,
            };
            world.motions.insert(v, m);
        }
    }
    seat(world, rider, v, from);
    Some(v)
}

/// The island pass phase 6: train `train` of El line `line` enters service
/// as `v` at `pos` (`transit::ensure_trains`), empty and persistent.
pub fn add_train(world: &mut World, v: EntityId, line: u8, train: u16, pos: TilePos) {
    let m = Motion {
        ride: RideKind::Train,
        kind: AssetKind::Car,
        asset: false,
        driver: None,
        riders: Vec::new(),
        pos,
        route: Route::Line { line, train },
    };
    world.motions.insert(v, m);
}

/// `rider` boards train `v` at the platform (`transit::board_train`).
pub fn board_train(world: &mut World, rider: EntityId, v: EntityId) {
    let Some(at) = pos_now(world, v) else { return };
    seat(world, rider, v, at);
}

/// A Full driver's walk starts in `vehicle` (an `Asset`): the vehicle is
/// steered by his steps (`Route::Driven`). `None` with `[motion]` off.
pub fn board_driven(world: &mut World, driver: EntityId, vehicle: EntityId, kind: AssetKind) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let from = crate::exec::walk_origin(world, driver)?;
    let m = Motion {
        ride: RideKind::Car,
        kind,
        asset: true,
        driver: Some(driver),
        riders: Vec::new(),
        pos: from,
        route: Route::Driven,
    };
    world.motions.insert(vehicle, m);
    world.aboard.insert(driver, vehicle);
    if let Some(m) = world.motions.get_mut(&vehicle) {
        m.riders.push(driver);
    }
    Some(vehicle)
}

/// `rider` climbs into `v` (a passenger: the body cab's fare at its
/// pick-up). No-op when `v` is not in motion.
pub fn join(world: &mut World, rider: EntityId, v: EntityId) {
    let Some(at) = world.motions.get(&v).map(|m| m.pos) else { return };
    if vehicle_of(world, rider) == Some(v) {
        return;
    }
    seat(world, rider, v, at);
}

/// The vehicle's position now.
fn pos_now(world: &World, v: EntityId) -> Option<TilePos> {
    let m = world.motions.get(&v)?;
    Some(match &m.route {
        Route::Timed { way, depart, arrive } => timed_pos(way, *depart, *arrive, world.tick),
        Route::Driven => m.pos,
        Route::Line { line, train } => train_pos(world, *line, *train).unwrap_or(m.pos),
    })
}

/// A train's tile now: the timetable's place; `None` on a cut line (it
/// stands where it stopped).
fn train_pos(world: &World, line: u8, train: u16) -> Option<TilePos> {
    use crate::systems::transit;
    if transit::line_cut(world, line) {
        return None;
    }
    transit::train_at(world, line, u64::from(train), world.tick).and_then(|at| transit::train_tile(world, line, at))
}

/// May a rider whose plan ended stay aboard a train to its next stop: alive,
/// a body, free.
fn holdable(world: &World, rider: EntityId) -> bool {
    world.is_alive(rider)
        && world
            .comp::<Brain>(rider)
            .is_some_and(|b| b.cuffed_by.is_none() && b.lod != crate::components::Lod::Statistical)
        && !world.has::<crate::components::Sentence>(rider)
}

/// `rider` steps off where the vehicle is now (the way's end once it has
/// arrived); the vehicle's motion ends with its last rider. No-op when he
/// is aboard nothing. The island pass phase 6: aboard a train between two
/// stops he rides on to the next one and gets off there (`advance_train`;
/// never mid-track), held from acting meanwhile (`TransitState::held`).
pub fn alight(world: &mut World, rider: EntityId) {
    if world.aboard.is_empty() {
        return;
    }
    let Some(&v) = world.aboard.get(&rider) else { return };
    if let Some(Route::Line { line, train }) = world.motions.get(&v).map(|m| m.route.clone()) {
        let between = !crate::systems::transit::line_cut(world, line)
            && matches!(
                crate::systems::transit::train_at(world, line, u64::from(train), world.tick),
                Some(crate::systems::transit::TrainAt::Track { .. })
            );
        if between && holdable(world, rider) {
            if world.transit.held.insert(rider) {
                crate::systems::transit::note_train(world, 1);
            }
            return;
        }
    }
    // The island pass phase 7: the guard off his escort's van (or his walk)
    // takes his prisoner off with him.
    let mates: Vec<EntityId> = match world.motions.get(&v) {
        Some(m) if is_escort(world, v, m) && m.driver == Some(rider) => {
            m.riders.iter().copied().filter(|&r| r != rider).collect()
        }
        _ => Vec::new(),
    };
    alight_now(world, rider);
    for r in mates {
        if world.aboard.get(&r) == Some(&v) {
            alight_now(world, r);
        }
    }
}

/// An escort's van or walk (the island pass phase 7): not a transfer run.
fn is_escort(world: &World, v: EntityId, m: &Motion) -> bool {
    matches!(m.ride, RideKind::Van | RideKind::Foot) && !world.van_runs.contains_key(&v)
}

/// The island pass phase 7: a prisoner stays aboard a van (the cuffed rider
/// rule; `advance` drops anyone else off his leg): a body, cuffed by a
/// guard aboard the same vehicle (an escort), or sentenced on a transfer
/// run. He never acts aboard (`exec::run` skips the cuffed and the
/// sentenced).
fn prisoner_aboard(world: &World, v: EntityId, rider: EntityId, run: bool) -> bool {
    let Some(b) = world.comp::<Brain>(rider) else { return false };
    if b.lod == crate::components::Lod::Statistical {
        return false;
    }
    if run {
        return world.has::<crate::components::Sentence>(rider);
    }
    b.cuffed_by.is_some_and(|g| world.aboard.get(&g) == Some(&v))
}

/// `rider` steps off where the vehicle is now, a train between stops too
/// (a death, an emigrant, a demotion off screen, a cut line's walk).
pub fn alight_now(world: &mut World, rider: EntityId) {
    if world.aboard.is_empty() {
        return;
    }
    if !world.transit.held.is_empty() {
        world.transit.held.remove(&rider);
    }
    let Some(v) = world.aboard.remove(&rider) else { return };
    let at = pos_now(world, v);
    if let Some(at) = at {
        if let Some(p) = world.comp_mut::<Position>(rider).filter(|p| p.building.is_none()) {
            p.tile = at;
        }
    }
    let empty = match world.motions.get_mut(&v) {
        Some(m) => {
            m.riders.retain(|&r| r != rider);
            if m.driver == Some(rider) {
                m.driver = None;
            }
            if let Some(at) = at {
                m.pos = at;
            }
            m.riders.is_empty()
        }
        None => false,
    };
    if empty {
        end(world, v);
    }
}

/// The motion is over: the record goes, and a vehicle spawned for the ride
/// with it (an asset parks by `vehicles::end_trip`).
fn end(world: &mut World, v: EntityId) {
    // A train runs on empty (the island pass phase 6).
    if world.motions.get(&v).is_some_and(|m| matches!(m.route, Route::Line { .. })) {
        return;
    }
    // A transfer van drives on to the Precinct (the island pass phase 7):
    // its run ends it (`stations::van_tick`).
    if !world.van_runs.is_empty() && world.van_runs.contains_key(&v) {
        return;
    }
    finish(world, v);
}

/// The motion is over whatever it is (`end`, and a transfer van at the
/// Precinct): every rider still linked is off.
pub fn finish(world: &mut World, v: EntityId) {
    let Some(m) = world.motions.remove(&v) else { return };
    for r in m.riders {
        if world.aboard.get(&r) == Some(&v) {
            world.aboard.remove(&r);
        }
    }
    if !m.asset {
        world.despawn(v);
    }
}

/// `lod::set_lod`: a Full driver demoted to Coarse (his walk now a timed
/// leg): the vehicle he steers runs that leg from where it stands.
pub fn to_timed(world: &mut World, driver: EntityId) {
    let Some(v) = vehicle_of(world, driver) else { return };
    if !matches!(world.motions.get(&v).map(|m| &m.route), Some(Route::Driven)) {
        return;
    }
    let (to, arrive) = match world.comp::<Brain>(driver).map(|b| &b.exec) {
        Some(ExecState::GotoTimed { target, arrive_tick, .. }) => (target.tile, *arrive_tick),
        _ => {
            alight(world, driver);
            return;
        }
    };
    let now = world.tick;
    let Some(m) = world.motions.get(&v) else { return };
    let from = m.pos;
    let route = Route::Timed { way: way(world, from, to, false), depart: now, arrive };
    if let Some(m) = world.motions.get_mut(&v) {
        m.route = route;
    }
    // He had not stepped out yet: the timed vehicle carries him from here.
    world.leave_building(driver);
    if let Some(p) = world.comp_mut::<Position>(driver) {
        p.tile = from;
    }
}

/// A rider's exec is still the leg he boarded for: a timed leg, a walk
/// (the driver's steering), a flight. A cuffed or sentenced rider is off
/// (the guard has him; the cells hold him).
fn on_leg(world: &World, rider: EntityId) -> bool {
    world.comp::<Brain>(rider).is_some_and(|b| {
        b.cuffed_by.is_none()
            && b.lod != crate::components::Lod::Statistical
            && matches!(b.exec, ExecState::GotoTimed { .. } | ExecState::Goto { .. } | ExecState::Fly { .. })
    }) && !world.has::<crate::components::Sentence>(rider)
}

/// Every tick, after the steps (`exec::run`): each vehicle advances (a
/// timed one along its way, a driven one to its driver's tile) and its
/// riders take its tile. A rider no longer on his leg (dead, a plan
/// replaced, inside a building) steps off where he is; an empty vehicle's
/// motion ends.
pub fn advance(world: &mut World) {
    if world.motions.is_empty() {
        return;
    }
    let tick = world.tick;
    let keys: Vec<EntityId> = world.motions.keys().copied().collect();
    for v in keys {
        let Some(m) = world.motions.get(&v) else { continue };
        let pos = match &m.route {
            Route::Timed { way, depart, arrive } => timed_pos(way, *depart, *arrive, tick),
            Route::Driven => match m.driver.and_then(|d| world.comp::<Position>(d)) {
                Some(p) if p.building.is_none() => p.tile,
                _ => m.pos,
            },
            &Route::Line { line, train } => {
                advance_train(world, v, line, train);
                continue;
            }
        };
        let riders = m.riders.clone();
        // A driven vehicle's driver steers it: his walk moves him (out of
        // the building he leaves on his first step), the vehicle follows.
        let steers = matches!(m.route, Route::Driven).then_some(m.driver).flatten();
        // The island pass phase 7: a van's (or an escort walk's) prisoners
        // ride on in cuffs or under sentence.
        let van = matches!(m.ride, RideKind::Van | RideKind::Foot);
        let run = van && world.van_runs.contains_key(&v);
        let driver = m.driver;
        let mut off: Vec<EntityId> = Vec::new();
        for &r in &riders {
            let leg = if van && driver != Some(r) { prisoner_aboard(world, v, r, run) } else { on_leg(world, r) };
            let ok = world.is_alive(r)
                && world.aboard.get(&r) == Some(&v)
                && leg
                && (steers == Some(r) || world.comp::<Position>(r).is_some_and(|p| p.building.is_none()));
            if !ok {
                off.push(r);
                continue;
            }
            if steers == Some(r) {
                continue;
            }
            if let Some(p) = world.comp_mut::<Position>(r) {
                if p.tile != pos {
                    p.tile = pos;
                }
            }
        }
        if let Some(m) = world.motions.get_mut(&v) {
            m.pos = pos;
        }
        for r in off {
            if world.aboard.get(&r) == Some(&v) {
                alight(world, r);
            } else if let Some(m) = world.motions.get_mut(&v) {
                m.riders.retain(|&x| x != r);
                if m.driver == Some(r) {
                    m.driver = None;
                }
            }
        }
        if world.motions.get(&v).is_some_and(|m| m.riders.is_empty()) {
            end(world, v);
        }
    }
}

/// A train advances to its timetable place (a cut line's stands) and
/// carries its riders: each on his ride's leg (`ExecState::Ride`), or held
/// to the next stop and off there; anyone else steps off where it is.
fn advance_train(world: &mut World, v: EntityId, line: u8, train: u16) {
    use crate::systems::transit::{self, TrainAt};
    let Some(m) = world.motions.get(&v) else { return };
    let cut = transit::line_cut(world, line);
    let at = if cut { None } else { transit::train_at(world, line, u64::from(train), world.tick) };
    let pos = at.and_then(|a| transit::train_tile(world, line, a)).unwrap_or(m.pos);
    let at_stop = matches!(at, Some(TrainAt::Stop(_)));
    let riders = m.riders.clone();
    let mut off: Vec<EntityId> = Vec::new();
    for &r in &riders {
        let on = if world.transit.held.contains(&r) {
            holdable(world, r) && !at_stop
        } else {
            world.comp::<Brain>(r).is_some_and(|b| {
                b.cuffed_by.is_none()
                    && b.lod != crate::components::Lod::Statistical
                    && matches!(b.exec, ExecState::Ride { .. })
            }) && !world.has::<crate::components::Sentence>(r)
        };
        let ok = on
            && world.is_alive(r)
            && world.aboard.get(&r) == Some(&v)
            && world.comp::<Position>(r).is_some_and(|p| p.building.is_none());
        if !ok {
            off.push(r);
            continue;
        }
        if let Some(p) = world.comp_mut::<Position>(r) {
            p.tile = pos;
        }
    }
    if let Some(m) = world.motions.get_mut(&v) {
        m.pos = pos;
    }
    for r in off {
        if world.aboard.get(&r) == Some(&v) {
            alight_now(world, r);
        } else if let Some(m) = world.motions.get_mut(&v) {
            m.riders.retain(|&x| x != r);
        }
    }
}

/// The island pass phase 7: an escort's leg (`exec::escort_leg`) is a
/// vehicle in motion: `ride` a `Van` (the guard at the wheel; it stands at
/// the arrest's kerb until `depart`, the call) or `Foot` (a Coarse guard's
/// timed walk), from the guard's walk origin to `to` over `depart..=arrive`,
/// the cuffed `suspect` aboard. `None` with `[motion.vans]` off.
#[allow(clippy::too_many_arguments)]
pub fn board_escort(
    world: &mut World,
    guard: EntityId,
    suspect: EntityId,
    ride: RideKind,
    to: TilePos,
    depart: Tick,
    arrive: Tick,
) -> Option<EntityId> {
    if !vans_on(world) {
        return None;
    }
    let kind = if ride == RideKind::Van { AssetKind::Truck } else { AssetKind::Car };
    let v = board_timed(world, guard, ride, kind, None, None, to, depart, arrive, true)?;
    join(world, suspect, v);
    Some(v)
}

/// The island pass phase 7: a transfer van (`stations::van`) sets out
/// empty from `from` (a Station's door) along the road to `to` (the
/// Precinct's) over `depart..=arrive`, no body at the wheel (the city's
/// driver, off screen). Kept until its run ends it ([`finish`]).
pub fn spawn_van(world: &mut World, from: TilePos, to: TilePos, depart: Tick, arrive: Tick) -> EntityId {
    let route = Route::Timed { way: way(world, from, to, false), depart, arrive };
    let v = world.spawn();
    let m = Motion {
        ride: RideKind::Van,
        kind: AssetKind::Truck,
        asset: false,
        driver: None,
        riders: Vec::new(),
        pos: from,
        route,
    };
    world.motions.insert(v, m);
    v
}

/// The island pass phase 7: the length in tiles of a road vehicle's way
/// from `from` to `to` (the van's drive is timed over it).
pub fn road_len(world: &World, from: TilePos, to: TilePos) -> u32 {
    u32::try_from(way_len(&way(world, from, to, false))).unwrap_or(u32::MAX)
}

/// The way's tile of a vehicle in motion now (`None`: not in motion).
pub fn pos_of(world: &World, v: EntityId) -> Option<TilePos> {
    pos_now(world, v)
}

/// `World::check_indices`: every aboard link names a vehicle in motion that
/// lists the rider, and every listed rider links back.
pub fn check(world: &World) -> Result<(), String> {
    for (&r, &v) in &world.aboard {
        if !world.motions.get(&v).is_some_and(|m| m.riders.contains(&r)) {
            return Err(format!("rider {r} aboard {v}, which does not carry him"));
        }
    }
    for v in world.van_runs.keys() {
        if !world.motions.get(v).is_some_and(|m| m.ride == RideKind::Van) {
            return Err(format!("transfer run {v} has no van in motion"));
        }
    }
    for (&v, m) in &world.motions {
        for &r in &m.riders {
            if world.aboard.get(&r) != Some(&v) {
                return Err(format!("vehicle {v} lists {r}, who is not aboard it"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timed_pos_walks_the_corners() {
        let way = vec![TilePos::new(0, 0), TilePos::new(10, 0), TilePos::new(10, 10)];
        assert_eq!(timed_pos(&way, 100, 120, 100), TilePos::new(0, 0));
        assert_eq!(timed_pos(&way, 100, 120, 105), TilePos::new(5, 0));
        assert_eq!(timed_pos(&way, 100, 120, 110), TilePos::new(10, 0));
        assert_eq!(timed_pos(&way, 100, 120, 115), TilePos::new(10, 5));
        assert_eq!(timed_pos(&way, 100, 120, 120), TilePos::new(10, 10));
        assert_eq!(timed_pos(&way, 100, 120, 500), TilePos::new(10, 10));
    }

    #[test]
    fn test_corners_keep_the_turns() {
        let from = TilePos::new(0, 0);
        let path: Vec<TilePos> =
            [(1, 0), (2, 0), (2, 1), (2, 2), (3, 2)].iter().map(|&(x, y)| TilePos::new(x, y)).collect();
        assert_eq!(
            corners(from, &path),
            vec![TilePos::new(0, 0), TilePos::new(2, 0), TilePos::new(2, 2), TilePos::new(3, 2)]
        );
    }
}
