use super::*;

/// A working vendor short of a batch buys `vendor_batch` shelf units at
/// the nearest stocked Market (the real unit price, `Flow::Wholesale`),
/// `synth_stretch` meals each, as far as his wallet goes.
pub(super) fn restock(world: &mut World) {
    let batch = world.config.jobs2.vendor_batch;
    let stretch = crate::systems::diet::stretch(world);
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        if kind_of_pitch(world, &p) != Some(PitchKind::Vendor) || p.stock >= batch * stretch / 2 || !working(world, &p)
        {
            continue;
        }
        let market = world
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .copied()
            .filter(|&m| world.comp::<Building>(m).is_some_and(|b| !b.demolished && b.stock_food > 0))
            .filter(|&m| !world.is_closed(m))
            .min_by_key(|&m| (world.comp::<Building>(m).map_or(u32::MAX, |b| b.door.manhattan(p.spot)), m));
        let Some(m) = market else { continue };
        let price = world.comp::<crate::components::Market>(m).map_or(0, |x| x.price_food).max(1);
        let stock = world.comp::<Building>(m).map_or(0, |b| b.stock_food);
        let units = batch.min(stock).min((coins(world, o).max(0) / price).min(i64::from(u32::MAX)) as u32);
        if units == 0 {
            continue;
        }
        let owner = world.owner_of(m);
        let paid = ownership::pay(world, Some(o), owner, i64::from(units) * price, Flow::Wholesale);
        ownership::credit(world, m, paid);
        if let Some(b) = world.comp_mut::<Building>(m) {
            b.stock_food -= units;
        }
        if let Some(x) = world.jobs2.pitches.get_mut(&o) {
            x.stock += units * stretch;
        }
    }
}

/// `lod::stat_eat`'s Market leg: an off-screen synth buyer takes a meal
/// from an open vendor's stall in his district on `p_vendor_pick` (a pure
/// hash of the hour and the buyer) at `vendor_markup` × the synth meal's
/// price, paid to the vendor (`Flow::Food`). `true` when he ate there.
pub fn stat_vendor_meal(world: &mut World, id: EntityId, market: EntityId) -> bool {
    if !on(world) || world.jobs2.pitches.is_empty() {
        return false;
    }
    let (q, synth) = crate::systems::diet::offer(world, id, market);
    if q != crate::systems::diet::Quality::Synth {
        return false;
    }
    let cfg = &world.config.jobs2;
    if hash(world, PURPOSE_PICK, u64::from(id.index), world.tick / TICKS_PER_HOUR) >= cfg.p_vendor_pick {
        return false;
    }
    let price = ((synth as f32 * cfg.vendor_markup).ceil() as i64).max(1);
    if coins(world, id) < price {
        return false;
    }
    let Some(d) = district_of_agent(world, id) else { return false };
    let vendor = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.district == d && p.owner != id && p.stock > 0)
        .filter(|p| kind_of_pitch(world, p) == Some(PitchKind::Vendor) && working(world, p))
        .map(|p| p.owner)
        .min_by_key(|o| (hash(world, PURPOSE_PICK, u64::from(o.index), u64::from(id.index)).to_bits(), *o));
    let Some(v) = vendor else { return false };
    let paid = ownership::pay(world, Some(id), Some(v), price, Flow::Food);
    if paid <= 0 {
        return false;
    }
    if let Some(p) = world.jobs2.pitches.get_mut(&v) {
        p.stock -= 1;
        p.take_today += paid;
    }
    let cfg = world.config.needs.clone();
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    crate::systems::health::ate(world, id, crate::components::MealKind::Synth);
    world.mark_day(id, crate::components::trace_flags::ATE);
    let row = ownership::jobs2_row(world);
    row.vendor_meals += 1;
    row.vendor_take += paid;
    // Lives at work phase 4b (§ 2.3): the vendor's regulars (the stall is a tile).
    if crate::systems::service::street_on(world) {
        crate::systems::service::encounter(world, v, id, None, d, crate::components::ServeKind::Buyer, paid);
    }
    true
}

/// The pressure pass phase 2 (OPEN-176, `[customers]`): a working
/// vendor's stall sells, each hour, up to `vendor_per_hour` meals to the
/// hungry bodies passing it (Full or Coarse, out on the street within
/// `vendor_tiles`, hunger under `vendor_hunger`, the price in hand; the
/// lowest hash of the hour first), at the off-screen buyer's price
/// (`vendor_markup` × the nearest Market's synth meal): the meal eaten
/// there, the coins to the vendor, the Buyer encounter on both sides. The
/// off-screen buyer is `stat_vendor_meal`.
pub(super) fn vendor_passers(world: &mut World) {
    let Some(c) = crate::systems::customers::cfg(world).cloned() else { return };
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    let hour = world.tick / TICKS_PER_HOUR;
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        if kind_of_pitch(world, &p) != Some(PitchKind::Vendor) || p.stock == 0 || !working(world, &p) {
            continue;
        }
        let market = world
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .copied()
            .filter(|&m| world.comp::<Building>(m).is_some_and(|b| !b.demolished))
            .min_by_key(|&m| (world.comp::<Building>(m).map_or(u32::MAX, |b| b.door.manhattan(p.spot)), m));
        let Some(m) = market else { continue };
        let mut buyers: Vec<(u32, EntityId)> = world
            .agents_near(p.spot, c.vendor_tiles, None)
            .into_iter()
            .filter(|&id| {
                id != o
                    && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
                    && world.comp::<Position>(id).is_some_and(|x| x.building.is_none())
                    && crate::systems::demography::is_adult(world, id)
                    && !world.has::<Sentence>(id)
                    && crate::systems::law::living(world, id)
                    && world.comp::<crate::components::Needs>(id).is_some_and(|n| n.hunger < c.vendor_hunger)
            })
            .map(|id| (hash(world, PURPOSE_PICK, u64::from(id.index) ^ u64::from(o.index), hour).to_bits(), id))
            .collect();
        buyers.sort_unstable();
        let mut sold = 0u8;
        for (_, id) in buyers {
            if sold >= c.vendor_per_hour {
                break;
            }
            if world.jobs2.pitches.get(&o).is_none_or(|x| x.stock == 0) {
                break;
            }
            let (_, synth) = crate::systems::diet::offer(world, id, m);
            let price = ((synth as f32 * world.config.jobs2.vendor_markup).ceil() as i64).max(1);
            if coins(world, id) < price {
                continue;
            }
            let paid = ownership::pay(world, Some(id), Some(o), price, Flow::Food);
            if paid <= 0 {
                continue;
            }
            if let Some(x) = world.jobs2.pitches.get_mut(&o) {
                x.stock -= 1;
                x.take_today += paid;
            }
            let cfg = world.config.needs.clone();
            if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                crate::needs::eat(n, &cfg);
            }
            crate::systems::health::ate(world, id, crate::components::MealKind::Synth);
            world.mark_day(id, crate::components::trace_flags::ATE);
            world.remember(id, crate::components::MemoryKind::Ate, None, 0.2, 0.3, false);
            let row = ownership::jobs2_row(world);
            row.vendor_meals += 1;
            row.vendor_take += paid;
            row.vendor_bodies += 1;
            crate::systems::service::encounter(
                world,
                o,
                id,
                None,
                p.district,
                crate::components::ServeKind::Buyer,
                paid,
            );
            sold += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// The labour corner (§ 3)
// ---------------------------------------------------------------------------

/// The corners: per district with an open Construction or Freight Yard,
/// the lowest-id one's door (the street outside it), ascending by district.
pub fn corners(world: &World) -> Vec<(DistrictId, TilePos)> {
    if world.config.island_trades.as_ref().is_some_and(|c| c.corner_per_yard) {
        return yard_corners(world);
    }
    let mut out: Vec<(DistrictId, EntityId)> = Vec::new();
    for kind in [BuildingKind::ConstructionYard, BuildingKind::FreightYard] {
        for &y in world.buildings_of_kind(kind) {
            if !crate::systems::industry::open(world, y) {
                continue;
            }
            let d = world.district_of_building(y);
            match out.iter_mut().find(|(x, _)| *x == d) {
                Some((_, b)) if y < *b => *b = y,
                Some(_) => {}
                None => out.push((d, y)),
            }
        }
    }
    out.sort();
    out.into_iter().filter_map(|(d, y)| world.comp::<Building>(y).map(|b| (d, world.outside_door(b)))).collect()
}

/// The island pass phase 8 (`[island_trades] corner_per_yard`; OPEN-168):
/// a corner outside every open Construction and Freight Yard, in id order,
/// one within `[jobs2] corner_tiles` of a corner already stood sharing it;
/// ascending by (district, tile). The base's one a district stood the
/// island's Vats corner 380 tiles from its Construction Yards, so their day
/// posts drew on a queue of five.
fn yard_corners(world: &World) -> Vec<(DistrictId, TilePos)> {
    let reach = world.config.jobs2.corner_tiles;
    let mut yards: Vec<EntityId> = [BuildingKind::ConstructionYard, BuildingKind::FreightYard]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|&y| crate::systems::industry::open(world, y))
        .collect();
    yards.sort_unstable();
    let mut out: Vec<(DistrictId, TilePos)> = Vec::new();
    for y in yards {
        let Some(b) = world.comp::<Building>(y) else { continue };
        let t = world.outside_door(b);
        if out.iter().any(|&(_, c)| c.manhattan(t) <= reach) {
            continue;
        }
        out.push((world.district_of_building(y), t));
    }
    out.sort();
    out
}

/// The island pass phase 8 (`[island_trades] corner_stat_ticks`;
/// OPEN-168): past `corner_stat_reach` tiles of every corner, the corner
/// nearest `from` by travel (`distance::travel`, the El where it wins)
/// within the cap, ties the lower index; `None` off. The island's Yards sit
/// at the Vats' edge, and the Statistical broke within 160 tiles of them
/// queued five a dawn.
pub(super) fn corner_by_travel(world: &World, corners: &[(DistrictId, TilePos)], from: TilePos) -> Option<usize> {
    let cap = world.config.island_trades.as_ref()?.corner_stat_ticks;
    corners
        .iter()
        .enumerate()
        .map(|(i, &(_, t))| (crate::systems::distance::travel(world, from, t), i))
        .filter(|&(d, _)| d <= cap)
        .min()
        .map(|(_, i)| i)
}

/// The nearest corner to `from` within `reach` tiles.
pub(super) fn corner_near(corners: &[(DistrictId, TilePos)], from: TilePos, reach: u32) -> Option<usize> {
    corners
        .iter()
        .enumerate()
        .map(|(i, &(_, t))| (t.manhattan(from), i))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, i)| i)
}

/// One who may stand at the corner: a free jobless adult, broke, lawful enough.
pub fn corner_ok(world: &World, id: EntityId) -> bool {
    let cfg = &world.config.jobs2;
    free_adult(world, id)
        && !world.has::<Job>(id)
        && world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= cfg.corner_lawful)
        // Round 4 T5 (OPEN-217): a searcher who takes worse work stands
        // there whatever his purse.
        && (coins(world, id) < cfg.corner_broke || crate::systems::search::at_shape_up(world, id))
}

/// The day's posts by employer: a Construction Yard's `day_per_site` a
/// site, a Freight Yard that handled `backlog_units` yesterday its
/// `yard_posts`, each Farm in a harvest season its `harvest_posts`; an
/// owner posts only what its purse pays a day.
pub(super) fn day_posts(world: &World, wage: i64) -> Vec<EntityId> {
    let cfg = &world.config.jobs2;
    let mut out: Vec<EntityId> = Vec::new();
    for y in world.buildings_of_kind(BuildingKind::ConstructionYard).to_vec() {
        if crate::systems::industry::open(world, y) {
            let n = usize::from(cfg.day_per_site) * crate::systems::industry::sites_of(world, y).len();
            out.extend(std::iter::repeat_n(y, n));
        }
    }
    for y in world.buildings_of_kind(BuildingKind::FreightYard).to_vec() {
        let yesterday = world.jobs2.yard_units.get(&y).map_or(0, |v| v.1);
        if crate::systems::industry::open(world, y) && yesterday >= cfg.backlog_units {
            out.extend(std::iter::repeat_n(y, usize::from(cfg.yard_posts)));
        }
    }
    // The pressure pass phase 2 (OPEN-176): every open Yard's foreman
    // takes `shape_up_posts` hands at the dawn shape-up, sites or none
    // (the corner's posts dried up with the sites: two picks in a week).
    if let Some(n) = crate::systems::customers::cfg(world).map(|c| usize::from(c.shape_up_posts)) {
        for k in [BuildingKind::ConstructionYard, BuildingKind::FreightYard] {
            for y in world.buildings_of_kind(k).to_vec() {
                if crate::systems::industry::open(world, y) {
                    out.extend(std::iter::repeat_n(y, n));
                }
            }
        }
    }
    if cfg.harvest_seasons.contains(&world.season()) {
        for f in world.buildings_of_kind(BuildingKind::Farm).to_vec() {
            if world.comp::<Building>(f).is_some_and(|b| !b.demolished && !b.derelict) {
                out.extend(std::iter::repeat_n(f, usize::from(cfg.harvest_posts)));
            }
        }
    }
    // An owner posts what its purse pays.
    let mut spent: std::collections::BTreeMap<Option<EntityId>, i64> = Default::default();
    out.retain(|&b| {
        let o = world.owner_of(b);
        let s = spent.entry(o).or_insert(0);
        *s += wage;
        world.purse(o) >= *s
    });
    out
}
