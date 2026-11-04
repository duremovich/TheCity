use super::*;

/// The Home a GangWork plan extorts and the order it serves (`None` =
/// freelancing). Expand: the unclaimed Home nearest the gang's Hideout;
/// Contest: the Home in the rival's territory nearest it; Raid, Retaliate
/// and LieLow: none. M13 D38: a dealer under any order it follows deals
/// at its gang's deal Bar instead (`stims::dealer_target`). A member below `freelance_loyalty` ignores the order
/// and takes the unclaimed Home nearest themselves. Always: inhabited right
/// now, not the actor's own Home, no guard within `sight_day_crime` of the
/// door. "Unclaimed" and "rival's" read the territory lists, the same
/// definition `claim` flips on, so a contested Home whose claim has been
/// reset still reads as the rival's until the third blow lands.
pub fn gang_work_target(world: &World, id: EntityId) -> Option<(EntityId, Option<Order>)> {
    // Round 6 V2 (TURF § 1): the day's order work first (a pimp's round any
    // hour), else, in the evening, his corner: the target is himself (as a
    // corner dealer's), the plan `turf::corner_plan`.
    // Round 6 V5 (TURF § 3): a muster to the corner outranks the day's work.
    if crate::systems::turf::muster_corner(world, id).is_some()
        && crate::systems::turf::corner_hold(world, id).is_some()
    {
        return Some((id, following_order(world, id)));
    }
    let done = crate::systems::turf::on(world)
        && world.comp::<Brain>(id).is_some_and(|b| b.gang_task_day == Some(world.day()));
    if !done || crate::systems::vice::pimp_target(world, id).is_some() {
        if let Some(t) = gang_work_reach(world, id) {
            return Some(t);
        }
    }
    crate::systems::turf::corner_hold(world, id).map(|_| (id, following_order(world, id)))
}

/// [`gang_work_target`] before the corner.
fn gang_work_reach(world: &World, id: EntityId) -> Option<(EntityId, Option<Order>)> {
    // Pressure pass round 3, S1 (OPEN-221): the order's Home (a shakedown, a
    // claim) or derelict lies within `[distance.choice] gang_work_ticks` of
    // where the member stands, else no GangWork (V11 at 20k: a dealer off his
    // spot walked 300-580 min to a Home picked near the Hideout).
    let t = gang_work_pick(world, id)?;
    let Some(cap) = crate::systems::distance::choice(world).map(|c| c.gang_work_ticks) else { return Some(t) };
    let errand = world.comp::<Building>(t.0).is_some_and(|b| b.kind == BuildingKind::Home || b.derelict)
        && matches!(t.1, None | Some(Order::Expand | Order::VirtRaid | Order::Contest | Order::Squat));
    if errand && crate::systems::distance::trip_to(world, id, t.0).is_some_and(|x| x > cap) {
        return None;
    }
    Some(t)
}

/// [`gang_work_target`] before S1's reach.
fn gang_work_pick(world: &World, id: EntityId) -> Option<(EntityId, Option<Order>)> {
    let gang = world.gang_of(id)?;
    let g = world.comp::<Gang>(gang)?;
    let own_home = world.comp::<Household>(id).and_then(|h| h.home);
    let actor_tile = world.comp::<Position>(id)?.tile;
    // L2 shadow fixes item 16: a dealer off its busy hours has no GangWork
    // (it fell through to the order's shakedowns and fights: Murders on
    // 42-47 rose 215 -> 364 with the hours gate alone).
    // Lives at work phase 4b (§ 2.3): a corner dealer as a Bar's; the corner
    // is a tile, so his target is himself (no walk scored; `stims::corner_plan`
    // walks him to it).
    let site = crate::systems::stims::site_today(world, id);
    if !crate::systems::fixes::deal_hours(world) {
        if site.is_some() {
            return None;
        }
    } else {
        match site {
            Some(crate::systems::stims::DealSite::Venue(bar)) => return Some((bar, following_order(world, id))),
            Some(crate::systems::stims::DealSite::Corner(_)) => return Some((id, following_order(world, id))),
            None => {}
        }
    }
    // Lives at work phase 4b (§ 2.3): a pimp's body in pitch hours walks
    // his round (`vice::pimp_plan`).
    if let Some(worker) = crate::systems::vice::pimp_target(world, id) {
        return Some((worker, following_order(world, id)));
    }
    let hideout_door = world.comp::<Building>(g.hideout).map_or(actor_tile, |b| b.door);
    // The order decides the origin and the filter before any Home is read:
    // the orders with no target return without the scan (perf).
    let theirs: Option<&[EntityId]>;
    let (from, order) = match following_order(world, id) {
        None if world.comp::<Personality>(id).map_or(0.0, |p| p.loyalty) < world.config.gangs.freelance_loyalty => {
            // Lives at work phase 6 (§ 4.5): a delegating leader does not
            // shake down for himself either.
            if crate::systems::careers::delegating(world, id) {
                return None;
            }
            theirs = None;
            (actor_tile, None)
        }
        None => return None,
        // M14 V30: VirtRaid's walkers work as Expand's.
        Some(o @ (Order::Expand | Order::VirtRaid)) => {
            // M14 V30: a freelance runner the gang's database sighted lately
            // is shaken down at Home first.
            if let Some(h) = sighted_runner_home(world, gang, own_home) {
                return Some((h, Some(o)));
            }
            theirs = None;
            (hideout_door, Some(o))
        }
        Some(Order::Contest) => {
            let rival = world.rival_of(gang)?;
            theirs = Some(world.comp::<Gang>(rival)?.territory.as_slice());
            (hideout_door, Some(Order::Contest))
        }
        // M13 D36: the gang's Harvest target (cached at the rescore).
        Some(Order::Harvest) => {
            return crate::systems::chrome::gang_harvest_target(world, gang)
                .filter(|&t| t != id)
                .map(|t| (t, Some(Order::Harvest)));
        }
        // M12 D38: the unheld derelict in the gang's districts nearest the Hideout.
        Some(Order::Squat) => {
            let r = world.config.crime.sight_day_crime;
            // L1: near the Hideout and near the member (the derelict nearest
            // the Hideout sat a 6 h walk from where the member stood).
            // Scale S4 perf: `squat_targets` (the unheld derelict buildings
            // in the gang's districts, not Lots) read on the building grid
            // by the summed walk, not every derelict in the city: the same pick.
            let g = world.comp::<Gang>(gang)?;
            let districts = gang_districts(world, g);
            let guarded = guard_test(world, r);
            return world
                .building_grid()
                .nearest_sum(&[hideout_door, actor_tile], |b| {
                    let bd = world.comp::<Building>(b)?;
                    let ok = matches!(bd.kind, BuildingKind::Home | BuildingKind::Bar | BuildingKind::Hotel)
                        && bd.derelict
                        && !bd.demolished
                        && g.territory.binary_search(&b).is_err()
                        && districts.binary_search(&world.district_of_building(b)).is_ok()
                        && !guarded(bd.door);
                    ok.then_some((bd.door.manhattan(hideout_door) + bd.door.manhattan(actor_tile), b.index))
                })
                .map(|(_, _, b)| (b, Some(Order::Squat)));
        }
        Some(_) => return None,
    };
    // Unclaimed (no gang holds it), or under Contest in the rival's list.
    // Round 6 V2 (TURF § 2): unclaimed is also no other gang's door: not in a
    // cell another gang holds (the holder protects his own), and no claim of
    // another standing gang on it (V14's `Block#478` paid three in 15 hours).
    let pick = |h: EntityId| match theirs {
        Some(t) => t.binary_search(&h).is_ok(),
        None => holder_of(world, h).is_none() && free_door(world, gang, h),
    };
    let r = world.config.crime.sight_day_crime;
    // L1: an order's Home is near the Hideout and near the member.
    let own = if from != actor_tile { Some(actor_tile) } else { None };
    // Scale S4 perf: the least `(door distance, index)` read outward from
    // `from` on the building grid (the distance is at least the door's from
    // `from`), not every Home: the same Home. A building of kind Home is on
    // the Home list unless demolished. Guarded: a guard within
    // `sight_day_crime` of the door.
    let guarded = guard_test(world, r);
    let score = |h: EntityId, chromed: bool| {
        if Some(h) == own_home {
            return None;
        }
        let b = world.comp::<Building>(h)?;
        // M12 D25: a derelict is no extortion target.
        if b.kind != BuildingKind::Home || b.demolished || b.derelict || b.occupants.is_empty() {
            return None;
        }
        if chromed && !crate::systems::creeds::chromed_home(world, h) {
            return None;
        }
        let dist = b.door.manhattan(from) + own.map_or(0, |t| b.door.manhattan(t));
        (pick(h) && !guarded(b.door)).then_some((dist, h.index))
    };
    let best_of = |grid: &crate::spatial::BuildingGrid, chromed: bool| match (theirs, own) {
        // Under Contest the candidates are the rival's territory.
        (Some(t), _) => t.iter().filter_map(|&h| score(h, chromed).map(|(d, i)| (d, i, h))).min(),
        (None, None) => grid.nearest(from, 0, |h| score(h, chromed)),
        // The walk via the Hideout: the cells by the summed distance.
        (None, Some(o)) => grid.nearest_sum(&[from, o], |h| score(h, chromed)),
    };
    // M15 W31: a Purist gang shakes down the chromed's Homes first: the
    // nearest chromed Home, else the nearest of any. Pressure pass round 5,
    // U8 (OPEN-229): the chromed are read on their own grid
    // (`World::chromed_homes`), not by a walk over every Home in the city per
    // member think (`goal::GangWork` 21.7 % of the 100k profile): the same
    // Home.
    let best =
        if crate::systems::creeds::is_purist(world, gang) { best_of(&world.chromed_homes(), true) } else { None };
    best.or_else(|| best_of(&world.building_grid(), false)).map(|(_, _, h)| (h, order))
}

/// M14 V30: the Home of the newest freelance runner (no gang) the gang's
/// database sighted within `sighting_days`: inhabited, standing, not the
/// actor's own, no guard within `sight_day_crime` of its door.
pub fn sighted_runner_home(world: &World, gang: EntityId, own_home: Option<EntityId>) -> Option<EntityId> {
    let db = world.db.get(&gang)?;
    let r = world.config.crime.sight_day_crime;
    db.sightings.iter().rev().find_map(|s| {
        if s.relayed || world.gang_of(s.who).is_some() || !law::living(world, s.who) {
            return None;
        }
        let h = world.comp::<Household>(s.who).and_then(|h| h.home).filter(|&h| Some(h) != own_home)?;
        let b = world.comp::<Building>(h).filter(|b| !b.demolished && !b.derelict && !b.occupants.is_empty())?;
        let guarded = world
            .guards()
            .iter()
            .any(|&g| world.comp::<Position>(g).is_some_and(|p| law::chebyshev(p.tile, b.door) <= r));
        (!guarded).then_some(h)
    })
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
    // Round 6 V2 (V14 `gang_member_6572 d22 14:24`): a man dead or down
    // collects nothing (his step ran out after the blow that killed him).
    if !law::living(world, actor) || !world.has::<Wallet>(actor) {
        return 0;
    }
    // Round 6 V5 (TURF § 3): a shakedown on another gang's held cell, seen
    // by its holders there, provokes them.
    if let Some(door) = world.comp::<Building>(home).map(|b| b.door) {
        crate::systems::turf::provoke_at(world, actor, door, crate::systems::turf::Provoke::Shakedown);
    }
    let mut amount = world.config.social.extort_amount;
    // M15 W31: a Purist tithes.
    if world.gang_of(actor).is_some_and(|g| crate::systems::creeds::is_purist(world, g)) {
        amount = (world.config.creeds.tithe_frac * amount as f32).round() as i64;
    }
    // L2 shadow fixes item 15: a gang-mate at home is not squeezed (the
    // leader shook down her own dealer and made him an Enemy).
    let mates = world.gang_of(actor);
    let occupants: Vec<EntityId> = world
        .comp::<Building>(home)
        .map(|b| {
            b.occupants
                .iter()
                .copied()
                .filter(|&o| o != actor && world.has::<Wallet>(o))
                // The pressure pass S6 (Dylan 2026-10-27): nobody under 18 is a victim.
                .filter(|&o| !crate::systems::ages::is_protected(world, o))
                .filter(|&o| mates.is_none() || world.gang_of(o) != mates)
                // Shadow V4 bug 9: nor a Friend or kin of the actor (the
                // extorter squeezed his new Friend with the rest).
                .filter(|&o| {
                    world.edge(actor, o).is_none_or(|e| {
                        !matches!(
                            e.kind,
                            crate::components::RelKind::Friend
                                | crate::components::RelKind::Family
                                | crate::components::RelKind::Spouse
                                | crate::components::RelKind::Parent
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    // M15 W27: with moves on, the shakedown is an Intimidate against the
    // strongest occupant; a refusal takes nothing and advances no claim.
    if !occupants.is_empty() && !intimidate(world, actor, home, &occupants, amount) {
        return 0;
    }
    // Round 6 V2: the Intimidate's backlash may have killed him.
    if !law::living(world, actor) || !world.has::<Wallet>(actor) {
        return 0;
    }
    let total: i64 = occupants.iter().map(|&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0))).sum();
    let take = amount.min(total);
    let mut taken = 0;
    let held: Vec<i64> = occupants.iter().map(|&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins)).collect();
    // Round 5 U7 (`OPEN-237`): the coins move on the books (`Flow::Coerced`,
    // as the gang's tribute), so the payer's money line names the shakedown
    // (it read `other`, `courier_50838 d23 03:28`).
    for &o in &occupants {
        let coins = world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0));
        let share = if total > 0 { (take * coins) / total } else { 0 };
        taken += crate::systems::ownership::pay(
            world,
            Some(o),
            Some(actor),
            share,
            crate::systems::ownership::Flow::Coerced,
        );
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
        taken += crate::systems::ownership::pay(
            world,
            Some(payer),
            Some(actor),
            1,
            crate::systems::ownership::Flow::Coerced,
        );
    }
    // Shadow V4 bug 9: the robbed are those who paid (9 of 17 shakedowns
    // took 0 coins and still made Enemies, memories and claims).
    // Round 7 W2 (`OPEN-238`): a payer asleep through it learns it when he
    // wakes (`consequence::woke`): no memory, edge or sighting now.
    let mut paid = Vec::new();
    let mut awake: Vec<(EntityId, i64, bool)> = Vec::new();
    for (i, &o) in occupants.iter().enumerate() {
        let lost = held[i] - world.comp::<Wallet>(o).map_or(held[i], |w| w.coins);
        if lost <= 0 {
            continue;
        }
        if crate::systems::presence::asleep(world, o) {
            crate::systems::consequence::slept_through(world, o, actor, lost);
            continue;
        }
        // Round 7 W5: robbed by him before inside the week (the line's "again").
        let again = crate::systems::consequence::robbed_by_within(world, o, actor);
        world.remember(o, MemoryKind::WasRobbed, Some(actor), 0.7, -0.7, false);
        social::robbed_by(world, o, actor);
        paid.push(o);
        awake.push((o, lost, again));
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
    // Shadow V4 bug 9 (and 4): an empty-handed shakedown advances no claim
    // and is no crime to file (a 0-coin shakedown drew 6 days).
    let name = world.name_of(actor);
    if taken == 0 {
        // Pressure pass F2: nor is it an Extortion (V9: "extorted 0 coins"
        // logged as a take): it is a door that gave nothing, `Refused`.
        empty_shakedown(world, actor, home, occupants.is_empty(), "");
        crate::systems::fviolence::note_act(world, actor, crate::ledger::ActKind::Extort);
        return 0;
    }
    let suffix = claim(world, actor, home).unwrap_or_default();
    world.push_event(
        EventKind::Extortion,
        &[actor, home],
        format!("{name} extorted {taken} coins from Block#{}{suffix}", home.index),
    );
    let tile = world.comp::<Position>(actor).map_or(TilePos::default(), |p| p.tile);
    // Round 5 U7 (`OPEN-237`): the payers are the deed's parties, not its
    // witnesses (they know who did it all the same).
    law::raise_crime_parties(world, actor, None, None, crate::components::Crime::Extortion, tile, &paid);
    // Round 7 W5 (`OPEN-237`): the payer awake says what he does about it,
    // as W2's woken sleeper does (`ceo_20314 d23 08:26`, `d27 10:59`: robbed at
    // home twice by one man, and no line but the money's).
    for (o, lost, again) in awake {
        crate::systems::consequence::paid_awake(world, o, actor, home, lost, again);
    }
    crate::systems::fviolence::note_act(world, actor, crate::ledger::ActKind::Extort);
    taken
}

/// Pressure pass F2: a shakedown that took nothing (nobody home, or nobody
/// with a coin) is a `Refused` at the door, not an `Extortion`: no claim, no
/// crime, the task done for the day.
fn empty_shakedown(world: &mut World, actor: EntityId, home: EntityId, nobody: bool, suffix: &str) {
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(actor) {
        b.gang_task_day = Some(today);
    }
    let name = world.name_of(actor);
    let why = if nobody { "nobody was home" } else { "nobody had a coin" };
    world.push_event(
        EventKind::Refused,
        &[actor, home],
        format!("Block#{} gave {name} nothing ({why}){suffix}", home.index),
    );
}

/// L2 (L23): the Home a Statistical member's GangWork day extorts under
/// order `o` (Expand, VirtRaid: unclaimed; Contest: the rival's), as
/// `gang_work_target` picks for a body but "inhabited" read off the
/// residents (a Statistical household stands at no door inside): the
/// nearest to the Hideout plus the member's own door, not its own Home,
/// not derelict, no guard within `sight_day_crime` (a Purist gang's
/// chromed Homes first).
pub fn stat_extort_target(world: &World, id: EntityId, o: Order) -> Option<EntityId> {
    // Lives at work phase 6 (§ 4.5): `following_order`'s rule off screen.
    if crate::systems::careers::delegating(world, id) {
        return None;
    }
    let gang = world.gang_of(id)?;
    let g = world.comp::<Gang>(gang)?;
    let own_home = world.comp::<Household>(id).and_then(|h| h.home);
    let actor_tile = world.comp::<Position>(id)?.tile;
    let from = world.comp::<Building>(g.hideout).map_or(actor_tile, |b| b.door);
    let theirs: Option<&[EntityId]> = match o {
        Order::Expand | Order::VirtRaid => None,
        Order::Contest => Some(world.comp::<Gang>(world.rival_of(gang)?)?.territory.as_slice()),
        _ => return None,
    };
    let guards: Vec<TilePos> =
        world.guards().iter().filter_map(|&g| world.comp::<Position>(g).map(|p| p.tile)).collect();
    let r = world.config.crime.sight_day_crime;
    let purist = crate::systems::creeds::is_purist(world, gang);
    let mut best: Option<((bool, u32, u32), EntityId)> = None;
    for &h in world.buildings_by_kind().get(&BuildingKind::Home)? {
        if Some(h) == own_home {
            continue;
        }
        let Some(b) = world.comp::<Building>(h) else { continue };
        if b.demolished || b.derelict || world.residents_of(h).is_empty() {
            continue;
        }
        let dist = b.door.manhattan(from) + b.door.manhattan(actor_tile);
        if best.is_some_and(|(k, _)| (false, dist, h.index) >= k) {
            continue;
        }
        let ok = match theirs {
            Some(t) => t.binary_search(&h).is_ok(),
            None => holder_of(world, h).is_none() && free_door(world, gang, h),
        };
        if !ok || guards.iter().any(|&gt| law::chebyshev(gt, b.door) <= r) {
            continue;
        }
        let key = (purist && !crate::systems::creeds::chromed_home(world, h), dist, h.index);
        if best.is_some_and(|(k, _)| key >= k) {
            continue;
        }
        best = Some((key, h));
    }
    best.map(|(_, h)| h)
}

/// L2 (L23): an extortion off screen by a Statistical member: no
/// Intimidate and no witness roll (nobody holds a body), the residents of
/// `home` pay `extort_amount` (a Purist's tithe) in proportion to their
/// coins, remember it and turn enemies, the claim advances, the
/// district's shakedown is logged and half the take goes straight to the
/// gang treasury (the SplitLoot a body walks to the Hideout for). Returns
/// the coins taken.
pub fn stat_extort(world: &mut World, actor: EntityId, home: EntityId) -> i64 {
    // Round 6 V2: nor off screen.
    if !law::living(world, actor) || !world.has::<Wallet>(actor) {
        return 0;
    }
    // Round 6 V5: off screen the shakedown on another's held cell provokes.
    if let Some(door) = world.comp::<Building>(home).map(|b| b.door) {
        crate::systems::turf::provoke_at(world, actor, door, crate::systems::turf::Provoke::Shakedown);
    }
    let mut amount = world.config.social.extort_amount;
    let gang = world.gang_of(actor);
    if gang.is_some_and(|g| crate::systems::creeds::is_purist(world, g)) {
        amount = (world.config.creeds.tithe_frac * amount as f32).round() as i64;
    }
    let payers: Vec<EntityId> = world
        .residents_of(home)
        .iter()
        .copied()
        .filter(|&o| o != actor && world.has::<Wallet>(o) && world.has::<Brain>(o) && !world.has::<Sentence>(o))
        // Round 5 U7: as the body's shakedown, nobody under 18 pays (S6).
        .filter(|&o| !crate::systems::ages::is_protected(world, o))
        .collect();
    let total: i64 = payers.iter().map(|&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0))).sum();
    let take = amount.min(total);
    if take <= 0 {
        // Pressure pass F2: nothing to take is no take (no claim, no grudge).
        empty_shakedown(world, actor, home, payers.is_empty(), " (off screen)");
        return 0;
    }
    let mut taken = 0;
    let held: Vec<i64> = payers.iter().map(|&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins)).collect();
    for &o in &payers {
        let coins = world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0));
        let share = if total > 0 { (take * coins) / total } else { 0 };
        if let Some(w) = world.comp_mut::<Wallet>(o) {
            w.coins -= share;
        }
        taken += share;
    }
    let mut guard = 0;
    while taken < take && guard < 64 {
        guard += 1;
        let Some(&payer) = payers.iter().max_by_key(|&&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins)) else {
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
    // Pressure pass F2: as a body's shakedown, the robbed are those who paid.
    for (i, &o) in payers.iter().enumerate() {
        if world.comp::<Wallet>(o).is_some_and(|w| w.coins < held[i]) {
            world.remember(o, MemoryKind::WasRobbed, Some(actor), 0.7, -0.7, false);
            social::robbed_by(world, o, actor);
        }
    }
    let to_gang = taken / 2;
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins += taken - to_gang;
    }
    if let Some(g) = gang {
        world.gang_credit(g, to_gang);
    }
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(actor) {
        b.gang_task_day = Some(today);
    }
    if taken > 0 {
        crate::systems::ownership::note_loss(world, home, taken, Some(actor));
    }
    if let Some(g) = gang {
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
        format!("{name} extorted {taken} coins from Block#{} (off screen){suffix}", home.index),
    );
    taken
}

/// M15 W27: the Shakedown's Intimidate against the strongest occupant
/// (max `might`, ties the lower id) for `Stake::Coins(take)`; logs the try
/// (`extort_log`, `extort_tries`, `extort_success`). A refusal: no coins,
/// no claim, no crime raised, the task done for the day (one try a day),
/// the `Refused` event (the backlash, if any, is the resolver's). Returns
/// whether the occupants pay.
fn intimidate(world: &mut World, actor: EntityId, home: EntityId, occupants: &[EntityId], amount: i64) -> bool {
    use crate::systems::moves;
    // The pressure pass S6 (Dylan 2026-10-27): nobody under 18 is a victim.
    // Round 7 W2 (`OPEN-238`): a sleeper is not stood over; with everyone
    // asleep the purses are taken from the sleeping (V15 `courier_50890 d23
    // 03:56`: "Intimidate from Dmitri Kemp ... SUCCESS" on a man asleep).
    let target = occupants
        .iter()
        .filter(|&&o| !crate::systems::ages::is_protected(world, o))
        .filter(|&&o| !crate::systems::presence::asleep(world, o))
        .map(|&o| (moves::might(world, o), o))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, o)| o);
    let Some(target) = target else { return true };
    let feared = crate::systems::reputation::rep(world, actor).dread >= 0.5;
    let allied = moves::allies_within(world, target, 8) > 0;
    // M16b § 4: the shakedown goes through `leverage` (Threat, a one-off
    // coin demand): the same roll and the same `p` as M15's.
    let out = crate::systems::leverage::shakedown(world, actor, target, amount);
    if world.extort_log.len() >= 4096 {
        world.extort_log.pop_front();
    }
    world.extort_log.push_back((feared, allied, out.success));
    world.stats.current.word.extort_tries += 1;
    if out.success {
        world.stats.current.word.extort_success += 1;
        return true;
    }
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(actor) {
        b.gang_task_day = Some(today);
    }
    let name = world.name_of(actor);
    let back = if out.backlash { ", and fought back" } else { "" };
    world.push_event(EventKind::Refused, &[actor, home], format!("Block#{} would not pay {name}{back}", home.index));
    false
}

/// The claim rules (spec › Contested Homes): own blows count up, a rival's
/// first blow takes the claim at one, the third blow holds the Home. Every
/// blow on a Home in another gang's territory makes that gang's members who
/// see it (`contested`) the actor's enemies; the flip itself logs
/// `TerritoryFlipped` and shocks the loser. Returns a suffix for the
/// Extortion event text when the Home is held.
pub(super) fn claim(world: &mut World, actor: EntityId, home: EntityId) -> Option<String> {
    let gang = world.gang_of(actor)?;
    let gname = world.comp::<Gang>(gang)?.name.clone();
    let holders: Vec<EntityId> = world
        .gangs()
        .into_iter()
        .filter(|&g| g != gang && world.comp::<Gang>(g).is_some_and(|gg| gg.territory.binary_search(&home).is_ok()))
        .collect();
    for &h in &holders {
        contested(world, actor, gang, h, home);
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
    // The city milestone C8 (vertical turf): a tall Home another gang holds
    // is taken floor by floor, the upper half carved off as its own Home.
    if let Some(&h0) = holders.first() {
        if let Some(upper) = crate::systems::vertical::split_claim(world, home, gang, h0) {
            return Some(upper);
        }
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

/// Pressure pass R4 (OPEN-218): a blow on Block `home` held by gang `h`
/// makes enemies of the holders who see it (a body in the Block, or within
/// the crime sight of the actor, `[crime] sight_day_crime` /
/// `sight_night_crime`), not of the whole gang: V10's 100k squat blow flipped
/// 51 of a 71-year-old's friends to Enemy in one minute with no line, and the
/// Fight goal then read every one of them near as a target (seven assaults in
/// two hours). The edges made are told (`Crew`, `[actor, holder gang, home]`).
fn contested(world: &mut World, actor: EntityId, gang: EntityId, h: EntityId, home: EntityId) {
    let Some(at) = world.comp::<Position>(actor).map(|p| p.tile) else { return };
    let cfg = &world.config.crime;
    let r = if world.is_dark() { cfg.sight_night_crime } else { cfg.sight_day_crime };
    let members = world.comp::<Gang>(h).map(|g| g.members.clone()).unwrap_or_default();
    let mut made: Vec<EntityId> = Vec::new();
    // Round 6 V2 (the under-18 rule, TURF): no one under 18 is made an enemy
    // over a claim, on either side.
    if crate::systems::ages::is_protected(world, actor) {
        return;
    }
    for m in members {
        if m == actor || crate::systems::ages::is_protected(world, m) {
            continue;
        }
        let sees =
            world.comp::<Position>(m).is_some_and(|p| p.building == Some(home) || law::chebyshev(p.tile, at) <= r);
        if !sees {
            continue;
        }
        let was = world.edge(m, actor).is_some_and(|e| e.kind == crate::components::RelKind::Enemy);
        social::make_enemy(world, m, actor, -0.7);
        if !was {
            made.push(m);
        }
    }
    if made.is_empty() {
        return;
    }
    let names = world.config.noise.contest_names;
    let shown: Vec<String> = made.iter().take(usize::from(names)).map(|&m| world.name_of(m)).collect();
    let more = made.len().saturating_sub(shown.len());
    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
    let (an, gn, hn) = (
        world.name_of(actor),
        world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone()),
        world.comp::<Gang>(h).map_or_else(String::new, |g| g.name.clone()),
    );
    let text = format!(
        "{an} struck at {hn}'s hold on Block#{} for {gn}: {} of {hn} saw it and count {an} an enemy now ({}{tail})",
        home.index,
        made.len(),
        shown.join(", ")
    );
    world.push_event(EventKind::Crew, &[actor, h, home], text);
}

/// SplitLoot at the Hideout: half the day's haul into the gang treasury.
pub fn split_loot(world: &mut World, actor: EntityId) -> i64 {
    // C9a: a member thrown out mid-split (`ulric_2230`, 2k seed 44 d70:
    // thrown out at 10:39, the split at 10:40) keeps his share: debited with
    // no gang to credit, the coin left the world (the coin identity broke).
    let Some(g) = world.gang_of(actor) else { return 0 };
    let loot = world.comp::<Brain>(actor).map_or(0, |b| b.loot_today);
    let share = (loot as f32 * 0.5).floor() as i64;
    let share = share.min(world.comp::<Wallet>(actor).map_or(0, |w| w.coins)).max(0);
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins -= share;
    }
    world.gang_credit(g, share);
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
    }
    *world.probe.gang_out.entry("fence (direct)".to_string()).or_insert(0) += pay;
    // The gang resells at market price. Real economy (plan E25b): booked as
    // a crossing from the World with the market on (`flow_fence`), so the
    // identity holds without changing gang income: the fence stays a mint,
    // now visible (a finding for Dylan, E46).
    let credit = i64::from(sold) * price;
    world.probe.fence_credit += credit;
    crate::systems::ownership::cross_in(
        world,
        crate::outside::WORLD_ACCOUNT,
        Some(gang),
        credit,
        crate::systems::ownership::Flow::Fence,
    );
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins += pay;
    }
    let cap = world.config.buildings.hideout.stock_cap;
    if let Some(m) = world.hideout_of(gang).and_then(|h| world.comp_mut::<Building>(h)) {
        m.stock_food = (m.stock_food + sold).min(cap);
    }
    pay
}
