//! World systems. Each is `pub fn run(world: &mut World)` and `World::tick`
//! calls them in the fixed order
//! `commands, time, lod, needs, memory, think, plan, exec, economy, law,
//! social, gang, demography, stats`.
//!
//! M0 ships `stats` only; the others arrive milestone by milestone.

#![deny(clippy::unwrap_used)]

pub mod stats;
