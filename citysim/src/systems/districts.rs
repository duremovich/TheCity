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
    Brain, Building, BuildingKind, Controller, Corp, CorpShock, District, DistrictId, Gang, Household, Mood, Position,
    Shock, TilePos, Zone, MAX_DISTRICTS,
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
        buildings.push((id, b.door, b.kind, b.demolished));
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
        let (sx, sy, c) = sum[i];
        d.centroid = match ((sx + c / 2).checked_div(c), (sy + c / 2).checked_div(c)) {
            (Some(x), Some(y)) => TilePos { x: x as u8, y: y as u8 },
            _ => TilePos::default(),
        };
    }
    buildings.sort_by_key(|&(id, ..)| id);
    for (id, door, kind, demolished) in buildings {
        let i = usize::from(grid[usize::from(door.y) * w + usize::from(door.x)] & ID_MASK);
        let d = &mut world.districts[i];
        d.buildings.push(id);
        if kind == BuildingKind::Home && !demolished {
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

    pub fn district(&self, d: DistrictId) -> &District {
        &self.districts[d.index().min(self.districts.len().saturating_sub(1))]
    }

    pub fn district_mut(&mut self, d: DistrictId) -> &mut District {
        let i = d.index().min(self.districts.len().saturating_sub(1));
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

/// D6: daily at midnight after `classes`.
pub fn run(world: &mut World) {
    if world.tick_of_day() == 0 {
        daily(world);
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
    aggregates(world);
    update_control(world);
}

/// D7: one O(agents) pass. Every living adult with a Brain is binned by the
/// district of its Home door, else (homeless) by its tile; children count in
/// `population` the same way. Fills population, adults, classes, happiness,
/// fear, coverage, crime rate, `residents` and the trace.
pub fn aggregates(world: &mut World) {
    let n = world.districts.len();
    let now = world.tick;
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
    let fear: Vec<f32> = (0..n)
        .map(|d| {
            let homes = &world.districts[d].homes;
            let base = if homes.is_empty() {
                0.0
            } else {
                homes.iter().map(|&h| crate::systems::classes::home_fear(world, h)).sum::<f32>() / homes.len() as f32
            };
            let crush = if world.districts[d].crush_until.is_some_and(|t| t > now) { 0.3 } else { 0.0 };
            (base + crush).min(1.0)
        })
        .collect();
    for (i, d) in world.districts.iter_mut().enumerate() {
        d.population = population[i];
        d.adults = adults[i];
        d.classes = classes[i];
        d.happiness = if adults[i] == 0 { 0.0 } else { happy[i] / adults[i] as f32 };
        d.coverage = coverage[i];
        d.fear = fear[i];
        let crimes: u32 = d.crimes.iter().map(|&c| u32::from(c)).sum();
        d.crime_rate = crimes as f32 / CRIME_DAYS as f32 / population[i].max(1) as f32 * 100.0;
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
/// seed or a pre-M12 load is silent; a later change logs `DistrictControl`
/// and shocks the faction that lost it.
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
        {
            let x = &mut world.districts[i];
            x.control = ctrl;
            x.control_since = now;
            x.control_init = true;
        }
        if !init {
            continue;
        }
        let name = world.districts[i].name.clone();
        let text =
            format!("{name}: {} -> {} ({share:.2})", controller_label(world, old), controller_label(world, ctrl));
        let actors: Vec<EntityId> = [old.entity(), ctrl.entity()].into_iter().flatten().collect();
        world.push_event(EventKind::DistrictControl, &actors, text);
        match old {
            Controller::Gang(g) => crate::systems::gang::push_shock(world, g, Shock::LostDistrict),
            Controller::Corp(c) => crate::systems::ownership::push_corp_shock(world, c, CorpShock::LostDistrict),
            Controller::Contested | Controller::City => {}
        }
    }
}
