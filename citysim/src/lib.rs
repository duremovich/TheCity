//! `citysim`: the headless Living City simulation.
//!
//! Everything deterministic lives here: the world, its components, the map,
//! time, the seeded RNG, events, stats, save/load and the world systems.
//! Rendering and I/O live in `citysim-app` and `citysim-cli`.
//!
//! Rules (see `docs/SPEC.md` › Coding rules): no `HashMap`/`HashSet`, no
//! `std::time`, every constant comes from `assets/config.toml`.

pub mod components;
pub mod config;
pub mod entity;
pub mod events;
pub mod exec;
pub mod goap;
pub mod levers;
pub mod map;
pub mod mood;
pub mod needs;
pub mod personality;
pub mod rng;
pub mod save;
pub mod stats;
pub mod story;
pub mod systems;
pub mod time;
pub mod utility;
pub mod world;

pub use components::*;
pub use config::Config;
pub use entity::EntityId;
pub use events::{Event, EventKind};
pub use exec::{ExecState, FailReason, StepResult};
pub use goap::{ActionKind, GoalState, Key, LocationKey, Plan, PlanCtx, StealSource, WorldState};
pub use levers::{Levers, PlayerCommand, Speed};
pub use map::Map;
pub use stats::{DailyStats, DayRow};
pub use time::{DayPhase, Season, Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
pub use utility::{Consideration, GoalScore, ThinkTrace};
pub use world::{load_stat_table, StatRow, StatTable, World, STAT_ROWS};

/// Advance the world by exactly one tick (one in-game minute).
pub fn tick(world: &mut World) {
    world.tick();
}
