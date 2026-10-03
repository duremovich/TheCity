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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub goal: GoalKind,
    pub target: Option<EntityId>,
    pub steps: Vec<ActionInstance>,
    pub started_tick: Tick,
}
