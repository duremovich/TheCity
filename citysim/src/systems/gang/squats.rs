use super::*;

// ---------------------------------------------------------------------------
// Targets and extortion
// ---------------------------------------------------------------------------

/// The gang whose territory holds this Home.
pub fn holder_of(world: &World, home: EntityId) -> Option<EntityId> {
    world
        .gang_list()
        .iter()
        .copied()
        .find(|&g| world.comp::<Gang>(g).is_some_and(|gg| gg.territory.binary_search(&home).is_ok()))
}

/// Round 6 V2 (TURF § 2): Home `h` is open to `gang`'s Expand: with `[turf]`,
/// not in a cell another gang holds and not under another standing gang's
/// claim (a door one gang is working is that gang's); always off.
pub fn free_door(world: &World, gang: EntityId, h: EntityId) -> bool {
    if !crate::systems::turf::on(world) {
        return true;
    }
    let Some(b) = world.comp::<Building>(h) else { return false };
    crate::systems::turf::open_to(world, gang, b.door)
        && b.claim.is_none_or(|c| c.gang == gang || !world.has::<Gang>(c.gang))
}

/// The order a loyal member's GangWork serves right now, or `None` while
/// freelancing (below `freelance_loyalty`) or under an order with no
/// extortion in it. Cheap: no Home scan.
pub fn following_order(world: &World, id: EntityId) -> Option<Order> {
    let g = world.gang_of(id).and_then(|g| world.comp::<Gang>(g))?;
    let loyalty = world.comp::<Personality>(id).map_or(0.0, |p| p.loyalty);
    if loyalty < world.config.gangs.freelance_loyalty {
        return None;
    }
    // M16a (plan C26, phase 2): under Job the members outside the crew keep
    // Expand's GangWork (the crew marches under the Contract goal).
    if g.order == Order::Job {
        // Lives at work phase 6 (§ 4.5): not a delegating leader.
        if crate::systems::careers::delegating(world, id) {
            return None;
        }
        return Some(Order::Expand);
    }
    // Lives at work phase 6 (§ 4.5): a delegating leader works none of the
    // street orders (Contest's defence and the Lead goal stay his).
    if matches!(g.order, Order::Expand | Order::Squat | Order::Harvest | Order::VirtRaid)
        && crate::systems::careers::delegating(world, id)
    {
        return None;
    }
    // M14 V30: under VirtRaid the members not running keep GangWork.
    matches!(g.order, Order::Expand | Order::Contest | Order::Squat | Order::Harvest | Order::VirtRaid)
        .then_some(g.order)
}

/// M12 D38 (phase 3): the derelict Blocks a gang's Squat order may take:
/// in the district of its Hideout or of a Home it holds, not held by it
/// yet, ascending.
pub fn squat_targets(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let districts = gang_districts(world, g);
    // Fix pass (phase 3 review): any derelict (a Block, a Bar, a Hotel), no
    // shanty Lot.
    crate::systems::street::derelict_buildings(world)
        .into_iter()
        .filter(|&b| g.territory.binary_search(&b).is_err())
        .filter(|&b| districts.binary_search(&world.district_of_building(b)).is_ok())
        .collect()
}

/// The districts of a gang's territory and Hideout, ascending.
pub(super) fn gang_districts(world: &World, g: &Gang) -> Vec<crate::components::DistrictId> {
    let mut districts: Vec<crate::components::DistrictId> =
        g.territory.iter().map(|&h| world.district_of_building(h)).collect();
    districts.push(world.district_of_building(g.hideout));
    districts.sort_unstable();
    districts.dedup();
    districts
}

/// Scale S4 perf: "a guard within `r` (Chebyshev) of this door", the
/// guards' tiles on a grid read on the first test (a search that tests
/// many doors read every guard per door). Pressure pass round 5, U8
/// (OPEN-229): the tick's grid (`World::guard_grid`), not one built per pick.
pub(super) fn guard_test(world: &World, r: u32) -> impl Fn(TilePos) -> bool + '_ {
    let grid: std::cell::OnceCell<std::sync::Arc<crate::world::GuardGrid>> = std::cell::OnceCell::new();
    move |door: TilePos| {
        let (tiles, grid) = &**grid.get_or_init(|| world.guard_grid());
        let mut near = Vec::new();
        grid.near(door, r, &mut near);
        near.iter().any(|&i| law::chebyshev(tiles[i as usize], door) <= r)
    }
}

/// A member serving the gang's Squat order, inside a derelict.
pub fn serving_squat(world: &World, id: EntityId) -> bool {
    following_order(world, id) == Some(Order::Squat)
        && world
            .comp::<Position>(id)
            .and_then(|p| p.building)
            .is_some_and(|b| crate::systems::street::is_derelict(world, b))
}

/// M12 D38: a member's `Occupy` under the Squat order is a blow of the
/// gang's claim on derelict `b` (no extortion, no crime); a homeless
/// member also squats there. When the claim reaches `CLAIM_HELD` the
/// non-member squatters go: one with courage > 0.6 fights the claimant
/// (and stays if they win), the rest are evicted (ban, `SquatEvicted`).
pub fn squat_claim(world: &mut World, actor: EntityId, b: EntityId) {
    squat_claim_with(world, actor, b, true);
}

/// `squat_claim`; L2 (L23): `fights` false for a Statistical member's
/// claim off screen (no fight is drawn: a brave squatter simply stays).
pub fn squat_claim_with(world: &mut World, actor: EntityId, b: EntityId, fights: bool) {
    let today = world.day();
    if let Some(br) = world.comp_mut::<Brain>(actor) {
        br.gang_task_day = Some(today);
    }
    let Some(gang) = world.gang_of(actor) else { return };
    if world.comp::<Household>(actor).is_some_and(|h| h.home.is_none())
        && !world.has::<crate::components::Squatter>(actor)
    {
        let _ = crate::systems::street::occupy(world, actor, b);
    }
    let was_held =
        world.comp::<Building>(b).and_then(|bd| bd.claim).is_some_and(|c| c.gang == gang && c.count >= CLAIM_HELD);
    claim(world, actor, b);
    crate::systems::fviolence::note_act(world, actor, crate::ledger::ActKind::Claim);
    let held =
        world.comp::<Building>(b).and_then(|bd| bd.claim).is_some_and(|c| c.gang == gang && c.count >= CLAIM_HELD);
    if !held || was_held {
        return;
    }
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let what = world.name_of(b);
    world.push_event(EventKind::Squatted, &[actor, b], format!("{gname} took derelict {what} as a squat"));
    let squatters: Vec<EntityId> =
        world.squatters_of(b).iter().copied().filter(|s| members.binary_search(s).is_err()).collect();
    for s in squatters {
        // Round 6 V2 (the under-18 rule, TURF): a child squatter is neither
        // fought nor put out (nor does the claimant fight if he is one).
        if crate::systems::ages::is_protected(world, s) || crate::systems::ages::is_protected(world, actor) {
            continue;
        }
        let brave = world.comp::<Personality>(s).is_some_and(|p| p.courage > 0.6);
        if brave && !fights {
            continue;
        }
        if brave && world.has::<Brain>(s) && world.has::<Brain>(actor) {
            // C3a: live when both are Full; the tail (`squat_end`) puts a
            // beaten squatter out.
            let tail = crate::systems::combat::Tail::Squat { gname: gname.clone() };
            crate::systems::combat::fight(world, s, actor, Default::default(), tail);
            continue;
        }
        if world.has::<crate::components::Squatter>(s) {
            crate::systems::street::evict_squatter(world, s, &format!("taken by {gname}"));
        }
    }
}

/// C3a, a brave squatter's stand's tail: beaten, he is put out.
pub(crate) fn squat_end(world: &mut World, gname: &str, c: &crate::systems::combat::Clash) {
    let s = c.a;
    if c.winner != s && world.has::<crate::components::Squatter>(s) {
        crate::systems::street::evict_squatter(world, s, &format!("taken by {gname}"));
    }
}
