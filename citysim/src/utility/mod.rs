//! Utility-based goal selection. M0 ships only the inspector trace type;
//! curves, considerations and scoring arrive in M2.

#![deny(clippy::unwrap_used)]

use serde::{Deserialize, Serialize};

use crate::components::GoalKind;
use crate::time::Tick;

/// One consideration's input and curve output, for the inspector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Consideration {
    pub name: String,
    pub input: f32,
    pub output: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoalScore {
    pub goal: GoalKind,
    pub score: f32,
    pub considerations: Vec<Consideration>,
}

/// The top goals from the last think, kept for Full and Coarse agents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThinkTrace {
    pub tick: Tick,
    pub goals: Vec<GoalScore>,
}
