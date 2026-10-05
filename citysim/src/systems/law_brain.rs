//! The law's brain (M9). A captain, the most lawful guard, scores three
//! postures daily (with hysteresis) and at once when pending shocks add up:
//! `Patrol` (the v1 routine), `Crackdown` on the most-reported gang (patrol
//! loops through its turf, one guard in three holds the Jail) and
//! `Garrison` (every guard holds the Jail). The player may pin a posture.
//! Mirrors `faction` for the gangs; the `Law` component lives on the Jail.

use std::collections::BTreeMap;

use crate::components::{BuildingKind, Law, LawShock, Personality, Posture, PostureScore, Sentence};
use crate::config::LawCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::TICKS_PER_DAY;
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::world::World;

/// `report_log` keeps at most this many reports.
pub const REPORT_LOG_CAP: usize = 128;

/// Everything `score_postures` reads, gathered once per rescoring.
#[derive(Clone, Debug, PartialEq)]
pub struct PostureInputs {
    /// The gang with the most reports in the window, bribes excluded.
    pub wanted_gang: Option<EntityId>,
    /// Its reports ÷ `crackdown_reports`, clamped to 1.
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

/// Guards on the payroll.
pub fn guards(world: &World) -> Vec<EntityId> {
    world.guards().to_vec()
}

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

/// Gather the inputs, or `None` when there is no captain to decide.
pub fn gather_inputs(world: &World) -> Option<PostureInputs> {
    let l = world.law()?;
    let captain = l.captain.filter(|&c| crate::systems::law::is_guard(world, c))?;
    let p = world.comp::<Personality>(captain)?;
    let cfg = &world.config.law;
    let now = world.tick;
    let (wanted, reports) = wanted_gang(world).map_or((None, 0), |(g, n)| (Some(g), n));
    let capacity = usize::from(world.config.buildings.jail.capacity).max(1);
    let jailed_gang =
        world.with::<Sentence>().into_iter().filter(|&s| world.has::<crate::components::GangMember>(s)).count();
    Some(PostureInputs {
        wanted_gang: wanted,
        pressure: (reports as f32 / cfg.crackdown_reports.max(1) as f32).clamp(0.0, 1.0),
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
    // No captain: nothing is scored and the posture is the pin, or Patrol.
    let next = match (pinned, inputs.is_some()) {
        (Some(p), _) => (p != current).then_some(p),
        (None, true) => brain,
        (None, false) => (current != Posture::Patrol).then_some(Posture::Patrol),
    };
    let posture = next.unwrap_or(current);
    let target = if posture == Posture::Crackdown { inputs.as_ref().and_then(|i| i.wanted_gang) } else { None };
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
pub fn rethink(world: &mut World) {
    rescore(world, 0.0, "shock");
    if let Some(l) = world.law_mut() {
        l.shocks.clear();
    }
}

pub fn push_shock(world: &mut World, shock: LawShock) {
    if let Some(l) = world.law_mut() {
        l.shocks.push(shock);
    }
}

/// A report was filed against a gang member: the law remembers which gang.
pub fn log_report(world: &mut World, suspect: EntityId) {
    let Some(gang) = world.gang_of(suspect) else { return };
    let tick = world.tick;
    if let Some(l) = world.law_mut() {
        if l.report_log.len() >= REPORT_LOG_CAP {
            l.report_log.pop_front();
        }
        l.report_log.push_back((tick, gang));
    }
}

/// Daily at midnight, and at once when pending shocks reach
/// `shock_severity_rethink`. Called from `law::run` before the daily sweep.
pub fn run(world: &mut World) {
    let Some(l) = world.law() else { return };
    let pending: f32 = l.shocks.iter().map(|s| s.severity()).sum();
    if world.tick_of_day() == 0 {
        let h = world.config.law.hysteresis;
        rescore(world, h, "daily");
        if let Some(l) = world.law_mut() {
            l.shocks.clear();
        }
    } else if pending >= world.config.law.shock_severity_rethink {
        rethink(world);
    }
}

impl Law {
    /// Is a Crackdown in force against this gang?
    pub fn cracking_down_on(&self, gang: EntityId) -> bool {
        self.posture == Posture::Crackdown && self.target == Some(gang)
    }
}
