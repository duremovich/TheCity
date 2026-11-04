//! The view milestone V5 (`OPEN-149`; standing decision 2026-10-20: nothing in camera range is Statistical): the
//! camera ring's own tiers, outside `[lod] max_full` / `max_coarse`. With a view set (`World::view_rect`, the
//! camera; later the player), every body whose place is in the ring (the view grown by `lod::VIEW_MARGIN`) is Full,
//! a band of `[lod] ring_band_tiles` around it Coarse (same brain, timed legs, so a step into the ring is a plausible
//! promotion), Statistical beyond. The ring re-assigns each tick, not hourly, and only bodies entering or leaving the
//! ring and the band change tier (`run`). Ranked by camera distance (then a story step), inside `ring_full_max` and
//! `ring_coarse_max`: a fast mover's ring stays bounded by the budget, and a body is let go `ring_hold_tiles` past
//! the band (no flap at the edge) to the tier it had when the ring took it (`Hold::base`); the hourly assignment
//! (`lod::assign`) skips the bodies the ring holds and fills its own budget with the rest of the city.
//!
//! A Statistical body has no place of its own (its `Position` is the demotion's snapshot, stale for days): its place
//! is its schedule's (`schedule_door`, the Statistical tier's own: at work on a worked shift as `lod::stat_work`
//! credits it, else at home). So the ring finds the Statistical bodies whose homes and workplaces stand in the band
//! (a door grid, `World::residents_of`, `World::staff_of`), promotes them into their building, and moves a stale
//! snapshot standing in the band to its schedule's door (a Statistical body drawn where it is not). No view, no
//! ring: a headless run is untouched. A view wider than `ring_max_side_tiles` (the 2D app's map overview) holds
//! nothing either.

use std::collections::BTreeMap;

use crate::components::{Brain, Building, Household, Job, Lod, Position, Rect, Sentence, TilePos};
use crate::entity::EntityId;
use crate::exec::ExecState;
use crate::spatial::TileGrid;
use crate::time::{Tick, TICKS_PER_HOUR};
use crate::world::World;

/// A body the ring holds: the tier it holds it at, and the tier it had when the ring took it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hold {
    pub tier: Lod,
    pub base: Lod,
}

/// The ring's counts at the last tick it ran (the view's HUD and log).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RingCounts {
    pub tick: Tick,
    /// Bodies held at Full / Coarse.
    pub full: usize,
    pub coarse: usize,
    /// In the ring (the view + margin): Full, Coarse, Statistical (budget bound or refused).
    pub in_full: usize,
    pub in_coarse: usize,
    pub in_stat: usize,
    /// Wanted Full / Coarse past the budget.
    pub over_full: usize,
    pub over_coarse: usize,
    /// This tick: taken, let go, tier changed inside, stale Statistical snapshots moved to their schedule's door.
    pub taken: usize,
    pub released: usize,
    pub moved: usize,
    pub replaced: usize,
    /// The camera's move since the last tick (tiles, the lead), and the ring's own wall time this tick (µs).
    pub lead: u32,
    pub micros: u64,
}

/// The ring's state on the world (not saved: a loaded world starts with an empty ring and takes its bodies on
/// the first tick with a view).
#[derive(Clone, Debug, Default)]
pub struct Ring {
    pub held: BTreeMap<EntityId, Hold>,
    pub counts: RingCounts,
    /// The view's centre at the last tick (the lead: the camera's move per tick).
    last_centre: Option<TilePos>,
    /// The doors of the Homes and workplaces (`TileGrid` items index `doors`), rebuilt hourly.
    doors: Option<(Tick, Vec<EntityId>, TileGrid)>,
}

impl Ring {
    pub fn holds(&self, id: EntityId) -> bool {
        !self.held.is_empty() && self.held.contains_key(&id)
    }
}

/// `r` grown by `m` tiles on every side, clipped to the map.
fn grow(r: Rect, m: u16, w: usize, h: usize) -> Rect {
    let x = r.x.saturating_sub(m);
    let y = r.y.saturating_sub(m);
    let x1 = (u32::from(r.x) + u32::from(r.w) + u32::from(m)).min(w as u32);
    let y1 = (u32::from(r.y) + u32::from(r.h) + u32::from(m)).min(h as u32);
    Rect { x, y, w: (x1.saturating_sub(u32::from(x))) as u16, h: (y1.saturating_sub(u32::from(y))) as u16 }
}

/// The higher of two tiers (Full above Coarse above Statistical).
fn higher(a: Lod, b: Lod) -> Lod {
    a.min(b)
}

/// Where the Statistical tier has `id` now: its workplace on a worked shift (`lod::stat_work`'s rule: on shift, a
/// workday, not sick), else its Home. `None`: no place of its own (sentenced, away abroad, homeless and off
/// shift): its `Position` stands.
pub fn schedule_door(world: &World, id: EntityId) -> Option<EntityId> {
    if world.has::<Sentence>(id) || crate::systems::abroad::is_away(world, id) {
        return None;
    }
    if let Some(j) = world.comp::<Job>(id) {
        if let Some(emp) = j.employer {
            let tod = world.tick_of_day();
            if j.on_shift(tod)
                && crate::exec::routine::workday_of(world, id, j, j.shift_key_at(world.tick))
                && !crate::systems::health::sick(world, id)
                && world.has::<Building>(emp)
            {
                return Some(emp);
            }
        }
    }
    world.comp::<Household>(id).and_then(|h| h.home).filter(|&b| world.has::<Building>(b))
}

/// A body's place now, cheaply (the band is wide: no road search): aboard, its vehicle's; a Coarse walker, the
/// straight line from his walk's start to its target at the walk's pace; else its `Position`.
fn body_place(world: &World, id: EntityId) -> Option<TilePos> {
    if let Some(v) = crate::systems::motion::vehicle_of(world, id) {
        if let Some(m) = world.motions.get(&v) {
            return Some(match &m.route {
                crate::systems::motion::Route::Timed { way, depart, arrive } => {
                    crate::systems::motion::timed_pos(way, *depart, *arrive, world.tick)
                }
                _ => m.pos,
            });
        }
    }
    let p = world.comp::<Position>(id)?;
    if let Some(ExecState::GotoTimed { target, arrive_tick, walk: Some(w), .. }) =
        world.comp::<Brain>(id).map(|b| &b.exec)
    {
        let now = world.tick;
        if now > w.depart && now < *arrive_tick {
            let f = (now - w.depart) as f64 / (*arrive_tick - w.depart) as f64;
            let lerp = |a: u16, b: u16| (f64::from(a) + (f64::from(b) - f64::from(a)) * f).round() as u16;
            return Some(TilePos { x: lerp(w.from.x, target.tile.x), y: lerp(w.from.y, target.tile.y) });
        }
    }
    Some(p.tile)
}

fn cheb(a: TilePos, b: TilePos) -> u32 {
    let dx = (i32::from(a.x) - i32::from(b.x)).unsigned_abs();
    let dy = (i32::from(a.y) - i32::from(b.y)).unsigned_abs();
    dx.max(dy)
}

/// The doors of every Home and workplace (a building someone lives or works in), as a grid; rebuilt on the hour.
fn door_grid(world: &mut World) {
    let now = world.tick;
    if world.ring.doors.as_ref().is_some_and(|(at, _, _)| now / TICKS_PER_HOUR == at / TICKS_PER_HOUR) {
        return;
    }
    let mut ids = Vec::new();
    let mut doors = Vec::new();
    for b in world.with::<Building>() {
        if world.residents_of(b).is_empty() && world.staff_of(b).is_empty() {
            continue;
        }
        if let Some(bd) = world.comp::<Building>(b) {
            ids.push(b);
            doors.push(bd.door);
        }
    }
    let grid = TileGrid::build(world.map.w(), world.map.h(), 16, doors.iter().copied());
    world.ring.doors = Some((now, ids, grid));
}

/// Let every held body go (the view is gone or off).
fn release_all(world: &mut World) {
    let held = std::mem::take(&mut world.ring.held);
    for (id, h) in held {
        let_go(world, id, h);
    }
}

/// Back to the tier the body had when the ring took it (a pinned body keeps Full), if nobody moved it since; to
/// Statistical at its schedule's door (not its phase's: a worker let go on shift is not sent home).
fn let_go(world: &mut World, id: EntityId, h: Hold) {
    let Some(b) = world.comp::<Brain>(id) else { return };
    let base = if b.pinned { Lod::Full } else { h.base };
    if b.lod == h.tier && base != h.tier {
        let at = if base == Lod::Statistical { schedule_door(world, id) } else { None };
        crate::systems::lod::set_lod_at(world, id, base, at);
    }
}

/// `r` moved by `(dx, dy)` tiles, clipped to the map.
fn shift(r: Rect, dx: i32, dy: i32, w: usize, h: usize) -> Rect {
    let x = (i32::from(r.x) + dx).clamp(0, w as i32 - 1);
    let y = (i32::from(r.y) + dy).clamp(0, h as i32 - 1);
    let x1 = (i32::from(r.x) + i32::from(r.w) + dx).clamp(x, w as i32);
    let y1 = (i32::from(r.y) + i32::from(r.h) + dy).clamp(y, h as i32);
    Rect { x: x as u16, y: y as u16, w: (x1 - x) as u16, h: (y1 - y) as u16 }
}

/// The smallest rect holding both.
fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let x1 = (u32::from(a.x) + u32::from(a.w)).max(u32::from(b.x) + u32::from(b.w));
    let y1 = (u32::from(a.y) + u32::from(a.h)).max(u32::from(b.y) + u32::from(b.h));
    Rect { x, y, w: (x1 - u32::from(x)) as u16, h: (y1 - u32::from(y)) as u16 }
}

/// S2 (`OPEN-312`, `OPEN-318`): the ring run now, between two ticks, on a view that is new or cut (a load, a
/// possession, the camera on another body): every body in it is promoted and placed before a frame is drawn, so
/// the first frame shows no Statistical or Coarse body in the camera's sight (the tick's own run comes a frame
/// late). No lead: a cut is not a camera move. `World::view_now` is its caller.
pub fn prime(world: &mut World) {
    world.ring.last_centre = None;
    run(world);
    crate::systems::bind::drain_queue(world);
}

/// The ring's tick (see the module doc): after the hourly assignment, before the Statistical hour.
pub fn run(world: &mut World) {
    let t0 = std::time::Instant::now();
    run_inner(world);
    world.ring.counts.micros = t0.elapsed().as_micros() as u64;
}

fn run_inner(world: &mut World) {
    let view = match world.view_rect {
        Some(v) if world.config.lod.force.is_none() => v,
        _ => {
            if !world.ring.held.is_empty() {
                release_all(world);
                world.ring.counts = RingCounts { tick: world.tick, ..Default::default() };
            }
            return;
        }
    };
    let cfg = world.config.lod.clone();
    if view.w.max(view.h) > cfg.ring_max_side_tiles {
        if !world.ring.held.is_empty() {
            release_all(world);
        }
        world.ring.counts = RingCounts { tick: world.tick, ..Default::default() };
        return;
    }
    let (w, h) = (world.map.w(), world.map.h());
    // The lead: the view is set before the tick from where the camera's subject stood, and the frame is drawn after
    // it where the subject stands then (a walker 21 tiles on, a rider 50): the ring reaches as far ahead as the
    // camera moved last tick (a jump past `ring_lead_max_tiles` is a cut, no lead).
    let now_centre = TilePos { x: view.x + view.w / 2, y: view.y + view.h / 2 };
    let (dx, dy) = match world.ring.last_centre.replace(now_centre) {
        Some(c) => (i32::from(now_centre.x) - i32::from(c.x), i32::from(now_centre.y) - i32::from(c.y)),
        None => (0, 0),
    };
    let lead = dx.unsigned_abs().max(dy.unsigned_abs());
    let (dx, dy) = if lead > u32::from(cfg.ring_lead_max_tiles) { (0, 0) } else { (dx, dy) };
    let here = grow(view, crate::systems::lod::VIEW_MARGIN as u16, w, h);
    let ring = union(here, shift(here, dx, dy, w, h));
    let band = grow(ring, cfg.ring_band_tiles, w, h);
    let ring_hold = grow(ring, cfg.ring_hold_tiles, w, h);
    let band_hold = grow(band, cfg.ring_hold_tiles, w, h);
    // ranked from where the camera will be
    let centre = TilePos {
        x: (i32::from(now_centre.x) + dx).clamp(0, w as i32 - 1) as u16,
        y: (i32::from(now_centre.y) + dy).clamp(0, h as i32 - 1) as u16,
    };
    let reach = u32::from(band.w.max(band.h)) / 2 + 1;
    let band_centre = TilePos { x: band.x + band.w / 2, y: band.y + band.h / 2 };

    // Every candidate's place: the held, the bodies standing near, the riders, and the Statistical bodies whose
    // schedule's building stands in the band.
    let mut place: BTreeMap<EntityId, (TilePos, Option<EntityId>)> = BTreeMap::new();
    let held: Vec<EntityId> = world.ring.held.keys().copied().collect();
    let mut gone = Vec::new();
    for id in held {
        if !world.is_alive(id) || !world.has::<Brain>(id) {
            gone.push(id);
            continue;
        }
        if let Some(t) = body_place(world, id) {
            place.insert(id, (t, None));
        }
    }
    for id in gone {
        world.ring.held.remove(&id);
    }
    let mut stale = Vec::new();
    let near = world.agents_near(band_centre, reach, None);
    for id in near {
        if place.contains_key(&id) {
            continue;
        }
        let Some(b) = world.comp::<Brain>(id) else { continue };
        if b.lod == Lod::Statistical {
            let Some(p) = world.comp::<Position>(id).map(|p| p.tile) else { continue };
            if !band.contains(p) {
                continue;
            }
            match schedule_door(world, id) {
                Some(d) => {
                    let door = world.comp::<Building>(d).map_or(p, |bd| bd.door);
                    if door != p {
                        stale.push((id, d));
                    }
                    if band.contains(door) {
                        place.insert(id, (door, Some(d)));
                    }
                }
                None => {
                    place.insert(id, (p, None));
                }
            }
        } else if let Some(t) = body_place(world, id).filter(|t| band.contains(*t)) {
            place.insert(id, (t, None));
        }
    }
    // Coarse walkers who set out from outside the band and are in it now.
    for &id in world.tier(Lod::Coarse) {
        if place.contains_key(&id) {
            continue;
        }
        if let Some(t) = body_place(world, id).filter(|t| band.contains(*t)) {
            place.insert(id, (t, None));
        }
    }
    for m in world.motions.values() {
        let t = match &m.route {
            crate::systems::motion::Route::Timed { way, depart, arrive } => {
                crate::systems::motion::timed_pos(way, *depart, *arrive, world.tick)
            }
            _ => m.pos,
        };
        if band.contains(t) {
            for &r in &m.riders {
                place.entry(r).or_insert((t, None));
            }
        }
    }
    door_grid(world);
    let mut idx = Vec::new();
    if let Some((_, ids, grid)) = world.ring.doors.as_ref() {
        grid.near(band_centre, reach, &mut idx);
        let mut stat_from = Vec::new();
        for &i in &idx {
            let b = ids[i as usize];
            let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else { continue };
            if !band.contains(door) {
                continue;
            }
            for &id in world.residents_of(b).iter().chain(world.staff_of(b)) {
                if !place.contains_key(&id) && world.comp::<Brain>(id).is_some_and(|br| br.lod == Lod::Statistical) {
                    stat_from.push(id);
                }
            }
        }
        for id in stat_from {
            if place.contains_key(&id) {
                continue;
            }
            if let Some(d) = schedule_door(world, id) {
                if let Some(door) = world.comp::<Building>(d).map(|bd| bd.door).filter(|t| band.contains(*t)) {
                    place.insert(id, (door, Some(d)));
                }
            }
        }
    }

    // Who wants what: Full in the ring (a held Full one until past the ring's hold), Coarse in the band (a held one
    // until past the band's hold); ranked by camera distance, then a story step, then id.
    let mut want_full = Vec::new();
    let mut want_coarse = Vec::new();
    for (&id, &(t, _)) in &place {
        let hold = world.ring.held.get(&id).copied();
        let in_ring = ring.contains(t) || (hold.is_some_and(|h| h.tier == Lod::Full) && ring_hold.contains(t));
        let in_band = band.contains(t) || (hold.is_some() && band_hold.contains(t));
        let story = world
            .comp::<Brain>(id)
            .and_then(|b| b.current_step())
            .is_some_and(|s| crate::systems::lod::story_relevant(s.action));
        let key = (cheb(t, centre), !story, id);
        if in_ring {
            want_full.push(key);
        } else if in_band {
            want_coarse.push(key);
        } else if hold.is_some_and(|h| h.base == Lod::Statistical) {
            // A body gone off on its own from a Home or a workplace in the band: let go, its schedule would stand it
            // back at that door in the camera's sight (and the ring take it again): it keeps its body while its
            // schedule's door stays in the band, ranked from that door.
            let door = schedule_door(world, id).and_then(|d| world.comp::<Building>(d)).map(|b| b.door);
            if let Some(door) = door.filter(|d| band_hold.contains(*d)) {
                want_coarse.push((cheb(door, centre), !story, id));
            }
        }
    }
    want_full.sort_unstable();
    let over_full = want_full.len().saturating_sub(cfg.ring_full_max);
    want_coarse.extend(want_full.drain(cfg.ring_full_max.min(want_full.len())..));
    want_coarse.sort_unstable();
    let over_coarse = want_coarse.len().saturating_sub(cfg.ring_coarse_max);
    want_coarse.truncate(cfg.ring_coarse_max);

    let mut target: BTreeMap<EntityId, Lod> = BTreeMap::new();
    for &(_, _, id) in &want_full {
        target.insert(id, Lod::Full);
    }
    for &(_, _, id) in &want_coarse {
        target.insert(id, Lod::Coarse);
    }

    let mut c = RingCounts { tick: world.tick, over_full, over_coarse, lead, ..Default::default() };
    // Let go of the held who want nothing now.
    let release: Vec<(EntityId, Hold)> =
        world.ring.held.iter().filter(|(id, _)| !target.contains_key(id)).map(|(&id, &h)| (id, h)).collect();
    for (id, h) in release {
        world.ring.held.remove(&id);
        let_go(world, id, h);
        c.released += 1;
    }
    // Take the newcomers, move the held between Full and Coarse.
    for (&id, &t) in &target {
        let Some(now) = world.comp::<Brain>(id).map(|b| b.lod) else { continue };
        match world.ring.held.get(&id).copied() {
            Some(h) => {
                let tier = higher(t, h.base);
                if tier != h.tier {
                    if now != tier {
                        crate::systems::lod::set_lod(world, id, tier);
                    }
                    world.ring.held.insert(id, Hold { tier, base: h.base });
                    c.moved += 1;
                }
            }
            None => {
                let tier = higher(t, now);
                if tier != now {
                    match place.get(&id).and_then(|&(_, b)| b) {
                        // S2 (`OPEN-312`): a Statistical body with no place of its own (homeless off shift) is
                        // promoted where it stands, not snapped to a phase door in the camera's sight.
                        None if now == Lod::Statistical => crate::systems::lod::set_lod_here(world, id, tier),
                        at => crate::systems::lod::set_lod_at(world, id, tier, at),
                    }
                }
                world.ring.held.insert(id, Hold { tier, base: now });
                c.taken += 1;
            }
        }
    }
    // A stale snapshot standing in the band and left Statistical: to its schedule's door (it is not where it
    // stands; one taken was placed by its promotion).
    for (id, d) in stale {
        if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
            world.stand_at_door(id, d);
            c.replaced += 1;
        }
    }
    for (&id, &(t, _)) in &place {
        if !ring.contains(t) {
            continue;
        }
        match world.comp::<Brain>(id).map(|b| b.lod) {
            Some(Lod::Full) => c.in_full += 1,
            Some(Lod::Coarse) => c.in_coarse += 1,
            Some(Lod::Statistical) => c.in_stat += 1,
            None => {}
        }
    }
    for h in world.ring.held.values() {
        match h.tier {
            Lod::Full => c.full += 1,
            Lod::Coarse => c.coarse += 1,
            Lod::Statistical => {}
        }
    }
    world.ring.counts = c;
}
