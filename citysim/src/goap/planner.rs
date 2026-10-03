//! Forward A* over `WorldState`. The state is ~25 keys and `Copy`, forward
//! search evaluates real costs against the agent's current context, and it
//! avoids the unbound-variable problem of backward GOAP.

use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use crate::goap::actions::{ActionKind, PlanCtx, PLANNABLE};
use crate::goap::world_state::{GoalState, WorldState};

/// Heuristic weight per unsatisfied goal key. Slightly inadmissible (the
/// cheapest action costs 0.5); accepted for speed.
pub const H_PER_KEY: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// No sequence within the limits reaches the goal.
    Unreachable,
    /// The expansion cap was hit first.
    ExpansionCap,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_expansions: usize,
    pub max_len: usize,
}

struct Node {
    g: f32,
    state: WorldState,
    depth: u8,
    parent: Option<usize>,
    via: Option<ActionKind>,
}

/// A found plan: the action sequence, its total cost and the expansions used.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub steps: Vec<ActionKind>,
    pub cost: f32,
    pub expansions: usize,
}

/// Search from `start` for a state satisfying `goal`.
pub fn plan(ctx: &PlanCtx, start: WorldState, goal: &GoalState, limits: Limits) -> Result<Found, (PlanError, usize)> {
    let h = |s: &WorldState| s.unsatisfied(goal) as f32 * H_PER_KEY;
    let mut nodes: Vec<Node> = vec![Node { g: 0.0, state: start, depth: 0, parent: None, via: None }];
    // (f, action order of the step that produced the node, node index)
    let mut open: BinaryHeap<Reverse<(OrderedFloat<f32>, usize, usize)>> = BinaryHeap::new();
    open.push(Reverse((OrderedFloat(h(&start)), 0, 0)));
    let mut closed: BTreeSet<WorldState> = BTreeSet::new();
    let mut expansions = 0usize;

    while let Some(Reverse((_, _, idx))) = open.pop() {
        let state = nodes[idx].state;
        if state.satisfies(goal) {
            let mut steps = Vec::new();
            let mut cur = idx;
            while let Some(p) = nodes[cur].parent {
                if let Some(a) = nodes[cur].via {
                    steps.push(a);
                }
                cur = p;
            }
            steps.reverse();
            return Ok(Found { steps, cost: nodes[idx].g, expansions });
        }
        if !closed.insert(state) {
            continue;
        }
        if usize::from(nodes[idx].depth) >= limits.max_len {
            continue;
        }
        expansions += 1;
        if expansions > limits.max_expansions {
            return Err((PlanError::ExpansionCap, expansions));
        }
        let g = nodes[idx].g;
        let depth = nodes[idx].depth;
        for (order, &kind) in PLANNABLE.iter().enumerate() {
            if !kind.allowed(ctx) || !kind.feasible(ctx) || !kind.preconditions(&state, ctx) {
                continue;
            }
            let next = kind.apply(&state, ctx);
            if closed.contains(&next) {
                continue;
            }
            let ng = g + kind.cost(ctx);
            let f = ng + h(&next);
            nodes.push(Node { g: ng, state: next, depth: depth + 1, parent: Some(idx), via: Some(kind) });
            open.push(Reverse((OrderedFloat(f), order, nodes.len() - 1)));
        }
    }
    Err((PlanError::Unreachable, expansions))
}
