//! The event log: a ring of 50,000 entries, never drained. The UI keeps a
//! read cursor; every emergent event appends exactly one entry. Each entry
//! has a contiguous id (M10), so the binder can rewrite the entry of a hole
//! it attributes in O(1).

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::components::{
    hole_id, Bound, Corpse, DeathCause, Hole, HoleId, HoleKind, Identity, Life, LifeEvent, LifeKind, LIFE_CAP,
};
use crate::entity::EntityId;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

pub const EVENT_RING_CAP: usize = 50_000;
/// `PlanAborted` goes to its own small ring, `World::debug_events`, not saved
/// (M10 phase 5b): at 2,000 residents it was ~1,000 a day and pushed every
/// story event out of the 50,000 within two months.
pub const DEBUG_RING_CAP: usize = 2_000;
/// The `id` of an event in the debug ring: it takes no place in the story
/// ring's contiguous numbering.
pub const DEBUG_EVENT_ID: u64 = u64::MAX;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Contiguous over the run (M10 D12); `0` in a pre-M10 save until
    /// `World::migrate_legacy` renumbers the ring.
    #[serde(default)]
    pub id: u64,
    pub tick: Tick,
    pub kind: EventKind,
    pub actors: SmallVec<[EntityId; 3]>,
    pub text: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum EventKind {
    Theft,
    Extortion,
    Assault,
    Murder,
    Witness,
    Report,
    Arrest,
    Sentence,
    Release,
    Unpunished,
    Birth,
    Death,
    Burial,
    Rotted,
    Proposal,
    Marriage,
    GangJoin,
    GangLeave,
    Betrayal,
    Hire,
    Fire,
    Quit,
    Immigration,
    Emigration,
    Starving,
    Homeless,
    PlanAborted,
    PriceChange,
    Restock,
    Inheritance,
    PlayerAction,
    PlayerActionFailed,
    /// M8 factions.
    OrderChanged,
    TerritoryFlipped,
    Raid,
    Disobeyed,
    Sacked,
    /// M9 the law.
    Jailbreak,
    Posture,
    Bribe,
    /// M10 off-screen lives: victim-side crimes whose actor is a hole until
    /// bound (`actors[0]` is `EntityId::NONE` until then), and the binding.
    Robbed,
    Assaulted,
    Attributed,
    /// M11 ownership and corps (`[evictee, home, owner?]`, `[agent, home]`, ...).
    Evicted,
    RentShort,
    Housed,
    Founded,
    Incorporated,
    Bankrupt,
    Acquired,
    BrokenUp,
    CorpOrder,
    Strike,
    Contract,
}

impl EventKind {
    pub const ALL: [EventKind; 54] = [
        EventKind::Theft,
        EventKind::Extortion,
        EventKind::Assault,
        EventKind::Murder,
        EventKind::Witness,
        EventKind::Report,
        EventKind::Arrest,
        EventKind::Sentence,
        EventKind::Release,
        EventKind::Unpunished,
        EventKind::Birth,
        EventKind::Death,
        EventKind::Burial,
        EventKind::Rotted,
        EventKind::Proposal,
        EventKind::Marriage,
        EventKind::GangJoin,
        EventKind::GangLeave,
        EventKind::Betrayal,
        EventKind::Hire,
        EventKind::Fire,
        EventKind::Quit,
        EventKind::Immigration,
        EventKind::Emigration,
        EventKind::Starving,
        EventKind::Homeless,
        EventKind::PlanAborted,
        EventKind::PriceChange,
        EventKind::Restock,
        EventKind::Inheritance,
        EventKind::PlayerAction,
        EventKind::PlayerActionFailed,
        EventKind::OrderChanged,
        EventKind::TerritoryFlipped,
        EventKind::Raid,
        EventKind::Disobeyed,
        EventKind::Sacked,
        EventKind::Jailbreak,
        EventKind::Posture,
        EventKind::Bribe,
        EventKind::Robbed,
        EventKind::Assaulted,
        EventKind::Attributed,
        EventKind::Evicted,
        EventKind::RentShort,
        EventKind::Housed,
        EventKind::Founded,
        EventKind::Incorporated,
        EventKind::Bankrupt,
        EventKind::Acquired,
        EventKind::BrokenUp,
        EventKind::CorpOrder,
        EventKind::Strike,
        EventKind::Contract,
    ];
}

impl World {
    /// Append one event at the current tick, evicting the oldest past the cap.
    /// Returns the event's id.
    /// `PlanAborted` goes to the debug ring instead and returns `DEBUG_EVENT_ID`.
    pub fn push_event(&mut self, kind: EventKind, actors: &[EntityId], text: impl Into<String>) -> u64 {
        if kind == EventKind::PlanAborted {
            if self.debug_events.len() >= DEBUG_RING_CAP {
                self.debug_events.pop_front();
            }
            let actors = SmallVec::from_slice(actors);
            self.debug_events.push_back(Event { id: DEBUG_EVENT_ID, tick: self.tick, kind, actors, text: text.into() });
            return DEBUG_EVENT_ID;
        }
        if self.events.len() >= EVENT_RING_CAP {
            self.events.pop_front();
        }
        let id = self.next_event_id;
        self.next_event_id += 1;
        let event = Event { id, tick: self.tick, kind, actors: SmallVec::from_slice(actors), text: text.into() };
        record_life(self, &event);
        self.events.push_back(event);
        id
    }

    /// The ring entry with this id, if it has not been evicted. O(1): ids
    /// are contiguous in the ring.
    pub fn event_mut(&mut self, id: u64) -> Option<&mut Event> {
        let first = self.events.front()?.id;
        let i = usize::try_from(id.checked_sub(first)?).ok()?;
        self.events.get_mut(i).filter(|e| e.id == id)
    }

    /// Events whose `actors` contain `id`, newest first.
    pub fn events_for(&self, id: EntityId) -> impl Iterator<Item = &Event> {
        self.events.iter().rev().filter(move |e| e.actors.contains(&id))
    }
}

// ---------------------------------------------------------------------------
// Lives (M10 phase 4): the one table that writes `Life`
// ---------------------------------------------------------------------------

/// What `life_kind` says an event did to the agent in `slot`.
struct LifeRow {
    kind: LifeKind,
    other: Option<EntityId>,
    hole: Option<HoleId>,
}

/// The table (M10 D36). Actor order per push site:
/// `Birth [child, mother, father]`, `Marriage [a, b]`, `Death [dead, spouse?]`,
/// `Death [dead, spouse or NONE, killer]` when the killer is known,
/// `Hire [id, employer]`, `Fire/Quit/Starving/Theft/Release/Homeless/Immigration/Betrayal [id, ..]`,
/// `Robbed/Assaulted [NONE, victim]`, `Assault/Murder [attacker, victim]`,
/// `Extortion [actor, home]`, `Arrest [guard, suspect]`, `Jailbreak [gang, jail, freed..]`,
/// `GangJoin/GangLeave [id, gang]`, `Burial [digger, corpse]`, `Witness [witness, actor]`.
fn life_kind(world: &World, event: &Event, slot: usize, actor: EntityId) -> Option<LifeRow> {
    use EventKind as E;
    let other = event.actors.iter().copied().find(|&o| o != actor && o != EntityId::NONE && world.has::<Identity>(o));
    let row = |kind| Some(LifeRow { kind, other, hole: None });
    let victim_side = |kind: LifeKind, hole_kind: HoleKind| {
        // A victim-side event whose actor is still `NONE` is a hole.
        let hole =
            event.actors.first().is_some_and(|a| *a == EntityId::NONE).then(|| hole_id(event.tick, actor, hole_kind));
        Some(LifeRow { kind, other, hole })
    };
    match (event.kind, slot) {
        (E::Birth, 0) => row(LifeKind::Born),
        (E::Marriage, _) => row(LifeKind::Married),
        (E::Death, 0) => {
            let violent = world.comp::<Corpse>(actor).is_some_and(|c| c.cause == DeathCause::Violence);
            // The killer, when `kill_by` named one.
            let killer = event.actors.get(2).copied().filter(|&k| k != EntityId::NONE && violent);
            Some(LifeRow { kind: if violent { LifeKind::Killed } else { LifeKind::Died }, other: killer, hole: None })
        }
        (E::Death, 1) => row(LifeKind::Widowed),
        (E::Hire, 0) => row(LifeKind::Hired),
        (E::Fire, 0) => row(LifeKind::Fired),
        (E::Quit, 0) => row(LifeKind::Quit),
        (E::Starving, 0) => row(LifeKind::Starving),
        (E::Theft, 0) => row(LifeKind::Stole),
        (E::Robbed, 1) => victim_side(LifeKind::Robbed, HoleKind::Robbed),
        (E::Assaulted, 1) => victim_side(LifeKind::Assaulted, HoleKind::Assaulted),
        (E::Assault, 0) => row(LifeKind::AssaultedSomeone),
        (E::Assault, 1) => row(LifeKind::Assaulted),
        (E::Murder, 0) => row(LifeKind::KilledSomeone),
        (E::Murder, 1) => victim_side(LifeKind::Killed, HoleKind::Killed),
        (E::Extortion, 0) => row(LifeKind::RobbedSomeone),
        (E::Arrest, 1) => row(LifeKind::Arrested),
        (E::Release, 0) => row(LifeKind::Released),
        // The gang and the Jail come first; the freed are the rest.
        (E::Jailbreak, s) if s >= 2 => row(LifeKind::Escaped),
        (E::GangJoin, 0) => row(LifeKind::JoinedGang),
        (E::GangLeave, 0) => row(LifeKind::LeftGang),
        (E::Betrayal, 0) => row(LifeKind::Betrayed),
        (E::Homeless, 0) => row(LifeKind::Evicted),
        // M11 (plan D39): `Evicted [evictee, home, owner?]`, `Housed [agent,
        // home]`, `Founded [founder, ..]`, `Incorporated [corp, exec]`.
        (E::Evicted, 0) => row(LifeKind::Evicted),
        (E::Housed, 0) => row(LifeKind::Housed),
        (E::Founded, 0) => row(LifeKind::Founded),
        (E::Incorporated, 1) => row(LifeKind::Incorporated),
        // M11 phase 3: corp events name corps and buildings; no biography row.
        (E::CorpOrder | E::Bankrupt | E::Acquired | E::BrokenUp | E::Contract, _) => None,
        (E::Immigration, 0) => row(LifeKind::Immigrated),
        (E::Burial, 1) => row(LifeKind::Buried),
        (E::Witness, 0) => row(LifeKind::Witnessed),
        _ => None,
    }
}

/// Append the event to the biography of every agent in `actors` that the
/// table has a row for. Per event, never per tick.
fn record_life(world: &mut World, event: &Event) {
    for (slot, &actor) in event.actors.iter().enumerate() {
        if actor == EntityId::NONE || !world.has::<Identity>(actor) {
            continue;
        }
        let Some(row) = life_kind(world, event, slot, actor) else { continue };
        let entry = LifeEvent {
            tick: event.tick,
            kind: row.kind,
            other: row.other,
            hole: row.hole,
            salience: row.kind.salience(),
        };
        add_life(world, actor, entry);
    }
}

/// Close a hole's biography entries (M10 D36): the victim's entry stops
/// reading "unknown" and names the actor; the actor's gains the crime at its
/// original tick with salience 1.0, so it is never evicted.
pub fn life_bound(world: &mut World, hole: &Hole, bound: Bound) {
    let actor = match bound {
        Bound::Actor(a) => Some(a),
        Bound::Unknown => None,
    };
    if let Some(life) = world.comp_mut::<Life>(hole.victim) {
        for e in life.events.iter_mut().filter(|e| e.hole == Some(hole.id)) {
            e.hole = None;
            e.other = actor;
        }
    }
    if let Some(a) = actor {
        let kind = match hole.kind {
            HoleKind::Robbed => LifeKind::RobbedSomeone,
            HoleKind::Assaulted => LifeKind::AssaultedSomeone,
            HoleKind::Killed => LifeKind::KilledSomeone,
        };
        add_life(world, a, LifeEvent { tick: hole.tick, kind, other: Some(hole.victim), hole: None, salience: 1.0 });
    }
}

/// Insert in tick order; a permanent kind already recorded this tick is
/// completed rather than doubled (a Death and its Murder both say Killed);
/// past `LIFE_CAP` the lowest `salience × recency` evictable entry goes.
fn add_life(world: &mut World, id: EntityId, entry: LifeEvent) {
    if !world.has::<Life>(id) {
        world.insert(id, Life::default());
    }
    let now = world.tick;
    let half_life = world.config.brain.memory_half_life_days.max(0.01);
    let Some(life) = world.comp_mut::<Life>(id) else { return };
    if entry.kind.permanent() {
        if let Some(e) = life.events.iter_mut().find(|e| e.kind == entry.kind && e.tick == entry.tick) {
            e.other = e.other.or(entry.other);
            e.hole = e.hole.or(entry.hole);
            return;
        }
    }
    if life.events.len() >= LIFE_CAP {
        let weight = |e: &LifeEvent| {
            let age_days = now.saturating_sub(e.tick) as f32 / TICKS_PER_DAY as f32;
            e.salience * 0.5f32.powf(age_days / half_life)
        };
        // Lowest weight; the oldest on a tie (the list is oldest first).
        let worst = life.events.iter().enumerate().filter(|(_, e)| !e.kind.permanent() && e.salience < 1.0).fold(
            None::<(usize, f32)>,
            |acc, (i, e)| {
                let w = weight(e);
                match acc {
                    Some((_, best)) if best <= w => acc,
                    _ => Some((i, w)),
                }
            },
        );
        if let Some((i, _)) = worst {
            life.events.remove(i);
        }
        // Nothing evictable: every entry is permanent or a bound crime, so the list grows.
    }
    let at = life.events.partition_point(|e| e.tick <= entry.tick);
    life.events.insert(at, entry);
}
