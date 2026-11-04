//! The pressure pass phase 5a (docs/PRESSURE_PASS.md § Phase 5): consequence,
//! the non-gang half. What happens next, so a started story is picked up.
//! Under `[consequence]` (absent: none of it):
//!
//! - **Witnesses react.** A civilian who notes a crime (not a petty one) does
//!   one thing that fits him, one weighted draw: reports (today's SawCrime
//!   memory and the walk to the Hall, `w_report x lawfulness`), flees (a
//!   violent crime: the culprit an Enemy, safety gone, so the Flee goal
//!   takes him), gossips (a crime that is a deed: his closest people hear
//!   it at once, `gossip.rs`'s rumour, so the culprit's name moves), or, the
//!   lawless and broke, keeps it to sell: a blackmail. The culprit sees who
//!   saw him on `learn_p` (an edge, which the Hit market's Silence reads).
//! - **Blackmail** is a later demand in person (a `Visit`, `VisitWhy::
//!   Blackmail`): the culprit pays (fear, and the lawful have more to lose)
//!   or refuses; refused, he silences the witness (a beating, an Assault or
//!   a Murder) or the witness goes to the law. A paid demand comes back.
//! - **Hits escalate.** A Hit or Beat a person bought that expires (or
//!   fails) unfilled: the buyer reposts it higher, goes himself (a Hunt, the
//!   cooldown lifted), or lets it go and the grudge cools; his diary shows
//!   the choice (`EventKind::HitEscalated`).
//! - **Jail is a place.** At sentencing the prisoner learns who reported
//!   him (a Betrayed grudge on a civilian reporter). Each day in the cells a
//!   prisoner may take a member cellmate's offer (a join), get a Fixer's
//!   name from a regular (a regular there too), fight a cellmate he
//!   dislikes, or borrow from one (an edge debt); a jailed worker's workday
//!   counts as an absence on phase 1's count (fired at `fire_absences`).
//!   Release writes the changed life: the post lost, the debt, the grudge,
//!   the gang, the name.
//! - **The faith on screen.** Every creed's place (a cult's or the Purists'
//!   chapel, a church) holds a service once a week at its hashed day and
//!   hour; the members (a church: the followers of its district) are called
//!   to it (a `Visit` to the building) and a congregant may bring an
//!   unchurched friend. An attendee hears the preacher, puts coins on the
//!   plate, a guest may convert, a congregant may walk out (a cult's leaver
//!   takes the row's apostasy grudge; a Purist's the preacher's
//!   `svc_heretic_w`): hunted as a heretic past `[hunt] hunt_min`.
//!
//! Statistical mirror: witness reactions happen where crimes are seen (on
//! screen; the off-screen crime rolls have no witnesses, as before); the
//! demands and the services are `Visit`s, which a Statistical visitor is
//! credited at the hour; the hit's choice, the cells and the release are
//! tier-blind passes.

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Crime, Gang, GangMember, Identity, Job, Lod, MemoryEntry, MemoryKind, Needs,
    Personality, Position, RelKind, Sentence,
};
use crate::config::ConsequenceCfg;
use crate::contract::{Broker, Contract, ContractKind, ContractStatus, PostExtra, Posting, Target};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::household::{PendingVisit, VisitWhy};
use crate::systems::ownership::{self, Flow};
use crate::time::Tick;
use crate::word::{Creed, GrudgeCause, HuntPhase, HuntState, HuntWhy};
use crate::world::World;

mod blackmail;
mod cells;
mod sentence;
mod services;
mod witness;

pub use blackmail::*;
pub use cells::*;
pub use sentence::*;
pub use services::*;
pub use witness::*;

const P_REACT: u64 = 0x0c05_0001;
const P_LEARN: u64 = 0x0c05_0002;
const P_HOUR: u64 = 0x0c05_0003;
const P_PAY: u64 = 0x0c05_0004;
const P_SILENCE: u64 = 0x0c05_0005;
const P_HIT: u64 = 0x0c05_0006;
const P_CELL: u64 = 0x0c05_0007;
const P_SVC_DAY: u64 = 0x0c05_0008;
const P_GUEST: u64 = 0x0c05_0009;
const P_CONVERT: u64 = 0x0c05_000a;
const P_WALKOUT: u64 = 0x0c05_000b;
/// How often (ticks) a body arrived at its visit is looked for.
const ARRIVAL_TICKS: u64 = 10;

/// A witness keeping a culprit's crime to sell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blackmail {
    pub witness: EntityId,
    pub culprit: EntityId,
    pub crime: Crime,
    /// The day of the next demand.
    pub next_day: u64,
    /// Demands paid.
    pub demands: u8,
    /// Demands tried and not made (the visit failed), since the last made.
    pub tries: u8,
    /// Coins paid so far.
    pub paid: i64,
}

/// Today's service at a creed's place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Service {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preacher: Option<EntityId>,
    pub creed: String,
    /// A cult's or the Purists' gang (none for a church).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gang: Option<EntityId>,
    /// The creed row (a cult or a church), none for the Purists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u8>,
    pub hour: u8,
    pub called: u16,
    pub came: u16,
}

/// A prisoner's run in the cells, for the release's line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Inside {
    pub since: u64,
    /// The post held going in: (role label, employer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<(String, Option<EntityId>)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporter: Option<EntityId>,
    /// Coins borrowed in the cells: (lender, coins).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owed: Vec<(EntityId, i64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joined: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixer: Option<EntityId>,
    pub fights: u8,
    pub missed: u8,
    /// Round 8 X6 (`OPEN-217`): the Home and the gang going in (the
    /// release says what waits outside).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gang: Option<EntityId>,
}

/// `World.consequence`: saved while it holds anything.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blackmail: Vec<Blackmail>,
    /// (buyer, target, reposts so far).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reposts: Vec<(EntityId, EntityId, u8)>,
    /// Today's services: place -> the service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<(EntityId, Service)>,
    /// Today's guests at a service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guests: Vec<EntityId>,
    /// Prisoners in the cells: prisoner -> the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inside: Vec<(EntityId, Inside)>,
    /// Round 7 W2 (`OPEN-238`): a shakedown's payers asleep when it took
    /// their coins, `(sleeper, extorter, tick, coins)`: found on waking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slept_through: Vec<(EntityId, EntityId, Tick, i64)>,
    /// Round 7 W2: a robbed sleeper's word the law may get, `(victim,
    /// suspect, until)` (`sleeper_reports` counts it when filed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owed_word: Vec<(EntityId, EntityId, Tick)>,
    /// Round 8 X6 (`OPEN-217`): the prisoners a line named today (`note_line`,
    /// read and cleared at midnight: `cell_days_lined`). Not saved.
    #[serde(skip)]
    pub lined: std::collections::BTreeSet<EntityId>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.blackmail.is_empty()
            && self.reposts.is_empty()
            && self.services.is_empty()
            && self.guests.is_empty()
            && self.inside.is_empty()
            && self.slept_through.is_empty()
            && self.owed_word.is_empty()
    }

    fn inside_mut(&mut self, id: EntityId) -> Option<&mut Inside> {
        self.inside.iter_mut().find(|(p, _)| *p == id).map(|(_, x)| x)
    }
}

pub fn cfg(world: &World) -> Option<&ConsequenceCfg> {
    world.config.consequence.as_ref()
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

#[derive(Clone, Copy)]
struct Traits {
    lawful: f32,
    greed: f32,
    pride: f32,
    social: f32,
    courage: f32,
    loyalty: f32,
}

fn traits(world: &World, id: EntityId) -> Traits {
    world.comp::<Personality>(id).map_or(
        Traits { lawful: 0.5, greed: 0.5, pride: 0.5, social: 0.5, courage: 0.5, loyalty: 0.5 },
        |p| Traits {
            lawful: p.lawfulness,
            greed: p.greed,
            pride: p.pride,
            social: p.sociability,
            courage: p.courage,
            loyalty: p.loyalty,
        },
    )
}

fn living(world: &World, id: EntityId) -> bool {
    crate::systems::law::living(world, id)
}

fn person(world: &World, id: EntityId) -> bool {
    world.has::<Identity>(id) && world.has::<Brain>(id) && living(world, id)
}

fn free(world: &World, id: EntityId) -> bool {
    person(world, id) && crate::systems::contracts::free_adult(world, id)
}

/// A gang's name (`World::name_of` reads people and buildings).
fn gang_name(world: &World, g: EntityId) -> String {
    world.comp::<Gang>(g).map_or_else(|| world.name_of(g), |x| x.name.clone())
}

/// (he, him, his) for `id`.
pub(crate) fn pro(world: &World, id: EntityId) -> (&'static str, &'static str, &'static str) {
    match world.comp::<Identity>(id).map(|i| i.sex) {
        Some(crate::components::Sex::Female) => ("she", "her", "her"),
        _ => ("he", "him", "his"),
    }
}

fn place_of(world: &World, id: EntityId) -> (Option<EntityId>, String) {
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    (here, here.map_or_else(|| "on the street".to_string(), |b| format!("at {}", world.name_of(b))))
}

/// Draw one of `w` (weights, all >= 0) on `u` in `0..1`; `None` when all are 0.
fn pick(w: &[f32], u: f32) -> Option<usize> {
    let total: f32 = w.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = u * total;
    for (i, &v) in w.iter().enumerate() {
        if x < v {
            return Some(i);
        }
        x -= v;
    }
    w.iter().rposition(|&v| v > 0.0)
}

/// A visit due today within `from..to`; `false` with the visits off (the
/// caller resolves at once) or the visitor busy.
fn schedule(world: &mut World, who: EntityId, kin: EntityId, why: VisitWhy, from: u8, to: u8) -> bool {
    if !crate::systems::household::on(world) || world.visits.contains_key(&who) {
        return false;
    }
    let span = to.saturating_sub(from).max(1);
    let u = hash(world, P_HOUR, who, world.day() ^ key(kin));
    let hour = from + ((u * f32::from(span)) as u8).min(span - 1);
    world.visits.insert(who, PendingVisit { kin, why, hour });
    true
}

/// The Visit's urgency for the two calls (0 without the section).
pub fn urgency(world: &World, why: VisitWhy) -> f32 {
    cfg(world).map_or(0.0, |c| match why {
        VisitWhy::Blackmail => c.bm_urgency,
        _ => c.svc_urgency,
    })
}

/// Every `ARRIVAL_TICKS` (from `living::run`): a body come to its service's
/// place, or to the culprit's building, is credited at once (the Visit's
/// Rest runs on; a Sleep that cut the Rest lost the credit). The pending
/// visits only (a handful), never a scan of the city.
pub fn arrivals(world: &mut World) {
    if cfg(world).is_none() || !world.tick.is_multiple_of(ARRIVAL_TICKS) || world.visits.is_empty() {
        return;
    }
    let tod = world.tick_of_day();
    let due: Vec<(EntityId, PendingVisit)> = world
        .visits
        .iter()
        .filter(|(_, v)| matches!(v.why, VisitWhy::Service | VisitWhy::Blackmail))
        .filter(|(_, v)| u16::from(v.hour) * crate::time::TICKS_PER_HOUR as u16 <= tod)
        .map(|(&id, &v)| (id, v))
        .collect();
    for (id, v) in due {
        if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
            continue;
        }
        let here = world.comp::<Position>(id).and_then(|p| p.building);
        let there = match v.why {
            VisitWhy::Service => Some(v.kin),
            _ => world.comp::<Position>(v.kin).and_then(|p| p.building),
        };
        if here.is_none() || here != there {
            continue;
        }
        // Round 8 X1 (`OPEN-246`): the asleep make no demand and attend no
        // service; nor is a sleeping culprit asked (V16 `orderly_20962 d26
        // 12:50`: blackmail paid while the blackmailer slept in the same
        // Block). The visit waits for both to be up.
        let sleeping = |x: EntityId| crate::systems::presence::asleep(world, x);
        if sleeping(id) || (v.why == VisitWhy::Blackmail && sleeping(v.kin)) {
            continue;
        }
        if world.has::<Sentence>(id) {
            continue;
        }
        world.visits.remove(&id);
        match v.why {
            VisitWhy::Service => attended(world, id, v.kin, true),
            _ => blackmailed(world, id, v.kin, true),
        }
    }
}

/// Midnight (LivingEarly, after the visits due are dealt): the demands due,
/// today's services called, the cells' day.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    blackmail_daily(world, &c);
    services_daily(world, &c);
    cells_daily(world, &c);
    let reposts = std::mem::take(&mut world.consequence.reposts);
    world.consequence.reposts = reposts.into_iter().filter(|&(b, t, _)| living(world, b) && living(world, t)).collect();
}
