//! Action vocabulary and the Action table: who may take each action, its
//! symbolic preconditions and effects, and its personality-scaled cost.
//! Durations and real-world effects live in `exec::actions`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::components::{
    ActionInstance, Building, BuildingKind, GangMember, GoalKind, Household, Identity, Job, Personality, Position,
    Role, Skills, Wallet,
};
use crate::entity::EntityId;
use crate::goap::world_state::{LocationKey, WorldState, FOOD_COUNT_MAX};
use crate::time::{Season, Tick};
use crate::world::World;

mod ctx;
mod effects;

pub use ctx::*;

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
    /// Gang: wait at the own Hideout for the raid's departure.
    Muster,
    /// Gang: fight at the expedition's door (the rival Hideout, or the Jail
    /// under BreakOut); resolves the raid.
    Brawl,
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
    /// M11 § 6 / D25: at the Hall, pay `found_cost` and turn a vacant Lot
    /// into a Bar or a Home of one's own.
    Register,
    /// M12 D21: at a Hotel, pay for a bed until 08:00.
    CheckIn,
    /// M12 D27: move into a derelict building (a squatter; under a gang's
    /// Squat order, a blow of the gang's claim).
    Occupy,
    /// M13 D29: at an open seller, buy the `Brain.shop_pick`.
    BuyAsset,
    /// M13 D26: beat a street-parked vehicle's lock and drive off in it.
    StealVehicle,
    /// M13 D34: at a Clinic, have the gang's implant (reserved in the
    /// Hideout's stock) installed for `install_fee`.
    Install,
    /// M13 D34: at a Clinic, pay for Therapy (sanity + `therapy_gain`).
    Therapy,
    /// M13 D34: at a Clinic, sell the highest-load implant back.
    Uninstall,
    /// M13 D35: take a fresh body's coins, goods and packs.
    Strip,
    /// M13 D35/D36: rip a body's chrome (a corpse, or an abductee at the Hideout).
    Rip,
    /// M13 D36: drag the Harvest target off (a fight; the M9 escort drag).
    Abduct,
    /// M13 D38: a dealer takes a batch of Stims from its gang's Hideout.
    PickUp,
    /// M13 D38: a dealer's shift at the deal Bar (registered as a dealer there).
    Deal,
    /// M13 D38/D40: buy doses at a Stims source (a dealer, a legal Market).
    BuyStims,
    /// M13 D39: take a dose from the inventory.
    UseStim,
    /// M13 D39: at a Clinic, pay for Detox (addiction x `detox_mult`).
    Detox,
    /// M14 V11/V12: at the order's chair, start the run (a chain of dice
    /// contests on the Virt plane) and sit `JackedIn` until it ends.
    JackIn,
    /// M14 V17/V29: at a Data buyer's Lab, sell the deck's Data.
    SellData,
    /// M14 V36: at a deck seller, raise the carried deck one tier.
    UpgradeDeck,
    /// M15 W21: at the ask venue, put a social move to the co-occupant who
    /// knows the Hunt's target best; the answer is the Hunt's intel.
    /// Scripted only (never in `PLANNABLE`).
    AskAround,
    /// M15 W22/W35: wait at a place for someone (a Hunt's target, within 2
    /// tiles or in the same building) or over a body (GuardBody). Scripted
    /// only (never in `PLANNABLE`).
    StakeOut,
    /// L1: on the street, pick scrap and litter for the Recycler's coins
    /// (`[life] scavenge_coins`, paid by the Treasury): the broke's Earn.
    Scavenge,
    /// L1: an exec's office hours at the corp's HQ (a scripted Work plan,
    /// never in `PLANNABLE`): until `[life] exec_shift` ends.
    Meeting,
    /// L2 L8: a Fab Tech's shift at its Fab (the `FarmWork` shape): Parts.
    FabWork,
    /// L2 L8: a sweeper's shift on its beat's streets (`[jobs] sweep`).
    Sweep,
    // --- L2 phase 2 (plan L14): scripted steps of the Unwind, Socialise
    // (HangOut) and Lead plans and Preach's GangWork; never in `PLANNABLE`,
    // `allowed` false, started by `leisure::can_start`.
    /// At a Club, Arcade, FightPit or Lounge: the entry paid at start.
    Enjoy,
    /// At a Den (a table) or a FightPit (a bet on the bout).
    Gamble,
    /// At a NoodleBar: a meal paid at start.
    EatOut,
    /// At a street spot (`World::unwind`'s spot): an hour with whoever is there.
    HangOut,
    /// A gang leader's round: a front's week booked, then the Hideout's pay-out.
    Collect,
    /// A Purist member works the busiest spot: a Persuade on a co-hanger.
    Preach,
    // --- M16a (plan C10, C15): the Contract goal's scripted steps; never in
    // `PLANNABLE`, `allowed` false, started by `contracts::can_start`.
    /// At an open Fixer's office: become its regular (a stamp on its book).
    Network,
    /// At a Guard record's client: stand the day's post.
    Guard,
    // --- The Real economy (plan E27, E28): the charity's scripted steps;
    // never in `PLANNABLE`, `allowed` false, started by `charity::can_start`.
    /// At a Mission with stock: a free meal from the kitchen.
    EatAlms,
    /// At a Mission's door: a gift from the surplus into its purse.
    Donate,
    // --- Hunger as sickness § 4: the dive plan's scripted step; never in
    // `PLANNABLE`, `allowed` false, started by `bins::can_start`.
    /// At a bin outside a building's door: a meal of scraps, eaten there.
    DumpsterDive,
    /// Hunger as sickness § 3: at a Clinic, the cancer treated (the Treat
    /// goal's scripted step; never in `PLANNABLE`).
    Oncology,
    /// Hunger as sickness P5i: at Home, a pantry meal packed to carry to a
    /// shift (the commute plan's scripted step; never in `PLANNABLE`).
    PackMeal,
    /// The Hit market (H9): at an open Fixer's office, the want posted (the
    /// Commission goal's scripted step; never in `PLANNABLE`, started by
    /// `contracts::can_start`).
    Commission,
    /// M18 P5h: at a letting desk, the application for a place (the
    /// FindHome goal's scripted step; never in `PLANNABLE`, started by
    /// `housing::can_start`).
    Apply,
    /// Round 9 Z2: at an open Fixer's desk, a record off its board taken in
    /// person (the Contract goal's walk-in; never in `PLANNABLE`, started
    /// by `contracts::can_start`).
    TakeJob,
    /// M16b phase 2: at the holding building with an abductee in tow, the
    /// captive handed over (an Abduct plan's scripted step; never in
    /// `PLANNABLE`, started by `contracts::can_start`).
    HandOver,
    /// M16b phase 3 (spec § 5): beside a downed agent, take it up and drag
    /// it (the Rescue goal's scripted step; never in `PLANNABLE`).
    CarryWounded,
    /// M16b phase 3: at a Clinic with a Ripperdoc inside, the carried
    /// downed agent stabilised (the Rescue goal's scripted step).
    Stabilise,
    /// Jobs & lives phase 2 (§ 3): an hour at one's Pitch (or at the labour
    /// corner before the dawn pick): the Pitch goal's scripted step, never in
    /// `PLANNABLE`, started by `streetwork::can_start`.
    Vend,
    /// The pre-view pass phase 2 (§ 1 P7): a stations shift's mid-shift
    /// break, stood outside the workplace's door to `[shift] break_min`
    /// (a shape plan's scripted step, never in `PLANNABLE`, `systems/shift.rs`).
    Break,
    /// The pre-view pass phase 3 (§ 1, P4): a `haul` trip's load set down at
    /// its destination (`[shift.haul] haul_handle_min`; a courier's parcels
    /// handed over there, P3). A work block would lie: the body is not at
    /// its post. A shape plan's scripted step, never in `PLANNABLE`.
    Unload,
    /// The pre-view pass phase 6 (§ 0 P14, P15, P4): the gang leader holds
    /// court at the Hideout to `[leaders] court_min` (the lieutenants take
    /// orders, the members hand over their take), or gives a waiting order
    /// at the lieutenant (`order_give_min`). A Collect would lie: nothing is
    /// collected. A Lead plan's scripted step (`systems/leaders.rs`), never
    /// in `PLANNABLE`.
    Court,
    /// The pressure pass phase 2 (care): at a Clinic or a back room, a lost
    /// fight's wound patched or a cancer cut back (the Treat goal's scripted
    /// step, `customers::care_plan`; never in `PLANNABLE`).
    Patch,
}

/// Every action the planner may consider, in tie-break order.
pub const PLANNABLE: [ActionKind; 88] = [
    ActionKind::GoTo(LocationKey::Home),
    ActionKind::GoTo(LocationKey::Farm),
    ActionKind::GoTo(LocationKey::Market),
    ActionKind::GoTo(LocationKey::Bar),
    ActionKind::GoTo(LocationKey::Jail),
    ActionKind::GoTo(LocationKey::Cemetery),
    ActionKind::GoTo(LocationKey::Hall),
    ActionKind::GoTo(LocationKey::Hideout),
    ActionKind::GoTo(LocationKey::RaidTarget),
    ActionKind::GoTo(LocationKey::Warehouse),
    ActionKind::GoTo(LocationKey::Street),
    ActionKind::GoTo(LocationKey::TargetHome),
    ActionKind::GoTo(LocationKey::SuspectTile),
    ActionKind::GoTo(LocationKey::CorpseTile),
    ActionKind::GoTo(LocationKey::PatrolWaypoint),
    // M11 D13: a non-city job's wage desk, a private guard's office.
    ActionKind::GoTo(LocationKey::Workplace),
    // M12 D21, D27.
    ActionKind::GoTo(LocationKey::Hotel),
    ActionKind::GoTo(LocationKey::Squat),
    // M12 D37.
    ActionKind::GoTo(LocationKey::MusterPoint),
    // M13 D47 (a Mechanic's shift), D29, D26.
    ActionKind::GoTo(LocationKey::Garage),
    ActionKind::GoTo(LocationKey::Seller),
    ActionKind::GoTo(LocationKey::Vehicle),
    // M13 D34, D36.
    ActionKind::GoTo(LocationKey::Clinic),
    ActionKind::GoTo(LocationKey::Victim),
    // M13 D38/D40.
    ActionKind::GoTo(LocationKey::StimSource),
    // M14 V5/V29.
    ActionKind::GoTo(LocationKey::Chair),
    ActionKind::GoTo(LocationKey::DataBuyer),
    // The island pass phase 2 (OPEN-171): a Mission's cot.
    ActionKind::GoTo(LocationKey::Cot),
    ActionKind::EatFromInventory,
    ActionKind::EatAtHome,
    ActionKind::BuyFood,
    ActionKind::StealFood(StealSource::Market),
    ActionKind::StealFood(StealSource::Home),
    ActionKind::StealFood(StealSource::Warehouse),
    ActionKind::Forage,
    ActionKind::Beg,
    ActionKind::Sleep,
    ActionKind::Rest,
    ActionKind::FarmWork,
    ActionKind::ClerkWork,
    ActionKind::BartendWork,
    ActionKind::CollectWage,
    ActionKind::SellFood,
    ActionKind::Fence,
    ActionKind::Chat,
    ActionKind::Drink,
    ActionKind::Flirt,
    ActionKind::Propose,
    ActionKind::ReportCrime,
    ActionKind::PatrolLeg,
    ActionKind::Arrest,
    ActionKind::Escort,
    ActionKind::HideFromLaw,
    ActionKind::FleeToHome,
    ActionKind::Attack,
    ActionKind::JoinGang,
    ActionKind::Extort,
    ActionKind::SplitLoot,
    ActionKind::Muster,
    ActionKind::Brawl,
    ActionKind::BuryCorpse,
    ActionKind::CarryCorpse,
    ActionKind::Wander,
    ActionKind::CollectDole,
    ActionKind::GuardJail,
    ActionKind::TendGraves,
    ActionKind::Register,
    ActionKind::CheckIn,
    ActionKind::Occupy,
    ActionKind::BuyAsset,
    ActionKind::StealVehicle,
    ActionKind::Install,
    ActionKind::Therapy,
    ActionKind::Uninstall,
    ActionKind::Strip,
    ActionKind::Rip,
    ActionKind::Abduct,
    ActionKind::PickUp,
    ActionKind::Deal,
    ActionKind::BuyStims,
    ActionKind::UseStim,
    ActionKind::Detox,
    ActionKind::JackIn,
    ActionKind::SellData,
    ActionKind::UpgradeDeck,
    // L1.
    ActionKind::Scavenge,
    // L2 L8: planned after `commute_plan`'s walk, as every shift action.
    ActionKind::FabWork,
    ActionKind::Sweep,
];

impl ActionKind {
    /// The on-shift action for a role.
    pub fn work_for(role: Role) -> ActionKind {
        match role {
            Role::Farmer => ActionKind::FarmWork,
            Role::Guard => ActionKind::GuardJail,
            Role::Clerk => ActionKind::ClerkWork,
            Role::Bartender => ActionKind::BartendWork,
            Role::Gravedigger => ActionKind::TendGraves,
            // M12 D23 (phase 3 deviation): a sweeper's shift is worked at the
            // Recycler; the street credit is the midnight ledger.
            Role::Sanitation => ActionKind::TendGraves,
            // M13 D16: a Ripperdoc's or Mechanic's shift is the clerk's
            // counter shift at their own Clinic or Garage.
            Role::Ripperdoc | Role::Mechanic => ActionKind::ClerkWork,
            // M14 V16: a Researcher's shift is the clerk's at its own Lab.
            Role::Researcher => ActionKind::ClerkWork,
            // M15 W36: a Reporter's shift is the clerk's at its own Feed.
            Role::Reporter => ActionKind::ClerkWork,
            // L2 L2: the leisure staff clerk at their venue; the Fab Tech fabs.
            Role::Host | Role::Attendant | Role::Cook | Role::Fighter | Role::Croupier | Role::Concierge => {
                ActionKind::ClerkWork
            }
            Role::Fabber => ActionKind::FabWork,
            // M16a (plan C8): the Fixer's staff clerk at its office.
            Role::Fixer => ActionKind::ClerkWork,
            // Real economy E26: a Volunteer's shift is the clerk's at its Mission.
            Role::Volunteer => ActionKind::ClerkWork,
            // Jobs and room J13: a trade works the generic shift at its
            // workplace (no new action).
            Role::Trade(_) => ActionKind::ClerkWork,
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
                | ActionKind::FabWork
                | ActionKind::Sweep
        )
    }

    /// Actions with an execution implementation so far. The rest arrive
    /// with their milestone (M4 law, M5 social and gang, M6 corpses).
    pub fn implemented(self) -> bool {
        matches!(
            self,
            ActionKind::GoTo(_)
                | ActionKind::EatFromInventory
                | ActionKind::EatAtHome
                | ActionKind::BuyFood
                | ActionKind::StealFood(_)
                | ActionKind::Forage
                | ActionKind::Beg
                | ActionKind::Sleep
                | ActionKind::Rest
                | ActionKind::FarmWork
                | ActionKind::ClerkWork
                | ActionKind::BartendWork
                | ActionKind::GuardJail
                | ActionKind::TendGraves
                | ActionKind::CollectWage
                | ActionKind::CollectDole
                | ActionKind::SellFood
                | ActionKind::Drink
                | ActionKind::Wander
                | ActionKind::StoreFood
                | ActionKind::HaulToMarket
                | ActionKind::ReportCrime
                | ActionKind::PatrolLeg
                | ActionKind::Arrest
                | ActionKind::Escort
                | ActionKind::HideFromLaw
                | ActionKind::FleeToHome
                | ActionKind::Chat
                | ActionKind::Flirt
                | ActionKind::Propose
                | ActionKind::JoinGang
                | ActionKind::Extort
                | ActionKind::SplitLoot
                | ActionKind::Fence
                | ActionKind::Attack
                | ActionKind::CarryCorpse
                | ActionKind::BuryCorpse
                | ActionKind::Muster
                | ActionKind::Brawl
                | ActionKind::Register
                | ActionKind::CheckIn
                | ActionKind::Occupy
                | ActionKind::BuyAsset
                | ActionKind::StealVehicle
                | ActionKind::Install
                | ActionKind::Therapy
                | ActionKind::Uninstall
                | ActionKind::Strip
                | ActionKind::Rip
                | ActionKind::Abduct
                | ActionKind::PickUp
                | ActionKind::Deal
                | ActionKind::BuyStims
                | ActionKind::UseStim
                | ActionKind::Detox
                | ActionKind::JackIn
                | ActionKind::SellData
                | ActionKind::UpgradeDeck
                | ActionKind::Scavenge
                | ActionKind::Meeting
                | ActionKind::FabWork
                | ActionKind::Sweep
                | ActionKind::Enjoy
                | ActionKind::Gamble
                | ActionKind::EatOut
                | ActionKind::HangOut
                | ActionKind::Collect
                | ActionKind::Preach
                | ActionKind::Network
                | ActionKind::TakeJob
                | ActionKind::Guard
                | ActionKind::HandOver
                | ActionKind::CarryWounded
                | ActionKind::Stabilise
                | ActionKind::EatAlms
                | ActionKind::Donate
                | ActionKind::DumpsterDive
                | ActionKind::Oncology
                | ActionKind::Patch
                | ActionKind::PackMeal
                | ActionKind::Vend
                | ActionKind::Break
                | ActionKind::Unload
                | ActionKind::Court
        )
    }

    /// M16a (plan C10, C15): a Contract plan's own scripted step (never
    /// planned; started by `contracts::can_start`).
    pub fn is_contract_step(self) -> bool {
        matches!(
            self,
            ActionKind::Network
                | ActionKind::TakeJob
                | ActionKind::Guard
                | ActionKind::Commission
                | ActionKind::HandOver
        )
    }

    /// Real economy E27, E28: a charity plan's scripted step (never
    /// planned; started by `charity::can_start`).
    pub fn is_charity_step(self) -> bool {
        matches!(self, ActionKind::EatAlms | ActionKind::Donate)
    }

    /// L2 L14: a leisure plan's scripted step (never planned; started by
    /// `leisure::can_start`).
    pub fn is_leisure_step(self) -> bool {
        matches!(
            self,
            ActionKind::Enjoy
                | ActionKind::Gamble
                | ActionKind::EatOut
                | ActionKind::HangOut
                | ActionKind::Collect
                | ActionKind::Preach
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

/// Everything the Action table needs about the agent and its surroundings,
/// gathered once per plan (and once per step start for the precondition
/// re-check).
#[derive(Clone, Debug)]
pub struct PlanCtx {
    pub agent: EntityId,
    pub target: Option<EntityId>,
    /// The goal being planned (`build_for`); `None` (`build`, `build_light`)
    /// gates nothing and computes every input. M13 review: a goal's
    /// actions are offered only to the goals they serve, so `UseStim`,
    /// `BuyAsset` and the like stop widening every node of every search.
    pub goal: Option<GoalKind>,
    pub role: Option<Role>,
    pub in_gang: bool,
    pub adult: bool,
    pub lawfulness: f32,
    pub greed: f32,
    pub pride: f32,
    pub sociability: f32,
    pub courage: f32,
    pub stealth: f32,
    pub farming: f32,
    pub fighting: f32,
    pub hunger: f32,
    /// `hunger < 0.15`.
    pub starving: bool,
    pub dark: bool,
    /// A guard within 8 tiles.
    pub guard8: bool,
    pub season: Season,
    /// M11: the employer's owner (the Treasury for a city job) is below zero.
    pub payer_negative: bool,
    /// M11 D13: where CollectWage runs: `Hall` for a city job, else the
    /// employer's key (`Farm`, `Market`, `Bar`, or `Workplace`).
    pub wage_at: LocationKey,
    pub wage_collectable: bool,
    pub dole_available: bool,
    pub on_shift: bool,
    /// BartendWork may also run in the Evening.
    pub evening: bool,
    pub homeless: bool,
    /// The employer's location key, when a shift is pending (resting there is allowed).
    pub workplace: Option<LocationKey>,
    pub pantry: u32,
    pub target_pantry: u32,
    pub target_occupied: bool,
    pub market_stock: u32,
    pub warehouse_stock: u32,
    pub farm_stock: u32,
    pub coins: i64,
    pub price: i64,
    /// Carrying food that is not stolen (StoreFood moves only that).
    pub food_unstolen: bool,
    /// Market stock beyond what others have reserved.
    pub market_free: bool,
    pub haul_min_stock: u32,
    /// Wages owed right now (`days_unpaid >= 1`).
    pub wage_due: bool,
    /// A co-located partner for Chat (the plan target, in the same building;
    /// M18 P3: or beside the agent on the street, `street_partner`).
    pub partner: Option<EntityId>,
    /// M18 P3: the plan target stands on the street within `[talk]
    /// street_tiles` of the agent, both outside: Chat on the street.
    pub street_partner: bool,
    /// M18 P5 (Dylan 2026-11-03): the body is the player's on autopilot,
    /// safe idle: no step that changes its story ([`ActionKind::safe_idle_ok`]).
    pub safe_idle: bool,
    /// The bound partner is here or at a Bar / Market the plan can walk to.
    pub partner_reachable: bool,
    pub gang_eligible: bool,
    /// Extort: the bound Home has occupants and no guard within 8.
    pub extort_ok: bool,
    /// SplitLoot: loot taken today not yet split.
    pub has_loot: bool,
    /// Fence: carrying stolen food and the gang can pay.
    pub can_fence: bool,
    /// Attack: the bound target is within 4 tiles (or the same building).
    pub hostile_adjacent: bool,
    /// The bound target is an unburied corpse.
    pub corpse_target: bool,
    /// Gravedigger, kin of the corpse, or nobody else alive to do it.
    pub may_bury: bool,
    /// An open warrant on this agent.
    pub wanted: bool,
    /// The bound suspect has been seen by a guard recently.
    pub suspect_located: bool,
    /// Guards: today's shift is a Jail day (else Patrol).
    pub jail_day: bool,
    /// L2 L8: Sanitation sweeps the beat (`jobs::sweep_on`).
    pub sweep: bool,
    /// L2 L14: inside a Club or Den with `leisure::on` (a Drink there).
    pub leisure_drink: bool,
    /// L2 L14 (spec § 2): bored and broke, `U(fun)` (0 otherwise): Beg and
    /// Scavenge are cheaper by half of it (cheap fun is a reason to beg).
    pub fun_term: f32,
    /// Guards on a Patrol shift (the legs loop to its end, OPEN-280).
    pub patrol_pending: bool,
    pub hall_open: bool,
    /// Already had today's drink (one per day; the M1/M2 rule, kept).
    pub drank_today: bool,
    /// The Bar is at capacity right now: no point walking over.
    pub bar_full: bool,
    /// Raid: the gang's muster is called and within the gather window.
    pub raid_pending: bool,
    /// The agent's gang Hideout is sacked: no SplitLoot or Fence.
    pub hideout_sacked: bool,
    /// M11 D26: `founding::can_found` (eligible, can afford a kind, a Lot exists).
    pub can_found: bool,
    /// The agent sleeps and idles at the Hideout tonight (`gang::holes_up_at`):
    /// on the night watch, lying low, or homeless; never when it is sacked or full.
    pub holes_up: bool,
    /// M12 D21: homeless, and a bed tonight within means and `hotel_reach`.
    pub hotel_available: bool,
    /// M12 D27: has a squat.
    pub squatter: bool,
    /// M12 D27: the bound target is a derelict with a free slot this agent
    /// may take (the Squat goal), or the gang's Squat target.
    pub squat_ok: bool,
    /// M12 D38: a member serving the gang's Squat order on a derelict target.
    pub gang_squat: bool,
    /// M13 D29: a `Brain.shop_pick` is set and the target is its seller.
    pub shop_pick: bool,
    /// M13 D26: the bound target is a vehicle this agent may steal.
    pub vehicle_target: bool,
    /// M13 D26: a gang at the agent's Hideout would pay for a stolen vehicle.
    pub can_fence_vehicle: bool,
    /// M13 review: the agent holds a stolen vehicle it has not fenced
    /// (`vehicles::stolen_held_by`): a thief interrupted before the fence
    /// may still plan it.
    pub holds_stolen_vehicle: bool,
    /// M13 D26: `[vehicles] steal_vehicle_cost`.
    pub steal_vehicle_cost: f32,
    /// M13 D34: an implant of the agent's gang is reserved for it, and the
    /// target is a Clinic.
    pub install_ready: bool,
    /// M13 D34: the target is a Clinic and the agent can pay for Therapy.
    pub therapy_affordable: bool,
    /// M13 D34: any implant installed.
    pub has_implant: bool,
    /// M13 D35: the target is a body this agent may strip.
    pub loot_target: bool,
    /// M13 D35: the target body has chrome and this agent may rip it.
    pub loot_implants: bool,
    /// M13 D35: a member, or `courage >= rip_courage`.
    pub may_rip: bool,
    /// M13 D36: the target is the gang's Harvest target.
    pub victim_target: bool,
    /// M13 D33: in a cyberpsychotic episode.
    pub episode: bool,
    /// M13 D38: a dealer of its gang whose bound target is its deal Bar.
    pub dealer: bool,
    /// M13 D38: Stims in the agent's gang's Hideout.
    pub hideout_stims: u32,
    /// M13 D38: `[stims] deal_batch`.
    pub deal_batch: u32,
    /// M13 D38/D40: the target is a Stims source and the agent can pay a dose.
    pub stim_affordable: bool,
    /// M13 D40: the bound source is a legal Market (no crime in it).
    pub stim_legal: bool,
    /// M13 D39: the target is a Clinic, addiction drives the Treat score, and
    /// the agent can pay for Detox.
    pub detox_affordable: bool,
    /// M13 D39: in withdrawal, `withdrawal_steal_bonus` off a theft's cost
    /// (0 otherwise: a branch).
    pub withdrawal_bonus: f32,
    /// The pressure pass phase 1 (OPEN-178): from the agent's misery
    /// (`[misery]` absent: 0): a theft's cost `− steal_cost_cut × misery`, the
    /// vehicle theft's lawfulness gate `+ steal_lawful_cut × misery`.
    pub misery_steal_cut: f32,
    pub misery_lawful_cut: f32,
    /// Hunger fix G3: `[life] steal_can_buy` added to a food theft's cost
    /// while the agent can buy a meal (0 otherwise: a branch).
    pub steal_can_buy: f32,
    /// M14 V11: the agent holds a `RunOrder` and a deck.
    pub run_order: bool,
    /// M14 V29: the carried deck holds Data.
    pub has_data: bool,
    /// M14 V36: the shop pick is an `UpgradeDeck`.
    pub shop_upgrade: bool,
    /// L1: a member's Hideout is a bed nearer than Home (`life::hideout_bed`).
    pub hideout_bed: bool,
    /// L1: a housed agent far from Home can take a Hotel bed (`life::away_hotel`).
    pub away_hotel: bool,
    /// L1: exhausted with every bed far: Sleep where the agent stands.
    pub rough_ok: bool,
    /// L1: where Flee runs without a Home (`life::refuge`).
    pub refuge: Option<LocationKey>,
    /// L1: below a meal (`life::broke`): Beg and Scavenge are income.
    pub poor: bool,
    /// L2 shadow fixes item 19: on the street at a HangOut spot with
    /// company: Beg here.
    pub beg_spot: bool,
    /// Door-to-door Manhattan distance from the agent to each reachable key.
    pub dist: BTreeMap<LocationKey, u32>,
}

/// M18 P3: `a` and `b` both stand on the street within `[talk]
/// street_tiles` (Chebyshev) of each other: near enough to talk.
pub fn street_beside(world: &World, a: EntityId, b: EntityId) -> bool {
    let (Some(p), Some(q)) = (world.comp::<Position>(a), world.comp::<Position>(b)) else { return false };
    p.building.is_none()
        && q.building.is_none()
        && p.tile.x.abs_diff(q.tile.x).max(p.tile.y.abs_diff(q.tile.y)) <= world.config.talk.street_tiles
}
