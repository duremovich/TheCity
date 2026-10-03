//! World systems. Each is `pub fn run(world: &mut World)` and `World::tick`
//! calls them in the fixed order
//! `commands, time, lod, needs, memory, think, plan, exec, economy, law,
//! social, gang, demography, stats`.
//!
//! Needs decay lives in `crate::needs`, execution in `crate::exec`.

#![deny(clippy::unwrap_used)]

pub mod economy;
pub mod lod;
pub mod plan;
pub mod stats;
pub mod think;
