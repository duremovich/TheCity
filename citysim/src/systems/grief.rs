//! Pressure pass round 8, X2 (docs/PRESSURE_PASS.md § X2; OPEN-247): a death
//! in the household is grief that takes days, and a man who has word of a
//! contract on him lies low.
//!
//! - **Mourning** ([`on_death`], under `[grief]`): the dead's household (the
//!   spouse wherever he sleeps, a parent, a child or a sibling under the same
//!   roof) mourns. An adult mourns `mourn_days_min..=mourn_days_max` days by
//!   his affinity to the dead, to the next midnight: each shift inside it is
//!   struck at the skip's moment (no work, no wage; [`hourly`]), the first `leave_days` the
//!   employer's leave, the rest absences on `[misery]`'s count (the firing at
//!   `fire_absences`); a household business gives any leave. His free hours
//!   are spent at home (`leisure::choice`). A child under 18 gets the line
//!   (who he lost, who he is at home with) and the `[family]` line's
//!   mourning; never a harm.
//! - **The funeral** ([`hourly`]): the day after the death at `wake_tick`
//!   the household holds it at the Home; the dead's Friends whose Home is
//!   within `wake_reach_tiles` of it are asked, a body among them makes it
//!   his Unwind (`leisure::choice`), and the line names who came. Nobody
//!   marries a widow(er) inside `wed_after_days` (`social::propose_allowed`).
//! - **The hot grudge** ([`hunt_flat`]): a mourner's Hunt on the killer of
//!   the one he mourns scores `+ hunt_hot_flat` for `hot_days` after the
//!   death (the hunt holds against the next morning's shift).
//! - **Lying low** ([`lie_low`], from `contracts::react`, under `[verbs]
//!   hit_low_days`, with or without `[grief]`): word of a Hit or a Beat taken
//!   on him keeps a man home: his shifts struck and counted as absences, his
//!   free hours indoors.
//!
//! Statistical mirror: the shifts are struck on both tiers (the wage is not
//! paid); at the funeral a body is there when it stands in the Home, an
//! off-screen mourner or a Friend off shift by the clock. No random draw.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{Brain, Building, Corpse, Household, Identity, Job, Lod, Position, RelKind, Sentence, Sex};
use crate::config::GriefCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// One mourner's days.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mourning {
    /// The dead, the name and what the dead was to him ("husband").
    pub of: EntityId,
    pub name: String,
    pub rel: String,
    pub since: Tick,
    pub until: Tick,
    /// The employer's leave shifts taken.
    #[serde(default)]
    pub leave_used: u32,
}

/// A funeral at the dead's Home.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wake {
    pub home: EntityId,
    pub name: String,
    pub at: Tick,
    pub household: Vec<EntityId>,
    pub invited: Vec<EntityId>,
    #[serde(default)]
    pub came: Vec<EntityId>,
}

/// `World.grief`: the mourners, the funerals due and the men lying low.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub mourners: BTreeMap<EntityId, Mourning>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wakes: Vec<Wake>,
    /// A man lying low: until, and why (the line's words).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub low: BTreeMap<EntityId, (Tick, String)>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.mourners.is_empty() && self.wakes.is_empty() && self.low.is_empty()
    }
}

/// `[grief]`; `None` off.
pub fn cfg(world: &World) -> Option<&GriefCfg> {
    world.config.grief.as_ref()
}

fn next_midnight(t: Tick) -> Tick {
    (t / TICKS_PER_DAY + 1) * TICKS_PER_DAY
}

/// Alive: an adult with his Brain, or a child (who has none).
fn living(world: &World, id: EntityId) -> bool {
    world.has::<Identity>(id)
        && !world.has::<Corpse>(id)
        && world.is_alive(id)
        && (world.has::<Brain>(id) || !crate::systems::demography::is_adult(world, id))
}

fn male(world: &World, id: EntityId) -> bool {
    world.comp::<Identity>(id).is_some_and(|i| i.sex == Sex::Male)
}

pub fn his(world: &World, id: EntityId) -> &'static str {
    if male(world, id) {
        "his"
    } else {
        "her"
    }
}

/// What `dead` was to `who` ("husband", "mother", "son", "sister").
fn rel_word(world: &World, who: EntityId, dead: EntityId) -> &'static str {
    let m = male(world, dead);
    let kind = world.edge(who, dead).map(|e| e.kind);
    let pick = |a: &'static str, b: &'static str| if m { a } else { b };
    match kind {
        Some(RelKind::Spouse) => pick("husband", "wife"),
        Some(RelKind::Parent) if crate::systems::demography::is_child_of(world, who, dead) => pick("father", "mother"),
        Some(RelKind::Parent) => pick("son", "daughter"),
        _ => pick("brother", "sister"),
    }
}

fn age(world: &World, id: EntityId) -> u32 {
    world.comp::<Identity>(id).map_or(0, |i| i.age_years())
}

/// From `demography::on_death`, before the dead is unlinked: the household
/// mourns, the children are told, a funeral is set for tomorrow.
pub fn on_death(world: &mut World, dead: EntityId) {
    let Some(c) = cfg(world).cloned() else { return };
    let now = world.tick;
    let home = world.comp::<Household>(dead).and_then(|h| h.home);
    let spouse = world.spouse_of(dead);
    let mut kin: Vec<EntityId> = world
        .kin_candidates(dead)
        // A child has no Brain (a line, never a shift); an adult mourner has one.
        .filter(|&o| {
            o != dead && living(world, o) && (world.has::<Brain>(o) || !crate::systems::demography::is_adult(world, o))
        })
        .filter(|&o| {
            world.edge(dead, o).is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family))
        })
        .filter(|&o| Some(o) == spouse || (home.is_some() && world.comp::<Household>(o).and_then(|h| h.home) == home))
        .collect();
    kin.sort();
    kin.dedup();
    if kin.is_empty() {
        return;
    }
    let dead_name = world.name_of(dead);
    let (adults, children): (Vec<EntityId>, Vec<EntityId>) =
        kin.iter().partition(|&&o| crate::systems::demography::is_adult(world, o));
    for &o in &adults {
        let aff = world.edge(o, dead).map_or(0.0, |e| e.affinity).clamp(0.0, 1.0);
        let span = c.mourn_days_max.saturating_sub(c.mourn_days_min) as f32;
        let days = c.mourn_days_min + (span * aff).round() as u32;
        let until = next_midnight(now + u64::from(days) * TICKS_PER_DAY);
        let rel = rel_word(world, o, dead).to_string();
        world.grief.mourners.insert(
            o,
            Mourning { of: dead, name: dead_name.clone(), rel: rel.clone(), since: now, until, leave_used: 0 },
        );
        let name = world.name_of(o);
        let to = until / TICKS_PER_DAY - 1;
        world.push_event(
            EventKind::Bereaved,
            &[o, dead],
            format!("{name} lost {} {rel} {dead_name}: mourns {days} days (through d{to})", his(world, o)),
        );
        // A shift under way or due today goes at once (the news, then home).
        strike_due(world, &c, o, TICKS_PER_DAY);
    }
    for &k in &children {
        let rel = rel_word(world, k, dead).to_string();
        let until = next_midnight(now + u64::from(c.mourn_days_max) * TICKS_PER_DAY);
        world.grief.mourners.insert(
            k,
            Mourning { of: dead, name: dead_name.clone(), rel: rel.clone(), since: now, until, leave_used: 0 },
        );
        // Who the child is at home with: a parent among the mourners first.
        let carer = adults
            .iter()
            .copied()
            .find(|&a| crate::systems::demography::is_child_of(world, k, a))
            .or_else(|| adults.first().copied());
        let with = match carer {
            Some(a) => format!("at home with {} {} {}", his(world, k), rel_word(world, k, a), world.name_of(a)),
            None => "no grown-up left at home".to_string(),
        };
        let text =
            format!("{} ({} y) lost {} {rel} {dead_name}; {with}", world.name_of(k), age(world, k), his(world, k));
        let mut actors = vec![k, dead];
        actors.extend(carer);
        world.push_event(EventKind::Bereaved, &actors, text);
    }
    // The funeral: tomorrow at `wake_tick`, at the Home, the near Friends asked.
    let Some(h) = home else { return };
    let Some(door) = world.comp::<Building>(h).map(|b| b.door) else { return };
    let mut invited: Vec<EntityId> = world
        .neighbours(dead)
        .filter(|&o| !kin.contains(&o) && living(world, o))
        .filter(|&o| world.edge(dead, o).is_some_and(|e| e.kind == RelKind::Friend && e.affinity > 0.0))
        .filter(|&o| crate::systems::demography::is_adult(world, o) && !world.has::<Sentence>(o))
        .filter(|&o| {
            world
                .comp::<Household>(o)
                .and_then(|x| x.home)
                .and_then(|x| world.comp::<Building>(x))
                .is_some_and(|b| b.door.manhattan(door) <= c.wake_reach_tiles)
        })
        .collect();
    invited.sort();
    let at = now / TICKS_PER_DAY * TICKS_PER_DAY + TICKS_PER_DAY + Tick::from(c.wake_tick);
    world.grief.wakes.push(Wake { home: h, name: dead_name, at, household: kin, invited, came: Vec::new() });
}

/// `id` mourns now.
pub fn mourning(world: &World, id: EntityId) -> Option<&Mourning> {
    if world.grief.mourners.is_empty() {
        return None;
    }
    world.grief.mourners.get(&id).filter(|m| m.until > world.tick)
}

/// `id` was widowed inside `[grief] wed_after_days` (no proposal either way).
pub fn widowed_lately(world: &World, id: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    world
        .comp::<Identity>(id)
        .and_then(|i| i.spouse_died_tick)
        .is_some_and(|t| world.tick.saturating_sub(t) < u64::from(c.wed_after_days) * TICKS_PER_DAY)
}

/// `id` keeps to his Home now: an adult mourning, or lying low.
pub fn holed_up(world: &World, id: EntityId) -> bool {
    (mourning(world, id).is_some() && crate::systems::demography::is_adult(world, id)) || lying_low(world, id)
}

/// `id` lies low now (word of a contract on him).
pub fn lying_low(world: &World, id: EntityId) -> bool {
    !world.grief.low.is_empty() && world.grief.low.get(&id).is_some_and(|&(t, _)| t > world.tick)
}

/// The `[family]` line's words for a child mourning ("mourning his father
/// Mako Jessop"), `None` otherwise.
pub fn family_note(world: &World, child: EntityId) -> Option<String> {
    let m = mourning(world, child)?;
    Some(format!("mourning {} {} {}", his(world, child), m.rel, m.name))
}

/// `contracts::react`: `id` lies low until `until` (the shifts struck at
/// midnight, counted; the free hours at home), `why` the line's words.
pub fn lie_low(world: &mut World, id: EntityId, until: Tick, why: String) {
    let e = world.grief.low.entry(id).or_insert((0, String::new()));
    if e.0 < until {
        *e = (until, why);
    }
    let low = world.grief.low.get(&id).map(|x| x.1.clone()).unwrap_or_default();
    strike_low(world, id, &low, TICKS_PER_DAY);
}

/// The contract he lay low over is closed: he goes about his days.
pub fn end_low(world: &mut World, id: EntityId) {
    world.grief.low.remove(&id);
}

/// The Hunt's flat while the grudge is hot: `id` mourns one `target` killed
/// (a `KilledKin` grudge on him for the mourned), within `hot_days`.
pub fn hunt_flat(world: &World, id: EntityId, target: EntityId) -> f32 {
    let Some(c) = cfg(world) else { return 0.0 };
    let Some(m) = world.grief.mourners.get(&id) else { return 0.0 };
    if world.tick >= m.since + u64::from(c.hot_days) * TICKS_PER_DAY {
        return 0.0;
    }
    let of = m.of;
    let hot = world.comp::<crate::word::Grudges>(id).is_some_and(|g| {
        g.list.iter().any(|x| {
            x.target == target
                && x.settled.is_none()
                && matches!(x.cause, crate::word::GrudgeCause::KilledKin(d) if d == of)
        })
    });
    if hot {
        c.hunt_hot_flat
    } else {
        0.0
    }
}

/// The shift of `id` to strike now: one starting within `lead` ticks and
/// before `until`, a workday, not yet struck; one under way only on the news
/// (`lead` a day: he leaves it), never at the hourly pass (a shift begun
/// before the mourning is his to finish; the next is struck).
fn due_shift(world: &World, id: EntityId, until: Tick, lead: Tick) -> Option<i64> {
    let job = world.comp::<Job>(id)?;
    job.employer?;
    let wait = Tick::from(job.ticks_until_shift(world.tick_of_day()));
    if wait > lead || world.tick + wait >= until {
        return None;
    }
    let key = job.next_shift_key(world.tick);
    if job.struck_shift == Some(key) {
        return None;
    }
    if wait == 0 && lead < TICKS_PER_DAY {
        return None;
    }
    if wait > 0 && job.last_shift_day == Some(key) {
        return None;
    }
    crate::exec::routine::workday_of(world, id, job, key).then_some(key)
}

/// The hourly pass's lead: `[misery] skip_lead_ticks` (the skip's moment);
/// 0 (never) with `[misery]` absent, where no absence is counted either.
fn lead(world: &World) -> Tick {
    crate::systems::misery::cfg(world).map_or(0, |c| Tick::from(c.skip_lead_ticks))
}

/// A mourner's due shift struck: the employer's leave, else an absence.
fn strike_due(world: &mut World, c: &GriefCfg, id: EntityId, lead: Tick) {
    let Some(m) = world.grief.mourners.get(&id).cloned() else { return };
    if world.has::<Sentence>(id) || due_shift(world, id, m.until, lead).is_none() {
        return;
    }
    let Some(job) = world.comp::<Job>(id).cloned() else { return };
    let own = crate::systems::economy::household_business(world, id, job.employer);
    let why = format!(": mourning {} {} {}", his(world, id), m.rel, m.name);
    if own || m.leave_used < c.leave_days {
        crate::systems::classes::walk_out(world, &[id]);
        let used = m.leave_used + u32::from(!own);
        if let Some(x) = world.grief.mourners.get_mut(&id) {
            x.leave_used = used;
        }
        let at = job.employer.map(|e| format!(" at {}", world.name_of(e))).unwrap_or_default();
        let leave =
            if own { "the household's own business".to_string() } else { format!("leave, {used} of {}", c.leave_days) };
        let text =
            format!("{} stays home from the shift as {}{at}{why} ({leave})", world.name_of(id), job.role.label());
        let mut actors = vec![id];
        actors.extend(job.employer);
        world.push_event(EventKind::Bereaved, &actors, text);
        crate::systems::ownership::jobs2_row(world).grief_leave += 1;
    } else {
        let moment = format!("{why}, past the employer's leave");
        if !crate::systems::misery::skip_for(world, id, &moment) {
            crate::systems::classes::walk_out(world, &[id]);
        }
        crate::systems::ownership::jobs2_row(world).grief_absent += 1;
    }
}

/// A man lying low: his due shift struck, an absence.
fn strike_low(world: &mut World, id: EntityId, why: &str, lead: Tick) {
    let Some(&(until, _)) = world.grief.low.get(&id) else { return };
    if world.has::<Sentence>(id) || due_shift(world, id, until, lead).is_none() {
        return;
    }
    let moment = format!(": lying low ({why})");
    if !crate::systems::misery::skip_for(world, id, &moment) {
        crate::systems::classes::walk_out(world, &[id]);
    }
    crate::systems::ownership::jobs2_row(world).hit_low_shifts += 1;
}

/// Midnight (after `misery::daily`): the days done let go.
pub fn daily(world: &mut World) {
    let now = world.tick;
    world.grief.mourners.retain(|_, m| m.until > now);
    world.grief.low.retain(|_, x| x.0 > now);
}

/// Hourly: the shifts of the mourners and the men lying low struck at the
/// skip's moment (`[misery] skip_lead_ticks` before the start).
fn strike_hour(world: &mut World) {
    let lead = lead(world);
    if lead == 0 {
        return;
    }
    if let Some(c) = cfg(world).cloned() {
        let ids: Vec<EntityId> = world.grief.mourners.keys().copied().collect();
        for id in ids {
            if crate::systems::demography::is_adult(world, id) && living(world, id) {
                strike_due(world, &c, id, lead);
            }
        }
    }
    let low: Vec<(EntityId, String)> = world.grief.low.iter().map(|(&id, x)| (id, x.1.clone())).collect();
    for (id, why) in low {
        if living(world, id) {
            strike_low(world, id, &why, lead);
        }
    }
}

/// The funeral `id` is asked to and may go to now: its Home and the door.
pub fn wake_pick(world: &World, id: EntityId) -> Option<(EntityId, crate::components::TilePos)> {
    if world.grief.wakes.is_empty() {
        return None;
    }
    let now = world.tick;
    let w = world.grief.wakes.iter().find(|w| {
        now + TICKS_PER_HOUR >= w.at && now < w.at + 2 * TICKS_PER_HOUR && w.invited.binary_search(&id).is_ok()
    })?;
    world.comp::<Building>(w.home).filter(|b| !b.demolished).map(|b| (w.home, world.outside_door(b)))
}

/// Hourly: the shifts struck ([`strike_hour`]); each funeral under way notes
/// who is there, and at its end the line.
pub fn hourly(world: &mut World) {
    if world.grief.is_empty() || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    strike_hour(world);
    let names = cfg(world).map_or(4, |c| usize::from(c.wake_names.max(1)));
    let now = world.tick;
    let mut i = 0;
    while i < world.grief.wakes.len() {
        let w = world.grief.wakes[i].clone();
        if now < w.at {
            i += 1;
            continue;
        }
        // A body is there when it stands in the Home; an off-screen one is
        // there by the clock (it has no walk to make), a mourner or a Friend
        // free of a shift and a cell.
        let here: Vec<EntityId> = w
            .household
            .iter()
            .chain(w.invited.iter())
            .copied()
            .filter(|&o| !w.came.contains(&o) && living(world, o) && !world.has::<Sentence>(o))
            .filter(|&o| match world.comp::<Brain>(o).map(|b| b.lod) {
                Some(Lod::Statistical) => {
                    w.household.contains(&o) || world.comp::<Job>(o).is_none_or(|j| !j.on_shift(world.tick_of_day()))
                }
                Some(_) => world.comp::<Position>(o).is_some_and(|p| p.building == Some(w.home)),
                // A child of the household (no Brain) stands with it.
                None => w.household.contains(&o),
            })
            .collect();
        world.grief.wakes[i].came.extend(here);
        if now < w.at + 2 * TICKS_PER_HOUR {
            i += 1;
            continue;
        }
        let w = world.grief.wakes.remove(i);
        let kin: Vec<EntityId> = w.came.iter().copied().filter(|o| w.household.contains(o)).collect();
        let friends: Vec<EntityId> = w.came.iter().copied().filter(|o| w.invited.contains(o)).collect();
        let list = |ids: &[EntityId]| -> String {
            let mut out: Vec<String> = ids.iter().take(names).map(|&a| world.name_of(a)).collect();
            if ids.len() > names {
                out.push(format!("{} more", ids.len() - names));
            }
            out.join(", ")
        };
        let who = match (kin.is_empty(), friends.is_empty()) {
            (true, true) => "nobody stood there".to_string(),
            (false, true) if w.invited.is_empty() => format!("{} there; no friend near to ask", list(&kin)),
            (false, true) => format!("{} there; none of the {} friends asked came", list(&kin), w.invited.len()),
            (true, false) => format!("{} came; the household was not there", list(&friends)),
            (false, false) => format!(
                "{}, and {} of the {} friends asked: {}",
                list(&kin),
                friends.len(),
                w.invited.len(),
                list(&friends)
            ),
        };
        let text = format!("The funeral of {} at {}: {who}", w.name, world.name_of(w.home));
        let mut actors: Vec<EntityId> = w.household.clone();
        actors.extend(friends.iter().take(names).copied());
        actors.push(w.home);
        world.push_event(EventKind::Bereaved, &actors, text);
        crate::systems::ownership::jobs2_row(world).grief_funerals += 1;
    }
}
