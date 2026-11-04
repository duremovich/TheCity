//! Every tunable constant, loaded once from `assets/config.toml`.
//!
//! The struct mirrors the `## Tuning defaults` table in the spec plus the
//! initial-world and building tables. Systems read from `world.config`; no
//! magic numbers live in system code.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::components::{AssetClass, AssetKind, BuildingKind, Lod, Niche, Role};

mod buildings;
mod combat;
mod contracts;
mod corps;
mod crime;
mod economy;
mod gangs;
mod health;
mod housing;
mod law;
mod life;
mod living;
mod mind;
mod movement;
mod profiles;
mod runtime;
mod service;
mod social;
mod virt;
mod work;
mod world;

pub use buildings::*;
pub use combat::*;
pub use contracts::*;
pub use corps::*;
pub use crime::*;
pub use economy::*;
pub use gangs::*;
pub use health::*;
pub use housing::*;
pub use law::*;
pub use life::*;
pub use living::*;
pub use mind::*;
pub use movement::*;
pub use runtime::*;
pub use service::*;
pub use social::*;
pub use virt::*;
pub use work::*;
pub use world::*;

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
    /// M18 P3 (the board and talk): the talk verbs' terms (`player::talk`).
    #[serde(default)]
    pub talk: TalkCfg,
    /// M18 P5 (the slice's start and loop): the nobodies the menu offers,
    /// a fresh arrival, the blackmail offer (`player` § P5).
    #[serde(default)]
    pub slice: SliceCfg,
    /// M18 P5h (housing as an errand): the FindHome goal's desks and
    /// refusals, the city's list, the capsule tenure (`systems::housing`).
    #[serde(default)]
    pub housing: HousingCfg,
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
    /// The combat model C1 (docs/COMBAT.md): the beat loop, wounds, the
    /// three skills, stances.
    #[serde(default)]
    pub combat: CombatCfg,
    /// C1 (docs/COMBAT.md § 4): the weapon kinds; row 0 is the bare hand.
    #[serde(default = "default_weapon_kinds")]
    pub weapon_kinds: Vec<WeaponKindCfg>,
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
    /// The pressure pass phase 5a (docs/PRESSURE_PASS.md § Phase 5):
    /// consequence, the non-gang half (`systems/consequence.rs`): witnesses
    /// react, an expired hit escalates, jail is a place, the faith's service.
    /// `None` (not written into a save) with the key absent: none of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consequence: Option<ConsequenceCfg>,
    /// The pressure pass phase 5b (docs/PRESSURE_PASS.md § Phase 5, the gang
    /// half): raids write a result, gang duty against the day job, Court's
    /// quorum and the gang that folds, release and return, the crew's rest
    /// day (`systems/crew.rs`). `None` (not written into a save) with the key
    /// absent: the gangs as phase 3 left them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew: Option<CrewCfg>,
    /// Pressure pass round 6, V2 (docs/TURF.md § 1-2): turf cells held by
    /// presence, the corner, turf that pays (`systems/turf.rs`). `None` (not
    /// written into a save) with the key absent: territory is Homes alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turf: Option<TurfCfg>,
    /// Pressure pass round 2, R1 (docs/PRESSURE_PASS.md § R1, `OPEN-213`):
    /// petty law for the fare, a ladder and never a cell (`systems/fares.rs`).
    /// `None` (not written into a save) with the key absent: a dodger is
    /// reported, arrested and jailed as any petty offender.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fares: Option<FaresCfg>,
    /// Pressure pass round 3, S3 (docs/PRESSURE_PASS.md § S3, `OPEN-223`):
    /// justice that reads right: the cells' kitchen and lights-out, the
    /// guard who acts on what he saw. `None` (not written into a save) with
    /// the key absent: the cells and the watch as round 2 left them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justice: Option<JusticeCfg>,
    /// V8 (`OPEN-256`): the cells' week (`systems/cells.rs`): a short term
    /// bought out at booking or bailed from the purse (the prisoner's, his
    /// kin's), the crowded Precinct letting its least dangerous go, the work
    /// detail and cards in the yard. `None` with the key absent: the cells as
    /// round 9 left them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cells: Option<CellsCfg>,
    /// The pressure pass round 2, R4 (OPEN-218, OPEN-219): `[noise]`, the
    /// diary's folds and the Camp answer's hour. Absent: the defaults.
    #[serde(default)]
    pub noise: NoiseCfg,
    /// Pressure pass round 2, R3 (docs/PRESSURE_PASS.md § R3; OPEN-216): the
    /// wounded and the sick (`systems/care.rs`). `None` (not written into a
    /// save) with the key absent: care as phase 2 left it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub care: Option<CareCfg>,
    /// Pressure pass round 2, R6b (docs/PRESSURE_PASS.md § R6b; OPEN-217,
    /// OPEN-207): the trades' verbs II (the corp's office, the reporter's
    /// story, the gangs' gun trade). `None` (not written into a save) with
    /// the key absent: as R6a left them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbs: Option<VerbsCfg>,
    /// Pressure pass round 4, T5 (docs/PRESSURE_PASS.md § T5; OPEN-217,
    /// OPEN-228's follow-up): the jobless look for work (`systems/search.rs`).
    /// `None` (not written into a save) with the key absent: as T4 left them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<SearchCfg>,
    /// Pressure pass round 8, X2 (OPEN-247): a death in the household is
    /// grief that takes days (`systems/grief.rs`). `None` (not written into
    /// a save) with the key absent: the Grief memory alone, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grief: Option<GriefCfg>,
    /// Pressure pass round 2, R6a (docs/PRESSURE_PASS.md § R6a; OPEN-217):
    /// the trades' verbs on a followed body (the cab's pool, the pimp's
    /// round, the pit's bettors, the madam's bookings, the joytoy's lonely).
    /// `None` (not written into a save) with the key absent: as R1-R4 left them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trade_verbs: Option<TradeVerbsCfg>,
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
    /// The city milestone C6b (docs/CITY_MILESTONE.md § C6b): floors as
    /// tenancies, the economy of floors (`systems/floors.rs`): a tenancy's
    /// floor rent to its host's owner, the evictions by the floor, founding
    /// into a vacant unit, a buyout's lease of floors, a tenancy's hours.
    /// `None` (not written into a save) with the key absent: tenancies pay
    /// no floor rent and founding takes a Lot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenancy: Option<TenancyCfg>,
    /// The city milestone C7 (docs/CITY_MILESTONE.md § C7): levels, portals
    /// and pathing (`systems/levels.rs`): the climb inside a structure by
    /// its stairs or its lift (a place with a queue), and the walkway
    /// portals' climbs. `None` (not written into a save) with the key
    /// absent: no climb (a body is on its floor the tick it enters) and a
    /// portal costs only its run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<LevelsCfg>,
    /// The view milestone V7 (docs/VIEW_MILESTONE.md § V7, OPEN-260):
    /// signals at the junctions and the marked crossings (`signals`, the
    /// map's `carriage` grid and `X` lines): a walker's path pays to step
    /// onto a carriageway off a crossing, a crossing costs its signal's
    /// expected wait, a Full walker waits at the kerb for the walk phase and
    /// a timed walk on foot adds the expected waits of the crossings it is
    /// likely to make. `None` (not written into a save) with the key absent:
    /// crossing a street costs nothing anywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossings: Option<CrossingsCfg>,
    /// The city milestone C8 (docs/CITY_MILESTONE.md § C8): the vertical
    /// gangs (`systems/vertical.rs`): a Hideout per district a gang holds
    /// turf in, on upper floors; the lift as turf (held, cut); the raid's
    /// climb. `None` (not written into a save) with the key absent: one
    /// Hideout a gang, on the floor the map gives it, no lift held or cut by
    /// a gang, the raid fights at the door.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<VerticalCfg>,
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
    /// The city milestone C2: `[view]`, the camera ring the 3D client reads
    /// (`citysim::view`). Read by the view only; no system of the city reads
    /// it. Absent: the defaults.
    #[serde(default)]
    pub view: ViewCfg,
    /// M18 W (work beats, docs/M18_PLAYER.md § W): `[work_beat]`, when a
    /// shift's block seam draws a beat (`systems::workbeats`). `None` (not
    /// written into a save) with the key absent: no beats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_beat: Option<WorkBeatCfg>,
    /// M18 W: the `[[work_beats]]` deck, one row a beat (who it reaches, when,
    /// its counterpart, its effect, its line).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_beats: Vec<WorkBeatRow>,
}

impl Config {
    /// The view milestone V1 (`OPEN-262`, `citysim::reload`): the config a reload may not change in a running
    /// world, as dotted paths (a section, or a key inside one; a path covers everything under it). Every other key
    /// is live: a reload swaps it in at a tick boundary and the next system to read it reads the new value (a key
    /// read only at a seeding, e.g. `[buildings.<kind>] capacity`, moves the next building that seeds). A reload
    /// keeps the world's value for these and reports the change as ignored. Build-time is what a world build or a
    /// cache derives once:
    /// - `world`: the map, the population, the opening jobs, stocks, coins and the names file (the build's);
    /// - `scale`: the reference city the build fitted from (and its `fitted`, `lot_copies`);
    /// - `districts`: the map's rows; the district grid and adjacency derive from them at the load;
    /// - `trades`: `Role::Trade` indexes the rows (the trade cache, a save's `Trade("key")`s); `faith.creeds` and
    ///   `gray.gang_types` likewise index rows the opening dealt;
    /// - `levers`, `policy`: seed `World::levers` once (`Levers::from_config`); a live lever moves by a command;
    /// - `ambient`: the Ambient flats are built once;
    /// - `transit`: the El's net (its stops and offsets, derived at the load) and its trains, spawned by slot once;
    /// - `perf`: the host's threads; the midnight stagger's slots (a change mid-day runs a pass twice or skips it);
    /// - the names and the opening roster: `gangs.names`, `gangs.splinter_names`, `gangs.treasury_initial`, the
    ///   corps' rows, `tech.seed`, `governance.boards`, `outside.parents` (the parents' accounts, seeded once);
    /// - `view.tile_m`, `view.north_m`: the frame the client draws the city in.
    ///
    /// The caches a live key feeds are rebuilt by the reload: the walkway portals' costs (`[exec]`'s pace and
    /// `[levels]`: `levels::init_portals`), the stat policy (`[lod] policy`: `World::reload_names`) and the
    /// world's memos.
    pub const BUILD_TIME: &'static [&'static str] = &[
        "world",
        "scale",
        "districts",
        "trades",
        "faith.creeds",
        "gray.gang_types",
        "levers",
        "policy",
        "ambient",
        "transit",
        "perf",
        "gangs.names",
        "gangs.splinter_names",
        "gangs.treasury_initial",
        "corps.names",
        "corps.niches",
        "corps.farms",
        "corps.markets",
        "corps.blocks",
        "corps.offices",
        "corps.treasury_initial",
        "corps.fab_rows",
        "corps.noodle_first_row",
        "corps.seed_labs",
        "corps.seed_feeds",
        "tech.seed",
        "governance.boards",
        "outside.parents",
        "view.tile_m",
        "view.north_m",
    ];

    /// Whether the dotted config path `path` (`"exec.walk_tiles_per_tick"`) is build-time ([`Config::BUILD_TIME`]):
    /// it is one of the list's paths or lies under one.
    pub fn is_build_time(path: &str) -> bool {
        Config::BUILD_TIME.iter().any(|b| path == *b || path.strip_prefix(b).is_some_and(|r| r.starts_with('.')))
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
        // M18 W: a `[[work_beats]]` role is a trade id, a Role's name, "exec" or "*".
        for r in self.work_beats.iter().flat_map(|b| b.jobs.iter()) {
            let trade = self.trades.iter().any(|t| &t.id == r);
            let legacy = Role::ALL.iter().any(|x| format!("{x:?}") == *r);
            if !trade && !legacy && r != "exec" && r != "*" {
                out.push(format!("[[work_beats]] job {r:?} is neither a trade id nor a Role"));
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

/// J9 review: [`Config`]'s cache of the trade roles (row order) and of
/// each kind's `(role, staff_per_floor)` trades.
#[derive(Clone, Debug, Default)]
pub struct TradeCache(std::sync::OnceLock<TradeCacheData>);

#[derive(Clone, Debug)]
pub struct TradeCacheData {
    roles: Vec<Role>,
    by_kind: std::collections::BTreeMap<BuildingKind, Vec<(Role, usize)>>,
}

/// The test tiers' thread switch: overrides `[perf] threads` in
/// [`Config::load_from`] (set by `tools/test.py`; docs/TESTING.md).
pub const THREADS_ENV: &str = "CITYSIM_THREADS";

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
        let mut cfg = Config::load_shipped_from(assets_dir);
        // The test tiers' thread switch (hygiene 2026-11-02, docs/TESTING.md
        // § The tiers): `tools/test.py` sets `CITYSIM_THREADS=1` for the
        // tests it runs, so a 2,000 city scores its think slice on the
        // calling thread (the same city on any thread; the shipped pool cost
        // twice the CPU at 2k). Unset, `[perf] threads` as written.
        if let Ok(v) = std::env::var(THREADS_ENV) {
            cfg.perf.threads = v
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|&t| t >= 1)
                .unwrap_or_else(|| panic!("{THREADS_ENV}={v:?}: a thread count of 1 or more"));
        }
        cfg
    }

    /// [`Config::load`] without the tiers' `CITYSIM_THREADS`: `[perf]
    /// threads` as `config.toml` ships it (the one change-tier test that
    /// keeps the think pool exercised, `careers.rs`, and `core_year`).
    pub fn load_shipped() -> Config {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_shipped_from(&dir)
    }

    /// `config.toml` under `assets_dir`, as written (no env switch).
    pub fn load_shipped_from(assets_dir: &Path) -> Config {
        let path = assets_dir.join("config.toml");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        Config::from_toml(&text, assets_dir).unwrap_or_else(|e| panic!("bad config {}: {e}", path.display()))
    }

    /// `config.toml`'s text parsed as [`Config::load_from`] reads it (the
    /// switches' strips included), its assets at `assets_dir`; a parse error
    /// or a broken bound is an `Err` (the view's reload, OPEN-262, keeps the
    /// old config on one). Not fitted or scaled: the world's build does that.
    pub fn from_toml(text: &str, assets_dir: &Path) -> Result<Config, String> {
        // one line (the view's log and HUD): the message and where
        let mut cfg: Config = toml::from_str(text).map_err(|e| match e.span() {
            Some(s) => {
                let before = &text[..s.start.min(text.len())];
                let col = before.len() - before.rfind('\n').map_or(0, |i| i + 1) + 1;
                format!("line {}, column {col}: {}", before.matches('\n').count() + 1, e.message().trim())
            }
            None => e.message().trim().to_string(),
        })?;
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
        if u64::from(cfg.perf.stagger_midnight) * u64::from(crate::systems::midnight::LAST_SLOT)
            >= crate::time::TICKS_PER_HOUR
        {
            return Err(format!(
                "[perf] stagger_midnight {} puts the last midnight slot past 00:59",
                cfg.perf.stagger_midnight
            ));
        }
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
        Ok(cfg)
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

    /// The view milestone V1 (OPEN-262): the keys a save's `Trade("key")` resolves against set back to these
    /// trades (a parse of another file set them to its rows: `trades_de`).
    pub fn set_trade_keys(&self) {
        crate::components::TradeId::set_keys(trade_ids_of(&self.trades));
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
}
