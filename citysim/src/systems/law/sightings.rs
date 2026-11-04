use super::*;

/// Any guard perceiving (SIGHT, or same building) a wanted suspect records it.
///
/// Scale S1: the guards are bucketed once a tick (a [`TileGrid`] by tile, a
/// map by building), so each suspect reads only the guards in its square
/// and its building, not every guard in the city: the same pairs, the same
/// `seen`, the same seers in ascending order (`sightings_scan` is the old
/// loop, kept as the test's reference).
pub(super) fn sightings(world: &mut World) {
    sightings_with(world, SIGHTINGS_GRID_PAIRS);
}

/// [`sightings`], with the grid past `grid_pairs` guard-suspect pairs.
pub(super) fn sightings_with(world: &mut World, grid_pairs: usize) {
    let open = world.open_suspects().count();
    if open == 0 {
        return;
    }
    let sight = world.config.crime.sight;
    // M13 D32: a guard with Eyes sees `Kit.sight` tiles further.
    let guards = guard_sights(world);
    let max_extra = guards.iter().map(|g| g.extra).max().unwrap_or(0);
    // A small city (a few thousand guard-suspect pairs) reads every guard
    // for each suspect: the grid's upkeep would cost more than it saves.
    let grid = (open * guards.len() > grid_pairs).then(|| {
        let grid = crate::spatial::TileGrid::build(
            world.map.w(),
            world.map.h(),
            sight + max_extra + 1,
            guards.iter().map(|g| g.tile),
        );
        let mut in_building: BTreeMap<EntityId, Vec<u32>> = BTreeMap::new();
        for (i, g) in guards.iter().enumerate() {
            if let Some(b) = g.building {
                in_building.entry(b).or_default().push(i as u32);
            }
        }
        (grid, in_building)
    });
    // With the grid, only a suspect with a guard in its square's cells or its
    // building can be seen (positions do not move in this pass): the rest
    // are passed over before their stores are read.
    let suspects: Vec<EntityId> = world
        .open_suspects()
        .filter(|&s| {
            grid.as_ref().is_none_or(|(grid, in_building)| {
                world.comp::<Position>(s).is_some_and(|p| {
                    grid.any_near(p.tile, sight + max_extra) || p.building.is_some_and(|b| in_building.contains_key(&b))
                })
            })
        })
        .filter(|&s| (living(world, s)) && !world.has::<Sentence>(s))
        .collect();
    let tick = world.tick;
    // M15 W12 (plan deviation: hourly, not every tick): the guards who see a
    // suspect hold a Sighting of them.
    let note = tick.is_multiple_of(crate::time::TICKS_PER_HOUR);
    let mut near: Vec<u32> = Vec::new();
    for s in suspects {
        let Some(ps) = world.comp::<Position>(s) else { continue };
        let (tile, building) = (ps.tile, ps.building);
        // Round 5 U4 (`OPEN-236`): the walls hold (`sees_scene`).
        let scene_out = outdoors(world, tile, building);
        let sees = |g: &GuardSight| {
            (g.building.is_some() && g.building == building)
                || (scene_out && g.outside && chebyshev(g.tile, tile) <= sight + g.extra)
        };
        // The guards who see `s`, ascending (`guards` is ascending by id, so
        // ascending indices are): from the grid's cells and the building, or
        // without the grid, only whether any does until a note needs them.
        near.clear();
        let seen = match &grid {
            Some((grid, in_building)) => {
                grid.near(tile, sight + max_extra, &mut near);
                if let Some(list) = building.and_then(|b| in_building.get(&b)) {
                    near.extend_from_slice(list);
                }
                near.sort_unstable();
                near.dedup();
                near.retain(|&i| sees(&guards[i as usize]));
                !near.is_empty()
            }
            None => guards.iter().any(sees),
        };
        if !seen {
            continue;
        }
        // Jobs & lives phase 6 (§ 7): papers bought while wanted pass the
        // hour's sighting on `id_w × tier` (nothing off: the map is empty).
        if !world.jobs2.gray.papers.is_empty() && crate::systems::gray::passes_sighting(world, s) {
            continue;
        }
        world.set_last_seen(s, (tile, tick));
        if note {
            if grid.is_none() {
                near.extend((0..guards.len() as u32).filter(|&i| sees(&guards[i as usize])));
            }
            let seers: Vec<EntityId> = near.iter().map(|&i| guards[i as usize].id).collect();
            for g in seers {
                crate::systems::gossip::maybe_sight(world, g, s, building, tile);
            }
        }
    }
}

/// Scale S1: past this many guard-suspect pairs a tick, `sightings` buckets
/// the guards by tile (the 2,000 city reads ~2,300 a tick over 30 days on
/// seed 42, the 20,000 probe city hundreds of thousands).
const SIGHTINGS_GRID_PAIRS: usize = 16_384;

/// A guard as `sightings` reads it: where, inside what, whether in the open
/// (Round 5 U4, `law::outdoors`), and its Eyes' reach.
#[derive(Clone, Copy, Debug)]
struct GuardSight {
    id: EntityId,
    tile: TilePos,
    building: Option<EntityId>,
    outside: bool,
    extra: u32,
}

/// Every guard with a Position, ascending by id.
fn guard_sights(world: &World) -> Vec<GuardSight> {
    world
        .guards()
        .iter()
        .filter_map(|&g| {
            let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(g).map(|p| GuardSight {
                id: g,
                tile: p.tile,
                building: p.building,
                outside: outdoors(world, p.tile, p.building),
                extra,
            })
        })
        .collect()
}

/// The pre-S1 `sightings` (every suspect against every guard), kept for
/// `test_sightings_grid_matches_the_scan`.
#[cfg(test)]
pub(super) fn sightings_scan(world: &mut World) {
    let suspects: Vec<EntityId> =
        world.open_suspects().filter(|&s| (living(world, s)) && !world.has::<Sentence>(s)).collect();
    if suspects.is_empty() {
        return;
    }
    let sight = world.config.crime.sight;
    let guards: Vec<(Position, u32)> = world
        .guards()
        .iter()
        .filter_map(|&g| {
            let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(g).map(|p| (p.clone(), extra))
        })
        .collect();
    let tick = world.tick;
    let note = tick.is_multiple_of(crate::time::TICKS_PER_HOUR);
    let ids: Vec<EntityId> = if note { world.guards().to_vec() } else { Vec::new() };
    for s in suspects {
        let Some(ps) = world.comp::<Position>(s) else { continue };
        let (tile, building) = (ps.tile, ps.building);
        let out = outdoors(world, tile, building);
        let seen = guards.iter().any(|(p, extra)| sees_scene(world, p, tile, building, out, sight + extra));
        if seen {
            world.set_last_seen(s, (tile, tick));
            if note {
                let seers: Vec<EntityId> = ids
                    .iter()
                    .copied()
                    .filter(|&g| {
                        let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
                        world
                            .comp::<Position>(g)
                            .is_some_and(|p| sees_scene(world, p, tile, building, out, sight + extra))
                    })
                    .collect();
                for g in seers {
                    crate::systems::gossip::maybe_sight(world, g, s, building, tile);
                }
            }
        }
    }
}

pub(super) fn expire_warrants(world: &mut World) {
    let expiry = world.config.crime.warrant_expiry_days * TICKS_PER_DAY;
    let tick = world.tick;
    let mut expired = Vec::new();
    for r in world.reports_mut().iter_mut().filter(|r| !r.resolved && tick.saturating_sub(r.tick) >= expiry) {
        r.resolved = true;
        expired.push(r.suspect);
    }
    for s in expired {
        if let Some(p) = world.comp_mut::<Personality>(s) {
            p.drift(Drift::CrimeUnpunished);
        }
        let name = world.name_of(s);
        world.push_event(EventKind::Unpunished, &[s], format!("warrant on {name} expired"));
    }
    world.reports_mut().retain(|r| !r.resolved || tick.saturating_sub(r.tick) < 2 * expiry);
}
