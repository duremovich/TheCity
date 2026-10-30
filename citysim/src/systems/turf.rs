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
use crate::config::{TurfCfg, WarCfg};
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
    /// V5 (the war): the heat on the cell for its holder, by provoking gang.
    pub heat: Vec<(EntityId, f32)>,
    /// V5: the fight scheduled on the corner (an answer, a front, a retake).
    pub fight: Option<Fight>,
    /// V5: won in a corner fight (or settled by a truce): held by the gang
    /// until the tick whatever the presence says.
    pub won: Option<(EntityId, Tick)>,
    /// V5: taken from the gang, who may come back for it until the tick.
    pub lost: Option<(EntityId, Tick)>,
    /// V5: the law's attention on the cell (fights and deaths there).
    pub law_heat: f32,
    /// V5: the law's crackdown on the cell runs to this tick.
    pub crackdown_until: Tick,
}

/// V5: why a corner fight is scheduled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FightKind {
    /// The holder pushes the trespassers off (past `answer_heat`).
    Answer,
    /// The two sides of a contested cell.
    Front,
    /// The gang that lost the cell comes back for it.
    Retake,
}

/// V5: a corner fight scheduled at `due`; `side` stands on the cell (its
/// holder, or the leader of a contest), `by` comes at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fight {
    pub kind: FightKind,
    pub side: EntityId,
    pub by: EntityId,
    pub opened: Tick,
    pub due: Tick,
}

/// V5: a war between two gangs (the lower id first).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct War {
    pub started: Tick,
    pub last: Tick,
    /// The cell it began over.
    pub cell: u32,
    pub fights: u32,
    pub flips: u32,
    pub deaths: u32,
    /// Each side's fit adults at the start.
    pub size: [u16; 2],
    /// Each side's losses (dead, jailed, gone) since, by tick.
    pub losses: [VecDeque<Tick>; 2],
    /// When each side was spent.
    pub exhausted: [Option<Tick>; 2],
    /// Each side's members who stood in its corner fights (their later
    /// deaths, jailings and leavings are the war's losses).
    pub men: [BTreeSet<EntityId>; 2],
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
    /// V5: the wars, by `(lower, higher)` gang id.
    pub wars: BTreeMap<(EntityId, EntityId), War>,
    /// V5: truces, `(lower, higher)` -> the tick they end.
    pub truces: BTreeMap<(EntityId, EntityId), Tick>,
    /// V5: a spent gang lies low until the tick.
    pub lie_low: BTreeMap<EntityId, Tick>,
    /// V5: the corner fights scheduled, `(cell, side, by)` (the muster's index).
    pub open: Vec<(u32, EntityId, EntityId)>,
    /// V5: member -> the night he sprayed his mark; cell -> the night its
    /// trespass was told.
    pub tagged: BTreeMap<EntityId, u64>,
    pub trespass_told: BTreeMap<u32, u64>,
    /// V5: a corner fight resolving (its blows provoke nothing).
    #[serde(skip)]
    pub fighting: bool,
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
            // V5 (V2's reads-wrong: "the blocks at (588, 732)"): a cell with
            // no building of its own is named by the nearest one.
            let t = centre(world, c);
            let near = world.building_grid().nearest(t, 0, |b| {
                let x = world.comp::<Building>(b)?;
                (!x.demolished && x.kind != BuildingKind::Lot).then(|| (x.door.manhattan(t), b.index))
            });
            match near {
                Some((_, _, b)) => format!("the blocks near {}", world.name_of(b)),
                None => format!("the blocks at ({}, {})", t.x, t.y),
            }
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
    if world.comp::<Gang>(g).is_none_or(|x| x.order == Order::LieLow) {
        return None;
    }
    // The under-18 rule: a child never stands a gang's corner, nor is he
    // mustered to one (V5).
    if world.has::<Sentence>(id) || crate::systems::ages::is_protected(world, id) {
        return None;
    }
    let tod = world.tick_of_day();
    if world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod)) {
        return None;
    }
    // V5: a muster outranks the evening's corner (any hour, no cap).
    if let Some(t) = muster_corner(world, id) {
        return Some(t);
    }
    if !corner_hours(c, tod) {
        return None;
    }
    if world.turf.corner_ticks.get(&id).is_some_and(|&t| t >= c.corner_ticks_day) {
        return None;
    }
    let k = corner_cell(world, id)?;
    let x = world.turf.cells.get(&k)?;
    // V5: nobody holds a corner the law cracks down on.
    if x.crackdown_until > world.tick {
        return None;
    }
    x.corner
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
    // V5: (cell, gang) -> its first body about the cell this hour (the eyes
    // a provocation needs); a member on another gang's held corner.
    let mut eyes: BTreeMap<(u32, EntityId), EntityId> = BTreeMap::new();
    let mut trespass: Vec<(u32, EntityId, EntityId)> = Vec::new();
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
            eyes.entry((k, g)).or_insert(m);
            // V5: another gang's held corner in the corner hours (the evening
            // a gang stands its corner; a walk past it by day is no trespass).
            if on_corner
                && corner_hours(&c, world.tick_of_day())
                && world.turf.cells.get(&k).and_then(|x| x.status.holder()).is_some_and(|h| h != g)
            {
                trespass.push((k, g, m));
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
    war_hourly(world, &trespass, &eyes);
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
        // V5: a cell won in a corner fight (or settled by a truce) holds.
        let new = world
            .turf
            .cells
            .get(&k)
            .and_then(|x| x.won)
            .filter(|&(w, until)| until > now && world.has::<Gang>(w))
            .map_or(new, |(w, _)| Status::Held(w));
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
    // V5: the war's day (the heat cooled, the off-screen rolls, exhaustion,
    // truces, the law's crackdowns).
    war_daily(world);
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

// ---------------------------------------------------------------------------
// V5: the war (TURF § 3)
// ---------------------------------------------------------------------------
//
// **Provocation** ([`provoke_at`], the hourly corner in [`war_hourly`], the
// off-screen night in [`war_daily`]): another gang's act on a held cell (a
// body on its corner, his mark sprayed by it, a deal, a shakedown, a beating
// of a holder's member there) raises the cell's heat for the holder, seen by
// a holder's adult body there (a Statistical act is seen off screen). Past
// `answer_heat` the holder **answers** ([`open_fight`]): the leader sends the
// cell's and the neighbouring cells' members to the corner (the muster:
// [`muster_corner`] in `corner_hold`, local, so reach is no gate) and the
// fight comes `answer_window_hours` later ([`resolve`]): the bodies within
// `muster_reach_tiles` of the corner and, off screen, the Statistical members
// living there stand (placed at the corner; promoted to Coarse on screen,
// the raid window's path), each side scaled by its presence about the cell,
// and `raid::fight_out` pairs them (each blow an Assault or a Murder,
// witnessed as any). **Hold or flip**: the side left standing holds the cell
// (`set_status`, the lock `hold_lock_days`); the side that lost it may come
// back within `retake_days` (**the counter-attack**: a daily roll, the
// leader's line). A contested cell is fought over (**the front**, a daily
// roll). A raid won or a Hideout sacked moves the cells about the Hideout
// ([`raid_moves`]). **Peace**: a side whose losses in the war (dead, jailed,
// gone) pass `exhaust_share` of its strength lies low (`faction::rescore`
// pins LieLow, [`lies_low`]); two spent sides' leaders strike a **truce**
// (the line held `truce_days`); a cell fought over past `crackdown_heat` is
// **cracked down on** by the law (no corner held there, the district's beat
// walks its Homes, [`crackdown_beat`]).
//
// The under-18 rule (CLAUDE.md): a protected member provokes nothing, is
// never mustered (`corner_hold` returns before the muster), never fights
// ([`fighters`]; `raid::fight_out` and its crossfire gate again), never
// counts as a loss; a family with children moves out of a contested cell.

/// `[turf.war]` (absent: no war).
pub fn war_cfg(world: &World) -> Option<&WarCfg> {
    cfg(world)?.war.as_ref()
}

fn pair(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn days(n: u16) -> Tick {
    Tick::from(n) * crate::time::TICKS_PER_DAY
}

fn hours(n: u16) -> Tick {
    Tick::from(n) * TICKS_PER_HOUR
}

/// `a` and `b` keep a truce.
pub fn in_truce(world: &World, a: EntityId, b: EntityId) -> bool {
    world.turf.truces.get(&pair(a, b)).is_some_and(|&t| t > world.tick)
}

/// A spent gang lies low (`faction::rescore` pins the order).
pub fn lies_low(world: &World, g: EntityId) -> bool {
    world.turf.lie_low.get(&g).is_some_and(|&t| t > world.tick)
}

fn cracked(world: &World, k: u32) -> bool {
    world.turf.cells.get(&k).is_some_and(|x| x.crackdown_until > world.tick)
}

fn night_of(world: &World) -> u64 {
    world.tick.saturating_sub(12 * TICKS_PER_HOUR) / crate::time::TICKS_PER_DAY
}

/// A keyed draw in [0, 1) (no stream touched).
fn draw(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    use rand::Rng;
    let x = purpose.rotate_left(56) ^ a.rotate_left(17) ^ b.rotate_left(37) ^ world.day();
    world.rng.keyed(crate::rng::splitmix64(x) >> 4).random()
}

/// The corner tile of cell `k` (its centre without one).
fn corner_tile(world: &World, k: u32) -> TilePos {
    world.turf.cells.get(&k).and_then(|x| x.corner).unwrap_or_else(|| centre(world, k))
}

/// "the corner by Bar#12" (else "the corner of the blocks near ...").
fn corner_name(world: &World, k: u32) -> String {
    match world.turf.cells.get(&k).and_then(|x| x.landmark) {
        Some(b) => format!("the corner by {}", world.name_of(b)),
        None => format!("the corner of {}", around(world, k)),
    }
}

fn leader_of(world: &World, g: EntityId) -> Option<EntityId> {
    world.comp::<Gang>(g)?.leader.filter(|&l| crate::systems::law::living(world, l))
}

/// "Vic Stone, The Rust Saints' leader," or "The Rust Saints".
fn who_leads(world: &World, g: EntityId) -> String {
    match leader_of(world, g) {
        Some(l) => format!("{}, {} leader,", world.name_of(l), poss(&gname(world, g))),
        None => gname(world, g),
    }
}

fn order_of(world: &World, g: EntityId) -> Option<Order> {
    world.comp::<Gang>(g).map(|x| x.order)
}

/// An adult member who can stand in a fight (living, free, not down).
fn can_fight(world: &World, m: EntityId) -> bool {
    crate::systems::law::living(world, m)
        && world.has::<Brain>(m)
        && !world.has::<Sentence>(m)
        // The under-18 rule: never a combatant.
        && !crate::systems::ages::is_protected(world, m)
        && !world.wounded.contains_key(&m)
        && world.comp::<Brain>(m).is_none_or(|b| b.cuffed_by.is_none())
}

/// `g`'s fit adults (a war side's strength at its start).
fn fit(world: &World, g: EntityId) -> u16 {
    world.comp::<Gang>(g).map_or(0, |x| x.members.iter().filter(|&&m| can_fight(world, m)).count() as u16)
}

/// `g`'s members living in cell `k` or within `muster_cells` of it.
fn muster_pool(world: &World, w: &WarCfg, k: u32, g: EntityId) -> Vec<EntityId> {
    let Some(x) = world.comp::<Gang>(g) else { return Vec::new() };
    x.members
        .iter()
        .copied()
        .filter(|&m| {
            can_fight(world, m) && home_cell(world, m).is_some_and(|h| cell_dist(world, h, k) <= w.muster_cells)
        })
        .collect()
}

/// The corner `id` is mustered to: a fight his gang stands in, due within
/// `answer_window_hours`, on a cell within `muster_cells` of his Home's.
pub fn muster_corner(world: &World, id: EntityId) -> Option<TilePos> {
    muster_fight(world, id).map(|x| x.0)
}

/// [`muster_corner`] with the tick the fight is due.
fn muster_fight(world: &World, id: EntityId) -> Option<(TilePos, Tick)> {
    if world.turf.open.is_empty() {
        return None;
    }
    let w = war_cfg(world)?;
    let g = world.gang_of(id)?;
    let lead = hours(w.answer_window_hours);
    let home = home_cell(world, id)?;
    world.turf.open.iter().filter(|&&(_, a, b)| a == g || b == g).find_map(|&(k, _, _)| {
        let f = world.turf.cells.get(&k)?.fight?;
        (f.due <= world.tick + lead && cell_dist(world, home, k) <= w.muster_cells)
            .then(|| (corner_tile(world, k), f.due))
    })
}

/// Round 7 W2 (`OPEN-241`): a mustered member standing on the corner he
/// was called to waits there for the fight: the ticks until it is due
/// (`exec::actions::duration`'s HangOut; the empty-spot leave and other
/// goals but danger or starving give way, `muster_holds`). V15: the called
/// reached the corner, hung out an hour alone (or left the empty spot) and
/// walked off before the fight; 391 of 955 called bodies at it at 20k.
pub fn muster_wait(world: &World, id: EntityId) -> Option<Tick> {
    let (t, due) = muster_fight(world, id)?;
    let reach = war_cfg(world)?.muster_reach_tiles;
    world
        .comp::<Position>(id)
        .filter(|p| p.building.is_none() && p.tile.manhattan(t) <= reach)
        .map(|_| due.saturating_sub(world.tick) + 1)
}

/// Round 7 W2 (`fixes::committed`): a mustered member's corner plan holds
/// while his fight is pending, as the raid's muster does (`crew::holds`).
pub fn muster_holds(world: &World, id: EntityId, plan: &Plan) -> bool {
    plan.goal == GoalKind::GangWork
        && plan.steps.iter().any(|s| s.action == ActionKind::HangOut)
        && world.unwind.get(&id).is_some_and(|u| u.spot.is_some() && u.spot == muster_corner(world, id))
}

/// A provocation's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provoke {
    Corner,
    Tag,
    Deal,
    Shakedown,
    Beating,
    /// An off-screen night of trespass (the daily roll).
    Night,
}

fn heat_of(w: &WarCfg, p: Provoke) -> f32 {
    match p {
        Provoke::Corner => w.heat_corner_hour,
        Provoke::Tag => w.heat_tag,
        Provoke::Deal => w.heat_deal,
        Provoke::Shakedown => w.heat_shakedown,
        Provoke::Beating => w.heat_beating,
        Provoke::Night => w.heat_stat,
    }
}

/// `gang`'s first adult body about cell `k` (the eyes a provocation needs).
fn eyes_in(world: &World, gang: EntityId, k: u32) -> Option<EntityId> {
    world.comp::<Gang>(gang)?.members.iter().copied().find(|&m| {
        world.comp::<Brain>(m).is_some_and(|b| b.lod != Lod::Statistical)
            && !world.has::<Sentence>(m)
            && !crate::systems::ages::is_protected(world, m)
            && crate::systems::law::living(world, m)
            && world.comp::<Position>(m).is_some_and(|p| cell_of(world, p.tile) == Some(k))
    })
}

/// The hooks' provocation (a deal, a shakedown): `actor`'s act at tile `t`
/// heats the cell for its holder when a holder's adult body there sees it (a
/// Statistical actor's act is seen off screen). Nobody under 18 provokes.
pub fn provoke_at(world: &mut World, actor: EntityId, t: TilePos, p: Provoke) {
    if world.turf.fighting {
        return;
    }
    let Some(w) = war_cfg(world) else { return };
    let heat = heat_of(w, p);
    let Some(k) = cell_of(world, t) else { return };
    let Some(h) = world.turf.cells.get(&k).and_then(|x| x.status.holder()) else { return };
    let Some(y) = world.gang_of(actor) else { return };
    if y == h || crate::systems::ages::is_protected(world, actor) {
        return;
    }
    let stat = world.comp::<Brain>(actor).is_some_and(|b| b.lod == Lod::Statistical);
    let seer = eyes_in(world, h, k);
    if seer.is_none() && !stat {
        return;
    }
    provoke(world, k, h, y, heat, Some(actor), seer, p);
}

/// From `law::resolve_fight_mods`: `winner` beat `loser`, a member of the
/// gang holding the cell where `loser` stands (not in a corner fight).
pub fn beaten(world: &mut World, winner: EntityId, loser: EntityId) {
    if world.turf.fighting || war_cfg(world).is_none() {
        return;
    }
    let Some(t) = world.comp::<Position>(loser).map(|p| p.tile) else { return };
    let (Some(gl), Some(gw)) = (world.gang_of(loser), world.gang_of(winner)) else { return };
    if gl == gw || holder_at(world, t) != Some(gl) {
        return;
    }
    provoke_at(world, winner, t, Provoke::Beating);
}

#[allow(clippy::too_many_arguments)]
fn provoke(
    world: &mut World,
    k: u32,
    h: EntityId,
    y: EntityId,
    heat: f32,
    actor: Option<EntityId>,
    seer: Option<EntityId>,
    p: Provoke,
) {
    let Some(w) = war_cfg(world).cloned() else { return };
    if heat <= 0.0 || in_truce(world, h, y) || cracked(world, k) || !world.has::<Gang>(y) {
        return;
    }
    // A gang lying low provokes nobody (its members' walks are not a push).
    if lies_low(world, y) || order_of(world, y) == Some(Order::LieLow) {
        return;
    }
    let now = world.tick;
    let Some(cell) = world.turf.cells.get_mut(&k) else { return };
    if cell.status != Status::Held(h) {
        return;
    }
    let v = match cell.heat.iter_mut().find(|e| e.0 == y) {
        Some(e) => {
            e.1 += heat;
            e.1
        }
        None => {
            cell.heat.push((y, heat));
            heat
        }
    };
    let open = cell.fight.is_some();
    crate::systems::ownership::jobs2_row(world).turf_provocations += 1;
    // Told once a cell a night, when a Full body did it or saw it.
    let night = night_of(world);
    let full = |id: Option<EntityId>| id.and_then(|x| world.comp::<Brain>(x)).is_some_and(|b| b.lod == Lod::Full);
    let tell = p != Provoke::Night && (full(actor) || full(seer)) && world.turf.trespass_told.get(&k) != Some(&night);
    if let Some(a) = actor.filter(|_| tell) {
        world.turf.trespass_told.insert(k, night);
        let (hn, yn, at) = (gname(world, h), gname(world, y), around(world, k));
        let by = match world.turf.cells.get(&k).and_then(|x| x.landmark) {
            Some(b) => format!("by {}", world.name_of(b)),
            None => format!("of {at}"),
        };
        let phrase = match p {
            Provoke::Corner => format!("hanging on {} corner {by}", poss(&hn)),
            Provoke::Tag => format!("spraying {} mark on the wall at {} corner {by}", poss(&yn), poss(&hn)),
            Provoke::Deal => format!("dealing on {} blocks ({at})", poss(&hn)),
            Provoke::Shakedown => format!("shaking down a door on {} blocks ({at})", poss(&hn)),
            Provoke::Beating | Provoke::Night => format!("beating one of {hn} on their own blocks ({at})"),
        };
        let an = world.name_of(a);
        let text = match seer {
            Some(s) => format!("{} of {hn} saw {an} of {yn} {phrase}", world.name_of(s)),
            None => format!("{an} of {yn} was seen {phrase}"),
        };
        let mut actors = vec![a];
        actors.extend(seer);
        actors.extend([h, y]);
        world.push_event(EventKind::Turf, &actors, text);
    }
    // A holder lying low does not answer (the heat stands).
    if v >= w.answer_heat && !open && !lies_low(world, h) {
        let due = if p == Provoke::Night { evening_due(world, &w) } else { now + hours(w.answer_window_hours) };
        open_fight(world, k, FightKind::Answer, h, y, due);
    }
}

/// Today's evening fight hour: `answer_window_hours` past `corner_from`.
fn evening_due(world: &World, w: &WarCfg) -> Tick {
    let from = cfg(world).map_or(1080, |c| c.corner_from);
    let start = world.tick - Tick::from(world.tick_of_day());
    let due = start + Tick::from(from) + hours(w.answer_window_hours);
    if due <= world.tick {
        world.tick + hours(w.answer_window_hours)
    } else {
        due
    }
}

/// A corner fight scheduled on cell `k` (`side` on it, `by` at it): the
/// moving side's leader decides it (his line), its people muster.
fn open_fight(world: &mut World, k: u32, kind: FightKind, side: EntityId, by: EntityId, due: Tick) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let now = world.tick;
    let Some(cell) = world.turf.cells.get_mut(&k) else { return };
    if cell.fight.is_some() {
        return;
    }
    cell.fight = Some(Fight { kind, side, by, opened: now, due });
    cell.heat.retain(|e| e.0 != by);
    world.turf.open.push((k, side, by));
    crate::systems::ownership::jobs2_row(world).turf_answers += 1;
    let (mover, other) = match kind {
        FightKind::Answer => (side, by),
        FightKind::Front | FightKind::Retake => (by, side),
    };
    let n = muster_pool(world, &w, k, mover).len();
    let at = corner_name(world, k);
    let (lead, on) = (who_leads(world, mover), gname(world, other));
    let theirs = match n {
        0 => "nobody of theirs".to_string(),
        1 => "the one of theirs".to_string(),
        n => format!("{n} of theirs"),
    };
    let when = if due.saturating_sub(now) > 6 * TICKS_PER_HOUR { " tonight" } else { "" };
    let text = match kind {
        _ if n == 0 => format!("{} had nobody about the blocks to send to {at}: {on} are on it", gname(world, mover)),
        FightKind::Answer => format!("{lead} called {theirs} about the blocks to {at}{when}: {on} were on their turf"),
        FightKind::Front => format!("{lead} called {theirs} about the blocks to {at}{when}: {on} claim the blocks too"),
        FightKind::Retake => {
            format!("{lead} sent {theirs} about the blocks back to {at}{when}: {on} took the blocks from them")
        }
    };
    if kind == FightKind::Answer {
        crate::systems::gang::push_shock(world, side, Shock::Trespass { by: Some(by) });
    }
    let mut actors: Vec<EntityId> = leader_of(world, mover).into_iter().collect();
    actors.extend([mover, other]);
    actors.extend(world.turf.cells.get(&k).and_then(|x| x.landmark));
    // Round 7 W2: the men called hear the call.
    actors.extend(hearers(world, mover, Some(k)));
    war_line(world, actors, text);
}

/// `g`'s presence over the window in cell `k` and the cells around it.
fn local_presence(world: &World, w: &WarCfg, k: u32, g: EntityId) -> f32 {
    let mut cells = vec![k];
    if w.muster_cells > 0 {
        cells.extend(neighbours(world, k));
    }
    cells
        .iter()
        .filter_map(|c| world.turf.cells.get(c))
        .flat_map(|x| x.days.iter().flat_map(|d| d.iter().filter(|p| p.gang == g).map(|p| p.p)))
        .sum()
}

/// A side's fighters at corner `t` of cell `k`: its adult bodies within
/// `muster_reach_tiles` of the corner, then (off screen) its Statistical
/// members living about the cell, nearest first, to `n` (at most
/// `side_max`). The Statistical ones are stood at the corner (promoted to
/// Coarse on screen). Nobody under 18 (`can_fight`).
fn fighters(world: &mut World, w: &WarCfg, k: u32, g: EntityId, t: TilePos, n: usize) -> Vec<EntityId> {
    let members = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
    let mut bodies: Vec<EntityId> = Vec::new();
    let mut far: Vec<(u32, EntityId)> = Vec::new();
    for m in members {
        if !can_fight(world, m) {
            continue;
        }
        let stat = world.comp::<Brain>(m).is_some_and(|b| b.lod == Lod::Statistical);
        if !stat {
            if world.comp::<Position>(m).is_some_and(|p| p.tile.manhattan(t) <= w.muster_reach_tiles) {
                bodies.push(m);
            }
        } else if home_cell(world, m).is_some_and(|h| cell_dist(world, h, k) <= w.muster_cells) {
            far.push((home_door(world, m).map_or(u32::MAX, |d| d.manhattan(t)), m));
        }
    }
    crate::systems::raid::by_strength(world, &mut bodies);
    bodies.truncate(w.side_max);
    far.sort_unstable();
    let want = n.min(w.side_max);
    let tick = world.tick;
    let shown = crate::systems::lod::on_screen(world, t);
    for (_, m) in far {
        if bodies.len() >= want {
            break;
        }
        if shown {
            crate::systems::lod::set_lod(world, m, Lod::Coarse);
        }
        world.remove_from_building(m);
        if let Some(p) = world.comp_mut::<Position>(m) {
            p.tile = t;
            p.building = None;
            p.entered = tick;
        }
        bodies.push(m);
    }
    bodies
}

/// Round 7 W2 (`OPEN-241`): gang `g`'s members who hear a war line: its
/// free, living adults living in cell `k` or within `muster_cells` of it
/// (with `k` None, all of them: the gang's own order, lying low or a
/// truce). V15: only the leader's diary heard his gang's war. Never a minor
/// (CLAUDE.md, the under-18 rule: no war line reaches a child as one of its
/// men).
fn hearers(world: &World, g: EntityId, k: Option<u32>) -> Vec<EntityId> {
    let Some(w) = war_cfg(world) else { return Vec::new() };
    let Some(x) = world.comp::<Gang>(g) else { return Vec::new() };
    x.members
        .iter()
        .copied()
        .filter(|&m| {
            crate::systems::law::living(world, m)
                && world.has::<Brain>(m)
                && !world.has::<Sentence>(m)
                && !crate::systems::ages::is_protected(world, m)
                && k.is_none_or(|k| home_cell(world, m).is_some_and(|h| cell_dist(world, h, k) <= w.muster_cells))
        })
        .collect()
}

/// Round 7 W2 (`OPEN-241`): a war line (the call, the fight, the war,
/// lying low, the truce, the crackdown, the quiet), its actors once each;
/// the gang members on it who do not lead their gang are counted
/// (`war_member_lines`).
fn war_line(world: &mut World, actors: Vec<EntityId>, text: String) {
    let mut seen: BTreeSet<EntityId> = BTreeSet::new();
    let actors: Vec<EntityId> = actors.into_iter().filter(|&a| seen.insert(a)).collect();
    let members = actors
        .iter()
        .filter(|&&a| world.gang_of(a).is_some_and(|g| world.comp::<Gang>(g).is_some_and(|x| x.leader != Some(a))))
        .count() as u32;
    if members > 0 {
        crate::systems::ownership::jobs2_row(world).war_member_lines += members;
    }
    world.push_event(EventKind::Turf, &actors, text);
}

/// Names, up to three ("A, B and 2 more").
fn names(world: &World, v: &[EntityId]) -> String {
    let shown: Vec<String> = v.iter().take(3).map(|&m| world.name_of(m)).collect();
    let more = v.len().saturating_sub(shown.len());
    match (shown.len(), more) {
        (0, _) => String::new(),
        (1, 0) => shown[0].clone(),
        (_, 0) => format!("{} and {}", shown[..shown.len() - 1].join(", "), shown[shown.len() - 1]),
        _ => format!("{} and {more} more", shown.join(", ")),
    }
}

/// The hour (from [`hourly`]): the corner's trespassers heat the cells
/// their holders' bodies watch; the corner fights due are fought.
fn war_hourly(world: &mut World, trespass: &[(u32, EntityId, EntityId)], eyes: &BTreeMap<(u32, EntityId), EntityId>) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let night = night_of(world);
    for &(k, y, m) in trespass {
        let Some(h) = world.turf.cells.get(&k).and_then(|x| x.status.holder()) else { continue };
        let Some(&seer) = eyes.get(&(k, h)) else { continue };
        // His mark by the corner, once a night (told first), then the hour.
        if world.turf.tagged.get(&m) != Some(&night) {
            world.turf.tagged.insert(m, night);
            provoke(world, k, h, y, w.heat_tag, Some(m), Some(seer), Provoke::Tag);
        }
        provoke(world, k, h, y, w.heat_corner_hour, Some(m), Some(seer), Provoke::Corner);
    }
    if world.turf.open.is_empty() {
        return;
    }
    let now = world.tick;
    let due: Vec<u32> = world
        .turf
        .open
        .iter()
        .filter(|&&(k, _, _)| world.turf.cells.get(&k).and_then(|x| x.fight).is_none_or(|f| f.due <= now))
        .filter(|&&(k, _, _)| !waits_for_called(world, k))
        .map(|x| x.0)
        .collect();
    for k in due {
        resolve(world, k);
    }
}

/// Round 7 W2 (`OPEN-241`): a corner fight due waits one hour for the
/// mover's called men on their way to the corner, due there within `[crew]
/// march_wait_ticks` (as a raid's column waits for its tail), at most
/// `march_wait_ticks` past its hour.
fn waits_for_called(world: &World, k: u32) -> bool {
    let Some(f) = world.turf.cells.get(&k).and_then(|x| x.fight) else { return false };
    let Some(wait) = crate::systems::crew::cfg(world).map(|c| c.march_wait_ticks).filter(|&t| t > 0) else {
        return false;
    };
    if world.tick >= f.due + wait {
        return false;
    }
    let mover = if f.kind == FightKind::Answer { f.side } else { f.by };
    let t = corner_tile(world, k);
    let Some(w) = war_cfg(world) else { return false };
    let reach = w.muster_reach_tiles;
    muster_pool(world, w, k, mover).into_iter().any(|m| {
        let Some(b) = world.comp::<Brain>(m) else { return false };
        let on_way = b.plan.as_ref().is_some_and(|p| {
            p.goal == GoalKind::GangWork
                && p.steps.get(usize::from(b.plan_step)).map(|s| s.action) == Some(ActionKind::GoTo(LocationKey::Spot))
        }) && world.unwind.get(&m).is_some_and(|u| u.spot == Some(t));
        if !on_way {
            return false;
        }
        let eta = match &b.exec {
            crate::exec::ExecState::GotoTimed { arrive_tick, .. } => arrive_tick.saturating_sub(world.tick),
            _ => world.comp::<Position>(m).map_or(Tick::MAX, |p| {
                Tick::from(p.tile.manhattan(t).saturating_sub(reach)) * world.config.exec.move_ticks_full
            }),
        };
        eta <= wait
    })
}

/// The war between `a` and `b` (started over cell `k` when new: told).
fn war_entry(world: &mut World, a: EntityId, b: EntityId, k: u32) {
    let key = pair(a, b);
    let now = world.tick;
    if let Some(x) = world.turf.wars.get_mut(&key) {
        x.last = now;
        return;
    }
    let size = [fit(world, key.0), fit(world, key.1)];
    world.turf.wars.insert(key, War { started: now, last: now, cell: k, size, ..Default::default() });
    crate::systems::ownership::jobs2_row(world).turf_wars += 1;
    let side = |world: &World, g: EntityId| {
        let n = if key.0 == g { size[0] } else { size[1] };
        match leader_of(world, g) {
            Some(l) => format!("{} leads {n} of {}", world.name_of(l), gname(world, g)),
            None => format!("{n} of {}", gname(world, g)),
        }
    };
    let text = format!(
        "{} and {} are at war over {} ({}; {})",
        gname(world, a),
        gname(world, b),
        around(world, k),
        side(world, a),
        side(world, b)
    );
    let mut actors = vec![a, b];
    actors.extend(leader_of(world, a));
    actors.extend(leader_of(world, b));
    actors.extend(world.turf.cells.get(&k).and_then(|x| x.landmark));
    actors.extend(hearers(world, a, Some(k)));
    actors.extend(hearers(world, b, Some(k)));
    war_line(world, actors, text);
}

/// The fight scheduled on cell `k`, fought: the hold or the flip, the war's
/// tally, the law's attention.
fn resolve(world: &mut World, k: u32) {
    world.turf.open.retain(|x| x.0 != k);
    let Some(f) = world.turf.cells.get_mut(&k).and_then(|x| x.fight.take()) else { return };
    let Some(w) = war_cfg(world).cloned() else { return };
    let (side, by) = (f.side, f.by);
    if !world.has::<Gang>(side) || !world.has::<Gang>(by) || in_truce(world, side, by) || cracked(world, k) {
        return;
    }
    let status = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status);
    if !status.involves(side) {
        return;
    }
    let now = world.tick;
    let t = corner_tile(world, k);
    let at = corner_name(world, k);
    // The aggressor: the holder pushing back an answer's trespassers, else
    // the side coming at the cell.
    let (ag, dg) = if f.kind == FightKind::Answer { (side, by) } else { (by, side) };
    let (pa, pd) = (local_presence(world, &w, k, ag), local_presence(world, &w, k, dg));
    let top = pa.max(pd).max(1e-3);
    let n = |p: f32| ((w.side_max as f32 * p / top).round() as usize).clamp(1, w.side_max.max(1));
    let (na, nd) = (n(pa), n(pd));
    // Round 7 W2 (`OPEN-241`): the mover's called bodies, and those at the
    // corner now (before the off-screen men are stood there).
    {
        let called: Vec<EntityId> = muster_pool(world, &w, k, ag)
            .into_iter()
            .filter(|&m| world.comp::<Brain>(m).is_some_and(|b| b.lod != Lod::Statistical))
            .collect();
        let came = called
            .iter()
            .filter(|&&m| world.comp::<Position>(m).is_some_and(|p| p.tile.manhattan(t) <= w.muster_reach_tiles))
            .count() as u32;
        let row = crate::systems::ownership::jobs2_row(world);
        row.war_called += called.len() as u32;
        row.war_came += came;
    }
    let mut agg = fighters(world, &w, k, ag, t, na);
    let mut def = fighters(world, &w, k, dg, t, nd);
    let landmark = world.turf.cells.get(&k).and_then(|x| x.landmark);
    if agg.is_empty() || def.is_empty() {
        if agg.is_empty() && def.is_empty() {
            return;
        }
        crate::systems::ownership::jobs2_row(world).turf_unopposed += 1;
        let (here, here_g, gone_g) = if agg.is_empty() { (def, dg, ag) } else { (agg, ag, dg) };
        let mut actors = vec![here_g, gone_g];
        actors.extend(here.iter().copied());
        actors.extend(landmark);
        let text = format!(
            "{} came to {at} ({}): nobody of {} there",
            gname(world, here_g),
            names(world, &here),
            poss(&gname(world, gone_g))
        );
        actors.extend(hearers(world, here_g, Some(k)));
        actors.extend(hearers(world, gone_g, Some(k)));
        war_line(world, actors, text);
        // An answer nobody of the holder's came to: the trespassers stand on
        // it (contested); a cell nobody defends is walked onto.
        let now_status = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status);
        let next = match f.kind {
            FightKind::Answer if here_g == by => Some(Status::Contested(side, by)),
            FightKind::Answer => None,
            _ if here_g == by => Some(Status::Held(by)),
            _ => None,
        };
        if let Some(next) = next.filter(|&x| x != now_status) {
            take(world, &w, k, now_status, next, here_g, gone_g);
        }
        if let Some(x) = world.turf.cells.get_mut(&k) {
            x.heat.clear();
        }
        return;
    }
    world.turf.fighting = true;
    crate::systems::raid::by_strength(world, &mut agg);
    crate::systems::raid::by_strength(world, &mut def);
    let (agg0, def0) = (agg.clone(), def.clone());
    let _tally = crate::systems::raid::fight_out(world, &mut agg, &mut def, t, &at, 1.0, None);
    world.turf.fighting = false;
    let (winner, loser) = if def.is_empty() { (ag, dg) } else { (dg, ag) };
    let dead: Vec<EntityId> =
        agg0.iter().chain(def0.iter()).copied().filter(|&m| !crate::systems::law::living(world, m)).collect();
    {
        let row = crate::systems::ownership::jobs2_row(world);
        row.turf_fights += 1;
        row.turf_war_deaths += dead.len() as u32;
    }
    war_entry(world, ag, dg, k);
    // The war's men, and its losses: each side's fallen in this fight
    // (beaten or dead), then its men's later deaths, jailings, leavings.
    let key = pair(ag, dg);
    if let Some(x) = world.turf.wars.get_mut(&key) {
        x.fights += 1;
        x.deaths += dead.len() as u32;
        for (g, before, after) in [(ag, &agg0, &agg), (dg, &def0, &def)] {
            let i = usize::from(key.0 != g);
            for &m in before {
                if after.contains(&m) {
                    x.men[i].insert(m);
                } else {
                    x.men[i].remove(&m);
                    x.losses[i].push_back(now);
                }
            }
        }
    }
    let old = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status);
    let retaken = world.turf.cells.get(&k).and_then(|x| x.lost).is_some_and(|(g, u)| g == winner && u > now);
    let flips = old != Status::Held(winner);
    let (wn, ln) = (gname(world, winner), gname(world, loser));
    let tail = if dead.is_empty() { String::new() } else { format!("; {} dead", names(world, &dead)) };
    let count = format!("{} against {}", agg0.len(), def0.len());
    let text = match (f.kind, winner == ag) {
        (FightKind::Answer, true) => format!("{wn} pushed {ln} off {at}, {count}{tail}"),
        (FightKind::Answer, false) => format!("{wn} beat {ln} back on {at}, {count}, and hold it now{tail}"),
        (FightKind::Front, true) => format!("{wn} took {at} from {ln}, {count}{tail}"),
        (FightKind::Front, false) => format!("{wn} drove {ln} off {at}, {count}{tail}"),
        (FightKind::Retake, true) => format!("{wn} came back for {at} and took it back from {ln}, {count}{tail}"),
        (FightKind::Retake, false) => format!("{wn} held {at} when {ln} came back for it, {count}{tail}"),
    };
    let mut actors = vec![winner, loser];
    actors.extend(agg0.iter().chain(def0.iter()).copied());
    actors.extend(landmark);
    actors.extend(hearers(world, winner, Some(k)));
    actors.extend(hearers(world, loser, Some(k)));
    war_line(world, actors, text);
    if flips {
        take(world, &w, k, old, Status::Held(winner), winner, loser);
        if retaken {
            crate::systems::ownership::jobs2_row(world).turf_retaken += 1;
        }
    }
    if let Some(x) = world.turf.cells.get_mut(&k) {
        x.won = Some((winner, now + days(w.hold_lock_days)));
        x.heat.clear();
        x.law_heat += 1.0 + dead.len() as f32 * w.crackdown_death_w;
    }
    if world.turf.cells.get(&k).is_some_and(|x| x.law_heat >= w.crackdown_heat) {
        crackdown(world, &w, k, ag, dg);
    }
}

/// Cell `k` passes from `old` to `new` in the war (`set_status`, told): the
/// winner's lock, the loser's window to come back, the war's flip.
fn take(world: &mut World, w: &WarCfg, k: u32, old: Status, new: Status, winner: EntityId, loser: EntityId) {
    let now = world.tick;
    set_status(world, k, old, new, true, now);
    if let Some(x) = world.turf.cells.get_mut(&k) {
        if new == Status::Held(winner) {
            x.won = Some((winner, now + days(w.hold_lock_days)));
            x.lost = old.involves(loser).then_some((loser, now + days(w.retake_days)));
        }
    }
    if new.holder().is_some() {
        crate::systems::ownership::jobs2_row(world).turf_flips += 1;
        if let Some(x) = world.turf.wars.get_mut(&pair(winner, loser)) {
            x.flips += 1;
        }
    }
    reindex(world);
}

/// The law's crackdown on cell `k` (its fights past `crackdown_heat`): no
/// corner held there for `crackdown_days`, the fights off, the district's
/// beat walks its Homes ([`crackdown_beat`]).
fn crackdown(world: &mut World, w: &WarCfg, k: u32, a: EntityId, b: EntityId) {
    if world.law().is_none() {
        return;
    }
    let now = world.tick;
    if let Some(x) = world.turf.cells.get_mut(&k) {
        x.crackdown_until = now + days(w.crackdown_days);
        x.law_heat = 0.0;
        x.heat.clear();
        x.fight = None;
    }
    world.turf.open.retain(|x| x.0 != k);
    crate::systems::ownership::jobs2_row(world).turf_crackdowns += 1;
    let text = format!(
        "The law cracked down on {}: {} and {} kept off the corner, the beat walks its Homes for {} days",
        around(world, k),
        gname(world, a),
        gname(world, b),
        w.crackdown_days
    );
    let mut actors = vec![a, b];
    actors.extend(leader_of(world, a));
    actors.extend(leader_of(world, b));
    actors.extend(world.turf.cells.get(&k).and_then(|x| x.landmark));
    // Round 7 W2: the men living on the blocks hear the law came for them.
    actors.extend(hearers(world, a, Some(k)));
    actors.extend(hearers(world, b, Some(k)));
    war_line(world, actors, text);
}

/// `law::district_route`: a cell of district `d` the law cracks down on
/// gives the beat its Homes (the nearest the corner, up to five).
pub fn crackdown_beat(world: &World, d: DistrictId) -> Option<Vec<EntityId>> {
    if !on(world) {
        return None;
    }
    let now = world.tick;
    let k = world
        .turf
        .cells
        .iter()
        .find(|(&k, x)| x.crackdown_until > now && world.district_of(centre(world, k)) == d)
        .map(|(&k, _)| k)?;
    let t = corner_tile(world, k);
    let mut homes: Vec<(u32, EntityId)> = world
        .district(d)
        .homes
        .iter()
        .copied()
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && cell_of(world, b.door) == Some(k))
        .map(|(h, b)| (b.door.manhattan(t), h))
        .collect();
    homes.sort_unstable();
    let v: Vec<EntityId> = homes.into_iter().take(5).map(|(_, h)| h).collect();
    (!v.is_empty()).then_some(v)
}

/// From `gang::on_member_killed`, `on_member_arrested`, `leave`: a war
/// side's loss (an adult's: a child is never a combatant).
pub fn note_loss(world: &mut World, gang: EntityId, id: EntityId) {
    if world.turf.wars.is_empty() || crate::systems::ages::is_protected(world, id) {
        return;
    }
    let now = world.tick;
    for ((a, b), war) in world.turf.wars.iter_mut() {
        let i = if *a == gang {
            0
        } else if *b == gang {
            1
        } else {
            continue;
        };
        if war.men[i].remove(&id) {
            war.losses[i].push_back(now);
        }
    }
}

/// From `raid::settle`: a raid won or a Hideout sacked moves `loser`'s cells
/// within `raid_cells` of its Hideout to `winner`.
pub fn raid_moves(world: &mut World, winner: EntityId, loser: EntityId) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let Some(hk) = world.hideout_of(loser).and_then(|h| door_of(world, h)).and_then(|t| cell_of(world, t)) else {
        return;
    };
    let cells: Vec<(u32, Status)> = world
        .turf
        .cells
        .iter()
        .filter(|(&k, x)| x.status.involves(loser) && cell_dist(world, k, hk) <= w.raid_cells)
        .map(|(&k, x)| (k, x.status))
        .collect();
    if cells.is_empty() {
        return;
    }
    war_entry(world, winner, loser, cells[0].0);
    for (k, old) in cells {
        take(world, &w, k, old, Status::Held(winner), winner, loser);
        crate::systems::ownership::jobs2_row(world).turf_raid_moves += 1;
    }
}

/// Midnight (after the holders): the heat cooled, the wars' exhaustion,
/// truces and quiet, the off-screen rolls (a night's trespass on a border,
/// a contested cell's front, the loser's counter-attack).
fn war_daily(world: &mut World) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let now = world.tick;
    for x in world.turf.cells.values_mut() {
        for e in &mut x.heat {
            e.1 *= w.heat_keep_day;
        }
        x.heat.retain(|e| e.1 >= 0.05);
        x.law_heat *= w.crackdown_keep_day;
        if x.law_heat < 0.05 {
            x.law_heat = 0.0;
        }
        if x.won.is_some_and(|(_, t)| t <= now) {
            x.won = None;
        }
        if x.lost.is_some_and(|(_, t)| t <= now) {
            x.lost = None;
        }
    }
    world.turf.truces.retain(|_, t| *t > now);
    world.turf.lie_low.retain(|_, t| *t > now);
    let night = night_of(world);
    world.turf.tagged.retain(|_, n| *n + 1 >= night);
    world.turf.trespass_told.retain(|_, n| *n + 1 >= night);
    let cells = &world.turf.cells;
    world.turf.open.retain(|&(k, _, _)| cells.get(&k).is_some_and(|x| x.fight.is_some()));
    // The wars: the spent, the truce, the quiet.
    let window = days(w.exhaust_days);
    let keys: Vec<(EntityId, EntityId)> = world.turf.wars.keys().copied().collect();
    for key in keys {
        let (a, b) = key;
        if !world.has::<Gang>(a) || !world.has::<Gang>(b) {
            world.turf.wars.remove(&key);
            continue;
        }
        let Some(war) = world.turf.wars.get_mut(&key) else { continue };
        let mut spent: Vec<(usize, usize)> = Vec::new();
        for i in 0..2 {
            while war.losses[i].front().is_some_and(|&t| t + window < now) {
                war.losses[i].pop_front();
            }
            let mark = (w.exhaust_share * f32::from(war.size[i])).ceil().max(f32::from(w.exhaust_min)) as usize;
            if war.exhausted[i].is_none_or(|t| t + window < now) && war.losses[i].len() >= mark {
                war.exhausted[i] = Some(now);
                spent.push((i, war.losses[i].len()));
            }
        }
        let both = war.exhausted.iter().all(|e| e.is_some_and(|t| t + window >= now));
        let quiet = war.last + days(w.war_quiet_days) <= now;
        let (size, started, fights, flips, deaths) = (war.size, war.started, war.fights, war.flips, war.deaths);
        for (i, lost) in spent {
            let (g, o) = if i == 0 { (a, b) } else { (b, a) };
            spend(world, &w, g, o, lost, size[i], started);
        }
        if both {
            truce(world, &w, key);
            world.turf.wars.remove(&key);
        } else if quiet {
            let since = started / crate::time::TICKS_PER_DAY;
            let text = format!(
                "The war between {} and {} went quiet (since day {since}: {fights} corner fights, {deaths} dead, \
                 {flips} cells changed hands)",
                gname(world, a),
                gname(world, b)
            );
            let mut actors = vec![a, b];
            actors.extend(hearers(world, a, None));
            actors.extend(hearers(world, b, None));
            war_line(world, actors, text);
            world.turf.wars.remove(&key);
        }
    }
    rolls(world, &w);
    reindex(world);
}

/// A side spent: its leader calls it off the corners, it lies low.
fn spend(world: &mut World, w: &WarCfg, g: EntityId, other: EntityId, lost: usize, size: u16, started: Tick) {
    let now = world.tick;
    world.turf.lie_low.insert(g, now + days(w.lielow_days));
    crate::systems::ownership::jobs2_row(world).turf_exhausted += 1;
    let since = started / crate::time::TICKS_PER_DAY;
    let gn = gname(world, g);
    let what = format!(
        "{lost} of their {size} beaten, dead, jailed or gone since the war with {} began on day {since}",
        gname(world, other)
    );
    let text = match leader_of(world, g) {
        Some(l) => format!("{} called {gn} off the corners to lie low: {what}", world.name_of(l)),
        None => format!("{gn} are spent and lie low: {what}"),
    };
    let mut actors = vec![g, other];
    actors.extend(leader_of(world, g));
    actors.extend(hearers(world, g, None));
    war_line(world, actors, text);
}

/// Two spent sides' leaders strike a truce: the contested cells between
/// them go to the side ahead, the line between them held `truce_days`, the
/// fights between them called off.
fn truce(world: &mut World, w: &WarCfg, key: (EntityId, EntityId)) {
    let (a, b) = key;
    let now = world.tick;
    let until = now + days(w.truce_days);
    world.turf.truces.insert(key, until);
    crate::systems::ownership::jobs2_row(world).turf_truces += 1;
    let split: Vec<(u32, Status, EntityId)> = world
        .turf
        .cells
        .iter()
        .filter_map(|(&k, x)| match x.status {
            Status::Contested(p, q) if pair(p, q) == key => Some((k, x.status, p)),
            _ => None,
        })
        .collect();
    for &(k, old, first) in &split {
        set_status(world, k, old, Status::Held(first), false, now);
    }
    // The line: every cell either holds that borders the other's, held.
    let mut line: Vec<u32> = Vec::new();
    for (&k, x) in &world.turf.cells {
        let Some(h) = x.status.holder().filter(|&h| h == a || h == b) else { continue };
        let o = if h == a { b } else { a };
        if neighbours(world, k).iter().any(|n| world.turf.cells.get(n).is_some_and(|y| y.status == Status::Held(o))) {
            line.push(k);
        }
    }
    for &k in &line {
        if let Some(x) = world.turf.cells.get_mut(&k) {
            if let Some(h) = x.status.holder() {
                x.won = Some((h, until));
            }
            if x.fight.is_some_and(|f| pair(f.side, f.by) == key) {
                x.fight = None;
            }
            x.heat.retain(|e| e.0 != a && e.0 != b);
        }
    }
    let cells = &world.turf.cells;
    world.turf.open.retain(|&(k, _, _)| cells.get(&k).is_some_and(|x| x.fight.is_some()));
    let held = |g: EntityId| world.turf.cells.values().filter(|x| x.status == Status::Held(g)).count();
    let (ha, hb) = (held(a), held(b));
    let named = |world: &World, g: EntityId| {
        leader_of(world, g).map_or_else(|| gname(world, g), |l| format!("{} of {}", world.name_of(l), gname(world, g)))
    };
    let (la, lb) = (named(world, a), named(world, b));
    let border = match line.len() {
        0 => String::new(),
        n => format!(", {n} on the border"),
    };
    let text = format!(
        "{la} and {lb} struck a truce: the line runs where it stands ({ha} blocks and {hb}{border}), held {} days",
        w.truce_days
    );
    let mut actors = vec![a, b];
    actors.extend(leader_of(world, a));
    actors.extend(leader_of(world, b));
    actors.extend(hearers(world, a, None));
    actors.extend(hearers(world, b, None));
    war_line(world, actors, text);
}

/// The day's off-screen rolls: a gang trespasses on a held cell it borders
/// (`provoke_p_day`, the heat of a night), a contested cell is fought over
/// (`front_p_day`), a gang comes back for a cell it lost (`retake_p_day`).
fn rolls(world: &mut World, w: &WarCfg) {
    let now = world.tick;
    let mut plan: Vec<(u32, FightKind, EntityId, EntityId)> = Vec::new();
    let mut provokes: Vec<(u32, EntityId, EntityId)> = Vec::new();
    let quiet =
        |world: &World, g: EntityId| lies_low(world, g) || order_of(world, g).is_none_or(|o| o == Order::LieLow);
    for (&k, x) in &world.turf.cells {
        if x.fight.is_some() || x.crackdown_until > now {
            continue;
        }
        match x.status {
            Status::Held(h) => {
                if let Some((l, until)) = x.lost {
                    if until > now
                        && l != h
                        && world.has::<Gang>(l)
                        && !in_truce(world, h, l)
                        && !quiet(world, l)
                        && draw(world, 1, u64::from(k), u64::from(l.index)) < w.retake_p_day
                    {
                        plan.push((k, FightKind::Retake, h, l));
                        continue;
                    }
                }
                let ys: BTreeSet<EntityId> = neighbours(world, k)
                    .iter()
                    .filter_map(|n| world.turf.cells.get(n).and_then(|y| y.status.holder()))
                    .filter(|&y| y != h && world.has::<Gang>(y))
                    .collect();
                for y in ys {
                    if in_truce(world, h, y) || quiet(world, y) {
                        continue;
                    }
                    let hot = matches!(order_of(world, y), Some(Order::Contest | Order::Expand | Order::Retaliate));
                    let p = w.provoke_p_day * if hot { w.provoke_order_mult } else { 1.0 };
                    if draw(world, 2, u64::from(k), u64::from(y.index)) < p {
                        provokes.push((k, h, y));
                        break;
                    }
                }
            }
            Status::Contested(a, b) => {
                if !in_truce(world, a, b)
                    && !quiet(world, a)
                    && !quiet(world, b)
                    && draw(world, 3, u64::from(k), 0) < w.front_p_day
                {
                    plan.push((k, FightKind::Front, a, b));
                }
            }
            Status::Open => {}
        }
    }
    for (k, h, y) in provokes {
        provoke(world, k, h, y, w.heat_stat, None, None, Provoke::Night);
    }
    let due = evening_due(world, w);
    for (k, kind, side, by) in plan {
        open_fight(world, k, kind, side, by, due);
    }
}
