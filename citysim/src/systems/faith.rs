//! Jobs & lives phase 6, faith (docs/JOBS_AND_LIVES.md § 8; Dylan's answer
//! 6). Under `[faith] enabled` (itself under `[jobs2] enabled`): off, every
//! entry point is a no-op, no creed row is seeded, nobody follows, nothing
//! is tithed, and the M15 Purist creed and The Unplugged run unchanged.
//!
//! Creeds are data (`[[faith.creeds]]` rows: `Creed::Config(i)`; no creed
//! name in code). Everything here is counters, rolls and coins.
//!
//! - **Churches.** A `church` row seeds a Mission per copy in its zone's
//!   seat district (the Mission's kitchen and purse, `charity::`). Each
//!   midnight its door holds a Preach: a resident of its district attends
//!   on `p_attend` (a follower, or one under `attend_belonging`, or one
//!   grieving within `grief_days`), gaining `attend_gain` belonging; a
//!   non-follower under `convert_belonging` or grieving converts on the
//!   row's `p_convert`.
//! - **Cults.** A `cult` row seeds a memberless gang with the creed at a
//!   chapel per copy (The Unplugged's shape): it recruits as a gang, along
//!   social edges past its `founders` when `recruit_edges`, casts out above
//!   its chrome `tolerance`, preaches through its members (`leisure::preach`,
//!   a success converts the listener), tilts its Contest and Raid orders by
//!   `violence_bias`, and a member who leaves takes an `Apostasy` grudge of
//!   `apostasy_w` from the leader, which `hunt` reads at `hunt_min`.
//! - **Growth.** A follower's Friend, Family, Spouse or Parent edge of
//!   affinity `spread_affinity` or more converts on the row's `p_spread` a
//!   day. A follower (or a cult member) tithes `tithe_frac` of each wage
//!   (`Flow::Tithe`) into its church's purse or its cult's treasury.
//! - **Influence hooks** (for M18's life paths; nothing acts on its own):
//!   [`influence`] reads a creed's followers, members, purse and its
//!   followers on boards and in the guard; `Levers.faith` holds the slots
//!   (`board_lean`, `posture_lean`, `established`) whose readers
//!   ([`board_lean`], [`posture_lean`]) return 0 at the default.
//!
//! Every coin moves between purses. The midnight pass is O(church district
//! residents + followers × degree).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::components::{Building, Gang, Job, Memory, MemoryKind, Needs, RelKind, Sentence};
use crate::config::CreedCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::word::Creed;
use crate::world::World;

const PURPOSE_ATTEND: u64 = 0x4641_0001;
const PURPOSE_CONVERT: u64 = 0x4641_0002;
const PURPOSE_SPREAD: u64 = 0x4641_0003;

/// Faith is live: `[jobs2] enabled` and `[faith] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.faith.enabled
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// Phase 6's faith state on `Jobs2State` (empty, and not saved, off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FaithState {
    /// A follower -> its creed row (a cult member's creed is its gang's).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub followers: BTreeMap<EntityId, u8>,
    /// A church (a Mission) -> its creed row.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub churches: BTreeMap<EntityId, u8>,
    /// Those who left a cult with an `Apostasy` grudge on them.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub apostates: BTreeSet<EntityId>,
    /// The rows' churches and chapels stand (the first midnight).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub seeded: bool,
}

impl FaithState {
    pub fn is_default(&self) -> bool {
        *self == FaithState::default()
    }
}

/// A creed row by index.
pub fn row(world: &World, i: u8) -> Option<&CreedCfg> {
    world.config.faith.creeds.get(usize::from(i))
}

/// A cult's creed row index (its gang's `Creed::Config`).
pub fn cult_creed(world: &World, gang: EntityId) -> Option<u8> {
    match world.comp::<Gang>(gang)?.creed? {
        Creed::Config(i) => Some(i),
        Creed::Purist => None,
    }
}

/// `id`'s creed: its cult's, else the one it follows.
pub fn creed_of(world: &World, id: EntityId) -> Option<u8> {
    if !on(world) {
        return None;
    }
    world.gang_of(id).and_then(|g| cult_creed(world, g)).or_else(|| world.jobs2.faith.followers.get(&id).copied())
}

/// The creed's name (its row's label).
pub fn label(world: &World, i: u8) -> String {
    row(world, i).map_or_else(|| "a creed".to_string(), |r| r.label.clone())
}

/// `id` follows creed `i` (a conversion), counted.
pub fn convert(world: &mut World, id: EntityId, i: u8) {
    if !on(world) || creed_of(world, id).is_some() || row(world, i).is_none() {
        return;
    }
    world.jobs2.faith.followers.insert(id, i);
    ownership::jobs2_row(world).conversions += 1;
    // The pressure pass phase 1 (OPEN-178): a convert misery delivered says so.
    let text = match crate::systems::misery::join_note(world, id) {
        Some(m) => {
            ownership::jobs2_row(world).misery_converts += 1;
            format!("{} took up {} ({m})", world.name_of(id), label(world, i))
        }
        None => format!("{} took up {}", world.name_of(id), label(world, i)),
    };
    world.push_event(EventKind::Preached, &[id], text);
}

/// Grieving within `grief_days` (a `Grief` memory).
fn grieving(world: &World, id: EntityId) -> bool {
    let since = world.tick.saturating_sub(world.config.faith.grief_days * TICKS_PER_DAY);
    world.comp::<Memory>(id).is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Grief && e.tick >= since))
}

fn adult_free(world: &World, id: EntityId) -> bool {
    crate::systems::demography::is_adult(world, id)
        && crate::systems::law::living(world, id)
        && !world.has::<Sentence>(id)
        && !crate::systems::captives::is_captive(world, id)
}

// ---------------------------------------------------------------------------
// The tithe and apostasy
// ---------------------------------------------------------------------------

/// `economy::collect_wage`: a follower's (or cult member's) `tithe_frac` of
/// the wage paid, into its cult's treasury or its church's purse (the
/// creed's church of the lowest id).
pub fn tithe(world: &mut World, agent: EntityId, paid: i64) {
    if !on(world) || paid <= 0 {
        return;
    }
    let Some(i) = creed_of(world, agent) else { return };
    let Some(frac) = row(world, i).map(|r| r.tithe_frac).filter(|&f| f > 0.0) else { return };
    let amount = ((paid as f32 * frac).round() as i64).max(1);
    let moved = match world.gang_of(agent).filter(|&g| cult_creed(world, g) == Some(i)) {
        Some(g) => ownership::pay(world, Some(agent), Some(g), amount, Flow::Tithe),
        None => {
            let church = world.jobs2.faith.churches.iter().find(|(_, &c)| c == i).map(|(&b, _)| b);
            match church {
                Some(b) => ownership::tithe_in(world, agent, b, amount),
                None => 0,
            }
        }
    };
    let _ = moved;
}

/// `gang::leave`: a cult member who leaves of its own accord (alive, not
/// cast out) takes the leader's `Apostasy` grudge at the row's
/// `apostasy_w`; it follows no creed now.
pub fn on_leave(world: &mut World, id: EntityId, gang: EntityId, reason: &str) {
    if !on(world) {
        return;
    }
    world.jobs2.faith.followers.remove(&id);
    let Some(i) = cult_creed(world, gang) else { return };
    let w = row(world, i).map_or(0.0, |r| r.apostasy_w);
    if w <= 0.0 || reason == "expelled" || !crate::systems::law::living(world, id) {
        return;
    }
    let Some(leader) = world.comp::<Gang>(gang).and_then(|g| g.leader).filter(|&l| l != id) else { return };
    if crate::systems::grudges::add(world, leader, id, crate::word::GrudgeCause::Apostasy, w, 0) {
        world.jobs2.faith.apostates.insert(id);
        ownership::jobs2_row(world).apostates += 1;
    }
}

/// The order brain's tilt: a cult row's `violence_bias` on its Contest and
/// Raid rows (re-sorted as `attention::tilt`; nothing off).
pub fn tilt_orders(world: &World, gang: EntityId, scores: &mut Vec<crate::components::OrderScore>) {
    if !on(world) {
        return;
    }
    let Some(b) = cult_creed(world, gang).and_then(|i| row(world, i)).map(|r| r.violence_bias).filter(|&b| b != 0.0)
    else {
        return;
    };
    let mut keyed: Vec<(f32, crate::components::OrderScore)> = scores
        .drain(..)
        .map(|mut s| {
            if matches!(s.order, crate::components::Order::Contest | crate::components::Order::Raid) {
                s.score += b;
            }
            (s.score, s)
        })
        .collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
    scores.extend(keyed.into_iter().map(|(_, s)| s));
}

/// A cult's member preached to `listener` with success
/// (`leisure::preach_done`): the listener takes up the creed.
pub fn on_preached(world: &mut World, gang: EntityId, listener: EntityId) {
    if let Some(i) = cult_creed(world, gang).filter(|_| on(world)) {
        convert(world, listener, i);
    }
}

/// A cult's `desist_mult` (1 for any other gang, and off).
pub fn desist_mult(world: &World, gang: EntityId) -> f32 {
    if !on(world) {
        return 1.0;
    }
    cult_creed(world, gang).and_then(|i| row(world, i)).map_or(1.0, |r| r.desist_mult)
}

/// A cult that tolerates no chrome buys none (`gang::daily`'s Arms).
pub fn no_chrome(world: &World, gang: EntityId) -> bool {
    on(world) && cult_creed(world, gang).and_then(|i| row(world, i)).is_some_and(|r| r.tolerance == 0)
}

/// May `id` preach for its gang (`leisure::preacher`): a cult's member.
pub fn cult_preacher(world: &World, gang: EntityId) -> bool {
    on(world) && cult_creed(world, gang).is_some()
}

// ---------------------------------------------------------------------------
// Influence hooks (M18's life paths drive them; nothing acts on its own)
// ---------------------------------------------------------------------------

/// A creed's reach: followers (cult members included), its cults' members,
/// its churches' purses plus its cults' treasuries, its followers on corp
/// boards and among the city guards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Influence {
    pub followers: u32,
    pub members: u32,
    pub purse: i64,
    pub board_seats: u32,
    pub guards: u32,
}

/// [`Influence`] of creed `i` now.
pub fn influence(world: &World, i: u8) -> Influence {
    let mut out = Influence::default();
    if !on(world) {
        return out;
    }
    let mut people: BTreeSet<EntityId> =
        world.jobs2.faith.followers.iter().filter(|(_, &c)| c == i).map(|(&a, _)| a).collect();
    for g in world.gangs() {
        if cult_creed(world, g) == Some(i) {
            if let Some(x) = world.comp::<Gang>(g) {
                out.members += x.members.len() as u32;
                out.purse += x.treasury;
                people.extend(x.members.iter().copied());
            }
        }
    }
    for (&b, &c) in &world.jobs2.faith.churches {
        if c == i {
            out.purse += world.comp::<Building>(b).and_then(|bd| bd.charity.as_ref()).map_or(0, |ch| ch.purse);
        }
    }
    out.followers = people.len() as u32;
    for c in crate::systems::governance::board_corps(world) {
        if let Some((members, _)) = crate::systems::governance::board_of(world, c) {
            out.board_seats += members.iter().filter(|m| people.contains(m)).count() as u32;
        }
    }
    out.guards = people
        .iter()
        .filter(|&&a| world.comp::<Job>(a).is_some_and(|j| j.role == crate::components::Role::Guard))
        .count() as u32;
    out
}

/// `governance::pref`'s hook: `Levers.faith.board_lean` when `m` and `cand`
/// share a creed (0 at the default lever, and off).
pub fn board_lean(world: &World, m: EntityId, cand: EntityId) -> f32 {
    let lean = world.levers.faith.board_lean;
    if lean == 0.0 || !on(world) {
        return 0.0;
    }
    match (creed_of(world, m), creed_of(world, cand)) {
        (Some(a), Some(b)) if a == b => lean,
        _ => 0.0,
    }
}

/// The law's posture hook: `Levers.faith.posture_lean` × the established
/// creed's share of the guards (0 at the default lever; a reader for M18,
/// wired to nothing yet).
pub fn posture_lean(world: &World) -> f32 {
    let lean = world.levers.faith.posture_lean;
    let Some(i) = world.levers.faith.established.filter(|_| lean != 0.0 && on(world)) else { return 0.0 };
    // Phase-8 review: the role index, not a scan of the city.
    let guards = world
        .workers(crate::components::Role::Guard)
        .iter()
        .filter(|&&a| world.comp::<Job>(a).is_some_and(|j| j.role == crate::components::Role::Guard))
        .count();
    if guards == 0 {
        return 0.0;
    }
    lean * influence(world, i).guards as f32 / guards as f32
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Midnight: each church's Preach (ascending), the spread along edges, the
/// dead dropped.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("faith::daily");
    if !world.jobs2.faith.seeded {
        seed(world);
    }
    crate::prof::timed!("faith::preach", preach(world));
    crate::prof::timed!("faith::spread", spread(world));
    let dead: Vec<EntityId> =
        world.jobs2.faith.followers.keys().copied().filter(|&a| !crate::systems::law::living(world, a)).collect();
    for a in dead {
        world.jobs2.faith.followers.remove(&a);
    }
    let gone: Vec<EntityId> = world.jobs2.faith.apostates.iter().copied().filter(|&a| !world.is_alive(a)).collect();
    for a in gone {
        world.jobs2.faith.apostates.remove(&a);
    }
}

fn preach(world: &mut World) {
    let cfg = world.config.faith.clone();
    let day = world.day();
    let churches: Vec<(EntityId, u8)> = world.jobs2.faith.churches.iter().map(|(&b, &c)| (b, c)).collect();
    for (b, i) in churches {
        if world.comp::<Building>(b).is_none_or(|bd| bd.demolished) {
            continue;
        }
        let p_convert = row(world, i).map_or(0.0, |r| r.p_convert);
        let d = world.district_of_building(b);
        let residents = world.districts.get(d.index()).map(|x| x.residents.clone()).unwrap_or_default();
        for id in residents {
            // The perf pass phase 7: the attendance draw first (pure, keyed
            // by the resident and the day), then the tests that read the
            // agent (all pure reads): the same attendees.
            if hash(world, PURPOSE_ATTEND, key(id), day) >= cfg.p_attend || !adult_free(world, id) {
                continue;
            }
            let mine = creed_of(world, id);
            let belonging = world.comp::<Needs>(id).map_or(1.0, |n| n.belonging);
            let grief = grieving(world, id);
            // The pressure pass phase 1 (OPEN-178): misery draws as loneliness does.
            let miserable = crate::systems::misery::faith_drawn(world, id);
            let drawn = mine == Some(i) || belonging < cfg.attend_belonging || grief || miserable;
            if !drawn {
                continue;
            }
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.belonging = (n.belonging + cfg.attend_gain).min(1.0);
            }
            ownership::jobs2_row(world).preach_attend += 1;
            if mine.is_none()
                && (belonging < cfg.convert_belonging || grief || miserable)
                && hash(world, PURPOSE_CONVERT, key(id), day)
                    < p_convert * crate::systems::misery::faith_mult(world, id)
            {
                convert(world, id, i);
            }
        }
    }
}

fn spread(world: &mut World) {
    let day = world.day();
    let min = world.config.faith.spread_affinity;
    let list: Vec<(EntityId, u8)> = world.jobs2.faith.followers.iter().map(|(&a, &c)| (a, c)).collect();
    for (a, i) in list {
        let p = row(world, i).map_or(0.0, |r| r.p_spread);
        if p <= 0.0 {
            continue;
        }
        // The perf pass phase 7: the spread's own draw first (pure, keyed by
        // the pair and the day; `p_spread` is ~0.001), so the edge is read
        // only for the few that would convert. The same set, sorted, so the
        // same converts in the same order.
        let mut near: Vec<EntityId> = world
            .neighbours(a)
            .filter(|&o| {
                hash(world, PURPOSE_SPREAD, key(o), day ^ key(a)) < p
                    && world.edge(a, o).is_some_and(|e| {
                        e.affinity >= min
                            && matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse | RelKind::Parent)
                    })
            })
            .collect();
        near.sort_unstable();
        for o in near {
            if creed_of(world, o).is_none() && adult_free(world, o) {
                convert(world, o, i);
            }
        }
    }
}

/// The first midnight (`daily`; the seeded city at tick 0 is the phase-5
/// city's): each row per copy: a church (a Mission on the city's deed) or a
/// cult (a memberless gang with an empty treasury: nothing minted) at a
/// chapel in its zone's seat district. No RNG, no coins.
fn seed(world: &mut World) {
    world.jobs2.faith.seeded = true;
    let rows = world.config.faith.creeds.clone();
    let copies = world.seed_copies();
    for (i, r) in rows.iter().enumerate() {
        let Ok(ix) = u8::try_from(i) else { break };
        for copy in 0..copies {
            let seat = crate::systems::districts::Seat::new(r.zone, r.share);
            if r.church {
                let Some(d) = crate::systems::districts::seat_district(world, seat, copy, copies) else { continue };
                if let Some(b) = crate::systems::charity::open_mission_in(world, d.index(), None) {
                    if let Some(bd) = world.comp_mut::<Building>(b) {
                        bd.label = Some(r.label.clone());
                    }
                    world.jobs2.faith.churches.insert(b, ix);
                    let at = world.district_name(world.district_of_building(b)).to_string();
                    world.push_event(
                        EventKind::Founded,
                        &[b],
                        format!("{} opened its doors in {at} (seeded)", r.label),
                    );
                }
            }
            if r.cult {
                let name = if copy == 0 { r.label.clone() } else { format!("{} {}", r.label, copy + 1) };
                crate::systems::creeds::seed_chapel_as(world, seat, name, Creed::Config(ix), 0, &r.label, copy, copies);
            }
        }
    }
}

/// The day's end (`systems::stats`): the faith snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let n = world.config.faith.creeds.len().min(4);
    let mut by = [0u32; 4];
    for (k, slot) in by.iter_mut().enumerate().take(n) {
        *slot = influence(world, k as u8).followers;
    }
    let members: u32 = world
        .gangs()
        .into_iter()
        .filter(|&g| cult_creed(world, g).is_some())
        .filter_map(|g| world.comp::<Gang>(g).map(|x| x.members.len() as u32))
        .sum();
    let purse: i64 = world
        .jobs2
        .faith
        .churches
        .keys()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.charity.as_ref()).map(|c| c.purse))
        .sum();
    let hunted = world.hunts.values().filter(|h| world.jobs2.faith.apostates.contains(&h.target)).count() as u32;
    let followers = world.jobs2.faith.followers.len() as u32 + members;
    let row = ownership::jobs2_row(world);
    row.followers = followers;
    row.followers_c1 = by[0];
    row.followers_c2 = by[1];
    row.followers_c3 = by[2];
    row.followers_c4 = by[3];
    row.cult_members = members;
    row.church_purse = purse;
    row.apostates_hunted = hunted;
}
