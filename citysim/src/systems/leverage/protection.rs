use super::*;

// ---------------------------------------------------------------------------
// Standing coercions: the weekly coins
// ---------------------------------------------------------------------------

/// Does the protection's gang still hold the building's district, and does
/// the building still have the owner it was levied on?
fn protection_holds(world: &World, c: &Coercion) -> bool {
    let (Some(g), Some(b)) = (c.holder, c.building) else { return false };
    if !world.has::<crate::components::Gang>(g) {
        return false;
    }
    let d = world.district_of_building(b);
    let held = if crate::systems::protection::on(world) {
        crate::systems::protection::ground(world, g).binary_search(&d).is_ok()
    } else {
        crate::systems::gang::held_districts(world, g).iter().any(|&(x, _)| x == d)
    };
    held && payer_of(world, b) == c.payer
}

/// Whose purse a business's protection comes out of: its owner, or for a
/// gang front on the city's deed (L2), the gang it fronts for.
pub fn payer_of(world: &World, b: EntityId) -> Option<EntityId> {
    let bd = world.comp::<Building>(b)?;
    bd.owner.or_else(|| bd.venue.as_ref().and_then(|v| v.front_of).filter(|&g| world.has::<crate::components::Gang>(g)))
}

/// Daily: every standing weekly coercion due pays, or ends: a payer short
/// refuses (`Refused`); a protection whose gang lost the district, or
/// whose building changed hands, a blackmail or tribute whose holder is
/// gone, or a coerced party jailed or dead, is `Broken`.
pub(super) fn collect(world: &mut World) {
    let now = world.tick;
    let due: Vec<CoercionId> = world
        .coercions
        .values()
        .filter(|c| c.is_active() && c.next_due <= now && matches!(c.demand, Demand::Coins { weekly: true, .. }))
        .map(|c| c.id)
        .collect();
    for id in due {
        // The pre-view pass phase 6 (P13): a protection whose gang's leader
        // has a body is collected on his round, in person; midnight only
        // ends a hold that is gone.
        let in_person = world.coercions.get(&id).is_some_and(|c| {
            c.why == CoercionWhy::Protection && c.holder.is_some_and(|g| crate::systems::leaders::collects(world, g))
        });
        if in_person {
            if broken_week(world, id) {
                close(world, id, CoercionStatus::Broken, "the hold is gone");
            }
            continue;
        }
        pay_week(world, id);
    }
}

/// The pre-view pass phase 6 (P13): one owed protection week collected in
/// person at the leader's round stop (the owner met there): paid, or
/// ended as at midnight. The coins paid (0: refused or broken).
pub fn collect_in_person(world: &mut World, id: CoercionId) -> i64 {
    pay_week(world, id)
}

/// A standing coercion whose hold is gone (see [`pay_week`]).
fn broken_week(world: &World, id: CoercionId) -> bool {
    let Some(c) = world.coercions.get(&id) else { return false };
    let gone = |e: EntityId| !law::living(world, e) || world.has::<Sentence>(e);
    match c.why {
        // The pressure pass phase 3: the hold is the gang's; the leaner
        // jailed or dead does not end it.
        CoercionWhy::Protection if crate::systems::protection::on(world) => !protection_holds(world, c) || gone(c.over),
        CoercionWhy::Protection => !protection_holds(world, c) || gone(c.over) || gone(c.agent),
        CoercionWhy::Parley => {
            c.holder.is_none_or(|h| !world.has::<crate::components::Gang>(h))
                || c.payer.is_none_or(|p| !world.has::<crate::components::Gang>(p))
        }
        _ => c.holder.is_none_or(|h| holder_gone(world, h)) || !law::living(world, c.over),
    }
}

/// A holder gone: a gang or a corp that no longer exists, else a person
/// not living (review finding 1: a faction holder is no person).
fn holder_gone(world: &World, h: EntityId) -> bool {
    if world.has::<crate::components::Gang>(h) || world.has::<crate::components::Corp>(h) {
        return false;
    }
    !law::living(world, h)
}

/// One standing coercion's week: paid, or ended (see [`collect`]). The
/// coins paid.
pub(super) fn pay_week(world: &mut World, id: CoercionId) -> i64 {
    let now = world.tick;
    let Some(c) = world.coercions.get(&id).cloned() else { return 0 };
    let Demand::Coins { amount, .. } = c.demand else { return 0 };
    if !c.is_active() {
        return 0;
    }
    if broken_week(world, id) {
        close(world, id, CoercionStatus::Broken, "the hold is gone");
        return 0;
    }
    let payer = c.payer.unwrap_or(c.over);
    if world.purse(Some(payer)) < amount || c.refusing {
        // The pressure pass phase 3: a protection week the till cannot meet
        // (or the owner will not) is a miss (punished at `punish_misses`),
        // tried again tomorrow.
        if c.why == CoercionWhy::Protection && crate::systems::protection::on(world) {
            if let Some(x) = world.coercions.get_mut(&id) {
                x.next_due = x.next_due.max(now) + TICKS_PER_DAY;
            }
            crate::systems::protection::miss(world, id);
            return 0;
        }
        close(world, id, CoercionStatus::Refused, "would not pay");
        return 0;
    }
    let paid = ownership::pay(world, Some(payer), c.holder, amount, Flow::Coerced);
    if let Some(b) = c.building {
        // M11 D20: a corp owner books it as a loss (its brain may Secure).
        ownership::note_loss(world, b, paid, Some(c.agent));
    }
    if let Some(x) = world.coercions.get_mut(&id) {
        x.paid = x.paid.saturating_add(1);
        x.next_due = now + 7 * TICKS_PER_DAY;
    }
    if c.why == CoercionWhy::Protection {
        let row = &mut world.stats.current.leverage;
        row.protection_paid += 1;
        row.protection_coins += paid;
        // The pressure pass phase 3: the protection joins the week's take
        // the leader's Collect shares out to the members.
        if crate::systems::protection::on(world) {
            if let Some(g) = c.holder.filter(|&g| world.has::<crate::components::Gang>(g)) {
                crate::systems::leisure::note_tribute(world, g, paid);
            }
            if let Some(x) = world.coercions.get_mut(&id) {
                x.missed = 0;
            }
        }
    }
    paid
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

/// The kinds a gang levies protection on (spec § 4, L2's venues included).
pub const PROTECTED: [BuildingKind; 11] = [
    BuildingKind::Bar,
    BuildingKind::Market,
    BuildingKind::Hotel,
    BuildingKind::Clinic,
    BuildingKind::Fixer,
    BuildingKind::Club,
    BuildingKind::Arcade,
    BuildingKind::NoodleBar,
    BuildingKind::FightPit,
    BuildingKind::Den,
    BuildingKind::Lounge,
];

/// The week's revenue a building took (the last seven days and today).
fn week_revenue(b: &Building) -> i64 {
    b.revenue.iter().sum::<i64>() + b.revenue_today
}

/// Who answers for a business: an agent owner itself, a corp's exec, a
/// gang's leader (a rival's front); living and free.
pub(crate) fn answers_for(world: &World, owner: EntityId) -> Option<EntityId> {
    let who = match ownership::owner_kind(world, Some(owner)) {
        OwnerKind::Agent(a) => Some(a),
        OwnerKind::Corp(c) => world.comp::<crate::components::Corp>(c).and_then(|x| x.exec),
        OwnerKind::Gang(g) => world.comp::<crate::components::Gang>(g).and_then(|x| x.leader),
        OwnerKind::City => None,
    }?;
    (law::living(world, who) && !world.has::<Sentence>(who)).then_some(who)
}

/// The business a gang would levy in district `d`: the highest week's
/// revenue (ties the lower id) of the protected kinds owned by an agent, a
/// corp with no Security cover there, or another gang; not derelict or
/// closed, not paying anyone already, not tried by this gang lately, and
/// taking at least the weekly levy.
pub fn protection_target(world: &World, gang: EntityId, d: crate::components::DistrictId) -> Option<EntityId> {
    // Round 6 V2 (TURF § 2): who pays whom; a business pays one gang unless
    // its cell is contested, where both sides squeeze it.
    let mut paying: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
    for c in world.coercions.values().filter(|c| c.is_active() && c.why == CoercionWhy::Protection) {
        if let Some(b) = c.building {
            paying.entry(b).or_default().extend(c.holder);
        }
    }
    let turf = crate::systems::turf::on(world);
    let weekly = world.config.leverage.protection_weekly;
    let mut best: Option<(i64, EntityId)> = None;
    for kind in PROTECTED {
        for &b in world.buildings_of_kind(kind) {
            if world.district_of_building(b) != d {
                continue;
            }
            if let Some(to) = paying.get(&b) {
                // A second gang squeezes it only in a cell they contest.
                let squeeze = turf && !to.contains(&gang) && crate::systems::turf::contested_by(world, gang, b);
                if !squeeze {
                    continue;
                }
            }
            // Round 6 V2: the holder of its cell (or a side of its contest).
            if !crate::systems::turf::leans(world, gang, b) {
                continue;
            }
            let Some(bd) = world.comp::<Building>(b) else { continue };
            if bd.demolished || bd.derelict || world.is_closed(b) {
                continue;
            }
            let Some(owner) = payer_of(world, b).filter(|&o| o != gang) else { continue };
            let ok = match ownership::owner_kind(world, Some(owner)) {
                OwnerKind::Agent(_) | OwnerKind::Gang(_) => true,
                OwnerKind::Corp(_) => bd.secured_by.is_none(),
                OwnerKind::City => false,
            };
            // Never a front of the same gang.
            let own_front = bd.venue.as_ref().is_some_and(|v| v.front_of == Some(gang));
            if !ok || own_front {
                continue;
            }
            let rev = week_revenue(bd);
            // The pressure pass phase 3: a levy sized to the business.
            let short = if crate::systems::protection::on(world) {
                crate::systems::protection::levy(world, rev, weekly).is_none()
            } else {
                rev < weekly
            };
            if short || tried_recently(world, gang, b, tried::PROTECT) {
                continue;
            }
            if best.is_none_or(|(r, x)| rev > r || (rev == r && b < x)) {
                best = Some((rev, b));
            }
        }
    }
    best.map(|(_, b)| b)
}

/// Daily, each gang on its own weekday (`gang index % 7`): under Expand or
/// Contest, its leader leans on one business per held district (see
/// [`protection_target`]) for `protection_weekly` a week. Paid at once and
/// weekly after ([`collect`]).
pub(super) fn protection_pass(world: &mut World) {
    let day = world.day();
    for (i, g) in world.gangs().into_iter().enumerate() {
        if day % 7 != i as u64 % 7 {
            continue;
        }
        let Some((leader, order)) =
            world.comp::<crate::components::Gang>(g).and_then(|x| x.leader.map(|l| (l, x.order)))
        else {
            continue;
        };
        // The pressure pass phase 3 (OPEN-162): keyed to held ground, not
        // the day's order (only a gang lying low leans on nobody).
        let ground_on = crate::systems::protection::on(world);
        let order_ok = if ground_on {
            order != crate::components::Order::LieLow
        } else {
            matches!(order, crate::components::Order::Expand | crate::components::Order::Contest)
        };
        if !order_ok || !free(world, leader) {
            continue;
        }
        let districts: Vec<_> = if ground_on {
            crate::systems::protection::ground(world, g)
        } else {
            crate::systems::gang::held_districts(world, g).into_iter().map(|(d, _)| d).collect()
        };
        let per = crate::systems::protection::cfg(world).map_or(1, |c| c.lean_per_district.max(1));
        for d in districts {
            for _ in 0..per {
                if !room(world) {
                    return;
                }
                let Some(b) = protection_target(world, g, d) else { break };
                try_protection(world, g, leader, b);
            }
        }
    }
}

/// One protection move: `leader` of `gang` on whoever answers for `b`.
/// True when it was made.
pub fn try_protection(world: &mut World, gang: EntityId, leader: EntityId, b: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(owner) = payer_of(world, b) else { return false };
    let Some(over) = answers_for(world, owner) else { return false };
    if over == leader {
        return false;
    }
    mark(world, gang, b, tried::PROTECT);
    let weekly = world.config.leverage.protection_weekly;
    let amount = match world.comp::<Building>(b) {
        Some(bd) if crate::systems::protection::on(world) => {
            crate::systems::protection::levy(world, week_revenue(bd), weekly).unwrap_or(weekly)
        }
        _ => weekly,
    };
    let demand = Demand::Coins { amount, weekly: true };
    let out = roll(world, leader, over, Some(owner), &Leverage::Threat, &demand);
    if !out.success {
        // The pressure pass phase 3 (OPEN-160): a refusal stands as a hold
        // the gang punishes in person.
        if crate::systems::protection::on(world) {
            let id = record(
                world,
                Some(gang),
                leader,
                over,
                Leverage::Threat,
                demand,
                CoercionWhy::Protection,
                Some(owner),
                Some(b),
            );
            crate::systems::protection::refused(world, id);
        }
        return false;
    }
    let id = record(
        world,
        Some(gang),
        leader,
        over,
        Leverage::Threat,
        demand,
        CoercionWhy::Protection,
        Some(owner),
        Some(b),
    );
    world.stats.current.leverage.protection_made += 1;
    if crate::systems::protection::on(world) {
        crate::systems::ownership::jobs2_row(world).pr_leaned += 1;
    }
    let d = world.district_of_building(b);
    crate::systems::gossip::post_deed(world, d, Deed::Extorted, Some(leader), Some(over));
    // The first week now (the pre-view pass phase 6, P13: a body leader
    // collects it on his round, in person).
    if !crate::systems::leaders::collects(world, gang) {
        collect_one(world, id);
    }
    true
}

/// The pressure pass phase 3: an owner who went to rival `gang` for cover
/// pays it `amount` a week from now on (no roll: he asked); the first week
/// now unless its leader collects in person.
pub(crate) fn cover(
    world: &mut World,
    gang: EntityId,
    leader: EntityId,
    over: EntityId,
    payer: EntityId,
    b: EntityId,
    amount: i64,
) {
    let demand = Demand::Coins { amount, weekly: true };
    let id = record(
        world,
        Some(gang),
        leader,
        over,
        Leverage::Threat,
        demand,
        CoercionWhy::Protection,
        Some(payer),
        Some(b),
    );
    world.stats.current.leverage.protection_made += 1;
    if !crate::systems::leaders::collects(world, gang) {
        collect_one(world, id);
    }
}

/// A coercion just made pays its first week now.
pub(super) fn collect_one(world: &mut World, id: CoercionId) {
    pay_week(world, id);
}
