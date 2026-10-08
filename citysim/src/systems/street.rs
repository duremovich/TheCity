//! M12 the street rung (docs/M12_DISTRICTS.md § 2 "Vagrancy" and § 4).
//!
//! A homeless adult has three rungs below a Home: a Hotel bed by the night
//! (plan D20-D22), a squat in a derelict building (D25-D27), and the
//! street, where the law sweeps (D15). Hotel guests and squatters keep
//! `Household.home == None`: they are Dregs who found a rung, not housed.
//!
//! The nightly pass (03:00, D22) books beds for the Statistical homeless,
//! settles them in derelict slots, clears one squat per swept district,
//! rolls Vagrancy on whoever is left on the street, and drops the street's
//! litter. The daily pass (midnight) abandons and re-lets Blocks. Full and
//! Coarse agents reach the same rungs through their Sleep and Squat goals
//! (`CheckIn`, `Occupy`). O(homeless) a night; nothing per tick.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Corp, Crime, DistrictId, Household, Job, Lod, MemoryKind, Niche, Position, Sentence,
    Squatter, Stance, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// The tick of day of the nightly pass (03:00).
pub const NIGHTLY_TOD: u16 = 180;
/// `District::vagrancy_log` keeps at most this many.
pub const VAGRANCY_LOG_CAP: usize = 64;
/// A Hotel bed is the guest's until 08:00.
pub const CHECKOUT_HOUR: u64 = 8;
/// D17: a squat's daily litter (amount, radius) and a rough sleeper's.
const SQUAT_LITTER: (u8, u8) = (4, 1);
const ROUGH_LITTER: (u8, u8) = (2, 0);
/// D17: a building going derelict, at its door.
pub const DERELICT_LITTER: (u8, u8) = (48, 2);

/// The street rung is on (`[street] enabled`).
pub fn enabled(world: &World) -> bool {
    world.config.street.enabled
}

/// D22 at 03:00: bookings, squats, the Sweep, Vagrancy (with `[law]
/// district_beats`), and the street's litter.
pub fn nightly(world: &mut World) {
    if enabled(world) {
        expire_bookings(world);
        book_statistical(world);
        assign_statistical_squats(world);
    }
    if world.config.law.district_beats {
        let swept = if enabled(world) { sweep_squats(world) } else { Vec::new() };
        vagrancy_except(world, &swept);
    }
    if crate::systems::litter::enabled(world) {
        street_litter(world);
    }
}

/// The daily pass at midnight (`districts::daily`): abandonment and re-letting.
pub fn daily(world: &mut World) {
    if !enabled(world) {
        return;
    }
    abandon_daily(world);
    relet_daily(world);
}

// ---------------------------------------------------------------------------
// Who is on the street
// ---------------------------------------------------------------------------

/// Homeless adults with a Brain who are free (no Sentence, not cuffed),
/// not emigrating, with no booked Hotel bed and no squat, ascending.
pub fn rough_sleepers(world: &World) -> Vec<EntityId> {
    homeless_adults(world)
        .into_iter()
        .filter(|&a| !world.has::<Squatter>(a) && booked_hotel(world, a).is_none())
        .collect()
}

/// Homeless adults with a Brain, free and staying (the rungs' candidates), ascending.
fn homeless_adults(world: &World) -> Vec<EntityId> {
    // scan-ok: nightly: the street rung
    world
        .citizens()
        .into_iter()
        .filter(|&a| world.comp::<Household>(a).is_some_and(|h| h.home.is_none()))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating))
        .filter(|&a| !world.has::<Sentence>(a) && crate::systems::demography::is_adult(world, a))
        .collect()
}

fn is_statistical(world: &World, a: EntityId) -> bool {
    world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical)
}

fn door_of(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

/// Where a walk to `agent`'s next place starts (outside its building).
fn origin(world: &World, agent: EntityId) -> Option<TilePos> {
    let pos = world.comp::<Position>(agent)?;
    Some(match pos.building.and_then(|b| world.comp::<Building>(b)) {
        Some(b) => world.outside_door(b),
        None => pos.tile,
    })
}

// ---------------------------------------------------------------------------
// Hotels (D20-D22)
// ---------------------------------------------------------------------------

/// A standing Hotel (not derelict, not demolished, not closed by a riot: M12 D33).
pub fn is_hotel(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Hotel && !bd.derelict && !bd.demolished)
        && !world.is_closed(b)
}

/// D20: a night's price: `round(night_price × level)`, the owner corp's Food
/// `price_level` when it is in Food, else 1.0.
pub fn hotel_price(world: &World, h: EntityId) -> i64 {
    let base = world.config.street.night_price as f32;
    let level = world
        .owner_of(h)
        .and_then(|o| world.comp::<Corp>(o))
        .filter(|c| c.niches.contains(&Niche::Food))
        .map_or(1.0, |c| c.level(Niche::Food));
    (base * level).round().max(0.0) as i64
}

/// The next 08:00 after `now`.
pub fn checkout_after(now: Tick) -> Tick {
    let day_start = now - now % TICKS_PER_DAY;
    let today = day_start + CHECKOUT_HOUR * TICKS_PER_HOUR;
    if now < today {
        today
    } else {
        today + TICKS_PER_DAY
    }
}

/// The Hotel an agent has a live booking at.
pub fn booked_hotel(world: &World, a: EntityId) -> Option<EntityId> {
    world.hotel_beds.get(&a).filter(|&&(_, until)| until > world.tick).map(|&(h, _)| h)
}

/// Beds at `h` not booked by a live booking.
pub fn free_beds(world: &World, h: EntityId) -> usize {
    free_beds_for(world, h, EntityId::NONE)
}

/// `free_beds` as `who` sees them: L2 (L30, `[lod] budget`) also minus
/// the live `Bed` reservations there by others not yet booked (a guest on
/// the way holds the bed it planned for).
pub fn free_beds_for(world: &World, h: EntityId, who: EntityId) -> usize {
    let beds = world.comp::<Building>(h).map_or(0, |b| usize::from(b.capacity));
    let now = world.tick;
    let taken = world.hotel_beds.values().filter(|&&(x, until)| x == h && until > now).count();
    let reserved = if crate::systems::lod::budget_on(world) { world.bed_reservations_unbooked(h, who) } else { 0 };
    beds.saturating_sub(taken + reserved)
}

/// Drop bookings whose night is over (03:00, and before any count).
pub fn expire_bookings(world: &mut World) {
    let now = world.tick;
    world.hotel_beds.retain(|_, &mut (_, until)| until > now);
}

/// D21: the Hotel a homeless Full or Coarse agent would walk to: its booked
/// one, else the nearest standing Hotel with a free bed it can pay for
/// tonight, door within `hotel_reach` (ties lower id).
pub fn hotel_for(world: &World, agent: EntityId) -> Option<EntityId> {
    if !enabled(world) {
        return None;
    }
    if let Some(h) = booked_hotel(world, agent) {
        return Some(h);
    }
    let from = origin(world, agent)?;
    let coins = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
    // L1: saving for a bed means walking to one (the two Sump Hotels sat
    // past 48 tiles from most of the street).
    let reach = if world.config.life.enabled {
        world.config.street.hotel_reach.max(world.config.life.hotel_reach_homeless)
    } else {
        world.config.street.hotel_reach
    };
    world
        .buildings_of_kind(BuildingKind::Hotel)
        .iter()
        .copied()
        .filter(|&h| is_hotel(world, h) && hotel_price(world, h) <= coins && free_beds_for(world, h, agent) > 0)
        .filter_map(|h| door_of(world, h).map(|d| (d.manhattan(from), h)))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, h)| h)
}

/// D21: is a bed tonight within the homeless agent's means and reach?
pub fn hotel_available(world: &World, agent: EntityId) -> bool {
    enabled(world)
        && world.comp::<Household>(agent).is_some_and(|h| h.home.is_none())
        && crate::systems::demography::is_adult(world, agent)
        && hotel_for(world, agent).is_some()
}

/// Book and pay a bed at `h` for tonight (D20): `Flow::Hotel` to the
/// owner (taxed), credited to the Hotel, until the next 08:00. A guest
/// already booked there pays nothing more.
fn book(world: &mut World, a: EntityId, h: EntityId) -> bool {
    if booked_hotel(world, a) == Some(h) {
        return true;
    }
    let price = hotel_price(world, h);
    let coins = world.comp::<Wallet>(a).map_or(0, |w| w.coins);
    if !is_hotel(world, h) || free_beds_for(world, h, a) == 0 || coins < price {
        return false;
    }
    let owner = world.owner_of(h);
    let paid = crate::systems::ownership::pay(world, Some(a), owner, price, crate::systems::ownership::Flow::Hotel);
    crate::systems::ownership::credit(world, h, paid);
    let until = checkout_after(world.tick);
    world.hotel_beds.insert(a, (h, until));
    world.stats.current.hotel_nights += 1;
    true
}

/// D21 `CheckIn` at completion: the agent is inside `h`; re-checks the bed
/// and the coins.
pub fn check_in(world: &mut World, a: EntityId, h: EntityId) -> Result<(), String> {
    if world.comp::<Position>(a).and_then(|p| p.building) != Some(h) {
        return Err("not at the Hotel".into());
    }
    if book(world, a, h) {
        Ok(())
    } else {
        Err("no bed".into())
    }
}

/// D22: beds for the Statistical homeless, by coins descending (ties lower
/// id), the nearest Hotel with a free bed first. Plan deviation: no
/// `hotel_reach` (an off-screen agent stands at its phase door, a Market
/// door outside the Sump, which no Sump Hotel is within reach of).
pub fn book_statistical(world: &mut World) {
    let mut guests: Vec<(i64, EntityId)> = homeless_adults(world)
        .into_iter()
        .filter(|&a| is_statistical(world, a) && !world.has::<Squatter>(a) && booked_hotel(world, a).is_none())
        .map(|a| (world.comp::<Wallet>(a).map_or(0, |w| w.coins), a))
        .collect();
    guests.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
    let hotels: Vec<EntityId> =
        world.buildings_of_kind(BuildingKind::Hotel).iter().copied().filter(|&h| is_hotel(world, h)).collect();
    if hotels.is_empty() {
        return;
    }
    for (coins, a) in guests {
        let Some(from) = world.comp::<Position>(a).map(|p| p.tile) else { continue };
        let pick = hotels
            .iter()
            .copied()
            .filter(|&h| free_beds(world, h) > 0 && hotel_price(world, h) <= coins)
            .filter_map(|h| door_of(world, h).map(|d| (d.manhattan(from), h)))
            .min();
        if let Some((_, h)) = pick {
            book(world, a, h);
        }
    }
}

/// D20: the opening Hotels: in the first `seed_hotels` Sump districts (id
/// order) with a vacant Lot, the Lot nearest the district's centroid becomes
/// a Hotel owned by the jobless adult (not an exec, not an owner) whose Home
/// door is nearest it (ties lower id); `Founded` "opened a Capsule Hotel".
pub fn seed_hotels(world: &mut World) {
    if !enabled(world) || world.config.street.seed_hotels == 0 {
        return;
    }
    let mut left = world.config.street.seed_hotels;
    let sump: Vec<(DistrictId, TilePos)> = world
        .districts
        .iter()
        .filter(|d| d.zone == crate::components::Zone::Sump)
        .map(|d| (d.id, d.centroid))
        .collect();
    for (d, centroid) in sump {
        if left == 0 {
            break;
        }
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter(|&l| world.district_of_building(l) == d)
            .filter_map(|l| door_of(world, l).map(|door| (door.manhattan(centroid), l)))
            .min()
            .map(|(_, l)| l);
        let Some(lot) = lot else { continue };
        let lot_door = door_of(world, lot).unwrap_or_default();
        let owners: std::collections::BTreeSet<EntityId> = world
            .with::<Building>()
            .into_iter()
            .filter_map(|b| world.comp::<Building>(b).and_then(|bd| bd.owner))
            .collect();
        let owner = world
            .citizens()
            .into_iter()
            .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a) && !owners.contains(&a))
            .filter(|&a| crate::systems::demography::is_adult(world, a))
            .filter(|&a| !crate::systems::founding::is_exec(world, a))
            .filter_map(|a| {
                let home = world.comp::<Household>(a).and_then(|h| h.home)?;
                Some((door_of(world, home)?.manhattan(lot_door), a))
            })
            .min()
            .map(|(_, a)| a);
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Hotel, owner).is_err() {
            continue;
        }
        left -= 1;
        let what = world.name_of(lot);
        let (actors, who) = match owner {
            Some(o) => (vec![o, lot], world.name_of(o)),
            None => (vec![lot], "the city".to_string()),
        };
        world.push_event(EventKind::Founded, &actors, format!("{who} opened a Capsule Hotel ({what})"));
    }
}

// ---------------------------------------------------------------------------
// Derelicts (D25, D26)
// ---------------------------------------------------------------------------

/// A derelict building.
pub fn is_derelict(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.derelict)
}

/// The kinds that may go derelict (D25): a bankrupt Farm, Market or
/// Security Office always goes to the City.
pub fn can_go_derelict(kind: BuildingKind) -> bool {
    matches!(kind, BuildingKind::Home | BuildingKind::Bar | BuildingKind::Hotel)
}

/// Put everyone living in or standing inside `b` out at its door, homeless
/// (no event, no `evicted_by`): the re-housing wait runs from now.
fn empty_building(world: &mut World, b: EntityId) {
    let Some(outside) = world.comp::<Building>(b).map(|bd| world.outside_door(bd)) else { return };
    let tick = world.tick;
    let residents: Vec<EntityId> = world.residents_of(b).to_vec();
    for &r in &residents {
        world.set_home(r, None);
        if let Some(h) = world.comp_mut::<Household>(r) {
            h.homeless_since = Some(tick);
        }
    }
    let inside: Vec<EntityId> = world.comp::<Building>(b).map(|bd| bd.occupants.clone()).unwrap_or_default();
    for a in inside {
        if world.has::<Brain>(a) {
            world.abort_plan(a);
        }
        world.remove_from_building(a);
        if let Some(p) = world.comp_mut::<Position>(a) {
            p.tile = outside;
            p.building = None;
            p.entered = tick;
        }
    }
    // Children whose Position still says inside (not in `occupants`).
    for r in residents {
        if world.comp::<Position>(r).is_some_and(|p| p.building == Some(b)) {
            if let Some(p) = world.comp_mut::<Position>(r) {
                p.tile = outside;
                p.building = None;
                p.entered = tick;
            }
        }
    }
    world.hotel_beds.retain(|_, &mut (h, _)| h != b);
}

/// D26: the opening derelicts: `seed_derelict_blocks` Sump Blocks by even
/// stride over the Sump Blocks sorted by (district, id), emptied (their
/// residents start homeless at the door), the pantry gone. Called after the
/// population is dealt and before `ownership::seed`; draws nothing.
pub fn seed_derelicts(world: &mut World) {
    let n = world.config.street.seed_derelict_blocks;
    if !enabled(world) || n == 0 {
        return;
    }
    let mut sump: Vec<(DistrictId, EntityId)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| !b.demolished))
        .filter(|&h| door_of(world, h).is_some_and(|d| world.map.zone(d) == crate::components::Zone::Sump))
        .map(|h| (world.district_of_building(h), h))
        .collect();
    sump.sort();
    let len = sump.len();
    if len == 0 {
        return;
    }
    let picks: Vec<EntityId> = (0..n.min(len)).map(|k| sump[k * len / n.min(len)].1).collect();
    for b in picks {
        empty_building(world, b);
        if let Some(bd) = world.comp_mut::<Building>(b) {
            bd.full_capacity.get_or_insert(bd.capacity);
            bd.derelict = true;
            bd.empty_since = Some(0);
            bd.stock_food = 0;
            bd.rent_per_day = 0;
        }
    }
    crate::systems::districts::rebuild(world);
}

/// D25: `b` goes derelict: residents and anyone inside are put out at the
/// door, the owner loses it (`owner = None`; a corp takes `BuildingLost`),
/// rent 0, staff let go, vacancies and any guard contract gone, the claim
/// kept; `Derelict` event and litter at the door. Only Blocks, Bars and
/// Hotels (`can_go_derelict`); anything else is refused.
pub fn make_derelict(world: &mut World, b: EntityId, why: &str) -> bool {
    let Some((kind, owner, door)) = world.comp::<Building>(b).map(|bd| (bd.kind, bd.owner, bd.door)) else {
        return false;
    };
    if !can_go_derelict(kind) || is_derelict(world, b) {
        return false;
    }
    empty_building(world, b);
    let staff = crate::systems::ownership::staff_at(world, b);
    let what = world.name_of(b);
    for s in staff {
        let text = format!("{} let go: {what} is derelict", world.name_of(s));
        crate::systems::economy::dismiss(world, s, Some(b), text);
    }
    world.vacancies.remove(&b);
    crate::systems::corps::end_contract(world, b, "the building is derelict");
    crate::systems::ownership::transfer_building(world, b, None);
    if let Some(c) = owner.filter(|&o| world.has::<Corp>(o)) {
        crate::systems::ownership::push_corp_shock(world, c, crate::components::CorpShock::BuildingLost);
    }
    let tick = world.tick;
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.full_capacity.get_or_insert(bd.capacity);
        bd.derelict = true;
        bd.owner = None;
        bd.rent_per_day = 0;
        bd.stock_food = 0;
        bd.empty_since = Some(tick);
    }
    let label = world.owner_label(owner);
    let mut actors = vec![b];
    actors.extend(owner);
    world.push_event(EventKind::Derelict, &actors, format!("{what} went derelict ({why}; was {label}'s)"));
    crate::systems::litter::deposit_near(world, door, Some(b), DERELICT_LITTER.0, DERELICT_LITTER.1);
    crate::systems::districts::rebuild(world);
    true
}

/// D26 return: a derelict building comes back to `owner` (bought,
/// nationalised, re-let). Its squatters are evicted (`SquatEvicted` each),
/// capacity back to the config's, `Derelict` event "re-let".
pub fn restore(world: &mut World, b: EntityId, owner: Option<EntityId>, why: &str) -> bool {
    if !is_derelict(world, b) {
        return false;
    }
    evict_squatters(world, b, why);
    let kind = world.comp::<Building>(b).map(|bd| bd.kind).unwrap_or(BuildingKind::Home);
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.derelict = false;
        // Fix pass (phase 3 review): back to the capacity it stood at before
        // it went derelict (the map's, a founded building's interior, a
        // Hotel's beds), not the config's.
        bd.capacity = bd.full_capacity.take().unwrap_or(bd.capacity);
        bd.empty_since = None;
    }
    crate::systems::corps::move_building(world, b, owner);
    let rent = if kind == BuildingKind::Home { crate::systems::ownership::rent_for(world, b) } else { 0 };
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.rent_per_day = rent;
    }
    let what = world.name_of(b);
    let label = world.owner_label(owner);
    let mut actors = vec![b];
    actors.extend(owner);
    world.push_event(EventKind::Derelict, &actors, format!("{what} re-let to {label} ({why})"));
    crate::systems::districts::rebuild(world);
    true
}

/// D26 abandonment, daily: a non-city Block with no residents for
/// `abandon_days` whose owner's purse is below zero goes derelict. Fix pass
/// (phase 3 review): a Bar or Hotel with no trade (yesterday's revenue 0)
/// for `abandon_days` under an owner in the red goes the same way.
pub fn abandon_daily(world: &mut World) {
    let now = world.tick;
    let days = world.config.street.abandon_days * TICKS_PER_DAY;
    let mut abandon = Vec::new();
    for kind in [BuildingKind::Home, BuildingKind::Bar, BuildingKind::Hotel] {
        for h in world.buildings_of_kind(kind).to_vec() {
            let Some((owner, derelict, demolished, idle)) = world.comp::<Building>(h).map(|b| {
                let idle = match kind {
                    BuildingKind::Home => world.residents_of(h).is_empty(),
                    _ => b.revenue.back().copied().unwrap_or(0) <= 0,
                };
                (b.owner, b.derelict, b.demolished, idle)
            }) else {
                continue;
            };
            if derelict || demolished {
                continue;
            }
            let since = {
                let Some(b) = world.comp_mut::<Building>(h) else { continue };
                if idle && owner.is_some() {
                    *b.empty_since.get_or_insert(now)
                } else {
                    b.empty_since = None;
                    continue;
                }
            };
            if now.saturating_sub(since) >= days && world.purse(owner) < 0 {
                abandon.push(h);
            }
        }
    }
    abandon.sort_unstable();
    for h in abandon {
        make_derelict(world, h, "abandoned");
    }
}

/// A derelict held by a live gang (its claim at `CLAIM_HELD`): the City
/// does not re-let it while the gang holds it (fix pass, phase 3 review).
fn gang_held(world: &World, b: EntityId) -> bool {
    world
        .comp::<Building>(b)
        .and_then(|bd| bd.claim)
        .is_some_and(|c| c.count >= crate::systems::gang::CLAIM_HELD && world.has::<crate::components::Gang>(c.gang))
}

/// Phase 3 decision ("who re-lets", `[street] relet_days`): one derelict a
/// day (any kind: a Block, a Bar or a Hotel, fix pass), the longest derelict
/// past `relet_days` (ties lower id) and not held by a gang, is repaired and
/// re-let by the City while the Treasury holds `city_absorb_floor`. Its
/// squatters are put out.
pub fn relet_daily(world: &mut World) {
    let days = world.config.street.relet_days;
    if days == 0 || world.purse(None) < world.config.street.city_absorb_floor {
        return;
    }
    let now = world.tick;
    let pick = derelicts(world)
        .into_iter()
        .filter(|&h| !gang_held(world, h))
        .filter_map(|h| {
            let b = world.comp::<Building>(h)?;
            let since = b.empty_since?;
            (now.saturating_sub(since) >= days * TICKS_PER_DAY).then_some((since, h))
        })
        .min();
    if let Some((_, h)) = pick {
        restore(world, h, None, "repaired and re-let by the city");
    }
}

// ---------------------------------------------------------------------------
// Squats (D27)
// ---------------------------------------------------------------------------

/// Free squat slots in a derelict building: capacity less its squatters.
pub fn squat_slots(world: &World, b: EntityId) -> usize {
    let Some(bd) = world.comp::<Building>(b).filter(|bd| bd.derelict && !bd.demolished) else { return 0 };
    usize::from(bd.capacity).saturating_sub(world.squatters_of(b).len())
}

/// Banned from `b` by an eviction still running.
pub fn squat_banned(world: &World, a: EntityId, b: EntityId) -> bool {
    world.comp::<Household>(a).and_then(|h| h.squat_ban).is_some_and(|(x, until)| x == b && world.tick < until)
}

/// Every derelict building (any kind), ascending.
pub fn derelicts(world: &World) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = [BuildingKind::Home, BuildingKind::Bar, BuildingKind::Hotel]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.derelict && !bd.demolished))
        .collect();
    out.sort_unstable();
    out
}

/// D27: the agent's squat, else the nearest derelict with a free slot
/// within `squat_reach` of it that it is not banned from (ties lower id).
pub fn squat_target(world: &World, agent: EntityId) -> Option<EntityId> {
    if let Some(s) = world.comp::<Squatter>(agent) {
        return Some(s.building);
    }
    if !enabled(world) {
        return None;
    }
    let from = origin(world, agent)?;
    let reach = world.config.street.squat_reach;
    derelicts(world)
        .into_iter()
        .filter(|&b| squat_slots(world, b) > 0 && !squat_banned(world, agent, b))
        .filter_map(|b| door_of(world, b).map(|d| (d.manhattan(from), b)))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, b)| b)
}

/// D27 eligibility for the Squat goal: an adult, homeless, not squatting,
/// no bed tonight within means, and a derelict slot within reach.
pub fn can_squat(world: &World, agent: EntityId) -> bool {
    enabled(world)
        && !world.has::<Squatter>(agent)
        && world.comp::<Household>(agent).is_some_and(|h| h.home.is_none())
        && crate::systems::demography::is_adult(world, agent)
        && !hotel_available(world, agent)
        && squat_target(world, agent).is_some()
}

/// Settle `a` in derelict `b`: a `Squatter`, a booking dropped; the first
/// squatter of an empty building logs `Squatted`. Refused without a slot,
/// under a ban, or for the housed.
pub fn occupy(world: &mut World, a: EntityId, b: EntityId) -> Result<(), String> {
    if world.comp::<Squatter>(a).is_some_and(|s| s.building == b) {
        return Ok(());
    }
    if world.comp::<Household>(a).is_none_or(|h| h.home.is_some()) {
        return Err("housed".into());
    }
    if squat_slots(world, b) == 0 || squat_banned(world, a, b) {
        return Err("no slot".into());
    }
    let first = world.squatters_of(b).is_empty();
    let now = world.tick;
    world.insert(a, Squatter { building: b, since: now });
    world.hotel_beds.remove(&a);
    if first {
        let (name, what) = (world.name_of(a), world.name_of(b));
        world.push_event(EventKind::Squatted, &[a, b], format!("{name} squatted in derelict {what}"));
    }
    Ok(())
}

/// Put one squatter out: the `Squatter` goes, a ban on that building for
/// `squat_ban_days`, an `Evicted` memory (0.6), `SquatEvicted`, and out of
/// the door if inside.
pub fn evict_squatter(world: &mut World, a: EntityId, why: &str) {
    let Some(b) = world.remove::<Squatter>(a).map(|s| s.building) else { return };
    let until = world.tick + world.config.street.squat_ban_days * TICKS_PER_DAY;
    if let Some(h) = world.comp_mut::<Household>(a) {
        h.squat_ban = Some((b, until));
    }
    world.remember(a, MemoryKind::Evicted, None, 0.6, -0.6, false);
    // M15: the squat eviction is talked about (actor the building's owner, if any).
    if world.config.gossip.enabled {
        let (d, owner) = (world.district_of_building(b), world.owner_of(b));
        crate::systems::gossip::post_deed(world, d, crate::word::Deed::Evicted, owner, Some(a));
    }
    if world.comp::<Position>(a).is_some_and(|p| p.building == Some(b)) {
        if world.has::<Brain>(a) {
            world.abort_plan(a);
        }
        world.leave_building(a);
    }
    let (name, what) = (world.name_of(a), world.name_of(b));
    world.push_event(EventKind::SquatEvicted, &[a, b], format!("{name} put out of {what} ({why})"));
}

/// Every squatter of `b` put out (D26 return, a Sweep, a gang's flip).
pub fn evict_squatters(world: &mut World, b: EntityId, why: &str) -> Vec<EntityId> {
    let out: Vec<EntityId> = world.squatters_of(b).to_vec();
    for &a in &out {
        evict_squatter(world, a, why);
    }
    out
}

/// D22: each Statistical homeless adult with no bed and no squat takes the
/// nearest derelict slot it is not banned from (ties lower id). Plan
/// deviation: any district, not only its own (an off-screen homeless agent
/// stands at a Market door, and the Sump has no Market).
pub fn assign_statistical_squats(world: &mut World) {
    let candidates: Vec<EntityId> = homeless_adults(world)
        .into_iter()
        .filter(|&a| is_statistical(world, a) && !world.has::<Squatter>(a) && booked_hotel(world, a).is_none())
        .collect();
    if candidates.is_empty() {
        return;
    }
    let all = derelicts(world);
    for a in candidates {
        let Some(from) = world.comp::<Position>(a).map(|p| p.tile) else { continue };
        let pick = all
            .iter()
            .copied()
            .filter(|&b| squat_slots(world, b) > 0 && !squat_banned(world, a, b))
            .filter_map(|b| door_of(world, b).map(|d| (d.manhattan(from), b)))
            .min();
        if let Some((_, b)) = pick {
            let _ = occupy(world, a, b);
        }
    }
}

/// D12/§ 4: under a Sweep stance the law clears one squat per district a
/// night, the one with the most squatters (ties lower id): every squatter
/// is put out and rolled as a Vagrancy arrest at `p = 1`.
pub fn sweep_squats(world: &mut World) -> Vec<EntityId> {
    let swept: Vec<DistrictId> = world.districts.iter().filter(|d| d.stance == Stance::Sweep).map(|d| d.id).collect();
    let mut caught = Vec::new();
    for d in swept {
        let pick = derelicts(world)
            .into_iter()
            .filter(|&b| world.district_of_building(b) == d && !world.squatters_of(b).is_empty())
            .map(|b| (std::cmp::Reverse(world.squatters_of(b).len()), b))
            .min()
            .map(|(_, b)| b);
        let Some(b) = pick else { continue };
        let door = door_of(world, b).unwrap_or_default();
        let out = evict_squatters(world, b, "swept by the law");
        for a in out {
            if world.has::<Brain>(a) && !world.has::<Sentence>(a) {
                vagrancy_hit(world, a, door, d);
            }
            caught.push(a);
        }
    }
    caught
}

/// D17: the night's street litter: each squat 4 r1 at its door, each rough
/// sleeper 2 r0 where it lies (not inside a building).
fn street_litter(world: &mut World) {
    for b in derelicts(world) {
        if world.squatters_of(b).is_empty() {
            continue;
        }
        if let Some(door) = door_of(world, b) {
            crate::systems::litter::deposit_near(world, door, Some(b), SQUAT_LITTER.0, SQUAT_LITTER.1);
        }
    }
    for a in rough_sleepers(world) {
        let Some(p) = world.comp::<Position>(a).filter(|p| p.building.is_none()).map(|p| p.tile) else { continue };
        crate::systems::litter::deposit(world, p, ROUGH_LITTER.0, ROUGH_LITTER.1);
    }
}

/// The street's snapshot for the CSV: squatters and derelict buildings.
pub fn snapshot(world: &World) -> (u32, u32) {
    let squatters: usize = world.squat_index.values().map(Vec::len).sum();
    (squatters as u32, derelicts(world).len() as u32)
}

// ---------------------------------------------------------------------------
// Vagrancy (D15)
// ---------------------------------------------------------------------------

/// A fine or a Vagrancy sentence in district `d`: the counter and the log.
pub fn note_vagrancy(world: &mut World, d: DistrictId) {
    world.stats.current.vagrancy += 1;
    let now = world.tick;
    if let Some(x) = world.districts.get_mut(d.index()) {
        if x.vagrancy_log.len() >= VAGRANCY_LOG_CAP {
            x.vagrancy_log.pop_front();
        }
        x.vagrancy_log.push_back(now);
    }
}

/// D15: the sweep probability for a rough sleeper in `d`.
pub fn vagrancy_p(world: &World, d: DistrictId) -> f32 {
    let cfg = &world.config.law;
    let dist = world.district(d);
    let sweep = if dist.stance == Stance::Sweep { cfg.sweep_mult } else { 1.0 };
    let curfew = if world.levers.curfew.get(d.index()).copied().unwrap_or(false) { cfg.curfew_mult } else { 1.0 };
    // Fix pass (phase 2 review): a district without Homes reads coverage_max
    // in `bind::district_coverage` (no Homes to divide by), which would double
    // the sweep where no guard walks; it rolls at coverage 1.
    let coverage = if dist.homes.is_empty() { 1.0 } else { dist.coverage };
    let alert = crate::systems::faction::alertness_mult(world, None);
    (cfg.vagrancy_base * coverage * sweep * curfew * alert).clamp(0.0, 1.0)
}

/// The on-shift, free city guard standing in `d` nearest `tile` (Manhattan,
/// ties the lower id); any tier.
fn nearest_guard_in(world: &World, d: DistrictId, tile: TilePos) -> Option<EntityId> {
    let tod = world.tick_of_day();
    world
        .guards()
        .iter()
        .copied()
        .filter(|&g| crate::systems::law::is_city_guard(world, g) && !world.has::<Sentence>(g))
        .filter(|&g| world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod)))
        .filter_map(|g| world.comp::<Position>(g).map(|p| (g, p.tile)))
        .filter(|&(_, t)| world.district_of(t) == d)
        .min_by_key(|&(g, t)| (t.manhattan(tile), g))
        .map(|(g, _)| g)
}

/// D15: a rough sleeper caught in `d`: one who can pay the fine pays it to
/// the Treasury; a broke Statistical vagrant is jailed for the night if the
/// Precinct has a free cell; a broke Full or Coarse vagrant is reported by
/// the nearest on-shift city guard in the district (or by nobody: a sweep)
/// and the arrest path chases them.
fn vagrancy_hit(world: &mut World, a: EntityId, tile: TilePos, d: DistrictId) {
    let fine = world.config.law.vagrancy_fine;
    let capacity = usize::from(world.config.buildings.jail.capacity);
    let name = world.name_of(a);
    let place = world.district_name(d).to_string();
    let coins = world.comp::<Wallet>(a).map_or(0, |w| w.coins);
    if fine > 0 && coins >= fine {
        let paid = crate::systems::ownership::pay(world, Some(a), None, fine, crate::systems::ownership::Flow::Fine);
        note_vagrancy(world, d);
        world.push_event(EventKind::Vagrancy, &[a], format!("{name} fined {paid} for sleeping rough in {place}"));
        return;
    }
    if is_statistical(world, a) {
        let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return };
        if world.sentenced().len() >= capacity {
            return;
        }
        let until = world.tick + crate::systems::law::sentence_ticks(world, Crime::Vagrancy);
        world.vagrancy_places.insert(a, d);
        crate::systems::law::sentence(world, a, Crime::Vagrancy, until, jail);
        return;
    }
    let witness = nearest_guard_in(world, d, tile);
    world.vagrancy_places.insert(a, d);
    crate::systems::law::file_report(world, Crime::Vagrancy, a, witness);
    let now = world.tick;
    world.last_seen.insert(a, (tile, now));
}

/// D15: the nightly Vagrancy pass. Each rough sleeper not inside a building
/// (a Statistical one wherever it stands) rolls `vagrancy_p` of the district
/// it lies in. Also writes `District.rough` (rough sleepers per district).
pub fn vagrancy(world: &mut World) {
    vagrancy_except(world, &[]);
}

/// `vagrancy` without rolling `skip` (squatters a Sweep just caught).
fn vagrancy_except(world: &mut World, skip: &[EntityId]) {
    let sleepers = rough_sleepers(world);
    let n = world.districts.len();
    let mut rough = vec![0u16; n];
    let mut rolls: Vec<(EntityId, TilePos, DistrictId)> = Vec::new();
    for a in sleepers {
        let Some(pos) = world.comp::<Position>(a) else { continue };
        let tile = pos.tile;
        let d = world.district_of(tile);
        let statistical = is_statistical(world, a);
        if (pos.building.is_some() && !statistical) || skip.contains(&a) {
            continue;
        }
        // Fix pass (phase 2 review): only the sleepers the law can roll count
        // as rough (one asleep inside a building is not on the street, and
        // `rough` feeds the Sweep stance).
        if let Some(r) = rough.get_mut(d.index()) {
            *r = r.saturating_add(1);
        }
        rolls.push((a, tile, d));
    }
    for (x, r) in world.districts.iter_mut().zip(&rough) {
        x.rough = *r;
    }
    for (a, tile, d) in rolls {
        let p = vagrancy_p(world, d);
        let roll: f32 = world.rng.world().random();
        if roll >= p {
            continue;
        }
        vagrancy_hit(world, a, tile, d);
    }
}
