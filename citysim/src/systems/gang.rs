//! The gangs: recruitment into one of several gangs, joining and leaving,
//! ranks and leaders, heat and shocks for the faction brain, extortion and
//! Home claims, loot splitting, fencing, the daily economy, betrayal.
//!
//! The brain that issues orders lives in `faction`; raids in `raid`.

use crate::components::{
    Brain, Building, BuildingKind, Claim, Gang, GangMember, Household, Memory, MemoryKind, Needs, Order, Personality,
    Position, Sentence, Shock, TilePos, Wallet,
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
    let mut best: Option<(f32, EntityId)> = None;
    for &gid in &gangs {
        let Some(g) = world.comp::<Gang>(gid) else { continue };
        if !recruiting(world, g) {
            continue;
        }
        for &m in &g.members {
            let Some(e) = world.edge(id, m) else { continue };
            if e.affinity >= cfg.join_gang_affinity && best.is_none_or(|(a, _)| e.affinity > a) {
                best = Some((e.affinity, gid));
            }
        }
    }
    if let Some((_, gid)) = best {
        return Some(gid);
    }
    let desperate = world.comp::<Needs>(id).is_some_and(|n| n.hunger < cfg.join_gang_desperation_hunger)
        && world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < cfg.join_gang_desperation_lawfulness);
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

/// `leader = argmax(loyalty + days_in_gang / 100)` over members with a Brain
/// and no Sentence; rank 2, others 0. A change of leader is a shock.
pub fn recompute_leader(world: &mut World, gang: EntityId) {
    let Some(g) = world.comp::<Gang>(gang) else { return };
    let (members, old) = (g.members.clone(), g.leader);
    let tick = world.tick;
    let leader = members
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
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(m, _)| m);
    for &m in &members {
        if let Some(gm) = world.comp_mut::<GangMember>(m) {
            gm.rank = if Some(m) == leader { 2 } else { 0 };
        }
    }
    // A leader who lost the post to a Sentence is the boss until they are out.
    // The standing boss is kept while they are still inside.
    let jailed_boss = old.filter(|&o| leader != Some(o) && world.has::<Sentence>(o));
    let boss_inside = world.comp::<Gang>(gang).and_then(|g| g.boss).is_some_and(|b| world.has::<Sentence>(b));
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.leader = leader;
        if jailed_boss.is_some() && !boss_inside {
            g.boss = jailed_boss;
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

/// Members with a `Sentence`.
pub fn jailed_headcount(world: &World, gang: EntityId) -> usize {
    world.comp::<Gang>(gang).map_or(0, |g| g.members.iter().filter(|&&m| world.has::<Sentence>(m)).count())
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
    recompute_leader(world, gang);
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
        }
        if g.boss == Some(id) {
            g.boss = None;
        }
    }
    log_heat(world, gang, id);
    push_shock(world, gang, Shock::MemberKilled { by_rival });
    recompute_leader(world, gang);
}

// ---------------------------------------------------------------------------
// Targets and extortion
// ---------------------------------------------------------------------------

/// The gang whose territory holds this Home.
pub fn holder_of(world: &World, home: EntityId) -> Option<EntityId> {
    world
        .gangs()
        .into_iter()
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
    matches!(g.order, Order::Expand | Order::Contest).then_some(g.order)
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
    let r = world.config.crime.sight_day_crime;
    let guards: Vec<TilePos> = world
        .citizens()
        .into_iter()
        .filter(|&g| law::is_guard(world, g))
        .filter_map(|g| world.comp::<Position>(g).map(|p| p.tile))
        .collect();
    let candidates: Vec<(EntityId, &Building)> = world
        .buildings_by_kind
        .get(&BuildingKind::Home)?
        .iter()
        .copied()
        .filter(|&h| Some(h) != own_home)
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && !b.occupants.is_empty())
        .filter(|(_, b)| !guards.iter().any(|&gt| law::chebyshev(gt, b.door) <= r))
        .collect();
    let nearest = |from: TilePos, pick: &dyn Fn(EntityId) -> bool| -> Option<EntityId> {
        candidates
            .iter()
            .filter(|(h, _)| pick(*h))
            .min_by_key(|(h, b)| (b.door.manhattan(from), h.index))
            .map(|(h, _)| *h)
    };
    let unclaimed = |h: EntityId| holder_of(world, h).is_none();
    match following_order(world, id) {
        None if world.comp::<Personality>(id).map_or(0.0, |p| p.loyalty) < world.config.gangs.freelance_loyalty => {
            nearest(actor_tile, &unclaimed).map(|h| (h, None))
        }
        None => None,
        Some(Order::Expand) => nearest(hideout_door, &unclaimed).map(|h| (h, Some(Order::Expand))),
        Some(Order::Contest) => {
            let rival = world.rival_of(gang)?;
            let theirs = world.comp::<Gang>(rival)?.territory.clone();
            nearest(hideout_door, &|h| theirs.binary_search(&h).is_ok()).map(|h| (h, Some(Order::Contest)))
        }
        Some(_) => None,
    }
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
    let suffix = claim(world, actor, home).unwrap_or_default();
    let name = world.name_of(actor);
    world.push_event(
        EventKind::Extortion,
        &[actor, home],
        format!("{name} extorted {taken} coins from Home#{}{suffix}", home.index),
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
            format!("{gname} took Home#{} from {hname}", home.index),
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
    let price = world.market().map_or(0, |m| m.price_food);
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
            let residents: Vec<EntityId> = world
                .citizens()
                .into_iter()
                .filter(|&c| world.comp::<Household>(c).and_then(|h| h.home) == Some(home))
                .collect();
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
    for r in world.crime_reports.iter_mut().filter(|r| r.suspect == betrayer) {
        r.resolved = true;
    }
    if let Some(b) = world.comp_mut::<Brain>(betrayer) {
        b.betraying = false;
    }
    let name = world.name_of(betrayer);
    world.push_event(EventKind::Betrayal, &[betrayer], format!("{name} betrayed the gang"));
}
