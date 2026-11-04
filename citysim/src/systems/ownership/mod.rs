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

mod corps;
mod ledger;
mod rent;

pub use corps::*;
pub use ledger::*;
pub use rent::*;

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
        *self.probe.gang_in.entry("credit (raid, loot)".to_string()).or_insert(0) += coins;
        *self.probe.gang_in_by.entry(gang).or_insert(0) += coins;
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
    /// The Hit market (H5): a relative chips into a buyer's purse (untaxed; counted in `hm_pooled` by the caller).
    Pool,
    /// Transit (docs/TRANSIT.md T9): an El fare, rider -> Treasury (untaxed: the city's own).
    Fare,
    /// M16b § 4: coins paid under a coercion (protection, hush money, a
    /// called debt, a tribute, a fake job's fee), untaxed.
    Coerced,
    /// M16b phase 3: a Trauma subscription's weekly fee, client -> Security
    /// corp (taxed as owner revenue).
    Trauma,
    /// M17 phase 1: the World's Ambient book buying at a city Market or
    /// venue (untaxed at the crossing; the owner pays the sale's tax
    /// explicitly, as `Export`). Its column is the Ambient group's
    /// `flow_ambient` (`AmbientDay.paid_*`), so the ledger line is empty.
    AmbientSale,
    /// M17 phase 2: a branch's coins home to its parent (a corp's share of
    /// its day, a gang's weekly tribute), untaxed; capital. Its column is the
    /// Outside group's `outside_remit`.
    Remit,
    /// M17 phase 2: a parent's coins into a branch (a subsidy), untaxed;
    /// capital, never revenue. The Outside group's `outside_parent_fund`.
    ParentFund,
    /// M17 phase 3: the State's weekly grant for its guards, State ->
    /// Treasury (untaxed; not a receipt the tax band reads).
    Grant,
    /// M17 phase 4: scrip's coin side: a wage's scrip share to the parent,
    /// a redemption or a Fixer's conversion from it (untaxed).
    Scrip,
    // --- Jobs & lives (docs/JOBS_AND_LIVES.md § 1): appended; phase 1
    // writes Handling, Power and Build, the rest are their phases'.
    /// § 2: a fee per unit crossing a Freight Yard (an export, an import,
    /// a cross-owner haul), payer -> the Yard's owner (taxed).
    Handling,
    /// § 2: a building's daily power, owner -> the Utility Plant's owner (taxed).
    Power,
    /// § 2: a build, refit or floor, payer -> the Construction Yard's owner
    /// (taxed; capital for the payer, as `Found`).
    Build,
    /// § 5: a tip, customer -> the worker on duty (untaxed).
    Tip,
    /// § 3: a cab or rickshaw fare, rider -> the Garage's owner or the
    /// rickshaw's own purse (taxed). The spec's `Fare`: transit's El fare
    /// holds that name.
    CabFare,
    /// § 3: a Pitch's cut to the gang holding its district (untaxed).
    StreetCut,
    /// § 4, § 7: a licence or papers from the Civic Clerk -> Treasury (untaxed).
    Licence,
    /// § 7: the gray market's sales (untaxed).
    Gray,
    /// § 8: a follower's tithe into a church's purse (untaxed).
    Tithe,
    /// Lives at work phase 1 (§ 2.1): the madam's cut of a tip at her
    /// house, worker -> madam (untaxed; `Jobs2Cols.flow_madam_cut`).
    MadamCut,
    /// § 4 (phase 3): a dealer's weapon sale, buyer -> the gang, and the
    /// dealer's cut (untaxed, `Flow::Stims`'s shape).
    Arms,
    /// Lives at work phase 3 (§ 3.5): a parent's buy-out of a Camp child,
    /// parent -> the Camp's owner (untaxed; `flow_other` plus
    /// `Jobs2Cols.flow_buyout`, the `Fine` shape).
    Buyout,
    /// Round 6 V4 (`OPEN-240`): the house's share of a fare, the driver ->
    /// the Garage's owner (untaxed: the fare was taxed at the driver; the
    /// owner's revenue all the same, `note_revenue`). The rider pays the
    /// cab (`streetwork::pay_ride`); U7's `FareCut` ran the owner's cut the
    /// other way and the rider's line named the owner (`fabber_84414`: a
    /// stranger or a corp each leg, never a cabbie).
    CabTake,
    /// Round 8 X5 (`OPEN-217`): a bout's purse, the pit's owner -> the
    /// winner, off the table's take (untaxed; `flow_other`, counted in
    /// `Jobs2Cols.bout_purse_coins`). W6 booked it as `GambleWin`.
    Purse,
    /// The city milestone C6b (`systems::floors::rent_pass`): a tenancy's
    /// floor rent, its owner -> its host's owner, the landlord (the
    /// Treasury for a city-owned host); taxed as rent, its column
    /// `Jobs2Cols.flow_floor_rent`.
    FloorRent,
}

impl Flow {
    /// Capital, not trading: buying and selling buildings, founding one, a
    /// subsidy. Kept out of a corp's `cashflow` (the brain's `flow` input),
    /// or one purchase reads as two weeks of losses.
    pub fn capital(self) -> bool {
        matches!(
            self,
            Flow::Sale | Flow::Found | Flow::Subsidy | Flow::Escrow | Flow::Remit | Flow::ParentFund | Flow::Build
        )
    }

    /// Owner revenue taxed at the moment of the flow (spec § 3); wage tax
    /// keeps `Job.tax_accum`.
    pub fn taxed(self) -> bool {
        matches!(
            self,
            Flow::Food
                | Flow::Drink
                | Flow::Rent
                | Flow::FloorRent
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
                | Flow::Trauma
                | Flow::Handling
                | Flow::Power
                | Flow::Build
                | Flow::CabFare
        )
    }
}

/// Daily at midnight, before the economy's price step (D42).
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    let rents: Vec<(EntityId, i64)> = {
        let _t = crate::prof::scope!("own::rents");
        let rents: Vec<(EntityId, i64)> =
            world.buildings_of_kind(BuildingKind::Home).iter().map(|&h| (h, rent_tenths_for(world, h))).collect();
        for &(h, _) in &rents {
            let r = rent_for(world, h);
            if let Some(b) = world.comp_mut::<Building>(h) {
                b.rent_per_day = r;
            }
        }
        rents
    };
    // Under v1_profile and in a pre-M11 save every rent is 0: no rent, no
    // evictions, no re-housing (M9 behaviour). The rent pass still runs: a
    // rent cap of 0 forgives what was owed.
    crate::prof::timed!("own::collect_rent", collect_rent(world, &rents));
    if rents.iter().any(|&(_, r)| r > 0) {
        crate::prof::timed!("own::evictions", evictions(world));
        // Lives at work phase 2 (§ 3.4): the spouse's Home is tried first.
        crate::prof::timed!("own::household::reunite", crate::systems::household::reunite(world));
        crate::prof::timed!("own::household::tied_housing", crate::systems::household::tied_housing(world));
        // The island pass phase 3a (I6): the company beds' grace and
        // evictions, then the staff who move in, before the city's rehouse.
        crate::prof::timed!("own::company::evictions", crate::systems::company::evictions(world));
        // The island pass phase 3b: the weekly buyouts.
        crate::prof::timed!("own::company::buyouts", crate::systems::company::buyouts(world));
        crate::prof::timed!("own::company::move_in", crate::systems::company::move_in(world));
        // M18 P5h: `rehouse` retired: housing is the FindHome errand; the
        // Statistical applicants roll it here.
        crate::prof::timed!("own::housing::daily", crate::systems::housing::daily(world));
    }
    // Phase 5: the books close before the upkeep lump (`Corp.closing`).
    for c in world.corps() {
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.closing = cc.treasury;
        }
    }
    // The city milestone C6b: the tenancies' floor rent to their landlords.
    crate::prof::timed!("own::floors::rent_pass", crate::systems::floors::rent_pass(world));
    crate::prof::timed!("own::upkeep", upkeep(world));
    crate::prof::timed!("own::exec_wages", exec_wages(world));
    // D27: an agent owning enough buildings becomes a corp (inheritance,
    // the seeded Bar owners); a `Register` checks its founder at once.
    crate::prof::timed!("own::founding::incorporate_daily", crate::systems::founding::incorporate_daily(world));
    crate::prof::timed!("own::bar_vacancies", bar_vacancies(world));
    crate::prof::timed!("own::rolls", rolls(world));
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
            Some(e) => {
                // Round 4 T5 (OPEN-234): the backstop: an exec holding a post
                // (one held when the chair came, by any path) lets it go.
                crate::systems::search::leave_post_for_chair(world, e, c);
                Some(e)
            }
            None => {
                // Class pay: the corp's management, then its professionals,
                // first; the new exec leaves the post (re-posted).
                let ranked = crate::systems::classpay::exec_from_ranks(world, c);
                let next = ranked.or_else(|| replacement_exec(world, c));
                if let Some(cc) = world.comp_mut::<Corp>(c) {
                    cc.exec = next;
                }
                if let Some(e) = ranked {
                    let at = world.comp::<Job>(e).and_then(|j| j.employer);
                    let was = crate::systems::classpay::held(world, e);
                    crate::systems::classpay::leave_for_promotion(world, e);
                    crate::systems::classpay::note_promoted(world, e, at, was);
                } else if let Some(e) = next {
                    // Shadow V4 bug 5: the fallback pick leaves its post too
                    // (an exec kept her Guard post and never held office).
                    crate::systems::classpay::leave_for_promotion(world, e);
                }
                if let Some(e) = next {
                    crate::systems::life::house_exec(world, e);
                }
                next
            }
        };
        // Round 5 U7 (`OPEN-237`): an exec held in the cells keeps the chair
        // and draws no salary for the day (V13: a jailed exec paid his
        // ExecWage; a worker in the cells works no shift and is paid none).
        // Round 6 V3 (`OPEN-238`): nor one laid up wounded (V14 `ceo_91004
        // d25 00:00` +40c ExecWage on the Ripperdoc's floor).
        if let Some(e) = exec
            .filter(|&e| !world.has::<crate::components::Sentence>(e) && !crate::systems::wounded::is_down(world, e))
        {
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
    let classpay = crate::systems::classpay::on(world);
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
        // Class pay P3: the graded premium's window (class pay on only).
        if classpay {
            let q = std::mem::take(&mut cc.prem_today);
            if now > 0 {
                cc.prem.push_back(q);
            }
            while cc.prem.len() > crate::systems::wages::WINDOW_DAYS {
                cc.prem.pop_front();
            }
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
