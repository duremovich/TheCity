//! Player commands and stored lever values. Every command is applied at the
//! start of the next tick and recorded with its tick, so a save plus its
//! command log replays deterministically.

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Corp, CorpOrder, CorpShock, Crime, DeathCause, Gang, Household, Niche, Position,
    Posture, Rect, Sentence, TileKind, TilePos, Wallet,
};
use crate::config::Config;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::world::World;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlayerCommand {
    /// Attribute an open hole now (the inspector's "find out"). A missing
    /// hole does nothing. Logged, so a replay binds at the same tick.
    Bind(crate::components::HoleId),
    ReleaseReserve {
        amount: u32,
    },
    SetTaxRate(f32),
    SetSentenceMult(f32),
    SetGuardCount(u8),
    SetImmigrationPerWeek(u8),
    GrantCoins {
        agent: EntityId,
        amount: i64,
    },
    Arrest(EntityId),
    Release(EntityId),
    DemolishHome(EntityId),
    BuildHome {
        rect: Rect,
    },
    Pin(EntityId, bool),
    /// Issued by the app whenever the view rect changes; makes LOD replayable.
    SetView(Option<Rect>),
    /// `0..=10`, default 3.
    SetDolePerDay(u8),
    /// App-only; logged so replays know the speed, no sim effect.
    SetSpeed(Speed),
    /// M9: pin the law's posture, or (`None`) hand it back to the captain.
    SetLawPosture(Option<Posture>),
    /// M12 D42: a district's allocation weight multiplier (`0.0..=5.0`; 0 withdraws the law).
    SetGuardWeight {
        district: crate::components::DistrictId,
        weight: f32,
    },
    /// M12 D42: pin a district's stance, or (`None`) hand it back to the captain.
    SetStance {
        district: crate::components::DistrictId,
        stance: Option<crate::components::Stance>,
    },
    // --- God commands (docs/VISION.md "How we test: god scenarios"). They
    // break the world's rules on purpose: money from nowhere, death without a
    // killer, sentences without a crime. Logged and replayed like the rest.
    /// Kill an agent: cause Violence, no killer.
    KillAgent(EntityId),
    /// Jail an agent for `days` (no crime, no report, no capacity check); a
    /// prisoner's sentence is extended to at least that.
    JailAgent {
        who: EntityId,
        days: u32,
    },
    /// Free a prisoner (as `Release`, but named for the god set).
    FreeAgent(EntityId),
    /// Add coins to a gang's treasury, out of thin air.
    FundGang {
        gang: EntityId,
        amount: i64,
    },
    /// A gang's treasury moves into the city Treasury.
    SeizeGangTreasury(EntityId),
    /// Kill every living member of a gang.
    KillGang(EntityId),
    /// Jail every member of a gang for `days` (extending sentences already running).
    JailGang {
        gang: EntityId,
        days: u32,
    },
    /// Dismiss every guard; `guard_count` is untouched, so the law re-hires.
    FireAllGuards,
    /// Set the city Treasury to exactly this many coins (may be negative).
    SetTreasury(i64),
    // --- M11 levers (docs/M11_OWNERSHIP.md § 8) ---
    /// Rent on city-owned Blocks per tier (Sump, Mid, Spire), each `0..=20`.
    SetCityRent([i64; 3]),
    /// A ceiling on every Block's rent; `None` lifts it.
    SetRentCap(Option<i64>),
    /// The city buys a building at its value from its owner.
    Nationalise(EntityId),
    /// Treasury -> a corp's treasury.
    Subsidise {
        corp: EntityId,
        amount: i64,
    },
    /// The city never evicts while set.
    NoCityEvictions(bool),
    /// Split a corp's monopoly niche in two (D31); refused without one.
    BreakUp(EntityId),
    // --- M11 god commands (docs/GOD_SCENARIOS_V2.md): corps, out of the rules.
    /// Add coins to a corp's treasury, out of thin air.
    FundCorp {
        corp: EntityId,
        amount: i64,
    },
    /// The corp's treasury goes to -1, `negative_since` is back-dated past
    /// `bankrupt_days`, and it goes bankrupt at once (`corps::bankrupt`), so
    /// a day's takings cannot save it before the midnight pass.
    BankruptCorp(EntityId),
    /// Every building of `owner` (`None` = the city) moves to `to` (an
    /// agent, gang, corp or `None` = the city). A corp losing buildings takes
    /// one `BuildingLost`, as when a rival's Acquire takes one.
    SeizeBuildings {
        owner: Option<EntityId>,
        to: Option<EntityId>,
    },
    /// Kill a corp's exec (Violence, no killer).
    KillExec(EntityId),
    /// Kill every non-exec employee of a corp (Violence, no killer).
    KillStaff(EntityId),
    /// Pin a corp's order for `days` (the daily and shock rescores keep the
    /// trace but do not switch). `niche` defaults to the order's current
    /// niche, else the corp's first.
    SetCorpOrder {
        corp: EntityId,
        order: CorpOrder,
        #[serde(default)]
        niche: Option<Niche>,
        days: u32,
    },
    /// The corp's non-exec workers walk out of their next shift, as a Street
    /// strike does (D35), whatever the unrest; the strike cooldown is untouched.
    StrikeNow(EntityId),
    /// Every corp's treasury to 0.
    WipeTreasuries,
    // --- M12 levers (docs/M12_DISTRICTS.md § 7, plan D42) ---
    /// The city's Sanitation headcount (`0..=60`).
    SetSanitation(u8),
    /// Jobs v2 J17 (P5a): release a `SetGuardCount`/`SetSanitation` pin back
    /// to the civic budget (CLI `civic=auto`; with `no_safety_net` only).
    SetCivicAuto,
    /// A district's sweeper weight multiplier (`0.0..=5.0`; 0 sends no sweepers).
    SetSanitationWeight {
        district: crate::components::DistrictId,
        weight: f32,
    },
    /// A district curfew: Vagrancy × `curfew_mult`, submission + `curfew_fear`,
    /// residents' happiness − 0.05 in the aggregate.
    SetCurfew {
        district: crate::components::DistrictId,
        on: bool,
    },
    /// Pin the law's riot response, or (`None`) hand it back to the captain.
    SetRiotResponse(Option<crate::components::RiotResponse>),
    // --- M12 god commands (docs/GOD_SCENARIOS_V3.md): districts, out of the rules.
    /// A riot in the district at the next muster hour, unrest, streak,
    /// cooldown and caps ignored (one eligible rioter is enough).
    Riot(crate::components::DistrictId),
    /// Every street tile of the district to `round(level × 254)`.
    Litter {
        district: crate::components::DistrictId,
        level: f32,
    },
    /// Split a gang by the D36 rule minus the roll and the character tests
    /// (lieutenant strength, loyalty); refused with the reason when the
    /// structure fails (no lieutenant, one held district, the cap, no site).
    SplitGang(EntityId),
    /// A Block, Bar or Hotel goes derelict now.
    Derelict(EntityId),
    /// `buyer` (an agent, gang or corp; `None` = the city) buys `building`
    /// for `price` (paid to the owner, from nowhere when the city sells to
    /// itself); a derelict is restored to the buyer.
    BuyBuilding {
        buyer: Option<EntityId>,
        building: EntityId,
        price: i64,
    },
    // --- M13 god commands (plan D48; phase 1 for tests, the CLI in phase 5).
    /// Give an agent a new asset, free and without import (`assets::grant`).
    GrantAsset {
        agent: EntityId,
        kind: crate::components::AssetKind,
        tier: u8,
    },
    /// Wreck an asset now (an implant fails instead).
    Wreck(EntityId),
    /// M13 D40/D48 (phase 4): Markets sell Stims legally, or stop.
    SetStimsLegal(bool),
    /// M13 D48: an extra share of upkeep to the Treasury for one asset
    /// class (`0.0..=5.0`; upkeep is charged x `1 + rate`).
    SetAssetTax {
        kind: crate::components::AssetClass,
        rate: f32,
    },
    /// M13 D12/D48: whether the city impounds vehicles with unpaid upkeep.
    SetImpound(bool),
    // --- M13 god commands (plan D48, docs/GOD_SCENARIOS_V4.md).
    /// Every living adult gets Arms and Nerves implants of `tier` (a slot
    /// already filled is kept), free and without import.
    ChromeEveryone {
        tier: u8,
    },
    /// `n` Stims into the Hideout stock of every gang dealing in, holding
    /// or hiding in the district (capped at the goods cap).
    FloodStims {
        district: crate::components::DistrictId,
        n: u32,
    },
    /// Every implant this lender (agent, gang or corp) financed is bricked
    /// at once: the loan is called (arrears to `repo_days`), so it stays
    /// bricked until the debtor catches up or the lender takes it back.
    Brick(EntityId),
    /// Pin `chase` on the agent's next trip (D25; consumed at its end).
    Chase(EntityId),
    // --- M14 god commands (plan V42; phase 1: the tests' and the gate's).
    /// Wipe the Data on a building's node (`tech::wipe_store`).
    WipeData(EntityId),
    /// Set a corp's tier in a track (through `tech::gain_tier`/`lose_tier`).
    SetTech {
        corp: EntityId,
        track: crate::virt::Track,
        tier: u8,
    },
    /// Put Data into a faction's first Lab, else its Hideout's node.
    GrantData {
        faction: EntityId,
        track: crate::virt::Track,
        units: u32,
    },
    // --- M14 levers and god commands (plan V42, phase 4).
    /// The Treasury's and the Precinct's ICE tier (`0..=3`), installed or
    /// lowered until both match; the upkeep is the Treasury's.
    SetCityIce(u8),
    /// An extra share (`0.0..=1.0`) of every Data sale paid to the Treasury.
    SetDataTax(f32),
    /// The sentence for Intrusion or Data Theft, in days (replaces the
    /// config's base; the sentence multiplier still applies).
    SetHackSentence {
        crime: Crime,
        days: u16,
    },
    /// Give an agent a Deck of `tier`, free (`assets::grant`).
    GrantDeck {
        agent: EntityId,
        tier: u8,
    },
    /// A building node's ICE to `tier` (`0..=3`), the city's own, free.
    SetIce {
        building: EntityId,
        tier: u8,
    },
    /// A jacked-in agent is fried at once, at command time: a lost contest
    /// at its next contested node with no save (`virt::god_fry`); the kill
    /// roll uses `p_flatline` only at ICE 3, else `p_fry_kill`.
    Fry(EntityId),
    /// Pin a run for an agent with a deck: `target` is a building (a Lab,
    /// a Hideout, a robot's or camera's building) or, for `Purpose::Ledger`,
    /// a building or corp whose owner's Ledger is the target. The contest
    /// rules apply.
    RunNow {
        agent: EntityId,
        target: EntityId,
        purpose: crate::virt::Purpose,
    },
    // --- M14 god commands (plan V42, phase 5: the god scenarios' plurals).
    /// Wipe the Data on every node the corp owns (`tech::wipe_store` each,
    /// ascending; the wipe rule drops a tier when no backup is left).
    WipeCorpData(EntityId),
    /// `GrantDeck` to the `n` adults living in the district with the best
    /// hacking (ties the lower id) who carry no deck.
    GrantDecks {
        district: crate::components::DistrictId,
        n: u16,
        tier: u8,
    },
    /// `SetIce` on every node the corp owns (building nodes and its
    /// Ledger), the city's own (maker `None`), free.
    SetCorpIce {
        corp: EntityId,
        tier: u8,
    },
    // --- M15 god commands (plan W42, phase 1).
    /// A deed is talked about: a pool entry at `reach` in `district`, plus
    /// one first-hand holder (a heard entry at hops 0, conf 1) on the living
    /// adult of that district nearest `about`.
    PlantRumour {
        about: EntityId,
        deed: crate::word::Deed,
        object: Option<EntityId>,
        district: crate::components::DistrictId,
        reach: f32,
    },
    /// Pin one reputation axis of an agent or faction for `days` (the
    /// daily rebuild leaves a pin alone until it runs out).
    SetReputation {
        who: EntityId,
        axis: crate::word::Axis,
        value: f32,
        days: u16,
    },
    // --- M15 god commands (plan W42, phase 2).
    /// Set one social skill of an agent; `suit` also dresses them at 3
    /// (the Spire suit), pinned so the daily appearance pass keeps it.
    GrantSkill {
        agent: EntityId,
        skill: crate::word::SocialSkill,
        value: f32,
        #[serde(default)]
        suit: bool,
    },
    /// Set or clear a gang's creed; a Purist gang expels its members above
    /// `creed_tolerance` at once.
    SetCreed {
        gang: EntityId,
        creed: Option<crate::word::Creed>,
    },
    // --- M15 god commands (plan W42, phase 3).
    /// Both factions' leaders (a gang's leader, a corp's exec, the captain)
    /// hold a grudge of `weight` on the other faction; the vendettas are
    /// rescored at once.
    DeclareVendetta {
        a: EntityId,
        b: EntityId,
        weight: f32,
    },
    /// The highest-affinity living Friend of `of` is killed by `by`, with
    /// one witness picked as the binder picks one, so the killing is named.
    KillFriend {
        of: EntityId,
        by: EntityId,
    },
    /// `hunter` holds a 1.0 grudge on `target` and hunts it now (past `max_hunts`).
    Hunt {
        hunter: EntityId,
        target: EntityId,
    },
    // --- M15 levers (plan W42, phase 4).
    /// Off: only the city's Civic Wire publishes; private Feeds keep their
    /// staff and lose their ads.
    SetPressLicence(bool),
    /// An extra share (`0.0..=1.0`) of every ad and Spin payment to a Feed,
    /// paid to the Treasury.
    SetNewsTax(f32),
    /// The Civic Wire buries every story about a faction (a gang, a corp
    /// or the Law) while `on`.
    CensorStories {
        faction: EntityId,
        on: bool,
    },
    // --- Life pass L2 phase 1 (plan L37).
    /// The budget band's hiring arm (public works) on or off.
    SetPublicWorks(bool),
    /// The World account buys (or stops buying) exports.
    SetExport(bool),
    /// God: a venue (or a Fab) on the Lot nearest the district's centroid,
    /// else a refitted derelict there, free; a gang owner makes it a front.
    OpenVenue {
        kind: BuildingKind,
        district: crate::components::DistrictId,
        owner: Option<EntityId>,
    },
    /// God: the World's price per unit of a good.
    SetExportPrice {
        good: crate::outside::ExportGood,
        price: i64,
    },
    /// God: fill every open vacancy at buildings of a kind now.
    HireAll {
        kind: BuildingKind,
    },
    // --- Life pass L2 phase 2 (plan L37).
    /// An extra tax on `Flow::Leisure` and `Flow::Gamble` (a fraction).
    SetLeisureTax(f32),
    /// Dens close and FightPit bets stop.
    BanGambling(bool),
    /// God: every leisure venue in a district closed for `days`.
    CloseLeisure {
        district: crate::components::DistrictId,
        days: u32,
    },
    /// God: every adult's `fun` in a district set to `value`.
    SetFun {
        district: crate::components::DistrictId,
        value: f32,
    },
    // --- Life pass L2 phase 4 (plan L37).
    /// God: the gang's order pinned to Contest for `days`, the district
    /// touched by its faction violence off screen until then.
    FactionStrike {
        gang: EntityId,
        district: crate::components::DistrictId,
        days: u64,
    },
    // --- M16a phase 1 (plan C39): god contract records (a game abstraction:
    // a record with a price, matched and resolved by the board's rules).
    /// God: post a record (`Origin::God`); the price is escrowed (a direct
    /// record checks the buyer's purse); `brokered` picks the open Fixer
    /// nearest the target.
    PostContract {
        buyer: Option<EntityId>,
        kind: crate::contract::ContractKind,
        target: crate::contract::Target,
        price: i64,
        brokered: bool,
        deadline_days: u16,
    },
    /// God: force a taker's acceptance of an open record (M18's player
    /// `TakeContract` is the same call, `contracts::accept`).
    TakeContract {
        contract: crate::contract::ContractId,
        taker: EntityId,
    },
    // --- The Real economy phase 1 (plan E47): the World's levers.
    /// God: pin a good's appetite (`None` releases it to the walk).
    SetAppetite {
        good: crate::outside::ExportGood,
        mult: Option<f32>,
    },
    /// God: pin a good's daily cap (`None` releases it to `cap × appetite`).
    SetExportCap {
        good: crate::outside::ExportGood,
        cap: Option<u32>,
    },
    /// The famine lever: a good's ask multiplier (lower: cheaper imports).
    SetImportAsk {
        good: crate::outside::ExportGood,
        mult: f32,
    },
    /// The customs rate on imports (over `[world_market] customs_rate`).
    SetCustoms(f32),
    /// God: the World neither buys nor sells (`SetExport` closes buying only).
    CloseWorld(bool),
    /// Real economy E28, E47: a gift into a Mission's purse. With no player
    /// body yet (M18) the god's gift is an outside donor's (`cross_in` from
    /// the World account), so the coin identity holds.
    Donate {
        mission: EntityId,
        amount: i64,
    },
    /// Real economy E21, E47 (phase 3a): release a `SetTaxRate` pin back to
    /// the Treasury's tax band (CLI `tax=auto`).
    SetTaxAuto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speed {
    X1,
    X2,
    X4,
    X8,
    X32,
    X128,
    X1000,
}

impl Speed {
    pub const ALL: [Speed; 7] = [Speed::X1, Speed::X2, Speed::X4, Speed::X8, Speed::X32, Speed::X128, Speed::X1000];

    pub fn multiplier(self) -> u32 {
        match self {
            Speed::X1 => 1,
            Speed::X2 => 2,
            Speed::X4 => 4,
            Speed::X8 => 8,
            Speed::X32 => 32,
            Speed::X128 => 128,
            Speed::X1000 => 1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Levers {
    /// `0.0..=0.3`, default 0.05.
    pub tax_rate: f32,
    /// `0.5..=3.0`, default 1.0.
    pub sentence_mult: f32,
    /// `0..=60`, default 36 (M10, 2,000 residents; v1 `0..=30`, 10).
    pub guard_count: u8,
    /// `0..=30`, default 13 (M10; v1 `0..=10`, 2).
    pub immigration_per_week: u8,
    /// `0..=10`, default 3.
    pub dole_per_day: u8,
    /// M11: rent on city-owned Blocks per tier (initially `[rent] base`).
    #[serde(default)]
    pub city_rent: [i64; 3],
    /// M11: a ceiling on any Block's rent.
    #[serde(default)]
    pub rent_cap: Option<i64>,
    /// M11: the city never evicts.
    #[serde(default)]
    pub no_city_evictions: bool,
    /// M12 D10: a multiplier on each district's allocation weight (1.0; 0
    /// abandons the district: no guards, stance Withdrawn).
    #[serde(default = "default_guard_weight")]
    pub guard_weight: [f32; crate::components::MAX_DISTRICTS],
    /// M12 D12: the player's stance pin per district; `None` = the captain.
    #[serde(default)]
    pub stance_pin: [Option<crate::components::Stance>; crate::components::MAX_DISTRICTS],
    /// M12 (curfew lands with the levers in phase 5; Vagrancy reads it now).
    #[serde(default)]
    pub curfew: [bool; crate::components::MAX_DISTRICTS],
    /// M12 D23: city Sanitation workers (`[levers] sanitation_count`).
    #[serde(default)]
    pub sanitation_count: u8,
    /// M12 D23: a multiplier on each district's sweeper weight (1.0).
    #[serde(default = "default_guard_weight")]
    pub sanitation_weight: [f32; crate::components::MAX_DISTRICTS],
    /// M12 D42: the player's riot-response pin; `None` = the captain (D34).
    #[serde(default)]
    pub riot_response: Option<crate::components::RiotResponse>,
    /// M13 D40: Markets restock and sell Stims legally.
    #[serde(default)]
    pub stims_legal: bool,
    /// M13 D12: the city impounds vehicles with unpaid upkeep.
    #[serde(default = "default_true")]
    pub impound: bool,
    /// M13 D48: upkeep is charged × `1 + rate`, per `AssetClass` (M14 V36:
    /// ten classes with Deck and Camera; a save with eight reads padded).
    #[serde(default, deserialize_with = "asset_tax_any_len")]
    pub asset_tax: [f32; crate::components::AssetClass::ALL.len()],
    /// M14 V42: the city's own nodes' ICE (`[levers] city_ice`).
    #[serde(default = "default_city_ice")]
    pub city_ice: u8,
    /// M14 V42: an extra share of every Data sale to the Treasury.
    #[serde(default)]
    pub data_tax: f32,
    /// M14 V42: the sentence in days for `[Intrusion, DataTheft]`; `None`
    /// reads `[crime] sentence_days_ext`.
    #[serde(default)]
    pub hack_sentence_days: [Option<u16>; 2],
    /// M15 W42: Feeds may publish (off: only the Civic Wire).
    #[serde(default = "default_true")]
    pub press_licence: bool,
    /// M15 W42: an extra share of ads and Spin payments to the Treasury.
    #[serde(default)]
    pub news_tax: f32,
    /// M15 W42: the factions the Civic Wire buries (`CensorStories`).
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub censored: std::collections::BTreeSet<EntityId>,
    /// L2 L37: the band may post public works (`SetPublicWorks`).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub public_works: bool,
    /// L2 L37: the World account buys (`[export] enabled` at seed, `SetExport`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub export_open: bool,
    /// L2 L37: the extra tax on leisure and gambling (`SetLeisureTax`).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub leisure_tax: f32,
    /// L2 L37: gambling banned (`BanGambling`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ban_gambling: bool,
    /// M16a (plan C39; `SetFixerLicence`, phase 4): Fixers are licensed.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub fixer_licence: bool,
    /// M16a (plan C39; `SetAccessoryMult`, phase 4): overrides `[law] accessory_mult`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory_mult: Option<f32>,
    /// M16a (plan C39; `PublicBounties`, phase 4): the law posts a Locate
    /// on every wanted suspect after one day.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub public_bounties: bool,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn is_true(v: &bool) -> bool {
    *v
}

fn default_city_ice() -> u8 {
    2
}

/// M14 V36: `asset_tax` from a save written with fewer classes (M13's
/// eight): the missing classes read 0.
fn asset_tax_any_len<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<[f32; crate::components::AssetClass::ALL.len()], D::Error> {
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = [f32; crate::components::AssetClass::ALL.len()];
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a list of asset-class tax rates")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = [0.0; crate::components::AssetClass::ALL.len()];
            let mut i = 0;
            while let Some(x) = seq.next_element::<f32>()? {
                if let Some(slot) = out.get_mut(i) {
                    *slot = x;
                }
                i += 1;
            }
            Ok(out)
        }
    }
    d.deserialize_tuple(crate::components::AssetClass::ALL.len(), V)
}

fn default_true() -> bool {
    true
}

fn default_guard_weight() -> [f32; crate::components::MAX_DISTRICTS] {
    [1.0; crate::components::MAX_DISTRICTS]
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.overlaps(&b)
}

impl Levers {
    pub fn from_config(cfg: &Config) -> Levers {
        Levers {
            tax_rate: cfg.levers.tax_rate,
            sentence_mult: cfg.levers.sentence_mult,
            guard_count: cfg.levers.guard_count,
            immigration_per_week: cfg.levers.immigration_per_week,
            dole_per_day: cfg.levers.dole_per_day,
            city_rent: cfg.rent.base,
            rent_cap: None,
            no_city_evictions: false,
            guard_weight: default_guard_weight(),
            stance_pin: [None; crate::components::MAX_DISTRICTS],
            curfew: [false; crate::components::MAX_DISTRICTS],
            sanitation_count: cfg.levers.sanitation_count,
            sanitation_weight: default_guard_weight(),
            riot_response: None,
            stims_legal: cfg.levers.stims_legal,
            impound: true,
            asset_tax: [0.0; crate::components::AssetClass::ALL.len()],
            city_ice: cfg.levers.city_ice,
            data_tax: 0.0,
            hack_sentence_days: [None; 2],
            press_licence: true,
            news_tax: 0.0,
            censored: Default::default(),
            // Real economy E22 (b) (phase 3a): no public works with `no_net`
            // (`budget::daily` does not run either: `treasury::daily` replaces it).
            public_works: !crate::systems::econ::no_net_cfg(cfg),
            // Real economy (plan E4, E12): the World market buys from seed;
            // `SetExport` closes and reopens it.
            export_open: true,
            leisure_tax: 0.0,
            ban_gambling: false,
            fixer_licence: true,
            accessory_mult: None,
            public_bounties: false,
        }
    }
}

impl World {
    /// A lever's outcome as an event (Real economy phase 1).
    fn lever_result(&mut self, r: Result<String, String>) {
        match r {
            Ok(text) => self.push_event(EventKind::PlayerAction, &[], text),
            Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], e),
        };
    }

    /// Queue a command for the start of the next tick.
    pub fn push_command(&mut self, cmd: PlayerCommand) {
        self.command_queue.push(cmd);
    }

    /// Move every command out of `cmds` into the queue, in order.
    pub fn push_commands(&mut self, cmds: &mut Vec<PlayerCommand>) {
        self.command_queue.append(cmds);
    }

    /// Drain the queue, apply each command in order, log it.
    pub fn apply_commands(&mut self) {
        let queue = std::mem::take(&mut self.command_queue);
        for cmd in queue {
            self.apply_command(&cmd);
            self.command_log.push((self.tick, cmd));
        }
    }

    fn apply_command(&mut self, cmd: &PlayerCommand) {
        match cmd {
            PlayerCommand::Bind(hole) => {
                crate::systems::bind::bind(self, *hole);
            }
            PlayerCommand::ReleaseReserve { amount } => self.cmd_release_reserve(*amount),
            PlayerCommand::SetTaxRate(r) => {
                self.levers.tax_rate = r.clamp(0.0, 0.3);
                // Real economy E21: a set rate pins against the band (written
                // only while the band runs: the state is the milestone's).
                if crate::systems::treasury::on(self) {
                    self.econ.tax_pinned = true;
                }
                self.push_event(EventKind::PlayerAction, &[], format!("Tax rate set to {:.2}", self.levers.tax_rate));
            }
            PlayerCommand::SetSentenceMult(m) => {
                self.levers.sentence_mult = m.clamp(0.5, 3.0);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Sentence multiplier set to {:.2}", self.levers.sentence_mult),
                );
            }
            PlayerCommand::SetGuardCount(n) => {
                self.levers.guard_count = (*n).min(60);
                // Jobs v2 J17: a set headcount pins against the civic budget.
                if crate::systems::treasury::on(self) {
                    self.econ.civic_pinned = true;
                }
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Guard count set to {}", self.levers.guard_count),
                );
            }
            PlayerCommand::SetImmigrationPerWeek(n) => {
                self.levers.immigration_per_week = (*n).min(30);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Immigration set to {} per week", self.levers.immigration_per_week),
                );
            }
            PlayerCommand::SetDolePerDay(n) => {
                self.levers.dole_per_day = (*n).min(10);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Dole set to {} per day", self.levers.dole_per_day),
                );
            }
            PlayerCommand::GrantCoins { agent, amount } => self.cmd_grant_coins(*agent, *amount),
            PlayerCommand::Pin(agent, pinned) => {
                if let Some(brain) = self.comp_mut::<Brain>(*agent) {
                    brain.pinned = *pinned;
                    let text = if *pinned { "Pinned" } else { "Unpinned" };
                    self.push_event(EventKind::PlayerAction, &[*agent], format!("{text} {}", self.name_of(*agent)));
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[*agent], "Pin: no such agent");
                }
            }
            PlayerCommand::SetView(rect) => {
                self.view_rect = *rect;
            }
            PlayerCommand::SetSpeed(_) => {}
            PlayerCommand::SetLawPosture(p) => {
                if let Some(l) = self.law_mut() {
                    l.pinned = *p;
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetLawPosture: no Precinct");
                    return;
                }
                crate::systems::law_brain::rescore(self, 0.0, "pinned");
                let text = match p {
                    Some(p) => format!("Law posture pinned to {p}"),
                    None => "Law posture handed back to the captain".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetGuardWeight { district, weight } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetGuardWeight: no such district");
                    return;
                }
                self.levers.guard_weight[i] = weight.clamp(0.0, 5.0);
                let text =
                    format!("Guard weight in {} set to {:.2}", self.district_name(*district), weight.clamp(0.0, 5.0));
                self.push_event(EventKind::PlayerAction, &[], text);
                crate::systems::law_brain::redeal(self, "lever");
            }
            PlayerCommand::SetStance { district, stance } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetStance: no such district");
                    return;
                }
                if let Some(crate::components::Stance::Crackdown(g)) = stance {
                    if !self.has::<Gang>(*g) {
                        self.push_event(EventKind::PlayerActionFailed, &[], "SetStance: no such gang");
                        return;
                    }
                }
                self.levers.stance_pin[i] = *stance;
                let name = self.district_name(*district).to_string();
                let text = match stance {
                    Some(s) => {
                        format!("Stance in {name} pinned to {}", crate::systems::law_brain::stance_label(self, *s))
                    }
                    None => format!("Stance in {name} handed back to the captain"),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
                crate::systems::law_brain::redeal(self, "pinned");
            }
            PlayerCommand::SetSanitation(n) => {
                self.levers.sanitation_count = (*n).min(60);
                // Jobs v2 J17: a set headcount pins against the civic budget.
                if crate::systems::treasury::on(self) {
                    self.econ.civic_pinned = true;
                }
                let text = format!("Sanitation headcount set to {}", self.levers.sanitation_count);
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetCivicAuto => {
                let text = if crate::systems::treasury::on(self) {
                    self.econ.civic_pinned = false;
                    Ok(format!(
                        "Civic headcounts released to the budget (now {} guards, {} sweepers)",
                        self.levers.guard_count, self.levers.sanitation_count
                    ))
                } else {
                    Err("SetCivicAuto: no civic budget (no_safety_net is off)".to_string())
                };
                self.lever_result(text);
            }
            PlayerCommand::SetSanitationWeight { district, weight } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetSanitationWeight: no such district");
                    return;
                }
                let w = weight.clamp(0.0, 5.0);
                self.levers.sanitation_weight[i] = w;
                let text = format!("Sanitation weight in {} set to {w:.2}", self.district_name(*district));
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetCurfew { district, on } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetCurfew: no such district");
                    return;
                }
                self.levers.curfew[i] = *on;
                self.districts[i].curfew = *on;
                let name = self.district_name(*district).to_string();
                let text = if *on { format!("Curfew in {name}") } else { format!("Curfew in {name} lifted") };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetStimsLegal(on) => {
                self.levers.stims_legal = *on;
                let text = if *on { "Stims legal: Markets stock and sell them" } else { "Stims banned again" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetAssetTax { kind, rate } => {
                let r = rate.clamp(0.0, 5.0);
                if let Some(t) = self.levers.asset_tax.get_mut(kind.index()) {
                    *t = r;
                }
                self.push_event(EventKind::PlayerAction, &[], format!("Asset tax on {kind:?} set to {r:.2}"));
            }
            PlayerCommand::SetImpound(on) => {
                self.levers.impound = *on;
                let text = if *on { "The city impounds unregistered vehicles" } else { "The city stops impounding" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetCityIce(n) => {
                self.levers.city_ice = (*n).min(3);
                crate::systems::virt::set_city_ice(self);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("City ICE set to {} (Treasury and Precinct)", self.levers.city_ice),
                );
            }
            PlayerCommand::SetDataTax(r) => {
                self.levers.data_tax = r.clamp(0.0, 1.0);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Data tax set to {:.0} %", self.levers.data_tax * 100.0),
                );
            }
            PlayerCommand::SetHackSentence { crime, days } => {
                let slot = match crime {
                    Crime::Intrusion => 0,
                    Crime::DataTheft => 1,
                    _ => {
                        self.push_event(
                            EventKind::PlayerActionFailed,
                            &[],
                            "SetHackSentence: only Intrusion and Data Theft".to_string(),
                        );
                        return;
                    }
                };
                let d = (*days).clamp(1, 365);
                self.levers.hack_sentence_days[slot] = Some(d);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Sentence for {} set to {d} days", crime.label()),
                );
            }
            PlayerCommand::SetPressLicence(on) => {
                self.levers.press_licence = *on;
                let text = if *on {
                    "Press licence granted: every Feed publishes"
                } else {
                    "Press licence revoked: only the Civic Wire publishes"
                };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetNewsTax(r) => {
                self.levers.news_tax = r.clamp(0.0, 1.0);
                self.push_event(EventKind::PlayerAction, &[], format!("News tax set to {:.2}", self.levers.news_tax));
            }
            PlayerCommand::CensorStories { faction, on } => {
                let known = self.has::<Gang>(*faction)
                    || self.has::<Corp>(*faction)
                    || self.has::<crate::components::Law>(*faction);
                if !known {
                    self.push_event(EventKind::PlayerActionFailed, &[], "CensorStories: no such faction".to_string());
                    return;
                }
                let name = if self.has::<crate::components::Law>(*faction) {
                    "the Law".to_string()
                } else {
                    self.owner_label(Some(*faction))
                };
                let text = if *on {
                    self.levers.censored.insert(*faction);
                    format!("The Civic Wire buries every story about {name}")
                } else {
                    self.levers.censored.remove(faction);
                    format!("The Civic Wire may run stories about {name} again")
                };
                self.push_event(EventKind::PlayerAction, &[*faction], text);
            }
            PlayerCommand::SetLeisureTax(rate) => {
                self.levers.leisure_tax = rate.clamp(0.0, 1.0);
                let text = format!("Leisure tax set to {:.0}%", self.levers.leisure_tax * 100.0);
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::BanGambling(on) => {
                self.levers.ban_gambling = *on;
                let text = if *on { "Gambling banned" } else { "Gambling legal again" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::CloseLeisure { district, days } => {
                let until = self.tick + u64::from(*days) * crate::time::TICKS_PER_DAY;
                let mut n = 0;
                for kind in BuildingKind::LEISURE {
                    for b in self.buildings_of_kind(kind).to_vec() {
                        if self.district_of_building(b) != *district {
                            continue;
                        }
                        if let Some(bd) = self.comp_mut::<crate::components::Building>(b) {
                            bd.closed_until = Some(bd.closed_until.unwrap_or(0).max(until));
                            n += 1;
                        }
                    }
                }
                let text = format!("God closed {n} leisure venues in district {} for {days} days", district.index());
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetFun { district, value } => {
                let v = value.clamp(0.0, 1.0);
                // scan-ok: a god command
                for id in self.citizens() {
                    let home_d = self
                        .comp::<crate::components::Household>(id)
                        .and_then(|h| h.home)
                        .map(|h| self.district_of_building(h))
                        .or_else(|| self.comp::<crate::components::Position>(id).map(|p| self.district_of(p.tile)));
                    if home_d == Some(*district) {
                        if let Some(n) = self.comp_mut::<crate::components::Needs>(id) {
                            n.fun = v;
                        }
                    }
                }
                let text = format!("God set fun to {v:.2} in district {}", district.index());
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetPublicWorks(on) => {
                self.levers.public_works = *on;
                let text = if *on { "Public works open" } else { "Public works closed" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetExport(on) => {
                self.levers.export_open = *on;
                let text = if *on { "Exports open to the World" } else { "Exports closed" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetExportPrice { good, price } => {
                let price = (*price).max(0);
                self.outside.export.price.insert(*good, price);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Export price of {} set to {price}", good.label()),
                );
            }
            // Real economy phase 1 (plan E47).
            PlayerCommand::SetAppetite { good, mult } => {
                let text = match crate::systems::world_market::book_mut(self, *good) {
                    Some(b) => {
                        b.appetite_pin = mult.map(|m| m.max(0.0));
                        if let Some(m) = b.appetite_pin {
                            b.appetite = m;
                        }
                        Ok(match mult {
                            Some(m) => format!("World appetite for {} pinned at {m:.2}", good.label()),
                            None => format!("World appetite for {} released to the walk", good.label()),
                        })
                    }
                    None => Err("SetAppetite: no World book (the market is off)".to_string()),
                };
                self.lever_result(text);
            }
            PlayerCommand::SetExportCap { good, cap } => {
                let text = match crate::systems::world_market::book_mut(self, *good) {
                    Some(b) => {
                        b.cap_pin = *cap;
                        Ok(match cap {
                            Some(c) => format!("World daily cap for {} pinned at {c}", good.label()),
                            None => format!("World daily cap for {} released", good.label()),
                        })
                    }
                    None => Err("SetExportCap: no World book (the market is off)".to_string()),
                };
                self.lever_result(text);
            }
            PlayerCommand::SetImportAsk { good, mult } => {
                let mult = mult.max(0.0);
                let text = match crate::systems::world_market::book_mut(self, *good) {
                    Some(b) => {
                        b.ask_mult = mult;
                        Ok(format!("World ask multiplier for {} set to {mult:.2}", good.label()))
                    }
                    None => Err("SetImportAsk: no World book (the market is off)".to_string()),
                };
                self.lever_result(text);
            }
            PlayerCommand::SetCustoms(rate) => {
                let rate = rate.clamp(0.0, 1.0);
                self.econ.customs_pin = Some(rate);
                self.push_event(EventKind::PlayerAction, &[], format!("Customs rate set to {rate:.2}"));
            }
            PlayerCommand::SetTaxAuto => {
                let text = if crate::systems::treasury::on(self) {
                    self.econ.tax_pinned = false;
                    Ok(format!("Tax rate released to the band (now {:.2})", self.levers.tax_rate))
                } else {
                    Err("SetTaxAuto: no tax band (no_safety_net is off)".to_string())
                };
                self.lever_result(text);
            }
            PlayerCommand::CloseWorld(closed) => {
                self.econ.world_closed = *closed;
                let text =
                    if *closed { "The World is closed: no exports, no imports" } else { "The World is open again" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::OpenVenue { kind, district, owner } => {
                match crate::systems::jobs::open_venue(self, *kind, *district, *owner) {
                    Ok(b) => {
                        let text = format!("God opened {} ({})", self.name_of(b), kind.label());
                        self.push_event(EventKind::PlayerAction, &[b], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[], format!("OpenVenue: {e}"));
                    }
                }
            }
            PlayerCommand::HireAll { kind } => {
                let n = crate::systems::jobs::hire_all(self, *kind);
                self.push_event(EventKind::PlayerAction, &[], format!("God filled {n} {} vacancies", kind.label()));
            }
            PlayerCommand::FactionStrike { gang, district, days } => {
                if crate::systems::fviolence::faction_strike(self, *gang, *district, *days) {
                    let name = crate::systems::grudges::label(self, *gang);
                    let place = self.district_name(*district).to_string();
                    let text = format!("God sent {name} to strike {place} for {days} days");
                    self.push_event(EventKind::PlayerAction, &[*gang], text);
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[*gang], "FactionStrike: not a gang".to_string());
                }
            }
            PlayerCommand::PostContract { buyer, kind, target, price, brokered, deadline_days } => {
                match crate::systems::contracts::god_post(
                    self,
                    *buyer,
                    *kind,
                    *target,
                    *price,
                    *brokered,
                    *deadline_days,
                ) {
                    Ok(id) => {
                        let who = self.owner_label(*buyer);
                        let on = self.name_of(target.id());
                        let listed = self.contracts.get(&id).map_or(*price, |c| c.price);
                        let text = format!("God: {who} posted contract #{id} ({}) on {on} for {listed}", kind.label());
                        self.push_event(EventKind::PlayerAction, &[target.id()], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[], format!("PostContract: {e}"));
                    }
                }
            }
            PlayerCommand::Donate { mission, amount } => {
                match crate::systems::charity::god_donate(self, *mission, *amount) {
                    Ok(moved) => {
                        let text = format!("God gave {moved} to {}", self.name_of(*mission));
                        self.push_event(EventKind::PlayerAction, &[*mission], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[*mission], format!("Donate: {e}"));
                    }
                }
            }
            PlayerCommand::TakeContract { contract, taker } => {
                match crate::systems::contracts::god_take(self, *contract, *taker) {
                    Ok(()) => {
                        let text = format!("God: {} took contract #{contract}", self.owner_label(Some(*taker)));
                        self.push_event(EventKind::PlayerAction, &[*taker], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[*taker], format!("TakeContract: {e}"));
                    }
                }
            }
            PlayerCommand::SetRiotResponse(r) => {
                self.levers.riot_response = *r;
                let text = match r {
                    Some(r) => format!("Riot response pinned to {r:?}"),
                    None => "Riot response handed back to the captain".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::Arrest(who) => match crate::systems::law::player_arrest(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Arrested {name}"));
                }
                Err(e) => {
                    self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Arrest: {e}"));
                }
            },
            PlayerCommand::Release(who) => match crate::systems::law::player_release(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Released {name}"));
                }
                Err(e) => {
                    self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Release: {e}"));
                }
            },
            PlayerCommand::KillAgent(_)
            | PlayerCommand::JailAgent { .. }
            | PlayerCommand::FreeAgent(_)
            | PlayerCommand::FundGang { .. }
            | PlayerCommand::SeizeGangTreasury(_)
            | PlayerCommand::KillGang(_)
            | PlayerCommand::JailGang { .. }
            | PlayerCommand::FireAllGuards
            | PlayerCommand::SetTreasury(_)
            | PlayerCommand::FundCorp { .. }
            | PlayerCommand::BankruptCorp(_)
            | PlayerCommand::SeizeBuildings { .. }
            | PlayerCommand::KillExec(_)
            | PlayerCommand::KillStaff(_)
            | PlayerCommand::SetCorpOrder { .. }
            | PlayerCommand::StrikeNow(_)
            | PlayerCommand::WipeTreasuries
            | PlayerCommand::Riot(_)
            | PlayerCommand::Litter { .. }
            | PlayerCommand::SplitGang(_)
            | PlayerCommand::Derelict(_)
            | PlayerCommand::BuyBuilding { .. }
            | PlayerCommand::GrantAsset { .. }
            | PlayerCommand::Wreck(_)
            | PlayerCommand::ChromeEveryone { .. }
            | PlayerCommand::FloodStims { .. }
            | PlayerCommand::Brick(_)
            | PlayerCommand::Chase(_)
            | PlayerCommand::WipeData(_)
            | PlayerCommand::SetTech { .. }
            | PlayerCommand::GrantData { .. }
            | PlayerCommand::GrantDeck { .. }
            | PlayerCommand::SetIce { .. }
            | PlayerCommand::Fry(_)
            | PlayerCommand::RunNow { .. }
            | PlayerCommand::WipeCorpData(_)
            | PlayerCommand::GrantDecks { .. }
            | PlayerCommand::SetCorpIce { .. }
            | PlayerCommand::PlantRumour { .. }
            | PlayerCommand::SetReputation { .. }
            | PlayerCommand::GrantSkill { .. }
            | PlayerCommand::SetCreed { .. }
            | PlayerCommand::DeclareVendetta { .. }
            | PlayerCommand::KillFriend { .. }
            | PlayerCommand::Hunt { .. } => {
                let _ = match self.cmd_god(cmd) {
                    Ok((actors, text)) => self.push_event(EventKind::PlayerAction, &actors, format!("God: {text}")),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("God: {e}")),
                };
            }
            PlayerCommand::SetCityRent(rent) => {
                self.levers.city_rent = rent.map(|r| r.clamp(0, 20));
                let [a, b, c] = self.levers.city_rent;
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("City rent set to {a}/{b}/{c} per Block per day"),
                );
            }
            PlayerCommand::SetRentCap(cap) => {
                self.levers.rent_cap = cap.map(|c| c.max(0));
                let text = match self.levers.rent_cap {
                    Some(c) => format!("Rent capped at {c} per Block per day"),
                    None => "Rent cap lifted".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::NoCityEvictions(on) => {
                self.levers.no_city_evictions = *on;
                let text = if *on { "The city stops evicting" } else { "The city evicts again" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::Nationalise(b) => {
                let _ = match crate::systems::ownership::nationalise(self, *b) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[*b], text),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*b], format!("Nationalise: {e}")),
                };
            }
            PlayerCommand::Subsidise { corp, amount } => {
                let _ = match crate::systems::ownership::subsidise(self, *corp, *amount) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[*corp], text),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*corp], format!("Subsidise: {e}")),
                };
            }
            PlayerCommand::BreakUp(corp) => {
                let _ = match crate::systems::corps::break_up(self, *corp) {
                    Ok(s) => {
                        let text =
                            format!("Broke up {} into {}", self.owner_label(Some(*corp)), self.owner_label(Some(s)));
                        self.push_event(EventKind::PlayerAction, &[*corp, s], text)
                    }
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*corp], format!("BreakUp: {e}")),
                };
            }
            PlayerCommand::DemolishHome(home) => {
                let _ = match self.cmd_demolish_home(*home) {
                    Ok(n) => self.push_event(
                        EventKind::PlayerAction,
                        &[*home],
                        format!("Demolished Block#{} ({n} residents made homeless)", home.index),
                    ),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*home], format!("DemolishHome: {e}")),
                };
            }
            PlayerCommand::BuildHome { rect } => {
                let _ = match self.cmd_build_home(*rect) {
                    Ok((id, housed)) => self.push_event(
                        EventKind::PlayerAction,
                        &[id],
                        format!("Built Block#{} at ({}, {}); {housed} moved in", id.index, rect.x, rect.y),
                    ),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("BuildHome: {e}")),
                };
            }
        }
    }

    /// The god commands: `Ok((actors, event text))` or why nothing happened.
    fn cmd_god(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{law, law_brain};
        let gang_name = |w: &World, g: EntityId| w.comp::<Gang>(g).map(|g| g.name.clone()).ok_or("no such gang");
        match *cmd {
            PlayerCommand::KillAgent(who) => {
                if !self.is_alive(who) || !self.has::<Brain>(who) {
                    return Err("KillAgent: no such agent".into());
                }
                let name = self.name_of(who);
                self.kill_by(who, DeathCause::Violence, None);
                Ok((vec![who], format!("struck {name} dead")))
            }
            PlayerCommand::JailAgent { who, days } => {
                let name = self.name_of(who);
                self.god_jail(who, days)?;
                Ok((vec![who], format!("jailed {name} for {days} days")))
            }
            PlayerCommand::FreeAgent(who) => {
                law::player_release(self, who).map_err(|e| format!("FreeAgent: {e}"))?;
                Ok((vec![who], format!("freed {}", self.name_of(who))))
            }
            PlayerCommand::FundGang { gang, amount } => {
                let name = gang_name(self, gang)?;
                if let Some(g) = self.comp_mut::<Gang>(gang) {
                    g.treasury += amount;
                }
                Ok((vec![gang], format!("gave {name} {amount} coins")))
            }
            PlayerCommand::SeizeGangTreasury(gang) => {
                let name = gang_name(self, gang)?;
                let coins = self.comp_mut::<Gang>(gang).map_or(0, |g| std::mem::take(&mut g.treasury));
                if let Some(t) = self.treasury_mut() {
                    t.coins += coins;
                }
                Ok((vec![gang], format!("seized {coins} coins from {name}")))
            }
            PlayerCommand::KillGang(gang) => {
                let name = gang_name(self, gang)?;
                let members = self.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
                for &m in &members {
                    self.kill_by(m, DeathCause::Violence, None);
                }
                Ok((vec![gang], format!("killed all {} of {name}", members.len())))
            }
            PlayerCommand::JailGang { gang, days } => {
                let name = gang_name(self, gang)?;
                let members = self.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
                let jailed = members.into_iter().filter(|&m| self.god_jail(m, days).is_ok()).count();
                Ok((vec![gang], format!("jailed {jailed} of {name} for {days} days")))
            }
            PlayerCommand::FireAllGuards => {
                let guards = law_brain::guards(self);
                for &g in &guards {
                    // As `law::reconcile_guards` dismisses: no vacancy (job
                    // search would rehire the whole watch the next morning;
                    // reconcile_guards rehires up to the lever, five a day),
                    // but a Fire event, so it reaches the biography. An
                    // escort in progress ends: the suspect walks.
                    let text = format!("{} dismissed from the guard", self.name_of(g));
                    crate::systems::economy::dismiss(self, g, None, text);
                }
                law_brain::recompute_captain(self);
                Ok((guards, "dismissed every guard".to_string()))
            }
            PlayerCommand::SetTreasury(coins) => {
                let t = self.treasury_mut().ok_or("SetTreasury: no Civic Hall")?;
                t.coins = coins;
                Ok((vec![], format!("set the Treasury to {coins}")))
            }
            PlayerCommand::Riot(_)
            | PlayerCommand::Litter { .. }
            | PlayerCommand::SplitGang(_)
            | PlayerCommand::Derelict(_)
            | PlayerCommand::BuyBuilding { .. } => self.cmd_god_district(cmd),
            PlayerCommand::GrantAsset { agent, kind, tier } => {
                let a =
                    crate::systems::assets::grant(self, agent, kind, tier).map_err(|e| format!("GrantAsset: {e}"))?;
                Ok((vec![agent, a], format!("granted {} a {}", self.name_of(agent), self.name_of(a))))
            }
            PlayerCommand::Wreck(a) => {
                if !self.has::<crate::components::Asset>(a) {
                    return Err("Wreck: no such asset".into());
                }
                let what = self.name_of(a);
                crate::systems::assets::wreck(self, a, "by god");
                Ok((vec![a], format!("wrecked the {what}")))
            }
            PlayerCommand::ChromeEveryone { .. }
            | PlayerCommand::FloodStims { .. }
            | PlayerCommand::Brick(_)
            | PlayerCommand::Chase(_) => self.cmd_god_assets(cmd),
            PlayerCommand::WipeData(_)
            | PlayerCommand::SetTech { .. }
            | PlayerCommand::GrantData { .. }
            | PlayerCommand::GrantDeck { .. }
            | PlayerCommand::SetIce { .. }
            | PlayerCommand::Fry(_)
            | PlayerCommand::RunNow { .. }
            | PlayerCommand::WipeCorpData(_)
            | PlayerCommand::GrantDecks { .. }
            | PlayerCommand::SetCorpIce { .. } => self.cmd_god_virt(cmd),
            PlayerCommand::PlantRumour { .. }
            | PlayerCommand::SetReputation { .. }
            | PlayerCommand::GrantSkill { .. }
            | PlayerCommand::SetCreed { .. }
            | PlayerCommand::DeclareVendetta { .. }
            | PlayerCommand::KillFriend { .. }
            | PlayerCommand::Hunt { .. } => self.cmd_god_word(cmd),
            _ => self.cmd_god_corp(cmd),
        }
    }

    /// The M15 god commands (plan W42, phase 1).
    fn cmd_god_word(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        match *cmd {
            PlayerCommand::PlantRumour { about, deed, object, district, reach } => {
                if !self.has::<crate::components::Identity>(about)
                    && !self.has::<Gang>(about)
                    && !self.has::<Corp>(about)
                {
                    return Err("PlantRumour: no such agent or faction".into());
                }
                if district.index() >= self.rumours.len() {
                    return Err(format!("PlantRumour: no district {}", district.index()));
                }
                let reach = reach.clamp(0.0, 1.0);
                let e = crate::word::PoolEntry {
                    deed,
                    actor: Some(about),
                    object,
                    tick: self.tick,
                    hops: 0,
                    reach,
                    hole: None,
                    story: None,
                    kin: Default::default(),
                    told: Default::default(),
                    district,
                    press: 0,
                };
                crate::systems::gossip::post(self, district, e);
                // One first-hand holder: the living adult of the district nearest `about`.
                let from = self.comp::<crate::components::Position>(about).map(|p| p.tile);
                let holder = self
                    .citizens()
                    .into_iter()
                    .filter(|&a| a != about && crate::systems::demography::is_adult(self, a))
                    .filter(|&a| self.has::<crate::components::Memory>(a))
                    .filter(|&a| crate::systems::gossip::home_district(self, a) == Some(district))
                    .min_by_key(|&a| {
                        let d = match (from, self.comp::<crate::components::Position>(a)) {
                            (Some(f), Some(p)) => f.manhattan(p.tile),
                            _ => u32::MAX,
                        };
                        (d, a)
                    });
                if let Some(h) = holder {
                    let sal = self.config.gossip.deed_sal.get(deed);
                    let sev = self.config.gossip.deed_sev.get(deed);
                    let entry = crate::components::MemoryEntry {
                        subject: Some(about),
                        salience: sal,
                        valence: -sev * sal,
                        deed: Some(deed),
                        object,
                        ..crate::components::MemoryEntry::blank(crate::components::MemoryKind::Rumour, self.tick)
                    };
                    crate::systems::memory::hear_entry(self, h, entry);
                }
                let what = object.map(|o| format!(" {}", self.name_of(o))).unwrap_or_default();
                let text = format!(
                    "planted \"{} {}{what}\" in {} at reach {reach:.2}",
                    self.name_of(about),
                    deed.label(),
                    self.district_name(district)
                );
                Ok((vec![about], text))
            }
            PlayerCommand::GrantSkill { agent, skill, value, suit } => {
                if !crate::systems::law::living(self, agent) || !self.has::<crate::components::Skills>(agent) {
                    return Err("GrantSkill: no such living agent".into());
                }
                let v = value.clamp(0.0, 1.0);
                if let Some(s) = self.comp_mut::<crate::components::Skills>(agent) {
                    *s.social_mut(skill) = v;
                }
                if suit {
                    let mut a = crate::systems::reputation::appearance_of(self, agent);
                    a.dress = 3;
                    a.dress_pin = Some(3);
                    self.insert(agent, a);
                }
                let dress = if suit { " (in a suit)" } else { "" };
                Ok((vec![agent], format!("set {}'s {} to {v:.2}{dress}", self.name_of(agent), skill.label())))
            }
            PlayerCommand::SetCreed { gang, creed } => {
                let Some(g) = self.comp_mut::<Gang>(gang) else { return Err("SetCreed: no such gang".into()) };
                g.creed = creed;
                let name = g.name.clone();
                crate::systems::creeds::enforce(self, gang);
                let what = creed.map_or("no creed", |c| c.label());
                Ok((vec![gang], format!("{name} now holds {what}")))
            }
            PlayerCommand::SetReputation { who, axis, value, days } => {
                let i = who.index as usize;
                if i >= self.reputation.len() || !self.is_alive(who) {
                    return Err("SetReputation: no such agent or faction".into());
                }
                let until = self.tick + u64::from(days) * crate::time::TICKS_PER_DAY;
                let r = self.reputation[i].get_or_insert_with(Default::default);
                let mut axes = r.pinned.filter(|&(_, t)| t > self.tick).map_or_else(|| r.axes(), |(a, _)| a);
                axes[axis.index()] = value.clamp(0.0, 1.0);
                r.set_axes(axes);
                r.pinned = Some((axes, until));
                Ok((
                    vec![who],
                    format!("pinned {}'s {} at {value:.2} for {days} days", self.name_of(who), axis.label()),
                ))
            }
            PlayerCommand::DeclareVendetta { a, b, weight } => {
                let is_faction =
                    |w: &World, f: EntityId| w.has::<Gang>(f) || w.has::<Corp>(f) || w.has::<crate::components::Law>(f);
                if a == b || !is_faction(self, a) || !is_faction(self, b) {
                    return Err("DeclareVendetta: two different factions".into());
                }
                let w = weight.clamp(0.0, 1.0);
                for x in [a, b] {
                    if faction_leader(self, x).is_none() {
                        return Err(format!(
                            "DeclareVendetta: {} has no leader",
                            crate::systems::grudges::label(self, x)
                        ));
                    }
                }
                // The feud is declared open and held for `declared_days`.
                crate::systems::grudges::declare(self, a, b, w);
                let text = format!(
                    "declared a vendetta between {} and {} ({w:.2})",
                    crate::systems::grudges::label(self, a),
                    crate::systems::grudges::label(self, b)
                );
                Ok((vec![a, b], text))
            }
            PlayerCommand::KillFriend { of, by } => {
                if !crate::systems::law::living(self, of) || !crate::systems::law::living(self, by) || of == by {
                    return Err("KillFriend: no such living agents".into());
                }
                let friend = self
                    .neighbours(of)
                    .filter(|&o| o != by && crate::systems::law::living(self, o))
                    .filter_map(|o| {
                        self.edge(of, o)
                            .filter(|e| e.kind == crate::components::RelKind::Friend)
                            .map(|e| (o, e.affinity))
                    })
                    .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
                    .map(|(o, _)| o);
                let Some(victim) = friend else { return Err("KillFriend: no living Friend".into()) };
                let tile = self.comp::<crate::components::Position>(victim).map(|p| p.tile).unwrap_or_default();
                let d = self.district_of(tile);
                self.kill_by(victim, crate::components::DeathCause::Violence, Some(by));
                // The natural killing's path: witnesses rolled at the death's
                // tile, the district's crime note, a guard's report, the
                // pool post naming the killer when anyone noticed.
                let crime = crate::components::Crime::Murder;
                crate::systems::law::raise_crime_on(self, by, None, Some(victim), crime, tile);
                let named = self.rumours.iter().any(|p| {
                    p.entries
                        .iter()
                        .any(|e| e.deed == crate::word::Deed::Killed && e.object == Some(victim) && e.actor == Some(by))
                });
                // Nobody noticed: one witness, as the binder picks one (a
                // living adult whose Home is in the death's district, on the
                // Hunt stream), so the killing is named.
                let pool: Vec<EntityId> = self
                    .citizens()
                    .into_iter()
                    .filter(|&w| w != by && w != of && crate::systems::law::living(self, w))
                    .filter(|&w| crate::systems::demography::is_adult(self, w))
                    .filter(|&w| crate::systems::gossip::home_district(self, w) == Some(d))
                    .collect();
                let mut rng = self.rng.word(crate::word::WordNs::Hunt, self.tick, u64::from(victim.index));
                let witness =
                    (!named && !pool.is_empty()).then(|| pool[rand::Rng::random_range(&mut rng, 0..pool.len())]);
                if let Some(w) = witness {
                    self.remember_crime(w, by, crime, crate::systems::law::crime_salience(crime), Some(victim));
                    crate::systems::gossip::name_actor(self, victim, by);
                    crate::systems::gossip::post_deed(self, d, crate::word::Deed::Killed, Some(by), Some(victim));
                }
                let text = format!(
                    "{} killed {}, {}'s friend{}",
                    self.name_of(by),
                    self.name_of(victim),
                    self.name_of(of),
                    witness.map(|w| format!(" (seen by {})", self.name_of(w))).unwrap_or_default()
                );
                Ok((vec![by, victim, of], text))
            }
            PlayerCommand::Hunt { hunter, target } => {
                crate::systems::hunt::god_hunt(self, hunter, target)?;
                Ok((vec![hunter, target], format!("{} hunts {}", self.name_of(hunter), self.name_of(target))))
            }
            _ => Err("not a word command".into()),
        }
    }
}

/// M15 W42: a faction's leader: a gang's leader, a corp's exec, the captain.
pub fn faction_leader(world: &World, f: EntityId) -> Option<EntityId> {
    if let Some(g) = world.comp::<Gang>(f) {
        return g.leader.filter(|&l| crate::systems::law::living(world, l));
    }
    if let Some(c) = world.comp::<Corp>(f) {
        return c.exec.filter(|&e| crate::systems::law::living(world, e));
    }
    world.comp::<crate::components::Law>(f).and_then(|l| l.captain).filter(|&c| crate::systems::law::living(world, c))
}

impl World {
    /// The M14 god commands on the plane (plan V42, phase 1).
    fn cmd_god_virt(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{tech, virt};
        match *cmd {
            PlayerCommand::WipeData(b) => {
                if !self.has::<Building>(b) {
                    return Err("WipeData: no such building".into());
                }
                virt::relink(self);
                let n = virt::node_of_building(self, b).ok_or("WipeData: the building has no node")?;
                let units = tech::wipe_store(self, n, None);
                Ok((vec![b], format!("wiped {units} Data at {}", self.name_of(b))))
            }
            PlayerCommand::SetTech { corp, track, tier } => {
                if !self.has::<Corp>(corp) {
                    return Err("SetTech: no such corp".into());
                }
                tech::set_tier(self, corp, track, tier);
                let now = self.comp::<Corp>(corp).map_or(0, |c| c.tech.tier_of(track));
                Ok((vec![corp], format!("set {}'s {track} to {now}", self.owner_label(Some(corp)))))
            }
            PlayerCommand::GrantData { faction, track, units } => {
                tech::grant_data(self, faction, track, units).map_err(|e| format!("GrantData: {e}"))?;
                Ok((vec![faction], format!("gave {} {units} {track} Data", self.owner_label(Some(faction)))))
            }
            PlayerCommand::GrantDeck { agent, tier } => {
                if !self.has::<Brain>(agent) {
                    return Err("GrantDeck: no such agent".into());
                }
                let a = crate::systems::assets::grant(self, agent, crate::components::AssetKind::Deck, tier)
                    .map_err(|e| format!("GrantDeck: {e}"))?;
                Ok((vec![agent, a], format!("granted {} a tier-{tier} Deck", self.name_of(agent))))
            }
            PlayerCommand::SetIce { building, tier } => {
                virt::relink(self);
                let n = virt::node_of_building(self, building).ok_or("SetIce: the building has no node")?;
                let tier = tier.min(3);
                let p = virt::profile_mut(self, n).ok_or("SetIce: the node has no ICE")?;
                p.ice = tier;
                p.ice_maker = None;
                p.ice_arrears = 0;
                virt::bump_epoch(self);
                Ok((vec![building], format!("set {} to ICE {tier}", self.name_of(building))))
            }
            PlayerCommand::Fry(agent) => {
                let text = virt::god_fry(self, agent).map_err(|e| format!("Fry: {e}"))?;
                Ok((vec![agent], text))
            }
            PlayerCommand::RunNow { agent, target, purpose } => {
                let text = virt::god_run_now(self, agent, target, purpose).map_err(|e| format!("RunNow: {e}"))?;
                Ok((vec![agent, target], text))
            }
            PlayerCommand::WipeCorpData(corp) => {
                if !self.has::<Corp>(corp) {
                    return Err("WipeCorpData: no such corp".into());
                }
                virt::relink(self);
                let nodes: Vec<crate::virt::NodeId> = (0..self.virt.nodes.len())
                    .map(|i| crate::virt::NodeId(i as u16))
                    .filter(|&n| self.virt.node(n).is_some_and(|x| x.alive && x.owner == Some(corp)))
                    .collect();
                let units: u32 = nodes.into_iter().map(|n| tech::wipe_store(self, n, None)).sum();
                Ok((vec![corp], format!("wiped {units} Data across {}", self.owner_label(Some(corp)))))
            }
            PlayerCommand::GrantDecks { district, n, tier } => {
                let mut picks: Vec<(f32, EntityId)> = self
                    .citizens()
                    .into_iter()
                    .filter(|&a| crate::systems::demography::is_adult(self, a))
                    .filter(|&a| self.comp::<crate::components::Kit>(a).is_none_or(|k| k.deck.is_none()))
                    .filter(|&a| {
                        self.comp::<crate::components::Household>(a)
                            .and_then(|h| h.home)
                            .is_some_and(|h| self.district_of_building(h) == district)
                    })
                    .map(|a| (self.comp::<crate::components::Skills>(a).map_or(0.0, |s| s.hacking), a))
                    .collect();
                picks.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                let mut given = Vec::new();
                for (_, a) in picks.into_iter().take(usize::from(n)) {
                    if crate::systems::assets::grant(self, a, crate::components::AssetKind::Deck, tier).is_ok() {
                        given.push(a);
                    }
                }
                if given.is_empty() {
                    return Err(format!("GrantDecks: no adult without a deck lives in district {}", district.index()));
                }
                let text =
                    format!("granted {} adults of district {} a tier-{tier} Deck", given.len(), district.index());
                Ok((given, text))
            }
            PlayerCommand::SetCorpIce { corp, tier } => {
                if !self.has::<Corp>(corp) {
                    return Err("SetCorpIce: no such corp".into());
                }
                virt::relink(self);
                let tier = tier.min(3);
                let nodes: Vec<crate::virt::NodeId> = (0..self.virt.nodes.len())
                    .map(|i| crate::virt::NodeId(i as u16))
                    .filter(|&n| self.virt.node(n).is_some_and(|x| x.alive && x.owner == Some(corp)))
                    .collect();
                let mut set = 0;
                for n in nodes {
                    if let Some(p) = virt::profile_mut(self, n) {
                        p.ice = tier;
                        p.ice_maker = None;
                        p.ice_arrears = 0;
                        set += 1;
                    }
                }
                virt::bump_epoch(self);
                Ok((vec![corp], format!("set {set} of {}'s nodes to ICE {tier}", self.owner_label(Some(corp)))))
            }
            _ => Err("not a Virt god command".into()),
        }
    }

    /// The M13 god commands on assets (plan D48, docs/GOD_SCENARIOS_V4.md).
    fn cmd_god_assets(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::components::{Asset, AssetKind, AssetLoc, Controller, Good, Slot};
        use crate::systems::{assets, demography, law, stims};
        if !self.config.assets.enabled {
            return Err("assets are off".into());
        }
        match *cmd {
            PlayerCommand::ChromeEveryone { tier } => {
                if assets::list_price(self, AssetKind::Implant(Slot::Arms), tier).is_none() {
                    return Err(format!("ChromeEveryone: no implant at tier {tier}"));
                }
                let adults: Vec<EntityId> = self
                    .citizens()
                    .into_iter()
                    .filter(|&a| law::living(self, a) && demography::is_adult(self, a))
                    .collect();
                let mut n = 0;
                for &a in &adults {
                    for slot in [Slot::Arms, Slot::Nerves] {
                        if assets::grant(self, a, AssetKind::Implant(slot), tier).is_ok() {
                            n += 1;
                        }
                    }
                }
                Ok((vec![], format!("chromed {} adults ({n} T{tier} implants)", adults.len())))
            }
            PlayerCommand::FloodStims { district, n } => {
                if district.index() >= self.districts.len() {
                    return Err("FloodStims: no such district".into());
                }
                let name = self.district_name(district).to_string();
                let control = self.districts[district.index()].control;
                let mut hideouts: Vec<(EntityId, EntityId)> = Vec::new();
                for g in self.gangs() {
                    let Some(h) = self.hideout_of(g) else { continue };
                    let deals_here = stims::deal_bar(self, g).is_some_and(|b| self.district_of_building(b) == district);
                    if deals_here || control == Controller::Gang(g) || self.district_of_building(h) == district {
                        hideouts.push((g, h));
                    }
                }
                if hideouts.is_empty() {
                    return Err(format!("FloodStims: no gang deals in {name}"));
                }
                let mut added = 0;
                for &(_, h) in &hideouts {
                    added += self.add_stock(h, Good::Stims, n);
                }
                let gangs: Vec<EntityId> = hideouts.iter().map(|&(g, _)| g).collect();
                Ok((gangs, format!("flooded {} Hideouts dealing in {name} with {added} Stims", hideouts.len())))
            }
            PlayerCommand::Brick(lender) => {
                if !(self.has::<Brain>(lender) || self.has::<Gang>(lender) || self.has::<Corp>(lender)) {
                    return Err("Brick: no such lender".into());
                }
                let repo_days = self.config.assets.repo_days;
                let mut bodies = Vec::new();
                let mut n = 0;
                for a in assets::all_assets(self) {
                    let Some(x) = self.comp_mut::<Asset>(a) else { continue };
                    if !x.kind.is_implant() {
                        continue;
                    }
                    let Some(f) = x.finance.as_mut() else { continue };
                    if f.lender != Some(lender) {
                        continue;
                    }
                    f.arrears = f.arrears.max(repo_days);
                    x.bricked = true;
                    n += 1;
                    if let AssetLoc::Installed(b) = x.loc {
                        bodies.push(b);
                    }
                }
                bodies.sort_unstable();
                bodies.dedup();
                for &b in &bodies {
                    assets::rekit(self, b);
                }
                self.stats.current.repos += n;
                let who = self.owner_label(Some(lender));
                Ok((vec![lender], format!("{who} bricked {n} financed implants in {} bodies", bodies.len())))
            }
            PlayerCommand::Chase(agent) => {
                if !law::living(self, agent) || !self.has::<Brain>(agent) {
                    return Err("Chase: no such agent".into());
                }
                self.chase_pins.insert(agent);
                Ok((vec![agent], format!("pinned a chase on {}'s next trip", self.name_of(agent))))
            }
            _ => Err("not an asset god command".into()),
        }
    }

    /// The M12 god commands on districts (docs/GOD_SCENARIOS_V3.md, plan D42).
    fn cmd_god_district(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{gang, litter, ownership, riot, street};
        let check_district = |w: &World, d: crate::components::DistrictId, what: &str| {
            if d.index() < w.districts.len() {
                Ok(w.district_name(d).to_string())
            } else {
                Err(format!("{what}: no such district"))
            }
        };
        match *cmd {
            PlayerCommand::Riot(d) => {
                let name = check_district(self, d, "Riot")?;
                riot::start(self, d, true).map_err(|e| format!("Riot in {name}: {e}"))?;
                Ok((vec![], format!("raised a riot in {name}")))
            }
            PlayerCommand::Litter { district, level } => {
                let name = check_district(self, district, "Litter")?;
                if self.litter.is_empty() {
                    return Err("Litter: litter is off".into());
                }
                let v = (level.clamp(0.0, 1.0) * 254.0).round() as u8;
                let tiles = self.districts[district.index()].streets.clone();
                for &i in &tiles {
                    if let Some(t) = self.litter.get_mut(i as usize) {
                        // Rubble (the Damage hook) stays rubble.
                        if *t != litter::RUBBLE {
                            *t = v;
                        }
                    }
                }
                litter::district_means(self);
                Ok((vec![], format!("littered {} street tiles of {name} to {v}", tiles.len())))
            }
            PlayerCommand::SplitGang(g) => {
                let name = self.comp::<Gang>(g).map(|x| x.name.clone()).ok_or("SplitGang: no such gang")?;
                let s = gang::split(self, g, None, false).map_err(|e| format!("SplitGang {name}: {e}"))?;
                let sn = self.comp::<Gang>(s).map_or_else(String::new, |x| x.name.clone());
                Ok((vec![g, s], format!("split {name}: the {sn}")))
            }
            PlayerCommand::Derelict(b) => {
                let what = self.name_of(b);
                let Some(bd) = self.comp::<Building>(b) else { return Err("Derelict: no such building".into()) };
                if bd.demolished || bd.derelict {
                    return Err(format!("Derelict: {what} is already gone or derelict"));
                }
                if !street::can_go_derelict(bd.kind) {
                    return Err(format!("Derelict: a {} cannot go derelict", bd.kind.label()));
                }
                if !street::make_derelict(self, b, "by god") {
                    return Err(format!("Derelict: {what} refused"));
                }
                Ok((vec![b], format!("made {what} derelict")))
            }
            PlayerCommand::BuyBuilding { buyer, building, price } => {
                let Some(bd) = self.comp::<Building>(building) else {
                    return Err("BuyBuilding: no such building".into());
                };
                if bd.demolished {
                    return Err("BuyBuilding: demolished".into());
                }
                let (owner, derelict) = (bd.owner, bd.derelict);
                if buyer.is_some_and(|t| !(self.has::<Wallet>(t) || self.has::<Gang>(t) || self.has::<Corp>(t))) {
                    return Err("BuyBuilding: the buyer is not an agent, gang or corp".into());
                }
                if owner == buyer && !derelict {
                    return Err("BuyBuilding: the buyer owns it".into());
                }
                let price = price.max(0);
                // As every M11 flow: the buyer pays, the owner (or the city) is paid.
                let paid = if owner == buyer {
                    0
                } else {
                    ownership::charge(self, buyer, owner, price, ownership::Flow::Sale)
                };
                let what = self.name_of(building);
                let (from, to) = (self.owner_label(owner), self.owner_label(buyer));
                if derelict {
                    street::restore(self, building, buyer, "bought by god");
                } else {
                    crate::systems::corps::move_building(self, building, buyer);
                    if buyer.is_none() {
                        crate::systems::corps::end_contract(self, building, "bought by the city");
                    }
                }
                if let Some(c) = owner.filter(|&o| self.has::<Corp>(o) && Some(o) != buyer) {
                    ownership::push_corp_shock(self, c, CorpShock::BuildingLost);
                }
                let actors: Vec<EntityId> = [Some(building), owner, buyer].into_iter().flatten().collect();
                Ok((actors, format!("{to} bought {what} from {from} for {paid}")))
            }
            _ => Err("not a god command".into()),
        }
    }

    /// The M11 god commands on corps (docs/GOD_SCENARIOS_V2.md).
    fn cmd_god_corp(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{classes, corps, ownership};
        let corp_name = |w: &World, c: EntityId| w.comp::<Corp>(c).map(|c| c.name.clone()).ok_or("no such corp");
        match *cmd {
            PlayerCommand::FundCorp { corp, amount } => {
                let name = corp_name(self, corp)?;
                if let Some(c) = self.comp_mut::<Corp>(corp) {
                    c.treasury += amount;
                }
                Ok((vec![corp], format!("gave {name} {amount} coins")))
            }
            PlayerCommand::BankruptCorp(corp) => {
                let name = corp_name(self, corp)?;
                let days = self.config.corps.bankrupt_days + 1;
                let back = self.tick.saturating_sub(days * crate::time::TICKS_PER_DAY);
                if let Some(c) = self.comp_mut::<Corp>(corp) {
                    c.treasury = -1;
                    c.negative_since = Some(back);
                }
                corps::bankrupt(self, corp);
                Ok((vec![corp], format!("bankrupted {name}")))
            }
            PlayerCommand::SeizeBuildings { owner, to } => {
                if to.is_some_and(|t| !(self.has::<Wallet>(t) || self.has::<Gang>(t) || self.has::<Corp>(t))) {
                    return Err("SeizeBuildings: the new owner is not an agent, gang or corp".into());
                }
                if owner == to {
                    return Err("SeizeBuildings: same owner".into());
                }
                let owned: Vec<EntityId> = self
                    .with::<Building>()
                    .into_iter()
                    .filter(|&b| self.comp::<Building>(b).is_some_and(|bd| bd.owner == owner && !bd.demolished))
                    .collect();
                if owned.is_empty() {
                    return Err(format!("SeizeBuildings: {} owns nothing", self.owner_label(owner)));
                }
                let (from_label, to_label) = (self.owner_label(owner), self.owner_label(to));
                for &b in &owned {
                    corps::move_building(self, b, to);
                    if to.is_none() {
                        // The city does not keep a corp's guard contract.
                        corps::end_contract(self, b, "seized by the city");
                    }
                }
                if let Some(c) = owner.filter(|&o| self.has::<Corp>(o)) {
                    ownership::push_corp_shock(self, c, CorpShock::BuildingLost);
                }
                let actors: Vec<EntityId> = owner.into_iter().chain(to).collect();
                Ok((actors, format!("gave all {} buildings of {from_label} to {to_label}", owned.len())))
            }
            PlayerCommand::KillExec(corp) => {
                let name = corp_name(self, corp)?;
                let exec = self.comp::<Corp>(corp).and_then(|c| c.exec);
                let exec = exec.filter(|&e| self.is_alive(e) && self.has::<Brain>(e));
                let exec = exec.ok_or_else(|| format!("KillExec: {name} has no living exec"))?;
                let who = self.name_of(exec);
                self.kill_by(exec, DeathCause::Violence, None);
                Ok((vec![corp, exec], format!("struck {who}, exec of {name}, dead")))
            }
            PlayerCommand::KillStaff(corp) => {
                let name = corp_name(self, corp)?;
                let staff = self.comp::<Corp>(corp).map(|c| classes::employees(self, c)).unwrap_or_default();
                for &a in &staff {
                    self.kill_by(a, DeathCause::Violence, None);
                }
                Ok((vec![corp], format!("killed all {} employees of {name}", staff.len())))
            }
            PlayerCommand::SetCorpOrder { corp, order, niche, days } => {
                let name = corp_name(self, corp)?;
                let (now, evict_days) = (self.tick, self.config.rent.evict_days);
                let c = self.comp_mut::<Corp>(corp).ok_or("no such corp")?;
                let niche = match niche {
                    Some(n) if !c.niches.contains(&n) => return Err(format!("SetCorpOrder: {name} is not in {n}")),
                    Some(n) => n,
                    None => c
                        .order_niche
                        .filter(|n| c.niches.contains(n))
                        .or_else(|| c.niches.iter().next().copied())
                        .ok_or("SetCorpOrder: the corp has no niche")?,
                };
                // As `corp_brain::rescore` switches (D22).
                if c.order == CorpOrder::Squeeze {
                    c.wage_mult = 1.0;
                    c.evict_days_override = None;
                }
                if order == CorpOrder::Squeeze {
                    match niche {
                        Niche::Housing => c.evict_days_override = Some(evict_days.saturating_sub(1).max(2)),
                        Niche::Food => c.wage_mult = 0.9,
                        Niche::Security | Niche::Tech => {}
                    }
                }
                c.order = order;
                c.order_niche = Some(niche);
                c.order_since = now;
                c.pinned_until = Some(now + u64::from(days) * crate::time::TICKS_PER_DAY);
                Ok((vec![corp], format!("pinned {name} to {order} in {niche} for {days} days")))
            }
            PlayerCommand::StrikeNow(corp) => {
                let name = corp_name(self, corp)?;
                let strikers = self.comp::<Corp>(corp).map(|c| classes::employees(self, c)).unwrap_or_default();
                if strikers.is_empty() {
                    return Err(format!("StrikeNow: {name} has no workers"));
                }
                classes::walk_out(self, &strikers);
                self.stats.current.strikes += 1;
                let text = format!("{} workers of {name} walk out (by god)", strikers.len());
                self.push_event(EventKind::Strike, &[corp], text);
                crate::systems::corp_brain::push_shock(self, corp, CorpShock::Strike);
                Ok((vec![corp], format!("called a strike at {name}")))
            }
            PlayerCommand::WipeTreasuries => {
                let all = self.corps();
                for &c in &all {
                    if let Some(cc) = self.comp_mut::<Corp>(c) {
                        cc.treasury = 0;
                    }
                }
                let n = all.len();
                Ok((all, format!("wiped the treasuries of {n} corps")))
            }
            _ => Err("not a god command".into()),
        }
    }

    /// A god sentence: no report, no capacity check. A prisoner's sentence is
    /// extended instead. It is filed as Assault: a full Jail makes room by
    /// freeing the Theft prisoner with the longest sentence, which a long
    /// god sentence for Theft always was.
    fn god_jail(&mut self, who: EntityId, days: u32) -> Result<(), String> {
        if !self.is_alive(who) || !self.has::<Brain>(who) {
            return Err("JailAgent: no such agent".into());
        }
        let until = self.tick + u64::from(days.max(1)) * crate::time::TICKS_PER_DAY;
        // Already inside: the sentence runs to whichever end is later.
        if let Some(s) = self.comp_mut::<Sentence>(who) {
            s.until_tick = s.until_tick.max(until);
            s.crime = s.crime.max(Crime::Assault);
            return Ok(());
        }
        let jail = self.building_of_kind(BuildingKind::Jail).ok_or("JailAgent: no Precinct")?;
        crate::systems::law::sentence(self, who, Crime::Assault, until, jail);
        Ok(())
    }

    /// Tiles become Ground, residents are homeless (one event each), the
    /// building is marked demolished and dropped from the kind index.
    fn cmd_demolish_home(&mut self, home: EntityId) -> Result<usize, String> {
        let Some(b) = self.comp::<Building>(home) else { return Err("no such building".into()) };
        if b.kind != BuildingKind::Home {
            return Err(format!("{} is not a Block", b.kind.label()));
        }
        if b.demolished {
            return Err("already demolished".into());
        }
        // M11 D44: only the city's own Blocks.
        if b.owner.is_some() {
            return Err(format!("owned by {}; nationalise it first", self.owner_label(b.owner)));
        }
        // M12 D26: with the street on, a demolished Block stands derelict at
        // half capacity (squatters may move in), its walls left as they are.
        if self.config.street.enabled {
            if b.derelict {
                return Err("already derelict".into());
            }
            let residents: Vec<EntityId> = self.residents_of(home).to_vec();
            let cap = b.capacity;
            for &r in &residents {
                let name = self.name_of(r);
                self.push_event(EventKind::Homeless, &[r], format!("{name} is homeless"));
            }
            crate::systems::street::make_derelict(self, home, "demolished");
            if let Some(bd) = self.comp_mut::<Building>(home) {
                bd.capacity = cap / 2;
            }
            return Ok(residents.len());
        }
        let (rect, door, occupants) = (b.rect, b.door, b.occupants.clone());
        let outside = self.outside_door(b);
        for o in occupants {
            self.abort_plan(o);
            self.remove_from_building(o);
            if let Some(p) = self.comp_mut::<Position>(o) {
                p.tile = outside;
                p.building = None;
            }
        }
        let residents: Vec<EntityId> = self.residents_of(home).to_vec();
        for &r in &residents {
            self.set_home(r, None);
            // Children living there stand outside too.
            if self.comp::<Position>(r).is_some_and(|p| p.building == Some(home)) {
                if let Some(p) = self.comp_mut::<Position>(r) {
                    p.tile = outside;
                    p.building = None;
                }
            }
            let name = self.name_of(r);
            self.push_event(EventKind::Homeless, &[r], format!("{name} is homeless"));
        }
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                self.map.set_tile(TilePos { x, y }, TileKind::Ground);
            }
        }
        let _ = door;
        if let Some(b) = self.comp_mut::<Building>(home) {
            b.demolished = true;
            b.occupants.clear();
            b.stock_food = 0;
        }
        if let Some(v) = self.buildings_by_kind.get_mut(&BuildingKind::Home) {
            v.retain(|&h| h != home);
        }
        self.invalidate_flow_fields();
        crate::systems::districts::rebuild(self);
        Ok(residents.len())
    }

    /// A 4×4..6×6 rect of Ground, clear of other buildings and of Water,
    /// paid from the Treasury: walls on the perimeter, a door mid-south,
    /// capacity 6; the lowest-index homeless move in.
    fn cmd_build_home(&mut self, rect: Rect) -> Result<(EntityId, usize), String> {
        if !(4..=6).contains(&rect.w) || !(4..=6).contains(&rect.h) {
            return Err(format!("rect must be 4x4 to 6x6, got {}x{}", rect.w, rect.h));
        }
        let (x1, y1) = (i32::from(rect.x) + i32::from(rect.w), i32::from(rect.y) + i32::from(rect.h));
        if !self.map.in_bounds(i32::from(rect.x), i32::from(rect.y)) || !self.map.in_bounds(x1 - 1, y1 - 1) {
            return Err("rect is off the map".into());
        }
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                let t = TilePos { x, y };
                if self.map.tile_at(t) != TileKind::Ground {
                    return Err(format!("({x}, {y}) is {:?}, not Ground", self.map.tile_at(t)));
                }
            }
        }
        for b in self.with::<Building>() {
            if let Some(bd) = self.comp::<Building>(b) {
                if !bd.demolished && rects_overlap(bd.rect, rect) {
                    return Err(format!("overlaps {}#{}", bd.kind.label(), b.index));
                }
            }
        }
        for y in (i32::from(rect.y) - 1)..=y1 {
            for x in (i32::from(rect.x) - 1)..=x1 {
                if self.map.in_bounds(x, y) && self.map.tile_at(TilePos { x: x as u8, y: y as u8 }) == TileKind::Water {
                    return Err("adjacent to Water".into());
                }
            }
        }
        let door = TilePos { x: rect.x + rect.w / 2, y: rect.y + rect.h - 1 };
        let outside = (i32::from(door.x), i32::from(door.y) + 1);
        if !self.map.in_bounds(outside.0, outside.1) {
            return Err("the door would open off the map".into());
        }
        let outside_tile = TilePos { x: outside.0 as u8, y: outside.1 as u8 };
        if !matches!(self.map.tile_at(outside_tile), TileKind::Ground | TileKind::Road) {
            return Err(format!("the door would open onto {:?}", self.map.tile_at(outside_tile)));
        }
        let cost = self.config.economy.build_home_cost;
        let coins = self.treasury().map_or(0, |t| t.coins);
        if coins < cost {
            return Err(format!("Treasury has {coins}, needs {cost}"));
        }
        if let Some(t) = self.treasury_mut() {
            t.coins -= cost;
        }
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                let t = TilePos { x, y };
                let kind = if t == door {
                    TileKind::Door
                } else if rect.on_perimeter(t) {
                    TileKind::Wall
                } else {
                    TileKind::Ground
                };
                self.map.set_tile(t, kind);
            }
        }
        let capacity = self.config.buildings.home.capacity;
        let id = self.spawn();
        self.insert(
            id,
            Building {
                kind: BuildingKind::Home,
                production_accum: 0.0,
                claim: None,
                child_food_debt: 0.0,
                rect,
                door,
                stock_food: 0,
                capacity,
                owner: None,
                occupants: Vec::new(),
                demolished: false,
                tier: 1,
                rent_per_day: self.levers.city_rent[1],
                revenue_today: 0,
                revenue: std::collections::VecDeque::new(),
                secured_by: None,
                derelict: false,
                empty_since: None,
                closed_until: None,
                full_capacity: None,
                stock_goods: [0; 2],
                asset_sales_today: 0,
                asset_sales: std::collections::VecDeque::new(),
                security: Default::default(),
                focus: None,
                hacked: None,
                last_door_open: None,
                label: None,
                feed: None,
                venue: None,
                charity: None,
                camp: None,
                input_accum: 0.0,
                floors: 1,
                floor_days: 0,
            },
        );
        self.buildings_by_kind.entry(BuildingKind::Home).or_default().push(id);
        self.invalidate_flow_fields();
        crate::systems::districts::rebuild(self);
        let homeless: Vec<EntityId> = self
            .citizens()
            .into_iter()
            .filter(|&c| matches!(self.comp::<Household>(c), Some(Household { home: None, .. })))
            .take(usize::from(capacity))
            .collect();
        let housed = homeless.len();
        for h in homeless {
            self.set_home(h, Some(id));
        }
        Ok((id, housed))
    }

    /// Split `amount` evenly over every Market (remainder to the lowest id),
    /// each share capped by that Market's room and by what the Warehouse holds
    /// (M10 D22).
    fn cmd_release_reserve(&mut self, amount: u32) {
        let markets: Vec<EntityId> = self
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .copied()
            .filter(|&m| self.comp::<Building>(m).is_some_and(|b| !b.demolished))
            .collect();
        let Some(wh) = self.building_of_kind(BuildingKind::Warehouse).filter(|_| !markets.is_empty()) else {
            self.push_event(EventKind::PlayerActionFailed, &[], "ReleaseReserve: no Reserve Depot or Street Market");
            return;
        };
        let market_cap = self.config.buildings.market.stock_cap;
        let mut available = self.comp::<Building>(wh).map_or(0, |b| b.stock_food);
        let n = markets.len() as u32;
        let mut moved = 0;
        // Each round splits what is left evenly over the Markets that still
        // have room (remainder to the lowest ids); a Market that fills drops
        // out and the next round shares its overflow among the rest.
        let mut left = amount;
        loop {
            let open: Vec<(EntityId, u32)> = markets
                .iter()
                .map(|&mk| (mk, self.comp::<Building>(mk).map_or(0, |b| market_cap.saturating_sub(b.stock_food))))
                .filter(|&(_, room)| room > 0)
                .collect();
            if left == 0 || available == 0 || open.is_empty() {
                break;
            }
            let k = open.len() as u32;
            let mut round = 0;
            for (i, &(mk, room)) in open.iter().enumerate() {
                let share = left / k + u32::from((i as u32) < left % k);
                let m = share.min(room).min(available);
                if m == 0 {
                    continue;
                }
                available -= m;
                round += m;
                if let Some(b) = self.comp_mut::<Building>(mk) {
                    b.stock_food += m;
                }
            }
            if round == 0 {
                break;
            }
            left -= round;
            moved += round;
        }
        if moved == 0 {
            self.push_event(EventKind::PlayerActionFailed, &[], "ReleaseReserve: nothing to move");
            return;
        }
        if let Some(b) = self.comp_mut::<Building>(wh) {
            b.stock_food -= moved;
        }
        let to = if n == 1 { "the Street Market".to_string() } else { format!("{n} Street Markets") };
        self.push_event(EventKind::PlayerAction, &[], format!("Released {moved} food from the Reserve Depot to {to}"));
    }

    fn cmd_grant_coins(&mut self, agent: EntityId, amount: i64) {
        if amount <= 0 || !self.has::<Wallet>(agent) {
            self.push_event(EventKind::PlayerActionFailed, &[agent], "GrantCoins: bad amount or no such agent");
            return;
        }
        let treasury = self.treasury().map_or(0, |t| t.coins);
        if treasury < amount {
            self.push_event(
                EventKind::PlayerActionFailed,
                &[agent],
                format!("GrantCoins: Treasury has {treasury}, needs {amount}"),
            );
            return;
        }
        if let Some(t) = self.treasury_mut() {
            t.coins -= amount;
        }
        if let Some(w) = self.comp_mut::<Wallet>(agent) {
            w.coins += amount;
        }
        let name = self.name_of(agent);
        self.push_event(EventKind::PlayerAction, &[agent], format!("Granted {amount} coins to {name}"));
    }
}
