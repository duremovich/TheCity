//! Pressure pass round 4, T5 (docs/PRESSURE_PASS.md § T5; OPEN-217,
//! OPEN-228's follow-up): the jobless look for work. Under `[search]` (and
//! `[jobs2] street`); absent, every entry point is a no-op.
//!
//! - **Who searches**: a free jobless adult (no exec, not retired, not
//!   inside round 2 F3's idle days after a quit or a firing,
//!   `Misery.idle_until`) for `search_days` after his last post
//!   (`Jobs2State.idle_since`; a post lost since the last midnight is day 0),
//!   and any jobless body the shape-up sent home today (T4's passed-over).
//! - **Asking Friends** (a body, the search day's first hour inside
//!   `hours_from..hours_to`): his `asks` closest Friends and kin are asked;
//!   one employed where a post he can take is open (`demography::can_take`)
//!   within `lead_ticks` gives the word of it, the nearest first ("X told Y
//!   of a Packer post at Z"). "Nobody knew of an opening" is told once a
//!   jobless spell (`Misery.search_spell`).
//! - **The doors** (the Pitch goal's search branch, `streetwork`): he walks
//!   to the lead's door, else the nearest door with an open post he can
//!   take within `reach_ticks`, else the nearest hiring board
//!   (`board_kinds`: the Hall, a Market, the yards), and asks (`ask_ticks`,
//!   the `Vend` step). At a door with the post open and his, he is taken on
//!   (`demography::hire`, the vacancy closed before the Ambient pool's
//!   midnight); else the line says why. A board names the nearest open post
//!   he can take within `lead_ticks` of it (the next door), or nothing going.
//!   A door `crowd` searchers a post are making for is nobody's next. Up to
//!   `tries` doors a day.
//! - **Worse work**: from `worse_after_days` without a post he stands at
//!   the dawn shape-up whatever his purse while he searches
//!   (`streetwork::corner_ok`), and the midnight job search hires him to a
//!   post past `[distance.choice] hire_max_ticks` up to `far_ticks`
//!   (`demography::job_search_round`; the far hire's move-in is
//!   `[commute_pressure]`'s), before the Ambient pool takes it.
//! - **The day labourer's day**: a hand picked at dawn works from
//!   `day_start_min` after `[jobs2] corner_hour` to the day shift's end
//!   (`streetwork::dawn_pick`), not three hours' Rest at the Yard.
//!
//! Statistical mirror: no walk and no door. An off-screen searcher is the
//! midnight job search's candidate as before, with the far reach from
//! `worse_after_days`, and the shape-up's queue on `p_corner` whatever his
//! purse while he searches. No child is touched (`demography::is_adult`,
//! `ages::is_protected` in `can_take`). No random draw.

use crate::components::{Brain, Building, Job, Lod, Misery, Position, RelKind, Role, SearchDay, Sentence, TilePos};
use crate::config::SearchCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership;
use crate::time::Tick;
use crate::utility::curves::{urgency, Curve, GATE, IDENTITY};
use crate::utility::Consideration;
use crate::world::World;

/// `[search]`, under the street's switch; `None` off.
pub fn cfg(world: &World) -> Option<&SearchCfg> {
    world.config.search.as_ref().filter(|_| crate::systems::streetwork::on(world))
}

/// Days since `id`'s last post (`Jobs2State.idle_since`; a post lost
/// since the last midnight: 0).
pub fn idle_days(world: &World, id: EntityId) -> u64 {
    let today = world.day();
    world.jobs2.idle_since.get(&id).map_or(0, |&d| today.saturating_sub(d))
}

/// A free jobless adult who may look for work: no post, not held, not
/// leaving, no exec, not retired, not inside F3's idle days, and in need of
/// a wage (`Needs.wealth` under 1: the secure quit's line, `fixes::daily`).
fn jobless(world: &World, id: EntityId) -> bool {
    if world.has::<Job>(id)
        || world.has::<Sentence>(id)
        || world.has::<crate::components::Retired>(id)
        || world.comp::<Misery>(id).is_some_and(|m| m.idle_until > world.tick)
        || world.comp::<crate::components::Needs>(id).is_some_and(|n| n.wealth >= 1.0)
    {
        return false;
    }
    // The midnight pass's free jobless adults (no exec among them).
    if world.jobs2.idle_since.contains_key(&id) {
        return true;
    }
    crate::systems::demography::is_adult(world, id)
        && world.comp::<Brain>(id).is_some_and(|b| !b.emigrating)
        && !crate::systems::founding::is_exec(world, id)
}

/// Today's search day of `id`, if one was begun.
fn today_of(world: &World, id: EntityId) -> Option<&SearchDay> {
    world.comp::<Misery>(id).and_then(|m| m.search.as_ref()).filter(|s| s.day == world.day())
}

/// Today's search day of `id`, begun now if it was not.
fn today_mut(world: &mut World, id: EntityId) -> Option<&mut SearchDay> {
    let day = world.day();
    if !world.has::<Misery>(id) {
        world.insert(id, Misery::default());
    }
    let m = world.comp_mut::<Misery>(id)?;
    if m.search.as_ref().is_none_or(|s| s.day != day) {
        m.search = Some(SearchDay { day, ..SearchDay::default() });
    }
    m.search.as_mut()
}

/// `id` looks for work today: jobless, inside `search_days` of his last
/// post, or sent home from the shape-up this morning.
pub fn searching(world: &World, id: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    jobless(world, id) && (idle_days(world, id) < c.search_days || today_of(world, id).is_some_and(|s| s.sent_home))
}

/// `id` takes worse work: jobless `worse_after_days` or more.
pub fn worse_work(world: &World, id: EntityId) -> bool {
    cfg(world).is_some_and(|c| idle_days(world, id) >= c.worse_after_days) && jobless(world, id)
}

/// The dawn shape-up whatever his purse: he searches and takes worse work
/// (`streetwork::corner_ok`).
pub fn at_shape_up(world: &World, id: EntityId) -> bool {
    searching(world, id) && worse_work(world, id)
}

/// The midnight job search's far post (`demography::job_search_round`):
/// `id` takes worse work and the post's door is within `far_ticks` of
/// `from` (the structural trip, as the hire's cap).
pub fn takes_far(world: &World, id: EntityId, from: TilePos, door: TilePos) -> bool {
    cfg(world).is_some_and(|c| crate::systems::distance::travel(world, from, door) <= c.far_ticks)
        && worse_work(world, id)
}

/// A shape-up body sent home (`streetwork::shape_up_told`): a search day
/// whatever his idle days (T4: the passed-over had no sequel).
pub fn note_sent_home(world: &mut World, id: EntityId) {
    if cfg(world).is_none() {
        return;
    }
    if let Some(s) = today_mut(world, id) {
        s.sent_home = true;
    }
}

fn in_hours(world: &World, c: &SearchCfg) -> bool {
    let tod = world.tick_of_day();
    tod >= c.hours_from && tod < c.hours_to
}

/// A body who walks the search (a Pitch holder stands his own).
fn body(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical) && !world.jobs2.pitches.contains_key(&id)
}

/// The open role at `b` that `id` can take (the first in its list).
fn open_role(world: &World, id: EntityId, b: EntityId) -> Option<Role> {
    if !world.comp::<Building>(b).is_some_and(|bd| !bd.demolished) {
        return None;
    }
    world.vacancies.get(&b)?.iter().copied().find(|&r| crate::systems::demography::can_take(world, id, b, r))
}

/// The door `id` is making for today (the ask settles whether the post is
/// still open).
pub fn board(world: &World, id: EntityId) -> Option<EntityId> {
    let c = cfg(world)?;
    if !in_hours(world, c) || !body(world, id) || !searching(world, id) {
        return None;
    }
    today_of(world, id).and_then(|s| s.board)
}

/// The street tile outside `id`'s door to ask at (the Pitch goal's spot).
pub fn board_tile(world: &World, id: EntityId) -> Option<TilePos> {
    let b = board(world, id)?;
    world.comp::<Building>(b).map(|bd| world.outside_door(bd))
}

fn from_of(world: &World, id: EntityId) -> Option<TilePos> {
    world.comp::<Position>(id).map(|p| p.tile)
}

/// The nearest door with an open post `id` can take within `cap` of
/// `from`, not asked today (`skip`), among `among` (every open post's
/// building when `None`).
fn nearest_door(
    world: &World,
    id: EntityId,
    from: TilePos,
    cap: Tick,
    skip: &[EntityId],
    among: Option<&[EntityId]>,
) -> Option<(Tick, EntityId)> {
    let all: Vec<EntityId>;
    let list: &[EntityId] = match among {
        Some(l) => l,
        None => {
            all = world.vacancies.keys().copied().collect();
            &all
        }
    };
    list.iter()
        .copied()
        .filter(|b| !skip.contains(b))
        .filter_map(|b| {
            let door = world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.door)?;
            // The walk is no shorter than the tiles; the El at most twice as quick.
            if u64::from(from.manhattan(door)) > 2 * cap {
                return None;
            }
            open_role(world, id, b)?;
            let t = crate::systems::distance::travel(world, from, door);
            (t <= cap).then_some((t, b))
        })
        .min()
}

/// The nearest standing building of `board_kinds` within `reach_ticks` of
/// `from`, not asked today (`skip`).
fn nearest_board(world: &World, from: TilePos, c: &SearchCfg, skip: &[EntityId]) -> Option<EntityId> {
    c.board_kinds
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|b| !skip.contains(b))
        .filter_map(|b| {
            let door = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict).map(|bd| bd.door)?;
            if u64::from(from.manhattan(door)) > 2 * c.reach_ticks {
                return None;
            }
            let t = crate::systems::distance::travel(world, from, door);
            (t <= c.reach_ticks).then_some((t, b))
        })
        .min()
        .map(|(_, b)| b)
}

/// The doors `crowd` searching bodies a post open there are making for
/// already (the board's naming reads them as the hourly pass does).
fn crowded_doors(world: &World, c: &SearchCfg) -> Vec<EntityId> {
    let mut heading: std::collections::BTreeMap<EntityId, u32> = Default::default();
    for id in world.bodies() {
        if let Some(b) = today_of(world, id).and_then(|s| s.board) {
            *heading.entry(b).or_insert(0) += 1;
        }
    }
    heading.into_iter().filter(|&(b, n)| crowded_at(world, c, b, n)).map(|(b, _)| b).collect()
}

/// `n` searchers making for `b` fill its open posts' crowd (a board with no
/// post of its own is never crowded).
fn crowded_at(world: &World, c: &SearchCfg, b: EntityId, n: u32) -> bool {
    world.vacancies.get(&b).is_some_and(|v| n >= u32::from(c.crowd) * v.len() as u32)
}

fn is_board(world: &World, c: &SearchCfg, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| c.board_kinds.contains(&bd.kind))
}

/// Hourly in the search hours (from `streetwork::hourly`): each searching
/// body asks his Friends on the day's first hour (one line) and is given
/// the next door to ask (a Friend's word first, then the nearest board),
/// up to `tries` a day.
pub fn hourly(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    if !in_hours(world, &c) {
        return;
    }
    let bodies: Vec<EntityId> =
        world.bodies().into_iter().filter(|&id| body(world, id) && searching(world, id)).collect();
    // The doors already made for (`crowd` searchers a post open there).
    let mut heading: std::collections::BTreeMap<EntityId, u32> = Default::default();
    for &id in &bodies {
        if let Some(b) = today_of(world, id).and_then(|s| s.board) {
            *heading.entry(b).or_insert(0) += 1;
        }
    }
    let crowded = |world: &World, heading: &std::collections::BTreeMap<EntityId, u32>| -> Vec<EntityId> {
        heading.iter().filter(|&(&b, &n)| crowded_at(world, &c, b, n)).map(|(&b, _)| b).collect()
    };
    for id in bodies {
        let Some(from) = from_of(world, id) else { continue };
        let asked = today_of(world, id).is_some_and(|s| s.asked);
        if !asked {
            let full = crowded(world, &heading);
            if let Some(b) = ask_friends(world, id, from, &c, &full) {
                *heading.entry(b).or_insert(0) += 1;
            }
        }
        let Some(s) = today_of(world, id) else { continue };
        if s.board.is_some() || s.tried.len() >= usize::from(c.tries) {
            continue;
        }
        let mut skip = s.tried.clone();
        skip.extend(crowded(world, &heading));
        // No open post within reach: the nearest hiring board.
        let next = nearest_door(world, id, from, c.reach_ticks, &skip, None)
            .map(|(_, b)| b)
            .or_else(|| nearest_board(world, from, &c, &skip));
        if let Some(b) = next {
            *heading.entry(b).or_insert(0) += 1;
        }
        if let Some(s) = today_mut(world, id) {
            s.board = next;
            s.lead = None;
        }
    }
}

/// The day's first hour: `id` asks his Friends and kin about work. One
/// employed where a post he can take is open within `lead_ticks` gives the
/// word (the nearest; the day's first door, returned).
fn ask_friends(world: &mut World, id: EntityId, from: TilePos, c: &SearchCfg, full: &[EntityId]) -> Option<EntityId> {
    let mut friends: Vec<(f32, EntityId)> = world
        .neighbours(id)
        .filter(|&o| o != id && crate::systems::law::living(world, o))
        .filter_map(|o| {
            world
                .edge(id, o)
                .filter(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse | RelKind::Parent))
                .map(|e| (e.affinity, o))
        })
        .collect();
    friends.sort_by(|p, q| q.0.total_cmp(&p.0).then(p.1.cmp(&q.1)));
    friends.truncate(usize::from(c.asks.max(1)));
    let asked: Vec<EntityId> = friends.iter().map(|&(_, f)| f).collect();
    // Each Friend's own workplace, the nearest open to him first.
    let mut lead: Option<(Tick, EntityId, EntityId)> = None;
    for &f in &asked {
        let Some(w) = world.comp::<Job>(f).and_then(|j| j.employer) else { continue };
        if let Some((t, b)) = nearest_door(world, id, from, c.lead_ticks, full, Some(&[w])) {
            if lead.is_none_or(|(lt, lb, _)| (t, b) < (lt, lb)) {
                lead = Some((t, b, f));
            }
        }
    }
    if let Some(s) = today_mut(world, id) {
        s.asked = true;
        if let Some((_, b, f)) = lead {
            s.board = Some(b);
            s.lead = Some(f);
        }
    }
    let row = ownership::jobs2_row(world);
    row.search_days += 1;
    row.search_leads += u32::from(lead.is_some());
    // "Nobody knew" is told once a jobless spell (the asked tell him when
    // they hear of one: a later day's line is a word of a post).
    let spell = world.jobs2.idle_since.get(&id).copied().unwrap_or_else(|| world.day());
    let told = world.comp::<Misery>(id).and_then(|m| m.search_spell) == Some(spell);
    if lead.is_none() && told {
        return None;
    }
    if lead.is_none() {
        if let Some(m) = world.comp_mut::<Misery>(id) {
            m.search_spell = Some(spell);
        }
    }
    let name = world.name_of(id);
    let cap = usize::from(c.friend_names.max(1));
    let named = names(world, &asked, cap);
    let text = match (lead, asked.is_empty()) {
        (_, true) => format!("{name} looked for work: no friend to ask"),
        (Some((_, b, f)), _) => {
            let role = open_role(world, id, b).map_or("a", |r| r.label());
            format!("{} told {name} of a {role} post at {}", world.name_of(f), world.name_of(b))
        }
        (None, _) => format!("{name} asked {named} about work: nobody knew of an opening"),
    };
    // His line (and the Friend's who gave the word), not every asked
    // Friend's diary.
    let mut actors = vec![id];
    actors.extend(lead.map(|(_, _, f)| f));
    world.push_event(EventKind::JobSearch, &actors, text);
    lead.map(|(_, b, _)| b)
}

/// Up to `cap` names, then "and N more".
fn names(world: &World, ids: &[EntityId], cap: usize) -> String {
    let mut out: Vec<String> = ids.iter().take(cap).map(|&a| world.name_of(a)).collect();
    let rest = ids.len().saturating_sub(cap);
    if rest > 0 {
        out.push(format!("{rest} more"));
    }
    match out.len() {
        0 => String::new(),
        1 => out.remove(0),
        _ => {
            let last = out.pop().unwrap_or_default();
            format!("{} and {last}", out.join(", "))
        }
    }
}

/// The Pitch goal's search branch (`streetwork::considerations`): a door
/// to ask at today.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    let c = cfg(world)?;
    board(world, id)?;
    let n = world.comp::<crate::components::Needs>(id)?;
    // The first days most eager (the search's want fades over `search_days`).
    let fresh = 1.0 - 0.5 * (idle_days(world, id) as f32 / c.search_days.max(1) as f32).min(1.0);
    Some((
        vec![
            Consideration::new("a door to ask at about work", 1.0, GATE),
            Consideration::new("days without a post", fresh, IDENTITY),
            Consideration::new("hunger", n.hunger, IDENTITY),
            Consideration::new("energy", n.energy, IDENTITY),
            Consideration::new("U(wealth)", urgency(n.wealth), Curve::Logistic { k: 4.0, mid: 0.2 }),
        ],
        0.0,
    ))
}

/// The ask's minutes at the door (`streetwork::vend_ticks`).
pub fn ask_ticks(world: &World, id: EntityId) -> Option<Tick> {
    let c = cfg(world)?;
    board(world, id)?;
    Some(Tick::from(c.ask_ticks.max(1)))
}

/// The ask settled (the `Vend` step's end at a door): the post open and
/// his, `id` is taken on there; else the line says why and the next door
/// waits for the hour.
pub fn on_ask(world: &mut World, id: EntityId) {
    let Some(b) = board(world, id) else { return };
    let lead = today_of(world, id).and_then(|s| s.lead);
    if let Some(s) = today_mut(world, id) {
        s.tried.push(b);
        s.board = None;
        s.lead = None;
    }
    ownership::jobs2_row(world).search_asks += 1;
    let (name, place) = (world.name_of(id), world.name_of(b));
    let told = lead.map(|f| format!(" ({} told of it)", world.name_of(f))).unwrap_or_default();
    match open_role(world, id, b) {
        Some(role) => {
            world.push_event(
                EventKind::JobSearch,
                &[id, b],
                format!("{name} asked at {place} about work{told} and was taken on as {}", role.label()),
            );
            crate::systems::demography::hire(world, id, b, role);
            crate::systems::demography::close_vacancy(world, id, b, role);
            ownership::jobs2_row(world).search_hires += 1;
        }
        None => {
            let c = cfg(world).cloned();
            if let Some(c) = c.filter(|c| is_board(world, c, b)) {
                // The hiring board: the nearest open post he can take within
                // `lead_ticks` of it is the next door.
                let from =
                    world.comp::<Building>(b).map_or_else(|| from_of(world, id).unwrap_or_default(), |bd| bd.door);
                let mut skip = today_of(world, id).map(|s| s.tried.clone()).unwrap_or_default();
                skip.extend(crowded_doors(world, &c));
                let next = nearest_door(world, id, from, c.lead_ticks, &skip, None).map(|(_, n)| n);
                let text = match next {
                    Some(n) => {
                        let role = open_role(world, id, n).map_or("a", |r| r.label());
                        format!("{name} read the board at {place}: a {role} post at {}", world.name_of(n))
                    }
                    None => format!("{name} read the board at {place}: nothing going"),
                };
                if let Some(s) = today_mut(world, id) {
                    s.board = next;
                }
                world.push_event(EventKind::JobSearch, &[id, b], text);
                return;
            }
            let why = if world.vacancies.contains_key(&b) { "turned away" } else { "the post was gone" };
            world.push_event(
                EventKind::JobSearch,
                &[id, b],
                format!("{name} asked at {place} about work{told}: {why}"),
            );
        }
    }
}

/// Round 4 T5 (OPEN-234): `id` takes the chair of `corp`: a post he holds
/// is let go, with a line (the post re-posted, `World::vacate_job`).
pub fn leave_post_for_chair(world: &mut World, id: EntityId, corp: EntityId) {
    if !world.has::<Job>(id) {
        return;
    }
    world.abort_plan(id);
    let Some(job) = world.vacate_job(id) else { return };
    let name = world.name_of(id);
    let at = job.employer.map(|e| format!(" at {}", world.name_of(e))).unwrap_or_default();
    let chair = world.owner_label(Some(corp));
    let mut actors = vec![id];
    actors.extend(job.employer);
    world.push_event(
        EventKind::JobSearch,
        &actors,
        format!("{name} gave up the post of {}{at} to take the chair of {chair}", job.role.label()),
    );
}
