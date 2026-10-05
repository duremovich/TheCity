//! The event log: a ring of 5,000 entries, never drained. The UI keeps a
//! read cursor; every emergent event appends exactly one entry.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::entity::EntityId;
use crate::time::Tick;
use crate::world::World;

pub const EVENT_RING_CAP: usize = 5_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
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
}

impl EventKind {
    pub const ALL: [EventKind; 40] = [
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
    ];
}

impl World {
    /// Append one event at the current tick, evicting the oldest past the cap.
    pub fn push_event(&mut self, kind: EventKind, actors: &[EntityId], text: impl Into<String>) {
        if self.events.len() >= EVENT_RING_CAP {
            self.events.pop_front();
        }
        self.events.push_back(Event { tick: self.tick, kind, actors: SmallVec::from_slice(actors), text: text.into() });
    }

    /// Events whose `actors` contain `id`, newest first.
    pub fn events_for(&self, id: EntityId) -> impl Iterator<Item = &Event> {
        self.events.iter().rev().filter(move |e| e.actors.contains(&id))
    }
}
