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
    /// M14 the Virt plane (§ 1).
    #[serde(default)]
    pub virt: VirtCfg,
    /// M14 decks (§ 2).
    #[serde(default)]
    pub decks: DecksCfg,
    /// M14 ICE and the contest (§ 3).
    #[serde(default)]
    pub ice: IceCfg,
    /// M14 Data and Labs (§ 4).
    #[serde(default)]
    pub data: DataCfg,
    /// M14 the tech tree (§ 5).
    #[serde(default)]
    pub tech: TechCfg,
    /// M14 hacking goals and orders (§ 6).
    #[serde(default)]
    pub hack: HackCfg,
    /// M14 faction databases (§ 6).
    #[serde(default)]
    pub db: DbCfg,
    /// M15 § 1 the word (pools, exchange, hearing, kin, reputation).
    #[serde(default)]
    pub gossip: GossipCfg,
    /// M15 § 2 reputation.
    #[serde(default)]
    pub reputation: ReputationCfg,
    /// M15 § 2 appearance and taste (phase 2).
    #[serde(default)]
    pub taste: TasteCfg,
    /// M15 § 2 the Purist creed (phase 2).
    #[serde(default)]
    pub creeds: CreedsCfg,
    /// M15 § 3 grudges (phase 3).
    #[serde(default)]
    pub grudges: GrudgesCfg,
    /// M15 § 4 the Hunt (phase 3).
    #[serde(default)]
    pub hunt: HuntCfg,
    /// M15 § 5 social skills (phase 2).
    #[serde(default)]
    pub skills: SkillsCfg,
    /// M15 § 6 the social move (phase 2; `contradict_conf` from phase 1).
    #[serde(default)]
    pub moves: MovesCfg,
    /// M15 § 6 competence and poaching (phase 2).
    #[serde(default)]
    pub competence: CompetenceCfg,
    /// M15 § 7 Feeds and stories (phase 4).
    #[serde(default)]
    pub news: NewsCfg,
    /// Life pass L1 (docs/history/SHADOW_V1.md): travel, sleep, stale targets, the
    /// poor's fallback, witness and edge spam, order dwell; the L2 shadow
    /// fixes' keys; the violence fixes (`violence_fixes`, in flight).
    #[serde(default)]
    pub life: LifeCfg,
    /// Life pass L2 (docs/LIFE_L2.md, plan L5).
    #[serde(default)]
    pub living: LivingCfg,
    /// L2 § 1 jobs (plan L5: `LivingJobsCfg`, as `JobsCfg` is `[world.jobs]`).
    #[serde(default)]
    pub jobs: LivingJobsCfg,
    /// L2 § 1-2 leisure (phase 1 reads only the price table).
    #[serde(default)]
    pub leisure: LeisureCfg,
    /// L2 § 1 the Treasury budget band.
    #[serde(default)]
    pub budget: BudgetCfg,
    /// L2 § 1 the export hook (off by default).
    #[serde(default = "ExportCfg::off")]
    pub export: ExportCfg,
    /// L2 § 3 faction violence off screen (phase 4; plan L24-L28).
    #[serde(default)]
    pub fviolence: FviolenceCfg,
    pub demography: DemographyCfg,
    pub brain: BrainCfg,
    pub exec: ExecCfg,
    pub lod: LodCfg,
    pub levers: LeversCfg,
    /// M16a (docs/M16_CONTRACTS.md, plan C3): contract records.
    #[serde(default)]
    pub contracts: ContractsCfg,
    /// M16a § 2: Fixers and regulars .
    #[serde(default)]
    pub fixers: FixersCfg,
    /// M16a § 3: missions and the strike (phase 2).
    #[serde(default)]
    pub missions: MissionsCfg,
    /// M16a § 3: Locate bounties (16a's three keys).
    #[serde(default)]
    pub bounty: BountyCfg,
    /// The Hit market (docs/HIT_MARKET.md): organic Hits and Beats.
    #[serde(default)]
    pub hitmarket: HitMarketCfg,
    /// M16b § 4 (phase 1): leverage and coercion.
    #[serde(default)]
    pub leverage: LeverageCfg,
    /// M16b phase 2: captives, the Prison and the new kinds.
    #[serde(default)]
    pub hostage: HostageCfg,
    /// M16b phase 3: the wounded.
    #[serde(default)]
    pub wounded: WoundedCfg,
    /// M16b phase 3: the Trauma Team.
    #[serde(default)]
    pub trauma: TraumaCfg,
    /// M16b phase 4: boards and votes.
    #[serde(default)]
    pub governance: GovernanceCfg,
    /// M16b phase 4: tags, scanners and the Purist Patrol.
    #[serde(default)]
    pub tags: TagsCfg,
    /// M16b phase 4: the attention addendum.
    #[serde(default)]
    pub attention: AttentionCfg,
    /// The Real economy (docs/ECONOMY_V2.md, plan E1): the World market is
    /// always on; the wage law and the safety net are policy (P10, `policy`).
    #[serde(default)]
    pub economy2: Economy2Cfg,
    /// P10 (docs/P10_POLICY.md): the policy a new world starts under.
    #[serde(default)]
    pub policy: PolicyCfg,
    /// Spec § 3 `[world_market]`: the World's book per good (phase 1).
    #[serde(default)]
    pub world_market: WorldMarketCfg,
    /// Real economy § 4 (plan E26-E30): Missions and donations.
    #[serde(default)]
    pub charity: CharityCfg,
    /// Real economy addendum 19 (plan E37-E42): the child protective
    /// service and the work camps.
    #[serde(default)]
    pub camp: CampCfg,
    /// Spec § 5 `[treasury]` (plan E13, E14, E21): the property rate and
    /// the working balance (phase 2; read only behind `econ::wages_on`),
    /// the tax band (phase 3a).
    #[serde(default)]
    pub treasury: TreasuryCfg,
    /// Jobs and room (docs/JOBS_V2.md § 2.3, plan J6-J8): floors.
    #[serde(default)]
    pub floors: FloorsCfg,
    /// Hunger as sickness (docs/HUNGER_SICKNESS.md § 8): synth and real
    /// meals, the malnutrition and toxicity meters, the bins, cancer.
    #[serde(default)]
    pub sickness: SicknessCfg,
    /// Class pay (docs/CLASS_PAY.md): graded posts and wages, read only
    /// under the market wage law (`classpay::on`).
    #[serde(default)]
    pub classpay: ClassPayCfg,
    /// Jobs and room J9 (docs/JOBS_V2.md § 1.3): the configured trades
    /// (`[[trades]]` rows), `Role::Trade(TradeId)` indexing them. Read
    /// through [`trades_de`], which also makes the keys a save's
    /// `Trade("key")` resolves against.
    #[serde(default, deserialize_with = "trades_de")]
    pub trades: Vec<TradeCfg>,
    /// J9 review: the trades as roles and per kind, built once on first
    /// read ([`Config::trade_roles`], [`Config::trades_at`]); not saved. A
    /// change to `trades` after the first read must call
    /// [`Config::reset_trade_cache`].
    #[serde(skip)]
    pub trade_cache: TradeCache,
    /// Transit (docs/TRANSIT.md): the El. Off by default in phase 1; a
    /// switched-off config is not written into a save, so an off world's
    /// save is byte-identical to a world built before transit existed.
    #[serde(default, skip_serializing_if = "TransitCfg::is_off")]
    pub transit: TransitCfg,
    /// Scale S4 (docs/SCALE_20K.md): the reference city the absolute keys
    /// were tuned on, which `Config::scaled_to` and `Config::fit_to_map`
    /// scale from.
    #[serde(default)]
    pub scale: ScaleCfg,
    /// Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier, the flats
    /// beyond the simulated residents' beds. Not written into a save at its
    /// defaults (a 2k save is unchanged).
    #[serde(default, skip_serializing_if = "AmbientCfg::is_default")]
    pub ambient: AmbientCfg,
    /// M17 (docs/M17_OUTSIDE.md, "Revised 2026-10-12"): the outside world.
    /// Phase 1: the Ambient book (Ambient consumption as outside demand).
    /// Not written into a save at its defaults.
    #[serde(default, skip_serializing_if = "OutsideCfg::is_default")]
    pub outside: OutsideCfg,
    /// Jobs & lives (docs/JOBS_AND_LIVES.md § 1): the milestone's master
    /// switch and phase 1's keys.
    #[serde(default)]
    pub jobs2: Jobs2Cfg,
    /// Jobs & lives phase 3 (§ 4): weapons as owned items, under the
    /// master switch.
    #[serde(default)]
    pub weapons: WeaponsCfg,
    /// Jobs & lives phase 4 (§ 5.1): the Vice venues, the street pitch and
    /// its levers, under the master switch.
    #[serde(default)]
    pub vice: ViceCfg,
    /// Jobs & lives phase 4 (§ 9): clothing items, Boutiques and the
    /// `status` need, under the master switch.
    #[serde(default)]
    pub fashion: FashionCfg,
    /// Jobs & lives phase 6 (§ 7): papers and forgers, black clinics,
    /// organs, the gang types.
    #[serde(default)]
    pub gray: GrayCfg,
    /// Jobs & lives phase 6 (§ 8): creeds as data, churches, cults, tithes.
    #[serde(default)]
    pub faith: FaithCfg,
    /// Jobs & lives phase 7 (§ 6): children's routines and the School, the
    /// elderly, disability.
    #[serde(default)]
    pub ages: AgesCfg,
    /// Lives at work (docs/LIVES_AT_WORK.md D13): the milestone's master
    /// switch (`[lives] enabled = false` turns it off and clears `[service]`
    /// and `[[serves]]`, so an off save is byte-identical to main's).
    #[serde(default, skip_serializing_if = "LivesCfg::is_off")]
    pub lives: LivesCfg,
    /// Lives at work phase 1 (§ 1, § 2): the service encounter's switches
    /// and numbers. Not written into a save at its defaults.
    #[serde(default, skip_serializing_if = "ServiceCfg::is_default")]
    pub service: ServiceCfg,
    /// Lives at work phase 1 (§ 1.1): the `[[serves]]` rows, one per
    /// serving trade (a trade id or a legacy `Role` name as the key).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serves: Vec<ServesCfg>,
    /// Lives at work phase 2 (§ 3): the evening and the household, under the
    /// `[lives]` master (`[household] enabled = false`). Not written into a save
    /// at its defaults (the off save is byte-identical).
    #[serde(default, skip_serializing_if = "HouseholdCfg::is_default")]
    pub household: HouseholdCfg,
    /// Lives at work phase 6 (§ 4.1-4.4): age bands, seniority and the
    /// foreman, the profession, retirement and the frailty exit, under the
    /// `[lives]` master (`[careers] enabled = false`). Not written into a save
    /// at its defaults (the off save is byte-identical).
    #[serde(default, skip_serializing_if = "CareersCfg::is_default")]
    pub careers: CareersCfg,
    /// Lives at work phase 6 (§ 4.1): the legacy `Role`s' age bands,
    /// `[roles.<Name>]` (the `Role`'s name; `Config::unresolved_names`
    /// checks it). Read with `[careers] enabled`; empty (not written) off.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub roles: std::collections::BTreeMap<String, RoleCfg>,
    /// The pre-view pass phase 2 (docs/PREVIEW_PASS.md § 1, OPEN-137): the
    /// visible shift's shapes (`systems/shift.rs`). `None` (not written into
    /// a save): every shift the one block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<ShiftCfg>,
    /// The pre-view pass phase 4 (docs/PREVIEW_PASS.md § 0 P3, P8; OPEN-119,
    /// OPEN-120, OPEN-154): settlement by presence (`systems/presence.rs`).
    /// `None` (not written into a save): encounters settle by link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<PresenceCfg>,
    /// The pre-view pass phase 5 (docs/PREVIEW_PASS.md § 0 P10-P12; OPEN-139,
    /// OPEN-115): the 20k legs (segments, the exec's ride home, the bed by
    /// ride, the escort van, the fares). `None` (not written into a save)
    /// also drops the map's `L+` lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<LegsCfg>,
    /// The pre-view pass phase 6 (docs/PREVIEW_PASS.md § 0 P9, P13-P15;
    /// OPEN-118, OPEN-128): the leader's round, court and orders given in
    /// person (`systems/leaders.rs`). `None` (not written into a save).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leaders: Option<LeadersCfg>,
    /// The pressure pass phase 1 (docs/PRESSURE_PASS.md § Phase 1;
    /// OPEN-178): misery acts (`systems/misery.rs`). `None` (not written
    /// into a save) with the key absent: mood moves only what it moved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub misery: Option<MiseryCfg>,
    /// The pressure pass phase 6 (docs/PRESSURE_PASS.md § Phase 6): a
    /// business under water (`systems/underwater.rs`). `None` (not written
    /// into a save) with the key absent: an owner bleeds his purse into the
    /// wages as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underwater: Option<UnderwaterCfg>,
    /// The pressure pass phase 2 (docs/PRESSURE_PASS.md § Phase 2; OPEN-176,
    /// OPEN-190, OPEN-120, OPEN-017): the dead trades get customers
    /// (`systems/customers.rs`). `None` (not written into a save) with the
    /// key absent: the trades serve whom they served.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customers: Option<CustomersCfg>,
    /// The pressure pass phase 3 (docs/PRESSURE_PASS.md § Phase 3):
    /// protection bites (`systems/protection.rs`). `None` (not written into
    /// a save) with the key absent: M16b's protection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<ProtectionCfg>,
    /// The island pass phase 1 (docs/ISLAND_PASS.md I1; OPEN-163 part):
    /// travel time in the job and bed picks (`systems/distance.rs`). `None`
    /// (not written into a save) with the key absent: the picks
    /// read no cap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<DistanceCfg>,
    /// The island pass phase 3a (docs/ISLAND_PASS.md I6; OPEN-163):
    /// seeded company housing (`systems/company.rs`). `None` (not written
    /// into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company_housing: Option<CompanyHousingCfg>,
    /// The island pass phase 4 (docs/ISLAND_PASS.md I2; OPEN-163): a far
    /// commute is a pressure that resolves over weeks (`systems/commute.rs`).
    /// `None` (not written into a save) with the key absent:
    /// the relocation and the company move-in are the base ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commute_pressure: Option<CommutePressureCfg>,
    /// The island pass phase 5 (docs/ISLAND_PASS.md I3; OPEN-165): a vehicle
    /// in motion is an entity with its own position, its riders aboard
    /// (`systems/motion.rs`). `None` (not written into a save) with
    /// the key absent: the timed legs as before (the body on
    /// its boarding tile, then at the door).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<MotionCfg>,
    /// The island pass phase 8 (docs/ISLAND_PASS.md I4; OPEN-168 part): the
    /// dead trades' bugs (`[island_trades]`: the courier's and the builder's
    /// destinations, the corner at every Yard, the cab's body driver, the
    /// arms dealer at the corner). `None` (not written into a save) with
    /// the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub island_trades: Option<IslandTradesCfg>,
    /// The island fix round F2 (Shadow V8 bugs 2, 4, 6; OPEN-189, OPEN-191,
    /// OPEN-193): a bed is taken only if it is the resident's own
    /// workplace's or within the commute cap (`systems/beds.rs`). `None`
    /// (not written into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beds: Option<BedsCfg>,
    /// The island pass phase 3c (OPEN-177): `[mood.*]`, the mood terms read
    /// from config. Empty (not written into a save) with
    /// the key absent.
    #[serde(default, skip_serializing_if = "MoodCfg::is_empty")]
    pub mood: MoodCfg,
    /// The island fix round F1 (OPEN-188, SHADOW_V8 D3 A): a trip's way is
    /// judged by tiles, not district labels (`[routes]`). `None` (not
    /// written into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routes: Option<RoutesCfg>,
    /// The perf pass phase 1 (docs/PERF_PASS.md; OPEN-195): the Rest loop's
    /// fix (`exec::routine::rest_fix`). `None` (not written into a save)
    /// with the key absent: the base rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<RestCfg>,
    /// The perf pass phase 8 (docs/PERF_PASS.md; OPEN-184): `[perf]`, how
    /// the host runs the tick (the think slice's scoring threads). A host
    /// setting, not the world's: `threads` is never written into a save (a
    /// loaded world reads the default until its host sets it), and no value
    /// of it moves the city (the thread-count identity). Phase 7b's
    /// `stagger_midnight` is the world's (it moves a midnight pass by
    /// minutes): written into a save when set, as a section is; a save
    /// without it reads 0, the base.
    #[serde(default, skip_serializing_if = "PerfCfg::host_only")]
    pub perf: PerfCfg,
}

/// `[perf]` (the perf pass phase 8, OPEN-184): the host's threads; phase
/// 7b: the midnight stagger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerfCfg {
    /// Threads scoring the tick's think slice (`systems::think::run`); 1
    /// scores it on the calling thread. Every value gives the same city.
    /// Never saved.
    #[serde(default = "PerfCfg::one_thread", skip_serializing)]
    pub threads: usize,
    /// The perf pass phase 7b (R8): ticks between two slots of the
    /// staggered midnight (`systems::midnight`); 0 (absent): every daily
    /// pass on tick 0, the base.
    #[serde(default)]
    pub stagger_midnight: u16,
}

impl Default for PerfCfg {
    fn default() -> Self {
        PerfCfg { threads: 1, stagger_midnight: 0 }
    }
}

impl PerfCfg {
    fn one_thread() -> usize {
        1
    }

    /// Nothing of the world's in it (no stagger): left out of a save.
    pub fn host_only(&self) -> bool {
        self.stagger_midnight == 0
    }
}

/// `[rest]` (the perf pass phase 1, OPEN-195): a Rest is cut only by a work
/// signal the Work goal will answer. No keys: the fix has no number.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RestCfg {}

/// `[routes]` (the island fix round F1, OPEN-188): when a trip's route is
/// longer than walking, walk; and a label is not space (the Spire is one
/// district 150-350 tiles across). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutesCfg {
    /// The exec's car (`streetwork::exec_ride`, the ride home and its bed
    /// estimate) drives a trip of this many Manhattan tiles or more, home
    /// to Hq or from where he stands to his Home; the base drove only
    /// between two district labels.
    pub exec_ride_min_tiles: u32,
    /// The El (`transit::route`, `commute_ticks`) is not boarded at a stop
    /// whose walk from the trip's start is past this, nor left at one whose
    /// walk to its end is; 0, no cap.
    pub access_cap_ticks: u64,
    /// A stop pair is no route when the alight stop lies on the walk to
    /// the board stop (within this many tiles of a shortest Manhattan way):
    /// the rider would walk past his stop to ride back to it.
    pub pass_slack_tiles: u32,
}

/// `[mood]`: the hourly mood target's config-read terms (the older terms
/// read their own sections: `[litter] mood`, `[fashion] status_w`, ...).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MoodCfg {
    /// The island pass phase 3c (OPEN-177): `[mood.housing]`, the Home's
    /// term (`mood::housing_bias`). `None`: no term.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub housing: Option<MoodHousingCfg>,
}

impl MoodCfg {
    pub fn is_empty(&self) -> bool {
        self.housing.is_none()
    }
}

/// `[mood.housing]` (docs/ISLAND_PASS.md phase 3c, OPEN-177): a flat bias
/// on the hourly mood target from the agent's Home. The homeless read none
/// (sleeping rough already costs energy and safety). Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoodHousingCfg {
    /// By the Home's tier (Sump, Mid, Spire).
    pub tier: [f32; 3],
    /// Crowding: `crowd_w x min(excess, crowd_max) x factor`, the excess
    /// the residents a floor over `[buildings] home` capacity (a plain
    /// Block's beds a floor), less 1.
    pub crowd_w: f32,
    pub crowd_max: f32,
    /// The factor for company floors inside a workplace above it (dorm
    /// storeys, container bunks), and below it (`[company_housing.inside]
    /// rooms` `below`: the coffin racks). Any other Home's is 1.
    pub inside_factor: f32,
    pub below_factor: f32,
    /// A bed in company housing (the company town's grip): subtracted.
    pub company: f32,
}

/// The island pass phase 3a `[company_housing]` (docs/ISLAND_PASS.md I6):
/// company blocks seeded beside the far-commuting workplaces at world
/// build, their beds reserved for that workplace's staff, the rent docked
/// from the wage, the bed lost `grace_days` after the job. Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyHousingCfg {
    /// The workplace kinds that put up company housing.
    pub kinds: Vec<BuildingKind>,
    /// A post is far when its holder's Home door (or the holder is
    /// homeless) is past this travel (`distance::travel`) from the
    /// workplace door; also the move-in rule for a new hire.
    pub far_ticks: u64,
    /// A company block's Lot: its door within this travel of the workplace
    /// door (the nearest first).
    pub lot_cap_ticks: u64,
    /// The share of a workplace's unmet far posts (far posts less the free
    /// beds within `far_ticks`) seeded as company beds at world build.
    pub seed_share: f32,
    /// No block for a workplace whose seeded want is below this many beds.
    pub min_beds: u32,
    /// At most this many blocks beside one workplace at world build.
    pub blocks_max: u32,
    /// With no vacant Lot within `lot_cap_ticks`, a block goes up on a
    /// plot of open ground this many tiles wide and high (walls included).
    pub plot_w: u16,
    pub plot_h: u16,
    /// A block's storeys: enough for the beds wanted, at most this (a Lot's
    /// own storeys at least).
    pub floors_max: u8,
    /// A company block's beds per floor (bunks; a plain Home's is
    /// `[buildings] home.capacity`), capped by the Lot's interior.
    pub beds_per_floor: u16,
    /// The company rent: this × `[rent] base[tier]` a day per housed
    /// worker, docked from the wage at collection.
    pub rent_mult: f32,
    /// A resident whose job at the workplace ended keeps the bed this many
    /// days, then is evicted.
    pub grace_days: u32,
    /// World build: the far posts' holders move into the seeded beds
    /// (the seed city lives where it is housed); off, only the new hires
    /// and the homeless staff fill them.
    pub fill_at_seed: bool,
    /// The island pass phase 3b (I6): the seeded beds as floors inside the
    /// workplace (`[company_housing.inside]`). `None` (not written into a
    /// save) with the key absent: 3a's blocks on a Lot,
    /// else a plot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inside: Option<CompanyInsideCfg>,
    /// The island pass phase 3b (I6): a corp buying a nearby block for its
    /// far staff (`[company_housing.buyout]`). `None` (not written into a
    /// save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buyout: Option<CompanyBuyoutCfg>,
}

/// The island pass phase 3b `[company_housing.inside]` (I6): the seeded
/// company beds are floors of the workplace building itself (a Home record
/// sharing the workplace's rect and door, no new footprint). Each kind's
/// floors have a display name, so the view can draw the building as both
/// workplace and warren. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyInsideCfg {
    /// Per workplace kind: the company floors' name and whether they sit
    /// below the workplace (a basement) or above it (storeys).
    pub rooms: Vec<CompanyRoomCfg>,
    /// The name for a kind with no `rooms` row.
    pub default_name: String,
}

/// One `[[company_housing.inside.rooms]]` row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyRoomCfg {
    pub kind: BuildingKind,
    pub name: String,
    /// The floors are a basement (else storeys on top).
    #[serde(default)]
    pub below: bool,
}

impl CompanyInsideCfg {
    /// The company floors' `(name, below)` inside a `kind` workplace.
    pub fn room(&self, kind: BuildingKind) -> (&str, bool) {
        self.rooms
            .iter()
            .find(|r| r.kind == kind)
            .map_or((self.default_name.as_str(), false), |r| (r.name.as_str(), r.below))
    }
}

/// The island pass phase 5 `[motion]` (docs/ISLAND_PASS.md I3): a vehicle
/// in motion advances along its way with its riders aboard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionCfg {
    /// A timed road vehicle's way follows the road path (the walk's A*);
    /// false: an L, along x then y (no search).
    pub road_route: bool,
    /// The island pass phase 6 `[motion.trains]` (OPEN-165): the El runs as
    /// trains on its timetable (`transit.rs` § Trains). `None` (not written
    /// into a save) with the key absent: the timed ride as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trains: Option<TrainsCfg>,
    /// The island pass phase 7 `[motion.vans]` (OPEN-166): the escort and
    /// the prisoner transfer ride a van (`stations.rs` § the van). `None`
    /// (not written into a save) with the key absent: the timed escort
    /// and the bodiless 04:00 transfer as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vans: Option<VansCfg>,
    /// The island pass phase 9 `[motion.walks]` (OPEN-181): a Coarse timed
    /// walk keeps its body on its way (`exec::walk_place`), and a promotion
    /// or an interrupt mid-walk sets it down there. `None` (not written into
    /// a save) with the key absent: the body stands at the
    /// walk's start until it arrives, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walks: Option<WalksCfg>,
}

/// `[motion.walks]`: a Coarse body's timed walk on foot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WalksCfg {
    /// The walk's way follows the road path (the Full walk's A*); false: an
    /// L, along x then y (no search).
    pub road_route: bool,
    /// A walk cut short (its plan dropped or replaced) leaves the body where
    /// it stands on its way; false: back at the walk's start.
    pub settle_interrupt: bool,
}

/// `[motion.vans]`: the law's vans. The drive is at the Car's speed
/// (`[vehicles] mult`); the escort's walk-or-van distance and the van's call
/// are `[legs]`'s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VansCfg {
    /// Prisoners one transfer van carries; a Station holding more due sends
    /// another van.
    pub capacity: u32,
}

/// `[motion.trains]`: the El's trains. The headway, the dwell, the speed and
/// the transfer walk are `[transit]`'s; the train count is derived (each
/// line's round trip over the headway).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainsCfg {
    /// Riders a train carries; a full train leaves the rest on the platform
    /// for the next one.
    pub capacity: u32,
}

/// The island pass phase 4 `[commute_pressure]` (docs/ISLAND_PASS.md I2):
/// a worker whose commute (`distance::travel`, the Home door to the work
/// tile) is past `far_ticks` builds a day of pressure each midnight; past
/// `move_days` the household looks for a Home nearer work (any it can afford
/// within `[distance] home_cap_ticks`, the corp's company beds included),
/// and past `job_days` with none found, the worker a nearer post of the same
/// role. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommutePressureCfg {
    /// A commute past this travel builds a day of pressure.
    pub far_ticks: u64,
    /// Days of pressure before the household looks for a nearer Home.
    pub move_days: u16,
    /// Days of pressure before, with no Home found, the worker looks for a
    /// nearer post of the same role.
    pub job_days: u16,
    /// A search that found nothing waits this many days for the next.
    pub retry_days: u16,
    /// After a move or a job switch, no pressure builds for this many days.
    pub cool_days: u16,
    /// A nearer post saves at least this much travel from the Home door.
    pub job_margin_ticks: u64,
    /// A worker hired since the last midnight with a far Home looks at once
    /// (phase 3a's move-in of a far hire, on this one path).
    pub hire_now: bool,
}

/// The island pass phase 3b `[company_housing.buyout]` (I6): a corp whose
/// staff still commute far buys a nearby residential block and converts
/// it. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyBuyoutCfg {
    /// The buyout pass runs at the midnight whose day is a multiple of this;
    /// each corp makes at most `per_corp` buyouts (or fallback builds) a pass.
    pub period_days: u32,
    pub per_corp: u32,
    /// A workplace buys when its far posts less its free company beds reach
    /// this many.
    pub min_want: u32,
    /// The block's door within this travel of the workplace door.
    pub cap_ticks: u64,
    /// Blocks of this tier and below only (0 Sump, 1 Mid, 2 Spire).
    pub tier_max: u8,
    /// The price: this x the block's value (`ownership::value(Home)` a
    /// floor, the first floor whole and each further one at `[floors]
    /// floor_cost_frac`).
    pub price_mult: f32,
    /// The corp buys only from what it holds above `treasury_ref` (its
    /// reserve), as a bankruptcy buyer.
    pub reserve: bool,
    /// The converted block's bunks a floor (the interior allowing; a block
    /// never loses beds by the conversion).
    pub bunks_per_floor: u16,
    /// With nothing for sale in reach, build a block on the nearest plain
    /// vacant Lot within `cap_ticks` (through `life::pay_housing`, the
    /// corp paying); false: no fallback.
    pub build_fallback: bool,
}

/// The island pass phase 1 `[distance]` (docs/ISLAND_PASS.md I1): each
/// pick's hard cap on the travel estimate (`distance::travel`, ticks), past
/// which a candidate is dropped while one within it is left. Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistanceCfg {
    /// The job: a candidate whose Home door (else tile) is past this from
    /// the workplace door.
    pub job_cap_ticks: u64,
    /// The bed a worker moves into: a Home whose door is past this from the
    /// workplace door.
    pub home_cap_ticks: u64,
    /// Tonight's bed (the Home or the gang's Hideout): past this from where
    /// the body stands.
    pub bed_cap_ticks: u64,
    /// The own Home's offer over a Hideout cot, in travel ticks: the Hideout
    /// is tonight's bed only when nearer by more than this.
    pub home_bed_ticks: u64,
    /// The island pass phase 2 (OPEN-163, OPEN-171): the free-time spot and
    /// the Hotel. `None` (not written into a save) with
    /// the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free: Option<FreeDistanceCfg>,
    /// The pressure pass phase 4 (OPEN-194): the reach of the non-urgent
    /// trips. `None` (not written into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<ReachCfg>,
}

/// The pressure pass phase 4 `[distance.reach]` (docs/PRESSURE_PASS.md;
/// OPEN-194): how far, in travel ticks (`distance::travel`), a non-urgent
/// trip is walked. Past it the trip is not made (a nearer alternative where
/// the purpose has one). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReachCfg {
    /// ReportCrime: the Hall's door from where the witness sets out.
    pub report_ticks: u64,
    /// Found (Register): the Hall's door from where the founder sets out.
    pub register_ticks: u64,
    /// Treat (therapy, detox; never a diagnosed cancer): the nearest Clinic.
    pub clinic_ticks: u64,
    /// A Hunt's or a hit Contract's first `GoTo(Intel)`: the ask venue (past
    /// it, the hunter's own district's busiest Bar) or the stakeout.
    pub stakeout_ticks: u64,
    /// A sweeper's beat: the district's centroid from the post's door (past
    /// it, the nearest district with a slot left, else the post's own).
    pub beat_ticks: u64,
}

/// The island pass phase 2 `[distance.free]` (docs/ISLAND_PASS.md I1):
/// the caps of the free-time spot and the night's bed out (a Hotel or a
/// Mission's cot). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FreeDistanceCfg {
    /// The HangOut spot (a street corner, a Hideout's door, a friend's
    /// Home): past this from where the body sets out it is dropped, and with
    /// none within, no spot (the own Home is never past it).
    pub spot_cap_ticks: u64,
    /// Tonight's paid or charity bed (`street::hotel_for`): past this from
    /// where the body sets out it is dropped while one within it is left.
    pub hotel_cap_ticks: u64,
    /// A Mission's cot's offer over a paid Hotel bed, in travel ticks (it
    /// is free): the cot unless a Hotel is nearer by more than this.
    pub cot_bed_ticks: u64,
    /// Island fix round F3 (OPEN-190): the stay-in fallback. `None` (not
    /// written into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stay_in: Option<StayInCfg>,
}

/// Island fix round F3 `[distance.free.stay_in]` (docs/ISLAND_PASS.md I1,
/// Dylan's D1 B; OPEN-190): with no HangOut spot within `spot_cap_ticks`
/// the Unwind is the own Home's HangOut (`leisure::stay_in_home`), with
/// its flatmates and Block-mates, else alone. A placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StayInCfg {
    /// The fun of an hour in alone (the screen, the deck, the window); in
    /// company, the larger of this and a HangOut's `gain.hangout_hour`.
    pub fun_hour: f32,
}

/// The island fix round F2 `[beds]` (docs/ISLAND_PASS.md I6, I8; OPEN-189,
/// OPEN-191, OPEN-193). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BedsCfg {
    /// A company bed is taken (or kept, by a corp's worker of another of its
    /// workplaces) only by staff whose post's door is within this travel of
    /// the bed's door (`distance::travel`).
    pub company_post_ticks: u64,
    /// Tonight's Hideout bed (the nearer bed, the watch, lying low) for a
    /// housed member: only within this travel from where the body stands;
    /// past it, Home.
    pub hideout_walk_ticks: u64,
    /// ... and, for a member with a post, only with the Hideout's door
    /// within this travel of the post's door (tomorrow's commute); past it,
    /// Home.
    pub hideout_work_ticks: u64,
    /// A Builder's far-site day ([island_trades] site_day) that starts away
    /// from the Yard walks straight to the site (the base: to the Yard to
    /// load, then out).
    pub site_from_home: bool,
}

/// The island pass phase 8 `[island_trades]` (docs/ISLAND_PASS.md I4;
/// OPEN-168 part). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IslandTradesCfg {
    /// A corp's parcels go through the open Yard (with room) that has the
    /// most of the corp's staffed buildings within this travel of its door
    /// (`distance::travel`), those buildings the recipients; with none in
    /// reach of any Yard, the base pick (the Yard nearest the corp's first
    /// building, `[jobs2] site_reach`).
    pub parcel_cap_ticks: u64,
    /// The recipient buildings one corp's parcels go to (the base's six).
    pub parcel_stops_max: u8,
    /// A haul trip's estimate reads `distance::travel` for the carry (the
    /// base: the doors' Manhattan distance at the walk's speed).
    pub haul_travel: bool,
    /// A Construction Yard's Builder whose Yard has open sites but none in
    /// `[shift.haul] haul_reach` carries one load out to the nearest site and
    /// works the rest of the shift there (the base: the trips ran inside the
    /// Yard).
    pub site_day: bool,
    /// The labour corner stands outside every open Construction and Freight
    /// Yard (one within `[jobs2] corner_tiles` of another shares it), not
    /// one a district (the island's Yards sit 380 tiles from the Vats'
    /// lowest-id one).
    pub corner_per_yard: bool,
    /// An off-screen broke jobless past `[jobs2] corner_stat_reach` tiles of
    /// every corner queues at the corner nearest his Home by travel within
    /// this (on `p_corner`, as the base's within the tiles).
    pub corner_stat_ticks: u64,
    /// The cab drivers on shift at each Garage that keep a body (ranked with
    /// the gangs, `lod::class_with`), lowest id first.
    pub cab_quota: u8,
    /// A corner dealer whose gang holds an armoury surplus sells it at the
    /// gang's deal Bar (`stims::deal_bar`, the nearest Bar when the gang
    /// deals at a corner) as the base's bar dealer does.
    pub corner_arms: bool,
}

/// The pre-view pass phase 6 `[leaders]` (docs/PREVIEW_PASS.md § 0 P9,
/// P13-P15). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeadersCfg {
    /// P13: the round starts from this hour and is not begun from `round_last_hour`.
    pub round_hour: u16,
    pub round_last_hour: u16,
    /// P13: stops a round (the owed protections first).
    pub round_stops_max: u8,
    /// P13: an off-screen owner is met at his business at this rate.
    pub owner_stat_p: f32,
    /// P14: court opens at this hour and runs `court_min` minutes.
    pub court_hour: u16,
    pub court_min: u16,
    /// P14: an off-screen lieutenant attends a court at this rate.
    pub court_stat_p: f32,
    /// P14: the leader sets out to be at court by `court_hour`, at most
    /// this many minutes ahead.
    pub court_walk_max: u64,
    /// P15: an order waits for a lieutenant at court this long, then the
    /// leader walks to one; past `order_wait_ticks + order_word_ticks` it
    /// goes by word (off screen).
    pub order_wait_ticks: u64,
    pub order_word_ticks: u64,
    /// P15: the order is given in the lieutenant's building or within this
    /// Chebyshev radius of him.
    pub order_reach: u32,
    /// P15: minutes the order takes, given at the lieutenant.
    pub order_give_min: u16,
}

/// The pressure pass phase 2 `[customers]` (docs/PRESSURE_PASS.md § Phase 2;
/// `systems/customers.rs`). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomersCfg {
    /// The door: a visitor with a cause (drunk or high, a brawl within
    /// `door_brawl_days`, a rival gang's colours, a dress under
    /// `door_dress_min`) is turned away by a bouncer on duty on `door_p`.
    pub door_brawl_days: u32,
    pub door_dress_min: u8,
    /// A visitor carrying a weapon is a cause.
    pub door_armed: bool,
    pub door_p: f32,
    /// A turned-away drunk or brawler swings at the bouncer on
    /// `door_swing_p × (1 − lawfulness)`; the turned-away holds `door_grudge`.
    pub door_swing_p: f32,
    pub door_grudge: f32,
    /// Clients: intimacy under `lonely_intimacy` is lonely (0..1 below it);
    /// a venue with a companion, a host or a dancer on duty scores `× (1 +
    /// lonely_pull × lonely + misery_pull × misery)`; Unwind's need is
    /// `max(U(fun), lonely_unwind × lonely)` for such a pick, with
    /// `client_coins` in the purse.
    pub lonely_intimacy: f32,
    pub lonely_pull: f32,
    pub misery_pull: f32,
    pub lonely_unwind: f32,
    pub client_coins: i64,
    /// Arms: safety under `fear_safety` is a reason to want a gun (the
    /// frightened buy one at a dealer's Bar or, licensed, a Gunsmith).
    pub fear_safety: f32,
    /// Care: a lost fight within `hurt_hours` is a wound to have seen to: a
    /// Clinic's patch-up at `patch_price`, or the back room's at
    /// `back_price` for the wanted, a gang member, or one short of the
    /// Clinic's price; `patch_min` minutes on the table.
    pub hurt_hours: u32,
    pub patch_price: i64,
    pub back_price: i64,
    pub patch_min: u32,
    /// Care: a diagnosed cancer whose pool falls short of the Clinic's
    /// treatment buys the back room's at `back_onco_price`: a remission of
    /// `back_onco_mult ×` the Clinic's, never a cure.
    pub back_onco_price: i64,
    pub back_onco_mult: f32,
    /// The back room's founder: while fewer stand than one per
    /// `back_room_adults` adults, a struck-off doc (else the least lawful
    /// licensed Ripperdoc under `rogue_lawful`, who leaves his Clinic for
    /// it) opens one: a vacant Lot he can pay for, else a derelict refitted.
    pub back_room_adults: u32,
    pub rogue_lawful: f32,
    /// A back room is walked to from within `back_reach_ticks` of travel.
    pub back_reach_ticks: u64,
    /// The cab: a body's GoTo of `hail_min_tiles` or more, not to work,
    /// hails a cab once a day on `p_hail` when it holds `hail_rich_coins`
    /// or is hurried (Treat, Flee, Rescue), with the fare.
    pub hail_min_tiles: u32,
    pub hail_rich_coins: i64,
    pub p_hail: f32,
    /// A cab driver's body is paid `min(1, fares / cab_fares_full)` of the
    /// day's wage (no fare, no pay for Rest at the Garage).
    pub cab_fares_full: u16,
    /// The shape-up: every open Construction and Freight Yard posts
    /// `shape_up_posts` day posts at the dawn pick; a broke jobless body
    /// walks to a corner within `shape_up_reach` tiles (`[jobs2] corner_reach` off).
    pub shape_up_posts: u8,
    pub shape_up_reach: u32,
    /// The vendor: a working stall sells up to `vendor_per_hour` meals an
    /// hour to bodies on the street within `vendor_tiles` with hunger under
    /// `vendor_hunger`.
    pub vendor_tiles: u32,
    pub vendor_per_hour: u8,
    pub vendor_hunger: f32,
    /// The scav's day: a scav gang's member walks `scav_reach` tiles to a
    /// Market's bin on the gang's round (off hours), or to a fresh body.
    pub scav_reach: u32,
}

/// The pressure pass phase 1 `[misery]` (docs/PRESSURE_PASS.md § Phase 1,
/// OPEN-178; `systems/misery.rs`). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MiseryCfg {
    /// Misery is `clamp((mood_line − mood) / mood_span, 0, 1)`.
    pub mood_line: f32,
    pub mood_span: f32,
    /// Absenteeism: a day's skip chance at misery 1, `× (1 − skip_lawful_damp
    /// × lawfulness) × (1 − skip_loyal_damp × loyalty)`.
    pub skip_p: f32,
    pub skip_lawful_damp: f32,
    pub skip_loyal_damp: f32,
    /// Absences counted over this many days; at `fire_absences` the worker is dismissed.
    pub absence_window_days: u32,
    pub fire_absences: u32,
    /// Quitting: consecutive midnights below `quit_mood` while employed.
    pub quit_mood: f32,
    pub quit_days: u16,
    /// Less `quit_commute_days` past this commute (ticks, home door to work door).
    pub quit_commute_ticks: u64,
    pub quit_commute_days: u16,
    /// Less `quit_pay_days` for a day's wage under this many meals.
    pub quit_pay_meals: f32,
    pub quit_pay_days: u16,
    /// Less `quit_bed_days` in a company bed.
    pub quit_bed_days: u16,
    pub quit_min_days: u16,
    /// A day's quit chance once the days are reached.
    pub quit_p: f32,
    /// Petty crime: the planner's steal costs `− steal_cost_cut × misery`;
    /// the vehicle theft's lawfulness gate `+ steal_lawful_cut × misery`.
    pub steal_cost_cut: f32,
    pub steal_lawful_cut: f32,
    /// Off screen: the hour's `p_steal × (1 + stat_steal_mult × misery)`.
    pub stat_steal_mult: f32,
    /// Fights: misery past `brawl_min`, drunk or high, an hour's chance at
    /// misery 1 `brawl_p × (1 − brawl_lawful_damp × lawfulness)`.
    pub brawl_min: f32,
    pub brawl_p: f32,
    pub brawl_lawful_damp: f32,
    /// A drink (a `Socialised` memory) within this many hours is drunk.
    pub drink_hours: u32,
    /// The street brawl's reach (tiles).
    pub brawl_reach: u32,
    /// The brawl's death chance multiplier (`FightMods.kill_mult`).
    pub brawl_kill_mult: f32,
    /// The struck's grudge on the starter; `× brawl_grudge_back` the starter's on a struck winner.
    pub brawl_grudge_w: f32,
    pub brawl_grudge_back: f32,
    /// Off screen: a day's brawl at misery 1 (`× (1 − brawl_lawful_damp ×
    /// lawfulness)`), a report filed on `stat_brawl_report_p`.
    pub stat_brawl_p: f32,
    pub stat_brawl_report_p: f32,
    /// Recruitment: JoinGang `+ join_flat × misery`; the edge's affinity
    /// `− join_affinity_cut × misery`; past `join_min` misery and under
    /// `join_lawfulness` taken off the street; the join's event names misery
    /// past `join_note_min`; off screen `stat_join_p × misery` a day.
    pub join_flat: f32,
    pub join_affinity_cut: f32,
    pub join_min: f32,
    pub join_lawfulness: f32,
    pub join_note_min: f32,
    pub stat_join_p: f32,
    /// The faith: a Preach draws past `faith_min` misery; `p_convert × (1 +
    /// faith_convert_mult × misery)`.
    pub faith_min: f32,
    pub faith_convert_mult: f32,
    /// F3 (quits, wages and the jobless): a body's skip is acted out at the
    /// first hour within `skip_lead_ticks` of the shift's start (it sets off
    /// or wakes, then does not go); the Unwind goal scores `+ skip_unwind_flat`
    /// until the skipped shift's end (a bar, a friend's, the street).
    pub skip_lead_ticks: u16,
    pub skip_unwind_flat: f32,
    /// F3: out of every hire for `quit_idle_days` after a quit (misery, unpaid)
    /// and `fired_idle_days` after a firing, `+` up to `idle_jitter_days`
    /// (a pure hash); the firing employer refuses him for `fired_block_days`.
    pub quit_idle_days: u32,
    pub fired_idle_days: u32,
    pub idle_jitter_days: u32,
    pub fired_block_days: u32,
    /// F3: the reputation: for `stain_days` a hire pass passes the fired over
    /// on `stain_fired_p`, a quitter on `stain_quit_p` (a hash of the day).
    pub stain_days: u32,
    pub stain_fired_p: f32,
    pub stain_quit_p: f32,
    /// F3: an unpaid quitter's `Betrayed` grudge on a boss who is a person
    /// (the wages owed stay an edge debt, `social::repay_debts`).
    pub stiff_grudge: f32,
    /// F3: a gang recruit's initiation: no walk-away (`gang::desist`) in its
    /// first `recruit_tenure_days` (every recruit).
    pub recruit_tenure_days: u32,
}

/// The pressure pass phase 6 `[underwater]` (docs/PRESSURE_PASS.md § Phase 6;
/// `systems/underwater.rs`): a person-owned business whose till cannot meet
/// its wages becomes a story with choices (cut back, borrow, sell, close),
/// and a worker walked out unpaid comes back for his wages. Absent: none of
/// it. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnderwaterCfg {
    /// Under water: the owner's purse under `short_cover_days` of the day's
    /// wage bill (the staff outside his household), or one of them owed
    /// `short_unpaid_days` days.
    pub short_cover_days: f32,
    pub short_unpaid_days: u8,
    /// The owner decides after `decide_days` short midnights in a row, and
    /// again no sooner than `decide_cooldown_days` later.
    pub decide_days: u16,
    pub decide_cooldown_days: u16,
    /// The till back over `recover_days` of the bill: afloat (the freeze lifts).
    pub recover_days: f32,
    /// The choice weights (`x the owner's personality`, see the module):
    /// hang on, shorter hours, lay staff off, borrow, sell, close.
    pub w_hold: f32,
    pub w_hours: f32,
    pub w_layoff: f32,
    pub w_borrow: f32,
    pub w_sell: f32,
    pub w_close: f32,
    /// Each earlier decision multiplies sell and close by `1 + escalate x n`.
    pub escalate: f32,
    /// Shorter hours: each shift and its wage cut by this share.
    pub hours_cut: f32,
    /// A layoff lets this share of the staff go (at least one), newest first.
    pub layoff_share: f32,
    /// A loan: `loan_days` of the wage bill, repaid over `loan_weeks` weekly
    /// installments with the lender's rate on top.
    pub loan_days: u32,
    pub loan_weeks: u32,
    /// A lender lends what it can spare up to `loan_days` of the bill, never
    /// under `loan_min_days` of it.
    pub loan_min_days: u32,
    pub rate_shark: f32,
    pub rate_gang: f32,
    pub rate_corp: f32,
    /// A loan shark: a free adult under `shark_lawful_max` holding
    /// `shark_wealth_mult x` the principal (the owner's district first).
    pub shark_lawful_max: f32,
    pub shark_wealth_mult: f32,
    /// A corp lends only to an owner at least `corp_lawful_min` lawful, from
    /// its treasury over its reserve by `corp_spare_mult x` the principal.
    pub corp_lawful_min: f32,
    pub corp_spare_mult: f32,
    /// A gang lends or buys from its treasury over `gang_reserve`.
    pub gang_reserve: i64,
    /// Missed installments: the gang's or the shark's collector comes; at
    /// `seize_misses` the lender takes the business; a corp repossesses at
    /// `corp_repo_misses`. The collector's beating kills on `x collector_kill_mult`.
    pub seize_misses: u8,
    pub corp_repo_misses: u8,
    pub collector_kill_mult: f32,
    /// A fire sale at `sell_price_mult x` the building's value; an owner over
    /// `front_lawful_max` lawfulness sells to no gang unless past `front_misery`.
    pub sell_price_mult: f32,
    /// A gang buys a front at `front_price_mult x` the value (muscle, not money).
    pub front_price_mult: f32,
    pub front_lawful_max: f32,
    pub front_misery: f32,
    /// A bought front: its staff join the gang on `front_join_p x (1 -
    /// lawfulness)` a day; `launder_share` of the gang's week's tribute is
    /// booked through its till (its weekday); a rival holding turf in its
    /// district hits it on `rival_hit_p` a week, taking up to `rival_take`
    /// from the gang and shutting it `rival_close_days`.
    pub front_join_p: f32,
    pub launder_share: f32,
    pub rival_hit_p: f32,
    pub rival_take: i64,
    pub rival_close_days: u32,
    /// A shut business stands for sale at `shut_price_mult x` its value to a
    /// would-be founder holding `buyer_reserve_days` of its wage bill on top;
    /// `shut_buys_per_day` a day; a Bar or Hotel unsold `shut_derelict_days`
    /// goes derelict.
    pub shut_price_mult: f32,
    pub buyer_reserve_days: u32,
    pub shut_buys_per_day: u32,
    pub shut_derelict_days: u32,
    /// The stiffed worker (an unpaid walkout or a layoff owed wages): from
    /// `confront_wait_days` after, on `confront_p x (0.5 + pride + misery)` a
    /// day, he goes to the owner in person (a Visit between the hours) at
    /// most `confront_max` times.
    pub confront_wait_days: u32,
    pub confront_p: f32,
    pub confront_max: u8,
    pub confront_hour_from: u8,
    pub confront_hour_to: u8,
    /// The Visit goal's urgency for the confrontation and the collector's call.
    pub confront_urgency: f32,
    pub dun_urgency: f32,
    /// The owner pays (what he holds, up to the owed) on `pay_base + pay_lawful
    /// x lawfulness + pay_loyal x loyalty - pay_greed x greed`.
    pub pay_base: f32,
    pub pay_lawful: f32,
    pub pay_loyal: f32,
    pub pay_greed: f32,
    /// Refused: the worker's grudge grows by `confront_grudge`; at
    /// `fight_grudge` he swings (`x confront_kill_mult`); under it, a worker
    /// at least `report_lawful` reports the owner to the Hall, which takes
    /// the owed out of the owner's purse as it comes in.
    pub confront_grudge: f32,
    pub fight_grudge: f32,
    pub confront_kill_mult: f32,
    pub report_lawful: f32,
    /// The beaten owner reports the collector (or the worker) at this lawfulness.
    pub victim_report_lawful: f32,
}

impl Default for UnderwaterCfg {
    fn default() -> Self {
        UnderwaterCfg {
            short_cover_days: 1.0,
            short_unpaid_days: 2,
            decide_days: 2,
            decide_cooldown_days: 5,
            recover_days: 3.0,
            w_hold: 0.5,
            w_hours: 1.0,
            w_layoff: 1.0,
            w_borrow: 1.0,
            w_sell: 0.6,
            w_close: 0.4,
            escalate: 0.6,
            hours_cut: 0.35,
            layoff_share: 0.5,
            loan_days: 14,
            loan_weeks: 4,
            loan_min_days: 3,
            rate_shark: 0.6,
            rate_gang: 0.4,
            rate_corp: 0.15,
            shark_lawful_max: 0.35,
            shark_wealth_mult: 2.0,
            corp_lawful_min: 0.4,
            corp_spare_mult: 3.0,
            gang_reserve: 20,
            seize_misses: 3,
            corp_repo_misses: 2,
            collector_kill_mult: 0.2,
            sell_price_mult: 0.6,
            front_price_mult: 0.25,
            front_lawful_max: 0.5,
            front_misery: 0.6,
            front_join_p: 0.1,
            launder_share: 0.3,
            rival_hit_p: 0.15,
            rival_take: 30,
            rival_close_days: 2,
            shut_price_mult: 0.5,
            buyer_reserve_days: 7,
            shut_buys_per_day: 2,
            shut_derelict_days: 21,
            confront_wait_days: 1,
            confront_p: 0.3,
            confront_max: 3,
            confront_hour_from: 10,
            confront_hour_to: 20,
            confront_urgency: 0.8,
            dun_urgency: 0.9,
            pay_base: 0.2,
            pay_lawful: 0.5,
            pay_loyal: 0.3,
            pay_greed: 0.4,
            confront_grudge: 0.2,
            fight_grudge: 0.5,
            confront_kill_mult: 0.2,
            report_lawful: 0.5,
            victim_report_lawful: 0.4,
        }
    }
}

/// The pressure pass phase 3 `[protection]` (docs/PRESSURE_PASS.md § Phase
/// 3; `systems/protection.rs`, `leverage::protection_pass`): protection
/// keyed to held ground, refusals punished in person, the owner's choice
/// after, and the gang's purse (bail, the enforcer's cut). Absent: M16b's
/// protection (Expand/Contest weekday, the turf's Homes only). Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProtectionCfg {
    /// The Hideout's district is held ground (a gang leans where it lives).
    pub hideout_ground: bool,
    /// Businesses leaned on per held district per weekly pass.
    pub lean_per_district: u8,
    /// The weekly levy: `levy_share` of the business's week's revenue, at
    /// least `levy_min` (a business taking less is not leaned on), at most
    /// `[leverage] protection_weekly`.
    pub levy_share: f32,
    pub levy_min: i64,
    /// Missed weeks (nobody met at a round stop, or the till short) before
    /// the gang punishes; a refused lean is punished at once.
    pub punish_misses: u8,
    /// The enforcer calls between these hours (open hours).
    pub punish_hour_from: u8,
    pub punish_hour_to: u8,
    /// The enforcer's Visit urgency.
    pub punish_urgency: f32,
    /// A smashed till: this many weeks of the levy taken from the payer's
    /// purse (what it holds of them) for the gang.
    pub smash_weeks: f32,
    /// A smashed till shuts the business this many days; a fire this many.
    pub smash_close_days: u16,
    pub fire_close_days: u16,
    /// The beating kills on `[crime] fight_death_p x` this.
    pub beat_kill_mult: f32,
    /// The enforcer keeps this share of what he takes; the rest is the gang's.
    pub enforcer_cut: f32,
    /// The punished owner's grudge on the enforcer.
    pub grudge: f32,
    /// A punished owner this lawful files the report himself (the choice's
    /// report is the weighted one).
    pub victim_report_lawful: f32,
    /// The owner's choice after a punishment, one weighted draw: pay
    /// `w_pay x (1.5 - courage)`, report `w_report x lawfulness`, a rival's
    /// cover `w_rival x (0.5 + courage)` (a rival gang with a leader), sell
    /// or close `w_sell x (0.25 + misery) x punishments` (with `[underwater]`).
    pub w_pay: f32,
    pub w_report: f32,
    pub w_rival: f32,
    pub w_sell: f32,
    /// A gang loses this much standing grudge on the rival that gave cover
    /// (its leader's grudge on the rival's leader).
    pub cover_grudge: f32,
    /// Bail: daily, a gang with the treasury frees a jailed member at
    /// `bail_per_day` a day of his sentence left, keeping `bail_reserve`;
    /// not past `bail_max_days` left.
    pub bail_per_day: i64,
    pub bail_reserve: i64,
    pub bail_min_days: u32,
    pub bail_max_days: u32,
    /// The daily stipend goes only to members with no wage post; the
    /// employed are paid at the weekly Collect (their cut of the take).
    pub stipend_jobless_only: bool,
    /// The stipends come out of the treasury above this war chest (a gang
    /// below it pays nobody daily, and its unpaid drift away).
    pub stipend_reserve: i64,
    /// OPEN-157: a contract's party the Fixer cannot meet now is sent there
    /// in person at its next open hour (`service::send_broker`).
    pub broker_visits: bool,
}

impl Default for ProtectionCfg {
    fn default() -> Self {
        ProtectionCfg {
            hideout_ground: true,
            lean_per_district: 3,
            levy_share: 0.15,
            levy_min: 10,
            punish_misses: 2,
            punish_hour_from: 10,
            punish_hour_to: 20,
            punish_urgency: 0.9,
            smash_weeks: 2.0,
            smash_close_days: 1,
            fire_close_days: 7,
            beat_kill_mult: 0.2,
            enforcer_cut: 0.25,
            grudge: 0.5,
            victim_report_lawful: 0.6,
            w_pay: 1.0,
            w_report: 0.6,
            w_rival: 0.5,
            w_sell: 0.3,
            cover_grudge: 0.4,
            bail_per_day: 5,
            bail_reserve: 100,
            bail_min_days: 2,
            bail_max_days: 20,
            stipend_jobless_only: true,
            stipend_reserve: 150,
            broker_visits: true,
        }
    }
}

/// The pre-view pass phase 5 `[legs]` (docs/PREVIEW_PASS.md § 2 "20k
/// legs"). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegsCfg {
    /// A Full or Coarse walking `GoTo` farther than this (Manhattan tiles)
    /// walks in segments of this length, each end re-planning the rest
    /// (the ride re-checked).
    pub leg_replan_tiles: u32,
    /// A segment's end snaps to a walkable Road tile within this radius.
    pub segment_snap: u32,
    /// P12: an escort walks (at walking speed) up to this many tiles; past
    /// it the van.
    pub escort_walk_tiles: u32,
    /// P12: the van's arrival at the arrest, in ticks, before the drive.
    pub van_call_ticks: u32,
    /// OPEN-140 remainder: an exec driven to his Hq is driven home too.
    pub exec_ride_home: bool,
    /// OPEN-140 remainder: the away-from-home bed reads the trip home by
    /// ride (the El, the exec's car), not the walk.
    pub bed_by_ride: bool,
    /// OPEN-156: the sweeper's walk back to the Recycler reads the ride.
    pub sweep_by_ride: bool,
    /// The `fares` shape (OPEN-115): a booked cab rider waits at his door
    /// for a driver's body that can reach him within this (cab speed);
    /// farther, the Garage's off-screen cab.
    pub fare_wait_ticks: u32,
    /// The `fares` shape: a commute of `[jobs2] cab_min_tiles` or more that
    /// the El does not shorten takes the cab when the purse allows (no
    /// `p_cab` roll), both tiers (`streetwork::cab_roll`).
    pub fares_el_missed: bool,
    /// The island pass phase 4 (OPEN-164): the plan timeout (`[brain]
    /// plan_timeout_ticks`) spares a leg in motion (a timed walk, a flight
    /// or a ride before its arrival, a walk with its path left and not
    /// blocked). False (and not written into a save) with
    /// the key absent: the base timeout, which cut a
    /// 931-tick ride at 900 and left the body at the board stop.
    #[serde(default, skip_serializing_if = "is_false")]
    pub timeout_spares_legs: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// The pre-view pass phase 4 `[presence]` (docs/PREVIEW_PASS.md § 0 P3):
/// an encounter posts when, and only when, both bodies are present. Every
/// value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresenceCfg {
    /// OPEN-154: a round's stops at most this many tiles from the post's door.
    pub round_reach_tiles: u32,
    /// A Statistical counterpart at home (not on shift, not the night) is
    /// drawn present at this rate (purpose-keyed).
    pub stat_home_p: f32,
    /// P8: the sweeper's round: street stops a shift (dirtiest first, within
    /// `round_reach_tiles` of the first).
    pub sweep_stops: u8,
    /// P8: minutes swept at each stop.
    pub sweep_stop_min: u16,
    /// P8: a stop cleans the street tiles within this Chebyshev radius.
    pub sweep_radius: u8,
    /// P8: a round's walk is estimated at the stops' Manhattan tiles times
    /// this (the street grid is no straight line).
    pub sweep_walk_factor: f32,
    /// An off-screen arms buyer's meeting waits this many hours for the dealer's body at the Bar.
    pub arms_wait_hours: u16,
}

/// The pre-view pass phase 2 (docs/PREVIEW_PASS.md § 0 P1): the shape a
/// shift is worked in. Phases 3-6 append `haul`, `counter` (and work
/// `round`/`fare` through `systems/shift.rs`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShiftShape {
    /// The one work block at the post, the shift's whole length (the old shift).
    #[default]
    Post,
    /// § 1 P2: 2-`station_spots_max` spots inside the workplace, worked in
    /// `station_block_min`..`station_block_max` minute blocks, one break.
    /// Saved as `spots` (an off `[law] stations` city's save holds no
    /// `station`); `stations` in the file.
    #[serde(rename = "spots", alias = "stations")]
    Stations,
    /// The pre-view pass phase 3 (§ 1): trips from the workplace's stock
    /// tile to a destination (a Market, a Warehouse, an open site, a
    /// parcel's address) and back, `[shift.haul]`; the courier's parcels.
    Haul,
    /// The pre-view pass phase 3 (§ 1): the counter or the door tile stood,
    /// arrivals served as they arrive, `[shift.counter]`.
    Counter,
}

/// The pre-view pass phase 3 `[shift.haul]` (docs/PREVIEW_PASS.md § 1).
/// `None` drops the shape. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HaulCfg {
    /// Units a trip carries (the device row; pay and output do not move, P5).
    pub haul_load: u16,
    /// No new trip starts within this many minutes (its estimate added) of
    /// the shift's end: the rest is worked at the post, where it closes.
    pub haul_cutoff_min: u16,
    /// The load at the stock tile and the unload at the destination, minutes each.
    pub haul_handle_min: u16,
    /// A destination's door at most this many tiles from the post's door.
    pub haul_reach: u32,
    /// The most destinations a shift's trips turn through.
    pub haul_dests_max: u8,
    /// The trade ids that drive the post's fleet truck when one is parked
    /// there (`vehicles::claim_haul`); the rest take what the GoTo finds.
    #[serde(default)]
    pub drive: Vec<String>,
}

/// The pre-view pass phase 3 `[shift.counter]` (docs/PREVIEW_PASS.md § 1).
/// `None` drops the shape. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CounterCfg {
    /// An off-screen visit to a venue with a counter body is served in the
    /// `counter_settle_window`-minute slot it arrives in, through the body's
    /// shift, not in the 21:00 evening batch.
    pub counter_settle_window: u16,
    /// The built-in roles (by the `Role`'s name) that stand the counter,
    /// over `[shift.roles]` (the Cook, OPEN-145). `counter_staff` in a save
    /// (an off `[careers]` city's save holds no `roles:`).
    #[serde(default, rename = "counter_staff", alias = "roles")]
    pub roles: Vec<String>,
}

/// The pre-view pass phase 2 `[shift]` (docs/PREVIEW_PASS.md § 1). Every
/// value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShiftCfg {
    /// P2: a stations block's length in minutes (a trade row may override both).
    // Saved as `block_min`, `block_max`, `spots_max`, `move_max` (an off
    // `[law] stations` city's save holds no `station`); the file's names read.
    #[serde(rename = "block_min", alias = "station_block_min")]
    pub station_block_min: u16,
    #[serde(rename = "block_max", alias = "station_block_max")]
    pub station_block_max: u16,
    /// P2: the most distinct spots a shift works (at least 2, at most the
    /// workplace's interior tiles).
    #[serde(rename = "spots_max", alias = "station_spots_max")]
    pub station_spots_max: u8,
    /// P2: the walk across the room between two spots, at most this many
    /// minutes (the tiles at `[exec] move_ticks_full` otherwise).
    #[serde(rename = "move_max", alias = "station_move_max")]
    pub station_move_max: u16,
    /// P7: the mid-shift break in minutes.
    pub break_min: u16,
    /// P7: the break's start, staggered per body per shift by up to this
    /// many minutes either side of mid-shift.
    pub break_stagger: u16,
    /// P7: a hungry worker's meal break walks to a food venue at most this
    /// many minutes away (one way); farther, the break is outside the door.
    pub break_walk_max: u16,
    /// P1: the shape of a trade whose `duty` is `post`.
    pub trade_post: ShiftShape,
    /// P1: the built-in roles' shapes, keyed by the `Role`'s name (absent:
    /// `post`); `[shift.roles]` in the file, `role_shapes` in a save (an off
    /// `[careers]` city's save holds no `roles:`).
    #[serde(default, rename = "role_shapes", alias = "roles")]
    pub roles: std::collections::BTreeMap<String, ShiftShape>,
    /// The pre-view pass phase 3: the `haul` shape's numbers (`None` off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haul: Option<HaulCfg>,
    /// The pre-view pass phase 3: the `counter` shape's numbers (`None` off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<CounterCfg>,
}

/// Lives at work phase 6 `[careers]` (docs/LIVES_AT_WORK.md § 4.1-4.4).
/// Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CareersCfg {
    pub enabled: bool,
    /// § 4.2: a seeded Job's seniority is `min(years over the row's
    /// min_age, this) × DAYS_PER_YEAR × u` days.
    pub seniority_cap_years: u32,
    /// § 4.2: the foreman is the most experienced worker this old or older.
    pub foreman_min_age: u32,
    /// § 4.2: the foreman's wage multiplier.
    pub foreman_premium: f32,
    /// § 4.3: shifts worked in a trade before it is the profession.
    pub profession_min_days: u32,
    /// § 4.3: a candidate holding the vacancy's profession ranks this many
    /// tiles nearer (the hiring key's offset, capped with skill and the
    /// rehire bonus at 512).
    pub profession_w: u32,
    /// § 4.4: a worker this old retires on savings or kin ...
    pub retire_from_years: u32,
    /// ... when the household's coins cover this many days of its costs.
    pub retire_days: u32,
    /// § 4.4: a worker whose `ages::work_mult` falls under this is let go.
    pub frail_exit_mult: f32,
    /// § 4.4: a retired agent under this many coins is retired no more.
    pub unretire_coins: i64,
}

impl Default for CareersCfg {
    fn default() -> Self {
        CareersCfg {
            enabled: false,
            seniority_cap_years: 30,
            foreman_min_age: 30,
            foreman_premium: 1.25,
            profession_min_days: 14,
            profession_w: 400,
            retire_from_years: 62,
            retire_days: 60,
            frail_exit_mult: 0.6,
            unretire_coins: 20,
        }
    }
}

impl CareersCfg {
    /// Not written into a save at its defaults (off).
    pub fn is_default(&self) -> bool {
        *self == CareersCfg::default()
    }
}

/// Lives at work phase 6 `[roles.<Name>]` (§ 4.1): a legacy role's age band
/// in years (0 none).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoleCfg {
    pub min_age: u32,
    pub max_age: u32,
}

/// Lives at work phase 2 `[household]` (docs/LIVES_AT_WORK.md § 3): the
/// home spot, the empty-spot leave, the household's rules and the visits.
/// Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HouseholdCfg {
    pub enabled: bool,
    /// § 3.1: a Home spot's score per household member home now, and per
    /// child at home.
    pub home_w: f32,
    pub child_w: f32,
    /// § 3.2: Friend/Family contacts' Homes offered as spots, at most.
    pub visit_hosts: u8,
    /// § 3.1: street dice between two bored co-hangers at home (off: none).
    pub home_dice: bool,
    /// § 3.3: a HangOut alone this long ends early; the spot scores ×
    /// `empty_penalty` for `empty_memory_hours`.
    pub empty_leave_ticks: u16,
    pub empty_penalty: f32,
    pub empty_memory_hours: u8,
    /// § 3.4: the newborn's days; the carer's away spots and night goals ×
    /// this; both parents' energy decay × this; the Home's pantry draw a
    /// day (Food, on the child food debt).
    pub newborn_days: u16,
    pub newborn_away_mult: f32,
    pub newborn_energy_mult: f32,
    pub newborn_cost_per_day: f32,
    /// § 3.4: the carer's night, hours of the day `[from, to)` (wrapping).
    pub night_from: u8,
    pub night_to: u8,
    /// § 3.4: a household business with no take for this many days is
    /// abandoned (its heir takes an outside post).
    pub abandon_days: u8,
    /// § 3.5: an elder parent unmet this long is visited; the visit's hour
    /// of the day `[from, to)`; the goal's flat bonus; the visitor's belonging.
    pub elder_visit_days: u8,
    pub visit_hour_from: u8,
    pub visit_hour_to: u8,
    pub visit_flat: f32,
    pub visit_belonging: f32,
    /// § 3.5 (review fix 8, doctrine 3): the Visit goal's reason by purpose
    /// and its curve. Left out of a save at its defaults (the shipped values).
    #[serde(default, skip_serializing_if = "VisitUrgencyCfg::is_default")]
    pub visit_urgency: VisitUrgencyCfg,
}

/// `[household.visit_urgency]`: the Visit goal's "the reason" consideration,
/// by the visit's purpose, through `Linear { m: curve.0, b: curve.1 }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisitUrgencyCfg {
    /// A kin downed, assaulted or arrested.
    pub hurt: f32,
    /// An elder parent unmet for `elder_visit_days`.
    pub elder: f32,
    /// A child taken to a Camp (the parent's answer).
    pub camp: f32,
    /// The Fixer's walk-in pickup (phase 5).
    pub papers: f32,
    /// The Fixer owner's weekly collection (phase 5).
    pub collect: f32,
    /// The curve's `(m, b)`.
    pub curve: (f32, f32),
}

impl Default for VisitUrgencyCfg {
    fn default() -> Self {
        VisitUrgencyCfg { hurt: 1.0, elder: 0.6, camp: 0.8, papers: 0.9, collect: 0.5, curve: (0.5, 0.5) }
    }
}

impl VisitUrgencyCfg {
    pub fn is_default(&self) -> bool {
        *self == VisitUrgencyCfg::default()
    }
}

impl Default for HouseholdCfg {
    fn default() -> Self {
        HouseholdCfg {
            enabled: false,
            home_w: 0.5,
            child_w: 0.3,
            visit_hosts: 3,
            home_dice: false,
            empty_leave_ticks: 20,
            empty_penalty: 0.3,
            empty_memory_hours: 6,
            newborn_days: 30,
            newborn_away_mult: 0.3,
            newborn_energy_mult: 1.15,
            newborn_cost_per_day: 0.5,
            night_from: 21,
            night_to: 6,
            abandon_days: 7,
            elder_visit_days: 7,
            visit_hour_from: 10,
            visit_hour_to: 20,
            visit_flat: 0.2,
            visit_belonging: 0.2,
            visit_urgency: VisitUrgencyCfg::default(),
        }
    }
}

impl HouseholdCfg {
    /// Not written into a save at its defaults (off).
    pub fn is_default(&self) -> bool {
        *self == HouseholdCfg::default()
    }
}

/// Lives at work `[lives]` (docs/LIVES_AT_WORK.md D13): the master switch.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivesCfg {
    /// Off (the default when the table is absent): nothing of the milestone runs.
    pub enabled: bool,
}

impl LivesCfg {
    /// Not written into a save while off.
    pub fn is_off(&self) -> bool {
        !self.enabled
    }
}

/// Lives at work phase 1 `[service]` (docs/LIVES_AT_WORK.md § 1, § 2.1):
/// the encounter's sub-switches (one per phase; `civic`, `street` and
/// `tower` are the later phases') and numbers. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceCfg {
    /// Phase 1: venues, vice and the Clinic.
    pub venues: bool,
    /// Phase 3 (§ 2.2, § 3.5): the civic counterparts (teacher, camp
    /// warden, jailer, super), the near round, tied housing and the parent's
    /// answer to a taken child (`[service] civic = false`).
    pub civic: bool,
    /// Phase 4 (§ 2.3): the street and freight: the cab roll shared by both
    /// tiers and the driver's body, the parcel, the lift, the client, the
    /// pimp's round, the dealer's corner and regulars
    /// (`[service] street = false`; not Jobs & lives phase 2's `[jobs2]
    /// street` switch, the pitches and rounds themselves).
    pub street: bool,
    /// Phase 5 (§ 2.4, § 2.2's civic clerk, § 3.6's `Decided`): the
    /// exec's Meeting and decision, the reporter's beat and byline, the
    /// Fixer's walk-in and contract encounters, the owner's collection, the
    /// clerk named (`[service] tower = false`).
    pub tower: bool,
    /// § 2.1: the madam's share of her worker's tips at her Brothel (worker -> madam), of the shift's total at shift end (8d, OPEN-123).
    pub madam_cut: f32,
    /// § 2.1: an Assault inside a kept venue is an ejection on this × (the
    /// bouncer's fighting − the attacker's), the attacker put at the door.
    pub p_eject: f32,
    /// § 2.1: a Cook takes one meal from the NoodleBar's stock at shift end
    /// (every tier).
    pub staff_meal: bool,
    /// D5: a `Served` memory's salience (its mood weight is 0 regardless).
    pub served_salience: f32,
    // --- Phase 3 (civic), each left out of a save at its default so the
    // off save is byte-identical (`ServiceCfg::strip_civic`).
    /// § 2.2 (warden): a Camp child-day's chance of a beating (a
    /// `ChildTaken`-class grief to its parents).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub p_camp_beating: f32,
    /// § 2.2 (warden): a Camp child-day's chance of an escape (the child
    /// back to a parent's Home with room, else the street).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub p_camp_escape: f32,
    /// § 3.5: the buy-out's fee to the Camp's owner (`Flow::Buyout`).
    #[serde(skip_serializing_if = "is_zero_i64_cfg")]
    pub buyout_fee: i64,
    /// § 3.5: the Home's pantry must hold this many days of the child's
    /// food (`demography::CHILD_FOOD_PER_DAY`) for the buy-out.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub buyout_food_days: f32,
    /// § 3.5: a gang member's break-out needs this many of his gang alive
    /// and free.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub breakout_crew: u8,
    /// § 3.5: a parent of a Camp child with no `last_met` with it in this
    /// many days is counted neglecting (printed, never a camp-take term).
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub neglect_days: u16,
    /// § 2.2 (teacher): a taught School day adds `teacher_floor + (1 −
    /// teacher_floor) × knowledge` to `Child.edu_taught`.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub teacher_floor: f32,
    // --- Phase 4 (the street), each left out of a save at its default
    // (`ServiceCfg::strip_street`).
    /// § 2.3 (cab): a rider waits this long at the door for a driver's body;
    /// farther, the Garage's off-screen cab takes the ride (the leg timed).
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub cab_wait_ticks: u32,
    /// § 2.3 (street worker): a client is a body out walking within this
    /// many tiles of the pitch before the district's hashed draw.
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub client_tiles: u32,
    /// § 2.3 (pimp): a pitch is protected by a pimp's body within this many
    /// tiles (a Statistical pimp credited by the clock in pitch hours).
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub pimp_tiles: u32,
    /// § 2.3 (pimp): the share of his cut the pimp passes up to the gang.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub pimp_pass_up: f32,
    /// § 2.3 (pimp): a pimp absent from every pitch of his for this many
    /// days loses the district (`name_pimps`).
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub pimp_absent_days: u16,
    /// § 2.3 (loader): after a pilfer, the owner reports a Theft on this
    /// chance (a hash of the loader and the day).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub p_short_report: f32,
    /// § 2.3 (dealer): a lookout at the corner or in the Bar multiplies the
    /// Dealing witness roll by this.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub witness_lookout_mult: f32,
    /// § 2.3 (cab): a `split` fare-duty trade's shifts under the street
    /// switch, one a driver in turn by id (the commute hours: the `split`
    /// day/night pair is on duty at neither). Empty: the `split` rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cab_shifts: Vec<(u16, u16)>,
    /// OPEN-111 (§ 14 phase 1, "Who is behind the counter"), Shadow V5 bug
    /// 1: the legacy venue roles whose staff cover the venue's trade hours,
    /// split at each venue between `[world] shift_day` and `[leisure]
    /// evening_shift` (the thinner first, the evening on a tie), instead of
    /// all on `[world] shift_day`. Read under `[service] venues`
    /// (`service::venue_shift`); empty, every legacy role's shift as before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evening_staff: Vec<crate::components::Role>,
    // --- Phase 5 (the tower), each left out of a save at its default
    // (`ServiceCfg::strip_tower`).
    /// § 2.4 (exec): the graded staff present at the Hq an exec meets in a
    /// Meeting hour, at most.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub meeting_attendees: u8,
    /// § 2.4 (exec): the two best corp orders within this of each other are
    /// the exec's call (the picked one gains it).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub decision_margin: f32,
    /// § 2.4 (exec): the orders a brave exec leans to ...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expansive: Vec<crate::components::CorpOrder>,
    /// ... and those a loyal one leans to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub defensive: Vec<crate::components::CorpOrder>,
    /// § 2.4 (reporter): the scenes of yesterday's deeds a reporter walks a shift.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub beat_stops: u8,
    /// § 2.4 (reporter): every this many bylines the reporter's grade rises one step.
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub byline_promote: u16,
    /// § 2.4 (reporter): a witness is a body within this many tiles of the scene.
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub witness_tiles: u32,
    // --- 8d (the gate's last rows), each left out of a save at its default.
    /// 8d (OPEN-126, Shadow V5 bug 17): a homeless worker moving into a
    /// Home (`ownership::rehouse`, the hunger fix's first-rent rule) must
    /// hold this many days of its rent and pays them up front to its owner
    /// (`Flow::Rent`), held as prepaid rent; the jobless keep
    /// `rehouse_coins_mult`. 0: the old rule (one day's rent, nothing paid).
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub move_in_rent_days: u16,
    /// 8d (OPEN-129): a corp exec homed at `[life] exec_home_tier` (the
    /// Spire) whose Hq stands in another district is driven to work: the
    /// leg timed at the Car's speed, in a corp vehicle when the corp owns a
    /// road one, else the nearest Garage's off-screen cab, its fare paid by
    /// the corp (`streetwork::exec_ride`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub exec_ride: bool,
    /// Review fix 8 (doctrine 3): the phase's memory valences, griefs, the
    /// fare dodger's grudge, the break-out's thresholds and the pimp's round
    /// curves. Left out of a save at its defaults (the shipped values).
    #[serde(default, skip_serializing_if = "ServeTuneCfg::is_default")]
    pub tune: ServeTuneCfg,
}

/// `[service.tune]`: the numbers the service systems read beside their
/// rows (each a `Served` memory's valence unless named otherwise).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServeTuneCfg {
    /// § 2.1 (bouncer): a refusal at the door, the bouncer's side and the refused's.
    pub refused_bouncer: f32,
    pub refused: f32,
    /// § 2.1 (bouncer): an ejection, the bouncer's side and the attacker's.
    pub eject_bouncer: f32,
    pub ejected: f32,
    /// § 2.2 (super): the tenant in arrears served a notice.
    pub notice: f32,
    /// § 2.2 (jailer): each jailer on duty when the Jail is broken.
    pub jailbreak: f32,
    /// § 2.2 (warden): a Camp beating's `ChildTaken` grief to each parent, salience and valence.
    pub camp_beating_salience: f32,
    pub camp_beating_valence: f32,
    /// § 2.3 (cab): the driver's `Robbed` grudge on a rider who skipped his fare.
    pub fare_dodge_grudge: f32,
    /// § 3.5: a parent breaks his child out of a Camp alone when lawfulness
    /// is under `breakout_lawfulness` and courage at least `breakout_courage`.
    pub breakout_lawfulness: f32,
    pub breakout_courage: f32,
    /// § 2.3 (pimp): the round's GangWork curves `(m, b)` on energy and loyalty.
    pub pimp_energy: (f32, f32),
    pub pimp_loyalty: (f32, f32),
}

impl Default for ServeTuneCfg {
    fn default() -> Self {
        ServeTuneCfg {
            refused_bouncer: -0.1,
            refused: -0.3,
            eject_bouncer: -0.1,
            ejected: -0.4,
            notice: -0.3,
            jailbreak: -0.3,
            camp_beating_salience: 0.5,
            camp_beating_valence: -0.6,
            fare_dodge_grudge: 0.4,
            breakout_lawfulness: 0.3,
            breakout_courage: 0.6,
            pimp_energy: (0.6, 0.4),
            pimp_loyalty: (0.3, 0.7),
        }
    }
}

impl ServeTuneCfg {
    pub fn is_default(&self) -> bool {
        *self == ServeTuneCfg::default()
    }
}

fn is_zero_u32_cfg(v: &u32) -> bool {
    *v == 0
}

fn is_zero_f32_cfg(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero_i64_cfg(v: &i64) -> bool {
    *v == 0
}

fn is_zero_u16_cfg(v: &u16) -> bool {
    *v == 0
}

impl Default for ServiceCfg {
    fn default() -> Self {
        ServiceCfg {
            venues: false,
            civic: false,
            street: false,
            tower: false,
            madam_cut: 0.3,
            p_eject: 0.5,
            staff_meal: true,
            served_salience: 0.15,
            p_camp_beating: 0.0,
            p_camp_escape: 0.0,
            buyout_fee: 0,
            buyout_food_days: 0.0,
            breakout_crew: 0,
            neglect_days: 0,
            teacher_floor: 0.0,
            cab_wait_ticks: 0,
            client_tiles: 0,
            pimp_tiles: 0,
            pimp_pass_up: 0.0,
            pimp_absent_days: 0,
            p_short_report: 0.0,
            witness_lookout_mult: 0.0,
            cab_shifts: Vec::new(),
            evening_staff: Vec::new(),
            meeting_attendees: 0,
            decision_margin: 0.0,
            expansive: Vec::new(),
            defensive: Vec::new(),
            beat_stops: 0,
            byline_promote: 0,
            witness_tiles: 0,
            move_in_rent_days: 0,
            exec_ride: false,
            tune: ServeTuneCfg::default(),
        }
    }
}

impl ServiceCfg {
    /// Not written into a save at its defaults (every switch off).
    pub fn is_default(&self) -> bool {
        *self == ServiceCfg::default()
    }

    /// Lives at work phase 3: the civic switch off and its keys at their
    /// defaults (the off switches; nothing reads them off).
    pub fn strip_civic(&mut self) {
        let d = ServiceCfg::default();
        self.civic = false;
        self.p_camp_beating = d.p_camp_beating;
        self.p_camp_escape = d.p_camp_escape;
        self.buyout_fee = d.buyout_fee;
        self.buyout_food_days = d.buyout_food_days;
        self.breakout_crew = d.breakout_crew;
        self.neglect_days = d.neglect_days;
        self.teacher_floor = d.teacher_floor;
    }

    /// Lives at work phase 4: the street switch off and its keys at their
    /// defaults (the off switches; nothing reads them off).
    pub fn strip_street(&mut self) {
        let d = ServiceCfg::default();
        self.street = false;
        self.cab_wait_ticks = d.cab_wait_ticks;
        self.client_tiles = d.client_tiles;
        self.pimp_tiles = d.pimp_tiles;
        self.pimp_pass_up = d.pimp_pass_up;
        self.pimp_absent_days = d.pimp_absent_days;
        self.p_short_report = d.p_short_report;
        self.witness_lookout_mult = d.witness_lookout_mult;
        self.cab_shifts = d.cab_shifts;
    }

    /// Lives at work phase 5: the tower switch off and its keys at their
    /// defaults (the off switches; nothing reads them off).
    pub fn strip_tower(&mut self) {
        let d = ServiceCfg::default();
        self.tower = false;
        self.meeting_attendees = d.meeting_attendees;
        self.decision_margin = d.decision_margin;
        self.expansive = d.expansive;
        self.defensive = d.defensive;
        self.beat_stops = d.beat_stops;
        self.byline_promote = d.byline_promote;
        self.witness_tiles = d.witness_tiles;
    }
}

/// Lives at work phase 1: an encounter's outcome a `[[serves]]` row's
/// `event_on` may name (§ 1.4: events only for the notable).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServeOutcome {
    Done,
    Refused,
    Ejected,
    Regular,
    /// Lives at work phase 3 (§ 2.2, super): a tenant in arrears served notice.
    Notice,
}

/// Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1.1): one `[[serves]]`
/// row. `key` is a `[[trades]]` id or a legacy `Role` name (resolved like
/// the names `Config::unresolved_names` checks).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServesCfg {
    pub key: String,
    /// What the trade serves (its queue's source: § 2).
    pub kind: crate::components::ServeKind,
    /// Encounters one server takes an hour.
    pub per_hour: u8,
    /// The encounter's share of the hour, minutes (the diary's reading).
    pub minutes: u8,
    /// The `Skills` field a meeting trains, by this much.
    pub skill: TradeSkill,
    pub skill_gain: f32,
    /// A served becomes a regular (an Acquaintance edge, `regulars_made`)
    /// at this many encounters within `regular_days` (0: never).
    pub regular_after: u8,
    pub regular_days: u8,
    /// The `Served` memory's valence.
    pub valence: f32,
    /// The outcomes that print an `EventKind::Served`.
    pub event_on: Vec<ServeOutcome>,
    /// Lives at work phase 3: a civic row (teacher, camp warden, jailer,
    /// super), live only with `[service] civic` and stripped from the
    /// config off (`Config::strip_civic`), so the off save matches main.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub civic: bool,
    /// Lives at work phase 4: a street row (cab driver, courier, loader,
    /// street worker, pimp, dealer, vendor, rickshaw, tinker), live only with
    /// `[service] street` and stripped off (`Config::strip_street`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub street: bool,
    /// Lives at work phase 5: a tower row (the exec, the reporter, the
    /// Fixer's staff, the forger, the civic clerk), live only with `[service]
    /// tower` and stripped off (`Config::strip_tower`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tower: bool,
}

impl Default for ServesCfg {
    fn default() -> Self {
        ServesCfg {
            key: String::new(),
            kind: crate::components::ServeKind::Visit,
            per_hour: 6,
            minutes: 10,
            skill: TradeSkill::None,
            skill_gain: 0.0,
            regular_after: 0,
            regular_days: 14,
            valence: 0.1,
            event_on: Vec::new(),
            civic: false,
            street: false,
            tower: false,
        }
    }
}

/// Jobs & lives phase 7 `[ages]` (docs/JOBS_AND_LIVES.md § 6; D11-D13;
/// Dylan's answers 4 and 5): a child's daily routine (School on fees,
/// Chores at a parent's Pitch, Play, Street, Camp), the School, the
/// elderly's frailty and the kin who take them in, the seeded ages to
/// `seed_max_years`, and the maimed (a level and a slot; a prosthetic
/// clears it). Live only under `[jobs2] enabled`. Placeholders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgesCfg {
    pub enabled: bool,
    /// The seeded population's oldest age (sim years; `[world]
    /// age_max_years` with the switch off).
    pub seed_max_years: f32,
    /// Schools seeded (Mid and up, on the city's deed) per copy; a School
    /// within `school_reach` tiles of the child's Home, its fee a school day
    /// paid by the household (`Flow::Contract`; the Treasury's with the
    /// `public_school` lever); `edu_gain` of education a school day at 18.
    pub schools: u32,
    pub school_reach: u32,
    pub school_fee: i64,
    pub edu_gain: f32,
    /// A Street child begs `child_beg_coins` a day from a resident of its
    /// district (a gift), steals on `child_theft_p` (a resident's coins),
    /// and is taken to a camp after `street_take_days` on the street.
    pub child_beg_coins: i64,
    pub child_theft_p: f32,
    pub street_take_days: u16,
    /// The elderly: from `frail_from_years` work output × (1 − `frail_slope`
    /// × the years over); the sick-day firing at `elder_fire_mult` of
    /// `[sickness] sick_fire_days`; a jobless elder with an adult child
    /// housed elsewhere moves in on `kin_take_in_p` a day.
    pub frail_from_years: u32,
    pub frail_slope: f32,
    pub elder_fire_mult: f32,
    pub kin_take_in_p: f32,
    /// At seed a seeded adult of `kin_from_years` or more has a grown child
    /// (a Parent edge to an adult 20-35 years younger) on `seed_kin_p`.
    pub kin_from_years: u32,
    pub seed_kin_p: f32,
    /// Disability: a stabilised survivor maimed on `p_maimed`; a packer's
    /// shift-hour on `p_press_maim`; a level's work output × (1 −
    /// `maimed_work`); a beggar's odds × `maimed_beg_mult`.
    pub p_maimed: f32,
    pub p_press_maim: f32,
    pub maimed_work: f32,
    pub maimed_beg_mult: f32,
}

impl Default for AgesCfg {
    fn default() -> Self {
        AgesCfg {
            enabled: false,
            seed_max_years: 75.0,
            schools: 2,
            school_reach: 60,
            school_fee: 2,
            edu_gain: 0.003,
            child_beg_coins: 1,
            child_theft_p: 0.02,
            street_take_days: 14,
            frail_from_years: 60,
            frail_slope: 0.03,
            elder_fire_mult: 0.6,
            kin_take_in_p: 0.3,
            kin_from_years: 45,
            seed_kin_p: 0.6,
            p_maimed: 0.25,
            p_press_maim: 0.0005,
            maimed_work: 0.2,
            maimed_beg_mult: 1.5,
        }
    }
}

/// Jobs & lives phase 6 `[gray]` (docs/JOBS_AND_LIVES.md § 7): papers,
/// forgers and a corruptible Clerk, black clinics and struck-off docs,
/// organs as a good (rotting, sold to Clinics and the World's capped book),
/// customs on the organs' export, and the gang types as data. A board
/// game's counters: tiers, units, rolls and coins. Live only under
/// `[jobs2] enabled`. Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrayCfg {
    pub enabled: bool,
    /// Fake papers by tier (1, 2) at a forger; a Clerk under `clerk_lawful`
    /// sells real ones for `id_bribe` (`Flow::Bribe`).
    pub id_price: [i64; 2],
    pub id_bribe: i64,
    pub clerk_lawful: f32,
    /// A wanted holder passes a guard's sighting for the hour on
    /// `id_w × tier` (papers bought while wanted).
    pub id_w: f32,
    /// At an arrest fake papers are exposed on this chance (`Forgery`).
    pub p_expose: f32,
    /// The Fixer's owner's share of a forger's sale.
    pub fixer_cut: f32,
    /// Organs a body yields to the gang that rips it, the share lost a day,
    /// a Clinic's shelf (bought from a gang at `organ_price`), a
    /// stabilisation's draw on it, and customs on the World's book.
    pub organs_per_body: u32,
    pub organ_rot: f32,
    pub clinic_organs: u32,
    pub organ_price: i64,
    pub p_organ_needed: f32,
    /// Jobs & lives phase 8 (the scavs): an abductor dragging its take home
    /// scores its GangWork this (and the plan's hysteresis), as a hunter its
    /// Hunt (`[hunt] hold_score`), so only an emergency drops the drag.
    pub drag_hold_score: f32,
    pub p_customs: f32,
    /// A gang builds a BlackClinic with its found cost and this to spare.
    pub black_clinic_spare: i64,
    /// A BlackClinic in a district the law sweeps or cracks down on its
    /// gang is found on this chance a day (`BlackMarket`, shut this long).
    pub p_bm_raid: f32,
    pub bm_close_days: u64,
    /// A licensed Ripperdoc is struck off on this × (1 − lawfulness) a day.
    pub p_struck_off: f32,
    /// The gang types (D-scavs): rows by share of the gangs (a pure hash).
    pub gang_types: Vec<GangTypeCfg>,
}

/// A gang type row (Dylan's answer 1): its share of the gangs, the flat it
/// adds to the Harvest order, its rip's kill chance and whether its Harvest
/// target counts organs (anyone in its ground).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GangTypeCfg {
    pub id: String,
    pub share: f32,
    pub harvest_flat: f32,
    pub kill_p: f32,
    pub organ_targets: bool,
}

impl Default for GangTypeCfg {
    fn default() -> Self {
        GangTypeCfg { id: String::new(), share: 0.0, harvest_flat: 0.0, kill_p: 0.5, organ_targets: false }
    }
}

impl Default for GrayCfg {
    fn default() -> Self {
        GrayCfg {
            enabled: false,
            id_price: [40, 120],
            id_bribe: 60,
            clerk_lawful: 0.35,
            id_w: 0.15,
            p_expose: 0.3,
            fixer_cut: 0.2,
            organs_per_body: 2,
            organ_rot: 0.25,
            clinic_organs: 3,
            organ_price: 30,
            p_organ_needed: 0.5,
            drag_hold_score: 0.75,
            p_customs: 0.1,
            black_clinic_spare: 200,
            p_bm_raid: 0.2,
            bm_close_days: 3,
            p_struck_off: 0.002,
            gang_types: Vec::new(),
        }
    }
}

/// Jobs & lives phase 6 `[faith]` (docs/JOBS_AND_LIVES.md § 8; Dylan's
/// answer 6): creeds as data (`creeds` rows; no creed name in code), the
/// churches (a Mission with a creed), the cults (a gang with a creed), the
/// daily Preach, conversion, the followers' spread along social edges,
/// tithes, and apostasy hunted through the grudges. Live only under
/// `[jobs2] enabled`. Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FaithCfg {
    pub enabled: bool,
    pub creeds: Vec<CreedCfg>,
    /// The daily Preach: a district resident attends on `p_attend` (a
    /// follower, or one under `attend_belonging` or grieving within
    /// `grief_days`), gaining `attend_gain` belonging; a non-follower
    /// attendee under `convert_belonging` or grieving converts on the row's
    /// `p_convert`.
    pub p_attend: f32,
    pub attend_belonging: f32,
    pub attend_gain: f32,
    pub convert_belonging: f32,
    pub grief_days: u64,
    /// A follower's Friend, Family or Spouse converts on the row's
    /// `p_spread` a day (the creed spreads through people).
    pub spread_affinity: f32,
}

impl Default for FaithCfg {
    fn default() -> Self {
        FaithCfg {
            enabled: false,
            creeds: Vec::new(),
            p_attend: 0.3,
            attend_belonging: 0.5,
            attend_gain: 0.2,
            convert_belonging: 0.4,
            grief_days: 7,
            spread_affinity: 0.3,
        }
    }
}

/// One `[[faith.creeds]]` row (D14): a church (a Mission with a creed) or a
/// cult (a gang with a creed), seeded per copy in its zone's district at
/// `share` of the way along it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreedCfg {
    pub id: String,
    pub label: String,
    pub church: bool,
    pub cult: bool,
    pub zone: crate::components::Zone,
    pub share: f32,
    /// Chrome tolerated (`Kit.visible`), as the Purists'.
    pub tolerance: u8,
    /// Of each wage, into the church's purse or the cult's treasury (`Flow::Tithe`).
    pub tithe_frac: f32,
    /// A cult recruits only along a social edge to a member (past `founders`).
    pub recruit_edges: bool,
    pub founders: usize,
    /// A leaver's grudge from the leader (0: none); `hunt` reads it at `hunt_min`.
    pub apostasy_w: f32,
    /// The flat a cult adds to its Contest and Raid orders.
    pub violence_bias: f32,
    /// A cult member's daily walk-away chance × this (`gang::desist`).
    pub desist_mult: f32,
    pub p_convert: f32,
    pub p_spread: f32,
}

impl Default for CreedCfg {
    fn default() -> Self {
        CreedCfg {
            id: String::new(),
            label: String::new(),
            church: false,
            cult: false,
            zone: crate::components::Zone::Sump,
            share: 0.5,
            tolerance: 0,
            tithe_frac: 0.1,
            recruit_edges: true,
            founders: 3,
            apostasy_w: 0.0,
            violence_bias: 0.0,
            desist_mult: 1.0,
            p_convert: 0.1,
            p_spread: 0.01,
        }
    }
}

/// Jobs & lives `[jobs2]` (docs/JOBS_AND_LIVES.md §§ 1-2): the master
/// switch in flight and phase 1's keys (the industries, the civic three,
/// the Hq). Every number is a placeholder. Off (the default when the table
/// is absent), no new kind seeds, no new trade row posts, no new flow books.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Jobs2Cfg {
    pub enabled: bool,
    /// § 2: Markets cut their synth shelf from Rations stock (off: it moves
    /// the diet rates and parity; no code reads it before the gate).
    pub rations_to_markets: bool,
    /// § 1: the appended goods' stock cap at a building (at least the
    /// kind's `stock_cap`).
    pub goods_cap: u32,
    // --- Packing (§ 2).
    /// Food units a Packer packs an on-duty hour (× 0.5 + her farming skill).
    pub pack_rate: f32,
    /// Rations made per Food unit packed.
    pub pack_ratio: f32,
    /// The World's Rations bid is this × its Food bid (`world_market::bid_ref`).
    pub packed_premium: f32,
    /// Rations a plant keeps back at the midnight sale.
    pub rations_floor: u32,
    /// A Food corp's Grow builds a Packing Plant while the World's Food
    /// fill is above this ...
    pub pack_fill: f32,
    /// ... and it owns fewer plants than its Farms ÷ this.
    pub farms_per_plant: u32,
    // --- Freight (§ 2).
    /// Coins a unit sold to the World pays the nearest Freight Yard.
    pub handling_per_unit: f32,
    /// Of an import's coins, to the nearest Freight Yard (an import names
    /// no units; JOBS_V2 J23's rate on value).
    pub handling_rate: f32,
    /// Coins a unit of a cross-owner Farm -> Market haul pays the Yard (the Drivers).
    pub haul_per_unit: f32,
    /// A Yard with no Loader on the last night shift passes the fee to the
    /// World (the leak JOBS_V2 named) instead of taking it.
    pub loader_gate: bool,
    /// Per Loader shift: a Ration walks out with the Loader (`Pilfered`).
    pub p_pilfer: f32,
    // --- Power (§ 2).
    /// Of the power a Utility Plant takes, to the World (its fuel).
    pub fuel_share: f32,
    /// Hours with no Plant Operator on shift before its districts go dark.
    pub outage_hours: u8,
    // --- Construction (§ 2).
    /// Days a build stands closed with the Yard's full crew.
    pub crew_days: f32,
    /// The slowest build: `crew_days` × this (a Yard with no Builders).
    pub crew_slow_max: f32,
    /// A Builder's round visits the Yard's sites within this many tiles
    /// (Manhattan, door to door); a farther site is built from the Yard.
    pub site_reach: u32,
    // --- The civic three (§ 2), after police and Sanitation.
    /// One Jailer per this many held.
    pub held_per_jailer: u32,
    pub clerks_max: u16,
    pub depot_hands_max: u16,
    /// A Jailer short (fewer than the held need) multiplies a breach's odds.
    pub jailer_short_mult: f32,
    // --- Seeding (row indices into `[corps] names`, never names).
    /// One Packing Plant per copy of each row.
    pub packing_rows: Vec<usize>,
    pub freight_rows: Vec<usize>,
    pub utility_rows: Vec<usize>,
    pub construction_rows: Vec<usize>,
    /// An Hq per copy of each megacorp row (`[corps] megacorp`) at seed.
    pub hq_megacorps: bool,
    // --- The Hq (§ 2).
    /// A local corp owning this many standing buildings builds an Hq.
    pub hq_min_buildings: usize,
    /// ... keeping this much of its treasury after the build.
    pub hq_reserve: i64,
    /// JOBS_V2 J30: an Hq's office costs at most this share of its corp's
    /// 7-day mean revenue a day (the overhead rows' places, `industry::hq_places`).
    pub admin_share: f32,
    // --- Phase 2, the street (§ 3): `street` is its sub-switch (read only
    // under `enabled`; `streetwork::on`).
    pub street: bool,
    /// Open Pitches a district holds at most, per 2,000 residents (scaled).
    pub pitches_per_district: u16,
    /// Coins a jobless adult holds to open a Pitch ...
    pub pitch_stake: i64,
    /// Shadow V4 bug 6: a knowledge row's hire has at least this knowledge.
    pub hire_knowledge_floor: f32,
    /// ... after this many days without a post ...
    pub pitch_after_days: u32,
    /// ... on this chance a day (a pure hash of the day and the agent).
    pub p_pitch_open: f32,
    /// A Pitch with no take for this many days closes.
    pub pitch_fail_days: u8,
    /// A body stands its Pitch within this many tiles of the spot.
    pub pitch_reach: u32,
    /// The share of a Pitch's day's take paid to the gang holding its
    /// district (`Flow::StreetCut`).
    pub cut_frac: f32,
    /// Knowledge at which a pitcher tinkers, fighting at which he pulls a rickshaw.
    pub tinker_skill: f32,
    pub rickshaw_skill: f32,
    /// Vendors: the meals bought at the nearest Market at the day's open
    /// (shelf units of synth, `synth_stretch` meals each) ...
    pub vendor_batch: u32,
    /// ... sold at the synth meal's price × this ...
    pub vendor_markup: f32,
    /// ... to an off-screen synth buyer in the district on this chance.
    pub p_vendor_pick: f32,
    /// A Pitch in a `Sweep` district is swept on this chance a day: a fine
    /// to the Treasury and closed for `sweep_close_days`; at most
    /// `sweeps_per_day` a day per 2,000 residents (scaled).
    pub p_pitch_sweep: f32,
    pub pitch_fine: i64,
    pub sweep_close_days: u8,
    pub sweeps_per_day: u16,
    /// The labour corner: the dawn pick's tick of day ...
    pub corner_hour: u16,
    /// ... from the jobless with fewer coins than this, at least this lawful ...
    pub corner_broke: i64,
    pub corner_lawful: f32,
    /// ... standing within this many tiles (a body) or living within
    /// `corner_stat_reach` (off screen, on `p_corner` a day) ...
    pub corner_tiles: u32,
    pub corner_stat_reach: u32,
    pub p_corner: f32,
    /// ... a body walks there from this many tiles at most.
    pub corner_reach: u32,
    /// Day posts: per open site a Construction Yard posts, per Farm in
    /// the harvest seasons, per Freight Yard that handled `backlog_units`
    /// yesterday.
    pub day_per_site: u8,
    pub harvest_posts: u8,
    pub harvest_seasons: Vec<crate::time::Season>,
    pub yard_posts: u8,
    pub backlog_units: u32,
    /// Couriers: parcels a day per standing building of a corp with two or
    /// more, a courier's day's parcels, and each parcel's fee.
    pub parcels_per_building: f32,
    pub parcels_per_courier: u32,
    pub parcel_fee: i64,
    /// Cabs and rickshaws (off screen, the shift's first hour): a commute of
    /// at least `cab_min_tiles` with `fare + cab_reserve` coins takes one on
    /// `p_cab`; the fare `fare_base + fare_per_tile × tiles` (a rickshaw's
    /// × `rickshaw_mult`); a driver's and a rickshaw's rides a day.
    pub cab_min_tiles: u32,
    pub cab_reserve: i64,
    pub p_cab: f32,
    pub fare_base: i64,
    pub fare_per_tile: f32,
    pub rickshaw_mult: f32,
    pub rides_per_driver: u32,
    pub rides_per_rickshaw: u32,
    /// Tinkers: an agent's vehicle due a repair within `tinker_reach` of an
    /// open tinker is fixed at `tinker_discount` × the Garage's price, on
    /// `tinker_fail_p` wrecked instead; `tinker_jobs` a day each.
    pub tinker_reach: u32,
    pub tinker_discount: f32,
    pub tinker_fail_p: f32,
    pub tinker_jobs: u8,
    /// A corp's Utility Plant dark on this many days running is taken by
    /// the city (the Nationalise path; 0 never).
    pub seize_after_days: u8,
}

impl Default for Jobs2Cfg {
    fn default() -> Self {
        Jobs2Cfg {
            enabled: false,
            rations_to_markets: false,
            goods_cap: 300,
            pack_rate: 4.0,
            pack_ratio: 1.0,
            packed_premium: 1.4,
            rations_floor: 0,
            pack_fill: 0.9,
            farms_per_plant: 6,
            handling_per_unit: 0.25,
            handling_rate: 0.08,
            haul_per_unit: 0.25,
            loader_gate: true,
            p_pilfer: 0.02,
            fuel_share: 0.4,
            outage_hours: 6,
            crew_days: 3.0,
            crew_slow_max: 4.0,
            site_reach: 48,
            held_per_jailer: 25,
            clerks_max: 8,
            depot_hands_max: 6,
            jailer_short_mult: 1.5,
            packing_rows: Vec::new(),
            freight_rows: Vec::new(),
            utility_rows: Vec::new(),
            construction_rows: Vec::new(),
            hq_megacorps: false,
            hq_min_buildings: 6,
            hq_reserve: 2000,
            admin_share: 0.05,
            street: false,
            pitches_per_district: 4,
            pitch_stake: 20,
            hire_knowledge_floor: 0.05,
            pitch_after_days: 5,
            p_pitch_open: 0.05,
            pitch_fail_days: 4,
            pitch_reach: 3,
            cut_frac: 0.1,
            tinker_skill: 0.5,
            rickshaw_skill: 0.4,
            vendor_batch: 5,
            vendor_markup: 1.25,
            p_vendor_pick: 0.5,
            p_pitch_sweep: 0.1,
            pitch_fine: 5,
            sweep_close_days: 1,
            sweeps_per_day: 4,
            corner_hour: 360,
            corner_broke: 20,
            corner_lawful: 0.2,
            corner_tiles: 6,
            corner_stat_reach: 160,
            p_corner: 0.3,
            corner_reach: 40,
            day_per_site: 1,
            harvest_posts: 1,
            harvest_seasons: vec![crate::time::Season::Summer, crate::time::Season::Autumn],
            yard_posts: 2,
            backlog_units: 300,
            parcels_per_building: 0.5,
            parcels_per_courier: 12,
            parcel_fee: 1,
            cab_min_tiles: 60,
            cab_reserve: 30,
            p_cab: 0.03,
            fare_base: 3,
            fare_per_tile: 0.05,
            rickshaw_mult: 0.6,
            rides_per_driver: 8,
            rides_per_rickshaw: 6,
            tinker_reach: 40,
            tinker_discount: 0.6,
            tinker_fail_p: 0.1,
            tinker_jobs: 3,
            seize_after_days: 5,
        }
    }
}

/// Jobs & lives phase 3 `[weapons]` (docs/JOBS_AND_LIVES.md § 4; Dylan's
/// answer 1): weapons as the first kind of the owned-item store, the
/// fight's tier term, Gunsmiths, armouries, dealers, crates, licences and
/// carry sightings, and the petty theft of carried and kept items. Live
/// only under `[jobs2] enabled`. Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeaponsCfg {
    pub enabled: bool,
    // --- The fight.
    /// `p_win += weapon_w × (tier_a − tier_b)` when either side is armed.
    pub weapon_w: f32,
    /// The loser's death chance × `(1 + kill_w × the winner's tier)`.
    pub kill_w: f32,
    /// A raid's pairing strength and a Hunt's might, per tier.
    pub might_w: f32,
    /// Condition a fight takes off the winner's weapon.
    pub wear_per_fight: u8,
    /// An armed suspect contesting the cuffs: the death chance × this.
    pub armed_arrest_mult: f32,
    // --- The store.
    /// Weapon entities alive at most, per 1,000 residents (R4).
    pub weapons_per_1000: f32,
    // --- Gunsmiths.
    /// `[corps]` rows (Security) given a Gunsmith per copy at seed.
    pub gunsmith_rows: Vec<usize>,
    pub parts_per_weapon: u32,
    /// Weapons a gunsmith makes an on-duty hour, × (0.5 + knowledge).
    pub make_rate: f32,
    /// Weapons a Gunsmith keeps in stock at most.
    pub gunsmith_stock: u32,
    // --- The law and the guards.
    /// The tier the Precinct and the Security Offices buy.
    pub guard_tier: u8,
    /// The Precinct's target stock per city guard, and a Security
    /// Office's per private guard.
    pub stock_per_guard: f32,
    /// The Treasury's arms line: coins a day at most.
    pub arms_budget: i64,
    /// A licence from the Civic Clerk (`Flow::Licence`).
    pub licence_fee: i64,
    /// The concealment contest's step per tier.
    pub conceal_step: f32,
    /// `UnlicensedArms` reports a day at most, per 1,000 residents (R3).
    pub ua_per_1000_day: f32,
    /// Carry sightings are rolled this many times in a district cracking
    /// down on the carrier's gang.
    pub crackdown_rolls: u8,
    // --- Owners.
    /// The stable share of each class (`Corp`, `Street`, `Dreg`) who want
    /// a weapon of their own when they can afford one.
    pub want_p_by_class: [f32; 3],
    /// The chance a day an owner with no other reason carries it (else it
    /// stays at home), by class.
    pub carry_p_by_class: [f32; 3],
    /// Lawfulness under this is a reason to want (and carry) one.
    pub want_lawless: f32,
    /// A theft or an assault suffered within this many days is a reason.
    pub robbed_days: u32,
    /// Coins a buyer keeps after the purchase.
    pub coin_reserve: i64,
    // --- Gangs and dealers.
    /// A gang's armoury target per member (the dealer sells the surplus).
    pub armoury_per_member: f32,
    /// A dealer's price × the tier's list.
    pub dealer_markup: f32,
    /// The dealer's cut of each sale (`Flow::Arms`).
    pub dealer_cut: i64,
    // --- Smuggling.
    /// Weapons a crate holds at most, their tier, and the price of each
    /// abroad; a gang ships one while its armoury is under its target plus
    /// `deal_float` (its dealer's stock).
    pub crate_size: u32,
    pub crate_tier: u8,
    pub crate_unit: i64,
    pub deal_float: u32,
    /// Days between a gang's crates.
    pub crate_days: u32,
    /// A crate is paid only from the coins above this many days of the
    /// gang's stipends (the roster and one recruit).
    pub crate_reserve_days: u32,
    /// The minute of the day crates land.
    pub crate_hour: u16,
    pub p_seize: f32,
    /// Coins to a Loader on shift at the Yard, and the seizure chance ×
    /// `bribe_mult` when one takes it.
    pub bribe: i64,
    pub bribe_mult: f32,
    // --- Petty theft (Dylan's answer 1).
    /// A Market thief also lifts a carried item from a body within
    /// `petty_reach` on this chance (a concealment contest after).
    pub p_pickpocket: f32,
    pub petty_reach: u32,
    /// ... or empties the till of a stall in the Market's district of up to
    /// this many coins.
    pub stall_take_max: i64,
    /// Petty item thefts a day at most, per 1,000 residents (R3).
    pub petty_per_1000_day: f32,
    /// What a fence pays of an item's value.
    pub fence_frac: f32,
}

impl Default for WeaponsCfg {
    fn default() -> Self {
        WeaponsCfg {
            enabled: false,
            weapon_w: 0.08,
            kill_w: 0.5,
            might_w: 0.15,
            wear_per_fight: 2,
            armed_arrest_mult: 1.5,
            weapons_per_1000: 100.0,
            gunsmith_rows: vec![7],
            parts_per_weapon: 3,
            make_rate: 0.15,
            gunsmith_stock: 12,
            guard_tier: 1,
            stock_per_guard: 0.5,
            arms_budget: 100,
            licence_fee: 60,
            conceal_step: 0.25,
            ua_per_1000_day: 1.0,
            crackdown_rolls: 2,
            want_p_by_class: [0.08, 0.05, 0.03],
            carry_p_by_class: [0.3, 0.6, 0.8],
            want_lawless: 0.25,
            robbed_days: 7,
            coin_reserve: 20,
            armoury_per_member: 0.3,
            dealer_markup: 1.5,
            dealer_cut: 10,
            crate_size: 4,
            crate_tier: 1,
            crate_unit: 40,
            deal_float: 6,
            crate_days: 3,
            crate_reserve_days: 7,
            crate_hour: 120,
            p_seize: 0.15,
            bribe: 15,
            bribe_mult: 0.5,
            p_pickpocket: 0.3,
            petty_reach: 3,
            stall_take_max: 10,
            petty_per_1000_day: 2.0,
            fence_frac: 0.4,
        }
    }
}

/// Jobs & lives phase 4 `[vice]` (docs/JOBS_AND_LIVES.md § 5.1): an
/// economic and legal simulation of venues and labour: the StripClub and
/// the Brothel (venues on the leisure ladder: visits, prices, tips), the
/// Lounge's companions, the `street_worker` Pitch (its night visits, the
/// pimp's cut, the street's assault roll), the sweeps (`Solicitation`) and
/// the policy levers seeded into `World::levers`. Live only under `[jobs2]
/// enabled`. Every number is a placeholder (R8: the income small and taxed
/// by cuts and fines).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViceCfg {
    pub enabled: bool,
    /// `[corps]` rows given the Vice niche at seed (and the seeded venues).
    pub corp_rows: Vec<usize>,
    /// Venues seeded per copy of each `corp_rows` row.
    pub strip_clubs: u32,
    pub brothels: u32,
    /// A visit's price by district tier (Sump, Mid, Spire).
    pub strip_price: [i64; 3],
    pub brothel_price: [i64; 3],
    /// The fun a visit gives, and the intimacy a Brothel's or a
    /// companion's visit gives.
    pub gain_strip: f32,
    pub gain_brothel: f32,
    pub intimacy_gain: f32,
    /// The ladder weighs a Brothel `1 + intimacy_pull × (1 − intimacy)`.
    pub intimacy_pull: f32,
    /// The visitor's tip: this share of the price, customer -> the worker
    /// on duty (`Flow::Tip`, untaxed).
    pub tip_share: f32,
    /// A visit's corp demand reference (`corp_brain::demand_of`'s Vice
    /// arm): visits a day per venue.
    pub vice_demand_ref: f32,
    // --- The street (on the phase-2 Pitch).
    /// A jobless opener under this lawfulness works a street pitch on
    /// `p_street` (else the phase-2 trades).
    pub street_lawful: f32,
    pub p_street: f32,
    /// A working street pitch's visit: on this chance an hour, at this
    /// price, from a resident of the district whose intimacy is under
    /// `street_intimacy`.
    pub p_street_visit: f32,
    pub street_price: i64,
    pub street_intimacy: f32,
    /// A pimp's gang takes this share of a street pitch's day (instead of
    /// `[jobs2] cut_frac`).
    pub pimp_cut: f32,
    /// A working street pitch-hour's assault roll (× the district's fear
    /// + 0.5), with no pimp assigned and the worker unarmed.
    pub p_assault_street: f32,
    /// A street pitch in a `Sweep` district is swept on this chance a day
    /// (`Solicitation`, the phase-2 fine and closing) while `street_legal`
    /// is off; reports a day at most per 1,000 residents (R3).
    pub p_street_sweep: f32,
    pub solicit_per_1000_day: f32,
    // --- The policy levers' seeds (`World::levers.vice`).
    pub brothels_licensed: bool,
    pub street_legal: bool,
    /// Phase 5's slots, seeded now.
    pub dollhouse_contracts: bool,
    pub captive_chip_trafficking: bool,
    // --- Jobs & lives phase 5 (§ 5.2, § 5.3). Sub-switches under
    // `enabled` (`chip`, `bd`, `patrol_sweep`), each byte-identical off.
    /// The chip contract and the Dollhouse (§ 5.2).
    pub chip: bool,
    /// The BD studios, the Braindance units and the Recorder (§ 5.3).
    pub bd: bool,
    /// Phase 4's carry-over: a street pitch outside a `Sweep` district is
    /// swept on `p_patrol_sweep` × the law's alertness (under the same caps).
    pub patrol_sweep: bool,
    pub p_patrol_sweep: f32,
    // --- The chip contract (§ 5.2). Numbers only.
    /// Dollhouses seeded per copy of each `corp_rows` row.
    pub dollhouses: u32,
    /// A visit's price by district tier (Sump, Mid, Spire), and its fun.
    pub dollhouse_price: [i64; 3],
    pub gain_dollhouse: f32,
    /// A contract runs this many days.
    pub chip_days: u32,
    /// Its wage is the row's × this (`Job.premium`).
    pub chip_mult: f32,
    /// The chip's `Kit.load` (above any implant tier's `sanity_cost`).
    pub chip_load: f32,
    /// The install and the removal, each paid by the holder to the Clinic.
    pub chip_fee: i64,
    /// The `Chipped` trait's one-time sanity cut at the removal.
    pub chipped_sanity: f32,
    /// A gang Dollhouse front's captives at most (a gang order each).
    pub captives_per_front: u32,
    // --- Braindance (§ 5.3).
    /// BdStudios seeded per copy of each `corp_rows` row.
    pub bd_studios: u32,
    /// Units a studio's produce-model staff make an hour, × (0.5 + skill).
    pub bd_rate: f32,
    /// A studio keeps this many units; the rest go to the Arcades and the World.
    pub bd_floor: u32,
    /// An Arcade's shelf of units, bought from a studio at `bd_wholesale`.
    pub arcade_bd: u32,
    pub bd_wholesale: i64,
    /// A black unit's price: `bd_wholesale × black_bd_price_mult`; a Bar
    /// drinker with a dealer present buys one on `p_black_buy`.
    pub black_bd_price_mult: f32,
    pub p_black_buy: f32,
    /// A gang with this many members keeps one Recorder, bought abroad;
    /// alive at most per 1,000 residents.
    pub recorder_members: usize,
    pub recorders_per_1000: f32,
}

impl Default for ViceCfg {
    fn default() -> Self {
        ViceCfg {
            enabled: false,
            corp_rows: vec![5],
            strip_clubs: 1,
            brothels: 1,
            strip_price: [4, 6, 10],
            brothel_price: [5, 8, 14],
            gain_strip: 0.55,
            gain_brothel: 0.45,
            intimacy_gain: 0.4,
            intimacy_pull: 1.0,
            tip_share: 0.15,
            vice_demand_ref: 4.0,
            street_lawful: 0.35,
            p_street: 0.5,
            p_street_visit: 0.3,
            street_price: 5,
            street_intimacy: 0.7,
            pimp_cut: 0.3,
            p_assault_street: 0.01,
            p_street_sweep: 0.2,
            solicit_per_1000_day: 1.0,
            brothels_licensed: true,
            street_legal: false,
            dollhouse_contracts: true,
            captive_chip_trafficking: true,
            chip: false,
            bd: false,
            patrol_sweep: false,
            p_patrol_sweep: 0.1,
            dollhouses: 1,
            dollhouse_price: [10, 16, 30],
            gain_dollhouse: 0.6,
            chip_days: 30,
            chip_mult: 2.0,
            chip_load: 0.4,
            chip_fee: 80,
            chipped_sanity: 0.1,
            captives_per_front: 1,
            bd_studios: 1,
            bd_rate: 0.5,
            bd_floor: 5,
            arcade_bd: 10,
            bd_wholesale: 6,
            black_bd_price_mult: 2.0,
            p_black_buy: 1.0,
            recorder_members: 6,
            recorders_per_1000: 2.0,
        }
    }
}

/// Jobs & lives phase 4 `[fashion]` (docs/JOBS_AND_LIVES.md § 9, Dylan's
/// answer 1): clothing as an item kind of the owned-item store, the
/// Boutique (a venue on the leisure ladder: grooming and clothes), the
/// `status` need, and the dress readers. Live only under `[jobs2] enabled`.
/// Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FashionCfg {
    pub enabled: bool,
    /// `[corps]` rows that own the seeded Boutiques, one per copy each.
    pub boutique_rows: Vec<usize>,
    /// A Boutique visit's grooming price by district tier (Sump, Mid, Spire).
    pub groom_price: [i64; 3],
    /// The fun and the `status` a Boutique visit gives; the `status` a
    /// High-rung evening gives.
    pub gain_boutique: f32,
    /// The ladder weighs a Boutique `1 + status_pull × (1 − status)`.
    pub status_pull: f32,
    pub status_gain: f32,
    pub status_high: f32,
    /// `status` decays `status_decay × (0.5 + pride) × class_w[class]` a
    /// day (Corp, Street, Dreg).
    pub status_decay: f32,
    pub class_w: [f32; 3],
    /// Mood's `status_w × (status − 0.5)`.
    pub status_w: f32,
    /// Clothing items alive at most, per 1,000 residents (R4).
    pub clothing_per_1000: f32,
    /// A dress-0 carrier in the Spire is noticed at a carry sighting × this.
    pub spire_dress_mult: f32,
    /// A fight's winner (no guard) takes the loser's worn clothing on this
    /// chance (under the petty cap, `[weapons] petty_per_1000_day`).
    pub p_mug_clothes: f32,
    /// A Boutique sale leaves this many meals' coins beyond the evening's
    /// reserve (clothes after food).
    pub clothes_reserve_meals: i64,
}

impl Default for FashionCfg {
    fn default() -> Self {
        FashionCfg {
            enabled: false,
            boutique_rows: vec![5],
            groom_price: [3, 6, 12],
            gain_boutique: 0.2,
            status_pull: 4.0,
            status_gain: 0.35,
            status_high: 0.15,
            status_decay: 0.01,
            class_w: [1.5, 1.0, 0.5],
            status_w: 0.1,
            clothing_per_1000: 200.0,
            spire_dress_mult: 1.5,
            p_mug_clothes: 0.3,
            clothes_reserve_meals: 10,
        }
    }
}

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

/// M17 phase 4b `[outside.succession]` (Dylan, 2026-10-13: "Gangs probably
/// should promote from within or even have a civil war to settle who the new
/// boss is"). A decapitated gang promotes the top of `leader_ranking`; when
/// the top two are within `war_margin` it fights a civil war for
/// `war_days`. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SuccessionCfg {
    pub enabled: bool,
    /// The top two of `leader_ranking` closer than this contest the chair.
    pub war_margin: f32,
    /// A civil war needs this many free members (the claimants included).
    pub war_min_members: usize,
    /// The war is settled by the sides' strength after this many days.
    pub war_days: u32,
    /// Each claimant's grudge on the other (a Hunt; lethal at `[hunt] lethal_min`).
    pub war_grudge: f32,
    /// The strongest `war_seconds` of each side hold `war_grudge_second` on the rival claimant.
    pub war_seconds: usize,
    pub war_grudge_second: f32,
    /// Enemy edges across the sides at most (as D36's split).
    pub war_enemy_pairs: usize,
    /// A side's pull: `loyalty_w × loyalty` toward the heir (the ranking's top), the edge
    /// affinity, and `regard_w × (standing + dread × (1 − courage))` of each claimant.
    pub loyalty_w: f32,
    pub regard_w: f32,
    /// A loser with this many followers left leads a splinter (D36's `split`), else flees.
    pub splinter_min: usize,
}

impl Default for SuccessionCfg {
    fn default() -> Self {
        SuccessionCfg {
            enabled: true,
            war_margin: 0.02,
            war_min_members: 8,
            war_days: 14,
            war_grudge: 0.9,
            war_seconds: 2,
            war_grudge_second: 0.8,
            war_enemy_pairs: 30,
            loyalty_w: 0.3,
            regard_w: 0.3,
            splinter_min: 2,
        }
    }
}

/// M17 phase 2 (`[outside.parents]`, the body's `[outside]` and
/// `[outside.orders]` keys that phase 2 reads). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParentsCfg {
    /// The accounts at seed (the World is the Real economy's, not a row).
    pub factions: Vec<ParentRow>,
    pub market_sd: f32,
    pub market_revert: f32,
    /// A corp branch remits when its closing is at least this × `treasury_ref`.
    pub remit_floor: f32,
    /// The share of a branch's positive daily cashflow sent home.
    pub remit_share: f32,
    /// The `SetTariff` lever's value at seed.
    pub tariff: f32,
    /// A syndicate's gang pays this share of its treasury above `tribute_floor`, weekly.
    pub tribute_share: f32,
    pub tribute_floor: i64,
    pub rep_rate: f32,
    pub att_decay: f32,
    pub att_remit: f32,
    pub att_cost: f32,
    pub att_loss: f32,
    /// The city's Embargo cuts the parent's income by this (a sanction's cut).
    pub embargo_cut: f32,
    /// A gang's value a member (`branch_value`).
    pub member_value: i64,
    /// A gang's `cash`: treasury ÷ this.
    pub gang_cash_ref: i64,
    pub hysteresis: f32,
    pub min_hold_days: u32,
    /// The daily action's ceiling, a share of the parent's treasury.
    pub max_spend_frac: f32,
    pub subsidise_below: f32,
    pub subsidise_to: f32,
    /// Coins a day into one branch, at most.
    pub subsidy_cap_day: i64,
    pub flat_hold: f32,
    pub flat_subsidise: f32,
    // --- Phase 3: bodies.
    pub flat_reinforce: f32,
    pub flat_retake: f32,
    pub flat_found_branch: f32,
    pub flat_abandon: f32,
    /// A branch shock (a building lost, a raid) this recent opens Reinforce.
    pub recent_days: u32,
    pub reinforce_cooldown_days: u32,
    pub bodies_per_day: u32,
    pub bodies_per_call: u32,
    /// Living Outsiders per parent at most, at 2,000 residents (`scaled_to`).
    pub outsiders_cap: u32,
    /// Paid abroad per body (minted out).
    pub body_cost: i64,
    /// Into the branch per body (`Flow::ParentFund`): the new hire's wages.
    pub body_fund: i64,
    pub outsider_skill: f32,
    pub tour_days: u32,
    pub retake_window_days: u32,
    pub retake_wealth_min: f32,
    pub retake_premium: f32,
    /// A holder refusing this many buy-backs gets a `Retake` posted.
    pub refusals_to_force: u8,
    pub hit_lawfulness: f32,
    pub branch_seed: i64,
    pub upkeep_grace_days: u32,
    pub abandon_attitude: f32,
    pub abandon_days: u32,
    pub found_parent_treasury: i64,
    pub found_parent_share: f32,
    pub found_parent_days: u32,
    pub found_parent_frac: f32,
    pub parent_income_rate: f32,
    pub parent_upkeep_rate: f32,
    pub max_accounts: usize,
    // Syndicates.
    pub syndicate_subsidy: i64,
    pub brothers: u32,
    pub brother_delay_days: u32,
    pub brother_cost: i64,
    pub syndicate_cooldown_days: u32,
    pub refound_days: u32,
    pub refound_cost: i64,
    pub refound_attitude: f32,
    pub refound_window_days: u32,
    /// M17 phase 4b (Dylan, 2026-10-13: brothers as a rare event, not an
    /// occupancy): a syndicate sends brothers only to a gang sacked today
    /// with fewer than this many free members ...
    pub brothers_gutted_below: usize,
    /// ... on a keyed roll under this (the god `SendBrothers` aside).
    pub brothers_p: f32,
    // The State.
    /// The State's extra guards at most, at 2,000 residents (`scaled_to`).
    pub state_guards_max: u16,
    pub state_guards_step: u16,
    pub heat_murders: f32,
    pub mandate_heat: f32,
    pub mandate_bias: f32,
    pub mandate_days: u32,
    /// A State guard's day of wages in the weekly grant.
    pub grant_wage: i64,
    pub flat_fund: f32,
    pub flat_mandate: f32,
    pub flat_withdraw: f32,
    // --- Phase 4: the weapons that reach the ledger, and scrip.
    pub uplink_link_tier: u8,
    pub uplink_ice: u8,
    pub edge_per_unit: f32,
    pub edge_wipe: f32,
    pub edge_recover: f32,
    pub edge_tech: f32,
    pub edge_min: f32,
    pub edge_max: f32,
    pub data_floor: u32,
    pub data_per_day: [u32; 3],
    pub uplink_frac: f32,
    pub uplink_cap: i64,
    pub sanction_days: u32,
    pub sanction_cut: f32,
    pub sanction_cost: i64,
    pub sanction_regard: f32,
    pub state_sanction_heat: f32,
    pub campaign_margin: f32,
    pub flat_sanction: f32,
    pub flat_campaign: f32,
    pub death_days: u32,
    pub insolvent_days: u32,
    /// Data a parent ships a branch's Lab when its upkeep lapses, per tier (1-3).
    pub ship_data: [u32; 3],
    pub scrip_enabled: bool,
    pub scrip_wage_share: f32,
    pub scrip_par_min: f32,
    pub convert_min: i64,
    pub fixer_cut: f32,
    pub scrip_unrest: f32,
}

/// One seeded outside account. `treasury`, `income` and `upkeep` are per
/// branch (a brand dealt k times at 20k has k branches); the State's per
/// 2,000 residents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParentRow {
    pub name: String,
    pub kind: crate::outside::OutsideKind,
    /// A corp template (megacorps), a gang name (syndicates, by seeded
    /// order), or "Law" (the State).
    pub branch: String,
    pub treasury: i64,
    pub income: i64,
    pub upkeep: i64,
    pub exposure: f32,
    /// `[greed, pride, lawfulness]`.
    pub persona: [f32; 3],
    /// M17 phase 4: Data held abroad per track (Chrome, Deck, Industry).
    #[serde(default)]
    pub data: [u32; 3],
    /// M17 phase 4: the tech ceiling on its branches per track (0: none).
    #[serde(default)]
    pub tech: [u8; 3],
}

impl Default for ParentsCfg {
    fn default() -> Self {
        use crate::outside::OutsideKind::{Megacorp, State, Syndicate};
        let row = |name: &str, kind, branch: &str, t: i64, i: i64, u: i64, e: f32, p: [f32; 3]| ParentRow {
            name: name.to_string(),
            kind,
            branch: branch.to_string(),
            treasury: t,
            income: i,
            upkeep: u,
            exposure: e,
            persona: p,
            data: [0; 3],
            tech: [0; 3],
        };
        ParentsCfg {
            factions: vec![
                ParentRow {
                    data: [400, 1500, 400],
                    tech: [2, 3, 2],
                    ..row("Arasaka Global", Megacorp, "Arasaka", 100_000, 3000, 2500, 0.15, [0.6, 0.8, 0.3])
                },
                ParentRow {
                    data: [400, 1200, 600],
                    tech: [2, 3, 2],
                    ..row("Militech International", Megacorp, "Militech", 100_000, 2800, 2400, 0.2, [0.7, 0.6, 0.4])
                },
                row("The State", State, "Law", 60_000, 900, 900, 0.0, [0.2, 0.5, 0.8]),
                row("Hollow Brothers", Syndicate, "The Hollow", 3000, 40, 30, 0.3, [0.6, 0.7, 0.1]),
                row("Ninefold Lodge", Syndicate, "Ninefold", 3000, 40, 30, 0.3, [0.5, 0.8, 0.1]),
            ],
            market_sd: 0.01,
            market_revert: 0.1,
            remit_floor: 0.5,
            remit_share: 0.3,
            tariff: 0.0,
            tribute_share: 0.1,
            tribute_floor: 100,
            rep_rate: 0.05,
            att_decay: 0.05,
            att_remit: 0.3,
            att_cost: 0.4,
            att_loss: 0.3,
            embargo_cut: 0.08,
            member_value: 100,
            gang_cash_ref: 300,
            hysteresis: 0.10,
            min_hold_days: 3,
            max_spend_frac: 0.05,
            subsidise_below: 0.3,
            subsidise_to: 0.5,
            subsidy_cap_day: 1500,
            flat_hold: 0.3,
            flat_subsidise: 0.0,
            flat_reinforce: 0.0,
            flat_retake: 0.15,
            flat_found_branch: 0.0,
            flat_abandon: -0.2,
            recent_days: 14,
            reinforce_cooldown_days: 21,
            bodies_per_day: 2,
            bodies_per_call: 6,
            outsiders_cap: 24,
            body_cost: 300,
            body_fund: 300,
            outsider_skill: 0.6,
            tour_days: 60,
            retake_window_days: 30,
            retake_wealth_min: 0.2,
            retake_premium: 1.5,
            refusals_to_force: 2,
            hit_lawfulness: 0.4,
            branch_seed: 4000,
            upkeep_grace_days: 14,
            abandon_attitude: -0.5,
            abandon_days: 60,
            found_parent_treasury: 20000,
            found_parent_share: 0.5,
            found_parent_days: 30,
            found_parent_frac: 0.5,
            parent_income_rate: 0.02,
            parent_upkeep_rate: 0.017,
            max_accounts: 12,
            syndicate_subsidy: 150,
            brothers: 3,
            brother_delay_days: 5,
            brother_cost: 150,
            syndicate_cooldown_days: 30,
            refound_days: 21,
            refound_cost: 600,
            refound_attitude: -0.5,
            refound_window_days: 60,
            brothers_gutted_below: 6,
            brothers_p: 0.3,
            state_guards_max: 12,
            state_guards_step: 4,
            heat_murders: 10.0,
            mandate_heat: 0.6,
            mandate_bias: 0.2,
            mandate_days: 14,
            grant_wage: 8,
            flat_fund: 0.0,
            flat_mandate: 0.0,
            flat_withdraw: 0.0,
            uplink_link_tier: 3,
            uplink_ice: 3,
            edge_per_unit: 0.0003,
            edge_wipe: 0.15,
            edge_recover: 0.01,
            edge_tech: 0.02,
            edge_min: 0.2,
            edge_max: 1.5,
            data_floor: 100,
            data_per_day: [5, 10, 5],
            uplink_frac: 0.01,
            uplink_cap: 2000,
            sanction_days: 30,
            sanction_cut: 0.08,
            sanction_cost: 60,
            sanction_regard: -0.4,
            state_sanction_heat: 0.7,
            campaign_margin: 0.1,
            flat_sanction: -0.1,
            flat_campaign: -0.1,
            death_days: 21,
            insolvent_days: 7,
            ship_data: [0, 20, 40],
            scrip_enabled: true,
            scrip_wage_share: 0.3,
            scrip_par_min: 0.3,
            convert_min: 20,
            fixer_cut: 0.1,
            scrip_unrest: 0.5,
        }
    }
}

impl OutsideCfg {
    pub fn is_default(&self) -> bool {
        *self == OutsideCfg::default()
    }
}

impl Config {
    /// The pre-view pass phase 3's shapes off (`[shift]` gone): no `haul` or `counter` shape anywhere: `[shift.haul]`
    /// and `[shift.counter]` (its `roles` with it) dropped, every trade row's
    /// `shape` cleared: each role back on its `[shift.roles]` shape.
    pub fn drop_haul_counter(&mut self) {
        for t in &mut self.trades {
            t.shape = None;
        }
        if let Some(s) = self.shift.as_mut() {
            s.haul = None;
            s.counter = None;
        }
    }

    /// M17 phase 5 (the rename hazard): every corp, gang or Law reference the
    /// config makes by name or row that resolves to no row of `[corps] names`
    /// or `[gangs] names`, as readable lines. The shipped config must give
    /// none (`tests/names.rs`): renaming a row in `names` without its
    /// references fails there, loudly, instead of silently seeding nothing.
    pub fn unresolved_names(&self) -> Vec<String> {
        let corps: Vec<&str> = (0..self.corps.names.len()).map(|i| self.corps.template_of(i)).collect();
        let gangs = &self.gangs.names;
        let mut out = Vec::new();
        for t in self.tech.seed.keys().filter(|t| !corps.contains(&t.as_str())) {
            out.push(format!("[tech] seed names corp {t:?}"));
        }
        for b in self.governance.boards.iter().filter(|b| !corps.contains(&b.as_str())) {
            out.push(format!("[governance] boards names corp {b:?}"));
        }
        for f in &self.outside.parents.factions {
            let ok = match f.kind {
                crate::outside::OutsideKind::Megacorp => corps.contains(&f.branch.as_str()),
                crate::outside::OutsideKind::Syndicate => gangs.contains(&f.branch),
                crate::outside::OutsideKind::State => f.branch == "Law",
                crate::outside::OutsideKind::World => true,
            };
            if !ok {
                out.push(format!("[outside.parents] {:?} names branch {:?}", f.name, f.branch));
            }
        }
        let n = self.corps.names.len();
        let rows = self.corps.fab_rows.iter().map(|&r| ("fab_rows", r));
        let rows = rows.chain(self.corps.noodle_first_row.map(|r| ("noodle_first_row", r)));
        let rows = rows.chain(self.corps.seed_labs.iter().map(|s| ("seed_labs", s.row)));
        let rows = rows.chain(self.corps.seed_feeds.iter().map(|s| ("seed_feeds", s.row)));
        for (key, r) in rows.filter(|&(_, r)| r >= n) {
            out.push(format!("[corps] {key} names row {r} of {n}"));
        }
        // Lives at work phase 1: a `[[serves]]` key is a trade id or a legacy role's name.
        for s in &self.serves {
            let trade = self.trades.iter().any(|t| t.id == s.key);
            let legacy = Role::ALL.iter().any(|r| format!("{r:?}") == s.key);
            // Lives at work phase 4: the pimp and the dealer hold no trade
            // row (gang posts named by `vice::name_pimps` and `stims::is_dealer`).
            // Lives at work phase 5: nor the exec (no Job: `Corp.exec`).
            let gang_post = s.key == "pimp" || s.key == "dealer" || s.key == "exec";
            if !trade && !legacy && !gang_post {
                out.push(format!("[[serves]] key {:?} is neither a trade id nor a Role", s.key));
            }
        }
        // Lives at work phase 6 (§ 4.1): a `[roles.<Name>]` key is a legacy role's name.
        for k in self.roles.keys() {
            if !Role::ALL.iter().any(|r| format!("{r:?}") == *k) {
                out.push(format!("[roles] key {k:?} is not a Role"));
            }
        }
        // The pre-view pass phase 2: a `[shift.roles]` key is a built-in role's name.
        for k in self.shift.iter().flat_map(|s| s.roles.keys()) {
            if !Role::ALL.iter().any(|r| format!("{r:?}") == *k) {
                out.push(format!("[shift.roles] key {k:?} is not a Role"));
            }
        }
        out
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

/// Scale S4 (docs/SCALE_20K.md § S4): the reference city the config's
/// absolute keys were tuned on, the shipped map (`assets/map.txt`) at 2,000
/// residents. `Config::scaled_to` scales the per-person keys by `population
/// / this population`; `Config::fit_to_map` (at `World::new`) the
/// per-building keys by the map's count of the kind over the count here.
/// On the shipped map at 2,000 every ratio is 1. `enabled = false` (the v1
/// profile, whose map is its own reference) leaves the map's keys alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScaleCfg {
    pub enabled: bool,
    /// Residents the absolute keys assume.
    pub population: u32,
    /// The reference map's building counts.
    pub farms: u32,
    pub markets: u32,
    pub bars: u32,
    pub hideouts: u32,
    pub offices: u32,
    pub lots: u32,
    /// The reference map's districts (`[districts]` rows).
    pub districts: u32,
    /// Review finding 4: `fit_to_map` has run on this config (it is
    /// idempotent: a world's own config, fitted and saved, handed back to
    /// `World::new` by a replay is not fitted twice). Not a config.toml key;
    /// written into a save only when set.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fitted: bool,
    /// Pressure pass F2: the copies of the opening roster whose businesses
    /// stand on Lots (`fit_to_map`: the map's Lots over `lots`, at most the
    /// roster's copies; 0 before fitting: the roster's). Not a config.toml
    /// key; written into a save only when set.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lot_copies: u32,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl Default for ScaleCfg {
    fn default() -> Self {
        ScaleCfg {
            enabled: true,
            population: 2000,
            farms: 12,
            markets: 3,
            bars: 3,
            hideouts: 2,
            offices: 2,
            lots: 160,
            districts: 8,
            fitted: false,
            lot_copies: 0,
        }
    }
}

impl ScaleCfg {
    /// Off: `fit_to_map` changes nothing (the v1 profile).
    pub fn off() -> Self {
        ScaleCfg { enabled: false, ..ScaleCfg::default() }
    }
}

/// Transit (docs/TRANSIT.md § 2): the El's lines, timetable and the walk-or-
/// ride rule. Read only while `enabled`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransitCfg {
    pub enabled: bool,
    /// Coins per ride, transfers free (Dylan 2026-10-10: a flat 1).
    pub fare: i64,
    /// Ticks between departures from each terminus.
    pub headway: u32,
    pub ride_ticks_per_tile: f32,
    /// Ticks per stop passed.
    pub dwell_ticks: u32,
    /// Ticks per transfer (the wait at a transfer, folded into the table).
    pub transfer_ticks: u32,
    /// Track tiles between stops.
    pub stop_spacing: u32,
    /// The nearest stops tried at each end of a trip.
    pub stop_candidates: usize,
    /// No ride under this Manhattan distance.
    pub min_ride_tiles: u32,
    pub min_saving_ticks: u32,
    /// What a coin of fare must save in ticks ...
    pub ticks_per_coin: u32,
    /// ... and for a wallet under `poor_meals` meals.
    pub poor_ticks_per_coin: u32,
    pub poor_meals: i64,
    /// A rider who can't pay jumps the gate below this lawfulness ...
    pub dodge_lawfulness: f32,
    /// ... and never pays below this.
    pub dodge_always: f32,
    /// Phase 3: a rider arrested in the last this many days pays the fare
    /// when it can, whatever its lawfulness (the deterrent a night in the
    /// cells is; a broke rider still jumps).
    pub dodge_deterred_days: u32,
    /// Service hours, tick of day: from `service_from` to `service_to`
    /// (wrapping midnight).
    pub service_from: u16,
    pub service_to: u16,
    /// Night, for the danger test (wrapping midnight).
    pub night_from: u16,
    pub night_to: u16,
    /// `crime_rate` at a stop's district above which an average-courage
    /// rider (courage 0.5) refuses at night; the bar is
    /// `danger_crime × (0.5 + courage)`.
    pub danger_crime: f32,
    /// Coins per open line per day, the Treasury to the World (power).
    pub power_per_line_day: i64,
    /// The El's maintenance crews per open line: the cap on
    /// `transit::staff_target` (the fares past the power still bound it
    /// from above: no fares, no crews). A real metro staffs 0.2-0.8 % of
    /// its population; 4 a line on the shipped 3 lines is 12 posts (0.6 %
    /// of 2,000). Uncapped, the fares paid ~250 crews on seed 42. Missing
    /// (an old save's config, from before the cap): `u16::MAX`, uncapped,
    /// the behaviour it was saved under (review finding 5); the shipped
    /// config.toml sets 4.
    pub staff_per_line: u16,
    /// A fare dodger's fine when the Precinct is full (`Flow::Fine`).
    pub dodge_fine: i64,
    pub lines: Vec<TransitLineCfg>,
}

/// One `[[transit.lines]]` row: a name and its waypoints (road tiles, the
/// track runs straight between consecutive ones).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransitLineCfg {
    pub name: String,
    pub waypoints: Vec<[u16; 2]>,
    /// The pre-view pass phase 5: a line in flight (a map's `L+` line),
    /// dropped with `[legs]` off (`Map::apply_to`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_flight: bool,
}

impl Default for TransitCfg {
    fn default() -> Self {
        TransitCfg {
            enabled: false,
            fare: 1,
            headway: 10,
            ride_ticks_per_tile: 0.25,
            dwell_ticks: 1,
            transfer_ticks: 5,
            stop_spacing: 16,
            stop_candidates: 3,
            min_ride_tiles: 24,
            min_saving_ticks: 15,
            ticks_per_coin: 20,
            poor_ticks_per_coin: 60,
            poor_meals: 2,
            dodge_lawfulness: 0.25,
            dodge_always: 0.1,
            dodge_deterred_days: 7,
            service_from: 300,
            service_to: 60,
            night_from: 1260,
            night_to: 300,
            danger_crime: 3.0,
            power_per_line_day: 20,
            staff_per_line: u16::MAX,
            dodge_fine: 5,
            lines: Vec::new(),
        }
    }
}

impl TransitCfg {
    /// Not written into a save while off.
    pub fn is_off(&self) -> bool {
        !self.enabled
    }
}

/// J9 review: [`Config`]'s cache of the trade roles (row order) and of
/// each kind's `(role, staff_per_floor)` trades.
#[derive(Clone, Debug, Default)]
pub struct TradeCache(std::sync::OnceLock<TradeCacheData>);

#[derive(Clone, Debug)]
pub struct TradeCacheData {
    roles: Vec<Role>,
    by_kind: std::collections::BTreeMap<BuildingKind, Vec<(Role, usize)>>,
}

/// J9: a trade's shift (`demography::hire`, J12).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeShift {
    /// `[world] shift_day`.
    Day,
    /// `[leisure] evening_shift`.
    Evening,
    /// `[world] shift_night`.
    Night,
    /// Night for even ids, day for odd (the guards' rule).
    Split,
}

/// J9, J13: what pays a trade and what its shift makes (docs/JOBS_V2.md
/// § 1.2). The wage is paid by the workplace's owner at the shift's end
/// like any staff; `produce` accrues units per on-duty hour
/// (`trades::accrue`); `service` staff count as the building's on-duty
/// staff where its service reads them; the others name the flow that pays.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeModel {
    Produce,
    Service,
    Handling,
    Contract,
    Civic,
    Overhead,
    Resale,
    Day,
}

/// J9: the skill a trade's hiring ranks on and competence reads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeSkill {
    #[default]
    None,
    Farming,
    Fighting,
    Persuasion,
    Knowledge,
    Hacking,
}

/// J9: one `[[trades]]` row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeCfg {
    /// The stable key (saves store it, not the index).
    pub id: String,
    pub label: String,
    /// The kinds that employ it.
    pub workplace: Vec<BuildingKind>,
    /// Places per floor of a workplace (per group with `per_owned`).
    pub staff_per_floor: u8,
    pub wage: i64,
    pub shift: TradeShift,
    #[serde(default)]
    pub skill: TradeSkill,
    pub model: TradeModel,
    /// J14 (the Super): with `n > 0`, a corp owner posts `staff_per_floor`
    /// places per `n` of its standing buildings of the workplace kind (a
    /// group started counts), at the lowest-id building of each group of
    /// `n` (`trades::owned_places`); no other owner posts any. 0: every
    /// building posts its places per floor.
    #[serde(default)]
    pub per_owned: u16,
    /// J14: how the shift is worked (`post`, or the Super's `round`).
    #[serde(default)]
    pub duty: TradeDuty,
    /// Class pay (docs/CLASS_PAY.md): the post's grade (0 labour, 1
    /// skilled, 2 professional, 3 management), read only with class pay on.
    #[serde(default)]
    pub grade: u8,
    /// Class pay: the row exists only with class pay on
    /// (`classpay::trade_live`); off, it posts nothing anywhere.
    #[serde(default)]
    pub classpay: bool,
    /// Class pay: only a corp-owned workplace posts the row's places.
    #[serde(default)]
    pub corp_only: bool,
    /// J14 (a `round` duty): the doors a shift's round visits.
    #[serde(default)]
    pub round_stops: u8,
    /// The pre-view pass phase 2 (§ 1): this row's stations block in
    /// minutes (0: `[shift] station_block_min` / `station_block_max`).
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub station_block_min: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub station_block_max: u16,
    /// The pre-view pass phase 3 (§ 1): the row's shift shape when it is not
    /// its `duty`'s (`haul`, `counter`); the duty keeps its hiring and its
    /// Statistical path. `None` drops it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ShiftShape>,
    /// Jobs & lives phase 1 (§ 1): the row exists only with `[jobs2]
    /// enabled` (`classpay::trade_live`); off, it posts nothing anywhere.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub jobs2: bool,
    /// Jobs & lives phase 2: the row exists only with `[jobs2] street` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub street: bool,
    /// Jobs & lives phase 3: the row exists only with `[weapons]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub weapons: bool,
    /// Jobs & lives phase 4: the row exists only with `[vice]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vice: bool,
    /// Jobs & lives phase 4: the row exists only with `[fashion]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fashion: bool,
    /// Jobs & lives phase 5: the row exists only with `[vice] chip` on; its
    /// posts are filled only by the chip contract (never the hiring pass).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub chip: bool,
    /// Jobs & lives phase 5: the row exists only with `[vice] bd` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bd: bool,
    /// Jobs & lives phase 6: the row exists only with `[gray]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gray: bool,
    /// Jobs & lives phase 6: hired only from the struck-off (`[gray]` on).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub struck_off: bool,
    /// Jobs & lives phase 7: the row exists only with `[ages]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ages: bool,
    /// Jobs & lives phase 4: the row's on-duty staff take a visit's tip
    /// (`[vice] tip_share`) at a Vice venue or the Lounge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tipped: bool,
    /// Jobs & lives phase 4 (§ 9): the dress a hire needs (read with
    /// `[fashion]` on; 0 none).
    #[serde(default, skip_serializing_if = "is_zero_u8_cfg")]
    pub min_dress: u8,
    /// Lives at work phase 3 (§ 2.2, the super): a `round` duty's stops are
    /// the `round_stops` Blocks of the pool nearest the worker's Home (ties
    /// by id), not the id-ordered rotation. Read with `[service] civic`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub round_near: bool,
    /// Lives at work phase 3 (§ 2.2, the super): the worker and his
    /// household are housed rent-free in a Block of his employer's owner
    /// (`ownership::collect_rent`; the midnight move-in
    /// `household::tied_housing`). Read with `[service] civic`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tied_housing: bool,
    /// Lives at work phase 6 (§ 4.1): the row's hiring age band in years
    /// (0 none). Read with `[careers] enabled`; 0 (not written) off.
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub min_age: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub max_age: u32,
}

fn is_zero_usize_cfg(v: &usize) -> bool {
    *v == 0
}

fn is_zero_u8_cfg(v: &u8) -> bool {
    *v == 0
}

/// J14: how a trade works its shift.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeDuty {
    /// At the workplace for the shift (the generic `Work`).
    #[default]
    Post,
    /// The Super's round: `round_stops` of the employer's Blocks in the
    /// post's district, door to door (`trades::round_plan`), then the post.
    /// Jobs & lives phase 1: a Builder's round walks his Yard's open sites.
    Round,
    // --- Jobs & lives (§ 1): appended, worked from phase 2 (the street).
    /// Wait at a rank, carry a rider (cabs, rickshaws).
    Fare,
    /// The dawn pick at the labour corner.
    Corner,
    /// Preach at a spot (preachers).
    Preach,
    /// Sell at a Pitch (vendors).
    Sell,
}

/// J9: the `trades` field's reader: the rows, checked (unique keys, at most
/// 255), and their ids made the keys a save on this thread resolves against.
fn trades_de<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<TradeCfg>, D::Error> {
    let rows = Vec::<TradeCfg>::deserialize(d)?;
    if rows.len() > usize::from(u8::MAX) {
        return Err(serde::de::Error::custom("[[trades]]: at most 255 rows"));
    }
    for (i, r) in rows.iter().enumerate() {
        if rows[..i].iter().any(|o| o.id == r.id) {
            return Err(serde::de::Error::custom(format!("[[trades]]: duplicate id {:?}", r.id)));
        }
    }
    crate::components::TradeId::set_keys(trade_ids_of(&rows));
    Ok(rows)
}

fn trade_ids_of(rows: &[TradeCfg]) -> Vec<crate::components::TradeId> {
    rows.iter().enumerate().map(|(i, r)| crate::components::TradeId::new(i as u8, &r.id, &r.label)).collect()
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
            // M16a (plan C8): a Fixer office's staff are hired, never seeded.
            Role::Fixer => 0,
            // Real economy E26: a Mission's Volunteers are hired, never seeded.
            Role::Volunteer => 0,
            Role::Trade(_) => 0,
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
    pub capacity: u16,
    pub stock_cap: u32,
    /// M11 D17: full staff for an owned niche building (Grow tops up to it;
    /// an agent-owned Bar posts a vacancy below it). 0 = no rule.
    #[serde(default)]
    pub staff: u32,
    // --- Jobs & lives phase 1 (D4): the `KindCfg` columns. Each is
    // optional: an absent key keeps the kind's built-in value (the M11-M17
    // constants), so an older config reads as before.
    /// What founding or building the kind costs (the new kinds; the older
    /// kinds keep `[corps] found_cost`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found_cost: Option<i64>,
    /// What a building of the kind sells for (the new kinds; the older
    /// kinds keep `[corps] value`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i64>,
    /// Floors at seed (the new kinds; a map building's own count otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors_seed: Option<u8>,
    /// The most floors the kind may have, ahead of `[floors] floors_max`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors_max: Option<u8>,
    /// The corp niche the kind sits in (`corp_brain::niche_of_kind`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub niche: Option<crate::components::Niche>,
    /// An agent may found it: the tie-break rank (1 first), 0 never
    /// (`founding::foundable`, which was the `FOUNDABLE` list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foundable: Option<u8>,
}

/// Jobs & lives phase 1 (D4): a kind's row of the `[buildings.<kind>]`
/// table, read through [`BuildingsCfg::for_kind`] and the `KindCfg`
/// readers ([`Config::kind_niche`], [`Config::kind_foundable`]).
pub type KindCfg = BuildingCfg;

impl BuildingCfg {
    /// The `KindCfg` columns absent (the struct-update base of the
    /// constructors below).
    pub const BASE: BuildingCfg = BuildingCfg {
        capacity: 0,
        stock_cap: 0,
        staff: 0,
        found_cost: None,
        value: None,
        floors_seed: None,
        floors_max: None,
        niche: None,
        foundable: None,
    };

    /// Jobs & lives phase 1 (§ 2): the industries and the Hq.
    pub fn packing_plant() -> BuildingCfg {
        BuildingCfg {
            capacity: 20,
            stock_cap: 300,
            staff: 0,
            found_cost: Some(900),
            value: Some(900),
            floors_seed: Some(2),
            floors_max: Some(4),
            ..BuildingCfg::BASE
        }
    }
    pub fn freight_yard() -> BuildingCfg {
        BuildingCfg {
            capacity: 24,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(800),
            value: Some(800),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    pub fn utility_plant() -> BuildingCfg {
        BuildingCfg {
            capacity: 14,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(1000),
            value: Some(1000),
            floors_seed: Some(2),
            floors_max: Some(2),
            ..BuildingCfg::BASE
        }
    }
    pub fn construction_yard() -> BuildingCfg {
        BuildingCfg {
            capacity: 14,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(700),
            value: Some(700),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 3 (§ 4): a Gunsmith (Parts in stock).
    /// Jobs & lives phase 4 (§ 5.1): a StripClub (dancers, bouncers, a barkeep).
    pub fn strip_club() -> BuildingCfg {
        BuildingCfg {
            capacity: 24,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(700),
            value: Some(700),
            floors_seed: Some(1),
            floors_max: Some(2),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 4 (§ 5.1): a Brothel (its workers, a barkeep, a madam).
    pub fn brothel() -> BuildingCfg {
        BuildingCfg {
            capacity: 16,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(600),
            value: Some(600),
            floors_seed: Some(1),
            floors_max: Some(2),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 4 (§ 9): a Boutique (tailors, barbers).
    pub fn boutique() -> BuildingCfg {
        BuildingCfg {
            capacity: 10,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(500),
            value: Some(500),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 5 (§ 5.2): a Dollhouse (its contract hosts).
    pub fn dollhouse() -> BuildingCfg {
        BuildingCfg {
            capacity: 12,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(900),
            value: Some(900),
            floors_seed: Some(1),
            floors_max: Some(2),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 7 (§ 6.1): a School (its teachers).
    /// Lives at work phase 7: the Station's row with `[buildings.station]`
    /// absent (stations off: none stands).
    pub const STATION_OFF: BuildingCfg = BuildingCfg { capacity: 0, stock_cap: 0, staff: 0, ..BuildingCfg::BASE };

    pub fn school() -> BuildingCfg {
        BuildingCfg {
            capacity: 40,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(600),
            value: Some(600),
            floors_seed: Some(1),
            floors_max: Some(2),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 6 (§ 7): a gang's BlackClinic (organs in stock).
    pub fn black_clinic() -> BuildingCfg {
        BuildingCfg {
            capacity: 8,
            stock_cap: 40,
            staff: 0,
            found_cost: Some(500),
            value: Some(500),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 5 (§ 5.3): a BdStudio (its staff make units).
    pub fn bd_studio() -> BuildingCfg {
        BuildingCfg {
            capacity: 10,
            stock_cap: 60,
            staff: 0,
            found_cost: Some(700),
            value: Some(700),
            floors_seed: Some(1),
            floors_max: Some(1),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    pub fn gunsmith() -> BuildingCfg {
        BuildingCfg {
            capacity: 8,
            stock_cap: 60,
            staff: 0,
            found_cost: Some(600),
            value: Some(600),
            floors_seed: Some(1),
            floors_max: Some(1),
            niche: Some(crate::components::Niche::Security),
            ..BuildingCfg::BASE
        }
    }
    pub fn hq() -> BuildingCfg {
        BuildingCfg {
            capacity: 30,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(1500),
            value: Some(1500),
            floors_seed: Some(3),
            floors_max: Some(8),
            ..BuildingCfg::BASE
        }
    }

    /// No room, no stock: the M10 kinds that do nothing until M11.
    pub fn inert() -> BuildingCfg {
        BuildingCfg { capacity: 0, stock_cap: 0, staff: 0, ..BuildingCfg::BASE }
    }

    /// M12 D20: a Hotel holds its guests (capacity is capped by `[street]
    /// hotel_beds` and the interior when built).
    pub fn hotel() -> BuildingCfg {
        BuildingCfg { capacity: 12, stock_cap: 0, staff: 0, ..BuildingCfg::BASE }
    }

    /// M14 (plan V16): a Lab of four Researchers.
    pub fn lab() -> BuildingCfg {
        BuildingCfg { capacity: 8, stock_cap: 0, staff: 4, ..BuildingCfg::BASE }
    }

    /// M15 W36: a Feed of one Reporter (phase 4: `assets/config.toml`).
    pub fn feed() -> BuildingCfg {
        BuildingCfg { capacity: 6, stock_cap: 0, staff: 1, ..BuildingCfg::BASE }
    }

    /// L2 § 1 (spec values; `[buildings.club]` … in `assets/config.toml`).
    pub fn club() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 0, staff: 10, ..BuildingCfg::BASE }
    }
    pub fn arcade() -> BuildingCfg {
        BuildingCfg { capacity: 16, stock_cap: 0, staff: 3, ..BuildingCfg::BASE }
    }
    pub fn noodle_bar() -> BuildingCfg {
        BuildingCfg { capacity: 10, stock_cap: 60, staff: 3, ..BuildingCfg::BASE }
    }
    pub fn fight_pit() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 0, staff: 4, ..BuildingCfg::BASE }
    }
    pub fn den() -> BuildingCfg {
        BuildingCfg { capacity: 20, stock_cap: 0, staff: 4, ..BuildingCfg::BASE }
    }
    pub fn lounge() -> BuildingCfg {
        BuildingCfg { capacity: 16, stock_cap: 0, staff: 6, ..BuildingCfg::BASE }
    }
    pub fn fab() -> BuildingCfg {
        BuildingCfg { capacity: 14, stock_cap: 400, staff: 12, ..BuildingCfg::BASE }
    }

    /// M16a (spec § 2): a Fixer's office of one Fixer.
    pub fn fixer() -> BuildingCfg {
        BuildingCfg { capacity: 8, stock_cap: 0, staff: 1, ..BuildingCfg::BASE }
    }

    /// Real economy E26: a Mission's kitchen (12 cots, 60 meals of stock,
    /// two Volunteers).
    pub fn mission() -> BuildingCfg {
        BuildingCfg { capacity: 12, stock_cap: 60, staff: 2, ..BuildingCfg::BASE }
    }

    /// Real economy E37: a work camp (no staff role: the children run the
    /// shift and the owner is the employer of record).
    pub fn camp() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 120, staff: 0, ..BuildingCfg::BASE }
    }

    /// M16b phase 2 (spec § 4): a private prison (plan deviation: no staff
    /// role; its guards are its corp's posted ones).
    pub fn prison() -> BuildingCfg {
        BuildingCfg { capacity: 20, stock_cap: 100, staff: 0, ..BuildingCfg::BASE }
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
    /// M16a (plan C8): a Fixer's office (none stands with contracts off).
    #[serde(default = "BuildingCfg::fixer")]
    pub fixer: BuildingCfg,
    /// Real economy E26 (none stands with `[charity]` off).
    #[serde(default = "BuildingCfg::mission")]
    pub mission: BuildingCfg,
    /// Real economy E37 (none stands with `[camp]` off).
    #[serde(default = "BuildingCfg::camp")]
    pub camp: BuildingCfg,
    /// M16b phase 2: a corp's private prison.
    #[serde(default = "BuildingCfg::prison")]
    pub prison: BuildingCfg,
    /// Jobs & lives phase 1 (§ 2): none stands with `[jobs2]` off.
    #[serde(default = "BuildingCfg::packing_plant")]
    pub packing_plant: BuildingCfg,
    #[serde(default = "BuildingCfg::freight_yard")]
    pub freight_yard: BuildingCfg,
    #[serde(default = "BuildingCfg::utility_plant")]
    pub utility_plant: BuildingCfg,
    #[serde(default = "BuildingCfg::construction_yard")]
    pub construction_yard: BuildingCfg,
    #[serde(default = "BuildingCfg::hq")]
    pub hq: BuildingCfg,
    /// Jobs & lives phase 3 (§ 4): none stands with `[weapons]` off.
    #[serde(default = "BuildingCfg::gunsmith")]
    pub gunsmith: BuildingCfg,
    /// Jobs & lives phase 4: none stands with `[vice]` / `[fashion]` off.
    #[serde(default = "BuildingCfg::strip_club")]
    pub strip_club: BuildingCfg,
    #[serde(default = "BuildingCfg::brothel")]
    pub brothel: BuildingCfg,
    #[serde(default = "BuildingCfg::boutique")]
    pub boutique: BuildingCfg,
    /// Jobs & lives phase 5: none stands with `[vice] chip` / `bd` off.
    #[serde(default = "BuildingCfg::dollhouse")]
    pub dollhouse: BuildingCfg,
    #[serde(default = "BuildingCfg::bd_studio")]
    pub bd_studio: BuildingCfg,
    /// Jobs & lives phase 6: none stands with `[gray]` off.
    #[serde(default = "BuildingCfg::black_clinic")]
    pub black_clinic: BuildingCfg,
    /// Jobs & lives phase 7: none stands with `[ages]` off.
    #[serde(default = "BuildingCfg::school")]
    pub school: BuildingCfg,
    /// Lives at work phase 7 (§ 5.2): the Station's seats (its guards and
    /// its held; the cells are `[law] station_cells`). Absent (and not
    /// written) with `[law] stations` off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<BuildingCfg>,
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
            BuildingKind::Fixer => &self.fixer,
            BuildingKind::Mission => &self.mission,
            BuildingKind::Camp => &self.camp,
            BuildingKind::Prison => &self.prison,
            BuildingKind::PackingPlant => &self.packing_plant,
            BuildingKind::FreightYard => &self.freight_yard,
            BuildingKind::UtilityPlant => &self.utility_plant,
            BuildingKind::ConstructionYard => &self.construction_yard,
            BuildingKind::Hq => &self.hq,
            BuildingKind::Gunsmith => &self.gunsmith,
            BuildingKind::StripClub => &self.strip_club,
            BuildingKind::Brothel => &self.brothel,
            BuildingKind::Boutique => &self.boutique,
            BuildingKind::Dollhouse => &self.dollhouse,
            BuildingKind::BdStudio => &self.bd_studio,
            BuildingKind::BlackClinic => &self.black_clinic,
            BuildingKind::School => &self.school,
            BuildingKind::Station => self.station.as_ref().unwrap_or(&BuildingCfg::STATION_OFF),
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
    /// Pressure pass F2: a crime's witnesses who note it (a `SawCrime`, an
    /// edge, a `Witness` line), at most; the guards on shift are rolled first
    /// and always count (a fare dodge on a 100k platform drew 26 witnesses
    /// in a minute, 641k `Witness` lines in 30 days). 0: no cap.
    #[serde(default = "CrimeCfg::default_witness_cap")]
    pub witness_cap: u32,
}

impl CrimeCfg {
    fn default_answer_radius() -> u32 {
        96
    }

    fn default_witness_cap() -> u32 {
        4
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
    /// M16b § 4: the spec's 48 h (`[law.sentence_hours] fraud`).
    pub fraud: u32,
    /// M16b § 4: the spec's 72 h.
    pub blackmail: u32,
    /// Jobs & lives phase 3 (§ 4): a dealer's weapon sale, a seized crate.
    pub arms_dealing: u32,
    pub smuggling: u32,
}

impl Default for SentenceDaysExt {
    fn default() -> Self {
        SentenceDaysExt {
            grand_theft: 6,
            dealing: 5,
            abduction: 20,
            manslaughter: 10,
            intrusion: 2,
            data_theft: 5,
            fraud: 2,
            blackmail: 3,
            arms_dealing: 6,
            smuggling: 8,
        }
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
    /// The pressure pass phase 4 (OPEN-210): an Acquaintance this weak
    /// (`|affinity|` below it) is pruned after `weak_prune_days` without
    /// contact.
    pub weak_affinity: f32,
    /// Days without contact before a weak Acquaintance is pruned.
    pub weak_prune_days: u64,
    /// The most Acquaintances one person keeps; past it, the daily pass
    /// drops the weakest (`|affinity|`), then the stalest. Friends, kin,
    /// spouses, Rivals, Enemies and an edge holding a debt are never
    /// dropped or counted.
    pub acquaintance_cap: usize,
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
    /// The perf pass phase 7d (OPEN-211): a gang still on `Order::Job` after its job settled, with the next
    /// order already held for the giving in person (`Gang.pending_order`), does not rescore every tick; it
    /// rescores at the daily tick or on a shock rethink.
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
    /// Shadow V4 bug 8: a member this many days in the gang outranks a newer
    /// one for the lead (0 = loyalty and tenure alone).
    #[serde(default)]
    pub leader_min_days: u32,
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
    /// Shadow V4 bug 11: no Hideout bed further than this many tiles from
    /// the member (0 = any distance).
    #[serde(default)]
    pub hole_up_max_tiles: u32,
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
    /// Lives at work phase 6 (§ 4.5): lieutenants, delegation, the cut and
    /// the ladder, under the `[lives]` master (`[gangs] lieutenants = false`).
    /// Off, every key below is 0 and none is written into a save.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lieutenants: bool,
    /// Rank 1 to the top `1 + members / lieutenant_per` after the leader.
    #[serde(default, skip_serializing_if = "is_zero_usize_cfg")]
    pub lieutenant_per: usize,
    /// The leader delegates the march and the shakedowns while the gang
    /// has this many fit members.
    #[serde(default, skip_serializing_if = "is_zero_usize_cfg")]
    pub delegate_min_fit: usize,
    /// The payout's cut off the top: the leader's, and the lieutenants' shared.
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub leader_cut: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub lieutenant_cut: f32,
    /// The leader score's weight per raid won as its leader.
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub raid_w: f32,
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

    /// Lives at work phase 6 (§ 4.5): the lieutenants' keys off (0, not written).
    pub fn strip_lieutenants(&mut self) {
        self.lieutenants = false;
        self.lieutenant_per = 0;
        self.delegate_min_fit = 0;
        self.leader_cut = 0.0;
        self.lieutenant_cut = 0.0;
        self.raid_w = 0.0;
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
    /// M16a (plan C26, phase 2): the gang's `Order::Job`.
    #[serde(default)]
    pub job: f32,
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
    // --- M16a (spec § 3 "the law's view", plan C28-C31): read only behind
    // `contracts::on`, by phase 3. Spec values as defaults.
    /// C28: a taker jailed within this many days of a fulfilled Hit or Beat is interrogated.
    #[serde(default = "LawCfg::d_interrogate_days")]
    pub interrogate_days: u32,
    /// C28: added to the interrogation move's bonus.
    #[serde(default)]
    pub interrogate_bias: f32,
    /// C29: a `Hired` rumour this sure names the buyer to the law.
    #[serde(default = "LawCfg::d_accessory_conf")]
    pub accessory_conf: f32,
    /// C29: the Conspiracy sentence is this × Murder's.
    #[serde(default = "LawCfg::d_accessory_mult")]
    pub accessory_mult: f32,
    /// C30: a Fixer's heat per fulfilled brokered Hit.
    #[serde(default = "LawCfg::d_heat_per_hit")]
    pub heat_per_hit: f32,
    /// C30: a Fixer's heat per interrogation naming it.
    #[serde(default = "LawCfg::d_heat_per_named")]
    pub heat_per_named: f32,
    /// C30: a Fixer's heat decay per day.
    #[serde(default = "LawCfg::d_heat_decay")]
    pub heat_decay: f32,
    /// C30: the office is busted at this heat.
    #[serde(default = "LawCfg::d_warrant_heat")]
    pub warrant_heat: f32,
    /// C30: days a busted office stays closed.
    #[serde(default = "LawCfg::d_close_days")]
    pub close_days: u32,
    /// C30, C31: heat a taken bribe or a guard on the take takes off.
    #[serde(default = "LawCfg::d_bribe_cool")]
    pub bribe_cool: f32,
    /// C21: a Fixer passes `Hired` to a regular at `fixer_talk × (1 − heat)`.
    #[serde(default = "LawCfg::d_fixer_talk")]
    pub fixer_talk: f32,
    /// C29: the accessory window after a fulfilment.
    #[serde(default = "LawCfg::d_accessory_days")]
    pub accessory_days: u32,
    /// C31: a city guard below this lawfulness may take a Guard record on a buyer.
    #[serde(default = "LawCfg::d_corrupt_lawfulness")]
    pub corrupt_lawfulness: f32,
    /// C30, C31: a Fixer owner at this heat bribes and may buy a guard.
    #[serde(default = "LawCfg::d_corrupt_heat")]
    pub corrupt_heat: f32,
    // --- Lives at work phase 7 (§ 5.2): district stations, under the
    // `[lives]` master (`[law] stations = false`). Off, every key below is
    // 0 and none is written into a save (`LawCfg::strip_stations`).
    /// Seed a Station in the districts far from the Precinct; guards post
    /// there and the escort books at the nearest with a free cell.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stations: bool,
    /// A district gets a Station only when its Lot's door is at least this
    /// many tiles (Manhattan) from the Precinct's door and from every
    /// Station already placed.
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub station_min_tiles: u32,
    /// A Station's cells: the held it books before an escort is sent on.
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub station_cells: u32,
    /// The van's hour, in ticks of the day: every held body whose sentence
    /// runs past the next van goes to the Precinct.
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub transfer_hour: u16,
    /// The share of a Station's guards on its desk a shift (the rest patrol).
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub station_desk_share: f32,
}

impl LawCfg {
    fn d_interrogate_days() -> u32 {
        30
    }
    fn d_accessory_conf() -> f32 {
        0.5
    }
    fn d_accessory_mult() -> f32 {
        0.75
    }
    fn d_heat_per_hit() -> f32 {
        0.15
    }
    fn d_heat_per_named() -> f32 {
        0.3
    }
    fn d_heat_decay() -> f32 {
        0.02
    }
    fn d_warrant_heat() -> f32 {
        0.8
    }
    fn d_close_days() -> u32 {
        7
    }
    fn d_bribe_cool() -> f32 {
        0.3
    }
    fn d_fixer_talk() -> f32 {
        0.3
    }
    fn d_accessory_days() -> u32 {
        60
    }
    fn d_corrupt_lawfulness() -> f32 {
        0.3
    }
    fn d_corrupt_heat() -> f32 {
        0.3
    }
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
    /// Lives at work phase 7 (§ 5.2): the stations' keys off (0, not written).
    pub fn strip_stations(&mut self) {
        self.stations = false;
        self.station_min_tiles = 0;
        self.station_cells = 0;
        self.transfer_hour = 0;
        self.station_desk_share = 0.0;
    }

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
    // --- Jobs and room P6 (docs/JOBS_V2.md § 4, plan J19-J21): immigration
    // that answers the city. Read only with the market wage law.
    /// J19: the wage a migrant would earn outside (the Harris-Todaro `R`);
    /// the `SetOutsideWage` lever (`EconState.outside_wage_pin`) overrides it.
    #[serde(default = "default_outside_wage")]
    pub outside_wage: f32,
    /// J19: `pull = clamp((E − R) ÷ R, 0, pull_cap)`.
    #[serde(default = "default_pull_cap")]
    pub pull_cap: f32,
    /// J19: migrants a week at full pull, full beds and a factor of 1.
    #[serde(default = "default_migrants_max")]
    pub migrants_max: u32,
    /// J19: empty Block beds at which the beds term reads 1 (floored at 0.25).
    #[serde(default = "default_beds_ref")]
    pub beds_ref: f32,
    /// J20: a vacancy open this many days draws a recruited migrant.
    #[serde(default = "default_offer_days")]
    pub offer_days: u64,
    /// J20: recruited migrants a week at most (on top of the pull's week).
    #[serde(default = "default_offers_max")]
    pub offers_max: u32,
    /// J21: a migrant's age in years, `[lo, hi)`.
    #[serde(default = "default_migrant_age")]
    pub migrant_age: [f32; 2],
    /// J21: an offer's role skill is at least this.
    #[serde(default = "default_offer_skill")]
    pub offer_skill: f32,
    /// J21: the chance a migrant brings a spouse (into the same Block).
    #[serde(default = "default_p_spouse")]
    pub p_spouse: f64,
    /// Dylan's open question (spec, "Immigrant families"): children a
    /// couple may bring, at most (0: spouse only, the plan's J21).
    #[serde(default)]
    pub migrant_children: u8,
}

fn default_outside_wage() -> f32 {
    3.5
}
fn default_pull_cap() -> f32 {
    1.0
}
fn default_migrants_max() -> u32 {
    20
}
fn default_beds_ref() -> f32 {
    60.0
}
fn default_offer_days() -> u64 {
    3
}
fn default_offers_max() -> u32 {
    6
}
fn default_migrant_age() -> [f32; 2] {
    [18.0, 45.0]
}
fn default_offer_skill() -> f32 {
    0.4
}
fn default_p_spouse() -> f64 {
    0.2
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
    /// Shadow V4 bug 20: a queue entry this old is served before the more
    /// urgent ones (oldest first), so it is planned before it goes stale
    /// (0 = urgency alone).
    #[serde(default)]
    pub plan_age_promote_ticks: u64,
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
    /// Scale S3a: the flow fields' byte budget in MiB (caps
    /// `flow_field_cache` by the map's size: `FlowCache::cap_for`).
    #[serde(default = "default_flow_budget_mb")]
    pub flow_field_budget_mb: usize,
    /// Scale S3a: maps of more tiles than this walk by exact A* paths
    /// (`exec::bigpath`) instead of per-door flow fields.
    #[serde(default = "default_flow_max_tiles")]
    pub flow_field_max_tiles: usize,
    /// Workplace room: an agent refused at a full building
    /// (`FailReason::BuildingFull`) passes it over in `World::local` for
    /// this many ticks, so the replan goes elsewhere (another Market, the
    /// home pantry) instead of back to the same door (0: off).
    #[serde(default)]
    pub refused_ticks: u64,
    /// Workplace room: a building's own staff are let in when it is full
    /// (the staff door; the trades' J13 rule for every role), so customers
    /// never lock the shift out (false: the trades and the sweepers only).
    #[serde(default)]
    pub staff_door: bool,
}

fn default_flow_cache() -> usize {
    512
}

fn default_flow_budget_mb() -> usize {
    64
}

fn default_flow_max_tiles() -> usize {
    131_072
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
    /// `docs/history/EXPERIMENT_LEARNED_STAT_POLICY.md`).
    #[serde(default = "default_stat_policy")]
    pub policy: String,
    /// M13 D46: a Statistical kitted agent's `p_robbed × (1 + flash_w × Kit.flash)`.
    #[serde(default)]
    pub flash_w: f32,
    /// M13 D37: the binder weights an Assaulted or Killed candidate by
    /// `1 + chrome_bind_w × Kit.fighting`.
    #[serde(default)]
    pub chrome_bind_w: f32,
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
    /// M16a (plan C35, phase 4): live contract parties' bodies (class 4).
    #[serde(default = "LodCfg::default_contract_quota")]
    pub contract_quota: usize,
    /// M16a (plan C35, phase 4): Fixer owners' bodies (class 2) in the
    /// matching window.
    #[serde(default = "LodCfg::default_fixer_quota")]
    pub fixer_quota: usize,
    /// Scale S1: a Statistical worker's hour of co-work drifts it with its
    /// crew, the `coworker_crew` co-workers of its role and employer nearest
    /// it in id order (half each side, wrapping), not with every one: the
    /// pairs grew with the square of a workplace's staff (a 20,000 city's
    /// Farms hold ~300). 0 is every co-worker (the pre-S1 rule); a group of
    /// at most `coworker_crew + 1` is every co-worker either way.
    #[serde(default)]
    pub coworker_crew: usize,
}

impl LodCfg {
    fn default_true() -> bool {
        true
    }
    fn default_contract_quota() -> usize {
        24
    }
    fn default_fixer_quota() -> usize {
        4
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
    /// M16a (plan C8; `founding::found_cost` prices it only with `contracts::on`).
    #[serde(default = "FoundCostCfg::default_fixer")]
    pub fixer: i64,
    /// Real economy E26 (priced only with `charity::on`).
    #[serde(default = "FoundCostCfg::default_mission")]
    pub mission: i64,
    /// Real economy E37 (corps only, with `camp::on`).
    #[serde(default = "FoundCostCfg::default_camp")]
    pub camp: i64,
}

impl FoundCostCfg {
    fn default_mission() -> i64 {
        150
    }
    fn default_camp() -> i64 {
        600
    }
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
    fn default_fixer() -> i64 {
        400
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
    /// Shadow V4 bug 11: a private guard's warrants lie within this many
    /// tiles of its office or a client of its corp.
    #[serde(default)]
    pub private_beat_tiles: u32,
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
    /// Scale S4: each row's template, the shipped row it copies (empty: every
    /// row is its own, `names`). `Config::fit_to_map` deals the roster once
    /// per ~2,000 residents' worth of employers ("Nutrix", "Nutrix 2", ...);
    /// the name-keyed seeding (the tech trees, the Labs, the Fabs, the Feeds)
    /// reads a row's template, so every copy of Zetatech is a Tech corp
    /// seeded as Zetatech.
    #[serde(default)]
    pub template: Vec<String>,
    /// M17 phase 5 (the rename hazard): the seeding roles by row index into
    /// `names` (a row's template at 20k), never by a corp's name, so a renamed
    /// row keeps its role. The Fab owners, dealt in turn (Zetatech, Militech).
    #[serde(default = "CorpsCfg::default_fab_rows")]
    pub fab_rows: Vec<usize>,
    /// The Food row the Noodle Bars' round-robin starts from (Nutrix).
    #[serde(default = "CorpsCfg::default_noodle_first_row")]
    pub noodle_first_row: Option<usize>,
    /// The seeded Labs (M14 V26): owner row, district by zone, focus track.
    #[serde(default = "CorpsCfg::default_seed_labs")]
    pub seed_labs: Vec<SeedSite>,
    /// The seeded corp Feeds (M15 W41, "<owner> Now"): owner row, district by zone.
    #[serde(default = "CorpsCfg::default_seed_feeds")]
    pub seed_feeds: Vec<SeedSite>,
}

/// M17 phase 5: a seeded building's owner row and seat (`seed_labs`, `seed_feeds`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeedSite {
    pub row: usize,
    pub zone: crate::components::Zone,
    #[serde(default)]
    pub track: crate::virt::Track,
}

impl Default for CorpsCfg {
    fn default() -> Self {
        CorpsCfg::none()
    }
}

impl CorpsCfg {
    fn default_fab_rows() -> Vec<usize> {
        vec![8, 7]
    }
    fn default_noodle_first_row() -> Option<usize> {
        Some(0)
    }
    fn default_seed_labs() -> Vec<SeedSite> {
        use crate::components::Zone;
        use crate::virt::Track;
        let site = |row, zone, track| SeedSite { row, zone, track };
        vec![
            site(8, Zone::Spire, Track::Chrome),
            site(8, Zone::Civic, Track::Industry),
            site(6, Zone::Vats, Track::Deck),
            site(7, Zone::Mid, Track::Deck),
        ]
    }
    fn default_seed_feeds() -> Vec<SeedSite> {
        vec![SeedSite { row: 0, zone: crate::components::Zone::Mid, track: crate::virt::Track::Industry }]
    }

    /// Row `i`'s template when `i` is a row of the shipped roster, else `None`.
    pub fn row_template(&self, i: usize) -> Option<&str> {
        (i < self.names.len()).then(|| self.template_of(i))
    }

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
                fixer: 400,
                mission: 150,
                camp: 600,
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
                fixer: 8,
                mission: 0,
                camp: 2,
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
                fixer: 500,
                mission: 300,
                camp: 800,
            },
            // A pre-M11 save (and v1_profile) shops at the nearest Market.
            shop_price_tiles: 0,
            grow_cooldown_days: 7,
            secure_per_day: 2,
            private_pursuit_radius: 16,
            private_beat_tiles: 40,
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
            template: Vec::new(),
            fab_rows: Vec::new(),
            noodle_first_row: None,
            seed_labs: Vec::new(),
            seed_feeds: Vec::new(),
        }
    }

    /// Scale S4: row `i`'s template, the shipped row it copies (its own
    /// name when the roster was never dealt again).
    pub fn template_of(&self, i: usize) -> &str {
        self.template.get(i).or_else(|| self.names.get(i)).map_or("", String::as_str)
    }

    /// Scale S4: the roster dealt `k` times (row `i + j × n` copies row `i`,
    /// named "<name> <j + 1>" past the first copy), the treasuries split `k`
    /// ways and each holding column re-dealt over the copies so that they ask
    /// for `[farms, markets, offices, blocks]` of `totals` together (largest
    /// remainder by the row's own count, ties to the lower row). `k = 1`
    /// with totals equal to the columns' sums changes nothing.
    pub fn replicate(&mut self, k: usize, totals: [u32; 4]) {
        let n = self.row_count();
        let k = k.max(1);
        let rows = |v: &Vec<u32>| -> Vec<u32> { (0..k).flat_map(|_| v.iter().copied()).collect() };
        let deal = |weights: Vec<u32>, total: u32| -> Vec<u32> {
            let sum: u64 = weights.iter().map(|&w| u64::from(w)).sum();
            if sum == 0 {
                return weights;
            }
            let exact: Vec<(u64, u64)> = weights
                .iter()
                .map(|&w| (u64::from(w) * u64::from(total) / sum, u64::from(w) * u64::from(total) % sum))
                .collect();
            let mut out: Vec<u32> = exact.iter().map(|&(q, _)| q as u32).collect();
            let left = u64::from(total) - exact.iter().map(|&(q, _)| q).sum::<u64>();
            let mut order: Vec<usize> = (0..weights.len()).filter(|&i| weights[i] > 0).collect();
            order.sort_by_key(|&i| (std::cmp::Reverse(exact[i].1), i));
            for &i in order.iter().take(left as usize) {
                out[i] += 1;
            }
            out
        };
        let template: Vec<String> = (0..k).flat_map(|_| (0..n).map(|i| self.template_of(i).to_string())).collect();
        self.names = (0..k)
            .flat_map(|j| (0..n).map(move |i| (i, j)))
            .map(|(i, j)| if j == 0 { self.names[i].clone() } else { format!("{} {}", self.names[i], j + 1) })
            .collect();
        self.niches = (0..k).flat_map(|_| self.niches.iter().cloned()).collect();
        self.megacorp = (0..k).flat_map(|_| (0..n).map(|i| self.megacorp.get(i).copied().unwrap_or(false))).collect();
        self.farms = deal(rows(&self.farms), totals[0]);
        self.markets = deal(rows(&self.markets), totals[1]);
        self.offices = deal(rows(&self.offices), totals[2]);
        self.blocks = deal(rows(&self.blocks), totals[3]);
        let kk = k as f64;
        self.treasury_initial =
            (0..k).flat_map(|_| self.treasury_initial.iter().map(|&t| (t as f64 / kk).round() as i64)).collect();
        self.template = template;
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
    /// Hunger as sickness P4b (Dylan, J25: the evicted land on the lower
    /// rungs, not the street): the squat rung grows with demand. Each night
    /// while the rough sleepers outnumber the free squat slots, up to
    /// `shanties_per_night` vacant Lots in the districts with the most rough
    /// sleepers go up as shanties: the Lot stays a Lot (a builder may
    /// clear it), derelict, ownerless, `shanty_capacity` slots; at most
    /// `shanty_lots_max` Lots at once, so the construction P2 kept them for
    /// goes on (always on since P10 with the street rung, `enabled`).
    #[serde(default = "default_shanty_capacity")]
    pub shanty_capacity: u8,
    #[serde(default = "default_shanties_per_night")]
    pub shanties_per_night: usize,
    #[serde(default = "default_shanty_lots_max")]
    pub shanty_lots_max: usize,
    /// P4b (with `shanties`): a Hotel booked full at the night's pass adds a
    /// floor of beds, paid by its owner at `floor_cost`, up to this many floors.
    #[serde(default = "default_hotel_floors_max")]
    pub hotel_floors_max: u8,
}

fn default_hotel_floors_max() -> u8 {
    4
}

fn default_shanty_lots_max() -> usize {
    20
}

fn default_shanty_capacity() -> u8 {
    24
}

fn default_shanties_per_night() -> usize {
    3
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
            shanty_capacity: 24,
            shanties_per_night: 3,
            shanty_lots_max: 20,
            hotel_floors_max: 4,
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
    /// Jobs & lives phase 3.
    #[serde(default)]
    pub weapon: T,
    /// Jobs & lives phase 4.
    #[serde(default)]
    pub clothing: T,
    /// Jobs & lives phase 5.
    #[serde(default)]
    pub recorder: T,
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
            AssetClass::Weapon => &self.weapon,
            AssetClass::Clothing => &self.clothing,
            AssetClass::Recorder => &self.recorder,
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
                weapon: vec![60, 160, 400],
                clothing: vec![20, 60, 180],
                recorder: vec![120],
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
                weapon: vec![0, 0, 0],
                clothing: vec![0, 0, 0],
                recorder: vec![0],
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
                weapon: 0,
                clothing: 1,
                recorder: 0,
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
                weapon: 3,
                clothing: 0,
                recorder: 0,
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
    /// Lives at work phase 4 (§ 2.3): a dealer stands a corner (a HangOut
    /// spot of the gang's most-held district) when the gang has no venue,
    /// instead of the nearest Bar. Read with `[service] street`; off in a
    /// save at false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deal_corner: bool,
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
            deal_corner: false,
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
    pub guard_count: u16,
    pub immigration_per_week: u16,
    /// P10: the dole's rate while the dole is enacted (`[policy] dole` at
    /// seed, the `safety_net` lever); a city without it seeds 0.
    pub dole_per_day: u8,
    /// M12 D23: city Sanitation workers (0 in a pre-M12 save: nobody sweeps).
    #[serde(default)]
    pub sanitation_count: u16,
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

/// P10 (docs/P10_POLICY.md): the city's policy at world creation. It seeds
/// the saved policy levers (`levers::Levers`); after day 0 only the levers
/// are read (`--lever day=D:<key>=<value>`, `PlayerCommand::SetPolicy`).
/// The default is the shipped city: the market wage law and no safety net.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyCfg {
    /// The levy (the M11 economy, the dole city's): the config wage
    /// schedule, the corps' upkeep levy to the Treasury and L2's budget band
    /// (public works sized by the surplus, the levy cut), the Treasury's
    /// scavenge coin. Off: wages from revenue, inputs and the property rate,
    /// the tax band and civic budget, the Recycler's till (the Real economy).
    pub levy: bool,
    /// The dole at `[levers] dole_per_day` a day, paid by the Treasury.
    pub dole: bool,
    /// Public works: Sanitation posts the Treasury band opens above its top.
    pub public_works: bool,
    /// School meals: a child whose pantry is empty eats from the Reserve.
    pub school_meals: bool,
    /// The Reserve's free release to a stranded Market (L2 phase 5).
    pub reserve_release: bool,
    /// The dole and public works lapse after this many days in a row on
    /// which the Treasury could not pay (0: never).
    pub lapse_days: u16,
}

impl Default for PolicyCfg {
    fn default() -> Self {
        PolicyCfg {
            levy: false,
            dole: false,
            public_works: false,
            school_meals: false,
            reserve_release: false,
            lapse_days: 7,
        }
    }
}

impl PolicyCfg {
    /// The dole city's policy (the old default before the flip): the levy
    /// and the whole safety net.
    pub fn dole_city() -> PolicyCfg {
        PolicyCfg {
            levy: true,
            dole: true,
            public_works: true,
            school_meals: true,
            reserve_release: true,
            ..PolicyCfg::default()
        }
    }
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
        VirtCfg {
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
}

impl IceCfg {
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
    /// Jobs & lives phase 3.
    pub weapon: Vec<u8>,
    /// Jobs & lives phase 4.
    pub clothing: Vec<u8>,
    /// Jobs & lives phase 5.
    pub recorder: Vec<u8>,
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
            AssetClass::Weapon => &self.weapon,
            AssetClass::Clothing => &self.clothing,
            AssetClass::Recorder => &self.recorder,
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
            // Jobs & lives: a Vice corp (phase 4) researches as Tech for now.
            Niche::Tech | Niche::Vice => &self.tech,
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
                weapon: vec![1, 2, 3],
                clothing: Vec::new(),
                recorder: Vec::new(),
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
}

impl TechCfg {
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
    /// M16a (plan C21): the `Hired` deed (a missing key reads 0).
    pub hired: f32,
    /// Real economy E29: the `Gave` deed (a missing key reads 0).
    pub gave: f32,
    /// M16b § 4: the `Blackmailed` deed (a missing key reads 0).
    pub blackmailed: f32,
    /// M16b phase 2: the `Rescued` deed (a missing key reads 0).
    pub rescued: f32,
    /// Lives at work phase 2 (§ 3.6): the work and household deeds (a
    /// missing key reads 0; left out of a save at 0, and zeroed by the off
    /// switches, so an off save is byte-identical).
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub employed: f32,
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub laid_off: f32,
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub born: f32,
    /// Lives at work phase 5 (§ 3.6): the exec's decision (left out of a
    /// save at 0; zeroed by the tower's off switches).
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub decided: f32,
}

fn is_zero_f32_deed(v: &f32) -> bool {
    *v == 0.0
}

impl DeedTable {
    /// Lives at work phase 2: the phase's keys at 0 (the off switches).
    pub fn without_household_deeds(&mut self) {
        self.employed = 0.0;
        self.laid_off = 0.0;
        self.born = 0.0;
    }
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
            Deed::Hired => self.hired,
            Deed::Gave => self.gave,
            Deed::Blackmailed => self.blackmailed,
            Deed::Rescued => self.rescued,
            Deed::Employed => self.employed,
            Deed::LaidOff => self.laid_off,
            Deed::Born => self.born,
            Deed::Decided => self.decided,
        }
    }

    /// The table from values in `Deed::ALL` order.
    pub fn of(v: [f32; 17]) -> DeedTable {
        let [killed, assaulted, robbed, extorted, stripped, arrested, married, evicted, struck, raided, founded, avenged, betrayed, repaid, poached, hired, gave] =
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
            hired,
            gave,
            blackmailed: 0.0,
            rescued: 0.0,
            employed: 0.0,
            laid_off: 0.0,
            born: 0.0,
            decided: 0.0,
        }
    }
}

/// M15 § 1: the word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GossipCfg {
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
    /// W10: kin captured at death.
    pub kin_cap: usize,
}

impl Default for GossipCfg {
    fn default() -> Self {
        GossipCfg {
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
            // M16b § 4: `blackmailed` reach0 0.2, sal 0.5, sev 0.4.
            reach0: DeedTable {
                blackmailed: 0.2,
                ..DeedTable::of([1.0, 0.5, 0.3, 0.2, 0.5, 0.6, 0.3, 0.4, 0.8, 0.9, 0.4, 1.0, 0.6, 0.1, 0.3, 0.3, 0.2])
            },
            deed_sal: DeedTable {
                blackmailed: 0.5,
                ..DeedTable::of([0.9, 0.6, 0.5, 0.4, 0.6, 0.5, 0.3, 0.5, 0.6, 0.8, 0.4, 0.9, 0.7, 0.3, 0.4, 0.7, 0.3])
            },
            deed_sev: DeedTable {
                blackmailed: 0.4,
                ..DeedTable::of([1.0, 0.5, 0.3, 0.2, 0.6, 0.0, 0.0, 0.3, 0.0, 0.6, 0.0, 0.8, 0.6, 0.0, 0.1, 0.7, 0.0])
            },
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
        //                        kil  ass  rob   ext  str  arr  mar  evi   stk  rai  fou  ave  bet   rep  poa  hir  gav
        let dread_w =
            DeedTable::of([1.0, 0.4, 0.15, 0.3, 0.2, 0.0, 0.0, 0.0, 0.0, 0.6, 0.0, 0.8, 0.0, 0.0, 0.0, 0.3, 0.0]);
        // M16b § 4: `blackmailed` honour_w -0.4.
        let honour_w = DeedTable {
            blackmailed: -0.4,
            ..DeedTable::of([
                0.0, 0.0, -0.3, -0.2, -0.6, 0.0, 0.1, -0.2, 0.0, 0.0, 0.0, 0.4, -1.0, 0.3, -0.1, -0.2, 0.2,
            ])
        };
        let heat_w =
            DeedTable::of([0.5, 0.2, 0.2, 0.1, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.4, 0.0]);
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
    /// Shadow V4 bug 15: the safety a holder loses on hearing of a Hit on
    /// himself.
    pub marked_safety_drop: f32,
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
        GrudgesCfg {
            grudge_min: 0.15,
            marked_safety_drop: 0.4,
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
        HuntCfg {
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
        MovesCfg {
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
    /// Jobs & lives phase 1 (§ 2, the Hq): the admin term's weight in a
    /// corp's competence (`[jobs2] enabled` only) ...
    pub admin_w: f32,
    /// ... and `admin = clamp(Σ office staff skill ÷ admin_ref, 0, 1)`.
    pub admin_ref: f32,
}

impl Default for CompetenceCfg {
    fn default() -> Self {
        CompetenceCfg {
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
            admin_w: 0.1,
            admin_ref: 10.0,
        }
    }
}

/// M15 § 7: Feeds, stories and Spin (phase 4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewsCfg {
    pub reach_base: f32,
    pub reach_per_reporter: f32,
    pub story_reach: f32,
    pub stories_per_day: u8,
    /// Shadow V4 bug 21: an organic deed runs in at most this many Feeds
    /// within `RECENT_DAYS` (0 = any number).
    pub outlets_per_deed: u8,
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
        NewsCfg {
            reach_base: 0.2,
            reach_per_reporter: 0.15,
            story_reach: 0.8,
            stories_per_day: 3,
            outlets_per_deed: 2,
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

/// Life pass L1 (docs/history/SHADOW_V1.md "Plan"): the cross-cutting causes that
/// made every shadowed resident's day a walk. `enabled = false` is the
/// master switch: every L1 branch reads it, so off is the ab79188 city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LifeCfg {
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
    /// Pressure pass F2: a guard at the last-seen tile with the suspect out
    /// of sight chases on (a hop) to a sighting another guard made in the
    /// last this many ticks (the radio), not only to one in his own sight.
    pub chase_radio_ticks: u64,
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
    /// Shadow V4 bug 1: with no dole, a jobless adult keeps Earn (Beg,
    /// Scavenge) below this many meals' coins, not only below one.
    pub earn_meals_no_net: i64,
    /// Shadow V4 bug 2: Rest restores energy only up to here (Sleep is the
    /// full restore).
    pub rest_energy_cap: f32,
    /// Shadow V4 bug 3: Fight's "lawless" consideration, `1 − this ×
    /// lawfulness`, and its "odds" one, `this + the agent's strength share`.
    pub fight_lawful_damp: f32,
    pub fight_odds_floor: f32,
    /// An escort's walk to the Precinct is a van ride of at most this long.
    pub escort_van_ticks: u64,
    /// L2 (L29, `[lod] budget`): after this many dry Scavenge hours in a row
    /// `Earn` cools for `scavenge_cool_hours`.
    pub scavenge_dry_max: u8,
    pub scavenge_cool_hours: u64,
    /// ... and a shift cut short pays pro rata from this share of it.
    pub shift_pro_rata: f32,
    pub starving_hunger: f32,
    /// Shadow V4 bug 19: under this hunger, with a meal in reach, Eat breaks
    /// a shift (a meal break), and Work weighs 0.3 under it.
    pub shift_hunger_break: f32,
    pub sleep_wake_energy: f32,
    /// Workplace room (2026-10-11): a housed agent sleeping rough (out of
    /// doors, its bed past `rough_min_tiles`) sleeps on past
    /// `[needs] sleep_ticks_full` until `sleep_wake_energy` or the walk to
    /// the next shift, not woken at the cap to walk hours home at night
    /// (false: the cap ends it).
    pub rough_sleep_on: bool,
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
    pub lod_dwell_ticks: u64,
    /// Item 10: guard-prisoner affinity in the cells stops at this.
    pub jail_affinity_cap: f32,
    /// Item 11: an employee who has missed this many workdays is dismissed
    /// (the absentee's dole at 2 was removed by roadmap addendum 17).
    pub noshow_fire_days: i64,
    /// Day gaps 3 (2026-10-11): a holder whose wealth is secure (`Needs::
    /// wealth` 1: Work's wealth term loses to Idle) who has missed this many
    /// workdays running quits the post (0: never).
    pub secure_quit_days: i64,
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
    /// The violence fixes (2026-10-09, the wages-on diagnosis): three
    /// missing mechanisms that inflated off-screen killings and chrome
    /// episodes. Master switch (in flight); off is the 644eb10 city byte for
    /// byte. The sub-switches are read only with it on.
    pub violence_fixes: bool,
    /// Item 1: the ledger's `VictimClass::Street` (no Home): what evictees
    /// suffer on screen is learned and applied to the homeless only, not
    /// to every housed civilian of the district.
    pub vf_street_class: bool,
    /// ... its prior: the civilian class's (`[fviolence] class_mult`
    /// civilian) times this, per kind.
    pub vf_street_prior_mult: f32,
    /// Item 2: a riot's or an episode's rates are scaled by the hours it
    /// was live that day ÷ 24 (exposure is learned per 24 body-hours).
    pub vf_short_sources: bool,
    /// Item 3: no chrome purchase whose projected load takes the target
    /// sanity under `[chrome] psycho` (gang members, guards and Fighters
    /// excepted), and courage alone no longer unlocks the five-slot list.
    pub vf_chrome_cap: bool,
    /// Item 4: rent taken from a wage or the dole (`[rent] pay_from_income`)
    /// leaves an agent in arrears a meal's worth (`mean_price`) first.
    pub vf_rent_meal: bool,
    /// Item 5: an immigrant arrives with a seeded adult's savings for its
    /// Home's tier (`demography::spawn_immigrant`), crossed in from the World.
    pub vf_arrival: bool,
    /// Hunger as sickness P1 (docs/HUNGER_SICKNESS.md § 6): the Full
    /// planner's hunger gaps the recalibration diaries found. Master switch
    /// (off is the d771073 city byte for byte); the sub-switches are read
    /// only with it on.
    pub hunger_fixes: bool,
    /// G1: starving with a meal in reach, eat before any paid leisure, a
    /// bed, a dose or a purchase, and before an exhausted or night sleep.
    pub hf_food_first: bool,
    /// G2: Eat's can-obtain gate opens on a stocked Mission in reach.
    pub hf_mission: bool,
    /// G3: a theft of food costs `steal_can_buy` more while a meal is affordable.
    pub hf_buy_not_steal: bool,
    /// G4: a Beg with no giver is a dry hour (as Scavenge's), not a failure.
    pub hf_beg_retry: bool,
    /// P5f: a worker on shift does not leave for leisure (Unwind gated to 0
    /// on shift; it was `gate_or(0.3)`: 5.2 % of Full shift minutes in the
    /// all-Full flipped city went to Unwind).
    pub hf_shift_first: bool,
    /// P5g (Dylan: relocation): an employed worker whose commute
    /// (`life::commute_cost`, travel ticks) is past `commute_move_ticks`
    /// moves to the nearest Home with room within that of work (or trades
    /// Homes), needing only the first rent (no `rehouse_coins_mult`
    /// deposit), when it saves `commute_margin_ticks`; up to
    /// `relocate_per_1000` a day per 1,000 residents.
    pub hf_relocate: bool,
    pub commute_move_ticks: u64,
    pub commute_margin_ticks: u64,
    pub relocate_per_1000: f32,
    /// P5g (Dylan: the hiring cap): a candidate within `commute_hire_ticks`
    /// (`life::commute_cost`) of the workplace is preferred over any beyond
    /// it (skill and the rehire bonus rank within each band); world creation
    /// houses the seeded staff near work where Homes allow (the relocation
    /// pass, uncapped).
    pub hf_hire_near: bool,
    pub commute_hire_ticks: u64,
    /// P5h (Dylan: the city grows toward jobs): an employer with at least
    /// `housing_demand_min` workers past `commute_move_ticks` and fewer free
    /// beds than that within `commute_move_ticks` of its door, for
    /// `housing_demand_days` midnights running, gets a Home on the nearest
    /// vacant Lot in that range (else a floor on the nearest Home there
    /// below `[floors] floors_max`); up to `housing_builds_per_1000` a day
    /// per 1,000 residents. Its owner corp pays `found_cost` / `floor_cost`
    /// when it can (company housing), else the Treasury buys it from the
    /// World when it can (keeping `[treasury] band[0]`), else nothing.
    pub hf_grow_housing: bool,
    /// P5i (coordinator call): meals around the shift. (a) At home before a
    /// shift its hunger would not last (under the Eat line by the shift's
    /// end), a worker eats from the pantry first (when a meal would not
    /// overfill it) and packs a meal to carry; (b) off shift, with the
    /// household's pantry and its carried meals under a shift's worth and
    /// coins for a meal, an idle worker makes a grocery run (buy, carry
    /// home, store) when the round trip ends `DEPARTURE_MARGIN` before the
    /// next shift's leaving time.
    pub hf_shift_meals: bool,
    pub grocery_score: f32,
    /// Item 11 (b), after the d19 diaries (up to 2.4 h a day of grocery
    /// walking, a third of runs ended by Sleep mid-walk): no grocery run
    /// starts that would leave the body under this energy by the time it is
    /// home (the energy decays over the round trip).
    pub grocery_energy_floor: f32,
    /// Item 11 (b): a run must bring home a meal for each this many ticks of
    /// its round trip (here to the shop and home): a 2 h trip for two meals,
    /// 7 h for seven. The d19 fighter walked 3.7 h each way for 1-2 meals; a
    /// poor man far from a shop shops rarely and big (0 turns it off).
    pub grocery_ticks_per_meal: u64,
    /// P5j (coordinator call): work-night sleep. The bedtime before a
    /// workday shift is the leaving time (`routine::must_leave_for_work`'s:
    /// the walk to work and `DEPARTURE_MARGIN` before the start) minus a
    /// full night (`[needs] sleep_ticks_full`) minus the walk home. Sleep
    /// gains `work_night_sleep_flat` x a pressure rising from 0 at
    /// `work_night_ramp_ticks` before bedtime to 1 at bedtime; past it,
    /// Unwind and the grocery run score x `work_night_unwind_mult` (a
    /// weight, not a ban: some stay out).
    pub hf_work_night: bool,
    /// Coordinator call (after the recalibration, 2026-10-10): with
    /// the market wage law, a homeless adult who holds a Job re-houses on the
    /// first rent (no `rehouse_coins_mult` deposit), as relocation does; the
    /// jobless keep the deposit. The flipped city had 82-92 employed adults
    /// on the street (~8 %), 930-1,420 beds empty, the deposit 21 coins on a
    /// ~6 a day wage.
    pub hf_rehouse_employed: bool,
    pub work_night_sleep_flat: f32,
    pub work_night_ramp_ticks: u64,
    pub work_night_unwind_mult: f32,
    pub housing_demand_min: u32,
    pub housing_demand_days: u16,
    pub housing_builds_per_1000: f32,
    /// The housing catch-up (`life::grow_housing`): with this > 0 a ready
    /// employer's unmet demand (far workers less the free beds in range)
    /// asks `ceil(unmet / a Home's beds / housing_catchup_days)` builds a
    /// day, the city's floored at `housing_builds_per_1000` and capped at
    /// `housing_builds_max_per_1000` per 1,000 residents, and a build keeps
    /// the employer's demand count. 0: the flat rule (one build per ready
    /// employer, the count reset by a build).
    pub housing_catchup_days: f32,
    pub housing_builds_max_per_1000: f32,
    /// The pre-roll (`life::preroll_housing`, world creation; with items
    /// 8-10, wages and no safety net on): Homes and floors toward the
    /// seeded staff's jobs, unpaid as the seed buildings, until the far
    /// workers have beds in range or no Lot or floor is left there.
    pub housing_preroll: bool,
    /// G2: a starving agent walks this much further to a Mission.
    pub hungry_reach_mult: f32,
    /// G3: the theft cost added while the agent can buy a meal.
    pub steal_can_buy: f32,
    /// G4: dry Begs in a row that cool Earn ...
    pub beg_dry_max: u8,
    /// ... for this many hours.
    pub beg_cool_hours: u64,
    /// Lives at work phase 7 (§ 5.3), under the `[lives]` master
    /// (`[life] exec_home_tier = 0`; 0 is off and not written): a corp
    /// exec's Home is of this tier or better (2: the Spire) when one has
    /// room, at seed, daily, and on relocation; no one is evicted for it.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub exec_home_tier: u8,
    /// § 5.3: the Spire's Homes are checked at seed against `execs ×
    /// exec_home_spare` beds and the result printed (never an eviction).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub exec_home_spare: f32,
    /// The perf pass phase 10 (ISLAND_PASS I9, a label is not space): an
    /// exec's Home stands within this many tiles (Manhattan, door to door)
    /// of his corp's Hq (`life::hq_of`). `life::house_exec` moves an exec
    /// housed under `exec_home_tier` into the free Home of that tier nearest
    /// the Hq (not the emptiest), and one housed at the tier but further
    /// than this into a nearer one within it when one has room. 0 is off (the
    /// base pick) and not written.
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub exec_home_reach: u16,
}

impl Default for LifeCfg {
    fn default() -> Self {
        LifeCfg {
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
            chase_radio_ticks: 30,
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
            earn_meals_no_net: 3,
            rest_energy_cap: 0.6,
            fight_lawful_damp: 0.6,
            fight_odds_floor: 0.5,
            escort_van_ticks: 60,
            scavenge_dry_max: 3,
            scavenge_cool_hours: 4,
            shift_pro_rata: 0.5,
            starving_hunger: 0.1,
            shift_hunger_break: 0.12,
            sleep_wake_energy: 0.99,
            rough_sleep_on: false,
            idle_sleep_energy: 0.9,
            energy_brake: 0.5,
            spot_walk_cap_tiles: 22,
            spot_lone_fun: 0.4,
            spot_enemy_penalty: 0.5,
            avoid_spot_ticks: 1440,
            flee_clear_ticks: 30,
            lod_dwell_ticks: 180,
            jail_affinity_cap: 0.3,
            noshow_fire_days: 7,
            secure_quit_days: 2,
            muster_walk_cap_ticks: 480,
            deal_busy_from: 720,
            deal_busy_to: 120,
            told_cooldown_days: 3,
            hack_gap_wealth: 0.9,
            violence_fixes: false,
            vf_street_class: true,
            vf_street_prior_mult: 3.0,
            vf_short_sources: true,
            vf_chrome_cap: true,
            vf_rent_meal: true,
            vf_arrival: true,
            hunger_fixes: false,
            hf_food_first: true,
            hf_mission: true,
            hf_buy_not_steal: true,
            hf_beg_retry: true,
            hf_shift_first: true,
            hf_relocate: true,
            commute_move_ticks: 80,
            commute_margin_ticks: 40,
            relocate_per_1000: 20.0,
            hf_hire_near: true,
            commute_hire_ticks: 80,
            hf_grow_housing: true,
            hf_shift_meals: true,
            grocery_score: 0.5,
            grocery_energy_floor: 0.4,
            grocery_ticks_per_meal: 60,
            hf_work_night: true,
            hf_rehouse_employed: true,
            work_night_sleep_flat: 0.5,
            work_night_ramp_ticks: 120,
            work_night_unwind_mult: 0.3,
            housing_demand_min: 6,
            housing_demand_days: 3,
            housing_builds_per_1000: 1.0,
            housing_catchup_days: 5.0,
            housing_builds_max_per_1000: 6.0,
            housing_preroll: true,
            hungry_reach_mult: 2.0,
            steal_can_buy: 12.0,
            beg_dry_max: 3,
            beg_cool_hours: 1,
            exec_home_tier: 0,
            exec_home_spare: 0.0,
            exec_home_reach: 0,
        }
    }
}

/// L2 (plan L5): the master switch of the living city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivingCfg {
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
        LivingCfg {
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
        LivingJobsCfg {
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
    /// Day gaps 1 (2026-10-11): a job holder's off hours end this many
    /// ticks before leaving for the next workday shift (`leisure::off_hours`:
    /// the agent's own shift, not the wall clock's Evening and Night).
    pub shift_lead_ticks: u64,
    /// Day gaps 2 (2026-10-11): a paid venue farther than this walk
    /// (`exec::timed_walk`, the commute's estimate: a vehicle reaches
    /// farther) is not on a Full agent's Unwind ladder (fabbers walked 171
    /// min to a Den after a 9 h shift).
    pub venue_walk_cap_ticks: u64,
}

impl Default for LeisureCfg {
    fn default() -> Self {
        LeisureCfg {
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
            shift_lead_ticks: 120,
            venue_walk_cap_ticks: 60,
        }
    }
}

/// L2 § 1 `[budget]`: the Treasury's band and public works.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetCfg {
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
        BudgetCfg {
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

/// L2 § 1 `[export]`: what is left of L2's export hook under the World
/// market (2026-10-09: the flat-price hook and its switch retired).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportCfg {
    /// The World account's opening treasury when a donation creates it first.
    pub treasury_ref: i64,
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
        ExportCfg { treasury_ref: 20_000, parts_floor: 20, data_floor: 100 }
    }
}

/// L2 § 3 `[fviolence]` (phase 4): the ledger's victim side and the daily
/// pass that applies it to the Statistical tier. Every number a placeholder
/// (phase 5 calibrates; `fv_mult` is the first knob).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FviolenceCfg {
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
    /// The island pass phase 10 `[fviolence.hands]` (OPEN-185): a source
    /// strikes off screen only with its off-screen hands (`fviolence::daily`).
    /// `None` (not written into a save) with the key absent:
    /// the victim-side rolls limited only by `day_cap`, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hands: Option<FvHandsCfg>,
}

/// `[fviolence.hands]`: the actor side's prior, per member-day of a
/// faction's members under a source (an order or a vendetta), Killed,
/// Assaulted, Robbed, Abducted, worth `prior_days` member-days of evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvHandsCfg {
    pub prior: [f32; 4],
    pub prior_days: f32,
}

impl Default for FvHandsCfg {
    fn default() -> Self {
        FvHandsCfg { prior: [0.002, 0.02, 0.005, 0.001], prior_days: 30.0 }
    }
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
        FviolenceCfg {
            rate_days: 14,
            prior_weight: 48.0,
            fv_mult: 1.0,
            day_cap: FvKindsCfg::default(),
            prior: FvPriorCfg::default(),
            class_mult: FvClassCfg::default(),
            hands: None,
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

    /// Lives at work phase 3: the civic switch off, its `[service]` keys at
    /// their defaults, the civic `[[serves]]` rows gone and the trade rows'
    /// `round_near`/`tied_housing` cleared (the off switches; nothing reads
    /// them off).
    pub fn strip_civic(&mut self) {
        self.service.strip_civic();
        self.serves.retain(|r| !r.civic);
        for t in &mut self.trades {
            t.round_near = false;
            t.tied_housing = false;
        }
    }

    /// Lives at work phase 4: the street switch off, its `[service]` keys at
    /// their defaults, the street `[[serves]]` rows gone and `[stims]
    /// deal_corner` off (the off switches; nothing reads them off).
    pub fn strip_street(&mut self) {
        self.service.strip_street();
        self.serves.retain(|r| !r.street);
        self.stims.deal_corner = false;
    }

    /// Lives at work phase 5: the tower switch off, its `[service]` keys at
    /// their defaults, the tower `[[serves]]` rows gone and the `decided`
    /// deed keys at 0 (the off switches; nothing reads them off).
    pub fn strip_tower(&mut self) {
        self.service.strip_tower();
        self.serves.retain(|r| !r.tower);
        self.gossip.reach0.decided = 0.0;
        self.gossip.deed_sal.decided = 0.0;
        self.gossip.deed_sev.decided = 0.0;
    }

    /// Lives at work phase 6 (§ 4.1-4.4): the careers switch off, `[careers]`
    /// at its defaults, the trade rows' bands 0 and `[roles]` empty (none
    /// written into a save).
    pub fn strip_careers(&mut self) {
        self.careers = CareersCfg::default();
        self.roles.clear();
        for t in &mut self.trades {
            t.min_age = 0;
            t.max_age = 0;
        }
    }

    /// Lives at work phase 7 (§ 5.2): the stations off, `[law]`'s station
    /// keys 0 and `[buildings.station]` absent (none written into a save).
    pub fn strip_stations(&mut self) {
        self.law.strip_stations();
        self.buildings.station = None;
    }

    /// Lives at work phase 2: the phase's deed keys at 0 in every deed table
    /// (the off switches; the deeds are never posted off).
    pub fn without_household_deeds(&mut self) {
        self.gossip.reach0.without_household_deeds();
        self.gossip.deed_sal.without_household_deeds();
        self.gossip.deed_sev.without_household_deeds();
    }

    pub fn load_from(assets_dir: &Path) -> Config {
        let path = assets_dir.join("config.toml");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let mut cfg: Config = toml::from_str(&text).unwrap_or_else(|e| panic!("bad config {}: {e}", path.display()));
        cfg.assets_dir = assets_dir.to_path_buf();
        // The pre-view pass's env switches retired at the island pass
        // phase 0 (OPEN-174), as the earlier milestones' did (OPEN-094);
        // every section's config path stays (`[shift]`, `[presence]`,
        // `[legs]`, `[leaders]`, `[hitmarket] enabled` and the rest). With
        // `[shift]` gone, its haul and counter shapes go with it. The island
        // pass's fifteen (`CITYSIM_DISTANCE_OFF` .. `CITYSIM_ROUTES_OFF`)
        // retired at the perf pass phase 0 (OPEN-202); their sections'
        // config paths stay (`[distance]`, `[company_housing]`, `[motion]`,
        // `[routes]` and the rest; absent, the base behaviour).
        if cfg.shift.is_none() {
            cfg.drop_haul_counter();
        }
        // The perf pass's four in-flight switches (`CITYSIM_REST_OFF`, `_DWELL_OFF`, `_EXEC_HOME_OFF`,
        // `_STAGGER_OFF`) retired at its close; the config paths stay. The last midnight slot stays inside
        // the night's first hour.
        assert!(
            u64::from(cfg.perf.stagger_midnight) * u64::from(crate::systems::midnight::LAST_SLOT)
                < crate::time::TICKS_PER_HOUR,
            "[perf] stagger_midnight {} puts the last midnight slot past 00:59",
            cfg.perf.stagger_midnight
        );
        // Lives at work (D13): a config with `[lives] enabled = false` by any
        // path clears the milestone's sections, so the off save holds no
        // `[lives]`, `[service]` or `[[serves]]`. The milestone's env
        // switches retired at its close (OPEN-094); the config paths stay.
        if !cfg.lives.enabled {
            cfg.service = ServiceCfg::default();
            cfg.serves.clear();
            cfg.household = HouseholdCfg::default();
            cfg.without_household_deeds();
            cfg.strip_civic();
            cfg.strip_street();
            cfg.strip_tower();
            cfg.strip_careers();
            cfg.gangs.strip_lieutenants();
            cfg.strip_stations();
            cfg.life.exec_home_tier = 0;
        }
        // A sub-switch off by any path drops its rows and keys, so the off
        // save writes none of them.
        if !cfg.service.civic {
            cfg.strip_civic();
        }
        if !cfg.service.street {
            cfg.strip_street();
        }
        if !cfg.service.tower {
            cfg.strip_tower();
        }
        if !cfg.careers.enabled {
            cfg.strip_careers();
        }
        if !cfg.gangs.lieutenants {
            cfg.gangs.strip_lieutenants();
        }
        if !cfg.law.stations {
            cfg.strip_stations();
        }
        if cfg.life.exec_home_tier == 0 {
            cfg.life.exec_home_spare = 0.0;
            cfg.life.exec_home_reach = 0;
        }
        if !cfg.household.enabled {
            cfg.household = HouseholdCfg::default();
            cfg.without_household_deeds();
        }
        cfg.districts.row_count();
        cfg
    }

    /// A role's config wage: a bespoke role's `[economy] wage_*`, a trade's
    /// row `wage` (J9).
    pub fn wage(&self, role: Role) -> i64 {
        match role {
            Role::Trade(t) => self.trades.get(t.index()).map_or(0, |r| r.wage),
            _ => self.economy.wage(role),
        }
    }

    /// J9: a trade's `[[trades]]` row.
    pub fn trade(&self, t: crate::components::TradeId) -> Option<&TradeCfg> {
        self.trades.get(t.index())
    }

    /// Jobs & lives phase 1 (D4): the niche a kind sits in: its
    /// `[buildings.<kind>] niche`, else the built-in M11-L2 table (Food:
    /// Farm, Market, Bar, NoodleBar; Housing: Home; Security: the Security
    /// Office; Tech: Clinic, Garage, Fab). The new kinds sit in none.
    pub fn kind_niche(&self, kind: BuildingKind) -> Option<crate::components::Niche> {
        use crate::components::Niche;
        if let Some(n) = self.buildings.for_kind(kind).niche {
            return Some(n);
        }
        match kind {
            BuildingKind::Farm | BuildingKind::Market | BuildingKind::Bar | BuildingKind::NoodleBar => {
                Some(Niche::Food)
            }
            BuildingKind::Home => Some(Niche::Housing),
            BuildingKind::SecurityOffice => Some(Niche::Security),
            BuildingKind::Clinic | BuildingKind::Garage | BuildingKind::Fab => Some(Niche::Tech),
            _ => None,
        }
    }

    /// Jobs & lives phase 1 (D4): a kind's founding rank (1 first; 0 an
    /// agent never founds it): its `[buildings.<kind>] foundable`, else the
    /// built-in order of M11's `FOUNDABLE` list (tie -> Bar, then Home, the
    /// Hotel, ...).
    pub fn kind_foundable(&self, kind: BuildingKind) -> u8 {
        if let Some(r) = self.buildings.for_kind(kind).foundable {
            return r;
        }
        match kind {
            BuildingKind::Bar => 1,
            BuildingKind::Home => 2,
            BuildingKind::Hotel => 3,
            BuildingKind::Clinic => 4,
            BuildingKind::Garage => 5,
            BuildingKind::Feed => 6,
            BuildingKind::Club => 7,
            BuildingKind::Arcade => 8,
            BuildingKind::NoodleBar => 9,
            BuildingKind::FightPit => 10,
            BuildingKind::Den => 11,
            BuildingKind::Lounge => 12,
            BuildingKind::Fixer => 13,
            BuildingKind::Mission => 14,
            _ => 0,
        }
    }

    /// Jobs & lives phase 1 (D4): the most floors `kind` may have: its
    /// `[buildings.<kind>] floors_max`, else `[floors] floors_max`.
    pub fn floors_max(&self, kind: BuildingKind) -> u8 {
        self.buildings.for_kind(kind).floors_max.map_or_else(|| self.floors.max_for(kind), |v| v.max(1))
    }

    /// J9: the configured trades as roles, in row order.
    pub fn trade_roles(&self) -> Vec<Role> {
        self.trade_cache_data().roles.clone()
    }

    /// J10: a kind's trades with their places per floor, in row order
    /// (cached).
    pub fn trades_at(&self, kind: BuildingKind) -> &[(Role, usize)] {
        self.trade_cache_data().by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// Drop the trade cache (after editing `trades` in place).
    pub fn reset_trade_cache(&mut self) {
        self.trade_cache = TradeCache::default();
    }

    fn trade_cache_data(&self) -> &TradeCacheData {
        let data = self.trade_cache.0.get_or_init(|| {
            let roles: Vec<Role> = trade_ids_of(&self.trades).into_iter().map(Role::Trade).collect();
            let mut by_kind: std::collections::BTreeMap<BuildingKind, Vec<(Role, usize)>> = Default::default();
            for (role, row) in roles.iter().zip(&self.trades) {
                for &k in &row.workplace {
                    by_kind.entry(k).or_default().push((*role, usize::from(row.staff_per_floor)));
                }
            }
            TradeCacheData { roles, by_kind }
        });
        debug_assert_eq!(data.roles.len(), self.trades.len(), "trades edited after the cache was read");
        data
    }

    /// J9: the trade keyed `key`.
    pub fn trade_role(&self, key: &str) -> Option<Role> {
        let i = self.trades.iter().position(|r| r.id == key)?;
        Some(Role::Trade(crate::components::TradeId::new(i as u8, &self.trades[i].id, &self.trades[i].label)))
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
        // Scale S4: the v1 map is its own reference (nothing is fitted to it).
        self.scale = ScaleCfg::off();
        // Transit: the v1 map has no lines (a unit test builds its own).
        self.transit = TransitCfg::default();
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
        // L1b's food price is the 2,000 city's (the v1 economy prices at 3).
        // Since 2026-10-09 the life pass, the plane, the word, the living
        // city, contracts and the World market run here too: their off
        // switches are retired. P10 (docs/P10_POLICY.md): the v1 city starts
        // under the dole city's policy (the levy and the safety net) but for
        // its school meals; hunger as sickness is always on (the street,
        // and with it the shanties, is cut above); `[life] violence_fixes`
        // follows the shipped default.
        self.economy.price_base = 3.0;
        self.policy = PolicyCfg { school_meals: false, ..PolicyCfg::dole_city() };
        self
    }

    /// The flip (Dylan, 2026-10-10): the shipped city runs wages from
    /// revenue with no safety net. P10 (docs/P10_POLICY.md): this is the old
    /// default as a policy preset, the dole city, which most unit-test files
    /// pin: the city starts under the levy (the wage schedule and the corp
    /// upkeep levy) with the whole safety net enacted (`PolicyCfg::dole_city`).
    /// Hunger as sickness and the shanties run here too (their switches
    /// retired with P10). The housing catch-up (`[life]
    /// housing_catchup_days`, 2026-10-11) is off: its housing growth is the
    /// flat P5h rule (the pre-roll reads the market wage law and no net).
    /// No `[company_housing]` (the island pass phase 3a) and no
    /// `[commute_pressure]` (phase 4).
    pub fn dole_city(mut self) -> Config {
        self.policy = PolicyCfg::dole_city();
        // Transit phase 3: the old default had no El.
        self.transit.enabled = false;
        self.life.housing_catchup_days = 0.0;
        // The island pass phase 3a: no company housing (the old default;
        // with no El its walked commutes would seed blocks on the Lots the
        // pinned tests read).
        self.company_housing = None;
        // The island pass phase 4: no commute pressure (the old default's
        // relocation, which the pinned tests read).
        self.commute_pressure = None;
        self
    }

    /// The Full city size the shipped planner budgets serve
    /// (`calibration_city` scales them past it).
    pub const PLAN_BUDGET_REF_AGENTS: u32 = 500;

    /// The city `calibrate` measures and the LOD parity test replays: the
    /// shipped city at `n` residents (`scaled_to`; both run the shipped
    /// population since the recalibration, see below) with only the cuts
    /// whose effect the Statistical tier models elsewhere. Gangless (the
    /// table never modelled gang actions, D26; off screen a gang's violence
    /// is `fviolence`'s ledger, L2 phase 4) and a full Warehouse (M10 phase
    /// 5b). Full farmers work about half the shift hours a Statistical
    /// farmer is credited with, so an all-Full city runs a food deficit the
    /// real, mostly Statistical city never sees: at 500 residents it emptied
    /// its Warehouse by day 24, the price reached 16, and the table learned
    /// famine theft as everyday theft.
    ///
    /// The recalibration (2026-10-09, `recal`; ported with hunger as
    /// sickness P5 onto the fixed planner): the table's rows condition on
    /// phase, lawfulness and hunger only, so every off-screen agent rolls the
    /// calibration city's average hour. That city was corp-less, rent-free,
    /// with equal wallets, no derelicts and 500 residents on the 2,000 map:
    /// on the flipped config about half its adults stayed jobless and broke
    /// while the shipped city is ~93 % employed (p_killed 5.7x, p_steal 3x),
    /// and at 500 residents a quarter of the police patrolled the same
    /// streets. The corps (their jobs, private guards, Markets and Blocks),
    /// rent, the tiered opening wallets, the class coupling and the seeded
    /// derelicts are the shipped city's now; three pooled seeds average one
    /// seed's landlords out.
    pub fn calibration_city(self, n: u32) -> Config {
        let mut c = self.scaled_to(n);
        c.gangs.max_members = 0;
        c.world.warehouse_initial = c.buildings.warehouse.stock_cap;
        // M12 D48 (phase 4): riots off, no splits, no crossfire (off screen
        // a riot is `fviolence`'s source).
        c.riots = RiotsCfg::off();
        c.gangs.split_base = 0.0;
        // M13 D46/D50: no assets. `lod::stat_probs` layers a kitted agent's
        // Kit and a stim withdrawal over the bare row, so the row must not
        // already hold them.
        c.assets = AssetsCfg::off();
        c.vehicles = VehiclesCfg::off();
        c.shop = ShopCfg::off();
        c.stims = StimsCfg::off();
        c.robots = RobotsCfg::off();
        // 2026-10-09 (the off switches retired): the life pass, the Virt
        // plane, the word, the living city, contracts and the World market
        // run here as in the shipped city. The starting policy (`[policy]`,
        // P10; the wage law and the safety net) follows the loaded config
        // too, as `[life] violence_fixes` does (the flip-readiness round: the
        // table and the parity test model the off-screen hours of the city
        // the config ships). `v1_profile` starts under the dole city's.
        // Hunger as sickness P5h (a harness change): the planner budgets were
        // tuned for a 500-agent Full city; the all-Full calibration and
        // parity cities scale them by n / 500 (never below the shipped
        // values). At 2,000 on budget 12 the queue held ~480 and 12% of
        // shift time was a Work goal waiting on a plan (1.1% at 4x).
        let scale = (f64::from(n) / f64::from(Self::PLAN_BUDGET_REF_AGENTS)).max(1.0);
        c.brain.plan_budget_per_tick = (c.brain.plan_budget_per_tick as f64 * scale).ceil() as usize;
        c.brain.plan_expansion_budget_per_tick =
            (c.brain.plan_expansion_budget_per_tick as f64 * scale).ceil() as usize;
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
        self.levers.guard_count = (f64::from(self.levers.guard_count) * f).round().clamp(0.0, 65535.0) as u16;
        self.levers.immigration_per_week =
            (f64::from(self.levers.immigration_per_week) * f).round().clamp(0.0, 65535.0) as u16;
        self.levers.sanitation_count = (f64::from(self.levers.sanitation_count) * f).round().clamp(0.0, 65535.0) as u16;
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
        // Flip readiness: the no-net Treasury's coin keys (read only with
        // the market wage law, P10) scale too. Unscaled, a
        // 300-resident city's Treasury sat under the 2,000 city's band, the
        // civic budget's target read 0 and the first midnight dismissed the
        // police (the district beats emptied a day later).
        let t = &mut self.treasury;
        t.treasury_initial = (t.treasury_initial as f64 * f).round() as i64;
        t.till_initial = (t.till_initial as f64 * f).round() as i64;
        for b in &mut t.band {
            *b = (*b as f64 * f).round() as i64;
        }
        t.sanitation_max = (f64::from(t.sanitation_max) * f).round().clamp(0.0, 65535.0) as u16;
        // Scale S4 (docs/SCALE_20K.md § S4): the city-wide counts, caps and
        // budgets that assumed 2,000 residents scale per capita, up only (a
        // city under 2,000 keeps the 2,000 city's value, as before S4: the
        // small test cities were calibrated on it).
        let up = f.max(1.0);
        let up_usize = |v: usize| (v as f64 * up).round() as usize;
        let up_u32 = |v: u32| (f64::from(v) * up).round() as u32;
        let up_i64 = |v: i64| (v as f64 * up).round() as i64;
        // The bodied tiers per capita (and the private guards' quota inside
        // Coarse): the forced classes, a gang's quota and the private guards,
        // grow with the city, a gang recruits only through its bodies, and
        // the Statistical tier is calibrated against a Full share of the
        // city. Measured on the generated 20k map (30 days; the 2,000 city
        // on day 30: gang members 46 and arrests 373 per 1,000, employed
        // 51-53 %): both at 50 / 150, gang members 0.8 and arrests 46 per
        // 1,000 (the gangs die without bodies) at 435-784 ticks/s; Coarse per
        // capita 28-30 and 160-220, employed 45-47 % at 117-235 ticks/s; with
        // Full per capita too (seeds 42-43, the S4 city) employed 48.0-48.3 %
        // vs 46.9-47.6 % at 114-125 ticks/s vs 125-128. A body's goal scoring
        // reads city-wide lists (Unwind's venues, Squat's derelicts), so the
        // bodies, not the residents, cost the run.
        let bodies = (self.lod.max_full + self.lod.max_coarse).max(1) as f64;
        self.lod.max_full = up_usize(self.lod.max_full);
        self.lod.max_coarse = up_usize(self.lod.max_coarse);
        self.lod.private_quota = up_usize(self.lod.private_quota);
        // The planner's and the paths' per-tick budgets serve the bodies.
        let k = (self.lod.max_full + self.lod.max_coarse) as f64 / bodies;
        let b = &mut self.brain;
        b.plan_budget_per_tick = (b.plan_budget_per_tick as f64 * k).ceil() as usize;
        b.plan_expansion_budget_per_tick = (b.plan_expansion_budget_per_tick as f64 * k).ceil() as usize;
        b.path_budget_per_tick = (b.path_budget_per_tick as f64 * k).ceil() as usize;
        // Migration (a week's arrivals and the beds that read as room).
        let d = &mut self.demography;
        d.migrants_max = up_u32(d.migrants_max);
        d.beds_ref *= up as f32;
        d.offers_max = up_u32(d.offers_max);
        // City-wide caps on live records and daily events.
        self.hunt.max_hunts = up_usize(self.hunt.max_hunts);
        self.contracts.max_open = up_usize(self.contracts.max_open);
        self.missions.max_missions = up_usize(self.missions.max_missions);
        let h = &mut self.hitmarket;
        h.max_open = up_usize(h.max_open);
        h.posts_per_day = up_usize(h.posts_per_day);
        h.corp_year_cap = up_u32(h.corp_year_cap);
        // M16b: the standing coercions' cap; phase 2: the captives'.
        self.leverage.max_coercions = up_usize(self.leverage.max_coercions);
        self.hostage.captive_quota = up_usize(self.hostage.captive_quota);
        // Phase 3: the downed.
        self.wounded.wounded_quota = up_usize(self.wounded.wounded_quota);
        // Phase 4: the tags city-wide.
        self.tags.max_tags = up_usize(self.tags.max_tags);
        let fv = &mut self.fviolence.day_cap;
        *fv = FvKindsCfg {
            killed: up_u32(fv.killed),
            assaulted: up_u32(fv.assaulted),
            robbed: up_u32(fv.robbed),
            abducted: up_u32(fv.abducted),
        };
        // The island pass phase 10 (OPEN-185): `vendetta_norm` is the
        // grudge weight one faction's members hold on another's that reads
        // as a full feud; a faction's members (and so the grudges between
        // two factions) grow with the city, so the norm is per 2,000
        // residents (with the phase's `[fviolence.hands]`; absolute before).
        if self.fviolence.hands.is_some() {
            self.grudges.vendetta_norm *= up as f32;
        }
        self.life.relocate_per_day = up_usize(self.life.relocate_per_day);
        // The outside's demand: the World buys per resident (money enters at
        // the 2,000 city's per-capita rate) and its account refills to match.
        // M17 phase 1: the Ambient book's caps are per resident (the
        // simulated businesses are sized for the simulated city).
        let oa = &mut self.outside.ambient;
        oa.food_cap = up_u32(oa.food_cap);
        oa.visit_cap = up_u32(oa.visit_cap);
        // M17 phase 3: the bodies per capita (a parent's Outsiders, the
        // State's guards; the brand's own branches scale its money).
        let op = &mut self.outside.parents;
        op.outsiders_cap = up_u32(op.outsiders_cap);
        op.state_guards_max = (f64::from(op.state_guards_max) * up).round().clamp(0.0, 65535.0) as u16;
        op.state_guards_step = (f64::from(op.state_guards_step) * up).round().clamp(1.0, 65535.0) as u16;
        op.heat_murders *= up as f32;
        // M17 phase 5: the promoted cast living at most, per capita.
        self.outside.abroad.promoted_max = up_u32(self.outside.abroad.promoted_max);
        let wm = &mut self.world_market;
        wm.treasury_ref = up_i64(wm.treasury_ref);
        wm.food.cap = up_u32(wm.food.cap);
        wm.parts.cap = up_u32(wm.parts.cap);
        wm.data.cap = up_u32(wm.data.cap);
        // Jobs & lives phase 1 (R2): the appended books per capita, as Data's.
        wm.rations.cap = up_u32(wm.rations.cap);
        wm.braindance.cap = up_u32(wm.braindance.cap);
        wm.organs.cap = up_u32(wm.organs.cap);
        // The civic three's caps per capita (police and Sanitation's own above).
        let j2 = &mut self.jobs2;
        j2.clerks_max = (f64::from(j2.clerks_max) * up).round().clamp(0.0, 65535.0) as u16;
        j2.depot_hands_max = (f64::from(j2.depot_hands_max) * up).round().clamp(0.0, 65535.0) as u16;
        // Phase 2: the street's caps per capita (pitches a district, sweeps a day).
        let per_d = |v: u16| (f64::from(v) * up).round().clamp(0.0, 65535.0) as u16;
        j2.pitches_per_district = per_d(j2.pitches_per_district);
        j2.sweeps_per_day = per_d(j2.sweeps_per_day);
        for c in &mut wm.cap_wages {
            *c = up_u32(*c);
        }
        self.export.treasury_ref = up_i64(self.export.treasury_ref);
        // The Treasury's other coin keys (the L2 band, the public works, the
        // derelict floor), as `[treasury]` above.
        for b in &mut self.budget.band {
            *b = up_i64(*b);
        }
        self.budget.works_max = (f64::from(self.budget.works_max) * up).round().clamp(0.0, 65535.0) as u16;
        self.street.city_absorb_floor = up_i64(self.street.city_absorb_floor);
        // The opening businesses dealt per resident (the Hotels, the venues
        // and Fabs, the Missions); the name-keyed rows deal `World::seed_copies()`.
        self.street.seed_hotels = up_usize(self.street.seed_hotels);
        self.street.seed_derelict_blocks = up_usize(self.street.seed_derelict_blocks);
        let v = &mut self.jobs.seed_venues;
        *v = SeedVenuesCfg {
            club: up_u32(v.club),
            arcade: up_u32(v.arcade),
            noodle_bar: up_u32(v.noodle_bar),
            fight_pit: up_u32(v.fight_pit),
            den: up_u32(v.den),
            lounge: up_u32(v.lounge),
            fab: up_u32(v.fab),
        };
        self.charity.seed = up_u32(self.charity.seed);
        // The civic headcounts' daily cut (the reconcilers' daily moves are
        // `per_capita_count`).
        self.treasury.civic_pace = (f64::from(self.treasury.civic_pace) * up).round().clamp(0.0, 255.0) as u8;
        self
    }

    /// Scale S4: residents over the reference city's (`[scale] population`).
    pub fn per_capita(&self) -> f64 {
        f64::from(self.world.population) / f64::from(self.scale.population.max(1))
    }

    /// Scale S4: `round(per_capita)`, at least 1: the population's count of
    /// reference cities. The role-seeding passes read the roster's copies
    /// instead (`World::seed_copies`, review finding 7).
    pub fn copies(&self) -> usize {
        (self.per_capita().round() as usize).max(1)
    }

    /// Scale S4: a city-wide count in code that assumed 2,000 residents (the
    /// guard and Sanitation reconcilers' five hires or dismissals a day, the
    /// law's report rings), per capita, up only.
    pub fn per_capita_count(&self, at_2000: usize) -> usize {
        (at_2000 as f64 * self.per_capita().max(1.0)).round() as usize
    }

    /// Scale S4: a `PlayerCommand` headcount's ceiling (`SetGuardCount`,
    /// `SetSanitation`, `SetImmigrationPerWeek`), per capita, up only.
    pub fn lever_ceiling(&self, at_2000: u16) -> u16 {
        (f64::from(at_2000) * self.per_capita().max(1.0)).round().clamp(0.0, 65535.0) as u16
    }

    /// Scale S4 (`World::new`, after the map's districts and lines are in):
    /// the keys held per building, per gang, per corp or per district,
    /// fitted to the map's counts against `[scale]`'s (every ratio is 1 on
    /// the shipped map, and ratios under 1 read 1). `scaled_to` scaled the
    /// city totals by population; here they are spread over the map's
    /// holders:
    /// - per Market: the opening shelf, the restock floor and batch and
    ///   `price_ref_stock` (the double scaling: a 20k city's ten times the
    ///   Markets each held ten times the shelf);
    /// - per gang (one per Hideout): `max_members`, and `max_gangs` grows
    ///   with the Hideouts;
    /// - the corps: the roster dealt once per reference city's employers
    ///   (Farms, Markets and Security Offices) and re-dealt so every one is
    ///   owned in the shipped proportion ([`CorpsCfg::replicate`]); the
    ///   agent Bar owners and their purse per Bar;
    /// - the one-per-city buildings (the Jail, the Hall, the Recycler, the
    ///   Reserve Depot): seats and stock per capita;
    /// - per district: the riot crowd and its bodies, the gang-landlord and
    ///   sweep thresholds by residents per district; the concurrent riots and
    ///   crackdowns by districts;
    /// - per Lot: the shanty ceiling.
    ///
    /// Once per config (`[scale] fitted`): a fitted config fits to itself.
    pub fn fit_to_map(&mut self, map: &crate::map::Map) {
        if !self.scale.enabled || self.scale.fitted {
            return;
        }
        self.scale.fitted = true;
        let s = self.scale.clone();
        let mut count: std::collections::BTreeMap<BuildingKind, u32> = Default::default();
        for b in &map.buildings {
            *count.entry(b.kind).or_insert(0) += 1;
        }
        let n = |k: BuildingKind| count.get(&k).copied().unwrap_or(0);
        let ratio = |have: u32, reference: u32| (f64::from(have) / f64::from(reference.max(1))).max(1.0);
        let r_market = ratio(n(BuildingKind::Market), s.markets);
        let r_bar = ratio(n(BuildingKind::Bar), s.bars);
        let r_hideout = ratio(n(BuildingKind::Hideout), s.hideouts);
        let r_lot = ratio(n(BuildingKind::Lot), s.lots);
        let districts = self.districts.names.len().max(1) as u32;
        let r_districts = ratio(districts, s.districts);
        let up = self.per_capita().max(1.0);
        // Residents per district against the reference city's.
        let r_dsize = (up / (f64::from(districts) / f64::from(s.districts.max(1)))).max(1.0);
        // Per Market.
        let w = &mut self.world;
        w.market_initial = (f64::from(w.market_initial) / r_market).round() as u32;
        let e = &mut self.economy;
        e.restock_floor = (f64::from(e.restock_floor) / r_market).round() as u32;
        e.restock_batch = (f64::from(e.restock_batch) / r_market).round() as u32;
        e.price_ref_stock /= r_market;
        // Per gang.
        let g = &mut self.gangs;
        g.max_members = (g.max_members as f64 / r_hideout).round() as usize;
        g.max_gangs = (g.max_gangs as f64 * r_hideout).round() as usize;
        // The corps: the roster once per reference city's employers.
        let employers = |a: u32, b: u32, c: u32| f64::from(a + b + c);
        let r_emp = employers(n(BuildingKind::Farm), n(BuildingKind::Market), n(BuildingKind::SecurityOffice))
            / employers(s.farms, s.markets, s.offices).max(1.0);
        // More corps, not bigger ones (measured on the generated 20k map,
        // seeds 42-44, 60 days, day 60: the roster dealt ten times read
        // employed 47.7-49.4 %, homeless 246-260 per 1,000; the nine rows
        // owning ten times the buildings, with the per-corp coin keys
        // (`hoard_heat`, `lobby_min_treasury`, the estate caps, the outside
        // treasuries, `corp_raid_cap`) ten times over, 45.1-46.9 % and
        // 278-284). Copies keep every per-corp key as tuned, an exec per
        // ~2,000 residents and a bankruptcy's share of the economy.
        let k = (r_emp.round() as usize).max(1);
        // Pressure pass F2: the opening businesses that stand on Lots (the
        // venues, Labs, Clinics, Garages, Fixers, Feeds, the industries, the
        // Hq, the vice and fashion houses, the School, the Missions and
        // Hotels) are dealt per Lot, not per employer: the reference city
        // seeds ~40 % of its Lots and leaves the rest to grow on. The 100k
        // island holds 600 Farms (k = 50) on 1,415 Lots (8.8 x 160); the
        // passes dealt 50 copies each in a fixed order, the Lots ran out at
        // the Noodle Bars, and every kind seeded after (the Hq, the School,
        // the Brothel and StripClub, the Dollhouse, the FightPit, the
        // Stations) stood nowhere: Office Worker, Teacher, Madam, Contract
        // Host and Fighter were never hired (Shadow V9). At 2k and 20k the
        // two ratios agree and nothing moves.
        let k_lot = (r_lot.round() as usize).clamp(1, k);
        if k_lot < k {
            self.scale.lot_copies = k_lot as u32;
            let f = k_lot as f64 / up;
            let down = |v: u32| if v == 0 { 0 } else { ((f64::from(v) * f).round() as u32).max(1) };
            let v = &mut self.jobs.seed_venues;
            *v = SeedVenuesCfg {
                club: down(v.club),
                arcade: down(v.arcade),
                noodle_bar: down(v.noodle_bar),
                fight_pit: down(v.fight_pit),
                den: down(v.den),
                lounge: down(v.lounge),
                fab: down(v.fab),
            };
            self.street.seed_hotels = down(self.street.seed_hotels as u32) as usize;
            self.charity.seed = down(self.charity.seed);
        }
        let c = &mut self.corps;
        if c.row_count() > 0 {
            let total = |v: &Vec<u32>, r: f64| (f64::from(v.iter().sum::<u32>()) * r).round() as u32;
            let totals = [
                total(&c.farms, ratio(n(BuildingKind::Farm), s.farms)),
                total(&c.markets, r_market),
                total(&c.offices, ratio(n(BuildingKind::SecurityOffice), s.offices)),
                c.blocks.iter().sum(),
            ];
            let same = [c.farms.iter().sum(), c.markets.iter().sum(), c.offices.iter().sum(), totals[3]];
            if k > 1 || totals != same {
                c.replicate(k, totals);
            }
        }
        c.bar_owner_count = (c.bar_owner_count as f64 * r_bar).round() as usize;
        c.bar_owner_coins = (c.bar_owner_coins as f64 / r_bar).round() as i64;
        // The one-per-city buildings: seats and stock per capita.
        let per_city = |cfg: &mut BuildingCfg, have: u32| {
            let r = up / f64::from(have.max(1));
            if r > 1.0 {
                cfg.capacity = (f64::from(cfg.capacity) * r).round().clamp(0.0, 65535.0) as u16;
                cfg.stock_cap = (f64::from(cfg.stock_cap) * r).round() as u32;
            }
        };
        let bl = &mut self.buildings;
        per_city(&mut bl.jail, n(BuildingKind::Jail));
        per_city(&mut bl.hall, n(BuildingKind::Hall));
        per_city(&mut bl.cemetery, n(BuildingKind::Cemetery));
        per_city(&mut bl.warehouse, n(BuildingKind::Warehouse));
        // Per district.
        let r = &mut self.riots;
        r.riot_max = (r.riot_max as f64 * r_dsize).round() as usize;
        r.riot_promote_max = (r.riot_promote_max as f64 * r_dsize).round() as usize;
        r.max_active_riots = (r.max_active_riots as f64 * r_districts).round() as usize;
        let l = &mut self.law;
        l.gang_landlord_homes = (l.gang_landlord_homes as f64 * r_dsize).round() as usize;
        l.sweep_full = (f64::from(l.sweep_full) * r_dsize).round() as u32;
        l.max_crackdowns = (l.max_crackdowns as f64 * r_districts).round() as usize;
        // Per Lot (the opening derelicts are per resident, `scaled_to`: they empty
        // occupied Blocks, and a map of tall Blocks has many Homes per resident).
        let st = &mut self.street;
        st.shanty_lots_max = (st.shanty_lots_max as f64 * r_lot).round() as usize;
    }
}

// ---------------------------------------------------------------------------
// M16a contracts (docs/M16_CONTRACTS.md § 1-3; plan C3). Contract records,
// Fixers, missions and bounties are game state between fictional agents:
// records with a price and a deadline, matched by a score, resolved by a
// seeded dice roll. Every section's `off()` carries the spec's values with
// the master switch off; nothing here is read without `contracts::on`.
// ---------------------------------------------------------------------------

/// A value per 16a `ContractKind` (spec tables `price_base`,
/// `deadline_days`); M16b appends its kinds' keys (serde default).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KindTable<T> {
    pub hit: T,
    pub beat: T,
    pub guard: T,
    pub locate: T,
    /// M16b phase 1.
    pub persuade: T,
    /// M16b phase 2.
    pub abduct: T,
    pub rescue: T,
    pub escort: T,
    pub steal: T,
    pub deliver: T,
    pub spin: T,
    /// M16b phase 3.
    pub trauma: T,
    /// M16b phase 4.
    pub tag: T,
    pub vote: T,
    /// M17 phase 3.
    pub retake: T,
}

impl<T: Default + Copy> KindTable<T> {
    pub fn get(&self, k: crate::contract::ContractKind) -> T {
        use crate::contract::ContractKind;
        match k {
            ContractKind::Hit => self.hit,
            ContractKind::Beat => self.beat,
            ContractKind::Guard => self.guard,
            ContractKind::Locate => self.locate,
            ContractKind::Persuade => self.persuade,
            ContractKind::Abduct => self.abduct,
            ContractKind::Rescue => self.rescue,
            ContractKind::Escort => self.escort,
            ContractKind::Steal => self.steal,
            ContractKind::Deliver => self.deliver,
            ContractKind::Spin => self.spin,
            ContractKind::Trauma => self.trauma,
            ContractKind::Tag => self.tag,
            ContractKind::Vote => self.vote,
            ContractKind::Retake => self.retake,
        }
    }
}

/// `[contracts]` (spec § 1-2; plan keys C20).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractsCfg {
    /// Open and taken records at once; a post past it is refused.
    pub max_open: usize,
    /// Closed records are dropped at midnight after this many days
    /// (fulfilled Hits and Beats: `max(this, [law] accessory_days)`, C2).
    pub closed_keep_days: u32,
    /// C7: the base price per kind.
    pub price_base: KindTable<i64>,
    /// The deadline per kind, in days.
    pub deadline_days: KindTable<u16>,
    /// C7: `× (1 + risk_w × risk)`.
    pub risk_w: f32,
    /// C7: `× (1 + standing_w × standing)`.
    pub standing_w: f32,
    /// C16: failed attempts before `Failed`.
    pub max_attempts: u8,
    /// C12, C13: a taker accepts at this `accept_score`.
    pub accept_min: f32,
    /// C13: the Contract goal's flat (mode A).
    pub contract_flat: f32,
    /// C6: `renege_base × (1 − lawfulness) × (1 − honour)`.
    pub renege_base: f32,
    /// C6: the reneged taker's `Betrayed` grudge.
    pub renege_grudge: f32,
    /// C20: a Hunt holder with `might_gap` below this hires instead.
    pub hire_gap: f32,
    /// C33 (phase 3): a gang below this strength ratio hires a Hit.
    pub hire_ratio: f32,
    /// C33 (phase 3): an exec below this lawfulness posts a Beat instead of the bribe.
    pub dirty_lobby: f32,
    /// C33 (phase 3): the law's public Locate after this many days unseen.
    pub law_bounty_days: u32,
    /// C33 (phase 3): a captain below this lawfulness posts a Hit.
    pub death_squad_lawfulness: f32,
    /// C20 (plan key, placeholder): the hire pass's Hunt score without the
    /// might term.
    pub hire_min: f32,
}

impl Default for ContractsCfg {
    fn default() -> Self {
        ContractsCfg {
            max_open: 256,
            closed_keep_days: 30,
            price_base: KindTable {
                hit: 400,
                beat: 120,
                guard: 25,
                locate: 20,
                persuade: 100,
                abduct: 300,
                rescue: 350,
                escort: 80,
                steal: 150,
                deliver: 40,
                spin: 120,
                trauma: 35,
                tag: 150,
                vote: 250,
                retake: 500,
            },
            deadline_days: KindTable {
                hit: 10,
                beat: 7,
                guard: 7,
                locate: 14,
                persuade: 7,
                abduct: 10,
                rescue: 7,
                escort: 3,
                steal: 7,
                deliver: 3,
                spin: 5,
                trauma: 7,
                tag: 10,
                vote: 14,
                retake: 10,
            },
            risk_w: 1.5,
            standing_w: 1.0,
            max_attempts: 2,
            accept_min: 0.35,
            contract_flat: 0.0,
            renege_base: 0.3,
            renege_grudge: 0.6,
            hire_gap: -0.1,
            hire_ratio: 0.7,
            dirty_lobby: 0.3,
            law_bounty_days: 3,
            death_squad_lawfulness: 0.2,
            hire_min: 0.3,
        }
    }
}

/// `[fixers] gun_lawfulness`: the illegal kinds' lawfulness ceilings (a
/// missing kind reads 1.0; M16b appends its kinds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GunLawfulnessCfg {
    pub hit: f32,
    pub beat: f32,
    /// M16b phase 2.
    pub abduct: f32,
    pub steal: f32,
    pub rescue: f32,
    /// M16b phase 4.
    pub tag: f32,
    pub vote: f32,
    /// M17 phase 3.
    pub retake: f32,
}

impl Default for GunLawfulnessCfg {
    fn default() -> Self {
        GunLawfulnessCfg { hit: 0.3, beat: 0.4, abduct: 0.3, steal: 0.4, rescue: 0.7, tag: 0.5, vote: 0.6, retake: 0.6 }
    }
}

impl GunLawfulnessCfg {
    pub fn get(&self, k: crate::contract::ContractKind) -> f32 {
        use crate::contract::ContractKind;
        match k {
            ContractKind::Hit => self.hit,
            ContractKind::Beat => self.beat,
            ContractKind::Abduct => self.abduct,
            ContractKind::Steal => self.steal,
            ContractKind::Rescue => self.rescue,
            ContractKind::Tag => self.tag,
            ContractKind::Vote => self.vote,
            ContractKind::Retake => self.retake,
            ContractKind::Guard
            | ContractKind::Locate
            | ContractKind::Persuade
            | ContractKind::Escort
            | ContractKind::Deliver
            | ContractKind::Spin
            | ContractKind::Trauma => 1.0,
        }
    }
}

/// `[fixers]` (spec § 2; plan keys C8, C13, C15, C20, C30).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FixersCfg {
    /// C16: the Fixer's share of a settled brokered record.
    pub fixer_cut: f32,
    /// C8: a founder below this lawfulness may open a Fixer's office.
    pub fixer_lawfulness: f32,
    /// C8: … with `persuasion + knowledge` at least this.
    pub fixer_skill: f32,
    /// C10: a regular's stamp lasts this many days.
    pub regular_days: u32,
    /// C13: the gun gate's best skill.
    pub gun_skill_min: f32,
    pub gun_lawfulness: GunLawfulnessCfg,
    /// C34 (phase 3): run orders a Fixer hands out a day.
    pub run_offers_per_day: u8,
    /// C12: the hour of the daily matching.
    pub match_hour: u8,
    /// C12: records a Fixer offers a day.
    pub offers_per_day: u8,
    /// C25 (phase 2): a squad's size.
    pub crew_max: u8,
    /// C8 (plan key, placeholder): open Fixers ≤ residents ÷ this, for NPC founding.
    pub fixers_per_pop: u32,
    /// C13 mode B (plan key, placeholder): the Network goal's flat.
    pub network_flat: f32,
    /// C13, C20 (plan key, placeholder): a Fixer within reach of a Home door.
    pub hire_reach_tiles: u32,
    /// C30 (plan key, placeholder, phase 3): a Fixer owner bribes at this score.
    pub bribe_min: f32,
    /// C15 (plan key, placeholder): a Guard taker's daily post, in hours.
    pub guard_hours: u32,
}

impl Default for FixersCfg {
    fn default() -> Self {
        FixersCfg {
            fixer_cut: 0.2,
            fixer_lawfulness: 0.35,
            fixer_skill: 0.8,
            regular_days: 14,
            gun_skill_min: 0.45,
            gun_lawfulness: GunLawfulnessCfg::default(),
            run_offers_per_day: 1,
            match_hour: 18,
            offers_per_day: 4,
            crew_max: 4,
            fixers_per_pop: 800,
            network_flat: -0.1,
            hire_reach_tiles: 64,
            bribe_min: 0.5,
            guard_hours: 8,
        }
    }
}

/// `[missions]` (spec § 3; plan key C23). Phase 1 reads `max_missions`,
/// `squad_below`, `strike_k`, `ledger_death_p`, `ally_cap_n`; the rest are
/// phase 2's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MissionsCfg {
    /// C17, C35: live records at once (the rest queue by price).
    pub max_missions: usize,
    /// C25: a solo estimate below this calls for a squad (phase 1 counts it).
    pub squad_below: f32,
    /// C24: `p_win = logistic(strike_k × (S_crew ÷ S_def − 1))`.
    pub strike_k: f32,
    pub loss_w: f32,
    pub pol_w: f32,
    pub hold_days: u8,
    pub sell_premium: f32,
    pub sell_regard: f32,
    /// C18: a ledger loss below this `p_win` may cost the taker its life.
    pub ledger_death_p: f32,
    /// C23 (plan key, placeholder): allies expected at the target's door.
    pub ally_cap_n: u8,
}

impl Default for MissionsCfg {
    fn default() -> Self {
        MissionsCfg {
            max_missions: 12,
            squad_below: 0.6,
            strike_k: 3.0,
            loss_w: 0.5,
            pol_w: 1.0,
            hold_days: 2,
            sell_premium: 1.5,
            sell_regard: 0.0,
            ledger_death_p: 0.15,
            ally_cap_n: 3,
        }
    }
}

/// `[bounty]` (spec § 3; 16a's three keys, M16b appends the tag keys).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BountyCfg {
    /// C27: coins per paid sighting.
    pub per_sighting: i64,
    /// C27: paid sightings per Locate record.
    pub cap_sightings: u8,
    /// C27: one paid sighting per this many hours.
    pub min_gap_hours: u32,
}

impl Default for BountyCfg {
    fn default() -> Self {
        BountyCfg { per_sighting: 15, cap_sightings: 10, min_gap_hours: 6 }
    }
}

/// `[hitmarket]` (docs/HIT_MARKET.md § 6; every value a placeholder): the
/// organic demand for Hits and Beats, priced at what the guns ask.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HitMarketCfg {
    /// H20: the switch in flight (off: M16a's hire and brain postings at
    /// `quote`).
    pub enabled: bool,
    /// H1: the price is the ask of the `ask_rank`-th cheapest candidate.
    pub ask_rank: usize,
    /// H1: no market price under this (Hit, Beat).
    pub price_floor: KindTable<i64>,
    /// Decision 7: a gun's weekly-wage floor is `max(wage_floor_min,
    /// wage_floor_mult × 7 × the city's median daily wage)`.
    pub wage_floor_min: i64,
    pub wage_floor_mult: f32,
    /// H4: an agent buyer puts this share down and owes the rest.
    pub deposit_share: f32,
    /// H3: a sharing relative's chip-in, a share of its wallet.
    pub kin_share: f32,
    /// H11: organic records open at once (`Contract.motive != None`).
    pub max_open: usize,
    /// H11: new organic records a day, city-wide.
    pub posts_per_day: usize,
    /// H12: a holder under this courage hires even when stronger.
    pub hire_courage: f32,
    /// H6: a stiffed taker's grudge on the buyer, × the unpaid share.
    pub stiff_grudge: f32,
    /// H8: after an organic record closes unfulfilled, the buyer waits this long.
    pub repost_days: u32,
    /// H19: a gang spends at most this share of its treasury on one record.
    pub gang_spend: f32,
    /// H18: a corp spends at most treasury ÷ this on one record.
    pub corp_reserve_mult: f32,
    /// H18: one corp Rival record per corp per this many days.
    pub corp_cooldown_days: u32,
    /// Decision 4: corp Rival Hits on execs, city-wide, per 365 days.
    pub corp_year_cap: u32,
    /// H18: an exec in `[dirty_lobby, dirty_beat)` buys a Beat.
    pub dirty_beat: f32,
    /// H18: a wrong by a rival corp counts this many days.
    pub rival_window_days: u32,
    /// P3 follow-up: an exec's expected defenders add up to this many
    /// private guards of the corps protecting its corp
    /// (`missions::exec_guards`). A stand-in for real bodyguards (roadmap
    /// addendum 13, ambient LOD: execs travel with company); retire it when
    /// those land.
    pub exec_guards: usize,
    // --- Phase 2: silence, debt and the errand.
    /// H8: a want not acted on lapses after this many days.
    pub want_days: u32,
    /// H9: the `Commission` step at the Fixer, in ticks (minutes).
    pub commission_mins: u64,
    /// H9: the Commission goal's flat.
    pub commission_flat: f32,
    /// H17: a Revenge buyer's `Commissioned` memory valence at fulfilment.
    pub closure_val: f32,
    /// Silence (§ 2): a suspect under this lawfulness buys its witness.
    pub silence_lawfulness: f32,
    /// H15: a beaten witness recants at `recant_p × (1 − courage)`.
    pub recant_p: f32,
    /// H14: a dead witness's report goes cold this many days after the death.
    pub cold_days: u32,
    /// Debt (§ 2): an edge debt of at least this many coins …
    pub debt_min: i32,
    /// … standing this many days …
    pub debt_days: u32,
    /// … owed to a creditor under this lawfulness buys a Beat.
    pub debt_lawfulness: f32,
    /// Debt: a Hit when a Debt Beat on the same debtor is this many days old
    /// and the debt still stands.
    pub debt_hit_days: u32,
    // --- The gaps pass (docs/HIT_MARKET.md "Gaps pass").
    /// Reach: an agent's direct Hit or Beat with fewer than `ask_rank` asks
    /// among its own people goes to the open Fixer nearest its Home (any
    /// distance) at that Fixer's market price × (1 + this): the go-between's
    /// premium for a Fixer out of walking reach.
    pub reach_premium: f32,
    /// Lawful buyers wait for the law: a revenge buyer above this lawfulness
    /// whose target is wanted (an open report) or held buys nothing while it lasts.
    pub lawful_wait: f32,
    /// Takers who can't close: a taken organic Hit or Beat still queued for
    /// a live slot (`[missions] max_missions`) this many hours after the take
    /// is worked on the ledger.
    pub queue_hours: u32,
}

impl Default for HitMarketCfg {
    fn default() -> Self {
        HitMarketCfg {
            enabled: true,
            ask_rank: 2,
            price_floor: KindTable { hit: 20, beat: 8, ..Default::default() },
            wage_floor_min: 50,
            wage_floor_mult: 0.5,
            deposit_share: 0.5,
            kin_share: 0.5,
            max_open: 16,
            posts_per_day: 3,
            hire_courage: 0.4,
            stiff_grudge: 0.8,
            repost_days: 7,
            gang_spend: 0.5,
            corp_reserve_mult: 5.0,
            corp_cooldown_days: 30,
            corp_year_cap: 2,
            dirty_beat: 0.45,
            rival_window_days: 30,
            exec_guards: 3,
            want_days: 30,
            commission_mins: 30,
            commission_flat: 0.0,
            closure_val: 0.3,
            silence_lawfulness: 0.3,
            recant_p: 0.5,
            cold_days: 3,
            debt_min: 15,
            debt_days: 21,
            debt_lawfulness: 0.35,
            debt_hit_days: 30,
            reach_premium: 0.5,
            lawful_wait: 0.6,
            queue_hours: 24,
        }
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

/// `[economy2] labour_share` (spec § 5, plan E17): the share of a niche's
/// revenue its payroll targets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabourShareCfg {
    pub food: f32,
    pub housing: f32,
    pub security: f32,
    pub tech: f32,
}

impl Default for LabourShareCfg {
    fn default() -> Self {
        LabourShareCfg { food: 0.6, housing: 0.4, security: 0.7, tech: 0.55 }
    }
}

impl LabourShareCfg {
    pub fn of(&self, niche: crate::components::Niche) -> f32 {
        use crate::components::Niche;
        match niche {
            Niche::Food => self.food,
            Niche::Housing => self.housing,
            Niche::Security => self.security,
            Niche::Tech => self.tech,
            // Jobs & lives: the Vice venues (phase 4) pay as the Food trade's
            // counters until they have their own share.
            Niche::Vice => self.food,
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

/// `[charity]` (spec § 4 verbatim plus the plan keys of E26-E29).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharityCfg {
    /// Missions at seed (the Sump first); 0 seeds none.
    pub seed: u32,
    /// A Mission serves the hungry poor whose door (Home, else tile) is within this.
    pub reach_tiles: u32,
    /// A meal costs the kitchen `wholesale + meal_markup` a unit.
    pub meal_markup: i64,
    /// A cot's ledger-only cost a night (the purse pays nobody).
    pub cot_price: i64,
    /// A clinic hour's cost (tier ≥ 1; no tier-0 Therapy target at HEAD: unused).
    pub clinic_price: i64,
    /// E28: `p = donate_base × (lawfulness + loyalty) ÷ 2 × (1 + creed)`.
    pub donate_base: f32,
    /// E28: the share of the surplus a donor gives.
    pub donate_frac: f32,
    /// E28, E30: a corp under Lobby gives this to its district's Mission.
    pub lobby_gift: i64,
    /// Spec: standing per 100 coins given; carried by the `Gave` deed's
    /// honour weight (E29), read by nothing directly.
    pub rep_gift: f32,
    /// E27 (plan key): served when hunger is below this.
    pub hunger_below: f32,
    /// E27 (plan key): the hourly service cap (doubled by a Volunteer on shift).
    pub meals_per_hour: u16,
    /// E28 (plan key): a donor keeps this many days of rent and meals.
    pub give_floor_days: i64,
    /// E28 (plan key): a gang under Expand with a front gives this a week.
    pub gang_gift: i64,
    /// E29 (plan key): a gift of at least this writes the `Gave` deed.
    pub rep_gift_min: i64,
    /// E26 (plan key): `Register`'s per-capita target of Missions is
    /// `population ÷ this` (the founding tie-break's denominator).
    pub residents_per_mission: u32,
}

impl Default for CharityCfg {
    fn default() -> Self {
        CharityCfg {
            seed: 1,
            reach_tiles: 40,
            meal_markup: 1,
            cot_price: 1,
            clinic_price: 6,
            donate_base: 0.02,
            donate_frac: 0.05,
            lobby_gift: 40,
            rep_gift: 0.02,
            hunger_below: 0.4,
            meals_per_hour: 10,
            give_floor_days: 14,
            gang_gift: 10,
            rep_gift_min: 20,
            residents_per_mission: 1000,
        }
    }
}

/// `[camp]` (addendum 19; plan E37-E42; placeholders).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CampCfg {
    /// E38: `Child.hunger_days` (consecutive unfed days; 3 kills) at which
    /// the service takes a child (must be ≤ 2).
    pub take_days: u8,
    /// E40: sim years; younger children are housed and fed, not worked.
    pub work_age: u32,
    /// E40: Parts per child-day at skill 0.
    pub camp_yield: f32,
    pub camp_skill_floor: f32,
    pub camp_skill_slope: f32,
    /// E40: farming (the production skill) per shift.
    pub camp_skill_gain: f32,
    /// E37: corps found a camp when every camp is this full.
    pub found_full: f32,
    /// E42: unfed days before the law closes a camp.
    pub close_days: u8,
    pub close_ban_days: u32,
    /// E42: the owner's standing per unfed child-day in the last 30 days.
    pub scandal_standing: f32,
    /// E41: the released adult's affinity to each parent.
    pub family_affinity: f32,
    /// E41: fed share at or above which the camp earns loyalty.
    pub loyal_share: f32,
    /// E41: fed share below which the released adult holds a grudge.
    pub grudge_share: f32,
}

impl Default for CampCfg {
    fn default() -> Self {
        CampCfg {
            take_days: 2,
            work_age: 10,
            camp_yield: 0.15,
            camp_skill_floor: 0.6,
            camp_skill_slope: 0.8,
            camp_skill_gain: 0.002,
            found_full: 0.8,
            close_days: 3,
            close_ban_days: 30,
            scandal_standing: -0.05,
            family_affinity: -0.2,
            loyal_share: 0.9,
            grudge_share: 0.7,
        }
    }
}

/// Jobs and room § 2.3 (plan J7, J8) `[floors]`: what a floor costs, when a
/// corp adds one, and how high each kind may go.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FloorsCfg {
    /// J8: a floor costs this × the kind's `found_cost`.
    pub floor_cost_frac: f32,
    /// J8: midnights at the ceiling with room for a floor's staff before a corp adds one.
    pub floor_days: u8,
    /// J8: a corp adds a floor only with no vacant Lot within this many
    /// tiles (Manhattan, door to door) of the building.
    pub lot_reach: u32,
    /// J8: the highest a kind may go, keyed by the map's kind names
    /// (`BuildingKind::parse`); a kind not listed stays at one floor.
    pub floors_max: std::collections::BTreeMap<String, u8>,
}

impl Default for FloorsCfg {
    fn default() -> Self {
        let max = [
            ("Farm", 3),
            ("Fab", 3),
            ("Home", 6),
            ("Club", 2),
            ("Arcade", 2),
            ("NoodleBar", 2),
            ("FightPit", 2),
            ("Den", 2),
            ("Lounge", 2),
        ];
        FloorsCfg {
            floor_cost_frac: 0.6,
            floor_days: 7,
            lot_reach: 48,
            floors_max: max.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }
}

impl FloorsCfg {
    /// J8: the most floors a `kind` may have (1 when not listed). Jobs &
    /// lives phase 1: a `[buildings.<kind>] floors_max` comes first
    /// ([`Config::floors_max`]).
    pub fn max_for(&self, kind: BuildingKind) -> u8 {
        self.floors_max.iter().find(|(k, _)| BuildingKind::parse(k) == Some(kind)).map_or(1, |(_, &v)| v.max(1))
    }
}

/// `[sickness]` (docs/HUNGER_SICKNESS.md § 8; starting values). Always on
/// since P10 (its `enabled` switch retired); `diet`, `bins` and `cancer`
/// are its parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SicknessCfg {
    /// § 1-2: synth and real meals, the meters, the debuffs.
    pub diet: bool,
    /// § 4: the bins and the dive.
    pub bins: bool,
    /// § 3: cancer and its treatment.
    pub cancer: bool,
    /// Synth meals a shelf unit is cut into (each at a `1 / stretch` price).
    pub synth_stretch: u32,
    /// Buy real at coins >= this many real meals (coordinator call on P2:
    /// 10 kept the employed on synth for good) ...
    pub real_meal_reserve: i64,
    /// ... or this malnourished with two real meals' coins.
    pub mal_seek_real: f32,
    /// An hour below this hunger is a hungry hour.
    pub mal_hungry_edge: f32,
    /// Malnutrition per fully hungry day.
    pub mal_hungry: f32,
    /// Malnutrition per day at an all-synth (or scraps) diet.
    pub mal_poor_diet: f32,
    /// A synth (or Mission) diet raises malnutrition only to here: run-down,
    /// not sick; hunger and bin scraps push past it (coordinator call on P2).
    pub synth_malnut_cap: f32,
    /// Malnutrition off per real meal (two a day at most).
    pub mal_real: f32,
    pub mal_onset: f32,
    /// Toxicity per synth or Mission meal.
    pub tox_synth: f32,
    /// Toxicity per bin meal.
    pub tox_dive: f32,
    /// The share of toxicity cleared a day (after P5: a flat clearance gave
    /// no resting level). The steady level of a diet of S synth meals a day
    /// is `tox_synth x S / tox_clear_frac`.
    pub tox_clear_frac: f32,
    /// World creation (and an arrival): an adult's toxicity approaches its
    /// diet's steady level by `1 - exp(-(age - 16) / tox_seed_years)`.
    pub tox_seed_years: f32,
    /// The seeded levels' spread: x `(1 - j/2 + j u)`, `u` a hash.
    pub tox_seed_jitter: f32,
    pub tox_onset: f32,
    pub mal_energy: f32,
    pub tox_energy: f32,
    pub cancer_energy: f32,
    pub mal_work: f32,
    pub cancer_work: f32,
    /// A day's sick chance per unit of malnutrition past the onset (scaled).
    pub sick_mal: f32,
    /// Sick days in the last 14 that cost the job (Dylan, decision 4).
    pub sick_fire_days: u32,
    pub mal_mood: f32,
    pub tox_mood: f32,
    pub cancer_mood: f32,
    /// The old-age hazard x (1 + this x (m + t)).
    pub frail_old: f32,
    /// The fight's skill term less this x (m' + c).
    pub frail_fight: f32,
    /// No conception at or above this malnutrition.
    pub fertile_mal_max: f32,
    /// The cancer's daily hazard: p = this x toxicity x age factor (Dylan,
    /// after P5: a stationary hazard; no dose term, which compounded).
    pub cancer_hazard: f32,
    /// The age factor `1 + max(0, age - cancer_age_from) / cancer_age_span`
    /// (sim years).
    pub cancer_age_from: f32,
    pub cancer_age_span: f32,
    pub cancer_growth: f32,
    pub cancer_symptom: f32,
    pub cancer_treat_price: i64,
    pub cure_stage: f32,
    pub remission: f32,
    pub treat_tox_mult: f32,
    pub p_stat_treat: f32,
    pub bin_cap: u16,
    pub bin_meals_per_unit: u16,
    pub venue_scrap: f32,
    /// Scrap meals into a Market's bin per shelf unit sold (retail waste).
    pub market_waste: f32,
    /// Scrap meals into a Home's bin per pantry meal eaten (household waste).
    pub home_scrap: f32,
    pub bin_rot: f32,
    pub dive_reach_tiles: u32,
    pub dive_satisfy: f32,
    pub dive_sick_p: f32,
    pub regular_days: u8,
    /// A held gang Home within this many tiles makes the bin the gang's.
    pub bin_turf_tiles: u32,
    pub bin_shake_p: f32,
    pub bin_toll: i64,
    /// Days at hunger 0 before a death by starvation.
    pub starve_grace_days: u64,
}

impl Default for SicknessCfg {
    fn default() -> Self {
        SicknessCfg {
            diet: true,
            bins: true,
            cancer: true,
            synth_stretch: 3,
            real_meal_reserve: 3,
            mal_seek_real: 0.5,
            mal_hungry_edge: 0.25,
            mal_hungry: 0.08,
            mal_poor_diet: 0.012,
            synth_malnut_cap: 0.45,
            mal_real: 0.02,
            mal_onset: 0.3,
            tox_synth: 0.005,
            tox_dive: 0.012,
            tox_clear_frac: 0.03,
            tox_seed_years: 8.0,
            tox_seed_jitter: 0.3,
            tox_onset: 0.4,
            mal_energy: 0.5,
            tox_energy: 0.3,
            cancer_energy: 1.0,
            mal_work: 0.4,
            cancer_work: 0.5,
            sick_mal: 0.3,
            sick_fire_days: 5,
            mal_mood: 0.2,
            tox_mood: 0.15,
            cancer_mood: 0.3,
            frail_old: 2.0,
            frail_fight: 0.2,
            fertile_mal_max: 0.5,
            cancer_hazard: 0.00017,
            cancer_age_from: 30.0,
            cancer_age_span: 10.0,
            cancer_growth: 0.015,
            cancer_symptom: 0.25,
            cancer_treat_price: 240,
            cure_stage: 0.6,
            remission: 0.4,
            treat_tox_mult: 0.5,
            p_stat_treat: 0.2,
            bin_cap: 40,
            bin_meals_per_unit: 2,
            venue_scrap: 0.3,
            market_waste: 0.2,
            home_scrap: 0.2,
            bin_rot: 0.5,
            dive_reach_tiles: 40,
            dive_satisfy: 0.6,
            dive_sick_p: 0.05,
            regular_days: 3,
            bin_turf_tiles: 15,
            bin_shake_p: 0.3,
            bin_toll: 1,
            starve_grace_days: 7,
        }
    }
}

/// `[classpay]` (docs/CLASS_PAY.md; placeholders, not tuned). Every key is
/// read only with `classpay::on` (the market wage law and
/// `enabled`): the dole city is the city before it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassPayCfg {
    pub enabled: bool,
    /// A post's wage multiple by grade: labour, skilled, professional, management.
    pub grade_mult: [f32; 4],
    /// The bespoke roles' base grade by `{role:?}` (others labour).
    pub role_grade: std::collections::BTreeMap<String, u8>,
    /// Of a corp building's labour staff in a role, the skilled posts (floor).
    pub skilled_share: f32,
    /// Days at the corp before a promotion.
    pub promote_days: u32,
    pub merit_edu: f32,
    pub merit_skill: f32,
    pub merit_tenure: f32,
    /// Tenure at the same corp counts in full at this many days.
    pub tenure_full_days: u32,
    /// Merit lost per tile of commute (Home door to workplace door).
    pub commute_w: f32,
    /// Education's mean by origin: Dreg, Sump, Mid, Spire.
    pub education: [f32; 4],
    /// The seeded and immigrant spread (+- uniform, a pure hash).
    pub education_spread: f32,
    /// A child's spread around its parents' mean.
    pub child_spread: f32,
    /// P3, the thin ladder: education gained a day in a skilled or graded post.
    pub edu_gain_per_day: f32,
    /// P3: the night school's cap.
    pub edu_cap: f32,
    /// P3, the status sink: coins an adult keeps before spending on status.
    pub status_floor: i64,
    /// P3: the share of the excess over `status_floor` spent a day.
    pub status_frac: f32,
    /// P3: the split: a Spire night out, fashion (a corp Market), cosmetic chrome (a corp Clinic).
    pub status_split: [f32; 3],
    /// P3 (Dylan): the most of the capital payout the graded premium may
    /// draw on; the rest always goes into labour's P*.
    pub premium_payout_share: f32,
}

impl Default for ClassPayCfg {
    fn default() -> Self {
        ClassPayCfg {
            enabled: false,
            grade_mult: [1.0, 1.3, 2.0, 3.0],
            role_grade: std::collections::BTreeMap::new(),
            skilled_share: 0.25,
            promote_days: 30,
            merit_edu: 0.5,
            merit_skill: 0.3,
            merit_tenure: 0.2,
            tenure_full_days: 180,
            commute_w: 0.0005,
            education: [0.10, 0.25, 0.50, 0.80],
            education_spread: 0.20,
            child_spread: 0.10,
            edu_gain_per_day: 0.0003,
            edu_cap: 0.75,
            status_floor: 60,
            status_frac: 0.05,
            status_split: [0.5, 0.25, 0.25],
            premium_payout_share: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// M16b § 4 leverage (docs/M16_CONTRACTS.md; phase 1). A coercion is a record
// on a struct and one seeded Intimidate roll between fictional agents.
// ---------------------------------------------------------------------------

/// `[leverage] lev_bias`: the logit's bias per leverage kind (phase 2
/// appends `hostage`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevBiasCfg {
    pub threat: f32,
    pub secret: f32,
    pub debt: f32,
    /// M16b phase 2.
    pub hostage: f32,
}

impl Default for LevBiasCfg {
    fn default() -> Self {
        LevBiasCfg { threat: 0.0, secret: 0.3, debt: 0.2, hostage: 0.5 }
    }
}

impl LevBiasCfg {
    pub fn get(&self, k: crate::contract::LeverageKind) -> f32 {
        use crate::contract::LeverageKind;
        match k {
            LeverageKind::Threat => self.threat,
            LeverageKind::Secret => self.secret,
            LeverageKind::Debt => self.debt,
            LeverageKind::Hostage => self.hostage,
        }
    }
}

/// `[leverage]` (spec § 4; the plan keys marked). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LeverageCfg {
    /// The switch in flight (the A/B and identity device; off: M16a's
    /// city, the shakedown M15's roll).
    pub enabled: bool,
    /// Standing coercions at once (per capita, up only).
    pub max_coercions: usize,
    pub lev_bias: LevBiasCfg,
    /// `demand_cost = demand_w × value ÷ max(purse + 7 × wage, 1)`.
    pub demand_w: f32,
    /// A Secret adds `secret_w × deed_sev × (1 − reach)`.
    pub secret_w: f32,
    /// A failed move is reported with `p = backlash_report × lawfulness`.
    pub backlash_report: f32,
    pub protection_weekly: i64,
    pub blackmail_weekly: i64,
    pub blackmail_lawfulness: f32,
    /// Plan key: the coerced's standing above the holder's by this much.
    pub blackmail_gap: f32,
    /// Plan key: a holder looks for a secret once in this many days.
    pub blackmail_days: u32,
    pub secret_known_max: u32,
    pub secret_public_reach: f32,
    pub silence_lawfulness: f32,
    pub poach_threat_lawfulness: f32,
    /// Plan key: a debt under this is not called (begging loans).
    pub debt_call_min: i64,
    pub fake_fee: i64,
    /// Plan key: a fake-job candidate tries on a day with this p.
    pub fake_p: f32,
    /// Plan key: the fake job's victim learns it after this many days.
    pub fake_days: u32,
    pub parley_ratio: f32,
    pub parley_tribute: i64,
    /// Plan key: a fulfilled Persuade adds this to the courted's affinity.
    pub persuade_affinity: f32,
    /// Plan key: a suitor rejected this often by the same person (30 days).
    pub court_rejections: u32,
    /// Plan key: ... and under this sociability buys a Persuade.
    pub court_sociability: f32,
    /// Plan key: a pair refused, broken or failed is not tried again for this long.
    pub retry_days: u32,
    /// M16b phase 2: a Hostage adds `hostage_w × rel_w(over, captive)`.
    pub hostage_w: f32,
    /// M16b phase 2: a spinning corp's exec under this fabricates.
    pub fabricate_lawfulness: f32,
    /// M16b phase 2: a fabricated plant's price multiplier.
    pub fabricate_mult: f32,
    /// M16b phase 2: a contradicting holder exposes a fabrication a day with this p.
    pub expose_p: f32,
}

impl Default for LeverageCfg {
    fn default() -> Self {
        LeverageCfg {
            enabled: true,
            max_coercions: 128,
            lev_bias: LevBiasCfg::default(),
            demand_w: 1.0,
            secret_w: 1.0,
            backlash_report: 0.5,
            protection_weekly: 40,
            blackmail_weekly: 30,
            blackmail_lawfulness: 0.35,
            blackmail_gap: 0.2,
            blackmail_days: 30,
            secret_known_max: 5,
            secret_public_reach: 0.3,
            silence_lawfulness: 0.3,
            poach_threat_lawfulness: 0.3,
            debt_call_min: 3,
            fake_fee: 40,
            fake_p: 0.1,
            fake_days: 7,
            parley_ratio: 0.3,
            parley_tribute: 30,
            persuade_affinity: 0.3,
            court_rejections: 2,
            court_sociability: 0.4,
            retry_days: 28,
            hostage_w: 1.5,
            fabricate_lawfulness: 0.35,
            fabricate_mult: 2.0,
            expose_p: 0.05,
        }
    }
}

/// `[hostage]` (M16b phase 2; spec § 4 "Hostages and the private prison",
/// with the plan keys marked). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HostageCfg {
    /// The switch in flight (off: the phase-1 city).
    pub enabled: bool,
    pub hostage_days: u32,
    /// Coins a day the holder pays for a captive's meal.
    pub hostage_food: i64,
    pub p_hostage_kill: f32,
    /// Plan deviation (spec `[lod] captive_quota`): captives held at once,
    /// city-wide (per capita, up only).
    pub captive_quota: usize,
    /// Plan key: a ransom's coins.
    pub ransom: i64,
    /// Plan key (spec `found_cost.prison`): what a corp pays to build one.
    pub prison_cost: i64,
    /// Plan key (spec `upkeep.prison`): a Prison's daily upkeep.
    pub prison_upkeep: i64,
    /// Plan key: a Steal on a building takes this share of its owner's purse ...
    pub steal_share: f32,
    /// ... at most this.
    pub steal_max: i64,
    /// Plan key: a corpse unburied this long may be bought a Deliver.
    pub bury_hours: u32,
    /// Spec `[contracts] abduct_share`: a Harvest's Abduct price, × the chrome's value.
    pub abduct_share: f32,
}

impl Default for HostageCfg {
    fn default() -> Self {
        HostageCfg {
            enabled: true,
            hostage_days: 7,
            hostage_food: 3,
            p_hostage_kill: 0.3,
            captive_quota: 6,
            ransom: 200,
            prison_cost: 800,
            prison_upkeep: 20,
            steal_share: 0.2,
            steal_max: 300,
            bury_hours: 24,
            abduct_share: 0.3,
        }
    }
}

/// `[wounded]` (M16b phase 3; spec § 5, with the plan keys marked). Every
/// value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WoundedCfg {
    /// The switch in flight (off: the phase-2 city).
    pub enabled: bool,
    /// A fight's death becomes a wound with `wound_share × (1 − armour)`.
    pub wound_share: f32,
    pub bleed_hours: u32,
    pub stabilise_price: i64,
    pub recover_days: u32,
    pub rescue_radius: u32,
    pub rescue_loyalty: f32,
    /// Spec `[lod] wounded_quota`: downed at once (per capita, up only).
    pub wounded_quota: usize,
    /// Plan key (the Statistical split of `p_killed`): the share of
    /// wounds a Full run saves; off screen an hour's killing survives as
    /// an assault with `wound_share × stat_saved`.
    pub stat_saved: f32,
    /// Plan key: the Rescue goal's flat (a Trauma crew's call adds
    /// `trauma_flat`).
    pub rescue_flat: f32,
    pub trauma_flat: f32,
    /// Review finding 6: a stabilised patient's daily meal (off a Market's
    /// shelf, as a captive's), paid by its carer, else itself.
    pub care_food: i64,
}

impl Default for WoundedCfg {
    fn default() -> Self {
        WoundedCfg {
            enabled: true,
            wound_share: 0.6,
            bleed_hours: 3,
            stabilise_price: 80,
            recover_days: 3,
            rescue_radius: 12,
            rescue_loyalty: 0.5,
            wounded_quota: 12,
            stat_saved: 0.16,
            rescue_flat: 0.0,
            trauma_flat: 0.5,
            care_food: 3,
        }
    }
}

/// `[trauma]` (M16b phase 3; spec § 5 "Trauma Team").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraumaCfg {
    pub fee_weekly: i64,
    pub trauma_crew: u32,
    pub subscribe_coins: i64,
    pub subscribe_heat: f32,
    pub p_subscribe: f32,
    pub trauma_stat_save: f32,
}

impl Default for TraumaCfg {
    fn default() -> Self {
        TraumaCfg {
            fee_weekly: 35,
            trauma_crew: 2,
            subscribe_coins: 400,
            subscribe_heat: 0.3,
            p_subscribe: 0.2,
            trauma_stat_save: 0.5,
        }
    }
}

/// `[governance]` (M16b phase 4; spec § 6). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GovernanceCfg {
    /// The phase 4 switch (off: no boards, tags or attention).
    pub enabled: bool,
    /// The roster names whose corps (and their copies) have boards.
    pub boards: Vec<String>,
    pub board_size: usize,
    pub vote_days: u32,
    pub campaign_days: u32,
    pub ambition_min: f32,
    pub vote_bribe: i64,
}

impl Default for GovernanceCfg {
    fn default() -> Self {
        GovernanceCfg {
            enabled: true,
            boards: vec!["Habitat".to_string(), "Nutrix".to_string()],
            board_size: 5,
            vote_days: 45,
            campaign_days: 7,
            ambition_min: 0.12,
            vote_bribe: 150,
        }
    }
}

/// `[tags]` (M16b phase 4; spec § 3's `[bounty]` tag and scanner keys and
/// the Patrol's `patrol_share`, kept together).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TagsCfg {
    pub enabled: bool,
    pub tag_price: [i64; 4],
    pub tag_conf: [f32; 4],
    pub tag_battery_days: u32,
    /// City-wide (per capita, up only).
    pub max_tags: usize,
    pub scanner_price: [i64; 4],
    pub scan_price: i64,
    pub scan_heat: f32,
    pub patrol_share: f32,
    /// Plan keys: the scanner a Clinic and a Security Office keep.
    pub clinic_scanner: u8,
    pub office_scanner: u8,
    /// Plan key: a scanning adult goes once in this many days.
    pub scan_days: u32,
    /// Plan key: the Patrol's reach around its spot.
    pub patrol_radius: u32,
}

impl Default for TagsCfg {
    fn default() -> Self {
        TagsCfg {
            enabled: true,
            tag_price: [0, 150, 400, 900],
            tag_conf: [0.0, 0.7, 0.85, 0.95],
            tag_battery_days: 10,
            max_tags: 32,
            scanner_price: [0, 300, 800, 2000],
            scan_price: 40,
            scan_heat: 0.4,
            patrol_share: 0.3,
            clinic_scanner: 1,
            office_scanner: 2,
            scan_days: 7,
            patrol_radius: 6,
        }
    }
}

/// `[attention]` (M16b phase 4; the addendum's table). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AttentionCfg {
    pub enabled: bool,
    pub alert_floor: f32,
    pub linger_days: u32,
    pub raid: f32,
    pub riot: f32,
    pub loud_run: f32,
    pub strike: f32,
    pub acquisition: f32,
    pub vendetta: f32,
    /// Plan key (deviation): the flat tilt `brain_tilt × (1 − alertness ÷
    /// pool)` a distracted brain adds to Hunker, Garrison and LieLow (a
    /// consideration at no distraction would move every rescore).
    pub brain_tilt: f32,
}

impl Default for AttentionCfg {
    fn default() -> Self {
        AttentionCfg {
            enabled: true,
            alert_floor: 0.2,
            linger_days: 2,
            raid: 0.5,
            riot: 0.4,
            loud_run: 0.3,
            strike: 0.2,
            acquisition: 0.15,
            vendetta: 0.15,
            brain_tilt: 0.2,
        }
    }
}
