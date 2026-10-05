//! M10 holes and the binder (`docs/M10_SCALE.md` § 3).
//!
//! A Statistical agent's hourly table samples what is done *to* it (robbed,
//! beaten, killed) without drawing who did it: the crime is a `Hole` until
//! something needs the answer. The daily pass binds consequential holes (a
//! killing, or a gang member or guard as victim) by the next morning and any
//! hole past `[bind] hole_ttl_days`; a promoted victim's holes bind at once
//! (`lod::set_lod` queues them); later the inspector binds on demand.
//!
//! A binding is a pure function of `(world seed, hole id)` and the candidates'
//! traces: the hole's own RNG stream (`SimRng::hole`) is never stored, so call
//! order, save/load and repetition draw the same numbers. The draws, in order:
//! the Unknown roll, the candidate, the witness roll, the witness.

use rand::Rng;

use crate::components::{
    trace_flags, Bound, BuildingKind, DayTrace, Hole, HoleId, HoleKind, LawShock, Lod, MemoryKind, Personality, Shock,
    Trace, Wallet, Zone,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{self, TICKS_PER_DAY};
use crate::world::World;

/// Insert a hole, index it under the victim, enforce the per-victim cap
/// (oldest goes Unknown without a draw), count it.
pub fn open_hole(world: &mut World, hole: Hole) -> HoleId {
    let (id, victim) = (hole.id, hole.victim);
    world.holes.insert(id, hole);
    let list = world.holes_by_agent.entry(victim).or_default();
    if let Err(i) = list.binary_search(&id) {
        list.insert(i, id);
    }
    world.stats.current.holes_opened += 1;
    let cap = world.config.bind.max_open_per_agent.max(1);
    while let Some(oldest) =
        world.holes_by_agent.get(&victim).filter(|l| l.len() > cap).and_then(|l| l.first().copied())
    {
        expire(world, oldest);
    }
    id
}

/// Take a hole out of both indices.
fn take(world: &mut World, id: HoleId) -> Option<Hole> {
    let hole = world.holes.remove(&id)?;
    if let Some(list) = world.holes_by_agent.get_mut(&hole.victim) {
        list.retain(|h| *h != id);
        if list.is_empty() {
            world.holes_by_agent.remove(&hole.victim);
        }
    }
    Some(hole)
}

/// The trace entry an agent has for `day`, or, for today (not yet written),
/// one built from live state.
fn trace_on(world: &World, id: EntityId, trace: &Trace, day: u64) -> Option<DayTrace> {
    trace.on_day(day).or_else(|| {
        (day == world.day() && crate::systems::law::living(world, id))
            .then(|| crate::systems::stats::live_day_trace(world, id))
    })
}

/// Weighted candidates for a hole, ascending by id (pub for tests):
/// alive and free that day per their trace, an adult with a Personality now,
/// not the victim or the victim's spouse. Weight exactly
/// `(1 − l)² × (1 + gang_claim_mult·g·c) × (1 + enemy_mult·e) × (1 + statistical_mult·s)`,
/// times `other_zone_weight` off the victim's zone.
pub fn candidates(world: &World, hole: &Hole) -> Vec<(EntityId, f64)> {
    let cfg = &world.config.bind;
    let day = time::day(hole.tick);
    let claim_gang = hole.home.and_then(|h| world.comp::<crate::components::Building>(h)).and_then(|b| b.claim);
    let claim_gang = claim_gang.map(|c| c.gang);
    let mut out = Vec::new();
    for id in world.with::<Trace>() {
        if id == hole.victim || Some(id) == hole.spouse {
            continue;
        }
        let Some(trace) = world.comp::<Trace>(id) else { continue };
        let Some(t) = trace_on(world, id, trace, day) else { continue };
        if !t.has(trace_flags::ALIVE) || t.has(trace_flags::JAILED) {
            continue;
        }
        let Some(p) = world.comp::<Personality>(id) else { continue };
        let l = f64::from(p.lawfulness);
        let g = t.has(trace_flags::GANG);
        let c = claim_gang.is_some() && claim_gang == world.gang_of(id);
        let e = world.enemies.get(&hole.victim).is_some_and(|s| s.contains(&id));
        let s = t.has(trace_flags::STATISTICAL_ALL_DAY);
        let mut w = (1.0 - l).powi(2)
            * (1.0 + cfg.gang_claim_mult * f64::from(u8::from(g && c)))
            * (1.0 + cfg.enemy_mult * f64::from(u8::from(e)))
            * (1.0 + cfg.statistical_mult * f64::from(u8::from(s)));
        if t.zone != hole.zone {
            w *= cfg.other_zone_weight;
        }
        if w > 0.0 {
            out.push((id, w));
        }
    }
    out
}

/// Guard presence in a zone yesterday, normalised by Homes (M10 D33):
/// `(hours_z / homes_z) / (hours_all / homes_all)` clamped to
/// `coverage_min..=coverage_max`; a zone without Homes reads the max, no
/// guard hours anywhere reads 1.
pub fn zone_law_coverage(world: &World, zone: Zone) -> f32 {
    let cfg = &world.config.bind;
    let hours = world.zone_watch.yesterday;
    let hours_all: u32 = hours.iter().sum();
    if hours_all == 0 {
        return 1.0;
    }
    let mut homes = [0u32; 5];
    for &h in world.buildings_of_kind(BuildingKind::Home) {
        if let Some(b) = world.comp::<crate::components::Building>(h).filter(|b| !b.demolished) {
            homes[world.map.zone(b.door).index()] += 1;
        }
    }
    let homes_all: u32 = homes.iter().sum();
    let (hz, nz) = (hours[zone.index()], homes[zone.index()]);
    if nz == 0 || homes_all == 0 {
        return cfg.coverage_max;
    }
    let cov = (hz as f32 / nz as f32) / (hours_all as f32 / homes_all as f32);
    cov.clamp(cfg.coverage_min, cfg.coverage_max)
}

/// Attribute a hole. Deterministic in `(world seed, hole id)` and the traces.
/// `None` if no such hole is open.
pub fn bind(world: &mut World, id: HoleId) -> Option<Bound> {
    // 1. Out of the indices.
    let hole = take(world, id)?;
    // 2. The hole's own stream.
    let mut rng = world.rng.hole(id);
    let cfg = world.config.bind.clone();
    // 3. Unknown first (D11), else a weighted draw over the candidates.
    let unknown = rng.random::<f64>() < cfg.p_unknown;
    let mut bound = Bound::Unknown;
    if !unknown {
        let cands = candidates(world, &hole);
        let total: f64 = cands.iter().map(|&(_, w)| w).sum();
        if total > 0.0 {
            let u = rng.random::<f64>() * total;
            let mut acc = 0.0;
            let mut pick = cands.last().map(|&(c, _)| c);
            for &(c, w) in &cands {
                acc += w;
                if u < acc {
                    pick = Some(c);
                    break;
                }
            }
            if let Some(c) = pick {
                bound = Bound::Actor(c);
            }
        }
    }
    // 4. The witness roll is always drawn (the spec's "bound witness decision").
    let p = cfg.p_witness * zone_law_coverage(world, hole.zone);
    let witnessed = rng.random::<f32>() < p;
    let mut witness = None;
    if let (true, Bound::Actor(actor)) = (witnessed, bound) {
        let day = time::day(hole.tick);
        let pool: Vec<EntityId> = world
            .with::<Trace>()
            .into_iter()
            .filter(|&w| w != actor && w != hole.victim)
            .filter(|&w| crate::systems::law::living(world, w) && crate::systems::demography::is_adult(world, w))
            .filter(|&w| {
                world
                    .comp::<Trace>(w)
                    .and_then(|t| trace_on(world, w, t, day))
                    .is_some_and(|t| t.has(trace_flags::ALIVE) && t.zone == hole.zone)
            })
            .collect();
        if !pool.is_empty() {
            witness = Some(pool[rng.random_range(0..pool.len())]);
        }
    }
    let victim = hole.victim;
    if let Bound::Actor(actor) = bound {
        // 5. The actor's side, and the victim's memories get their subject.
        match hole.kind {
            HoleKind::Assaulted | HoleKind::Killed => {
                world.remember(actor, MemoryKind::Fought, Some(victim), 0.7, -0.5, false);
                world.remember(actor, MemoryKind::Won, Some(victim), 0.6, 0.4, false);
            }
            HoleKind::Robbed => {
                if let Some(w) = world.comp_mut::<Wallet>(actor) {
                    w.coins += hole.loot;
                }
            }
        }
        let kinds: &[MemoryKind] = match hole.kind {
            HoleKind::Robbed => &[MemoryKind::WasRobbed],
            HoleKind::Assaulted | HoleKind::Killed => &[MemoryKind::Fought, MemoryKind::Lost],
        };
        if let Some(m) = world.comp_mut::<crate::components::Memory>(victim) {
            for e in m.entries.iter_mut() {
                if kinds.contains(&e.kind) && e.tick == hole.tick && e.subject.is_none() {
                    e.subject = Some(actor);
                }
            }
        }
        // Only a living victim gets the Enemy edge: a corpse's edges are
        // dropped when it is freed, and nobody should come to hate the dead.
        if crate::systems::law::living(world, victim) {
            crate::systems::social::robbed_by(world, victim, actor);
        }
        // 6. A witness reports it: a cold case on the normal arrest path.
        if let Some(w) = witness {
            let crime = hole.kind.crime();
            world.remember_crime(w, actor, crime, crate::systems::law::crime_salience(crime));
            crate::systems::law::file_report(world, crime, actor, Some(w));
            if world.comp::<crate::components::Brain>(actor).is_some_and(|b| b.lod == Lod::Statistical) {
                crate::systems::lod::set_lod(world, actor, Lod::Coarse);
            }
        }
        // 7. Consequences (D31).
        if hole.kind == HoleKind::Killed {
            if let (Some(gang), Some(theirs)) = (hole.gang, world.gang_of(actor)) {
                if theirs != gang {
                    crate::systems::gang::push_shock(world, gang, Shock::MemberKilled { by_rival: true });
                }
            }
        }
        if hole.kind == HoleKind::Assaulted && crate::systems::law::is_guard(world, victim) {
            crate::systems::law_brain::push_shock(world, LawShock::GuardBeaten);
        }
    }
    // 8. The ring entry names the actor (or says nobody ever will).
    rewrite_event(world, &hole, bound);
    // 9. Counters, 10. the Attributed event.
    match bound {
        Bound::Actor(_) => world.stats.current.holes_bound += 1,
        Bound::Unknown => world.stats.current.holes_unknown += 1,
    }
    attributed_event(world, &hole, bound, witness);
    Some(bound)
}

fn rewrite_event(world: &mut World, hole: &Hole, bound: Bound) {
    let actor_name = match bound {
        Bound::Actor(a) => Some(world.name_of(a)),
        Bound::Unknown => None,
    };
    if let Some(e) = world.event_mut(hole.event_id) {
        match (bound, actor_name) {
            (Bound::Actor(a), Some(name)) => {
                if let Some(slot) = e.actors.first_mut() {
                    *slot = a;
                }
                e.text.push_str(&format!(" (laid to {name})"));
            }
            _ => e.text.push_str(" (never named)"),
        }
    }
}

fn attributed_event(world: &mut World, hole: &Hole, bound: Bound, witness: Option<EntityId>) {
    let day = time::day(hole.tick);
    let victim = world.name_of(hole.victim);
    let what = format!("the {} {} of {victim} on day {day}", hole.zone, hole.kind.noun());
    match bound {
        Bound::Actor(a) => {
            let seen = witness.map(|w| format!(", seen by {}", world.name_of(w))).unwrap_or_default();
            let text = format!("{what} is laid to {}{seen}", world.name_of(a));
            world.push_event(EventKind::Attributed, &[a, hole.victim], text);
        }
        Bound::Unknown => {
            world.push_event(EventKind::Attributed, &[hole.victim], format!("{what} will never be solved"));
        }
    }
}

/// Close a hole as Unknown without a draw (the per-victim cap).
pub fn expire(world: &mut World, id: HoleId) {
    let Some(hole) = take(world, id) else { return };
    rewrite_event(world, &hole, Bound::Unknown);
    world.stats.current.holes_unknown += 1;
    attributed_event(world, &hole, Bound::Unknown, None);
}

/// Daily pass at `tick_of_day == 0`: every consequential hole from before
/// today, then every hole past `hole_ttl_days`, each in ascending id order.
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    let now = world.tick;
    let day_start = time::day(now) * TICKS_PER_DAY;
    let due: Vec<HoleId> =
        world.holes.values().filter(|h| h.consequential && h.tick < day_start).map(|h| h.id).collect();
    for id in due {
        bind(world, id);
    }
    let ttl = world.config.bind.hole_ttl_days * TICKS_PER_DAY;
    let old: Vec<HoleId> = world.holes.values().filter(|h| h.tick + ttl <= now).map(|h| h.id).collect();
    for id in old {
        bind(world, id);
    }
}

/// Promotion binds queued by `lod::set_lod`; drained at the end of
/// `lod::run`, oldest first. A bind may promote a witnessed actor and queue
/// more; the loop runs until the queue is empty.
pub fn drain_queue(world: &mut World) {
    while !world.bind_queue.is_empty() {
        let mut ids = std::mem::take(&mut world.bind_queue);
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            bind(world, id);
        }
    }
}

/// Bind every open hole, oldest first (tests, end-of-run parity).
pub fn bind_all(world: &mut World) {
    let ids: Vec<HoleId> = world.holes.keys().copied().collect();
    for id in ids {
        bind(world, id);
    }
}
