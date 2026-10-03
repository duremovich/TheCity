//! Action vocabulary and plans. The action table (costs, preconditions,
//! effects) is implemented in M3.

use serde::{Deserialize, Serialize};

use crate::components::{ActionInstance, GoalKind};
use crate::entity::EntityId;
use crate::goap::world_state::LocationKey;
use crate::time::Tick;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum StealSource {
    Market,
    Home,
    Warehouse,
}

/// Enum order is the tie-break order (lower variant wins on equal f-cost).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ActionKind {
    GoTo(LocationKey),
    EatFromInventory,
    EatAtHome,
    BuyFood,
    StealFood(StealSource),
    Forage,
    Beg,
    Sleep,
    Rest,
    FarmWork,
    HaulToMarket,
    ClerkWork,
    BartendWork,
    CollectWage,
    SellFood,
    Fence,
    Chat,
    Drink,
    Flirt,
    Propose,
    ReportCrime,
    PatrolLeg,
    Arrest,
    Escort,
    HideFromLaw,
    FleeToHome,
    Attack,
    JoinGang,
    Extort,
    SplitLoot,
    BuryCorpse,
    CarryCorpse,
    Wander,
    CollectDole,
    GuardJail,
    /// The forced plan of a sentenced agent; never chosen by the planner.
    ServeTime,
    /// At Home: move carried (unstolen) food into the pantry. Dur 5. In the
    /// Building table but missing from the spec's enum.
    StoreFood,
    /// Gravedigger's shift with no corpse to bury: at the Cemetery, runs to
    /// shift end like GuardJail. Not in the spec; needed so the role is paid.
    TendGraves,
}

impl ActionKind {
    /// The on-shift action for a role.
    pub fn work_for(role: crate::components::Role) -> ActionKind {
        use crate::components::Role;
        match role {
            Role::Farmer => ActionKind::FarmWork,
            Role::Guard => ActionKind::GuardJail,
            Role::Clerk => ActionKind::ClerkWork,
            Role::Bartender => ActionKind::BartendWork,
            Role::Gravedigger => ActionKind::TendGraves,
        }
    }

    pub fn is_work(self) -> bool {
        matches!(
            self,
            ActionKind::FarmWork
                | ActionKind::GuardJail
                | ActionKind::ClerkWork
                | ActionKind::BartendWork
                | ActionKind::TendGraves
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub goal: GoalKind,
    pub target: Option<EntityId>,
    pub steps: Vec<ActionInstance>,
    pub started_tick: Tick,
}
