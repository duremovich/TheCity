use super::*;

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
    // Shadow V4 bug 5: never a corp's exec (the corp scan only once a gang
    // would take the agent: this runs per think).
    let gang = recruit_gang_any(world, id)?;
    (!crate::systems::founding::is_exec(world, id)).then_some(gang)
}

fn recruit_gang_any(world: &World, id: EntityId) -> Option<EntityId> {
    if world.has::<GangMember>(id) {
        return None;
    }
    let cfg = &world.config.social;
    let gangs = world.gang_list();
    // The member with the best qualifying edge: the highest affinity, ties to
    // the first gang in `gangs()` then the first in its roster. Walks the
    // agent's own edges rather than every roster (M10 phase 5b).
    let open: Vec<bool> = gangs.iter().map(|&g| world.comp::<Gang>(g).is_some_and(|g| recruiting(world, g))).collect();
    // The pressure pass phase 1 (OPEN-178): the miserable need a weaker tie.
    let min_affinity = cfg.join_gang_affinity - crate::systems::misery::join_affinity_cut(world, id);
    // Pressure pass round 6, V2 (TURF § 2): the holder of the recruit's Home
    // cell asks first (the corner is where you are asked): its members' ties
    // count `recruit_affinity_cut` weaker and win over any other gang's.
    let holder = crate::systems::turf::home_holder(world, id);
    let turf_cut = crate::systems::turf::cfg(world).map_or(0.0, |c| c.recruit_affinity_cut);
    let mut best: Option<(bool, f32, usize, usize, EntityId)> = None;
    if open.contains(&true) {
        // The perf pass phase 6 (the slope): an agent's edges grow with the
        // days (co-workers and off-screen meetings), the open gangs' rosters
        // do not, so the shorter of the two is read. The same candidates
        // either way: a member `m` of gang `gid` at roster position `mi`
        // with an edge to `id`; the best is the highest affinity, then the
        // lowest `(gi, mi)`, a total order, so the walk's order is free (a
        // member listed twice keeps its first position, as `position` did).
        let known = world.neighbours.get(&id);
        let roster_len: usize = gangs
            .iter()
            .zip(&open)
            .filter(|(_, &o)| o)
            .filter_map(|(&g, _)| world.comp::<Gang>(g))
            .map(|g| g.members.len())
            .sum();
        // Pressure pass round 3, S1 (OPEN-221): with `[distance.choice]` and
        // `[crew]` on, a gang with no try-out place within the recruit's reach
        // (`tryout_place`) is no pick: the best tie among the gangs he can
        // walk to (V11: the best tie's Hideout across the island, JoinGang
        // not scored, a nearer gang's tie never read). Read once a gang.
        let reach_on = crate::systems::distance::choice(world).is_some() && crate::systems::crew::cfg(world).is_some();
        let mut in_reach: Vec<Option<bool>> = vec![None; gangs.len()];
        let mut consider = |gi: usize, gid: EntityId, m: EntityId, mi: usize| {
            // M15 W31: a Purist gang refuses the chromed.
            if !crate::systems::creeds::tolerated(world, gid, id)
                || !crate::systems::creeds::edge_recruits(world, gid, id, m)
            {
                return;
            }
            let Some(e) = world.edge(id, m) else { return };
            let own = holder == Some(gid);
            if e.affinity < min_affinity - if own { turf_cut } else { 0.0 } {
                return;
            }
            let better = best.is_none_or(|(o, a, bg, bm, _)| {
                (own && !o) || (own == o && (e.affinity > a || (e.affinity == a && (gi, mi) < (bg, bm))))
            });
            if better && reach_on && !*in_reach[gi].get_or_insert_with(|| tryout_place(world, id, gid).is_some()) {
                return;
            }
            if better {
                best = Some((own, e.affinity, gi, mi, gid));
            }
        };
        if roster_len < known.map_or(0, |s| s.len()) {
            for (gi, &gid) in gangs.iter().enumerate() {
                if !open[gi] {
                    continue;
                }
                let Some(g) = world.comp::<Gang>(gid) else { continue };
                for (mi, &m) in g.members.iter().enumerate() {
                    if world.comp::<GangMember>(m).is_some_and(|gm| gm.gang == gid)
                        && known.is_some_and(|s| s.contains(&m))
                    {
                        consider(gi, gid, m, mi);
                    }
                }
            }
        } else {
            for m in world.neighbours(id) {
                let Some(gid) = world.comp::<GangMember>(m).map(|gm| gm.gang) else { continue };
                // `gangs` is ascending (Scale S4 perf: a search, not a scan).
                let Ok(gi) = gangs.binary_search(&gid) else { continue };
                if !open[gi] {
                    continue;
                }
                let Some(mi) = world.comp::<Gang>(gid).and_then(|g| g.members.iter().position(|&x| x == m)) else {
                    continue;
                };
                consider(gi, gid, m, mi);
            }
        }
    }
    if let Some((_, _, _, _, gid)) = best {
        return Some(gid);
    }
    // M11 D36: a fresh, lawless evictee is desperate too (the spiral's first link).
    let desperate = (world.comp::<Needs>(id).is_some_and(|n| n.hunger < cfg.join_gang_desperation_hunger)
        && world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < cfg.join_gang_desperation_lawfulness))
        || crate::systems::classes::evicted_desperate(world, id)
        // The pressure pass phase 1 (OPEN-178): the miserable and lawless too.
        || crate::systems::misery::join_desperate(world, id);
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
        .iter()
        .copied()
        .filter_map(|gid| world.comp::<Gang>(gid).map(|g| (gid, g)))
        .filter(|(_, g)| !g.is_sacked(world.tick))
        .filter(|&(gid, _)| may_reform(world, gid))
        .filter(|&(gid, _)| crate::systems::creeds::tolerated(world, gid, id))
        .filter(|&(gid, _)| crate::systems::creeds::open_to_strangers(world, gid))
        .filter(|(_, g)| {
            (desperate && recruiting(world, g))
                || (arrested && g.members.is_empty() && world.config.gangs.max_members > 0)
        })
        .filter_map(|(gid, g)| {
            // Round 6 V2: the holder of his cell before the nearest Hideout.
            world
                .comp::<Building>(g.hideout)
                .map(|b| (holder != Some(gid), b.door.manhattan(from), g.hideout.index, gid))
        })
        .min()
        .map(|(_, _, _, gid)| gid)
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
    // M17 phase 3: an emptied gang with a living syndicate re-forms only
    // through its syndicate's refound (`parents::syndicate_daily`).
    if crate::systems::parents::has_syndicate(world, gang) {
        return false;
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
    hideout_with(world, id, || recruit_gang(world, id))
}

/// [`hideout_for`] with `recruit_gang`'s answer supplied by the caller
/// (Scale S4 perf: a plan context reads it for `eligible` too).
pub fn hideout_with(world: &World, id: EntityId, recruit: impl FnOnce() -> Option<EntityId>) -> Option<EntityId> {
    // The city milestone C8: a member's own Hideout, the nearest of his gang's.
    if world.gang_of(id).is_some() {
        return crate::systems::vertical::own_hideout(world, id);
    }
    world
        .gang_of(id)
        .or_else(recruit)
        .and_then(|g| world.hideout_of(g))
        .or_else(|| world.building_of_kind(BuildingKind::Hideout))
}

/// Is this member on tonight's watch? `min(night_watch, n / 2)` members
/// sleep at the Hideout each night so a raid meets someone: the roster in
/// joining order (newest first: the grunts stand watch), rotated by the day,
/// so the duty goes round. The night is keyed on the day it starts: a watch
/// that began at 22:00 is the same watch at 02:00.
pub fn on_watch(world: &World, id: EntityId) -> bool {
    on_watch_night(world, id, (world.tick + TICKS_PER_DAY / 2) / TICKS_PER_DAY)
}

/// [`on_watch`] on the night keyed `night` (the night that starts on the
/// evening of day `night - 1`; the city milestone C9b reads tonight's at the
/// midnight before it, `crew::post_gang`'s seat guard).
pub fn on_watch_night(world: &World, id: EntityId, night: u64) -> bool {
    let Some(g) = world.gang_of(id).and_then(|g| world.comp::<Gang>(g)) else { return false };
    // Jailed members cannot stand watch: only the free roster counts.
    let mut roster: Vec<(Tick, EntityId)> = g
        .members
        .iter()
        .filter(|&&m| !world.has::<Sentence>(m))
        .map(|&m| (world.comp::<GangMember>(m).map_or(0, |gm| gm.joined_tick), m))
        .collect();
    // L1: the watch is stood by members who live within a walk of the
    // Hideout (or have no Home): a Spire member walked 5.5 h each way to it.
    let reach = 2 * world.config.life.commute_cap_tiles;
    // C8: the watch is stood at the seat (the stash and the chest), by those within a walk of it.
    let door = world.comp::<Building>(g.hideout).map(|b| b.door);
    let near: Vec<(Tick, EntityId)> = roster
        .iter()
        .copied()
        .filter(|&(_, m)| {
            match world.comp::<Household>(m).and_then(|h| h.home).and_then(|h| world.comp::<Building>(h)) {
                Some(b) => door.is_some_and(|d| d.manhattan(b.door) <= reach),
                None => true,
            }
        })
        .collect();
    if !near.is_empty() {
        roster = near;
    }
    let n = roster.len();
    let watch = world.config.gangs.night_watch.min(n / 2);
    if watch == 0 {
        return false;
    }
    roster.sort_by(|a, b| b.cmp(a));
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
    // The city milestone C9b: the seat's standing guard holds the night there.
    let watch = on_watch(world, id) || crate::systems::crew::seat_guard_today(world, id);
    if g.is_sacked(world.tick) || !(g.order == Order::LieLow || homeless || watch) {
        return None;
    }
    // The city milestone C8: his own Hideout (the nearest of the gang's to his
    // Home) lying low or homeless; the watch at the seat (the stash and the chest).
    let hq = if watch && g.order != Order::LieLow && !homeless {
        g.hideout
    } else {
        crate::systems::vertical::own_hideout(world, id)?
    };
    let b = world.comp::<Building>(hq)?;
    // Shadow V4 bug 11: lying low or homeless, not a bed past `[gangs]
    // hole_up_max_tiles` (a sweeper walked 901 min to his Hideout bed at
    // 20k; the watch is already stood from within a walk, `on_watch`).
    let max = world.config.gangs.hole_up_max_tiles;
    let far = max > 0
        && !watch
        && !b.occupants.contains(&id)
        && world.comp::<crate::components::Position>(id).is_some_and(|p| p.tile.manhattan(b.door) > max);
    // The island fix round F2 (OPEN-191, D2 = A): a housed member, the
    // watch included, only within the walk and tomorrow's commute.
    let far = far || (!homeless && crate::systems::beds::hideout_ok(world, id, hq) == Some(false));
    (!far && (!b.is_full() || b.occupants.contains(&id))).then_some(hq)
}

/// JoinGang at the Hideout: join the gang `recruit_gang` names.
pub fn join(world: &mut World, id: EntityId) -> bool {
    let Some(gang) = recruit_gang(world, id) else { return false };
    enlist_chosen(world, id, gang);
    true
}

// ---------------------------------------------------------------------------
// The try-out (pressure pass round 2, R5, OPEN-214)
// ---------------------------------------------------------------------------

/// Where `id` would try out for `gang` from where he stands: its Hideout
/// within `[crew] tryout_reach_ticks`, else its nearest front within it
/// (the Vice and leisure venues it holds); `None` past both (V10: 300- to
/// 800-tile JoinGang walks that never arrived). Without `[crew]`, the
/// Hideout.
pub fn tryout_place(world: &World, id: EntityId, gang: EntityId) -> Option<EntityId> {
    let hideout = world.hideout_of(gang)?;
    let Some(c) = crate::systems::crew::cfg(world) else { return Some(hideout) };
    let here = world.comp::<Position>(id)?;
    if here.building == Some(hideout) {
        return Some(hideout);
    }
    let from = here.tile;
    let reach = c.tryout_reach_ticks;
    let ticks = |b: EntityId| {
        world
            .comp::<Building>(b)
            .filter(|bd| !bd.demolished)
            // S1 (OPEN-221): the recruit's own trip (the walk without the fare).
            .map(|bd| crate::systems::distance::trip(world, id, from, bd.door))
    };
    if ticks(hideout).is_some_and(|t| t <= reach) {
        return Some(hideout);
    }
    crate::systems::leisure::fronts_of(world, gang)
        .into_iter()
        .filter_map(|b| ticks(b).map(|t| (t, b)))
        .filter(|&(t, _)| t <= reach)
        .min()
        .map(|(_, b)| b)
}

/// JoinGang's goal gate with `[crew]` on: a gang would take `id` and a
/// try-out place is in reach.
pub fn tryout_open(world: &World, id: EntityId) -> bool {
    recruit_gang(world, id).is_some_and(|g| tryout_place(world, id, g).is_some())
}

/// The try-out plan with `[crew]` on (`plan::plan_for` bypasses the
/// planner, as the raid's chain): `[GoTo(Hideout) place, JoinGang place]`,
/// the walk dropped when he is inside. `None` with no place in reach.
pub fn tryout_plan(world: &World, id: EntityId) -> Option<crate::goap::Plan> {
    crate::systems::crew::cfg(world)?;
    let gang = recruit_gang(world, id)?;
    let place = tryout_place(world, id, gang)?;
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let mut steps = Vec::new();
    if world.comp::<Position>(id).and_then(|p| p.building) != Some(place) {
        steps.push(step(crate::goap::ActionKind::GoTo(crate::goap::LocationKey::Hideout), Some(place)));
    }
    steps.push(step(crate::goap::ActionKind::JoinGang, Some(place)));
    Some(crate::goap::Plan { goal: GoalKind::JoinGang, target: Some(place), steps, started_tick: world.tick })
}

/// The gang whose try-out `place` is: the gang whose Hideout it is, else
/// the gang fronting it.
fn gang_at(world: &World, place: EntityId) -> Option<EntityId> {
    let gangs = world.gang_list();
    gangs
        .iter()
        .copied()
        .find(|&g| world.hideout_of(g) == Some(place))
        .or_else(|| gangs.iter().copied().find(|&g| crate::systems::leisure::fronts_of(world, g).contains(&place)))
}

/// The try-out at `place` ended with no gang to take `id` (the treasury
/// fell short, the roster filled, the tie cooled on the walk): turned away,
/// said so, and no second try for `tryout_cool_days`.
pub fn turned_away(world: &mut World, id: EntityId, place: Option<EntityId>) {
    let Some(c) = crate::systems::crew::cfg(world).cloned() else { return };
    let gang = place.and_then(|p| gang_at(world, p));
    let why = match gang.and_then(|g| world.comp::<Gang>(g)) {
        Some(g) if g.is_sacked(world.tick) => "the place had just been sacked",
        Some(g) if g.members.len() >= world.config.gangs.max_members => "they were full up",
        Some(g) if !recruiting(world, g) => "the treasury could not feed another mouth",
        _ => "nobody there would vouch for him",
    };
    let gname = gang.and_then(|g| world.comp::<Gang>(g)).map_or_else(|| "the gang".to_string(), |g| g.name.clone());
    let at = place.map_or_else(|| "the door".to_string(), |p| world.name_of(p));
    let text = format!("{} tried out for {gname} at {at} and was turned away: {why}", world.name_of(id));
    let mut actors = vec![id];
    actors.extend(gang);
    actors.extend(place);
    world.push_event(EventKind::Crew, &actors, text);
    crate::systems::ownership::jobs2_row(world).crew_tryout_refused += 1;
    let until = world.tick + c.tryout_cool_days * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.cooldowns.insert(GoalKind::JoinGang, until);
    }
}

/// Put `id` into `gang` with every side effect of joining. No eligibility
/// check: `join` does that; tests call this directly.
pub fn enlist(world: &mut World, id: EntityId, gang: EntityId) {
    // M18 P5 (Dylan 2026-11-03): the player's body joins a gang only by its
    // own pick (the JoinGang step, a front's offer taken: `enlist_chosen`);
    // a member moved with its gang is a member already.
    if crate::systems::player::possessed(world, id) && !world.has::<GangMember>(id) {
        return;
    }
    enlist_chosen(world, id, gang);
}

/// [`enlist`] for a join the body chose (the player's own pick too).
pub fn enlist_chosen(world: &mut World, id: EntityId, gang: EntityId) {
    // The pressure pass S6 (Dylan 2026-10-27): gangs from 18 (every caller
    // picks an adult; the backstop).
    if crate::systems::ages::is_protected(world, id) {
        return;
    }
    let tick = world.tick;
    world.insert(id, GangMember { gang, rank: 0, joined_tick: tick, raids_led: 0, raids_won: 0 });
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
    // The pressure pass phase 1 (OPEN-178): a recruit misery delivered says so.
    let misery = crate::systems::misery::join_note(world, id);
    if misery.is_some() {
        crate::systems::ownership::jobs2_row(world).misery_joins += 1;
    }
    let mut text = match (cites, misery) {
        (Some(n), Some(m)) => format!("{name} joined {gname} (cites mem#{n}; {m})"),
        (Some(n), None) => format!("{name} joined {gname} (cites mem#{n})"),
        (None, Some(m)) => format!("{name} joined {gname} ({m})"),
        (None, None) => format!("{name} joined {gname}"),
    };
    // Round 6 V2 (TURF § 2): a recruit from the gang's own blocks says so.
    if crate::systems::turf::home_holder(world, id) == Some(gang) {
        crate::systems::ownership::jobs2_row(world).turf_recruits += 1;
        text.push_str(": asked on the corner of the gang's blocks, where the recruit lives");
    }
    world.push_event(EventKind::GangJoin, &[id, gang], text);
    recompute_leader(world, gang);
}

/// Leave the gang (betrayal, emigration; death goes through `on_member_killed`).
pub fn leave(world: &mut World, id: EntityId, reason: &str) {
    let Some(gm) = world.remove::<GangMember>(id) else { return };
    // Round 6 V5: a war side's loss (a desister, a leaver).
    crate::systems::turf::note_loss(world, gm.gang, id);
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
    // M13 fix round: the gang's bike stays with the gang.
    crate::systems::vehicles::return_gang_vehicles(world, id, gm.gang);
    // Jobs & lives phase 6 (§ 8): a cult's apostate (nothing off).
    crate::systems::faith::on_leave(world, id, gm.gang, reason);
    recompute_leader(world, gm.gang);
}
