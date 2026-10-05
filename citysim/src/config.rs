//! Every tunable constant, loaded once from `assets/config.toml`.
//!
//! The struct mirrors the `## Tuning defaults` table in the spec plus the
//! initial-world and building tables. Systems read from `world.config`; no
//! magic numbers live in system code.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::components::{BuildingKind, Lod, Niche, Role};

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
    /// M10 holes and the binder; absent from pre-M10 saves likewise.
    #[serde(default = "BindCfg::from_assets")]
    pub bind: BindCfg,
    /// M11 rent (docs/M11_OWNERSHIP.md § 4); absent from pre-M11 saves, which
    /// keep M9 behaviour: rent 0 (plan D40).
    #[serde(default = "RentCfg::off")]
    pub rent: RentCfg,
    /// M11 corps; absent from pre-M11 saves: no corps (plan D40).
    #[serde(default = "CorpsCfg::none")]
    pub corps: CorpsCfg,
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
    /// M11 (VISION "Brutality, enhancement and status"): the initial coin
    /// draw is multiplied by this per Home tier (0 Sump, 1 Mid, 2 Spire).
    #[serde(default = "default_coins_by_tier")]
    pub coins_by_tier: [f32; 3],
}

fn default_coins_by_tier() -> [f32; 3] {
    [1.0, 1.0, 1.0]
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
    /// M11 D17: full staff for an owned niche building (Grow tops up to it;
    /// an agent-owned Bar posts a vacancy below it). 0 = no rule.
    #[serde(default)]
    pub staff: u32,
}

impl BuildingCfg {
    /// No room, no stock: the M10 kinds that do nothing until M11.
    pub fn inert() -> BuildingCfg {
        BuildingCfg { capacity: 0, stock_cap: 0, staff: 0 }
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
    /// M11 D27: an exec's daily draw from their corp.
    #[serde(default = "default_wage_exec")]
    pub wage_exec: i64,
}

fn default_wage_exec() -> i64 {
    10
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
    /// M10 phase 5c: a guard is owed a shift on law duty for at least this
    /// share of it (or completed: five legs, a held Jail).
    #[serde(default = "default_shift_duty_share")]
    pub shift_duty_share: f32,
    pub posture_flat: PostureFlatCfg,
}

fn default_target_margin() -> usize {
    2
}

fn default_patrol_beat_homes() -> usize {
    60
}

fn default_shift_duty_share() -> f32 {
    0.5
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
    /// M10: days of `Trace` kept per adult.
    #[serde(default = "default_trace_days")]
    pub trace_days: usize,
    /// M10: multiplies the table's `p_robbed`, `p_assaulted` and `p_killed`
    /// at use (D26); the parity test pins it to 1.0.
    #[serde(default = "default_stat_violence_mult")]
    pub stat_violence_mult: f32,
}

fn default_trace_days() -> usize {
    120
}

fn default_stat_violence_mult() -> f32 {
    1.0
}

/// M10 holes and the binder (`docs/M10_SCALE.md` § 3). A candidate's weight is
/// `(1 - lawfulness)^lawfulness_power x (1 + gang_claim_mult x gang x claims) x (1 + enemy_mult x enemy)
/// x (1 + statistical_mult x statistical)`, times `other_zone_weight` off the victim's zone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BindCfg {
    /// A hole older than this binds (or goes Unknown) at the daily pass.
    pub hole_ttl_days: u64,
    /// Open holes per victim; the oldest expires to Unknown past it.
    pub max_open_per_agent: usize,
    /// Drawn first: nobody ever learns who did it (M10 D11).
    pub p_unknown: f64,
    /// Witness chance of a bound crime, times the zone's law coverage.
    pub p_witness: f32,
    pub coverage_min: f32,
    pub coverage_max: f32,
    pub gang_claim_mult: f64,
    pub enemy_mult: f64,
    pub statistical_mult: f64,
    pub other_zone_weight: f64,
    /// The exponent on `1 - lawfulness`; tuned by the parity test's actor shares.
    #[serde(default = "BindCfg::default_lawfulness_power")]
    pub lawfulness_power: f64,
}

impl BindCfg {
    fn default_lawfulness_power() -> f64 {
        2.0
    }

    /// The `[bind]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> BindCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).bind
    }
}

/// M11 rent (docs/M11_OWNERSHIP.md § 4, plan D6/D7). Rent is per Home per day
/// by tier, split equally among the Home's adult residents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RentCfg {
    /// Per Home per day, by tier (0 Sump, 1 Mid, 2 Spire); a corp's Housing
    /// `price_level` multiplies it, the city's is `levers.city_rent`.
    pub base: [i64; 3],
    pub evict_days: u8,
    /// A homeless adult moves in with coins >= this x the Home's rent.
    pub rehouse_coins_mult: i64,
    /// An evicting owner refuses the evictee this long.
    pub refuse_days: u64,
    /// Tier-0 pantries spoil this much faster.
    pub sump_spoilage_mult: f32,
    /// Rent due is paid out of a wage or the dole the moment it is collected
    /// (before it can be spent on food), not only from what is left at
    /// midnight. Phase 2 measurement: see `assets/config.toml`.
    #[serde(default)]
    pub pay_from_income: bool,
}

impl RentCfg {
    /// A save from before M11: no rent and M9's spoilage (plan D40; the
    /// plan's `off()` keeps the 1.5 Sump spoilage, which would change an old
    /// save's behaviour, so it is 1.0 here).
    pub fn off() -> RentCfg {
        RentCfg {
            base: [0, 0, 0],
            evict_days: 7,
            rehouse_coins_mult: 3,
            refuse_days: 30,
            sump_spoilage_mult: 1.0,
            pay_from_income: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FoundCostCfg {
    pub bar: i64,
    pub home: i64,
}

/// D4: daily upkeep per building kind, owner -> Treasury, non-city owners only.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpkeepCfg {
    pub farm: i64,
    pub market: i64,
    pub bar: i64,
    /// Per Block by tier (Sump, Mid, Spire); a scalar in the TOML means the
    /// same for every tier (phase 2's form).
    #[serde(deserialize_with = "per_tier")]
    pub home: [i64; 3],
    pub security_office: i64,
}

/// `home = 1` or `home = [0, 1, 2]`.
fn per_tier<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[i64; 3], D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Tiered {
        One(i64),
        Three([i64; 3]),
    }
    Ok(match Tiered::deserialize(d)? {
        Tiered::One(v) => [v; 3],
        Tiered::Three(v) => v,
    })
}

impl UpkeepCfg {
    /// A building's daily upkeep; a Block's by its tier.
    pub fn for_building(&self, kind: BuildingKind, tier: u8) -> i64 {
        match kind {
            BuildingKind::Farm => self.farm,
            BuildingKind::Market => self.market,
            BuildingKind::Bar => self.bar,
            BuildingKind::Home => self.home[usize::from(tier.min(2))],
            BuildingKind::SecurityOffice => self.security_office,
            _ => 0,
        }
    }
}

/// D28: what a building sells for (Bar and Home use `found_cost`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValueCfg {
    pub farm: i64,
    pub market: i64,
    pub security_office: i64,
}

/// D48: flat terms on each corp order's score.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorpOrderFlatCfg {
    pub grow: f32,
    pub squeeze: f32,
    pub undercut: f32,
    pub acquire: f32,
    pub secure: f32,
    pub hunker: f32,
    pub lobby: f32,
}

/// M11 corps (docs/M11_OWNERSHIP.md § 5 and the plan's additions). One row
/// per corp in the row vectors; `none()` is the corp-free city.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CorpsCfg {
    pub names: Vec<String>,
    pub niches: Vec<Vec<String>>,
    pub farms: Vec<u32>,
    pub markets: Vec<u32>,
    pub blocks: Vec<u32>,
    pub offices: Vec<u32>,
    pub treasury_initial: Vec<i64>,
    pub bar_owner_coins: i64,
    pub hoard_heat: i64,
    pub acquire_premium: f32,
    pub acquire_cooldown_days: u64,
    pub undercut_floor: f32,
    pub hysteresis: f32,
    pub shock_severity_rethink: f32,
    pub bankrupt_days: u64,
    pub incorporate_buildings: usize,
    pub found_cost: FoundCostCfg,
    pub wholesale: i64,
    pub contract_per_guard_day: i64,
    pub security_guards: u32,
    pub lobby_min_treasury: i64,
    pub monopoly_markup_cap: f32,
    pub bar_owner_count: usize,
    pub upkeep: UpkeepCfg,
    pub value: ValueCfg,
    pub shop_price_tiles: i64,
    pub grow_cooldown_days: u64,
    pub secure_per_day: usize,
    pub private_pursuit_radius: u32,
    pub found_cooldown_days: u64,
    pub residents_per_bar: u32,
    pub hoard_tilt: f32,
    pub megacorp: Vec<bool>,
    pub outside_treasury_initial: i64,
    pub order_flat: CorpOrderFlatCfg,
}

impl Default for CorpsCfg {
    fn default() -> Self {
        CorpsCfg::none()
    }
}

impl CorpsCfg {
    /// No corps and no agent Bar owners; every scalar at its documented default.
    pub fn none() -> CorpsCfg {
        CorpsCfg {
            names: Vec::new(),
            niches: Vec::new(),
            farms: Vec::new(),
            markets: Vec::new(),
            blocks: Vec::new(),
            offices: Vec::new(),
            treasury_initial: Vec::new(),
            bar_owner_coins: 200,
            hoard_heat: 5000,
            acquire_premium: 1.2,
            acquire_cooldown_days: 10,
            undercut_floor: 0.6,
            hysteresis: 0.10,
            shock_severity_rethink: 0.5,
            bankrupt_days: 14,
            incorporate_buildings: 2,
            found_cost: FoundCostCfg { bar: 300, home: 400 },
            wholesale: 2,
            contract_per_guard_day: 10,
            security_guards: 6,
            lobby_min_treasury: 400,
            monopoly_markup_cap: 2.0,
            bar_owner_count: 0,
            upkeep: UpkeepCfg { farm: 120, market: 600, bar: 15, home: [3; 3], security_office: 60 },
            value: ValueCfg { farm: 1000, market: 1000, security_office: 500 },
            shop_price_tiles: 12,
            grow_cooldown_days: 7,
            secure_per_day: 2,
            private_pursuit_radius: 16,
            found_cooldown_days: 10,
            residents_per_bar: 300,
            hoard_tilt: 0.1,
            megacorp: Vec::new(),
            outside_treasury_initial: 100_000,
            order_flat: CorpOrderFlatCfg {
                grow: 0.0,
                squeeze: 0.0,
                undercut: 0.0,
                acquire: 0.0,
                secure: 0.0,
                hunker: 0.15,
                lobby: 0.0,
            },
        }
    }

    /// The number of corp rows; panics when the row vectors disagree.
    pub fn row_count(&self) -> usize {
        let n = self.names.len();
        let lens = [
            ("niches", self.niches.len()),
            ("farms", self.farms.len()),
            ("markets", self.markets.len()),
            ("blocks", self.blocks.len()),
            ("offices", self.offices.len()),
            ("treasury_initial", self.treasury_initial.len()),
        ];
        for (name, len) in lens {
            assert!(len == n, "[corps] {name} has {len} rows, names has {n}");
        }
        n
    }

    /// Row `i`'s niches; panics on an unknown name.
    pub fn niches_of(&self, i: usize) -> std::collections::BTreeSet<Niche> {
        self.niches[i]
            .iter()
            .map(|s| Niche::parse(s).unwrap_or_else(|| panic!("[corps] row {i}: unknown niche {s:?}")))
            .collect()
    }
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
        // M10 raised these for the 256 x 192 map and two 60-member gangs.
        self.gangs.recruit_on_promise = 5;
        self.gangs.raid_gather_hours = 3;
        self.economy.restock_floor = 400;
        self.economy.restock_batch = 200;
        self.lod.max_coarse = 100;
        // Not a D23 key: the v1 city had no pursuit limit.
        self.law.pursuit_radius = 512;
        // M11 D9: a city-owned v1 city, no rent, no corps, equal wallets.
        self.rent.base = [0, 0, 0];
        self.corps = CorpsCfg::none();
        self.world.coins_by_tier = [1.0, 1.0, 1.0];
        self
    }

    /// The city `calibrate` measures and the LOD parity test replays: `n`
    /// residents (`scaled_to`), gangless (the table never modelled gang
    /// actions, D26) and a full Warehouse (M10 phase 5b). Full farmers work
    /// about half the shift hours a Statistical farmer is credited with, so
    /// an all-Full city runs a food deficit the real, mostly Statistical
    /// city never sees: at 500 residents it emptied its Warehouse by day 24,
    /// the price reached 16, and the table learned famine theft as everyday
    /// theft.
    pub fn calibration_city(self, n: u32) -> Config {
        let mut c = self.scaled_to(n);
        c.gangs.max_members = 0;
        c.world.warehouse_initial = c.buildings.warehouse.stock_cap;
        // M11: the calibration city stays city-owned and rent-free with equal
        // wallets, so the table measures behaviour, not one seed's landlords.
        c.rent.base = [0, 0, 0];
        c.corps = CorpsCfg::none();
        c.world.coins_by_tier = [1.0, 1.0, 1.0];
        c
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
        // M11: corp treasuries, Block holdings and the Bar owners' purse
        // scale; Farms, Markets and Offices do not (every one exists at any n).
        let c = &mut self.corps;
        for t in &mut c.treasury_initial {
            *t = (*t as f64 * f).round() as i64;
        }
        for b in &mut c.blocks {
            *b = (f64::from(*b) * f).round() as u32;
        }
        c.bar_owner_coins = (c.bar_owner_coins as f64 * f).round() as i64;
        self
    }
}
