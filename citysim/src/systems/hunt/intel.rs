use super::*;

/// The busiest Bar of a district (most occupants, ties the lower id).
pub fn busiest_bar(world: &World, d: crate::components::DistrictId) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished) && world.district_of_building(b) == d)
        .max_by(|&a, &b| {
            let n = |x: EntityId| world.comp::<Building>(x).map_or(0, |bd| bd.occupants.len());
            n(a).cmp(&n(b)).then(b.cmp(&a))
        })
}

/// The hunter's freshest held Sighting of `target` (any age), and the
/// gang database's: `(tick, building, tile)`.
pub(super) fn sightings_of(
    world: &World,
    hunter: EntityId,
    target: EntityId,
) -> Option<(Tick, Option<EntityId>, crate::components::TilePos)> {
    let mut best: Option<(Tick, Option<EntityId>, crate::components::TilePos)> = None;
    if let Some(m) = world.comp::<Memory>(hunter) {
        for e in m.heard.iter().filter(|e| e.kind == MemoryKind::Sighting && e.subject == Some(target)) {
            let Some(door) = e.at.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) else { continue };
            if best.is_none_or(|(t, _, _)| e.tick > t) {
                best = Some((e.tick, e.at, door));
            }
        }
    }
    if let Some(db) = world.gang_of(hunter).and_then(|g| world.db.get(&g)) {
        for s in db.sightings.iter().filter(|s| s.who == target) {
            if best.is_none_or(|(t, _, _)| s.tick > t) {
                best = Some((s.tick, None, s.tile));
            }
        }
    }
    best
}

/// W21 (1): a Sighting of the target younger than `fresh_sighting_hours`.
pub fn fresh_intel(world: &World, hunter: EntityId, target: EntityId) -> Option<Intel> {
    let horizon = world.tick.saturating_sub(Tick::from(world.config.hunt.fresh_sighting_hours) * TICKS_PER_HOUR);
    let (t, building, tile) = sightings_of(world, hunter, target)?;
    (t >= horizon).then_some(Intel { building, tile, source: IntelSource::Sighting })
}

/// The district where the hunter last knew the target to be: its newest
/// sighting's, else the target's Home's, else where it stands.
pub(crate) fn last_known_district(
    world: &World,
    hunter: EntityId,
    target: EntityId,
) -> Option<crate::components::DistrictId> {
    if let Some((_, _, tile)) = sightings_of(world, hunter, target) {
        return Some(world.district_of(tile));
    }
    crate::systems::gossip::home_district(world, target)
        .or_else(|| world.comp::<Position>(target).map(|p| world.district_of(p.tile)))
}

pub(super) fn intel_at(world: &World, b: EntityId, source: IntelSource) -> Option<Intel> {
    world.comp::<Building>(b).map(|bd| Intel { building: Some(b), tile: bd.door, source })
}

/// W21: where the target's Trace says it is at `now`: its Home at night
/// (20:00-06:00) when it slept there on ≥ 7 of the last 14 days, its
/// workplace while on shift, else the busiest Bar of its modal district of
/// the last 14 days, else its Home, else where it stands.
pub fn habit(world: &World, target: EntityId, now: Tick) -> Intel {
    let day = time::day(now);
    let tod = (now % TICKS_PER_DAY) as u16;
    let hour = tod / TICKS_PER_HOUR as u16;
    let home = world.comp::<Household>(target).and_then(|h| h.home).filter(|&h| world.has::<Building>(h));
    let mut slept = 0;
    let mut counts = [0u16; crate::components::MAX_DISTRICTS];
    if let Some(tr) = world.comp::<Trace>(target) {
        for d in day.saturating_sub(14)..day {
            if let Some(t) = tr.on_day(d) {
                if t.has(trace_flags::SLEPT_AT_HOME) {
                    slept += 1;
                }
                if !t.district.is_unset() {
                    if let Some(c) = counts.get_mut(t.district.index()) {
                        *c += 1;
                    }
                }
            }
        }
    }
    let night = !(6..20).contains(&hour);
    if let (Some(h), true) = (home, night && slept >= 7) {
        if let Some(i) = intel_at(world, h, IntelSource::Habit) {
            return i;
        }
    }
    if let Some(e) = world.comp::<Job>(target).filter(|j| j.on_shift(tod)).and_then(|j| j.employer) {
        if let Some(i) = intel_at(world, e, IntelSource::Habit) {
            return i;
        }
    }
    let modal = counts
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0)))
        .map(|(i, _)| crate::components::DistrictId(i as u8));
    if let Some(i) = modal.and_then(|d| busiest_bar(world, d)).and_then(|b| intel_at(world, b, IntelSource::Habit)) {
        return i;
    }
    if let Some(i) = home.and_then(|h| intel_at(world, h, IntelSource::Home)) {
        return i;
    }
    let p = world.comp::<Position>(target);
    Intel {
        building: p.and_then(|p| p.building),
        tile: p.map_or_else(Default::default, |p| p.tile),
        source: IntelSource::Habit,
    }
}

/// W19: the hunter's scripted plan (taking up the Hunt first when there is
/// none): a fresh sighting skips the asking.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !world.hunts.contains_key(&id) && !adopt(world, id, HuntWhy::Goal) {
        return None;
    }
    let s = world.hunts.get(&id)?.clone();
    if !target_ok(world, s.target) {
        return None;
    }
    let mut next = s.clone();
    let mut told = None;
    if next.phase == HuntPhase::Ask {
        if let Some(i) = fresh_intel(world, id, s.target) {
            next.phase = HuntPhase::Watch;
            next.intel = Some(i);
        } else if let Some((o, i)) = asked_own(world, id, s.target) {
            // Round 9 Z1: word along his own people before a Bar's.
            next.phase = HuntPhase::Watch;
            next.intel = Some(i);
            told = Some(o);
        } else {
            match last_known_district(world, id, s.target).and_then(|d| busiest_bar(world, d)) {
                Some(v) => next.venue = Some(v),
                None => {
                    next.phase = HuntPhase::Watch;
                    next.intel = Some(habit(world, s.target, world.tick));
                }
            }
        }
    }
    if next.phase == HuntPhase::Watch && next.intel.is_none() {
        next.intel = Some(habit(world, s.target, world.tick));
    }
    if !within_reach(world, id, next.phase, &mut next.venue, next.intel) {
        // Round 9 Z1: past reach, his known haunts in reach (his Home, his
        // post), else the hunt waits for word (kept, not cooled away).
        match haunt_in_reach(world, id, s.target) {
            Some(i) => {
                next.phase = HuntPhase::Watch;
                next.intel = Some(i);
                next.venue = None;
            }
            None => return wait_for_word(world, id, next),
        }
    }
    if let (Some(o), Some(i)) = (told, next.intel) {
        let place = i.building.map_or_else(|| "on the street".to_string(), |b| format!("at {}", world.name_of(b)));
        let text =
            format!("{} asked {} after {}: seen {place}", world.name_of(id), world.name_of(o), world.name_of(s.target));
        line(world, &[id, o], text);
    }
    let phase = next.phase;
    world.hunts.insert(id, next);
    reindex(world);
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let t = Some(s.target);
    let mut steps = vec![step(ActionKind::GoTo(LocationKey::Intel), None)];
    if phase == HuntPhase::Ask {
        steps.push(step(ActionKind::AskAround, t));
        steps.push(step(ActionKind::GoTo(LocationKey::Intel), None));
    }
    steps.push(step(ActionKind::StakeOut, t));
    steps.push(step(ActionKind::Attack, t));
    Some(Plan { goal: GoalKind::Hunt, target: t, steps, started_tick: world.tick })
}

/// Round 9 Z1: word along the hunter's own people: the freshest Sighting
/// of `target` (within `fresh_sighting_hours`) held by his kin or a
/// friend: `(who told, intel)`.
fn asked_own(world: &World, hunter: EntityId, target: EntityId) -> Option<(EntityId, Intel)> {
    let horizon = world.tick.saturating_sub(Tick::from(world.config.hunt.fresh_sighting_hours) * TICKS_PER_HOUR);
    let n = world.neighbours.get(&hunter)?;
    n.iter()
        .copied()
        .filter(|&o| o != target && crate::systems::law::living(world, o))
        .filter(|&o| {
            world.edge(hunter, o).is_some_and(|e| {
                matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend)
            })
        })
        .filter_map(|o| sightings_of(world, o, target).filter(|x| x.0 >= horizon).map(|x| (x, o)))
        .max_by(|a, b| a.0 .0.cmp(&b.0 .0).then(b.1.cmp(&a.1)))
        .map(|((_, b, tile), o)| (o, Intel { building: b, tile, source: IntelSource::Asked }))
}

/// Z1: the man's known haunts (his Home, his post) the first within reach.
fn haunt_in_reach(world: &World, hunter: EntityId, target: EntityId) -> Option<Intel> {
    let home = world.comp::<Household>(target).and_then(|h| h.home).and_then(|h| intel_at(world, h, IntelSource::Home));
    let post = world.comp::<Job>(target).and_then(|j| j.employer).and_then(|e| intel_at(world, e, IntelSource::Habit));
    [home, post].into_iter().flatten().find(|&i| {
        let mut v = None;
        within_reach(world, hunter, HuntPhase::Watch, &mut v, Some(i))
    })
}

/// Z1: no word of the man and nowhere near to ask: the hunt is kept, asking
/// again after `wait_hours` (the goal held off that long), with the line
/// once a grudge. `None` (the plan's), `wait_hours` 0 as before.
fn wait_for_word(world: &mut World, id: EntityId, mut next: HuntState) -> Option<Plan> {
    let hours = world.config.hunt.wait_hours;
    if hours == 0 {
        return None;
    }
    let t = next.target;
    next.phase = HuntPhase::Ask;
    next.venue = None;
    next.intel = None;
    next.stakeout_until = None;
    world.hunts.insert(id, next);
    reindex(world);
    let until = world.tick + Tick::from(hours) * TICKS_PER_HOUR;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        let c = b.cooldowns.entry(GoalKind::Hunt).or_insert(until);
        *c = (*c).max(until);
    }
    if !noted(world, id, t, NOTED_WORD) {
        mark(world, id, t, NOTED_WORD);
        let (_, him) = pro(world, t);
        let (he, _) = pro(world, id);
        let text = format!(
            "{} asked after {}{}: nobody near had word of {him}; {he} waits for word",
            world.name_of(id),
            world.name_of(t),
            over(world, id, t)
        );
        line(world, &[id], text);
    }
    None
}

/// The pressure pass phase 4 (OPEN-194): `next`'s first `GoTo(Intel)` is
/// within `[distance.reach] stakeout_ticks` of where `id` sets out. An ask
/// venue past it gives way to the busiest Bar of the hunter's own district
/// when that one is within it (word is asked for nearer home); a stakeout
/// past it, or an ask with no near Bar, is `false`: the leg is not planned
/// and the goal cools (V8: legs of 174-250 min cut by Sleep every evening).
/// `true` off.
pub fn within_reach(
    world: &World,
    id: EntityId,
    phase: HuntPhase,
    venue: &mut Option<EntityId>,
    intel: Option<Intel>,
) -> bool {
    let Some(cap) = crate::systems::distance::reach(world).map(|r| r.stakeout_ticks) else { return true };
    let Some(from) = crate::systems::life::origin(world, id) else { return true };
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let door = |b: EntityId| world.comp::<Building>(b).map(|bd| bd.door);
    let near = |b: Option<EntityId>, tile: Option<crate::components::TilePos>| {
        if b.is_some() && b == here {
            return true;
        }
        b.and_then(door).or(tile).is_none_or(|t| crate::systems::distance::travel(world, from, t) <= cap)
    };
    match phase {
        HuntPhase::Ask => {
            if near(*venue, None) {
                return true;
            }
            match busiest_bar(world, world.district_of(from)).filter(|&b| near(Some(b), None)) {
                Some(b) => {
                    *venue = Some(b);
                    true
                }
                None => false,
            }
        }
        HuntPhase::Watch => intel.is_none_or(|i| near(i.building, Some(i.tile))),
    }
}

/// `LocationKey::Intel` for `agent`: the venue while asking, the intel
/// building while watching (`None` for a street intel).
pub fn intel_building(world: &World, agent: EntityId) -> Option<EntityId> {
    let s = chase(world, agent)?;
    match s.phase {
        HuntPhase::Ask => s.venue,
        HuntPhase::Watch => s.intel.and_then(|i| i.building),
    }
}

/// `LocationKey::Intel`'s tile for `agent` when it is no building.
pub fn intel_tile(world: &World, agent: EntityId) -> Option<crate::components::TilePos> {
    let s = chase(world, agent)?;
    match s.phase {
        HuntPhase::Ask => None,
        HuntPhase::Watch => s.intel.map(|i| i.tile),
    }
}

/// W22: a `GoTo(Intel)` starts: in `Watch` a Statistical target is
/// promoted to Coarse (it snaps to its phase door) and marked hunted.
pub fn on_goto_intel(world: &mut World, hunter: EntityId) {
    let Some(s) = chase(world, hunter) else { return };
    if s.phase != HuntPhase::Watch {
        return;
    }
    let t = s.target;
    if world.comp::<Brain>(t).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, t, Lod::Coarse);
    }
    reindex(world);
}

/// Is the target within 2 tiles of the hunter or in its building?
pub fn contact(world: &World, hunter: EntityId) -> bool {
    chase(world, hunter).is_some_and(|s| crate::systems::law::near(world, hunter, s.target, 2))
}
