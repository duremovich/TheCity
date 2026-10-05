//! The `World`: entity arena, component stores, graph, blackboard and the
//! fixed-order tick.

use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::components::*;
use crate::config::Config;
use crate::entity::{Component, EntityId};
use crate::events::Event;
use crate::exec::{FlowField, Reservation};
use crate::levers::{Levers, PlayerCommand};
use crate::map::Map;
use crate::rng::SimRng;
use crate::stats::DailyStats;
use crate::systems;
use crate::time::{self, Tick, DAYS_PER_YEAR, TICKS_PER_DAY};

/// Plan-queue key: highest urgency first, then lowest id. `Reverse` has no
/// serde impl, so the ordering is flipped by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Urgency(pub OrderedFloat<f32>);

impl PartialOrd for Urgency {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Urgency {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.cmp(&self.0)
    }
}

/// Calibrated hourly behaviour table for Statistical agents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatRow {
    pub p_eat: f32,
    pub p_work: f32,
    pub p_social: f32,
    pub p_sleep: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatTable {
    pub morning: StatRow,
    pub work: StatRow,
    pub evening: StatRow,
    pub night: StatRow,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub tick: Tick,
    pub rng: SimRng,
    pub config: Config,
    pub map: Map,
    // entity allocator
    pub generations: Vec<u32>,
    /// Freed slots; `spawn` always reuses the lowest.
    pub free_list: BTreeSet<u32>,
    pub alive: Vec<bool>,
    // agent components (index = EntityId.index)
    pub position: Vec<Option<Position>>,
    pub identity: Vec<Option<Identity>>,
    pub needs: Vec<Option<Needs>>,
    pub personality: Vec<Option<Personality>>,
    pub mood: Vec<Option<Mood>>,
    pub wallet: Vec<Option<Wallet>>,
    pub inventory: Vec<Option<Inventory>>,
    pub job: Vec<Option<Job>>,
    pub household: Vec<Option<Household>>,
    pub brain: Vec<Option<Brain>>,
    pub memory: Vec<Option<Memory>>,
    pub skills: Vec<Option<Skills>>,
    pub sentence: Vec<Option<Sentence>>,
    pub gang_member: Vec<Option<GangMember>>,
    pub corpse: Vec<Option<Corpse>>,
    pub child: Vec<Option<Child>>,
    // non-agent components
    pub building: Vec<Option<Building>>,
    pub gang: Vec<Option<Gang>>,
    pub market: Vec<Option<Market>>,
    pub treasury: Vec<Option<Treasury>>,
    /// M9: on the Jail. Absent from older saves; `migrate_legacy` fills it.
    #[serde(default)]
    pub law: Vec<Option<Law>>,
    // graph + blackboard
    pub edges: BTreeMap<(EntityId, EntityId), Edge>,
    pub crime_reports: Vec<CrimeReport>,
    pub buildings_by_kind: BTreeMap<BuildingKind, Vec<EntityId>>,
    /// Rebuilt each tick for Full agents.
    pub agents_by_tile: BTreeMap<TilePos, Vec<EntityId>>,
    pub levers: Levers,
    pub stats: DailyStats,
    /// Ring of 5,000, never drained; the UI keeps a read cursor.
    pub events: VecDeque<Event>,
    /// Value = enqueue tick.
    pub plan_queue: BTreeMap<(Urgency, EntityId), Tick>,
    pub reservations: BTreeMap<EntityId, Vec<Reservation>>,
    pub door_queue: BTreeMap<TilePos, u8>,
    #[serde(skip)]
    pub flow_fields: BTreeMap<EntityId, FlowField>,
    pub last_seen: BTreeMap<EntityId, (TilePos, Tick)>,
    pub vacancies: BTreeMap<EntityId, Vec<Role>>,
    pub edge_roads: Vec<TilePos>,
    pub view_rect: Option<Rect>,
    pub command_queue: Vec<PlayerCommand>,
    pub command_log: Vec<(Tick, PlayerCommand)>,
    /// `None` until `citysim-cli calibrate` has produced `assets/stat_table.toml` (M7).
    pub stat_table: Option<StatTable>,
    /// BuyFood in progress: `(units, coins paid)` so a lost stock can be refunded.
    #[serde(default)]
    pub pending_purchase: BTreeMap<EntityId, (u32, i64)>,
    /// Adjacency index over `edges`, kept in step by `edge_entry` / `remove_edge`;
    /// rebuilt on load.
    #[serde(skip)]
    pub neighbours: BTreeMap<EntityId, BTreeSet<EntityId>>,
    /// Spouse lookup, both directions; written only by `set_spouse` / death.
    #[serde(skip)]
    pub spouses: BTreeMap<EntityId, EntityId>,
    /// Enemy edges by agent, both directions; kept by `social::reindex_kind`.
    #[serde(skip)]
    pub enemies: BTreeMap<EntityId, BTreeSet<EntityId>>,
    /// Name tables for births and immigrants; reloaded from assets on load.
    #[serde(skip)]
    pub names: NameTables,
}

use crate::components::{Law, LawShock};

/// Wire every component store: the `Component` impl, plus `grow`/`clear`
/// so spawn/despawn can never miss a store.
macro_rules! components {
    ($($field:ident : $ty:ty),* $(,)?) => {
        $(
            impl Component for $ty {
                fn store(world: &World) -> &Vec<Option<Self>> { &world.$field }
                fn store_mut(world: &mut World) -> &mut Vec<Option<Self>> { &mut world.$field }
            }
        )*
        impl World {
            pub(crate) fn grow_components(&mut self) {
                $( self.$field.push(None); )*
            }
            pub(crate) fn clear_components(&mut self, i: usize) {
                $( self.$field[i] = None; )*
            }
        }
    };
}

components! {
    position: Position,
    identity: Identity,
    needs: Needs,
    personality: Personality,
    mood: Mood,
    wallet: Wallet,
    inventory: Inventory,
    job: Job,
    household: Household,
    brain: Brain,
    memory: Memory,
    skills: Skills,
    sentence: Sentence,
    gang_member: GangMember,
    corpse: Corpse,
    child: Child,
    building: Building,
    gang: Gang,
    market: Market,
    treasury: Treasury,
    law: Law,
}

/// First/last name tables from `assets/names.txt`.
#[derive(Clone, Debug, Default)]
pub struct NameTables {
    first_male: Vec<String>,
    first_female: Vec<String>,
    last: Vec<String>,
}

impl NameTables {
    pub fn load(config: &Config) -> NameTables {
        let text =
            std::fs::read_to_string(config.asset("names.txt")).unwrap_or_else(|e| panic!("cannot read names.txt: {e}"));
        NameTables::parse(&text)
    }

    pub fn parse(text: &str) -> NameTables {
        let mut tables = NameTables { first_male: Vec::new(), first_female: Vec::new(), last: Vec::new() };
        let mut section = "";
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = match name {
                    "first_male" => "m",
                    "first_female" => "f",
                    "last" => "l",
                    other => panic!("names.txt: unknown section [{other}]"),
                };
                continue;
            }
            match section {
                "m" => tables.first_male.push(line.to_string()),
                "f" => tables.first_female.push(line.to_string()),
                "l" => tables.last.push(line.to_string()),
                _ => panic!("names.txt: name {line:?} before any section header"),
            }
        }
        assert!(
            !tables.first_male.is_empty() && !tables.first_female.is_empty() && !tables.last.is_empty(),
            "names.txt: every section needs at least one name"
        );
        tables
    }
}

impl World {
    /// Build the initial city: map, buildings, gang, 300 citizens with homes
    /// and jobs. Deterministic for a given `(seed, config)`.
    pub fn new(seed: u64, config: Config) -> World {
        let map = Map::load(&config.asset("map.txt"));
        let names = NameTables::load(&config);
        // Absent until `citysim-cli calibrate` (M7) has written it; any other
        // read failure is a broken checkout and must not pass silently.
        let stat_path = config.asset("stat_table.toml");
        let stat_table = match std::fs::read_to_string(&stat_path) {
            Ok(t) => Some(toml::from_str(&t).unwrap_or_else(|e| panic!("bad {}: {e}", stat_path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => panic!("cannot read {}: {e}", stat_path.display()),
        };

        let edge_roads = map.edge_roads();
        let levers = Levers::from_config(&config);
        let mut w = World {
            tick: 0,
            rng: SimRng::new(seed),
            config,
            map,
            generations: Vec::new(),
            free_list: BTreeSet::new(),
            alive: Vec::new(),
            position: Vec::new(),
            identity: Vec::new(),
            needs: Vec::new(),
            personality: Vec::new(),
            mood: Vec::new(),
            wallet: Vec::new(),
            inventory: Vec::new(),
            job: Vec::new(),
            household: Vec::new(),
            brain: Vec::new(),
            memory: Vec::new(),
            skills: Vec::new(),
            sentence: Vec::new(),
            gang_member: Vec::new(),
            corpse: Vec::new(),
            child: Vec::new(),
            building: Vec::new(),
            gang: Vec::new(),
            market: Vec::new(),
            treasury: Vec::new(),
            law: Vec::new(),
            edges: BTreeMap::new(),
            crime_reports: Vec::new(),
            buildings_by_kind: BTreeMap::new(),
            agents_by_tile: BTreeMap::new(),
            levers,
            stats: DailyStats::new(),
            events: VecDeque::new(),
            plan_queue: BTreeMap::new(),
            reservations: BTreeMap::new(),
            door_queue: BTreeMap::new(),
            flow_fields: BTreeMap::new(),
            last_seen: BTreeMap::new(),
            vacancies: BTreeMap::new(),
            edge_roads,
            view_rect: None,
            command_queue: Vec::new(),
            command_log: Vec::new(),
            stat_table,
            pending_purchase: BTreeMap::new(),
            neighbours: BTreeMap::new(),
            spouses: BTreeMap::new(),
            enemies: BTreeMap::new(),
            names: names.clone(),
        };
        w.spawn_buildings();
        w.spawn_gangs();
        w.spawn_population(&names);
        w
    }

    pub fn seed(&self) -> u64 {
        self.rng.seed()
    }

    // -----------------------------------------------------------------------
    // Initial world
    // -----------------------------------------------------------------------

    fn spawn_buildings(&mut self) {
        let defs = self.map.buildings.clone();
        for def in defs {
            let cfg = self.config.buildings.for_kind(def.kind);
            let stock_food = match def.kind {
                BuildingKind::Home => self.config.world.home_pantry_initial,
                BuildingKind::Market => self.config.world.market_initial,
                BuildingKind::Warehouse => self.config.world.warehouse_initial,
                _ => 0,
            };
            let capacity = cfg.capacity;
            let id = self.spawn();
            self.insert(
                id,
                Building {
                    kind: def.kind,
                    production_accum: 0.0,
                    claim: None,
                    child_food_debt: 0.0,
                    rect: def.rect,
                    door: def.door,
                    stock_food,
                    capacity,
                    owner: None,
                    occupants: Vec::new(),
                    demolished: false,
                },
            );
            match def.kind {
                BuildingKind::Market => {
                    let price = self.config.world.price_initial;
                    self.insert(id, Market { price_food: price, price_history: VecDeque::new() });
                }
                BuildingKind::Hall => {
                    let coins = self.config.world.treasury_initial;
                    self.insert(id, Treasury { coins });
                }
                BuildingKind::Jail => self.insert(id, Law::default()),
                _ => {}
            }
            self.buildings_by_kind.entry(def.kind).or_default().push(id);
        }
    }

    /// One gang per Hideout, in map order, named and funded from `[gangs]`.
    fn spawn_gangs(&mut self) {
        let hideouts = self.buildings_by_kind.get(&BuildingKind::Hideout).cloned().unwrap_or_default();
        let cfg = self.config.gangs.clone();
        for (i, hideout) in hideouts.into_iter().enumerate() {
            let id = self.spawn();
            let name = cfg.names.get(i).cloned().unwrap_or_else(|| format!("Gang {}", i + 1));
            let treasury = cfg.treasury_initial.get(i).or(cfg.treasury_initial.last()).copied().unwrap_or(0);
            self.insert(id, Gang::new(name, hideout, treasury));
        }
    }

    fn spawn_population(&mut self, names: &NameTables) {
        let wc = self.config.world.clone();
        let n = wc.population as usize;

        // --- individuals -------------------------------------------------
        let mut ids = Vec::with_capacity(n);
        for i in 0..n {
            let id = self.spawn();
            let sex = if i % 2 == 0 { Sex::Male } else { Sex::Female };
            let rng = self.rng.world();
            let first = match sex {
                Sex::Male => &names.first_male,
                Sex::Female => &names.first_female,
            };
            let first = first[rng.random_range(0..first.len())].clone();
            let last = names.last[rng.random_range(0..names.last.len())].clone();
            let years: f32 = rng.random_range(wc.age_min_years..wc.age_max_years);
            let age_days = (years * DAYS_PER_YEAR as f32) as u32;
            let born_tick = -(i64::from(age_days) * TICKS_PER_DAY as i64);
            let coins = rng.random_range(wc.initial_coins_min..=wc.initial_coins_max);
            let food = rng.random_range(wc.initial_food_min..=wc.initial_food_max);
            let personality = Personality::random(rng);
            let skills = Skills {
                stealth: rng.random_range(wc.skill_min..wc.skill_max),
                fighting: rng.random_range(wc.skill_min..wc.skill_max),
                farming: rng.random_range(wc.skill_min..wc.skill_max),
            };

            self.insert(
                id,
                Identity { name: format!("{first} {last}"), age_days, sex, born_tick, spouse_died_tick: None },
            );
            self.insert(
                id,
                Needs {
                    hunger: wc.needs_initial.hunger,
                    energy: wc.needs_initial.energy,
                    safety: wc.needs_initial.safety,
                    wealth: 0.0,
                    belonging: wc.needs_initial.belonging,
                    intimacy: wc.needs_initial.intimacy,
                    starving_since: None,
                },
            );
            self.insert(id, personality);
            self.insert(id, Mood::default());
            self.insert(id, Wallet { coins });
            self.insert(id, Inventory { food, stolen_food: 0 });
            self.insert(id, Brain::default());
            self.insert(id, Memory::default());
            self.insert(id, skills);
            self.insert(id, Household { home: None });
            ids.push(id);
        }
        self.recompute_wealth();

        // --- homes: seeded shuffle, residents_per_home each ----------------
        let homes = self.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default();
        let mut shuffled = ids.clone();
        shuffled.shuffle(self.rng.world());
        for (chunk, &home) in shuffled.chunks(wc.residents_per_home as usize).zip(homes.iter()) {
            let door = self.comp::<Building>(home).map(|b| b.door).unwrap_or_default();
            for &id in chunk {
                self.insert(id, Household { home: Some(home) });
                self.insert(id, Position { tile: door, building: None, entered: 0 });
                self.enter_building(id, home); // an interior slot each
            }
            if chunk.len() >= 2 && self.rng.world().random_bool(wc.spouse_p) {
                self.set_spouse(chunk[0], chunk[1]);
                let e = self.edge_entry(chunk[0], chunk[1]);
                e.affinity = 0.6;
                e.trust = 0.6;
            }
        }
        // Anyone left over (population > homes × residents) is homeless on the
        // road outside the Market door: on the street, so `building` is None.
        let market_door = self
            .building_of_kind(BuildingKind::Market)
            .and_then(|m| self.comp::<Building>(m).map(|b| (b.rect, b.door)))
            .unwrap_or_default();
        let street = self
            .map
            .neighbours4(market_door.1)
            .find(|&n| !market_door.0.contains(n) && self.map.tile_at(n) == TileKind::Road)
            .unwrap_or(market_door.1);
        for &id in &ids {
            if !self.has::<Position>(id) {
                self.insert(id, Position { tile: street, building: None, entered: 0 });
            }
        }

        // --- jobs: seeded shuffle, fixed role order ------------------------
        let mut pool = ids.clone();
        pool.shuffle(self.rng.world());
        let mut next = 0usize;
        for role in Role::ALL {
            let count = wc.jobs.count(role) as usize;
            let workplaces = self.buildings_by_kind.get(&role.workplace()).cloned().unwrap_or_default();
            for k in 0..count {
                let Some(&id) = pool.get(next) else { break };
                next += 1;
                let employer = workplaces.get(k % workplaces.len().max(1)).copied();
                let shifts = if role == Role::Guard && id.index % 2 == 0 {
                    wc.shift_night.clone()
                } else {
                    wc.shift_day.clone()
                };
                let wage_per_day = self.config.economy.wage(role);
                self.insert(
                    id,
                    Job {
                        employer,
                        role,
                        wage_per_day,
                        shifts,
                        days_unpaid: 0,
                        tax_accum: 0.0,
                        last_shift_day: None,
                        last_wage_attempt_day: None,
                    },
                );
            }
        }
    }

    /// `wealth = clamp(coins / (7 × price × (1 + greed)), 0, 1)` for every citizen.
    pub fn recompute_wealth(&mut self) {
        let price = self.market().map_or(1, |m| m.price_food).max(1) as f32;
        let days = self.config.needs.wealth_days_secure;
        for id in self.citizens() {
            let greed = self.comp::<Personality>(id).map_or(0.5, |p| p.greed);
            let coins = self.comp::<Wallet>(id).map_or(0, |w| w.coins) as f32;
            if let Some(n) = self.comp_mut::<Needs>(id) {
                n.wealth = (coins / (days * price * (1.0 + greed))).clamp(0.0, 1.0);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Living citizens: entities with an `Identity` and no `Corpse`, ascending.
    pub fn citizens(&self) -> Vec<EntityId> {
        self.entities().filter(|&id| self.has::<Identity>(id) && !self.has::<Corpse>(id)).collect()
    }

    pub fn population(&self) -> usize {
        self.citizens().len()
    }

    /// The first building of a kind (the only one for singletons).
    pub fn building_of_kind(&self, kind: BuildingKind) -> Option<EntityId> {
        self.buildings_by_kind.get(&kind).and_then(|v| v.first().copied())
    }

    pub fn market(&self) -> Option<&Market> {
        self.building_of_kind(BuildingKind::Market).and_then(|id| self.comp::<Market>(id))
    }

    pub fn market_mut(&mut self) -> Option<&mut Market> {
        self.building_of_kind(BuildingKind::Market).and_then(|id| self.comp_mut::<Market>(id))
    }

    pub fn treasury(&self) -> Option<&Treasury> {
        self.building_of_kind(BuildingKind::Hall).and_then(|id| self.comp::<Treasury>(id))
    }

    pub fn treasury_mut(&mut self) -> Option<&mut Treasury> {
        self.building_of_kind(BuildingKind::Hall).and_then(|id| self.comp_mut::<Treasury>(id))
    }

    /// M9: the law, on the Jail.
    pub fn law(&self) -> Option<&Law> {
        self.building_of_kind(BuildingKind::Jail).and_then(|id| self.comp::<Law>(id))
    }

    pub fn law_mut(&mut self) -> Option<&mut Law> {
        self.building_of_kind(BuildingKind::Jail).and_then(|id| self.comp_mut::<Law>(id))
    }

    /// Every gang, ascending by id (map order of their Hideouts).
    pub fn gangs(&self) -> Vec<EntityId> {
        self.with::<Gang>()
    }

    /// The gang an agent belongs to.
    pub fn gang_of(&self, agent: EntityId) -> Option<EntityId> {
        self.comp::<GangMember>(agent).map(|g| g.gang).filter(|&g| self.has::<Gang>(g))
    }

    /// A gang's Hideout, if the building still exists.
    pub fn hideout_of(&self, gang: EntityId) -> Option<EntityId> {
        self.comp::<Gang>(gang).map(|g| g.hideout).filter(|&h| self.has::<Building>(h))
    }

    /// The other gang; with more than two, the one holding the most territory.
    pub fn rival_of(&self, gang: EntityId) -> Option<EntityId> {
        self.gangs()
            .into_iter()
            .filter(|&g| g != gang)
            .filter_map(|g| self.comp::<Gang>(g).map(|gg| (std::cmp::Reverse(gg.territory.len()), g)))
            .min()
            .map(|(_, g)| g)
    }

    /// Position of a gang in `gangs()`: picks its colour in the app.
    pub fn gang_index(&self, gang: EntityId) -> usize {
        self.gangs().iter().position(|&g| g == gang).unwrap_or(0)
    }

    /// The agent's name, or `#index` for anything without an `Identity`.
    pub fn name_of(&self, id: EntityId) -> String {
        match self.comp::<Identity>(id) {
            Some(i) => i.name.clone(),
            None => match self.comp::<Building>(id) {
                Some(b) => format!("{}#{}", b.kind, id.index),
                None => format!("#{}", id.index),
            },
        }
    }

    pub fn tick_of_day(&self) -> u16 {
        time::tick_of_day(self.tick)
    }

    pub fn day(&self) -> u64 {
        time::day(self.tick)
    }

    pub fn phase(&self) -> time::DayPhase {
        time::phase(self.tick)
    }

    pub fn season(&self) -> time::Season {
        time::season(self.tick)
    }

    pub fn is_dark(&self) -> bool {
        time::is_dark(self.tick)
    }

    /// Called by the app every frame; queues a `SetView` command only when the
    /// rect differs from the last one queued (or applied, if none is pending),
    /// so a paused app does not flood the command log. Headless runs leave it `None`.
    pub fn set_view(&mut self, rect: Option<Rect>) {
        let pending = self.command_queue.iter().rev().find_map(|c| match c {
            PlayerCommand::SetView(r) => Some(*r),
            _ => None,
        });
        if pending.unwrap_or(self.view_rect) != rect {
            self.push_command(PlayerCommand::SetView(rect));
        }
    }

    // -----------------------------------------------------------------------
    // Tick
    // -----------------------------------------------------------------------

    /// One in-game minute, systems in the fixed order
    /// `commands, time, lod, needs, memory, think, plan, exec, economy, law,
    /// social, gang, demography, stats`.
    pub fn tick(&mut self) {
        self.apply_commands();
        // time: the clock is `self.tick`; daily hooks live in the systems that need them.
        systems::lod::run(self);
        crate::needs::run(self);
        systems::memory::run(self);
        crate::mood::run(self);
        systems::think::run(self);
        systems::plan::run(self);
        crate::exec::run(self);
        systems::economy::run(self);
        systems::law::run(self);
        systems::social::run(self);
        systems::gang::run(self);
        systems::demography::run(self);
        systems::stats::run(self);
        self.tick += 1;
    }

    // -----------------------------------------------------------------------
    // Shared mutations used by several systems
    // -----------------------------------------------------------------------

    /// Add a memory, evicting the lowest `salience × recency` entry past the cap.
    pub fn remember(
        &mut self,
        id: EntityId,
        kind: MemoryKind,
        subject: Option<EntityId>,
        salience: f32,
        valence: f32,
        second_hand: bool,
    ) {
        let tick = self.tick;
        let mut cap = self.config.brain.memory_cap;
        let half_life = self.config.brain.memory_half_life_days;
        // A Statistical agent keeps only what the hourly tick can act on, in
        // eight slots; the entries survive promotion, when the cap becomes 24.
        if self.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
            if !matches!(kind, MemoryKind::Grief | MemoryKind::WasRobbed | MemoryKind::MetInJail) {
                return;
            }
            cap = 8;
        }
        let Some(m) = self.comp_mut::<Memory>(id) else { return };
        let entry = MemoryEntry { kind, subject, tick, salience, valence, second_hand, crime: None };
        systems::memory::insert(m, entry, tick, cap, half_life);
    }

    /// The edge between two agents, if any.
    pub fn edge(&self, a: EntityId, b: EntityId) -> Option<&Edge> {
        self.edges.get(&edge_key(a, b))
    }

    /// The edge between two agents, created as an Acquaintance at affinity 0
    /// if absent. Keeps the neighbour index in step.
    pub fn edge_entry(&mut self, a: EntityId, b: EntityId) -> &mut Edge {
        let key = edge_key(a, b);
        let tick = self.tick;
        if !self.edges.contains_key(&key) {
            self.neighbours.entry(a).or_default().insert(b);
            self.neighbours.entry(b).or_default().insert(a);
        }
        self.edges.entry(key).or_insert_with(|| Edge::new(RelKind::Acquaintance, tick))
    }

    pub fn remove_edge(&mut self, a: EntityId, b: EntityId) {
        self.edges.remove(&edge_key(a, b));
        for (x, y) in [(a, b), (b, a)] {
            if let Some(s) = self.enemies.get_mut(&x) {
                s.remove(&y);
            }
        }
        if let Some(n) = self.neighbours.get_mut(&a) {
            n.remove(&b);
        }
        if let Some(n) = self.neighbours.get_mut(&b) {
            n.remove(&a);
        }
    }

    /// A first name for `sex` from the tables (seeded).
    pub fn random_first_name(&mut self, sex: Sex) -> String {
        let table = match sex {
            Sex::Male => &self.names.first_male,
            Sex::Female => &self.names.first_female,
        };
        if table.is_empty() {
            return "Nameless".to_string();
        }
        let i = self.rng.world().random_range(0..table.len());
        table[i].clone()
    }

    /// A full name from the tables (seeded).
    pub fn random_name(&mut self, sex: Sex) -> String {
        let first = self.random_first_name(sex);
        let last = if self.names.last.is_empty() {
            "Doe".to_string()
        } else {
            let i = self.rng.world().random_range(0..self.names.last.len());
            self.names.last[i].clone()
        };
        format!("{first} {last}")
    }

    /// After a load: the tables are not saved.
    pub fn reload_names(&mut self) {
        self.names = NameTables::load(&self.config);
    }

    /// Remove an entity from every index and free it (emigrants, rotted or
    /// buried corpses). Edges to it are dropped.
    pub fn remove_agent(&mut self, id: EntityId) {
        if !self.is_alive(id) {
            return;
        }
        self.abort_plan(id);
        self.vacate_job(id);
        self.remove_from_building(id);
        if self.has::<GangMember>(id) {
            systems::gang::leave(self, id, "gone");
        }
        crate::systems::social::unlink(self, id);
        for o in self.neighbours(id).collect::<Vec<_>>() {
            self.remove_edge(id, o);
        }
        self.neighbours.remove(&id);
        self.release_all(id);
        self.reservations.retain(|_, rs| {
            rs.retain(|r| !matches!(r.kind, crate::exec::ReservationKind::Partner { other } | crate::exec::ReservationKind::Corpse { corpse: other } | crate::exec::ReservationKind::Suspect { suspect: other } if other == id));
            !rs.is_empty()
        });
        self.pending_purchase.remove(&id);
        self.plan_queue.retain(|&(_, who), _| who != id);
        self.last_seen.remove(&id);
        self.crime_reports.retain(|r| r.suspect != id);
        for id2 in self.citizens() {
            if let Some(b) = self.comp_mut::<Brain>(id2) {
                if b.escorting == Some(id) {
                    b.escorting = None;
                }
                if b.cuffed_by == Some(id) {
                    b.cuffed_by = None;
                }
                if b.carrying_corpse == Some(id) {
                    b.carrying_corpse = None;
                }
            }
        }
        self.despawn(id);
    }

    /// Rebuild `neighbours`, `spouses` and `enemies` from `edges` (after a
    /// load; the indices are not saved).
    pub fn rebuild_indices(&mut self) {
        self.neighbours.clear();
        self.spouses.clear();
        self.enemies.clear();
        let living = |w: &World, id: EntityId| w.has::<Identity>(id) && !w.has::<Corpse>(id);
        let edges: Vec<((EntityId, EntityId), RelKind)> = self.edges.iter().map(|(&k, e)| (k, e.kind)).collect();
        for ((a, b), kind) in edges {
            self.neighbours.entry(a).or_default().insert(b);
            self.neighbours.entry(b).or_default().insert(a);
            match kind {
                RelKind::Spouse if living(self, a) && living(self, b) => {
                    self.spouses.insert(a, b);
                    self.spouses.insert(b, a);
                }
                RelKind::Enemy => {
                    self.enemies.entry(a).or_default().insert(b);
                    self.enemies.entry(b).or_default().insert(a);
                }
                _ => {}
            }
        }
    }

    /// Fix up a save written before M8: a gang without a Hideout (the serde
    /// default) takes the Hideouts in map order, its territory keeps Homes
    /// only, and every held Home gets a full claim. A gang that already has a
    /// Hideout is left alone: its claims are live state.
    pub fn migrate_legacy(&mut self) {
        // M9: a save from before the law had a brain has no `law` store.
        let n = self.alive.len();
        if self.law.len() < n {
            self.law.resize_with(n, || None);
        }
        if let Some(jail) = self.building_of_kind(BuildingKind::Jail) {
            if !self.has::<Law>(jail) {
                self.insert(jail, Law::default());
            }
        }
        let hideouts = self.buildings_by_kind.get(&BuildingKind::Hideout).cloned().unwrap_or_default();
        for (i, gang) in self.gangs().into_iter().enumerate() {
            let Some(g) = self.comp::<Gang>(gang) else { continue };
            if self.has::<Building>(g.hideout) {
                continue;
            }
            let hideout = hideouts.get(i).or(hideouts.first()).copied().unwrap_or(EntityId::NONE);
            let territory: Vec<EntityId> = g
                .territory
                .iter()
                .copied()
                .filter(|&b| self.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Home))
                .collect();
            if let Some(g) = self.comp_mut::<Gang>(gang) {
                g.hideout = hideout;
                g.territory = territory.clone();
            }
            for home in territory {
                if let Some(b) = self.comp_mut::<Building>(home) {
                    b.claim = Some(Claim { gang, count: systems::gang::CLAIM_HELD });
                }
            }
        }
    }

    /// Everyone `id` has an edge with, ascending.
    pub fn neighbours(&self, id: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.neighbours.get(&id).into_iter().flat_map(|s| s.iter().copied())
    }

    /// Marry two agents: a Spouse edge and the lookup in both directions.
    pub fn set_spouse(&mut self, a: EntityId, b: EntityId) {
        let tick = self.tick;
        let e = self.edge_entry(a, b);
        e.kind = RelKind::Spouse;
        e.last_interaction = tick;
        self.spouses.insert(a, b);
        self.spouses.insert(b, a);
    }

    pub fn spouse_of(&self, id: EntityId) -> Option<EntityId> {
        self.spouses.get(&id).copied()
    }

    /// Everyone on an Enemy edge with `id`.
    pub fn enemies_of(&self, id: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.enemies.get(&id).into_iter().flat_map(|s| s.iter().copied())
    }

    /// A SawCrime memory that also records which crime was seen.
    pub fn remember_crime(&mut self, id: EntityId, subject: EntityId, crime: Crime, salience: f32) {
        let tick = self.tick;
        let cap = self.config.brain.memory_cap;
        let half_life = self.config.brain.memory_half_life_days;
        let Some(m) = self.comp_mut::<Memory>(id) else { return };
        let entry = MemoryEntry {
            kind: MemoryKind::SawCrime,
            subject: Some(subject),
            tick,
            salience,
            valence: -salience,
            second_hand: false,
            crime: Some(crime),
        };
        systems::memory::insert(m, entry, tick, cap, half_life);
    }

    /// Remove the Job, posting a vacancy at the employer. Returns the old Job.
    pub fn vacate_job(&mut self, id: EntityId) -> Option<Job> {
        let job = self.remove::<Job>(id)?;
        if let Some(employer) = job.employer {
            self.vacancies.entry(employer).or_default().push(job.role);
        }
        Some(job)
    }

    /// Drop the agent from its building's occupant list; the Position is left
    /// to the caller (a leaver moves to the street, a corpse stays put).
    pub fn remove_from_building(&mut self, id: EntityId) {
        let Some(here) = self.comp::<Position>(id).and_then(|p| p.building) else { return };
        if let Some(b) = self.comp_mut::<Building>(here) {
            if let Ok(i) = b.occupants.binary_search(&id) {
                b.occupants.remove(i);
            }
        }
    }

    /// Death: every component but `Position` and `Identity` goes, a `Corpse`
    /// is added, the building and household forget the agent, the job is
    /// vacated, and the event and daily counter are recorded.
    pub fn kill(&mut self, id: EntityId, cause: DeathCause) {
        self.kill_by(id, cause, None);
    }

    /// `kill` with the killer named, so a gang knows whether a rival did it.
    pub fn kill_by(&mut self, id: EntityId, cause: DeathCause, killer: Option<EntityId>) {
        if !self.has::<Identity>(id) || self.has::<Corpse>(id) {
            return;
        }
        let tick = self.tick;
        let name = self.name_of(id);
        // A dead guard lets their suspect go; a dead suspect frees their guard.
        if let Some(b) = self.comp::<Brain>(id) {
            let (escorting, cuffed_by) = (b.escorting, b.cuffed_by);
            if let Some(s) = escorting {
                if let Some(sb) = self.comp_mut::<Brain>(s) {
                    sb.cuffed_by = None;
                }
            }
            if let Some(g) = cuffed_by {
                if let Some(gb) = self.comp_mut::<Brain>(g) {
                    gb.escorting = None;
                }
            }
        }
        // Witnesses, grief, widowhood and inheritance read the living state.
        systems::demography::on_death(self, id);
        crate::systems::social::on_death(self, id);
        if systems::law::is_guard(self, id) {
            systems::law_brain::push_shock(self, LawShock::GuardKilled);
        }
        self.vacate_job(id);
        self.remove_from_building(id);
        if self.has::<GangMember>(id) {
            systems::gang::on_member_killed(self, id, killer);
        }
        self.remove::<Needs>(id);
        self.remove::<Personality>(id);
        self.remove::<Mood>(id);
        self.remove::<Wallet>(id);
        self.remove::<Inventory>(id);
        self.remove::<Household>(id);
        self.remove::<Brain>(id);
        self.remove::<Memory>(id);
        self.remove::<Skills>(id);
        self.remove::<Sentence>(id);
        self.remove::<GangMember>(id);
        self.remove::<Child>(id);
        self.release_all(id);
        self.pending_purchase.remove(&id);
        self.plan_queue.retain(|&(_, who), _| who != id);
        self.insert(id, Corpse { died_tick: tick, cause, buried: false, buried_tick: None });
        match cause {
            DeathCause::Starvation => self.stats.current.deaths_starvation += 1,
            DeathCause::OldAge => self.stats.current.deaths_old_age += 1,
            DeathCause::Violence | DeathCause::Execution => self.stats.current.deaths_violence += 1,
        }
        self.push_event(crate::events::EventKind::Death, &[id], format!("{name} died of {cause:?}"));
    }

    /// Rebuild a world from its seed and command log: the commands are queued
    /// at the ticks they were applied, and the world runs to `end_tick`.
    pub fn replay(seed: u64, config: Config, log: &[(Tick, PlayerCommand)], end_tick: Tick) -> World {
        let mut w = World::new(seed, config);
        let mut next = 0;
        while w.tick < end_tick {
            while next < log.len() && log[next].0 == w.tick {
                w.push_command(log[next].1.clone());
                next += 1;
            }
            w.tick();
        }
        w
    }

    /// Run `n` ticks.
    pub fn run_ticks(&mut self, n: u64) {
        for _ in 0..n {
            self.tick();
        }
    }
}
