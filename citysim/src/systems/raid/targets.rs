use super::*;

/// M14 V34: the newest pending `Shock::Hacked { by }` on `gang` naming
/// another living gang (a traced run on its nodes): the gang a Retaliate
/// chosen now fights. Read whenever Retaliate is chosen, whatever shock
/// triggered it, so a stale pending `Hacked` naming a third gang outranks
/// the `Raided` that set the order off (newest Hacked wins; deterministic,
/// accepted).
pub fn hack_grudge(world: &World, gang: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    g.shocks.iter().rev().find_map(|s| match *s {
        Shock::Hacked { by: Some(b) } if b != gang && world.has::<Gang>(b) => Some(b),
        _ => None,
    })
}

/// The gang a raid of `gang`'s fights: under Retaliate the gang a traced
/// run named when the order was chosen (`Gang.retaliate_on`, M14 V34), else
/// `World::rival_of`.
pub fn raid_rival(world: &World, gang: EntityId) -> Option<EntityId> {
    let named = world
        .comp::<Gang>(gang)
        .filter(|g| g.order == Order::Retaliate)
        .and_then(|g| g.retaliate_on)
        .filter(|&r| r != gang && world.has::<Gang>(r));
    // V8 (`OPEN-264`): a raid under way fights the gang it was ordered on, the
    // one whose Hideout it picked (`Gang.raid_hideout`), while that stands
    // (V19: the neighbour changed under the march, and the plan named one
    // Hideout while the climb went up another).
    let ordered = || {
        let g = world.comp::<Gang>(gang)?;
        let h = g.raid_hideout.filter(|_| g.raid_at.is_some())?;
        if world.comp::<Building>(h).is_none_or(|b| b.demolished) {
            return None;
        }
        world.gang_list().iter().copied().find(|&o| o != gang && crate::systems::vertical::is_hideout_of(world, o, h))
    };
    named.or_else(ordered).or_else(|| world.rival_of(gang))
}

/// The city milestone C9b: the rival Hideout `gang`'s standing raid on
/// `rival` lands on: the one chosen when it was ordered (`Gang.raid_hideout`,
/// `vertical::raid_pick`) while it stands and is `rival`'s, else the seat.
pub fn rival_target(world: &World, gang: EntityId, rival: EntityId) -> Option<EntityId> {
    world
        .comp::<Gang>(gang)
        .and_then(|g| g.raid_hideout)
        .filter(|&h| {
            world.comp::<Building>(h).is_some_and(|b| !b.demolished)
                && crate::systems::vertical::is_hideout_of(world, rival, h)
        })
        .or_else(|| world.hideout_of(rival))
}

/// The street tile outside the rival Hideout's door.
pub fn rival_hideout_tile(world: &World, agent: EntityId) -> Option<TilePos> {
    let gang = world.gang_of(agent)?;
    let rival = raid_rival(world, gang)?;
    let hideout = rival_target(world, gang, rival)?;
    world.comp::<Building>(hideout).map(|b| world.outside_door(b))
}

/// The street tile outside the Jail door.
pub fn jail_tile(world: &World) -> Option<TilePos> {
    let jail = world.building_of_kind(BuildingKind::Jail)?;
    world.comp::<Building>(jail).map(|b| world.outside_door(b))
}

/// D39: the corp building a gang's standing Raid aims at, while it stands;
/// M15 W18: or a standing Retaliate on a corp in vendetta with the gang.
pub fn corp_target(world: &World, gang: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    let retaliate = g.order == Order::Retaliate && g.retaliate_on.is_some_and(|c| world.has::<Corp>(c));
    if g.order != Order::Raid && !retaliate {
        return None;
    }
    g.raid_target.filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
}

/// M15 W18 (spec "the expedition's rival"): the faction a gang's
/// expedition fights: a standing Retaliate's named gang or corp
/// (`Gang.retaliate_on`), else `World::rival_of`. The gang-only readers
/// keep [`raid_rival`].
pub fn expedition_rival(world: &World, gang: EntityId) -> Option<EntityId> {
    let named = world
        .comp::<Gang>(gang)
        .filter(|g| g.order == Order::Retaliate)
        .and_then(|g| g.retaliate_on)
        .filter(|&r| r != gang && (world.has::<Gang>(r) || world.has::<Corp>(r)));
    named.or_else(|| world.rival_of(gang))
}

/// A gang's expedition target under its standing order: the Jail under
/// BreakOut, a corp building under a corp Raid, else the rival Hideout.
pub fn gang_target(world: &World, gang: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    if g.order.target_is_jail() {
        return world.building_of_kind(BuildingKind::Jail);
    }
    corp_target(world, gang).or_else(|| raid_rival(world, gang).and_then(|r| rival_target(world, gang, r)))
}

/// The building this agent's expedition ends at (the plan's bound target).
pub fn target_building(world: &World, agent: EntityId) -> Option<EntityId> {
    match expedition_of(world, agent)? {
        Expedition::Riot(id) => riot_by_id(world, id).map(|r| r.target),
        Expedition::Gang(g) => gang_target(world, g),
        // M16a (plan C22): the building at the mission's door, if any.
        Expedition::Mission(id) => world.missions.get(&id).and_then(|m| m.door_building),
    }
}

/// Where this agent's expedition ends (`LocationKey::RaidTarget`): the
/// street tile outside the target's door.
pub fn target_tile(world: &World, agent: EntityId) -> Option<TilePos> {
    // M16a (plan C22): the street tile outside the mission's door.
    if let Some(Expedition::Mission(id)) = expedition_of(world, agent) {
        return world.missions.get(&id).map(|m| m.door);
    }
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
            // Round 8 X4 (`OPEN-241`): the place chosen when the raid was
            // ordered, while it stands.
            if let Some(b) = world.comp::<Gang>(gid).and_then(|g| g.raid_muster) {
                if let Some(bd) = world.comp::<Building>(b).filter(|bd| !bd.demolished) {
                    return Some(if crate::systems::vertical::is_hideout_of(world, gid, b) {
                        MusterAt::Inside(b)
                    } else {
                        MusterAt::Door(world.outside_door(bd))
                    });
                }
            }
            // The city milestone C8: the gang's Hideout nearest the target (its departure).
            let target_door = gang_target(world, gid).and_then(|b| world.comp::<Building>(b)).map(|b| b.door);
            let hideout = match target_door {
                Some(t) => crate::systems::vertical::nearest_to(world, gid, t),
                None => world.hideout_of(gid),
            };
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
                        .min_by_key(|&(d, h, _)| (d, h));
                    // Round 7 W2 (`OPEN-241`): past `muster_near_tiles`, the
                    // held Home nearer the target than the Hideout (V15
                    // `cultist_62792 d21`: 293 tiles to the Hideout, then 220
                    // more toward a Precinct across the island, cut asleep).
                    let hideout_d = hideout.and_then(|h| world.comp::<Building>(h)).map(|b| b.door.manhattan(t));
                    if let Some((_, _, b)) = best.filter(|&(d, _, _)| d <= near || hideout_d.is_some_and(|x| d < x)) {
                        return Some(MusterAt::Door(world.outside_door(b)));
                    }
                }
            }
            hideout.map(MusterAt::Inside)
        }
        // M16a (plan C22): outside the mission's muster.
        Expedition::Mission(id) => world.missions.get(&id).map(|m| MusterAt::Door(m.muster)),
    }
}

/// Round 8 X4 (`OPEN-241`): where gang `gid`'s raid ordered now musters.
/// Of its held Homes within `muster_near_tiles` of the target's door (the
/// `[gangs] muster_candidates` nearest it) and its Hideout (when within it
/// too, or with no such Home), the place the
/// most of its fit adults can reach from home within `[crew]
/// duty_reach_ticks` (each one's own trip, `raid::muster_trip`'s), ties to
/// the nearer the door. V16 (100k): the muster was the held Home nearest the
/// target whoever lived near it, and the members sat it out "3 h 08 from
/// home" (`gang_leader_61823 d26 00:07`). `None` with `[crew]` or the
/// candidates off (the old rule). With the pick: its members within reach,
/// and those within reach of the old rule's place (the counters).
pub fn pick_muster(world: &World, gid: EntityId) -> Option<(EntityId, usize, usize)> {
    let reach = crate::systems::crew::cfg(world)?.duty_reach_ticks;
    let n = world.config.gangs.muster_candidates;
    let near = world.config.gangs.muster_near_tiles;
    if n == 0 {
        return None;
    }
    let g = world.comp::<Gang>(gid)?;
    let t = gang_target(world, gid).and_then(|b| world.comp::<Building>(b)).map(|b| b.door)?;
    // The old rule's place (the held Home nearest the door within
    // `muster_near_tiles` or nearer than the Hideout, else the Hideout).
    let hideout_at =
        world.hideout_of(gid).and_then(|h| world.comp::<Building>(h).map(|b| (b.door.manhattan(t), b.door)));
    let old = g
        .territory
        .iter()
        .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(t), h, b.door)))
        .min_by_key(|&(d, h, _)| (d, h))
        .filter(|&(d, _, _)| d <= near || hideout_at.is_some_and(|x| d < x.0))
        .map(|x| x.2)
        .or(hideout_at.map(|x| x.1));
    let mut cands: Vec<(u32, EntityId, TilePos)> = g
        .territory
        .iter()
        .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(t), h, b.door)))
        .filter(|&(d, _, _)| d <= near)
        .collect();
    cands.sort_unstable_by_key(|&(d, h, _)| (d, h));
    cands.truncate(n);
    // The Hideout, within `muster_near_tiles` of the door or with no held
    // Home there (the march from it must make the door in its window). The
    // city milestone C8: each of the gang's Hideouts within it, else its
    // nearest to the door.
    let hs: Vec<(u32, EntityId, TilePos)> = crate::systems::vertical::hideouts_of(world, gid)
        .into_iter()
        .filter_map(|h| world.comp::<Building>(h).map(|b| (b.door.manhattan(t), h, b.door)))
        .collect();
    let none_near = cands.is_empty();
    let mut within: Vec<(u32, EntityId, TilePos)> = hs.iter().copied().filter(|&(d, _, _)| d <= near).collect();
    if within.is_empty() && none_near {
        within.extend(
            crate::systems::vertical::nearest_to(world, gid, t).and_then(|h| hs.iter().copied().find(|x| x.1 == h)),
        );
    }
    cands.extend(within);
    // The fit adults' Home doors (a minor never marches: the under-18 rule).
    let homes: Vec<(EntityId, TilePos)> = g
        .members
        .iter()
        .copied()
        .filter(|&m| {
            law::living(world, m)
                && world.has::<Brain>(m)
                && !world.has::<Sentence>(m)
                && !crate::systems::ages::is_protected(world, m)
        })
        .filter_map(|m| crate::systems::contracts::home_door(world, m).map(|d| (m, d)))
        .collect();
    let within = |door: TilePos| {
        homes.iter().filter(|&&(m, from)| crate::systems::distance::trip_ahead(world, m, from, door) <= reach).count()
    };
    let (n_pick, _, pick) = cands
        .into_iter()
        .map(|(d, h, door)| (within(door), d, h))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)))?;
    Some((pick, n_pick, old.map_or(0, within)))
}

/// Pressure pass round 3, S1 (OPEN-221): the trip from where `agent` sets
/// out (his Home door with `from_home`, else where he stands) to his gang
/// expedition's muster (`muster_point`), with the place's label; `None` with
/// `[distance.choice]` or `[crew]` off, for a riot or a mission, or with no
/// muster. 0 inside it.
pub fn muster_trip(world: &World, agent: EntityId, from_home: bool) -> Option<(Tick, String)> {
    crate::systems::distance::choice(world)?;
    crate::systems::crew::cfg(world)?;
    if !matches!(expedition_of(world, agent)?, Expedition::Gang(_)) {
        return None;
    }
    let (to, label, inside) = match muster_point(world, agent)? {
        MusterAt::Door(t) => (t, format!("a door in {}", world.district_name(world.district_of(t))), None),
        MusterAt::Inside(h) => (world.comp::<Building>(h)?.door, world.name_of(h), Some(h)),
    };
    if from_home {
        let from = crate::systems::contracts::home_door(world, agent)
            .or_else(|| crate::systems::life::origin(world, agent))?;
        return Some((crate::systems::distance::trip_ahead(world, agent, from, to), label));
    }
    if inside.is_some() && world.comp::<Position>(agent).and_then(|p| p.building) == inside {
        return Some((0, label));
    }
    let from = crate::systems::life::origin(world, agent)?;
    Some((crate::systems::distance::trip(world, agent, from, to), label))
}

/// S1 (OPEN-221): the Raid goal's reach gate: a gang member does not set
/// out for a muster past `[crew] duty_reach_ticks` from where he stands (V11:
/// a leader's 330- and 630-min walks to his own muster, both cut, "missed").
/// A member already on his Raid plan is not re-read (the march carries him
/// away from the muster). True off.
pub fn muster_in_reach(world: &World, agent: EntityId) -> bool {
    if world.comp::<crate::components::Brain>(agent).and_then(|b| b.plan_goal())
        == Some(crate::components::GoalKind::Raid)
    {
        return true;
    }
    let Some(c) = crate::systems::crew::cfg(world) else { return true };
    muster_trip(world, agent, false).is_none_or(|(t, _)| t <= c.duty_reach_ticks)
}
