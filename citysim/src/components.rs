//! Plain-data components and the enums they share. Every component is stored
//! as `Vec<Option<T>>` on [`crate::World`], indexed by `EntityId.index`.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::entity::EntityId;
use crate::exec::ExecState;
use crate::goap::actions::{ActionKind, Plan};
use crate::time::Tick;
use crate::utility::{Consideration, ThinkTrace};

// ---------------------------------------------------------------------------
// Core value types
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct TilePos {
    pub x: u8,
    pub y: u8,
}

impl TilePos {
    pub const fn new(x: u8, y: u8) -> Self {
        TilePos { x, y }
    }

    pub fn manhattan(self, other: TilePos) -> u32 {
        (i32::from(self.x) - i32::from(other.x)).unsigned_abs()
            + (i32::from(self.y) - i32::from(other.y)).unsigned_abs()
    }
}

impl fmt::Display for TilePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.x, self.y)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: u8,
    pub y: u8,
    pub w: u8,
    pub h: u8,
}

impl Rect {
    /// Exclusive right and bottom edges, in `u16` so a rect reaching column 255 cannot overflow.
    fn x1(&self) -> u16 {
        u16::from(self.x) + u16::from(self.w)
    }

    fn y1(&self) -> u16 {
        u16::from(self.y) + u16::from(self.h)
    }

    pub fn contains(&self, p: TilePos) -> bool {
        p.x >= self.x && u16::from(p.x) < self.x1() && p.y >= self.y && u16::from(p.y) < self.y1()
    }

    pub fn on_perimeter(&self, p: TilePos) -> bool {
        self.contains(p)
            && (p.x == self.x || u16::from(p.x) + 1 == self.x1() || p.y == self.y || u16::from(p.y) + 1 == self.y1())
    }

    pub fn centre(&self) -> (f32, f32) {
        (f32::from(self.x) + f32::from(self.w) / 2.0, f32::from(self.y) + f32::from(self.h) / 2.0)
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        u16::from(self.x) < o.x1()
            && u16::from(o.x) < self.x1()
            && u16::from(self.y) < o.y1()
            && u16::from(o.y) < self.y1()
    }
}

/// A coarse district of the map (M10): the second grid of a v2 map file.
/// A v1 map has no zone grid and reads `Mid` everywhere.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Zone {
    Spire,
    Civic,
    Vats,
    #[default]
    Mid,
    Sump,
}

impl Zone {
    pub const ALL: [Zone; 5] = [Zone::Spire, Zone::Civic, Zone::Vats, Zone::Mid, Zone::Sump];

    /// Position in `Zone::ALL`.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The map-file letter.
    pub fn letter(self) -> char {
        match self {
            Zone::Spire => 'S',
            Zone::Civic => 'C',
            Zone::Vats => 'V',
            Zone::Mid => 'M',
            Zone::Sump => 'U',
        }
    }

    pub fn parse(c: char) -> Option<Zone> {
        Zone::ALL.into_iter().find(|z| z.letter() == c)
    }
}

impl fmt::Display for Zone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum TileKind {
    Ground,
    Wall,
    Road,
    Door,
    Farmland,
    Water,
}

impl TileKind {
    pub fn walkable(self) -> bool {
        !matches!(self, TileKind::Wall | TileKind::Water)
    }

    /// Step cost for pathfinding.
    pub fn move_cost(self) -> f32 {
        match self {
            TileKind::Ground | TileKind::Door => 1.0,
            TileKind::Road => 0.7,
            TileKind::Farmland => 1.2,
            TileKind::Wall | TileKind::Water => f32::INFINITY,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum BuildingKind {
    Home,
    Farm,
    Market,
    Bar,
    Jail,
    Cemetery,
    Hall,
    Hideout,
    Warehouse,
    /// M10: two in the Vats, loaded and drawn, inert until M11.
    SecurityOffice,
    /// M10: an empty walled-off plot (no walls, one door), inert until M11.
    Lot,
    /// M12 D20: beds by the night for the homeless who can pay.
    Hotel,
    /// M13 D16: the Ripperdoc: chrome, Therapy, Detox (inert until phase 3).
    Clinic,
    /// M13 D16: vehicles, repairs, secure parking (inert until phase 2).
    Garage,
    /// M14 D16 (plan V16): makes Data in its focus track.
    Lab,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 15] = [
        BuildingKind::Home,
        BuildingKind::Farm,
        BuildingKind::Market,
        BuildingKind::Bar,
        BuildingKind::Jail,
        BuildingKind::Cemetery,
        BuildingKind::Hall,
        BuildingKind::Hideout,
        BuildingKind::Warehouse,
        BuildingKind::SecurityOffice,
        BuildingKind::Lot,
        BuildingKind::Hotel,
        BuildingKind::Clinic,
        BuildingKind::Garage,
        BuildingKind::Lab,
    ];

    pub fn parse(s: &str) -> Option<BuildingKind> {
        Some(match s {
            "Home" => BuildingKind::Home,
            "Farm" => BuildingKind::Farm,
            "Market" => BuildingKind::Market,
            "Bar" => BuildingKind::Bar,
            "Jail" => BuildingKind::Jail,
            "Cemetery" => BuildingKind::Cemetery,
            "Hall" => BuildingKind::Hall,
            "Hideout" => BuildingKind::Hideout,
            "Warehouse" => BuildingKind::Warehouse,
            "SecurityOffice" => BuildingKind::SecurityOffice,
            "Lot" => BuildingKind::Lot,
            "Hotel" => BuildingKind::Hotel,
            "Clinic" => BuildingKind::Clinic,
            "Garage" => BuildingKind::Garage,
            "Lab" => BuildingKind::Lab,
            _ => return None,
        })
    }

    /// The display name (M11 section 1). Identifiers and `Display` stay the v1 names.
    pub fn label(self) -> &'static str {
        match self {
            BuildingKind::Home => "Block",
            BuildingKind::Farm => "Vat Farm",
            BuildingKind::Market => "Street Market",
            BuildingKind::Bar => "Bar",
            BuildingKind::Jail => "Precinct",
            BuildingKind::Cemetery => "Recycler",
            BuildingKind::Hall => "Civic Hall",
            BuildingKind::Hideout => "Hideout",
            BuildingKind::Warehouse => "Reserve Depot",
            BuildingKind::SecurityOffice => "Security Office",
            BuildingKind::Lot => "Lot",
            BuildingKind::Hotel => "Capsule Hotel",
            BuildingKind::Clinic => "Ripperdoc",
            BuildingKind::Garage => "Garage",
            BuildingKind::Lab => "Lab",
        }
    }

    /// The letter drawn at the building's centre.
    pub fn letter(self) -> char {
        match self {
            BuildingKind::Home => 'H',
            BuildingKind::Farm => 'F',
            BuildingKind::Market => 'M',
            BuildingKind::Bar => 'B',
            BuildingKind::Jail => 'J',
            BuildingKind::Cemetery => 'C',
            BuildingKind::Hall => 'T',
            BuildingKind::Hideout => 'G',
            BuildingKind::Warehouse => 'W',
            BuildingKind::SecurityOffice => 'O',
            BuildingKind::Lot => 'L',
            BuildingKind::Hotel => 'N',
            BuildingKind::Clinic => 'R',
            BuildingKind::Garage => 'V',
            BuildingKind::Lab => 'Q',
        }
    }
}

impl fmt::Display for BuildingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Sex {
    Male,
    Female,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Lod {
    Full,
    Coarse,
    Statistical,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Role {
    Farmer,
    Guard,
    Clerk,
    Bartender,
    Gravedigger,
    /// M12 D23: the city's street sweepers, employed at the Recycler.
    Sanitation,
    /// M13 D16: staff of a Clinic.
    Ripperdoc,
    /// M13 D16: staff of a Garage.
    Mechanic,
    /// M14 (plan V16): staff of a Lab.
    Researcher,
}

impl Role {
    pub const ALL: [Role; 9] = [
        Role::Farmer,
        Role::Guard,
        Role::Clerk,
        Role::Bartender,
        Role::Gravedigger,
        Role::Sanitation,
        Role::Ripperdoc,
        Role::Mechanic,
        Role::Researcher,
    ];

    /// The display name (M11 section 1).
    pub fn label(self) -> &'static str {
        match self {
            Role::Farmer => "Vat Tech",
            Role::Guard => "Guard",
            Role::Clerk => "Clerk",
            Role::Bartender => "Bartender",
            Role::Gravedigger => "Recycler Tech",
            Role::Sanitation => "Sanitation",
            Role::Ripperdoc => "Ripperdoc",
            Role::Mechanic => "Mechanic",
            Role::Researcher => "Researcher",
        }
    }

    /// The building kind that employs this role.
    pub fn workplace(self) -> BuildingKind {
        match self {
            Role::Farmer => BuildingKind::Farm,
            Role::Guard => BuildingKind::Jail,
            Role::Clerk => BuildingKind::Market,
            Role::Bartender => BuildingKind::Bar,
            Role::Gravedigger => BuildingKind::Cemetery,
            Role::Sanitation => BuildingKind::Cemetery,
            Role::Ripperdoc => BuildingKind::Clinic,
            Role::Mechanic => BuildingKind::Garage,
            Role::Researcher => BuildingKind::Lab,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Ordered by severity (`Crime::severity`), not by declaration: Vagrancy
/// (M12 D15, appended so saves keep their variant names) is the least.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Crime {
    Theft,
    Extortion,
    Assault,
    Murder,
    /// M12 D15: sleeping rough where the law sweeps.
    Vagrancy,
    /// M13 D26: a vehicle stolen (or a botched attempt).
    GrandTheft,
    /// M13 D25: a crash that killed, the driver seen.
    Manslaughter,
    /// M13 D36: a Harvest crew dragged someone off for their chrome.
    Abduction,
    /// M13 D38: a dealer's sale of illegal Stims.
    Dealing,
}

impl Crime {
    /// The display name (M11 section 1).
    pub fn label(self) -> &'static str {
        match self {
            Crime::Theft => "Theft",
            Crime::Extortion => "Shakedown",
            Crime::Assault => "Assault",
            Crime::Murder => "Murder",
            Crime::Vagrancy => "Vagrancy",
            Crime::GrandTheft => "Grand Theft",
            Crime::Manslaughter => "Manslaughter",
            Crime::Abduction => "Abduction",
            Crime::Dealing => "Dealing",
        }
    }

    /// The order the law ranks crimes by (the most severe open report sets
    /// a sentence): Vagrancy, Theft, Grand Theft, Dealing, Shakedown,
    /// Assault, Manslaughter, Abduction, Murder. Unique per crime (`Ord`
    /// reads it).
    pub fn severity(self) -> u8 {
        match self {
            Crime::Vagrancy => 0,
            Crime::Theft => 1,
            Crime::GrandTheft => 2,
            Crime::Dealing => 3,
            Crime::Extortion => 4,
            Crime::Assault => 5,
            Crime::Manslaughter => 6,
            Crime::Abduction => 7,
            Crime::Murder => 8,
        }
    }
}

impl PartialOrd for Crime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Crime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.severity().cmp(&other.severity())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum DeathCause {
    Starvation,
    Violence,
    OldAge,
    Execution,
    /// M13 D47: a crash (phase 2).
    Accident,
    /// M13 D47: a stim overdose (phase 4).
    Overdose,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum RelKind {
    Acquaintance,
    Friend,
    Family,
    Spouse,
    Parent,
    Rival,
    Enemy,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum MemoryKind {
    Ate,
    Starved,
    WasRobbed,
    SawCrime,
    WasArrested,
    Fought,
    Won,
    Lost,
    Socialised,
    Rejected,
    Courted,
    Married,
    Grief,
    SawCorpse,
    Paid,
    Unpaid,
    /// Jail co-location (Social graph, M5).
    MetInJail,
    /// Witnessed outburst of a very low mood (Mood table).
    Tantrum,
    /// M9: broken out of the Jail by the gang.
    Escaped,
    /// M11: could not pay the day's rent in full.
    RentShort,
    /// M11: put out of the Home for arrears.
    Evicted,
    /// M12 D35: hit as a bystander in a brawl at a door.
    CaughtInCrossfire,
    /// M13 D11: a lender towed, bricked or took an asset.
    Repossessed,
    /// M13 D25: hit by a vehicle and lived.
    Crashed,
    /// M13 D35: an heir saw the body of their kin stripped.
    Stripped,
    /// M13 D33: came down from a cyberpsychotic episode.
    Episode,
    /// M13 D33: saw someone go berserk.
    SawEpisode,
    /// M13 D36: dragged off and ripped, and lived.
    Abducted,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum GoalKind {
    Eat,
    Sleep,
    Work,
    Earn,
    Socialise,
    Court,
    Flee,
    Fight,
    ReportCrime,
    Patrol,
    Arrest,
    JoinGang,
    GangWork,
    /// Gang members: muster at the Hideout and brawl at the rival's door.
    Raid,
    Bury,
    Idle,
    /// M11 D26: open a business (a Bar or a Home) on a vacant Lot.
    Found,
    /// M12 D27: a homeless adult who cannot afford a Hotel moves into a derelict.
    Squat,
    /// M13 D43: buy a vehicle, chrome or a pack.
    Shop,
    /// M13 D34: Therapy or (phase 4, D39) Detox at a Clinic.
    Treat,
    /// M13 D35: strip a fresh body (and rip its chrome).
    Loot,
    /// M13 D39: buy a dose (from a dealer at a Bar, or a legal Market) and use it.
    GetHigh,
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
}

impl Order {
    pub const ALL: [Order; 8] = [
        Order::Expand,
        Order::Contest,
        Order::Raid,
        Order::Retaliate,
        Order::LieLow,
        Order::BreakOut,
        Order::Squat,
        Order::Harvest,
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
}

impl LawShock {
    pub fn severity(self) -> f32 {
        match self {
            LawShock::Jailbreak => 1.0,
            LawShock::GuardKilled => 0.8,
            LawShock::GuardBeaten => 0.4,
            LawShock::BribeRefused => 0.5,
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
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
    /// Pending since the last rescoring. Not saved.
    #[serde(skip)]
    pub shocks: Vec<LawShock>,
    /// M11 D21: a corp's bought Crackdown (phase 3).
    #[serde(default)]
    pub lobby: Option<LobbyHold>,
    /// M12 D10: each patrol guard's district beat from the last allocation
    /// (empty with `[law] district_beats` off, under Garrison, or before the
    /// first allocation: those guards walk the M11 route).
    #[serde(default)]
    pub beats: BTreeMap<EntityId, DistrictId>,
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
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Niche {
    Food,
    Housing,
    Security,
    /// M13 D17: Clinics and Garages (Zetatech).
    Tech,
}

impl Niche {
    pub const ALL: [Niche; 4] = [Niche::Food, Niche::Housing, Niche::Security, Niche::Tech];

    pub fn label(self) -> &'static str {
        match self {
            Niche::Food => "Food",
            Niche::Housing => "Housing",
            Niche::Security => "Security",
            Niche::Tech => "Tech",
        }
    }

    pub fn parse(s: &str) -> Option<Niche> {
        Niche::ALL.into_iter().find(|n| n.label() == s)
    }
}

impl fmt::Display for Niche {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A corp's standing order (phase 3's brain; phase 2 seeds every corp at `Hunker`).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum CorpOrder {
    Grow,
    Squeeze,
    Undercut,
    Acquire,
    Secure,
    #[default]
    Hunker,
    Lobby,
    /// M14 (plan V31): research the focus track, build and staff Labs.
    Research,
}

impl CorpOrder {
    pub const ALL: [CorpOrder; 8] = [
        CorpOrder::Grow,
        CorpOrder::Squeeze,
        CorpOrder::Undercut,
        CorpOrder::Acquire,
        CorpOrder::Secure,
        CorpOrder::Hunker,
        CorpOrder::Lobby,
        CorpOrder::Research,
    ];
}

impl fmt::Display for CorpOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Something that happened to a corp since its last rescoring.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum CorpShock {
    Robbed(i64),
    Extorted,
    EmployeeKilled,
    Bankrupt(EntityId),
    Strike,
    Undercut,
    BuildingLost,
    /// M12 D8: lost control of a district.
    LostDistrict,
    /// M12 D39: a gang raid on one of its buildings was won.
    Raided,
    /// M12 D32: a riot hit one of its buildings ...
    Rioted,
    /// ... or another building in a district where it owns some (half severity).
    RiotNearby,
    /// M14 (plan V34): a tech track dropped a tier.
    TechLost,
}

impl CorpShock {
    pub fn severity(self) -> f32 {
        match self {
            CorpShock::Robbed(c) if c > 100 => 0.6,
            CorpShock::Robbed(_) => 0.3,
            CorpShock::Extorted => 0.4,
            CorpShock::EmployeeKilled => 0.5,
            CorpShock::Bankrupt(_) => 0.5,
            CorpShock::Strike => 0.8,
            CorpShock::Undercut => 0.3,
            CorpShock::BuildingLost => 0.5,
            CorpShock::LostDistrict => 0.4,
            CorpShock::Raided => 0.7,
            CorpShock::Rioted => 0.9,
            CorpShock::RiotNearby => 0.45,
            CorpShock::TechLost => 0.6,
        }
    }
}

/// One `(order, niche)` score from the last rescoring, for the Corp panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CorpOrderScore {
    pub order: CorpOrder,
    pub niche: Niche,
    pub score: f32,
    pub considerations: Vec<Consideration>,
}

/// D20: a crime loss on an owned building, and the gang behind it if any.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CorpLoss {
    pub tick: Tick,
    pub coins: i64,
    pub gang: Option<EntityId>,
    /// The building it happened at (`Secure` contracts the newest first).
    #[serde(default)]
    pub building: Option<EntityId>,
}

fn one_f32() -> f32 {
    1.0
}

/// A corporation: a faction that owns buildings, earns through them and pays
/// their upkeep. Its own entity, carrying only this component.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corp {
    pub name: String,
    pub niches: BTreeSet<Niche>,
    pub treasury: i64,
    /// The agent whose Personality the brain reads; `None` = a default.
    pub exec: Option<EntityId>,
    /// Sorted; redundant with `Building.owner`, kept for O(1) listing.
    pub buildings: Vec<EntityId>,
    #[serde(default)]
    pub order: CorpOrder,
    #[serde(default)]
    pub order_niche: Option<Niche>,
    #[serde(default)]
    pub order_since: Tick,
    #[serde(skip)]
    pub order_trace: Vec<CorpOrderScore>,
    #[serde(skip)]
    pub shocks: Vec<CorpShock>,
    /// Per niche: Food markup, Housing rent level, Security contract price.
    #[serde(default)]
    pub price_level: BTreeMap<Niche, f32>,
    /// Net coins per day, last 14, newest last.
    #[serde(default)]
    pub cashflow: VecDeque<i64>,
    /// D20: crime losses on owned buildings, last 14 days.
    #[serde(default)]
    pub loss_log: VecDeque<CorpLoss>,
    #[serde(default)]
    pub negative_since: Option<Tick>,
    /// M11 phase 5: the treasury at the last midnight close, before that
    /// night's upkeep: what solvency (`negative_since`) and restocking read,
    /// so a corp with a day's profit but less than a day's upkeep in the
    /// bank is not in the red (the upkeep lump is the intra-day trough).
    #[serde(default)]
    pub closing: i64,
    /// M11 phase 5: no upkeep before this tick (a newly incorporated corp).
    #[serde(default)]
    pub upkeep_grace_until: Option<Tick>,
    /// Security only: `(client building, until)`, sorted (phase 3).
    #[serde(default)]
    pub contracts: Vec<(EntityId, Tick)>,
    #[serde(default)]
    pub lobby_until: Option<Tick>,
    #[serde(default)]
    pub last_acquisition_tick: Option<Tick>,
    /// D38: the seeding row (CSV columns); spinoffs and incorporations `None`.
    #[serde(default)]
    pub slot: Option<u8>,
    /// The `cash` denominator: `treasury_initial`, or the treasury at founding.
    #[serde(default)]
    pub treasury_ref: i64,
    /// Net coins through `World::purse_add` today; rolled into `cashflow`.
    #[serde(default)]
    pub cashflow_today: i64,
    /// D22: wages paid × this (Food Squeeze 0.9).
    #[serde(default = "one_f32")]
    pub wage_mult: f32,
    /// D22: Housing Squeeze evicts sooner.
    #[serde(default)]
    pub evict_days_override: Option<u8>,
    #[serde(default)]
    pub share_hist: BTreeMap<Niche, VecDeque<f32>>,
    /// D23.
    #[serde(default)]
    pub last_build_tick: Option<Tick>,
    /// D41: a branch of an outside parent (no behaviour in M11).
    #[serde(default)]
    pub parent: Option<ParentId>,
    #[serde(default)]
    pub outside_treasury: i64,
    /// D49: `Dictator` = `exec` decides; no Board in M11.
    #[serde(default)]
    pub governance: Governance,
    /// God `SetCorpOrder`: the order holds until this tick (the rescores
    /// keep the trace but do not switch).
    #[serde(default)]
    pub pinned_until: Option<Tick>,
    /// M12 fix pass: the tick of the last raid or riot that took from one of
    /// its buildings; for `[corps] raided_days` the Secure and Lobby input
    /// `losses` floors at `raided_losses` (the raided corp hardens).
    #[serde(default)]
    pub raided_at: Option<Tick>,
    /// M14 (plan V21): the tech tree. A pre-M14 save reads `Tech::unset`
    /// (tier 0), which `migrate_legacy` seeds by name.
    #[serde(default = "crate::virt::Tech::unset")]
    pub tech: crate::virt::Tech,
    /// M14 (plan V25): the ICE on this corp's Ledger node.
    #[serde(default)]
    pub ledger_ice: crate::virt::SecurityProfile,
    /// M14 (plan V27): ICE spend per day, last 30, newest last.
    #[serde(default)]
    pub ice_spend: VecDeque<i64>,
    #[serde(default)]
    pub ice_spend_today: i64,
}

impl Corp {
    /// The highest `price_level` the corp charges in any niche; 0 with none
    /// (M12 review: one fold for the strike target and the riot grievance; a
    /// corp with no price levels never outranks a pricing one as a strike
    /// target, and the riot reads it through `max(1.0)`, no grievance).
    pub fn max_price_level(&self) -> f32 {
        self.price_level.values().copied().fold(0.0f32, f32::max)
    }

    pub fn new(name: String, niches: BTreeSet<Niche>, treasury: i64, exec: Option<EntityId>) -> Corp {
        let price_level = niches.iter().map(|&n| (n, 1.0)).collect();
        Corp {
            name,
            niches,
            treasury,
            exec,
            buildings: Vec::new(),
            order: CorpOrder::Hunker,
            order_niche: None,
            order_since: 0,
            order_trace: Vec::new(),
            shocks: Vec::new(),
            price_level,
            cashflow: VecDeque::new(),
            loss_log: VecDeque::new(),
            negative_since: None,
            closing: treasury,
            upkeep_grace_until: None,
            contracts: Vec::new(),
            lobby_until: None,
            last_acquisition_tick: None,
            slot: None,
            treasury_ref: treasury.max(1000),
            cashflow_today: 0,
            wage_mult: 1.0,
            evict_days_override: None,
            share_hist: BTreeMap::new(),
            last_build_tick: None,
            parent: None,
            outside_treasury: 0,
            governance: Governance::Dictator,
            pinned_until: None,
            raided_at: None,
            tech: crate::virt::Tech::default(),
            ledger_ice: crate::virt::SecurityProfile::default(),
            ice_spend: VecDeque::new(),
            ice_spend_today: 0,
        }
    }

    /// The niche's price level (1.0 when unset).
    pub fn level(&self, niche: Niche) -> f32 {
        self.price_level.get(&niche).copied().unwrap_or(1.0)
    }
}

/// A gang's hold on a Home: `count` extortions by `gang`; at 3 the Home is territory.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Claim {
    pub gang: EntityId,
    pub count: u8,
}

// ---------------------------------------------------------------------------
// Agent components
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub tile: TilePos,
    /// `Some` if inside a building rect (including its door).
    pub building: Option<EntityId>,
    /// The tick the agent entered `building` (co-location is measured from here).
    #[serde(default)]
    pub entered: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    /// `"<First> <Last>"` from the name tables.
    pub name: String,
    /// `0..=12000` (100 years); +1 per day.
    pub age_days: u32,
    pub sex: Sex,
    /// Negative for the initial population.
    pub born_tick: i64,
    /// Widowed at this tick (Grief; the Spouse edge stays).
    #[serde(default)]
    pub spouse_died_tick: Option<Tick>,
}

/// Under 18: Statistical only, no Brain or Needs; fed from the Home pantry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Child {
    /// Consecutive days the pantry could not feed them; three kills.
    pub hunger_days: u8,
}

impl Identity {
    pub fn age_years(&self) -> u32 {
        self.age_days / crate::time::DAYS_PER_YEAR as u32
    }
}

/// All `f32` in `0.0..=1.0`; 1.0 = fully satisfied, 0.0 = critical.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub energy: f32,
    pub safety: f32,
    /// Derived hourly: `clamp(coins / (7 * price_food * (1 + greed)), 0, 1)`.
    pub wealth: f32,
    pub belonging: f32,
    pub intimacy: f32,
    /// `Some(t)` while `hunger == 0.0`.
    pub starving_since: Option<Tick>,
}

/// All `f32` in `0.0..=1.0`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Personality {
    pub lawfulness: f32,
    pub greed: f32,
    pub pride: f32,
    pub sociability: f32,
    pub courage: f32,
    pub loyalty: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mood {
    /// `-1.0..=1.0`, default 0.0.
    pub value: f32,
    /// `Some` while `value < -0.8`.
    pub low_since: Option<Tick>,
    pub last_computed: Tick,
}

impl Default for Mood {
    fn default() -> Self {
        Mood { value: 0.0, low_since: None, last_computed: 0 }
    }
}

/// `coins >= 0` for agents; debts live on edges.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub coins: i64,
}

/// `food 0..=20` (carry cap 20); `stolen_food <= food`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub food: u32,
    pub stolen_food: u32,
    /// M13 D7: stim doses carried (load 1 each).
    #[serde(default)]
    pub stims: u16,
    /// M13 D7: Parts carried (load 3 each).
    #[serde(default)]
    pub parts: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    /// Building entity.
    pub employer: Option<EntityId>,
    pub role: Role,
    pub wage_per_day: i64,
    /// `tick_of_day` ranges `[start, end)`; default `[(540, 1080)]`.
    pub shifts: Vec<(u16, u16)>,
    /// Quits at 7.
    pub days_unpaid: u8,
    pub tax_accum: f32,
    /// Key of the last shift worked (see `shift_key_at`); one shift per key.
    #[serde(default)]
    pub last_shift_day: Option<i64>,
    /// Day of the last CollectWage that the Treasury could not pay in full;
    /// one such visit per day, so a short Treasury is not hammered.
    #[serde(default)]
    pub last_wage_attempt_day: Option<u64>,
    /// Guards: ticks of the running shift spent on law duty (Patrol, Arrest,
    /// or Work on a Jail day); `law::credit_guard_shifts` reads and clears it
    /// when the shift ends.
    #[serde(default)]
    pub duty_ticks: u16,
    /// M11 D17: when hired (seeded jobs 0); Hunker fires the newest first.
    #[serde(default)]
    pub hired_tick: Tick,
    /// M11 D35: the shift key a strike struck (`classes::walk_out`): no
    /// work, no wage, a guard's shift clock included.
    #[serde(default)]
    pub struck_shift: Option<i64>,
}

impl Job {
    pub fn on_shift(&self, tick_of_day: u16) -> bool {
        self.shifts.iter().any(|&(s, e)| tick_of_day >= s && tick_of_day < e)
    }

    /// Is a Hall visit for wages worthwhile today: wages owed, and no
    /// short payment yet today.
    pub fn wage_collectable(&self, day: u64) -> bool {
        self.days_unpaid >= 1 && self.last_wage_attempt_day != Some(day)
    }

    /// Ticks until the next shift start, or 0 if on shift now.
    pub fn ticks_until_shift(&self, tick_of_day: u16) -> u16 {
        if self.on_shift(tick_of_day) {
            return 0;
        }
        self.shifts
            .iter()
            .map(|&(s, _)| if s > tick_of_day { s - tick_of_day } else { 1440 - tick_of_day + s })
            .min()
            .unwrap_or(0)
    }

    /// The day that owns the shift containing `tick`. A segment starting at 0
    /// that continues a 1440-ending segment (night guards) belongs to the
    /// previous day, so the 21:00-06:00 shift has one key, not two.
    pub fn shift_key_at(&self, tick: Tick) -> i64 {
        let day = (tick / 1440) as i64;
        let tod = (tick % 1440) as u16;
        let in_tail = self.shifts.iter().any(|&(s, e)| s == 0 && tod < e);
        let wraps = self.shifts.iter().any(|&(_, e)| e == 1440);
        if in_tail && wraps {
            day - 1
        } else {
            day
        }
    }

    /// Key of the shift in progress, or of the next one to start.
    pub fn next_shift_key(&self, tick: Tick) -> i64 {
        let tod = (tick % 1440) as u16;
        self.shift_key_at(tick + Tick::from(self.ticks_until_shift(tod)))
    }

    /// Absolute tick at which the shift containing `tick` ends. A shift that
    /// runs to 1440 and continues from 0 (night guards) is one shift.
    pub fn shift_end(&self, tick: Tick) -> Option<Tick> {
        let tod = (tick % 1440) as u16;
        let &(_, end) = self.shifts.iter().find(|&&(s, e)| tod >= s && tod < e)?;
        let mut until = tick + Tick::from(end - tod);
        if end == 1440 {
            if let Some(&(_, e2)) = self.shifts.iter().find(|&&(s, _)| s == 0) {
                until += Tick::from(e2);
            }
        }
        Some(until)
    }
}

/// `None` = homeless.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Household {
    pub home: Option<EntityId>,
    /// M11 D6: this adult's share of the Home's rent, accrued daily; the
    /// integer part is due at midnight.
    #[serde(default)]
    pub rent_due: f32,
    /// M11 D7: consecutive midnights a due rent went short; eviction at `evict_days`.
    #[serde(default)]
    pub arrears: u8,
    /// M11 D7: `(owner, tick)` of the last eviction; that owner refuses the
    /// agent for `[rent] refuse_days`.
    #[serde(default)]
    pub evicted_by: Option<(Option<EntityId>, Tick)>,
    /// Rent paid per day, `(day, coins)`, the last seven days only (the
    /// inspector's "paid N in 7 days"; `rent_paid_7d`).
    #[serde(default)]
    pub rent_paid_log: VecDeque<(u64, i64)>,
    /// M11 § 7: consecutive midnights as a Dreg with mood below
    /// `[classes] dreg_emigrate_mood`; at `dreg_emigrate_days` they leave.
    #[serde(default)]
    pub miserable_days: u8,
    /// M12 D27: put out of this squat; banned from it until the tick.
    #[serde(default)]
    pub squat_ban: Option<(EntityId, Tick)>,
    /// M12 D28: put on the street without an eviction (a seeded derelict,
    /// a Block gone derelict); the re-housing wait runs from here as from
    /// an eviction. Cleared when housed.
    #[serde(default)]
    pub homeless_since: Option<Tick>,
}

impl Household {
    pub fn new(home: Option<EntityId>) -> Household {
        Household {
            home,
            rent_due: 0.0,
            arrears: 0,
            evicted_by: None,
            rent_paid_log: VecDeque::new(),
            miserable_days: 0,
            squat_ban: None,
            homeless_since: None,
        }
    }

    /// Rent paid on `today` and the six days before.
    pub fn rent_paid_7d(&self, today: u64) -> i64 {
        self.rent_paid_log.iter().filter(|&&(d, _)| d + 7 > today).map(|&(_, c)| c).sum()
    }

    /// Record `coins` of rent paid on `today`; days older than a week drop.
    pub fn note_rent_paid(&mut self, today: u64, coins: i64) {
        if coins <= 0 {
            return;
        }
        while self.rent_paid_log.front().is_some_and(|&(d, _)| d + 7 <= today) {
            self.rent_paid_log.pop_front();
        }
        match self.rent_paid_log.back_mut() {
            Some((d, c)) if *d == today => *c += coins,
            _ => self.rent_paid_log.push_back((today, coins)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Brain {
    /// Default Coarse.
    pub lod: Lod,
    pub current_goal: Option<GoalKind>,
    pub goal_since: Tick,
    pub plan: Option<Plan>,
    pub plan_step: u8,
    pub cooldowns: BTreeMap<GoalKind, Tick>,
    pub pinned: bool,
    pub betraying: bool,
    /// Top-5 goals and consideration outputs, for the inspector.
    pub last_think: Option<ThinkTrace>,
    pub last_think_tick: Tick,
    /// Coarse: when the current action resolves.
    pub action_until: Tick,
    /// Execution state of the current plan step.
    #[serde(default)]
    pub exec: ExecState,
    /// Day on which the dole was last collected.
    #[serde(default)]
    pub last_dole_day: Option<u64>,
    /// Last think triggered by a plan ending or failing (one per 30 ticks).
    #[serde(default)]
    pub last_urgent_think_tick: Tick,
    /// The last plan failure: a second consecutive failure of the same goal
    /// cools it; the first is simply replanned.
    #[serde(default)]
    pub last_plan_failure: Option<(GoalKind, Tick)>,
    /// Waiting in `World::plan_queue`.
    #[serde(default)]
    pub plan_queued: bool,
    /// Cuffed by this guard: no thinking or acting until jailed or freed.
    #[serde(default)]
    pub cuffed_by: Option<EntityId>,
    /// A guard escorting this cuffed suspect to the Jail.
    #[serde(default)]
    pub escorting: Option<EntityId>,
    /// Guards: the current patrol loop (buildings) and legs completed this shift.
    #[serde(default)]
    pub patrol_route: Vec<EntityId>,
    #[serde(default)]
    pub patrol_legs: u8,
    /// The shift key the patrol route belongs to.
    #[serde(default)]
    pub patrol_shift_key: Option<i64>,
    /// Gang: the day the extortion task was done, and coins taken today (for SplitLoot).
    #[serde(default)]
    pub gang_task_day: Option<u64>,
    #[serde(default)]
    pub loot_today: i64,
    /// `(day, any edge with affinity >= 0.3 to an unmarried agent)`: the Court
    /// gate, refreshed daily because the neighbour scan is O(degree).
    #[serde(default)]
    pub court_candidate: Option<(u64, bool)>,
    /// The corpse being carried to the Cemetery; its Position follows.
    #[serde(default)]
    pub carrying_corpse: Option<EntityId>,
    /// Walking to the map edge to leave the city; no goals, no plans.
    #[serde(default)]
    pub emigrating: bool,
    /// The day the spouse intimacy bonus was last granted (once a night).
    #[serde(default)]
    pub last_spouse_night: Option<u64>,
    /// Gang: the order the current GangWork plan serves; `None` while freelancing.
    #[serde(default)]
    pub following_order: Option<Order>,
    /// Gang: the day a `Disobeyed` event was last logged for this member.
    #[serde(default)]
    pub disobeyed_day: Option<u64>,
    /// M10: the last day the agent was not Statistical at an hourly
    /// assignment (the trace's `STATISTICAL_ALL_DAY` is `body_day != today`).
    #[serde(default)]
    pub body_day: Option<u64>,
    /// M11 D26: the day of the last `Register` (the Found cooldown).
    #[serde(default)]
    pub last_found_day: Option<u64>,
    /// M13 D29: what a Shop plan is buying, fixed when the seller is bound.
    #[serde(default)]
    pub shop_pick: Option<ShopPick>,
    /// M13 D36: dragged off by this gang member (for the UI and the law).
    #[serde(default)]
    pub abducted_by: Option<EntityId>,
}

impl Default for Brain {
    fn default() -> Self {
        Brain {
            lod: Lod::Coarse,
            current_goal: None,
            goal_since: 0,
            plan: None,
            plan_step: 0,
            cooldowns: BTreeMap::new(),
            pinned: false,
            betraying: false,
            last_think: None,
            last_think_tick: 0,
            action_until: 0,
            exec: ExecState::Idle,
            last_dole_day: None,
            last_urgent_think_tick: 0,
            last_plan_failure: None,
            plan_queued: false,
            cuffed_by: None,
            escorting: None,
            patrol_route: Vec::new(),
            patrol_legs: 0,
            patrol_shift_key: None,
            gang_task_day: None,
            loot_today: 0,
            court_candidate: None,
            carrying_corpse: None,
            emigrating: false,
            last_spouse_night: None,
            following_order: None,
            disobeyed_day: None,
            body_day: None,
            last_found_day: None,
            shop_pick: None,
            abducted_by: None,
        }
    }
}

impl Brain {
    /// The plan step currently being executed.
    pub fn current_step(&self) -> Option<&ActionInstance> {
        self.plan.as_ref().and_then(|p| p.steps.get(usize::from(self.plan_step)))
    }

    /// Drop the plan and reset execution.
    pub fn clear_plan(&mut self) {
        self.plan = None;
        self.plan_step = 0;
        self.exec = ExecState::Idle;
        self.following_order = None;
    }

    /// The current plan's goal, if any.
    pub fn plan_goal(&self) -> Option<GoalKind> {
        self.plan.as_ref().map(|p| p.goal)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub kind: MemoryKind,
    pub subject: Option<EntityId>,
    pub tick: Tick,
    /// `0.0..=1.0` magnitude.
    pub salience: f32,
    /// `-1.0..=1.0` sign of mood impact.
    pub valence: f32,
    /// Heard rather than seen.
    pub second_hand: bool,
    /// For SawCrime: which crime, so a report names it after salience has decayed.
    #[serde(default)]
    pub crime: Option<Crime>,
}

/// Cap 24; evict lowest `salience * recency`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Memory {
    pub entries: Vec<MemoryEntry>,
}

/// `f32 0.0..=1.0`, initial `U(0.1, 0.4)`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skills {
    pub stealth: f32,
    pub fighting: f32,
    pub farming: f32,
    /// M14 (plan V35): `hack_seed_scale x U(0,1)^3` from a keyed stream. A
    /// pre-M14 save reads -1 (`Skills::unset_hacking`), backfilled by
    /// `migrate_legacy`.
    #[serde(default = "Skills::unset_hacking")]
    pub hacking: f32,
}

impl Skills {
    pub fn unset_hacking() -> f32 {
        -1.0
    }
}

/// Present only while jailed.
/// M12 D27: living in a derelict building, rent-free and unhoused (a Dreg).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Squatter {
    pub building: EntityId,
    pub since: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sentence {
    pub until_tick: Tick,
    pub crime: Crime,
}

/// `rank`: 0 grunt, 1 lieutenant, 2 leader.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GangMember {
    pub gang: EntityId,
    pub rank: u8,
    #[serde(default)]
    pub joined_tick: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corpse {
    pub died_tick: Tick,
    pub cause: DeathCause,
    pub buried: bool,
    /// Freed a day after this.
    #[serde(default)]
    pub buried_tick: Option<Tick>,
    /// M13 D13: the coins and goods the dead carried, until stripped or settled.
    #[serde(default)]
    pub loot: Loot,
    /// M13 D13: stripped (by hand or off screen).
    #[serde(default)]
    pub stripped: bool,
    /// M13 D13: inheritance has run on what was left (`assets::settle_corpse`).
    #[serde(default)]
    pub settled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionInstance {
    pub action: ActionKind,
    /// Building or agent.
    pub target: Option<EntityId>,
    pub tile: Option<TilePos>,
}

// ---------------------------------------------------------------------------
// Non-agent entities
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub kind: BuildingKind,
    /// Farm only.
    pub production_accum: f32,
    /// Home only: which gang is working this Home and how far along
    /// (`systems::gang::CLAIM_HELD` holds it).
    #[serde(default)]
    pub claim: Option<Claim>,
    /// Homes: children's food owed to the pantry, `0.5 × children` per day.
    #[serde(default)]
    pub child_food_debt: f32,
    pub rect: Rect,
    pub door: TilePos,
    /// `0..=cap` per kind table.
    pub stock_food: u32,
    pub capacity: u8,
    /// `None` = city-owned.
    pub owner: Option<EntityId>,
    /// Sorted.
    pub occupants: Vec<EntityId>,
    /// Set by `DemolishHome`; drawn dashed, never used.
    pub demolished: bool,
    /// M10: 0 Sump, 1 Mid, 2 Spire (the map's seventh `B` field). Scales the
    /// Sleep safety bonus at home; rent is M11.
    #[serde(default = "default_tier")]
    pub tier: u8,
    /// M11 D6: a Home's daily rent, split among its adult residents; set by
    /// the owner at the rent pass.
    #[serde(default)]
    pub rent_per_day: i64,
    /// M11: coins earned through this building today (gross), and the last
    /// seven days, newest last.
    #[serde(default)]
    pub revenue_today: i64,
    #[serde(default)]
    pub revenue: VecDeque<i64>,
    /// M11 D19: the security corp holding a contract on this building (phase 3).
    #[serde(default)]
    pub secured_by: Option<EntityId>,
    /// M12 D25: abandoned: no owner, no rent, no staff; squatters may move in.
    #[serde(default)]
    pub derelict: bool,
    /// M12 fix pass: the capacity a derelict had before it went derelict (a
    /// demolished Block stands at half), restored when it returns to use.
    #[serde(default)]
    pub full_capacity: Option<u8>,
    /// M12 D25/D26: a Block with no residents since this tick (abandonment),
    /// and, once derelict, the tick it went derelict (re-letting).
    #[serde(default)]
    pub empty_since: Option<Tick>,
    /// M12 D33: looted by a riot; no trade or beds until this tick.
    #[serde(default)]
    pub closed_until: Option<Tick>,
    /// M13 D6: Stims and Parts in stock (`Good as usize - 1`); Food keeps `stock_food`.
    #[serde(default)]
    pub stock_goods: [u32; 2],
    /// M13 D17: assets sold here today, and the last seven days (newest last).
    #[serde(default)]
    pub asset_sales_today: u16,
    #[serde(default)]
    pub asset_sales: VecDeque<u16>,
    /// M14 (plan V25): the building node's ICE.
    #[serde(default, skip_serializing_if = "crate::virt::SecurityProfile::is_bare")]
    pub security: crate::virt::SecurityProfile,
    /// M14 (plan V16): a Lab's track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<crate::virt::Track>,
    /// M14 (plan V32/V33, phase 3 writes it): a hack on its node, until.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hacked: Option<(crate::virt::HackEffect, Tick)>,
}

pub fn default_tier() -> u8 {
    1
}

impl Building {
    /// Interior tiles (everything inside the perimeter), row-major.
    pub fn interior(&self) -> impl Iterator<Item = TilePos> + '_ {
        let r = self.rect;
        let (x0, y0) = (u16::from(r.x) + 1, u16::from(r.y) + 1);
        let (x1, y1) = (u16::from(r.x) + u16::from(r.w) - 1, u16::from(r.y) + u16::from(r.h) - 1);
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| TilePos { x: x as u8, y: y as u8 }))
    }

    pub fn is_full(&self) -> bool {
        self.occupants.len() >= usize::from(self.capacity)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gang {
    pub name: String,
    /// Sorted.
    pub members: Vec<EntityId>,
    pub treasury: i64,
    /// Homes held (claim count 3), sorted. The Hideout is `hideout`, not territory.
    pub territory: Vec<EntityId>,
    pub leader: Option<EntityId>,
    pub empty_since: Option<Tick>,
    /// This gang's Hideout building.
    #[serde(default = "EntityId::none")]
    pub hideout: EntityId,
    /// Standing order from the faction brain.
    #[serde(default)]
    pub order: Order,
    #[serde(default)]
    pub order_since: Tick,
    /// Every order's score from the last rescoring, best first. Not saved.
    #[serde(skip)]
    pub order_trace: Vec<OrderScore>,
    /// Scheduled muster departure while `order.is_raid()`.
    #[serde(default)]
    pub raid_at: Option<Tick>,
    /// Departure tick of the last raid this gang launched (cooldown).
    #[serde(default)]
    pub last_raid_tick: Option<Tick>,
    #[serde(default)]
    pub retaliate_until: Option<Tick>,
    /// The Hideout is unusable until this tick.
    #[serde(default)]
    pub sacked_until: Option<Tick>,
    /// Pending since the last rescoring; drained by the brain. Not saved.
    #[serde(skip)]
    pub shocks: Vec<Shock>,
    /// `(tick, member)` arrested or killed, newest last, capped at 64.
    #[serde(default)]
    pub heat_log: VecDeque<(Tick, EntityId)>,
    /// M9: the leader at the moment of their arrest, while they sit inside.
    /// The acting leader leans toward breaking them out.
    #[serde(default)]
    pub boss: Option<EntityId>,
    /// M9: departure of the last breakout (its own cooldown).
    #[serde(default)]
    pub last_breakout_tick: Option<Tick>,
    /// M9: the captain has been paid or has refused; the gang makes no
    /// further offer until here.
    #[serde(default)]
    pub bribe_until: Option<Tick>,
    /// M9: a bribe was taken; no Crackdown against this gang until here.
    #[serde(default)]
    pub paid_until: Option<Tick>,
    /// M11 D49: who decides; `Dictator` = the leader. No Board in M11.
    #[serde(default)]
    pub governance: Governance,
    /// M12 D36: the old leader, when a decapitation (death or arrest) just
    /// handed the gang a new one; `gang::run` resolves the split check.
    #[serde(default)]
    pub split_check: Option<EntityId>,
    /// M12 D39: the corp building the standing Raid aims at; `None` = the
    /// rival Hideout (M8).
    #[serde(default)]
    pub raid_target: Option<EntityId>,
    /// M12 D40: the roster emptied after having members (a dead gang, not a
    /// fresh one); cleared by the next recruit.
    #[serde(default)]
    pub emptied: bool,
    /// M12 D40: the claims of an empty gang were cleared.
    #[serde(default)]
    pub claims_cleared: bool,
    /// M12 D36: the gang this one split from (lineage for the Hideout panel).
    #[serde(default)]
    pub split_from: Option<EntityId>,
    /// M13 D36: the Harvest target (`chrome::harvest_target`), cached at each
    /// rescore so a member's GangWork never rescans the chromed.
    #[serde(default)]
    pub harvest_target: Option<EntityId>,
}

impl Gang {
    pub fn new(name: String, hideout: EntityId, treasury: i64) -> Gang {
        Gang {
            name,
            members: Vec::new(),
            treasury,
            territory: Vec::new(),
            leader: None,
            empty_since: Some(0),
            hideout,
            order: Order::Expand,
            order_since: 0,
            order_trace: Vec::new(),
            raid_at: None,
            last_raid_tick: None,
            retaliate_until: None,
            sacked_until: None,
            shocks: Vec::new(),
            heat_log: VecDeque::new(),
            boss: None,
            last_breakout_tick: None,
            bribe_until: None,
            paid_until: None,
            governance: Governance::Dictator,
            split_check: None,
            raid_target: None,
            emptied: false,
            claims_cleared: false,
            split_from: None,
            harvest_target: None,
        }
    }

    pub fn is_sacked(&self, now: Tick) -> bool {
        self.sacked_until.is_some_and(|t| t > now)
    }
}

/// Stock lives on the Market building's `Building.stock_food`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Market {
    /// `1..=30`.
    pub price_food: i64,
    /// Cap 120, one per day.
    pub price_history: VecDeque<i64>,
    /// M11 D16: units sold today, and the last seven days' sales and
    /// opening stock (newest last), rolled in `economy::daily_price`.
    #[serde(default)]
    pub sales_today: u32,
    #[serde(default)]
    pub sales: VecDeque<u32>,
    #[serde(default)]
    pub stock_hist: VecDeque<u32>,
    /// M11 phase 5: the price in tenths of a coin (`[economy] price_tenths`);
    /// 0 reads as `price_food x 10` (an old save, the flag off).
    #[serde(default)]
    pub price_tenths: i64,
    /// M13 D6/D40: legal Stims doses sold today (phase 4).
    #[serde(default)]
    pub stim_sales_today: u32,
}

impl Market {
    /// The price in tenths of a coin.
    pub fn tenths(&self) -> i64 {
        if self.price_tenths > 0 {
            self.price_tenths
        } else {
            self.price_food * 10
        }
    }

    pub fn new(price_food: i64) -> Market {
        Market {
            price_food,
            price_history: VecDeque::new(),
            sales_today: 0,
            sales: VecDeque::new(),
            stock_hist: VecDeque::new(),
            price_tenths: 0,
            stim_sales_today: 0,
        }
    }
}

/// May go negative; wages unpaid while `< 0`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Treasury {
    pub coins: i64,
}

/// Stored in `World::crime_reports`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrimeReport {
    pub crime: Crime,
    pub suspect: EntityId,
    /// `None` for a Statistical-tick theft.
    pub witness: Option<EntityId>,
    pub tick: Tick,
    pub resolved: bool,
}

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

/// Key is always `(min(a, b), max(a, b))`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    /// `-1.0..=1.0`, default 0.0.
    pub affinity: f32,
    /// `0.0..=1.0`, default 0.3.
    #[serde(default = "first_trust", skip_serializing_if = "is_first_trust")]
    pub trust: f32,
    /// Coins the lower id owes the higher id; negative = reverse.
    /// This, `trust` and the two `Option`s are left out of a save at their
    /// defaults (M10 review: ~55 of ~170 bytes per edge, and edges are most
    /// of a save; an off-screen acquaintance never touched again keeps all four).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub debt: i32,
    pub kind: RelKind,
    pub last_interaction: Tick,
    /// Spouse edges only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_birth_tick: Option<Tick>,
    /// When the current debt was incurred (ageing charges after 14 days).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debt_since: Option<Tick>,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn first_trust() -> f32 {
    0.3
}

fn is_first_trust(v: &f32) -> bool {
    *v == first_trust()
}

impl Edge {
    pub fn new(kind: RelKind, tick: Tick) -> Self {
        Edge {
            affinity: 0.0,
            trust: 0.3,
            debt: 0,
            kind,
            last_interaction: tick,
            last_birth_tick: None,
            debt_since: None,
        }
    }
}

/// Canonical edge key.
pub fn edge_key(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

// ---------------------------------------------------------------------------
// M10: holes (off-screen crimes whose actor is drawn lazily) and traces
// ---------------------------------------------------------------------------

/// `(tick << 24) | (victim.index << 2) | kind` (M10 D8): unique per victim,
/// kind and tick, and ascending ids are oldest first.
pub type HoleId = u64;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum HoleKind {
    Robbed,
    Assaulted,
    Killed,
    /// M13 D37: taken off screen for the chrome (packs as 3 in `hole_id`).
    Abducted,
}

impl HoleKind {
    /// The crime a witness reports once the hole is bound.
    pub fn crime(self) -> Crime {
        match self {
            HoleKind::Robbed => Crime::Theft,
            HoleKind::Assaulted => Crime::Assault,
            HoleKind::Killed => Crime::Murder,
            HoleKind::Abducted => Crime::Abduction,
        }
    }

    /// "robbery", "beating", "killing".
    pub fn noun(self) -> &'static str {
        match self {
            HoleKind::Robbed => "robbery",
            HoleKind::Assaulted => "beating",
            HoleKind::Killed => "killing",
            HoleKind::Abducted => "abduction",
        }
    }
}

/// The packed hole key (M10 D8).
pub fn hole_id(tick: Tick, victim: EntityId, kind: HoleKind) -> HoleId {
    debug_assert!(victim.index < 1 << 22, "hole key packs the victim index in 22 bits");
    (tick << 24) | (u64::from(victim.index) << 2) | kind as u64
}

/// An off-screen crime against a Statistical victim, actor not yet drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hole {
    pub id: HoleId,
    pub kind: HoleKind,
    pub victim: EntityId,
    pub zone: Zone,
    /// M12 D5: the district of the victim's tile; UNSET in a pre-M12 save
    /// until `migrate_legacy` sets the zone's first district.
    #[serde(default = "DistrictId::unset")]
    pub district: DistrictId,
    pub tick: Tick,
    /// The ring entry to rewrite on binding.
    pub event_id: u64,
    /// Killed, or the victim is a gang member or a guard: bound by the next daily pass.
    pub consequential: bool,
    /// Captured at creation: `World::spouses` forgets the dead (M10 D10).
    #[serde(default)]
    pub spouse: Option<EntityId>,
    /// Coins a Robbed victim lost; paid to the actor at bind, gone on Unknown.
    #[serde(default)]
    pub loot: i64,
    /// The victim's Home and gang at creation: a Killed victim's are gone by
    /// bind time, and the binder's gang-claim weight and gang shock need them.
    #[serde(default)]
    pub home: Option<EntityId>,
    #[serde(default)]
    pub gang: Option<EntityId>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Bound {
    Actor(EntityId),
    Unknown,
}

/// Bits of `DayTrace::flags`.
pub mod trace_flags {
    pub const ALIVE: u16 = 1;
    pub const JAILED: u16 = 2;
    pub const HOMELESS: u16 = 4;
    pub const EMPLOYED: u16 = 8;
    pub const GANG: u16 = 16;
    pub const STATISTICAL_ALL_DAY: u16 = 32;
    pub const SLEPT_AT_HOME: u16 = 64;
    pub const ATE: u16 = 128;
    /// M12 phase 3: slept in a booked Hotel bed.
    pub const HOTEL: u16 = 1 << 8;
    /// M12 phase 3: slept in a squat.
    pub const SQUAT: u16 = 1 << 9;
}

/// One day of an adult's life, packed into a u32 for the save (M12 D4):
/// zone (3 bits) | flags 0-7 (8) << 3 | hunger band (2) << 11 | mood band (2) << 13
/// | district (4) << 15 | has-district (1) << 19 | flags 8-9 (2) << 20.
/// A pre-M12 value has bit 19 clear and decodes `district = UNSET`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "u32", into = "u32")]
pub struct DayTrace {
    /// The zone the agent ended the day in.
    pub zone: Zone,
    /// The district the agent ended the day in.
    pub district: DistrictId,
    pub flags: u16,
    /// `0..=3` band at day end.
    pub hunger: u8,
    /// `0..=3` band at day end.
    pub mood: u8,
}

/// Bit 19 of a packed `DayTrace`: the district nibble is meaningful.
const TRACE_HAS_DISTRICT: u32 = 1 << 19;

impl DayTrace {
    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    /// Hunger `< 0.25 → 0, < 0.5 → 1, < 0.75 → 2, else 3`.
    pub fn hunger_band(h: f32) -> u8 {
        if h < 0.25 {
            0
        } else if h < 0.5 {
            1
        } else if h < 0.75 {
            2
        } else {
            3
        }
    }

    /// Mood (`-1..1`) `< -0.5 → 0, < 0 → 1, < 0.5 → 2, else 3`.
    pub fn mood_band(m: f32) -> u8 {
        if m < -0.5 {
            0
        } else if m < 0.0 {
            1
        } else if m < 0.5 {
            2
        } else {
            3
        }
    }
}

impl From<u32> for DayTrace {
    fn from(v: u32) -> DayTrace {
        let z = (v & 0b111) as usize;
        let district =
            if v & TRACE_HAS_DISTRICT != 0 { DistrictId(((v >> 15) & 0xf) as u8) } else { DistrictId::UNSET };
        DayTrace {
            zone: Zone::ALL[z.min(Zone::ALL.len() - 1)],
            district,
            flags: ((v >> 3) & 0xff) as u16 | (((v >> 20) & 0b11) as u16) << 8,
            hunger: ((v >> 11) & 0b11) as u8,
            mood: ((v >> 13) & 0b11) as u8,
        }
    }
}

impl From<DayTrace> for u32 {
    fn from(t: DayTrace) -> u32 {
        let district = if t.district.is_unset() { 0 } else { u32::from(t.district.0 & 0xf) << 15 | TRACE_HAS_DISTRICT };
        t.zone.index() as u32
            | u32::from(t.flags & 0xff) << 3
            | u32::from(t.hunger & 0b11) << 11
            | u32::from(t.mood & 0b11) << 13
            | district
            | u32::from((t.flags >> 8) & 0b11) << 20
    }
}

/// The last `[lod] trace_days` days of an adult, newest last. Survives death
/// (the binder and the biography read the past of the dead too).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    /// The day of the newest entry.
    pub last_day: u64,
    pub days: VecDeque<DayTrace>,
}

impl Trace {
    /// The entry for `day`, if recorded and not yet evicted.
    pub fn on_day(&self, day: u64) -> Option<DayTrace> {
        if self.days.is_empty() || day > self.last_day {
            return None;
        }
        let back = usize::try_from(self.last_day - day).ok()?;
        let n = self.days.len();
        if back >= n {
            return None;
        }
        self.days.get(n - 1 - back).copied()
    }

    /// Record `day`, one entry per day, the oldest evicted past `cap`.
    pub fn push(&mut self, day: u64, t: DayTrace, cap: usize) {
        if !self.days.is_empty() && day <= self.last_day {
            // Same day twice (a re-run of the daily pass): replace.
            if day == self.last_day {
                if let Some(last) = self.days.back_mut() {
                    *last = t;
                }
            }
            return;
        }
        // Keep indices contiguous: a skipped day repeats the previous entry.
        if let Some(&prev) = self.days.back() {
            let gap = (day - self.last_day - 1).min(cap as u64);
            for _ in 0..gap {
                self.days.push_back(prev);
            }
        }
        self.days.push_back(t);
        self.last_day = day;
        while self.days.len() > cap.max(1) {
            self.days.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// M11: classes (docs/M11_OWNERSHIP.md § 7)
// ---------------------------------------------------------------------------

/// Derived, never stored: Corp = employed by a corp-owned building or a
/// corp's exec; Dreg = no Home; Street = everyone else.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Class {
    Corp,
    Street,
    Dreg,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Corp, Class::Street, Class::Dreg];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Class::Corp => "Corp",
            Class::Street => "Street",
            Class::Dreg => "Dreg",
        }
    }
}

/// One class's daily aggregate (§ 7), recomputed at midnight after rent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClassAggregate {
    /// Adults with a Brain in the class.
    pub count: u32,
    /// Mean `(mood + 1) / 2`.
    pub happiness: f32,
    /// Employed adults / adults.
    pub employment: f32,
    /// Mean over members of yesterday's guard-hours near their Home ÷
    /// `fear_hours_full`, clamped to 1 (D34; a Dreg reads its zone's Homes).
    pub fear: f32,
    /// `happiness × min(employment + 0.5, 1)`.
    pub loyalty: f32,
    /// `0.3 + 0.7 × fear`.
    pub submission: f32,
    /// `(1 − loyalty) × (1 − submission) + 0.1 × evictions_7d ÷ count`.
    pub unrest: f32,
    pub evictions_7d: u32,
    /// The inputs, for the City panel; not saved.
    #[serde(skip)]
    pub trace: Vec<(&'static str, f32)>,
}

/// D34: guard-hours per Home (on-shift guards within `fear_radius` of the
/// door, counted on the hour), today and yesterday.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HomeWatch {
    pub today: BTreeMap<EntityId, u16>,
    pub yesterday: BTreeMap<EntityId, u16>,
}

/// Guard-on-shift ticks per zone (`Zone::index`), today and yesterday (M10 D33).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZoneWatch {
    pub today: [u32; 5],
    pub yesterday: [u32; 5],
}

// ---------------------------------------------------------------------------
// M12: districts (docs/M12_DISTRICTS.md § 1)
// ---------------------------------------------------------------------------

/// Most districts a config may define (the `[u32; 12]` watch slots, the
/// low nibble of `World::district_grid`).
pub const MAX_DISTRICTS: usize = 12;

/// A district's index in `World::districts` (`[districts]` row order).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct DistrictId(pub u8);

impl DistrictId {
    /// Not yet known: a pre-M12 trace entry or hole, fixed by `World::migrate_legacy`.
    pub const UNSET: DistrictId = DistrictId(255);

    pub fn unset() -> DistrictId {
        DistrictId::UNSET
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn is_unset(self) -> bool {
        self == DistrictId::UNSET
    }
}

impl District {
    /// M12 phase 2 (plan deviation on "inhabited = `adults > 0`"): the
    /// district has standing Blocks and adults binned to it. A few homeless
    /// adults sleeping in the Civic made it "inhabited" at a crime rate of
    /// 257 per 100, drew Crackdowns and six to eight guards.
    pub fn inhabited(&self) -> bool {
        self.adults > 0 && !self.homes.is_empty()
    }
}

/// Who controls a district (plan D8): the top presence with at least
/// `[districts] control_min_share` of the total, else Contested.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Controller {
    #[default]
    Contested,
    City,
    Gang(EntityId),
    Corp(EntityId),
}

impl Controller {
    /// The CSV code (plan D45): 0 Contested, 1 City, 2 Gang, 3 Corp.
    pub fn csv_code(self) -> u8 {
        match self {
            Controller::Contested => 0,
            Controller::City => 1,
            Controller::Gang(_) => 2,
            Controller::Corp(_) => 3,
        }
    }

    /// Tie order (plan D8): City, then gangs, then corps; then lower id.
    pub fn tie_rank(self) -> (u8, EntityId) {
        match self {
            Controller::Contested => (3, EntityId::NONE),
            Controller::City => (0, EntityId::NONE),
            Controller::Gang(g) => (1, g),
            Controller::Corp(c) => (2, c),
        }
    }

    /// The controlling entity, if the controller is one.
    pub fn entity(self) -> Option<EntityId> {
        match self {
            Controller::Gang(e) | Controller::Corp(e) => Some(e),
            Controller::Contested | Controller::City => None,
        }
    }
}

/// The captain's per-district stance (phase 2 scores it; Patrol until then).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Stance {
    #[default]
    Patrol,
    Crackdown(EntityId),
    Sweep,
    Cordon,
    Withdrawn,
}

/// M12 D30: what a riot marches on.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RiotTarget {
    /// A corp-owned Market, Block or Hotel in the district.
    Corp,
    /// The Precinct.
    Precinct,
    /// The controlling gang's Hideout or held squat.
    Gang,
}

/// M12 D34: how the law meets a riot.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RiotResponse {
    /// Cordon the district; the guards fight only at the door.
    Contain,
    /// Fight at the door, then report the stragglers.
    Disperse,
    /// Cordon, and fight to kill (`crush_kill_mult`); fear for 14 days.
    Crush,
}

/// M12 D30: a district riot from its trigger at midnight to its clash (or
/// its fizzling). Rioters run the Raid goal (`raid::Expedition::Riot`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Riot {
    pub id: u32,
    pub district: DistrictId,
    pub target: EntityId,
    pub kind: RiotTarget,
    /// The muster building (rioters gather at its door).
    pub muster: EntityId,
    pub muster_at: Tick,
    /// Most miserable first, ties the lower id (`riot::eligible`'s order;
    /// `riot::promoted` promotes the head of it).
    pub rioters: Vec<EntityId>,
    pub response: RiotResponse,
    pub started: Tick,
    /// The first rioter out of the muster stamps it.
    #[serde(default)]
    pub departed: Option<Tick>,
}

/// Guard-on-shift ticks per district (`DistrictId::index`), today and yesterday.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DistrictWatch {
    pub today: [u32; MAX_DISTRICTS],
    pub yesterday: [u32; MAX_DISTRICTS],
}

/// One district: a cut of a zone (plan "District cuts"), its daily
/// aggregates (recomputed at midnight by `systems::districts::daily`; `trace`
/// explains each), its controller and, from phase 2, its law.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct District {
    pub id: DistrictId,
    pub name: String,
    pub zone: Zone,
    /// Buildings whose door is inside, sorted; rebuilt on load and on any wall change.
    #[serde(skip)]
    pub buildings: Vec<EntityId>,
    /// Walkable tiles inside no building rect (the litter denominator).
    #[serde(skip)]
    pub walk_tiles: u32,
    /// M12 phase 3: those tiles' grid indices, ascending (litter draws,
    /// means and sweeping).
    #[serde(skip)]
    pub streets: Vec<u32>,
    /// Standing, non-derelict Blocks (Homes) whose door is inside, sorted.
    #[serde(skip)]
    pub homes: Vec<EntityId>,
    /// Living adults binned here by the last aggregate pass, ascending.
    /// M14 phase 1 fix: saved (it was skipped, so a resumed world read no
    /// residents until the next midnight and a crash's Statistical-victim
    /// fallback found nobody: save/load diverged on seed 3).
    #[serde(default)]
    pub residents: Vec<EntityId>,
    /// Mean walkable street tile, rounded.
    #[serde(skip)]
    pub centroid: TilePos,
    // --- daily aggregates ---
    /// Every resident binned here (adults and children).
    #[serde(default)]
    pub population: u32,
    /// Living adults with a Brain binned here.
    #[serde(default)]
    pub adults: u32,
    /// Corp, Street, Dreg adults (`Class::index`).
    #[serde(default)]
    pub classes: [u32; 3],
    /// Mean `(mood + 1) / 2` of the resident adults.
    #[serde(default)]
    pub happiness: f32,
    /// `bind::district_coverage`, normalised `coverage_min..=coverage_max`.
    #[serde(default)]
    pub coverage: f32,
    #[serde(default)]
    pub fear: f32,
    /// Phase 4: Street + Dreg residents only.
    #[serde(default)]
    pub unrest: f32,
    #[serde(default)]
    pub unrest_streak: u16,
    /// M12 D41: the same formula over its Street-class residents only (the
    /// strike trigger: Dregs hold no shifts to walk out of).
    #[serde(default)]
    pub street_unrest: f32,
    /// Phase 3: mean litter ÷ 255 over `walk_tiles`.
    #[serde(default)]
    pub litter: f32,
    /// Crimes per day, the last 7, newest last.
    #[serde(default)]
    pub crimes: VecDeque<u16>,
    /// Crimes today, rolled into `crimes` at midnight.
    #[serde(default)]
    pub crimes_today: u16,
    /// Crimes per 100 residents per day over the last 7 days.
    #[serde(default)]
    pub crime_rate: f32,
    #[serde(default)]
    pub control: Controller,
    #[serde(default)]
    pub control_share: f32,
    #[serde(default)]
    pub control_since: Tick,
    /// The first control computation after a seed or a load is silent.
    #[serde(default)]
    pub control_init: bool,
    /// The last non-Contested controller (phase 1 review): a faction is
    /// shocked only when another faction takes the district from it, not
    /// when its share wobbles across `control_min_share` into Contested.
    #[serde(default)]
    pub last_holder: Controller,
    // --- the law (phase 2) ---
    #[serde(default)]
    pub stance: Stance,
    #[serde(default)]
    pub stance_since: Tick,
    /// City guards allocated today.
    #[serde(default)]
    pub guards: u8,
    /// Sanitation workers allocated today (phase 3).
    #[serde(default)]
    pub sweepers: u8,
    #[serde(default)]
    pub curfew: bool,
    #[serde(default)]
    pub crush_until: Option<Tick>,
    /// Rough sleepers last night (phase 2/3).
    #[serde(default)]
    pub rough: u16,
    #[serde(default)]
    pub vagrancy_log: VecDeque<Tick>,
    #[serde(default)]
    pub shakedowns: VecDeque<(Tick, EntityId)>,
    #[serde(default)]
    pub last_riot: Option<Tick>,
    #[serde(default)]
    pub last_strike: Option<Tick>,
    /// The inputs of the last daily pass, by name, for the District panel.
    #[serde(skip)]
    pub trace: Vec<(&'static str, f32)>,
    /// M12 D12: every stance's score from the last rescoring, best first.
    #[serde(skip)]
    pub stance_trace: Vec<StanceScore>,
    /// M12 D10: the allocation weight's terms from the last allocation.
    #[serde(skip)]
    pub alloc_trace: Vec<(&'static str, f32)>,
}

// ---------------------------------------------------------------------------
// M10: lives (the biography's events)
// ---------------------------------------------------------------------------

/// What happened to an agent, as the Story tab tells it. Written only by
/// `events::record_life` and `events::life_bound`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum LifeKind {
    Born,
    Married,
    Widowed,
    Hired,
    Fired,
    Quit,
    Paid,
    Starving,
    Robbed,
    Assaulted,
    Stole,
    RobbedSomeone,
    AssaultedSomeone,
    Killed,
    KilledSomeone,
    Died,
    Arrested,
    Released,
    Escaped,
    JoinedGang,
    LeftGang,
    Betrayed,
    Evicted,
    Housed,
    Immigrated,
    Buried,
    Witnessed,
    /// M11: opened a business on a Lot (phase 4).
    Founded,
    /// M11: became the exec of their own corp (phase 4).
    Incorporated,
}

impl LifeKind {
    /// Never evicted from the list.
    pub fn permanent(self) -> bool {
        matches!(
            self,
            LifeKind::Born
                | LifeKind::Married
                | LifeKind::Widowed
                | LifeKind::Killed
                | LifeKind::KilledSomeone
                | LifeKind::Died
        )
    }

    /// The eviction weight a fresh entry of this kind starts with.
    pub fn salience(self) -> f32 {
        match self {
            LifeKind::Born
            | LifeKind::Married
            | LifeKind::Widowed
            | LifeKind::Killed
            | LifeKind::KilledSomeone
            | LifeKind::Died => 1.0,
            LifeKind::Betrayed
            | LifeKind::RobbedSomeone
            | LifeKind::AssaultedSomeone
            | LifeKind::Founded
            | LifeKind::Incorporated => 0.9,
            LifeKind::Robbed | LifeKind::Assaulted | LifeKind::Arrested | LifeKind::Escaped | LifeKind::JoinedGang => {
                0.7
            }
            LifeKind::Fired | LifeKind::LeftGang | LifeKind::Evicted => 0.6,
            LifeKind::Hired | LifeKind::Starving | LifeKind::Stole | LifeKind::Released | LifeKind::Immigrated => 0.5,
            LifeKind::Quit | LifeKind::Housed => 0.4,
            LifeKind::Buried | LifeKind::Witnessed => 0.3,
            LifeKind::Paid => 0.2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifeEvent {
    pub tick: Tick,
    pub kind: LifeKind,
    pub other: Option<EntityId>,
    /// An open victim hole: "by an unknown assailant" until it is bound.
    pub hole: Option<HoleId>,
    pub salience: f32,
}

/// An agent's biography: at most `LIFE_CAP` events, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Life {
    pub events: Vec<LifeEvent>,
}

pub const LIFE_CAP: usize = 48;

// ---------------------------------------------------------------------------
// M13 assets (docs/M13_ASSETS.md § 1, plan phase 1)
// ---------------------------------------------------------------------------

/// An implant's place in the body: one implant per slot.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Slot {
    Arms,
    Legs,
    Nerves,
    Eyes,
    Skin,
}

impl Slot {
    pub const ALL: [Slot; 5] = [Slot::Arms, Slot::Legs, Slot::Nerves, Slot::Eyes, Slot::Skin];

    pub fn label(self) -> &'static str {
        match self {
            Slot::Arms => "Arms",
            Slot::Legs => "Legs",
            Slot::Nerves => "Nerves",
            Slot::Eyes => "Eyes",
            Slot::Skin => "Skin",
        }
    }
}

/// What an asset is. Appended only (saves name the variants).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum AssetKind {
    Motorcycle,
    Car,
    Truck,
    Flyer,
    Implant(Slot),
    Robot,
    Pack,
    /// M14 (plan D52): a Virt bridge, carried then planted at a building.
    /// Only its asset entry exists in M13.
    Bridge,
}

/// The lever and CSV grouping of an `AssetKind` (implants by class, not slot).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum AssetClass {
    Motorcycle,
    Car,
    Truck,
    Flyer,
    Implant,
    Robot,
    Pack,
    /// Plan D52.
    Bridge,
}

impl AssetClass {
    pub const ALL: [AssetClass; 8] = [
        AssetClass::Motorcycle,
        AssetClass::Car,
        AssetClass::Truck,
        AssetClass::Flyer,
        AssetClass::Implant,
        AssetClass::Robot,
        AssetClass::Pack,
        AssetClass::Bridge,
    ];

    /// Position in `ALL` (indexes `Levers::asset_tax`).
    pub fn index(self) -> usize {
        self as usize
    }
}

impl AssetKind {
    pub fn class(self) -> AssetClass {
        match self {
            AssetKind::Motorcycle => AssetClass::Motorcycle,
            AssetKind::Car => AssetClass::Car,
            AssetKind::Truck => AssetClass::Truck,
            AssetKind::Flyer => AssetClass::Flyer,
            AssetKind::Implant(_) => AssetClass::Implant,
            AssetKind::Robot => AssetClass::Robot,
            AssetKind::Pack => AssetClass::Pack,
            AssetKind::Bridge => AssetClass::Bridge,
        }
    }

    /// Motorcycle, Car, Truck, Flyer.
    pub fn is_vehicle(self) -> bool {
        matches!(self, AssetKind::Motorcycle | AssetKind::Car | AssetKind::Truck | AssetKind::Flyer)
    }

    /// A vehicle that keeps to the ground (not the Flyer).
    pub fn is_road_vehicle(self) -> bool {
        self.is_vehicle() && self != AssetKind::Flyer
    }

    pub fn is_implant(self) -> bool {
        matches!(self, AssetKind::Implant(_))
    }

    pub fn label(self) -> &'static str {
        match self {
            AssetKind::Motorcycle => "Motorcycle",
            AssetKind::Car => "Car",
            AssetKind::Truck => "Truck",
            AssetKind::Flyer => "Flyer",
            AssetKind::Implant(Slot::Arms) => "Arms implant",
            AssetKind::Implant(Slot::Legs) => "Legs implant",
            AssetKind::Implant(Slot::Nerves) => "Nerves implant",
            AssetKind::Implant(Slot::Eyes) => "Eyes implant",
            AssetKind::Implant(Slot::Skin) => "Skin implant",
            AssetKind::Robot => "Security robot",
            AssetKind::Pack => "Pack",
            AssetKind::Bridge => "Bridge",
        }
    }
}

/// A traded good. Food lives in `Building.stock_food`; Stims and Parts in
/// `Building.stock_goods[good as usize - 1]` (plan D6).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Good {
    Food,
    Stims,
    Parts,
}

impl Good {
    pub const ALL: [Good; 3] = [Good::Food, Good::Stims, Good::Parts];

    pub fn label(self) -> &'static str {
        match self {
            Good::Food => "Food",
            Good::Stims => "Stims",
            Good::Parts => "Parts",
        }
    }
}

/// Where an asset is. Write it only through `assets::set_loc` (plan D2).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AssetLoc {
    /// At a building (its door): Homes, workplaces, Garages, Hideouts.
    Parked(EntityId),
    /// Being driven by an agent.
    InUse(EntityId),
    /// A pack (or a bridge) on an agent.
    Carried(EntityId),
    /// Chrome in a body (a living agent or a corpse).
    Installed(EntityId),
    /// A robot guarding a building.
    Posted(EntityId),
    /// For sale at a Clinic, Garage or Security Office, or a gang's take at its Hideout.
    Stock(EntityId),
    /// Taken by an unbound Abducted hole.
    Limbo(HoleId),
}

impl AssetLoc {
    /// The building or agent the loc names (`None` for Limbo).
    pub fn holder(self) -> Option<EntityId> {
        match self {
            AssetLoc::Parked(e)
            | AssetLoc::InUse(e)
            | AssetLoc::Carried(e)
            | AssetLoc::Installed(e)
            | AssetLoc::Posted(e)
            | AssetLoc::Stock(e) => Some(e),
            AssetLoc::Limbo(_) => None,
        }
    }
}

/// A finance plan (plan D10): `lender: None` is the city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finance {
    pub lender: Option<EntityId>,
    pub remaining: i64,
    pub per_day: i64,
    pub arrears: u8,
}

/// An owned thing (plan D1): an entity carrying only this component.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub kind: AssetKind,
    /// 1..=3; also the lock tier (vehicles) and the sensor tier (robots).
    pub tier: u8,
    /// M11 owner; `None` = the city. Write only through `assets::set_owner`.
    pub owner: Option<EntityId>,
    /// Write only through `assets::set_loc`.
    pub loc: AssetLoc,
    /// 0..=100; 0 = wrecked (vehicles, robots) or failed (chrome).
    pub condition: u8,
    /// `list × condition / 100`, recomputed daily.
    pub value: i64,
    pub upkeep_per_day: i64,
    pub upkeep_arrears: u8,
    pub finance: Option<Finance>,
    /// Chrome locked by its lender: modifiers off, load stays.
    pub bricked: bool,
    /// Set by theft or a rip; cleared by a chop, a fence or recovery.
    pub stolen: bool,
    pub bought: Tick,
    /// Plan D24: who drives a fleet vehicle.
    #[serde(default)]
    pub keeper: Option<EntityId>,
    /// Plan D1: the list price at purchase.
    #[serde(default)]
    pub list: i64,
    /// Phase 5: midnights a kept corp car has stood parked away from its
    /// owner's buildings (recalled at 2, `vehicles::fleet_recall`).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub away_days: u8,
    /// M14 (plan V23): the corp whose tech tier caps this asset's effect;
    /// `None` for pre-M14 assets and god grants (uncapped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maker: Option<EntityId>,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

/// Derived, never saved: rebuilt on load and by `assets::rekit` after any
/// change (plan D3).
#[derive(Clone, Debug, PartialEq)]
pub struct Kit {
    pub vehicle: Option<EntityId>,
    pub fighting: f32,
    pub stealth: f32,
    pub reflex: f32,
    pub strength: f32,
    pub sight: u8,
    pub armour: f32,
    pub walk_mult: f32,
    /// Σ sanity_cost of installed chrome (bricked and failed included).
    pub load: f32,
    pub chrome_value: i64,
    /// Installed tiers in Arms, Eyes, Skin.
    pub visible: u8,
    /// Coarse GotoTimed multiplier of the vehicle (phase 2; 1.0 until then).
    pub timed_mult: f32,
    /// `min(1, (visible + vehicle tier, a flyer as 3) / 9)`.
    pub flash: f32,
    /// Plan D3: the kind of the vehicle being driven, cached from `World::trips`.
    pub driving: Option<AssetKind>,
    /// Plan D3: any implant installed (bricked or failed included).
    pub chrome: bool,
}

impl Default for Kit {
    fn default() -> Self {
        Kit {
            vehicle: None,
            fighting: 0.0,
            stealth: 0.0,
            reflex: 0.0,
            strength: 0.0,
            sight: 0,
            armour: 0.0,
            walk_mult: 1.0,
            load: 0.0,
            chrome_value: 0,
            visible: 0,
            timed_mult: 1.0,
            flash: 0.0,
            driving: None,
            chrome: false,
        }
    }
}

impl Kit {
    /// No vehicle, no implant, not driving: every Kit term is skipped.
    pub fn is_bare(&self) -> bool {
        self.vehicle.is_none() && !self.chrome && self.driving.is_none()
    }
}

/// Body stats (plan D4) on every agent, all tiers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    /// `U(0.2, 0.6)`; children inherit the parents' mean ± 0.05.
    pub strength: f32,
    pub reflex: f32,
    /// 0..=1, default 1.
    pub sanity: f32,
    /// 0..=1.
    pub addiction: f32,
    /// The last stim.
    pub last_use: Option<Tick>,
    /// In a cyberpsychotic episode (phase 3).
    pub episode_until: Option<Tick>,
    /// Plan D4: the last purchase (the Shop cooldown).
    #[serde(default)]
    pub last_shop: Option<Tick>,
}

/// For M15: stored daily, read by nobody in M13 (plan D5).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub dress: u8,
    pub chrome: u8,
    pub colours: Option<EntityId>,
}

/// What a corpse carried (plan D13).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Loot {
    pub coins: i64,
    pub food: u32,
    pub stims: u16,
    pub parts: u16,
}

impl Loot {
    pub fn is_empty(&self) -> bool {
        self.coins == 0 && self.food == 0 && self.stims == 0 && self.parts == 0
    }
}

/// What a shopper is buying, fixed at bind time (plan D29).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShopPick {
    pub kind: AssetKind,
    pub tier: u8,
    /// A used asset in the seller's stock, if the pick is one.
    pub used: Option<EntityId>,
}

/// A vehicle trip in progress (plan D20; filled from phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trip {
    pub vehicle: EntityId,
    pub start: Tick,
    pub from: TilePos,
    pub road_tiles: u16,
    pub steps: u16,
    pub half: u16,
    pub mid: Option<TilePos>,
    pub chase: bool,
}
