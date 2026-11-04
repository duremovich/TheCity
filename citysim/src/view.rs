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
//!
//! **The diary minute and the frame** (the view milestone V6, `OPEN-270`; the one place this rule is written): a
//! frame's `tick` is `World.tick` after the tick ran, so frame `T + 1` shows the state minute `T` left, and a shadow
//! diary line stamped `T` (its `now` is the tick that ran) is that state. A shot "at" a diary minute `T`
//! (cityview `--shots T`, live or recorded) shows frame `T + 1`, and the HUD's caption leads with the diary minute
//! (`diary_minute`), not the frame's clock (which reads one minute on).
//!
//! **The recording** (V6): `citysim-cli shadow --record` writes each pick's ring a tick as `city-simrec/0.1`, one
//! gzip'd JSON-lines file per pick per diary day: a `RecHeader`, then a `RecFrame` a tick, the frame as a delta on
//! the one before (`RecEncoder`; the bodies, vehicles and buildings new or changed, the ids gone). cityview's
//! `--sim FILE` plays it (`RecDecoder`) through the follow-cam's path, so a diary line and the shot at its minute
//! are the same world's.

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
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a body is doing, as the view colours and times it (`ExecState`, flattened).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// The city milestone C7: a climb inside a structure from storey `from`
    /// to `to` (a Full body's `ExecState::Climb`, a Coarse leg's climb): by
    /// the lift (`lift`), waiting at the lower end until `board`; the
    /// storey reached now is the agent's `floor`.
    Climb {
        from: u8,
        to: u8,
        lift: bool,
        board: Tick,
        arrive: Tick,
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
            ViewAct::Climb { .. } => "climb".into(),
            ViewAct::Child => "child".into(),
        }
    }
}

/// A body in the ring.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewAgent {
    pub id: EntityId,
    /// Where the body is now (placed: see the module doc).
    pub t: TilePos,
    /// Inside this building (`Position.building`); `None` for a placed walker or a rider.
    pub b: Option<EntityId>,
    /// The storey it stands on inside `b` (`Position.floor`, C6a), on the street its walkway deck's level (C7; 0
    /// the ground), on a climb the storey it has reached (rounded; `act` holds the climb): the view draws a body
    /// indoors on its floor in the cutaway, on a deck at the deck's height.
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
    /// The view milestone V3: `b`'s rect (a body indoors stands at its door tile; the view draws it inside the rect).
    pub rect: Option<Rect>,
}

/// A vehicle's route, as the view reads it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ViewRoute {
    Driven,
    Timed { way: Vec<TilePos>, depart: Tick, arrive: Tick },
    Line { line: u8, train: u16, at: Option<TrainAt> },
}

/// A vehicle in motion in the ring, or an El train (every train, wherever it is).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewUnit {
    pub kind: BuildingKind,
    pub rect: Rect,
    pub door: TilePos,
    pub floor: u8,
    pub lit: bool,
    pub fresh: bool,
}

/// A building's fixed facts (the file form's header; the client reads them once).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewEvent {
    pub id: u64,
    pub tick: Tick,
    pub kind: EventKind,
    pub actors: Vec<EntityId>,
    pub t: Option<TilePos>,
    pub text: String,
}

/// One tick of the camera ring.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
/// S2 (`OPEN-312`): a Statistical body standing on the street in a ring (its `Position` the demotion's snapshot,
/// stale for hours): the building its schedule has it in now (`ring::schedule_door`), that building's door and the
/// storey it would stand on; `None` off the street, outside the rings, or with no place of its own (homeless off
/// shift: its `Position` stands).
fn stat_place(
    world: &World,
    e: EntityId,
    p: &Position,
    brain: Option<&Brain>,
    in_rings: &impl Fn(TilePos) -> bool,
) -> Option<(EntityId, TilePos, u8)> {
    if p.building.is_some() || brain.is_none_or(|b| b.lod != Lod::Statistical) || !in_rings(p.tile) {
        return None;
    }
    let d = crate::systems::ring::schedule_door(world, e)?;
    let door = world.comp::<Building>(d)?.door;
    Some((d, door, crate::systems::floors::floor_in(world, e, d)))
}

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
/// V6: `cache` keeps each walk's way across frames (a walk's way is fixed until the walk changes; the road search
/// was most of a recording's time).
fn walk_near(
    world: &World,
    id: EntityId,
    ring: &Rect,
    margin: u16,
    cache: Option<&mut WayCache>,
) -> Option<(TilePos, Vec<TilePos>)> {
    let b = world.comp::<Brain>(id)?;
    let ExecState::GotoTimed { target, walk: Some(w), arrive_tick, .. } = &b.exec else { return None };
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
    let Some(cache) = cache else {
        let (way, depart, arrive) = crate::exec::walk_route(world, id)?;
        return Some(timed_place(&way, depart, arrive, world.tick));
    };
    let key = (w.from, target.tile, w.depart, *arrive_tick);
    let hit = cache.ways.get(&id).filter(|c| c.0 == key).map(|c| (c.1.clone(), c.2, c.3));
    let (way, depart, arrive) = match hit {
        Some(r) => r,
        None => {
            let r = crate::exec::walk_route(world, id)?;
            cache.ways.insert(id, (key, r.0.clone(), r.1, r.2, world.tick));
            r
        }
    };
    if let Some(c) = cache.ways.get_mut(&id) {
        c.4 = world.tick;
    }
    Some(timed_place(&way, depart, arrive, world.tick))
}

/// V6: the Coarse walkers' ways a snapshot placed, kept for the next frame (`snapshot_many_cached`): keyed by the
/// walk (its start, target, departure and arrival), dropped once a frame passes without it.
#[derive(Default)]
pub struct WayCache {
    #[allow(clippy::type_complexity)]
    ways: BTreeMap<EntityId, ((TilePos, TilePos, Tick, Tick), Vec<TilePos>, Tick, Tick, Tick)>,
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
        ExecState::Climb { from, to, lift, board, arrive_tick, .. } => {
            ViewAct::Climb { from: *from, to: *to, lift: *lift, board: *board, arrive: *arrive_tick }
        }
    }
}

/// The camera ring at the world's tick (see the module doc). Pure: the same world and ring give the same frame.
pub fn snapshot(world: &World, ring: Rect) -> ViewFrame {
    snapshot_many(world, &[ring]).pop().expect("one ring, one frame")
}

/// The view milestone V6 (`OPEN-270`): a frame for each of `rings` from one pass over the world (the shadow's
/// recording: one world, many picks, each pick's ring as the world has it). `snapshot` is its one-ring case.
pub fn snapshot_many(world: &World, rings: &[Rect]) -> Vec<ViewFrame> {
    snapshot_many_cached(world, rings, None)
}

/// `snapshot_many` with the walkers' ways kept in `cache` across calls (the recorder's, a frame a tick).
pub fn snapshot_many_cached(world: &World, rings: &[Rect], mut cache: Option<&mut WayCache>) -> Vec<ViewFrame> {
    let tick = world.tick;
    let margin = world.config.view.way_margin_tiles;
    // A Full walker takes up to this many path tiles a tick (Y4's pace; a chromed leg more, clipped by the view).
    let steps = usize::try_from(world.config.exec.walk_tiles_per_tick.max(1) * 2).unwrap_or(usize::MAX);
    let mut frames: Vec<ViewFrame> = rings
        .iter()
        .map(|&ring| ViewFrame {
            tick,
            day: time::day(tick),
            clock: time::clock(tick),
            dark: time::is_dark(tick),
            season: time::season(tick),
            ring,
            agents: vec![],
            vehicles: vec![],
            buildings: vec![],
            corpses: vec![],
            events: vec![],
        })
        .collect();
    let in_rings = |t: TilePos| rings.iter().any(|r| r.contains(t));

    // Vehicles first: a rider stands at his vehicle's place.
    let mut vehicles: BTreeMap<EntityId, ViewVehicle> = BTreeMap::new();
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
        if !train && !in_rings(t) {
            continue;
        }
        let x = ViewVehicle {
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
        };
        for (f, ring) in frames.iter_mut().zip(rings) {
            if train || ring.contains(t) {
                f.vehicles.push(x.clone());
            }
        }
        vehicles.insert(v, x);
    }

    // every placed body's tile, for the events' places
    let mut placed: BTreeMap<EntityId, TilePos> = BTreeMap::new();
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
        let mut floor = p.floor;
        let mut act_climb = None;
        let t = if let Some((t, cb, f)) = crate::exec::climb_place(world, e) {
            // C7: on a climb at the door of the building climbed
            b = Some(cb);
            floor = f.round() as u8;
            if let Some(ExecState::GotoTimed { climb: Some(c), arrive_tick, .. }) = brain.map(|br| &br.exec) {
                let leg = if c.down.is_some() && tick < c.depart + c.down_ticks() { c.down } else { c.up };
                if let Some(l) = leg {
                    let (s, e) = if l.to == 0 && l.from > 0 && c.down == Some(l) {
                        (c.depart, c.depart + l.plan.ticks)
                    } else {
                        (arrive_tick.saturating_sub(l.plan.ticks), *arrive_tick)
                    };
                    act_climb = Some(ViewAct::Climb {
                        from: l.from,
                        to: l.to,
                        lift: l.plan.lift,
                        board: s + l.plan.wait,
                        arrive: e,
                    });
                }
            }
            t
        } else if let Some(v) = aboard.and_then(|v| world.motions.get(&v).map(|_| v)) {
            b = None;
            match vehicles.get(&v) {
                Some(x) => x.t,
                // a vehicle outside the rings: its rider is outside them too
                None => continue,
            }
        } else if let Some((t, way)) = rings.iter().find_map(|r| walk_near(world, e, r, margin, cache.as_deref_mut())) {
            b = None;
            ahead = way;
            t
        } else if let Some(d) = stat_place(world, e, p, brain, &in_rings) {
            // S2 (`OPEN-312`): a Statistical body on the street is a stale snapshot; it is drawn where its schedule
            // has it (inside its home or workplace), where the ring's promotion will put it
            b = Some(d.0);
            floor = d.2;
            d.1
        } else {
            p.tile
        };
        if !in_rings(t) {
            continue;
        }
        placed.insert(e, t);
        let act = act_climb.unwrap_or_else(|| brain.map_or(ViewAct::Child, act_of));
        if let (Some(br), true) = (brain, ahead.is_empty() && b.is_none()) {
            if let ExecState::Goto { path, .. } = &br.exec {
                ahead = path.iter().rev().take(steps).copied().collect();
            }
        }
        let app = world.comp::<Appearance>(e);
        let kit = crate::systems::assets::kit(world, e);
        let agent = ViewAgent {
            id: e,
            t,
            b,
            floor: if b.is_some() {
                floor
            } else if p.building.is_none() && aboard.is_none() {
                p.floor
            } else {
                0
            },
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
            rect: b.and_then(|b| world.comp::<Building>(b)).map(|bd| bd.rect),
        };
        for (f, ring) in frames.iter_mut().zip(rings) {
            if ring.contains(t) {
                f.agents.push(agent.clone());
            }
        }
    }

    for b in world.with::<Building>() {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if !rings.iter().any(|r| bd.rect.overlaps(r)) {
            continue;
        }
        let vb = ViewBuilding {
            id: b,
            occupants: bd.occupants.len(),
            demolished: bd.demolished,
            derelict: bd.derelict,
            closed: bd.closed_until.is_some_and(|t| t > tick) || bd.under_construction_until.is_some_and(|t| t > tick),
            unit: view_unit(world, b, bd),
        };
        for (f, ring) in frames.iter_mut().zip(rings) {
            if bd.rect.overlaps(ring) {
                f.buildings.push(vb.clone());
            }
        }
    }

    for c in world.with::<Corpse>() {
        let Some(p) = world.comp::<Position>(c) else { continue };
        for (f, ring) in frames.iter_mut().zip(rings) {
            if ring.contains(p.tile) {
                f.corpses.push((c, p.tile));
            }
        }
    }

    // the last tick's events (the tick that ran is `tick - 1`), placed at the first actor's place
    let place = |a: EntityId| placed.get(&a).copied().or_else(|| world.comp::<Position>(a).map(|p| p.tile));
    let mut last: Vec<&crate::events::Event> =
        world.events.iter().rev().take_while(|ev| ev.tick + 1 >= tick).filter(|ev| ev.tick + 1 == tick).collect();
    last.reverse();
    for ev in last {
        let Some(t) = ev.actors.iter().find_map(|a| place(*a)) else { continue };
        for (f, ring) in frames.iter_mut().zip(rings) {
            if ring.contains(t) {
                f.events.push(ViewEvent {
                    id: ev.id,
                    tick: ev.tick,
                    kind: ev.kind,
                    actors: ev.actors.to_vec(),
                    t: Some(t),
                    text: ev.text.clone(),
                });
            }
        }
    }
    if let Some(c) = cache {
        c.ways.retain(|_, w| w.4 == tick);
    }
    frames
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

/// The walk-matched rate: game ticks a real second at which a walker on screen walks `[view] walk_m_s` (the pace
/// is `[exec] walk_tiles_per_tick` tiles every `move_ticks_full` ticks, a tile `[view] tile_m`).
pub fn walk_matched_rate(world: &World) -> f64 {
    let e = &world.config.exec;
    let m_per_tick = world.config.view.tile_m * e.walk_tiles_per_tick.max(1) as f64 / e.move_ticks_full.max(1) as f64;
    world.config.view.walk_m_s / m_per_tick
}

/// The camera's subject's place (the ring's centre): a building's door (C6b: a shop-house's units), its vehicle's
/// (a train at its timetable's tile), else its walk's (a Coarse walker on his way, a climber at the door), else its
/// `Position`. `None`: no place (gone).
pub fn subject_tile(world: &World, e: EntityId) -> Option<TilePos> {
    if let Some(b) = world.comp::<Building>(e) {
        return Some(b.door);
    }
    if let Some(v) = crate::systems::motion::vehicle_of(world, e) {
        if let Some(m) = world.motions.get(&v) {
            if let Route::Line { line, train } = m.route {
                let at = train_at(world, line, u64::from(train), world.tick);
                return Some(at.and_then(|a| train_tile(world, line, a)).unwrap_or(m.pos));
            }
            return Some(m.pos);
        }
    }
    crate::exec::walk_place(world, e).map(|p| p.tile)
}

/// The diary's minute a frame shows: `frame.tick - 1` (the module doc's rule), as the diary stamps it (`d21 08:00`).
pub fn diary_minute(frame_tick: Tick) -> String {
    let m = frame_tick.saturating_sub(1);
    format!("d{} {}", time::day(m), time::clock(m))
}

/// The view milestone V6: the follow-cam's caption for `e` in `frame` (the HUD's line; a recording keeps it a
/// tick): the diary minute the frame shows, who, the tier, what it is doing (the frame's act), its tile, the place
/// (the building and storey, the vehicle, the street or a deck's level) and its goal and step, in the diary's words.
pub fn caption(world: &World, frame: &ViewFrame, e: EntityId) -> String {
    let when = diary_minute(frame.tick);
    if world.has::<Building>(e) {
        return format!("diary {when}: {} (its door)", world.name_of(e));
    }
    let Some(a) = frame.agent(e) else {
        return format!("diary {when}: {} #{}: not in its ring", world.name_of(e), e.index);
    };
    let place = if let Some(v) = a.aboard {
        let what = frame.vehicle(v).map_or("a vehicle".to_string(), |x| match &x.route {
            ViewRoute::Line { line, .. } => {
                let name = world.map.lines.get(usize::from(*line)).map_or("?", |l| l.name.as_str());
                format!("the {name} train #{}", v.index)
            }
            _ => format!("a {:?} #{}", x.ride, v.index),
        });
        format!("aboard {what}")
    } else if let Some(b) = a.b {
        format!("in {} floor {}", world.name_of(b), a.floor)
    } else if a.floor > 0 {
        format!("on the street, deck level {}", a.floor)
    } else {
        "on the street".to_string()
    };
    let lod = a.lod.map_or("child".to_string(), |l| format!("{l:?}"));
    let goal = a.goal.map_or("-".to_string(), |g| format!("{g:?}"));
    let step = a.step.map_or("-".to_string(), |s| format!("{s:?}"));
    format!(
        "diary {when}: {} #{} ({lod}) {} at ({}, {}) {place}: {goal} / {step}",
        world.name_of(e),
        e.index,
        a.act.word(),
        a.t.x,
        a.t.y
    )
}

/// The view milestone V6 (`OPEN-270`): the recording's schema (`citysim-cli shadow --record`, cityview `--sim`).
pub const REC_SCHEMA: &str = "city-simrec/0.1";

/// A recording's first line: one pick's ring for one diary day (`citysim-cli shadow --record`), its world's fixed
/// facts. The lines after it are `RecFrame`s, one a tick, each a delta on the one before (the first in full).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecHeader {
    pub schema: String,
    pub seed: u64,
    pub map_size: [usize; 2],
    pub subject: EntityId,
    pub name: String,
    pub arch: String,
    /// The diary day the file holds: frames `day * TICKS_PER_DAY + 1 ..= (day + 1) * TICKS_PER_DAY`.
    pub day: u64,
    pub tile_m: f64,
    pub north_m: f64,
    pub half: u16,
    /// The walk-matched rate in the recorded world (`walk_matched_rate`), the player's default.
    pub rate: f64,
    /// The El lines' names, by index.
    pub lines: Vec<String>,
    /// The diary minute and the frame (the module doc's rule), said once in the file.
    pub minute: String,
    /// The shadow's view followed the pick (`shadow --record-view`: its ring Full, the camera ring's); else no
    /// view was set and the ring's bodies are at the tier the city gave them.
    #[serde(default)]
    pub view: bool,
}

/// One tick of a recording: the frame's clock and ring in full, its bodies, vehicles and buildings as the ones new
/// or changed since the tick before and the ids gone from the ring, its corpses and events in full, the caption.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecFrame {
    pub tick: Tick,
    pub day: u64,
    pub clock: String,
    pub dark: bool,
    pub season: Season,
    pub ring: Rect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<ViewAgent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gone: Vec<EntityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vehicles: Vec<ViewVehicle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vgone: Vec<EntityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buildings: Vec<ViewBuilding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bgone: Vec<EntityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corpses: Vec<(EntityId, TilePos)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<ViewEvent>,
    pub caption: String,
}

/// `now` against the last tick's `prev` (keyed by id): the new or changed and the ids gone; `prev` becomes `now`.
fn delta<T: Clone + PartialEq>(
    prev: &mut BTreeMap<EntityId, T>,
    now: &[T],
    id: impl Fn(&T) -> EntityId,
) -> (Vec<T>, Vec<EntityId>) {
    let mut changed = vec![];
    let mut next = BTreeMap::new();
    for x in now {
        let k = id(x);
        if prev.get(&k) != Some(x) {
            changed.push(x.clone());
        }
        next.insert(k, x.clone());
    }
    let mut gone: Vec<EntityId> = prev.keys().filter(|k| !next.contains_key(k)).copied().collect();
    gone.sort();
    *prev = next;
    (changed, gone)
}

/// A heading to the whole degree (a recording's bodies: the view draws no finer, and a fraction moved every tick).
fn whole(h: Option<f32>) -> Option<f32> {
    h.map(f32::round)
}

/// The writer's side: each frame as a delta on the last one it wrote (a fresh encoder writes the first in full).
#[derive(Default)]
pub struct RecEncoder {
    agents: BTreeMap<EntityId, ViewAgent>,
    vehicles: BTreeMap<EntityId, ViewVehicle>,
    buildings: BTreeMap<EntityId, ViewBuilding>,
}

impl RecEncoder {
    pub fn encode(&mut self, f: &ViewFrame, caption: String) -> RecFrame {
        let agents: Vec<ViewAgent> =
            f.agents.iter().map(|a| ViewAgent { facing: whole(a.facing), ..a.clone() }).collect();
        let vehicles: Vec<ViewVehicle> =
            f.vehicles.iter().map(|v| ViewVehicle { facing: whole(v.facing), ..v.clone() }).collect();
        let (agents, gone) = delta(&mut self.agents, &agents, |a| a.id);
        let (vehicles, vgone) = delta(&mut self.vehicles, &vehicles, |v| v.id);
        let (buildings, bgone) = delta(&mut self.buildings, &f.buildings, |b| b.id);
        RecFrame {
            tick: f.tick,
            day: f.day,
            clock: f.clock.clone(),
            dark: f.dark,
            season: f.season,
            ring: f.ring,
            agents,
            gone,
            vehicles,
            vgone,
            buildings,
            bgone,
            corpses: f.corpses.clone(),
            events: f.events.clone(),
            caption,
        }
    }
}

/// The player's side: the deltas applied in order give each tick's frame (its bodies, vehicles and buildings by id).
#[derive(Default)]
pub struct RecDecoder {
    agents: BTreeMap<EntityId, ViewAgent>,
    vehicles: BTreeMap<EntityId, ViewVehicle>,
    buildings: BTreeMap<EntityId, ViewBuilding>,
}

impl RecDecoder {
    /// The frame `r` makes of the state so far, and its caption.
    pub fn apply(&mut self, r: RecFrame) -> (ViewFrame, String) {
        for k in &r.gone {
            self.agents.remove(k);
        }
        for k in &r.vgone {
            self.vehicles.remove(k);
        }
        for k in &r.bgone {
            self.buildings.remove(k);
        }
        self.agents.extend(r.agents.into_iter().map(|a| (a.id, a)));
        self.vehicles.extend(r.vehicles.into_iter().map(|v| (v.id, v)));
        self.buildings.extend(r.buildings.into_iter().map(|b| (b.id, b)));
        let f = ViewFrame {
            tick: r.tick,
            day: r.day,
            clock: r.clock,
            dark: r.dark,
            season: r.season,
            ring: r.ring,
            agents: self.agents.values().cloned().collect(),
            vehicles: self.vehicles.values().cloned().collect(),
            buildings: self.buildings.values().cloned().collect(),
            corpses: r.corpses,
            events: r.events,
        };
        (f, r.caption)
    }
}
