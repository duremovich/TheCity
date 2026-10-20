//! Lives at work phase 7: district Stations (docs/LIVES_AT_WORK.md § 5.2,
//! D11). `[law] stations` under the `[lives]` master (`[law] stations = false`).
//!
//! A Station is the city's: a booking desk and `station_cells` cells on a
//! district's Lot ([`seed`]: the farthest districts from the Precinct first,
//! each at least `station_min_tiles` from the Precinct's door and from every
//! Station already placed). A city guard is posted ([`hire_post`]) to the
//! post nearest his Home among the Stations under their share (`guard_count
//! × residents nearest the Station ÷ residents`) and the Precinct, which
//! keeps the remainder: at seed ([`seed`] moves the seeded guards) and on
//! `law::reconcile_guards`'s hires. His post is the `Jail` key's building
//! ([`jail_for`]); `station_desk_share` of a Station's guards hold its desk
//! a shift (`law::jail_duty`), the rest patrol, its districts first
//! (`law_brain::allocate`).
//!
//! The escort books at the nearest of the Precinct and the Stations with a
//! free cell ([`escort_target`]); an arrest whose nearest Station is full is
//! counted (`stations_full`). At a Station a fine or a sentence that ends
//! before the next van is served there; at `transfer_hour` the van takes
//! every held body whose sentence runs past the next van to the Precinct,
//! bodiless ([`van`]), the `Sentence` unchanged.
//!
//! The Statistical tier has no escort: its arrests are `p_theft_caught`,
//! re-learned at the gate's regen (§ 6, D14). No stream draw anywhere.

use crate::components::TilePos;
use crate::components::{Building, BuildingKind, Household, Job, Position, Role, Sentence};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::TICKS_PER_DAY;
use crate::world::World;

/// The switch: `[lives]` on and `[law] stations` on.
pub fn on(world: &World) -> bool {
    world.config.lives.enabled && world.config.law.stations
}

/// The standing Stations, ascending (none off).
pub fn stations(world: &World) -> Vec<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Station)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .collect()
}

/// Is `b` a standing Station?
pub fn is_station(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Station && !bd.demolished)
}

/// A city guard's Station post (`Job.employer` a Station), if any.
pub fn station_post(world: &World, agent: EntityId) -> Option<EntityId> {
    if world.buildings_of_kind(BuildingKind::Station).is_empty() {
        return None;
    }
    world
        .comp::<Job>(agent)
        .filter(|j| j.role == Role::Guard)
        .and_then(|j| j.employer)
        .filter(|&e| is_station(world, e))
}

/// The held in `b`'s cells (sentenced bodies inside it).
pub fn held_at(world: &World, b: EntityId) -> usize {
    world.sentenced().iter().filter(|&&p| world.comp::<Position>(p).is_some_and(|pos| pos.building == Some(b))).count()
}

/// The Precinct's prisoners: every `Sentence` but a Station's held (all of
/// them with no Station standing).
pub fn precinct_held(world: &World) -> usize {
    let st = world.buildings_of_kind(BuildingKind::Station);
    if st.is_empty() {
        return world.sentenced().len();
    }
    world
        .sentenced()
        .iter()
        .filter(|&&p| world.comp::<Position>(p).and_then(|pos| pos.building).is_none_or(|b| !st.contains(&b)))
        .count()
}

/// Review fix 5: the Precinct's room as every booking reads it: its
/// prisoners ([`precinct_held`]) and the Stations' held whose term runs
/// past the next van (the van's own rule: they are the Precinct's by
/// morning). With no Station standing, `precinct_held`.
pub fn precinct_bound(world: &World) -> usize {
    let st = world.buildings_of_kind(BuildingKind::Station);
    if st.is_empty() {
        return world.sentenced().len();
    }
    let van = next_van(world) + TICKS_PER_DAY;
    world
        .sentenced()
        .iter()
        .filter(|&&p| {
            let at_station = world.comp::<Position>(p).and_then(|pos| pos.building).is_some_and(|b| st.contains(&b));
            !at_station || world.comp::<Sentence>(p).is_some_and(|s| s.until_tick > van)
        })
        .count()
}

/// A Station with a free cell.
pub fn has_cell(world: &World, s: EntityId) -> bool {
    held_at(world, s) < world.config.law.station_cells as usize
}

fn door(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

/// The `Jail` key's building for `agent`: an escort's cells, a Station
/// guard's post, else the Precinct.
pub fn jail_for(world: &World, agent: EntityId) -> Option<EntityId> {
    let precinct = world.building_of_kind(BuildingKind::Jail);
    if world.buildings_of_kind(BuildingKind::Station).is_empty() {
        return precinct;
    }
    if world.comp::<crate::components::Brain>(agent).is_some_and(|b| b.escorting.is_some()) {
        return escort_target(world, agent).or(precinct);
    }
    station_post(world, agent).or(precinct)
}

/// The nearest (door Manhattan from the guard's tile, ties the lower id) of
/// the Precinct and the Stations with a free cell.
pub fn escort_target(world: &World, guard: EntityId) -> Option<EntityId> {
    let from = world.comp::<Position>(guard)?.tile;
    let precinct = world.building_of_kind(BuildingKind::Jail);
    stations(world)
        .into_iter()
        .filter(|&s| has_cell(world, s))
        .chain(precinct)
        .filter_map(|b| door(world, b).map(|d| (d.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// At an arrest: the nearest post to the suspect is a Station with its
/// cells full (the escort will be sent on): `stations_full`.
pub fn on_arrest(world: &mut World, guard: EntityId) {
    if !on(world) {
        return;
    }
    let Some(from) = world.comp::<Position>(guard).map(|p| p.tile) else { return };
    let nearest = stations(world)
        .into_iter()
        .chain(world.building_of_kind(BuildingKind::Jail))
        .filter_map(|b| door(world, b).map(|d| (d.manhattan(from), b)))
        .min()
        .map(|(_, b)| b);
    if nearest.is_some_and(|s| is_station(world, s) && !has_cell(world, s)) {
        crate::systems::ownership::jobs2_row(world).stations_full += 1;
    }
}

/// Every post (the Stations, then the Precinct) with its door and its
/// share of `want` guards: `want × residents whose district's centroid is
/// nearest it ÷ residents` (floored) for a Station; the Precinct none
/// (`usize::MAX`: the remainder).
pub fn shares(world: &World, want: usize) -> Vec<(EntityId, TilePos, usize)> {
    let mut posts: Vec<(EntityId, TilePos)> =
        stations(world).into_iter().filter_map(|s| door(world, s).map(|d| (s, d))).collect();
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return Vec::new() };
    let Some(pd) = door(world, precinct) else { return Vec::new() };
    posts.push((precinct, pd));
    // Residents by district (their Home's), at seed before any aggregate.
    let mut by_d = vec![0usize; world.districts.len()];
    // scan-ok: seed and a hiring day: the post shares
    for a in world.citizens() {
        if let Some(h) = world.comp::<Household>(a).and_then(|h| h.home) {
            let d = world.district_of_building(h).index();
            if let Some(n) = by_d.get_mut(d) {
                *n += 1;
            }
        }
    }
    let total: usize = by_d.iter().sum();
    let mut covered = vec![0usize; posts.len()];
    for (i, d) in world.districts.iter().enumerate() {
        let c = d.centroid;
        if let Some((k, _)) = posts.iter().enumerate().min_by_key(|(_, &(b, door))| (door.manhattan(c), b)) {
            covered[k] += by_d[i];
        }
    }
    let last = posts.len() - 1;
    posts
        .into_iter()
        .enumerate()
        .map(|(k, (b, d))| {
            let share = if k == last { usize::MAX } else { (want * covered[k]).checked_div(total).unwrap_or(0) };
            (b, d, share)
        })
        .collect()
}

/// The city guards posted at each of `posts` (by `Job.employer`).
fn posted(world: &World, posts: &[(EntityId, TilePos, usize)]) -> Vec<usize> {
    let guards = crate::systems::law_brain::guards(world);
    posts
        .iter()
        .map(|&(b, _, _)| guards.iter().filter(|&&g| world.comp::<Job>(g).and_then(|j| j.employer) == Some(b)).count())
        .collect()
}

/// The post for a guard living at `home_door`: the nearest of the posts
/// under their share (the Precinct always); ties the lower id. Counts the
/// hire in `have`.
pub fn hire_post(
    posts: &[(EntityId, TilePos, usize)],
    have: &mut [usize],
    home_door: Option<TilePos>,
) -> Option<EntityId> {
    let k = posts
        .iter()
        .enumerate()
        .filter(|&(k, &(_, _, share))| have[k] < share)
        .map(|(k, &(b, d, _))| (home_door.map_or(u32::MAX, |h| h.manhattan(d)), b, k))
        .min()
        .map(|(_, _, k)| k)?;
    have[k] += 1;
    Some(posts[k].0)
}

/// `law::reconcile_guards`'s hire under the switch: the post for `id`, and
/// the posts' counts the caller keeps across its hires.
pub struct Posts {
    pub posts: Vec<(EntityId, TilePos, usize)>,
    pub have: Vec<usize>,
}

impl Posts {
    pub fn new(world: &World, want: usize) -> Option<Posts> {
        if !on(world) || world.buildings_of_kind(BuildingKind::Station).is_empty() {
            return None;
        }
        let posts = shares(world, want);
        if posts.is_empty() {
            return None;
        }
        let have = posted(world, &posts);
        Some(Posts { posts, have })
    }

    pub fn pick(&mut self, world: &World, id: EntityId) -> Option<EntityId> {
        let home = world.comp::<Household>(id).and_then(|h| h.home).and_then(|h| door(world, h));
        hire_post(&self.posts, &mut self.have, home)
    }
}

/// Move a guard's post (the Job's employer and its index).
fn repost(world: &mut World, guard: EntityId, post: EntityId) {
    if let Some(j) = world.comp_mut::<Job>(guard) {
        if j.employer == Some(post) {
            return;
        }
        j.employer = Some(post);
        j.duty_fixed = None;
    }
    world.index_job(guard);
}

/// The vacant Lot a district's Station would stand on: a Civic or Mid door
/// first, then any; an interior of `station_cells` tiles; nearest the
/// district's centroid; the lower id.
fn district_lot(world: &World, d: usize) -> Option<EntityId> {
    let need = world.config.law.station_cells as usize;
    let centroid = world.districts.get(d)?.centroid;
    crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter(|&l| world.district_of_building(l).index() == d)
        .filter_map(|l| {
            let bd = world.comp::<Building>(l)?;
            let interior = usize::from(bd.rect.w.saturating_sub(2)) * usize::from(bd.rect.h.saturating_sub(2));
            if interior < need {
                return None;
            }
            let zone = world.map.zone(bd.door);
            let off = !matches!(zone, crate::components::Zone::Civic | crate::components::Zone::Mid);
            Some((off, bd.door.manhattan(centroid), l))
        })
        .min()
        .map(|(_, _, l)| l)
}

/// `World::new`, after the seeded venues and Schools: the Stations on their
/// Lots (the farthest districts first; each `station_min_tiles` or more
/// from the Precinct's door and from every Station placed), a `Founded`
/// event each naming its walk, then the seeded city guards posted
/// ([`Posts`], ascending id). No RNG, no coins. A no-op off.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return };
    let Some(pd) = door(world, precinct) else { return };
    let min = world.config.law.station_min_tiles;
    let mut cands: Vec<(std::cmp::Reverse<u32>, usize, EntityId, TilePos)> = (0..world.districts.len())
        .filter_map(|d| {
            let lot = district_lot(world, d)?;
            let ld = door(world, lot)?;
            Some((std::cmp::Reverse(ld.manhattan(pd)), d, lot, ld))
        })
        .collect();
    cands.sort();
    let mut placed: Vec<TilePos> = Vec::new();
    for (std::cmp::Reverse(dist), d, lot, ld) in cands {
        if dist < min || placed.iter().any(|p| p.manhattan(ld) < min) {
            continue;
        }
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Station, None).is_err() {
            continue;
        }
        placed.push(ld);
        let at = world.districts.get(d).map_or("?", |x| x.name.as_str());
        let text =
            format!("the city opened a Station in {at} (seeded; Lot {}; {dist} tiles from the Precinct)", lot.index);
        world.push_event(EventKind::Founded, &[lot], text);
    }
    let want = usize::from(world.levers.guard_count) + usize::from(world.outside.state_guards);
    let Some(mut posts) = Posts::new(world, want) else { return };
    // Every seeded guard is dealt afresh, in id order.
    for h in posts.have.iter_mut() {
        *h = 0;
    }
    for g in crate::systems::law_brain::guards(world) {
        if let Some(p) = posts.pick(world, g) {
            repost(world, g, p);
        }
    }
}

/// The tick of the next van (`transfer_hour`, strictly after now).
pub fn next_van(world: &World) -> crate::time::Tick {
    let day = world.tick - world.tick % TICKS_PER_DAY;
    let at = day + crate::time::Tick::from(world.config.law.transfer_hour);
    if at > world.tick {
        at
    } else {
        at + TICKS_PER_DAY
    }
}

/// Hourly, at `transfer_hour`: every held body in a Station whose sentence
/// runs past the next van goes to the Precinct, bodiless (`Position` set to
/// the Precinct, no walk), the `Sentence` unchanged; a `Transferred` event
/// and `transfers` each.
pub fn van(world: &mut World) {
    if !on(world) || world.tick_of_day() != world.config.law.transfer_hour {
        return;
    }
    let st = stations(world);
    if st.is_empty() {
        return;
    }
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return };
    let now = world.tick;
    // Review fix 5: the van fills the Precinct to its capacity; the rest
    // wait in the Station's cells for the next van.
    let capacity = usize::from(world.config.buildings.jail.capacity);
    let mut room = capacity.saturating_sub(precinct_held(world));
    let due: Vec<(EntityId, EntityId)> = world
        .sentenced()
        .iter()
        .copied()
        .filter(|&p| world.comp::<Sentence>(p).is_some_and(|s| s.until_tick > now + TICKS_PER_DAY))
        .filter_map(|p| {
            world.comp::<Position>(p).and_then(|pos| pos.building).filter(|b| st.contains(b)).map(|b| (p, b))
        })
        .collect();
    for (p, s) in due {
        if room == 0 {
            break;
        }
        room -= 1;
        world.remove_from_building(p);
        world.enter_building(p, precinct);
        let text = format!(
            "{} taken from the {} in {} to the Precinct",
            world.name_of(p),
            BuildingKind::Station.label(),
            world.district_name(world.district_of_building(s))
        );
        world.push_event(EventKind::Transferred, &[p, s, precinct], text);
        crate::systems::ownership::jobs2_row(world).transfers += 1;
    }
}

/// The day's end: the Stations standing (a daily count).
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let n = stations(world).len() as u32;
    crate::systems::ownership::jobs2_row(world).stations = n;
}

/// `law::allocate`'s deal under the switch: the patrol guards in `patrol`
/// (ascending) to the districts' `counts`, each Station guard to the
/// districts whose centroid is nearest his Station first (the nearest of
/// them with a slot left), then everyone left, in id order, to the district
/// with a slot left whose centroid is nearest his Home (his post without
/// one; ties the lower index: a patrol walks from home to the beat; the off deal filled the slots district by district in id order,
/// which sent Precinct guards across the island).
pub fn deal_beats(world: &World, patrol: &[EntityId], counts: &[u16]) -> Option<Vec<(EntityId, usize)>> {
    if !on(world) {
        return None;
    }
    let st = stations(world);
    if st.is_empty() {
        return None;
    }
    let precinct = world.building_of_kind(BuildingKind::Jail);
    let posts: Vec<(EntityId, TilePos)> =
        st.iter().copied().chain(precinct).filter_map(|b| door(world, b).map(|d| (b, d))).collect();
    // Each district's post (the nearest to its centroid) and walk.
    let owner: Vec<(EntityId, u32)> = world
        .districts
        .iter()
        .map(|d| {
            posts
                .iter()
                .map(|&(b, door)| (door.manhattan(d.centroid), b))
                .min()
                .map_or((EntityId::NONE, 0), |(t, b)| (b, t))
        })
        .collect();
    let mut left: Vec<u16> = counts.to_vec();
    let mut out: Vec<(EntityId, usize)> = Vec::with_capacity(patrol.len());
    let mut rest: Vec<EntityId> = Vec::new();
    for &g in patrol {
        let Some(s) = station_post(world, g) else {
            rest.push(g);
            continue;
        };
        let pick = owner
            .iter()
            .enumerate()
            .filter(|&(i, &(b, _))| b == s && left.get(i).is_some_and(|&n| n > 0))
            .min_by_key(|&(i, &(_, t))| (t, i))
            .map(|(i, _)| i);
        match pick {
            Some(i) => {
                left[i] -= 1;
                out.push((g, i));
            }
            None => rest.push(g),
        }
    }
    for g in rest {
        let from = world
            .comp::<Household>(g)
            .and_then(|h| h.home)
            .or_else(|| station_post(world, g).or(precinct))
            .and_then(|b| door(world, b));
        let pick = world
            .districts
            .iter()
            .enumerate()
            .filter(|&(i, _)| left.get(i).is_some_and(|&n| n > 0))
            .min_by_key(|&(i, d)| (from.map_or(0, |f| f.manhattan(d.centroid)), i))
            .map(|(i, _)| i);
        let Some(i) = pick else { break };
        left[i] -= 1;
        out.push((g, i));
    }
    Some(out)
}
