use super::*;

// ---------------------------------------------------------------------------
// Couriers (§ 3)
// ---------------------------------------------------------------------------

/// A Yard's couriers (its `contract` staff on a `round` duty).
pub(super) fn couriers(world: &World, yard: EntityId) -> usize {
    courier_staff(world, yard).count()
}

fn courier_staff(world: &World, yard: EntityId) -> impl Iterator<Item = EntityId> + '_ {
    world.staff_of(yard).iter().copied().filter(move |&s| {
        world
            .comp::<Job>(s)
            .and_then(|j| crate::systems::trades::row_of(world, j.role))
            .is_some_and(|r| r.model == TradeModel::Contract && r.duty == TradeDuty::Round)
    })
}

/// The pre-view pass phase 3 (OPEN-141): `courier` works his parcels as
/// `haul` trips (a Freight Yard's courier whose row's shape is `haul`).
pub fn haul_courier(world: &World, courier: EntityId) -> bool {
    let Some(j) = world.comp::<Job>(courier) else { return false };
    j.employer.and_then(|y| world.comp::<Building>(y)).is_some_and(|b| b.kind == BuildingKind::FreightYard)
        && crate::systems::trades::on_round(world, courier)
        && crate::systems::shift::shape_of(world, j.role) == crate::config::ShiftShape::Haul
}

/// The pre-view pass phase 3 (OPEN-141): a haul courier's addresses: the
/// distinct buildings his Yard's pending parcels go to, each dealt to one
/// of the Yard's couriers by a hash of the address (stable as parcels are
/// handed over; the round's rank rotation wrapped, so a Statistical
/// colleague's first-hour round took every parcel before a body's first
/// leg arrived). Ascending; none off.
pub fn courier_addresses(world: &World, courier: EntityId) -> Vec<EntityId> {
    let Some(yard) = world.comp::<Job>(courier).and_then(|j| j.employer) else { return Vec::new() };
    // The couriers working today (a rest day's addresses go to the others).
    let staff: Vec<EntityId> = courier_staff(world, yard)
        .filter(|&s| {
            s == courier
                || world
                    .comp::<Job>(s)
                    .is_some_and(|j| crate::exec::routine::workday_of(world, s, j, j.shift_key_at(world.tick)))
        })
        .collect();
    let Some(rank) = staff.iter().position(|&s| s == courier) else { return Vec::new() };
    let n = staff.len() as u64;
    let mut v: Vec<EntityId> = world
        .jobs2
        .parcels
        .iter()
        .filter(|p| p.yard == yard)
        .map(|p| p.at)
        .filter(|at| {
            crate::rng::splitmix64(PURPOSE_PARCEL ^ u64::from(at.index) << 20 ^ u64::from(yard.index)) % n
                == rank as u64
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The pre-view pass phase 3 (OPEN-141): the parcels pending for `b` from
/// `courier`'s Yard.
pub fn parcels_for(world: &World, courier: EntityId, b: EntityId) -> usize {
    let Some(yard) = world.comp::<Job>(courier).and_then(|j| j.employer) else { return 0 };
    world.jobs2.parcels.iter().filter(|p| p.yard == yard && p.at == b).count()
}

/// Midnight: each corp with two or more standing buildings sends
/// `parcels_per_building` a building through the nearest open Freight Yard
/// with couriers and room on their day (`parcels_per_courier` each):
/// `parcel_fee` a parcel to the Yard's owner (`Flow::Handling`); the
/// sender's buildings within `site_reach` of the Yard join its couriers'
/// round.
pub(super) fn parcels(world: &mut World) {
    let cfg = world.config.jobs2.clone();
    world.jobs2.parcel_stops.clear();
    // The pre-view pass phase 3 (OPEN-141): a parcel left from an earlier
    // day (its courier off, or its address on no round) goes back to the
    // Yard at midnight; it lay pending for good before (2k seed 42 x 30:
    // 4,320 sent, 952 handed over, 172 returned).
    if crate::systems::shift::haul_on(world) {
        let today = world.day();
        let before = world.jobs2.parcels.len();
        world.jobs2.parcels.retain(|p| p.day >= today);
        let back = (before - world.jobs2.parcels.len()) as u32;
        if back > 0 {
            ownership::jobs2_row(world).parcels_returned += back;
        }
    }
    let yards: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::FreightYard)
        .iter()
        .copied()
        .filter(|&y| crate::systems::industry::open(world, y))
        .collect();
    let mut room: std::collections::BTreeMap<EntityId, u32> =
        yards.iter().map(|&y| (y, couriers(world, y) as u32 * cfg.parcels_per_courier)).collect();
    if room.values().all(|&r| r == 0) {
        return;
    }
    for c in world.corps() {
        let Some(bs) = world.comp::<Corp>(c).map(|x| x.buildings.clone()) else { continue };
        let standing: Vec<EntityId> = bs
            .into_iter()
            .filter(|&b| world.comp::<Building>(b).is_some_and(|x| !x.demolished && !x.derelict))
            .collect();
        if standing.len() < 2 {
            continue;
        }
        let want = (standing.len() as f32 * cfg.parcels_per_building).round() as u32;
        let from = world.comp::<Building>(standing[0]).map(|b| b.door).unwrap_or_default();
        // The island pass phase 8 (OPEN-168): the Yard with the most of the
        // corp's staffed buildings in travel reach, those its recipients.
        let reach = parcel_reach(world, &standing, &yards, &room);
        let yard = reach.as_ref().map(|(y, _)| *y).or_else(|| {
            yards
                .iter()
                .copied()
                .filter(|y| room.get(y).is_some_and(|&r| r > 0))
                .min_by_key(|&y| (world.comp::<Building>(y).map_or(u32::MAX, |b| b.door.manhattan(from)), y))
        });
        let Some(y) = yard else { break };
        let n = want.min(room[&y]);
        if n == 0 {
            continue;
        }
        *room.get_mut(&y).expect("a yard") -= n;
        let owner = world.owner_of(y);
        let paid = ownership::charge(world, Some(c), owner, i64::from(n) * cfg.parcel_fee, Flow::Handling);
        ownership::credit(world, y, paid);
        let row = ownership::jobs2_row(world);
        row.parcels += n;
        row.parcel_fees += paid;
        let ydoor = world.comp::<Building>(y).map(|b| b.door).unwrap_or_default();
        let near: Vec<EntityId> = match reach {
            Some((_, near)) => near,
            None => standing
                .iter()
                .copied()
                .filter(|&b| world.comp::<Building>(b).is_some_and(|x| x.door.manhattan(ydoor) <= cfg.site_reach))
                .take(6)
                .collect(),
        };
        // Lives at work phase 4 (§ 2.3): the parcels are named, and the
        // round's stops are their recipients' buildings (with staff to sign).
        let stops =
            if crate::systems::service::street_on(world) { name_parcels(world, y, &standing, &near, n) } else { near };
        world.jobs2.parcel_stops.entry(y).or_default().extend(stops);
    }
}

/// The island pass phase 8 (OPEN-168; `[island_trades] parcel_cap_ticks`):
/// the open Yard with room holding the most of `standing`'s staffed
/// buildings within the cap's travel of its door (ties the lower id), and
/// those buildings (ascending, the first `parcel_stops_max`); `None` off or
/// with none in reach of any Yard (the base's pick). The base took the Yard
/// nearest the corp's first building and its buildings within `site_reach`
/// tiles: on the island, where the Yards cluster at the Vats' edge, that
/// left most corps' parcels with no address. No random draw.
fn parcel_reach(
    world: &World,
    standing: &[EntityId],
    yards: &[EntityId],
    room: &std::collections::BTreeMap<EntityId, u32>,
) -> Option<(EntityId, Vec<EntityId>)> {
    let cfg = world.config.island_trades.as_ref()?;
    let staffed: Vec<(EntityId, TilePos)> = standing
        .iter()
        .copied()
        .filter(|&b| !world.staff_of(b).is_empty())
        .filter_map(|b| world.comp::<Building>(b).map(|x| (b, x.door)))
        .collect();
    let mut best: Option<(usize, EntityId, Vec<EntityId>)> = None;
    for &y in yards {
        if !room.get(&y).is_some_and(|&r| r > 0) {
            continue;
        }
        let Some(ydoor) = world.comp::<Building>(y).map(|b| b.door) else { continue };
        // Not the Yard's own door (the Yard, its company floors): no trip.
        let near: Vec<EntityId> = staffed
            .iter()
            .filter(|&&(_, d)| d != ydoor && crate::systems::distance::travel(world, ydoor, d) <= cfg.parcel_cap_ticks)
            .map(|&(b, _)| b)
            .collect();
        // Two at least: a parcel is from one of the corp's buildings to another.
        if near.len() >= 2 && best.as_ref().is_none_or(|(n, _, _)| near.len() > *n) {
            best = Some((near.len(), y, near));
        }
    }
    best.map(|(_, y, mut near)| {
        near.truncate(usize::from(cfg.parcel_stops_max));
        (y, near)
    })
}

/// Lives at work phase 4 (§ 2.3): `n` parcels of one corp through `yard`:
/// each from a sender among `standing` (hashed) to a staff member (hashed)
/// of a recipient among `near` with staff, not the sender (hashed). Returns
/// the recipient buildings (the couriers' stops); none with no recipient.
fn name_parcels(world: &mut World, yard: EntityId, standing: &[EntityId], near: &[EntityId], n: u32) -> Vec<EntityId> {
    let (day, fee) = (world.day(), world.config.jobs2.parcel_fee);
    let to_staff: Vec<EntityId> = near.iter().copied().filter(|&b| !world.staff_of(b).is_empty()).collect();
    let mut stops = Vec::new();
    for k in 0..u64::from(n) {
        let pick = |w: &World, salt: u64, len: usize| {
            let u = hash(w, PURPOSE_PARCEL, u64::from(yard.index) << 20 | day, k * 4 + salt);
            ((u * len as f32) as usize).min(len - 1)
        };
        let from = standing[pick(world, 0, standing.len())];
        let at: Vec<EntityId> = to_staff.iter().copied().filter(|&b| b != from).collect();
        if at.is_empty() {
            continue;
        }
        let at = at[pick(world, 1, at.len())];
        let staff = world.staff_of(at).to_vec();
        let to = staff[pick(world, 2, staff.len())];
        world.jobs2.parcels.push(Parcel { from, to, at, yard, fee, day });
        stops.push(at);
    }
    stops
}

/// Lives at work phase 4 (§ 2.3, the courier's stop): a courier of `yard`
/// at `b` (a body standing in it on his round, `trades::round_plan`; a
/// Statistical courier credited at his shift's first hour, `lod::stat_work`)
/// hands each of the yard's parcels for `b` to its recipient when present
/// (`service::present`), else to a present staff member (hashed, D6): a
/// `Parcel` encounter; with nobody present it stays for the shift's end.
pub fn courier_stop(world: &mut World, courier: EntityId, b: EntityId) {
    if !crate::systems::service::street_on(world) {
        return;
    }
    let Some(yard) = world.comp::<Job>(courier).and_then(|j| j.employer) else { return };
    let mine: Vec<usize> = (0..world.jobs2.parcels.len())
        .filter(|&i| world.jobs2.parcels[i].yard == yard && world.jobs2.parcels[i].at == b)
        .collect();
    let mut done = Vec::new();
    for i in mine {
        let p = world.jobs2.parcels[i];
        let hand = if crate::systems::service::present(world, p.to, b) {
            Some(p.to)
        } else {
            let hour = world.tick / TICKS_PER_HOUR;
            world
                .staff_of(b)
                .iter()
                .copied()
                .filter(|&s| s != courier && crate::systems::service::present(world, s, b))
                .map(|s| (hash(world, PURPOSE_PARCEL, u64::from(p.to.index) ^ hour, u64::from(s.index)).to_bits(), s))
                .min()
                .map(|(_, s)| s)
        };
        let Some(r) = hand else { continue };
        crate::systems::service::serve(world, courier, r, b, crate::components::ServeKind::Parcel, 0);
        ownership::jobs2_row(world).parcels_delivered += 1;
        done.push(i);
    }
    for i in done.into_iter().rev() {
        world.jobs2.parcels.remove(i);
    }
}

/// Lives at work phase 4 (§ 2.3): a courier's shift end (`service::
/// shift_end`, both tiers): the yard's parcels still pending at the stops of
/// his round go back to the Yard (`parcels_returned`).
pub fn parcels_back(world: &mut World, courier: EntityId) {
    if !crate::systems::service::street_on(world) || world.jobs2.parcels.is_empty() {
        return;
    }
    let Some(job) = world.comp::<Job>(courier) else { return };
    let Some(yard) = job.employer else { return };
    if !crate::systems::trades::on_round(world, courier) {
        return;
    }
    // The pre-view pass phase 3 (OPEN-141): a haul courier's own addresses.
    let route = if haul_courier(world, courier) {
        courier_addresses(world, courier)
    } else {
        crate::systems::trades::round_for(world, courier, job.shift_key_at(world.tick))
    };
    let before = world.jobs2.parcels.len();
    world.jobs2.parcels.retain(|p| p.yard != yard || !route.contains(&p.at));
    let back = (before - world.jobs2.parcels.len()) as u32;
    if back > 0 {
        ownership::jobs2_row(world).parcels_returned += back;
    }
}

/// The couriers' round stops at `yard` today (`trades::new_round`).
pub fn parcel_stops(world: &World, yard: EntityId) -> Vec<EntityId> {
    let mut v = world.jobs2.parcel_stops.get(&yard).cloned().unwrap_or_default();
    v.sort();
    v.dedup();
    v
}
