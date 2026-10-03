//! Think scheduling: utility scoring every 30 ticks, staggered by
//! `tick % 30 == id.index % 30`, plus one urgent think per 30 ticks when a
//! plan ends or fails. On a goal change the current plan is aborted (unless
//! the step is uninterruptible) and a new plan is built for the winner.

use crate::components::{Brain, GoalKind, Lod, Sentence};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{routine, ExecState};
use crate::goap::ActionKind;
use crate::utility;
use crate::world::World;

/// Steps that a goal change must not abort.
fn uninterruptible(brain: &Brain) -> bool {
    match &brain.exec {
        ExecState::Use { kind: ActionKind::Sleep, started, .. } => brain.last_think_tick.saturating_sub(*started) >= 60,
        ExecState::Use { kind, .. } => matches!(
            kind,
            ActionKind::Arrest
                | ActionKind::Escort
                | ActionKind::ServeTime
                | ActionKind::BuryCorpse
                | ActionKind::HaulToMarket
        ),
        _ => false,
    }
}

pub fn run(world: &mut World) {
    let interval = world.config.brain.think_interval_ticks.max(1);
    let tick = world.tick;
    for id in world.citizens() {
        let Some(brain) = world.comp::<Brain>(id) else { continue };
        if brain.lod == Lod::Statistical || world.has::<Sentence>(id) {
            continue;
        }
        let scheduled = tick % interval == u64::from(id.index) % interval;
        let urgent = brain.plan.is_none()
            && (brain.last_think.is_none() || tick.saturating_sub(brain.last_urgent_think_tick) >= interval);
        if scheduled || urgent {
            think_once(world, id, scheduled);
        }
    }
}

/// Score goals, switch goal if a different one wins, and plan when idle.
pub fn think_once(world: &mut World, id: EntityId, scheduled: bool) {
    let tick = world.tick;
    let Some((winner, trace)) = utility::think(world, id) else { return };
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let changed = brain.current_goal != Some(winner);
    let abort = changed && brain.plan.is_some() && !uninterruptible(brain);
    // The previous goal's plan just failed and a different goal now wins.
    let failed_over =
        changed && brain.plan.is_none() && brain.last_plan_failure.is_some_and(|(g, _)| Some(g) == brain.current_goal);
    let old = brain.current_goal;

    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.last_think = Some(trace);
        if scheduled {
            b.last_think_tick = tick;
        } else {
            b.last_urgent_think_tick = tick;
        }
        if changed {
            b.current_goal = Some(winner);
            b.goal_since = tick;
        }
    }
    if abort {
        world.abort_plan(id);
        if let Some(old) = old {
            world.push_event(EventKind::PlanAborted, &[id], format!("{old:?} -> {winner:?}"));
        }
    }
    // A goal change is a change of mind: a different goal displacing one still
    // being pursued, or taking over from one whose plan just failed. Picking
    // the next goal after a plan completes is not flapping and is not counted.
    if abort || failed_over {
        world.stats.current.goal_changes += 1;
    }

    // Plan for the goal if nothing is running.
    let idle = world.comp::<Brain>(id).is_some_and(|b| b.plan.is_none());
    if idle {
        plan_for_goal(world, id, winner);
    }
}

/// Build a plan for `goal` (M2: the routine; M3: GOAP). A goal that cannot be
/// planned right now is cooled so the next think picks something else.
pub fn plan_for_goal(world: &mut World, id: EntityId, goal: GoalKind) {
    let tick = world.tick;
    let cooldown = world.config.brain.goal_cooldown_ticks;
    match routine::plan_for_goal(world, id, goal) {
        Some(plan) => {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.plan = Some(plan);
                b.plan_step = 0;
                b.exec = ExecState::Idle;
            }
        }
        None => {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                if goal != GoalKind::Idle {
                    b.cooldowns.insert(goal, tick + cooldown);
                }
            }
        }
    }
}
