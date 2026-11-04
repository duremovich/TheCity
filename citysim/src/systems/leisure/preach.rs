use super::*;

/// Plan L20: a Purist gang's preacher today: under Expand, `preach_share`
/// of the members by `id.index % 100`, a busiest spot in a held district.
/// Jobs & lives phase 6: a cult's members preach too, under any order but a
/// raid (Shadow V4 bug 13: under Expand only, the Choir at LieLow, Squat or
/// Harvest never preached).
pub fn preacher(world: &World, id: EntityId) -> bool {
    let Some(gang) = world.gang_of(id) else { return false };
    let order = world.comp::<Gang>(gang).map(|g| g.order);
    let purist = crate::systems::creeds::is_purist(world, gang) && order == Some(crate::components::Order::Expand);
    let cult = crate::systems::faith::cult_preacher(world, gang) && order.is_some_and(|o| !o.is_raid());
    (purist || cult)
        && u64::from(id.index % 100) < (world.config.leisure.preach_share * 100.0) as u64
        && world.comp::<Brain>(id).is_none_or(|b| b.gang_task_day != Some(world.day()))
}

/// The busiest spot (most hanging out now, then the nearest to `from`, then
/// the lower tile) of the gang's held districts, else of its Hideout's
/// district. Pressure pass F2: the preacher's own district first when the
/// gang holds it, and ties by distance (with nobody hanging out anywhere the
/// lowest tile of every held district won: a preacher in D4 set out for a
/// corner of the map 2-3 h away, 17 plans and no Preach in a week).
fn busiest_spot(world: &World, id: EntityId, gang: EntityId, from: TilePos) -> Option<TilePos> {
    // Pressure pass round 6, V2 (TURF § 2): with `[turf]`, the preacher
    // preaches on his gang's own corners (the busiest within his reach), and
    // only without one in reach on the district's spots as before.
    if crate::systems::turf::on(world) {
        let cap = crate::systems::distance::choice(world).map(|c| c.gang_work_ticks);
        let rides = std::cell::OnceCell::new();
        let rides = || *rides.get_or_init(|| crate::systems::distance::rider(world, id));
        let best = crate::systems::turf::corners_of(world, gang)
            .into_iter()
            .filter(|&t| {
                cap.is_none_or(|c| crate::systems::distance::trip_by(world, from, t, rides, Some(world.tick)) <= c)
            })
            .map(|t| (world.hangouts.get(&t).map_or(0, |v| v.len()), t.manhattan(from), t))
            .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)))
            .map(|(_, _, t)| t);
        if best.is_some() {
            return best;
        }
    }
    let mut ds: Vec<DistrictId> =
        crate::systems::gang::held_districts(world, gang).into_iter().map(|(d, _)| d).collect();
    let here = world.district_of(from);
    if ds.contains(&here) {
        ds = vec![here];
    }
    if ds.is_empty() {
        ds.push(world.district_of_building(world.hideout_of(gang)?));
    }
    // Pressure pass round 3, S1 (OPEN-221): a spot past `[distance.choice]
    // gang_work_ticks` of the preacher is no pulpit (V11: a cultist's five
    // sermons, each walk cut, no Preach).
    let cap = crate::systems::distance::choice(world).map(|c| c.gang_work_ticks);
    let rides = std::cell::OnceCell::new();
    let rides = || *rides.get_or_init(|| crate::systems::distance::rider(world, id));
    ds.iter()
        .filter_map(|d| world.spots.get(d.index()))
        .flat_map(|v| v.iter())
        .filter(|s| !matches!(s.kind, SpotKind::Hideout(g) if g != gang))
        .filter(|s| {
            cap.is_none_or(|c| crate::systems::distance::trip_by(world, from, s.tile, rides, Some(world.tick)) <= c)
        })
        .map(|s| (world.hangouts.get(&s.tile).map_or(0, |v| v.len()), s.tile.manhattan(from), s.tile))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)))
        .map(|(_, _, t)| t)
}

/// Plan L20: Preach's GangWork: `GoTo(Spot) → Preach` at the busiest spot.
pub fn preach_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !preacher(world, id) {
        return None;
    }
    let gang = world.gang_of(id)?;
    let from = world.comp::<Position>(id)?.tile;
    let tile = busiest_spot(world, id, gang, from)?;
    world.unwind.insert(
        id,
        UnwindPick {
            rung: Rung::Free,
            act: ActionKind::Preach,
            venue: None,
            spot: Some(tile),
            score: 0.0,
            stay_in: false,
        },
    );
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::Preach, None)];
    Some(Plan { goal: GoalKind::GangWork, target: None, steps, started_tick: world.tick })
}

/// Plan L20: a Preach ends: a Persuade (M15 `moves::resolve`, stake the
/// gang) on the co-hanger of highest affinity (else the first present);
/// success leaves the heard `Persuaded` entry and `+preach_opinion` on the
/// listener's edge to the gang's leader, `Preached`. The day's GangWork is
/// done either way.
pub(super) fn preach_done(world: &mut World, id: EntityId) {
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.gang_task_day = Some(today);
    }
    let Some(gang) = world.gang_of(id) else { return };
    let tile = crate::systems::leisure::spot_tile(world, id);
    let listener = tile
        .and_then(|t| world.hangouts.get(&t))
        .map(|v| {
            v.iter()
                .copied()
                .filter(|&o| o != id && world.has::<Brain>(o) && world.gang_of(o) != Some(gang))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|o| (o, world.edge(id, o).map_or(f32::NEG_INFINITY, |e| e.affinity)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(o, _)| o);
    let Some(t) = listener else { return };
    let m = crate::word::SocialMove {
        actor: id,
        target: t,
        kind: crate::word::MoveKind::Persuade,
        stake: crate::word::Stake::Join(gang),
    };
    let out = crate::systems::moves::resolve(world, &m);
    if !out.success {
        return;
    }
    if let Some(l) = world.comp::<Gang>(gang).and_then(|g| g.leader).filter(|&l| l != t) {
        let tick = world.tick;
        let gain = world.config.leisure.preach_opinion;
        let e = world.edge_entry(t, l);
        e.affinity = (e.affinity + gain).min(1.0);
        e.last_interaction = tick;
    }
    world.stats.current.living.preached += 1;
    // Jobs & lives phase 6: a cult's success converts the listener.
    crate::systems::faith::on_preached(world, gang, t);
    let text = format!("{} preached to {}", world.name_of(id), world.name_of(t));
    world.push_event(EventKind::Preached, &[id, t], text);
}

// ---------------------------------------------------------------------------
// The hand-over of the seeded venues (phase 1 deviation)
// ---------------------------------------------------------------------------

/// The seeded venues on the city's deed pass to their heir (a front's gang,
/// a Club's or the Lounge's megacorp) once they earn: from day 14, each of
/// the last 7 days' revenue covered a day of the staff's wages and the
/// kind's upkeep, and the heir's purse holds a week of both.
pub fn hand_over(world: &mut World) {
    let mut moves: Vec<(EntityId, EntityId)> = Vec::new();
    for kind in BuildingKind::LEISURE {
        for &b in world.buildings_of_kind(kind) {
            let Some(bd) = world.comp::<Building>(b).filter(|bd| bd.owner.is_none() && !bd.demolished && !bd.derelict)
            else {
                continue;
            };
            let Some(v) = bd.venue.as_ref() else { continue };
            let Some(heir) = v.heir.or(v.front_of) else { continue };
            if !(world.has::<Gang>(heir) || world.has::<Corp>(heir)) || bd.revenue.len() < 7 {
                continue;
            }
            // Every one of the last 7 days covered the day's wages and
            // upkeep, past the opening fortnight (measured: the day-1 burst
            // of full purses passed a 7-day sum, then the venues' takings
            // fell and the heirs' Cooks, Bartenders and Fighters went unpaid).
            let worst: i64 = bd.revenue.iter().rev().take(7).copied().min().unwrap_or(0);
            let wages: i64 = ownership::staff_at(world, b)
                .into_iter()
                .filter_map(|s| world.comp::<Job>(s).map(|j| j.wage_per_day))
                .sum();
            let upkeep = {
                let t = world.comp::<Building>(b).map_or(0, |x| x.tier);
                world.config.corps.upkeep.for_building(kind, t)
            };
            // ... and the heir holds a week of them (a gang at its stipends' floor
            // took a pit whose Fighters then went unpaid).
            if world.day() >= 14 && worst >= wages + upkeep && world.purse(Some(heir)) >= 7 * (wages + upkeep) {
                moves.push((b, heir));
            }
        }
    }
    for (b, heir) in moves {
        crate::systems::corps::move_building(world, b, Some(heir));
        if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
            v.heir = None;
            // Review fix: the city-era take is the Treasury's, not the week's.
            v.take_week = 0;
        }
        let text = format!("{} took over {} (it earns)", world.owner_label(Some(heir)), world.name_of(b));
        world.push_event(EventKind::Founded, &[heir, b], text);
    }
}
