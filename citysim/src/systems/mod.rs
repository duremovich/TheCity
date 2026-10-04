//! World systems. Each is `pub fn run(world: &mut World)` and `World::tick`
//! calls them in the fixed order
//! `commands, time, lod, needs, memory, think, plan, exec, economy, law,
//! social, gang, demography, stats`.
//!
//! Needs decay lives in `crate::needs`, execution in `crate::exec`.

#![deny(clippy::unwrap_used)]

pub mod demography;
pub mod economy;
pub mod gang;
pub mod law;
pub mod lod;
pub mod memory;
pub mod plan;
pub mod social;
pub mod stats;
pub mod think;
