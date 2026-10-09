//! M11 ownership (docs/M11_OWNERSHIP.md § 3-4, plan phase 2): purses, the one
//! transfer path every owner flow goes through, the seeding of the eight
//! corps, and the daily pass (rent, evictions, re-housing, upkeep, exec
//! wages, rolls).
//!
//! Money conservation: every coin an owner earns or pays moves through
//! [`pay`] or [`charge`] (plan D1-D3), so the sum of wallets, gang and corp
//! treasuries and the Treasury changes only by the documented sources and
//! sinks (endowments, immigrants, the fence, god commands, emigrants' wallets).
//! Tax is a transfer into the Treasury, withheld at the moment of the flow.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{
    Brain, Building, BuildingKind, Child, Corp, CorpLoss, CorpShock, Corpse, Gang, Household, Identity, Job,
    MemoryKind, Niche, Personality, Position, Role, Sentence, Treasury, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::TICKS_PER_DAY;
use crate::world::World;

/// Days of eviction ticks kept in `World::eviction_log`.
const EVICTION_LOG_DAYS: u64 = 60;
/// `Corp.revenue` / `Building.revenue` window.
const REVENUE_DAYS: usize = 7;
/// `Corp.cashflow` window.
const CASHFLOW_DAYS: usize = 14;
/// `Corp.loss_log` window.
const LOSS_DAYS: u64 = 14;
/// Pending corp shocks kept between rescorings.
const SHOCK_CAP: usize = 32;

// ---------------------------------------------------------------------------
// Owners and purses (D1)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OwnerKind {
    City,
    Corp(EntityId),
    Gang(EntityId),
    Agent(EntityId),
}

/// `None` is the city; then a Corp, a Gang, an agent's Wallet in that order.
/// Anything else (a corpse, a despawned id) reads as the city.
pub fn owner_kind(world: &World, owner: Option<EntityId>) -> OwnerKind {
    let Some(o) = owner else { return OwnerKind::City };
    if world.has::<Corp>(o) {
        OwnerKind::Corp(o)
    } else if world.has::<Gang>(o) {
        OwnerKind::Gang(o)
    } else if world.has::<Wallet>(o) {
        OwnerKind::Agent(o)
    } else {
        debug_assert!(false, "owner {o:?} is no corp, gang or living agent");
        OwnerKind::City
    }
}

impl World {
    /// The coins behind an owner: an agent's Wallet, a gang's or corp's
    /// treasury, or the city Treasury.
    pub fn purse(&self, owner: Option<EntityId>) -> i64 {
        match owner_kind(self, owner) {
            OwnerKind::City => self.treasury().map_or(0, |t| t.coins),
            OwnerKind::Corp(c) => self.comp::<Corp>(c).map_or(0, |c| c.treasury),
            OwnerKind::Gang(g) => self.comp::<Gang>(g).map_or(0, |g| g.treasury),
            OwnerKind::Agent(a) => self.comp::<Wallet>(a).map_or(0, |w| w.coins),
        }
    }

    /// Add to an owner's purse (may go negative for corps and the city). Only
    /// `pay`, `charge` and `refund` call it; a corp's `cashflow_today` moves too.
    pub(crate) fn purse_add(&mut self, owner: Option<EntityId>, delta: i64) {
        match owner_kind(self, owner) {
            OwnerKind::City => {
                if let Some(t) = self.treasury_mut() {
                    t.coins += delta;
                }
            }
            OwnerKind::Corp(c) => {
                if let Some(c) = self.comp_mut::<Corp>(c) {
                    c.treasury += delta;
                    c.cashflow_today += delta;
                }
            }
            OwnerKind::Gang(g) => {
                if let Some(g) = self.comp_mut::<Gang>(g) {
                    g.treasury += delta;
                }
            }
            OwnerKind::Agent(a) => {
                if let Some(w) = self.comp_mut::<Wallet>(a) {
                    w.coins += delta;
                }
            }
        }
    }

    /// M13 D38: coins into a gang's treasury from no purse (tribute, a split
    /// of the day's loot, a fence's resale, a raid's prize, loot): the one
    /// writer of `treasury +=` for gangs outside `pay`/`charge`, counted as
    /// gang income (`stats.gang_income`, assets on only).
    pub fn gang_credit(&mut self, gang: EntityId, coins: i64) {
        let Some(g) = self.comp_mut::<Gang>(gang) else { return };
        g.treasury += coins;
        if coins > 0 && self.config.assets.enabled {
            self.stats.current.gang_income += coins;
        }
    }

    /// "the city", a corp's or gang's name, an agent's name.
    pub fn owner_label(&self, owner: Option<EntityId>) -> String {
        match owner {
            None => "the city".to_string(),
            Some(o) => {
                if let Some(c) = self.comp::<Corp>(o) {
                    c.name.clone()
                } else if let Some(g) = self.comp::<Gang>(o) {
                    g.name.clone()
                } else if self.has::<Identity>(o) {
                    self.name_of(o)
                } else {
                    "nobody".to_string()
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The transfer path (D2, D3)
// ---------------------------------------------------------------------------

/// What a transfer is for: picks the ledger column and whether tax is withheld.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Flow {
    Food,
    Drink,
    Wage,
    Rent,
    Contract,
    Upkeep,
    Wholesale,
    Found,
    Sale,
    Bribe,
    ExecWage,
    Subsidy,
    SellFood,
    JailFood,
    /// Wage tax a non-city employer owes the Treasury (`economy::collect_wage`).
    Tax,
    /// M12 D15: a Vagrancy fine, agent -> Treasury (untaxed).
    Fine,
    /// M12 D20: a Hotel night, guest -> the Hotel's owner (taxed).
    Hotel,
    /// M12 D24: an owner pays the City to sweep around its doors (untaxed).
    Sanitation,
    /// M12 D39 (phase 4): a corp robbed by a raid (untaxed).
    Robbery,
    /// M13 D8: an asset sale, an install fee, a repair, an impound sale (taxed).
    Asset,
    /// M13 D8: an asset's daily upkeep, owner -> Treasury (untaxed).
    AssetUpkeep,
    /// M13 D8: a finance payment, owner -> lender (untaxed).
    Finance,
    /// M13 D8: a seller's import, a gang's cook, a Market's Stims restock -> Treasury (untaxed).
    Import,
    /// M13 D8: a dealer's sale and cut (untaxed).
    Stims,
    /// M13 D8: a Market's legal Stims sale (taxed; the `flow_stims` column).
    StimsLegal,
    /// M13 D8: Parts bought by a Clinic or Garage (taxed, as wholesale).
    Parts,
    /// M13 D8: Therapy and Detox (taxed).
    Treatment,
    /// M14 V17/V40: a Data sale, Tech corp -> seller (taxed, as wholesale).
    Data,
    /// M14 V20/V40: a Ledger run's take (untaxed).
    Hack,
    /// M14 V26/V40: a node's daily ICE upkeep, owner -> Treasury (untaxed).
    IceUpkeep,
    /// M14 V21/V40: research upkeep and tier purchases, corp -> Treasury (untaxed).
    Research,
    /// M14 V5/V40: a public terminal's fee, runner -> Bar or Hotel owner (taxed).
    Terminal,
    /// M15 W37: a corp's daily ads, split across the licensed Feeds by reach
    /// (taxed as owner revenue; the Civic Wire's share is the Treasury's).
    Ads,
    /// M15 W40: a Spin corp's plant or bury payment to a Feed's owner (taxed).
    Plant,
    /// L2 L12: a venue's entry or meal, visitor -> owner (taxed).
    Leisure,
    /// L2 L12: a gambling loss or a bout's bet, loser -> house (taxed).
    Gamble,
    /// L2 L12 (plan): a house paying a win (untaxed).
    GambleWin,
    /// L2 L12: a gang's weekly cut to its members (untaxed).
    Tribute,
    /// L2 L11: the World account's purchase (untaxed at the crossing; the
    /// owner pays the sale's tax explicitly).
    Export,
    /// L2 L12: street dice, agent to agent (untaxed; review fix: not a
    /// house's pay-out, so out of `flow_gamble`).
    StreetDice,
    /// L2 shadow fixes item 21: the Recycler's coin for a scavenged haul,
    /// Treasury -> agent (untaxed; in `flow_other` as `Sanitation` was).
    Scavenge,
    /// M16a (plan C5): coins into or out of a contract record's escrow
    /// (capital, untaxed; `flow_escrow` is the day's net into escrow).
    Escrow,
    /// M16a (plan C5): a contract record's pay-out to its taker and crew,
    /// or a paid sighting (untaxed).
    Payout,
    /// M16a (plan C5): a Fixer's cut of a settled record (taxed: a
    /// registered business's revenue).
    FixerCut,
    // --- The Real economy (docs/ECONOMY_V2.md, plan E5, E24, E25).
    /// E5: customs on an import to the World, importer -> Treasury (untaxed).
    Customs,
    /// E24: an immigrant's pocket from the World, an emigrant's wallet to it (untaxed).
    Migrant,
    /// E25b: the fence's resale credit, the World -> a gang (untaxed; a mint, now visible).
    Fence,
    /// Real economy E28: a gift into a Mission's purse (untaxed; from an
    /// agent, a gang, a corp, or the World for a god's gift).
    Donate,
    /// Real economy E27: a Mission's meal or cot, ledger-only (the purse
    /// bought the food; the value reaches the eater in kind).
    Alms,
    /// Real economy E39: the city Camp's restock, Treasury -> Market owner
    /// (untaxed; the Jail meal's shape).
    CampFood,
    // --- Phase 2 (plan E13).
    /// E13: the property rate, a non-city owner -> Treasury (untaxed; replaces `Upkeep` with wages on).
    Property,
    /// E13: production inputs and daily power, a producer (any owner) -> the World (untaxed).
    Inputs,
}

impl Flow {
    /// Capital, not trading: buying and selling buildings, founding one, a
    /// subsidy. Kept out of a corp's `cashflow` (the brain's `flow` input),
    /// or one purchase reads as two weeks of losses.
    pub fn capital(self) -> bool {
        matches!(self, Flow::Sale | Flow::Found | Flow::Subsidy | Flow::Escrow)
    }

    /// Owner revenue taxed at the moment of the flow (spec § 3); wage tax
    /// keeps `Job.tax_accum`.
    pub fn taxed(self) -> bool {
        matches!(
            self,
            Flow::Food
                | Flow::Drink
                | Flow::Rent
                | Flow::Contract
                | Flow::Wholesale
                | Flow::Hotel
                | Flow::Asset
                | Flow::StimsLegal
                | Flow::Parts
                | Flow::Treatment
                | Flow::Data
                | Flow::Terminal
                | Flow::Ads
                | Flow::Plant
                | Flow::Leisure
                | Flow::Gamble
                | Flow::FixerCut
        )
    }
}

fn ledger(world: &mut World, flow: Flow, coins: i64) {
    let row = &mut world.stats.current;
    match flow {
        Flow::Food => row.flow_food += coins,
        Flow::Drink => row.flow_drink += coins,
        Flow::Wage | Flow::ExecWage => row.flow_wages += coins,
        Flow::Rent => row.flow_rent += coins,
        Flow::Upkeep => row.flow_upkeep += coins,
        Flow::Wholesale => row.flow_wholesale += coins,
        Flow::Contract => row.flow_contract += coins,
        Flow::Tax => row.flow_tax += coins,
        Flow::Found
        | Flow::Sale
        | Flow::Bribe
        | Flow::Subsidy
        | Flow::SellFood
        | Flow::JailFood
        | Flow::Fine
        | Flow::Hotel
        | Flow::Sanitation
        | Flow::Scavenge
        | Flow::Robbery => row.flow_other += coins,
        Flow::Asset => row.flow_asset += coins,
        Flow::AssetUpkeep => row.flow_asset_upkeep += coins,
        Flow::Finance => row.flow_finance += coins,
        Flow::Import => row.flow_import += coins,
        Flow::Stims | Flow::StimsLegal => row.flow_stims += coins,
        Flow::Parts => row.flow_parts += coins,
        Flow::Treatment => row.flow_treatment += coins,
        Flow::Data => row.virt.flow_data += coins,
        Flow::Hack => row.virt.flow_hack += coins,
        Flow::IceUpkeep => row.virt.flow_ice_upkeep += coins,
        Flow::Research => row.virt.flow_research += coins,
        Flow::Terminal => row.virt.flow_terminal += coins,
        Flow::Ads => row.word.flow_ads += coins,
        Flow::Plant => row.word.flow_plant += coins,
        // L2 L12, L34: `flow_gamble` is net to houses (losses - wins).
        Flow::Leisure => row.living.flow_leisure += coins,
        Flow::Gamble => row.living.flow_gamble += coins,
        Flow::GambleWin => {
            row.living.flow_gamble -= coins;
            row.living.flow_gamble_win += coins;
        }
        Flow::Tribute => row.living.flow_tribute += coins,
        Flow::Export => row.living.flow_export += coins,
        Flow::StreetDice => row.living.flow_street_dice += coins,
        // M16a (plan C38).
        Flow::Escrow => row.contract.flow_escrow += coins,
        Flow::Payout => row.contract.flow_payout += coins,
        Flow::FixerCut => row.contract.flow_fixer_cut += coins,
        // Real economy (plan E45).
        Flow::Customs => row.econ.flow_customs += coins,
        Flow::Migrant => row.econ.flow_migrant_in += coins,
        Flow::Fence => row.econ.flow_fence += coins,
        // Real economy phase 3b/3c (E27, E28, E39).
        Flow::Donate => row.econ.flow_donate += coins,
        Flow::Alms => row.econ.flow_alms += coins,
        Flow::CampFood => row.econ.flow_camp_food += coins,
        Flow::Property => row.econ.flow_property += coins,
        Flow::Inputs => row.econ.flow_inputs += coins,
    }
}

/// The ledger line of a crossing out of the city (`cross_out`): the flows
/// with an outbound column of their own, else `ledger`.
fn ledger_out(world: &mut World, flow: Flow, coins: i64) {
    let row = &mut world.stats.current;
    match flow {
        Flow::Migrant => row.econ.flow_migrant_out += coins,
        Flow::Import => {
            row.flow_import += coins;
            row.econ.flow_import_out += coins;
        }
        other => ledger(world, other, coins),
    }
}

/// M14 V26: a flow that moves no coins (the city paying itself, the
/// Treasury's own nodes' ICE upkeep) still shows in its ledger column.
pub fn ledger_only(world: &mut World, flow: Flow, coins: i64) {
    if coins > 0 {
        ledger(world, flow, coins);
    }
}

/// Whole coins of tax on `amount` paid to `payee`, the fraction carried in
/// `World::tax_accum`.
fn withhold(world: &mut World, payee: EntityId, amount: i64) -> i64 {
    let rate = world.levers.tax_rate;
    if rate <= 0.0 {
        return 0;
    }
    let acc = world.tax_accum.entry(payee).or_insert(0.0);
    *acc += amount as f32 * rate;
    let tax = (acc.floor() as i64).clamp(0, amount);
    *acc -= tax as f32;
    tax
}

/// The `shadow` tool's flow note (L2 shadow V2): a no-op unless the log is on
/// and an end of the transfer is an agent.
fn shadow_flow(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, coins: i64, flow: Flow, refund: bool) {
    if world.shadow_notes.is_none() {
        return;
    }
    let agent = |e: Option<EntityId>| e.is_some_and(|e| world.has::<crate::components::Identity>(e));
    if !(agent(from) || agent(to)) {
        return;
    }
    let tick = world.tick;
    if let Some(log) = world.shadow_notes.as_mut() {
        log.push(crate::word::ShadowNote::Flow { tick, from, to, coins, flow, refund });
    }
}

fn transfer(
    world: &mut World,
    from: Option<EntityId>,
    to: Option<EntityId>,
    amount: i64,
    flow: Flow,
    full: bool,
) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let moved = if full { amount } else { amount.min(world.purse(from).max(0)) };
    if moved <= 0 || from == to {
        // Paying oneself moves nothing and is no flow.
        return moved.max(0);
    }
    ledger(world, flow, moved);
    shadow_flow(world, from, to, moved, flow, false);
    world.purse_add(from, -moved);
    let tax = match to {
        Some(payee) if flow.taxed() => withhold(world, payee, moved),
        _ => 0,
    };
    world.purse_add(to, moved - tax);
    if tax > 0 {
        world.purse_add(None, tax);
        world.stats.current.flow_tax += tax;
    }
    // M13 D38: what reaches a gang's purse is gang income (dealing apart).
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved - tax;
        if flow == Flow::Stims {
            world.stats.current.gang_income_dealing += moved - tax;
        }
    }
    if flow.capital() {
        uncount_cashflow(world, from, moved);
        uncount_cashflow(world, to, -(moved - tax));
    }
    // Real economy phase 2 (plan E16, E45): a corp payee's revenue window
    // (net of the withheld tax; capital flows never count), and the
    // population's other inflows (wages on only).
    if crate::systems::wages::on(world) {
        note_revenue(world, to, flow, moved - tax);
        crate::systems::wages::note_inflow(world, to, flow, moved - tax);
    }
    moved
}

/// Plan E16: a taxed flow reaching a corp is revenue (`Corp.rev_today`).
fn note_revenue(world: &mut World, to: Option<EntityId>, flow: Flow, net: i64) {
    if !flow.taxed() || net <= 0 {
        return;
    }
    if let Some(c) = to.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.rev_today += net;
    }
}

/// Plan E16: a corp's gross payroll today (`Corp.pay_today`, wages on only).
pub fn note_payroll(world: &mut World, payer: Option<EntityId>, gross: i64) {
    if gross <= 0 || !crate::systems::wages::on(world) {
        return;
    }
    if let Some(c) = payer.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.pay_today += gross;
    }
}

/// Take a capital transfer back out of a corp's `cashflow_today`.
fn uncount_cashflow(world: &mut World, owner: Option<EntityId>, delta: i64) {
    if let Some(c) = owner.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.cashflow_today += delta;
    }
}

/// Move `min(amount, max(purse(from), 0))` from one purse to another: nobody
/// pays from a negative purse. Returns the coins moved.
pub fn pay(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    transfer(world, from, to, amount, flow, false)
}

/// As `pay`, but a corp or the city pays the full amount and may go
/// negative (upkeep, the city's purchases); agents and gangs are capped.
pub fn charge(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    let full = matches!(owner_kind(world, from), OwnerKind::City | OwnerKind::Corp(_));
    transfer(world, from, to, amount, flow, full)
}

/// Undo a `pay(agent -> owner, amount, flow)` (a purchase whose stock was
/// gone): the owner returns its share and the Treasury the tax it took. The
/// tax comes back through the payee's accumulator (`withhold` run
/// backwards), so the owner and the Treasury end where they started, not a
/// coin of rounding apart. Conserves coins; the owner may dip below zero.
pub fn refund(world: &mut World, agent: EntityId, owner: Option<EntityId>, amount: i64, flow: Flow) {
    if amount <= 0 || owner == Some(agent) {
        return;
    }
    ledger(world, flow, -amount);
    shadow_flow(world, owner, Some(agent), amount, flow, true);
    let rate = world.levers.tax_rate;
    let tax = match owner {
        Some(payee) if flow.taxed() && rate > 0.0 => {
            let acc = world.tax_accum.entry(payee).or_insert(0.0);
            *acc -= amount as f32 * rate;
            let back = if *acc < 0.0 { ((-*acc).ceil() as i64).clamp(0, amount) } else { 0 };
            *acc += back as f32;
            back
        }
        _ => 0,
    };
    world.purse_add(owner, -(amount - tax));
    if tax > 0 {
        world.purse_add(None, -tax);
        world.stats.current.flow_tax -= tax;
    }
    world.purse_add(Some(agent), amount);
}

/// Undo a `pay(owner -> Treasury, amount, Flow::Tax)` surcharge (the
/// `SetLeisureTax` cut on an abandoned purchase or a refunded bet): the
/// Treasury returns `amount` to `owner` and `flow_tax` falls by it.
/// Conserves coins; the Treasury may dip below zero.
pub fn refund_tax(world: &mut World, owner: Option<EntityId>, amount: i64) {
    if amount <= 0 || owner.is_none() {
        return;
    }
    shadow_flow(world, None, owner, amount, Flow::Tax, true);
    world.purse_add(None, -amount);
    world.purse_add(owner, amount);
    world.stats.current.flow_tax -= amount;
}

/// L2 L11 (M17 plan O6): coins cross into the city from an outside
/// account: `from`'s treasury pays (never below 0), `to`'s purse gains,
/// the ledger column moves, `outside.inbound` grows. Untaxed (an export's
/// tax is the seller's explicit `pay`). Returns the coins moved.
pub fn cross_in(
    world: &mut World,
    from: crate::outside::OutsideId,
    to: Option<EntityId>,
    amount: i64,
    flow: Flow,
) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let Some(f) = world.outside.faction_mut(from) else { return 0 };
    let moved = amount.min(f.treasury.max(0));
    if moved <= 0 {
        return 0;
    }
    f.treasury -= moved;
    f.income_today -= moved;
    world.outside.inbound += moved;
    ledger(world, flow, moved);
    world.purse_add(to, moved);
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved;
    }
    moved
}

/// Real economy (plan E5): coins cross out of the city to an outside
/// account: `from`'s purse pays (`amount` in full for a corp or the city
/// with `full`, else capped at the purse), the account's treasury and
/// `income_today` gain, `outside.outbound` grows, the ledger column moves
/// (`ledger_out`). Untaxed. Returns the coins moved.
pub fn cross_out(
    world: &mut World,
    from: Option<EntityId>,
    to: crate::outside::OutsideId,
    amount: i64,
    flow: Flow,
    full: bool,
) -> i64 {
    if amount <= 0 || world.outside.faction(to).is_none() {
        return 0;
    }
    let moved = if full { amount } else { amount.min(world.purse(from).max(0)) };
    if moved <= 0 {
        return 0;
    }
    ledger_out(world, flow, moved);
    shadow_flow(world, from, None, moved, flow, false);
    world.purse_add(from, -moved);
    if let Some(f) = world.outside.faction_mut(to) {
        f.treasury += moved;
        f.income_today += moved;
    }
    world.outside.outbound += moved;
    moved
}

/// Plan E5: why a purse imports (the `Imported` event names the Food case).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ImportWhy {
    /// `assets::buy`'s D30 import of a new asset, a robot included.
    Asset,
    /// `assets::repairs`' missing Parts.
    Repair,
    /// A gang's cook or a Market's legal Stims restock.
    Stims,
    /// A self-installed ICE tier.
    Ice,
    /// E11: a Market's food from the World at the ask.
    Food,
}

/// Plan E5: an import, paid by `from`. With the market off it is
/// `charge(from, None, amount, Flow::Import)` exactly (the City's customs,
/// M13 D8); with it on the coins cross out to the World (`Flow::Import`,
/// `flow_import_out`) and customs `round(customs_rate × moved)` go to the
/// Treasury on top (`Flow::Customs`). Returns the coins that left for the good.
pub fn import(world: &mut World, from: Option<EntityId>, amount: i64, _why: ImportWhy) -> i64 {
    let full = matches!(owner_kind(world, from), OwnerKind::City | OwnerKind::Corp(_));
    let moved = cross_out(world, from, crate::outside::WORLD_ACCOUNT, amount, Flow::Import, full);
    if moved > 0 {
        let customs = (crate::systems::world_market::customs_rate(world) * moved as f32).round() as i64;
        if customs > 0 {
            charge(world, from, None, customs, Flow::Customs);
        }
    }
    moved
}

/// M16a (plan C5): coins from `from`'s purse into a contract record's
/// escrow (a game abstraction: a balance held on the record), capped as
/// `pay` (`min(amount, max(purse, 0))`). Ledger `Flow::Escrow` (capital: a
/// corp's posting is not a week of losses). Returns the coins moved.
pub fn escrow_in(world: &mut World, from: Option<EntityId>, id: crate::contract::ContractId, amount: i64) -> i64 {
    if amount <= 0 || !world.contracts.contains_key(&id) {
        return 0;
    }
    let moved = amount.min(world.purse(from).max(0));
    if moved <= 0 {
        return 0;
    }
    ledger(world, Flow::Escrow, moved);
    shadow_flow(world, from, None, moved, Flow::Escrow, false);
    world.purse_add(from, -moved);
    uncount_cashflow(world, from, moved);
    if let Some(c) = world.contracts.get_mut(&id) {
        c.escrow += moved;
    }
    world.escrow_held += moved;
    moved
}

/// M16a (plan C5): coins out of a record's escrow to `to` (`None` the
/// Treasury), at most what it holds: tax withheld when `flow` is taxed,
/// ledger `flow` (unless a refund, `Flow::Escrow`) and `Flow::Escrow`
/// negative; a gang payee's take is gang income. Returns the coins moved.
pub fn escrow_out(
    world: &mut World,
    id: crate::contract::ContractId,
    to: Option<EntityId>,
    amount: i64,
    flow: Flow,
) -> i64 {
    let held = world.contracts.get(&id).map_or(0, |c| c.escrow);
    let moved = amount.min(held);
    if moved <= 0 {
        return 0;
    }
    if let Some(c) = world.contracts.get_mut(&id) {
        c.escrow -= moved;
    }
    world.escrow_held -= moved;
    ledger(world, Flow::Escrow, -moved);
    if flow != Flow::Escrow {
        ledger(world, flow, moved);
    }
    shadow_flow(world, None, to, moved, flow, flow == Flow::Escrow);
    let tax = match to {
        Some(payee) if flow.taxed() => withhold(world, payee, moved),
        _ => 0,
    };
    world.purse_add(to, moved - tax);
    if tax > 0 {
        world.purse_add(None, tax);
        world.stats.current.flow_tax += tax;
    }
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved - tax;
    }
    if flow.capital() {
        uncount_cashflow(world, to, -(moved - tax));
    }
    if crate::systems::wages::on(world) {
        note_revenue(world, to, flow, moved - tax);
        crate::systems::wages::note_inflow(world, to, flow, moved - tax);
    }
    moved
}

/// Real economy E28 (the escrow precedent): coins from `from`'s purse
/// (`None` the Treasury) into a Mission's `Charity.purse`, capped as `pay`
/// for an agent or a gang, the full amount for a corp or the city. Ledger
/// `Flow::Donate` (untaxed). Returns the coins moved.
pub fn charity_in(world: &mut World, from: Option<EntityId>, mission: EntityId, amount: i64) -> i64 {
    if amount <= 0 || world.comp::<Building>(mission).is_none_or(|b| b.charity.is_none()) {
        return 0;
    }
    let full = matches!(owner_kind(world, from), OwnerKind::City | OwnerKind::Corp(_));
    let moved = if full { amount } else { amount.min(world.purse(from).max(0)) };
    if moved <= 0 {
        return 0;
    }
    ledger(world, Flow::Donate, moved);
    shadow_flow(world, from, Some(mission), moved, Flow::Donate, false);
    world.purse_add(from, -moved);
    if let Some(c) = world.comp_mut::<Building>(mission).and_then(|b| b.charity.as_mut()) {
        c.purse += moved;
    }
    moved
}

/// Real economy E27 (the escrow precedent): coins out of a Mission's purse
/// to `to` (`None` the Treasury), at most what it holds: tax withheld when
/// `flow` is taxed, the ledger line, a gang payee's take as gang income.
/// Returns the coins moved.
pub fn charity_out(world: &mut World, mission: EntityId, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    let held = world.comp::<Building>(mission).and_then(|b| b.charity.as_ref()).map_or(0, |c| c.purse);
    let moved = amount.min(held);
    if moved <= 0 {
        return 0;
    }
    if let Some(c) = world.comp_mut::<Building>(mission).and_then(|b| b.charity.as_mut()) {
        c.purse -= moved;
    }
    ledger(world, flow, moved);
    shadow_flow(world, Some(mission), to, moved, flow, false);
    let tax = match to {
        Some(payee) if flow.taxed() => withhold(world, payee, moved),
        _ => 0,
    };
    world.purse_add(to, moved - tax);
    if tax > 0 {
        world.purse_add(None, tax);
        world.stats.current.flow_tax += tax;
    }
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved - tax;
    }
    moved
}

/// Real economy E33 (phase 3a): pay `amount` (capped at the till) from the
/// Recycler's till to `to`, untaxed, with the ledger line (the escrow and
/// charity precedent). Returns the coins moved (an empty till pays 0).
pub fn till_out(world: &mut World, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    let moved = amount.min(world.econ.recycler_till);
    if moved <= 0 {
        return 0;
    }
    world.econ.recycler_till -= moved;
    ledger(world, flow, moved);
    shadow_flow(world, None, to, moved, flow, false);
    world.purse_add(to, moved);
    if crate::systems::wages::on(world) {
        crate::systems::wages::note_inflow(world, to, flow, moved);
    }
    moved
}

/// Gross coins earned through a building today.
pub fn credit(world: &mut World, building: EntityId, coins: i64) {
    if let Some(b) = world.comp_mut::<Building>(building) {
        b.revenue_today += coins;
    }
}

/// Coins in every purse: wallets, gang and corp treasuries, the Treasury,
/// (M13 D13) the coins on unsettled corpses, (M16a C5) the coins held in
/// contract records' escrow and (Real economy E25a, market on) a robbery's
/// loot in flight on an open hole.
pub fn total_coins(world: &World) -> i64 {
    let wallets: i64 = world.with::<Wallet>().iter().filter_map(|&a| world.comp::<Wallet>(a)).map(|w| w.coins).sum();
    let gangs: i64 = world.gangs().iter().filter_map(|&g| world.comp::<Gang>(g)).map(|g| g.treasury).sum();
    let corps: i64 = world.corps().iter().filter_map(|&c| world.comp::<Corp>(c)).map(|c| c.treasury).sum();
    let city: i64 = world.with::<Treasury>().iter().filter_map(|&t| world.comp::<Treasury>(t)).map(|t| t.coins).sum();
    let loot: i64 = world.loot_corpses.iter().filter_map(|&c| world.comp::<Corpse>(c)).map(|c| c.loot.coins).sum();
    let escrow: i64 = world.contracts.values().map(|c| c.escrow).sum();
    // Real economy (plan E25a): a robbery's coins in flight on an open hole
    // are coins held (as escrow is).
    let holes: i64 = world.holes.values().map(|h| h.loot).sum();
    // Real economy E26: the Missions' purses are coins held.
    let purses: i64 = crate::systems::charity::purses(world);
    // Real economy E33 (phase 3a): the Recycler's till (0 without `no_net`).
    let till = world.econ.recycler_till;
    wallets + gangs + corps + city + loot + escrow + holes + purses + till
}

// ---------------------------------------------------------------------------
// Buildings, values, roles
// ---------------------------------------------------------------------------

/// D28: what a building of `kind` sells for; 0 = not for sale.
pub fn value(world: &World, kind: BuildingKind) -> i64 {
    let c = &world.config.corps;
    match kind {
        BuildingKind::Bar => c.found_cost.bar,
        BuildingKind::Home => c.found_cost.home,
        BuildingKind::Farm => c.value.farm,
        BuildingKind::Market => c.value.market,
        BuildingKind::SecurityOffice => c.value.security_office,
        BuildingKind::Clinic => c.value.clinic,
        BuildingKind::Garage => c.value.garage,
        BuildingKind::Lab => c.value.lab,
        BuildingKind::Feed => c.value.feed,
        BuildingKind::Club => c.value.club,
        BuildingKind::Arcade => c.value.arcade,
        BuildingKind::NoodleBar => c.value.noodle_bar,
        BuildingKind::FightPit => c.value.fight_pit,
        BuildingKind::Den => c.value.den,
        BuildingKind::Lounge => c.value.lounge,
        BuildingKind::Fab => c.value.fab,
        BuildingKind::Fixer => c.value.fixer,
        // Real economy E26, E37.
        BuildingKind::Mission => c.value.mission,
        BuildingKind::Camp => c.value.camp,
        _ => 0,
    }
}

/// D17: the role an owned niche building employs.
pub fn role_for(kind: BuildingKind) -> Option<Role> {
    match kind {
        BuildingKind::Farm => Some(Role::Farmer),
        BuildingKind::Market => Some(Role::Clerk),
        BuildingKind::Bar => Some(Role::Bartender),
        BuildingKind::SecurityOffice => Some(Role::Guard),
        BuildingKind::Clinic => Some(Role::Ripperdoc),
        BuildingKind::Garage => Some(Role::Mechanic),
        BuildingKind::Lab => Some(Role::Researcher),
        BuildingKind::Feed => Some(Role::Reporter),
        // L2 L2: one role per kind.
        BuildingKind::Club => Some(Role::Host),
        BuildingKind::Arcade => Some(Role::Attendant),
        BuildingKind::NoodleBar => Some(Role::Cook),
        BuildingKind::FightPit => Some(Role::Fighter),
        BuildingKind::Den => Some(Role::Croupier),
        BuildingKind::Lounge => Some(Role::Concierge),
        BuildingKind::Fab => Some(Role::Fabber),
        // M16a (plan C8).
        BuildingKind::Fixer => Some(Role::Fixer),
        // Real economy E26 (a Camp has no staff role, E37).
        BuildingKind::Mission => Some(Role::Volunteer),
        _ => None,
    }
}

/// Everyone employed at one of `buildings` (ascending, as `Corp.buildings`),
/// by workplace, each list ascending; a building with no staff has no
/// entry. Jobs and room J3: read from the employer index
/// (`World::employers`), not a scan of every worker per call.
pub fn staff_by_building(world: &World, buildings: &[EntityId]) -> BTreeMap<EntityId, Vec<EntityId>> {
    buildings
        .iter()
        .filter_map(|&b| world.employers.get(&b).filter(|v| !v.is_empty()).map(|v| (b, v.clone())))
        .collect()
}

/// Everyone employed at `building`, ascending (J3: the employer index).
pub fn staff_at(world: &World, building: EntityId) -> Vec<EntityId> {
    world.staff_of(building).to_vec()
}

/// Everyone employed at one of a corp's buildings (its exec too, if they
/// hold a job there), ascending.
pub fn employees_of(world: &World, corp: EntityId) -> Vec<EntityId> {
    let Some(c) = world.comp::<Corp>(corp) else { return Vec::new() };
    let mut out: Vec<EntityId> = staff_by_building(world, &c.buildings).into_values().flatten().collect();
    out.sort_unstable();
    out
}

/// D30: a building changes hands. Employees and open vacancies stay with the
/// building; a Home's rent is re-set from the new owner at the next pass.
pub fn transfer_building(world: &mut World, b: EntityId, to: Option<EntityId>) {
    let Some(from) = world.comp::<Building>(b).map(|bd| bd.owner) else { return };
    if from == to {
        return;
    }
    if let Some(c) = from.and_then(|f| world.comp_mut::<Corp>(f)) {
        if let Ok(i) = c.buildings.binary_search(&b) {
            c.buildings.remove(i);
        }
    }
    if let Some(c) = to.and_then(|t| world.comp_mut::<Corp>(t)) {
        if let Err(i) = c.buildings.binary_search(&b) {
            c.buildings.insert(i, b);
        }
    }
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.owner = to;
    }
    // M14 V4: the plane relinks (a flag; nothing reads it with the plane off).
    crate::systems::virt::mark_dirty(world);
}

/// A new corp entity (tests and incorporation).
pub fn spawn_corp(
    world: &mut World,
    name: String,
    niches: BTreeSet<Niche>,
    treasury: i64,
    exec: Option<EntityId>,
) -> EntityId {
    let id = world.spawn();
    world.insert(id, Corp::new(name, niches, treasury, exec));
    // M14 V21: a new corp's tree is [1, 1, 1] with the focus of its first
    // niche (the seeded corps are re-seeded by name in `World::new`).
    let tech = crate::systems::tech::seeded_tech(world, id);
    if let Some(c) = world.comp_mut::<Corp>(id) {
        c.tech = crate::virt::Tech::seeded([1, 1, 1], tech.focus);
    }
    crate::systems::virt::mark_dirty(world);
    id
}

/// Queue a shock for the corp brain; once the pending severities reach
/// `[corps] shock_severity_rethink`, `corp_brain::run` rescores this tick.
pub fn push_corp_shock(world: &mut World, corp: EntityId, shock: CorpShock) {
    let threshold = world.config.corps.shock_severity_rethink;
    let Some(c) = world.comp_mut::<Corp>(corp) else { return };
    if c.shocks.len() >= SHOCK_CAP {
        c.shocks.remove(0);
    }
    c.shocks.push(shock);
    if c.shocks.iter().map(|s| s.severity()).sum::<f32>() >= threshold {
        world.corp_rethink = true;
    }
}

/// D20: a crime cost a corp-owned building `coins`; `culprit` is the actor.
pub fn note_loss(world: &mut World, building: EntityId, coins: i64, culprit: Option<EntityId>) {
    let Some(corp) = world.corp_of_building(building) else { return };
    let gang = culprit.and_then(|a| world.gang_of(a));
    let tick = world.tick;
    let extortion = world.comp::<Building>(building).is_some_and(|b| b.kind == BuildingKind::Home);
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.loss_log.push_back(CorpLoss { tick, coins, gang, building: Some(building) });
    }
    let shock = if extortion { CorpShock::Extorted } else { CorpShock::Robbed(coins) };
    push_corp_shock(world, corp, shock);
}

/// D45: an agent owner died or left; each building goes to the spouse, else
/// the first living adult child, else the city.
pub fn on_owner_gone(world: &mut World, agent: EntityId) {
    let owned: Vec<EntityId> = world
        .with::<Building>()
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(agent)))
        .collect();
    if owned.is_empty() {
        return;
    }
    let alive = |w: &World, id: EntityId| w.has::<Wallet>(id) && w.has::<Brain>(id) && !w.has::<Corpse>(id);
    let heir = world.spouse_of(agent).filter(|&s| s != agent && alive(world, s)).or_else(|| {
        crate::systems::demography::children_of_agent(world, agent)
            .into_iter()
            .filter(|&c| c != agent && alive(world, c) && crate::systems::demography::is_adult(world, c))
            .min()
    });
    let from = world.name_of(agent);
    for b in owned {
        transfer_building(world, b, heir);
        let what = world.name_of(b);
        let (actors, text) = match heir {
            Some(h) => (vec![h, b], format!("{what} inherited by {} from {from}", world.name_of(h))),
            None => (vec![b], format!("{what} passed to the city from {from}")),
        };
        world.push_event(EventKind::Acquired, &actors, text);
    }
}

/// The Nationalise lever: the Treasury buys a building at its value. A
/// derelict (M12 D26) has nobody to pay: the City repairs and re-lets it.
pub fn nationalise(world: &mut World, b: EntityId) -> Result<String, String> {
    let Some(bd) = world.comp::<Building>(b) else { return Err("no such building".into()) };
    let (kind, owner) = (bd.kind, bd.owner);
    if bd.derelict {
        let what = world.name_of(b);
        crate::systems::street::restore(world, b, None, "nationalised");
        return Ok(format!("the city took derelict {what} back"));
    }
    if owner.is_none() {
        return Err("already the city's".into());
    }
    let price = value(world, kind);
    if price <= 0 {
        return Err(format!("a {} is not for sale", kind.label()));
    }
    let coins = world.purse(None);
    if coins < price {
        return Err(format!("Treasury has {coins}, needs {price}"));
    }
    let seller = world.owner_label(owner);
    pay(world, None, owner, price, Flow::Sale);
    // As any sale: niches, contracts and staff follow (a nationalised
    // Security Office lets its guards go).
    crate::systems::corps::move_building(world, b, None);
    let what = world.name_of(b);
    let text = format!("the city nationalised {what} from {seller} for {price}");
    world.push_event(EventKind::Acquired, &[b], text.clone());
    Ok(text)
}

/// The Subsidise lever: Treasury -> a corp.
pub fn subsidise(world: &mut World, corp: EntityId, amount: i64) -> Result<String, String> {
    if !world.has::<Corp>(corp) {
        return Err("not a corp".into());
    }
    if amount <= 0 {
        return Err("bad amount".into());
    }
    let coins = world.purse(None);
    if coins < amount {
        return Err(format!("Treasury has {coins}, needs {amount}"));
    }
    charge(world, None, Some(corp), amount, Flow::Subsidy);
    Ok(format!("subsidised {} with {amount}", world.owner_label(Some(corp))))
}

// ---------------------------------------------------------------------------
// Seeding (the plan's Seeding table, D10-D12)
// ---------------------------------------------------------------------------

fn door(world: &World, b: EntityId) -> crate::components::TilePos {
    world.comp::<Building>(b).map_or_else(Default::default, |bd| bd.door)
}

/// Deal the opening ownership from `[corps]`. No RNG: the world stream is
/// untouched. Missing buildings are skipped (a row asking for more than exist
/// takes what is left).
pub fn seed(world: &mut World) {
    // Every Hideout belongs to its gang.
    for g in world.gangs() {
        if let Some(h) = world.hideout_of(g) {
            if let Some(b) = world.comp_mut::<Building>(h) {
                b.owner = Some(g);
            }
        }
    }
    let cfg = world.config.corps.clone();
    let n = cfg.row_count();
    let mut corps = Vec::with_capacity(n);
    for i in 0..n {
        let niches = cfg.niches_of(i);
        let treasury = cfg.treasury_initial[i];
        let id = spawn_corp(world, cfg.names[i].clone(), niches, treasury, None);
        if let Some(c) = world.comp_mut::<Corp>(id) {
            c.slot = Some(i as u8);
            c.treasury_ref = treasury.max(1);
            if cfg.megacorp.get(i).copied().unwrap_or(false) {
                c.parent = Some(crate::components::OUTSIDE_PARENT_BASE + i as u32);
                c.outside_treasury = cfg.outside_treasury_initial;
            }
        }
        corps.push(id);
    }
    // 1. Farms, config order, file order.
    let mut farms = world.buildings_of_kind(BuildingKind::Farm).to_vec().into_iter();
    for (i, &c) in corps.iter().enumerate() {
        for _ in 0..cfg.farms[i] {
            let Some(f) = farms.next() else { break };
            transfer_building(world, f, Some(c));
        }
    }
    // 2. Markets: by farms descending (ties config order), the unassigned
    //    Market nearest the corp's first Farm (ties lower id).
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(cfg.farms[i]), i));
    let mut free_markets: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Market).to_vec();
    for &i in &order {
        let c = corps[i];
        let first_farm = world.comp::<Corp>(c).and_then(|cc| {
            cc.buildings
                .iter()
                .copied()
                .find(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Farm))
        });
        for _ in 0..cfg.markets[i] {
            let pick = match first_farm {
                Some(f) => {
                    let from = door(world, f);
                    free_markets.iter().copied().min_by_key(|&m| (door(world, m).manhattan(from), m))
                }
                None => free_markets.first().copied(),
            };
            let Some(m) = pick else { break };
            free_markets.retain(|&x| x != m);
            transfer_building(world, m, Some(c));
        }
    }
    // 3. Blocks: one pool by (tier descending, file order); corps by blocks
    //    descending (ties config order) take the next `blocks[i]`.
    let mut pool: Vec<(std::cmp::Reverse<u8>, usize, EntityId)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .enumerate()
        // M12 D26: the seeded derelicts belong to nobody.
        .filter(|&(_, &h)| world.comp::<Building>(h).is_some_and(|b| !b.derelict))
        .filter_map(|(k, &h)| world.comp::<Building>(h).map(|b| (std::cmp::Reverse(b.tier), k, h)))
        .collect();
    pool.sort();
    let mut pool = pool.into_iter().map(|(_, _, h)| h);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(cfg.blocks[i]), i));
    for &i in &order {
        for _ in 0..cfg.blocks[i] {
            let Some(h) = pool.next() else { break };
            transfer_building(world, h, Some(corps[i]));
        }
    }
    // 4. Security Offices, config order, file order; each posts its guards.
    let mut offices = world.buildings_of_kind(BuildingKind::SecurityOffice).to_vec().into_iter();
    for (i, &c) in corps.iter().enumerate() {
        for _ in 0..cfg.offices[i] {
            let Some(o) = offices.next() else { break };
            transfer_building(world, o, Some(c));
            let roles = world.vacancies.entry(o).or_default();
            roles.extend(std::iter::repeat_n(Role::Guard, cfg.security_guards as usize));
        }
    }
    // 5. Bars: the Food corp with the most Farms takes the Bar nearest its
    //    Market; then the first agent owners (D11).
    let mut free_bars: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Bar).to_vec();
    let food_lead = (0..n)
        .filter(|&i| world.comp::<Corp>(corps[i]).is_some_and(|c| c.niches.contains(&Niche::Food)))
        .min_by_key(|&i| (std::cmp::Reverse(cfg.farms[i]), i));
    if let Some(i) = food_lead {
        let market = world.comp::<Corp>(corps[i]).and_then(|cc| {
            cc.buildings
                .iter()
                .copied()
                .find(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Market))
        });
        if let Some(m) = market {
            let from = door(world, m);
            if let Some(bar) = free_bars.iter().copied().min_by_key(|&b| (door(world, b).manhattan(from), b)) {
                free_bars.retain(|&x| x != bar);
                transfer_building(world, bar, Some(corps[i]));
            }
        }
    }
    let mut owners: BTreeSet<EntityId> = BTreeSet::new();
    for bar in free_bars.into_iter().take(cfg.bar_owner_count) {
        let from = door(world, bar);
        let pick = world
            .citizens()
            .into_iter()
            .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a) && !owners.contains(&a))
            .filter(|&a| crate::systems::demography::is_adult(world, a))
            .filter_map(|a| {
                let home = world.comp::<Household>(a).and_then(|h| h.home)?;
                Some((door(world, home).manhattan(from), a))
            })
            .min();
        let Some((_, owner)) = pick else { break };
        owners.insert(owner);
        transfer_building(world, bar, Some(owner));
        if let Some(w) = world.comp_mut::<Wallet>(owner) {
            w.coins = cfg.bar_owner_coins;
        }
    }
    // 6. Execs (D12): the greediest jobless adult per corp, config order.
    let mut execs: BTreeSet<EntityId> = BTreeSet::new();
    for &c in &corps {
        // L1: by age (`exec_min_age_years`), persuasion + knowledge, then
        // coins (the greediest jobless adult over 18 made a 19-year-old on
        // the dole Zetatech's exec); the M11 pick when nobody qualifies.
        let seasoned = crate::systems::life::pick_exec(
            world,
            world
                .citizens()
                .into_iter()
                .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a))
                .filter(|&a| !execs.contains(&a) && !owners.contains(&a)),
        );
        let pick = seasoned.or_else(|| {
            world
                .citizens()
                .into_iter()
                .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a))
                .filter(|&a| !execs.contains(&a) && !owners.contains(&a))
                .filter(|&a| crate::systems::demography::is_adult(world, a))
                .filter_map(|a| world.comp::<Personality>(a).map(|p| (p.greed, a)))
                .max_by(|x, y| x.0.total_cmp(&y.0).then(y.1.cmp(&x.1)))
                .map(|(_, a)| a)
        });
        if let Some(a) = pick {
            execs.insert(a);
            // L1: the exec lives in the Spire when a Block there has room.
            crate::systems::life::house_exec(world, a);
        }
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.exec = pick;
        }
    }
    // Opening rents: corp Blocks at level 1.0, city Blocks the lever.
    for h in world.buildings_of_kind(BuildingKind::Home).to_vec() {
        let r = rent_for(world, h);
        if let Some(b) = world.comp_mut::<Building>(h) {
            b.rent_per_day = r;
        }
    }
}

// ---------------------------------------------------------------------------
// The daily pass (plan 2.4)
// ---------------------------------------------------------------------------

/// A Home's rent today: the city's lever, a corp's `level × base[tier]`, an
/// agent landlord's `base[tier]`; every one under the rent cap.
pub fn rent_for(world: &World, home: EntityId) -> i64 {
    let Some(b) = world.comp::<Building>(home) else { return 0 };
    // M12 D25: a derelict is let to nobody.
    if b.demolished || b.derelict {
        return 0;
    }
    let tier = usize::from(b.tier.min(2));
    let base = world.config.rent.base[tier];
    let rent = match owner_kind(world, b.owner) {
        OwnerKind::City => world.levers.city_rent[tier],
        OwnerKind::Corp(c) => {
            let level = world.comp::<Corp>(c).map_or(1.0, |c| c.level(Niche::Housing));
            (level * base as f32).round() as i64
        }
        OwnerKind::Agent(_) | OwnerKind::Gang(_) => base,
    };
    let rent = rent.max(0);
    world.levers.rent_cap.map_or(rent, |cap| rent.min(cap.max(0)))
}

/// A Home's rent in tenths of a coin: what accrues (`rent_due` is
/// fractional). With `[economy] price_tenths` a corp's Housing level keeps
/// its fraction (Squeeze to 1.2 on a Mid Block is 2.4 a day, an Undercut to
/// 0.9 is 1.8, where `rent_for` rounds both back to 2); otherwise `rent_for`
/// x 10.
pub fn rent_tenths_for(world: &World, home: EntityId) -> i64 {
    let whole = rent_for(world, home) * 10;
    if !world.config.economy.price_tenths {
        return whole;
    }
    let Some(b) = world.comp::<Building>(home).filter(|b| !b.demolished && !b.derelict) else { return 0 };
    let OwnerKind::Corp(c) = owner_kind(world, b.owner) else { return whole };
    let base = world.config.rent.base[usize::from(b.tier.min(2))];
    let level = world.comp::<Corp>(c).map_or(1.0, |c| c.level(Niche::Housing));
    let t = ((level * base as f32 * 10.0).round() as i64).max(0);
    world.levers.rent_cap.map_or(t, |cap| t.min(cap.max(0) * 10))
}

/// D7: the owner's eviction threshold.
fn evict_days(world: &World, owner: Option<EntityId>) -> u8 {
    owner
        .and_then(|o| world.comp::<Corp>(o))
        .and_then(|c| c.evict_days_override)
        .unwrap_or(world.config.rent.evict_days)
}

/// Daily at midnight, before the economy's price step (D42).
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    let rents: Vec<(EntityId, i64)> =
        world.buildings_of_kind(BuildingKind::Home).iter().map(|&h| (h, rent_tenths_for(world, h))).collect();
    for &(h, _) in &rents {
        let r = rent_for(world, h);
        if let Some(b) = world.comp_mut::<Building>(h) {
            b.rent_per_day = r;
        }
    }
    // Under v1_profile and in a pre-M11 save every rent is 0: no rent, no
    // evictions, no re-housing (M9 behaviour). The rent pass still runs: a
    // rent cap of 0 forgives what was owed.
    collect_rent(world, &rents);
    if rents.iter().any(|&(_, r)| r > 0) {
        evictions(world);
        rehouse(world);
    }
    // Phase 5: the books close before the upkeep lump (`Corp.closing`).
    for c in world.corps() {
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.closing = cc.treasury;
        }
    }
    upkeep(world);
    exec_wages(world);
    // D27: an agent owning enough buildings becomes a corp (inheritance,
    // the seeded Bar owners); a `Register` checks its founder at once.
    crate::systems::founding::incorporate_daily(world);
    bar_vacancies(world);
    rolls(world);
}

/// Settle the rent already due, then accrue today's share: a coin that falls
/// due at midnight has the whole next day (and its wage or dole, when
/// `[rent] pay_from_income`) to be paid before it counts as arrears. `rents`
/// are in tenths (`rent_tenths_for`). A Home whose rent is 0 forgives what
/// its residents owe (a rent cap or `SetCityRent` to 0 left arrears that
/// never decayed); an owner living in their own Home pays nobody.
fn collect_rent(world: &mut World, rents: &[(EntityId, i64)]) {
    for &(home, rent) in rents {
        let owner = world.owner_of(home);
        if rent <= 0 || owner.is_some_and(|o| world.residents_of(home).contains(&o)) {
            let free: Vec<EntityId> =
                world.residents_of(home).iter().copied().filter(|&a| rent <= 0 || Some(a) == owner).collect();
            for a in free {
                if let Some(h) = world.comp_mut::<Household>(a) {
                    h.rent_due = 0.0;
                    h.arrears = 0;
                }
            }
            if rent <= 0 {
                continue;
            }
        }
        // The paying adults: an owner living in their own Home is not one,
        // so the Home still yields its whole rent from the others.
        let adults: Vec<EntityId> = world
            .residents_of(home)
            .iter()
            .copied()
            .filter(|&a| Some(a) != owner)
            .filter(|&a| world.has::<Brain>(a) && crate::systems::demography::is_adult(world, a))
            .collect();
        if adults.is_empty() {
            continue;
        }
        let share = rent as f32 / 10.0 / adults.len() as f32;
        for a in adults {
            settle_rent(world, a, home, true);
            if let Some(h) = world.comp_mut::<Household>(a) {
                h.rent_due += share;
            }
        }
    }
}

/// Pay the whole coins of rent due from the wallet. At midnight
/// (`midnight`), a short payment is a day of arrears (`RentShort` on the
/// first); a full one clears them.
fn settle_rent(world: &mut World, a: EntityId, home: EntityId, midnight: bool) {
    let Some(due) = world.comp::<Household>(a).map(|h| h.rent_due.floor() as i64) else { return };
    if due < 1 {
        // Not a whole coin owed: not in arrears.
        if midnight {
            if let Some(h) = world.comp_mut::<Household>(a) {
                h.arrears = 0;
            }
        }
        return;
    }
    // An owner housed in their own Home (an heir, a founder) owes nobody.
    if world.owner_of(home) == Some(a) {
        if let Some(h) = world.comp_mut::<Household>(a) {
            h.rent_due = 0.0;
            h.arrears = 0;
        }
        return;
    }
    let owner = world.owner_of(home);
    let mut paid = pay(world, Some(a), owner, due, Flow::Rent);
    // The transition wave (phase 3a, `[economy2] household_rent`, no_net
    // only): a co-resident spouse covers a short share from their own
    // wallet before it counts as arrears (the household's purse, not a
    // transfer from outside it).
    if paid < due && midnight && household_rent(world) {
        if let Some(s) = world
            .spouse_of(a)
            .filter(|&s| !world.has::<Corpse>(s) && world.comp::<Household>(s).and_then(|h| h.home) == Some(home))
        {
            paid += pay(world, Some(s), owner, due - paid, Flow::Rent);
        }
    }
    credit(world, home, paid);
    world.stats.current.rent_paid += paid;
    let today = world.day();
    let Some(h) = world.comp_mut::<Household>(a) else { return };
    h.rent_due -= paid as f32;
    h.note_rent_paid(today, paid);
    if paid >= due {
        h.arrears = 0;
        return;
    }
    // Inside the Precinct the debt runs on but the clock stops: an evicted
    // prisoner's spouse went with them, and the jailed were most evictions.
    if !midnight || world.has::<Sentence>(a) {
        return;
    }
    let Some(h) = world.comp_mut::<Household>(a) else { return };
    h.arrears = h.arrears.saturating_add(1);
    let first = h.arrears == 1;
    world.stats.current.rent_short += 1;
    let subject = owner.filter(|&o| world.has::<Identity>(o));
    world.remember(a, MemoryKind::RentShort, subject, 0.4, -0.4, false);
    if first {
        let name = world.name_of(a);
        let label = world.owner_label(owner);
        world.push_event(
            EventKind::RentShort,
            &[a, home],
            format!("{name} could not pay the rent on {} to {label} ({paid} of {due})", world.name_of(home)),
        );
    }
}

/// The transition wave (phase 3a): a co-resident spouse covers a short
/// rent share with `[economy2] household_rent` and `no_net`.
fn household_rent(world: &World) -> bool {
    world.config.economy2.household_rent && crate::systems::econ::no_net(world)
}

/// `[rent] pay_from_income`: rent comes out of a wage or the dole the moment
/// it is collected, before it can go on food (`economy::collect_wage`,
/// `collect_dole`).
///
/// Violence fix 4 (`[life] vf_rent_meal`): an agent in arrears keeps a
/// meal's worth (`World::mean_price`) of its purse; the rest pays. Arrears
/// built over unpaid first days swallowed a new guard's whole wage before
/// food, and the guard starved housed (seed 43, year 3).
pub fn pay_rent_from_income(world: &mut World, agent: EntityId) {
    if !world.config.rent.pay_from_income {
        return;
    }
    let Some(home) = world.comp::<Household>(agent).and_then(|h| h.home) else { return };
    let in_arrears = world.comp::<Household>(agent).is_some_and(|h| h.arrears > 0);
    let keep = if in_arrears && crate::systems::fixes::rent_meal(world) {
        world.mean_price().max(0).min(world.comp::<crate::components::Wallet>(agent).map_or(0, |w| w.coins.max(0)))
    } else {
        0
    };
    // The meal is held aside while the rent settles (no flow: the coins
    // never leave the purse).
    if keep > 0 {
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(agent) {
            w.coins -= keep;
        }
    }
    settle_rent(world, agent, home, false);
    if keep > 0 {
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(agent) {
            w.coins += keep;
        }
    }
}

fn evictions(world: &mut World) {
    let no_city = world.levers.no_city_evictions;
    let mut due: Vec<EntityId> = Vec::new();
    for home in world.buildings_of_kind(BuildingKind::Home).to_vec() {
        let owner = world.owner_of(home);
        if owner.is_none() && no_city {
            continue;
        }
        let limit = evict_days(world, owner);
        due.extend(
            world
                .residents_of(home)
                .iter()
                .copied()
                .filter(|&a| world.has::<Brain>(a) && world.comp::<Household>(a).is_some_and(|h| h.arrears >= limit)),
        );
    }
    for a in due {
        // A spouse already put out with their partner is no longer housed.
        if world.comp::<Household>(a).is_some_and(|h| h.home.is_some()) {
            evict(world, a, "arrears");
        }
    }
    let horizon = world.tick.saturating_sub(EVICTION_LOG_DAYS * TICKS_PER_DAY);
    while world.eviction_log.front().is_some_and(|&t| t < horizon) {
        world.eviction_log.pop_front();
    }
}

/// Put someone out of their Home where they stand.
fn put_out(world: &mut World, who: EntityId, home: EntityId, outside: crate::components::TilePos) {
    world.set_home(who, None);
    if world.comp::<Position>(who).is_some_and(|p| p.building == Some(home)) {
        if world.has::<Brain>(who) {
            world.abort_plan(who);
        }
        world.remove_from_building(who);
        let tick = world.tick;
        if let Some(p) = world.comp_mut::<Position>(who) {
            p.tile = outside;
            p.building = None;
            p.entered = tick;
        }
    }
}

/// D8: evict `agent` (and a spouse in the same Home, and the children of the
/// evicted whose other parent is not staying).
pub fn evict(world: &mut World, agent: EntityId, reason: &str) {
    let Some(home) = world.comp::<Household>(agent).and_then(|h| h.home) else { return };
    let Some(outside) = world.comp::<Building>(home).map(|b| world.outside_door(b)) else { return };
    let owner = world.owner_of(home);
    let tick = world.tick;
    let mut adults = vec![agent];
    if let Some(s) = world.spouse_of(agent) {
        if world.comp::<Household>(s).is_some_and(|h| h.home == Some(home)) && world.has::<Brain>(s) {
            adults.push(s);
        }
    }
    let staying: Vec<EntityId> =
        world.residents_of(home).iter().copied().filter(|r| !adults.contains(r) && !world.has::<Child>(*r)).collect();
    let children: Vec<EntityId> = world
        .residents_of(home)
        .iter()
        .copied()
        .filter(|&c| world.has::<Child>(c))
        .filter(|&c| adults.iter().any(|&a| crate::systems::demography::is_child_of(world, c, a)))
        .filter(|&c| !staying.iter().any(|&p| crate::systems::demography::is_child_of(world, c, p)))
        .collect();
    let label = world.owner_label(owner);
    let place = world.name_of(home);
    for &a in &adults {
        put_out(world, a, home, outside);
        if let Some(h) = world.comp_mut::<Household>(a) {
            h.arrears = 0;
            h.rent_due = 0.0;
            h.evicted_by = Some((owner, tick));
        }
        // M15 W5: with the word on a corp landlord is the memory's subject too.
        let subject = owner.filter(|&o| world.has::<Identity>(o) || world.has::<Corp>(o));
        world.remember(a, MemoryKind::Evicted, subject, 0.8, -0.8, false);
        let d = world.district_of_building(home);
        crate::systems::gossip::post_deed(world, d, crate::word::Deed::Evicted, subject, Some(a));
        if let OwnerKind::Agent(o) = owner_kind(world, owner) {
            crate::systems::social::adjust(world, a, o, -0.3, 0.0);
        }
        let name = world.name_of(a);
        let mut actors = vec![a, home];
        if let OwnerKind::Agent(o) = owner_kind(world, owner) {
            actors.push(o);
        }
        world.push_event(EventKind::Evicted, &actors, format!("{name} was evicted from {place} by {label} ({reason})"));
        world.eviction_log.push_back(tick);
        crate::systems::districts::note_eviction(world, home, owner);
        world.stats.current.evictions += 1;
    }
    for c in children {
        put_out(world, c, home, outside);
    }
    // M12 D17: belongings on the street at the Block door.
    crate::systems::litter::deposit(world, outside, 24, 1);
}

/// Homeless adults move in where they can pay `rehouse_coins_mult × rent`,
/// nearest door first, with a homeless spouse and their homeless children.
fn rehouse(world: &mut World) {
    let mult = world.config.rent.rehouse_coins_mult;
    let refuse = world.config.rent.refuse_days * TICKS_PER_DAY;
    let wait = world.config.rent.rehouse_wait_days * TICKS_PER_DAY;
    let tick = world.tick;
    // Phase 5: an evictee sleeps rough `rehouse_wait_days` before anyone
    // takes them in (re-housing was the same night, so no Dreg ever existed).
    // M12 D28: so does anyone put on the street without an eviction (a
    // derelict's residents, the seeded homeless).
    let waiting = |w: &World, a: EntityId| {
        let h = w.comp::<Household>(a);
        wait > 0
            && (h.and_then(|h| h.evicted_by).is_some_and(|(_, t)| tick < t + wait)
                || h.and_then(|h| h.homeless_since).is_some_and(|t| tick < t + wait))
    };
    let homeless: Vec<EntityId> = world
        // scan-ok: daily: re-housing
        .citizens()
        .into_iter()
        .filter(|&a| world.comp::<Household>(a).is_some_and(|h| h.home.is_none() && h.arrears == 0))
        .filter(|&a| !waiting(world, a))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| !b.emigrating))
        .filter(|&a| !world.has::<Sentence>(a) && crate::systems::demography::is_adult(world, a))
        .collect();
    if homeless.is_empty() {
        return;
    }
    // Each Home's own capacity (a built Home's is its interior's).
    let homes: Vec<(EntityId, crate::components::TilePos, Option<EntityId>, usize)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter_map(|&h| {
            world
                .comp::<Building>(h)
                .filter(|b| !b.demolished && !b.derelict)
                .map(|b| (h, b.door, b.owner, usize::from(b.capacity)))
        })
        .collect();
    for a in homeless {
        // Housed already, as someone's spouse, earlier in this pass.
        if world.comp::<Household>(a).is_none_or(|h| h.home.is_some()) {
            continue;
        }
        let spouse = world
            .spouse_of(a)
            .filter(|&s| world.has::<Brain>(s) && world.comp::<Household>(s).is_some_and(|h| h.home.is_none()));
        let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
            .into_iter()
            .filter(|&c| world.has::<Child>(c) && world.comp::<Household>(c).is_some_and(|h| h.home.is_none()))
            .collect();
        let need = 1 + usize::from(spouse.is_some()) + kids.len();
        let coins = world.comp::<Wallet>(a).map_or(0, |w| w.coins);
        let refused_by = world.comp::<Household>(a).and_then(|h| h.evicted_by).filter(|&(_, t)| tick < t + refuse);
        let from = world.comp::<Position>(a).map_or_else(Default::default, |p| p.tile);
        let pick = homes
            .iter()
            .filter(|&&(h, _, owner, cap)| {
                world.residents_of(h).len() + need <= cap
                    && coins >= mult * rent_for(world, h)
                    && refused_by.is_none_or(|(o, _)| o != owner)
            })
            .map(|&(h, d, _, _)| (d.manhattan(from), h))
            .min();
        let Some((_, home)) = pick else { continue };
        let movers: Vec<EntityId> = std::iter::once(a).chain(spouse).collect();
        let place = world.name_of(home);
        let label = world.owner_label(world.owner_of(home));
        for &m in &movers {
            // M12 D28: housed: the squat and the booking end (`set_home`).
            world.set_home(m, Some(home));
            let name = world.name_of(m);
            world.push_event(EventKind::Housed, &[m, home], format!("{name} moved into {place} ({label})"));
            world.stats.current.housed += 1;
        }
        let d = door(world, home);
        for c in kids {
            world.set_home(c, Some(home));
            if let Some(p) = world.comp_mut::<Position>(c) {
                p.tile = d;
                p.building = Some(home);
                p.entered = tick;
            }
        }
    }
}

/// D4: every non-city owner pays its buildings' upkeep to the Treasury.
fn upkeep(world: &mut World) {
    // Real economy phase 2 (plan E13): with wages on the property rate and
    // the daily power replace `[corps] upkeep` and the band's `upkeep_mult`.
    if crate::systems::wages::on(world) {
        crate::systems::wages::upkeep(world);
        return;
    }
    let up = world.config.corps.upkeep.clone();
    for b in world.with::<Building>() {
        let Some((kind, tier, owner)) =
            world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (bd.kind, bd.tier, bd.owner))
        else {
            continue;
        };
        let mut cost = up.for_building(kind, tier);
        if owner.is_none() || cost <= 0 {
            continue;
        }
        // L2 L10: the band scales corp upkeep (a branch: off reads `cost`).
        if owner.is_some_and(|o| world.has::<Corp>(o)) {
            cost = crate::systems::budget::upkeep_cost(world, cost);
        }
        let now = world.tick;
        if owner.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.upkeep_grace_until).is_some_and(|t| now < t) {
            continue;
        }
        charge(world, owner, None, cost, Flow::Upkeep);
    }
}

/// D27/D12: each exec draws their wage; a missing exec is replaced by the
/// corp's greediest adult employee.
fn exec_wages(world: &mut World) {
    for c in world.corps() {
        // L1: a salary scaled to the corp (`life::exec_pay`; the flat wage when off).
        let wage = crate::systems::life::exec_pay(world, c);
        let exec = world.comp::<Corp>(c).and_then(|cc| cc.exec);
        let living = exec.filter(|&e| world.has::<Wallet>(e) && world.has::<Brain>(e) && !world.has::<Corpse>(e));
        let exec = match living {
            Some(e) => Some(e),
            None => {
                let next = replacement_exec(world, c);
                if let Some(cc) = world.comp_mut::<Corp>(c) {
                    cc.exec = next;
                }
                if let Some(e) = next {
                    crate::systems::life::house_exec(world, e);
                }
                next
            }
        };
        if let Some(e) = exec {
            let paid = pay(world, Some(c), Some(e), wage, Flow::ExecWage);
            // Real economy phase 2 (plan E16): the exec's wage is payroll.
            note_payroll(world, Some(c), paid);
        }
    }
}

/// The greediest adult employee who is not already some corp's exec (ties
/// lower id), as `seed` and `corps::break_up` pick.
fn replacement_exec(world: &World, corp: EntityId) -> Option<EntityId> {
    let execs: BTreeSet<EntityId> =
        world.corps().into_iter().filter_map(|c| world.comp::<Corp>(c).and_then(|c| c.exec)).collect();
    // L1: the most seasoned employee first (as at seed), else the greediest.
    let seasoned =
        crate::systems::life::pick_exec(world, employees_of(world, corp).into_iter().filter(|a| !execs.contains(a)));
    if seasoned.is_some() {
        return seasoned;
    }
    let mut best: Option<(f32, EntityId)> = None;
    for a in employees_of(world, corp) {
        if execs.contains(&a) || !crate::systems::demography::is_adult(world, a) {
            continue;
        }
        let greed = world.comp::<Personality>(a).map_or(0.0, |p| p.greed);
        if best.is_none_or(|(g, id)| greed > g || (greed == g && a < id)) {
            best = Some((greed, a));
        }
    }
    best.map(|(_, a)| a)
}

/// D17: an agent-owned Bar short of bartenders posts one vacancy a day.
fn bar_vacancies(world: &mut World) {
    // L2 L3: `[jobs] bar_staff` with jobs on.
    let staff = crate::systems::jobs::full_staff(world, BuildingKind::Bar);
    if staff == 0 {
        return;
    }
    for bar in world.buildings_of_kind(BuildingKind::Bar).to_vec() {
        if !matches!(owner_kind(world, world.owner_of(bar)), OwnerKind::Agent(_)) {
            continue;
        }
        let working = staff_at(world, bar)
            .into_iter()
            .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == Role::Bartender))
            .count();
        let open = world.vacancies.get(&bar).map_or(0, |v| v.iter().filter(|&&r| r == Role::Bartender).count());
        if working + open < staff && open == 0 {
            world.vacancies.entry(bar).or_default().push(Role::Bartender);
        }
    }
}

/// Revenue, cashflow and loss windows; `negative_since`; the tax remainders
/// of payees that are gone (they leaked into the save).
fn rolls(world: &mut World) {
    let gone: Vec<EntityId> = world
        .tax_accum
        .keys()
        .copied()
        .filter(|&p| !(world.has::<Corp>(p) || world.has::<Gang>(p) || world.has::<Wallet>(p)))
        .collect();
    for p in gone {
        world.tax_accum.remove(&p);
    }
    for b in world.with::<Building>() {
        if let Some(bd) = world.comp_mut::<Building>(b) {
            if bd.revenue_today == 0 && bd.revenue.is_empty() {
                continue;
            }
            let today = std::mem::take(&mut bd.revenue_today);
            bd.revenue.push_back(today);
            while bd.revenue.len() > REVENUE_DAYS {
                bd.revenue.pop_front();
            }
        }
    }
    let now = world.tick;
    let horizon = now.saturating_sub(LOSS_DAYS * TICKS_PER_DAY);
    let wages = crate::systems::wages::on(world);
    for c in world.corps() {
        let Some(cc) = world.comp_mut::<Corp>(c) else { continue };
        let today = std::mem::take(&mut cc.cashflow_today);
        // Tick 0 has charged the first upkeep and earned nothing yet: not a
        // day of trading, so it is no cashflow entry (the brain read every
        // corp as losing money for two weeks).
        if now > 0 {
            cc.cashflow.push_back(today);
        }
        while cc.cashflow.len() > CASHFLOW_DAYS {
            cc.cashflow.pop_front();
        }
        // Real economy phase 2 (plan E16): the revenue and payroll windows
        // (wages on only, so a save and these rolls are untouched off).
        if wages {
            let (r, p) = (std::mem::take(&mut cc.rev_today), std::mem::take(&mut cc.pay_today));
            if now > 0 {
                cc.rev.push_back(r);
                cc.pay.push_back(p);
            }
            while cc.rev.len() > crate::systems::wages::WINDOW_DAYS {
                cc.rev.pop_front();
            }
            while cc.pay.len() > crate::systems::wages::WINDOW_DAYS {
                cc.pay.pop_front();
            }
            cc.import_today = 0;
        }
        // In the red at the close, before the upkeep lump (phase 5; was
        // after it, the intra-day trough). A corp at exactly 0 that took in
        // nothing today counts too (M11 review: god v2's Kessler, empty Sump
        // Blocks with no upkeep and an exec wage paid only from a positive
        // purse, sat at 0 for 60 days and could never go bankrupt).
        let stalled = cc.closing == 0 && today <= 0 && now > 0;
        if cc.closing < 0 || stalled {
            cc.negative_since.get_or_insert(now);
        } else {
            cc.negative_since = None;
        }
        while cc.loss_log.front().is_some_and(|l| l.tick < horizon) {
            cc.loss_log.pop_front();
        }
    }
}

/// Every building an owner holds, by owner (the app's legend; tests).
pub fn holdings(world: &World) -> BTreeMap<Option<EntityId>, Vec<EntityId>> {
    let mut out: BTreeMap<Option<EntityId>, Vec<EntityId>> = BTreeMap::new();
    for b in world.with::<Building>() {
        if let Some(bd) = world.comp::<Building>(b) {
            out.entry(bd.owner).or_default().push(b);
        }
    }
    out
}

/// A corp's buildings of one kind, ascending.
pub fn owned_of_kind(world: &World, owner: Option<EntityId>, kind: BuildingKind) -> Vec<EntityId> {
    world
        .buildings_of_kind(kind)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == owner && !bd.demolished))
        .collect()
}
