//! The `World`: entity arena, component stores, graph, blackboard and the
//! fixed-order tick.

use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::components::*;
use crate::config::Config;
use crate::entity::{Component, EntityId};
use crate::events::Event;
use crate::exec::{FlowCache, Reservation};
use crate::levers::{Levers, PlayerCommand};
use crate::map::Map;
use crate::rng::SimRng;
use crate::stats::DailyStats;
use crate::systems;
use crate::time::{self, Tick, DAYS_PER_YEAR, TICKS_PER_DAY};

/// Position of a role in `Role::ALL`.
fn role_index(role: Role) -> usize {
    Role::ALL.iter().position(|&r| r == role).expect("every role is in Role::ALL")
}

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

/// Rows in a v2 table: 4 phases x 3 lawfulness buckets x 2 hunger buckets.
pub const STAT_ROWS: usize = 24;

/// One row of the calibrated hourly table (M10 v2). The first four are one
/// outcome drawn per hour, as in v1; the rest are independent hourly rolls.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct StatRow {
    /// "Morning law0 hunger1", for humans.
    #[serde(default)]
    pub label: String,
    pub p_eat: f32,
    pub p_work: f32,
    pub p_social: f32,
    pub p_sleep: f32,
    /// Actor side: Market theft beyond the eat path's desperation theft.
    pub p_steal: f32,
    /// Actor side: a Flirt with a known edge of affinity >= 0.3.
    pub p_flirt: f32,
    /// Victim side: the actor is a hole until bound.
    pub p_robbed: f32,
    pub p_assaulted: f32,
    pub p_killed: f32,
    /// Independent roll: the agent meets a stranger from its zone (a new
    /// edge). `calibrate`: half the Full agents' new non-co-worker edge ends
    /// per agent-hour (each meeting gives two agents an edge), pooled over
    /// hunger like the other rolls. Full agents meet in every phase (the
    /// Market, the street, home), not in Social hours: per Social hour the
    /// rate exceeded 1 in every row. 0 in a table without it.
    #[serde(default)]
    pub p_meet: f32,
    /// Independent roll: a Chat (a known edge's drift). `calibrate`: Full
    /// Chats begun per agent-hour, pooled over hunger. A Chat is a few
    /// minutes of an hour some other category dominates, so the Social
    /// outcome (dominant hours) undercounted them by about half.
    #[serde(default)]
    pub p_chat: f32,
    /// Given a Chat: it is with a housemate rather than a random known edge.
    /// `calibrate`: the share of Full Chats begun with someone of the same
    /// Home, pooled over hunger (a Full agent chats with whoever is there,
    /// and at home that is the household).
    #[serde(default)]
    pub p_chat_home: f32,
}

/// Calibrated hourly behaviour table for Statistical agents (`citysim-cli
/// calibrate`, v2). Rows by `phase x 6 + lawfulness bucket x 2 + hunger bucket`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatTable {
    /// 2.
    pub version: u32,
    pub seed: u64,
    pub days: u64,
    pub agents: u32,
    /// `[0.3, 0.7]`: lawfulness `< e0` is bucket 0, `< e1` bucket 1, else 2.
    pub lawfulness_edges: [f32; 2],
    /// `0.4`: hunger `< edge` is bucket 0 (hungry), else 1.
    pub hunger_edge: f32,
    /// The chance a jobless Full agent who starts the Work phase below its
    /// savings line (`SAVINGS_DAYS` meals) collects the dole that day; it
    /// has to walk to the Hall. 1 in a table without it.
    #[serde(default = "one_f32")]
    pub p_dole_day: f32,
    /// The chance an off-screen theft is reported and its thief given a body
    /// (`lod::stat_theft`): Full arrests per Full theft. Off screen that
    /// report is the only road to an arrest (an Assaulted hole files its
    /// witness's report only when bound, at `hole_ttl_days`), and nearly
    /// every one ends in one, so it carries the Full city's whole arrest
    /// rate; the Full share of thefts reported (0.53) put Statistical
    /// arrests 18 % over Full. A table without it falls back to
    /// `[crime] stat_theft_caught_p`.
    #[serde(default)]
    pub p_theft_caught: Option<f32>,
    pub rows: Vec<StatRow>,
}

fn one_f32() -> f32 {
    1.0
}

impl StatTable {
    /// Morning 0, Work 1, Evening 2, Night 3 (`DayPhase` declares Night first).
    pub fn phase_index(phase: time::DayPhase) -> usize {
        match phase {
            time::DayPhase::Morning => 0,
            time::DayPhase::Work => 1,
            time::DayPhase::Evening => 2,
            time::DayPhase::Night => 3,
        }
    }

    pub fn lawfulness_bucket(&self, lawfulness: f32) -> usize {
        if lawfulness < self.lawfulness_edges[0] {
            0
        } else if lawfulness < self.lawfulness_edges[1] {
            1
        } else {
            2
        }
    }

    pub fn hunger_bucket(&self, hunger: f32) -> usize {
        usize::from(hunger >= self.hunger_edge)
    }

    /// `phase x 6 + lawfulness bucket x 2 + hunger bucket`.
    pub fn index(&self, phase: time::DayPhase, lawfulness: f32, hunger: f32) -> usize {
        Self::phase_index(phase) * 6 + self.lawfulness_bucket(lawfulness) * 2 + self.hunger_bucket(hunger)
    }

    pub fn row(&self, phase: time::DayPhase, lawfulness: f32, hunger: f32) -> &StatRow {
        &self.rows[self.index(phase, lawfulness, hunger)]
    }

    /// "Morning law0 hunger1" for row `i`.
    pub fn label(i: usize) -> String {
        let phase = ["Morning", "Work", "Evening", "Night"][i / 6];
        format!("{phase} law{} hunger{}", (i % 6) / 2, i % 2)
    }
}

/// Read `assets/stat_table.toml`: `(table, legacy)`. Missing gives `(None,
/// false)`; the pre-M10 four-row table `(None, true)` (M10 D28); a v2 table
/// with the wrong row count, or anything unparsable, panics.
pub fn load_stat_table(config: &Config) -> (Option<StatTable>, bool) {
    let path = config.asset("stat_table.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (None, false),
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    };
    match toml::from_str::<StatTable>(&text) {
        Ok(t) => {
            assert!(
                t.rows.len() == STAT_ROWS,
                "{}: {} rows, expected {STAT_ROWS}: run `cargo run --release -p citysim-cli -- calibrate`",
                path.display(),
                t.rows.len()
            );
            (Some(t), false)
        }
        Err(_) if text.contains("[morning]") => (None, true),
        Err(e) => panic!("bad {}: {e}", path.display()),
    }
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
    /// M10: per adult, kept after death. Absent from older saves; `migrate_legacy` fills it.
    #[serde(default)]
    pub trace: Vec<Option<Trace>>,
    /// M10: per agent, kept after death. Absent from older saves; `migrate_legacy` fills it.
    #[serde(default)]
    pub life: Vec<Option<Life>>,
    /// M11: corps, each its own entity. Absent from older saves; `migrate_legacy` fills it.
    #[serde(default)]
    pub corp: Vec<Option<Corp>>,
    // graph + blackboard
    pub edges: BTreeMap<(EntityId, EntityId), Edge>,
    /// Private so every write goes through `reports_mut`, which drops the
    /// per-suspect index; read with `crime_reports()`.
    crime_reports: Vec<CrimeReport>,
    /// Per suspect: (open reports, latest report tick). Built lazily from
    /// `crime_reports` on the first read after a write (M10 phase 5b: the
    /// warrant checks walked every report per citizen per hour).
    #[serde(skip)]
    report_index: std::sync::OnceLock<ReportIndex>,
    /// Every gang, ascending by id; kept by the Gang insert/remove hooks and
    /// rebuilt on load (`gangs()` was a full entity scan).
    #[serde(skip)]
    gang_ids: Vec<EntityId>,
    /// Every corp, ascending by id; kept by the Corp insert/remove hooks and
    /// rebuilt on load (a Corp store scan is ~600 bytes a slot, and the corp
    /// brain asks several times a rescoring).
    #[serde(skip)]
    corp_ids: Vec<EntityId>,
    /// Residents by Home, each list ascending; kept by the Household hooks
    /// and `set_home`, rebuilt on load (M10 review: the per-Home
    /// `citizens()` scans). `resident_home` is the reverse, so a removed
    /// Household can be unfiled.
    #[serde(skip)]
    residents: BTreeMap<EntityId, Vec<EntityId>>,
    #[serde(skip)]
    resident_home: BTreeMap<EntityId, EntityId>,
    /// `mean_price` for the tick it was read at, for the per-agent wealth
    /// refresh; `economy::daily_price` drops it.
    #[serde(skip)]
    pub(crate) mean_price_cache: Option<(Tick, i64)>,
    /// GangWork's "no guard within `sight_day_crime` of the door" per Home,
    /// keyed on the guards' tiles: reused until a guard moves.
    #[serde(skip)]
    pub guarded_homes: GuardedHomes,
    /// M11: a corp's pending shocks reached `[corps] shock_severity_rethink`
    /// (set by `ownership::push_corp_shock`), so `corp_brain::run` scans the
    /// corps this tick. Shocks are not saved, so neither is this.
    #[serde(skip)]
    pub corp_rethink: bool,
    pub buildings_by_kind: BTreeMap<BuildingKind, Vec<EntityId>>,
    /// Rebuilt each tick for Full agents.
    pub agents_by_tile: BTreeMap<TilePos, Vec<EntityId>>,
    pub levers: Levers,
    pub stats: DailyStats,
    /// Ring of 50,000, never drained; the UI keeps a read cursor.
    pub events: VecDeque<Event>,
    /// The last `DEBUG_RING_CAP` `PlanAborted` events; not saved.
    #[serde(skip)]
    pub debug_events: VecDeque<Event>,
    /// The id the next `push_event` assigns (M10 D12).
    #[serde(default)]
    pub next_event_id: u64,
    /// Value = enqueue tick.
    pub plan_queue: BTreeMap<(Urgency, EntityId), Tick>,
    pub reservations: BTreeMap<EntityId, Vec<Reservation>>,
    pub door_queue: BTreeMap<TilePos, u8>,
    /// Lazily built, LRU-evicted (`[exec] flow_field_cache`); pure functions of map and door.
    #[serde(skip)]
    pub flow_fields: FlowCache,
    pub last_seen: BTreeMap<EntityId, (TilePos, Tick)>,
    pub vacancies: BTreeMap<EntityId, Vec<Role>>,
    pub edge_roads: Vec<TilePos>,
    pub view_rect: Option<Rect>,
    pub command_queue: Vec<PlayerCommand>,
    pub command_log: Vec<(Tick, PlayerCommand)>,
    /// `None` until `citysim-cli calibrate` has produced `assets/stat_table.toml`.
    /// Not saved (M10 D28): reloaded from the assets on load, like the names.
    #[serde(skip)]
    pub stat_table: Option<StatTable>,
    /// The assets hold the pre-M10 four-row table: `run_statistical` panics
    /// with the calibrate message.
    #[serde(skip)]
    pub stat_table_legacy: bool,
    /// M10: open holes, oldest first (the key packs the tick on top).
    #[serde(default)]
    pub holes: BTreeMap<HoleId, Hole>,
    /// Open holes by victim; rebuilt on load.
    #[serde(skip)]
    pub holes_by_agent: BTreeMap<EntityId, SmallVec<[HoleId; 4]>>,
    /// Victim holes of agents promoted off the Statistical tier, bound at the
    /// end of `lod::run` (M10 D32).
    #[serde(default)]
    pub bind_queue: Vec<HoleId>,
    /// Guard-on-shift ticks per zone, for `bind::zone_law_coverage`.
    #[serde(default)]
    pub zone_watch: ZoneWatch,
    /// Today's ATE and SLEPT_AT_HOME bits per agent, folded into the trace at day end.
    #[serde(default)]
    pub day_marks: BTreeMap<EntityId, u8>,
    /// BuyFood in progress: `(units, coins paid)` so a lost stock can be refunded.
    #[serde(default)]
    pub pending_purchase: BTreeMap<EntityId, (u32, i64)>,
    /// M11 D2: the fractional tax owed per payee, withheld whole coins at a time.
    #[serde(default)]
    pub tax_accum: BTreeMap<EntityId, f32>,
    /// M11: eviction ticks over the last 60 days, oldest first.
    #[serde(default)]
    pub eviction_log: VecDeque<Tick>,
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
    /// Agents by LOD (`Lod as usize`), ascending. Kept by the Brain hooks and `retier`; rebuilt on load.
    #[serde(skip)]
    pub by_tier: [Vec<EntityId>; 3],
    /// Job holders by `Role` (index in `Role::ALL`), ascending. Kept by the Job hooks; rebuilt on load.
    #[serde(skip)]
    pub by_role: [Vec<EntityId>; 5],
    /// The Statistical tier bucketed by hourly slot (`id.index % 60`), each
    /// ascending: the spread tick reads one bucket per tick. Kept with `by_tier`.
    #[serde(skip)]
    pub stat_slots: StatSlots,
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
    trace: Trace,
    life: Life,
    corp: Corp,
}

/// Per suspect `(open reports, latest report tick)`, and the suspects with an
/// open report, ascending.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportIndex {
    pub by_suspect: BTreeMap<EntityId, (u32, Tick)>,
    pub open: Vec<EntityId>,
}

/// For `gang::gang_work_target`: `guarded[i]` = a guard stands within
/// `sight_day_crime` of the door of the `i`-th Home in `buildings_by_kind`,
/// computed for the guard tiles in the key. A pure function of the key, the
/// Home list and the config, so reusing it changes nothing.
#[derive(Default, Debug)]
pub struct GuardedHomes(pub std::sync::Mutex<Option<GuardedKeyed>>);

/// The guard tiles a `GuardedHomes` entry was computed for, and the flags.
pub type GuardedKeyed = (Vec<TilePos>, std::sync::Arc<Vec<bool>>);

impl Clone for GuardedHomes {
    fn clone(&self) -> Self {
        GuardedHomes::default()
    }
}

/// Statistical agents by hourly slot: `slots[s]` holds the ids with
/// `index % 60 == s`, ascending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatSlots(pub Vec<Vec<EntityId>>);

impl Default for StatSlots {
    fn default() -> Self {
        StatSlots(vec![Vec::new(); time::TICKS_PER_HOUR as usize])
    }
}

impl StatSlots {
    fn slot_of(id: EntityId) -> usize {
        (u64::from(id.index) % time::TICKS_PER_HOUR) as usize
    }

    /// The bucket for `tick % 60`.
    pub fn at(&self, tick: Tick) -> &[EntityId] {
        &self.0[(tick % time::TICKS_PER_HOUR) as usize]
    }
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
        let map = Map::load(&config.asset(&config.world.map));
        let names = NameTables::load(&config);
        // Absent until `citysim-cli calibrate` has written it; any other read
        // failure is a broken checkout and must not pass silently.
        let (stat_table, stat_table_legacy) = load_stat_table(&config);

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
            trace: Vec::new(),
            life: Vec::new(),
            corp: Vec::new(),
            edges: BTreeMap::new(),
            crime_reports: Vec::new(),
            report_index: std::sync::OnceLock::new(),
            gang_ids: Vec::new(),
            corp_ids: Vec::new(),
            residents: BTreeMap::new(),
            resident_home: BTreeMap::new(),
            mean_price_cache: None,
            guarded_homes: GuardedHomes::default(),
            corp_rethink: false,
            buildings_by_kind: BTreeMap::new(),
            agents_by_tile: BTreeMap::new(),
            levers,
            stats: DailyStats::new(),
            events: VecDeque::new(),
            debug_events: VecDeque::new(),
            next_event_id: 0,
            plan_queue: BTreeMap::new(),
            reservations: BTreeMap::new(),
            door_queue: BTreeMap::new(),
            flow_fields: FlowCache::default(),
            last_seen: BTreeMap::new(),
            vacancies: BTreeMap::new(),
            edge_roads,
            view_rect: None,
            command_queue: Vec::new(),
            command_log: Vec::new(),
            stat_table,
            stat_table_legacy,
            holes: BTreeMap::new(),
            holes_by_agent: BTreeMap::new(),
            bind_queue: Vec::new(),
            zone_watch: ZoneWatch::default(),
            day_marks: BTreeMap::new(),
            pending_purchase: BTreeMap::new(),
            tax_accum: BTreeMap::new(),
            eviction_log: VecDeque::new(),
            neighbours: BTreeMap::new(),
            spouses: BTreeMap::new(),
            enemies: BTreeMap::new(),
            by_tier: Default::default(),
            by_role: Default::default(),
            stat_slots: StatSlots::default(),
            names: names.clone(),
        };
        w.spawn_buildings();
        w.spawn_gangs();
        w.spawn_population(&names);
        systems::ownership::seed(&mut w);
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
                    tier: def.tier,
                    rent_per_day: 0,
                    revenue_today: 0,
                    revenue: VecDeque::new(),
                    secured_by: None,
                },
            );
            match def.kind {
                BuildingKind::Market => {
                    let price = self.config.world.price_initial;
                    self.insert(id, Market::new(price));
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
            self.insert(id, Household::new(None));
            ids.push(id);
        }
        self.recompute_wealth();

        // --- homes: seeded shuffle, residents_per_home each ----------------
        let mut homes = self.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default();
        // Under capacity, spread the households over the whole map by even
        // stride (M10 D24), so a 500-agent world still spans every zone.
        let per = (wc.residents_per_home as usize).max(1);
        let needed = n.div_ceil(per);
        if needed < homes.len() {
            homes = (0..needed).map(|k| homes[k * homes.len() / needed]).collect();
        }
        let mut shuffled = ids.clone();
        shuffled.shuffle(self.rng.world());
        for (chunk, &home) in shuffled.chunks(wc.residents_per_home as usize).zip(homes.iter()) {
            let door = self.comp::<Building>(home).map(|b| b.door).unwrap_or_default();
            for &id in chunk {
                self.insert(id, Household::new(Some(home)));
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
        // Under capacity, an empty Home starts with an empty pantry: in the
        // 500-resident calibration city 300 unwatched pantries of opening food
        // were free loot, and the Full thieves the table learns from cleared
        // them (M10 phase 5b).
        let used: BTreeSet<EntityId> = homes.iter().copied().take(n.div_ceil(per)).collect();
        for h in self.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default() {
            if !used.contains(&h) {
                if let Some(b) = self.comp_mut::<Building>(h) {
                    b.stock_food = 0;
                }
            }
        }
        // M11: inequality from day one. The coin draw above is scaled by the
        // tier of the Home the agent was dealt (no new draws: the world
        // stream is untouched). The homeless keep the draw.
        let mult = wc.coins_by_tier;
        if mult != [1.0, 1.0, 1.0] {
            for &id in &ids {
                let Some(tier) = self
                    .comp::<Household>(id)
                    .and_then(|h| h.home)
                    .and_then(|h| self.comp::<Building>(h))
                    .map(|b| usize::from(b.tier.min(2)))
                else {
                    continue;
                };
                if let Some(w) = self.comp_mut::<Wallet>(id) {
                    w.coins = (w.coins as f32 * mult[tier]).round() as i64;
                }
            }
            self.recompute_wealth();
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
        // M10: each slot goes to the unassigned resident living nearest the
        // workplace (home door to workplace door; ties in shuffle order), as
        // `job_search` already hires. On the 256 x 192 map a shuffled
        // assignment put commutes at 100-300 tiles, most of a shift.
        let mut pool = ids.clone();
        pool.shuffle(self.rng.world());
        let home_door: BTreeMap<EntityId, TilePos> = pool
            .iter()
            .filter_map(|&id| {
                let home = self.comp::<Household>(id).and_then(|h| h.home)?;
                Some((id, self.comp::<Building>(home)?.door))
            })
            .collect();
        let mut taken = vec![false; pool.len()];
        for role in Role::ALL {
            let count = wc.jobs.count(role) as usize;
            let workplaces = self.buildings_by_kind.get(&role.workplace()).cloned().unwrap_or_default();
            for k in 0..count {
                let employer = workplaces.get(k % workplaces.len().max(1)).copied();
                let work_door = employer.and_then(|e| self.comp::<Building>(e)).map(|b| b.door);
                let pick = pool
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| !taken[i])
                    .map(|(i, id)| {
                        let d = match (home_door.get(id), work_door) {
                            (Some(h), Some(w)) => h.manhattan(w),
                            _ => u32::MAX,
                        };
                        (d, i)
                    })
                    .min();
                let Some((_, i)) = pick else { break };
                taken[i] = true;
                let id = pool[i];
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
                        duty_ticks: 0,
                        hired_tick: 0,
                    },
                );
            }
        }
    }

    /// `wealth = clamp(coins / (7 × price × (1 + greed)), 0, 1)` for every citizen.
    pub fn recompute_wealth(&mut self) {
        for id in self.citizens() {
            // scan-ok: hourly
            self.recompute_wealth_for(id);
        }
    }

    /// `recompute_wealth` for one agent (the spread Statistical tick calls it
    /// per processed agent).
    pub fn recompute_wealth_for(&mut self, id: EntityId) {
        let tick = self.tick;
        let price = match self.mean_price_cache {
            Some((t, p)) if t == tick => p,
            _ => {
                let p = self.mean_price();
                self.mean_price_cache = Some((tick, p));
                p
            }
        };
        let price = price.max(1) as f32;
        let days = self.config.needs.wealth_days_secure;
        let greed = self.comp::<Personality>(id).map_or(0.5, |p| p.greed);
        let coins = self.comp::<Wallet>(id).map_or(0, |w| w.coins) as f32;
        if let Some(n) = self.comp_mut::<Needs>(id) {
            n.wealth = (coins / (days * price * (1.0 + greed))).clamp(0.0, 1.0);
        }
    }

    // -----------------------------------------------------------------------
    // Tier and role indices (kept incrementally; no per-tick scans)
    // -----------------------------------------------------------------------

    pub(crate) fn after_insert<T: Component>(&mut self, id: EntityId) {
        use std::any::TypeId;
        if TypeId::of::<T>() == TypeId::of::<Brain>() {
            self.index_brain(id);
        } else if TypeId::of::<T>() == TypeId::of::<Job>() {
            self.index_job(id);
        } else if TypeId::of::<T>() == TypeId::of::<Gang>() {
            Self::list_insert(&mut self.gang_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Corp>() {
            Self::list_insert(&mut self.corp_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Household>() {
            self.index_household(id);
        }
    }

    pub(crate) fn after_remove<T: Component>(&mut self, id: EntityId) {
        use std::any::TypeId;
        if TypeId::of::<T>() == TypeId::of::<Brain>() {
            self.unindex_brain(id);
        } else if TypeId::of::<T>() == TypeId::of::<Job>() {
            self.unindex_job(id);
        } else if TypeId::of::<T>() == TypeId::of::<Gang>() {
            self.unindex_gang(id);
        } else if TypeId::of::<T>() == TypeId::of::<Corp>() {
            self.unindex_corp(id);
        } else if TypeId::of::<T>() == TypeId::of::<Household>() {
            self.unindex_household(id);
        }
    }

    /// File the agent under its Household's Home.
    fn index_household(&mut self, id: EntityId) {
        self.unindex_household(id);
        if let Some(home) = self.comp::<Household>(id).and_then(|h| h.home) {
            Self::list_insert(self.residents.entry(home).or_default(), id);
            self.resident_home.insert(id, home);
        }
    }

    pub(crate) fn unindex_household(&mut self, id: EntityId) {
        if let Some(home) = self.resident_home.remove(&id) {
            if let Some(list) = self.residents.get_mut(&home) {
                Self::list_remove(list, id);
                if list.is_empty() {
                    self.residents.remove(&home);
                }
            }
        }
    }

    /// Move an agent into a Home (or out of any): the one write to
    /// `Household::home`, so `residents_of` stays in step.
    pub fn set_home(&mut self, id: EntityId, home: Option<EntityId>) {
        if let Some(h) = self.comp_mut::<Household>(id) {
            h.home = home;
            self.index_household(id);
        }
    }

    /// Everyone whose Household names this Home, ascending (the living:
    /// a death removes the Household).
    pub fn residents_of(&self, home: EntityId) -> &[EntityId] {
        self.residents.get(&home).map_or(&[], Vec::as_slice)
    }

    /// Drop a gang from `gang_ids` (its Gang removed, or despawned).
    pub(crate) fn unindex_gang(&mut self, id: EntityId) {
        Self::list_remove(&mut self.gang_ids, id);
    }

    /// Drop a corp from `corp_ids` (its Corp removed, or despawned).
    pub(crate) fn unindex_corp(&mut self, id: EntityId) {
        Self::list_remove(&mut self.corp_ids, id);
    }

    fn list_remove(list: &mut Vec<EntityId>, id: EntityId) {
        if let Ok(i) = list.binary_search(&id) {
            list.remove(i);
        }
    }

    fn list_insert(list: &mut Vec<EntityId>, id: EntityId) {
        if let Err(i) = list.binary_search(&id) {
            list.insert(i, id);
        }
    }

    /// File the agent under its Brain's current LOD.
    pub fn index_brain(&mut self, id: EntityId) {
        self.unindex_brain(id);
        if let Some(b) = self.comp::<Brain>(id) {
            let lod = b.lod;
            Self::list_insert(&mut self.by_tier[lod as usize], id);
            if lod == Lod::Statistical {
                Self::list_insert(&mut self.stat_slots.0[StatSlots::slot_of(id)], id);
            }
        }
    }

    pub fn unindex_brain(&mut self, id: EntityId) {
        for list in &mut self.by_tier {
            Self::list_remove(list, id);
        }
        Self::list_remove(&mut self.stat_slots.0[StatSlots::slot_of(id)], id);
    }

    /// Call after any write to `Brain::lod`.
    pub fn retier(&mut self, id: EntityId) {
        self.index_brain(id);
    }

    /// File the agent under its Job's role.
    pub fn index_job(&mut self, id: EntityId) {
        self.unindex_job(id);
        if let Some(j) = self.comp::<Job>(id) {
            let r = role_index(j.role);
            Self::list_insert(&mut self.by_role[r], id);
        }
    }

    pub fn unindex_job(&mut self, id: EntityId) {
        for list in &mut self.by_role {
            Self::list_remove(list, id);
        }
    }

    /// Agents with a Brain at this LOD, ascending.
    pub fn tier(&self, lod: Lod) -> &[EntityId] {
        &self.by_tier[lod as usize]
    }

    /// Full and Coarse agents (the ones with bodies), ascending.
    pub fn bodies(&self) -> Vec<EntityId> {
        let (a, b) = (&self.by_tier[Lod::Full as usize], &self.by_tier[Lod::Coarse as usize]);
        let mut out = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            if a[i] < b[j] {
                out.push(a[i]);
                i += 1;
            } else {
                out.push(b[j]);
                j += 1;
            }
        }
        out.extend_from_slice(&a[i..]);
        out.extend_from_slice(&b[j..]);
        out
    }

    /// Job holders of one role, ascending.
    pub fn workers(&self, role: Role) -> &[EntityId] {
        &self.by_role[role_index(role)]
    }

    /// Every guard (a Job holder with `Role::Guard`), ascending.
    pub fn guards(&self) -> &[EntityId] {
        self.workers(Role::Guard)
    }

    /// Rebuild both indices from the stores (after a load).
    pub fn rebuild_tiers_and_roles(&mut self) {
        let (tiers, roles, slots) = self.indices_from_stores();
        self.by_tier = tiers;
        self.by_role = roles;
        self.stat_slots = slots;
        self.gang_ids = self.with::<Gang>();
        self.corp_ids = self.with::<Corp>();
        (self.residents, self.resident_home) = self.residents_from_stores();
        self.guarded_homes = GuardedHomes::default();
        self.mean_price_cache = None;
    }

    #[allow(clippy::type_complexity)]
    fn residents_from_stores(&self) -> (BTreeMap<EntityId, Vec<EntityId>>, BTreeMap<EntityId, EntityId>) {
        let mut residents: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        let mut back = BTreeMap::new();
        for id in self.entities() {
            if let Some(home) = self.comp::<Household>(id).and_then(|h| h.home) {
                residents.entry(home).or_default().push(id);
                back.insert(id, home);
            }
        }
        (residents, back)
    }

    fn indices_from_stores(&self) -> ([Vec<EntityId>; 3], [Vec<EntityId>; 5], StatSlots) {
        let mut tiers: [Vec<EntityId>; 3] = Default::default();
        let mut roles: [Vec<EntityId>; 5] = Default::default();
        let mut slots = StatSlots::default();
        for id in self.entities() {
            if let Some(b) = self.comp::<Brain>(id) {
                tiers[b.lod as usize].push(id);
                if b.lod == Lod::Statistical {
                    slots.0[StatSlots::slot_of(id)].push(id);
                }
            }
            if let Some(j) = self.comp::<Job>(id) {
                roles[role_index(j.role)].push(id);
            }
        }
        (tiers, roles, slots)
    }

    /// Compare the incremental indices with a rebuild from the stores.
    pub fn check_indices(&self) -> Result<(), String> {
        let (tiers, roles, slots) = self.indices_from_stores();
        if slots != self.stat_slots {
            return Err("stat_slots out of sync with the Statistical tier".to_string());
        }
        if tiers != self.by_tier {
            return Err(format!(
                "by_tier out of sync: have {:?}, stores say {:?}",
                self.by_tier.iter().map(Vec::len).collect::<Vec<_>>(),
                tiers.iter().map(Vec::len).collect::<Vec<_>>()
            ));
        }
        if roles != self.by_role {
            return Err(format!(
                "by_role out of sync: have {:?}, stores say {:?}",
                self.by_role.iter().map(Vec::len).collect::<Vec<_>>(),
                roles.iter().map(Vec::len).collect::<Vec<_>>()
            ));
        }
        let (residents, resident_home) = self.residents_from_stores();
        if residents != self.residents || resident_home != self.resident_home {
            return Err("residents out of sync with the Household stores".to_string());
        }
        if self.gang_ids != self.with::<Gang>() {
            return Err(format!("gang_ids out of sync: {:?}", self.gang_ids));
        }
        if self.corp_ids != self.with::<Corp>() {
            return Err(format!("corp_ids out of sync: {:?}", self.corp_ids));
        }
        if let Some(idx) = self.report_index.get() {
            if *idx != Self::build_report_index(&self.crime_reports) {
                return Err("report_index out of sync with crime_reports".to_string());
            }
        }
        Ok(())
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

    /// Every building of a kind, ascending (map order), demolished included.
    pub fn buildings_of_kind(&self, kind: BuildingKind) -> &[EntityId] {
        self.buildings_by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// The building of `kind` whose door is nearest `from` by Manhattan
    /// distance; ties to the lower index; demolished ones skipped.
    pub fn nearest_of_kind(&self, kind: BuildingKind, from: TilePos) -> Option<EntityId> {
        self.buildings_of_kind(kind)
            .iter()
            .filter_map(|&b| {
                self.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (bd.door.manhattan(from), b))
            })
            .min()
            .map(|(_, b)| b)
    }

    /// The building of `kind` an agent uses: the one it is inside, else its
    /// employer if that is of `kind` (a clerk's own Market, a bartender's own
    /// Bar, as the Farm rule already did), else the nearest door to its tile
    /// (M10 D21). Stable between planning and execution because the agent
    /// stands on the same tile at both.
    pub fn local(&self, agent: EntityId, kind: BuildingKind) -> Option<EntityId> {
        let pos = self.comp::<Position>(agent)?;
        let usable = |b: EntityId| self.comp::<Building>(b).is_some_and(|bd| bd.kind == kind && !bd.demolished);
        if let Some(b) = pos.building.filter(|&b| usable(b)) {
            return Some(b);
        }
        if let Some(e) = self.comp::<Job>(agent).and_then(|j| j.employer).filter(|&e| usable(e)) {
            return Some(e);
        }
        if kind == BuildingKind::Market {
            return self.market_by_price(pos.tile);
        }
        self.nearest_of_kind(kind, pos.tile)
    }

    /// M11 D15: the Market a shopper at `from` picks, every tier alike: the
    /// least `door distance + shop_price_tiles × price` (ties lower id), so a
    /// cheaper Market draws custom from further off. Prices change only at
    /// midnight, so the choice holds from planning to execution.
    pub fn market_by_price(&self, from: TilePos) -> Option<EntityId> {
        let per_coin = self.config.corps.shop_price_tiles;
        self.buildings_of_kind(BuildingKind::Market)
            .iter()
            .filter_map(|&m| {
                let b = self.comp::<Building>(m).filter(|b| !b.demolished)?;
                Some((i64::from(b.door.manhattan(from)) + per_coin * self.price_at(m), m))
            })
            .min()
            .map(|(_, m)| m)
    }

    // -----------------------------------------------------------------------
    // M11: corps and owners
    // -----------------------------------------------------------------------

    /// Every corp, ascending by id (an index kept by the Corp hooks).
    pub fn corps(&self) -> Vec<EntityId> {
        self.corp_ids.clone()
    }

    /// Position of a corp in `corps()`: picks its colour in the app.
    pub fn corp_index(&self, corp: EntityId) -> usize {
        self.corps().iter().position(|&c| c == corp).unwrap_or(0)
    }

    /// A building's owner (`None` = the city).
    pub fn owner_of(&self, building: EntityId) -> Option<EntityId> {
        self.comp::<Building>(building).and_then(|b| b.owner)
    }

    /// The corp owning a building, if its owner is one.
    pub fn corp_of_building(&self, building: EntityId) -> Option<EntityId> {
        self.owner_of(building).filter(|&o| self.has::<Corp>(o))
    }

    /// The corp an agent works for: the one it is exec of, else the one
    /// owning its employer. A daily-or-rarer query (the exec lookup scans corps).
    pub fn corp_of_agent(&self, agent: EntityId) -> Option<EntityId> {
        self.corps()
            .into_iter()
            .find(|&c| self.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(agent)))
            .or_else(|| self.comp::<Job>(agent).and_then(|j| j.employer).and_then(|e| self.corp_of_building(e)))
    }

    /// M11 D13: where an agent collects wages: the Civic Hall for a city job
    /// (an employer the city owns, or none), else the employer building.
    pub fn wage_desk(&self, agent: EntityId) -> Option<EntityId> {
        let employer = self.comp::<Job>(agent).and_then(|j| j.employer);
        match employer {
            Some(e) if self.owner_of(e).is_some() => Some(e),
            _ => self.building_of_kind(BuildingKind::Hall),
        }
    }

    /// One Market's food price (`price_initial` if it has no `Market`).
    pub fn price_at(&self, market: EntityId) -> i64 {
        self.comp::<Market>(market).map_or(self.config.world.price_initial, |m| m.price_food)
    }

    /// The rounded mean price over every Market: the city-wide reading for
    /// wealth, stats, fines, Jail upkeep, the fence and the UI.
    pub fn mean_price(&self) -> i64 {
        let prices: Vec<i64> = self
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .filter_map(|&m| self.comp::<Market>(m))
            .map(|m| m.price_food)
            .collect();
        if prices.is_empty() {
            return self.config.world.price_initial;
        }
        let n = prices.len() as i64;
        (prices.iter().sum::<i64>() + n / 2) / n
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

    /// Every crime report, oldest first.
    pub fn crime_reports(&self) -> &[CrimeReport] {
        &self.crime_reports
    }

    /// The reports, for writing; drops the per-suspect index.
    pub fn reports_mut(&mut self) -> &mut Vec<CrimeReport> {
        self.report_index.take();
        &mut self.crime_reports
    }

    fn build_report_index(reports: &[CrimeReport]) -> ReportIndex {
        let mut m: BTreeMap<EntityId, (u32, Tick)> = BTreeMap::new();
        for r in reports {
            let e = m.entry(r.suspect).or_insert((0, 0));
            e.0 += u32::from(!r.resolved);
            e.1 = e.1.max(r.tick);
        }
        // M11 phase 2: the suspects with an open report, kept apart so the
        // guards' warrant checks do not walk 60 days of closed ones.
        let open = m.iter().filter(|(_, &(open, _))| open > 0).map(|(&s, _)| s).collect();
        ReportIndex { by_suspect: m, open }
    }

    fn report_index(&self) -> &ReportIndex {
        self.report_index.get_or_init(|| Self::build_report_index(&self.crime_reports))
    }

    /// `(open reports, latest report tick)` on `suspect`, if any report names them.
    pub fn reports_on(&self, suspect: EntityId) -> Option<(u32, Tick)> {
        self.report_index().by_suspect.get(&suspect).copied()
    }

    /// Suspects with an open report, ascending, each once.
    pub fn open_suspects(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.report_index().open.iter().copied()
    }

    /// Every gang, ascending by id (map order of their Hideouts).
    pub fn gangs(&self) -> Vec<EntityId> {
        self.gang_ids.clone()
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
        self.gang_ids
            .iter()
            .copied()
            .filter(|&g| g != gang)
            .filter_map(|g| self.comp::<Gang>(g).map(|gg| (std::cmp::Reverse(gg.territory.len()), g)))
            .min()
            .map(|(_, g)| g)
    }

    /// Position of a gang in `gangs()`: picks its colour in the app.
    pub fn gang_index(&self, gang: EntityId) -> usize {
        self.gang_ids.iter().position(|&g| g == gang).unwrap_or(0)
    }

    /// The agent's name, or `#index` for anything without an `Identity`.
    pub fn name_of(&self, id: EntityId) -> String {
        match self.comp::<Identity>(id) {
            Some(i) => i.name.clone(),
            None => match self.comp::<Building>(id) {
                Some(b) => format!("{}#{}", b.kind.label(), id.index),
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
    /// `commands, time, lod, needs, memory, think, plan, exec, ownership,
    /// economy, bind, law, social, gang, corp_brain, demography, stats`. The binder runs
    /// before the law so a cold-case report reaches the captain's daily
    /// rescoring (M10 D32); ownership's daily pass (rent, evictions,
    /// re-housing, upkeep) runs before the economy's price step (M11 D42).
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
        systems::ownership::run(self);
        systems::economy::run(self);
        systems::bind::run(self);
        systems::law::run(self);
        systems::social::run(self);
        systems::gang::run(self);
        systems::corp_brain::run(self);
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
        // M10 D30: the off-screen crimes and courtships the hourly table now
        // produces leave their memories too. Starved too: `needs::starvation`
        // drifts lawfulness once a day by checking for today's entry, and
        // without it a starving Statistical agent drifted every hour.
        if self.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
            if !matches!(
                kind,
                MemoryKind::Grief
                    | MemoryKind::Starved
                    | MemoryKind::WasRobbed
                    | MemoryKind::MetInJail
                    | MemoryKind::Fought
                    | MemoryKind::Lost
                    | MemoryKind::Won
                    | MemoryKind::Courted
                    | MemoryKind::Rejected
                    | MemoryKind::RentShort
                    | MemoryKind::Evicted
            ) {
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
        let (table, legacy) = load_stat_table(&self.config);
        self.stat_table = table;
        self.stat_table_legacy = legacy;
    }

    /// Set a `trace_flags` bit (ATE, SLEPT_AT_HOME) for today.
    pub fn mark_day(&mut self, id: EntityId, bit: u8) {
        *self.day_marks.entry(id).or_default() |= bit;
    }

    /// Remove an entity from every index and free it (emigrants, rotted or
    /// buried corpses). Edges to it are dropped.
    pub fn remove_agent(&mut self, id: EntityId) {
        if !self.is_alive(id) {
            return;
        }
        // M11 D45: an emigrant's buildings pass to their heirs.
        systems::ownership::on_owner_gone(self, id);
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
        crate::systems::bind::drop_victim_holes(self, id);
        self.reports_mut().retain(|r| r.suspect != id);
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
        self.rebuild_tiers_and_roles();
        self.holes_by_agent.clear();
        for (&hid, h) in &self.holes {
            self.holes_by_agent.entry(h.victim).or_default().push(hid);
        }
    }

    /// Fix up a save written before M8: a gang without a Hideout (the serde
    /// default) takes the Hideouts in map order, its territory keeps Homes
    /// only, and every held Home gets a full claim. A gang that already has a
    /// Hideout is left alone: its claims are live state.
    pub fn migrate_legacy(&mut self) {
        // M10: a pre-M10 ring has no event ids; number it 0..n.
        if self.next_event_id == 0 && !self.events.is_empty() {
            for (i, e) in self.events.iter_mut().enumerate() {
                e.id = i as u64;
            }
            self.next_event_id = self.events.len() as u64;
        }
        // M9: a save from before the law had a brain has no `law` store.
        let n = self.alive.len();
        if self.law.len() < n {
            self.law.resize_with(n, || None);
        }
        if self.trace.len() < n {
            self.trace.resize_with(n, || None);
        }
        if self.life.len() < n {
            self.life.resize_with(n, || None);
        }
        // M11: a save from before corps has no `corp` store (no corps, city
        // ownership, rent 0 through `RentCfg::off`).
        if self.corp.len() < n {
            self.corp.resize_with(n, || None);
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
        // Widowhood is recorded on the Death event, and `on_death` unlinks the pair.
        let spouse = self.spouse_of(id);
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
        if cause == DeathCause::Violence && systems::law::is_guard(self, id) {
            systems::law_brain::push_shock(self, LawShock::GuardKilled);
        }
        // M11 D20: a corp loses an employee to violence.
        if cause == DeathCause::Violence {
            if let Some(corp) = self.comp::<Job>(id).and_then(|j| j.employer).and_then(|e| self.corp_of_building(e)) {
                systems::ownership::push_corp_shock(self, corp, CorpShock::EmployeeKilled);
            }
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
        // `[dead, spouse?]`, or `[dead, spouse or NONE, killer]` when the
        // killer is known, so the biography names them (an attacker who
        // lost the fight has no Murder event saying who killed them).
        let actors: smallvec::SmallVec<[EntityId; 3]> = match (spouse, killer) {
            (s, Some(k)) => smallvec::smallvec![id, s.unwrap_or(EntityId::NONE), k],
            (Some(s), None) => smallvec::smallvec![id, s],
            (None, None) => smallvec::smallvec![id],
        };
        self.push_event(crate::events::EventKind::Death, &actors, format!("{name} died of {cause:?}"));
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
