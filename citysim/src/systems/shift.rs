//! The pre-view pass phase 2 (docs/PREVIEW_PASS.md § 0 P1, P2, P4, P5, P7,
//! § 1; OPEN-137): the visible shift. A shift is worked in a **shape**
//! (`config::ShiftShape`): a trade row's `duty` (`post` is worked as
//! `[shift] trade_post`) or a built-in role's `[shift.roles]` key. `post` is
//! the old shift, one work action to the shift's end at one tile.
//!
//! **`stations`** (P2): 2-`station_spots_max` distinct interior tiles of the
//! workplace, drawn per body per shift (`draw_spots`, purpose-keyed, doctrine
//! 5), worked in `station_block_min`..`station_block_max` minute blocks with a
//! `GoTo(Station)` across the room between them (the role's own work action
//! in each block: the shadow tool still reads it as work, P4). A shift is
//! three shape plans, each the Work goal's bypass (`plan::plan_for`, after
//! the commute, the round, the beat and the fare): the first half's blocks to
//! mid-shift, the break (P7: hungry, a meal at the nearest open Market or the
//! packed one outside the door; fed, `break_min` outside the door), and the
//! second half's blocks, the last of which runs to the shift's end.
//!
//! **Pay and output do not move** (P5): a block before the last completes
//! without the shift's close (`block_done`); the last one closes it at the
//! post as before (`exec::actions::end_shift`: the wage, `last_shift_day`),
//! and a producing trade's output is accrued over the shift's span from its
//! first block (`span_start`), as the one block was; a pro-rata cut reads the
//! same span (`fixes::work_aborted`).
//!
//! **Statistical mirror**: none for the movement. A Statistical body is off
//! camera by definition (OPEN-149) and has no plan: it keeps the one block
//! and the same pay (`plan` never runs for it).
//!
//! **Where phases 3-6 plug in**: a new `ShiftShape` (`haul`, `counter`) and
//! its `TradeDuty` arm in `shape_of`; its plan builder as an arm of `plan`'s
//! match; a block that is not the shift's last keeps returning `true` from
//! `block_done` (its `ShiftState.blocks` entry non-zero) so the wage stays at
//! the shift's close. `round` and `fare` are worked by `trades::round_plan`
//! and `streetwork::fare_plan` today (their arms here when they move).

use crate::components::{ActionInstance, Brain, Building, GoalKind, Inventory, Job, Lod, Position, Role, ShiftState};
use crate::components::{BuildingKind, TilePos, Wallet};
use crate::config::{ShiftShape, TradeDuty};
use crate::entity::EntityId;
use crate::exec::routine;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::time::Tick;
use crate::world::World;
use rand::Rng;

/// The keyed-stream prefix of the shift draws (bits 44-59; `SimRng::keyed`
/// keys stay below 2^60).
pub const SHIFT_KEY: u64 = 0x5A1F << 44;
/// The purposes under `SHIFT_KEY`: the spots, a block's length.
const PURPOSE_SPOTS: u64 = 1;
const PURPOSE_BLOCK: u64 = 2;
const PURPOSE_BREAK: u64 = 3;

/// `[shift]` is on (not `CITYSIM_STATIONS_OFF`).
pub fn on(world: &World) -> bool {
    world.config.shift.is_some()
}

/// The pre-view pass phase 3: `[shift.haul]` is on (not `CITYSIM_HAUL_COUNTER_OFF`).
pub fn haul_on(world: &World) -> bool {
    world.config.shift.as_ref().is_some_and(|c| c.haul.is_some())
}

/// The pre-view pass phase 3: `[shift.counter]` is on (not `CITYSIM_HAUL_COUNTER_OFF`).
pub fn counter_on(world: &World) -> bool {
    world.config.shift.as_ref().is_some_and(|c| c.counter.is_some())
}

/// P1: the shape `role`'s shift is worked in (`post` with `[shift]` off):
/// a trade row's `shape` (phase 3: `haul`, `counter`, live with their
/// section), else its duty's (`post` as `trade_post`); a built-in role in
/// `[shift.counter] roles` the counter, else its `[shift.roles]` key.
pub fn shape_of(world: &World, role: Role) -> ShiftShape {
    let Some(cfg) = world.config.shift.as_ref() else { return ShiftShape::Post };
    let live = |s: ShiftShape| match s {
        ShiftShape::Haul => cfg.haul.is_some(),
        ShiftShape::Counter => cfg.counter.is_some(),
        _ => true,
    };
    match role {
        Role::Trade(_) => match crate::systems::trades::row_of(world, role) {
            Some(r) if r.shape.is_some_and(live) => r.shape.unwrap_or_default(),
            Some(r) if r.duty == TradeDuty::Post => cfg.trade_post,
            _ => ShiftShape::Post,
        },
        r => {
            let name = format!("{r:?}");
            if cfg.counter.as_ref().is_some_and(|c| c.roles.contains(&name)) {
                return ShiftShape::Counter;
            }
            cfg.roles.get(&name).copied().unwrap_or_default()
        }
    }
}

/// A keyed draw for `id`'s shift `key`: `purpose`, its `ordinal`th.
fn draw(world: &World, id: EntityId, key: i64, purpose: u64, ordinal: u64) -> rand_chacha::ChaCha8Rng {
    let h = crate::rng::splitmix64(
        u64::from(id.index) ^ (key as u64).rotate_left(24) ^ (purpose << 56) ^ ordinal.rotate_left(40),
    );
    world.rng.keyed(SHIFT_KEY | (h & ((1 << 44) - 1)))
}

/// P2: the spots of `id`'s shift `key` at `post`: 2..=`station_spots_max`
/// distinct walkable interior tiles of its footprint (fewer than 2: none).
fn draw_spots(world: &World, id: EntityId, post: EntityId, key: i64, max: u8) -> Vec<TilePos> {
    let Some(bd) = world.comp::<Building>(post) else { return Vec::new() };
    let mut tiles: Vec<TilePos> = bd.interior().filter(|&t| world.map.walkable(t)).collect();
    if tiles.len() < 2 {
        return Vec::new();
    }
    let mut r = draw(world, id, key, PURPOSE_SPOTS, 0);
    let hi = usize::from(max.max(2)).min(tiles.len());
    let n = r.random_range(2..=hi);
    // A partial Fisher-Yates: the first `n` of a seeded shuffle.
    for i in 0..n {
        let j = r.random_range(i..tiles.len());
        tiles.swap(i, j);
    }
    tiles.truncate(n);
    tiles
}

/// A block's bounds in minutes for `role` (its trade row's override, else `[shift]`).
fn block_bounds(world: &World, role: Role) -> (u64, u64) {
    let Some(cfg) = world.config.shift.as_ref() else { return (0, 0) };
    let row = crate::systems::trades::row_of(world, role);
    let lo = row.map_or(0, |r| r.station_block_min);
    let hi = row.map_or(0, |r| r.station_block_max);
    let lo = if lo == 0 { cfg.station_block_min } else { lo };
    let hi = if hi == 0 { cfg.station_block_max } else { hi };
    (u64::from(lo.max(1)), u64::from(hi.max(lo.max(1))))
}

/// The start of the shift segment holding `tick` (a night shift's tail from
/// 00:00 starts the evening before).
fn segment_start(job: &Job, tick: Tick) -> Option<Tick> {
    let tod = (tick % 1440) as u16;
    let &(s, _) = job.shifts.iter().find(|&&(s, e)| tod >= s && tod < e)?;
    let start = tick - Tick::from(tod - s);
    if s == 0 {
        if let Some(&(s0, _)) = job.shifts.iter().find(|&&(_, e)| e == 1440) {
            return Some(start.saturating_sub(Tick::from(1440 - s0)));
        }
    }
    Some(start)
}

/// P2: a walk across the room between two spots: the tiles at `[exec]
/// move_ticks_full`, at least one tick, at most `[shift] station_move_max`.
pub fn move_ticks(world: &World, from: TilePos, to: TilePos) -> Tick {
    let max = world.config.shift.as_ref().map_or(1, |c| Tick::from(c.station_move_max.max(1)));
    (Tick::from(from.manhattan(to)) * world.config.exec.move_ticks_full).clamp(1, max)
}

fn step(action: ActionKind, target: Option<EntityId>, tile: Option<TilePos>) -> ActionInstance {
    ActionInstance { action, target, tile }
}

/// The Work goal's bypass (`plan::plan_for`): the next part of a shaped
/// shift for `id` on shift, or `None` (the planner's one block: a `post`
/// shape, a Statistical body, a workplace under two interior tiles).
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    // The cheap tests first, on borrows: most callers are off shift or `post`.
    let (shape, post, key, end, start, role) = {
        world.config.shift.as_ref()?;
        let job = world.comp::<Job>(id)?;
        let post = job.employer?;
        if !job.on_shift(world.tick_of_day()) {
            return None;
        }
        let shape = shape_of(world, job.role);
        if shape == ShiftShape::Post {
            return None;
        }
        let key = job.next_shift_key(world.tick);
        if job.last_shift_day == Some(key) || !routine::workday_of(world, id, job, key) {
            return None;
        }
        if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
            return None;
        }
        (shape, post, key, job.shift_end(world.tick)?, segment_start(job, world.tick)?, job.role)
    };
    match shape {
        ShiftShape::Stations => stations_plan(world, id, post, key, start, end, role, false),
        ShiftShape::Counter => stations_plan(world, id, post, key, start, end, role, true),
        ShiftShape::Haul => haul_plan(world, id, post, key, start, end, role),
        ShiftShape::Post => None,
    }
}

/// The pre-view pass phase 3 (`counter`): the tile `role` stands at `post`:
/// a door keeper (a fighting trade: the bouncer) the interior tile nearest
/// the door, anyone else the one farthest from it (behind the counter);
/// ties by tile. `None` with no walkable interior tile.
fn counter_tile(world: &World, post: EntityId, role: Role) -> Option<TilePos> {
    let bd = world.comp::<Building>(post)?;
    let door = bd.door;
    let keeper =
        crate::systems::trades::row_of(world, role).is_some_and(|r| r.skill == crate::config::TradeSkill::Fighting);
    let tiles = bd.interior().filter(|&t| world.map.walkable(t));
    if keeper {
        tiles.min_by_key(|&t| (t.manhattan(door), t))
    } else {
        tiles.max_by_key(|&t| (t.manhattan(door), std::cmp::Reverse(t)))
    }
}

/// P7 for the haul and the counter: the break's tick of `id`'s shift `key`
/// (mid-shift, staggered per body per shift by up to `break_stagger`).
fn break_mid(world: &World, id: EntityId, key: i64, start: Tick, end: Tick) -> Tick {
    let stagger = world.config.shift.as_ref().map_or(0, |c| c.break_stagger);
    let half = end.saturating_sub(start) / 2;
    let s = i64::from(stagger).min(half as i64 / 2);
    let jitter = if s > 0 { draw(world, id, key, PURPOSE_BREAK, 0).random_range(-s..=s) } else { 0 };
    (start + half).saturating_add_signed(jitter)
}

/// The pre-view pass phase 3 (`haul`): the stock tile (the interior tile
/// nearest the door, where loads are made up) and the drop tiles of an
/// in-building trip (up to `station_spots_max - 1` more, keyed per body per
/// shift): the stock tile first.
fn haul_spots(world: &World, id: EntityId, post: EntityId, key: i64) -> Vec<TilePos> {
    let Some(bd) = world.comp::<Building>(post) else { return Vec::new() };
    let door = bd.door;
    let mut tiles: Vec<TilePos> = bd.interior().filter(|&t| world.map.walkable(t)).collect();
    tiles.sort_by_key(|&t| (t.manhattan(door), t));
    if tiles.is_empty() {
        return tiles;
    }
    let stock = tiles.remove(0);
    let max = world.config.shift.as_ref().map_or(2, |c| usize::from(c.station_spots_max.max(2))) - 1;
    let mut r = draw(world, id, key, PURPOSE_SPOTS, 1);
    let n = max.min(tiles.len());
    for i in 0..n {
        let j = r.random_range(i..tiles.len());
        tiles.swap(i, j);
    }
    tiles.truncate(n);
    let mut out = vec![stock];
    out.extend(tiles);
    out
}

/// The pre-view pass phase 3 (`haul`): the shift's destinations: a
/// courier's addresses (his share of the Yard's parcels, OPEN-141); a
/// Construction Yard's open sites in `haul_reach`; a Freight Yard's, a
/// Farm's or a Warehouse's nearest standing staffed workplaces (not a Home,
/// not the post) in reach; nearest first, `haul_dests_max` (the courier's all).
/// None (a Market's night stocker; no parcel; nothing in reach): the trips
/// run inside the post.
pub fn haul_dests(world: &World, id: EntityId, post: EntityId) -> Vec<EntityId> {
    let Some(hc) = world.config.shift.as_ref().and_then(|c| c.haul.as_ref()) else { return Vec::new() };
    let Some(bd) = world.comp::<Building>(post) else { return Vec::new() };
    let door = bd.door;
    if crate::systems::streetwork::haul_courier(world, id) {
        return crate::systems::streetwork::courier_addresses(world, id);
    }
    let cands: Vec<EntityId> = match bd.kind {
        BuildingKind::ConstructionYard => crate::systems::industry::sites_of(world, post),
        // The goods go where people work: the staffed workplaces (not Homes)
        // in reach (2k: the Yard's nearest Market is 56 tiles off).
        BuildingKind::FreightYard | BuildingKind::Farm | BuildingKind::Warehouse => world
            .buildings_by_kind
            .iter()
            .filter(|(k, _)| **k != BuildingKind::Home)
            .flat_map(|(_, v)| v.iter().copied())
            .filter(|&b| b != post && !world.staff_of(b).is_empty())
            .filter(|&b| world.comp::<Building>(b).is_some_and(|x| !x.demolished && !x.derelict))
            .collect(),
        _ => Vec::new(),
    };
    let mut near: Vec<(u32, EntityId)> = cands
        .into_iter()
        .filter_map(|b| world.comp::<Building>(b).map(|x| (x.door.manhattan(door), b)))
        .filter(|&(d, _)| d <= hc.haul_reach)
        .collect();
    near.sort_unstable();
    near.into_iter().take(usize::from(hc.haul_dests_max)).map(|(_, b)| b).collect()
}

/// The pre-view pass phase 3 (`haul`, § 1): the haul shift's next part:
/// a trip (load at the stock tile, carry, unload, the next call walks
/// back), the break at mid-shift (P7), or, when no trip fits before
/// `haul_cutoff_min` of the shift's end (or a courier has no parcel left
/// to try), the close at the post's stock tile to the shift's end (paid).
fn haul_plan(
    world: &mut World,
    id: EntityId,
    post: EntityId,
    key: i64,
    start: Tick,
    end: Tick,
    _role: Role,
) -> Option<Plan> {
    let now = world.tick;
    let (handle, cutoff, break_min) = {
        let c = world.config.shift.as_ref()?;
        let h = c.haul.as_ref()?;
        (Tick::from(h.haul_handle_min.max(1)), Tick::from(h.haul_cutoff_min), Tick::from(c.break_min))
    };
    let fresh = world.comp::<Brain>(id)?.shift.as_ref().is_none_or(|s| s.key != key);
    if fresh {
        let spots = haul_spots(world, id, post, key);
        let dests = haul_dests(world, id, post);
        if spots.is_empty() || (dests.is_empty() && spots.len() < 2) {
            return None;
        }
        let st = ShiftState { key, spots, dests, from: now, ..ShiftState::default() };
        world.comp_mut::<Brain>(id)?.shift = Some(Box::new(st));
    }
    let mid = break_mid(world, id, key, start, end);
    let stage = world.comp::<Brain>(id)?.shift.as_ref()?.stage;
    // P7: the break once the first half is worked, if the shift has room.
    if stage == 0 && !fresh && now >= mid && end.saturating_sub(now) >= break_min + cutoff {
        let (steps, meal) = break_steps(world, id, post);
        let s = world.comp_mut::<Brain>(id)?.shift.as_mut()?;
        s.stage = 1;
        s.meal = meal;
        s.blocks.clear();
        s.plan_tick = now;
        crate::systems::ownership::jobs2_row(world).shift_breaks += 1;
        return Some(Plan { goal: GoalKind::Work, target: Some(post), steps, started_tick: now });
    }
    // Before the break, the first half; after it (or past mid-shift with
    // no room for it, or come on after mid-shift), the second.
    let next_stage = if stage == 0 && now < mid { 0 } else { 2 };
    let job = world.comp::<Job>(id)?.clone();
    let pos = world.comp::<Position>(id)?.clone();
    let post_door = world.comp::<Building>(post)?.door;
    let inside = pos.building == Some(post);
    let (spots, dests, drawn) = {
        let s = world.comp::<Brain>(id)?.shift.as_ref()?;
        (s.spots.clone(), s.dests.clone(), s.drawn)
    };
    let courier = crate::systems::streetwork::haul_courier(world, id);
    // The trip's destination: the next of the shift's (a courier's next
    // address with a parcel still pending, each tried twice at most), else
    // a drop tile inside the post (a courier with his parcels done works
    // the Yard's floor).
    let inside_trip = || {
        (spots.len() >= 2).then(|| {
            let n = (spots.len() - 1) as u8;
            (post, spots.get(1 + usize::from(drawn % n)).copied(), drawn.wrapping_add(1))
        })
    };
    let dest: Option<(EntityId, Option<TilePos>, u8)> = if dests.is_empty() {
        inside_trip()
    } else if courier {
        let n = dests.len();
        (usize::from(drawn)..2 * n)
            .find(|&k| crate::systems::streetwork::parcels_for(world, id, dests[k % n]) > 0)
            .map(|k| (dests[k % n], None, u8::try_from(k + 1).unwrap_or(u8::MAX)))
            .or_else(inside_trip)
    } else {
        Some((dests[usize::from(drawn) % dests.len()], None, drawn.wrapping_add(1)))
    };
    // The trip's estimate: to the post (when out), the load, the carry out
    // and back, the unload.
    let mt = world.config.exec.move_ticks_full.max(1);
    let back = if inside { 0 } else { crate::systems::fixes::walk_estimate(world, id, post_door) };
    let trip = dest.map(|(b, _, _)| {
        let d = world.comp::<Building>(b).map_or(0, |x| Tick::from(x.door.manhattan(post_door)) * mt);
        // A quarter over: a street walk is longer than the doors' distance.
        (back + 2 * handle + 2 * d) * 5 / 4
    });
    let close = match trip {
        Some(t) => now + t + cutoff > end,
        None => true,
    };
    let mut steps = Vec::new();
    if !inside {
        steps.push(step(ActionKind::GoTo(routine::workplace_key_for(world, id, &job)), Some(post), None));
    }
    steps.push(step(ActionKind::GoTo(LocationKey::Station), Some(post), Some(spots[0])));
    let work = routine::work_action(world, job.role);
    let mut blocks = Vec::new();
    let mut new_drawn = drawn;
    if close {
        // The rest of the shift at the post (closed and paid at its end).
        steps.push(step(work, Some(post), None));
        blocks.push(0);
    } else if let Some((b, tile, k)) = dest {
        steps.push(step(work, Some(post), None));
        blocks.push(u16::try_from(handle).unwrap_or(u16::MAX));
        if b == post {
            steps.push(step(ActionKind::GoTo(LocationKey::Station), Some(post), tile));
        } else {
            // A driving trade takes the post's fleet truck (the GoTo drives
            // a vehicle it holds, `vehicles::vehicle_for_trip`).
            let drives = crate::systems::trades::row_of(world, job.role).is_some_and(|r| {
                world.config.shift.as_ref().and_then(|c| c.haul.as_ref()).is_some_and(|h| h.drive.contains(&r.id))
            });
            if drives && inside && crate::systems::vehicles::fleet_vehicle_at(world, post).is_some() {
                crate::systems::vehicles::claim_haul(world, id, post);
            }
            steps.push(step(ActionKind::GoTo(LocationKey::TargetHome), Some(b), None));
        }
        steps.push(step(ActionKind::Unload, Some(b), None));
        new_drawn = k;
    }
    let s = world.comp_mut::<Brain>(id)?.shift.as_mut()?;
    s.stage = if close { 2 } else { next_stage };
    s.drawn = new_drawn;
    s.blocks = blocks;
    s.plan_tick = now;
    Some(Plan { goal: GoalKind::Work, target: Some(post), steps, started_tick: now })
}

/// `exec::actions::duration` of an `Unload`: `[shift.haul] haul_handle_min`.
pub fn unload_ticks(world: &World) -> Tick {
    world.config.shift.as_ref().and_then(|c| c.haul.as_ref()).map_or(1, |h| Tick::from(h.haul_handle_min.max(1)))
}

/// `exec::actions::on_complete` of an `Unload` at `target`: the trip's
/// device rows; a courier's parcels for the address handed over to whoever
/// is there to sign (P3, `streetwork::courier_stop`), else they stay
/// (`parcels_missed`: narrated off screen, back at the shift's end).
pub fn unloaded(world: &mut World, id: EntityId, target: Option<EntityId>) {
    let Some(b) = target else { return };
    let load = world.config.shift.as_ref().and_then(|c| c.haul.as_ref()).map_or(0, |h| u32::from(h.haul_load));
    let post = world.comp::<Job>(id).and_then(|j| j.employer);
    if let Some(s) = world.comp_mut::<Brain>(id).and_then(|br| br.shift.as_mut()) {
        s.trips = s.trips.saturating_add(1);
    }
    if crate::systems::streetwork::haul_courier(world, id) && Some(b) != post {
        let before = crate::systems::streetwork::parcels_for(world, id, b);
        crate::systems::streetwork::courier_stop(world, id, b);
        if before > 0 && crate::systems::streetwork::parcels_for(world, id, b) == before {
            crate::systems::ownership::jobs2_row(world).parcels_missed += 1;
        }
    }
    let row = crate::systems::ownership::jobs2_row(world);
    row.haul_trips += 1;
    row.haul_units += load;
    row.haul_away += u32::from(Some(b) != post);
}

/// P2, P7: the stations shift's next part (§ 1). The pre-view pass phase 3
/// (`counter`): one spot, the counter tile (`counter_tile`), each half one
/// block; the break only while another counter body covers the post.
#[allow(clippy::too_many_arguments)]
fn stations_plan(
    world: &mut World,
    id: EntityId,
    post: EntityId,
    key: i64,
    start: Tick,
    end: Tick,
    role: Role,
    counter: bool,
) -> Option<Plan> {
    let now = world.tick;
    let (spots_max, break_min, stagger) =
        world.config.shift.as_ref().map(|c| (c.station_spots_max, c.break_min, c.break_stagger))?;
    let (bmin, bmax) = block_bounds(world, role);
    let bmax = if counter { Tick::MAX / 4 } else { bmax };
    let fresh = world.comp::<Brain>(id)?.shift.as_ref().is_none_or(|s| s.key != key);
    if fresh {
        let spots = if counter {
            counter_tile(world, post, role).into_iter().collect()
        } else {
            draw_spots(world, id, post, key, spots_max)
        };
        if spots.len() < if counter { 1 } else { 2 } {
            return None;
        }
        let st = ShiftState { key, spots, from: now, ..ShiftState::default() };
        world.comp_mut::<Brain>(id)?.shift = Some(Box::new(st));
    }
    // P7: the break at mid-shift, staggered per body per shift by up to
    // `break_stagger` minutes either way (a crew does not walk out at once).
    let half = end.saturating_sub(start) / 2;
    let jitter = {
        let s = i64::from(stagger).min(half as i64 / 2);
        if s > 0 {
            draw(world, id, key, PURPOSE_BREAK, 0).random_range(-s..=s)
        } else {
            0
        }
    };
    let mid = (start + half).saturating_add_signed(jitter);
    let stage = world.comp::<Brain>(id)?.shift.as_ref()?.stage;
    let mut next = stage;
    if stage == 0 {
        if fresh && now + bmin >= mid {
            // Came on after the first half: no break.
            next = 2;
        } else if !fresh && now + bmin >= mid {
            // The first half worked: the break, if the shift has room for it.
            next = if end.saturating_sub(now) >= Tick::from(break_min) + bmin { 1 } else { 2 };
        }
    } else if stage == 1 {
        next = 2;
    }
    // P7 (as built): a seller open only with its staff inside (a Clinic,
    // `fixes::staff_present`) is minded: its last doctor on shift inside
    // keeps it through the break (taken alone, 2k AssetBought -14 % and
    // fewer Ripperdocs and Orderlies employed).
    if next == 1 && !covered(world, id, post, counter) {
        next = 2;
    }
    // P7: a server does not walk out on a customer mid-procedure (an
    // implant on the table, a sale at the counter): the break waits a block
    // (a Clinic sells only with its doctor in, `fixes::staff_present`).
    let defer = next == 1 && mid_service(world, id, post);
    if next == 1 && !defer {
        let (steps, meal) = break_steps(world, id, post);
        let s = world.comp_mut::<Brain>(id)?.shift.as_mut()?;
        s.stage = 1;
        s.meal = meal;
        s.blocks.clear();
        s.plan_tick = now;
        let row = crate::systems::ownership::jobs2_row(world);
        row.shift_breaks += 1;
        return Some(Plan { goal: GoalKind::Work, target: Some(post), steps, started_tick: now });
    }
    // The blocks to mid-shift (the first half) or to the shift's end; a
    // deferred break, one more block of `station_block_min`.
    if defer {
        next = 0;
    }
    let target = if defer {
        now + bmin + Tick::from(world.config.shift.as_ref().map_or(1, |c| c.station_move_max))
    } else if next == 0 {
        mid
    } else {
        end
    };
    let job = world.comp::<Job>(id)?.clone();
    let pos = world.comp::<Position>(id)?.clone();
    let inside = pos.building == Some(post);
    let door = world.comp::<Building>(post)?.door;
    let mut steps = Vec::new();
    let mut t = now;
    let mut here = pos.tile;
    if !inside {
        steps.push(step(ActionKind::GoTo(routine::workplace_key_for(world, id, &job)), Some(post), None));
        t += crate::systems::fixes::walk_estimate(world, id, door);
        here = door;
    }
    let (spots, mut drawn) = {
        let s = world.comp::<Brain>(id)?.shift.as_ref()?;
        (s.spots.clone(), s.drawn)
    };
    let mut blocks = Vec::new();
    loop {
        let spot = spots[usize::from(drawn) % spots.len()];
        steps.push(step(ActionKind::GoTo(LocationKey::Station), Some(post), Some(spot)));
        t += move_ticks(world, here, spot);
        here = spot;
        let remaining = target.saturating_sub(t);
        let last = remaining <= bmax;
        let len = if last {
            remaining
        } else {
            let mut r = draw(world, id, key, PURPOSE_BLOCK, u64::from(drawn));
            r.random_range(bmin..=bmax).min(remaining - bmin)
        };
        drawn = drawn.wrapping_add(1);
        // The second half's last block runs to the shift's end (paid there).
        blocks.push(if last && next == 2 { 0 } else { u16::try_from(len.max(1)).unwrap_or(u16::MAX) });
        steps.push(step(routine::work_action(world, job.role), Some(post), None));
        t += len;
        if last || blocks.len() >= 32 {
            break;
        }
    }
    let s = world.comp_mut::<Brain>(id)?.shift.as_mut()?;
    s.stage = next;
    s.drawn = drawn;
    s.blocks = blocks;
    s.plan_tick = now;
    Some(Plan { goal: GoalKind::Work, target: Some(post), steps, started_tick: now })
}

/// P7: `post` stays open while `id` is on the break: it is no seller that
/// needs its staff inside (`assets::seller_open`'s Clinic rule), or another
/// of `id`'s role is on shift inside it.
/// The pre-view pass phase 3 (`counter`): a counter is covered by another
/// counter body of the post (any counter role) on shift inside it.
fn covered(world: &World, id: EntityId, post: EntityId, counter: bool) -> bool {
    if !counter && world.comp::<Building>(post).is_none_or(|b| b.kind != BuildingKind::Clinic) {
        return true;
    }
    let tod = world.tick_of_day();
    let Some(role) = world.comp::<Job>(id).map(|j| j.role) else { return false };
    world.staff_of(post).iter().any(|&s| {
        s != id
            && world.comp::<Job>(s).is_some_and(|j| {
                j.on_shift(tod) && if counter { shape_of(world, j.role) == ShiftShape::Counter } else { j.role == role }
            })
            && world.comp::<Position>(s).is_some_and(|p| p.building == Some(post))
    })
}

/// P7: a customer of `post` other than `id` is mid-procedure there (an
/// implant, a sale, a treatment running).
fn mid_service(world: &World, id: EntityId, post: EntityId) -> bool {
    let Some(bd) = world.comp::<Building>(post) else { return false };
    bd.occupants.iter().any(|&o| {
        o != id
            && world.comp::<Job>(o).is_none_or(|j| j.employer != Some(post))
            && world.comp::<Brain>(o).is_some_and(|b| {
                matches!(
                    b.exec,
                    crate::exec::ExecState::Use {
                        kind: ActionKind::BuyAsset
                            | ActionKind::Install
                            | ActionKind::Therapy
                            | ActionKind::Uninstall
                            | ActionKind::Detox
                            | ActionKind::Oncology
                            | ActionKind::Stabilise,
                        ..
                    }
                )
            })
    })
}

/// P7: the break's steps, and whether it is a meal: hungry (under the
/// planner's `HUNGER_SATISFIED`), the packed meal outside the door or a meal
/// at the nearest open food venue within `break_walk_max` (`food_venue`); fed
/// (or neither to hand), `break_min` outside the door.
fn break_steps(world: &World, id: EntityId, _post: EntityId) -> (Vec<ActionInstance>, bool) {
    let outside = || vec![step(ActionKind::GoTo(LocationKey::Street), None, None), step(ActionKind::Break, None, None)];
    if routine::fed(world, id) {
        return (outside(), false);
    }
    if world.comp::<Inventory>(id).is_some_and(|i| i.food > 0) {
        let steps = vec![
            step(ActionKind::GoTo(LocationKey::Street), None, None),
            step(ActionKind::EatFromInventory, None, None),
            step(ActionKind::Break, None, None),
        ];
        return (steps, true);
    }
    match food_venue(world, id) {
        Some((m, BuildingKind::Market)) => {
            let steps = vec![
                step(ActionKind::GoTo(LocationKey::Market), Some(m), None),
                step(ActionKind::BuyFood, Some(m), None),
                step(ActionKind::EatFromInventory, None, None),
            ];
            (steps, true)
        }
        // The NoodleBar's meal (the leisure `EatOut`, the cook's counter).
        Some((m, _)) => {
            let steps = vec![
                step(ActionKind::GoTo(LocationKey::Seller), Some(m), None),
                step(ActionKind::EatOut, Some(m), None),
            ];
            (steps, true)
        }
        None => (outside(), false),
    }
}

/// P7: the nearest open food venue `id` can pay at, a Market (`BuyFood`)
/// or a NoodleBar (`EatOut`), within `break_walk_max` minutes' walk (ties
/// by id).
fn food_venue(world: &World, id: EntityId) -> Option<(EntityId, BuildingKind)> {
    let walk_max = world.config.shift.as_ref().map_or(0, |c| Tick::from(c.break_walk_max));
    let from = crate::exec::walk_origin(world, id)?;
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let reach = walk_max / world.config.exec.move_ticks_full.max(1);
    let mut best: Option<(Tick, u32, EntityId, BuildingKind)> = None;
    for kind in [BuildingKind::Market, BuildingKind::NoodleBar] {
        for &m in world.buildings_by_kind.get(&kind).map_or(&[][..], |v| v.as_slice()) {
            let Some(b) = world.comp::<Building>(m) else { continue };
            if Tick::from(b.door.manhattan(from)) > reach || b.stock_food == 0 || b.demolished || b.derelict {
                continue;
            }
            let ok = match kind {
                BuildingKind::Market => !world.is_closed(m) && coins >= crate::systems::diet::price_at(world, id, m),
                _ => {
                    crate::systems::leisure::open(world, m)
                        && crate::systems::leisure::door_ok(world, id, m)
                        && coins >= crate::systems::leisure::price_of(world, m).max(1)
                }
            };
            if !ok {
                continue;
            }
            let walk = crate::systems::fixes::walk_estimate(world, id, b.door);
            if walk <= walk_max && best.is_none_or(|(w, i, _, _)| (walk, m.index) < (w, i)) {
                best = Some((walk, m.index, m, kind));
            }
        }
    }
    best.map(|(_, _, m, k)| (m, k))
}

/// The shift state behind `id`'s running plan, when the plan is a shape
/// plan, with the plan's current step index.
fn shaped(world: &World, id: EntityId) -> Option<(&ShiftState, &Plan, usize)> {
    let b = world.comp::<Brain>(id)?;
    let s = b.shift.as_deref()?;
    let p = b.plan.as_ref()?;
    (p.goal == GoalKind::Work && p.started_tick == s.plan_tick).then_some((s, p, usize::from(b.plan_step)))
}

/// `exec::start_step`: `id`'s running plan is a shape plan (its steps
/// check `exec::actions::can_start` only, as a scripted plan's do).
pub fn scripted(world: &World, id: EntityId) -> bool {
    shaped(world, id).is_some()
}

/// The block (minutes; 0 the last, to the shift's end) of the work step at
/// `i` in a shape plan.
fn block_at(s: &ShiftState, p: &Plan, i: usize) -> Option<u16> {
    let n = p.steps[..i.min(p.steps.len())].iter().filter(|st| st.action.is_work()).count();
    s.blocks.get(n).copied()
}

/// `exec::actions::duration` of a work step: a stations block's length
/// (capped at the shift's end), or `None` (to the shift's end).
pub fn block_ticks(world: &World, id: EntityId) -> Option<Tick> {
    let (s, p, i) = shaped(world, id)?;
    let b = block_at(s, p, i).filter(|&b| b > 0)?;
    Some(Tick::from(b))
}

/// `exec::actions::duration` of a `Break`: to `break_min` after the break
/// plan began.
pub fn break_ticks(world: &World, id: EntityId) -> Tick {
    let min = world.config.shift.as_ref().map_or(0, |c| Tick::from(c.break_min));
    match shaped(world, id) {
        Some((s, _, _)) => (s.plan_tick + min).saturating_sub(world.tick).max(1),
        None => min.max(1),
    }
}

/// `exec::actions::on_complete` of a work step that `started`: notes the
/// block (the device rows) and is `true` for a block before the shift's
/// last (done, the shift not closed: no wage, no output yet).
pub fn block_done(world: &mut World, id: EntityId, started: Tick) -> bool {
    let now = world.tick;
    let Some((s, p, i)) = shaped(world, id) else { return false };
    let block = block_at(s, p, i);
    let spot = i
        .checked_sub(1)
        .and_then(|j| p.steps.get(j))
        .filter(|st| st.action == ActionKind::GoTo(LocationKey::Station))
        .and_then(|st| st.tile)
        .and_then(|t| s.spots.iter().position(|&x| x == t));
    let end = world.comp::<Job>(id).and_then(|j| j.shift_end(started));
    let mid_shift = block.is_some_and(|b| b > 0) && end.is_some_and(|e| now < e);
    let len = u16::try_from(now.saturating_sub(started)).unwrap_or(u16::MAX);
    if let Some(s) = world.comp_mut::<Brain>(id).and_then(|b| b.shift.as_mut()) {
        s.longest = s.longest.max(len);
        if let Some(k) = spot.filter(|&k| k < 8) {
            s.worked |= 1 << k;
        }
    }
    mid_shift
}

/// The tick a work step that `started` reads its shift from (the output
/// and a pro-rata cut): the shift's first block, for a stations shift.
pub fn span_start(world: &World, id: EntityId, started: Tick) -> Tick {
    let key = world.comp::<Job>(id).map(|j| j.shift_key_at(started));
    world
        .comp::<Brain>(id)
        .and_then(|b| b.shift.as_deref())
        .filter(|s| Some(s.key) == key)
        .map_or(started, |s| s.from.min(started))
}

/// The shift's close (the last block done, before `end_shift`): the
/// stations shift's device rows (`Jobs2Cols` `shift_*`).
pub fn shift_closed(world: &mut World, id: EntityId) {
    let Some((longest, tiles, meal)) =
        shaped(world, id).map(|(s, _, _)| (s.longest, s.worked.count_ones(), s.meal && s.stage == 2))
    else {
        return;
    };
    let shape = world.comp::<Job>(id).map_or(ShiftShape::Post, |j| shape_of(world, j.role));
    let row = crate::systems::ownership::jobs2_row(world);
    match shape {
        ShiftShape::Haul => row.haul_shifts += 1,
        ShiftShape::Counter => row.counter_shifts += 1,
        _ => row.shift_stations += 1,
    }
    row.shift_tiles += tiles;
    row.shift_break_meals += u32::from(meal);
    row.shift_block_max = row.shift_block_max.max(u32::from(longest));
    match longest {
        0..=60 => row.shift_long_60 += 1,
        61..=90 => row.shift_long_90 += 1,
        91..=180 => row.shift_long_180 += 1,
        _ => row.shift_long_over += 1,
    }
}

/// `fixes::committed`: `id` is between the shape plans of a stations shift
/// still being worked (the Work goal holds, as the one block did).
pub fn holds(world: &World, id: EntityId) -> bool {
    let Some(b) = world.comp::<Brain>(id) else { return false };
    let Some(s) = b.shift.as_deref() else { return false };
    // A shape plan that failed (a closed workplace) lets go, as the planner's
    // unplannable Work did: no hold on a cooled or just-failed Work.
    let now = world.tick;
    if b.last_plan_failure.is_some_and(|(g, _)| g == GoalKind::Work)
        || b.cooldowns.get(&GoalKind::Work).is_some_and(|&t| t > now)
    {
        return false;
    }
    world.comp::<Job>(id).is_some_and(|j| {
        let key = j.next_shift_key(world.tick);
        s.key == key && j.on_shift(world.tick_of_day()) && j.last_shift_day != Some(key)
    })
}

/// A walk to the workplace that re-enters a shift already begun (the return
/// from the mid-shift break or from a haul trip) is no commute: the shape
/// plan that sent `id` out was planned after the shift's first. The ride
/// share counts a workday's one set-out, as the Statistical tier's one
/// workday roll does (parity, the pre-view pass gate).
pub fn reentering_shift(world: &World, id: EntityId) -> bool {
    let Some(s) = world.comp::<Brain>(id).and_then(|b| b.shift.as_deref()) else { return false };
    world.comp::<Job>(id).is_some_and(|j| s.key == j.next_shift_key(world.tick) && s.plan_tick > s.from)
}

/// `exec::actions::can_start` of a `Break`: off the premises.
pub fn can_break(world: &World, id: EntityId) -> bool {
    world.comp::<Position>(id).is_some_and(|p| p.building.is_none())
}
