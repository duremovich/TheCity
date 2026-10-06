//! M11 founding (docs/M11_OWNERSHIP.md § 6, plan D25-D27): building on a
//! vacant Lot (a corp's `Grow`, an agent's `Register`), the `Found` goal's
//! eligibility, and incorporation of an agent who owns enough buildings.

use crate::components::{
    Brain, Building, BuildingKind, Corp, Corpse, GangMember, Household, Identity, Job, Lod, Position, Sentence,
    TileKind, TilePos, Wallet, Zone,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::world::World;

/// The kinds an agent can found, in tie-break order (D25: tie -> Bar; M12
/// D20 appends the Hotel, foundable only with `[street] enabled`; M13 D16
/// the Clinic and Garage, each foundable only with its `[assets]
/// found_clinic` / `found_garage` flag).
const FOUNDABLE: [BuildingKind; 5] =
    [BuildingKind::Bar, BuildingKind::Home, BuildingKind::Hotel, BuildingKind::Clinic, BuildingKind::Garage];

/// Vacant Lots (kind Lot, not demolished), ascending.
pub fn vacant_lots(world: &World) -> Vec<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Lot)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lot && !bd.demolished))
        .collect()
}

/// What founding a building of `kind` costs (Bar, Home, and with the street
/// on a Hotel; M13 D16 a Clinic or Garage with `[assets] found_clinic` /
/// `found_garage`, per kind from phase 2).
pub fn found_cost(world: &World, kind: BuildingKind) -> Option<i64> {
    let c = &world.config.corps.found_cost;
    let a = &world.config.assets;
    match kind {
        BuildingKind::Bar => Some(c.bar),
        BuildingKind::Home => Some(c.home),
        BuildingKind::Hotel if world.config.street.enabled => Some(c.hotel),
        BuildingKind::Clinic if a.enabled && a.found_clinic => Some(c.clinic),
        BuildingKind::Garage if a.enabled && a.found_garage => Some(c.garage),
        _ => None,
    }
}

/// The vacant Lot whose door is nearest `from` (Manhattan, ties lower id).
pub fn nearest_lot(world: &World, from: TilePos) -> Option<EntityId> {
    vacant_lots(world)
        .into_iter()
        .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(from), l)))
        .min()
        .map(|(_, l)| l)
}

/// D25: convert a vacant Lot in place into a `kind` building owned by
/// `owner`: same `EntityId`, walls on the perimeter but the door, the
/// interior left Ground, capacity `min(cfg, interior tiles)`, tier from the
/// door's zone (Spire 2, Sump 0, else 1), full staff posted as vacancies.
/// Agents standing in the rect are put outside the door with their plans
/// dropped, and the flow fields are rebuilt.
pub fn build_on_lot(
    world: &mut World,
    lot: EntityId,
    kind: BuildingKind,
    owner: Option<EntityId>,
) -> Result<EntityId, String> {
    // M12 D36: a splinter gang builds its Hideout on a Lot.
    // M13 D16: and the Clinic and Garage.
    if !matches!(
        kind,
        BuildingKind::Bar
            | BuildingKind::Home
            | BuildingKind::Hotel
            | BuildingKind::Hideout
            | BuildingKind::Clinic
            | BuildingKind::Garage
    ) {
        return Err(format!("cannot build a {} on a Lot", kind.label()));
    }
    let Some(b) = world.comp::<Building>(lot) else { return Err("no such Lot".into()) };
    if b.kind != BuildingKind::Lot || b.demolished {
        return Err(format!("{} is not a vacant Lot", world.name_of(lot)));
    }
    let (rect, door) = (b.rect, b.door);
    for y in rect.y..rect.y + rect.h {
        for x in rect.x..rect.x + rect.w {
            let t = TilePos { x, y };
            if t != door && rect.on_perimeter(t) {
                world.map.set_tile(t, TileKind::Wall);
            }
        }
    }
    let outside = world.comp::<Building>(lot).map(|bd| world.outside_door(bd)).unwrap_or(door);
    // Anyone standing in the rect (a body crossing the open Lot) steps out.
    let inside: Vec<EntityId> = world
        // scan-ok: rare: a building goes up
        .citizens()
        .into_iter()
        .filter(|&a| world.comp::<Position>(a).is_some_and(|p| p.building.is_none() && rect.contains(p.tile)))
        .collect();
    let tick = world.tick;
    for a in inside {
        world.abort_plan(a);
        if let Some(p) = world.comp_mut::<Position>(a) {
            p.tile = outside;
            p.entered = tick;
        }
    }
    let cfg = world.config.buildings.for_kind(kind).clone();
    let interior = usize::from(rect.w.saturating_sub(2)) * usize::from(rect.h.saturating_sub(2));
    let mut capacity = u8::try_from(interior).unwrap_or(u8::MAX).min(cfg.capacity);
    if kind == BuildingKind::Hotel {
        // M12 D20: a bed per `[street] hotel_beds`, as the interior allows.
        capacity = u8::try_from(interior).unwrap_or(u8::MAX).min(world.config.street.hotel_beds);
    }
    let tier = match world.map.zone(door) {
        Zone::Spire => 2,
        Zone::Sump => 0,
        _ => 1,
    };
    if let Some(bd) = world.comp_mut::<Building>(lot) {
        bd.kind = kind;
        bd.capacity = capacity;
        bd.tier = tier;
        bd.stock_food = 0;
    }
    if let Some(v) = world.buildings_by_kind.get_mut(&BuildingKind::Lot) {
        v.retain(|&x| x != lot);
    }
    // Kept ascending, as every other `buildings_by_kind` list.
    let list = world.buildings_by_kind.entry(kind).or_default();
    if let Err(i) = list.binary_search(&lot) {
        list.insert(i, lot);
    }
    crate::systems::ownership::transfer_building(world, lot, owner);
    let rent = crate::systems::ownership::rent_for(world, lot);
    if let Some(bd) = world.comp_mut::<Building>(lot) {
        bd.rent_per_day = if kind == BuildingKind::Home { rent } else { 0 };
    }
    if let Some(role) = crate::systems::ownership::role_for(kind) {
        if cfg.staff > 0 {
            world.vacancies.entry(lot).or_default().extend(std::iter::repeat_n(role, cfg.staff as usize));
        }
    }
    world.invalidate_flow_fields_for_lot(rect);
    // M12 D1: a new Block joins its district's `homes`.
    crate::systems::districts::rebuild(world);
    Ok(lot)
}

/// Agents with a Brain (every tier): the per-capita denominator, O(1).
pub fn living(world: &World) -> usize {
    [Lod::Full, Lod::Coarse, Lod::Statistical].iter().map(|&l| world.tier(l).len()).sum()
}

/// D25: the affordable kind with the lowest `count ÷ target`, where
/// `target(Bar) = population ÷ residents_per_bar` and `target(Home) =
/// population ÷ residents_per_home`; ties to the Bar.
pub fn choose_kind(world: &World, coins: i64) -> Option<BuildingKind> {
    let pop = living(world).max(1) as f32;
    let per = |kind: BuildingKind| -> f32 {
        match kind {
            BuildingKind::Bar => world.config.corps.residents_per_bar.max(1) as f32,
            BuildingKind::Hotel => world.config.corps.residents_per_hotel.max(1) as f32,
            BuildingKind::Clinic => world.config.corps.residents_per_clinic.max(1) as f32,
            BuildingKind::Garage => world.config.corps.residents_per_garage.max(1) as f32,
            _ => world.config.world.residents_per_home.max(1) as f32,
        }
    };
    let mut best: Option<(f32, BuildingKind)> = None;
    for kind in FOUNDABLE {
        let Some(cost) = found_cost(world, kind) else { continue };
        if coins < cost {
            continue;
        }
        let count = world
            .buildings_of_kind(kind)
            .iter()
            .filter(|&&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
            .count() as f32;
        let ratio = count / (pop / per(kind)).max(1e-3);
        if best.is_none_or(|(r, _)| ratio < r) {
            best = Some((ratio, kind));
        }
    }
    best.map(|(_, k)| k)
}

/// The executive of some corp (they grow through the corp, not by founding).
pub fn is_exec(world: &World, agent: EntityId) -> bool {
    world.corps().into_iter().any(|c| world.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(agent)))
}

/// D26: may `agent` take up the Found goal now? An adult with a Brain, not a
/// gang member, not jailed, not a corp's exec, jobless or paid no more than
/// the dole, off the founding cooldown, who can afford a foundable kind,
/// with a vacant Lot in the city. Cheap checks first: this runs per think.
pub fn can_found(world: &World, agent: EntityId) -> bool {
    let coins = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
    let c = &world.config.corps.found_cost;
    if coins < c.bar.min(c.home) {
        return false;
    }
    let Some(brain) = world.comp::<Brain>(agent) else { return false };
    let cd = world.config.corps.found_cooldown_days;
    if brain.last_found_day.is_some_and(|d| world.day().saturating_sub(d) < cd) || brain.emigrating {
        return false;
    }
    if world.has::<GangMember>(agent) || world.has::<Sentence>(agent) || world.has::<Corpse>(agent) {
        return false;
    }
    if !crate::systems::demography::is_adult(world, agent) {
        return false;
    }
    let dole = i64::from(world.levers.dole_per_day);
    // M11 phase 5b: an owner working in their own building pays themselves,
    // so their wage is not a reason to stay put (seed 42's Bar owner tended
    // his own Bar at 5 a day with 625 coins and could never found again).
    let self_employed = |j: &Job| j.employer.is_some_and(|e| world.owner_of(e) == Some(agent));
    if world.comp::<Job>(agent).is_some_and(|j| j.wage_per_day > dole && !self_employed(j)) {
        return false;
    }
    // The city-wide gates before the per-kind scan (`choose_kind` counts
    // every Bar and Home): this runs per think, per plan and per hourly
    // LOD assignment. All three are pure, so the order changes nothing.
    any_vacant_lot(world) && !is_exec(world, agent) && choose_kind(world, coins).is_some()
}

/// Is any Lot vacant? `vacant_lots` without the list.
pub fn any_vacant_lot(world: &World) -> bool {
    world
        .buildings_of_kind(BuildingKind::Lot)
        .iter()
        .any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lot && !bd.demolished))
}

/// D25 / § 6: the `Register` action's effect. The founder pays `found_cost`
/// to the Treasury (`Flow::Found`), the Lot nearest their Home door (their
/// tile when homeless) becomes a building of the chosen kind owned by them,
/// `Founded` is logged with the founder in slot 0 ("registered", where a
/// corp's `Grow` says "built"), and an owner of `incorporate_buildings`
/// becomes a corp.
pub fn register(world: &mut World, agent: EntityId) -> Result<EntityId, String> {
    if !can_found(world, agent) {
        return Err("cannot found".into());
    }
    let coins = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
    let kind = choose_kind(world, coins).ok_or("nothing affordable")?;
    let cost = found_cost(world, kind).ok_or("not foundable")?;
    let from = world
        .comp::<Household>(agent)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(agent).map(|p| p.tile))
        .ok_or("nowhere")?;
    let lot = nearest_lot(world, from).ok_or("no vacant Lot")?;
    // Paid once the building stands (a failed build costs nothing), as a
    // corp's `Grow`.
    build_on_lot(world, lot, kind, Some(agent))?;
    let paid = ownership::pay(world, Some(agent), None, cost, Flow::Found);
    debug_assert_eq!(paid, cost, "can_found checked the wallet");
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(agent) {
        b.last_found_day = Some(today);
    }
    let (name, what) = (world.name_of(agent), world.name_of(lot));
    world.push_event(EventKind::Founded, &[agent, lot], format!("{name} registered {what} on a Lot for {cost}"));
    world.stats.current.foundings += 1;
    maybe_incorporate(world, agent);
    Ok(lot)
}

/// D27: an agent owning `[corps] incorporate_buildings` or more becomes a
/// corp `"{last name} Holdings"`: niches from the kinds owned, treasury 0,
/// exec the agent, no CSV slot; the buildings move to it. `Incorporated`
/// with the corp in slot 0 and the exec in slot 1 (the Life row).
pub fn maybe_incorporate(world: &mut World, agent: EntityId) -> Option<EntityId> {
    if !world.has::<Wallet>(agent) || !world.has::<Brain>(agent) || world.has::<Corpse>(agent) {
        return None;
    }
    let owned: Vec<EntityId> = world
        .with::<Building>()
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(agent) && !bd.demolished))
        .collect();
    incorporate_owned(world, agent, owned)
}

fn incorporate_owned(world: &mut World, agent: EntityId, owned: Vec<EntityId>) -> Option<EntityId> {
    let need = world.config.corps.incorporate_buildings.max(1);
    if owned.len() < need || is_exec(world, agent) {
        return None;
    }
    let niches: std::collections::BTreeSet<_> = owned
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| crate::systems::corp_brain::niche_of_kind(bd.kind)))
        .collect();
    if niches.is_empty() {
        return None;
    }
    let full = world.comp::<Identity>(agent).map(|i| i.name.clone()).unwrap_or_default();
    let last = full.split_whitespace().last().unwrap_or("Nobody").to_string();
    let name = format!("{last} Holdings");
    let corp = ownership::spawn_corp(world, name.clone(), niches, 0, Some(agent));
    // Phase 5: the founder's savings become the corp's capital (an exec
    // draws a wage from it from now on) and its first `incorporate_grace_days`
    // carry no upkeep: at treasury 0 Varley Holdings went bankrupt under
    // upkeep on day 40, twelve days after it incorporated.
    // The founder keeps a day's exec wage to eat on until the first draw.
    let float = world.config.economy.wage_exec;
    let capital = world.comp::<Wallet>(agent).map_or(0, |w| (w.coins - float).max(0));
    if capital > 0 {
        ownership::pay(world, Some(agent), Some(corp), capital, ownership::Flow::Subsidy);
    }
    let grace = world.config.corps.incorporate_grace_days * crate::time::TICKS_PER_DAY;
    let now = world.tick;
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        // The `cash` denominator, as a spinoff's.
        c.treasury_ref = 1000;
        c.closing = c.treasury;
        if grace > 0 {
            c.upkeep_grace_until = Some(now + grace);
        }
    }
    for &b in &owned {
        crate::systems::corps::move_building(world, b, Some(corp));
    }
    let who = world.name_of(agent);
    world.push_event(
        EventKind::Incorporated,
        &[corp, agent],
        format!("{who} incorporated {name} ({} buildings)", owned.len()),
    );
    world.stats.current.incorporations += 1;
    Some(corp)
}

/// The daily pass (`ownership::run`): every agent owner of enough buildings
/// incorporates. One walk over the buildings.
pub fn incorporate_daily(world: &mut World) {
    let need = world.config.corps.incorporate_buildings.max(1);
    let mut by_owner: std::collections::BTreeMap<EntityId, Vec<EntityId>> = Default::default();
    for b in world.with::<Building>() {
        let Some(owner) = world.comp::<Building>(b).filter(|bd| !bd.demolished).and_then(|bd| bd.owner) else {
            continue;
        };
        if world.has::<Identity>(owner) && world.has::<Brain>(owner) && !world.has::<Corpse>(owner) {
            by_owner.entry(owner).or_default().push(b);
        }
    }
    for (agent, owned) in by_owner {
        if owned.len() >= need {
            incorporate_owned(world, agent, owned);
        }
    }
}
