//! Pressure pass round 6, V2 (docs/TURF.md § 1-2; OPEN-241, OPEN-217's
//! street): turf you hold, under `[turf]` (absent: territory is Homes alone).
//!
//! **Cells** ([`cell_of`]): the map cut into squares of `cell_tiles`. A
//! gang's **presence** in a cell is summed daily ([`daily`], at the gangs'
//! midnight slot, before the faction brain reads `World::rival_of`): its held
//! Homes and squats there (claims at `CLAIM_HELD`), its members living there,
//! its Hideout and fronts, its bodies' hours on the cell's street or in a
//! venue there and on the corner (sampled hourly, [`hourly`]), and each
//! Statistical member's day (`stat_street_hours`, credited to his corner's
//! cell, else his Home's: the off-screen city holds turf the same way). Over
//! `presence_days` the cell's **status** is held (one gang past `hold_min`
//! and ahead of the next by `hold_margin`), contested (two gangs close) or
//! open; a holder keeps it while at `keep_share` of `hold_min` and unpassed.
//!
//! **The holder change** is one place, [`set_status`]: a `Turf` event
//! ("X hold the blocks around Market#412 now: 9 of their people on the corner
//! most nights") and, for a cell taken from a gang, its `Shock::LostTurf`
//! grudge. V5 (the war, TURF § 3) flips cells through it.
//!
//! **The corner** ([`Cell::corner`]): the street outside the door of the
//! cell's venue (a Bar, Market, Club or NoodleBar) nearest its Homes, else
//! the stoop of its fullest Home. A member with no order's work and no shift
//! holds his corner in the evening ([`corner_hold`], [`corner_plan`]: a
//! GangWork arm, `GoTo(Spot) -> HangOut`); the dealer deals there
//! (`stims::member_site_trip`), the preacher preaches there
//! (`leisure::busiest_spot`), a Street pitch opens in a held cell where the
//! holder's members live and the district's pimp is named among them
//! (`streetwork::openings`, `vice::name_pimps`).
//!
//! **Turf pays** ([`tribute`]): each Home of a held cell pays its holder
//! (`tribute_held` a Home it holds, `tribute_home` any other); protection
//! leans on a business only for the holder of its cell, or for either side of
//! a contested one ([`leans`]). **The neighbour** is the rival
//! (`World::rival_of` reads [`State::rival`]). **Recruits** come from the
//! turf ([`home_holder`], `gang::recruit_gang`). **The residents**: after dark
//! spots and venues in a contested cell are passed over ([`avoids`]); a
//! family with children moves out of a contested cell ([`family_moves`]).
//!
//! The under-18 rule (CLAUDE.md): no protected member counts for presence,
//! holds a corner or is named on a holder line; tribute is paid by adults only
//! (`ages::is_protected`); a family move is the children's protection.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, DistrictId, Gang, GoalKind, Household, Job, Lod, Order, Position,
    Sentence, Shock, TilePos, Wallet,
};
use crate::config::TurfCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::time::{Tick, TICKS_PER_HOUR};
use crate::world::World;

/// A cell's standing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[default]
    Open,
    Held(EntityId),
    /// The leader by presence, then the challenger.
    Contested(EntityId, EntityId),
}

impl Status {
    pub fn holder(self) -> Option<EntityId> {
        match self {
            Status::Held(g) => Some(g),
            _ => None,
        }
    }

    pub fn contested(self) -> Option<(EntityId, EntityId)> {
        match self {
            Status::Contested(a, b) => Some((a, b)),
            _ => None,
        }
    }

    /// `g` holds the cell or is one side of its contest.
    pub fn involves(self, g: EntityId) -> bool {
        match self {
            Status::Open => false,
            Status::Held(h) => h == g,
            Status::Contested(a, b) => a == g || b == g,
        }
    }
}

/// A gang's presence in a cell on one day.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pres {
    pub gang: EntityId,
    pub p: f32,
    /// Distinct members credited there that day.
    pub faces: u16,
    /// Members living there.
    pub residents: u16,
    /// Distinct members on its corner that day.
    #[serde(default)]
    pub corner: u16,
}

/// One cell's state (only cells with a status or a presence are kept).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cell {
    pub status: Status,
    pub since: Tick,
    /// The corner tile and the building it is named by (held or contested).
    pub corner: Option<TilePos>,
    pub landmark: Option<EntityId>,
    /// The window, newest last.
    pub days: VecDeque<Vec<Pres>>,
}

impl Cell {
    /// `gang`'s members living in the cell (the last day's count).
    pub fn residents(&self, gang: EntityId) -> u16 {
        self.days.back().and_then(|d| d.iter().find(|p| p.gang == gang)).map_or(0, |p| p.residents)
    }
}

/// `World.turf`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub cells: BTreeMap<u32, Cell>,
    /// Gang -> its held cells, ascending (derived at the pass).
    pub held: BTreeMap<EntityId, Vec<u32>>,
    /// Gang -> its neighbour, the rival (`World::rival_of`).
    pub rival: BTreeMap<EntityId, EntityId>,
    /// Today's body hours by cell: `(gang, presence)`.
    pub hours: BTreeMap<u32, Vec<(EntityId, f32)>>,
    /// Today's bodies seen by cell: `(gang, member)`, sorted.
    pub faces: BTreeMap<u32, Vec<(EntityId, EntityId)>>,
    /// Today's bodies on the cell's corner: `(gang, member)`, sorted.
    pub cornered: BTreeMap<u32, Vec<(EntityId, EntityId)>>,
    /// Member -> ticks on the corner tonight (planned).
    pub corner_ticks: BTreeMap<EntityId, u16>,
    /// Member -> the day his corner was told; resident -> the night his
    /// contested cell's pass-over was told.
    pub told: BTreeMap<EntityId, u64>,
    pub avoided: BTreeMap<EntityId, u64>,
    /// The first pass seeds silently.
    pub init: bool,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && !self.init
    }
}

pub fn cfg(world: &World) -> Option<&TurfCfg> {
    world.config.turf.as_ref()
}

pub fn on(world: &World) -> bool {
    cfg(world).is_some()
}

fn side(world: &World) -> u32 {
    u32::from(cfg(world).map_or(24, |c| c.cell_tiles.max(1)))
}

fn cols(world: &World) -> u32 {
    let s = side(world);
    (world.map.w() as u32).div_ceil(s)
}

/// The cell of tile `t` (`None` off).
pub fn cell_of(world: &World, t: TilePos) -> Option<u32> {
    if !on(world) {
        return None;
    }
    let s = side(world);
    Some(u32::from(t.y) / s * cols(world) + u32::from(t.x) / s)
}

/// The cell's centre tile.
pub fn centre(world: &World, c: u32) -> TilePos {
    let (s, n) = (side(world), cols(world));
    let (cx, cy) = (c % n, c / n);
    TilePos { x: (cx * s + s / 2).min(u32::from(u16::MAX)) as u16, y: (cy * s + s / 2).min(u32::from(u16::MAX)) as u16 }
}

/// Cells `a` and `b` apart in cells (Chebyshev).
pub fn cell_dist(world: &World, a: u32, b: u32) -> u32 {
    let n = cols(world);
    let (ax, ay, bx, by) = (a % n, a / n, b % n, b / n);
    ax.abs_diff(bx).max(ay.abs_diff(by))
}

/// The eight cells around `c` on the grid.
pub fn neighbours(world: &World, c: u32) -> Vec<u32> {
    let n = cols(world);
    let rows = (world.map.h() as u32).div_ceil(side(world));
    let (cx, cy) = ((c % n) as i64, (c / n) as i64);
    let mut out = Vec::with_capacity(8);
    for dy in -1..=1i64 {
        for dx in -1..=1i64 {
            let (x, y) = (cx + dx, cy + dy);
            if (dx, dy) != (0, 0) && x >= 0 && y >= 0 && x < i64::from(n) && y < i64::from(rows) {
                out.push((y * i64::from(n) + x) as u32);
            }
        }
    }
    out
}

/// The status of the cell at tile `t` (`Open` off or unknown).
pub fn status_at(world: &World, t: TilePos) -> Status {
    cell_of(world, t).and_then(|c| world.turf.cells.get(&c)).map_or(Status::Open, |x| x.status)
}

/// The gang holding the cell of tile `t`.
pub fn holder_at(world: &World, t: TilePos) -> Option<EntityId> {
    status_at(world, t).holder()
}

fn door_of(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

/// `id`'s Home (or squat) door.
fn home_door(world: &World, id: EntityId) -> Option<TilePos> {
    let home = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .or_else(|| world.comp::<crate::components::Squatter>(id).map(|s| s.building))?;
    door_of(world, home)
}

/// The cell of `id`'s Home (or squat).
pub fn home_cell(world: &World, id: EntityId) -> Option<u32> {
    cell_of(world, home_door(world, id)?)
}

/// The gang holding `id`'s Home cell (JoinGang's turf).
pub fn home_holder(world: &World, id: EntityId) -> Option<EntityId> {
    let c = home_cell(world, id)?;
    world.turf.cells.get(&c)?.status.holder().filter(|&g| world.has::<Gang>(g))
}

/// A gang's name.
fn gname(world: &World, g: EntityId) -> String {
    world.comp::<Gang>(g).map_or_else(|| world.name_of(g), |x| x.name.clone())
}

/// `name`'s possessive ("The Wire Hands'", "Ninefold's").
fn poss(name: &str) -> String {
    if name.ends_with('s') {
        format!("{name}'")
    } else {
        format!("{name}'s")
    }
}

/// "the blocks around Market#412" for cell `c`.
fn around(world: &World, c: u32) -> String {
    match world.turf.cells.get(&c).and_then(|x| x.landmark) {
        Some(b) => format!("the blocks around {}", world.name_of(b)),
        None => {
            let t = centre(world, c);
            format!("the blocks at ({}, {})", t.x, t.y)
        }
    }
}

// ---------------------------------------------------------------------------
// The corner
// ---------------------------------------------------------------------------

/// `id`'s corner cell: his Home's when his gang holds it, else the nearest
/// cell his gang holds whose corner is within `corner_reach_tiles` of his
/// Home door.
pub fn corner_cell(world: &World, id: EntityId) -> Option<u32> {
    let c = cfg(world)?;
    let g = world.gang_of(id)?;
    let door = home_door(world, id)?;
    let own = cell_of(world, door)?;
    if world.turf.cells.get(&own).is_some_and(|x| x.status == Status::Held(g)) {
        return Some(own);
    }
    world
        .turf
        .held
        .get(&g)?
        .iter()
        .filter_map(|&k| {
            let t = world.turf.cells.get(&k)?.corner?;
            let d = t.manhattan(door);
            (d <= c.corner_reach_tiles).then_some((d, k))
        })
        .min()
        .map(|(_, k)| k)
}

fn corner_hours(c: &TurfCfg, tod: u16) -> bool {
    if c.corner_from <= c.corner_to {
        (c.corner_from..c.corner_to).contains(&tod)
    } else {
        tod >= c.corner_from || tod < c.corner_to
    }
}

/// The corner `id` would hold now: a member's body (not Statistical, not
/// under 18, not inside, not on shift) in the corner hours, his gang not
/// lying low, under `corner_ticks_day` tonight, with a corner cell.
pub fn corner_hold(world: &World, id: EntityId) -> Option<TilePos> {
    let c = cfg(world)?;
    let g = world.gang_of(id)?;
    let b = world.comp::<Brain>(id)?;
    if b.lod == Lod::Statistical || b.cuffed_by.is_some() {
        return None;
    }
    let tod = world.tick_of_day();
    if !corner_hours(c, tod) {
        return None;
    }
    if world.comp::<Gang>(g).is_none_or(|x| x.order == Order::LieLow) {
        return None;
    }
    // The under-18 rule: a child never stands a gang's corner.
    if world.has::<Sentence>(id) || crate::systems::ages::is_protected(world, id) {
        return None;
    }
    if world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod)) {
        return None;
    }
    if world.turf.corner_ticks.get(&id).is_some_and(|&t| t >= c.corner_ticks_day) {
        return None;
    }
    let k = corner_cell(world, id)?;
    world.turf.cells.get(&k)?.corner
}

/// GangWork's scripted plan for holding the corner: `GoTo(Spot) -> HangOut`
/// at it, when the GangWork target is the member himself and he deals no
/// corner (the dealer's is `stims::corner_plan`).
pub fn corner_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if crate::systems::stims::corner_target(world, id).is_some() {
        return None;
    }
    let t = corner_hold(world, id)?;
    if crate::systems::gang::gang_work_target(world, id).map(|x| x.0) != Some(id) {
        return None;
    }
    world.unwind.insert(
        id,
        crate::living::UnwindPick {
            rung: crate::living::Rung::Free,
            act: ActionKind::HangOut,
            venue: None,
            spot: Some(t),
            score: 0.0,
            stay_in: false,
        },
    );
    let ticks = world.config.leisure.hangout_ticks.min(u64::from(u16::MAX)) as u16;
    let e = world.turf.corner_ticks.entry(id).or_default();
    *e = e.saturating_add(ticks.max(1));
    crate::systems::ownership::jobs2_row(world).turf_corner_holds += 1;
    let step = |action, target| ActionInstance { action, target, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::HangOut, None)];
    Some(Plan { goal: GoalKind::GangWork, target: None, steps, started_tick: world.tick })
}

/// The corners of `gang`'s held cells (the deal spot, the pulpit).
pub fn corners_of(world: &World, gang: EntityId) -> Vec<TilePos> {
    world
        .turf
        .held
        .get(&gang)
        .map(|v| v.iter().filter_map(|k| world.turf.cells.get(k).and_then(|x| x.corner)).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Presence: the hour and the day
// ---------------------------------------------------------------------------

/// Hourly (from `gang::run`): each member's body out on a cell's street or
/// in a venue there credits his gang `w_street_hour`, on the cell's corner
/// `w_corner_hour`; a Full body on his own corner is told once a night.
pub fn hourly(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let reach = world.config.jobs2.pitch_reach;
    let today = world.day();
    let mut tells: Vec<(EntityId, EntityId, u32)> = Vec::new();
    let mut corner_hours_n = 0u32;
    // (cell, gang) -> the members on its corner this hour.
    let mut on: BTreeMap<(u32, EntityId), u16> = BTreeMap::new();
    for g in world.gangs() {
        let members = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
        for m in members {
            let Some(b) = world.comp::<Brain>(m) else { continue };
            if b.lod == Lod::Statistical || world.has::<Sentence>(m) || crate::systems::ages::is_protected(world, m) {
                continue;
            }
            let Some(p) = world.comp::<Position>(m) else { continue };
            let counts = match p.building {
                None => true,
                Some(bd) => world
                    .comp::<Building>(bd)
                    .is_some_and(|x| x.venue.is_some() || matches!(x.kind, BuildingKind::Bar | BuildingKind::Hideout)),
            };
            if !counts {
                continue;
            }
            let tile = p.tile;
            let outdoors = p.building.is_none();
            let Some(k) = cell_of(world, tile) else { continue };
            let on_corner =
                outdoors && world.turf.cells.get(&k).and_then(|x| x.corner).is_some_and(|t| t.manhattan(tile) <= reach);
            let w = if on_corner { c.w_corner_hour } else { c.w_street_hour };
            let v = world.turf.hours.entry(k).or_default();
            match v.iter_mut().find(|(x, _)| *x == g) {
                Some(e) => e.1 += w,
                None => v.push((g, w)),
            }
            let f = world.turf.faces.entry(k).or_default();
            if let Err(i) = f.binary_search(&(g, m)) {
                f.insert(i, (g, m));
            }
            if on_corner {
                corner_hours_n += 1;
                *on.entry((k, g)).or_default() += 1;
                let f = world.turf.cornered.entry(k).or_default();
                if let Err(i) = f.binary_search(&(g, m)) {
                    f.insert(i, (g, m));
                }
                // Told once a night: a Full body holding (or dealing) this
                // corner in the corner hours, not one walking past it.
                let corner = world.turf.cells.get(&k).and_then(|x| x.corner);
                let holding = corner_hours(&c, world.tick_of_day())
                    && world.unwind.get(&m).is_some_and(|u| {
                        u.spot.is_some() && u.spot == corner && matches!(u.act, ActionKind::HangOut | ActionKind::Deal)
                    });
                let held = world.turf.cells.get(&k).is_some_and(|x| x.status == Status::Held(g));
                let full = world.comp::<Brain>(m).is_some_and(|b| b.lod == Lod::Full);
                if holding && held && full && world.turf.told.get(&m) != Some(&today) {
                    tells.push((m, g, k));
                }
            }
        }
    }
    if corner_hours_n > 0 {
        crate::systems::ownership::jobs2_row(world).turf_corner_hours += corner_hours_n;
    }
    for (m, g, k) in tells {
        world.turf.told.insert(m, today);
        let mates = on.get(&(k, g)).copied().unwrap_or(1).saturating_sub(1);
        let by = match world.turf.cells.get(&k).and_then(|x| x.landmark) {
            Some(b) => format!("by {}", world.name_of(b)),
            None => around(world, k),
        };
        let with = match mates {
            0 => "alone on it".to_string(),
            1 => "one more of theirs on it".to_string(),
            n => format!("{n} more of theirs on it"),
        };
        let text = format!("{} holds {} corner {by} ({with})", world.name_of(m), poss(&gname(world, g)));
        world.push_event(EventKind::Turf, &[m, g], text);
    }
}

/// The cell index of every standing building, by door (the day's pass).
fn cell_buildings(world: &World) -> BTreeMap<u32, Vec<EntityId>> {
    let mut out: BTreeMap<u32, Vec<EntityId>> = BTreeMap::new();
    // scan-ok: daily: the turf pass
    for list in world.buildings_by_kind().values() {
        for &b in list {
            let Some(bd) = world.comp::<Building>(b) else { continue };
            if bd.demolished || bd.kind == BuildingKind::Lot {
                continue;
            }
            if let Some(k) = cell_of(world, bd.door) {
                out.entry(k).or_default().push(b);
            }
        }
    }
    for v in out.values_mut() {
        v.sort_unstable();
    }
    out
}

/// The cell's corner and its landmark: the outside door of its venue (a Bar,
/// Market, Club or NoodleBar) nearest the mean of its Home doors, else the
/// stoop of its fullest Home.
fn corner_in(world: &World, bs: &[EntityId]) -> Option<(TilePos, EntityId)> {
    let homes: Vec<TilePos> = bs
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).filter(|x| x.kind == BuildingKind::Home && !x.derelict))
        .map(|x| x.door)
        .collect();
    let mid = if homes.is_empty() {
        None
    } else {
        let n = homes.len() as u32;
        let (sx, sy) = homes.iter().fold((0u32, 0u32), |a, t| (a.0 + u32::from(t.x), a.1 + u32::from(t.y)));
        Some(TilePos { x: (sx / n) as u16, y: (sy / n) as u16 })
    };
    let venue = bs
        .iter()
        .filter_map(|&b| {
            let x = world.comp::<Building>(b)?;
            let ok = matches!(
                x.kind,
                BuildingKind::Bar | BuildingKind::Market | BuildingKind::Club | BuildingKind::NoodleBar
            ) && !x.derelict;
            ok.then(|| (mid.map_or(0, |m| x.door.manhattan(m)), b, world.outside_door(x)))
        })
        .min();
    if let Some((_, b, t)) = venue {
        return Some((t, b));
    }
    bs.iter()
        .filter_map(|&b| {
            let x = world.comp::<Building>(b).filter(|x| x.kind == BuildingKind::Home && !x.derelict)?;
            Some((std::cmp::Reverse(world.residents_of(b).len()), b, world.outside_door(x)))
        })
        .min()
        .map(|(_, b, t)| (t, b))
}

/// Midnight (the gangs' slot, from `gang::run` before the brains): the day's
/// presence into each cell's window, the statuses and their changes told,
/// the corners, the held lists, the neighbours, the families' moves.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let _t = crate::prof::scope!("turf::daily");
    let now = world.tick;
    let held_min = crate::systems::gang::CLAIM_HELD;
    let index = cell_buildings(world);
    // The day's presence: cell -> gang -> (p, faces, residents).
    let mut day: BTreeMap<u32, BTreeMap<EntityId, (f32, u16)>> = BTreeMap::new();
    let mut seen: BTreeSet<(u32, EntityId, EntityId)> = BTreeSet::new();
    let add = |day: &mut BTreeMap<u32, BTreeMap<EntityId, (f32, u16)>>, k: u32, g: EntityId, p: f32, res: u16| {
        let e = day.entry(k).or_default().entry(g).or_insert((0.0, 0));
        e.0 += p;
        e.1 += res;
    };
    for (&k, bs) in &index {
        for &b in bs {
            let Some(bd) = world.comp::<Building>(b) else { continue };
            if let Some(cl) = bd.claim.filter(|cl| cl.count >= held_min && world.has::<Gang>(cl.gang)) {
                if bd.kind == BuildingKind::Home || bd.derelict {
                    add(&mut day, k, cl.gang, c.w_held_home, 0);
                }
            }
            if let Some(g) = bd.venue.as_ref().and_then(|v| v.front_of).filter(|&g| world.has::<Gang>(g)) {
                add(&mut day, k, g, c.w_front, 0);
            }
        }
    }
    for g in world.gangs() {
        let Some((hideout, members)) = world.comp::<Gang>(g).map(|x| (x.hideout, x.members.clone())) else { continue };
        if let Some(k) = door_of(world, hideout).and_then(|t| cell_of(world, t)) {
            add(&mut day, k, g, c.w_hideout, 0);
        }
        for m in members {
            if !crate::systems::law::living(world, m)
                || world.has::<Sentence>(m)
                || crate::systems::ages::is_protected(world, m)
            {
                continue;
            }
            let Some(home) = home_cell(world, m) else { continue };
            add(&mut day, home, g, c.w_resident, 1);
            seen.insert((home, g, m));
            if world.comp::<Brain>(m).is_some_and(|b| b.lod == Lod::Statistical) {
                let k = corner_cell(world, m).unwrap_or(home);
                add(&mut day, k, g, c.stat_street_hours, 0);
                seen.insert((k, g, m));
            }
        }
    }
    let hours = std::mem::take(&mut world.turf.hours);
    for (k, v) in hours {
        for (g, p) in v {
            if world.has::<Gang>(g) {
                add(&mut day, k, g, p, 0);
            }
        }
    }
    let faces = std::mem::take(&mut world.turf.faces);
    for (k, v) in faces {
        for (g, m) in v {
            seen.insert((k, g, m));
        }
    }
    let mut face_n: BTreeMap<(u32, EntityId), u16> = BTreeMap::new();
    for &(k, g, _) in &seen {
        *face_n.entry((k, g)).or_default() += 1;
    }
    let mut corner_n: BTreeMap<(u32, EntityId), u16> = BTreeMap::new();
    for (k, v) in std::mem::take(&mut world.turf.cornered) {
        for (g, _) in v {
            *corner_n.entry((k, g)).or_default() += 1;
        }
    }
    world.turf.corner_ticks.clear();
    let today = world.day();
    world.turf.avoided.retain(|_, d| *d + 1 >= today);
    world.turf.told.retain(|_, d| *d + 1 >= today);
    // Every cell with a presence today or a window to roll.
    let keys: BTreeSet<u32> = day.keys().copied().chain(world.turf.cells.keys().copied()).collect();
    let init = world.turf.init;
    let mut changes: Vec<(u32, Status, Status)> = Vec::new();
    for k in keys {
        let today_v: Vec<Pres> = day
            .get(&k)
            .map(|m| {
                m.iter()
                    .map(|(&g, &(p, res))| Pres {
                        gang: g,
                        p,
                        faces: face_n.get(&(k, g)).copied().unwrap_or(0),
                        residents: res,
                        corner: corner_n.get(&(k, g)).copied().unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let cell = world.turf.cells.entry(k).or_default();
        cell.days.push_back(today_v);
        while cell.days.len() > usize::from(c.presence_days.max(1)) {
            cell.days.pop_front();
        }
        let mut sum: BTreeMap<EntityId, f32> = BTreeMap::new();
        for d in &cell.days {
            for p in d {
                *sum.entry(p.gang).or_default() += p.p;
            }
        }
        let mut ranked: Vec<(EntityId, f32)> = sum.into_iter().filter(|&(g, _)| world.has::<Gang>(g)).collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let old = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status);
        let new = judge(&c, old, &ranked);
        if new != old {
            changes.push((k, old, new));
        }
        let empty = world.turf.cells.get(&k).is_some_and(|x| x.days.iter().all(Vec::is_empty));
        if empty && new == Status::Open && old == Status::Open {
            world.turf.cells.remove(&k);
        }
    }
    // The corners of held and contested cells (and of those changing now,
    // whose line names the corner).
    let live: BTreeSet<u32> = world
        .turf
        .cells
        .iter()
        .filter(|(_, x)| x.status != Status::Open)
        .map(|(&k, _)| k)
        .chain(changes.iter().map(|&(k, _, _)| k))
        .collect();
    for k in live {
        let pick = index.get(&k).and_then(|bs| corner_in(world, bs));
        if let Some(x) = world.turf.cells.get_mut(&k) {
            x.corner = pick.map(|p| p.0);
            x.landmark = pick.map(|p| p.1);
        }
    }
    for (k, old, new) in changes {
        set_status(world, k, old, new, init, now);
    }
    world.turf.init = true;
    reindex(world);
    family_moves(world, &c, &index);
    let held_n: u32 = world.turf.held.values().map(|v| v.len() as u32).sum();
    let contested_n = world.turf.cells.values().filter(|x| matches!(x.status, Status::Contested(..))).count() as u32;
    let gangs_n = world.turf.held.values().filter(|v| !v.is_empty()).count() as u32;
    let row = crate::systems::ownership::jobs2_row(world);
    row.turf_cells_held = held_n;
    row.turf_cells_contested = contested_n;
    row.turf_gangs_holding = gangs_n;
}

/// A cell's next status from `old` and the window's presence (best first).
fn judge(c: &TurfCfg, old: Status, ranked: &[(EntityId, f32)]) -> Status {
    let p_of = |g: EntityId| ranked.iter().find(|x| x.0 == g).map_or(0.0, |x| x.1);
    let keep = c.keep_share * c.hold_min;
    if let Status::Held(h) = old {
        let ph = p_of(h);
        if ph >= keep {
            // The best challenger.
            let ch = ranked.iter().find(|x| x.0 != h).copied();
            return match ch {
                Some((q, pq)) if pq >= c.hold_min && pq > ph * (1.0 + c.hold_margin) => Status::Held(q),
                Some((q, pq)) if pq >= keep && pq * (1.0 + c.hold_margin) > ph => Status::Contested(h, q),
                _ => Status::Held(h),
            };
        }
    }
    let Some(&(g1, p1)) = ranked.first() else { return Status::Open };
    if p1 < c.hold_min {
        return Status::Open;
    }
    match ranked.get(1) {
        Some(&(g2, p2)) if p2 >= keep && p2 * (1.0 + c.hold_margin) > p1 => Status::Contested(g1, g2),
        _ => Status::Held(g1),
    }
}

/// The holder change, one place (V5 flips cells through it): the status
/// written and, with `tell`, the `Turf` line and the loser's `LostTurf`.
pub fn set_status(world: &mut World, k: u32, old: Status, new: Status, tell: bool, now: Tick) {
    if let Some(x) = world.turf.cells.get_mut(&k) {
        x.status = new;
        x.since = now;
    } else if new != Status::Open {
        world.turf.cells.insert(k, Cell { status: new, since: now, ..Default::default() });
    }
    if !tell {
        return;
    }
    let at = around(world, k);
    let landmark = world.turf.cells.get(&k).and_then(|x| x.landmark);
    // On the corner most nights: the window's median of the nights' counts.
    let faces = |world: &World, g: EntityId| -> u16 {
        let Some(x) = world.turf.cells.get(&k) else { return 0 };
        let mut v: Vec<u16> = x.days.iter().map(|d| d.iter().find(|p| p.gang == g).map_or(0, |p| p.corner)).collect();
        v.sort_unstable();
        v.get(v.len() / 2).copied().unwrap_or(0)
    };
    let about = |world: &World, g: EntityId| -> u16 {
        world.turf.cells.get(&k).map_or(0, |x| {
            x.days.iter().filter_map(|d| d.iter().find(|p| p.gang == g)).map(|p| p.faces).max().unwrap_or(0)
        })
    };
    // The members named: those living there, then the lowest ids (adults only).
    let named = |world: &World, g: EntityId| -> Vec<EntityId> {
        let members = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
        let mut v: Vec<EntityId> = members
            .iter()
            .copied()
            .filter(|&m| home_cell(world, m) == Some(k) && !crate::systems::ages::is_protected(world, m))
            .take(3)
            .collect();
        if v.is_empty() {
            v.extend(
                members
                    .iter()
                    .copied()
                    .filter(|&m| corner_cell(world, m) == Some(k) && !crate::systems::ages::is_protected(world, m))
                    .take(3),
            );
        }
        v
    };
    let row_change = |world: &mut World| crate::systems::ownership::jobs2_row(world).turf_changes += 1;
    match (old, new) {
        (_, Status::Held(y)) if old.holder() != Some(y) => {
            let n = faces(world, y);
            let lost = match old {
                Status::Held(x) => Some(x),
                Status::Contested(a, b) => [a, b].into_iter().find(|&g| g != y),
                Status::Open => None,
            };
            let from = lost.map_or(String::new(), |x| format!(", taken from {}", gname(world, x)));
            let m = about(world, y);
            let text = if n > 0 {
                format!("{} hold {at} now{from}: {n} of their people on the corner most nights", gname(world, y))
            } else if m > 0 {
                format!(
                    "{} hold {at} now{from}: {m} of their people about the blocks, nobody on the corner yet",
                    gname(world, y)
                )
            } else {
                format!("{} hold {at} now{from}: their Hideout's blocks, nobody on the corner yet", gname(world, y))
            };
            let mut actors = vec![y];
            actors.extend(named(world, y));
            if let Some(x) = lost {
                actors.push(x);
                actors.extend(named(world, x));
            }
            actors.extend(landmark);
            world.push_event(EventKind::Turf, &actors, text);
            row_change(world);
            if let Some(x) = lost.filter(|&x| world.has::<Gang>(x)) {
                crate::systems::gang::push_shock(world, x, Shock::LostTurf { by: Some(y) });
                crate::systems::ownership::jobs2_row(world).turf_lost += 1;
            }
        }
        (Status::Held(x), Status::Contested(a, b)) => {
            let y = if a == x { b } else { a };
            let text = format!(
                "{} are on {} corner: {at} are contested ({} of theirs about the blocks, {} of {})",
                gname(world, y),
                poss(&gname(world, x)),
                about(world, y),
                about(world, x),
                poss(&gname(world, x))
            );
            let mut actors = vec![y];
            actors.extend(named(world, y));
            actors.push(x);
            actors.extend(named(world, x));
            actors.extend(landmark);
            world.push_event(EventKind::Turf, &actors, text);
            crate::systems::ownership::jobs2_row(world).turf_contests += 1;
        }
        (Status::Held(x), Status::Open) => {
            let text = format!("{} no longer hold {at}: nobody of theirs on the corner", gname(world, x));
            let mut actors = vec![x];
            actors.extend(named(world, x));
            actors.extend(landmark);
            world.push_event(EventKind::Turf, &actors, text);
            row_change(world);
        }
        _ => {}
    }
}

/// The held lists and the neighbours from the statuses.
fn reindex(world: &mut World) {
    let mut held: BTreeMap<EntityId, Vec<u32>> = BTreeMap::new();
    for (&k, x) in &world.turf.cells {
        if let Status::Held(g) = x.status {
            held.entry(g).or_default().push(k);
        }
    }
    // The neighbour: the gang holding the most cells bordering ours (a
    // contest counts its other side), ties by its presence, then the id.
    let mut rival: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    for (&g, cells) in &held {
        let mut border: BTreeMap<EntityId, (u32, f32)> = BTreeMap::new();
        let mine: BTreeSet<u32> = cells.iter().copied().collect();
        let mut near: BTreeSet<u32> = BTreeSet::new();
        for &k in cells {
            for n in neighbours(world, k) {
                if !mine.contains(&n) {
                    near.insert(n);
                }
            }
        }
        for n in near {
            let Some(x) = world.turf.cells.get(&n) else { continue };
            let other = match x.status {
                Status::Held(q) if q != g => Some(q),
                Status::Contested(a, b) if a == g || b == g => Some(if a == g { b } else { a }),
                Status::Contested(a, _) => Some(a),
                _ => None,
            };
            if let Some(q) = other.filter(|&q| q != g && world.has::<Gang>(q)) {
                let e = border.entry(q).or_default();
                e.0 += 1;
            }
        }
        for (q, e) in border.iter_mut() {
            e.1 = held.get(q).map_or(0, |v| v.len()) as f32;
        }
        if let Some((q, _)) =
            border.into_iter().max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(a.1 .1.total_cmp(&b.1 .1)).then(b.0.cmp(&a.0)))
        {
            rival.insert(g, q);
        }
    }
    world.turf.held = held;
    world.turf.rival = rival;
}

/// `World::rival_of`'s turf answer: the neighbour, while both stand.
pub fn rival(world: &World, gang: EntityId) -> Option<EntityId> {
    world.turf.rival.get(&gang).copied().filter(|&r| r != gang && world.has::<Gang>(r))
}

// ---------------------------------------------------------------------------
// Turf that pays
// ---------------------------------------------------------------------------

/// Daily (from `gang::daily_economy`, in place of the per-Home tribute):
/// each Home of a held cell pays its holder, `tribute_held` a Home it holds
/// (claim at `CLAIM_HELD`), `tribute_home` any other, from its adults'
/// purses (`Flow::Coerced`); a held Home outside any held cell pays its
/// claimant `tribute_held` as before. Nobody under 18 pays.
pub fn tribute(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let held_min = crate::systems::gang::CLAIM_HELD;
    let mut dues: Vec<(EntityId, EntityId, i64)> = Vec::new();
    // scan-ok: daily: the tribute pass over the Homes
    for &h in world.buildings_of_kind(BuildingKind::Home) {
        let Some(bd) = world.comp::<Building>(h) else { continue };
        if bd.demolished || bd.derelict {
            continue;
        }
        let claim = bd.claim.filter(|cl| cl.count >= held_min && world.has::<Gang>(cl.gang)).map(|cl| cl.gang);
        let holder = holder_at(world, bd.door).filter(|&g| world.has::<Gang>(g));
        let (to, owed) = match (holder, claim) {
            (Some(g), Some(cg)) if g == cg => (g, c.tribute_held),
            (Some(g), _) => (g, c.tribute_home),
            (None, Some(cg)) => {
                // Contested or open: the claimant keeps his Home's tribute.
                (cg, c.tribute_held)
            }
            (None, None) => continue,
        };
        if owed > 0 {
            dues.push((h, to, owed));
        }
    }
    let mut total = 0i64;
    let mut by_gang: BTreeMap<EntityId, i64> = BTreeMap::new();
    for (h, g, mut owed) in dues {
        let payers: Vec<EntityId> = world
            .residents_of(h)
            .iter()
            .copied()
            .filter(|&r| !crate::systems::ages::is_protected(world, r))
            .filter(|&r| world.gang_of(r) != Some(g))
            .collect();
        let start = owed;
        for r in payers {
            if owed == 0 {
                break;
            }
            let coins = world.comp::<Wallet>(r).map_or(0, |w| w.coins);
            let pay = owed.min(coins.max(0));
            if pay > 0 {
                owed -= crate::systems::ownership::pay(
                    world,
                    Some(r),
                    Some(g),
                    pay,
                    crate::systems::ownership::Flow::Coerced,
                );
            }
        }
        let paid = start - owed;
        if paid > 0 {
            *by_gang.entry(g).or_default() += paid;
            total += paid;
        }
    }
    for (g, paid) in by_gang {
        // L2 L20: the week's tribute, for the leader's Collect.
        crate::systems::leisure::note_tribute(world, g, paid);
    }
    crate::systems::ownership::jobs2_row(world).turf_tribute += total;
}

/// Protection by cell: `gang` leans on business `b` when it holds `b`'s
/// cell or is one side of its contest (off: always).
pub fn leans(world: &World, gang: EntityId, b: EntityId) -> bool {
    if !on(world) {
        return true;
    }
    door_of(world, b).is_some_and(|t| status_at(world, t).involves(gang))
}

/// `b` stands in a cell `gang` contests (a second gang may squeeze it).
pub fn contested_by(world: &World, gang: EntityId, b: EntityId) -> bool {
    door_of(world, b)
        .is_some_and(|t| matches!(status_at(world, t), Status::Contested(..)) && status_at(world, t).involves(gang))
}

/// The districts of `gang`'s held and contested cells, ascending.
pub fn ground(world: &World, gang: EntityId) -> Vec<DistrictId> {
    let mut out: Vec<DistrictId> = world
        .turf
        .cells
        .iter()
        .filter(|(_, x)| x.status.involves(gang))
        .map(|(&k, _)| world.district_of(centre(world, k)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// GangWork's Expand: a door in a cell another gang holds is that gang's
/// (the holder protects his own; V14's `Block#478` paid three gangs).
pub fn open_to(world: &World, gang: EntityId, door: TilePos) -> bool {
    holder_at(world, door).is_none_or(|g| g == gang || !world.has::<Gang>(g))
}

// ---------------------------------------------------------------------------
// The residents read the colours
// ---------------------------------------------------------------------------

/// After dark, `id` (no side of it) passes over tile `t` in a contested cell.
pub fn avoids(world: &World, id: EntityId, t: TilePos) -> bool {
    if !on(world) || !world.is_dark() {
        return false;
    }
    match status_at(world, t) {
        Status::Contested(a, b) => world.gang_of(id).is_none_or(|g| g != a && g != b),
        _ => false,
    }
}

/// A body's evening pick made after dark (`leisure::unwind_plan`,
/// `hangout_plan`): when a spot of its district lies in a contested cell
/// within `avoid_tiles` of where it sets out (passed over by `avoids`), the
/// pass-over is told once a night.
pub fn note_avoided(world: &mut World, id: EntityId, from: TilePos) {
    let Some(c) = cfg(world) else { return };
    let r = c.avoid_tiles;
    if !world.is_dark() || world.comp::<Brain>(id).is_none_or(|b| b.lod != Lod::Full) {
        return;
    }
    let night = world.tick.saturating_sub(12 * TICKS_PER_HOUR) / crate::time::TICKS_PER_DAY;
    if world.turf.avoided.get(&id) == Some(&night) {
        return;
    }
    let d = world.district_of(from);
    let hit = world.spots.get(d.index()).and_then(|v| {
        v.iter()
            .map(|s| s.tile)
            .filter(|&t| t.manhattan(from) <= r && avoids(world, id, t))
            .min_by_key(|t| (t.manhattan(from), t.x, t.y))
    });
    let Some(t) = hit else { return };
    let Some(k) = cell_of(world, t) else { return };
    let Status::Contested(a, b) = status_at(world, t) else { return };
    world.turf.avoided.insert(id, night);
    let text = format!(
        "{} kept off {} after dark ({} and {} at odds there)",
        world.name_of(id),
        around(world, k),
        gname(world, a),
        gname(world, b)
    );
    world.push_event(EventKind::Turf, &[id], text);
    crate::systems::ownership::jobs2_row(world).turf_avoided += 1;
}

/// Daily: up to `family_moves_per_1000` per 1,000 residents, a household
/// with children in a contested cell moves into the nearest Home with room
/// it can pay outside any contested cell within `family_move_tiles` (the
/// adults told, the children with them). The children's protection.
fn family_moves(world: &mut World, c: &TurfCfg, index: &BTreeMap<u32, Vec<EntityId>>) {
    let pop = world.citizens().len() as f32;
    let limit = (c.family_moves_per_1000 * pop / 1000.0).round() as usize;
    if limit == 0 {
        return;
    }
    let contested: Vec<u32> =
        world.turf.cells.iter().filter(|(_, x)| matches!(x.status, Status::Contested(..))).map(|(&k, _)| k).collect();
    let mut moved = 0usize;
    for k in contested {
        let homes: Vec<EntityId> = index
            .get(&k)
            .map(|v| {
                v.iter()
                    .copied()
                    .filter(|&b| world.comp::<Building>(b).is_some_and(|x| x.kind == BuildingKind::Home))
                    .collect()
            })
            .unwrap_or_default();
        for h in homes {
            if moved >= limit {
                return;
            }
            let residents = world.residents_of(h).to_vec();
            // The household: an adult with children at this Home (not a member
            // of either side: they stay for the fight).
            let Status::Contested(ga, gb) = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status) else {
                continue;
            };
            let head = residents.iter().copied().find(|&a| {
                world.has::<Brain>(a)
                    && !crate::systems::ages::is_protected(world, a)
                    && !world.has::<Sentence>(a)
                    && !crate::systems::life::housed_now(world, a)
                    && world.gang_of(a).is_none_or(|g| g != ga && g != gb)
                    && crate::systems::demography::children_of_agent(world, a).iter().any(|&ch| {
                        world.has::<crate::components::Child>(ch)
                            && world.comp::<Household>(ch).and_then(|x| x.home) == Some(h)
                    })
            });
            let Some(a) = head else { continue };
            let spouse = world
                .spouse_of(a)
                .filter(|&s| world.comp::<Household>(s).and_then(|x| x.home) == Some(h) && world.has::<Brain>(s));
            let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
                .into_iter()
                .filter(|&ch| {
                    world.has::<crate::components::Child>(ch)
                        && world.comp::<Household>(ch).and_then(|x| x.home) == Some(h)
                })
                .collect();
            let need = 1 + usize::from(spouse.is_some()) + kids.len();
            let purse = world.comp::<Wallet>(a).map_or(0, |w| w.coins)
                + spouse.and_then(|s| world.comp::<Wallet>(s)).map_or(0, |w| w.coins);
            let Some(door) = door_of(world, h) else { continue };
            let r = c.family_move_tiles;
            let pick = world.building_grid().nearest(door, 0, |b| {
                if b == h {
                    return None;
                }
                let x = world.comp::<Building>(b)?;
                let d = x.door.manhattan(door);
                let ok = x.kind == BuildingKind::Home
                    && !x.demolished
                    && !x.derelict
                    && d <= r
                    && crate::systems::company::open_to_all(world, b)
                    && world.residents_of(b).len() + need <= usize::from(x.capacity)
                    && !matches!(status_at(world, x.door), Status::Contested(..))
                    && purse >= crate::systems::ownership::rent_for(world, b);
                ok.then_some((d, b.index))
            });
            let Some((_, _, to)) = pick.filter(|&(d, _, _)| d <= r) else { continue };
            let at = around(world, k);
            let (an, bn, place) = (gname(world, ga), gname(world, gb), world.name_of(to));
            let mover = world.name_of(a);
            for m in std::iter::once(a).chain(spouse) {
                world.set_home(m, Some(to));
                if let Some(x) = world.comp_mut::<Household>(m) {
                    x.rent_due = x.rent_due.min(0.0);
                }
                let name = world.name_of(m);
                let text = if m == a {
                    format!("{name} moved the children out of {at} ({an} and {bn} at odds there), into {place}")
                } else {
                    format!("{name} moved with {mover} and the children, into {place}")
                };
                world.push_event(EventKind::Housed, &[m, to], text);
            }
            let tdoor = door_of(world, to).unwrap_or(door);
            let tick = world.tick;
            for ch in kids {
                world.set_home(ch, Some(to));
                world.remove_from_building(ch);
                if let Some(p) = world.comp_mut::<Position>(ch) {
                    p.tile = tdoor;
                    p.building = None;
                    p.entered = tick;
                }
                world.enter_building(ch, to);
            }
            moved += 1;
            crate::systems::ownership::jobs2_row(world).turf_family_moves += 1;
        }
    }
}
