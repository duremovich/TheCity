//! The Real economy (docs/ECONOMY_V2.md, plan E3): the milestone's types.
//! Logic lives in `systems::{econ, world_market}` (phase 1) and, later,
//! `systems::{wages, treasury, charity, camp}`; every path runs behind its
//! section's `on()` (plan E1), so with `[economy2] enabled = false`
//! (`--econ-off`) none of these is ever written and the `EC_BASE` city is
//! reproduced to the column.
//!
//! Everything here is a ledger abstraction: the World's book per good is a
//! set of counters and 30-day rings on the outside account; a "crossing" is
//! coins moved between an integer purse and that account.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Days of the World's per-good rings (`WorldBook.{bought, sold, paid, charged, caps}`).
pub const BOOK_DAYS: usize = 30;
/// Days of `EconState.trade_ring` (`trade_balance_30`).
pub const TRADE_DAYS: usize = 30;

/// Plan E49: the purpose constants of the milestone's pure hashes
/// (`systems::econ::{hash_unit, hash_normal}`).
pub const PURPOSE_APPETITE: u64 = 0xEC01;
pub const PURPOSE_DONATE: u64 = 0xEC02;

/// Plan E4 (spec § 3): the World account's book for one good. `appetite`
/// is the clamped sum of the walk and the season level (E7); `walk` the
/// random walk alone (plan field). Rings are newest last.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldBook {
    pub appetite: f32,
    #[serde(default = "one_f32")]
    pub walk: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appetite_pin: Option<f32>,
    /// Plan field: a god's daily cap (`SetExportCap`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_pin: Option<u32>,
    /// Plan field: the per-good ask multiplier (`SetImportAsk`), over `[world_market] ask_mult`.
    #[serde(default = "one_f32")]
    pub ask_mult: f32,
    /// Units the World bought from the city today, and sold to it.
    #[serde(default)]
    pub bought_today: u32,
    #[serde(default)]
    pub sold_today: u32,
    /// Coins the World paid the city today, and charged it (the ask).
    #[serde(default)]
    pub paid_today: i64,
    #[serde(default)]
    pub charged_today: i64,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub bought: VecDeque<u32>,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub sold: VecDeque<u32>,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub paid: VecDeque<i64>,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub charged: VecDeque<i64>,
    /// Plan field (E18, E19): each day's `cap_today`, for the fill ratio and the unfilled order.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub caps: VecDeque<u32>,
}

fn one_f32() -> f32 {
    1.0
}

impl Default for WorldBook {
    fn default() -> Self {
        WorldBook {
            appetite: 1.0,
            walk: 1.0,
            appetite_pin: None,
            cap_pin: None,
            ask_mult: 1.0,
            bought_today: 0,
            sold_today: 0,
            paid_today: 0,
            charged_today: 0,
            bought: VecDeque::new(),
            sold: VecDeque::new(),
            paid: VecDeque::new(),
            charged: VecDeque::new(),
            caps: VecDeque::new(),
        }
    }
}

impl WorldBook {
    /// Close the day: today's counters into the rings (capped at `BOOK_DAYS`), reset.
    pub fn roll(&mut self, cap_today: u32) {
        fn push<T>(ring: &mut VecDeque<T>, v: T) {
            if ring.len() >= BOOK_DAYS {
                ring.pop_front();
            }
            ring.push_back(v);
        }
        push(&mut self.bought, std::mem::take(&mut self.bought_today));
        push(&mut self.sold, std::mem::take(&mut self.sold_today));
        push(&mut self.paid, std::mem::take(&mut self.paid_today));
        push(&mut self.charged, std::mem::take(&mut self.charged_today));
        push(&mut self.caps, cap_today);
    }

    /// Σ of the last `days` entries of a ring.
    pub fn sum_u32(ring: &VecDeque<u32>, days: usize) -> u64 {
        ring.iter().rev().take(days).map(|&v| u64::from(v)).sum()
    }
}

/// Plan E3: `World::econ`, the milestone's world state (saved when not
/// default; never written with the economy off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EconState {
    /// E21 (phase 3a): `SetTaxRate` pins the rate against the band; `SetTaxAuto` releases it.
    pub tax_pinned: bool,
    /// E45: the last `TRADE_DAYS` days' `trade_balance`, newest last.
    #[serde(skip_serializing_if = "VecDeque::is_empty")]
    pub trade_ring: VecDeque<i64>,
    /// E33 (phase 3a): the Recycler's purse in all but name (in `total_coins`).
    pub recycler_till: i64,
    /// E47 `CloseWorld`: the World neither buys nor sells.
    pub world_closed: bool,
    /// E47 `SetCustoms`: a god's customs rate over `[world_market] customs_rate`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customs_pin: Option<f32>,
}

impl EconState {
    pub fn is_default(&self) -> bool {
        *self == EconState::default()
    }
}

/// Plan 1.2 (the census): probe counters of the coin sources and sinks
/// outside `pay`/`charge`, written at their sites, never saved or read by
/// the simulation (`tests/outside.rs::probe_coin_census`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoinProbe {
    /// Coins minted into immigrants' wallets (E24: crossed in from the World with the market on).
    pub immigrant_coins: i64,
    /// Coins destroyed with emigrants' wallets (E24: crossed out with the market on).
    pub emigrant_coins: i64,
    /// The fence's resale credit (E25b: a crossing from the World with the market on).
    pub fence_credit: i64,
    /// Hole loot paid to a bound actor (conserving; recorded).
    pub loot_bound: i64,
    /// Hole loot lost on an Unknown binding, an expiry or a dropped victim
    /// (E25a: to the Treasury as unclaimed property with the market on).
    pub loot_lost: i64,
    /// Coins a god command minted or burned (`FundGang`, `SetTreasury`, …).
    pub god_coins: i64,
}
