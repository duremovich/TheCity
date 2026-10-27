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

/// The perf pass phase 5: "is a guard other than `id` within `sight` tiles
/// of `t`" for the tick, from one [`crate::spatial::TileGrid`] of the
/// guards' tiles (rebuilt per call: guards move every tick). In a unit test
/// with `spatial::scans()` on, the row-sorted list it replaced.
fn guard_near_fn(world: &World, sight: u32) -> impl Fn(EntityId, crate::components::TilePos) -> bool {
    let mut guard_tiles: Vec<(u16, EntityId, crate::components::TilePos)> = world
        .guards()
        .iter()
        .copied()
        .filter_map(|id| world.comp::<Position>(id).map(|p| (p.tile.y, id, p.tile)))
        .collect();
    let scans = crate::spatial::scans();
    let grid = if scans {
        guard_tiles.sort_unstable();
        None
    } else {
        let tiles = guard_tiles.iter().map(|&(_, _, t)| t);
        Some(crate::spatial::TileGrid::build(world.map.w(), world.map.h(), sight.max(1), tiles))
    };
    move |id: EntityId, t: crate::components::TilePos| -> bool {
        if let Some(grid) = &grid {
            return grid.any_near_with(t, sight, |i| {
                let (_, g, gt) = guard_tiles[i as usize];
                g != id && gt.manhattan(t) <= sight
            });
        }
        let lo = guard_tiles.partition_point(|&(y, _, _)| u32::from(y) + sight < u32::from(t.y));
        guard_tiles[lo..]
            .iter()
            .take_while(|&&(y, _, _)| u32::from(y) <= u32::from(t.y) + sight)
            .any(|&(_, g, gt)| g != id && gt.manhattan(t) <= sight)
    }
}

/// One meal.
pub fn eat(n: &mut Needs, cfg: &NeedsCfg) {
    n.hunger = (n.hunger + cfg.food_satisfy).min(1.0);
}

pub fn run(world: &mut World) {
    let season = world.season().index();
    let season_energy_mult = world.config.economy.energy_decay_mult[season];
    let hourly = world.tick.is_multiple_of(TICKS_PER_HOUR);

    // Guard positions once per tick for the safety modifier. The perf pass
    // phase 5: on a grid of `sight`-tile cells, so each agent reads the
    // guards in the cells around it, not every guard within `sight` rows
    // across the map (the row-sorted list's band; M11 sorted it to cut the
    // all-pairs scan). "Any guard within `sight`" is order-free: the grid's
    // superset, cut by the same test, is the same answer.
    let sight = world.config.crime.sight;
    let guard_near_at = guard_near_fn(world, sight);

    let assets = world.config.assets.enabled;
    let lights_out = crate::systems::law::in_lights_out(world);
    for id in world.bodies() {
        let Some(brain) = world.comp::<Brain>(id) else { continue };
        if brain.lod == Lod::Statistical {
            continue;
        }
        // M13 D39: one Body read (withdrawal is derived from `last_use`).
        let energy_mult = if assets { crate::systems::stims::energy_mult(world, id) } else { None };
        // Hunger as sickness § 2: the body's energy debuff (None stays None off).
        let energy_mult = crate::systems::health::energy_mult(world, id, energy_mult);
        // Lives at work phase 2 (§ 3.4): a new parent's nights (None stays None off).
        let energy_mult = match (energy_mult, crate::systems::household::energy_mult(world, id)) {
            (m, None) => m,
            (None, Some(n)) => Some(n),
            (Some(m), Some(n)) => Some(m * n),
        };
        // Pressure pass round 3, S3: a prisoner sleeps in the cell at lights-out.
        let sleeping = matches!(brain.exec, ExecState::Use { kind: ActionKind::Sleep, .. })
            || (lights_out && crate::systems::law::asleep_in_cell(world, id));
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
            // Hunger as sickness § 2: the hour's hungry mark.
            crate::systems::health::hourly_mark(world, id);
        }
        // L2 L13: an hour of `fun` at every body (a no-op with leisure off).
        crate::systems::leisure::bodies_hourly(world);
    }
}

/// Starvation bookkeeping for one agent; may kill. Per tick for Full and
/// Coarse agents, hourly for Statistical ones.
pub fn starvation(world: &mut World, id: EntityId) {
    let tick = world.tick;
    // Hunger as sickness § 7: a week at hunger 0, not three days.
    let grace = if crate::systems::diet::on(world) {
        world.config.sickness.starve_grace_days * time::TICKS_PER_DAY
    } else {
        world.config.needs.starvation_grace_ticks
    };
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
        // Real economy (plan E34): by the agent's Home district, else its tile's.
        let d = match world.comp::<crate::components::Household>(id).and_then(|h| h.home) {
            Some(h) => world.district_of_building(h),
            None => world
                .comp::<crate::components::Position>(id)
                .map_or(crate::components::DistrictId(0), |p| world.district_of(p.tile)),
        };
        if let Some(n) = world.stats.current.econ.d_starving.get_mut(d.index()) {
            *n += 1;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{DeathCause, Job, Role, TilePos};
    use crate::time::TICKS_PER_DAY;

    /// The perf pass phase 5: the needs' guard grid (`guard_near_fn`) is
    /// the row-sorted list it replaced (`spatial::scans()`) for every body
    /// and at tiles around every guard, through a day of the city, a guard
    /// hired, one killed and one moved. The grid is rebuilt per tick, so it
    /// has no invalidation to miss; this pins the cell cut (a cell too
    /// small, or a ring short, drops a guard at the reach's edge).
    #[test]
    fn test_guard_grid_matches_the_scan() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 9 * TICKS_PER_HOUR);
        let sight = w.config.crime.sight;
        let check = |w: &World, at: &str| -> usize {
            let grid = guard_near_fn(w, sight);
            crate::spatial::SCANS.with(|s| s.set(true));
            let scan = guard_near_fn(w, sight);
            crate::spatial::SCANS.with(|s| s.set(false));
            let mut probes: Vec<(EntityId, TilePos)> =
                w.bodies().into_iter().filter_map(|id| w.comp::<Position>(id).map(|p| (id, p.tile))).collect();
            // Tiles at and just past the reach of every guard, on each axis.
            let nobody = EntityId { index: u32::MAX, generation: 0 };
            for &g in w.guards() {
                let Some(t) = w.comp::<Position>(g).map(|p| p.tile) else { continue };
                for d in [sight, sight + 1] {
                    let d = d as u16;
                    let (x0, x1, y1) = (t.x.saturating_sub(d), t.x.saturating_add(d), t.y.saturating_add(d));
                    for at in [TilePos { x: x1, y: t.y }, TilePos { x: x0, y: t.y }, TilePos { x: t.x, y: y1 }] {
                        probes.push((g, at));
                        probes.push((nobody, at));
                    }
                }
            }
            let mut near = 0;
            for &(id, t) in &probes {
                let want = scan(id, t);
                assert_eq!(grid(id, t), want, "{at}: {id} at {t:?}");
                near += usize::from(want);
            }
            near
        };
        let mut near = check(&w, "day 1 09:00");
        // A guard hired: a citizen with a guard's Job.
        let guards = w.guards().to_vec();
        let citizens = w.citizens(); // scan-ok: a unit test.
        let recruit = citizens.into_iter().find(|a| !guards.contains(a) && w.has::<Position>(*a)).expect("a citizen");
        let job = w.comp::<Job>(guards[0]).cloned().expect("a guard's Job");
        assert_eq!(job.role, Role::Guard);
        w.insert(recruit, job);
        near += check(&w, "a guard hired");
        // One killed, one moved across the map.
        w.kill(guards[1], DeathCause::Violence);
        near += check(&w, "a guard killed");
        if let Some(p) = w.comp_mut::<Position>(guards[2]) {
            p.tile = TilePos { x: 3, y: 3 };
            p.building = None;
        }
        near += check(&w, "a guard moved");
        for _ in 0..6 {
            w.run_ticks(61);
            near += check(&w, &format!("tick {}", w.tick));
        }
        assert!(near > 0, "no body was near a guard: the test exercises nothing");
    }
}
