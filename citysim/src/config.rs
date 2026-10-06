//! Every tunable constant, loaded once from `assets/config.toml`.
//!
//! The struct mirrors the `## Tuning defaults` table in the spec plus the
//! initial-world and building tables. Systems read from `world.config`; no
//! magic numbers live in system code.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::components::{AssetClass, AssetKind, BuildingKind, Lod, Niche, Role};

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
    /// M11 classes (§ 7); absent from pre-M11 saves: no coupling, no Dreg
    /// emigration (plan D40).
    #[serde(default = "ClassesCfg::off")]
    pub classes: ClassesCfg,
    /// M12 districts; absent from pre-M12 saves: the assets' cuts (a pure
    /// partition, behaviour-neutral alone; plan D46).
    #[serde(default = "DistrictsCfg::from_assets")]
    pub districts: DistrictsCfg,
    /// M12 phase 3 litter (§ 3); absent from pre-M12 saves: off (plan D46).
    #[serde(default = "LitterCfg::off")]
    pub litter: LitterCfg,
    /// M12 phase 3 the street (§ 4): Hotels, derelicts, squats; absent from
    /// pre-M12 saves: off (plan D46).
    #[serde(default = "StreetCfg::off")]
    pub street: StreetCfg,
    /// M12 phase 4 riots and crossfire (§ 6); absent from pre-M12 saves: off (plan D46).
    #[serde(default = "RiotsCfg::off")]
    pub riots: RiotsCfg,
    /// M13 assets (§ 1); absent from pre-M13 saves: off (plan D50).
    #[serde(default = "AssetsCfg::off")]
    pub assets: AssetsCfg,
    /// M13 chrome (§ 3); absent from pre-M13 saves: the spec's values, read
    /// by nothing while `[assets]` is off (plan D50).
    #[serde(default = "ChromeCfg::off")]
    pub chrome: ChromeCfg,
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
    /// M12 D23: hired by `levers.sanitation_count`, not seeded.
    #[serde(default)]
    pub sanitation: u32,
}

impl JobsCfg {
    pub fn count(&self, role: Role) -> u32 {
        match role {
            Role::Farmer => self.farmer,
            Role::Guard => self.guard,
            Role::Clerk => self.clerk,
            Role::Bartender => self.bartender,
            Role::Gravedigger => self.gravedigger,
            Role::Sanitation => self.sanitation,
            // M13 D16: hired by the Clinics and Garages that are built, never seeded.
            Role::Ripperdoc | Role::Mechanic => 0,
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

    /// M12 D20: a Hotel holds its guests (capacity is capped by `[street]
    /// hotel_beds` and the interior when built).
    pub fn hotel() -> BuildingCfg {
        BuildingCfg { capacity: 12, stock_cap: 0, staff: 0 }
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
    /// M12 D20: a Capsule Hotel (beds are `[street] hotel_beds`).
    #[serde(default = "BuildingCfg::hotel")]
    pub hotel: BuildingCfg,
    /// M13 D16: the Ripperdoc.
    #[serde(default = "BuildingCfg::inert")]
    pub clinic: BuildingCfg,
    /// M13 D16.
    #[serde(default = "BuildingCfg::inert")]
    pub garage: BuildingCfg,
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
            BuildingKind::Hotel => &self.hotel,
            BuildingKind::Clinic => &self.clinic,
            BuildingKind::Garage => &self.garage,
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
    /// M12 D23: a city sweeper's wage.
    #[serde(default = "default_wage_sanitation")]
    pub wage_sanitation: i64,
    /// M13 D16: a Clinic's staff.
    #[serde(default = "default_wage_ripperdoc")]
    pub wage_ripperdoc: i64,
    /// M13 D16: a Garage's staff.
    #[serde(default = "default_wage_mechanic")]
    pub wage_mechanic: i64,
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
    /// M11 phase 5: a corp-owned Market's price carries tenths (`base x
    /// level`): each shopper pays a whole coin, rounded up with probability
    /// equal to the fraction from a hash of (agent, day, Market), so a 0.9
    /// Undercut on price 3 is 2.7 on average. Identical at level 1.0; off
    /// (`false`) is the integer `round(base x level)` of phase 3.
    #[serde(default)]
    pub price_tenths: bool,
}

fn default_wage_exec() -> i64 {
    10
}

fn default_wage_sanitation() -> i64 {
    5
}

fn default_wage_ripperdoc() -> i64 {
    8
}

fn default_wage_mechanic() -> i64 {
    7
}

impl EconomyCfg {
    pub fn wage(&self, role: Role) -> i64 {
        match role {
            Role::Farmer => self.wage_farmer,
            Role::Guard => self.wage_guard,
            Role::Clerk => self.wage_clerk,
            Role::Bartender => self.wage_bartender,
            Role::Gravedigger => self.wage_gravedigger,
            Role::Sanitation => self.wage_sanitation,
            Role::Ripperdoc => self.wage_ripperdoc,
            Role::Mechanic => self.wage_mechanic,
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
    /// M12 D37: a raid musters at the gang's held Home or squat nearest the
    /// target door when one lies within this many tiles; 0 = the Hideout (M8).
    #[serde(default)]
    pub muster_near_tiles: u32,
    /// M12 D40: a gang empty this many days loses its claims (0 = only the 30-day disband).
    #[serde(default)]
    pub empty_claims_days: u64,
    /// M12 D40: an emptied gang re-forms only after this many days (0 = at once) ...
    #[serde(default)]
    pub reform_days: u64,
    /// ... and only while its Hideout's district coverage is below this.
    #[serde(default = "GangsCfg::default_reform_max_coverage")]
    pub reform_max_coverage: f32,
    /// M12 D36: splits after a decapitation. `split_base` 0 = never (M11).
    #[serde(default = "GangsCfg::default_split_strength_ratio")]
    pub split_strength_ratio: f32,
    #[serde(default = "GangsCfg::default_split_loyalty")]
    pub split_loyalty: f32,
    #[serde(default)]
    pub split_base: f32,
    #[serde(default = "GangsCfg::default_max_gangs")]
    pub max_gangs: usize,
    #[serde(default)]
    pub splinter_names: Vec<String>,
    /// M12 D39: a won raid on a corp building takes `corp_raid_frac` of the
    /// corp treasury, capped at `corp_raid_cap`; a cap of 0 = no corp raids (M11).
    #[serde(default)]
    pub corp_raid_frac: f32,
    #[serde(default)]
    pub corp_raid_cap: i64,
    /// M12 fix pass: a won corp raid takes this share of the building's food
    /// stock (the coins stay `corp_raid_frac`, capped at `corp_raid_cap`).
    #[serde(default = "GangsCfg::default_corp_raid_loot_frac")]
    pub corp_raid_loot_frac: f32,
    /// M12 fix pass: the private guards on a corp building's side (its
    /// owner's Security Office or its contractor's) posted at the door when a
    /// raid arrives: up to this many on shift, nearest first.
    #[serde(default = "GangsCfg::default_corp_raid_posted")]
    pub corp_raid_posted: usize,
    /// M12 fix pass: a corp raid's crew breaks once this share of it has
    /// lost a pairing (rounded up, at least one); 0 = to the last raider.
    #[serde(default)]
    pub corp_raid_break: f32,
    /// M12 phase 4: Raid and Retaliate need this many fit members (a crew
    /// for the door); 0 = any (M11).
    #[serde(default)]
    pub raid_min_members: usize,
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
    fn default_reform_max_coverage() -> f32 {
        f32::MAX
    }
    fn default_split_strength_ratio() -> f32 {
        0.8
    }
    fn default_split_loyalty() -> f32 {
        0.6
    }
    fn default_max_gangs() -> usize {
        4
    }
    fn default_corp_raid_loot_frac() -> f32 {
        0.25
    }
    fn default_corp_raid_posted() -> usize {
        3
    }

    /// M12 D46: the phase 4 gang keys at their M11 behaviour (no muster
    /// points, no claim loss, instant re-forming, no splits, no corp raids).
    pub fn m11_behaviour(&mut self) {
        self.muster_near_tiles = 0;
        self.empty_claims_days = 0;
        self.reform_days = 0;
        self.reform_max_coverage = f32::MAX;
        self.split_base = 0.0;
        self.corp_raid_cap = 0;
        self.raid_min_members = 0;
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
    /// M12 D38 (phase 3): the gang's Squat order.
    #[serde(default)]
    pub squat: f32,
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
    /// Reports in the window that read as full pressure (absolute; used
    /// when `crackdown_reports_per_1000` is 0).
    pub crackdown_reports: u32,
    /// M11 review: full pressure per 1,000 living residents (18: ~36 at 2,000);
    /// 0 keeps the absolute `crackdown_reports` (v1_profile, older configs).
    #[serde(default)]
    pub crackdown_reports_per_1000: f32,
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
    // --- M12 phase 2: the law in districts (docs/M12_DISTRICTS.md § 2, plan D9-D15) ---
    /// Plan D9-D11: per-district allocation, beats, stances and Vagrancy. `false`
    /// (older configs, `v1_profile`) keeps the M11 routes and nothing else runs.
    #[serde(default)]
    pub district_beats: bool,
    #[serde(default = "LawCfg::d_alloc_base")]
    pub alloc_base: f32,
    #[serde(default = "LawCfg::d_alloc_crime")]
    pub alloc_crime: f32,
    /// M12 phase 5 calibration: the crime term is `alloc_crime × min(3,
    /// rate ÷ mean) ^ alloc_crime_exp` (1.0 = the phase 2 linear term).
    #[serde(default = "LawCfg::d_alloc_crime_exp")]
    pub alloc_crime_exp: f32,
    #[serde(default = "LawCfg::d_alloc_paid")]
    pub alloc_paid: f32,
    #[serde(default = "LawCfg::d_alloc_gang_landlord")]
    pub alloc_gang_landlord: f32,
    #[serde(default = "LawCfg::d_alloc_riot")]
    pub alloc_riot: f32,
    #[serde(default = "LawCfg::d_gang_landlord_homes")]
    pub gang_landlord_homes: usize,
    #[serde(default = "LawCfg::d_max_crackdowns")]
    pub max_crackdowns: usize,
    /// Rough sleepers in a district that read as full Sweep pressure.
    #[serde(default = "LawCfg::d_sweep_full")]
    pub sweep_full: u32,
    #[serde(default = "LawCfg::d_private_fill_coverage")]
    pub private_fill_coverage: f32,
    #[serde(default = "LawCfg::d_private_fill_weight")]
    pub private_fill_weight: f32,
    /// Per rough night at coverage 1.0.
    #[serde(default = "LawCfg::d_vagrancy_base")]
    pub vagrancy_base: f32,
    #[serde(default = "LawCfg::d_sweep_mult")]
    pub sweep_mult: f32,
    #[serde(default = "LawCfg::d_curfew_mult")]
    pub curfew_mult: f32,
    #[serde(default = "LawCfg::d_vagrancy_fine")]
    pub vagrancy_fine: i64,
    /// One night.
    #[serde(default = "LawCfg::d_vagrancy_sentence_ticks")]
    pub vagrancy_sentence_ticks: u64,
    #[serde(default)]
    pub stance_flat: StanceFlatCfg,
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

    fn d_alloc_base() -> f32 {
        1.0
    }
    fn d_alloc_crime() -> f32 {
        1.0
    }
    fn d_alloc_crime_exp() -> f32 {
        1.0
    }
    fn d_alloc_paid() -> f32 {
        0.5
    }
    fn d_alloc_gang_landlord() -> f32 {
        1.5
    }
    fn d_alloc_riot() -> f32 {
        3.0
    }
    fn d_gang_landlord_homes() -> usize {
        20
    }
    fn d_max_crackdowns() -> usize {
        2
    }
    fn d_sweep_full() -> u32 {
        10
    }
    fn d_private_fill_coverage() -> f32 {
        0.7
    }
    fn d_private_fill_weight() -> f32 {
        0.5
    }
    fn d_vagrancy_base() -> f32 {
        0.04
    }
    fn d_sweep_mult() -> f32 {
        3.0
    }
    fn d_curfew_mult() -> f32 {
        2.0
    }
    fn d_vagrancy_fine() -> i64 {
        3
    }
    fn d_vagrancy_sentence_ticks() -> u64 {
        600
    }
}

/// M12 D12: flat terms added to each district stance's product.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StanceFlatCfg {
    pub patrol: f32,
    pub crackdown: f32,
    pub sweep: f32,
}

impl Default for StanceFlatCfg {
    fn default() -> Self {
        StanceFlatCfg { patrol: 0.2, crackdown: 0.0, sweep: 0.0 }
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
    /// M11 phase 5: a child whose Home pantry is empty eats from the Reserve.
    #[serde(default)]
    pub school_meals: bool,
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
    /// Experiment: the Statistical tier's policy, `"table"` (the calibrated
    /// 24-row table) or `"mlp"` (`assets/stat_mlp.toml`; see
    /// `docs/EXPERIMENT_LEARNED_STAT_POLICY.md`).
    #[serde(default = "default_stat_policy")]
    pub policy: String,
}

fn default_stat_policy() -> String {
    "table".to_string()
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
    /// M12 D3: a candidate in the hole's zone but another district. 1.0 (the
    /// serde default, `v1_profile`) is the M11 binder: the whole zone weighs 1.
    #[serde(default = "BindCfg::default_same_zone_weight")]
    pub same_zone_weight: f64,
    /// M12 D3: the witness roll reads `District.coverage` and the witness
    /// pool is the hole's district; `false` is the M11 zone binder.
    #[serde(default)]
    pub district_coverage: bool,
}

impl BindCfg {
    fn default_lawfulness_power() -> f64 {
        2.0
    }

    fn default_same_zone_weight() -> f64 {
        1.0
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
    /// M11 phase 5: an evictee sleeps rough this many days before any owner
    /// takes them in (0 = re-housed the same night).
    #[serde(default)]
    pub rehouse_wait_days: u64,
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
            rehouse_wait_days: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FoundCostCfg {
    pub bar: i64,
    pub home: i64,
    /// M12 D20: a Capsule Hotel on a Lot.
    #[serde(default = "FoundCostCfg::default_hotel")]
    pub hotel: i64,
    /// M13 D16.
    #[serde(default)]
    pub clinic: i64,
    #[serde(default)]
    pub garage: i64,
}

impl FoundCostCfg {
    fn default_hotel() -> i64 {
        250
    }
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
    /// M12 D20.
    #[serde(default = "UpkeepCfg::default_hotel")]
    pub hotel: i64,
    /// M13 D16.
    #[serde(default)]
    pub clinic: i64,
    #[serde(default)]
    pub garage: i64,
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
    fn default_hotel() -> i64 {
        10
    }

    /// A building's daily upkeep; a Block's by its tier.
    pub fn for_building(&self, kind: BuildingKind, tier: u8) -> i64 {
        match kind {
            BuildingKind::Farm => self.farm,
            BuildingKind::Market => self.market,
            BuildingKind::Bar => self.bar,
            BuildingKind::Home => self.home[usize::from(tier.min(2))],
            BuildingKind::SecurityOffice => self.security_office,
            BuildingKind::Hotel => self.hotel,
            BuildingKind::Clinic => self.clinic,
            BuildingKind::Garage => self.garage,
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
    /// M13 D16.
    #[serde(default)]
    pub clinic: i64,
    #[serde(default)]
    pub garage: i64,
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
    /// Squeeze's `price_level` ceiling without a monopoly (spec: 1.5).
    pub squeeze_cap: f32,
    pub bar_owner_count: usize,
    pub upkeep: UpkeepCfg,
    pub value: ValueCfg,
    pub shop_price_tiles: i64,
    pub grow_cooldown_days: u64,
    pub secure_per_day: usize,
    pub private_pursuit_radius: u32,
    pub found_cooldown_days: u64,
    pub residents_per_bar: u32,
    /// M12 D20: `Register`'s target count of Hotels is `population ÷ this`.
    pub residents_per_hotel: u32,
    /// M13 D16: `Register`'s target count of Clinics is `population ÷ this`.
    pub residents_per_clinic: u32,
    /// M13 D16: likewise Garages.
    pub residents_per_garage: u32,
    /// M11 phase 4: a flat term on the Found goal's score (calibration knob).
    #[serde(default)]
    pub found_flat: f32,
    pub hoard_tilt: f32,
    pub megacorp: Vec<bool>,
    pub outside_treasury_initial: i64,
    pub order_flat: CorpOrderFlatCfg,
    /// M11 phase 5 (estate rule): a bankruptcy estate skips corps whose share
    /// of the building's niche is at or above this.
    pub estate_share_cap: f32,
    /// M11 phase 5 (estate rule): an estate's building goes only to corps
    /// already in its niche (agents may still buy).
    pub estate_niche_only: bool,
    /// M11 phase 5 (estate rule): one buyer takes at most this many buildings
    /// of one estate (0 = no cap).
    pub estate_buyer_cap: usize,
    /// M11 review (estate settlement): the most of a dissolved corp's positive
    /// balance its exec takes as severance; the rest goes to the Treasury.
    pub estate_heir_cap: i64,
    /// M11 phase 5: a newly incorporated corp pays no upkeep for this many days.
    pub incorporate_grace_days: u64,
    /// Hunker never lays off Farm staff (phase 3 follow-up).
    pub hunker_spares_farms: bool,
    /// M11 phase 5: Housing Grow builds only when the corp's own Blocks are
    /// at least this full (0 = no gate).
    pub grow_min_occupancy: f32,
    /// M12 fix pass: for this many days after a raid or a riot took from one
    /// of its buildings, a corp's `losses` input reads at least
    /// `raided_losses` (it hardens: Secure, or Lobby against the gang).
    #[serde(default = "CorpsCfg::default_raided_days")]
    pub raided_days: u64,
    #[serde(default = "CorpsCfg::default_raided_losses")]
    pub raided_losses: f32,
}

impl Default for CorpsCfg {
    fn default() -> Self {
        CorpsCfg::none()
    }
}

impl CorpsCfg {
    fn default_raided_days() -> u64 {
        7
    }
    fn default_raided_losses() -> f32 {
        0.5
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
            found_cost: FoundCostCfg { bar: 300, home: 400, hotel: 250, clinic: 500, garage: 600 },
            wholesale: 2,
            contract_per_guard_day: 10,
            security_guards: 6,
            lobby_min_treasury: 400,
            monopoly_markup_cap: 2.0,
            squeeze_cap: 1.5,
            bar_owner_count: 0,
            upkeep: UpkeepCfg {
                farm: 120,
                market: 600,
                bar: 15,
                home: [3; 3],
                security_office: 60,
                hotel: 10,
                clinic: 10,
                garage: 10,
            },
            value: ValueCfg { farm: 1000, market: 1000, security_office: 500, clinic: 600, garage: 800 },
            // A pre-M11 save (and v1_profile) shops at the nearest Market.
            shop_price_tiles: 0,
            grow_cooldown_days: 7,
            secure_per_day: 2,
            private_pursuit_radius: 16,
            found_cooldown_days: 10,
            residents_per_bar: 300,
            residents_per_hotel: 800,
            residents_per_clinic: 700,
            residents_per_garage: 700,
            found_flat: 0.0,
            hoard_tilt: 0.1,
            megacorp: Vec::new(),
            outside_treasury_initial: 100_000,
            estate_share_cap: 0.5,
            estate_niche_only: true,
            estate_buyer_cap: 0,
            estate_heir_cap: 300,
            incorporate_grace_days: 0,
            hunker_spares_farms: true,
            grow_min_occupancy: 0.0,
            raided_days: 7,
            raided_losses: 0.5,
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

/// M11 classes (docs/M11_OWNERSHIP.md § 7; plan D34-D36).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassesCfg {
    /// Street unrest above this calls a strike.
    pub strike_threshold: f32,
    pub strike_cooldown_days: u64,
    /// D34: Chebyshev tiles from a Home's door that an on-shift guard watches.
    pub fear_radius: u32,
    /// Guard-hours a day near a Home that read as full fear.
    pub fear_hours_full: f32,
    /// Immigration = lever x Logistic{k, mid}(Street happiness); false = the lever.
    pub couple_immigration: bool,
    pub immigration_k: f32,
    pub immigration_mid: f32,
    pub dreg_emigrate_mood: f32,
    /// 0 = off.
    pub dreg_emigrate_days: u8,
    /// D36: an evictee this recent and this lawless is desperate for a gang.
    pub evicted_recruit_days: u64,
    pub evicted_recruit_lawfulness: f32,
    /// M12 D29: district unrest gains `rent_burden_w × burden_d` (0 = off).
    #[serde(default)]
    pub rent_burden_w: f32,
    /// M12 D29: submission gains this under a district curfew.
    #[serde(default)]
    pub curfew_fear: f32,
}

impl Default for ClassesCfg {
    fn default() -> Self {
        ClassesCfg::off()
    }
}

impl ClassesCfg {
    /// The documented defaults with the coupling and Dreg emigration off
    /// (pre-M11 saves, `v1_profile`: plan D9, D40).
    pub fn off() -> ClassesCfg {
        ClassesCfg {
            strike_threshold: 0.6,
            strike_cooldown_days: 7,
            fear_radius: 6,
            fear_hours_full: 1.0,
            couple_immigration: false,
            immigration_k: 8.0,
            immigration_mid: 0.5,
            dreg_emigrate_mood: -0.5,
            dreg_emigrate_days: 0,
            evicted_recruit_days: 14,
            evicted_recruit_lawfulness: 0.5,
            rent_burden_w: 0.0,
            curfew_fear: 0.0,
        }
    }
}

/// M12 control presence weights (plan D8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlWeights {
    pub held_home: f32,
    pub owned_home: f32,
    pub owned_other: f32,
    pub city_per_coverage: f32,
}

/// M12 districts (docs/M12_DISTRICTS.md § 1). One row per district: a name,
/// a zone letter and a half-open x range inside that zone. A tile is in the
/// first row whose zone matches its zone and whose range holds its x.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistrictsCfg {
    pub names: Vec<String>,
    pub zones: Vec<String>,
    pub x_from: Vec<u16>,
    pub x_to: Vec<u16>,
    /// Below this share of the total presence the district is Contested.
    pub control_min_share: f32,
    pub control_weights: ControlWeights,
}

impl DistrictsCfg {
    /// The `[districts]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> DistrictsCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).districts
    }

    /// One district, "City", covering the whole map (the v1 city, plan D1).
    pub fn single() -> DistrictsCfg {
        DistrictsCfg {
            names: vec!["City".to_string()],
            zones: vec!["M".to_string()],
            x_from: vec![0],
            x_to: vec![256],
            control_min_share: 0.4,
            control_weights: ControlWeights {
                held_home: 1.0,
                owned_home: 1.0,
                owned_other: 3.0,
                city_per_coverage: 1.0,
            },
        }
    }

    /// The number of rows; panics when the row vectors disagree, a zone
    /// letter is unknown or there are more than `MAX_DISTRICTS` rows.
    pub fn row_count(&self) -> usize {
        let n = self.names.len();
        for (name, len) in [("zones", self.zones.len()), ("x_from", self.x_from.len()), ("x_to", self.x_to.len())] {
            assert!(len == n, "[districts] {name} has {len} rows, names has {n}");
        }
        assert!(
            (1..=crate::components::MAX_DISTRICTS).contains(&n),
            "[districts] has {n} rows; 1..={} allowed",
            crate::components::MAX_DISTRICTS
        );
        for i in 0..n {
            assert!(self.zone_of(i).is_some(), "[districts] row {i}: unknown zone {:?}", self.zones[i]);
        }
        n
    }

    /// Row `i`'s zone, from its letter.
    pub fn zone_of(&self, i: usize) -> Option<crate::components::Zone> {
        let mut chars = self.zones.get(i)?.chars();
        let c = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        crate::components::Zone::parse(c)
    }
}

/// M12 litter (docs/M12_DISTRICTS.md § 3, plan D16-D19, D23-D24).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LitterCfg {
    /// `off()` = false: no deposits, no delay, no sanitation credit.
    pub enabled: bool,
    /// Lost by every tile in 1..=254 at midnight.
    pub decay: u8,
    /// Ticks added to a Full mover's next step per band (clean, littered, trashed, heaped).
    pub step_ticks: [u8; 4],
    /// A Coarse walk is `1 + timed_mult × district litter` longer.
    pub timed_mult: f32,
    pub sleep_penalty: f32,
    /// Mood: `− mood × district litter` in the hourly update.
    pub mood: f32,
    pub clean_per_shift: u32,
    /// Coins per 32 units an owner cleans around its doors.
    pub owner_clean_cost: i64,
    pub owner_clean_units: u32,
    pub gang_clean_per_member: u32,
    pub clean_pride: f32,
    /// The Damage hook (D19): rubble (255) blocks movement. Off in M12.
    pub rubble_blocks: bool,
    pub rubble_clean_mult: u32,
    /// M12 fix pass: every deposit in the spec's table is scaled by this
    /// (then capped at 254), so one crime leaves a visible mark.
    #[serde(default = "LitterCfg::d_deposit_mult")]
    pub deposit_mult: f32,
    /// M12 fix pass: a district's `litter` is the share of its street tiles
    /// at or above this value (32, the littered band).
    #[serde(default = "LitterCfg::d_visible")]
    pub visible: u8,
}

impl LitterCfg {
    /// No litter at all (a pre-M12 save, `v1_profile`).
    pub fn off() -> LitterCfg {
        LitterCfg {
            enabled: false,
            decay: 1,
            step_ticks: [0, 0, 1, 2],
            timed_mult: 0.5,
            sleep_penalty: 0.15,
            mood: 0.2,
            clean_per_shift: 120,
            owner_clean_cost: 1,
            owner_clean_units: 64,
            gang_clean_per_member: 8,
            clean_pride: 0.5,
            rubble_blocks: false,
            rubble_clean_mult: 4,
            deposit_mult: 1.0,
            visible: 32,
        }
    }
    fn d_deposit_mult() -> f32 {
        1.0
    }
    fn d_visible() -> u8 {
        32
    }
}

/// M12 the street (docs/M12_DISTRICTS.md § 4, plan D20-D28).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreetCfg {
    /// `off()` = false: no Hotels or derelicts seeded, no Squat goal, no nightly booking.
    pub enabled: bool,
    pub hotel_beds: u8,
    pub night_price: i64,
    /// Tiles from a Full or Coarse agent to a Hotel door it will walk to.
    pub hotel_reach: u32,
    pub seed_hotels: usize,
    pub seed_derelict_blocks: usize,
    /// An unsold estate's building goes to the City only while the Treasury holds this.
    pub city_absorb_floor: i64,
    /// A non-city Block empty this long whose owner's purse is negative is abandoned.
    pub abandon_days: u64,
    pub squat_reach: u32,
    pub squat_ban_days: u64,
    /// Phase 3 decision (plan D25 leaves "who re-lets" open): the City
    /// repairs and re-lets a derelict Block after this many days derelict,
    /// while the Treasury holds `city_absorb_floor` (0 = never).
    #[serde(default)]
    pub relet_days: u64,
}

impl StreetCfg {
    /// The street rung off (a pre-M12 save, `v1_profile`).
    pub fn off() -> StreetCfg {
        StreetCfg {
            enabled: false,
            hotel_beds: 12,
            night_price: 3,
            hotel_reach: 48,
            seed_hotels: 0,
            seed_derelict_blocks: 0,
            city_absorb_floor: 5000,
            abandon_days: 14,
            squat_reach: 40,
            squat_ban_days: 14,
            relet_days: 0,
        }
    }
}

/// M12 riots and crossfire (docs/M12_DISTRICTS.md § 6, plan D29-D35).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RiotsCfg {
    /// `off()` = false: no district riots (crossfire still reads `p_crossfire`).
    pub enabled: bool,
    pub riot_threshold: f32,
    pub riot_days: u16,
    pub riot_cooldown_days: u64,
    pub riot_min: usize,
    pub riot_max: usize,
    pub max_active_riots: usize,
    pub riot_lawfulness: f32,
    pub riot_muster_hour: u16,
    pub loot_frac: f32,
    pub riot_close_days: u64,
    pub riot_vent: f32,
    pub crush_kill_mult: f32,
    pub crossfire_radius: u32,
    pub p_crossfire: f32,
    pub p_crossfire_kill: f32,
    pub riot_stat_bystanders: usize,
    /// Plan risk 3 (LOD): a rioter ranks with the gangs (class 2) from this
    /// many hours before the muster until the riot ends; 24 or more = from
    /// the trigger at midnight.
    #[serde(default = "RiotsCfg::default_promote_hours")]
    pub riot_promote_hours: u16,
    /// M12 fix pass (throughput): only the first this many of a riot's
    /// rioters (most miserable first) rank with the gangs for a body; the
    /// rest march as they are. 0 = all of them.
    #[serde(default)]
    pub riot_promote_max: usize,
}

impl RiotsCfg {
    fn default_promote_hours() -> u16 {
        24
    }

    /// No riots and no crossfire (a pre-M12 save, `v1_profile`, the calibration city).
    pub fn off() -> RiotsCfg {
        RiotsCfg {
            enabled: false,
            riot_threshold: 0.55,
            riot_days: 3,
            riot_cooldown_days: 14,
            riot_min: 6,
            riot_max: 40,
            max_active_riots: 2,
            riot_lawfulness: 0.5,
            riot_muster_hour: 20,
            loot_frac: 0.3,
            riot_close_days: 3,
            riot_vent: 0.5,
            crush_kill_mult: 3.0,
            crossfire_radius: 3,
            p_crossfire: 0.0,
            p_crossfire_kill: 0.1,
            riot_stat_bystanders: 4,
            riot_promote_hours: 24,
            riot_promote_max: 0,
        }
    }
}

/// M13 plan 1.2: one value per `AssetKind` class (implants share a row).
/// Every field defaults, so `parts_per` may omit `pack` and `bridge`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PerKind<T: Default> {
    #[serde(default)]
    pub motorcycle: T,
    #[serde(default)]
    pub car: T,
    #[serde(default)]
    pub truck: T,
    #[serde(default)]
    pub flyer: T,
    #[serde(default)]
    pub implant: T,
    #[serde(default)]
    pub robot: T,
    #[serde(default)]
    pub pack: T,
    /// Plan D52 (M14's Virt bridge).
    #[serde(default)]
    pub bridge: T,
}

impl<T: Default> PerKind<T> {
    pub fn get(&self, kind: AssetKind) -> &T {
        match kind.class() {
            AssetClass::Motorcycle => &self.motorcycle,
            AssetClass::Car => &self.car,
            AssetClass::Truck => &self.truck,
            AssetClass::Flyer => &self.flyer,
            AssetClass::Implant => &self.implant,
            AssetClass::Robot => &self.robot,
            AssetClass::Pack => &self.pack,
            AssetClass::Bridge => &self.bridge,
        }
    }
}

/// M13 assets (docs/M13_ASSETS.md § 1, plan D1-D15, D50). Tier lists read
/// `get(kind)[tier - 1]`; a missing tier is not sold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsCfg {
    /// `off()` = false: no pass, no Kit term, no Body read; the CSV columns stay 0.
    pub enabled: bool,
    pub price: PerKind<Vec<i64>>,
    pub upkeep: PerKind<Vec<i64>>,
    pub wear: PerKind<u8>,
    pub parts_per: PerKind<u32>,
    pub parts_price: i64,
    pub import_frac: f32,
    /// Import coins one Part from a seller's stock replaces.
    pub part_credit: i64,
    pub down_frac: f32,
    pub interest: f32,
    pub term_days: u32,
    pub repo_days: u8,
    pub brick_repo_days: u8,
    pub impound_days: u8,
    pub impound_frac: f32,
    pub unpaid_wear_mult: u8,
    pub repair_below: u8,
    /// Coins per condition point.
    pub repair_price: i64,
    pub fail_shock: f32,
    /// Plan D13: 0 runs inheritance at death (M12); phase 3 sets 12.
    pub loot_window_hours: u32,
    pub rip_courage: f32,
    pub scav_strip_base: f32,
    pub carry_base: u32,
    pub carry_per_strength: f32,
    /// Carry added by a pack, per tier.
    pub pack: Vec<u32>,
    /// Plan D6: the Stims/Parts cap where a kind's `stock_cap` is lower.
    pub goods_cap: u32,
    /// Plan D14: a Clinic or Garage below this many Parts buys `parts_batch`.
    pub parts_floor: u32,
    pub parts_batch: u32,
    /// Phase 1 deviation (plan D16 grows `FOUNDABLE`): `Register` and
    /// `choose_kind` may pick a Clinic or Garage only while set. False in
    /// phase 1, so the inert kinds are founded by nobody; phase 2 sets it.
    pub found_sellers: bool,
}

impl Default for AssetsCfg {
    fn default() -> Self {
        AssetsCfg::off()
    }
}

impl AssetsCfg {
    /// Assets off (a pre-M13 save, `v1_profile`, the calibration city):
    /// every other key at its `assets/config.toml` value.
    pub fn off() -> AssetsCfg {
        AssetsCfg {
            enabled: false,
            price: PerKind {
                motorcycle: vec![300],
                car: vec![800, 1400],
                truck: vec![1500],
                flyer: vec![6000],
                implant: vec![150, 500, 1500],
                robot: vec![800, 2000, 5000],
                pack: vec![30, 80],
                bridge: vec![200, 600, 1800],
            },
            upkeep: PerKind {
                motorcycle: vec![1],
                car: vec![2, 3],
                truck: vec![4],
                flyer: vec![15],
                implant: vec![0, 1, 3],
                robot: vec![4, 8, 15],
                pack: vec![0, 0],
                bridge: vec![0, 0, 0],
            },
            wear: PerKind { motorcycle: 1, car: 1, truck: 1, flyer: 1, implant: 0, robot: 1, pack: 0, bridge: 0 },
            parts_per: PerKind {
                motorcycle: 4,
                car: 8,
                truck: 12,
                flyer: 30,
                implant: 3,
                robot: 10,
                pack: 0,
                bridge: 0,
            },
            parts_price: 15,
            import_frac: 0.6,
            part_credit: 20,
            down_frac: 0.25,
            interest: 0.2,
            term_days: 60,
            repo_days: 5,
            brick_repo_days: 10,
            impound_days: 10,
            impound_frac: 0.5,
            unpaid_wear_mult: 3,
            repair_below: 50,
            repair_price: 2,
            fail_shock: 0.1,
            loot_window_hours: 0,
            rip_courage: 0.5,
            scav_strip_base: 0.15,
            carry_base: 20,
            carry_per_strength: 10.0,
            pack: vec![10, 25],
            goods_cap: 1000,
            parts_floor: 20,
            parts_batch: 20,
            found_sellers: false,
        }
    }
}

/// M13 chrome (docs/M13_ASSETS.md § 3). Phase 1 carries the Kit-table keys
/// (plan "Kit tables"), the sanity cost and the used-sale fraction; phase 3
/// adds the rest of the spec's `[chrome]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromeCfg {
    /// Arms: `fighting + arms_fighting × tier`.
    pub arms_fighting: f32,
    /// Arms: `strength + arms_strength × tier`.
    pub arms_strength: f32,
    /// Legs: `walk_mult` per tier.
    pub legs_walk: Vec<f32>,
    /// Nerves: `reflex + nerves_reflex × tier`.
    pub nerves_reflex: f32,
    /// Eyes: `sight + eyes_sight × tier` tiles.
    pub eyes_sight: u8,
    /// Eyes: `stealth + eyes_stealth × tier`.
    pub eyes_stealth: f32,
    /// Skin: `armour = skin_armour × tier`.
    pub skin_armour: f32,
    /// Sanity load per installed implant, per tier.
    pub sanity_cost: Vec<f32>,
    /// A used asset sells at `used_frac × list × condition / 100` (plan D29).
    pub used_frac: f32,
}

impl Default for ChromeCfg {
    fn default() -> Self {
        ChromeCfg::off()
    }
}

impl ChromeCfg {
    /// The spec's values (assets off reads none of them).
    pub fn off() -> ChromeCfg {
        ChromeCfg {
            arms_fighting: 0.1,
            arms_strength: 0.25,
            legs_walk: vec![0.85, 0.75, 0.65],
            nerves_reflex: 0.15,
            eyes_sight: 2,
            eyes_stealth: 0.05,
            skin_armour: 0.15,
            sanity_cost: vec![0.08, 0.15, 0.25],
            used_frac: 0.6,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeversCfg {
    pub tax_rate: f32,
    pub sentence_mult: f32,
    pub guard_count: u8,
    pub immigration_per_week: u8,
    pub dole_per_day: u8,
    /// M12 D23: city Sanitation workers (0 in a pre-M12 save: nobody sweeps).
    #[serde(default)]
    pub sanitation_count: u8,
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
        cfg.districts.row_count();
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
        self.world.jobs = JobsCfg { farmer: 24, guard: 10, clerk: 3, bartender: 2, gravedigger: 1, sanitation: 0 };
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
        // The M11 review's per-capita pressure: the v1 city keeps its absolute 6.
        self.law.crackdown_reports_per_1000 = 0.0;
        // M11 D9: a city-owned v1 city, no rent, no corps, equal wallets.
        self.rent.base = [0, 0, 0];
        self.corps = CorpsCfg::none();
        self.world.coins_by_tier = [1.0, 1.0, 1.0];
        self.classes.couple_immigration = false;
        self.classes.dreg_emigrate_days = 0;
        // M11 phase 5 recalibrated the 2,000 city's dole, tax, school meals and
        // prices; the v1 gates and unit tests keep the v1 economy.
        self.levers.dole_per_day = 3;
        self.levers.tax_rate = 0.05;
        self.demography.school_meals = false;
        self.economy.price_tenths = false;
        self.rent.rehouse_wait_days = 0;
        // M12 D1: the v1 city is one district.
        self.districts = DistrictsCfg::single();
        // M12 D46: the M11 law and binder.
        self.law.district_beats = false;
        self.bind.same_zone_weight = 1.0;
        self.bind.district_coverage = false;
        // M12 D46: no litter, no Hotels, derelicts or squats, nobody sweeps.
        self.litter = LitterCfg::off();
        self.street = StreetCfg::off();
        self.levers.sanitation_count = 0;
        // M12 D46 (phase 4): no riots or crossfire, the M11 gangs, no district
        // unrest terms, and no hoard tilt.
        self.riots = RiotsCfg::off();
        self.gangs.m11_behaviour();
        self.classes.rent_burden_w = 0.0;
        self.classes.curfew_fear = 0.0;
        self.corps.hoard_tilt = 0.0;
        // M13 D50: no assets, so the v1, M8 and M9 unit tests run unchanged.
        self.assets = AssetsCfg::off();
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
        // The real city shops by price (D15); keep that walk, not the
        // corp-free default's nearest Market. The dole, tax and school
        // meals stay the 2,000 city's.
        let tiles = c.corps.shop_price_tiles;
        c.corps = CorpsCfg::none();
        c.corps.shop_price_tiles = tiles;
        c.world.coins_by_tier = [1.0, 1.0, 1.0];
        // M11 phase 4: no class coupling (immigration at the lever, no Dreg
        // emigration), as v1_profile.
        c.classes.couple_immigration = false;
        c.classes.dreg_emigrate_days = 0;
        // M12 D48: litter and the street on, but no seeded derelicts (the
        // table measures behaviour, not one seed's housing shortage).
        c.street.seed_derelict_blocks = 0;
        // M12 D48 (phase 4): riots off, no splits, no crossfire.
        c.riots = RiotsCfg::off();
        c.gangs.split_base = 0.0;
        // M13 D46/D50: no assets: the table and the parity test are unchanged.
        c.assets = AssetsCfg::off();
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
            sanitation: job(j.sanitation),
        };
        self.world.population = n;
        self.world.market_initial = scale(self.world.market_initial);
        self.world.warehouse_initial = scale(self.world.warehouse_initial);
        self.world.treasury_initial = (self.world.treasury_initial as f64 * f).round() as i64;
        self.levers.guard_count = (f64::from(self.levers.guard_count) * f).round().clamp(0.0, 255.0) as u8;
        self.levers.immigration_per_week =
            (f64::from(self.levers.immigration_per_week) * f).round().clamp(0.0, 255.0) as u8;
        self.levers.sanitation_count = (f64::from(self.levers.sanitation_count) * f).round().clamp(0.0, 255.0) as u8;
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
