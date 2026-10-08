//! M15 types (docs/M15_WORD_AND_BLOOD.md § 1-7; plan phase 1.1): deeds,
//! the per-district rumour pools, reputation and the faction matrix,
//! grudges, the Hunt's state, the social move, Feeds and stories, the
//! creed and taste. Phase 1 fills the knowledge layer (deeds, pools,
//! reputation); every other type is defined now so saves are stable.
//!
//! Everything here is game state between fictional agents of a simulated
//! city: a "deed" is a tagged record of an earlier event, a "rumour" is a
//! copy of that record held by another agent, and reputation is a set of
//! numbers rebuilt daily from those records. The behaviour lives in
//! `systems::{gossip, reputation, word}`.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::components::{DistrictId, HoleId, TilePos};
use crate::entity::EntityId;
use crate::time::Tick;

/// What travels: one tagged kind of event (spec § 1).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Deed {
    Killed,
    Assaulted,
    Robbed,
    Extorted,
    Stripped,
    Arrested,
    Married,
    Evicted,
    Struck,
    Raided,
    Founded,
    Avenged,
    Betrayed,
    Repaid,
    Poached,
}

impl Deed {
    pub const ALL: [Deed; 15] = [
        Deed::Killed,
        Deed::Assaulted,
        Deed::Robbed,
        Deed::Extorted,
        Deed::Stripped,
        Deed::Arrested,
        Deed::Married,
        Deed::Evicted,
        Deed::Struck,
        Deed::Raided,
        Deed::Founded,
        Deed::Avenged,
        Deed::Betrayed,
        Deed::Repaid,
        Deed::Poached,
    ];

    /// Position in [`Deed::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Lower-case, as the config tables and the CLI spell it.
    pub fn label(self) -> &'static str {
        match self {
            Deed::Killed => "killed",
            Deed::Assaulted => "assaulted",
            Deed::Robbed => "robbed",
            Deed::Extorted => "extorted",
            Deed::Stripped => "stripped",
            Deed::Arrested => "arrested",
            Deed::Married => "married",
            Deed::Evicted => "evicted",
            Deed::Struck => "struck",
            Deed::Raided => "raided",
            Deed::Founded => "founded",
            Deed::Avenged => "avenged",
            Deed::Betrayed => "betrayed",
            Deed::Repaid => "repaid",
            Deed::Poached => "poached",
        }
    }

    pub fn parse(s: &str) -> Option<Deed> {
        let s = s.to_ascii_lowercase();
        Deed::ALL.iter().copied().find(|d| d.label() == s)
    }
}

/// A deed read off a memory or a pool entry (plan W3).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct DeedRef {
    pub deed: Deed,
    pub actor: Option<EntityId>,
    pub object: Option<EntityId>,
}

/// One deed being talked about in a district (spec § 1, plan W6).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolEntry {
    pub deed: Deed,
    /// `None` until a hole binds or a witness names the actor.
    pub actor: Option<EntityId>,
    pub object: Option<EntityId>,
    /// When the deed happened.
    pub tick: Tick,
    /// Of the freshest telling in the pool.
    pub hops: u8,
    /// `0..=1`: how much of the district is talking about it.
    pub reach: f32,
    /// Anonymous until bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole: Option<HoleId>,
    /// The Feed that ran it, if any (phase 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story: Option<EntityId>,
    /// W10: the dead's kin, captured at `kill_by` (Killed only; empty on a leaked copy).
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub kin: SmallVec<[EntityId; 8]>,
    /// Plan deviation (W10): the kin the channel has already told, so a
    /// kin whose heard store later lets it go is not told again daily.
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub told: SmallVec<[EntityId; 4]>,
    /// The district it was first posted in (a leaked copy keeps it).
    #[serde(default)]
    pub district: DistrictId,
    /// M15 W38 (plan field): a story's `slant × 100`, carried to the
    /// heard entries drawn from it (0 = not a story).
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub press: i8,
}

fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

impl PoolEntry {
    pub fn deed_ref(&self) -> DeedRef {
        DeedRef { deed: self.deed, actor: self.actor, object: self.object }
    }
}

/// One district's talk, at most `[gossip] pool_cap` entries (lowest reach out).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RumourPool {
    pub entries: Vec<PoolEntry>,
}

/// The four axes of what the city knows about an agent or a faction,
/// rebuilt daily (spec § 2, plan W13). Saved (`top` skipped) so a load
/// mid-day reads what the last midnight wrote.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reputation {
    /// `0..=1` known violence.
    pub dread: f32,
    /// `0..=1` known wealth and position.
    pub standing: f32,
    /// `0..=1`, 0.5 = nothing known; kept word.
    pub honour: f32,
    /// `0..=1` known to the law.
    pub heat: f32,
    pub known_by: u16,
    /// The largest contributions, for the Known tab.
    #[serde(skip)]
    pub top: SmallVec<[(Deed, f32); 4]>,
    /// A god `SetReputation` override `[dread, standing, honour, heat]` until the tick.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<([f32; 4], Tick)>,
    /// W38: Σ slant × conf of held stories about the agent's ties (phase 4).
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub press: f32,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

impl Default for Reputation {
    fn default() -> Self {
        Reputation {
            dread: 0.0,
            standing: 0.0,
            honour: 0.5,
            heat: 0.0,
            known_by: 0,
            top: SmallVec::new(),
            pinned: None,
            press: 0.0,
        }
    }
}

impl Reputation {
    pub fn axis(&self, a: Axis) -> f32 {
        match a {
            Axis::Dread => self.dread,
            Axis::Standing => self.standing,
            Axis::Honour => self.honour,
            Axis::Heat => self.heat,
        }
    }

    /// `[dread, standing, honour, heat]`.
    pub fn axes(&self) -> [f32; 4] {
        [self.dread, self.standing, self.honour, self.heat]
    }

    pub fn set_axes(&mut self, a: [f32; 4]) {
        [self.dread, self.standing, self.honour, self.heat] = a;
    }
}

/// One faction's view of another (spec § 2 "The matrix").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Regard {
    /// `-1..=1`.
    pub value: f32,
    /// `0..=1`.
    pub fear: f32,
}

impl Default for Regard {
    fn default() -> Self {
        Regard { value: 0.0, fear: 0.5 }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Axis {
    Dread,
    Standing,
    Honour,
    Heat,
}

impl Axis {
    pub const ALL: [Axis; 4] = [Axis::Dread, Axis::Standing, Axis::Honour, Axis::Heat];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Axis::Dread => "dread",
            Axis::Standing => "standing",
            Axis::Honour => "honour",
            Axis::Heat => "heat",
        }
    }

    pub fn parse(s: &str) -> Option<Axis> {
        let s = s.to_ascii_lowercase();
        Axis::ALL.iter().copied().find(|a| a.label() == s)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SocialSkill {
    Persuasion,
    Intimidation,
    Knowledge,
    Deception,
}

impl SocialSkill {
    pub const ALL: [SocialSkill; 4] =
        [SocialSkill::Persuasion, SocialSkill::Intimidation, SocialSkill::Knowledge, SocialSkill::Deception];

    /// Position in [`SocialSkill::ALL`] (and in `Skills::last_used`).
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            SocialSkill::Persuasion => "persuasion",
            SocialSkill::Intimidation => "intimidation",
            SocialSkill::Knowledge => "knowledge",
            SocialSkill::Deception => "deception",
        }
    }

    pub fn parse(s: &str) -> Option<SocialSkill> {
        let s = s.to_ascii_lowercase();
        SocialSkill::ALL.iter().copied().find(|k| k.label() == s)
    }
}

/// M15 W28 (plan deviation: the plan kept `(EntityId, Tick)`): who left a
/// group today with the largest share of its competence, for the
/// `TalentLost` event's text: the agent, when, its share and the skill
/// read (killed or left is read at midnight: a corpse or a living agent).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TalentGone {
    pub who: EntityId,
    pub tick: Tick,
    pub share: f32,
    pub skill: String,
}

/// The keyed-stream namespaces of M15's rolls (plan W8, `SimRng::word`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WordNs {
    Exchange = 1,
    Hear,
    Kin,
    Distort,
    Move,
    Hunt,
    Skill,
    Story,
    /// L2 (plan L31): Statistical leisure (phase 2).
    Leisure,
    /// L2: a Gamble at a venue, street dice (phase 2).
    Gamble,
    /// L2: a bout's winner and the spectators' sides (phase 2).
    Bout,
    /// L2: faction violence per agent-day (phase 4).
    FViolence,
    /// L2: the Statistical GangWork day (phase 3).
    GangHour,
    /// L2: held prisoners' meetings and pitch (phase 3).
    Held,
}

// ---------------------------------------------------------------------------
// § 3 grudges (phase 3)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GrudgeCause {
    KilledKin(EntityId),
    KilledFriend(EntityId),
    Assaulted,
    Robbed,
    Stripped(EntityId),
    Evicted,
    Betrayed,
    Inherited(EntityId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grudge {
    /// An agent, or a gang or corp entity when only the faction is known.
    pub target: EntityId,
    pub cause: GrudgeCause,
    /// `0..=1`.
    pub weight: f32,
    pub since: Tick,
    /// 0 for a first wrong; n + 1 when the wrong was itself an Avenged of chain n.
    pub chain: u8,
    /// Target dead or avenged; kept `settle_keep_days` for the biography.
    pub settled: Option<Tick>,
}

/// Component, cap 4 (lowest unsettled weight out).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Grudges {
    pub list: SmallVec<[Grudge; 4]>,
}

/// An open feud between two factions (plan W18).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vendetta {
    pub a: EntityId,
    pub b: EntityId,
    pub since: Tick,
    pub kills: [u16; 2],
    /// Plan field: `[V(a, b), V(b, a)]` at the last midnight, so the gang
    /// brain and Lobby read the feud's weight between midnights.
    #[serde(default)]
    pub w: [f32; 2],
    /// M15 phase 5: a god `DeclareVendetta` holds the feud open until the
    /// tick, each side's weight at least the declared one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared: Option<(Tick, f32)>,
}

// ---------------------------------------------------------------------------
// § 4 the Hunt (phase 3)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HuntPhase {
    Ask,
    Watch,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HuntWhy {
    Goal,
    Stat,
    God,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum IntelSource {
    Sighting,
    Asked,
    Habit,
    Home,
    Deceived,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Intel {
    pub building: Option<EntityId>,
    pub tile: TilePos,
    pub source: IntelSource,
}

/// One hunter's state (plan W20).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HuntState {
    pub target: EntityId,
    pub grudge_target: EntityId,
    pub chain: u8,
    pub since: Tick,
    pub phase: HuntPhase,
    pub venue: Option<EntityId>,
    pub intel: Option<Intel>,
    pub stakeout_until: Option<Tick>,
    #[serde(default)]
    pub deceived: bool,
    pub why: HuntWhy,
    /// Plan field: who sent the hunter the wrong way (a grudge at 0.3 when
    /// the stake-out finds them out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liar: Option<EntityId>,
    /// Plan field: the grudge weight when the Hunt was adopted (a strike
    /// at `lethal_min` or more kills at `hunt_kill_p`).
    #[serde(default)]
    pub weight: f32,
    /// Plan field (throughput): the might gap to the target at adoption,
    /// which a hunter's every think reads instead of walking both
    /// rosters and edge lists again.
    #[serde(default)]
    pub gap: f32,
}

// ---------------------------------------------------------------------------
// § 6 the social move (phase 2)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MoveKind {
    Persuade,
    Intimidate,
    Deceive,
    Charm,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Stake {
    Coins(i64),
    Info { about: EntityId },
    Job { building: EntityId, wage: i64 },
    Join(EntityId),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct SocialMove {
    pub actor: EntityId,
    pub target: EntityId,
    pub kind: MoveKind,
    pub stake: Stake,
}

#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct MoveOutcome {
    pub success: bool,
    pub p: f32,
    pub refused: bool,
    pub backlash: bool,
}

// ---------------------------------------------------------------------------
// § 7 the news (phase 4)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Story {
    pub id: u32,
    pub feed: EntityId,
    pub deed: Deed,
    /// Who the story names (the actor's gang when distorted).
    pub actor: EntityId,
    /// Review fix: the deed's actor when the story was distorted (`actor` is
    /// then the gang), so the three-day dedupe and the buries read the deed,
    /// not the headline. `None` when undistorted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EntityId>,
    pub object: Option<EntityId>,
    pub tick: Tick,
    /// `-1..=1`.
    pub slant: f32,
    pub paid_by: Option<EntityId>,
}

impl Story {
    /// The deed's own actor: `source` when distorted, else `actor`.
    pub fn deed_actor(&self) -> EntityId {
        self.source.unwrap_or(self.actor)
    }
}

/// On a Feed building (plan W36).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FeedState {
    pub name: String,
    pub reach: f32,
    /// District bitset.
    pub covers: u16,
    /// `(actor, until)`.
    pub buried: VecDeque<(EntityId, Tick)>,
    pub stories_today: u8,
}

// ---------------------------------------------------------------------------
// § 2 creed and taste (phase 2)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Creed {
    Purist,
}

impl Creed {
    pub fn label(self) -> &'static str {
        match self {
            Creed::Purist => "Purist",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Audience {
    Corp,
    Street,
    Dreg,
    Gang,
    Purist,
}

/// One audience's taste weights (spec § 2 "Appearance and taste").
#[derive(Copy, Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Taste {
    pub dress: f32,
    pub chrome: f32,
    pub own_colours: f32,
    pub rival_colours: f32,
}

/// The `shadow` observation tool's read-only notes (`World::shadow_notes`):
/// pushed only while the tool has set the log to `Some`; never read by the
/// sim and never drawing RNG.
#[derive(Clone, Debug)]
pub enum ShadowNote {
    /// One resolved social move.
    Move { tick: Tick, m: SocialMove, out: MoveOutcome },
    /// One gossip telling (`gossip::exchange`): `heard` when the listener
    /// took it in.
    Told { tick: Tick, from: EntityId, to: EntityId, r: DeedRef, heard: bool },
    /// L2 (shadow V2): one coin transfer with an agent at an end
    /// (`ownership::transfer`; `refund` for an `ownership::refund`, where
    /// `from` is the owner paying back to `to`).
    Flow {
        tick: Tick,
        from: Option<EntityId>,
        to: Option<EntityId>,
        coins: i64,
        flow: crate::systems::ownership::Flow,
        refund: bool,
    },
    /// L2 (shadow V2): a Statistical gang member's GangWork day hit
    /// (`fviolence::stat_gang_day`); `caught` when the deed was reported.
    StatGang { tick: Tick, id: EntityId, act: crate::ledger::ActKind, caught: bool },
    /// L2 (shadow V2): a held prisoner's cell settlement (`law::settle_held`).
    Settled { tick: Tick, id: EntityId, ticks: u32 },
}
