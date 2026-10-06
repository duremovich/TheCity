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
}

/// Every action the planner may consider, in tie-break order.
pub const PLANNABLE: [ActionKind; 79] = [
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
    /// A co-located partner for Chat (the plan target, in the same building).
    pub partner: Option<EntityId>,
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
    /// Guards on a Patrol shift with legs left.
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
    /// Door-to-door Manhattan distance from the agent to each reachable key.
    pub dist: BTreeMap<LocationKey, u32>,
}

impl PlanCtx {
    /// Full context for planning (distances and the guard scan included).
    pub fn build(world: &World, agent: EntityId, target: Option<EntityId>) -> PlanCtx {
        PlanCtx::build_inner(world, agent, target, false)
    }

    /// Context for re-checking one action's preconditions at step start:
    /// skips the O(population) guard scan and the distance table, which
    /// only costs and GoTo need.
    pub fn build_light(world: &World, agent: EntityId, target: Option<EntityId>) -> PlanCtx {
        PlanCtx::build_inner(world, agent, target, true)
    }

    fn build_inner(world: &World, agent: EntityId, target: Option<EntityId>, light: bool) -> PlanCtx {
        let p = world.comp::<Personality>(agent);
        let s = world.comp::<Skills>(agent);
        let job = world.comp::<Job>(agent);
        let hunger = world.comp::<crate::components::Needs>(agent).map_or(1.0, |n| n.hunger);
        let pos = world.comp::<Position>(agent);
        let tile = pos.map(|p| p.tile);
        let home = world.comp::<Household>(agent).and_then(|h| h.home);
        let sight = world.config.crime.sight_day_crime; // "guard8"
        let guard8 = !light
            && tile.is_some_and(|t| {
                world
                    .guards()
                    .iter()
                    .any(|&g| g != agent && world.comp::<Position>(g).is_some_and(|gp| gp.tile.manhattan(t) <= sight))
            });
        // M10: the agent's own Market (the one it is in, else the nearest).
        let market = world.local(agent, BuildingKind::Market);
        let stock_of = |kind: BuildingKind| -> u32 {
            world.local(agent, kind).and_then(|b| world.comp::<Building>(b)).map_or(0, |b| b.stock_food)
        };
        let farm = job
            .and_then(|j| j.employer)
            .filter(|&e| world.comp::<Building>(e).is_some_and(|b| b.kind == BuildingKind::Farm));
        let tod = world.tick_of_day();
        let day = world.day();
        let wage_desk = job.and_then(|_| world.wage_desk(agent));
        let wage_at = match wage_desk.and_then(|d| world.comp::<Building>(d)) {
            Some(b) if b.kind != BuildingKind::Hall => LocationKey::of_building(b.kind),
            _ => LocationKey::Hall,
        };
        let payer = job.and_then(|j| j.employer).and_then(|e| world.owner_of(e));

        // Distances: every singleton building, the agent's Home, the employer's
        // workplace for Farm, and the bound target.
        let origin = pos.map(|p| match p.building.and_then(|b| world.comp::<Building>(b)) {
            Some(b) => world.outside_door(b),
            None => p.tile,
        });
        let mut dist = BTreeMap::new();
        if let (Some(o), false) = (origin, light) {
            dist.insert(LocationKey::Street, 0);
            if let Some(&(t, _)) = target.and_then(|s| world.last_seen.get(&s)) {
                dist.insert(LocationKey::SuspectTile, o.manhattan(t));
            }
            let mut add = |key: LocationKey, b: Option<EntityId>| {
                if let Some(door) = b.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) {
                    dist.insert(key, o.manhattan(door));
                }
            };
            add(LocationKey::Home, home);
            add(LocationKey::Farm, farm.or_else(|| world.local(agent, BuildingKind::Farm)));
            for kind in [
                BuildingKind::Market,
                BuildingKind::Bar,
                BuildingKind::Jail,
                BuildingKind::Cemetery,
                BuildingKind::Hall,
                BuildingKind::Warehouse,
            ] {
                add(LocationKey::of_building(kind), world.local(agent, kind));
            }
            add(LocationKey::Hideout, crate::systems::gang::hideout_for(world, agent));
            if wage_at == LocationKey::Workplace {
                add(LocationKey::Workplace, world.wage_desk(agent));
            }
            // M13 D47: a Mechanic's Garage (its employer).
            if job.is_some_and(|j| j.role == Role::Mechanic) {
                add(LocationKey::Garage, world.resolve_building(agent, LocationKey::Garage, None));
            }
            add(LocationKey::TargetHome, target);
            if let Some(b) = world
                .comp::<crate::components::Brain>(agent)
                .and_then(|b| b.patrol_route.get(usize::from(b.patrol_legs) % b.patrol_route.len().max(1)).copied())
            {
                add(LocationKey::PatrolWaypoint, Some(b));
            }
            // A corpse is reached at its tile, or through its building's door.
            if let Some(c) = target.filter(|&t| world.has::<crate::components::Corpse>(t)) {
                if let Some(p) = world.comp::<Position>(c) {
                    let there = p.building.and_then(|b| world.comp::<Building>(b)).map_or(p.tile, |b| b.door);
                    dist.insert(LocationKey::CorpseTile, o.manhattan(there));
                }
            }
            if let Some(t) = crate::systems::raid::target_tile(world, agent) {
                dist.insert(LocationKey::RaidTarget, o.manhattan(t));
            }
            // M12 D37: the muster point, while an expedition is pending.
            if crate::systems::raid::raid_pending(world, agent) {
                let there = match crate::systems::raid::muster_point(world, agent) {
                    Some(crate::systems::raid::MusterAt::Inside(b)) => world.comp::<Building>(b).map(|bd| bd.door),
                    Some(crate::systems::raid::MusterAt::Door(t)) => Some(t),
                    None => None,
                };
                if let Some(t) = there {
                    dist.insert(LocationKey::MusterPoint, o.manhattan(t));
                }
            }
        }

        // M13 D33/D36: the target is a living agent this agent hunts: its
        // episode's quarry, or its gang's Harvest target.
        let episode = crate::systems::chrome::in_episode(world, agent);
        let victim_target = target.is_some_and(|t| {
            world.gang_of(agent).is_some_and(|g| crate::systems::chrome::gang_harvest_target(world, g) == Some(t))
        });
        let victim_plan =
            target.is_some_and(|t| t != agent && crate::systems::law::living(world, t)) && (episode || victim_target);
        // M13 D34/D35: the Clinic and the body.
        let target_clinic =
            target.filter(|&t| world.comp::<Building>(t).is_some_and(|b| b.kind == BuildingKind::Clinic));
        let may_rip = crate::systems::chrome::may_rip(world, agent);
        let loot_target = target.is_some_and(|t| crate::systems::chrome::may_loot(world, agent, t));
        let loot_implants = may_rip
            && target.is_some_and(|t| {
                world.comp::<crate::components::Corpse>(t).is_some_and(|c| !c.buried)
                    && !crate::systems::chrome::installed(world, t).is_empty()
            });
        // M13 D38-D40: dealing, buying and Detox.
        let stims_on = world.config.assets.enabled;
        let gang_hideout = world.gang_of(agent).and_then(|g| world.hideout_of(g));
        let dealer = stims_on && target.is_some() && crate::systems::stims::dealer_target(world, agent) == target;
        let source_price =
            if stims_on { target.and_then(|t| crate::systems::stims::source_price(world, t)) } else { None };
        let stim_source = source_price.is_some();
        let coins_now = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
        let detox_pick = stims_on && target_clinic.is_some() && crate::systems::stims::detox_drives(world, agent);
        let withdrawal_bonus = if stims_on && crate::systems::stims::in_withdrawal(world, agent) {
            world.config.stims.withdrawal_steal_bonus
        } else {
            0.0
        };
        // M12 D21/D27: the street's rungs (homeless agents only; cheap scans
        // over the few Hotels and derelicts).
        let homeless = home.is_none();
        let hotel = if homeless { crate::systems::street::hotel_for(world, agent) } else { None };
        let squat = world.comp::<crate::components::Squatter>(agent).map(|s| s.building);
        let derelict_target = target.filter(|&t| crate::systems::street::is_derelict(world, t));
        let gang_squat = derelict_target.is_some()
            && world.gang_of(agent).is_some_and(|g| {
                world.comp::<crate::components::Gang>(g).is_some_and(|x| x.order == crate::components::Order::Squat)
            });
        let squat_ok = derelict_target.is_some_and(|t| {
            gang_squat
                || (homeless
                    && crate::systems::street::squat_slots(world, t) > 0
                    && !crate::systems::street::squat_banned(world, agent, t))
        });
        if let (Some(o), false) = (origin, light) {
            let mut add = |key: LocationKey, b: Option<EntityId>| {
                if let Some(door) = b.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) {
                    dist.insert(key, o.manhattan(door));
                }
            };
            add(LocationKey::Hotel, hotel);
            add(LocationKey::Squat, squat.or(derelict_target));
            // M13 D29: the bound seller (a Garage, a Market for a pack).
            // M13 D38: a dealer's deal Bar too.
            if target.and_then(|t| world.comp::<Building>(t)).is_some_and(|b| {
                matches!(b.kind, BuildingKind::Garage | BuildingKind::Clinic | BuildingKind::Market | BuildingKind::Bar)
            }) {
                add(LocationKey::Seller, target);
            }
            // M13 D38/D40: the bound Stims source.
            if stim_source {
                add(LocationKey::StimSource, target);
            }
            // M13 D34: the bound Clinic, else a Ripperdoc's employer.
            let clinic = target
                .filter(|&t| world.comp::<Building>(t).is_some_and(|b| b.kind == BuildingKind::Clinic))
                .or_else(|| {
                    job.filter(|j| j.role == Role::Ripperdoc)
                        .and_then(|_| world.resolve_building(agent, LocationKey::Clinic, None))
                });
            add(LocationKey::Clinic, clinic);
            // M13 D26: the bound vehicle's door, from the street outside it.
            if let Some(t) = target.and_then(|t| crate::systems::vehicles::vehicle_stand(world, t)) {
                dist.insert(LocationKey::Vehicle, o.manhattan(t));
            }
            // M13 D33/D36: the bound quarry (an episode's, a Harvest crew's).
            if victim_plan {
                if let Some(p) = target.and_then(|t| world.comp::<Position>(t)) {
                    let there = p.building.and_then(|b| world.comp::<Building>(b)).map_or(p.tile, |b| b.door);
                    dist.insert(LocationKey::Victim, o.manhattan(there));
                }
            }
        }

        let (target_pantry, target_occupied) = target
            .and_then(|t| world.comp::<Building>(t))
            .map_or((0, false), |b| (b.stock_food, !b.occupants.is_empty()));

        let in_gang = world.has::<GangMember>(agent);
        let hideout_sacked = world
            .gang_of(agent)
            .and_then(|g| world.comp::<crate::components::Gang>(g))
            .is_some_and(|g| g.is_sacked(world.tick));

        PlanCtx {
            agent,
            target,
            role: job.map(|j| j.role),
            in_gang,
            adult: world.comp::<Identity>(agent).is_some_and(|i| i.age_days >= 18 * crate::time::DAYS_PER_YEAR as u32),
            lawfulness: p.map_or(0.5, |p| p.lawfulness),
            greed: p.map_or(0.5, |p| p.greed),
            pride: p.map_or(0.5, |p| p.pride),
            sociability: p.map_or(0.5, |p| p.sociability),
            courage: p.map_or(0.5, |p| p.courage),
            stealth: s.map_or(0.2, |s| s.stealth),
            farming: s.map_or(0.2, |s| s.farming),
            fighting: s.map_or(0.2, |s| s.fighting),
            hunger,
            starving: hunger < 0.15,
            dark: world.is_dark(),
            guard8,
            season: world.season(),
            payer_negative: world.purse(payer) < 0,
            wage_at,
            // Whether wages are owed is symbolic (`has_wage_due`); the context
            // carries only today's short-payment block.
            wage_collectable: job.is_some_and(|j| j.last_wage_attempt_day != Some(day)),
            dole_available: job.is_none()
                && world.levers.dole_per_day > 0
                && world.treasury().is_some_and(|t| t.coins >= 0)
                && world.comp::<crate::components::Brain>(agent).is_some_and(|b| b.last_dole_day != Some(day)),
            on_shift: job.is_some_and(|j| j.on_shift(tod)),
            evening: world.phase() == crate::time::DayPhase::Evening,
            homeless: home.is_none(),
            workplace: job
                .filter(|j| crate::exec::routine::shift_pending(world, agent, j))
                .map(|j| crate::exec::routine::workplace_key_for(world, agent, j)),
            pantry: home.and_then(|h| world.comp::<Building>(h)).map_or(0, |b| b.stock_food),
            target_pantry,
            target_occupied,
            market_stock: stock_of(BuildingKind::Market),
            warehouse_stock: stock_of(BuildingKind::Warehouse),
            farm_stock: farm.and_then(|f| world.comp::<Building>(f)).map_or(0, |b| b.stock_food),
            coins: world.comp::<Wallet>(agent).map_or(0, |w| w.coins),
            price: market.map_or(1, |m| world.price_for(m, agent)).max(1),
            food_unstolen: world.comp::<crate::components::Inventory>(agent).is_some_and(|i| i.food > i.stolen_food),
            market_free: market
                .and_then(|m| world.comp::<Building>(m).map(|b| (m, b.stock_food)))
                .is_some_and(|(m, stock)| stock > WorldState::reserved_by_others(world, m, Some(agent))),
            haul_min_stock: world.config.economy.haul_min_stock,
            wage_due: job.is_some_and(|j| j.days_unpaid >= 1),
            partner: target.filter(|&t| {
                world.has::<crate::components::Brain>(t)
                    && world
                        .comp::<Position>(t)
                        .is_some_and(|p| p.building.is_some() && p.building == pos.and_then(|q| q.building))
            }),
            partner_reachable: target.is_some_and(|t| {
                world.has::<crate::components::Brain>(t)
                    && world.comp::<Position>(t).and_then(|p| p.building).is_some_and(|b| {
                        Some(b) == pos.and_then(|q| q.building)
                            || world
                                .comp::<Building>(b)
                                .is_some_and(|bd| matches!(bd.kind, BuildingKind::Bar | BuildingKind::Market))
                    })
            }),
            gang_eligible: crate::systems::gang::eligible(world, agent),
            extort_ok: target
                .and_then(|t| world.comp::<Building>(t))
                .is_some_and(|b| b.kind == BuildingKind::Home && !b.occupants.is_empty() && !b.derelict),
            has_loot: world.comp::<crate::components::Brain>(agent).is_some_and(|b| b.loot_today > 0),
            can_fence: world.comp::<crate::components::Inventory>(agent).is_some_and(|i| i.stolen_food > 0)
                && world
                    .gang_of(agent)
                    .and_then(|g| world.comp::<crate::components::Gang>(g))
                    .is_some_and(|g| g.treasury > 0),
            hostile_adjacent: target.is_some_and(|t| {
                world.has::<crate::components::Brain>(t) && crate::systems::law::near(world, agent, t, 4)
            }),
            corpse_target: target
                .is_some_and(|t| world.comp::<crate::components::Corpse>(t).is_some_and(|c| !c.buried)),
            may_bury: {
                let digger = job.is_some_and(|j| j.role == Role::Gravedigger);
                let kin = target.is_some_and(|t| {
                    world.edge(agent, t).is_some_and(|e| {
                        matches!(
                            e.kind,
                            crate::components::RelKind::Family
                                | crate::components::RelKind::Spouse
                                | crate::components::RelKind::Parent
                        )
                    })
                });
                let corpse = target.is_some_and(|t| world.has::<crate::components::Corpse>(t));
                digger
                    || kin
                    || corpse
                        && !world.workers(Role::Gravedigger).iter().any(|&c| world.has::<crate::components::Brain>(c))
            },
            wanted: crate::systems::law::wanted(world, agent),
            suspect_located: target.is_some_and(|t| crate::systems::law::is_located_suspect(world, t)),
            jail_day: job.is_some_and(|j| crate::systems::law::jail_duty(world, agent, j.next_shift_key(world.tick))),
            patrol_pending: job.is_some_and(|j| {
                j.role == Role::Guard
                    && j.on_shift(tod)
                    && !crate::systems::law::jail_duty(world, agent, j.next_shift_key(world.tick))
                    && j.last_shift_day != Some(j.next_shift_key(world.tick))
                    && world
                        .comp::<crate::components::Brain>(agent)
                        .is_some_and(|b| b.patrol_legs < world.config.crime.patrol_legs_per_shift)
            }),
            hall_open: matches!(
                world.phase(),
                crate::time::DayPhase::Morning | crate::time::DayPhase::Work | crate::time::DayPhase::Evening
            ),
            drank_today: crate::utility::goals::drank_today(world, agent),
            bar_full: crate::utility::goals::bar_full_for(world, agent),
            raid_pending: crate::systems::raid::raid_pending(world, agent),
            hideout_sacked,
            holes_up: crate::systems::gang::holes_up_at(world, agent).is_some(),
            can_found: crate::systems::founding::can_found(world, agent),
            hotel_available: homeless && hotel.is_some() && crate::systems::demography::is_adult(world, agent),
            squatter: squat.is_some(),
            squat_ok,
            gang_squat,
            shop_pick: target.is_some_and(|t| world.has::<Building>(t))
                && world.comp::<crate::components::Brain>(agent).is_some_and(|b| b.shop_pick.is_some()),
            vehicle_target: target.is_some_and(|t| crate::systems::vehicles::may_steal(world, agent, t)),
            can_fence_vehicle: crate::systems::vehicles::can_fence(world, agent, target),
            steal_vehicle_cost: world.config.vehicles.steal_vehicle_cost,
            install_ready: target_clinic.is_some() && crate::systems::chrome::reserved_implant(world, agent).is_some(),
            // M13 D39: the Treat goal's Detox branch takes the Clinic visit
            // when addiction drives its score.
            therapy_affordable: !detox_pick
                && target_clinic.is_some_and(|c| coins_now >= crate::systems::chrome::therapy_price(world, c)),
            has_implant: world.comp::<crate::components::Kit>(agent).is_some_and(|k| k.chrome),
            loot_target,
            loot_implants,
            may_rip,
            victim_target,
            episode,
            dealer,
            hideout_stims: gang_hideout.map_or(0, |h| world.stock(h, crate::components::Good::Stims)),
            deal_batch: world.config.stims.deal_batch,
            stim_affordable: source_price.is_some_and(|p| coins_now >= p),
            stim_legal: stim_source
                && target.and_then(|t| world.comp::<Building>(t)).is_some_and(|b| b.kind == BuildingKind::Market),
            detox_affordable: detox_pick
                && target_clinic.is_some_and(|c| coins_now >= crate::systems::stims::detox_price(world, c)),
            withdrawal_bonus,
            dist,
        }
    }

    fn is(&self, role: Role) -> bool {
        self.role == Some(role)
    }
}

/// Set one key of a world state (test and feasibility helper).
pub fn set_key(ws: &mut WorldState, key: crate::goap::world_state::Key, value: bool) {
    use crate::goap::world_state::Key as K;
    match key {
        K::HungerSatisfied => ws.hunger_satisfied = value,
        K::EnergySatisfied => ws.energy_satisfied = value,
        K::BelongingSatisfied => ws.belonging_satisfied = value,
        K::HasFood => ws.has_food = value,
        K::HasCoins => ws.has_coins = value,
        K::HasSavings => ws.has_savings = value,
        K::HasWageDue => ws.has_wage_due = value,
        K::ShiftDone => ws.shift_done = value,
        K::HasSpouse => ws.has_spouse = value,
        K::HasPartnerCandidate => ws.has_partner_candidate = value,
        K::IsSafe => ws.is_safe = value,
        K::ThreatRemoved => ws.threat_removed = value,
        K::CrimeReported => ws.crime_reported = value,
        K::SuspectJailed => ws.suspect_jailed = value,
        K::SuspectCuffed => ws.suspect_cuffed = value,
        K::InGang => ws.in_gang = value,
        K::GangTaskDone => ws.gang_task_done = value,
        K::RaidDone => ws.raid_done = value,
        K::Mustered => ws.mustered = value,
        K::CorpseBuried => ws.corpse_buried = value,
        K::CarryingCorpse => ws.carrying_corpse = value,
        K::CarryingStolen => ws.carrying_stolen = value,
        K::IsDark => ws.is_dark = value,
        K::KnownCorpse => ws.known_corpse = value,
        K::KnownSuspectLocation => ws.known_suspect_location = value,
        K::PatrolLegDone => ws.patrol_leg_done = value,
        K::FoodSourceAvailable => ws.food_source_available = value,
        K::ForageAvailable => ws.forage_available = value,
        K::Founded => ws.founded = value,
        K::CheckedIn => ws.checked_in = value,
        K::Squatting => ws.squatting = value,
        K::Bought => ws.bought = value,
        K::CarryingVehicle => ws.carrying_vehicle = value,
        K::Treated => ws.treated = value,
        K::Stripped => ws.stripped = value,
        K::HasStims => ws.has_stims = value,
        K::High => ws.high = value,
    }
}

impl ActionKind {
    /// The "Who" column: may this agent take the action at all?
    pub fn allowed(self, ctx: &PlanCtx) -> bool {
        if !self.implemented() {
            return false;
        }
        match self {
            ActionKind::FarmWork => ctx.is(Role::Farmer),
            // Appended by the executor after FarmWork; never planned.
            ActionKind::HaulToMarket => false,
            ActionKind::ClerkWork => ctx.is(Role::Clerk) || ctx.is(Role::Ripperdoc) || ctx.is(Role::Mechanic),
            ActionKind::BartendWork => ctx.is(Role::Bartender),
            ActionKind::GuardJail => ctx.is(Role::Guard) && ctx.jail_day,
            ActionKind::TendGraves => ctx.is(Role::Gravedigger) || ctx.is(Role::Sanitation),
            ActionKind::CarryCorpse | ActionKind::BuryCorpse => ctx.adult && ctx.may_bury,
            ActionKind::CollectWage => ctx.role.is_some(),
            ActionKind::CollectDole => ctx.role.is_none() && ctx.adult,
            ActionKind::Beg => !ctx.is(Role::Guard),
            ActionKind::Extort | ActionKind::SplitLoot => ctx.in_gang,
            // M13 D26: anyone fences a stolen vehicle at a Hideout.
            ActionKind::Fence => ctx.in_gang || ctx.vehicle_target,
            ActionKind::BuyAsset => ctx.adult,
            ActionKind::StealVehicle => ctx.adult && ctx.lawfulness < 0.4 && !ctx.is(Role::Guard),
            ActionKind::Install | ActionKind::Therapy | ActionKind::Uninstall | ActionKind::Strip => ctx.adult,
            ActionKind::Rip => ctx.adult && ctx.may_rip,
            ActionKind::Abduct => ctx.in_gang,
            // M13 D38/D39.
            ActionKind::PickUp | ActionKind::Deal => ctx.in_gang && ctx.dealer,
            ActionKind::BuyStims | ActionKind::UseStim | ActionKind::Detox => ctx.adult,
            // M12 D31: a rioter marches too.
            ActionKind::Muster | ActionKind::Brawl => ctx.in_gang || ctx.raid_pending,
            ActionKind::PatrolLeg | ActionKind::Arrest | ActionKind::Escort => ctx.is(Role::Guard),
            ActionKind::JoinGang => !ctx.in_gang && !ctx.is(Role::Guard) && ctx.adult && ctx.gang_eligible,
            ActionKind::Flirt | ActionKind::Propose => ctx.adult,
            ActionKind::Register => ctx.adult && !ctx.in_gang,
            ActionKind::CheckIn => ctx.adult && ctx.homeless,
            ActionKind::Occupy => ctx.adult && (ctx.homeless || ctx.gang_squat),
            ActionKind::ServeTime => false,
            _ => true,
        }
    }

    /// Symbolic preconditions against the observed state.
    pub fn preconditions(self, ws: &WorldState, ctx: &PlanCtx) -> bool {
        let at = |k: LocationKey| ws.at == k;
        match self {
            ActionKind::GoTo(k) => ws.at != k && ctx.dist.contains_key(&k),
            ActionKind::EatFromInventory => ws.has_food,
            ActionKind::EatAtHome => at(LocationKey::Home) && ctx.pantry > 0,
            ActionKind::BuyFood => at(LocationKey::Market) && ws.has_coins && ctx.market_free,
            ActionKind::StealFood(StealSource::Market) => {
                at(LocationKey::Market) && !ws.carrying_stolen && ctx.market_free
            }
            ActionKind::StealFood(StealSource::Home) => {
                at(LocationKey::TargetHome) && !ws.carrying_stolen && ctx.target_pantry > 0
            }
            ActionKind::StealFood(StealSource::Warehouse) => {
                at(LocationKey::Warehouse) && !ws.carrying_stolen && ctx.warehouse_stock > 0 && ctx.stealth >= 0.4
            }
            ActionKind::Forage => at(LocationKey::Farm) && ws.forage_available,
            // `bar_full` is a plan-time hint only: once inside, the agent counts as an occupant.
            ActionKind::Beg => at(LocationKey::Market) || (at(LocationKey::Bar) && !ctx.bar_full),
            // A member on watch, lying low or homeless beds down at the Hideout.
            // M12 D21: one who can afford a bed never plans the street; a
            // booked guest sleeps at the Hotel, a squatter in the squat.
            ActionKind::Sleep => {
                (at(LocationKey::Home) && !ctx.holes_up)
                    || (ctx.homeless && !ctx.hotel_available && !ctx.squatter && at(LocationKey::Street))
                    || (ctx.holes_up && at(LocationKey::Hideout))
                    || (ws.checked_in && at(LocationKey::Hotel))
                    || (ctx.squatter && at(LocationKey::Squat))
            }
            // Rest at Bar or Home; also at the workplace while waiting for a
            // shift, and at the Hideout for a gang member.
            ActionKind::Rest => {
                (at(LocationKey::Bar) && !ctx.bar_full)
                    || at(LocationKey::Home)
                    || ctx.workplace.is_some_and(at)
                    || (ctx.in_gang && at(LocationKey::Hideout))
            }
            ActionKind::FarmWork => at(LocationKey::Farm) && !ws.shift_done && ctx.on_shift,
            // M13 D16: Ripperdocs and Mechanics clerk at their workplace.
            ActionKind::ClerkWork => {
                (if ctx.is(Role::Clerk) { at(LocationKey::Market) } else { ctx.workplace.is_some_and(at) })
                    && !ws.shift_done
                    && ctx.on_shift
            }
            ActionKind::BartendWork => at(LocationKey::Bar) && !ws.shift_done && (ctx.on_shift || ctx.evening),
            ActionKind::GuardJail => at(LocationKey::Jail) && !ws.shift_done && ctx.on_shift,
            ActionKind::TendGraves => at(LocationKey::Cemetery) && !ws.shift_done && ctx.on_shift,
            ActionKind::HaulToMarket => at(LocationKey::Farm) && ctx.farm_stock >= ctx.haul_min_stock,
            ActionKind::CollectWage => at(ctx.wage_at) && ws.has_wage_due && ctx.wage_collectable,
            ActionKind::CollectDole => at(LocationKey::Hall) && ctx.dole_available,
            ActionKind::SellFood => at(LocationKey::Market) && ws.has_food && !ws.carrying_stolen,
            ActionKind::Drink => at(LocationKey::Bar) && ctx.coins >= 2 && !ctx.drank_today && !ctx.bar_full,
            ActionKind::StoreFood => at(LocationKey::Home) && ctx.food_unstolen,
            ActionKind::Wander => true,
            ActionKind::ReportCrime => at(LocationKey::Hall) && !ws.crime_reported && ctx.hall_open,
            ActionKind::PatrolLeg => at(LocationKey::PatrolWaypoint) && ctx.patrol_pending,
            ActionKind::Arrest => at(LocationKey::SuspectTile) && ws.known_suspect_location && !ws.suspect_cuffed,
            ActionKind::Escort => ws.suspect_cuffed && ctx.dist.contains_key(&LocationKey::Jail),
            ActionKind::HideFromLaw => (at(LocationKey::Hideout) || at(LocationKey::Home)) && ctx.wanted,
            ActionKind::FleeToHome => !ws.is_safe && ctx.dist.contains_key(&LocationKey::Home),
            ActionKind::Chat => {
                matches!(ws.at, LocationKey::Market | LocationKey::Bar | LocationKey::Home | LocationKey::Farm)
                    && ctx.partner.is_some()
            }
            // Flirt and Propose plan toward a partner who is here or at a Bar /
            // Market; execution re-checks co-location (PartnerLeft otherwise).
            ActionKind::Flirt => {
                matches!(ws.at, LocationKey::Bar | LocationKey::Market) && ctx.partner_reachable && !ws.has_spouse
            }
            ActionKind::Propose => ws.has_partner_candidate && ctx.partner_reachable && !ws.has_spouse,
            ActionKind::JoinGang => at(LocationKey::Hideout) && !ws.in_gang,
            ActionKind::Extort => ws.in_gang && at(LocationKey::TargetHome) && !ctx.guard8 && ctx.extort_ok,
            ActionKind::SplitLoot => {
                ws.in_gang && at(LocationKey::Hideout) && ws.gang_task_done && ctx.has_loot && !ctx.hideout_sacked
            }
            ActionKind::Fence => {
                at(LocationKey::Hideout)
                    && !ctx.hideout_sacked
                    && ((ctx.in_gang && ws.carrying_stolen && ctx.can_fence)
                        || (ws.carrying_vehicle && ctx.can_fence_vehicle))
            }
            ActionKind::BuyAsset => at(LocationKey::Seller) && ctx.shop_pick && !ctx.install_ready && !ws.bought,
            // M13 D34: a gang implant is installed where it is bought: at the seller.
            ActionKind::Install => at(LocationKey::Seller) && ctx.install_ready && !ws.bought,
            ActionKind::Therapy => at(LocationKey::Clinic) && ctx.therapy_affordable && !ws.treated,
            ActionKind::Uninstall => at(LocationKey::Clinic) && ctx.has_implant && !ws.treated,
            ActionKind::Strip => at(LocationKey::CorpseTile) && ctx.loot_target && !ws.stripped,
            // "Strip [-> Rip]": a body is ripped once it is stripped (the
            // executor appends the Rip to a ripper's Strip).
            ActionKind::Rip => {
                (at(LocationKey::CorpseTile) && ctx.loot_implants && !ctx.loot_target)
                    || (at(LocationKey::Hideout) && ws.carrying_corpse)
            }
            ActionKind::Abduct => at(LocationKey::Victim) && ctx.victim_target && !ws.carrying_corpse,
            ActionKind::StealVehicle => at(LocationKey::Vehicle) && ctx.vehicle_target && !ws.carrying_vehicle,
            // M13 D38/D39.
            ActionKind::PickUp => at(LocationKey::Hideout) && ctx.hideout_stims >= ctx.deal_batch && !ws.has_stims,
            ActionKind::Deal => at(LocationKey::Seller) && ws.has_stims && !ws.gang_task_done,
            ActionKind::BuyStims => at(LocationKey::StimSource) && ctx.stim_affordable && !ws.has_stims,
            ActionKind::UseStim => ws.has_stims && !ws.high,
            ActionKind::Detox => at(LocationKey::Clinic) && ctx.detox_affordable && !ws.treated,
            ActionKind::Muster => at(LocationKey::MusterPoint) && !ws.mustered && ctx.raid_pending,
            ActionKind::Brawl => at(LocationKey::RaidTarget) && ws.mustered && !ws.raid_done,
            // M13 D33: a berserker closes in on its quarry first.
            ActionKind::Attack => {
                (ctx.hostile_adjacent || (ctx.episode && at(LocationKey::Victim))) && !ws.threat_removed
            }
            ActionKind::CarryCorpse => at(LocationKey::CorpseTile) && ws.known_corpse && !ws.carrying_corpse,
            ActionKind::BuryCorpse => at(LocationKey::Cemetery) && ws.carrying_corpse,
            ActionKind::Register => at(LocationKey::Hall) && !ws.founded && ctx.can_found,
            ActionKind::CheckIn => at(LocationKey::Hotel) && ctx.hotel_available && !ws.checked_in,
            ActionKind::Occupy => {
                (at(LocationKey::Squat) || at(LocationKey::TargetHome))
                    && ctx.squat_ok
                    && (ctx.gang_squat || !ws.squatting)
            }
            _ => false,
        }
    }

    /// Could this action ever run for this agent today, ignoring where it is
    /// and the symbolic flags? The context-only half of `preconditions`, used
    /// to fail an unsatisfiable goal before the search exhausts itself.
    pub fn feasible(self, ctx: &PlanCtx) -> bool {
        if !self.allowed(ctx) {
            return false;
        }
        match self {
            ActionKind::GoTo(k) => ctx.dist.contains_key(&k),
            ActionKind::EatAtHome => ctx.pantry > 0,
            ActionKind::BuyFood => ctx.coins >= ctx.price && ctx.market_free,
            ActionKind::StealFood(StealSource::Market) => ctx.market_free,
            ActionKind::StealFood(StealSource::Home) => ctx.target_pantry > 0,
            ActionKind::StealFood(StealSource::Warehouse) => ctx.warehouse_stock > 0 && ctx.stealth >= 0.4,
            ActionKind::Forage => matches!(ctx.season, Season::Summer | Season::Autumn) && !ctx.dark,
            ActionKind::FarmWork | ActionKind::ClerkWork | ActionKind::GuardJail | ActionKind::TendGraves => {
                ctx.on_shift
            }
            ActionKind::BartendWork => ctx.on_shift || ctx.evening,
            ActionKind::HaulToMarket => ctx.farm_stock >= ctx.haul_min_stock,
            ActionKind::CollectWage => ctx.wage_collectable,
            ActionKind::CollectDole => ctx.dole_available,
            ActionKind::Drink => ctx.coins >= 2 && !ctx.drank_today && !ctx.bar_full,
            ActionKind::StoreFood => ctx.food_unstolen,
            ActionKind::ReportCrime => ctx.hall_open,
            ActionKind::PatrolLeg => ctx.patrol_pending && ctx.dist.contains_key(&LocationKey::PatrolWaypoint),
            ActionKind::Arrest => ctx.suspect_located,
            ActionKind::HideFromLaw => ctx.wanted,
            ActionKind::FleeToHome => ctx.dist.contains_key(&LocationKey::Home),
            ActionKind::Chat => ctx.partner.is_some(),
            ActionKind::Flirt | ActionKind::Propose => ctx.partner_reachable,
            ActionKind::JoinGang => ctx.gang_eligible && ctx.dist.contains_key(&LocationKey::Hideout),
            ActionKind::Extort => ctx.extort_ok && ctx.dist.contains_key(&LocationKey::TargetHome),
            ActionKind::SplitLoot => ctx.has_loot && !ctx.hideout_sacked,
            ActionKind::Fence => ((ctx.in_gang && ctx.can_fence) || ctx.can_fence_vehicle) && !ctx.hideout_sacked,
            ActionKind::BuyAsset => ctx.shop_pick && ctx.dist.contains_key(&LocationKey::Seller),
            ActionKind::Install => ctx.install_ready && ctx.dist.contains_key(&LocationKey::Seller),
            ActionKind::Therapy => ctx.therapy_affordable && ctx.dist.contains_key(&LocationKey::Clinic),
            ActionKind::Uninstall => ctx.has_implant && ctx.dist.contains_key(&LocationKey::Clinic),
            ActionKind::Strip => ctx.loot_target && ctx.dist.contains_key(&LocationKey::CorpseTile),
            ActionKind::Rip => ctx.loot_implants || ctx.victim_target,
            ActionKind::Abduct => ctx.victim_target && ctx.dist.contains_key(&LocationKey::Victim),
            ActionKind::StealVehicle => ctx.vehicle_target && ctx.dist.contains_key(&LocationKey::Vehicle),
            ActionKind::PickUp => ctx.hideout_stims >= ctx.deal_batch && ctx.dist.contains_key(&LocationKey::Hideout),
            ActionKind::Deal => ctx.dist.contains_key(&LocationKey::Seller),
            ActionKind::BuyStims => ctx.stim_affordable && ctx.dist.contains_key(&LocationKey::StimSource),
            ActionKind::Detox => ctx.detox_affordable && ctx.dist.contains_key(&LocationKey::Clinic),
            ActionKind::Muster => ctx.raid_pending && ctx.dist.contains_key(&LocationKey::MusterPoint),
            ActionKind::Brawl => ctx.raid_pending && ctx.dist.contains_key(&LocationKey::RaidTarget),
            ActionKind::Attack => ctx.hostile_adjacent || (ctx.episode && ctx.dist.contains_key(&LocationKey::Victim)),
            ActionKind::CarryCorpse => ctx.corpse_target,
            ActionKind::BuryCorpse => ctx.corpse_target && ctx.dist.contains_key(&LocationKey::Cemetery),
            ActionKind::Register => ctx.can_found && ctx.dist.contains_key(&LocationKey::Hall),
            ActionKind::CheckIn => ctx.hotel_available && ctx.dist.contains_key(&LocationKey::Hotel),
            ActionKind::Occupy => ctx.squat_ok,
            _ => true,
        }
    }

    /// Does this action set `key` to `value`? (The symbolic effect, read off
    /// `apply` on a blank state.)
    pub fn produces(self, key: crate::goap::world_state::Key, value: bool, ctx: &PlanCtx) -> bool {
        let blank = WorldState::default();
        let a = self.apply(&blank, ctx);
        // Also from a state where the key is already the opposite, so "set false" shows.
        let mut flipped = blank;
        set_key(&mut flipped, key, !value);
        let b = self.apply(&flipped, ctx);
        a.get(key) == value && b.get(key) == value
    }

    /// Symbolic effects. `coin_bucket` and `food_count` are kept so `has_coins`
    /// and `has_food` after a step are computable from the state alone.
    pub fn apply(self, ws: &WorldState, _ctx: &PlanCtx) -> WorldState {
        let mut n = *ws;
        let eat = |n: &mut WorldState| {
            n.hunger_satisfied = true;
            n.food_count = n.food_count.saturating_sub(1);
            n.has_food = n.food_count >= 1;
        };
        let gain_food = |n: &mut WorldState, units: u8| {
            n.food_count = (n.food_count + units).min(FOOD_COUNT_MAX);
            n.has_food = true;
        };
        let spend = |n: &mut WorldState| {
            n.coin_bucket = n.coin_bucket.saturating_sub(1);
            n.has_coins = n.coin_bucket >= 1;
            n.has_savings = false;
        };
        let income = |n: &mut WorldState| {
            n.coin_bucket = 2;
            n.has_coins = true;
            n.has_savings = true; // the best income available to this agent; see commit notes
        };
        match self {
            ActionKind::GoTo(k) => n.at = k,
            ActionKind::EatFromInventory => eat(&mut n),
            ActionKind::EatAtHome => n.hunger_satisfied = true,
            ActionKind::BuyFood => {
                gain_food(&mut n, 1);
                spend(&mut n);
            }
            ActionKind::StealFood(_) => {
                gain_food(&mut n, 2);
                n.carrying_stolen = true;
            }
            ActionKind::Forage => gain_food(&mut n, 1),
            ActionKind::Beg => {
                // Yields 1-2 coins at best: only a meal's worth when the price is that low.
                if _ctx.price <= 2 {
                    n.coin_bucket = n.coin_bucket.max(1);
                    n.has_coins = true;
                }
            }
            // Rest really gives +0.1 energy: claiming `energy_satisfied` sent tired
            // agents to the Bar to doze instead of home to sleep.
            ActionKind::Sleep => n.energy_satisfied = true,
            ActionKind::Rest => {}
            k if k.is_work() => {
                n.shift_done = true;
                n.has_wage_due = true;
            }
            ActionKind::HaulToMarket => n.at = LocationKey::Market,
            ActionKind::CollectWage => {
                n.has_wage_due = false;
                income(&mut n);
            }
            ActionKind::CollectDole => income(&mut n),
            ActionKind::SellFood => {
                n.has_food = false;
                n.food_count = 0;
                n.coin_bucket = n.coin_bucket.max(1);
                n.has_coins = true;
            }
            ActionKind::Drink => {
                n.belonging_satisfied = true;
                spend(&mut n);
            }
            ActionKind::StoreFood => {
                n.has_food = false;
                n.food_count = 0;
            }
            ActionKind::Wander => n.at = LocationKey::Street,
            ActionKind::ReportCrime => n.crime_reported = true,
            ActionKind::PatrolLeg => n.patrol_leg_done = true,
            ActionKind::Arrest => n.suspect_cuffed = true,
            ActionKind::Escort => {
                n.at = LocationKey::Jail;
                n.suspect_jailed = true;
            }
            ActionKind::HideFromLaw => n.is_safe = true,
            ActionKind::FleeToHome => {
                n.at = LocationKey::Home;
                n.is_safe = true;
            }
            ActionKind::Chat => n.belonging_satisfied = true,
            // Flirt's planning effect is deterministic; the roll happens at execution.
            ActionKind::Flirt => n.has_partner_candidate = true,
            ActionKind::Propose => n.has_spouse = true,
            ActionKind::JoinGang => n.in_gang = true,
            ActionKind::Extort => {
                n.gang_task_done = true;
                n.coin_bucket = n.coin_bucket.max(1);
                n.has_coins = true;
            }
            ActionKind::SplitLoot | ActionKind::Fence => {
                n.has_coins = true;
                n.coin_bucket = 2;
                n.has_savings = true;
                if self == ActionKind::Fence {
                    // A vehicle's fence leaves the food; food's leaves the vehicle.
                    if n.carrying_vehicle {
                        n.carrying_vehicle = false;
                    } else {
                        n.carrying_stolen = false;
                        n.has_food = false;
                        n.food_count = 0;
                    }
                }
            }
            ActionKind::Attack => n.threat_removed = true,
            ActionKind::Muster => n.mustered = true,
            ActionKind::Brawl => n.raid_done = true,
            ActionKind::CarryCorpse => {
                n.carrying_corpse = true;
                n.at = LocationKey::Street;
            }
            ActionKind::BuryCorpse => {
                n.carrying_corpse = false;
                n.corpse_buried = true;
            }
            ActionKind::Register => n.founded = true,
            ActionKind::CheckIn => {
                n.checked_in = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::Occupy => {
                n.squatting = true;
                if _ctx.gang_squat {
                    n.gang_task_done = true;
                }
            }
            ActionKind::BuyAsset => {
                n.bought = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::StealVehicle => n.carrying_vehicle = true,
            ActionKind::Install => {
                n.bought = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::Therapy => {
                n.treated = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::Uninstall => n.treated = true,
            ActionKind::Strip => n.stripped = true,
            ActionKind::Rip => {
                n.stripped = true;
                n.gang_task_done = true;
                n.carrying_corpse = false;
            }
            // The abductee rides the corpse-carry key (plan, Actions table).
            ActionKind::Abduct => n.carrying_corpse = true,
            ActionKind::PickUp => n.has_stims = true,
            ActionKind::Deal => n.gang_task_done = true,
            ActionKind::BuyStims => {
                n.has_stims = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::UseStim => n.high = true,
            ActionKind::Detox => {
                n.treated = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            _ => {}
        }
        n
    }

    /// The Action table's cost, clamped to `[0.5, 60]`.
    pub fn cost(self, ctx: &PlanCtx) -> f32 {
        let steal_mods = |base: f32| {
            let mut c = base + ctx.lawfulness * 20.0;
            if ctx.guard8 {
                c += 6.0;
            }
            if ctx.starving {
                c -= 4.0;
            }
            if ctx.dark {
                c -= 3.0;
            }
            // M13 D39: withdrawal makes a theft cheaper (as starving does).
            if ctx.withdrawal_bonus != 0.0 {
                c -= ctx.withdrawal_bonus;
            }
            c - ctx.stealth * 4.0
        };
        let c = match self {
            ActionKind::GoTo(k) => {
                let d = ctx.dist.get(&k).copied().unwrap_or(0) as f32;
                2.0 + d / 16.0 + if ctx.dark && k == LocationKey::Street { 3.0 } else { 0.0 }
            }
            ActionKind::EatFromInventory => 1.0,
            ActionKind::EatAtHome => 2.0 + if ctx.pantry <= 2 { 4.0 } else { 0.0 },
            ActionKind::BuyFood => 3.0 + ctx.greed * 2.0,
            ActionKind::StealFood(StealSource::Market) => steal_mods(8.0),
            ActionKind::StealFood(StealSource::Home) => steal_mods(7.0) + if ctx.target_occupied { 5.0 } else { 0.0 },
            ActionKind::StealFood(StealSource::Warehouse) => steal_mods(9.0) + 4.0,
            ActionKind::Forage => 6.0 + if ctx.hunger < 0.3 { 2.0 } else { 0.0 },
            ActionKind::Beg => 5.0 + ctx.pride * 10.0 + (1.0 - ctx.sociability) * 4.0,
            ActionKind::Sleep => 1.0 + if ctx.homeless { 6.0 } else { 0.0 },
            ActionKind::Rest => 4.0,
            ActionKind::FarmWork => 3.0 - ctx.farming * 2.0,
            ActionKind::HaulToMarket => 3.0,
            ActionKind::ClerkWork | ActionKind::BartendWork | ActionKind::GuardJail | ActionKind::TendGraves => 3.0,
            ActionKind::CollectWage => 2.0 + if ctx.payer_negative { 4.0 } else { 0.0 },
            ActionKind::SellFood => 3.0 - ctx.greed * 2.0,
            ActionKind::Fence => 3.0 + ctx.lawfulness * 8.0,
            ActionKind::Chat => 3.0 - ctx.sociability * 2.0,
            ActionKind::Drink => 4.0 - ctx.sociability * 2.0,
            ActionKind::Flirt => 4.0 + (1.0 - ctx.sociability) * 4.0,
            ActionKind::Propose => 5.0 - ctx.courage * 2.0,
            ActionKind::ReportCrime => 2.0 + (1.0 - ctx.lawfulness) * 10.0 + if ctx.in_gang { 8.0 } else { 0.0 },
            ActionKind::PatrolLeg => 2.0,
            ActionKind::Arrest => 4.0 + (1.0 - ctx.courage) * 6.0,
            ActionKind::Escort => 3.0,
            ActionKind::HideFromLaw => 4.0 - (1.0 - ctx.lawfulness) * 2.0,
            ActionKind::FleeToHome => 2.0 - (1.0 - ctx.courage) * 3.0,
            ActionKind::Attack => 10.0 + (1.0 - ctx.courage) * 15.0 + ctx.lawfulness * 10.0 - ctx.fighting * 6.0,
            ActionKind::JoinGang => 6.0 + ctx.lawfulness * 15.0 - (1.0 - ctx.hunger) * 4.0,
            ActionKind::Extort => 6.0 + ctx.lawfulness * 12.0,
            ActionKind::SplitLoot => 2.0,
            ActionKind::Muster => 1.0,
            ActionKind::Brawl => 15.0,
            ActionKind::BuryCorpse => 2.0,
            ActionKind::CarryCorpse => 3.0,
            ActionKind::Wander => 1.0,
            ActionKind::CollectDole => 2.0,
            ActionKind::ServeTime => 60.0,
            ActionKind::StoreFood => 1.0,
            ActionKind::Register => 20.0,
            ActionKind::CheckIn => 4.0,
            ActionKind::Occupy => 6.0,
            ActionKind::BuyAsset => 10.0,
            // M13 D26: StealFood's lawfulness, guard, starving and dark terms.
            ActionKind::StealVehicle => steal_mods(ctx.steal_vehicle_cost),
            ActionKind::Install | ActionKind::Therapy => 5.0,
            ActionKind::Uninstall => 8.0,
            ActionKind::Strip => 4.0 + 4.0 * ctx.lawfulness,
            ActionKind::Rip | ActionKind::Abduct => 6.0,
            ActionKind::PickUp => 2.0,
            ActionKind::Deal => 3.0,
            ActionKind::BuyStims => {
                if ctx.stim_legal {
                    3.0
                } else {
                    3.0 + 3.0 * ctx.lawfulness
                }
            }
            ActionKind::UseStim => 1.0,
            ActionKind::Detox => 5.0,
        };
        c.clamp(0.5, 60.0)
    }

    /// The entity a step is about, if the kind needs one bound in the plan.
    pub fn instance(self, ctx: &PlanCtx) -> ActionInstance {
        let target = match self {
            ActionKind::GoTo(LocationKey::TargetHome)
            | ActionKind::StealFood(StealSource::Home)
            | ActionKind::GoTo(LocationKey::SuspectTile)
            | ActionKind::Arrest
            | ActionKind::Escort
            | ActionKind::Chat
            | ActionKind::Flirt
            | ActionKind::Propose
            | ActionKind::Extort
            | ActionKind::Attack
            | ActionKind::GoTo(LocationKey::CorpseTile)
            | ActionKind::CarryCorpse
            | ActionKind::BuryCorpse
            | ActionKind::Occupy
            | ActionKind::GoTo(LocationKey::Seller)
            | ActionKind::BuyAsset
            | ActionKind::GoTo(LocationKey::Vehicle)
            | ActionKind::StealVehicle
            | ActionKind::GoTo(LocationKey::Clinic)
            | ActionKind::GoTo(LocationKey::Victim)
            | ActionKind::Strip
            | ActionKind::Rip
            | ActionKind::Abduct
            | ActionKind::Deal
            | ActionKind::GoTo(LocationKey::StimSource)
            | ActionKind::BuyStims => ctx.target,
            _ => None,
        };
        ActionInstance { action: self, target, tile: None }
    }
}
