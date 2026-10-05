//! The law's brain (M9). A captain, the most lawful guard, scores three
//! postures daily (with hysteresis) and at once when pending shocks add up:
//! `Patrol` (the v1 routine), `Crackdown` on the most-reported gang (patrol
//! loops through its turf, one guard in three holds the Jail) and
//! `Garrison` (every guard holds the Jail). The player may pin a posture.
//! Mirrors `faction` for the gangs; the `Law` component lives on the Jail.

use std::collections::BTreeMap;

use crate::components::{
    Building, BuildingKind, Crime, DistrictId, Gang, Job, Law, LawShock, Personality, Position, Posture, PostureScore,
    Sentence, Stance, StanceScore,
};
use crate::config::LawCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::TICKS_PER_DAY;
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::world::World;

/// `report_log` keeps at most this many reports.
pub const REPORT_LOG_CAP: usize = 128;
/// M12 D13: `World::report_places` keeps at most this many.
pub const REPORT_PLACES_CAP: usize = 512;

/// Everything `score_postures` reads, gathered once per rescoring.
#[derive(Clone, Debug, PartialEq)]
pub struct PostureInputs {
    /// The gang with the most reports in the window, bribes excluded.
    pub wanted_gang: Option<EntityId>,
    /// Its reports ÷ `crackdown_reports()` (per capita since the M11 review), clamped to 1.
    pub pressure: f32,
    /// Gang members in the Jail ÷ its capacity, clamped to 1.
    pub jailed_gang: f32,
    /// A breakout within `garrison_days`.
    pub breakout_recent: bool,
    /// Guards on the payroll.
    pub guards: usize,
    /// A refused bribe hardens the crackdown.
    pub hardened: bool,
    /// The captain's traits.
    pub courage: f32,
    pub lawfulness: f32,
}

/// Product of the outputs plus a flat term; `None` when a gate is shut.
fn score(posture: Posture, cs: Vec<Consideration>, flat: f32) -> Option<PostureScore> {
    if cs.iter().any(|c| c.output <= 0.0) {
        return None;
    }
    let raw: f32 = cs.iter().map(|c| c.output).product();
    Some(PostureScore { posture, score: raw + flat, considerations: cs })
}

/// Score every posture, best first. Patrol always scores, so the result is never empty.
pub fn score_postures(i: &PostureInputs, cfg: &LawCfg) -> Vec<PostureScore> {
    let f = &cfg.posture_flat;
    let enough = i.guards >= cfg.min_guards;
    // The Jail is threatened by the convicts inside and by a breakout just past.
    let threat = if i.breakout_recent { 1.0 } else { i.jailed_gang };
    let scored = [
        score(
            Posture::Patrol,
            vec![
                Consideration::new("1-pressure", 1.0 - i.pressure, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-jailed gang", 1.0 - i.jailed_gang, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.patrol,
        ),
        score(
            Posture::Crackdown,
            vec![
                Consideration::new("a wanted gang", can(i.wanted_gang.is_some() && enough), GATE),
                Consideration::new("pressure", i.pressure, Curve::Logistic { k: 8.0, mid: 0.5 }),
                Consideration::new("courage", i.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("lawfulness", i.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.crackdown + if i.hardened { 0.3 } else { 0.0 },
        ),
        score(
            Posture::Garrison,
            vec![
                Consideration::new("guards", can(enough), GATE),
                Consideration::new("threat", threat, Curve::Logistic { k: 8.0, mid: 0.3 }),
                Consideration::new("1-courage", 1.0 - i.courage, Curve::Linear { m: 0.4, b: 0.6 }),
            ],
            f.garrison + if i.breakout_recent { 0.5 } else { 0.0 },
        ),
    ];
    let mut out: Vec<PostureScore> = scored.into_iter().flatten().collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.posture.cmp(&b.posture)));
    out
}

/// The posture to switch to, if the best beats the current one by `hysteresis`.
pub fn choose(scores: &[PostureScore], current: Posture, hysteresis: f32) -> Option<Posture> {
    let best = scores.first()?;
    let current_score = scores.iter().find(|s| s.posture == current).map_or(0.0, |s| s.score);
    (best.posture != current && best.score > current_score + hysteresis).then_some(best.posture)
}

/// Guards on the city payroll: employed at the Precinct (M11 D18). Private
/// guards (a Security Office's) are guards for sightings, witnesses and
/// fear (`World::guards`), but never the captain, the Jail roster, a bribe's
/// price or a breach's defenders.
pub fn guards(world: &World) -> Vec<EntityId> {
    world.guards().iter().copied().filter(|&g| is_city_guard(world, g)).collect()
}

/// A guard on the city payroll (`law::is_city_guard`, re-exported here
/// where the roster lives).
pub use crate::systems::law::is_city_guard;

/// The captain: the most lawful living guard, ties by lower index. Stored
/// on the `Law` and returned.
pub fn recompute_captain(world: &mut World) -> Option<EntityId> {
    let captain = guards(world)
        .into_iter()
        .filter_map(|g| world.comp::<Personality>(g).map(|p| (p.lawfulness, g)))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, g)| g);
    if let Some(l) = world.law_mut() {
        l.captain = captain;
    }
    captain
}

/// Reports against each gang's members within `window_days`.
pub fn reports_by_gang(world: &World) -> BTreeMap<EntityId, usize> {
    let window = world.config.law.window_days * TICKS_PER_DAY;
    let now = world.tick;
    let mut out = BTreeMap::new();
    if let Some(l) = world.law() {
        for &(_, g) in l.report_log.iter().filter(|&&(t, _)| now.saturating_sub(t) < window) {
            *out.entry(g).or_insert(0) += 1;
        }
    }
    out
}

/// The gang with the most recent reports, gangs whose bribe was taken
/// excluded (ties: the lower id), with its count. The standing Crackdown
/// target is kept unless a challenger leads it by `target_margin` reports.
pub fn wanted_gang(world: &World) -> Option<(EntityId, usize)> {
    let now = world.tick;
    let counts: BTreeMap<EntityId, usize> = reports_by_gang(world)
        .into_iter()
        .filter(|&(g, n)| {
            n > 0 && world.comp::<crate::components::Gang>(g).is_some_and(|gg| gg.paid_until.is_none_or(|t| t <= now))
        })
        .collect();
    let best = counts.iter().map(|(&g, &n)| (g, n)).max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))?;
    let margin = world.config.law.target_margin;
    if let Some(cur) = world.law().and_then(|l| l.target) {
        if let Some(&n) = counts.get(&cur) {
            if best.1 < n + margin {
                return Some((cur, n));
            }
        }
    }
    Some(best)
}

/// Reports in the window that read as full pressure: `[law]
/// crackdown_reports_per_1000` per 1,000 living residents (at least 1), or
/// the absolute `crackdown_reports` when that key is 0.
pub fn crackdown_reports(world: &World) -> u32 {
    let cfg = &world.config.law;
    if cfg.crackdown_reports_per_1000 <= 0.0 {
        return cfg.crackdown_reports.max(1);
    }
    let residents = crate::systems::founding::living(world) as f32;
    ((cfg.crackdown_reports_per_1000 * residents / 1000.0).round() as u32).max(1)
}

/// M12 D13 (plan deviation): full pressure in one district: `[law]
/// crackdown_reports_per_1000` per 1,000 of the district's residents (at
/// least 1), or the absolute `crackdown_reports` when that key is 0. The
/// city-wide bar (~36 at 2,000) read on one district's reports never fired:
/// no district Crackdown in 30 days on seed 42.
pub fn crackdown_reports_in(world: &World, d: DistrictId) -> u32 {
    let cfg = &world.config.law;
    if cfg.crackdown_reports_per_1000 <= 0.0 {
        return cfg.crackdown_reports.max(1);
    }
    let residents = world.districts.get(d.index()).map_or(0, |x| x.population) as f32;
    ((cfg.crackdown_reports_per_1000 * residents / 1000.0).round() as u32).max(1)
}

/// Gather the inputs, or `None` when there is no captain to decide.
pub fn gather_inputs(world: &World) -> Option<PostureInputs> {
    let l = world.law()?;
    let captain = l.captain.filter(|&c| crate::systems::law::is_guard(world, c))?;
    let p = world.comp::<Personality>(captain)?;
    let cfg = &world.config.law;
    let now = world.tick;
    let (wanted, reports) = wanted_gang(world).map_or((None, 0), |(g, n)| (Some(g), n));
    let capacity = usize::from(world.config.buildings.jail.capacity).max(1);
    // M12 D15: a Vagrancy night is not a convict worth guarding.
    let jailed_gang = world
        .with::<Sentence>()
        .into_iter()
        .filter(|&s| world.has::<crate::components::GangMember>(s))
        .filter(|&s| world.comp::<Sentence>(s).is_some_and(|x| x.crime != Crime::Vagrancy))
        .count();
    Some(PostureInputs {
        wanted_gang: wanted,
        pressure: (reports as f32 / crackdown_reports(world) as f32).clamp(0.0, 1.0),
        jailed_gang: (jailed_gang as f32 / capacity as f32).clamp(0.0, 1.0),
        breakout_recent: l
            .last_breakout_tick
            .is_some_and(|t| now.saturating_sub(t) < cfg.garrison_days * TICKS_PER_DAY),
        guards: guards(world).len(),
        hardened: l.hardened_until.is_some_and(|t| t > now),
        courage: p.courage,
        lawfulness: p.lawfulness,
    })
}

/// Score the postures and switch when the best clears `hysteresis` (or at
/// once to the player's pin). A Crackdown's target is refreshed every
/// rescoring, since the most-reported gang can change. Logs `Posture`.
pub fn rescore(world: &mut World, hysteresis: f32, why: &str) {
    recompute_captain(world);
    let now = world.tick;
    let cfg = world.config.law.clone();
    let Some((current, pinned, old_target)) = world.law().map(|l| (l.posture, l.pinned, l.target)) else { return };
    let inputs = gather_inputs(world);
    let scores = inputs.as_ref().map(|i| score_postures(i, &cfg)).unwrap_or_default();
    let brain = choose(&scores, current, hysteresis);
    // M11 D21: a corp's bought crackdown, while it holds and its gang lives,
    // forces a Crackdown on that gang (the player's pin still wins).
    let lobby = world.law().and_then(|l| l.lobby);
    let live_lobby = lobby.filter(|h| h.until > now && world.has::<crate::components::Gang>(h.gang));
    if lobby.is_some() && live_lobby.is_none() {
        if let Some(l) = world.law_mut() {
            l.lobby = None;
        }
    }
    let forced = live_lobby.filter(|_| pinned.is_none());
    let lobby_why;
    let why = match forced {
        Some(h) => {
            let corp =
                world.comp::<crate::components::Corp>(h.corp).map_or_else(|| "a corp".to_string(), |c| c.name.clone());
            lobby_why = format!("lobbied by {corp}");
            lobby_why.as_str()
        }
        None => why,
    };
    // No captain: nothing is scored and the posture is the pin, or Patrol.
    let next = match (pinned, forced, inputs.is_some()) {
        (Some(p), _, _) => (p != current).then_some(p),
        (None, Some(_), _) => (current != Posture::Crackdown).then_some(Posture::Crackdown),
        (None, None, true) => brain,
        (None, None, false) => (current != Posture::Patrol).then_some(Posture::Patrol),
    };
    let posture = next.unwrap_or(current);
    let target = match (posture, forced) {
        (Posture::Crackdown, Some(h)) => Some(h.gang),
        (Posture::Crackdown, None) => inputs.as_ref().and_then(|i| i.wanted_gang),
        _ => None,
    };
    let best_score = scores.first().map_or(0.0, |s| s.score);
    let current_score = scores.iter().find(|s| s.posture == current).map_or(0.0, |s| s.score);
    if let Some(l) = world.law_mut() {
        l.posture_trace = scores;
        l.target = target;
        if let Some(p) = next {
            l.posture = p;
            l.posture_since = now;
        }
    }
    let on = |world: &World, g: Option<EntityId>| {
        g.and_then(|g| world.comp::<crate::components::Gang>(g)).map_or(String::new(), |g| format!(" on {}", g.name))
    };
    let jail = world.building_of_kind(BuildingKind::Jail);
    let mut actors: Vec<EntityId> = jail.into_iter().collect();
    actors.extend(target);
    if let Some(p) = next {
        let text = format!("Law: {current} -> {p}{} ({why}, {best_score:.2} vs {current_score:.2})", on(world, target));
        world.push_event(EventKind::Posture, &actors, text);
    } else if posture == Posture::Crackdown && target != old_target {
        let text = format!("Law: the crackdown turns{}", on(world, target));
        world.push_event(EventKind::Posture, &actors, text);
    }
}

/// An immediate rescoring with no hysteresis; the pending shocks are consumed.
/// M12 D10/D12: the districts are re-dealt and re-scored too.
pub fn rethink(world: &mut World) {
    rescore(world, 0.0, "shock");
    if world.config.law.district_beats {
        allocate(world);
        rescore_stances(world, 0.0, "shock");
    }
    if let Some(l) = world.law_mut() {
        l.shocks.clear();
    }
}

pub fn push_shock(world: &mut World, shock: LawShock) {
    if let Some(l) = world.law_mut() {
        l.shocks.push(shock);
    }
}

/// A report was filed against a gang member: the law remembers which gang,
/// and (M12 D13) in which district the suspect stood.
pub fn log_report(world: &mut World, suspect: EntityId) {
    let Some(gang) = world.gang_of(suspect) else { return };
    let tick = world.tick;
    if let Some(l) = world.law_mut() {
        if l.report_log.len() >= REPORT_LOG_CAP {
            l.report_log.pop_front();
        }
        l.report_log.push_back((tick, gang));
    }
    let tile = world.comp::<Position>(suspect).map(|p| p.tile).unwrap_or_default();
    let d = world.district_of(tile);
    if world.report_places.len() >= REPORT_PLACES_CAP {
        world.report_places.pop_front();
    }
    world.report_places.push_back((tick, gang, d));
}

/// Daily at midnight, and at once when pending shocks reach
/// `shock_severity_rethink`. Called from `law::run` before the daily sweep.
pub fn run(world: &mut World) {
    let Some(l) = world.law() else { return };
    let pending: f32 = l.shocks.iter().map(|s| s.severity()).sum();
    if world.tick_of_day() == 0 {
        let h = world.config.law.hysteresis;
        rescore(world, h, "daily");
        // M12 D6/D10/D12: after the posture, deal the guards, then the stances.
        if world.config.law.district_beats {
            allocate(world);
            rescore_stances(world, h, "daily");
        }
        if let Some(l) = world.law_mut() {
            l.shocks.clear();
        }
    } else if pending >= world.config.law.shock_severity_rethink {
        rethink(world);
    }
}

// ---------------------------------------------------------------------------
// M12 phase 2: the law in districts (docs/M12_DISTRICTS.md § 2, plan D9-D13)
// ---------------------------------------------------------------------------

/// A lever moved (guard weight, stance pin): re-deal and re-score now.
pub fn redeal(world: &mut World, why: &str) {
    if !world.config.law.district_beats || world.law().is_none() {
        return;
    }
    allocate(world);
    let h = world.config.law.hysteresis;
    rescore_stances(world, h, why);
}

/// D10: the city guards on patrol: on the payroll, free, and not on Jail
/// duty for their next shift; ascending id.
pub fn patrol_guards(world: &World) -> Vec<EntityId> {
    let now = world.tick;
    let mut out: Vec<EntityId> = guards(world)
        .into_iter()
        .filter(|&g| !world.has::<Sentence>(g))
        .filter(|&g| {
            world.comp::<Job>(g).is_some_and(|j| !crate::systems::law::jail_duty(world, g, j.next_shift_key(now)))
        })
        .collect();
    out.sort_unstable();
    out
}

/// Mean crime rate over the inhabited districts (0 with none).
pub fn mean_crime_rate(world: &World) -> f32 {
    let rates: Vec<f32> = world.districts.iter().filter(|d| d.inhabited()).map(|d| d.crime_rate).collect();
    if rates.is_empty() {
        0.0
    } else {
        rates.iter().sum::<f32>() / rates.len() as f32
    }
}

/// D10 `paid_d`: each district's share of `Σ (7-day revenue × tax_rate + 7 ×
/// upkeep)` over buildings with a non-city owner, plus 0.5 (capped at 1)
/// wherever a live Lobby hold's corp owns a building.
pub fn paid_shares(world: &World) -> Vec<f32> {
    let n = world.districts.len();
    let mut paid = vec![0.0f32; n];
    let rate = world.levers.tax_rate;
    let upkeep = &world.config.corps.upkeep;
    for (i, d) in world.districts.iter().enumerate() {
        for &b in &d.buildings {
            let Some(bd) = world.comp::<Building>(b).filter(|bd| !bd.demolished && bd.owner.is_some()) else {
                continue;
            };
            let revenue: i64 = bd.revenue.iter().rev().take(7).sum();
            paid[i] += revenue.max(0) as f32 * rate + 7.0 * upkeep.for_building(bd.kind, bd.tier) as f32;
        }
    }
    let total: f32 = paid.iter().sum();
    let mut out: Vec<f32> = paid.iter().map(|&p| if total > 0.0 { p / total } else { 0.0 }).collect();
    let now = world.tick;
    if let Some(h) = world.law().and_then(|l| l.lobby).filter(|h| h.until > now && world.has::<Gang>(h.gang)) {
        for (i, d) in world.districts.iter().enumerate() {
            let owns =
                d.buildings.iter().any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(h.corp)));
            if owns {
                out[i] = (out[i] + 0.5).min(1.0);
            }
        }
    }
    out
}

/// D10 `landlord_d`: the district's controlling gang when it holds at least
/// `gang_landlord_homes` Homes there (a gang whose bribe was taken excluded).
pub fn gang_landlord(world: &World, d: DistrictId) -> Option<EntityId> {
    let dist = world.districts.get(d.index())?;
    let crate::components::Controller::Gang(g) = dist.control else { return None };
    let gang = world.comp::<Gang>(g)?;
    if gang.paid_until.is_some_and(|t| t > world.tick) {
        return None;
    }
    let held = gang.territory.iter().filter(|&&h| world.district_of_building(h) == d).count();
    (held >= world.config.law.gang_landlord_homes).then_some(g)
}

/// D10: every district's allocation weight and its terms, by district index.
pub fn alloc_weights(world: &World) -> Vec<(f32, Vec<(&'static str, f32)>)> {
    let cfg = &world.config.law;
    let mean = mean_crime_rate(world);
    let paid = paid_shares(world);
    let garrison = crate::systems::law::garrisoned(world);
    world
        .districts
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let inhabited = d.inhabited();
            let base = if inhabited { cfg.alloc_base } else { 0.0 };
            let crime = if mean > 0.0 && inhabited { cfg.alloc_crime * (d.crime_rate / mean).min(3.0) } else { 0.0 };
            let paid_t = cfg.alloc_paid * paid.get(i).copied().unwrap_or(0.0);
            let landlord = if gang_landlord(world, d.id).is_some() { cfg.alloc_gang_landlord } else { 0.0 };
            // Riots arrive in phase 4.
            let riot = 0.0;
            let lever = world.levers.guard_weight.get(i).copied().unwrap_or(1.0).max(0.0);
            let off = garrison || d.stance == Stance::Withdrawn;
            let w = if off { 0.0 } else { (base + crime + paid_t + landlord + riot) * lever };
            let terms = vec![
                ("base", base),
                ("crime", crime),
                ("paid", paid_t),
                ("gang landlord", landlord),
                ("riot", riot),
                ("lever", lever),
                ("off", if off { 1.0 } else { 0.0 }),
                ("weight", w),
            ];
            (w, terms)
        })
        .collect()
}

/// D10: one district's weight and terms (the panel).
pub fn alloc_weight(world: &World, d: DistrictId) -> (f32, Vec<(&'static str, f32)>) {
    alloc_weights(world).into_iter().nth(d.index()).unwrap_or_default()
}

/// D10: deal the patrol guards to districts by weight (largest remainder),
/// in id order: the first `guards[0]` to district 0, and so on. Writes
/// `District.guards`, `District.alloc_trace` and `Law.beats`. A guard's
/// beat takes effect at its next route (`plan.rs`), never mid-shift.
pub fn allocate(world: &mut World) {
    if world.law().is_none() || world.districts.is_empty() {
        return;
    }
    let weights = alloc_weights(world);
    let patrol = patrol_guards(world);
    let w: Vec<f32> = weights.iter().map(|(x, _)| *x).collect();
    let counts = crate::util::largest_remainder(patrol.len() as u32, &w);
    let mut beats = BTreeMap::new();
    let mut it = patrol.into_iter();
    for (i, &c) in counts.iter().enumerate() {
        for g in it.by_ref().take(usize::from(c)) {
            beats.insert(g, DistrictId(i as u8));
        }
    }
    for (i, (d, (_, terms))) in world.districts.iter_mut().zip(weights).enumerate() {
        d.guards = counts.get(i).copied().unwrap_or(0);
        d.alloc_trace = terms;
    }
    if let Some(l) = world.law_mut() {
        l.beats = beats;
    }
}

/// D13: the district's most-reported gang in `window_days` (gangs whose
/// bribe was taken excluded; ties the lower id) and its count.
pub fn top_gang(world: &World, d: DistrictId) -> Option<(EntityId, usize)> {
    let window = world.config.law.window_days * TICKS_PER_DAY;
    let now = world.tick;
    let mut counts: BTreeMap<EntityId, usize> = BTreeMap::new();
    for &(t, g, place) in &world.report_places {
        if place == d && now.saturating_sub(t) < window {
            *counts.entry(g).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|&(g, _)| world.comp::<Gang>(g).is_some_and(|gg| gg.paid_until.is_none_or(|t| t <= now)))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
}

/// Everything `score_stances` reads for one district (Law tables).
#[derive(Clone, Debug, PartialEq)]
pub struct StanceInputs {
    pub district: DistrictId,
    /// The most-reported gang there, else the gang landlord.
    pub top_gang: Option<EntityId>,
    /// The top gang's reports ÷ `crackdown_reports_in(d)`, clamped to 1.
    pub pressure: f32,
    /// Crime rate ÷ the inhabited mean, clamped to 2, halved (traced only).
    pub crime: f32,
    /// Rough sleepers last night ÷ `sweep_full`, clamped to 1.
    pub vagrants: f32,
    /// A riot musters or runs here (phase 4).
    pub riot: bool,
    /// The top gang is the district's gang landlord.
    pub landlord: bool,
    pub coverage: f32,
    /// Guards allocated today.
    pub guards: u8,
    /// A Crackdown slot is free (`max_crackdowns`).
    pub slot_free: bool,
    pub courage: f32,
    pub lawfulness: f32,
    /// Corp-class share of the resident adults.
    pub corp_share: f32,
}

/// Gather a district's stance inputs, or `None` without a captain.
pub fn gather_stance_inputs(world: &World, d: DistrictId) -> Option<StanceInputs> {
    let l = world.law()?;
    let captain = l.captain.filter(|&c| crate::systems::law::is_guard(world, c))?;
    let p = world.comp::<Personality>(captain)?;
    let dist = world.districts.get(d.index())?;
    let cfg = &world.config.law;
    let reported = top_gang(world, d);
    let landlord = gang_landlord(world, d);
    let top = reported.map(|(g, _)| g).or(landlord);
    let pressure = reported.map_or(0.0, |(_, n)| (n as f32 / crackdown_reports_in(world, d) as f32).clamp(0.0, 1.0));
    let mean = mean_crime_rate(world);
    let crime = if mean > 0.0 { (dist.crime_rate / mean).min(2.0) / 2.0 } else { 0.0 };
    Some(StanceInputs {
        district: d,
        top_gang: top,
        pressure,
        crime,
        vagrants: (f32::from(dist.rough) / cfg.sweep_full.max(1) as f32).clamp(0.0, 1.0),
        riot: false,
        landlord: landlord.is_some() && landlord == top,
        coverage: dist.coverage,
        guards: dist.guards,
        slot_free: true,
        courage: p.courage,
        lawfulness: p.lawfulness,
        corp_share: if dist.adults == 0 { 0.0 } else { dist.classes[0] as f32 / dist.adults as f32 },
    })
}

fn stance_rank(s: Stance) -> u8 {
    match s {
        Stance::Patrol => 0,
        Stance::Crackdown(_) => 1,
        Stance::Sweep => 2,
        Stance::Cordon => 3,
        Stance::Withdrawn => 4,
    }
}

fn score_stance(stance: Stance, cs: Vec<Consideration>, flat: f32) -> Option<StanceScore> {
    if cs.iter().any(|c| c.output <= 0.0) {
        return None;
    }
    let raw: f32 = cs.iter().map(|c| c.output).product();
    Some(StanceScore { stance, score: raw + flat, considerations: cs })
}

/// D12: score a district's stances, best first (Law tables). Patrol always
/// scores. Patrol reads `1 − max(pressure, landlord, vagrants)` (plan
/// deviation: with the table's `1 − pressure` alone Patrol scored 1.2 on a
/// quiet district and no Sweep or landlord Crackdown, each capped at 1.0,
/// could ever beat it).
pub fn score_stances(i: &StanceInputs, cfg: &LawCfg) -> Vec<StanceScore> {
    let f = &cfg.stance_flat;
    let landlord = if i.landlord { 1.0 } else { 0.0 };
    let press = i.pressure.max(landlord).max(i.vagrants);
    let crackdown = i.top_gang.map(|g| {
        score_stance(
            Stance::Crackdown(g),
            vec![
                Consideration::new(
                    "top gang, guards, slot",
                    can(i.guards as usize >= cfg.min_guards && i.slot_free),
                    GATE,
                ),
                Consideration::new(
                    "pressure or landlord",
                    i.pressure.max(landlord),
                    Curve::Logistic { k: 8.0, mid: 0.5 },
                ),
                Consideration::new("courage", i.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("lawfulness", i.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.crackdown,
        )
    });
    let scored = [
        score_stance(
            Stance::Patrol,
            vec![Consideration::new("1-pressure", 1.0 - press, Curve::Linear { m: 0.6, b: 0.4 })],
            f.patrol,
        ),
        crackdown.flatten(),
        score_stance(
            Stance::Sweep,
            vec![
                Consideration::new("vagrants", i.vagrants, Curve::Logistic { k: 8.0, mid: 0.5 }),
                Consideration::new("lawfulness", i.lawfulness, Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 }),
                Consideration::new("1-corp share", 1.0 - i.corp_share, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.sweep,
        ),
        score_stance(Stance::Cordon, vec![Consideration::new("riot", can(i.riot), GATE)], 1.0),
    ];
    let mut out: Vec<StanceScore> = scored.into_iter().flatten().collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(stance_rank(a.stance).cmp(&stance_rank(b.stance))));
    out
}

/// The stance to switch to, if the best beats the current one by `hysteresis`.
pub fn choose_stance(scores: &[StanceScore], current: Stance, hysteresis: f32) -> Option<Stance> {
    let best = scores.first()?;
    let current_score = scores.iter().find(|s| s.stance == current).map_or(0.0, |s| s.score);
    (best.stance != current && best.score > current_score + hysteresis).then_some(best.stance)
}

/// A stance's name for the log and the panel.
pub fn stance_label(world: &World, s: Stance) -> String {
    match s {
        Stance::Patrol => "Patrol".to_string(),
        Stance::Crackdown(g) => {
            format!("Crackdown on {}", world.comp::<Gang>(g).map_or_else(|| world.name_of(g), |x| x.name.clone()))
        }
        Stance::Sweep => "Sweep".to_string(),
        Stance::Cordon => "Cordon".to_string(),
        Stance::Withdrawn => "Withdrawn".to_string(),
    }
}

/// D12: the captain's per-district brain, after allocation. Forced first:
/// a pin, a zero guard weight (Withdrawn), Garrison (every stance Patrol),
/// an uninhabited district (Patrol), a live Lobby hold (Crackdown on its
/// gang where that gang holds the most Homes). The rest are scored; the
/// Crackdown slots left go to the holders that still score it, then to the
/// best Crackdown scores among the districts that would take it, and every
/// other district has the Crackdown gate shut. Logs `Stance` per change.
pub fn rescore_stances(world: &mut World, hysteresis: f32, why: &str) {
    if world.law().is_none() || world.districts.is_empty() {
        return;
    }
    let cfg = world.config.law.clone();
    let now = world.tick;
    let n = world.districts.len();
    let garrison = crate::systems::law::garrisoned(world);
    let lobby = world.law().and_then(|l| l.lobby).filter(|h| h.until > now && world.has::<Gang>(h.gang));
    let lobby_district = lobby.and_then(|h| {
        let territory = world.comp::<Gang>(h.gang).map(|g| g.territory.clone()).unwrap_or_default();
        let mut held = vec![0usize; n];
        for t in territory {
            if let Some(x) = held.get_mut(world.district_of_building(t).index()) {
                *x += 1;
            }
        }
        // Most Homes, ties the lower id; a gang holding none: its Hideout's district.
        let best = (0..n).max_by(|&a, &b| held[a].cmp(&held[b]).then(b.cmp(&a)))?;
        if held[best] > 0 {
            Some(DistrictId(best as u8))
        } else {
            world.hideout_of(h.gang).map(|hq| world.district_of_building(hq))
        }
    });
    let lobby_why = lobby.map(|h| {
        let corp =
            world.comp::<crate::components::Corp>(h.corp).map_or_else(|| "a corp".to_string(), |c| c.name.clone());
        format!("lobbied by {corp}")
    });

    // 1. Forced stances.
    let mut forced: Vec<Option<(Stance, String)>> = vec![None; n];
    for (i, f) in forced.iter_mut().enumerate() {
        let d = &world.districts[i];
        *f = if let Some(p) = world.levers.stance_pin.get(i).copied().flatten() {
            Some((p, "pinned".to_string()))
        } else if world.levers.guard_weight.get(i).is_some_and(|&w| w <= 0.0) {
            Some((Stance::Withdrawn, "withdrawn".to_string()))
        } else if garrison {
            Some((Stance::Patrol, "garrison".to_string()))
        } else if let (Some(h), Some(ld)) = (lobby, lobby_district.filter(|ld| ld.index() == i)) {
            let _ = ld;
            Some((Stance::Crackdown(h.gang), lobby_why.clone().unwrap_or_default()))
        } else if !d.inhabited() {
            Some((Stance::Patrol, "uninhabited".to_string()))
        } else {
            None
        };
    }

    // 2. Score the rest with the slot open.
    let inputs: Vec<Option<StanceInputs>> = (0..n)
        .map(|i| if forced[i].is_none() { gather_stance_inputs(world, DistrictId(i as u8)) } else { None })
        .collect();
    let mut scores: Vec<Vec<StanceScore>> =
        inputs.iter().map(|x| x.as_ref().map(|i| score_stances(i, &cfg)).unwrap_or_default()).collect();

    // 3. Crackdown slots.
    let used = forced.iter().filter(|f| matches!(f, Some((Stance::Crackdown(_), _)))).count();
    let mut slots = cfg.max_crackdowns.saturating_sub(used);
    let cd_score = |sc: &[StanceScore]| sc.iter().find(|s| matches!(s.stance, Stance::Crackdown(_))).map(|s| s.score);
    let mut wants: Vec<(bool, f32, usize)> = Vec::new();
    for (i, sc) in scores.iter().enumerate() {
        let Some(cd) = cd_score(sc) else { continue };
        let current = world.districts[i].stance;
        let holder = matches!(current, Stance::Crackdown(_));
        let would = choose_stance(sc, current, hysteresis).is_some_and(|s| matches!(s, Stance::Crackdown(_)));
        if holder || would {
            wants.push((holder, cd, i));
        }
    }
    // Holders first, then the best Crackdown score, ties the lower id.
    wants.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
    let mut granted = vec![false; n];
    for &(_, _, i) in &wants {
        if slots == 0 {
            break;
        }
        granted[i] = true;
        slots -= 1;
    }
    for i in 0..n {
        if granted[i] || cd_score(&scores[i]).is_none() {
            continue;
        }
        if let Some(mut inp) = inputs[i].clone() {
            inp.slot_free = false;
            scores[i] = score_stances(&inp, &cfg);
        }
    }

    // 4. Apply.
    let jail = world.building_of_kind(BuildingKind::Jail);
    for i in 0..n {
        let current = world.districts[i].stance;
        let (next, reason, best, cur) = match &forced[i] {
            Some((s, r)) => (Some(*s).filter(|&s| s != current), r.clone(), 0.0, 0.0),
            None if inputs[i].is_none() => (None, String::new(), 0.0, 0.0),
            None => {
                let best = scores[i].first().map_or(0.0, |s| s.score);
                let cur = scores[i].iter().find(|s| s.stance == current).map_or(0.0, |s| s.score);
                (choose_stance(&scores[i], current, hysteresis), why.to_string(), best, cur)
            }
        };
        world.districts[i].stance_trace = std::mem::take(&mut scores[i]);
        let Some(next) = next else { continue };
        world.districts[i].stance = next;
        world.districts[i].stance_since = now;
        let name = world.districts[i].name.clone();
        let text = format!(
            "{name}: {} -> {} ({reason}, {best:.2} vs {cur:.2})",
            stance_label(world, current),
            stance_label(world, next)
        );
        let mut actors: Vec<EntityId> = jail.into_iter().collect();
        if let Stance::Crackdown(g) = next {
            actors.push(g);
        }
        world.push_event(EventKind::Stance, &actors, text);
    }
}

impl Law {
    /// Is the posture's Crackdown against this gang? (M12 D9: the city-wide
    /// question, district stances included, is `law::cracking_down_on`.)
    pub fn cracking_down_on(&self, gang: EntityId) -> bool {
        self.posture == Posture::Crackdown && self.target == Some(gang)
    }
}
