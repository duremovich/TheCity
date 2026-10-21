//! The pre-view pass phase 5 (docs/PREVIEW_PASS.md § 0 P10, § 2 "20k
//! legs"; OPEN-139): a long walking `GoTo` walks in segments.
//!
//! A Full or Coarse `GoTo` that walks (no vehicle, no cab, no El) farther
//! than `[legs] leg_replan_tiles` walks to a segment end first: the Road
//! tile nearest the point that far along the straight line to the target.
//! There the step re-starts from where the body stands (`exec::start_step`
//! again: the cab, the El and the next segment re-checked), so a Coarse
//! body is seen en route a segment at a time instead of frozen at its door
//! for the whole leg, and a Full one re-reads its ride at each end.
//! `World::segments` names the plan step a segment belongs to (saved: a
//! save mid-segment resumes it). Statistical mirror: none (off camera, no
//! movement; `OPEN-149`).

use crate::components::{Brain, TileKind, TilePos};
use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;

/// `[legs]` on.
pub fn on(world: &World) -> bool {
    world.config.legs.is_some()
}

/// The plan step `id` runs: its plan's start tick and the step index.
pub fn plan_key(world: &World, id: EntityId) -> Option<(Tick, u8)> {
    let b = world.comp::<Brain>(id)?;
    b.plan.as_ref().map(|p| (p.started_tick, b.plan_step))
}

/// The end of the next segment from `from` toward `to`: the walkable Road
/// tile within `segment_snap` of the point `leg_replan_tiles` along the
/// straight line (nearest, ties the lower `(y, x)`), when the leg is longer
/// than that and the end is nearer the target by at least half a segment.
pub fn segment_end(world: &World, from: TilePos, to: TilePos) -> Option<TilePos> {
    let cfg = world.config.legs.as_ref()?;
    let len = cfg.leg_replan_tiles.max(1);
    let d = from.manhattan(to);
    if d <= len {
        return None;
    }
    let f = len as f32 / d as f32;
    let lerp = |a: u16, b: u16| (f32::from(a) + (f32::from(b) - f32::from(a)) * f).round() as i32;
    let (px, py) = (lerp(from.x, to.x), lerp(from.y, to.y));
    let r = cfg.segment_snap as i32;
    // A Road tile first; else open walkable land (an import's hills, the
    // fields): never a building's floor.
    let mut best: Option<(u8, u32, (u16, u16), TilePos)> = None;
    for dy in -r..=r {
        for dx in -r..=r {
            let (x, y) = (px + dx, py + dy);
            if !world.map.in_bounds(x, y) {
                continue;
            }
            let t = TilePos::new(x as u16, y as u16);
            let rank = match world.map.tile_at(t) {
                TileKind::Road => 0,
                TileKind::Mountain | TileKind::Farmland => 1,
                _ => continue,
            };
            if !world.map.walkable(t) {
                continue;
            }
            let k = (rank, dx.unsigned_abs() + dy.unsigned_abs(), (t.y, t.x), t);
            if best.is_none_or(|b| (k.0, k.1, k.2) < (b.0, b.1, b.2)) {
                best = Some(k);
            }
        }
    }
    let (_, _, _, t) = best?;
    (t != from && t.manhattan(to) + len / 2 <= d).then_some(t)
}

/// A segment's walk ended (`exec::step_agent`'s Done): the step re-starts
/// from here (the exec back to Idle, the step not advanced). `false` when
/// the arrival is no segment end of this step (a stale entry is dropped).
pub fn segment_done(world: &mut World, id: EntityId) -> bool {
    let Some(key) = world.segments.remove(&id) else { return false };
    if plan_key(world, id) != Some(key) {
        return false;
    }
    crate::systems::ownership::jobs2_row(world).leg_segments += 1;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.exec = crate::exec::ExecState::Idle;
    }
    true
}
