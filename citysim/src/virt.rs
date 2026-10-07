//! M14 types (docs/M14_VIRT.md § 1, § 3, § 4, § 5; plan phase 1.1): the
//! Virt plane's nodes and links, Data stores, security profiles, the tech
//! tree, and the run types (filled by phase 2, defined now so saves are
//! stable).
//!
//! Everything here is a game abstraction: a second board of abstract nodes
//! and links over the city map, where a fictional character's attempt to
//! enter a node is a seeded dice contest (character tier plus skill against
//! a node tier), as `law::resolve_fight` resolves a brawl. The behaviour
//! lives in `systems::virt` and `systems::tech`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::components::{DistrictId, TilePos};
use crate::entity::EntityId;
use crate::time::Tick;

/// A run's id: `(start_tick << 22) | runner.index` (plan V10).
pub type RunId = u64;

/// The index into `VirtPlane.nodes`; stable across relinks (plan V1).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct NodeId(pub u16);

impl NodeId {
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// What a node stands for.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum NodeKind {
    /// The street net of a district: terminals, Homes, agent- and city-owned shops.
    Public(DistrictId),
    /// A faction-owned non-Home building, a Lab, a Hideout, the Precinct.
    Building(EntityId),
    /// A corp's treasury; `Ledger(hall)` is the city's Treasury.
    Ledger(EntityId),
}

/// The kind of a node's owner, cached at relink (plan V1).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum OwnerTag {
    #[default]
    City,
    Corp,
    Gang,
}

fn yes() -> bool {
    true
}

/// One node of the plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    /// The faction (a corp, a gang, or an agent for an agent-owned Lab);
    /// `None` = the city.
    pub owner: Option<EntityId>,
    #[serde(default)]
    pub owner_kind: OwnerTag,
    /// Plan V1: a node whose building is gone or no longer qualifies is
    /// kept dead (id and state), skipped by every pass.
    #[serde(default = "yes")]
    pub alive: bool,
    /// The door, or the district's centroid: where the overlay draws it.
    pub pos: TilePos,
    /// After a traced run: +1 to `def` until then (phase 2).
    pub alarm_until: Option<Tick>,
    /// Phase 3 writes it (mirrors `Building.hacked`).
    pub hacked: Option<(HackEffect, Tick)>,
    /// Empty except on Labs and Hideouts.
    pub store: DataStore,
    /// The last eight successful runs.
    pub breaches: VecDeque<Tick>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum LinkKind {
    Street,
    Access,
    Trunk,
}

/// A link between two nodes. `firewall` is the M14 spec addendum's ICE on
/// a link (plan V64): carried in the data model and kept by relink at 0;
/// nothing contests it yet.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Link {
    pub a: NodeId,
    pub b: NodeId,
    pub kind: LinkKind,
    pub tier: u8,
    #[serde(default)]
    pub firewall: u8,
}

/// A route tree's cache key (plan V6).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct RouteKey {
    pub from: NodeId,
    pub deck_eff: u8,
    pub att_q: u16,
    pub patron: EntityId,
}

/// One single-source search (phase 2 fills it).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RouteTree {
    pub dist: Vec<f32>,
    pub pred: Vec<Option<NodeId>>,
    pub hops: Vec<u8>,
}

/// One route read from a tree (phase 2).
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub nodes: SmallVec<[NodeId; 8]>,
    pub p_success: f32,
    pub p_fry: f32,
    pub p_trace: f32,
}

/// The chair building and its portal node (plan V5).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Portal {
    pub building: EntityId,
    pub node: NodeId,
}

/// The plane. `nodes` and `links` are saved; the rest is rebuilt by
/// `systems::virt::rebuild_index` (on load and by every relink).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VirtPlane {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// Per node: `(neighbour, link index)`, alive nodes only.
    #[serde(skip)]
    pub adj: Vec<SmallVec<[(NodeId, u16); 6]>>,
    #[serde(skip)]
    pub of_building: BTreeMap<EntityId, NodeId>,
    /// Corp -> its Ledger; the Hall -> the Treasury's.
    #[serde(skip)]
    pub ledger_of: BTreeMap<EntityId, NodeId>,
    /// By `DistrictId`.
    #[serde(skip)]
    pub public_of: Vec<NodeId>,
    /// Bumped by every relink and every ICE, alarm or hack write (plan V6);
    /// the route cache's stamp.
    #[serde(skip)]
    pub epoch: u64,
    /// Plan V6's route trees (deviation: `Arc`, not `Rc`, so `World` stays
    /// `Send`; behind a lock so a `&World` scorer, the Hack goal's think,
    /// can fill it).
    #[serde(skip)]
    pub cache: RouteCache,
}

/// Plan V6: the route trees of the current stamp, keyed by `RouteKey`. A
/// pure function of the saved state (never saved, cloned empty); `searches`
/// counts the Dijkstra runs (the one-search-per-scorer test reads it).
///
/// The stamp is the plane's `epoch` plus a validity bound: the next hour
/// boundary or the earliest alarm or hack expiry after the build, whichever
/// comes first (M14 review). An alarm or a DoorOpen lapses mid-hour with no
/// write (`def` reads `until > tick`), so a tree built while one stood must
/// not outlive it: a save taken after the lapse loads with an empty cache
/// and rebuilds without it.
#[derive(Debug, Default)]
pub struct RouteCache {
    inner: Mutex<CacheInner>,
}

#[derive(Debug, Default)]
struct CacheInner {
    epoch: u64,
    /// The trees are valid while `tick < until`.
    until: Tick,
    trees: BTreeMap<RouteKey, Arc<RouteTree>>,
    searches: u64,
}

impl Clone for RouteCache {
    fn clone(&self) -> Self {
        RouteCache::default()
    }
}

impl RouteCache {
    /// Drop every tree (a relink, any ICE, alarm or hack write).
    pub fn clear(&mut self) {
        if let Ok(c) = self.inner.get_mut() {
            c.trees.clear();
        }
    }

    /// The tree for `key` at tick `now` and plane epoch `epoch`, built by
    /// `build` on a miss. Another epoch or a passed validity bound drops the
    /// old trees first, and `until` (called only then) gives the new bound.
    pub fn get_or_build(
        &self,
        now: Tick,
        epoch: u64,
        until: impl FnOnce() -> Tick,
        key: RouteKey,
        build: impl FnOnce() -> RouteTree,
    ) -> Arc<RouteTree> {
        let Ok(mut c) = self.inner.lock() else { return Arc::new(build()) };
        if c.epoch != epoch || now >= c.until {
            c.epoch = epoch;
            c.until = until();
            c.trees.clear();
        }
        if let Some(t) = c.trees.get(&key) {
            return Arc::clone(t);
        }
        let t = Arc::new(build());
        c.searches += 1;
        c.trees.insert(key, Arc::clone(&t));
        t
    }

    /// Dijkstra runs since the world was built or loaded.
    pub fn searches(&self) -> u64 {
        self.inner.lock().map_or(0, |c| c.searches)
    }
}

impl VirtPlane {
    pub fn node(&self, n: NodeId) -> Option<&Node> {
        self.nodes.get(n.index())
    }

    pub fn node_mut(&mut self, n: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(n.index())
    }

    /// Alive nodes.
    pub fn alive_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.alive).count()
    }
}

/// A store of Data per track (spec § 4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataStore {
    pub units: [u32; 3],
}

impl DataStore {
    pub fn total(&self) -> u32 {
        self.units.iter().sum()
    }

    pub fn get(&self, t: Track) -> u32 {
        self.units[t.index()]
    }
}

/// A node's ICE (spec § 3, plan V25): on `Building.security`, on
/// `Corp.ledger_ice` and on the Hall for the Treasury.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityProfile {
    /// 0..=3 installed.
    pub ice: u8,
    /// The corp whose Deck tier caps it; `None` = the city's own (uncapped).
    pub ice_maker: Option<EntityId>,
    pub ice_arrears: u8,
}

impl SecurityProfile {
    /// No ICE, no maker, no arrears (the default; not serialized).
    pub fn is_bare(&self) -> bool {
        *self == SecurityProfile::default()
    }
}

/// The three tech tracks (spec § 5).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Track {
    Chrome,
    Deck,
    #[default]
    Industry,
}

impl Track {
    pub const ALL: [Track; 3] = [Track::Chrome, Track::Deck, Track::Industry];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Track::Chrome => "Chrome",
            Track::Deck => "Deck",
            Track::Industry => "Industry",
        }
    }

    /// `Chrome`, `Deck`, `Industry` (any case).
    pub fn parse(s: &str) -> Option<Track> {
        Track::ALL.into_iter().find(|t| t.label().eq_ignore_ascii_case(s))
    }
}

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A corp's tech tree (spec § 5, plan V21). Tier 0 is never a live tier:
/// it marks a field absent from a pre-M14 save (`Tech::unset`), which
/// `World::migrate_legacy` seeds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tech {
    /// 1..=3 per track.
    pub tier: [u8; 3],
    /// Data spent toward the next tier.
    pub progress: [u32; 3],
    /// Consecutive days of unpaid upkeep.
    pub lapse: [u8; 3],
    pub focus: Track,
}

impl Default for Tech {
    fn default() -> Self {
        Tech::seeded([1, 1, 1], Track::Industry)
    }
}

impl Tech {
    pub fn seeded(tiers: [u8; 3], focus: Track) -> Tech {
        Tech { tier: tiers, progress: [0; 3], lapse: [0; 3], focus }
    }

    /// The serde default: a save written before M14 (plan V44).
    pub fn unset() -> Tech {
        Tech::seeded([0, 0, 0], Track::Industry)
    }

    pub fn is_unset(&self) -> bool {
        self.tier == [0, 0, 0]
    }

    pub fn tier_of(&self, t: Track) -> u8 {
        self.tier[t.index()]
    }
}

/// What a run is for (spec § 3; `Overwatch` carries the gang, plan 1.1).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Purpose {
    Data { wipe: bool },
    Ledger,
    Door,
    Robot(EntityId),
    Camera(EntityId),
    Overwatch(EntityId),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HackEffect {
    DoorOpen,
    RobotTurned(EntityId),
    Blind,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RunOutcome {
    Success,
    Bounced,
    Traced,
    Fried,
    Flatlined,
    Captured,
    Dumped,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RunPhase {
    Hop,
    BreakIn,
    Act,
    Extract,
    Out,
}

/// Who wrote a `RunOrder` (plan V11).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RunWhy {
    Freelance,
    GangOrder,
    CorpOrder,
    Prelude,
    Overwatch,
    Stat,
    God,
}

/// Plan V66 (the spec addendum's quiet and loud): how a run is made.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum RunMode {
    /// Fewer units taken, lower trace odds, no alarm.
    #[default]
    Quiet,
    /// More units taken, higher trace odds, the alarm.
    Loud,
}

/// The one way into a run (plan V11; phase 2 reads it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunOrder {
    pub patron: Option<EntityId>,
    pub purpose: Purpose,
    pub target: NodeId,
    pub chair: EntityId,
    pub not_before: Tick,
    pub expires: Tick,
    pub why: RunWhy,
    /// Plan V66.
    #[serde(default)]
    pub mode: RunMode,
}

/// A run in progress (spec § 3 plus the plan's fields; phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub runner: EntityId,
    pub deck: EntityId,
    pub chair: EntityId,
    pub patron: Option<EntityId>,
    pub purpose: Purpose,
    pub target: NodeId,
    pub route: SmallVec<[NodeId; 8]>,
    pub at: u8,
    pub phase: RunPhase,
    pub next_at: Tick,
    pub payload: [u32; 3],
    pub log: SmallVec<[(Tick, NodeId, bool); 8]>,
    #[serde(default)]
    pub portal: NodeId,
    #[serde(default)]
    pub att: f32,
    #[serde(default)]
    pub deck_eff: u8,
    #[serde(default)]
    pub contests: u8,
    #[serde(default)]
    pub outcome: Option<RunOutcome>,
    /// Where the payload came from (Captured, Dumped).
    #[serde(default)]
    pub source: Option<NodeId>,
    /// Plan V66.
    #[serde(default)]
    pub mode: RunMode,
    /// Plan V11: who ordered it (the CSV and the sell-append read it).
    #[serde(default = "freelance")]
    pub why: RunWhy,
}

fn freelance() -> RunWhy {
    RunWhy::Freelance
}

/// A faction's record of a seen agent (plan V28; phase 2 writes it).
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Sighting {
    pub who: EntityId,
    pub tile: TilePos,
    pub tick: Tick,
    pub confidence: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FactionDb {
    pub sightings: VecDeque<Sighting>,
}
