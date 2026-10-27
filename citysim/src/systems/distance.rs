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
//! Phase 2 (`[distance.free]`): the
//! HangOut spot (`leisure::best_spot_kind`, the think's nearest spot, the
//! Socialise HangOut) is scored by this travel and dropped past
//! `spot_cap_ticks`; with none within there is no spot (a discretionary
//! hour is not walked for: the own Home stays, as it never was capped).
//! Tonight's bed out for the homeless (`street::hotel_for`) is a
//! Mission's cot or a Hotel bed by [`pick`] (the cot's offer
//! `cot_bed_ticks`, the cap `hotel_cap_ticks`), and a cot is walked to as
//! `LocationKey::Cot`, never as a "Hotel" (OPEN-171).
//!
//! Island fix round F3 (`[distance.free.stay_in]`;
//! OPEN-190, Dylan's D1 B): the stay-in has a fallback. Phase 2
//! offered the own Home only with company due (`household::company_due`),
//! so past the cap a person living alone, or with a household out, had no
//! Unwind at all (Shadow V8: a joytoy's fun 0.95 to 0.00). Now, with no
//! spot within the cap, the Unwind's pick is the own Home's HangOut
//! (`leisure::stay_in_home`): flatmates and Block-mates inside are its
//! company (a first meeting forms an edge), else it runs alone at
//! `fun_hour`, never left early as an empty spot. Its Statistical mirror is
//! the table's free evening (`leisure::stat_daily`'s `free_stat`, with no
//! spot and no cap), unchanged: the fallback brings the Full tier back
//! toward it. No random draw.
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
//! No random draw. With `[distance]` absent every
//! pick is the base one.
//!
//! The pressure pass phase 4 (`[distance.reach]`, OPEN-194): the
//! non-urgent trips the caps above never covered. ReportCrime and Treat are
//! not scored, and Found is skipped, with the Hall or the nearest Clinic
//! past its reach ([`out_of_reach`]); a Hunt's or a hit Contract's ask
//! venue past `stakeout_ticks` is the hunter's own district's busiest Bar,
//! and a stakeout past it is not planned (`hunt::within_reach`); a
//! sweeper's beat is dealt within `beat_ticks` of its post
//! (`districts::sanitation`). Tier-blind (a Statistical body does not
//! score these goals or walk a beat). No random draw.
//!
//! Pressure pass round 3, S1 (`[distance.choice]`, OPEN-221): distance in
//! the goals' place picks. Every body-specific reach reads [`trip`], the
//! body's own trip (the El only when he would board it and it runs, else the
//! walk; [`trip_ahead`] for a trip planned at midnight). The picks: a
//! dealer's spot today, the nearest of his gang's held districts' spots
//! (`stims::site_today`); a member's order work, its Home or derelict
//! (`gang::gang_work_target`), the pimp's round pitch (`vice::round_pitch`),
//! the preacher's spot (`leisure::preach_plan`); the muster (`crew::post_gang`,
//! `raid::muster_in_reach` in the Raid goal) on `[crew] duty_reach_ticks`;
//! the rival Hideout a raid or retaliation marches to on `[crew]
//! raid_target_reach_ticks` (`faction`); the hire's hard cap
//! (`demography::job_search_round`, the corner's dawn pick); the Builder's
//! far site day (`shift::far_site`); a kin visit and the week's Camp answer
//! (`household`); the gang a recruit tries out for (`gang::recruit_gang`, on
//! `[crew] tryout_reach_ticks`). Past the reach the body does not set out:
//! the nearer option, else another goal, and a Crew or Visited line says so
//! for the muster, the dealer's day ([`daily`]) and the Camp answer.
//! Statistical mirror: the hire, the site, the Camp answer and the raid's
//! rival are tier-blind; the dealer's spot, the muster and the order work
//! are a body's walk (a Statistical body deals and raids by its table).

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

/// Island fix round F3's `[distance.free.stay_in]` (OPEN-190), `None` when
/// it, `[distance.free]` or `[distance]` is absent.
pub fn stay_in(world: &World) -> Option<&crate::config::StayInCfg> {
    free(world).and_then(|f| f.stay_in.as_ref())
}

/// The pressure pass phase 4's `[distance.reach]` (OPEN-194), `None` off.
pub fn reach(world: &World) -> Option<&crate::config::ReachCfg> {
    world.config.distance.as_ref().and_then(|d| d.reach.as_ref())
}

/// The pressure pass phase 4 (OPEN-194): building `b` is past the reach
/// `cap` reads from `[distance.reach]`, from where `id` sets out (inside
/// it: never). `false` off, or with no origin or door.
pub fn out_of_reach(world: &World, id: EntityId, b: EntityId, cap: impl Fn(&crate::config::ReachCfg) -> Tick) -> bool {
    let Some(r) = reach(world) else { return false };
    if world.comp::<Position>(id).and_then(|p| p.building) == Some(b) {
        return false;
    }
    trip_to(world, id, b).is_some_and(|t| t > cap(r))
}

/// Pressure pass round 3, S1's `[distance.choice]` (OPEN-221), `None` off.
pub fn choice(world: &World) -> Option<&crate::config::ChoiceCfg> {
    world.config.distance.as_ref().and_then(|d| d.choice.as_ref())
}

/// S1 (OPEN-221): the trip `id` would make from `from` to `to` setting out
/// now, in ticks: [`travel`] when he would board the El (`transit::
/// pays_or_dodges`: the fare in hand, or the nerve to jump the gate, and no
/// ban) and it runs now and at the estimate's end (`transit::in_service`),
/// else the walk (Manhattan at `[exec] move_ticks_full`). [`travel`] exactly
/// off. The night's danger test is not read. 20k: JoinGang legs of 400+
/// tiles under the 180-tick try-out reach, the reach read the El for a body
/// with no fare or after the last train (`noroute`, 00:00-04:00), and the
/// walk was the leg.
pub fn trip(world: &World, id: EntityId, from: TilePos, to: TilePos) -> Tick {
    trip_when(world, id, from, to, Some(world.tick))
}

/// S1: [`trip`] for a trip planned ahead (a duty posted at midnight, the
/// week's Camp answer): the El's hours not read.
pub fn trip_ahead(world: &World, id: EntityId, from: TilePos, to: TilePos) -> Tick {
    trip_when(world, id, from, to, None)
}

fn trip_when(world: &World, id: EntityId, from: TilePos, to: TilePos, now: Option<Tick>) -> Tick {
    trip_by(world, from, to, || rider(world, id), now)
}

/// S1: `id` would board the El now (`transit::pays_or_dodges`), for a
/// caller reading many trips of one body ([`trip_by`]).
pub fn rider(world: &World, id: EntityId) -> bool {
    crate::systems::transit::pays_or_dodges(world, id).is_some()
}

/// S1: [`trip`] with the rider test supplied (read only when the El would
/// win), `now` `None` for a trip planned ahead.
pub fn trip_by(world: &World, from: TilePos, to: TilePos, rides: impl FnOnce() -> bool, now: Option<Tick>) -> Tick {
    let t = travel(world, from, to);
    let walk = Tick::from(from.manhattan(to)) * world.config.exec.move_ticks_full;
    if choice(world).is_none() || t >= walk {
        return t;
    }
    let runs = now.is_none_or(|n| {
        crate::systems::transit::in_service(world, n) && crate::systems::transit::in_service(world, n + t)
    });
    if runs && rides() {
        t
    } else {
        walk
    }
}

/// S1: [`trip`] from where `id` sets out now (`life::origin`) to building
/// `b`'s door (0 inside it); `None` with no origin or door.
pub fn trip_to(world: &World, id: EntityId, b: EntityId) -> Option<Tick> {
    if world.comp::<Position>(id).and_then(|p| p.building) == Some(b) {
        return Some(0);
    }
    let from = crate::systems::life::origin(world, id)?;
    let door = world.comp::<Building>(b)?.door;
    Some(trip(world, id, from, door))
}

/// S1 (OPEN-221): a duration for the diary ("2 h 05").
pub fn hours(t: Tick) -> String {
    use crate::time::TICKS_PER_HOUR;
    format!("{} h {:02}", t / TICKS_PER_HOUR, t % TICKS_PER_HOUR)
}

/// S1 (OPEN-221), the LivingEarly midnight after the crew's duties: a gang
/// dealer with a body whose nearest spot today lies past `[distance.choice]
/// deal_ticks` of his Home is off it today (`stims::site_today` reads the
/// same pick), and his diary says why: one Crew line a day. Nothing off.
pub fn daily(world: &mut World) {
    let Some(cap) = choice(world).map(|c| c.deal_ticks) else { return };
    if !crate::systems::stims::on(world) {
        return;
    }
    let mut lines: Vec<(EntityId, EntityId, String)> = Vec::new();
    for &gang in world.gang_list() {
        let Some(g) = world.comp::<crate::components::Gang>(gang) else { continue };
        for &m in &g.members {
            let bodied =
                world.comp::<crate::components::Brain>(m).is_some_and(|b| b.lod != crate::components::Lod::Statistical);
            if !bodied
                || world.has::<crate::components::Sentence>(m)
                || !crate::systems::stims::is_dealer(world, m)
                || crate::systems::gang::following_order(world, m).is_none()
                || !crate::systems::stims::stocked(world, m, gang)
            {
                continue;
            }
            let Some(from) = set_out(world, m) else { continue };
            let Some((site, t)) = crate::systems::stims::member_site_trip(world, m, gang, from, cap, false) else {
                continue;
            };
            if t <= cap {
                continue;
            }
            let at = match site {
                crate::systems::stims::DealSite::Venue(b) => world.name_of(b),
                crate::systems::stims::DealSite::Corner(c) => {
                    format!("the corner in {}", world.district_name(world.district_of(c)))
                }
            };
            let text = format!(
                "{} stays off {}'s trade today: the nearest spot, {at}, is {} from home",
                world.name_of(m),
                g.name,
                hours(t)
            );
            lines.push((m, gang, text));
        }
    }
    for (m, gang, text) in lines {
        world.push_event(crate::events::EventKind::Crew, &[m, gang], text);
        crate::systems::ownership::jobs2_row(world).choice_deal_off += 1;
    }
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
