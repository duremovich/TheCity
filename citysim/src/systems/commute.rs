//! The island pass phase 4 (docs/ISLAND_PASS.md I2; OPEN-163): a far job is
//! a pressure that resolves over weeks.
//!
//! - **The pressure** ([`Pressure`], `World.commute_pressure`): each
//!   midnight the relocation pass (`life::relocate`) reads every employed,
//!   housed adult's commute (`distance::travel`, the Home door to the work
//!   tile). Past `[commute_pressure] far_ticks` it is a day of pressure; a
//!   commute within it drops the count. No pressure builds in the
//!   `cool_days` after a move or a job switch.
//! - **Moving nearer** (the relocation's own pick, made the one path): past
//!   `move_days` of pressure (or at once for a hire since the last
//!   midnight, `hire_now`: phase 3a's far-hire move-in), the household
//!   looks for a Home nearer work: any Home with room it can afford (the
//!   first rent) within `[distance] home_cap_ticks` of work that saves
//!   `[life] commute_margin_ticks`, and the corp's company beds
//!   ([`company_for`]: no rent up front, it is docked), else a Home trade.
//!   While this is on, phase 3b's weekly far-staff move and 3a's far-hire
//!   gate are this pass (`company::fill` takes only the homeless staff).
//! - **The household** (as the relocation always had it): a co-resident
//!   spouse and the children move together; a spouse with a job of her own
//!   moves only when both commutes shrink by the margin; a household whose
//!   Home has a School within `[ages] school_reach` moves only to one that
//!   has one ([`school_kept`]).
//! - **A nearer job** ([`job_moves`], [`nearer_job`]): past `job_days` with
//!   no Home found, the worker seeks; at the next midnight, before the job
//!   search (the day's vacancies, before the unemployed and the Ambient
//!   pool take them), he takes an open post of his own role whose door is
//!   within `[distance] job_cap_ticks` of his Home and saves
//!   `job_margin_ticks` (nearest first, ties the lower id): the normal quit
//!   (`economy::quit_job`, "far commute", the old employer posting the
//!   vacancy) and the normal hire. A company bed's resident then goes
//!   through 3a's grace and eviction. Graded, foreman and contract posts do
//!   not switch.
//! - A search that found nothing waits `retry_days`.
//!
//! Statistical mirror: none needed. The pass walks every citizen whatever
//! his tier (a Statistical worker has a Home, a Job and a commute as a Full
//! one does) and moves him by `World::set_home`, the relocation's way; the
//! Ambient labour pool's households are not residents (the pool hires the
//! household nearest the post). No random draw: movers longest commute
//! first, ties the lower id. `CITYSIM_COMMUTE_PRESSURE_OFF=1` drops
//! `[commute_pressure]`: the base relocation and company move-in.

use crate::components::{Building, Job, TilePos};
use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;
use serde::{Deserialize, Serialize};

/// One worker's commute pressure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pressure {
    /// Consecutive midnights with the commute past `far_ticks`.
    pub days: u16,
    /// No search before this day (a search found nothing).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub retry: u32,
    /// No pressure builds before this day (a move or a switch).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cool: u32,
    /// Past `job_days` with no Home found: looks for a nearer post at the
    /// next midnight's job search ([`job_moves`]).
    #[serde(default, skip_serializing_if = "is_false")]
    pub seek: bool,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// `[commute_pressure]` on.
pub fn on(world: &World) -> bool {
    world.config.commute_pressure.is_some()
}

fn today(world: &World) -> u32 {
    u32::try_from(world.day()).unwrap_or(u32::MAX)
}

/// The destination cap: a Home within this of work (`[distance]
/// home_cap_ticks`, else `far_ticks`).
pub fn home_cap(world: &World) -> Tick {
    let far = world.config.commute_pressure.as_ref().map_or(0, |c| c.far_ticks);
    world.config.distance.as_ref().map_or(far, |d| d.home_cap_ticks)
}

/// Midnight, for an evaluated worker `a` with commute `d`: the day's
/// pressure. Returns true when `a` looks for a Home tonight.
pub fn note(world: &mut World, a: EntityId, d: Tick) -> bool {
    let Some(cfg) = world.config.commute_pressure.clone() else { return false };
    let day = today(world);
    let since = world.tick.saturating_sub(crate::time::TICKS_PER_DAY);
    let hired = world.comp::<Job>(a).is_some_and(|j| j.hired_tick >= since && j.hired_tick > 0);
    let mut p = world.commute_pressure.get(&a).copied().unwrap_or_default();
    let cooling = p.cool > day;
    if d <= cfg.far_ticks || cooling {
        p.days = 0;
        p.seek = false;
    } else {
        p.days = p.days.saturating_add(1);
    }
    let look = !cooling && d > cfg.far_ticks && p.retry <= day && (p.days >= cfg.move_days || (cfg.hire_now && hired));
    store(world, a, p);
    look
}

fn store(world: &mut World, a: EntityId, p: Pressure) {
    let day = today(world);
    if p.days == 0 && p.cool <= day && p.retry <= day && !p.seek {
        world.commute_pressure.remove(&a);
    } else {
        world.commute_pressure.insert(a, p);
    }
}

/// The entries of the workers the pass did not read tonight (no job, no
/// Home, a sentence, gone) lose their far days; the gauge.
pub fn prune(world: &mut World, read: &std::collections::BTreeSet<EntityId>) {
    let day = today(world);
    let mut far = 0u32;
    world.commute_pressure.retain(|a, p| {
        if !read.contains(a) {
            p.days = 0;
            p.seek = false;
        }
        if p.days > 0 {
            far += 1;
        }
        p.days > 0 || p.cool > day || p.retry > day
    });
    if far > 0 {
        crate::systems::ownership::jobs2_row(world).pressure_far = far;
    }
}

/// `a` moved (or switched posts): the count drops and the cooldown runs.
pub fn moved(world: &mut World, a: EntityId) {
    let Some(cfg) = world.config.commute_pressure.as_ref() else { return };
    let cool = today(world).saturating_add(u32::from(cfg.cool_days));
    store(world, a, Pressure { days: 0, retry: 0, cool, seek: false });
}

/// `a`'s search found nothing: the next waits `retry_days`; past
/// `job_days`, he looks for a nearer post at the next midnight's job
/// search ([`job_moves`]).
pub fn found_nothing(world: &mut World, a: EntityId) {
    let Some(cfg) = world.config.commute_pressure.clone() else { return };
    let mut p = world.commute_pressure.get(&a).copied().unwrap_or_default();
    p.seek |= p.days >= cfg.job_days;
    p.retry = today(world).saturating_add(u32::from(cfg.retry_days.max(1)));
    store(world, a, p);
}

/// Midnight, before `demography::job_search` (the day's vacancies, before
/// the unemployed and the Ambient pool take them): each seeker ([`Pressure::
/// seek`]) still housed far from work, longest commute first (ties the
/// lower id), takes the nearest open post of his role ([`nearer_job`]);
/// one who found none seeks again the next midnight.
pub fn job_moves(world: &mut World) {
    let Some(cfg) = world.config.commute_pressure.clone() else { return };
    let seekers: Vec<EntityId> = world.commute_pressure.iter().filter(|(_, p)| p.seek).map(|(&a, _)| a).collect();
    if seekers.is_empty() || world.vacancies.is_empty() {
        return;
    }
    let mut keyed: Vec<(std::cmp::Reverse<Tick>, EntityId, TilePos)> = Vec::new();
    for a in seekers {
        let home = world.comp::<crate::components::Household>(a).and_then(|h| h.home);
        let hd = home.and_then(|h| world.comp::<Building>(h)).map(|b| b.door);
        let work =
            world.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| crate::systems::life::work_tile(world, a, e));
        match (hd, work) {
            (Some(hd), Some(wt)) if crate::systems::law::living(world, a) => {
                let d = crate::systems::distance::travel(world, hd, wt);
                if d > cfg.far_ticks {
                    keyed.push((std::cmp::Reverse(d), a, hd));
                    continue;
                }
            }
            _ => {}
        }
        if let Some(p) = world.commute_pressure.get_mut(&a) {
            p.seek = false;
        }
    }
    keyed.sort_unstable();
    for (std::cmp::Reverse(d), a, hd) in keyed {
        if world.vacancies.is_empty() {
            break;
        }
        nearer_job(world, a, hd, d);
    }
}

/// Company Home `h` takes `a`'s household: its beds are reserved for the
/// workplace `a` works at, or another of `h`'s owner's (`company::on_staff`).
pub fn company_for(world: &World, a: EntityId, h: EntityId) -> bool {
    // The island fix round F2 (OPEN-189): the workplace's own staff only,
    // within `[beds] company_post_ticks` of the post.
    if let Some(take) = crate::systems::beds::company_take(world, a, h) {
        return take;
    }
    let Some(w) = crate::systems::company::of(world, h) else { return false };
    let Some(e) = world.comp::<Job>(a).and_then(|j| j.employer) else { return false };
    e == w || world.owner_of(h).is_some_and(|o| world.owner_of(e) == Some(o))
}

/// A household whose Home `from` has a School in reach moves only to a
/// Home `to` with one (the children keep their schooling).
pub fn school_kept(world: &World, from: TilePos, to: TilePos) -> bool {
    crate::systems::ages::school_near(world, from).is_none() || crate::systems::ages::school_near(world, to).is_some()
}

/// Past `job_days` with no Home found: the open post of `a`'s role nearest
/// his Home door, within `[distance] job_cap_ticks` (`far_ticks` off) and
/// saving `job_margin_ticks` on his commute `d` (ties the lower employer):
/// the normal quit and the normal hire. Returns true on a switch.
pub fn nearer_job(world: &mut World, a: EntityId, home_door: TilePos, d: Tick) -> bool {
    let Some(cfg) = world.config.commute_pressure.clone() else { return false };
    let Some(job) = world.comp::<Job>(a).cloned() else { return false };
    let Some(employer) = job.employer else { return false };
    if job.foreman || job.until.is_some() || job.grade >= crate::systems::classpay::PROFESSIONAL {
        return false;
    }
    // A household business's owner stays at it.
    if world.owner_of(employer).is_some_and(|o| o == a || world.spouse_of(a) == Some(o)) {
        return false;
    }
    let role = job.role;
    if !crate::systems::trades::skill_ok(world, a, role)
        || !crate::systems::fashion::dress_ok(world, a, role)
        || !crate::systems::gray::hire_ok(world, a, role)
        || !crate::systems::dollhouse::hire_ok(world, role)
    {
        return false;
    }
    let cap = world.config.distance.as_ref().map_or(cfg.far_ticks, |x| x.job_cap_ticks);
    let mut best: Option<(Tick, EntityId)> = None;
    for (&e, roles) in &world.vacancies {
        if e == employer || !roles.contains(&role) || crate::systems::life::quit_blocks(world, a, e) {
            continue;
        }
        if crate::systems::classpay::on(world)
            && crate::systems::classpay::base_grade(world, Some(e), role) >= crate::systems::classpay::PROFESSIONAL
        {
            continue;
        }
        let Some(door) = world.comp::<Building>(e).filter(|b| !b.demolished && !b.derelict).map(|b| b.door) else {
            continue;
        };
        let t = crate::systems::distance::travel(world, home_door, door);
        if t > cap || t + cfg.job_margin_ticks > d {
            continue;
        }
        if best.is_none_or(|b| (t, e) < b) {
            best = Some((t, e));
        }
    }
    let Some((_, to)) = best else { return false };
    crate::systems::economy::quit_job(world, a, "far commute");
    crate::systems::demography::hire(world, a, to, role);
    crate::systems::demography::close_vacancy(world, a, to, role);
    moved(world, a);
    crate::systems::ownership::jobs2_row(world).pressure_jobs += 1;
    true
}
