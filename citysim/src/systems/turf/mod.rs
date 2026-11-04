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

mod corners;
mod muster;
mod provoke;
mod status;
mod war;

pub use corners::*;
pub use muster::*;
pub use provoke::*;
pub use status::*;
pub use war::*;

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
    /// Round 8 X4: member -> the due tick of the fight he was roused for
    /// (once a fight).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roused: BTreeMap<EntityId, Tick>,
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
