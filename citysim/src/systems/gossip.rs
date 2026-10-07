//! M15 § 1 (plan phase 1.5-1.7): deeds travel. A deed is a tagged record
//! of an earlier event; it reaches other agents as a copy in their `heard`
//! store, along three channels: an exchange when two bodies finish a Chat
//! or a Drink, a per-district pool that the Statistical tier draws from once
//! a day, and the kin channel that hands a killing or a beating to the
//! victim's family and friends. Every roll is on a keyed word stream
//! (`SimRng::word`), never the world or an agent stream, and nothing here
//! writes `Memory.entries` or an edge while `[gossip] legacy_second_hand`
//! holds (plan W9, W46): phase 1 adds knowledge that no decision reads.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use smallvec::SmallVec;

use crate::components::{
    Brain, Crime, DistrictId, GangMember, Hole, HoleKind, Household, Lod, Memory, MemoryEntry, MemoryKind, Personality,
    Position, RelKind, TilePos, Trace,
};
use crate::entity::EntityId;
use crate::rng::splitmix64;
use crate::systems::memory::{self, HeardInsert};
use crate::time::{self, Tick, TICKS_PER_DAY};
use crate::word::{Deed, DeedRef, PoolEntry, WordNs};
use crate::world::World;

/// The deed a crime is (W3's table): Murder Killed, Assault Assaulted,
/// Theft and Grand Theft Robbed, Extortion Extorted; the rest none.
pub fn deed_of_crime(crime: Crime) -> Option<Deed> {
    match crime {
        Crime::Murder => Some(Deed::Killed),
        Crime::Assault => Some(Deed::Assaulted),
        Crime::Theft | Crime::GrandTheft => Some(Deed::Robbed),
        Crime::Extortion => Some(Deed::Extorted),
        Crime::Vagrancy
        | Crime::Manslaughter
        | Crime::Dealing
        | Crime::Abduction
        | Crime::Intrusion
        | Crime::DataTheft => None,
    }
}

/// The deed a hole is (an abduction is not a deed).
pub fn deed_of_hole(kind: HoleKind) -> Option<Deed> {
    match kind {
        HoleKind::Robbed => Some(Deed::Robbed),
        HoleKind::Assaulted => Some(Deed::Assaulted),
        HoleKind::Killed => Some(Deed::Killed),
        HoleKind::Abducted => None,
    }
}

/// The district of an agent's Home, if it has one.
pub fn home_district(world: &World, id: EntityId) -> Option<DistrictId> {
    world.comp::<Household>(id).and_then(|h| h.home).map(|h| world.district_of_building(h))
}

/// Where an agent's deeds are talked about: its Home's district, else the
/// district it stands in.
pub fn talk_district(world: &World, id: EntityId) -> DistrictId {
    home_district(world, id)
        .or_else(|| world.comp::<Position>(id).map(|p| world.district_of(p.tile)))
        .unwrap_or_default()
}

fn same_key(a: &PoolEntry, b: &PoolEntry) -> bool {
    a.deed == b.deed && a.actor == b.actor && a.object == b.object && time::day(a.tick) == time::day(b.tick)
}

// ---------------------------------------------------------------------------
// Posting and naming (W5, W6, W10)
// ---------------------------------------------------------------------------

/// W6: post an entry to district `d`'s pool. A matching entry (same deed,
/// actor, object and day) keeps the max reach and the min hops and takes a
/// hole, kin or story it lacked; past `pool_cap` the lowest-reach entry
/// goes (ties: the oldest, then the lowest object index).
pub fn post(world: &mut World, d: DistrictId, e: PoolEntry) {
    if !world.config.gossip.enabled || world.rumours.is_empty() {
        return;
    }
    let cap = world.config.gossip.pool_cap.max(1);
    let i = d.index().min(world.rumours.len() - 1);
    let pool = &mut world.rumours[i].entries;
    if let Some(x) = pool.iter_mut().find(|x| same_key(x, &e)) {
        x.reach = x.reach.max(e.reach);
        x.hops = x.hops.min(e.hops);
        if x.hole.is_none() {
            x.hole = e.hole;
        }
        if x.kin.is_empty() {
            x.kin = e.kin;
        }
        if x.story.is_none() {
            x.story = e.story;
        }
        return;
    }
    pool.push(e);
    while pool.len() > cap {
        let worst = pool
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.reach
                    .total_cmp(&b.reach)
                    .then(a.tick.cmp(&b.tick))
                    .then(a.object.map_or(u32::MAX, |o| o.index).cmp(&b.object.map_or(u32::MAX, |o| o.index)))
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        pool.remove(worst);
    }
}

/// A first telling of a deed done now: `reach0[deed]`, hops 0.
pub fn post_deed(world: &mut World, d: DistrictId, deed: Deed, actor: Option<EntityId>, object: Option<EntityId>) {
    post_deed_at(world, d, deed, actor, object, 1.0);
}

/// `post_deed` at `reach0[deed] × mult` (W26: a failed Deceive's Betrayed at half).
pub fn post_deed_at(
    world: &mut World,
    d: DistrictId,
    deed: Deed,
    actor: Option<EntityId>,
    object: Option<EntityId>,
    mult: f32,
) {
    if !world.config.gossip.enabled {
        return;
    }
    let reach = (world.config.gossip.reach0.get(deed) * mult).clamp(0.0, 1.0);
    let e = PoolEntry {
        deed,
        actor,
        object,
        tick: world.tick,
        hops: 0,
        reach,
        hole: None,
        story: None,
        kin: SmallVec::new(),
        told: SmallVec::new(),
        district: d,
    };
    post(world, d, e);
}

/// A raid or a riot at a door (actor the gang, or `None` for a riot's
/// crowd; object the target's owner), talked about in the door's district.
pub fn post_raid(world: &mut World, actor: Option<EntityId>, object: Option<EntityId>, door: TilePos) {
    if !world.config.gossip.enabled {
        return;
    }
    let d = world.district_of(door);
    post_deed(world, d, Deed::Raided, actor, object);
}

/// W10: the dead's kin (Spouse, Parents, Family, then Friends by affinity,
/// ties the lower id), at most `kin_cap`, read before `social::on_death`
/// unlinks them.
pub fn kin_of(world: &World, dead: EntityId) -> SmallVec<[EntityId; 8]> {
    let cap = world.config.gossip.kin_cap;
    let mut close: Vec<(u8, f32, EntityId)> = world
        .neighbours(dead)
        .filter(|&o| world.has::<Brain>(o))
        .filter_map(|o| {
            let e = world.edge(dead, o)?;
            let rank = match e.kind {
                RelKind::Spouse => 0,
                RelKind::Parent => 1,
                RelKind::Family => 2,
                RelKind::Friend => 3,
                _ => return None,
            };
            Some((rank, e.affinity, o))
        })
        .collect();
    close.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
    close.into_iter().take(cap).map(|(_, _, o)| o).collect()
}

/// W10 (`World::kill_by` with Violence): the killing goes into the
/// death's pool unnamed, carrying the kin.
pub fn post_killing(world: &mut World, dead: EntityId) {
    let kin = kin_of(world, dead);
    let d = world.comp::<Position>(dead).map(|p| world.district_of(p.tile)).unwrap_or_default();
    let reach = world.config.gossip.reach0.get(Deed::Killed);
    let e = PoolEntry {
        deed: Deed::Killed,
        actor: None,
        object: Some(dead),
        tick: world.tick,
        hops: 0,
        reach,
        hole: None,
        story: None,
        kin,
        told: SmallVec::new(),
        district: d,
    };
    post(world, d, e);
}

/// W48: a killer the city may come to know; sampled seven days on.
pub fn watch_killer(world: &mut World, actor: EntityId) {
    if world.kill_watch.len() >= 256 {
        world.kill_watch.pop_front();
    }
    let t = world.tick;
    world.kill_watch.push_back((t, actor));
}

/// Merge the duplicates a rename made in one pool (first kept).
fn dedupe(pool: &mut Vec<PoolEntry>) {
    let mut i = 0;
    while i < pool.len() {
        let mut j = i + 1;
        while j < pool.len() {
            if same_key(&pool[i], &pool[j]) {
                let x = pool.remove(j);
                let y = &mut pool[i];
                y.reach = y.reach.max(x.reach);
                y.hops = y.hops.min(x.hops);
                if y.kin.is_empty() {
                    y.kin = x.kin;
                }
                if y.hole.is_none() {
                    y.hole = x.hole;
                }
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

/// W10: a noticed Murder names today's anonymous killing of `object` in
/// every pool, and the killer goes on the watch list.
pub fn name_actor(world: &mut World, object: EntityId, actor: EntityId) {
    if !world.config.gossip.enabled {
        return;
    }
    let today = world.day();
    for pool in world.rumours.iter_mut() {
        let mut hit = false;
        for e in pool.entries.iter_mut() {
            if e.deed == Deed::Killed && e.actor.is_none() && e.object == Some(object) && time::day(e.tick) == today {
                e.actor = Some(actor);
                hit = true;
            }
        }
        if hit {
            dedupe(&mut pool.entries);
        }
    }
    watch_killer(world, actor);
}

/// W6: a bound hole names its anonymous pool entries (posted at
/// `open_hole`, the death's, the post-backs) and every held anonymous
/// rumour of it; a bound killing goes on the watch list. Plan deviation:
/// takes the hole (already out of `World::holes` at bind), not its id.
pub fn name_hole(world: &mut World, hole: &Hole, actor: EntityId) {
    if !world.config.gossip.enabled {
        return;
    }
    let Some(deed) = deed_of_hole(hole.kind) else { return };
    let day = time::day(hole.tick);
    let victim = Some(hole.victim);
    let matches = |e: &PoolEntry| {
        e.actor.is_none()
            && (e.hole == Some(hole.id) || (e.deed == deed && e.object == victim && time::day(e.tick) == day))
    };
    for pool in world.rumours.iter_mut() {
        let mut hit = false;
        for e in pool.entries.iter_mut().filter(|e| matches(e)) {
            e.actor = Some(actor);
            hit = true;
        }
        if hit {
            dedupe(&mut pool.entries);
        }
    }
    // Held anonymous rumours of it take the name too: one walk of the heard
    // stores, only when such a rumour was ever handed out (`anon_heard`).
    if !world.anon_heard.remove(&(deed, hole.victim, day)) {
        if hole.kind == HoleKind::Killed {
            watch_killer(world, actor);
        }
        return;
    }
    // scan-ok: per bind that has an anonymous rumour out, never per tick.
    for id in world.citizens() {
        let Some(m) = world.comp_mut::<Memory>(id) else { continue };
        for e in m.heard.iter_mut() {
            if e.kind == MemoryKind::Rumour
                && e.subject.is_none()
                && e.deed == Some(deed)
                && e.object == victim
                && time::day(e.tick) == day
            {
                e.subject = Some(actor);
            }
        }
    }
    if hole.kind == HoleKind::Killed {
        watch_killer(world, actor);
    }
}

/// Daily: forget `anon_heard` keys older than any hole can wait to bind.
pub fn prune_anon(world: &mut World) {
    let keep = world.config.bind.hole_ttl_days + 2;
    let today = world.day();
    world.anon_heard.retain(|&(_, _, d)| d + keep >= today);
}

/// W6 (`bind::open_hole`): an off-screen crime goes into the hole's
/// district's pool unnamed, carrying the hole id.
pub fn post_hole(world: &mut World, hole: &Hole) {
    if !world.config.gossip.enabled {
        return;
    }
    let Some(deed) = deed_of_hole(hole.kind) else { return };
    let d = if hole.district.is_unset() { DistrictId(0) } else { hole.district };
    let reach = world.config.gossip.reach0.get(deed);
    let e = PoolEntry {
        deed,
        actor: None,
        object: Some(hole.victim),
        tick: hole.tick,
        hops: 0,
        reach,
        hole: Some(hole.id),
        story: None,
        kin: SmallVec::new(),
        told: SmallVec::new(),
        district: d,
    };
    post(world, d, e);
}

// ---------------------------------------------------------------------------
// The exchange (W7, W8)
// ---------------------------------------------------------------------------

/// Where an exchange happens.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Venue {
    Chat,
    Drink,
    Ask,
}

/// The speaker's knowledge (W8): its social skill, 0.5 for everyone while
/// moves are off (`moves::knowledge`).
pub fn knowledge(world: &World, id: EntityId) -> f32 {
    crate::systems::moves::knowledge(world, id)
}

/// W8: one exchange, `from` telling `to`. The pick: of the speaker's deed
/// memories (`entries ∪ heard`) with salience ≥ `gossip_min`, hops <
/// `max_hops` and an actor that is not the listener, the max of `weight ×
/// conf × novelty` (novelty 0.3 when the listener already holds it), ties
/// the lower actor index, then the older tick. The listener gets a Rumour
/// at hops + 1, salience × `hop_salience`, conf × (0.5 + 0.5 × trust in the
/// speaker), with the distortion roll; on the Exchange stream keyed by
/// `(tick, from, to)`. While `legacy_second_hand` no edge changes.
pub fn exchange(world: &mut World, from: EntityId, to: EntityId, venue: Venue) {
    let _ = venue;
    if !world.config.gossip.enabled || from == to {
        return;
    }
    let (Some(ms), Some(ml)) = (world.comp::<Memory>(from), world.comp::<Memory>(to)) else { return };
    let g = &world.config.gossip;
    let now = world.tick;
    let half_life = world.config.brain.memory_half_life_days;
    // The listener's deeds once (≤ 32), not one walk per candidate.
    let known: SmallVec<[(DeedRef, u64); 32]> = memory::deeds(to, ml).map(|(x, xr)| (xr, time::day(x.tick))).collect();
    let mut best: Option<(f32, u32, Tick, MemoryEntry, DeedRef)> = None;
    for (e, r) in memory::deeds(from, ms) {
        if e.salience < g.gossip_min || memory::hops_of(e) >= g.max_hops || r.actor == Some(to) {
            continue;
        }
        let day = time::day(e.tick);
        let novelty = if known.iter().any(|&(k, kd)| k == r && kd == day) { 0.3 } else { 1.0 };
        let score = memory::weight(e, now, half_life) * e.conf * novelty;
        let key = r.actor.map_or(u32::MAX, |a| a.index);
        let better = best
            .as_ref()
            .is_none_or(|&(s, k, t, _, _)| score > s || (score == s && (key < k || (key == k && e.tick < t))));
        if better {
            best = Some((score, key, e.tick, e.clone(), r));
        }
    }
    let Some((_, _, _, src, mut r)) = best else { return };
    let trust = world.edge(to, from).map_or(0.0, |e| e.trust);
    let hop_salience = g.hop_salience;
    let legacy = g.legacy_second_hand;
    let mut rng = world.rng.word(WordNs::Exchange, now, (u64::from(from.index) << 32) | u64::from(to.index));
    let d = talk_district(world, from);
    distort(world, from, &mut r, d, &mut rng);
    // W8 (phase 2): a speaker telling of their own deed lies with `p =
    // deception × 0.5`, naming an Enemy instead (one more draw on the same
    // stream; a lie the listener may later meet as a contradiction).
    if r.actor == Some(from) && crate::systems::moves::on(world) {
        lie(world, from, &mut r, d, &mut rng);
    }
    let entry = MemoryEntry {
        subject: r.actor,
        salience: src.salience * hop_salience,
        valence: src.valence * hop_salience,
        second_hand: true,
        deed: Some(r.deed),
        object: r.object,
        hops: memory::hops_of(&src).saturating_add(1),
        conf: src.conf * (0.5 + 0.5 * trust),
        press: src.press,
        ..MemoryEntry::blank(MemoryKind::Rumour, src.tick)
    };
    let hops = entry.hops;
    let out = memory::hear_entry(world, to, entry);
    if let Some(log) = world.shadow_notes.as_mut() {
        let heard = matches!(out, HeardInsert::Inserted | HeardInsert::Contradicted);
        log.push(crate::word::ShadowNote::Told { tick: now, from, to, r, heard });
    }
    if matches!(out, HeardInsert::Inserted | HeardInsert::Contradicted) {
        let w = &mut world.stats.current.word;
        w.rumours_heard += 1;
        w.rumour_hops_max = w.rumour_hops_max.max(u32::from(hops));
        // W9: from phase 3 the listener's existing edge to the actor takes
        // the hit; never a new edge.
        if !legacy {
            if let Some(a) = r.actor {
                if world.edge(to, a).is_some() {
                    let sev = world.config.gossip.deed_sev.get(r.deed);
                    crate::systems::social::adjust(world, to, a, -0.1 * sev, 0.0);
                }
            }
        }
    }
}

/// W7: at Drink completion, the co-drinker in the Bar with an edge to the
/// drinker and the highest affinity (ties the lower id; no draw) trades one
/// exchange each way.
pub fn drink(world: &mut World, drinker: EntityId) {
    if !world.config.gossip.enabled {
        return;
    }
    let Some(bar) = world.comp::<Position>(drinker).and_then(|p| p.building) else { return };
    let Some(b) = world.comp::<crate::components::Building>(bar) else { return };
    let partner = b
        .occupants
        .iter()
        .copied()
        .filter(|&o| o != drinker && world.has::<Brain>(o) && world.has::<Memory>(o))
        .filter(|&o| !world.has::<crate::components::Sentence>(o))
        .filter_map(|o| world.edge(drinker, o).map(|e| (o, e.affinity)))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
        .map(|(o, _)| o);
    if let Some(o) = partner {
        exchange(world, drinker, o, Venue::Drink);
        exchange(world, o, drinker, Venue::Drink);
    }
}

/// W8: the distortion roll of one telling, `p = distort_base × (1 −
/// knowledge)`: the actor becomes the actor's gang, else an Enemy of the
/// speaker whose Home is in the deed's district (lowest id), else stays.
/// Counts `distorted`; true on a swap.
pub fn distort(world: &mut World, speaker: EntityId, r: &mut DeedRef, d: DistrictId, rng: &mut ChaCha8Rng) -> bool {
    let k = knowledge(world, speaker);
    let swapped = distort_with(world, speaker, r, d, rng, k);
    if swapped {
        world.stats.current.word.distorted += 1;
    }
    swapped
}

/// W8 (phase 2): the speaker's lie about its own deed: with `p = deception
/// × 0.5` the actor becomes an Enemy of the speaker, the one whose Home is
/// in the deed's district first, else the lowest id. Counts `distorted`.
pub fn lie(world: &mut World, speaker: EntityId, r: &mut DeedRef, d: DistrictId, rng: &mut ChaCha8Rng) -> bool {
    let p = 0.5 * world.comp::<crate::components::Skills>(speaker).map_or(0.0, |s| s.deception);
    let roll: f32 = rng.random();
    if roll >= p {
        return false;
    }
    let object = r.object;
    let candidate = |e: &EntityId| *e != speaker && Some(*e) != object;
    let local = world.enemies_of(speaker).filter(candidate).find(|&e| home_district(world, e) == Some(d));
    let Some(swap) = local.or_else(|| world.enemies_of(speaker).find(candidate)) else { return false };
    r.actor = Some(swap);
    world.stats.current.word.distorted += 1;
    true
}

/// `distort` at a given speaker knowledge (no counter): at knowledge 1 it
/// never fires.
pub fn distort_with(
    world: &World,
    speaker: EntityId,
    r: &mut DeedRef,
    d: DistrictId,
    rng: &mut ChaCha8Rng,
    knowledge: f32,
) -> bool {
    let p = world.config.gossip.distort_base * (1.0 - knowledge).clamp(0.0, 1.0);
    let roll: f32 = rng.random();
    if roll >= p {
        return false;
    }
    let Some(actor) = r.actor else { return false };
    let swap = world.comp::<GangMember>(actor).map(|g| g.gang).filter(|&g| g != actor).or_else(|| {
        world.enemies_of(speaker).filter(|&e| e != actor && e != speaker).find(|&e| home_district(world, e) == Some(d))
    });
    match swap {
        Some(s) => {
            r.actor = Some(s);
            true
        }
        None => false,
    }
}

// ---------------------------------------------------------------------------
// The daily pass (W10, W11)
// ---------------------------------------------------------------------------

/// W11 step 1: every entry's reach × `pool_decay`; an entry whose reach
/// was ≥ `leak_min` is posted to each adjacent district at that reach ×
/// `leak_frac`, hops + 1 (a leaked copy carries no kin); below `reach_min`
/// an entry is dropped.
pub fn decay_and_leak(world: &mut World) {
    let g = world.config.gossip.clone();
    let mut leaks: Vec<(usize, PoolEntry)> = Vec::new();
    for (d, pool) in world.rumours.iter_mut().enumerate() {
        let adj = world.district_adjacent.get(d).copied().unwrap_or(0);
        for e in pool.entries.iter_mut() {
            let before = e.reach;
            e.reach *= g.pool_decay;
            if before >= g.leak_min && adj != 0 {
                for n in 0..16usize {
                    if adj & (1 << n) != 0 && n != d {
                        let mut c = e.clone();
                        c.reach = before * g.leak_frac;
                        c.hops = e.hops.saturating_add(1);
                        c.kin = SmallVec::new();
                        c.told = SmallVec::new();
                        leaks.push((n, c));
                    }
                }
            }
        }
        pool.entries.retain(|e| e.reach >= g.reach_min);
    }
    for (n, c) in leaks {
        if n < world.rumours.len() {
            post(world, DistrictId(n as u8), c);
        }
    }
}

/// The district a Statistical agent heard talk in: yesterday's trace
/// entry, else its Home's.
fn hearing_district(world: &World, id: EntityId, yesterday: Option<u64>) -> Option<DistrictId> {
    yesterday
        .and_then(|y| world.comp::<Trace>(id).and_then(|t| t.on_day(y)))
        .map(|t| t.district)
        .filter(|d| !d.is_unset())
        .or_else(|| home_district(world, id))
}

/// A Rumour heard off a pool entry.
fn pool_rumour(world: &World, e: &PoolEntry, hops: u8, conf: f32, salience: f32) -> MemoryEntry {
    let sev = world.config.gossip.deed_sev.get(e.deed);
    MemoryEntry {
        subject: e.actor,
        salience,
        valence: -sev * salience,
        second_hand: true,
        deed: Some(e.deed),
        object: e.object,
        hops,
        conf,
        ..MemoryEntry::blank(MemoryKind::Rumour, e.tick)
    }
}

/// W11 step 2 (spec "Hearing"), plus W39: each Statistical adult, in id
/// order, draws once on its Hear stream with `p = hear_p × (0.5 +
/// sociability)` from its district's pool, the entry picked by reach; it
/// holds a Rumour at hops + 1, `conf = pool_conf`, `salience = deed_sal ×
/// reach`. Then its own first-hand deeds with salience ≥ `gossip_min` that
/// the pool lacks go back in at `reach0 × 0.5`. A Full or Coarse adult
/// draws once from story entries only (none before phase 4).
pub fn hear(world: &mut World) {
    let g = world.config.gossip.clone();
    let day = world.day();
    let yesterday = day.checked_sub(1);
    let stat: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
    for id in stat {
        if !crate::systems::demography::is_adult(world, id) || !world.has::<Memory>(id) {
            continue;
        }
        let Some(d) = hearing_district(world, id, yesterday) else { continue };
        let di = d.index();
        if di >= world.rumours.len() {
            continue;
        }
        let soc = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
        let p = g.hear_p * (0.5 + soc);
        let mut rng = world.rng.word(WordNs::Hear, day, u64::from(id.index));
        if rng.random::<f32>() < p {
            let pool = &world.rumours[di].entries;
            let total: f32 = pool.iter().map(|e| e.reach).sum();
            if total > 0.0 {
                let u = rng.random::<f32>() * total;
                let mut acc = 0.0;
                let mut pick = pool.len() - 1;
                for (i, e) in pool.iter().enumerate() {
                    acc += e.reach;
                    if u < acc {
                        pick = i;
                        break;
                    }
                }
                let e = pool[pick].clone();
                let entry =
                    pool_rumour(world, &e, e.hops.saturating_add(1), g.pool_conf, g.deed_sal.get(e.deed) * e.reach);
                let hops = entry.hops;
                let out = memory::hear_entry(world, id, entry);
                if matches!(out, HeardInsert::Inserted | HeardInsert::Contradicted) {
                    let w = &mut world.stats.current.word;
                    w.rumours_heard += 1;
                    w.rumour_hops_max = w.rumour_hops_max.max(u32::from(hops));
                }
            }
        }
        // Post-back: off-screen talk goes back into the pool.
        let backs: Vec<PoolEntry> = world
            .comp::<Memory>(id)
            .map(|m| {
                m.entries
                    .iter()
                    .filter(|e| !e.second_hand && e.salience >= g.gossip_min)
                    .filter_map(|e| memory::deed_of(id, e).map(|r| (e, r)))
                    .map(|(e, r)| PoolEntry {
                        deed: r.deed,
                        actor: r.actor,
                        object: r.object,
                        tick: e.tick,
                        hops: memory::hops_of(e),
                        reach: g.reach0.get(r.deed) * 0.5,
                        hole: None,
                        story: None,
                        kin: SmallVec::new(),
                        told: SmallVec::new(),
                        district: d,
                    })
                    .collect()
            })
            .unwrap_or_default();
        for b in backs {
            if !world.rumours[di].entries.iter().any(|x| same_key(x, &b)) {
                post(world, d, b);
            }
        }
    }
    // W39: bodies read the Feeds (story entries only).
    if world.rumours.iter().any(|p| p.entries.iter().any(|e| e.story.is_some())) {
        let mut bodies: Vec<EntityId> = world.tier(Lod::Full).to_vec();
        bodies.extend_from_slice(world.tier(Lod::Coarse));
        bodies.sort_unstable();
        for id in bodies {
            if !crate::systems::demography::is_adult(world, id) || !world.has::<Memory>(id) {
                continue;
            }
            let Some(d) = hearing_district(world, id, yesterday) else { continue };
            let di = d.index();
            if di >= world.rumours.len() {
                continue;
            }
            let soc = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
            let p = g.hear_p * (0.5 + soc);
            let mut rng = world.rng.word(WordNs::Hear, day, u64::from(id.index));
            if rng.random::<f32>() >= p {
                continue;
            }
            let stories: Vec<&PoolEntry> = world.rumours[di].entries.iter().filter(|e| e.story.is_some()).collect();
            let total: f32 = stories.iter().map(|e| e.reach).sum();
            if total <= 0.0 {
                continue;
            }
            let u = rng.random::<f32>() * total;
            let mut acc = 0.0;
            let mut pick = stories.len() - 1;
            for (i, e) in stories.iter().enumerate() {
                acc += e.reach;
                if u < acc {
                    pick = i;
                    break;
                }
            }
            let e = stories[pick].clone();
            let entry = pool_rumour(world, &e, e.hops.saturating_add(1), g.pool_conf, g.deed_sal.get(e.deed) * e.reach);
            if matches!(memory::hear_entry(world, id, entry), HeardInsert::Inserted | HeardInsert::Contradicted) {
                world.stats.current.word.rumours_heard += 1;
            }
        }
    }
}

/// One pool entry's kin-channel work: (pool, entry index, the deed, its
/// tick, its hops, its hash, the kin still to tell).
type KinWork = (usize, usize, DeedRef, Tick, u8, u64, SmallVec<[EntityId; 8]>);

/// W10: the kin channel. For each pool entry first posted in this district
/// with deed Killed or Assaulted and a named actor, each living kin not yet
/// holding it hears it with `p = kin_p[min(hops, 3)]` on the Kin stream,
/// at any tier, at hops `max(hops, 1)`, conf 1. Killed: the kin captured at
/// death; Assaulted: the living victim's, read the same way (`kin_cap`) at
/// the entry's first pass and kept on the entry. A kin told, found holding
/// it, dead or gone goes on the entry's `told` list and is not rolled
/// again. O(Σ kin) a day.
pub fn kin(world: &mut World) {
    let g = world.config.gossip.clone();
    let day = world.day();
    // An Assaulted entry's kin are read off the living victim's edges once,
    // at its first pass, and kept on the entry like a killing's.
    let mut fill: Vec<(usize, usize, SmallVec<[EntityId; 8]>)> = Vec::new();
    for (d, pool) in world.rumours.iter().enumerate() {
        for (i, e) in pool.entries.iter().enumerate() {
            if e.district.index() == d
                && e.deed == Deed::Assaulted
                && e.actor.is_some()
                && e.kin.is_empty()
                && e.told.is_empty()
            {
                if let Some(o) = e.object {
                    fill.push((d, i, assault_kin(world, o)));
                }
            }
        }
    }
    for (d, i, k) in fill {
        world.rumours[d].entries[i].kin = k;
    }
    let mut work: Vec<KinWork> = Vec::new();
    for (d, pool) in world.rumours.iter().enumerate() {
        for (i, e) in pool.entries.iter().enumerate() {
            if e.district.index() != d || e.actor.is_none() || !matches!(e.deed, Deed::Killed | Deed::Assaulted) {
                continue;
            }
            let todo: SmallVec<[EntityId; 8]> = e.kin.iter().copied().filter(|k| !e.told.contains(k)).collect();
            if todo.is_empty() {
                continue;
            }
            let hash = splitmix64(
                (e.deed.index() as u64)
                    ^ (u64::from(e.actor.map_or(0, |a| a.index)) << 8)
                    ^ (u64::from(e.object.map_or(0, |o| o.index)) << 32)
                    ^ e.tick.rotate_left(17),
            );
            work.push((d, i, e.deed_ref(), e.tick, e.hops, hash, todo));
        }
    }
    for (d, i, r, tick, hops, hash, todo) in work {
        let p = g.kin_p[usize::from(hops.min(3))];
        let sal = g.deed_sal.get(r.deed);
        for k in todo {
            // The dead, the gone and the actor themself are done with; a
            // holder is done; a failed roll waits for tomorrow.
            let done = match world.comp::<Memory>(k).filter(|_| world.has::<Brain>(k) && Some(k) != r.actor) {
                None => true,
                Some(m) if memory::holds_deed(k, m, &r, tick) => true,
                Some(_) => {
                    let mut rng = world.rng.word(WordNs::Kin, day, (u64::from(k.index) << 20) ^ hash);
                    let told = rng.random::<f32>() < p;
                    if told {
                        let sev = g.deed_sev.get(r.deed);
                        let entry = MemoryEntry {
                            subject: r.actor,
                            salience: sal,
                            valence: -sev * sal,
                            second_hand: true,
                            deed: Some(r.deed),
                            object: r.object,
                            hops: hops.max(1),
                            conf: 1.0,
                            ..MemoryEntry::blank(MemoryKind::Rumour, tick)
                        };
                        let h = entry.hops;
                        if matches!(
                            memory::hear_entry(world, k, entry),
                            HeardInsert::Inserted | HeardInsert::Contradicted
                        ) {
                            let w = &mut world.stats.current.word;
                            w.rumours_heard += 1;
                            w.rumour_hops_max = w.rumour_hops_max.max(u32::from(h));
                        }
                    }
                    told
                }
            };
            if done {
                if let Some(x) = world.rumours.get_mut(d).and_then(|p| p.entries.get_mut(i)) {
                    x.told.push(k);
                }
            }
        }
    }
}

/// A living victim's kin as `kin_of` reads a killing's: Spouse, Parents,
/// Family, then Friends by affinity, at most `kin_cap` (plan deviation:
/// W10 read every Friend live, and a Statistical victim carries hundreds of
/// Friend edges: one beating told ~500 people and cost the kin pass 10+ ms
/// a day by day 100).
fn assault_kin(world: &World, victim: EntityId) -> SmallVec<[EntityId; 8]> {
    if !crate::systems::law::living(world, victim) {
        return SmallVec::new();
    }
    kin_of(world, victim)
}

/// W2 (plan deviation: trimmed at the demotion, not at the next insert,
/// so no Statistical agent ever holds more than its cap): a demoted agent
/// drops its Sightings and keeps its `rumour_cap_statistical` strongest
/// (`weight × conf`) rumours. Touches `heard` only.
pub fn on_demoted(world: &mut World, id: EntityId) {
    let cap = world.config.gossip.rumour_cap_statistical;
    let (now, half_life) = (world.tick, world.config.brain.memory_half_life_days);
    let Some(m) = world.comp_mut::<Memory>(id) else { return };
    if m.heard.is_empty() {
        return;
    }
    m.heard.retain(|e| e.kind != MemoryKind::Sighting);
    while m.heard.len() > cap {
        let worst = m
            .heard
            .iter()
            .enumerate()
            .map(|(i, x)| (i, memory::weight(x, now, half_life) * x.conf))
            .fold((0, f32::INFINITY), |acc, (i, w)| if w < acc.1 { (i, w) } else { acc })
            .0;
        m.heard.swap_remove(worst);
    }
}

/// Drop every held Sighting older than `[db] sighting_days`.
pub fn expire_sightings(world: &mut World) {
    let horizon = world.tick.saturating_sub(Tick::from(world.config.db.sighting_days) * TICKS_PER_DAY);
    // scan-ok: daily (the word's midnight chain).
    for id in world.citizens() {
        let Some(m) = world.comp_mut::<Memory>(id) else { continue };
        if m.heard.iter().any(|e| e.kind == MemoryKind::Sighting && e.tick < horizon) {
            m.heard.retain(|e| e.kind != MemoryKind::Sighting || e.tick >= horizon);
        }
    }
}

// ---------------------------------------------------------------------------
// Sightings (W12): memory only in phase 1; the relay to `FactionDb` is phase 3
// ---------------------------------------------------------------------------

/// W12: does `observer` care where `who` is? `who` is the target of an
/// unsettled grudge of the observer, an Enemy, wanted, or (phase 3) a
/// member of a faction in an open vendetta with the observer's gang. O(1)
/// (a grudge list of at most 4, a set lookup, the few vendettas).
pub fn wants_sighting(world: &World, observer: EntityId, who: EntityId) -> bool {
    if observer == who {
        return false;
    }
    let grudge = crate::systems::grudges::holds(world, observer, who, 0.0);
    grudge
        || world.enemies.get(&observer).is_some_and(|s| s.contains(&who))
        || crate::systems::law::wanted(world, who)
        || in_vendetta(world, observer, who)
}

/// Are `a`'s gang and `b`'s gang in an open vendetta?
fn in_vendetta(world: &World, a: EntityId, b: EntityId) -> bool {
    if world.vendettas.is_empty() {
        return false;
    }
    let (Some(ga), Some(gb)) = (world.gang_of(a), world.gang_of(b)) else { return false };
    world.vendettas.iter().any(|v| (v.a, v.b) == (ga, gb) || (v.a, v.b) == (gb, ga))
}

/// W12: at a co-location event, a body that cares about `who` notes where
/// it saw them (a heard `Sighting`, refreshed in place). A Statistical
/// observer has no eyes. Phase 3 relays it: a gang member's to its gang's
/// `FactionDb`, a guard's to the city's, at `conf × relay_conf`.
pub fn maybe_sight(world: &mut World, observer: EntityId, who: EntityId, at: Option<EntityId>, tile: TilePos) {
    if !world.config.gossip.enabled {
        return;
    }
    if world.comp::<Brain>(observer).is_none_or(|b| b.lod == Lod::Statistical) || !world.has::<Memory>(observer) {
        return;
    }
    if !wants_sighting(world, observer, who) {
        return;
    }
    let entry =
        MemoryEntry { subject: Some(who), salience: 0.5, at, ..MemoryEntry::blank(MemoryKind::Sighting, world.tick) };
    let conf = entry.conf;
    memory::hear_entry(world, observer, entry);
    // W12 relay (phase 3).
    let owner = if let Some(g) = world.gang_of(observer) {
        Some(Some(g))
    } else if crate::systems::law::is_city_guard(world, observer) {
        Some(None)
    } else {
        None
    };
    if let Some(owner) = owner {
        let s = crate::virt::Sighting {
            who,
            tile,
            tick: world.tick,
            confidence: conf * world.config.gossip.relay_conf,
            relayed: true,
        };
        relay_sighting(world, owner.unwrap_or(EntityId::NONE), s);
    }
}

/// W12 relay (plan deviation): one relayed sighting per person in a
/// faction's database (the newer replaces the older, at the back), and
/// past `[db] db_cap` the oldest relayed one goes before any trace or
/// camera sighting, so the eyes on the street never push M14's caught
/// runners out of a 32-entry database.
fn relay_sighting(world: &mut World, owner: EntityId, s: crate::virt::Sighting) {
    let cap = world.config.db.db_cap.max(1);
    let db = world.db.entry(owner).or_default();
    if let Some(i) = db.sightings.iter().position(|x| x.relayed && x.who == s.who) {
        db.sightings.remove(i);
    }
    db.sightings.push_back(s);
    while db.sightings.len() > cap {
        let n = db.sightings.len();
        let i = db.sightings.iter().take(n - 1).position(|x| x.relayed).unwrap_or(0);
        db.sightings.remove(i);
    }
}
