//! The event log: a ring of 50,000 entries, never drained. The UI keeps a
//! read cursor; every emergent event appends exactly one entry. Each entry
//! has a contiguous id (M10), so the binder can rewrite the entry of a hole
//! it attributes in O(1).

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;

pub const EVENT_RING_CAP: usize = 50_000;

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
}

impl EventKind {
    pub const ALL: [EventKind; 43] = [
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
    ];
}

impl World {
    /// Append one event at the current tick, evicting the oldest past the cap.
    /// Returns the event's id.
    pub fn push_event(&mut self, kind: EventKind, actors: &[EntityId], text: impl Into<String>) -> u64 {
        if self.events.len() >= EVENT_RING_CAP {
            self.events.pop_front();
        }
        let id = self.next_event_id;
        self.next_event_id += 1;
        self.events.push_back(Event {
            id,
            tick: self.tick,
            kind,
            actors: SmallVec::from_slice(actors),
            text: text.into(),
        });
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
