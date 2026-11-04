use super::*;

// ---------------------------------------------------------------------------
// M10: lives (the biography's events)
// ---------------------------------------------------------------------------

/// What happened to an agent, as the Story tab tells it. Written only by
/// `events::record_life` and `events::life_bound`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum LifeKind {
    Born,
    Married,
    Widowed,
    Hired,
    Fired,
    Quit,
    Paid,
    Starving,
    Robbed,
    Assaulted,
    Stole,
    RobbedSomeone,
    AssaultedSomeone,
    Killed,
    KilledSomeone,
    Died,
    Arrested,
    Released,
    Escaped,
    JoinedGang,
    LeftGang,
    Betrayed,
    Evicted,
    Housed,
    Immigrated,
    Buried,
    Witnessed,
    /// M11: opened a business on a Lot (phase 4).
    Founded,
    /// M11: became the exec of their own corp (phase 4).
    Incorporated,
    /// M14 V14: died in a node's ICE (`other` = the node's owner).
    Flatlined,
    /// Real economy E38: taken from the Home by the child protective
    /// service (`other` = a parent).
    Taken,
    /// Real economy E38: a child taken from this parent (`other` = the child).
    ChildTaken,
    /// Real economy E41: released from a work camp at adulthood.
    CampRaised,
}

impl LifeKind {
    /// Never evicted from the list.
    pub fn permanent(self) -> bool {
        matches!(
            self,
            LifeKind::Born
                | LifeKind::Married
                | LifeKind::Widowed
                | LifeKind::Killed
                | LifeKind::KilledSomeone
                | LifeKind::Died
                | LifeKind::Flatlined
        )
    }

    /// The eviction weight a fresh entry of this kind starts with.
    pub fn salience(self) -> f32 {
        match self {
            LifeKind::Born
            | LifeKind::Married
            | LifeKind::Widowed
            | LifeKind::Killed
            | LifeKind::KilledSomeone
            | LifeKind::Died
            | LifeKind::Flatlined => 1.0,
            LifeKind::Betrayed
            | LifeKind::RobbedSomeone
            | LifeKind::AssaultedSomeone
            | LifeKind::Founded
            | LifeKind::Incorporated => 0.9,
            // Real economy E38, E41: a childhood taken is as heavy as a betrayal.
            LifeKind::Taken | LifeKind::ChildTaken | LifeKind::CampRaised => 0.9,
            LifeKind::Robbed | LifeKind::Assaulted | LifeKind::Arrested | LifeKind::Escaped | LifeKind::JoinedGang => {
                0.7
            }
            LifeKind::Fired | LifeKind::LeftGang | LifeKind::Evicted => 0.6,
            LifeKind::Hired | LifeKind::Starving | LifeKind::Stole | LifeKind::Released | LifeKind::Immigrated => 0.5,
            LifeKind::Quit | LifeKind::Housed => 0.4,
            LifeKind::Buried | LifeKind::Witnessed => 0.3,
            LifeKind::Paid => 0.2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifeEvent {
    pub tick: Tick,
    pub kind: LifeKind,
    pub other: Option<EntityId>,
    /// An open victim hole: "by an unknown assailant" until it is bound.
    pub hole: Option<HoleId>,
    pub salience: f32,
}

/// An agent's biography: at most `LIFE_CAP` events, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Life {
    pub events: Vec<LifeEvent>,
}

pub const LIFE_CAP: usize = 48;
