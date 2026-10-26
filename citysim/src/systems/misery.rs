//! The pressure pass phase 1 (docs/PRESSURE_PASS.md § Phase 1; OPEN-178):
//! misery acts. Mood (`mood.rs`, -1..1) moved only the Socialise/Court
//! weights, emigration, riots and class unrest; under `[misery]` a miserable
//! person behaves differently every day. Absent `[misery]`, every entry
//! point is a no-op and `misery` reads 0.
//!
//! **Misery** is `clamp((mood_line − mood) / mood_span, 0, 1)`: 0 at or
//! above the line, 1 at `mood_span` below it. Each mechanism reads it:
//!
//! - **Absenteeism** ([`daily`], the LivingEarly midnight, after the
//!   no-shows): a holder not on shift rolls once a day (a pure hash of the
//!   agent and the day) to skip the next shift, at `skip_p × misery`, damped
//!   by lawfulness and loyalty (the conscientiousness the Personality does
//!   not carry). A skip is a one-person strike (`classes::walk_out`): the
//!   shift is marked worked and struck, so no work and no wage on either
//!   tier, and the day's hours are off hours (leisure, the Bar, bed). The
//!   absences in the last `absence_window_days` are counted on [`Misery`];
//!   at `fire_absences` the employer dismisses the worker (a `Fire` event,
//!   a `Sacked` memory, not rehired there for `[life] quit_rehire_days`); a
//!   worker's own business never fires him.
//! - **Quitting** ([`daily`]): consecutive midnights below `quit_mood` while
//!   employed (`Misery.low_days`); past `quit_days` (less `quit_commute_days`
//!   for a commute past `quit_commute_ticks`, `quit_pay_days` for a wage
//!   under `quit_pay_meals` meals, `quit_bed_days` for a company bed, at
//!   least `quit_min_days`) the worker quits on `quit_p` a day
//!   (`economy::quit_job`; a company bed goes with the job through
//!   `company::evictions`).
//! - **Petty crime**: the planner's steal costs fall by `steal_cost_cut ×
//!   misery` and the vehicle theft's lawfulness gate by `steal_lawful_cut ×
//!   misery` (`goap::actions::PlanCtx.misery`); off screen the hour's
//!   `p_steal` is `× (1 + stat_steal_mult × misery)` (`lod::stat_probs`).
//! - **Fights** ([`hourly`]): a body (Full or Coarse) past `brawl_min`
//!   misery in a Bar or a leisure venue (drinking there), or on the street
//!   drunk (a drink within `drink_hours`) or high, rolls `brawl_p × misery × (1 −
//!   brawl_lawful_damp × lawfulness)` an hour (once a day at most) and
//!   swings at someone within `brawl_reach`: an enemy or grudge target
//!   first, a rival gang's member, else a stranger. The fight is
//!   `law::resolve_fight_mods` at `brawl_kill_mult`, an Assault (or a
//!   Murder) raised, a `Brawl` event, the bouncer's ejection, and grudges
//!   both ways (`brawl_grudge_w` on the starter, `× brawl_grudge_back` on
//!   a winner who was struck): Enemy edges the Fight and Hunt goals read.
//!   Off screen ([`daily`]) a miserable Statistical adult brawls with a
//!   stranger in its zone on `stat_brawl_p × misery × lawless` a day.
//! - **Recruitment**: `JoinGang` scores `+ join_flat × misery`; a recruit's
//!   edge to a member needs `join_affinity_cut × misery` less affinity; the
//!   miserable and lawless (past `join_min`, under `join_lawfulness`) are
//!   taken off the street as the desperate are (`gang::recruit_gang_any`);
//!   off screen a miserable Statistical adult a gang would take joins on
//!   `stat_join_p × misery` a day. The faith's Preach draws a resident past
//!   `faith_min` misery as it draws the lonely, and converts on `p_convert ×
//!   (1 + faith_convert_mult × misery)` (`faith::preach`). A join or a
//!   conversion that misery opened names it in its event.
//!
//! Statistical mirror: the daily rolls are tier-blind (the skip, the
//! firing, the quit); the theft is the hourly table's; the brawl and the
//! gang join are the daily rolls above.

use crate::components::{
    Brain, Building, BuildingKind, Crime, Job, Lod, MemoryKind, Mood, Personality, Position, RelKind, Role, Sentence,
};
use crate::config::MiseryCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::TICKS_PER_HOUR;
use crate::world::World;
use rand::Rng;

const PURPOSE_SKIP: u64 = 0x6d15_0001;
const PURPOSE_QUIT: u64 = 0x6d15_0002;
const PURPOSE_BRAWL: u64 = 0x6d15_0003;
const PURPOSE_STAT_BRAWL: u64 = 0x6d15_0004;
const PURPOSE_STAT_JOIN: u64 = 0x6d15_0005;
const PURPOSE_PICK: u64 = 0x6d15_0006;

/// A worker's misery bookkeeping (only for those it has touched).
pub use crate::components::Misery;

pub fn cfg(world: &World) -> Option<&MiseryCfg> {
    world.config.misery.as_ref()
}

/// Misery from a mood value: 0 at or above `mood_line`, 1 at `mood_line −
/// mood_span` and below.
pub fn misery_of(cfg: &MiseryCfg, mood: f32) -> f32 {
    ((cfg.mood_line - mood) / cfg.mood_span.max(0.01)).clamp(0.0, 1.0)
}

/// `id`'s misery now (0 with `[misery]` absent).
pub fn misery(world: &World, id: EntityId) -> f32 {
    let Some(cfg) = cfg(world) else { return 0.0 };
    world.comp::<Mood>(id).map_or(0.0, |m| misery_of(cfg, m.value))
}

fn mood(world: &World, id: EntityId) -> f32 {
    world.comp::<Mood>(id).map_or(0.0, |m| m.value)
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

fn misery_mut(world: &mut World, id: EntityId) -> Option<&mut Misery> {
    if !world.has::<Misery>(id) {
        world.insert(id, Misery::default());
    }
    world.comp_mut::<Misery>(id)
}

/// Is `id`'s job at its own business (nobody fires the owner)?
fn own_business(world: &World, id: EntityId, job: &Job) -> bool {
    job.employer.is_some_and(|e| world.owner_of(e) == Some(id))
}

/// Midnight (LivingEarly, after `fixes::daily`): the quits, the skips and
/// the firings (both tiers), then the off-screen brawls and gang joins.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let day = world.day();
    let tod = world.tick_of_day();
    let mut quits: Vec<(EntityId, u16)> = Vec::new();
    let mut skips: Vec<(EntityId, f32)> = Vec::new();
    let mut stat: Vec<(EntityId, f32)> = Vec::new();
    // scan-ok: daily: the misery rolls
    for id in world.citizens() {
        let Some(b) = world.comp::<Brain>(id) else { continue };
        if b.emigrating || world.has::<Sentence>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let statistical = b.lod == Lod::Statistical;
        let m = misery(world, id);
        let low = mood(world, id) < c.quit_mood;
        if let Some(job) = world.comp::<Job>(id).cloned() {
            if job.employer.is_some() && !own_business(world, id, &job) {
                // The quit's count of low midnights.
                if low || world.comp::<Misery>(id).is_some_and(|x| x.low_days > 0) {
                    let days = match misery_mut(world, id) {
                        Some(x) if low => {
                            x.low_days = x.low_days.saturating_add(1);
                            x.low_days
                        }
                        Some(x) => {
                            x.low_days = 0;
                            0
                        }
                        None => 0,
                    };
                    if low && days >= quit_days(world, &c, id, &job) && hash(world, PURPOSE_QUIT, id, day) < c.quit_p {
                        quits.push((id, days));
                        continue;
                    }
                }
                if m > 0.0 && !job.on_shift(tod) {
                    let p = world.comp::<Personality>(id).map_or(0.0, |p| {
                        c.skip_p
                            * m
                            * (1.0 - c.skip_lawful_damp * p.lawfulness).max(0.0)
                            * (1.0 - c.skip_loyal_damp * p.loyalty).max(0.0)
                    });
                    let key = job.next_shift_key(world.tick);
                    if hash(world, PURPOSE_SKIP, id, day) < p
                        && crate::exec::routine::workday_of(world, id, &job, key)
                        && job.last_shift_day != Some(key)
                    {
                        skips.push((id, m));
                    }
                }
            }
        }
        if statistical && m >= c.brawl_min {
            stat.push((id, m));
        }
    }
    for (id, days) in quits {
        quit(world, id, days);
    }
    for (id, m) in skips {
        skip(world, &c, id, m);
    }
    for (id, m) in stat {
        stat_brawl(world, &c, id, m);
        stat_join(world, &c, id, m);
    }
}

/// The days below `quit_mood` this worker stands before quitting.
fn quit_days(world: &World, c: &MiseryCfg, id: EntityId, job: &Job) -> u16 {
    let mut d = i32::from(c.quit_days);
    let work = job.employer.and_then(|e| world.comp::<Building>(e)).map(|b| b.door);
    let home = crate::systems::contracts::home_door(world, id);
    if let (Some(w), Some(h)) = (work, home) {
        if crate::systems::life::commute_cost(world, h, w) > c.quit_commute_ticks {
            d -= i32::from(c.quit_commute_days);
        }
    }
    let meal = crate::systems::life::meal_price(world, id).max(1);
    if (job.wage_per_day as f32) < c.quit_pay_meals * meal as f32 {
        d -= i32::from(c.quit_pay_days);
    }
    if crate::systems::company::housed(world, id) {
        d -= i32::from(c.quit_bed_days);
    }
    d.max(i32::from(c.quit_min_days)) as u16
}

fn quit(world: &mut World, id: EntityId, days: u16) {
    world.abort_plan(id);
    let reason = format!("miserable: {days} days low, mood {:+.2}", mood(world, id));
    crate::systems::economy::quit_job(world, id, &reason);
    if let Some(x) = world.comp_mut::<Misery>(id) {
        x.low_days = 0;
        x.absences.clear();
    }
    crate::systems::ownership::jobs2_row(world).misery_quits += 1;
}

/// One skipped shift (a strike of one), counted; the firing at the count.
fn skip(world: &mut World, c: &MiseryCfg, id: EntityId, m: f32) {
    let Some(job) = world.comp::<Job>(id).cloned() else { return };
    crate::systems::classes::walk_out(world, &[id]);
    let day = world.day() as u32;
    let window = c.absence_window_days;
    let count = match misery_mut(world, id) {
        Some(x) => {
            x.absences.retain(|&d| d + window > day);
            x.absences.push(day);
            x.absences.len() as u32
        }
        None => 0,
    };
    let text = format!(
        "{} skips the shift as {} (mood {:+.2}, misery {m:.2}; absence {count} in {window} days)",
        world.name_of(id),
        job.role.label(),
        mood(world, id),
    );
    let mut actors = vec![id];
    actors.extend(job.employer);
    world.push_event(EventKind::Skipped, &actors, text);
    crate::systems::ownership::jobs2_row(world).misery_skips += 1;
    if count >= c.fire_absences {
        fire(world, id, count, window);
    }
}

fn fire(world: &mut World, id: EntityId, count: u32, window: u32) {
    world.abort_plan(id);
    let Some(job) = world.vacate_job(id) else { return };
    crate::systems::life::note_quit(world, id, job.employer);
    let text =
        format!("{} dismissed as {} (absent {count} times in {window} days)", world.name_of(id), job.role.label());
    let mut actors = vec![id];
    actors.extend(job.employer);
    world.push_event(EventKind::Fire, &actors, text);
    world.remember(id, MemoryKind::Sacked, job.employer, 0.8, -0.6, false);
    if let Some(x) = world.comp_mut::<Misery>(id) {
        x.absences.clear();
        x.low_days = 0;
    }
    crate::systems::ownership::jobs2_row(world).misery_fired += 1;
}

// ---------------------------------------------------------------------------
// Brawls
// ---------------------------------------------------------------------------

/// Drunk (a drink within `drink_hours`) or high: the label, else `None`.
fn under_the_influence(world: &World, c: &MiseryCfg, id: EntityId) -> Option<&'static str> {
    if crate::systems::stims::is_high(world, id) {
        return Some("high");
    }
    let since = world.tick.saturating_sub(u64::from(c.drink_hours) * TICKS_PER_HOUR);
    world
        .comp::<crate::components::Memory>(id)
        // A drink's `Socialised` names nobody (a chat's names the partner)
        // and is the light one (salience 0.2, valence 0.3: the Drink action,
        // the Bar visit); a gang's welcome (0.4, 0.2) is no drink.
        .is_some_and(|m| {
            m.entries.iter().any(|e| {
                e.kind == MemoryKind::Socialised
                    && e.subject.is_none()
                    && e.salience < 0.3
                    && e.valence > 0.25
                    && e.tick >= since
            })
        })
        .then_some("drunk")
}

/// A brawl's ground: a Bar, a leisure venue, or the street (`None`).
fn brawl_ground(world: &World, id: EntityId) -> Option<Option<EntityId>> {
    let p = world.comp::<Position>(id)?;
    match p.building {
        None => Some(None),
        Some(b) => {
            let k = world.comp::<Building>(b)?.kind;
            (k == BuildingKind::Bar || k.is_leisure()).then_some(Some(b))
        }
    }
}

fn brawler_ok(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| {
        !b.emigrating && !matches!(b.exec, crate::exec::ExecState::Use { kind: crate::goap::ActionKind::Sleep, .. })
    }) && crate::systems::law::living(world, id)
        && crate::systems::demography::is_adult(world, id)
        && !world.has::<Sentence>(id)
        && !crate::systems::wounded::is_down(world, id)
        && !world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)
}

/// The one `a` swings at among `near`: an enemy or grudge target (0), a
/// rival gang's member (1), a stranger (2); never kin, a spouse or a friend.
/// Within a class the draw is a pure hash of the pair and the hour.
fn pick_mark(world: &World, a: EntityId, near: &[EntityId]) -> Option<(EntityId, &'static str)> {
    let my_gang = world.gang_of(a);
    let hour = world.tick / TICKS_PER_HOUR;
    near.iter()
        .copied()
        .filter(|&o| o != a && brawler_ok(world, o))
        .filter_map(|o| {
            let edge = world.edge(a, o);
            let close = edge.is_some_and(|e| {
                matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse | RelKind::Parent)
            });
            if close || (my_gang.is_some() && world.gang_of(o) == my_gang) {
                return None;
            }
            let enemy = edge.is_some_and(|e| matches!(e.kind, RelKind::Enemy | RelKind::Rival))
                || crate::systems::grudges::holds(world, a, o, world.config.grudges.fight_grudge_min);
            let rival = my_gang.is_some() && world.gang_of(o).is_some();
            let (class, why) = if enemy {
                (0u8, "a grudge")
            } else if rival {
                (1, "a rival's colours")
            } else {
                (2, "a stranger's look")
            };
            Some(((class, (hash(world, PURPOSE_PICK, o, hour ^ u64::from(a.index)) * 1e6) as u32, o.index), o, why))
        })
        .min_by_key(|(k, _, _)| *k)
        .map(|(_, o, why)| (o, why))
}

/// Hourly (Full and Coarse bodies): the miserable and drunk pick fights.
pub fn hourly(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let day = world.day() as u32;
    let hour = world.tick / TICKS_PER_HOUR;
    let cands: Vec<(EntityId, f32)> = world
        .bodies()
        .into_iter()
        .filter_map(|id| {
            let m = misery(world, id);
            (m >= c.brawl_min).then_some((id, m))
        })
        .collect();
    for (a, m) in cands {
        if !brawler_ok(world, a) || world.comp::<Misery>(a).is_some_and(|x| x.brawl_day == Some(day)) {
            continue;
        }
        let Some(ground) = brawl_ground(world, a) else { continue };
        // In a Bar or a venue the body is drinking there; on the street only
        // a drink within `drink_hours` or a dose counts.
        let Some(state) = under_the_influence(world, &c, a).or(ground.map(|_| "drinking")) else { continue };
        let lawful = world.comp::<Personality>(a).map_or(0.5, |p| p.lawfulness);
        let p = c.brawl_p * m * (1.0 - c.brawl_lawful_damp * lawful).max(0.0);
        if hash(world, PURPOSE_BRAWL, a, hour) >= p {
            continue;
        }
        let Some(tile) = world.comp::<Position>(a).map(|p| p.tile) else { continue };
        let near: Vec<EntityId> = match ground {
            Some(b) => world.comp::<Building>(b).map(|bd| bd.occupants.clone()).unwrap_or_default(),
            None => {
                let r = c.brawl_reach;
                let found = world.agents_near(tile, r, None);
                found
                    .into_iter()
                    .filter(|&o| {
                        world.comp::<Position>(o).is_some_and(|p| p.building.is_none() && p.tile.manhattan(tile) <= r)
                    })
                    .collect()
            }
        };
        let Some((b, why)) = pick_mark(world, a, &near) else { continue };
        brawl(world, &c, a, b, m, state, why, ground);
    }
}

#[allow(clippy::too_many_arguments)]
fn brawl(
    world: &mut World,
    c: &MiseryCfg,
    a: EntityId,
    b: EntityId,
    m: f32,
    state: &str,
    why: &str,
    ground: Option<EntityId>,
) {
    let day = world.day() as u32;
    if let Some(x) = misery_mut(world, a) {
        x.brawl_day = Some(day);
    }
    let tile = world.comp::<Position>(a).map_or_else(Default::default, |p| p.tile);
    let mods = crate::systems::law::FightMods { kill_mult: c.brawl_kill_mult, ..Default::default() };
    let (winner, loser, died) = crate::systems::law::resolve_fight_mods(world, a, b, mods);
    let place = ground.map_or_else(|| "on the street".to_string(), |g| format!("at {}", world.name_of(g)));
    let text = format!(
        "{} started a brawl with {} {place} (miserable {:+.2} and {state}; {why}); {} won{}",
        world.name_of(a),
        world.name_of(b),
        mood(world, a),
        world.name_of(winner),
        if died { format!(", {} died", world.name_of(loser)) } else { String::new() },
    );
    let _ = m;
    world.push_event(EventKind::Brawl, &[a, b], text);
    let murder = died && loser == b;
    if crate::systems::law::living(world, a) {
        let crime = if murder { Crime::Murder } else { Crime::Assault };
        if murder {
            let t = format!("{} killed {} in a brawl", world.name_of(a), world.name_of(b));
            world.push_event(EventKind::Murder, &[a, b], t);
        }
        crate::systems::law::raise_crime_on(world, a, (!murder).then_some(b), Some(b), crime, tile);
        crate::systems::service::maybe_eject(world, a);
    }
    // The grudges: the struck on the starter; a struck winner's starter on him.
    if crate::systems::law::living(world, b) {
        crate::systems::grudges::add(world, b, a, crate::word::GrudgeCause::Assaulted, c.brawl_grudge_w, 0);
    }
    if winner == b && crate::systems::law::living(world, a) {
        let w = c.brawl_grudge_w * c.brawl_grudge_back;
        crate::systems::grudges::add(world, a, b, crate::word::GrudgeCause::Assaulted, w, 0);
    }
    crate::systems::ownership::jobs2_row(world).brawls += 1;
}

/// Off screen: a miserable Statistical adult's day may hold a brawl with a
/// stranger in its zone (both Statistical; the fight and the grudges as on
/// screen, no witness: a report filed on `stat_brawl_report_p`).
fn stat_brawl(world: &mut World, c: &MiseryCfg, a: EntityId, m: f32) {
    if !world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical) || !brawler_ok(world, a) {
        return;
    }
    let lawful = world.comp::<Personality>(a).map_or(0.5, |p| p.lawfulness);
    let p = c.stat_brawl_p * m * (1.0 - c.brawl_lawful_damp * lawful).max(0.0);
    if hash(world, PURPOSE_STAT_BRAWL, a, world.day()) >= p {
        return;
    }
    let Some(b) = crate::systems::lod::stranger_in_zone(world, a) else { return };
    if !brawler_ok(world, b) {
        return;
    }
    let day = world.day() as u32;
    if let Some(x) = misery_mut(world, a) {
        x.brawl_day = Some(day);
    }
    let mods = crate::systems::law::FightMods { kill_mult: c.brawl_kill_mult, ..Default::default() };
    let (winner, _, died) = crate::systems::law::resolve_fight_mods(world, a, b, mods);
    let text = format!(
        "{} started a brawl with {} (off screen; miserable {:+.2}); {} won{}",
        world.name_of(a),
        world.name_of(b),
        mood(world, a),
        world.name_of(winner),
        if died { ", a death" } else { "" }
    );
    world.push_event(EventKind::Brawl, &[a, b], text);
    if crate::systems::law::living(world, b) {
        crate::systems::grudges::add(world, b, a, crate::word::GrudgeCause::Assaulted, c.brawl_grudge_w, 0);
    }
    if world.rng.agent(a).random::<f32>() < c.stat_brawl_report_p {
        crate::systems::law::file_report(world, Crime::Assault, a, None);
    }
    crate::systems::ownership::jobs2_row(world).brawls += 1;
}

/// Off screen: a miserable Statistical adult a gang would take joins it.
fn stat_join(world: &mut World, c: &MiseryCfg, a: EntityId, m: f32) {
    if world.has::<crate::components::GangMember>(a)
        || !world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical)
        || world.comp::<Job>(a).is_some_and(|j| j.role == Role::Guard)
    {
        return;
    }
    if hash(world, PURPOSE_STAT_JOIN, a, world.day()) >= c.stat_join_p * m {
        return;
    }
    let Some(g) = crate::systems::gang::recruit_gang(world, a) else { return };
    crate::systems::gang::enlist(world, a, g);
}

// ---------------------------------------------------------------------------
// Readers for the other systems
// ---------------------------------------------------------------------------

/// The JoinGang flat (`join_flat × misery`).
pub fn join_flat(world: &World, id: EntityId) -> f32 {
    cfg(world).map_or(0.0, |c| c.join_flat * misery(world, id))
}

/// The affinity a recruit's edge to a member may lack (`join_affinity_cut × misery`).
pub fn join_affinity_cut(world: &World, id: EntityId) -> f32 {
    cfg(world).map_or(0.0, |c| c.join_affinity_cut * misery(world, id))
}

/// Miserable and lawless enough to be taken off the street by a gang.
pub fn join_desperate(world: &World, id: EntityId) -> bool {
    cfg(world).is_some_and(|c| {
        misery(world, id) >= c.join_min
            && world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < c.join_lawfulness)
    })
}

/// The recruitment's note for a join's event: `Some` when misery counts.
pub fn join_note(world: &World, id: EntityId) -> Option<String> {
    let c = cfg(world)?;
    let m = misery(world, id);
    (m >= c.join_note_min).then(|| format!("miserable, mood {:+.2}", mood(world, id)))
}

/// The faith's draw: a resident past `faith_min` misery attends a Preach.
pub fn faith_drawn(world: &World, id: EntityId) -> bool {
    cfg(world).is_some_and(|c| misery(world, id) >= c.faith_min)
}

/// The conversion's multiplier `1 + faith_convert_mult × misery`.
pub fn faith_mult(world: &World, id: EntityId) -> f32 {
    cfg(world).map_or(1.0, |c| 1.0 + c.faith_convert_mult * misery(world, id))
}

/// The planner's theft terms: `(steal_cost_cut × misery, steal_lawful_cut × misery)`.
pub fn steal_cut(world: &World, id: EntityId) -> (f32, f32) {
    cfg(world).map_or((0.0, 0.0), |c| {
        let m = misery(world, id);
        (c.steal_cost_cut * m, c.steal_lawful_cut * m)
    })
}

/// The off-screen theft's multiplier `1 + stat_steal_mult × misery`.
pub fn stat_steal_mult(world: &World, id: EntityId) -> f32 {
    cfg(world).map_or(1.0, |c| 1.0 + c.stat_steal_mult * misery(world, id))
}

/// A skipped shift today keeps the Earn goal's shift gate open (the
/// absentee's day is his own).
pub fn skipped_now(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| j.struck_shift.is_some_and(|k| k == j.shift_key_at(world.tick)))
        && world.has::<Misery>(id)
}

/// A theft's note for its event: `Some` when the thief is miserable.
pub fn theft_note(world: &World, id: EntityId) -> Option<String> {
    let m = misery(world, id);
    (m > 0.0).then(|| format!(" (miserable, mood {:+.2})", mood(world, id)))
}
