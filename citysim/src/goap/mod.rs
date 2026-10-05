//! GOAP: symbolic world state, the action table and the forward A* planner.
//! The plan queue that feeds the planner lives in `systems::plan`.

#![deny(clippy::unwrap_used)]

pub mod actions;
pub mod planner;
pub mod world_state;

pub use actions::{ActionKind, Plan, PlanCtx, StealSource};
pub use planner::{Found, Limits, PlanError};
pub use world_state::{GoalState, Key, LocationKey, WorldState};

use crate::components::GoalKind;

/// The GOAP goal state for each goal (spec › Goal table, last column).
/// `None` for goals that bypass the planner (Idle).
pub fn goal_state(goal: GoalKind) -> Option<GoalState> {
    Some(match goal {
        GoalKind::Eat => vec![(Key::HungerSatisfied, true)],
        GoalKind::Sleep => vec![(Key::EnergySatisfied, true)],
        GoalKind::Work => vec![(Key::ShiftDone, true), (Key::HasWageDue, false)],
        GoalKind::Earn => vec![(Key::HasSavings, true)],
        GoalKind::Socialise => vec![(Key::BelongingSatisfied, true)],
        GoalKind::Court => vec![(Key::HasSpouse, true)],
        GoalKind::Flee => vec![(Key::IsSafe, true)],
        GoalKind::Fight => vec![(Key::ThreatRemoved, true)],
        GoalKind::ReportCrime => vec![(Key::CrimeReported, true)],
        GoalKind::Patrol => vec![(Key::PatrolLegDone, true)],
        GoalKind::Arrest => vec![(Key::SuspectJailed, true)],
        GoalKind::JoinGang => vec![(Key::InGang, true)],
        GoalKind::GangWork => vec![(Key::GangTaskDone, true)],
        GoalKind::Raid => vec![(Key::RaidDone, true)],
        GoalKind::Bury => vec![(Key::CorpseBuried, true)],
        GoalKind::Idle => return None,
    })
}
