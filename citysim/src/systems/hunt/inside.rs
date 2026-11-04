use super::*;

/// Round 9 Z1, daily over the grudge store: a grudge over a killing heavy
/// enough to hunt whose killer is held (in the cells), not yet told, is
/// told once: handed inside to a cellmate of the holder's own when one is
/// there (`hand_inside`), else the wait's line (`hunts_waiting`). A told
/// grudge whose man is out again is told that (the holder's Hunt free at
/// once, `NOTED_HELD` cleared). A holder under 18 never: `hunter_ok`.
pub(super) fn held_pass(world: &mut World) {
    if world.config.hunt.killed_hunt_min <= 0.0 {
        return;
    }
    #[derive(PartialEq)]
    enum Turn {
        Held,
        Out,
        Left,
    }
    let mut todo: Vec<(EntityId, EntityId, Turn)> = Vec::new();
    // scan-ok: daily, the grudge store.
    for id in world.with::<Grudges>() {
        let Some(g) = world.comp::<Grudges>(id) else { continue };
        for x in g.list.iter().filter(|x| x.settled.is_none() && killing(x.cause).is_some()) {
            if x.noted & NOTED_HELD != 0 {
                if !held(world, x.target) && crate::systems::law::living(world, x.target) {
                    todo.push((id, x.target, Turn::Out));
                }
            } else if x.weight >= min_for(world, x.cause) && world.has::<Sentence>(x.target) {
                todo.push((id, x.target, Turn::Held));
            } else if x.weight >= min_for(world, x.cause)
                && x.noted & NOTED_LEFT == 0
                && !world.hunts.contains_key(&id)
                && crowded(world, id, x.target)
            {
                todo.push((id, x.target, Turn::Left));
            }
        }
    }
    for (id, t, turn) in todo {
        if !hunter_ok(world, id) || !crate::systems::law::living(world, t) {
            continue;
        }
        let why = over(world, id, t);
        let (hn, tn) = (world.name_of(id), world.name_of(t));
        if turn == Turn::Left {
            // Z1: the man has hunters enough (`hunters_per_target`).
            mark(world, id, t, NOTED_LEFT);
            let names: Vec<String> =
                world.hunts.iter().filter(|(_, s)| s.target == t).map(|(&h, _)| world.name_of(h)).take(2).collect();
            let (_, him) = pro(world, t);
            let text = format!("{hn} leaves {tn}{why} to {}, already after {him}", names.join(" and "));
            line(world, &[id], text);
            world.stats.current.word.hunts_handed += 1;
            continue;
        }
        if turn == Turn::Held {
            mark(world, id, t, NOTED_HELD);
            if hand_inside(world, id, t) {
                continue;
            }
            world.stats.current.word.hunts_waiting += 1;
            line(world, &[id], format!("{hn}'s grudge{why} waits: {tn} is in {}", held_where(world, t)));
        } else {
            if let Some(g) = world.comp_mut::<Grudges>(id) {
                if let Some(x) = g.list.iter_mut().find(|x| x.target == t && x.settled.is_none()) {
                    x.noted &= !NOTED_HELD;
                }
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.cooldowns.remove(&GoalKind::Hunt);
            }
            line(world, &[id], format!("{hn} heard {tn} is out of the cells{why}"));
        }
    }
}

/// Z1: a held killer's cellmate of the holder's own (kin, a friend, a gang
/// mate; an adult held in the same cells, never one under 18) takes the
/// grudge inside: an `Inherited` grudge at `hand_w`, the line; the yard
/// does the rest (`inside_mark`). Bounded by the holder's edges and gang.
fn hand_inside(world: &mut World, id: EntityId, t: EntityId) -> bool {
    let c = world.config.hunt.clone();
    if c.inside_fight_p <= 0.0 || c.hand_w <= 0.0 {
        return false;
    }
    let Some(jail) = crate::systems::law::cells_of(world, t) else { return false };
    let Some(g) = crate::systems::grudges::grudge_on(world, id, t) else { return false };
    let w = (g.weight * c.hand_w).min(1.0);
    if w < min_for(world, GrudgeCause::Inherited(id)) {
        return false;
    }
    let mut cands: Vec<EntityId> = world.neighbours.get(&id).map(|n| n.iter().copied().collect()).unwrap_or_default();
    cands.retain(|&o| {
        world
            .edge(id, o)
            .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend))
    });
    if let Some(gang) = world.gang_of(id).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        cands.extend(gang.members.iter().copied());
    }
    let Some((_, o)) = cands
        .into_iter()
        .filter(|&o| o != id && o != t && crate::systems::law::living(world, o))
        .filter(|&o| crate::systems::law::cells_of(world, o) == Some(jail))
        .filter(|&o| crate::systems::demography::is_adult(world, o) && !crate::systems::ages::is_protected(world, o))
        .filter(|&o| takes_against(world, o, t))
        .map(|o| (might_gap(world, o, t), o))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
    else {
        return false;
    };
    let text = format!(
        "{} put word inside {}: {}, held there with {}, will see to it{}",
        world.name_of(id),
        world.name_of(jail),
        world.name_of(o),
        world.name_of(t),
        over(world, id, t)
    );
    line(world, &[id, o, jail], text);
    let chain = g.chain;
    crate::systems::grudges::add(world, o, t, GrudgeCause::Inherited(id), w, chain);
    world.stats.current.word.hunts_handed += 1;
    true
}

/// Z1, from `consequence::cell_day`: the cellmate `p` goes for (a held
/// man he holds a grudge over a killing on, or one handed him, heavy
/// enough to hunt; never one under 18), on `inside_fight_p` a day (`roll`).
pub fn inside_mark(world: &World, p: EntityId, mates: &[EntityId], roll: f32) -> Option<EntityId> {
    let c = world.config.hunt.inside_fight_p;
    if c <= 0.0 || roll >= c || crate::systems::ages::is_protected(world, p) || cooled(world, p) {
        return None;
    }
    world
        .comp::<Grudges>(p)?
        .list
        .iter()
        .filter(|x| x.settled.is_none() && blood(x.cause) && x.weight >= min_for(world, x.cause))
        .filter(|x| mates.contains(&x.target) && !crate::systems::ages::is_protected(world, x.target))
        .max_by(|a, b| a.weight.total_cmp(&b.weight).then(b.target.cmp(&a.target)))
        .map(|x| x.target)
}

/// Z1, from `consequence::cell_day` after a yard fight `p` started on `m`:
/// a win on a grudge over a killing (or one handed him) is the hunt's end
/// inside: the `Avenged` deed and line, the grudge settled (a death) or
/// answered (a beating), the chain.
pub fn cell_strike(world: &mut World, p: EntityId, m: EntityId, winner: EntityId, died: bool, jail: EntityId) {
    // A killing settled the grudge already (`grudges::on_death`).
    let Some(g) = world.comp::<Grudges>(p).and_then(|gs| {
        gs.list.iter().rev().find(|x| x.target == m && blood(x.cause) && (died || x.settled.is_none())).cloned()
    }) else {
        return;
    };
    if winner != p {
        // A loss holds him off as a lost Hunt does (no fight a day on it).
        cool(world, p);
        return;
    }
    let now = world.tick;
    world.kill_chain.insert(m, (g.chain.saturating_add(1), now));
    let d = world.district_of_building(jail);
    crate::systems::gossip::post_deed(world, d, Deed::Avenged, Some(p), Some(m));
    let text = format!(
        "{} avenged {} on {} in {}{}",
        world.name_of(p),
        for_whom(world, Some(g.cause)),
        world.name_of(m),
        world.name_of(jail),
        if died { " (killed)" } else { "" }
    );
    world.push_event(EventKind::Avenged, &[p, m, jail], text);
    if died {
        if let Some(gs) = world.comp_mut::<Grudges>(p) {
            if let Some(x) = gs.list.iter_mut().find(|x| x.target == m && x.settled.is_none()) {
                x.settled = Some(now);
            }
        }
    } else {
        crate::systems::grudges::answer_beating(world, p, m);
        answered(world, p, m, killing(g.cause));
    }
    let w = &mut world.stats.current.word;
    w.avenged += 1;
    w.hunts_ended += 1;
    if died {
        w.revenge_kills += 1;
    }
}
