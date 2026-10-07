//! Utility-based goal selection: response curves, considerations, Dave
//! Mark's compensation factor, hysteresis and cooldowns.
//!
//! `think` scores every goal the agent is eligible for and returns the
//! winner plus a trace for the inspector. It never plans: the think system
//! hands the winner to the routine (M2) or the GOAP planner (M3).

#![deny(clippy::unwrap_used)]

pub mod curves;
pub mod goals;

use serde::{Deserialize, Serialize};

use crate::components::GoalKind;
use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;

pub use curves::Curve;

/// Trace entries kept per think.
pub const TRACE_TOP_N: usize = 5;

/// One consideration's input and curve output, for the inspector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Consideration {
    /// Always a literal: borrowed, so a think allocates no names (perf).
    pub name: std::borrow::Cow<'static, str>,
    pub input: f32,
    pub output: f32,
}

impl Consideration {
    pub fn new(name: &'static str, input: f32, curve: Curve) -> Self {
        Consideration { name: std::borrow::Cow::Borrowed(name), input, output: curve.eval(input) }
    }

    /// A pre-computed output (e.g. a mood multiplier already applied).
    pub fn raw(name: &'static str, input: f32, output: f32) -> Self {
        Consideration { name: std::borrow::Cow::Borrowed(name), input, output: output.clamp(0.0, 1.0) }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoalScore {
    pub goal: GoalKind,
    /// Product of consideration outputs.
    pub raw: f32,
    /// After compensation, hysteresis and flat bonus.
    pub score: f32,
    pub considerations: Vec<Consideration>,
}

/// The top goals from the last think, kept for Full and Coarse agents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThinkTrace {
    pub tick: Tick,
    pub goals: Vec<GoalScore>,
}

/// Dave Mark's compensation: `raw + (1 − raw) × (1 − 1/n) × raw`.
pub fn compensate(raw: f32, n: usize) -> f32 {
    if n == 0 {
        return raw;
    }
    let modif = (1.0 - raw) * (1.0 - 1.0 / n as f32);
    raw + modif * raw
}

/// Score one goal from its considerations. `None` if any consideration is
/// zero (the goal is unavailable).
pub fn score_goal(
    goal: GoalKind,
    considerations: Vec<Consideration>,
    current: Option<GoalKind>,
    hysteresis: f32,
    flat_bonus: f32,
) -> Option<GoalScore> {
    if considerations.iter().any(|c| c.output <= 0.0) {
        return None;
    }
    let raw: f32 = considerations.iter().map(|c| c.output).product();
    let mut score = compensate(raw, considerations.len());
    if current == Some(goal) {
        score += hysteresis;
    }
    score += flat_bonus;
    Some(GoalScore { goal, raw, score, considerations })
}

/// Evaluate every goal for an agent. Returns the winner and the trace
/// (top `TRACE_TOP_N` by score). `None` when the agent cannot think.
pub fn think(world: &World, id: EntityId) -> Option<(GoalKind, ThinkTrace)> {
    think_with_offer(world, id).map(|(goal, trace, _)| (goal, trace))
}

/// [`think`], also returning the Shop goal's offer when it was scored (M13
/// review: `shop_choice` runs once per think, for `already_satisfied` and
/// the considerations both, and the planner reuses it).
pub fn think_with_offer(
    world: &World,
    id: EntityId,
) -> Option<(GoalKind, ThinkTrace, Option<crate::systems::assets::ShopOffer>)> {
    let brain = world.comp::<crate::components::Brain>(id)?;
    let current = brain.current_goal;
    // Hysteresis protects a goal being pursued, not one already achieved:
    // otherwise Eat keeps winning at hunger 0.9 and every meal is wasted.
    let hysteresis = if brain.plan.is_some() { world.config.brain.goal_hysteresis } else { 0.0 };
    let tick = world.tick;

    let has_spouse = goals::has_spouse(world, id);
    let mut scored: Vec<GoalScore> = Vec::new();
    let mut shop = None;
    for goal in goals::GOAL_ORDER {
        // A cooled goal scores 0 (skipped). M14 V29: Hack's sale of a deck's
        // Data is not cooled by the run that took it.
        if brain.cooldowns.get(&goal).is_some_and(|&until| until > tick)
            && !(goal == GoalKind::Hack
                && (crate::systems::virt::deck_data(world, id) > 0
                    || crate::systems::virt::standing_order(world, id).is_some()))
        {
            continue;
        }
        // M13 D43: Shop's gate and considerations are its one offer.
        if goal == GoalKind::Shop {
            let Some(o) = crate::systems::assets::shop_choice(world, id, true) else { continue };
            let mut cs = o.considerations.clone();
            let mut flat = world.config.shop.shop_flat;
            // L1: the walk to the seller weighs on the purchase, flat and all.
            if world.config.life.enabled {
                let tiles = crate::systems::life::tiles_to(world, id, o.seller).unwrap_or(0);
                let t = crate::systems::life::travel(world, tiles);
                flat *= t.output;
                cs.push(t);
            }
            shop = Some(o);
            if let Some(s) = score_goal(goal, cs, current, hysteresis, flat) {
                scored.push(s);
            }
            continue;
        }
        // M15 W20: a hunter on a Hunt scores it at least `hold_score`, so
        // only an emergency interrupts it and the plan resumes.
        if goal == GoalKind::Hunt {
            let Some((cs, flat)) = crate::systems::hunt::considerations(world, id) else { continue };
            if let Some(mut s) = score_goal(goal, cs, current, hysteresis, flat) {
                if world.hunts.contains_key(&id) {
                    s.score = s.score.max(world.config.hunt.hold_score);
                }
                scored.push(s);
            }
            continue;
        }
        // M14 V29: Hack's gate and considerations are its one offer.
        if goal == GoalKind::Hack {
            let Some(o) = crate::systems::virt::hack_choice(world, id) else { continue };
            let mut cs = o.considerations;
            let mut flat = world.config.hack.hack_flat;
            // L1: a freelance run's walk to the chair (or the Data buyer)
            // weighs on it; a standing order is the order's.
            if world.config.life.enabled && crate::systems::virt::standing_order(world, id).is_none() {
                let at = if o.sell { o.lab } else { o.portal.map(|p| p.building) };
                let tiles = at.and_then(|b| crate::systems::life::tiles_to(world, id, b)).unwrap_or(0);
                let t = crate::systems::life::travel(world, tiles);
                flat *= t.output;
                cs.push(t);
            }
            if let Some(s) = score_goal(goal, cs, current, hysteresis, flat) {
                scored.push(s);
            }
            continue;
        }
        if goals::already_satisfied(world, id, goal, has_spouse) {
            continue;
        }
        let Some((considerations, flat)) = goals::considerations(world, id, goal, has_spouse) else { continue };
        if let Some(s) = score_goal(goal, considerations, current, hysteresis, flat) {
            scored.push(s);
        }
    }
    // Highest score wins; ties break by table order (stable sort).
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let winner = scored.first()?.goal;
    scored.truncate(TRACE_TOP_N);
    Some((winner, ThinkTrace { tick, goals: scored }, shop))
}
