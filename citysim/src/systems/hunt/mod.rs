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

mod chases;
mod inside;
mod intel;
mod strike;

pub use chases::*;
pub use inside::*;
pub use intel::*;
pub use strike::*;

/// May `h` hunt at all: a living adult, free, not leaving. Round 9 Z1:
/// never one under 18 (`ages::is_protected`, the hard rule).
pub fn hunter_ok(world: &World, h: EntityId) -> bool {
    crate::systems::law::living(world, h)
        && crate::systems::demography::is_adult(world, h)
        && !crate::systems::ages::is_protected(world, h)
        && !world.has::<Sentence>(h)
        && world.comp::<Brain>(h).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
        && !crate::systems::abroad::is_away(world, h)
}

/// Can `t` be hunted: a living agent, free, not leaving. Round 9 Z1:
/// never one under 18 (`ages::is_protected`, the hard rule).
pub fn target_ok(world: &World, t: EntityId) -> bool {
    crate::systems::law::living(world, t)
        && !crate::systems::ages::is_protected(world, t)
        && !world.has::<Sentence>(t)
        && world.comp::<Brain>(t).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
        && !crate::systems::abroad::is_away(world, t)
}

/// W20: the heaviest unsettled grudge of `holder` on a huntable agent with
/// `weight ≥ hunt_min` (ties the lower target id): `(target, weight, chain)`.
pub fn heaviest_eligible(world: &World, holder: EntityId) -> Option<(EntityId, f32, u8)> {
    world
        .comp::<Grudges>(holder)?
        .list
        .iter()
        .filter(|x| x.settled.is_none() && x.weight >= min_for(world, x.cause) && x.target != holder)
        // Round 9 Z1: no third hunter on one man (`hunters_per_target`).
        .filter(|x| !crowded(world, holder, x.target))
        // The Hit market (gaps pass): a stiffed gun's grudge waits out the
        // debt's grace (`hitmarket::stiff_sweep`).
        // Only the betrayal waits (fix round): a grudge of another cause on
        // the same buyer (a killing) counts as ever.
        .filter(|x| {
            x.cause != GrudgeCause::Betrayed
                || world.hm_stiffs.is_empty()
                || !world.hm_stiffs.contains_key(&(holder, x.target))
        })
        .filter(|x| world.has::<crate::components::Identity>(x.target) && target_ok(world, x.target))
        .max_by(|a, b| a.weight.total_cmp(&b.weight).then(b.target.cmp(&a.target)))
        .map(|x| (x.target, x.weight, x.chain))
}

/// Round 9 Z1: the lines said on a grudge (`Grudge::noted`): the killer
/// held (the wait), left to the law, no word near (the wait for word).
pub const NOTED_HELD: u8 = 1;
pub const NOTED_LAW: u8 = 2;
pub const NOTED_WORD: u8 = 4;
/// Z1: left to the hunters already after him (`hunters_per_target`).
pub const NOTED_LEFT: u8 = 8;

/// Z1: the dead a grudge is over (a killing of kin or a friend).
pub fn killing(cause: GrudgeCause) -> Option<EntityId> {
    match cause {
        GrudgeCause::KilledKin(v) | GrudgeCause::KilledFriend(v) => Some(v),
        _ => None,
    }
}

/// Z1: a killing, or a killing handed on (`Inherited`).
fn blood(cause: GrudgeCause) -> bool {
    killing(cause).is_some() || matches!(cause, GrudgeCause::Inherited(_))
}

/// Z1: the weight a grudge of `cause` hunts from: a killing (and one handed
/// on) at `killed_hunt_min`, anything else at `hunt_min`.
pub fn min_for(world: &World, cause: GrudgeCause) -> f32 {
    let h = &world.config.hunt;
    if blood(cause) && h.killed_hunt_min > 0.0 {
        h.killed_hunt_min.min(h.hunt_min)
    } else {
        h.hunt_min
    }
}

/// Z1: `target` is hunted by `hunters_per_target` others already (the
/// holder's own Hunt on him not counted). Bounded by `max_hunts`.
fn crowded(world: &World, holder: EntityId, target: EntityId) -> bool {
    let cap = world.config.hunt.hunters_per_target;
    cap > 0
        && !world.hunts.is_empty()
        && world.hunts.iter().filter(|(&h, s)| h != holder && s.target == target).count() >= cap
}

/// Z1: whom a grudge is for ("Joss Inchbald", "Moth Wexford's feud").
fn for_whom(world: &World, cause: Option<GrudgeCause>) -> String {
    // The dead's name while its entity stands; once the body is gone
    // (`name_of` reads `#id`) the deed alone (as `hitmarket::revenge_why`).
    // A mourner still holds the name (`grief::Mourning`).
    let named = |v: EntityId| world.has::<crate::components::Identity>(v);
    let mourned = |v: EntityId| world.grief.mourners.values().find(|m| m.of == v).map(|m| m.name.clone());
    match cause {
        Some(GrudgeCause::KilledKin(v) | GrudgeCause::KilledFriend(v) | GrudgeCause::Stripped(v)) if named(v) => {
            world.name_of(v)
        }
        Some(GrudgeCause::KilledKin(v) | GrudgeCause::KilledFriend(v)) => {
            mourned(v).unwrap_or_else(|| "a killing".to_string())
        }
        Some(GrudgeCause::Inherited(v)) if named(v) => format!("{}'s feud", world.name_of(v)),
        Some(GrudgeCause::Inherited(_)) => "a feud handed down".to_string(),
        _ => "themself".to_string(),
    }
}

/// Z1: " over Joss Inchbald" for a grudge over a killing, else "".
fn over(world: &World, holder: EntityId, target: EntityId) -> String {
    let cause = crate::systems::grudges::grudge_on(world, holder, target).map(|g| g.cause);
    if cause.is_some_and(blood) {
        format!(" over {}", for_whom(world, cause))
    } else {
        String::new()
    }
}

/// Z1: set `bit` on `holder`'s unsettled grudge on `target`.
fn mark(world: &mut World, holder: EntityId, target: EntityId, bit: u8) {
    if let Some(g) = world.comp_mut::<Grudges>(holder) {
        if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
            x.noted |= bit;
        }
    }
}

fn noted(world: &World, holder: EntityId, target: EntityId, bit: u8) -> bool {
    crate::systems::grudges::grudge_on(world, holder, target).is_some_and(|g| g.noted & bit != 0)
}

/// Z1: a hunt's line (`EventKind::Hunt`).
fn line(world: &mut World, actors: &[EntityId], text: String) {
    world.push_event(EventKind::Hunt, actors, text);
}

/// Z1: a hunt that ends, with the line that says how.
fn ended(world: &mut World, hunter: EntityId, text: String) {
    line(world, &[hunter], text);
    world.stats.current.word.hunts_ended += 1;
}

/// Z1: where a held man is and till when ("the cells of Precinct#4460 till
/// d43"), or the law's cuffs.
fn held_where(world: &World, t: EntityId) -> String {
    match world.comp::<Sentence>(t) {
        Some(s) => {
            let at = crate::systems::law::cells_of(world, t)
                .map_or_else(|| "the cells".to_string(), |b| format!("the cells of {}", world.name_of(b)));
            format!("{at} till d{}", s.until_tick / TICKS_PER_DAY)
        }
        None => "the law's cuffs".to_string(),
    }
}

/// Z1: may `o` take up a grudge on `target` handed him: not of the
/// target's gang, and no kin, friend or warm edge of his (V18's 20k
/// `gang_member_6252`: a member took his leader's feud on his own Friend).
fn takes_against(world: &World, o: EntityId, target: EntityId) -> bool {
    let theirs = world.gang_of(target);
    (theirs.is_none() || world.gang_of(o) != theirs)
        && world.edge(o, target).is_none_or(|e| {
            e.affinity <= 0.0
                && !matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend)
        })
}

/// Z1: in the cells or in cuffs.
fn held(world: &World, t: EntityId) -> bool {
    world.has::<Sentence>(t) || world.comp::<Brain>(t).is_some_and(|b| b.cuffed_by.is_some())
}

fn pro(world: &World, id: EntityId) -> (&'static str, &'static str) {
    if crate::systems::grief::his(world, id) == "his" {
        ("he", "him")
    } else {
        ("she", "her")
    }
}

fn cooled(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).and_then(|b| b.cooldowns.get(&GoalKind::Hunt)).is_some_and(|&t| t > world.tick)
}

/// W20: the Hunt's considerations (spec § 4 table). A hunter with a
/// `HuntState` is scored on its target whatever the cap; anyone else needs
/// an eligible grudge and a free slot under `max_hunts`.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !hunter_ok(world, id) {
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
            // The Hit market (D8, phase 4 fix): no Hunt on a head it is paying for.
            if crate::systems::hitmarket::paying_for(world, id, t) {
                return None;
            }
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
    // Round 8 X2 (`OPEN-247`): the mourner's Hunt on the killer holds while
    // the grudge is hot (`[grief] hunt_hot_flat`).
    Some((cs, world.config.hunt.hunt_flat + crate::systems::grief::hunt_flat(world, id, target)))
}

/// M16a (plan C20): the Hunt's considerations on `target` at `weight`
/// without the might-gap term (the hire pass's score), at any tier.
pub fn hire_considerations(
    world: &World,
    id: EntityId,
    target: EntityId,
    weight: f32,
) -> Option<(Vec<Consideration>, f32)> {
    if !hunter_ok(world, id) || !target_ok(world, target) {
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
        // Round 9 Z1: the end says how.
        let t = world.hunts.get(&h).map(|s| s.target);
        let words = t.and_then(|t| end_words(world, h, t));
        end(world, h);
        if let Some(text) = words {
            ended(world, h, text);
        }
    }
    reindex(world);
}

/// W22/W23 at midnight: a Hunt past `hunt_days` is abandoned (`weight ×=
/// 0.7`, `HuntAbandoned`, cooled); then the Statistical pass.
pub fn daily(world: &mut World) {
    tick(world);
    let now = world.tick;
    let limit = Tick::from(world.config.hunt.hunt_days) * TICKS_PER_DAY;
    let stale: Vec<(EntityId, EntityId)> =
        world.hunts.iter().filter(|(_, s)| now.saturating_sub(s.since) >= limit).map(|(&h, s)| (h, s.target)).collect();
    for (h, t) in stale {
        end(world, h);
        // Round 9 Z1: the words before the weight falls (`over`).
        let text = format!(
            "{} gave up on {}{} after {} days of looking",
            world.name_of(h),
            world.name_of(t),
            over(world, h, t),
            world.config.hunt.hunt_days
        );
        if let Some(g) = world.comp_mut::<Grudges>(h) {
            if let Some(x) = g.list.iter_mut().find(|x| x.target == t && x.settled.is_none()) {
                x.weight *= 0.7;
            }
        }
        cool(world, h);
        world.stats.current.word.hunts_abandoned += 1;
        world.stats.current.word.hunts_ended += 1;
        world.push_event(EventKind::HuntAbandoned, &[h, t], text);
    }
    reindex(world);
    held_pass(world);
    stat_pass(world);
}

/// W23: Statistical adults with an eligible grudge (the grudge store, not
/// a citizen scan), each on one hash-picked day in three, score the Hunt;
/// those at `stat_hunt_min` or more, heaviest grudge first (ties the lower
/// id), are promoted to Coarse and take up the Hunt while slots last.
pub fn stat_pass(world: &mut World) {
    if world.hunts.len() >= world.config.hunt.max_hunts {
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
