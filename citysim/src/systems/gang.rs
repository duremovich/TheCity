//! The gangs: recruitment into one of several gangs, joining and leaving,
//! ranks and leaders, heat and shocks for the faction brain, extortion and
//! Home claims, loot splitting, fencing, the daily economy, betrayal.
//!
//! The brain that issues orders lives in `faction`; raids in `raid`.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Claim, DistrictId, Gang, GangMember, Household, Memory, MemoryKind, Needs, Order,
    Personality, Position, Sentence, Shock, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::systems::{faction, law, social};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// A claim at this count holds the Home: it is territory.
pub const CLAIM_HELD: u8 = 3;
/// `heat_log` keeps at most this many arrests and deaths.
pub const HEAT_LOG_CAP: usize = 64;

// ---------------------------------------------------------------------------
// Recruitment
// ---------------------------------------------------------------------------

/// Can `g` take recruits: not sacked, under `max_members`, and able to pay
/// a day's stipend for every member beyond the first `recruit_on_promise`,
/// the recruit included. The promise covers the core for good: a gang of
/// exactly `recruit_on_promise` sits at the stipend's break-even, and
/// asking its treasury to fund the whole roster left it there for ever.
fn recruiting(world: &World, g: &Gang) -> bool {
    let stipend = world.config.social.gang_stipend;
    let cfg = &world.config.gangs;
    let n = g.members.len();
    let funded = (n + 1).saturating_sub(cfg.recruit_on_promise) as i64;
    !g.is_sacked(world.tick) && n < cfg.max_members && g.treasury >= stipend * funded
}

/// The gang a non-member would join right now, if any: the gang of the member
/// they have the best qualifying edge to; a desperate (`hunger < 0.2 &&
/// lawfulness < 0.3`) or bootstrap (empty gang, WasArrested memory) recruit
/// joins the gang whose Hideout is nearest their Home (ties: lower index).
pub fn recruit_gang(world: &World, id: EntityId) -> Option<EntityId> {
    if world.has::<GangMember>(id) {
        return None;
    }
    let cfg = &world.config.social;
    let gangs = world.gangs();
    // The member with the best qualifying edge: the highest affinity, ties to
    // the first gang in `gangs()` then the first in its roster. Walks the
    // agent's own edges rather than every roster (M10 phase 5b).
    let open: Vec<bool> = gangs.iter().map(|&g| world.comp::<Gang>(g).is_some_and(|g| recruiting(world, g))).collect();
    let mut best: Option<(f32, usize, usize, EntityId)> = None;
    if open.contains(&true) {
        for m in world.neighbours(id) {
            let Some(gid) = world.comp::<GangMember>(m).map(|gm| gm.gang) else { continue };
            let Some(gi) = gangs.iter().position(|&g| g == gid) else { continue };
            if !open[gi] {
                continue;
            }
            let Some(e) = world.edge(id, m) else { continue };
            if e.affinity < cfg.join_gang_affinity {
                continue;
            }
            let Some(mi) = world.comp::<Gang>(gid).and_then(|g| g.members.iter().position(|&x| x == m)) else {
                continue;
            };
            let better = best.is_none_or(|(a, bg, bm, _)| e.affinity > a || (e.affinity == a && (gi, mi) < (bg, bm)));
            if better {
                best = Some((e.affinity, gi, mi, gid));
            }
        }
    }
    if let Some((_, _, _, gid)) = best {
        return Some(gid);
    }
    // M11 D36: a fresh, lawless evictee is desperate too (the spiral's first link).
    let desperate = (world.comp::<Needs>(id).is_some_and(|n| n.hunger < cfg.join_gang_desperation_hunger)
        && world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < cfg.join_gang_desperation_lawfulness))
        || crate::systems::classes::evicted_desperate(world, id);
    let arrested =
        world.comp::<Memory>(id).is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::WasArrested));
    if !desperate && !arrested {
        return None;
    }
    let from = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))?;
    gangs
        .into_iter()
        .filter_map(|gid| world.comp::<Gang>(gid).map(|g| (gid, g)))
        .filter(|(_, g)| !g.is_sacked(world.tick))
        .filter(|&(gid, _)| may_reform(world, gid))
        .filter(|(_, g)| {
            (desperate && recruiting(world, g))
                || (arrested && g.members.is_empty() && world.config.gangs.max_members > 0)
        })
        .filter_map(|(gid, g)| {
            world.comp::<Building>(g.hideout).map(|b| (b.door.manhattan(from), g.hideout.index, gid))
        })
        .min()
        .map(|(_, _, gid)| gid)
}

/// M12 D40: no instant hydra. A gang whose roster emptied after having
/// members takes a bootstrap recruit only once it has been empty
/// `reform_days`, its Hideout's district is not held by another gang, and
/// that district's coverage is below `reform_max_coverage`. A gang with
/// members, or one that never had any (a fresh city), is always open.
pub fn may_reform(world: &World, gang: EntityId) -> bool {
    use crate::components::Controller;
    let Some(g) = world.comp::<Gang>(gang) else { return false };
    if !g.emptied || !g.members.is_empty() {
        return true;
    }
    let cfg = &world.config.gangs;
    let waited = g.empty_since.is_none_or(|t| world.tick.saturating_sub(t) >= cfg.reform_days * TICKS_PER_DAY);
    let Some(hq) = world.hideout_of(gang) else { return waited };
    let d = world.district(world.district_of_building(hq));
    let taken = matches!(d.control, Controller::Gang(o) if o != gang);
    waited && !taken && d.coverage < cfg.reform_max_coverage
}

/// Eligibility for JoinGang: some gang would take them.
pub fn eligible(world: &World, id: EntityId) -> bool {
    recruit_gang(world, id).is_some()
}

/// The Hideout a `LocationKey::Hideout` means for this agent: their gang's,
/// or the gang they would join, or the first on the map.
pub fn hideout_for(world: &World, id: EntityId) -> Option<EntityId> {
    world
        .gang_of(id)
        .or_else(|| recruit_gang(world, id))
        .and_then(|g| world.hideout_of(g))
        .or_else(|| world.building_of_kind(BuildingKind::Hideout))
}

/// Is this member on tonight's watch? `min(night_watch, n / 2)` members
/// sleep at the Hideout each night so a raid meets someone: the roster in
/// joining order (newest first: the grunts stand watch), rotated by the day,
/// so the duty goes round. The night is keyed on the day it starts: a watch
/// that began at 22:00 is the same watch at 02:00.
pub fn on_watch(world: &World, id: EntityId) -> bool {
    let Some(g) = world.gang_of(id).and_then(|g| world.comp::<Gang>(g)) else { return false };
    // Jailed members cannot stand watch: only the free roster counts.
    let mut roster: Vec<(Tick, EntityId)> = g
        .members
        .iter()
        .filter(|&&m| !world.has::<Sentence>(m))
        .map(|&m| (world.comp::<GangMember>(m).map_or(0, |gm| gm.joined_tick), m))
        .collect();
    let n = roster.len();
    let watch = world.config.gangs.night_watch.min(n / 2);
    if watch == 0 {
        return false;
    }
    roster.sort_by(|a, b| b.cmp(a));
    let night = (world.tick + TICKS_PER_DAY / 2) / TICKS_PER_DAY;
    let start = (night % n as u64) as usize;
    (0..watch).any(|i| roster[(start + i) % n].1 == id)
}

/// The Hideout a member idles and sleeps in right now: their gang's, while
/// on watch, while the gang lies low, or when the member has no Home;
/// never when it is sacked or full (for anyone not already inside). `None`
/// for everyone else.
pub fn holes_up_at(world: &World, id: EntityId) -> Option<EntityId> {
    let gang = world.gang_of(id)?;
    let g = world.comp::<Gang>(gang)?;
    let homeless = world.comp::<Household>(id).is_none_or(|h| h.home.is_none());
    if g.is_sacked(world.tick) || !(g.order == Order::LieLow || homeless || on_watch(world, id)) {
        return None;
    }
    let b = world.comp::<Building>(g.hideout)?;
    (!b.is_full() || b.occupants.contains(&id)).then_some(g.hideout)
}

/// JoinGang at the Hideout: join the gang `recruit_gang` names.
pub fn join(world: &mut World, id: EntityId) -> bool {
    let Some(gang) = recruit_gang(world, id) else { return false };
    enlist(world, id, gang);
    true
}

/// Put `id` into `gang` with every side effect of joining. No eligibility
/// check: `join` does that; tests call this directly.
pub fn enlist(world: &mut World, id: EntityId, gang: EntityId) {
    let tick = world.tick;
    world.insert(id, GangMember { gang, rank: 0, joined_tick: tick });
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if let Err(i) = g.members.binary_search(&id) {
            g.members.insert(i, id);
        }
        g.empty_since = None;
        g.emptied = false;
        g.claims_cleared = false;
    }
    if let Some(p) = world.comp_mut::<Personality>(id) {
        p.drift(Drift::JoinedGang);
    }
    world.remember(id, MemoryKind::Socialised, None, 0.4, 0.2, false);
    // A recruit who met a current member in jail cites that memory.
    let (members, gname) = world.comp::<Gang>(gang).map(|g| (g.members.clone(), g.name.clone())).unwrap_or_default();
    let cites = world.comp::<Memory>(id).and_then(|m| {
        m.entries
            .iter()
            .position(|e| e.kind == MemoryKind::MetInJail && e.subject.is_some_and(|s| s != id && members.contains(&s)))
    });
    let name = world.name_of(id);
    let text = match cites {
        Some(n) => format!("{name} joined {gname} (cites mem#{n})"),
        None => format!("{name} joined {gname}"),
    };
    world.push_event(EventKind::GangJoin, &[id, gang], text);
    recompute_leader(world, gang);
}

/// Leave the gang (betrayal, emigration; death goes through `on_member_killed`).
pub fn leave(world: &mut World, id: EntityId, reason: &str) {
    let Some(gm) = world.remove::<GangMember>(id) else { return };
    let tick = world.tick;
    let gname = world.comp::<Gang>(gm.gang).map_or_else(|| "the gang".to_string(), |g| g.name.clone());
    if let Some(g) = world.comp_mut::<Gang>(gm.gang) {
        g.members.retain(|&m| m != id);
        if g.members.is_empty() {
            g.empty_since = Some(tick);
            g.emptied = true;
        }
        if g.boss == Some(id) {
            g.boss = None;
        }
    }
    if let Some(p) = world.comp_mut::<Personality>(id) {
        p.drift(Drift::LeftGang);
    }
    let name = world.name_of(id);
    world.push_event(EventKind::GangLeave, &[id, gm.gang], format!("{name} left {gname} ({reason})"));
    recompute_leader(world, gm.gang);
}

/// The leader ranking: free members with a Brain by `loyalty +
/// days_in_gang / 100`, best first (ties the lower id). M12 D36: the
/// runner-up is the lieutenant.
pub fn leader_ranking(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let tick = world.tick;
    let mut ranked: Vec<(EntityId, f32)> = g
        .members
        .iter()
        .copied()
        .filter(|&m| world.has::<Brain>(m) && !world.has::<Sentence>(m))
        .map(|m| {
            let loyalty = world.comp::<Personality>(m).map_or(0.0, |p| p.loyalty);
            let days = world
                .comp::<GangMember>(m)
                .map_or(0.0, |g| tick.saturating_sub(g.joined_tick) as f32 / TICKS_PER_DAY as f32);
            (m, loyalty + days / 100.0)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().map(|(m, _)| m).collect()
}

/// `leader = argmax(loyalty + days_in_gang / 100)` over members with a Brain
/// and no Sentence; rank 2, others 0. A change of leader is a shock.
pub fn recompute_leader(world: &mut World, gang: EntityId) {
    recompute_leader_after(world, gang, None);
}

/// [`recompute_leader`] after `fallen` was killed or jailed: when the fallen
/// was the leader and a new one takes over, the gang is due a split check
/// (M12 D36, `Gang.split_check`).
fn recompute_leader_after(world: &mut World, gang: EntityId, fallen: Option<EntityId>) {
    let Some(g) = world.comp::<Gang>(gang) else { return };
    let (members, old) = (g.members.clone(), g.leader);
    let leader = leader_ranking(world, gang).first().copied();
    for &m in &members {
        if let Some(gm) = world.comp_mut::<GangMember>(m) {
            gm.rank = if Some(m) == leader { 2 } else { 0 };
        }
    }
    // A leader who lost the post to a Sentence is the boss until they are out.
    // The standing boss is kept while they are still inside.
    let jailed_boss = old.filter(|&o| leader != Some(o) && world.has::<Sentence>(o));
    let boss_inside = world.comp::<Gang>(gang).and_then(|g| g.boss).is_some_and(|b| world.has::<Sentence>(b));
    let decapitated = fallen.is_some() && old == fallen && leader.is_some() && leader != old;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.leader = leader;
        if jailed_boss.is_some() && !boss_inside {
            g.boss = jailed_boss;
        }
        if decapitated {
            g.split_check = old;
        }
    }
    if old.is_some() && leader != old {
        push_shock(world, gang, Shock::LeaderChanged);
    }
}

/// A member is out of the Jail (served, player-released or broken out):
/// they are nobody's boss any more and may lead again.
pub fn on_member_released(world: &mut World, id: EntityId) {
    let Some(gang) = world.gang_of(id) else { return };
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if g.boss == Some(id) {
            g.boss = None;
        }
    }
    recompute_leader(world, gang);
}

/// Members serving a sentence (a Vagrancy night excluded).
pub fn jailed_headcount(world: &World, gang: EntityId) -> usize {
    // M12 D15: a night in the cells for Vagrancy is no reason to storm the Jail.
    let held = |m: EntityId| world.comp::<Sentence>(m).is_some_and(|s| s.crime != crate::components::Crime::Vagrancy);
    world.comp::<Gang>(gang).map_or(0, |g| g.members.iter().filter(|&&m| held(m)).count())
}

/// Members not in the Jail.
pub fn fit_headcount(world: &World, gang: EntityId) -> usize {
    world.comp::<Gang>(gang).map_or(0, |g| g.members.iter().filter(|&&m| !world.has::<Sentence>(m)).count())
}

// ---------------------------------------------------------------------------
// Heat and shocks
// ---------------------------------------------------------------------------

pub fn push_shock(world: &mut World, gang: EntityId, shock: Shock) {
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.shocks.push(shock);
    }
}

/// A member's killer turned out to be a rival (an off-screen killing bound
/// later): the pending `MemberKilled { by_rival: false }` becomes
/// `by_rival: true`; once the brain has consumed it, only the difference
/// arrives (`Shock::RivalNamed`). One death is never two shocks.
pub fn upgrade_kill_shock(world: &mut World, gang: EntityId) {
    let Some(g) = world.comp_mut::<Gang>(gang) else { return };
    if let Some(s) = g.shocks.iter_mut().find(|s| **s == Shock::MemberKilled { by_rival: false }) {
        *s = Shock::MemberKilled { by_rival: true };
    } else {
        g.shocks.push(Shock::RivalNamed);
    }
}

fn log_heat(world: &mut World, gang: EntityId, member: EntityId) {
    let tick = world.tick;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if g.heat_log.len() >= HEAT_LOG_CAP {
            g.heat_log.pop_front();
        }
        g.heat_log.push_back((tick, member));
    }
}

/// `law::sentence` jailed a member: heat, a shock, and maybe a new leader.
pub fn on_member_arrested(world: &mut World, id: EntityId) {
    let Some(gang) = world.gang_of(id) else { return };
    log_heat(world, gang, id);
    push_shock(world, gang, Shock::MemberArrested);
    recompute_leader_after(world, gang, Some(id));
}

/// `World::kill_by`, before the components go: the member leaves the roster;
/// the gang logs the heat and the shock (`by_rival` when a rival member did it).
pub fn on_member_killed(world: &mut World, id: EntityId, killer: Option<EntityId>) {
    let Some(gang) = world.gang_of(id) else { return };
    let by_rival = killer.and_then(|k| world.gang_of(k)).is_some_and(|k| k != gang);
    let tick = world.tick;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.members.retain(|&m| m != id);
        if g.members.is_empty() {
            g.empty_since = Some(tick);
            g.emptied = true;
        }
        if g.boss == Some(id) {
            g.boss = None;
        }
    }
    log_heat(world, gang, id);
    push_shock(world, gang, Shock::MemberKilled { by_rival });
    recompute_leader_after(world, gang, Some(id));
}

// ---------------------------------------------------------------------------
// Targets and extortion
// ---------------------------------------------------------------------------

/// The gang whose territory holds this Home.
pub fn holder_of(world: &World, home: EntityId) -> Option<EntityId> {
    world
        .gang_list()
        .iter()
        .copied()
        .find(|&g| world.comp::<Gang>(g).is_some_and(|gg| gg.territory.binary_search(&home).is_ok()))
}

/// The order a loyal member's GangWork serves right now, or `None` while
/// freelancing (below `freelance_loyalty`) or under an order with no
/// extortion in it. Cheap: no Home scan.
pub fn following_order(world: &World, id: EntityId) -> Option<Order> {
    let g = world.gang_of(id).and_then(|g| world.comp::<Gang>(g))?;
    let loyalty = world.comp::<Personality>(id).map_or(0.0, |p| p.loyalty);
    if loyalty < world.config.gangs.freelance_loyalty {
        return None;
    }
    matches!(g.order, Order::Expand | Order::Contest | Order::Squat).then_some(g.order)
}

/// M12 D38 (phase 3): the derelict Blocks a gang's Squat order may take:
/// in the district of its Hideout or of a Home it holds, not held by it
/// yet, ascending.
pub fn squat_targets(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let mut districts: Vec<crate::components::DistrictId> =
        g.territory.iter().map(|&h| world.district_of_building(h)).collect();
    districts.push(world.district_of_building(g.hideout));
    districts.sort_unstable();
    districts.dedup();
    // Fix pass (phase 3 review): any derelict (a Block, a Bar, a Hotel).
    crate::systems::street::derelicts(world)
        .into_iter()
        .filter(|&b| g.territory.binary_search(&b).is_err())
        .filter(|&b| districts.binary_search(&world.district_of_building(b)).is_ok())
        .collect()
}

/// A member serving the gang's Squat order, inside a derelict.
pub fn serving_squat(world: &World, id: EntityId) -> bool {
    following_order(world, id) == Some(Order::Squat)
        && world
            .comp::<Position>(id)
            .and_then(|p| p.building)
            .is_some_and(|b| crate::systems::street::is_derelict(world, b))
}

/// M12 D38: a member's `Occupy` under the Squat order is a blow of the
/// gang's claim on derelict `b` (no extortion, no crime); a homeless
/// member also squats there. When the claim reaches `CLAIM_HELD` the
/// non-member squatters go: one with courage > 0.6 fights the claimant
/// (and stays if they win), the rest are evicted (ban, `SquatEvicted`).
pub fn squat_claim(world: &mut World, actor: EntityId, b: EntityId) {
    let today = world.day();
    if let Some(br) = world.comp_mut::<Brain>(actor) {
        br.gang_task_day = Some(today);
    }
    let Some(gang) = world.gang_of(actor) else { return };
    if world.comp::<Household>(actor).is_some_and(|h| h.home.is_none())
        && !world.has::<crate::components::Squatter>(actor)
    {
        let _ = crate::systems::street::occupy(world, actor, b);
    }
    let was_held =
        world.comp::<Building>(b).and_then(|bd| bd.claim).is_some_and(|c| c.gang == gang && c.count >= CLAIM_HELD);
    claim(world, actor, b);
    let held =
        world.comp::<Building>(b).and_then(|bd| bd.claim).is_some_and(|c| c.gang == gang && c.count >= CLAIM_HELD);
    if !held || was_held {
        return;
    }
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let what = world.name_of(b);
    world.push_event(EventKind::Squatted, &[actor, b], format!("{gname} took derelict {what} as a squat"));
    let squatters: Vec<EntityId> =
        world.squatters_of(b).iter().copied().filter(|s| members.binary_search(s).is_err()).collect();
    for s in squatters {
        let brave = world.comp::<Personality>(s).is_some_and(|p| p.courage > 0.6);
        if brave && world.has::<Brain>(s) && world.has::<Brain>(actor) {
            let (winner, _, _) = law::resolve_fight(world, s, actor);
            if winner == s {
                continue;
            }
        }
        if world.has::<crate::components::Squatter>(s) {
            crate::systems::street::evict_squatter(world, s, &format!("taken by {gname}"));
        }
    }
}

/// The Home a GangWork plan extorts and the order it serves (`None` =
/// freelancing). Expand: the unclaimed Home nearest the gang's Hideout;
/// Contest: the Home in the rival's territory nearest it; Raid, Retaliate
/// and LieLow: none. A member below `freelance_loyalty` ignores the order
/// and takes the unclaimed Home nearest themselves. Always: inhabited right
/// now, not the actor's own Home, no guard within `sight_day_crime` of the
/// door. "Unclaimed" and "rival's" read the territory lists, the same
/// definition `claim` flips on, so a contested Home whose claim has been
/// reset still reads as the rival's until the third blow lands.
pub fn gang_work_target(world: &World, id: EntityId) -> Option<(EntityId, Option<Order>)> {
    let gang = world.gang_of(id)?;
    let g = world.comp::<Gang>(gang)?;
    let own_home = world.comp::<Household>(id).and_then(|h| h.home);
    let actor_tile = world.comp::<Position>(id)?.tile;
    let hideout_door = world.comp::<Building>(g.hideout).map_or(actor_tile, |b| b.door);
    // The order decides the origin and the filter before any Home is read:
    // the orders with no target return without the scan (perf).
    let theirs: Option<&[EntityId]>;
    let (from, order) = match following_order(world, id) {
        None if world.comp::<Personality>(id).map_or(0.0, |p| p.loyalty) < world.config.gangs.freelance_loyalty => {
            theirs = None;
            (actor_tile, None)
        }
        None => return None,
        Some(Order::Expand) => {
            theirs = None;
            (hideout_door, Some(Order::Expand))
        }
        Some(Order::Contest) => {
            let rival = world.rival_of(gang)?;
            theirs = Some(world.comp::<Gang>(rival)?.territory.as_slice());
            (hideout_door, Some(Order::Contest))
        }
        // M12 D38: the unheld derelict in the gang's districts nearest the Hideout.
        Some(Order::Squat) => {
            let guards: Vec<TilePos> =
                world.guards().iter().filter_map(|&g| world.comp::<Position>(g).map(|p| p.tile)).collect();
            let r = world.config.crime.sight_day_crime;
            return squat_targets(world, gang)
                .into_iter()
                .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door, b)))
                .filter(|&(door, _)| !guards.iter().any(|&gt| law::chebyshev(gt, door) <= r))
                .map(|(door, b)| (door.manhattan(hideout_door), b.index, b))
                .min()
                .map(|(_, _, b)| (b, Some(Order::Squat)));
        }
        Some(_) => return None,
    };
    let homes = world.buildings_by_kind.get(&BuildingKind::Home)?;
    // Unclaimed (no gang holds it), or under Contest in the rival's list.
    let pick = |h: EntityId| match theirs {
        Some(t) => t.binary_search(&h).is_ok(),
        None => holder_of(world, h).is_none(),
    };
    // Guarded: a guard within `sight_day_crime` of the door. The nearest
    // unguarded candidate by `(door distance, index)`; the guard test runs
    // only for a candidate that would beat the best so far (it was a
    // Homes x guards table rebuilt whenever a guard moved; perf).
    let guards: Vec<TilePos> =
        world.guards().iter().filter_map(|&g| world.comp::<Position>(g).map(|p| p.tile)).collect();
    let r = world.config.crime.sight_day_crime;
    let guarded = |door: TilePos| guards.iter().any(|&gt| law::chebyshev(gt, door) <= r);
    let mut best: Option<((u32, u32), EntityId)> = None;
    for &h in homes {
        if Some(h) == own_home {
            continue;
        }
        let Some(b) = world.comp::<Building>(h) else { continue };
        // M12 D25: a derelict is no extortion target.
        if b.demolished || b.derelict || b.occupants.is_empty() {
            continue;
        }
        let key = (b.door.manhattan(from), h.index);
        if best.is_some_and(|(k, _)| key >= k) || !pick(h) || guarded(b.door) {
            continue;
        }
        best = Some((key, h));
    }
    best.map(|(_, h)| (h, order))
}

/// The Home a GangWork plan extorts.
pub fn extort_target(world: &World, id: EntityId) -> Option<EntityId> {
    gang_work_target(world, id).map(|(h, _)| h)
}

/// Record which order a GangWork plan serves (inspector) and log a
/// once-a-day `Disobeyed` when a freelancer works while the gang musters or
/// lies low. Called by `plan::plan_for` once the GangWork plan is bound, so
/// it reads the cheap `following_order` instead of rescanning the Homes.
pub fn note_gang_work(world: &mut World, id: EntityId) {
    let following = following_order(world, id);
    let (order, gname) = world
        .gang_of(id)
        .and_then(|g| world.comp::<Gang>(g))
        .map(|g| (Some(g.order), g.name.clone()))
        .unwrap_or_default();
    let today = world.day();
    let visible = order.is_some_and(|o| o.is_raid() || o == Order::LieLow);
    let mut log = false;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.following_order = following;
        if following.is_none() && visible && b.disobeyed_day != Some(today) {
            b.disobeyed_day = Some(today);
            log = true;
        }
    }
    if log {
        let name = world.name_of(id);
        world.push_event(
            EventKind::Disobeyed,
            &[id],
            format!("{name} ignores {gname}'s {:?} order and works alone", order.unwrap_or_default()),
        );
    }
}

/// Extort at a Home: take `min(extort_amount, occupants' coins)` proportionally
/// into the actor's wallet; the Home's claim advances (`claim`); victims
/// remember WasRobbed and become enemies.
pub fn extort(world: &mut World, actor: EntityId, home: EntityId) -> i64 {
    let amount = world.config.social.extort_amount;
    let occupants: Vec<EntityId> = world
        .comp::<Building>(home)
        .map(|b| b.occupants.iter().copied().filter(|&o| o != actor && world.has::<Wallet>(o)).collect())
        .unwrap_or_default();
    let total: i64 = occupants.iter().map(|&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0))).sum();
    let take = amount.min(total);
    let mut taken = 0;
    for &o in &occupants {
        let coins = world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0));
        let share = if total > 0 { (take * coins) / total } else { 0 };
        if let Some(w) = world.comp_mut::<Wallet>(o) {
            w.coins -= share;
        }
        taken += share;
        world.remember(o, MemoryKind::WasRobbed, Some(actor), 0.7, -0.7, false);
        social::robbed_by(world, o, actor);
    }
    // The rounding remainder comes a coin at a time from whoever still has one.
    let mut guard = 0;
    while taken < take && guard < 64 {
        guard += 1;
        let Some(&payer) = occupants.iter().max_by_key(|&&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins)) else {
            break;
        };
        if world.comp::<Wallet>(payer).is_none_or(|w| w.coins <= 0) {
            break;
        }
        if let Some(w) = world.comp_mut::<Wallet>(payer) {
            w.coins -= 1;
        }
        taken += 1;
    }
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins += taken;
    }
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(actor) {
        b.loot_today += taken;
        b.gang_task_day = Some(today);
    }
    // M11 D20: a corp landlord books the shakedown as a loss.
    if taken > 0 {
        crate::systems::ownership::note_loss(world, home, taken, Some(actor));
    }
    // M12 D30: the district remembers who shook it down (a riot's grievance).
    if let Some(g) = world.gang_of(actor) {
        let d = world.district_of_building(home);
        let now = world.tick;
        if let Some(x) = world.districts.get_mut(d.index()) {
            if x.shakedowns.len() >= 64 {
                x.shakedowns.pop_front();
            }
            x.shakedowns.push_back((now, g));
        }
    }
    let suffix = claim(world, actor, home).unwrap_or_default();
    let name = world.name_of(actor);
    world.push_event(
        EventKind::Extortion,
        &[actor, home],
        format!("{name} extorted {taken} coins from Block#{}{suffix}", home.index),
    );
    let tile = world.comp::<Position>(actor).map_or(TilePos::default(), |p| p.tile);
    law::raise_crime(world, actor, None, crate::components::Crime::Extortion, tile);
    taken
}

/// The claim rules (spec › Contested Homes): own blows count up, a rival's
/// first blow takes the claim at one, the third blow holds the Home. Every
/// blow on a Home in another gang's territory makes that gang's members the
/// actor's enemies; the flip itself logs `TerritoryFlipped` and shocks the
/// loser. Returns a suffix for the Extortion event text when the Home is held.
fn claim(world: &mut World, actor: EntityId, home: EntityId) -> Option<String> {
    let gang = world.gang_of(actor)?;
    let gname = world.comp::<Gang>(gang)?.name.clone();
    let holders: Vec<EntityId> = world
        .gangs()
        .into_iter()
        .filter(|&g| g != gang && world.comp::<Gang>(g).is_some_and(|gg| gg.territory.binary_search(&home).is_ok()))
        .collect();
    for &h in &holders {
        let members = world.comp::<Gang>(h).map(|g| g.members.clone()).unwrap_or_default();
        for m in members {
            social::make_enemy(world, m, actor, -0.7);
        }
    }
    let count = {
        let b = world.comp_mut::<Building>(home)?;
        let count = match b.claim {
            Some(c) if c.gang == gang => c.count.saturating_add(1),
            _ => 1,
        };
        b.claim = Some(Claim { gang, count });
        count
    };
    if count < CLAIM_HELD {
        return None;
    }
    let mut suffix = None;
    for h in holders {
        if let Some(g) = world.comp_mut::<Gang>(h) {
            if let Ok(i) = g.territory.binary_search(&home) {
                g.territory.remove(i);
            }
        }
        let hname = world.comp::<Gang>(h).map_or_else(String::new, |g| g.name.clone());
        world.push_event(
            EventKind::TerritoryFlipped,
            &[gang, h, home],
            format!("{gname} took Block#{} from {hname}", home.index),
        );
        push_shock(world, h, Shock::HomeFlippedAgainst);
        suffix = Some(format!(" (took it from {hname})"));
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if let Err(i) = g.territory.binary_search(&home) {
            g.territory.insert(i, home);
        }
    }
    Some(suffix.unwrap_or_else(|| format!(" (now {gname} territory)")))
}

/// SplitLoot at the Hideout: half the day's haul into the gang treasury.
pub fn split_loot(world: &mut World, actor: EntityId) -> i64 {
    let loot = world.comp::<Brain>(actor).map_or(0, |b| b.loot_today);
    let share = (loot as f32 * 0.5).floor() as i64;
    let share = share.min(world.comp::<Wallet>(actor).map_or(0, |w| w.coins)).max(0);
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins -= share;
    }
    if let Some(g) = world.gang_of(actor).and_then(|g| world.comp_mut::<Gang>(g)) {
        g.treasury += share;
    }
    if let Some(b) = world.comp_mut::<Brain>(actor) {
        b.loot_today = 0;
    }
    world.remember(actor, MemoryKind::Paid, None, 0.2, 0.2, false);
    share
}

/// Fence at the Hideout: stolen food sold to the gang at floor(price × 0.8)
/// each, paid from the gang treasury; the food goes into the Hideout's stock.
pub fn fence(world: &mut World, actor: EntityId) -> i64 {
    let Some(gang) = world.gang_of(actor) else { return 0 };
    let price = world.mean_price();
    let each = (price as f32 * 0.8).floor() as i64;
    let units = world.comp::<crate::components::Inventory>(actor).map_or(0, |i| i.stolen_food);
    let treasury = world.comp::<Gang>(gang).map_or(0, |g| g.treasury);
    let affordable = if each > 0 { (treasury / each).max(0) as u32 } else { units };
    let sold = units.min(affordable);
    if sold == 0 {
        return 0;
    }
    let pay = each * i64::from(sold);
    if let Some(i) = world.comp_mut::<crate::components::Inventory>(actor) {
        i.food -= sold;
        i.stolen_food -= sold;
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.treasury -= pay;
        g.treasury += i64::from(sold) * price; // the gang resells at market price
    }
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins += pay;
    }
    let cap = world.config.buildings.hideout.stock_cap;
    if let Some(m) = world.hideout_of(gang).and_then(|h| world.comp_mut::<Building>(h)) {
        m.stock_food = (m.stock_food + sold).min(cap);
    }
    pay
}

// ---------------------------------------------------------------------------
// Per tick: the brain; daily: leaders, tribute, stipend, disbanding, betrayal
// ---------------------------------------------------------------------------

pub fn run(world: &mut World) {
    let daily = world.tick_of_day() == 0;
    let hysteresis = world.config.gangs.hysteresis;
    let threshold = world.config.gangs.shock_severity_rethink;
    for gang in world.gangs() {
        // M12 D36: a decapitation this tick may split the gang.
        if let Some(old) = world.comp_mut::<Gang>(gang).and_then(|g| g.split_check.take()) {
            let _ = split(world, gang, Some(old), true);
        }
        if daily {
            recompute_leader(world, gang);
            // The law has just rescored (it runs before this system): a gang
            // under Crackdown may buy its way out before choosing its order.
            faction::consider_bribe(world, gang);
        }
        let pending: f32 = world.comp::<Gang>(gang).map_or(0.0, |g| g.shocks.iter().map(|s| s.severity()).sum());
        if daily {
            if faction::rescore(world, gang, hysteresis) {
                if let Some(g) = world.comp_mut::<Gang>(gang) {
                    g.shocks.clear();
                }
            }
        } else if pending >= threshold {
            faction::rethink(world, gang);
        }
    }
    if daily {
        daily_economy(world);
    }
}

fn daily_economy(world: &mut World) {
    let today = world.day();
    // scan-ok: daily
    for id in world.citizens() {
        if let Some(b) = world.comp_mut::<Brain>(id) {
            if b.gang_task_day != Some(today) {
                b.loot_today = 0;
            }
        }
    }
    let stipend = world.config.social.gang_stipend;
    let now = world.tick;
    for gang in world.gangs() {
        // Territory tribute: 2 coins/day per Home when the occupants can pay.
        let territory = world.comp::<Gang>(gang).map(|g| g.territory.clone()).unwrap_or_default();
        for home in territory {
            let residents: Vec<EntityId> = world.residents_of(home).to_vec();
            let mut owed = 2;
            for r in residents {
                if owed == 0 {
                    break;
                }
                let coins = world.comp::<Wallet>(r).map_or(0, |w| w.coins);
                let pay = owed.min(coins.max(0));
                if pay > 0 {
                    if let Some(w) = world.comp_mut::<Wallet>(r) {
                        w.coins -= pay;
                    }
                    owed -= pay;
                }
            }
            if let Some(g) = world.comp_mut::<Gang>(gang) {
                g.treasury += 2 - owed;
            }
        }
        // Stipend while the treasury holds it, leader first; none while sacked.
        let sacked = world.comp::<Gang>(gang).is_some_and(|g| g.is_sacked(now));
        if !sacked {
            let (mut members, leader) =
                world.comp::<Gang>(gang).map(|g| (g.members.clone(), g.leader)).unwrap_or_default();
            if let Some(l) = leader {
                members.retain(|&m| m != l);
                members.insert(0, l);
            }
            for m in members {
                if !world.comp::<Gang>(gang).is_some_and(|g| g.treasury >= stipend) {
                    break;
                }
                if let Some(g) = world.comp_mut::<Gang>(gang) {
                    g.treasury -= stipend;
                }
                if let Some(w) = world.comp_mut::<Wallet>(m) {
                    w.coins += stipend;
                }
            }
        }
        // M12 D40: an empty gang's claims lapse after `empty_claims_days`.
        clear_claims_if_empty(world, gang);
        // Disband after 30 days with no members: treasury and territory go, the name stays.
        let empty_since =
            world.comp::<Gang>(gang).and_then(|g| if g.members.is_empty() { g.empty_since } else { None });
        if empty_since.is_some_and(|since| now.saturating_sub(since) >= 30 * TICKS_PER_DAY) {
            let territory = world.comp::<Gang>(gang).map(|g| g.territory.clone()).unwrap_or_default();
            for home in territory {
                if let Some(b) = world.comp_mut::<Building>(home) {
                    b.claim = None;
                }
            }
            if let Some(g) = world.comp_mut::<Gang>(gang) {
                g.treasury = 0;
                g.territory.clear();
                g.empty_since = Some(now);
            }
        }
        // Betrayal: a member with loyalty < 0.3 and an open warrant on themselves.
        let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
        for m in members {
            let disloyal = world.comp::<Personality>(m).is_some_and(|p| p.loyalty < 0.3);
            let wanted = law::wanted(world, m);
            if let Some(b) = world.comp_mut::<Brain>(m) {
                b.betraying = disloyal && wanted;
            }
        }
        // M13 D27: the chop shop, then D44: a bike for a member.
        crate::systems::vehicles::chop_daily(world, gang);
        crate::systems::vehicles::gang_bikes(world, gang);
    }
}

/// A betrayer filed a report naming the leader: they leave, every remaining
/// member becomes their enemy, and their own warrant is resolved.
pub fn betrayal_filed(world: &mut World, betrayer: EntityId) {
    let Some(gang) = world.gang_of(betrayer) else { return };
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    leave(world, betrayer, "betrayal");
    for m in members.into_iter().filter(|&m| m != betrayer) {
        social::make_enemy(world, m, betrayer, -0.7);
    }
    for r in world.reports_mut().iter_mut().filter(|r| r.suspect == betrayer) {
        r.resolved = true;
    }
    if let Some(b) = world.comp_mut::<Brain>(betrayer) {
        b.betraying = false;
    }
    let name = world.name_of(betrayer);
    world.push_event(EventKind::Betrayal, &[betrayer], format!("{name} betrayed the gang"));
}

// ---------------------------------------------------------------------------
// M12 D36, D40: splits and the hydra rule
// ---------------------------------------------------------------------------

/// D40, daily: a gang empty for `empty_claims_days` (0 = off) loses every
/// claim and its territory at once, not at the 30-day disband.
pub fn clear_claims_if_empty(world: &mut World, gang: EntityId) {
    let days = world.config.gangs.empty_claims_days;
    let now = world.tick;
    let due = world.comp::<Gang>(gang).is_some_and(|g| {
        days > 0
            && g.members.is_empty()
            && !g.claims_cleared
            && g.empty_since.is_some_and(|t| now.saturating_sub(t) >= days * TICKS_PER_DAY)
    });
    if !due {
        return;
    }
    let mut lost = 0usize;
    for kind in [BuildingKind::Home, BuildingKind::Bar, BuildingKind::Hotel] {
        for b in world.buildings_of_kind(kind).to_vec() {
            if let Some(bd) = world.comp_mut::<Building>(b) {
                if bd.claim.is_some_and(|c| c.gang == gang) {
                    bd.claim = None;
                    lost += 1;
                }
            }
        }
    }
    let name = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let held = world.comp::<Gang>(gang).map_or(0, |g| g.territory.len());
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.territory.clear();
        g.claims_cleared = true;
    }
    if lost > 0 || held > 0 {
        world.push_event(
            EventKind::GangLeave,
            &[gang],
            format!("{name} lost its claims on {lost} Blocks ({held} held): nobody left to hold them"),
        );
    }
}

/// D36: the districts a gang holds Homes in, `(district, held Homes)`,
/// ascending by district.
pub fn held_districts(world: &World, gang: EntityId) -> Vec<(DistrictId, usize)> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let mut counts: std::collections::BTreeMap<DistrictId, usize> = std::collections::BTreeMap::new();
    for &h in &g.territory {
        *counts.entry(world.district_of_building(h)).or_default() += 1;
    }
    counts.into_iter().collect()
}

/// D36: a decapitated gang (the old leader dead or jailed, a new one in)
/// may split. The lieutenant (runner-up of `leader_ranking`) founds a
/// splinter when the gang holds Homes in two districts or more, the
/// lieutenant's strength is at least `split_strength_ratio` of the new
/// leader's, the mean member loyalty is below `split_loyalty`, the city has
/// fewer than `max_gangs` gangs, and (with `roll`) a draw under
/// `split_base × (1 − mean loyalty)` lands. The splinter takes the held
/// district other than the leader's Home district with the most held Homes
/// (ties lower id); its Hideout is a gang-held squat there, else the vacant
/// Lot there nearest the district's centroid, else no split. It takes
/// every member whose Home (or squat) is there or who likes the lieutenant
/// better than the leader, those Homes' claims, and the treasury share by
/// headcount. `Split` event; Enemy edges across the rosters (30 pairs at
/// most); `Shock::Split` on both. Returns the splinter, or why not.
pub fn split(world: &mut World, gang: EntityId, old_leader: Option<EntityId>, roll: bool) -> Result<EntityId, String> {
    use crate::systems::raid;
    let cfg = world.config.gangs.clone();
    if roll && cfg.split_base <= 0.0 {
        return Err("splits are off".into());
    }
    let ranking = leader_ranking(world, gang);
    let (Some(&leader), Some(&lt)) = (ranking.first(), ranking.get(1)) else {
        return Err("no lieutenant".into());
    };
    let held = held_districts(world, gang);
    if held.len() < 2 {
        return Err(format!("holds Homes in {} district(s)", held.len()));
    }
    // M12 phase 5: the god `SplitGang` (`roll` false) skips the character
    // tests too (the lieutenant's strength, the gang's loyalty); only the
    // structure (a lieutenant, two held districts, the cap, a Hideout site)
    // can refuse it.
    let (sl, sll) = (raid::strength(world, leader), raid::strength(world, lt));
    if roll && sll < cfg.split_strength_ratio * sl {
        return Err(format!("lieutenant too weak ({sll:.2} vs {sl:.2})"));
    }
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let loyalty = members.iter().map(|&m| world.comp::<Personality>(m).map_or(0.5, |p| p.loyalty)).sum::<f32>()
        / members.len().max(1) as f32;
    if roll && loyalty >= cfg.split_loyalty {
        return Err(format!("too loyal ({loyalty:.2})"));
    }
    // Fix pass (phase 4 review): an emptied gang (a ghost waiting to
    // re-form, or never to) does not count toward the cap.
    let live = world
        .gang_list()
        .iter()
        .filter(|&&g| world.comp::<Gang>(g).is_some_and(|x| !(x.emptied && x.members.is_empty())))
        .count();
    if live >= cfg.max_gangs {
        return Err(format!("{live} gangs already"));
    }
    if roll {
        let p = cfg.split_base * (1.0 - loyalty);
        let r: f32 = world.rng.world().random();
        if r >= p {
            return Err(format!("held together ({r:.2} vs {p:.2})"));
        }
    }
    let leader_d = home_or_squat(world, leader).map(|b| world.district_of_building(b));
    let Some(&(sd, _)) =
        held.iter().filter(|(d, _)| Some(*d) != leader_d).max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
    else {
        return Err("no district apart from the leader's".into());
    };
    let Some(hideout) = splinter_hideout(world, gang, sd) else {
        return Err(format!("no squat or Lot in {}", world.district_name(sd)));
    };
    // The splinter.
    let used: Vec<String> =
        world.gang_list().iter().filter_map(|&g| world.comp::<Gang>(g).map(|g| g.name.clone())).collect();
    let old_name = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let name = cfg
        .splinter_names
        .iter()
        .find(|n| !used.contains(n))
        .cloned()
        .unwrap_or_else(|| format!("{old_name} Splinter {}", world.gang_list().len()));
    // Fix pass (phase 4 review): a jailed old boss stays with the gang it
    // ran (unless it is the lieutenant who leads the splinter).
    let boss = world.comp::<Gang>(gang).and_then(|g| g.boss);
    let movers: Vec<EntityId> = members
        .iter()
        .copied()
        .filter(|&m| m != leader)
        .filter(|&m| m == lt || Some(m) != boss)
        .filter(|&m| {
            m == lt
                || home_or_squat(world, m).is_some_and(|b| world.district_of_building(b) == sd)
                || world.edge(m, lt).map_or(0.0, |e| e.affinity) > world.edge(m, leader).map_or(0.0, |e| e.affinity)
        })
        .collect();
    let treasury = world.comp::<Gang>(gang).map_or(0, |g| g.treasury.max(0));
    let share = treasury * movers.len() as i64 / members.len().max(1) as i64;
    let splinter = world.spawn();
    let mut sg = Gang::new(name.clone(), hideout, share);
    sg.empty_since = None;
    sg.split_from = Some(gang);
    world.insert(splinter, sg);
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.treasury -= share;
        g.members.retain(|m| !movers.contains(m));
        if g.raid_target.is_some_and(|t| t == hideout) {
            g.raid_target = None;
        }
    }
    for &m in &movers {
        if let Some(gm) = world.comp_mut::<GangMember>(m) {
            gm.gang = splinter;
        }
    }
    // Claims on the splinter district's Homes go with it.
    let moved: Vec<EntityId> = world
        .comp::<Gang>(gang)
        .map(|g| g.territory.iter().copied().filter(|&h| world.district_of_building(h) == sd).collect())
        .unwrap_or_default();
    for &h in &moved {
        if let Some(b) = world.comp_mut::<Building>(h) {
            if let Some(c) = b.claim.as_mut() {
                c.gang = splinter;
            }
        }
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.territory.retain(|h| !moved.contains(h));
    }
    if let Some(g) = world.comp_mut::<Gang>(splinter) {
        g.members = movers.clone();
        g.territory = moved.clone();
    }
    recompute_leader(world, splinter);
    recompute_leader(world, gang);
    // The halves are enemies: 30 pairs each way at most.
    let rest = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let mut pairs = 0;
    'outer: for &a in &movers {
        for &b in &rest {
            if pairs >= 30 {
                break 'outer;
            }
            social::make_enemy(world, a, b, -0.5);
            pairs += 1;
        }
    }
    push_shock(world, gang, Shock::Split);
    push_shock(world, splinter, Shock::Split);
    let ltn = world.name_of(lt);
    let dn = world.district_name(sd).to_string();
    let why = match old_leader {
        Some(o) => format!(" after {} fell", world.name_of(o)),
        None => String::new(),
    };
    world.push_event(
        EventKind::Split,
        &[gang, splinter, lt],
        format!(
            "{ltn} split from {old_name} with {} members and {} Blocks in {dn}{why}: the {name}",
            movers.len(),
            moved.len()
        ),
    );
    faction::rethink(world, splinter);
    faction::rethink(world, gang);
    Ok(splinter)
}

/// The Home an agent lives in, else the derelict it squats.
fn home_or_squat(world: &World, a: EntityId) -> Option<EntityId> {
    world
        .comp::<Household>(a)
        .and_then(|h| h.home)
        .or_else(|| world.comp::<crate::components::Squatter>(a).map(|s| s.building))
}

/// D36: the splinter's Hideout in district `d`: a derelict the gang holds
/// there (lowest id), turned into a Hideout; else the vacant Lot there
/// nearest the district's centroid, built as one. `None` with neither.
fn splinter_hideout(world: &mut World, gang: EntityId, d: DistrictId) -> Option<EntityId> {
    let squat = world.comp::<Gang>(gang).and_then(|g| {
        g.territory
            .iter()
            .copied()
            .find(|&b| world.district_of_building(b) == d && crate::systems::street::is_derelict(world, b))
    });
    if let Some(b) = squat {
        convert_to_hideout(world, gang, b);
        return Some(b);
    }
    let centroid = world.district(d).centroid;
    let lot = world
        .buildings_of_kind(BuildingKind::Lot)
        .iter()
        .copied()
        .filter_map(|l| world.comp::<Building>(l).filter(|b| !b.demolished).map(|b| (b.door, l)))
        .filter(|&(door, _)| world.district_of(door) == d)
        .map(|(door, l)| (door.manhattan(centroid), l))
        .min()
        .map(|(_, l)| l)?;
    crate::systems::founding::build_on_lot(world, lot, BuildingKind::Hideout, None).ok()
}

/// A held derelict becomes a Hideout: its squatters put out, its claim and
/// its place in the old gang's territory dropped, its kind and capacity
/// changed, the district lists rebuilt.
fn convert_to_hideout(world: &mut World, gang: EntityId, b: EntityId) {
    crate::systems::street::evict_squatters(world, b, "a gang's new Hideout");
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.territory.retain(|&h| h != b);
    }
    let cap = world.config.buildings.hideout.capacity;
    let interior = world
        .comp::<Building>(b)
        .map_or(0, |bd| usize::from(bd.rect.w.saturating_sub(2)) * usize::from(bd.rect.h.saturating_sub(2)));
    let old_kind = world.comp::<Building>(b).map(|bd| bd.kind);
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.kind = BuildingKind::Hideout;
        bd.derelict = false;
        bd.full_capacity = None;
        bd.empty_since = None;
        bd.claim = None;
        bd.rent_per_day = 0;
        bd.stock_food = 0;
        bd.capacity = u8::try_from(interior).unwrap_or(u8::MAX).min(cap);
    }
    if let Some(k) = old_kind {
        if let Some(v) = world.buildings_by_kind.get_mut(&k) {
            v.retain(|&x| x != b);
        }
    }
    let list = world.buildings_by_kind.entry(BuildingKind::Hideout).or_default();
    if let Err(i) = list.binary_search(&b) {
        list.insert(i, b);
    }
    crate::systems::districts::rebuild(world);
}
