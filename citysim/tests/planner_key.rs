//! M15 phase 3: the planner's key word was widened (`WorldState::pack`'s
//! two counters packed into two bits each, freeing twelve flag bits). The
//! closed set only tests membership, so any injective packing must find the
//! same plans: this checks the packing is injective over every field, and
//! that the planner finds exactly the plans, costs and expansion counts of a
//! reference search keyed on the whole `WorldState`, for every body and
//! every planned goal of a seeded city.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use citysim::goap::actions::PLANNABLE;
use citysim::goap::planner::{self, Found, Limits, PlanError, H_PER_KEY};
use citysim::goap::world_state::{FLAG_BASE, MAX_FLAGS};
use citysim::systems::plan;
use citysim::utility::goals::GOAL_ORDER;
use citysim::{ActionKind, Config, GoalState, Key, LocationKey, PlanCtx, World, WorldState, TICKS_PER_DAY};
use ordered_float::OrderedFloat;

const KEYS: [Key; 39] = [
    Key::HungerSatisfied,
    Key::EnergySatisfied,
    Key::BelongingSatisfied,
    Key::HasFood,
    Key::HasCoins,
    Key::HasSavings,
    Key::HasWageDue,
    Key::ShiftDone,
    Key::HasSpouse,
    Key::HasPartnerCandidate,
    Key::IsSafe,
    Key::ThreatRemoved,
    Key::CrimeReported,
    Key::SuspectJailed,
    Key::SuspectCuffed,
    Key::InGang,
    Key::GangTaskDone,
    Key::RaidDone,
    Key::Mustered,
    Key::CorpseBuried,
    Key::CarryingCorpse,
    Key::CarryingStolen,
    Key::IsDark,
    Key::KnownCorpse,
    Key::KnownSuspectLocation,
    Key::PatrolLegDone,
    Key::FoodSourceAvailable,
    Key::ForageAvailable,
    Key::Founded,
    Key::CheckedIn,
    Key::Squatting,
    Key::Bought,
    Key::CarryingVehicle,
    Key::Treated,
    Key::Stripped,
    Key::HasStims,
    Key::High,
    Key::RunDone,
    Key::HasData,
];

#[test]
fn test_pack_is_injective() {
    assert!(KEYS.len() as u32 <= MAX_FLAGS, "the flags fit the key");
    const { assert!(FLAG_BASE >= 12) };
    let mut seen = BTreeSet::new();
    let mut ats: Vec<LocationKey> = LocationKey::GOTO.to_vec();
    ats.push(LocationKey::Anywhere);
    for &at in &ats {
        for bucket in 0..=2u8 {
            for food in 0..=3u8 {
                let ws = WorldState { at, coin_bucket: bucket, food_count: food, ..WorldState::default() };
                assert!(seen.insert(ws.pack()), "{at:?} {bucket} {food}");
                assert_eq!(
                    WorldState::repack_at(ws.pack(), LocationKey::Home),
                    WorldState { at: LocationKey::Home, ..ws }.pack()
                );
            }
        }
    }
    // Each flag sets its own bit, above the counters.
    let mut bits = 0u64;
    for k in KEYS {
        let mut ws = WorldState::default();
        citysim::goap::actions::set_key(&mut ws, k, true);
        let d = ws.pack() ^ WorldState::default().pack();
        assert_eq!(d.count_ones(), 1, "{k:?}");
        assert!(d >= 1 << FLAG_BASE, "{k:?} above the counters");
        assert_eq!(bits & d, 0, "{k:?} shares a bit");
        bits |= d;
    }
}

/// `planner::plan` with the closed set keyed on the whole state.
fn reference(ctx: &PlanCtx, start: WorldState, goal: &GoalState, limits: Limits) -> Result<Found, (PlanError, usize)> {
    struct Node {
        g: f32,
        state: WorldState,
        depth: u8,
        parent: Option<usize>,
        via: Option<ActionKind>,
    }
    let h = |s: &WorldState| s.unsatisfied(goal) as f32 * H_PER_KEY;
    let mut nodes = vec![Node { g: 0.0, state: start, depth: 0, parent: None, via: None }];
    let mut open: BinaryHeap<Reverse<(OrderedFloat<f32>, usize, usize)>> = BinaryHeap::new();
    open.push(Reverse((OrderedFloat(h(&start)), 0, 0)));
    let mut closed: BTreeSet<WorldState> = BTreeSet::new();
    let mut expansions = 0usize;
    let usable: Vec<(usize, ActionKind, f32)> = PLANNABLE
        .iter()
        .enumerate()
        .filter(|(_, k)| k.allowed(ctx) && k.feasible(ctx))
        .map(|(o, &k)| (o, k, k.cost(ctx)))
        .collect();
    while let Some(Reverse((_, _, idx))) = open.pop() {
        let state = nodes[idx].state;
        if state.satisfies(goal) {
            let mut steps = Vec::new();
            let mut cur = idx;
            while let Some(p) = nodes[cur].parent {
                steps.extend(nodes[cur].via);
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
        let (g, depth) = (nodes[idx].g, nodes[idx].depth);
        for &(order, kind, cost) in &usable {
            let next = match kind {
                ActionKind::GoTo(k) => {
                    if state.at == k {
                        continue;
                    }
                    WorldState { at: k, ..state }
                }
                _ => {
                    if !kind.preconditions(&state, ctx) {
                        continue;
                    }
                    kind.apply(&state, ctx)
                }
            };
            if closed.contains(&next) {
                continue;
            }
            nodes.push(Node { g: g + cost, state: next, depth: depth + 1, parent: Some(idx), via: Some(kind) });
            open.push(Reverse((OrderedFloat(g + cost + h(&next)), order, nodes.len() - 1)));
        }
    }
    Err((PlanError::Unreachable, expansions))
}

#[test]
fn test_planner_parity_with_whole_state_keys() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(3 * TICKS_PER_DAY + 13 * 60);
    let limits = Limits { max_expansions: w.config.brain.plan_max_expansions, max_len: w.config.brain.plan_max_len };
    let mut compared = 0;
    let mut found = 0;
    for id in w.bodies() {
        for goal in GOAL_ORDER {
            let Some(gs) = citysim::goap::goal_state(goal) else { continue };
            let target = plan::bind_target(&w, id, goal);
            let ctx = PlanCtx::build_for(&w, id, target, goal);
            let start = WorldState::observe(&w, id, target);
            let a = planner::plan(&ctx, start, &gs, limits);
            let b = reference(&ctx, start, &gs, limits);
            assert_eq!(a, b, "{id:?} {goal:?}");
            compared += 1;
            found += usize::from(a.is_ok());
        }
    }
    assert!(compared > 1000 && found > 100, "{compared} searches, {found} plans");
}
