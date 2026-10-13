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
        }
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
}

impl ExportGood {
    pub const ALL: [ExportGood; 3] = [ExportGood::Food, ExportGood::Parts, ExportGood::Data];

    pub fn label(self) -> &'static str {
        match self {
            ExportGood::Food => "food",
            ExportGood::Parts => "parts",
            ExportGood::Data => "data",
        }
    }

    pub fn parse(s: &str) -> Option<ExportGood> {
        ExportGood::ALL.into_iter().find(|g| g.label().eq_ignore_ascii_case(s))
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
