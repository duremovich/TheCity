//! M11 founding (docs/M11_OWNERSHIP.md § 6, plan D25). Phase 3 needs only
//! the construction half: a corp's `Grow` order builds on a vacant Lot.
//! Phase 4 adds the NPC side (the `Found` goal, `Register`, incorporation).

use crate::components::{Building, BuildingKind, Position, TileKind, TilePos, Zone};
use crate::entity::EntityId;
use crate::world::World;

/// Vacant Lots (kind Lot, not demolished), ascending.
pub fn vacant_lots(world: &World) -> Vec<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Lot)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lot && !bd.demolished))
        .collect()
}

/// What founding a building of `kind` costs (Bar and Home only).
pub fn found_cost(world: &World, kind: BuildingKind) -> Option<i64> {
    let c = &world.config.corps.found_cost;
    match kind {
        BuildingKind::Bar => Some(c.bar),
        BuildingKind::Home => Some(c.home),
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
    if !matches!(kind, BuildingKind::Bar | BuildingKind::Home) {
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
    let capacity = u8::try_from(interior).unwrap_or(u8::MAX).min(cfg.capacity);
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
    world.buildings_by_kind.entry(kind).or_default().push(lot);
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
    world.guarded_homes = Default::default();
    Ok(lot)
}
