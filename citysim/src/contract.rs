//! M16a types (docs/M16_CONTRACTS.md § 1-3; plan C1, C2, C4): the
//! contract record, the Fixer's book, a mission, a taker's chase, the
//! strike terms and the keyed-stream namespaces.
//!
//! Everything here is game state between fictional agents of a simulated
//! city: a `Contract` is a record in `World::contracts` (buyer, kind,
//! target, price, deadline, broker, taker, status); "Hit", "Beat", "Guard"
//! and "Locate" are enum variants naming what a record asks for; fulfilment
//! is the existing seeded dice roll (`law::resolve_fight`, a ledger draw, a
//! sighting written to a `FactionDb`); escrow is coins moved between purses
//! by `ownership`. The behaviour lives in `systems::contracts` and
//! `systems::missions`. 16a names no 16b type: every enum here is appended
//! to by M16b (RON names variants, so a 16a save loads in 16b).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::components::{CorpOrder, Order, TilePos};
use crate::entity::EntityId;
use crate::time::Tick;
use crate::word::{HuntPhase, Intel};

/// `World::next_contract`, monotonic from 1, saved.
pub type ContractId = u64;
/// A mission is keyed by its contract.
pub type MissionId = ContractId;

/// What a contract record asks for (a game abstraction: a label on a
/// record resolved by a dice roll). M16b appends its ten kinds.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ContractKind {
    /// A record whose fulfilment is the target's death by the taker's hand
    /// (a `resolve_fight` with `kill_p`, or a ledger draw).
    Hit,
    /// A record whose fulfilment is a fight won by the taker.
    Beat,
    /// A record whose fulfilment is a client (agent or building) unharmed
    /// at the deadline while the taker stands its post.
    Guard,
    /// A record that pays per sighting of the target written to the
    /// buyer's `FactionDb`.
    Locate,
}

impl ContractKind {
    pub const ALL: [ContractKind; 4] =
        [ContractKind::Hit, ContractKind::Beat, ContractKind::Guard, ContractKind::Locate];

    /// Lower-case, as the config tables and the CLI spell it.
    pub fn label(self) -> &'static str {
        match self {
            ContractKind::Hit => "hit",
            ContractKind::Beat => "beat",
            ContractKind::Guard => "guard",
            ContractKind::Locate => "locate",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.to_ascii_lowercase();
        ContractKind::ALL.iter().copied().find(|k| k.label() == s)
    }

    /// Position in [`ContractKind::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// The law files on its fulfilment (a Murder or an Assault).
    pub fn illegal(self) -> bool {
        matches!(self, ContractKind::Hit | ContractKind::Beat)
    }

    /// Fulfilled through a fight roll.
    pub fn violent(self) -> bool {
        matches!(self, ContractKind::Hit | ContractKind::Beat)
    }

    /// Fulfilled by a death.
    pub fn lethal(self) -> bool {
        self == ContractKind::Hit
    }
}

/// Who or what a record names. M16b appends `Node`, `Carry`, `Seat`, `Story`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Target {
    Agent(EntityId),
    Building(EntityId),
}

impl Target {
    /// The entity named.
    pub fn id(self) -> EntityId {
        match self {
            Target::Agent(a) | Target::Building(a) => a,
        }
    }
}

/// The record's terms. M16b appends `Coerced(CoercionId)`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Terms {
    Pay { price: i64 },
}

/// The status machine's states (`contracts::set_status` is the one writer).
/// M16b appends `Standing`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ContractStatus {
    Open,
    Taken,
    Fulfilled,
    Failed,
    Expired,
    Reneged,
    Cancelled,
}

impl ContractStatus {
    /// `Open` or `Taken`: escrow may still be held.
    pub fn is_live(self) -> bool {
        matches!(self, ContractStatus::Open | ContractStatus::Taken)
    }

    pub fn label(self) -> &'static str {
        match self {
            ContractStatus::Open => "open",
            ContractStatus::Taken => "taken",
            ContractStatus::Fulfilled => "fulfilled",
            ContractStatus::Failed => "failed",
            ContractStatus::Expired => "expired",
            ContractStatus::Reneged => "reneged",
            ContractStatus::Cancelled => "cancelled",
        }
    }
}

/// Which system wrote the record. M16b appends `Court, Bury, Spin, Board,
/// Subscription`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Origin {
    Hunt,
    GangOrder(Order),
    CorpOrder(CorpOrder),
    Law,
    God,
}

/// How a taken record is resolved: a ledger draw at `due` (no bodies), a
/// live chase by bodies, or (M18) the player.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Render {
    #[default]
    Ledger,
    Live,
    Played,
}

impl Render {
    pub fn label(self) -> &'static str {
        match self {
            Render::Ledger => "LEDGER",
            Render::Live => "LIVE",
            Render::Played => "PLAYED",
        }
    }
}

/// The strike decision's choices (phase 2 fills them; C24).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Decision {
    Strike,
    Hold,
    SellOut,
    Fail,
}

/// The last strike decision's numbers, for the Mission panel (C24).
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StrikeTerms {
    pub p_win: f32,
    /// `Controller::csv_code` of the door's district.
    pub controller: u8,
    pub political: f32,
    pub ev: f32,
    pub decision: Decision,
}

/// One contract record (spec § 1 plus the plan's fields, C1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub id: ContractId,
    /// An M11 owner (an agent, a gang, a corp); `None` = the city or the law.
    pub buyer: Option<EntityId>,
    /// The person who placed it (the buyer, an exec, a leader, the captain).
    pub agent: Option<EntityId>,
    pub kind: ContractKind,
    pub target: Target,
    /// The terms as posted. The price paid is `price` (the one source: a
    /// sell-out raises `price` only).
    pub terms: Terms,
    /// Coins held in escrow (inside `ownership::total_coins`); 0 when direct.
    pub escrow: i64,
    /// The Fixer building, `None` for a direct record.
    pub broker: Option<EntityId>,
    pub posted: Tick,
    pub deadline: Tick,
    pub origin: Origin,
    pub status: ContractStatus,
    /// An agent, a gang or a corp.
    pub taker: Option<EntityId>,
    pub crew: SmallVec<[EntityId; 4]>,
    pub attempts: u8,
    /// `1 − p_win` of the median gun at posting.
    pub risk: f32,
    /// The strike's political term, at posting and refreshed at the strike.
    pub political: f32,
    /// Decided at posting (direct records only), hidden.
    pub renege: bool,
    /// The ledger resolution tick.
    pub due: Option<Tick>,
    pub render: Render,
    /// Who knows the buyer is behind it (the `Hired` first-hand holders).
    pub known_by: SmallVec<[EntityId; 8]>,
    // --- plan fields (C1), each `#[serde(default)]`.
    /// The listed price (escrowed or not), the one every reader uses.
    #[serde(default)]
    pub price: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taken: Option<Tick>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed: Option<Tick>,
    #[serde(default)]
    pub paid_sightings: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_paid: Option<Tick>,
    /// The last strike decision (phase 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<StrikeTerms>,
    #[serde(default)]
    pub holds: u8,
    #[serde(default)]
    pub sold_out: bool,
    /// C29: the placing agent was charged for it (phase 3).
    #[serde(default)]
    pub charged: bool,
    /// C28: arrest ticks already used for an interrogation (phase 3).
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub interrogated: SmallVec<[Tick; 2]>,
    /// Plan field: the tick a `Guard` record was taken, for the client
    /// building's `loss_log` check at the deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_since: Option<Tick>,
    /// Phase 4 review: the tick the record was first worked live (its run
    /// or mission opened, or a city guard went on the take); `None` while
    /// it sits queued. A Guard never started by its deadline expires
    /// (refunded, the taker unpaid) instead of being paid for a post it
    /// never stood.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<Tick>,
}

impl Contract {
    /// `Open`: on the board, untaken.
    pub fn is_open(&self) -> bool {
        self.status == ContractStatus::Open
    }

    /// `Open` or `Taken`.
    pub fn is_live(&self) -> bool {
        self.status.is_live()
    }

    /// Placed through a Fixer (escrowed).
    pub fn brokered(&self) -> bool {
        self.broker.is_some()
    }

    pub fn target_agent(&self) -> Option<EntityId> {
        match self.target {
            Target::Agent(a) => Some(a),
            Target::Building(_) => None,
        }
    }
}

/// A squad's march (phase 2 fills it; C22).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mission {
    pub contract: ContractId,
    /// The taker first.
    pub crew: SmallVec<[EntityId; 6]>,
    pub muster: TilePos,
    pub door: TilePos,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door_building: Option<EntityId>,
    pub raid_at: Tick,
    /// The M14 stream (deferred in 16a: always `None`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_by: Option<EntityId>,
    #[serde(default)]
    pub defenders_hint: u8,
}

/// A Fixer's record book (a component on the Fixer building, C2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Broker {
    /// Open and taken records, ascending.
    pub book: Vec<ContractId>,
    /// The last `Network` visit (or Statistical regular stamp).
    pub regulars: BTreeMap<EntityId, Tick>,
    /// Plan: gangs whose members networked here.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub gangs: BTreeSet<EntityId>,
    /// `fixer_cut` at founding.
    pub cut: f32,
    /// `0..=1` (phase 3).
    pub heat: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_until: Option<Tick>,
    /// `FixerCut` per day, the last 14, newest last.
    pub income: VecDeque<i64>,
    /// Plan: today's `FixerCut`.
    #[serde(default)]
    pub income_today: i64,
}

/// A live taker's chase (the Hunt's intel fields, C14): `LocationKey::Intel`
/// resolves from it through `hunt::chase`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContractRun {
    pub contract: ContractId,
    pub target: EntityId,
    pub since: Tick,
    pub phase: HuntPhase,
    pub venue: Option<EntityId>,
    pub intel: Option<Intel>,
    pub stakeout_until: Option<Tick>,
    #[serde(default)]
    pub deceived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liar: Option<EntityId>,
}

/// The one entry point's argument (`contracts::post`).
#[derive(Clone, Debug, PartialEq)]
pub struct Posting {
    pub buyer: Option<EntityId>,
    pub agent: Option<EntityId>,
    pub kind: ContractKind,
    pub target: Target,
    pub broker: Option<EntityId>,
    pub deadline_days: u16,
    pub origin: Origin,
    /// A god's price override.
    pub price: Option<i64>,
}

/// Why `contracts::post` refused.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    Disabled,
    MaxOpen,
    CannotPay,
    NoTarget,
    Duplicate,
    BrokerClosed,
}

/// The keyed-stream namespaces of M16a's rolls (C4, `SimRng::contract`):
/// a nibble in bits 56-59 of a stream on `rng::CONTRACT_KEY`. 10 of 15
/// left for M16b and M17.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ContractNs {
    /// The renege draw at posting (direct records), key `(id, 0)`.
    Renege = 1,
    /// The ledger's due tick, key `(id, 0)`.
    Due,
    /// The ledger's outcome, key `(id, attempts)`.
    Outcome,
    /// The ledger's taker death, key `(id, attempts)`.
    TakerDeath,
    /// The Fixer's talk (phase 3), key `(tick, from << 32 | to)`.
    Talk,
}

/// C38: per-Fixer CSV slots (`f0_heat` … `f3_income`), Fixers by building id.
pub const FIXER_SLOTS: usize = 4;
