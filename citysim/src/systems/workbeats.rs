//! M18 W, the work beats (docs/M18_PLAYER.md § W; BIG_PICTURE_2026-11-02 § 1):
//! a shift reads as a day at work, not ten identical blocks in an empty room.
//!
//! At a shift's seams a `[[work_beats]]` row is drawn (`[work_beat]`): a
//! station block's end that is not the shift's last (`shift::block_done`,
//! `p_block`), the break (`shift::stations_plan` / `haul_plan`, `p_break`),
//! a corp exec's office hour (`service::meeting_hour`, `p_hour`). The rows
//! open there are those of the body's role whose hour band holds the minute
//! and whose counterpart exists; one is picked by weight. The counterpart is
//! a real coworker on shift (`service::present`: a body inside the
//! workplace, or a Statistical one the attendance table has at work, named
//! in the line): a colleague, the lunch partner with the warmest edge (so
//! the same face recurs across a week), the foreman or a higher grade, the
//! owner or the corp's exec. Nobody under 18 is ever a counterpart.
//!
//! Effects only through `social` (the co-work drift, an affinity and trust
//! step), memories (both sides keep the row's kind and valence) and gossip
//! (`gossip::tell`: a rumour told across the desk). One `WorkBeat` event a
//! beat, `[body, counterpart, workplace]`: the diary's line and the
//! possessed body's log. All draws are keyed hashes (no world stream).
//!
//! The same code path for every body: a possessed one's shift seams draw
//! as an NPC's do.

use crate::components::{Brain, Building, Identity, Job, Lod, Role, Sentence};
use crate::config::{BeatAt, BeatEffect, BeatWho, WorkBeatRow};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::world::World;
use serde::{Deserialize, Serialize};

/// The keyed-hash purposes (`econ::hash_unit`).
const PURPOSE_ROLL: u64 = 0x3EA7_0001;
const PURPOSE_PICK: u64 = 0x3EA7_0002;
const PURPOSE_WHO: u64 = 0x3EA7_0003;

/// A body's beats in its current shift (`World.shift_beats`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShiftBeats {
    /// The shift key (an exec: the day).
    pub key: i64,
    /// The `once` rows already drawn (bit = row index, the first 64).
    pub fired: u64,
    /// Beats this shift.
    pub count: u8,
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

/// The deck is on: `[work_beat]` present and a row in `[[work_beats]]`.
pub fn on(world: &World) -> bool {
    world.config.work_beat.is_some() && !world.config.work_beats.is_empty()
}

/// The key a row's `roles` match: a trade's id, a built-in role's name.
fn role_key(world: &World, role: Role) -> Option<String> {
    match role {
        Role::Trade(t) => world.config.trade(t).map(|c| c.id.clone()),
        r => Some(format!("{r:?}")),
    }
}

/// `who` can be a beat's counterpart: an adult with a living body, free.
fn fit(world: &World, who: EntityId) -> bool {
    world.has::<Identity>(who)
        && world.has::<Brain>(who)
        && !world.has::<Sentence>(who)
        && crate::systems::law::living(world, who)
        && crate::systems::demography::is_adult(world, who)
}

/// The coworkers of `id` present at `post` now (`service::present`: a body
/// inside it, or a Statistical one the attendance table has at work),
/// ascending. No preference for bodies: at 20k the diary's pick is often the
/// one body in its room, and a body-first pick made it every beat's
/// counterpart (the first shadow read).
fn present_mates(world: &World, id: EntityId, post: EntityId) -> Vec<EntityId> {
    world
        .staff_of(post)
        .iter()
        .copied()
        .filter(|&s| s != id && crate::systems::service::present(world, s, post) && fit(world, s))
        .collect()
}

/// A hashed pick of `from` for `id` at this tick and row.
fn pick(world: &World, id: EntityId, row: usize, from: &[EntityId]) -> Option<EntityId> {
    if from.is_empty() {
        return None;
    }
    let u = hash(world, PURPOSE_WHO ^ row as u64, key(id), world.tick);
    Some(from[((u * from.len() as f32) as usize).min(from.len() - 1)])
}

/// The grade a coworker holds (its post's, else the role's base).
fn grade(world: &World, s: EntityId) -> u8 {
    world.comp::<Job>(s).map_or(0, |j| j.grade.max(crate::systems::classpay::base_grade(world, j.employer, j.role)))
}

/// The foreman present, else the present coworker of the highest grade
/// above `id`'s (ties the lower id).
fn superior(world: &World, id: EntityId, mates: &[EntityId]) -> Option<EntityId> {
    if let Some(&f) = mates.iter().find(|&&s| world.comp::<Job>(s).is_some_and(|j| j.foreman)) {
        return Some(f);
    }
    let mine = grade(world, id);
    mates
        .iter()
        .map(|&s| (grade(world, s), s))
        .filter(|&(g, _)| g > mine)
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, s)| s)
}

/// The workplace's person owner at it, else its corp's exec when the post
/// is the exec's Hq office, else the foreman present. "At it": a body inside
/// the post, or a Statistical one (named; the exec's office hours).
fn boss(world: &World, id: EntityId, post: EntityId, mates: &[EntityId]) -> Option<EntityId> {
    let here = |who: EntityId| match world.comp::<Brain>(who).map(|b| b.lod) {
        Some(Lod::Statistical) => true,
        Some(_) => world.comp::<crate::components::Position>(who).is_some_and(|p| p.building == Some(post)),
        None => false,
    };
    let owner = world.owner_of(post);
    let person = owner.filter(|&o| o != id && world.has::<Identity>(o) && fit(world, o) && here(o));
    let exec = owner.filter(|&o| world.has::<crate::components::Corp>(o)).and_then(|corp| {
        let e = world.comp::<crate::components::Corp>(corp)?.exec?;
        (e != id && fit(world, e) && crate::systems::life::office_of(world, corp, e) == Some(post)).then_some(e)
    });
    person.or(exec).or_else(|| mates.iter().copied().find(|&s| world.comp::<Job>(s).is_some_and(|j| j.foreman)))
}

/// The coworker present with the warmest edge to `id` (ties the lower id);
/// none with an edge, a hashed one.
fn partner(world: &World, id: EntityId, row: usize, mates: &[EntityId]) -> Option<EntityId> {
    mates
        .iter()
        .filter_map(|&s| world.edge(id, s).filter(|e| e.affinity > 0.0).map(|e| (e.affinity, s)))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, s)| s)
        .or_else(|| pick(world, id, row, mates))
}

/// The counterpart of `row` for `id` at `post`.
fn counterpart(
    world: &World,
    id: EntityId,
    post: EntityId,
    row: usize,
    r: &WorkBeatRow,
    mates: &[EntityId],
) -> Option<EntityId> {
    match r.who {
        BeatWho::Colleague => pick(world, id, row, mates),
        BeatWho::Partner => partner(world, id, row, mates),
        BeatWho::Superior => superior(world, id, mates),
        BeatWho::Boss => boss(world, id, post, mates),
        // Any staffer present, hashed (the graded few alone made one clerk the
        // exec's every hour: the first shadow read).
        BeatWho::Staff => pick(world, id, row, mates),
    }
}

/// `tod` lies in the `[from, to)` band (`from > to` wraps midnight).
fn in_band(tod: u16, (from, to): (u16, u16)) -> bool {
    if from <= to {
        tod >= from && tod < to
    } else {
        tod >= from || tod < to
    }
}

/// A shift seam of `id` working `role` at `post`: maybe a beat. `shift_key`
/// is the shift's (an exec's: the day). True when a beat was drawn.
pub fn seam(world: &mut World, id: EntityId, post: EntityId, role: Option<Role>, at: BeatAt, shift_key: i64) -> bool {
    let Some(cfg) = world.config.work_beat.clone() else { return false };
    if world.config.work_beats.is_empty() || !fit(world, id) {
        return false;
    }
    let p = match at {
        BeatAt::Block => cfg.p_block,
        BeatAt::Break => cfg.p_break,
        BeatAt::Hour => cfg.p_hour,
    };
    let tick = world.tick;
    if hash(world, PURPOSE_ROLL ^ at as u64, key(id), tick) >= p {
        return false;
    }
    let tally = tally_at(world, id, shift_key);
    if tally.count >= cfg.max_per_shift {
        return false;
    }
    // A break already shared (another's lunch drew this body) is not drawn again.
    if at == BeatAt::Break && shared_break(world, &tally) {
        return false;
    }
    let rkey = match role {
        Some(r) => role_key(world, r),
        None => Some("exec".to_string()),
    };
    let tod = world.tick_of_day();
    // A coworker who has had a shift's worth of beats (as either side) is
    // left to work: the room's beats spread over the room.
    let mates: Vec<EntityId> =
        present_mates(world, id, post).into_iter().filter(|&m| tally_of(world, m).count < cfg.max_per_shift).collect();
    // The rows open here, each with its counterpart.
    let mut open: Vec<(usize, f32, EntityId)> = Vec::new();
    for (i, r) in world.config.work_beats.iter().enumerate() {
        if r.at != at || r.weight <= 0.0 || !in_band(tod, r.hours) {
            continue;
        }
        let reaches = r.roles.iter().any(|x| Some(x) == rkey.as_ref() || (x == "*" && role.is_some()));
        if !reaches || (r.once && i < 64 && tally.fired & (1 << i) != 0) {
            continue;
        }
        if let Some(c) = counterpart(world, id, post, i, r, &mates).filter(|&c| c != id) {
            open.push((i, r.weight, c));
        }
    }
    let total: f32 = open.iter().map(|o| o.1).sum();
    if total <= 0.0 {
        return false;
    }
    let mut u = hash(world, PURPOSE_PICK ^ at as u64, key(id), tick) * total;
    let &(i, _, them) = open
        .iter()
        .find(|o| {
            u -= o.1;
            u < 0.0
        })
        .unwrap_or(open.last().expect("non-empty"));
    let row = world.config.work_beats[i].clone();
    if !apply(world, id, them, post, role, &row, cfg.salience) {
        return false;
    }
    let mut t = tally;
    t.count = t.count.saturating_add(1);
    if row.once && i < 64 {
        t.fired |= 1 << i;
    }
    world.shift_beats.insert(id, t);
    // The counterpart's shift counts the beat too.
    let mut c = tally_of(world, them);
    c.count = c.count.saturating_add(1);
    // A shared break (a partner row) is the counterpart's break too.
    if row.who == BeatWho::Partner && row.once && i < 64 {
        c.fired |= 1 << i;
    }
    world.shift_beats.insert(them, c);
    true
}

/// A `break` partner row has fired in `t` (the body ate with someone).
fn shared_break(world: &World, t: &ShiftBeats) -> bool {
    world
        .config
        .work_beats
        .iter()
        .enumerate()
        .take(64)
        .any(|(i, r)| r.at == BeatAt::Break && r.who == BeatWho::Partner && t.fired & (1 << i) != 0)
}

/// `who`'s beats in the shift (or day) keyed `k`.
fn tally_at(world: &World, who: EntityId, k: i64) -> ShiftBeats {
    world
        .shift_beats
        .get(&who)
        .filter(|t| t.key == k)
        .cloned()
        .unwrap_or(ShiftBeats { key: k, ..ShiftBeats::default() })
}

/// `who`'s beats now: its shift's (`Job::next_shift_key`), else the day's (an exec).
fn tally_of(world: &World, who: EntityId) -> ShiftBeats {
    let k = world.comp::<Job>(who).map_or(world.day() as i64, |j| j.next_shift_key(world.tick));
    tally_at(world, who, k)
}

/// A rumour's party as the line names it: a person, a building, else "somebody" (the dead lose their names).
fn party(world: &World, who: Option<EntityId>) -> String {
    match who {
        Some(w) if world.has::<Identity>(w) || world.has::<Building>(w) => world.name_of(w),
        _ => "somebody".to_string(),
    }
}

/// A told deed as a clause: "Raven Vasquez robbed Amara Nightingale".
fn deed_clause(world: &World, r: &crate::word::DeedRef) -> String {
    use crate::word::Deed as D;
    let (a, o) = (party(world, r.actor), party(world, r.object));
    match r.deed {
        D::Killed => format!("{a} killed {o}"),
        D::Assaulted => format!("{a} beat {o}"),
        D::Robbed => format!("{a} robbed {o}"),
        D::Extorted => format!("{a} squeezed {o}"),
        D::Stripped => format!("{a} stripped {o}"),
        D::Arrested => format!("{o} was taken in by {a}"),
        D::Married => format!("{a} married {o}"),
        D::Evicted => format!("{o} was put out by {a}"),
        D::Struck => format!("{a} walked out"),
        D::Raided => format!("{a} hit {o}"),
        D::Founded => format!("{a} opened {o}"),
        D::Avenged => format!("{a} settled it with {o}"),
        D::Betrayed => format!("{a} sold out {o}"),
        D::Repaid => format!("{a} paid {o} back"),
        D::Poached => format!("{a} poached {o}"),
        D::Hired => format!("{a} put money on {o}"),
        D::Gave => format!("{a} gave to {o}"),
        D::Blackmailed => format!("{a} has something on {o}"),
        D::Rescued => format!("{a} got {o} out"),
        D::Employed => format!("{a} got taken on at {o}"),
        D::LaidOff => format!("{a} got laid off from {o}"),
        D::Born => format!("{a} had a child"),
        D::Decided => format!("{a} made a call"),
        D::TookOver => format!("{a} took over {o}"),
    }
}

/// The beat's effects and its line; false when it had nothing to carry (a
/// rumour with nothing to tell).
fn apply(
    world: &mut World,
    me: EntityId,
    them: EntityId,
    post: EntityId,
    role: Option<Role>,
    row: &WorkBeatRow,
    salience: f32,
) -> bool {
    let mut about = String::new();
    match row.effect {
        BeatEffect::Rumour => {
            let Some(r) = crate::systems::gossip::tell(world, them, me) else { return false };
            // The dead lose their names: a deed with nobody named is told,
            // but makes no line ("somebody killed somebody").
            if party(world, r.actor) == "somebody" && party(world, r.object) == "somebody" {
                return false;
            }
            about = deed_clause(world, &r);
            crate::systems::social::interacted(world, me, them);
        }
        BeatEffect::Drift => crate::systems::social::interacted(world, me, them),
        BeatEffect::Nudge => {}
        BeatEffect::Skill => {
            let skill = role.and_then(|r| {
                crate::systems::trades::row_of(world, r)
                    .map(|t| t.skill)
                    .or_else(|| crate::systems::service::row_for(world, r).map(|s| s.skill))
            });
            if let Some(s) = skill {
                crate::systems::service::train(world, me, s, row.skill_gain);
            }
        }
    }
    if row.affinity != 0.0 || row.trust != 0.0 {
        crate::systems::social::adjust(world, me, them, row.affinity, row.trust);
    }
    world.remember(me, row.memory, Some(them), salience, row.valence, false);
    world.remember(them, row.memory, Some(me), salience, row.valence, false);
    let line = row
        .text
        .replace("{me}", &world.name_of(me))
        .replace("{them}", &world.name_of(them))
        .replace("{at}", &world.name_of(post))
        .replace("{about}", &about);
    world.push_event(EventKind::WorkBeat, &[me, them, post], line);
    true
}

/// `shift::block_done` / the break: a shaped shift's seam for `id`.
pub fn shift_seam(world: &mut World, id: EntityId, at: BeatAt) {
    if !on(world) {
        return;
    }
    let Some((post, role, k)) =
        world.comp::<Job>(id).and_then(|j| j.employer.map(|e| (e, j.role, j.next_shift_key(world.tick))))
    else {
        return;
    };
    if world.comp::<Building>(post).is_none() {
        return;
    }
    seam(world, id, post, Some(role), at, k);
}
