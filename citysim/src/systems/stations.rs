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
//!
//! **The van** (the island pass phase 7, I3, OPEN-166; `[motion.vans]`):
//! the 04:00 transfer is a van run, not a write of the body to the
//! Precinct. Each Station's due prisoners (the same pick and the same room
//! rule) board a van at its door, `capacity` a van, and ride the road to
//! the Precinct's door over the Car's drive time ([`VanRun`],
//! `motion::spawn_van`, no body at the wheel: the city's driver, off
//! screen). Full and Coarse prisoners are aboard (`motion`'s cuffed-rider
//! rule keeps a sentenced rider on a run), their position the van's; at the
//! arrival ([`van_tick`]) each steps off at the door and is booked into the
//! Precinct. **Statistical mirror:** a Statistical prisoner rides the same
//! run bodiless (out of the Station's cells at the departure, into the
//! Precinct at the van's arrival: the same duration). A promotion mid-ride
//! puts his body aboard ([`on_promoted`]); a demotion steps him off (the
//! run carries him on bodiless). The `Transferred` event and `transfers`
//! are at the departure, as before.

use serde::{Deserialize, Serialize};

use crate::components::TilePos;
use crate::components::{Brain, Building, BuildingKind, Household, Job, Lod, Position, Role, Sentence};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// The island pass phase 7: a prisoner transfer on the road, keyed by its
/// van (`World::van_runs`; the van's `Motion` is `RideKind::Van`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VanRun {
    /// The Station it left.
    pub from: EntityId,
    /// The Precinct.
    pub to: EntityId,
    /// The van's arrival at the Precinct's door.
    pub arrive: Tick,
    /// Every prisoner on the run (aboard, or bodiless off screen).
    pub prisoners: Vec<EntityId>,
}

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
    let vans = crate::systems::motion::vans_on(world);
    let mut runs: Vec<(EntityId, Vec<EntityId>)> = Vec::new();
    for (p, s) in due {
        if room == 0 {
            break;
        }
        room -= 1;
        if vans {
            // The island pass phase 7: on the Station's van, not written
            // to the Precinct.
            match runs.iter_mut().find(|(st, _)| *st == s) {
                Some((_, ps)) => ps.push(p),
                None => runs.push((s, vec![p])),
            }
        } else {
            world.remove_from_building(p);
            world.enter_building(p, precinct);
        }
        let text = format!(
            "{} taken from the {} in {} to the Precinct",
            world.name_of(p),
            BuildingKind::Station.label(),
            world.district_name(world.district_of_building(s))
        );
        world.push_event(EventKind::Transferred, &[p, s, precinct], text);
        crate::systems::ownership::jobs2_row(world).transfers += 1;
    }
    for (s, ps) in runs {
        send_vans(world, s, precinct, ps);
    }
}

/// The island pass phase 7: the Station `s`'s due prisoners `ps` set out
/// for the Precinct, `[motion.vans] capacity` a van, in the van's order.
fn send_vans(world: &mut World, s: EntityId, precinct: EntityId, ps: Vec<EntityId>) {
    let Some(cap) = world.config.motion.as_ref().and_then(|m| m.vans.as_ref()).map(|v| v.capacity.max(1) as usize)
    else {
        return;
    };
    let (Some(sb), Some(to)) = (world.comp::<Building>(s), door(world, precinct)) else { return };
    let from = world.outside_door(sb);
    let now = world.tick;
    let road = crate::systems::motion::road_len(world, from, to).max(from.manhattan(to));
    let arrive = now + crate::systems::vehicles::road_ticks(world, road);
    for load in ps.chunks(cap) {
        let v = crate::systems::motion::spawn_van(world, from, to, now, arrive);
        for &p in load {
            let body = world.comp::<Brain>(p).is_some_and(|b| b.lod != Lod::Statistical);
            if body {
                crate::systems::motion::join(world, p, v);
            } else {
                world.leave_building(p);
            }
        }
        let run = VanRun { from: s, to: precinct, arrive, prisoners: load.to_vec() };
        world.van_runs.insert(v, run);
    }
}

/// The island pass phase 7: a suspect escorted to Station `s` and booked to
/// the Precinct (its cells filled while the escort drove) rides a van on
/// from the Station's door, his `Sentence` the Precinct's.
pub fn send_on(world: &mut World, s: EntityId, suspect: EntityId) {
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return };
    let here = world.comp::<Position>(suspect).and_then(|p| p.building);
    if here != Some(precinct) || !world.has::<Sentence>(suspect) || !world.is_alive(suspect) {
        return;
    }
    let Some(from) = world.comp::<Building>(s).map(|b| world.outside_door(b)) else { return };
    world.remove_from_building(suspect);
    if let Some(p) = world.comp_mut::<Position>(suspect) {
        p.tile = from;
        p.building = None;
    }
    send_vans(world, s, precinct, vec![suspect]);
}

/// Every tick (`law::run`): a transfer van at the Precinct's door sets its
/// prisoners down and they are booked in (the Statistical ones with them);
/// the van's motion ends. A prisoner no longer sentenced (or dead) is off
/// the run where he is.
pub fn van_tick(world: &mut World) {
    if world.van_runs.is_empty() {
        return;
    }
    let now = world.tick;
    let done: Vec<EntityId> = world.van_runs.iter().filter(|(_, r)| r.arrive <= now).map(|(&v, _)| v).collect();
    for v in done {
        let Some(run) = world.van_runs.remove(&v) else { continue };
        for p in run.prisoners {
            if crate::systems::motion::vehicle_of(world, p) == Some(v) {
                crate::systems::motion::alight_now(world, p);
            }
            if !world.is_alive(p) || !world.has::<Sentence>(p) {
                continue;
            }
            // Off the run already and inside somewhere else: left there.
            if world.comp::<Position>(p).is_some_and(|pos| pos.building.is_some()) {
                continue;
            }
            world.enter_building(p, run.to);
        }
        crate::systems::motion::finish(world, v);
    }
}

/// `lod::set_lod`, a promotion out of the Statistical tier: a prisoner on a
/// transfer run gets his body aboard its van.
pub fn on_promoted(world: &mut World, id: EntityId) {
    if world.van_runs.is_empty() || !world.has::<Sentence>(id) {
        return;
    }
    let Some(v) = world.van_runs.iter().find(|(_, r)| r.prisoners.contains(&id)).map(|(&v, _)| v) else { return };
    if world.motions.contains_key(&v) && crate::systems::motion::vehicle_of(world, id).is_none() {
        crate::systems::motion::join(world, id, v);
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
/// districts whose centroid is nearest his Station first, then everyone
/// left to any district with a slot left (the off deal filled the slots
/// district by district in id order, which sent Precinct guards across the
/// island).
///
/// The pressure pass R2 (`OPEN-215`, F1's leftover): both rounds deal by
/// the guard's Home (his post without one), as the patrol walks from home
/// to the beat: a district's nearness is the Manhattan from the Home door
/// to its nearest Market door (a district with no Market: its nearest
/// Home door; neither: its centroid), and each round deals its (guard,
/// district) pairs nearest first (ties the lower guard, then the lower
/// index), not each guard in id order to his nearest district with a slot
/// left: the last guards dealt took whatever districts were left, across
/// the island (20k seed 42, unscaled: Patrol legs averaged 146 tiles, a third of
/// them into another district). With `[distance]` on, the rest are dealt
/// only within `job_cap_ticks` (walked) of the Home; a guard left over
/// takes his nearest district, slot or none (a far district's slots stay
/// empty, `law_brain::allocate` counting the beats dealt).
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
    // Each district's post (the nearest to its centroid).
    let owner: Vec<EntityId> = world
        .districts
        .iter()
        .map(|d| {
            posts.iter().map(|&(b, door)| (door.manhattan(d.centroid), b)).min().map_or(EntityId::NONE, |(_, b)| b)
        })
        .collect();
    // Each district's anchors: its Markets' doors, else its Homes', else
    // its centroid.
    let standing_doors = |ids: &[EntityId], kind: Option<BuildingKind>| -> Vec<TilePos> {
        ids.iter()
            .filter_map(|&b| world.comp::<Building>(b))
            .filter(|b| !b.demolished && kind.is_none_or(|k| b.kind == k))
            .map(|b| b.door)
            .collect()
    };
    let anchors: Vec<Vec<TilePos>> = world
        .districts
        .iter()
        .map(|d| {
            let m = standing_doors(&d.buildings, Some(BuildingKind::Market));
            if !m.is_empty() {
                return m;
            }
            let h = standing_doors(&d.homes, None);
            if h.is_empty() {
                vec![d.centroid]
            } else {
                h
            }
        })
        .collect();
    let home_of = |g: EntityId| {
        world
            .comp::<Household>(g)
            .and_then(|h| h.home)
            .or_else(|| station_post(world, g).or(precinct))
            .and_then(|b| door(world, b))
    };
    let near = |from: Option<TilePos>, i: usize| -> u32 {
        from.and_then(|f| anchors[i].iter().map(|a| a.manhattan(f)).min()).unwrap_or(0)
    };
    let mut left: Vec<u16> = counts.to_vec();
    let mut out: Vec<(EntityId, usize)> = Vec::with_capacity(patrol.len());
    // One round: the pairs nearest first, each guard dealt once.
    let deal = |guards: &[(EntityId, Option<TilePos>, Option<EntityId>)],
                reach: Option<u64>,
                left: &mut Vec<u16>,
                out: &mut Vec<(EntityId, usize)>|
     -> Vec<EntityId> {
        let mut pairs: Vec<(u32, usize, usize)> = Vec::new();
        for (gi, &(_, from, post)) in guards.iter().enumerate() {
            for (i, &own) in owner.iter().enumerate() {
                let t = near(from, i);
                if left.get(i).is_some_and(|&n| n > 0)
                    && post.is_none_or(|s| own == s)
                    && reach.is_none_or(|r| u64::from(t) <= r)
                {
                    pairs.push((t, gi, i));
                }
            }
        }
        pairs.sort_unstable();
        let mut dealt = vec![false; guards.len()];
        for (_, gi, i) in pairs {
            if dealt[gi] || left[i] == 0 {
                continue;
            }
            left[i] -= 1;
            dealt[gi] = true;
            out.push((guards[gi].0, i));
        }
        guards.iter().zip(dealt).filter(|(_, d)| !d).map(|(g, _)| g.0).collect()
    };
    let posted: Vec<(EntityId, Option<TilePos>, Option<EntityId>)> =
        patrol.iter().filter_map(|&g| station_post(world, g).map(|s| (g, home_of(g), Some(s)))).collect();
    let mut rest: Vec<EntityId> = patrol.iter().copied().filter(|&g| station_post(world, g).is_none()).collect();
    rest.extend(deal(&posted, None, &mut left, &mut out));
    rest.sort_unstable();
    let rest: Vec<(EntityId, Option<TilePos>, Option<EntityId>)> =
        rest.into_iter().map(|g| (g, home_of(g), None)).collect();
    // The rest: only within `[distance] job_cap_ticks` (walked) of the
    // Home, then each guard still undealt to his nearest district, slot
    // or none (a far district's empty slots stay empty: the island's
    // Precinct guards were dealt the Vats 700 tiles from home).
    let reach = crate::systems::distance::job_cap(world).map(|c| c / world.config.exec.move_ticks_full.max(1));
    let undealt = deal(&rest, reach, &mut left, &mut out);
    for g in undealt {
        let from = home_of(g);
        if let Some(i) = (0..world.districts.len()).min_by_key(|&i| (near(from, i), i)) {
            out.push((g, i));
        }
    }
    Some(out)
}
