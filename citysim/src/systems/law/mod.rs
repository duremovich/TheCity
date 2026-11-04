//! Law and jail: witnessing, crime reports and warrants, suspect sightings,
//! arrest and escort, sentencing with jail capacity, release, jail upkeep,
//! and the player's Arrest / Release commands.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Crime, CrimeReport, GoalKind, Job, Kit, LawShock, MemoryKind, Needs, Personality,
    Position, Posture, Role, Sentence, Skills, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::ActionKind;
use crate::personality::Drift;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// Chebyshev distance between two tiles.
/// Lives at work phase 7 (§ 5.2): the `Jail` key's building and a Station
/// guard's post (`systems::stations`).
pub use crate::systems::stations::{jail_for, station_post};

mod arrest;
mod chase;
mod court;
mod crime;
mod jail;
mod patrol;
mod sightings;
mod word;

pub use arrest::*;
pub use chase::*;
pub use court::*;
pub use crime::*;
pub use jail::*;
pub use patrol::*;
use sightings::*;
pub use word::*;

pub fn chebyshev(a: TilePos, b: TilePos) -> u32 {
    let dx = (i32::from(a.x) - i32::from(b.x)).unsigned_abs();
    let dy = (i32::from(a.y) - i32::from(b.y)).unsigned_abs();
    dx.max(dy)
}

/// Are two agents within `r` Chebyshev tiles, or in the same building?
pub fn near(world: &World, a: EntityId, b: EntityId, r: u32) -> bool {
    let (Some(pa), Some(pb)) = (world.comp::<Position>(a), world.comp::<Position>(b)) else { return false };
    (pa.building.is_some() && pa.building == pb.building) || chebyshev(pa.tile, pb.tile) <= r
}

/// Round 8 X1 (`OPEN-246`): a wall of the cells stands between `a` and `b`:
/// one of them is held (a `Sentence`) and they are not in the same building
/// (a cellmate is within reach; the victim at the Precinct's door, the
/// street, a passer-by are not).
pub fn held_apart(world: &World, a: EntityId, b: EntityId) -> bool {
    if !world.has::<Sentence>(a) && !world.has::<Sentence>(b) {
        return false;
    }
    let here = |x: EntityId| world.comp::<Position>(x).and_then(|p| p.building);
    here(a).is_none() || here(a) != here(b)
}

/// Round 5 U4 (`OPEN-236`): is the spot `(tile, building)` in the open?
/// Outside any building, on a building's door tile (the threshold sees both
/// ways), or inside a wall-less kind (`[crime] open_air`).
pub fn outdoors(world: &World, tile: TilePos, building: Option<EntityId>) -> bool {
    let Some(b) = building else { return true };
    world.comp::<Building>(b).is_none_or(|bd| bd.door == tile || world.config.crime.open_air.contains(&bd.kind))
}

/// Round 5 U4 (`OPEN-236`): who is indoors sees indoors. A body at `p` sees
/// a scene at `(tile, building)` when both are in the same building, or both
/// are [`outdoors`] within `reach` Chebyshev tiles; never through a wall (the
/// cells do not see the El gate, the street does not see into a flat).
/// `scene_out` is `outdoors(tile, building)`, read once by the caller.
pub fn sees_scene(
    world: &World,
    p: &Position,
    tile: TilePos,
    building: Option<EntityId>,
    scene_out: bool,
    reach: u32,
) -> bool {
    if building.is_some() && p.building == building {
        return true;
    }
    scene_out && chebyshev(p.tile, tile) <= reach && outdoors(world, p.tile, p.building)
}

/// A live agent: the entity exists and still has a Brain (corpses keep
/// Position and Identity, so `is_alive` alone is not enough).
pub fn living(world: &World, id: EntityId) -> bool {
    world.is_alive(id) && world.has::<Brain>(id)
}

pub fn is_guard(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)
}

/// Pressure pass round 9, Y2 (`OPEN-250`): `id` is on leave from the post
/// now: mourning or lying low (`grief::holed_up`), or today's shift struck
/// (the employer's leave, an absence, a strike: `classes::walk_out`). V17
/// `bereaved_6559 d21 09:19`: a guard a day widowed, on leave, made arrests.
pub fn on_leave(world: &World, id: EntityId) -> bool {
    crate::systems::grief::holed_up(world, id)
        || world.comp::<Job>(id).is_some_and(|j| j.struck_shift.is_some_and(|k| k == j.shift_key_at(world.tick)))
}

/// Pressure pass F2: a guard on shift now (the law at work, whose sighting
/// of a crime is a report).
pub fn on_duty(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard && j.on_shift(world.tick_of_day()))
        && !on_leave(world, id)
}

/// Guards (any LOD) within `r` tiles of `id`, excluding `id`.
pub fn guard_within(world: &World, id: EntityId, r: u32) -> bool {
    world.guards().iter().any(|&g| g != id && near(world, id, g, r))
}

/// The witness notice probability: `0.6 − 0.5 × stealth + 0.3 × guard`.
pub fn notice_probability(cfg: &crate::config::CrimeCfg, actor_stealth: f32, witness_is_guard: bool) -> f32 {
    let p = cfg.witness_base - cfg.witness_stealth_factor * actor_stealth
        + if witness_is_guard { cfg.witness_guard_bonus } else { 0.0 };
    p.clamp(0.0, 1.0)
}

pub fn crime_salience(crime: Crime) -> f32 {
    match crime {
        Crime::Theft => 0.5,
        Crime::Extortion => 0.6,
        Crime::Assault => 0.8,
        Crime::Murder => 1.0,
        Crime::Vagrancy => 0.2,
        // Transit T10: petty.
        Crime::FareEvasion => 0.1,
        // M13 D47.
        Crime::GrandTheft => 0.6,
        Crime::Manslaughter => 0.8,
        Crime::Abduction => 0.9,
        // M13 D38.
        Crime::Dealing => 0.5,
        // M14 V18.
        Crime::Intrusion => 0.4,
        Crime::DataTheft => 0.6,
        // M16a (plan C29).
        Crime::Conspiracy => 0.8,
        // M16b § 4.
        Crime::Fraud => 0.4,
        Crime::Blackmail => 0.5,
        // Jobs & lives phase 3 (§ 4).
        Crime::ArmsDealing => 0.6,
        Crime::UnlicensedArms => 0.3,
        Crime::Smuggling => 0.6,
        // Jobs & lives phase 4: petty.
        Crime::Solicitation => 0.1,
        // Jobs & lives phase 5.
        Crime::Trafficking => 0.9,
        // Jobs & lives phase 6.
        Crime::Forgery => 0.4,
        Crime::BlackMarket => 0.5,
        // Round 7 W5: a parent's break-out of his own child.
        Crime::CampEscape => 0.5,
    }
}

/// M13 D32: an agent's stealth, its skill plus its Kit's (Eyes); read as
/// M12 when the Kit adds 0.
pub fn stealth(world: &World, id: EntityId) -> f32 {
    let skill = world.comp::<Skills>(id).map_or(0.0, |s| s.stealth);
    let kit = world.comp::<Kit>(id).map_or(0.0, |k| k.stealth);
    if kit != 0.0 {
        skill + kit
    } else {
        skill
    }
}

/// Has `suspect` been reported for anything since `tick` (open, or filed and
/// resolved after the sighting)? A witness does not re-file a crime the law
/// already dealt with.
pub fn reported_since(world: &World, suspect: EntityId, tick: Tick) -> bool {
    world.reports_on(suspect).is_some_and(|(open, last)| open > 0 || last >= tick)
}

/// Is there an open warrant on `suspect`?
pub fn wanted(world: &World, suspect: EntityId) -> bool {
    world.reports_on(suspect).is_some_and(|(open, _)| open > 0)
}

/// Per tick: guards perceive wanted suspects; escorted suspects follow.
/// Daily: warrants expire, prisoners are fed and released.
pub fn run(world: &mut World) {
    crate::prof::timed!("law::tally_watch", tally_watch(world));
    crate::prof::timed!("law::credit_guard_shifts", credit_guard_shifts(world));
    crate::prof::timed!("law::sightings", sightings(world));
    // Round 6 V3 (`OPEN-238`): bodies found, the case opened; victims who
    // tell the beat.
    crate::prof::timed!("law::find_bodies", crate::systems::demography::find_bodies(world));
    // Round 7 W2 (`OPEN-238`): a sleeper robbed in his sleep wakes to it.
    if world.tick.is_multiple_of(world.config.crime.case.find_every_ticks.max(1)) {
        crate::systems::consequence::woke(world);
    }
    if world.tick.is_multiple_of(crate::time::TICKS_PER_HOUR) {
        crate::prof::timed!("law::tell_the_beat", tell_the_beat(world));
        // Round 8 X1 (`OPEN-246`): the promises to tell the law, kept or dropped.
        crate::prof::timed!("law::keep_word", keep_word(world));
        // Round 7 W3 (`OPEN-244`): the warrant the street never saw, at the door.
        crate::prof::timed!("law::serve_at_door", serve_at_door(world));
    }
    // The island pass phase 7: the transfer vans at the Precinct.
    crate::prof::timed!("law::stations::van_tick", crate::systems::stations::van_tick(world));
    // L2 (L21): the held prisoners' needs decay hour by hour, as a body's
    // do through the day (a day's decay at once left them fed and rested
    // all day, and their mood lifted their districts' unrest off the riot
    // line: riots on 42-47 read 0.17 a seed against 2.67 with the budget off).
    if world.tick.is_multiple_of(crate::time::TICKS_PER_HOUR) {
        crate::prof::timed!("law::held_hourly", held_hourly(world));
        // Round 8 X6 (`OPEN-217`): the cells' meal at its hour.
        crate::prof::timed!("law::jail_meals", jail_meals(world));
        // Lives at work phase 7 (§ 5.2): the van, at `transfer_hour`.
        crate::prof::timed!("law::stations::van", crate::systems::stations::van(world));
    }
    if crate::systems::midnight::due(world, crate::systems::midnight::Mid::Law) {
        crate::prof::timed!("law::expire_warrants", expire_warrants(world));
        // Round 7 W3: the warrants standing (a print).
        let open = world.open_suspects().count() as u32;
        crate::systems::ownership::jobs2_row(world).law_open_warrants = open;
        // L2 (L21): the held prisoners' day (meetings, the pitch), before their meal.
        crate::prof::timed!("law::held_daily", held_daily(world));
        crate::prof::timed!("law::jail_upkeep", jail_upkeep(world));
        crate::prof::timed!("law::reconcile_guards", reconcile_guards(world));
        // M12 D23: the city's sweepers, toward `levers.sanitation_count`.
        crate::prof::timed!(
            "law::districts::reconcile_sanitation",
            crate::systems::districts::reconcile_sanitation(world)
        );
        // M16a (plan C29): after `word::run`'s pass.
        crate::prof::timed!("law::accessory_check", accessory_check(world));
    }
    // The captain's daily rescoring, after the guard roster is reconciled.
    crate::prof::timed!("law::law_brain::run", crate::systems::law_brain::run(world));
    crate::prof::timed!("law::releases", releases(world));
}

/// M10 D33, M12 D3: one tick of watch per on-shift, unjailed guard, in the
/// district it stands in (`bind::district_coverage` reads yesterday's
/// totals; `bind::zone_law_coverage` sums them by zone). `World::zone_watch`
/// is kept for old saves and no longer written.
fn tally_watch(world: &mut World) {
    let tod = world.tick_of_day();
    for i in 0..world.guards().len() {
        let g = world.guards()[i];
        if world.has::<Sentence>(g) || !world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod)) {
            continue;
        }
        let Some(tile) = world.comp::<Position>(g).map(|p| p.tile) else { continue };
        let d = world.district_of(tile).index().min(crate::components::MAX_DISTRICTS - 1);
        world.district_watch.today[d] += 1;
    }
    if world.tick.is_multiple_of(crate::time::TICKS_PER_HOUR) {
        tally_home_watch(world, tod);
    }
}

/// M11 D34: on the hour, one guard-hour to every standing Home whose door is
/// within `[classes] fear_radius` (Chebyshev) of an on-shift, unjailed guard.
/// O(guards x Homes) once an hour.
pub fn tally_home_watch(world: &mut World, tod: u16) {
    let r = i32::try_from(world.config.classes.fear_radius).unwrap_or(i32::MAX);
    let tiles: Vec<crate::components::TilePos> = world
        .guards()
        .iter()
        .filter(|&&g| !world.has::<Sentence>(g) && world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod)))
        .filter_map(|&g| world.comp::<Position>(g).map(|p| p.tile))
        .collect();
    if tiles.is_empty() {
        return;
    }
    let doors: Vec<(EntityId, crate::components::TilePos)> = world
        .buildings_of_kind(crate::components::BuildingKind::Home)
        .iter()
        .filter_map(|&h| world.comp::<crate::components::Building>(h).filter(|b| !b.demolished).map(|b| (h, b.door)))
        .collect();
    for t in tiles {
        for &(h, d) in &doors {
            let dx = (i32::from(d.x) - i32::from(t.x)).abs();
            let dy = (i32::from(d.y) - i32::from(t.y)).abs();
            if dx.max(dy) <= r {
                let e = world.home_watch.today.entry(h).or_insert(0);
                *e = e.saturating_add(1);
            }
        }
    }
}

/// M10 5c: a guard's day of wages is owed here, and only here, by the shift
/// clock: each on-shift tick on law duty (Patrol, Arrest, or Work on a Jail
/// day) counts, and when the shift ends the day is owed if it was completed
/// (five patrol legs, a held Jail) or on duty for `[law] shift_duty_share` of
/// it, whatever plan was running at the end. Before, a Patrol day was owed
/// only by a PatrolLeg finishing at or after the end (whose precondition needs
/// the shift still running) or by an arrest or delivery: on seed 42 at 2,000,
/// 412 of 584 workday shifts in 20 days went unpaid, most of them on duty
/// for most of the shift, and the watch starved on full pay. A segment end
/// that a 0-start segment continues (night guards) is not an end, so each
/// shift is judged once.
fn credit_guard_shifts(world: &mut World) {
    use crate::components::GoalKind;
    let tod = world.tick_of_day();
    let last = if tod == 0 { 1439 } else { tod - 1 };
    let ended_tick = world.tick.saturating_sub(1);
    let share = world.config.law.shift_duty_share;
    // A copy: `maybe_quit` can take a guard off the roster mid-loop.
    let guards = world.guards().to_vec();
    for g in guards {
        // L2 shadow fixes item 10: fix the coming shift's duty once.
        let key = world.comp::<Job>(g).map(|j| j.next_shift_key(world.tick));
        let stale = world.comp::<Job>(g).is_some_and(|j| j.duty_fixed.map(|d| d.0) != key);
        if let (true, Some(key)) = (stale, key) {
            let v = jail_duty_now(world, g, key);
            if let Some(j) = world.comp_mut::<Job>(g) {
                j.duty_fixed = Some((key, v));
            }
        }
        let Some(job) = world.comp::<Job>(g) else { continue };
        let (on_now, ended) = (job.on_shift(tod), job.on_shift(last) && !job.on_shift(tod));
        if on_now {
            let key = job.shift_key_at(world.tick);
            let on_duty = !world.has::<Sentence>(g)
                && job.struck_shift != Some(key)
                && match world.comp::<Brain>(g).and_then(|b| b.current_goal) {
                    Some(GoalKind::Patrol | GoalKind::Arrest) => true,
                    Some(GoalKind::Work) => jail_duty(world, g, key),
                    _ => false,
                };
            if on_duty {
                if let Some(j) = world.comp_mut::<Job>(g) {
                    j.duty_ticks = j.duty_ticks.saturating_add(1);
                }
            }
            continue;
        }
        if !ended {
            continue;
        }
        let key = job.shift_key_at(ended_tick);
        let length: u32 = job.shifts.iter().map(|&(s, e)| u32::from(e.saturating_sub(s))).sum();
        let worked = job.last_shift_day == Some(key) || f32::from(job.duty_ticks) >= share * length as f32;
        // A struck shift (M11 D35) is marked worked but owes nothing.
        let struck = job.struck_shift == Some(key);
        let owed =
            worked && !struck && crate::exec::routine::workday_of(world, g, job, key) && !world.has::<Sentence>(g);
        if let Some(j) = world.comp_mut::<Job>(g) {
            j.duty_ticks = 0;
            if owed {
                j.last_shift_day = Some(key);
                j.days_unpaid = j.days_unpaid.saturating_add(1);
            }
        }
        if let Some(b) = world.comp_mut::<Brain>(g) {
            b.patrol_legs = 0;
            b.patrol_route.clear();
        }
        if owed {
            // L1: the shift's wage at its end, not on a walk to the Hall.
            crate::systems::economy::collect_wage(world, g);
        }
    }
}

/// The `guard_count` lever, reconciled daily with at most five changes (Scale
/// S4: five per 2,000 residents, `Config::per_capita_count`):
/// hire unemployed, unjailed adults of lawfulness >= 0.4 (highest first);
/// fire the guard with the lowest loyalty.
pub fn reconcile_guards(world: &mut World) {
    // M17 phase 3: the State's funded guards on top (0 with the outside off).
    let want = usize::from(world.levers.guard_count) + usize::from(world.outside.state_guards);
    // M11 D18: the city's payroll only; private guards are the corps'.
    let guards: Vec<EntityId> = crate::systems::law_brain::guards(world);
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return };
    // Lives at work phase 7 (§ 5.2): with Stations, each hire's post is the
    // nearest his Home of the Stations under their share and the Precinct.
    let mut posts = if guards.len() < want { crate::systems::stations::Posts::new(world, want) } else { None };
    if guards.len() < want {
        let candidates: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = world
            // scan-ok: daily: reconcile_guards
            .citizens()
            .into_iter()
            .filter(|&id| world.has::<Brain>(id) && !world.has::<Job>(id) && !world.has::<Sentence>(id))
            .filter(|&id| crate::systems::demography::is_adult(world, id))
            // Pressure pass F3: the idle days and the reputation (`misery::pool_out`).
            .filter(|&id| !crate::systems::misery::pool_out(world, id))
            // Lives at work phase 6 (§ 4.1, § 4.4): the Guard's band, the retired.
            .filter(|&id| crate::systems::careers::age_ok(world, id, Role::Guard))
            // Round 8 X3 (`OPEN-247`): the law's post reads a record.
            .filter(|&id| crate::systems::careers::record_ok(world, id, Role::Guard))
            .filter_map(|id| world.comp::<Personality>(id).map(|p| (ordered_float::OrderedFloat(p.lawfulness), id)))
            .filter(|&(l, _)| l.0 >= 0.4)
            .collect();
        // The pressure pass F1: those within `[distance] job_cap_ticks` of a
        // city post (a Station or the Precinct) first, the most lawful first
        // among them, then the rest (the base took the most lawful of the
        // island: Sump guards posted on Civic Core). Off: the base order.
        let doors: Vec<crate::components::TilePos> = posts
            .as_ref()
            .map(|p| p.posts.iter().map(|&(_, d, _)| d).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .chain(world.comp::<crate::components::Building>(jail).map(|b| b.door))
            .collect();
        let far = |id: EntityId| match (
            crate::systems::distance::job_cap(world),
            crate::systems::distance::set_out(world, id),
        ) {
            (Some(cap), Some(from)) => doors.iter().all(|&d| crate::systems::distance::travel(world, from, d) > cap),
            _ => false,
        };
        let mut candidates: Vec<(bool, ordered_float::OrderedFloat<f32>, EntityId)> =
            candidates.into_iter().map(|(l, id)| (far(id), l, id)).collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        for (_, _, id) in candidates.into_iter().take((want - guards.len()).min(world.config.per_capita_count(5))) {
            let post = posts.as_mut().and_then(|p| p.pick(world, id)).unwrap_or(jail);
            crate::systems::demography::hire(world, id, post, Role::Guard);
            // A dead guard's vacancy is this hire, not a second one (at the
            // post, else at any other city post).
            let at = std::iter::once(post)
                .chain(std::iter::once(jail))
                .chain(crate::systems::stations::stations(world))
                .find(|b| world.vacancies.get(b).is_some_and(|v| v.contains(&Role::Guard)));
            if let Some(at) = at {
                if let Some(v) = world.vacancies.get_mut(&at) {
                    if let Some(i) = v.iter().position(|&r| r == Role::Guard) {
                        v.remove(i);
                    }
                    if v.is_empty() {
                        world.vacancies.remove(&at);
                    }
                }
            }
        }
    } else if guards.len() > want {
        let mut by_loyalty: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = guards
            .iter()
            .filter_map(|&g| world.comp::<Personality>(g).map(|p| (ordered_float::OrderedFloat(p.loyalty), g)))
            .collect();
        by_loyalty.sort();
        for (_, g) in by_loyalty.into_iter().take((guards.len() - want).min(world.config.per_capita_count(5))) {
            let text = format!("{} dismissed from the guard", world.name_of(g));
            crate::systems::economy::dismiss(world, g, None, text);
        }
    }
    crate::systems::law_brain::recompute_captain(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scale S1: the bucketed `sightings` is the old every-guard scan: the
    /// shipped city on day 2 with a warrant on every tenth adult (so every
    /// guard sees someone), over an hour of ticks (the hourly Sighting
    /// note included), the whole world byte for byte after each tick.
    #[test]
    fn test_sightings_grid_matches_the_scan() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(2 * TICKS_PER_DAY + 7 * crate::time::TICKS_PER_HOUR - 3);
        // scan-ok: a unit test, once.
        let people: Vec<EntityId> = w.citizens().into_iter().filter(|&a| living(&w, a)).collect();
        let tick = w.tick;
        for &s in people.iter().step_by(10) {
            w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
        }
        let mut seen_any = 0usize;
        for _ in 0..6 {
            let mut b = w.clone();
            sightings_scan(&mut b);
            let want = crate::save::to_ron(&b);
            // The grid always, and never (the small city's walk).
            for grid_pairs in [0, usize::MAX] {
                let mut a = w.clone();
                sightings_with(&mut a, grid_pairs);
                assert_eq!(a.last_seen(), b.last_seen(), "tick {}, grid past {grid_pairs}", w.tick);
                assert_eq!(crate::save::to_ron(&a), want, "tick {}, grid past {grid_pairs}", w.tick);
            }
            seen_any += b.last_seen().values().filter(|&&(_, t)| t == w.tick).count();
            // On to the next hour boundary or tick (both paths taken).
            w.run_ticks(1);
        }
        assert!(seen_any > 0, "no suspect was seen: the test exercises nothing");
    }

    /// Scale S1: the warrants a guard can chase, read from the by-tick
    /// sighting index (and the lazily built claim set), are the old scan of
    /// every open warrant's: every guard, and the whole city, at ticks over
    /// two days of a city with a warrant on every tenth adult.
    #[test]
    fn test_located_suspects_index_matches_the_scan() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 9 * crate::time::TICKS_PER_HOUR);
        // scan-ok: a unit test, once.
        let people: Vec<EntityId> = w.citizens().into_iter().filter(|&a| living(&w, a)).collect();
        let tick = w.tick;
        for &s in people.iter().step_by(10) {
            w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
        }
        let mut found = 0usize;
        for _ in 0..24 {
            w.run_ticks(97);
            assert_eq!(located_suspects(&w, None), located_suspects_scan(&w, None), "the city, tick {}", w.tick);
            for &g in w.guards() {
                let Some(tile) = w.comp::<Position>(g).map(|p| p.tile) else { continue };
                let by = Some(Pursuer { tile, guard: g });
                let got = located_suspects(&w, by);
                assert_eq!(got, located_suspects_scan(&w, by), "guard {g}, tick {}", w.tick);
                assert_eq!(any_located_suspect(&w, by), !got.is_empty());
                found += got.len();
            }
        }
        assert!(found > 0, "no guard had a warrant in reach: the test exercises nothing");
    }

    /// The perf pass phase 5: the claim index (`World::claimed_by_other`)
    /// is the scan it replaced (`life::claimed_suspects`) for every guard
    /// and every claimed suspect, read at once after each of its
    /// invalidation points in one tick: an Arrest plan installed, an escort
    /// begun, both dropped, a citizen holding an Arrest plan hired as a
    /// guard, a claiming guard fired and another killed; then through a day
    /// of the city with a warrant on every tenth adult. Without the Brain
    /// mark (`comp_mut`) or the Job marks (`index_job` / `unindex_job`), the
    /// read after the write is stale.
    #[test]
    fn test_claim_index_matches_the_scan() {
        use crate::components::{DeathCause, GoalKind, Job, Role};
        use crate::goap::Plan;
        use crate::systems::life::claimed_suspects;
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 9 * crate::time::TICKS_PER_HOUR);
        // scan-ok: a unit test.
        let people: Vec<EntityId> = w.citizens().into_iter().filter(|&a| living(&w, a)).collect();
        let check = |w: &World, at: &str| -> usize {
            let guards = w.guards().to_vec();
            let mut all: BTreeSet<EntityId> = BTreeSet::new();
            for &g in &guards {
                all.extend(claimed_suspects(w, g));
            }
            let claimed = all.len();
            all.extend(people.iter().step_by(97).copied());
            for &g in &guards {
                let scan = claimed_suspects(w, g);
                for &s in &all {
                    assert_eq!(w.claimed_by_other(g, s), scan.contains(&s), "{at}: guard {g}, suspect {s}");
                }
            }
            claimed
        };
        let arrest = |w: &mut World, g: EntityId, s: EntityId| {
            let tick = w.tick;
            if let Some(b) = w.comp_mut::<Brain>(g) {
                b.plan = Some(Plan { goal: GoalKind::Arrest, target: Some(s), steps: Vec::new(), started_tick: tick });
            }
        };
        check(&w, "day 1 09:00");
        let guards = w.guards().to_vec();
        let civ: Vec<EntityId> = people.iter().copied().filter(|a| !guards.contains(a)).collect();
        let (g1, g2, g3) = (guards[0], guards[1], guards[2]);
        let (s1, s2, s3) = (civ[10], civ[20], civ[30]);
        // An Arrest plan installed: g1 is after s1.
        arrest(&mut w, g1, s1);
        check(&w, "an Arrest plan installed");
        assert!(w.claimed_by_other(g2, s1) && !w.claimed_by_other(g1, s1), "the plan claimed nobody");
        // An escort begun: g2 takes s2.
        if let Some(b) = w.comp_mut::<Brain>(g2) {
            b.escorting = Some(s2);
        }
        check(&w, "an escort begun");
        assert!(w.claimed_by_other(g1, s2), "the escort claimed nobody");
        // Both dropped.
        if let Some(b) = w.comp_mut::<Brain>(g1) {
            b.plan = None;
        }
        if let Some(b) = w.comp_mut::<Brain>(g2) {
            b.escorting = None;
        }
        check(&w, "both dropped");
        assert!(!w.claimed_by_other(g3, s1) && !w.claimed_by_other(g3, s2), "a dropped claim stayed");
        // A citizen after s3 (no guard: nobody's claim) hired as a guard.
        let recruit = civ[40];
        arrest(&mut w, recruit, s3);
        check(&w, "a citizen's Arrest plan");
        assert!(!w.claimed_by_other(g1, s3), "a citizen's plan claimed");
        let job = w.comp::<Job>(g1).cloned().expect("a guard's Job");
        assert_eq!(job.role, Role::Guard);
        w.insert(recruit, job);
        check(&w, "a guard hired");
        assert!(w.claimed_by_other(g1, s3), "the hire's claim unseen");
        // The recruit fired, g2 (after s1) killed.
        w.remove::<Job>(recruit);
        check(&w, "a guard fired");
        assert!(!w.claimed_by_other(g1, s3), "the fired guard's claim stayed");
        arrest(&mut w, g2, s1);
        check(&w, "an Arrest plan before the kill");
        w.kill(g2, DeathCause::Violence);
        check(&w, "a guard killed");
        // A day of the city, a warrant on every tenth adult, read every 97 ticks.
        let tick = w.tick;
        for &s in people.iter().step_by(10) {
            w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
        }
        let mut claims = 0;
        for _ in 0..15 {
            w.run_ticks(97);
            claims += check(&w, &format!("tick {}", w.tick));
        }
        assert!(claims > 0, "no guard claimed a suspect in the city: the test exercises nothing");
    }

    /// The perf pass phase 5: the law's indexes (the claim index, the
    /// needs' guard grid) are the scans they replaced (`spatial::scans()`),
    /// byte for byte: two days of the shipped city with a warrant on every
    /// tenth adult, run both ways, save the same (`band_refusals` aside: the
    /// switch also runs phase 3's labour-pool scan, which never counted it).
    #[test]
    fn test_law_indexes_match_the_scans() {
        let run = |scans: bool| {
            crate::spatial::SCANS.with(|s| s.set(scans));
            let mut w = World::new(42, crate::config::Config::load());
            w.run_ticks(9 * crate::time::TICKS_PER_HOUR);
            let tick = w.tick;
            // scan-ok: a unit test, once.
            for s in w.citizens().into_iter().step_by(10) {
                w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
            }
            w.run_ticks(2 * TICKS_PER_DAY);
            crate::spatial::SCANS.with(|s| s.set(false));
            // Phase 3's scan of the labour pool counts no `band_refusals`
            // (a saved counter of the indexed walk): masked, all else equal.
            let ron = crate::save::to_ron(&w);
            let mut out = String::with_capacity(ron.len());
            let mut rest = ron.as_str();
            while let Some(i) = rest.find("band_refusals:") {
                out.push_str(&rest[..i]);
                rest = rest[i + "band_refusals:".len()..].trim_start_matches(|c: char| c.is_ascii_digit());
                rest = rest.strip_prefix(',').unwrap_or(rest);
            }
            out.push_str(rest);
            (out, w.stats.history.iter().map(|r| r.arrests).sum::<u32>())
        };
        let (a, arrests) = run(false);
        let (b, _) = run(true);
        assert!(arrests > 0, "no arrest: the law's claims exercised nothing");
        assert!(a == b, "the indexes and the scans part");
    }
}
