use super::*;

/// The next patrol route: `[Market, Bar, Hall, Home(rng), Home(rng)]` on a
/// beat. M10: with several Markets the beat is one drawn at random, the Bar
/// is the one nearest it and the Homes come from the `[law]
/// patrol_beat_homes` nearest it, so a loop fits in a shift on the 256 x 192
/// map (with every Home in play a single leg could take half the shift). On
/// the v1 map (one Market, 60 Homes) the loop is the v1 loop. Under
/// a Crackdown: `[Market, Home(t), Home(t), Hideout(t), Home(t)]`, the Homes
/// drawn from the target gang's territory (or, with fewer than three held,
/// from the six inhabited Homes nearest its Hideout).
///
/// M12 D11: a city guard with a district beat (`Law.beats`, dealt daily by
/// `law_brain::allocate`) walks its district instead ([`district_route`]);
/// a guard without one (districts off, Garrison, a fresh load) walks the
/// M11 route above exactly.
pub fn new_patrol_route(world: &mut World, guard: EntityId) -> Vec<EntityId> {
    if let Some(route) = private_patrol_route(world, guard) {
        return route;
    }
    let beat = world
        .law()
        .filter(|_| world.config.law.district_beats)
        .and_then(|l| l.beats.get(&guard).copied())
        .filter(|d| d.index() < world.districts.len());
    if let Some(d) = beat {
        let from = crate::exec::walk_origin(world, guard);
        return district_route(world, d, from);
    }
    let target = world.law().filter(|l| l.posture == Posture::Crackdown).and_then(|l| l.target);
    if let Some(gang) = target.filter(|&g| world.has::<crate::components::Gang>(g)) {
        return crackdown_route(world, gang, None);
    }
    let mut route = Vec::new();
    let markets: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .collect();
    // No draw with a single Market, so the v1 city keeps its v1 sequence.
    let beat = match markets.len() {
        0 => None,
        1 => Some(markets[0]),
        n => Some(markets[world.rng.world().random_range(0..n)]),
    };
    let beat_door = beat.and_then(|m| world.comp::<Building>(m)).map(|b| b.door);
    route.extend(beat);
    route.extend(beat_door.and_then(|d| world.nearest_of_kind(BuildingKind::Bar, d)));
    route.extend(world.building_of_kind(BuildingKind::Hall));
    let mut homes = world.buildings_by_kind().get(&BuildingKind::Home).cloned().unwrap_or_default();
    let beat_homes = world.config.law.patrol_beat_homes;
    if let (Some(door), true) = (beat_door, homes.len() > beat_homes) {
        let mut near: Vec<(u32, EntityId)> = homes
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(door), h)))
            .collect();
        near.sort();
        homes = near.into_iter().take(beat_homes).map(|(_, h)| h).collect();
    }
    if !homes.is_empty() {
        for _ in 0..2 {
            let i = world.rng.world().random_range(0..homes.len());
            route.push(homes[i]);
        }
    }
    route
}

/// M12 D11: a beat in district `d`. Under the district's `Crackdown(g)` the
/// crackdown loop on g's turf in d; else a Market in d drawn as the city's
/// beat is (no draw with one), the Bar nearest it, the Hall only when d
/// holds it, and two Homes drawn from the `patrol_beat_homes` Homes of d
/// nearest the Market (none in d: the nearest anywhere). Plan deviation: a
/// district with Homes and no Market (the Spire, the three Sumps) walks
/// four Homes drawn from the `patrol_beat_homes` nearest its centroid, so
/// the beat stays in the district; with the plan's Market nearest the
/// centroid, Sump guards spent their loop at a Mid Market and the Sump read
/// the coverage floor (0.5) with four guards allocated.
///
/// The pressure pass F1 (`[distance]` on, `from` the guard's place): the
/// Market is drawn among the district's within `[distance] job_cap_ticks`
/// of `from` (none: the nearest), and the Hall joins only within that
/// travel of the beat. A district label spans up to 1,000 tiles on the
/// island: a random Market of it sent guards on 4-13 h patrol legs.
/// The pressure pass R2: a district with no Market walks the
/// `patrol_beat_homes` of its Homes nearest `from`, not its centroid.
pub fn district_route(
    world: &mut World,
    d: crate::components::DistrictId,
    from: Option<crate::components::TilePos>,
) -> Vec<EntityId> {
    use crate::components::Stance;
    // M12 D34: under a Cordon every guard on the beat stands at the riot's target.
    if world.district(d).stance == Stance::Cordon {
        if let Some(t) = crate::systems::riot::cordon_target(world, d) {
            return vec![t];
        }
    }
    if let Stance::Crackdown(g) = world.district(d).stance {
        if world.has::<crate::components::Gang>(g) {
            return crackdown_route(world, g, Some(d));
        }
    }
    // Round 6 V5 (TURF § 3): the law's crackdown on a turf cell of the
    // district: the beat walks the cell's Homes.
    if let Some(route) = crate::systems::turf::crackdown_beat(world, d) {
        return route;
    }
    let standing = |w: &World, b: EntityId| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished);
    let of_kind = |w: &World, k: BuildingKind| -> Vec<EntityId> {
        w.district(d)
            .buildings
            .iter()
            .copied()
            .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| bd.kind == k && !bd.demolished))
            .collect()
    };
    let markets = of_kind(world, BuildingKind::Market);
    // R2: the patrol's own reach (`[distance.reach] patrol_ticks`), else
    // the job cap.
    let patrol_cap = crate::systems::distance::reach(world).and_then(|r| r.patrol_ticks);
    let near_cap = crate::systems::distance::job_cap(world).map(|c| patrol_cap.unwrap_or(c)).zip(from);
    // The draw stays one over the district's Markets (the world stream
    // unchanged), mapped onto those within reach.
    let mut pool = markets.clone();
    // The pressure pass R2 (OPEN-215): with `[distance]` on, a district
    // whose Markets all lie past the cap of the guard walks its Homes
    // nearest him (below), one Market or several: the Spire's one Market
    // sent Mid-Levels guards on 340-550-tile legs.
    let homes_d: Vec<EntityId> = world.district(d).homes.iter().copied().filter(|&h| standing(world, h)).collect();
    let far_markets = near_cap.is_some_and(|(cap, f)| {
        !homes_d.is_empty()
            && markets.iter().all(|&m| {
                world.comp::<Building>(m).is_none_or(|bd| crate::systems::distance::travel(world, f, bd.door) > cap)
            })
    });
    if let Some((cap, f)) = near_cap.filter(|_| markets.len() > 1) {
        let door = |w: &World, b: EntityId| w.comp::<Building>(b).map_or(f, |bd| bd.door);
        let within: Vec<EntityId> = markets
            .iter()
            .copied()
            .filter(|&m| crate::systems::distance::travel(world, f, door(world, m)) <= cap)
            .collect();
        pool = if within.is_empty() {
            markets.iter().copied().min_by_key(|&m| (door(world, m).manhattan(f), m)).into_iter().collect()
        } else {
            within
        };
    }
    let hall_here = !of_kind(world, BuildingKind::Hall).is_empty();
    let centroid = world.district(d).centroid;
    let beat = match markets.len() {
        _ if far_markets => None,
        0 if !homes_d.is_empty() => None,
        0 => world.nearest_of_kind(BuildingKind::Market, centroid),
        1 => Some(markets[0]),
        n => {
            let i = world.rng.world().random_range(0..n);
            Some(pool[i % pool.len().max(1)])
        }
    };
    let draws = if (markets.is_empty() || far_markets) && !homes_d.is_empty() { 4 } else { 2 };
    let beat_door = beat.and_then(|m| world.comp::<Building>(m)).map(|b| b.door);
    let mut route = Vec::new();
    route.extend(beat);
    if beat.is_some() {
        let bar = beat_door.and_then(|door| world.nearest_of_kind(BuildingKind::Bar, door).map(|b| (door, b)));
        // R2: the Bar only within the patrol's reach of the Market.
        route.extend(
            bar.filter(|&(door, b)| {
                patrol_cap.is_none_or(|cap| {
                    world
                        .comp::<Building>(b)
                        .is_some_and(|bd| crate::systems::distance::travel(world, door, bd.door) <= cap)
                })
            })
            .map(|(_, b)| b),
        );
    }
    if hall_here {
        let hall = world.building_of_kind(BuildingKind::Hall);
        // R2: a Homes-only beat measures the Hall from the guard.
        let anchor = beat_door.or(near_cap.map(|(_, f)| f));
        let near = |h: EntityId| match (near_cap, anchor, world.comp::<Building>(h).map(|b| b.door)) {
            (Some((cap, _)), Some(b), Some(hd)) => crate::systems::distance::travel(world, b, hd) <= cap,
            _ => true,
        };
        route.extend(hall.filter(|&h| near(h)));
    }
    let mut homes = if homes_d.is_empty() {
        world.buildings_by_kind().get(&BuildingKind::Home).cloned().unwrap_or_default()
    } else {
        homes_d
    };
    let beat_homes = world.config.law.patrol_beat_homes;
    // The pressure pass R2 (OPEN-215): a district with no Market walks the
    // Homes nearest the guard (`[distance]` on), not its centroid's: the
    // Spire is one label 150-350 tiles across.
    let from = beat_door.or(near_cap.map(|(_, f)| f)).unwrap_or(centroid);
    if homes.len() > beat_homes {
        let mut near: Vec<(u32, EntityId)> = homes
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(from), h)))
            .collect();
        near.sort();
        // R2: within the patrol's reach of the loop's anchor (walked), the
        // nearest always (the Spire's 60 nearest towers spanned 200 tiles).
        let tiles = patrol_cap.map(|c| world.config.exec.walk_tiles(c));
        let first = near.first().map(|&(t, _)| t).unwrap_or(0);
        homes = near
            .into_iter()
            .take(beat_homes)
            .filter(|&(t, _)| tiles.is_none_or(|r| u64::from(t) <= r) || t == first)
            .map(|(_, h)| h)
            .collect();
    }
    if !homes.is_empty() {
        for _ in 0..draws {
            let i = world.rng.world().random_range(0..homes.len());
            route.push(homes[i]);
        }
    }
    route
}

/// M12 D9: is the law cracking down on `gang` anywhere: the posture's
/// Crackdown, or (with district beats) any district's `Crackdown(gang)`?
pub fn cracking_down_on(world: &World, gang: EntityId) -> bool {
    use crate::components::Stance;
    world.law().is_some_and(|l| l.posture == Posture::Crackdown && l.target == Some(gang))
        || (world.config.law.district_beats && world.districts.iter().any(|d| d.stance == Stance::Crackdown(gang)))
}

/// M11 D18: a private guard walks up to five of its corp's contracted
/// clients, nearest its office first (Shadow V4 bug 11: five drawn
/// city-wide when it had more), or with no clients its corp's own
/// buildings nearest its Security Office.
/// `None` for a city guard, and for a guard whose Office no corp owns (none
/// should exist: `corps::move_building` lays them off when it changes hands).
fn private_patrol_route(world: &mut World, guard: EntityId) -> Option<Vec<EntityId>> {
    if !is_private_guard(world, guard) {
        return None;
    }
    let office = world.comp::<Job>(guard).and_then(|j| j.employer)?;
    let from = world.comp::<Building>(office)?.door;
    let corp = world.corp_of_building(office)?;
    let usable = |w: &World, b: EntityId| w.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.door);
    let clients: Vec<EntityId> = world
        .comp::<crate::components::Corp>(corp)
        .map(|c| c.contracts.iter().map(|&(b, _)| b).collect())
        .unwrap_or_default();
    let clients: Vec<EntityId> = clients.into_iter().filter(|&b| usable(world, b).is_some()).collect();
    // Shadow V4 bug 11: the five clients nearest the office, not five drawn
    // city-wide (a 382-min first leg, 88 h of travel a week at 20k).
    let pool: Vec<EntityId> = if clients.is_empty() {
        world.comp::<crate::components::Corp>(corp).map(|c| c.buildings.clone()).unwrap_or_default()
    } else {
        clients
    };
    let mut near: Vec<(u32, EntityId)> =
        pool.into_iter().filter_map(|b| usable(world, b).map(|d| (d.manhattan(from), b))).collect();
    near.sort();
    Some(near.into_iter().take(5).map(|(_, b)| b).collect())
}

/// The turf a Crackdown patrols: the gang's territory, or the six inhabited
/// Homes nearest its Hideout while it holds fewer than three.
pub fn crackdown_turf(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return Vec::new() };
    if g.territory.len() >= 3 {
        return g.territory.clone();
    }
    let Some(door) = world.comp::<Building>(g.hideout).map(|b| b.door) else { return Vec::new() };
    let mut homes: Vec<(u32, EntityId)> = world
        .buildings_by_kind()
        .get(&BuildingKind::Home)
        .map(|v| v.as_slice())
        .unwrap_or(&[])
        .iter()
        .copied()
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && !b.occupants.is_empty())
        .map(|(h, b)| (b.door.manhattan(door), h))
        .collect();
    homes.sort();
    homes.into_iter().take(6).map(|(_, h)| h).collect()
}

/// M12 D11: the turf a district Crackdown patrols: g's held Homes in `d`,
/// or, with fewer than three, the six inhabited Homes of `d` nearest g's
/// Hideout door.
pub fn crackdown_turf_in(world: &World, gang: EntityId, d: crate::components::DistrictId) -> Vec<EntityId> {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return Vec::new() };
    let held: Vec<EntityId> = g.territory.iter().copied().filter(|&h| world.district_of_building(h) == d).collect();
    if held.len() >= 3 {
        return held;
    }
    let door = world.comp::<Building>(g.hideout).map_or(world.district(d).centroid, |b| b.door);
    let mut homes: Vec<(u32, EntityId)> = world
        .district(d)
        .homes
        .iter()
        .copied()
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && !b.occupants.is_empty())
        .map(|(h, b)| (b.door.manhattan(door), h))
        .collect();
    homes.sort();
    homes.into_iter().take(6).map(|(_, h)| h).collect()
}

/// The crackdown loop: `[Market, Home(t), Home(t), Hideout, Home(t)]`. With
/// `district` (M12 D11) the turf is the district's ([`crackdown_turf_in`]),
/// the Market the one nearest the turf's first Home, and the Hideout stop is
/// kept only when it lies in the district; `None` is the M9 loop exactly.
pub fn crackdown_route(
    world: &mut World,
    gang: EntityId,
    district: Option<crate::components::DistrictId>,
) -> Vec<EntityId> {
    let turf = match district {
        Some(d) => crackdown_turf_in(world, gang, d),
        None => crackdown_turf(world, gang),
    };
    let hideout = match district {
        Some(d) => world.hideout_of(gang).filter(|&h| world.district_of_building(h) == d),
        None => world.hideout_of(gang),
    };
    let mut route = Vec::new();
    // The Market nearest the target's Hideout (the only one on the v1 map);
    // in a district, the one nearest the turf's first Home.
    let near = match district {
        Some(_) => turf.first().and_then(|&h| world.comp::<Building>(h)).map(|b| b.door),
        None => hideout.and_then(|h| world.comp::<Building>(h)).map(|b| b.door),
    };
    if let Some(m) = near.and_then(|d| world.nearest_of_kind(BuildingKind::Market, d)) {
        route.push(m);
    }
    let pick = |world: &mut World| {
        if !turf.is_empty() {
            let i = world.rng.world().random_range(0..turf.len());
            Some(turf[i])
        } else {
            None
        }
    };
    route.extend(pick(world));
    route.extend(pick(world));
    route.extend(hideout);
    route.extend(pick(world));
    route
}

/// The action a guard's shift uses today.
pub fn guard_duty_action(world: &World, guard: EntityId) -> ActionKind {
    let key = world.comp::<Job>(guard).map_or(0, |j| j.next_shift_key(world.tick));
    if jail_duty(world, guard, key) {
        ActionKind::GuardJail
    } else {
        ActionKind::PatrolLeg
    }
}
