//! The social graph: edges created by co-location, drifted by interaction,
//! promoted by threshold, decayed and pruned daily; proposals; gossip.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Edge, Household, Job, MemoryKind, Needs, Personality, Position, RelKind, Sentence,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// Ticks of shared building before an edge exists (config: edge_create_ticks).
fn edge_create_ticks(world: &World) -> u16 {
    world.config.social.edge_create_ticks as u16
}

/// `m = 1.5` when the two are within 0.2 lawfulness of each other.
pub fn similarity_mult(world: &World, a: EntityId, b: EntityId) -> f32 {
    let la = world.comp::<Personality>(a).map_or(0.5, |p| p.lawfulness);
    let lb = world.comp::<Personality>(b).map_or(0.5, |p| p.lawfulness);
    if (la - lb).abs() < 0.2 {
        1.5
    } else {
        1.0
    }
}

/// Apply the promotion thresholds after a change. Family, Parent and Spouse
/// never change by threshold.
pub fn promote(e: &mut Edge) {
    if matches!(e.kind, RelKind::Family | RelKind::Parent | RelKind::Spouse) {
        return;
    }
    let a = e.affinity;
    e.kind = match e.kind {
        _ if a <= -0.6 => RelKind::Enemy,
        RelKind::Enemy if a > -0.3 => RelKind::Rival,
        RelKind::Enemy => RelKind::Enemy,
        RelKind::Rival if a > 0.0 => RelKind::Acquaintance,
        RelKind::Rival => RelKind::Rival,
        RelKind::Acquaintance | RelKind::Friend if a <= -0.3 => RelKind::Rival,
        RelKind::Acquaintance if a >= 0.4 => RelKind::Friend,
        other => other,
    };
}

/// Death effects on the graph: Grief (0.9, -0.9) to every Spouse / Parent /
/// Family partner, the widow(er) freed to remarry (the Spouse edge itself
/// stays, per the spec), the dead dropped from the enemies index.
pub fn on_death(world: &mut World, id: EntityId) {
    grieve(world, id);
    unlink(world, id);
}

/// Grief (0.9, -0.9) to every Spouse / Parent / Family partner.
pub fn grieve(world: &mut World, id: EntityId) {
    let kin: Vec<EntityId> = world
        .neighbours(id)
        .filter(|&o| {
            world.has::<Brain>(o)
                && world
                    .edge(id, o)
                    .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family))
        })
        .collect();
    for k in kin {
        world.remember(k, MemoryKind::Grief, Some(id), 0.9, -0.9, false);
    }
}

/// Drop `id` from the spouse lookup (the widow(er) may remarry) and the
/// enemies index. No memories: used for emigrants and freed corpses too.
pub fn unlink(world: &mut World, id: EntityId) {
    if let Some(spouse) = world.spouses.remove(&id) {
        world.spouses.remove(&spouse);
        if let Some(p) = world.comp_mut::<Personality>(spouse) {
            p.drift(crate::personality::Drift::SpouseDied);
        }
    }
    if let Some(enemies) = world.enemies.remove(&id) {
        for e in enemies {
            if let Some(s) = world.enemies.get_mut(&e) {
                s.remove(&id);
            }
        }
    }
}

/// Keep the enemies index in step with an edge's kind. Call after any kind change.
pub fn reindex_kind(world: &mut World, a: EntityId, b: EntityId) {
    let enemy = world.edge(a, b).is_some_and(|e| e.kind == RelKind::Enemy);
    reindex_kind_as(world, a, b, enemy);
}

/// `reindex_kind` with the edge's Enemy-ness already in hand.
fn reindex_kind_as(world: &mut World, a: EntityId, b: EntityId, enemy: bool) {
    for (x, y) in [(a, b), (b, a)] {
        if enemy {
            world.enemies.entry(x).or_default().insert(y);
        } else if let Some(s) = world.enemies.get_mut(&x) {
            s.remove(&y);
        }
    }
}

/// Nudge an edge (creating it at affinity 0 if absent) and re-promote.
pub fn adjust(world: &mut World, a: EntityId, b: EntityId, d_affinity: f32, d_trust: f32) {
    if a == b {
        return;
    }
    let tick = world.tick;
    let e = world.edge_entry(a, b);
    e.affinity = (e.affinity + d_affinity).clamp(-1.0, 1.0);
    e.trust = (e.trust + d_trust).clamp(0.0, 1.0);
    e.last_interaction = tick;
    promote(e);
    let enemy = e.kind == RelKind::Enemy;
    reindex_kind_as(world, a, b, enemy);
}

/// Being robbed or extorted by someone: an Enemy, at once.
pub fn robbed_by(world: &mut World, victim: EntityId, robber: EntityId) {
    make_enemy(world, victim, robber, -0.6);
}

/// Turn an edge hostile: `d_affinity` (negative), trust to zero, kind Enemy.
pub fn make_enemy(world: &mut World, a: EntityId, b: EntityId, d_affinity: f32) {
    if a == b {
        return;
    }
    let tick = world.tick;
    let e = world.edge_entry(a, b);
    e.affinity = (e.affinity + d_affinity).clamp(-1.0, 1.0);
    e.trust = 0.0;
    e.kind = RelKind::Enemy;
    e.last_interaction = tick;
    reindex_kind(world, a, b);
}

/// Witnessing someone's crime.
pub fn witnessed_crime_of(world: &mut World, witness: EntityId, actor: EntityId) {
    adjust(world, witness, actor, -0.2, -0.1);
}

/// A fight: both lose affinity, the loser loses trust in the winner.
pub fn fought(world: &mut World, winner: EntityId, loser: EntityId) {
    adjust(world, winner, loser, -0.3, 0.0);
    let tick = world.tick;
    let e = world.edge_entry(winner, loser);
    e.trust = (e.trust - 0.2).max(0.0);
    e.last_interaction = tick;
}

/// Interaction drift (Chat, Drink together, Flirt, each hour of co-work;
/// co-jail drifts in four-hour steps): `affinity += 0.05 × m`, `trust += 0.02`.
pub fn interacted(world: &mut World, a: EntityId, b: EntityId) {
    let m = similarity_mult(world, a, b);
    let step = world.config.social.affinity_per_hour * m;
    adjust(world, a, b, step, 0.02);
}

pub fn has_spouse(world: &World, id: EntityId) -> bool {
    world.spouses.contains_key(&id)
}

pub fn spouse_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.spouse_of(id)
}

/// Everyone held by a Partner reservation of someone other than `holder`
/// (an agent's own reservation must not hide their own partner from them).
pub fn reserved_partners(world: &World, holder: EntityId) -> std::collections::BTreeSet<EntityId> {
    world
        .reservations
        .iter()
        .filter(|(&h, _)| h != holder)
        .flat_map(|(_, rs)| rs.iter())
        .filter_map(|r| match r.kind {
            crate::exec::ReservationKind::Partner { other } => Some(other),
            _ => None,
        })
        .collect()
}

/// The co-located agent with the highest affinity who is reservable.
pub fn best_colocated_partner(world: &World, id: EntityId, min_affinity: f32, unmarried: bool) -> Option<EntityId> {
    // Alone (the usual case at home): no reservation scan (perf).
    let here = world.comp::<crate::components::Position>(id)?.building?;
    let b = world.comp::<Building>(here)?;
    if !b.occupants.iter().any(|&o| o != id && world.has::<Brain>(o) && !world.has::<Sentence>(o)) {
        return None;
    }
    let reserved = reserved_partners(world, id);
    best_colocated_partner_in(world, id, min_affinity, unmarried, &reserved)
}

/// As `best_colocated_partner`, with the reservation set supplied by the caller.
pub fn best_colocated_partner_in(
    world: &World,
    id: EntityId,
    min_affinity: f32,
    unmarried: bool,
    reserved: &std::collections::BTreeSet<EntityId>,
) -> Option<EntityId> {
    let here = world.comp::<crate::components::Position>(id)?.building?;
    let b = world.comp::<Building>(here)?;
    b.occupants
        .iter()
        .copied()
        .filter(|&o| o != id && world.has::<Brain>(o) && !world.has::<Sentence>(o))
        .filter(|&o| !unmarried || !has_spouse(world, o))
        .filter(|o| !reserved.contains(o))
        .map(|o| (o, world.edge(id, o).map_or(0.0, |e| e.affinity)))
        .filter(|&(_, a)| a >= min_affinity)
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
        .map(|(o, _)| o)
}

/// A known partner candidate: an edge with affinity >= `min`, both unmarried.
pub fn known_candidate(world: &World, id: EntityId, min: f32) -> Option<EntityId> {
    if has_spouse(world, id) {
        return None;
    }
    // One memory scan for the week's rejections, not one per neighbour.
    let week = 7 * TICKS_PER_DAY;
    let rejected: smallvec::SmallVec<[EntityId; 8]> = world
        .comp::<crate::components::Memory>(id)
        .map(|m| {
            m.entries
                .iter()
                .filter(|e| e.kind == MemoryKind::Rejected && world.tick.saturating_sub(e.tick) < week)
                .filter_map(|e| e.subject)
                .collect()
        })
        .unwrap_or_default();
    world
        .neighbours(id)
        .map(|o| (o, world.edge(id, o).map_or(0.0, |e| e.affinity)))
        .filter(|&(_, a)| a >= min)
        .filter(|&(o, _)| world.has::<Brain>(o) && !has_spouse(world, o) && !rejected.contains(&o))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
        .map(|(o, _)| o)
}

/// The best known candidate who is at a Bar or Market right now: somewhere a
/// Court plan can walk to and find them. Scans the two venues' occupants.
pub fn candidate_at_venue(world: &World, id: EntityId, min: f32) -> Option<EntityId> {
    if has_spouse(world, id) {
        return None;
    }
    let week = 7 * TICKS_PER_DAY;
    let rejected: smallvec::SmallVec<[EntityId; 8]> = world
        .comp::<crate::components::Memory>(id)
        .map(|m| {
            m.entries
                .iter()
                .filter(|e| e.kind == MemoryKind::Rejected && world.tick.saturating_sub(e.tick) < week)
                .filter_map(|e| e.subject)
                .collect()
        })
        .unwrap_or_default();
    let reserved = reserved_partners(world, id);
    candidate_at_venue_in(world, id, min, &rejected, &reserved)
}

/// As `candidate_at_venue`, with the rejection and reservation sets supplied.
pub fn candidate_at_venue_in(
    world: &World,
    id: EntityId,
    min: f32,
    rejected: &[EntityId],
    reserved: &std::collections::BTreeSet<EntityId>,
) -> Option<EntityId> {
    let venues = [BuildingKind::Bar, BuildingKind::Market];
    venues
        .iter()
        .filter_map(|k| world.buildings_by_kind.get(k))
        .flatten()
        .filter_map(|&b| world.comp::<Building>(b))
        .flat_map(|bd| bd.occupants.iter().copied())
        .filter(|&o| o != id && world.has::<Brain>(o) && !world.has::<Sentence>(o))
        .filter(|&o| !has_spouse(world, o) && !rejected.contains(&o) && !reserved.contains(&o))
        .map(|o| (o, world.edge(id, o).map_or(0.0, |e| e.affinity)))
        .filter(|&(_, a)| a >= min)
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
        .map(|(o, _)| o)
}

/// The Court plan's target: a proposable partner in the room (Propose here),
/// else a candidate in the room when this is a Bar or Market (Flirt here),
/// else a candidate now at a Bar or Market (walk there and Flirt).
pub fn court_target(world: &World, id: EntityId) -> Option<EntityId> {
    if has_spouse(world, id) {
        return None;
    }
    let here = world.comp::<Position>(id)?.building;
    let venue = here
        .and_then(|b| world.comp::<Building>(b))
        .is_some_and(|bd| matches!(bd.kind, BuildingKind::Bar | BuildingKind::Market));
    let reserved = reserved_partners(world, id);
    if here.is_some() {
        let min = world.config.social.propose_affinity;
        if let Some(t) =
            best_colocated_partner_in(world, id, min, true, &reserved).filter(|&t| propose_allowed(world, id, t))
        {
            return Some(t);
        }
        if venue {
            if let Some(t) = best_colocated_partner_in(world, id, 0.3, true, &reserved) {
                return Some(t);
            }
        }
    }
    let rejected = rejected_recently_by(world, id);
    candidate_at_venue_in(world, id, 0.3, &rejected, &reserved)
}

/// Subjects of this week's Rejected memories.
pub fn rejected_recently_by(world: &World, id: EntityId) -> smallvec::SmallVec<[EntityId; 8]> {
    let week = 7 * TICKS_PER_DAY;
    world
        .comp::<crate::components::Memory>(id)
        .map(|m| {
            m.entries
                .iter()
                .filter(|e| e.kind == MemoryKind::Rejected && world.tick.saturating_sub(e.tick) < week)
                .filter_map(|e| e.subject)
                .collect()
        })
        .unwrap_or_default()
}

/// Propose: both unmarried, affinity >= 0.6, trust >= 0.5; roll
/// `rng < 0.5 + 0.5 × affinity`. Returns whether they married.
/// Both unmarried, affinity >= 0.6, trust >= 0.5.
pub fn propose_allowed(world: &World, proposer: EntityId, target: EntityId) -> bool {
    let (aff, trust) = world.edge(proposer, target).map_or((0.0, 0.0), |e| (e.affinity, e.trust));
    !has_spouse(world, proposer)
        && !has_spouse(world, target)
        && aff >= world.config.social.propose_affinity
        && trust >= world.config.social.propose_trust
}

pub fn propose(world: &mut World, proposer: EntityId, target: EntityId) -> bool {
    let aff = world.edge(proposer, target).map_or(0.0, |e| e.affinity);
    if !propose_allowed(world, proposer, target) {
        world.remember(proposer, MemoryKind::Rejected, Some(target), 0.6, -0.5, false);
        adjust(world, proposer, target, -0.1, 0.0);
        return false;
    }
    let roll: f32 = world.rng.world().random();
    if roll >= 0.5 + 0.5 * aff {
        world.remember(proposer, MemoryKind::Rejected, Some(target), 0.6, -0.5, false);
        adjust(world, proposer, target, -0.1, 0.0);
        let (a, b) = (world.name_of(proposer), world.name_of(target));
        world.push_event(EventKind::Proposal, &[proposer, target], format!("{a} proposed to {b}: refused"));
        return false;
    }
    marry(world, proposer, target);
    true
}

/// The wedding: a Spouse edge, memories, trait drift, intimacy, and one of
/// them moves in with the other.
pub fn marry(world: &mut World, a: EntityId, b: EntityId) {
    world.set_spouse(a, b);
    for who in [a, b] {
        world.remember(who, MemoryKind::Married, Some(if who == a { b } else { a }), 1.0, 0.9, false);
        if let Some(p) = world.comp_mut::<Personality>(who) {
            p.drift(Drift::Married);
        }
        if let Some(n) = world.comp_mut::<Needs>(who) {
            n.intimacy = (n.intimacy + 0.5).min(1.0);
        }
    }
    // Move in: the target joins the proposer's Home if it has room, else the reverse.
    let home_a = world.comp::<Household>(a).and_then(|h| h.home);
    let home_b = world.comp::<Household>(b).and_then(|h| h.home);
    let room = |w: &World, h: Option<EntityId>| {
        h.and_then(|h| w.comp::<Building>(h).map(|bd| w.residents_of(h).len() < usize::from(bd.capacity)))
            .unwrap_or(false)
    };
    if home_a != home_b {
        if room(world, home_a) {
            world.set_home(b, home_a);
        } else if room(world, home_b) {
            world.set_home(a, home_b);
        }
    }
    let (na, nb) = (world.name_of(a), world.name_of(b));
    world.push_event(EventKind::Marriage, &[a, b], format!("{na} married {nb}"));
    if world.config.gossip.enabled {
        let d = crate::systems::gossip::talk_district(world, a);
        crate::systems::gossip::post_deed(world, d, crate::word::Deed::Married, Some(a), Some(b));
    }
}

/// Gossip on Chat completion: each party copies one own SawCrime/WasRobbed
/// memory (salience >= 0.5, with a subject) to the partner, second-hand at
/// 0.6 × salience; the receiver's edge to the subject gets affinity −0.1.
pub fn gossip(world: &mut World, from: EntityId, to: EntityId) {
    let candidates: Vec<(MemoryKind, EntityId, f32, Option<crate::components::Crime>)> = world
        .comp::<crate::components::Memory>(from)
        .map(|m| {
            m.entries
                .iter()
                .filter(|e| matches!(e.kind, MemoryKind::SawCrime | MemoryKind::WasRobbed) && e.salience >= 0.5)
                .filter_map(|e| e.subject.map(|s| (e.kind, s, e.salience, e.crime)))
                .collect()
        })
        .unwrap_or_default();
    if candidates.is_empty() {
        return;
    }
    let i = world.rng.world().random_range(0..candidates.len());
    let (kind, subject, salience, crime) = candidates[i];
    let already = world
        .comp::<crate::components::Memory>(to)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == kind && e.subject == Some(subject)));
    if already || subject == to {
        return;
    }
    let tick = world.tick;
    let cap = world.config.brain.memory_cap;
    let half_life = world.config.brain.memory_half_life_days;
    if let Some(m) = world.comp_mut::<crate::components::Memory>(to) {
        let entry = crate::components::MemoryEntry {
            kind,
            subject: Some(subject),
            tick,
            salience: salience * 0.6,
            valence: -salience * 0.6,
            second_hand: true,
            crime,
            ..crate::components::MemoryEntry::blank(kind, tick)
        };
        crate::systems::memory::insert(m, entry, tick, cap, half_life);
    }
    adjust(world, to, subject, -0.1, 0.0);
}

/// Per tick: co-location counting and edge creation; hourly co-work /
/// co-jail drift. Daily: decay, pruning, debt ageing.
pub fn run(world: &mut World) {
    colocation(world);
    if world.tick_of_day() == 0 {
        daily(world);
    }
}

/// Co-location without a pair table: the shared time of two agents in a
/// building is the later entrant's stay, so the later entrant drives each
/// pair. An agent whose stay reaches 30 ticks creates edges with everyone who
/// was already there; each full hour of its stay drifts the pairs that share a
/// workplace or the Jail.
/// Co-jailed pairs drift once every this many hours (M10 phase 5b).
const JAIL_DRIFT_HOURS: u64 = 4;

fn colocation(world: &mut World) {
    let create_at = Tick::from(edge_create_ticks(world));
    let tick = world.tick;
    let mut events: Vec<(EntityId, EntityId, EntityId, bool)> = Vec::new(); // (a, b, building, create)
    for a in world.bodies() {
        let Some(pa) = world.comp::<Position>(a) else { continue };
        let Some(building) = pa.building else { continue };
        let stay = tick.saturating_sub(pa.entered);
        let create = stay == create_at;
        let hour = stay >= create_at && stay.is_multiple_of(TICKS_PER_HOUR);
        if !create && !hour {
            continue;
        }
        let Some(bd) = world.comp::<Building>(building) else { continue };
        // The Jail's ~70 prisoners drift every `JAIL_DRIFT_HOURS` hours, by
        // that many hours' worth: the hourly pairs were a fifth of a tick.
        if !create && bd.kind == BuildingKind::Jail && !stay.is_multiple_of(JAIL_DRIFT_HOURS * TICKS_PER_HOUR) {
            continue;
        }
        // An hourly pair outside the Jail drifts only between co-workers of
        // this building: skip the occupant scan when `a` is not one (perf;
        // such pairs had no effect below).
        let works_here = |o: EntityId| world.comp::<Job>(o).is_some_and(|j| j.employer == Some(building));
        let jail = bd.kind == BuildingKind::Jail;
        if !create && !jail && !works_here(a) {
            continue;
        }
        for &b in &bd.occupants {
            if b == a || !world.has::<Brain>(b) {
                continue;
            }
            if !create && !jail && !works_here(b) {
                continue;
            }
            let Some(pb) = world.comp::<Position>(b) else { continue };
            let stay_b = tick.saturating_sub(pb.entered);
            // The later entrant (ties: the lower id) owns the pair.
            if stay_b < stay || (stay_b == stay && b < a) {
                continue;
            }
            events.push((a, b, building, create));
        }
    }
    for (a, b, building, create) in events {
        let jail = world.comp::<Building>(building).is_some_and(|bd| bd.kind == BuildingKind::Jail);
        // M15 W12: each body of the pair may note where it saw the other
        // (not inside the Precinct: where an inmate is is no news, and its
        // ~160 cellmates' pairs were most of the calls).
        if world.config.gossip.enabled && !jail {
            let tile = world.comp::<Position>(b).map_or_else(Default::default, |p| p.tile);
            crate::systems::gossip::maybe_sight(world, a, b, Some(building), tile);
            crate::systems::gossip::maybe_sight(world, b, a, Some(building), tile);
        }
        if create {
            if world.edge(a, b).is_none() {
                let (sa, sb) = (
                    world.comp::<Personality>(a).map_or(0.5, |p| p.sociability),
                    world.comp::<Personality>(b).map_or(0.5, |p| p.sociability),
                );
                let u1: f32 = world.rng.world().random::<f32>();
                let u2: f32 = world.rng.world().random();
                first_meeting(world, a, b, first_affinity(sa, sb, u1, u2));
            }
            if jail {
                world.remember(a, MemoryKind::MetInJail, Some(b), 0.5, 0.0, false);
                world.remember(b, MemoryKind::MetInJail, Some(a), 0.5, 0.0, false);
            }
        } else {
            let cowork = world.comp::<Job>(a).is_some_and(|j| j.employer == Some(building))
                && world.comp::<Job>(b).is_some_and(|j| j.employer == Some(building));
            if jail {
                let step = world.config.social.affinity_per_hour * similarity_mult(world, a, b);
                let hours = JAIL_DRIFT_HOURS as f32;
                adjust(world, a, b, hours * step, hours * 0.02);
            } else if cowork {
                interacted(world, a, b);
            }
        }
    }
}

/// The v1 first-meeting affinity: the pair's mean sociability x 0.1 plus
/// N(0, 0.05) noise (Box-Muller from two uniforms), clamped to ±0.15.
pub fn first_affinity(sa: f32, sb: f32, u1: f32, u2: f32) -> f32 {
    let noise = (-2.0 * u1.max(1e-6).ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos() * 0.05;
    ((sa + sb) / 2.0 * 0.1 + noise).clamp(-0.15, 0.15)
}

/// A new Acquaintance edge at `affinity`, trust 0.3 (`colocation`'s first
/// meeting; `lod::stat_social`'s meeting off screen).
pub fn first_meeting(world: &mut World, a: EntityId, b: EntityId, affinity: f32) {
    let tick = world.tick;
    let e = world.edge_entry(a, b);
    e.affinity = affinity;
    e.trust = 0.3;
    e.kind = RelKind::Acquaintance;
    e.last_interaction = tick;
}

fn daily(world: &mut World) {
    let tick = world.tick;
    let week = 7 * TICKS_PER_DAY;
    let month = 30 * TICKS_PER_DAY;
    let mut prune = Vec::new();
    let mut aged_debts = Vec::new();
    let mut rekinded = Vec::new();
    // Each edge's update is its own; only the three lists depend on order,
    // and they are sorted into key order below (the hash map's order is not
    // the key order; sorting every edge daily cost ~6 %).
    for (&(a, b), e) in world.edges.iter_mut_unordered() {
        if tick.saturating_sub(e.last_interaction) >= week {
            e.affinity *= 0.98;
            let before = e.kind;
            promote(e);
            if e.kind != before {
                rekinded.push((a, b));
            }
        }
        if e.kind == RelKind::Acquaintance
            && e.affinity.abs() < 0.05
            && tick.saturating_sub(e.last_interaction) >= month
        {
            prune.push((a, b));
        }
        if e.debt != 0 {
            if let Some(since) = e.debt_since {
                if tick.saturating_sub(since) >= 14 * TICKS_PER_DAY {
                    aged_debts.push((a, b));
                    e.debt_since = Some(tick); // charged once per fortnight
                }
            }
        }
    }
    rekinded.sort_unstable();
    prune.sort_unstable();
    aged_debts.sort_unstable();
    for (a, b) in rekinded {
        reindex_kind(world, a, b);
    }
    for (a, b) in prune {
        world.remove_edge(a, b);
    }
    // A debt older than 14 days costs affinity on the donor's side (the creditor).
    for (a, b) in aged_debts {
        let e = world.edge_entry(a, b);
        e.affinity = (e.affinity - 0.15).max(-1.0);
        promote(e);
        reindex_kind(world, a, b);
    }
}

/// Automatic repayment at CollectWage when `coins > 20`: pay
/// `min(debt, coins − 10)` to each creditor; both sides gain affinity and trust.
pub fn repay_debts(world: &mut World, id: EntityId) {
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
    if coins <= 20 {
        return;
    }
    let creditors: Vec<(EntityId, i32)> = world
        .neighbours(id)
        .filter_map(|o| {
            world.edge(id, o).map(|e| (o, e)).and_then(|(o, e)| {
                // the lower id owes the higher id when debt > 0
                let (lo, _) = crate::components::edge_key(id, o);
                let owed = if lo == id { e.debt } else { -e.debt };
                (owed > 0).then_some((o, owed))
            })
        })
        .collect();
    for (creditor, owed) in creditors {
        let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
        let pay = i64::from(owed).min(coins - 10).max(0);
        if pay <= 0 {
            break;
        }
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(id) {
            w.coins -= pay;
        }
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(creditor) {
            w.coins += pay;
        }
        let (lo, _) = crate::components::edge_key(id, creditor);
        let e = world.edge_entry(id, creditor);
        e.debt += if lo == id { -(pay as i32) } else { pay as i32 };
        let repaid = e.debt == 0;
        if repaid {
            e.debt_since = None;
        }
        adjust(world, id, creditor, 0.1, 0.1);
        // M15: a debt repaid in full is talked about.
        if repaid && world.config.gossip.enabled {
            let d = crate::systems::gossip::talk_district(world, id);
            crate::systems::gossip::post_deed(world, d, crate::word::Deed::Repaid, Some(id), Some(creditor));
        }
    }
}

/// Was `who` rejected by `by` in the last 7 days? (Court: blocks Flirt at them.)
impl World {
    pub fn rejected_recently(&self, who: EntityId, by: EntityId) -> bool {
        let week = 7 * TICKS_PER_DAY;
        self.comp::<crate::components::Memory>(who).is_some_and(|m| {
            m.entries.iter().any(|e| {
                e.kind == MemoryKind::Rejected && e.subject == Some(by) && self.tick.saturating_sub(e.tick) < week
            })
        })
    }
}

/// Keep `Tick` in the public API for callers.
pub type SocialTick = Tick;
