//! Every tunable constant, loaded once from `assets/config.toml`.
//!
//! The struct mirrors the `## Tuning defaults` table in the spec plus the
//! initial-world and building tables. Systems read from `world.config`; no
//! magic numbers live in system code.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::components::{BuildingKind, Lod, Role};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// Directory that holds `map.txt`, `names.txt`, `stat_table.toml`.
    /// Not saved: a loaded world re-resolves it on this machine.
    #[serde(skip, default = "Config::assets_dir_or_empty")]
    pub assets_dir: PathBuf,
    pub world: WorldCfg,
    pub buildings: BuildingsCfg,
    pub needs: NeedsCfg,
    pub economy: EconomyCfg,
    pub crime: CrimeCfg,
    pub social: SocialCfg,
    /// Absent from pre-M8 saves: read from the assets on this machine instead.
    #[serde(default = "GangsCfg::from_assets")]
    pub gangs: GangsCfg,
    /// M9; absent from pre-M9 saves likewise.
    #[serde(default = "LawCfg::from_assets")]
    pub law: LawCfg,
    pub demography: DemographyCfg,
    pub brain: BrainCfg,
    pub exec: ExecCfg,
    pub lod: LodCfg,
    pub levers: LeversCfg,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldCfg {
    /// The map file, relative to the assets directory (an absolute path
    /// replaces it). M10: the 256 x 192 v2 map; `map_v1.txt` is the old one.
    #[serde(default = "default_map")]
    pub map: String,
    pub population: u32,
    pub residents_per_home: u32,
    pub spouse_p: f64,
    pub age_min_years: f32,
    pub age_max_years: f32,
    pub initial_coins_min: i64,
    pub initial_coins_max: i64,
    pub initial_food_min: u32,
    pub initial_food_max: u32,
    pub skill_min: f32,
    pub skill_max: f32,
    pub home_pantry_initial: u32,
    pub market_initial: u32,
    pub warehouse_initial: u32,
    pub treasury_initial: i64,
    pub price_initial: i64,
    /// `tick_of_day` ranges `[start, end)` for the default shift.
    pub shift_day: Vec<(u16, u16)>,
    /// Night shift for guards with an even `EntityId.index`.
    pub shift_night: Vec<(u16, u16)>,
    pub jobs: JobsCfg,
    pub needs_initial: NeedsInitialCfg,
}

fn default_map() -> String {
    "map.txt".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobsCfg {
    pub farmer: u32,
    pub guard: u32,
    pub clerk: u32,
    pub bartender: u32,
    pub gravedigger: u32,
}

impl JobsCfg {
    pub fn count(&self, role: Role) -> u32 {
        match role {
            Role::Farmer => self.farmer,
            Role::Guard => self.guard,
            Role::Clerk => self.clerk,
            Role::Bartender => self.bartender,
            Role::Gravedigger => self.gravedigger,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeedsInitialCfg {
    pub hunger: f32,
    pub energy: f32,
    pub safety: f32,
    pub belonging: f32,
    pub intimacy: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingCfg {
    pub capacity: u8,
    pub stock_cap: u32,
}

impl BuildingCfg {
    /// No room, no stock: the M10 kinds that do nothing until M11.
    pub fn inert() -> BuildingCfg {
        BuildingCfg { capacity: 0, stock_cap: 0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingsCfg {
    pub home: BuildingCfg,
    pub farm: BuildingCfg,
    pub market: BuildingCfg,
    pub bar: BuildingCfg,
    pub jail: BuildingCfg,
    pub cemetery: BuildingCfg,
    pub hall: BuildingCfg,
    pub hideout: BuildingCfg,
    pub warehouse: BuildingCfg,
    /// M10, inert until M11.
    #[serde(default = "BuildingCfg::inert")]
    pub security_office: BuildingCfg,
    /// M10, inert until M11.
    #[serde(default = "BuildingCfg::inert")]
    pub lot: BuildingCfg,
}

impl BuildingsCfg {
    pub fn for_kind(&self, kind: BuildingKind) -> &BuildingCfg {
        match kind {
            BuildingKind::Home => &self.home,
            BuildingKind::Farm => &self.farm,
            BuildingKind::Market => &self.market,
            BuildingKind::Bar => &self.bar,
            BuildingKind::Jail => &self.jail,
            BuildingKind::Cemetery => &self.cemetery,
            BuildingKind::Hall => &self.hall,
            BuildingKind::Hideout => &self.hideout,
            BuildingKind::Warehouse => &self.warehouse,
            BuildingKind::SecurityOffice => &self.security_office,
            BuildingKind::Lot => &self.lot,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeedsCfg {
    pub hunger_decay_per_tick: f32,
    pub energy_decay_per_tick: f32,
    pub safety_recover_per_tick: f32,
    pub belonging_decay_per_tick: f32,
    pub intimacy_decay_per_tick: f32,
    pub wealth_days_secure: f32,
    pub food_satisfy: f32,
    pub sleep_ticks_full: u32,
    pub chat_belonging: f32,
    pub starvation_grace_ticks: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EconomyCfg {
    pub price_base: f64,
    pub price_ref_stock: f64,
    pub price_min_stock: u32,
    pub price_cap: i64,
    pub wage_farmer: i64,
    pub wage_guard: i64,
    pub wage_clerk: i64,
    pub wage_bartender: i64,
    pub wage_gravedigger: i64,
    pub farm_yield_base: f32,
    pub farm_skill_floor: f32,
    pub farm_skill_slope: f32,
    /// Spring, Summer, Autumn, Winter.
    pub season_mult: [f32; 4],
    /// Spring, Summer, Autumn, Winter.
    pub energy_decay_mult: [f32; 4],
    pub spoilage_pantry: f32,
    pub spoilage_market: f32,
    pub haul_batch: u32,
    pub haul_min_stock: u32,
    /// Daily: while the Market holds less than this, clerks restock it from the
    /// Warehouse, at most `restock_batch` a day. 0 disables (spec v1 behaviour).
    #[serde(default)]
    pub restock_floor: u32,
    #[serde(default)]
    pub restock_batch: u32,
    pub build_home_cost: i64,
}

impl EconomyCfg {
    pub fn wage(&self, role: Role) -> i64 {
        match role {
            Role::Farmer => self.wage_farmer,
            Role::Guard => self.wage_guard,
            Role::Clerk => self.wage_clerk,
            Role::Bartender => self.wage_bartender,
            Role::Gravedigger => self.wage_gravedigger,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrimeCfg {
    pub steal_base_cost: f32,
    pub steal_lawfulness_factor: f32,
    pub steal_guard_penalty: f32,
    pub steal_starving_bonus: f32,
    pub steal_dark_bonus: f32,
    pub beg_base_cost: f32,
    pub beg_pride_factor: f32,
    pub forage_base_cost: f32,
    pub witness_base: f32,
    pub witness_stealth_factor: f32,
    pub witness_guard_bonus: f32,
    pub sight: u32,
    pub sight_day_crime: u32,
    pub sight_night_crime: u32,
    /// Theft, Extortion, Assault, Murder.
    pub sentence_days: [u32; 4],
    pub fight_death_p: f64,
    pub stat_theft_caught_p: f64,
    /// Unresolved warrants expire after this many days.
    pub warrant_expiry_days: u64,
    /// A sighting locates a suspect for this many ticks.
    pub suspect_seen_ticks: u64,
    pub patrol_legs_per_shift: u8,
    /// A Theft arrest at a full Jail becomes a fine of this many × price_food.
    pub fine_mult: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocialCfg {
    pub edge_create_ticks: u32,
    pub affinity_per_hour: f32,
    pub friend_threshold: f32,
    pub rival_threshold: f32,
    pub enemy_threshold: f32,
    pub propose_affinity: f32,
    pub propose_trust: f32,
    pub gang_stipend: i64,
    pub extort_amount: i64,
    pub join_gang_affinity: f32,
    pub join_gang_desperation_hunger: f32,
    pub join_gang_desperation_lawfulness: f32,
}

/// Several gangs and their faction brain (M8, `docs/M8_FACTIONS.md`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GangsCfg {
    /// One gang per Hideout, in map file order; a missing name is `Gang N`.
    pub names: Vec<String>,
    /// Per gang; the last entry repeats.
    pub treasury_initial: Vec<i64>,
    /// GangWork's order consideration: `Linear{order_weight, 1 - order_weight}` on loyalty while following.
    pub order_weight: f32,
    /// Below this loyalty a member ignores the order.
    pub freelance_loyalty: f32,
    /// A gang smaller than this recruits on promise; past it the treasury must fund the stipend.
    pub recruit_on_promise: usize,
    /// No gang recruits past this headcount.
    pub max_members: usize,
    pub heat_days: u64,
    /// A new order must beat the current one by this at the daily rescoring.
    pub hysteresis: f32,
    /// own / rival headcount needed to Contest / Raid (Logistic mids).
    pub contest_min_ratio: f32,
    pub raid_min_ratio: f32,
    /// Rival treasury worth raiding.
    pub raid_min_prize: i64,
    pub raid_cooldown_days: u64,
    /// Treasury fraction taken on a won raid; a sack takes everything.
    pub raid_prize_frac: f32,
    /// Departure at this hour of the day the order is set (or the next day if under two hours away).
    pub raid_muster_hour: u16,
    /// The Raid goal opens this many hours before `raid_at`.
    pub raid_gather_hours: u16,
    /// Raiders within this many tiles of the rival door join the brawl.
    pub raid_gather_radius: u32,
    pub retaliate_days: u64,
    pub sacked_days: u64,
    /// Pending shock severities that force an immediate rescoring.
    pub shock_severity_rethink: f32,
    /// Members who sleep at the Hideout each night (at most half the gang), so a raid meets someone.
    #[serde(default = "GangsCfg::default_night_watch")]
    pub night_watch: usize,
    /// M9 breakouts: their own cooldown, the fit headcount needed, and the
    /// convicts freed per breach.
    #[serde(default = "GangsCfg::default_breakout_cooldown_days")]
    pub breakout_cooldown_days: u64,
    #[serde(default = "GangsCfg::default_breakout_min_members")]
    pub breakout_min_members: usize,
    #[serde(default = "GangsCfg::default_breakout_max_freed")]
    pub breakout_max_freed: usize,
    pub order_flat: OrderFlatCfg,
}

impl GangsCfg {
    fn default_night_watch() -> usize {
        4
    }
    fn default_breakout_cooldown_days() -> u64 {
        15
    }
    fn default_breakout_min_members() -> usize {
        2
    }
    fn default_breakout_max_freed() -> usize {
        3
    }

    /// The `[gangs]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> GangsCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).gangs
    }
}

/// Flat terms added to each order's product of considerations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderFlatCfg {
    pub expand: f32,
    pub contest: f32,
    pub raid: f32,
    pub retaliate: f32,
    pub lielow: f32,
    #[serde(default = "OrderFlatCfg::default_breakout")]
    pub breakout: f32,
}

impl OrderFlatCfg {
    fn default_breakout() -> f32 {
        0.28
    }
}

/// The law as a faction (M9, `docs/M9_LAW.md`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LawCfg {
    /// Reports against gang members counted for pressure.
    pub window_days: u64,
    /// Reports in the window that read as full pressure.
    pub crackdown_reports: u32,
    /// A jailbreak holds the Jail this long.
    pub garrison_days: u64,
    pub hysteresis: f32,
    pub shock_severity_rethink: f32,
    /// Crackdown and Garrison need this many guards on the payroll.
    pub min_guards: usize,
    /// Captain lawfulness at or above this refuses bribes.
    pub incorruptible: f32,
    /// A bribe costs `bribe_base + bribe_per_guard x guards`.
    pub bribe_base: i64,
    pub bribe_per_guard: i64,
    /// A bribe (taken or refused) holds for this long.
    pub bribe_days: u64,
    /// The gang pays when its bribe score reaches this.
    pub bribe_threshold: f32,
    /// A Crackdown keeps its target unless a challenger leads it by this many reports.
    #[serde(default = "default_target_margin")]
    pub target_margin: usize,
    /// M10: a patrol loop draws its Homes from this many nearest its Market
    /// (60: the whole v1 city).
    #[serde(default = "default_patrol_beat_homes")]
    pub patrol_beat_homes: usize,
    /// M10: a guard chases only warrants last seen within this many tiles
    /// (Manhattan). The default reaches across any map.
    #[serde(default = "default_pursuit_radius")]
    pub pursuit_radius: u32,
    pub posture_flat: PostureFlatCfg,
}

fn default_target_margin() -> usize {
    2
}

fn default_patrol_beat_homes() -> usize {
    60
}

fn default_pursuit_radius() -> u32 {
    512
}

impl LawCfg {
    /// The `[law]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> LawCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).law
    }
}

/// Flat terms added to each posture's product of considerations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostureFlatCfg {
    pub patrol: f32,
    pub crackdown: f32,
    pub garrison: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DemographyCfg {
    pub birth_p_per_day: f64,
    pub old_age_days: u32,
    pub old_age_p_per_day: f64,
    pub emigrate_mood: f32,
    pub emigrate_days: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrainCfg {
    pub memory_cap: usize,
    pub memory_half_life_days: f32,
    pub goal_hysteresis: f32,
    pub goal_cooldown_ticks: u64,
    pub think_interval_ticks: u64,
    pub plan_budget_per_tick: usize,
    pub plan_max_expansions: usize,
    pub plan_max_len: usize,
    pub plan_timeout_ticks: u64,
    /// A search past this many expansions in one tick resumes next tick.
    pub plan_expansion_budget_per_tick: usize,
    pub path_budget_per_tick: usize,
    pub mood_w_need: f32,
    pub mood_w_memory: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecCfg {
    /// One tile per this many ticks for Full agents.
    pub move_ticks_full: u64,
    pub interrupt_check_ticks: u64,
    pub door_capacity_per_tick: u8,
    pub door_queue_max_ticks: u64,
    pub reservation_ttl: u64,
    pub astar_max_expansions: usize,
    /// Calibration only: every walk is a timed arrival (manhattan x 2 ticks).
    #[serde(default)]
    pub straight_line_paths: bool,
    /// Flow fields kept (LRU); each is one byte per tile. Eviction never
    /// changes results: a field is a pure function of the map and its door.
    #[serde(default = "default_flow_cache")]
    pub flow_field_cache: usize,
}

fn default_flow_cache() -> usize {
    512
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LodCfg {
    pub max_full: usize,
    pub max_coarse: usize,
    /// Set by the CLI's `--force-lod`; never in the file.
    #[serde(default)]
    pub force: Option<Lod>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeversCfg {
    pub tax_rate: f32,
    pub sentence_mult: f32,
    pub guard_count: u8,
    pub immigration_per_week: u8,
    pub dole_per_day: u8,
}

impl Config {
    /// Locate the assets directory and parse `config.toml`.
    ///
    /// Looks at `$CITYSIM_ASSETS`, then `assets/` in the current directory
    /// and up to three parents (so `cargo test` inside a crate finds the
    /// workspace's assets). Panics if nothing is found: a missing config is a
    /// broken checkout.
    pub fn load() -> Config {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir)
    }

    pub fn load_from(assets_dir: &Path) -> Config {
        let path = assets_dir.join("config.toml");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let mut cfg: Config = toml::from_str(&text).unwrap_or_else(|e| panic!("bad config {}: {e}", path.display()));
        cfg.assets_dir = assets_dir.to_path_buf();
        cfg
    }

    fn assets_dir_or_empty() -> PathBuf {
        Config::find_assets_dir().unwrap_or_default()
    }

    pub fn find_assets_dir() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("CITYSIM_ASSETS") {
            let p = PathBuf::from(dir);
            if p.join("config.toml").is_file() {
                return Some(p);
            }
        }
        let mut here = std::env::current_dir().ok()?;
        for _ in 0..4 {
            let candidate = here.join("assets");
            if candidate.join("config.toml").is_file() {
                return Some(candidate);
            }
            if !here.pop() {
                break;
            }
        }
        None
    }

    pub fn asset(&self, name: &str) -> PathBuf {
        self.assets_dir.join(name)
    }

    /// The v1 city (96 x 64, 300 residents) on top of the current tuning:
    /// only the scale keys are put back. Unit tests that hand-build small
    /// worlds use it; nothing else may load `map_v1.txt` (M10 D23).
    pub fn v1_profile(mut self) -> Config {
        self.world.map = "map_v1.txt".to_string();
        self.world.population = 300;
        self.world.jobs = JobsCfg { farmer: 24, guard: 10, clerk: 3, bartender: 2, gravedigger: 1 };
        self.world.market_initial = 600;
        self.world.warehouse_initial = 1500;
        self.world.treasury_initial = 5000;
        self.buildings.jail.capacity = 16;
        self.buildings.farm.capacity = 12;
        self.buildings.hideout.capacity = 12;
        self.buildings.warehouse.stock_cap = 3000;
        self.levers.guard_count = 10;
        self.levers.immigration_per_week = 2;
        self.gangs.max_members = 20;
        self.economy.restock_floor = 400;
        self.economy.restock_batch = 200;
        self.lod.max_coarse = 100;
        // Not a D23 key: the v1 city had no pursuit limit.
        self.law.pursuit_radius = 512;
        self
    }

    /// The same city at `n` residents: jobs, opening stocks, the Treasury, the
    /// guard and immigration levers and the gang cap scale by `n / population`
    /// (M10 D25; `calibrate` and the parity test run 500 on the 2,000 map).
    /// The per-Market restock floor and batch and `price_ref_stock` scale too,
    /// so a scaled Market's smaller shelf prices like the full city's.
    pub fn scaled_to(mut self, n: u32) -> Config {
        let from = self.world.population.max(1);
        let f = f64::from(n) / f64::from(from);
        let scale = |v: u32| (f64::from(v) * f).round() as u32;
        let job = |v: u32| if v == 0 { 0 } else { scale(v).max(1) };
        let j = &mut self.world.jobs;
        *j = JobsCfg {
            farmer: job(j.farmer),
            guard: job(j.guard),
            clerk: job(j.clerk),
            bartender: job(j.bartender),
            gravedigger: job(j.gravedigger),
        };
        self.world.population = n;
        self.world.market_initial = scale(self.world.market_initial);
        self.world.warehouse_initial = scale(self.world.warehouse_initial);
        self.world.treasury_initial = (self.world.treasury_initial as f64 * f).round() as i64;
        self.levers.guard_count = (f64::from(self.levers.guard_count) * f).round().clamp(0.0, 255.0) as u8;
        self.levers.immigration_per_week =
            (f64::from(self.levers.immigration_per_week) * f).round().clamp(0.0, 255.0) as u8;
        self.gangs.max_members = (self.gangs.max_members as f64 * f).round() as usize;
        self.economy.restock_floor = scale(self.economy.restock_floor);
        self.economy.restock_batch = scale(self.economy.restock_batch);
        self.economy.price_ref_stock *= f;
        self
    }
}
