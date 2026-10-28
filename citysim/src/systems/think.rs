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

/// The body's turn in this tick's think, if it has one: `Some(scheduled)`
/// (its staggered slot, or an urgent think when its plan ended).
fn turn(world: &World, id: EntityId, tick: crate::time::Tick, interval: u64) -> Option<bool> {
    let brain = world.comp::<Brain>(id)?;
    if brain.lod == Lod::Statistical || world.has::<Sentence>(id) || brain.cuffed_by.is_some() || brain.emigrating {
        return None;
    }
    let scheduled = tick % interval == u64::from(id.index) % interval;
    let urgent = brain.plan.is_none()
        && !brain.plan_queued
        && (brain.last_think.is_none() || tick.saturating_sub(brain.last_urgent_think_tick) >= interval);
    (scheduled || urgent).then_some(scheduled)
}

/// The think slice: each body whose turn it is thinks, in id order.
///
/// The perf pass phase 8 (OPEN-184): with `[perf] threads` above 1 the
/// slice is scored ahead on that many threads ([`score`] reads only) and
/// each body applies its score in its turn ([`apply`], on this thread, every
/// write: the brain, the plan queue, aborts, events, counters), so the city
/// is the serial loop's at every thread count. A score is used only while
/// it is fresh: computed against the world as the serial loop would have
/// left it at that body's turn. Most applies write only the body's own
/// think bookkeeping, which no other body's score reads (its brain's
/// `current_goal`, `goal_since`, `last_think*`, `court_candidate`,
/// `plan_queued`; the plan queue, `shop_offers`, the day's counters), and
/// the Idle plan installed on a body standing idle writes only its plan,
/// which other scores read by goal and step alone, never Idle's. An apply
/// that aborts a plan, or installs the Idle plan on a body mid-walk or
/// aboard, writes more (the body's plan and step, its reservations, its tile
/// and building, an escort let go, a vehicle parked: reads of other
/// scores), so every score after it is stale: the bodies from the next one
/// on are scored again before they are used. A body's turn is re-read at its place (an earlier apply
/// may have freed it or given it a plan); one given a turn by an earlier
/// apply is scored there, on this thread. With 1 thread it is the serial
/// loop.
pub fn run(world: &mut World) {
    let interval = world.config.brain.think_interval_ticks.max(1);
    let tick = world.tick;
    world.shop_offers.clear();
    let bodies = world.bodies();
    let threads = world.config.perf.threads.max(1);
    if threads == 1 {
        for id in bodies {
            if let Some(scheduled) = turn(world, id, tick, interval) {
                think_once(world, id, scheduled);
            }
        }
        return;
    }
    // The whole slice runs on the pool, this thread waiting: the turns on
    // one of its threads, which takes its share of each batch while the
    // others are still awake from the last.
    #[cfg(test)]
    let scans = crate::spatial::scans();
    pool(threads).install(move || {
        #[cfg(test)]
        crate::spatial::SCANS.with(|s| s.set(scans));
        run_pooled(world, bodies, threads);
    });
}

/// [`run`] above 1 thread, on a pool thread.
fn run_pooled(world: &mut World, bodies: Vec<EntityId>, threads: usize) {
    let interval = world.config.brain.think_interval_ticks.max(1);
    let tick = world.tick;
    let slice: Vec<EntityId> = bodies.iter().copied().filter(|&id| turn(world, id, tick, interval).is_some()).collect();
    let mut scores: Vec<Option<Scored>> = (0..slice.len()).map(|_| None).collect();
    let mut wide: Vec<bool> = vec![false; slice.len()];
    crate::prof::timed!("think::ahead", score_ahead(world, &slice, 0, slice.len(), threads, &mut scores, &mut wide));
    // The scores before `stale_from` (those not yet taken) were computed
    // after the last wide apply.
    let mut stale_from = slice.len();
    let mut next = 0;
    for id in bodies {
        let at = (slice.get(next) == Some(&id)).then(|| {
            next += 1;
            next - 1
        });
        let Some(scheduled) = turn(world, id, tick, interval) else {
            if at.is_some() {
                // A score taken ahead for a body an earlier apply took the
                // turn from (counted under `prof`).
                let _counted = crate::prof::scope!("think::dropped");
            }
            continue;
        };
        let s = match at {
            Some(i) => {
                if i >= stale_from {
                    // Scored again from here to the next apply its stale
                    // score says will be wide (at least a thread's worth).
                    let mut end = (i..slice.len()).find(|&j| wide[j]).map_or(slice.len(), |j| j + 1);
                    end = end.max(i + threads).min(slice.len());
                    crate::prof::timed!(
                        "think::again",
                        score_ahead(world, &slice, i, end, threads, &mut scores, &mut wide)
                    );
                    stale_from = end;
                }
                scores[i].take().unwrap_or_else(|| score(world, id))
            }
            None => crate::prof::timed!("think::late", score(world, id)),
        };
        if apply(world, id, scheduled, s) {
            stale_from = stale_from.min(next);
        }
    }
}

/// Score `slice[from..to]` against `world` on `threads` threads into
/// `scores`, and mark in `wide` the bodies whose apply is likely to be wide
/// (a goal change over a plan: [`apply`]'s abort without its holds; a guess
/// only sizes the next batch). Each score reads the world
/// only (its memo caches are locked and pure: a function of the world,
/// which nothing writes while the batch is scored), so a score is the same
/// on any thread.
fn score_ahead(
    world: &World,
    slice: &[EntityId],
    from: usize,
    to: usize,
    threads: usize,
    scores: &mut [Option<Scored>],
    wide: &mut [bool],
) {
    use rayon::prelude::*;
    // A unit test's index-vs-scan mode is this thread's (`spatial::SCANS`):
    // the pool's threads take it for the batch.
    #[cfg(test)]
    let scans = crate::spatial::scans();
    let _ = threads;
    let batch: Vec<Scored> = slice[from..to]
        .par_iter()
        .map(|&id| {
            #[cfg(test)]
            crate::spatial::SCANS.with(|s| s.set(scans));
            score(world, id)
        })
        .collect();
    for (k, sc) in batch.into_iter().enumerate() {
        let i = from + k;
        wide[i] = likely_wide(world, slice[i], &sc);
        scores[i] = Some(sc);
    }
}

/// [`apply`]'s wide writes, guessed from a score: a different winner over a
/// running plan (an abort, unless a hold defers it).
fn likely_wide(world: &World, id: EntityId, s: &Scored) -> bool {
    let (Some(brain), Some((winner, ..))) = (world.comp::<Brain>(id), s.think.as_ref()) else { return false };
    brain.plan.is_some() && brain.current_goal != Some(*winner)
}

/// One pool per thread count, built on first use and kept for the process.
fn pool(threads: usize) -> std::sync::Arc<rayon::ThreadPool> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    static POOLS: Mutex<BTreeMap<usize, Arc<rayon::ThreadPool>>> = Mutex::new(BTreeMap::new());
    let mut pools = POOLS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    pools
        .entry(threads)
        .or_insert_with(|| {
            Arc::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap_or_else(|e| panic!("think pool of {threads} threads: {e}")),
            )
        })
        .clone()
}

/// One body's think, scored: what [`apply`] writes.
pub struct Scored {
    /// The Court gate's daily pre-filter, when the brain's was stale.
    court: Option<bool>,
    think: Option<(GoalKind, utility::ThinkTrace, Option<crate::systems::assets::ShopOffer>)>,
}

/// The reading half of a think: the Court gate's pre-filter once a day,
/// then every goal scored (`utility::think_scored`). Writes nothing.
pub fn score(world: &World, id: EntityId) -> Scored {
    // The Court gate's neighbour scan runs once a day, not every think.
    let today = world.day();
    let stale = world.comp::<Brain>(id).is_some_and(|b| b.court_candidate.is_none_or(|(d, _)| d != today));
    let court = stale.then(|| {
        crate::prof::timed!("think::known_candidate", crate::systems::social::known_candidate(world, id, 0.3).is_some())
    });
    let think = crate::prof::timed!("think::think_with_offer", utility::think_scored(world, id, court));
    Scored { court, think }
}

/// Score goals, switch goal if a different one wins, and queue for planning
/// when idle.
pub fn think_once(world: &mut World, id: EntityId, scheduled: bool) {
    let s = score(world, id);
    apply(world, id, scheduled, s);
}

/// The writing half of a think, on the world [`score`] read. True when it
/// was wide ([`run`]): it aborted a plan, or installed the Idle plan on a
/// body not standing idle.
fn apply(world: &mut World, id: EntityId, scheduled: bool, s: Scored) -> bool {
    let tick = world.tick;
    if let Some(candidate) = s.court {
        let today = world.day();
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.court_candidate = Some((today, candidate));
        }
    }
    let Some((winner, trace, offer)) = s.think else { return false };
    let Some(brain) = world.comp::<Brain>(id) else { return false };
    let score = trace.goals.first().map_or(0.0, |g| g.score);
    // A different winner while an uninterruptible step runs is deferred: the
    // goal and its plan stay together until the step completes.
    // M12 phase 4: a committed march is released once its expedition is over.
    let march_over =
        brain.plan.as_ref().is_some_and(|p| p.goal == GoalKind::Raid) && crate::systems::raid::raid_done(world, id);
    let held = brain.plan.is_some() && uninterruptible(brain, tick) && !march_over;
    // L2 shadow fixes items 1-4, 15: a latched commute, an in-shift work
    // step, a Sleep, a paid step and a started Lead give way only to
    // starvation or danger.
    let committed = !held && crate::systems::fixes::committed(world, id, brain, winner);
    let held = held || committed;
    let changed = brain.current_goal != Some(winner) && !held;
    let abort = changed && brain.plan.is_some();
    let old = brain.current_goal;

    if committed {
        world.stats.current.living.fix_held += 1;
    }
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
        // Round 4 T3 (OPEN-222): an arrest chase given up says so.
        if old == Some(GoalKind::Arrest) {
            crate::systems::law::chase_dropped(world, id, Some(winner));
        }
        crate::prof::timed!("think::abort", world.abort_plan(id));
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
            // The Idle plan on a body standing idle, off any vehicle, writes
            // only its own plan, which no other score reads but by goal (an
            // Arrest, a Lead, a Work, a Flee: never Idle) or by a step Idle
            // has none of; otherwise the install may set the body down off a
            // walk or a vehicle (wide).
            let quiet = world.comp::<Brain>(id).is_some_and(|b| matches!(b.exec, ExecState::Idle))
                && !world.aboard.contains_key(&id)
                && world.has::<crate::components::Position>(id);
            crate::prof::timed!("think::idle_plan", plan::plan_for(world, id, GoalKind::Idle));
            abort || !quiet
        }
        Some(_) => {
            plan::enqueue(world, id, score);
            abort
        }
        None => abort,
    }
}
