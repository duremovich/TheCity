//! Forward A* over `WorldState`. The state is ~25 keys and `Copy`, forward
//! search evaluates real costs against the agent's current context, and it
//! avoids the unbound-variable problem of backward GOAP.

use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::hash::{BuildHasherDefault, Hasher};

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
    /// `state.pack()`, computed once.
    key: u64,
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

/// A multiplicative hasher for the packed `u64` closed-set keys (the set is
/// only probed for membership, so its iteration order never matters).
#[derive(Default)]
struct PackHasher(u64);

impl Hasher for PackHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

/// The closed set, keyed on the packed state (`WorldState::pack` is
/// injective). Membership only, never iterated, so the hash order cannot
/// reach a plan: the `disallowed_types` rule guards iteration order, which
/// this set has none of (and its hasher is fixed, not seeded per process).
#[allow(clippy::disallowed_types)]
type ClosedSet = std::collections::HashSet<u64, BuildHasherDefault<PackHasher>>;

/// Search from `start` for a state satisfying `goal`.
pub fn plan(ctx: &PlanCtx, start: WorldState, goal: &GoalState, limits: Limits) -> Result<Found, (PlanError, usize)> {
    let h = |s: &WorldState| s.unsatisfied(goal) as f32 * H_PER_KEY;
    let mut nodes: Vec<Node> = Vec::with_capacity(64);
    nodes.push(Node { g: 0.0, state: start, key: start.pack(), depth: 0, parent: None, via: None });
    // (f, action order of the step that produced the node, node index)
    let mut open: BinaryHeap<Reverse<(OrderedFloat<f32>, usize, usize)>> = BinaryHeap::with_capacity(64);
    open.push(Reverse((OrderedFloat(h(&start)), 0, 0)));
    let mut closed = ClosedSet::default();
    let mut expansions = 0usize;
    // `allowed`, `feasible` and `cost` read only the context: decide them
    // once per search, not once per expansion (M11 phase 2: Arrest plans spent
    // ~60 us re-checking 54 actions at each of ~54 expansions). Same order,
    // same costs, so the same plans.
    let usable: Vec<(usize, ActionKind, f32)> = PLANNABLE
        .iter()
        .enumerate()
        .filter(|(_, k)| k.allowed(ctx) && k.feasible(ctx))
        .map(|(order, &k)| (order, k, k.cost(ctx)))
        .collect();

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
        if !closed.insert(nodes[idx].key) {
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
        let state_key = nodes[idx].key;
        for &(order, kind, cost) in &usable {
            let (next, key) = match kind {
                // `feasible` (in `usable`) already required the GoTo key's
                // distance entry, so only the location test of its
                // precondition remains; its effect is `at = k` alone.
                ActionKind::GoTo(k) => {
                    if state.at == k {
                        continue;
                    }
                    let mut next = state;
                    next.at = k;
                    (next, WorldState::repack_at(state_key, k))
                }
                _ => {
                    if !kind.preconditions(&state, ctx) {
                        continue;
                    }
                    let next = kind.apply(&state, ctx);
                    (next, next.pack())
                }
            };
            if closed.contains(&key) {
                continue;
            }
            let ng = g + cost;
            let f = ng + h(&next);
            nodes.push(Node { g: ng, state: next, key, depth: depth + 1, parent: Some(idx), via: Some(kind) });
            open.push(Reverse((OrderedFloat(f), order, nodes.len() - 1)));
        }
    }
    Err((PlanError::Unreachable, expansions))
}
