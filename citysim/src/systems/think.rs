//! Think scheduling: utility scoring every 30 ticks, staggered by
//! `tick % 30 == id.index % 30`, plus one urgent think per 30 ticks when a
//! plan ends or fails. On a goal change the current plan is aborted (unless
//! the step is uninterruptible) and the agent is queued for planning.

use crate::components::{Brain, GoalKind, Lod, Sentence};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::ActionKind;
use crate::systems::plan;
use crate::utility;
use crate::world::World;

/// Steps that a goal change must not abort.
fn uninterruptible(brain: &Brain, now: crate::time::Tick) -> bool {
    // M12 phase 4: a march that has left the muster is committed: the
    // marchers reach the door together instead of peeling off to bed.
    if brain.current_step().is_some_and(|s| {
        matches!(
            s.action,
            ActionKind::Arrest
                | ActionKind::Escort
                | ActionKind::Brawl
                | ActionKind::GoTo(crate::goap::LocationKey::RaidTarget)
        )
    }) {
        return true;
    }
    // M14 V12: a seated runner stays in the chair until its run ends, and a
    // runner dazed by a lost contest until the daze passes.
    if matches!(brain.exec, ExecState::JackedIn { .. }) || brain.dazed_until.is_some_and(|t| t > now) {
        return true;
    }
    match &brain.exec {
        // Think runs before exec in the tick: a step completing this very tick
        // keeps its completion effects (a shift's wage, a meal) instead of
        // being aborted one tick short by a goal that only wins because it ended.
        ExecState::Use { until, .. } if *until <= now => true,
        ExecState::Use { kind: ActionKind::Sleep, started, .. } => now.saturating_sub(*started) >= 60,
        // M12 phase 4: a muster in its last two hours holds its crew.
        ExecState::Use { kind: ActionKind::Muster, until, .. } => {
            until.saturating_sub(now) <= 2 * crate::time::TICKS_PER_HOUR
        }
        // M13 phase 3: an install on the table runs to the end.
        ExecState::Use { kind: ActionKind::BuyAsset, .. } => {
            brain.shop_pick.as_ref().is_some_and(|p| p.kind.is_implant() || p.used.is_some())
        }
        ExecState::Use { kind, .. } => matches!(
            kind,
            ActionKind::Arrest
                | ActionKind::Escort
                | ActionKind::ServeTime
                | ActionKind::BuryCorpse
                | ActionKind::HaulToMarket
                | ActionKind::Install
                | ActionKind::Therapy
                | ActionKind::Uninstall
                | ActionKind::Rip
                | ActionKind::Detox
        ),
        _ => false,
    }
}

pub fn run(world: &mut World) {
    let interval = world.config.brain.think_interval_ticks.max(1);
    let tick = world.tick;
    world.shop_offers.clear();
    for id in world.bodies() {
        let Some(brain) = world.comp::<Brain>(id) else { continue };
        if brain.lod == Lod::Statistical || world.has::<Sentence>(id) || brain.cuffed_by.is_some() || brain.emigrating {
            continue;
        }
        let scheduled = tick % interval == u64::from(id.index) % interval;
        let urgent = brain.plan.is_none()
            && !brain.plan_queued
            && (brain.last_think.is_none() || tick.saturating_sub(brain.last_urgent_think_tick) >= interval);
        if scheduled || urgent {
            think_once(world, id, scheduled);
        }
    }
}

/// Score goals, switch goal if a different one wins, and queue for planning
/// when idle.
pub fn think_once(world: &mut World, id: EntityId, scheduled: bool) {
    let tick = world.tick;
    // The Court gate's neighbour scan runs once a day, not every think.
    let today = world.day();
    let stale = world.comp::<Brain>(id).is_some_and(|b| b.court_candidate.is_none_or(|(d, _)| d != today));
    if stale {
        let candidate = crate::systems::social::known_candidate(world, id, 0.3).is_some();
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.court_candidate = Some((today, candidate));
        }
    }
    let Some((winner, trace, offer)) = utility::think_with_offer(world, id) else { return };
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let score = trace.goals.first().map_or(0.0, |g| g.score);
    // A different winner while an uninterruptible step runs is deferred: the
    // goal and its plan stay together until the step completes.
    // M12 phase 4: a committed march is released once its expedition is over.
    let march_over =
        brain.plan.as_ref().is_some_and(|p| p.goal == GoalKind::Raid) && crate::systems::raid::raid_done(world, id);
    let held = brain.plan.is_some() && uninterruptible(brain, tick) && !march_over;
    let changed = brain.current_goal != Some(winner) && !held;
    let abort = changed && brain.plan.is_some();
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
    // M13 review: the Shop offer just scored serves this tick's plan,
    // unless an abort (a trip ended, a vehicle parked) may have changed it.
    if winner == GoalKind::Shop && !abort {
        world.shop_offers.insert(id, (tick, offer));
    }
    if abort {
        world.abort_plan(id);
        if let Some(old) = old {
            world.push_event(EventKind::PlanAborted, &[id], format!("{old:?} -> {winner:?}"));
        }
    }
    // A goal change is a pursued goal being taken up: from Idle, after a plan
    // completed or failed, or displacing another goal. Dropping to Idle is the
    // absence of a goal, not a change.
    if changed && winner != GoalKind::Idle {
        world.stats.current.goal_changes += 1;
    }

    // Queue the current goal for planning if nothing is running. Idle plans
    // immediately (it bypasses the planner and costs nothing).
    let current =
        world.comp::<Brain>(id).and_then(|b| if b.plan.is_none() && !b.plan_queued { b.current_goal } else { None });
    match current {
        Some(GoalKind::Idle) => {
            plan::plan_for(world, id, GoalKind::Idle);
        }
        Some(_) => plan::enqueue(world, id, score),
        None => {}
    }
}
