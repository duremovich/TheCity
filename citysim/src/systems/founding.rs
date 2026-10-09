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
/// found_clinic` / `found_garage` flag; M15 W36 the Feed, with `[news]` on;
/// L2 L6 the six leisure kinds, priced only with `jobs::on`; M16a C8 the
/// Fixer, priced only with `contracts::on` and chosen only by a founder
/// who could run one, `fixer_ok`; the Real economy E26 the Mission, priced
/// only with `charity::on` and chosen only by a lawful, humble founder,
/// `charity::founder_ok`).
const FOUNDABLE: [BuildingKind; 14] = [
    BuildingKind::Bar,
    BuildingKind::Home,
    BuildingKind::Hotel,
    BuildingKind::Clinic,
    BuildingKind::Garage,
    BuildingKind::Feed,
    BuildingKind::Club,
    BuildingKind::Arcade,
    BuildingKind::NoodleBar,
    BuildingKind::FightPit,
    BuildingKind::Den,
    BuildingKind::Lounge,
    BuildingKind::Fixer,
    BuildingKind::Mission,
];

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
        BuildingKind::Feed if c.feed > 0 => Some(c.feed),
        // L2 L6: the venues and the Fab (corps only), with jobs on.
        BuildingKind::Club => Some(c.club),
        BuildingKind::Arcade => Some(c.arcade),
        BuildingKind::NoodleBar => Some(c.noodle_bar),
        BuildingKind::FightPit => Some(c.fight_pit),
        BuildingKind::Den => Some(c.den),
        BuildingKind::Lounge => Some(c.lounge),
        BuildingKind::Fab => Some(c.fab),
        // M16a (plan C8): a Fixer's office, with contracts on.
        BuildingKind::Fixer => Some(c.fixer),
        // Real economy E26, E37: a Mission with charities on, a Camp (corps) with camps on.
        BuildingKind::Mission => Some(c.mission),
        BuildingKind::Camp => Some(c.camp),
        // Real economy phase 2 (plan E19 deviation): a corp's Farm on the
        // World's demand, with wages on (`[economy2] farm_found_cost`).
        BuildingKind::Farm if crate::systems::wages::on(world) => Some(world.config.economy2.farm_found_cost),
        _ => None,
    }
}

/// What an agent's `Register` pays: `found_cost`, except a Clinic or a
/// Garage at `[assets] found_cost_clinic` / `found_cost_garage` when set
/// (M13 phase 5; corps build at `found_cost`).
pub fn agent_found_cost(world: &World, kind: BuildingKind) -> Option<i64> {
    let cost = found_cost(world, kind)?;
    let a = &world.config.assets;
    Some(match kind {
        BuildingKind::Clinic => a.found_cost_clinic.unwrap_or(cost),
        BuildingKind::Garage => a.found_cost_garage.unwrap_or(cost),
        _ => cost,
    })
}

/// L2 review fix: the vacant Lot nearest `from` whose door's tier allows
/// `kind` (`tier_ok`), ties lower id.
pub fn nearest_lot_for(world: &World, kind: BuildingKind, from: TilePos) -> Option<EntityId> {
    vacant_lots(world)
        .into_iter()
        .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door, l)))
        .filter(|&(door, _)| tier_ok(kind, door_tier(world, door)))
        .map(|(door, l)| (door.manhattan(from), l))
        .min()
        .map(|(_, l)| l)
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
    // L2 L6: and the seven L2 kinds.
    if !matches!(
        kind,
        BuildingKind::Bar
            | BuildingKind::Home
            | BuildingKind::Hotel
            | BuildingKind::Hideout
            | BuildingKind::Clinic
            | BuildingKind::Garage
            | BuildingKind::Lab
            | BuildingKind::Feed
            | BuildingKind::Club
            | BuildingKind::Arcade
            | BuildingKind::NoodleBar
            | BuildingKind::FightPit
            | BuildingKind::Den
            | BuildingKind::Lounge
            | BuildingKind::Fab
            // M16a (plan C8).
            | BuildingKind::Fixer
            // Real economy E26, E37.
            | BuildingKind::Mission
            | BuildingKind::Camp
            // Real economy phase 2 (plan E19): a corp's Farm on World demand.
            | BuildingKind::Farm
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
    let tier = door_tier(world, door);
    convert(world, lot, kind, owner, tier);
    Ok(lot)
}

/// The tier of a door's zone (Spire 2, Sump 0, else 1), as `build_on_lot`.
pub fn door_tier(world: &World, door: TilePos) -> u8 {
    match world.map.zone(door) {
        Zone::Spire => 2,
        Zone::Sump => 0,
        _ => 1,
    }
}

/// L2 L6: `build_on_lot`'s tail after the walls, shared with `refit`: the
/// kind, capacity `min(cfg, interior)` (a Hotel's beds), `tier`, the
/// `buildings_by_kind` move, the owner, rent, the full staff posted as
/// vacancies, a leisure kind's `Venue`, the flow fields and the districts.
/// Jobs and room J7: the seats (`floor_seats` kinds) and the staff posted
/// are per floor, × the building's `floors`.
pub fn convert(world: &mut World, b: EntityId, kind: BuildingKind, owner: Option<EntityId>, tier: u8) {
    let Some((rect, old, floors)) = world.comp::<Building>(b).map(|bd| (bd.rect, bd.kind, bd.floors.max(1))) else {
        return;
    };
    let cfg = world.config.buildings.for_kind(kind).clone();
    let interior = usize::from(rect.w.saturating_sub(2)) * usize::from(rect.h.saturating_sub(2));
    let mut capacity = u8::try_from(interior).unwrap_or(u8::MAX).min(cfg.capacity);
    if kind == BuildingKind::Hotel {
        // M12 D20: a bed per `[street] hotel_beds`, as the interior allows.
        capacity = u8::try_from(interior).unwrap_or(u8::MAX).min(world.config.street.hotel_beds);
    }
    let capacity = with_floors(kind, capacity, floors);
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.kind = kind;
        bd.capacity = capacity;
        bd.tier = tier;
        bd.stock_food = 0;
    }
    if let Some(v) = world.buildings_by_kind.get_mut(&old) {
        v.retain(|&x| x != b);
    }
    // Kept ascending, as every other `buildings_by_kind` list.
    let list = world.buildings_by_kind.entry(kind).or_default();
    if let Err(i) = list.binary_search(&b) {
        list.insert(i, b);
    }
    crate::systems::ownership::transfer_building(world, b, owner);
    let rent = crate::systems::ownership::rent_for(world, b);
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.rent_per_day = if kind == BuildingKind::Home { rent } else { 0 };
    }
    if let Some(role) = crate::systems::ownership::role_for(kind) {
        if cfg.staff > 0 {
            let n = cfg.staff as usize * usize::from(floors);
            world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, n));
        }
    }
    // Jobs and room J15: and every trade of the kind its places, as full
    // staff (a `per_owned` trade's group places are the trades pass's).
    for (role, _) in crate::systems::ownership::roles_for(world, kind) {
        if !role.is_trade() {
            continue;
        }
        let n = crate::systems::jobs::places_of(world, b, role);
        if n > 0 {
            world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, n));
        }
    }
    // L2 L4: a leisure kind opens with today's price (no building of these
    // kinds stands with L2 off).
    if kind.is_leisure() {
        let price = crate::systems::jobs::venue_price(world, b, kind, tier, owner);
        if let Some(bd) = world.comp_mut::<Building>(b) {
            bd.venue = Some(crate::living::Venue { price, ..Default::default() });
        }
    }
    // M16a (plan C8): a Fixer's office opens its record book at the cut.
    if kind == BuildingKind::Fixer {
        let cut = world.config.fixers.fixer_cut;
        world.insert(b, crate::contract::Broker { cut, ..Default::default() });
    }
    // Real economy E26, E37: a Mission opens with its purse, a Camp with its state.
    crate::systems::charity::on_convert(world, b, kind);
    crate::systems::camp::on_convert(world, b, kind);
    world.invalidate_flow_fields_for_lot(rect);
    // M12 D1: a new Block joins its district's `homes`.
    crate::systems::districts::rebuild(world);
}

// ---------------------------------------------------------------------------
// Floors (Jobs and room § 2.3, plan J6-J8)
// ---------------------------------------------------------------------------

/// J7: the kinds whose seats or beds floors multiply: Blocks, Hotels and
/// the leisure venues (a Farm's or Market's `capacity` is its visitors'
/// room and its stock cap is not multiplied).
pub fn floor_seats(kind: BuildingKind) -> bool {
    matches!(kind, BuildingKind::Home | BuildingKind::Hotel) || kind.is_leisure()
}

/// J7: one floor's `capacity` × `floors` for a [`floor_seats`] kind,
/// clamped at 255 (`capacity` is a `u8`); `per_floor` otherwise.
pub fn with_floors(kind: BuildingKind, per_floor: u8, floors: u8) -> u8 {
    if !floor_seats(kind) || floors <= 1 {
        return per_floor;
    }
    u8::try_from(usize::from(per_floor) * usize::from(floors)).unwrap_or(u8::MAX)
}

/// J8: what a floor on a `kind` costs: `floor_cost_frac × found_cost`
/// (`None` for a kind no one can found, so no one can build up).
pub fn floor_cost(world: &World, kind: BuildingKind) -> Option<i64> {
    let cost = found_cost(world, kind)?;
    Some((world.config.floors.floor_cost_frac * cost as f32).round() as i64)
}

/// J8 `AddFloor`: `b` gains a storey paid by `payer` ([`floor_cost`] to
/// the Treasury, `Flow::Found`, until P4's Construction Yard). The
/// building must stand (not demolished, derelict or a Lot), be under its
/// kind's `[floors] floors_max` and the payer must hold the cost. Seats
/// grow by one floor's worth (clamped at 255); the new floor's places are
/// posted by the staffing passes (`jobs::places_of`). `FloorAdded` is logged.
pub fn add_floor(world: &mut World, b: EntityId, payer: Option<EntityId>) -> Result<(), String> {
    let Some(bd) = world.comp::<Building>(b) else { return Err("no such building".into()) };
    if bd.demolished || bd.derelict || bd.kind == BuildingKind::Lot {
        return Err(format!("{} is not a standing building", world.name_of(b)));
    }
    let (kind, floors, capacity) = (bd.kind, bd.floors.max(1), bd.capacity);
    let max = world.config.floors.max_for(kind);
    if floors >= max {
        return Err(format!("{} already has {floors} floors (max {max})", world.name_of(b)));
    }
    let cost = floor_cost(world, kind).ok_or_else(|| format!("no one builds up a {}", kind.label()))?;
    if payer.is_some() && world.purse(payer) < cost {
        return Err(format!("{} cannot pay {cost}", world.owner_label(payer)));
    }
    let per_floor = capacity / floors;
    let new_capacity = with_floors(kind, per_floor, floors + 1).max(capacity);
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.floors = floors + 1;
        bd.capacity = new_capacity;
        bd.floor_days = 0;
    }
    let paid = if payer.is_some() && cost > 0 { ownership::pay(world, payer, None, cost, Flow::Found) } else { 0 };
    let (who, what) = (world.owner_label(payer), world.name_of(b));
    let mut actors: Vec<EntityId> = payer.into_iter().collect();
    actors.push(b);
    world.push_event(
        EventKind::FloorAdded,
        &actors,
        format!("{who} added a floor to {what} (now {} floors) for {paid}", floors + 1),
    );
    Ok(())
}

/// L2 L6: may a derelict in district tier `tier` be refitted as `kind`?
/// The venue tier rule of the seeding table (Spire 2, Mid 1, Sump 0).
pub fn tier_ok(kind: BuildingKind, tier: u8) -> bool {
    match kind {
        BuildingKind::Lounge => tier == 2,
        BuildingKind::Club => tier >= 1,
        BuildingKind::FightPit => tier == 0,
        BuildingKind::Den | BuildingKind::NoodleBar => tier <= 1,
        _ => true,
    }
}

/// L2 L6: the derelict Block nearest `from` that may become `kind` (the
/// tier rule; ties lower id), with jobs on.
pub fn refit_ok(world: &World, kind: BuildingKind, from: TilePos) -> Option<EntityId> {
    if !kind.is_leisure() {
        return None;
    }
    crate::systems::street::derelicts(world)
        .into_iter()
        .filter_map(|d| {
            let bd = world.comp::<Building>(d)?;
            (bd.kind == BuildingKind::Home && !bd.demolished && tier_ok(kind, bd.tier))
                .then(|| (bd.door.manhattan(from), d))
        })
        .min()
        .map(|(_, d)| d)
}

/// Real economy phase 2 (plan E19): the kinds a corp's Grow may refit a
/// derelict Block into with wages on (its niche kinds that `grow_kind`
/// returns; a Block stays Lots-only).
pub fn grow_refit_kind(world: &World, kind: BuildingKind) -> bool {
    crate::systems::wages::on(world)
        && matches!(
            kind,
            BuildingKind::Farm | BuildingKind::Bar | BuildingKind::Garage | BuildingKind::Clinic | BuildingKind::Fab
        )
}

/// `refit_ok` for a corp's Grow (plan E19): the leisure kinds as L2, plus
/// `grow_refit_kind`'s (any tier).
pub fn refit_ok_for(world: &World, kind: BuildingKind, from: TilePos) -> Option<EntityId> {
    if !grow_refit_kind(world, kind) {
        return refit_ok(world, kind, from);
    }
    crate::systems::street::derelicts(world)
        .into_iter()
        .filter_map(|d| {
            let bd = world.comp::<Building>(d)?;
            (bd.kind == BuildingKind::Home && !bd.demolished).then(|| (bd.door.manhattan(from), d))
        })
        .min()
        .map(|(_, d)| d)
}

/// L2 L6: a derelict Block becomes a leisure kind owned by `owner`:
/// squatters out, no longer derelict, tier kept; the owner pays
/// `refit_frac × found_cost` to the Treasury (`Flow::Found`) once it
/// stands. `Refit` is logged.
pub fn refit(world: &mut World, b: EntityId, kind: BuildingKind, owner: Option<EntityId>) -> Result<EntityId, String> {
    refit_with(world, b, kind, owner, true)
}

/// `refit`, with the owner paying (`charge`) or not (a god's `OpenVenue`).
pub fn refit_with(
    world: &mut World,
    b: EntityId,
    kind: BuildingKind,
    owner: Option<EntityId>,
    charge: bool,
) -> Result<EntityId, String> {
    // M16a (plan C9): a seeded Fixer may stand in a refitted derelict.
    // Real economy E26, E37: a Mission or a Camp too; phase 2 (plan E19): a corp's Grow kinds with wages on.
    if !kind.is_leisure()
        && !matches!(kind, BuildingKind::Fixer | BuildingKind::Mission | BuildingKind::Camp)
        && !grow_refit_kind(world, kind)
    {
        return Err(format!("cannot refit as a {}", kind.label()));
    }
    let cost = if charge { found_cost(world, kind).ok_or("not foundable")? } else { 0 };
    let Some(bd) = world.comp::<Building>(b) else { return Err("no such building".into()) };
    if !bd.derelict || bd.kind != BuildingKind::Home || bd.demolished {
        return Err(format!("{} is not a derelict Block", world.name_of(b)));
    }
    let tier = bd.tier;
    let price = (world.config.jobs.refit_frac * cost as f32).round() as i64;
    if owner.is_some() && world.purse(owner) < price {
        return Err(format!("{} cannot pay {price}", world.owner_label(owner)));
    }
    crate::systems::street::evict_squatters(world, b, "refitted");
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.derelict = false;
        bd.full_capacity = None;
        bd.empty_since = None;
    }
    convert(world, b, kind, owner, tier);
    let paid = if owner.is_some() && price > 0 { ownership::pay(world, owner, None, price, Flow::Found) } else { 0 };
    let (who, what) = (world.owner_label(owner), world.name_of(b));
    let mut actors: Vec<EntityId> = owner.into_iter().collect();
    actors.push(b);
    world.push_event(
        EventKind::Refit,
        &actors,
        format!("{who} refitted {what} (a derelict Block) as a {} for {paid}", kind.label()),
    );
    Ok(b)
}

/// Agents with a Brain (every tier): the per-capita denominator, O(1).
pub fn living(world: &World) -> usize {
    [Lod::Full, Lod::Coarse, Lod::Statistical].iter().map(|&l| world.tier(l).len()).sum()
}

/// D25: the affordable kind with the lowest `count ÷ target`, where
/// `target(Bar) = population ÷ residents_per_bar` and `target(Home) =
/// population ÷ residents_per_home`; ties to the Bar.
pub fn choose_kind(world: &World, coins: i64) -> Option<BuildingKind> {
    choose_kind_for(world, coins, false)
}

/// L2 L6: `choose_kind` for a founder; the Lounge only for a Corp-class
/// founder (`lounge_ok`), the leisure kinds by their `residents_per_*`.
pub fn choose_kind_for(world: &World, coins: i64, lounge_ok: bool) -> Option<BuildingKind> {
    choose_kind_with(world, coins, lounge_ok, false)
}

/// M16a (plan C8): may `agent` open a Fixer's office: contracts on and
/// Fixers licensed, lawfulness under `fixer_lawfulness`, persuasion +
/// knowledge at least `fixer_skill`, and open Fixers under `residents ÷
/// fixers_per_pop`.
pub fn fixer_ok(world: &World, agent: EntityId) -> bool {
    if !world.levers.fixer_licence {
        return false;
    }
    let f = &world.config.fixers;
    let Some(p) = world.comp::<crate::components::Personality>(agent) else { return false };
    let Some(s) = world.comp::<crate::components::Skills>(agent) else { return false };
    if p.lawfulness >= f.fixer_lawfulness || s.persuasion + s.knowledge < f.fixer_skill {
        return false;
    }
    let cap = living(world) / (f.fixers_per_pop.max(1) as usize);
    crate::systems::contracts::open_fixers(world).len() < cap
}

/// `choose_kind_for` with the Fixer among the kinds when `fixer` (C8).
pub fn choose_kind_with(world: &World, coins: i64, lounge_ok: bool, fixer: bool) -> Option<BuildingKind> {
    choose_kind_full(world, coins, lounge_ok, fixer, false)
}

/// `choose_kind_with` with the Mission among the kinds when `mission`
/// (Real economy E26: `charity::founder_ok`).
pub fn choose_kind_full(
    world: &World,
    coins: i64,
    lounge_ok: bool,
    fixer: bool,
    mission: bool,
) -> Option<BuildingKind> {
    let pop = living(world).max(1) as f32;
    let per = |kind: BuildingKind| -> f32 {
        match kind {
            BuildingKind::Bar => world.config.corps.residents_per_bar.max(1) as f32,
            BuildingKind::Hotel => world.config.corps.residents_per_hotel.max(1) as f32,
            BuildingKind::Clinic => world
                .config
                .assets
                .founder_residents_per_seller
                .unwrap_or(world.config.corps.residents_per_clinic)
                .max(1) as f32,
            BuildingKind::Garage => world
                .config
                .assets
                .founder_residents_per_seller
                .unwrap_or(world.config.corps.residents_per_garage)
                .max(1) as f32,
            BuildingKind::Feed => world.config.news.residents_per_feed.max(1) as f32,
            BuildingKind::Club => world.config.corps.residents_per_club.max(1) as f32,
            BuildingKind::Arcade => world.config.corps.residents_per_arcade.max(1) as f32,
            BuildingKind::NoodleBar => world.config.corps.residents_per_noodle.max(1) as f32,
            BuildingKind::FightPit => world.config.corps.residents_per_pit.max(1) as f32,
            BuildingKind::Den => world.config.corps.residents_per_den.max(1) as f32,
            BuildingKind::Lounge => world.config.corps.residents_per_lounge.max(1) as f32,
            BuildingKind::Fixer => world.config.fixers.fixers_per_pop.max(1) as f32,
            BuildingKind::Mission => world.config.charity.residents_per_mission.max(1) as f32,
            _ => world.config.world.residents_per_home.max(1) as f32,
        }
    };
    let mut best: Option<(f32, BuildingKind)> = None;
    for kind in FOUNDABLE {
        let Some(cost) = agent_found_cost(world, kind) else { continue };
        if coins < cost
            || (kind == BuildingKind::Lounge && !lounge_ok)
            || (kind == BuildingKind::Fixer && !fixer)
            || (kind == BuildingKind::Mission && !mission)
        {
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
    // L2 L6: with jobs on, the NoodleBar is the cheapest foundable rung.
    let floor = c.bar.min(c.home).min(c.noodle_bar);
    if coins < floor {
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
    if is_exec(world, agent) {
        return false;
    }
    let kind = || {
        choose_kind_full(
            world,
            coins,
            lounge_ok(world, agent, coins),
            fixer_ok(world, agent),
            crate::systems::charity::founder_ok(world, agent),
        )
    };
    if any_vacant_lot(world) {
        return kind().is_some();
    }
    // L2 L6: no Lot left: only a leisure kind, refitted from a derelict.
    refit_room(world) && kind().is_some_and(|k| k.is_leisure())
}

/// L2 L6: with jobs on, a derelict Block may take a refit when no Lot is left.
fn refit_room(world: &World) -> bool {
    !crate::systems::street::derelicts(world).is_empty()
}

/// L2 L6: the Lounge is for the wealthiest: a Corp-class founder who can pay it.
fn lounge_ok(world: &World, agent: EntityId, coins: i64) -> bool {
    coins >= world.config.corps.found_cost.lounge
        && crate::systems::classes::class_of(world, agent) == crate::components::Class::Corp
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
    let kind = choose_kind_full(
        world,
        coins,
        lounge_ok(world, agent, coins),
        fixer_ok(world, agent),
        crate::systems::charity::founder_ok(world, agent),
    )
    .ok_or("nothing affordable")?;
    let cost = agent_found_cost(world, kind).ok_or("not foundable")?;
    let from = world
        .comp::<Household>(agent)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(agent).map(|p| p.tile))
        .ok_or("nowhere")?;
    // L2 L6: no Lot left (or a kind no Lot takes): a derelict Block is
    // refitted at `refit_frac` of the cost instead.
    // Review fix: a leisure kind takes only a Lot its tier allows (a
    // Lounge off the Spire priced at 0).
    let lot = if kind.is_leisure() { nearest_lot_for(world, kind, from) } else { nearest_lot(world, from) };
    let Some(lot) = lot else {
        let d = refit_ok(world, kind, from).ok_or("no vacant Lot")?;
        let b = refit(world, d, kind, Some(agent))?;
        let today = world.day();
        if let Some(br) = world.comp_mut::<Brain>(agent) {
            br.last_found_day = Some(today);
        }
        world.stats.current.foundings += 1;
        let dd = world.district_of_building(b);
        crate::systems::gossip::post_deed(world, dd, crate::word::Deed::Founded, Some(agent), Some(b));
        maybe_incorporate(world, agent);
        return Ok(b);
    };
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
    let d = world.district_of_building(lot);
    crate::systems::gossip::post_deed(world, d, crate::word::Deed::Founded, Some(agent), Some(lot));
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
