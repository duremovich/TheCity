//! Need decay, satisfiers, starvation. Runs every tick for Full and Coarse
//! agents; the Statistical tick (M7) applies 60 steps at once.

use crate::components::{Brain, Identity, Lod, MemoryKind, Needs, Personality, Position, Sentence};
use crate::config::NeedsCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::ActionKind;
use crate::time::{self, TICKS_PER_HOUR};
use crate::world::World;

/// Everything that modifies one agent's decay this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct DecayCtx {
    pub sleeping: bool,
    /// Asleep on the street: energy recovers at x0.6.
    pub sleeping_outside: bool,
    pub farm_working: bool,
    pub jailed: bool,
    /// Fleeing or fighting.
    pub exerting: bool,
    pub season_energy_mult: f32,
    pub guard_near: bool,
    pub threat_near: bool,
    pub under_18: bool,
    pub sociability: f32,
    /// M13 D39: energy decay × this in withdrawal; `None` is today's
    /// arithmetic (a branch, not a multiply by 1).
    pub energy_mult: Option<f32>,
}

impl DecayCtx {
    pub fn plain() -> Self {
        DecayCtx { season_energy_mult: 1.0, sociability: 0.5, ..Default::default() }
    }
}

/// Apply `ticks` ticks of decay to one agent's needs.
pub fn decay(n: &mut Needs, cfg: &NeedsCfg, ctx: &DecayCtx, ticks: u32) {
    let t = ticks as f32;
    let mut hunger = cfg.hunger_decay_per_tick;
    if ctx.jailed || ctx.sleeping {
        hunger *= 0.5;
    }
    if ctx.farm_working {
        hunger *= 1.3;
    }
    n.hunger = (n.hunger - hunger * t).max(0.0);

    if ctx.sleeping {
        let gain = if ctx.sleeping_outside { 0.6 } else { 1.0 };
        n.energy = (n.energy + gain * t / cfg.sleep_ticks_full as f32).min(1.0);
    } else {
        let mut energy = cfg.energy_decay_per_tick * ctx.season_energy_mult;
        if ctx.exerting {
            energy *= 1.5;
        }
        if let Some(m) = ctx.energy_mult {
            energy *= m;
        }
        n.energy = (n.energy - energy * t).max(0.0);
    }

    if !ctx.threat_near {
        let mut recover = cfg.safety_recover_per_tick;
        if ctx.guard_near {
            recover *= 2.0;
        }
        n.safety = (n.safety + recover * t).min(1.0);
    }

    let belonging = cfg.belonging_decay_per_tick * (0.5 + ctx.sociability);
    n.belonging = (n.belonging - belonging * t).max(0.0);

    let mut intimacy = cfg.intimacy_decay_per_tick;
    if ctx.under_18 {
        intimacy *= 0.5;
    }
    n.intimacy = (n.intimacy - intimacy * t).max(0.0);
}

/// One meal.
pub fn eat(n: &mut Needs, cfg: &NeedsCfg) {
    n.hunger = (n.hunger + cfg.food_satisfy).min(1.0);
}

pub fn run(world: &mut World) {
    let season = world.season().index();
    let season_energy_mult = world.config.economy.energy_decay_mult[season];
    let hourly = world.tick.is_multiple_of(TICKS_PER_HOUR);

    // Guard positions once per tick for the safety modifier.
    // Sorted by row so each agent checks only the guards within `sight` rows
    // (M11: twelve private guards made the all-pairs scan a third dearer).
    let mut guard_tiles: Vec<(u8, EntityId, crate::components::TilePos)> = world
        .guards()
        .iter()
        .copied()
        .filter_map(|id| world.comp::<Position>(id).map(|p| (p.tile.y, id, p.tile)))
        .collect();
    guard_tiles.sort_unstable();
    let sight = world.config.crime.sight;
    let guard_near_at = |id: EntityId, t: crate::components::TilePos| -> bool {
        let lo = guard_tiles.partition_point(|&(y, _, _)| u32::from(y) + sight < u32::from(t.y));
        guard_tiles[lo..]
            .iter()
            .take_while(|&&(y, _, _)| u32::from(y) <= u32::from(t.y) + sight)
            .any(|&(_, g, gt)| g != id && gt.manhattan(t) <= sight)
    };

    let assets = world.config.assets.enabled;
    for id in world.bodies() {
        let Some(brain) = world.comp::<Brain>(id) else { continue };
        if brain.lod == Lod::Statistical {
            continue;
        }
        // M13 D39: one Body read (withdrawal is derived from `last_use`).
        let energy_mult = if assets { crate::systems::stims::energy_mult(world, id) } else { None };
        let sleeping = matches!(brain.exec, ExecState::Use { kind: ActionKind::Sleep, .. });
        let sleeping_outside = sleeping && world.comp::<Position>(id).is_some_and(|p| p.building.is_none());
        let farm_working = matches!(brain.exec, ExecState::Use { kind: ActionKind::FarmWork, .. });
        let jailed = world.has::<Sentence>(id);
        let tile = world.comp::<Position>(id).map(|p| p.tile);
        let guard_near = tile.is_some_and(|t| guard_near_at(id, t));
        let under_18 = world.comp::<Identity>(id).is_some_and(|i| i.age_days < 18 * 120);
        let sociability = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
        let ctx = DecayCtx {
            sleeping,
            sleeping_outside,
            farm_working,
            jailed,
            exerting: false,
            season_energy_mult,
            guard_near,
            threat_near: false,
            under_18,
            sociability,
            energy_mult,
        };
        let cfg = world.config.needs.clone();
        let Some(n) = world.comp_mut::<Needs>(id) else { continue };
        decay(n, &cfg, &ctx, 1);
        starvation(world, id);
    }

    // Bodies only: a Statistical agent's wealth is refreshed by its own
    // spread hourly run (`lod::run_statistical`).
    if hourly {
        for id in world.bodies() {
            world.recompute_wealth_for(id);
        }
        // L2 L13: an hour of `fun` at every body (a no-op with leisure off).
        crate::systems::leisure::bodies_hourly(world);
    }
}

/// Starvation bookkeeping for one agent; may kill. Per tick for Full and
/// Coarse agents, hourly for Statistical ones.
pub fn starvation(world: &mut World, id: EntityId) {
    let tick = world.tick;
    let grace = world.config.needs.starvation_grace_ticks;
    let Some(n) = world.comp_mut::<Needs>(id) else { return };
    if n.hunger > 0.0 {
        n.starving_since = None;
        return;
    }
    let started = n.starving_since.is_none();
    let since = *n.starving_since.get_or_insert(tick);
    if started {
        let name = world.name_of(id);
        world.push_event(EventKind::Starving, &[id], format!("{name} is starving"));
    }
    // One Starved memory and one trait drift per day of starvation.
    let today = time::day(tick);
    let remembered_today = world
        .comp::<crate::components::Memory>(id)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Starved && time::day(e.tick) == today));
    if !remembered_today {
        world.remember(id, MemoryKind::Starved, None, 0.6, -0.6, false);
        if let Some(p) = world.comp_mut::<Personality>(id) {
            p.lawfulness = (p.lawfulness - 0.02).max(0.0);
            p.courage = (p.courage + 0.01).min(1.0);
        }
    }
    if tick.saturating_sub(since) >= grace {
        world.kill(id, crate::components::DeathCause::Starvation);
    }
}
