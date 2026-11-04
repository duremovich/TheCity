use super::*;

/// Muster complete: the expedition departs. Off when the order changed
/// meanwhile, or the target fell under full cover (the step fails and the
/// member replans). V8b (`OPEN-276`): a gang's muster holds for its column
/// (`Hold`) and breaks up short of it.
pub fn depart(world: &mut World, agent: EntityId) -> Departure {
    let now = world.tick;
    let gid = match expedition_of(world, agent) {
        Some(Expedition::Riot(id)) => {
            let Some(r) = world.riots.iter_mut().find(|r| r.id == id) else { return Departure::Off };
            if r.muster_at > now {
                return Departure::Off;
            }
            r.departed.get_or_insert(now);
            return Departure::Go;
        }
        Some(Expedition::Gang(g)) => g,
        // M16a (plan C22, C24): the strike decision.
        Some(Expedition::Mission(id)) => {
            return if crate::systems::missions::depart(world, agent, id) { Departure::Go } else { Departure::Off };
        }
        None => return Departure::Off,
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
            return Departure::Off;
        }
    }
    let Some(due) = world.comp::<Gang>(gid).and_then(|g| g.raid_at).filter(|&t| t <= now) else {
        return Departure::Off;
    };
    // V8b (`OPEN-276`): the column forms at the muster before it leaves: the
    // muster holds past its hour until `raid_min_members` stand there, and
    // breaks up short of them (20k: the lone member who came marched the
    // island to the door and called it off "1 at the door"). The march
    // clock starts when it leaves.
    if world.comp::<Gang>(gid).is_some_and(|g| !left(g)) {
        if let Some(hold) = hold_ticks(world) {
            let came = at_muster(world, gid, agent);
            if came.len() < world.config.gangs.raid_min_members.max(1) {
                if now < due + hold {
                    return Departure::Hold;
                }
                broke_up(world, gid, &came);
                return Departure::Off;
            }
            // The time model T1 (`OPEN-304`): the column waits at the muster,
            // inside its hold, for a member on his way there and due within
            // `[crew] march_wait_ticks` (at 3.5 tiles a minute the stragglers
            // trailed the column to the door and found it over: raid_missed
            // 14 -> 69 at 20k s42 x 30).
            if now < due + hold && muster_tail_due(world, gid, agent, &came) {
                return Departure::Hold;
            }
            if now > due {
                crate::systems::ownership::jobs2_row(world).raid_held += 1;
                if let Some(g) = world.comp_mut::<Gang>(gid) {
                    g.raid_at = Some(now);
                }
            }
        }
        if let Some(g) = world.comp_mut::<Gang>(gid) {
            g.raid_left = Some(now);
        }
    }
    let Some(g) = world.comp_mut::<Gang>(gid) else { return Departure::Off };
    // The first member out the door stamps the departure; the rest follow it.
    let mut first = false;
    if g.order.target_is_jail() {
        if g.last_breakout_tick.is_none_or(|t| t + TICKS_PER_DAY < now) {
            g.last_breakout_tick = Some(now);
            first = true;
        }
    } else if g.last_raid_tick.is_none_or(|t| t + TICKS_PER_DAY < now) {
        g.last_raid_tick = Some(now);
        first = true;
    }
    // M14 V37: the stream (and the V32 door-then-departure log).
    if first {
        crate::systems::virt::on_raid_departed(world, gid);
        crate::systems::ownership::jobs2_row(world).raid_departed += 1;
    }
    // Jobs & lives phase 3 (section 4): the member draws from the armoury.
    crate::systems::weapons::issue(world, agent);
    // Round 3 S2 (OPEN-222): he sat the muster and marched with it; his
    // duty is kept whether or not the march reaches the door.
    crate::systems::crew::sat_muster(world, gid, agent);
    Departure::Go
}

/// `Brawl` at the expedition's door: a riot's clash, a breach of the Jail
/// under BreakOut, a corp building under a corp Raid, else a raid on the
/// rival Hideout.
pub fn resolve(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let gid = match expedition_of(world, actor)? {
        Expedition::Riot(_) => return crate::systems::riot::clash(world, actor),
        Expedition::Gang(g) => g,
        // M16a (plan C23): the mission's brawl.
        Expedition::Mission(_) => return crate::systems::missions::brawl(world, actor),
    };
    // The pressure pass phase 5b: a marcher arriving after the brawl reads
    // its result.
    if world.comp::<Gang>(gid).is_some_and(|g| g.raid_at.is_none()) {
        crate::systems::crew::late_to_raid(world, actor, gid);
        return None;
    }
    // C3a: the brawl is being fought (a live engagement). C3b: a marcher
    // who comes to the door now joins it (a body not Full stays out).
    if raid_fighting(world, gid) {
        if crate::systems::combat::join_raid(world, gid, actor) {
            let text = format!("{} came late to the door and joined the fight", world.name_of(actor));
            world.push_event(EventKind::Raid, &[actor, gid], text);
        }
        return None;
    }
    // Shadow V4 bug 9: a crew under `raid_min_members` at the door once the
    // wait is over calls the raid off (lone raids, 1 against 6, after an
    // 8-h march).
    let min = world.config.gangs.raid_min_members;
    if min > 0 && world.comp::<Gang>(gid).is_some_and(|g| g.raid_at.is_some()) {
        let door = gang_target(world, gid).and_then(|b| world.comp::<Building>(b)).map(|b| b.door);
        let present = door.map_or(0, |d| raiders_at(world, gid, actor, d).len());
        if present < min {
            crate::systems::ownership::jobs2_row(world).raid_at_target += 1;
            let gname = world.comp::<Gang>(gid).map_or_else(String::new, |g| g.name.clone());
            if let Some(g) = world.comp_mut::<Gang>(gid) {
                g.raid_at = None;
            }
            world.push_event(
                EventKind::Raid,
                &[gid],
                format!("{gname}'s raid called off: {present} at the door, too few to go in"),
            );
            faction::rethink(world, gid);
            return None;
        }
    }
    // M12 D38: count an expedition that reached a door under full cover.
    if let Some(order) = world.comp::<Gang>(gid).filter(|g| g.raid_at.is_some()).map(|g| g.order) {
        if order.is_raid() && faction::target_cover(world, gid, order) >= 1.0 {
            world.stats.current.raids_into_cover += 1;
        }
    }
    // Lives at work phase 6 (§ 4.5): the raid is resolved in its head's name
    // (the leader at the door, else the best lieutenant there; the actor
    // off), and the head's ladder counts it.
    let raiding = world.comp::<Gang>(gid).is_some_and(|g| g.raid_at.is_some());
    // The pressure pass phase 5b: the muster kept by those at the door.
    if raiding && crate::systems::crew::on(world) {
        let door = gang_target(world, gid).and_then(|b| world.comp::<Building>(b)).map(|b| b.door);
        let present = door.map_or_else(Vec::new, |d| raiders_at(world, gid, actor, d));
        crate::systems::crew::mustered(world, gid, &present);
        // Round 7 W2 (`OPEN-241`): the column when it went in.
        let pool = world.comp::<Gang>(gid).map(|g| g.members.clone()).unwrap_or_default();
        let marching = marchers(world, &pool).max(present.len());
        let row = crate::systems::ownership::jobs2_row(world);
        row.raid_marchers += marching as u32;
        row.raid_at_door += present.len() as u32;
        row.raid_at_target += 1;
    }
    let head = if raiding && crate::systems::careers::lieutenants_on(world) {
        let door = gang_target(world, gid).and_then(|b| world.comp::<Building>(b)).map(|b| b.door);
        let present = door.map_or_else(Vec::new, |d| raiders_at(world, gid, actor, d));
        crate::systems::careers::raid_head(world, gid, actor, &present)
    } else {
        actor
    };
    let outcome = if own_gang(world, head).is_some_and(|g| g.order.target_is_jail()) {
        breach(world, head)
    } else if corp_target(world, gid).is_some() {
        corp_brawl(world, head)
    } else {
        brawl(world, head)
    };
    if raiding {
        if let Some(o) = outcome {
            crate::systems::careers::credit_raid(world, head, matches!(o, Outcome::Won | Outcome::Sacked));
        }
    }
    outcome
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
        // M16a (plan C22): the gang arm's rule over the crew, goal Contract.
        Some(Expedition::Mission(id)) => return crate::systems::missions::wait_for_crew(world, actor, id),
        None => return false,
    };
    if now + TICKS_PER_HOUR >= depart + faction::march_ticks(world) {
        return false;
    }
    let Some(door) = target.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) else { return false };
    let marching = marchers(world, &pool);
    let present = gathered(world, &pool, actor, door);
    if present.len() < cap.min(marching) {
        return true;
    }
    // Round 7 W2 (`OPEN-241`): a gang's column waits for its tail, a
    // marcher on the march to the door due there within `[crew]
    // march_wait_ticks` (V15: the first three went in; the rest of the
    // column, a Coarse body's timed walk or a Full body's steps behind,
    // found it over and walked off).
    let Some(Expedition::Gang(_)) = expedition_of(world, actor) else { return false };
    let Some(wait) = crate::systems::crew::cfg(world).map(|c| c.march_wait_ticks).filter(|&t| t > 0) else {
        return false;
    };
    pool.iter().any(|&m| {
        !present.contains(&m)
            && law::living(world, m)
            && !world.has::<Sentence>(m)
            && !crate::systems::ages::is_protected(world, m)
            && march_eta(world, m, door).is_some_and(|eta| eta <= wait)
    })
}

/// T1 (`OPEN-304`): a member of `gid` not yet at the muster (`came`), on
/// his Raid plan's walk to it (`GoTo(MusterPoint)`), due there within
/// `[crew] march_wait_ticks` (a timed walk's arrival, a Full body's tiles
/// at the walking pace).
fn muster_tail_due(world: &World, gid: EntityId, actor: EntityId, came: &[EntityId]) -> bool {
    let Some(wait) = crate::systems::crew::cfg(world).map(|c| c.march_wait_ticks).filter(|&t| t > 0) else {
        return false;
    };
    let door = match muster_point(world, actor) {
        Some(MusterAt::Inside(h)) => world.comp::<Building>(h).map(|b| b.door),
        Some(MusterAt::Door(t)) => Some(t),
        None => None,
    };
    let Some(door) = door else { return false };
    let members = world.comp::<Gang>(gid).map(|g| g.members.clone()).unwrap_or_default();
    members.iter().any(|&m| {
        !came.contains(&m)
            && law::living(world, m)
            && !world.has::<Sentence>(m)
            && !crate::systems::ages::is_protected(world, m)
            && eta_on(world, m, crate::goap::LocationKey::MusterPoint, door).is_some_and(|eta| eta <= wait)
    })
}

/// Round 7 W2: the ticks until `m`, on his Raid plan's march to the door
/// (`GoTo(RaidTarget)`), is there: a timed walk's arrival, a Full body's
/// tiles at the walking pace; `None` off the march.
fn march_eta(world: &World, m: EntityId, door: TilePos) -> Option<Tick> {
    eta_on(world, m, crate::goap::LocationKey::RaidTarget, door)
}

/// The ticks until `m`, on his Raid plan's `GoTo(key)` to `door`, is there.
fn eta_on(world: &World, m: EntityId, key: crate::goap::LocationKey, door: TilePos) -> Option<Tick> {
    let b = world.comp::<Brain>(m)?;
    if b.current_goal != Some(GoalKind::Raid) {
        return None;
    }
    let step = b.plan.as_ref()?.steps.get(usize::from(b.plan_step))?.action;
    if step != crate::goap::ActionKind::GoTo(key) {
        return None;
    }
    match &b.exec {
        crate::exec::ExecState::GotoTimed { arrive_tick, .. } => Some(arrive_tick.saturating_sub(world.tick)),
        crate::exec::ExecState::Goto { .. } => {
            let p = world.comp::<Position>(m)?;
            Some(world.config.exec.walk_ticks(Tick::from(p.tile.manhattan(door))))
        }
        _ => None,
    }
}

/// Of `pool`, the free living agents whose goal is Raid (the column).
fn marchers(world: &World, pool: &[EntityId]) -> usize {
    pool.iter()
        .filter(|&&m| {
            law::living(world, m)
                && !world.has::<Sentence>(m)
                && world.comp::<Brain>(m).is_some_and(|b| b.current_goal == Some(GoalKind::Raid))
        })
        .count()
}

/// The raiders present at a door: the actor, plus every fit member of the
/// gang within `raid_gather_radius` whose goal is Raid, strongest first.
pub(super) fn raiders_at(world: &World, gid: EntityId, actor: EntityId, door: TilePos) -> Vec<EntityId> {
    let members = world.comp::<Gang>(gid).map(|g| g.members.clone()).unwrap_or_default();
    gathered(world, &members, actor, door)
}

/// Of `pool`, the actor and every free agent within `raid_gather_radius` of
/// the door whose goal is Raid, strongest first (riots reuse it, D32).
pub fn gathered(world: &World, pool: &[EntityId], actor: EntityId, door: TilePos) -> Vec<EntityId> {
    gathered_for(world, pool, actor, door, GoalKind::Raid)
}

/// [`gathered`] for an expedition whose marchers pursue `goal` (M16a plan
/// C22: a mission's crew is under `GoalKind::Contract`).
pub fn gathered_for(world: &World, pool: &[EntityId], actor: EntityId, door: TilePos, goal: GoalKind) -> Vec<EntityId> {
    let radius = world.config.gangs.raid_gather_radius;
    let mut out: Vec<EntityId> = pool
        .iter()
        .copied()
        .filter(|&m| !world.has::<Sentence>(m) && law::living(world, m))
        // Round 3 V12 fix: a minor never marches into a brawl.
        .filter(|&m| !crate::systems::ages::is_protected(world, m))
        .filter(|&m| {
            m == actor
                // V8 (`OPEN-264`): or inside the building at the door (on
                // his climb to the target's floor, or on it).
                || (world.comp::<Position>(m).is_some_and(|p| {
                    law::chebyshev(p.tile, door) <= radius
                        || p.building.and_then(|b| world.comp::<Building>(b)).is_some_and(|bd| bd.door == door)
                })
                    && world.comp::<Brain>(m).is_some_and(|b| b.current_goal == Some(goal)))
        })
        .collect();
    by_strength(world, &mut out);
    out
}
