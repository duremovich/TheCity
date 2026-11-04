use super::*;

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
pub struct NeedsInitialCfg {
    pub hunger: f32,
    pub energy: f32,
    pub safety: f32,
    pub belonging: f32,
    pub intimacy: f32,
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
    /// M16a (plan C8): a Fixer office's staff, on L2's wage scale.
    #[serde(default = "default_wage_fixer")]
    pub wage_fixer: i64,
    /// Real economy E26: a Mission's Volunteer (unpaid; a staff meal instead).
    #[serde(default)]
    pub wage_volunteer: i64,
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
fn default_wage_fixer() -> i64 {
    6
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
            Role::Fixer => self.wage_fixer,
            Role::Volunteer => self.wage_volunteer,
            // J9: a trade's wage is its row's (`Config::wage`).
            Role::Trade(_) => 0,
        }
    }
}
