use super::*;

/// A provocation's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provoke {
    Corner,
    Tag,
    Deal,
    Shakedown,
    Beating,
    /// An off-screen night of trespass (the daily roll).
    Night,
}

fn heat_of(w: &WarCfg, p: Provoke) -> f32 {
    match p {
        Provoke::Corner => w.heat_corner_hour,
        Provoke::Tag => w.heat_tag,
        Provoke::Deal => w.heat_deal,
        Provoke::Shakedown => w.heat_shakedown,
        Provoke::Beating => w.heat_beating,
        Provoke::Night => w.heat_stat,
    }
}

/// `gang`'s first adult body about cell `k` (the eyes a provocation needs).
fn eyes_in(world: &World, gang: EntityId, k: u32) -> Option<EntityId> {
    world.comp::<Gang>(gang)?.members.iter().copied().find(|&m| {
        world.comp::<Brain>(m).is_some_and(|b| b.lod != Lod::Statistical)
            && !world.has::<Sentence>(m)
            && !crate::systems::ages::is_protected(world, m)
            && crate::systems::law::living(world, m)
            && world.comp::<Position>(m).is_some_and(|p| cell_of(world, p.tile) == Some(k))
    })
}

/// The hooks' provocation (a deal, a shakedown): `actor`'s act at tile `t`
/// heats the cell for its holder when a holder's adult body there sees it (a
/// Statistical actor's act is seen off screen). Nobody under 18 provokes.
pub fn provoke_at(world: &mut World, actor: EntityId, t: TilePos, p: Provoke) {
    if world.turf.fighting {
        return;
    }
    let Some(w) = war_cfg(world) else { return };
    let heat = heat_of(w, p);
    let Some(k) = cell_of(world, t) else { return };
    let Some(h) = world.turf.cells.get(&k).and_then(|x| x.status.holder()) else { return };
    let Some(y) = world.gang_of(actor) else { return };
    if y == h || crate::systems::ages::is_protected(world, actor) {
        return;
    }
    let stat = world.comp::<Brain>(actor).is_some_and(|b| b.lod == Lod::Statistical);
    let seer = eyes_in(world, h, k);
    if seer.is_none() && !stat {
        return;
    }
    provoke(world, k, h, y, heat, Some(actor), seer, p);
}

/// From `law::resolve_fight_mods`: `winner` beat `loser`, a member of the
/// gang holding the cell where `loser` stands (not in a corner fight).
pub fn beaten(world: &mut World, winner: EntityId, loser: EntityId) {
    if world.turf.fighting || war_cfg(world).is_none() {
        return;
    }
    let Some(t) = world.comp::<Position>(loser).map(|p| p.tile) else { return };
    let (Some(gl), Some(gw)) = (world.gang_of(loser), world.gang_of(winner)) else { return };
    if gl == gw || holder_at(world, t) != Some(gl) {
        return;
    }
    provoke_at(world, winner, t, Provoke::Beating);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn provoke(
    world: &mut World,
    k: u32,
    h: EntityId,
    y: EntityId,
    heat: f32,
    actor: Option<EntityId>,
    seer: Option<EntityId>,
    p: Provoke,
) {
    let Some(w) = war_cfg(world).cloned() else { return };
    if heat <= 0.0 || in_truce(world, h, y) || cracked(world, k) || !world.has::<Gang>(y) {
        return;
    }
    // A gang lying low provokes nobody (its members' walks are not a push).
    if lies_low(world, y) || order_of(world, y) == Some(Order::LieLow) {
        return;
    }
    let now = world.tick;
    let Some(cell) = world.turf.cells.get_mut(&k) else { return };
    if cell.status != Status::Held(h) {
        return;
    }
    let v = match cell.heat.iter_mut().find(|e| e.0 == y) {
        Some(e) => {
            e.1 += heat;
            e.1
        }
        None => {
            cell.heat.push((y, heat));
            heat
        }
    };
    let open = cell.fight.is_some();
    crate::systems::ownership::jobs2_row(world).turf_provocations += 1;
    // Told once a cell a night, when a Full body did it or saw it.
    let night = night_of(world);
    let full = |id: Option<EntityId>| id.and_then(|x| world.comp::<Brain>(x)).is_some_and(|b| b.lod == Lod::Full);
    let tell = p != Provoke::Night && (full(actor) || full(seer)) && world.turf.trespass_told.get(&k) != Some(&night);
    if let Some(a) = actor.filter(|_| tell) {
        world.turf.trespass_told.insert(k, night);
        let (hn, yn, at) = (gname(world, h), gname(world, y), around(world, k));
        let by = match world.turf.cells.get(&k).and_then(|x| x.landmark) {
            Some(b) => format!("by {}", world.name_of(b)),
            None => format!("of {at}"),
        };
        let phrase = match p {
            Provoke::Corner => format!("hanging on {} corner {by}", poss(&hn)),
            Provoke::Tag => format!("spraying {} mark on the wall at {} corner {by}", poss(&yn), poss(&hn)),
            Provoke::Deal => format!("dealing on {} blocks ({at})", poss(&hn)),
            Provoke::Shakedown => format!("shaking down a door on {} blocks ({at})", poss(&hn)),
            Provoke::Beating | Provoke::Night => format!("beating one of {hn} on their own blocks ({at})"),
        };
        let an = world.name_of(a);
        let text = match seer {
            Some(s) => format!("{} of {hn} saw {an} of {yn} {phrase}", world.name_of(s)),
            None => format!("{an} of {yn} was seen {phrase}"),
        };
        let mut actors = vec![a];
        actors.extend(seer);
        actors.extend([h, y]);
        world.push_event(EventKind::Turf, &actors, text);
    }
    // A holder lying low does not answer (the heat stands).
    if v >= w.answer_heat && !open && !lies_low(world, h) {
        let due = if p == Provoke::Night { evening_due(world, &w) } else { now + hours(w.answer_window_hours) };
        open_fight(world, k, FightKind::Answer, h, y, due);
    }
}

/// Today's evening fight hour: `answer_window_hours` past `corner_from`.
pub(super) fn evening_due(world: &World, w: &WarCfg) -> Tick {
    let from = cfg(world).map_or(1080, |c| c.corner_from);
    let start = world.tick - Tick::from(world.tick_of_day());
    let due = start + Tick::from(from) + hours(w.answer_window_hours);
    if due <= world.tick {
        world.tick + hours(w.answer_window_hours)
    } else {
        due
    }
}

/// A corner fight scheduled on cell `k` (`side` on it, `by` at it): the
/// moving side's leader decides it (his line), its people muster.
pub(super) fn open_fight(world: &mut World, k: u32, kind: FightKind, side: EntityId, by: EntityId, due: Tick) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let now = world.tick;
    let Some(cell) = world.turf.cells.get_mut(&k) else { return };
    if cell.fight.is_some() {
        return;
    }
    cell.fight = Some(Fight { kind, side, by, opened: now, due });
    cell.heat.retain(|e| e.0 != by);
    world.turf.open.push((k, side, by));
    crate::systems::ownership::jobs2_row(world).turf_answers += 1;
    let (mover, other) = match kind {
        FightKind::Answer => (side, by),
        FightKind::Front | FightKind::Retake => (by, side),
    };
    let n = muster_pool(world, &w, k, mover).len();
    let at = corner_name(world, k);
    let (lead, on) = (who_leads(world, mover), gname(world, other));
    let theirs = match n {
        0 => "nobody of theirs".to_string(),
        1 => "the one of theirs".to_string(),
        n => format!("{n} of theirs"),
    };
    let when = if due.saturating_sub(now) > 6 * TICKS_PER_HOUR { " tonight" } else { "" };
    let text = match kind {
        _ if n == 0 => format!("{} had nobody about the blocks to send to {at}: {on} are on it", gname(world, mover)),
        FightKind::Answer => format!("{lead} called {theirs} about the blocks to {at}{when}: {on} were on their turf"),
        FightKind::Front => format!("{lead} called {theirs} about the blocks to {at}{when}: {on} claim the blocks too"),
        FightKind::Retake => {
            format!("{lead} sent {theirs} about the blocks back to {at}{when}: {on} took the blocks from them")
        }
    };
    if kind == FightKind::Answer {
        crate::systems::gang::push_shock(world, side, Shock::Trespass { by: Some(by) });
    }
    let mut actors: Vec<EntityId> = leader_of(world, mover).into_iter().collect();
    actors.extend([mover, other]);
    actors.extend(world.turf.cells.get(&k).and_then(|x| x.landmark));
    // Round 7 W2: the men called hear the call.
    actors.extend(hearers(world, mover, Some(k)));
    war_line(world, actors, text);
}

/// `g`'s presence over the window in cell `k` and the cells around it.
pub(super) fn local_presence(world: &World, w: &WarCfg, k: u32, g: EntityId) -> f32 {
    let mut cells = vec![k];
    if w.muster_cells > 0 {
        cells.extend(neighbours(world, k));
    }
    cells
        .iter()
        .filter_map(|c| world.turf.cells.get(c))
        .flat_map(|x| x.days.iter().flat_map(|d| d.iter().filter(|p| p.gang == g).map(|p| p.p)))
        .sum()
}

/// A side's fighters at corner `t` of cell `k`: its adult bodies within
/// `muster_reach_tiles` of the corner, then (off screen) its Statistical
/// members living about the cell, nearest first, to `n` (at most
/// `side_max`). The Statistical ones are stood at the corner (promoted to
/// Coarse on screen). Nobody under 18 (`can_fight`).
pub(super) fn fighters(world: &mut World, w: &WarCfg, k: u32, g: EntityId, t: TilePos, n: usize) -> Vec<EntityId> {
    let members = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
    let mut bodies: Vec<EntityId> = Vec::new();
    let mut far: Vec<(u32, EntityId)> = Vec::new();
    for m in members {
        if !can_fight(world, m) {
            continue;
        }
        let stat = world.comp::<Brain>(m).is_some_and(|b| b.lod == Lod::Statistical);
        if !stat {
            if world.comp::<Position>(m).is_some_and(|p| p.tile.manhattan(t) <= w.muster_reach_tiles) {
                bodies.push(m);
            }
        } else if home_cell(world, m).is_some_and(|h| cell_dist(world, h, k) <= w.muster_cells) {
            far.push((home_door(world, m).map_or(u32::MAX, |d| d.manhattan(t)), m));
        }
    }
    crate::systems::raid::by_strength(world, &mut bodies);
    bodies.truncate(w.side_max);
    far.sort_unstable();
    let want = n.min(w.side_max);
    let tick = world.tick;
    let shown = crate::systems::lod::on_screen(world, t);
    for (_, m) in far {
        if bodies.len() >= want {
            break;
        }
        if shown {
            crate::systems::lod::set_lod(world, m, Lod::Coarse);
        }
        world.remove_from_building(m);
        if let Some(p) = world.comp_mut::<Position>(m) {
            p.tile = t;
            p.building = None;
            p.floor = 0;
            p.entered = tick;
        }
        bodies.push(m);
    }
    bodies
}

/// Round 7 W2 (`OPEN-241`): gang `g`'s members who hear a war line: its
/// free, living adults living in cell `k` or within `muster_cells` of it
/// (with `k` None, all of them: the gang's own order, lying low or a
/// truce). V15: only the leader's diary heard his gang's war. Never a minor
/// (CLAUDE.md, the under-18 rule: no war line reaches a child as one of its
/// men).
pub(super) fn hearers(world: &World, g: EntityId, k: Option<u32>) -> Vec<EntityId> {
    let Some(w) = war_cfg(world) else { return Vec::new() };
    let Some(x) = world.comp::<Gang>(g) else { return Vec::new() };
    x.members
        .iter()
        .copied()
        .filter(|&m| {
            crate::systems::law::living(world, m)
                && world.has::<Brain>(m)
                && !world.has::<Sentence>(m)
                && !crate::systems::ages::is_protected(world, m)
                && k.is_none_or(|k| home_cell(world, m).is_some_and(|h| cell_dist(world, h, k) <= w.muster_cells))
        })
        .collect()
}

/// Round 7 W2 (`OPEN-241`): a war line (the call, the fight, the war,
/// lying low, the truce, the crackdown, the quiet), its actors once each;
/// the gang members on it who do not lead their gang are counted
/// (`war_member_lines`).
pub(super) fn war_line(world: &mut World, actors: Vec<EntityId>, text: String) {
    let mut seen: BTreeSet<EntityId> = BTreeSet::new();
    let actors: Vec<EntityId> = actors.into_iter().filter(|&a| seen.insert(a)).collect();
    let members = actors
        .iter()
        .filter(|&&a| world.gang_of(a).is_some_and(|g| world.comp::<Gang>(g).is_some_and(|x| x.leader != Some(a))))
        .count() as u32;
    if members > 0 {
        crate::systems::ownership::jobs2_row(world).war_member_lines += members;
    }
    world.push_event(EventKind::Turf, &actors, text);
}

/// Names, up to three ("A, B and 2 more").
pub(super) fn names(world: &World, v: &[EntityId]) -> String {
    let shown: Vec<String> = v.iter().take(3).map(|&m| world.name_of(m)).collect();
    let more = v.len().saturating_sub(shown.len());
    match (shown.len(), more) {
        (0, _) => String::new(),
        (1, 0) => shown[0].clone(),
        (_, 0) => format!("{} and {}", shown[..shown.len() - 1].join(", "), shown[shown.len() - 1]),
        _ => format!("{} and {more} more", shown.join(", ")),
    }
}

/// The hour (from [`hourly`]): the corner's trespassers heat the cells
/// their holders' bodies watch; the corner fights due are fought.
pub(super) fn war_hourly(
    world: &mut World,
    trespass: &[(u32, EntityId, EntityId)],
    eyes: &BTreeMap<(u32, EntityId), EntityId>,
) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let night = night_of(world);
    for &(k, y, m) in trespass {
        let Some(h) = world.turf.cells.get(&k).and_then(|x| x.status.holder()) else { continue };
        let Some(&seer) = eyes.get(&(k, h)) else { continue };
        // His mark by the corner, once a night (told first), then the hour.
        if world.turf.tagged.get(&m) != Some(&night) {
            world.turf.tagged.insert(m, night);
            provoke(world, k, h, y, w.heat_tag, Some(m), Some(seer), Provoke::Tag);
        }
        provoke(world, k, h, y, w.heat_corner_hour, Some(m), Some(seer), Provoke::Corner);
    }
    if world.turf.open.is_empty() {
        return;
    }
    rouse(world, &w);
    let now = world.tick;
    let due: Vec<u32> = world
        .turf
        .open
        .iter()
        .filter(|&&(k, _, _)| world.turf.cells.get(&k).and_then(|x| x.fight).is_none_or(|f| f.due <= now))
        .filter(|&&(k, _, _)| !waits_for_called(world, k))
        .map(|x| x.0)
        .collect();
    for k in due {
        resolve(world, k);
    }
}

/// Round 8 X4 (`OPEN-241`): a corner fight due within `rouse_hours` rouses
/// both sides' called members (`muster_pool`) asleep in or about the cell
/// (their body within `muster_cells` of it, Full or Coarse), off shift now
/// and at the fight, above `rouse_min_energy`: the Sleep ends (a wake line)
/// and the brain, free of it, weighs the corner's muster. Once a fight.
/// V16 (100k): called bodies at the corner 1,586 of 3,297; W2's diagnosis, a
/// third asleep (fights at 01:00-05:00 from beatings). The pool is adults
/// only (`can_fight`: the under-18 rule), and a man inside, cuffed or down is
/// none of it.
fn rouse(world: &mut World, w: &WarCfg) {
    if w.rouse_hours == 0 {
        return;
    }
    let now = world.tick;
    world.turf.roused.retain(|_, due| *due + TICKS_PER_HOUR > now);
    let lead = hours(w.rouse_hours);
    let fights: Vec<(u32, Fight)> = world
        .turf
        .open
        .iter()
        .filter_map(|&(k, _, _)| world.turf.cells.get(&k).and_then(|x| x.fight).map(|f| (k, f)))
        .filter(|(_, f)| f.due > now && f.due <= now + lead)
        .collect();
    let due_tod = |due: Tick| (due % crate::time::TICKS_PER_DAY) as u16;
    for (k, f) in fights {
        let at = corner_name(world, k);
        for g in [f.side, f.by] {
            let gn = gname(world, g);
            for m in muster_pool(world, w, k, g) {
                if world.turf.roused.contains_key(&m) || !crate::systems::presence::asleep(world, m) {
                    continue;
                }
                let Some(b) = world.comp::<Brain>(m) else { continue };
                if b.lod == Lod::Statistical {
                    continue;
                }
                if world.comp::<crate::components::Needs>(m).is_none_or(|n| n.energy < w.rouse_min_energy) {
                    continue;
                }
                let tod = world.tick_of_day();
                if world.comp::<Job>(m).is_some_and(|j| j.on_shift(tod) || j.on_shift(due_tod(f.due))) {
                    continue;
                }
                let near = world
                    .comp::<Position>(m)
                    .and_then(|p| cell_of(world, p.tile))
                    .is_some_and(|c| cell_dist(world, c, k) <= w.muster_cells);
                if !near {
                    continue;
                }
                world.turf.roused.insert(m, f.due);
                world.abort_plan(m);
                // Awake: the next tick's think weighs the call (not the
                // Sleep he was in, replanned without one).
                if let Some(b) = world.comp_mut::<Brain>(m) {
                    b.current_goal = None;
                    b.last_urgent_think_tick = 0;
                }
                crate::systems::ownership::jobs2_row(world).war_roused += 1;
                let text = format!("{} was woken: {gn} are called to {at} for the fight", world.name_of(m));
                world.push_event(EventKind::Turf, &[m, g], text);
            }
        }
    }
}

/// Round 7 W2 (`OPEN-241`): a corner fight due waits one hour for the
/// mover's called men on their way to the corner, due there within `[crew]
/// march_wait_ticks` (as a raid's column waits for its tail), at most
/// `march_wait_ticks` past its hour.
pub(super) fn waits_for_called(world: &World, k: u32) -> bool {
    let Some(f) = world.turf.cells.get(&k).and_then(|x| x.fight) else { return false };
    let Some(wait) = crate::systems::crew::cfg(world).map(|c| c.march_wait_ticks).filter(|&t| t > 0) else {
        return false;
    };
    if world.tick >= f.due + wait {
        return false;
    }
    let mover = if f.kind == FightKind::Answer { f.side } else { f.by };
    let t = corner_tile(world, k);
    let Some(w) = war_cfg(world) else { return false };
    let reach = w.muster_reach_tiles;
    muster_pool(world, w, k, mover).into_iter().any(|m| {
        let Some(b) = world.comp::<Brain>(m) else { return false };
        let on_way = b.plan.as_ref().is_some_and(|p| {
            p.goal == GoalKind::GangWork
                && p.steps.get(usize::from(b.plan_step)).map(|s| s.action) == Some(ActionKind::GoTo(LocationKey::Spot))
        }) && world.unwind.get(&m).is_some_and(|u| u.spot == Some(t));
        if !on_way {
            return false;
        }
        let eta = match &b.exec {
            crate::exec::ExecState::GotoTimed { arrive_tick, .. } => arrive_tick.saturating_sub(world.tick),
            _ => world.comp::<Position>(m).map_or(Tick::MAX, |p| {
                world.config.exec.walk_ticks(Tick::from(p.tile.manhattan(t).saturating_sub(reach)))
            }),
        };
        eta <= wait
    })
}

/// The war between `a` and `b` (started over cell `k` when new: told).
pub(super) fn war_entry(world: &mut World, a: EntityId, b: EntityId, k: u32) {
    let key = pair(a, b);
    let now = world.tick;
    if let Some(x) = world.turf.wars.get_mut(&key) {
        x.last = now;
        return;
    }
    let size = [fit(world, key.0), fit(world, key.1)];
    world.turf.wars.insert(key, War { started: now, last: now, cell: k, size, ..Default::default() });
    crate::systems::ownership::jobs2_row(world).turf_wars += 1;
    let side = |world: &World, g: EntityId| {
        let n = if key.0 == g { size[0] } else { size[1] };
        match leader_of(world, g) {
            Some(l) => format!("{} leads {n} of {}", world.name_of(l), gname(world, g)),
            None => format!("{n} of {}", gname(world, g)),
        }
    };
    let text = format!(
        "{} and {} are at war over {} ({}; {})",
        gname(world, a),
        gname(world, b),
        around(world, k),
        side(world, a),
        side(world, b)
    );
    let mut actors = vec![a, b];
    actors.extend(leader_of(world, a));
    actors.extend(leader_of(world, b));
    actors.extend(world.turf.cells.get(&k).and_then(|x| x.landmark));
    // Round 8 X4 (`OPEN-241`): the war's start reaches all of each side's
    // adults, as its end does (V16 `cultist_80865`: a member far from the
    // cell heard only the lie-low line of a war he never heard begin).
    actors.extend(hearers(world, a, None));
    actors.extend(hearers(world, b, None));
    war_line(world, actors, text);
}
