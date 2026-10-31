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
    // The Hit market (D8, phase 4 fix): a buyer does not hunt the head it is
    // paying for (a god's Hunt still goes ahead and ends the want).
    if why != HuntWhy::God && crate::systems::hitmarket::paying_for(world, id, target) {
        return false;
    }
    // M16a (plan C20): a holder too weak to win who can pay hires instead
    // (a Goal or Stat Hunt; never a god's). Round 9 Z1: one who cannot pay
    // hands the grudge on (`hand_off`).
    if why != HuntWhy::God && hand_off(world, id, target, weight, chain) {
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
    heard(world, id, target);
    true
}

/// Round 9 Z1: a holder who would hire rather than go (too weak or too
/// cowardly, `hitmarket::hires`) hands the grudge on, each with a line: a
/// lawful one leaves a wanted man to the law (`hitmarket::waits_for_law`);
/// one who can pay hires through a fixer (`contracts::try_hire`, the posting
/// path; the take is the market's); one who cannot asks the strongest of
/// his own people who will go (`hand_on`), else tells the law of a killing
/// (`tell_law`). True when handed (no Hunt taken up). The under-18 rule:
/// a minor neither holds a Hunt nor takes one (`hunter_ok`).
fn hand_off(world: &mut World, id: EntityId, target: EntityId, weight: f32, chain: u8) -> bool {
    let hires = crate::systems::hitmarket::hires(world, id, target);
    let law_wait = crate::systems::hitmarket::waits_for_law(world, id, target);
    if hires && law_wait && !noted(world, id, target, NOTED_LAW) {
        let what = crate::systems::law::wanted_for(world, target)
            .map_or_else(|| "the law has the name".to_string(), |c| format!("wanted for {}", c.label()));
        let text = format!(
            "{} leaves {} to the law{}: {what}",
            world.name_of(id),
            world.name_of(target),
            over(world, id, target)
        );
        line(world, &[id], text);
        mark(world, id, target, NOTED_LAW);
        world.stats.current.word.hunts_handed += 1;
    }
    let wanting = world.wants.contains_key(&id);
    if crate::systems::contracts::try_hire(world, id, target, weight) {
        if !law_wait {
            world.stats.current.word.hunts_handed += 1;
            // The walk to the fixer is the errand (H9); a direct post has
            // its own line.
            if let Some(f) = world.wants.get(&id).filter(|w| !wanting && w.target == target).map(|w| w.fixer) {
                let text = format!(
                    "{}, {}, will pay to have it done{}: off to {}",
                    world.name_of(id),
                    unfit(world, id, target),
                    over(world, id, target),
                    world.name_of(f)
                );
                line(world, &[id, f], text);
            }
        }
        return true;
    }
    if !hires {
        return false;
    }
    hand_on(world, id, target, weight, chain) || tell_law(world, id, target)
}

/// Z1: why `id` does not go himself, in words.
fn unfit(world: &World, id: EntityId, target: EntityId) -> String {
    let old = u64::from(world.config.ages.frail_from_years) * crate::time::DAYS_PER_YEAR;
    if world.comp::<crate::components::Identity>(id).is_some_and(|i| u64::from(i.age_days) >= old) {
        "too old to fight".to_string()
    } else if world.comp::<Personality>(id).is_some_and(|p| p.courage < world.config.hitmarket.hire_courage) {
        "without the nerve".to_string()
    } else {
        format!("no match for {}", pro(world, target).1)
    }
}

/// Z1: the Hunt held off `days` (a grudge handed on is not hunted twice).
fn hold_off(world: &mut World, id: EntityId, days: u32) {
    let until = world.tick + Tick::from(days) * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        let t = b.cooldowns.entry(GoalKind::Hunt).or_insert(until);
        *t = (*t).max(until);
    }
}

/// Z1: a grudge over a killing handed to the strongest of `id`'s own who
/// will go himself (kin, a friend, a gang mate: a free adult not hunting,
/// not of the target's gang, who would not hire): an `Inherited` grudge at
/// `hand_w` of the weight, the line, and `id`'s Hunt held off `hunt_days`.
fn hand_on(world: &mut World, id: EntityId, target: EntityId, weight: f32, chain: u8) -> bool {
    let share = world.config.hunt.hand_w;
    if share <= 0.0 {
        return false;
    }
    if !crate::systems::grudges::grudge_on(world, id, target).is_some_and(|g| blood(g.cause)) {
        return false;
    }
    let w = (weight * share).min(1.0);
    if w < min_for(world, GrudgeCause::Inherited(id)) {
        return false;
    }
    let mut cands: Vec<EntityId> = world
        .neighbours
        .get(&id)
        .map(|n| {
            n.iter()
                .copied()
                .filter(|&o| {
                    world.edge(id, o).is_some_and(|e| {
                        matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend)
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(g) = world.gang_of(id).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        cands.extend(g.members.iter().copied());
    }
    let Some((_, o)) = cands
        .into_iter()
        .filter(|&o| o != id && o != target && hunter_ok(world, o) && !world.hunts.contains_key(&o))
        .filter(|&o| takes_against(world, o, target))
        .filter(|&o| !crate::systems::hitmarket::hires(world, o, target))
        .map(|o| (might_gap(world, o, target), o))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
    else {
        return false;
    };
    let text = format!(
        "{}, {}, asked {} to go after {}{}",
        world.name_of(id),
        unfit(world, id, target),
        world.name_of(o),
        world.name_of(target),
        over(world, id, target)
    );
    line(world, &[id, o], text);
    crate::systems::grudges::add(world, o, target, GrudgeCause::Inherited(id), w, chain);
    world.stats.current.word.hunts_handed += 1;
    let days = world.config.hunt.hunt_days;
    hold_off(world, id, days);
    true
}

/// Z1: a killing told to the law by a holder who tells it
/// (`law::tells_the_law`) when the killer is not wanted already: a report
/// naming him, its line, the Hunt held off `hunt_cooldown_days`.
fn tell_law(world: &mut World, id: EntityId, target: EntityId) -> bool {
    if world.config.hunt.hand_w <= 0.0 || noted(world, id, target, NOTED_LAW) {
        return false;
    }
    let Some(dead) = crate::systems::grudges::grudge_on(world, id, target).and_then(|g| killing(g.cause)) else {
        return false;
    };
    if crate::systems::law::wanted(world, target) || !crate::systems::law::tells_the_law(world, id, target) {
        return false;
    }
    let text = format!(
        "{}, {}, told the law {} killed {}",
        world.name_of(id),
        unfit(world, id, target),
        world.name_of(target),
        if world.has::<crate::components::Identity>(dead) {
            world.name_of(dead)
        } else {
            format!("one of {} own", crate::systems::grief::his(world, id))
        }
    );
    let actors = [id, target];
    if !crate::systems::law::file_report_as(
        world,
        crate::components::Crime::Murder,
        target,
        Some(id),
        Some((text, &actors)),
    ) {
        return false;
    }
    mark(world, id, target, NOTED_LAW);
    world.stats.current.word.hunts_handed += 1;
    let days = world.config.hunt.hunt_cooldown_days;
    hold_off(world, id, days);
    true
}

/// Z1: the hunted gets word (`heard_p`, a hash of the pair): his safety
/// falls, he remembers the threat, and he acts with a line: one brave and
/// lawless enough swears to get the hunter first (a grudge at
/// `heard_counter_w`), anyone else lies low at home `heard_low_days`
/// (`grief::lie_low`). Never on a minor (`target_ok`, `hunter_ok`).
fn heard(world: &mut World, hunter: EntityId, target: EntityId) {
    const PURPOSE_HUNTED: u64 = 0x2a_4e_7d_31;
    let c = world.config.hunt.clone();
    if c.heard_p <= 0.0 || !target_ok(world, target) || !world.has::<crate::components::Identity>(target) {
        return;
    }
    let roll = crate::systems::econ::hash_unit(
        world.seed(),
        PURPOSE_HUNTED,
        u64::from(hunter.index),
        (u64::from(target.index) << 20) ^ time::day(world.tick),
    );
    if roll >= c.heard_p {
        return;
    }
    if let Some(n) = world.comp_mut::<crate::components::Needs>(target) {
        n.safety = (n.safety - c.heard_safety).max(0.0);
    }
    world.remember(target, MemoryKind::Threatened, Some(hunter), 0.6, -0.5, false);
    world.stats.current.word.hunted_heard += 1;
    let (tn, hn) = (world.name_of(target), world.name_of(hunter));
    let (he, him) = pro(world, target);
    let why = over(world, hunter, target);
    let bold = world
        .comp::<Personality>(target)
        .is_some_and(|p| p.courage >= c.heard_counter_courage && p.lawfulness <= c.heard_counter_lawful);
    if bold && c.heard_counter_w > 0.0 && hunter_ok(world, hunter) {
        crate::systems::grudges::add(world, target, hunter, GrudgeCause::Assaulted, c.heard_counter_w, 0);
        line(world, &[target], format!("{tn} got word {hn} is looking for {him}{why}, and swore to get {hn} first"));
    } else if c.heard_low_days > 0 {
        let until = (world.tick / TICKS_PER_DAY + 1 + u64::from(c.heard_low_days)) * TICKS_PER_DAY;
        let text =
            format!("{tn} got word {hn} is looking for {him}{why}: {he} lies low at home {} days", c.heard_low_days);
        line(world, &[target], text);
        crate::systems::grief::lie_low(world, target, until, format!("word {hn} is looking for {him}"));
    } else {
        line(world, &[target], format!("{tn} got word {hn} is looking for {him}{why}"));
    }
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
    if !world.hunts.contains_key(&id) && !adopt(world, id, HuntWhy::Goal) {
        return None;
    }
    let s = world.hunts.get(&id)?.clone();
    if !target_ok(world, s.target) {
        return None;
    }
    let mut next = s.clone();
    let mut told = None;
    if next.phase == HuntPhase::Ask {
        if let Some(i) = fresh_intel(world, id, s.target) {
            next.phase = HuntPhase::Watch;
            next.intel = Some(i);
        } else if let Some((o, i)) = asked_own(world, id, s.target) {
            // Round 9 Z1: word along his own people before a Bar's.
            next.phase = HuntPhase::Watch;
            next.intel = Some(i);
            told = Some(o);
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
    if !within_reach(world, id, next.phase, &mut next.venue, next.intel) {
        // Round 9 Z1: past reach, his known haunts in reach (his Home, his
        // post), else the hunt waits for word (kept, not cooled away).
        match haunt_in_reach(world, id, s.target) {
            Some(i) => {
                next.phase = HuntPhase::Watch;
                next.intel = Some(i);
                next.venue = None;
            }
            None => return wait_for_word(world, id, next),
        }
    }
    if let (Some(o), Some(i)) = (told, next.intel) {
        let place = i.building.map_or_else(|| "on the street".to_string(), |b| format!("at {}", world.name_of(b)));
        let text =
            format!("{} asked {} after {}: seen {place}", world.name_of(id), world.name_of(o), world.name_of(s.target));
        line(world, &[id, o], text);
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

/// Round 9 Z1: word along the hunter's own people: the freshest Sighting
/// of `target` (within `fresh_sighting_hours`) held by his kin or a
/// friend: `(who told, intel)`.
fn asked_own(world: &World, hunter: EntityId, target: EntityId) -> Option<(EntityId, Intel)> {
    let horizon = world.tick.saturating_sub(Tick::from(world.config.hunt.fresh_sighting_hours) * TICKS_PER_HOUR);
    let n = world.neighbours.get(&hunter)?;
    n.iter()
        .copied()
        .filter(|&o| o != target && crate::systems::law::living(world, o))
        .filter(|&o| {
            world.edge(hunter, o).is_some_and(|e| {
                matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend)
            })
        })
        .filter_map(|o| sightings_of(world, o, target).filter(|x| x.0 >= horizon).map(|x| (x, o)))
        .max_by(|a, b| a.0 .0.cmp(&b.0 .0).then(b.1.cmp(&a.1)))
        .map(|((_, b, tile), o)| (o, Intel { building: b, tile, source: IntelSource::Asked }))
}

/// Z1: the man's known haunts (his Home, his post) the first within reach.
fn haunt_in_reach(world: &World, hunter: EntityId, target: EntityId) -> Option<Intel> {
    let home = world.comp::<Household>(target).and_then(|h| h.home).and_then(|h| intel_at(world, h, IntelSource::Home));
    let post = world.comp::<Job>(target).and_then(|j| j.employer).and_then(|e| intel_at(world, e, IntelSource::Habit));
    [home, post].into_iter().flatten().find(|&i| {
        let mut v = None;
        within_reach(world, hunter, HuntPhase::Watch, &mut v, Some(i))
    })
}

/// Z1: no word of the man and nowhere near to ask: the hunt is kept, asking
/// again after `wait_hours` (the goal held off that long), with the line
/// once a grudge. `None` (the plan's), `wait_hours` 0 as before.
fn wait_for_word(world: &mut World, id: EntityId, mut next: HuntState) -> Option<Plan> {
    let hours = world.config.hunt.wait_hours;
    if hours == 0 {
        return None;
    }
    let t = next.target;
    next.phase = HuntPhase::Ask;
    next.venue = None;
    next.intel = None;
    next.stakeout_until = None;
    world.hunts.insert(id, next);
    reindex(world);
    let until = world.tick + Tick::from(hours) * TICKS_PER_HOUR;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        let c = b.cooldowns.entry(GoalKind::Hunt).or_insert(until);
        *c = (*c).max(until);
    }
    if !noted(world, id, t, NOTED_WORD) {
        mark(world, id, t, NOTED_WORD);
        let (_, him) = pro(world, t);
        let (he, _) = pro(world, id);
        let text = format!(
            "{} asked after {}{}: nobody near had word of {him}; {he} waits for word",
            world.name_of(id),
            world.name_of(t),
            over(world, id, t)
        );
        line(world, &[id], text);
    }
    None
}

/// The pressure pass phase 4 (OPEN-194): `next`'s first `GoTo(Intel)` is
/// within `[distance.reach] stakeout_ticks` of where `id` sets out. An ask
/// venue past it gives way to the busiest Bar of the hunter's own district
/// when that one is within it (word is asked for nearer home); a stakeout
/// past it, or an ask with no near Bar, is `false`: the leg is not planned
/// and the goal cools (V8: legs of 174-250 min cut by Sleep every evening).
/// `true` off.
pub fn within_reach(
    world: &World,
    id: EntityId,
    phase: HuntPhase,
    venue: &mut Option<EntityId>,
    intel: Option<Intel>,
) -> bool {
    let Some(cap) = crate::systems::distance::reach(world).map(|r| r.stakeout_ticks) else { return true };
    let Some(from) = crate::systems::life::origin(world, id) else { return true };
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let door = |b: EntityId| world.comp::<Building>(b).map(|bd| bd.door);
    let near = |b: Option<EntityId>, tile: Option<crate::components::TilePos>| {
        if b.is_some() && b == here {
            return true;
        }
        b.and_then(door).or(tile).is_none_or(|t| crate::systems::distance::travel(world, from, t) <= cap)
    };
    match phase {
        HuntPhase::Ask => {
            if near(*venue, None) {
                return true;
            }
            match busiest_bar(world, world.district_of(from)).filter(|&b| near(Some(b), None)) {
                Some(b) => {
                    *venue = Some(b);
                    true
                }
                None => false,
            }
        }
        HuntPhase::Watch => intel.is_none_or(|i| near(i.building, Some(i.tile))),
    }
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
        // Pressure pass round 9, Y2 (`OPEN-250`): nor across the cells' wall
        // (the mark taken in while the hunter closed: V17's one blocked hit
        // at 100k reached the swing itself).
        ActionKind::Attack => target.is_some_and(|t| {
            crate::systems::law::living(world, t)
                && crate::systems::law::near(world, id, t, 4)
                && !crate::systems::law::held_apart(world, id, t)
        }),
        _ => crate::exec::actions::can_start(world, id, kind, target),
    }
}

/// W22: the stake-out begins (its timeout noted).
pub fn start_stakeout(world: &mut World, hunter: EntityId, until: Tick) {
    if let Some(mut s) = chase(world, hunter) {
        s.stakeout_until = Some(until);
        set_chase(world, hunter, s);
    }
    // Round 8 X2 (`OPEN-247`): a contract run at its mark's door is worked
    // (`contracts::stalled_runs` reads it).
    let now = world.tick;
    if let Some(r) = world.contract_runs.get_mut(&hunter) {
        r.worked = now;
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
            let lie = friend && {
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
                let success = {
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
        let for_whom = for_whom(world, grudge.as_ref().map(|g| g.cause));
        let text = format!(
            "{} avenged {for_whom} on {}{}",
            world.name_of(hunter),
            world.name_of(target),
            if died { " (killed)" } else { "" }
        );
        world.push_event(EventKind::Avenged, &[hunter, target], text);
        if died {
            if let Some(g) = world.comp_mut::<Grudges>(hunter) {
                if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
                    x.settled = Some(now);
                }
            }
        } else {
            crate::systems::grudges::answer_beating(world, hunter, target);
            answered(world, hunter, target, grudge.as_ref().and_then(|g| killing(g.cause)));
        }
        let w = &mut world.stats.current.word;
        w.avenged += 1;
        w.hunts_ended += 1;
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
        // Round 9 Z1: the end says how.
        if crate::systems::law::living(world, hunter) {
            let (he, _) = pro(world, hunter);
            let text = format!(
                "{} went for {}{} and lost; {he} nurses it, the grudge heavier",
                world.name_of(hunter),
                world.name_of(target),
                over(world, hunter, target)
            );
            ended(world, hunter, text);
        }
    }
}

/// Round 9 Z1: `by` beat `target` over `dead`: the others who hold a
/// grudge on him over the same dead take it as answered (`answer_beating`);
/// a Hunt of theirs on him whose grudge falls under its weight ends with
/// the line. A killing settles them all already (`grudges::on_death`).
fn answered(world: &mut World, by: EntityId, target: EntityId, dead: Option<EntityId>) {
    let Some(dead) = dead else { return };
    // scan-ok: per Avenged beating, the grudge store.
    let others: Vec<EntityId> = world
        .with::<Grudges>()
        .into_iter()
        .filter(|&o| o != by)
        .filter(|&o| {
            world.comp::<Grudges>(o).is_some_and(|g| {
                g.list.iter().any(|x| x.target == target && x.settled.is_none() && killing(x.cause) == Some(dead))
            })
        })
        .collect();
    for o in others {
        crate::systems::grudges::answer_beating(world, o, target);
        let hunting = world.hunts.get(&o).is_some_and(|s| s.target == target);
        let w = crate::systems::grudges::grudge_on(world, o, target);
        if hunting && w.is_none_or(|g| g.settled.is_some() || g.weight < min_for(world, g.cause)) {
            end(world, o);
            reindex(world);
            let text = format!(
                "{} heard {} beat {} over {}: answered, {} let it go",
                world.name_of(o),
                world.name_of(by),
                world.name_of(target),
                for_whom(world, Some(GrudgeCause::KilledFriend(dead))),
                pro(world, o).0
            );
            ended(world, o, text);
        }
    }
}

/// Round 9 Z1: why a Hunt ends without a strike, in words (`None`: the
/// hunter is dead, nothing to say). A man held first is the wait's (the
/// grudge marked `NOTED_HELD`, `hunts_waiting`).
fn end_words(world: &mut World, h: EntityId, t: EntityId) -> Option<String> {
    if !crate::systems::law::living(world, h) {
        return None;
    }
    let (hn, tn) = (world.name_of(h), world.name_of(t));
    let why = over(world, h, t);
    let (_, him) = pro(world, t);
    let (he, _) = pro(world, h);
    if !hunter_ok(world, h) {
        return Some(if held(world, h) {
            format!("{hn}'s hunt for {tn}{why} ended in {}", held_where(world, h))
        } else {
            format!("{hn} gave up on {tn}{why}: {he} is leaving")
        });
    }
    if !crate::systems::law::living(world, t) {
        return Some(format!("{tn} was dead before {hn} found {him}{why}"));
    }
    if held(world, t) {
        mark(world, h, t, NOTED_HELD);
        world.stats.current.word.hunts_waiting += 1;
        return Some(format!("the law took {tn} first ({}); {hn} will wait for {him}{why}", held_where(world, t)));
    }
    Some(format!("{tn} left the city before {hn} found {him}{why}"))
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

/// Round 9 Z1, daily over the grudge store: a grudge over a killing heavy
/// enough to hunt whose killer is held (in the cells), not yet told, is
/// told once: handed inside to a cellmate of the holder's own when one is
/// there (`hand_inside`), else the wait's line (`hunts_waiting`). A told
/// grudge whose man is out again is told that (the holder's Hunt free at
/// once, `NOTED_HELD` cleared). A holder under 18 never: `hunter_ok`.
fn held_pass(world: &mut World) {
    if world.config.hunt.killed_hunt_min <= 0.0 {
        return;
    }
    #[derive(PartialEq)]
    enum Turn {
        Held,
        Out,
        Left,
    }
    let mut todo: Vec<(EntityId, EntityId, Turn)> = Vec::new();
    // scan-ok: daily, the grudge store.
    for id in world.with::<Grudges>() {
        let Some(g) = world.comp::<Grudges>(id) else { continue };
        for x in g.list.iter().filter(|x| x.settled.is_none() && killing(x.cause).is_some()) {
            if x.noted & NOTED_HELD != 0 {
                if !held(world, x.target) && crate::systems::law::living(world, x.target) {
                    todo.push((id, x.target, Turn::Out));
                }
            } else if x.weight >= min_for(world, x.cause) && world.has::<Sentence>(x.target) {
                todo.push((id, x.target, Turn::Held));
            } else if x.weight >= min_for(world, x.cause)
                && x.noted & NOTED_LEFT == 0
                && !world.hunts.contains_key(&id)
                && crowded(world, id, x.target)
            {
                todo.push((id, x.target, Turn::Left));
            }
        }
    }
    for (id, t, turn) in todo {
        if !hunter_ok(world, id) || !crate::systems::law::living(world, t) {
            continue;
        }
        let why = over(world, id, t);
        let (hn, tn) = (world.name_of(id), world.name_of(t));
        if turn == Turn::Left {
            // Z1: the man has hunters enough (`hunters_per_target`).
            mark(world, id, t, NOTED_LEFT);
            let names: Vec<String> =
                world.hunts.iter().filter(|(_, s)| s.target == t).map(|(&h, _)| world.name_of(h)).take(2).collect();
            let (_, him) = pro(world, t);
            let text = format!("{hn} leaves {tn}{why} to {}, already after {him}", names.join(" and "));
            line(world, &[id], text);
            world.stats.current.word.hunts_handed += 1;
            continue;
        }
        if turn == Turn::Held {
            mark(world, id, t, NOTED_HELD);
            if hand_inside(world, id, t) {
                continue;
            }
            world.stats.current.word.hunts_waiting += 1;
            line(world, &[id], format!("{hn}'s grudge{why} waits: {tn} is in {}", held_where(world, t)));
        } else {
            if let Some(g) = world.comp_mut::<Grudges>(id) {
                if let Some(x) = g.list.iter_mut().find(|x| x.target == t && x.settled.is_none()) {
                    x.noted &= !NOTED_HELD;
                }
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.cooldowns.remove(&GoalKind::Hunt);
            }
            line(world, &[id], format!("{hn} heard {tn} is out of the cells{why}"));
        }
    }
}

/// Z1: a held killer's cellmate of the holder's own (kin, a friend, a gang
/// mate; an adult held in the same cells, never one under 18) takes the
/// grudge inside: an `Inherited` grudge at `hand_w`, the line; the yard
/// does the rest (`inside_mark`). Bounded by the holder's edges and gang.
fn hand_inside(world: &mut World, id: EntityId, t: EntityId) -> bool {
    let c = world.config.hunt.clone();
    if c.inside_fight_p <= 0.0 || c.hand_w <= 0.0 {
        return false;
    }
    let Some(jail) = crate::systems::law::cells_of(world, t) else { return false };
    let Some(g) = crate::systems::grudges::grudge_on(world, id, t) else { return false };
    let w = (g.weight * c.hand_w).min(1.0);
    if w < min_for(world, GrudgeCause::Inherited(id)) {
        return false;
    }
    let mut cands: Vec<EntityId> = world.neighbours.get(&id).map(|n| n.iter().copied().collect()).unwrap_or_default();
    cands.retain(|&o| {
        world
            .edge(id, o)
            .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend))
    });
    if let Some(gang) = world.gang_of(id).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        cands.extend(gang.members.iter().copied());
    }
    let Some((_, o)) = cands
        .into_iter()
        .filter(|&o| o != id && o != t && crate::systems::law::living(world, o))
        .filter(|&o| crate::systems::law::cells_of(world, o) == Some(jail))
        .filter(|&o| crate::systems::demography::is_adult(world, o) && !crate::systems::ages::is_protected(world, o))
        .filter(|&o| takes_against(world, o, t))
        .map(|o| (might_gap(world, o, t), o))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
    else {
        return false;
    };
    let text = format!(
        "{} put word inside {}: {}, held there with {}, will see to it{}",
        world.name_of(id),
        world.name_of(jail),
        world.name_of(o),
        world.name_of(t),
        over(world, id, t)
    );
    line(world, &[id, o, jail], text);
    let chain = g.chain;
    crate::systems::grudges::add(world, o, t, GrudgeCause::Inherited(id), w, chain);
    world.stats.current.word.hunts_handed += 1;
    true
}

/// Z1, from `consequence::cell_day`: the cellmate `p` goes for (a held
/// man he holds a grudge over a killing on, or one handed him, heavy
/// enough to hunt; never one under 18), on `inside_fight_p` a day (`roll`).
pub fn inside_mark(world: &World, p: EntityId, mates: &[EntityId], roll: f32) -> Option<EntityId> {
    let c = world.config.hunt.inside_fight_p;
    if c <= 0.0 || roll >= c || crate::systems::ages::is_protected(world, p) || cooled(world, p) {
        return None;
    }
    world
        .comp::<Grudges>(p)?
        .list
        .iter()
        .filter(|x| x.settled.is_none() && blood(x.cause) && x.weight >= min_for(world, x.cause))
        .filter(|x| mates.contains(&x.target) && !crate::systems::ages::is_protected(world, x.target))
        .max_by(|a, b| a.weight.total_cmp(&b.weight).then(b.target.cmp(&a.target)))
        .map(|x| x.target)
}

/// Z1, from `consequence::cell_day` after a yard fight `p` started on `m`:
/// a win on a grudge over a killing (or one handed him) is the hunt's end
/// inside: the `Avenged` deed and line, the grudge settled (a death) or
/// answered (a beating), the chain.
pub fn cell_strike(world: &mut World, p: EntityId, m: EntityId, winner: EntityId, died: bool, jail: EntityId) {
    // A killing settled the grudge already (`grudges::on_death`).
    let Some(g) = world.comp::<Grudges>(p).and_then(|gs| {
        gs.list.iter().rev().find(|x| x.target == m && blood(x.cause) && (died || x.settled.is_none())).cloned()
    }) else {
        return;
    };
    if winner != p {
        // A loss holds him off as a lost Hunt does (no fight a day on it).
        cool(world, p);
        return;
    }
    let now = world.tick;
    world.kill_chain.insert(m, (g.chain.saturating_add(1), now));
    let d = world.district_of_building(jail);
    crate::systems::gossip::post_deed(world, d, Deed::Avenged, Some(p), Some(m));
    let text = format!(
        "{} avenged {} on {} in {}{}",
        world.name_of(p),
        for_whom(world, Some(g.cause)),
        world.name_of(m),
        world.name_of(jail),
        if died { " (killed)" } else { "" }
    );
    world.push_event(EventKind::Avenged, &[p, m, jail], text);
    if died {
        if let Some(gs) = world.comp_mut::<Grudges>(p) {
            if let Some(x) = gs.list.iter_mut().find(|x| x.target == m && x.settled.is_none()) {
                x.settled = Some(now);
            }
        }
    } else {
        crate::systems::grudges::answer_beating(world, p, m);
        answered(world, p, m, killing(g.cause));
    }
    let w = &mut world.stats.current.word;
    w.avenged += 1;
    w.hunts_ended += 1;
    if died {
        w.revenge_kills += 1;
    }
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
