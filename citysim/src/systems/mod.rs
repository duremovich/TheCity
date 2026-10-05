//! World systems. Each is `pub fn run(world: &mut World)` and `World::tick`
//! calls them in the fixed order
//! `commands, time, lod, needs, memory, think, plan, exec, ownership, economy,
//! bind, law, social, gang, corp_brain, demography, stats`.
//!
//! Needs decay lives in `crate::needs`, execution in `crate::exec`.

#![deny(clippy::unwrap_used)]

pub mod bind;
pub mod corp_brain;
pub mod corps;
pub mod demography;
pub mod economy;
pub mod faction;
pub mod founding;
pub mod gang;
pub mod law;
pub mod law_brain;
pub mod lod;
pub mod memory;
pub mod ownership;
pub mod plan;
pub mod raid;
pub mod social;
pub mod stats;
pub mod think;
