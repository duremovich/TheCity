//! M12 district riots (docs/M12_DISTRICTS.md § 6, plan D30-D34).
//!
//! A district angry for `riot_days` midnights in a row riots: its most
//! miserable lawless Street and Dreg residents muster at the district's
//! most central Bar or Market (else a Hotel, else a Block door) at
//! `riot_muster_hour` and march on the target their grievance names: a
//! corp's Market, Block or Hotel, the Precinct, or the controlling gang's
//! Hideout. Rioters run the gangs' Raid goal (`raid::Expedition::Riot`);
//! the first rioter at the door resolves the clash through
//! `raid::fight_out` (crossfire included). Nothing here runs per agent per
//! tick: the trigger is daily, `run` looks at the live riots (at most
//! `max_active_riots`) once a tick.

use crate::components::{
    Brain, Building, BuildingKind, Corp, CorpShock, Crime, DistrictId, Gang, GangMember, Job, Mood, Personality,
    Position, Riot, RiotResponse, RiotTarget, Sentence, Stance,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::raid::{self, Outcome};
use crate::systems::{faction, law, law_brain};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// Days a grievance (evictions, Vagrancy, shakedowns) counts toward a target.
const GRIEVANCE_DAYS: u64 = 14;
/// Disperse: rioters this near the door afterwards are reported.
const DISPERSE_TILES: u32 = 8;
/// Crush: the district's fear + 0.3 for this long.
const CRUSH_DAYS: u64 = 14;

pub fn enabled(world: &World) -> bool {
    world.config.riots.enabled
}

/// Is a riot mustering or running in `d`? (`law_brain`'s `alloc_riot` term.)
pub fn riot_in(world: &World, d: DistrictId) -> bool {
    world.riots.iter().any(|r| r.district == d)
}

/// D34: the door the guards on `d`'s beat are routed to while a Contain or
/// Crush riot musters there (the district stance is then Cordon).
pub fn cordon_target(world: &World, d: DistrictId) -> Option<EntityId> {
    world
        .riots
        .iter()
        .find(|r| r.district == d && matches!(r.response, RiotResponse::Contain | RiotResponse::Crush))
        .map(|r| r.target)
}

/// Per tick: a riot not resolved by `muster_at + RAID_MARCH_TICKS` fizzles.
pub fn run(world: &mut World) {
    if world.riots.is_empty() {
        return;
    }
    let now = world.tick;
    let stale: Vec<u32> =
        world.riots.iter().filter(|r| now >= r.muster_at + faction::RAID_MARCH_TICKS).map(|r| r.id).collect();
    for id in stale {
        finish(world, id, "fizzled", 0, 0, 0, None);
    }
}

/// D30, at midnight after the unrest pass: every district (ascending) with
/// `unrest_streak ≥ riot_days`, no riot within `riot_cooldown_days`, none
/// live there, fewer than `max_active_riots` city-wide, riots.
pub fn trigger(world: &mut World) {
    if !enabled(world) {
        return;
    }
    let cfg = world.config.riots.clone();
    let now = world.tick;
    for i in 0..world.districts.len() {
        if world.riots.len() >= cfg.max_active_riots {
            break;
        }
        let d = &world.districts[i];
        let cooled = d.last_riot.is_none_or(|t| now.saturating_sub(t) >= cfg.riot_cooldown_days * TICKS_PER_DAY);
        if d.unrest_streak < cfg.riot_days || !cooled || riot_in(world, d.id) {
            continue;
        }
        let _ = start(world, DistrictId(i as u8), false);
    }
}

/// Eligible rioters of `d` (riot tables): resident adults (binned to `d` by
/// the aggregate pass) of the Street or Dreg class, free, not emigrating,
/// no guard, no gang member (a gang marches on its own orders; plan
/// decision), lawfulness below `riot_lawfulness`, mood below 0. Most
/// miserable first, ties lower id; at most `riot_max`.
pub fn eligible(world: &World, d: DistrictId) -> Vec<EntityId> {
    let cfg = &world.config.riots;
    let execs = crate::systems::classes::exec_set(world);
    let Some(dist) = world.districts.get(d.index()) else { return Vec::new() };
    let mut out: Vec<(f32, EntityId)> = dist
        .residents
        .iter()
        .copied()
        .filter(|&a| law::living(world, a) && !world.has::<Sentence>(a) && !world.has::<GangMember>(a))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| !b.emigrating && b.cuffed_by.is_none()))
        .filter(|&a| !law::is_guard(world, a))
        .filter(|&a| crate::systems::classes::class_in(world, a, &execs) != crate::components::Class::Corp)
        .filter(|&a| world.comp::<Personality>(a).is_some_and(|p| p.lawfulness < cfg.riot_lawfulness))
        .filter_map(|a| world.comp::<Mood>(a).filter(|m| m.value < 0.0).map(|m| (m.value, a)))
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    out.into_iter().take(cfg.riot_max).map(|(_, a)| a).collect()
}

/// The highest `price_level` a corp charges.
fn price_level(world: &World, corp: EntityId) -> f32 {
    world.comp::<Corp>(corp).map_or(1.0, |c| c.price_level.values().copied().fold(1.0f32, f32::max))
}

/// D30 riot tables: the target with the highest grievance (ties lower
/// building id). A corp-owned Market, Block or Hotel in `d`: its owner's
/// evictions in `d` in 14 days + `(price_level − 1) × 10`. The Precinct:
/// `d`'s Vagrancy fines and sentences in 14 days, × 2 under Crackdown or
/// Sweep there. The controlling gang's Hideout (or a squat it holds) in
/// `d`: its shakedowns there in 14 days. All zero: the Precinct when the
/// city logged any Vagrancy in 14 days, else the highest-priced corp
/// building in `d`, else none.
pub fn score_targets(world: &World, d: DistrictId) -> Option<(EntityId, RiotTarget, f32)> {
    let dist = world.districts.get(d.index())?;
    let now = world.tick;
    let since = now.saturating_sub(GRIEVANCE_DAYS * TICKS_PER_DAY);
    let mut best: Option<(f32, EntityId, RiotTarget)> = None;
    let mut consider = |score: f32, b: EntityId, kind: RiotTarget| {
        if best.is_none_or(|(s, id, _)| score > s || (score == s && b < id)) {
            best = Some((score, b, kind));
        }
    };
    let mut priciest: Option<(f32, EntityId)> = None;
    for &b in &dist.buildings {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if bd.demolished
            || bd.derelict
            || !matches!(bd.kind, BuildingKind::Market | BuildingKind::Home | BuildingKind::Hotel)
        {
            continue;
        }
        let Some(corp) = world.corp_of_building(b) else { continue };
        let evictions = world
            .eviction_places
            .iter()
            .filter(|&&(t, ed, owner)| t >= since && ed == d && owner == Some(corp))
            .count() as f32;
        let level = price_level(world, corp);
        consider(evictions + (level - 1.0) * 10.0, b, RiotTarget::Corp);
        if priciest.is_none_or(|(l, id)| level > l || (level == l && b < id)) {
            priciest = Some((level, b));
        }
    }
    let jail = world.building_of_kind(BuildingKind::Jail);
    if let Some(j) = jail {
        let vag = dist.vagrancy_log.iter().filter(|&&t| t >= since).count() as f32;
        let mult = if matches!(dist.stance, Stance::Crackdown(_) | Stance::Sweep) { 2.0 } else { 1.0 };
        consider(vag * mult, j, RiotTarget::Precinct);
    }
    if let crate::components::Controller::Gang(g) = dist.control {
        let place = world.hideout_of(g).filter(|&h| world.district_of_building(h) == d).or_else(|| {
            world.comp::<Gang>(g).and_then(|gg| {
                gg.territory
                    .iter()
                    .copied()
                    .find(|&h| world.district_of_building(h) == d && crate::systems::street::is_derelict(world, h))
            })
        });
        if let Some(h) = place {
            let shakedowns = dist.shakedowns.iter().filter(|&&(t, sg)| t >= since && sg == g).count() as f32;
            consider(shakedowns, h, RiotTarget::Gang);
        }
    }
    match best {
        Some((s, b, k)) if s > 0.0 => Some((b, k, s)),
        _ => {
            let any_vagrancy = world.districts.iter().any(|x| x.vagrancy_log.iter().any(|&t| t >= since));
            if let (true, Some(j)) = (any_vagrancy, jail) {
                Some((j, RiotTarget::Precinct, 0.0))
            } else {
                priciest.map(|(_, b)| (b, RiotTarget::Corp, 0.0))
            }
        }
    }
}

/// D30: the district's Bar or Market nearest its centroid, else its Hotel,
/// else the Block door nearest the centroid (ties lower id).
pub fn muster_building(world: &World, d: DistrictId) -> Option<EntityId> {
    let dist = world.districts.get(d.index())?;
    let c = dist.centroid;
    let nearest = |kinds: &[BuildingKind]| {
        dist.buildings
            .iter()
            .copied()
            .filter_map(|b| world.comp::<Building>(b).map(|bd| (b, bd)))
            .filter(|(_, bd)| kinds.contains(&bd.kind) && !bd.demolished && !bd.derelict)
            .map(|(b, bd)| (bd.door.manhattan(c), b))
            .min()
            .map(|(_, b)| b)
    };
    nearest(&[BuildingKind::Bar, BuildingKind::Market])
        .or_else(|| nearest(&[BuildingKind::Hotel]))
        .or_else(|| nearest(&[BuildingKind::Home]))
}

/// D34: the captain's riot response: courage < 0.4 Contain; lawfulness <
/// 0.4 and courage ≥ 0.6 Crush; else Disperse. No captain: Contain.
pub fn response(world: &World) -> RiotResponse {
    // M12 D42: the player's pin wins.
    if let Some(r) = world.levers.riot_response {
        return r;
    }
    let Some(c) = world.law().and_then(|l| l.captain).filter(|&c| law::is_guard(world, c)) else {
        return RiotResponse::Contain;
    };
    let Some(p) = world.comp::<Personality>(c) else { return RiotResponse::Contain };
    if p.courage < 0.4 {
        RiotResponse::Contain
    } else if p.lawfulness < 0.4 && p.courage >= 0.6 {
        RiotResponse::Crush
    } else {
        RiotResponse::Disperse
    }
}

/// D30: start a riot in `d` (also the god command with `forced`: the streak,
/// cooldown and caps ignored, at least one rioter needed). Scores the
/// target, picks the muster, takes the rioters, schedules the muster at the
/// next `riot_muster_hour`. Returns the riot id or why not.
pub fn start(world: &mut World, d: DistrictId, forced: bool) -> Result<u32, String> {
    let cfg = world.config.riots.clone();
    if d.index() >= world.districts.len() {
        return Err("no such district".into());
    }
    if riot_in(world, d) {
        return Err("a riot is already live there".into());
    }
    let rioters = eligible(world, d);
    let need = if forced { 1 } else { cfg.riot_min };
    if rioters.len() < need {
        return Err(format!("{} eligible rioters (need {need})", rioters.len()));
    }
    let Some((target, kind, grievance)) = score_targets(world, d) else {
        return Err("nothing to riot against".into());
    };
    let Some(muster) = muster_building(world, d) else { return Err("nowhere to muster".into()) };
    let now = world.tick;
    let id = world.next_riot_id;
    world.next_riot_id += 1;
    let response = response(world);
    let muster_at = faction::next_muster(now, cfg.riot_muster_hour);
    for &a in &rioters {
        world.rioter_of.insert(a, id);
    }
    let n = rioters.len();
    world.riots.push(Riot {
        id,
        district: d,
        target,
        kind,
        muster,
        muster_at,
        rioters,
        response,
        started: now,
        departed: None,
    });
    let (dn, tn, mn) = (world.district_name(d).to_string(), world.name_of(target), world.name_of(muster));
    let hour = (muster_at % TICKS_PER_DAY) / TICKS_PER_HOUR;
    world.push_event(
        EventKind::Riot,
        &[target],
        format!(
            "{dn} is rising: {n} gather at {mn} at {hour}:00 against {tn} ({kind:?}, grievance {grievance:.1}; the law will {response:?})"
        ),
    );
    Ok(id)
}

/// D32: the first rioter at the target's door resolves the riot. Rioters
/// at the door: the riot's rioters within `raid_gather_radius` whose goal
/// is Raid, plus the actor; fewer than `riot_min ÷ 2` disperse without a
/// fight. Defenders: the city guards on the district's beat who are on
/// shift, or standing within the radius (or inside the target); the
/// target's private guards within the radius; the controlling gang's
/// members inside a gang target. Won (nobody left standing): looting. Lost:
/// each rioter who lost a pairing and lived is jailed at once (Assault).
pub fn clash(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let id = *world.rioter_of.get(&actor)?;
    let riot = raid::riot_by_id(world, id)?.clone();
    let cfg = world.config.riots.clone();
    let door = world.comp::<Building>(riot.target)?.door;
    let mut rioters = raid::gathered(world, &riot.rioters, actor, door);
    let d = riot.district;
    let tname = world.name_of(riot.target);
    if rioters.len() < (cfg.riot_min / 2).max(1) {
        let n = rioters.len();
        finish(world, id, "dispersed", n, 0, 0, Some(actor));
        return Some(Outcome::Lost);
    }
    let radius = world.config.gangs.raid_gather_radius;
    let tod = world.tick_of_day();
    let target = riot.target;
    let near = |w: &World, a: EntityId| {
        w.comp::<Position>(a).is_some_and(|p| p.building == Some(target) || law::chebyshev(p.tile, door) <= radius)
    };
    let beat: Vec<EntityId> =
        world.law().map(|l| l.beats.iter().filter(|(_, &bd)| bd == d).map(|(&g, _)| g).collect()).unwrap_or_default();
    let free = |w: &World, g: EntityId| law::living(w, g) && !w.has::<Sentence>(g);
    let mut defenders: Vec<EntityId> =
        law_brain::guards(world).into_iter().filter(|&g| free(world, g) && near(world, g)).collect();
    // The beat's on-shift guards answer the riot; how many is the law's
    // response strength, scaled by its alertness (the M16 hook, 1.0 in M12).
    let answering: Vec<EntityId> = law_brain::guards(world)
        .into_iter()
        .filter(|&g| free(world, g) && !defenders.contains(&g))
        .filter(|&g| beat.contains(&g) && world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod)))
        .collect();
    let k = (answering.len() as f32 * faction::alertness_mult(world, None)).round().max(0.0) as usize;
    defenders.extend(answering.into_iter().take(k));
    for g in raid::private_guards_of(world, target) {
        if free(world, g) && near(world, g) && !defenders.contains(&g) {
            defenders.push(g);
        }
    }
    // Fix pass: a corp target's posted guards (as a corp raid's) hold its door.
    for g in raid::posted_guards(world, target) {
        if !defenders.contains(&g) {
            world.abort_plan(g);
            world.stand_at_door(g, target);
            defenders.push(g);
        }
    }
    if riot.kind == RiotTarget::Gang {
        let gang = world.comp::<Building>(target).and_then(|_| match world.district(d).control {
            crate::components::Controller::Gang(g) => Some(g),
            _ => None,
        });
        if let Some(g) = gang {
            let inside: Vec<EntityId> = world
                .comp::<Gang>(g)
                .map(|gg| gg.members.clone())
                .unwrap_or_default()
                .into_iter()
                .filter(|&m| {
                    !world.has::<Sentence>(m) && world.comp::<Position>(m).is_some_and(|p| p.building == Some(target))
                })
                .collect();
            for m in inside {
                if !defenders.contains(&m) {
                    defenders.push(m);
                }
            }
        }
    }
    defenders.retain(|x| !rioters.contains(x));
    raid::by_strength(world, &mut defenders);
    let kill_mult = if riot.response == RiotResponse::Crush { cfg.crush_kill_mult } else { 1.0 };
    let (n, m) = (rioters.len(), defenders.len());
    let all_defenders = defenders.clone();
    let place = format!("{tname} in {}", world.district_name(d));
    let tally = raid::fight_out(world, &mut rioters, &mut defenders, door, &place, kill_mult, Some(d));
    let outcome = if defenders.is_empty() { Outcome::Won } else { Outcome::Lost };
    let now = world.tick;
    if outcome == Outcome::Won {
        let standing: Vec<EntityId> = rioters.iter().copied().filter(|&r| law::living(world, r)).collect();
        loot(world, &riot, &standing, actor);
    } else if let Some(jail) = world.building_of_kind(BuildingKind::Jail) {
        let until = now + law::sentence_ticks(world, Crime::Assault);
        for r in tally.losers.iter().copied() {
            if law::living(world, r) && !world.has::<Sentence>(r) {
                law::sentence(world, r, Crime::Assault, until, jail);
            }
        }
    }
    match riot.response {
        RiotResponse::Disperse => disperse(world, &riot, door, &all_defenders),
        RiotResponse::Crush => {
            if let Some(x) = world.districts.get_mut(d.index()) {
                x.crush_until = Some(now + CRUSH_DAYS * TICKS_PER_DAY);
            }
        }
        RiotResponse::Contain => {}
    }
    let verdict = if outcome == Outcome::Won { "won" } else { "lost" };
    finish(world, id, verdict, n, m, tally.deaths, Some(actor));
    Some(outcome)
}

/// D34 Disperse: every free rioter left within 8 tiles of the door is
/// reported for Assault, the nearest defending guard the witness, and seen
/// there (the Arrest goal chases them while the warrant lives).
fn disperse(world: &mut World, riot: &Riot, door: crate::components::TilePos, defenders: &[EntityId]) {
    let now = world.tick;
    let guards: Vec<(crate::components::TilePos, EntityId)> = defenders
        .iter()
        .copied()
        .filter(|&g| law::is_guard(world, g) && law::living(world, g))
        .filter_map(|g| world.comp::<Position>(g).map(|p| (p.tile, g)))
        .collect();
    for &r in &riot.rioters {
        if !law::living(world, r) || world.has::<Sentence>(r) {
            continue;
        }
        let Some(tile) = world.comp::<Position>(r).map(|p| p.tile) else { continue };
        if law::chebyshev(tile, door) > DISPERSE_TILES {
            continue;
        }
        let witness = guards.iter().map(|&(t, g)| (t.manhattan(tile), g)).min().map(|(_, g)| g);
        law::file_report(world, Crime::Assault, r, witness);
        world.last_seen.insert(r, (tile, now));
    }
}

/// D32 / riot tables, a won riot: a Market loses `loot_frac` of its stock
/// into the rioters' inventories (stolen food, round-robin); a Block or
/// Hotel's owner loses `loot_frac` × the building's 7-day revenue, split
/// evenly into the rioters' wallets (`Flow::Robbery`); the Precinct frees
/// `breakout_max_freed` convicts, the longest remaining sentences first; a
/// gang Hideout loses `loot_frac` of the treasury and of the stock (plan
/// decision: the tables name no Hideout loot). The target is closed for
/// `riot_close_days`; a `Looted` event.
fn loot(world: &mut World, riot: &Riot, rioters: &[EntityId], actor: EntityId) {
    let cfg = world.config.riots.clone();
    let now = world.tick;
    let target = riot.target;
    let Some((kind, stock, revenue)) =
        world.comp::<Building>(target).map(|b| (b.kind, b.stock_food, b.revenue.iter().sum::<i64>()))
    else {
        return;
    };
    let took = match kind {
        BuildingKind::Jail => {
            let mut convicts: Vec<(Tick, EntityId)> = world
                .sentenced()
                .iter()
                .copied()
                .filter_map(|m| world.comp::<Sentence>(m).map(|s| (s.until_tick.saturating_sub(now), m)))
                .collect();
            convicts.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            let freed: Vec<EntityId> =
                convicts.into_iter().take(world.config.gangs.breakout_max_freed).map(|(_, m)| m).collect();
            for &m in &freed {
                law::escape(world, m);
            }
            format!("{} convicts freed", freed.len())
        }
        BuildingKind::Hideout => {
            let gang = world.gang_list().iter().copied().find(|&g| world.hideout_of(g) == Some(target));
            let coins = gang
                .and_then(|g| world.comp::<Gang>(g))
                .map_or(0, |g| (g.treasury.max(0) as f32 * cfg.loot_frac).floor() as i64);
            if let Some(g) = gang {
                let each = coins / rioters.len().max(1) as i64;
                for &r in rioters {
                    crate::systems::ownership::pay(
                        world,
                        Some(g),
                        Some(r),
                        each,
                        crate::systems::ownership::Flow::Robbery,
                    );
                }
            }
            let units = (stock as f32 * cfg.loot_frac).floor() as u32;
            if let Some(b) = world.comp_mut::<Building>(target) {
                b.stock_food -= units;
            }
            raid::share_food(world, rioters, units);
            format!("{coins} coins and {units} food")
        }
        BuildingKind::Market => {
            let units = (stock as f32 * cfg.loot_frac).floor() as u32;
            if let Some(b) = world.comp_mut::<Building>(target) {
                b.stock_food -= units;
            }
            raid::share_food(world, rioters, units);
            format!("{units} food")
        }
        _ => {
            let owner = world.owner_of(target);
            let total = (revenue.max(0) as f32 * cfg.loot_frac).floor() as i64;
            let each = total / rioters.len().max(1) as i64;
            let mut moved = 0;
            for &r in rioters {
                moved += crate::systems::ownership::charge(
                    world,
                    owner,
                    Some(r),
                    each,
                    crate::systems::ownership::Flow::Robbery,
                );
            }
            if moved > 0 {
                crate::systems::ownership::note_loss(world, target, moved, Some(actor));
            }
            format!("{moved} coins")
        }
    };
    // Fix pass (phase 4 review): only a Market, Bar or Hotel closes (nothing
    // reads a closed Block, Precinct or Hideout).
    if matches!(kind, BuildingKind::Market | BuildingKind::Bar | BuildingKind::Hotel) {
        if let Some(b) = world.comp_mut::<Building>(target) {
            b.closed_until = Some(now + cfg.riot_close_days * TICKS_PER_DAY);
        }
    }
    // Fix pass: a looted corp hardens like a raided one (`corp_brain`'s floor).
    if let Some(c) = world.corp_of_building(target) {
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.raided_at = Some(now);
        }
    }
    let tn = world.name_of(target);
    let dn = world.district_name(riot.district).to_string();
    let closed = if world.is_closed(target) { format!("; closed {} days", cfg.riot_close_days) } else { String::new() };
    world.push_event(EventKind::Looted, &[target, actor], format!("rioters from {dn} looted {tn}: {took}{closed}"));
}

/// D32's tail, whatever the outcome: the `Riot` event, and (a riot that
/// reached its door) litter 64 r3 at the door, unrest × `riot_vent`, the
/// streak reset, `last_riot`, `CorpShock::Rioted` on the target's corp and
/// `RiotNearby` on every other corp owning buildings in the district, the
/// `riots` counter. A fizzled riot sets only the cooldown and streak.
fn finish(world: &mut World, id: u32, outcome: &str, n: usize, m: usize, dead: usize, actor: Option<EntityId>) {
    let Some(pos) = world.riots.iter().position(|r| r.id == id) else { return };
    let riot = world.riots.remove(pos);
    for a in &riot.rioters {
        if world.rioter_of.get(a) == Some(&id) {
            world.rioter_of.remove(a);
        }
    }
    let now = world.tick;
    let d = riot.district;
    let dn = world.district_name(d).to_string();
    let tn = world.name_of(riot.target);
    let fizzled = outcome == "fizzled";
    // Fix pass (phase 4 review): a crowd too thin to fight at the door is a
    // fizzle too: the cooldown and the streak, no vent, mess, shock or count.
    let dispersed = outcome == "dispersed";
    let vent = world.config.riots.riot_vent;
    if let Some(x) = world.districts.get_mut(d.index()) {
        x.unrest_streak = 0;
        x.last_riot = Some(now);
        if !fizzled && !dispersed {
            x.unrest *= vent;
        }
    }
    let mut actors = vec![riot.target];
    actors.extend(actor);
    if fizzled {
        world.push_event(
            EventKind::Riot,
            &actors,
            format!("{dn}'s riot against {tn} fizzled: nobody reached the door"),
        );
        return;
    }
    if dispersed {
        world.push_event(
            EventKind::Riot,
            &actors,
            format!("{dn}'s riot against {tn} dispersed: {n} at the door, too few to fight"),
        );
        return;
    }
    world.stats.current.riots += 1;
    if let Some(door) = world.comp::<Building>(riot.target).map(|b| b.door) {
        crate::systems::litter::deposit(world, door, 64, 3);
    }
    let target_corp = world.corp_of_building(riot.target);
    if let Some(c) = target_corp {
        crate::systems::ownership::push_corp_shock(world, c, CorpShock::Rioted);
    }
    let mut others: Vec<EntityId> = world
        .district(d)
        .buildings
        .iter()
        .filter_map(|&b| world.corp_of_building(b))
        .filter(|&c| Some(c) != target_corp)
        .collect();
    others.sort_unstable();
    others.dedup();
    for c in others {
        crate::systems::ownership::push_corp_shock(world, c, CorpShock::RiotNearby);
    }
    world.push_event(
        EventKind::Riot,
        &actors,
        format!("{dn} rioted at {tn}: {outcome} ({n} rioters vs {m} defenders, {dead} dead)"),
    );
}

/// LOD (plan risk 3): a rioter of a live riot ranks with the gang members
/// from `riot_promote_hours` before the muster until the riot ends (then the
/// hourly ranking demotes it). Fix pass: only the riot's first
/// `riot_promote_max` rioters (most miserable first; 0 = all) are promoted.
pub fn promoted(world: &World, agent: EntityId) -> bool {
    let Some(&id) = world.rioter_of.get(&agent) else { return false };
    let cfg = &world.config.riots;
    let hours = Tick::from(cfg.riot_promote_hours);
    raid::riot_by_id(world, id).is_some_and(|r| {
        world.tick + hours * TICKS_PER_HOUR >= r.muster_at
            && (cfg.riot_promote_max == 0 || r.rioters.iter().take(cfg.riot_promote_max).any(|&a| a == agent))
    })
}
