//! World systems. Each is `pub fn run(world: &mut World)` and `World::tick`
//! calls them in the fixed order
//! `commands, time, lod, needs, memory, mood, think, plan, exec, ownership,
//! classes, districts, economy, bind, law, social, gang, corp_brain,
//! demography, stats`. `districts` also runs the street's nightly pass at
//! 03:00 (`street::nightly`, M12 D6); `law` deals the district beats and
//! scores the stances right after the captain's daily posture (D10, D12).
//!
//! Needs decay lives in `crate::needs`, execution in `crate::exec`.

#![deny(clippy::unwrap_used)]

pub mod bind;
pub mod classes;
pub mod corp_brain;
pub mod corps;
pub mod demography;
pub mod districts;
pub mod economy;
pub mod faction;
pub mod founding;
pub mod gang;
pub mod law;
pub mod law_brain;
pub mod litter;
pub mod lod;
pub mod memory;
pub mod ownership;
pub mod plan;
pub mod raid;
pub mod social;
pub mod stat_policy;
pub mod stats;
pub mod street;
pub mod think;
