use super::*;

// ---------------------------------------------------------------------------
// Indices and the two writers (plan D2)
// ---------------------------------------------------------------------------

fn owner_key(owner: Option<EntityId>) -> EntityId {
    owner.unwrap_or(EntityId::NONE)
}

/// File an asset in every index from its current `loc` and `owner`.
pub(super) fn file(world: &mut World, a: EntityId) {
    let Some((loc, owner, vehicle)) = world.comp::<Asset>(a).map(|x| (x.loc, x.owner, x.kind.is_vehicle())) else {
        return;
    };
    file_loc(world, a, loc);
    list_insert(world.assets_by_owner.entry(owner_key(owner)).or_default(), a);
    if vehicle {
        if let Err(i) = world.vehicles.binary_search(&a) {
            world.vehicles.insert(i, a);
        }
    }
}

fn file_loc(world: &mut World, a: EntityId, loc: AssetLoc) {
    match loc {
        AssetLoc::Limbo(h) => list_insert(world.limbo.entry(h).or_default(), a),
        other => {
            if let Some(holder) = other.holder() {
                list_insert(world.assets_at.entry(holder).or_default(), a);
            }
        }
    }
}

fn unfile_loc(world: &mut World, a: EntityId, loc: AssetLoc) {
    match loc {
        AssetLoc::Limbo(h) => {
            if let Some(l) = world.limbo.get_mut(&h) {
                list_remove(l, a);
                if l.is_empty() {
                    world.limbo.remove(&h);
                }
            }
        }
        other => {
            if let Some(holder) = other.holder() {
                if let Some(l) = world.assets_at.get_mut(&holder) {
                    list_remove(l, a);
                    if l.is_empty() {
                        world.assets_at.remove(&holder);
                    }
                }
            }
        }
    }
}

fn unfile_owner(world: &mut World, a: EntityId, owner: Option<EntityId>) {
    let key = owner_key(owner);
    if let Some(l) = world.assets_by_owner.get_mut(&key) {
        list_remove(l, a);
        if l.is_empty() {
            world.assets_by_owner.remove(&key);
        }
    }
}

/// Re-kit the agents an asset touches: its holder, owner and keeper.
pub(super) fn rekit_touched(world: &mut World, ids: &[Option<EntityId>]) {
    let mut seen: SmallVec<[EntityId; 6]> = SmallVec::new();
    for id in ids.iter().flatten() {
        if !seen.contains(id) {
            seen.push(*id);
            rekit(world, *id);
        }
    }
}

pub(super) fn touched(world: &World, a: EntityId) -> [Option<EntityId>; 3] {
    world.comp::<Asset>(a).map_or([None; 3], |x| [x.loc.holder(), x.owner, x.keeper])
}

/// The one writer of `Asset.loc`.
pub fn set_loc(world: &mut World, a: EntityId, loc: AssetLoc) {
    let Some(old) = world.comp::<Asset>(a).map(|x| x.loc) else { return };
    let before = touched(world, a);
    unfile_loc(world, a, old);
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.loc = loc;
    }
    file_loc(world, a, loc);
    let after = touched(world, a);
    rekit_touched(world, &[before[0], before[1], before[2], after[0]]);
}

/// The one writer of `Asset.owner` (`None` = the city).
pub fn set_owner(world: &mut World, a: EntityId, owner: Option<EntityId>) {
    let Some(old) = world.comp::<Asset>(a).map(|x| x.owner) else { return };
    let before = touched(world, a);
    unfile_owner(world, a, old);
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.owner = owner;
    }
    list_insert(world.assets_by_owner.entry(owner_key(owner)).or_default(), a);
    rekit_touched(world, &[before[0], before[1], before[2], owner]);
}

/// Set (or clear) a fleet vehicle's keeper (plan D24). A new keeper (or
/// none) starts the soft recall's count afresh (`away_days`, phase 5): one
/// keeper's night away never adds to the next one's.
pub fn set_keeper(world: &mut World, a: EntityId, keeper: Option<EntityId>) {
    let Some(old) = world.comp::<Asset>(a).map(|x| x.keeper) else { return };
    if let Some(x) = world.comp_mut::<Asset>(a) {
        x.keeper = keeper;
        if old != keeper {
            x.away_days = 0;
        }
    }
    rekit_touched(world, &[old, keeper]);
}

/// A change of title clears the last holder's claims: no keeper, not
/// stolen, no upkeep arrears, no finance (a financed buyer's plan is set
/// after), the soft recall's count reset. Called by the impound, the
/// impound sale, a tow, every used-asset resale and [`into_gang_stock`].
pub fn reset_title(world: &mut World, a: EntityId) {
    set_keeper(world, a, None);
    if let Some(m) = world.comp_mut::<Asset>(a) {
        m.stolen = false;
        m.upkeep_arrears = 0;
        m.finance = None;
        m.away_days = 0;
    }
}

/// Stolen goods into a gang's Hideout stock (an off-screen strip, a rip, a
/// settled abduction, an off-screen vehicle theft): the title reset
/// ([`reset_title`]), owned by `gang`, in `hideout`'s stock, stolen, and
/// unbricked (the gang's ripperdoc strips the lender's lock).
pub fn into_gang_stock(world: &mut World, a: EntityId, gang: EntityId, hideout: EntityId) {
    reset_title(world, a);
    set_owner(world, a, Some(gang));
    set_loc(world, a, AssetLoc::Stock(hideout));
    if let Some(m) = world.comp_mut::<Asset>(a) {
        m.stolen = true;
        m.bricked = false;
    }
}

/// A new asset: condition 100, value = `list`, upkeep from the config.
pub fn spawn_asset(
    world: &mut World,
    kind: AssetKind,
    tier: u8,
    owner: Option<EntityId>,
    loc: AssetLoc,
    list: i64,
) -> EntityId {
    let upkeep_per_day = upkeep_for(world, kind, tier);
    let id = world.spawn();
    let bought = world.tick;
    world.insert(
        id,
        Asset {
            kind,
            tier,
            owner,
            loc,
            condition: 100,
            value: list,
            upkeep_per_day,
            upkeep_arrears: 0,
            finance: None,
            bricked: false,
            stolen: false,
            bought,
            keeper: None,
            list,
            away_days: 0,
            maker: None,
            data: [0; 3],
            turned: None,
            prov: None,
            wkind: 0,
        },
    );
    file(world, id);
    let t = touched(world, id);
    rekit_touched(world, &t);
    id
}

/// Unfile, re-kit and free an asset.
pub fn despawn(world: &mut World, a: EntityId) {
    let Some((loc, owner, vehicle)) = world.comp::<Asset>(a).map(|x| (x.loc, x.owner, x.kind.is_vehicle())) else {
        return;
    };
    let t = touched(world, a);
    unfile_loc(world, a, loc);
    unfile_owner(world, a, owner);
    if vehicle {
        if let Ok(i) = world.vehicles.binary_search(&a) {
            world.vehicles.remove(i);
        }
    }
    world.robot_sellers.remove(&a);
    world.despawn(a);
    rekit_touched(world, &t);
}

/// The assets at a building or agent (its `loc` names it), ascending.
pub fn assets_at(world: &World, holder: EntityId) -> &[EntityId] {
    world.assets_at.get(&holder).map_or(&[], |l| l.as_slice())
}

/// The assets an owner holds (`None` = the city), ascending.
pub fn assets_of(world: &World, owner: Option<EntityId>) -> &[EntityId] {
    world.assets_by_owner.get(&owner_key(owner)).map_or(&[], |l| l.as_slice())
}

/// Every asset, ascending (each is filed under exactly one owner).
pub fn all_assets(world: &World) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = world.assets_by_owner.values().flatten().copied().collect();
    v.sort_unstable();
    v
}

/// The D2 indices rebuilt from the stores, for `rebuild` and `check`.
#[allow(clippy::type_complexity)]
fn indices_from_stores(
    world: &World,
) -> (
    std::collections::BTreeMap<EntityId, List>,
    std::collections::BTreeMap<EntityId, List>,
    Vec<EntityId>,
    std::collections::BTreeMap<crate::components::HoleId, List>,
    Vec<EntityId>,
) {
    let mut at: std::collections::BTreeMap<EntityId, List> = Default::default();
    let mut by_owner: std::collections::BTreeMap<EntityId, List> = Default::default();
    let mut vehicles = Vec::new();
    let mut limbo: std::collections::BTreeMap<crate::components::HoleId, List> = Default::default();
    let mut loot = Vec::new();
    for id in world.entities() {
        if let Some(x) = world.comp::<Asset>(id) {
            match x.loc {
                AssetLoc::Limbo(h) => limbo.entry(h).or_default().push(id),
                other => {
                    if let Some(h) = other.holder() {
                        at.entry(h).or_default().push(id);
                    }
                }
            }
            by_owner.entry(owner_key(x.owner)).or_default().push(id);
            if x.kind.is_vehicle() {
                vehicles.push(id);
            }
        }
        if world.comp::<Corpse>(id).is_some_and(|c| !c.settled) {
            loot.push(id);
        }
    }
    (at, by_owner, vehicles, limbo, loot)
}

/// After a load (plan D2, D3, D50): the indices from the stores, a Kit on
/// every agent with a Body, each rebuilt. Tolerates stores shorter than the
/// arena.
pub fn rebuild(world: &mut World) {
    let n = world.generations.len();
    if world.kit.len() < n {
        world.kit.resize_with(n, || None);
    }
    let (at, by_owner, vehicles, limbo, loot) = indices_from_stores(world);
    world.assets_at = at;
    world.assets_by_owner = by_owner;
    world.vehicles = vehicles;
    world.limbo = limbo;
    world.loot_corpses = loot;
    let bodies: Vec<EntityId> = world.entities().filter(|&id| world.has::<Body>(id)).collect();
    for id in bodies {
        world.kit[id.index as usize] = Some(Kit::default());
        rekit(world, id);
    }
}

/// Compare the incremental indices with a rebuild from the stores.
pub fn check(world: &World) -> Result<(), String> {
    let (at, by_owner, vehicles, limbo, loot) = indices_from_stores(world);
    if at != world.assets_at || by_owner != world.assets_by_owner {
        return Err("assets_at / assets_by_owner out of sync with the Asset store".to_string());
    }
    if vehicles != world.vehicles || limbo != world.limbo {
        return Err("vehicles / limbo out of sync with the Asset store".to_string());
    }
    if loot != world.loot_corpses {
        return Err(format!("loot_corpses out of sync: {:?}", world.loot_corpses));
    }
    for id in world.entities() {
        if let Some(k) = world.comp::<Kit>(id) {
            if *k != compute_kit(world, id) {
                return Err(format!("Kit of {id} is stale"));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The Kit (plan D3, Kit tables)
// ---------------------------------------------------------------------------

/// The agent's own vehicle (lowest id, not wrecked, not in stock or limbo),
/// else the vehicle it keeps (plan D24).
pub fn vehicle_of(world: &World, agent: EntityId) -> Option<EntityId> {
    let usable =
        |x: &Asset| x.kind.is_vehicle() && x.condition > 0 && !matches!(x.loc, AssetLoc::Stock(_) | AssetLoc::Limbo(_));
    assets_of(world, Some(agent)).iter().copied().find(|&a| world.comp::<Asset>(a).is_some_and(usable)).or_else(|| {
        world
            .vehicles
            .iter()
            .copied()
            .find(|&a| world.comp::<Asset>(a).is_some_and(|x| x.keeper == Some(agent) && usable(x)))
    })
}
