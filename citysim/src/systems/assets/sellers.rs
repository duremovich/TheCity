use super::*;

/// Spec section 6: a vehicle parked in a Garage its owner does not own pays
/// `garage_rent` a day to the Garage's owner (`Flow::Rent`), in full or not
/// at all.
pub(super) fn garage_rent(world: &mut World) {
    let rent = world.config.shop.garage_rent;
    if rent <= 0 {
        return;
    }
    for g in world.buildings_of_kind(BuildingKind::Garage).to_vec() {
        let landlord = world.owner_of(g);
        for a in assets_at(world, g).to_vec() {
            let Some(owner) = world.comp::<Asset>(a).filter(|x| x.loc == AssetLoc::Parked(g)).map(|x| x.owner) else {
                continue;
            };
            if owner.is_none() || owner == landlord || !live_owner(world, owner) || !can_pay(world, owner, rent) {
                continue;
            }
            let paid = ownership::charge(world, owner, landlord, rent, Flow::Rent);
            ownership::credit(world, g, paid);
        }
    }
}

/// D18: last in `World::new`, a seller on the vacant Lot nearest each row's
/// district centroid (Manhattan from the Lot door, ties lower id). Phase 2
/// rows: a Garage in the Civic owned by the Tech corp, and one in Sump
/// Central owned by the jobless adult (not an exec nor a building owner)
/// whose Home door is nearest the Lot's (ties lower id), dealt `[shop]
/// seed_owner_coins`, its founding cooldown started. Phase 3 rows: a
/// Clinic in the Spire (the Tech corp's) and back-alley docs in Sump West
/// and Mid East (agents', as Sump Central's Garage).
/// Scale S4: the rows by role (`districts::Seat`, the shipped names on the
/// shipped map), dealt `World::seed_copies` times (copy `j`'s corp rows go to
/// the `j`-th Tech corp, wrapped).
/// Returns the Lots built, in row order.
pub fn seed_sellers(world: &mut World) -> Vec<EntityId> {
    use crate::components::Zone;
    use crate::systems::districts::Seat;
    let mut built = Vec::new();
    if !world.config.assets.enabled {
        return built;
    }
    let techs: Vec<EntityId> = world
        .corps()
        .into_iter()
        .filter(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&crate::components::Niche::Tech)))
        .collect();
    let rows: [(Seat, bool, BuildingKind); 5] = [
        (Seat::new(Zone::Civic, 0.0), true, BuildingKind::Garage),
        (Seat::new(Zone::Sump, 0.5), false, BuildingKind::Garage),
        (Seat::new(Zone::Spire, 0.0), true, BuildingKind::Clinic),
        (Seat::new(Zone::Sump, 0.0), false, BuildingKind::Clinic),
        (Seat::new(Zone::Mid, 0.75), false, BuildingKind::Clinic),
    ];
    let copies = world.lot_copies();
    for copy in 0..copies {
        let tech = if techs.is_empty() { None } else { Some(techs[copy % techs.len()]) };
        for &(seat, corp_owned, kind) in &rows {
            if let Some(lot) = seed_seller(world, seat, copy, copies, corp_owned, kind, tech) {
                built.push(lot);
            }
        }
    }
    built
}

/// One `seed_sellers` row's copy: the Lot built, if any.
fn seed_seller(
    world: &mut World,
    seat: crate::systems::districts::Seat,
    copy: usize,
    copies: usize,
    corp_owned: bool,
    kind: BuildingKind,
    tech: Option<EntityId>,
) -> Option<EntityId> {
    {
        let d = crate::systems::districts::seat_district(world, seat, copy, copies)?;
        let (district, centroid) = world.districts.get(d.index()).map(|x| (x.name.clone(), x.centroid))?;
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter(|&l| world.district_of_building(l) == d)
            .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(centroid), l)))
            .min()
            .map(|(_, l)| l);
        let lot = lot?;
        let lot_door = world.comp::<Building>(lot).map(|b| b.door).unwrap_or_default();
        let owner = if corp_owned {
            tech
        } else {
            let owners: std::collections::BTreeSet<EntityId> = world
                .with::<Building>()
                .into_iter()
                .filter_map(|b| world.comp::<Building>(b).and_then(|bd| bd.owner))
                .collect();
            world
                // scan-ok: once, at seed
                .citizens()
                .into_iter()
                .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a) && !owners.contains(&a))
                .filter(|&a| crate::systems::demography::is_adult(world, a))
                .filter(|&a| !crate::systems::founding::is_exec(world, a))
                .filter_map(|a| {
                    let home = world.comp::<crate::components::Household>(a).and_then(|h| h.home)?;
                    Some((world.comp::<Building>(home)?.door.manhattan(lot_door), a))
                })
                .min()
                .map(|(_, a)| a)
        };
        crate::systems::founding::build_on_lot(world, lot, kind, owner).ok()?;
        if !corp_owned {
            let coins = world.config.shop.seed_owner_coins;
            if let Some(w) = owner.and_then(|o| world.comp_mut::<Wallet>(o)) {
                w.coins = coins;
            }
            // The seller is their registration: the founding cooldown runs
            // from today (else the seed coins founded a Bar on day 0, the
            // pair incorporated and went bankrupt, and the city foreclosed
            // the Garage within a fortnight).
            let today = world.day();
            if let Some(b) = owner.and_then(|o| world.comp_mut::<Brain>(o)) {
                b.last_found_day = Some(today);
            }
        }
        let (what, who) = (world.name_of(lot), world.owner_label(owner));
        let rect = world.comp::<Building>(lot).map(|b| b.rect);
        let actors: Vec<EntityId> = owner.into_iter().chain([lot]).collect();
        world.push_event(
            EventKind::Founded,
            &actors,
            format!("{who} opened {what} in {district} (seeded; Lot {} {rect:?})", lot.index),
        );
        Some(lot)
    }
}
