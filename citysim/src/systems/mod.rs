//! World systems. Each is `pub fn run(world: &mut World)` and `World::tick`
//! calls them in the fixed order
//! `commands, time, lod, needs, memory, mood, think, plan, exec, virt,
//! ownership, assets, tech, classes, districts, economy, bind, word, law,
//! social, gang, corp_brain, living, contracts, demography, stats` (M16a
//! C36: `contracts` keeps the record board: the ledger's due ticks per
//! tick, the matching at `match_hour`, expiries and regulars at midnight;
//! L2 L36: `living`
//! holds every L2 daily and hourly pass; M15 W47: `word` after
//! `bind`, its midnight chain of pools, hearing, kin, reputation,
//! competence, grudges and vendettas, and the Hunt's daily pass; phase 3:
//! `hunt::tick` every tick over the Hunts). `districts` also runs the street's nightly pass at
//! 03:00 (`street::nightly`, M12 D6); `law` deals the district beats and
//! scores the stances right after the captain's daily posture (D10, D12).
//!
//! Needs decay lives in `crate::needs`, execution in `crate::exec`.

#![deny(clippy::unwrap_used)]

pub mod assets;
pub mod bind;
pub mod budget;
pub mod camp;
pub mod charity;
pub mod chrome;
pub mod classes;
pub mod competence;
pub mod contracts;
pub mod corp_brain;
pub mod corps;
pub mod creeds;
pub mod demography;
pub mod districts;
pub mod econ;
pub mod economy;
pub mod faction;
pub mod fixes;
pub mod founding;
pub mod fviolence;
pub mod gang;
pub mod gossip;
pub mod grudges;
pub mod hunt;
pub mod jobs;
pub mod law;
pub mod law_brain;
pub mod leisure;
pub mod life;
pub mod litter;
pub mod living;
pub mod lod;
pub mod memory;
pub mod missions;
pub mod moves;
pub mod news;
pub mod outside;
pub mod ownership;
pub mod plan;
pub mod raid;
pub mod reputation;
pub mod riot;
pub mod robots;
pub mod security;
pub mod social;
pub mod stat_policy;
pub mod stats;
pub mod stims;
pub mod street;
pub mod tech;
pub mod think;
pub mod vehicles;
pub mod virt;
pub mod word;
pub mod world_market;
