//! M15 § 4 (plan phase 3, W19-W24): the Hunt.
//!
//! A game goal: an adult holding a heavy grudge on a living agent goes
//! after them. The plan is scripted, never searched (`plan::plan_for`'s
//! bypass): `GoTo(Intel) → AskAround → GoTo(Intel) → StakeOut → Attack`,
//! or `GoTo(Intel) → StakeOut → Attack` when a fresh sighting already says
//! where the target is. `LocationKey::Intel` resolves from the hunter's
//! `HuntState` (the ask venue, then the intel). The strike is the ordinary
//! `Attack` through `law::resolve_fight` (a lethal Hunt's win kills at
//! `hunt_kill_p`); a win posts an `Avenged` deed and sets the victim's
//! `kill_chain`, so the victim's kin who learn of it start the next grudge
//! one link deeper. Statistical holders are scored once in three days and
//! promoted to Coarse to hunt. At most `max_hunts` Hunts run (god Hunts
//! excepted); `tick` walks only them.

use rand::Rng;

use crate::components::{
    trace_flags, Brain, Building, BuildingKind, GoalKind, Household, Job, Lod, Memory, MemoryEntry, MemoryKind,
    Personality, Position, RelKind, Sentence, Trace,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::StepResult;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::rng::splitmix64;
use crate::time::{self, Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::word::{
    Deed, GrudgeCause, Grudges, HuntPhase, HuntState, HuntWhy, Intel, IntelSource, MoveKind, SocialMove, Stake, WordNs,
};
use crate::world::World;

/// The Hunt runs with the word and `[hunt] enabled`.
pub fn on(world: &World) -> bool {
    world.config.gossip.enabled && world.config.hunt.enabled
}

/// May `h` hunt at all: a living adult, free, not leaving.
pub fn hunter_ok(world: &World, h: EntityId) -> bool {
    crate::systems::law::living(world, h)
        && crate::systems::demography::is_adult(world, h)
        && !world.has::<Sentence>(h)
        && world.comp::<Brain>(h).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
}

/// Can `t` be hunted: a living agent, free, not leaving.
pub fn target_ok(world: &World, t: EntityId) -> bool {
    crate::systems::law::living(world, t)
        && !world.has::<Sentence>(t)
        && world.comp::<Brain>(t).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
}

/// W20: the heaviest unsettled grudge of `holder` on a huntable agent with
/// `weight ≥ hunt_min` (ties the lower target id): `(target, weight, chain)`.
pub fn heaviest_eligible(world: &World, holder: EntityId) -> Option<(EntityId, f32, u8)> {
    let min = world.config.hunt.hunt_min;
    world
        .comp::<Grudges>(holder)?
        .list
        .iter()
        .filter(|x| x.settled.is_none() && x.weight >= min && x.target != holder)
        .filter(|x| world.has::<crate::components::Identity>(x.target) && target_ok(world, x.target))
        .max_by(|a, b| a.weight.total_cmp(&b.weight).then(b.target.cmp(&a.target)))
        .map(|x| (x.target, x.weight, x.chain))
}

fn cooled(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).and_then(|b| b.cooldowns.get(&GoalKind::Hunt)).is_some_and(|&t| t > world.tick)
}

/// W20: the Hunt's considerations (spec § 4 table). A hunter with a
/// `HuntState` is scored on its target whatever the cap; anyone else needs
/// an eligible grudge and a free slot under `max_hunts`.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) || !hunter_ok(world, id) {
        return None;
    }
    let (target, weight, gap) = match world.hunts.get(&id) {
        Some(s) => {
            let w = crate::systems::grudges::grudge_on(world, id, s.target).map_or(s.weight, |g| g.weight);
            (s.target, w, Some(s.gap))
        }
        None => {
            if world.hunts.len() >= world.config.hunt.max_hunts {
                return None;
            }
            let (t, w, _) = heaviest_eligible(world, id)?;
            (t, w, None)
        }
    };
    if !target_ok(world, target) {
        return None;
    }
    let p = world.comp::<Personality>(id)?;
    let gap = gap.unwrap_or_else(|| might_gap(world, id, target));
    let heat = crate::systems::reputation::rep(world, id).heat;
    let dread = crate::systems::reputation::rep(world, target).dread;
    let cs = vec![
        Consideration::new("grudge open", can(true), GATE),
        Consideration::new("grudge weight", weight, Curve::Linear { m: 0.9, b: 0.1 }),
        Consideration::new("courage", p.courage, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("0.5+might gap", (0.5 + gap).clamp(0.0, 1.0), Curve::Logistic { k: 6.0, mid: 0.4 }),
        Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("1-heat", 1.0 - heat, Curve::Linear { m: 0.4, b: 0.6 }),
        Consideration::new("1-dread x fear", 1.0 - dread * (1.0 - p.courage), Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    Some((cs, world.config.hunt.hunt_flat))
}

/// M16a (plan C20): the Hunt's considerations on `target` at `weight`
/// without the might-gap term (the hire pass's score), at any tier.
pub fn hire_considerations(
    world: &World,
    id: EntityId,
    target: EntityId,
    weight: f32,
) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) || !hunter_ok(world, id) || !target_ok(world, target) {
        return None;
    }
    let p = world.comp::<Personality>(id)?;
    let heat = crate::systems::reputation::rep(world, id).heat;
    let dread = crate::systems::reputation::rep(world, target).dread;
    let cs = vec![
        Consideration::new("grudge open", can(true), GATE),
        Consideration::new("grudge weight", weight, Curve::Linear { m: 0.9, b: 0.1 }),
        Consideration::new("courage", p.courage, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("1-heat", 1.0 - heat, Curve::Linear { m: 0.4, b: 0.6 }),
        Consideration::new("1-dread x fear", 1.0 - dread * (1.0 - p.courage), Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    Some((cs, world.config.hunt.hunt_flat))
}

/// W26's `might(hunter) − might(target)`.
pub fn might_gap(world: &World, hunter: EntityId, target: EntityId) -> f32 {
    crate::systems::moves::might(world, hunter) - crate::systems::moves::might(world, target)
}

/// Rebuild `hunted_by` from `hunts` (the targets of Hunts in `Watch`),
/// and M16a's twin `chased_by` from `contract_runs` (C14).
pub fn reindex(world: &mut World) {
    world.hunted_by =
        world.hunts.iter().filter(|(_, s)| s.phase == HuntPhase::Watch).map(|(&h, s)| (s.target, h)).collect();
    if !world.contract_runs.is_empty() || !world.chased_by.is_empty() {
        world.chased_by = world
            .contract_runs
            .iter()
            .filter(|(_, s)| s.phase == HuntPhase::Watch)
            .map(|(&h, s)| (s.target, h))
            .collect();
    }
}

/// M16a (plan C14): the intel state a chase step reads, from the agent's
/// Hunt (`World::hunts`) or else its contract run (`World::contract_runs`):
/// a copy, written back by [`set_chase`]. With no contract run every
/// reader sees exactly the Hunt's state.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Chase {
    pub target: EntityId,
    pub phase: HuntPhase,
    pub venue: Option<EntityId>,
    pub intel: Option<Intel>,
    pub stakeout_until: Option<Tick>,
    pub deceived: bool,
    pub liar: Option<EntityId>,
}

/// The agent's chase: its Hunt's, else its contract run's.
pub fn chase(world: &World, id: EntityId) -> Option<Chase> {
    if let Some(s) = world.hunts.get(&id) {
        return Some(Chase {
            target: s.target,
            phase: s.phase,
            venue: s.venue,
            intel: s.intel,
            stakeout_until: s.stakeout_until,
            deceived: s.deceived,
            liar: s.liar,
        });
    }
    if world.contract_runs.is_empty() {
        return None;
    }
    world.contract_runs.get(&id).map(|r| Chase {
        target: r.target,
        phase: r.phase,
        venue: r.venue,
        intel: r.intel,
        stakeout_until: r.stakeout_until,
        deceived: r.deceived,
        liar: r.liar,
    })
}

/// Write a chase back to whichever store holds the agent (the target is
/// never rewritten).
pub fn set_chase(world: &mut World, id: EntityId, c: Chase) {
    if let Some(s) = world.hunts.get_mut(&id) {
        s.phase = c.phase;
        s.venue = c.venue;
        s.intel = c.intel;
        s.stakeout_until = c.stakeout_until;
        s.deceived = c.deceived;
        s.liar = c.liar;
        return;
    }
    if let Some(r) = world.contract_runs.get_mut(&id) {
        r.phase = c.phase;
        r.venue = c.venue;
        r.intel = c.intel;
        r.stakeout_until = c.stakeout_until;
        r.deceived = c.deceived;
        r.liar = c.liar;
    }
}

/// W20: take up a Hunt on the heaviest eligible grudge (a god Hunt skips
/// the cap). `HuntStarted`, `stats.hunts`, the chain statistic.
pub fn adopt(world: &mut World, id: EntityId, why: HuntWhy) -> bool {
    if world.hunts.contains_key(&id) {
        return true;
    }
    if why != HuntWhy::God && world.hunts.len() >= world.config.hunt.max_hunts {
        return false;
    }
    let Some((target, weight, chain)) = heaviest_eligible(world, id) else { return false };
    // M16a (plan C20): a holder too weak to win who can pay hires instead
    // (a Goal or Stat Hunt; never a god's).
    if why != HuntWhy::God && crate::systems::contracts::try_hire(world, id, target, weight) {
        return false;
    }
    let now = world.tick;
    let gap = might_gap(world, id, target);
    world.hunts.insert(
        id,
        HuntState {
            target,
            grudge_target: target,
            chain,
            since: now,
            phase: HuntPhase::Ask,
            venue: None,
            intel: None,
            stakeout_until: None,
            deceived: false,
            why,
            liar: None,
            weight,
            gap,
        },
    );
    let w = &mut world.stats.current.word;
    w.hunts += 1;
    // Plan deviation: the CSV's `chain_max` is a chain's length (the
    // grudge's chain + 1), so "a revenge chain of length 2" reads 2.
    w.chain_max = w.chain_max.max(u32::from(chain) + 1);
    let text = format!("{} went looking for {}", world.name_of(id), world.name_of(target));
    world.push_event(EventKind::HuntStarted, &[id, target], text);
    true
}

/// The busiest Bar of a district (most occupants, ties the lower id).
pub fn busiest_bar(world: &World, d: crate::components::DistrictId) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished) && world.district_of_building(b) == d)
        .max_by(|&a, &b| {
            let n = |x: EntityId| world.comp::<Building>(x).map_or(0, |bd| bd.occupants.len());
            n(a).cmp(&n(b)).then(b.cmp(&a))
        })
}

/// The hunter's freshest held Sighting of `target` (any age), and the
/// gang database's: `(tick, building, tile)`.
fn sightings_of(
    world: &World,
    hunter: EntityId,
    target: EntityId,
) -> Option<(Tick, Option<EntityId>, crate::components::TilePos)> {
    let mut best: Option<(Tick, Option<EntityId>, crate::components::TilePos)> = None;
    if let Some(m) = world.comp::<Memory>(hunter) {
        for e in m.heard.iter().filter(|e| e.kind == MemoryKind::Sighting && e.subject == Some(target)) {
            let Some(door) = e.at.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) else { continue };
            if best.is_none_or(|(t, _, _)| e.tick > t) {
                best = Some((e.tick, e.at, door));
            }
        }
    }
    if let Some(db) = world.gang_of(hunter).and_then(|g| world.db.get(&g)) {
        for s in db.sightings.iter().filter(|s| s.who == target) {
            if best.is_none_or(|(t, _, _)| s.tick > t) {
                best = Some((s.tick, None, s.tile));
            }
        }
    }
    best
}

/// W21 (1): a Sighting of the target younger than `fresh_sighting_hours`.
pub fn fresh_intel(world: &World, hunter: EntityId, target: EntityId) -> Option<Intel> {
    let horizon = world.tick.saturating_sub(Tick::from(world.config.hunt.fresh_sighting_hours) * TICKS_PER_HOUR);
    let (t, building, tile) = sightings_of(world, hunter, target)?;
    (t >= horizon).then_some(Intel { building, tile, source: IntelSource::Sighting })
}

/// The district where the hunter last knew the target to be: its newest
/// sighting's, else the target's Home's, else where it stands.
pub(crate) fn last_known_district(
    world: &World,
    hunter: EntityId,
    target: EntityId,
) -> Option<crate::components::DistrictId> {
    if let Some((_, _, tile)) = sightings_of(world, hunter, target) {
        return Some(world.district_of(tile));
    }
    crate::systems::gossip::home_district(world, target)
        .or_else(|| world.comp::<Position>(target).map(|p| world.district_of(p.tile)))
}

fn intel_at(world: &World, b: EntityId, source: IntelSource) -> Option<Intel> {
    world.comp::<Building>(b).map(|bd| Intel { building: Some(b), tile: bd.door, source })
}

/// W21: where the target's Trace says it is at `now`: its Home at night
/// (20:00-06:00) when it slept there on ≥ 7 of the last 14 days, its
/// workplace while on shift, else the busiest Bar of its modal district of
/// the last 14 days, else its Home, else where it stands.
pub fn habit(world: &World, target: EntityId, now: Tick) -> Intel {
    let day = time::day(now);
    let tod = (now % TICKS_PER_DAY) as u16;
    let hour = tod / TICKS_PER_HOUR as u16;
    let home = world.comp::<Household>(target).and_then(|h| h.home).filter(|&h| world.has::<Building>(h));
    let mut slept = 0;
    let mut counts = [0u16; crate::components::MAX_DISTRICTS];
    if let Some(tr) = world.comp::<Trace>(target) {
        for d in day.saturating_sub(14)..day {
            if let Some(t) = tr.on_day(d) {
                if t.has(trace_flags::SLEPT_AT_HOME) {
                    slept += 1;
                }
                if !t.district.is_unset() {
                    if let Some(c) = counts.get_mut(t.district.index()) {
                        *c += 1;
                    }
                }
            }
        }
    }
    let night = !(6..20).contains(&hour);
    if let (Some(h), true) = (home, night && slept >= 7) {
        if let Some(i) = intel_at(world, h, IntelSource::Habit) {
            return i;
        }
    }
    if let Some(e) = world.comp::<Job>(target).filter(|j| j.on_shift(tod)).and_then(|j| j.employer) {
        if let Some(i) = intel_at(world, e, IntelSource::Habit) {
            return i;
        }
    }
    let modal = counts
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0)))
        .map(|(i, _)| crate::components::DistrictId(i as u8));
    if let Some(i) = modal.and_then(|d| busiest_bar(world, d)).and_then(|b| intel_at(world, b, IntelSource::Habit)) {
        return i;
    }
    if let Some(i) = home.and_then(|h| intel_at(world, h, IntelSource::Home)) {
        return i;
    }
    let p = world.comp::<Position>(target);
    Intel {
        building: p.and_then(|p| p.building),
        tile: p.map_or_else(Default::default, |p| p.tile),
        source: IntelSource::Habit,
    }
}

/// W19: the hunter's scripted plan (taking up the Hunt first when there is
/// none): a fresh sighting skips the asking.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !on(world) {
        return None;
    }
    if !world.hunts.contains_key(&id) && !adopt(world, id, HuntWhy::Goal) {
        return None;
    }
    let s = world.hunts.get(&id)?.clone();
    if !target_ok(world, s.target) {
        return None;
    }
    let mut next = s.clone();
    if next.phase == HuntPhase::Ask {
        if let Some(i) = fresh_intel(world, id, s.target) {
            next.phase = HuntPhase::Watch;
            next.intel = Some(i);
        } else {
            match last_known_district(world, id, s.target).and_then(|d| busiest_bar(world, d)) {
                Some(v) => next.venue = Some(v),
                None => {
                    next.phase = HuntPhase::Watch;
                    next.intel = Some(habit(world, s.target, world.tick));
                }
            }
        }
    }
    if next.phase == HuntPhase::Watch && next.intel.is_none() {
        next.intel = Some(habit(world, s.target, world.tick));
    }
    let phase = next.phase;
    world.hunts.insert(id, next);
    reindex(world);
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let t = Some(s.target);
    let mut steps = vec![step(ActionKind::GoTo(LocationKey::Intel), None)];
    if phase == HuntPhase::Ask {
        steps.push(step(ActionKind::AskAround, t));
        steps.push(step(ActionKind::GoTo(LocationKey::Intel), None));
    }
    steps.push(step(ActionKind::StakeOut, t));
    steps.push(step(ActionKind::Attack, t));
    Some(Plan { goal: GoalKind::Hunt, target: t, steps, started_tick: world.tick })
}

/// `LocationKey::Intel` for `agent`: the venue while asking, the intel
/// building while watching (`None` for a street intel).
pub fn intel_building(world: &World, agent: EntityId) -> Option<EntityId> {
    let s = chase(world, agent)?;
    match s.phase {
        HuntPhase::Ask => s.venue,
        HuntPhase::Watch => s.intel.and_then(|i| i.building),
    }
}

/// `LocationKey::Intel`'s tile for `agent` when it is no building.
pub fn intel_tile(world: &World, agent: EntityId) -> Option<crate::components::TilePos> {
    let s = chase(world, agent)?;
    match s.phase {
        HuntPhase::Ask => None,
        HuntPhase::Watch => s.intel.map(|i| i.tile),
    }
}

/// W22: a `GoTo(Intel)` starts: in `Watch` a Statistical target is
/// promoted to Coarse (it snaps to its phase door) and marked hunted.
pub fn on_goto_intel(world: &mut World, hunter: EntityId) {
    let Some(s) = chase(world, hunter) else { return };
    if s.phase != HuntPhase::Watch {
        return;
    }
    let t = s.target;
    if world.comp::<Brain>(t).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, t, Lod::Coarse);
    }
    reindex(world);
}

/// Is the target within 2 tiles of the hunter or in its building?
pub fn contact(world: &World, hunter: EntityId) -> bool {
    chase(world, hunter).is_some_and(|s| crate::systems::law::near(world, hunter, s.target, 2))
}

/// The scripted steps' start check (in place of the planner's symbolic
/// preconditions): `AskAround` at the venue, `StakeOut` at the intel (a
/// Hunt) or beside the body (GuardBody), `Attack` within 4 tiles of a
/// living target.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    let pos = world.comp::<Position>(id);
    let at_building = |b: EntityId| {
        pos.is_some_and(|p| {
            p.building == Some(b)
                || world
                    .comp::<Building>(b)
                    .is_some_and(|bd| crate::systems::law::chebyshev(p.tile, world.outside_door(bd)) <= 1)
        })
    };
    match kind {
        ActionKind::AskAround => {
            chase(world, id).and_then(|s| s.venue).is_some_and(at_building)
                && chase(world, id).is_some_and(|s| target_ok(world, s.target))
        }
        ActionKind::StakeOut => {
            let goal = world.comp::<Brain>(id).and_then(|b| b.plan_goal());
            if goal == Some(GoalKind::GuardBody) {
                return target.is_some_and(|c| {
                    world.comp::<crate::components::Corpse>(c).is_some_and(|k| !k.buried)
                        && crate::systems::law::near(world, id, c, 1)
                });
            }
            let Some(s) = chase(world, id) else { return false };
            if !target_ok(world, s.target) {
                return false;
            }
            match s.intel {
                Some(Intel { building: Some(b), .. }) => at_building(b),
                Some(i) => pos.is_some_and(|p| crate::systems::law::chebyshev(p.tile, i.tile) <= 1),
                None => false,
            }
        }
        ActionKind::Attack => {
            target.is_some_and(|t| crate::systems::law::living(world, t) && crate::systems::law::near(world, id, t, 4))
        }
        _ => crate::exec::actions::can_start(world, id, kind, target),
    }
}

/// W22: the stake-out begins (its timeout noted).
pub fn start_stakeout(world: &mut World, hunter: EntityId, until: Tick) {
    if let Some(mut s) = chase(world, hunter) {
        s.stakeout_until = Some(until);
        set_chase(world, hunter, s);
    }
}

/// W22: the stake-out ran out with no contact: the Hunt goes back to
/// asking (intel cleared); a hunter sent the wrong way finds out
/// (`Deceived`, and a grudge on the liar at 0.3).
pub fn stakeout_failed(world: &mut World, hunter: EntityId) -> StepResult {
    let Some(s) = chase(world, hunter) else {
        return StepResult::Failed(crate::exec::FailReason::PreconditionLost);
    };
    if s.deceived {
        if let Some(liar) = s.liar {
            let text = format!("{} sent {} the wrong way", world.name_of(liar), world.name_of(hunter));
            world.push_event(EventKind::Deceived, &[hunter, liar], text);
            crate::systems::grudges::add(world, hunter, liar, GrudgeCause::Betrayed, 0.3, 0);
        }
    }
    if let Some(mut x) = chase(world, hunter) {
        x.phase = HuntPhase::Ask;
        x.intel = None;
        x.venue = None;
        x.deceived = false;
        x.liar = None;
        x.stakeout_until = None;
        set_chase(world, hunter, x);
    }
    reindex(world);
    StepResult::Failed(crate::exec::FailReason::PreconditionLost)
}

/// The kill chance of a strike: `Some(hunt_kill_p)` when `hunter` hunts
/// `victim` on a grudge of `lethal_min` or more.
pub fn strike_kill_p(world: &World, hunter: EntityId, victim: EntityId) -> Option<f32> {
    // M16a (plan C15): a contract Hit's taker strikes its target at
    // `hunt_kill_p` (a Beat at the base rate).
    if !world.contract_runs.is_empty() && !world.hunts.contains_key(&hunter) {
        return crate::systems::contracts::strike_kill_p(world, hunter, victim);
    }
    let s = world.hunts.get(&hunter).filter(|s| s.target == victim)?;
    let w = crate::systems::grudges::grudge_on(world, hunter, victim).map_or(s.weight, |g| g.weight);
    (w >= world.config.hunt.lethal_min).then_some(world.config.hunt.hunt_kill_p)
}

/// The respondent of an AskAround at `venue`: the co-occupant with the
/// highest affinity to the target (any edge; ties the lower id).
fn respondent(world: &World, hunter: EntityId, target: EntityId, venue: EntityId) -> Option<EntityId> {
    let b = world.comp::<Building>(venue)?;
    b.occupants
        .iter()
        .copied()
        .filter(|&o| o != hunter && o != target && crate::systems::law::living(world, o))
        .filter(|&o| crate::systems::demography::is_adult(world, o) && world.has::<Memory>(o))
        .filter_map(|o| world.edge(o, target).map(|e| (o, e.affinity)))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
        .map(|(o, _)| o)
}

/// W21: `AskAround` completes. The respondent (else the Trace habit, no
/// move) trades one exchange each way with the hunter; a Friend of the
/// target may lie first (a Deceive against the hunter's knowledge: a
/// success sends the hunter to a random other Bar); else the hunter's move
/// (Charm on its own Friend, Intimidate when its dread is the higher,
/// Persuade otherwise, `+intel_k × knowledge` on its side): a success gives
/// the respondent's freshest sighting of the target, else the habit; a
/// failure leaves the target's Home. The Hunt moves to `Watch`.
pub fn ask_around(world: &mut World, hunter: EntityId) -> StepResult {
    let Some(s) = chase(world, hunter) else {
        return StepResult::Failed(crate::exec::FailReason::PreconditionLost);
    };
    let target = s.target;
    if !target_ok(world, target) {
        return StepResult::Failed(crate::exec::FailReason::PartnerLeft);
    }
    let now = world.tick;
    let venue = s.venue.or_else(|| world.comp::<Position>(hunter).and_then(|p| p.building));
    let r = venue.and_then(|v| respondent(world, hunter, target, v));
    let mut deceived_by = None;
    let intel = match r {
        None => habit(world, target, now),
        Some(r) => {
            crate::systems::gossip::exchange(world, r, hunter, crate::systems::gossip::Venue::Ask);
            crate::systems::gossip::exchange(world, hunter, r, crate::systems::gossip::Venue::Ask);
            let friend = world.edge(r, target).is_some_and(|e| e.kind == RelKind::Friend);
            let moves = crate::systems::moves::on(world);
            let lie = friend && moves && {
                let m = SocialMove {
                    actor: r,
                    target: hunter,
                    kind: MoveKind::Deceive,
                    stake: Stake::Info { about: target },
                };
                crate::systems::moves::resolve(world, &m).success
            };
            if lie {
                deceived_by = Some(r);
                wrong_bar(world, hunter, r, target)
            } else {
                let success = !moves || {
                    let kind = if world.edge(hunter, r).is_some_and(|e| e.kind == RelKind::Friend) {
                        MoveKind::Charm
                    } else if crate::systems::reputation::rep(world, hunter).dread
                        > crate::systems::reputation::rep(world, r).dread
                    {
                        MoveKind::Intimidate
                    } else {
                        MoveKind::Persuade
                    };
                    let bonus = world.config.hunt.intel_k * crate::systems::moves::knowledge(world, hunter);
                    let m = SocialMove { actor: hunter, target: r, kind, stake: Stake::Info { about: target } };
                    crate::systems::moves::resolve_with(world, &m, bonus).success
                };
                if success {
                    sightings_of(world, r, target)
                        .map(|(_, b, tile)| Intel { building: b, tile, source: IntelSource::Asked })
                        .unwrap_or_else(|| habit(world, target, now))
                } else {
                    world
                        .comp::<Household>(target)
                        .and_then(|h| h.home)
                        .and_then(|h| intel_at(world, h, IntelSource::Home))
                        .unwrap_or_else(|| habit(world, target, now))
                }
            }
        }
    };
    if let Some(mut x) = chase(world, hunter) {
        x.phase = HuntPhase::Watch;
        x.intel = Some(intel);
        x.stakeout_until = None;
        x.deceived = deceived_by.is_some();
        x.liar = deceived_by;
        set_chase(world, hunter, x);
    }
    reindex(world);
    StepResult::Done
}

/// A Friend's lie: a random Bar (Hunt stream) other than where the habit
/// puts the target.
fn wrong_bar(world: &World, hunter: EntityId, liar: EntityId, target: EntityId) -> Intel {
    let truth = habit(world, target, world.tick).building;
    let bars: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .filter(|&b| Some(b) != truth && world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .collect();
    if bars.is_empty() {
        return habit(world, target, world.tick);
    }
    let mut rng = world.rng.word(WordNs::Hunt, world.tick, (u64::from(hunter.index) << 32) | u64::from(liar.index));
    let b = bars[rng.random_range(0..bars.len())];
    intel_at(world, b, IntelSource::Deceived).unwrap_or_else(|| habit(world, target, world.tick))
}

/// W22, from `Attack`'s completion after its crime is raised: a hunter
/// striking its Hunt's target. A win posts the `Avenged` deed (and gives a
/// living target the first-hand memory), sets `kill_chain[target]` to the
/// grudge's chain + 1, settles the grudge (a death) or takes `beat_settle`
/// off it; a loss adds 0.1 and cools the Hunt `hunt_cooldown_days`. The
/// `HuntState` ends either way.
pub fn on_strike(world: &mut World, hunter: EntityId, target: EntityId, winner: EntityId, died: bool) {
    // M16a (plan C15): a contract taker's strike (no Hunt: returns below).
    crate::systems::contracts::on_strike(world, hunter, target, winner, died);
    let Some(s) = world.hunts.get(&hunter).filter(|s| s.target == target).cloned() else { return };
    world.hunts.remove(&hunter);
    reindex(world);
    let now = world.tick;
    let cfg = world.config.clone();
    // Settled already when the strike killed (by `grudges::on_death`).
    let grudge = world.comp::<Grudges>(hunter).and_then(|g| g.list.iter().rev().find(|x| x.target == target).cloned());
    if winner == hunter {
        world.kill_chain.insert(target, (s.chain.saturating_add(1), now));
        let tile = world
            .comp::<Position>(target)
            .or_else(|| world.comp::<Position>(hunter))
            .map_or_else(Default::default, |p| p.tile);
        let d = world.district_of(tile);
        crate::systems::gossip::post_deed(world, d, Deed::Avenged, Some(hunter), Some(target));
        if crate::systems::law::living(world, target) && world.has::<Memory>(target) {
            let sal = cfg.gossip.deed_sal.get(Deed::Avenged);
            let e = MemoryEntry {
                subject: Some(hunter),
                salience: sal,
                valence: -cfg.gossip.deed_sev.get(Deed::Avenged) * sal,
                deed: Some(Deed::Avenged),
                object: Some(target),
                ..MemoryEntry::blank(MemoryKind::Rumour, now)
            };
            crate::systems::memory::hear_entry(world, target, e);
        }
        let for_whom = match grudge.map(|g| g.cause) {
            Some(GrudgeCause::KilledKin(v) | GrudgeCause::KilledFriend(v) | GrudgeCause::Stripped(v)) => {
                world.name_of(v)
            }
            Some(GrudgeCause::Inherited(v)) => format!("{}'s feud", world.name_of(v)),
            _ => "themself".to_string(),
        };
        let text = format!(
            "{} avenged {for_whom} on {}{}",
            world.name_of(hunter),
            world.name_of(target),
            if died { " (killed)" } else { "" }
        );
        world.push_event(EventKind::Avenged, &[hunter, target], text);
        if let Some(g) = world.comp_mut::<Grudges>(hunter) {
            if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
                if died {
                    x.settled = Some(now);
                } else {
                    x.weight -= cfg.grudges.beat_settle;
                    if x.weight <= 0.0 {
                        x.settled = Some(now);
                        x.weight = 0.0;
                    }
                }
            }
        }
        let w = &mut world.stats.current.word;
        w.avenged += 1;
        if died {
            w.revenge_kills += 1;
        }
    } else {
        if let Some(g) = world.comp_mut::<Grudges>(hunter) {
            if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
                x.weight = (x.weight + 0.1).min(1.0);
            }
        }
        cool(world, hunter);
        world.stats.current.word.hunts_failed += 1;
    }
}

fn cool(world: &mut World, hunter: EntityId) {
    let until = world.tick + Tick::from(world.config.hunt.hunt_cooldown_days) * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(hunter) {
        b.cooldowns.insert(GoalKind::Hunt, until);
    }
}

/// End `hunter`'s Hunt without a strike (its plan aborted if it is the Hunt's).
fn end(world: &mut World, hunter: EntityId) {
    world.hunts.remove(&hunter);
    if world.comp::<Brain>(hunter).and_then(|b| b.plan_goal()) == Some(GoalKind::Hunt) {
        world.abort_plan(hunter);
    }
}

/// From `grudges::on_death`: the dead's own Hunt ends, and every Hunt on
/// the dead (its plan fails at the next step).
pub fn on_death(world: &mut World, dead: EntityId) {
    if world.hunts.is_empty() {
        return;
    }
    // A Hunt on the dead stays until `tick`: the strike that killed it
    // still reads it (`on_strike` runs after the fight's death).
    world.hunts.remove(&dead);
    reindex(world);
}

/// W22, every tick (`word::run`) over the Hunts only (scan-ok: bounded by
/// `max_hunts`): a Hunt whose hunter or target cannot go on (dead,
/// jailed, cuffed, leaving) ends.
pub fn tick(world: &mut World) {
    if world.hunts.is_empty() {
        return;
    }
    let dead: Vec<EntityId> = world
        .hunts
        .iter()
        .filter(|(&h, s)| !hunter_ok(world, h) || !target_ok(world, s.target))
        .map(|(&h, _)| h)
        .collect();
    if dead.is_empty() {
        return;
    }
    for h in dead {
        end(world, h);
    }
    reindex(world);
}

/// W22/W23 at midnight: a Hunt past `hunt_days` is abandoned (`weight ×=
/// 0.7`, `HuntAbandoned`, cooled); then the Statistical pass.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    tick(world);
    let now = world.tick;
    let limit = Tick::from(world.config.hunt.hunt_days) * TICKS_PER_DAY;
    let stale: Vec<(EntityId, EntityId)> =
        world.hunts.iter().filter(|(_, s)| now.saturating_sub(s.since) >= limit).map(|(&h, s)| (h, s.target)).collect();
    for (h, t) in stale {
        end(world, h);
        if let Some(g) = world.comp_mut::<Grudges>(h) {
            if let Some(x) = g.list.iter_mut().find(|x| x.target == t && x.settled.is_none()) {
                x.weight *= 0.7;
            }
        }
        cool(world, h);
        world.stats.current.word.hunts_abandoned += 1;
        let text = format!("{} gave up on {}", world.name_of(h), world.name_of(t));
        world.push_event(EventKind::HuntAbandoned, &[h, t], text);
    }
    reindex(world);
    stat_pass(world);
}

/// W23: Statistical adults with an eligible grudge (the grudge store, not
/// a citizen scan), each on one hash-picked day in three, score the Hunt;
/// those at `stat_hunt_min` or more, heaviest grudge first (ties the lower
/// id), are promoted to Coarse and take up the Hunt while slots last.
pub fn stat_pass(world: &mut World) {
    if !on(world) || world.hunts.len() >= world.config.hunt.max_hunts {
        return;
    }
    let day = world.day();
    let seed = world.seed();
    let min = world.config.hunt.stat_hunt_min;
    let mut queue: Vec<(f32, EntityId)> = Vec::new();
    // scan-ok: daily, the grudge store.
    for id in world.with::<Grudges>() {
        if world.comp::<Brain>(id).is_none_or(|b| b.lod != Lod::Statistical) || world.hunts.contains_key(&id) {
            continue;
        }
        if !splitmix64(seed ^ u64::from(id.index) ^ day).is_multiple_of(3) || cooled(world, id) || !hunter_ok(world, id)
        {
            continue;
        }
        let Some((_, w, _)) = heaviest_eligible(world, id) else { continue };
        let Some((cs, flat)) = considerations(world, id) else { continue };
        let Some(s) = crate::utility::score_goal(GoalKind::Hunt, cs, None, 0.0, flat) else { continue };
        if s.score >= min {
            queue.push((w, id));
        }
    }
    queue.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, id) in queue {
        if world.hunts.len() >= world.config.hunt.max_hunts {
            break;
        }
        crate::systems::lod::set_lod(world, id, Lod::Coarse);
        adopt(world, id, HuntWhy::Stat);
    }
    reindex(world);
}

/// W42 god `Hunt`: `hunter` holds a 1.0 grudge on `target` and hunts it
/// now, past the cap (promoted from Statistical).
pub fn god_hunt(world: &mut World, hunter: EntityId, target: EntityId) -> Result<(), String> {
    if !on(world) {
        return Err("the Hunt is off".into());
    }
    if !hunter_ok(world, hunter) {
        return Err("Hunt: the hunter cannot hunt".into());
    }
    if hunter == target || !target_ok(world, target) || !world.has::<crate::components::Identity>(target) {
        return Err("Hunt: no huntable target".into());
    }
    if let Some(s) = world.hunts.get(&hunter) {
        if s.target != target {
            end(world, hunter);
        }
    }
    crate::systems::grudges::add(world, hunter, target, GrudgeCause::Assaulted, 1.0, 0);
    if let Some(g) = world.comp_mut::<Grudges>(hunter) {
        if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
            x.weight = 1.0;
        }
    }
    if world.comp::<Brain>(hunter).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, hunter, Lod::Coarse);
    }
    // The god's target, not the heaviest grudge.
    if !world.hunts.contains_key(&hunter) {
        let gap = might_gap(world, hunter, target);
        world.hunts.insert(
            hunter,
            HuntState {
                target,
                grudge_target: target,
                chain: 0,
                since: world.tick,
                phase: HuntPhase::Ask,
                venue: None,
                intel: None,
                stakeout_until: None,
                deceived: false,
                why: HuntWhy::God,
                liar: None,
                weight: 1.0,
                gap,
            },
        );
        world.stats.current.word.hunts += 1;
        let text = format!("{} went looking for {}", world.name_of(hunter), world.name_of(target));
        world.push_event(EventKind::HuntStarted, &[hunter, target], text);
    }
    if let Some(b) = world.comp_mut::<Brain>(hunter) {
        b.cooldowns.remove(&GoalKind::Hunt);
    }
    reindex(world);
    Ok(())
}
