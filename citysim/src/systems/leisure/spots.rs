use super::*;

// ---------------------------------------------------------------------------
// Spots (plan L15, L16)
// ---------------------------------------------------------------------------

/// Plan L15, midnight (and once lazily): each district's spots, ≤ 24: the
/// street outside the doors of its Bars, Markets, Clubs and NoodleBars, each
/// gang's Hideout door, and in a Sump district `barrels_per_sump` fire
/// barrels on street tiles within 6 of a Home door (`splitmix64`).
pub fn spots_daily(world: &mut World) {
    let n = world.districts.len();
    let mut all: Vec<Vec<Spot>> = vec![Vec::new(); n];
    for kind in [BuildingKind::Bar, BuildingKind::Market, BuildingKind::Club, BuildingKind::NoodleBar] {
        for &b in world.buildings_of_kind(kind) {
            if !standing(world, b) {
                continue;
            }
            let Some(bd) = world.comp::<Building>(b) else { continue };
            let d = world.district_of_building(b).index();
            if let Some(v) = all.get_mut(d) {
                v.push(Spot { tile: world.outside_door(bd), kind: SpotKind::Door(b) });
            }
        }
    }
    for &g in world.gang_list() {
        let Some(h) = world.hideout_of(g) else { continue };
        let Some(bd) = world.comp::<Building>(h).filter(|bd| !bd.demolished) else { continue };
        let d = world.district_of_building(h).index();
        if let Some(v) = all.get_mut(d) {
            v.push(Spot { tile: world.outside_door(bd), kind: SpotKind::Hideout(g) });
        }
    }
    let w = world.map.w();
    let k_max = world.config.leisure.barrels_per_sump as u64;
    for (i, d) in world.districts.iter().enumerate() {
        if d.zone != Zone::Sump || k_max == 0 {
            continue;
        }
        let doors: Vec<TilePos> = d
            .homes
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| b.door))
            .collect();
        let near: Vec<TilePos> = d
            .streets
            .iter()
            .map(|&ix| TilePos { x: (ix as usize % w) as u16, y: (ix as usize / w) as u16 })
            .filter(|&t| doors.iter().any(|&dd| dd.manhattan(t) <= BARREL_REACH))
            .collect();
        if near.is_empty() {
            continue;
        }
        let mut taken: Vec<TilePos> = Vec::new();
        for k in 0..k_max {
            let ix = (splitmix64(world.seed() ^ (i as u64).rotate_left(17) ^ (k + 1).rotate_left(41))
                % near.len() as u64) as usize;
            let t = near[ix];
            if !taken.contains(&t) {
                taken.push(t);
            }
        }
        if let Some(v) = all.get_mut(i) {
            v.extend(taken.into_iter().map(|t| Spot { tile: t, kind: SpotKind::Barrel }));
        }
    }
    for v in &mut all {
        v.sort_by_key(|s| (s.tile, s.kind));
        v.dedup_by_key(|s| s.tile);
        v.truncate(MAX_SPOTS);
    }
    world.spots = all;
}

/// May `id` hang out at `s` (a Hideout's door is its members' only)?
fn spot_ok(world: &World, id: EntityId, s: &Spot) -> bool {
    // L2 shadow fixes item 6: not the street just fled.
    if crate::systems::fixes::avoided(world, id, s.tile) {
        return false;
    }
    // Pressure pass round 6, V2 (TURF § 2): after dark a contested cell's
    // street is passed over (its own sides aside).
    if !matches!(s.kind, SpotKind::Home(_)) && crate::systems::turf::avoids(world, id, s.tile) {
        return false;
    }
    match s.kind {
        SpotKind::Hideout(g) => world.gang_of(id) == Some(g),
        _ => true,
    }
}

/// The spots of `id`'s district (its tile's), and of its Home's; its own
/// Home (Lives at work phase 2) and, for the scored pick, its friends'.
pub(super) fn spots_near(world: &World, id: EntityId, from: TilePos) -> Vec<Spot> {
    spots_near_with(world, id, from, false)
}

fn spots_near_with(world: &World, id: EntityId, from: TilePos, with_friends: bool) -> Vec<Spot> {
    let mut ds: SmallVec<[DistrictId; 2]> = SmallVec::new();
    ds.push(world.district_of(from));
    if let Some(d) = world.comp::<Household>(id).and_then(|h| h.home).map(|h| world.district_of_building(h)) {
        if !ds.contains(&d) {
            ds.push(d);
        }
    }
    let mut out: Vec<Spot> = Vec::new();
    for d in ds {
        if let Some(v) = world.spots.get(d.index()) {
            out.extend(v.iter().copied().filter(|s| spot_ok(world, id, s)));
        }
    }
    // Lives at work phase 2 (§ 3.1, § 3.2): the agent's own Home and the
    // Homes of friends due home, within the walk cap (nothing off).
    let cap = world.config.life.spot_walk_cap_tiles;
    for (h, _) in crate::systems::household::home_spots(world, id, from, cap, with_friends) {
        if let Some(bd) = world.comp::<Building>(h) {
            let tile = world.outside_door(bd);
            if !out.iter().any(|s| s.tile == tile) {
                out.push(Spot { tile, kind: SpotKind::Home(h) });
            }
        }
    }
    out
}

/// The nearest spot `id` may use (think time), with a 0 social score.
pub(super) fn nearest_spot(world: &World, id: EntityId, from: TilePos) -> Option<(TilePos, f32)> {
    // The island pass phase 2: by the travel estimate, none past the cap
    // (the own Home's never is).
    if crate::systems::distance::free(world).is_some() {
        let cap = spot_cap(world);
        let own = home_of(world, id);
        return spots_near(world, id, from)
            .into_iter()
            .map(|s| (spot_dist(world, id, from, s.tile), s))
            .filter(|(d, s)| *d <= cap || matches!(s.kind, SpotKind::Home(h) if Some(h) == own))
            .map(|(d, s)| (d, s.tile))
            .min()
            .map(|(_, t)| (t, 0.0));
    }
    spots_near(world, id, from).into_iter().map(|s| (s.tile.manhattan(from), s.tile)).min().map(|(_, t)| (t, 0.0))
}

/// Plan L16: `id`'s top contacts (≤ 6): kin and Friends first, then
/// gang-mates, by affinity.
pub(super) fn contacts(world: &World, id: EntityId) -> Vec<(EntityId, f32, bool)> {
    let gang = world.gang_of(id);
    let mut v: Vec<(u8, f32, EntityId, bool)> = world
        .neighbours(id)
        .filter(|&c| world.has::<Brain>(c) && !world.has::<Sentence>(c))
        .filter_map(|c| {
            let e = world.edge(id, c)?;
            let kin = matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent);
            let rank = match e.kind {
                RelKind::Spouse | RelKind::Family | RelKind::Parent => 0,
                RelKind::Friend => 1,
                _ if gang.is_some() && world.gang_of(c) == gang => 2,
                _ => return None,
            };
            (e.affinity > 0.0).then_some((rank, e.affinity, c, kin))
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
    v.truncate(MAX_CONTACTS);
    v.into_iter().map(|(_, a, c, kin)| (c, a, kin)).collect()
}

pub(super) fn pair(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Plan L16: the best spot for `id` and its social score: `Σ affinity ×
/// P(contact there within the hour)` (`hunt::habit` at `now + 60`: its
/// building's outside door 1.0, its district 0.5; present now +1.0), a kin
/// term `kin_w × min(7, days since they met) × P`, a Call's `call_w` on the
/// own Hideout's spot, times the walk; ties the nearer, then the lower tile.
pub fn best_spot(world: &World, id: EntityId, from: TilePos) -> Option<(TilePos, f32)> {
    best_spot_kind(world, id, from).map(|(s, social)| (s.tile, social))
}

/// [`best_spot`] with the spot's kind (a Home spot's HangOut runs inside it).
pub fn best_spot_kind(world: &World, id: EntityId, from: TilePos) -> Option<(Spot, f32)> {
    spot_pick(world, id, from).0
}

/// [`best_spot_kind`], and whether there was no spot within the cap
/// (`[distance.free]` on: the stay-in's condition, island fix round F3).
pub(super) fn spot_pick(world: &World, id: EntityId, from: TilePos) -> (Option<(Spot, f32)>, bool) {
    let free_on = crate::systems::distance::free(world).is_some();
    let mut spots = spots_near_with(world, id, from, true);
    if spots.is_empty() {
        return (None, free_on);
    }
    // L2 shadow fixes item 5: a spot past `spot_walk_cap_tiles` is not worth
    // the walk (195 min at midnight to an empty corner): within the cap,
    // else the nearest alone.
    let cap = spot_cap(world);
    // Lives at work phase 2 (§ 3.1): the own Home is never past the cap.
    let own = home_of(world, id);
    // Lives at work phase 2 (§ 3.3): after an empty leave, only spots with
    // company now (none: no HangOut).
    // The stay-in's test (island fix round F3) is the travel alone, over
    // every spot before that filter: none within the cap.
    let none_within = |all: &[Spot]| free_on && !all.iter().any(|s| spot_dist(world, id, from, s.tile) <= cap);
    let mut before: Option<Vec<Spot>> = None;
    if crate::systems::household::company_only(world, id) {
        before = Some(spots.clone());
        spots.retain(|s| {
            let home = match s.kind {
                SpotKind::Home(h) => Some(h),
                _ => None,
            };
            crate::systems::household::company_at(world, id, s.tile, home)
        });
        if spots.is_empty() {
            return (None, before.as_deref().is_some_and(none_within));
        }
    }
    // The island pass phase 2 (`[distance.free]`): the cap is on the travel
    // estimate, and with none within there is no spot (an hour's company is
    // not walked 4 h for; island d30: 2,204 Unwind spot legs of 4 h or more).
    let dist: Vec<(Spot, u64)> = spots.iter().map(|&s| (s, spot_dist(world, id, from, s.tile))).collect();
    let near: Vec<(Spot, u64)> = dist
        .iter()
        .copied()
        .filter(|&(s, d)| d <= cap || matches!(s.kind, SpotKind::Home(h) if Some(h) == own))
        .collect();
    let spots: Vec<(Spot, u64)> = if !near.is_empty() {
        near
    } else if free_on {
        return (None, before.as_deref().is_none_or(none_within));
    } else {
        dist.iter().copied().min_by_key(|&(s, d)| (d, s.tile)).into_iter().collect()
    };
    let enemies = crate::systems::fixes::enemy_tiles(world, id);
    let now = world.tick;
    let today = world.day();
    let cs = contacts(world, id);
    let habits: Vec<(EntityId, f32, bool, Option<TilePos>, DistrictId)> = cs
        .iter()
        .map(|&(c, a, kin)| {
            let i = crate::systems::hunt::habit(world, c, now + TICKS_PER_HOUR);
            let door = i.building.and_then(|b| world.comp::<Building>(b)).map(|bd| world.outside_door(bd));
            (c, a, kin, door, world.district_of(i.tile))
        })
        .collect();
    let gang = world.gang_of(id);
    let call = gang.and_then(|g| world.comp::<Gang>(g)).and_then(|g| g.call_until).is_some_and(|t| t > now);
    let cfg = &world.config.leisure;
    let best = spots
        .iter()
        .map(|(s, d)| {
            let sd = world.district_of(s.tile);
            let present = world.hangouts.get(&s.tile);
            let mut social = 0.0f32;
            for &(c, a, kin, door, d) in &habits {
                let mut p = if door == Some(s.tile) {
                    1.0
                } else if d == sd {
                    0.5
                } else {
                    0.0
                };
                if present.is_some_and(|v| v.contains(&c)) {
                    p += 1.0;
                }
                social += a * p;
                if kin && p > 0.0 {
                    let days = world.last_met.get(&pair(id, c)).map_or(7, |&d| today.saturating_sub(d)).min(7);
                    social += cfg.kin_w * days as f32 * p.min(1.0);
                }
            }
            if call
                && matches!(s.kind, SpotKind::Hideout(g) if Some(g) == gang)
                && s.tile.manhattan(from) <= cfg.call_tiles
            {
                social += cfg.call_w;
            }
            // L2 shadow fixes item 5: the hour's company on the corner now
            // (strangers too, a little), less a strong term per Enemy near
            // it (a runner went back 18 times to the street where six beat
            // him).
            let others = present.map_or(0, |v| v.iter().filter(|&&o| o != id).count());
            social += (0.1 * others as f32).min(0.5);
            social -= cfg_life_penalty(world) * crate::systems::fixes::enemies_near(&enemies, s.tile) as f32;
            // Lives at work phase 2 (§ 3.1, § 3.3, § 3.4): the household term
            // of a Home spot, the empty-spot memory, the carer's away mult
            // (each 1 or 0 off).
            let own_home = matches!(s.kind, SpotKind::Home(h) if Some(h) == home_of(world, id));
            if let SpotKind::Home(h) = s.kind {
                social += crate::systems::household::home_term(world, id, h);
            }
            let mult = crate::systems::household::empty_mult(world, id, s.tile)
                * crate::systems::household::away_mult(world, id, own_home);
            ((0.5 + social).max(0.0) * spot_travel(world, id, from, s.tile) * mult, *d, *s, social)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.tile.cmp(&a.2.tile)))
        .map(|(_, _, s, social)| (s, social));
    (best, false)
}
