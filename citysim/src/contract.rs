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
use crate::word::{Deed, HuntPhase, Intel};

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
    /// M16b (§ 4, phase 1): a record whose fulfilment is a social move by
    /// the taker on the target (a Charm toward the buyer for a Court
    /// suitor), resolved at any tier as one Move-stream draw.
    Persuade,
    /// M16b phase 2: a record whose fulfilment is the target dragged to a
    /// holding building and handed over (`HandOver`): a hostage or a
    /// Harvest's take.
    Abduct,
    /// M16b phase 2: a mission on a holding building; a win frees its
    /// holder's captives inside.
    Rescue,
    /// M16b phase 2: a bodyguard's post on an agent client (a `Guard` on a
    /// person, taken by agents and Security corps alike).
    Escort,
    /// M16b phase 2: a theft from a building, or, on a Lab with a node, a
    /// heist through a run order (`RunWhy::Contract`) whose Data goes to
    /// the buyer.
    Steal,
    /// M16b phase 2: a cargo carried to a building (a corpse to the
    /// Recycler).
    Deliver,
    /// M16b phase 2: a story planted on a Feed (fabricated or true).
    Spin,
    /// M16b phase 3 (spec § 5): a standing subscription (`Standing`) to a
    /// Security corp's Trauma crew, renewed weekly.
    Trauma,
    /// M16b phase 4 (spec § 3): a tracking tag planted on the target (its
    /// sightings to the buyer's database for the battery's days).
    Tag,
    /// M16b phase 4 (spec § 6): a board member leaned on to vote for the
    /// buyer (an Intimidate pinning the vote).
    Vote,
    /// M17 phase 3: a parent's branch takes a lost building back: a strike
    /// at the building (the mission's strength estimate, one ledger draw);
    /// a win forces the sale back to the branch at its value.
    Retake,
}

impl ContractKind {
    pub const ALL: [ContractKind; 15] = [
        ContractKind::Hit,
        ContractKind::Beat,
        ContractKind::Guard,
        ContractKind::Locate,
        ContractKind::Persuade,
        ContractKind::Abduct,
        ContractKind::Rescue,
        ContractKind::Escort,
        ContractKind::Steal,
        ContractKind::Deliver,
        ContractKind::Spin,
        ContractKind::Trauma,
        ContractKind::Tag,
        ContractKind::Vote,
        ContractKind::Retake,
    ];

    /// M16b phase 2's six kinds (`LeverageCols`' per-kind columns).
    pub const PHASE2: [ContractKind; 6] = [
        ContractKind::Abduct,
        ContractKind::Rescue,
        ContractKind::Escort,
        ContractKind::Steal,
        ContractKind::Deliver,
        ContractKind::Spin,
    ];

    /// M16a's four kinds: the `k_{kind}_*` CSV slots of `ContractCols` and
    /// the City panel's rows (M16b's kinds count in `LeverageCols`).
    pub const M16A: [ContractKind; 4] =
        [ContractKind::Hit, ContractKind::Beat, ContractKind::Guard, ContractKind::Locate];

    /// Lower-case, as the config tables and the CLI spell it.
    pub fn label(self) -> &'static str {
        match self {
            ContractKind::Hit => "hit",
            ContractKind::Beat => "beat",
            ContractKind::Guard => "guard",
            ContractKind::Locate => "locate",
            ContractKind::Persuade => "persuade",
            ContractKind::Abduct => "abduct",
            ContractKind::Rescue => "rescue",
            ContractKind::Escort => "escort",
            ContractKind::Steal => "steal",
            ContractKind::Deliver => "deliver",
            ContractKind::Spin => "spin",
            ContractKind::Trauma => "trauma",
            ContractKind::Tag => "tag",
            ContractKind::Vote => "vote",
            ContractKind::Retake => "retake",
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
        matches!(
            self,
            ContractKind::Hit
                | ContractKind::Beat
                | ContractKind::Abduct
                | ContractKind::Rescue
                | ContractKind::Steal
                | ContractKind::Tag
                | ContractKind::Vote
                | ContractKind::Retake
        )
    }

    /// M16b: worked by a fight (the risk and the nerve read the strike
    /// estimate): the violent kinds, an abduction and a rescue.
    pub fn fights(self) -> bool {
        matches!(self, ContractKind::Hit | ContractKind::Beat | ContractKind::Abduct | ContractKind::Rescue)
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

/// Who or what a record names. M16b phase 2 appends `Carry` and `Story`
/// (a heist on a node names its Lab: `Building`; the board's `Seat` is
/// phase 4's).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Target {
    Agent(EntityId),
    Building(EntityId),
    /// A Deliver: `what` carried to the building `to`.
    Carry {
        what: Cargo,
        to: EntityId,
    },
    /// A Spin: the deed a story names (`fabricated`: it never happened).
    Story {
        deed: Deed,
        actor: EntityId,
        object: Option<EntityId>,
        fabricated: bool,
    },
}

/// What a Deliver carries.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Cargo {
    Corpse(EntityId),
    Captive(EntityId),
}

impl Cargo {
    pub fn id(self) -> EntityId {
        match self {
            Cargo::Corpse(e) | Cargo::Captive(e) => e,
        }
    }
}

impl Target {
    /// The entity named: the agent, the building, the cargo, the story's actor.
    pub fn id(self) -> EntityId {
        match self {
            Target::Agent(a) | Target::Building(a) => a,
            Target::Carry { what, .. } => what.id(),
            Target::Story { actor, .. } => actor,
        }
    }
}

/// The record's terms. M16b appends `Coerced(CoercionId)`: a job done
/// under a threat, unpaid (`price` 0, no escrow).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Terms {
    Pay { price: i64 },
    Coerced(CoercionId),
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
    /// M16b phase 3: a Trauma subscription in force (not live: never
    /// matched, never on the ledger).
    Standing,
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
            ContractStatus::Standing => "standing",
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
    /// M16b phase 1: a suitor rejected twice buys a `Persuade` on the courted.
    Court,
    /// M16b phase 2: kin buy a corpse's carry to the Recycler.
    Bury,
    /// M16b phase 2: a spinning corp's plant (`Corp.spin_since`).
    Spin,
    /// M16b phase 3: a Trauma subscription.
    Subscription,
    /// M16b phase 4: a board campaign's Persuade or Vote.
    Board,
    /// M17 phase 3: an outside parent's order (a Retake, a Hit on a raider).
    Parent,
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
    /// The Hit market (docs/HIT_MARKET.md H4): the deposit put down when the
    /// record was posted on credit; 0 = paid (or owed) in full, the M16a
    /// record. The balance `price − deposit` is charged at fulfilment.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub deposit: i64,
    /// The Hit market (H13): why the buyer wanted it; `None` for M16a's
    /// records, the law's and the god's.
    #[serde(default, skip_serializing_if = "Motive::is_none")]
    pub motive: Motive,
    /// M16b phase 2: an Abduct's holding building and demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Hold>,
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

/// The Hit market (docs/HIT_MARKET.md H13): why a record was bought. A
/// label on a record, read by the event text, the stats and the brakes.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum Motive {
    /// M16a's records, the law's and the god's.
    #[default]
    None,
    /// A grudge holder pays for what he would not or could not do himself.
    Revenge,
    /// A suspect pays to be rid of a witness (phase 2).
    Silence,
    /// A creditor pays to collect (phase 2).
    Debt,
    /// A gang or corp buys a rival's head.
    Rival,
    /// A corp buys the culprit gang's leader (M16a's Lobby row, at the market).
    Culprit,
    /// A gang buys silence for its member (phase 2).
    Protect,
}

impl Motive {
    pub const ALL: [Motive; 7] =
        [Motive::None, Motive::Revenge, Motive::Silence, Motive::Debt, Motive::Rival, Motive::Culprit, Motive::Protect];

    pub fn is_none(&self) -> bool {
        *self == Motive::None
    }

    pub fn label(self) -> &'static str {
        match self {
            Motive::None => "none",
            Motive::Revenge => "revenge",
            Motive::Silence => "silence",
            Motive::Debt => "debt",
            Motive::Rival => "rival",
            Motive::Culprit => "culprit",
            Motive::Protect => "protect",
        }
    }
}

/// The Hit market (H18): how a rival corp wronged a corp (`Corp.last_wronged`).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Wrong {
    /// A hostile buyout of one of its buildings.
    Acquired,
    /// One of its staff poached.
    Poached,
    /// Undercut: a guard client moved, or a niche share taken.
    Undercut,
}

impl Wrong {
    pub fn label(self) -> &'static str {
        match self {
            Wrong::Acquired => "the buyout",
            Wrong::Poached => "the poaching",
            Wrong::Undercut => "the undercutting",
        }
    }
}

/// The Hit market (H8, H9; phase 2): an agent buyer at Full or Coarse who
/// would post with an open Fixer in reach walks there first: the want
/// waits in `World::wants` until the `Commission` step at `fixer` (or
/// lapses). Re-priced and re-paid at the Fixer; no coin moves before.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Want {
    pub target: EntityId,
    pub kind: ContractKind,
    pub motive: Motive,
    /// The motive's weight (the brakes' order and the goal's score).
    pub weight: f32,
    pub since: Tick,
    pub fixer: EntityId,
}

/// The Hit market (Debt, phase 2): a debt the Debt pass has seen standing
/// at `debt_min` or more since `seen` (`Edge.debt_since` is re-stamped
/// every fortnight by `social::daily`, so it cannot age a debt), and the
/// last Debt Beat fulfilled on the debtor (the Hit escalation's clock).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DebtClock {
    pub seen: Tick,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat: Option<Tick>,
}

/// The Hit market: what `contracts::post_with` adds to a `Posting`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PostExtra {
    /// H4: put this much down now (escrowed on the record, brokered or
    /// direct); the rest is owed at fulfilment. `None`: the full price.
    pub deposit: Option<i64>,
    pub motive: Motive,
    /// A few words for the event text (`"for Mara Voss"`).
    pub why: Option<String>,
}

impl Contract {
    /// The Hit market (H4): the part of the price owed at fulfilment.
    pub fn balance(&self) -> i64 {
        if self.deposit > 0 {
            (self.price - self.deposit).max(0)
        } else {
            0
        }
    }

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
            _ => None,
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
    /// The Hit market (H21): a relative's chip-in roll, key `(day, holder << 32 | kin)`.
    Want,
    /// The Hit market (H15, phase 2): a beaten witness recants.
    Recant,
    /// M16b phase 1: a leverage roll other than the move's own draw, with a
    /// purpose byte in `a`'s top bits (`leverage::Purpose`).
    Leverage,
}

/// C38: per-Fixer CSV slots (`f0_heat` … `f3_income`), Fixers by building id.
pub const FIXER_SLOTS: usize = 4;

// ---------------------------------------------------------------------------
// M16b § 4: leverage (phase 1)
// ---------------------------------------------------------------------------

/// `World::next_coercion`, monotonic from 1, saved.
pub type CoercionId = u64;

/// What a coercion holds over the coerced (a label on a record; the move
/// is one seeded Intimidate roll, `leverage::coerce`). M16b phase 2
/// appends `Hostage`.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Leverage {
    /// Credibility from might and dread (M15's Intimidate terms).
    Threat,
    /// A deed the holder knows about the coerced.
    Secret { deed: Deed, actor: EntityId, object: Option<EntityId>, day: u32 },
    /// An edge debt called in.
    Debt { amount: i64 },
    /// M16b phase 2: a captive the holder keeps.
    Hostage { captive: EntityId },
}

impl Leverage {
    /// The kind's slot (`[leverage] lev_bias`, the CSV's per-kind columns).
    pub fn kind(&self) -> LeverageKind {
        match self {
            Leverage::Threat => LeverageKind::Threat,
            Leverage::Secret { .. } => LeverageKind::Secret,
            Leverage::Debt { .. } => LeverageKind::Debt,
            Leverage::Hostage { .. } => LeverageKind::Hostage,
        }
    }
}

/// The leverage kinds, for tables and counters.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum LeverageKind {
    Threat,
    Secret,
    Debt,
    Hostage,
}

impl LeverageKind {
    pub const ALL: [LeverageKind; 4] =
        [LeverageKind::Threat, LeverageKind::Secret, LeverageKind::Debt, LeverageKind::Hostage];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            LeverageKind::Threat => "threat",
            LeverageKind::Secret => "secret",
            LeverageKind::Debt => "debt",
            LeverageKind::Hostage => "hostage",
        }
    }
}

/// What the holder demands (spec § 4). Phase 1 makes `Coins`, `Do`,
/// `Join` and `Silence`; `Sell`, `Vote`, `Release` and `Access` arrive
/// with their phases.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Demand {
    /// A shakedown, protection, hush money, a called debt, a tribute.
    Coins { amount: i64, weekly: bool },
    /// A coerced job (`Terms::Coerced`).
    Do(ContractId),
    /// Poaching by threat: take the post at the wage.
    Join { building: EntityId, wage: i64 },
    /// Withdraw testimony: the report naming `suspect` filed at `tick`
    /// (reports have no id) loses its witness.
    Silence { suspect: EntityId, tick: Tick },
    /// M16b phase 2: a building sold to the holder at its value.
    Sell(EntityId),
    /// M16b phase 2: a convict let out (the captain's to grant).
    Release(EntityId),
    /// M16b phase 4 (spec § 6): a board member's vote pinned for `for_`.
    Vote { corp: EntityId, for_: EntityId },
}

/// A coercion's state.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CoercionStatus {
    /// Standing: weekly coins due, or a coerced job under way.
    Active,
    /// The demand was met (a coerced job done).
    Complied,
    /// The coerced would not, or could not, pay a later week.
    Refused,
    /// It ended otherwise (the holder lost its hold, a party is gone).
    Broken,
    /// A blackmail whose secret went public.
    Lapsed,
}

impl CoercionStatus {
    pub fn label(self) -> &'static str {
        match self {
            CoercionStatus::Active => "active",
            CoercionStatus::Complied => "complied",
            CoercionStatus::Refused => "refused",
            CoercionStatus::Broken => "broken",
            CoercionStatus::Lapsed => "lapsed",
        }
    }
}

/// Why a standing coercion was made (the weekly pass and the counters
/// read it).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CoercionWhy {
    #[default]
    Other,
    /// A gang's weekly take from a business in a district it holds.
    Protection,
    /// Hush money over a secret.
    Blackmail,
    /// A gang's weekly tribute for peace after a Parley.
    Parley,
    /// A job done under a threat.
    Job,
    /// M16b phase 2: a captive held for a demand.
    Hostage,
    /// M16b phase 4: a board member's vote pinned by a campaign.
    Vote,
}

impl CoercionWhy {
    pub fn label(self) -> &'static str {
        match self {
            CoercionWhy::Other => "other",
            CoercionWhy::Protection => "protection",
            CoercionWhy::Blackmail => "blackmail",
            CoercionWhy::Parley => "tribute",
            CoercionWhy::Job => "job",
            CoercionWhy::Hostage => "hostage",
            CoercionWhy::Vote => "vote",
        }
    }
}

/// One coercion (spec § 4): a record of who holds what over whom and for
/// what; every payment is coins moved by `ownership::pay` (`Flow::Coerced`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Coercion {
    pub id: CoercionId,
    /// The owner the demand pays: an agent, a gang or a corp.
    pub holder: Option<EntityId>,
    /// Who made the move.
    pub agent: EntityId,
    /// The coerced agent (an owner, an exec, a leader).
    pub over: EntityId,
    pub leverage: Leverage,
    pub demand: Demand,
    pub since: Tick,
    pub until: Tick,
    pub next_due: Tick,
    /// Payments made.
    pub paid: u16,
    pub status: CoercionStatus,
    #[serde(default)]
    pub why: CoercionWhy,
    /// The purse that pays (a building's owner for protection, a gang for a
    /// tribute); `None`: the coerced's own wallet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<EntityId>,
    /// Protection: the business it is levied on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed: Option<Tick>,
    /// The pressure pass phase 3 (`[protection]`): weeks missed since the
    /// last payment or punishment (a round stop met nobody, the till short).
    #[serde(default)]
    pub missed: u8,
    /// Phase 3: punishments dealt on it (0 the till, 1 the owner, 2+ a fire).
    #[serde(default)]
    pub punished: u8,
    /// Phase 3: the owner refuses to pay (a refused lean, or a report after a
    /// punishment) until he chooses to pay up: each week due is a miss.
    #[serde(default)]
    pub refusing: bool,
}

impl Coercion {
    pub fn is_active(&self) -> bool {
        self.status == CoercionStatus::Active
    }
}

/// The fake job (spec § 4 Fraud): `faker` sold `victim` a record that
/// does not exist for `fee`; the victim learns it at `due`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FakeJob {
    pub faker: EntityId,
    pub victim: EntityId,
    pub fee: i64,
    pub due: Tick,
}

// ---------------------------------------------------------------------------
// M16b phase 2: captives
// ---------------------------------------------------------------------------

/// A held agent (spec § 4, `World::captives`): cuffed to its holding
/// building (`Brain.cuffed_by`, so it neither thinks nor walks), a body
/// while held (`Brain.abducted_by`), fed by the holder daily.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Captive {
    /// The owner it is held for (a gang or a corp).
    pub holder: Option<EntityId>,
    /// Who handed it over.
    pub agent: EntityId,
    pub held_at: EntityId,
    pub since: Tick,
    pub coercion: Option<CoercionId>,
    pub until: Tick,
}

/// An Abduct record's terms beyond its target (`Contract.hold`): where
/// the captive goes and what it is held for (`None`: a Harvest's take,
/// ripped at the Hideout).
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Hold {
    pub at: EntityId,
    pub demand: Option<Demand>,
    /// Who the demand is put to (`None`: the default answerer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over: Option<EntityId>,
}

// ---------------------------------------------------------------------------
// M16b phase 3: the wounded
// ---------------------------------------------------------------------------

/// A downed agent (spec § 5, `World::wounded`): it lies where it fell
/// (`Brain.cuffed_by` itself, so it neither thinks nor walks), or is
/// dragged by a carrier (`cuffed_by` the carrier); unstabilised past
/// `bleed_until` it dies; stabilised it rises at `rise_at`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wounded {
    pub by: Option<EntityId>,
    pub since: Tick,
    pub bleed_until: Tick,
    pub tile: TilePos,
    pub stable: bool,
    /// Who is coming for it (a rescuer's claim), if anyone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rescuer: Option<EntityId>,
    /// A Trauma client's corp, when its beacon called a crew.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trauma: Option<EntityId>,
    /// Stabilised: on its feet again at this tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rise_at: Option<Tick>,
    /// A bleed-out is a Murder (the death would have been one).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub murder: bool,
    /// Review finding 8: the nearest staffed Clinic to where it lies,
    /// refreshed every tick while it bleeds (`wounded::tick_down`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clinic: Option<EntityId>,
    /// Review finding 6: who paid the stabilisation (and pays the care).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub care_by: Option<EntityId>,
}

// ---------------------------------------------------------------------------
// M16b phase 4: tags and attention
// ---------------------------------------------------------------------------

/// A tracking tag on an agent (spec § 3; plan deviation: a record in
/// `World::tags`, not an M13 asset): its owner's database gets the
/// carrier's sighting at `conf` once an hour until the battery runs out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub id: u64,
    pub on: EntityId,
    pub owner: EntityId,
    pub tier: u8,
    pub until: Tick,
}

/// A threat holding a share of a faction's attention (the addendum's table).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ThreatKind {
    /// A raid or a mission at its door, a breach in progress.
    Raid,
    /// A riot in a district it controls, or at its building.
    Riot,
    /// A loud run against its nodes.
    LoudRun,
    /// A strike against it.
    Strike,
    /// A hostile acquisition offer made to it.
    Acquisition,
    /// An open vendetta.
    Vendetta,
}

impl ThreatKind {
    pub fn label(self) -> &'static str {
        match self {
            ThreatKind::Raid => "raid",
            ThreatKind::Riot => "riot",
            ThreatKind::LoudRun => "loud run",
            ThreatKind::Strike => "strike",
            ThreatKind::Acquisition => "acquisition",
            ThreatKind::Vendetta => "vendetta",
        }
    }
}

/// A faction's attention (the addendum): the pool (management competence
/// and headcount), the threats holding shares of it, each `(kind, source,
/// until)` (its live span plus `linger_days`), and what is left
/// (`alertness`, never under `alert_floor`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Attention {
    pub pool: f32,
    pub alertness: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threats: Vec<(ThreatKind, EntityId, Tick)>,
}
