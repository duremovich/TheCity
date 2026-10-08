//! M15 § 3 (plan phase 3, W15-W18, W35): grudges, their inheritance and
//! settlement, the factions' vendettas, and Guard the body.
//!
//! A grudge is a game record on an agent: who wronged it (or someone close
//! to it), how much it weighs (`0..=1`) and how deep in a chain of answered
//! wrongs it sits. It forms when an adult learns a deed (first-hand or as a
//! rumour) whose object is the adult itself or someone it has a close edge
//! with; it decays daily, settles when its target dies, passes to Spouse and
//! adult children when its holder dies, and feeds the Fight goal, the Hunt
//! (`systems::hunt`) and the faction pairs' vendettas (which the gang brain's
//! Retaliate and the corp brain's Lobby read). Everything here runs on
//! state and counters: no draw on any stream.

use smallvec::SmallVec;

use crate::components::{
    Brain, Corp, Corpse, Gang, Identity, Law, Lod, Memory, MemoryKind, Personality, Position, RelKind,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::word::{Deed, DeedRef, Grudge, GrudgeCause, Grudges, Vendetta};
use crate::world::World;

/// A holder keeps at most this many grudges (spec § 3).
pub const GRUDGE_CAP: usize = 4;
/// `kill_chain` entries are kept this long (spec § 3).
pub const CHAIN_DAYS: u64 = 60;

/// Grudges, vendettas and Guard the body run with the word (`[gossip]
/// enabled`).
pub fn on(world: &World) -> bool {
    world.config.gossip.enabled
}

/// How a holder stands to a deed's object (spec `rel_w`, plan W15).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Rel {
    /// The holder is the object.
    Own,
    /// Spouse, Parent or Family.
    Kin,
    Friend,
    /// The holder's own gang leader (plan row `leader`).
    Leader,
    /// A member of the holder's gang (plan row `comrade`).
    Comrade,
}

/// `rel_w` of `holder` toward a deed's `object`, `None` when nothing ties
/// them (or the weight is 0).
pub fn relation(world: &World, holder: EntityId, object: Option<EntityId>) -> Option<(f32, Rel)> {
    let o = object?;
    let r = &world.config.grudges.rel_w;
    if o == holder {
        return (r.own > 0.0).then_some((r.own, Rel::Own));
    }
    if let Some(e) = world.edge(holder, o) {
        let hit = match e.kind {
            RelKind::Spouse => Some((r.spouse, Rel::Kin)),
            RelKind::Parent => Some((r.parent, Rel::Kin)),
            RelKind::Family => Some((r.family, Rel::Kin)),
            RelKind::Friend => Some((r.friend * (0.5 + 0.5 * e.affinity.clamp(-1.0, 1.0)), Rel::Friend)),
            _ => None,
        };
        if let Some((w, rel)) = hit.filter(|&(w, _)| w > 0.0) {
            return Some((w, rel));
        }
    }
    let gang = world.gang_of(holder)?;
    if world.comp::<Gang>(gang).and_then(|g| g.leader) == Some(o) {
        return (r.leader > 0.0).then_some((r.leader, Rel::Leader));
    }
    if world.gang_of(o) == Some(gang) && r.comrade > 0.0 {
        return Some((r.comrade, Rel::Comrade));
    }
    None
}

/// The cause a deed leaves on a holder standing to its object as `rel`.
fn cause_of(deed: Deed, rel: Rel, object: Option<EntityId>) -> GrudgeCause {
    let o = object.unwrap_or(EntityId::NONE);
    match deed {
        Deed::Killed | Deed::Avenged => match rel {
            Rel::Kin => GrudgeCause::KilledKin(o),
            Rel::Friend | Rel::Leader | Rel::Comrade => GrudgeCause::KilledFriend(o),
            Rel::Own => GrudgeCause::Assaulted,
        },
        Deed::Assaulted | Deed::Raided => GrudgeCause::Assaulted,
        Deed::Robbed | Deed::Extorted => GrudgeCause::Robbed,
        Deed::Stripped => GrudgeCause::Stripped(o),
        Deed::Evicted => GrudgeCause::Evicted,
        Deed::Betrayed | Deed::Poached => GrudgeCause::Betrayed,
        Deed::Arrested | Deed::Married | Deed::Struck | Deed::Founded | Deed::Repaid => GrudgeCause::Assaulted,
    }
}

/// The chain a grudge over `object` starts at: the `kill_chain` of a Hunt's
/// victim (W15), else 0.
pub fn chain_of(world: &World, object: Option<EntityId>) -> u8 {
    object.and_then(|o| world.kill_chain.get(&o)).map_or(0, |&(c, _)| c)
}

/// Does `holder` hold an unsettled grudge on `target` of at least `min`?
/// Stale-safe: ids compare with their generation.
pub fn holds(world: &World, holder: EntityId, target: EntityId, min: f32) -> bool {
    world
        .comp::<Grudges>(holder)
        .is_some_and(|g| g.list.iter().any(|x| x.target == target && x.settled.is_none() && x.weight >= min))
}

/// The unsettled grudge of `holder` on `target`, if any.
pub fn grudge_on(world: &World, holder: EntityId, target: EntityId) -> Option<Grudge> {
    world.comp::<Grudges>(holder)?.list.iter().find(|x| x.target == target && x.settled.is_none()).cloned()
}

/// A display name for an agent or a faction.
pub fn label(world: &World, id: EntityId) -> String {
    if let Some(g) = world.comp::<Gang>(id) {
        return g.name.clone();
    }
    if let Some(c) = world.comp::<Corp>(id) {
        return c.name.clone();
    }
    if world.has::<Law>(id) {
        return "the Law".to_string();
    }
    world.name_of(id)
}

fn cause_label(c: GrudgeCause) -> &'static str {
    match c {
        GrudgeCause::KilledKin(_) => "killed kin",
        GrudgeCause::KilledFriend(_) => "killed a friend",
        GrudgeCause::Assaulted => "a beating",
        GrudgeCause::Robbed => "a robbery",
        GrudgeCause::Stripped(_) => "a stripped body",
        GrudgeCause::Evicted => "an eviction",
        GrudgeCause::Betrayed => "a betrayal",
        GrudgeCause::Inherited(_) => "inherited",
    }
}

/// W15: `holder` learned the deed `r` (first-hand or heard, at `conf`):
/// `w = deed_sev × rel_w × conf`; at `w ≥ grudge_min` a grudge on the actor
/// (a gang or corp entity when the actor is one) is merged or pushed. The
/// hops are not read (a rumour's doubt is its conf). Called after a deed
/// memory goes in through the world (`World::remember`, `remember_crime`,
/// `memory::hear_entry`, the binder's rename), never on a merge.
pub fn on_learn(world: &mut World, holder: EntityId, r: &DeedRef, conf: f32, hops: u8) {
    let _ = hops;
    if !on(world) {
        return;
    }
    let Some(actor) = r.actor else { return };
    if actor == holder || !world.is_alive(actor) {
        return;
    }
    let sev = world.config.gossip.deed_sev.get(r.deed);
    if sev <= 0.0 {
        return;
    }
    if !crate::systems::law::living(world, holder) || !crate::systems::demography::is_adult(world, holder) {
        return;
    }
    // A member holds nothing against its own gang (a distorted rumour can
    // name it).
    if world.gang_of(holder) == Some(actor) {
        return;
    }
    let Some((rel_w, rel)) = relation(world, holder, r.object) else { return };
    let w = sev * rel_w * conf.clamp(0.0, 1.0);
    if w < world.config.grudges.grudge_min {
        return;
    }
    let chain = chain_of(world, r.object);
    add(world, holder, actor, cause_of(r.deed, rel, r.object), w, chain);
}

/// Merge `w` into `holder`'s unsettled grudge on `target`
/// (`1 − (1 − old)(1 − w)`, the deeper chain kept), else push one; past
/// the cap a settled entry goes first (the oldest), else the lightest
/// unsettled one if lighter than `w`, else the new one is dropped. An agent
/// target becomes an Enemy (`make_enemy`, −0.4); `GrudgeFormed` at `w ≥
/// 0.5`. Returns whether anything changed.
pub fn add(world: &mut World, holder: EntityId, target: EntityId, cause: GrudgeCause, w: f32, chain: u8) -> bool {
    if holder == target || !world.has::<Brain>(holder) {
        return false;
    }
    let now = world.tick;
    let w = w.clamp(0.0, 1.0);
    if !world.has::<Grudges>(holder) {
        world.insert(holder, Grudges::default());
    }
    let Some(g) = world.comp_mut::<Grudges>(holder) else { return false };
    let mut fresh = false;
    if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
        x.weight = 1.0 - (1.0 - x.weight) * (1.0 - w);
        x.chain = x.chain.max(chain);
    } else {
        if g.list.len() >= GRUDGE_CAP {
            let settled =
                g.list.iter().enumerate().filter_map(|(i, x)| x.settled.map(|t| (t, i))).min().map(|(_, i)| i);
            let out = settled.or_else(|| {
                g.list
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.weight.total_cmp(&b.1.weight).then(a.0.cmp(&b.0)))
                    .filter(|(_, x)| x.weight < w)
                    .map(|(i, _)| i)
            });
            match out {
                Some(i) => {
                    g.list.remove(i);
                }
                None => return false,
            }
        }
        g.list.push(Grudge { target, cause, weight: w, since: now, chain, settled: None });
        fresh = true;
    }
    if world.has::<Identity>(target) && crate::systems::law::living(world, target) {
        crate::systems::social::make_enemy(world, holder, target, -0.4);
    }
    if fresh {
        world.stats.current.word.grudges += 1;
    }
    if w >= 0.5 {
        let text = format!(
            "{} swore against {} ({}, {w:.2})",
            world.name_of(holder),
            label(world, target),
            cause_label(cause)
        );
        world.push_event(EventKind::GrudgeFormed, &[holder, target], text);
    }
    true
}

/// W16, from `World::kill_by` before the dead's components go: every
/// grudge on the dead settles (by anyone's hand), the dead's own unsettled
/// grudges of `weight ≥ inherit_min` pass to its Spouse and adult children
/// at `× inherit_frac` (`Inherited`, chain kept), a vendetta between the
/// killer's and the dead's factions counts the kill, and the Hunts the dead
/// was part of end.
pub fn on_death(world: &mut World, dead: EntityId, killer: Option<EntityId>) {
    if !on(world) {
        return;
    }
    let now = world.tick;
    // scan-ok: per death, the grudge store (no per-tick work).
    for h in world.with::<Grudges>() {
        if let Some(g) = world.comp_mut::<Grudges>(h) {
            for x in g.list.iter_mut().filter(|x| x.target == dead && x.settled.is_none()) {
                x.settled = Some(now);
            }
        }
    }
    let cfg = world.config.grudges.clone();
    if let Some(own) = world.comp::<Grudges>(dead).cloned() {
        let mut heirs: Vec<EntityId> = world.spouse_of(dead).into_iter().collect();
        for c in crate::systems::demography::children_of_agent(world, dead) {
            if crate::systems::demography::is_adult(world, c) && !heirs.contains(&c) {
                heirs.push(c);
            }
        }
        heirs.retain(|&h| crate::systems::law::living(world, h));
        heirs.sort_unstable();
        for x in own.list.iter().filter(|x| x.settled.is_none() && x.weight >= cfg.inherit_min) {
            for &h in &heirs {
                if h == x.target {
                    continue;
                }
                if add(world, h, x.target, GrudgeCause::Inherited(dead), x.weight * cfg.inherit_frac, x.chain) {
                    world.stats.current.word.grudges_inherited += 1;
                }
            }
        }
        world.remove::<Grudges>(dead);
    }
    if let Some(k) = killer.filter(|&k| k != dead) {
        let (kf, df) = (factions_of(world, k), factions_of(world, dead));
        for v in world.vendettas.iter_mut() {
            if kf.contains(&v.a) && df.contains(&v.b) {
                v.kills[0] = v.kills[0].saturating_add(1);
            } else if kf.contains(&v.b) && df.contains(&v.a) {
                v.kills[1] = v.kills[1].saturating_add(1);
            }
        }
    }
    crate::systems::hunt::on_death(world, dead);
}

/// Is `id` the faction `f` or one of its people (a gang's member, a corp's
/// exec or staff, a city guard or the captain for the Law)? O(1), no
/// allocation (unlike `corp_of_agent`, a daily query): safe on the
/// co-location path (`gossip::wants_sighting`).
pub fn member_of(world: &World, id: EntityId, f: EntityId) -> bool {
    if id == f {
        return true;
    }
    if world.has::<Gang>(f) {
        return world.gang_of(id) == Some(f);
    }
    if let Some(c) = world.comp::<Corp>(f) {
        return c.exec == Some(id)
            || world
                .comp::<crate::components::Job>(id)
                .and_then(|j| j.employer)
                .and_then(|e| world.corp_of_building(e))
                == Some(f);
    }
    if world.has::<Law>(f) {
        return crate::systems::law::is_city_guard(world, id) || world.law().is_some_and(|l| l.captain == Some(id));
    }
    false
}

/// The factions an agent belongs to: its gang, its corp (exec or
/// employee) and the Law (a city guard).
pub fn factions_of(world: &World, id: EntityId) -> SmallVec<[EntityId; 3]> {
    let mut out = SmallVec::new();
    if world.has::<Gang>(id) || world.has::<Corp>(id) || world.has::<Law>(id) {
        out.push(id);
        return out;
    }
    if let Some(g) = world.gang_of(id) {
        out.push(g);
    }
    if let Some(c) = world.corp_of_agent(id) {
        out.push(c);
    }
    if crate::systems::law::is_city_guard(world, id) {
        if let Some(j) = world.building_of_kind(crate::components::BuildingKind::Jail).filter(|&j| world.has::<Law>(j))
        {
            out.push(j);
        }
    }
    out
}

/// W16/W18 at midnight (the word's chain, after competence): grudges
/// decay (`grudge_decay`, half for KilledKin and KilledFriend) and go at 0;
/// a grudge whose target is gone (emigrated) settles; settled entries go
/// after `settle_keep_days`; `kill_chain` drops entries past 60 days; then
/// the vendettas.
pub fn daily(world: &mut World) {
    prune_guards(world);
    if !on(world) {
        return;
    }
    let now = world.tick;
    let cfg = world.config.grudges.clone();
    let keep = Tick::from(cfg.settle_keep_days) * TICKS_PER_DAY;
    // scan-ok: daily, the grudge store.
    for h in world.with::<Grudges>() {
        let gone: SmallVec<[EntityId; 4]> = world
            .comp::<Grudges>(h)
            .map(|g| {
                g.list.iter().filter(|x| x.settled.is_none() && !world.is_alive(x.target)).map(|x| x.target).collect()
            })
            .unwrap_or_default();
        let Some(g) = world.comp_mut::<Grudges>(h) else { continue };
        for x in g.list.iter_mut() {
            if x.settled.is_some() {
                continue;
            }
            if gone.contains(&x.target) {
                x.settled = Some(now);
                continue;
            }
            let d = match x.cause {
                GrudgeCause::KilledKin(_) | GrudgeCause::KilledFriend(_) => cfg.grudge_decay * 0.5,
                _ => cfg.grudge_decay,
            };
            x.weight -= d;
        }
        g.list.retain(|x| match x.settled {
            Some(t) => now.saturating_sub(t) < keep,
            None => x.weight > 0.0,
        });
        if g.list.is_empty() {
            world.remove::<Grudges>(h);
        }
    }
    let horizon = now.saturating_sub(CHAIN_DAYS * TICKS_PER_DAY);
    world.kill_chain.retain(|_, &mut (_, t)| t >= horizon);
    vendettas(world);
}

/// W18: `V(A, B)` for every ordered pair of live factions, the unsettled
/// grudge weight A's members hold on B's members or on B, ÷
/// `vendetta_norm`, clamped to 1. A pair whose two directions sum to
/// `vendetta_open` opens a vendetta (`Vendetta`); an open one below
/// `vendetta_close` ends (`VendettaEnded`). O(grudges).
pub fn vendettas(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.grudges.clone();
    let live = crate::systems::reputation::factions(world);
    let mut v: std::collections::BTreeMap<(EntityId, EntityId), f32> = std::collections::BTreeMap::new();
    // scan-ok: daily, the grudge store.
    for h in world.with::<Grudges>() {
        let hf = factions_of(world, h);
        if hf.is_empty() {
            continue;
        }
        let Some(g) = world.comp::<Grudges>(h) else { continue };
        for x in g.list.iter().filter(|x| x.settled.is_none() && x.weight > 0.0) {
            if !world.is_alive(x.target) {
                continue;
            }
            let tf = factions_of(world, x.target);
            for &a in &hf {
                for &b in &tf {
                    if a != b && live.contains(&a) && live.contains(&b) {
                        *v.entry((a, b)).or_insert(0.0) += x.weight;
                    }
                }
            }
        }
    }
    let norm = cfg.vendetta_norm.max(1e-6);
    let get = |a: EntityId, b: EntityId| (v.get(&(a, b)).copied().unwrap_or(0.0) / norm).clamp(0.0, 1.0);
    let now = world.tick;
    // The open ones: update, or end.
    let mut ended = Vec::new();
    for (i, vd) in world.vendettas.iter_mut().enumerate() {
        vd.w = [get(vd.a, vd.b), get(vd.b, vd.a)];
        let gone = !live.contains(&vd.a) || !live.contains(&vd.b);
        if gone || vd.w[0] + vd.w[1] < cfg.vendetta_close {
            ended.push(i);
        }
    }
    for &i in ended.iter().rev() {
        let vd = world.vendettas.remove(i);
        let text = format!("the feud between {} and {} cooled", label(world, vd.a), label(world, vd.b));
        world.push_event(EventKind::VendettaEnded, &[vd.a, vd.b], text);
    }
    // New ones, by pair (the lower id first).
    let mut opened = Vec::new();
    for &(a, b) in v.keys() {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        if (lo, hi) != (a, b) && v.contains_key(&(lo, hi)) {
            continue; // the pair is visited from its lower side
        }
        let sum = get(lo, hi) + get(hi, lo);
        let open = world.vendettas.iter().any(|x| (x.a, x.b) == (lo, hi) || (x.a, x.b) == (hi, lo));
        if !open && sum >= cfg.vendetta_open && !opened.iter().any(|x: &Vendetta| (x.a, x.b) == (lo, hi)) {
            opened.push(Vendetta { a: lo, b: hi, since: now, kills: [0, 0], w: [get(lo, hi), get(hi, lo)] });
        }
    }
    for vd in opened {
        let text = format!("blood between {} and {}", label(world, vd.a), label(world, vd.b));
        world.push_event(EventKind::Vendetta, &[vd.a, vd.b], text);
        world.vendettas.push(vd);
    }
}

/// W18: `gang`'s open vendetta of the highest `V(gang, ·)` against a gang
/// or a corp (the Law is never a Retaliate target), ties the lower id.
pub fn vendetta_for(world: &World, gang: EntityId) -> Option<(EntityId, f32)> {
    world
        .vendettas
        .iter()
        .filter_map(|v| {
            if v.a == gang {
                Some((v.b, v.w[0]))
            } else if v.b == gang {
                Some((v.a, v.w[1]))
            } else {
                None
            }
        })
        .filter(|&(o, _)| world.has::<Gang>(o) || world.has::<Corp>(o))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
}

/// W18 (Lobby): the gang with the highest vendetta weight against `corp`
/// (ties the lower id), if any vendetta involves the corp.
pub fn vendetta_culprit(world: &World, corp: EntityId) -> Option<EntityId> {
    world
        .vendettas
        .iter()
        .filter_map(|v| {
            if v.a == corp {
                Some((v.b, v.w[1]))
            } else if v.b == corp {
                Some((v.a, v.w[0]))
            } else {
                None
            }
        })
        .filter(|&(o, _)| world.has::<Gang>(o))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
        .map(|(g, _)| g)
}

// ---------------------------------------------------------------------------
// Guard the body (W35)
// ---------------------------------------------------------------------------

/// Is this body still worth standing over: unburied, unstripped, its loot
/// window open?
fn guardable(world: &World, c: EntityId) -> bool {
    world.comp::<Corpse>(c).is_some_and(|k| !k.buried && !k.stripped && !k.settled)
}

/// W35: the body a Full or Coarse adult would guard: a Spouse's, kin's or
/// Friend's (the agent's `SawCorpse` and `Grief` memories), within 12
/// tiles, guardable; the closest tie first, then the lower id. With its
/// `rel_w`.
pub fn guard_target(world: &World, id: EntityId) -> Option<(EntityId, f32)> {
    let m = world.comp::<Memory>(id)?;
    let here = world.comp::<Position>(id)?.tile;
    let mut best: Option<(f32, EntityId)> = None;
    for e in m.entries.iter().filter(|e| matches!(e.kind, MemoryKind::SawCorpse | MemoryKind::Grief)) {
        let Some(c) = e.subject else { continue };
        if !guardable(world, c) {
            continue;
        }
        let Some((w, rel)) = relation(world, id, Some(c)) else { continue };
        if !matches!(rel, Rel::Kin | Rel::Friend) {
            continue;
        }
        let near = world.comp::<Position>(c).is_some_and(|p| crate::systems::law::chebyshev(p.tile, here) <= 12);
        if !near {
            continue;
        }
        if best.is_none_or(|(bw, bc)| w > bw || (w == bw && c < bc)) {
            best = Some((w, c));
        }
    }
    best.map(|(w, c)| (c, w))
}

/// W35's considerations (spec § 3): the gate, `rel_w` Linear{0.8, 0.2},
/// `courage` Linear{0.5, 0.5}. `None` off, at Statistical, or with no body.
pub fn guard_body_considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) || !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
        return None;
    }
    let (_, w) = guard_target(world, id)?;
    let courage = world.comp::<Personality>(id)?.courage;
    Some((
        vec![
            Consideration::new("kin body unguarded", can(true), GATE),
            Consideration::new("rel_w", w, Curve::Linear { m: 0.8, b: 0.2 }),
            Consideration::new("courage", courage, Curve::Linear { m: 0.5, b: 0.5 }),
        ],
        0.0,
    ))
}

/// W35: `GoTo(CorpseTile) → StakeOut` over the body for `guard_hours`.
pub fn guard_body_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    let (c, _) = guard_target(world, id)?;
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::CorpseTile), Some(c)), step(ActionKind::StakeOut, Some(c))];
    Some(Plan { goal: crate::components::GoalKind::GuardBody, target: Some(c), steps, started_tick: world.tick })
}

/// Is `guard` standing over `corpse` right now (its GuardBody wait runs)?
pub fn guarding(world: &World, guard: EntityId, corpse: EntityId) -> bool {
    let Some(b) = world.comp::<Brain>(guard) else { return false };
    matches!(b.exec, ExecState::Use { kind: ActionKind::StakeOut, .. })
        && b.plan.as_ref().is_some_and(|p| p.goal == crate::components::GoalKind::GuardBody && p.target == Some(corpse))
        && crate::systems::law::living(world, guard)
}

/// W35: the wait begins: the guard is registered over the body.
pub fn start_guard(world: &mut World, guard: EntityId, corpse: EntityId) {
    let v = world.guards_of_corpse.entry(corpse).or_default();
    if !v.contains(&guard) {
        v.push(guard);
        v.sort_unstable();
    }
    world.stats.current.word.guard_body += 1;
}

/// W35: the wait ended (done or abandoned).
pub fn end_guard(world: &mut World, guard: EntityId, corpse: EntityId) {
    if let Some(v) = world.guards_of_corpse.get_mut(&corpse) {
        v.retain(|g| *g != guard);
        if v.is_empty() {
            world.guards_of_corpse.remove(&corpse);
        }
    }
}

/// W35, at a Strip or Rip's completion: the first guard standing over the
/// body and adjacent to the stripper fights it (`resolve_fight(guard,
/// stripper)`); true when the stripper may go on (no guard, or it won).
pub fn guard_contests(world: &mut World, stripper: EntityId, corpse: EntityId) -> bool {
    if !on(world) {
        return true;
    }
    let guards: SmallVec<[EntityId; 2]> = world.guards_of_corpse.get(&corpse).cloned().unwrap_or_default();
    let Some(g) = guards
        .into_iter()
        .find(|&g| g != stripper && guarding(world, g, corpse) && crate::systems::law::near(world, g, stripper, 1))
    else {
        return true;
    };
    let (winner, _, _) = crate::systems::law::resolve_fight(world, g, stripper);
    winner == stripper
}

/// Review fix, daily: drop the guards whose wait ended without `end_guard`
/// (killed mid-wait: `kill_by` runs no `on_abort`) and the bodies nobody
/// stands over any more (buried or gone), so the map does not grow for the
/// run's length. `guard_contests` reads it through `guarding` either way.
fn prune_guards(world: &mut World) {
    if world.guards_of_corpse.is_empty() {
        return;
    }
    let mut map = std::mem::take(&mut world.guards_of_corpse);
    map.retain(|&c, v| {
        v.retain(|g| guarding(world, *g, c));
        !v.is_empty()
    });
    world.guards_of_corpse = map;
}

/// After a load: the guards whose GuardBody wait is running, by body.
pub fn rebuild_guards(world: &mut World) {
    world.guards_of_corpse.clear();
    for id in world.bodies() {
        let Some(c) = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).and_then(|p| p.target) else { continue };
        if guarding(world, id, c) {
            world.guards_of_corpse.entry(c).or_default().push(id);
        }
    }
}
