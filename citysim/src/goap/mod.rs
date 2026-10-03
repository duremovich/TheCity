//! GOAP: symbolic world state, actions and the forward A* planner.
//!
//! M0 ships the data types only; the planner itself arrives in M3.

#![deny(clippy::unwrap_used)]

pub mod actions;
pub mod world_state;

pub use actions::{ActionKind, Plan, StealSource};
pub use world_state::{LocationKey, WorldState};

pub const PLANNER_MAX_EXPANSIONS: usize = 200;
pub const PLAN_MAX_LEN: usize = 6;
pub const PLANNER_BUDGET_PER_TICK: usize = 12;
pub const PLAN_TIMEOUT_TICKS: crate::time::Tick = 900;
pub const PLANNER_EXPANSION_BUDGET_PER_TICK: usize = 600;
pub const GOAL_COOLDOWN_TICKS: crate::time::Tick = 120;
