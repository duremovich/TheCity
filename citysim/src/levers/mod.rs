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

mod apply;
mod commands;
mod god;

pub use god::*;

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
    SetGuardCount(u16),
    SetImmigrationPerWeek(u16),
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
    /// M18 P1 (`systems/player.rs`): a human takes over an adult body
    /// (`true`, autopilot off) or lets it go for good (`false`).
    Possess(EntityId, bool),
    /// M18 P1: a directed plan for the possessed body (a click's walk).
    Steps {
        agent: EntityId,
        steps: Vec<crate::components::ActionInstance>,
    },
    /// M18 P1: a goal for the possessed body, planned by `plan::plan_for`.
    Goal {
        agent: EntityId,
        goal: crate::components::GoalKind,
    },
    /// M18 P1: the possessed body drops its plan and stands.
    Stop(EntityId),
    /// M18 P1: the possessed body's own brain on (`true`) or off.
    SetAutopilot(EntityId, bool),
    /// C4 (docs/COMBAT.md § 5): a fight order for the possessed body
    /// (`combat::order`): a stance, Attack, Draw or Holster, Take cover,
    /// Flee, Yield, Help; applied at the next beat of its fight.
    Fight {
        agent: EntityId,
        order: crate::systems::combat::Order,
    },
    /// M18 P2: a menu verb for the possessed body (`player::act`).
    Act {
        agent: EntityId,
        verb: crate::systems::player::Verb,
    },
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
    // --- M11 god commands (docs/history/GOD_SCENARIOS_V2.md): corps, out of the rules.
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
    SetSanitation(u16),
    /// Jobs v2 J17 (P5a): release a `SetGuardCount`/`SetSanitation` pin back
    /// to the civic budget (CLI `civic=auto`; under the market wage law only).
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
    // --- M12 god commands (docs/history/GOD_SCENARIOS_V3.md): districts, out of the rules.
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
    // --- M13 god commands (plan D48, docs/history/GOD_SCENARIOS_V4.md).
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
    /// Jobs and room J19 (P6): the wage a migrant would earn outside, over
    /// `[demography] outside_wage` (CLI `outside_wage=<coins>`; M17's outside
    /// moves it). Read only with wages on.
    SetOutsideWage(f32),
    // --- M16a phase 4 (plan C39): the contract board's levers.
    /// Off: Fixers are unlicensed: every office's heat is floored at 0.5
    /// (`contracts::heat_add`) and no agent registers a new one
    /// (`founding::fixer_ok`). On: licensed again (the floor lifts; heat
    /// decays as before).
    SetFixerLicence(bool),
    /// The Conspiracy sentence as a multiple of Murder's (over `[law]
    /// accessory_mult`).
    SetAccessoryMult(f32),
    /// On: the law posts a public Locate on every wanted suspect after one
    /// day instead of `[contracts] law_bounty_days`.
    PublicBounties(bool),
    // --- Transit (docs/TRANSIT.md T15): god closures of the El.
    /// God: close a line (by `[[transit.lines]]` row) for `days`; 0 reopens.
    CutLine {
        line: u8,
        days: u32,
    },
    /// God: close a stop (by the network's stop index) for `days`; 0 reopens.
    CloseStop {
        stop: u16,
        days: u32,
    },
    // --- P10 (docs/P10_POLICY.md): the city's policy as levers (the
    // council's acts once M18's player holds a seat; a god's today). The
    // dole is `SetDolePerDay`, public works `SetPublicWorks`.
    /// The levy on (the M11 wage schedule, the corps' upkeep levy and L2's
    /// budget band) or off (the market wage law: wages from revenue, the
    /// property rate, the tax band). A mid-run switch runs, uncalibrated.
    SetLevy(bool),
    /// School meals from the Reserve on or off.
    SetSchoolMeals(bool),
    /// The Reserve's free release to a stranded Market on or off.
    SetReserveRelease(bool),
    /// The whole safety net enacted (the dole at `[levers] dole_per_day`,
    /// public works, school meals, the Reserve release) or repealed (all
    /// four off). The levy is its own lever.
    SetSafetyNet(bool),
    /// Jobs & lives phase 4 (§ 5.1): street work legal (no sweeps) or swept.
    SetStreetLegal(bool),
    /// Jobs & lives phase 4 (§ 5.1): brothels licensed (closed only under a
    /// Crackdown) or not (closed in a Sweep or Crackdown district).
    SetBrothelsLicensed(bool),
    // --- M16b phase 4 (spec § 8): the 16b lever and god commands.
    /// Off: corps cannot build Prisons, and a Prison admits no new captive.
    SetPrisonPermit(bool),
    /// God: `captive` held at `at` for `holder` (`None` the head of no
    /// faction: the god's own hold), with `demand` over the captive's
    /// payer (a `Release` over the captain).
    Hostage {
        captive: EntityId,
        holder: Option<EntityId>,
        at: EntityId,
        demand: Option<crate::contract::Demand>,
    },
    /// God: `agent` goes down at once, its bleed timer running.
    Wound {
        agent: EntityId,
        by: Option<EntityId>,
    },
    /// God: a Trauma subscription for `agent` with `corp`'s crew (the
    /// agent pays).
    Subscribe {
        agent: EntityId,
        corp: EntityId,
    },
    /// God: a tier-`tier` tag on `agent` for `owner`.
    TagAgent {
        agent: EntityId,
        owner: EntityId,
        tier: u8,
    },
    /// God: a coercion made without the roll (`leverage::force`).
    Coerce {
        agent: EntityId,
        over: EntityId,
        leverage: crate::contract::Leverage,
        demand: crate::contract::Demand,
    },
    /// God: a board vote now (a corp without a board seats one).
    CallVote(EntityId),
    /// God: a Secret coercion's deed posted at reach 1.0 (it lapses).
    ExposeSecret(crate::contract::CoercionId),
    /// M17 phase 1: pin the World's Ambient appetite (`None` releases it to
    /// the walk). The global economy's lever on Ambient demand.
    SetAmbientAppetite(Option<f32>),
    /// M17 phase 2: the city's sanction on a parent without expiry while
    /// set: no remittance out, no subsidy in, and its cut on the income.
    Embargo {
        id: crate::outside::OutsideId,
        on: bool,
    },
    /// M17 phase 2: the share of every remittance the Treasury keeps.
    SetTariff(f32),
    /// M17 phase 3, god: a parent sends `bodies` Outsiders now (the
    /// cooldown ignored, `outsiders_cap` not).
    ForceReinforce {
        id: crate::outside::OutsideId,
        bodies: u32,
    },
    /// M17 phase 3, god: a parent's order pinned for `days`.
    SetOutsideOrder {
        id: crate::outside::OutsideId,
        order: crate::outside::OutsideOrder,
        days: u32,
    },
    /// M17 phase 4, god: a run's effect on a parent's uplink now, for
    /// `patron` (a theft, a wipe, a Ledger siphon).
    ForceUplinkRun {
        parent: crate::outside::OutsideId,
        purpose: crate::virt::Purpose,
        patron: Option<EntityId>,
    },
    /// M17 phase 4, god: a parent dies now.
    KillParent(crate::outside::OutsideId),
    /// M17 phase 4, god: a parent's treasury to 0 (absorbed abroad).
    WipeParentTreasury(crate::outside::OutsideId),
    /// M17 phase 4, god: a parent's edge.
    SetParentEdge {
        parent: crate::outside::OutsideId,
        edge: f32,
    },
    /// M17 phase 4, god: a sanction on a parent for `days`.
    SanctionParent {
        parent: crate::outside::OutsideId,
        days: u32,
        cut: f32,
    },
    /// M17 phase 4: no scrip issued; every holder redeemed at health.
    BanScrip(bool),
    /// M17 phase 4b, god: a gang's syndicate sends its brothers at the next
    /// midnight (the cooldown ignored; the syndicate pays).
    SendBrothers(EntityId),
    /// M17 phase 5, god: a mission abroad on a parent's asset with this crew
    /// (the leader first), `buyer` of record.
    PostAbroad {
        buyer: Option<EntityId>,
        on: crate::outside::OutsideId,
        asset: crate::outside::OutsideAsset,
        crew: Vec<EntityId>,
    },
    /// M17 phase 5, god: a Blackwall breach behind a parent's uplink, the
    /// patron's Lab stores wiped with it.
    BlackwallBreach {
        on: crate::outside::OutsideId,
        patron: Option<EntityId>,
    },
    /// M17 phase 5: the city's cap on the State's guards (`None`: uncapped;
    /// `Some(0)` refuses the State's men).
    SetStateGuards(Option<u16>),
    /// M17 phase 5, god: a corp founds its parent now.
    FoundParent(EntityId),
    /// Jobs & lives phase 3 (section 4), god: `n` weapons at `tier` into a
    /// gang's armoury, past the store's cap (`god_flood_guns`).
    FloodGuns {
        gang: EntityId,
        n: u32,
        tier: u8,
    },
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
    pub guard_count: u16,
    /// `0..=30`, default 13 (M10; v1 `0..=10`, 2).
    pub immigration_per_week: u16,
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
    #[serde(default = "default_guard_weight", deserialize_with = "crate::components::district_weights_any_len")]
    pub guard_weight: [f32; crate::components::MAX_DISTRICTS],
    /// M12 D12: the player's stance pin per district; `None` = the captain.
    #[serde(default, deserialize_with = "crate::components::district_slots_any_len")]
    pub stance_pin: [Option<crate::components::Stance>; crate::components::MAX_DISTRICTS],
    /// M12 (curfew lands with the levers in phase 5; Vagrancy reads it now).
    #[serde(default, deserialize_with = "crate::components::district_slots_any_len")]
    pub curfew: [bool; crate::components::MAX_DISTRICTS],
    /// M12 D23: city Sanitation workers (`[levers] sanitation_count`).
    #[serde(default)]
    pub sanitation_count: u16,
    /// M12 D23: a multiplier on each district's sweeper weight (1.0).
    #[serde(default = "default_guard_weight", deserialize_with = "crate::components::district_weights_any_len")]
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
    /// M16b phase 4 (`SetPrisonPermit`): corps may build Prisons and hold
    /// new captives in them.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub prison_permit: bool,
    /// M16a (plan C39; `PublicBounties`, phase 4): the law posts a Locate
    /// on every wanted suspect after one day.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub public_bounties: bool,
    // --- P10 (docs/P10_POLICY.md): the policy levers the retired
    // `[economy2] wages` / `no_safety_net` switches became. With
    // `dole_per_day` and `public_works` above they are the city's policy
    // (seeded from `[policy]`; serde defaults are the shipped city's).
    /// The levy (`SetLevy`): the M11 wage schedule, the corps' upkeep levy
    /// to the Treasury and L2's budget band; off, the market wage law
    /// (`econ::wages_on`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub levy: bool,
    /// School meals (`SetSchoolMeals`): a child whose pantry is empty eats
    /// from the Reserve.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub school_meals: bool,
    /// The Reserve's free release to a stranded Market (`SetReserveRelease`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reserve_release: bool,
    /// Jobs & lives phase 4 (§ 5.1, D7): the vice policy levers, seeded
    /// from `[vice]` (the utopia/dystopia game moves them).
    #[serde(default, skip_serializing_if = "ViceLevers::is_default")]
    pub vice: ViceLevers,
    /// Jobs & lives phase 6 (§ 8): the faith's influence slots (M18's life
    /// paths move them; every reader returns 0 at the default).
    #[serde(default, skip_serializing_if = "FaithLevers::is_default")]
    pub faith: FaithLevers,
    /// Jobs & lives phase 7 (§ 6.1, Dylan's answer 4): the Treasury pays the
    /// School fees (off by default: fees only).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub public_school: bool,
}

/// Jobs & lives phase 6 (§ 8, the takeover hooks): a board member's lean
/// toward a candidate of its creed (`faith::board_lean`), the law's lean
/// by the established creed's share of the guard (`faith::posture_lean`),
/// and the established creed (a row index). All off by default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FaithLevers {
    pub board_lean: f32,
    pub posture_lean: f32,
    pub established: Option<u8>,
}

impl FaithLevers {
    pub fn is_default(&self) -> bool {
        *self == FaithLevers::default()
    }
}

/// Jobs & lives phase 4 (D7, Dylan's answer 2): brothels licensed and
/// tolerated, street work swept; phase 5's dollhouse contracts legal and a
/// captive's chip a Trafficking (their slots seeded now).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ViceLevers {
    pub brothels_licensed: bool,
    pub street_legal: bool,
    pub dollhouse_contracts: bool,
    pub captive_chip_trafficking: bool,
}

impl ViceLevers {
    pub fn is_default(&self) -> bool {
        *self == ViceLevers::default()
    }

    /// The seeds of `[vice]`'s policy keys (all off with `[vice]` off).
    pub fn from_config(cfg: &Config) -> ViceLevers {
        if !(cfg.jobs2.enabled && cfg.vice.enabled) {
            return ViceLevers::default();
        }
        ViceLevers {
            brothels_licensed: cfg.vice.brothels_licensed,
            street_legal: cfg.vice.street_legal,
            dollhouse_contracts: cfg.vice.dollhouse_contracts,
            captive_chip_trafficking: cfg.vice.captive_chip_trafficking,
        }
    }
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

/// The range a `SetTaxRate` may set: `-[treasury] credit_max` (the surplus
/// credit, a negative rate; 0 with no credit) up to 0.3. The city panel's
/// tax slider reads it too.
pub fn tax_lever_range(cfg: &Config) -> (f32, f32) {
    ((-cfg.treasury.credit_max).min(0.0), 0.3)
}

impl Levers {
    pub fn from_config(cfg: &Config) -> Levers {
        let policy = &cfg.policy;
        Levers {
            // Real economy E36 (phase 3a): the market wage law opens at
            // `[treasury] tax_initial` (P10: under the market law, the levy
            // not enacted; was `no_net`).
            tax_rate: if !policy.levy && cfg.treasury.tax_initial >= 0.0 {
                cfg.treasury.tax_initial
            } else {
                cfg.levers.tax_rate
            },
            sentence_mult: cfg.levers.sentence_mult,
            guard_count: cfg.levers.guard_count,
            immigration_per_week: cfg.levers.immigration_per_week,
            // P10: the dole at its rate only where the policy enacts it (the
            // shipped city seeds 0, Real economy E22 (a)).
            dole_per_day: if policy.dole { cfg.levers.dole_per_day } else { 0 },
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
            // P10: public works as the policy enacts them (Real economy E22
            // (b): none in the shipped city).
            public_works: policy.public_works,
            // Real economy (plan E4, E12): the World market buys from seed;
            // `SetExport` closes and reopens it.
            export_open: true,
            leisure_tax: 0.0,
            ban_gambling: false,
            fixer_licence: true,
            prison_permit: true,
            accessory_mult: None,
            public_bounties: false,
            levy: policy.levy,
            school_meals: policy.school_meals,
            reserve_release: policy.reserve_release,
            vice: ViceLevers::from_config(cfg),
            faith: FaithLevers::default(),
            public_school: false,
        }
    }
}
