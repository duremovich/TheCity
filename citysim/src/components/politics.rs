use super::*;

/// Jobs & lives phase 2 (§ 1, D3): a venue without a building (a vendor's
/// stall, a rickshaw's rank, a tinker's mat), its owner's own
/// (`Jobs2State::pitches`, one an owner, capped per district).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pitch {
    /// The street tile it stands on (a HangOut spot of its district).
    pub spot: TilePos,
    pub district: DistrictId,
    /// The `[[trades]]` row it works (a vendor, a rickshaw, a tinker).
    pub trade: TradeId,
    pub owner: EntityId,
    pub since: Tick,
    /// Coins taken today and over the last seven days (rolled at midnight).
    pub take_today: i64,
    pub take_week: i64,
    /// The gang its district's cut went to last (`Flow::StreetCut`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_to: Option<EntityId>,
    /// A vendor's meals in hand.
    #[serde(default)]
    pub stock: u32,
    /// Swept: closed until this tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_until: Option<Tick>,
    /// Days in a row with no take.
    #[serde(default)]
    pub dry_days: u8,
    /// Today's work done (a rickshaw's rides, a tinker's repairs).
    #[serde(default)]
    pub jobs_today: u8,
    /// The pre-view pass phase 4 (P3, OPEN-119): of `take_today`, the coins
    /// the pimp's cut was already taken from at a collection in person
    /// (reset at midnight; written only when non-zero).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub cut_paid: i64,
}

/// A gang's standing order, issued by the faction brain (`systems::faction`).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum Order {
    #[default]
    Expand,
    Contest,
    Raid,
    Retaliate,
    LieLow,
    /// M9: muster, march to the Jail, breach it and free our convicts.
    BreakOut,
    /// M12 D38 (phase 3): take a derelict Block in the gang's districts
    /// through the claim machinery.
    Squat,
    /// M13 D36: abduct the city's most visible chrome and rip it at the Hideout.
    Harvest,
    /// M14 V30: up to `virt_runners` members run nodes from the Hideout
    /// chair each day (a dice contest on the Virt plane); the rest keep
    /// GangWork. Not a muster order.
    VirtRaid,
    /// M16a (plan C26, phase 2): the gang works a taken contract record as
    /// a mission (`Expedition::Mission`); not a muster order of the raid
    /// machinery (`is_raid` false).
    Job,
}

impl Order {
    pub const ALL: [Order; 10] = [
        Order::Expand,
        Order::Contest,
        Order::Raid,
        Order::Retaliate,
        Order::LieLow,
        Order::BreakOut,
        Order::Squat,
        Order::Harvest,
        Order::VirtRaid,
        Order::Job,
    ];

    /// Members muster and march under these.
    pub fn is_raid(self) -> bool {
        matches!(self, Order::Raid | Order::Retaliate | Order::BreakOut)
    }

    /// The expedition's door is the Jail's, not the rival Hideout's.
    pub fn target_is_jail(self) -> bool {
        self == Order::BreakOut
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Something that happened to a gang since its last rescoring. When the
/// pending severities add up to `shock_severity_rethink` the brain rescores
/// at once, without hysteresis.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Shock {
    MemberKilled {
        by_rival: bool,
    },
    MemberArrested,
    /// Our raid on the rival failed.
    RaidLost,
    /// The rival's raid on us succeeded; half the treasury went.
    Raided,
    HomeFlippedAgainst,
    LeaderChanged,
    Sacked,
    /// M9: our breakout was beaten at the Jail door.
    BreakoutFailed,
    /// M9: a breakout freed one of ours.
    MemberFreed,
    /// M10: a member's killing, already shocked as `MemberKilled { by_rival:
    /// false }` and consumed, was later laid to a rival: the difference.
    RivalNamed,
    /// M12 D8: lost control of a district (not a grudge).
    LostDistrict,
    /// M12 D36: the gang split (both halves; a grudge).
    Split,
    /// M14 V34: a run on one of our nodes was traced; `by` is the patron,
    /// else the runner's gang, else `None` (a freelancer).
    Hacked {
        by: Option<EntityId>,
    },
    /// M16a (plan C24, phase 2): a contract strike in one of our districts
    /// by `by` (the buyer's faction).
    Trespass {
        by: Option<EntityId>,
    },
    /// Pressure pass round 6, V2 (TURF § 1): a turf cell we held was taken by
    /// `by` (a grudge: V5's counter-attack scores Retaliate on it).
    LostTurf {
        by: Option<EntityId>,
    },
}

impl Shock {
    pub fn severity(self) -> f32 {
        match self {
            Shock::MemberKilled { by_rival: true } => 1.0,
            Shock::MemberKilled { by_rival: false } => 0.6,
            Shock::MemberArrested => 0.3,
            Shock::RaidLost => 0.8,
            Shock::Raided => 0.6,
            Shock::HomeFlippedAgainst => 0.4,
            Shock::LeaderChanged => 0.5,
            Shock::Sacked => 1.0,
            Shock::BreakoutFailed => 0.8,
            Shock::MemberFreed => 0.3,
            Shock::RivalNamed => {
                Shock::MemberKilled { by_rival: true }.severity() - Shock::MemberKilled { by_rival: false }.severity()
            }
            Shock::LostDistrict => 0.5,
            Shock::Split => 0.8,
            Shock::Hacked { .. } => 0.5,
            Shock::Trespass { .. } => 0.4,
            // As a lost district (a cell is a few blocks of one).
            Shock::LostTurf { .. } => 0.5,
        }
    }

    /// A grievance against the rival: feeds the Retaliate order. A flipped
    /// Home counts (the spec lists only kills, lost raids and sacks): it is
    /// the everyday wrong, and without it a gang whose rival never raids
    /// has nothing to avenge.
    pub fn is_grudge(self) -> bool {
        matches!(
            self,
            Shock::MemberKilled { by_rival: true }
                | Shock::RivalNamed
                | Shock::RaidLost
                | Shock::Raided
                | Shock::Sacked
                | Shock::HomeFlippedAgainst
                | Shock::Split
                // M14 V34: a run attributed to someone (in phase 2 only a
                // runner's gang is ever named).
                | Shock::Hacked { by: Some(_) }
                // M16a C24: a strike by a named faction.
                | Shock::Trespass { by: Some(_) }
                // Round 6 V2: a cell taken by a named gang.
                | Shock::LostTurf { by: Some(_) }
        )
    }
}

/// One order's score from the last rescoring, for the Hideout panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrderScore {
    pub order: Order,
    pub score: f32,
    pub considerations: Vec<Consideration>,
    /// M12 D39: a Raid score aimed at the gang's corp prize, not the rival
    /// Hideout (`faction::is_corp_raid`). False for every other order.
    #[serde(default)]
    pub corp_target: bool,
}

/// M9: the law's posture, chosen by the captain (`systems::law_brain`).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum Posture {
    /// The v1 routine: half the guards hold the Jail, half walk the loop.
    #[default]
    Patrol,
    /// Patrol loops through the target gang's turf; one guard in three holds the Jail.
    Crackdown,
    /// Every guard holds the Jail.
    Garrison,
}

impl Posture {
    pub const ALL: [Posture; 3] = [Posture::Patrol, Posture::Crackdown, Posture::Garrison];
}

impl fmt::Display for Posture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// M9: something that happened to the law since its last rescoring.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum LawShock {
    /// Convicts were broken out of the Jail.
    Jailbreak,
    GuardKilled,
    /// A guard lost a fight (a contested arrest, a breach).
    GuardBeaten,
    BribeRefused,
    /// M15 W28: competence fell by `talent_drop` in a day.
    TalentLost,
}

impl LawShock {
    pub fn severity(self) -> f32 {
        match self {
            LawShock::Jailbreak => 1.0,
            LawShock::GuardKilled => 0.8,
            LawShock::GuardBeaten => 0.4,
            LawShock::BribeRefused => 0.5,
            LawShock::TalentLost => 0.3,
        }
    }
}

/// One posture's score from the last rescoring, for the Jail panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PostureScore {
    pub posture: Posture,
    pub score: f32,
    pub considerations: Vec<Consideration>,
}

/// M9: the law as a faction. One per city, on the Jail building.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Law {
    #[serde(default)]
    pub posture: Posture,
    #[serde(default)]
    pub posture_since: Tick,
    /// The gang a Crackdown is against.
    #[serde(default)]
    pub target: Option<EntityId>,
    /// The most lawful guard; decides the posture.
    #[serde(default)]
    pub captain: Option<EntityId>,
    /// The player's pin; `None` = the captain decides.
    #[serde(default)]
    pub pinned: Option<Posture>,
    /// The last breakout that freed convicts.
    #[serde(default)]
    pub last_breakout_tick: Option<Tick>,
    /// A refused bribe hardens the crackdown until here.
    #[serde(default)]
    pub hardened_until: Option<Tick>,
    /// `(tick, gang)` per report filed against a gang member, newest last, capped.
    #[serde(default)]
    pub report_log: VecDeque<(Tick, EntityId)>,
    /// Every posture's score from the last rescoring, best first. Not saved.
    #[serde(skip)]
    pub posture_trace: Vec<PostureScore>,
    /// Pending since the last rescoring. M15 phase 2 (determinism): saved,
    /// so a save taken between a shock and the rethink it would trigger
    /// re-deals the same beats after the load (`virt_review`'s mid-hour save
    /// diverged on a lost pending shock once M15 moved the trajectory; the
    /// word-off city diverged the same way from another save point).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shocks: Vec<LawShock>,
    /// M11 D21: a corp's bought Crackdown (phase 3).
    #[serde(default)]
    pub lobby: Option<LobbyHold>,
    /// M12 D10: each patrol guard's district beat from the last allocation
    /// (empty with `[law] district_beats` off, under Garrison, or before the
    /// first allocation: those guards walk the M11 route).
    #[serde(default)]
    pub beats: BTreeMap<EntityId, DistrictId>,
    /// M15 W28: the captain's knowledge and the guards' fighting against
    /// the city means (`competence::daily`); `comp_ref` until the first pass.
    #[serde(default = "default_competence")]
    pub competence: f32,
}

/// M15 W28: `[competence] comp_ref`'s default, the neutral competence.
pub fn default_competence() -> f32 {
    0.25
}

impl Default for Law {
    fn default() -> Self {
        Law {
            posture: Posture::default(),
            posture_since: 0,
            target: None,
            captain: None,
            pinned: None,
            last_breakout_tick: None,
            hardened_until: None,
            report_log: VecDeque::new(),
            posture_trace: Vec::new(),
            shocks: Vec::new(),
            lobby: None,
            beats: BTreeMap::new(),
            competence: default_competence(),
        }
    }
}

/// M12 D12: one district stance's score from the last rescoring.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StanceScore {
    pub stance: Stance,
    pub score: f32,
    pub considerations: Vec<Consideration>,
}

/// M11 D21: a corp paid the captain to crack down on `gang` until `until`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LobbyHold {
    pub corp: EntityId,
    pub gang: EntityId,
    pub until: Tick,
}

// ---------------------------------------------------------------------------
// M11: corps (docs/M11_OWNERSHIP.md § 5)
// ---------------------------------------------------------------------------

/// D41: the outside parent a branch belongs to (M17 gives it a body).
pub type ParentId = u32;

/// Outside parents are numbered from here (`OUTSIDE_PARENT_BASE + config
/// row`), a namespace of their own: not a corp slot, not an entity id.
pub const OUTSIDE_PARENT_BASE: ParentId = 1000;

/// D49 hook: who decides for a faction. `Dictator` = the corp's `exec` / the
/// gang's `leader`. M16 adds the vote.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Governance {
    #[default]
    Dictator,
    Board {
        members: Vec<EntityId>,
        /// M16b phase 4 (spec § 6): the chair (the corp's exec).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chair: Option<EntityId>,
        /// The last vote's tick (the board's founding at first).
        #[serde(default)]
        last_vote: Tick,
        /// The last vote's ballots `(member, candidate)`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        votes: Vec<(EntityId, EntityId)>,
        /// A campaign's Persuade pins `(member, candidate)` until the vote
        /// (a `Vote` coercion pins through `World::coercions`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pinned: Vec<(EntityId, EntityId)>,
    },
}
