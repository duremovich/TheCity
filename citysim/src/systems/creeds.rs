//! M15 § 2 "The Purist creed" (plan phase 2, W31): a gang's creed and The
//! Unplugged.
//!
//! A creed is a tag on a gang (`Gang.creed`). Under the Purist tolerance
//! rule an agent with `Kit.visible ≤ creed_tolerance` is tolerated; above
//! it a Purist gang refuses them as a recruit, casts out a member who
//! installs chrome, shakes down their Homes first, buys no chrome, decks or
//! robots, breaks what it rips into Parts for the Recycler, and tithes at
//! `tithe_frac × extort_amount`. One Purist gang, The Unplugged, is seeded
//! on a Sump Central derelict with no members; it recruits like any gang.
//! All of it is game state of fictional agents.

use crate::components::{Building, BuildingKind, Gang, Kit, Personality};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::word::Creed;
use crate::world::World;

/// The gang's creed, if any.
pub fn creed_of(world: &World, gang: EntityId) -> Option<Creed> {
    world.comp::<Gang>(gang).and_then(|g| g.creed)
}

/// Is the gang Purist?
pub fn is_purist(world: &World, gang: EntityId) -> bool {
    creed_of(world, gang) == Some(Creed::Purist)
}

/// Jobs & lives phase 6 (§ 8): a cult's `[faith] creeds` row (none with
/// `[faith]` off: no gang holds a `Creed::Config` then).
fn cult_row(world: &World, gang: EntityId) -> Option<&crate::config::CreedCfg> {
    match creed_of(world, gang)? {
        Creed::Config(i) => world.config.faith.creeds.get(usize::from(i)),
        Creed::Purist => None,
    }
}

/// `Kit.visible` (0 without a Kit).
fn visible(world: &World, id: EntityId) -> u8 {
    world.comp::<Kit>(id).map_or(0, |k| k.visible)
}

/// Is `id` above the Purist tolerance (`Kit.visible > creed_tolerance`)?
pub fn chromed(world: &World, id: EntityId) -> bool {
    visible(world, id) > world.config.creeds.creed_tolerance
}

/// Shadow V4 bug 13: the visible chrome tiers `id` may still take and keep
/// its creed gang (a Purist's `creed_tolerance`, a cult row's `tolerance`,
/// less what shows); `None` outside a creed gang (Purists and cultists were
/// sold the chrome that expelled them the same minute).
pub fn visible_room(world: &World, id: EntityId) -> Option<u8> {
    let gang = world.gang_of(id)?;
    let tolerance = match cult_row(world, gang) {
        Some(row) => row.tolerance,
        None if is_purist(world, gang) => world.config.creeds.creed_tolerance,
        None => return None,
    };
    Some(tolerance.saturating_sub(visible(world, id)))
}

/// W31: does `gang` tolerate `id`? Every gang does, a Purist gang only
/// those within `creed_tolerance`.
pub fn tolerated(world: &World, gang: EntityId, id: EntityId) -> bool {
    if let Some(row) = cult_row(world, gang) {
        return visible(world, id) <= row.tolerance;
    }
    !is_purist(world, gang) || !chromed(world, id)
}

/// Phase 2 review: may `gang` take `id` through the edge to `member`? Any
/// gang: yes; a Purist gang only along a Friend, Family, Parent or Spouse
/// edge (a creed spreads through people).
pub fn edge_recruits(world: &World, gang: EntityId, id: EntityId, member: EntityId) -> bool {
    use crate::components::RelKind;
    let edges = match cult_row(world, gang) {
        Some(row) => row.recruit_edges,
        None => is_purist(world, gang),
    };
    !edges
        || world
            .edge(id, member)
            .is_some_and(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Parent | RelKind::Spouse))
}

/// Phase 2 review: may `gang` take a bootstrap or desperate recruit by its
/// Hideout's nearness? Any gang: yes; a Purist gang only while it has fewer
/// than `[creeds] founders` members (the Chapel's founders).
pub fn open_to_strangers(world: &World, gang: EntityId) -> bool {
    if let Some(row) = cult_row(world, gang) {
        return !row.recruit_edges || world.comp::<Gang>(gang).is_some_and(|g| g.members.len() < row.founders);
    }
    !is_purist(world, gang) || world.comp::<Gang>(gang).is_some_and(|g| g.members.len() < world.config.creeds.founders)
}

/// W31 (`World::new`, when `[gossip] enabled` and `[creeds] seed_purist`):
/// the derelict Block in Sump Central nearest its centroid becomes a
/// Hideout labelled "Chapel", held by The Unplugged (`purist_treasury`,
/// creed Purist, no members). No derelict there: the vacant Lot nearest
/// the centroid, built as a Hideout. Neither: no Unplugged. No RNG.
/// Scale S4: Sump Central by role (`districts::Seat`, the Sump's middle),
/// and a chapel per `World::seed_copies` ("The Unplugged 2", ...), each its own
/// Purist gang. Returns the first.
pub fn seed_unplugged(world: &mut World) -> Option<EntityId> {
    if !world.config.creeds.seed_purist {
        return None;
    }
    if world.gang_list().iter().any(|&g| is_purist(world, g)) {
        return None;
    }
    let copies = world.seed_copies();
    let mut first = None;
    for copy in 0..copies {
        if let Some(g) = seed_chapel(world, copy, copies) {
            first.get_or_insert(g);
        }
    }
    first
}

/// One `seed_unplugged` chapel.
fn seed_chapel(world: &mut World, copy: usize, copies: usize) -> Option<EntityId> {
    let cfg = world.config.creeds.clone();
    let name = if copy == 0 { cfg.purist_name.clone() } else { format!("{} {}", cfg.purist_name, copy + 1) };
    let seat = crate::systems::districts::Seat::new(crate::components::Zone::Sump, 0.5);
    seed_chapel_as(world, seat, name, Creed::Purist, cfg.purist_treasury, "Chapel", copy, copies)
}

/// Jobs & lives phase 6 (§ 8): a chapel for a creed's gang: the derelict
/// Block nearest the seat district's centroid (else its nearest vacant Lot,
/// built as a Hideout) becomes the Hideout of a new memberless gang `name`
/// with `creed` and `treasury`, labelled `label`. No RNG.
#[allow(clippy::too_many_arguments)]
pub(crate) fn seed_chapel_as(
    world: &mut World,
    seat: crate::systems::districts::Seat,
    name: String,
    creed: Creed,
    treasury: i64,
    label: &str,
    copy: usize,
    copies: usize,
) -> Option<EntityId> {
    let d = crate::systems::districts::seat_district(world, seat, copy, copies)?;
    let centroid = world.district(d).centroid;
    let derelict = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter_map(|h| world.comp::<Building>(h).filter(|b| b.derelict && !b.demolished).map(|b| (b.door, h)))
        .filter(|&(door, _)| world.district_of(door) == d)
        .map(|(door, h)| (door.manhattan(centroid), h))
        .min()
        .map(|(_, h)| h);
    let chapel = match derelict {
        Some(b) => {
            let gang = world.spawn();
            world.insert(gang, Gang::new(name, b, treasury));
            crate::systems::gang::convert_to_hideout(world, gang, b);
            (gang, b)
        }
        None => {
            let lot = world
                .buildings_of_kind(BuildingKind::Lot)
                .iter()
                .copied()
                .filter_map(|l| world.comp::<Building>(l).filter(|b| !b.demolished).map(|b| (b.door, l)))
                .filter(|&(door, _)| world.district_of(door) == d)
                .map(|(door, l)| (door.manhattan(centroid), l))
                .min()
                .map(|(_, l)| l)?;
            let b = crate::systems::founding::build_on_lot(world, lot, BuildingKind::Hideout, None).ok()?;
            let gang = world.spawn();
            world.insert(gang, Gang::new(name, b, treasury));
            (gang, b)
        }
    };
    let (gang, b) = chapel;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.creed = Some(creed);
    }
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.label = Some(label.to_string());
    }
    // As every seeded Hideout (`ownership::seed`): it belongs to its gang.
    crate::systems::ownership::transfer_building(world, b, Some(gang));
    crate::systems::virt::mark_dirty(world);
    Some(gang)
}

/// W31: cast `id` out of its gang for chrome (`Expelled` event, `LeftGang`).
fn expel(world: &mut World, id: EntityId, gang: EntityId) {
    let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    crate::systems::gang::leave(world, id, "expelled");
    let name = world.name_of(id);
    world.push_event(EventKind::Expelled, &[id, gang], format!("{gname} cast out {name} for chrome"));
    world.stats.current.word.expelled += 1;
}

/// W31 (M13's install completion, `chrome::installed_now`): a Purist
/// member now above tolerance is expelled.
pub fn on_install(world: &mut World, id: EntityId) {
    let Some(gang) = world.gang_of(id) else { return };
    // Jobs & lives phase 6: a cult casts out on its row's tolerance too.
    let cult_out = cult_row(world, gang).is_some_and(|r| visible(world, id) > r.tolerance);
    if (is_purist(world, gang) && chromed(world, id)) || cult_out {
        expel(world, id, gang);
    }
}

/// W31: every member of a Purist gang above tolerance is expelled at once
/// (god `SetCreed`, a Purist splinter).
pub fn enforce(world: &mut World, gang: EntityId) {
    if !is_purist(world, gang) {
        return;
    }
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    for m in members {
        if chromed(world, m) {
            expel(world, m, gang);
        }
    }
}

/// Spec § 2: a splinter gains the creed when its lieutenant shows no
/// chrome (`Kit.visible == 0`) and has `lawfulness ≥ 0.5` (with the word on).
pub fn splinter_creed(world: &mut World, splinter: EntityId, lieutenant: EntityId) {
    let lawful = world.comp::<Personality>(lieutenant).is_some_and(|p| p.lawfulness >= 0.5);
    if visible(world, lieutenant) == 0 && lawful {
        if let Some(g) = world.comp_mut::<Gang>(splinter) {
            g.creed = Some(Creed::Purist);
        }
        enforce(world, splinter);
    }
}

/// Does a Home hold a resident above the Purist tolerance (the Purist
/// Shakedown's first pick)?
pub fn chromed_home(world: &World, home: EntityId) -> bool {
    world.residents_of(home).iter().any(|&r| chromed(world, r))
}
