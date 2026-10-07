//! Life pass L1 (docs/SHADOW_V1.md): the helpers the goals, the planner
//! and the executor share to make a day less of a walk. Every caller reads
//! `on` first: with `[life] enabled = false` nothing here runs.

use crate::components::{
    Brain, Building, BuildingKind, Corp, Gang, GoalKind, Household, Job, Memory, MemoryKind, Needs, Position, Sentence,
    TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::LocationKey;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::utility::Consideration;
use crate::world::World;

/// Is the life pass on?
pub fn on(world: &World) -> bool {
    world.config.life.enabled
}

/// Ticks to walk `tiles` (Manhattan) on foot: `move_ticks_full` a tile,
/// padded by half for the detours a road grid forces (as the commute gate).
pub fn walk_ticks(world: &World, tiles: u32) -> u64 {
    u64::from(tiles) * world.config.exec.move_ticks_full * 3 / 2
}

/// Where a walk starts: outside the current building, or the agent's tile.
pub fn origin(world: &World, id: EntityId) -> Option<TilePos> {
    crate::exec::walk_origin(world, id)
}

/// Door-to-door tiles from the agent to building `b` (0 inside it).
pub fn tiles_to(world: &World, id: EntityId, b: EntityId) -> Option<u32> {
    if world.comp::<Position>(id).and_then(|p| p.building) == Some(b) {
        return Some(0);
    }
    let door = world.comp::<Building>(b)?.door;
    Some(origin(world, id)?.manhattan(door))
}

/// Tiles from the agent to a street tile.
pub fn tiles_to_tile(world: &World, id: EntityId, t: TilePos) -> Option<u32> {
    Some(origin(world, id)?.manhattan(t))
}

/// `1 / (1 + walk / travel_half_ticks)`, floored at `travel_min`.
pub fn travel_factor(world: &World, ticks: u64) -> f32 {
    let half = world.config.life.travel_half_ticks.max(1.0);
    (1.0 / (1.0 + ticks as f32 / half)).max(world.config.life.travel_min)
}

/// The travel consideration for a goal whose venue is `tiles` away: input
/// the walk in hours, output the factor.
pub fn travel(world: &World, tiles: u32) -> Consideration {
    let ticks = walk_ticks(world, tiles);
    Consideration::raw("travel", ticks as f32 / 60.0, travel_factor(world, ticks))
}

/// The local meal price (the Eat goal's), at least 1.
pub fn meal_price(world: &World, id: EntityId) -> i64 {
    world.local(id, BuildingKind::Market).map_or(3, |m| world.price_for(m, id)).max(1)
}

fn coins(world: &World, id: EntityId) -> i64 {
    world.comp::<Wallet>(id).map_or(0, |w| w.coins)
}

fn home_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Household>(id).and_then(|h| h.home)
}

// ---------------------------------------------------------------------------
// Beds
// ---------------------------------------------------------------------------

/// A member's own Hideout as a bed: not sacked, room (or already inside),
/// and nearer than Home by `bed_margin_tiles` (any distance when homeless).
pub fn hideout_bed(world: &World, id: EntityId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let gang = world.gang_of(id)?;
    let g = world.comp::<Gang>(gang)?;
    if g.is_sacked(world.tick) {
        return None;
    }
    let h = g.hideout;
    let b = world.comp::<Building>(h)?;
    if b.is_full() && !b.occupants.contains(&id) {
        return None;
    }
    let t = tiles_to(world, id, h)?;
    match home_of(world, id).and_then(|home| tiles_to(world, id, home)) {
        Some(home) => (t + world.config.life.bed_margin_tiles <= home).then_some(h),
        None => Some(h),
    }
}

/// A housed adult far from Home: the nearest standing Hotel with a free
/// bed it can pay for and keep `hotel_reserve_meals` meals, nearer than
/// Home by `bed_margin_tiles`. Its booking, if it holds one.
pub fn away_hotel(world: &World, id: EntityId) -> Option<EntityId> {
    if !on(world) || !crate::systems::street::enabled(world) || !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    let home = home_of(world, id)?;
    if let Some(h) = crate::systems::street::booked_hotel(world, id) {
        return Some(h);
    }
    let home_t = tiles_to(world, id, home)?;
    let budget = coins(world, id) - world.config.life.hotel_reserve_meals * meal_price(world, id);
    let margin = world.config.life.bed_margin_tiles;
    world
        .buildings_of_kind(BuildingKind::Hotel)
        .iter()
        .copied()
        .filter(|&h| {
            crate::systems::street::is_hotel(world, h)
                && crate::systems::street::hotel_price(world, h) <= budget
                && crate::systems::street::free_beds(world, h) > 0
        })
        .filter_map(|h| tiles_to(world, id, h).map(|t| (t, h)))
        .filter(|&(t, _)| t + margin <= home_t)
        .min()
        .map(|(_, h)| h)
}

/// The nearest bed this agent may use tonight, in tiles: Home, the
/// Hideout (a member), a booked or affordable Hotel, the squat.
pub fn nearest_bed_tiles(world: &World, id: EntityId) -> Option<u32> {
    let mut best: Option<u32> = None;
    let mut take = |t: Option<u32>| {
        if let Some(t) = t {
            best = Some(best.map_or(t, |b| b.min(t)));
        }
    };
    take(home_of(world, id).and_then(|h| tiles_to(world, id, h)));
    take(hideout_bed(world, id).and_then(|h| tiles_to(world, id, h)));
    let hotel =
        if home_of(world, id).is_some() { away_hotel(world, id) } else { crate::systems::street::hotel_for(world, id) };
    take(hotel.and_then(|h| tiles_to(world, id, h)));
    take(world.comp::<crate::components::Squatter>(id).and_then(|s| tiles_to(world, id, s.building)));
    best
}

/// Exhausted, and every bed past `rough_min_tiles`: lie down where you are.
pub fn rough_ok(world: &World, id: EntityId) -> bool {
    on(world)
        && world.comp::<Needs>(id).is_some_and(|n| n.energy < world.config.life.exhausted_energy)
        && nearest_bed_tiles(world, id).is_none_or(|t| t > world.config.life.rough_min_tiles)
}

// ---------------------------------------------------------------------------
// Flee
// ---------------------------------------------------------------------------

/// Where a frightened agent runs: Home; without one, the nearest of its
/// squat, its gang's Hideout, the Precinct and the local Bar.
pub fn refuge(world: &World, id: EntityId) -> Option<(LocationKey, EntityId)> {
    if let Some(h) = home_of(world, id) {
        return Some((LocationKey::Home, h));
    }
    if !on(world) {
        return None;
    }
    let mut cands: Vec<(LocationKey, EntityId)> = Vec::new();
    if let Some(s) = world.comp::<crate::components::Squatter>(id) {
        cands.push((LocationKey::Squat, s.building));
    }
    if let Some(h) = world.gang_of(id).and_then(|g| world.hideout_of(g)) {
        cands.push((LocationKey::Hideout, h));
    }
    if !crate::systems::law::wanted(world, id) {
        if let Some(j) = world.building_of_kind(BuildingKind::Jail) {
            cands.push((LocationKey::Jail, j));
        }
    }
    if let Some(b) = world.local(id, BuildingKind::Bar) {
        cands.push((LocationKey::Bar, b));
    }
    cands.into_iter().filter_map(|(k, b)| tiles_to(world, id, b).map(|t| (t, k, b))).min().map(|(_, k, b)| (k, b))
}

// ---------------------------------------------------------------------------
// The dole
// ---------------------------------------------------------------------------

/// Days of dole due at a Hall visit now: the days since the last one, at
/// most `dole_bulk_days` (a first visit: the days since day 0); 0 once
/// taken today.
pub fn dole_days(world: &World, id: EntityId) -> u64 {
    let today = world.day();
    let bulk = world.config.life.dole_bulk_days.max(1);
    match world.comp::<Brain>(id).and_then(|b| b.last_dole_day) {
        Some(d) if d >= today => 0,
        Some(d) => (today - d).min(bulk),
        None => (today + 1).min(bulk),
    }
}

/// Is a Hall trip for the dole worth it now: `dole_trip_days` accrued, or
/// the agent cannot buy a meal, or the Hall is near.
pub fn dole_trip_due(world: &World, id: EntityId) -> bool {
    // Paid where the agent is (`dole_in_place`): no trip at all.
    if world.config.life.dole_in_place {
        return false;
    }
    let days = dole_days(world, id);
    if days == 0 {
        return false;
    }
    days >= world.config.life.dole_trip_days
        || coins(world, id) < meal_price(world, id)
        || world
            .building_of_kind(BuildingKind::Hall)
            .and_then(|h| tiles_to(world, id, h))
            .is_some_and(|t| t <= world.config.life.near_tiles)
}

/// Below a meal (and, homeless, a meal and a Hotel night): the poor's Earn.
pub fn broke(world: &World, id: EntityId) -> bool {
    let mut need = meal_price(world, id);
    if home_of(world, id).is_none() && crate::systems::street::enabled(world) {
        need += world.config.street.night_price;
    }
    coins(world, id) < need
}

// ---------------------------------------------------------------------------
// The law
// ---------------------------------------------------------------------------

/// Another guard already chases `suspect` (an Arrest plan bound to it, or
/// the escort).
pub fn arrest_claimed(world: &World, guard: EntityId, suspect: EntityId) -> bool {
    world.guards().iter().any(|&g| {
        g != guard
            && world.comp::<Brain>(g).is_some_and(|b| {
                b.escorting == Some(suspect)
                    || b.plan.as_ref().is_some_and(|p| p.goal == GoalKind::Arrest && p.target == Some(suspect))
            })
    })
}

/// A sighting fresh enough to chase from `from`: seen within
/// `arrest_fresh_ticks`, or within `near_tiles` of the guard.
pub fn sighting_fresh(world: &World, suspect: EntityId, from: TilePos) -> bool {
    world.last_seen.get(&suspect).is_some_and(|&(t, at)| {
        world.tick.saturating_sub(at) <= world.config.life.arrest_fresh_ticks
            || t.manhattan(from) <= world.config.life.near_tiles
    })
}

/// The guard reached the suspect's last-seen tile. A suspect in sight
/// (`[crime] sight`) is chased on (a fresh GoTo is spliced in, at most
/// `chase_hops` times); one gone is a stale sighting: it is dropped, so no
/// guard walks there again until someone sees them. Returns whether the
/// chase goes on.
pub fn arrive_at_suspect(world: &mut World, guard: EntityId, suspect: EntityId) -> bool {
    if crate::systems::law::near(world, guard, suspect, 1) {
        return true;
    }
    let sight = world.config.crime.sight;
    let hops = world.comp::<Brain>(guard).map_or(0, |b| b.chase_hops);
    let visible = crate::systems::law::living(world, suspect)
        && !world.has::<Sentence>(suspect)
        && crate::systems::law::near(world, guard, suspect, sight + 2);
    if visible && hops < world.config.life.chase_hops {
        let tile = world.comp::<Position>(suspect).map(|p| p.tile).unwrap_or_default();
        let now = world.tick;
        world.last_seen.insert(suspect, (tile, now));
        if let Some(b) = world.comp_mut::<Brain>(guard) {
            b.chase_hops = hops + 1;
            let at = usize::from(b.plan_step);
            if let Some(plan) = b.plan.as_mut() {
                let at = at.min(plan.steps.len());
                plan.steps.insert(
                    at,
                    crate::components::ActionInstance {
                        action: crate::goap::ActionKind::GoTo(LocationKey::SuspectTile),
                        target: Some(suspect),
                        tile: None,
                    },
                );
            }
        }
        return true;
    }
    // Stale: nobody chases this sighting again.
    world.last_seen.remove(&suspect);
    false
}

// ---------------------------------------------------------------------------
// Witnesses
// ---------------------------------------------------------------------------

/// Has `w` already seen `actor` commit `crime` within `witness_dedupe_ticks`?
pub fn saw_recently(world: &World, w: EntityId, actor: EntityId, crime: crate::components::Crime) -> bool {
    let window = world.config.life.witness_dedupe_ticks;
    let now = world.tick;
    world.comp::<Memory>(w).is_some_and(|m| {
        m.entries.iter().any(|e| {
            e.kind == MemoryKind::SawCrime
                && e.subject == Some(actor)
                && e.crime == Some(crime)
                && now.saturating_sub(e.tick) < window
        })
    })
}

/// A deterministic shuffle key for pair `(a, b)` on `day` (no RNG draw):
/// which of a room's strangers an arrival meets first.
pub fn pair_key(a: EntityId, b: EntityId, day: u64) -> u64 {
    let mut h = (u64::from(a.index) << 32) ^ u64::from(b.index) ^ day.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h
}

// ---------------------------------------------------------------------------
// Housing: a commute that fits a day
// ---------------------------------------------------------------------------

/// Daily: up to `relocate_per_day` employed adults whose commute is past
/// `commute_cap_tiles` move (with a spouse and children who share their
/// Home) into the Home with room nearest their workplace that they can pay
/// `[rent] rehouse_coins_mult` x rent for, when it is nearer by the margin.
/// Longest commutes first (ties lower id). A guard's workplace is the Precinct.
pub fn relocate(world: &mut World) {
    if !on(world) {
        return;
    }
    let cap = world.config.life.commute_cap_tiles;
    let margin = world.config.life.bed_margin_tiles;
    let mult = world.config.rent.rehouse_coins_mult;
    let mut movers: Vec<(std::cmp::Reverse<u32>, EntityId, EntityId, TilePos)> = Vec::new();
    // scan-ok: daily: the relocation pass
    // An exec's workplace is its corp's HQ.
    let execs: std::collections::BTreeMap<EntityId, EntityId> = world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).and_then(|cc| cc.exec).map(|e| (e, c)))
        .collect();
    for a in world.citizens() {
        let employer = match world.comp::<Job>(a) {
            Some(job) => job.employer,
            None => execs.get(&a).and_then(|&c| hq_of(world, c, a)),
        };
        let Some(employer) = employer else { continue };
        if world.has::<Sentence>(a) || world.comp::<Brain>(a).is_none_or(|b| b.emigrating) {
            continue;
        }
        let Some(home) = home_of(world, a) else { continue };
        let Some(work) = world.comp::<Building>(employer).map(|b| b.door) else { continue };
        let Some(hd) = world.comp::<Building>(home).map(|b| b.door) else { continue };
        let d = hd.manhattan(work);
        if d > cap {
            movers.push((std::cmp::Reverse(d), a, home, work));
        }
    }
    movers.sort_unstable();
    let homes: Vec<(EntityId, TilePos, usize)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter_map(|&h| {
            world
                .comp::<Building>(h)
                .filter(|b| !b.demolished && !b.derelict)
                .map(|b| (h, b.door, usize::from(b.capacity)))
        })
        .collect();
    let mut moved = 0usize;
    let tick = world.tick;
    for (std::cmp::Reverse(d), a, home, work) in movers {
        if moved >= world.config.life.relocate_per_day {
            break;
        }
        // Moved already as someone's spouse this pass.
        if home_of(world, a) != Some(home) {
            continue;
        }
        let spouse = world.spouse_of(a).filter(|&s| home_of(world, s) == Some(home) && world.has::<Brain>(s));
        // A spouse with a job of their own stays put (two commutes, one Home).
        if spouse.is_some_and(|s| world.has::<Job>(s)) {
            continue;
        }
        let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
            .into_iter()
            .filter(|&c| world.has::<crate::components::Child>(c) && home_of(world, c) == Some(home))
            .collect();
        let need = 1 + usize::from(spouse.is_some()) + kids.len();
        let purse = coins(world, a);
        let evicted_by = world.comp::<Household>(a).and_then(|h| h.evicted_by).map(|(o, _)| o);
        let pick = homes
            .iter()
            .filter(|&&(h, _, cap)| {
                h != home
                    && world.residents_of(h).len() + need <= cap
                    && purse >= mult * crate::systems::ownership::rent_for(world, h)
                    && evicted_by.is_none_or(|o| o != world.owner_of(h))
            })
            .map(|&(h, door, _)| (door.manhattan(work), h))
            .filter(|&(dd, _)| dd + margin <= d)
            .min();
        let Some((_, to)) = pick else { continue };
        let place = world.name_of(to);
        for m in std::iter::once(a).chain(spouse) {
            world.set_home(m, Some(to));
            if let Some(h) = world.comp_mut::<Household>(m) {
                h.rent_due = 0.0;
            }
            let name = world.name_of(m);
            world.push_event(EventKind::Housed, &[m, to], format!("{name} moved nearer work, into {place}"));
        }
        let door = world.comp::<Building>(to).map(|b| b.door).unwrap_or_default();
        for c in kids {
            world.set_home(c, Some(to));
            world.remove_from_building(c);
            if let Some(p) = world.comp_mut::<Position>(c) {
                p.tile = door;
                p.building = None;
                p.entered = tick;
            }
            world.enter_building(c, to);
        }
        moved += 1;
    }
}

// ---------------------------------------------------------------------------
// Execs
// ---------------------------------------------------------------------------

/// An exec's day salary: `exec_pay_frac` of the corp's treasury, floored at
/// `[economy] wage_exec` and capped at `exec_pay_cap` (the flat wage when off).
pub fn exec_pay(world: &World, corp: EntityId) -> i64 {
    let floor = world.config.economy.wage_exec;
    if !on(world) {
        return floor;
    }
    let t = world.comp::<Corp>(corp).map_or(0, |c| c.treasury);
    let scaled = (t.max(0) as f32 * world.config.life.exec_pay_frac).round() as i64;
    scaled.clamp(floor, world.config.life.exec_pay_cap.max(floor))
}

/// The exec pick's score (higher is better): an adult of at least
/// `exec_min_age_years` (else ineligible), by wealth (coins), then
/// persuasion + knowledge, then age. `None` for the ineligible. Wealth
/// leads: picking by the exec skill first put a top-percentile exec at
/// every corp, and M15's competence (normalised on the old pick) rose ~10 %
/// city-wide, Farm output with it.
pub fn exec_score(world: &World, a: EntityId) -> Option<(i64, i64, u32)> {
    let ident = world.comp::<crate::components::Identity>(a)?;
    let min_days = (world.config.life.exec_min_age_years * crate::time::DAYS_PER_YEAR as f32) as u32;
    if ident.age_days < min_days {
        return None;
    }
    let skill = world.comp::<crate::components::Skills>(a).map_or(0.0, |s| s.persuasion + s.knowledge);
    Some((coins(world, a), (skill * 1000.0).round() as i64, ident.age_days))
}

/// The corp's HQ for its exec's office hours: the corp's working building
/// (no Home, no Lot) nearest the exec's Home, ties to the most senior kind
/// and then the lower id.
pub fn hq_of(world: &World, corp: EntityId, exec: EntityId) -> Option<EntityId> {
    const RANK: [BuildingKind; 10] = [
        BuildingKind::Lab,
        BuildingKind::SecurityOffice,
        BuildingKind::Feed,
        BuildingKind::Market,
        BuildingKind::Clinic,
        BuildingKind::Garage,
        BuildingKind::Hotel,
        BuildingKind::Bar,
        BuildingKind::Farm,
        BuildingKind::Warehouse,
    ];
    let c = world.comp::<Corp>(corp)?;
    let from = home_of(world, exec).and_then(|h| world.comp::<Building>(h)).map(|b| b.door);
    c.buildings
        .iter()
        .copied()
        .filter_map(|b| {
            let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict)?;
            let rank = RANK.iter().position(|&k| k == bd.kind)?;
            Some((from.map_or(0, |f| f.manhattan(bd.door)), rank, b))
        })
        .min()
        .map(|(_, _, b)| b)
}

/// The corp `id` is the exec of.
pub fn exec_corp(world: &World, id: EntityId) -> Option<EntityId> {
    world.corps().into_iter().find(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(id)))
}

/// An exec's office day to keep: on, an exec of a corp with an HQ, not
/// jailed, today's office hours not yet kept, and the shift on (or about to
/// start within the walk).
pub fn exec_day_pending(world: &World, id: EntityId) -> Option<EntityId> {
    if !on(world) || world.has::<Sentence>(id) || world.has::<Job>(id) {
        return None;
    }
    let today = world.day();
    if world.comp::<Brain>(id).is_some_and(|b| b.office_day == Some(today)) {
        return None;
    }
    let corp = exec_corp(world, id)?;
    let hq = hq_of(world, corp, id)?;
    let (s, e) = world.config.life.exec_shift;
    let tod = u64::from(world.tick_of_day());
    if !crate::exec::routine::is_workday(today as i64) || tod >= u64::from(e) {
        return None;
    }
    let walk = tiles_to(world, id, hq).map_or(0, |t| walk_ticks(world, t));
    (tod + walk + 30 >= u64::from(s)).then_some(hq)
}

/// Move an exec (with a spouse and children sharing the Home) into a free
/// Spire Home (tier 2) when they live lower down and one has room.
pub fn house_exec(world: &mut World, exec: EntityId) {
    if !on(world) {
        return;
    }
    let Some(home) = home_of(world, exec) else { return };
    if world.comp::<Building>(home).is_some_and(|b| b.tier >= 2) {
        return;
    }
    let spouse = world.spouse_of(exec).filter(|&s| home_of(world, s) == Some(home) && world.has::<Brain>(s));
    let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, exec)
        .into_iter()
        .filter(|&c| world.has::<crate::components::Child>(c) && home_of(world, c) == Some(home))
        .collect();
    let need = 1 + usize::from(spouse.is_some()) + kids.len();
    let pick = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| {
            world.comp::<Building>(h).is_some_and(|b| {
                b.tier >= 2
                    && !b.demolished
                    && !b.derelict
                    && world.residents_of(h).len() + need <= usize::from(b.capacity)
            })
        })
        .min_by_key(|&h| (world.residents_of(h).len(), h));
    let Some(to) = pick else { return };
    let tick = world.tick;
    let place = world.name_of(to);
    for m in std::iter::once(exec).chain(spouse) {
        world.set_home(m, Some(to));
        let name = world.name_of(m);
        world.push_event(EventKind::Housed, &[m, to], format!("{name} moved into {place} (the Spire)"));
    }
    let door = world.comp::<Building>(to).map(|b| b.door).unwrap_or_default();
    for c in kids {
        world.set_home(c, Some(to));
        world.remove_from_building(c);
        if let Some(p) = world.comp_mut::<Position>(c) {
            p.tile = door;
            p.building = None;
            p.entered = tick;
        }
        world.enter_building(c, to);
    }
}

/// An exec's office hours at the HQ: `GoTo(Seller)` bound to the HQ, then
/// `Meeting` until the shift ends. `None` without an office day to keep.
pub fn exec_plan(world: &World, id: EntityId) -> Option<crate::goap::Plan> {
    let hq = exec_day_pending(world, id)?;
    let mut steps = Vec::new();
    if world.comp::<Position>(id).and_then(|p| p.building) != Some(hq) {
        steps.push(crate::components::ActionInstance {
            action: crate::goap::ActionKind::GoTo(LocationKey::Seller),
            target: Some(hq),
            tile: None,
        });
    }
    steps.push(crate::components::ActionInstance {
        action: crate::goap::ActionKind::Meeting,
        target: Some(hq),
        tile: None,
    });
    Some(crate::goap::Plan { goal: GoalKind::Work, target: Some(hq), steps, started_tick: world.tick })
}

/// Ticks until today's office shift ends (an early arrival waits for it; 0
/// when it is over).
pub fn exec_shift_left(world: &World) -> Tick {
    let (_, e) = world.config.life.exec_shift;
    Tick::from(e.saturating_sub(world.tick_of_day()))
}

/// Today's office shift is over.
pub fn exec_shift_over(world: &World) -> bool {
    world.tick_of_day() >= world.config.life.exec_shift.1
}

/// Days a quitter is not rehired by the employer it left.
pub fn quit_blocks(world: &World, a: EntityId, employer: EntityId) -> bool {
    on(world)
        && world
            .comp::<Brain>(a)
            .and_then(|b| b.quit_from)
            .is_some_and(|(e, until)| e == employer && world.tick < until)
}

/// Record a quit (`quit_rehire_days` before that employer hires them again).
pub fn note_quit(world: &mut World, a: EntityId, employer: Option<EntityId>) {
    if !on(world) {
        return;
    }
    let until = world.tick + world.config.life.quit_rehire_days * TICKS_PER_DAY;
    if let (Some(e), Some(b)) = (employer, world.comp_mut::<Brain>(a)) {
        b.quit_from = Some((e, until));
    }
}

/// L1: the daily dole paid where the agent is, at the Work phase's start
/// (09:00), to every free jobless adult body (Full and Coarse; the
/// Statistical tier keeps its own daily roll) below its savings line, as
/// the Full Earn goal's gate reads it; execs draw a salary instead. Same
/// amount, same day: `economy::collect_dole` pays it (treasury not negative,
/// once a day). The Hall walk it replaces was 2-4 h a day for 4 coins.
pub fn dole_in_place(world: &mut World) {
    if !on(world) || !world.config.life.dole_in_place {
        return;
    }
    let execs: std::collections::BTreeSet<EntityId> =
        world.corps().into_iter().filter_map(|c| world.comp::<Corp>(c).and_then(|cc| cc.exec)).collect();
    let today = world.day();
    let due: Vec<EntityId> = world
        .bodies()
        .into_iter()
        .filter(|&a| {
            world.comp::<Brain>(a).is_some_and(|b| {
                b.lod != crate::components::Lod::Statistical && !b.emigrating && b.last_dole_day != Some(today)
            }) && !world.has::<Job>(a)
                && !world.has::<Sentence>(a)
                && !execs.contains(&a)
                && crate::systems::demography::is_adult(world, a)
                && coins(world, a) < crate::goap::world_state::SAVINGS_DAYS.saturating_mul(meal_price(world, a))
        })
        .collect();
    for a in due {
        crate::systems::economy::collect_dole(world, a);
    }
}
