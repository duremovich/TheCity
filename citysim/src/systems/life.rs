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

/// Hunger as sickness (parity, after P5): can `id` buy a meal now: a
/// stocked food Market (`World::food_market`) whose meal it can pay for. A
/// city whose shelves are bare is no meal however many coins are held (the
/// all-Full calibration city starved 199 in 30 days walking to empty
/// Markets with the coins of a synth meal, never eligible for a bin or a
/// Mission).
pub fn can_buy_meal(world: &World, id: EntityId) -> bool {
    let coins = coins(world, id);
    world
        .food_market(id)
        .filter(|&m| world.comp::<Building>(m).is_some_and(|b| b.stock_food > 0))
        .is_some_and(|m| coins >= crate::systems::diet::price_at(world, id, m).max(1))
}

/// The local meal price (the Eat goal's), at least 1.
pub fn meal_price(world: &World, id: EntityId) -> i64 {
    // Hunger as sickness § 1: the meal this agent would buy (its own price).
    if crate::systems::diet::on(world) {
        return crate::systems::diet::meal_price(world, id);
    }
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
    // L2 (L30): a bed another member reserved on the way is taken.
    if !b.occupants.contains(&id) && b.occupants.len() + world.reserved_at(h, id) >= usize::from(b.capacity) {
        return None;
    }
    let t = tiles_to(world, id, h)?;
    // L2 shadow fixes item 20: the nearer of Home and the Hideout, no margin
    // (a dealer walked 160 min to sleep at Home past her Hideout).
    match home_of(world, id).and_then(|home| tiles_to(world, id, home)) {
        Some(home) => (t < home).then_some(h),
        None => Some(h),
    }
}

/// A housed adult far from Home: the nearest standing Hotel with a free
/// bed it can pay for and keep `hotel_reserve_meals` meals, nearer than
/// Home by `bed_margin_tiles`. Its booking, if it holds one.
pub fn away_hotel(world: &World, id: EntityId) -> Option<EntityId> {
    if !crate::systems::street::enabled(world) || !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    let home = home_of(world, id)?;
    if let Some(h) = crate::systems::street::booked_hotel(world, id) {
        return Some(h);
    }
    let home_t = tiles_to(world, id, home)?;
    let budget = coins(world, id) - world.config.life.hotel_reserve_meals * meal_price(world, id);
    let margin = world.config.life.bed_margin_tiles;
    // Scale S4 perf: the walks first, then the bed tests nearest first
    // until one passes (a bed count reads every booking), not the tests
    // over every Hotel: the same pick.
    let mut near: Vec<(u32, EntityId)> = world
        .buildings_of_kind(BuildingKind::Hotel)
        .iter()
        .copied()
        .filter_map(|h| tiles_to(world, id, h).map(|t| (t, h)))
        .filter(|&(t, _)| t + margin <= home_t)
        .collect();
    near.sort_unstable();
    near.into_iter()
        .find(|&(_, h)| {
            crate::systems::street::is_hotel(world, h)
                && crate::systems::street::hotel_price(world, h) <= budget
                && crate::systems::street::free_beds_for(world, h, id) > 0
        })
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
    world.comp::<Needs>(id).is_some_and(|n| n.energy < world.config.life.exhausted_energy)
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
    // Hunger as sickness § 3: a diagnosed patient short of the treatment.
    coins(world, id) < need || crate::systems::health::saving_short(world, id)
}

/// The jobless poor's Earn (Beg, Scavenge): broke, or (Shadow V4 bug 1)
/// with no dole a jobless non-exec adult under `[life] earn_meals_no_net`
/// meals' coins at the local Market's price (Earn's savings line's; one
/// meal's coins stopped the hustle: 46-65 idle hours a week).
pub fn earn_short(world: &World, id: EntityId) -> bool {
    if broke(world, id) {
        return true;
    }
    if world.levers.dole_per_day > 0 || world.has::<Job>(id) || exec_corp(world, id).is_some() {
        return false;
    }
    let price = world.local(id, BuildingKind::Market).map_or(3, |m| world.price_for(m, id)).max(1);
    coins(world, id) < world.config.life.earn_meals_no_net.saturating_mul(price)
}

// ---------------------------------------------------------------------------
// The law
// ---------------------------------------------------------------------------

/// The suspects another guard than `guard` already chases (an Arrest plan
/// bound to them, or the escort): one pass over the guards' Brains, so a
/// scoring round tests membership per suspect instead of re-walking every
/// guard per (guard, suspect) pair (review fix).
pub fn claimed_suspects(world: &World, guard: EntityId) -> std::collections::BTreeSet<EntityId> {
    let mut out = std::collections::BTreeSet::new();
    for &g in world.guards().iter().filter(|&&g| g != guard) {
        let Some(b) = world.comp::<Brain>(g) else { continue };
        if let Some(s) = b.escorting {
            out.insert(s);
        }
        if let Some(s) = b.plan.as_ref().filter(|p| p.goal == GoalKind::Arrest).and_then(|p| p.target) {
            out.insert(s);
        }
    }
    out
}

/// A sighting fresh enough to chase from `from`: seen within
/// `arrest_fresh_ticks`, or within `near_tiles` of the guard.
pub fn sighting_fresh(world: &World, suspect: EntityId, from: TilePos) -> bool {
    world.last_seen().get(&suspect).is_some_and(|&seen| sighting_fresh_at(world, seen, from))
}

/// `sighting_fresh` on a `last_seen` entry already read (Scale S1).
pub fn sighting_fresh_at(world: &World, (t, at): (TilePos, crate::time::Tick), from: TilePos) -> bool {
    world.tick.saturating_sub(at) <= world.config.life.arrest_fresh_ticks
        || t.manhattan(from) <= world.config.life.near_tiles
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
        world.set_last_seen(suspect, (tile, now));
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
    world.remove_last_seen(&suspect);
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

/// Hunger as sickness P5g (the transit spec's call): the one measure of a
/// commute, in travel ticks from a home door to a work door. Today the walk
/// (tiles x `[exec] move_ticks_full`); transit (the El) swaps the body.
/// Every P5g commute rule (relocation, the hiring band, the seeded staff's
/// housing) reads it. Transit phase 2: `transit::commute_ticks`, the walk
/// or the ride when the El is on and wins (the walk exactly with it off),
/// so a Home near a stop is near the Vats.
pub fn commute_cost(world: &World, home: TilePos, work: TilePos) -> crate::time::Tick {
    crate::systems::transit::commute_ticks(world, home, work)
}

/// Hunger fix item 10 (P5h), daily before `relocate`: housing demand at
/// each employer (its workers whose `commute_cost` is past
/// `commute_move_ticks`). Demand of at least `housing_demand_min` with
/// fewer free beds than that within range of the employer's door counts a
/// midnight; any other midnight resets it. After `housing_demand_days`,
/// highest demand first (ties lower id), up to `housing_builds_per_1000` a
/// day per 1,000 residents: a Home on the nearest vacant Lot in range (a
/// plain Lot before a shanty's, whose squatters `founding::convert` puts
/// out), else a floor on the nearest Home in range below `[floors]
/// floors_max`. The employer's owner corp pays `found_cost` / `floor_cost`
/// (Flow::Found, as Grow and AddFloor) when its purse holds it; else the
/// Treasury buys it from the World when it holds the cost above its band
/// floor (`housing_payer`); else nothing goes up today. No RNG; bounded by
/// the Lots and the floor cap.
///
/// The catch-up (`housing_catchup_days` > 0; the flipped city's Farms
/// outgrew two builds a day: 458 far workers on day 120, seed 42): a ready
/// employer's unmet demand is its far workers less the free beds in range;
/// it gets `ceil(unmet / beds a build adds / housing_catchup_days)` builds
/// today (at least one), the city `ceil(sum of unmet / beds / catch-up
/// days)`, floored at `housing_builds_per_1000` and capped at
/// `housing_builds_max_per_1000` per 1,000 residents; and a build keeps the
/// employer's demand count (the next midnight builds again while the beds
/// fill and the far workers remain; free beds in range reset it). Payment
/// is unchanged: no unpaid builds.
pub fn grow_housing(world: &mut World) {
    if !crate::systems::fixes::grow_housing(world) {
        if !world.housing_demand.is_empty() {
            world.housing_demand.clear();
        }
        return;
    }
    let cfg = world.config.life.clone();
    let cap = cfg.commute_move_ticks;
    let demand = housing_far(world);
    let homes = housing_homes(world);
    // (demand, employer, unmet far workers)
    let mut ready: Vec<(std::cmp::Reverse<u32>, EntityId, u32)> = Vec::new();
    let mut next: std::collections::BTreeMap<EntityId, u16> = std::collections::BTreeMap::new();
    for (&employer, &n) in &demand {
        if n < cfg.housing_demand_min {
            continue;
        }
        let Some(work) = world.comp::<Building>(employer).map(|b| b.door) else { continue };
        let free = free_beds_near(world, &homes, work, cap);
        if free >= cfg.housing_demand_min as usize {
            continue;
        }
        let days = world.housing_demand.get(&employer).copied().unwrap_or(0).saturating_add(1);
        next.insert(employer, days);
        if days >= cfg.housing_demand_days {
            let unmet = n.saturating_sub(u32::try_from(free).unwrap_or(u32::MAX));
            ready.push((std::cmp::Reverse(n), employer, unmet));
        }
    }
    world.housing_demand = next;
    ready.sort_unstable();
    let pop = world.citizens().len() as f32;
    let base = (cfg.housing_builds_per_1000 * pop / 1000.0).round().max(1.0) as usize;
    let catchup = cfg.housing_catchup_days > 0.0;
    let beds = f32::from(world.config.buildings.for_kind(BuildingKind::Home).capacity.max(1));
    // Builds the unmet demand asks for today (the catch-up).
    let want = |unmet: u32| -> usize { (unmet as f32 / beds / cfg.housing_catchup_days).ceil().max(0.0) as usize };
    let limit = if catchup {
        let max = (cfg.housing_builds_max_per_1000 * pop / 1000.0).round().max(base as f32) as usize;
        want(ready.iter().map(|&(_, _, u)| u).sum()).clamp(base, max)
    } else {
        base
    };
    let mut built = 0;
    'employers: for (_, employer, unmet) in ready {
        let mine = if catchup { want(unmet).max(1) } else { 1 };
        for _ in 0..mine {
            if built >= limit {
                break 'employers;
            }
            if !build_housing_near(world, employer) {
                break;
            }
            built += 1;
            if !catchup {
                world.housing_demand.remove(&employer);
            }
        }
    }
}

/// P5h: the housing demand per employer, `grow_housing`'s measure: its
/// workers (not sentenced, not emigrating, not sweepers, housed) whose
/// `commute_cost` from their Home's door to its door is past
/// `commute_move_ticks`.
fn housing_far(world: &World) -> std::collections::BTreeMap<EntityId, u32> {
    let cap = world.config.life.commute_move_ticks;
    let mut demand: std::collections::BTreeMap<EntityId, u32> = std::collections::BTreeMap::new();
    // scan-ok: daily: the housing demand pass
    for a in world.citizens() {
        let Some(employer) = world.comp::<Job>(a).and_then(|j| j.employer) else { continue };
        if world.has::<Sentence>(a) || world.comp::<Brain>(a).is_none_or(|b| b.emigrating) {
            continue;
        }
        // A sweeper's day is its beat, not the employer's door: no demand there.
        if crate::systems::jobs::is_sweeper(world, a) {
            continue;
        }
        let Some(work) = world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door) else { continue };
        let Some(hd) = home_of(world, a).and_then(|h| world.comp::<Building>(h)).map(|b| b.door) else { continue };
        if commute_cost(world, hd, work) > cap {
            *demand.entry(employer).or_default() += 1;
        }
    }
    demand
}

/// P5h: every standing Home's `(door, beds, residents)`.
fn housing_homes(world: &World) -> Vec<(TilePos, usize, usize)> {
    world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter_map(|&h| {
            let b = world.comp::<Building>(h).filter(|b| !b.demolished && !b.derelict)?;
            Some((b.door, usize::from(b.capacity), world.residents_of(h).len()))
        })
        .collect()
}

/// P5h: the free beds in `homes` within `cap` (`commute_cost`) of `work`.
fn free_beds_near(world: &World, homes: &[(TilePos, usize, usize)], work: TilePos, cap: u64) -> usize {
    homes
        .iter()
        .filter(|&&(door, _, _)| commute_cost(world, door, work) <= cap)
        .map(|&(_, c, r)| c.saturating_sub(r))
        .sum()
}

/// The pre-roll (the seed city has stood for years: it is not a city that
/// hired 800 people this morning into Homes laid out for 500), at world
/// creation after the staffing: the seeded staff housed near work
/// (`relocate_up_to`, uncapped; hunger fix item 9), then, with `[life]
/// housing_preroll`, items 8-10, wages and no safety net on (the flipped
/// city whose staffing outgrew the map's Homes; the dole city's creation,
/// and the wages-on dole city the unit tests pin to the shipped map, are as
/// before), rounds of: each employer whose far workers (`grow_housing`'s
/// measure) outnumber the free beds within `commute_move_ticks` of its door
/// gets one build (`build_housing_near`'s rule: a plain vacant Lot in range,
/// then a shanty's, then a floor on a Home below `[floors] floors_max`;
/// most unmet first, ties lower id), then the uncapped relocation fills
/// them; until no employer builds (no unmet demand, or no Lot or floor left
/// in range). A pre-roll building is a seed building: it stands unpaid (no
/// coin moves, so `econ::identity` and the escrow are untouched; the
/// opening hoard is already the corps' capital) and silent (no event),
/// owned as a paid build would be by the employer's owner corp (company
/// housing) else the city, with a seed Home's pantry. Then the day's job
/// search fills the places the new Blocks posted (their trades), the
/// relocation runs once more, and every adult housed away from the Block it
/// was dealt into wakes up in its own (the seed city lives where it is
/// housed: a Statistical body left in the old Block never slept at home).
/// No RNG. Returns the builds (Lots, floors).
pub fn preroll_housing(world: &mut World) -> (u32, u32) {
    relocate_up_to(world, usize::MAX);
    let on = world.config.life.housing_preroll
        && crate::systems::fixes::relocate_fix(world)
        && crate::systems::fixes::grow_housing(world)
        && crate::systems::fixes::hire_near(world)
        && crate::systems::wages::on(world);
    let mut built = (0u32, 0u32);
    if !on {
        return built;
    }
    let cap = world.config.life.commute_move_ticks;
    loop {
        let demand = housing_far(world);
        let homes = housing_homes(world);
        let mut unmet: Vec<(std::cmp::Reverse<usize>, EntityId)> = demand
            .iter()
            .filter_map(|(&employer, &n)| {
                let work = world.comp::<Building>(employer)?.door;
                let free = free_beds_near(world, &homes, work, cap);
                (n as usize > free).then_some((std::cmp::Reverse(n as usize - free), employer))
            })
            .collect();
        unmet.sort_unstable();
        let mut round = 0;
        for (_, employer) in unmet {
            match build_near(world, employer, true) {
                Some(true) => built.0 += 1,
                Some(false) => built.1 += 1,
                None => continue,
            }
            round += 1;
        }
        if round == 0 {
            break;
        }
        relocate_up_to(world, usize::MAX);
    }
    if built != (0, 0) {
        crate::systems::demography::job_search(world);
        relocate_up_to(world, usize::MAX);
    }
    // Bodies to their Homes: the creation's movers are still inside the
    // Block they were dealt into.
    let tick = world.tick;
    // scan-ok: once, at world creation
    for a in world.citizens() {
        let Some(to) = home_of(world, a) else { continue };
        let Some(at) = world.comp::<Position>(a).and_then(|p| p.building) else { continue };
        if at == to || world.comp::<Building>(at).is_none_or(|b| b.kind != BuildingKind::Home) {
            continue;
        }
        let Some(door) = world.comp::<Building>(to).map(|b| b.door) else { continue };
        world.remove_from_building(a);
        if let Some(p) = world.comp_mut::<Position>(a) {
            p.tile = door;
            p.building = None;
            p.entered = tick;
        }
        world.enter_building(a, to);
    }
    built
}

/// P5h: one Home (or a floor) within `commute_move_ticks` of `employer`'s
/// door, as `grow_housing` says. Returns whether anything went up.
pub fn build_housing_near(world: &mut World, employer: EntityId) -> bool {
    build_near(world, employer, false).is_some()
}

/// `build_housing_near`, paid (`housing_payer`) or, at the pre-roll,
/// unpaid (`seeded`: a seed building, owned by the employer's owner corp,
/// else the city; a Lot's Home opens with `[world] home_pantry_initial`).
/// `Some(true)` a Home on a Lot, `Some(false)` a floor, `None` nothing.
fn build_near(world: &mut World, employer: EntityId, seeded: bool) -> Option<bool> {
    let cap = world.config.life.commute_move_ticks;
    let (work, owner) = world.comp::<Building>(employer).map(|b| (b.door, b.owner))?;
    let corp = owner.filter(|&o| world.has::<Corp>(o));
    let what = world.name_of(employer);
    // A vacant Lot in range: plain Lots first, then shanties; nearest; ties lower id.
    let lot = crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter_map(|l| {
            let d = commute_cost(world, world.comp::<Building>(l)?.door, work);
            (d <= cap).then(|| (crate::systems::street::is_shanty(world, l), d, l))
        })
        .min()
        .map(|(_, _, l)| l);
    if let Some(lot) = lot {
        let cost = crate::systems::founding::found_cost(world, BuildingKind::Home).unwrap_or(0);
        let by = if seeded { corp } else { housing_payer(world, corp, cost)? };
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Home, by).is_err() {
            return None;
        }
        if seeded {
            // A seed Block: its pantry, no event (it stood before day 0).
            let pantry = world.config.world.home_pantry_initial;
            if let Some(b) = world.comp_mut::<Building>(lot) {
                b.stock_food = pantry;
            }
            return Some(true);
        }
        let paid = pay_housing(world, by, cost);
        let (who, home) = (world.owner_label(by), world.name_of(lot));
        let mut actors: Vec<EntityId> = by.into_iter().collect();
        actors.push(lot);
        world.push_event(
            EventKind::Founded,
            &actors,
            format!("{who} built {home} on a Lot for {paid}: housing for {what}'s far workers"),
        );
        return Some(true);
    }
    // Else a floor on the nearest Home in range below its cap.
    let max = world.config.floors.max_for(BuildingKind::Home);
    let home = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter_map(|&h| {
            let b = world.comp::<Building>(h).filter(|b| !b.demolished && !b.derelict && b.floors.max(1) < max)?;
            let d = commute_cost(world, b.door, work);
            (d <= cap).then_some((d, h))
        })
        .min()
        .map(|(_, h)| h)?;
    if seeded {
        // A seed storey: `add_floor`'s seats, no coin and no event.
        let (floors, capacity) = world.comp::<Building>(home).map(|b| (b.floors.max(1), b.capacity))?;
        let per_floor = capacity / u16::from(floors);
        let new_capacity =
            crate::systems::founding::with_floors(world, BuildingKind::Home, per_floor, floors + 1).max(capacity);
        let b = world.comp_mut::<Building>(home)?;
        b.capacity = new_capacity;
        b.floors = floors + 1;
        b.floor_days = 0;
        return Some(false);
    }
    let cost = crate::systems::founding::floor_cost(world, BuildingKind::Home).unwrap_or(0);
    let by = housing_payer(world, corp, cost)?;
    // A corp's floor is paid inside `add_floor` (Flow::Found); the city's
    // after it stands.
    crate::systems::founding::add_floor(world, home, by).ok()?;
    if by.is_none() {
        pay_housing(world, None, cost);
    }
    Some(false)
}

/// P5h (the no-safety-net doctrine: no unpaid builds): who pays `cost` for
/// housing toward `corp`'s workplace. The owner corp when its purse holds
/// it (`Some(Some(corp))`); else the Treasury (`Some(None)`) when it holds
/// the cost and keeps `[treasury] band[0]` after it (with the Treasury band
/// on; else just the cost), the civic budget's floor; else nobody (`None`:
/// nothing is built today).
pub(crate) fn housing_payer(world: &World, corp: Option<EntityId>, cost: i64) -> Option<Option<EntityId>> {
    if let Some(c) = corp.filter(|&c| world.purse(Some(c)) >= cost) {
        return Some(Some(c));
    }
    let floor = if crate::systems::treasury::on(world) { world.config.treasury.band[0] } else { 0 };
    (world.purse(None) - cost >= floor).then_some(None)
}

/// P5h: the payment for a Home or floor: a corp pays the Treasury
/// `Flow::Found` (Grow's and AddFloor's path); the Treasury buys the build
/// from the World (`ownership::import`, `ImportWhy::Build`), so the coins
/// leave the city as any import's. Returns the coins paid.
pub(crate) fn pay_housing(world: &mut World, by: Option<EntityId>, cost: i64) -> i64 {
    if cost <= 0 {
        return 0;
    }
    match by {
        // Jobs & lives phase 1 (§ 2): a Construction Yard's build when one
        // stands (no site: the Block opens at once), else M11's path.
        Some(c) => crate::systems::industry::pay_build(world, Some(c), cost, None, false),
        // The Treasury hires the city's own Yard before it imports the build.
        None if crate::systems::industry::yard_open(world) => {
            crate::systems::industry::pay_build(world, None, cost, None, false)
        }
        None => crate::systems::ownership::import(world, None, cost, crate::systems::ownership::ImportWhy::Build),
    }
}

/// Items 8-10: where `id`'s working day is spent, the point every commute
/// rule measures to: its employer's door, except a sweeper's (Sweep on),
/// which is its beat district's centroid (the Recycler is only its wage
/// desk; the behaviour tier's d19 sweepers walked 10.25 h a day, 7.43 with
/// items 8-10 off, housed toward the Recycler and away from their beats).
pub fn work_tile(world: &World, id: EntityId, employer: EntityId) -> Option<TilePos> {
    if crate::systems::jobs::is_sweeper(world, id) {
        if let Some(c) =
            crate::systems::jobs::beat_of(world, id).and_then(|d| world.districts.get(d.index())).map(|x| x.centroid)
        {
            return Some(c);
        }
    }
    world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door)
}

/// Daily: up to `relocate_per_day` employed adults whose commute is past
/// `commute_cap_tiles` move (with a spouse and children who share their
/// Home) into the Home with room nearest their workplace that they can pay
/// `[rent] rehouse_coins_mult` x rent for, when it is nearer by the margin.
/// Longest commutes first (ties lower id). A guard's workplace is the Precinct.
/// Hunger fix item 8 (P5g): with the fix, the daily cap is
/// `relocate_per_1000` per 1,000 residents (`relocate_up_to` has the rule).
pub fn relocate(world: &mut World) {
    let limit = if crate::systems::fixes::relocate_fix(world) {
        let pop = world.citizens().len() as f32;
        (world.config.life.relocate_per_1000 * pop / 1000.0).round().max(world.config.life.relocate_per_day as f32)
            as usize
    } else {
        world.config.life.relocate_per_day
    };
    relocate_up_to(world, limit);
}

/// `relocate` with a mover limit (world creation: uncapped). Hunger fix item
/// 8 (P5g): a mover is a worker whose `commute_cost` is past
/// `commute_move_ticks`; the destination is the nearest Home with room
/// within that of work (else a trade), paid by the first rent (no
/// `rehouse_coins_mult` deposit), saving `commute_margin_ticks`. Off, the
/// L1 rule in tiles (`commute_cap_tiles`, `bed_margin_tiles`).
pub fn relocate_up_to(world: &mut World, limit: usize) {
    relocate_up_to_with(world, limit, true);
}

/// [`relocate_up_to`]; `lazy` false fills every Home's measure to a work
/// tile before the pick's filters (the pre-review walk, for the tests).
fn relocate_up_to_with(world: &mut World, limit: usize, lazy: bool) {
    let fix = crate::systems::fixes::relocate_fix(world);
    // The measure: travel ticks with the fix, the L1 tiles without.
    let cost = |w: &World, a: TilePos, b: TilePos| -> u64 {
        if fix {
            commute_cost(w, a, b)
        } else {
            u64::from(a.manhattan(b))
        }
    };
    let cap: u64 =
        if fix { world.config.life.commute_move_ticks } else { u64::from(world.config.life.commute_cap_tiles) };
    let margin: u64 =
        if fix { world.config.life.commute_margin_ticks } else { u64::from(world.config.life.bed_margin_tiles) };
    let mult = if fix { 1 } else { world.config.rent.rehouse_coins_mult };
    let mut movers: Vec<(std::cmp::Reverse<u64>, EntityId, EntityId, TilePos)> = Vec::new();
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
        let Some(work) =
            (if fix { work_tile(world, a, employer) } else { world.comp::<Building>(employer).map(|b| b.door) })
        else {
            continue;
        };
        let Some(hd) = world.comp::<Building>(home).map(|b| b.door) else { continue };
        let d = cost(world, hd, work);
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
    // Scale S1: each Home's measure to a work tile, once a pass per tile (the
    // pick and the trade read it for every mover; a pure function of the two
    // doors and the network, which a pass does not change).
    let mut memo = DoorCosts::new(homes.iter().map(|&(_, door, _)| door).collect());
    let mut moved = 0usize;
    let tick = world.tick;
    for (std::cmp::Reverse(d), a, home, work) in movers {
        if moved >= limit {
            break;
        }
        // Moved already as someone's spouse this pass.
        if home_of(world, a) != Some(home) {
            continue;
        }
        let spouse = world.spouse_of(a).filter(|&s| home_of(world, s) == Some(home) && world.has::<Brain>(s));
        // A spouse with a job of their own stays put (two commutes, one
        // Home). Hunger fix item 8 (P5h): with the fix the couple moves when the two
        // commutes together shrink by the margin (39 of the 42 far workers
        // left at a Farm the new Homes served on day 8 had a working spouse).
        let spouse_work = spouse
            .and_then(|s| world.comp::<Job>(s).map(|j| (s, j.employer)))
            .map(|(s, e)| e.and_then(|e| work_tile(world, s, e)));
        let spouse_work = match spouse_work {
            Some(None) => continue,
            Some(Some(_)) if !fix => continue,
            Some(Some(sw)) => Some(sw),
            None => None,
        };
        let hd = world.comp::<Building>(home).map(|b| b.door).unwrap_or(work);
        let both = |door: TilePos| spouse_work.map_or(0, |sw| cost(world, door, sw));
        let d_pair = d + both(hd);
        let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
            .into_iter()
            .filter(|&c| world.has::<crate::components::Child>(c) && home_of(world, c) == Some(home))
            .collect();
        let need = 1 + usize::from(spouse.is_some()) + kids.len();
        let purse = coins(world, a);
        let evicted_by = world.comp::<Household>(a).and_then(|h| h.evicted_by).map(|(o, _)| o);
        // Class pay: a graded post holder or an exec keeps its tier.
        let min_tier = crate::systems::classpay::min_home_tier(world, a, home);
        if !lazy {
            memo.fill(world, work, &cost);
            if let Some(sw) = spouse_work {
                memo.fill(world, sw, &cost);
            }
        }
        // Review finding 9: the room, purse, tier and evictor filters first;
        // a Home's measure only for the Homes they leave (memoised by work
        // tile and Home), the work leg before the spouse's. The same pick:
        // the filters and the minimum are the eager walk's.
        let pick = crate::prof::timed!("reloc::pick", {
            let mut best: Option<(u64, EntityId)> = None;
            for (i, &(h, _, room)) in homes.iter().enumerate() {
                let open = h != home
                    && (min_tier == 0 || world.comp::<Building>(h).is_some_and(|b| b.tier >= min_tier))
                    && world.residents_of(h).len() + need <= room
                    && purse >= mult * crate::systems::ownership::rent_for(world, h)
                    && evicted_by.is_none_or(|o| o != world.owner_of(h));
                if !open {
                    continue;
                }
                let dd = memo.cost(world, work, i, &cost);
                if (fix && dd > cap) || dd + margin > d_pair {
                    continue;
                }
                let sp = spouse_work.map_or(0, |sw| memo.cost(world, sw, i, &cost));
                if dd + sp + margin > d_pair {
                    continue;
                }
                if best.is_none_or(|b| (dd + sp, h) < b) {
                    best = Some((dd + sp, h));
                }
            }
            best
        });
        // Hunger fix item 8 (P5g): no Home with room near work (the Farms'
        // neighbourhoods are full: 282 beds within 40 tiles of the 12 Farms
        // for 504 farmers on the flipped seed 42): trade Homes with a single
        // resident there whose own day the move does not lengthen past the
        // cap (no job, or a job no farther from the mover's Home), and who
        // can pay the mover's Home's first rent.
        let pick = match pick {
            Some(p) => Some(p),
            None if fix && spouse.is_none() && kids.is_empty() => {
                let mover = Mover { purse, evicted_by, min_tier };
                // The trade walks the Homes nearest first: the whole row.
                memo.fill(world, work, &cost);
                let near = NearHomes { homes: &homes, to_work: memo.row(work), order: memo.order(work) };
                if let Some((_, to, r)) =
                    crate::prof::timed!("reloc::swap", swap_partner(world, a, home, near, cap, d, margin, mover))
                {
                    let (an, rn) = (world.name_of(a), world.name_of(r));
                    let (to_name, from_name) = (world.name_of(to), world.name_of(home));
                    world.set_home(a, Some(to));
                    world.set_home(r, Some(home));
                    for m in [a, r] {
                        if let Some(h) = world.comp_mut::<Household>(m) {
                            h.rent_due = 0.0;
                        }
                    }
                    world.push_event(
                        EventKind::Housed,
                        &[a, to],
                        format!("{an} traded Homes with {rn} to live nearer work, into {to_name}"),
                    );
                    world.push_event(
                        EventKind::Housed,
                        &[r, home],
                        format!("{rn} traded Homes with {an}, into {from_name}"),
                    );
                    moved += 1;
                }
                continue;
            }
            None => None,
        };
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

/// Scale S1: per work tile, the measure from each Home door of a
/// relocation pass's list to it (`rows[tile].0[i]` for `doors[i]`) and the
/// list's indices nearest first (ties in list order). Review finding 9: a
/// row fills lazily, a Home at a time ([`DoorCosts::cost`]); `fill`
/// completes it and sorts the order (the trade's walk).
struct DoorCosts {
    doors: Vec<TilePos>,
    /// `UNKNOWN` until measured.
    rows: std::collections::BTreeMap<TilePos, (Vec<u64>, Vec<u32>)>,
}

/// A [`DoorCosts`] cell not measured yet (a real measure of `u64::MAX` is
/// measured again on each read: the same answer).
const UNKNOWN: u64 = u64::MAX;

impl DoorCosts {
    fn new(doors: Vec<TilePos>) -> DoorCosts {
        DoorCosts { doors, rows: std::collections::BTreeMap::new() }
    }

    /// The measure from Home `i`'s door to `to`, memoised.
    fn cost(&mut self, world: &World, to: TilePos, i: usize, cost: &impl Fn(&World, TilePos, TilePos) -> u64) -> u64 {
        let n = self.doors.len();
        let row = &mut self.rows.entry(to).or_insert_with(|| (vec![UNKNOWN; n], Vec::new())).0;
        if row[i] == UNKNOWN {
            row[i] = cost(world, self.doors[i], to);
        }
        row[i]
    }

    /// Every Home's measure to `to` and the order, nearest first.
    fn fill(&mut self, world: &World, to: TilePos, cost: &impl Fn(&World, TilePos, TilePos) -> u64) {
        let n = self.doors.len();
        let (row, order) = self.rows.entry(to).or_insert_with(|| (vec![UNKNOWN; n], Vec::new()));
        if !order.is_empty() || n == 0 {
            return;
        }
        for (i, c) in row.iter_mut().enumerate() {
            if *c == UNKNOWN {
                *c = cost(world, self.doors[i], to);
            }
        }
        *order = (0..n as u32).collect();
        order.sort_unstable_by_key(|&i| (row[i as usize], i));
    }

    /// The row `fill` made for `to` (empty if none).
    fn row(&self, to: TilePos) -> &[u64] {
        self.rows.get(&to).map_or(&[], |r| r.0.as_slice())
    }

    /// The row's indices, nearest first (empty if none).
    fn order(&self, to: TilePos) -> &[u32] {
        self.rows.get(&to).map_or(&[], |r| r.1.as_slice())
    }
}

/// Scale S1: a relocation pass's standing Homes (`(Home, door, capacity)`),
/// each one's measure to the mover's work, and their indices nearest first.
struct NearHomes<'a> {
    homes: &'a [(EntityId, TilePos, usize)],
    to_work: &'a [u64],
    order: &'a [u32],
}

/// The mover's side of a trade (`swap_partner`), as the plain pick reads
/// it: its purse (the first rent of the Home it takes), the landlord that
/// evicted it (never traded back into), and its tier floor (class pay).
struct Mover {
    purse: i64,
    evicted_by: Option<Option<EntityId>>,
    min_tier: u8,
}

/// Hunger fix item 8 (P5g): the swap for mover `a` (Home `home`, work door
/// `work`, commute `d`): the nearest Home within `cap` of work, the mover
/// can pay the first rent of, not its evictor's and at its tier floor,
/// holding a single adult resident (no spouse or child there) who is not
/// itself a worker nearer that Home than `home`, can pay `home`'s first
/// rent, keeps its own tier floor in `home` and was not evicted by `home`'s
/// landlord. `(distance, that Home, the resident)`. Scale S1: `near` is the
/// pass's standing Homes (`(Home, door, capacity)`, the Homes of the kind
/// list that are neither demolished nor derelict, in its order) with each
/// one's `commute_cost` to the mover's work and the list's indices nearest
/// first: the first Home in that order with a partner is the nearest (ties
/// in list order), and the walk stops past `cap` or the margin.
#[allow(clippy::too_many_arguments)]
fn swap_partner(
    world: &World,
    a: EntityId,
    home: EntityId,
    near: NearHomes<'_>,
    cap: u64,
    d: u64,
    margin: u64,
    mover: Mover,
) -> Option<(u64, EntityId, EntityId)> {
    let min_tier = mover.min_tier;
    let home_door = world.comp::<Building>(home)?.door;
    let home_tier = world.comp::<Building>(home)?.tier;
    let home_owner = world.owner_of(home);
    let home_rent = crate::systems::ownership::rent_for(world, home);
    let NearHomes { homes, to_work, order } = near;
    for &i in order {
        let (h, dd) = (homes[i as usize].0, to_work[i as usize]);
        // Nearest first: every Home after this one is as far or farther.
        if dd > cap || dd + margin > d {
            break;
        }
        let Some(b) = world.comp::<Building>(h).filter(|b| b.tier >= min_tier) else {
            continue;
        };
        if h == home {
            continue;
        }
        // The mover's checks, as the plain pick's.
        if mover.purse < crate::systems::ownership::rent_for(world, h)
            || mover.evicted_by.is_some_and(|o| o == world.owner_of(h))
        {
            continue;
        }
        let residents = world.residents_of(h);
        for &r in residents.iter() {
            if r == a || !world.has::<Brain>(r) || !crate::systems::demography::is_adult(world, r) {
                continue;
            }
            if world.spouse_of(r).is_some()
                || crate::systems::demography::children_of_agent(world, r).iter().any(|&c| home_of(world, c) == Some(h))
            {
                continue;
            }
            if world.has::<Sentence>(r) || world.comp::<Brain>(r).is_some_and(|br| br.emigrating) {
                continue;
            }
            // Its own day no worse: no job, or its work no nearer `h` than `home`.
            let ok = match world.comp::<Job>(r).and_then(|j| j.employer).and_then(|e| work_tile(world, r, e)) {
                None => true,
                Some(wt) => commute_cost(world, home_door, wt) <= commute_cost(world, b.door, wt),
            };
            if !ok || coins(world, r) < home_rent {
                continue;
            }
            // The partner's tier floor and evictor, as its own pick's.
            if crate::systems::classpay::min_home_tier(world, r, h) > home_tier
                || world.comp::<Household>(r).and_then(|x| x.evicted_by).is_some_and(|(o, _)| o == home_owner)
            {
                continue;
            }
            return Some((dd, h, r));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Execs
// ---------------------------------------------------------------------------

/// An exec's day salary: `exec_pay_frac` of the corp's treasury, floored at
/// `[economy] wage_exec` and capped at `exec_pay_cap` (the flat wage when off).
pub fn exec_pay(world: &World, corp: EntityId) -> i64 {
    let floor = world.config.economy.wage_exec;
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

/// L1b: the exec from `candidates`: among the eligible (`exec_score`), the
/// wealthiest tenth (at least 5), and of those the greediest (ties: the
/// exec skill, then the lower id). The corp brain reads its exec's greed
/// (Squeeze scores greed², Acquire greed): with wealth alone the execs were
/// a random draw of greed and no corp ever held Squeeze or bid hostile
/// (seeds 42-44: Squeeze 1 of 3, hostile Acquired 0).
pub fn pick_exec(world: &World, candidates: impl Iterator<Item = EntityId>) -> Option<EntityId> {
    let mut rich: Vec<((i64, i64, u32), EntityId)> =
        candidates.filter_map(|a| exec_score(world, a).map(|k| (k, a))).collect();
    rich.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
    let n = if world.config.life.exec_greed { (rich.len() / 10).max(5).min(rich.len()) } else { rich.len().min(1) };
    rich.truncate(n);
    let greed = |a: EntityId| world.comp::<crate::components::Personality>(a).map_or(0.0, |p| p.greed);
    rich.into_iter()
        .max_by(|x, y| greed(x.1).total_cmp(&greed(y.1)).then(x.0 .1.cmp(&y.0 .1)).then(y.1.cmp(&x.1)))
        .map(|(_, a)| a)
}

/// The corp's HQ for its exec's office hours: the corp's most senior
/// working building (its Hq first), the nearest the exec's Home of that
/// kind, then the lower id; a corp with none of the ranked kinds gives a
/// desk at any other working building it holds, never a Home or a Lot
/// (Shadow V4 review 7; bug 5: distance
/// came first and there was no Hq in the ranking, so CEOs held office at
/// Vat Farms and Arasaka's head moved to the Sump).
pub fn hq_of(world: &World, corp: EntityId, exec: EntityId) -> Option<EntityId> {
    const RANK: [BuildingKind; 11] = [
        BuildingKind::Hq,
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
            let bd = world.comp::<Building>(b).filter(|bd| {
                !bd.demolished && !bd.derelict && !matches!(bd.kind, BuildingKind::Home | BuildingKind::Lot)
            })?;
            let rank = RANK.iter().position(|&k| k == bd.kind).unwrap_or(RANK.len());
            Some((rank, from.map_or(0, |f| f.manhattan(bd.door)), b))
        })
        .min()
        .map(|(_, _, b)| b)
}

/// The corp `id` is the exec of.
pub fn exec_corp(world: &World, id: EntityId) -> Option<EntityId> {
    world.corp_slice().iter().copied().find(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(id)))
}

/// An exec's office day to keep: on, an exec of a corp with an HQ, not
/// jailed, today's office hours not yet kept, and the shift on (or about to
/// start within the walk).
pub fn exec_day_pending(world: &World, id: EntityId) -> Option<EntityId> {
    if world.has::<Sentence>(id) || world.has::<Job>(id) {
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
    // L2 shadow fixes item 1: the walk to the office, once started today,
    // holds (the CEO set out and turned back up to four times a morning).
    let started = world
        .comp::<Brain>(id)
        .and_then(|b| b.plan.as_ref())
        .filter(|p| p.goal == GoalKind::Work && p.target == Some(hq))
        .is_some_and(|p| p.started_tick / TICKS_PER_DAY == today);
    if started {
        return Some(hq);
    }
    let door = world.comp::<Building>(hq).map_or_else(Default::default, |b| b.door);
    // Review item 6: the trip to arrive 30 ticks before the office opens,
    // read against that deadline (`transit::trip_due`, the departure
    // gate's), not at "now": a now-based estimate moves with the timetable
    // and the 05:00 start of service, and turned the exec back.
    let until = u64::from(s).saturating_sub(tod);
    crate::systems::transit::trip_due(world, id, door, until, 30).then_some(hq)
}

/// Move an exec (with a spouse and children sharing the Home) into a free
/// Spire Home (tier 2) when they live lower down and one has room.
pub fn house_exec(world: &mut World, exec: EntityId) {
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
    // Shadow V4 bug 6: `EntityId::NONE` blocks every employer (a secure quit).
    world
        .comp::<Brain>(a)
        .and_then(|b| b.quit_from)
        .is_some_and(|(e, until)| (e == employer || e == EntityId::NONE) && world.tick < until)
}

/// Shadow V4 bug 6: a "no need of the wage" quit keeps the quitter out of
/// every hire for `quit_rehire_days` (another employer rehired him the
/// same midnight).
pub fn note_secure_quit(world: &mut World, a: EntityId) {
    let until = world.tick + world.config.life.quit_rehire_days * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(a) {
        b.quit_from = Some((EntityId::NONE, until));
    }
}

/// Record a quit (`quit_rehire_days` before that employer hires them again).
pub fn note_quit(world: &mut World, a: EntityId, employer: Option<EntityId>) {
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
    if !world.config.life.dole_in_place {
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
            }) && crate::systems::economy::dole_eligible(world, a)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TICKS_PER_DAY;

    /// Review finding 9: the lazy measure (the filters first, a Home's
    /// commute only for the Homes they leave) moves exactly whom the eager
    /// walk moved (every Home's commute to each work tile first): a city on
    /// day 5 with the move threshold cut to a third (more movers, the
    /// couples' two legs and the trades), byte for byte after the pass. The
    /// shipped city and the generated 4,000 city.
    #[test]
    fn test_lazy_relocation_matches_the_eager_walk() {
        for map in ["", "gen:4000"] {
            let mut c = crate::config::Config::load();
            if !map.is_empty() {
                c = c.scaled_to(4_000);
                c.world.map = map.into();
            }
            let mut w = World::new(42, c);
            w.run_ticks(5 * TICKS_PER_DAY + 3);
            w.config.life.commute_move_ticks /= 3;
            let housed = |x: &World| x.events.iter().filter(|e| e.kind == EventKind::Housed).count();
            let before = housed(&w);
            let (mut a, mut b) = (w.clone(), w);
            relocate_up_to_with(&mut a, usize::MAX, true);
            relocate_up_to_with(&mut b, usize::MAX, false);
            let moved = housed(&a) - before;
            println!("{map:?}: {moved} Housed events");
            assert!(moved >= 10, "the test moves people ({map:?}: {moved})");
            assert!(crate::save::to_ron(&a) == crate::save::to_ron(&b), "lazy == eager on {map:?}");
        }
    }

    /// Review finding 9, the measurement: the generated 20,000 city on day
    /// `CITYSIM_RELOC_DAYS` (default 5), one night's relocation pass (its
    /// daily limit) eager and lazy on clones of the same world, three times
    /// each: wall time and the same answer. Prints.
    #[test]
    #[ignore]
    fn measure_relocation_lazy_vs_eager() {
        let days: u64 = std::env::var("CITYSIM_RELOC_DAYS").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        let mut c = crate::config::Config::load().scaled_to(20_000);
        c.world.map = "gen:20000".into();
        let mut w = World::new(42, c);
        w.run_ticks(days * TICKS_PER_DAY + 3);
        let pop = w.citizens().len() as f32;
        let limit = (w.config.life.relocate_per_1000 * pop / 1000.0).round().max(w.config.life.relocate_per_day as f32)
            as usize;
        let mut out = Vec::new();
        for lazy in [false, true, false, true, false, true] {
            let mut x = w.clone();
            let t = std::time::Instant::now();
            relocate_up_to_with(&mut x, limit, lazy);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            println!("relocation {} day {days} limit {limit}: {ms:.1} ms", if lazy { "lazy " } else { "eager" });
            out.push(crate::save::to_ron(&x));
        }
        assert!(out.windows(2).all(|p| p[0] == p[1]), "lazy == eager");
    }
}
