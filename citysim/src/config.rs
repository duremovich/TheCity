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
    /// M13 phase 2 vehicles (§ 2); absent from pre-M13 saves: the spec's
    /// values, read by nothing while `[assets]` is off (plan D50).
    #[serde(default = "VehiclesCfg::off")]
    pub vehicles: VehiclesCfg,
    /// M13 phase 2 the Shop goal, the Garage and fleets (§ 6); absent from
    /// pre-M13 saves likewise.
    #[serde(default = "ShopCfg::off")]
    pub shop: ShopCfg,
    /// M13 phase 4 stims, addiction and the drug trade (§ 4); absent from
    /// pre-M13 saves: the spec's values, read by nothing while `[assets]`
    /// is off (plan D50).
    #[serde(default = "StimsCfg::off")]
    pub stims: StimsCfg,
    /// M13 phase 4 the security robot (§ 5); absent from pre-M13 saves likewise.
    #[serde(default = "RobotsCfg::off")]
    pub robots: RobotsCfg,
    /// M14 the Virt plane (§ 1); absent from pre-M14 saves: off (plan V44).
    /// `enabled = false` turns off relink, runs, ICE upkeep, Lab production,
    /// research upkeep and every tier cap.
    #[serde(default = "VirtCfg::off")]
    pub virt: VirtCfg,
    /// M14 decks (§ 2); read only while `[virt]` is on.
    #[serde(default = "DecksCfg::off")]
    pub decks: DecksCfg,
    /// M14 ICE and the contest (§ 3); read only while `[virt]` is on.
    #[serde(default = "IceCfg::off")]
    pub ice: IceCfg,
    /// M14 Data and Labs (§ 4); read only while `[virt]` is on.
    #[serde(default = "DataCfg::off")]
    pub data: DataCfg,
    /// M14 the tech tree (§ 5); read only while `[virt]` is on.
    #[serde(default = "TechCfg::off")]
    pub tech: TechCfg,
    /// M14 hacking goals and orders (§ 6); read only while `[virt]` is on.
    #[serde(default = "HackCfg::off")]
    pub hack: HackCfg,
    /// M14 faction databases (§ 6); read only while `[virt]` is on.
    #[serde(default = "DbCfg::off")]
    pub db: DbCfg,
    /// M15 § 1 the word (pools, exchange, hearing, kin, reputation);
    /// absent from pre-M15 saves: off (plan W44). `enabled = false` is the
    /// master switch for every M15 system.
    #[serde(default = "GossipCfg::off")]
    pub gossip: GossipCfg,
    /// M15 § 2 reputation; read only while `[gossip]` is on.
    #[serde(default = "ReputationCfg::off")]
    pub reputation: ReputationCfg,
    /// M15 § 2 appearance and taste (phase 2).
    #[serde(default = "TasteCfg::off")]
    pub taste: TasteCfg,
    /// M15 § 2 the Purist creed (phase 2).
    #[serde(default = "CreedsCfg::off")]
    pub creeds: CreedsCfg,
    /// M15 § 3 grudges (phase 3).
    #[serde(default = "GrudgesCfg::off")]
    pub grudges: GrudgesCfg,
    /// M15 § 4 the Hunt (phase 3).
    #[serde(default = "HuntCfg::off")]
    pub hunt: HuntCfg,
    /// M15 § 5 social skills (phase 2).
    #[serde(default = "SkillsCfg::off")]
    pub skills: SkillsCfg,
    /// M15 § 6 the social move (phase 2; `contradict_conf` from phase 1).
    #[serde(default = "MovesCfg::off")]
    pub moves: MovesCfg,
    /// M15 § 6 competence and poaching (phase 2).
    #[serde(default = "CompetenceCfg::off")]
    pub competence: CompetenceCfg,
    /// M15 § 7 Feeds and stories (phase 4).
    #[serde(default = "NewsCfg::off")]
    pub news: NewsCfg,
    /// Life pass L1 (docs/SHADOW_V1.md): travel, sleep, stale targets, the
    /// poor's fallback, witness and edge spam, order dwell; absent from
    /// pre-L1 saves: off.
    #[serde(default = "LifeCfg::off")]
    pub life: LifeCfg,
    /// Life pass L2 (docs/LIFE_L2.md, plan L5): the master switch; absent
    /// from pre-L2 saves: off. `enabled = false` (`--l2-off`) is the
    /// M15-closing city byte for byte.
    #[serde(default = "LivingCfg::off")]
    pub living: LivingCfg,
    /// L2 § 1 jobs (plan L5: `LivingJobsCfg`, as `JobsCfg` is `[world.jobs]`).
    #[serde(default = "LivingJobsCfg::off")]
    pub jobs: LivingJobsCfg,
    /// L2 § 1-2 leisure (phase 1 reads only the price table).
    #[serde(default = "LeisureCfg::off")]
    pub leisure: LeisureCfg,
    /// L2 § 1 the Treasury budget band.
    #[serde(default = "BudgetCfg::off")]
    pub budget: BudgetCfg,
    /// L2 § 1 the export hook (off by default).
    #[serde(default = "ExportCfg::off")]
    pub export: ExportCfg,
    /// L2 § 3 faction violence off screen (phase 4; plan L24-L28).
    #[serde(default = "FviolenceCfg::off")]
    pub fviolence: FviolenceCfg,
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
            // M14: likewise the Labs' Researchers.
            // M15 W36: and the Feeds' Reporters.
            Role::Ripperdoc | Role::Mechanic | Role::Researcher | Role::Reporter => 0,
            // L2 (plan L2): the venues' and Fabs' staff are hired, never seeded.
            Role::Host
            | Role::Attendant
            | Role::Cook
            | Role::Fighter
            | Role::Croupier
            | Role::Concierge
            | Role::Fabber => 0,
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

    /// M14 (plan V16): a Lab of four Researchers.
    pub fn lab() -> BuildingCfg {
        BuildingCfg { capacity: 8, stock_cap: 0, staff: 4 }
    }

    /// M15 W36: a Feed of one Reporter (phase 4: `assets/config.toml`).
    pub fn feed() -> BuildingCfg {
        BuildingCfg { capacity: 6, stock_cap: 0, staff: 1 }
    }

    /// L2 § 1 (spec values; `[buildings.club]` … in `assets/config.toml`).
    pub fn club() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 0, staff: 10 }
    }
    pub fn arcade() -> BuildingCfg {
        BuildingCfg { capacity: 16, stock_cap: 0, staff: 3 }
    }
    pub fn noodle_bar() -> BuildingCfg {
        BuildingCfg { capacity: 10, stock_cap: 60, staff: 3 }
    }
    pub fn fight_pit() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 0, staff: 4 }
    }
    pub fn den() -> BuildingCfg {
        BuildingCfg { capacity: 20, stock_cap: 0, staff: 4 }
    }
    pub fn lounge() -> BuildingCfg {
        BuildingCfg { capacity: 16, stock_cap: 0, staff: 6 }
    }
    pub fn fab() -> BuildingCfg {
        BuildingCfg { capacity: 14, stock_cap: 400, staff: 12 }
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
    /// M14 (plan V16).
    #[serde(default = "BuildingCfg::lab")]
    pub lab: BuildingCfg,
    /// M15 W36.
    #[serde(default = "BuildingCfg::feed")]
    pub feed: BuildingCfg,
    /// L2 § 1: the seven new kinds (none stands with L2 off).
    #[serde(default = "BuildingCfg::club")]
    pub club: BuildingCfg,
    #[serde(default = "BuildingCfg::arcade")]
    pub arcade: BuildingCfg,
    #[serde(default = "BuildingCfg::noodle_bar")]
    pub noodle_bar: BuildingCfg,
    #[serde(default = "BuildingCfg::fight_pit")]
    pub fight_pit: BuildingCfg,
    #[serde(default = "BuildingCfg::den")]
    pub den: BuildingCfg,
    #[serde(default = "BuildingCfg::lounge")]
    pub lounge: BuildingCfg,
    #[serde(default = "BuildingCfg::fab")]
    pub fab: BuildingCfg,
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
            BuildingKind::Lab => &self.lab,
            BuildingKind::Feed => &self.feed,
            BuildingKind::Club => &self.club,
            BuildingKind::Arcade => &self.arcade,
            BuildingKind::NoodleBar => &self.noodle_bar,
            BuildingKind::FightPit => &self.fight_pit,
            BuildingKind::Den => &self.den,
            BuildingKind::Lounge => &self.lounge,
            BuildingKind::Fab => &self.fab,
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
    /// L2 § 2 (plan L13): `fun`'s decay per tick at sociability 0.5 before
    /// `(0.6 + 0.4 × sociability) × class_mult` (read only with `leisure::on`).
    #[serde(default = "default_fun_decay")]
    pub fun_decay_per_tick: f32,
    /// L2 L13: mood's `fun_mood × (fun − 0.5)` bias.
    #[serde(default = "default_fun_mood")]
    pub fun_mood: f32,
    /// L2 L13: `Unwind` is satisfied at this `fun`.
    #[serde(default = "default_fun_satisfied")]
    pub fun_satisfied: f32,
}

fn default_fun_decay() -> f32 {
    0.00023
}

fn default_fun_mood() -> f32 {
    0.3
}

fn default_fun_satisfied() -> f32 {
    0.6
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
    /// M14 (plan V16): a Lab's staff.
    #[serde(default = "default_wage_researcher")]
    pub wage_researcher: i64,
    /// M15 W36: a Feed's staff (phase 4 reads it).
    #[serde(default = "default_wage_reporter")]
    pub wage_reporter: i64,
    /// L2 § 1: the seven new roles' wages (spec values as serde defaults;
    /// read only by hires at L2 buildings, which stand only with `jobs::on`).
    #[serde(default = "default_wage_host")]
    pub wage_host: i64,
    #[serde(default = "default_wage_attendant")]
    pub wage_attendant: i64,
    #[serde(default = "default_wage_cook")]
    pub wage_cook: i64,
    #[serde(default = "default_wage_fighter")]
    pub wage_fighter: i64,
    #[serde(default = "default_wage_croupier")]
    pub wage_croupier: i64,
    #[serde(default = "default_wage_concierge")]
    pub wage_concierge: i64,
    #[serde(default = "default_wage_fabber")]
    pub wage_fabber: i64,
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

fn default_wage_researcher() -> i64 {
    4
}

fn default_wage_reporter() -> i64 {
    8
}

fn default_wage_host() -> i64 {
    6
}
fn default_wage_attendant() -> i64 {
    5
}
fn default_wage_cook() -> i64 {
    5
}
fn default_wage_fighter() -> i64 {
    7
}
fn default_wage_croupier() -> i64 {
    6
}
fn default_wage_concierge() -> i64 {
    8
}
fn default_wage_fabber() -> i64 {
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
            Role::Researcher => self.wage_researcher,
            Role::Reporter => self.wage_reporter,
            Role::Host => self.wage_host,
            Role::Attendant => self.wage_attendant,
            Role::Cook => self.wage_cook,
            Role::Fighter => self.wage_fighter,
            Role::Croupier => self.wage_croupier,
            Role::Concierge => self.wage_concierge,
            Role::Fabber => self.wage_fabber,
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
    /// M12 review: a guard answers an alarm at a door (a corp raid, a riot)
    /// only from within this many tiles of it (Chebyshev), on shift and
    /// awake; the brawl then stands it at the door.
    #[serde(default = "CrimeCfg::default_answer_radius")]
    pub answer_radius: u32,
    /// M13 D47: sentences for the crimes appended after Vagrancy (days).
    #[serde(default)]
    pub sentence_days_ext: SentenceDaysExt,
}

impl CrimeCfg {
    fn default_answer_radius() -> u32 {
        96
    }
}

/// M13 D47: `[crime] sentence_days_ext`, matched by `law::sentence_ticks`
/// before it indexes `sentence_days` (as M12 D15 did Vagrancy).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SentenceDaysExt {
    pub grand_theft: u32,
    pub dealing: u32,
    pub abduction: u32,
    pub manslaughter: u32,
    /// M14 V18: the spec's 48 h.
    pub intrusion: u32,
    /// M14 V18: the spec's 120 h.
    pub data_theft: u32,
}

impl Default for SentenceDaysExt {
    fn default() -> Self {
        SentenceDaysExt { grand_theft: 6, dealing: 5, abduction: 20, manslaughter: 10, intrusion: 2, data_theft: 5 }
    }
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
    /// L2 (L23): the Statistical GangWork day's priors per member-day and
    /// the member-days of evidence they are worth (ledger maths).
    #[serde(default = "GangsCfg::default_stat_extort_prior")]
    pub stat_extort_prior: f32,
    #[serde(default = "GangsCfg::default_stat_claim_prior")]
    pub stat_claim_prior: f32,
    #[serde(default = "GangsCfg::default_stat_deal_prior")]
    pub stat_deal_prior: f32,
    #[serde(default = "GangsCfg::default_stat_prior_days")]
    pub stat_prior_days: f32,
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
    fn default_stat_extort_prior() -> f32 {
        0.02
    }
    fn default_stat_claim_prior() -> f32 {
        0.02
    }
    fn default_stat_deal_prior() -> f32 {
        0.01
    }
    fn default_stat_prior_days() -> f32 {
        30.0
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
    /// M13 D36 (phase 3): the gang's Harvest order.
    #[serde(default)]
    pub harvest: f32,
    /// M14 V30 (phase 3): the gang's VirtRaid order.
    #[serde(default)]
    pub virt_raid: f32,
    /// M14 review: added to VirtRaid's flat while the gang holds a hack
    /// grudge whose wipe is in its best runner's reach.
    #[serde(default)]
    pub virt_grudge: f32,
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
    /// M13 D46: a Statistical kitted agent's `p_robbed × (1 + flash_w × Kit.flash)`.
    #[serde(default)]
    pub flash_w: f32,
    /// M13 D37: the binder weights an Assaulted or Killed candidate by
    /// `1 + chrome_bind_w × Kit.fighting`.
    #[serde(default)]
    pub chrome_bind_w: f32,
    /// L2 phase 3 (plan L21-L23, L29-L30): the LOD budget and the churn;
    /// `false` (the default, and every pre-L2 config) is the M15 city.
    #[serde(default)]
    pub budget: bool,
    /// L2 (L21): sentenced, unpinned agents are held Statistical.
    #[serde(default = "LodCfg::default_true")]
    pub held_prisoners: bool,
    /// L2 (L22): class-2 bodies per gang, and in a raid window.
    #[serde(default = "LodCfg::default_gang_quota")]
    pub gang_quota: usize,
    #[serde(default = "LodCfg::default_private_quota")]
    pub private_quota: usize,
    #[serde(default = "LodCfg::default_raid_quota")]
    pub raid_quota: usize,
    /// L2 (L21, L22): the raid window opens this many hours before the muster.
    #[serde(default = "LodCfg::default_raid_promote_hours")]
    pub raid_promote_hours: u16,
    /// L2 (L22): members this close to the order's target rank in the front line.
    #[serde(default = "LodCfg::default_front_tiles")]
    pub front_tiles: u32,
    /// L2 (L21): a prisoner this close to release holds a body (the walk out).
    #[serde(default = "LodCfg::default_release_soon_hours")]
    pub release_soon_hours: u16,
    /// L2 (L22, spec § 4): the public watch ranks 3 only on shift (off
    /// shift it ranks as a civilian); `false` keeps M10 D20 (always 3).
    #[serde(default = "LodCfg::default_true")]
    pub watch_on_shift_only: bool,
}

impl LodCfg {
    fn default_true() -> bool {
        true
    }
    fn default_gang_quota() -> usize {
        25
    }
    fn default_private_quota() -> usize {
        12
    }
    fn default_raid_quota() -> usize {
        40
    }
    fn default_raid_promote_hours() -> u16 {
        4
    }
    fn default_front_tiles() -> u32 {
        30
    }
    fn default_release_soon_hours() -> u16 {
        2
    }
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
    /// M14 (spec § 4): a Lab, built only by the Research order.
    #[serde(default)]
    pub lab: i64,
    /// M15 § 7: a Feed (foundable through `Register` with `[news]` on).
    #[serde(default)]
    pub feed: i64,
    /// L2 § 1 (spec values as defaults; `founding::found_cost` prices them
    /// only with `jobs::on`).
    #[serde(default = "FoundCostCfg::default_club")]
    pub club: i64,
    #[serde(default = "FoundCostCfg::default_arcade")]
    pub arcade: i64,
    #[serde(default = "FoundCostCfg::default_noodle_bar")]
    pub noodle_bar: i64,
    #[serde(default = "FoundCostCfg::default_fight_pit")]
    pub fight_pit: i64,
    #[serde(default = "FoundCostCfg::default_den")]
    pub den: i64,
    #[serde(default = "FoundCostCfg::default_lounge")]
    pub lounge: i64,
    /// Corps only (the Fab is never `Register`ed).
    #[serde(default = "FoundCostCfg::default_fab")]
    pub fab: i64,
}

impl FoundCostCfg {
    fn default_hotel() -> i64 {
        250
    }
    fn default_club() -> i64 {
        400
    }
    fn default_arcade() -> i64 {
        200
    }
    fn default_noodle_bar() -> i64 {
        120
    }
    fn default_fight_pit() -> i64 {
        300
    }
    fn default_den() -> i64 {
        250
    }
    fn default_lounge() -> i64 {
        900
    }
    fn default_fab() -> i64 {
        900
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
    /// M14 (spec § 4).
    #[serde(default)]
    pub lab: i64,
    /// M15 § 7.
    #[serde(default)]
    pub feed: i64,
    /// L2 § 1 (spec values as defaults; no building of these kinds stands
    /// with L2 off).
    #[serde(default = "UpkeepCfg::default_club")]
    pub club: i64,
    #[serde(default = "UpkeepCfg::default_arcade")]
    pub arcade: i64,
    #[serde(default = "UpkeepCfg::default_noodle_bar")]
    pub noodle_bar: i64,
    #[serde(default = "UpkeepCfg::default_fight_pit")]
    pub fight_pit: i64,
    #[serde(default = "UpkeepCfg::default_den")]
    pub den: i64,
    #[serde(default = "UpkeepCfg::default_lounge")]
    pub lounge: i64,
    #[serde(default = "UpkeepCfg::default_fab")]
    pub fab: i64,
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
    fn default_club() -> i64 {
        10
    }
    fn default_arcade() -> i64 {
        4
    }
    fn default_noodle_bar() -> i64 {
        2
    }
    fn default_fight_pit() -> i64 {
        4
    }
    fn default_den() -> i64 {
        4
    }
    fn default_lounge() -> i64 {
        20
    }
    fn default_fab() -> i64 {
        15
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
            BuildingKind::Lab => self.lab,
            BuildingKind::Feed => self.feed,
            BuildingKind::Club => self.club,
            BuildingKind::Arcade => self.arcade,
            BuildingKind::NoodleBar => self.noodle_bar,
            BuildingKind::FightPit => self.fight_pit,
            BuildingKind::Den => self.den,
            BuildingKind::Lounge => self.lounge,
            BuildingKind::Fab => self.fab,
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
    /// M14 (spec § 4).
    #[serde(default)]
    pub lab: i64,
    /// M15 § 7.
    #[serde(default)]
    pub feed: i64,
    /// L2 § 1 (spec values as defaults).
    #[serde(default = "ValueCfg::default_club")]
    pub club: i64,
    #[serde(default = "ValueCfg::default_arcade")]
    pub arcade: i64,
    #[serde(default = "ValueCfg::default_noodle_bar")]
    pub noodle_bar: i64,
    #[serde(default = "ValueCfg::default_fight_pit")]
    pub fight_pit: i64,
    #[serde(default = "ValueCfg::default_den")]
    pub den: i64,
    #[serde(default = "ValueCfg::default_lounge")]
    pub lounge: i64,
    #[serde(default = "ValueCfg::default_fab")]
    pub fab: i64,
}

impl ValueCfg {
    fn default_club() -> i64 {
        600
    }
    fn default_arcade() -> i64 {
        300
    }
    fn default_noodle_bar() -> i64 {
        180
    }
    fn default_fight_pit() -> i64 {
        400
    }
    fn default_den() -> i64 {
        350
    }
    fn default_lounge() -> i64 {
        1400
    }
    fn default_fab() -> i64 {
        1500
    }
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
    /// M14 (plan V31).
    #[serde(default)]
    pub research: f32,
    /// M14 (plan V31, phase 3).
    #[serde(default)]
    pub virt_raid: f32,
    /// M15 W40.
    #[serde(default)]
    pub spin: f32,
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
    /// L2 (plan L5): `Register`'s per-capita targets of the leisure kinds
    /// (the spec's `residents_per` table as flat keys), read only with
    /// `jobs::on`.
    pub residents_per_club: u32,
    pub residents_per_arcade: u32,
    pub residents_per_noodle: u32,
    pub residents_per_pit: u32,
    pub residents_per_den: u32,
    pub residents_per_lounge: u32,
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
            found_cost: FoundCostCfg {
                bar: 300,
                home: 400,
                hotel: 250,
                clinic: 500,
                garage: 600,
                lab: 800,
                feed: 500,
                club: 400,
                arcade: 200,
                noodle_bar: 120,
                fight_pit: 300,
                den: 250,
                lounge: 900,
                fab: 900,
            },
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
                lab: 0,
                feed: 0,
                club: 10,
                arcade: 4,
                noodle_bar: 2,
                fight_pit: 4,
                den: 4,
                lounge: 20,
                fab: 15,
            },
            value: ValueCfg {
                farm: 1000,
                market: 1000,
                security_office: 500,
                clinic: 600,
                garage: 800,
                lab: 1200,
                feed: 800,
                club: 600,
                arcade: 300,
                noodle_bar: 180,
                fight_pit: 400,
                den: 350,
                lounge: 1400,
                fab: 1500,
            },
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
            residents_per_club: 600,
            residents_per_arcade: 600,
            residents_per_noodle: 120,
            residents_per_pit: 1000,
            residents_per_den: 800,
            residents_per_lounge: 2000,
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
                research: 0.0,
                virt_raid: 0.0,
                spin: 0.0,
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
    /// M14 V36.
    #[serde(default)]
    pub deck: T,
    /// M14 V28.
    #[serde(default)]
    pub camera: T,
}

impl AssetsCfg {
    /// The down payment share for `kind` (phase 5: implants have their own).
    pub fn down_frac_of(&self, kind: AssetKind) -> f32 {
        if kind.is_implant() {
            self.implant_down_frac
        } else {
            self.down_frac
        }
    }
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
            AssetClass::Deck => &self.deck,
            AssetClass::Camera => &self.camera,
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
    /// Phase 5 (the chrome finance spiral): an implant's down payment share
    /// (1.0 = cash only). `down_frac` for every other kind.
    pub implant_down_frac: f32,
    /// Phase 5: what an agent's `Register` pays for a Clinic or a Garage,
    /// overriding `[corps] found_cost` when set (seeded founders hold
    /// 200-300 coins against 500 and 600, so no NPC ever founded one).
    pub found_cost_clinic: Option<i64>,
    pub found_cost_garage: Option<i64>,
    /// Phase 5: an agent founder's per-capita target for a Clinic or a
    /// Garage (`founding::choose_kind`), overriding `[corps]
    /// residents_per_clinic`/`_garage` (which still cap the Tech corp): the
    /// seeded three Clinics and three Garages met the 700 target, so a
    /// founder always chose a Bar.
    pub founder_residents_per_seller: Option<u32>,
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
    /// Phase 1 deviation (plan D16 grows `FOUNDABLE`), per kind from phase
    /// 2: `Register`, `choose_kind` and a Tech corp's `Grow` may build a
    /// Garage only while `found_garage`, a Clinic only while `found_clinic`
    /// (false until phase 3 makes the Clinic do something).
    pub found_garage: bool,
    pub found_clinic: bool,
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
                deck: vec![400, 1500, 5000],
                camera: vec![120, 400, 1200],
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
                deck: vec![1, 3, 8],
                camera: vec![1, 2, 4],
            },
            wear: PerKind {
                motorcycle: 1,
                car: 1,
                truck: 1,
                flyer: 1,
                implant: 0,
                robot: 1,
                pack: 0,
                bridge: 0,
                deck: 0,
                camera: 1,
            },
            parts_per: PerKind {
                motorcycle: 4,
                car: 8,
                truck: 12,
                flyer: 30,
                implant: 3,
                robot: 10,
                pack: 0,
                bridge: 0,
                deck: 4,
                camera: 2,
            },
            parts_price: 15,
            import_frac: 0.6,
            part_credit: 20,
            down_frac: 0.25,
            implant_down_frac: 0.25,
            found_cost_clinic: None,
            found_cost_garage: None,
            founder_residents_per_seller: None,
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
            found_garage: false,
            found_clinic: false,
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
    /// Plan D28: `security::contest` moves the odds this much per tier.
    pub contest_step: f32,
    /// Spec § 3: a Clinic or Garage buys a used asset at this share of its
    /// value (phase 2: a hunkering corp's fleet sale, D44).
    pub buyback_frac: f32,
    /// Phase 3 (spec § 3): the fee to install a gang's own implant, per tier.
    pub install_fee: Vec<i64>,
    /// Sanity lost at once on an install.
    pub install_shock: f32,
    /// Daily: sanity moves toward `1 − Kit.load` by at most this.
    pub sanity_drift: f32,
    /// Daily: an extra sanity loss per installed implant whose upkeep is unpaid.
    pub unmedicated_drift: f32,
    /// Below this sanity: `−edgy_mood` mood, `+edgy_fight` on the Fight flat.
    pub edgy: f32,
    pub edgy_mood: f32,
    pub edgy_fight: f32,
    /// Below this sanity an episode can start (daily roll).
    pub psycho: f32,
    /// `p_episode = episode_base × (psycho − sanity) ÷ psycho`.
    pub episode_base: f32,
    /// × on `p_episode` within `[stims] stim_hours` of a stim (phase 4 reads it).
    pub stim_episode_mult: f32,
    pub episode_hours: u32,
    /// An episode's `Attack` kills at `fight_death_p × this`.
    pub berserk_kill_mult: f32,
    /// A guard's contested arrest of an episode kills at this.
    pub psycho_kill: f32,
    pub therapy_price: i64,
    pub therapy_gain: f32,
    /// Harvest sees chrome at `Kit.visible ≥` this.
    pub harvest_min_visible: u8,
    /// Daily, while a gang holds Harvest: a Statistical target's roll
    /// `abduct_base × chrome_value ÷ 1000 × (2 − coverage)`.
    pub abduct_base: f32,
    /// A ripped live abductee dies with this chance.
    pub p_rip_kill: f32,
    /// Plan D35: the Loot goal sees unsettled bodies within this many tiles
    /// (Manhattan; a plan key the spec leaves unnamed).
    pub loot_reach: u32,
    /// Plan D44 (phase 3 deviation): a gang buys its members' Arms with the
    /// treasury at this (the spec's `gang_buy_floor` 600 is never reached
    /// before phase 4's dealing money; seed 42's gangs hold 0-600).
    pub gang_chrome_floor: i64,
    /// Plan D34 (phase 3 deviation): the Treat goal is considered below this
    /// sanity (1.0: any body that has lost some; a whole one has nothing to
    /// treat, else the Treat curve's floor sent the calm to Therapy whenever
    /// they idled).
    pub treat_below: f32,
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
            contest_step: 0.25,
            buyback_frac: 0.4,
            install_fee: vec![30, 80, 200],
            install_shock: 0.05,
            sanity_drift: 0.05,
            unmedicated_drift: 0.02,
            edgy: 0.5,
            edgy_mood: 0.2,
            edgy_fight: 0.1,
            psycho: 0.25,
            episode_base: 0.05,
            stim_episode_mult: 2.0,
            episode_hours: 3,
            berserk_kill_mult: 4.0,
            psycho_kill: 0.3,
            therapy_price: 60,
            therapy_gain: 0.15,
            harvest_min_visible: 3,
            abduct_base: 0.002,
            p_rip_kill: 0.5,
            loot_reach: 16,
            gang_chrome_floor: 300,
            treat_below: 1.0,
        }
    }
}

/// M13 § 2 vehicles (plan phase 2, D19-D27). Read only while `[assets]
/// enabled`; `off()` carries the spec's values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VehiclesCfg {
    /// The road share a Coarse trip's `timed_mult` assumes (D21).
    pub road_share: f32,
    /// A Full driver's steps per executor call (D19).
    pub max_steps_per_tick: u8,
    /// A flyer's ticks per Chebyshev tile (D22).
    pub flyer_ticks_per_tile: f32,
    /// Step multipliers `[road, ground, farmland]` per road vehicle (a Door
    /// reads ground; 1.0 is walking pace).
    pub mult: PerKind<Vec<f32>>,
    /// `HaulToMarket` batch multiplier per vehicle (D23).
    pub haul: PerKind<u32>,
    pub crash_per_tile: f32,
    pub speed: PerKind<f32>,
    pub chase_mult: f32,
    pub crash_radius: u32,
    pub dodge_w: f32,
    pub dodge_cap: f32,
    pub p_crash_kill: f32,
    pub crash_wear: u8,
    pub crash_litter: u8,
    pub steal_vehicle_cost: f32,
    pub fence_frac: f32,
    pub vehicle_theft_base: f32,
    pub garage_mult: f32,
    pub bikes_max_share: f32,
    /// Plan D26: a thief binds a street-parked vehicle within this many tiles.
    pub steal_reach: u32,
}

impl Default for VehiclesCfg {
    fn default() -> Self {
        VehiclesCfg::off()
    }
}

impl VehiclesCfg {
    /// The spec's values (assets off reads none of them).
    pub fn off() -> VehiclesCfg {
        VehiclesCfg {
            road_share: 0.7,
            max_steps_per_tick: 4,
            flyer_ticks_per_tile: 0.3,
            mult: PerKind {
                motorcycle: vec![0.25, 0.6, 0.8],
                car: vec![0.2, 1.0, 1.0],
                truck: vec![0.3, 1.0, 0.6],
                ..PerKind::default()
            },
            haul: PerKind { motorcycle: 1, car: 2, truck: 6, flyer: 1, ..PerKind::default() },
            crash_per_tile: 0.00004,
            speed: PerKind { motorcycle: 1.5, car: 1.0, truck: 1.2, flyer: 0.0, ..PerKind::default() },
            chase_mult: 6.0,
            crash_radius: 2,
            dodge_w: 0.6,
            dodge_cap: 0.9,
            p_crash_kill: 0.3,
            crash_wear: 30,
            crash_litter: 24,
            steal_vehicle_cost: 10.0,
            fence_frac: 0.2,
            vehicle_theft_base: 0.004,
            garage_mult: 0.2,
            bikes_max_share: 0.5,
            steal_reach: 40,
        }
    }

    /// `mult[kind][column]` (0 road, 1 ground, 2 farmland); 1.0 when absent.
    pub fn mult_of(&self, kind: AssetKind, column: usize) -> f32 {
        self.mult.get(kind).get(column).copied().unwrap_or(1.0)
    }
}

/// M13 § 6 the Shop goal, the Garage and fleets (plan D17, D18, D43, D44).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShopCfg {
    pub shop_cooldown_days: u32,
    /// Tiles of commute at which the vehicle term reads 0.5.
    pub commute_ref: u32,
    pub shop_flat: f32,
    pub gang_buy_floor: i64,
    /// Coins a day a vehicle parked in someone else's Garage pays its owner.
    pub garage_rent: i64,
    /// Plan D43: upkeep + finance a day at most this share of daily income.
    pub max_burden: f32,
    /// Plan D43: a Statistical shopper buys at this score or above.
    pub stat_shop_min: f32,
    /// Plan D18: a seeded Garage owner's wallet.
    pub seed_owner_coins: i64,
    /// Plan D44: Food corps buy trucks under any order but Hunker.
    pub fleet_any_order: bool,
    /// Plan D17: a Tech corp's demand reference (sales a day per building).
    pub tech_demand_ref: f32,
    /// M13 phase 5 (the chrome finance spiral): the D43 burden test reads
    /// income net of this many meals a day at the mean Market price
    /// (0 = gross income, the plan's rule).
    pub burden_meals: f32,
    /// Phase 5: a corp buys its exec a flyer with this many times its price
    /// in the treasury and the books (0 = never).
    pub exec_flyer_cash_mult: f32,
}

impl Default for ShopCfg {
    fn default() -> Self {
        ShopCfg::off()
    }
}

impl ShopCfg {
    /// The spec's and plan's values (assets off reads none of them).
    pub fn off() -> ShopCfg {
        ShopCfg {
            shop_cooldown_days: 7,
            commute_ref: 60,
            shop_flat: 0.02,
            gang_buy_floor: 600,
            garage_rent: 1,
            max_burden: 0.5,
            stat_shop_min: 0.3,
            seed_owner_coins: 300,
            fleet_any_order: false,
            tech_demand_ref: 1.0,
            burden_meals: 0.0,
            exec_flyer_cash_mult: 0.0,
        }
    }
}

/// M13 § 4 stims (plan phase 4, D38-D40). Read only while `[assets]
/// enabled`; `off()` carries the plan's values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StimsCfg {
    /// Coins of precursors per dose cooked (`Flow::Import`); also a legal
    /// Market's restock cost a dose (D40).
    pub cook_cost: i64,
    /// Doses a gang cooks per member a day.
    pub cook_per_member: u32,
    /// A gang cooks its Hideout stock up to this.
    pub cook_target: u32,
    /// A dealer picks up this many doses; the gang deals only with this much in stock.
    pub deal_batch: u32,
    /// A dealer's dose sells at `deal_price × (0.8 + 0.4 × leader greed)`.
    pub deal_price: i64,
    /// Coins a dose the dealer keeps.
    pub dealer_cut: i64,
    /// A legal Market's dose sells at `stim_price × the owner's Food price_level`.
    pub stim_price: i64,
    /// Legal Markets restock Stims to this at midnight.
    pub stim_restock_floor: u32,
    pub stim_energy: f32,
    /// Mood bias for `stim_hours` after a dose.
    pub stim_mood: f32,
    pub stim_hours: u32,
    /// `addiction += addict_per_use × (1.5 − lawfulness)` a dose.
    pub addict_per_use: f32,
    /// A day with no dose lowers addiction by this.
    pub addiction_decay: f32,
    /// Hooked at `addiction ≥` this.
    pub hooked: f32,
    /// Hooked with no dose for this long: withdrawal.
    pub withdrawal_hours: u32,
    pub withdrawal_mood: f32,
    /// Energy decay × this in withdrawal.
    pub withdrawal_energy: f32,
    /// `StealFood` (and `StealVehicle`) cost − this in withdrawal.
    pub withdrawal_steal_bonus: f32,
    /// A Statistical agent's `p_steal` × this in withdrawal.
    pub withdrawal_steal_mult: f32,
    /// A dose taken at `addiction ≥ 0.8` kills with this chance.
    pub p_overdose: f32,
    /// Doses a hooked Statistical adult buys a day.
    pub uses_per_day: u32,
    pub detox_price: i64,
    /// Detox: `addiction × detox_mult`.
    pub detox_mult: f32,
    /// A hooked Statistical adult with the coins and the will detoxes with this chance a day.
    pub p_stat_detox: f32,
    /// The GetHigh goal's flat.
    pub get_high_flat: f32,
}

impl Default for StimsCfg {
    fn default() -> Self {
        StimsCfg::off()
    }
}

impl StimsCfg {
    /// The plan's values (assets off reads none of them).
    pub fn off() -> StimsCfg {
        StimsCfg {
            cook_cost: 1,
            cook_per_member: 2,
            cook_target: 200,
            deal_batch: 10,
            deal_price: 5,
            dealer_cut: 1,
            stim_price: 6,
            stim_restock_floor: 300,
            stim_energy: 0.4,
            stim_mood: 0.3,
            stim_hours: 6,
            addict_per_use: 0.06,
            addiction_decay: 0.02,
            hooked: 0.5,
            withdrawal_hours: 24,
            withdrawal_mood: 0.4,
            withdrawal_energy: 1.3,
            withdrawal_steal_bonus: 3.0,
            withdrawal_steal_mult: 1.5,
            p_overdose: 0.002,
            uses_per_day: 2,
            detox_price: 80,
            detox_mult: 0.3,
            p_stat_detox: 0.05,
            get_high_flat: 0.0,
        }
    }
}

/// `[robots] robot_tier_by_kind` (spec § 5): the tier `Secure` buys per
/// building kind; a kind left out buys tier 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RobotTiers {
    pub farm: u8,
    pub market: u8,
    pub home: u8,
    pub security_office: u8,
    pub clinic: u8,
    pub garage: u8,
}

impl Default for RobotTiers {
    fn default() -> Self {
        RobotTiers { farm: 2, market: 2, home: 1, security_office: 3, clinic: 2, garage: 2 }
    }
}

/// M13 § 5 the security robot (plan phase 4, D41, D42).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RobotsCfg {
    /// `law::fighting` of a robot, per tier.
    pub robot_fighting: Vec<f32>,
    /// D42: a robot's cost is its price plus this many days of upkeep,
    /// against as many days of a guard contract.
    pub robot_horizon_days: u32,
    /// D42: a corp buys a robot only with its treasury at this × the price.
    pub robot_cash_mult: f32,
    pub robot_tier_by_kind: RobotTiers,
    /// M14: a hacked robot changes owner for this long (nothing reads it in M13).
    pub hack_hours: u32,
}

impl Default for RobotsCfg {
    fn default() -> Self {
        RobotsCfg::off()
    }
}

impl RobotsCfg {
    /// The plan's values (assets off reads none of them).
    pub fn off() -> RobotsCfg {
        RobotsCfg {
            robot_fighting: vec![0.5, 0.75, 1.0],
            robot_horizon_days: 60,
            robot_cash_mult: 2.0,
            robot_tier_by_kind: RobotTiers::default(),
            hack_hours: 24,
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
    /// M13 D40: Markets sell Stims legally from day 0.
    #[serde(default)]
    pub stims_legal: bool,
    /// M14 (plan V26/V42): the ICE seeded on the Treasury and the Precinct.
    #[serde(default = "default_city_ice")]
    pub city_ice: u8,
}

fn default_city_ice() -> u8 {
    2
}

// ---------------------------------------------------------------------------
// M14 Data and Virt (docs/M14_VIRT.md, plan phase 1.2). Each section has an
// `off()`; `VirtCfg::off().enabled` is false and gates every M14 branch, the
// others hold the TOML's values (read only while `[virt]` is on).
// ---------------------------------------------------------------------------

/// M14 § 1: the plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VirtCfg {
    pub enabled: bool,
    pub terminal_fee: i64,
    pub jacked_fight_mult: f32,
    pub dump_sanity: f32,
    /// Per deck tier.
    pub hop_ticks: Vec<u32>,
    pub contest_ticks: u32,
    pub hop_eps: f32,
    /// No free choice targets a node below this (orders may, plan V11).
    pub min_route_p: f32,
    pub public_links_tier: u8,
    pub access_links_tier: u8,
    pub trunk_links_tier: u8,
}

impl Default for VirtCfg {
    fn default() -> Self {
        VirtCfg::off()
    }
}

impl VirtCfg {
    pub fn off() -> VirtCfg {
        VirtCfg {
            enabled: false,
            terminal_fee: 3,
            jacked_fight_mult: 0.3,
            dump_sanity: 0.1,
            hop_ticks: vec![8, 5, 3],
            contest_ticks: 5,
            hop_eps: 0.01,
            min_route_p: 0.25,
            public_links_tier: 1,
            access_links_tier: 1,
            trunk_links_tier: 2,
        }
    }
}

/// M14 § 2: decks and the hacking skill's seed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecksCfg {
    pub deck_shop_min: f32,
    /// Plan V35: `hacking = hack_seed_scale x U(0,1)^3`.
    pub hack_seed_scale: f32,
    pub hack_drift: f32,
    pub upgrade_frac: f32,
    pub gang_deck_floor: i64,
    pub gang_decks_max: u32,
}

impl Default for DecksCfg {
    fn default() -> Self {
        DecksCfg::off()
    }
}

impl DecksCfg {
    pub fn off() -> DecksCfg {
        DecksCfg {
            deck_shop_min: 0.4,
            hack_seed_scale: 0.8,
            hack_drift: 0.01,
            upgrade_frac: 0.7,
            gang_deck_floor: 900,
            gang_decks_max: 3,
        }
    }
}

/// `[ice] ice_seed`: the ICE seeded on a building node by kind (plan V26).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceSeed {
    pub lab: u8,
    pub security_office: u8,
    pub farm: u8,
    pub market: u8,
    pub hideout: u8,
    pub clinic: u8,
    pub garage: u8,
    pub bar: u8,
    pub hotel: u8,
}

impl IceSeed {
    pub fn for_kind(&self, kind: BuildingKind) -> u8 {
        match kind {
            BuildingKind::Lab => self.lab,
            BuildingKind::SecurityOffice => self.security_office,
            BuildingKind::Farm => self.farm,
            BuildingKind::Market => self.market,
            BuildingKind::Hideout => self.hideout,
            BuildingKind::Clinic => self.clinic,
            BuildingKind::Garage => self.garage,
            BuildingKind::Bar => self.bar,
            BuildingKind::Hotel => self.hotel,
            _ => 0,
        }
    }
}

/// `[ice] act_ticks` per purpose (phase 2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActTicks {
    pub data: u32,
    pub ledger: u32,
    pub door: u32,
    pub robot: u32,
    pub camera: u32,
}

/// M14 § 3: ICE, its upkeep and the contest's outcomes. The contest step is
/// `[chrome] contest_step` (one value, plan 1.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceCfg {
    pub hack_w: f32,
    /// Per ICE tier 0..=3.
    pub ice_price: Vec<i64>,
    pub self_install_frac: f32,
    /// Per ICE tier 0..=3.
    pub ice_upkeep: Vec<i64>,
    pub ice_off_days: u8,
    pub ice_value_steps: Vec<i64>,
    pub ice_seed: IceSeed,
    pub alarm_hours: u32,
    pub fry_base: Vec<f32>,
    pub fry_gap: f32,
    pub fry_sanity: f32,
    pub fry_energy: f32,
    pub p_fry_kill: Vec<f32>,
    pub flatline_gap: f32,
    pub p_flatline: f32,
    pub fry_wear: u8,
    pub trace_p: Vec<f32>,
    pub stealth_w: f32,
    pub daze_ticks: u32,
    pub act_ticks: ActTicks,
    pub door_strip_cap: f32,
    pub door_cooldown_days: u32,
    /// M14 addendum (plan V65): a trace's attribution confidence after `h`
    /// hops between the portal and the target is `(1 - decay)^h`.
    pub trace_decay_per_hop: f32,
    /// Plan V65: below this confidence a trace names nobody (no Sighting,
    /// no report, no `last_seen`, the shock names no one).
    pub trace_floor: f32,
}

impl Default for IceCfg {
    fn default() -> Self {
        IceCfg::off()
    }
}

impl IceCfg {
    pub fn off() -> IceCfg {
        IceCfg {
            hack_w: 1.0,
            ice_price: vec![0, 150, 600, 2000],
            self_install_frac: 0.5,
            ice_upkeep: vec![0, 0, 1, 2],
            ice_off_days: 2,
            ice_value_steps: vec![200, 1500, 6000],
            ice_seed: IceSeed {
                lab: 2,
                security_office: 2,
                farm: 1,
                market: 1,
                hideout: 0,
                clinic: 1,
                garage: 1,
                bar: 0,
                hotel: 0,
            },
            alarm_hours: 48,
            fry_base: vec![0.0, 0.05, 0.25, 0.6],
            fry_gap: 1.0,
            fry_sanity: 0.3,
            fry_energy: 0.5,
            p_fry_kill: vec![0.0, 0.0, 0.05, 0.3],
            flatline_gap: 1.5,
            p_flatline: 0.8,
            fry_wear: 20,
            trace_p: vec![0.0, 0.2, 0.45, 0.7],
            stealth_w: 0.5,
            daze_ticks: 30,
            act_ticks: ActTicks { data: 30, ledger: 30, door: 10, robot: 15, camera: 5 },
            door_strip_cap: 0.5,
            door_cooldown_days: 3,
            trace_decay_per_hop: 0.35,
            trace_floor: 0.15,
        }
    }

    /// `fry_base[ice]`, `p_fry_kill[ice]`, `trace_p[ice]` (0 past the table).
    pub fn at(v: &[f32], ice: u8) -> f32 {
        v.get(usize::from(ice)).copied().unwrap_or(0.0)
    }

    /// `ice_upkeep[tier]` (0 past the table).
    pub fn upkeep(&self, tier: u8) -> i64 {
        self.ice_upkeep.get(usize::from(tier)).copied().unwrap_or(0)
    }

    /// `ice_price[tier]` (`None` past the table).
    pub fn price(&self, tier: u8) -> Option<i64> {
        self.ice_price.get(usize::from(tier)).copied()
    }
}

/// M14 § 4: Data and Labs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataCfg {
    pub data_per_shift: f32,
    pub store_cap: u32,
    pub steal_units: Vec<u32>,
    pub data_price: i64,
    pub gang_data_keep: u32,
    pub backup_days: u32,
    pub ledger_frac: f32,
    pub ledger_cap: Vec<i64>,
    pub door_loss: i64,
    /// Plan: a seeded Lab's opening store in its focus track.
    pub seed_store: u32,
    /// M14 phase 3 (procurement budget): a corp's Data purchases in a day
    /// are capped at this × its closing treasury, and never take the
    /// treasury below `treasury_ref / 4` (M13's fleet-buy reserve).
    pub buy_budget_frac: f32,
}

impl Default for DataCfg {
    fn default() -> Self {
        DataCfg::off()
    }
}

impl DataCfg {
    pub fn off() -> DataCfg {
        DataCfg {
            data_per_shift: 3.0,
            store_cap: 2000,
            steal_units: vec![60, 150, 400],
            data_price: 3,
            gang_data_keep: 0,
            backup_days: 3,
            ledger_frac: 0.05,
            ledger_cap: vec![0, 400, 1500],
            door_loss: 100,
            seed_store: 300,
            buy_budget_frac: 0.05,
        }
    }
}

/// `[tech] requires`: the owner's tier in the kind's track a seller needs to
/// sell tier `t` (`requires[t - 1]`; a tier past the list is not gated).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TechRequires {
    pub implant: Vec<u8>,
    pub deck: Vec<u8>,
    pub robot: Vec<u8>,
    pub camera: Vec<u8>,
    pub motorcycle: Vec<u8>,
    pub car: Vec<u8>,
    pub truck: Vec<u8>,
    pub flyer: Vec<u8>,
    pub pack: Vec<u8>,
    pub bridge: Vec<u8>,
}

impl TechRequires {
    /// The list for an asset kind's class.
    pub fn of(&self, kind: AssetKind) -> &[u8] {
        match kind.class() {
            AssetClass::Motorcycle => &self.motorcycle,
            AssetClass::Car => &self.car,
            AssetClass::Truck => &self.truck,
            AssetClass::Flyer => &self.flyer,
            AssetClass::Implant => &self.implant,
            AssetClass::Robot => &self.robot,
            AssetClass::Pack => &self.pack,
            AssetClass::Bridge => &self.bridge,
            AssetClass::Deck => &self.deck,
            AssetClass::Camera => &self.camera,
        }
    }
}

/// `[tech] focus_by_niche`: a corp's opening focus by its first niche.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusByNiche {
    pub food: String,
    pub housing: String,
    pub security: String,
    pub tech: String,
}

impl Default for FocusByNiche {
    fn default() -> Self {
        FocusByNiche {
            food: "Industry".into(),
            housing: "Industry".into(),
            security: "Deck".into(),
            tech: "Chrome".into(),
        }
    }
}

impl FocusByNiche {
    pub fn for_niche(&self, n: Niche) -> crate::virt::Track {
        let s = match n {
            Niche::Food => &self.food,
            Niche::Housing => &self.housing,
            Niche::Security => &self.security,
            Niche::Tech => &self.tech,
        };
        crate::virt::Track::parse(s).unwrap_or(crate::virt::Track::Industry)
    }
}

/// M14 § 5: the tech tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TechCfg {
    pub requires: TechRequires,
    /// Data to reach tier i.
    pub tier_cost: Vec<u32>,
    pub tier_coins: Vec<i64>,
    pub upkeep_data: Vec<u32>,
    pub upkeep_coins: Vec<i64>,
    pub decay_days: u8,
    pub research_rate: u32,
    pub orphan_cap: u8,
    pub industry_prod: Vec<f32>,
    pub focus_by_niche: FocusByNiche,
    /// `[chrome, deck, industry]` by `Corp.name`; every other corp `[1, 1, 1]`.
    pub seed: std::collections::BTreeMap<String, [u8; 3]>,
    /// Plan V47: coins per seeded Lab to its owner, out of nothing.
    pub seed_lab_grant: i64,
    /// M14 phase 5 (deviation from the spec's Research score): Research's
    /// tech gap reads the street tier (the best living corp's tier in the
    /// corp's focus track), not only niche rivals. Off = the spec.
    pub research_gap_street: bool,
    /// M14 phase 5: a floor under Research's `max lapse` consideration so a
    /// corp that is not lapsing can still score it. 0 = the spec.
    pub research_lapse_floor: f32,
    /// M14 phase 5: days a corp must have held Research before it builds a
    /// Lab (0 = the spec: on the first day).
    pub research_build_days: u32,
    /// M14 phase 5: Research is gated shut until the corp has this many days
    /// of cashflow on its books (0 = the spec: from day 0).
    pub research_min_days: u32,
}

impl Default for TechCfg {
    fn default() -> Self {
        TechCfg::off()
    }
}

impl TechCfg {
    pub fn off() -> TechCfg {
        TechCfg {
            requires: TechRequires {
                implant: vec![1, 2, 3],
                deck: vec![1, 2, 3],
                robot: vec![1, 2, 3],
                camera: vec![1, 2, 3],
                motorcycle: vec![1],
                car: vec![1, 2],
                truck: vec![2],
                flyer: vec![3],
                pack: vec![1, 1],
                bridge: Vec::new(),
            },
            tier_cost: vec![0, 0, 400, 1200],
            tier_coins: vec![0, 0, 800, 2500],
            upkeep_data: vec![0, 0, 3, 8],
            upkeep_coins: vec![0, 0, 0, 0],
            decay_days: 7,
            research_rate: 40,
            orphan_cap: 2,
            industry_prod: vec![0.0, 0.0, 0.1, 0.2],
            focus_by_niche: FocusByNiche::default(),
            seed: [("Zetatech", [3, 1, 3]), ("Arasaka", [1, 3, 1]), ("Militech", [1, 2, 1])]
                .into_iter()
                .map(|(n, t)| (n.to_string(), t))
                .collect(),
            seed_lab_grant: 2000,
            research_gap_street: false,
            research_lapse_floor: 0.0,
            research_build_days: 0,
            research_min_days: 0,
        }
    }

    pub fn upkeep_data_at(&self, tier: u8) -> u32 {
        self.upkeep_data.get(usize::from(tier)).copied().unwrap_or(0)
    }

    pub fn upkeep_coins_at(&self, tier: u8) -> i64 {
        self.upkeep_coins.get(usize::from(tier)).copied().unwrap_or(0)
    }
}

/// M14 § 6: the Hack goal and the orders (phases 2-3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HackCfg {
    pub hack_min: f32,
    pub hack_ref: i64,
    pub hack_flat: f32,
    pub hack_cooldown_days: u32,
    pub fried_cooldown_days: u32,
    pub virt_runners: u32,
    pub door_lead_hours: u32,
    pub door_hours: u32,
    pub blind_hours: u32,
    pub wipe_min: f32,
    pub stream_min: f32,
    pub order_score: f32,
    pub stat_hack_min: f32,
    /// Plan V66 (addendum's quiet and loud): a quiet run takes `quiet_take`
    /// of the units (or coins) and is traced at `quiet_trace` of the odds,
    /// and raises no alarm; a loud one `loud_take` and `loud_trace` (the
    /// trace capped at 1). `*_att` add to the runner's `att` (0: the
    /// contest table holds in both modes).
    pub quiet_take: f32,
    pub loud_take: f32,
    pub quiet_trace: f32,
    pub loud_trace: f32,
    pub quiet_att: f32,
    pub loud_att: f32,
    /// Plan V66: a freelancer runs loud when `(1 - lawfulness) x courage`
    /// reaches this.
    pub loud_min: f32,
}

impl Default for HackCfg {
    fn default() -> Self {
        HackCfg::off()
    }
}

impl HackCfg {
    pub fn off() -> HackCfg {
        HackCfg {
            hack_min: 0.3,
            hack_ref: 500,
            hack_flat: 0.0,
            hack_cooldown_days: 2,
            fried_cooldown_days: 10,
            virt_runners: 2,
            door_lead_hours: 2,
            door_hours: 6,
            blind_hours: 12,
            wipe_min: 0.6,
            stream_min: 0.2,
            order_score: 0.9,
            stat_hack_min: 0.5,
            quiet_take: 0.5,
            loud_take: 1.5,
            quiet_trace: 0.5,
            loud_trace: 2.0,
            quiet_att: 0.0,
            loud_att: 0.0,
            loud_min: 0.3,
        }
    }
}

/// M14 § 6: faction databases (phases 2-3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DbCfg {
    pub db_cap: usize,
    pub sighting_days: u32,
}

impl Default for DbCfg {
    fn default() -> Self {
        DbCfg::off()
    }
}

impl DbCfg {
    pub fn off() -> DbCfg {
        DbCfg { db_cap: 32, sighting_days: 14 }
    }
}

// ---------------------------------------------------------------------------
// M15 Word and blood (docs/M15_WORD_AND_BLOOD.md; plan phase 1.2)
// ---------------------------------------------------------------------------

/// A value per `Deed` (spec deed-keyed tables); a missing key reads 0.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeedTable {
    pub killed: f32,
    pub assaulted: f32,
    pub robbed: f32,
    pub extorted: f32,
    pub stripped: f32,
    pub arrested: f32,
    pub married: f32,
    pub evicted: f32,
    pub struck: f32,
    pub raided: f32,
    pub founded: f32,
    pub avenged: f32,
    pub betrayed: f32,
    pub repaid: f32,
    pub poached: f32,
}

impl DeedTable {
    pub fn get(&self, d: crate::word::Deed) -> f32 {
        use crate::word::Deed;
        match d {
            Deed::Killed => self.killed,
            Deed::Assaulted => self.assaulted,
            Deed::Robbed => self.robbed,
            Deed::Extorted => self.extorted,
            Deed::Stripped => self.stripped,
            Deed::Arrested => self.arrested,
            Deed::Married => self.married,
            Deed::Evicted => self.evicted,
            Deed::Struck => self.struck,
            Deed::Raided => self.raided,
            Deed::Founded => self.founded,
            Deed::Avenged => self.avenged,
            Deed::Betrayed => self.betrayed,
            Deed::Repaid => self.repaid,
            Deed::Poached => self.poached,
        }
    }

    /// The table from values in `Deed::ALL` order.
    pub fn of(v: [f32; 15]) -> DeedTable {
        let [killed, assaulted, robbed, extorted, stripped, arrested, married, evicted, struck, raided, founded, avenged, betrayed, repaid, poached] =
            v;
        DeedTable {
            killed,
            assaulted,
            robbed,
            extorted,
            stripped,
            arrested,
            married,
            evicted,
            struck,
            raided,
            founded,
            avenged,
            betrayed,
            repaid,
            poached,
        }
    }
}

/// M15 § 1: the word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GossipCfg {
    pub enabled: bool,
    pub gossip_min: f32,
    pub max_hops: u8,
    pub hop_salience: f32,
    pub distort_base: f32,
    pub pool_cap: usize,
    pub pool_decay: f32,
    pub leak_min: f32,
    pub leak_frac: f32,
    pub reach_min: f32,
    pub hear_p: f32,
    pub pool_conf: f32,
    pub kin_p: [f32; 4],
    pub rumour_cap: usize,
    pub rumour_cap_statistical: usize,
    pub relay_conf: f32,
    pub reach0: DeedTable,
    pub deed_sal: DeedTable,
    pub deed_sev: DeedTable,
    /// W9: today's `social::gossip` runs exactly; the new channel writes
    /// `heard` only and touches no edge.
    pub legacy_second_hand: bool,
    /// W10: kin captured at death.
    pub kin_cap: usize,
}

impl Default for GossipCfg {
    fn default() -> Self {
        GossipCfg::off()
    }
}

impl GossipCfg {
    pub fn off() -> GossipCfg {
        GossipCfg {
            enabled: false,
            gossip_min: 0.3,
            max_hops: 6,
            hop_salience: 0.7,
            distort_base: 0.15,
            pool_cap: 64,
            pool_decay: 0.8,
            leak_min: 0.4,
            leak_frac: 0.4,
            reach_min: 0.05,
            hear_p: 0.5,
            pool_conf: 0.6,
            kin_p: [0.9, 0.7, 0.5, 0.3],
            rumour_cap: 8,
            rumour_cap_statistical: 3,
            relay_conf: 0.8,
            reach0: DeedTable::of([1.0, 0.5, 0.3, 0.2, 0.5, 0.6, 0.3, 0.4, 0.8, 0.9, 0.4, 1.0, 0.6, 0.1, 0.3]),
            deed_sal: DeedTable::of([0.9, 0.6, 0.5, 0.4, 0.6, 0.5, 0.3, 0.5, 0.6, 0.8, 0.4, 0.9, 0.7, 0.3, 0.4]),
            deed_sev: DeedTable::of([1.0, 0.5, 0.3, 0.2, 0.6, 0.0, 0.0, 0.3, 0.0, 0.6, 0.0, 0.8, 0.6, 0.0, 0.1]),
            legacy_second_hand: true,
            kin_cap: 12,
        }
    }
}

/// M15 § 2: reputation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReputationCfg {
    pub half_life_days: f32,
    pub hop_w: [f32; 4],
    pub pool_w: f32,
    pub dread_scale: f32,
    pub honour_scale: f32,
    pub heat_scale: f32,
    pub fame_scale: f32,
    pub heat_wanted: f32,
    pub heat_arrest: f32,
    pub bribe_honour: f32,
    pub dread_w: DeedTable,
    pub honour_w: DeedTable,
    pub heat_w: DeedTable,
    pub own_bias: f32,
    pub heat_pressure_w: f32,
    pub harvest_dread_max: f32,
    pub acquire_honour_min: f32,
    pub bind_dread_w: f32,
}

impl Default for ReputationCfg {
    fn default() -> Self {
        ReputationCfg::off()
    }
}

impl ReputationCfg {
    pub fn off() -> ReputationCfg {
        //                        kil  ass  rob   ext  str  arr  mar  evi   stk  rai  fou  ave  bet   rep  poa
        let dread_w = DeedTable::of([1.0, 0.4, 0.15, 0.3, 0.2, 0.0, 0.0, 0.0, 0.0, 0.6, 0.0, 0.8, 0.0, 0.0, 0.0]);
        let honour_w = DeedTable::of([0.0, 0.0, -0.3, -0.2, -0.6, 0.0, 0.1, -0.2, 0.0, 0.0, 0.0, 0.4, -1.0, 0.3, -0.1]);
        let heat_w = DeedTable::of([0.5, 0.2, 0.2, 0.1, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        ReputationCfg {
            half_life_days: 7.0,
            hop_w: [1.0, 0.8, 0.6, 0.4],
            pool_w: 2.0,
            dread_scale: 3.0,
            honour_scale: 3.0,
            heat_scale: 2.0,
            fame_scale: 20.0,
            heat_wanted: 0.6,
            heat_arrest: 0.2,
            bribe_honour: 0.5,
            dread_w,
            honour_w,
            heat_w,
            own_bias: 0.3,
            heat_pressure_w: 0.8,
            harvest_dread_max: 0.6,
            acquire_honour_min: 0.3,
            bind_dread_w: 0.5,
        }
    }
}

/// M15 § 2: appearance and taste per audience (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TasteCfg {
    pub chrome_ref: u8,
    pub first_look_w: f32,
    pub meet_gap: f32,
    pub meet_taste_w: f32,
    pub corp: crate::word::Taste,
    pub street: crate::word::Taste,
    pub dreg: crate::word::Taste,
    pub gang: crate::word::Taste,
    pub purist: crate::word::Taste,
}

impl Default for TasteCfg {
    fn default() -> Self {
        TasteCfg::off()
    }
}

impl TasteCfg {
    pub fn off() -> TasteCfg {
        use crate::word::Taste;
        let t = |dress, chrome, own_colours, rival_colours| Taste { dress, chrome, own_colours, rival_colours };
        TasteCfg {
            chrome_ref: 3,
            first_look_w: 0.1,
            meet_gap: 0.5,
            meet_taste_w: 0.3,
            corp: t(0.6, 0.0, 0.3, -0.3),
            street: t(0.1, 0.2, 0.2, -0.2),
            dreg: t(-0.2, 0.1, 0.0, 0.0),
            gang: t(0.0, 0.3, 0.5, -0.8),
            purist: t(-0.2, -1.0, 0.5, -0.5),
        }
    }
}

/// M15 § 2: the Purist creed (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreedsCfg {
    pub seed_purist: bool,
    pub purist_name: String,
    pub purist_treasury: i64,
    pub creed_tolerance: u8,
    pub purist_chrome: f32,
    pub tithe_frac: f32,
    /// Plan deviation (phase 2 review): a creed spreads through people. A
    /// Purist gang takes a recruit only along a Friend, Family, Parent or
    /// Spouse edge to a member, except its first `founders` members, who
    /// may come as any gang's bootstrap recruits do (nearest Hideout).
    pub founders: usize,
}

impl Default for CreedsCfg {
    fn default() -> Self {
        CreedsCfg::off()
    }
}

impl CreedsCfg {
    pub fn off() -> CreedsCfg {
        CreedsCfg {
            seed_purist: false,
            purist_name: "The Unplugged".to_string(),
            purist_treasury: 50,
            creed_tolerance: 0,
            purist_chrome: 0.3,
            tithe_frac: 0.6,
            founders: 3,
        }
    }
}

/// M15 § 3 `rel_w` plus plan rows `leader`, `comrade` (W15).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RelWeights {
    pub spouse: f32,
    pub parent: f32,
    pub family: f32,
    pub friend: f32,
    pub own: f32,
    pub leader: f32,
    pub comrade: f32,
}

impl Default for RelWeights {
    fn default() -> Self {
        RelWeights { spouse: 1.0, parent: 1.0, family: 0.9, friend: 0.6, own: 0.8, leader: 0.7, comrade: 0.0 }
    }
}

/// M15 § 3: grudges (phase 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrudgesCfg {
    pub grudge_min: f32,
    pub grudge_decay: f32,
    pub beat_settle: f32,
    pub inherit_min: f32,
    pub inherit_frac: f32,
    pub settle_keep_days: u32,
    pub rel_w: RelWeights,
    pub guard_hours: u32,
    pub vendetta_norm: f32,
    pub vendetta_open: f32,
    pub vendetta_close: f32,
    /// W17.
    pub fight_grudge_min: f32,
    /// M15 phase 5: days a god `DeclareVendetta` holds the feud open.
    pub declared_days: u32,
}

impl Default for GrudgesCfg {
    fn default() -> Self {
        GrudgesCfg::off()
    }
}

impl GrudgesCfg {
    pub fn off() -> GrudgesCfg {
        GrudgesCfg {
            grudge_min: 0.15,
            grudge_decay: 0.01,
            beat_settle: 0.5,
            inherit_min: 0.4,
            inherit_frac: 0.6,
            settle_keep_days: 30,
            rel_w: RelWeights::default(),
            guard_hours: 6,
            vendetta_norm: 3.0,
            vendetta_open: 0.5,
            vendetta_close: 0.2,
            fight_grudge_min: 0.5,
            declared_days: 30,
        }
    }
}

/// M15 § 4: the Hunt (phase 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HuntCfg {
    pub enabled: bool,
    pub hunt_min: f32,
    pub max_hunts: usize,
    pub hunt_flat: f32,
    pub fresh_sighting_hours: u32,
    pub intel_k: f32,
    pub stakeout_hours: u32,
    pub lethal_min: f32,
    pub hunt_kill_p: f32,
    pub hunt_cooldown_days: u32,
    pub hunt_days: u32,
    pub street_silence: bool,
    /// W20.
    pub hold_score: f32,
    /// W23.
    pub stat_hunt_min: f32,
}

impl Default for HuntCfg {
    fn default() -> Self {
        HuntCfg::off()
    }
}

impl HuntCfg {
    pub fn off() -> HuntCfg {
        HuntCfg {
            enabled: false,
            hunt_min: 0.5,
            max_hunts: 16,
            hunt_flat: 0.0,
            fresh_sighting_hours: 48,
            intel_k: 0.5,
            stakeout_hours: 3,
            lethal_min: 0.75,
            hunt_kill_p: 0.5,
            hunt_cooldown_days: 3,
            hunt_days: 10,
            street_silence: true,
            hold_score: 0.6,
            stat_hunt_min: 0.2,
        }
    }
}

/// M15 § 5: social skills (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsCfg {
    pub skill_scale: f32,
    pub rarity_exp: f32,
    pub inherit_skill: f32,
    pub skill_drift: f32,
    pub skill_rust: f32,
    pub knowledge_work: f32,
    /// W25.
    pub secondary_scale: f32,
}

impl Default for SkillsCfg {
    fn default() -> Self {
        SkillsCfg::off()
    }
}

impl SkillsCfg {
    pub fn off() -> SkillsCfg {
        SkillsCfg {
            skill_scale: 1.0,
            rarity_exp: 4.0,
            inherit_skill: 0.5,
            skill_drift: 0.005,
            skill_rust: 0.001,
            knowledge_work: 0.0005,
            secondary_scale: 0.6,
        }
    }
}

/// M15 § 6: the move kinds' biases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoveBias {
    pub persuade: f32,
    pub intimidate: f32,
    pub deceive: f32,
    pub charm: f32,
}

impl Default for MoveBias {
    fn default() -> Self {
        MoveBias { persuade: 0.0, intimidate: 0.8, deceive: 0.0, charm: 0.2 }
    }
}

/// M15 § 6: the social move (phase 2; `contradict_conf` is read from phase 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MovesCfg {
    pub enabled: bool,
    pub move_k: f32,
    pub bias: MoveBias,
    pub w_m: f32,
    pub w_r: f32,
    pub w_l: f32,
    pub p_min: f32,
    pub p_max: f32,
    pub chrome_might: f32,
    pub ally_might: f32,
    pub ally_cap: f32,
    pub backlash_courage: f32,
    pub backlash_fight: f32,
    pub contradict_conf: f32,
}

impl Default for MovesCfg {
    fn default() -> Self {
        MovesCfg::off()
    }
}

impl MovesCfg {
    pub fn off() -> MovesCfg {
        MovesCfg {
            enabled: false,
            move_k: 4.0,
            bias: MoveBias::default(),
            w_m: 0.5,
            w_r: 0.6,
            w_l: 0.3,
            p_min: 0.05,
            p_max: 0.95,
            chrome_might: 0.2,
            ally_might: 0.1,
            ally_cap: 0.5,
            backlash_courage: 0.6,
            backlash_fight: 0.3,
            contradict_conf: 0.3,
        }
    }
}

/// M15 § 6: competence and poaching (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompetenceCfg {
    pub enabled: bool,
    pub exec_w: f32,
    pub comp_w: f32,
    pub comp_ref: f32,
    pub comp_min: f32,
    pub comp_max: f32,
    pub talent_drop: f32,
    pub poach_min: f32,
    pub poach_gap: f32,
    pub poach_premium: f32,
    /// Phase 2 review: a skill's competence term is `2 ×` its percentile
    /// among the city's adults at seed (the median corp at multiplier 1);
    /// false: the plan's `skill ÷ city mean` (W28; the median corp near 0.85
    /// of `comp_ref`, the exec term being a heavy-tailed skill).
    pub rank_norm: bool,
}

impl Default for CompetenceCfg {
    fn default() -> Self {
        CompetenceCfg::off()
    }
}

impl CompetenceCfg {
    pub fn off() -> CompetenceCfg {
        CompetenceCfg {
            enabled: false,
            exec_w: 0.4,
            comp_w: 1.0,
            comp_ref: 0.25,
            comp_min: 0.75,
            comp_max: 1.35,
            talent_drop: 0.05,
            poach_min: 0.5,
            poach_gap: 0.2,
            poach_premium: 1.3,
            rank_norm: true,
        }
    }
}

/// M15 § 7: Feeds, stories and Spin (phase 4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewsCfg {
    pub enabled: bool,
    pub reach_base: f32,
    pub reach_per_reporter: f32,
    pub story_reach: f32,
    pub stories_per_day: u8,
    pub ad_rate: i64,
    pub plant_price: i64,
    pub bury_price: i64,
    pub bury_days: u32,
    pub press_w: f32,
    pub press_loyalty: f32,
    pub press_cap: f32,
    pub censor_lawfulness: f32,
    /// Plan key (W36): `Register`'s per-capita target for a Feed
    /// (`founding::choose_kind`), as `residents_per_bar` for a Bar.
    pub residents_per_feed: u32,
    /// Orchestrator deviation (W40): Spin is a side spend; a corp spins
    /// while its Spin score (the spec's considerations plus
    /// `order_flat.spin`) is at least this.
    pub spin_min: f32,
}

impl Default for NewsCfg {
    fn default() -> Self {
        NewsCfg::off()
    }
}

impl NewsCfg {
    pub fn off() -> NewsCfg {
        NewsCfg {
            enabled: false,
            reach_base: 0.2,
            reach_per_reporter: 0.15,
            story_reach: 0.8,
            stories_per_day: 3,
            ad_rate: 3,
            plant_price: 120,
            bury_price: 60,
            bury_days: 3,
            press_w: 0.3,
            press_loyalty: 0.1,
            press_cap: 0.15,
            censor_lawfulness: 0.3,
            residents_per_feed: 1000,
            spin_min: 0.2,
        }
    }
}

/// Life pass L1 (docs/SHADOW_V1.md "Plan"): the cross-cutting causes that
/// made every shadowed resident's day a walk. `enabled = false` is the
/// master switch: every L1 branch reads it, so off is the ab79188 city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LifeCfg {
    pub enabled: bool,
    /// A discretionary goal's travel factor: `1 / (1 + walk / travel_half_ticks)`
    /// (a walk this long halves the score), floored at `travel_min`.
    pub travel_half_ticks: f32,
    pub travel_min: f32,
    /// A Sleep is not cut by the commute gate before this many ticks.
    pub sleep_min_ticks: u64,
    /// Below this energy Sleep takes `exhausted_flat` and may lie down
    /// where the agent stands when every bed is past `rough_min_tiles`.
    pub exhausted_energy: f32,
    pub exhausted_flat: f32,
    /// Sleep's flat at Night for anyone off shift (with energy below
    /// `ENERGY_SATISFIED`: the goal is skipped above it).
    pub night_sleep_flat: f32,
    pub rough_min_tiles: u32,
    /// A housed agent's Sleep phase factor outside the Night and Evening
    /// (was 0.4 for everyone).
    pub housed_day_sleep: f32,
    /// A second bed (the Hideout, a Hotel) is taken when it is at least
    /// this many tiles nearer than Home.
    pub bed_margin_tiles: u32,
    /// Coins a housed agent keeps after a Hotel night (in meals).
    pub hotel_reserve_meals: i64,
    /// The homeless's Hotel reach (was `[street] hotel_reach`).
    pub hotel_reach_homeless: u32,
    /// The daily dole is paid where the agent is at 09:00 (no Hall walk).
    pub dole_in_place: bool,
    /// Days of dole paid at one Hall visit (accrued since the last one);
    /// 1 is the daily dole (off). Read only with `dole_in_place` off.
    pub dole_bulk_days: u64,
    /// The Hall is visited for the dole only with this many days accrued,
    /// unless the agent cannot buy a meal or the Hall is within `near_tiles`.
    pub dole_trip_days: u64,
    pub near_tiles: u32,
    /// An idle housed agent this far from Home idles where it is (or at
    /// its gang's nearer Hideout) instead of walking home to sit down.
    pub idle_far_tiles: u32,
    /// Arrest's "in shift" factor off shift (0: on shift only; was 0.5).
    pub arrest_off_shift: f32,
    /// A guard re-chases a suspect seen moving at most this many times per plan.
    pub chase_hops: u8,
    /// A suspect seen longer ago than this is chased only within `near_tiles`.
    pub arrest_fresh_ticks: u64,
    /// One `SawCrime` per (witness, actor, crime) within this many ticks.
    pub witness_dedupe_ticks: u64,
    /// New edges one arrival makes in a building (a Jail arrival: `jail_meet_max`).
    pub colocation_new_edges: usize,
    pub jail_meet_max: usize,
    /// In the cells an arrival also meets `jail_meet_max` gang members, and
    /// a member's pairs keep the cells' drift: the Jail is where gangs recruit.
    pub jail_pitch: bool,
    /// A gang order holds this long against a shock rescore unless the
    /// pending shocks reach `order_severe` (Retaliate and BreakOut exempt).
    pub order_dwell_ticks: u64,
    pub order_severe: f32,
    /// An employed adult commuting past this many tiles may move nearer.
    pub commute_cap_tiles: u32,
    pub relocate_per_day: usize,
    /// An employer does not rehire an agent who quit it for this many days.
    pub quit_rehire_days: u64,
    /// Execs: the youngest pick, the share of the corp's treasury paid as a
    /// day's salary (floored at `[economy] wage_exec`) and its cap, and the
    /// office shift at the corp's HQ.
    pub exec_min_age_years: f32,
    /// The exec is the greediest of the wealthiest tenth (else the wealthiest).
    pub exec_greed: bool,
    pub exec_pay_frac: f32,
    pub exec_pay_cap: i64,
    pub exec_shift: (u16, u16),
    /// Scavenge: coins the Recycler pays per haul (Treasury, `Flow::Sanitation`),
    /// found on `scavenge_p` of hours.
    pub scavenge_coins: i64,
    pub scavenge_p: f32,
    /// An escort's walk to the Precinct is a van ride of at most this long.
    pub escort_van_ticks: u64,
    /// L2 (L29, `[lod] budget`): after this many dry Scavenge hours in a row
    /// `Earn` cools for `scavenge_cool_hours`.
    pub scavenge_dry_max: u8,
    pub scavenge_cool_hours: u64,
    /// L2 shadow fixes (docs/SHADOW_V2.md "L2 shadow fixes (what landed)"):
    /// the master switch of the 21 `[bug]` items. Off (`LifeCfg::off`, a
    /// pre-fix save, `--life-off`, and `--l2-off` through
    /// `Config::living_off`) is the 56f3110 city byte for byte. The
    /// sub-switches below are read only with it on.
    pub l2_fixes: bool,
    /// Item 1: a commute once started holds until arrival (the Work plan
    /// owns the walk for its shift, as `raid_plan` owns its chain), and the
    /// leave-for-work gate reads the walk at the real speed.
    pub commute_latch: bool,
    /// Item 2: an in-shift work step is not outbid (only starvation below
    /// `starving_hunger` or danger interrupts) ...
    pub shift_commit: bool,
    /// ... and a shift cut short pays pro rata from this share of it.
    pub shift_pro_rata: f32,
    pub starving_hunger: f32,
    /// Item 3: a Sleep is held from its first tick (starvation or danger
    /// excepted) and runs to `sleep_wake_energy` (it ended at 0.9 and Idle
    /// planned the next a few minutes later: 1-6 min Sleeps all night);
    /// Idle plans Sleep only below `idle_sleep_energy`; Unwind, Eat and
    /// Earn carry an energy term below `energy_brake`.
    pub sleep_commit: bool,
    pub sleep_wake_energy: f32,
    pub idle_sleep_energy: f32,
    pub energy_brake: f32,
    /// Item 5: the free spot's walk cap (else the nearest spot), the lone
    /// hour's share of a HangOut's fun, the penalty per Enemy near a spot.
    pub spot_walk_cap_tiles: u32,
    pub spot_lone_fun: f32,
    pub spot_enemy_penalty: f32,
    /// Item 6: a fled-from tile is avoided by the spot pick this long, and
    /// Unwind and Socialise cool this long after the flight.
    pub avoid_spot_ticks: u64,
    pub flee_clear_ticks: u64,
    /// Item 7: an agent keeps its tier at least this long (an equal-priority
    /// newcomer waits), and a demotion waits for a paid step to end.
    pub lod_dwell: bool,
    pub lod_dwell_ticks: u64,
    /// Item 8: a Statistical hungry hour whose Eat fails sleeps at night.
    pub stat_sleep_futile_eat: bool,
    /// Item 10: guard-prisoner affinity in the cells stops at this.
    pub jail_affinity_cap: f32,
    /// Item 11: an employee who has missed this many workdays draws the
    /// dole, and is dismissed at `noshow_fire_days`.
    pub noshow_dole_days: i64,
    pub noshow_fire_days: i64,
    /// Item 15: a Muster is held for the farthest member's walk (capped).
    pub muster_walk_cap_ticks: u64,
    /// Item 16: a Deal shift starts only from `deal_busy_from` to
    /// `deal_busy_to` (ticks of day, wrapping past midnight).
    pub deal_busy_from: u16,
    pub deal_busy_to: u16,
    /// Item 17: a teller does not tell the same listener the same deed
    /// again within this many days.
    pub told_cooldown_days: u64,
    /// Item 18: no freelance run where the target's ICE beats the attack by
    /// `[ice] flatline_gap` unless `U(wealth)` is at least this.
    pub hack_gap_wealth: f32,
    /// Item 19: Beg at a HangOut spot with company.
    pub beg_at_spot: bool,
}

impl Default for LifeCfg {
    fn default() -> Self {
        LifeCfg::off()
    }
}

impl LifeCfg {
    pub fn off() -> LifeCfg {
        LifeCfg {
            enabled: false,
            travel_half_ticks: 120.0,
            travel_min: 0.15,
            sleep_min_ticks: 240,
            exhausted_energy: 0.12,
            exhausted_flat: 0.6,
            night_sleep_flat: 0.2,
            rough_min_tiles: 45,
            housed_day_sleep: 0.25,
            bed_margin_tiles: 20,
            hotel_reserve_meals: 2,
            hotel_reach_homeless: 120,
            dole_in_place: true,
            dole_bulk_days: 1,
            dole_trip_days: 1,
            near_tiles: 30,
            idle_far_tiles: 30,
            arrest_off_shift: 0.0,
            chase_hops: 3,
            arrest_fresh_ticks: 120,
            witness_dedupe_ticks: 360,
            colocation_new_edges: 2,
            jail_meet_max: 2,
            jail_pitch: true,
            order_dwell_ticks: 1440,
            order_severe: 1.0,
            commute_cap_tiles: 30,
            relocate_per_day: 8,
            quit_rehire_days: 7,
            exec_min_age_years: 30.0,
            exec_greed: true,
            exec_pay_frac: 0.002,
            exec_pay_cap: 40,
            exec_shift: (540, 1020),
            scavenge_coins: 1,
            scavenge_p: 0.15,
            escort_van_ticks: 60,
            scavenge_dry_max: 3,
            scavenge_cool_hours: 4,
            l2_fixes: false,
            commute_latch: true,
            shift_commit: true,
            shift_pro_rata: 0.5,
            starving_hunger: 0.1,
            sleep_commit: true,
            sleep_wake_energy: 0.99,
            idle_sleep_energy: 0.9,
            energy_brake: 0.5,
            spot_walk_cap_tiles: 22,
            spot_lone_fun: 0.4,
            spot_enemy_penalty: 0.5,
            avoid_spot_ticks: 1440,
            flee_clear_ticks: 30,
            lod_dwell: true,
            lod_dwell_ticks: 180,
            stat_sleep_futile_eat: true,
            jail_affinity_cap: 0.3,
            noshow_dole_days: 2,
            noshow_fire_days: 7,
            muster_walk_cap_ticks: 480,
            deal_busy_from: 720,
            deal_busy_to: 120,
            told_cooldown_days: 3,
            hack_gap_wealth: 0.9,
            beg_at_spot: true,
        }
    }
}

/// L2 (plan L5): the master switch of the living city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivingCfg {
    pub enabled: bool,
    /// L2 (coordinator ruling on M14, 2026-10-08): a corp keeps its standing
    /// order this many days after taking it against a shock rescore ...
    pub corp_order_dwell_days: u64,
    /// ... unless the challenger beats it by this margin, or the pending
    /// shocks reach `[life] order_severe` (the gangs' dwell rule, L1).
    pub corp_order_margin: f32,
    /// L2 phase 5 (gang desistance, `gang::desist`): an ordinary member's
    /// daily chance to walk away, times the terms below (0: no desistance).
    pub desist_base: f32,
    /// x this while the member holds a Job that has paid a wage.
    pub desist_employed: f32,
    /// x `desist_unpaid` when no stipend or tribute reached it for this many days.
    pub desist_unpaid_days: u64,
    pub desist_unpaid: f32,
    /// x this when married or housed outside the Sump districts.
    pub desist_settled: f32,
    /// x `desist_aged` from this age (sim years).
    pub desist_age: u32,
    pub desist_aged: f32,
    /// x this when released from a sentence in the last 30 days.
    pub desist_jailed: f32,
    /// x this (a brake) with an unsettled grudge on a rival gang's member or
    /// the gang in an open vendetta.
    pub desist_feud: f32,
    /// x this (a brake) for a member the gang has invested in or earns
    /// through: the keeper of an asset its gang owns (Arms, a deck), or a
    /// deal in the last 7 days.
    pub desist_invested: f32,
    /// A leaver's `JoinGang` cooldown, days.
    pub desist_cooldown_days: u64,
    /// L2 phase 5 (`economy::reserve_release`): a Market priced above this
    /// and under `restock_floor` gets Reserve food free (0: off) ...
    pub reserve_release_price: i64,
    /// ... up to this many units a day.
    pub reserve_release_batch: u32,
}

impl Default for LivingCfg {
    fn default() -> Self {
        LivingCfg::off()
    }
}

impl LivingCfg {
    pub fn off() -> LivingCfg {
        LivingCfg {
            enabled: false,
            corp_order_dwell_days: 3,
            corp_order_margin: 0.05,
            desist_base: 0.0,
            desist_employed: 4.0,
            desist_unpaid_days: 14,
            desist_unpaid: 3.0,
            desist_settled: 2.0,
            desist_age: 35,
            desist_aged: 2.0,
            desist_jailed: 2.0,
            desist_feud: 0.25,
            desist_invested: 0.25,
            desist_cooldown_days: 60,
            reserve_release_price: 0,
            reserve_release_batch: 200,
        }
    }
}

/// L2 § 1: the venues and Fabs seeded at day 0 (`jobs::seed_venues`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeedVenuesCfg {
    pub club: u32,
    pub arcade: u32,
    pub noodle_bar: u32,
    pub fight_pit: u32,
    pub den: u32,
    pub lounge: u32,
    pub fab: u32,
}

impl Default for SeedVenuesCfg {
    fn default() -> Self {
        SeedVenuesCfg { club: 3, arcade: 3, noodle_bar: 10, fight_pit: 2, den: 2, lounge: 1, fab: 2 }
    }
}

impl SeedVenuesCfg {
    pub fn count(&self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::Club => self.club,
            BuildingKind::Arcade => self.arcade,
            BuildingKind::NoodleBar => self.noodle_bar,
            BuildingKind::FightPit => self.fight_pit,
            BuildingKind::Den => self.den,
            BuildingKind::Lounge => self.lounge,
            BuildingKind::Fab => self.fab,
            _ => 0,
        }
    }
}

/// L2 § 1 `[jobs]` (plan L5: `LivingJobsCfg`; `JobsCfg` is `[world.jobs]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivingJobsCfg {
    pub enabled: bool,
    pub seed_venues: SeedVenuesCfg,
    /// Of `found_cost`: a refit of a derelict Block.
    pub refit_frac: f32,
    pub fab_yield: f32,
    pub fab_skill_floor: f32,
    pub fab_skill_slope: f32,
    /// Coins imported in 14 days by a Tech corp's own sellers before its
    /// `Grow` builds a Fab.
    pub fab_import_trigger: i64,
    pub scrap_per_part: u32,
    /// Litter units a sweeper clears per hour of `Sweep`.
    pub sweep_per_hour: u32,
    /// Plan L3: a Market's full staff with jobs on (`[buildings]
    /// market.staff` stays the off value).
    pub market_staff: u32,
    /// Plan L3: a Bar's.
    pub bar_staff: u32,
    /// Plan L8: Sanitation sweeps its beat (false: TendGraves and the D23 ledger).
    pub sweep: bool,
}

impl Default for LivingJobsCfg {
    fn default() -> Self {
        LivingJobsCfg::off()
    }
}

impl LivingJobsCfg {
    pub fn off() -> LivingJobsCfg {
        LivingJobsCfg {
            enabled: false,
            seed_venues: SeedVenuesCfg::default(),
            refit_frac: 0.5,
            fab_yield: 0.6,
            fab_skill_floor: 0.6,
            fab_skill_slope: 0.8,
            fab_import_trigger: 300,
            scrap_per_part: 4,
            sweep_per_hour: 6,
            market_staff: 12,
            bar_staff: 5,
            sweep: true,
        }
    }
}

/// L2 § 1: a venue's price by tier (Sump, Mid, Spire); 0 = not sold there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PriceBaseCfg {
    pub club: [i64; 3],
    pub arcade: [i64; 3],
    /// A NoodleBar meal: the Market's price plus this.
    pub noodle_bar_markup: i64,
    pub fight_pit: [i64; 3],
    pub den: [i64; 3],
    pub lounge: [i64; 3],
}

impl Default for PriceBaseCfg {
    fn default() -> Self {
        PriceBaseCfg {
            club: [0, 5, 12],
            arcade: [2, 3, 6],
            noodle_bar_markup: 1,
            fight_pit: [3, 0, 0],
            den: [2, 3, 0],
            lounge: [0, 0, 25],
        }
    }
}

impl PriceBaseCfg {
    /// The tier price of a leisure kind (a NoodleBar's is its markup).
    pub fn of(&self, kind: BuildingKind, tier: u8) -> i64 {
        let t = usize::from(tier.min(2));
        match kind {
            BuildingKind::Club => self.club[t],
            BuildingKind::Arcade => self.arcade[t],
            BuildingKind::NoodleBar => self.noodle_bar_markup,
            BuildingKind::FightPit => self.fight_pit[t],
            BuildingKind::Den => self.den[t],
            BuildingKind::Lounge => self.lounge[t],
            _ => 0,
        }
    }
}

/// L2 § 2: the fun a satisfier gives (plan keys; the spec's leisure-kinds
/// table and rung ladder).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FunGainCfg {
    pub club: f32,
    /// A Club priced at the Spire tier (the high rung).
    pub club_spire: f32,
    pub arcade: f32,
    pub noodle_bar: f32,
    pub fight_pit: f32,
    pub den: f32,
    pub lounge: f32,
    /// A drink (a Bar, a Club, a Den).
    pub drink: f32,
    /// An hour of HangOut.
    pub hangout_hour: f32,
    /// Watching a raid or a bout from the door (an hour).
    pub watch: f32,
    /// The free rung off screen (`stat_daily`).
    pub free_stat: f32,
}

impl Default for FunGainCfg {
    fn default() -> Self {
        FunGainCfg {
            club: 0.6,
            club_spire: 0.7,
            arcade: 0.35,
            noodle_bar: 0.1,
            fight_pit: 0.5,
            den: 0.5,
            lounge: 0.9,
            drink: 0.1,
            hangout_hour: 0.1,
            watch: 0.3,
            free_stat: 0.3,
        }
    }
}

/// L2 § 1-2 `[leisure]`. Phase 1 reads the price table and the front
/// markup; phase 2 owns `enabled` and the rest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LeisureCfg {
    pub enabled: bool,
    pub price_base: PriceBaseCfg,
    pub front_markup: f32,
    // --- phase 2 (spec § 1-2 and plan keys).
    pub house_edge: f32,
    pub stake_frac: f32,
    pub stake_cap: i64,
    /// A win above this posts `Gambled`.
    pub big_win: i64,
    pub bout_hour: u16,
    /// A Club's door Host refuses a visitor with `rep.heat` at least this
    /// when the Host's courage beats the heat.
    pub door_heat: f32,
    /// Corp, Street, Dreg.
    pub class_mult: [f32; 3],
    pub hangout_ticks: u64,
    /// Socialise's "no drink possible" clause is dropped with a spot this near.
    pub hangout_reach: u32,
    pub barrels_per_sump: u32,
    pub kin_w: f32,
    pub watch_tiles: u32,
    pub fronts_max: u32,
    pub crackdown_close_days: u64,
    /// 0 = day 0 of the week (`day % 7`).
    pub collect_weekday: u64,
    pub tribute_share: f32,
    pub call_tiles: u32,
    pub call_hours: u64,
    pub preach_share: f32,
    pub preach_opinion: f32,
    /// Plan key (L20): the Hideout spot's score bonus during a Call.
    pub call_w: f32,
    /// Plan key: courage above which the mid rung's FightPit and Den
    /// outscore the Club (`(0.5 + courage)` below it is halved).
    pub gamble_rung_courage: f32,
    /// Plan key: the fun each satisfier gives.
    pub gain: FunGainCfg,
    /// Plan key (deviation): the evening shift of the Hosts, Fighters,
    /// Croupiers and Concierges (so a bout at `bout_hour` has Fighters on
    /// shift); Attendants and Cooks keep `[world] shift_day`.
    pub evening_shift: Vec<(u16, u16)>,
    /// Plan key: hours before `bout_hour` a FightPit takes bets.
    pub bet_window_hours: u16,
}

impl Default for LeisureCfg {
    fn default() -> Self {
        LeisureCfg::off()
    }
}

impl LeisureCfg {
    pub fn off() -> LeisureCfg {
        LeisureCfg {
            enabled: false,
            price_base: PriceBaseCfg::default(),
            front_markup: 1.2,
            house_edge: 0.08,
            stake_frac: 0.3,
            stake_cap: 20,
            big_win: 40,
            bout_hour: 23,
            door_heat: 0.6,
            class_mult: [1.3, 1.0, 0.8],
            hangout_ticks: 60,
            hangout_reach: 40,
            barrels_per_sump: 3,
            kin_w: 0.1,
            watch_tiles: 12,
            fronts_max: 2,
            crackdown_close_days: 3,
            collect_weekday: 6,
            tribute_share: 0.5,
            call_tiles: 120,
            call_hours: 3,
            preach_share: 0.5,
            preach_opinion: 0.05,
            call_w: 2.0,
            gamble_rung_courage: 0.5,
            gain: FunGainCfg::default(),
            evening_shift: vec![(1080, 1440)],
            bet_window_hours: 2,
        }
    }
}

/// L2 § 1 `[budget]`: the Treasury's band and public works.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetCfg {
    pub enabled: bool,
    /// `[lo, hi]` Treasury coins.
    pub band: [i64; 2],
    /// Plan key: days in a row on one side before the band acts (spec 3).
    pub band_hold_days: u16,
    pub works_step: u16,
    pub works_max: u16,
    pub upkeep_step: f32,
    pub upkeep_floor: f32,
    /// L2 phase 5 (plan key): a public-works hire's daily wage; 0 keeps
    /// `[economy] wage_sanitation` (the M12 sweeper's wage).
    pub works_wage: i64,
}

impl Default for BudgetCfg {
    fn default() -> Self {
        BudgetCfg::off()
    }
}

impl BudgetCfg {
    pub fn off() -> BudgetCfg {
        BudgetCfg {
            enabled: false,
            band: [30_000, 60_000],
            band_hold_days: 3,
            works_step: 10,
            works_max: 120,
            upkeep_step: 0.05,
            upkeep_floor: 0.3,
            works_wage: 0,
        }
    }
}

/// L2 § 1 the export hook's per-good numbers (Food, Parts, Data).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportGoodsCfg {
    pub food: i64,
    pub parts: i64,
    pub data: i64,
}

/// L2 § 1 `[export]`: the World account buys a slice of Food, Parts, Data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportCfg {
    pub enabled: bool,
    pub treasury_ref: i64,
    pub cap_per_day: ExportGoodsCfg,
    pub price: ExportGoodsCfg,
    /// A corp Farm's food kept back from export.
    pub export_floor: u32,
    /// Plan key: a Fab's Parts kept back.
    pub parts_floor: u32,
    /// Plan key (deviation: M14 has no Data sell floor): a Lab's Data kept back.
    pub data_floor: u32,
}

impl Default for ExportCfg {
    fn default() -> Self {
        ExportCfg::off()
    }
}

impl ExportCfg {
    pub fn off() -> ExportCfg {
        ExportCfg {
            enabled: false,
            treasury_ref: 20_000,
            cap_per_day: ExportGoodsCfg { food: 50, parts: 20, data: 5 },
            price: ExportGoodsCfg { food: 3, parts: 18, data: 40 },
            export_floor: 200,
            parts_floor: 20,
            data_floor: 100,
        }
    }
}

/// L2 § 3 `[fviolence]` (phase 4): the ledger's victim side and the daily
/// pass that applies it to the Statistical tier. Every number a placeholder
/// (phase 5 calibrates; `fv_mult` is the first knob).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FviolenceCfg {
    pub enabled: bool,
    /// Days of the cells' windows the rates read (at most `ledger::RATE_DAYS`).
    pub rate_days: usize,
    /// Body-days of evidence the prior is worth.
    pub prior_weight: f32,
    pub fv_mult: f32,
    pub day_cap: FvKindsCfg,
    pub prior: FvPriorCfg,
    /// L2 phase 5: a prior per victim class, as a multiplier per kind
    /// (Killed, Assaulted, Robbed, Abducted) on every source's prior.
    pub class_mult: FvClassCfg,
}

/// L2 phase 5 (calibration (b)): per `VictimClass`, the per-kind
/// multipliers of the source priors (1.0: the spec's priors as they are).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvClassCfg {
    pub civilian: [f32; 4],
    pub member: [f32; 4],
    pub watch: [f32; 4],
}

impl Default for FvClassCfg {
    fn default() -> Self {
        FvClassCfg { civilian: [1.0; 4], member: [1.0; 4], watch: [1.0; 4] }
    }
}

/// Per-kind day caps, city-wide (Killed, Assaulted, Robbed, Abducted).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvKindsCfg {
    pub killed: u32,
    pub assaulted: u32,
    pub robbed: u32,
    pub abducted: u32,
}

impl Default for FvKindsCfg {
    fn default() -> Self {
        FvKindsCfg { killed: 3, assaulted: 12, robbed: 20, abducted: 2 }
    }
}

impl FvKindsCfg {
    /// In the ledger's order: Killed, Assaulted, Robbed, Abducted.
    pub fn get(&self, k: usize) -> u32 {
        [self.killed, self.assaulted, self.robbed, self.abducted][k.min(3)]
    }
}

/// The per-source priors per agent-day, `[Killed, Assaulted, Robbed,
/// Abducted]`; `harvest_abducted` is `Order(Harvest)`'s Abducted prior.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvPriorCfg {
    pub order: [f32; 4],
    pub vendetta: [f32; 4],
    pub riot: [f32; 4],
    pub episode: [f32; 4],
    pub harvest_abducted: f32,
}

impl Default for FvPriorCfg {
    fn default() -> Self {
        FvPriorCfg {
            order: [0.0003, 0.002, 0.003, 0.0],
            vendetta: [0.0004, 0.002, 0.0, 0.0],
            riot: [0.001, 0.01, 0.01, 0.0],
            episode: [0.0005, 0.003, 0.0, 0.0],
            harvest_abducted: 0.0004,
        }
    }
}

impl Default for FviolenceCfg {
    fn default() -> Self {
        FviolenceCfg::off()
    }
}

impl FviolenceCfg {
    pub fn off() -> FviolenceCfg {
        FviolenceCfg {
            enabled: false,
            rate_days: 14,
            prior_weight: 48.0,
            fv_mult: 1.0,
            day_cap: FvKindsCfg::default(),
            prior: FvPriorCfg::default(),
            class_mult: FvClassCfg::default(),
        }
    }
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
        self.vehicles = VehiclesCfg::off();
        self.shop = ShopCfg::off();
        self.stims = StimsCfg::off();
        self.robots = RobotsCfg::off();
        // L1: the life pass is the 2,000 city's (the v1 gates keep v1 days),
        // and so is L1b's food price (the v1 economy prices at 3).
        self.life = LifeCfg::off();
        self.economy.price_base = 3.0;
        // M14 V44: no Virt plane, Labs, ICE, tech caps; M15 W44: no word;
        // L2 (plan L5): no living city.
        self.virt_off().word_off().living_off()
    }

    /// M14 (plan V44, V46): every M14 section `off()`: no relink, runs, ICE
    /// upkeep, Lab production, research upkeep or tier cap (`--virt-off`).
    pub fn virt_off(mut self) -> Config {
        self.virt = VirtCfg::off();
        self.decks = DecksCfg::off();
        self.ice = IceCfg::off();
        self.data = DataCfg::off();
        self.tech = TechCfg::off();
        self.hack = HackCfg::off();
        self.db = DbCfg::off();
        self
    }

    /// M15 (plan W44, W46): every M15 section `off()`: no pools, exchange,
    /// hearing, kin channel or reputation, and today's `social::gossip`
    /// (`--word-off`).
    pub fn word_off(mut self) -> Config {
        self.gossip = GossipCfg::off();
        self.reputation = ReputationCfg::off();
        self.taste = TasteCfg::off();
        self.creeds = CreedsCfg::off();
        self.grudges = GrudgesCfg::off();
        self.hunt = HuntCfg::off();
        self.skills = SkillsCfg::off();
        self.moves = MovesCfg::off();
        self.competence = CompetenceCfg::off();
        self.news = NewsCfg::off();
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
        c.vehicles = VehiclesCfg::off();
        c.shop = ShopCfg::off();
        c.stims = StimsCfg::off();
        c.robots = RobotsCfg::off();
        // M14 V44: the parity table never saw a Researcher or a runner;
        // M15 W44: nor a rumour; L2 (plan L17): nor a venue.
        c.virt_off().word_off().living_off()
    }

    /// L2 (plan L17): the leisure calibration city: `[living]`, `[jobs]`
    /// and `[leisure]` on (their spec values), the band, the export and
    /// the rest of L2 off; venues seed on the city's deed (no corps).
    pub fn with_leisure(mut self) -> Config {
        self.living.enabled = true;
        self.jobs.enabled = true;
        self.leisure.enabled = true;
        self.budget = BudgetCfg::off();
        self.export = ExportCfg::off();
        self
    }

    /// L2 (plan L5, L32): every L2 section `off()` (`--l2-off`): no
    /// venues, Fabs, staffing overrides, Sweep, band or export, and (phase
    /// 3) no LOD budget.
    pub fn living_off(mut self) -> Config {
        self.lod.budget = false;
        // The L2 shadow fixes ride `[life]` but are L2's: off with it.
        self.life.l2_fixes = false;
        self.living = LivingCfg::off();
        self.jobs = LivingJobsCfg::off();
        self.leisure = LeisureCfg::off();
        self.budget = BudgetCfg::off();
        self.export = ExportCfg::off();
        self.fviolence = FviolenceCfg::off();
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
