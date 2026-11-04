use super::*;

/// Each asset with a live, non-city owner pays its upkeep (`Flow::AssetUpkeep`
/// to the Treasury) in full or not at all; unpaid: `upkeep_arrears += 1`.
/// Returns the unpaid ids.
pub(super) fn upkeep(world: &mut World) -> Vec<EntityId> {
    let mut unpaid = Vec::new();
    for a in all_assets(world) {
        let Some(owner) = world.comp::<Asset>(a).map(|x| x.owner) else { continue };
        if owner.is_none() || !live_owner(world, owner) {
            continue;
        }
        let cost = upkeep_of(world, a);
        let paid = if cost > 0 && can_pay(world, owner, cost) {
            ownership::charge(world, owner, None, cost, Flow::AssetUpkeep)
        } else {
            0
        };
        let Some(x) = world.comp_mut::<Asset>(a) else { continue };
        if paid >= cost {
            x.upkeep_arrears = 0;
        } else {
            x.upkeep_arrears = x.upkeep_arrears.saturating_add(1);
            unpaid.push(a);
        }
    }
    unpaid
}

/// Each finance plan takes the day's payment and any missed days, capped
/// at what remains (`Flow::Finance`), in full or not at all: short, nothing
/// is taken and `arrears += 1`; paid: arrears 0. A plan at 0 remaining
/// ends. (The catch-up due compounds with the arrears; the Shop's
/// affordability reads the plain `per_day`, D43.)
pub(super) fn finance(world: &mut World) {
    for a in all_assets(world) {
        let Some((owner, f)) = world.comp::<Asset>(a).and_then(|x| x.finance.clone().map(|f| (x.owner, f))) else {
            continue;
        };
        if !live_owner(world, owner) {
            continue;
        }
        let due = (f.per_day * (1 + i64::from(f.arrears))).min(f.remaining).max(0);
        let paid = if owner == f.lender {
            due
        } else if can_pay(world, owner, due) {
            ownership::charge(world, owner, f.lender, due, Flow::Finance)
        } else {
            0
        };
        let Some(x) = world.comp_mut::<Asset>(a) else { continue };
        let Some(fin) = x.finance.as_mut() else { continue };
        fin.remaining -= paid;
        if paid >= due {
            fin.arrears = 0;
        } else {
            fin.arrears = fin.arrears.saturating_add(1);
        }
        if fin.remaining <= 0 {
            x.finance = None;
        }
    }
}

/// Plan D11, D12.
pub(super) fn repossess(world: &mut World) {
    let cfg = world.config.assets.clone();
    for a in all_assets(world) {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        if let Some(f) = x.finance.clone() {
            // Arrears cleared: the lender unbricks.
            if x.kind.is_implant() && x.bricked && f.arrears == 0 {
                if let Some(m) = world.comp_mut::<Asset>(a) {
                    m.bricked = false;
                }
                rekit_touched(world, &[x.loc.holder()]);
                continue;
            }
            let Some(lender) = f.lender.filter(|&l| live_owner(world, Some(l))) else { continue };
            if f.arrears < cfg.repo_days {
                continue;
            }
            match x.kind {
                k if k.is_vehicle() => {
                    // An `InUse` one is towed by `vehicles::end_trip` when it parks.
                    tow(world, a);
                }
                AssetKind::Implant(_) => {
                    if !x.bricked {
                        if let Some(m) = world.comp_mut::<Asset>(a) {
                            m.bricked = true;
                        }
                        rekit_touched(world, &[x.loc.holder()]);
                        repossessed(world, &x, Some(lender), a, "bricked");
                        world.stats.current.repos += 1;
                    } else if u32::from(f.arrears) >= u32::from(cfg.repo_days) + u32::from(cfg.brick_repo_days) {
                        // Dead metal: the lender owns it in place.
                        take_back(world, a, lender);
                        let (owner, what) = (world.owner_label(x.owner), world.name_of(a));
                        let text = format!("{} took {owner}'s {what}", world.owner_label(Some(lender)));
                        let actors = [x.owner.unwrap_or(EntityId::NONE), lender, a];
                        world.push_event(EventKind::Repossessed, &actors, text);
                    }
                }
                _ => {
                    let from = asset_tile(world, a).unwrap_or_default();
                    let seller_kind = match x.kind {
                        AssetKind::Robot => BuildingKind::SecurityOffice,
                        AssetKind::Pack => BuildingKind::Market,
                        _ => BuildingKind::Garage,
                    };
                    let to = nearest_building(world, seller_kind, from, Some(Some(lender)));
                    take_back(world, a, lender);
                    if let Some(b) = to {
                        set_loc(world, a, AssetLoc::Stock(b));
                    }
                    repossessed(world, &x, Some(lender), a, "took");
                    world.stats.current.repos += 1;
                }
            }
        }
    }
    impound(world);
}

/// D11: tow a financed vehicle to its live lender's Garage nearest it,
/// else to the Precinct (then sold as an impound, the proceeds to the
/// lender). Not while it is driven. The caller has checked the arrears.
pub fn tow(world: &mut World, a: EntityId) -> bool {
    let Some(x) = world.comp::<Asset>(a).cloned() else { return false };
    let Some(lender) = x.finance.as_ref().and_then(|f| f.lender).filter(|&l| live_owner(world, Some(l))) else {
        return false;
    };
    if !x.kind.is_vehicle() || matches!(x.loc, AssetLoc::InUse(_)) {
        return false;
    }
    let from = asset_tile(world, a).unwrap_or_default();
    let to = nearest_building(world, BuildingKind::Garage, from, Some(Some(lender)))
        .or_else(|| world.building_of_kind(BuildingKind::Jail));
    reset_title(world, a);
    take_back(world, a, lender);
    if let Some(b) = to {
        set_loc(world, a, AssetLoc::Stock(b));
    }
    repossessed(world, &x, Some(lender), a, "towed");
    world.stats.current.repos += 1;
    true
}

/// The lender becomes the owner; the plan and the arrears end.
pub(super) fn take_back(world: &mut World, a: EntityId, lender: EntityId) {
    set_owner(world, a, Some(lender));
    if let Some(m) = world.comp_mut::<Asset>(a) {
        m.finance = None;
        m.upkeep_arrears = 0;
    }
}

/// The `Repossessed` event, memory and counter for a tow, brick or take.
pub(super) fn repossessed(world: &mut World, before: &Asset, lender: Option<EntityId>, a: EntityId, how: &str) {
    let owner = before.owner;
    let what = world.name_of(a);
    let (o, l) = (world.owner_label(owner), world.owner_label(lender));
    let text = match how {
        "bricked" => format!("{l} bricked {o}'s {what}"),
        "towed" => format!("{l} towed {o}'s {what}"),
        "impounded" => format!("the city impounded {o}'s {what}"),
        _ => format!("{l} took {o}'s {what}"),
    };
    let actors = [owner.unwrap_or(EntityId::NONE), lender.unwrap_or(EntityId::NONE), a];
    world.push_event(EventKind::Repossessed, &actors, text);
    if let Some(agent) = is_agent(world, owner) {
        let subject = lender.filter(|&x| world.has::<Identity>(x));
        world.remember(agent, MemoryKind::Repossessed, subject, 0.6, -0.6, false);
    }
}

/// Plan D12: vehicles a non-city owner left unregistered `impound_days`
/// go to the Precinct; every vehicle in Precinct stock is offered to the
/// nearest Garage whose owner can pay `impound_frac × value`.
pub(super) fn impound(world: &mut World) {
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return };
    let days = world.config.assets.impound_days;
    if world.levers.impound {
        for a in world.vehicles.clone() {
            let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
            if x.owner.is_none()
                || x.upkeep_arrears < days
                || matches!(x.loc, AssetLoc::InUse(_) | AssetLoc::Stock(_) | AssetLoc::Limbo(_))
            {
                continue;
            }
            reset_title(world, a);
            set_owner(world, a, None);
            set_loc(world, a, AssetLoc::Stock(precinct));
            repossessed(world, &x, None, a, "impounded");
            world.stats.current.impounds += 1;
        }
    }
    // The impound sale: the proceeds to the vehicle's owner (the city, or a
    // lender whose tow found no Garage of its own).
    let door = world.comp::<Building>(precinct).map_or_else(TilePos::default, |b| b.door);
    let frac = world.config.assets.impound_frac;
    for a in assets_at(world, precinct).to_vec() {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        if !x.kind.is_vehicle() || x.loc != AssetLoc::Stock(precinct) {
            continue;
        }
        let price = (frac * x.value as f32).round() as i64;
        let mut garages: Vec<(u32, EntityId)> = world
            .buildings_of_kind(BuildingKind::Garage)
            .iter()
            .filter_map(|&g| {
                world.comp::<Building>(g).filter(|b| !b.demolished && !b.derelict).map(|b| (b.door.manhattan(door), g))
            })
            .collect();
        garages.sort_unstable();
        let Some(g) = garages.into_iter().map(|(_, g)| g).find(|&g| {
            let o = world.owner_of(g);
            o != x.owner && world.purse(o) >= price
        }) else {
            continue;
        };
        let buyer = world.owner_of(g);
        ownership::pay(world, buyer, x.owner, price, Flow::Asset);
        reset_title(world, a);
        set_owner(world, a, buyer);
        set_loc(world, a, AssetLoc::Stock(g));
        let (what, who) = (world.name_of(a), world.owner_label(buyer));
        world.push_event(
            EventKind::AssetBought,
            &[buyer.unwrap_or(EntityId::NONE), g, a],
            format!("{who} bought the impounded {what} for {price}"),
        );
    }
}

/// Plan D9 step 4: `condition −= wear` (× `unpaid_wear_mult` unpaid), value
/// from condition; at 0 a vehicle or robot is wrecked, an implant fails.
/// Stock and limbo do not wear.
pub(super) fn wear(world: &mut World, unpaid: &[EntityId]) {
    let mult = world.config.assets.unpaid_wear_mult.max(1);
    let mut gone = Vec::new();
    for a in all_assets(world) {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        let w = *world.config.assets.wear.get(x.kind);
        let idle = matches!(x.loc, AssetLoc::Stock(_) | AssetLoc::Limbo(_));
        let mut was = x.condition;
        if w > 0 && !idle && x.condition > 0 {
            let w = if unpaid.binary_search(&a).is_ok() { w.saturating_mul(mult) } else { w };
            let c = x.condition.saturating_sub(w);
            if let Some(m) = world.comp_mut::<Asset>(a) {
                m.condition = c;
            }
            if c == 0 {
                gone.push(a);
            }
            was = c;
        }
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.value = m.list * i64::from(was) / 100;
        }
    }
    for a in gone {
        wreck(world, a, "worn out");
    }
}

/// A vehicle, robot, pack or bridge is wrecked: `Wrecked` event, half its
/// `parts_per` into the stock of the building it stood at (a driver's or
/// carrier's nearest), despawned. An implant fails instead: condition 0,
/// sanity − `fail_shock`, never despawned.
pub fn wreck(world: &mut World, a: EntityId, why: &str) {
    let Some(x) = world.comp::<Asset>(a).cloned() else { return };
    if x.kind.is_implant() {
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.condition = 0;
            m.value = 0;
        }
        if let AssetLoc::Installed(body) = x.loc {
            let shock = world.config.assets.fail_shock;
            if let Some(b) = world.comp_mut::<Body>(body) {
                b.sanity = (b.sanity - shock).clamp(0.0, 1.0);
            }
        }
        rekit_touched(world, &[x.loc.holder()]);
        let (what, owner) = (world.name_of(a), world.owner_label(x.owner));
        world.push_event(
            EventKind::Wrecked,
            &[a, x.owner.unwrap_or(EntityId::NONE)],
            format!("{owner}'s {what} failed ({why})"),
        );
        return;
    }
    let at = match x.loc {
        AssetLoc::Parked(b) | AssetLoc::Posted(b) | AssetLoc::Stock(b) => Some(b),
        AssetLoc::InUse(_) | AssetLoc::Carried(_) | AssetLoc::Installed(_) => {
            asset_tile(world, a).and_then(|t| nearest_any_building(world, t))
        }
        AssetLoc::Limbo(_) => None,
    };
    let parts = *world.config.assets.parts_per.get(x.kind) / 2;
    if let Some(b) = at {
        world.add_stock(b, Good::Parts, parts);
    }
    let (what, owner) = (world.name_of(a), world.owner_label(x.owner));
    world.push_event(
        EventKind::Wrecked,
        &[a, x.owner.unwrap_or(EntityId::NONE)],
        format!("{owner}'s {what} was wrecked ({why})"),
    );
    despawn(world, a);
}

/// Owners of vehicles below `repair_below` buy the points back to 100 from
/// the nearest Garage at `repair_price × level` a point (`Flow::Asset`),
/// one Part per 25 points from its stock (a missing Part costs the Garage
/// `part_credit` as `Import`). An agent or gang pays only in full.
/// Phase 5: robots too, at the nearest Security Office (the seller; one
/// repairing its own robot pays only the missing Parts): with repairs for
/// vehicles alone every robot wore out around day 101.
pub(super) fn repairs(world: &mut World) {
    let cfg = world.config.assets.clone();
    let robots: Vec<EntityId> = all_assets(world)
        .into_iter()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Robot))
        .collect();
    for a in world.vehicles.clone().into_iter().chain(robots) {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        if x.condition >= cfg.repair_below
            || x.condition == 0
            || x.owner.is_none()
            || !live_owner(world, x.owner)
            || matches!(x.loc, AssetLoc::Stock(_) | AssetLoc::Limbo(_))
        {
            continue;
        }
        let Some(from) = asset_tile(world, a) else { continue };
        let shop = if x.kind == AssetKind::Robot { BuildingKind::SecurityOffice } else { BuildingKind::Garage };
        let Some(g) = nearest_building(world, shop, from, None) else { continue };
        let garage_owner = world.owner_of(g);
        let points = 100 - i64::from(x.condition);
        let cost = if garage_owner == x.owner {
            0
        } else {
            (cfg.repair_price as f32 * seller_level(world, g) * points as f32).round() as i64
        };
        // Jobs & lives phase 2 (§ 3): an agent's vehicle near a working
        // tinker is fixed on the street at a discount, or wrecked.
        if let Some(o) = x.owner.filter(|_| cost > 0 && x.kind != AssetKind::Robot) {
            // A botched repair wrecks the vehicle there (`assets::wreck`).
            if let Some(fixed) = crate::systems::streetwork::tinker_repair(world, a, o, from, cost) {
                if fixed {
                    if let Some(m) = world.comp_mut::<Asset>(a) {
                        m.condition = 100;
                        m.value = m.list;
                    }
                }
                continue;
            }
        }
        if !can_pay(world, x.owner, cost) {
            continue;
        }
        if cost > 0 {
            let paid = ownership::charge(world, x.owner, garage_owner, cost, Flow::Asset);
            ownership::credit(world, g, paid);
        }
        let need = (points as u32).div_ceil(25);
        let have = world.take_stock(g, Good::Parts, need);
        let missing = i64::from(need - have);
        if missing > 0 {
            ownership::import(world, garage_owner, missing * cfg.part_credit, ownership::ImportWhy::Repair);
            // L2 L9: the Fab trigger's tally.
            crate::systems::jobs::note_import(world, garage_owner, missing * cfg.part_credit);
        }
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.condition = 100;
            m.value = m.list;
        }
    }
}
