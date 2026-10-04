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
    pub demography: DemographyCfg,
    pub brain: BrainCfg,
    pub exec: ExecCfg,
    pub lod: LodCfg,
    pub levers: LeversCfg,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldCfg {
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
    pub gang_name: String,
    pub gang_treasury_initial: i64,
    /// `tick_of_day` ranges `[start, end)` for the default shift.
    pub shift_day: Vec<(u16, u16)>,
    /// Night shift for guards with an even `EntityId.index`.
    pub shift_night: Vec<(u16, u16)>,
    pub jobs: JobsCfg,
    pub needs_initial: NeedsInitialCfg,
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
}
