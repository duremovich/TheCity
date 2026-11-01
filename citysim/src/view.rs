//! The view's snapshot (city milestone C2; TheCityGraphics `spec/integration/SIM_VIEW_CONTRACT.md` § 2, § 5):
//! what the 3D client draws of the camera ring at one tick, as structs, with no I/O. `snapshot(&World, ring)` is
//! the bridge converter's `frame()` (TheCityGraphics `engine/simsnap`) moved in: every body in the ring at every
//! tier, every vehicle in motion in it, every El train, the ring's buildings, its corpses and the last tick's
//! events. The file form (`city-simview/0.1`, JSON) is `citysim-cli export-view`; in process the client's sim
//! thread calls this after each tick (`view/cityview` `simthread.rs`).
//!
//! Where a body is: a Coarse walker mid-walk on his way (`exec::walk_place`, not his `Position`, which stays at
//! the walk's start until he arrives); a rider at his vehicle's place; a train at its timetable's place
//! (`transit::train_at`); a timed vehicle along its way (`motion::timed_pos`). `ahead` is the way to the next
//! tick's place (the Full walker's next path tiles, a timed walk's or vehicle's corners), so the view lerps along
//! the street, not through the block between two ticks. Nothing here writes the world.

use crate::components::{
    Appearance, Brain, Building, BuildingKind, Corpse, GoalKind, Identity, Inventory, Lod, Position, Rect, Sex, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::ActionKind;
use crate::systems::motion::{point_at, timed_pos, way_len, RideKind, Route};
use crate::systems::transit::{train_at, train_tile, TrainAt};
use crate::time::{self, Season, Tick};
use crate::world::World;
use crate::AssetKind;

/// What a body is doing, as the view colours and times it (`ExecState`, flattened).
#[derive(Clone, Debug, PartialEq)]
pub enum ViewAct {
    Idle,
    /// A Full walk: the next tile and the tick its step lands.
    Walk {
        next: Option<TilePos>,
        step_at: Tick,
    },
    /// A Coarse timed goto (a walk on its way when `exec::walk_place` places it).
    Timed {
        arrive: Tick,
    },
    Use {
        kind: ActionKind,
        until: Tick,
    },
    Wait {
        until: Tick,
    },
    Fly {
        from: TilePos,
        depart: Tick,
        arrive: Tick,
    },
    Ride {
        depart: Tick,
        arrive: Tick,
    },
    JackedIn {
        since: Tick,
    },
    /// A child: no Brain, no exec (Statistical by construction).
    Child,
}

impl ViewAct {
    /// The file form's `act` word (`use:<ActionKind>` for a Use).
    pub fn word(&self) -> String {
        match self {
            ViewAct::Idle => "idle".into(),
            ViewAct::Walk { .. } => "walk".into(),
            ViewAct::Timed { .. } => "timed".into(),
            ViewAct::Use { kind, .. } => format!("use:{kind:?}"),
            ViewAct::Wait { .. } => "wait".into(),
            ViewAct::Fly { .. } => "fly".into(),
            ViewAct::Ride { .. } => "ride".into(),
            ViewAct::JackedIn { .. } => "jacked_in".into(),
            ViewAct::Child => "child".into(),
        }
    }
}

/// A body in the ring.
#[derive(Clone, Debug)]
pub struct ViewAgent {
    pub id: EntityId,
    /// Where the body is now (placed: see the module doc).
    pub t: TilePos,
    /// Inside this building (`Position.building`); `None` for a placed walker or a rider.
    pub b: Option<EntityId>,
    /// The storey it stands on inside `b` (`Position.floor`, C6a; 0 on the street): the view draws a body indoors
    /// on its floor in the cutaway.
    pub floor: u8,
    /// `None`: a child (no Brain).
    pub lod: Option<Lod>,
    pub act: ViewAct,
    pub goal: Option<GoalKind>,
    pub step: Option<ActionKind>,
    /// Compass degrees clockwise from north (the city frame, y north) of the way ahead; `None`: standing (the
    /// view keeps the last heading it drew).
    pub facing: Option<f32>,
    /// The tiles to the next tick's place, in order (empty: standing, or no known way).
    pub ahead: Vec<TilePos>,
    pub aboard: Option<EntityId>,
    pub cuffed: Option<EntityId>,
    pub sex: Sex,
    /// Years (`Identity.age_days` / 120).
    pub age: u32,
    pub dress: Option<u8>,
    pub chrome: Option<u8>,
    pub colours: Option<EntityId>,
    pub weapon: u8,
    pub deck: u8,
    pub food: Option<u32>,
}

/// A vehicle's route, as the view reads it.
#[derive(Clone, Debug)]
pub enum ViewRoute {
    Driven,
    Timed { way: Vec<TilePos>, depart: Tick, arrive: Tick },
    Line { line: u8, train: u16, at: Option<TrainAt> },
}

/// A vehicle in motion in the ring, or an El train (every train, wherever it is).
#[derive(Clone, Debug)]
pub struct ViewVehicle {
    pub id: EntityId,
    pub ride: RideKind,
    pub kind: AssetKind,
    pub asset: bool,
    /// Placed: a train at `train_at`'s tile, a timed vehicle along its way.
    pub t: TilePos,
    pub facing: Option<f32>,
    pub ahead: Vec<TilePos>,
    pub driver: Option<EntityId>,
    pub riders: Vec<EntityId>,
    pub route: ViewRoute,
    /// A transfer van's prisoners (aboard, or bodiless off screen) and its arrival.
    pub van_run: Option<(Vec<EntityId>, Tick)>,
}

/// A building's state this tick (the ring's: its rect overlaps the ring).
#[derive(Clone, Debug)]
pub struct ViewBuilding {
    pub id: EntityId,
    pub occupants: usize,
    pub demolished: bool,
    pub derelict: bool,
    /// Looted shut or under construction now.
    pub closed: bool,
    /// C6b: a business's unit (a tenancy that is no Home or vacant Lot): its sign.
    pub unit: Option<ViewUnit>,
}

/// C6b: a business on its host's floors, for its sign on the facade: the host's rect, the door its sign hangs by,
/// its storey, its kind (the sign's colour), lit while open (`floors::open_at`), and founded within the last day.
#[derive(Clone, Debug)]
pub struct ViewUnit {
    pub kind: BuildingKind,
    pub rect: Rect,
    pub door: TilePos,
    pub floor: u8,
    pub lit: bool,
    pub fresh: bool,
}

/// A building's fixed facts (the file form's header; the client reads them once).
#[derive(Clone, Debug)]
pub struct ViewBuildingInfo {
    pub id: EntityId,
    pub kind: BuildingKind,
    pub rect: Rect,
    pub door: TilePos,
    pub floors: u8,
    pub tier: u8,
    pub capacity: u16,
    pub owner: Option<EntityId>,
    /// C6a: a tenancy's host (the structure it stands in), else `None`.
    pub within: Option<EntityId>,
    /// C6a: the storeys it stands on, `lo..=hi` (`Building.span`).
    pub span: Option<(u8, u8)>,
    pub label: Option<String>,
}

/// An event of the last tick whose first placed actor stands in the ring.
#[derive(Clone, Debug)]
pub struct ViewEvent {
    pub id: u64,
    pub tick: Tick,
    pub kind: EventKind,
    pub actors: Vec<EntityId>,
    pub t: Option<TilePos>,
    pub text: String,
}

/// One tick of the camera ring.
#[derive(Clone, Debug)]
pub struct ViewFrame {
    /// `World.tick` after the tick ran (the frame shows the state the tick left).
    pub tick: Tick,
    pub day: u64,
    pub clock: String,
    pub dark: bool,
    pub season: Season,
    pub ring: Rect,
    pub agents: Vec<ViewAgent>,
    pub vehicles: Vec<ViewVehicle>,
    pub buildings: Vec<ViewBuilding>,
    pub corpses: Vec<(EntityId, TilePos)>,
    pub events: Vec<ViewEvent>,
}

impl ViewFrame {
    pub fn agent(&self, id: EntityId) -> Option<&ViewAgent> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn vehicle(&self, id: EntityId) -> Option<&ViewVehicle> {
        self.vehicles.iter().find(|v| v.id == id)
    }
}

/// Compass degrees clockwise from north (city frame, y north) of a step from `a` to `b` in sim tiles (y south).
pub fn heading(a: TilePos, b: TilePos) -> Option<f32> {
    let (dx, dy) = (f32::from(b.x) - f32::from(a.x), f32::from(a.y) - f32::from(b.y));
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    Some((dx.atan2(dy).to_degrees() + 360.0) % 360.0)
}

/// A ring of `half` tiles about `centre` on a `w` x `h` map, clipped to it.
pub fn ring_about(centre: TilePos, half: u16, w: usize, h: usize) -> Rect {
    let (w, h) = (w.min(usize::from(u16::MAX)) as i64, h.min(usize::from(u16::MAX)) as i64);
    let half = i64::from(half.max(1));
    let x0 = (i64::from(centre.x) - half).clamp(0, w - 1);
    let y0 = (i64::from(centre.y) - half).clamp(0, h - 1);
    Rect { x: x0 as u16, y: y0 as u16, w: (2 * half).min(w - x0) as u16, h: (2 * half).min(h - y0) as u16 }
}

/// The corners of `way` passed between `done0` and `done1` tiles from its start, then the place at `done1`.
fn way_span(way: &[TilePos], done0: u64, done1: u64) -> Vec<TilePos> {
    let mut out = vec![];
    if done1 <= done0 || way.len() < 2 {
        return out;
    }
    let mut at = 0u64;
    for w in way.windows(2) {
        at += u64::from(crate::systems::law::chebyshev(w[0], w[1]));
        if at > done0 && at < done1 {
            out.push(w[1]);
        }
    }
    let end = point_at(way, done1);
    if out.last() != Some(&end) {
        out.push(end);
    }
    out
}

/// A timed way's place at `tick` and its way on to `tick + 1`.
fn timed_place(way: &[TilePos], depart: Tick, arrive: Tick, tick: Tick) -> (TilePos, Vec<TilePos>) {
    let now = timed_pos(way, depart, arrive, tick);
    if tick >= arrive || arrive <= depart {
        return (now, vec![]);
    }
    let total = way_len(way);
    let done = |t: Tick| if t <= depart { 0 } else { total * (t.min(arrive) - depart) / (arrive - depart) };
    (now, way_span(way, done(tick), done(tick + 1)))
}

/// A Coarse body's walk this tick, if its box (start to target, grown by `margin`) comes near the ring: the walk's
/// way is one road search, so a walk nowhere near the camera is not searched.
fn walk_near(world: &World, id: EntityId, ring: &Rect, margin: u16) -> Option<(TilePos, Vec<TilePos>)> {
    let b = world.comp::<Brain>(id)?;
    let ExecState::GotoTimed { target, walk: Some(w), .. } = &b.exec else { return None };
    let (lo_x, hi_x) = (w.from.x.min(target.tile.x), w.from.x.max(target.tile.x));
    let (lo_y, hi_y) = (w.from.y.min(target.tile.y), w.from.y.max(target.tile.y));
    let grown = Rect {
        x: lo_x.saturating_sub(margin),
        y: lo_y.saturating_sub(margin),
        w: (hi_x - lo_x).saturating_add(margin.saturating_mul(2)).saturating_add(1),
        h: (hi_y - lo_y).saturating_add(margin.saturating_mul(2)).saturating_add(1),
    };
    if !grown.overlaps(ring) {
        return None;
    }
    let (way, depart, arrive) = crate::exec::walk_route(world, id)?;
    Some(timed_place(&way, depart, arrive, world.tick))
}

fn act_of(b: &Brain) -> ViewAct {
    match &b.exec {
        ExecState::Idle => ViewAct::Idle,
        ExecState::Goto { path, next_move_tick, .. } => {
            ViewAct::Walk { next: path.last().copied(), step_at: *next_move_tick }
        }
        ExecState::GotoTimed { arrive_tick, .. } => ViewAct::Timed { arrive: *arrive_tick },
        ExecState::Use { kind, until, .. } => ViewAct::Use { kind: *kind, until: *until },
        ExecState::Wait { until } => ViewAct::Wait { until: *until },
        ExecState::Fly { from, depart, arrive_tick, .. } => {
            ViewAct::Fly { from: *from, depart: *depart, arrive: *arrive_tick }
        }
        ExecState::Ride { depart, arrive_tick, .. } => ViewAct::Ride { depart: *depart, arrive: *arrive_tick },
        ExecState::JackedIn { since, .. } => ViewAct::JackedIn { since: *since },
    }
}

/// The camera ring at the world's tick (see the module doc). Pure: the same world and ring give the same frame.
pub fn snapshot(world: &World, ring: Rect) -> ViewFrame {
    let tick = world.tick;
    let margin = world.config.view.way_margin_tiles;
    // A Full walker takes up to this many path tiles a tick (Y4's pace; a chromed leg more, clipped by the view).
    let steps = usize::try_from(world.config.exec.walk_tiles_per_tick.max(1) * 2).unwrap_or(usize::MAX);

    // Vehicles first: a rider stands at his vehicle's place.
    let mut vehicles = vec![];
    for (&v, m) in &world.motions {
        let (t, ahead, route) = match &m.route {
            Route::Driven => {
                // the driver's next path tile is the car's
                let ahead = m
                    .driver
                    .and_then(|d| world.comp::<Brain>(d))
                    .and_then(|b| match &b.exec {
                        ExecState::Goto { path, .. } => Some(path.iter().rev().take(steps).copied().collect()),
                        _ => None,
                    })
                    .unwrap_or_default();
                (m.pos, ahead, ViewRoute::Driven)
            }
            Route::Timed { way, depart, arrive } => {
                let (t, ahead) = timed_place(way, *depart, *arrive, tick);
                (t, ahead, ViewRoute::Timed { way: way.clone(), depart: *depart, arrive: *arrive })
            }
            Route::Line { line, train } => {
                let at = train_at(world, *line, u64::from(*train), tick);
                let t = at.and_then(|a| train_tile(world, *line, a)).unwrap_or(m.pos);
                let next =
                    train_at(world, *line, u64::from(*train), tick + 1).and_then(|a| train_tile(world, *line, a));
                let ahead = next.filter(|n| *n != t).map(|n| vec![n]).unwrap_or_default();
                (t, ahead, ViewRoute::Line { line: *line, train: *train, at })
            }
        };
        let train = matches!(m.route, Route::Line { .. });
        if !train && !ring.contains(t) {
            continue;
        }
        vehicles.push(ViewVehicle {
            id: v,
            ride: m.ride,
            kind: m.kind,
            asset: m.asset,
            t,
            facing: ahead.first().and_then(|n| heading(t, *n)),
            ahead,
            driver: m.driver,
            riders: m.riders.clone(),
            route,
            van_run: world.van_runs.get(&v).map(|r| (r.prisoners.clone(), r.arrive)),
        });
    }

    let mut agents = vec![];
    // scan-ok: the view, once a published frame (not once a tick of the sim) over every body
    for e in world.entities() {
        if world.has::<Corpse>(e) {
            continue;
        }
        let (Some(p), Some(idn)) = (world.comp::<Position>(e), world.comp::<Identity>(e)) else { continue };
        let brain = world.comp::<Brain>(e);
        let aboard = world.aboard.get(&e).copied();
        // the place: aboard -> the vehicle's; a Coarse walker -> his way's; else his Position
        let mut b = p.building;
        let mut ahead = vec![];
        let t = if let Some(v) = aboard.and_then(|v| world.motions.get(&v).map(|_| v)) {
            b = None;
            match vehicles.iter().find(|x| x.id == v) {
                Some(x) => x.t,
                // a vehicle outside the ring: its rider is outside it too
                None => continue,
            }
        } else if let Some((t, way)) = walk_near(world, e, &ring, margin) {
            b = None;
            ahead = way;
            t
        } else {
            p.tile
        };
        if !ring.contains(t) {
            continue;
        }
        let act = brain.map_or(ViewAct::Child, act_of);
        if let (Some(br), true) = (brain, ahead.is_empty() && b.is_none()) {
            if let ExecState::Goto { path, .. } = &br.exec {
                ahead = path.iter().rev().take(steps).copied().collect();
            }
        }
        let app = world.comp::<Appearance>(e);
        let kit = crate::systems::assets::kit(world, e);
        agents.push(ViewAgent {
            id: e,
            t,
            b,
            floor: if b.is_some() { p.floor } else { 0 },
            lod: brain.map(|br| br.lod),
            act,
            goal: brain.and_then(|br| br.current_goal),
            step: brain.and_then(|br| br.current_step()).map(|s| s.action),
            facing: ahead.first().and_then(|n| heading(t, *n)),
            ahead,
            aboard,
            cuffed: brain.and_then(|br| br.cuffed_by),
            sex: idn.sex,
            age: idn.age_days / 120,
            dress: app.map(|a| a.dress),
            chrome: app.map(|a| a.chrome),
            colours: app.and_then(|a| a.colours),
            weapon: kit.weapon,
            deck: kit.deck_tier,
            food: world.comp::<Inventory>(e).map(|i| i.food),
        });
    }

    let mut buildings = vec![];
    for b in world.with::<Building>() {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if !bd.rect.overlaps(&ring) {
            continue;
        }
        buildings.push(ViewBuilding {
            id: b,
            occupants: bd.occupants.len(),
            demolished: bd.demolished,
            derelict: bd.derelict,
            closed: bd.closed_until.is_some_and(|t| t > tick) || bd.under_construction_until.is_some_and(|t| t > tick),
            unit: view_unit(world, b, bd),
        });
    }

    let corpses = world
        .with::<Corpse>()
        .into_iter()
        .filter_map(|c| world.comp::<Position>(c).filter(|p| ring.contains(p.tile)).map(|p| (c, p.tile)))
        .collect();

    // the last tick's events (the tick that ran is `tick - 1`), placed at the first actor's place
    let place = |a: EntityId| {
        agents.iter().find(|x| x.id == a).map(|x| x.t).or_else(|| world.comp::<Position>(a).map(|p| p.tile))
    };
    let events = world
        .events
        .iter()
        .rev()
        .take_while(|ev| ev.tick + 1 >= tick)
        .filter(|ev| ev.tick + 1 == tick)
        .filter_map(|ev| {
            let t = ev.actors.iter().find_map(|a| place(*a));
            t.filter(|t| ring.contains(*t)).map(|t| ViewEvent {
                id: ev.id,
                tick: ev.tick,
                kind: ev.kind,
                actors: ev.actors.to_vec(),
                t: Some(t),
                text: ev.text.clone(),
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    ViewFrame {
        tick,
        day: time::day(tick),
        clock: time::clock(tick),
        dark: time::is_dark(tick),
        season: time::season(tick),
        ring,
        agents,
        vehicles,
        buildings,
        corpses,
        events,
    }
}

/// C6b: `b`'s sign, when it is a business on its host's floors.
fn view_unit(world: &World, b: EntityId, bd: &Building) -> Option<ViewUnit> {
    bd.within?;
    if bd.demolished || bd.derelict || matches!(bd.kind, BuildingKind::Home | BuildingKind::Lot) {
        return None;
    }
    let tod = world.tick_of_day();
    let fresh =
        world.floor_index.founded.get(&b).is_some_and(|&t| world.tick.saturating_sub(t) < crate::time::TICKS_PER_DAY);
    Some(ViewUnit {
        kind: bd.kind,
        rect: bd.rect,
        door: bd.door,
        floor: bd.span.map_or(0, |(lo, _)| lo),
        lit: crate::systems::floors::open_at(world, b, tod),
        fresh,
    })
}

/// The ring's buildings' fixed facts (the file form's header).
pub fn buildings_in(world: &World, ring: Rect) -> Vec<ViewBuildingInfo> {
    world
        .with::<Building>()
        .into_iter()
        .filter_map(|b| {
            let bd = world.comp::<Building>(b)?;
            bd.rect.overlaps(&ring).then(|| ViewBuildingInfo {
                id: b,
                kind: bd.kind,
                rect: bd.rect,
                door: bd.door,
                floors: bd.floors,
                tier: bd.tier,
                capacity: bd.capacity,
                owner: bd.owner,
                within: bd.within,
                span: bd.span,
                label: bd.label.clone(),
            })
        })
        .collect()
}

/// Pin every Brain standing in `ring` (OPEN-149's rule, "nothing in camera range is Statistical", emulated with
/// `Brain.pinned`, simsnap's stopgap); unpin the ones `pinned` holds that have left `keep` (the ring grown by the
/// view's margin). `pinned` is the caller's set of the pins it set (a body pinned by anyone else is left pinned).
/// Returns how many it pinned.
pub fn pin_ring(world: &mut World, ring: Rect, keep: Rect, pinned: &mut std::collections::BTreeSet<EntityId>) -> usize {
    let mut n = 0;
    let gone: Vec<EntityId> = pinned
        .iter()
        .copied()
        .filter(|&e| !world.comp::<Position>(e).is_some_and(|p| keep.contains(p.tile)) || !world.is_alive(e))
        .collect();
    for e in gone {
        pinned.remove(&e);
        if let Some(b) = world.comp_mut::<Brain>(e) {
            b.pinned = false;
        }
    }
    // scan-ok: the view, once a published frame
    let inside: Vec<EntityId> = world
        .entities()
        .filter(|&e| world.comp::<Position>(e).is_some_and(|p| ring.contains(p.tile)) && world.has::<Brain>(e))
        .collect();
    for e in inside {
        if let Some(b) = world.comp_mut::<Brain>(e) {
            if !b.pinned {
                b.pinned = true;
                pinned.insert(e);
                n += 1;
            }
        }
    }
    n
}

/// The walk-matched rate: game ticks a real second at which a walker on screen walks `[view] walk_m_s` (the pace
/// is `[exec] walk_tiles_per_tick` tiles every `move_ticks_full` ticks, a tile `[view] tile_m`).
pub fn walk_matched_rate(world: &World) -> f64 {
    let e = &world.config.exec;
    let m_per_tick = world.config.view.tile_m * e.walk_tiles_per_tick.max(1) as f64 / e.move_ticks_full.max(1) as f64;
    world.config.view.walk_m_s / m_per_tick
}
