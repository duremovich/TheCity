//! Expeditions: the muster, the march to a door, and the brawl there,
//! resolved strongest-against-strongest through `law::resolve_fight`.
//!
//! A gang's raid on the rival Hideout has three outcomes: lost, won (half the
//! treasury), sacked. M9: the same march under `Order::BreakOut` ends at the
//! Jail door, where `breach` fights the guards present and frees the gang's
//! convicts. M12 phase 4: a Raid may aim at a corp building instead
//! (`Gang.raid_target`, [`corp_brawl`]); a district riot runs the same
//! machinery with its rioters as raiders ([`Expedition::Riot`],
//! `systems::riot`); musters gather near the target ([`muster_point`]); and
//! every brawl at a door rolls once for bystanders ([`crossfire`]).

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Corp, CorpShock, Crime, DistrictId, Gang, GoalKind, Inventory, Job, MemoryKind,
    Needs, Order, Position, Sentence, Shock, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::{faction, gang, law, law_brain};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

mod brawl;
mod corp;
mod march;
mod targets;

pub use brawl::*;
pub use corp::*;
pub use march::*;
pub use targets::*;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Lost,
    Won,
    Sacked,
}

/// M12 D31: whose march an agent is on: its gang's, or a district riot's;
/// M16a (plan C22): or a contract mission's (a crew list marching to a
/// door, keyed by its record).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Expedition {
    Gang(EntityId),
    Riot(u32),
    Mission(crate::contract::ContractId),
}

/// M12 D37: where an expedition gathers.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MusterAt {
    /// Inside a building (the gang's Hideout, M8).
    Inside(EntityId),
    /// On the street tile outside a door (a held Home or squat near the
    /// target, a riot's muster building).
    Door(TilePos),
}

/// The riot with this id, if it is live.
pub fn riot_by_id(world: &World, id: u32) -> Option<&crate::components::Riot> {
    world.riots.iter().find(|r| r.id == id)
}

/// D31: a live riot's rioter marches with the riot; anyone else with its
/// gang. M16a (plan C22): a mission's crew member marches with its mission
/// first (nothing else changes while no mission exists).
pub fn expedition_of(world: &World, agent: EntityId) -> Option<Expedition> {
    if !world.missions.is_empty() {
        if let Some(&m) = world.mission_of.get(&agent) {
            return Some(Expedition::Mission(m));
        }
    }
    if let Some(&id) = world.rioter_of.get(&agent) {
        if riot_by_id(world, id).is_some() {
            return Some(Expedition::Riot(id));
        }
    }
    world.gang_of(agent).map(Expedition::Gang)
}

fn own_gang(world: &World, agent: EntityId) -> Option<&Gang> {
    world.gang_of(agent).and_then(|g| world.comp::<Gang>(g))
}

/// The tick the expedition leaves the muster: the gang's `raid_at`, a riot's `muster_at`.
pub fn departure(world: &World, agent: EntityId) -> Option<Tick> {
    match expedition_of(world, agent)? {
        Expedition::Riot(id) => riot_by_id(world, id).map(|r| r.muster_at),
        Expedition::Gang(g) => world.comp::<Gang>(g).and_then(|g| g.raid_at),
        // M16a (plan C22): the mission's own muster hour.
        Expedition::Mission(id) => world.missions.get(&id).map(|m| m.raid_at),
    }
}

/// The Raid goal's gate: the expedition musters within `raid_gather_hours`.
pub fn raid_pending(world: &World, agent: EntityId) -> bool {
    let window = Tick::from(world.config.gangs.raid_gather_hours) * TICKS_PER_HOUR;
    match expedition_of(world, agent) {
        Some(Expedition::Riot(id)) => riot_by_id(world, id).is_some_and(|r| r.muster_at <= world.tick + window),
        Some(Expedition::Gang(_)) => {
            let Some(g) = own_gang(world, agent) else { return false };
            // M14 V32/V37: the member running the prelude or streaming the
            // raid stays in the chair, not in the march.
            let seated = !world.run_orders.is_empty()
                && world
                    .run_orders
                    .get(&agent)
                    .is_some_and(|o| matches!(o.why, crate::virt::RunWhy::Prelude | crate::virt::RunWhy::Overwatch));
            // Lives at work phase 6 (§ 4.5): a delegating leader stays home
            // (the march is his lieutenants').
            // The city milestone C9b: the seat's standing guard stays home.
            !seated
                && !crate::systems::crew::seat_guard_today(world, agent)
                && g.order.is_raid()
                && g.raid_at.is_some_and(|t| t <= world.tick + window)
                && !crate::systems::careers::delegating(world, agent)
        }
        // M16a (plan C22): the Raid goal never adopts a mission (its crew
        // marches under `GoalKind::Contract`).
        Some(Expedition::Mission(_)) => false,
        None => false,
    }
}

/// No expedition is pending for this agent (the Raid goal state).
pub fn raid_done(world: &World, agent: EntityId) -> bool {
    match expedition_of(world, agent) {
        Some(Expedition::Riot(_)) => false,
        Some(Expedition::Gang(_)) => own_gang(world, agent).is_none_or(|g| g.raid_at.is_none()),
        // M16a (plan C22): the mission is over once its record is not Taken.
        Some(Expedition::Mission(id)) => {
            world.contracts.get(&id).is_none_or(|c| c.status != crate::contract::ContractStatus::Taken)
        }
        None => true,
    }
}

/// The muster has departed: latecomers skip the wait. V8b (`OPEN-276`): a
/// gang's, once its column has left (`Gang.raid_left`), not at its hour (a
/// held muster still gathers the late).
pub fn mustered(world: &World, agent: EntityId) -> bool {
    if let Some(Expedition::Gang(g)) = expedition_of(world, agent) {
        if hold_ticks(world).is_some() {
            return world.comp::<Gang>(g).is_some_and(left);
        }
    }
    departure(world, agent).is_some_and(|t| world.tick >= t)
}

/// V8b (`OPEN-276`): what a member's Muster step does at its end.
pub enum Departure {
    /// The column leaves: he marches.
    Go,
    /// The muster holds for its column (`[crew] muster_hold_ticks`).
    Hold,
    /// No raid: the order changed, the law holds the target, or the muster broke up.
    Off,
}

/// V8b: `[crew] muster_hold_ticks` (`None` off or 0).
fn hold_ticks(world: &World) -> Option<Tick> {
    crate::systems::crew::cfg(world).map(|c| c.muster_hold_ticks).filter(|&t| t > 0)
}

/// V8b: the gang's standing raid has left its muster.
fn left(g: &Gang) -> bool {
    g.raid_at.is_some_and(|t| g.raid_left.is_some_and(|l| l >= t))
}

/// V8b (`OPEN-276`): the actor and the members standing at his raid's
/// muster (inside the Hideout, or within `raid_gather_radius` of the door)
/// on the Raid goal, fit, free and adult.
fn at_muster(world: &World, gid: EntityId, actor: EntityId) -> Vec<EntityId> {
    let radius = world.config.gangs.raid_gather_radius;
    let (inside, door) = match muster_point(world, actor) {
        Some(MusterAt::Inside(h)) => (Some(h), world.comp::<Building>(h).map(|b| b.door)),
        Some(MusterAt::Door(t)) => (None, Some(t)),
        None => return vec![actor],
    };
    let members = world.comp::<Gang>(gid).map(|g| g.members.clone()).unwrap_or_default();
    members
        .into_iter()
        .filter(|&m| {
            m == actor
                || (law::living(world, m)
                    && !world.has::<Sentence>(m)
                    && !crate::systems::ages::is_protected(world, m)
                    && world.comp::<Brain>(m).is_some_and(|b| b.current_goal == Some(GoalKind::Raid))
                    && world.comp::<Position>(m).is_some_and(|p| {
                        (inside.is_some() && p.building == inside)
                            || door.is_some_and(|d| law::chebyshev(p.tile, d) <= radius)
                    }))
        })
        .collect()
}

/// V8b (`OPEN-276`): the muster held its `muster_hold_ticks` and fewer than
/// `raid_min_members` came: the raid breaks up there, with who came and
/// where the rest of the free adults were. The ones who came kept the duty.
fn broke_up(world: &mut World, gid: EntityId, came: &[EntityId]) {
    let Some(g) = world.comp::<Gang>(gid) else { return };
    let (gname, members) = (g.name.clone(), g.members.clone());
    let what = gang_target(world, gid).map_or_else(|| "its target".to_string(), |b| world.name_of(b));
    let place = match came.first().and_then(|&a| muster_point(world, a)) {
        Some(MusterAt::Inside(h)) => world.name_of(h),
        Some(MusterAt::Door(t)) => format!("a door in {}", world.district_name(world.district_of(t))),
        None => "the muster".to_string(),
    };
    let (mut work, mut sleep, mut cells, mut way, mut other) = (0, 0, 0, 0, 0);
    for &m in members.iter().filter(|m| !came.contains(m)) {
        if !law::living(world, m) || crate::systems::ages::is_protected(world, m) {
            continue;
        }
        if world.has::<Sentence>(m) {
            cells += 1;
            continue;
        }
        match world.comp::<Brain>(m).and_then(|b| b.current_goal) {
            Some(GoalKind::Work) => work += 1,
            Some(GoalKind::Sleep) => sleep += 1,
            Some(GoalKind::Raid) => way += 1,
            _ => other += 1,
        }
    }
    let rest: Vec<String> = [
        (work, "at work"),
        (sleep, "asleep"),
        (way, "on the way"),
        (cells, "in the cells"),
        (other, "about their own business"),
    ]
    .into_iter()
    .filter(|&(n, _)| n > 0)
    .map(|(n, w)| format!("{n} {w}"))
    .collect();
    let names: Vec<String> = came.iter().map(|&m| world.name_of(m)).collect();
    let text = format!(
        "{gname}'s raid on {what} broke up at the muster at {place}: {} came ({}){}",
        came.len(),
        names.join(", "),
        if rest.is_empty() { String::new() } else { format!("; {}", rest.join(", ")) }
    );
    for &m in came {
        crate::systems::crew::sat_muster(world, gid, m);
    }
    if let Some(g) = world.comp_mut::<Gang>(gid) {
        g.raid_at = None;
        g.raid_left = None;
    }
    crate::systems::ownership::jobs2_row(world).raid_broke += 1;
    let mut actors = vec![gid];
    actors.extend_from_slice(came);
    world.push_event(EventKind::Raid, &actors, text);
    faction::rethink(world, gid);
}
