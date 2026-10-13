//! L2 § 1, plan L11: the outside world's account book, with M17's names
//! (M17 plan O1's module). L2 builds only the World account and the export
//! hook (`systems::outside`); M17 appends the rest of `OutsideFaction`
//! with `#[serde(default)]`.
//!
//! Conservation (M17's identity, from L2 on): `ownership::total_coins +
//! Σ outside treasuries − outside.minted` is constant to the coin. With
//! `[export]` off nothing crosses and `outside` stays empty.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::components::{ParentId, OUTSIDE_PARENT_BASE};

/// M11 D41: outside parties are numbered from `OUTSIDE_PARENT_BASE`.
pub type OutsideId = ParentId;

/// Plan L11 (deviation: M17 O3 numbers the World 1069; the spec put it at
/// `OUTSIDE_PARENT_BASE` itself, which is corp row 0's `Corp.parent`).
pub const WORLD_ACCOUNT: OutsideId = OUTSIDE_PARENT_BASE + 69;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OutsideKind {
    Megacorp,
    Syndicate,
    State,
    World,
}

/// An outside party (M17 appends persona, edge, rep, data, tech, …).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutsideFaction {
    pub id: OutsideId,
    pub name: String,
    pub kind: OutsideKind,
    pub treasury: i64,
    pub treasury_ref: i64,
    pub base_income: i64,
    pub base_upkeep: i64,
    pub income_today: i64,
    /// The outside market, mean 1.0 (L2 holds it at 1.0).
    pub market: f32,
    pub dead: bool,
    /// Real economy (plan E4, spec § 3): the World's book per good (the
    /// World account only; M17's parents carry none).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub books: BTreeMap<ExportGood, crate::econ::WorldBook>,
    /// M17 phase 1: the World's Ambient book (the World account only; made
    /// on the first day a world with Ambient flats runs with `[outside]` on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<AmbientBook>,
    /// M17 phase 2: a parent's state (megacorps, syndicates, the State;
    /// `None` on the World account).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Parent>,
}

/// M17 § 2: what a parent decides (phase 2 scores Hold and Subsidise; the
/// rest arrive with their phases, appended here once so the enum is stable).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub enum OutsideOrder {
    #[default]
    Hold,
    Subsidise,
    Reinforce,
    Retake,
    Sanction,
    Campaign,
    FoundBranch,
    Abandon,
    /// M17 phase 3: the State's orders.
    Fund,
    Mandate,
    Withdraw,
}

impl OutsideOrder {
    pub fn label(self) -> &'static str {
        match self {
            OutsideOrder::Hold => "Hold",
            OutsideOrder::Subsidise => "Subsidise",
            OutsideOrder::Reinforce => "Reinforce",
            OutsideOrder::Retake => "Retake",
            OutsideOrder::Sanction => "Sanction",
            OutsideOrder::Campaign => "Campaign",
            OutsideOrder::FoundBranch => "FoundBranch",
            OutsideOrder::Abandon => "Abandon",
            OutsideOrder::Fund => "Fund",
            OutsideOrder::Mandate => "Mandate",
            OutsideOrder::Withdraw => "Withdraw",
        }
    }

    /// Every order (append-only order).
    pub const ALL: [OutsideOrder; 11] = [
        OutsideOrder::Hold,
        OutsideOrder::Subsidise,
        OutsideOrder::Reinforce,
        OutsideOrder::Retake,
        OutsideOrder::Sanction,
        OutsideOrder::Campaign,
        OutsideOrder::FoundBranch,
        OutsideOrder::Abandon,
        OutsideOrder::Fund,
        OutsideOrder::Mandate,
        OutsideOrder::Withdraw,
    ];

    /// By label, any case.
    pub fn parse(s: &str) -> Option<OutsideOrder> {
        OutsideOrder::ALL.into_iter().find(|o| o.label().eq_ignore_ascii_case(s))
    }
}

/// M17 § 1: the board abroad, as one aggregate.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Persona {
    pub greed: f32,
    pub pride: f32,
    pub lawfulness: f32,
}

impl Default for Persona {
    fn default() -> Self {
        Persona { greed: 0.5, pride: 0.5, lawfulness: 0.5 }
    }
}

/// M17 § 1: a parent's 30-day rings (newest last) and today's counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutsideLedger {
    /// Income abroad a day (after every factor).
    pub income: VecDeque<i64>,
    /// Coins the branches sent home a day (net of the tariff).
    pub remitted: VecDeque<i64>,
    /// Coins the parent spent on its branches a day (subsidies).
    pub spent: VecDeque<i64>,
    /// The branches' value at each midnight: Σ building values (corps), or
    /// `member_value` a member (gangs).
    pub branch_value: VecDeque<i64>,
    pub remitted_today: i64,
    pub tariff_today: i64,
    pub spent_today: i64,
    pub subsidies_today: u32,
    /// The last midnight's tariff and subsidy count (the CSV's).
    pub tariff_last: i64,
    pub subsidies_last: u32,
    /// Phase 4: the districts the corp branches controlled at each midnight.
    pub districts: VecDeque<i64>,
}

/// M17 phase 2: a parent's state on its `OutsideFaction`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Parent {
    pub persona: Persona,
    /// The share of income riding on the city.
    pub exposure: f32,
    /// `0..=1`, the outside's view of it.
    pub rep: f32,
    /// `-1..=1` toward its branches.
    pub attitude: f32,
    /// `0..=1`, the single gauge (scrip's value from phase 4).
    pub health: f32,
    /// `0..=1`, the last midnight's.
    pub branch_loss: f32,
    /// Corps, gangs or the Law's Jail, ascending.
    pub branches: Vec<crate::entity::EntityId>,
    /// The roster name a refounded branch takes.
    pub branch_name: String,
    pub order: OutsideOrder,
    pub order_since: crate::time::Tick,
    pub order_target: Option<crate::entity::EntityId>,
    /// Every order's score from the last rescore, best first. Not saved.
    #[serde(skip)]
    pub order_trace: Vec<(OutsideOrder, f32)>,
    pub ledger: OutsideLedger,
    pub failing_since: Option<crate::time::Tick>,
    pub insolvent_since: Option<crate::time::Tick>,
    /// The city's `Embargo`: no remittance out, no subsidy in, and the
    /// sanction's cut on income.
    pub embargoed: bool,
    // --- Phase 3: bodies.
    /// The branches' buildings at the last midnight (losses are read against it).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub held: Vec<crate::entity::EntityId>,
    /// Buildings lost within `retake_window_days`, newest last.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lost: Vec<LostBuilding>,
    pub last_reinforce: Option<crate::time::Tick>,
    /// Bodies still to send in this Reinforce episode.
    pub reinforce_left: u32,
    /// The next cohort's number (every episode one).
    pub cohort_next: u32,
    pub abandoned_until: Option<crate::time::Tick>,
    /// The last corp branch's death (FoundBranch's clock).
    pub branch_died_at: Option<crate::time::Tick>,
    /// A god's `SetOutsideOrder`: the order holds until this tick.
    pub pinned_until: Option<crate::time::Tick>,
    /// Syndicates: each gang branch's leader at the last midnight.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub leaders: Vec<(crate::entity::EntityId, Option<crate::entity::EntityId>)>,
    /// Syndicates: brothers due for a gang at a tick (the killer, when known).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub brothers_due: Vec<BrothersDue>,
    pub last_brothers: Option<crate::time::Tick>,
    /// Syndicates: the ticks of its refounds (the window's count).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refounds: Vec<crate::time::Tick>,
    /// M17 review: branches this parent founded (the next one's numeral).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub founded: u32,
    // --- Phase 4: the weapons that reach the ledger.
    /// `0.2..=1.5` competitive edge (Data theft and wipes lower it).
    pub edge: f32,
    /// Data held abroad, per track (the uplink node is its window).
    pub data: crate::virt::DataStore,
    /// The ceiling on its branches' tech tiers (0: none).
    pub tech: [u8; 3],
    /// Scrip in city hands.
    pub scrip_out: i64,
    /// The rival branch a Sanction or Campaign aims at.
    pub rival: Option<OutsideId>,
}

/// M17 phase 4: a sanction on a parent (`by: None` = the city or the State).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sanction {
    pub by: Option<OutsideId>,
    pub on: OutsideId,
    pub until: crate::time::Tick,
    pub cut: f32,
    /// M17 review: a crew abroad burned a convoy (`by` is `None` but this is
    /// no State sanction: subsidies and bodies still cross).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub crew: bool,
}

/// M17 phase 3: a branch building lost (to a raid, a squat, a sale).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LostBuilding {
    pub building: crate::entity::EntityId,
    pub at: crate::time::Tick,
    /// Buy-back offers refused so far.
    pub refusals: u8,
    /// The `Retake` record posted on it.
    pub contract: Option<crate::contract::ContractId>,
    /// The Hit on the raider gang's leader was posted.
    #[serde(default)]
    pub hit_posted: bool,
}

/// M17 phase 3: brothers a syndicate sends a gang.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrothersDue {
    pub gang: crate::entity::EntityId,
    pub at: crate::time::Tick,
    pub killer: Option<crate::entity::EntityId>,
    /// A refound (the gang emptied), not a reinforcement.
    pub refound: bool,
}

/// M17 phase 3: an agent from abroad (a side table, not a component: the
/// 20k city's agents carry nothing for it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outsider {
    pub from: OutsideId,
    pub arrived: crate::time::Tick,
    pub tour_until: crate::time::Tick,
    pub cohort: u32,
    /// A syndicate's brother (a member, no tour).
    #[serde(default)]
    pub brother: bool,
}

/// M17 phase 3: the day's bodies counters (the CSV's).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutsideDay {
    /// Phase 4: coins crossing in from, and out to, the parents (every flow).
    pub parents_in: i64,
    pub parents_out: i64,
    // Phase 4.
    pub uplink_runs: u32,
    pub uplink_breached: u32,
    pub data_shipped: u32,
    pub sanctions: u32,
    pub campaigns: u32,
    pub parents_dead: u32,
    pub scrip_paid: i64,
    pub scrip_spent: i64,
    pub scrip_converted: i64,
    pub scrip_wiped: i64,
    pub arrived: u32,
    pub left: u32,
    pub settled: u32,
    pub reinforcements: u32,
    pub buybacks: u32,
    pub retakes_posted: u32,
    pub retakes_won: u32,
    pub branches_founded: u32,
    pub parents_founded: u32,
    pub abandoned: u32,
    pub brothers: u32,
    pub refounded: u32,
    pub grants: u32,
    pub mandates: u32,
    pub withdrawals: u32,
    /// Phase 4b: the succession (promotions after a fall, civil wars opened,
    /// and how each ended for its loser).
    pub successions: u32,
    pub wars: u32,
    pub war_dead: u32,
    pub war_fled: u32,
    pub war_splinter: u32,
    pub war_jailed: u32,
    /// Phase 5: missions abroad posted and resolved (won), crew dead
    /// abroad, cast promoted and arrived, Blackwall breaches.
    pub abroad_posted: u32,
    pub abroad_done: u32,
    pub abroad_won: u32,
    pub abroad_dead: u32,
    pub promoted: u32,
    pub cast_arrived: u32,
    pub breaches: u32,
}

/// M17 phase 1 (docs/M17_OUTSIDE.md, "Revised 2026-10-12"): the Ambient
/// flats' households as a World buyer at retail. The appetite walks as a
/// good's; the day's demand per district is fixed at the first selling hour
/// (`due_day`) from the Ambient district aggregates and spread over the
/// selling hours. Rings are newest last (`econ::BOOK_DAYS`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientBook {
    pub appetite: f32,
    pub walk: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appetite_pin: Option<f32>,
    /// The day `food_due` and `visit_due` were set for.
    pub due_day: u64,
    /// Today's food units and venue visits due per district (`World::districts` order).
    pub food_due: Vec<u32>,
    pub visit_due: Vec<u32>,
    /// Ambient visits each venue took today (the `visit_share` cap), by venue.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub venue_today: BTreeMap<crate::entity::EntityId, u32>,
    /// Daily food units bought, visits made and coins paid, newest last.
    #[serde(skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub units: std::collections::VecDeque<u32>,
    #[serde(skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub visits: std::collections::VecDeque<u32>,
    #[serde(skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub paid: std::collections::VecDeque<i64>,
}

impl Default for AmbientBook {
    fn default() -> Self {
        AmbientBook {
            appetite: 1.0,
            walk: 1.0,
            appetite_pin: None,
            due_day: u64::MAX,
            food_due: Vec::new(),
            visit_due: Vec::new(),
            venue_today: BTreeMap::new(),
            units: Default::default(),
            visits: Default::default(),
            paid: Default::default(),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ExportGood {
    Food,
    Parts,
    Data,
    /// Jobs & lives phase 1 (D6): the Packing Plants' Rations.
    Rations,
    /// Jobs & lives (D6): braindance (its book inert until phase 5).
    Braindance,
    /// Jobs & lives (D6, D10): organs (its book inert until phase 6).
    Organs,
}

impl ExportGood {
    /// The books every world keeps (the appended ones only with `[jobs2]`
    /// on: `world_market::goods`).
    pub const ALL: [ExportGood; 3] = [ExportGood::Food, ExportGood::Parts, ExportGood::Data];

    /// Jobs & lives phase 1: every book, the appended ones included.
    pub const ALL_JOBS2: [ExportGood; 6] = [
        ExportGood::Food,
        ExportGood::Parts,
        ExportGood::Data,
        ExportGood::Rations,
        ExportGood::Braindance,
        ExportGood::Organs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ExportGood::Food => "food",
            ExportGood::Parts => "parts",
            ExportGood::Data => "data",
            ExportGood::Rations => "rations",
            ExportGood::Braindance => "braindance",
            ExportGood::Organs => "organs",
        }
    }

    pub fn parse(s: &str) -> Option<ExportGood> {
        ExportGood::ALL_JOBS2.into_iter().find(|g| g.label().eq_ignore_ascii_case(s))
    }
}

/// What the World account bought (L2's; M17 reads it as the World's demand).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExportBook {
    pub sold: BTreeMap<ExportGood, u64>,
    pub paid: BTreeMap<ExportGood, i64>,
    /// Plan field (L37 `SetExportPrice`): a god price per good, the World's
    /// reference-bid pin (`world_market::bid_ref`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub price: BTreeMap<ExportGood, i64>,
}

/// `World::outside`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Outside {
    pub factions: Vec<OutsideFaction>,
    /// Σ coins the outside created (refills) since day 0.
    pub minted: i64,
    /// Cumulative coins across the boundary, in and out of the city.
    pub inbound: i64,
    pub outbound: i64,
    pub export: ExportBook,
    pub next_id: OutsideId,
    /// M17 phase 2: the city's `SetTariff`: the share of every remittance
    /// kept by the Treasury.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub tariff: f32,
    /// M17 phase 2: `inbound`/`outbound` at the last day's close (the CSV's
    /// daily crossings).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub inbound_mark: i64,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub outbound_mark: i64,
    // --- Phase 3: bodies.
    /// Agents from abroad and the brothers, by entity.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outsiders: BTreeMap<crate::entity::EntityId, Outsider>,
    /// The State's extra guards (the reconcile hires `guard_count` + these).
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub state_guards: u16,
    /// The State's Mandate holds until this tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandate_until: Option<crate::time::Tick>,
    /// Days each unparented corp has held the founding thresholds.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub found_streak: BTreeMap<crate::entity::EntityId, u32>,
    /// The parents with CSV columns (the seeded ones: the header is fixed).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub csv_parents: u8,
    #[serde(default, skip_serializing_if = "is_default_day")]
    pub today: OutsideDay,
    // --- Phase 4.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sanctions: Vec<Sanction>,
    /// Scrip held, per holder: `(issuer, units)`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scrip: BTreeMap<crate::entity::EntityId, Vec<(OutsideId, i64)>>,
    /// `BanScrip`: no issuance.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub scrip_banned: bool,
    // --- Phase 5: Abroad, the cast, the Blackwall, the State's cap.
    /// The missions abroad under way (a ledger record each).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abroad: Vec<Abroad>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub abroad_next: u32,
    /// The agents away on a mission (`Abroad.id`): off the map, Statistical
    /// and skipped by the hourly table.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub away: BTreeMap<crate::entity::EntityId, u32>,
    /// Cast members promoted to arrive later as immigrants.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promoted: Vec<Promoted>,
    /// The promoted who arrived and live (the `promoted_max` cap).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promoted_living: Vec<crate::entity::EntityId>,
    /// The Blackwall's last breach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_breach: Option<crate::time::Tick>,
    /// `SetStateGuards`: the city's cap on the State's guards (`None`: the config's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_guards_cap: Option<u16>,
    /// A Blackwall breach rolled on a run, landing at the next midnight:
    /// `(parent, runner, patron)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breach_due: Option<(OutsideId, crate::entity::EntityId, Option<crate::entity::EntityId>)>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// M17 phase 5 (§ 5): an asset abroad an `Abroad` mission strikes.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum OutsideAsset {
    /// Coins: `abroad_loot` from the parent's treasury.
    Depot,
    /// Data: `abroad_data` a track into the buyer's store, and `edge`.
    DataCentre,
    /// A sanction's cut on the parent for a week.
    Convoy,
}

impl OutsideAsset {
    pub fn label(self) -> &'static str {
        match self {
            OutsideAsset::Depot => "depot",
            OutsideAsset::DataCentre => "data centre",
            OutsideAsset::Convoy => "convoy",
        }
    }

    pub fn parse(s: &str) -> Option<OutsideAsset> {
        match s.to_ascii_lowercase().as_str() {
            "depot" => Some(OutsideAsset::Depot),
            "data_centre" | "datacentre" | "data" => Some(OutsideAsset::DataCentre),
            "convoy" => Some(OutsideAsset::Convoy),
            _ => None,
        }
    }
}

/// M17 phase 5 (§ 5): a mission abroad, always at the Ledger LOD: the crew
/// is away (`Outside.away`) until `back_at`, then one keyed draw resolves it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Abroad {
    pub id: u32,
    /// The buyer of record (a branch, a gang, an agent; `None`: god's).
    pub buyer: Option<crate::entity::EntityId>,
    pub on: OutsideId,
    pub asset: OutsideAsset,
    /// The crew, its leader first.
    pub crew: Vec<crate::entity::EntityId>,
    pub since: crate::time::Tick,
    pub back_at: crate::time::Tick,
}

/// M17 phase 5 (§ 5): a cast member's role abroad.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CastRole {
    Guard,
    Clerk,
    Runner,
    Fixer,
    Bystander,
}

/// M17 phase 5 (§ 5): a transient NPC of a mission abroad, a record (never
/// an entity), hash-derived from the mission and its index. `personality`
/// is `[lawfulness, greed, pride, sociability, courage, loyalty]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CastMember {
    pub seed: u64,
    pub name: String,
    pub role: CastRole,
    pub personality: [f32; 6],
    /// `-1..=1` toward the crew.
    pub disposition: f32,
}

/// M17 phase 5 (§ 5): a cast member who will arrive as an immigrant with an
/// edge (Friend or Enemy) to `toward` on day `arrive_on`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Promoted {
    pub cast: CastMember,
    pub toward: crate::entity::EntityId,
    pub rel: crate::components::RelKind,
    pub arrive_on: u64,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

fn is_default_day(v: &OutsideDay) -> bool {
    *v == OutsideDay::default()
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

impl Outside {
    /// `skip_serializing_if`: nothing has crossed, no account exists.
    pub fn is_empty(&self) -> bool {
        self.factions.is_empty()
            && self.minted == 0
            && self.inbound == 0
            && self.outbound == 0
            && self.export == ExportBook::default()
    }

    pub fn faction(&self, id: OutsideId) -> Option<&OutsideFaction> {
        self.factions.iter().find(|f| f.id == id)
    }

    pub fn faction_mut(&mut self, id: OutsideId) -> Option<&mut OutsideFaction> {
        self.factions.iter_mut().find(|f| f.id == id)
    }

    /// M17 phase 2: the parents (every account but the World), in account order.
    pub fn parents(&self) -> impl Iterator<Item = &OutsideFaction> {
        self.factions.iter().filter(|f| f.parent.is_some())
    }

    /// Σ outside treasuries (the conservation identity's term).
    pub fn treasuries(&self) -> i64 {
        self.factions.iter().map(|f| f.treasury).sum()
    }
}
