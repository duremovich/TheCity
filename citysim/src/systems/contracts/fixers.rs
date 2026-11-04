use super::*;

// ---------------------------------------------------------------------------
// Fixers
// ---------------------------------------------------------------------------

/// An open Fixer's office: standing, a `Broker`, not closed by the law.
pub fn fixer_open(world: &World, b: EntityId) -> bool {
    let now = world.tick;
    world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Fixer && !bd.demolished && !bd.derelict)
        && world.comp::<Broker>(b).is_some_and(|k| k.closed_until.is_none_or(|t| t <= now))
}

/// Does `id` own a Fixer's office (open or not)?
pub fn owns_fixer(world: &World, id: EntityId) -> bool {
    world.buildings_of_kind(BuildingKind::Fixer).iter().any(|&f| world.owner_of(f) == Some(id))
}

/// Every open Fixer, ascending.
pub fn open_fixers(world: &World) -> Vec<EntityId> {
    world.buildings_of_kind(BuildingKind::Fixer).iter().copied().filter(|&b| fixer_open(world, b)).collect()
}

/// C35 (phase 4): `lod::BudgetSets.fixer`: from `match_hour − 2` to
/// midnight, the owners (living, free agents) of open Fixers by book size
/// descending (ties the lower owner id), up to `[lod] fixer_quota`; empty
/// outside the window (matching runs at any tier).
pub fn fixer_set(world: &World) -> std::collections::BTreeSet<EntityId> {
    let mut set = std::collections::BTreeSet::new();
    let quota = world.config.lod.fixer_quota;
    let from = u64::from(world.config.fixers.match_hour.saturating_sub(2)) * TICKS_PER_HOUR;
    if quota == 0 || u64::from(world.tick_of_day()) < from {
        return set;
    }
    let mut owners: Vec<(std::cmp::Reverse<usize>, EntityId)> = open_fixers(world)
        .into_iter()
        .filter_map(|f| {
            let owner = world.owner_of(f).filter(|&o| world.has::<Identity>(o))?;
            if !crate::systems::law::living(world, owner) || world.has::<Sentence>(owner) {
                return None;
            }
            Some((std::cmp::Reverse(world.comp::<Broker>(f).map_or(0, |k| k.book.len())), owner))
        })
        .collect();
    owners.sort_unstable();
    for (_, o) in owners {
        if set.len() >= quota {
            break;
        }
        set.insert(o);
    }
    set
}

/// `id`'s Home door; `None` homeless.
pub fn home_door(world: &World, id: EntityId) -> Option<TilePos> {
    world.comp::<Household>(id).and_then(|h| h.home).and_then(|h| world.comp::<Building>(h)).map(|b| b.door)
}

/// A home door, else where the agent stands.
pub(super) fn home_or_here(world: &World, id: EntityId) -> Option<TilePos> {
    home_door(world, id).or_else(|| world.comp::<Position>(id).map(|p| p.tile))
}

/// C20: the open Fixer whose door is nearest `id`'s Home door within
/// `hire_reach_tiles` (Manhattan; ties the lower id).
pub fn nearest_fixer(world: &World, id: EntityId) -> Option<EntityId> {
    let from = home_or_here(world, id)?;
    let reach = world.config.fixers.hire_reach_tiles;
    world
        .buildings_of_kind(BuildingKind::Fixer)
        .iter()
        .copied()
        .filter(|&b| fixer_open(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, b)| b)
}

/// The open Fixer nearest a tile (any distance; ties the lower id).
pub(crate) fn nearest_fixer_to(world: &World, from: TilePos) -> Option<EntityId> {
    open_fixers(world)
        .into_iter()
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// Is `id` a regular of any open Fixer whose stamp holds past `now + slack`?
pub(super) fn is_regular(world: &World, id: EntityId, slack: Tick) -> bool {
    let keep = Tick::from(world.config.fixers.regular_days) * TICKS_PER_DAY;
    let now = world.tick;
    world.buildings_of_kind(BuildingKind::Fixer).iter().any(|&f| {
        fixer_open(world, f)
            && world.comp::<Broker>(f).and_then(|k| k.regulars.get(&id)).is_some_and(|&t| t + keep > now + slack)
    })
}

/// C9: the districts that get a seeded Fixer, in order, by role (Scale S4:
/// Mid East on the shipped map), each dealt `World::seed_copies` times.
pub const SEEDED_FIXERS: [crate::systems::districts::Seat; 1] =
    [crate::systems::districts::Seat::new(crate::components::Zone::Mid, 0.75)];

/// C9 (deviation, coordinator 2026-10-08): one seeded Fixer, Mid East's,
/// on the Lots left after L2's venues (no RNG); the second is founded by an
/// NPC through `Register` under the per-capita cap (`fixers_per_pop`), so
/// Sump Central keeps its one vacant Lot for M12 splits and Registers.
/// Per district, the vacant Lot whose door
/// is nearest the district's centroid (ties the lower id), else one in an
/// adjacent district (ascending index), else a derelict refitted free
/// (the district, then the adjacent ones), else skipped and logged. The
/// owner is the jobless adult who is no exec, no gang leader and owns no
/// building, lowest lawfulness first (ties: Home door nearest the Lot's,
/// then the lower id). No coin is minted.
pub fn seed_fixers(world: &mut World) -> Vec<EntityId> {
    let mut built = Vec::new();
    let copies = world.lot_copies();
    for (copy, seat) in (0..copies).flat_map(|j| SEEDED_FIXERS.into_iter().map(move |s| (j, s))) {
        let Some(d) = crate::systems::districts::seat_district(world, seat, copy, copies).map(|d| d.index()) else {
            log(world, 0, format!("no {:?} district: Fixer not seeded", seat.zone));
            continue;
        };
        let name = world.districts[d].name.clone();
        let adjacent: Vec<usize> = {
            let mask = world.district_adjacent.get(d).copied().unwrap_or(0);
            (0..world.districts.len()).filter(|&i| i != d && mask & (1 << i) != 0).collect()
        };
        let order: Vec<usize> = std::iter::once(d).chain(adjacent).collect();
        let centroid = world.districts[d].centroid;
        let lot = order.iter().find_map(|&di| {
            crate::systems::founding::vacant_lots(world)
                .into_iter()
                .filter(|&l| world.district_of_building(l).index() == di)
                .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(centroid), l)))
                .min()
                .map(|(_, l)| l)
        });
        let derelict = || {
            order.iter().find_map(|&di| {
                crate::systems::street::derelicts(world)
                    .into_iter()
                    .filter(|&x| world.district_of_building(x).index() == di)
                    .filter_map(|x| {
                        let bd = world.comp::<Building>(x).filter(|bd| bd.kind == BuildingKind::Home)?;
                        Some((bd.door.manhattan(centroid), x))
                    })
                    .min()
                    .map(|(_, x)| x)
            })
        };
        let site = match lot {
            Some(l) => Some((l, true)),
            None => derelict().map(|x| (x, false)),
        };
        let Some((site, on_lot)) = site else {
            log(world, 0, format!("no Lot or derelict in or beside {name}: Fixer not seeded"));
            continue;
        };
        let door = world.comp::<Building>(site).map(|b| b.door).unwrap_or_default();
        let owner = seed_owner(world, door);
        let made = if on_lot {
            crate::systems::founding::build_on_lot(world, site, BuildingKind::Fixer, owner)
        } else {
            crate::systems::founding::refit_with(world, site, BuildingKind::Fixer, owner, false)
        };
        if made.is_err() {
            log(world, 0, format!("{name}: the Fixer could not be built"));
            continue;
        }
        let at = world.district_name(world.district_of_building(site)).to_string();
        let who = world.owner_label(owner);
        let how = if on_lot { "Lot" } else { "refit" };
        let text = format!("{who} opened a Fixer in {at} (seeded; {how} {})", site.index);
        let mut actors: Vec<EntityId> = owner.into_iter().collect();
        actors.push(site);
        world.push_event(EventKind::Founded, &actors, text.clone());
        log(world, 0, text);
        built.push(site);
    }
    world.fixers_seeded = !built.is_empty();
    built
}

/// C9: a seeded Fixer's owner.
fn seed_owner(world: &World, door: TilePos) -> Option<EntityId> {
    let owners: std::collections::BTreeSet<EntityId> =
        crate::systems::ownership::holdings(world).keys().flatten().copied().collect();
    let leaders: std::collections::BTreeSet<EntityId> =
        world.gangs().iter().filter_map(|&g| world.comp::<crate::components::Gang>(g).and_then(|x| x.leader)).collect();
    world
        // scan-ok: twice at seed (and at a deferred seeding).
        .citizens()
        .into_iter()
        .filter(|&a| crate::systems::demography::is_adult(world, a) && world.has::<Brain>(a))
        .filter(|&a| !world.has::<Job>(a) && !world.has::<Sentence>(a))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| !b.emigrating))
        .filter(|&a| !owners.contains(&a) && !leaders.contains(&a))
        .filter(|&a| !crate::systems::founding::is_exec(world, a))
        .filter_map(|a| {
            let l = world.comp::<Personality>(a)?.lawfulness;
            let dist = home_or_here(world, a).map_or(u32::MAX, |t| t.manhattan(door));
            Some((ordered_float::OrderedFloat(l), dist, a))
        })
        .min()
        .map(|(_, _, a)| a)
}

// ---------------------------------------------------------------------------
// Phase 3: the law and the street (C21's talk, C27's public bounty,
// C28-C34). Game state only: a heat value on a record book, a record a city
// guard holds, a rumour copied between memories by a keyed roll.
// ---------------------------------------------------------------------------

/// C21: a first-hand-style `Hired` rumour (actor `actor`, object `object`)
/// written into `holder` at `hops` and conf 1 (interrogation's naming, the
/// take's knowledge); `holder` joins record `id`'s `known_by`.
pub fn hear_named(world: &mut World, id: ContractId, holder: EntityId, actor: EntityId, object: EntityId, hops: u8) {
    if let Some(c) = world.contracts.get_mut(&id) {
        if !c.known_by.contains(&holder) {
            c.known_by.push(holder);
        }
    }
    if !world.has::<Memory>(holder) {
        return;
    }
    let sal = world.config.gossip.deed_sal.get(Deed::Hired);
    let e = MemoryEntry {
        subject: Some(actor),
        salience: sal,
        valence: -world.config.gossip.deed_sev.get(Deed::Hired) * sal,
        second_hand: hops > 0,
        deed: Some(Deed::Hired),
        object: Some(object),
        hops,
        conf: 1.0,
        ..MemoryEntry::blank(MemoryKind::Rumour, world.tick)
    };
    crate::systems::memory::hear_entry(world, holder, e);
}

/// C21, the Fixer's talk (from `gossip::exchange` at `Venue::Drink`, after
/// the ordinary exchange): when `from` owns an open Fixer and `to` is one of
/// its regulars, each `Hired` rumour `from` holds passes with `p =
/// fixer_talk × (1 − heat)`, one draw per rumour on `ContractNs::Talk`
/// (key `tick, from << 32 | to`). The listener's copy is a telling as the
/// exchange writes one (hops + 1, salience × `hop_salience`, conf × (0.5 +
/// 0.5 × trust in the speaker)).
pub fn fixer_talk(world: &mut World, from: EntityId, to: EntityId) {
    if from == to {
        return;
    }
    let office = world.buildings_of_kind(BuildingKind::Fixer).iter().copied().find(|&f| {
        world.owner_of(f) == Some(from)
            && fixer_open(world, f)
            && world.comp::<Broker>(f).is_some_and(|k| k.regulars.contains_key(&to))
    });
    let Some(f) = office else { return };
    let heat = world.comp::<Broker>(f).map_or(0.0, |k| k.heat);
    let p = world.config.law.fixer_talk * (1.0 - heat);
    if p <= 0.0 {
        return;
    }
    let Some(ms) = world.comp::<Memory>(from) else { return };
    let held: Vec<MemoryEntry> = crate::systems::memory::deeds(from, ms)
        .filter(|(_, r)| r.deed == Deed::Hired && r.actor != Some(to))
        .map(|(e, _)| e.clone())
        .collect();
    if held.is_empty() || !world.has::<Memory>(to) {
        return;
    }
    let trust = world.edge(to, from).map_or(0.0, |e| e.trust);
    let hop_salience = world.config.gossip.hop_salience;
    let now = world.tick;
    let mut rng = world.rng.contract(ContractNs::Talk, now, (u64::from(from.index) << 32) | u64::from(to.index));
    for src in held {
        if rng.random::<f32>() >= p {
            continue;
        }
        let entry = MemoryEntry {
            subject: src.subject,
            salience: src.salience * hop_salience,
            valence: src.valence * hop_salience,
            second_hand: true,
            deed: Some(Deed::Hired),
            object: src.object,
            hops: crate::systems::memory::hops_of(&src).saturating_add(1),
            conf: src.conf * (0.5 + 0.5 * trust),
            ..MemoryEntry::blank(MemoryKind::Rumour, src.tick)
        };
        crate::systems::memory::hear_entry(world, to, entry);
    }
}

/// C29: does `holder` hold a `Hired` rumour about record `c` (object its
/// target, actor its buyer or placing agent) at `conf ≥ accessory_conf`?
/// What the accessory rule reads: nothing here reads `known_by`.
pub fn holds_hired(world: &World, holder: EntityId, c: &Contract) -> bool {
    let Some(m) = world.comp::<Memory>(holder) else { return false };
    let min = world.config.law.accessory_conf;
    let target = c.target.id();
    crate::systems::memory::deeds(holder, m).any(|(e, r)| {
        r.deed == Deed::Hired
            && r.object == Some(target)
            && r.actor.is_some()
            && (r.actor == c.buyer || r.actor == c.agent)
            && e.conf >= min
    })
}
