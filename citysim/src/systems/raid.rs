//! Hideout raids: the muster at the own Hideout, the march to the rival's
//! door, and the brawl there, resolved strongest-against-strongest through
//! `law::resolve_fight`. Three outcomes: lost, won (half the treasury), sacked.
//!
//! M9: the same march under `Order::BreakOut` ends at the Jail door, where
//! `breach` fights the guards present and frees the gang's convicts.

use crate::components::{
    Brain, Building, BuildingKind, Crime, Gang, GoalKind, Order, Position, Sentence, Shock, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::{faction, gang, law};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Lost,
    Won,
    Sacked,
}

fn own_gang(world: &World, agent: EntityId) -> Option<&Gang> {
    world.gang_of(agent).and_then(|g| world.comp::<Gang>(g))
}

/// The Raid goal's gate: the gang musters within `raid_gather_hours`.
pub fn raid_pending(world: &World, agent: EntityId) -> bool {
    let Some(g) = own_gang(world, agent) else { return false };
    let window = Tick::from(world.config.gangs.raid_gather_hours) * TICKS_PER_HOUR;
    g.order.is_raid() && g.raid_at.is_some_and(|t| t <= world.tick + window)
}

/// No raid is pending for this agent's gang (the Raid goal state).
pub fn raid_done(world: &World, agent: EntityId) -> bool {
    own_gang(world, agent).is_none_or(|g| g.raid_at.is_none())
}

/// The muster has departed: latecomers skip the wait.
pub fn mustered(world: &World, agent: EntityId) -> bool {
    own_gang(world, agent).is_some_and(|g| g.raid_at.is_some_and(|t| world.tick >= t))
}

/// The street tile outside the rival Hideout's door.
pub fn rival_hideout_tile(world: &World, agent: EntityId) -> Option<TilePos> {
    let gang = world.gang_of(agent)?;
    let rival = world.rival_of(gang)?;
    let hideout = world.hideout_of(rival)?;
    world.comp::<Building>(hideout).map(|b| world.outside_door(b))
}

/// The street tile outside the Jail door.
pub fn jail_tile(world: &World) -> Option<TilePos> {
    let jail = world.building_of_kind(BuildingKind::Jail)?;
    world.comp::<Building>(jail).map(|b| world.outside_door(b))
}

/// Where this agent's expedition ends: the Jail under BreakOut, else the
/// rival Hideout (`LocationKey::RaidTarget`).
pub fn target_tile(world: &World, agent: EntityId) -> Option<TilePos> {
    if own_gang(world, agent).is_some_and(|g| g.order.target_is_jail()) {
        jail_tile(world)
    } else {
        rival_hideout_tile(world, agent)
    }
}

/// Muster complete: the raid departs. False when the order changed meanwhile
/// (the step fails and the member replans).
pub fn depart(world: &mut World, agent: EntityId) -> bool {
    let Some(gid) = world.gang_of(agent) else { return false };
    let now = world.tick;
    let Some(g) = world.comp_mut::<Gang>(gid) else { return false };
    if g.raid_at.is_none_or(|t| t > now) {
        return false;
    }
    // The first member out the door stamps the departure; the rest follow it.
    if g.order.target_is_jail() {
        if g.last_breakout_tick.is_none_or(|t| t + TICKS_PER_DAY < now) {
            g.last_breakout_tick = Some(now);
        }
    } else if g.last_raid_tick.is_none_or(|t| t + TICKS_PER_DAY < now) {
        g.last_raid_tick = Some(now);
    }
    true
}

/// `Brawl` at the expedition's door: a breach of the Jail under BreakOut,
/// a raid on the rival Hideout otherwise.
pub fn resolve(world: &mut World, actor: EntityId) -> Option<Outcome> {
    if own_gang(world, actor).is_some_and(|g| g.order.target_is_jail()) {
        breach(world, actor)
    } else {
        brawl(world, actor)
    }
}

/// The raiders present at a door: the actor, plus every fit member of the
/// gang within `raid_gather_radius` whose goal is Raid, strongest first.
fn raiders_at(world: &World, gid: EntityId, actor: EntityId, door: TilePos) -> Vec<EntityId> {
    let radius = world.config.gangs.raid_gather_radius;
    let mut raiders: Vec<EntityId> = world
        .comp::<Gang>(gid)
        .map(|g| g.members.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&m| !world.has::<Sentence>(m))
        .filter(|&m| {
            m == actor
                || (world.comp::<Position>(m).is_some_and(|p| law::chebyshev(p.tile, door) <= radius)
                    && world.comp::<Brain>(m).is_some_and(|b| b.current_goal == Some(GoalKind::Raid)))
        })
        .collect();
    by_strength(world, &mut raiders);
    raiders
}

/// Strongest against strongest until one side is out. Each pairing is an
/// Assault (or a Murder) by the raider, witnessed as usual. Returns
/// `(raider losses, deaths)`; the vectors hold whoever is still standing.
fn fight_out(
    world: &mut World,
    raiders: &mut Vec<EntityId>,
    defenders: &mut Vec<EntityId>,
    door: TilePos,
    place: &str,
) -> (usize, usize) {
    let (mut raider_losses, mut deaths) = (0usize, 0usize);
    while let (Some(&r), Some(&d)) = (raiders.first(), defenders.first()) {
        let (_, loser, died) = law::resolve_fight(world, r, d);
        if died {
            deaths += 1;
        }
        // The raider is the aggressor: Murder when the defender died, Assault
        // otherwise; a raider who died is charged with nothing.
        let murder = died && loser == d;
        let crime = if murder { Crime::Murder } else { Crime::Assault };
        let kind = if murder { EventKind::Murder } else { EventKind::Assault };
        let fell = if died && loser == r { " and died" } else { "" };
        let text = format!("{} attacked {} at {place}{fell}", world.name_of(r), world.name_of(d));
        world.push_event(kind, &[r, d], text);
        if law::living(world, r) {
            law::raise_crime(world, r, (!murder).then_some(d), crime, door);
        }
        if loser == d {
            defenders.remove(0);
        } else {
            raiders.remove(0);
            raider_losses += 1;
        }
    }
    (raider_losses, deaths)
}

/// The first raider at the Jail door resolves the breakout; later arrivals
/// find `raid_at` cleared and return `None`. Defenders are the guards inside
/// the Jail or within `raid_gather_radius` of its door. A win frees up to
/// `breakout_max_freed` of the gang's convicts, the boss first, then the
/// longest remaining sentence (`law::escape`).
pub fn breach(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let gid = world.gang_of(actor)?;
    if world.comp::<Gang>(gid).is_none_or(|g| g.raid_at.is_none()) {
        return None;
    }
    let jail = world.building_of_kind(BuildingKind::Jail)?;
    let door = world.comp::<Building>(jail)?.door;
    let radius = world.config.gangs.raid_gather_radius;
    let (gname, boss) = world.comp::<Gang>(gid).map(|g| (g.name.clone(), g.boss))?;

    let mut raiders = raiders_at(world, gid, actor, door);
    let mut defenders: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&g| law::is_guard(world, g) && law::living(world, g))
        .filter(|&g| {
            world
                .comp::<Position>(g)
                .is_some_and(|p| p.building == Some(jail) || law::chebyshev(p.tile, door) <= radius)
        })
        .collect();
    by_strength(world, &mut defenders);
    let (n_raiders, n_defenders) = (raiders.len(), defenders.len());
    let (_, deaths) = fight_out(world, &mut raiders, &mut defenders, door, "the Jail");
    let outcome = if defenders.is_empty() { Outcome::Won } else { Outcome::Lost };

    let mut freed = Vec::new();
    if outcome == Outcome::Won {
        let now = world.tick;
        let mut convicts: Vec<(bool, Tick, EntityId)> = world
            .comp::<Gang>(gid)
            .map(|g| g.members.clone())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|m| world.comp::<Sentence>(m).map(|s| (Some(m) != boss, now.saturating_sub(s.until_tick), m)))
            .collect();
        // The boss first (false sorts first), then the longest remaining sentence.
        convicts.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        for (_, _, m) in convicts.into_iter().take(world.config.gangs.breakout_max_freed) {
            law::escape(world, m);
            gang::push_shock(world, gid, Shock::MemberFreed);
            freed.push(m);
        }
    }
    let now = world.tick;
    match outcome {
        Outcome::Won => {
            let names: Vec<String> = freed.iter().map(|&m| world.name_of(m)).collect();
            let mut actors = vec![gid, jail];
            actors.extend(freed.iter().copied());
            world.push_event(
                EventKind::Jailbreak,
                &actors,
                format!("{gname} broke {} out of the Jail: {}", freed.len(), names.join(", ")),
            );
        }
        _ => gang::push_shock(world, gid, Shock::BreakoutFailed),
    }
    if let Some(g) = world.comp_mut::<Gang>(gid) {
        g.raid_at = None;
        g.last_breakout_tick = Some(now);
    }
    world.push_event(
        EventKind::Raid,
        &[gid, jail, actor],
        format!(
            "{gname} stormed the Jail: {outcome:?} ({n_raiders} raiders vs {n_defenders} guards, {deaths} dead, {} freed)",
            freed.len()
        ),
    );
    crate::systems::law::on_breakout(world, gid, outcome == Outcome::Won);
    faction::rethink(world, gid);
    Some(outcome)
}

/// `fighting + 0.25 × courage`: the brawl's pairing order.
fn strength(world: &World, id: EntityId) -> f32 {
    law::fighting(world, id) + 0.25 * law::courage(world, id)
}

fn by_strength(world: &World, ids: &mut [EntityId]) {
    ids.sort_by(|&a, &b| strength(world, b).total_cmp(&strength(world, a)).then(a.cmp(&b)));
}

/// The first raider at the door resolves the raid; later arrivals find
/// `raid_at` cleared and return `None`.
pub fn brawl(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let gid = world.gang_of(actor)?;
    let rival = world.rival_of(gid)?;
    if world.comp::<Gang>(gid).is_none_or(|g| g.raid_at.is_none()) {
        return None;
    }
    let (rival_hideout, rival_name) = world.comp::<Gang>(rival).map(|g| (g.hideout, g.name.clone()))?;
    let door = world.comp::<Building>(rival_hideout)?.door;
    let gname = world.comp::<Gang>(gid).map(|g| g.name.clone())?;

    let mut raiders = raiders_at(world, gid, actor, door);
    let mut defenders: Vec<EntityId> = world
        .comp::<Gang>(rival)
        .map(|g| g.members.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&m| !world.has::<Sentence>(m))
        .filter(|&m| world.comp::<Position>(m).is_some_and(|p| p.building == Some(rival_hideout)))
        .collect();
    by_strength(world, &mut defenders);
    let (n_raiders, n_defenders) = (raiders.len(), defenders.len());
    let place = format!("the {rival_name} Hideout");
    let (raider_losses, deaths) = fight_out(world, &mut raiders, &mut defenders, door, &place);
    let outcome = if n_defenders == 0 || (defenders.is_empty() && raider_losses == 0) {
        Outcome::Sacked
    } else if defenders.is_empty() {
        Outcome::Won
    } else {
        Outcome::Lost
    };
    settle(world, gid, rival, outcome);
    world.push_event(
        EventKind::Raid,
        &[gid, rival, actor],
        format!(
            "{gname} raided {rival_name}: {outcome:?} ({n_raiders} raiders vs {n_defenders} defenders, {deaths} dead)"
        ),
    );
    Some(outcome)
}

/// Move the prize, mark the sack, shock the loser, and let the attacker
/// rethink at once.
fn settle(world: &mut World, gid: EntityId, rival: EntityId, outcome: Outcome) {
    let now = world.tick;
    let cfg = world.config.gangs.clone();
    // A retaliation is satisfied by being launched, win or lose (spec): a lost
    // one is not a fresh grudge, or a weaker gang would raid nightly to the end.
    let retaliation = world.comp::<Gang>(gid).is_some_and(|g| g.order == Order::Retaliate);
    match outcome {
        Outcome::Lost => {
            if !retaliation {
                gang::push_shock(world, gid, Shock::RaidLost);
            }
        }
        Outcome::Won => {
            let prize = world
                .comp::<Gang>(rival)
                .map_or(0, |g| (g.treasury.max(0) as f32 * cfg.raid_prize_frac).floor() as i64);
            if let Some(g) = world.comp_mut::<Gang>(rival) {
                g.treasury -= prize;
            }
            if let Some(g) = world.comp_mut::<Gang>(gid) {
                g.treasury += prize;
            }
            gang::push_shock(world, rival, Shock::Raided);
        }
        Outcome::Sacked => {
            let Some((loot, rival_hideout, rival_name)) =
                world.comp::<Gang>(rival).map(|g| (g.treasury.max(0), g.hideout, g.name.clone()))
            else {
                return;
            };
            let food = world.comp::<Building>(rival_hideout).map_or(0, |b| b.stock_food);
            if let Some(b) = world.comp_mut::<Building>(rival_hideout) {
                b.stock_food = 0;
            }
            let cap = world.config.buildings.hideout.stock_cap;
            if let Some(b) = world.hideout_of(gid).and_then(|h| world.comp_mut::<Building>(h)) {
                b.stock_food = (b.stock_food + food).min(cap);
            }
            if let Some(g) = world.comp_mut::<Gang>(rival) {
                g.treasury -= loot;
                g.sacked_until = Some(now + cfg.sacked_days * TICKS_PER_DAY);
                // The grudge outlives the sack: the Sacked shock is consumed
                // while Retaliate is still gated, so the brain reads the
                // grievance from retaliate_until once it can muster again.
                g.retaliate_until = Some(now + (cfg.sacked_days + cfg.retaliate_days) * TICKS_PER_DAY);
            }
            if let Some(g) = world.comp_mut::<Gang>(gid) {
                g.treasury += loot;
            }
            // Everyone inside is put out on the street.
            let inside = world.comp::<Building>(rival_hideout).map(|b| b.occupants.clone()).unwrap_or_default();
            for o in inside {
                world.stand_at_door(o, rival_hideout);
            }
            gang::push_shock(world, rival, Shock::Sacked);
            let gname = world.comp::<Gang>(gid).map_or_else(String::new, |g| g.name.clone());
            world.push_event(
                EventKind::Sacked,
                &[rival, gid, rival_hideout],
                format!("{rival_name}'s Hideout sacked by {gname}: {loot} coins and {food} food taken"),
            );
        }
    }
    if let Some(g) = world.comp_mut::<Gang>(gid) {
        g.raid_at = None;
        g.retaliate_until = None;
    }
    faction::rethink(world, gid);
}
