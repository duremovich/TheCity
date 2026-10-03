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
}

/// Every action the planner may consider, in tie-break order.
pub const PLANNABLE: [ActionKind; 50] = [
    ActionKind::GoTo(LocationKey::Home),
    ActionKind::GoTo(LocationKey::Farm),
    ActionKind::GoTo(LocationKey::Market),
    ActionKind::GoTo(LocationKey::Bar),
    ActionKind::GoTo(LocationKey::Jail),
    ActionKind::GoTo(LocationKey::Cemetery),
    ActionKind::GoTo(LocationKey::Hall),
    ActionKind::GoTo(LocationKey::Hideout),
    ActionKind::GoTo(LocationKey::Warehouse),
    ActionKind::GoTo(LocationKey::Street),
    ActionKind::GoTo(LocationKey::TargetHome),
    ActionKind::GoTo(LocationKey::SuspectTile),
    ActionKind::GoTo(LocationKey::CorpseTile),
    ActionKind::GoTo(LocationKey::PatrolWaypoint),
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
    ActionKind::BuryCorpse,
    ActionKind::CarryCorpse,
    ActionKind::Wander,
    ActionKind::CollectDole,
    ActionKind::GuardJail,
    ActionKind::TendGraves,
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
    pub treasury_negative: bool,
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
                world.citizens().into_iter().any(|g| {
                    g != agent
                        && world.comp::<Job>(g).is_some_and(|j| j.role == Role::Guard)
                        && world.comp::<Position>(g).is_some_and(|gp| gp.tile.manhattan(t) <= sight)
                })
            });
        let stock_of = |kind: BuildingKind| -> u32 {
            world.building_of_kind(kind).and_then(|b| world.comp::<Building>(b)).map_or(0, |b| b.stock_food)
        };
        let farm = job
            .and_then(|j| j.employer)
            .filter(|&e| world.comp::<Building>(e).is_some_and(|b| b.kind == BuildingKind::Farm));
        let tod = world.tick_of_day();
        let day = world.day();

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
            add(LocationKey::Farm, farm.or_else(|| world.building_of_kind(BuildingKind::Farm)));
            for kind in [
                BuildingKind::Market,
                BuildingKind::Bar,
                BuildingKind::Jail,
                BuildingKind::Cemetery,
                BuildingKind::Hall,
                BuildingKind::Hideout,
                BuildingKind::Warehouse,
            ] {
                add(LocationKey::of_building(kind), world.building_of_kind(kind));
            }
            add(LocationKey::TargetHome, target);
            if let Some(b) = world
                .comp::<crate::components::Brain>(agent)
                .and_then(|b| b.patrol_route.get(usize::from(b.patrol_legs) % b.patrol_route.len().max(1)).copied())
            {
                add(LocationKey::PatrolWaypoint, Some(b));
            }
        }

        let (target_pantry, target_occupied) = target
            .and_then(|t| world.comp::<Building>(t))
            .map_or((0, false), |b| (b.stock_food, !b.occupants.is_empty()));

        PlanCtx {
            agent,
            target,
            role: job.map(|j| j.role),
            in_gang: world.has::<GangMember>(agent),
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
            treasury_negative: world.treasury().is_some_and(|t| t.coins < 0),
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
                .map(|j| crate::exec::routine::workplace_key(j.role)),
            pantry: home.and_then(|h| world.comp::<Building>(h)).map_or(0, |b| b.stock_food),
            target_pantry,
            target_occupied,
            market_stock: stock_of(BuildingKind::Market),
            warehouse_stock: stock_of(BuildingKind::Warehouse),
            farm_stock: farm.and_then(|f| world.comp::<Building>(f)).map_or(0, |b| b.stock_food),
            coins: world.comp::<Wallet>(agent).map_or(0, |w| w.coins),
            price: world.market().map_or(1, |m| m.price_food).max(1),
            food_unstolen: world.comp::<crate::components::Inventory>(agent).is_some_and(|i| i.food > i.stolen_food),
            market_free: world
                .building_of_kind(BuildingKind::Market)
                .and_then(|m| world.comp::<Building>(m).map(|b| (m, b.stock_food)))
                .is_some_and(|(m, stock)| stock > WorldState::reserved_by_others(world, m, Some(agent))),
            haul_min_stock: world.config.economy.haul_min_stock,
            wage_due: job.is_some_and(|j| j.days_unpaid >= 1),
            wanted: crate::systems::law::wanted(world, agent),
            suspect_located: target.is_some_and(|t| crate::systems::law::located_suspects(world).contains(&t)),
            jail_day: job.is_some_and(|j| crate::systems::law::jail_day(agent, j.next_shift_key(world.tick))),
            patrol_pending: job.is_some_and(|j| {
                j.role == Role::Guard
                    && j.on_shift(tod)
                    && !crate::systems::law::jail_day(agent, j.next_shift_key(world.tick))
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
        K::CorpseBuried => ws.corpse_buried = value,
        K::CarryingCorpse => ws.carrying_corpse = value,
        K::CarryingStolen => ws.carrying_stolen = value,
        K::IsDark => ws.is_dark = value,
        K::KnownCorpse => ws.known_corpse = value,
        K::KnownSuspectLocation => ws.known_suspect_location = value,
        K::PatrolLegDone => ws.patrol_leg_done = value,
        K::FoodSourceAvailable => ws.food_source_available = value,
        K::ForageAvailable => ws.forage_available = value,
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
            ActionKind::ClerkWork => ctx.is(Role::Clerk),
            ActionKind::BartendWork => ctx.is(Role::Bartender),
            ActionKind::GuardJail => ctx.is(Role::Guard) && ctx.jail_day,
            ActionKind::TendGraves => ctx.is(Role::Gravedigger),
            ActionKind::CollectWage => ctx.role.is_some(),
            ActionKind::CollectDole => ctx.role.is_none() && ctx.adult,
            ActionKind::Beg => !ctx.is(Role::Guard),
            ActionKind::Fence | ActionKind::Extort | ActionKind::SplitLoot => ctx.in_gang,
            ActionKind::PatrolLeg | ActionKind::Arrest | ActionKind::Escort => ctx.is(Role::Guard),
            ActionKind::JoinGang => !ctx.in_gang && !ctx.is(Role::Guard) && ctx.adult,
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
            ActionKind::Sleep => at(LocationKey::Home) || (ctx.homeless && at(LocationKey::Street)),
            // Rest at Bar or Home; also at the workplace while waiting for a shift.
            ActionKind::Rest => {
                (at(LocationKey::Bar) && !ctx.bar_full) || at(LocationKey::Home) || ctx.workplace.is_some_and(at)
            }
            ActionKind::FarmWork => at(LocationKey::Farm) && !ws.shift_done && ctx.on_shift,
            ActionKind::ClerkWork => at(LocationKey::Market) && !ws.shift_done && ctx.on_shift,
            ActionKind::BartendWork => at(LocationKey::Bar) && !ws.shift_done && (ctx.on_shift || ctx.evening),
            ActionKind::GuardJail => at(LocationKey::Jail) && !ws.shift_done && ctx.on_shift,
            ActionKind::TendGraves => at(LocationKey::Cemetery) && !ws.shift_done && ctx.on_shift,
            ActionKind::HaulToMarket => at(LocationKey::Farm) && ctx.farm_stock >= ctx.haul_min_stock,
            ActionKind::CollectWage => at(LocationKey::Hall) && ws.has_wage_due && ctx.wage_collectable,
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
            ActionKind::CollectWage => 2.0 + if ctx.treasury_negative { 4.0 } else { 0.0 },
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
            ActionKind::BuryCorpse => 2.0,
            ActionKind::CarryCorpse => 3.0,
            ActionKind::Wander => 1.0,
            ActionKind::CollectDole => 2.0,
            ActionKind::ServeTime => 60.0,
            ActionKind::StoreFood => 1.0,
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
            | ActionKind::Escort => ctx.target,
            _ => None,
        };
        ActionInstance { action: self, target, tile: None }
    }
}
