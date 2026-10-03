//! Level of detail. Every 60 ticks the highest-priority agents become Full
//! and the rest Coarse. M7 adds the Statistical tier, hysteresis and the
//! calibrated hourly tick; the ranking and transitions here are final.

use crate::components::{Brain, Lod, Position, Sentence, TilePos};
use crate::entity::EntityId;
use crate::exec::{self, ExecState};
use crate::goap::ActionKind;
use crate::map::{MAP_H, MAP_W};
use crate::time::TICKS_PER_HOUR;
use crate::world::World;

/// Tiles beyond the view rect that still count as on screen.
const VIEW_MARGIN: i32 = 8;

/// Plan steps that keep an agent interesting while off screen.
pub fn story_relevant(kind: ActionKind) -> bool {
    matches!(
        kind,
        ActionKind::StealFood(_)
            | ActionKind::Arrest
            | ActionKind::Attack
            | ActionKind::Propose
            | ActionKind::JoinGang
            | ActionKind::Extort
            | ActionKind::BuryCorpse
    )
}

pub fn run(world: &mut World) {
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if let Some(forced) = world.config.lod.force {
        for id in world.citizens() {
            set_lod(world, id, forced);
        }
        return;
    }

    let view = world.view_rect;
    let centre = match view {
        Some(r) => TilePos { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        None => TilePos { x: (MAP_W / 2) as u8, y: (MAP_H / 2) as u8 },
    };
    let on_screen = |t: TilePos| -> bool {
        let Some(r) = view else { return false };
        let (x, y) = (i32::from(t.x), i32::from(t.y));
        x >= i32::from(r.x) - VIEW_MARGIN
            && x < i32::from(r.x) + i32::from(r.w) + VIEW_MARGIN
            && y >= i32::from(r.y) - VIEW_MARGIN
            && y < i32::from(r.y) + i32::from(r.h) + VIEW_MARGIN
    };

    let mut ranked: Vec<(i32, u32, u32, EntityId)> = Vec::new();
    for id in world.citizens() {
        if world.has::<Sentence>(id) {
            set_lod(world, id, Lod::Coarse);
            continue;
        }
        let (Some(pos), Some(brain)) = (world.comp::<Position>(id), world.comp::<Brain>(id)) else { continue };
        let story = brain.current_step().is_some_and(|s| story_relevant(s.action));
        let priority = i32::from(on_screen(pos.tile)) * 3 + i32::from(brain.pinned) * 2 + i32::from(story);
        ranked.push((-priority, pos.tile.manhattan(centre), id.index, id));
    }
    ranked.sort_unstable();

    let max_full = world.config.lod.max_full;
    for (rank, &(_, _, _, id)) in ranked.iter().enumerate() {
        set_lod(world, id, if rank < max_full { Lod::Full } else { Lod::Coarse });
    }
}

/// Change an agent's LOD, converting its execution state per the transition
/// table: a walk becomes a timed arrival and back; Use and Wait keep their
/// remaining duration.
pub fn set_lod(world: &mut World, id: EntityId, lod: Lod) {
    let Some(brain) = world.comp::<Brain>(id) else { return };
    if brain.lod == lod {
        return;
    }
    let exec = brain.exec.clone();
    let new_exec = match (exec, lod) {
        (ExecState::Goto { target, .. }, Lod::Coarse | Lod::Statistical) => exec::timed_goto(world, id, target),
        (ExecState::GotoTimed { target, .. }, Lod::Full) => match exec::walking_goto(world, id, target) {
            Some(state) => state,
            // Unreachable on foot: let the step fail and the agent replan.
            None => ExecState::Idle,
        },
        (other, _) => other,
    };
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.lod = lod;
        b.exec = new_exec;
    }
}
