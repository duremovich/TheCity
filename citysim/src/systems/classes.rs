//! M11 classes (docs/M11_OWNERSHIP.md § 7, plan phase 4, D34-D36). Class
//! membership is derived, never stored: Corp = employed by a corp-owned
//! building or a corp's exec; Dreg = no Home; Street = everyone else.
//!
//! `run` is daily at midnight, after ownership's rent pass and before the
//! economy and the brains (D42): it recomputes `World::classes`, counts the
//! Dregs' miserable days (and sends the long-miserable jobless away), and
//! calls a strike when Street unrest runs high. Immigration reads
//! [`immigration_factor`] weekly; the gang recruiter and the LOD ranker read
//! [`evicted_desperate`] (D36).

use std::collections::BTreeSet;

use crate::components::{
    Brain, Building, BuildingKind, Class, ClassAggregate, Corp, CorpShock, Household, Job, Mood, Personality, Position,
    Sentence,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::utility::curves::Curve;
use crate::world::World;

/// Days of eviction counted in a class's `evictions_7d`.
const EVICTION_WINDOW_DAYS: u64 = 7;
/// Zones on the map (`Zone::index`).
const ZONES: usize = 5;

/// Every corp's exec, for `class_of` in a pass.
fn execs(world: &World) -> BTreeSet<EntityId> {
    world.corps().into_iter().filter_map(|c| world.comp::<Corp>(c).and_then(|cc| cc.exec)).collect()
}

fn class_with(world: &World, agent: EntityId, execs: &BTreeSet<EntityId>) -> Class {
    if execs.contains(&agent) {
        return Class::Corp;
    }
    let corp_job = world
        .comp::<Job>(agent)
        .and_then(|j| j.employer)
        .and_then(|e| world.owner_of(e))
        .is_some_and(|o| world.has::<Corp>(o));
    if corp_job {
        return Class::Corp;
    }
    if world.comp::<Household>(agent).is_none_or(|h| h.home.is_none()) {
        return Class::Dreg;
    }
    Class::Street
}

/// Every corp's exec (for [`class_in`] over many agents in one pass).
pub fn exec_set(world: &World) -> BTreeSet<EntityId> {
    execs(world)
}

/// [`class_of`] with the exec set computed once by the caller.
pub fn class_in(world: &World, agent: EntityId, execs: &BTreeSet<EntityId>) -> Class {
    class_with(world, agent, execs)
}

/// The agent's class (§ 7): Corp first (a homeless corp worker is Corp).
pub fn class_of(world: &World, agent: EntityId) -> Class {
    class_with(world, agent, &execs(world))
}

/// The aggregate formulas of § 7 over one class's members' inputs:
/// `(mood, employed, fear)` per member, plus the class's 7-day evictions.
pub fn aggregate(members: &[(f32, bool, f32)], evictions_7d: u32) -> ClassAggregate {
    let count = members.len() as u32;
    if count == 0 {
        return ClassAggregate { evictions_7d, ..ClassAggregate::default() };
    }
    let n = count as f32;
    let happiness = members.iter().map(|&(m, _, _)| (m + 1.0) / 2.0).sum::<f32>() / n;
    let employment = members.iter().filter(|&&(_, e, _)| e).count() as f32 / n;
    let fear = (members.iter().map(|&(_, _, f)| f).sum::<f32>() / n).clamp(0.0, 1.0);
    let loyalty = happiness * (employment + 0.5).min(1.0);
    let submission = 0.3 + 0.7 * fear;
    let unrest = (1.0 - loyalty) * (1.0 - submission) + 0.1 * evictions_7d as f32 / n;
    ClassAggregate {
        count,
        happiness,
        employment,
        fear,
        loyalty,
        submission,
        unrest,
        evictions_7d,
        trace: vec![
            ("count", n),
            ("happiness", happiness),
            ("employment", employment),
            ("fear", fear),
            ("loyalty", loyalty),
            ("submission", submission),
            ("evictions_7d", evictions_7d as f32),
            ("unrest", unrest),
        ],
    }
}

/// D34: one Home's fear, yesterday's guard-hours ÷ `fear_hours_full`, capped at 1.
pub fn home_fear(world: &World, home: EntityId) -> f32 {
    let full = world.config.classes.fear_hours_full.max(1e-6);
    let hours = world.home_watch.yesterday.get(&home).copied().unwrap_or(0);
    (f32::from(hours) / full).min(1.0)
}

/// Daily at midnight after ownership (D42).
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    compute(world);
    miserable_dregs(world);
    strike(world);
}

/// Per zone, the mean fear over its standing Homes (a Dreg's fear).
fn zone_fear(world: &World) -> [f32; ZONES] {
    let mut zone_sum = [0.0f32; ZONES];
    let mut zone_n = [0u32; ZONES];
    for &h in world.buildings_of_kind(BuildingKind::Home) {
        let Some(b) = world.comp::<Building>(h).filter(|b| !b.demolished) else { continue };
        let z = world.map.zone(b.door).index().min(ZONES - 1);
        zone_sum[z] += home_fear(world, h);
        zone_n[z] += 1;
    }
    std::array::from_fn(|z| if zone_n[z] == 0 { 0.0 } else { zone_sum[z] / zone_n[z] as f32 })
}

/// One member's aggregate inputs: `(class, mood, employed, fear)`. A housed
/// agent's fear is its Home's; a Dreg's is the mean over its zone's Homes.
pub fn member_inputs(world: &World, agent: EntityId) -> (Class, f32, bool, f32) {
    member(world, agent, &execs(world), &zone_fear(world))
}

fn member(world: &World, a: EntityId, execs: &BTreeSet<EntityId>, zones: &[f32; ZONES]) -> (Class, f32, bool, f32) {
    let class = class_with(world, a, execs);
    let mood = world.comp::<Mood>(a).map_or(0.0, |m| m.value);
    let employed = world.has::<Job>(a);
    let fear = match world.comp::<Household>(a).and_then(|h| h.home) {
        Some(h) => home_fear(world, h),
        None => world.comp::<Position>(a).map_or(0.0, |p| zones[world.map.zone(p.tile).index().min(ZONES - 1)]),
    };
    (class, mood, employed, fear)
}

/// Recompute `World::classes` from the living adults with a Brain.
pub fn compute(world: &mut World) {
    let execs = execs(world);
    let zones = zone_fear(world);
    let mut members: [Vec<(f32, bool, f32)>; 3] = Default::default();
    // scan-ok: daily: class aggregates
    for a in world.citizens() {
        if !world.has::<Brain>(a) || !crate::systems::demography::is_adult(world, a) {
            continue;
        }
        let (class, mood, employed, fear) = member(world, a, &execs, &zones);
        members[class.index()].push((mood, employed, fear));
    }
    // § 7 puts the eviction term on the Street aggregate (an evictee is a
    // Dreg by the next midnight): the city's 7-day count goes there.
    let horizon = world.tick.saturating_sub(EVICTION_WINDOW_DAYS * TICKS_PER_DAY);
    let evictions = world.eviction_log.iter().filter(|&&t| t >= horizon).count() as u32;
    world.classes = [aggregate(&members[0], 0), aggregate(&members[1], evictions), aggregate(&members[2], 0)];
}

/// Street unrest (the corp brain's `unrest` input, the strike trigger).
pub fn street_unrest(world: &World) -> f32 {
    world.classes[Class::Street.index()].unrest
}

/// Weekly immigration multiplier: Logistic{k, mid} over Street happiness;
/// 1 when the coupling is off (the lever is then the count).
pub fn immigration_factor(world: &World) -> f32 {
    let c = &world.config.classes;
    if !c.couple_immigration {
        return 1.0;
    }
    let h = world.classes[Class::Street.index()].happiness;
    Curve::Logistic { k: c.immigration_k, mid: c.immigration_mid }.eval(h)
}

/// This week's immigrants: `immigration_per_week × immigration_factor`, rounded.
pub fn immigrants_this_week(world: &World) -> u8 {
    let lever = world.levers.immigration_per_week;
    if !world.config.classes.couple_immigration {
        return lever;
    }
    (f32::from(lever) * immigration_factor(world)).round().clamp(0.0, 255.0) as u8
}

/// D36: evicted within `[classes] evicted_recruit_days` and lawless: desperate
/// enough for a gang (`gang::recruit_gang`) and worth a body (`lod`).
pub fn evicted_desperate(world: &World, agent: EntityId) -> bool {
    let c = &world.config.classes;
    let Some((_, at)) = world.comp::<Household>(agent).and_then(|h| h.evicted_by) else { return false };
    world.tick.saturating_sub(at) <= c.evicted_recruit_days * TICKS_PER_DAY
        && world.comp::<Personality>(agent).is_some_and(|p| p.lawfulness < c.evicted_recruit_lawfulness)
}

/// § 7 Emigration: a Dreg below `dreg_emigrate_mood` adds a miserable day,
/// anyone else resets; at `dreg_emigrate_days` (0 = off) a jobless, free
/// Dreg leaves by the existing emigration path.
fn miserable_dregs(world: &mut World) {
    let c = world.config.classes.clone();
    let execs = execs(world);
    let mut leaving = Vec::new();
    // scan-ok: daily: Dreg misery
    for a in world.citizens() {
        if !world.has::<Brain>(a) || !crate::systems::demography::is_adult(world, a) {
            continue;
        }
        let dreg = class_with(world, a, &execs) == Class::Dreg;
        let low = world.comp::<Mood>(a).is_some_and(|m| m.value < c.dreg_emigrate_mood);
        let Some(h) = world.comp_mut::<Household>(a) else { continue };
        h.miserable_days = if dreg && low { h.miserable_days.saturating_add(1) } else { 0 };
        let days = h.miserable_days;
        if c.dreg_emigrate_days > 0
            && days >= c.dreg_emigrate_days
            && !world.has::<Job>(a)
            && !world.has::<Sentence>(a)
            && world.comp::<Brain>(a).is_some_and(|b| !b.emigrating && b.cuffed_by.is_none())
        {
            leaving.push(a);
        }
    }
    for a in leaving {
        crate::systems::demography::start_emigrating(world, a, "miserable on the street");
    }
}

/// The corp a strike hits (D35): the highest max `price_level` among corps
/// with at least one non-exec employee (ties: larger treasury, then lower
/// id), with those employees.
pub fn strike_target(world: &World) -> Option<(EntityId, Vec<EntityId>)> {
    let mut best: Option<(f32, i64, EntityId, Vec<EntityId>)> = None;
    for c in world.corps() {
        let Some(cc) = world.comp::<Corp>(c) else { continue };
        let staff = employees(world, cc);
        if staff.is_empty() {
            continue;
        }
        let level = cc.price_level.values().copied().fold(0.0f32, f32::max);
        let better = best.as_ref().is_none_or(|&(l, t, id, _)| {
            level > l || (level == l && (cc.treasury > t || (cc.treasury == t && c < id)))
        });
        if better {
            best = Some((level, cc.treasury, c, staff));
        }
    }
    best.map(|(_, _, c, s)| (c, s))
}

/// A corp's non-exec employees, ascending.
pub(crate) fn employees(world: &World, cc: &Corp) -> Vec<EntityId> {
    crate::systems::ownership::staff_by_building(world, &cc.buildings)
        .into_values()
        .flatten()
        .filter(|&a| Some(a) != cc.exec && world.has::<Brain>(a))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// D35: `strikers` walk out of the shift in progress, or the next one: it
/// is marked worked and struck (no wage; a guard's shift clock owes nothing
/// either), and a body already on its way to work or at it drops that plan
/// (a Work or Patrol plan running at the strike was finished and paid).
pub(crate) fn walk_out(world: &mut World, strikers: &[EntityId]) {
    use crate::components::{GoalKind, Lod};
    let now = world.tick;
    for &a in strikers {
        let Some(j) = world.comp_mut::<Job>(a) else { continue };
        let key = j.next_shift_key(now);
        j.last_shift_day = Some(key);
        j.struck_shift = Some(key);
        j.duty_ticks = 0;
        let working = world.comp::<Brain>(a).is_some_and(|b| {
            b.lod != Lod::Statistical
                && b.plan.as_ref().is_some_and(|p| matches!(p.goal, GoalKind::Work | GoalKind::Patrol))
        });
        if working {
            world.abort_plan(a);
        }
    }
}

/// D35: Street unrest above `strike_threshold`, no strike in the last
/// `strike_cooldown_days`: the target corp's workers skip their next shift
/// (no wage, no work), one `Strike` event, `CorpShock::Strike`.
pub fn strike(world: &mut World) {
    let c = world.config.classes.clone();
    // Tick 0 has no yesterday's watch (fear reads 0): no strike on day 0.
    if world.tick == 0 || street_unrest(world) <= c.strike_threshold {
        return;
    }
    let now = world.tick;
    let cooldown: Tick = c.strike_cooldown_days * TICKS_PER_DAY;
    if world.last_strike.is_some_and(|t| now.saturating_sub(t) < cooldown) {
        return;
    }
    let Some((corp, strikers)) = strike_target(world) else { return };
    walk_out(world, &strikers);
    world.last_strike = Some(now);
    world.stats.current.strikes += 1;
    let name = world.owner_label(Some(corp));
    let unrest = street_unrest(world);
    world.push_event(
        EventKind::Strike,
        &[corp],
        format!("{} workers of {name} walk out (Street unrest {unrest:.2})", strikers.len()),
    );
    crate::systems::corp_brain::push_shock(world, corp, CorpShock::Strike);
}
