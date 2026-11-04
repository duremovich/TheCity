//! S2 (`OPEN-313`, the sim half): the bystanders of a live fight react. Each tick a live engagement runs (and at
//! the view's first stepped beat of a tick), the Full bodies within `[combat] stir_tiles` of its front, in the same
//! place (the street, or the same building), who are not in it and did not join it, react once in
//! `stir_cool_ticks`: a timid one (courage under `stir_flee_courage`) runs (a Flee plan to a street tile
//! `stir_flee_tiles` off, away from the fight, at the flight's run, `flee_run_mult`), else, on a keyed draw under
//! `stir_watch_p`, it stops where it stands and watches for `stir_watch_ticks` (its plan given up for a stand);
//! the rest walk on. The law on duty comes in through the joiners (`recruit`), not here. At most `stir_max` a fight
//! a tick, headless as with a view (S3: the shadows read the played city). Nobody the human drives, no one
//! cuffed, down, knocked out, sentenced or aboard.

use super::{hash, key, Engagement};
use crate::components::{ActionInstance, Brain, GoalKind, Lod, Personality, Position, TileKind, TilePos};
use crate::entity::EntityId;
use crate::exec::ExecState;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::time::Tick;
use crate::world::World;

const PURPOSE_STIR: u64 = 0x434F_0009;

/// The bystanders of `e` (engagement `eid`) react (see the module doc).
pub(super) fn bystanders(world: &mut World, e: &Engagement, eid: u32) {
    let c = world.config.combat.clone();
    if c.stir_tiles == 0 || c.stir_max == 0 {
        return;
    }
    let Some(front) = e.first[0].or(e.first[1]).or_else(|| e.members().next()) else { return };
    let Some(at) = world.comp::<Position>(front).cloned() else { return };
    let now = world.tick;
    world.combat.stirred.retain(|_, t| *t > now);
    let near = world.agents_near(at.tile, u32::from(c.stir_tiles), at.building);
    let mut n = 0;
    for id in near {
        if n >= c.stir_max {
            break;
        }
        if e.seen.contains(&id) || !stirrable(world, id, &at, c.stir_tiles) {
            continue;
        }
        let courage = world.comp::<Personality>(id).map_or(0.5, |p| p.courage);
        let reacted = if courage < c.stir_flee_courage {
            run_off(world, id, at.tile, c.stir_flee_tiles)
        } else if hash(world, PURPOSE_STIR, key(id), u64::from(eid)) < c.stir_watch_p {
            watch(world, id, c.stir_watch_ticks)
        } else {
            false
        };
        // one look a cool-down either way: a body that walked on is not asked again each tick
        world.combat.stirred.insert(id, now + Tick::from(c.stir_cool_ticks.max(1)));
        if reacted {
            n += 1;
        }
    }
}

/// A Full body in the fight's place and sight, free to react, and not already reacting.
fn stirrable(world: &World, id: EntityId, at: &Position, tiles: u16) -> bool {
    let Some(b) = world.comp::<Brain>(id) else { return false };
    let Some(p) = world.comp::<Position>(id) else { return false };
    b.lod == Lod::Full
        && p.building == at.building
        && p.tile.manhattan(at.tile) <= u32::from(tiles)
        && !world.combat.stirred.contains_key(&id)
        && !world.combat.engaged.contains_key(&id)
        && !crate::systems::player::drives(world, id)
        && crate::systems::think::held(world, id).is_none()
        && !world.aboard.contains_key(&id)
        && !b.current_goal.is_some_and(crate::systems::fixes::danger)
}

/// Runs off: a Flee plan to a street tile about `tiles` off, on the far side from the fight.
fn run_off(world: &mut World, id: EntityId, fight: TilePos, tiles: u16) -> bool {
    let Some(here) = world.comp::<Position>(id).map(|p| p.tile) else { return false };
    let Some(to) = away_tile(world, here, fight, tiles) else { return false };
    world.abort_plan(id);
    let step = ActionInstance { action: ActionKind::GoTo(LocationKey::Picked), target: None, tile: Some(to) };
    let plan = Plan { goal: GoalKind::Flee, target: None, steps: vec![step], started_tick: world.tick };
    crate::systems::plan::install(world, id, plan);
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Flee);
        b.goal_since = now;
    }
    crate::systems::ownership::jobs2_row(world).stir_fled += 1;
    true
}

/// Stops and watches: the plan given up for a stand of `ticks` where it is (a `Rest` step held as a plain `Wait`,
/// S3: no doze's energy, which a Use of Rest gave on its end).
fn watch(world: &mut World, id: EntityId, ticks: u16) -> bool {
    let now = world.tick;
    world.abort_plan(id);
    let step = ActionInstance { action: ActionKind::Rest, target: None, tile: None };
    let plan = Plan { goal: GoalKind::Idle, target: None, steps: vec![step], started_tick: now };
    crate::systems::plan::install(world, id, plan);
    let until = now + Tick::from(ticks.max(1));
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Idle);
        b.goal_since = now;
        b.exec = ExecState::Wait { until };
        b.action_until = until;
    }
    crate::systems::ownership::jobs2_row(world).stir_watched += 1;
    true
}

/// A walkable street tile (ground or road) about `tiles` from `here`, along the line from the fight through `here`
/// (the nearest such tile to that point within 4 of it).
fn away_tile(world: &World, here: TilePos, fight: TilePos, tiles: u16) -> Option<TilePos> {
    let (dx, dy) = (i32::from(here.x) - i32::from(fight.x), i32::from(here.y) - i32::from(fight.y));
    let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
    let (ux, uy) = if dx == 0 && dy == 0 { (1.0, 0.0) } else { (dx as f32 / len, dy as f32 / len) };
    let (w, h) = (world.map.w() as i32, world.map.h() as i32);
    let goal = (
        (f32::from(here.x) + ux * f32::from(tiles)).round() as i32,
        (f32::from(here.y) + uy * f32::from(tiles)).round() as i32,
    );
    let mut best: Option<(i32, TilePos)> = None;
    for oy in -4..=4 {
        for ox in -4..=4 {
            let (x, y) = (goal.0 + ox, goal.1 + oy);
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            let t = TilePos { x: x as u16, y: y as u16 };
            if !matches!(world.map.tile_at(t), TileKind::Ground | TileKind::Road) {
                continue;
            }
            let d = ox.abs() + oy.abs();
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, t));
            }
        }
    }
    best.map(|b| b.1)
}
