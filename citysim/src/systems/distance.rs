//! The island pass phase 1 (docs/ISLAND_PASS.md I1; OPEN-163 part):
//! distance in the job and bed picks.
//!
//! A destination pick scores each candidate by what it offers against the
//! estimated travel time from where the person will set out, and drops a
//! candidate past the purpose's hard cap (`[distance]`) while one within it
//! is left; with none within, the nearest wins ([`pick`]).
//!
//! The travel estimate is [`travel`]: `transit::commute_ticks`, the leg
//! planner's own estimate (the walk at `[exec] move_ticks_full`, or the El
//! where it wins: `stop_candidates`² hop reads from the network's
//! precomputed stop-to-stop table). No path search per candidate, so it
//! holds at 20k; the same measure the hiring band, the relocation and the
//! housing demand already read.
//!
//! The sites: the job (`demography::job_search` and its pool, the graded
//! pick, the seeded staff at world creation, the corner's day labour, the
//! chip posts) and the bed (`ownership::rehouse` for a homeless worker,
//! `life::hideout_bed`, which `plan::bed_of` reserves). The hangout and the
//! Hotel are phase 2; the move-nearer pressure is phase 4 (`systems/commute.rs`).
//!
//! Phase 2 (`[distance.free]`, `CITYSIM_DISTANCE2_OFF=1` drops it): the
//! HangOut spot (`leisure::best_spot_kind`, the think's nearest spot, the
//! Socialise HangOut) is scored by this travel and dropped past
//! `spot_cap_ticks`; with none within there is no spot (a discretionary
//! hour is not walked for: the own Home stays, as it never was capped).
//! Tonight's bed out for the homeless (`street::hotel_for`) is a
//! Mission's cot or a Hotel bed by [`pick`] (the cot's offer
//! `cot_bed_ticks`, the cap `hotel_cap_ticks`), and a cot is walked to as
//! `LocationKey::Cot`, never as a "Hotel" (OPEN-171).
//!
//! Phase 2's Statistical mirror: none needed. A Statistical body takes
//! no HangOut walk and books no bed by plan (the evening and the night are
//! the table's, off camera; `leisure::stat_daily` reads the paid venues
//! only, whose walk cap `[leisure] venue_walk_cap_ticks` is unchanged).
//!
//! Statistical mirror: none needed. The job and housing picks are
//! tier-blind (a Statistical adult is hired and housed by the same code),
//! so the cap reaches both tiers alike; the Hideout bed is a Full or Coarse
//! planner's context (a Statistical body sleeps by the table, off camera).
//! No random draw. `CITYSIM_DISTANCE_OFF=1` drops `[distance]` and every
//! pick is the base one.

use crate::components::{Building, Household, Job, Position, TilePos};
use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;

/// `[distance]` on.
pub fn on(world: &World) -> bool {
    world.config.distance.is_some()
}

/// The travel estimate in ticks from `from` to `to` (see the module).
pub fn travel(world: &World, from: TilePos, to: TilePos) -> Tick {
    crate::systems::transit::commute_ticks(world, from, to)
}

/// The island pass phase 2's `[distance.free]` (the free-time spot and
/// the Hotel), `None` off (either switch).
pub fn free(world: &World) -> Option<&crate::config::FreeDistanceCfg> {
    world.config.distance.as_ref().and_then(|d| d.free.as_ref())
}

/// The job's cap, `None` off.
pub fn job_cap(world: &World) -> Option<Tick> {
    world.config.distance.as_ref().map(|d| d.job_cap_ticks)
}

/// A job candidate setting out from `from` is past the job's cap to the
/// workplace door (`false` off).
pub fn past_job_cap(world: &World, from: TilePos, door: TilePos) -> bool {
    job_cap(world).is_some_and(|c| travel(world, from, door) > c)
}

/// Where `id` sets out for work: its Home door, else its tile.
pub fn set_out(world: &World, id: EntityId) -> Option<TilePos> {
    world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
}

/// `id`'s workplace door, if it holds a post at a standing building.
pub fn work_door(world: &World, id: EntityId) -> Option<TilePos> {
    let employer = world.comp::<Job>(id)?.employer?;
    world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door)
}

/// The pick: each candidate is `(travel, score, item)`, the lower score
/// better (the travel less what it offers, in ticks). Those past `cap` are
/// dropped while one within it is left; the best `(score, item)` within the
/// cap wins, and with none within, the nearest `(travel, item)`. Ties go to
/// the lower item, so the pick is deterministic.
pub fn pick<T: Ord + Copy>(cands: impl IntoIterator<Item = (Tick, i64, T)>, cap: Tick) -> Option<T> {
    let mut within: Option<(i64, T)> = None;
    let mut nearest: Option<(Tick, T)> = None;
    for (t, score, item) in cands {
        if t <= cap {
            if within.is_none_or(|b| (score, item) < b) {
                within = Some((score, item));
            }
        } else if within.is_none() && nearest.is_none_or(|b| (t, item) < b) {
            nearest = Some((t, item));
        }
    }
    within.map(|(_, x)| x).or(nearest.map(|(_, x)| x))
}

#[cfg(test)]
mod tests {
    use super::pick;

    #[test]
    fn test_pick_drops_past_the_cap_unless_none_is_left() {
        // Equal offer: the near one.
        assert_eq!(pick([(200, 200, 1u32), (40, 40, 2)], 120), Some(2));
        // A better offer within the cap beats a nearer one.
        assert_eq!(pick([(30, 30, 1u32), (90, 10, 2)], 120), Some(2));
        // A past-cap candidate loses to any within, whatever it offers.
        assert_eq!(pick([(130, -500, 1u32), (110, 110, 2)], 120), Some(2));
        // None within: the nearest wins, not the best score.
        assert_eq!(pick([(300, -500, 1u32), (200, 200, 2)], 120), Some(2));
        // The only candidate past the cap is still picked.
        assert_eq!(pick([(900, 900, 7u32)], 120), Some(7));
        assert_eq!(pick(std::iter::empty::<(u64, i64, u32)>(), 120), None);
    }
}
