//! GOAP: symbolic world state, actions and the forward A* planner.
//!
//! M0 ships the data types only; the planner itself arrives in M3.

#![deny(clippy::unwrap_used)]

pub mod actions;
pub mod world_state;

pub use actions::{ActionKind, Plan, StealSource};
pub use world_state::{LocationKey, WorldState};

// Planner limits (max expansions, plan length, per-tick budgets, timeout,
// cooldown) live in `config.brain`, not here: one source of truth.
