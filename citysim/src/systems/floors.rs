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
//!
//! **C6b, the economy of floors** (`[tenancy]`; absent, none of it): the host's owner is the **landlord**. Each
//! tenancy pays him floor rent at midnight ([`rent_pass`], `Flow::FloorRent`; a city-owned host's to the Treasury),
//! `floor_rent[tier]` a storey of its span (x a corp landlord's Housing level, the Home rent's rule) plus
//! `revenue_share` of its last seven days' mean revenue; a corp or the city pays in full (as upkeep), a person or a
//! gang from what it holds. `evict_days` short in a row and the landlord takes the floor back ([`vacate`]): a
//! business of a `unit_kinds` kind goes back to a vacant unit (a tenancy Lot), a Home's lease (and any other kind)
//! passes to the landlord with whoever lives or works there; a gang's Hideout is never put out (nobody evicts a
//! gang: its arrears stand; the city milestone C9b: its landlord answers, `vertical::landlord_pass`, run at the
//! end of the rent pass, and a debt he swallowed is rent 0). **Founding takes a unit** ([`unit_near`], [`found_unit`]): the founder's Register
//! takes a vacant unit within `found_reach_ticks` of his Home door's walk first, then a Lot. **A buyout leases
//! floors** (`company::buyouts`): the top storeys of a Home host as a Home tenancy of company bunks, the lease paid
//! once, the floors then on floor rent. **Hours** ([`hours_of`]): a tenancy's own `Building.hours`, else its kind's
//! row; the view lights a unit's sign while it is open.
//!
//! Statistical mirror: rent is a daily pass over the buildings, blind to tier; a founder is a body (an agent who can
//! found is promoted, `lod`'s rule), so a founding into a unit is as a founding on a Lot on either tier.

use crate::components::{Building, BuildingKind, Corp, Gang, Household, Job, Niche, Position, RelKind, TilePos};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::world::World;
use std::collections::BTreeMap;

/// C6b: the tenancies by host and the vacant units (`World.floor_index`, not saved; [`reindex`]).
#[derive(Clone, Debug, Default)]
pub struct FloorIndex {
    /// The index has been built (before: a scan).
    pub built: bool,
    /// Host -> its tenancies, ascending.
    pub tenancies: BTreeMap<EntityId, Vec<EntityId>>,
    /// Host -> its vacant units (only hosts with one).
    pub vacant: BTreeMap<EntityId, u32>,
    /// The city's vacant units.
    pub total: u32,
    /// A unit founded this run -> the tick (the view lights it a day; kept across a reindex, not saved).
    pub founded: BTreeMap<EntityId, crate::time::Tick>,
}

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
    reindex(world);
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

/// The tenancies standing on `host` (`within` it), ascending: the index (C6b), a scan of the store before it is
/// built.
pub fn tenancies_of(world: &World, host: EntityId) -> Vec<EntityId> {
    if world.floor_index.built {
        return world.floor_index.tenancies.get(&host).cloned().unwrap_or_default();
    }
    world
        .with::<Building>()
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.within == Some(host)))
        .collect()
}

/// The work units on `host` no use stands on: the map's `units=` (shops, workshops and offices, a works floor
/// one) less the host itself when it is a workplace and each standing tenancy but company floors (C6b: a Home
/// founded on a unit's Lot holds its unit). Founding takes from these (Ambient's rule: not entities until used).
pub fn vacant_units(world: &World, host: EntityId) -> u32 {
    let Some(units) = world.comp::<Building>(host).map(|b| u32::from(b.units)) else { return 0 };
    let mut n = u32::from(world.comp::<Building>(host).is_some_and(|bd| bd.kind != BuildingKind::Home));
    n += tenancies_of(world, host)
        .into_iter()
        .filter(|&t| world.comp::<Building>(t).is_some_and(|b| !b.demolished) && !world.company_homes.contains_key(&t))
        .count() as u32;
    units.saturating_sub(n)
}

/// C6b: rebuild `World.floor_index` (the seed, the load, every founding into a unit, floor vacated or lease).
pub fn reindex(world: &mut World) {
    let mut tenancies: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    let mut hosts: Vec<EntityId> = Vec::new();
    for b in world.with::<Building>() {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if let Some(h) = bd.within {
            tenancies.entry(h).or_default().push(b);
        }
        if bd.units > 0 {
            hosts.push(b);
        }
    }
    for v in tenancies.values_mut() {
        v.sort_unstable();
    }
    let founded = std::mem::take(&mut world.floor_index.founded);
    world.floor_index = FloorIndex { built: true, tenancies, vacant: BTreeMap::new(), total: 0, founded };
    let mut vacant = BTreeMap::new();
    let mut total = 0u32;
    for h in hosts {
        let ok = world.comp::<Building>(h).is_some_and(|b| !b.demolished && !b.derelict && b.within.is_none());
        let n = if ok { vacant_units(world, h) } else { 0 };
        if n > 0 {
            vacant.insert(h, n);
            total += n;
        }
    }
    world.floor_index.vacant = vacant;
    world.floor_index.total = total;
}

/// C6b: `[tenancy]` is on.
pub fn on(world: &World) -> bool {
    world.config.tenancy.is_some()
}

/// C6b: any vacant unit in the city (O(1): `can_found` runs per think).
pub fn any_vacant(world: &World) -> bool {
    on(world) && world.floor_index.total > 0
}

/// C6b: may a unit take a business of `kind` (`[tenancy] unit_kinds`)?
pub fn unit_kind(world: &World, kind: BuildingKind) -> bool {
    world.config.tenancy.as_ref().is_some_and(|c| c.unit_kinds.contains(&kind))
}

/// C6b: the host with a vacant unit for `kind` nearest `from` by the walk (`distance::trip_by`, no El), within
/// `[tenancy] found_reach_ticks`, its door's tier allowing a leisure kind (`founding::tier_ok`); ties lower id.
/// The host and the walk.
pub fn unit_near(world: &World, kind: BuildingKind, from: TilePos) -> Option<(EntityId, u64)> {
    let cfg = world.config.tenancy.as_ref()?;
    if !cfg.unit_kinds.contains(&kind) {
        return None;
    }
    world
        .floor_index
        .vacant
        .keys()
        .filter_map(|&h| {
            let b = world.comp::<Building>(h)?;
            if b.demolished || b.derelict || b.under_construction_until.is_some() {
                return None;
            }
            let tier = crate::systems::founding::door_tier(world, b.door);
            if kind.is_leisure() && !crate::systems::founding::tier_ok(kind, tier) {
                return None;
            }
            let t = crate::systems::distance::trip_by(world, from, b.door, || false, None);
            (t <= cfg.found_reach_ticks).then_some((t, h))
        })
        .min()
        .map(|(t, h)| (h, t))
}

/// C6b: the storey a new unit on `host` takes: on a Home whose flats start above the ground, the shop storeys
/// under them (the least taken, lowest first: two shops may share a ground floor); else the lowest storey no use
/// stands on (an office stack's next floor up), past a Home's flats.
pub fn unit_floor(world: &World, host: EntityId) -> u8 {
    let Some(hb) = world.comp::<Building>(host) else { return 0 };
    let mut taken: BTreeMap<u8, u32> = BTreeMap::new();
    let mut mark = |span: Option<(u8, u8)>| {
        if let Some((lo, hi)) = span {
            for f in lo..=hi {
                *taken.entry(f).or_insert(0) += 1;
            }
        }
    };
    for t in tenancies_of(world, host) {
        if let Some(b) = world.comp::<Building>(t).filter(|b| !b.demolished) {
            mark(b.span);
        }
    }
    let home = hb.kind == BuildingKind::Home;
    match hb.span {
        Some((lo, _)) if home && lo > 0 => {
            (0..lo).min_by_key(|f| (taken.get(f).copied().unwrap_or(0), *f)).unwrap_or(0)
        }
        span => {
            if !home {
                mark(span);
            }
            let flats = if home { span } else { None };
            (0..=u8::MAX)
                .find(|f| !taken.contains_key(f) && flats.is_none_or(|(lo, hi)| *f < lo || *f > hi))
                .unwrap_or(0)
        }
    }
}

/// C6b: a business of `kind` owned by `owner` founded into a vacant unit of `host`: a new tenancy (`within` the
/// host, its rect and door, one storey, [`unit_floor`]) built as a Lot is (`founding::convert`: the kind, the
/// seats, the staff posted as vacancies, the venue's price, the districts), its hours its kind's. The caller pays.
pub fn found_unit(world: &mut World, host: EntityId, kind: BuildingKind, owner: Option<EntityId>) -> Option<EntityId> {
    let (rect, door, tier) = world.comp::<Building>(host).map(|b| (b.rect, b.door, b.tier))?;
    let f = unit_floor(world, host);
    let mut b = Building::blank(BuildingKind::Lot, rect, door, tier);
    b.within = Some(host);
    b.span = Some((f, f));
    let id = world.spawn();
    world.insert(id, b);
    let list = world.buildings_by_kind_mut().entry(BuildingKind::Lot).or_default();
    if let Err(i) = list.binary_search(&id) {
        list.insert(i, id);
    }
    crate::systems::founding::convert(world, id, kind, owner, tier);
    reindex(world);
    let now = world.tick;
    world.floor_index.founded.insert(id, now);
    ownership::jobs2_row(world).unit_foundings += 1;
    Some(id)
}

/// C6b: a tenancy's floor rent a day (0: a host, a vacant unit, one its landlord owns, a derelict).
pub fn floor_rent_for(world: &World, t: EntityId) -> i64 {
    let Some(cfg) = world.config.tenancy.as_ref() else { return 0 };
    let Some(b) = world.comp::<Building>(t) else { return 0 };
    let Some(host) = b.within else { return 0 };
    if b.demolished || b.derelict || b.kind == BuildingKind::Lot {
        return 0;
    }
    let landlord = world.owner_of(host);
    // C9b: a gang's floor rent its landlord swallowed as protection.
    if landlord == b.owner || crate::systems::vertical::rent_waived(world, t) {
        return 0;
    }
    let storeys = b.span.map_or(1, |(lo, hi)| i64::from(hi.saturating_sub(lo)) + 1);
    let base = cfg.floor_rent[usize::from(b.tier.min(2))] * storeys;
    // The Home rent's rule: a corp landlord's Housing level.
    let base = match landlord.and_then(|l| world.comp::<Corp>(l)) {
        Some(c) => (c.level(Niche::Housing) * base as f32).round() as i64,
        None => base,
    };
    let mean = if b.revenue.is_empty() { 0.0 } else { b.revenue.iter().sum::<i64>() as f32 / b.revenue.len() as f32 };
    (base + (cfg.revenue_share * mean.max(0.0)).round() as i64).max(0)
}

/// C6b, the midnight rent pass (`ownership::run`, before the upkeep): every tenancy's floor rent set and paid to
/// its landlord. A corp or the city pays in full; a person or a gang what it holds, and a short day is a day of
/// arrears (a `Floor` line on the first); `evict_days` of them and the floor is taken back ([`vacate`]), but a
/// gang's. Building-level, blind to tier.
pub fn rent_pass(world: &mut World) {
    let Some(evict_days) = world.config.tenancy.as_ref().map(|c| c.evict_days) else { return };
    // The day's hosts gone derelict or demolished drop their vacant units.
    reindex(world);
    let all: Vec<EntityId> = world.floor_index.tenancies.values().flatten().copied().collect();
    let mut evict = Vec::new();
    for t in all {
        let rent = floor_rent_for(world, t);
        let Some(bd) = world.comp_mut::<Building>(t) else { continue };
        bd.floor_rent = rent;
        if rent <= 0 {
            bd.floor_arrears = 0;
            continue;
        }
        let (payer, host) = (bd.owner, bd.within);
        let landlord = host.and_then(|h| world.owner_of(h));
        // A corp or the city pays in full, as upkeep (`charge`); a person or a gang what it holds.
        let full = payer.is_none_or(|p| world.has::<Corp>(p));
        let paid = if full {
            ownership::charge(world, payer, landlord, rent, Flow::FloorRent)
        } else {
            ownership::pay(world, payer, landlord, rent, Flow::FloorRent)
        };
        let short = paid < rent;
        let arrears = {
            let Some(bd) = world.comp_mut::<Building>(t) else { continue };
            bd.floor_arrears = if short { bd.floor_arrears.saturating_add(1) } else { 0 };
            bd.floor_arrears
        };
        if !short {
            continue;
        }
        ownership::jobs2_row(world).floor_short += 1;
        let gang = payer.is_some_and(|p| world.has::<Gang>(p));
        if arrears == 1 {
            let (who, what, whom) = (world.owner_label(payer), world.name_of(t), world.owner_label(landlord));
            let on = host.map(|h| world.name_of(h)).unwrap_or_default();
            let floors = span_text(world, t);
            let mut actors: Vec<EntityId> = payer.into_iter().collect();
            actors.push(t);
            actors.extend(host);
            world.push_event(
                EventKind::Floor,
                &actors,
                format!(
                    "{who} came up short on the floor rent of {what} ({floors}, {on}) to {whom}: paid {paid} of {rent}"
                ),
            );
        }
        if arrears >= evict_days && !gang {
            evict.push(t);
        }
    }
    for t in evict {
        vacate(world, t, "the floor rent unpaid");
    }
    let total = world.floor_index.total;
    ownership::jobs2_row(world).units_vacant = total;
    // C9b: the landlord's answer to a gang that will not pay.
    crate::systems::vertical::landlord_pass(world);
}

/// C6b: the landlord takes tenancy `t` back. A business of a `[tenancy] unit_kinds` kind goes back to a vacant
/// unit: everyone inside put out at the door, the staff let go, its vacancies and its security contract ended, a
/// tenancy Lot owned by nobody (founding's Lot path and a corp's Grow build on it). Any other kind (a Home's lease,
/// a Mission, a Fixer) passes to the landlord with whoever lives or works there: no household is put out (no new
/// harm route). A `Floor` line either way.
pub fn vacate(world: &mut World, t: EntityId, why: &str) {
    let Some((kind, owner, host, tier)) = world.comp::<Building>(t).map(|b| (b.kind, b.owner, b.within, b.tier)) else {
        return;
    };
    let Some(host) = host else { return };
    let landlord = world.owner_of(host);
    let (what, on, who, whom) =
        (world.name_of(t), world.name_of(host), world.owner_label(owner), world.owner_label(landlord));
    let floors = span_text(world, t);
    let mut actors: Vec<EntityId> = owner.into_iter().collect();
    actors.extend([t, host]);
    actors.extend(landlord);
    if !unit_kind(world, kind) {
        crate::systems::corps::move_building(world, t, landlord);
        if let Some(b) = world.comp_mut::<Building>(t) {
            b.floor_arrears = 0;
            b.floor_rent = 0;
        }
        if kind == BuildingKind::Home {
            let r = ownership::rent_for(world, t);
            if let Some(b) = world.comp_mut::<Building>(t) {
                b.rent_per_day = r;
            }
        }
        world.push_event(
            EventKind::Floor,
            &actors,
            format!("{whom} took back the lease of {what} ({floors}, {on}) from {who} ({why}): it is {whom}'s now"),
        );
        ownership::jobs2_row(world).floor_taken += 1;
        return;
    }
    crate::systems::street::empty_building(world, t);
    let staff = ownership::staff_at(world, t);
    for s in staff {
        let text = format!("{} let go: {what} lost its floor ({why})", world.name_of(s));
        crate::systems::economy::dismiss(world, s, Some(t), text);
    }
    world.vacancies.remove(&t);
    crate::systems::corps::end_contract(world, t, "the floor is vacant");
    world.underwater.shut.remove(&t);
    ownership::transfer_building(world, t, None);
    if let Some(b) = world.comp_mut::<Building>(t) {
        b.venue = None;
        b.label = None;
        b.stock_food = 0;
        b.synth_food = 0;
        b.stock_goods = [0; 2];
        b.stock_x = [0; 3];
        b.closed_until = None;
        b.under_construction_until = None;
        b.floor_arrears = 0;
        b.floor_rent = 0;
        b.hours = None;
        b.floors = 1;
    }
    crate::systems::founding::convert(world, t, BuildingKind::Lot, None, tier);
    if let Some(c) = owner.filter(|&o| world.has::<Corp>(o)) {
        ownership::push_corp_shock(world, c, crate::components::CorpShock::BuildingLost);
    }
    world.push_event(
        EventKind::Floor,
        &actors,
        format!("{whom} put {who} out of {what} ({floors}, {on}; {why}): the unit stands vacant"),
    );
    ownership::jobs2_row(world).floor_evicted += 1;
    reindex(world);
}

/// C6b: `b`'s opening hours, `[from, to)` minutes: its own (`Building.hours`), else its kind's `[tenancy] hours`
/// row; `None` with `[tenancy]` off.
pub fn hours_of(world: &World, b: EntityId) -> Option<(u16, u16)> {
    let cfg = world.config.tenancy.as_ref()?;
    let bd = world.comp::<Building>(b)?;
    Some(bd.hours.unwrap_or_else(|| cfg.hours_of(bd.kind)))
}

/// C6b: is `b` open at minute `tod` of the day (`to` below `from` runs past midnight)? `true` with no hours.
pub fn open_at(world: &World, b: EntityId, tod: u16) -> bool {
    match hours_of(world, b) {
        Some((from, to)) if from <= to => tod >= from && tod < to,
        Some((from, to)) => tod >= from || tod < to,
        None => true,
    }
}

/// C6b: the floors a business stands on, for the diary: "Bar#12, floor 0 of Block#3 (floor rent 4 a day to
/// Arasaka, 2 days short)"; `None` for a building that is no tenancy.
pub fn lease_text(world: &World, b: EntityId) -> Option<String> {
    let bd = world.comp::<Building>(b)?;
    let host = bd.within?;
    let landlord = world.owner_of(host);
    let rent = bd.floor_rent;
    let arrears = if bd.floor_arrears > 0 { format!(", {} days short", bd.floor_arrears) } else { String::new() };
    Some(format!(
        "{}, {} of {} (floor rent {rent} a day to {}{arrears})",
        world.name_of(b),
        span_text(world, b),
        world.name_of(host),
        world.owner_label(landlord)
    ))
}

/// C6b: the `#floors` lines of `run --report` (diagnostic): per district, the hosts with units, the units, the
/// vacant ones and the tenancies; then the city's vacant units and Lots and the floor rent due a day.
pub fn report(world: &World) -> Vec<String> {
    let mut by: BTreeMap<u8, (u32, u32, u32, u32)> = BTreeMap::new();
    for (&h, ts) in &world.floor_index.tenancies {
        by.entry(world.district_of_building(h).0).or_default().3 += ts.len() as u32;
    }
    for b in world.with::<Building>() {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if bd.units == 0 || bd.within.is_some() {
            continue;
        }
        let e = by.entry(world.district_of_building(b).0).or_default();
        e.0 += 1;
        e.1 += u32::from(bd.units);
        e.2 += world.floor_index.vacant.get(&b).copied().unwrap_or(0);
    }
    let mut out: Vec<String> = by
        .into_iter()
        .map(|(d, (hosts, units, vacant, ten))| {
            let name = world.district(crate::components::DistrictId(d)).name.clone();
            format!("#floors\td{}\t{name}\thosts {hosts}\tunits {units}\tvacant {vacant}\ttenancies {ten}", d + 1)
        })
        .collect();
    let due: i64 = world
        .floor_index
        .tenancies
        .values()
        .flatten()
        .filter_map(|&t| world.comp::<Building>(t))
        .map(|b| b.floor_rent)
        .sum();
    let lots = crate::systems::founding::vacant_lots(world).len();
    out.push(format!(
        "#floors\tcity\tvacant units {}\tvacant Lots {lots}\tfloor rent due {due} a day",
        world.floor_index.total
    ));
    out
}

/// A building's floors for a line: "floor 3", "floors 2-5", or "" without a span.
pub fn span_text(world: &World, b: EntityId) -> String {
    match world.comp::<Building>(b).and_then(|bd| bd.span) {
        Some((lo, hi)) if lo == hi => format!("floor {lo}"),
        Some((lo, hi)) => format!("floors {lo}-{hi}"),
        None => String::new(),
    }
}
