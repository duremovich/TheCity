use super::*;

/// The fight scheduled on cell `k`, fought: the hold or the flip, the war's
/// tally, the law's attention.
pub(super) fn resolve(world: &mut World, k: u32) {
    world.turf.open.retain(|x| x.0 != k);
    let Some(f) = world.turf.cells.get_mut(&k).and_then(|x| x.fight.take()) else { return };
    let Some(w) = war_cfg(world).cloned() else { return };
    let (side, by) = (f.side, f.by);
    if !world.has::<Gang>(side) || !world.has::<Gang>(by) || in_truce(world, side, by) || cracked(world, k) {
        return;
    }
    let status = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status);
    if !status.involves(side) {
        return;
    }
    let now = world.tick;
    let t = corner_tile(world, k);
    let at = corner_name(world, k);
    // The aggressor: the holder pushing back an answer's trespassers, else
    // the side coming at the cell.
    let (ag, dg) = if f.kind == FightKind::Answer { (side, by) } else { (by, side) };
    let (pa, pd) = (local_presence(world, &w, k, ag), local_presence(world, &w, k, dg));
    let top = pa.max(pd).max(1e-3);
    let n = |p: f32| ((w.side_max as f32 * p / top).round() as usize).clamp(1, w.side_max.max(1));
    let (na, nd) = (n(pa), n(pd));
    // Round 7 W2 (`OPEN-241`): the mover's called bodies, and those at the
    // corner now (before the off-screen men are stood there).
    {
        let called: Vec<EntityId> = muster_pool(world, &w, k, ag)
            .into_iter()
            .filter(|&m| world.comp::<Brain>(m).is_some_and(|b| b.lod != Lod::Statistical))
            .collect();
        let came: Vec<EntityId> = called
            .iter()
            .copied()
            .filter(|&m| world.comp::<Position>(m).is_some_and(|p| p.tile.manhattan(t) <= w.muster_reach_tiles))
            .collect();
        let roused = came.iter().filter(|m| world.turf.roused.contains_key(m)).count() as u32;
        let row = crate::systems::ownership::jobs2_row(world);
        row.war_called += called.len() as u32;
        row.war_came += came.len() as u32;
        row.war_roused_came += roused;
    }
    let mut agg = fighters(world, &w, k, ag, t, na);
    let mut def = fighters(world, &w, k, dg, t, nd);
    let landmark = world.turf.cells.get(&k).and_then(|x| x.landmark);
    if agg.is_empty() || def.is_empty() {
        if agg.is_empty() && def.is_empty() {
            return;
        }
        crate::systems::ownership::jobs2_row(world).turf_unopposed += 1;
        let (here, here_g, gone_g) = if agg.is_empty() { (def, dg, ag) } else { (agg, ag, dg) };
        let mut actors = vec![here_g, gone_g];
        actors.extend(here.iter().copied());
        actors.extend(landmark);
        let text = format!(
            "{} came to {at} ({}): nobody of {} there",
            gname(world, here_g),
            names(world, &here),
            poss(&gname(world, gone_g))
        );
        actors.extend(hearers(world, here_g, Some(k)));
        actors.extend(hearers(world, gone_g, Some(k)));
        war_line(world, actors, text);
        // An answer nobody of the holder's came to: the trespassers stand on
        // it (contested); a cell nobody defends is walked onto.
        let now_status = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status);
        let next = match f.kind {
            FightKind::Answer if here_g == by => Some(Status::Contested(side, by)),
            FightKind::Answer => None,
            _ if here_g == by => Some(Status::Held(by)),
            _ => None,
        };
        if let Some(next) = next.filter(|&x| x != now_status) {
            take(world, &w, k, now_status, next, here_g, gone_g);
        }
        if let Some(x) = world.turf.cells.get_mut(&k) {
            x.heat.clear();
        }
        return;
    }
    world.turf.fighting = true;
    crate::systems::raid::by_strength(world, &mut agg);
    crate::systems::raid::by_strength(world, &mut def);
    let (agg0, def0) = (agg.clone(), def.clone());
    let _tally = crate::systems::raid::fight_out(world, &mut agg, &mut def, t, &at, 1.0, None);
    world.turf.fighting = false;
    let (winner, loser) = if def.is_empty() { (ag, dg) } else { (dg, ag) };
    let dead: Vec<EntityId> =
        agg0.iter().chain(def0.iter()).copied().filter(|&m| !crate::systems::law::living(world, m)).collect();
    {
        let row = crate::systems::ownership::jobs2_row(world);
        row.turf_fights += 1;
        row.turf_war_deaths += dead.len() as u32;
    }
    war_entry(world, ag, dg, k);
    // The war's men, and its losses: each side's fallen in this fight
    // (beaten or dead), then its men's later deaths, jailings, leavings.
    let key = pair(ag, dg);
    if let Some(x) = world.turf.wars.get_mut(&key) {
        x.fights += 1;
        x.deaths += dead.len() as u32;
        for (g, before, after) in [(ag, &agg0, &agg), (dg, &def0, &def)] {
            let i = usize::from(key.0 != g);
            for &m in before {
                if after.contains(&m) {
                    x.men[i].insert(m);
                } else {
                    x.men[i].remove(&m);
                    x.losses[i].push_back(now);
                }
            }
        }
    }
    // Round 8 X4 (`OPEN-241`): the fallen are this war's loss only; his
    // gang's other wars let him go (his later death or jailing is not a
    // second loss there).
    let fallen: Vec<(EntityId, EntityId)> = [(ag, &agg0, &agg), (dg, &def0, &def)]
        .iter()
        .flat_map(|(g, before, after)| before.iter().filter(|m| !after.contains(m)).map(move |&m| (*g, m)))
        .collect();
    for (other, war) in world.turf.wars.iter_mut() {
        if *other == key {
            continue;
        }
        for &(g, m) in &fallen {
            if other.0 == g {
                war.men[0].remove(&m);
            } else if other.1 == g {
                war.men[1].remove(&m);
            }
        }
    }
    let old = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status);
    let retaken = world.turf.cells.get(&k).and_then(|x| x.lost).is_some_and(|(g, u)| g == winner && u > now);
    let flips = old != Status::Held(winner);
    let (wn, ln) = (gname(world, winner), gname(world, loser));
    let tail = if dead.is_empty() { String::new() } else { format!("; {} dead", names(world, &dead)) };
    let count = format!("{} against {}", agg0.len(), def0.len());
    let text = match (f.kind, winner == ag) {
        (FightKind::Answer, true) => format!("{wn} pushed {ln} off {at}, {count}{tail}"),
        (FightKind::Answer, false) => format!("{wn} beat {ln} back on {at}, {count}, and hold it now{tail}"),
        (FightKind::Front, true) => format!("{wn} took {at} from {ln}, {count}{tail}"),
        (FightKind::Front, false) => format!("{wn} drove {ln} off {at}, {count}{tail}"),
        (FightKind::Retake, true) => format!("{wn} came back for {at} and took it back from {ln}, {count}{tail}"),
        (FightKind::Retake, false) => format!("{wn} held {at} when {ln} came back for it, {count}{tail}"),
    };
    let mut actors = vec![winner, loser];
    actors.extend(agg0.iter().chain(def0.iter()).copied());
    actors.extend(landmark);
    actors.extend(hearers(world, winner, Some(k)));
    actors.extend(hearers(world, loser, Some(k)));
    war_line(world, actors, text);
    if flips {
        take(world, &w, k, old, Status::Held(winner), winner, loser);
        if retaken {
            crate::systems::ownership::jobs2_row(world).turf_retaken += 1;
        }
    }
    if let Some(x) = world.turf.cells.get_mut(&k) {
        x.won = Some((winner, now + days(w.hold_lock_days)));
        x.heat.clear();
        x.law_heat += 1.0 + dead.len() as f32 * w.crackdown_death_w;
    }
    if world.turf.cells.get(&k).is_some_and(|x| x.law_heat >= w.crackdown_heat) {
        crackdown(world, &w, k, ag, dg);
    }
}

/// Cell `k` passes from `old` to `new` in the war (`set_status`, told): the
/// winner's lock, the loser's window to come back, the war's flip.
pub(super) fn take(world: &mut World, w: &WarCfg, k: u32, old: Status, new: Status, winner: EntityId, loser: EntityId) {
    let now = world.tick;
    set_status(world, k, old, new, true, now);
    if let Some(x) = world.turf.cells.get_mut(&k) {
        if new == Status::Held(winner) {
            x.won = Some((winner, now + days(w.hold_lock_days)));
            x.lost = old.involves(loser).then_some((loser, now + days(w.retake_days)));
        }
    }
    if new.holder().is_some() {
        crate::systems::ownership::jobs2_row(world).turf_flips += 1;
        if let Some(x) = world.turf.wars.get_mut(&pair(winner, loser)) {
            x.flips += 1;
        }
    }
    reindex(world);
}

/// The law's crackdown on cell `k` (its fights past `crackdown_heat`): no
/// corner held there for `crackdown_days`, the fights off, the district's
/// beat walks its Homes ([`crackdown_beat`]).
pub(super) fn crackdown(world: &mut World, w: &WarCfg, k: u32, a: EntityId, b: EntityId) {
    if world.law().is_none() {
        return;
    }
    let now = world.tick;
    if let Some(x) = world.turf.cells.get_mut(&k) {
        x.crackdown_until = now + days(w.crackdown_days);
        x.law_heat = 0.0;
        x.heat.clear();
        x.fight = None;
    }
    world.turf.open.retain(|x| x.0 != k);
    crate::systems::ownership::jobs2_row(world).turf_crackdowns += 1;
    let text = format!(
        "The law cracked down on {}: {} and {} kept off the corner, the beat walks its Homes for {} days",
        around(world, k),
        gname(world, a),
        gname(world, b),
        w.crackdown_days
    );
    let mut actors = vec![a, b];
    actors.extend(leader_of(world, a));
    actors.extend(leader_of(world, b));
    actors.extend(world.turf.cells.get(&k).and_then(|x| x.landmark));
    // Round 7 W2: the men living on the blocks hear the law came for them.
    actors.extend(hearers(world, a, Some(k)));
    actors.extend(hearers(world, b, Some(k)));
    war_line(world, actors, text);
}

/// `law::district_route`: a cell of district `d` the law cracks down on
/// gives the beat its Homes (the nearest the corner, up to five).
pub fn crackdown_beat(world: &World, d: DistrictId) -> Option<Vec<EntityId>> {
    if !on(world) {
        return None;
    }
    let now = world.tick;
    let k = world
        .turf
        .cells
        .iter()
        .find(|(&k, x)| x.crackdown_until > now && world.district_of(centre(world, k)) == d)
        .map(|(&k, _)| k)?;
    let t = corner_tile(world, k);
    let mut homes: Vec<(u32, EntityId)> = world
        .district(d)
        .homes
        .iter()
        .copied()
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && cell_of(world, b.door) == Some(k))
        .map(|(h, b)| (b.door.manhattan(t), h))
        .collect();
    homes.sort_unstable();
    let v: Vec<EntityId> = homes.into_iter().take(5).map(|(_, h)| h).collect();
    (!v.is_empty()).then_some(v)
}

/// From `gang::on_member_killed`, `on_member_arrested`, `leave`: a war
/// side's loss (an adult's: a child is never a combatant). Round 8 X4
/// (`OPEN-241`): a man who stood in two of his gang's wars is one loss, to
/// the war he fought last (V16 `cultist_80865 d26 00:05`: a death in the
/// war with the Unplugged 4 also counted in the war with the Unplugged 5,
/// and the lie-low line named the war he did not fall in).
pub fn note_loss(world: &mut World, gang: EntityId, id: EntityId) {
    if world.turf.wars.is_empty() || crate::systems::ages::is_protected(world, id) {
        return;
    }
    let now = world.tick;
    let side = |key: &(EntityId, EntityId)| {
        if key.0 == gang {
            Some(0)
        } else if key.1 == gang {
            Some(1)
        } else {
            None
        }
    };
    let last = world
        .turf
        .wars
        .iter()
        .filter_map(|(key, war)| side(key).filter(|&i| war.men[i].contains(&id)).map(|_| (war.last, *key)))
        .max()
        .map(|x| x.1);
    for (key, war) in world.turf.wars.iter_mut() {
        let Some(i) = side(key) else { continue };
        if war.men[i].remove(&id) && Some(*key) == last {
            war.losses[i].push_back(now);
        }
    }
}

/// From `raid::settle`: a raid won or a Hideout sacked moves `loser`'s cells
/// within `raid_cells` of the Hideout `at` raided (the city milestone C9b: its
/// seat or the district Hideout the raid landed on) to `winner`.
pub fn raid_moves_at(world: &mut World, winner: EntityId, loser: EntityId, at: EntityId) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let Some(hk) = door_of(world, at).and_then(|t| cell_of(world, t)) else {
        return;
    };
    let cells: Vec<(u32, Status)> = world
        .turf
        .cells
        .iter()
        .filter(|(&k, x)| x.status.involves(loser) && cell_dist(world, k, hk) <= w.raid_cells)
        .map(|(&k, x)| (k, x.status))
        .collect();
    if cells.is_empty() {
        return;
    }
    war_entry(world, winner, loser, cells[0].0);
    for (k, old) in cells {
        take(world, &w, k, old, Status::Held(winner), winner, loser);
        crate::systems::ownership::jobs2_row(world).turf_raid_moves += 1;
    }
}

/// Midnight (after the holders): the heat cooled, the wars' exhaustion,
/// truces and quiet, the off-screen rolls (a night's trespass on a border,
/// a contested cell's front, the loser's counter-attack).
pub(super) fn war_daily(world: &mut World) {
    let Some(w) = war_cfg(world).cloned() else { return };
    let now = world.tick;
    for x in world.turf.cells.values_mut() {
        for e in &mut x.heat {
            e.1 *= w.heat_keep_day;
        }
        x.heat.retain(|e| e.1 >= 0.05);
        x.law_heat *= w.crackdown_keep_day;
        if x.law_heat < 0.05 {
            x.law_heat = 0.0;
        }
        if x.won.is_some_and(|(_, t)| t <= now) {
            x.won = None;
        }
        if x.lost.is_some_and(|(_, t)| t <= now) {
            x.lost = None;
        }
    }
    world.turf.truces.retain(|_, t| *t > now);
    world.turf.lie_low.retain(|_, t| *t > now);
    world.turf.roused.retain(|_, due| *due + TICKS_PER_HOUR > now);
    let night = night_of(world);
    world.turf.tagged.retain(|_, n| *n + 1 >= night);
    world.turf.trespass_told.retain(|_, n| *n + 1 >= night);
    let cells = &world.turf.cells;
    world.turf.open.retain(|&(k, _, _)| cells.get(&k).is_some_and(|x| x.fight.is_some()));
    // The wars: the spent, the truce, the quiet.
    let window = days(w.exhaust_days);
    let keys: Vec<(EntityId, EntityId)> = world.turf.wars.keys().copied().collect();
    for key in keys {
        let (a, b) = key;
        if !world.has::<Gang>(a) || !world.has::<Gang>(b) {
            world.turf.wars.remove(&key);
            continue;
        }
        let Some(war) = world.turf.wars.get_mut(&key) else { continue };
        let mut spent: Vec<(usize, usize)> = Vec::new();
        for i in 0..2 {
            while war.losses[i].front().is_some_and(|&t| t + window < now) {
                war.losses[i].pop_front();
            }
            let mark = (w.exhaust_share * f32::from(war.size[i])).ceil().max(f32::from(w.exhaust_min)) as usize;
            if war.exhausted[i].is_none_or(|t| t + window < now) && war.losses[i].len() >= mark {
                war.exhausted[i] = Some(now);
                spent.push((i, war.losses[i].len()));
            }
        }
        let both = war.exhausted.iter().all(|e| e.is_some_and(|t| t + window >= now));
        let quiet = war.last + days(w.war_quiet_days) <= now;
        let (size, started, fights, flips, deaths) = (war.size, war.started, war.fights, war.flips, war.deaths);
        for (i, lost) in spent {
            let (g, o) = if i == 0 { (a, b) } else { (b, a) };
            spend(world, &w, g, o, lost, size[i], started);
        }
        if both {
            truce(world, &w, key);
            world.turf.wars.remove(&key);
        } else if quiet {
            let since = started / crate::time::TICKS_PER_DAY;
            let text = format!(
                "The war between {} and {} went quiet (since day {since}: {fights} corner fights, {deaths} dead, \
                 {flips} cells changed hands)",
                gname(world, a),
                gname(world, b)
            );
            let mut actors = vec![a, b];
            actors.extend(hearers(world, a, None));
            actors.extend(hearers(world, b, None));
            war_line(world, actors, text);
            world.turf.wars.remove(&key);
        }
    }
    rolls(world, &w);
    reindex(world);
}

/// A side spent: its leader calls it off the corners, it lies low.
fn spend(world: &mut World, w: &WarCfg, g: EntityId, other: EntityId, lost: usize, size: u16, started: Tick) {
    let now = world.tick;
    world.turf.lie_low.insert(g, now + days(w.lielow_days));
    crate::systems::ownership::jobs2_row(world).turf_exhausted += 1;
    let since = started / crate::time::TICKS_PER_DAY;
    let gn = gname(world, g);
    let what = format!(
        "{lost} of their {size} beaten, dead, jailed or gone since the war with {} began on day {since}",
        gname(world, other)
    );
    let text = match leader_of(world, g) {
        Some(l) => format!("{} called {gn} off the corners to lie low: {what}", world.name_of(l)),
        None => format!("{gn} are spent and lie low: {what}"),
    };
    let mut actors = vec![g, other];
    actors.extend(leader_of(world, g));
    actors.extend(hearers(world, g, None));
    war_line(world, actors, text);
}

/// Two spent sides' leaders strike a truce: the contested cells between
/// them go to the side ahead, the line between them held `truce_days`, the
/// fights between them called off.
pub(super) fn truce(world: &mut World, w: &WarCfg, key: (EntityId, EntityId)) {
    let (a, b) = key;
    let now = world.tick;
    let until = now + days(w.truce_days);
    world.turf.truces.insert(key, until);
    crate::systems::ownership::jobs2_row(world).turf_truces += 1;
    let split: Vec<(u32, Status, EntityId)> = world
        .turf
        .cells
        .iter()
        .filter_map(|(&k, x)| match x.status {
            Status::Contested(p, q) if pair(p, q) == key => Some((k, x.status, p)),
            _ => None,
        })
        .collect();
    for &(k, old, first) in &split {
        set_status(world, k, old, Status::Held(first), false, now);
    }
    // The line: every cell either holds that borders the other's, held.
    let mut line: Vec<u32> = Vec::new();
    for (&k, x) in &world.turf.cells {
        let Some(h) = x.status.holder().filter(|&h| h == a || h == b) else { continue };
        let o = if h == a { b } else { a };
        if neighbours(world, k).iter().any(|n| world.turf.cells.get(n).is_some_and(|y| y.status == Status::Held(o))) {
            line.push(k);
        }
    }
    for &k in &line {
        if let Some(x) = world.turf.cells.get_mut(&k) {
            if let Some(h) = x.status.holder() {
                x.won = Some((h, until));
            }
            if x.fight.is_some_and(|f| pair(f.side, f.by) == key) {
                x.fight = None;
            }
            x.heat.retain(|e| e.0 != a && e.0 != b);
        }
    }
    let cells = &world.turf.cells;
    world.turf.open.retain(|&(k, _, _)| cells.get(&k).is_some_and(|x| x.fight.is_some()));
    let held = |g: EntityId| world.turf.cells.values().filter(|x| x.status == Status::Held(g)).count();
    let (ha, hb) = (held(a), held(b));
    let named = |world: &World, g: EntityId| {
        leader_of(world, g).map_or_else(|| gname(world, g), |l| format!("{} of {}", world.name_of(l), gname(world, g)))
    };
    let (la, lb) = (named(world, a), named(world, b));
    let border = match line.len() {
        0 => String::new(),
        n => format!(", {n} on the border"),
    };
    let text = format!(
        "{la} and {lb} struck a truce: the line runs where it stands ({ha} blocks and {hb}{border}), held {} days",
        w.truce_days
    );
    let mut actors = vec![a, b];
    actors.extend(leader_of(world, a));
    actors.extend(leader_of(world, b));
    actors.extend(hearers(world, a, None));
    actors.extend(hearers(world, b, None));
    war_line(world, actors, text);
}

/// The day's off-screen rolls: a gang trespasses on a held cell it borders
/// (`provoke_p_day`, the heat of a night), a contested cell is fought over
/// (`front_p_day`), a gang comes back for a cell it lost (`retake_p_day`).
pub(super) fn rolls(world: &mut World, w: &WarCfg) {
    let now = world.tick;
    let mut plan: Vec<(u32, FightKind, EntityId, EntityId)> = Vec::new();
    let mut provokes: Vec<(u32, EntityId, EntityId)> = Vec::new();
    let quiet =
        |world: &World, g: EntityId| lies_low(world, g) || order_of(world, g).is_none_or(|o| o == Order::LieLow);
    for (&k, x) in &world.turf.cells {
        if x.fight.is_some() || x.crackdown_until > now {
            continue;
        }
        match x.status {
            Status::Held(h) => {
                if let Some((l, until)) = x.lost {
                    if until > now
                        && l != h
                        && world.has::<Gang>(l)
                        && !in_truce(world, h, l)
                        && !quiet(world, l)
                        && draw(world, 1, u64::from(k), u64::from(l.index)) < w.retake_p_day
                    {
                        plan.push((k, FightKind::Retake, h, l));
                        continue;
                    }
                }
                let ys: BTreeSet<EntityId> = neighbours(world, k)
                    .iter()
                    .filter_map(|n| world.turf.cells.get(n).and_then(|y| y.status.holder()))
                    .filter(|&y| y != h && world.has::<Gang>(y))
                    .collect();
                for y in ys {
                    if in_truce(world, h, y) || quiet(world, y) {
                        continue;
                    }
                    let hot = matches!(order_of(world, y), Some(Order::Contest | Order::Expand | Order::Retaliate));
                    let p = w.provoke_p_day * if hot { w.provoke_order_mult } else { 1.0 };
                    if draw(world, 2, u64::from(k), u64::from(y.index)) < p {
                        provokes.push((k, h, y));
                        break;
                    }
                }
            }
            Status::Contested(a, b) => {
                if !in_truce(world, a, b)
                    && !quiet(world, a)
                    && !quiet(world, b)
                    && draw(world, 3, u64::from(k), 0) < w.front_p_day
                {
                    plan.push((k, FightKind::Front, a, b));
                }
            }
            Status::Open => {}
        }
    }
    for (k, h, y) in provokes {
        provoke(world, k, h, y, w.heat_stat, None, None, Provoke::Night);
    }
    let due = evening_due(world, w);
    for (k, kind, side, by) in plan {
        open_fight(world, k, kind, side, by, due);
    }
}
