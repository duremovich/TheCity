use super::*;

// ---------------------------------------------------------------------------
// The daily pass (plan 2.4)
// ---------------------------------------------------------------------------

/// A Home's rent today: the city's lever, a corp's `level × base[tier]`, an
/// agent landlord's `base[tier]`; every one under the rent cap.
pub fn rent_for(world: &World, home: EntityId) -> i64 {
    let Some(b) = world.comp::<Building>(home) else { return 0 };
    // M12 D25: a derelict is let to nobody.
    if b.demolished || b.derelict {
        return 0;
    }
    // The island pass phase 3a (I6): a company bed's rent, per worker.
    if world.company_homes.contains_key(&home) {
        return (crate::systems::company::rent_tenths(world, home) + 5) / 10;
    }
    let tier = usize::from(b.tier.min(2));
    let base = world.config.rent.base[tier];
    let rent = match owner_kind(world, b.owner) {
        OwnerKind::City => world.levers.city_rent[tier],
        OwnerKind::Corp(c) => {
            let level = world.comp::<Corp>(c).map_or(1.0, |c| c.level(Niche::Housing));
            (level * base as f32).round() as i64
        }
        OwnerKind::Agent(_) => base,
        // M18 P5h: a gang's protection rides on the rent (`[housing] gang_protection`).
        OwnerKind::Gang(_) if world.config.housing.enabled => {
            (base as f32 * (1.0 + world.config.housing.gang_protection.max(0.0))).round() as i64
        }
        OwnerKind::Gang(_) => base,
    };
    let rent = rent.max(0);
    world.levers.rent_cap.map_or(rent, |cap| rent.min(cap.max(0)))
}

/// A Home's rent in tenths of a coin: what accrues (`rent_due` is
/// fractional). With `[economy] price_tenths` a corp's Housing level keeps
/// its fraction (Squeeze to 1.2 on a Mid Block is 2.4 a day, an Undercut to
/// 0.9 is 1.8, where `rent_for` rounds both back to 2); otherwise `rent_for`
/// x 10.
pub fn rent_tenths_for(world: &World, home: EntityId) -> i64 {
    // The island pass phase 3a (I6): a company bed's rent, per worker.
    if world.company_homes.contains_key(&home) {
        return crate::systems::company::rent_tenths(world, home);
    }
    let whole = rent_for(world, home) * 10;
    if !world.config.economy.price_tenths {
        return whole;
    }
    let Some(b) = world.comp::<Building>(home).filter(|b| !b.demolished && !b.derelict) else { return 0 };
    let OwnerKind::Corp(c) = owner_kind(world, b.owner) else { return whole };
    let base = world.config.rent.base[usize::from(b.tier.min(2))];
    let level = world.comp::<Corp>(c).map_or(1.0, |c| c.level(Niche::Housing));
    let t = ((level * base as f32 * 10.0).round() as i64).max(0);
    world.levers.rent_cap.map_or(t, |cap| t.min(cap.max(0) * 10))
}

/// D7: the owner's eviction threshold.
fn evict_days(world: &World, owner: Option<EntityId>) -> u8 {
    owner
        .and_then(|o| world.comp::<Corp>(o))
        .and_then(|c| c.evict_days_override)
        .unwrap_or(world.config.rent.evict_days)
}

/// Settle the rent already due, then accrue today's share: a coin that falls
/// due at midnight has the whole next day (and its wage or dole, when
/// `[rent] pay_from_income`) to be paid before it counts as arrears. `rents`
/// are in tenths (`rent_tenths_for`). A Home whose rent is 0 forgives what
/// its residents owe (a rent cap or `SetCityRent` to 0 left arrears that
/// never decayed); an owner living in their own Home pays nobody.
pub(super) fn collect_rent(world: &mut World, rents: &[(EntityId, i64)]) {
    for &(home, rent) in rents {
        // The island pass phase 3a (I6): a company block's rent is owed per
        // worker on the staff (and in the grace), not split per adult.
        if world.company_homes.contains_key(&home) {
            collect_company_rent(world, home, rent);
            continue;
        }
        let owner = world.owner_of(home);
        // Lives at work phase 3 (§ 2.2, tied housing): a resident whose
        // tied trade's employer belongs to this Home's owner is freed as the
        // owner living in their own Home is.
        let tied: Vec<EntityId> =
            world.residents_of(home).iter().copied().filter(|&a| tied_here(world, a, home)).collect();
        if rent <= 0 || owner.is_some_and(|o| world.residents_of(home).contains(&o)) || !tied.is_empty() {
            let free: Vec<EntityId> = world
                .residents_of(home)
                .iter()
                .copied()
                .filter(|&a| rent <= 0 || Some(a) == owner || tied.contains(&a))
                .collect();
            for a in free {
                if let Some(h) = world.comp_mut::<Household>(a) {
                    h.rent_due = 0.0;
                    h.arrears = 0;
                }
            }
            if rent <= 0 {
                continue;
            }
        }
        // The paying adults: an owner living in their own Home is not one,
        // so the Home still yields its whole rent from the others.
        let adults: Vec<EntityId> = world
            .residents_of(home)
            .iter()
            .copied()
            .filter(|&a| Some(a) != owner && !tied.contains(&a))
            .filter(|&a| world.has::<Brain>(a) && crate::systems::demography::is_adult(world, a))
            .collect();
        if adults.is_empty() {
            continue;
        }
        let share = rent as f32 / 10.0 / adults.len() as f32;
        for a in adults {
            settle_rent(world, a, home, true);
            if let Some(h) = world.comp_mut::<Household>(a) {
                h.rent_due += share;
            }
        }
    }
}

/// The island pass phase 3a (I6): company block `home`'s rent, `rent`
/// tenths a day per resident who owes it (`company::payer`); the rest of
/// the household owes nothing. Settled first as `collect_rent` settles.
fn collect_company_rent(world: &mut World, home: EntityId, rent: i64) {
    let residents: Vec<EntityId> = world.residents_of(home).to_vec();
    for a in residents {
        if !world.has::<Brain>(a) {
            continue;
        }
        if rent <= 0 || !crate::systems::company::payer(world, a, home) {
            if let Some(h) = world.comp_mut::<Household>(a) {
                h.rent_due = 0.0;
                h.arrears = 0;
            }
            continue;
        }
        settle_rent(world, a, home, true);
        if let Some(h) = world.comp_mut::<Household>(a) {
            h.rent_due += rent as f32 / 10.0;
        }
    }
}

/// Lives at work phase 3 (§ 2.2, tied housing): is `a` a tied trade's
/// worker (`[[trades]] tied_housing`) housed in a Home of his employer's
/// owner? Rent-free there (`collect_rent`, `settle_rent`). Civic only.
pub fn tied_here(world: &World, a: EntityId, home: EntityId) -> bool {
    if !crate::systems::service::civic_on(world) {
        return false;
    }
    let Some(job) = world.comp::<Job>(a) else { return false };
    let tied = crate::systems::trades::row_of(world, job.role).is_some_and(|r| r.tied_housing);
    let employer_owner = job.employer.and_then(|e| world.owner_of(e));
    tied && employer_owner.is_some() && employer_owner == world.owner_of(home)
}

/// Pay the whole coins of rent due from the wallet. At midnight
/// (`midnight`), a short payment is a day of arrears (`RentShort` on the
/// first); a full one clears them.
pub(crate) fn settle_rent(world: &mut World, a: EntityId, home: EntityId, midnight: bool) {
    let Some(due) = world.comp::<Household>(a).map(|h| h.rent_due.floor() as i64) else { return };
    if due < 1 {
        // Not a whole coin owed: not in arrears.
        if midnight {
            if let Some(h) = world.comp_mut::<Household>(a) {
                h.arrears = 0;
            }
        }
        return;
    }
    // An owner housed in their own Home (an heir, a founder) owes nobody;
    // nor does a tied worker in his employer's owner's Block (phase 3).
    if world.owner_of(home) == Some(a) || tied_here(world, a, home) {
        if let Some(h) = world.comp_mut::<Household>(a) {
            h.rent_due = 0.0;
            h.arrears = 0;
        }
        return;
    }
    let owner = world.owner_of(home);
    let mut paid = pay(world, Some(a), owner, due, Flow::Rent);
    // The transition wave (phase 3a, `[economy2] household_rent`, the market wage law
    // only): a co-resident spouse covers a short share from their own
    // wallet before it counts as arrears (the household's purse, not a
    // transfer from outside it).
    if paid < due && midnight && household_rent(world) {
        if let Some(s) = world
            .spouse_of(a)
            .filter(|&s| !world.has::<Corpse>(s) && world.comp::<Household>(s).and_then(|h| h.home) == Some(home))
        {
            paid += pay(world, Some(s), owner, due - paid, Flow::Rent);
        }
    }
    credit(world, home, paid);
    world.stats.current.rent_paid += paid;
    let today = world.day();
    let Some(h) = world.comp_mut::<Household>(a) else { return };
    h.rent_due -= paid as f32;
    h.note_rent_paid(today, paid);
    if paid >= due {
        h.arrears = 0;
        return;
    }
    // Inside the Precinct the debt runs on but the clock stops: an evicted
    // prisoner's spouse went with them, and the jailed were most evictions.
    if !midnight || world.has::<Sentence>(a) {
        return;
    }
    let Some(h) = world.comp_mut::<Household>(a) else { return };
    h.arrears = h.arrears.saturating_add(1);
    let first = h.arrears == 1;
    world.stats.current.rent_short += 1;
    let subject = owner.filter(|&o| world.has::<Identity>(o));
    world.remember(a, MemoryKind::RentShort, subject, 0.4, -0.4, false);
    if first {
        let name = world.name_of(a);
        let label = world.owner_label(owner);
        world.push_event(
            EventKind::RentShort,
            &[a, home],
            format!("{name} could not pay the rent on {} to {label} ({paid} of {due})", world.name_of(home)),
        );
    }
}

/// The transition wave (phase 3a): a co-resident spouse covers a short
/// rent share with `[economy2] household_rent` under the market wage law (P10: was `no_net`).
fn household_rent(world: &World) -> bool {
    world.config.economy2.household_rent && crate::systems::econ::wages_on(world)
}

/// `[rent] pay_from_income`: rent comes out of a wage or the dole the moment
/// it is collected, before it can go on food (`economy::collect_wage`,
/// `collect_dole`).
///
/// Violence fix 4 (`[life] vf_rent_meal`): an agent in arrears keeps a
/// meal's worth (`World::mean_price`) of its purse; the rest pays. Arrears
/// built over unpaid first days swallowed a new guard's whole wage before
/// food, and the guard starved housed (seed 43, year 3).
pub fn pay_rent_from_income(world: &mut World, agent: EntityId) {
    // The island pass phase 3a (I6): a company bed's rent is docked from
    // the wage whatever `pay_from_income` says.
    if !world.config.rent.pay_from_income && !crate::systems::company::housed(world, agent) {
        return;
    }
    let Some(home) = world.comp::<Household>(agent).and_then(|h| h.home) else { return };
    let in_arrears = world.comp::<Household>(agent).is_some_and(|h| h.arrears > 0);
    let keep = if in_arrears && crate::systems::fixes::rent_meal(world) {
        world.mean_price().max(0).min(world.comp::<crate::components::Wallet>(agent).map_or(0, |w| w.coins.max(0)))
    } else {
        0
    };
    // The meal is held aside while the rent settles (no flow: the coins
    // never leave the purse).
    if keep > 0 {
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(agent) {
            w.coins -= keep;
        }
    }
    settle_rent(world, agent, home, false);
    if keep > 0 {
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(agent) {
            w.coins += keep;
        }
    }
}

pub(super) fn evictions(world: &mut World) {
    let no_city = world.levers.no_city_evictions;
    let mut due: Vec<EntityId> = Vec::new();
    for home in world.buildings_of_kind(BuildingKind::Home).to_vec() {
        let owner = world.owner_of(home);
        if owner.is_none() && no_city {
            continue;
        }
        let limit = evict_days(world, owner);
        due.extend(
            world
                .residents_of(home)
                .iter()
                .copied()
                .filter(|&a| world.has::<Brain>(a) && world.comp::<Household>(a).is_some_and(|h| h.arrears >= limit)),
        );
    }
    for a in due {
        // A spouse already put out with their partner is no longer housed.
        if world.comp::<Household>(a).is_some_and(|h| h.home.is_some()) {
            evict(world, a, "arrears");
        }
    }
    let horizon = world.tick.saturating_sub(EVICTION_LOG_DAYS * TICKS_PER_DAY);
    while world.eviction_log.front().is_some_and(|&t| t < horizon) {
        world.eviction_log.pop_front();
    }
}

/// Put someone out of their Home where they stand.
fn put_out(world: &mut World, who: EntityId, home: EntityId, outside: crate::components::TilePos) {
    world.set_home(who, None);
    if world.comp::<Position>(who).is_some_and(|p| p.building == Some(home)) {
        if world.has::<Brain>(who) {
            world.abort_plan(who);
        }
        world.remove_from_building(who);
        let tick = world.tick;
        if let Some(p) = world.comp_mut::<Position>(who) {
            p.tile = outside;
            p.building = None;
            p.floor = 0;
            p.entered = tick;
        }
    }
}

/// D8: evict `agent` (and a spouse in the same Home, and the children of the
/// evicted whose other parent is not staying).
pub fn evict(world: &mut World, agent: EntityId, reason: &str) {
    let Some(home) = world.comp::<Household>(agent).and_then(|h| h.home) else { return };
    let Some(outside) = world.comp::<Building>(home).map(|b| world.outside_door(b)) else { return };
    let owner = world.owner_of(home);
    let tick = world.tick;
    let mut adults = vec![agent];
    if let Some(s) = world.spouse_of(agent) {
        if world.comp::<Household>(s).is_some_and(|h| h.home == Some(home)) && world.has::<Brain>(s) {
            adults.push(s);
        }
    }
    let staying: Vec<EntityId> =
        world.residents_of(home).iter().copied().filter(|r| !adults.contains(r) && !world.has::<Child>(*r)).collect();
    let children: Vec<EntityId> = world
        .residents_of(home)
        .iter()
        .copied()
        .filter(|&c| world.has::<Child>(c))
        .filter(|&c| adults.iter().any(|&a| crate::systems::demography::is_child_of(world, c, a)))
        .filter(|&c| !staying.iter().any(|&p| crate::systems::demography::is_child_of(world, c, p)))
        .collect();
    let label = world.owner_label(owner);
    let place = world.name_of(home);
    for &a in &adults {
        put_out(world, a, home, outside);
        if let Some(h) = world.comp_mut::<Household>(a) {
            h.arrears = 0;
            h.rent_due = 0.0;
            h.evicted_by = Some((owner, tick));
        }
        // M15 W5: with the word on a corp landlord is the memory's subject too.
        let subject = owner.filter(|&o| world.has::<Identity>(o) || world.has::<Corp>(o));
        world.remember(a, MemoryKind::Evicted, subject, 0.8, -0.8, false);
        let d = world.district_of_building(home);
        crate::systems::gossip::post_deed(world, d, crate::word::Deed::Evicted, subject, Some(a));
        if let OwnerKind::Agent(o) = owner_kind(world, owner) {
            crate::systems::social::adjust(world, a, o, -0.3, 0.0);
        }
        let name = world.name_of(a);
        let mut actors = vec![a, home];
        if let OwnerKind::Agent(o) = owner_kind(world, owner) {
            actors.push(o);
        }
        world.push_event(EventKind::Evicted, &actors, format!("{name} was evicted from {place} by {label} ({reason})"));
        world.eviction_log.push_back(tick);
        crate::systems::districts::note_eviction(world, home, owner);
        world.stats.current.evictions += 1;
    }
    for c in children {
        put_out(world, c, home, outside);
    }
    // M12 D17: belongings on the street at the Block door.
    crate::systems::litter::deposit(world, outside, 24, 1);
}

/// 8d (OPEN-126, Shadow V5 bug 17): the mover pays `days` of the Home's
/// rent up front to its owner (`Flow::Rent`, credited as `settle_rent`
/// credits a payment) and holds it as prepaid rent (`rent_due` below 0, so
/// the first nights accrue against it). No coin made or lost.
pub(crate) fn move_in_deposit(world: &mut World, a: EntityId, home: EntityId, days: i64) {
    let due = days * rent_for(world, home);
    if due <= 0 {
        return;
    }
    let owner = world.owner_of(home);
    let paid = if owner == Some(a) { 0 } else { pay(world, Some(a), owner, due, Flow::Rent) };
    if paid <= 0 {
        return;
    }
    credit(world, home, paid);
    world.stats.current.rent_paid += paid;
    let today = world.day();
    if let Some(h) = world.comp_mut::<Household>(a) {
        h.rent_due -= paid as f32;
        h.note_rent_paid(today, paid);
    }
}
