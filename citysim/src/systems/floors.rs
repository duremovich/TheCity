//! The city milestone C6a (docs/CITY_MILESTONE.md § C6a): floors as tenancies, the store's half.
//!
//! A cut of the imported city stays one structure on the tile map, its **host** (the cut's main use, a
//! `Building` with `within: None`); each other use on its floors is a **tenancy**, a `Building` with `within:
//! Some(host)`, the host's rect, its own kind, owner, rent, capacity and door, and a `span` of storeys. Every
//! system keyed on a building keeps working: a tenancy is a building. What is new is where on the structure a body
//! stands: `Position.floor`, set on entry (`floor_in`: a resident at his flat's storey, a worker on one of his
//! post's floors, a visitor at the span's lowest), and a household's flat (`Household.flat`, a unit of its Home's
//! span: one of the Home's household slots, its storey `flat_floor`).
//!
//! Vacant units are not entities: a host's units (the map's `units=`) less the uses standing on it are
//! [`vacant_units`], a pure function C6b's founding materialises.
//!
//! Statistical mirror: the tenancy is the agent's building as before; the floor is drawn from it on promotion
//! (the promoted body enters through `enter_building`, which sets it).
//!
//! The 2k city (`mapgen`) has no spans: every floor reads 0 and no flat is assigned.

use crate::components::{Building, BuildingKind, Household, Job, Position, RelKind};
use crate::entity::EntityId;
use crate::world::World;

/// The structure `b` stands in: its host (`within`, followed to the top: company floors inside a tenancy), else
/// `b` itself.
pub fn host_of(world: &World, b: EntityId) -> EntityId {
    let mut at = b;
    for _ in 0..4 {
        match world.comp::<Building>(at).and_then(|bd| bd.within) {
            Some(h) if h != at => at = h,
            _ => break,
        }
    }
    at
}

/// The storey of flat `flat` of Home `home` (`b` its building): the flats dealt evenly up its span from a storey
/// fixed per Home (a hash of its id), wrapping, so a tower whose few sim households hold one or two flats has them
/// somewhere up the tower, not all on its lowest flat floor (the rest of its flats are the Ambient tier's); 0
/// without a span.
pub fn flat_floor(b: &Building, home: EntityId, flat: u16) -> u8 {
    let Some((lo, hi)) = b.span else { return 0 };
    let slots = u32::from(b.floors.max(1));
    let n = u32::from(hi - lo) + 1;
    let f = u32::from(flat).min(slots - 1);
    let offset = home.index.wrapping_mul(0x9E37_79B9) >> 16;
    let k = (f * n / slots + offset) % n;
    lo.saturating_add(u8::try_from(k).unwrap_or(u8::MAX)).min(hi)
}

/// `a`'s flat in his current Home, if one is assigned (a flat naming another Home is stale: `None`).
pub fn flat_of(world: &World, a: EntityId) -> Option<u16> {
    let h = world.comp::<Household>(a)?;
    match (h.home, h.flat) {
        (Some(home), Some((fh, f))) if fh == home => Some(f),
        _ => None,
    }
}

/// The storey `a` stands on inside `b`: at his own Home his flat's; at his post one of its floors (fixed per
/// body); else (a visitor, a customer) the span's lowest. 0 for a building with no span.
pub fn floor_in(world: &World, a: EntityId, b: EntityId) -> u8 {
    let Some(bd) = world.comp::<Building>(b) else { return 0 };
    let Some((lo, hi)) = bd.span else { return 0 };
    if world.comp::<Household>(a).and_then(|h| h.home) == Some(b) {
        if let Some(f) = flat_of(world, a) {
            return flat_floor(bd, b, f);
        }
    }
    if world.comp::<Job>(a).and_then(|j| j.employer) == Some(b) {
        let n = u32::from(hi - lo) + 1;
        return lo + u8::try_from(a.index % n).unwrap_or(0);
    }
    lo
}

/// Set `a`'s `Position.floor` from the building he stands in (0 on the street).
pub fn settle(world: &mut World, a: EntityId) {
    let Some(b) = world.comp::<Position>(a).map(|p| p.building) else { return };
    let f = b.map_or(0, |b| floor_in(world, a, b));
    if let Some(p) = world.comp_mut::<Position>(a) {
        p.floor = f;
    }
}

/// Give `a` a flat in his Home when it has a span and he has none there: his kin's under this roof (a spouse, a
/// parent, family), else the lowest flat with a bed free (a slot holds the Home's seats a floor), else the least
/// crowded. A Home without a span (the 2k city) deals no flats.
pub fn assign_flat(world: &mut World, a: EntityId) {
    let Some(home) = world.comp::<Household>(a).and_then(|h| h.home) else { return };
    if flat_of(world, a).is_some() {
        return;
    }
    let Some(b) = world.comp::<Building>(home).filter(|b| b.span.is_some() && b.kind == BuildingKind::Home) else {
        return;
    };
    let slots = usize::from(b.floors.max(1));
    let per = (usize::from(b.capacity) / slots).max(1);
    let residents = world.residents_of(home);
    let kin = world
        .kin_candidates(a)
        .filter(|&o| {
            world.edge(a, o).is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family))
        })
        .chain(world.spouse_of(a))
        .filter(|o| residents.binary_search(o).is_ok())
        .find_map(|o| flat_of(world, o));
    let flat = kin.unwrap_or_else(|| {
        let mut n = vec![0usize; slots];
        for &r in residents {
            if r != a {
                if let Some(f) = flat_of(world, r) {
                    n[usize::from(f).min(slots - 1)] += 1;
                }
            }
        }
        let j = n
            .iter()
            .position(|&c| c < per)
            .unwrap_or_else(|| n.iter().enumerate().min_by_key(|&(j, &c)| (c, j)).map_or(0, |(j, _)| j));
        u16::try_from(j).unwrap_or(u16::MAX)
    });
    if let Some(h) = world.comp_mut::<Household>(a) {
        h.flat = Some((home, flat));
    }
}

/// A newborn at home: his mother's flat when she lives there, his storey.
pub fn born(world: &mut World, child: EntityId, mother: EntityId) {
    let home = world.comp::<Household>(child).and_then(|h| h.home);
    let theirs = world.comp::<Household>(mother).filter(|h| h.home == home).and(flat_of(world, mother));
    if let (Some(home), Some(f)) = (home, theirs) {
        if let Some(h) = world.comp_mut::<Household>(child) {
            h.flat = Some((home, f));
        }
    }
    settle(world, child);
}

/// `World::new`'s last pass: the households' flats dealt afresh in ascending id once the kin are linked (the
/// seeding's `set_home`s ran before the couples and children were), then every body indoors set on its storey.
/// Nothing on a map without spans.
pub fn seed(world: &mut World) {
    if !world.with::<Building>().into_iter().any(|b| world.comp::<Building>(b).is_some_and(|bd| bd.span.is_some())) {
        return;
    }
    let mut all: Vec<EntityId> = world.with::<Household>();
    all.sort_unstable();
    for &a in &all {
        if let Some(h) = world.comp_mut::<Household>(a) {
            h.flat = None;
        }
    }
    for &a in &all {
        assign_flat(world, a);
    }
    let bodies: Vec<EntityId> = world.with::<Position>();
    for a in bodies {
        if world.comp::<Position>(a).is_some_and(|p| p.building.is_some()) {
            settle(world, a);
        }
    }
}

/// The tenancies standing on `host` (`within` it), ascending. A scan of the store: for the view and the reads,
/// not a tick's hot loop.
pub fn tenancies_of(world: &World, host: EntityId) -> Vec<EntityId> {
    world
        .with::<Building>()
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.within == Some(host)))
        .collect()
}

/// The work units on `host` no use stands on: the map's `units=` (shops, workshops and offices, a works floor
/// one) less the host itself when it is a workplace and each of its tenancies but company floors. C6b's founding
/// takes from these (Ambient's rule: not entities until used).
pub fn vacant_units(world: &World, host: EntityId) -> u32 {
    let Some(units) = world.comp::<Building>(host).map(|b| u32::from(b.units)) else { return 0 };
    let used = |b: EntityId| world.comp::<Building>(b).is_some_and(|bd| bd.kind != BuildingKind::Home);
    let mut n = u32::from(used(host));
    n += tenancies_of(world, host).into_iter().filter(|&t| used(t) && !world.company_homes.contains_key(&t)).count()
        as u32;
    units.saturating_sub(n)
}

/// A building's floors for a line: "floor 3", "floors 2-5", or "" without a span.
pub fn span_text(world: &World, b: EntityId) -> String {
    match world.comp::<Building>(b).and_then(|bd| bd.span) {
        Some((lo, hi)) if lo == hi => format!("floor {lo}"),
        Some((lo, hi)) => format!("floors {lo}-{hi}"),
        None => String::new(),
    }
}
