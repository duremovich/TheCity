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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Lost,
    Won,
    Sacked,
}

/// M12 D31: whose march an agent is on: its gang's, or a district riot's.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Expedition {
    Gang(EntityId),
    Riot(u32),
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

/// D31: a live riot's rioter marches with the riot; anyone else with its gang.
pub fn expedition_of(world: &World, agent: EntityId) -> Option<Expedition> {
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
    }
}

/// The Raid goal's gate: the expedition musters within `raid_gather_hours`.
pub fn raid_pending(world: &World, agent: EntityId) -> bool {
    let window = Tick::from(world.config.gangs.raid_gather_hours) * TICKS_PER_HOUR;
    match expedition_of(world, agent) {
        Some(Expedition::Riot(id)) => riot_by_id(world, id).is_some_and(|r| r.muster_at <= world.tick + window),
        Some(Expedition::Gang(_)) => {
            let Some(g) = own_gang(world, agent) else { return false };
            g.order.is_raid() && g.raid_at.is_some_and(|t| t <= world.tick + window)
        }
        None => false,
    }
}

/// No expedition is pending for this agent (the Raid goal state).
pub fn raid_done(world: &World, agent: EntityId) -> bool {
    match expedition_of(world, agent) {
        Some(Expedition::Riot(_)) => false,
        Some(Expedition::Gang(_)) => own_gang(world, agent).is_none_or(|g| g.raid_at.is_none()),
        None => true,
    }
}

/// The muster has departed: latecomers skip the wait.
pub fn mustered(world: &World, agent: EntityId) -> bool {
    departure(world, agent).is_some_and(|t| world.tick >= t)
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

/// D39: the corp building a gang's standing Raid aims at, while it stands.
pub fn corp_target(world: &World, gang: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    if g.order != Order::Raid {
        return None;
    }
    g.raid_target.filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
}

/// A gang's expedition target under its standing order: the Jail under
/// BreakOut, a corp building under a corp Raid, else the rival Hideout.
pub fn gang_target(world: &World, gang: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    if g.order.target_is_jail() {
        return world.building_of_kind(BuildingKind::Jail);
    }
    corp_target(world, gang).or_else(|| world.rival_of(gang).and_then(|r| world.hideout_of(r)))
}

/// The building this agent's expedition ends at (the plan's bound target).
pub fn target_building(world: &World, agent: EntityId) -> Option<EntityId> {
    match expedition_of(world, agent)? {
        Expedition::Riot(id) => riot_by_id(world, id).map(|r| r.target),
        Expedition::Gang(g) => gang_target(world, g),
    }
}

/// Where this agent's expedition ends (`LocationKey::RaidTarget`): the
/// street tile outside the target's door.
pub fn target_tile(world: &World, agent: EntityId) -> Option<TilePos> {
    let b = target_building(world, agent)?;
    world.comp::<Building>(b).map(|bd| world.outside_door(bd))
}

/// D37: where this agent's expedition gathers. A riot: outside its muster
/// building. A gang: outside the held Home or squat nearest the target's
/// door, when one lies within `muster_near_tiles` of it (ties lower id);
/// else inside the Hideout (M8). `None` without an expedition or a Hideout.
pub fn muster_point(world: &World, agent: EntityId) -> Option<MusterAt> {
    match expedition_of(world, agent)? {
        Expedition::Riot(id) => {
            let r = riot_by_id(world, id)?;
            world.comp::<Building>(r.muster).map(|b| MusterAt::Door(world.outside_door(b)))
        }
        Expedition::Gang(gid) => {
            let hideout = world.hideout_of(gid);
            let near = world.config.gangs.muster_near_tiles;
            if near > 0 {
                if let Some(t) = gang_target(world, gid).and_then(|b| world.comp::<Building>(b)).map(|b| b.door) {
                    let g = world.comp::<Gang>(gid)?;
                    let best = g
                        .territory
                        .iter()
                        .filter_map(|&h| {
                            world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(t), h, b))
                        })
                        .filter(|&(d, _, _)| d <= near)
                        .min_by_key(|&(d, h, _)| (d, h));
                    if let Some((_, _, b)) = best {
                        return Some(MusterAt::Door(world.outside_door(b)));
                    }
                }
            }
            hideout.map(MusterAt::Inside)
        }
    }
}

/// Muster complete: the expedition departs. False when the order changed
/// meanwhile, or the target fell under full cover (the step fails and the
/// member replans).
pub fn depart(world: &mut World, agent: EntityId) -> bool {
    let now = world.tick;
    let gid = match expedition_of(world, agent) {
        Some(Expedition::Riot(id)) => {
            let Some(r) = world.riots.iter_mut().find(|r| r.id == id) else { return false };
            if r.muster_at > now {
                return false;
            }
            r.departed.get_or_insert(now);
            return true;
        }
        Some(Expedition::Gang(g)) => g,
        None => return false,
    };
    // M12 D38: no raid departs into a garrisoned Jail or a district cracking
    // down on the raider or cordoned, whatever held when the order was
    // scored; the member replans and the brain rescores the gang.
    let order = world.comp::<Gang>(gid).map(|g| g.order);
    if let Some(order) = order.filter(|o| o.is_raid()) {
        if faction::target_cover(world, gid, order) >= 1.0 {
            // Phase 4: the muster is called off once (one event), and the
            // brain picks a new order at once.
            if world.comp::<Gang>(gid).is_some_and(|g| g.raid_at.is_some_and(|t| t <= now)) {
                let gname = world.comp::<Gang>(gid).map_or_else(String::new, |g| g.name.clone());
                let what = gang_target(world, gid).map_or_else(|| "its target".to_string(), |b| world.name_of(b));
                if let Some(g) = world.comp_mut::<Gang>(gid) {
                    g.raid_at = None;
                }
                world.push_event(
                    EventKind::Raid,
                    &[gid],
                    format!("{gname}'s raid on {what} called off: the law holds the district"),
                );
                faction::rethink(world, gid);
            }
            return false;
        }
    }
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

/// `Brawl` at the expedition's door: a riot's clash, a breach of the Jail
/// under BreakOut, a corp building under a corp Raid, else a raid on the
/// rival Hideout.
pub fn resolve(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let gid = match expedition_of(world, actor)? {
        Expedition::Riot(_) => return crate::systems::riot::clash(world, actor),
        Expedition::Gang(g) => g,
    };
    // M12 D38: count an expedition that reached a door under full cover.
    if let Some(order) = world.comp::<Gang>(gid).filter(|g| g.raid_at.is_some()).map(|g| g.order) {
        if order.is_raid() && faction::target_cover(world, gid, order) >= 1.0 {
            world.stats.current.raids_into_cover += 1;
        }
    }
    if own_gang(world, actor).is_some_and(|g| g.order.target_is_jail()) {
        breach(world, actor)
    } else if corp_target(world, gid).is_some() {
        corp_brawl(world, actor)
    } else {
        brawl(world, actor)
    }
}

/// Phase 4 (raids ≥ 3 at the door): the first marcher at the door holds
/// the Brawl while fewer than `min(3, marchers)` stand there (a riot:
/// `riot_min ÷ 2`), marchers being the expedition's free agents whose goal
/// is Raid, until the last hour of the march window. Event-driven: only a
/// marcher standing at the door asks, once a tick.
pub fn wait_for_crew(world: &World, actor: EntityId) -> bool {
    let now = world.tick;
    let (pool, target, depart, cap) = match expedition_of(world, actor) {
        Some(Expedition::Riot(id)) => {
            let Some(r) = riot_by_id(world, id) else { return false };
            (r.rioters.clone(), Some(r.target), r.muster_at, (world.config.riots.riot_min / 2).max(1))
        }
        Some(Expedition::Gang(g)) => {
            let Some(gg) = world.comp::<Gang>(g) else { return false };
            let Some(t) = gg.raid_at else { return false };
            (gg.members.clone(), gang_target(world, g), t, 3)
        }
        None => return false,
    };
    if now + TICKS_PER_HOUR >= depart + faction::RAID_MARCH_TICKS {
        return false;
    }
    let Some(door) = target.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) else { return false };
    let marching = pool
        .iter()
        .filter(|&&m| {
            law::living(world, m)
                && !world.has::<Sentence>(m)
                && world.comp::<Brain>(m).is_some_and(|b| b.current_goal == Some(GoalKind::Raid))
        })
        .count();
    let present = gathered(world, &pool, actor, door).len();
    present < cap.min(marching)
}

/// The raiders present at a door: the actor, plus every fit member of the
/// gang within `raid_gather_radius` whose goal is Raid, strongest first.
fn raiders_at(world: &World, gid: EntityId, actor: EntityId, door: TilePos) -> Vec<EntityId> {
    let members = world.comp::<Gang>(gid).map(|g| g.members.clone()).unwrap_or_default();
    gathered(world, &members, actor, door)
}

/// Of `pool`, the actor and every free agent within `raid_gather_radius` of
/// the door whose goal is Raid, strongest first (riots reuse it, D32).
pub fn gathered(world: &World, pool: &[EntityId], actor: EntityId, door: TilePos) -> Vec<EntityId> {
    let radius = world.config.gangs.raid_gather_radius;
    let mut out: Vec<EntityId> = pool
        .iter()
        .copied()
        .filter(|&m| !world.has::<Sentence>(m) && law::living(world, m))
        .filter(|&m| {
            m == actor
                || (world.comp::<Position>(m).is_some_and(|p| law::chebyshev(p.tile, door) <= radius)
                    && world.comp::<Brain>(m).is_some_and(|b| b.current_goal == Some(GoalKind::Raid)))
        })
        .collect();
    by_strength(world, &mut out);
    out
}

/// Strongest against strongest until one side is out. Each pairing is an
/// Assault (or a Murder) by the raider, witnessed as usual. The vectors hold
/// whoever is still standing.
#[derive(Default)]
pub struct Tally {
    pub raider_losses: usize,
    /// Deaths on either side.
    pub deaths: usize,
    /// Defenders who lost the pairing and lived.
    pub defenders_beaten: usize,
    /// Raiders who lost a pairing and lived (a lost riot sentences them).
    pub losers: Vec<EntityId>,
    /// Bystanders hit (D35).
    pub crossfire: usize,
}

/// One brawl at `door` (D32, D35): the crossfire roll, the litter, then the
/// pairings with the kill chance × `kill_mult` (a Crush). `riot`: the
/// rioting district (its Statistical residents may be caught too, and the
/// litter is a riot's).
pub fn fight_out(
    world: &mut World,
    raiders: &mut Vec<EntityId>,
    defenders: &mut Vec<EntityId>,
    door: TilePos,
    place: &str,
    kill_mult: f32,
    riot: Option<DistrictId>,
) -> Tally {
    fight_out_until(world, raiders, defenders, door, place, kill_mult, riot, usize::MAX)
}

/// [`fight_out`], but the raiders break and scatter once `max_losses` of
/// them have lost a pairing (M12 fix pass: a corp raid's crew at a held
/// door, `[gangs] corp_raid_break`); the defenders still standing hold.
#[allow(clippy::too_many_arguments)]
pub fn fight_out_until(
    world: &mut World,
    raiders: &mut Vec<EntityId>,
    defenders: &mut Vec<EntityId>,
    door: TilePos,
    place: &str,
    kill_mult: f32,
    riot: Option<DistrictId>,
    max_losses: usize,
) -> Tally {
    let mut t = Tally::default();
    // A brawl needs two sides: a door nobody defends has no crossfire.
    if let (Some(&attacker), false) = (raiders.first(), defenders.is_empty()) {
        let parties: Vec<EntityId> = raiders.iter().chain(defenders.iter()).copied().collect();
        t.crossfire = crossfire(world, door, &parties, attacker, riot, place);
    }
    // M12 D17: a brawl at a door (a raid, a breach) wrecks the street there;
    // a riot's mess is laid by `riot::finish` (64 r3) whether or not it fought.
    if riot.is_none() && !defenders.is_empty() && !raiders.is_empty() {
        crate::systems::litter::deposit(world, door, 32, 2);
    }
    while let (Some(&r), Some(&d)) = (raiders.first(), defenders.first()) {
        if t.raider_losses >= max_losses {
            break;
        }
        let (_, loser, died) = law::resolve_fight_with(world, r, d, kill_mult);
        if died {
            t.deaths += 1;
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
            if !died {
                t.defenders_beaten += 1;
            }
        } else {
            raiders.remove(0);
            t.raider_losses += 1;
            if !died {
                t.losers.push(r);
            }
        }
    }
    t
}

/// D35: once per brawl, before the pairings. Every body within
/// `crossfire_radius` of the door that is not a party rolls `p_crossfire`
/// (ascending id); a riot also draws `riot_stat_bystanders` Statistical
/// residents of its district at the same odds. A hit is an Assault by
/// `attacker` (memory `CaughtInCrossfire`, safety −0.6); `p_crossfire_kill`
/// of hits kill. Returns the hits.
pub fn crossfire(
    world: &mut World,
    door: TilePos,
    parties: &[EntityId],
    attacker: EntityId,
    riot: Option<DistrictId>,
    place: &str,
) -> usize {
    let cfg = world.config.riots.clone();
    if cfg.p_crossfire <= 0.0 {
        return 0;
    }
    // Fix pass (phase 4 review): nobody on either side is a bystander: the
    // parties' gangs, any rioter of a riot among them, and (when a guard
    // fights) every guard.
    let gangs: Vec<EntityId> = parties.iter().filter_map(|&p| world.gang_of(p)).collect();
    let riots: Vec<u32> = parties.iter().filter_map(|p| world.rioter_of.get(p).copied()).collect();
    let guard_side = parties.iter().any(|&p| law::is_guard(world, p));
    let own_side = |w: &World, a: EntityId| {
        w.gang_of(a).is_some_and(|g| gangs.contains(&g))
            || w.rioter_of.get(&a).is_some_and(|r| riots.contains(r))
            || (guard_side && law::is_guard(w, a))
    };
    let mut candidates: Vec<EntityId> = world
        .bodies()
        .into_iter()
        .filter(|a| !parties.contains(a))
        .filter(|&a| !own_side(world, a))
        .filter(|&a| law::living(world, a) && !world.has::<Sentence>(a))
        .filter(|&a| world.comp::<Position>(a).is_some_and(|p| law::chebyshev(p.tile, door) <= cfg.crossfire_radius))
        .collect();
    if let Some(d) = riot {
        let mut pool: Vec<EntityId> = world
            .districts
            .get(d.index())
            .map(|x| x.residents.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| !parties.contains(a) && !candidates.contains(a) && !own_side(world, *a))
            .filter(|&a| {
                law::living(world, a)
                    && !world.has::<Sentence>(a)
                    && world.comp::<Brain>(a).is_some_and(|b| b.lod == crate::components::Lod::Statistical)
            })
            .collect();
        for _ in 0..cfg.riot_stat_bystanders {
            if pool.is_empty() {
                break;
            }
            let i = world.rng.world().random_range(0..pool.len());
            candidates.push(pool.swap_remove(i));
        }
    }
    let mut hits = 0;
    for b in candidates {
        let roll: f32 = world.rng.world().random();
        if roll >= cfg.p_crossfire {
            continue;
        }
        hits += 1;
        world.stats.current.crossfire += 1;
        world.remember(b, MemoryKind::CaughtInCrossfire, Some(attacker), 0.7, -0.7, false);
        if let Some(n) = world.comp_mut::<Needs>(b) {
            n.safety = (n.safety - 0.6).max(0.0);
        }
        let kill: f32 = world.rng.world().random();
        let died = kill < cfg.p_crossfire_kill;
        let (an, bn) = (world.name_of(attacker), world.name_of(b));
        let fell = if died { " and died" } else { "" };
        let kind = if died { EventKind::Murder } else { EventKind::Assault };
        world.push_event(kind, &[attacker, b], format!("{an} hit bystander {bn} at {place}{fell}"));
        world.push_event(
            EventKind::Crossfire,
            &[attacker, b],
            format!("{bn} caught in the crossfire at {place}{fell}"),
        );
        if died {
            world.kill_by(b, crate::components::DeathCause::Violence, Some(attacker));
        }
        // Fix pass (phase 4 review): a hit is a crime as a pairing is (a
        // report, a warrant, Murder when it kills), raised after the death.
        if law::living(world, attacker) {
            let crime = if died { Crime::Murder } else { Crime::Assault };
            law::raise_crime(world, attacker, (!died).then_some(b), crime, door);
        }
    }
    hits
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
    let (gname, boss) = world.comp::<Gang>(gid).map(|g| (g.name.clone(), g.boss))?;

    let mut raiders = raiders_at(world, gid, actor, door);
    let mut defenders = jail_defenders(world, jail, door);
    let (n_raiders, n_defenders) = (raiders.len(), defenders.len());
    let tally = fight_out(world, &mut raiders, &mut defenders, door, "the Precinct", 1.0, None);
    let (deaths, beaten) = (tally.deaths, tally.defenders_beaten);
    let outcome = if defenders.is_empty() { Outcome::Won } else { Outcome::Lost };

    let mut freed = Vec::new();
    if outcome == Outcome::Won {
        let now = world.tick;
        let mut convicts: Vec<(bool, Tick, EntityId)> = world
            .comp::<Gang>(gid)
            .map(|g| g.members.clone())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|m| world.comp::<Sentence>(m).map(|s| (Some(m) != boss, s.until_tick.saturating_sub(now), m)))
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
        // Nobody inside to free: the breach fizzles, no jailbreak, no shock.
        Outcome::Won if freed.is_empty() => {}
        Outcome::Won => {
            let names: Vec<String> = freed.iter().map(|&m| world.name_of(m)).collect();
            let mut actors = vec![gid, jail];
            actors.extend(freed.iter().copied());
            world.push_event(
                EventKind::Jailbreak,
                &actors,
                format!("{gname} broke {} out of the Precinct: {}", freed.len(), names.join(", ")),
            );
        }
        _ => gang::push_shock(world, gid, Shock::BreakoutFailed),
    }
    if let Some(g) = world.comp_mut::<Gang>(gid) {
        g.raid_at = None;
        g.last_breakout_tick = Some(now);
    }
    let verdict = if outcome == Outcome::Won && freed.is_empty() {
        "fizzled, nobody to free".to_string()
    } else {
        format!("{outcome:?}")
    };
    world.push_event(
        EventKind::Raid,
        &[gid, jail, actor],
        format!(
            "{gname} stormed the Precinct: {verdict} ({n_raiders} raiders vs {n_defenders} guards, {deaths} dead, {} freed)",
            freed.len()
        ),
    );
    crate::systems::law::on_breakout(world, gid, outcome == Outcome::Won && !freed.is_empty(), beaten);
    faction::rethink(world, gid);
    Some(outcome)
}

/// The city guards inside the Jail or within `raid_gather_radius` of its door.
pub fn jail_defenders(world: &World, jail: EntityId, door: TilePos) -> Vec<EntityId> {
    let radius = world.config.gangs.raid_gather_radius;
    let mut defenders: Vec<EntityId> = law_brain::guards(world)
        .into_iter()
        .filter(|&g| law::living(world, g))
        .filter(|&g| {
            world
                .comp::<Position>(g)
                .is_some_and(|p| p.building == Some(jail) || law::chebyshev(p.tile, door) <= radius)
        })
        .collect();
    by_strength(world, &mut defenders);
    defenders
}

/// `fighting + 0.25 × courage`: the brawl's pairing order.
pub fn strength(world: &World, id: EntityId) -> f32 {
    law::fighting(world, id) + 0.25 * law::courage(world, id)
}

pub fn by_strength(world: &World, ids: &mut [EntityId]) {
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
    let tally = fight_out(world, &mut raiders, &mut defenders, door, &place, 1.0, None);
    let (raider_losses, deaths) = (tally.raider_losses, tally.deaths);
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

/// The private guards on a building's side: guards employed by a Security
/// Office owned by the building's corp or by the corp holding its security
/// contract (`secured_by`), ascending.
pub fn private_guards_of(world: &World, b: EntityId) -> Vec<EntityId> {
    let owner = world.corp_of_building(b);
    let contractor = world.comp::<Building>(b).and_then(|bd| bd.secured_by);
    world
        .guards()
        .iter()
        .copied()
        .filter(|&g| law::is_private_guard(world, g))
        .filter(|&g| {
            let corp = world.comp::<Job>(g).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e));
            corp.is_some() && (corp == owner || corp == contractor)
        })
        .collect()
}

/// D39: the prize of a won raid on `corp`: `corp_raid_frac` of its treasury,
/// capped at `corp_raid_cap` (never negative).
pub fn corp_prize_value(world: &World, corp: EntityId) -> i64 {
    let cfg = &world.config.gangs;
    let t = world.comp::<Corp>(corp).map_or(0, |c| c.treasury.max(0));
    ((t as f32 * cfg.corp_raid_frac).floor() as i64).min(cfg.corp_raid_cap).max(0)
}

/// M12 fix pass: the private guards posted at a corp building when a raid
/// reaches it: of [`private_guards_of`] (its owner's Security Office or its
/// contractor's), the living, free, uncuffed ones on shift now (any shift
/// while the owner holds Secure: its response calls the off-shift in),
/// nearest the door first (ties lower id), up to `[gangs] corp_raid_posted`
/// × `alertness_mult` of the owner. A guard on contract is at its client
/// when the raid comes (the M11 route walks it there); the brawl stands it
/// at the door. M12 review: only guards within `[crime] answer_radius` of
/// the door are posted, and Secure calls in only the off-shift who are
/// awake ([`can_answer`]), not the ones asleep at Home.
pub fn posted_guards(world: &World, b: EntityId) -> Vec<EntityId> {
    let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else { return Vec::new() };
    let owner = world.corp_of_building(b);
    let cap = world.config.gangs.corp_raid_posted as f32 * faction::alertness_mult(world, owner);
    let cap = cap.round().max(0.0) as usize;
    let tod = world.tick_of_day();
    let secure =
        owner.and_then(|c| world.comp::<Corp>(c)).is_some_and(|c| c.order == crate::components::CorpOrder::Secure);
    let mut out: Vec<(u32, EntityId)> = private_guards_of(world, b)
        .into_iter()
        .filter(|&g| {
            let on = world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod));
            (secure || on) && can_answer(world, g, door, on)
        })
        .filter(|&g| !world.rioter_of.contains_key(&g) && world.gang_of(g).is_none())
        .filter_map(|g| world.comp::<Position>(g).map(|p| (p.tile.manhattan(door), g)))
        .collect();
    out.sort_unstable();
    out.into_iter().take(cap).map(|(_, g)| g).collect()
}

/// M12 review: can guard `g` answer an alarm at `door`: living, free (no
/// Sentence), not cuffed, escorting or emigrating, within `[crime]
/// answer_radius` of the door (Chebyshev), and on shift (`on_shift`), or
/// off shift but awake (its goal is not Sleep). The alarm wakes a guard on
/// duty who dozed (the M10 night-shift Sleep residual); it does not drag an
/// off-duty one out of bed.
fn can_answer(world: &World, g: EntityId, door: TilePos, on_shift: bool) -> bool {
    let radius = world.config.crime.answer_radius;
    law::living(world, g)
        && !world.has::<Sentence>(g)
        && world.comp::<Brain>(g).is_some_and(|b| {
            b.cuffed_by.is_none()
                && b.escorting.is_none()
                && !b.emigrating
                && (on_shift || b.current_goal != Some(GoalKind::Sleep))
        })
        && world.comp::<Position>(g).is_some_and(|p| law::chebyshev(p.tile, door) <= radius)
}

/// M12 review (one rule for a corp raid and a riot): the city guards who
/// answer an alarm at `door` in district `d`. The beat's guards on shift,
/// and in the Precinct's own district the watch on shift inside the
/// Precinct (it is next door; that district has no Homes and only the paid
/// term's thin beat), who [`can_answer`] and are not in `exclude`. The beat
/// first, nearest the door first (Manhattan, ties lower id), then the
/// watch; the first `round(n × alertness_mult)` of them (the M16 hook, 1.0
/// in M12). The caller stands them at the door.
pub fn answering_guards(world: &World, d: DistrictId, door: TilePos, exclude: &[EntityId]) -> Vec<EntityId> {
    let tod = world.tick_of_day();
    let on_shift = |g: EntityId| world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod));
    let mut out: Vec<(bool, u32, EntityId)> = Vec::new();
    if let Some(l) = world.law() {
        for (&g, &bd) in &l.beats {
            if bd == d && !exclude.contains(&g) && on_shift(g) && can_answer(world, g, door, true) {
                if let Some(p) = world.comp::<Position>(g) {
                    out.push((false, p.tile.manhattan(door), g));
                }
            }
        }
    }
    if let Some(jail) = world.building_of_kind(BuildingKind::Jail).filter(|&j| world.district_of_building(j) == d) {
        for g in law_brain::guards(world) {
            if exclude.contains(&g) || out.iter().any(|&(_, _, x)| x == g) {
                continue;
            }
            let Some(p) = world.comp::<Position>(g).filter(|p| p.building == Some(jail)) else { continue };
            if on_shift(g) && can_answer(world, g, door, true) {
                out.push((true, p.tile.manhattan(door), g));
            }
        }
    }
    out.sort_unstable();
    let k = (out.len() as f32 * faction::alertness_mult(world, None)).round().max(0.0) as usize;
    out.into_iter().take(k).map(|(_, _, g)| g).collect()
}

/// D39: a raid on a corp building. Defenders: the corp's private guards (and
/// its contractor's) within `raid_gather_radius` of the door or inside, the
/// guards posted there ([`posted_guards`], fix pass: stood at the door), its
/// employees inside, and the city guards on the district's beat within the
/// radius. Won (nobody left standing): `corp_prize_value` from the corp to
/// the gang (`Flow::Robbery`), `[gangs] corp_raid_loot_frac` of the
/// building's food stock split among the raiders as stolen food (fix pass:
/// a quarter, not all of it), `CorpShock::Robbed` and `Raided`, the corp's
/// `raided_at`. Lost: `RaidLost`.
pub fn corp_brawl(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let gid = world.gang_of(actor)?;
    let target = corp_target(world, gid)?;
    if world.comp::<Gang>(gid).is_none_or(|g| g.raid_at.is_none()) {
        return None;
    }
    let door = world.comp::<Building>(target)?.door;
    let gname = world.comp::<Gang>(gid).map(|g| g.name.clone())?;
    let corp = world.corp_of_building(target);
    let radius = world.config.gangs.raid_gather_radius;
    let d = world.district_of_building(target);
    let near = |w: &World, a: EntityId| {
        w.comp::<Position>(a).is_some_and(|p| p.building == Some(target) || law::chebyshev(p.tile, door) <= radius)
    };
    let mut defenders: Vec<EntityId> =
        private_guards_of(world, target).into_iter().filter(|&g| law::living(world, g) && near(world, g)).collect();
    for g in posted_guards(world, target) {
        if !defenders.contains(&g) {
            if world.has::<Brain>(g) {
                world.abort_plan(g);
            }
            world.stand_at_door(g, target);
            defenders.push(g);
        }
    }
    for s in crate::systems::ownership::staff_at(world, target) {
        if world.comp::<Position>(s).is_some_and(|p| p.building == Some(target)) && !defenders.contains(&s) {
            defenders.push(s);
        }
    }
    // Fix pass: the beat's guards near the door fight; the rest answer the
    // alarm ([`answering_guards`], the riot's rule too).
    let beat: Vec<EntityId> =
        world.law().map(|l| l.beats.iter().filter(|(_, &bd)| bd == d).map(|(&g, _)| g).collect()).unwrap_or_default();
    for g in beat {
        if law::living(world, g) && !defenders.contains(&g) && near(world, g) {
            defenders.push(g);
        }
    }
    for g in answering_guards(world, d, door, &defenders) {
        world.abort_plan(g);
        world.stand_at_door(g, target);
        defenders.push(g);
    }
    defenders.retain(|&x| !world.has::<Sentence>(x) && x != actor && world.gang_of(x) != Some(gid));
    by_strength(world, &mut defenders);
    let mut raiders = raiders_at(world, gid, actor, door);
    let (n_raiders, n_defenders) = (raiders.len(), defenders.len());
    let what = world.name_of(target);
    let owner = world.owner_label(corp);
    let place = format!("{owner}'s {what}");
    // Fix pass: the crew breaks once `corp_raid_break` of it is down (at
    // least one pairing lost); 0 fights to the last raider (phase 4).
    let brk = world.config.gangs.corp_raid_break;
    let max_losses = if brk > 0.0 { ((n_raiders as f32 * brk).ceil() as usize).max(1) } else { usize::MAX };
    let tally = fight_out_until(world, &mut raiders, &mut defenders, door, &place, 1.0, None, max_losses);
    let outcome = if defenders.is_empty() { Outcome::Won } else { Outcome::Lost };
    let mut took = (0i64, 0u32);
    if outcome == Outcome::Won {
        let prize = corp.map_or(0, |c| corp_prize_value(world, c));
        let moved =
            crate::systems::ownership::pay(world, corp, Some(gid), prize, crate::systems::ownership::Flow::Robbery);
        let stock = world.comp::<Building>(target).map_or(0, |b| b.stock_food);
        let frac = world.config.gangs.corp_raid_loot_frac.clamp(0.0, 1.0);
        let standing: Vec<EntityId> = raiders.iter().copied().filter(|&r| law::living(world, r)).collect();
        let food = if standing.is_empty() { 0 } else { (stock as f32 * frac).floor() as u32 };
        if food > 0 {
            if let Some(b) = world.comp_mut::<Building>(target) {
                b.stock_food = b.stock_food.saturating_sub(food);
            }
            share_food(world, &standing, food);
        }
        took = (moved, food);
        if let Some(c) = corp {
            let now = world.tick;
            if let Some(cc) = world.comp_mut::<Corp>(c) {
                cc.raided_at = Some(now);
            }
            crate::systems::ownership::note_loss(world, target, moved, Some(actor));
            crate::systems::ownership::push_corp_shock(world, c, CorpShock::Robbed(moved));
            crate::systems::ownership::push_corp_shock(world, c, CorpShock::Raided);
        }
    } else if world.comp::<Gang>(gid).is_some_and(|g| g.order != Order::Retaliate) {
        gang::push_shock(world, gid, Shock::RaidLost);
    }
    if let Some(g) = world.comp_mut::<Gang>(gid) {
        g.raid_at = None;
        g.raid_target = None;
        g.retaliate_until = None;
    }
    world.push_event(
        EventKind::Raid,
        &[gid, target, actor],
        format!(
            "{gname} raided {place}: {outcome:?} ({n_raiders} raiders vs {n_defenders} defenders, {} dead, {} coins and {} food taken)",
            tally.deaths, took.0, took.1
        ),
    );
    faction::rethink(world, gid);
    Some(outcome)
}

/// Food units into `to`'s inventories round-robin, as stolen food.
pub fn share_food(world: &mut World, to: &[EntityId], units: u32) {
    if to.is_empty() {
        return;
    }
    for i in 0..units as usize {
        if let Some(inv) = world.comp_mut::<Inventory>(to[i % to.len()]) {
            inv.food += 1;
            inv.stolen_food += 1;
        }
    }
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
