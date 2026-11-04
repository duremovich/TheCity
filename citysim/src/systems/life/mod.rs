//! Life pass L1 (docs/history/SHADOW_V1.md): the helpers the goals, the planner
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

mod execs;
mod housing;
mod relocate;

pub use execs::*;
pub use housing::*;
pub use relocate::*;

/// Ticks to walk `tiles` (Manhattan) on foot at the pace (`[exec]
/// walk_ticks`), padded by half for the detours a road grid forces (as the
/// commute gate).
pub fn walk_ticks(world: &World, tiles: u32) -> u64 {
    world.config.exec.walk_ticks(u64::from(tiles) * 3 / 2)
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

/// The pre-view pass phase 5 (OPEN-140's remainder, `[legs] bed_by_ride`):
/// the trip home in walking tiles' worth: the walk, or the ride where it is
/// shorter (the El's `transit::commute_ticks`, the exec's car home:
/// `streetwork::ride_home_ticks`), at the pace (`[exec] walk_tiles`). The walk's
/// tiles exactly with `[legs]` off.
pub fn home_reach(world: &World, id: EntityId, home: EntityId) -> Option<u32> {
    let tiles = tiles_to(world, id, home)?;
    if tiles == 0 || !world.config.legs.as_ref().is_some_and(|l| l.bed_by_ride) {
        return Some(tiles);
    }
    let (from, door) = (origin(world, id)?, world.comp::<Building>(home)?.door);
    let mut ticks = crate::systems::transit::commute_ticks(world, from, door);
    if let Some(c) = crate::systems::streetwork::ride_home_ticks(world, id, from, door) {
        ticks = ticks.min(c);
    }
    Some(u32::try_from(world.config.exec.walk_tiles(ticks)).unwrap_or(u32::MAX).min(tiles))
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
// The housing churn (the pre-view pass phase 1, OPEN-140)
// ---------------------------------------------------------------------------

/// Every `Housed` line (`push_event`): its body is housed at this tick
/// (`housed_now`), and, when it stands outside the Home named, walks there
/// before it idles or sleeps elsewhere (`moving_in`).
pub fn on_housed(world: &mut World, who: Option<EntityId>, home: Option<EntityId>) {
    let Some(who) = who else { return };
    let tick = world.tick;
    if world.housed_now.0 != tick {
        world.housed_now = (tick, std::collections::BTreeSet::new());
    }
    world.housed_now.1.insert(who);
    let Some(h) = home.filter(|&h| home_of(world, who) == Some(h)) else { return };
    if world.comp::<crate::components::Position>(who).is_some_and(|p| p.building != Some(h)) {
        world.moving_in.insert(who, h);
    }
}

/// A `Housed` line named `id` at this tick: one writer per body per
/// midnight, so the kin take-in, the Spire and the "nearer work"
/// relocation leave a body the midnight already moved (the first build of
/// each moved it and the next undid it in the same minute: Shadow V6 § 5).
pub fn housed_now(world: &World, id: EntityId) -> bool {
    world.housed_now.0 == world.tick && world.housed_now.1.contains(&id)
}

/// The Home `id` was moved into and has not walked into yet (OPEN-140: a
/// day-0 retiree's `move_in` left the body wandering 9 h in the old
/// district and sleeping in a Soup Kitchen).
pub fn moving_in(world: &World, id: EntityId) -> Option<EntityId> {
    world.moving_in.get(&id).copied().filter(|&h| home_of(world, id) == Some(h))
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
    // The city milestone C8: his own Hideout (the nearest of the gang's to his Home).
    let h = crate::systems::vertical::own_hideout(world, id)?;
    let b = world.comp::<Building>(h)?;
    if b.is_full() && !b.occupants.contains(&id) {
        return None;
    }
    // L2 (L30): a bed another member reserved on the way is taken.
    if !b.occupants.contains(&id) && b.occupants.len() + world.reserved_at(h, id) >= usize::from(b.capacity) {
        return None;
    }
    let t = tiles_to(world, id, h)?;
    // The island fix round F2 (OPEN-191, D2 = A): a housed member's Hideout
    // only within the walk and tomorrow's commute.
    if crate::systems::beds::hideout_ok(world, id, h) == Some(false) {
        return None;
    }
    // The island pass phase 1 (OPEN-163): the bed is scored by the travel
    // from here (`distance::travel`; 0 inside), the Home's offer
    // (`[distance] home_bed_ticks`) off its score; a bed past
    // `bed_cap_ticks` is dropped while the other is within it, and with
    // neither within, the nearer (ties the Home). Homeless: the Hideout.
    if let Some(cfg) = world.config.distance.as_ref() {
        let Some(home) = home_of(world, id) else { return Some(h) };
        let here = origin(world, id)?;
        let at = |b: EntityId| -> Option<Tick> {
            if world.comp::<Position>(id).and_then(|p| p.building) == Some(b) {
                return Some(0);
            }
            Some(crate::systems::distance::travel(world, here, world.comp::<Building>(b)?.door))
        };
        let Some(th) = at(home) else { return Some(h) };
        let tc = at(h)?;
        let offer = cfg.home_bed_ticks as i64;
        // `(travel, score, item)`: the Home is item 0, so it wins a tie.
        let cands = [(th, th as i64 - offer, 0u8), (tc, tc as i64, 1u8)];
        return (crate::systems::distance::pick(cands, cfg.bed_cap_ticks) == Some(1)).then_some(h);
    }
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
    // OPEN-140: a body moved in sleeps in its new Home first.
    if moving_in(world, id).is_some() {
        return None;
    }
    let home_t = home_reach(world, id, home)?;
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
    earn_short_if(world, id, broke(world, id))
}

/// [`earn_short`] with `broke` read by the caller (the view milestone V2b:
/// the off-screen hour read it twice, each a Market search).
pub fn earn_short_if(world: &World, id: EntityId, broke: bool) -> bool {
    if broke {
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
/// guard per (guard, suspect) pair (review fix). The perf pass phase 5: the
/// scan `World::claimed_by_other` replaces (the law reads the claim index;
/// a unit test's `spatial::scans()` reads this).
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
    let free = crate::systems::law::living(world, suspect) && !world.has::<Sentence>(suspect);
    let visible = free && crate::systems::law::near(world, guard, suspect, sight + 2);
    // Pressure pass F2: out of his sight, a fresh sighting by another guard
    // (`law::sightings`) elsewhere is the radio: he chases on to it (13 of 16
    // of a 100k guard's Arrests walked 20-45 min to a tile the suspect had
    // left and failed `PreconditionLost`).
    let now = world.tick;
    let here = world.comp::<Position>(guard).map(|p| p.tile);
    // Pressure pass round 7, W3 (`OPEN-244`): a radioed sighting past his
    // pursuit radius is not his to walk to (a guard chased one 259 tiles
    // across the city, 4 h 46, on a rest day): the chase drops and is handed
    // to a guard near it (`law::chase_dropped`).
    let reach = crate::systems::law::pursuit_radius_for(world, guard);
    let radioed = free
        && world.last_seen().get(&suspect).is_some_and(|&(t, at)| {
            now.saturating_sub(at) <= world.config.life.chase_radio_ticks
                && Some(t) != here
                && here.is_some_and(|h| h.manhattan(t) <= reach)
        });
    if (visible || radioed) && hops < world.config.life.chase_hops {
        if visible {
            let tile = world.comp::<Position>(suspect).map(|p| p.tile).unwrap_or_default();
            world.set_last_seen(suspect, (tile, now));
        }
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
    // Round 7 W3: a fresh sighting elsewhere (past his reach) stands for the
    // guard near it.
    let elsewhere = free
        && world
            .last_seen()
            .get(&suspect)
            .is_some_and(|&(t, at)| now.saturating_sub(at) <= world.config.life.chase_radio_ticks && Some(t) != here);
    if elsewhere {
        return false;
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
    saw_within(world, w, actor, crime, world.config.life.witness_dedupe_ticks)
}

/// `w` holds a `SawCrime` of `actor` for `crime` from the last `window` ticks.
pub fn saw_within(world: &World, w: EntityId, actor: EntityId, crime: crate::components::Crime, window: Tick) -> bool {
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
        // The two maps side by side (the change tier's budget, 2026-10-19).
        let one = |map: &str| {
            let mut c = crate::config::Config::load();
            // Round 9 Y4: R2's pace, at which the 2k and 4k cities' commutes
            // were far enough to move and hire people (at 21 tiles a minute
            // nobody there is past `commute_move_ticks`: 2 Housed events).
            c.exec.walk_tiles_per_tick = 1;
            c.exec.step_res = 4;
            // ... and the El at R2's speed (Y4's 30 km/h El shortens the
            // commutes the moves read).
            c.transit.ride_ticks_per_tile = 0.25;
            c.transit.dwell_ticks = 1.0;
            c.transit.stop_spacing = 16;
            c.life.bed_margin_tiles = 20;
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
            relocate_up_to_with(&mut a, usize::MAX, true, false);
            relocate_up_to_with(&mut b, usize::MAX, false, false);
            let moved = housed(&a) - before;
            println!("{map:?}: {moved} Housed events");
            assert!(moved >= 10, "the test moves people ({map:?}: {moved})");
            assert!(crate::save::to_ron(&a) == crate::save::to_ron(&b), "lazy == eager on {map:?}");
        };
        std::thread::scope(|s| {
            let hs = ["", "gen:4000"].map(|map| s.spawn(move || one(map)));
            for h in hs {
                h.join().expect("a relocation city");
            }
        });
    }

    /// The perf pass phase 3 (the world build): the city built with the
    /// indexed reads (relocation's Homes, the labour pool, the graded pick,
    /// the free beds, the housing build's Lots and storeys) is the city built
    /// with the scans they replaced (`spatial::scans`), byte for byte: the
    /// same hires in the same order, the same movers to the same Homes, over
    /// the pre-roll's rounds of builds (an index left stale by a build, a
    /// hire or a move shows here; the perf pass phase 7: and
    /// `districts::rebuild`'s grid memo, built every call under the scans).
    /// The shipped city and the generated 4,000
    /// city. `band_refusals` is cleared on both sides: the scan's
    /// `pick_candidate` counts none (the pooled walk's count is pinned by
    /// the save identity against the base binary).
    #[test]
    fn test_indexed_build_matches_the_scans() {
        let one = |map: &str| {
            let mut c = crate::config::Config::load();
            // Round 9 Y4: R2's pace, at which the 2k and 4k cities' commutes
            // were far enough to move and hire people (at 21 tiles a minute
            // nobody there is past `commute_move_ticks`: 2 Housed events).
            c.exec.walk_tiles_per_tick = 1;
            c.exec.step_res = 4;
            // ... and the El at R2's speed (Y4's 30 km/h El shortens the
            // commutes the moves read).
            c.transit.ride_ticks_per_tile = 0.25;
            c.transit.dwell_ticks = 1.0;
            c.transit.stop_spacing = 16;
            c.life.bed_margin_tiles = 20;
            if !map.is_empty() {
                c = c.scaled_to(4_000);
                c.world.map = map.into();
            }
            let build = |scans: bool| {
                crate::spatial::SCANS.with(|s| s.set(scans));
                let mut w = World::new(42, c.clone());
                crate::spatial::SCANS.with(|s| s.set(false));
                if let Some(j) = w.stats.current.jobs2.as_mut() {
                    j.band_refusals = 0;
                }
                w
            };
            let (a, b) = (build(false), build(true));
            let count = |k: EventKind| a.events.iter().filter(|e| e.kind == k).count();
            let (housed, hired) = (count(EventKind::Housed), count(EventKind::Hire));
            println!("{map:?}: {housed} Housed, {hired} Hire events");
            assert!(housed >= 50 && hired >= 100, "the build moves and hires ({map:?}: {housed}, {hired})");
            assert!(crate::save::to_ron(&a) == crate::save::to_ron(&b), "indexed == scans on {map:?}");
        };
        std::thread::scope(|s| {
            let hs = ["", "gen:4000"].map(|map| s.spawn(move || one(map)));
            for h in hs {
                h.join().expect("a built city");
            }
        });
    }

    /// Review finding 9, the measurement: the generated 20,000 city on day
    /// `CITYSIM_RELOC_DAYS` (default 5), one night's relocation pass (its
    /// daily limit) eager and lazy on clones of the same world, three times
    /// each: wall time and the same answer. Prints.
    #[test]
    #[ignore = "tier: diag"]
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
            relocate_up_to_with(&mut x, limit, lazy, false);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            println!("relocation {} day {days} limit {limit}: {ms:.1} ms", if lazy { "lazy " } else { "eager" });
            out.push(crate::save::to_ron(&x));
        }
        assert!(out.windows(2).all(|p| p[0] == p[1]), "lazy == eager");
    }
}
