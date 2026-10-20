//! M16b phase 4 (docs/M16_CONTRACTS.md, the attention addendum): each
//! faction's attention.
//!
//! A game abstraction: a faction (a gang, a corp, the law at
//! `EntityId::NONE`) has a daily pool, `clamp(0.4 + 0.4 × competence + 0.2
//! × min(headcount ÷ 20, 1), 0.3, 1)`; each live threat against it (a raid
//! or a mission at its door, a riot at its building or in a district it
//! controls, a loud run on its nodes, a strike, a hostile offer, an open
//! vendetta) holds its share from `[attention]` for its span plus
//! `linger_days`; what is left is its alertness, never under
//! `alert_floor`. `faction::alertness_mult` (and `virt::alertness_mult`
//! through it) reads `alertness ÷ pool`: 1.0 with no threat, so an
//! undistracted faction rolls as before. Recomputed hourly, the pool at
//! midnight; O(factions + threats).
//!
//! `[attention] enabled = false`: no entry is
//! kept and every read is 1.0 (the phase-3 city).

use std::collections::BTreeMap;

use crate::components::{BuildingKind, Controller, Corp, Gang, Job, Law, RiotTarget};
use crate::contract::{Attention, ThreatKind};
use crate::entity::EntityId;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// Is attention on?
pub fn on(world: &World) -> bool {
    world.config.attention.enabled
}

/// The law's key in `World::attention`.
pub const LAW: EntityId = EntityId::NONE;

fn key(faction: Option<EntityId>) -> EntityId {
    faction.unwrap_or(LAW)
}

/// The multiplier every detection and response roll of `faction` reads
/// (`None` the law): `alertness ÷ pool`, 1.0 off, with no entry, or with no
/// threat.
pub fn alertness_mult(world: &World, faction: Option<EntityId>) -> f32 {
    if world.attention.is_empty() || !on(world) {
        return 1.0;
    }
    world.attention.get(&key(faction)).map_or(1.0, ratio)
}

fn ratio(a: &Attention) -> f32 {
    if a.pool > 0.0 {
        (a.alertness / a.pool).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// How distracted `faction` is: `1 − alertness_mult` (0 undistracted).
pub fn distraction(world: &World, faction: Option<EntityId>) -> f32 {
    1.0 - alertness_mult(world, faction)
}

/// The brains' tilt (plan deviation from the addendum's consideration): a
/// distracted faction's retreat order (`is_retreat`: Hunker, Garrison,
/// LieLow) gains `brain_tilt × distraction` and the scores are re-sorted
/// (stable: equal scores keep their order). No-op undistracted.
pub fn tilt<T>(
    world: &World,
    faction: Option<EntityId>,
    scores: &mut Vec<T>,
    is_retreat: impl Fn(&T) -> bool,
    score: impl Fn(&mut T) -> &mut f32,
) {
    let d = distraction(world, faction);
    if d <= 0.0 {
        return;
    }
    let add = world.config.attention.brain_tilt * d;
    let mut keyed: Vec<(f32, T)> = scores
        .drain(..)
        .map(|mut s| {
            if is_retreat(&s) {
                *score(&mut s) += add;
            }
            (*score(&mut s), s)
        })
        .collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
    scores.extend(keyed.into_iter().map(|(_, s)| s));
}

/// The share a threat of `kind` holds.
pub fn share(world: &World, kind: ThreatKind) -> f32 {
    let c = &world.config.attention;
    match kind {
        ThreatKind::Raid => c.raid,
        ThreatKind::Riot => c.riot,
        ThreatKind::LoudRun => c.loud_run,
        ThreatKind::Strike => c.strike,
        ThreatKind::Acquisition => c.acquisition,
        ThreatKind::Vendetta => c.vendetta,
    }
}

/// The addendum's pool.
pub fn pool_of(competence: f32, headcount: usize) -> f32 {
    (0.4 + 0.4 * competence + 0.2 * (headcount as f32 / 20.0).min(1.0)).clamp(0.3, 1.0)
}

/// `pool − Σ shares` (the sum clamped to 1), never under `floor`.
pub fn alertness_of(pool: f32, shares: f32, floor: f32) -> f32 {
    (pool - shares.clamp(0.0, 1.0)).max(floor)
}

fn linger(world: &World) -> Tick {
    Tick::from(world.config.attention.linger_days) * TICKS_PER_DAY
}

/// A threat event (a strike called, an offer made): `faction` holds the
/// share of `kind` from `source` for `days` plus `linger_days`. No-op off.
pub fn note(world: &mut World, faction: Option<EntityId>, kind: ThreatKind, source: EntityId, days: u32) {
    if !on(world) {
        return;
    }
    let until = world.tick + Tick::from(days) * TICKS_PER_DAY + linger(world);
    hold(world, key(faction), kind, source, until);
    recompute(world, key(faction));
}

fn hold(world: &mut World, k: EntityId, kind: ThreatKind, source: EntityId, until: Tick) {
    let Some(a) = world.attention.get_mut(&k) else { return };
    match a.threats.iter_mut().find(|t| t.0 == kind && t.1 == source) {
        Some(t) => t.2 = t.2.max(until),
        None => a.threats.push((kind, source, until)),
    }
}

fn recompute(world: &mut World, k: EntityId) {
    let floor = world.config.attention.alert_floor;
    let now = world.tick;
    let shares: Vec<f32> = world
        .attention
        .get(&k)
        .map(|a| a.threats.iter().filter(|t| t.2 >= now).map(|t| share(world, t.0)).collect())
        .unwrap_or_default();
    if let Some(a) = world.attention.get_mut(&k) {
        a.threats.retain(|t| t.2 >= now);
        a.alertness = alertness_of(a.pool, shares.iter().sum(), floor);
    }
}

/// The faction a building answers for: a Hideout's gang, the Precinct's
/// and the Jail's law, else its corp owner.
pub fn faction_of_building(world: &World, b: EntityId) -> Option<Option<EntityId>> {
    let kind = world.comp::<crate::components::Building>(b).map(|bd| bd.kind)?;
    if kind == BuildingKind::Jail {
        return Some(None);
    }
    if kind == BuildingKind::Hideout {
        if let Some(&g) = world.gang_list().iter().find(|&&g| world.hideout_of(g) == Some(b)) {
            return Some(Some(g));
        }
    }
    world.owner_of(b).filter(|&o| world.has::<Corp>(o) || world.has::<Gang>(o)).map(Some)
}

/// Hourly: the pools at midnight (or the first pass), the live threats
/// held, the alertness recomputed, the day's columns.
pub fn run(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if world.attention.is_empty() || crate::time::tick_of_day(world.tick) == 0 {
        pools(world);
    }
    let until = world.tick + TICKS_PER_HOUR + linger(world);
    for (f, kind, src) in live_threats(world) {
        hold(world, key(f), kind, src, until);
    }
    let keys: Vec<EntityId> = world.attention.keys().copied().collect();
    for k in keys {
        recompute(world, k);
    }
    let row = &mut world.stats.current.gov;
    row.distracted = world.attention.values().filter(|a| a.alertness < a.pool).count() as u32;
    row.alert_law = world.attention.get(&LAW).map_or(1.0, ratio);
    row.alert_min = world.attention.values().map(ratio).fold(1.0, f32::min);
}

/// Every faction's pool: gangs at `comp_ref` (no gang competence) by
/// members, corps by competence and staff, the law by its competence and
/// city guards. Threats carry over; gone factions drop.
fn pools(world: &mut World) {
    let comp_ref = world.config.competence.comp_ref;
    let mut staff: BTreeMap<EntityId, usize> = BTreeMap::new();
    // scan-ok: daily, one pass over the employed.
    for a in world.with::<Job>() {
        let corp = world.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| world.owner_of(e));
        if let Some(c) = corp.filter(|&c| world.has::<Corp>(c)) {
            *staff.entry(c).or_default() += 1;
        }
    }
    let mut pools: Vec<(EntityId, f32)> = Vec::new();
    for &g in world.gang_list() {
        let n = world.comp::<Gang>(g).map_or(0, |x| x.members.len());
        pools.push((g, pool_of(comp_ref, n)));
    }
    for c in world.corps() {
        let comp = world.comp::<Corp>(c).map_or(comp_ref, |x| x.competence);
        pools.push((c, pool_of(comp, staff.get(&c).copied().unwrap_or(0))));
    }
    let law_comp = world
        .building_of_kind(BuildingKind::Jail)
        .and_then(|j| world.comp::<Law>(j))
        .map_or(comp_ref, |l| l.competence);
    let guards = world.guards().iter().filter(|&&g| crate::systems::law::is_city_guard(world, g)).count();
    pools.push((LAW, pool_of(law_comp, guards)));
    let mut next: BTreeMap<EntityId, Attention> = BTreeMap::new();
    for (k, pool) in pools {
        let threats = world.attention.get(&k).map(|a| a.threats.clone()).unwrap_or_default();
        next.insert(k, Attention { pool, alertness: pool, threats });
    }
    world.attention = next;
}

/// The threats live this hour, `(faction, kind, source)`.
pub fn live_threats(world: &World) -> Vec<(Option<EntityId>, ThreatKind, EntityId)> {
    let mut out = Vec::new();
    // A gang's raid from the muster on: the rival gang, or the corp whose
    // building it aims at.
    for &g in world.gang_list() {
        let Some(x) = world.comp::<Gang>(g) else { continue };
        if x.raid_at.is_none() || !x.order.is_raid() {
            continue;
        }
        let target = match x.raid_target {
            Some(b) => faction_of_building(world, b).flatten(),
            None => crate::systems::raid::raid_rival(world, g),
        };
        if let Some(t) = target {
            out.push((Some(t), ThreatKind::Raid, g));
        }
    }
    // A mission at a faction's door.
    for m in world.missions.values() {
        if let Some(f) = m.door_building.and_then(|b| faction_of_building(world, b)) {
            out.push((f, ThreatKind::Raid, m.door_building.unwrap_or(EntityId::NONE)));
        }
    }
    // A riot: its target's faction and the district's controller.
    for r in &world.riots {
        let target = match r.kind {
            RiotTarget::Precinct => Some(None),
            RiotTarget::Corp | RiotTarget::Gang => faction_of_building(world, r.target),
        };
        if let Some(f) = target {
            out.push((f, ThreatKind::Riot, r.target));
        }
        let ctl = world.districts.get(r.district.index()).map_or(Controller::Contested, |d| d.control);
        let f = match ctl {
            Controller::Gang(g) => Some(Some(g)),
            Controller::Corp(c) => Some(Some(c)),
            Controller::City => Some(None),
            Controller::Contested => None,
        };
        if let Some(f) = f {
            out.push((f, ThreatKind::Riot, r.target));
        }
    }
    // A loud run against a faction's node.
    for run in world.runs.values() {
        if run.mode != crate::virt::RunMode::Loud || run.outcome.is_some() {
            continue;
        }
        let owner = crate::systems::virt::owner_of(world, run.target);
        out.push((owner, ThreatKind::LoudRun, run.runner));
    }
    // An open vendetta, both sides.
    for v in &world.vendettas {
        out.push((Some(v.a), ThreatKind::Vendetta, v.b));
        out.push((Some(v.b), ThreatKind::Vendetta, v.a));
    }
    out
}
