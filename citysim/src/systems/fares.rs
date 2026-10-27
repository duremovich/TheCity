//! Pressure pass round 2, R1 (docs/PRESSURE_PASS.md § R1, `OPEN-213`):
//! petty law for the fare. Fare evasion is never a cell.
//!
//! A dodger is caught at the El by the on-duty guard who saw the gate jumped
//! (`law::raise_crime`'s law's eye; nobody else's sighting is a warrant, and
//! `law::file_report` files no Fare Evasion while `[fares]` is set). Then the
//! ladder, on the spot: the fine if the wallet holds it, else the guard beats
//! him (down on the platform `beat_down_minutes`, a `Lost` memory, a grudge on
//! the guard); caught `ban_after` times inside `ban_window_days` he is banned
//! from the El for `ban_days` and walks (`transit::pays_or_dodges` refuses
//! him the ride). A banned rider under `ban_break_lawfulness` boards anyway;
//! caught, he is beaten whatever his wallet holds and banned afresh.
//! Caught within `wary_hours` he jumps no gate (pays the fare or walks), so
//! the ban takes three bad days, not three tries at one gate in an hour.
//!
//! Off screen (`transit::stat_commute`, `stat_errand`) the same rule, coarse:
//! a jumped leg caught at the Full city's rate is fined, else beaten (the
//! memory and the mood, no body, no guard), and counts toward the ban.
//!
//! The El's own staff (`TransitState.staff`) are a headcount of sweepers, not
//! bodies posted at the stops, so the catcher is always a guard; a stop with
//! no guard on duty in sight catches nobody (the beating rung is never
//! skipped on screen: there is no catch without the body that beats).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{Brain, Lod, MemoryKind, Needs, Personality, Position, Sentence, TilePos, Wallet};
use crate::config::FaresCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::systems::law;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// The dodgers' records and the beaten lying on the platform. Saved while
/// it holds anything.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub book: BTreeMap<EntityId, Record>,
    /// Beaten dodgers down on the platform: (who, on his feet at).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub down: Vec<(EntityId, Tick)>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.book.is_empty() && self.down.is_empty()
    }
}

/// One dodger's record: the catches inside the window, the ban.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caught: Vec<Tick>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banned_until: Option<Tick>,
}

pub fn cfg(world: &World) -> Option<&FaresCfg> {
    world.config.fares.as_ref()
}

/// Is `[fares]` set?
pub fn on(world: &World) -> bool {
    cfg(world).is_some()
}

/// Is `id` banned from the El now?
pub fn banned(world: &World, id: EntityId) -> bool {
    on(world) && world.fares.book.get(&id).and_then(|r| r.banned_until).is_some_and(|t| t > world.tick)
}

/// A banned rider who boards anyway (under `ban_break_lawfulness`).
pub fn breaks_ban(world: &World, id: EntityId) -> bool {
    let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    cfg(world).is_some_and(|c| law < c.ban_break_lawfulness)
}

/// Was `id` caught at the gate at or after `since` (the deterrence that
/// `[transit] dodge_deterred_days` read from an arrest before R1)?
pub fn caught_since(world: &World, id: EntityId, since: Tick) -> bool {
    world.fares.book.get(&id).and_then(|r| r.caught.last()).is_some_and(|&t| t >= since)
}

/// Caught within `wary_hours`: he jumps no gate (pays the fare or walks).
pub fn wary(world: &World, id: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    caught_since(world, id, world.tick.saturating_sub(c.wary_hours * crate::time::TICKS_PER_HOUR))
}

/// Was `id` caught this tick (the ride's caller decides whether he rides on)?
pub fn caught_now(world: &World, id: EntityId) -> bool {
    caught_since(world, id, world.tick)
}

/// Is `id` lying on the platform after a beating?
pub fn is_down(world: &World, id: EntityId) -> bool {
    world.fares.down.iter().any(|&(x, _)| x == id)
}

/// The on-duty `guard` (none off screen) caught `dodger` jumping the gate
/// at `tile`: the ladder.
pub fn caught(world: &mut World, guard: Option<EntityId>, dodger: EntityId, tile: TilePos) {
    let Some(c) = cfg(world).cloned() else { return };
    if !law::living(world, dodger) || world.has::<Sentence>(dodger) || is_down(world, dodger) {
        return;
    }
    let now = world.tick;
    let window = c.ban_window_days * TICKS_PER_DAY;
    let was_banned = banned(world, dodger);
    let rec = world.fares.book.entry(dodger).or_default();
    // Two guards on one platform: one catch.
    if rec.caught.last() == Some(&now) {
        return;
    }
    rec.caught.retain(|&t| t + window > now);
    rec.caught.push(now);
    let times = rec.caught.len();
    {
        let row = ownership::jobs2_row(world);
        row.fare_caught += 1;
        if guard.is_none() {
            row.fare_stat += 1;
        }
    }
    let gname = guard.map_or_else(|| "the El's guard".to_string(), |g| world.name_of(g));
    let dname = world.name_of(dodger);
    let place = world.district_name(world.district_of(tile)).to_string();
    let actors: Vec<EntityId> = std::iter::once(dodger).chain(guard).collect();
    if was_banned {
        beat(world, &c, guard, dodger);
        ban(world, &c, dodger);
        ownership::jobs2_row(world).fare_ban_broken += 1;
        let him = him(world, dodger);
        let text = format!(
            "{gname} caught {dname} on the El in {place} inside the ban and beat {him} for Fare Evasion; banned {} days afresh",
            c.ban_days
        );
        world.push_event(EventKind::Fare, &actors, text);
        return;
    }
    let coins = world.comp::<Wallet>(dodger).map_or(0, |w| w.coins);
    if coins >= c.fine {
        ownership::pay(world, Some(dodger), None, c.fine, Flow::Fine);
        world.remember(dodger, MemoryKind::Paid, guard, 0.4, -0.4, false);
        ownership::jobs2_row(world).fare_fined += 1;
        let text = format!("{gname} fined {dname} {} coins at the El in {place} for Fare Evasion", c.fine);
        world.push_event(EventKind::Fare, &actors, text);
    } else {
        let laid = beat(world, &c, guard, dodger);
        ownership::jobs2_row(world).fare_beaten += 1;
        let tail = if laid { format!(", left on the platform {} min", c.beat_down_minutes) } else { String::new() };
        let text = format!("{gname} beat {dname} at the El in {place} for Fare Evasion: no fare, no fine{tail}");
        world.push_event(EventKind::Fare, &actors, text);
    }
    if times >= usize::from(c.ban_after) {
        ban(world, &c, dodger);
        ownership::jobs2_row(world).fare_banned += 1;
        let text = format!(
            "{dname} banned from the El for {} days: caught at the gate {times} times in {} days",
            c.ban_days, c.ban_window_days
        );
        world.push_event(EventKind::Fare, &[dodger], text);
    }
}

/// Off screen: a Statistical dodger caught (`transit::stat_commute`,
/// `stat_errand`): the ladder with no guard's body and no body of his own.
pub fn stat_caught(world: &mut World, dodger: EntityId) {
    let tile = world.comp::<Position>(dodger).map(|p| p.tile).unwrap_or_default();
    caught(world, None, dodger, tile);
}

fn him(world: &World, id: EntityId) -> &'static str {
    match world.comp::<crate::components::Identity>(id).map(|i| i.sex) {
        Some(crate::components::Sex::Female) => "her",
        _ => "him",
    }
}

fn ban(world: &mut World, c: &FaresCfg, dodger: EntityId) {
    let until = world.tick + c.ban_days * TICKS_PER_DAY;
    world.fares.book.entry(dodger).or_default().banned_until = Some(until);
}

/// The beating: a `Lost` memory (the street hears "assaulted by" the guard),
/// a grudge on the guard, the mood and the safety; a body lies on the
/// platform `beat_down_minutes`. True when the body was laid down.
fn beat(world: &mut World, c: &FaresCfg, guard: Option<EntityId>, dodger: EntityId) -> bool {
    world.remember(dodger, MemoryKind::Lost, guard, 0.6, -0.6, false);
    if let Some(g) = guard {
        world.remember(g, MemoryKind::Won, Some(dodger), 0.3, 0.2, false);
        crate::systems::grudges::add(world, dodger, g, crate::word::GrudgeCause::Assaulted, c.beat_grudge, 0);
    }
    if let Some(n) = world.comp_mut::<Needs>(dodger) {
        n.safety = (n.safety - c.beat_safety).max(0.0);
    }
    if let Some(m) = world.comp_mut::<crate::components::Mood>(dodger) {
        m.value = (m.value - c.beat_mood).max(-1.0);
    }
    if let Some(p) = world.comp_mut::<Personality>(dodger) {
        p.drift(Drift::LostFight);
    }
    let body = world.comp::<Brain>(dodger).is_some_and(|b| b.lod != Lod::Statistical);
    if !body || c.beat_down_minutes == 0 || world.comp::<Brain>(dodger).is_some_and(|b| b.cuffed_by.is_some()) {
        return false;
    }
    world.abort_plan(dodger);
    if let Some(b) = world.comp_mut::<Brain>(dodger) {
        b.cuffed_by = Some(dodger);
        b.current_goal = None;
    }
    let up = world.tick + c.beat_down_minutes;
    world.fares.down.push((dodger, up));
    true
}

/// Every tick: the beaten get up; at the tail of midnight the stale records go.
pub fn run(world: &mut World) {
    if !world.fares.down.is_empty() {
        let now = world.tick;
        let (up, lying): (Vec<_>, Vec<_>) = world.fares.down.iter().copied().partition(|&(_, t)| t <= now);
        world.fares.down = lying;
        for (id, _) in up {
            if !law::living(world, id) {
                continue;
            }
            let mine =
                world.comp::<Brain>(id).is_some_and(|b| b.cuffed_by == Some(id)) && !world.wounded.contains_key(&id);
            if mine {
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.cuffed_by = None;
                }
                let text = format!("{} picked {}self up off the platform", world.name_of(id), him(world, id));
                world.push_event(EventKind::Fare, &[id], text);
            }
        }
    }
    if world.fares.book.is_empty() || !crate::systems::midnight::due(world, crate::systems::midnight::Mid::Tail) {
        return;
    }
    let Some(c) = cfg(world) else { return };
    let keep = c.ban_window_days.max(u64::from(world.config.transit.dodge_deterred_days)) * TICKS_PER_DAY;
    let now = world.tick;
    world.fares.book.retain(|_, r| {
        r.caught.retain(|&t| t + keep > now);
        !r.caught.is_empty() || r.banned_until.is_some_and(|t| t > now)
    });
}
