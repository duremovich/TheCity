//! L2 § 1, plan L11: the outside world's account book, with M17's names
//! (M17 plan O1's module). L2 builds only the World account and the export
//! hook (`systems::outside`); M17 appends the rest of `OutsideFaction`
//! with `#[serde(default)]`.
//!
//! Conservation (M17's identity, from L2 on): `ownership::total_coins +
//! Σ outside treasuries − outside.minted` is constant to the coin. With
//! `[export]` off nothing crosses and `outside` stays empty.

use std::collections::BTreeMap;

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
    /// Plan field (L37 `SetExportPrice`): a god price per good, over `[export] price`.
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

    /// Σ outside treasuries (the conservation identity's term).
    pub fn treasuries(&self) -> i64 {
        self.factions.iter().map(|f| f.treasury).sum()
    }
}
