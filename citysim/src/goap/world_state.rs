//! The symbolic snapshot the planner searches over, and how it is observed
//! from components.

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Corpse, GangMember, Household, Inventory, Job, Memory, MemoryKind, Needs, Position,
    Wallet,
};
use crate::entity::EntityId;
use crate::exec::ReservationKind;
use crate::time::Season;
use crate::world::World;

// Thresholds that define the symbolic keys (spec › WorldState comments).
/// `hunger_satisfied`: `needs.hunger >= 0.6`.
pub const HUNGER_SATISFIED: f32 = 0.6;
/// `energy_satisfied`: `needs.energy >= 0.6`.
pub const ENERGY_SATISFIED: f32 = 0.6;
/// `belonging_satisfied`: `needs.belonging >= 0.5`.
pub const BELONGING_SATISFIED: f32 = 0.5;
/// `is_safe`: `needs.safety >= 0.4`.
pub const SAFE: f32 = 0.4;
/// `has_savings`: `coins >= 7 × price_food`.
pub const SAVINGS_DAYS: i64 = 7;
/// `food_count` saturates here.
pub const FOOD_COUNT_MAX: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum LocationKey {
    /// Only in goal states / preconditions, never observed.
    #[default]
    Anywhere,
    Home,
    Farm,
    Market,
    Bar,
    Jail,
    Cemetery,
    Hall,
    Hideout,
    /// The street tile outside the current expedition's door: the rival
    /// Hideout under Raid / Retaliate, the Jail under BreakOut (raiders stop there).
    #[serde(alias = "RivalHideout")]
    RaidTarget,
    Warehouse,
    /// On a non-building tile.
    Street,
    /// The Home bound to `Plan.target` (StealFood(Home), Extort).
    TargetHome,
    /// Last known tile of the `Plan.target` suspect.
    SuspectTile,
    /// Tile of the `Plan.target` corpse.
    CorpseTile,
    /// Next patrol leg.
    PatrolWaypoint,
    /// M11 D13: the employer building of a non-city job (the wage desk), and
    /// a Security Office for the private guard inside it. A city job's
    /// resolves to the Civic Hall, so an old plan still completes.
    Workplace,
    /// M12 D21: the booked Hotel, else `street::hotel_for`.
    Hotel,
    /// M12 D27: the agent's squat, else the bound derelict target.
    Squat,
    /// M12 D37: where the expedition gathers (`raid::muster_point`): a held
    /// Home's or a riot muster's door tile, else inside the Hideout.
    MusterPoint,
    /// M13 D47: a Garage (its own key once plans reach one).
    Garage,
    /// M13 D29: the plan's target building as a seller (a Garage, a Market
    /// for a pack): resolved like `TargetHome`, never a Home.
    Seller,
    /// M13 D26: the street tile outside the door the plan's target vehicle
    /// is parked at (no building).
    Vehicle,
    /// M13 D16/D34: a Clinic (a Ripperdoc's shift, Install, Therapy).
    Clinic,
    /// M13 D36/D33: next to the plan's target agent (a Harvest victim, an
    /// episode's quarry): its building, else its tile.
    Victim,
}

impl LocationKey {
    /// Keys a `GoTo` may target, in enum (tie-break) order.
    pub const GOTO: [LocationKey; 24] = [
        LocationKey::Home,
        LocationKey::Farm,
        LocationKey::Market,
        LocationKey::Bar,
        LocationKey::Jail,
        LocationKey::Cemetery,
        LocationKey::Hall,
        LocationKey::Hideout,
        LocationKey::RaidTarget,
        LocationKey::Warehouse,
        LocationKey::Street,
        LocationKey::TargetHome,
        LocationKey::SuspectTile,
        LocationKey::CorpseTile,
        LocationKey::PatrolWaypoint,
        LocationKey::Workplace,
        LocationKey::Hotel,
        LocationKey::Squat,
        LocationKey::MusterPoint,
        LocationKey::Garage,
        LocationKey::Seller,
        LocationKey::Vehicle,
        LocationKey::Clinic,
        LocationKey::Victim,
    ];

    pub fn of_building(kind: BuildingKind) -> LocationKey {
        match kind {
            BuildingKind::Home => LocationKey::Home,
            BuildingKind::Farm => LocationKey::Farm,
            BuildingKind::Market => LocationKey::Market,
            BuildingKind::Bar => LocationKey::Bar,
            BuildingKind::Jail => LocationKey::Jail,
            BuildingKind::Cemetery => LocationKey::Cemetery,
            BuildingKind::Hall => LocationKey::Hall,
            BuildingKind::Hideout => LocationKey::Hideout,
            BuildingKind::Warehouse => LocationKey::Warehouse,
            // M11: a private guard inside their office is at their workplace.
            BuildingKind::SecurityOffice => LocationKey::Workplace,
            BuildingKind::Lot => LocationKey::Street,
            BuildingKind::Hotel => LocationKey::Hotel,
            BuildingKind::Garage => LocationKey::Garage,
            BuildingKind::Clinic => LocationKey::Clinic,
        }
    }
}

/// A boolean key of the world state, for goal states.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Key {
    HungerSatisfied,
    EnergySatisfied,
    BelongingSatisfied,
    HasFood,
    HasCoins,
    HasSavings,
    HasWageDue,
    ShiftDone,
    HasSpouse,
    HasPartnerCandidate,
    IsSafe,
    ThreatRemoved,
    CrimeReported,
    SuspectJailed,
    SuspectCuffed,
    InGang,
    GangTaskDone,
    /// No raid is pending for the agent's gang.
    RaidDone,
    /// The gang's raid has departed (latecomers skip the muster).
    Mustered,
    CorpseBuried,
    CarryingCorpse,
    CarryingStolen,
    IsDark,
    KnownCorpse,
    KnownSuspectLocation,
    PatrolLegDone,
    FoodSourceAvailable,
    ForageAvailable,
    /// M11 D26: a business registered (never observed true; `Register` sets it).
    Founded,
    /// M12 D21: a live Hotel booking tonight.
    CheckedIn,
    /// M12 D27: living in a squat.
    Squatting,
    /// M13 D43: an asset bought (never observed true; `BuyAsset` sets it).
    Bought,
    /// M13 D26 (phase 2 deviation): driving or holding a stolen vehicle not
    /// yet fenced. Its own key, not `CarryingStolen`, so a non-member's
    /// plan cannot fence stolen food it has no gang to sell to.
    CarryingVehicle,
    /// M13 D34: treated at a Clinic (never observed true; Therapy sets it).
    Treated,
    /// M13 D35: a body stripped (never observed true; Strip and Rip set it).
    Stripped,
}

/// Partial goal state: at most 3 listed keys.
pub type GoalState = Vec<(Key, bool)>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct WorldState {
    pub at: LocationKey,
    pub hunger_satisfied: bool,
    pub energy_satisfied: bool,
    pub belonging_satisfied: bool,
    pub has_food: bool,
    pub has_coins: bool,
    pub has_savings: bool,
    /// 0: < price, 1: < 2*price, 2: more.
    pub coin_bucket: u8,
    /// `inventory.food` saturating at 3.
    pub food_count: u8,
    pub has_wage_due: bool,
    pub shift_done: bool,
    pub has_spouse: bool,
    pub has_partner_candidate: bool,
    pub is_safe: bool,
    pub threat_removed: bool,
    pub crime_reported: bool,
    pub suspect_jailed: bool,
    pub suspect_cuffed: bool,
    pub in_gang: bool,
    pub gang_task_done: bool,
    pub raid_done: bool,
    pub mustered: bool,
    pub corpse_buried: bool,
    pub carrying_corpse: bool,
    pub carrying_stolen: bool,
    pub is_dark: bool,
    pub known_corpse: bool,
    pub known_suspect_location: bool,
    pub patrol_leg_done: bool,
    pub food_source_available: bool,
    pub forage_available: bool,
    pub founded: bool,
    pub checked_in: bool,
    pub squatting: bool,
    pub bought: bool,
    pub carrying_vehicle: bool,
    pub treated: bool,
    pub stripped: bool,
}

impl WorldState {
    /// An injective 64-bit packing (the planner's closed set keys on it):
    /// `at` in bits 0-7, `coin_bucket` 8-15, `food_count` 16-23, then one
    /// bit per flag from 24. The destructuring is exhaustive, so a new field
    /// fails to compile here until it is packed.
    pub fn pack(&self) -> u64 {
        let WorldState {
            at,
            hunger_satisfied,
            energy_satisfied,
            belonging_satisfied,
            has_food,
            has_coins,
            has_savings,
            coin_bucket,
            food_count,
            has_wage_due,
            shift_done,
            has_spouse,
            has_partner_candidate,
            is_safe,
            threat_removed,
            crime_reported,
            suspect_jailed,
            suspect_cuffed,
            in_gang,
            gang_task_done,
            raid_done,
            mustered,
            corpse_buried,
            carrying_corpse,
            carrying_stolen,
            is_dark,
            known_corpse,
            known_suspect_location,
            patrol_leg_done,
            food_source_available,
            forage_available,
            founded,
            checked_in,
            squatting,
            bought,
            carrying_vehicle,
            treated,
            stripped,
        } = *self;
        let flags = [
            hunger_satisfied,
            energy_satisfied,
            belonging_satisfied,
            has_food,
            has_coins,
            has_savings,
            has_wage_due,
            shift_done,
            has_spouse,
            has_partner_candidate,
            is_safe,
            threat_removed,
            crime_reported,
            suspect_jailed,
            suspect_cuffed,
            in_gang,
            gang_task_done,
            raid_done,
            mustered,
            corpse_buried,
            carrying_corpse,
            carrying_stolen,
            is_dark,
            known_corpse,
            known_suspect_location,
            patrol_leg_done,
            food_source_available,
            forage_available,
            founded,
            checked_in,
            squatting,
            bought,
            carrying_vehicle,
            treated,
            stripped,
        ];
        let mut k = u64::from(at as u8) | (u64::from(coin_bucket) << 8) | (u64::from(food_count) << 16);
        for (i, f) in flags.into_iter().enumerate() {
            k |= u64::from(f) << (24 + i);
        }
        k
    }

    /// `pack()` of this state with `at` replaced, from its packed key.
    pub fn repack_at(key: u64, at: LocationKey) -> u64 {
        (key & !0xFF) | u64::from(at as u8)
    }

    pub fn get(&self, key: Key) -> bool {
        match key {
            Key::HungerSatisfied => self.hunger_satisfied,
            Key::EnergySatisfied => self.energy_satisfied,
            Key::BelongingSatisfied => self.belonging_satisfied,
            Key::HasFood => self.has_food,
            Key::HasCoins => self.has_coins,
            Key::HasSavings => self.has_savings,
            Key::HasWageDue => self.has_wage_due,
            Key::ShiftDone => self.shift_done,
            Key::HasSpouse => self.has_spouse,
            Key::HasPartnerCandidate => self.has_partner_candidate,
            Key::IsSafe => self.is_safe,
            Key::ThreatRemoved => self.threat_removed,
            Key::CrimeReported => self.crime_reported,
            Key::SuspectJailed => self.suspect_jailed,
            Key::SuspectCuffed => self.suspect_cuffed,
            Key::InGang => self.in_gang,
            Key::GangTaskDone => self.gang_task_done,
            Key::RaidDone => self.raid_done,
            Key::Mustered => self.mustered,
            Key::CorpseBuried => self.corpse_buried,
            Key::CarryingCorpse => self.carrying_corpse,
            Key::CarryingStolen => self.carrying_stolen,
            Key::IsDark => self.is_dark,
            Key::KnownCorpse => self.known_corpse,
            Key::KnownSuspectLocation => self.known_suspect_location,
            Key::PatrolLegDone => self.patrol_leg_done,
            Key::FoodSourceAvailable => self.food_source_available,
            Key::ForageAvailable => self.forage_available,
            Key::Founded => self.founded,
            Key::CheckedIn => self.checked_in,
            Key::Squatting => self.squatting,
            Key::Bought => self.bought,
            Key::CarryingVehicle => self.carrying_vehicle,
            Key::Treated => self.treated,
            Key::Stripped => self.stripped,
        }
    }

    /// `satisfied(ws, goal)` compares only the listed keys.
    pub fn satisfies(&self, goal: &GoalState) -> bool {
        goal.iter().all(|&(k, v)| self.get(k) == v)
    }

    /// Number of goal keys not yet satisfied (the planner's heuristic input).
    pub fn unsatisfied(&self, goal: &GoalState) -> usize {
        goal.iter().filter(|&&(k, v)| self.get(k) != v).count()
    }

    /// Coin bucket from a wallet and the price: 0 below price, 1 below twice it, 2 more.
    pub fn bucket(coins: i64, price: i64) -> u8 {
        let price = price.max(1);
        if coins < price {
            0
        } else if coins < 2 * price {
            1
        } else {
            2
        }
    }

    /// Food units reserved on a building by every holder.
    pub fn reserved_units(world: &World, building: EntityId) -> u32 {
        WorldState::reserved_by_others(world, building, None)
    }

    /// Food units reserved on a building by holders other than `except`.
    pub fn reserved_by_others(world: &World, building: EntityId, except: Option<EntityId>) -> u32 {
        world
            .reservations
            .iter()
            .filter(|(&holder, _)| Some(holder) != except)
            .flat_map(|(_, v)| v.iter())
            .filter_map(|r| match r.kind {
                ReservationKind::FoodUnits { building: b, units } if b == building => Some(units),
                _ => None,
            })
            .sum()
    }

    /// Snapshot an agent's state. `target` is `Plan.target` (bound at plan
    /// time) or `None`.
    pub fn observe(world: &World, agent: EntityId, target: Option<EntityId>) -> WorldState {
        let needs = world.comp::<Needs>(agent).cloned().unwrap_or(Needs {
            hunger: 1.0,
            energy: 1.0,
            safety: 1.0,
            wealth: 1.0,
            belonging: 1.0,
            intimacy: 1.0,
            starving_since: None,
        });
        let inv = world.comp::<Inventory>(agent).cloned().unwrap_or_default();
        let coins = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
        let market = world.local(agent, BuildingKind::Market);
        let price = market.map_or(1, |m| world.price_for(m, agent)).max(1);
        let home = world.comp::<Household>(agent).and_then(|h| h.home);
        let pos = world.comp::<Position>(agent);
        let job = world.comp::<Job>(agent);

        // Symbolic places that depend on the plan: adjacent to (or in the same
        // building as) the bound suspect is SuspectTile; inside the current
        // patrol stop, while patrolling, is PatrolWaypoint.
        // M13 D36: an abductee in tow rides the corpse-carry key.
        let carrying = world.comp::<Brain>(agent).is_some_and(|b| b.carrying_corpse.is_some())
            || crate::systems::chrome::dragging(world, agent).is_some();
        let suspect_target = target
            .filter(|&t| world.has::<Brain>(t))
            .filter(|&t| crate::systems::law::is_guard(world, agent) && crate::systems::law::wanted(world, t));
        let at_suspect = suspect_target.is_some_and(|t| crate::systems::law::near(world, agent, t, 1));
        // Next to (or in the same building as) the bound unburied corpse.
        let at_corpse = target
            .filter(|&t| world.comp::<Corpse>(t).is_some_and(|c| !c.buried))
            .is_some_and(|t| crate::systems::law::near(world, agent, t, 1));
        let patrolling = world.comp::<Brain>(agent).is_some_and(|b| {
            b.current_goal == Some(crate::components::GoalKind::Patrol)
                && b.patrol_route
                    .get(usize::from(b.patrol_legs) % b.patrol_route.len().max(1))
                    .is_some_and(|&stop| pos.is_some_and(|p| p.building == Some(stop)))
        });
        let raid_tile = crate::systems::raid::target_tile(world, agent);
        // M12 D37: a Raid goal's muster point, while the expedition is pending.
        let mustering =
            world.comp::<Brain>(agent).is_some_and(|b| b.current_goal == Some(crate::components::GoalKind::Raid))
                && crate::systems::raid::raid_pending(world, agent);
        let muster = if mustering { crate::systems::raid::muster_point(world, agent) } else { None };
        let at_muster = match muster {
            Some(crate::systems::raid::MusterAt::Inside(b)) => pos.is_some_and(|p| p.building == Some(b)),
            Some(crate::systems::raid::MusterAt::Door(t)) => pos.is_some_and(|p| p.building.is_none() && p.tile == t),
            None => false,
        };
        // M12 D27: the agent's squat (or, while planning one, the bound derelict).
        let squat = world.comp::<crate::components::Squatter>(agent).map(|s| s.building);
        // M13 D29: inside the bound seller while shopping.
        let shopping = target.is_some()
            && world.comp::<Brain>(agent).is_some_and(|b| b.current_goal == Some(crate::components::GoalKind::Shop));
        // M13 D26: on the street outside the bound vehicle's door.
        let vehicle_tile = target.and_then(|t| crate::systems::vehicles::vehicle_stand(world, t));
        // M13 D33/D36: next to the quarry (an episode's, a Harvest crew's),
        // unless it is the abductee being dragged along.
        let episode = crate::systems::chrome::in_episode(world, agent);
        let dragging = crate::systems::chrome::dragging(world, agent);
        let at_victim = target.is_some_and(|t| {
            Some(t) != dragging
                && t != agent
                && world.has::<Brain>(t)
                && crate::systems::law::near(world, agent, t, 1)
                && (episode
                    || world
                        .gang_of(agent)
                        .is_some_and(|g| crate::systems::chrome::gang_harvest_target(world, g) == Some(t)))
        });
        let at = match pos.and_then(|p| p.building) {
            _ if at_suspect => LocationKey::SuspectTile,
            _ if at_victim => LocationKey::Victim,
            _ if at_corpse && !carrying => LocationKey::CorpseTile,
            _ if patrolling => LocationKey::PatrolWaypoint,
            _ if at_muster => LocationKey::MusterPoint,
            Some(b) if Some(b) == home => LocationKey::Home,
            Some(b)
                if Some(b) == target && world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Home) =>
            {
                LocationKey::TargetHome
            }
            Some(b) if Some(b) == squat || (Some(b) == target && crate::systems::street::is_derelict(world, b)) => {
                LocationKey::Squat
            }
            Some(b) if shopping && Some(b) == target => LocationKey::Seller,
            Some(b) => match world.comp::<Building>(b) {
                Some(bd) if bd.kind == BuildingKind::Home => LocationKey::Street, // someone else's home
                Some(bd) => LocationKey::of_building(bd.kind),
                None => LocationKey::Street,
            },
            None if raid_tile.is_some() && raid_tile == pos.map(|p| p.tile) => LocationKey::RaidTarget,
            None if vehicle_tile.is_some() && vehicle_tile == pos.map(|p| p.tile) => LocationKey::Vehicle,
            None => LocationKey::Street,
        };

        // `shift_done`: the shift this moment belongs to has been worked. Keyed
        // on the current shift day, not the next one, so it stays true after
        // the shift ends and the wage trip can still be planned.
        // ... and once the shift key rolls over (midnight, or 06:00 for night
        // guards) a worker still owed wages counts as "shift done" while no
        // shift is on, so the wage trip remains plannable.
        let (has_wage_due, shift_done) = match job {
            Some(j) => {
                let due = j.days_unpaid >= 1;
                let done =
                    j.last_shift_day == Some(j.shift_key_at(world.tick)) || (due && !j.on_shift(world.tick_of_day()));
                (due, done)
            }
            None => (false, false),
        };

        let has_spouse = crate::systems::social::has_spouse(world, agent);
        // The bound target is the candidate when there is one; otherwise any.
        // Propose needs a bound partner, so an unbound plan has no candidate.
        let has_partner_candidate = target
            .filter(|&t| world.has::<Brain>(t))
            .is_some_and(|t| crate::systems::social::propose_allowed(world, agent, t));
        let gang_task_done = world.comp::<Brain>(agent).is_some_and(|b| b.gang_task_day == Some(world.day()));
        // A hostile (Enemy edge) within 4 tiles.
        let hostile_near =
            world.enemies_of(agent).any(|o| world.has::<Brain>(o) && crate::systems::law::near(world, agent, o, 4));

        let memory = world.comp::<Memory>(agent);
        // No unreported first-hand SawCrime (salience >= 0.5) whose subject lacks an open report.
        let crime_reported = memory.is_none_or(|m| {
            !m.entries.iter().any(|e| {
                e.kind == MemoryKind::SawCrime
                    && e.salience >= 0.5
                    && !e.second_hand
                    && e.subject.is_some_and(|s| !crate::systems::law::reported_since(world, s, e.tick))
            })
        });
        let wanted = crate::systems::law::wanted(world, agent);
        let guard8 = wanted && crate::systems::law::guard_within(world, agent, world.config.crime.sight_day_crime);
        let suspect_located = target.is_some_and(|t| crate::systems::law::is_located_suspect(world, t));
        let known_corpse = memory.is_some_and(|m| {
            m.entries.iter().any(|e| {
                e.kind == MemoryKind::SawCorpse
                    && e.subject.is_some_and(|c| world.comp::<Corpse>(c).is_some_and(|k| !k.buried))
            })
        });

        let market_free = market
            .and_then(|m| world.comp::<Building>(m).map(|b| (m, b.stock_food)))
            .is_some_and(|(m, stock)| stock > WorldState::reserved_by_others(world, m, Some(agent)));
        let pantry_free = home
            .and_then(|h| world.comp::<Building>(h).map(|b| (h, b.stock_food)))
            .is_some_and(|(h, stock)| stock > WorldState::reserved_by_others(world, h, Some(agent)));

        let season = world.season();
        let dark = world.is_dark();

        WorldState {
            at,
            hunger_satisfied: needs.hunger >= HUNGER_SATISFIED,
            energy_satisfied: needs.energy >= ENERGY_SATISFIED,
            belonging_satisfied: needs.belonging >= BELONGING_SATISFIED,
            has_food: inv.food >= 1,
            has_coins: coins >= price,
            has_savings: coins >= SAVINGS_DAYS * price,
            coin_bucket: WorldState::bucket(coins, price),
            food_count: inv.food.min(u32::from(FOOD_COUNT_MAX)) as u8,
            has_wage_due,
            shift_done,
            has_spouse,
            has_partner_candidate,
            // Not safe while wanted with a guard within 8 (HideFromLaw reader).
            is_safe: needs.safety >= SAFE && !guard8,
            // M13 D33: no threat is ever removed for a berserker.
            threat_removed: !hostile_near && !episode,
            crime_reported,
            suspect_jailed: target.is_some_and(|t| world.has::<crate::components::Sentence>(t)),
            suspect_cuffed: target.is_some_and(|t| world.comp::<Brain>(t).is_some_and(|b| b.cuffed_by.is_some())),
            in_gang: world.has::<GangMember>(agent),
            gang_task_done,
            raid_done: crate::systems::raid::raid_done(world, agent),
            mustered: crate::systems::raid::mustered(world, agent),
            corpse_buried: target.is_some_and(|t| world.comp::<Corpse>(t).is_some_and(|c| c.buried)),
            carrying_corpse: carrying,
            carrying_stolen: inv.stolen_food >= 1,
            is_dark: dark,
            known_corpse,
            known_suspect_location: suspect_located,
            patrol_leg_done: false, // one leg per plan; the goal re-wins for the next
            food_source_available: market_free || pantry_free,
            forage_available: matches!(season, Season::Summer | Season::Autumn) && !dark,
            founded: false,
            checked_in: crate::systems::street::booked_hotel(world, agent).is_some(),
            squatting: squat.is_some(),
            bought: false,
            carrying_vehicle: crate::systems::vehicles::stolen_held_by(world, agent).is_some(),
            treated: false,
            stripped: false,
        }
    }
}
