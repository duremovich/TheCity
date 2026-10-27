//! The event log: a ring of 50,000 entries, never drained. The UI keeps a
//! read cursor; every emergent event appends exactly one entry. Each entry
//! has a contiguous id (M10), so the binder can rewrite the entry of a hole
//! it attributes in O(1).

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::components::{
    hole_id, Bound, Corpse, DeathCause, Hole, HoleId, HoleKind, Identity, Life, LifeEvent, LifeKind, LIFE_CAP,
};
use crate::entity::EntityId;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

pub const EVENT_RING_CAP: usize = 50_000;
/// `PlanAborted` goes to its own small ring, `World::debug_events`, not saved
/// (M10 phase 5b): at 2,000 residents it was ~1,000 a day and pushed every
/// story event out of the 50,000 within two months.
pub const DEBUG_RING_CAP: usize = 2_000;
/// The `id` of an event in the debug ring: it takes no place in the story
/// ring's contiguous numbering.
pub const DEBUG_EVENT_ID: u64 = u64::MAX;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Contiguous over the run (M10 D12).
    #[serde(default)]
    pub id: u64,
    pub tick: Tick,
    pub kind: EventKind,
    pub actors: SmallVec<[EntityId; 3]>,
    pub text: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum EventKind {
    Theft,
    Extortion,
    Assault,
    Murder,
    Witness,
    Report,
    Arrest,
    Sentence,
    Release,
    Unpunished,
    Birth,
    Death,
    Burial,
    Rotted,
    Proposal,
    Marriage,
    GangJoin,
    GangLeave,
    Betrayal,
    Hire,
    Fire,
    Quit,
    Immigration,
    Emigration,
    Starving,
    Homeless,
    PlanAborted,
    PriceChange,
    Restock,
    Inheritance,
    PlayerAction,
    PlayerActionFailed,
    /// M8 factions.
    OrderChanged,
    TerritoryFlipped,
    Raid,
    Disobeyed,
    Sacked,
    /// M9 the law.
    Jailbreak,
    Posture,
    Bribe,
    /// M10 off-screen lives: victim-side crimes whose actor is a hole until
    /// bound (`actors[0]` is `EntityId::NONE` until then), and the binding.
    Robbed,
    Assaulted,
    Attributed,
    /// M11 ownership and corps (`[evictee, home, owner?]`, `[agent, home]`, ...).
    Evicted,
    RentShort,
    Housed,
    Founded,
    Incorporated,
    Bankrupt,
    Acquired,
    BrokenUp,
    CorpOrder,
    Strike,
    Contract,
    /// M12 districts: a district's controller changed (`[old?, new?]`).
    DistrictControl,
    /// M12 D12: a district's stance changed (`[jail, target gang?]`).
    Stance,
    /// M12 D15: a rough sleeper fined or jailed for the night.
    Vagrancy,
    /// M12 D23: the sweepers' allocation across districts changed.
    Sanitation,
    /// M12 D27: a derelict building taken by a squatter (`[squatter, building]`).
    Squatted,
    /// M12 D27: a squatter put out (`[squatter, building]`).
    SquatEvicted,
    /// M12 D25: a building went derelict (`[building, old owner?]`), or was re-let.
    Derelict,
    /// M12 D32: a district riot clashed, dispersed or fizzled (`[target, actor?]`).
    Riot,
    /// M12 D32: a won riot looted its target (`[target, district's riot actor]`).
    Looted,
    /// M12 D35: a bystander hit in a brawl at a door (`[attacker, bystander]`).
    Crossfire,
    /// M12 D36: a gang split (`[old gang, splinter, lieutenant]`).
    Split,
    /// M13 (spec § 9, plan "Events"): `[buyer, seller building, asset]`.
    AssetBought,
    /// M13 D11/D12: `[owner?, lender?, asset]` (NONE for the city).
    Repossessed,
    /// M13: `[asset, owner?]`.
    Wrecked,
    /// M13 (phase 2): `[thief or NONE, owner?, asset]`.
    VehicleStolen,
    /// M13 (phase 2): `[gang, asset]`.
    Chopped,
    /// M13 (phase 2): `[driver, victim?, asset]`.
    Crash,
    /// M13 (phase 3): `[agent, clinic, asset]`.
    Installed,
    /// M13 (phase 3): `[agent]`.
    Episode,
    /// M13 (phase 3): `[agent, clinic]`.
    Treated,
    /// M13 (phase 3): `[abductor or NONE, victim]`.
    Abducted,
    /// M13 (phase 3): `[ripper or gang, body]`.
    Harvested,
    /// M13 D15/D35: `[stripper or gang or NONE, corpse]`.
    Stripped,
    /// M13 (phase 4): `[agent]`.
    Overdose,
    // --- M14 (plan V43; violet). Actors per the plan's Events table.
    /// `[runner, chair]` (phase 2).
    JackedIn,
    /// `[runner, owner or NONE, patron or NONE]` (phase 2).
    DataStolen,
    /// `[runner or NONE, owner]`.
    DataWiped,
    /// `[seller, buyer corp]` (phase 2).
    DataSold,
    /// `[runner, owner or NONE]` (phase 2).
    LedgerHacked,
    /// `[runner, gang, building]` (phase 3).
    DoorHacked,
    /// `[runner, building]` (phase 3).
    RobotTurned,
    /// `[runner, building]` (phase 3).
    Blinded,
    /// `[runner, owner or NONE, chair]` (phase 2).
    Traced,
    /// `[runner, owner or NONE]` (phase 2).
    Fried,
    /// `[runner, owner or NONE]` (phase 2).
    Flatlined,
    /// `[runner]` (phase 2).
    Dumpshock,
    /// `[owner or NONE, building or NONE]`.
    IceRaised,
    /// `[owner or NONE, building or NONE]`.
    IceLowered,
    /// `[corp]`.
    TechGained,
    /// `[corp]`.
    TechLost,
    // --- M15 (plan W43; crimson). Phase 2.
    /// `[poacher corp, agent, old corp]`.
    Poached,
    /// `[group, gone or NONE]`.
    TalentLost,
    /// `[agent, gang]`.
    Expelled,
    /// `[actor, home]`: a failed Intimidate's Shakedown.
    Refused,
    // --- M15 phase 3 (plan W43; crimson).
    /// `[holder, target]`.
    GrudgeFormed,
    /// `[hunter, target]`.
    HuntStarted,
    /// `[hunter, target]`.
    HuntAbandoned,
    /// `[hunter, target]`.
    Avenged,
    /// `[a, b]`.
    Vendetta,
    /// `[a, b]`.
    VendettaEnded,
    /// `[seller, client building, new seller]`.
    ContractLost,
    /// `[hunter, respondent]`.
    Deceived,
    // --- M15 phase 4 (plan W43; crimson).
    /// `[feed, actor, object or NONE]`.
    Story,
    /// `[corp, feed]`.
    Planted,
    /// `[corp, feed]`.
    Buried,
    // --- Life pass L2 phase 1 (plan L35; amber).
    /// `[owner?, building]`: a derelict Block refitted as a leisure kind.
    Refit,
    /// `[]`: the World account's purchases today (one a day, the totals).
    Exported,
    /// `[recycler]`: the budget band posted public-works jobs.
    WorksPosted,
    // --- Life pass L2 phase 2 (plan L35; amber).
    /// `[winner, house]`: a win above `[leisure] big_win`.
    Gambled,
    /// `[winner, loser, pit]`: a FightPit's nightly bout.
    Bout,
    /// `[leader, gang]`: the weekly Collect paid the members.
    Collected,
    /// `[preacher, listener]`: a Purist's Preach landed.
    Preached,
    /// `[leader, gang]`: the leader's Call after an order change.
    Called,
    // --- M16a (plan C37; amber): contract records, a game abstraction.
    /// `[agent or NONE, target, broker or NONE]`: a record put on the board.
    ContractPosted,
    /// `[taker, target]`: a record taken.
    ContractTaken,
    /// `[taker or NONE, target]`: a record fulfilled.
    ContractFulfilled,
    /// `[taker or NONE, target]`: an attempt failed (or the record did).
    ContractFailed,
    /// `[taker or NONE, target]`: a record ran past its deadline.
    ContractExpired,
    /// `[buyer, taker]`: a direct buyer did not pay.
    Reneged,
    /// `[gang, target, previous taker]` (phase 2).
    SoldOut,
    /// `[taker, target]` (phase 2).
    StrikeDeclined,
    /// `[writer, target]`: a sighting paid under a Locate record.
    BountyPaid,
    /// `[agent, target, holder]` (phase 3).
    Accessory,
    /// `[fixer, owner]` (phase 3).
    FixerBusted,
    /// `[guard, buyer]` (phase 3).
    GuardTaken,
    // --- The Real economy phase 1 (plan E44; amber): the World market.
    /// `[market]`: a Market bought food from the World at the ask.
    Imported,
    /// `[]`: a good's appetite walk crossed ±0.25 from 1.0.
    AppetiteShift,
    // --- The Real economy phase 3b/3c (plan E44; amber).
    /// `[donor or NONE, mission]`: a gift of 5 or more into a Mission's purse.
    Donated,
    /// `[mission]`: the day's meals and cots (one a day per Mission).
    MissionServed,
    /// `[child, parent, parent]`: the child protective service took a child
    /// to a work camp (a Life row for the child and each parent).
    ChildTaken,
    /// `[adult, camp]`: released from a camp at adulthood (a Life row).
    CampRaised,
    /// `[camp, owner or NONE]`: the law closed a camp that could not feed.
    CampClosed,
    // --- The Real economy phase 2 (plan E44; amber): wages from revenue.
    /// `[corp]`: a corp's `wage_rev` crossed a 0.1 step.
    WageMoved,
    /// `[agent, building]`: the revenue rule let the newest hire go.
    LaidOff,
    // --- The Real economy phase 3a (plan E21, E44; amber).
    /// `[]`: the Treasury's tax band moved the tax rate a step.
    TaxMoved,
    // --- Jobs and room P2 (plan J8; amber).
    /// `[payer?, building]`: a storey added (`founding::add_floor`).
    FloorAdded,
    // --- Jobs and room P3 (plan J14; amber).
    /// `[super, post]`: a Super walked the round of his Blocks (`trades::round_plan`).
    SuperRound,
    // --- Hunger as sickness (docs/HUNGER_SICKNESS.md § 9).
    /// `[winner, loser, bin]` or `[gang, diver]`: a bin's regular saw a diver
    /// off, or a gang shook one down.
    BinContest,
    /// `[agent]`: a cancer turned symptomatic.
    Diagnosed,
    /// `[agent, clinic]`: a cancer cured at a Clinic.
    Cured,
    // --- Class pay (docs/CLASS_PAY.md).
    /// `[agent, workplace]`: promoted to a higher-graded post (skilled in
    /// place, or a professional/management post, or a corp's exec).
    Promoted,
    // --- The Hit market (docs/HIT_MARKET.md H6; amber).
    /// `[buyer, taker]`: a buyer on credit could not pay the balance.
    Stiffed,
    /// `[witness, suspect]`: a beaten witness took back the report (H15).
    Recanted,
    /// `[suspect, witness]`: the report's witness is dead and no guard saw
    /// it: the case goes cold (H14).
    CaseCold,
    // --- P10 (docs/P10_POLICY.md; amber).
    /// `[]`: the Treasury could not pay the safety net for `[policy]
    /// lapse_days` days in a row: the dole and public works lapsed.
    PolicyLapsed,
    // --- Scale S5 (docs/SCALE_20K.md § S5).
    /// `[adult, spouse?, home]`: an Ambient household entered the simulation.
    Materialised,
    // --- M16b § 4 leverage (phase 1; amber).
    /// `[agent, over, holder?]`: a coercion made (or a fake job sold).
    Coerced,
    /// `[over, agent]`: a standing coercion refused or broken.
    CoercionBroken,
    /// `[agent, over]`: a blackmail lapsed: its secret went public.
    Lapsed,
    /// `[leader, rival leader]`: a Parley made peace for a tribute.
    Parleyed,
    /// `[victim, faker]`: a fake job's buyer learned it never existed.
    Defrauded,
    // --- M16b phase 2: captives and stories (amber).
    /// `[captive, holder agent, building]`: a captive handed over.
    HostageTaken,
    /// `[captive, building]`: a captive let go (a demand met, a rescue, an
    /// expiry, the hold gone).
    HostageFreed,
    /// `[agent, captive]`: a captive killed at the hold's end.
    HostageKilled,
    /// `[holder, feed]`: a fabricated story found out.
    Exposed,
    // --- M16b phase 3: the wounded (amber).
    /// `[downed, attacker]`: a fight's loser down, bleeding.
    Wounded,
    /// `[dead, attacker]`: a downed agent died before it was stabilised.
    BledOut,
    /// `[agent]`: a stabilised agent back on its feet.
    Recovered,
    /// `[carrier, downed, clinic]`: a downed agent stabilised at a Clinic.
    Rescued,
    /// `[corp, client]`: a Trauma client's beacon called its crew.
    TraumaCall,
    /// `[corp, client]`: a Trauma client stabilised by its crew in time.
    TraumaSave,
    // --- M16b phase 4: boards, tags, the Patrol (amber).
    /// `[corp, chair]`: a board's vote, with its tally.
    VoteHeld,
    /// `[member, corp]`: a pinned member voted against its own preference.
    VoteSwung,
    /// `[carrier, owner]`: a tag planted.
    Tagged,
    /// `[carrier, scanner building]`: a tag found and destroyed.
    TagFound,
    /// `[member, gang]`: a Purist Patrol's scan at a hang-out.
    PatrolScan,
    /// M17 phase 2: `[branch]`: a parent sent coins to a weak branch.
    Subsidised,
    /// M17 phase 2: a parent's income stayed under its upkeep for a week.
    ParentFailing,
    /// M17 phase 2: a parent changed its order.
    OutsideOrder,
    /// M17 phase 3: bodies.
    Reinforced,
    OutsidersArrived,
    OutsiderLeft,
    Settled,
    BuyBackOffered,
    BranchRetook,
    BranchFounded,
    ParentFounded,
    Abandoned,
    BrothersArrived,
    GangRefounded,
    StateGrant,
    StateMandate,
    StateWithdrew,
    /// M17 phase 4: the weapons that reach the ledger, and scrip.
    UplinkBreached,
    DataShipped,
    Sanctioned,
    CampaignPosted,
    ParentDied,
    ScripWiped,
    /// M17 phase 4b: the succession. `Succession [new leader, gang, fallen]`
    /// (a promotion), `CivilWar [heir, challenger, gang]`, `TookSides
    /// [member, claimant, gang]`, `CivilWarEnded [winner, loser, gang]`.
    Succession,
    CivilWar,
    TookSides,
    CivilWarEnded,
    /// M17 phase 5: `AbroadDeparted [crew..]`, `AbroadResolved [crew..]`,
    /// `CastArrived [arrival, toward]` (a promoted cast member),
    /// `BlackwallBreach [runner, patron]`.
    AbroadDeparted,
    AbroadResolved,
    CastArrived,
    BlackwallBreach,
    // --- Jobs & lives phase 1 (docs/JOBS_AND_LIVES.md § 2).
    /// A Utility Plant's districts went dark (no operator for `outage_hours`).
    PowerOut,
    /// ... and the lights came back.
    PowerBack,
    /// A build paid to a Construction Yard stands (its site opens).
    BuildDone,
    /// A Loader walked out with a Ration.
    Pilfered,
    // --- Jobs & lives phase 2 (§ 3), the street.
    /// A jobless adult opened a Pitch.
    PitchOpened,
    /// A Pitch closed (a hire, a jail, no take).
    PitchClosed,
    /// A Pitch swept in a Sweep district (a fine, closed a day).
    PitchSwept,
    /// The labour corner's dawn pick.
    DawnPick,
    /// A tinker's repair went wrong (the vehicle wrecked).
    TinkerBotched,
    // --- Jobs & lives phase 3 (§ 4), weapons.
    /// A smuggled crate passed the Freight Yard into a gang's armoury.
    CrateLanded,
    /// ... or customs seized it into the Precinct's stock.
    CrateSeized,
    /// The Civic Clerk sold a weapons licence.
    LicenceSold,
    // --- Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1.4).
    /// `[server, served, place]`: a service encounter's notable outcome (a
    /// door refusal, an ejection, a new regular), only for the outcomes a
    /// `[[serves]]` row's `event_on` names.
    Served,
    // --- Lives at work phase 2 (docs/LIVES_AT_WORK.md § 3.5).
    /// `[visitor, kin, place?]`: a visit made to a hurt kin or an elder parent.
    Visited,
    // --- Lives at work phase 3 (docs/LIVES_AT_WORK.md § 2.2, § 3.5).
    /// `[child, camp, home?]`: a Camp child slipped out (`p_camp_escape`).
    CampEscape,
    /// `[parent, child, camp]`: a parent bought a child out of a Camp.
    CampBuyout,
    /// `[parent, child, camp]`: a parent (and his crew) broke a child out of a Camp.
    CampBreakout,
    /// `[super, tenant, block]`: a super served an arrears notice at the door.
    Notice,
    // --- Lives at work phase 4 (docs/LIVES_AT_WORK.md § 2.3).
    /// `[rider, driver, garage]`: a cab fare dodged (the rider rode short).
    FareShort,
    // --- Lives at work phase 6 (docs/LIVES_AT_WORK.md § 4.4).
    /// `[worker, employer]`: a worker retired (on savings or to kin).
    Retired,
    // --- Lives at work phase 7 (docs/LIVES_AT_WORK.md § 5.2).
    /// `[prisoner, station, precinct]`: the night van took a held body from
    /// a Station's cells to the Precinct.
    Transferred,
    // --- The pre-view pass phase 6 (docs/PREVIEW_PASS.md § 0 P13-P15).
    /// `[leader, owner, business]`: a week's protection collected in person
    /// on the leader's round.
    Protected,
    /// `[leader, lieutenant, gang]`: an order given in person (at court, or
    /// at the lieutenant after the wait).
    OrderGiven,
    /// `[member, leader, gang]`: a member's take handed to the leader at court.
    TakeHanded,
    /// The pressure pass phase 1 (OPEN-178): `[worker, employer]`, a shift
    /// skipped in a miserable spell (`systems::misery`).
    Skipped,
    /// The pressure pass phase 1 (OPEN-178): `[starter, mark]`, a brawl a
    /// miserable, drunk or high body started (`systems::misery`).
    Brawl,
    /// The pressure pass phase 6 (`systems::underwater`): `[owner, business]`,
    /// an owner under water hangs on, cuts the hours, shuts the business,
    /// is afloat again; `[gang, front]` a front's laundering.
    UnderWater,
    /// Phase 6: `[borrower, lender, business]`, a loan made, an installment
    /// missed, a collector's call (`[collector, borrower, lender]`), repaid.
    Loan,
    /// Phase 6: `[worker, owner, place?]`, a worker owed wages confronts the
    /// owner in person (paid, refused, a fight, a report, a garnishment).
    Confronted,
    /// The pressure pass phase 3 (`systems::protection`): `[enforcer, owner,
    /// business, gang]`, a refused or missed protection punished in person
    /// (the till smashed, the owner beaten, a fire), and `[owner, gang,
    /// business]` the owner's choice after (pays, reports, a rival's cover,
    /// sells or shuts); `[member, gang]` a member bailed out.
    Punished,
    /// The pressure pass phase 5a (`systems::consequence`): `[witness,
    /// culprit]`, a witness fled, told his people, or kept it to sell; the
    /// culprit saw who saw him.
    WitnessReacted,
    /// Phase 5a: `[witness, culprit, place?]`, a blackmailer's demand in
    /// person (paid, refused and silenced, refused and reported).
    Blackmail,
    /// Phase 5a: `[buyer, target, fixer?]`, an unfilled Hit or Beat: reposted
    /// higher, the buyer goes himself, or he lets it go.
    HitEscalated,
    /// Phase 5a: `[attendee, preacher, place]`, a creed's service: preached,
    /// heard, the plate, a guest converted, a congregant walked out.
    Service,
    /// Phase 5a: `[prisoner, other?]`, the cells (a recruit, a Fixer's name, a
    /// fight, a loan, a missed shift) and the release into a changed life.
    Cells,
    /// The pressure pass phase 5b (`systems::crew`): `[member, gang, place]`
    /// a duty kept, missed or chosen against the shift, a court held or
    /// failed, a rest day with the crew, a welcome back, the colours taken,
    /// the talk inside; `[gang, buyer, members..]` a gang absorbed or folded.
    Crew,
    /// Pressure pass round 2, R1 (`systems::fares`): `[dodger, guard?]` a
    /// fare dodger caught at the El (fined, beaten), banned, a ban broken,
    /// up off the platform.
    Fare,
}

impl EventKind {
    pub const ALL: [EventKind; 238] = [
        EventKind::Theft,
        EventKind::Extortion,
        EventKind::Assault,
        EventKind::Murder,
        EventKind::Witness,
        EventKind::Report,
        EventKind::Arrest,
        EventKind::Sentence,
        EventKind::Release,
        EventKind::Unpunished,
        EventKind::Birth,
        EventKind::Death,
        EventKind::Burial,
        EventKind::Rotted,
        EventKind::Proposal,
        EventKind::Marriage,
        EventKind::GangJoin,
        EventKind::GangLeave,
        EventKind::Betrayal,
        EventKind::Hire,
        EventKind::Fire,
        EventKind::Quit,
        EventKind::Immigration,
        EventKind::Emigration,
        EventKind::Starving,
        EventKind::Homeless,
        EventKind::PlanAborted,
        EventKind::PriceChange,
        EventKind::Restock,
        EventKind::Inheritance,
        EventKind::PlayerAction,
        EventKind::PlayerActionFailed,
        EventKind::OrderChanged,
        EventKind::TerritoryFlipped,
        EventKind::Raid,
        EventKind::Disobeyed,
        EventKind::Sacked,
        EventKind::Jailbreak,
        EventKind::Posture,
        EventKind::Bribe,
        EventKind::Robbed,
        EventKind::Assaulted,
        EventKind::Attributed,
        EventKind::Evicted,
        EventKind::RentShort,
        EventKind::Housed,
        EventKind::Founded,
        EventKind::Incorporated,
        EventKind::Bankrupt,
        EventKind::Acquired,
        EventKind::BrokenUp,
        EventKind::CorpOrder,
        EventKind::Strike,
        EventKind::Contract,
        EventKind::DistrictControl,
        EventKind::Stance,
        EventKind::Vagrancy,
        EventKind::Sanitation,
        EventKind::Squatted,
        EventKind::SquatEvicted,
        EventKind::Derelict,
        EventKind::Riot,
        EventKind::Looted,
        EventKind::Crossfire,
        EventKind::Split,
        EventKind::AssetBought,
        EventKind::Repossessed,
        EventKind::Wrecked,
        EventKind::VehicleStolen,
        EventKind::Chopped,
        EventKind::Crash,
        EventKind::Installed,
        EventKind::Episode,
        EventKind::Treated,
        EventKind::Abducted,
        EventKind::Harvested,
        EventKind::Stripped,
        EventKind::Overdose,
        EventKind::JackedIn,
        EventKind::DataStolen,
        EventKind::DataWiped,
        EventKind::DataSold,
        EventKind::LedgerHacked,
        EventKind::DoorHacked,
        EventKind::RobotTurned,
        EventKind::Blinded,
        EventKind::Traced,
        EventKind::Fried,
        EventKind::Flatlined,
        EventKind::Dumpshock,
        EventKind::IceRaised,
        EventKind::IceLowered,
        EventKind::TechGained,
        EventKind::TechLost,
        EventKind::Poached,
        EventKind::TalentLost,
        EventKind::Expelled,
        EventKind::Refused,
        EventKind::GrudgeFormed,
        EventKind::HuntStarted,
        EventKind::HuntAbandoned,
        EventKind::Avenged,
        EventKind::Vendetta,
        EventKind::VendettaEnded,
        EventKind::ContractLost,
        EventKind::Deceived,
        EventKind::Story,
        EventKind::Planted,
        EventKind::Buried,
        EventKind::Refit,
        EventKind::Exported,
        EventKind::WorksPosted,
        EventKind::Gambled,
        EventKind::Bout,
        EventKind::Collected,
        EventKind::Preached,
        EventKind::Called,
        EventKind::ContractPosted,
        EventKind::ContractTaken,
        EventKind::ContractFulfilled,
        EventKind::ContractFailed,
        EventKind::ContractExpired,
        EventKind::Reneged,
        EventKind::SoldOut,
        EventKind::StrikeDeclined,
        EventKind::BountyPaid,
        EventKind::Accessory,
        EventKind::FixerBusted,
        EventKind::GuardTaken,
        EventKind::Imported,
        EventKind::AppetiteShift,
        EventKind::Donated,
        EventKind::MissionServed,
        EventKind::ChildTaken,
        EventKind::CampRaised,
        EventKind::CampClosed,
        EventKind::WageMoved,
        EventKind::LaidOff,
        EventKind::TaxMoved,
        EventKind::FloorAdded,
        EventKind::SuperRound,
        EventKind::BinContest,
        EventKind::Diagnosed,
        EventKind::Cured,
        EventKind::Promoted,
        EventKind::Stiffed,
        EventKind::Recanted,
        EventKind::CaseCold,
        EventKind::PolicyLapsed,
        EventKind::Materialised,
        EventKind::Coerced,
        EventKind::CoercionBroken,
        EventKind::Lapsed,
        EventKind::Parleyed,
        EventKind::Defrauded,
        EventKind::HostageTaken,
        EventKind::HostageFreed,
        EventKind::HostageKilled,
        EventKind::Exposed,
        EventKind::Wounded,
        EventKind::BledOut,
        EventKind::Recovered,
        EventKind::Rescued,
        EventKind::TraumaCall,
        EventKind::TraumaSave,
        EventKind::VoteHeld,
        EventKind::VoteSwung,
        EventKind::Tagged,
        EventKind::TagFound,
        EventKind::PatrolScan,
        EventKind::Subsidised,
        EventKind::ParentFailing,
        EventKind::OutsideOrder,
        EventKind::Reinforced,
        EventKind::OutsidersArrived,
        EventKind::OutsiderLeft,
        EventKind::Settled,
        EventKind::BuyBackOffered,
        EventKind::BranchRetook,
        EventKind::BranchFounded,
        EventKind::ParentFounded,
        EventKind::Abandoned,
        EventKind::BrothersArrived,
        EventKind::GangRefounded,
        EventKind::StateGrant,
        EventKind::StateMandate,
        EventKind::StateWithdrew,
        EventKind::UplinkBreached,
        EventKind::DataShipped,
        EventKind::Sanctioned,
        EventKind::CampaignPosted,
        EventKind::ParentDied,
        EventKind::ScripWiped,
        EventKind::Succession,
        EventKind::CivilWar,
        EventKind::TookSides,
        EventKind::CivilWarEnded,
        EventKind::AbroadDeparted,
        EventKind::AbroadResolved,
        EventKind::CastArrived,
        EventKind::BlackwallBreach,
        EventKind::PowerOut,
        EventKind::PowerBack,
        EventKind::BuildDone,
        EventKind::Pilfered,
        EventKind::PitchOpened,
        EventKind::PitchClosed,
        EventKind::PitchSwept,
        EventKind::DawnPick,
        EventKind::TinkerBotched,
        EventKind::CrateLanded,
        EventKind::CrateSeized,
        EventKind::LicenceSold,
        EventKind::Served,
        EventKind::Visited,
        EventKind::CampEscape,
        EventKind::CampBuyout,
        EventKind::CampBreakout,
        EventKind::Notice,
        EventKind::FareShort,
        EventKind::Retired,
        EventKind::Transferred,
        EventKind::Protected,
        EventKind::OrderGiven,
        EventKind::TakeHanded,
        EventKind::Skipped,
        EventKind::Brawl,
        EventKind::UnderWater,
        EventKind::Loan,
        EventKind::Confronted,
        EventKind::Punished,
        EventKind::WitnessReacted,
        EventKind::Blackmail,
        EventKind::HitEscalated,
        EventKind::Service,
        EventKind::Cells,
        EventKind::Crew,
        EventKind::Fare,
    ];
}

impl World {
    /// Append one event at the current tick, evicting the oldest past the cap.
    /// Returns the event's id.
    /// `PlanAborted` goes to the debug ring instead and returns `DEBUG_EVENT_ID`.
    pub fn push_event(&mut self, kind: EventKind, actors: &[EntityId], text: impl Into<String>) -> u64 {
        if kind == EventKind::PlanAborted {
            if self.debug_events.len() >= DEBUG_RING_CAP {
                self.debug_events.pop_front();
            }
            let actors = SmallVec::from_slice(actors);
            self.debug_events.push_back(Event { id: DEBUG_EVENT_ID, tick: self.tick, kind, actors, text: text.into() });
            return DEBUG_EVENT_ID;
        }
        if self.events.len() >= EVENT_RING_CAP {
            self.events.pop_front();
        }
        let id = self.next_event_id;
        self.next_event_id += 1;
        let event = Event { id, tick: self.tick, kind, actors: SmallVec::from_slice(actors), text: text.into() };
        record_life(self, &event);
        // OPEN-140: a move's body, for the midnight's one writer and the walk home.
        if kind == EventKind::Housed {
            crate::systems::life::on_housed(self, actors.first().copied(), actors.get(1).copied());
        }
        self.events.push_back(event);
        id
    }

    /// The ring entry with this id, if it has not been evicted. O(1): ids
    /// are contiguous in the ring.
    pub fn event_mut(&mut self, id: u64) -> Option<&mut Event> {
        let first = self.events.front()?.id;
        let i = usize::try_from(id.checked_sub(first)?).ok()?;
        self.events.get_mut(i).filter(|e| e.id == id)
    }

    /// Events whose `actors` contain `id`, newest first.
    pub fn events_for(&self, id: EntityId) -> impl Iterator<Item = &Event> {
        self.events.iter().rev().filter(move |e| e.actors.contains(&id))
    }
}

// ---------------------------------------------------------------------------
// Lives (M10 phase 4): the one table that writes `Life`
// ---------------------------------------------------------------------------

/// What `life_kind` says an event did to the agent in `slot`.
struct LifeRow {
    kind: LifeKind,
    other: Option<EntityId>,
    hole: Option<HoleId>,
}

/// The table (M10 D36). Actor order per push site:
/// `Birth [child, mother, father]`, `Marriage [a, b]`, `Death [dead, spouse?]`,
/// `Death [dead, spouse or NONE, killer]` when the killer is known,
/// `Hire [id, employer]`, `Fire/Quit/Starving/Theft/Release/Homeless/Immigration/Betrayal [id, ..]`,
/// `Robbed/Assaulted [NONE, victim]`, `Assault/Murder [attacker, victim]`,
/// `Extortion [actor, home]`, `Arrest [guard, suspect]`, `Jailbreak [gang, jail, freed..]`,
/// `GangJoin/GangLeave [id, gang]`, `Burial [digger, corpse]`, `Witness [witness, actor]`.
fn life_kind(world: &World, event: &Event, slot: usize, actor: EntityId) -> Option<LifeRow> {
    use EventKind as E;
    let other = event.actors.iter().copied().find(|&o| o != actor && o != EntityId::NONE && world.has::<Identity>(o));
    let row = |kind| Some(LifeRow { kind, other, hole: None });
    let victim_side = |kind: LifeKind, hole_kind: HoleKind| {
        // A victim-side event whose actor is still `NONE` is a hole.
        let hole =
            event.actors.first().is_some_and(|a| *a == EntityId::NONE).then(|| hole_id(event.tick, actor, hole_kind));
        Some(LifeRow { kind, other, hole })
    };
    match (event.kind, slot) {
        (E::Birth, 0) => row(LifeKind::Born),
        (E::Marriage, _) => row(LifeKind::Married),
        (E::Death, 0) => {
            let violent = world.comp::<Corpse>(actor).is_some_and(|c| c.cause == DeathCause::Violence);
            // The killer, when `kill_by` named one.
            let killer = event.actors.get(2).copied().filter(|&k| k != EntityId::NONE && violent);
            Some(LifeRow { kind: if violent { LifeKind::Killed } else { LifeKind::Died }, other: killer, hole: None })
        }
        (E::Death, 1) => row(LifeKind::Widowed),
        (E::Hire, 0) => row(LifeKind::Hired),
        (E::Fire, 0) => row(LifeKind::Fired),
        // Real economy phase 2 (plan E44): a layoff on the revenue rule.
        (E::LaidOff, 0) => row(LifeKind::Fired),
        (E::Quit, 0) => row(LifeKind::Quit),
        // Class pay: a promotion is a hire into the new post.
        (E::Promoted, 0) => row(LifeKind::Hired),
        (E::Starving, 0) => row(LifeKind::Starving),
        (E::Theft, 0) => row(LifeKind::Stole),
        (E::Robbed, 1) => victim_side(LifeKind::Robbed, HoleKind::Robbed),
        (E::Assaulted, 1) => victim_side(LifeKind::Assaulted, HoleKind::Assaulted),
        (E::Assault, 0) => row(LifeKind::AssaultedSomeone),
        (E::Assault, 1) => row(LifeKind::Assaulted),
        (E::Murder, 0) => row(LifeKind::KilledSomeone),
        (E::Murder, 1) => victim_side(LifeKind::Killed, HoleKind::Killed),
        (E::Extortion, 0) => row(LifeKind::RobbedSomeone),
        (E::Arrest, 1) => row(LifeKind::Arrested),
        (E::Release, 0) => row(LifeKind::Released),
        // The gang and the Jail come first; the freed are the rest.
        (E::Jailbreak, s) if s >= 2 => row(LifeKind::Escaped),
        (E::GangJoin, 0) => row(LifeKind::JoinedGang),
        (E::GangLeave, 0) => row(LifeKind::LeftGang),
        (E::Betrayal, 0) => row(LifeKind::Betrayed),
        (E::Homeless, 0) => row(LifeKind::Evicted),
        // M11 (plan D39): `Evicted [evictee, home, owner?]`, `Housed [agent,
        // home]`, `Founded [founder, ..]`, `Incorporated [corp, exec]`.
        (E::Evicted, 0) => row(LifeKind::Evicted),
        (E::Housed, 0) => row(LifeKind::Housed),
        (E::Founded, 0) => row(LifeKind::Founded),
        (E::Incorporated, 1) => row(LifeKind::Incorporated),
        // M11 phase 3: corp events name corps and buildings; no biography row.
        (E::CorpOrder | E::Bankrupt | E::Acquired | E::BrokenUp | E::Contract, _) => None,
        (E::Immigration, 0) => row(LifeKind::Immigrated),
        (E::Burial, 1) => row(LifeKind::Buried),
        (E::Witness, 0) => row(LifeKind::Witnessed),
        // M14 V14: `Flatlined [runner, owner or NONE]`.
        (E::Flatlined, 0) => row(LifeKind::Flatlined),
        // Real economy E38, E41: `ChildTaken [child, parents..]`, `CampRaised [adult, camp]`.
        (E::ChildTaken, 0) => row(LifeKind::Taken),
        (E::ChildTaken, _) => row(LifeKind::ChildTaken),
        (E::CampRaised, 0) => row(LifeKind::CampRaised),
        _ => None,
    }
}

/// Append the event to the biography of every agent in `actors` that the
/// table has a row for. Per event, never per tick.
fn record_life(world: &mut World, event: &Event) {
    for (slot, &actor) in event.actors.iter().enumerate() {
        if actor == EntityId::NONE || !world.has::<Identity>(actor) {
            continue;
        }
        let Some(row) = life_kind(world, event, slot, actor) else { continue };
        let entry = LifeEvent {
            tick: event.tick,
            kind: row.kind,
            other: row.other,
            hole: row.hole,
            salience: row.kind.salience(),
        };
        add_life(world, actor, entry);
    }
}

/// Close a hole's biography entries (M10 D36): the victim's entry stops
/// reading "unknown" and names the actor; the actor's gains the crime at its
/// original tick with salience 1.0, so it is never evicted.
pub fn life_bound(world: &mut World, hole: &Hole, bound: Bound) {
    let actor = match bound {
        Bound::Actor(a) => Some(a),
        Bound::Unknown => None,
    };
    if let Some(life) = world.comp_mut::<Life>(hole.victim) {
        for e in life.events.iter_mut().filter(|e| e.hole == Some(hole.id)) {
            e.hole = None;
            e.other = actor;
        }
    }
    if let Some(a) = actor {
        let kind = match hole.kind {
            HoleKind::Robbed => LifeKind::RobbedSomeone,
            HoleKind::Assaulted => LifeKind::AssaultedSomeone,
            // M13 D37: an abductee is dead by the time the hole binds.
            HoleKind::Killed | HoleKind::Abducted => LifeKind::KilledSomeone,
        };
        add_life(world, a, LifeEvent { tick: hole.tick, kind, other: Some(hole.victim), hole: None, salience: 1.0 });
    }
}

/// Insert in tick order; a permanent kind already recorded this tick is
/// completed rather than doubled (a Death and its Murder both say Killed);
/// past `LIFE_CAP` the lowest `salience × recency` evictable entry goes.
fn add_life(world: &mut World, id: EntityId, entry: LifeEvent) {
    if !world.has::<Life>(id) {
        world.insert(id, Life::default());
    }
    let now = world.tick;
    let half_life = world.config.brain.memory_half_life_days.max(0.01);
    let Some(life) = world.comp_mut::<Life>(id) else { return };
    if entry.kind.permanent() {
        if let Some(e) = life.events.iter_mut().find(|e| e.kind == entry.kind && e.tick == entry.tick) {
            e.other = e.other.or(entry.other);
            e.hole = e.hole.or(entry.hole);
            return;
        }
    }
    if life.events.len() >= LIFE_CAP {
        let weight = |e: &LifeEvent| {
            let age_days = now.saturating_sub(e.tick) as f32 / TICKS_PER_DAY as f32;
            e.salience * 0.5f32.powf(age_days / half_life)
        };
        // Lowest weight; the oldest on a tie (the list is oldest first).
        let worst = life.events.iter().enumerate().filter(|(_, e)| !e.kind.permanent() && e.salience < 1.0).fold(
            None::<(usize, f32)>,
            |acc, (i, e)| {
                let w = weight(e);
                match acc {
                    Some((_, best)) if best <= w => acc,
                    _ => Some((i, w)),
                }
            },
        );
        if let Some((i, _)) = worst {
            life.events.remove(i);
        }
        // Nothing evictable: every entry is permanent or a bound crime, so the list grows.
    }
    let at = life.events.partition_point(|e| e.tick <= entry.tick);
    life.events.insert(at, entry);
}
