use super::*;

/// M17 `[outside]`: the switch and its sub-tables. The World market is the Real economy's and is
/// not under it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutsideCfg {
    pub enabled: bool,
    pub ambient: AmbientDemandCfg,
    /// M17 phase 2: the parents' ledger and orders.
    pub parents: ParentsCfg,
    /// M17 phase 4b: a gang's succession (promotion, or a civil war).
    pub succession: SuccessionCfg,
    /// M17 phase 5: missions abroad and their cast (§ 5).
    pub abroad: AbroadCfg,
    /// M17 phase 5: the Blackwall and the outside's traces (§ 6).
    pub blackwall: BlackwallCfg,
}

impl Default for OutsideCfg {
    fn default() -> Self {
        OutsideCfg {
            enabled: true,
            ambient: AmbientDemandCfg::default(),
            parents: ParentsCfg::default(),
            succession: SuccessionCfg::default(),
            abroad: AbroadCfg::default(),
            blackwall: BlackwallCfg::default(),
        }
    }
}

/// M17 phase 5 `[outside.abroad]` (§ 5): a mission abroad is one keyed draw
/// after `abroad_days` away, `p = logistic(strike_k × (S_crew ÷ def − 1))`
/// with `def = def[asset] × (0.5 + 0.5 × edge)`. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AbroadCfg {
    pub abroad_days: u32,
    /// `[depot, data_centre, convoy]`.
    pub def: [f32; 3],
    pub strike_k: f32,
    pub loot: i64,
    /// The crew's share of a depot's loot (the rest to the buyer).
    pub crew_share: f32,
    pub data: u32,
    pub convoy_cut: f32,
    pub convoy_days: u32,
    pub death_p: f32,
    pub crew_max: usize,
    pub cast_size: u32,
    pub promote_friend: f32,
    pub promote_enemy: f32,
    pub promote_p: f32,
    pub promote_delay_days: u32,
    /// Promoted cast living at most, at 2,000 residents (`scaled_to`).
    pub promoted_max: u32,
    /// A Campaign with no run and no Beat sends this many of its Outsiders at least.
    pub campaign_crew_min: usize,
}

impl Default for AbroadCfg {
    fn default() -> Self {
        AbroadCfg {
            abroad_days: 3,
            def: [3.0, 4.0, 2.0],
            strike_k: 4.0,
            loot: 1500,
            crew_share: 0.4,
            data: 150,
            convoy_cut: 0.05,
            convoy_days: 7,
            death_p: 0.3,
            crew_max: 6,
            cast_size: 6,
            promote_friend: 0.6,
            promote_enemy: -0.6,
            promote_p: 0.3,
            promote_delay_days: 30,
            promoted_max: 8,
            campaign_crew_min: 2,
        }
    }
}

/// M17 phase 5 `[outside.blackwall]` (§ 6): the one boogeyman. A
/// successful run on an uplink at a deck of `cap` rolls `breach_p`, never
/// twice inside `cooldown_days`. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlackwallCfg {
    /// The deck, ICE and daemon tier ceiling (M14's tiers stop here).
    pub cap: u8,
    pub breach_p: f32,
    pub alarm_days: u32,
    pub cooldown_days: u32,
    /// The conf of the sighting the outside's trace writes in its branches' databases.
    pub trace_conf: f32,
}

impl Default for BlackwallCfg {
    fn default() -> Self {
        BlackwallCfg { cap: 3, breach_p: 0.002, alarm_days: 7, cooldown_days: 60, trace_conf: 0.8 }
    }
}

impl OutsideCfg {
    pub fn is_default(&self) -> bool {
        *self == OutsideCfg::default()
    }
}

/// M17 phase 1 `[outside.ambient]`: the Ambient flats' households buying at
/// simulated businesses with World money (export demand at retail). Every
/// value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientDemandCfg {
    /// The share of the Ambient households' meals (and venue visits) bought
    /// at simulated businesses.
    pub capture: f32,
    /// Units of food a day, city-wide, at 2,000 residents (`scaled_to`, up only).
    pub food_cap: u32,
    /// Venue visits a day, city-wide, at 2,000 residents (`scaled_to`, up only).
    pub visit_cap: u32,
    /// A Market sells to the book only its stock above this many days of
    /// its own mean daily sales (7 days).
    pub keep_days: f32,
    /// A venue takes at most this share of its own 7-day mean daily visits
    /// as Ambient visits a day.
    pub visit_share: f32,
    /// The appetite walk (as `[world_market]`'s per good).
    pub revert: f32,
    pub sd: f32,
    pub appetite_min: f32,
    pub appetite_max: f32,
    /// The hours food sells, `[from, to)`.
    pub food_hours: [u8; 2],
    /// The hours venues take visits, `[from, to)`.
    pub visit_hours: [u8; 2],
}

impl Default for AmbientDemandCfg {
    fn default() -> Self {
        AmbientDemandCfg {
            capture: 0.02,
            food_cap: 150,
            visit_cap: 60,
            keep_days: 1.0,
            visit_share: 0.5,
            revert: 0.05,
            sd: 0.03,
            appetite_min: 0.3,
            appetite_max: 3.0,
            food_hours: [8, 20],
            visit_hours: [18, 24],
        }
    }
}

/// Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier. A map Home's Ambient
/// flats are its `flats=` beyond its household slots (`floors`), or, on a
/// map without `flats=`, `round(floors x flats_per_slot)` (0 by default: the
/// shipped map has no Ambient tier).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientCfg {
    /// Off: no Ambient flats on any map.
    pub enabled: bool,
    /// Ambient flats per household slot on a map without `flats=`.
    pub flats_per_slot: f32,
    /// The share of Ambient flats with a household in them (a hash per flat).
    pub occupancy: f32,
    /// The share of Ambient households with a second adult.
    pub couple_p: f32,
    /// Labour-pool hires a day at 2,000 residents (`Config::per_capita_count`).
    pub hires_per_day: u32,
    /// Also prefer an Ambient household within `[life] commute_hire_ticks`
    /// of the workplace to a simulated candidate beyond it.
    pub hire_far: bool,
    /// The donor's contacts copied onto a materialised adult, at most.
    pub edges_max: u32,
}

impl Default for AmbientCfg {
    fn default() -> Self {
        AmbientCfg {
            enabled: true,
            flats_per_slot: 0.0,
            occupancy: 0.92,
            couple_p: 0.25,
            hires_per_day: 10,
            hire_far: false,
            edges_max: 96,
        }
    }
}

impl AmbientCfg {
    pub fn is_default(&self) -> bool {
        *self == AmbientCfg::default()
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
    /// M16a (plan C8).
    #[serde(default = "UpkeepCfg::default_fixer")]
    pub fixer: i64,
    /// Real economy E26, E37 (plan deviation: the `[treasury] property_rate`
    /// of phase 2 mirrors these; at the base a Mission is the city's and
    /// pays none, a corp's Camp pays 2).
    #[serde(default)]
    pub mission: i64,
    #[serde(default = "UpkeepCfg::default_camp")]
    pub camp: i64,
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
    fn default_fixer() -> i64 {
        8
    }
    fn default_camp() -> i64 {
        2
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
            BuildingKind::Fixer => self.fixer,
            BuildingKind::Mission => self.mission,
            BuildingKind::Camp => self.camp,
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
    /// M16a (plan C8).
    #[serde(default = "ValueCfg::default_fixer")]
    pub fixer: i64,
    /// Real economy E26, E37.
    #[serde(default = "ValueCfg::default_mission")]
    pub mission: i64,
    #[serde(default = "ValueCfg::default_camp")]
    pub camp: i64,
}

impl ValueCfg {
    fn default_mission() -> i64 {
        300
    }
    fn default_camp() -> i64 {
        800
    }
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
    fn default_fixer() -> i64 {
        500
    }
}

// ---------------------------------------------------------------------------
// The Real economy (docs/ECONOMY_V2.md; plan E1, E4, E35)
// ---------------------------------------------------------------------------

/// `[economy2]` (plan E1): the Real economy's parameters. Its phase switches
/// (`wages`, `no_safety_net`) retired with P10: the wage law and the safety
/// net are policy levers (`[policy]`, docs/P10_POLICY.md); the keys below
/// marked "wages on" are read under the market wage law (the levy not
/// enacted, `wages::on`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Economy2Cfg {
    /// E35: a contingency only (0: off): coins an agent needs to start emigrating.
    pub emigrate_cost: i64,
    // --- Phase 2 (spec § 5, plan E13-E20): wages from revenue.
    /// E17: the labour share of revenue per niche (`P* = share × R`).
    pub labour_share: LabourShareCfg,
    /// E17: `wage_rev` moves at most this a day, inside the clamps.
    pub wage_step: f32,
    pub wage_floor_mult: f32,
    pub wage_cap_mult: f32,
    /// E17: a vacancy unfilled this many days is a shortage (wages up).
    pub shortage_days: u64,
    /// E18: post a vacancy a building a day when `P < hire_below × P*` for
    /// 3 days and the niche demand is at least `hire_demand`.
    pub hire_below: f32,
    pub hire_demand: f32,
    /// E18: above `fire_above × P*` for 7 days (at the wage floor) the newest goes.
    pub fire_above: f32,
    /// E18: Farm staff per 100 units of unfilled World Food order a day.
    pub export_staff: f32,
    /// E13: coins to the World per unit produced.
    pub input_per_food: f32,
    pub input_per_part: f32,
    pub input_per_data: f32,
    /// E13: coins a day to the World per standing building of the kind.
    pub power: PowerCfg,
    /// E20: venue prices follow the payroll-weighted mean `wage_rev` at this strength.
    pub venue_wage_pass: f32,
    /// Plan field (E19 deviation): what a corp pays to build a Farm when its
    /// Food demand came from the World (`[corps] found_cost` has no Farm:
    /// Farms are map-placed before this milestone).
    pub farm_found_cost: i64,
    // --- The jobs round (after phase 2's stop rule; read only behind
    // `econ::wages_on`; every key's off value reproduces phase 2).
    /// Staffing from revenue: a corp under its margin may post past
    /// `full_staff` up to `ceil(full_staff × staff_ceiling_mult)` per
    /// building while its target covers the marginal hire (1.0: phase 2's cap).
    pub staff_ceiling_mult: f32,
    /// Counter-cyclical capital: a corp holding more than `capital_hi_days`
    /// × max(7-day revenue, 7-day payroll) adds the excess ÷
    /// `capital_payout_days` a day to its payroll target (0: off).
    pub capital_hi_days: f32,
    pub capital_payout_days: f32,
    /// The band's floor: under `capital_lo_days` × max(R, P) a corp posts
    /// nothing and lays off at once when over `fire_above × P*` (0: off).
    pub capital_lo_days: f32,
    /// A corp's business-fleet purchase (outside `asset_import_per_day`)
    /// must leave it `fleet_floor_days` × max(R, P) (0: no floor).
    pub fleet_floor_days: f32,
    /// The first-week asset burst: coins of asset imports (`ImportWhy::Asset`)
    /// a corp may front a day (0: unlimited, phase 2).
    pub asset_import_per_day: i64,
    /// The overflow leg: with wages on the Reserve buys a Farm's surplus only
    /// from a positive Treasury, the rest stays at the Farm (false: phase 2).
    pub overflow_paid_only: bool,
    /// Phase 3a (the transition wave; read only under the market wage law): a
    /// co-resident spouse covers a short rent share before arrears.
    pub household_rent: bool,
    // --- Jobs and room P1 (docs/JOBS_V2.md § 3; plan J2, J4; read only
    // with wages on; each off value reproduces the jobs round).
    /// J2: hire before raise: `wage_rev` steps up only on a shortage (a
    /// vacancy unfilled `shortage_days`), not on `P < 0.9 P*` (false: both).
    pub raise_on_shortage_only: bool,
    /// J4: the hiring key's skill weight: `round(skill_w × 100 × skill)`
    /// tiles off the distance for the role's skill (0: distance only).
    pub skill_w: f32,
    /// J4: tiles off the hiring key for an adult laid off from the same
    /// role in the last `demography::REHIRE_DAYS` (0: no bonus).
    pub rehire_bonus: u32,
    /// Flip readiness (the transition wave; read only with wages on): at
    /// world creation each corp posts its buildings' places up to the
    /// staffing ceiling while its working capital covers `seed_staff_days`
    /// days of the added payroll, and the jobless fill them by the hiring
    /// key before day 0 (`wages::seed_staff`; 0: off, the dole era's seed).
    pub seed_staff_days: f32,
    /// Workplace room (read only with wages on): the jobs round's places
    /// past full staff fit the building's room on their shift
    /// (`wages::room_capped`): the on-site staff of a shift never past
    /// `capacity` (less `customer_room` of it at a customer-facing kind);
    /// full staff is never cut (false: the jobs round's `staff_ceiling_mult`
    /// alone).
    pub room_cap: bool,
    /// Workplace room: the share of a customer-facing building's
    /// `capacity` (a Market, a venue, a Clinic, a Garage, a Hotel) its
    /// staff leave to the customers.
    pub customer_room: f32,
    /// Workplace room: with `room_cap`, a Farm or a Fab whose day shift
    /// fills its room hires its bespoke role onto the night shift
    /// (`[world] shift_night`), a second shift's room (false: day only).
    pub relief_shift: bool,
}

impl Default for Economy2Cfg {
    fn default() -> Self {
        Economy2Cfg {
            emigrate_cost: 0,
            labour_share: LabourShareCfg::default(),
            wage_step: 0.03,
            wage_floor_mult: 0.8,
            wage_cap_mult: 2.5,
            shortage_days: 5,
            hire_below: 0.85,
            hire_demand: 0.5,
            fire_above: 1.25,
            export_staff: 2.0,
            input_per_food: 0.5,
            input_per_part: 4.0,
            input_per_data: 8.0,
            power: PowerCfg::default(),
            venue_wage_pass: 0.5,
            farm_found_cost: 1000,
            staff_ceiling_mult: 1.0,
            capital_hi_days: 0.0,
            capital_payout_days: 30.0,
            capital_lo_days: 0.0,
            fleet_floor_days: 0.0,
            asset_import_per_day: 0,
            overflow_paid_only: false,
            household_rent: false,
            raise_on_shortage_only: false,
            skill_w: 0.0,
            rehire_bonus: 0,
            seed_staff_days: 0.0,
            room_cap: false,
            customer_room: 0.4,
            relief_shift: false,
        }
    }
}

/// `[economy2] power` (spec § 5, plan E13): coins a day to the World per
/// standing building of the kind, any owner (a stand-in for a generator
/// until Power is a good). Kinds absent from the table pay 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PowerCfg {
    pub market: i64,
    pub bar: i64,
    pub club: i64,
    pub arcade: i64,
    pub lounge: i64,
    pub clinic: i64,
    pub garage: i64,
    pub lab: i64,
    pub fab: i64,
}

impl Default for PowerCfg {
    fn default() -> Self {
        PowerCfg { market: 20, bar: 3, club: 8, arcade: 4, lounge: 15, clinic: 6, garage: 6, lab: 10, fab: 12 }
    }
}

impl PowerCfg {
    pub fn for_kind(&self, kind: BuildingKind) -> i64 {
        match kind {
            BuildingKind::Market => self.market,
            BuildingKind::Bar => self.bar,
            BuildingKind::Club => self.club,
            BuildingKind::Arcade => self.arcade,
            BuildingKind::Lounge => self.lounge,
            BuildingKind::Clinic => self.clinic,
            BuildingKind::Garage => self.garage,
            BuildingKind::Lab => self.lab,
            BuildingKind::Fab => self.fab,
            _ => 0,
        }
    }
}

/// `[treasury] property_rate` (spec § 5, plan E13): the small daily rate a
/// non-city owner pays the Treasury per standing building (replaces
/// `[corps] upkeep` with `econ::wages_on`). `venue` covers the six leisure
/// kinds; kinds absent from the table pay 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropertyRateCfg {
    pub farm: i64,
    pub market: i64,
    pub bar: i64,
    /// Per Block by tier (Sump, Mid, Spire); a scalar means every tier.
    #[serde(deserialize_with = "per_tier")]
    pub home: [i64; 3],
    pub hotel: i64,
    pub clinic: i64,
    pub garage: i64,
    pub lab: i64,
    pub fab: i64,
    pub venue: i64,
}

impl Default for PropertyRateCfg {
    fn default() -> Self {
        PropertyRateCfg {
            farm: 6,
            market: 10,
            bar: 1,
            home: [0, 0, 1],
            hotel: 1,
            clinic: 2,
            garage: 2,
            lab: 3,
            fab: 3,
            venue: 1,
        }
    }
}

impl PropertyRateCfg {
    pub fn for_building(&self, kind: BuildingKind, tier: u8) -> i64 {
        match kind {
            BuildingKind::Farm => self.farm,
            BuildingKind::Market => self.market,
            BuildingKind::Bar => self.bar,
            BuildingKind::Home => self.home[usize::from(tier.min(2))],
            BuildingKind::Hotel => self.hotel,
            BuildingKind::Clinic => self.clinic,
            BuildingKind::Garage => self.garage,
            BuildingKind::Lab => self.lab,
            BuildingKind::Fab => self.fab,
            k if k.is_leisure() => self.venue,
            _ => 0,
        }
    }
}

/// `[treasury]` (spec § 5, plan E13, E14, E21): the property rate, the
/// working balance the Treasury opens with (the rest of `[world]
/// treasury_initial` is the corps' working capital, `econ::seed_capital`),
/// and phase 3a's tax band.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TreasuryCfg {
    pub property_rate: PropertyRateCfg,
    pub treasury_initial: i64,
    pub tax_min: f32,
    pub tax_max: f32,
    pub tax_step: f32,
    pub band: [i64; 2],
    /// The surplus credit: while the Treasury sits above `band[1]` the rate
    /// may fall past `tax_min` down to `-credit_max`. A negative rate is a
    /// credit on every wage paid (`economy::collect_wage`), paid from the
    /// coins above `band[0]` only, so the Treasury hoards no income its
    /// capped posts can't spend. 0: the rate stops at `tax_min`.
    pub credit_max: f32,
    /// Phase 3a (plan key, the transition wave): the tax rate a market-law
    /// city opens with (`Levers::from_config`); negative keeps `[levers] tax_rate`.
    pub tax_initial: f32,
    /// Phase 3a (plan key, the transition wave): what the Recycler's till
    /// pays a scavenger's find under the market wage law (0: `[life] scavenge_coins`).
    pub scrap_coins: i64,
    /// Flip readiness (the transition wave; the market wage law only): coins moved from
    /// the Treasury into the Recycler's till at world creation (a purse
    /// move, `treasury::seed_till`): the scrap dealer's opening float, so a
    /// find pays before its first Parts sale (0: the till opens empty and
    /// pays nothing until the Recycler has sold, ~day 21-24).
    pub till_initial: i64,
    /// Jobs v2 J16 (P5a, the market wage law only): the civic budget's share of the
    /// trailing receipts (`Flow::{Tax, Fine, Customs, Property}` and the El's fares).
    pub civic_share: f32,
    /// J16: days over which the hoard above `band[1]` is paid out.
    pub payout_days: u32,
    /// J16: days of receipts the budget averages.
    pub receipts_days: u32,
    /// J16: the police ceiling per 1,000 residents.
    pub police_per_1000: f32,
    /// J16: the Sanitation ceiling.
    pub sanitation_max: u16,
    /// J16: a headcount never drops more than this a day (the reconcilers' pace).
    pub civic_pace: u8,
    /// P5a (risk 8): police per Jail bed, the ceiling's cap by Jail room
    /// (`treasury::police_ceiling`).
    pub police_per_jail_bed: f32,
}

impl Default for TreasuryCfg {
    fn default() -> Self {
        TreasuryCfg {
            property_rate: PropertyRateCfg::default(),
            treasury_initial: 12_000,
            tax_min: 0.05,
            tax_max: 0.20,
            tax_step: 0.01,
            band: [8_000, 25_000],
            credit_max: 0.0,
            tax_initial: -1.0,
            scrap_coins: 0,
            till_initial: 0,
            civic_share: 0.8,
            payout_days: 60,
            receipts_days: 14,
            police_per_1000: 35.0,
            sanitation_max: 50,
            civic_pace: 5,
            police_per_jail_bed: 0.25,
        }
    }
}

/// `[world_market] <good>` (spec § 3): one good's bid curve, spread and
/// season levels (Spring, Summer, Autumn, Winter).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GoodCfg {
    /// The first unit's bid at appetite and market 1.0.
    pub bid_ref: f32,
    /// No unit fetches less.
    pub bid_floor: f32,
    /// Units a day at appetite 1.0 (`cap_today = round(cap × appetite)`).
    pub cap: u32,
    /// The bid falls by this fraction of `bid_ref` over the day's cap.
    pub slope: f32,
    /// The ask over the bid: `ask = bid_ref × market × (1 + spread) × ask_mult`.
    pub spread: f32,
    /// E7: a level added to the walk per season.
    pub season: [f32; 4],
}

impl Default for GoodCfg {
    fn default() -> Self {
        GoodCfg { bid_ref: 1.0, bid_floor: 0.5, cap: 0, slope: 0.5, spread: 0.5, season: [0.0; 4] }
    }
}

/// `[world_market]` (spec § 3, plan E4-E12): the World account's book.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldMarketCfg {
    /// The World account's treasury, refilled to this both ways each midnight (E6).
    pub treasury_ref: i64,
    /// E7: the appetite walk's mean reversion and daily noise.
    pub revert: f32,
    pub sd: f32,
    pub appetite_min: f32,
    pub appetite_max: f32,
    pub food: GoodCfg,
    pub parts: GoodCfg,
    pub data: GoodCfg,
    /// E11, E12: coins a unit over the ask before a Market imports; the shelf's ceiling is `ask + this`.
    pub haul_margin: i64,
    /// E5: of an import's price, to the Treasury on top.
    pub customs_rate: f32,
    /// The famine lever (lower: cheaper imports), over every good's own `ask_mult`.
    pub ask_mult: f32,
    /// E10 (phase 2): chrome and decks kept in a seller's stock before Parts-equivalent export.
    pub asset_floor: u32,
    /// The jobs round (export demand, read only with `econ::wages_on`): the
    /// World's daily cap per good `[food, parts, data]` in a wages city
    /// (0: the good's own `cap`).
    pub cap_wages: [u32; 3],
    /// Jobs & lives phase 1 (D6, risk R2): the appended goods' books, read
    /// only with `[jobs2] enabled`, each capped per capita (`scaled_to`).
    /// The Rations bid is `[jobs2] packed_premium` × Food's; BD and Organs
    /// stay inert until their phases (5, 6).
    pub rations: GoodCfg,
    pub braindance: GoodCfg,
    pub organs: GoodCfg,
}

impl Default for WorldMarketCfg {
    fn default() -> Self {
        WorldMarketCfg {
            treasury_ref: 50_000,
            revert: 0.05,
            sd: 0.03,
            appetite_min: 0.3,
            appetite_max: 3.0,
            food: GoodCfg {
                bid_ref: 3.4,
                bid_floor: 1.5,
                cap: 350,
                slope: 0.5,
                spread: 0.6,
                season: [0.0, -0.15, 0.0, 0.25],
            },
            parts: GoodCfg { bid_ref: 16.0, bid_floor: 8.0, cap: 40, slope: 0.4, spread: 0.5, season: [0.0; 4] },
            data: GoodCfg { bid_ref: 35.0, bid_floor: 15.0, cap: 12, slope: 0.5, spread: 0.8, season: [0.0; 4] },
            haul_margin: 1,
            customs_rate: 0.1,
            ask_mult: 1.0,
            asset_floor: 2,
            cap_wages: [0; 3],
            rations: GoodCfg { bid_ref: 4.8, bid_floor: 2.0, cap: 150, slope: 0.5, spread: 0.6, season: [0.0; 4] },
            braindance: GoodCfg { bid_ref: 12.0, bid_floor: 5.0, cap: 20, slope: 0.5, spread: 0.6, season: [0.0; 4] },
            organs: GoodCfg { bid_ref: 40.0, bid_floor: 20.0, cap: 6, slope: 0.5, spread: 0.8, season: [0.0; 4] },
        }
    }
}

impl WorldMarketCfg {
    /// The good's block.
    pub fn good(&self, good: crate::outside::ExportGood) -> &GoodCfg {
        match good {
            crate::outside::ExportGood::Food => &self.food,
            crate::outside::ExportGood::Parts => &self.parts,
            crate::outside::ExportGood::Data => &self.data,
            crate::outside::ExportGood::Rations => &self.rations,
            crate::outside::ExportGood::Braindance => &self.braindance,
            crate::outside::ExportGood::Organs => &self.organs,
        }
    }
}
