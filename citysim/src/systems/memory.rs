//! Memory: a bounded, decaying list of salient events per agent.
//!
//! `remember` merges rapid duplicates and evicts the lowest
//! `salience × recency` entry past the cap; the daily decay halves salience
//! every seven days and drops entries under `MEMORY_DROP_BELOW`.

use crate::components::{Brain, Crime, Lod, Memory, MemoryEntry, MemoryKind};
use crate::entity::EntityId;
use crate::time::{self, Tick, TICKS_PER_DAY};
use crate::word::{Deed, DeedRef};
use crate::world::World;

/// Entries below this salience are forgotten.
pub const MEMORY_DROP_BELOW: f32 = 0.05;
/// Same kind and subject within this many ticks merge into one entry.
pub const MERGE_WINDOW: Tick = 60;

/// `salience × 0.5^(age_days / half_life)`.
pub fn weight(e: &MemoryEntry, now: Tick, half_life_days: f32) -> f32 {
    let age_days = now.saturating_sub(e.tick) as f32 / TICKS_PER_DAY as f32;
    e.salience * 0.5f32.powf(age_days / half_life_days.max(0.01))
}

/// Insert an entry into a memory with the spec's merge and eviction rules.
/// Returns false when it merged into an entry already there (M15 W15: a
/// grudge forms on a new memory only).
pub fn insert(mem: &mut Memory, entry: MemoryEntry, now: Tick, cap: usize, half_life_days: f32) -> bool {
    if let Some(e) = mem.entries.iter_mut().find(|e| {
        e.kind == entry.kind && e.subject == entry.subject && entry.tick.saturating_sub(e.tick) < MERGE_WINDOW
    }) {
        e.salience = e.salience.max(entry.salience);
        e.tick = entry.tick;
        return false;
    }
    if mem.entries.len() >= cap {
        let (worst, _) = mem
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i, weight(e, now, half_life_days)))
            .fold((0, f32::INFINITY), |acc, (i, w)| if w < acc.1 { (i, w) } else { acc });
        mem.entries.swap_remove(worst);
    }
    mem.entries.push(entry);
    true
}

/// Daily decay at `tick_of_day == 0`: `salience ×= 0.5^(1/7)`; entries under
/// 0.05 are removed. Runs before any system that could hit the cap.
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    let half_life = world.config.brain.memory_half_life_days.max(0.01);
    let factor = 0.5f32.powf(1.0 / half_life);
    for id in world.citizens() {
        let Some(m) = world.comp_mut::<Memory>(id) else { continue };
        for e in &mut m.entries {
            e.salience *= factor;
        }
        m.entries.retain(|e| e.salience >= MEMORY_DROP_BELOW);
        // M15 W2: the heard store decays by the same factor.
        if !m.heard.is_empty() {
            for e in &mut m.heard {
                e.salience *= factor;
            }
            m.heard.retain(|e| e.salience >= MEMORY_DROP_BELOW);
        }
    }
}

/// First-hand memories of `kind` about `subject`.
pub fn has_first_hand(world: &World, who: EntityId, kind: MemoryKind, subject: Option<EntityId>) -> bool {
    world
        .comp::<Memory>(who)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == kind && e.subject == subject && !e.second_hand))
}

// ---------------------------------------------------------------------------
// M15 phase 1: deeds read off memories, and the heard store (plan W1-W4)
// ---------------------------------------------------------------------------

/// W3: a memory read as a deed. `holder` is the memory's owner (a
/// `WasRobbed`'s object is the holder). First-hand: `SawCrime` by its crime
/// (Murder Killed, Assault Assaulted, Theft and Grand Theft Robbed,
/// Extortion Extorted, the rest none), `WasRobbed` Robbed, `Lost`
/// Assaulted (`Fought` alone is not a deed), `Stripped`, `Grief` Killed
/// with no actor, `Evicted` (actor the owner); `Rumour` and `Threatened`
/// by their fields.
pub fn deed_of(holder: EntityId, e: &MemoryEntry) -> Option<DeedRef> {
    let r = |deed, actor, object| Some(DeedRef { deed, actor, object });
    match e.kind {
        MemoryKind::SawCrime => {
            let deed = match e.crime? {
                Crime::Murder => Deed::Killed,
                Crime::Assault => Deed::Assaulted,
                Crime::Theft | Crime::GrandTheft => Deed::Robbed,
                Crime::Extortion => Deed::Extorted,
                Crime::Vagrancy
                | Crime::Manslaughter
                | Crime::Dealing
                | Crime::Abduction
                | Crime::Intrusion
                | Crime::DataTheft
                | Crime::Conspiracy => return None,
            };
            r(deed, e.subject, e.object)
        }
        MemoryKind::WasRobbed => r(Deed::Robbed, e.subject, Some(holder)),
        MemoryKind::Lost => r(Deed::Assaulted, e.subject, Some(holder)),
        MemoryKind::Stripped => r(Deed::Stripped, e.subject, e.object),
        MemoryKind::Grief => r(Deed::Killed, None, e.subject),
        MemoryKind::Evicted => r(Deed::Evicted, e.subject, Some(holder)),
        MemoryKind::Rumour | MemoryKind::Threatened => r(e.deed?, e.subject, e.object),
        _ => None,
    }
}

/// The hops a memory stands at: its field, and 1 for a legacy second-hand
/// entry (`social::gossip`'s copies in `entries`).
pub fn hops_of(e: &MemoryEntry) -> u8 {
    e.hops.max(u8::from(e.second_hand))
}

/// Every deed memory of a holder, `entries` then `heard`.
pub fn deeds<'a>(holder: EntityId, m: &'a Memory) -> impl Iterator<Item = (&'a MemoryEntry, DeedRef)> + 'a {
    m.entries.iter().chain(m.heard.iter()).filter_map(move |e| deed_of(holder, e).map(|d| (e, d)))
}

/// What `insert_heard` did.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HeardInsert {
    Inserted,
    Merged,
    Contradicted,
    Dropped,
}

/// W4: two deeds are the same telling when deed, object and day agree (an
/// objectless deed also needs the same actor, see `insert_heard`).
fn same_deed(a: &DeedRef, at: Tick, b: &DeedRef, bt: Tick) -> bool {
    a.deed == b.deed && a.object == b.object && time::day(at) == time::day(bt)
}

/// Does the holder already hold this deed (same deed, object, day and
/// actor) in `entries ∪ heard`?
pub fn holds_deed(holder: EntityId, m: &Memory, r: &DeedRef, tick: Tick) -> bool {
    deeds(holder, m).any(|(x, xr)| same_deed(&xr, x.tick, r, tick) && xr.actor == r.actor)
}

/// W2/W4: put a Rumour or a Sighting into the holder's `heard` store,
/// never touching `entries`. A deed merges with a matching deed (same deed,
/// object and day) in `entries ∪ heard`: a first-hand match with the same
/// actor gains nothing (a holder who saw it is not told it); a heard match
/// keeps the max conf and salience and the min hops, and an anonymous one
/// takes the name; a different named actor lowers both confidences by
/// `contradict` and the new entry still goes in. A Sighting refreshes the
/// holder's sighting of the same person. Past `cap` the lowest `weight ×
/// conf` heard entry goes if it is weaker than the new one, else the new
/// one is dropped (a demoted holder is trimmed here, never eagerly).
/// Plan deviation: the holder is a parameter (a `WasRobbed`'s object is
/// the holder, so `deed_of` needs it).
pub fn insert_heard(
    m: &mut Memory,
    holder: EntityId,
    mut e: MemoryEntry,
    now: Tick,
    cap: usize,
    half_life: f32,
    contradict: f32,
) -> HeardInsert {
    if e.kind == MemoryKind::Sighting {
        if let Some(x) = m.heard.iter_mut().find(|x| x.kind == MemoryKind::Sighting && x.subject == e.subject) {
            if e.tick >= x.tick {
                x.tick = e.tick;
                x.at = e.at;
            }
            x.conf = x.conf.max(e.conf);
            x.salience = x.salience.max(e.salience);
            return HeardInsert::Merged;
        }
    } else if let Some(r) = deed_of(holder, &e) {
        // First-hand: the same named deed is already known.
        let seen = m
            .entries
            .iter()
            .any(|x| deed_of(holder, x).is_some_and(|xr| same_deed(&xr, x.tick, &r, e.tick) && xr.actor == r.actor));
        if seen {
            return HeardInsert::Merged;
        }
        let mut contradicted = false;
        for x in m.heard.iter_mut() {
            let Some(xr) = deed_of(holder, x) else { continue };
            if !same_deed(&xr, x.tick, &r, e.tick) {
                continue;
            }
            // Plan deviation (W4): only a killing contradicts (a death has
            // one killer); two named actors of any other deed are two deeds
            // (a brawl's raiders all beat the same defender), and a deed
            // with no object names nobody to tell tellings apart.
            let named_apart = matches!((xr.actor, r.actor), (Some(a), Some(b)) if a != b);
            if named_apart && (r.deed != Deed::Killed || r.object.is_none()) {
                continue;
            }
            match (xr.actor, r.actor) {
                (Some(a), Some(b)) if a != b => {
                    x.conf = (x.conf - contradict).max(0.0);
                    contradicted = true;
                    continue;
                }
                (None, Some(b)) => x.subject = Some(b),
                _ => {}
            }
            x.conf = x.conf.max(e.conf);
            x.salience = x.salience.max(e.salience);
            x.hops = x.hops.min(e.hops);
            if e.press != 0 && x.press == 0 {
                x.press = e.press;
            }
            return HeardInsert::Merged;
        }
        if contradicted {
            e.conf = (e.conf - contradict).max(0.0);
            if e.conf <= 0.0 {
                return HeardInsert::Dropped;
            }
        }
        if push_heard(m, e, now, cap, half_life) {
            return if contradicted { HeardInsert::Contradicted } else { HeardInsert::Inserted };
        }
        return HeardInsert::Dropped;
    }
    if push_heard(m, e, now, cap, half_life) {
        HeardInsert::Inserted
    } else {
        HeardInsert::Dropped
    }
}

/// The cap rule of `insert_heard`: true if `e` went in.
fn push_heard(m: &mut Memory, e: MemoryEntry, now: Tick, cap: usize, half_life: f32) -> bool {
    let value = |x: &MemoryEntry| weight(x, now, half_life) * x.conf;
    if cap == 0 {
        return false;
    }
    while m.heard.len() >= cap {
        let (worst, w) = m
            .heard
            .iter()
            .enumerate()
            .map(|(i, x)| (i, value(x)))
            .fold((0, f32::INFINITY), |acc, (i, w)| if w < acc.1 { (i, w) } else { acc });
        if m.heard.len() == cap && w >= value(&e) {
            return false;
        }
        m.heard.swap_remove(worst);
    }
    m.heard.push(e);
    true
}

/// W2: the heard cap of an agent: `rumour_cap` on screen,
/// `rumour_cap_statistical` off it.
pub fn heard_cap(world: &World, id: EntityId) -> usize {
    let g = &world.config.gossip;
    if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
        g.rumour_cap_statistical
    } else {
        g.rumour_cap
    }
}

/// `insert_heard` on an agent through the world (its cap, the half-life,
/// `[moves] contradict_conf`), counting contradictions and noting every
/// unnamed rumour in `World::anon_heard`. A Statistical agent takes no
/// Sighting (W2).
pub fn hear_entry(world: &mut World, id: EntityId, e: MemoryEntry) -> HeardInsert {
    if e.kind == MemoryKind::Sighting && world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
        return HeardInsert::Dropped;
    }
    let cap = heard_cap(world, id);
    let half_life = world.config.brain.memory_half_life_days;
    let contradict = world.config.moves.contradict_conf;
    let now = world.tick;
    let anon = (e.kind == MemoryKind::Rumour && e.subject.is_none())
        .then_some(e.deed.zip(e.object))
        .flatten()
        .map(|(d, o)| (d, o, time::day(e.tick)));
    // M15 W15: a heard deed that goes in may leave a grudge.
    let learn = (e.kind == MemoryKind::Rumour).then(|| deed_of(id, &e).map(|r| (r, e.conf, e.hops))).flatten();
    let Some(m) = world.comp_mut::<Memory>(id) else { return HeardInsert::Dropped };
    let out = insert_heard(m, id, e, now, cap, half_life, contradict);
    if let (Some((r, conf, hops)), HeardInsert::Inserted | HeardInsert::Contradicted) = (learn, out) {
        crate::systems::grudges::on_learn(world, id, &r, conf, hops);
    }
    // An unnamed rumour went in: a later bind renames held copies.
    if let (Some(k), HeardInsert::Inserted | HeardInsert::Contradicted) = (anon, out) {
        world.anon_heard.insert(k);
    }
    if out == HeardInsert::Contradicted {
        world.stats.current.word.contradicted += 1;
    }
    out
}
