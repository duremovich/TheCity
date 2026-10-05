//! M12 districts (docs/M12_DISTRICTS.md § 1, plan phase 1, D1-D8).
//!
//! The map's zones are cut on road lines into `[districts]` rows. A tile's
//! district is one byte read from `World::district_grid` (built by
//! [`rebuild`] at world creation, on load and after every wall change), so
//! nothing here costs anything per agent per tick.
//!
//! `run` is daily at midnight, after `classes` and before the economy
//! (D6): it rolls each district's crime counter, recomputes the aggregates
//! (population, class mix, happiness, fear, crime rate, coverage) and the
//! controller. Phase 1 changes no behaviour: nothing outside this module
//! reads the aggregates yet, and nothing here draws a random number.

use std::collections::BTreeSet;

use crate::components::{
    Brain, Building, BuildingKind, Controller, Corp, CorpShock, District, DistrictId, Gang, Household, Job, Mood,
    Position, Role, Shock, TilePos, Zone, MAX_DISTRICTS,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::world::World;

/// Bit 7 of a `district_grid` byte: walkable and inside no building rect
/// (the litter and sweep domain, phase 3).
pub const STREET_BIT: u8 = 0x80;
/// The low nibble of a `district_grid` byte.
const ID_MASK: u8 = 0x0f;
/// Days kept in `District::crimes`.
const CRIME_DAYS: usize = 7;
/// Days kept in `World::eviction_places`.
pub const EVICTION_PLACE_DAYS: u64 = 30;

/// One `[districts]` row, resolved.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    zone: Zone,
    x_from: u16,
    x_to: u16,
}

/// The rows in force. A map without zones (the v1 map) is one district,
/// unless every configured row is a Mid row (a test may cut a zoneless map
/// into Mid districts by x); plan D1.
fn rows(world: &World) -> Vec<Row> {
    let cfg = &world.config.districts;
    let n = cfg.row_count();
    let all: Vec<Row> = (0..n)
        .map(|i| Row {
            name: cfg.names[i].clone(),
            zone: cfg.zone_of(i).unwrap_or_default(),
            x_from: cfg.x_from[i],
            x_to: cfg.x_to[i],
        })
        .collect();
    if !world.map.has_zones() && all.iter().any(|r| r.zone != Zone::Mid) {
        let single = crate::config::DistrictsCfg::single();
        return vec![Row {
            name: single.names[0].clone(),
            zone: Zone::Mid,
            x_from: single.x_from[0],
            x_to: single.x_to[0],
        }];
    }
    all
}

/// The district a tile falls in by the rows: the first row matching its
/// zone and x. No match is a config error (`debug_assert!`), else 0.
fn row_of(rows: &[Row], zone: Zone, x: u16) -> u8 {
    let hit = rows.iter().position(|r| r.zone == zone && r.x_from <= x && x < r.x_to);
    debug_assert!(hit.is_some(), "no [districts] row covers zone {zone} x {x}");
    hit.unwrap_or(0) as u8
}

/// D1: rebuild the grid, the rows (creating `World::districts` when the
/// save had none or a different row count), each district's `buildings`,
/// `homes`, `walk_tiles` and `centroid`, and the adjacency masks. Saved
/// aggregates of an existing row are kept. Draws nothing.
pub fn rebuild(world: &mut World) {
    let rows = rows(world);
    let (w, h) = (world.map.w(), world.map.h());

    // Building rects (standing buildings; a demolished Home is open ground).
    let mut in_building = vec![false; w * h];
    let mut buildings: Vec<(EntityId, TilePos, BuildingKind, bool)> = Vec::new();
    for id in world.with::<Building>() {
        let Some(b) = world.comp::<Building>(id) else { continue };
        // M12 D25: a derelict Block is nobody's Home.
        buildings.push((id, b.door, b.kind, b.demolished || b.derelict));
        if b.demolished {
            continue;
        }
        let r = b.rect;
        for y in u16::from(r.y)..u16::from(r.y) + u16::from(r.h) {
            for x in u16::from(r.x)..u16::from(r.x) + u16::from(r.w) {
                let (x, y) = (usize::from(x), usize::from(y));
                if x < w && y < h {
                    in_building[y * w + x] = true;
                }
            }
        }
    }

    let n = rows.len();
    let mut grid = vec![0u8; w * h];
    let mut walk = vec![0u32; n];
    let mut streets: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut sum = vec![(0u64, 0u64, 0u64); n];
    for y in 0..h {
        for x in 0..w {
            let p = TilePos { x: x as u8, y: y as u8 };
            let d = row_of(&rows, world.map.zone(p), x as u16);
            let i = y * w + x;
            let street = world.map.walkable(p) && !in_building[i];
            grid[i] = d | if street { STREET_BIT } else { 0 };
            let s = &mut sum[usize::from(d)];
            if street {
                walk[usize::from(d)] += 1;
                streets[usize::from(d)].push(i as u32);
                s.0 += x as u64;
                s.1 += y as u64;
                s.2 += 1;
            }
        }
    }
    let mut adjacent = vec![0u16; n];
    for y in 0..h {
        for x in 0..w {
            let d = usize::from(grid[y * w + x] & ID_MASK);
            if x + 1 < w {
                let e = usize::from(grid[y * w + x + 1] & ID_MASK);
                if e != d {
                    adjacent[d] |= 1 << e;
                    adjacent[e] |= 1 << d;
                }
            }
            if y + 1 < h {
                let s = usize::from(grid[(y + 1) * w + x] & ID_MASK);
                if s != d {
                    adjacent[d] |= 1 << s;
                    adjacent[s] |= 1 << d;
                }
            }
        }
    }

    // Rows: keep a save's districts when they match the config, else start fresh.
    let matches = world.districts.len() == n && world.districts.iter().zip(&rows).all(|(d, r)| d.name == r.name);
    if !matches {
        world.districts = rows
            .iter()
            .enumerate()
            .map(|(i, r)| District {
                id: DistrictId(i as u8),
                name: r.name.clone(),
                zone: r.zone,
                coverage: 1.0,
                ..District::default()
            })
            .collect();
    }
    for (i, d) in world.districts.iter_mut().enumerate() {
        d.id = DistrictId(i as u8);
        d.zone = rows[i].zone;
        d.buildings.clear();
        d.homes.clear();
        d.walk_tiles = walk[i];
        d.streets = std::mem::take(&mut streets[i]);
        let (sx, sy, c) = sum[i];
        d.centroid = match ((sx + c / 2).checked_div(c), (sy + c / 2).checked_div(c)) {
            (Some(x), Some(y)) => TilePos { x: x as u8, y: y as u8 },
            _ => TilePos::default(),
        };
    }
    buildings.sort_by_key(|&(id, ..)| id);
    for (id, door, kind, unhoused) in buildings {
        let i = usize::from(grid[usize::from(door.y) * w + usize::from(door.x)] & ID_MASK);
        let d = &mut world.districts[i];
        d.buildings.push(id);
        if kind == BuildingKind::Home && !unhoused {
            d.homes.push(id);
        }
    }
    world.district_grid = grid;
    world.district_adjacent = adjacent;
}

/// The first district cut from `zone` (D4: where a pre-M12 trace entry goes);
/// district 0 when no row has that zone (the one-district v1 city).
pub fn first_of_zone(world: &World, zone: Zone) -> DistrictId {
    world.districts.iter().find(|d| d.zone == zone).map_or(DistrictId(0), |d| d.id)
}

impl World {
    /// D2: the district a tile lies in; one byte read.
    pub fn district_of(&self, p: TilePos) -> DistrictId {
        let i = usize::from(p.y) * self.map.w() + usize::from(p.x);
        DistrictId(self.district_grid.get(i).map_or(0, |&b| b & ID_MASK))
    }

    /// D2: walkable and inside no building rect.
    pub fn is_street(&self, p: TilePos) -> bool {
        let i = usize::from(p.y) * self.map.w() + usize::from(p.x);
        self.district_grid.get(i).is_some_and(|&b| b & STREET_BIT != 0)
    }

    /// The district of a building's door (`DistrictId(0)` for a non-building).
    pub fn district_of_building(&self, b: EntityId) -> DistrictId {
        self.comp::<Building>(b).map_or(DistrictId(0), |bd| self.district_of(bd.door))
    }

    /// The district `d` (out of range: the last). Before `rebuild` has run
    /// (a hand-built world) there are none: an empty default (review fix).
    pub fn district(&self, d: DistrictId) -> &District {
        static EMPTY: std::sync::OnceLock<District> = std::sync::OnceLock::new();
        match self.districts.len() {
            0 => EMPTY.get_or_init(District::default),
            n => &self.districts[d.index().min(n - 1)],
        }
    }

    /// As `district`; before `rebuild` it creates the one default district
    /// (and `debug_assert!`s: a caller mutating districts should have built them).
    pub fn district_mut(&mut self, d: DistrictId) -> &mut District {
        debug_assert!(!self.districts.is_empty(), "district_mut before districts::rebuild");
        if self.districts.is_empty() {
            self.districts.push(District::default());
        }
        let i = d.index().min(self.districts.len() - 1);
        &mut self.districts[i]
    }

    /// The district's display name; "?" for UNSET or out of range.
    pub fn district_name(&self, d: DistrictId) -> &str {
        self.districts.get(d.index()).map_or("?", |x| x.name.as_str())
    }
}

/// D7: a crime at `tile` counts toward its district's crime rate.
pub fn note_crime(world: &mut World, tile: TilePos) {
    let d = world.district_of(tile);
    note_crime_in(world, d);
}

/// D7: a crime in district `d` (a hole's district).
pub fn note_crime_in(world: &mut World, d: DistrictId) {
    if let Some(x) = world.districts.get_mut(d.index()) {
        x.crimes_today = x.crimes_today.saturating_add(1);
    }
}

/// D29 input (phase 3/4 read it): an eviction from `home` by `owner`, kept 30 days.
pub fn note_eviction(world: &mut World, home: EntityId, owner: Option<EntityId>) {
    let d = world.district_of_building(home);
    let tick = world.tick;
    world.eviction_places.push_back((tick, d, owner));
    let horizon = tick.saturating_sub(EVICTION_PLACE_DAYS * crate::time::TICKS_PER_DAY);
    while world.eviction_places.front().is_some_and(|&(t, ..)| t < horizon) {
        world.eviction_places.pop_front();
    }
}

/// D6: daily at midnight after `classes`; at 03:00 the street's nightly
/// pass (Hotels and squats with `[street] enabled`, Vagrancy with `[law]
/// district_beats`, the street's litter with `[litter] enabled`).
pub fn run(world: &mut World) {
    // M12 D31: live riots fizzle past their march window (at most two).
    crate::systems::riot::run(world);
    let tod = world.tick_of_day();
    if tod == 0 {
        daily(world);
    } else if tod == crate::systems::street::NIGHTLY_TOD
        && (world.config.law.district_beats || world.config.street.enabled || world.config.litter.enabled)
    {
        crate::systems::street::nightly(world);
    }
}

/// The daily pass: roll the crime counters, the aggregates (with coverage),
/// then control.
pub fn daily(world: &mut World) {
    if world.districts.is_empty() {
        return;
    }
    // The first midnight of a fresh world has no finished day to roll.
    if world.tick > 0 {
        for d in &mut world.districts {
            d.crimes.push_back(std::mem::take(&mut d.crimes_today));
            while d.crimes.len() > CRIME_DAYS {
                d.crimes.pop_front();
            }
        }
    }
    // M12 phase 3: the street's day (abandonment, re-letting), then the
    // litter's (decay, the sweepers, owners and gangs cleaning, the means).
    crate::systems::street::daily(world);
    if crate::systems::litter::enabled(world) {
        crate::systems::litter::decay(world);
        crate::systems::litter::district_means(world);
        sanitation(world);
        owner_cleaning(world);
        gang_cleaning(world);
        crate::systems::litter::district_means(world);
    }
    aggregates(world);
    update_control(world);
    // M12 phase 4: unrest per district (D29), the riot trigger (D30), the
    // district strikes (D41), all reading today's aggregates.
    unrest(world);
    crate::systems::riot::trigger(world);
    crate::systems::classes::strike(world);
}

/// D29: each district's unrest over its Street and Dreg resident adults:
/// M11's class formula (`classes::aggregate`) with the district's 7-day
/// evictions, submission + `curfew_fear` under a curfew, plus
/// `rent_burden_w × burden`, `burden` = the mean over its renters of their
/// share of the Home's rent ÷ their daily income (wage, else the dole).
/// `unrest_streak` counts the midnights above `[riots] riot_threshold`.
pub fn unrest(world: &mut World) {
    use crate::components::Class;
    let c = world.config.classes.clone();
    let threshold = world.config.riots.riot_threshold;
    let dole = i64::from(world.levers.dole_per_day);
    let horizon = world.tick.saturating_sub(7 * crate::time::TICKS_PER_DAY);
    let execs = crate::systems::classes::exec_set(world);
    let fears = district_fear(world);
    for i in 0..world.districts.len() {
        let residents = world.districts[i].residents.clone();
        let mut members: Vec<(f32, bool, f32)> = Vec::new();
        let mut street: Vec<(f32, bool, f32)> = Vec::new();
        let (mut burden, mut renters) = (0.0f32, 0u32);
        for a in residents {
            let (class, mood, employed, fear) = crate::systems::classes::member_in(world, a, &execs, &fears);
            if class == Class::Corp {
                continue;
            }
            members.push((mood, employed, fear));
            if class == Class::Street {
                street.push((mood, employed, fear));
            }
            let Some(h) = world.comp::<Household>(a).and_then(|h| h.home) else { continue };
            let rent = world.comp::<Building>(h).map_or(0, |b| b.rent_per_day);
            if rent <= 0 {
                continue;
            }
            let adults = world
                .residents_of(h)
                .iter()
                .filter(|&&r| crate::systems::demography::is_adult(world, r))
                .count()
                .max(1) as f32;
            let income = world.comp::<Job>(a).map_or(dole, |j| j.wage_per_day).max(1) as f32;
            burden += rent as f32 / adults / income;
            renters += 1;
        }
        let evictions =
            world.eviction_places.iter().filter(|&&(t, d, _)| t >= horizon && d.index() == i).count() as u32;
        let curfew = if world.districts[i].curfew { c.curfew_fear } else { 0.0 };
        let burden = if renters == 0 { 0.0 } else { burden / renters as f32 };
        let formula = |m: &[(f32, bool, f32)]| {
            let agg = crate::systems::classes::aggregate(m, evictions);
            let submission = (agg.submission + curfew).min(1.0);
            let n = m.len();
            let unrest = if n == 0 {
                0.0
            } else {
                (1.0 - agg.loyalty) * (1.0 - submission) + 0.1 * evictions as f32 / n as f32 + c.rent_burden_w * burden
            };
            (unrest, agg.loyalty, submission)
        };
        let (unrest, loyalty, submission) = formula(&members);
        let (street_unrest, _, _) = formula(&street);
        let n = members.len();
        let d = &mut world.districts[i];
        d.unrest = unrest;
        d.street_unrest = street_unrest;
        d.unrest_streak = if unrest > threshold { d.unrest_streak.saturating_add(1) } else { 0 };
        d.trace.extend([
            ("unrest_n", n as f32),
            ("unrest_loyalty", loyalty),
            ("unrest_submission", submission),
            ("evictions_7d", evictions as f32),
            ("rent_burden", burden),
            ("unrest", unrest),
            ("street_unrest", street_unrest),
            ("unrest_streak", f32::from(d.unrest_streak)),
        ]);
    }
}

// ---------------------------------------------------------------------------
// Cleaning (plan D23, D24)
// ---------------------------------------------------------------------------

/// D23: the city's sweepers (`Role::Sanitation`), ascending.
pub fn sweepers(world: &World) -> Vec<EntityId> {
    world.workers(Role::Sanitation).to_vec()
}

/// D23, daily at midnight: deal the sweepers to districts by `litter_d ×
/// walk_tiles_d × levers.sanitation_weight[d]` (largest remainder, ties
/// the lower district; in id order, district 0 first), log `Sanitation`
/// when the allocation changes, then credit every sweeper whose shift
/// yesterday was worked with `clean_per_shift` units off its beat's
/// dirtiest street tiles.
pub fn sanitation(world: &mut World) {
    let workers = sweepers(world);
    let n = world.districts.len();
    // The credit: yesterday's beats, yesterday's shifts.
    let per = world.config.litter.clean_per_shift;
    let mut units = vec![0u32; n];
    for &s in &workers {
        let Some(&d) = world.sweep_beats.get(&s) else { continue };
        let worked = world.comp::<Job>(s).is_some_and(|j| {
            let yesterday = j.shift_key_at(world.tick.saturating_sub(1));
            j.last_shift_day == Some(yesterday)
        });
        if worked && d.index() < n {
            units[d.index()] += per;
        }
    }
    for (i, &u) in units.iter().enumerate() {
        crate::systems::litter::clean_dirtiest(world, DistrictId(i as u8), u);
    }
    // Today's allocation.
    let weights: Vec<f32> = world
        .districts
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let lever = world.levers.sanitation_weight.get(i).copied().unwrap_or(1.0).max(0.0);
            d.litter * d.walk_tiles as f32 * lever
        })
        .collect();
    let alloc = crate::util::largest_remainder(workers.len() as u32, &weights);
    let old: Vec<u8> = world.districts.iter().map(|d| d.sweepers).collect();
    world.sweep_beats.clear();
    let mut it = workers.iter();
    for (i, &k) in alloc.iter().enumerate() {
        for _ in 0..k {
            if let Some(&s) = it.next() {
                world.sweep_beats.insert(s, DistrictId(i as u8));
            }
        }
    }
    for (d, &k) in world.districts.iter_mut().zip(&alloc) {
        d.sweepers = k;
    }
    if old != alloc && !workers.is_empty() {
        let text = world
            .districts
            .iter()
            .filter(|d| d.sweepers > 0)
            .map(|d| format!("{} {}", d.name, d.sweepers))
            .collect::<Vec<_>>()
            .join(", ");
        world.push_event(EventKind::Sanitation, &[], format!("sweepers dealt: {text}"));
    }
}

/// D24: an owner keeps its doors clean when it can: a corp holding Secure,
/// or any owner whose buildings took in more today than their upkeep,
/// pays the City `owner_clean_cost` per 32 units (rounded up) to clear up to
/// `owner_clean_units` within 2 tiles of each owned door (ascending); a door
/// it cannot pay for is skipped. Buildings ascending by id.
pub fn owner_cleaning(world: &mut World) {
    let cfg = world.config.litter.clone();
    let up = world.config.corps.upkeep.clone();
    // Per owner: today's revenue and upkeep over its standing buildings.
    // Per owner: (revenue, upkeep, doors).
    type Books = std::collections::BTreeMap<EntityId, (i64, i64, Vec<TilePos>)>;
    let mut books: Books = Default::default();
    for b in world.with::<Building>() {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        let Some(o) = bd.owner else { continue };
        if bd.demolished || bd.derelict || bd.kind == BuildingKind::Lot {
            continue;
        }
        let e = books.entry(o).or_default();
        // `revenue_today` was rolled into `revenue` by `ownership::run` at this midnight.
        e.0 += bd.revenue.back().copied().unwrap_or(0);
        e.1 += up.for_building(bd.kind, bd.tier);
        e.2.push(bd.door);
    }
    for (owner, (revenue, upkeep, doors)) in books {
        let secure = world.comp::<Corp>(owner).is_some_and(|c| c.order == crate::components::CorpOrder::Secure);
        if !(secure || revenue > upkeep) {
            continue;
        }
        let corp = world.has::<Corp>(owner);
        for door in doors {
            let dirt = crate::systems::litter::units_around(world, door, 2);
            if dirt == 0 {
                continue;
            }
            let units = dirt.min(cfg.owner_clean_units);
            let cost = cfg.owner_clean_cost * i64::from(units.div_ceil(32));
            if cost > 0 {
                if !corp && world.purse(Some(owner)) < cost {
                    continue;
                }
                let flow = crate::systems::ownership::Flow::Sanitation;
                if corp {
                    crate::systems::ownership::charge(world, Some(owner), None, cost, flow);
                } else {
                    crate::systems::ownership::pay(world, Some(owner), None, cost, flow);
                }
            }
            crate::systems::litter::clean_around(world, door, 2, units);
        }
    }
}

/// D24: a gang controlling a district whose leader's pride is at least
/// `clean_pride`, under an order that is no raid and not LieLow, clears
/// `gang_clean_per_member × fit members` units around its held Homes' doors
/// there, free (Homes ascending, the units shared out in turn).
pub fn gang_cleaning(world: &mut World) {
    let cfg = world.config.litter.clone();
    for i in 0..world.districts.len() {
        let Controller::Gang(g) = world.districts[i].control else { continue };
        let Some(gang) = world.comp::<Gang>(g) else { continue };
        if gang.order.is_raid() || gang.order == crate::components::Order::LieLow {
            continue;
        }
        let pride = gang.leader.and_then(|l| world.comp::<crate::components::Personality>(l)).map_or(0.0, |p| p.pride);
        if pride < cfg.clean_pride {
            continue;
        }
        let d = DistrictId(i as u8);
        let doors: Vec<TilePos> = gang
            .territory
            .iter()
            .copied()
            .filter(|&h| world.district_of_building(h) == d)
            .filter_map(|h| world.comp::<Building>(h).map(|b| b.door))
            .collect();
        let mut left = cfg.gang_clean_per_member * crate::systems::gang::fit_headcount(world, g) as u32;
        for door in doors {
            if left == 0 {
                break;
            }
            left -= crate::systems::litter::clean_around(world, door, 2, left);
        }
    }
}

/// D23 (plan deviation: its own reconcile, not `law::reconcile_guards`
/// generalised): hire jobless free adults (no lawfulness floor; the
/// poorest first, ties lower id) or let the newest go, at most five a
/// day, toward `levers.sanitation_count`; the Recycler employs them, the
/// Treasury pays them.
pub fn reconcile_sanitation(world: &mut World) {
    let want = usize::from(world.levers.sanitation_count);
    let have = sweepers(world);
    let Some(recycler) = world.building_of_kind(BuildingKind::Cemetery) else { return };
    if have.len() < want {
        let mut candidates: Vec<(i64, EntityId)> = world
            // scan-ok: daily: reconcile_sanitation
            .citizens()
            .into_iter()
            .filter(|&id| {
                world.has::<Brain>(id) && !world.has::<Job>(id) && !world.has::<crate::components::Sentence>(id)
            })
            .filter(|&id| !world.has::<crate::components::GangMember>(id))
            .filter(|&id| world.comp::<Brain>(id).is_some_and(|b| !b.emigrating))
            .filter(|&id| crate::systems::demography::is_adult(world, id))
            .filter(|&id| !crate::systems::founding::is_exec(world, id))
            .map(|id| (world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins), id))
            .collect();
        candidates.sort_unstable();
        for (_, id) in candidates.into_iter().take((want - have.len()).min(5)) {
            crate::systems::demography::hire(world, id, recycler, Role::Sanitation);
        }
    } else if have.len() > want {
        let mut newest: Vec<(std::cmp::Reverse<u64>, EntityId)> =
            have.iter().filter_map(|&s| world.comp::<Job>(s).map(|j| (std::cmp::Reverse(j.hired_tick), s))).collect();
        newest.sort_unstable();
        for (_, s) in newest.into_iter().take((have.len() - want).min(5)) {
            let text = format!("{} let go from Sanitation", world.name_of(s));
            crate::systems::economy::dismiss(world, s, Some(recycler), text);
            world.sweep_beats.remove(&s);
        }
    }
}

/// D7: one O(agents) pass. Every living adult with a Brain is binned by the
/// district of its Home door, else (homeless) by its tile; children count in
/// `population` the same way. Fills population, adults, classes, happiness,
/// fear, coverage, crime rate, `residents` and the trace.
pub fn aggregates(world: &mut World) {
    let n = world.districts.len();
    let execs = crate::systems::classes::exec_set(world);
    let mut population = vec![0u32; n];
    let mut adults = vec![0u32; n];
    let mut classes = vec![[0u32; 3]; n];
    let mut happy = vec![0.0f32; n];
    let mut residents: Vec<Vec<EntityId>> = vec![Vec::new(); n];
    // scan-ok: daily: district aggregates
    for a in world.citizens() {
        let home = world.comp::<Household>(a).and_then(|h| h.home);
        let at = match home.and_then(|h| world.comp::<Building>(h)) {
            Some(b) => b.door,
            None => match world.comp::<Position>(a) {
                Some(p) => p.tile,
                None => continue,
            },
        };
        let d = world.district_of(at).index().min(n - 1);
        population[d] += 1;
        if !world.has::<Brain>(a) || !crate::systems::demography::is_adult(world, a) {
            continue;
        }
        adults[d] += 1;
        let class = crate::systems::classes::class_in(world, a, &execs);
        classes[d][class.index()] += 1;
        happy[d] += (world.comp::<Mood>(a).map_or(0.0, |m| m.value) + 1.0) / 2.0;
        residents[d].push(a);
    }
    let coverage: Vec<f32> =
        (0..n).map(|d| crate::systems::bind::district_coverage(world, DistrictId(d as u8))).collect();
    let fear = district_fear(world);
    for (i, d) in world.districts.iter_mut().enumerate() {
        d.population = population[i];
        d.adults = adults[i];
        d.classes = classes[i];
        d.happiness = if adults[i] == 0 { 0.0 } else { happy[i] / adults[i] as f32 };
        d.coverage = coverage[i];
        d.fear = fear[i];
        let crimes: u32 = d.crimes.iter().map(|&c| u32::from(c)).sum();
        // Phase 1's note: a district without Blocks (the Civic, the Vats) has
        // no residents to divide by (a homeless adult or two at most), so its
        // rate reads 0; its crimes still count in `crimes`.
        d.crime_rate = if population[i] == 0 || d.homes.is_empty() {
            0.0
        } else {
            // Over the days of history there are (review fix: a fresh world
            // has fewer than 7, and dividing by 7 understated its rate).
            crimes as f32 / d.crimes.len().max(1) as f32 / population[i] as f32 * 100.0
        };
        d.residents = std::mem::take(&mut residents[i]);
        d.trace = vec![
            ("population", population[i] as f32),
            ("adults", adults[i] as f32),
            ("corp", classes[i][0] as f32),
            ("street", classes[i][1] as f32),
            ("dreg", classes[i][2] as f32),
            ("happiness", d.happiness),
            ("coverage", d.coverage),
            ("fear", d.fear),
            ("crimes_7d", crimes as f32),
            ("crime_rate", d.crime_rate),
            ("homes", d.homes.len() as f32),
        ];
    }
}

/// D7: each district's fear: the mean `classes::home_fear` over its standing
/// Homes (0 with none), + 0.3 while a Crush holds, capped at 1. A Dreg's fear
/// (M12 D3, `classes::member`) reads its district's.
pub fn district_fear(world: &World) -> Vec<f32> {
    let now = world.tick;
    world
        .districts
        .iter()
        .map(|d| {
            let base = if d.homes.is_empty() {
                0.0
            } else {
                d.homes.iter().map(|&h| crate::systems::classes::home_fear(world, h)).sum::<f32>()
                    / d.homes.len() as f32
            };
            let crush = if d.crush_until.is_some_and(|t| t > now) { 0.3 } else { 0.0 };
            (base + crush).min(1.0)
        })
        .collect()
}

/// D8: presence per faction in `d`, highest first; ties City, then gangs,
/// then corps, then lower id. Factions with no presence are left out.
pub fn presence(world: &World, d: DistrictId) -> Vec<(Controller, f32)> {
    let Some(dist) = world.districts.get(d.index()) else { return Vec::new() };
    let wts = &world.config.districts.control_weights;
    let held = crate::systems::gang::CLAIM_HELD;
    let hideouts: BTreeSet<EntityId> =
        world.gang_list().iter().filter_map(|&g| world.comp::<Gang>(g).map(|x| x.hideout)).collect();
    let mut city = 0.0f32;
    let mut out: Vec<(Controller, f32)> = Vec::new();
    let add = |out: &mut Vec<(Controller, f32)>, c: Controller, v: f32| {
        if let Some(e) = out.iter_mut().find(|(k, _)| *k == c) {
            e.1 += v;
        } else {
            out.push((c, v));
        }
    };
    for &b in &dist.buildings {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if bd.demolished || bd.kind == BuildingKind::Lot {
            continue;
        }
        let is_home = bd.kind == BuildingKind::Home;
        if is_home {
            if let Some(c) = bd.claim.filter(|c| c.count >= held && world.has::<Gang>(c.gang)) {
                add(&mut out, Controller::Gang(c.gang), wts.held_home);
            }
        }
        // D8: a derelict counts for nobody (a gang's held squat counted above).
        if bd.derelict {
            continue;
        }
        if bd.kind == BuildingKind::Hideout && hideouts.contains(&b) {
            // A gang's own Hideout is its presence, whoever holds the deed.
            if let Some(&g) = world.gang_list().iter().find(|&&g| world.comp::<Gang>(g).is_some_and(|x| x.hideout == b))
            {
                add(&mut out, Controller::Gang(g), wts.owned_other);
            }
            continue;
        }
        let w = if is_home { wts.owned_home } else { wts.owned_other };
        match bd.owner {
            None => city += w,
            Some(o) if world.has::<Corp>(o) => add(&mut out, Controller::Corp(o), w),
            Some(o) if world.has::<Gang>(o) => add(&mut out, Controller::Gang(o), wts.owned_other),
            Some(_) => {}
        }
    }
    let city = city * dist.coverage * wts.city_per_coverage;
    if city > 0.0 {
        out.push((Controller::City, city));
    }
    out.retain(|&(_, v)| v > 0.0);
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.tie_rank().cmp(&b.0.tie_rank())));
    out
}

/// The controller and its share from a presence list (D8).
pub fn control_of(presence: &[(Controller, f32)], min_share: f32) -> (Controller, f32) {
    let total: f32 = presence.iter().map(|&(_, v)| v).sum();
    let Some(&(top, v)) = presence.first() else { return (Controller::Contested, 0.0) };
    if total <= 0.0 {
        return (Controller::Contested, 0.0);
    }
    let share = v / total;
    if share >= min_share {
        (top, share)
    } else {
        (Controller::Contested, share)
    }
}

/// A controller's name for the event log and the panel.
pub fn controller_label(world: &World, c: Controller) -> String {
    match c {
        Controller::Contested => "Contested".to_string(),
        Controller::City => "the City".to_string(),
        Controller::Gang(g) => world.comp::<Gang>(g).map_or_else(|| world.name_of(g), |x| x.name.clone()),
        Controller::Corp(c) => world.comp::<Corp>(c).map_or_else(|| world.name_of(c), |x| x.name.clone()),
    }
}

/// D8: recompute every district's controller. The first computation after a
/// seed or a pre-M12 load is silent; a later change logs `DistrictControl`.
/// The faction that held it is shocked only when another faction (not
/// Contested) takes it over, so a share hovering at `control_min_share` does
/// not shock its gang every few days (phase 1 review).
pub fn update_control(world: &mut World) {
    let min_share = world.config.districts.control_min_share;
    let now = world.tick;
    for i in 0..world.districts.len().min(MAX_DISTRICTS) {
        let d = DistrictId(i as u8);
        let pres = presence(world, d);
        let (ctrl, share) = control_of(&pres, min_share);
        let top3: Vec<(Controller, f32)> = pres.iter().take(3).copied().collect();
        let (old, init) = {
            let x = &world.districts[i];
            (x.control, x.control_init)
        };
        {
            let x = &mut world.districts[i];
            x.control_share = share;
            for (k, (_, v)) in top3.iter().enumerate() {
                x.trace.push((["presence_1", "presence_2", "presence_3"][k], *v));
            }
            x.trace.push(("control_share", share));
        }
        if init && ctrl == old {
            continue;
        }
        // A save from before the field (or a fresh district) has no holder
        // recorded: the standing controller is it.
        let last = match world.districts[i].last_holder {
            Controller::Contested => old,
            h => h,
        };
        {
            let x = &mut world.districts[i];
            x.control = ctrl;
            x.control_since = now;
            x.control_init = true;
        }
        if !init {
            if ctrl != Controller::Contested {
                world.districts[i].last_holder = ctrl;
            }
            continue;
        }
        let name = world.districts[i].name.clone();
        let text =
            format!("{name}: {} -> {} ({share:.2})", controller_label(world, old), controller_label(world, ctrl));
        let actors: Vec<EntityId> = [old.entity(), ctrl.entity()].into_iter().flatten().collect();
        world.push_event(EventKind::DistrictControl, &actors, text);
        // Review fix: a share wobbling across `control_min_share` flips the
        // district to Contested and back every few days; only another
        // faction taking it (directly or after a Contested spell) is a loss.
        if ctrl != Controller::Contested {
            world.districts[i].last_holder = ctrl;
            if last != ctrl {
                match last {
                    Controller::Gang(g) => crate::systems::gang::push_shock(world, g, Shock::LostDistrict),
                    Controller::Corp(c) => {
                        crate::systems::ownership::push_corp_shock(world, c, CorpShock::LostDistrict)
                    }
                    Controller::Contested | Controller::City => {}
                }
            }
        }
    }
}
