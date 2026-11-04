use super::*;

/// C3a: a gang raid's brawl in progress, for its end (`raid_end`).
#[derive(Clone, Debug)]
pub struct RaidEnd {
    pub gid: EntityId,
    pub rival: EntityId,
    pub rival_hideout: EntityId,
    pub door: TilePos,
    pub place: String,
    pub at: String,
    pub gname: String,
    pub rival_name: String,
    pub raiders0: Vec<EntityId>,
    pub defenders0: Vec<EntityId>,
    pub chest0: i64,
    pub food0: u32,
    pub seat: bool,
    pub actor: EntityId,
    pub n_raiders: usize,
    pub n_defenders: usize,
}

/// Is a raid by `gid` being fought (its brawl a live engagement)?
pub(super) fn raid_fighting(world: &World, gid: EntityId) -> bool {
    world.combat.engagements.values().any(|e| matches!(&e.tail, crate::systems::combat::Tail::Raid(r) if r.gid == gid))
}

/// C3a, the gang raid's end (its brawl's tail): the outcome, the settle,
/// the crew's tally, the Raid line.
pub(crate) fn raid_end(world: &mut World, x: &RaidEnd, e: &crate::systems::combat::Engagement) {
    let RaidEnd { gid, rival, rival_hideout, ref place, ref at, ref gname, ref rival_name, seat, actor, .. } = *x;
    let (n_raiders, n_defenders, chest0, food0) = (x.n_raiders, x.n_defenders, x.chest0, x.food0);
    let (raiders, defenders) = (&e.sides[0], &e.sides[1]);
    let (raider_losses, deaths) = (e.losses[0], e.deaths);
    let outcome = if n_defenders == 0 || (defenders.is_empty() && raider_losses == 0) {
        Outcome::Sacked
    } else if defenders.is_empty() {
        Outcome::Won
    } else {
        Outcome::Lost
    };
    settle(world, gid, rival, outcome, rival_hideout);
    {
        let row = crate::systems::ownership::jobs2_row(world);
        if seat {
            row.raids_seat += 1;
        } else {
            row.raids_nearest += 1;
        }
    }
    let coins = chest0 - world.comp::<Gang>(rival).map_or(0, |g| g.treasury);
    let food = food0.saturating_sub(world.comp::<Building>(rival_hideout).map_or(0, |b| b.stock_food));
    crate::systems::crew::raid_result(
        world,
        crate::systems::crew::RaidLeft {
            gang: gid,
            rival: Some(rival),
            place,
            raiders: &x.raiders0,
            defenders: &x.defenders0,
            raiders_standing: raiders,
            defenders_standing: defenders,
            won: outcome != Outcome::Lost,
            coins: coins.max(0),
            food,
            freed: None,
        },
    );
    world.push_event(
        EventKind::Raid,
        &[gid, rival, actor],
        format!(
            "{gname} raided {rival_name}: {outcome:?} ({n_raiders} raiders vs {n_defenders} defenders, {deaths} dead; at {at})"
        ),
    );
    // Lives at work phase 6 (§ 4.5): the head's ladder counts a live brawl
    // here (`resolve` credits the outcome `brawl` returns at once).
    if e.live {
        crate::systems::careers::credit_raid(world, actor, matches!(outcome, Outcome::Won | Outcome::Sacked));
    }
    world.combat.last_raid = Some(outcome);
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
    let mut v: Vec<EntityId> = out.into_iter().take(cap).map(|(_, g)| g).collect();
    // M16a (plan C32): the agents standing a Guard post on the building.
    if let Some(posted) = world.contract_guards.get(&b) {
        for &g in posted {
            if !v.contains(&g) && crate::systems::law::living(world, g) {
                v.push(g);
            }
        }
    }
    v
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
    // M14 V32: the guards summoned to the door (posted, then answering) are
    // stood there after the DoorOpen strip, so a stripped one never comes.
    let mut summoned: Vec<EntityId> = Vec::new();
    for g in posted_guards(world, target) {
        if !defenders.contains(&g) {
            summoned.push(g);
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
    let posted_n = summoned.len();
    // The city milestone C8: the answer climbs to the target's floor too.
    let mut answering = answering_guards(world, d, door, &defenders);
    let stayed = crate::systems::vertical::climbing_guards(world, target, &mut answering);
    if stayed > 0 {
        let floor = world.comp::<Building>(target).and_then(|b| b.span).map_or(0, |(lo, _)| lo);
        let text = format!(
            "The beat's guards climbed to {} on floor {floor}: {} made it up, {stayed} too far to come in time",
            world.name_of(target),
            answering.len()
        );
        world.push_event(EventKind::Raid, &[target], text);
    }
    for g in answering {
        summoned.push(g);
        defenders.push(g);
    }
    // M14 V32: under DoorOpen the robots and then the summoned guards
    // (weakest first) stay away while the removed strength is within
    // `door_strip_cap` of the whole defence.
    let stripped = if crate::systems::virt::hack_live(world, target, crate::virt::HackEffect::DoorOpen) {
        let robots = crate::systems::robots::defenders_at(world, target);
        let all: Vec<EntityId> = defenders.iter().chain(robots.iter()).copied().collect();
        let total: f32 = all.iter().map(|&x| strength(world, x)).sum();
        let mut weak: Vec<(EntityId, f32)> = summoned.iter().map(|&g| (g, strength(world, g))).collect();
        weak.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let cands: Vec<(EntityId, f32)> = robots.iter().map(|&r| (r, strength(world, r))).chain(weak).collect();
        door_strip(total, &cands, world.config.ice.door_strip_cap)
    } else {
        Vec::new()
    };
    for (i, &g) in summoned.iter().enumerate() {
        if stripped.contains(&g) {
            continue;
        }
        if i >= posted_n || world.has::<Brain>(g) {
            world.abort_plan(g);
        }
        world.stand_at_door(g, target);
    }
    defenders.retain(|&x| !stripped.contains(&x));
    defenders.retain(|&x| !world.has::<Sentence>(x) && x != actor && world.gang_of(x) != Some(gid));
    by_strength(world, &mut defenders);
    robots_first(world, target, &mut defenders);
    defenders.retain(|x| !stripped.contains(x));
    let mut raiders = raiders_at(world, gid, actor, door);
    turned_join(world, target, gid, &mut raiders);
    let (n_raiders, n_defenders) = (raiders.len(), defenders.len());
    let what = world.name_of(target);
    let owner = world.owner_label(corp);
    let place = format!("{owner}'s {what}");
    // Fix pass: the crew breaks once `corp_raid_break` of it is down (at
    // least one pairing lost); 0 fights to the last raider (phase 4).
    let brk = world.config.gangs.corp_raid_break;
    let max_losses = if brk > 0.0 { ((n_raiders as f32 * brk).ceil() as usize).max(1) } else { usize::MAX };
    crate::systems::gossip::post_raid(world, Some(gid), corp.or(Some(target)), door);
    let (raiders0, defenders0) = (raiders.clone(), defenders.clone());
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
        // M13 D38: the building's Stims and Parts go to the winner's Hideout.
        crate::systems::stims::loot_goods(world, target, gid);
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
    faction::rethink(world, gid);
    // The pressure pass phase 5b: the result every raider and guard reads.
    crate::systems::crew::raid_result(
        world,
        crate::systems::crew::RaidLeft {
            gang: gid,
            rival: None,
            place: &place,
            raiders: &raiders0,
            defenders: &defenders0,
            raiders_standing: &raiders,
            defenders_standing: &defenders,
            won: outcome == Outcome::Won,
            coins: took.0,
            food: took.1,
            freed: None,
        },
    );
    world.push_event(
        EventKind::Raid,
        &[gid, target, actor],
        format!(
            "{gname} raided {place}: {outcome:?} ({n_raiders} raiders vs {n_defenders} defenders, {} dead, {} coins and {} food taken)",
            tally.deaths, took.0, took.1
        ),
    );
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
fn settle(world: &mut World, gid: EntityId, rival: EntityId, outcome: Outcome, at: EntityId) {
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
            // The city milestone C9b: a district Hideout holds a share of the chest.
            let frac = crate::systems::vertical::take_frac(world, rival, at).unwrap_or(cfg.raid_prize_frac);
            let prize = world.comp::<Gang>(rival).map_or(0, |g| (g.treasury.max(0) as f32 * frac).floor() as i64);
            if let Some(g) = world.comp_mut::<Gang>(rival) {
                g.treasury -= prize;
            }
            *world.probe.gang_out.entry("raid prize (direct)".to_string()).or_insert(0) += prize;
            world.gang_credit(gid, prize);
            // M13 D38: the loser Hideout's Stims and Parts (C9b: the one raided).
            crate::systems::stims::loot_goods(world, at, gid);
            gang::push_shock(world, rival, Shock::Raided);
            // Round 6 V5 (TURF § 3): a raid won moves the cells about the Hideout.
            crate::systems::turf::raid_moves_at(world, gid, rival, at);
            // The city milestone C8: the loser's lift cut on the way out.
            crate::systems::vertical::raid_cut(world, gid, rival, at);
        }
        Outcome::Sacked => {
            // The city milestone C9b: a district Hideout sacked gives up its
            // floor (a share of the chest, its food and goods, the bodies put
            // out); the seat stands, so the gang is not sacked.
            let district = crate::systems::vertical::take_frac(world, rival, at);
            let Some((loot, rival_hideout, rival_name)) = world.comp::<Gang>(rival).map(|g| {
                let all = g.treasury.max(0);
                (district.map_or(all, |f| (all as f32 * f).floor() as i64), at, g.name.clone())
            }) else {
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
                if district.is_none() {
                    g.sacked_until = Some(now + cfg.sacked_days * TICKS_PER_DAY);
                }
                // The grudge outlives the sack: the Sacked shock is consumed
                // while Retaliate is still gated, so the brain reads the
                // grievance from retaliate_until once it can muster again.
                g.retaliate_until = Some(now + (cfg.sacked_days + cfg.retaliate_days) * TICKS_PER_DAY);
            }
            *world.probe.gang_out.entry("sack loot (direct)".to_string()).or_insert(0) += loot;
            world.gang_credit(gid, loot);
            // M13 D38: the sacked Hideout's Stims and Parts.
            crate::systems::stims::loot_goods(world, rival_hideout, gid);
            // M14 V19: its Data moves to the winner's Hideout node (not wiped;
            // C9b: the node is at the seat).
            if district.is_none() {
                crate::systems::virt::move_store(world, rival_hideout, gid);
            }
            // Everyone inside is put out on the street.
            let inside = world.comp::<Building>(rival_hideout).map(|b| b.occupants.clone()).unwrap_or_default();
            for o in inside {
                world.stand_at_door(o, rival_hideout);
            }
            gang::push_shock(world, rival, if district.is_some() { Shock::Raided } else { Shock::Sacked });
            // Round 6 V5: and a Hideout sacked.
            crate::systems::turf::raid_moves_at(world, gid, rival, rival_hideout);
            // The city milestone C8: the loser's lift cut on the way out.
            crate::systems::vertical::raid_cut(world, gid, rival, rival_hideout);
            let gname = world.comp::<Gang>(gid).map_or_else(String::new, |g| g.name.clone());
            let which = if district.is_some() {
                let host = world.comp::<Building>(rival_hideout).and_then(|b| b.within).unwrap_or(rival_hideout);
                format!("Hideout in {}", world.name_of(host))
            } else {
                "Hideout".to_string()
            };
            world.push_event(
                EventKind::Sacked,
                &[rival, gid, rival_hideout],
                format!("{rival_name}'s {which} sacked by {gname}: {loot} coins and {food} food taken"),
            );
        }
    }
    if let Some(g) = world.comp_mut::<Gang>(gid) {
        g.raid_at = None;
        g.retaliate_until = None;
    }
    faction::rethink(world, gid);
}
